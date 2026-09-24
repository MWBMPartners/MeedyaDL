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
the build.

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
    (
        "an `if` inside the shell script is not mistaken for the step's condition",
        f"      - name: Export it\n        run: |\n          if: nonsense\n{EXPORT}",
        True,
        False,
    ),
]


def main() -> int:
    failures: list[str] = []
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
    print(f"OK -- {len(CASES)} cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())
