#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Tauri npm package <-> Rust crate version sync check.

Every Tauri plugin ships as TWO halves that must agree: a Rust crate that
does the work in the backend, and an npm package that exposes it to the
frontend over IPC. The Tauri CLI refuses to build when the two halves are on
different major.minor releases, because the JS binding and the Rust command
signatures are only guaranteed compatible within a minor line.

The CLI's own words (verified by reading the shipped binary and the upstream
source of `crates/tauri-cli/src/info/plugins.rs`):

    Found version mismatched Tauri packages. Make sure the NPM package and
    Rust crate versions are on the same major/minor releases:

and the predicate behind it is literally:

    crate_version.major != npm_version.major
      || crate_version.minor != npm_version.minor

So PATCH differences are tolerated by design (crate 2.4.10 vs npm 2.4.9 is
fine); only major.minor has to line up. This script reproduces exactly that
comparison.

WHY THIS EXISTS (the v1.10.5 incident)
--------------------------------------
`src-tauri/Cargo.toml` pins the Tauri crates loosely as version "2", so any
step that regenerates `Cargo.lock` silently re-resolves them upward. The
release PR's own "chore(release): update Cargo.lock" step did exactly that on
`main`, moving `tauri-plugin-notification` to 2.4.0 and
`tauri-plugin-updater` to 2.11.0 while the npm halves stayed pinned by
`package-lock.json` at 2.3.3 and 2.10.1.

`dependabot.yml` routes npm version updates to the `alpha` branch only, so
the matching npm bumps landed on `alpha` and never reached `main`. `alpha`
had zero mismatches; the drift was `main`-only and therefore invisible to
everyone actually developing.

Nothing caught it until `tauri build` ran — which is AFTER the tag was cut
and after a stable GitHub Release object already existed. All six platform
builds failed, and the empty Release sat in the "Latest" slot offering users
an update with no assets to download.

This check is the fast, no-build guard that moves that failure from release
time to PR time. It reads two lockfiles and compares numbers; it needs no
`npm ci`, no `cargo build`, and no network.

THE DISQUALIFYING BUG THIS REWRITE FIXES
-----------------------------------------
An earlier draft of this script printed a WARNING and returned exit 0 the
moment `package-lock.json` was missing or malformed — EVEN UNDER --strict.
A guard that silently reports success when it never actually performed the
comparison it exists to perform is worse than no guard at all: it would have
green-ticked the v1.10.5 release PR just as confidently as a genuinely clean
one. "I found nothing to compare" must never be indistinguishable from "I
compared everything and found no drift". This script now treats every case
where the comparison could not actually run — a missing or unparseable
`package-lock.json`, a `package-lock.json` with no `packages` map, a missing
or unparseable `Cargo.lock`, or a parseable pair of files that nonetheless
yields ZERO comparable npm/crate pairs — as a hard, `--strict`-failing
finding of its own, distinct from (and reported the same way as) an actual
version mismatch.

WHAT IS COMPARED
----------------
Driven from the npm side, using the RESOLVED versions in
`package-lock.json` (not the caret ranges in `package.json`) because CI and
the release build both run `npm ci`, which installs exactly what the lockfile
says. The lockfile is the version that actually ships.

Only major.minor is compared — see the CLI predicate quoted above. A patch
difference (crate 2.4.10 vs npm 2.4.9) is never reported.

Name mapping mirrors the CLI's own pairing:

    npm @tauri-apps/api            <->  crate tauri
    npm @tauri-apps/plugin-<name>  <->  crate tauri-plugin-<name>

HOISTED VS NESTED package-lock.json ENTRIES
--------------------------------------------
npm's `packages` map keys every install location as a path, e.g.
`node_modules/@tauri-apps/plugin-fs` (hoisted to the project root) or
`node_modules/some-dep/node_modules/@tauri-apps/plugin-fs` (nested inside
another package's own `node_modules`, when a version conflict prevents
hoisting). `@tauri-apps/*` packages are direct `package.json` dependencies
in this repo and are hoisted today, but the parser does not assume that: it
matches ANY key ending in `node_modules/@tauri-apps/<name>` at any nesting
depth, and when the same package name appears at multiple depths it prefers
the shallowest (most-hoisted) one — that is the version `npm ci` actually
places at the top level and what the Tauri CLI's own `info` command reads.

PACKAGES WITH NO RUST COUNTERPART
---------------------------------
`@tauri-apps/cli` and its platform-specific `@tauri-apps/cli-*` siblings
(cli-darwin-arm64, cli-win32-x64-msvc, ...) are the build tool itself. The
`tauri-cli` crate is not a dependency of the app and never appears in
`src-tauri/Cargo.lock`, so pairing them would false-positive on every single
run. They are excluded by name before any comparison happens — the same
exclusion the CLI makes by simply not putting them in its pair list.

Any OTHER `@tauri-apps/*` package with no matching crate is reported as
INFORMATIONAL, never as a mismatch. A Rust-only plugin legitimately has no
npm half, and a newly added JS binding whose Rust half is still missing is a
different (runtime) bug from the one this script is about — flagging it as a
version mismatch would be wrong.

Exit code:
  0 — the comparison ran and found no mismatch, OR findings without --strict
  1 — --strict was passed AND (at least one major.minor mismatch, OR the
      comparison could not actually be performed — see "THE DISQUALIFYING
      BUG" above)

Usage:
  python3 tools/audit-checks/check_tauri_version_sync.py [--strict]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_JSON = REPO_ROOT / "package.json"
PACKAGE_LOCK = REPO_ROOT / "package-lock.json"
CARGO_LOCK = REPO_ROOT / "src-tauri" / "Cargo.lock"

# A scoped Tauri package's install path inside package-lock.json's `packages`
# map, at ANY nesting depth: a top-level, hoisted entry
# (`node_modules/@tauri-apps/api`) or a nested duplicate living inside
# another package's own node_modules
# (`node_modules/some-dep/node_modules/@tauri-apps/api`). Anchored to the end
# of the key so `node_modules/@tauri-apps/foo/node_modules/bar` (a *child* of
# a Tauri package, not a Tauri package itself) is not mistaken for a match.
LOCK_PKG_KEY_RE = re.compile(r"node_modules/(@tauri-apps/[A-Za-z0-9._-]+)$")

# The build tool, not a runtime half of a Rust crate. `@tauri-apps/cli` plus
# every `@tauri-apps/cli-<platform>` optional binary.
CLI_PACKAGE_RE = re.compile(r"^@tauri-apps/cli(-[A-Za-z0-9._-]+)?$")

# Leading `major.minor` of a semver string, tolerating a pre-release or build
# suffix (`2.0.0-rc.5`, `2.4.0+meta`).
SEMVER_HEAD_RE = re.compile(r"^\s*v?(\d+)\.(\d+)(?:\.(\d+))?")

# A `[[package]]` table header in Cargo.lock -- used both to locate crate
# entries and, if none appear at all, as the signal that the file isn't
# really a Cargo.lock (see collect_cargo_crates).
CARGO_PACKAGE_HEADER_RE = re.compile(r"^\[\[package\]\]\s*$")
# A `[[package]]` name line in Cargo.lock, e.g. `name = "tauri-plugin-fs"`.
CARGO_NAME_RE = re.compile(r'^name\s*=\s*"([^"]+)"\s*$')
CARGO_VERSION_RE = re.compile(r'^version\s*=\s*"([^"]+)"\s*$')


class ComparisonUnavailable(Exception):
    """Raised when the npm <-> Rust comparison could not actually be run.

    This is deliberately a HARD failure path, not a soft "skip and return 0"
    -- see the module docstring's "THE DISQUALIFYING BUG" section. Every
    raise site here corresponds to one of the four situations the task
    requires this script to fail loudly on: a missing/unparseable
    package-lock.json, a package-lock.json with no `packages` map, a
    missing/unparseable Cargo.lock, or (raised from `check()` once both
    files are read) zero comparable npm/crate pairs discovered.
    """


def npm_to_crate(npm_name: str) -> str | None:
    """Map an npm package name to the Rust crate it must stay in sync with.

    Returns None for packages that have no Rust counterpart by design (the
    CLI and its per-platform binaries)."""
    if CLI_PACKAGE_RE.match(npm_name):
        return None
    if npm_name == "@tauri-apps/api":
        # The JS API is the frontend half of the core `tauri` crate; there is
        # no crate literally called `tauri-api`.
        return "tauri"
    if npm_name.startswith("@tauri-apps/plugin-"):
        return "tauri-plugin-" + npm_name[len("@tauri-apps/plugin-") :]
    # An unrecognised @tauri-apps/* shape. Fall through with no crate guess;
    # it surfaces as informational rather than as a bogus mismatch.
    return None


def major_minor(version: str) -> tuple[int, int] | None:
    """Parse the leading `major.minor` out of a semver string."""
    m = SEMVER_HEAD_RE.match(version or "")
    if not m:
        return None
    return int(m.group(1)), int(m.group(2))


def line_of(path: Path, pattern: re.Pattern[str]) -> int:
    """First 1-indexed line in `path` matching `pattern`, or 0 if absent.

    Used only to anchor a finding bullet at a human-clickable location; a 0
    means "found the data but not the line", which never changes a verdict.
    """
    try:
        for i, line in enumerate(path.read_text(encoding="utf-8", errors="ignore").splitlines()):
            if pattern.search(line):
                return i + 1
    except OSError:
        pass
    return 0


def collect_npm_packages() -> dict[str, tuple[str, int, int]]:
    """Read the RESOLVED @tauri-apps/* versions out of package-lock.json.

    Returns {npm_name: (version, line_no, depth)}, where `depth` is the
    number of `node_modules/` segments in the entry's install path (1 for a
    hoisted top-level entry, >1 for a nested duplicate). When a package name
    appears at multiple depths, the shallowest (most-hoisted) one wins --
    that is the version `npm ci` places where the Tauri CLI's own version
    check reads it from. See "HOISTED VS NESTED" in the module docstring.

    Raises ComparisonUnavailable if package-lock.json is missing,
    unreadable, not valid JSON, or has no `packages` map -- every one of
    those means the comparison this script exists to perform literally
    cannot run, which must never be reported as a clean pass.
    """
    rel = PACKAGE_LOCK.relative_to(REPO_ROOT)
    if not PACKAGE_LOCK.exists():
        raise ComparisonUnavailable(
            f"{rel} not found -- cannot resolve installed npm @tauri-apps/* versions, "
            "so the npm <-> Rust comparison cannot run"
        )
    try:
        raw = PACKAGE_LOCK.read_text(encoding="utf-8", errors="ignore")
    except OSError as exc:
        raise ComparisonUnavailable(f"could not read {rel}: {exc}") from exc
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ComparisonUnavailable(f"{rel} is not valid JSON ({exc}) -- cannot parse it") from exc

    packages = data.get("packages")
    if not isinstance(packages, dict):
        raise ComparisonUnavailable(
            f"{rel} has no `packages` map (lockfileVersion < 2, or a malformed/truncated "
            "file) -- cannot resolve installed versions"
        )

    found: dict[str, tuple[str, int, int]] = {}
    for key, meta in sorted(packages.items()):
        m = LOCK_PKG_KEY_RE.search(key)
        if not m or not isinstance(meta, dict):
            continue
        version = meta.get("version")
        if not isinstance(version, str):
            continue
        name = m.group(1)
        depth = key.count("node_modules/")
        existing = found.get(name)
        if existing is not None and existing[2] <= depth:
            # Already have an entry at this depth or shallower for this
            # package -- keep it (sorted() iteration makes this deterministic
            # rather than depending on dict insertion order).
            continue
        line_no = line_of(PACKAGE_LOCK, re.compile(re.escape(f'"{key}":')))
        found[name] = (version, line_no, depth)
    return found


def collect_cargo_crates() -> dict[str, list[tuple[str, int]]]:
    """Read every `tauri*` crate version out of src-tauri/Cargo.lock.

    A crate name maps to a LIST because Cargo.lock can legitimately carry two
    major versions of the same crate side by side. A match against any one of
    them satisfies the check.

    Raises ComparisonUnavailable if Cargo.lock is missing, unreadable, or
    does not contain a single `[[package]]` table -- the last case means the
    file isn't actually a Cargo.lock (empty, truncated, or replaced by
    something else), which is a parse failure, not "zero tauri crates
    installed".
    """
    rel = CARGO_LOCK.relative_to(REPO_ROOT)
    if not CARGO_LOCK.exists():
        raise ComparisonUnavailable(
            f"{rel} not found -- cannot resolve installed Rust tauri* crate versions, "
            "so the npm <-> Rust comparison cannot run"
        )
    try:
        lines = CARGO_LOCK.read_text(encoding="utf-8", errors="ignore").splitlines()
    except OSError as exc:
        raise ComparisonUnavailable(f"could not read {rel}: {exc}") from exc

    if not any(CARGO_PACKAGE_HEADER_RE.match(line) for line in lines):
        raise ComparisonUnavailable(
            f"{rel} contains no [[package]] entries -- it is empty, truncated, or not a "
            "real Cargo.lock, so crate versions cannot be resolved"
        )

    crates: dict[str, list[tuple[str, int]]] = {}
    pending_name: str | None = None
    pending_line = 0
    for i, line in enumerate(lines):
        nm = CARGO_NAME_RE.match(line)
        if nm:
            pending_name = nm.group(1)
            pending_line = i + 1
            continue
        if pending_name is None:
            continue
        vm = CARGO_VERSION_RE.match(line)
        if vm:
            if pending_name == "tauri" or pending_name.startswith("tauri-"):
                crates.setdefault(pending_name, []).append((vm.group(1), pending_line))
            pending_name = None
    return crates


def collect_declared_ranges() -> dict[str, str]:
    """Read the @tauri-apps/* ranges declared in package.json.

    Purely contextual: when a lockfile mismatch is reported, knowing whether
    package.json is ALSO stale tells the fixer whether one file or two need
    editing. Never affects the verdict, and never raises -- package.json
    going missing doesn't stop the lockfile-vs-Cargo.lock comparison that is
    this script's actual job."""
    ranges: dict[str, str] = {}
    try:
        data = json.loads(PACKAGE_JSON.read_text(encoding="utf-8", errors="ignore"))
    except (OSError, json.JSONDecodeError):
        return ranges
    for field in ("dependencies", "devDependencies", "optionalDependencies"):
        block = data.get(field)
        if isinstance(block, dict):
            for name, spec in block.items():
                if name.startswith("@tauri-apps/") and isinstance(spec, str):
                    ranges[name] = spec
    return ranges


def check() -> int:
    strict = "--strict" in sys.argv

    try:
        npm_packages = collect_npm_packages()
        crates = collect_cargo_crates()
    except ComparisonUnavailable as exc:
        print("### Tauri version-sync check could not run\n")
        print(f"  • {exc}")
        print()
        print(
            "This is reported as a finding, not a skip: a comparison that never ran must "
            "never be indistinguishable from one that ran clean."
        )
        return 1 if strict else 0

    declared = collect_declared_ranges()

    cli_only = sorted(n for n in npm_packages if CLI_PACKAGE_RE.match(n))
    comparable = sorted(n for n in npm_packages if npm_to_crate(n) is not None)

    print(f"@tauri-apps/* packages in package-lock.json : {len(npm_packages)}")
    print(f"  build-tool only (@tauri-apps/cli*), skipped : {len(cli_only)}")
    print(f"tauri* crates in src-tauri/Cargo.lock       : {len(crates)}")
    print(f"Comparable npm <-> crate pairs              : {len(comparable)}")
    print()

    if not comparable:
        # Parseable, well-shaped files that nonetheless yield nothing to
        # compare -- e.g. package-lock.json has zero @tauri-apps/* entries,
        # or only @tauri-apps/cli* ones. Same "never report as a clean pass"
        # principle as the ComparisonUnavailable branch above: finding
        # nothing to compare is not the same thing as comparing everything
        # and finding no drift, and must not exit 0 under --strict.
        print("### Tauri version-sync check could not run\n")
        print(
            "  • package-lock.json and src-tauri/Cargo.lock both parsed, but zero comparable "
            "npm/Rust @tauri-apps/* pairs were found -- nothing was actually compared"
        )
        print()
        print(
            "This is reported as a finding, not a skip: a comparison that never ran must "
            "never be indistinguishable from one that ran clean."
        )
        return 1 if strict else 0

    mismatches: list[str] = []
    no_counterpart: list[str] = []
    unparseable: list[str] = []

    for npm_name in comparable:
        npm_version, npm_line, _depth = npm_packages[npm_name]
        crate_name = npm_to_crate(npm_name)
        assert crate_name is not None  # guaranteed by the `comparable` filter

        crate_entries = crates.get(crate_name)
        if not crate_entries:
            no_counterpart.append(
                f"  • package-lock.json:{npm_line} — {npm_name} {npm_version} is installed "
                f"but no `{crate_name}` crate appears in src-tauri/Cargo.lock"
            )
            continue

        npm_mm = major_minor(npm_version)
        if npm_mm is None:
            unparseable.append(
                f"  • package-lock.json:{npm_line} — {npm_name} version \"{npm_version}\" "
                f"is not parseable as semver; cannot compare"
            )
            continue

        # A match against ANY resolved version of the crate is enough — see
        # collect_cargo_crates() on side-by-side major versions.
        parsed = [(v, ln, major_minor(v)) for (v, ln) in crate_entries]
        if any(mm == npm_mm for (_v, _ln, mm) in parsed):
            continue

        crate_version, crate_line, crate_mm = parsed[0]
        crate_desc = crate_version if len(parsed) == 1 else ", ".join(v for (v, _l, _m) in parsed)
        crate_mm_desc = f"{crate_mm[0]}.{crate_mm[1]}" if crate_mm else "?"
        context = ""
        spec = declared.get(npm_name)
        if spec:
            spec_mm = major_minor(spec.lstrip("^~>=< v"))
            if spec_mm != crate_mm:
                context = f"; package.json still declares \"{spec}\", so bump that too"
        mismatches.append(
            f"  • package-lock.json:{npm_line} — {npm_name} {npm_version} vs crate "
            f"`{crate_name}` {crate_desc} (src-tauri/Cargo.lock:{crate_line}) — "
            f"major.minor differ ({npm_mm[0]}.{npm_mm[1]} vs {crate_mm_desc}); "
            f"`tauri build` will refuse to build{context}"
        )

    if mismatches:
        print("### Tauri npm package and Rust crate are on different major/minor releases\n")
        for bullet in mismatches:
            print(bullet)
        print()

    if no_counterpart:
        print("### Tauri npm package with no matching Rust crate (informational)\n")
        for bullet in no_counterpart:
            print(bullet)
        print()

    if unparseable:
        print("### Version string could not be parsed (informational)\n")
        for bullet in unparseable:
            print(bullet)
        print()

    if not (mismatches or no_counterpart or unparseable):
        print("OK — every @tauri-apps/* npm package matches its Rust crate on major.minor.")

    if mismatches and strict:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
