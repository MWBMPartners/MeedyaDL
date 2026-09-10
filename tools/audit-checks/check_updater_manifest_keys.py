#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Updater manifest platform keys: one list, in one place.

WHAT `latest.json` IS
---------------------
`latest.json` is the small file the app downloads to find out whether a
newer version exists and, if so, which file to fetch for this exact
machine. Each machine looks itself up in that file by a short name — a
"platform key" — such as `linux-x86_64-deb` or `windows-aarch64`. If a
machine's key is missing, that machine is simply never offered an
update. Nothing fails, nothing is logged, nobody notices.

WHY THIS CHECK EXISTS
---------------------
The list of platform keys used to be typed out by hand in three separate
places: the builder inside `release.yml`, a second checker further down
the same file, and a third copy inside the repair tool
`fix-updater-manifest.yml`. On 2026-09-10 six Linux `.deb`/`.rpm` keys
were added to one of those copies and not the other two. The result was
a repair tool that would have DELETED six working update paths from any
release it was pointed at — and a checker that would have called the
result "complete", because the checker's idea of "complete" was written
from the same six names as the thing it was checking.

That is the shape this check is built to prevent. A list written down by
hand in more than one place drifts, and a checker written from the same
hand as the thing it checks will agree with that hand forever.

After the fix there is exactly ONE list, `manifest_rows()` in
`scripts/release/updater-manifest.sh`. So the rule this script enforces
is simple and total:

    No workflow file may name an updater platform key in code at all.

THE TWO DESIGN POINTS THAT MATTER
---------------------------------
1. **Every hardcoded key is reported, not only unrecognised ones.**
   A check that only complained about keys it did not recognise would
   have sailed straight past the original bug, because the bug was a
   loop over six keys that were all perfectly real. The problem was
   never that the names were wrong. It was that they were written down
   somewhere a person has to remember to update.

2. **An empty or unreadable canonical list is itself a finding.**
   If `manifest_rows()` is renamed, moved or restructured, this script
   suddenly has nothing to compare against. It must say so loudly rather
   than print a reassuring tick over an empty check — quietly agreeing
   with a belief instead of testing reality is exactly how the original
   bug survived as long as it did.

WHAT COUNTS AS A KEY
--------------------
An operating system name (`darwin`, `windows` or `linux`), a dash, an
architecture, and optionally another dash and the installer format
(`-deb`, `-rpm`, `-nsis`). That is the shape the Tauri updater asks for.

The architecture deliberately is NOT a fixed list of the three we ship
today. Writing `aarch64|x86_64|armv7` here would mean a genuinely new
platform — the exact thing most likely to be added to one workflow and
forgotten in the other — sailed straight past this check. Instead an
architecture is recognised as any run of letters, digits and
underscores that contains at least one letter AND at least one digit
(plus `universal`, the one real architecture name with no digit in it).

That rule is what keeps GitHub's own runner labels out of the results:
`windows-latest` has no digit, `windows-2022` has no letter, and the
`linux-gnu` / `windows-msvc` halves of a Rust target triple have no
digit either. All four are extremely common in workflow files and none
of them is an updater key.

Lines whose first non-blank character is `#` are skipped. Every long
explanatory block in these workflows is written as whole-line comments,
so in practice this is accurate. A trailing `# ... linux-x86_64 ...` on
the end of a real line would still be reported; that is a deliberate
trade of a rare false positive (cost: one exceptions entry with a
written reason) against writing a bash parser.

Exit code:
  0 — no findings, OR findings without --strict
  1 — at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_updater_manifest_keys.py [--strict]
  python3 tools/audit-checks/check_updater_manifest_keys.py --root <dir>
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Where the one and only list of platform keys lives, relative to the
# repo root. Kept as a constant so the "we cannot find it any more"
# finding below can name it precisely.
CANONICAL_SCRIPT = Path("scripts") / "release" / "updater-manifest.sh"

# Where workflow files live, relative to the repo root.
WORKFLOW_DIR = Path(".github") / "workflows"

# An updater platform key: an operating system, an architecture, and
# optionally the installer format that goes with it.
#
# The architecture half is a shape, not a list — see the module docstring
# for why. It is "a run of letters, digits and underscores containing at
# least one letter and at least one digit", spelled here as two
# lookaheads, or the literal word `universal` (macOS universal binaries,
# the one architecture name with no digit in it).
#
# The two lookaheads cannot run past a dash, because a dash is not in
# `[a-z0-9_]`. That is what stops `windows-latest-2022` from borrowing
# the digit out of the following word.
PLATFORM_KEY_RE = re.compile(
    r"\b(?:darwin|windows|linux)-"
    r"(?:universal|(?=[a-z0-9_]*[a-z])(?=[a-z0-9_]*[0-9])[a-z0-9_]+)"
    r"(?:-[a-z0-9]+)?\b"
)

# Finds the `manifest_rows()` shell function and the here-document inside
# it. The delimiter is captured rather than assumed to be `EOF`, so
# renaming it does not silently blind this check.
MANIFEST_ROWS_RE = re.compile(
    r"^manifest_rows\s*\(\s*\)\s*\{", re.MULTILINE
)
HEREDOC_START_RE = re.compile(r"<<-?\s*[\"']?([A-Za-z_][A-Za-z0-9_]*)[\"']?\s*$")

# Known, checked exceptions.
#
# Shape: "<repo-relative path>:<platform key>" -> a written reason.
#
# EMPTY IS THE CORRECT STARTING STATE. After the shared script landed,
# no workflow legitimately names a platform key in executable code. Add
# an entry only when a real, reasoned case turns up — never to quiet a
# finding you have not understood. Every entry must say WHY, in a
# sentence someone else can check.
EXCEPTIONS: dict[str, str] = {}


def _read_text(path: Path) -> str | None:
    """Read a file, returning None rather than raising if it cannot be
    read. A missing or unreadable canonical script is a finding we want
    to report properly, not a stack trace."""
    try:
        return path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return None


def canonical_keys(root: Path) -> tuple[set[str], int | None]:
    """Pull the platform keys out of `manifest_rows()` in the shared
    release script.

    Returns `(keys, line_number_of_manifest_rows)`. The line number is
    None when the function could not be found at all, so the caller can
    point its finding at something sensible.

    Each row in the here-document is `<sig file>|<download file>|<keys>`,
    and the third field can hold more than one key separated by spaces
    (the two Windows rows do, because the updater asks for the `-nsis`
    name first and falls back to the bare one).
    """
    script_path = root / CANONICAL_SCRIPT
    text = _read_text(script_path)
    if text is None:
        return set(), None

    match = MANIFEST_ROWS_RE.search(text)
    if match is None:
        return set(), None

    start_line = text[: match.start()].count("\n") + 1
    lines = text.splitlines()

    keys: set[str] = set()
    delimiter: str | None = None

    # Walk forward from the function's opening line, collecting the rows
    # between the here-document's start marker and its terminator.
    for raw in lines[start_line:]:
        stripped = raw.strip()
        if delimiter is None:
            heredoc = HEREDOC_START_RE.search(stripped)
            if heredoc is not None:
                delimiter = heredoc.group(1)
            elif stripped == "}":
                # Function ended before any here-document appeared.
                break
            continue

        if stripped == delimiter:
            break

        if not stripped:
            continue

        fields = stripped.split("|")
        if len(fields) < 3:
            continue
        for key in fields[2].split():
            keys.add(key)

    return keys, start_line


def scan_workflows(root: Path) -> list[tuple[str, int, str]]:
    """Find every updater platform key named in workflow code.

    Returns `[(repo-relative path, line number, key)]` in file order,
    with one entry per occurrence so a loop over six keys reports six
    findings rather than one vague one.
    """
    findings: list[tuple[str, int, str]] = []
    workflow_dir = root / WORKFLOW_DIR
    if not workflow_dir.is_dir():
        return findings

    paths = sorted(
        p for p in workflow_dir.iterdir()
        if p.is_file() and p.suffix in (".yml", ".yaml")
    )
    for path in paths:
        text = _read_text(path)
        if text is None:
            continue
        rel = path.relative_to(root).as_posix()
        for line_no, line in enumerate(text.splitlines(), start=1):
            # Whole-line comments carry the long explanations in these
            # files, and an explanation naming a key is not drift.
            if line.lstrip().startswith("#"):
                continue
            for key in PLATFORM_KEY_RE.findall(line):
                if f"{rel}:{key}" in EXCEPTIONS:
                    continue
                findings.append((rel, line_no, key))
    return findings


def check(root: Path, strict: bool) -> int:
    """Run the check against a repository root and print the result in
    the house format. `root` is a parameter rather than a hard-coded
    constant purely so the negative test can point this at a small,
    deliberately-broken fake tree."""
    keys, rows_line = canonical_keys(root)
    hardcoded = scan_workflows(root)

    print(f"Platform keys defined in {CANONICAL_SCRIPT.as_posix()} : {len(keys)}")
    print(f"Platform keys hardcoded in workflow code            : {len(hardcoded)}")
    print()

    bullets: list[str] = []

    # The blinded-check finding comes first, because if this fires then
    # everything below it is being judged against nothing.
    if not keys:
        where = f"{CANONICAL_SCRIPT.as_posix()}:{rows_line or 1}"
        bullets.append(
            f"  • {where} — could not read any platform keys from manifest_rows(); "
            f"this check has nothing to compare against and is silently passing. "
            f"Either the shared release script moved or was renamed, or the list "
            f"inside it changed shape. Fix that before trusting any other line here."
        )

    for rel, line_no, key in hardcoded:
        recognised = (
            "one of the keys that list already carries"
            if key in keys
            else "not even a key that list carries"
        )
        bullets.append(
            f"  • {rel}:{line_no} — hardcodes updater platform key '{key}' ({recognised}); "
            f"the only place these may be listed is manifest_rows() in "
            f"{CANONICAL_SCRIPT.as_posix()}, or the two lists drift and nothing notices"
        )

    if bullets:
        # The `### ` heading is not decoration. `pr-security.yml` extracts
        # this output with `grep -A100 '###'` before surfacing it, so a
        # script that prints bullets without a heading has every finding
        # thrown away with nothing reporting that it happened.
        print("### Updater manifest platform keys hardcoded outside the shared script\n")
        for bullet in bullets:
            print(bullet)
        print()
    else:
        print(
            "OK — every updater platform key comes from "
            f"{CANONICAL_SCRIPT.as_posix()}."
        )

    if bullets and strict:
        return 1
    return 0


def main(argv: list[str]) -> int:
    strict = "--strict" in argv

    root = REPO_ROOT
    if "--root" in argv:
        idx = argv.index("--root")
        if idx + 1 >= len(argv):
            print("::error::--root needs a directory after it.")
            return 2
        root = Path(argv[idx + 1]).resolve()

    return check(root, strict)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
