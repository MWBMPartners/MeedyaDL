#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Self-test for `check_capabilities_open_path.py`.

Builds small fake repositories holding one capabilities file each and runs the
real check against them as a subprocess, asserting the printed output (the
`### ` heading included, since the workflow discards bullets without it).
Also runs it against the real repository, which must be clean.

Run directly: `python3 tools/audit-checks/test_check_capabilities_open_path.py`
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_CHECK = _HERE / "check_capabilities_open_path.py"


GOOD_OPEN = "((mailto:\\w+)|(tel:\\w+)|(https?://\\w+)|(x-apple\\.systempreferences:\\S+)).+"


def run(permissions, description="mentions opener:allow-open-path in prose", strict=True, raw=None, open_pattern=GOOD_OPEN):
    with tempfile.TemporaryDirectory() as tmp:
        cap = Path(tmp) / "src-tauri" / "capabilities"
        cap.mkdir(parents=True)
        body = raw if raw is not None else json.dumps(
            {"description": description, "permissions": permissions}, indent=2
        )
        (cap / "default.json").write_text(body, encoding="utf-8")
        (Path(tmp) / "src-tauri" / "tauri.conf.json").write_text(
            json.dumps({"plugins": {"shell": {"open": open_pattern}}}), encoding="utf-8")
        args = [sys.executable, str(_CHECK), "--root", tmp] + (["--strict"] if strict else [])
        p = subprocess.run(args, capture_output=True, text=True)
        return p.returncode, p.stdout + p.stderr


def main() -> int:
    failures: list[str] = []

    def expect(label, cond):
        if not cond:
            failures.append(label)

    code, out = run(["core:default", "notification:allow-is-permission-granted",
                     "opener:allow-reveal-item-in-dir"])
    expect("clean file: exit 0, no bullets (prose mention ignored)", code == 0 and "•" not in out)

    for bad in ("opener:allow-open-path", "opener:default"):
        code, out = run(["core:default", bad])
        expect(f"{bad}: exit 1 under --strict", code == 1)
        expect(f"{bad}: bullet under a ### heading",
               "### " in out and "•" in out and "default.json:" in out and bad in out)
        code, _ = run(["core:default", bad], strict=False)
        expect(f"{bad}: exit 0 without --strict", code == 0)

    code, out = run([{"identifier": "opener:allow-open-path", "allow": [{"path": "**"}]}])
    expect("object form is caught", code == 1 and "•" in out)

    code, out = run(["notification:default"])
    expect("other notification permission is caught", code == 1 and "notification:default" in out)

    code, out = run(["notification:allow-notify"])
    expect("notification:allow-notify is caught", code == 1)

    code, out = run([], raw="{ not json")
    expect("unreadable file is itself a finding", code == 1 and "•" in out and "###" in out)

    code, out = run([], open_pattern="((mailto:\\w+)|(https?://\\w+)|(file:\\S+)).+")
    expect("widened shell open (file:) is caught", code == 1 and "file" in out and "tauri.conf.json:0" in out)
    for label, pat in (
        ("word scheme", "((https?://\\\\w+)|(\\\\w+:.*))"),
        ("any text", "((https?://\\\\w+)|(.+))"),
        ("any path", "((https?://\\\\w+)|(/.*))"),
    ):
        code, out = run([], open_pattern=pat)
        expect(f"bypass '{label}' is caught", code == 1 and "tauri.conf.json:0" in out)
    code, out = run([], open_pattern=".*")
    expect("shell open with no scheme list is caught", code == 1)
    code, out = run([], raw="{ not json")
    expect("unknown line is reported as 0, not 1", "default.json:0" in out and "default.json:1" not in out)

    real = subprocess.run([sys.executable, str(_CHECK), "--strict"], capture_output=True, text=True)
    expect("real repository is clean", real.returncode == 0 and "•" not in real.stdout)

    if failures:
        print("FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("OK -- all capability check cases behave")
    return 0


if __name__ == "__main__":
    sys.exit(main())
