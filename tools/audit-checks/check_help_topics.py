#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Help documentation consistency check.

The in-app Help screen used to carry its own, hand-typed copy of every
help topic's words, separate from the real files in `help/*.md` that
people read on GitHub. Whenever someone edited one and forgot the other,
the two silently drifted apart -- a user reading the in-app Help screen
saw different words than someone reading the same topic on GitHub, and
nothing ever warned either of them (issue #949, where the two copies
disagreed about which "coming soon" version a user was on).

That hand-typed copy is gone now. The Help screen reads its content
straight from `help/*.md` at build time (`src/components/help/helpTopics.ts`).
But reading the same file does not make two sources the same source --
there are still two lists that both have to describe the same set of
pages, and nothing in TypeScript or Rust checks that they agree:

  (A) The files on disk       -- `help/*.md`.
  (B) The app's own list      -- `HELP_TOPIC_MANIFEST` in `helpTopics.ts`,
                                 which says which files are shown, in what
                                 order, with what label and icon.

A file can exist with no manifest line (the app will never show it -- a
page nobody can reach). A manifest line can name a file that doesn't
exist (helpTopics.ts already throws at app startup for this one, but
this check catches it at review time, before anyone runs the app). Code
elsewhere in the app can link to a help page by its id -- a settings
row's "?" button, a "Learn more" click -- and that id can be wrong.
Help pages can link to *each other* and get a filename wrong. A help
page can use GitHub-only emoji syntax (`:rocket:`) that renders as a
picture on GitHub but as the literal text ":rocket:" inside the app,
which has no emoji renderer. A translated help page can exist with no
English original for the app to fall back to. And a translated page's
language folder (`help/<language>/`) can name a language the app has
never heard of -- a typo, or a translation nobody wired up in the
Settings language dropdown -- which means nobody can ever actually
select it and see the page.

This script checks all eight of those things.

Exit code:
  0 -- no findings (or only informational), OR findings without --strict
  1 -- at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_help_topics.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HELP_DIR = REPO_ROOT / "help"
HELPTOPICS_TS = REPO_ROOT / "src" / "components" / "help" / "helpTopics.ts"
I18N_TS = REPO_ROOT / "src" / "lib" / "i18n.ts"
FRONTEND_SRC = REPO_ROOT / "src"

# Files that DEFINE the `helpTopic` prop / `navigateToHelp` action rather
# than USING them to link to a specific page. Scanning these would either
# find nothing (the type declaration `helpTopic?: HelpTopicId` isn't a
# string literal our regex matches) or, in HelpButton.tsx's own doc
# comment example, a placeholder id that isn't a real link a user can
# click -- skipped explicitly so nobody has to reason about that each time
# the check runs.
DEFINITION_SITE_FILES = {"HelpButton.tsx", "uiStore.ts"}

# Help pages that are deliberately NOT shown in the app, with the reason
# a person can act on. An id here must NOT appear in HELP_TOPIC_MANIFEST --
# if it does, the exception is stale (see check 3) and should be deleted.
EXCEPTIONS: dict[str, str] = {
    "index": (
        "The table of contents for someone reading the files on GitHub, "
        "where there is no sidebar to browse by. Inside the app, the Help "
        "screen's own sidebar already does that job, so an in-app copy of "
        "index.md would just be a page of links to pages already listed "
        "beside it."
    ),
}


def strip_line_comments(text: str) -> str:
    """Strip // line comments and /* */ block comments while preserving
    line numbers (block comments are replaced by an equal count of
    newlines so reported line numbers still line up with the original
    file). Copied from check_ipc_commands.py rather than imported, to
    keep each audit script runnable on its own with no local package
    setup."""
    text = re.sub(
        r"/\*.*?\*/",
        lambda m: "\n" * m.group(0).count("\n"),
        text,
        flags=re.DOTALL,
    )
    text = re.sub(r"//[^\n]*", "", text)
    return text


def strip_fenced_code(text: str) -> str:
    """Blank out ``` ... ``` fenced code blocks, keeping line count intact,
    so a sample command or config snippet inside a help page (an ssh
    tunnel with `host:port:host:port`, an SRT timestamp, a freeform atom
    name typed in backticks) is never mistaken for a broken cross-link or
    a stray emoji shortcode."""
    return re.sub(
        r"```.*?```",
        lambda m: "\n" * m.group(0).count("\n"),
        text,
        flags=re.DOTALL,
    )


def strip_inline_code(text: str) -> str:
    """Blank out `inline code` spans the same way, for the same reason --
    a single-backtick example like `----:MeedyaMeta:` is prose about a
    freeform atom name, not GitHub emoji syntax."""
    return re.sub(
        r"`[^`\n]*`",
        lambda m: " " * len(m.group(0)),
        text,
    )


def extract_bracket_block(text: str, opener: str) -> tuple[str, int]:
    """Find `opener` (e.g. "HELP_TOPIC_MANIFEST = [") and bracket-match
    from its `[` to the matching `]`, the same way
    check_ipc_commands.py's collect_registered_commands() finds the
    generate_handler![ ... ] block. Returns (block_text, offset_of_block_start)
    so a caller can map a match inside the block back to a line number in
    the original file."""
    start = text.find(opener)
    if start == -1:
        return "", -1
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
    return text[open_idx + 1 : end_idx], open_idx + 1


def line_no_at(text: str, idx: int) -> int:
    """1-indexed line number of character offset `idx` in `text`."""
    return text[:idx].count("\n") + 1


# ---------------------------------------------------------------------------
# (A) The files on disk.
# ---------------------------------------------------------------------------


def collect_help_pages() -> dict[str, Path]:
    """Every English help page: help/<id>.md. Deliberately NOT recursive --
    a future help/<language>/ subdirectory holds translations, which check
    7 (collect_translation_gaps) looks at separately, not pages of this
    list."""
    return {md.stem: md for md in sorted(HELP_DIR.glob("*.md"))}


# ---------------------------------------------------------------------------
# (B) The app's own list.
# ---------------------------------------------------------------------------


def collect_manifest_entries() -> dict[str, int]:
    """Returns {id: line_no} for every entry in HELP_TOPIC_MANIFEST, read
    from helpTopics.ts. Bracket-matches the array the way
    check_ipc_commands.py bracket-matches generate_handler![ ... ] --
    this script does not attempt to parse TypeScript for real."""
    if not HELPTOPICS_TS.exists():
        print(f"WARNING: {HELPTOPICS_TS} does not exist", file=sys.stderr)
        return {}
    text = HELPTOPICS_TS.read_text(encoding="utf-8", errors="ignore")
    block, block_offset = extract_bracket_block(text, "HELP_TOPIC_MANIFEST = [")
    if block_offset == -1:
        print(
            "WARNING: HELP_TOPIC_MANIFEST = [ not found in helpTopics.ts",
            file=sys.stderr,
        )
        return {}
    stripped = strip_line_comments(block)
    entries: dict[str, int] = {}
    for m in re.finditer(r"""id:\s*['"]([a-z][a-z0-9-]*)['"]""", stripped):
        entries.setdefault(m.group(1), line_no_at(text, block_offset + m.start()))
    return entries


# ---------------------------------------------------------------------------
# Check 4: code that links to a help page by id.
# ---------------------------------------------------------------------------

HELP_TOPIC_ATTR_RE = re.compile(r"""helpTopic=["']([a-z][a-z0-9-]*)["']""")
NAVIGATE_TO_HELP_RE = re.compile(r"""navigateToHelp\(\s*["']([a-z][a-z0-9-]*)["']\s*\)""")


def collect_frontend_help_links() -> list[tuple[str, str, int]]:
    """Every `helpTopic="..."` / `navigateToHelp('...')` deep link found
    under src/**/*.ts and *.tsx. Returns (target_id, rel_path, line_no)
    tuples. Skips test files (they mock components and don't reflect real
    navigation) and the two files that DEFINE these APIs rather than use
    them."""
    links: list[tuple[str, str, int]] = []
    for ext in ("*.ts", "*.tsx"):
        for f in sorted(FRONTEND_SRC.rglob(ext)):
            if f.name in DEFINITION_SITE_FILES:
                continue
            rel = str(f.relative_to(REPO_ROOT))
            rel_posix = rel.replace("\\", "/")
            if ".test." in f.name or "/test/" in rel_posix or "__tests__" in rel_posix:
                continue
            try:
                raw = f.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            stripped = strip_line_comments(raw)
            for regex in (HELP_TOPIC_ATTR_RE, NAVIGATE_TO_HELP_RE):
                for m in regex.finditer(stripped):
                    links.append((m.group(1), rel, line_no_at(stripped, m.start())))
    return links


# ---------------------------------------------------------------------------
# Check 5: help pages linking to each other.
# ---------------------------------------------------------------------------

MD_LINK_RE = re.compile(r"\]\(([^)]+)\)")


def collect_help_cross_links() -> list[tuple[str, int, str, Path]]:
    """Every `[text](other-page.md)` (optionally `#with-an-anchor`) style
    link found in help/**/*.md. Returns (source_rel, line_no, raw_target,
    resolved_path) tuples for links that name a local .md file -- external
    http(s)/mailto: links are not this check's concern, and neither is a
    bare `#anchor-on-the-same-page` link."""
    results: list[tuple[str, int, str, Path]] = []
    for md in sorted(HELP_DIR.rglob("*.md")):
        rel = str(md.relative_to(REPO_ROOT))
        text = md.read_text(encoding="utf-8", errors="ignore")
        scannable = strip_fenced_code(text)
        for m in MD_LINK_RE.finditer(scannable):
            target = m.group(1).strip()
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            path_part = target.split("#", 1)[0]
            if not path_part.endswith(".md"):
                continue
            resolved = (md.parent / path_part).resolve()
            results.append((rel, line_no_at(scannable, m.start()), target, resolved))
    return results


# ---------------------------------------------------------------------------
# Check 6: GitHub-only emoji shortcodes.
# ---------------------------------------------------------------------------

# A GitHub emoji shortcode is a colon, a run of letters/digits/underscore/
# plus/minus with at least one letter in it, and a closing colon --
# `:rocket:`, `:white_check_mark:`, `:+1:`. The "at least one letter"
# requirement is what keeps this from matching an SRT/TTML timestamp
# (`00:00:12,340`) or a `host:port:host:port` ssh tunnel command, both of
# which are colon-digits-colon with no letters. Fenced code blocks and
# inline code spans are blanked out first (see strip_fenced_code /
# strip_inline_code) so a sample command or a freeform-atom name typed in
# backticks is never mistaken for real emoji syntax.
EMOJI_SHORTCODE_RE = re.compile(r":(?=[A-Za-z0-9_+-]*[A-Za-z])[A-Za-z0-9_+-]+:")


def collect_emoji_shortcodes() -> list[tuple[str, int, str]]:
    """Every apparent GitHub emoji shortcode found in help/**/*.md, outside
    fenced code blocks and inline code spans. Returns (rel_path, line_no,
    shortcode) tuples."""
    results: list[tuple[str, int, str]] = []
    for md in sorted(HELP_DIR.rglob("*.md")):
        rel = str(md.relative_to(REPO_ROOT))
        text = md.read_text(encoding="utf-8", errors="ignore")
        scannable = strip_inline_code(strip_fenced_code(text))
        for m in EMOJI_SHORTCODE_RE.finditer(scannable):
            results.append((rel, line_no_at(scannable, m.start()), m.group(0)))
    return results


# ---------------------------------------------------------------------------
# Check 7: translated pages with no English original.
# ---------------------------------------------------------------------------


def collect_translation_gaps() -> list[tuple[str, str]]:
    """A future help/<language>/<name>.md with no matching help/<name>.md.
    Nothing produces this today (no help/<language>/ directory exists
    yet), but it is here ahead of the German and French translations that
    are the next piece of this work -- a translated page with no English
    original means the app's fallback-to-English has nothing to fall back
    to. Returns (rel_path, filename) tuples."""
    results: list[tuple[str, str]] = []
    for sub in sorted(p for p in HELP_DIR.iterdir() if p.is_dir()):
        for md in sorted(sub.glob("*.md")):
            english = HELP_DIR / md.name
            if not english.exists():
                results.append((str(md.relative_to(REPO_ROOT)), md.name))
    return results


# ---------------------------------------------------------------------------
# Check 8: a translated-page folder for a language the app doesn't know.
# ---------------------------------------------------------------------------


def collect_locale_codes() -> set[str]:
    """Every language code the app actually knows about -- read from the
    `LOCALES` list in `src/lib/i18n.ts`, the one place (per that file's
    own comment) a supported language is declared. Bracket-matches the
    array the same way `collect_manifest_entries()` above bracket-matches
    `HELP_TOPIC_MANIFEST` -- this script does not attempt to parse
    TypeScript for real, same as everywhere else in this file."""
    if not I18N_TS.exists():
        print(f"WARNING: {I18N_TS} does not exist", file=sys.stderr)
        return set()
    text = I18N_TS.read_text(encoding="utf-8", errors="ignore")
    block, block_offset = extract_bracket_block(text, "LOCALES = [")
    if block_offset == -1:
        print("WARNING: LOCALES = [ not found in i18n.ts", file=sys.stderr)
        return set()
    stripped = strip_line_comments(block)
    return {
        m.group(1) for m in re.finditer(r"""code:\s*['"]([a-z]{2,3})['"]""", stripped)
    }


def collect_unknown_language_folders() -> list[str]:
    """Every `help/<name>/` directory whose name is NOT one of the codes
    in `LOCALES`. Two ways this happens in practice: a typo in the
    folder name (`help/frr/` instead of `help/fr/`), or a translation
    added for a language nobody has added to `LOCALES` yet -- which
    means it never appears in the Settings language dropdown, so no
    user can ever select it and no one would ever see the page sitting
    there. Returns the bare folder names, sorted."""
    locale_codes = collect_locale_codes()
    return sorted(
        p.name for p in HELP_DIR.iterdir() if p.is_dir() and p.name not in locale_codes
    )


def check() -> int:
    help_pages = collect_help_pages()
    manifest = collect_manifest_entries()
    frontend_links = collect_frontend_help_links()
    cross_links = collect_help_cross_links()
    emoji_shortcodes = collect_emoji_shortcodes()
    translation_gaps = collect_translation_gaps()
    unknown_language_folders = collect_unknown_language_folders()

    print(f"Help pages in help/*.md            : {len(help_pages)}")
    print(f"Entries in HELP_TOPIC_MANIFEST      : {len(manifest)}")
    print(f"In-app help deep links checked      : {len(frontend_links)}")
    print(f"Help-page-to-help-page links checked: {len(cross_links)}")
    print()

    findings = 0

    # 1. A help page the app never shows.
    unshown = sorted(set(help_pages) - set(manifest) - set(EXCEPTIONS))
    if unshown:
        print("### Help page exists but is not in HELP_TOPIC_MANIFEST (the Help screen will never show it)\n")
        for id_ in unshown:
            print(
                f"  • help/{id_}.md — add a line for it to HELP_TOPIC_MANIFEST in "
                f"src/components/help/helpTopics.ts (label, icon, and where it belongs in "
                f"the sidebar order), or, if it's deliberately not meant to appear in the "
                f"app, add it to EXCEPTIONS in this script with the reason."
            )
        print()
        findings += len(unshown)

    # 2. A manifest entry with no file.
    missing_file = sorted(set(manifest) - set(help_pages))
    if missing_file:
        print("### HELP_TOPIC_MANIFEST lists a page with no matching file\n")
        for id_ in missing_file:
            line = manifest[id_]
            print(
                f"  • src/components/help/helpTopics.ts:{line} — '{id_}' is in "
                f"HELP_TOPIC_MANIFEST but help/{id_}.md does not exist. Add the file, or "
                f"remove the manifest line. (The app itself already refuses to start over "
                f"this — this check catches it at review time instead of at launch.)"
            )
        print()
        findings += len(missing_file)

    # 3. A stale repo-only exception.
    stale_exceptions = sorted(id_ for id_ in EXCEPTIONS if id_ in manifest)
    if stale_exceptions:
        print("### Recorded as repo-only, but it is now shown in the app\n")
        for id_ in stale_exceptions:
            print(
                f"  • tools/audit-checks/check_help_topics.py — '{id_}' is listed in "
                f"EXCEPTIONS as deliberately not shown in the app, but it is now in "
                f"HELP_TOPIC_MANIFEST. Remove the entry — it's no longer true."
            )
        print()
        findings += len(stale_exceptions)

    # 4. A deep link in app code to a page that doesn't exist.
    broken_links = [
        (target, rel, line) for target, rel, line in frontend_links if target not in manifest
    ]
    if broken_links:
        print("### Code links to a help page that is not in HELP_TOPIC_MANIFEST\n")
        for target, rel, line in sorted(broken_links, key=lambda t: (t[1], t[2])):
            print(
                f"  • {rel}:{line} — links to help page '{target}', which "
                f"HELP_TOPIC_MANIFEST does not list. Fix the id, or add the page."
            )
        print()
        findings += len(broken_links)

    # 5. A help page linking to a help page that doesn't exist.
    broken_cross_links = [
        (rel, line, target) for rel, line, target, resolved in cross_links if not resolved.exists()
    ]
    if broken_cross_links:
        print("### A help page links to another help page that does not exist\n")
        for rel, line, target in broken_cross_links:
            print(
                f"  • {rel}:{line} — links to '{target}', which does not exist. Fix the "
                f"filename, or remove the link."
            )
        print()
        findings += len(broken_cross_links)

    # 6. GitHub-only emoji syntax.
    if emoji_shortcodes:
        print("### GitHub-only emoji syntax in a help page (shows as literal text in the app)\n")
        for rel, line, shortcode in emoji_shortcodes:
            print(
                f"  • {rel}:{line} — '{shortcode}' renders as a picture on GitHub, but the "
                f"app has no emoji renderer and shows it as the literal text you typed. Use "
                f"the actual character, or remove it."
            )
        print()
        findings += len(emoji_shortcodes)

    # 7. A translated page with no English original.
    if translation_gaps:
        print("### Translated help page has no English original to fall back to\n")
        for rel, name in translation_gaps:
            print(
                f"  • {rel} — there is no help/{name} for this to be a translation of. Add "
                f"the English page, or remove the translation."
            )
        print()
        findings += len(translation_gaps)

    # 8. A translated-page folder for a language the app doesn't know.
    if unknown_language_folders:
        print("### Translated help page folder is not a language the app knows about\n")
        for name in unknown_language_folders:
            print(
                f"  • help/{name}/ — '{name}' is not one of the language codes in LOCALES "
                f"(src/lib/i18n.ts), so no one can ever select it in the Settings language "
                f"dropdown and no one would ever see the pages inside it. Add '{name}' to "
                f"LOCALES if this is meant to be a real, supported language, or remove the "
                f"help/{name}/ folder if it isn't."
            )
        print()
        findings += len(unknown_language_folders)

    if findings == 0:
        translated_page_count = sum(
            1 for sub in HELP_DIR.iterdir() if sub.is_dir() for _ in sub.glob("*.md")
        )
        print(
            f"OK — {len(help_pages)} help page(s), {len(manifest)} HELP_TOPIC_MANIFEST "
            f"entries, {len(frontend_links)} in-app deep link(s), {len(cross_links)} "
            f"page-to-page link(s), and {translated_page_count} translated page(s) all "
            f"agree. {len(EXCEPTIONS)} page ({', '.join(sorted(EXCEPTIONS))}) is "
            f"deliberately repo-only, with a recorded reason."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
