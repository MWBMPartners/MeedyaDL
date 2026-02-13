// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Version Bump Script for MeedyaDL
// ===================================
//
// Updates the application version across all source-of-truth files in a single
// atomic operation. This ensures package.json, tauri.conf.json, Cargo.toml, and
// Cargo.lock always stay in sync.
//
// Usage:
//   node scripts/bump-version.mjs <bump>
//
// Where <bump> is one of:
//   major    -- Increment the major version (1.2.3 → 2.0.0)
//   minor    -- Increment the minor version (1.2.3 → 1.3.0)
//   patch    -- Increment the patch version (1.2.3 → 1.2.4)
//   X.Y.Z    -- Set an explicit version (e.g., 1.0.0-beta.1)
//
// Output:
//   Prints the new version string to stdout (e.g., "1.2.4") for capture by CI.
//
// Files updated:
//   - package.json              -- npm package version (.version field)
//   - src-tauri/tauri.conf.json -- Tauri app version (.version field)
//   - src-tauri/Cargo.toml      -- Rust crate version ([package] section)
//   - src-tauri/Cargo.lock      -- Lock file version (meedyadl entry)
//
// This script has zero external dependencies — it uses only Node.js built-ins.
//
// @see https://semver.org/ -- Semantic Versioning specification
// @see scripts/generate-icons.mjs -- Sister script following the same conventions

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

// ---------------------------------------------------------------------------
// Resolve project root directory
// ---------------------------------------------------------------------------
// __dirname equivalent for ESM: derive the script's directory from import.meta.url,
// then go up one level to reach the project root (scripts/ → project root).
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ROOT = resolve(__dirname, '..');

// ---------------------------------------------------------------------------
// File paths (relative to project root)
// ---------------------------------------------------------------------------
const PACKAGE_JSON = resolve(ROOT, 'package.json');
const TAURI_CONF = resolve(ROOT, 'src-tauri', 'tauri.conf.json');
const CARGO_TOML = resolve(ROOT, 'src-tauri', 'Cargo.toml');
const CARGO_LOCK = resolve(ROOT, 'src-tauri', 'Cargo.lock');

/**
 * Parse a semver version string into its three numeric components.
 *
 * Only handles the `MAJOR.MINOR.PATCH` core — pre-release and build metadata
 * are stripped for the purpose of computing the next version. This is sufficient
 * because bump operations (major/minor/patch) always produce clean versions.
 *
 * @param {string} version - A version string like "0.1.0" or "1.2.3-beta.1"
 * @returns {{ major: number, minor: number, patch: number }}
 */
function parseSemver(version) {
  /* Split on dots, parse as integers, ignore pre-release suffix */
  const [major, minor, patch] = version.split('-')[0].split('.').map(Number);
  return { major, minor, patch };
}

/**
 * Compute the new version based on the bump type or explicit version.
 *
 * @param {string} currentVersion - The current version (e.g., "0.1.0")
 * @param {string} bump - One of "major", "minor", "patch", or an explicit "X.Y.Z"
 * @returns {string} The new version string
 */
function computeNewVersion(currentVersion, bump) {
  /* If the bump argument looks like a version number, use it directly */
  if (/^\d+\.\d+\.\d+/.test(bump)) {
    return bump;
  }

  const { major, minor, patch } = parseSemver(currentVersion);

  switch (bump) {
    case 'major':
      return `${major + 1}.0.0`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    default:
      console.error(`Error: Invalid bump type "${bump}". Use major, minor, patch, or X.Y.Z`);
      process.exit(1);
  }
}

/**
 * Update a JSON file's "version" field.
 *
 * Reads the file, parses as JSON, sets the top-level "version" key to the new
 * value, and writes back with 2-space indentation and a trailing newline (matching
 * the project's Prettier configuration).
 *
 * @param {string} filePath - Absolute path to the JSON file
 * @param {string} newVersion - The new version string
 */
function updateJsonFile(filePath, newVersion) {
  const content = JSON.parse(readFileSync(filePath, 'utf-8'));
  content.version = newVersion;
  writeFileSync(filePath, JSON.stringify(content, null, 2) + '\n', 'utf-8');
}

/**
 * Update the version in Cargo.toml's [package] section.
 *
 * Uses a targeted regex that matches the `version = "..."` line that follows
 * `name = "meedyadl"` within the [package] section. This avoids accidentally
 * modifying dependency version fields elsewhere in the file.
 *
 * @param {string} filePath - Absolute path to Cargo.toml
 * @param {string} currentVersion - The current version to find
 * @param {string} newVersion - The new version to set
 */
function updateCargoToml(filePath, currentVersion, newVersion) {
  let content = readFileSync(filePath, 'utf-8');

  /*
   * Match the version line in the [package] section (follows name = "meedyadl").
   * Allow any number of intermediate lines (comments, blanks) between name and version,
   * since Cargo.toml may have comment lines between the two fields.
   * The [\s\S]*? is a non-greedy match for any characters including newlines.
   */
  const pattern = new RegExp(
    `(name = "meedyadl"\\n[\\s\\S]*?version = ")${escapeRegex(currentVersion)}(")`
  );
  const updated = content.replace(pattern, `$1${newVersion}$2`);

  if (updated === content) {
    console.error(`Error: Could not find version "${currentVersion}" in ${filePath}`);
    process.exit(1);
  }

  writeFileSync(filePath, updated, 'utf-8');
}

/**
 * Update the version in Cargo.lock for the meedyadl package.
 *
 * Cargo.lock uses a similar format: `name = "meedyadl"` followed by
 * `version = "X.Y.Z"` on the next line. We target this specific block.
 *
 * @param {string} filePath - Absolute path to Cargo.lock
 * @param {string} currentVersion - The current version to find
 * @param {string} newVersion - The new version to set
 */
function updateCargoLock(filePath, currentVersion, newVersion) {
  let content = readFileSync(filePath, 'utf-8');

  /* Match the version line immediately after name = "meedyadl" */
  const pattern = new RegExp(
    `(name = "meedyadl"\\nversion = ")${escapeRegex(currentVersion)}(")`
  );
  const updated = content.replace(pattern, `$1${newVersion}$2`);

  if (updated === content) {
    console.error(`Error: Could not find meedyadl version "${currentVersion}" in ${filePath}`);
    process.exit(1);
  }

  writeFileSync(filePath, updated, 'utf-8');
}

/**
 * Escape special regex characters in a string.
 *
 * @param {string} str - The string to escape
 * @returns {string} The escaped string safe for use in a RegExp constructor
 */
function escapeRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// ===========================================================================
// Main
// ===========================================================================

/* Validate CLI arguments */
const bump = process.argv[2];
if (!bump) {
  console.error('Usage: node scripts/bump-version.mjs <major|minor|patch|X.Y.Z>');
  process.exit(1);
}

/* Read the current version from package.json (the primary source of truth) */
const pkg = JSON.parse(readFileSync(PACKAGE_JSON, 'utf-8'));
const currentVersion = pkg.version;

/* Compute the new version */
const newVersion = computeNewVersion(currentVersion, bump);

/* Update all four files */
updateJsonFile(PACKAGE_JSON, newVersion);
updateJsonFile(TAURI_CONF, newVersion);
updateCargoToml(CARGO_TOML, currentVersion, newVersion);
updateCargoLock(CARGO_LOCK, currentVersion, newVersion);

/* Print the new version to stdout for CI capture */
console.log(newVersion);
