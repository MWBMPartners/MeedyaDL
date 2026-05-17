#!/bin/sh
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Sync the repo's shared Claude memory files into the developer's local
# Claude Code memory store.
#
# Claude Code reads per-project memory from `~/.claude/projects/<sanitised-
# repo-path>/memory/`, where the sanitised path is the absolute repo path
# with `/`, `.`, and ` ` replaced by `-`. That location is per-user and
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
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
SHARED_DIR="$REPO_ROOT/.claude/memory"

if [ ! -d "$SHARED_DIR" ]; then
    printf 'error: %s does not exist — are you in the MeedyaDL repo?\n' "$SHARED_DIR" >&2
    exit 1
fi

# ----------------------------------------------------------------
# Compute the sanitised path Claude Code uses for this repo
# ----------------------------------------------------------------
# Rule observed in Claude Code: take the absolute repo path and replace
# every `/`, `.`, and ` ` (space) with `-`. The leading `/` becomes a
# leading `-`. Example:
#   /Users/lance.manasse/Projects/MeedyaDL
#   → -Users-lance-manasse-Projects-MeedyaDL
SANITISED=$(printf '%s' "$REPO_ROOT" | sed 's![ /.]!-!g')
USER_DIR="$HOME/.claude/projects/$SANITISED/memory"

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
