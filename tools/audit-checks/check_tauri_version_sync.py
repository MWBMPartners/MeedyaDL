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

A GUARD THAT CANNOT VERIFY MUST NEVER REPORT SUCCESS
------------------------------------------------------
An earlier draft of this script printed a WARNING and exited 0 (even under
--strict) whenever package-lock.json was missing or unparseable. That is
precisely the failure mode this whole check exists to close: a guard that
silently passes when it cannot do its job gives false confidence, which is
worse than no guard at all. So every condition that means "I cannot actually
perform the comparison" — a missing or unparseable package-lock.json, no
`packages` map in it, a missing or unreadable Cargo.lock, or zero
`@tauri-apps/*` npm packages with a Rust-crate counterpart to compare against
— is now itself an unmistakable finding, and fails the run under --strict.
"I found nothing to compare" is never reported as an OK.

WHAT IS COMPARED
----------------
Driven from the npm side, using the RESOLVED versions in
`package-lock.json` (not the caret ranges in `package.json`) because CI and
the release build both run `npm ci`, which installs exactly what the lockfile
says. The lockfile is the version that actually ships.

Name mapping mirrors the CLI's own pairing:

    npm @tauri-apps/api            <->  crate tauri
    npm @tauri-apps/plugin-<name>  <->  crate tauri-plugin-<name>

HOISTED AND NESTED LOCKFILE ENTRIES
------------------------------------
`package-lock.json`'s `packages` map keys every installed copy of a package
by its install path, e.g. `node_modules/@tauri-apps/plugin-fs` for the
top-level (hoisted) copy, or `node_modules/some-dep/node_modules/@tauri-apps/
plugin-fs` for a copy nested under a dependency that needed a different
version. Both shapes are collected here (matched by the LAST `node_modules/
<name>` path segment, regardless of nesting depth) and every distinct
resolved version for a given package name is compared: if ANY resolved
version's major.minor matches ANY resolved Cargo.lock crate version's
major.minor, that pair is fine. This mirrors how `collect_cargo_crates()`
below already tolerates two Cargo.lock entries for the same crate at
different major versions.

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
  0 — no findings, OR findings without --strict
  1 — at least one major.minor mismatch, OR the check could not be performed
      at all (see "A GUARD THAT CANNOT VERIFY" above) — AND --strict was
      passed

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

# A `node_modules/@tauri-apps/<name>` path segment, at ANY nesting depth —
# `node_modules/@tauri-apps/api` (hoisted, start of string) as well as
# `.../node_modules/@tauri-apps/plugin-fs` (nested under another package,
# preceded by a `/`). See "HOISTED AND NESTED LOCKFILE ENTRIES" above.
LOCK_PKG_KEY_RE = re.compile(r"(?:^|/)node_modules/(@tauri-apps/[A-Za-z0-9._-]+)$")

# The build tool, not a runtime half of a Rust crate. `@tauri-apps/cli` plus
# every `@tauri-apps/cli-<platform>` optional binary.
CLI_PACKAGE_RE = re.compile(r"^@tauri-apps/cli(-[A-Za-z0-9._-]+)?$")

# Leading `major.minor` of a semver string, tolerating a pre-release or build
# suffix (`2.0.0-rc.5`, `2.4.0+meta`).
SEMVER_HEAD_RE = re.compile(r"^\s*v?(\d+)\.(\d+)(?:\.(\d+))?")

# A `[[package]]` name line in Cargo.lock, e.g. `name = "tauri-plugin-fs"`.
CARGO_NAME_RE = re.compile(r'^name\s*=\s*"([^"]+)"\s*$')
CARGO_VERSION_RE = re.compile(r'^version\s*=\s*"([^"]+)"\s*$')


class CheckError(Exception):
    """Raised when the comparison cannot be performed at all — a missing or
    unparseable input file, as opposed to a normal "N findings" result.

    Every raise site here corresponds to one of the disqualifying conditions
    in the module docstring's "A GUARD THAT CANNOT VERIFY" section. The
    message is already formatted as a `  • ` bullet so it prints identically
    to a real finding — this must never be swallowed into a silent 0/OK."""


def npm_to_crate(npm_name: str) -> str | None:
    """Map an npm package name to the Rust crate it must stay in sync with.

    Returns None for packages that have no Rust counterpart by design (the
    CLI and its per-platform binaries) or whose shape this script does not
    recognise (surfaced as informational, never as a bogus mismatch)."""
    if CLI_PACKAGE_RE.match(npm_name):
        return None
    if npm_name == "@tauri-apps/api":
        # The JS API is the frontend half of the core `tauri` crate; there is
        # no crate literally called `tauri-api`.
        return "tauri"
    if npm_name.startswith("@tauri-apps/plugin-"):
        return "tauri-plugin-" + npm_name[len("@tauri-apps/plugin-") :]
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


def collect_npm_packages() -> dict[str, list[tuple[str, int]]]:
    """Read every resolved @tauri-apps/* version out of package-lock.json.

    Returns {npm_name: [(version, line_no), ...]} — a LIST per name because a
    hoisted top-level copy and a nested duplicate at a different version can
    legitimately coexist (see "HOISTED AND NESTED LOCKFILE ENTRIES" above).

    Raises CheckError — never returns a silently-empty {} — when the file is
    missing, unreadable, not valid JSON, or has no `packages` map at all."""
    if not PACKAGE_LOCK.exists():
        raise CheckError(
            f"  • {PACKAGE_LOCK.name} — file not found; cannot verify Tauri npm/Rust "
            "version sync (a guard that cannot do its job must not report success)"
        )
    try:
        raw = PACKAGE_LOCK.read_text(encoding="utf-8", errors="ignore")
    except OSError as exc:
        raise CheckError(f"  • {PACKAGE_LOCK.name} — could not read file: {exc}") from exc
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise CheckError(f"  • {PACKAGE_LOCK.name} — could not parse as JSON: {exc}") from exc

    packages = data.get("packages")
    if not isinstance(packages, dict):
        raise CheckError(
            f"  • {PACKAGE_LOCK.name} — no `packages` map found (lockfileVersion < 2?); "
            "cannot verify Tauri npm/Rust version sync"
        )

    found: dict[str, list[tuple[str, int]]] = {}
    for key, meta in packages.items():
        m = LOCK_PKG_KEY_RE.search(key)
        if not m or not isinstance(meta, dict):
            continue
        version = meta.get("version")
        if not isinstance(version, str):
            continue
        line_no = line_of(PACKAGE_LOCK, re.compile(re.escape(f'"{key}":')))
        found.setdefault(m.group(1), []).append((version, line_no))
    return found


def collect_cargo_crates() -> dict[str, list[tuple[str, int]]]:
    """Read every `tauri*` crate version out of src-tauri/Cargo.lock.

    A crate name maps to a LIST because Cargo.lock can legitimately carry two
    major versions of the same crate side by side. A match against any one of
    them satisfies the check.

    Raises CheckError — never returns a silently-empty {} — when the file is
    missing or unreadable."""
    if not CARGO_LOCK.exists():
        raise CheckError(
            f"  • src-tauri/Cargo.lock — file not found; cannot verify Tauri npm/Rust "
            "version sync (a guard that cannot do its job must not report success)"
        )
    try:
        lines = CARGO_LOCK.read_text(encoding="utf-8", errors="ignore").splitlines()
    except OSError as exc:
        raise CheckError(f"  • src-tauri/Cargo.lock — could not read file: {exc}") from exc

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
    editing. Never affects the verdict, so a missing/unparseable
    package.json degrades to "no context" rather than a CheckError."""
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
    # `hard_failures` collects every "I cannot actually verify this" bullet —
    # both CheckError-raised structural failures and the "found nothing
    # comparable" case below. Any of these fails the run under --strict,
    # exactly like a real mismatch: see the module docstring.
    hard_failures: list[str] = []

    try:
        npm_packages = collect_npm_packages()
    except CheckError as exc:
        hard_failures.append(str(exc))
        npm_packages = {}

    try:
        crates = collect_cargo_crates()
    except CheckError as exc:
        hard_failures.append(str(exc))
        crates = {}

    declared = collect_declared_ranges()

    cli_only = sorted(n for n in npm_packages if CLI_PACKAGE_RE.match(n))
    comparable = sorted(n for n in npm_packages if npm_to_crate(n) is not None)

    print(f"@tauri-apps/* packages in package-lock.json : {len(npm_packages)}")
    print(f"  build-tool only (@tauri-apps/cli*), skipped : {len(cli_only)}")
    print(f"tauri* crates in src-tauri/Cargo.lock       : {len(crates)}")
    print(f"Comparable npm <-> crate pairs              : {len(comparable)}")
    print()

    # Zero comparable pairs — with otherwise-valid inputs — is just as much a
    # "could not verify" state as a missing file: nothing was actually
    # compared, so this must not print "OK". Skipped when a structural
    # failure already explains the emptiness, to avoid a redundant bullet.
    if not hard_failures and not comparable:
        hard_failures.append(
            f"  • {PACKAGE_LOCK.name} — zero @tauri-apps/* npm packages with a Rust-crate "
            "counterpart were found (only the CLI package(s), or none at all); nothing "
            'was compared. "I found nothing to compare" must never be reported as success.'
        )

    mismatches: list[str] = []
    no_counterpart: list[str] = []
    unparseable: list[str] = []

    for npm_name in comparable:
        npm_versions = npm_packages[npm_name]
        crate_name = npm_to_crate(npm_name)
        assert crate_name is not None  # guaranteed by the `comparable` filter

        crate_entries = crates.get(crate_name)
        if not crate_entries:
            versions_desc = ", ".join(v for v, _ln in npm_versions)
            first_line = npm_versions[0][1]
            no_counterpart.append(
                f"  • package-lock.json:{first_line} — {npm_name} {versions_desc} is "
                f"installed but no `{crate_name}` crate appears in src-tauri/Cargo.lock"
            )
            continue

        # Parse every resolved npm version's major.minor. A version that
        # doesn't parse is reported on its own; the remaining, parseable
        # versions still get compared against the crate.
        parsed_npm = [(v, ln, major_minor(v)) for (v, ln) in npm_versions]
        for v, ln, mm in parsed_npm:
            if mm is None:
                unparseable.append(
                    f"  • package-lock.json:{ln} — {npm_name} version \"{v}\" is not "
                    "parseable as semver; cannot compare"
                )
        good_npm = [(v, ln, mm) for (v, ln, mm) in parsed_npm if mm is not None]
        if not good_npm:
            continue  # every resolved version was unparseable; already reported above

        crate_parsed = [(v, ln, major_minor(v)) for (v, ln) in crate_entries]
        crate_mm_set = {mm for (_v, _ln, mm) in crate_parsed if mm is not None}

        # A match against ANY resolved npm version and ANY resolved crate
        # version is enough — see the module docstring on side-by-side
        # major versions / hoisted-vs-nested duplicates.
        if any(mm in crate_mm_set for (_v, _ln, mm) in good_npm):
            continue

        npm_desc = ", ".join(v for (v, _ln, _mm) in good_npm)
        npm_line = good_npm[0][1]
        npm_mm_desc = "/".join(sorted({f"{mm[0]}.{mm[1]}" for (_v, _ln, mm) in good_npm}))
        crate_desc = crate_parsed[0][0] if len(crate_parsed) == 1 else ", ".join(
            v for (v, _ln, _mm) in crate_parsed
        )
        crate_line = crate_parsed[0][1]
        crate_mm_desc = "/".join(sorted({f"{mm[0]}.{mm[1]}" for (_v, _ln, mm) in crate_parsed if mm}))
        context = ""
        spec = declared.get(npm_name)
        if spec:
            spec_mm = major_minor(spec.lstrip("^~>=< v"))
            spec_mm_desc = f"{spec_mm[0]}.{spec_mm[1]}" if spec_mm else None
            if spec_mm_desc is not None and spec_mm_desc not in crate_mm_desc.split("/"):
                context = f'; package.json still declares "{spec}", so bump that too'
        mismatches.append(
            f"  • package-lock.json:{npm_line} — {npm_name} {npm_desc} vs crate "
            f"`{crate_name}` {crate_desc} (src-tauri/Cargo.lock:{crate_line}) — "
            f"major.minor differ ({npm_mm_desc} vs {crate_mm_desc}); "
            f"`tauri build` will refuse to build{context}"
        )

    if hard_failures:
        print("### Cannot verify Tauri npm/Rust version sync\n")
        for bullet in hard_failures:
            print(bullet)
        print()

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

    if not (hard_failures or mismatches or no_counterpart or unparseable):
        print("OK — every @tauri-apps/* npm package matches its Rust crate on major.minor.")

    if (hard_failures or mismatches) and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
