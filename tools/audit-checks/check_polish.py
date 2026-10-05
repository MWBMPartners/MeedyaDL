#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/check_polish.py
==================================

The fast, automatic half of the maintainer's rule "Nothing may look
unfinished, careless or AI-made": the things a script can see without a
browser or the network. It runs before every push (tools/hooks/pre-push,
with --strict) and on every pull request (pr-security.yml, check 8).

WHAT IT CHECKS -- one `### ` section per rule, findings as bullets
--------------------------------------------------------------------
 1. Placeholder addresses and text a visitor can see: example.com,
    hosting preview domains, localhost, lorem ipsum, TODO/FIXME, "coming
    soon", in help pages, the README, the translation files, interface
    strings and tauri.conf.json.
 2. Internal references on screen: issue numbers ("#537") and old version
    notes ("(v0.52+)", "pre-v1.6", "Requires GAMDL 2.9.1+") in the in-app
    help pages, translations and interface strings.
 3. One name and one version: package.json, src-tauri/Cargo.toml and
    tauri.conf.json agree exactly; the release-please manifest is never
    ahead of them, and matches them on a finished release.
 4. Each screen names the native window (a `setTitle` call, and the
    permission that allows it).
 5. Icons at every size the installers need.
 6. An unknown page falls back to a real screen (the router has a
    `default:`).
 7. The crash screen is a way forward: no developer wording, a Reload
    button.
 8. Headings and button names: exactly one <h1> per screen (the shared
    page header), no heading inside a button, and every icon-only button
    has a name.
 9. Debug output in shipped code: console.log / console.debug /
    `debugger` in the interface, dbg!/println! in the backend.
10. Drafts, backups, Word files and test files in public/, help/ or the
    installer's bundled resources.
11. The bundle-size warning not silenced: `chunkSizeWarningLimit` not
    raised above Vite's default (500 kB).
12. Nothing fake: no `href="#"`, no empty click handlers.
13. Every "Settings > Tab > Section" mentioned in help, README,
    translations or the interface names a real tab and a real section or
    control on it.
14. Claims of features that have no code behind them (a small, hand-kept
    map: BPM tagging, direct-to-cloud upload, Spotify for everyone).
15. No message names the hidden developer unlock.
16. Ratchets -- counts that may only go down: hand-made <button>,
    <select> and <input> outside components/common, hard-coded pixel font
    sizes, corner rounding that bypasses the theme token, and error toasts
    shaped "Failed to X: <raw error>". A count above its ceiling is a
    finding; so is a count BELOW it, because the ceiling must then be
    lowered in the same change, or it would let the count creep back up.

WHAT IT CANNOT CHECK -- the release audit does these in a real browser
--------------------------------------------------------------------
How screens look at different sizes (overflow, clipping, overlap),
consistency by eye, loading and empty states, whether an error message
says what to do next, whether links on the internet still work, and
whether a feature actually works when used. A clean run here means the
mechanical rules hold, not that the app looks finished.

HOW IT READS THE CODE
---------------------
By pattern, not by parsing: comments are blanked out of TypeScript and
Rust first, Markdown is read outside code blocks and inline code, and
interface text means string literals and JSX text. That is deliberately
simple, so expect the occasional false alarm; each rule says what it
skips and why. A rule that cannot find the file it needs reports that as
a finding of its own -- "could not check" must never look like "clean".

THIS FILE QUOTES THE PATTERNS IT HUNTS FOR
-----------------------------------------
So do its test (test_check_polish.py), the folder README and the pre-push
hook. None of them is in any scanned location today, and EXEMPT_FILES
below names them anyway, so a later widening of a scan cannot make the
check report itself. Their README says the same.

Exit code:
  0 -- no findings, or findings without --strict
  1 -- --strict and at least one finding

Usage:
  python3 tools/audit-checks/check_polish.py [--strict] [--root PATH]

`--root` points the check at another copy of the repository; the
self-test uses it to plant one fault at a time in a throw-away copy.
"""

from __future__ import annotations

import bisect
import functools
import json
import re
import struct
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Where to look
# ---------------------------------------------------------------------------

ROOT = Path(__file__).resolve().parents[2]
if "--root" in sys.argv:
    ROOT = Path(sys.argv[sys.argv.index("--root") + 1]).resolve()

# Files that quote the patterns this check hunts for, on purpose. They are
# not in any scanned location today; naming them here makes sure they never
# become findings if a scan is widened later (see the docstring).
EXEMPT_FILES = {
    "tools/audit-checks/check_polish.py",
    "tools/audit-checks/test_check_polish.py",
    "tools/audit-checks/README.md",
    "tools/hooks/pre-push",
}

FINDINGS: dict[str, list[tuple[str, int, str]]] = {}
SECTION_ORDER: list[str] = []


def add(section: str, path: Path | str, line: int, message: str) -> None:
    """Record one finding under `section`."""
    rel = str(path.relative_to(ROOT)) if isinstance(path, Path) else path
    if section not in FINDINGS:
        FINDINGS[section] = []
        SECTION_ORDER.append(section)
    FINDINGS[section].append((rel, line, message))


def rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


# Every rule reads the same files, so reads, file lists and the cleaned-up
# text below are each worked out once per run (this keeps the check fast
# enough to run before every push).
@functools.lru_cache(maxsize=None)
def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


@functools.lru_cache(maxsize=None)
def _files(pattern: str) -> tuple[Path, ...]:
    return tuple(sorted(p for p in ROOT.glob(pattern) if p.is_file() and rel(p) not in EXEMPT_FILES))


def files(pattern: str) -> list[Path]:
    """Repository files matching a glob, minus exempt files."""
    return list(_files(pattern))


class Lines:
    """Line numbers for positions in one piece of text, looked up quickly."""

    def __init__(self, text: str) -> None:
        self.starts = [i for i, c in enumerate(text) if c == "\n"]

    def at(self, pos: int) -> int:
        return bisect.bisect_left(self.starts, pos) + 1


def is_test_file(path: Path) -> bool:
    name = path.name
    return (
        ".test." in name
        or ".spec." in name
        or "/testing/" in str(path)
        or "/test/" in str(path)
        or name in ("tests.rs", "integration_tests.rs")
        or name.startswith("test_")
    )


def interface_sources() -> list[Path]:
    """Shipped interface code: src/**/*.ts(x), without tests."""
    return [p for p in files("src/**/*.ts") + files("src/**/*.tsx") if not is_test_file(p)]


def backend_sources() -> list[Path]:
    """Shipped backend code: src-tauri/src/**/*.rs, without test files."""
    return [p for p in files("src-tauri/src/**/*.rs") if not is_test_file(p)]


# ---------------------------------------------------------------------------
# Reading text out of code and Markdown
# ---------------------------------------------------------------------------

def blank_comments(code: str) -> str:
    """TypeScript/Rust text with comments replaced by spaces.

    Line numbers stay the same (newlines inside block comments are kept).
    `//` right after a `:` is not a comment (it is "https://"), nor is one
    inside a quoted string on the same line -- a rough rule, but URLs are
    the only common case.
    """

    def keep_newlines(m: re.Match[str]) -> str:
        return re.sub(r"[^\n]", " ", m.group(0))

    code = re.sub(r"/\*.*?\*/", keep_newlines, code, flags=re.S)
    out = []
    for line in code.split("\n"):
        m = re.search(r"(?<![:\"'`\\])//", line)
        if m and line[: m.start()].count('"') % 2 == 0 and line[: m.start()].count("'") % 2 == 0:
            line = line[: m.start()]
        out.append(line)
    return "\n".join(out)


def before_rust_tests(code: str) -> str:
    """Rust text up to the first `#[cfg(test)]`: test modules sit at the end by convention."""
    i = code.find("#[cfg(test)]")
    return code if i < 0 else code[:i]


STRING_RE = re.compile(r"'(?:[^'\\\n]|\\.)*'|\"(?:[^\"\\\n]|\\.)*\"|`(?:[^`\\]|\\.)*`", re.S)


def string_literals(code: str) -> list[tuple[int, str]]:
    """(line number, contents) of each quoted string in comment-free code."""
    lines = Lines(code)
    return [(lines.at(m.start()), m.group(0)[1:-1]) for m in STRING_RE.finditer(code)]


def jsx_text(code: str) -> list[tuple[int, str]]:
    """(line number, text) of lines that look like JSX text between tags.

    Rough: a line with letters that has no code punctuation (=, ;, {, (,
    =>) and does not start with a tag or a brace.
    """
    out = []
    for n, line in enumerate(code.split("\n"), 1):
        s = line.strip()
        if not s or s[0] in "<{})/*:.?&|[]'\"`" or s.startswith("import "):
            continue
        if re.search(r"[=;{}()]|=>", s):
            continue
        if re.search(r"[A-Za-z]{2}", s):
            out.append((n, s))
    return out


@functools.lru_cache(maxsize=None)
def clean(path: Path) -> str:
    """A TypeScript file with its comments blanked (see blank_comments)."""
    return blank_comments(read(path))


@functools.lru_cache(maxsize=None)
def clean_rust(path: Path) -> str:
    """A Rust file without its test modules, comments blanked."""
    return blank_comments(before_rust_tests(read(path)))


@functools.lru_cache(maxsize=None)
def interface_text(path: Path) -> tuple[tuple[int, str], ...]:
    """Text a person could see, from one interface source file."""
    code = clean(path)
    items = string_literals(code)
    if path.suffix == ".tsx":
        items += jsx_text(code)
    return tuple(items)


def markdown_prose(path: Path) -> list[tuple[int, str]]:
    """(line number, text) of Markdown outside code blocks, inline code and HTML comments."""
    out = []
    in_fence = in_comment = False
    for n, line in enumerate(read(path).split("\n"), 1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if "<!--" in line and "-->" not in line:
            in_comment = True
            continue
        if in_comment:
            if "-->" in line:
                in_comment = False
            continue
        line = re.sub(r"<!--.*?-->", "", line)
        out.append((n, re.sub(r"`[^`]*`", "", line)))
    return out


def locale_values() -> list[tuple[Path, int, str]]:
    """(file, line, value) for every string value in the translation files."""
    out = []
    for p in files("public/locales/*/translation.json"):
        for n, line in enumerate(read(p).split("\n"), 1):
            m = re.match(r'\s*"[^"]+"\s*:\s*"(.*)",?\s*$', line)
            if m:
                out.append((p, n, m.group(1)))
    return out


def in_app_help() -> list[Path]:
    """Help pages the app shows (index.md is the GitHub table of contents only)."""
    return [p for p in files("help/**/*.md") if p.name != "index.md"]


# ---------------------------------------------------------------------------
# 1. Placeholder addresses and text
# ---------------------------------------------------------------------------

PLACEHOLDER_ADDRESS = re.compile(
    r"\bexample\.(?:com|org|net)\b|[\w-]+\.(?:vercel\.app|netlify\.app|pages\.dev|herokuapp\.com)\b|\blorem ipsum\b",
    re.I,
)
LOCALHOST = re.compile(r"\blocalhost\b", re.I)
UNFINISHED_WORDS = re.compile(r"\b(?:TODO|FIXME)\b|\bcoming soon\b", re.I)

# tauri.conf.json keys whose values are never shown to anyone: the dev
# server address and the content-security policy (which names Tauri's own
# internal `ipc.localhost` origin).
TAURI_CONF_NOT_SHOWN = ("devUrl", "csp", "$schema")

SECTION_PLACEHOLDER = "Placeholder address or unfinished wording where people can see it"


def check_placeholders() -> None:
    # Help pages and the README: prose only. Code blocks and inline code are
    # left alone -- they hold commands to adapt (an ssh tunnel to localhost,
    # "your.apple.id@example.com" in a curl template), which are labelled
    # examples by their nature.
    for p in files("help/**/*.md") + files("README.md"):
        for n, text in markdown_prose(p):
            for pat in (PLACEHOLDER_ADDRESS, LOCALHOST):
                m = pat.search(text)
                if m:
                    add(SECTION_PLACEHOLDER, p, n, f'"{m.group(0)}" in text people read')
            m = UNFINISHED_WORDS.search(text)
            # A "## Coming Soon" heading over a list of planned services is
            # a true statement, not an unfinished page.
            if m and not text.lstrip().startswith("#"):
                add(SECTION_PLACEHOLDER, p, n, f'"{m.group(0)}" in text people read')
    for p, n, value in locale_values():
        for pat in (PLACEHOLDER_ADDRESS, LOCALHOST, UNFINISHED_WORDS):
            m = pat.search(value)
            if m:
                add(SECTION_PLACEHOLDER, p, n, f'"{m.group(0)}" in a translation')
    # Interface strings: addresses and unfinished words. A bare `localhost`
    # token is not flagged here -- code compares hosts against it (the
    # wrapper address check) without ever showing it.
    for p in interface_sources():
        for n, text in interface_text(p):
            m = PLACEHOLDER_ADDRESS.search(text) or UNFINISHED_WORDS.search(text)
            if m:
                add(SECTION_PLACEHOLDER, p, n, f'"{m.group(0)}" in interface text')
            elif " " in text and LOCALHOST.search(text):
                add(SECTION_PLACEHOLDER, p, n, '"localhost" in interface text')
    conf = ROOT / "src-tauri/tauri.conf.json"
    if conf.exists():
        for n, line in enumerate(read(conf).split("\n"), 1):
            key = re.match(r'\s*"([^"]+)"\s*:', line)
            if key and key.group(1) in TAURI_CONF_NOT_SHOWN:
                continue
            m = PLACEHOLDER_ADDRESS.search(line) or LOCALHOST.search(line)
            if m:
                add(SECTION_PLACEHOLDER, conf, n, f'"{m.group(0)}" in the app configuration')
    # Backend: placeholder addresses in string literals (messages it shows).
    for p in backend_sources():
        code = clean_rust(p)
        for n, text in string_literals(code):
            m = PLACEHOLDER_ADDRESS.search(text)
            if m:
                add(SECTION_PLACEHOLDER, p, n, f'"{m.group(0)}" in a backend string')


# ---------------------------------------------------------------------------
# 2. Internal references on screen
# ---------------------------------------------------------------------------

ISSUE_NUMBER = re.compile(r"(?<![\w&#/-])#\d{3,4}(?![\w])")
HEX_COLOUR_ONLY = re.compile(r"^#[0-9a-fA-F]{3,8}$")
OLD_VERSION_NOTE = re.compile(
    r"\(v0\.\d|\bv0\.\d+(?:\.\d+)?\+|\bpre-v\d|\bpost-#\d|\bbefore v\d+\.\d+|"
    r"Requires GAMDL v?[0-2]\.\d|GAMDL v?2\.\d+(?:\.\d+)?\+",
    re.I,
)
SECTION_INTERNAL = "Internal reference on screen (issue number or old version note)"


def check_internal_references() -> None:
    def scan(path: Path, n: int, text: str, where: str) -> None:
        if HEX_COLOUR_ONLY.match(text.strip()):
            return
        m = ISSUE_NUMBER.search(text) or OLD_VERSION_NOTE.search(text)
        if m:
            add(SECTION_INTERNAL, path, n, f'"{m.group(0)}" {where}')

    for p in in_app_help():
        for n, text in markdown_prose(p):
            # A link's address may carry an issue number (GAMDL's tracker);
            # only the words people read count.
            scan(p, n, re.sub(r"\]\([^)]*\)", "]", text), "in an in-app help page")
    for p, n, value in locale_values():
        scan(p, n, value, "in a translation")
    for p in interface_sources():
        for n, text in interface_text(p):
            scan(p, n, text, "in interface text")


# ---------------------------------------------------------------------------
# 3. One name and one version
# ---------------------------------------------------------------------------

SECTION_IDENTITY = "Name or version disagrees between the build files"
PRODUCT_NAME = "MeedyaDL"
PACKAGE_NAME = "meedyadl"


def base_version(v: str) -> tuple[int, ...]:
    m = re.match(r"(\d+)\.(\d+)\.(\d+)", v)
    return tuple(int(x) for x in m.groups()) if m else ()


def check_identity() -> None:
    try:
        pkg = json.loads(read(ROOT / "package.json"))
        conf = json.loads(read(ROOT / "src-tauri/tauri.conf.json"))
        cargo = read(ROOT / "src-tauri/Cargo.toml")
        manifest = json.loads(read(ROOT / ".release-please-manifest.json"))
    except (OSError, ValueError) as e:
        add(SECTION_IDENTITY, "(build files)", 0, f"could not read a build file, so names and versions were not compared: {e}")
        return
    # The [package] table is the line that is exactly "[package]" -- the
    # file also mentions it inside comments.
    parts = re.split(r"^\[package\]\s*$", cargo, maxsplit=1, flags=re.M)
    pkg_block = re.split(r"^\[", parts[-1], maxsplit=1, flags=re.M)[0] if len(parts) == 2 else ""

    cargo_name = re.search(r'^name\s*=\s*"([^"]+)"', pkg_block, re.M)
    cargo_ver = re.search(r'^version\s*=\s*"([^"]+)"', pkg_block, re.M)
    names = {
        "package.json name": (pkg.get("name"), PACKAGE_NAME),
        "Cargo.toml name": (cargo_name.group(1) if cargo_name else None, PACKAGE_NAME),
        "tauri.conf.json productName": (conf.get("productName"), PRODUCT_NAME),
    }
    for where, (got, want) in names.items():
        if got != want:
            add(SECTION_IDENTITY, where, 0, f'is "{got}", expected "{want}"')
    versions = {
        "package.json": pkg.get("version"),
        "src-tauri/Cargo.toml": cargo_ver.group(1) if cargo_ver else None,
        "src-tauri/tauri.conf.json": conf.get("version"),
    }
    if len(set(versions.values())) != 1:
        listed = ", ".join(f"{k} {v}" for k, v in versions.items())
        add(SECTION_IDENTITY, "package.json", 0, f"the three build files disagree on the version: {listed}")
        return
    version = versions["package.json"] or ""
    released = manifest.get(".", "")
    # The release-please manifest records the last finished release made
    # from main; on alpha/beta/rc the channel workflows bump the build files
    # with a pre-release suffix and leave the manifest alone. So it may be
    # BEHIND a pre-release, never ahead of it, and equal on a finished one.
    if base_version(released) > base_version(version):
        add(SECTION_IDENTITY, ".release-please-manifest.json", 0, f"says {released}, ahead of the build's {version}")
    elif "-" not in version and released != version:
        add(SECTION_IDENTITY, ".release-please-manifest.json", 0, f"says {released} but this finished release is {version}")


# ---------------------------------------------------------------------------
# 4. Native window title, 5. icons, 6. router fallback, 7. crash screen
# ---------------------------------------------------------------------------

SECTION_TITLE = "Screens do not name the native window"
SECTION_ICONS = "Icon missing or the wrong size"
SECTION_ROUTER = "An unknown page has nowhere to go"
SECTION_CRASH = "The crash screen is not a way forward"


def check_window_title() -> None:
    calls = [p for p in interface_sources() if re.search(r"\.setTitle\(", blank_comments(read(p)))]
    if not calls:
        add(SECTION_TITLE, "src", 0, "nothing calls setTitle(), so the window title never follows the screen")
    cap = ROOT / "src-tauri/capabilities/default.json"
    if not cap.exists() or "core:window:allow-set-title" not in read(cap):
        add(SECTION_TITLE, "src-tauri/capabilities/default.json", 0, "the core:window:allow-set-title permission is missing")


def png_size(path: Path) -> tuple[int, int] | None:
    data = path.read_bytes()[:24]
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    return struct.unpack(">II", data[16:24])


def check_icons() -> None:
    conf_path = ROOT / "src-tauri/tauri.conf.json"
    try:
        icons = json.loads(read(conf_path))["bundle"]["icon"]
    except (OSError, ValueError, KeyError) as e:
        add(SECTION_ICONS, "src-tauri/tauri.conf.json", 0, f"could not read bundle.icon, so icons were not checked: {e}")
        return
    base = ROOT / "src-tauri"
    for name in icons:
        p = base / name
        if not p.exists():
            add(SECTION_ICONS, f"src-tauri/{name}", 0, "listed in bundle.icon but missing")
            continue
        m = re.match(r"(\d+)x(\d+)(@2x)?\.png$", p.name)
        if m:
            want = int(m.group(1)) * (2 if m.group(3) else 1)
            got = png_size(p)
            if got != (want, want):
                add(SECTION_ICONS, p, 0, f"is {got}, its name says {want}x{want}")
        elif p.suffix == ".ico":
            data = p.read_bytes()
            count = struct.unpack("<H", data[4:6])[0] if len(data) >= 6 else 0
            sizes = {(data[6 + 16 * i] or 256) for i in range(count)} if len(data) >= 6 + 16 * count else set()
            missing = [s for s in (16, 32, 48, 256) if s not in sizes]
            if missing:
                add(SECTION_ICONS, p, 0, f"has no {', '.join(f'{s}px' for s in missing)} image")
        elif p.suffix == ".icns":
            if p.read_bytes()[:4] != b"icns" or p.stat().st_size < 10_000:
                add(SECTION_ICONS, p, 0, "is not a usable .icns file")
    for tray in ("tray-icon.png", "tray-icon@2x.png"):
        p = base / "icons/tray" / tray
        if not p.exists():
            add(SECTION_ICONS, f"src-tauri/icons/tray/{tray}", 0, "tray icon missing")


def check_router() -> None:
    app = ROOT / "src/App.tsx"
    if not app.exists():
        add(SECTION_ROUTER, "src/App.tsx", 0, "missing, so the page router was not checked")
        return
    code = clean(app)
    m = re.search(r"switch\s*\(\s*currentPage\s*\)\s*{(.*?)\n\s{4}}", code, re.S)
    if not m:
        add(SECTION_ROUTER, app, 0, "could not find the `switch (currentPage)` page router")
    elif "default:" not in m.group(1):
        add(SECTION_ROUTER, app, Lines(code).at(m.start()), "the page router has no `default:` for an unknown page")


def check_crash_screen() -> None:
    screen = ROOT / "src/components/layout/CrashScreen.tsx"
    main = ROOT / "src/main.tsx"
    if not screen.exists():
        add(SECTION_CRASH, "src/components/layout/CrashScreen.tsx", 0, "missing")
        return
    text = read(screen)
    if not re.search(r">\s*Reload\b|Reload MeedyaDL", text):
        add(SECTION_CRASH, screen, 0, "has no Reload button")
    for word in ("React Error", "Component Stack:", "Error Caught"):
        if word in blank_comments(text):
            add(SECTION_CRASH, screen, 0, f'shows developer wording ("{word}")')
    if main.exists() and "<CrashScreen" not in read(main):
        add(SECTION_CRASH, main, 0, "the error boundary does not show CrashScreen")


# ---------------------------------------------------------------------------
# 8. Headings and button names
# ---------------------------------------------------------------------------

SECTION_HEADINGS = "Heading structure: one <h1> per screen, no heading inside a button"
SECTION_BUTTON_NAMES = "Icon-only button with no name for screen readers"
BUTTON_OPEN = re.compile(r"<(button|Button)\b((?:[^>{]|\{(?:[^{}]|\{[^{}]*\})*\})*?)(/?)>", re.S)


def page_components() -> list[tuple[str, Path | None]]:
    """(component name, file) for each screen the page router can show."""
    app = ROOT / "src/App.tsx"
    if not app.exists():
        return []
    code = clean(app)
    m = re.search(r"switch\s*\(\s*currentPage\s*\)\s*{(.*?)\n\s{4}}", code, re.S)
    names = sorted(set(re.findall(r"return\s*<(\w+)", m.group(1)))) if m else []
    out = []
    for name in names:
        found = [p for p in files(f"src/**/{name}.tsx") if not is_test_file(p)]
        out.append((name, found[0] if found else None))
    return out


def check_headings() -> None:
    pages = page_components()
    if not pages:
        add(SECTION_HEADINGS, "src/App.tsx", 0, "could not find the page components, so headings were not checked")
    for name, path in pages:
        if path is None:
            add(SECTION_HEADINGS, "src", 0, f"could not find {name}.tsx")
            continue
        code = blank_comments(read(path))
        count = len(re.findall(r"<PageHeader\b", code)) + len(re.findall(r"<h1\b", code))
        if count != 1:
            add(SECTION_HEADINGS, path, 0, f"{name} has {count} top-level headings (<PageHeader> or <h1>); a screen needs exactly one")
    header = ROOT / "src/components/layout/PageHeader.tsx"
    if header.exists() and not re.search(r"<h1\b", blank_comments(read(header))):
        add(SECTION_HEADINGS, header, 0, "the shared page header's title is not an <h1>")
    for p in files("src/**/*.tsx"):
        if is_test_file(p):
            continue
        code = clean(p)
        lines = Lines(code)
        for m in BUTTON_OPEN.finditer(code):
            if m.group(3):
                continue
            end = code.find(f"</{m.group(1)}>", m.end())
            body = code[m.end() : end if end >= 0 else m.end()]
            if re.search(r"<h[1-6]\b", body):
                add(SECTION_HEADINGS, p, lines.at(m.start()), "a heading inside a button is flattened into the button's name")


def check_button_names() -> None:
    # Folded in from the 2026-10 audit's check_icon_buttons.py.
    for p in files("src/**/*.tsx"):
        if is_test_file(p):
            continue
        code = clean(p)
        lines = Lines(code)
        for m in BUTTON_OPEN.finditer(code):
            tag, attrs, self_closing = m.groups()
            if re.search(r"aria-label(?:ledby)?=|title=", attrs):
                continue
            if self_closing:
                body = ""
            else:
                end = code.find(f"</{tag}>", m.end())
                body = code[m.end() : end if end >= 0 else m.end()]
            text = re.sub(r"<[A-Z][A-Za-z0-9.]*\b[^>]*/>", "", body)
            if re.sub(r"\s", "", text) == "":
                add(SECTION_BUTTON_NAMES, p, lines.at(m.start()), f"<{tag}> has no visible text and no aria-label or title")


# ---------------------------------------------------------------------------
# 9. Debug output, 10. drafts in shipped folders, 11. bundle-size warning,
#     12. nothing fake
# ---------------------------------------------------------------------------

SECTION_DEBUG = "Debug output left in shipped code"
SECTION_DRAFTS = "Draft, backup or test file in a folder that ships"
SECTION_CHUNK = "The bundle-size warning is silenced"
SECTION_FAKE = "Link or button that does nothing"
DRAFT_NAME = re.compile(
    r"\.(?:docx?|odt|pages|bak|orig|rej|tmp|swp)$|~$|^\.DS_Store$|draft|\.test\.|\.spec\.|^Thumbs\.db$",
    re.I,
)
VITE_DEFAULT_CHUNK_LIMIT = 500


def check_debug_output() -> None:
    for p in interface_sources():
        code = clean(p)
        lines = Lines(code)
        for m in re.finditer(r"\bconsole\.(?:log|debug)\s*\(|^\s*debugger\s*;?\s*$", code, re.M):
            add(SECTION_DEBUG, p, lines.at(m.start()), m.group(0).strip())
    for p in backend_sources():
        code = clean_rust(p)
        lines = Lines(code)
        for m in re.finditer(r"(?<![\w])(?:dbg|println)!\s*\(", code):
            add(SECTION_DEBUG, p, lines.at(m.start()), m.group(0).strip())


def bundled_resource_dirs() -> list[Path]:
    try:
        resources = json.loads(read(ROOT / "src-tauri/tauri.conf.json"))["bundle"].get("resources", [])
    except (OSError, ValueError, KeyError):
        return []
    out = []
    for r in resources if isinstance(resources, list) else list(resources):
        d = (ROOT / "src-tauri" / r.split("*")[0]).resolve()
        if d.exists():
            out.append(d if d.is_dir() else d.parent)
    return out


def check_drafts() -> None:
    dirs = {ROOT / "public", ROOT / "help", *bundled_resource_dirs()}
    for d in sorted(dirs):
        if not d.is_dir():
            continue
        for p in sorted(d.rglob("*")):
            if p.is_file() and DRAFT_NAME.search(p.name):
                add(SECTION_DRAFTS, p, 0, "ships to everyone who installs MeedyaDL; move it out (docs/drafts/ for drafts)")


def check_chunk_limit() -> None:
    vite = ROOT / "vite.config.ts"
    if not vite.exists():
        add(SECTION_CHUNK, "vite.config.ts", 0, "missing, so the bundle-size setting was not checked")
        return
    code = blank_comments(read(vite))
    m = re.search(r"chunkSizeWarningLimit\s*:\s*([\d_]+)", code)
    if m and int(m.group(1).replace("_", "")) > VITE_DEFAULT_CHUNK_LIMIT:
        add(
            SECTION_CHUNK,
            vite,
            Lines(code).at(m.start()),
            f"chunkSizeWarningLimit is {m.group(1)} kB, above Vite's {VITE_DEFAULT_CHUNK_LIMIT}; make the bundle smaller instead",
        )


# Empty handlers kept on purpose, each with its reason. Keep this short.
#  - The "Anonymous Usage Analytics" switch in Settings > Advanced >
#    Error Reporting is disabled and labelled "Not available yet"; it was
#    deliberately left visible (see the comment above it in AdvancedTab.tsx).
#    Whether a switch for an unbuilt feature should be shown at all is a
#    question raised with the maintainer (#1295), not settled here.
EMPTY_HANDLER_ALLOWED = {("src/components/settings/tabs/AdvancedTab.tsx", "Anonymous Usage Analytics")}


def check_fake() -> None:
    for p in files("src/**/*.tsx"):
        if is_test_file(p):
            continue
        code = clean(p)
        lines = Lines(code)
        for m in re.finditer(r"href=[\"']#[\"']|href=\{[\"']#[\"']\}", code):
            add(SECTION_FAKE, p, lines.at(m.start()), 'href="#" goes nowhere')
        for m in re.finditer(r"on[A-Z]\w*=\{\s*\(\s*\w*\s*\)\s*=>\s*(?:\{\s*\}|undefined|null)\s*\}", code):
            context = code[max(0, m.start() - 600) : m.start()]
            if any(rel(p) == f and marker in context for f, marker in EMPTY_HANDLER_ALLOWED):
                continue
            add(SECTION_FAKE, p, lines.at(m.start()), "an event handler that does nothing")


# ---------------------------------------------------------------------------
# 13. Settings places
# ---------------------------------------------------------------------------

SECTION_SETTINGS = 'A "Settings > ..." place that does not exist'
SEP = r"\s*(?:>|→|&gt;|›)\s*"
SETTINGS_PATH = re.compile(
    r"(?<![Ss]ystem )(?<![Ss]ystem's )\bSettings" + SEP + r"((?:\*\*)?[A-Z][^>→›\n]*?(?:" + SEP + r"[^>→›\n]*?)*)(?=[.,;:)(`\"'|—–]|\s-\s|\*\*\s|\s{2}|$)"
)


def settings_places() -> tuple[dict[str, set[str]], dict[str, set[str]]] | None:
    """({tab label: places on it}, {group label: tab labels}), read from the source."""
    page = ROOT / "src/components/settings/SettingsPage.tsx"
    groups_file = ROOT / "src/components/settings/settingsGroups.ts"
    if not page.exists() or not groups_file.exists():
        return None
    tabs: dict[str, set[str]] = {}
    tab_ids: dict[str, str] = {}
    for m in re.finditer(r"\{\s*id:\s*'([\w-]+)',\s*label:\s*'([^']+)',\s*icon:\s*\w+,\s*component:\s*(\w+)\s*\}", read(page)):
        tab_id, label, component = m.groups()
        tab_ids[tab_id] = label
        places: set[str] = set()
        sources = files(f"src/components/settings/tabs/{component}.tsx")
        # A tab's own sub-components (CrashReportSection, DevToolsSection, ...)
        for s in list(sources):
            for imp in re.findall(r"from '\./(\w+)'", read(s)):
                sources += files(f"src/components/settings/tabs/{imp}.tsx")
        for s in sources:
            text = read(s)
            places |= set(re.findall(r'\b(?:title|label)="([^"]+)"', text))
            # Short text inside an element: headings, labels, the small
            # bold "title" paragraphs some sections use, and button text
            # (the Fallback tab's "Video Fallback" is a button).
            for t in re.findall(r">\s*([^<>{}=;]{2,80}?)\s*</(?:h[1-6]|label|p|span|summary|Button|button)>", text, re.S):
                places.add(re.sub(r"\s+", " ", t).strip())
        tabs[label] = {p for p in places if p}
    groups: dict[str, set[str]] = {}
    for m in re.finditer(r"\{\s*id:\s*'[\w-]+',\s*label:\s*'([^']+)',\s*tabs:\s*\[([^\]]*)\]", read(groups_file), re.S):
        groups[m.group(1)] = {tab_ids.get(t, t) for t in re.findall(r"'([\w-]+)'", m.group(2))}
    return (tabs, groups) if tabs else None


def place_matches(doc: str, places: set[str]) -> bool:
    """True when the place a document names is a real section or control.

    Either the document names the whole place, perhaps with words after it
    ("Video Fallback chain"), or a shorter form of a longer control label
    ("Enhanced Lyrics" for "Enhanced Lyrics (Word-by-Word Sync)"). Places
    under four letters are ignored, so a stray short word on a tab cannot
    make any name match.
    """
    d = doc.strip(" *").lower()
    for place in places:
        p = place.lower()
        if len(p) < 4:
            continue
        if d == p or p.startswith(d) and len(d) >= 4:
            return True
        if d.startswith(p) and not d[len(p)].isalnum():
            return True
    return False


def check_settings_paths() -> None:
    known = settings_places()
    if known is None:
        add(SECTION_SETTINGS, "src/components/settings", 0, "could not read the Settings tabs, so no Settings place was checked")
        return
    tabs, groups = known

    def check_text(path: Path, n: int, text: str) -> None:
        for m in SETTINGS_PATH.finditer(text):
            parts = [p.strip(" *") for p in re.split(SEP, m.group(1)) if p.strip(" *")]
            if not parts:
                continue
            first = parts[0]
            tab = next((t for t in sorted(tabs, key=len, reverse=True) if first.startswith(t)), None)
            if tab is None:
                group = next((g for g in groups if first.startswith(g)), None)
                if group is None:
                    add(SECTION_SETTINGS, path, n, f'no Settings tab called "{first[:40]}"')
                elif len(parts) > 1 and not place_matches(parts[1], groups[group]):
                    add(SECTION_SETTINGS, path, n, f'no "{parts[1][:40]}" tab under {group}')
                continue
            for part in parts[1:3]:
                if not place_matches(part, tabs[tab]):
                    add(SECTION_SETTINGS, path, n, f'no "{part[:40]}" on the {tab} tab')
                    break

    for p in files("help/**/*.md") + files("README.md"):
        for n, text in markdown_prose(p):
            check_text(p, n, text)
    for p, n, value in locale_values():
        if "/en/" in str(p):
            check_text(p, n, value)
    for p in interface_sources():
        for n, text in interface_text(p):
            check_text(p, n, text.replace("&gt;", ">"))


# ---------------------------------------------------------------------------
# 14. Claims of features with no code behind them
# ---------------------------------------------------------------------------

SECTION_CLAIMS = "A feature is claimed that has no working code behind it"


def bpm_has_a_caller() -> bool:
    """True once something outside bpm_service.rs calls into it (#1160)."""
    for p in backend_sources():
        if p.name == "bpm_service.rs":
            continue
        if re.search(r"bpm_service::\w", blank_comments(read(p))):
            return True
    return False


def cloud_upload_offered() -> bool:
    """True once rclone leaves NOT_OFFERED_YET (direct-to-cloud upload, #859)."""
    dm = ROOT / "src-tauri/src/services/dependency_manager.rs"
    if not dm.exists():
        return False
    m = re.search(r"const NOT_OFFERED_YET[^=]*=\s*&\[([^\]]*)\]", read(dm))
    return bool(m) and '"rclone"' not in m.group(1)


def spotify_for_everyone() -> bool:
    """True once the Spotify tab stops being developer-only (#101)."""
    g = ROOT / "src/components/settings/settingsGroups.ts"
    if not g.exists():
        return False
    m = re.search(r"DEVELOPER_ONLY_TABS[^=]*=\s*new Set\(\[([^\]]*)\]", read(g))
    return bool(m) and "'spotify'" not in m.group(1)


# The hand-kept map. Each entry: what the claim looks like, and the test that
# makes it true. Where the claim is looked for: help pages, the README and
# the translations -- text everyone reads. (The Download page's Spotify
# wording is shown only with developer access; tests cover that.)
FEATURE_CLAIMS = [
    {
        "feature": "BPM tagging",
        "claim": re.compile(r"\bBPM (?:analysis|tagging|detection|tags? (?:are|is) written)\b|✅[^|\n]*\(`?tmpo`?\)", re.I),
        "true_when": bpm_has_a_caller,
        "why": "nothing calls bpm_service (#1160)",
    },
    {
        "feature": "direct-to-cloud upload",
        "claim": re.compile(r"\bCloud Destinations?\b|\bdirect-to-cloud upload\b|\binstall(?:s|ing)? rclone\b", re.I),
        "true_when": cloud_upload_offered,
        "why": "rclone is in NOT_OFFERED_YET until cloud upload lands (#859)",
    },
    {
        "feature": "Spotify downloads for everyone",
        "claim": re.compile(r"\bApple Music or Spotify\b|\bSpotify links?\b[^.\n]{0,60}\b(?:accepted|queues? it)\b", re.I),
        "true_when": spotify_for_everyone,
        "why": "Spotify is a developer-only preview (DEVELOPER_ONLY_TABS)",
    },
]


def check_claims() -> None:
    texts: list[tuple[Path, int, str]] = []
    for p in files("help/**/*.md") + files("README.md"):
        texts += [(p, n, t) for n, t in markdown_prose(p)]
        # README tables put claims in inline code too; the bpm table row
        # in help uses `tmpo` in backticks -- read whole lines as well.
        texts += [(p, n, line) for n, line in enumerate(read(p).split("\n"), 1) if "✅" in line]
    texts += locale_values()
    for claim in FEATURE_CLAIMS:
        if claim["true_when"]():
            continue
        seen = set()
        for p, n, text in texts:
            m = claim["claim"].search(text)
            if m and (p, n) not in seen:
                seen.add((p, n))
                add(SECTION_CLAIMS, p, n, f'claims {claim["feature"]} ("{m.group(0)[:40]}"), but {claim["why"]}')


# ---------------------------------------------------------------------------
# 15. The hidden developer unlock is never named
# ---------------------------------------------------------------------------

SECTION_UNLOCK = "A message names the hidden developer unlock"
UNLOCK = re.compile(r"\bkonami\b|up,? up,? down,? down", re.I)


def check_hidden_unlock() -> None:
    for p in interface_sources():
        for n, text in interface_text(p):
            if UNLOCK.search(text):
                add(SECTION_UNLOCK, p, n, "interface text names the hidden unlock")
    for p in backend_sources():
        for n, text in string_literals(clean_rust(p)):
            if UNLOCK.search(text):
                add(SECTION_UNLOCK, p, n, "a backend message names the hidden unlock")
    for p in in_app_help():
        for n, text in markdown_prose(p):
            if UNLOCK.search(text):
                add(SECTION_UNLOCK, p, n, "a help page names the hidden unlock")
    for p, n, value in locale_values():
        if UNLOCK.search(value):
            add(SECTION_UNLOCK, p, n, "a translation names the hidden unlock")


# ---------------------------------------------------------------------------
# 16. Ratchets
# ---------------------------------------------------------------------------

SECTION_RATCHET = "Ratchet: a count that may only go down has moved"

# Ceilings, set on 2026-10-05 after the first polish pass. They may only go
# DOWN: when a change makes a count smaller, lower its ceiling to the new
# count in the same change (the check insists, so the count cannot quietly
# creep back up later). Raising one needs a reason written here.
RATCHETS = {
    "raw_button": 51,
    "raw_select": 3,
    "raw_input": 11,
    "px_font_size": 47,
    "non_token_rounding": 85,
    "failed_to_toast": 0,
}

RATCHET_WHAT = {
    "raw_button": "hand-made <button> outside components/common (use <Button>)",
    "raw_select": "hand-made <select> outside components/common (use <Select>)",
    "raw_input": "hand-made <input> outside components/common (use <Input>, <Toggle>, ...)",
    "px_font_size": "hard-coded text-[Npx] font size (use the type scale)",
    "non_token_rounding": "corner rounding that bypasses rounded-platform",
    "failed_to_toast": 'toast whose message is the raw error ("Failed to X: ${error}", "${err}", "err.message")',
}


@functools.lru_cache(maxsize=None)
def _ratchet_counts() -> tuple[tuple[str, int], ...]:
    return tuple(_count_ratchets().items())


def ratchet_counts() -> dict[str, int]:
    """Today's count for each ratchet (worked out once per run)."""
    return dict(_ratchet_counts())


# A toast whose message is, or is built from, the raw error. The polish
# pass (M8) rewrote every one: the message says what went wrong and what to
# do next, and the raw text goes under "Details" (showError /
# showBackendError / withErrorToast in src/lib). Three shapes are counted:
#   - "Failed to X: ${...}" (the original ratchet);
#   - a template that interpolates the caught error (`${e}`, `${err}`,
#     `${error}`, `${msg}`, `${reason}`);
#   - the caught error by itself (`err instanceof Error ? err.message : ...`).
# A variable called `message` is not counted: in this codebase it usually
# holds a sentence written for people.
RAW_ERROR_TOAST = re.compile(
    r"addToast\(\s*(?:"
    r"`[^`]*\bFailed to\b[^`]*\$\{"
    r"|`[^`]*\$\{\s*(?:e|err|error|msg|reason)\s*(?:\}|\binstanceof\b|\.message)"
    r"|(?:e|err|error)\s+instanceof\s+Error\s*\?"
    r")"
)


def _count_ratchets() -> dict[str, int]:
    counts = dict.fromkeys(RATCHETS, 0)
    for p in interface_sources():
        code = clean(p)
        outside_common = p.suffix == ".tsx" and "/components/" in str(p) and "/components/common/" not in str(p)
        if outside_common:
            counts["raw_button"] += len(re.findall(r"<button\b", code))
            counts["raw_select"] += len(re.findall(r"<select\b", code))
            counts["raw_input"] += len(re.findall(r"<input\b", code))
        counts["px_font_size"] += len(re.findall(r"\btext-\[\d+px\]", code))
        counts["non_token_rounding"] += len(
            re.findall(r"(?<![\w-])rounded(?:-(?:t|b|l|r|s|e|tl|tr|bl|br))?(?:-(?:sm|md|lg|xl|2xl|3xl))?(?![\w-])", code)
        )
        counts["failed_to_toast"] += len(re.findall(RAW_ERROR_TOAST, code))
    return counts


def check_ratchets() -> None:
    for name, count in ratchet_counts().items():
        ceiling = RATCHETS[name]
        if count > ceiling:
            add(SECTION_RATCHET, "tools/audit-checks/check_polish.py", 0, f"{RATCHET_WHAT[name]}: {count}, above the ceiling of {ceiling}")
        elif count < ceiling:
            add(
                SECTION_RATCHET,
                "tools/audit-checks/check_polish.py",
                0,
                f'{RATCHET_WHAT[name]}: {count}, below the ceiling of {ceiling} -- good; lower RATCHETS["{name}"] to {count} in this change',
            )


# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

RULES = [
    check_placeholders,
    check_internal_references,
    check_identity,
    check_window_title,
    check_icons,
    check_router,
    check_crash_screen,
    check_headings,
    check_button_names,
    check_debug_output,
    check_drafts,
    check_chunk_limit,
    check_fake,
    check_settings_paths,
    check_claims,
    check_hidden_unlock,
    check_ratchets,
]


def main() -> int:
    for rule in RULES:
        rule()
    total = sum(len(v) for v in FINDINGS.values())
    print(f"Polish check: {len(RULES)} rules over {ROOT.name}")
    counts = ratchet_counts()
    print("Ratchet counts: " + ", ".join(f"{k} {counts[k]}/{RATCHETS[k]}" for k in RATCHETS))
    print()
    for section in SECTION_ORDER:
        print(f"### {section}\n")
        for path, line, message in FINDINGS[section]:
            where = f"{path}:{line}" if line else path
            print(f"  • {where} — {message}")
        print()
    # The last line, always: the self-test checks it is there, so a crash
    # part-way (which prints a traceback instead) can never pass as clean.
    print(f"Polish check finished: {total} finding(s).")
    return 1 if total and "--strict" in sys.argv else 0


if __name__ == "__main__":
    sys.exit(main())
