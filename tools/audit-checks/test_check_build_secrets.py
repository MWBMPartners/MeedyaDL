#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/test_check_build_secrets.py
===============================================

Tests for how `check_build_secrets.py` decides that a value exported to
`$GITHUB_ENV` earlier in a job really reaches a build step.

WHY THIS EXISTS
---------------
That check exists so a feature switched on by a build-time value cannot
ship silently dead. Its first per-step version counted any line shaped
like an export -- including a commented-out one, and one in a step with
`if: false` that never runs. Codex proved it in the batch-4 review: with
the only Safari-version export commented out, `--strict` still said "OK"
and exited 0. These tests pin the rule so that cannot come back.

Each case is a small made-up workflow job, fed straight to the check's
own functions -- no copy of the real release.yml is touched.

Proven able to fail: with the old line-by-line scan put back, the
commented-out and `if: false` cases both report the value as reaching
the build. Codex's second round found four more holes (a comment after
code on the same line, a YAML comment cutting a step short, and two
multi-line conditions compared by their `>-` marker); its third round
found two ways past the shell-comment reader that fixed the first of
those, and its fourth round beat the rule that replaced the reader (a
`#` INSIDE the loose match). Now only a line that is, in its entirety, a
plain export is counted. Every case Codex used has a case here, and the
round-4 one is also run end to end through the whole check, because a
function-level case cannot show a finding actually reaches the report.

Pure stdlib, no pytest, same house style as the checks themselves.
Run directly: `python3 tools/audit-checks/test_check_build_secrets.py`
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("check_build_secrets", HERE / "check_build_secrets.py")
assert _spec and _spec.loader
check = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check)


def job(export_step: str, build_if: str = "runner.os == 'macOS'") -> list[str]:
    """A two-step job: an exporting step (given), then the build step."""
    text = f"""jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
{export_step}
      - name: Build it
        if: {build_if}
        env:
          OTHER: x
        run: npm run build
"""
    return text.splitlines()


def exports_for(lines: list[str]) -> tuple[set[str], dict[str, list[str]]]:
    """Runs the real function against the build step in `lines`."""
    job_start = 2  # 1-indexed line of `  publish:`
    steps = check._step_ranges(lines, job_start, len(lines) + 1)
    name, indent, sstart, send = next(s for s in steps if s[0] == "Build it")
    return check._github_env_exports_before(lines, job_start, (job_start, indent, sstart, send))


EXPORT = '          echo "SAFARI=1" >> "$GITHUB_ENV"'

CASES = [
    # (description, exporting step, expected certain?, expected uncertain?)
    (
        "an ordinary export with no condition reaches the build",
        f"      - name: Export it\n        run: |\n{EXPORT}",
        True,
        False,
    ),
    (
        "a commented-out export does NOT count",
        f"      - name: Export it\n        run: |\n          # {EXPORT.strip()}",
        False,
        False,
    ),
    (
        "an export in a step that never runs is not counted as reaching it",
        f"      - name: Export it\n        if: false\n        run: |\n{EXPORT}",
        False,
        True,
    ),
    (
        "an export under some OTHER condition is reported, not assumed",
        f"      - name: Export it\n        if: runner.os == 'Linux'\n        run: |\n{EXPORT}",
        False,
        True,
    ),
    (
        "an export under the build step's OWN condition does count",
        f"      - name: Export it\n        if: runner.os == 'macOS'\n        run: |\n{EXPORT}",
        True,
        False,
    ),
    # Any `#` before the export on its line: never counted, always
    # reported. The three shapes Codex used, across rounds 2 and 3.
    (
        "an export after `true #` is not counted, and is reported",
        f"      - name: Export it\n        run: |\n          true # {EXPORT.strip()}",
        False,
        True,
    ),
    (
        "an export after `true;#` (a comment straight after an operator) is not counted",
        f"      - name: Export it\n        run: |\n          true;# {EXPORT.strip()}",
        False,
        True,
    ),
    (
        "an escaped quote before a `#` cannot smuggle an export through",
        f'      - name: Export it\n        run: |\n          printf "%s" "\\"" # {EXPORT.strip()}',
        False,
        True,
    ),
    (
        "a `#` INSIDE what the loose pattern matches cannot smuggle an export through",
        '      - name: Export it\n        run: |\n          echo "SAFARI=1" # " >> "$GITHUB_ENV"',
        False,
        True,
    ),
    (
        "anything after the export on the line means it is not counted",
        f"      - name: Export it\n        run: |\n{EXPORT} ; exit 0",
        False,
        True,
    ),
    # Two shell ERRORS that the first strict pattern still accepted
    # (Codex, batch-4 round 5).
    (
        "a backslash escaping the closing quote is not a working export",
        '      - name: Export it\n        run: |\n          echo "SAFARI=1\\" >> "$GITHUB_ENV"',
        False,
        True,
    ),
    (
        "an unbalanced quote around $GITHUB_ENV is not a working export",
        '      - name: Export it\n        run: |\n          echo "SAFARI=1" >> "$GITHUB_ENV',
        False,
        True,
    ),
    # Commands and arithmetic can fail and export nothing (Codex, round 6).
    (
        "an unfinished command in the value is not a working export",
        '      - name: Export it\n        run: |\n          echo "SAFARI=$(" >> "$GITHUB_ENV"',
        False,
        True,
    ),
    (
        "arithmetic that fails is not a working export",
        '      - name: Export it\n        run: |\n          echo "SAFARI=$((1/0))" >> "$GITHUB_ENV"',
        False,
        True,
    ),
    (
        "any command at all is refused, since it might fail",
        '      - name: Export it\n        run: |\n          echo "SAFARI=`date`" >> "$GITHUB_ENV"',
        False,
        True,
    ),
    (
        "a GitHub placeholder holding quoted text is not trusted",
        "      - name: Export it\n        run: |\n          echo \"SAFARI=${{ '$((1/0))' }}\" >> \"$GITHUB_ENV\"",
        False,
        True,
    ),
    # What the real release.yml actually uses must keep counting.
    (
        "a plain variable in the value still counts",
        '      - name: Export it\n        run: |\n          echo "SAFARI=$VALUE" >> "$GITHUB_ENV"',
        True,
        False,
    ),
    (
        "a GitHub ${{ }} placeholder in the value still counts",
        '      - name: Export it\n        run: |\n          echo "SAFARI=/usr/${{ matrix.x }}/y" >> "$GITHUB_ENV"',
        True,
        False,
    ),
    (
        "$GITHUB_ENV with no quotes at all is still a working export",
        '      - name: Export it\n        run: |\n          echo "SAFARI=1" >> $GITHUB_ENV',
        True,
        False,
    ),
    # A `#` inside the value is part of the value, and must not cost a
    # working export.
    (
        "a `#` inside the exported value does not stop it counting",
        '      - name: Export it\n        run: |\n          echo "SAFARI=#1" >> "$GITHUB_ENV"',
        True,
        False,
    ),
    (
        "a `${#...}` length inside the value does not stop it counting",
        '      - name: Export it\n        run: |\n          echo "SAFARI=${#list}" >> "$GITHUB_ENV"',
        True,
        False,
    ),
    (
        "a YAML comment inside the step does not cut the step short",
        f"      - name: Export it\n      # a note at the dash's own depth\n        run: |\n{EXPORT}",
        True,
        False,
    ),
    (
        "a multi-line condition is never taken to match the build's",
        f"      - name: Export it\n        if: >-\n          runner.os == 'Linux'\n        run: |\n{EXPORT}",
        False,
        True,
    ),
    (
        "an `if` inside the shell script is not mistaken for the step's condition",
        f"      - name: Export it\n        run: |\n          if: nonsense\n{EXPORT}",
        True,
        False,
    ),
]


# Both steps written with `if: >-`: the markers are identical, the
# conditions are not. This is the exact case Codex reproduced.
BOTH_MULTILINE = (
    "a multi-line export condition and a multi-line build condition are not "
    "assumed equal just because both start `>-`",
    f"      - name: Export it\n        if: >-\n          runner.os == 'Linux'\n        run: |\n{EXPORT}",
)


def end_to_end_failure() -> str | None:
    """Runs the WHOLE check against a copy of the real release.yml whose
    Safari export has been replaced by Codex's round-4 line, and requires
    a failing exit and a printed finding. The function-level cases above
    cannot show that a finding actually reaches the report; this does.

    The copy is written inside the audit-checks folder -- never under
    .github/workflows/, where a stray file would be a live workflow -- under
    a name unique to this run, so two runs at once cannot overwrite or
    delete each other's copy (Codex, batch-4 round 5), and it is deleted
    afterwards even if this fails. It has to be inside the repository,
    because the check reports paths relative to it."""
    import contextlib
    import io as _io
    import os
    import tempfile

    real = check.RELEASE_WORKFLOW
    text = real.read_text(encoding="utf-8")
    working = '            echo "MEEDYADL_SAFARI_VERSION=$VALUE" >> "$GITHUB_ENV"'
    if working not in text:
        return "could not find the Safari export line in release.yml to replace"
    disabled = '            echo "MEEDYADL_SAFARI_VERSION=1" # " >> "$GITHUB_ENV"'
    # mkstemp creates the file and hands back its name BEFORE anything is
    # written, so the clean-up below covers a failed write too (a full
    # disk, say). The first version wrote first and only then entered the
    # try, so a failed write left the copy behind (Codex, round 6).
    fd, name = tempfile.mkstemp(dir=HERE, prefix=".tmp-release-copy-", suffix=".yml")
    copy = Path(name)
    saved_argv = sys.argv[:]
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text.replace(working, disabled, 1))
        check.RELEASE_WORKFLOW = copy
        sys.argv = [sys.argv[0], "--strict"]
        out = _io.StringIO()
        with contextlib.redirect_stdout(out):
            code = check.check()
        report = out.getvalue()
    finally:
        check.RELEASE_WORKFLOW = real
        sys.argv = saved_argv
        copy.unlink(missing_ok=True)
    if code != 1:
        return f"end to end: --strict exited {code}, want 1"
    if "###" not in report or "MEEDYADL_SAFARI_VERSION" not in report:
        return "end to end: no finding naming MEEDYADL_SAFARI_VERSION under a ### heading"
    return None


def main() -> int:
    failures: list[str] = []
    problem = end_to_end_failure()
    if problem:
        failures.append(problem)
    description, step = BOTH_MULTILINE
    certain, uncertain = exports_for(job(step, build_if=">-\n          runner.os == 'macOS'"))
    if "SAFARI" in certain or "SAFARI" not in uncertain:
        failures.append(f"{description}: counted as reaching={'SAFARI' in certain} (want False)")
    for description, step, want_certain, want_uncertain in CASES:
        certain, uncertain = exports_for(job(step))
        got_certain = "SAFARI" in certain
        got_uncertain = "SAFARI" in uncertain
        if got_certain != want_certain or got_uncertain != want_uncertain:
            failures.append(
                f"{description}: counted as reaching={got_certain} (want {want_certain}), "
                f"reported as unconfirmed={got_uncertain} (want {want_uncertain})"
            )
    if failures:
        print("FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"OK -- {len(CASES) + 1} cases, plus the end-to-end run")
    return 0


if __name__ == "__main__":
    sys.exit(main())
