#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release-notes/splice-body.py
# =====================================
#
# Shared footer-preserving splice for GitHub Release bodies. Replaces
# everything ABOVE the "## Choose your download" footer (appended by
# release.yml's finalize-release job once platform builds finish) with a
# fresh notes file, while keeping the footer -- and the "---" divider
# immediately above it -- byte-for-byte intact.
#
# Factored out of apply-notes.sh (#1028) so the exact same footer regex
# is used by BOTH:
#   - scripts/release-notes/apply-notes.sh (curated tier-1 notes, run by
#     a maintainer / CI against a stable or a pre-staged prerelease file)
#   - .github/workflows/release.yml's "ensure-release" self-heal path
#     (#1046 -- regenerates + splices a commit-speak prerelease body
#     automatically on re-run, no hand-editing required)
#
# USAGE:
#   splice-body.py <notes_file> <body_file> <out_file>
#
#   notes_file  Path to the new notes content (Markdown, no footer).
#   body_file   Path to the CURRENT release body (may or may not already
#               have a "## Choose your download" footer appended).
#   out_file    Where to write the spliced result.
#
# BEHAVIOUR:
#   - If body_file contains "\n---\n\n## Choose your download" (the
#     divider release.yml's finalize-release job inserts before the
#     table), everything from that "---" onward is preserved verbatim
#     and appended after notes_file + a blank line.
#   - Else if body_file contains a bare "## Choose your download" heading
#     with no divider (unlikely, but tolerated), everything from the
#     heading onward is preserved.
#   - Else (no footer yet -- builds haven't finished) the output is just
#     notes_file with a trailing newline.
#
# Exits 0 always; this script never fails on "no footer found" since
# that's a normal, expected state (pre-build-completion).

import re
import sys


def splice(notes: str, body: str) -> str:
    """Return notes + the preserved "Choose your download" footer (if any)
    from body. notes is expected to already be stripped of trailing
    newlines by the caller; this function re-normalises regardless."""
    notes = notes.rstrip("\n")

    # Primary: the "---" divider line immediately before the heading.
    # Match starts at the newline before the dashes so we can step past
    # it and land exactly on the first "-".
    divider = re.search(r"\n-{3,}\s*\n+(?=## Choose your download)", body)
    if divider:
        footer = body[divider.start() + 1 :]
    else:
        # Fallback: no divider found (e.g. a release object with no build
        # table appended yet, or a hand-edited body) -- footer starts at
        # the heading itself, if present at all.
        heading = re.search(r"^## Choose your download", body, re.MULTILINE)
        footer = body[heading.start() :] if heading else ""

    if footer:
        return notes + "\n\n" + footer
    return notes + "\n"


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "Usage: splice-body.py <notes_file> <body_file> <out_file>",
            file=sys.stderr,
        )
        return 1

    notes_path, body_path, out_path = sys.argv[1:4]

    with open(notes_path, "r", encoding="utf-8") as f:
        notes = f.read()

    with open(body_path, "r", encoding="utf-8") as f:
        body = f.read()

    result = splice(notes, body)

    with open(out_path, "w", encoding="utf-8") as f:
        f.write(result)

    return 0


if __name__ == "__main__":
    sys.exit(main())
