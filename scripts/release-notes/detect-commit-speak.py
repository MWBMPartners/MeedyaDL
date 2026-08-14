#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release-notes/detect-commit-speak.py
# ==============================================
#
# Heuristic detector for #1046: is a GitHub Release body raw commit-speak
# (an old-format git-cliff render, or bare "chore(alpha): X.Y.Z" version-
# bump noise) rather than the #1027/#1028 ELI5 format?
#
# Used by .github/workflows/release.yml's "ensure-release" self-heal path
# to decide whether a PRERELEASE whose GitHub Release already exists (and
# has no tier-1 curated notes file staged) should be regenerated. Stables
# are never auto-rewritten by that path -- they always require an
# explicit curated file -- so this detector is only consulted for
# prerelease tags.
#
# USAGE:
#   detect-commit-speak.py < body.md
#   echo "$BODY" | detect-commit-speak.py
#
# EXIT CODE:
#   0  the body LOOKS LIKE commit-speak / bare noise -- should self-heal.
#   1  the body already has ELI5-format markers -- leave it alone.
#
# HEURISTIC (mirrors the diagnosis report .github/audits/
# release-notes-eli5-diagnosis-2026-07-24.md section 4, G1):
#   Treat as commit-speak if the body matches an old-format signal
#   (a raw git-cliff "## [x.y.z]" version header, or a bare
#   "### Maintenance" bump-only section) AND lacks every one of the
#   new-format markers that #1027/#1028 always emit for a real ELI5
#   render (What's new / What's fixed / Notes / Under the hood / the
#   honest "no user-facing changes" preamble).

import re
import sys

# Old-format signals: a raw git-cliff version header, or the maintenance-only
# bump noise that was the literal symptom reported in #1046.
OLD_FORMAT_SIGNALS = (
    re.compile(r"^## \[[0-9]", re.MULTILINE),
    re.compile(r"###\s*(?:\U0001F9F9\s*)?Maintenance", re.IGNORECASE),
)

# New-format markers: any one of these means #1027/#1028 already ran and
# produced a real ELI5 body -- never self-heal over it.
NEW_FORMAT_MARKERS = (
    "### What's new",
    "### What's fixed",
    "### Notes",
    "### Under the hood",
    "_No user-facing changes",
)


def is_commit_speak(body: str) -> bool:
    if not body or not body.strip():
        # An empty/missing body is not "commit-speak" in the literal
        # sense, but it's certainly not a good ELI5 body either -- treat
        # it the same way so a blank release also gets healed.
        return True

    has_new_format_marker = any(marker in body for marker in NEW_FORMAT_MARKERS)
    if has_new_format_marker:
        return False

    has_old_format_signal = any(
        pattern.search(body) for pattern in OLD_FORMAT_SIGNALS
    )
    return has_old_format_signal


def main() -> int:
    body = sys.stdin.read()
    return 0 if is_commit_speak(body) else 1


if __name__ == "__main__":
    sys.exit(main())
