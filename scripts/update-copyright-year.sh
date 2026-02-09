#!/bin/bash
# Copyright (c) 2024-2026 MWBM Partners Ltd
# Licensed under the MIT License. See LICENSE file in the project root.
#
# Updates the copyright end-year in all source files to the current year.
# This script should be run at the start of each new year (or in CI)
# to keep copyright notices current.
#
# Usage: ./scripts/update-copyright-year.sh
#
# Matches patterns like:
#   Copyright (c) 2024-2025 MWBM Partners Ltd
#   Copyright (c) 2024-2026 MWBM Partners Ltd
# And updates the end year to the current year.

# Get the current year
CURRENT_YEAR=$(date +%Y)

echo "Updating copyright year to: $CURRENT_YEAR"

# Update Rust source files (.rs)
find ./src-tauri/src -name "*.rs" -exec \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" {} \;

# Update TypeScript/TSX source files (.ts, .tsx)
find ./src -name "*.ts" -o -name "*.tsx" | xargs \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g"

# Update CSS files
find ./src -name "*.css" | xargs \
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g"

# Update configuration files
for file in vite.config.ts tailwind.config.js postcss.config.js commitlint.config.js; do
    if [ -f "$file" ]; then
        sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" "$file"
    fi
done

# Update the LICENSE file
if [ -f "LICENSE" ]; then
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" LICENSE
fi

# Update Cargo.toml copyright
if [ -f "src-tauri/tauri.conf.json" ]; then
    sed -i '' "s/Copyright (c) 2024-[0-9]*/Copyright (c) 2024-$CURRENT_YEAR/g" src-tauri/tauri.conf.json
fi

echo "Copyright year updated to $CURRENT_YEAR in all source files."
