#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Dead-setting check: a setting the app lets someone change, that nothing in
the backend ever reads.

This is exactly the shape of bug that shipped in the video quality settings
page for a while: a "Video Fallback" list of resolutions sat in Settings,
could be reordered and edited like any other setting, and looked exactly as
meaningful as every setting next to it. It changed nothing. GAMDL treats a
requested resolution as a ceiling, not a request -- it always returns the
best quality at or below what you asked for in a single attempt, so there
was never a "resolution unavailable" failure for the list to step down
through. The list was quietly disconnected from the one thing settings are
for.

Nothing in the type system catches this. A setting field can exist in the
Rust `AppSettings` struct, have a working UI control that saves it to disk
correctly, round-trip through the settings store perfectly -- and still be
inert, because inert only means "nothing downstream ever looks at the
value", which compiles fine in every language.

This script looks for the same shape mechanically:

  (A) Every field in `pub struct AppSettings` (`src-tauri/src/models/settings.rs`).
  (B) Whether the app lets someone change it -- a `useSettingsField('<name>')`
      call, or a `<name>:` key inside an `updateSettings({...})` call,
      anywhere under `src/` (test files excluded).
  (C) Whether the backend ever reads it -- a `.<name>` token anywhere in
      `src-tauri/src/**/*.rs`, except inside `models/settings.rs` itself
      (which only ever *defines* the field, never *uses* it) and test files.

A field that is (B) but not (C), and is not listed in `FRONTEND_ONLY` below
with a reason, is a finding: the app can change it, and nothing in the
backend cares.

What this deliberately does NOT try to catch: a setting genuinely acted on
by the frontend alone is not a bug -- the theme, the UI language, how long a
toast stays on screen, and so on are real settings whose entire job is to
change how the frontend behaves, and the backend has no reason to know about
them. Those belong in `FRONTEND_ONLY`, each with the reason a person can
check. This script cannot tell "correctly frontend-only" apart from
"accidentally connected to nothing" on its own -- that judgement call is
why `FRONTEND_ONLY` exists as a maintained, reasoned list rather than a
blanket exemption.

This is a heuristic, not a real type-checker, and it shares the known limits
of every other regex-based script in this directory: it can't see a field
read through struct destructuring (`let AppSettings { name, .. } = ...`)
rather than `.name` access, and it can't see a setting changed through some
other store method that doesn't literally spell `updateSettings({ name: `.
Both directions err towards fewer false alarms rather than catching every
possible shape -- see the README's "Conventions" section for why that
trade-off is deliberate here.

Exit code:
  0 -- no findings, OR findings without --strict
  1 -- at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_settings_reach_backend.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SETTINGS_RS = REPO_ROOT / "src-tauri" / "src" / "models" / "settings.rs"
RUST_SRC = REPO_ROOT / "src-tauri" / "src"
WEB_SRC = REPO_ROOT / "src"

# Settings that ARE changeable in the app and are NOT read by the backend,
# on purpose, because the setting's entire job is to change how the
# frontend itself behaves. Each entry was checked individually -- how it's
# consumed, and confirmed there's no backend code path that also needs to
# know about it -- before being added here. See the module docstring for
# why this can't just be "anything frontend-only is fine": that's exactly
# what the dead resolution list looked like too, right up until someone
# checked.
#
# Do not add an entry here because a setting "seems like" a UI thing. Read
# where it's actually used first.
FRONTEND_ONLY: dict[str, str] = {
    "theme_override": (
        "Applied as a CSS class on <html> by useTheme() (App.tsx / "
        "hooks/useTheme.ts). Picking light/dark is a rendering decision; "
        "the backend never draws anything."
    ),
    "high_contrast": (
        "Same as theme_override -- toggles a CSS theme file "
        "(a11y-high-contrast.css) via useTheme(). Purely a rendering choice."
    ),
    "colour_blind_mode": (
        "Same as theme_override -- selects a CSS theme file "
        "(a11y-colour-blind.css) plus an SVG colour-mode query string for "
        "the sidebar logo. Purely a rendering choice."
    ),
    "auto_check_updates": (
        "Read once, in App.tsx's own startup/periodic-check effect, to "
        "decide whether to call the update-check IPC command at all. The "
        "backend's check_all_updates command has no on/off switch of its "
        "own -- the frontend deciding not to call it IS the toggle."
    ),
    "update_check_interval_hours": (
        "Drives a setInterval() timer that lives entirely in App.tsx. The "
        "backend command it calls on each tick takes no interval "
        "parameter -- the interval is a fact about the JS timer, not "
        "something the backend needs to know."
    ),
    "smart_redownload_detection": (
        "Read in DownloadForm.tsx to decide whether to call the "
        "check_redownload_status IPC command before queueing a URL. Same "
        "shape as auto_check_updates -- the backend command exists either "
        "way; the frontend simply doesn't call it when this is off."
    ),
    "clipboard_monitoring": (
        "Read inside useClipboardMonitor.ts's own polling loop to decide "
        "whether to keep calling the read_clipboard IPC command. The "
        "backend command just reads the clipboard once, unconditionally, "
        "whenever asked -- polling on/off is a frontend-timer decision."
    ),
    "abort_queue_confirm": (
        "Read by StatusBar.tsx / DownloadQueue.tsx to decide whether to "
        "show a confirmation modal before calling abort_all_downloads. "
        "The backend command aborts unconditionally when called; asking "
        "first is a frontend UX step, not a backend behaviour."
    ),
    "relocation_declined": (
        "Read once by App.tsx's startup effect to decide whether to show "
        "the macOS self-relocation offer modal again. A 'don't ask me "
        "again' flag with no backend counterpart to ask."
    ),
    "setup_completed": (
        "Read by App.tsx's startup effect to decide whether to render the "
        "SetupWizard or the main app. The backend doesn't gate any IPC "
        "command on setup status -- it's a frontend routing decision."
    ),
    "crash_report_prompt_shown": (
        "Read once by App.tsx's startup effect to decide whether to show "
        "the crash-reporting opt-in modal again. Another 'don't ask me "
        "again' flag with no backend counterpart."
    ),
    "notification_auto_dismiss_seconds": (
        "Read by uiStore.ts's toast helper to compute how many "
        "milliseconds before an in-app toast auto-dismisses. Purely a "
        "frontend timer; native OS notifications are not durationed by "
        "this app at all, so there is nothing for the backend to do with "
        "the value."
    ),
    "sidebar_collapsed": (
        "Read by Sidebar.tsx to pick a CSS width class (narrow icon-only "
        "vs. full width with labels) and by App.tsx's startup effect to "
        "apply that choice to uiStore before first paint. Whether the "
        "sidebar is drawn wide or narrow is a rendering decision the "
        "backend has no part in -- there is no download-pipeline or "
        "file-system behaviour this setting could plug into."
    ),
}


def _is_test_file(path: Path) -> bool:
    """True for a file this script should not scan as production code.

    Rust: anything ending `test.rs` or `tests.rs` with a `_` or start-of-
    name boundary before it (`config_test.rs`, `integration_tests.rs`).
    TypeScript: `.test.`/`.spec.` in the filename, or living under a
    `__tests__/` or `test/` directory -- the same convention every other
    script in this directory uses.

    The directory check needs a path RELATIVE to the repo root, not the
    absolute path `Path.rglob()` hands back. `"/test/" in rel` on an
    absolute path checks the whole path from the filesystem root down --
    including whatever the checkout itself happens to be sitting inside.
    Clone this repo under, say, `~/test/MeedyaDL`, and every single
    frontend file's absolute path contains `/test/` before the repo even
    starts, so this would have excluded the entire frontend from the scan
    and still printed "OK" as if it had checked something. Every sibling
    script in this directory relativises first for exactly this reason.
    """
    name = path.name
    if path.suffix == ".rs":
        return bool(re.search(r"(^|_)tests?\.rs$", name))
    if path.suffix in (".ts", ".tsx"):
        rel = path.relative_to(REPO_ROOT).as_posix()
        return ".test." in name or ".spec." in name or "/__tests__/" in rel or "/test/" in rel
    return False


def _iter_source(root: Path, suffixes: tuple[str, ...]):
    """Yield every non-test file under `root` with one of `suffixes`."""
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.suffix in suffixes and not _is_test_file(path):
            yield path


def _line_at(text: str, idx: int) -> int:
    """1-indexed line number of character offset `idx` in `text`."""
    return text[:idx].count("\n") + 1


# ---------------------------------------------------------------------------
# (A) Every field in `pub struct AppSettings`.
# ---------------------------------------------------------------------------

FIELD_RE = re.compile(r"^\s*pub\s+([a-z_][a-z0-9_]*)\s*:", re.MULTILINE)


def collect_app_settings_fields() -> list[str]:
    """Bracket-matches `pub struct AppSettings { ... }` from `settings.rs`
    the same way the other scripts in this directory bracket-match a Rust
    enum body or a TypeScript array literal -- not a real parser, just
    enough to find the one block that matters and stop at its matching
    close brace, so a field-shaped line in some unrelated struct later in
    the file can never be picked up by accident."""
    if not SETTINGS_RS.exists():
        print(f"WARNING: {SETTINGS_RS} does not exist", file=sys.stderr)
        return []
    text = SETTINGS_RS.read_text(encoding="utf-8", errors="ignore")
    m = re.search(r"pub\s+struct\s+AppSettings\s*\{", text)
    if not m:
        print("WARNING: `pub struct AppSettings {` not found in settings.rs", file=sys.stderr)
        return []
    start = m.end()
    depth = 1
    i = start
    while depth > 0 and i < len(text):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        i += 1
    block = text[start:i]
    # dict.fromkeys() dedupes while preserving first-seen order, in case a
    # future field is ever re-declared (it shouldn't be, but a set would
    # silently make the output order nondeterministic between runs).
    return list(dict.fromkeys(FIELD_RE.findall(block)))


# ---------------------------------------------------------------------------
# (B) Whether the app lets someone change each field.
# ---------------------------------------------------------------------------

USE_SETTINGS_FIELD_RE = re.compile(r"""useSettingsField\(\s*['"]([a-zA-Z_][a-zA-Z0-9_]*)['"]""")
UPDATE_SETTINGS_CALL_RE = re.compile(r"updateSettings\s*\(")
# An object-literal key immediately followed by a colon -- `name:` inside
# `updateSettings({ name: value, other: value2 })`. Requires the colon not
# be doubled (`::`), which would be a TypeScript type annotation rather
# than an object key, though that shape doesn't arise inside a call's
# argument list in practice. Deliberately not restricted to depth-1 keys:
# a settings field is occasionally nested inside a small update object
# (`updateSettings({ duplicate_detection: { scope: 'x' } })`), and scanning
# every depth costs nothing extra here -- a key that happens to share a
# name with an AppSettings field but isn't really one just becomes a
# no-op entry in the "changeable" set, since it will never also appear in
# the field list from part (A).
IDENT_COLON_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*:(?!:)")


def _matching_paren(text: str, open_idx: int) -> int:
    """Index of the `)` that matches the `(` at `open_idx`, or -1."""
    depth = 0
    i = open_idx
    while i < len(text):
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def collect_frontend_changeable() -> dict[str, list[str]]:
    """Every AppSettings field name the app lets a user change, mapped to
    the `path:line` site(s) where that happens. Two shapes count, per the
    module docstring: `useSettingsField('name')`, and any `name:` key
    found inside a bracket-matched `updateSettings(...)` call."""
    changeable: dict[str, list[str]] = {}

    def record(name: str, path: Path, idx: int, text: str) -> None:
        rel = path.relative_to(REPO_ROOT).as_posix()
        changeable.setdefault(name, []).append(f"{rel}:{_line_at(text, idx)}")

    for path in _iter_source(WEB_SRC, (".ts", ".tsx")):
        text = path.read_text(encoding="utf-8", errors="ignore")
        for m in USE_SETTINGS_FIELD_RE.finditer(text):
            record(m.group(1), path, m.start(), text)
        for m in UPDATE_SETTINGS_CALL_RE.finditer(text):
            open_idx = m.end() - 1
            close_idx = _matching_paren(text, open_idx)
            if close_idx == -1:
                continue
            body = text[open_idx + 1 : close_idx]
            for km in IDENT_COLON_RE.finditer(body):
                record(km.group(1), path, open_idx + 1 + km.start(), text)

    return changeable


# ---------------------------------------------------------------------------
# (C) Whether the backend ever reads each field.
# ---------------------------------------------------------------------------


CFG_TEST_RE = re.compile(r"#\[cfg\(test\)\]")


def _matching_brace(text: str, open_idx: int) -> int:
    """Index of the `}` that matches the `{` at `open_idx`, or `len(text)`
    if the braces never balance (shouldn't happen in code that compiles,
    but a truncated read or a stray brace inside a string literal
    shouldn't crash the script -- it should just stop stripping there)."""
    depth = 0
    i = open_idx
    while i < len(text):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return len(text)


def _strip_cfg_test_blocks(text: str) -> str:
    """Remove every `#[cfg(test)]`-attributed item -- almost always a whole
    `mod tests { ... }` block -- from `text` before it's scanned for field
    reads.

    Without this, a field only used inside a test's own assertion (for
    example `assert_eq!(settings.video_codec_fallback_chain, ...)`) reads
    as a genuine backend use, when all it proves is that the test file
    compiles. That happened for real: `video_codec_fallback_chain` looked
    "read by the backend" purely because of a line inside
    `config_service.rs`'s `#[cfg(test)] mod tests { ... }`, while its
    actual only production reader (`AppSettings::video_codec_priority_cli`)
    sat in a file this script otherwise skips entirely (see
    `_collect_impl_app_settings_self_reads` below). Both problems had to be
    fixed together, or removing the test line alone would have made the
    check wrongly report a live field as dead.

    This finds each `#[cfg(test)]` attribute, brace-matches forward from
    the next `{` (same trick `collect_app_settings_fields` uses for the
    struct body) to find where that item ends, and drops everything from
    the attribute through the closing brace. It does not care whether the
    attributed item is a `mod`, a single `fn`, or anything else -- none of
    it runs in a real build, so none of it should count as a real read.
    """
    out: list[str] = []
    pos = 0
    for m in CFG_TEST_RE.finditer(text):
        if m.start() < pos:
            continue  # already inside a block we've dropped
        out.append(text[pos : m.start()])
        brace = text.find("{", m.end())
        if brace == -1:
            # No block follows this attribute -- nothing sensible to strip;
            # keep the rest of the file as-is rather than guessing.
            pos = m.start()
            break
        end = _matching_brace(text, brace)
        pos = end + 1
    out.append(text[pos:])
    return "".join(out)


def _collect_impl_app_settings_self_reads(field_names: list[str]) -> set[str]:
    """Every AppSettings field referenced as `self.<field>` inside
    `impl AppSettings { ... }` in settings.rs.

    `collect_backend_reads` skips `models/settings.rs` entirely, and for
    good reason: the struct DEFINITION in that file is nothing but
    `pub name: Type,` lines, and every one of those would look like a
    "read" to a naive `.name` scan even though it's only a declaration.

    But that blanket skip throws out the one part of the file that IS a
    genuine consumer: the inherent `impl AppSettings { ... }` block, where
    methods such as `video_codec_priority_cli()` read fields off `self` to
    build the values the backend actually sends onward (in that case, the
    codec list GAMDL receives on the command line). A field whose only
    real reader is a method like that was being reported as dead --
    wrongly -- until this function let that one block back in, scanned on
    its own rather than as part of the whole file.
    """
    if not SETTINGS_RS.exists():
        return set()
    text = SETTINGS_RS.read_text(encoding="utf-8", errors="ignore")
    m = re.search(r"^impl\s+AppSettings\s*\{", text, re.MULTILINE)
    if not m:
        return set()
    start = text.find("{", m.end() - 1)
    end = _matching_brace(text, start)
    block = text[start + 1 : end]
    return {name for name in field_names if re.search(r"\bself\." + re.escape(name) + r"\b", block)}


def collect_backend_reads(field_names: list[str]) -> set[str]:
    """Every AppSettings field name found as a `.name` token somewhere in
    `src-tauri/src/**/*.rs`, outside `models/settings.rs`'s field
    declarations and outside test code -- plus, as a deliberate carve-out
    of that `models/settings.rs` exclusion, any field read as `self.name`
    inside `impl AppSettings { ... }` in that same file. See
    `_strip_cfg_test_blocks` and `_collect_impl_app_settings_self_reads`
    for why each half exists."""
    reads: set[str] = set()
    patterns = {name: re.compile(r"\." + re.escape(name) + r"\b") for name in field_names}
    for path in _iter_source(RUST_SRC, (".rs",)):
        if path.resolve() == SETTINGS_RS.resolve():
            continue
        text = _strip_cfg_test_blocks(path.read_text(encoding="utf-8", errors="ignore"))
        for name, pattern in patterns.items():
            if name in reads:
                continue
            if pattern.search(text):
                reads.add(name)
    reads |= _collect_impl_app_settings_self_reads(field_names)
    return reads


def check() -> int:
    fields = collect_app_settings_fields()
    if not fields:
        print("WARNING: no AppSettings fields found -- something upstream of this check is broken", file=sys.stderr)
        return 1 if "--strict" in sys.argv else 0

    changeable = collect_frontend_changeable()
    backend_reads = collect_backend_reads(fields)

    candidates = [f for f in fields if f in changeable]
    dead = [f for f in candidates if f not in backend_reads and f not in FRONTEND_ONLY]

    print(f"AppSettings fields                 : {len(fields)}")
    print(f"Changeable in the app               : {len(candidates)}")
    print(f"Read by the backend                 : {sum(1 for f in fields if f in backend_reads)}")
    print(f"Recorded as frontend-only, on purpose: {len(FRONTEND_ONLY)}")
    print()

    findings = 0

    if dead:
        print("### A setting the app lets you change, that the backend never reads\n")
        for name in dead:
            sites = changeable[name]
            print(
                f"  • {sites[0]} — '{name}' can be changed in the app but no `.{name}` "
                f"token appears anywhere in src-tauri/src (outside models/settings.rs). "
                f"Wire it into the backend behaviour it's supposed to control, or add it "
                f"to FRONTEND_ONLY in this script with the reason it's genuinely acted on "
                f"by the frontend alone."
            )
            for extra in sites[1:]:
                print(f"      also changeable at {extra}")
        print()
        findings += len(dead)

    # A stale FRONTEND_ONLY entry: recorded as deliberately frontend-only,
    # but it isn't even a field on AppSettings any more (renamed, removed).
    stale = sorted(name for name in FRONTEND_ONLY if name not in fields)
    if stale:
        print("### FRONTEND_ONLY lists a name that is not an AppSettings field\n")
        for name in stale:
            print(
                f"  • tools/audit-checks/check_settings_reach_backend.py — '{name}' is in "
                f"FRONTEND_ONLY but there is no such field in AppSettings any more. Remove "
                f"the entry."
            )
        print()
        findings += len(stale)

    if findings == 0:
        print(
            f"OK — {len(candidates)} changeable setting(s) are all either read by the "
            f"backend or recorded in FRONTEND_ONLY with a reason."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
