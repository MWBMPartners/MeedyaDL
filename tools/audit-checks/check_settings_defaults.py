#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Do the app's two lists of starting settings still agree?

The settings a brand-new install gets are written down twice: once in Rust
(`AppSettings::default()` in `src-tauri/src/models/settings.rs`) and once in
TypeScript (`DEFAULT_SETTINGS` in `src/stores/settingsStore.ts`). The Rust one
is the real one. The TypeScript one is a placeholder shown for the moment
before the real settings are read from disk.

Nothing compared them, so they drifted — and the drift was not harmless. Three
values disagreed, and two of the three were exactly the values a settings
upgrade step exists to REPAIR: the folder and file name patterns that, without
the identifier on the end, let two playlists with the same name overwrite each
other's file and two compilations with the same album name pile into one
folder (#545, #552). Pressing "Reset" and then "Save" put somebody straight
back onto the patterns known to lose files.

"Reset" now asks the backend for the real defaults, so the drift can no longer
do that particular harm. This check exists because the placeholder is still a
second copy, and a second copy of anything drifts eventually. It is the answer
to the question this project keeps having to ask: *what would compare the two
sides?* Nothing did, which is why this exists.

What it reports:

  1. A setting whose starting value differs between the two files.
  2. A setting present in one file and missing from the other — including a
     missing settings version number, which was its own fault: writing version
     zero to disk makes every upgrade step run again at the next launch.

What it deliberately does NOT report, because these are differences of shape
rather than of value, and reporting them would be noise nobody reads:

  * A value this check cannot compare honestly — a Rust value built by calling
    a function, or one assembled from other values, rather than written out
    literally. Those are listed at the end of a run as "not compared", so the
    count is never mistaken for "everything was checked".
  * Settings the TypeScript side has no business carrying.

Both files are read with targeted matching rather than a parser, so this runs
on any Python 3 with nothing installed — the house style for these checks.

Exit code:
  0 — no findings, OR findings without --strict
  1 — at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_settings_defaults.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SETTINGS_RS = REPO_ROOT / "src-tauri" / "src" / "models" / "settings.rs"
SETTINGS_STORE_TS = REPO_ROOT / "src" / "stores" / "settingsStore.ts"

# Settings that are genuinely allowed to differ, each with the reason.
# Anything added here needs a reason a reader can check, not just a name.
ALLOWED_TO_DIFFER: dict[str, str] = {
    # The Rust side reads this from the build. The page cannot know it.
    "settings_version": "the version number comes from the Rust build",
}


def _strip_rust_comments(text: str) -> str:
    """Remove `//` comments so a value mentioned in prose is not mistaken
    for the real one. Leaves string contents alone, since a `//` inside a
    quoted value (a web address, say) is not a comment."""
    out = []
    for line in text.splitlines():
        in_string = False
        escaped = False
        cut = len(line)
        for i, ch in enumerate(line):
            if escaped:
                escaped = False
                continue
            if ch == "\\":
                escaped = True
                continue
            if ch == '"':
                in_string = not in_string
                continue
            if not in_string and ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
                cut = i
                break
        out.append(line[:cut])
    return "\n".join(out)


def _rust_defaults() -> tuple[dict[str, str], set[str]]:
    """The literal starting values from `AppSettings::default()`.

    Returns the values it could read, and the names it found but could not
    compare honestly (a value built by calling something, rather than
    written out). Those are reported separately so a clean run never
    silently means "half of them were skipped"."""
    text = _strip_rust_comments(SETTINGS_RS.read_text(encoding="utf-8", errors="ignore"))

    # Find `impl Default for AppSettings`, then the struct being built
    # inside its `fn default`.
    #
    # Searching for the first `Self {` after the `impl` line is NOT
    # enough, and getting that wrong is how the first version of this
    # check silently reported nothing: the function's own signature ends
    # `-> Self {`, so the first match is the function body, not the
    # struct. Every field then sits one level deeper than expected and
    # the "only top-level fields" rule skips all of them. So step past
    # the signature first.
    start = text.find("impl Default for AppSettings")
    if start == -1:
        return {}, set()
    signature = text.find("fn default() -> Self {", start)
    if signature == -1:
        return {}, set()
    brace = text.find("Self {", signature + len("fn default() -> Self {"))
    if brace == -1:
        return {}, set()

    depth = 0
    end = brace
    for i in range(brace + len("Self "), len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                end = i
                break
    body = text[brace + len("Self {") : end]

    values: dict[str, str] = {}
    uncomparable: set[str] = set()

    # Only top-level fields: skip anything nested inside a sub-object.
    depth = 0
    for raw in body.splitlines():
        line = raw.strip()
        if depth == 0:
            m = re.match(r"^([a-z_][a-z0-9_]*)\s*:\s*(.+?),?\s*$", line)
            if m:
                name, value = m.group(1), m.group(2).rstrip(",").strip()
                if _is_literal(value):
                    values[name] = _normalise_rust(value)
                else:
                    uncomparable.add(name)
        depth += line.count("{") + line.count("[") + line.count("(")
        depth -= line.count("}") + line.count("]") + line.count(")")
        depth = max(depth, 0)

    return values, uncomparable


def _is_literal(value: str) -> bool:
    """Is this a value written out, rather than one built by calling
    something? `"x".to_string()` counts as written out; `compute()` does
    not."""
    if re.fullmatch(r'"[^"]*"\.to_string\(\)', value):
        return True
    if re.fullmatch(r'"[^"]*"', value):
        return True
    if value in ("true", "false"):
        return True
    if re.fullmatch(r"-?\d+(\.\d+)?(_?[a-z0-9]+)?", value):
        return True
    if value == "None":
        return True
    # A plain enum value such as `RemuxMode::Mp4box`.
    if re.fullmatch(r"[A-Z][A-Za-z0-9]*::[A-Za-z0-9_]+", value):
        return True
    return False


# Every Rust file that might hold a settings enum. Read once per run.
_RUST_SOURCES: str | None = None


def _rust_sources() -> str:
    """Every Rust source file, joined, so an enum can be found wherever it
    happens to live."""
    global _RUST_SOURCES
    if _RUST_SOURCES is None:
        parts = []
        for path in sorted((REPO_ROOT / "src-tauri" / "src").rglob("*.rs")):
            parts.append(path.read_text(encoding="utf-8", errors="ignore"))
        _RUST_SOURCES = "\n".join(parts)
    return _RUST_SOURCES


def _serde_name(enum_name: str, variant: str) -> str | None:
    """What a Rust enum value is actually called once it reaches the page.

    Reads the enum's own serde attributes rather than guessing from the
    spelling of the variant. Guessing was the first version of this check
    and it was wrong twice out of two: `VideoResolution::P2160` is
    written to disk as `2160p`, not `p2160`, because that variant carries
    its own rename; and `LogLevel::Info` is `INFO`, not `info`, because
    that enum renames everything to capitals. Both would have been
    reported as drift when nothing had drifted — which is worse than
    useless, because a check that cries wolf teaches people to skim it.

    Returns None when the enum cannot be found or its naming cannot be
    read, so the caller can record it as "not compared" rather than
    inventing an answer.
    """
    text = _rust_sources()
    m = re.search(rf"((?:#\[[^\]]*\]\s*)*)pub enum {re.escape(enum_name)}\b", text)
    if not m:
        return None

    attrs = m.group(1)
    rename_all = None
    ra = re.search(r'rename_all\s*=\s*"([^"]+)"', attrs)
    if ra:
        rename_all = ra.group(1)

    # The body of the enum, so a per-variant rename can be found.
    body_start = text.find("{", m.end())
    if body_start == -1:
        return None
    depth = 0
    body_end = body_start
    for i in range(body_start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                body_end = i
                break
    body = text[body_start:body_end]

    # A rename on this particular variant beats the whole-enum rule.
    per_variant = re.search(
        rf'#\[serde\(rename\s*=\s*"([^"]+)"\)\]\s*(?://[^\n]*\n\s*)*{re.escape(variant)}\b',
        body,
    )
    if per_variant:
        return per_variant.group(1)

    if rename_all == "lowercase":
        return variant.lower()
    if rename_all == "UPPERCASE":
        return variant.upper()
    if rename_all == "kebab-case":
        return _split_words(variant, "-")
    if rename_all == "snake_case":
        return _split_words(variant, "_")
    if rename_all in ("camelCase", "PascalCase", "SCREAMING_SNAKE_CASE"):
        # Possible to work out, but no settings enum uses these today, so
        # saying "cannot compare" is more honest than writing code nothing
        # exercises.
        return None
    if rename_all is None:
        # No rename at all: serde uses the variant's own spelling.
        return variant
    return None


def _split_words(variant: str, joiner: str) -> str:
    """`Mp4Box` -> `mp4-box` (or `mp4_box`), the way serde splits on each
    capital letter."""
    words = re.findall(r"[A-Z]+(?![a-z])|[A-Z][a-z0-9]*|[a-z0-9]+", variant)
    return joiner.join(w.lower() for w in words)


def _normalise_rust(value: str) -> str:
    """Reduce a Rust value to something comparable with the TypeScript one."""
    m = re.fullmatch(r'"([^"]*)"\.to_string\(\)', value)
    if m:
        return m.group(1)
    m = re.fullmatch(r'"([^"]*)"', value)
    if m:
        return m.group(1)
    if value == "None":
        return "null"
    m = re.fullmatch(r"([A-Z][A-Za-z0-9]*)::([A-Za-z0-9_]+)", value)
    if m:
        resolved = _serde_name(m.group(1), m.group(2))
        return resolved if resolved is not None else "__not-compared__"
    return re.sub(r"_", "", value)


def _ts_defaults() -> tuple[dict[str, str], set[str]]:
    """The starting values from `DEFAULT_SETTINGS` in the settings store."""
    text = SETTINGS_STORE_TS.read_text(encoding="utf-8", errors="ignore")
    start = text.find("const DEFAULT_SETTINGS")
    if start == -1:
        return {}, set()
    brace = text.find("{", start)
    depth = 0
    end = brace
    for i in range(brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                end = i
                break
    body = text[brace + 1 : end]

    values: dict[str, str] = {}
    uncomparable: set[str] = set()
    depth = 0
    for raw in body.splitlines():
        line = raw.strip()
        if line.startswith("//") or line.startswith("*") or line.startswith("/*"):
            continue
        if depth == 0:
            m = re.match(r"^([a-z_][a-z0-9_]*)\s*:\s*(.+?),?\s*(?://.*)?$", line)
            if m:
                name, value = m.group(1), m.group(2).rstrip(",").strip()
                value = re.sub(r"\s*//.*$", "", value).strip()
                if re.fullmatch(r"'[^']*'", value):
                    values[name] = value[1:-1]
                elif re.fullmatch(r'"[^"]*"', value):
                    values[name] = value[1:-1]
                elif value in ("true", "false", "null"):
                    values[name] = value
                elif re.fullmatch(r"-?\d+(\.\d+)?", value):
                    values[name] = value
                else:
                    uncomparable.add(name)
        depth += line.count("{") + line.count("[")
        depth -= line.count("}") + line.count("]")
        depth = max(depth, 0)

    return values, uncomparable


def _optional_in_page_type() -> set[str]:
    """Settings the page's own type marks with a `?`, meaning the
    placeholder is allowed to leave them out.

    Without this the check reports six perfectly correct omissions, and a
    check that reports things which are fine is one people stop reading.
    """
    types_file = REPO_ROOT / "src" / "types" / "index.ts"
    if not types_file.exists():
        return set()
    text = types_file.read_text(encoding="utf-8", errors="ignore")
    return set(re.findall(r"^\s*([a-z_][a-z0-9_]*)\?\s*:", text, re.MULTILINE))


def check() -> int:
    if not SETTINGS_RS.exists() or not SETTINGS_STORE_TS.exists():
        print("### Could not find both settings files\n")
        print(f"  • {SETTINGS_RS} or {SETTINGS_STORE_TS} is missing")
        # Loud rather than quiet: a check that cannot run must never look
        # like a check that found nothing.
        return 1

    rust, rust_skipped = _rust_defaults()
    ts, ts_skipped = _ts_defaults()

    if not rust:
        print("### Could not read the starting settings from the Rust side\n")
        print(
            f"  • {SETTINGS_RS.relative_to(REPO_ROOT).as_posix()} — no "
            "`impl Default for AppSettings` could be read. This check cannot "
            "tell you anything until that is fixed."
        )
        return 1
    if not ts:
        print("### Could not read the starting settings from the page\n")
        print(
            f"  • {SETTINGS_STORE_TS.relative_to(REPO_ROOT).as_posix()} — no "
            "`DEFAULT_SETTINGS` could be read."
        )
        return 1

    differing: list[str] = []
    missing_from_ts: list[str] = []
    optional_in_page = _optional_in_page_type()

    skipped = rust_skipped | ts_skipped
    for name, rust_value in sorted(rust.items()):
        if name in ALLOWED_TO_DIFFER or name in skipped:
            continue
        if rust_value == "__not-compared__":
            skipped.add(name)
            continue
        if name not in ts:
            if name not in ts_skipped and name not in optional_in_page:
                missing_from_ts.append(name)
            continue
        if ts[name] != rust_value:
            differing.append(
                f"{name} — Rust starts it at {rust_value!r}, the page at {ts[name]!r}"
            )

    findings = 0

    if differing:
        findings += len(differing)
        print("### A setting starts at a different value in the page than in the app\n")
        for line in differing:
            print(
                f"  • {SETTINGS_STORE_TS.relative_to(REPO_ROOT).as_posix()} — {line}"
            )
        print(
            "\n    The Rust value is the real one. Two of these drifted before, "
            "and both were values a settings upgrade step exists to repair, so "
            "the drift put back a bug that had been fixed (#545, #552)."
        )
        print()

    if missing_from_ts:
        findings += len(missing_from_ts)
        print("### A setting the app has is missing from the page's copy\n")
        for name in missing_from_ts:
            print(
                f"  • {SETTINGS_STORE_TS.relative_to(REPO_ROOT).as_posix()} — "
                f"{name} is missing"
            )
        print()

    if findings == 0:
        print(
            f"OK — the page's starting settings agree with the app's "
            f"({len(rust) - len(skipped)} compared)."
        )
        if skipped:
            # Never let a clean result imply everything was checked.
            print(
                f"     {len(skipped)} not compared (a value built rather than "
                f"written out): {', '.join(sorted(skipped))}"
            )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
