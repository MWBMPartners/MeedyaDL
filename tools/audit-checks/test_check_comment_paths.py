#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/test_check_comment_paths.py
===============================================

Tests for the one judgement call in `check_comment_paths.py`: which
paths are placeholders it should skip, and which are real claims it
must check.

WHY THIS EXISTS
---------------
That check exists to find comments naming files that do not exist. Its
"skip this, it is only a placeholder" rule is therefore the one place it
can go quiet about a real problem. The first version of the rule did
exactly that: it matched any two capitals around a dot, so it would have
skipped a broken reference to a file like `src/API.Client.ts` because of
the "I.C" in the middle. Codex found it in the batch-4 review.

So these tests pin BOTH halves. It must skip the genuine `vX.Y.Z`
placeholders the release tooling comments use -- or the check would cry
wolf on every pull request and people would learn to ignore it. And it
must NOT skip anything else -- or it goes back to looking away.

Proven able to fail: with the old pattern put back, the "must still be
checked" test fails on `src/API.Client.ts` and four others. It also
caught the first attempt at the new pattern, which still matched the
"Y.Z" at the end of `scripts/v2X.Y.Z.sh`; and Codex's second round found
the next attempt still matched the start of `src/X.Y.Client.ts`, which is
why the rule now only accepts a complete file name.

Pure stdlib, no pytest, same house style as the checks themselves.
Run directly: `python3 tools/audit-checks/test_check_comment_paths.py`
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location(
    "check_comment_paths", HERE / "check_comment_paths.py"
)
assert _spec and _spec.loader
check = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check)

# The real placeholders, taken from the comments that use them today
# (preserve-release-pr-body.yml), plus the two-part form.
MUST_BE_SKIPPED = [
    ".github/release-notes/vX.Y.Z.md",
    ".github/release-drafts/vX.Y.Z.md",
    ".github/release-notes/X.Y.Z.md",
    "docs/vX.Y.md",
]

# Real-looking names that merely contain capitals around a dot. Every one
# of these must reach the "does this file exist?" test.
MUST_STILL_BE_CHECKED = [
    "src/API.Client.ts",  # the case Codex named
    "src/lib/A.B.ts",  # two capitals, but not the placeholder letters
    "tools/MAX.Y.py",  # a capital run ending right before the dot
    "src/X.Yaml.ts",  # X.Y followed straight by more letters
    "scripts/v2X.Y.Z.sh",  # a digit glued to the front
    "src/A.Y.Z.ts",  # the placeholder's tail inside a longer dotted run
    "src/X.Y.Client.ts",  # the placeholder as the START of a longer name (Codex, round 2)
    "src/vX.Y.Z.Client.ts",  # the same, with the "v"
    "src/normal/file.ts",  # no capitals at all
]


def main() -> int:
    failures: list[str] = []
    for path in MUST_BE_SKIPPED:
        if not check.PLACEHOLDER_VERSION_RE.search(path):
            failures.append(f"should be skipped as a placeholder but is not: {path}")
    for path in MUST_STILL_BE_CHECKED:
        if check.PLACEHOLDER_VERSION_RE.search(path):
            failures.append(
                f"would be SILENTLY SKIPPED, though it is a real claim: {path}"
            )
    if failures:
        print("FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(
        f"OK -- {len(MUST_BE_SKIPPED)} placeholders skipped, "
        f"{len(MUST_STILL_BE_CHECKED)} real names still checked"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
