#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaSuite
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
// SKIP_NPM / isTauriNpmPackage below.
//
// SCOPE — read this before assuming a clean run means "all licences
// verified". This script can only compare licence strings for the two
// kinds of dependency that carry machine-readable licence metadata:
//
//   - direct Rust crates named in `src-tauri/Cargo.toml`'s
//     `[dependencies]` (via `cargo metadata`), and
//   - direct npm runtime deps named in `package.json`'s `dependencies`
//     (via each package's own `node_modules/<pkg>/package.json`).
//
// It does NOT cover, and has no mechanism to cover:
//   - transitive dependencies of either (that's `cargo-deny check
//     licenses` / the licence allowlist in `src-tauri/deny.toml`);
//   - the download engines and external tools listed in
//     ACKNOWLEDGEMENTS.md's "Download Engines" / "External Tools"
//     tables (GAMDL, votify, yt-dlp, get_iplayer, FFmpeg, mp4decrypt,
//     N_m3u8DL-RE, MP4Box, MediaInfo, Python, rclone). None of those
//     are Cargo or npm dependencies, so there is no `cargo
//     metadata`/`package.json` entry for this script to read — their
//     licence text is sourced from each project's own upstream LICENSE
//     file and has to be verified and kept in sync BY HAND. This is
//     exactly how ACKNOWLEDGEMENTS.md was able to say mp4decrypt /
//     Bento4 was "MIT" for a stretch of time when it is actually
//     GPL-2.0 with a linking exception — this script had no way to
//     catch that, and still doesn't. Don't treat a clean run of this
//     script as proof those rows are correct.

import { execFileSync } from 'child_process';
import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const ACK_PATH = join(ROOT, 'ACKNOWLEDGEMENTS.md');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');
const PACKAGE_JSON = join(ROOT, 'package.json');

// Read ACKNOWLEDGEMENTS.md once and parse out the licence column from
// its markdown tables — by reading each table's own HEADER row to find
// where the "Licence" column actually is, not by assuming every table
// has the same shape. This matters because it doesn't: the main
// dependency tables are 4 columns (Crate/Version/Licence/Description),
// but "Tauri Plugins", "Download Engines", and "External Tools" are
// only 3 (name/Licence/Purpose — no version column). An earlier version
// of this function assumed column 3 was always the licence, which
// silently misread every "Tauri Plugins" row's PURPOSE text as its
// licence — invisible for as long as those rows were skipped entirely,
// and would have produced false "mismatch" noise the moment that skip
// was lifted (see isTauriNpmPackage above; this widened check is what
// exposed it). Names that appear in more than one table (rare, but
// defensive) take the last value seen.
function parseAckLicences(path) {
  const lines = readFileSync(path, 'utf8').split('\n');
  const map = new Map();

  const isTableRow = (line) => line.trim().startsWith('|');
  const isSeparatorRow = (line) => /^\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/.test(line.trim());
  const splitRow = (line) =>
    line
      .trim()
      .replace(/^\|/, '')
      .replace(/\|$/, '')
      .split('|')
      .map((c) => c.trim());

  let licenceCol = -1; // index of the "Licence" column in the CURRENT table; -1 = not in one

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!isTableRow(line)) {
      licenceCol = -1; // left whatever table we were in
      continue;
    }
    if (isSeparatorRow(line)) continue; // the "|---|---|" row itself

    if (isSeparatorRow(lines[i + 1] ?? '')) {
      // This row is a HEADER — the next line being a separator is what
      // makes it one, not its position or content. Find the licence
      // column for every row until the table ends.
      const headers = splitRow(line).map((h) => h.toLowerCase());
      licenceCol = headers.findIndex((h) => /^licen[cs]e$/.test(h));
      continue;
    }

    if (licenceCol === -1) continue; // inside a table with no recognised licence column

    const cells = splitRow(line);
    if (cells.length <= licenceCol) continue;
    // The name column is sometimes a markdown link (`[name](url)`) or
    // an inline code span (`` `name` ``) rather than a bare word —
    // strip that decoration so it can be matched against a plain
    // Cargo/npm dependency name.
    const name = cells[0]
      .replace(/^\[([^\]]+)\]\([^)]*\)$/, '$1')
      .replace(/^`([^`]+)`$/, '$1')
      .trim();
    map.set(name, cells[licenceCol].trim());
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

// npm's half of the Tauri ecosystem (`@tauri-apps/api`,
// `@tauri-apps/plugin-*`) is deliberately excluded from THIS script's
// npm check. ACKNOWLEDGEMENTS.md documents Tauri's dual licence once,
// against the Rust crate rows under "Tauri Plugins" (which this script
// DOES now check — see below), and covers the matching npm packages
// with a single cross-reference sentence rather than a full duplicate
// row per package. Since there's no per-package ACK row to parse for
// `@tauri-apps/plugin-dialog` etc., checking them here would only ever
// produce a "missing from ACKNOWLEDGEMENTS.md" false alarm, not a real
// licence-drift finding.
function isTauriNpmPackage(name) {
  return name === '@tauri-apps/api' || name.startsWith('@tauri-apps/plugin-');
}

// meedya-core, meedya-fingerprint, and meedya-lyrics are all MWBMPartners'
// own sibling crates from the MeedyaSuite-core repo, and all three are
// listed by name in ACKNOWLEDGEMENTS.md's Rust dependency table with
// their real (MIT) licence — so none of them need a skip here any more.
// (meedya-core used to be skipped on the theory that "our own crate"
// doesn't need a third-party acknowledgement, but that reasoning was
// applied inconsistently — meedya-fingerprint/meedya-lyrics were never
// skipped — and meedya-core was simply missing from ACK, not
// deliberately omitted. It's tracked now.)
const SKIP_RUST = new Set();
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

// Well-known licence boilerplate, matched against a crate's own
// `license-file` content when Cargo.toml has no `license` string at
// all (rookie is the current example: it declares `license-file =
// "MIT-LICENSE.txt"` and nothing else, so `cargo metadata` reports an
// empty licence — with no fallback, the old code treated that as "no
// data, silently skip", which is how rookie's real MIT licence went
// unchecked). This is deliberately just enough to recognise the small
// set of permissive licences this project actually allows (see
// `src-tauri/deny.toml`) — it is not a general SPDX detector. A file
// that doesn't match anything here still isn't force-fitted into a
// guess; the caller is left with no upstream signal, same as today.
const LICENCE_FILE_SIGNATURES = [
  ['MIT', (t) => t.includes('permission is hereby granted, free of charge')],
  ['Apache-2.0', (t) => t.includes('apache license') && t.includes('version 2.0')],
  ['BSD-3-Clause', (t) => t.includes('neither the name')],
  ['BSD-2-Clause', (t) => t.includes('redistributions of source code must retain')],
  ['ISC', (t) => t.includes('permission to use, copy, modify, and/or distribute this software')],
  ['MPL-2.0', (t) => t.includes('mozilla public license')],
  ['LGPL-2.1', (t) => t.includes('lesser general public license')],
  ['GPL-2.0', (t) => t.includes('gnu general public license')],
];

function identifyLicenceFromFile(filePath) {
  let text;
  try {
    text = readFileSync(filePath, 'utf8').toLowerCase();
  } catch {
    return '';
  }
  for (const [spdx, matches] of LICENCE_FILE_SIGNATURES) {
    if (matches(text)) return spdx;
  }
  return '';
}

/** Read every Rust crate's declared licence from cargo-metadata. */
function readRustUpstreamLicences() {
  let rawAll;
  try {
    // `cargo metadata` (without `--no-deps`) returns every package in the
    // resolved tree with its declared SPDX licence string. The caller
    // looks up direct deps by name, so extra transitive entries are
    // harmless — and cheaper than a second `--no-deps` round-trip.
    rawAll = execFileSync(
      'cargo',
      ['metadata', '--format-version=1', '--manifest-path', join('src-tauri', 'Cargo.toml')],
      { cwd: ROOT, maxBuffer: 256 * 1024 * 1024, encoding: 'utf8' },
    );
  } catch (e) {
    // This must FAIL, not degrade to "Rust half skipped" plus a warning.
    // A silent skip here used to mean every Rust dependency was exempted
    // from the whole check — with an exit-0 tick printed at the end, as
    // if everything had actually been verified. That is the exact
    // "reported success after checking nothing" failure mode this
    // project has already had to fix elsewhere (see channel-security-
    // audit.yml and the #1146 discipline it names): a tool that could
    // not run must never be reported as "checked and fine".
    console.error('::error::cargo metadata failed — cannot verify ANY Rust dependency licence.');
    console.error(`    ${e.message}`);
    console.error('    Make sure `cargo` is on PATH (e.g. `export PATH="$HOME/.cargo/bin:$PATH"`)');
    console.error('    and the dependency tree has been fetched, then re-run. This script exits');
    console.error('    non-zero here on purpose — it must not report success for a half of the');
    console.error('    check it never actually ran.');
    process.exit(1);
  }
  const metaAll = JSON.parse(rawAll);
  const byName = new Map();
  for (const pkg of metaAll.packages) {
    let licence = pkg.license ?? '';
    if (!licence && pkg.license_file) {
      const filePath = join(dirname(pkg.manifest_path), pkg.license_file);
      licence = identifyLicenceFromFile(filePath);
    }
    // Last seen wins; cargo deduplicates names already.
    byName.set(pkg.name, licence);
  }
  return byName;
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

const rustDeps = parseCargoDirectDeps(CARGO_TOML).filter((n) => !SKIP_RUST.has(n));
const npmDeps = parseNpmRuntimeDeps(PACKAGE_JSON).filter(
  (n) => !SKIP_NPM.has(n) && !isTauriNpmPackage(n),
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

// A cell that's blank, or just a typographic placeholder like an
// em-dash, is treated the same as "not filled in" — see isEmptyAckValue
// below.
function isEmptyAckValue(v) {
  if (!v) return true;
  const t = v.trim();
  return t === '' || t === '—' || t === '-' || /^n\/a$/i.test(t);
}

function compareOne(kind, name, upstream) {
  const hasAckRow = ackLicences.has(name);
  const ack = ackLicences.get(name);

  // A row that names the dependency but leaves the licence blank (or a
  // placeholder dash) is a documentation fault on its own — this must
  // be reported even when upstream data is unavailable, never silently
  // skipped because "we couldn't verify it either". This is exactly
  // how rookie's ACKNOWLEDGEMENTS.md entry ("—") went unnoticed: the
  // old code's `if (!upstream) return` skipped the row before this
  // check ever ran.
  if (hasAckRow && isEmptyAckValue(ack)) {
    missingAck.push({ kind, name, upstream: upstream || '(unknown — see licence-file fallback)' });
    return;
  }

  if (!upstream) return; // No upstream signal, and ACK has a real value — nothing to compare.

  if (!hasAckRow) {
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
