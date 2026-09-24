#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Build-time secret wiring check.

Several features are switched on by a value supplied when the app is built,
rather than by a setting a user can see. The Rust side reads them with
`option_env!("NAME")`; the frontend reads them as `import.meta.env.VITE_NAME`.
Both treat a missing value as "this feature is not configured" and then, by
design, say nothing at all.

That silence is correct for someone building their own copy. It is dangerous
for us, because it means a feature can be complete, tested, shipped, and
completely inert in every build a user could install, with nothing anywhere
reporting a problem.

That is not hypothetical. On 2026-09-08 a sweep found three separate features
in exactly that state, all shipped, none ever working once:

  * crash reporting             — SENTRY_DSN / VITE_SENTRY_DSN never set (#1161)
  * the remote pause switch     — the three INTAPPS_* values never set (#1163)
  * developer access            — DEV_ACCESS_HASH never set, which also left
                                  the gate openable with an empty passphrase
                                  (#1162)

Each had been reviewed and merged. Nothing failed. Nobody could have noticed.

This check closes that gap. It reads every build-time value the code actually
asks for, and confirms each one is either passed through by the release
workflow or listed below as deliberately not needed. A new one added without
either lands as a finding at review time, when someone is already looking.

CHECKED PER BUILD STEP, NOT JUST "SOMEWHERE IN THE FILE"
----------------------------------------------------------
`release.yml` does not build the app once. It builds it three separate
times, each with its own `env:` block: "Build and publish" (Windows x64,
Windows ARM64, Linux x64, Linux ARM64), "Build macOS with notarization
retry" (macOS), and "Build ARMv7" (Linux ARMv7). Those three, and only
those three, actually invoke the compiler/bundler (`tauri-action` or
`npx tauri build`) and produce the binary a build-time value gets baked
into — every other step in this file (uploading artifacts, notarising the
macOS disk image, rebuilding the updater manifest, ...) never builds the
app, so listing a value there would do nothing regardless.

An earlier version of this check only asked "does this name appear
ANYWHERE in release.yml", which is true today because all nine values
happen to be copy-pasted into all three `env:` blocks. But that check
would have said "fine" the moment someone added a TENTH value to only the
first block — exactly the shape of the original bug (#1161/#1162/#1163):
a finished, reviewed feature that ships completely inert, this time only
on macOS (the platform with the most users) and ARMv7, because the value
never reached the `env:` block that step actually reads from.

Two values — MEEDYADL_CHROME_MAJOR and MEEDYADL_SAFARI_VERSION — are
deliberately NOT repeated in any of the three `env:` blocks, and that is
correct, not a gap: they are resolved once, earlier in the same job, and
exported with `echo "NAME=value" >> "$GITHUB_ENV"`, which GitHub Actions
then makes visible to every later step in that same job automatically —
see the "Chrome major version resolution" note in `release.yml` itself.
This check treats a value as wired to a build step if EITHER it is a key
in that step's own `env:` block, OR it was exported to `$GITHUB_ENV`
earlier in the same job — never by trusting that the name merely appears
somewhere in the file, which is exactly the looseness that would hide a
value wired to only one of the three.

An export only counts if it will actually run whenever the build does:
it must not be commented out, and the step it sits in must have no `if:`
condition of its own, or exactly the same condition as the build step.
An export under any other condition is reported as "cannot confirm", not
assumed to work, because this check cannot evaluate GitHub's expression
language. (The first per-step version counted any line shaped like an
export. Codex showed in the batch-4 review that commenting out the only
Safari export still printed "OK". test_check_build_secrets.py now pins
this, and fails on that old behaviour.)

What it does NOT do: it cannot tell whether a secret has a value on GitHub —
that needs repository admin rights the CI job does not have, and should not
have. It only checks the wiring, which is the half that lives in the
repository and the half that was missing all three times.

Exit code:
  0 — no findings, OR findings without --strict
  1 — at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_build_secrets.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUST_SRC = ROOT / "src-tauri" / "src"
WEB_SRC = ROOT / "src"
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"

# Values the release build is deliberately not expected to supply, each with
# the reason. Anything here is skipped; anything not here must be wired.
#
# Keep this list short and keep the reasons honest. "We haven't got round to
# it" is not a reason to add an entry — that is exactly the state this check
# exists to make visible.
INTENTIONALLY_UNSET: dict[str, str] = {}

# Values read by the Rust side via `option_env!("NAME")`.
OPTION_ENV = re.compile(r'option_env!\(\s*"([A-Z0-9_]+)"\s*\)')

# Values read by the frontend as `import.meta.env.VITE_NAME`. The bare prefix
# `VITE_` shows up in comments and type declarations, so require at least one
# character after it.
VITE_ENV = re.compile(r"import\.meta\.env\.(VITE_[A-Z0-9_]+)")

# The three steps in release.yml that actually compile/bundle the app -- see
# the "CHECKED PER BUILD STEP" section of the module docstring for why these
# three, and only these three, are the ones a build-time value has to reach.
# This is a short, named, reviewable list on purpose (the same shape as
# CRASH_REPORT_HOSTS and INTENTIONALLY_UNSET below) rather than "every step
# whose name contains the word Build" -- release.yml also has steps like
# "Rebuild updater manifest (latest.json)" that build nothing an app secret
# would ever need to reach. If one of these three steps is ever renamed,
# this list has to be updated to match -- and check() below says so loudly
# rather than silently checking nothing, see _find_build_steps().
BUILD_STEP_NAMES = (
    "Build and publish",
    "Build macOS with notarization retry",
    "Build ARMv7",
)

# A GitHub Actions job header -- a bare `  job-id:` at exactly two spaces of
# indentation. Only matched after the top-level `jobs:` key (see
# _find_job_ranges()), because `on:` also has two-space-indented children
# ("push:", "workflow_dispatch:") that are not jobs and must not be read as
# one.
_JOB_HEADER_RE = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")

# A GitHub Actions step's `- name: ...` line, capturing its own indentation
# (so a later line can be recognised as "still inside this step" by being
# more indented) and the step's name.
_STEP_NAME_RE = re.compile(r"^(\s*)-\s*name:\s*(.+?)\s*$")

# A step's `env:` block header, same indentation-capture idea.
_ENV_BLOCK_RE = re.compile(r"^(\s*)env:\s*$")

# One `NAME: value` line inside an env: block.
_ENV_KEY_RE = re.compile(r"^\s*([A-Za-z0-9_]+):")

# A shell line that exports a build-time value into every later step of the
# same job -- `echo "NAME=..." >> "$GITHUB_ENV"` (with or without the quotes
# around $GITHUB_ENV; release.yml uses the quoted form). See the "Chrome
# major version resolution" note in release.yml itself for why this exists:
# a value resolved once, early in the job, does not need to be typed again
# into each build step's own env: block to reach that step.
_GITHUB_ENV_EXPORT_RE = re.compile(r'echo\s+"([A-Z0-9_]+)=.*?"\s*>>\s*"?\$GITHUB_ENV"?')

# A step's own `if:` key, capturing its indentation so _step_if() can tell
# the step's condition apart from an `if` line inside its shell script.
_STEP_IF_RE = re.compile(r"^(\s*)if:\s*(.+?)\s*$")


def _find_job_ranges(lines: list[str]) -> list[tuple[str, int, int]]:
    """Returns `[(job_id, start_line, end_line)]`, 1-indexed, `end_line`
    exclusive -- the line range each top-level job occupies in the workflow
    file. Only looks after the `jobs:` key, so `on:`'s own two-space-indented
    children are never mistaken for jobs."""
    jobs_at = None
    for i, line in enumerate(lines, start=1):
        if line == "jobs:":
            jobs_at = i
            break
    if jobs_at is None:
        return []

    starts: list[tuple[str, int]] = []
    for i in range(jobs_at + 1, len(lines) + 1):
        m = _JOB_HEADER_RE.match(lines[i - 1])
        if m:
            starts.append((m.group(1), i))

    ranges: list[tuple[str, int, int]] = []
    for idx, (job_id, start) in enumerate(starts):
        end = starts[idx + 1][1] if idx + 1 < len(starts) else len(lines) + 1
        ranges.append((job_id, start, end))
    return ranges


def _step_ranges(
    lines: list[str], start: int, end: int
) -> list[tuple[str, int, int, int]]:
    """Within 1-indexed line range [start, end), returns one entry per
    `- name: ...` step: `(step_name, dash_indent, step_start, step_end)`,
    `step_end` exclusive. A step's range runs until the next line at an
    indentation no deeper than its own dash -- the next sibling step, or the
    point control leaves the `steps:` list (a shallower job-level key, or
    the next job)."""
    steps: list[tuple[str, int, int, int]] = []
    i = start
    while i < end:
        line = lines[i - 1]
        m = _STEP_NAME_RE.match(line)
        if not m:
            i += 1
            continue
        indent = len(m.group(1))
        name = m.group(2)
        j = i + 1
        while j < end:
            nxt = lines[j - 1]
            if nxt.strip() == "":
                j += 1
                continue
            nxt_indent = len(nxt) - len(nxt.lstrip(" "))
            if nxt_indent <= indent:
                break
            j += 1
        steps.append((name, indent, i, j))
        i = j
    return steps


def _step_env_names(
    lines: list[str], step_indent: int, step_start: int, step_end: int
) -> set[str]:
    """The set of `NAME:` keys inside this step's own `env:` block, if it has
    one. A commented-out line (its first non-blank character is `#`) is
    never read as a key -- these blocks carry long explanatory comments, not
    code, and a comment mentioning a name is not the same claim as an actual
    `env:` entry."""
    names: set[str] = set()
    i = step_start
    while i < step_end:
        m = _ENV_BLOCK_RE.match(lines[i - 1])
        if m and len(m.group(1)) > step_indent:
            env_indent = len(m.group(1))
            j = i + 1
            while j < step_end:
                inner = lines[j - 1]
                if inner.strip() == "":
                    j += 1
                    continue
                inner_indent = len(inner) - len(inner.lstrip(" "))
                if inner_indent <= env_indent:
                    break
                if not inner.strip().startswith("#"):
                    km = _ENV_KEY_RE.match(inner)
                    if km:
                        names.add(km.group(1))
                j += 1
            break
        i += 1
    return names


def _find_build_steps(
    lines: list[str],
) -> tuple[dict[str, tuple[int, int, int, int]], dict[str, int]]:
    """Locates each of BUILD_STEP_NAMES in the workflow file.

    Returns `(found, dupes)`:
      * `found`: `{step_name: (job_start, step_indent, step_start, step_end)}`
        for every build step name that was located exactly once.
        `job_start` is included so the caller can look for a $GITHUB_ENV
        export anywhere earlier in the SAME job -- a value exported by a
        different job never reaches this one, since each job runs on its
        own fresh runner.
      * `dupes`: `{step_name: count}` for a name found more than once (which
        this check cannot resolve -- it would not know which occurrence is
        the real build step), so the caller can report it rather than
        silently picking one.
    """
    found: dict[str, tuple[int, int, int, int]] = {}
    dupes: dict[str, int] = {}
    for job_id, jstart, jend in _find_job_ranges(lines):
        for name, indent, sstart, send in _step_ranges(lines, jstart, jend):
            if name not in BUILD_STEP_NAMES:
                continue
            if name in found or name in dupes:
                dupes[name] = dupes.get(name, 1) + 1
                continue
            found[name] = (jstart, indent, sstart, send)
    return found, dupes


def _step_if(lines: list[str], step_indent: int, step_start: int, step_end: int) -> str | None:
    """This step's own `if:` condition, whitespace-collapsed, or None if it
    has none. Only a key at the step's own key depth counts (two spaces
    deeper than its dash), so an `if` inside its shell script is not
    mistaken for the step's condition."""
    want = step_indent + 2
    for i in range(step_start, step_end):
        line = lines[i - 1]
        m = _STEP_IF_RE.match(line)
        if m and len(m.group(1)) == want:
            return " ".join(m.group(2).split())
    return None


def _github_env_exports_before(
    lines: list[str], job_start: int, build_step: tuple[int, int, int, int]
) -> tuple[set[str], dict[str, list[str]]]:
    """Names exported via `echo "NAME=..." >> "$GITHUB_ENV"` by earlier
    steps of the same job, split by whether they are CERTAIN to reach the
    build step.

    Returns `(certain, uncertain)`:
      * `certain`: exported by a step that runs whenever the build step
        does -- one with no `if:` of its own, or with exactly the same
        `if:` as the build step.
      * `uncertain`: `{NAME: ["'step name' (if: condition)", ...]}` for
        names exported only by a step with some OTHER condition. This check
        cannot evaluate GitHub's expression language, so it cannot tell
        whether that step runs; it reports it rather than assuming.

    # Why it works step by step, not line by line

    The first version read every line between the start of the job and the
    build step, and counted any line shaped like an export. So a
    commented-out export counted, and so did one in a step with
    `if: false`, which never runs. Codex proved both in the batch-4 review:
    with the only Safari-version export commented out, `--strict` still
    printed "OK" and exited 0 -- the exact silent pass this whole script
    exists to prevent.

    # What it still cannot see

    Only steps with a `- name:` line are read (the same limit as
    _step_ranges). An export inside an unnamed step is missed -- but that
    produces a loud "not supplied" finding, never a false "fine". Nor does
    it look inside the shell: an export wrapped in the script's own `if`
    (the Chrome and Safari steps export only when their lookup worked) is
    counted, because the app falls back to a compiled-in value when that
    export is absent, by design.
    """
    scan_end = build_step[2]  # stop where the build step itself begins
    build_indent, build_start, build_end = build_step[1], build_step[2], build_step[3]
    build_if = _step_if(lines, build_indent, build_start, build_end)

    certain: set[str] = set()
    uncertain: dict[str, list[str]] = {}
    for step_name, indent, sstart, send in _step_ranges(lines, job_start, scan_end):
        exported: set[str] = set()
        for i in range(sstart, send):
            line = lines[i - 1]
            # A shell comment and a YAML comment look the same here, and
            # neither runs.
            if line.lstrip().startswith("#"):
                continue
            m = _GITHUB_ENV_EXPORT_RE.search(line)
            if m:
                exported.add(m.group(1))
        if not exported:
            continue
        cond = _step_if(lines, indent, sstart, send)
        if cond is None or cond == build_if:
            certain |= exported
        else:
            for name in exported:
                uncertain.setdefault(name, []).append(f"'{step_name}' (if: {cond})")
    # A name also exported unconditionally somewhere is certain; the
    # conditional copy does not make it less so.
    for name in certain:
        uncertain.pop(name, None)
    return certain, uncertain


def _iter_source(root: Path, suffixes: tuple[str, ...]):
    """Yield (path, text) for every source file under `root`, tests excluded.

    Test files are skipped because a test may legitimately reference a value
    the shipped app does not read.
    """
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix not in suffixes:
            continue
        name = path.name
        if ".test." in name or ".spec." in name or name.endswith("_test.rs"):
            continue
        yield path, path.read_text(encoding="utf-8", errors="ignore")


def _find_reads() -> dict[str, list[str]]:
    """Collect every build-time value the code reads, and where it reads it."""
    reads: dict[str, list[str]] = {}

    def record(name: str, path: Path, line_no: int) -> None:
        rel = path.relative_to(ROOT).as_posix()
        reads.setdefault(name, []).append(f"{rel}:{line_no}")

    for path, text in _iter_source(RUST_SRC, (".rs",)):
        for line_no, line in enumerate(text.splitlines(), start=1):
            for match in OPTION_ENV.finditer(line):
                record(match.group(1), path, line_no)

    for path, text in _iter_source(WEB_SRC, (".ts", ".tsx")):
        for line_no, line in enumerate(text.splitlines(), start=1):
            for match in VITE_ENV.finditer(line):
                record(match.group(1), path, line_no)

    return reads


# Where crash reports can be sent to.
#
# The app's own security rules list which addresses the page is allowed to
# contact. A reporting address that is not on that list is refused by the
# WebView, silently — the report never leaves, nothing appears in the log, and
# the person who switched reporting on has no way to tell (#998).
#
# So the two have to agree: if the app can be built with a reporting address,
# the security rules must permit reaching it.
#
# Keeping the list short is a feature, not an inconvenience. It means a
# reporting address that arrives from anywhere else — a mistake, or a tampered
# lookup — is refused rather than quietly sending crash reports, which contain
# stack traces, somewhere nobody chose.
CRASH_REPORT_HOSTS = (
    "app.glitchtip.com",  # the hosted service we use
    "sentry.mwbm.cloud",  # reserved for running it ourselves later
)

TAURI_CONF = ROOT / "src-tauri" / "tauri.conf.json"


def check_crash_report_hosts_are_reachable() -> list[str]:
    """Confirms the security rules permit reaching the crash-report addresses.

    Returns:
        One finding per address the rules would block, empty if all are fine.
    """
    if not TAURI_CONF.exists():
        return [f"{TAURI_CONF.relative_to(ROOT).as_posix()} — not found"]

    import json

    try:
        config = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return [f"{TAURI_CONF.relative_to(ROOT).as_posix()} — could not be read: {exc}"]

    rules = config.get("app", {}).get("security", {}).get("csp") or ""
    rel = TAURI_CONF.relative_to(ROOT).as_posix()
    return [
        f"{rel} — crash reports can be sent to {host}, but the app's security rules "
        f"do not allow the page to contact it, so reports would be refused with no "
        f"sign to the user"
        for host in CRASH_REPORT_HOSTS
        if host not in rules
    ]


def check() -> int:
    if not RELEASE_WORKFLOW.exists():
        print(f"  • {RELEASE_WORKFLOW.relative_to(ROOT)} — release workflow not found")
        return 1 if "--strict" in sys.argv else 0

    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8", errors="ignore")
    lines = workflow.splitlines()
    rel_workflow = RELEASE_WORKFLOW.relative_to(ROOT).as_posix()
    reads = _find_reads()

    if not reads:
        print("OK — no build-time values are read by the app.")
        return 0

    build_steps, dupes = _find_build_steps(lines)
    unresolved = [name for name in BUILD_STEP_NAMES if name not in build_steps]

    if unresolved or dupes:
        # This is the "blinded check" case -- see check_updater_manifest_keys.py
        # for the same shape. If a build step got renamed, this script has
        # nothing correct to compare against for it: reporting "OK" here would
        # be agreeing with a guess, not a finding, and that is exactly the
        # failure shape this whole file of checks exists to avoid ("could not
        # check" must never look like "checked and found nothing"). So this
        # stops here rather than silently falling back to a looser check that
        # would hide precisely the bug this script exists to catch.
        print("### Could not verify per-build-step wiring in release.yml")
        print()
        for name in unresolved:
            print(
                f"  • {rel_workflow} — could not find a step named '{name}'. "
                f"BUILD_STEP_NAMES in this script no longer matches release.yml "
                f"-- if this step was renamed, update BUILD_STEP_NAMES to match; "
                f"until then, this check cannot confirm any build-time value "
                f"reaches this build."
            )
        for name, extra_count in dupes.items():
            print(
                f"  • {rel_workflow} — found {extra_count + 1} steps named "
                f"'{name}', not one. This check cannot tell which is the real "
                f"build step, so it cannot confirm any build-time value "
                f"reaches it."
            )
        print()
        if "--strict" in sys.argv:
            return 1
        return 0

    # For each build step, the names it can actually see at compile time:
    # its own `env:` block, plus anything exported to $GITHUB_ENV earlier in
    # the same job (see the module docstring's "CHECKED PER BUILD STEP"
    # section for why the second half of that union has to exist too).
    wired_per_step: dict[str, set[str]] = {}
    # (value, build step) -> the conditional steps that export it. Only
    # used to explain a finding: such a value is NOT counted as wired.
    unconfirmed: dict[tuple[str, str], list[str]] = {}
    for name, (job_start, indent, sstart, send) in build_steps.items():
        certain, uncertain = _github_env_exports_before(
            lines, job_start, (job_start, indent, sstart, send)
        )
        wired_per_step[name] = _step_env_names(lines, indent, sstart, send) | certain
        for value, sources in uncertain.items():
            if value not in wired_per_step[name]:
                unconfirmed[(value, name)] = sources

    fully_missing: list[tuple[str, list[str]]] = []
    partially_missing: list[tuple[str, list[str], list[str]]] = []
    for name, sites in sorted(reads.items()):
        if name in INTENTIONALLY_UNSET:
            continue
        lacking = [step for step in BUILD_STEP_NAMES if name not in wired_per_step[step]]
        if not lacking:
            continue
        if len(lacking) == len(BUILD_STEP_NAMES):
            fully_missing.append((name, sites))
        else:
            partially_missing.append((name, sites, lacking))

    if fully_missing or partially_missing:
        # The heading matters, and not only cosmetically. `pr-security.yml`
        # extracts a check's findings with `grep -A100 '###'` before handing
        # them to `add_section`, which silently drops an empty body. A script
        # that prints bullets but no `###` line therefore has every finding
        # discarded, with nothing reporting that it happened — which is exactly
        # the class of failure this script exists to catch, and exactly what
        # this script did on the day it was written. Every other check in this
        # directory emits one; so does this.
        print("### Build-time value not supplied to every build step in release.yml")
        print()
        if fully_missing:
            print("Read by the app, but set in none of the three build steps:")
            print()
            for name, sites in fully_missing:
                print(f"  • {sites[0]} — {name} is read here but set nowhere in release.yml")
                for extra in sites[1:]:
                    print(f"      also read at {extra}")
            print()
        if partially_missing:
            print(
                "Read by the app, and set for SOME build steps but not others -- "
                "this is the shape that ships a feature working on some platforms "
                "and silently inert on others:"
            )
            print()
            for name, sites, lacking in partially_missing:
                have = [s for s in BUILD_STEP_NAMES if s not in lacking]
                print(
                    f"  • {sites[0]} — {name} reaches {', '.join(have)}, but not "
                    f"{', '.join(lacking)}"
                )
                for extra in sites[1:]:
                    print(f"      also read at {extra}")
            print()
        notes = [
            (value, step, sources)
            for (value, step), sources in sorted(unconfirmed.items())
            if any(value == n for n, *_ in fully_missing + partially_missing)
        ]
        if notes:
            print(
                "Some of the above ARE exported earlier in the job, but only by a step "
                "with its own `if:` condition. This check cannot work out whether that "
                "step runs, so it does not count it:"
            )
            print()
            for value, step, sources in notes:
                print(f"      {value} for '{step}': only via {', '.join(sources)}")
            print()
        print("  Each of these makes a feature silently inert on the affected build(s).")
        print("  Either add the value to every one of the three build steps' env:")
        print("  blocks in release.yml (or export it to $GITHUB_ENV earlier in the")
        print("  same job, before all three), or add it to INTENTIONALLY_UNSET in")
        print("  this script with the reason it is not needed.")
        print()
        if "--strict" in sys.argv:
            return 1
        return 0

    blocked_hosts = check_crash_report_hosts_are_reachable()
    if blocked_hosts:
        print("### Crash reports could not be delivered — the security rules block the address\n")
        for finding in blocked_hosts:
            print(f"  • {finding}")
        print()
        if "--strict" in sys.argv:
            return 1
        return 0

    total = len(reads)
    skipped = len(INTENTIONALLY_UNSET)
    print(
        f"OK — all {total - skipped} build-time value(s) the app reads are supplied "
        f"by the release build."
    )
    return 0


if __name__ == "__main__":
    sys.exit(check())
