#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
scripts/release-notes/test_lint_notes.py
=========================================

Negative-test fixtures for `scripts/release-notes/lint-notes.py`'s
denylist, per the repo convention in `tools/audit-checks/README.md`
("zero-finding on a clean tree; add a negative test when changing the
denylist") — extended here into an actual runnable assertion set rather
than a manual "inject the drift, confirm it's caught, revert" pass,
since the denylist itself has no test coverage otherwise.

Each case is (label, text, expected_rule). `expected_rule` is the rule
name that MUST appear in the findings for that text (a positive/"catches"
case), or `None` for a negative/"stays clean" case, where `expected_rule`
is instead read as "this specific rule name must NOT appear" (the
adjacent-phrasing false-positive check that matters most for a denylist
this size — see the `assert_clean_of` cases below).

Pure stdlib, no pytest — same house style as `tools/audit-checks/*.py`.
Run directly: `python3 scripts/release-notes/test_lint_notes.py`.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
_LINT_NOTES_PATH = _SCRIPT_DIR / "lint-notes.py"

# lint-notes.py has a hyphen in its filename (matches the repo's CLI-script
# naming convention), so it can't be `import`ed normally — load it by path.
_spec = importlib.util.spec_from_file_location("lint_notes", _LINT_NOTES_PATH)
assert _spec is not None and _spec.loader is not None
lint_notes = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lint_notes)


def _rule_names(text: str) -> set[str]:
    return {f.rule for f in lint_notes.lint_text(text, "fixture")}


# ---------------------------------------------------------------------------
# Positive cases: this exact rule MUST fire on this text.
# ---------------------------------------------------------------------------
CATCHES: list[tuple[str, str, str]] = [
    (
        "syllable-level (hyphenated) is caught",
        "MeedyaDL now recognises the difference between syllable-level and standard timing.",
        "mechanism-syllable-lyrics",
    ),
    (
        "word-level timing is caught",
        "It preserves word-level timing in a plain-text-editable document.",
        "mechanism-syllable-lyrics",
    ),
    (
        "Apple-side (hyphenated) is caught",
        "A known Apple-side bug is now automatically worked around.",
        "mechanism-apple-side",
    ),
    (
        "retries-without-X-instead-of-Y is caught",
        "MeedyaDL retries without cover art instead of the whole download failing.",
        "mechanism-retry-against-service",
    ),
    (
        "timeout-and-retry-when-fetching is caught",
        "An upstream update adds a proper timeout and automatic retry when fetching artwork.",
        "mechanism-retry-against-service",
    ),
    (
        "reverse-DNS bundle identifier is caught",
        "The desktop app's identifier is com.meedyasuite.meedyadl.",
        "mechanism-bundle-identifier",
    ),
    (
        "CDN is caught",
        "Assets now load faster from our CDN.",
        "mechanism-cdn",
    ),
    (
        "named iTunes Lookup service is caught",
        "MeedyaDL now queries the iTunes Lookup API for extra track details.",
        "mechanism-lookup-service",
    ),
    (
        "generic lookup-service phrasing is caught",
        "A new lookup service resolves cross-platform links automatically.",
        "mechanism-lookup-service",
    ),
]

# ---------------------------------------------------------------------------
# Negative cases: the named rule must NOT fire on this adjacent, allowed
# phrasing. This is the false-positive guard — the whole reason these
# rules are scoped as tightly as they are (see the inline comments next
# to each rule in lint-notes.py).
# ---------------------------------------------------------------------------
STAYS_CLEAN_OF: list[tuple[str, str, str]] = [
    (
        "word-by-word is the approved phrasing, not a syllable/word-level leak",
        "Word-by-word synced lyrics download more reliably.",
        "mechanism-syllable-lyrics",
    ),
    (
        "'Apple Music-side' is plain adjective use, not the hyphenated Apple-side pattern",
        "A known Apple Music-side quirk with music-video artwork is fixed.",
        "mechanism-apple-side",
    ),
    (
        "the literal 'Retry without Wrapper' UI pill label is not a retry-against-service leak",
        '"Retry without Wrapper" pill is now actually visible when a download fails.',
        "mechanism-retry-against-service",
    ),
    (
        "an ordinary compare/repo URL is not a bundle identifier",
        "See github.com/MWBMPartners/MeedyaDL for the full technical changelog.",
        "mechanism-bundle-identifier",
    ),
    (
        "'MusicBrainz lookups' is allowed capability language (MusicBrainz is allowlisted)",
        "The activity log now shows more detail while MusicBrainz lookups run.",
        "mechanism-lookup-service",
    ),
    (
        "plain 'cross-platform lookups' capability language is not the lookup-service pattern",
        "Cross-region matching for cross-platform lookups now works correctly.",
        "mechanism-lookup-service",
    ),
]


def _run_corpus_regression() -> list[str]:
    """Every committed release-notes file must be error-tier clean — this
    is the same invariant `release-note-gate.yml` enforces per-file, run
    here too so a denylist change that regresses the whole corpus fails
    loudly in the same place as the rule-level fixtures above."""
    failures: list[str] = []
    notes_dir = _SCRIPT_DIR.parent.parent / ".github" / "release-notes"
    for fp in sorted(notes_dir.glob("v*.md")):
        findings = lint_notes.lint_file(fp)
        errors = [f for f in findings if f.tier == "error"]
        if errors:
            rendered = ", ".join(f"{f.rule}:'{f.text}'" for f in errors)
            failures.append(f"{fp.name} has error-tier findings: {rendered}")
    return failures


def main() -> int:
    failures: list[str] = []

    for label, text, expected_rule in CATCHES:
        found = _rule_names(text)
        if expected_rule not in found:
            failures.append(
                f"CATCHES failed: {label!r} — expected rule {expected_rule!r} "
                f"in findings, got {sorted(found)!r}"
            )

    for label, text, forbidden_rule in STAYS_CLEAN_OF:
        found = _rule_names(text)
        if forbidden_rule in found:
            failures.append(
                f"STAYS_CLEAN_OF failed: {label!r} — rule {forbidden_rule!r} "
                f"fired on allowed text {text!r}"
            )

    failures.extend(_run_corpus_regression())

    total_cases = len(CATCHES) + len(STAYS_CLEAN_OF)
    if failures:
        print(f"{len(failures)} failure(s) out of {total_cases} rule fixtures + corpus regression:\n")
        for f in failures:
            print(f"  • {f}")
        return 1

    print(f"OK — {total_cases} rule fixtures passed, corpus regression clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
