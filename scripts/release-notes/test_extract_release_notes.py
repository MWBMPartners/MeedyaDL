#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
scripts/release-notes/test_extract_release_notes.py
====================================================
Tests for `extract-release-notes.py`, the step that decides what text the
Release Note Gate lints.

WHY THIS EXISTS
---------------
If that step reads a note differently from the release templates, part of
a note reaches the published release notes without ever being checked.
Each case here is a way that happened, or nearly did. The four "Codex"
cases are the ones Codex found on 24 Sept 2026, by feeding sample messages
to both the gate and the real release tool and comparing. Each was checked
against git-cliff 2.14 locally when this was written.

Pure stdlib, no pytest. Run directly:
`python3 scripts/release-notes/test_extract_release_notes.py`.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

_PATH = Path(__file__).resolve().parent / "extract-release-notes.py"
# The file name has hyphens (the CLI-script convention here), so it is
# loaded by path rather than imported.
_spec = importlib.util.spec_from_file_location("extract_release_notes", _PATH)
assert _spec and _spec.loader
_mod = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_mod)
extract_notes = _mod.extract_notes
RefusedNoteError = _mod.RefusedNoteError

# (label, message, expected notes)
CASES: list[tuple[str, str, list[str]]] = [
    ("one line", "Body.\n\nRelease-Note: Downloads work.", ["Downloads work."]),
    (
        "wrapped note is joined, as the templates now render it",
        "Release-Note: Fixed a case where a single could be given the\nwrong album's details.",
        ["Fixed a case where a single could be given the wrong album's details."],
    ),
    (
        "stops at a blank line, so a Generated-with line is left out",
        "Release-Note: Downloads work.\n\n🤖 Generated with [Claude Code](x)",
        ["Downloads work."],
    ),
    (
        "stops at the next trailer",
        "Release-Note: Downloads work.\nCo-Authored-By: Someone <a@b>",
        ["Downloads work."],
    ),
    (
        "two notes in a row",
        "Release-Note: First.\nRelease-Note: Second\nwraps.",
        ["First.", "Second wraps."],
    ),
    ("none is still a note, for the linter to allow", "Release-Note: none", ["none"]),
    ("no note at all", "Just a description.\n", []),
    # --- The four found by Codex, 24 Sept 2026 ---
    (
        "Codex 1: no space after the colon is still a note",
        "Release-Note: Downloads work.\n\nRelease-Note:Uses a token.",
        ["Downloads work.", "Uses a token."],
    ),
    (
        "Codex 3: Windows line endings do not split a phrase",
        "Release-Note: Protects private\r\nkeys during sign-in.\r\n",
        ["Protects private keys during sign-in."],
    ),
    # --- Found by the stand-in review, 24 Sept 2026 ---
    (
        "stand-in 1: the 'Release-Note #text' form is a note too",
        "Release-Note #Now reads your private keys from the keychain.\nRelease-Note: none",
        ["Now reads your private keys from the keychain.", "none"],
    ),
    (
        "a line of ordinary spaces still ends a note",
        "Release-Note: Downloads work.\n   \nA whole description.",
        ["Downloads work."],
    ),
    (
        "stand-in 11: the trailer line that ENDS a note is not checked",
        "Release-Note: Fixed a thing.\nCo-Authored-By: Zo\u00eb \U0001F468\u200d\U0001F469\u200d\U0001F467 <a@b>",
        ["Fixed a thing."],
    ),
    (
        "Codex: 'Name:value' with no space ends a note, as it does for the release tool",
        "Release-Note: Downloads work.\nCo-Authored-By:Someone \U0001F468\u200d\U0001F469\u200d\U0001F467 <a@b>",
        ["Downloads work."],
    ),
    (
        "Codex: the 'Name #value' trailer form ends a note too",
        "Release-Note: Downloads work.\nReviewed-by #Someone \U0001F468\u200d\U0001F469\u200d\U0001F467 <a@b>",
        ["Downloads work."],
    ),
    # U+FE0F is removed before linting, so the linter sees what a reader
    # sees (Codex, then a stand-in, 25 Sept 2026).
    (
        "U+FE0F between letters is removed, so the hidden word is linted",
        "Release-Note: Now stores your key\ufe0fchain safely.",
        ["Now stores your keychain safely."],
    ),
    (
        "U+FE0F between digits and punctuation is removed too",
        "Release-Note: Passwords now use SHA-\ufe0f256.",
        ["Passwords now use SHA-256."],
    ),
    (
        "the information emoji is not refused",
        "Release-Note: \u2139\ufe0f Downloads now resume.",
        ["\u2139 Downloads now resume."],
    ),
    (
        "a keycap emoji is not refused",
        "Release-Note: Step 1\ufe0f\u20e3 is clearer.",
        ["Step 1\u20e3 is clearer."],
    ),
    (
        "U+FE0F on a continuation line is removed too",
        "Release-Note: Fixed the\nkey\ufe0fchain problem.",
        ["Fixed the keychain problem."],
    ),
    (
        "a warning emoji is not refused",
        "Release-Note: The \u26a0\ufe0f warning now explains itself.",
        ["The \u26a0 warning now explains itself."],
    ),
    (
        "emoji and soft hyphens OUTSIDE a note are not checked",
        "A family \U0001F468\u200d\U0001F469 and a soft\u00adhyphen.\n\nRelease-Note: Downloads work.",
        ["Downloads work."],
    ),
]

# Messages that must be REFUSED: an empty Release-Note: line (Codex cases 2
# and 4), or a note holding a control character (stand-in case 2). In each,
# the gate cannot tell safely what the release tool would show.
REFUSED: list[tuple[str, str]] = [
    ("Codex 2: empty line, note on the next line", "Release-Note: Downloads work.\nRelease-Note:\nUses a token."),
    ("Codex 4: empty line, blank, then a description", "Release-Note: \n\nA whole PR description."),
    ("empty with only spaces", "Release-Note:    "),
    ("stand-in 1: empty '#' form, note on the next line", "Release-Note #\nprivate keys"),
    (
        "stand-in 2: a control character Python calls blank but the release tool does not",
        "Release-Note: Fixed sign-in.\n\x1f\nNow stores your private keys in the keychain.",
    ),
    ("a control character inside the note itself", "Release-Note: Fixed\x07 sign-in."),
    # --- Found by the second stand-in review, 25 Sept 2026 ---
    ("stand-in 7: a lone carriage return is refused, not deleted", "Release-Note: Now stores it in the keychain\rsafely."),
    ("stand-in 8: a soft hyphen hiding a word", "Release-Note: Now stores it in the key\u00adchain safely."),
    ("a zero-width space hiding a word", "Release-Note: Now stores it in the key\u200bchain safely."),
    ("a C1 control character", "Release-Note: Fixed\x85 sign-in."),
    # --- Found by the third stand-in review, 25 Sept 2026 ---
    ("stand-in 9: a line separator inside a note", "Release-Note: Moved your private\u2028key into safer storage."),
    ("stand-in 9: a paragraph separator inside a note", "Release-Note: Moved your private\u2029key into safer storage."),
    ("stand-in 10: a combining grapheme joiner hiding a word", "Release-Note: Moved your key\u034fchain."),
    ("stand-in 10: a text variation selector hiding a word", "Release-Note: Moved your key\ufe0echain."),
    ("a supplementary variation selector hiding a word", "Release-Note: Fixed the key\U000e0100chain problem."),
    ("a note of nothing but U+FE0F is empty", "Release-Note: \ufe0f"),
    ("'none' with U+FE0F in it would publish a 'none' bullet", "Release-Note: \ufe0fnone"),
]


def main() -> int:
    failures = 0
    for label, message, expected in CASES:
        try:
            got = extract_notes(message)
        except RefusedNoteError as err:
            print(f"FAIL {label}: refused unexpectedly ({err})")
            failures += 1
            continue
        if got != expected:
            print(f"FAIL {label}: expected {expected!r}, got {got!r}")
            failures += 1
    for label, message in REFUSED:
        try:
            got = extract_notes(message)
        except RefusedNoteError:
            continue
        print(f"FAIL {label}: should have been refused, got {got!r}")
        failures += 1
    total = len(CASES) + len(REFUSED)
    if failures:
        print(f"{failures} of {total} cases failed.")
        return 1
    print(f"All {total} cases passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
