#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
scripts/release-notes/extract-release-notes.py
===============================================
Reads a pull request description (or any commit-style message) on stdin
and prints each `Release-Note:` note on its own line, joined back into one
line if it was wrapped. The Release Note Gate pipes this into
`lint-notes.py --trailer`, so what gets linted is what the release notes
will actually show.

WHY THIS EXISTS
---------------
The release templates (`.github/cliff-eli5-body.tera` and
`.github/cliff-cumulative-body.tera`) render a note as its whole first
paragraph, joined into one line. The gate has to lint that same text, or
part of a note reaches the published release notes unchecked. The first
attempt at matching the templates was a few lines of awk inside the
workflow. Codex tried to break it (24 Sept 2026) and found four ways the
two disagreed. All four were older than that change, because the gate
before it only ever looked at lines starting with exactly
"Release-Note: ":

1. `Release-Note:text`, with no space: the release tool still treats it as
   a note, but the gate never saw it.
2. An empty `Release-Note:` line with the note on the next line.
3. Windows line endings: a stray carriage return inside the joined text
   stopped a banned phrase from matching.
4. `Release-Note:` followed by a blank line and a whole description: the
   release tool renders the description as the note.

The rules below close each of them. Where the release tool's own reading
is odd or hard to predict (cases 2 and 4), this does not try to copy it:
an empty `Release-Note:` line is REFUSED, with a message saying how to
write it. Refusing when unsure beats guessing and reporting success.

WHAT IT DOES
------------
* Carriage returns are removed first.
* A note starts at any line beginning `Release-Note:`, with or without a
  space after the colon.
* It carries on over the following lines, joined with single spaces,
  until a blank line, another `Name: value` trailer line, or the end.
  (Stopping at the blank line keeps a "Generated with" line or a pasted
  description out, as the templates do.)
* A `Release-Note:` with nothing after it on the same line is an error
  (exit 1).

This joins MORE text than the templates might ever render, never less:
an extra line that is linted but not shown costs nothing, and a line that
is shown but never linted is the fault this exists to prevent.

Pure standard library, like the other scripts here. Tests:
`python3 scripts/release-notes/test_extract_release_notes.py`.
"""
from __future__ import annotations

import re
import sys

# A note starts at "Release-Note:" whether or not a space follows.
_NOTE_START = re.compile(r"^Release-Note:[ \t]*(.*)$")
# Any other "Name: value" trailer line ends the note being joined. The
# release tool starts a new footer at such a line, so its text is not part
# of the note.
_OTHER_TRAILER = re.compile(r"^[A-Za-z][A-Za-z-]*: ")


class EmptyNoteError(ValueError):
    """A `Release-Note:` line had nothing after it on the same line."""


def extract_notes(text: str) -> list[str]:
    """Every Release-Note in `text`, each joined into one line.

    Raises EmptyNoteError for a `Release-Note:` line with nothing after it,
    because what the release tool would render in that case cannot be
    predicted safely.
    """
    notes: list[str] = []
    current: str | None = None
    for raw in text.replace("\r", "").split("\n"):
        start = _NOTE_START.match(raw)
        if start:
            if current is not None:
                notes.append(current)
            value = start.group(1).strip()
            if not value:
                raise EmptyNoteError(
                    "A 'Release-Note:' line has nothing after it. Put the whole "
                    "note on the same line as 'Release-Note:' (it may wrap onto "
                    "the next lines, but must start on that one), or write "
                    "'Release-Note: none'."
                )
            current = value
            continue
        if current is None:
            continue
        if not raw.strip() or _OTHER_TRAILER.match(raw):
            notes.append(current)
            current = None
            continue
        current = f"{current} {raw.strip()}"
    if current is not None:
        notes.append(current)
    return notes


def main() -> int:
    try:
        notes = extract_notes(sys.stdin.read())
    except EmptyNoteError as err:
        # The ::error:: prefix makes GitHub show it on the pull request.
        print(f"::error::{err}", file=sys.stderr)
        return 1
    for note in notes:
        print(note)
    return 0


if __name__ == "__main__":
    sys.exit(main())
