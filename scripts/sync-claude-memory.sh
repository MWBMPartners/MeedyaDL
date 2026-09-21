#!/bin/sh
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Sync the repo's shared Claude memory files into the developer's local
# Claude Code memory store.
#
# Claude Code reads per-project memory from `~/.claude/projects/<sanitised-
# repo-path>/memory/`, where the sanitised path is the absolute repo path
# with every character that is not a letter or digit replaced by `-` (see
# the note where it is computed below). That location is per-user and
# can't be loaded directly from inside the repo, so every contributor
# bootstraps it once with this script (and re-runs it after `git pull`
# whenever someone updates a shared memory file).
#
# Behaviour:
#   1. Computes the sanitised path for this clone.
#   2. Copies every `*.md` from `.claude/memory/` (except README.md and
#      MEMORY.md) into the dev's local memory dir.
#   3. Merges the shared `MEMORY.md` hooks into the dev's personal
#      `MEMORY.md` between sentinel markers, preserving any personal
#      entries above/below the marker block.
#
# The copy is intentionally one-way (repo → home). Local edits in your
# home memory don't propagate back. If you want a change shared, edit
# the file under `.claude/memory/` in the repo and commit.

set -eu

# ----------------------------------------------------------------
# Resolve repo root regardless of where the script is invoked from
# ----------------------------------------------------------------
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# `pwd -P` gives the real path with any symbolic links resolved (for example
# /private/tmp rather than /tmp on a Mac). Claude Code names its memory
# folder from the real path, so the plain `pwd` used before could produce a
# name it never reads when the clone sits behind a link.
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
SHARED_DIR="$REPO_ROOT/.claude/memory"

if [ ! -d "$SHARED_DIR" ]; then
    printf 'error: %s does not exist — are you in the MeedyaDL repo?\n' "$SHARED_DIR" >&2
    exit 1
fi

# ----------------------------------------------------------------
# Keep .OpenAI/CONTEXT.md in sync with .claude/CLAUDE.md
# ----------------------------------------------------------------
# .OpenAI/CONTEXT.md exists so a different AI coding tool sees the same
# project instructions Claude Code does. It is meant to be a straight copy
# of .claude/CLAUDE.md, but nothing enforced that, and the two drifted
# badly out of sync in practice (including .OpenAI/CONTEXT.md telling
# people to run `xattr -cr`, which this project explicitly says not to
# do). This step re-copies CLAUDE.md over CONTEXT.md every time this
# script runs (i.e. after every `git pull`, per the convention above) so
# the two files cannot drift apart again. This is a within-repo copy
# (unlike the home-directory sync below) — if you want to change what
# either tool sees, edit .claude/CLAUDE.md and commit; do not hand-edit
# .OpenAI/CONTEXT.md, it will just be overwritten next run.
CLAUDE_MD="$REPO_ROOT/.claude/CLAUDE.md"
OPENAI_CONTEXT="$REPO_ROOT/.OpenAI/CONTEXT.md"
if [ -f "$CLAUDE_MD" ]; then
    mkdir -p "$(dirname "$OPENAI_CONTEXT")"
    cp "$CLAUDE_MD" "$OPENAI_CONTEXT"
    printf 'Refreshed %s from .claude/CLAUDE.md\n' ".OpenAI/CONTEXT.md"
fi

# ----------------------------------------------------------------
# Compute the sanitised path Claude Code uses for this repo
# ----------------------------------------------------------------
# Rule observed in Claude Code: take the absolute repo path and replace
# EVERY character that is not a plain letter or digit with `-` — slashes,
# dots, spaces, `&`, underscores, brackets, all of them. The leading `/`
# becomes a leading `-`. Example:
#   /Users/someone/Projects/Work & Play/MeedyaDL
#   → -Users-someone-Projects-Work---Play-MeedyaDL
#
# This used to replace only `/`, `.` and space. That looked right for a
# simple path, but on a clone under a folder such as "Coding & Development"
# it kept the `&`, so every run copied the memory into a folder Claude Code
# never reads — the real memory folder stayed empty, and nothing said so
# (found 2026-09-21, #1198).
#
# What this cannot do, said plainly:
#   - Claude Code shortens very long paths (roughly 200+ characters) and
#     adds a hash to the end. This script does not copy that rule, because
#     it is not documented.
#   - Letters outside plain English (é, ß, an emoji) may not come out the
#     same: `sed` turns each such character into one `-`, while Claude Code
#     turns some of them (an emoji, for instance) into two.
# The check below catches both cases by warning, rather than by guessing.
SANITISED=$(printf '%s' "$REPO_ROOT" | sed 's/[^A-Za-z0-9]/-/g')
PROJECT_DIR="$HOME/.claude/projects/$SANITISED"
USER_DIR="$PROJECT_DIR/memory"

# Check that this is a folder Claude Code really uses, rather than trusting
# the naming rule above. Claude Code keeps one `<id>.jsonl` file per session
# in its project folder, so a folder with none is either one Claude Code has
# never used or one whose old sessions it has since cleaned up (it deletes
# them after about a month by default). The copy still goes ahead — on a
# brand-new clone that has not been opened in Claude Code yet, this is
# expected — but it says so out loud: this warning would have caught the
# `&` mistake described above on its very first run. It is a warning, not a
# stop, because the only thing at stake is where some copies of repo files
# land.
if ! ls "$PROJECT_DIR"/*.jsonl >/dev/null 2>&1; then
    printf 'Warning: %s\n' "$PROJECT_DIR" >&2
    printf '  has no Claude Code session files in it. Either Claude Code has not\n' >&2
    printf '  been opened in this clone yet or not for a long while (fine: open\n' >&2
    printf '  it once, then re-run), or Claude Code names its folder differently\n' >&2
    printf '  from this script, in which case the memory is being copied somewhere\n' >&2
    printf '  it will never be read. Look in ~/.claude/projects/ for the folder\n' >&2
    printf '  that matches this clone.\n' >&2
fi

# Point out the folder an older version of this script may have filled by
# mistake (it only turned `/`, `.` and spaces into `-`). Normally it holds
# only stale copies of repo files, but this script cannot be sure nobody
# added a note of their own there, so it only points the folder out and
# never deletes anything itself.
OLD_SANITISED=$(printf '%s' "$REPO_ROOT" | sed 's![ /.]!-!g')
OLD_PROJECT_DIR="$HOME/.claude/projects/$OLD_SANITISED"
# Two safety points. It only suggests deleting when the folder holds no
# Claude Code session files: if it has some, Claude Code really uses it, and
# it is not a leftover. And the suggested command wraps the path in single
# quotes, so pasting it can never run anything hidden in a folder name (a
# double-quoted path would still expand `$(...)` or backticks); a path that
# itself contains a single quote gets no ready-made command at all.
if [ "$OLD_SANITISED" != "$SANITISED" ] && [ -d "$OLD_PROJECT_DIR" ] \
    && ! ls "$OLD_PROJECT_DIR"/*.jsonl >/dev/null 2>&1; then
    printf 'Note: an older version of this script copied memory into\n' >&2
    printf '  %s\n' "$OLD_PROJECT_DIR" >&2
    printf '  which Claude Code never reads. Normally it holds only stale copies of\n' >&2
    printf '  repo files. Look inside first; if nothing there is yours, delete it' >&2
    case "$OLD_PROJECT_DIR" in
        *"'"*) printf ' by hand.\n' >&2 ;;
        *)     printf ':\n  rm -rf '"'"'%s'"'"'\n' "$OLD_PROJECT_DIR" >&2 ;;
    esac
fi

mkdir -p "$USER_DIR"

# ----------------------------------------------------------------
# Copy shared memory files (skip README.md and MEMORY.md)
# ----------------------------------------------------------------
COPIED=0
for src in "$SHARED_DIR"/*.md; do
    # `*.md` literal if no matches — guard
    [ -e "$src" ] || continue
    name=$(basename "$src")
    case "$name" in
        README.md|MEMORY.md) continue ;;
    esac
    cp "$src" "$USER_DIR/$name"
    COPIED=$((COPIED + 1))
done

# ----------------------------------------------------------------
# Merge MEMORY.md hooks under sentinel markers (idempotent)
# ----------------------------------------------------------------
SHARED_INDEX="$SHARED_DIR/MEMORY.md"
USER_INDEX="$USER_DIR/MEMORY.md"
START_MARKER="<!-- claude-memory:shared:start (managed by scripts/sync-claude-memory.sh) -->"
END_MARKER="<!-- claude-memory:shared:end -->"

if [ ! -f "$SHARED_INDEX" ]; then
    printf 'warning: %s missing — only the individual memory files were synced.\n' \
        "$SHARED_INDEX" >&2
    printf 'Synced %d memory file(s) to %s\n' "$COPIED" "$USER_DIR"
    exit 0
fi

# Build the new shared block
TMP_BLOCK=$(mktemp)
trap 'rm -f "$TMP_BLOCK" "$TMP_BLOCK.merged" "$TMP_BLOCK.list"' EXIT
{
    echo "$START_MARKER"
    cat "$SHARED_INDEX"
    echo "$END_MARKER"
} > "$TMP_BLOCK"

if [ ! -f "$USER_INDEX" ]; then
    # Fresh install — write block as-is
    cp "$TMP_BLOCK" "$USER_INDEX"
else
    # Existing personal index. Two cleanups before appending the fresh
    # shared block:
    #
    #   1. Strip any prior shared block (between sentinels) so re-runs
    #      stay idempotent.
    #   2. Strip any *outside-the-block* lines that link to one of the
    #      shared files. Without this, contributors who ran a previous
    #      version of this workflow (or who hand-edited their personal
    #      index to point at a shared file) end up with duplicated hooks
    #      in their personal MEMORY.md every time they sync. The shared
    #      block is the canonical reference for shared files; personal
    #      hooks should only point at user_*.md / feedback_*.md.
    #
    # Write the list of shared filenames to a temp file (one per line).
    # We pass it to awk via a separate file rather than `-v filelist=…`
    # because BSD awk (macOS) rejects multi-line `-v` values, and we
    # avoid `case` inside `$()` because POSIX-mode bash 3.x on macOS
    # misparses `;;` there. Plain `[ ]` tests + a temp file are
    # universally portable.
    SHARED_LIST="$TMP_BLOCK.list"
    : > "$SHARED_LIST"
    for f in "$SHARED_DIR"/*.md; do
        [ -e "$f" ] || continue
        name=$(basename "$f")
        if [ "$name" != "README.md" ] && [ "$name" != "MEMORY.md" ]; then
            printf '%s\n' "$name" >> "$SHARED_LIST"
        fi
    done

    awk -v start="$START_MARKER" -v end="$END_MARKER" '
        # First file: build the shared-filenames set.
        NR == FNR { if ($0 != "") shared[$0] = 1; next }
        # Second file (the user index): filter.
        $0 == start { in_block = 1; next }
        $0 == end   { in_block = 0; next }
        in_block    { next }
        {
            # Strip outside-block lines that reference a shared file via
            # a Markdown link target like `(filename.md)`, even if the
            # surrounding hook text differs from ours.
            for (f in shared) {
                if (index($0, "(" f ")") > 0) next
            }
            print
        }
    ' "$SHARED_LIST" "$USER_INDEX" > "$TMP_BLOCK.merged"
    cat "$TMP_BLOCK" >> "$TMP_BLOCK.merged"
    mv "$TMP_BLOCK.merged" "$USER_INDEX"
fi

printf 'Synced %d memory file(s) and refreshed shared index hooks at:\n  %s\n' \
    "$COPIED" "$USER_DIR"
