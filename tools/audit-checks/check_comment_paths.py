#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Comment file-path existence check.

A comment that names a specific file -- "see src-tauri/src/commands/foo.rs"
-- is a claim a reader trusts without checking, exactly like any other fact
a comment states. Nothing here today verifies that claim. A file gets
renamed, split into a directory of its own, or moved, and every comment
that used to point at it keeps pointing at the old name forever, because
renaming a file does not touch the text of a comment sitting in some other
file.

A 2026-09 audit of this repository found exactly that: eleven comments
across the Rust backend and the TypeScript frontend naming a file that no
longer exists -- a command module renamed from singular to plural
("commands/dependency.rs" -> "commands/dependencies.rs"), a type moved from
one model file into the service that actually owns it, a single-file
service that had since become its own directory of files. Each one had
been sitting there, unnoticed, since the rename that made it wrong. None of
them caused a compiler error, because a comment is not code -- nothing
reads it, nothing runs it, nothing complains when it stops being true.

This script is the thing that would have caught all eleven at once: it
finds every file path mentioned inside a comment (Rust `//`, `///`, `//!`,
`/* */`; TypeScript/JavaScript `//`, `/** */`; Python `#`) that starts with
one of this repo's real top-level source directories, and confirms that
exact path exists on disk relative to the repository root.

What counts as "a file path" here is deliberately narrow, to keep this from
drowning in noise:

  * It must start with one of KNOWN_TOP_LEVEL_PREFIXES below -- a real
    top-level directory of this repository (src-tauri/, src/, tools/,
    scripts/, help/, public/, assets/, .github/, .claude/). A path with no
    such prefix (a bare filename, a relative "../foo", a path inside some
    OTHER project a comment happens to mention) is not checked -- there
    would be no way to resolve it against this repository unambiguously.
  * It must end in one of KNOWN_EXTENSIONS below. A directory mention with
    no filename ("the services/download_queue/ module") is not checked --
    only a specific FILE claim is verifiable this cheaply.
  * A path containing a template placeholder character (`{`, `<`) is
    skipped -- `public/locales/{lang}/translation.json` is not a claim
    about a literal file called "{lang}", it is describing a family of
    files, and checking the literal string would always fail for the
    wrong reason.
  * A path containing this repository's OTHER placeholder idiom -- a
    single uppercase letter standing in for a real value, chained with
    literal dots, such as the "X.Y.Z" in "vX.Y.Z.md" standing in for
    "whatever version number this release turns out to be" -- is skipped
    for the same reason. This idiom shows up throughout the release
    tooling comments (`.github/workflows/preserve-release-pr-body.yml`
    among others) and is never how a real file in this repository is
    actually named: nothing on disk has a bare single letter as its own
    dot/dash/underscore/slash-delimited segment (checked across the whole
    repository, 2026-09-23, before adding this rule, specifically to make
    sure it could not paper over a real broken reference). Detected via
    PLACEHOLDER_VERSION_RE below. It fires ONLY on the version placeholder
    itself: the letters X, Y and Z, each standing alone, joined by dots,
    two or three of them ("X.Y", "X.Y.Z", "vX.Y.Z"), with no letter or
    digit directly either side.

    The first version of this rule matched ANY two capitals around a dot,
    and its description here claimed that was narrow. It was not: it
    also matched the "I.C" inside a real name like `src/API.Client.ts`,
    so a broken reference to that file would have been silently skipped
    -- the one thing this script exists never to do. Codex found it in
    the batch-4 review (finding E); the tests in
    test_check_comment_paths.py pin both halves, what it must skip and
    what it must NOT.
  * A path is skipped when it is preceded on the same line by "://" --
    it is almost certainly the tail end of a URL (a GitHub permalink to a
    specific historical commit, which can legitimately name a path that
    does not exist on the current tree), not a bare claim about this
    repository's current state.

This also reads GitHub Actions workflow files (`.github/workflows/*.yml`).
They were left out of the first version of this script by accident, not on
purpose -- and that accident mattered, because this repository's workflow
comments are, if anything, MORE densely written and MORE confidently worded
than the source code's, and a 2026-09 review found the single worst example
of this whole bug class sitting in one: a comment stated, as settled fact,
that Linux ARM builds "never produce an updater signature on any release",
and that statement was used to tell an automated guard to stop checking ARM
at all. The statement was false. Because the one check that could have
caught it had been told, in a comment, to stop looking, every Linux ARM
user had no working in-app update path for the entire life of the project
before anyone noticed. A comment stating a fact about what a file contains,
or does not contain, is exactly the kind of claim this script exists to
keep honest -- it should never have been scanning code comments while
skipping the comments most likely to make that particular kind of claim.

What this does NOT do: it does not check paths inside Markdown files
(`help/*.md`, `README.md`, ...) -- `check_help_topics.py` already checks
help-page-to-help-page links, and a general prose mention of a path in a
Markdown file is far more likely to be a deliberately loose description
than a precise claim. It does not understand a file that legitimately
moved AFTER a comment was written to describe history ("this used to live
in X before it moved") -- a comment describing the past by naming a path
that no longer exists is, accurately, indistinguishable from a comment
that is simply wrong about the present, and re-reading either kind is the
point of a review-time check like this one.

Exit code:
  0 -- no findings, OR findings without --strict
  1 -- at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_comment_paths.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Extensions this script will bother checking. Deliberately excludes
# binary/asset extensions (png, icns, ...) even though comments do
# occasionally name one (e.g. "assets/brand/icon.icns") -- those are
# extremely unlikely to be renamed by a routine refactor, and the
# false-positive risk of a generated/platform-specific asset not existing
# in a fresh checkout isn't worth the small extra coverage.
KNOWN_EXTENSIONS = (
    "rs", "ts", "tsx", "js", "mjs", "py",
    "toml", "json", "yml", "yaml", "md", "sh", "css", "html", "svg",
)

# Real top-level source directories of this repository. A comment path has
# to start with one of these (with the trailing slash) to be considered a
# checkable claim -- see the module docstring for why.
KNOWN_TOP_LEVEL_PREFIXES = (
    "src-tauri/",
    "src/",
    "tools/",
    "scripts/",
    "help/",
    "public/",
    "assets/",
    ".github/",
    ".claude/",
)

# Directories never worth walking into: build output, dependencies, VCS
# metadata. Checked as a path-part membership test, so this also skips
# nested occurrences (e.g. src-tauri/target/...).
SKIP_DIR_PARTS = {"node_modules", "target", "dist", "build", ".git", "__pycache__"}

# File extensions this script scans for comments (source code and GitHub
# Actions workflows -- Markdown is deliberately still out of scope, see the
# module docstring). ".yml"/".yaml" were added 2026-09-23: they use the same
# `#`-to-end-of-line comment style as Python, so they ride the existing
# hash-comment path in extract_comment_spans() below rather than needing a
# parser of their own.
SCAN_EXTENSIONS = {".rs", ".ts", ".tsx", ".js", ".mjs", ".py", ".yml", ".yaml"}

# Extensions that use `#`-to-end-of-line comments (Python and YAML share this
# style; everything else scanned here uses `//` / `/* */`).
HASH_COMMENT_EXTENSIONS = {".py", ".yml", ".yaml"}

_ext_alt = "|".join(re.escape(e) for e in KNOWN_EXTENSIONS)
PATH_RE = re.compile(
    r"(?<![\w./-])"
    r"(?P<path>(?:" + "|".join(re.escape(p) for p in KNOWN_TOP_LEVEL_PREFIXES) + r")"
    r"[A-Za-z0-9_./-]*\.(?:" + _ext_alt + r"))\b"
)

# This repository's "fill in the real value" idiom for a path that stands
# for a whole family of files rather than one literal file -- see the
# module docstring for why this exists and how narrow it deliberately is.
# The look-behind and look-ahead are what stop it matching INSIDE a real
# name ("API.Client" has a letter right before the I and right after the
# C). The look-behind also refuses a dot, so it cannot pick up the tail
# end of a longer dotted run ("...X.Y.Z" matching just its "Y.Z") -- the
# placeholder has to be a whole segment on its own. Restricting to X/Y/Z
# is what stops it matching some other pair of capitals that merely
# happens to sit either side of a dot.
PLACEHOLDER_VERSION_RE = re.compile(r"(?<![A-Za-z0-9.])v?[XYZ](?:\.[XYZ]){1,2}(?![A-Za-z0-9])")

# Mentioned-path string -> reason it is not a claim about a real file in
# THIS repository. Keyed by the exact string this script would otherwise
# report, since the same illustrative example could recur in more than one
# place.
EXCEPTIONS: dict[str, str] = {
    "src/rust/main.rs": (
        "Describes the wrapper-v2 daemon's OWN source layout (a separate "
        "project this app talks to over HTTP), not a path inside MeedyaDL."
    ),
    "help/some-page.md": (
        "A placeholder name used in HelpViewer.tsx's own comment to "
        "illustrate the general SHAPE of a relative help-page link "
        "('some-page.md' stands in for whichever real page is linked), "
        "not a claim that a file literally named this exists."
    ),
    "help/new-page.md": (
        "A hypothetical filename used in helpTopics.test.ts's own comment "
        "to describe the scenario the test is guarding against ('someone "
        "adds a page and forgets the manifest line'), not a claim that "
        "this file exists."
    ),
    "help/some-new-page.md": (
        "Same as help/new-page.md above -- an illustrative filename in a "
        "test comment, not a real file."
    ),
    "src/advisories/diags.rs": (
        "Describes cargo-deny's OWN source layout (channel-security-audit.yml "
        "cites the exact file, inside the cargo-deny crate, that defines the "
        "'index-failure' / 'index-cache-load-failure' diagnostic codes the "
        "workflow checks for), not a path inside MeedyaDL. It only matches "
        "KNOWN_TOP_LEVEL_PREFIXES by coincidence, because cargo-deny's crate "
        "also happens to keep its code under a top-level src/ directory."
    ),
}


def iter_source_files():
    for ext in SCAN_EXTENSIONS:
        for f in sorted(REPO_ROOT.rglob(f"*{ext}")):
            rel = f.relative_to(REPO_ROOT)
            if SKIP_DIR_PARTS & set(rel.parts):
                continue
            yield f, rel


def extract_comment_spans(text: str, is_hash_comment: bool) -> list[tuple[int, int]]:
    """Returns (start, end) character-offset spans of every comment in
    `text` -- `#...` to end of line for Python and YAML (they share this
    comment style), `//...` to end of line and `/* ... */` for everything
    else. Deliberately simple: does not try to tell a `//` (or, for YAML, a
    `#`) inside a string literal from a real comment start. That can only
    ever widen a scanned span to include a little real code, which risks a
    missed distinction, never a false alarm about a path that isn't really
    mentioned -- the file-path regex below still has to match something
    path-shaped inside whatever text gets returned."""
    spans: list[tuple[int, int]] = []
    n = len(text)
    if is_hash_comment:
        i = 0
        while i < n:
            if text[i] == "#":
                start = i
                while i < n and text[i] != "\n":
                    i += 1
                spans.append((start, i))
            else:
                i += 1
        return spans

    i = 0
    while i < n:
        two = text[i : i + 2]
        if two == "//":
            start = i
            while i < n and text[i] != "\n":
                i += 1
            spans.append((start, i))
            continue
        if two == "/*":
            start = i
            end = text.find("*/", i + 2)
            end = n if end == -1 else end + 2
            spans.append((start, end))
            i = end
            continue
        i += 1
    return spans


def line_no_at(text: str, idx: int) -> int:
    return text[:idx].count("\n") + 1


def check() -> int:
    findings: list[tuple[str, int, str]] = []
    checked_mentions = 0
    files_scanned = 0

    for f, rel in iter_source_files():
        try:
            text = f.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        files_scanned += 1
        rel_str = str(rel).replace("\\", "/")
        for start, end in extract_comment_spans(
            text, is_hash_comment=f.suffix in HASH_COMMENT_EXTENSIONS
        ):
            comment_text = text[start:end]
            for m in PATH_RE.finditer(comment_text):
                mentioned = m.group("path")
                if "{" in mentioned or "<" in mentioned:
                    continue
                if PLACEHOLDER_VERSION_RE.search(mentioned):
                    continue
                abs_match_start = start + m.start()
                # Skip the tail of a URL -- "://" appearing anywhere
                # earlier in the same line as this match.
                line_start = comment_text.rfind("\n", 0, m.start()) + 1
                line_prefix = comment_text[line_start : m.start()]
                if "://" in line_prefix:
                    continue
                checked_mentions += 1
                if mentioned in EXCEPTIONS:
                    continue
                target = REPO_ROOT / mentioned
                if not target.exists():
                    line_no = line_no_at(text, abs_match_start)
                    findings.append((rel_str, line_no, mentioned))

    print(f"Source files scanned  : {files_scanned}")
    print(f"Path-shaped mentions  : {checked_mentions}")
    print()

    if findings:
        print("### Comment names a file path that does not exist\n")
        for rel_str, line_no, mentioned in sorted(findings, key=lambda t: (t[0], t[1])):
            print(
                f"  • {rel_str}:{line_no} — comment mentions `{mentioned}`, which does "
                f"not exist in the repository. The file was likely renamed or moved -- "
                f"update the comment to point at the real location, or remove the "
                f"reference if it's describing history."
            )
        print()
    else:
        print(
            f"OK — every file path mentioned in a comment across {files_scanned} "
            f"source file(s) ({checked_mentions} checkable mention(s)) exists."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
