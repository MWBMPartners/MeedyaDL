#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Capability grants that would let the page open files or post notifications.

Two permissions are deliberately NOT granted to the WebView in
`src-tauri/capabilities/*.json`:

1. Opening a file. The old `opener:allow-open-path` grant had a scope of `**`,
   which on Windows meant anything running in the page could run any program
   on the computer. Opening now goes through the `open_downloaded_file`
   command, which allows only the kinds of file the app produces. This check
   reports `opener:allow-open-path`, `opener:default` (a bundle that can grow
   to include it) and any other `opener:` permission that names opening a path.
   Revealing a file in the file manager (`opener:allow-reveal-item-in-dir`)
   runs nothing and is allowed.

2. Posting a system notification. Since #1431 only the backend posts them. The
   one notification permission the page holds is
   `notification:allow-is-permission-granted`; any other `notification:`
   permission is reported.

Only the `permissions` lists are read (not the description text, which names
the removed grant on purpose). A permission may be a plain string or an object
with an `identifier` key; both are checked.

Output: findings as `  • path:line — message` bullets under a `### ` heading
(the workflow keeps only what follows that heading). Exit 0 normally, 1 under
--strict when anything is found. A capabilities file that cannot be read as
JSON is itself a finding, so "could not check" never looks like "clean".

Usage:
  python3 tools/audit-checks/check_capabilities_open_path.py [--strict] [--root <dir>]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ALLOWED_NOTIFICATION = "notification:allow-is-permission-granted"
# Schemes the WebView may ask the system to open (`plugins.shell.open`):
# web, mail, phone, and the macOS settings deep link the app uses.
ALLOWED_OPEN_SCHEMES = {"http", "https", "mailto", "tel", "x-apple.systempreferences"}
# Permissions that let the page open an arbitrary file or folder.
FORBIDDEN_OPEN = ("opener:allow-open-path", "opener:default")


def _identifier(entry: object) -> str | None:
    """The permission name of a plain-string or object entry."""
    if isinstance(entry, str):
        return entry
    if isinstance(entry, dict) and isinstance(entry.get("identifier"), str):
        return entry["identifier"]
    return None


def _problem(name: str) -> str | None:
    """Why this permission is not allowed, or None if it is fine."""
    if name in FORBIDDEN_OPEN or (
        name.startswith("opener:") and "open-path" in name
    ):
        return (
            f"`{name}` lets the page open any file. Opening goes through the "
            "`open_downloaded_file` command instead; only "
            "`opener:allow-reveal-item-in-dir` is allowed here."
        )
    if name.startswith("notification:") and name != ALLOWED_NOTIFICATION:
        return (
            f"`{name}` is not allowed. The page may hold only "
            f"`{ALLOWED_NOTIFICATION}`; the backend posts notifications."
        )
    return None


def _line_of(text: str, name: str) -> int:
    """1-based line of the first quoted use of `name` in a permissions list.

    The description field names the removed grant in prose, so only a
    whole-string match (`"name"`) counts, and only past the "permissions" key.
    """
    start = text.find('"permissions"')
    start = 0 if start < 0 else start
    idx = text.find(f'"{name}"', start)
    if idx < 0:
        idx = text.find(f'"{name}"')
    if idx < 0:
        return 0  # unknown line: say 0 rather than pointing at line 1
    return text.count("\n", 0, idx) + 1


def check_shell_open(root: Path) -> list[str]:
    """Report if `plugins.shell.open` allows more than web/mail/phone links."""
    conf = root / "src-tauri" / "tauri.conf.json"
    rel = "src-tauri/tauri.conf.json"
    try:
        data = json.loads(conf.read_text(encoding="utf-8"))
    except (OSError, ValueError) as err:
        return [f"  • {rel}:0 — could not be read ({err}); shell open pattern not checked"]
    pattern = (((data.get("plugins") or {}).get("shell") or {}).get("open")
               if isinstance(data, dict) else None)
    if pattern is True:
        return [f"  • {rel}:0 — `plugins.shell.open` is `true`, which lets the page open any address or file"]
    if not isinstance(pattern, str):
        return []  # nothing is opened through the plugin
    found = re.findall(r"(?:^|[(|])([A-Za-z][\w\\.+-]*\??):", pattern)
    schemes = {s.replace("\\", "").replace("?", "").lower() for s in found}
    if not schemes:
        return [f"  • {rel}:0 — `plugins.shell.open` has no recognisable scheme list; widened or unreadable pattern"]
    extra = sorted(schemes - ALLOWED_OPEN_SCHEMES)
    if extra:
        return [
            f"  • {rel}:0 — `plugins.shell.open` allows more than web, mail, phone and settings links: "
            + ", ".join(extra)
        ]
    return []


def check(root: Path) -> list[str]:
    findings: list[str] = []
    cap_dir = root / "src-tauri" / "capabilities"
    files = sorted(cap_dir.glob("*.json")) if cap_dir.is_dir() else []
    if not files:
        findings.append(
            f"  • {cap_dir.relative_to(root) if cap_dir.is_relative_to(root) else cap_dir}"
            ":0 — no capability files found, so nothing could be checked"
        )
        return findings
    findings.extend(check_shell_open(root))
    for path in files:
        rel = path.relative_to(root)
        try:
            text = path.read_text(encoding="utf-8")
            data = json.loads(text)
        except (OSError, ValueError) as err:
            findings.append(f"  • {rel}:0 — could not be read as JSON ({err}); not checked")
            continue
        perms = data.get("permissions") if isinstance(data, dict) else None
        if not isinstance(perms, list):
            findings.append(f"  • {rel}:0 — has no `permissions` list; not checked")
            continue
        for entry in perms:
            name = _identifier(entry)
            if name is None:
                continue
            why = _problem(name)
            if why:
                findings.append(f"  • {rel}:{_line_of(text, name)} — {why}")
    return findings


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parents[2]
    if "--root" in argv:
        i = argv.index("--root")
        if i + 1 >= len(argv):
            print("::error::--root needs a directory after it.")
            return 2
        root = Path(argv[i + 1]).resolve()
    findings = check(root)
    if findings:
        print("### Capability grants that open files or post notifications")
        print()
        print("\n".join(findings))
        return 1 if "--strict" in argv else 0
    print("OK -- no capability grants open files or post notifications")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
