#!/usr/bin/env bash
# Copyright (c) 2026 MeedyaSuite. MIT-licensed (see LICENSE).
#
# scripts/install-dev-hooks.sh
# ============================
#
# Installs opt-in git hooks for the MeedyaDL local dev workflow. These hooks
# live in .git/hooks/ (which is NOT committed) so each contributor runs this
# script once per clone to enable them. The script is idempotent — re-running
# it overwrites any existing hooks with the latest versions.
#
# WHAT IT INSTALLS:
#   post-merge   →  Runs scripts/cleanup-after-pr.sh --conditional
#                   Fires when you `git pull` or `git merge` (i.e. after a
#                   PR merges and you sync local main/alpha). Only cleans
#                   when disk free is below threshold (default 20 GB).
#
# USAGE:
#   ./scripts/install-dev-hooks.sh
#
# To override the disk threshold:
#   export MEEDYADL_CLEANUP_THRESHOLD_GB=40
#   # add to ~/.zshrc to make it permanent
#
# To disable the hook later, just remove .git/hooks/post-merge.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
HOOK_FILE="$HOOKS_DIR/post-merge"
CLEANUP_SCRIPT="$REPO_ROOT/scripts/cleanup-after-pr.sh"

if [[ ! -d "$REPO_ROOT/.git" ]]; then
  echo "ERROR: $REPO_ROOT does not look like a git repository" >&2
  exit 1
fi

if [[ ! -f "$CLEANUP_SCRIPT" ]]; then
  echo "ERROR: $CLEANUP_SCRIPT not found" >&2
  exit 1
fi

# Ensure the cleanup script itself is executable
chmod +x "$CLEANUP_SCRIPT"

mkdir -p "$HOOKS_DIR"

cat > "$HOOK_FILE" <<EOF
#!/usr/bin/env bash
# AUTO-INSTALLED by scripts/install-dev-hooks.sh — DO NOT EDIT BY HAND.
# Remove this file (.git/hooks/post-merge) to disable the cleanup hook.
#
# Runs cleanup-after-pr.sh --conditional. Only cleans dev caches when disk
# is under the threshold (default 20 GB; override via MEEDYADL_CLEANUP_THRESHOLD_GB).
set -euo pipefail
exec "$CLEANUP_SCRIPT" --conditional
EOF

chmod +x "$HOOK_FILE"

echo "Installed: $HOOK_FILE"
echo
echo "Next steps:"
echo "  - The hook fires automatically on \`git pull\` and \`git merge\`."
echo "  - It calls $CLEANUP_SCRIPT --conditional"
echo "  - Default threshold: clean only if disk free < 20 GB."
echo "  - Override:  export MEEDYADL_CLEANUP_THRESHOLD_GB=40"
echo "  - Manual:    \`./scripts/cleanup-after-pr.sh\` (always clean, ignores threshold)"
echo "  - Uninstall: \`rm $HOOK_FILE\`"
