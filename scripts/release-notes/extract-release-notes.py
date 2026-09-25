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

A stand-in review the same night (a fresh Opus agent, while Codex was
out) found two more, by the same method:

5. `Release-Note #text` -- git-cliff also accepts the "Token #value" form
   of a trailer, and publishes it, but the gate only looked for a colon.
6. A line holding only an invisible control character (\x1c to \x1f).
   Python counts those as blank; git-cliff does not, so it joined the
   next line into the note while this script stopped short of it.

A second stand-in review (25 Sept, Codex out again) found two more:

7. A lone carriage return (not part of a Windows line ending). This
   script used to delete every carriage return first, which glued two
   words together in its copy ("keychain\rsafely" became
   "keychainsafely"), so a banned word no longer matched -- while the
   release notes kept the character, which a reader sees as a break.
8. Invisible formatting characters, such as a soft hyphen inside a word
   ("key\u00adchain"): not shown to a reader, but they stop a banned word
   from matching.

A third stand-in (25 Sept) found more of the same kind:

9.  Line and paragraph separators (U+2028/U+2029) inside a note: the lint
    split the note in two at them, so a phrase across one was never
    matched. Both are now refused here, and the lint splits on "\n" only.
10. Other invisible characters that are not category Cf -- the combining
    grapheme joiner, variation selectors, Hangul and Mongolian fillers --
    can also hide a banned word. The known ones are refused (see
    _INVISIBLE). An emoji's own variation selector (U+FE0F, as in a
    warning sign) is still allowed.
11. The check also ran on the line that ENDS a note (for example a
    "Co-Authored-By:" line with an emoji in a name), refusing it although
    it is not part of the note. It now runs only on lines that are.

What no extractor can fix: letters from other alphabets that look the same
(a Cyrillic "е" in "kеychain"), full-width letters, and unusual space
characters in a banned phrase. Those are limits of a list of banned words;
normalising text inside lint-notes.py is the place for that, if wanted.

The rules below close each of them. Where the release tool's own reading
is odd or hard to predict (cases 2 and 4), this does not try to copy it:
an empty `Release-Note:` line is REFUSED, with a message saying how to
write it. Refusing when unsure beats guessing and reporting success.

WHAT IT DOES
------------
* Windows line endings (carriage return + line feed) become plain line
  feeds. Any OTHER carriage return is left in place, and is then refused
  with the other control characters below.
* A note starts at any line beginning `Release-Note:` or `Release-Note #`,
  with or without a space after the colon.
* It carries on over the following lines, joined with single spaces,
  until a blank line, another `Name: value` trailer line, or the end.
  (Stopping at the blank line keeps a "Generated with" line or a pasted
  description out, as the templates do.)
* A line counts as blank only by the release tool's own rule (Rust's
  idea of white space, which does not include \x1c to \x1f).
* A `Release-Note:` with nothing after it on the same line is an error
  (exit 1). So is a note containing any control character other than a
  tab, or any invisible formatting character (Unicode category Cf, such
  as a soft hyphen or zero-width space): what the release tool would show
  is then hard to be sure of, a banned word could hide behind one, and no
  real note needs one.

This joins MORE text than the templates might ever render, never less:
an extra line that is linted but not shown costs nothing, and a line that
is shown but never linted is the fault this exists to prevent.

Pure standard library, like the other scripts here. Tests:
`python3 scripts/release-notes/test_extract_release_notes.py`.
"""
from __future__ import annotations

import re
import sys
import unicodedata

# A note starts at "Release-Note:" whether or not a space follows, or at
# "Release-Note #", the other trailer form the release tool accepts.
_NOTE_START = re.compile(r"^Release-Note(?:[ \t]*:|[ \t]*#)[ \t]*(.*)$")
# Characters Python calls white space but Rust (and so the release tool)
# does not. A line of these is NOT blank to the release tool.
_PYTHON_ONLY_SPACE = "\x1c\x1d\x1e\x1f"
# Any control character except tab (including a lone carriage return and
# the C1 range 0x80-0x9F), and the line and paragraph separators. A note
# containing one is refused.
_CONTROL = re.compile(r"[\x00-\x08\x0b-\x1f\x7f-\x9f\u2028\u2029]")
# Invisible characters that are NOT Unicode category Cf, so the Cf check
# misses them, but that can still hide a banned word. U+FE0F is left out on
# purpose: it is the variation selector an emoji like a warning sign uses.
# It is REMOVED from the text handed to the linter instead (see
# extract_notes), so the linter sees exactly what a reader sees:
# "key<U+FE0F>chain" and "SHA-<U+FE0F>256" are linted as "keychain" and
# "SHA-256". A first attempt refused it only when next to a letter; that
# missed digits and punctuation (SHA-256, (#123)) and wrongly refused the
# information emoji, whose base character Unicode counts as a letter
# (Codex, then a stand-in review, 25 Sept 2026).
_INVISIBLE = re.compile(
    "[\u034f\u115f\u1160\u17b4\u17b5\u180b-\u180f\u3164\ufe00-\ufe0e\uffa0]"
)


def _refused_character(line: str) -> bool:
    """A control, invisible or formatting character a note may not hold."""
    return bool(
        _CONTROL.search(line)
        or _INVISIBLE.search(line)
        or any(unicodedata.category(c) == "Cf" for c in line)
    )
# Any other trailer line ends the note being joined: the release tool starts
# a new footer there, so its text is not part of the note. Its rule (checked
# against git-cliff 2.13.1, 25 Sept 2026): a run of characters with no white
# space, brackets, ":" or "!", then ":" or " #" -- so "Co-Authored-By:Name"
# (no space) and "Reviewed-by #Name" end a note too. This used to require
# "Name: " with a space, so those lines were joined into the note, and an
# emoji in a co-author's name got the whole PR refused (Codex, 25 Sept).
# "#" is also kept out of the name here: in a doubtful case the line stays
# in the note, which checks MORE text, never less.
_OTHER_TRAILER = re.compile(r"^[^\s():!#]+(?::| #)")


class RefusedNoteError(ValueError):
    """A note the gate refuses: empty, or holding a control character."""


def _is_blank(line: str) -> bool:
    """Blank by the release tool's rule, not Python's."""
    return all(c.isspace() and c not in _PYTHON_ONLY_SPACE for c in line)


def extract_notes(text: str) -> list[str]:
    """Every Release-Note in `text`, each joined into one line.

    Raises RefusedNoteError for a `Release-Note:` line with nothing after it,
    because what the release tool would render in that case cannot be
    predicted safely.
    """
    notes: list[str] = []
    current: str | None = None
    for raw in text.replace("\r\n", "\n").split("\n"):
        start = _NOTE_START.match(raw)
        # Is this line part of a note? A note's own first line, or a line
        # that continues one — NOT the blank or trailer line that ends one.
        in_note = start is not None or (
            current is not None
            and not _is_blank(raw)
            and not _OTHER_TRAILER.match(raw)
        )
        if in_note and _refused_character(raw):
            raise RefusedNoteError(
                "A 'Release-Note:' note contains an invisible control or "
                "formatting character. Retype the note as plain text."
            )
        if start:
            if current is not None:
                notes.append(current)
            value = start.group(1).strip()
            if not value:
                raise RefusedNoteError(
                    "A 'Release-Note:' line has nothing after it. Put the whole "
                    "note on the same line as 'Release-Note:' (it may wrap onto "
                    "the next lines, but must start on that one), or write "
                    "'Release-Note: none'."
                )
            current = value.replace("\ufe0f", "")
            continue
        if current is None:
            continue
        if _is_blank(raw) or _OTHER_TRAILER.match(raw):
            notes.append(current)
            current = None
            continue
        current = f"{current} {raw.strip().replace(chr(0xFE0F), '')}"
    if current is not None:
        notes.append(current)
    return notes


def main() -> int:
    try:
        notes = extract_notes(sys.stdin.read())
    except RefusedNoteError as err:
        # The ::error:: prefix makes GitHub show it on the pull request.
        print(f"::error::{err}", file=sys.stderr)
        return 1
    for note in notes:
        print(note)
    return 0


if __name__ == "__main__":
    sys.exit(main())
