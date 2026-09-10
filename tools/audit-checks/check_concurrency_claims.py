#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Concurrency-claim check.

A comment that says two things happen "in parallel" or "concurrently" is
making a claim about HOW the code runs, not just what it does. That claim
is easy to write and easy to leave behind: a future built as two futures
awaited one after another (`let a = x().await; let b = y().await;`) does
nothing at the same time no matter what a nearby comment says -- in Rust,
a future does not run until it is polled, so building two of them and then
awaiting each in turn is sequential, full stop, regardless of the comment
sitting above it. A 2026-09 audit of this repository found five such
comments: two describing a preflight-check function that awaits each check
in turn, one describing a dependency-status loop that checks one tool at a
time with a per-tool timeout, one describing a cross-platform cover-art
picker that asks one service at a time, and one where the futures actually
were built together but then awaited one by one immediately below -- so
even the "we built them together" half of the claim didn't make the
"they run together" half true.

This script looks for a comment containing the word "parallel" or
"concurrently" and checks whether the surrounding code actually contains
one of the mechanisms that would make that true: `join!`, `join_all`,
`try_join`, `spawn` (covers `tokio::spawn`, `tauri::async_runtime::spawn`,
`std::thread::spawn`, and `spawn_blocking`), `Promise.all`, or
`allSettled`. If none of those appear anywhere in the block of code the
comment sits inside (or, for a doc comment, in the function it documents),
this is reported as a finding.

## This is a rough heuristic, not a prover -- read this before adding a finding to EXCEPTIONS

This script does not parse Rust, TypeScript, or JavaScript. It cannot tell
whether the mechanism it's looking for is REALLY what makes the comment's
claim true, only whether one of those six words/symbols appears somewhere
in a text region it has approximated as "the surrounding function". Three
concrete ways that approximation can be wrong, all in the false-positive
direction (missing a real mechanism, not inventing a fake one):

  1. The comment's own function calls a HELPER function that does the
     actual `spawn`/`join!` -- the mechanism is real, it's just not
     textually present in the function the comment sits in.
  2. The comment is describing concurrency at a level bigger than any
     single function -- "N concurrent downloads across the whole queue",
     enforced by a semaphore/counter that no single function's text would
     show.
  3. The "surrounding function" itself is approximate: this script finds
     it by counting matched `{`/`}` pairs backward from the comment (or,
     for a doc comment immediately above a function signature, forward to
     that function's own opening brace) -- it has no real notion of what
     a "function" is versus an `if` block, a `match` arm, or a `class`.
     A comment with no enclosing brace pair at all (a module-level `//!`
     doc comment at the top of a file, before any code) falls back to
     scanning the ENTIRE file, which is deliberately the most forgiving
     choice available, not a precise one.

None of that makes the check useless -- it made exactly the five findings
described above, correctly, with nothing else in this repository falsely
caught alongside them. But when it flags something and the comment is
actually correct because the real mechanism lives somewhere this script
can't see, the fix is an entry in EXCEPTIONS with the reason, not a
rewording of a true comment just to satisfy a script.

Exit code:
  0 -- no findings, OR findings without --strict
  1 -- at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_concurrency_claims.py [--strict]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

SCAN_EXTENSIONS = {".rs", ".ts", ".tsx", ".js", ".mjs"}
SKIP_DIR_PARTS = {"node_modules", "target", "dist", "build", ".git", "__pycache__"}

# The literal words this check looks for -- deliberately just these two,
# not the wider "concurrent" family (which also covers phrases like "N
# concurrent downloads", a capacity/limit concept unrelated to whether any
# one function's own code executes two things at the same time).
CLAIM_RE = re.compile(r"\b(parallel|concurrently)\b", re.IGNORECASE)

# Mechanisms that back up a "runs in parallel / concurrently" claim. Plain
# substrings, not word-bounded regexes -- "spawn" deliberately matches
# tokio::spawn, tauri::async_runtime::spawn, thread::spawn, and
# spawn_blocking alike, since all of them are genuine concurrency starts.
REQUIRED_TOKENS = ("join!", "join_all", "try_join", "spawn", "Promise.all", "allSettled")

# Mentioned claim text (the single comment LINE the match is on, trimmed
# of leading/trailing whitespace) -> reason the wording is correct even
# though this script can't see the mechanism nearby. See the module
# docstring's "rough heuristic" section for when to add one of these
# instead of rewording a true comment. Grouped by which of the three
# reasons in the docstring applies.
EXCEPTIONS: dict[str, str] = {
    # --- Reason 1: cargo's own test-runner parallelism. Multiple test
    # functions/modules genuinely run at the same time (cargo test's own
    # scheduler does this), but nothing in THIS repository's source spawns
    # or joins them -- that mechanism belongs to the test harness, not to
    # any function this script can point at. ---
    "/// can bump in parallel — racy by design, not useful here.": (
        "Describes other #[test] functions in the same suite running at "
        "the same time, via cargo test's own scheduler -- not a mechanism "
        "in this file's own code."
    ),
    "/// process-global and shared with other parallel tests, so we": (
        "Same as above: cargo test's own parallelism between test "
        "functions, not a mechanism this file's code starts itself."
    ),
    "// `capability_cache_test_lock` so parallel tests in other modules": (
        "Same as above: the lock exists BECAUSE cargo runs tests in "
        "parallel, but the parallelism itself is the test harness's, not "
        "this file's."
    ),
    "/// guard's lifetime so parallel tests in OTHER modules can't flip": (
        "Same as above: cargo test's own parallelism."
    ),
    "/// parallel tests in different files (e.g.,": (
        "Same as above: cargo test's own parallelism between test files."
    ),
    # --- Reason 2/3: the real mechanism is a genuine tokio::spawn or
    # tokio::join! elsewhere in the codebase (a different function, or a
    # different file entirely) -- this script only looks at the one
    # function/block the comment sits in, so a cross-referenced mechanism
    # a few functions or files away is invisible to it by design. Each
    # entry names where the real mechanism actually lives. ---
    "/// finished; the parallel companion GAMDL run, oblivious to that": (
        "The companion GAMDL subprocess this describes is started by "
        "`spawn_companion_downloads()` in companions.rs (a real "
        "tokio::spawn) -- a different function than the one this comment "
        "sits in, which only builds the folder-name string."
    ),
    "// parallel post #779 Option 2). User": (
        "Sits inside the AcoustID progress-callback closure passed INTO "
        "process_acoustid_for_directory(); the tokio::join! that actually "
        "runs AcoustID/MusicBrainz/ReplayGain together lives outside all "
        "three task closures, in the code that calls them -- see the "
        "`tokio::join!(acoustid_task, musicbrainz_lookup_task, "
        "replaygain_task)` call further down in this same file."
    ),
    "// be on Music Video Discovery in parallel — see #706 / #712 for the": (
        "progress_stages.rs is a shared label/weight registry, not where "
        "the enrichment task and companion task are actually spawned as "
        "separate tokio tasks -- that happens in download_queue.rs, which "
        "this comment already points at via #706/#712."
    ),
    "/// (chromaprint, FFmpeg ebur128) can run truly in parallel while the": (
        "file_locks.rs defines the lock primitive; the tokio::join! that "
        "actually runs AcoustID and ReplayGain at the same time lives in "
        "download_queue/processing.rs (#779 Option 2), not in this file."
    ),
    "* 5. Dependency checks run in parallel (Python, GAMDL, tools)": (
        "A numbered step in App.tsx's own high-level startup summary, "
        "describing behaviour implemented in dependencyStore.ts's "
        "`checkAll()`, which genuinely calls Promise.all() -- see the "
        "next three entries below, all pointing at the same function."
    ),
    "* `checkAll` runs parallel IPC calls to check Python, GAMDL, and tool": (
        "Describes dependencyStore.ts's checkAll(), which does call "
        "Promise.all() -- just not in App.tsx, which only calls checkAll()."
    ),
    "*    Runs checks in parallel for faster startup.": (
        "Same as above: describes dependencyStore.ts's checkAll(), called "
        "from this effect but implemented (and Promise.all()-using) "
        "elsewhere."
    ),
    "/* Step 3: Check all dependency statuses in parallel via IPC */": (
        "Same as above: describes dependencyStore.ts's checkAll()."
    ),
    "* Check all dependencies in parallel: Python, GAMDL, and external tools.": (
        "This is the doc comment on the `checkAll` METHOD SIGNATURE inside "
        "the store's TypeScript interface (a type declaration ending in "
        "`;`, with no body at all) -- the real implementation, a few "
        "hundred lines further down in the same file, has its own doc "
        "comment right above `checkAll: async () => {` and DOES contain "
        "`await Promise.all([...])`. This script can't see across that "
        "gap from an interface signature to its separate implementation."
    ),
    "// Single bulk IPC (#835). Replaces the N-parallel `retryDownload`": (
        "Describes REMOVED, historical code (the old per-item retry flow "
        "this PR replaced), not the current function -- there is nothing "
        "left in the codebase for this script to find a mechanism in, "
        "because the whole point of the change was removing it."
    ),
    "* On mount: check the managed Python and, in parallel, detect any compatible": (
        "The three calls right below this comment (checkPython(); "
        "detectSystemPythons(); diagnosePythonVenv();) are fired one "
        "after another with no `await` between them. In JavaScript that "
        "genuinely starts all three running at the same time -- an async "
        "function begins executing immediately when called, and only "
        "yields back to the caller at its first `await` -- so, unlike "
        "Rust, no join!/spawn/Promise.all() is needed to make \"in "
        "parallel\" true here. This script's token list doesn't recognise "
        "the bare-unawaited-calls pattern."
    ),
    # --- Metaphorical use of "parallel": describes something ADDITIONAL
    # or ALONGSIDE in a conceptual sense (a second, similar hook; an
    # independent feature track), not "runs at the same moment in time".
    # English genuinely overloads this word both ways, and a rough
    # heuristic can't tell which sense is meant. ---
    "* than introducing a parallel `useSpotifySettingsField` hook (which": (
        "\"a parallel hook\" means an additional, similarly-shaped hook -- "
        "a code-organisation comparison, not a claim about runtime timing."
    ),
    "//     (Phase 5c #717 already provides this signal as a parallel rail.)": (
        "\"a parallel rail\" means an independent, alternative feature "
        "track -- a roadmap/scope comparison, not a claim about runtime "
        "timing."
    ),
    # --- Infrastructure outside this codebase's own process (GitHub
    # Actions' own job scheduler), which this repository's code neither
    # starts nor could observe via join!/spawn. ---
    "/// This catches the race condition where parallel CI builds each overwrite": (
        "Describes GitHub Actions running multiple platform build jobs at "
        "the same time -- CI infrastructure this repository's own code "
        "does not spawn and has no join!/spawn call for."
    ),
    "// parallel platform builds each overwrite `latest.json` — the last build to": (
        "Same as above: GitHub Actions' own job scheduler, not this "
        "repository's code."
    ),
    # --- Describes user behaviour (a person clicking twice), not code
    # concurrency. ---
    '/// (already removed by a parallel click) from "guard violation".': (
        "\"a parallel click\" describes a user clicking twice in quick "
        "succession -- human behaviour the UI has to tolerate, not a "
        "claim about this code running two things at once."
    ),
}


def iter_source_files():
    for ext in SCAN_EXTENSIONS:
        for f in sorted(REPO_ROOT.rglob(f"*{ext}")):
            rel = f.relative_to(REPO_ROOT)
            if SKIP_DIR_PARTS & set(rel.parts):
                continue
            yield f, rel


def extract_comment_spans(text: str) -> list[tuple[int, int]]:
    """Returns (start, end) character-offset spans of every `//...` and
    `/* ... */` comment in `text`. Same deliberately simple approach as
    check_comment_paths.py's extract_comment_spans -- does not distinguish
    a `//` inside a string literal from a real comment start."""
    spans: list[tuple[int, int]] = []
    n = len(text)
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


def build_skeleton(text: str) -> str:
    """A same-length copy of `text` with every comment and string-literal
    character replaced by a space (newlines kept, so line numbers stay
    aligned). Brace-counting over this skeleton instead of the raw text
    means a `{` or `}` written inside a comment or a string (an example
    code snippet, a JSON blob) never throws off the block boundaries this
    script computes -- only braces that are real, structural code count."""
    n = len(text)
    out = list(text)
    i = 0
    in_line_comment = False
    in_block_comment = False
    in_str: str | None = None  # the quote character we're inside, if any
    while i < n:
        c = text[i]
        if in_line_comment:
            if c == "\n":
                in_line_comment = False
            else:
                out[i] = " "
            i += 1
            continue
        if in_block_comment:
            if text[i : i + 2] == "*/":
                out[i] = out[i + 1] = " "
                i += 2
                in_block_comment = False
                continue
            out[i] = c if c == "\n" else " "
            i += 1
            continue
        if in_str:
            out[i] = " "
            if c == "\\" and i + 1 < n:
                out[i + 1] = " "
                i += 2
                continue
            if c == in_str:
                in_str = None
            i += 1
            continue
        if text[i : i + 2] == "//":
            in_line_comment = True
            out[i] = out[i + 1] = " "
            i += 2
            continue
        if text[i : i + 2] == "/*":
            in_block_comment = True
            out[i] = out[i + 1] = " "
            i += 2
            continue
        if c in ("\"", "'", "`"):
            in_str = c
            out[i] = " "
            i += 1
            continue
        i += 1
    return "".join(out)


def matching_close(skeleton: str, open_idx: int) -> int:
    """Index of the `}` that closes the `{` at `open_idx`, by depth
    counting. Returns len(skeleton) (i.e. "runs to end of file") if the
    braces are unbalanced -- should not happen on real source, but a
    heuristic tool should never crash on a file it misjudges."""
    depth = 0
    for i in range(open_idx, len(skeleton)):
        if skeleton[i] == "{":
            depth += 1
        elif skeleton[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return len(skeleton)


def forward_attached_block(skeleton: str, start: int, limit: int = 600) -> tuple[int, int] | None:
    """If `start` (the end of a comment) is immediately followed -- allowing
    a function signature's own parameter list, but nothing that looks like
    a separate statement or struct/interface field -- by a `{`, returns
    that block's (open, close) span. This is what lets a doc comment
    (`///`, `/** */`) sitting directly above `pub async fn foo(...) {`
    count that function's own body as "the surrounding function", even
    though the doc comment itself sits textually BEFORE the opening brace.

    Depth-tracks only `(`/`)`/`[`/`]` (not `<`/`>`) while scanning forward,
    so a parameter list's internal commas don't trip the "this is a
    separate statement" check below; a top-level `;` or `,` before any
    `{` means this comment is attached to something else (a statement, a
    struct/interface field) and there is no function body to find."""
    end = min(len(skeleton), start + limit)
    depth = 0
    i = start
    while i < end:
        c = skeleton[i]
        if c in "([":
            depth += 1
        elif c in ")]":
            if depth > 0:
                depth -= 1
        elif c == "{" and depth == 0:
            return (i, matching_close(skeleton, i))
        elif c in ";," and depth == 0:
            return None
        i += 1
    return None


def enclosing_block(skeleton: str, pos: int) -> tuple[int, int] | None:
    """The innermost `{...}` block that textually contains `pos`, found by
    a simple open-brace stack scanned from the start of the file. Returns
    None if `pos` sits at brace depth 0 (e.g. a module-level comment
    before any code) -- callers fall back to the whole file in that case."""
    stack: list[int] = []
    for i, c in enumerate(skeleton[:pos]):
        if c == "{":
            stack.append(i)
        elif c == "}" and stack:
            stack.pop()
    if not stack:
        return None
    start = stack[-1]
    return (start, matching_close(skeleton, start))


def line_no_at(text: str, idx: int) -> int:
    return text[:idx].count("\n") + 1


def check() -> int:
    findings: list[tuple[str, int, str]] = []
    claims_seen = 0
    files_scanned = 0

    for f, rel in iter_source_files():
        try:
            text = f.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        files_scanned += 1
        skeleton = build_skeleton(text)
        rel_str = str(rel).replace("\\", "/")

        for start, end in extract_comment_spans(text):
            comment_text = text[start:end]
            m = CLAIM_RE.search(comment_text)
            if not m:
                continue
            claims_seen += 1

            region = forward_attached_block(skeleton, end)
            if region is None:
                region = enclosing_block(skeleton, start)
            if region is None:
                region = (0, len(skeleton))
            region_text = skeleton[region[0] : region[1]]

            if any(tok in region_text for tok in REQUIRED_TOKENS):
                continue

            # Report just the single line the match is on, not the whole
            # (possibly many-line) comment span -- a block-comment claim
            # buried on line 12 of a 20-line JSDoc block should not print
            # all 20 lines as its "snippet".
            claim_line_start = comment_text.rfind("\n", 0, m.start()) + 1
            claim_line_end = comment_text.find("\n", m.end())
            if claim_line_end == -1:
                claim_line_end = len(comment_text)
            claim_line = comment_text[claim_line_start:claim_line_end].strip()

            # A claim explicitly negated ("not in parallel", "nothing
            # running ... in parallel", "not run concurrently") is not a
            # concurrency claim at all -- it's the comment correctly
            # stating the opposite, exactly the wording this script's own
            # fixes elsewhere in this repository now use. Checked against
            # a WINDOW around the match, spanning outward across the
            # comment's own line-wrap boundaries (a `/** ... */` or `///`
            # block often wraps one sentence across several `* `-prefixed
            # lines, so the negating word and "parallel" can end up on
            # different physical lines of the same sentence).
            window_start = max(0, m.start() - 100)
            window_end = min(len(comment_text), m.end() + 40)
            window = comment_text[window_start:window_end]
            if re.search(
                r"\b(not|never|nothing|none|without)\b[^.;]{0,90}\b(parallel|concurrently)\b",
                window,
                re.IGNORECASE,
            ):
                continue
            if re.search(
                r"\b(parallel|concurrently)\b[^.;]{0,15}\b(not|never)\b",
                window,
                re.IGNORECASE,
            ):
                continue

            if claim_line in EXCEPTIONS:
                continue

            line_no = line_no_at(text, start + m.start())
            findings.append((rel_str, line_no, claim_line[:160]))

    print(f"Source files scanned        : {files_scanned}")
    print(f"'parallel'/'concurrently' comments checked: {claims_seen}")
    print()

    if findings:
        print(
            "### Comment claims work happens \"in parallel\" / \"concurrently\", "
            "but no join!/join_all/try_join/spawn/Promise.all/allSettled found nearby\n"
        )
        for rel_str, line_no, snippet in sorted(findings, key=lambda t: (t[0], t[1])):
            print(
                f"  • {rel_str}:{line_no} — \"{snippet}\" — the surrounding code has "
                f"none of join!/join_all/try_join/spawn/Promise.all/allSettled. Either "
                f"the work genuinely runs one after another and the comment should say "
                f"so, or the real mechanism lives somewhere this rough heuristic can't "
                f"see -- see the module docstring before adding an exception."
            )
        print()
    else:
        print(
            f"OK — every 'parallel'/'concurrently' comment across {files_scanned} "
            f"source file(s) ({claims_seen} checked) has a concurrency mechanism nearby."
        )

    if findings and "--strict" in sys.argv:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check())
