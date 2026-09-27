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
// #1224 / follow-up: a dependency declared for only ONE platform —
// inside a `[target.'cfg(target_os = "macos")'.dependencies]` table
// rather than the plain `[dependencies]` table (e.g. `libc`, which
// src-tauri only needs on macOS, to read the flag Finder sets on an
// alias) — used to be invisible to this script. The original fix taught
// a hand-rolled regular-expression parser every `[target.<spec>.
// dependencies]` header shape Cargo accepts, plus the dotted-key form
// written with no `[...]` header at all. A Codex review of that fix
// reproduced two further faults, and both are inherent to reading TOML
// with regular expressions rather than two more shapes to add to an
// ever-growing list: (1) valid TOML the regex still could not read — a
// comment trailing a table header on the same line, a dependency
// declared as its own sub-table, spaces around the dots in a dotted
// key, a quoted dependency name, an inline table with trailing content;
// (2) a dotted assignment ignores which TABLE it is actually inside —
// TOML dotted keys are valid ANYWHERE, so
// `target.'cfg(unix)'.dependencies.fake = "1"` sitting under
// `[package.metadata]` (nothing to do with real dependencies) was
// wrongly counted as one, because the regex matched the line's shape
// without checking which section it was really inside. Cargo itself
// already contains a real TOML parser — it has to load this same file
// to build the project — so this script now asks Cargo for the answer
// (`cargo metadata --no-deps`) instead of re-implementing a second,
// necessarily incomplete one. What Cargo reports about its own manifest
// can never disagree with what Cargo itself will do with that manifest.
// See `scripts/lib/cargo-direct-deps.mjs` for the full rationale and
// `scripts/test-check-acknowledgements.mjs` for the regression tests
// (both the original per-target blind spot and the two further faults).

import { readFileSync } from 'fs';
import { join, dirname, resolve } from 'path';
import { fileURLToPath } from 'url';
import { readDirectRustDeps } from './lib/cargo-direct-deps.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const ACK_PATH = join(ROOT, 'ACKNOWLEDGEMENTS.md');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');
const PACKAGE_JSON = join(ROOT, 'package.json');

// Read ACKNOWLEDGEMENTS.md once and lowercase it for case-insensitive
// substring matching below. We don't try to parse the markdown — too
// many false-negative shapes (table cell, list bullet, header text).
const ackText = readFileSync(ACK_PATH, 'utf8').toLowerCase();

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
// `import`ed. This module no longer exports a parsing function itself
// (that moved to `scripts/lib/cargo-direct-deps.mjs`, which the test
// file imports directly), but the guard still matters: without it,
// simply `import`ing this file for any reason would ALSO run the real
// check against this machine's actual repo state — including shelling
// out to `cargo` and calling `process.exit()` at the end, either of
// which would break an importer that only wanted to reuse a helper.
function main() {
  const rustDeps = readDirectRustDeps(CARGO_TOML).filter((n) => !SKIP_RUST.has(n));
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
