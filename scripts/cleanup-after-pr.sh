#!/usr/bin/env bash
# Copyright (c) 2024-2026 MeedyaSuite. MIT-licensed (see LICENSE).
#
# scripts/cleanup-after-pr.sh
# ===========================
#
# Reclaims disk space by clearing fully-regeneratable dev caches. Designed
# to be invoked AFTER a PR merges (manually, or via the post-merge git hook
# installed by scripts/install-dev-hooks.sh).
#
# WHY post-PR rather than post-commit:
#   A Tauri build produces a 20-40 GB `target/` directory. If you cleared
#   after every commit you'd pay a 5-10 min full rebuild on every iteration.
#   PRs are real milestones (3-5/week, not 30+/day) and after a PR merges
#   you typically pull main/alpha which already partially invalidates
#   target/ anyway. Per-PR cadence is the sweet spot.
#
# WHAT IS CLEARED (in order — biggest wins first):
#   1. src-tauri/target/                       Rust build artefacts (20-40 GB)
#   2. node_modules/                           npm dependencies (~340 MB)
#   3. Vite caches (.vite/, dist/)             Frontend build (~few MB)
#   4. ~/.cargo/registry/{cache,src}/          Cargo download/source caches (~2 GB)
#   5. ~/.cargo/git/checkouts/                 Cargo git-source checkouts
#   6. npm cache (~/.npm/)                     npm content-addressable cache (~2 GB)
#   7. ~/Library/Caches/pip/ (macOS only)      Python pip cache (~500 MB)
#   8. brew cleanup --prune=all (macOS only)   Homebrew downloads & old versions (~1.5 GB)
#
# WHAT IS NOT CLEARED (don't touch):
#   - .git/                       version history is sacred
#   - ~/Library/Caches/com.apple* macOS system caches (managed by OS)
#   - ~/Library/Caches/Mozilla*   browser data
#   - User app caches (Affinity, Edge, Adobe, Mega, etc.) — not dev caches
#   - APFS local snapshots        require sudo, not safe to auto-clear
#
# CONDITIONAL MODE (optional):
#   Pass `--conditional` to only clean if disk free is below the threshold
#   defined by THRESHOLD_GB (default 20 GB). This is the recommended mode
#   for the post-merge git hook — you don't lose incremental compilation
#   when you have headroom.
#
# USAGE:
#   ./scripts/cleanup-after-pr.sh             # always clean
#   ./scripts/cleanup-after-pr.sh --conditional  # only if disk < THRESHOLD_GB
#
# EXIT CODES:
#   0  cleanup ran successfully (or was skipped in conditional mode)
#   1  something failed (printed to stderr)

set -euo pipefail

# ----- Configuration -----
THRESHOLD_GB="${MEEDYADL_CLEANUP_THRESHOLD_GB:-20}"  # override via env var
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ----- Helpers -----
log() { printf "[cleanup-after-pr] %s\n" "$*"; }
err() { printf "[cleanup-after-pr] ERROR: %s\n" "$*" >&2; }

free_gb() {
  # Reads how much free space is left on the disk that holds this repo, and
  # prints it in whole gigabytes. Must work the same way on both macOS and
  # Linux, because the post-merge git hook that calls this with
  # `--conditional` runs on whichever machine the contributor is using.
  #
  # This used to run `df -g /Users`, which only works on macOS: `-g` (report
  # in 1-GB blocks) is a BSD-df option that Linux's GNU df does not have,
  # and `/Users` is a macOS-only path that does not exist on Linux at all.
  # On Linux this printed nothing, and the caller treated "I couldn't read
  # it" as permission to run the FULL clean rather than skip it -- so every
  # `git pull` on a Linux machine deleted the Rust build directory,
  # node_modules, and the shared Cargo/npm caches, because the safe answer
  # ("I don't know, so don't guess") was never wired in; only the unsafe one
  # ("I don't know, so clean anyway") was.
  #
  # `df -Pk` fixes both problems at once: `-P` is the POSIX output format
  # that both BSD df (macOS) and GNU df (Linux) understand identically --
  # one line per filesystem, never wrapped even for a long path -- and `-k`
  # (1024-byte blocks) is the one block-size flag both of them accept, so
  # the parsing below is identical on either OS. Pointing it at "$REPO_ROOT"
  # instead of a hardcoded mount point also means it measures the disk the
  # repo is actually on, which is not necessarily "/Users" even on macOS.
  #
  # Returns 1 (and prints nothing) if the read failed or produced something
  # that is not a plain number, so the caller can tell "zero space" apart
  # from "couldn't tell" and never has to guess.
  local kb
  kb=$(df -Pk "$REPO_ROOT" 2>/dev/null | awk 'NR==2 {print $4}')
  if [[ -z "$kb" ]] || ! [[ "$kb" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  # 1 GB = 1024 * 1024 KB. Bash integer division truncates rather than
  # rounds, matching the truncating behaviour `df -g` had on macOS, so the
  # THRESHOLD_GB comparison below still means the same thing it always did.
  echo $(( kb / 1048576 ))
}

human_size() {
  du -sh "$1" 2>/dev/null | awk '{print $1}' | head -1
}

# ----- Conditional check -----
if [[ "${1:-}" == "--conditional" ]]; then
  if free=$(free_gb); then
    if [[ "$free" -ge "$THRESHOLD_GB" ]]; then
      log "Disk free=${free}GB >= ${THRESHOLD_GB}GB threshold — skipping cleanup"
      exit 0
    else
      log "Disk free=${free}GB < ${THRESHOLD_GB}GB threshold — proceeding with full clean"
    fi
  else
    # We genuinely don't know how much space is free (df failed, or this
    # OS's df prints a shape we don't recognise). The safe answer here is
    # to skip, not to clean -- guessing "must be full clean" is exactly the
    # bug this branch used to have (see free_gb's comment above), and a
    # skipped cleanup costs nothing but disk space, while a wrongly-forced
    # one deletes a build cache the contributor may have wanted to keep.
    # Anyone who does want the clean regardless can still run this script
    # without --conditional.
    log "Could not determine free disk space for $REPO_ROOT — skipping cleanup rather than guessing. Run without --conditional to force a full clean."
    exit 0
  fi
fi

log "Starting full dev-cache cleanup for $REPO_ROOT"
echo

# ----- 1. Rust target/ for this repo -----
if [[ -d "$REPO_ROOT/src-tauri/target" ]]; then
  size=$(human_size "$REPO_ROOT/src-tauri/target")
  log "Clearing $REPO_ROOT/src-tauri/target ($size)"
  rm -rf "$REPO_ROOT/src-tauri/target"
fi

# ----- 2. node_modules for this repo -----
if [[ -d "$REPO_ROOT/node_modules" ]]; then
  size=$(human_size "$REPO_ROOT/node_modules")
  log "Clearing $REPO_ROOT/node_modules ($size) — restore with: npm install"
  rm -rf "$REPO_ROOT/node_modules"
fi

# ----- 3. Vite caches -----
for d in "$REPO_ROOT/.vite" "$REPO_ROOT/dist" "$REPO_ROOT/node_modules/.vite"; do
  if [[ -d "$d" ]]; then
    log "Clearing $d"
    rm -rf "$d"
  fi
done

# ----- 4. Cargo registry caches (shared across all Rust projects) -----
for d in "$HOME/.cargo/registry/cache" "$HOME/.cargo/registry/src" "$HOME/.cargo/git/checkouts"; do
  if [[ -d "$d" ]]; then
    size=$(human_size "$d")
    log "Clearing $d ($size)"
    rm -rf "$d"
  fi
done

# ----- 5. npm cache -----
if command -v npm > /dev/null 2>&1; then
  log "Clearing npm cache"
  npm cache clean --force 2>&1 | tail -2 || true
fi

# ----- 6. pip cache (macOS) -----
if [[ "$OSTYPE" == darwin* ]] && [[ -d "$HOME/Library/Caches/pip" ]]; then
  size=$(human_size "$HOME/Library/Caches/pip")
  log "Clearing pip cache ($size)"
  rm -rf "$HOME/Library/Caches/pip"
fi

# ----- 7. Homebrew cleanup (macOS) -----
if command -v brew > /dev/null 2>&1; then
  log "Running brew cleanup --prune=all"
  brew cleanup --prune=all 2>&1 | tail -2 || true
fi

echo
log "Cleanup complete."
if free=$(free_gb); then
  log "Free disk: ${free}GB"
fi
log "Next \`cargo check\` or \`cargo tauri dev\` will trigger a full rebuild (5-10 min)."
log "Next \`npm run dev\` requires \`npm install\` first."
