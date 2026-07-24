#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Tauri IPC contract consistency check.

MeedyaDL's frontend talks to the Rust backend exclusively through Tauri's
`invoke()` bridge. Three sources have to agree for an IPC call to work at
runtime, and the Rust compiler only enforces ONE of the three links:

  (A) Backend definition  — a `#[tauri::command]` function under
                            `src-tauri/src/`.
  (B) Backend registration — the function listed in the
                            `tauri::generate_handler![ ... ]` block in
                            `src-tauri/src/lib.rs`.
  (C) Frontend call site  — an `invoke('command_name')` literal somewhere
                            under `src/`.

The compiler rejects (B) referencing a non-existent (A) — that link is safe.
But it is blind to the two links that actually break shipping builds:

  * (A) WITHOUT (B): a command is defined but never registered. It compiles
    cleanly, yet every frontend `invoke()` of it fails at runtime with
    "command <x> not found". Dead IPC.

  * (C) WITHOUT (B): the frontend invokes a name that no registered command
    answers (typo, renamed-but-not-updated, or deleted backend command).
    Also a runtime "command not found" — only discovered when a user clicks
    the button.

This is the MeedyaDL analog of WebMS-Intra's `check_route_targets.py`
("code references a target that doesn't exist in another source").

Exit code:
  0 — no findings (or only informational), OR findings without --strict
  1 — at least one (C)-without-(B) finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_ipc_commands.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_SRC = REPO_ROOT / "src-tauri" / "src"
LIB_RS = RUST_SRC / "lib.rs"
FRONTEND_SRC = REPO_ROOT / "src"

# A `#[tauri::command]` / `#[command]` attribute, with or without args
# (e.g. `#[tauri::command(rename_all = "snake_case")]`).
COMMAND_ATTR_RE = re.compile(r"#\[\s*(?:tauri::)?command\b")
# The function name on (or shortly after) the attribute line.
FN_NAME_RE = re.compile(r"\bfn\s+([a-z_][A-Za-z0-9_]*)")
# A frontend invoke() call with a string-literal target, with or without the
# `invoke<ReturnType>(...)` generic turbofish.
INVOKE_RE = re.compile(r"""invoke\s*(?:<[^>]*>)?\s*\(\s*['"]([a-z_][a-z0-9_]*)['"]""")

# The JSDoc usage example in tauri-commands.ts uses this placeholder; it is
# never a real command. Excluded explicitly in addition to comment-stripping.
PLACEHOLDER_NAMES = {"command_name"}


def strip_line_comments(text: str) -> str:
    """Strip // line comments and /* */ block comments while preserving line
    numbers (block comments are replaced by an equal count of newlines so
    reported line numbers still line up with the original file)."""
    text = re.sub(
        r"/\*.*?\*/",
        lambda m: "\n" * m.group(0).count("\n"),
        text,
        flags=re.DOTALL,
    )
    # Strip // to end-of-line. This is a heuristic (it will also blank a //
    # that appears inside a string literal), which is acceptable here: we
    # only ever match command-name *identifiers*, and a false strip can at
    # most hide a call site, never invent one.
    text = re.sub(r"//[^\n]*", "", text)
    return text


def collect_defined_commands() -> dict[str, tuple[str, int]]:
    """Scan every .rs file under src-tauri/src for `#[tauri::command]`
    functions. Returns {fn_name: (relative_path, line_no)}."""
    defined: dict[str, tuple[str, int]] = {}
    for rs in sorted(RUST_SRC.rglob("*.rs")):
        try:
            lines = rs.read_text(encoding="utf-8", errors="ignore").splitlines()
        except OSError:
            continue
        rel = str(rs.relative_to(REPO_ROOT))
        for i, line in enumerate(lines):
            if not COMMAND_ATTR_RE.search(line):
                continue
            # The fn may be on this line or up to a few attribute/doc lines
            # below the #[tauri::command] attribute.
            for j in range(i, min(i + 8, len(lines))):
                m = FN_NAME_RE.search(lines[j])
                if m:
                    defined.setdefault(m.group(1), (rel, j + 1))
                    break
    return defined


def collect_registered_commands() -> set[str]:
    """Extract the generate_handler![ ... ] block from lib.rs and collect the
    final path segment of every registered command (e.g.
    `commands::system::get_platform_info` -> `get_platform_info`)."""
    text = LIB_RS.read_text(encoding="utf-8", errors="ignore")
    start = text.find("generate_handler![")
    if start == -1:
        print("WARNING: generate_handler![ not found in lib.rs", file=sys.stderr)
        return set()
    # Bracket-match from the opening [ to its partner so we capture exactly
    # the macro's argument list, nothing after it.
    open_idx = text.index("[", start)
    depth = 0
    end_idx = open_idx
    for idx in range(open_idx, len(text)):
        c = text[idx]
        if c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth == 0:
                end_idx = idx
                break
    block = strip_line_comments(text[open_idx + 1 : end_idx])
    registered: set[str] = set()
    for raw in block.split(","):
        token = raw.strip()
        if not token:
            continue
        # Take the segment after the last `::` path separator.
        ident = token.split("::")[-1].strip()
        if re.fullmatch(r"[a-z_][a-z0-9_]*", ident):
            registered.add(ident)
    return registered


def collect_frontend_invokes() -> dict[str, tuple[str, int]]:
    """Scan TS/TSX under src/ (excluding test + mock files) for invoke()
    string-literal targets. Returns {name: (relative_path, line_no)} keeping
    the first occurrence."""
    invokes: dict[str, tuple[str, int]] = {}
    for ext in ("*.ts", "*.tsx"):
        for ts in sorted(FRONTEND_SRC.rglob(ext)):
            rel = str(ts.relative_to(REPO_ROOT))
            # Test scaffolding mocks invoke() with arbitrary names — skip it.
            if ".test." in ts.name or "/test/" in rel.replace("\\", "/") or "__tests__" in rel:
                continue
            try:
                raw = ts.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            stripped = strip_line_comments(raw)
            for m in INVOKE_RE.finditer(stripped):
                name = m.group(1)
                if name in PLACEHOLDER_NAMES:
                    continue
                line_no = stripped[: m.start()].count("\n") + 1
                invokes.setdefault(name, (rel, line_no))
    return invokes


def check() -> int:
    defined = collect_defined_commands()
    registered = collect_registered_commands()
    invokes = collect_frontend_invokes()

    print(f"#[tauri::command] functions defined : {len(defined)}")
    print(f"Commands registered in lib.rs       : {len(registered)}")
    print(f"Distinct frontend invoke() targets  : {len(invokes)}")
    print()

    # (A) without (B): defined but never registered -> dead IPC.
    defined_not_registered = sorted(set(defined) - registered)
    # (C) without (B): frontend calls a command that is not registered.
    frontend_unregistered = sorted(set(invokes) - registered)
    # (B) without (A): registered name with no discoverable definition.
    # Normally a compile error, so a non-empty set here usually means this
    # parser missed an unusual definition site — surfaced as informational.
    registered_not_defined = sorted(registered - set(defined))

    high_severity = 0

    if frontend_unregistered:
        print("### Frontend invoke() targets with no registered backend command\n")
        for name in frontend_unregistered:
            rel, ln = invokes[name]
            print(f"  • {rel}:{ln} — invoke('{name}') has no matching registered #[tauri::command]")
        print()
        high_severity += len(frontend_unregistered)

    if defined_not_registered:
        print("### Commands defined but NOT registered in generate_handler![] (unreachable IPC)\n")
        for name in defined_not_registered:
            rel, ln = defined[name]
            print(f"  • {rel}:{ln} — `{name}` is #[tauri::command] but absent from lib.rs generate_handler![]")
        print()

    if registered_not_defined:
        print("### Registered in lib.rs but no #[tauri::command] definition found (informational)\n")
        for name in registered_not_defined:
            print(f"  • lib.rs — `{name}` is registered but this checker found no defining function (parser gap?)")
        print()

    if not (frontend_unregistered or defined_not_registered or registered_not_defined):
        print("OK — frontend, backend definitions, and registration are consistent.")

    if high_severity and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
