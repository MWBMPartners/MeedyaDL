#!/bin/bash
# Copyright (c) 2024-2026 MWBM Partners Ltd
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Copyright Year Updater for gamdl-GUI
# ======================================
#
# This script updates the copyright end-year across all source files in the project
# to the current calendar year. It should be run at the start of each new year
# (or automated in CI) to keep copyright notices legally current.
#
# The script searches for the pattern "Copyright (c) 2024-XXXX" (where XXXX is any
# 4-digit year) and replaces the end year with the current system year. The start
# year (2024) is the project's inception year and never changes.
#
# File types updated:
#   1. Rust source files     (.rs)  -- in src-tauri/src/
#   2. TypeScript/TSX files  (.ts, .tsx) -- in src/
#   3. CSS files             (.css) -- in src/
#   4. Config files          (vite.config.ts, tailwind.config.js, etc.) -- in project root
#   5. LICENSE file          -- in project root
#   6. tauri.conf.json       -- contains bundle.copyright string
#
# Platform Note:
#   This script uses macOS-style `sed -i ''` (empty string for backup extension).
#   On Linux, change `sed -i ''` to `sed -i` (no argument after -i) or use
#   `sed -i.bak` and remove .bak files afterward.
#
# Usage:
#   ./scripts/update-copyright-year.sh
#
# Example transformations:
#   "Copyright (c) 2024-2025 MWBM Partners Ltd"  -->  "Copyright (c) 2024-2027 MWBM Partners Ltd"
#   "Copyright (c) 2024-2026 MWBM Partners Ltd"  -->  "Copyright (c) 2024-2027 MWBM Partners Ltd"
#
# Related files:
#   - All files with copyright headers (see the patterns below)
#   - src-tauri/tauri.conf.json -- bundle.copyright field
#   - LICENSE                    -- Project license file

# --- Step 1: Determine the current year ---
# Uses the `date` command to get the 4-digit year (e.g., "2026")
CURRENT_YEAR=$(date +%Y)

echo "Updating copyright year to: $CURRENT_YEAR"

# --- Step 2: Update Rust source files (.rs) ---
# Searches recursively in src-tauri/src/ for all .rs files.
# The regex `2024-[0-9]*` matches "2024-" followed by any number of digits.
# The `g` flag replaces all occurrences per line (in case of multiple matches).
# `-i ''` performs in-place editing with no backup file (macOS sed syntax).
find ./src-tauri/src -name "*.rs" -exec \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" {} \;

# --- Step 3: Update TypeScript/TSX source files (.ts, .tsx) ---
# Uses `find` with `-o` (OR) to match both .ts and .tsx extensions.
# Pipes results to `xargs` for batch processing with sed.
find ./src -name "*.ts" -o -name "*.tsx" | xargs \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g"

# --- Step 4: Update CSS files (.css) ---
# Covers globals.css and all theme CSS files in src/styles/
find ./src -name "*.css" | xargs \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g"

# --- Step 5: Update root configuration files ---
# These are individual files in the project root that contain copyright headers.
# Uses a for loop to only process files that exist (avoids sed errors).
for file in vite.config.ts tailwind.config.js postcss.config.js commitlint.config.js; do
    if [ -f "$file" ]; then
        sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" "$file"
    fi
done

# --- Step 6: Update the LICENSE file ---
# The LICENSE file contains the full copyright notice and terms.
if [ -f "LICENSE" ]; then
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" LICENSE
fi

# --- Step 7: Update tauri.conf.json ---
# The bundle.copyright field is embedded into platform installers (DMG, MSI, DEB)
# and displayed in the application's "About" dialog.
if [ -f "src-tauri/tauri.conf.json" ]; then
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" src-tauri/tauri.conf.json
fi

echo "Copyright year updated to $CURRENT_YEAR in all source files."
