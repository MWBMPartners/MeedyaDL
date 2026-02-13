#!/bin/bash
# Copyright (c) 2024-2026 MeedyaDL
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Copyright Year Updater for MeedyaDL
# ======================================
#
# Updates the copyright end-year across all source files in the project
# to the current calendar year. Run at the start of each new year or
# automate in CI to keep copyright notices legally current.
#
# Covers all file types: Rust, TypeScript, CSS, config, markdown, YAML,
# shell scripts, HTML, SVG, LICENSE, and tauri.conf.json.
#
# Platform Note:
#   Auto-detects macOS vs Linux for correct sed in-place syntax.
#
# Usage:
#   ./scripts/update-copyright-year.sh
#
# IMPORTANT: This script excludes itself from bulk find/sed processing
# to avoid corrupting the sed pattern strings in its own body. Its own
# copyright header (line 2) is updated via a targeted line-number sed.

# --- Determine the current year and platform ---
CURRENT_YEAR=$(date +%Y)

echo "Updating copyright year to: $CURRENT_YEAR"

# Detect platform for correct sed in-place syntax
if [[ "$OSTYPE" == "darwin"* ]]; then
    SED_INPLACE=(sed -i '')
else
    SED_INPLACE=(sed -i)
fi

# Build the sed substitution command. The start year (2024) never changes.
# shellcheck disable=SC1117
YEAR_PATTERN='Copyright (c) 2024-[0-9][0-9][0-9][0-9]'
YEAR_REPLACE="Copyright (c) 2024-${CURRENT_YEAR}"
PATTERN="s/${YEAR_PATTERN}/${YEAR_REPLACE}/g"

# --- Rust source files (.rs) ---
find ./src-tauri/src -name "*.rs" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- Rust build script and Cargo.toml ---
for file in src-tauri/build.rs src-tauri/Cargo.toml; do
    [ -f "$file" ] && "${SED_INPLACE[@]}" "$PATTERN" "$file"
done

# --- TypeScript/TSX source files (.ts, .tsx) ---
find ./src \( -name "*.ts" -o -name "*.tsx" \) -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- CSS files (.css) ---
find ./src -name "*.css" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- Root configuration files ---
for file in vite.config.ts vitest.config.ts tailwind.config.js postcss.config.js commitlint.config.js eslint.config.js cliff.toml; do
    [ -f "$file" ] && "${SED_INPLACE[@]}" "$PATTERN" "$file"
done

# --- LICENSE file ---
[ -f "LICENSE" ] && "${SED_INPLACE[@]}" "$PATTERN" LICENSE

# --- tauri.conf.json (bundle.copyright field) ---
[ -f "src-tauri/tauri.conf.json" ] && "${SED_INPLACE[@]}" "$PATTERN" src-tauri/tauri.conf.json

# --- Help documentation (.md) ---
find ./help -name "*.md" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- Project root markdown files ---
for file in README.md Project_Plan.md PROJECT_STATUS.md CHANGELOG.md; do
    [ -f "$file" ] && "${SED_INPLACE[@]}" "$PATTERN" "$file"
done

# --- GitHub Actions workflow files (.yml) ---
find ./.github/workflows -name "*.yml" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- Scripts (excluding this file to prevent self-corruption) ---
SELF_NAME="update-copyright-year.sh"
find ./scripts \( -name "*.sh" -o -name "*.mjs" -o -name "*.js" \) ! -name "$SELF_NAME" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# Update only line 2 of this script (the copyright header) without touching the body
"${SED_INPLACE[@]}" "2s/${YEAR_PATTERN}/${YEAR_REPLACE}/" "./scripts/${SELF_NAME}"

# --- HTML entry point and other root files ---
for file in index.html .gitignore; do
    [ -f "$file" ] && "${SED_INPLACE[@]}" "$PATTERN" "$file"
done

# --- SVG assets ---
find ./assets -name "*.svg" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

# --- Claude context files ---
[ -d ".claude" ] && find ./.claude -name "*.md" -exec "${SED_INPLACE[@]}" "$PATTERN" {} \;

echo "Copyright year updated to $CURRENT_YEAR in all source files."
