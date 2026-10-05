#!/usr/bin/env bash
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# tools/install-hooks.sh
# ======================
#
# Installs MeedyaDL's git hooks into THIS clone. Git never copies hooks
# between clones (a hook is a file inside .git, which is not pushed or
# pulled), so each clone -- yours, a new laptop's, a contributor's -- runs
# this once:
#
#     ./tools/install-hooks.sh
#
# It writes a small pre-push hook that runs tools/hooks/pre-push from the
# repository, so later changes to the checks apply without reinstalling.
# It honours core.hooksPath if one is set. It will not overwrite a
# pre-push hook it did not write itself; it says so and stops instead.

set -eu
top=$(git rev-parse --show-toplevel)
hooks_dir=$(cd "$top" && git rev-parse --git-path hooks)
case "$hooks_dir" in /*) ;; *) hooks_dir="$top/$hooks_dir" ;; esac
mkdir -p "$hooks_dir"
target="$hooks_dir/pre-push"
marker="# Installed by MeedyaDL tools/install-hooks.sh"

if [ -e "$target" ] && ! grep -qF "$marker" "$target"; then
  echo "A pre-push hook already exists at $target and was not written by this script."
  echo "Not overwriting it. Move it aside (or merge it with tools/hooks/pre-push) and run this again."
  exit 1
fi

cat > "$target" <<'HOOK'
#!/usr/bin/env bash
# Installed by MeedyaDL tools/install-hooks.sh
# Runs the repository's own pre-push checks (tools/hooks/pre-push).
exec "$(git rev-parse --show-toplevel)/tools/hooks/pre-push" "$@"
HOOK
chmod +x "$target"
echo "Installed the pre-push hook at $target"
echo "It runs tools/hooks/pre-push before every push in this clone."
