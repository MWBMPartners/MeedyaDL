#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
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
    reads = _find_reads()

    if not reads:
        print("OK — no build-time values are read by the app.")
        return 0

    missing: list[tuple[str, list[str]]] = []
    for name, sites in sorted(reads.items()):
        if name in INTENTIONALLY_UNSET:
            continue
        # A value counts as wired if the release workflow mentions it at all —
        # as an `env:` entry, or written into $GITHUB_ENV by an earlier step.
        # Deliberately loose: this check is about catching a value nobody
        # thought about, not about policing how it gets there.
        if re.search(rf"\b{re.escape(name)}\b", workflow):
            continue
        missing.append((name, sites))

    if missing:
        # The heading matters, and not only cosmetically. `pr-security.yml`
        # extracts a check's findings with `grep -A100 '###'` before handing
        # them to `add_section`, which silently drops an empty body. A script
        # that prints bullets but no `###` line therefore has every finding
        # discarded, with nothing reporting that it happened — which is exactly
        # the class of failure this script exists to catch, and exactly what
        # this script did on the day it was written. Every other check in this
        # directory emits one; so does this.
        print("### Build-time value read by the app but never supplied by the release build")
        print()
        print("Build-time values the app reads but the release build never supplies:")
        print()
        for name, sites in missing:
            print(f"  • {sites[0]} — {name} is read here but set nowhere in release.yml")
            for extra in sites[1:]:
                print(f"      also read at {extra}")
        print()
        print("  Each of these makes a feature silently inert in every shipped build.")
        print("  Either pass it through release.yml, or add it to INTENTIONALLY_UNSET")
        print("  in this script with the reason it is not needed.")
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
