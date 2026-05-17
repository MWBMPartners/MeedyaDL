#!/usr/bin/env node
// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

// Drift check for ACKNOWLEDGEMENTS.md (#802).
//
// Run: `node scripts/check-acknowledgements.mjs` (or via `npm run check:acks`).
// Exits non-zero if any direct dependency in `src-tauri/Cargo.toml` or
// `package.json` is NOT named in `ACKNOWLEDGEMENTS.md`. The check is
// purely set-coverage — it does not validate licence strings, versions,
// or descriptions — so it can run cheaply in CI on every push without
// false-positive churn from auto-bumped patch versions or whitespace
// shuffles.
//
// The intent (per #802 gap #5) is to make sure that when someone adds a
// new direct dependency they also remember to extend ACKNOWLEDGEMENTS.md
// with the licence and one-line purpose. The check is intentionally
// generous: it only requires the *name* of the dep to appear somewhere
// inside ACKNOWLEDGEMENTS.md, on the principle that an entry someone
// added but forgot to fill in correctly is better than no entry at all.
//
// Tauri's own plugin crates (`tauri-plugin-*`) and the `tauri` framework
// itself are bundled together under the "Tauri Plugins" section, so we
// permit those to match a generic "Tauri Plugins" mention rather than
// requiring each individual plugin name. Dev/build-time-only deps
// (`@types/*`, eslint-*, vitest, prettier, vite, etc. on the npm side;
// `cargo-*` and CI-only crates on the Rust side) don't ship in the
// binary and are excluded from the check.

import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const ACK_PATH = join(ROOT, 'ACKNOWLEDGEMENTS.md');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');
const PACKAGE_JSON = join(ROOT, 'package.json');

// Read ACKNOWLEDGEMENTS.md once and lowercase it for case-insensitive
// substring matching below. We don't try to parse the markdown — too
// many false-negative shapes (table cell, list bullet, header text).
const ackText = readFileSync(ACK_PATH, 'utf8').toLowerCase();

/** Parse `[dependencies]` and `[dev-dependencies]` keys from a Cargo.toml. */
function parseCargoDirectDeps(cargoToml) {
  const lines = readFileSync(cargoToml, 'utf8').split('\n');
  const deps = [];
  let inSection = false;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.startsWith('#')) continue;
    if (line.startsWith('[')) {
      // Stay only inside `[dependencies]`. We skip dev-dependencies and
      // build-dependencies because those don't ship in the binary.
      inSection = line === '[dependencies]';
      continue;
    }
    if (!inSection || !line) continue;
    const m = line.match(/^([A-Za-z0-9_-]+)\s*=/);
    if (m) deps.push(m[1]);
  }
  return deps;
}

/** Return runtime npm dependency names (excludes dev/build-time deps). */
function parseNpmRuntimeDeps(packageJson) {
  const pkg = JSON.parse(readFileSync(packageJson, 'utf8'));
  return Object.keys(pkg.dependencies ?? {});
}

// Tauri plugin crates collapse to a single "Tauri Plugins" inventory
// section in ACKNOWLEDGEMENTS.md — they don't need to be listed by name
// to satisfy the drift check. Both halves of the Tauri ecosystem
// (Rust crate `tauri-plugin-*` + npm package `@tauri-apps/plugin-*`)
// share that same logical section.
function isTauriPlugin(name) {
  return (
    name.startsWith('tauri-plugin-') ||
    name === 'tauri' ||
    name === '@tauri-apps/api' ||
    name.startsWith('@tauri-apps/plugin-')
  );
}

// Skip dev/build-time crates that don't ship inside the binary, and
// MWBMPartners' own sibling crates / packages (covered by our own
// LICENSE rather than a third-party acknowledgement).
//
// Adjust conservatively: over-skipping is safer than over-flagging,
// but every entry here should have a documented rationale.
const SKIP_RUST = new Set([
  // MWBMPartners' own sibling crate from the MeedyaSuite-core project —
  // governed by our own LICENSE, not third-party.
  'meedya-core',
]);
const SKIP_NPM = new Set([
  // @testing-library/dom is mis-classified in `dependencies` but is
  // strictly a test utility — never reaches the production bundle.
  // Tracked as a separate `package.json` cleanup; not a licence gap.
  '@testing-library/dom',
]);

function isMentioned(name) {
  // Case-insensitive substring match against the lowercased
  // ACKNOWLEDGEMENTS.md. Generous on purpose — see top-of-file comment.
  return ackText.includes(name.toLowerCase());
}

const rustDeps = parseCargoDirectDeps(CARGO_TOML).filter((n) => !SKIP_RUST.has(n));
const npmDeps = parseNpmRuntimeDeps(PACKAGE_JSON).filter((n) => !SKIP_NPM.has(n));

const missingRust = rustDeps.filter((n) => !isTauriPlugin(n) && !isMentioned(n));
const missingNpm = npmDeps.filter((n) => !isTauriPlugin(n) && !isMentioned(n));

// Tauri-plugin escape hatch: as a soft fallback, require the literal
// string "Tauri Plugins" (case-insensitive) to appear in the file.
const tauriPluginMentioned = ackText.includes('tauri plugins') || ackText.includes('tauri-plugin-');
const tauriOmitted = !tauriPluginMentioned && rustDeps.some(isTauriPlugin);

if (!missingRust.length && !missingNpm.length && !tauriOmitted) {
  console.log(
    `✓ ACKNOWLEDGEMENTS.md covers ${rustDeps.length} Rust deps + ${npmDeps.length} npm deps (no drift).`,
  );
  process.exit(0);
}

console.error('✗ ACKNOWLEDGEMENTS.md drift detected (#802 gap #5).\n');
if (missingRust.length) {
  console.error(`  Rust direct dependencies missing from ACKNOWLEDGEMENTS.md:`);
  for (const n of missingRust) console.error(`    - ${n}`);
}
if (missingNpm.length) {
  console.error(`  npm runtime dependencies missing from ACKNOWLEDGEMENTS.md:`);
  for (const n of missingNpm) console.error(`    - ${n}`);
}
if (tauriOmitted) {
  console.error(
    `  Tauri plugin block missing — at least one tauri-plugin-* is in Cargo.toml but ACKNOWLEDGEMENTS.md has no "Tauri Plugins" section.`,
  );
}
console.error('\nFix by either:');
console.error("  - Adding the dependency to ACKNOWLEDGEMENTS.md with its licence + purpose, or");
console.error("  - Adding it to the SKIP_RUST / SKIP_NPM allowlist in this script (with rationale).");
process.exit(1);
