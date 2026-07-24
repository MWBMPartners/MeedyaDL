#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
scripts/release-notes/lint-notes.py
====================================

Denylist-driven content linter for MeedyaDL release notes, over BOTH
authoring surfaces:

  - `Release-Note:` trailer lines (one per PR — read from stdin with
    --trailer)
  - committed tier-1 curated notes files (`.github/release-notes/*.md`,
    passed as FILE arguments)

Enforces the two requirements in `.github/release-notes/STYLE_GUIDE.md`:

  (a) plain English / no commit-speak ("Hard bans" section), and
  (b) never disclose HOW a feature is delivered — credentials, acquisition
      paths, protocol/stream internals, storage/crypto internals
      ("Never reveal how a feature is delivered" section).

Design + denylist seed: `.github/audits/release-notes-policy-audit-2026-07-24.md`
section 4 ("Enforcement design"). Two deliberate refinements over the seed's
literal regex text, made while calibrating this script to be zero-finding
on the actual (post-rewrite) corpus, both noted inline below:

  1. `mechanism-private-key` uses `private keys?` (not `private key`) so the
     plural form is caught too — a bare `\\bprivate key\\b` cannot match
     "...private keys..." because `\\b` requires a boundary immediately
     after "key", and "s" is a word character.
  2. `commit-speak-backticked-identifier` carves out bare UI template
     placeholders (`` `{album}` ``, `` `{platform}` ``) — these are
     legitimate, user-facing filename/folder template variables (Settings
     > filename templates), not code identifiers, and the audit's own
     grading (v1.9.0.md) treats the `{platform}` template chip as a clean
     UI element.

This is a pure-stdlib script (no third-party deps), matching the house
style of `tools/audit-checks/*.py`: zero-finding on a clean tree; add a
negative test / seed line when changing the denylist.

USAGE
  lint-notes.py [--trailer] [--strict] [FILE ...]
  echo "$LINE" | lint-notes.py --trailer

  --trailer   Read newline-separated `Release-Note:` trailer text from
              stdin instead of linting FILE arguments. Each stdin line is
              treated as one independent trailer (already stripped of the
              leading "Release-Note: " prefix by the caller — see
              `.github/workflows/release-note-gate.yml` wiring).
  --strict    Escalate warning-tier findings to blocking (exit 1 if any
              warning-tier finding exists, even with zero errors).

EXIT CODES
  0  no error-tier findings (and, under --strict, no warning-tier findings)
  1  at least one error-tier finding (or, under --strict, any finding)

OUTPUT
  "  • path:line — [tier] rule: matched 'text'" per finding — errors
  before warnings, then file/line order within each tier.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Allowlist: product/brand names that would otherwise trip the noisy
# CamelCase heuristic (warning tier). Extend in-repo, reviewed — audit §4.1.
# ---------------------------------------------------------------------------
ALLOWLIST = {
    "MeedyaDL", "MeedyaSuite", "MeedyaConverter", "MeedyaManager", "GitHub",
    "MusicKit", "LRCGET", "LRCLIB", "YouTube", "SoundCloud", "AppImage",
    "ChromeOS", "VoiceOver", "NVDA", "MacBook", "iTunes", "iPlayer",
    "MusicBrainz", "AcoustID", "ReplayGain", "Raspberry Pi", "Dolby", "PyPI",
}

# A backticked span that is exactly a bare `{identifier}` template
# placeholder (filename/folder template variables users configure in
# Settings, e.g. `{album}`, `{platform}`) — see refinement 2 in the module
# docstring.
_TEMPLATE_PLACEHOLDER_RE = re.compile(r"^\{[A-Za-z_]+\}$")

# The workflow-appended "Choose your download" footer is not authored
# content — never lint it when pointed at a live/spliced release body.
_FOOTER_MARKER = "## Choose your download"

# ---------------------------------------------------------------------------
# Denylist. Each rule is (name, compiled_regex). Tiers are enforced
# separately below (errors always block; warnings only block under
# --strict). Ordering within a tier mirrors audit §4.2 for traceability.
# ---------------------------------------------------------------------------

ERROR_MECHANISM_RULES: list[tuple[str, re.Pattern[str]]] = [
    ("mechanism-token-phrase", re.compile(r"(?i)\b(developer|web[- ]?player|user|bearer|access|auth)\s+token\b")),
    ("mechanism-token-standalone", re.compile(r"(?i)\btoken\b")),
    ("mechanism-music-user-token", re.compile(r"(?i)\bMusic-User-Token\b")),
    ("mechanism-jwt", re.compile(r"\bJWT\b")),
    # Plural-inclusive — see refinement 1 in the module docstring.
    ("mechanism-private-key", re.compile(r"(?i)\bprivate keys?\b")),
    ("mechanism-p8", re.compile(r"\.p8\b")),
    ("mechanism-keychain", re.compile(r"(?i)\bkeychain\b")),
    ("mechanism-endpoint", re.compile(r"(?i)\bendpoint\b")),
    ("mechanism-api-version-path", re.compile(r"\b/v[0-9]+/")),
    ("mechanism-amp-api", re.compile(r"amp-api")),
    ("mechanism-api-music-apple", re.compile(r"api\.music\.apple")),
    ("mechanism-syllable-lyrics", re.compile(r"syllable-lyrics")),
    ("mechanism-scraping", re.compile(r"(?i)\b(scrape[sd]?|scraping)\b")),
    ("mechanism-extraction", re.compile(r"(?i)\bextract(s|ed|ing|ion)?\b.{0,40}\b(token|cookie|credential)s?\b")),
    ("mechanism-m3u8", re.compile(r"\bm3u8\b")),
    ("mechanism-hls", re.compile(r"\bHLS\b")),
    ("mechanism-ttml", re.compile(r"\bTTML\b")),
    # Negative lookahead allows the Settings label "decryption address".
    ("mechanism-decrypt", re.compile(r"(?i)\bdecrypt(ion|ing|ed)?\b(?!.{0,20}address)")),
    ("mechanism-aes", re.compile(r"(?i)\bAES-[0-9]+")),
    ("mechanism-pbkdf", re.compile(r"\bPBKDF")),
    ("mechanism-sha", re.compile(r"\bSHA-?[0-9]+")),
    ("mechanism-iterations", re.compile(r"[0-9][0-9\s]*iterations")),
    ("mechanism-sqlite", re.compile(r"(?i)\bSQLite\b")),
    ("mechanism-webview", re.compile(r"\bWebView\b")),
    ("mechanism-js-evaluation", re.compile(r"(?i)\bJavaScript evaluation\b")),
]

COMMIT_SPEAK_RULES: list[tuple[str, re.Pattern[str]]] = [
    ("commit-speak-snake-case", re.compile(r"\b[a-z0-9]+_[a-z0-9]+_?[a-z0-9]*\b")),
    ("commit-speak-cli-flag", re.compile(r"--[a-z][a-z0-9-]+\b")),
    ("commit-speak-function-call", re.compile(r"\b\w+\(\)")),
    ("commit-speak-backticked-identifier", re.compile(r"`[^`]*[_(){}]`")),
    ("commit-speak-cliff-header", re.compile(r"^## \[[0-9]", re.MULTILINE)),
    ("commit-speak-scope-prefix-bullet", re.compile(r"^\s*-\s*\*\*\([a-z-]+\)\*\*", re.MULTILINE)),
    ("commit-speak-bare-issue-citation", re.compile(r"\(#[0-9]+(,\s*#[0-9]+)*\)$", re.MULTILINE)),
    ("commit-speak-jargon", re.compile(r"(?i)\b(refactor|wire[- ]up|hoist)\b")),
    ("commit-speak-draft-source-residue", re.compile(r"<!--\s*SOURCE")),
]

WARNING_RULES: list[tuple[str, re.Pattern[str]]] = [
    ("warn-konami", re.compile(r"(?i)Konami")),
    ("warn-api", re.compile(r"(?i)\bAPI\b")),
    ("warn-json", re.compile(r"(?i)\bJSON\b")),
    ("warn-database", re.compile(r"(?i)\bdatabase\b")),
    ("warn-apples-x", re.compile(r"(?i)Apple's .{0,20}(data|format|service)")),
    ("warn-commit-jargon-common", re.compile(r"(?i)\b(emit|guard|gate)\b")),
    ("warn-camelcase", re.compile(r"\b[A-Z][a-z]+[A-Z]\w*\b")),
]

# (tier_label, rules) in severity order for both matching and reporting.
_TIERED_RULESETS: list[tuple[str, list[tuple[str, re.Pattern[str]]]]] = [
    ("error", ERROR_MECHANISM_RULES),
    ("error", COMMIT_SPEAK_RULES),
    ("warning", WARNING_RULES),
]


class Finding:
    __slots__ = ("path", "line_no", "tier", "rule", "text")

    def __init__(self, path: str, line_no: int, tier: str, rule: str, text: str) -> None:
        self.path = path
        self.line_no = line_no
        self.tier = tier
        self.rule = rule
        self.text = text

    def render(self) -> str:
        return f"  • {self.path}:{self.line_no} — [{self.tier}] {self.rule}: matched '{self.text}'"


def _strip_footer(lines: list[str]) -> list[str]:
    """Drop the workflow-appended 'Choose your download' footer onward —
    it is not authored content and must never trip the linter when this
    script is pointed at a live/spliced release body."""
    for i, line in enumerate(lines):
        if line.strip() == _FOOTER_MARKER:
            return lines[:i]
    return lines


def _is_allowlisted_camelcase(matched: str) -> bool:
    return matched in ALLOWLIST


def _is_allowlisted_backtick(matched: str) -> bool:
    """True if a `commit-speak-backticked-identifier` match is actually a
    bare UI template placeholder (refinement 2 in the module docstring)."""
    inner = matched.strip("`")
    return bool(_TEMPLATE_PLACEHOLDER_RE.match(inner))


def lint_text(text: str, path: str, strip_footer: bool = False) -> list[Finding]:
    """Run every rule against `text`, returning findings in
    error-then-warning, file-order sequence. `path` is only used for
    reporting — pass a synthetic label like "trailer" for stdin input."""
    lines = text.splitlines()
    if strip_footer:
        lines = _strip_footer(lines)

    findings: list[Finding] = []
    for tier, ruleset in _TIERED_RULESETS:
        for rule_name, pattern in ruleset:
            for line_no, line in enumerate(lines, start=1):
                for m in pattern.finditer(line):
                    matched = m.group(0)
                    if rule_name == "warn-camelcase" and _is_allowlisted_camelcase(matched):
                        continue
                    if rule_name == "commit-speak-backticked-identifier" and _is_allowlisted_backtick(matched):
                        continue
                    findings.append(Finding(path, line_no, tier, rule_name, matched))
    return findings


def lint_file(file_path: Path) -> list[Finding]:
    text = file_path.read_text(encoding="utf-8", errors="ignore")
    return lint_text(text, str(file_path), strip_footer=True)


def lint_trailer_stream(stream: str) -> list[Finding]:
    """Each stdin line is an independent trailer — line numbers in the
    report therefore mean "the Nth trailer line on stdin", not a position
    inside a file."""
    findings: list[Finding] = []
    for i, line in enumerate(stream.splitlines(), start=1):
        if not line.strip():
            continue
        line_findings = lint_text(line, "trailer", strip_footer=False)
        # Re-number: lint_text() reports the line's own internal position
        # (always 1, since a trailer is single-line), but the report should
        # identify *which* trailer on stdin tripped the rule.
        findings.extend(Finding("trailer", i, f.tier, f.rule, f.text) for f in line_findings)
    return findings


def main(argv: list[str]) -> int:
    args = argv[1:]
    trailer_mode = "--trailer" in args
    strict = "--strict" in args
    file_args = [a for a in args if a not in ("--trailer", "--strict")]

    all_findings: list[Finding] = []

    if trailer_mode:
        stdin_text = sys.stdin.read()
        all_findings.extend(lint_trailer_stream(stdin_text))
    else:
        if not file_args:
            print("lint-notes.py: no FILE arguments given (and --trailer not set)", file=sys.stderr)
            print(__doc__, file=sys.stderr)
            return 2
        for arg in file_args:
            fp = Path(arg)
            if not fp.is_file():
                print(f"lint-notes.py: {arg}: not a file", file=sys.stderr)
                return 2
            all_findings.extend(lint_file(fp))

    errors = [f for f in all_findings if f.tier == "error"]
    warnings = [f for f in all_findings if f.tier == "warning"]

    for f in errors:
        print(f.render())
    for f in warnings:
        print(f"::warning::{f.render().strip()}")

    total = len(errors) + len(warnings)
    if total == 0:
        print("OK — no mechanism-disclosure or commit-speak findings.")
    else:
        print(f"\n{len(errors)} error(s), {len(warnings)} warning(s).")

    if errors:
        return 1
    if strict and warnings:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
