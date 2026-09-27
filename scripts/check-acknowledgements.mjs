#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaSuite
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
//
// #1224: a dependency declared for only ONE platform — inside a
// `[target.'cfg(target_os = "macos")'.dependencies]` table rather than
// the plain `[dependencies]` table (e.g. `libc`, which src-tauri only
// needs on macOS, to read the flag Finder sets on an alias) — used to be
// invisible to this script. The old parser recognised exactly one table
// header, the literal string `[dependencies]`, so a per-target table's
// contents were silently skipped in both directions: a missing
// ACKNOWLEDGEMENTS.md entry for a macOS-only crate would never be
// caught, and an existing one was never actually being checked either —
// `libc` already had a row by the maintainer's own diligence, not
// because this script had ever verified it. Fixed by teaching the
// parser every table-header shape Cargo accepts for a per-target
// dependency (see TARGET_DEPS_HEADER / TARGET_DEPS_DOTTED_KEY below).
// Reproduced in `scripts/test-check-acknowledgements.mjs`.

import { readFileSync } from 'fs';
import { join, dirname, resolve } from 'path';
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

// Matches the header of a per-target dependency table, e.g.:
//   [target.'cfg(target_os = "macos")'.dependencies]
//   [target."cfg(target_os = \"macos\")".dependencies]
//   [target.x86_64-pc-windows-msvc.dependencies]
// Cargo requires the `cfg(...)` predicate to be quoted (it contains
// parentheses, spaces, and an embedded double-quoted string), and TOML
// allows either single or double quotes for that; a real target triple
// like `x86_64-pc-windows-msvc` needs no quoting at all, because every
// character in it is already a legal bare TOML key. The literal
// `.dependencies]` at the end (a dot, not a hyphen, right before the
// word) is what stops this from also matching a per-target
// `.build-dependencies]` / `.dev-dependencies]` table — this script
// deliberately doesn't read those (see the comment on
// parseCargoDirectDepsFromText below for why).
const TARGET_DEPS_HEADER =
  /^\[target\.(?:'[^']*'|"(?:[^"\\]|\\.)*"|[A-Za-z0-9_.+-]+)\.dependencies\]$/;

// Matches the same per-target table written as one dotted-key
// assignment instead of a `[...]` header — valid TOML, e.g.:
//   target.'cfg(target_os = "macos")'.dependencies.libc = "0.2"
// Nothing in this repo's Cargo.toml uses this form today, but Cargo
// genuinely accepts it, so a future edit could reintroduce the exact
// blind spot #1224 fixed if this script only ever looked for the
// `[...]` header shape.
const TARGET_DEPS_DOTTED_KEY =
  /^target\.(?:'[^']*'|"(?:[^"\\]|\\.)*"|[A-Za-z0-9_.+-]+)\.dependencies\.([A-Za-z0-9_-]+)\s*=/;

/**
 * Parse direct dependency names out of Cargo.toml TEXT (not a path).
 * Kept separate from `parseCargoDirectDeps` below so a test can hand it
 * a small in-memory sample instead of needing a real file on disk —
 * see `scripts/test-check-acknowledgements.mjs`.
 *
 * Reads the plain `[dependencies]` table AND every per-target
 * `[target.<spec>.dependencies]` table, in either header form, plus the
 * dotted-key form (see TARGET_DEPS_HEADER / TARGET_DEPS_DOTTED_KEY
 * above). Deliberately does NOT read `[dev-dependencies]` or
 * `[build-dependencies]` — plain or per-target — because neither ships
 * inside the compiled binary, so neither needs an ACKNOWLEDGEMENTS.md
 * entry; that was already this script's scope before #1224, and the
 * per-target fix does not widen it. A dependency named in more than one
 * table (e.g. it appears in both the plain table and a target-specific
 * one) is only counted once — the caller gets a de-duplicated list.
 */
export function parseCargoDirectDepsFromText(text) {
  const lines = text.split('\n');
  const deps = new Set();
  let inSection = false;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.startsWith('#')) continue;

    // A dotted-key dependency line is a self-contained assignment — it
    // doesn't live inside any `[section]` at all — so check for it
    // before, and independently of, the section-tracking logic below.
    const dotted = line.match(TARGET_DEPS_DOTTED_KEY);
    if (dotted) {
      deps.add(dotted[1]);
      continue;
    }

    if (line.startsWith('[')) {
      // Stay inside the plain `[dependencies]` table OR any per-target
      // `[target.<spec>.dependencies]` table. We skip dev-dependencies
      // and build-dependencies (plain or per-target) because those
      // don't ship in the binary.
      inSection = line === '[dependencies]' || TARGET_DEPS_HEADER.test(line);
      continue;
    }
    if (!inSection || !line) continue;
    const m = line.match(/^([A-Za-z0-9_-]+)\s*=/);
    if (m) deps.add(m[1]);
  }
  return [...deps];
}

/** Parse direct dependency names from a Cargo.toml FILE PATH. */
function parseCargoDirectDeps(cargoToml) {
  return parseCargoDirectDepsFromText(readFileSync(cargoToml, 'utf8'));
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

// Skip dev/build-time crates that don't ship inside the binary.
//
// meedya-core used to be skipped here on the theory that "our own
// sibling crate from MeedyaSuite-core" doesn't need a third-party
// acknowledgement — but that reasoning was never applied consistently
// (meedya-fingerprint and meedya-lyrics, from the same repo, were never
// skipped) and the practical effect was that meedya-core was simply
// missing from ACKNOWLEDGEMENTS.md rather than deliberately omitted.
// It's listed by name now, so no skip is needed.
//
// Adjust conservatively: over-skipping is safer than over-flagging,
// but every entry here should have a documented rationale.
const SKIP_RUST = new Set([]);
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

// The real coverage check only runs when this file is executed directly
// (`node scripts/check-acknowledgements.mjs`), never when it's merely
// `import`ed — e.g. by the test file, which only wants
// `parseCargoDirectDepsFromText`. Without this guard, importing the
// module for that one function would ALSO run the real check against
// this machine's actual repo state and call `process.exit()` at the
// end, killing the test process before its own assertions ever ran.
function main() {
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
}

// Node sets `process.argv[1]` to the absolute path of the script that
// was actually run. Comparing it against this module's own resolved
// path is the ESM equivalent of Python's `if __name__ == "__main__":` —
// it's what lets the test file `import` this module for its parsing
// function alone without ever triggering `main()` above. Running this
// file directly (the normal CLI use, and what `npm run check:acks`
// does) is unaffected: `resolve(process.argv[1])` then equals this
// file's own path, so `isMainModule` is true exactly as before.
const isMainModule =
  Boolean(process.argv[1]) && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMainModule) {
  main();
}
