#!/usr/bin/env bash
# Copyright (c) 2026 MeedyaSuite. MIT-licensed (see LICENSE).
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
  # macOS `df -g` reports in 1-GB blocks. Other Unixes vary; if we're not
  # on macOS the worst that happens is df may not have `-g` and we get
  # `--conditional` defaulting to "always clean" which is still safe.
  df -g /Users 2>/dev/null | awk 'NR==2 {print $4}'
}

human_size() {
  du -sh "$1" 2>/dev/null | awk '{print $1}' | head -1
}

# ----- Conditional check -----
if [[ "${1:-}" == "--conditional" ]]; then
  free=$(free_gb)
  if [[ -z "$free" ]] || ! [[ "$free" =~ ^[0-9]+$ ]]; then
    log "Could not determine free disk space; proceeding with full clean"
  elif [[ "$free" -ge "$THRESHOLD_GB" ]]; then
    log "Disk free=${free}GB >= ${THRESHOLD_GB}GB threshold — skipping cleanup"
    exit 0
  else
    log "Disk free=${free}GB < ${THRESHOLD_GB}GB threshold — proceeding with full clean"
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
if df -g /Users > /dev/null 2>&1; then
  log "Free disk: $(free_gb)GB"
fi
log "Next \`cargo check\` or \`cargo tauri dev\` will trigger a full rebuild (5-10 min)."
log "Next \`npm run dev\` requires \`npm install\` first."
