#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/test_check_updater_manifest_keys.py
=======================================================

Negative test for `check_updater_manifest_keys.py`.

WHY THIS EXISTS
---------------
A check nobody has ever seen fail is not evidence of anything. It might
be catching the drift it was written for, or it might be quietly
matching nothing at all — and from the outside those two look identical,
because both print a tick. The whole reason the updater-manifest bug
survived so long is that a checker went on agreeing with a belief
instead of ever being allowed to contradict it. So this file makes the
check fail on purpose, four different ways, and asserts it noticed.

HOW IT WORKS
------------
Each case builds a small fake repository in a temporary folder: a copy
of the real shared release script (or a deliberately broken one) plus a
synthetic workflow file. The check is then run against that folder with
`--root`, as a real subprocess, so what is asserted is the actual
printed output a person or the PR-comment pipeline would see — heading
included.

A small fake tree is used rather than a full copy of the repository
because the only two things the check reads are the shared script and
the workflow folder. Copying several hundred megabytes to exercise two
files would make the test slow enough that people stop running it.

Pure stdlib, no pytest — same house style as the checks themselves.
Run directly: `python3 tools/audit-checks/test_check_updater_manifest_keys.py`
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

_THIS_DIR = Path(__file__).resolve().parent
_REPO_ROOT = _THIS_DIR.parents[1]
_CHECK = _THIS_DIR / "check_updater_manifest_keys.py"
_REAL_SCRIPT = _REPO_ROOT / "scripts" / "release" / "updater-manifest.sh"

# A workflow that names no platform key anywhere. Each case adds its own
# drift to a copy of this, so the only difference between a clean run and
# a failing one is the line the case inserted.
_CLEAN_WORKFLOW = """\
name: Fake release
on: workflow_dispatch
jobs:
  finalize:
    runs-on: ubuntu-latest
    steps:
      - name: Rebuild the updater manifest
        run: |
          bash scripts/release/updater-manifest.sh build "$TAG"
      - name: Verify the updater manifest
        # A comment may mention linux-x86_64 freely — explaining a thing
        # is not the same as writing it down as a second source of truth.
        run: |
          bash scripts/release/updater-manifest.sh verify "$TAG" strict
"""


def _make_tree(tmp: Path, workflow_body: str, script_text: str | None = None) -> Path:
    """Build a minimal fake repository and return its root.

    `script_text` of None means "use the real shared script", which is
    what every case except the blinded-check one wants.
    """
    root = tmp / "fake-repo"
    (root / ".github" / "workflows").mkdir(parents=True)
    (root / "scripts" / "release").mkdir(parents=True)

    if script_text is None:
        shutil.copyfile(_REAL_SCRIPT, root / "scripts" / "release" / "updater-manifest.sh")
    else:
        (root / "scripts" / "release" / "updater-manifest.sh").write_text(
            script_text, encoding="utf-8"
        )

    (root / ".github" / "workflows" / "fake-release.yml").write_text(
        workflow_body, encoding="utf-8"
    )
    return root


def _run(root: Path, strict: bool = False) -> tuple[int, str]:
    """Run the check as a real subprocess against `root`."""
    argv = [sys.executable, "-B", str(_CHECK), "--root", str(root)]
    if strict:
        argv.append("--strict")
    proc = subprocess.run(argv, capture_output=True, text=True)
    return proc.returncode, proc.stdout + proc.stderr


def _bullets(output: str) -> list[str]:
    """Every finding line the check printed."""
    return [line for line in output.splitlines() if line.lstrip().startswith("•")]


def _has_heading(output: str) -> bool:
    """True when the output carries a `### ` heading.

    This is asserted on every failing case for a reason that is easy to
    forget: `pr-security.yml` pulls a check's findings out with
    `grep -A100 '###'`. Bullets printed without a heading are thrown
    away, silently, and the check appears to pass forever.
    """
    return any(line.startswith("### ") for line in output.splitlines())


# ---------------------------------------------------------------------------
# Case 1 — a brand new platform key written into a workflow.
#
# The everyday way this drifts: somebody adds a platform, wires it into
# one workflow because that is the one they were looking at, and the
# other copy of the list never learns about it.
# ---------------------------------------------------------------------------
def case_new_key_in_workflow() -> list[str]:
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        workflow = _CLEAN_WORKFLOW + (
            "      - name: Sneak in a new platform\n"
            "        run: |\n"
            "          for key in linux-riscv64-deb; do :; done\n"
        )
        root = _make_tree(Path(tmp), workflow)

        code, out = _run(root)
        bullets = _bullets(out)
        if len(bullets) != 1:
            failures.append(
                f"case 1: expected exactly 1 finding for the new key, got "
                f"{len(bullets)}:\n{out}"
            )
        elif "linux-riscv64-deb" not in bullets[0]:
            failures.append(f"case 1: the finding does not name the key:\n{bullets[0]}")
        elif "fake-release.yml:" not in bullets[0]:
            failures.append(
                f"case 1: the finding does not name the file and line:\n{bullets[0]}"
            )
        if not _has_heading(out):
            failures.append("case 1: findings were printed with no '### ' heading")
        if code != 0:
            failures.append(f"case 1: expected exit 0 without --strict, got {code}")

        strict_code, _ = _run(root, strict=True)
        if strict_code != 1:
            failures.append(f"case 1: expected exit 1 with --strict, got {strict_code}")
    return failures


# ---------------------------------------------------------------------------
# Case 2 — the exact loop this work deletes, put back.
#
# This is the case that matters most. Every one of these six keys is
# real and correct. A check that only complained about keys it did not
# recognise would wave this straight through — and this is the precise
# shape of the bug being fixed: a second, hand-written copy of a list
# that already exists somewhere else.
# ---------------------------------------------------------------------------
def case_rehardcoded_existing_keys() -> list[str]:
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        workflow = _CLEAN_WORKFLOW + (
            "      - name: Re-add the old hand-written check\n"
            "        run: |\n"
            "          for key in darwin-aarch64 windows-x86_64 windows-x86_64-nsis"
            " windows-aarch64 windows-aarch64-nsis linux-x86_64; do\n"
            "            echo \"$key\"\n"
            "          done\n"
        )
        root = _make_tree(Path(tmp), workflow)

        code, out = _run(root)
        bullets = _bullets(out)
        if len(bullets) != 6:
            failures.append(
                f"case 2: expected 6 findings (one per key in the re-added loop), "
                f"got {len(bullets)}:\n{out}"
            )
        for key in (
            "darwin-aarch64",
            "windows-x86_64",
            "windows-x86_64-nsis",
            "windows-aarch64",
            "windows-aarch64-nsis",
            "linux-x86_64",
        ):
            if not any(f"'{key}'" in b for b in bullets):
                failures.append(f"case 2: no finding named the key '{key}'")
        if not _has_heading(out):
            failures.append("case 2: findings were printed with no '### ' heading")
        if code != 0:
            failures.append(f"case 2: expected exit 0 without --strict, got {code}")

        strict_code, _ = _run(root, strict=True)
        if strict_code != 1:
            failures.append(f"case 2: expected exit 1 with --strict, got {strict_code}")
    return failures


# ---------------------------------------------------------------------------
# Case 3 — the check itself goes blind.
#
# If the shared script is renamed, moved or restructured, this check has
# nothing left to compare against. It must say so, loudly. A check that
# quietly passes because it has stopped reading anything is worse than
# no check at all, because it also stops anyone looking.
# ---------------------------------------------------------------------------
def case_blinded_check() -> list[str]:
    failures: list[str] = []
    emptied_script = (
        "#!/usr/bin/env bash\n"
        "# The list has been moved somewhere this check does not know about.\n"
        "manifest_rows() {\n"
        "  cat <<EOF\n"
        "EOF\n"
        "}\n"
    )
    with tempfile.TemporaryDirectory() as tmp:
        root = _make_tree(Path(tmp), _CLEAN_WORKFLOW, script_text=emptied_script)

        code, out = _run(root)
        bullets = _bullets(out)
        if not bullets:
            failures.append(
                "case 3: an empty manifest_rows() produced no finding — the check "
                f"passed while reading nothing:\n{out}"
            )
        elif "nothing to compare against" not in bullets[0]:
            failures.append(
                f"case 3: the finding does not say the check is blind:\n{bullets[0]}"
            )
        if not _has_heading(out):
            failures.append("case 3: findings were printed with no '### ' heading")
        if code != 0:
            failures.append(f"case 3: expected exit 0 without --strict, got {code}")

        strict_code, _ = _run(root, strict=True)
        if strict_code != 1:
            failures.append(f"case 3: expected exit 1 with --strict, got {strict_code}")

    # The same must hold when the script is missing altogether, which is
    # what a rename actually looks like from this check's point of view.
    with tempfile.TemporaryDirectory() as tmp:
        root = _make_tree(Path(tmp), _CLEAN_WORKFLOW)
        (root / "scripts" / "release" / "updater-manifest.sh").unlink()
        code, out = _run(root)
        if not _bullets(out):
            failures.append(
                f"case 3: a missing shared script produced no finding:\n{out}"
            )
        if not _has_heading(out):
            failures.append("case 3: missing-script finding had no '### ' heading")
    return failures


# ---------------------------------------------------------------------------
# Case 4 — silent on the real repository.
#
# Zero findings on a clean tree is mandatory house rule. A check that
# cries wolf on day one gets switched off in week two.
# ---------------------------------------------------------------------------
def case_clean_tree() -> list[str]:
    failures: list[str] = []
    code, out = _run(_REPO_ROOT)
    bullets = _bullets(out)
    if bullets:
        rendered = "\n".join(bullets)
        failures.append(
            f"case 4: the real repository produced {len(bullets)} finding(s):\n{rendered}"
        )
    if code != 0:
        failures.append(f"case 4: expected exit 0 on the clean tree, got {code}")

    strict_code, _ = _run(_REPO_ROOT, strict=True)
    if strict_code != 0:
        failures.append(
            f"case 4: expected exit 0 with --strict on the clean tree, got {strict_code}"
        )
    return failures


CASES = [
    ("a new platform key written into a workflow is caught", case_new_key_in_workflow),
    ("re-adding the old six-key loop is caught", case_rehardcoded_existing_keys),
    ("a check with nothing to compare against says so", case_blinded_check),
    ("the real repository is silent", case_clean_tree),
]


def main() -> int:
    failures: list[str] = []
    for label, run_case in CASES:
        case_failures = run_case()
        if case_failures:
            print(f"FAIL — {label}")
            failures.extend(case_failures)
        else:
            print(f"PASS — {label}")

    print()
    if failures:
        print(f"{len(failures)} assertion(s) failed:\n")
        for failure in failures:
            print(f"  • {failure}")
        return 1
    print(f"All {len(CASES)} cases behaved as expected.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
