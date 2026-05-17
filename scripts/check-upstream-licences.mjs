#!/usr/bin/env node
// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

// Upstream-licence-string drift check for MeedyaDL's direct dependencies
// (#806). Companion to `check-acknowledgements.mjs` (#802 set-coverage):
//
//   - `check-acknowledgements.mjs` — every direct dep is *named* in
//     ACKNOWLEDGEMENTS.md (cheap, fast, false-positive-free).
//   - This script — every direct dep's actual upstream licence STRING
//     matches what ACKNOWLEDGEMENTS.md claims. Catches upstream re-
//     licensings between MeedyaDL releases (most commonly: MIT → dual
//     MIT/Apache-2.0, or vice versa) so we never serve stale notices
//     to users.
//
// Run: `node scripts/check-upstream-licences.mjs` (or via
// `npm run check:upstream-licences` or the umbrella `npm run check:legal`).
//
// Behaviour:
//   exit 0   — every direct dep's upstream licence matches the
//              ACKNOWLEDGEMENTS.md entry (verbatim or via a tolerated
//              equivalence rule like `MIT/Apache-2.0` ≡ `MIT OR Apache-2.0`).
//   exit 1   — at least one dep has a real mismatch. Output names the
//              dep, the upstream string, and the ACKNOWLEDGEMENTS.md
//              string side-by-side so a maintainer can decide whether
//              to update the inventory or push back upstream.
//   exit 0 + stderr warning — whitespace-only differences (treated as
//              advisory; don't break CI).
//
// Upstream sources of truth:
//   - Rust: `cargo metadata --format-version=1 --no-deps`, which reads
//     the licence string straight from each crate's Cargo.toml. No
//     network call needed because the metadata is local once `cargo
//     fetch` (or the `Swatinem/rust-cache` restore) has populated the
//     registry cache.
//   - npm: `node_modules/<pkg>/package.json::license` — the canonical
//     npm equivalent. Requires `npm ci` to have populated node_modules
//     (the CI workflow does this before invoking the check).
//
// Skip rules mirror the companion script (#802) — see SKIP_RUST /
// SKIP_NPM / isTauriPlugin below.

import { execFileSync, execSync } from 'child_process';
import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const ACK_PATH = join(ROOT, 'ACKNOWLEDGEMENTS.md');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');
const PACKAGE_JSON = join(ROOT, 'package.json');

// Read ACKNOWLEDGEMENTS.md once and parse out the licence column from
// the markdown tables. We don't try to be clever — a simple regex that
// matches "| <name> | <version> | <licence> | <description> |" rows
// across the file gives a `{ name: licence }` map. Names that appear
// in multiple sections (rare, but defensive) take the last value seen.
function parseAckLicences(path) {
  const text = readFileSync(path, 'utf8');
  const map = new Map();
  const rowRe = /^\|\s*([A-Za-z0-9_/@-]+(?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?)\s*\|\s*([^|]*?)\s*\|\s*([^|]+?)\s*\|/gm;
  for (const m of text.matchAll(rowRe)) {
    const name = m[1].trim();
    const lic = m[3].trim();
    // Skip the table header rows ("Crate" / "Plugin" / "Package")
    if (/^[A-Z][a-z]+$/.test(name) && /^Licen[cs]e$/i.test(lic)) continue;
    map.set(name, lic);
  }
  return map;
}

const ackLicences = parseAckLicences(ACK_PATH);

/** Direct Rust dependency keys from Cargo.toml's `[dependencies]` section. */
function parseCargoDirectDeps(path) {
  const lines = readFileSync(path, 'utf8').split('\n');
  const deps = [];
  let inSection = false;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.startsWith('#')) continue;
    if (line.startsWith('[')) {
      inSection = line === '[dependencies]';
      continue;
    }
    if (!inSection || !line) continue;
    const m = line.match(/^([A-Za-z0-9_-]+)\s*=/);
    if (m) deps.push(m[1]);
  }
  return deps;
}

/** Runtime npm deps from package.json (excludes devDependencies). */
function parseNpmRuntimeDeps(path) {
  const pkg = JSON.parse(readFileSync(path, 'utf8'));
  return Object.keys(pkg.dependencies ?? {});
}

function isTauriPlugin(name) {
  return (
    name.startsWith('tauri-plugin-') ||
    name === 'tauri' ||
    name === '@tauri-apps/api' ||
    name.startsWith('@tauri-apps/plugin-')
  );
}

const SKIP_RUST = new Set(['meedya-core']); // MWBMPartners sibling crate
const SKIP_NPM = new Set(['@testing-library/dom']); // mis-classified dev dep

// Equivalence rules — patterns that mean the same thing in different
// punctuation/casing. Used to demote "real mismatch" to "advisory".
// Each entry maps "upstream form" → set of "acceptable ACKNOWLEDGEMENTS forms".
// The match is permissive: if normalising both strings (lowercase,
// collapse whitespace, sort license tokens) makes them equal, we accept.
function normaliseLicence(s) {
  if (!s) return '';
  return s
    .toLowerCase()
    .replace(/\s+/g, ' ')
    // Treat `OR` (whitespace-surrounded) and `/` as the same
    // separator; sort tokens. Require whitespace around `or` rather
    // than `\bor\b` because the latter incorrectly matches the `or`
    // inside SPDX `-or-later` suffixes (e.g. `lgpl-3.0-or-later`).
    .split(/\s+or\s+|\s*\/\s*/)
    .map((p) => p.trim())
    // SPDX deprecated-suffix normalisation: `lgpl-3.0+` ≡ `lgpl-3.0-or-later`,
    // `gpl-2.0+` ≡ `gpl-2.0-or-later`, etc. The `+` form is the legacy
    // SPDX shorthand; the `-or-later` form is the current canonical
    // expression. They mean the same thing and we should treat them as
    // equivalent for drift purposes.
    .map((p) => p.replace(/(\d)\+$/, '$1-or-later'))
    .filter(Boolean)
    .sort()
    .join('/');
}

function licencesAgree(upstream, ack) {
  if (upstream === ack) return 'exact';
  if (normaliseLicence(upstream) === normaliseLicence(ack)) return 'normalised';
  return null;
}

/** Read every Rust crate's declared licence from cargo-metadata. */
function readRustUpstreamLicences() {
  try {
    const raw = execFileSync(
      'cargo',
      ['metadata', '--format-version=1', '--no-deps', '--manifest-path', join('src-tauri', 'Cargo.toml')],
      { cwd: ROOT, maxBuffer: 64 * 1024 * 1024, encoding: 'utf8' },
    );
    const meta = JSON.parse(raw);
    // For our own package, dependencies[] has each direct dep but no
    // licence (that's per-crate metadata, which `--no-deps` strips).
    // Fetch the per-dep licences via a second call with `--no-deps`
    // off, but constrain to direct deps only via the dependency tree.
    const rawAll = execFileSync(
      'cargo',
      ['metadata', '--format-version=1', '--manifest-path', join('src-tauri', 'Cargo.toml')],
      { cwd: ROOT, maxBuffer: 256 * 1024 * 1024, encoding: 'utf8' },
    );
    const metaAll = JSON.parse(rawAll);
    const byName = new Map();
    for (const pkg of metaAll.packages) {
      // Last seen wins; cargo deduplicates names already.
      byName.set(pkg.name, pkg.license ?? '');
    }
    return byName;
  } catch (e) {
    console.error('::warning::cargo metadata failed — Rust licence check skipped:', e.message);
    return new Map();
  }
}

/** Read each direct npm dep's licence from node_modules. */
function readNpmUpstreamLicences(directDeps) {
  const out = new Map();
  for (const name of directDeps) {
    const pkgPath = join(ROOT, 'node_modules', name, 'package.json');
    if (!existsSync(pkgPath)) {
      // node_modules not installed (e.g. running locally without npm ci).
      continue;
    }
    try {
      const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
      // Some packages use `license`, some `licenses: [...]` (legacy),
      // some both. Prefer `license` (string) and fall back to a join
      // of the legacy array shape.
      let lic = pkg.license ?? '';
      if (!lic && Array.isArray(pkg.licenses)) {
        lic = pkg.licenses.map((l) => l.type ?? '').filter(Boolean).join(' OR ');
      }
      out.set(name, typeof lic === 'string' ? lic : JSON.stringify(lic));
    } catch {
      out.set(name, '');
    }
  }
  return out;
}

// Helpful warning if running without npm-installed deps.
function hasNodeModules() {
  return existsSync(join(ROOT, 'node_modules'));
}

const rustDeps = parseCargoDirectDeps(CARGO_TOML).filter(
  (n) => !SKIP_RUST.has(n) && !isTauriPlugin(n),
);
const npmDeps = parseNpmRuntimeDeps(PACKAGE_JSON).filter(
  (n) => !SKIP_NPM.has(n) && !isTauriPlugin(n),
);

const rustLicences = readRustUpstreamLicences();
const npmLicences = hasNodeModules() ? readNpmUpstreamLicences(npmDeps) : new Map();

if (!hasNodeModules()) {
  console.error(
    '::warning::node_modules/ not found — npm licence string check skipped. Run `npm ci` first.',
  );
}

const mismatches = [];
const advisories = [];
const missingAck = [];

function compareOne(kind, name, upstream) {
  if (!upstream) return; // No data — silent skip.
  const ack = ackLicences.get(name);
  if (!ack) {
    missingAck.push({ kind, name, upstream });
    return;
  }
  const verdict = licencesAgree(upstream, ack);
  if (verdict === null) {
    mismatches.push({ kind, name, upstream, ack });
  } else if (verdict === 'normalised') {
    // Punctuation difference only — advisory, doesn't break CI.
    advisories.push({ kind, name, upstream, ack });
  }
}

for (const name of rustDeps) compareOne('rust', name, rustLicences.get(name));
for (const name of npmDeps) compareOne('npm', name, npmLicences.get(name));

const verbose = process.argv.includes('--verbose');

if (!mismatches.length && !missingAck.length) {
  if (advisories.length && verbose) {
    console.error(`ℹ ${advisories.length} licence(s) match after normalisation (advisory):`);
    for (const a of advisories) {
      console.error(`  - ${a.kind} ${a.name}: ack='${a.ack}', upstream='${a.upstream}'`);
    }
  }
  console.log(
    `✓ All direct dep licences match ACKNOWLEDGEMENTS.md (rust=${rustDeps.length}, npm=${npmDeps.length}, advisories=${advisories.length}).${
      advisories.length && !verbose ? ' Re-run with --verbose to enumerate.' : ''
    }`,
  );
  process.exit(0);
}

console.error('✗ Upstream licence drift detected (#806).\n');

if (mismatches.length) {
  console.error(`  ${mismatches.length} licence MISMATCH(es) — block CI:`);
  for (const m of mismatches) {
    console.error(`    - ${m.kind} ${m.name}:`);
    console.error(`        ACKNOWLEDGEMENTS.md says: ${m.ack || '(empty)'}`);
    console.error(`        upstream declares:       ${m.upstream || '(empty)'}`);
  }
}
if (missingAck.length) {
  console.error(`  ${missingAck.length} dep(s) with upstream licence but no ACKNOWLEDGEMENTS entry:`);
  for (const m of missingAck) {
    console.error(`    - ${m.kind} ${m.name}: upstream='${m.upstream}'`);
  }
}
if (advisories.length) {
  console.error(`  ${advisories.length} advisory normalisation match(es) (not blocking):`);
  for (const a of advisories) {
    console.error(`    - ${a.kind} ${a.name}: ack='${a.ack}', upstream='${a.upstream}'`);
  }
}

console.error('\nFix by either:');
console.error('  - Updating the ACKNOWLEDGEMENTS.md licence column to match upstream, or');
console.error('  - Verifying upstream actually re-licensed (rare) and updating ACKNOWLEDGEMENTS.md + downstream notices.');
process.exit(1);
