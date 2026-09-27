#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

// Regression test for #1224 and its follow-up.
//
// #1224 (the original fault): a dependency declared for only one
// platform, inside a `[target.'cfg(target_os = "macos")'.dependencies]`
// table in src-tauri/Cargo.toml rather than the plain `[dependencies]`
// table, was invisible to both `check-acknowledgements.mjs` (#802) and
// `check-upstream-licences.mjs` (#806). The first fix taught a
// hand-rolled regular-expression reader of Cargo.toml's raw TEXT every
// per-target table-header shape Cargo accepts, plus a dotted-key form.
//
// The follow-up (this file, current shape): a Codex review of that
// regex-based fix reproduced two further faults that are inherent to
// reading TOML with regular expressions, not two more shapes to bolt on
// to a growing list --
//
//   1. Valid TOML the regex still could not read: a comment trailing a
//      table header on the same line, a dependency declared as its own
//      SUB-TABLE, spaces around the dots in a dotted key, a quoted
//      dependency name, an inline table with trailing content.
//   2. A dotted assignment ignored which TABLE it was actually inside --
//      TOML dotted keys are valid ANYWHERE, so a line shaped exactly
//      like a per-target dependency but sitting under an unrelated
//      table (e.g. `[package.metadata]`) was wrongly counted as a real
//      one, because the regex matched the LINE's shape without
//      checking which section it was really inside.
//
// The fix removes the hand-rolled reader entirely and asks Cargo's own
// parser for the answer instead (`cargo metadata --no-deps`, wrapped by
// `scripts/lib/cargo-direct-deps.mjs`). Cargo already has to load and
// understand this exact file to build the project, so what it reports
// about its own manifest cannot disagree with what it will actually do
// with that manifest -- removing the entire class of fault rather than
// patching one more instance of it.
//
// There is no existing test harness for the plain-Node scripts in this
// folder (only the frontend has Vitest), so this file is deliberately
// dependency-free: Node's own built-in `assert` module, run directly
// with `node scripts/test-check-acknowledgements.mjs`. It is not wired
// into `package.json` because none of the other check scripts' tests
// are either -- there is nothing to match the style of.
//
// Two kinds of test:
//   (a) UNIT tests of the pure `directDepsFromCargoMetadata` function
//       against small, hand-built `cargo metadata --no-deps` JSON
//       fixtures -- fast, deterministic, no subprocess.
//   (b) One INTEGRATION test that runs the real `cargo metadata
//       --no-deps` against the real `src-tauri/Cargo.toml` (via
//       `readDirectRustDeps`, the impure half of the same module) and
//       checks it against known facts about that actual file: `libc` is
//       declared only under the macOS-only per-target table (proving
//       Cargo's own reading covers exactly the shape #1224 was about),
//       and `tempfile` -- a real `[dev-dependencies]` entry in that same
//       file -- is correctly excluded.
//
// Run: `node scripts/test-check-acknowledgements.mjs`
// Exit 0 = every assertion passed. Exit 1 = at least one failed (the
// failing assertion's message is printed).

import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  directDepsFromCargoMetadata,
  readDirectRustDeps,
} from './lib/cargo-direct-deps.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const REAL_CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');

// A minimal but faithful `cargo metadata --no-deps --format-version 1`
// fixture: one root package with `manifest_path` set to a made-up path
// (so it never has to match anything real on disk) and a `dependencies`
// array covering the cases the task asks for. Real `cargo metadata`
// output has many more fields per dependency (`source`, `req`,
// `optional`, `uses_default_features`, `features`, `registry`); only
// the fields `directDepsFromCargoMetadata` actually reads are included
// here, which is deliberate -- it proves the function doesn't secretly
// depend on a field this fixture leaves out.
const FIXTURE_MANIFEST_PATH = '/fixture/src-tauri/Cargo.toml';

function makeMetadata(dependencies) {
  return {
    packages: [
      {
        name: 'fixture-pkg',
        manifest_path: FIXTURE_MANIFEST_PATH,
        dependencies,
      },
    ],
  };
}

const tests = [
  {
    name: 'a normal (kind: null) dependency with no target is included',
    fn: () => {
      const meta = makeMetadata([
        { name: 'serde', kind: null, target: null, rename: null },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      assert.ok(deps.includes('serde'), `expected 'serde'; got ${JSON.stringify(deps)}`);
    },
  },
  {
    name: 'a target-specific normal dependency (kind: null, target: cfg(...)) is included -- #1224\'s exact shape',
    fn: () => {
      const meta = makeMetadata([
        {
          name: 'libc',
          kind: null,
          target: 'cfg(target_os = "macos")',
          rename: null,
        },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      assert.ok(
        deps.includes('libc'),
        `expected 'libc' (kind: null, target: a macOS cfg predicate) to be found; got ${JSON.stringify(deps)}`,
      );
    },
  },
  {
    name: 'a dev dependency (kind: "dev") is excluded, whatever its target',
    fn: () => {
      const meta = makeMetadata([
        { name: 'serde', kind: null, target: null, rename: null },
        { name: 'tempfile', kind: 'dev', target: null, rename: null },
        { name: 'proptest', kind: 'dev', target: 'cfg(target_os = "macos")', rename: null },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      assert.ok(deps.includes('serde'), 'the real normal dependency must still be found');
      assert.ok(!deps.includes('tempfile'), '[dev-dependencies] must be excluded');
      assert.ok(!deps.includes('proptest'), 'a per-target [dev-dependencies] entry must also be excluded');
    },
  },
  {
    name: 'a build dependency (kind: "build") is excluded, whatever its target',
    fn: () => {
      const meta = makeMetadata([
        { name: 'serde', kind: null, target: null, rename: null },
        { name: 'tauri-build', kind: 'build', target: null, rename: null },
        { name: 'cc', kind: 'build', target: 'x86_64-pc-windows-msvc', rename: null },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      assert.ok(deps.includes('serde'), 'the real normal dependency must still be found');
      assert.ok(!deps.includes('tauri-build'), '[build-dependencies] must be excluded');
      assert.ok(!deps.includes('cc'), 'a per-target [build-dependencies] entry must also be excluded');
    },
  },
  {
    name: 'a renamed dependency is reported under its real crate name, not its local alias',
    fn: () => {
      // Cargo.toml equivalent: `my-alias = { package = "actual-crate", version = "1" }`.
      // `cargo metadata` reports the real crate as `name` and the local
      // alias as `rename` -- ACKNOWLEDGEMENTS.md, and the licence this
      // dependency actually carries, belong to the real crate.
      const meta = makeMetadata([
        { name: 'actual-crate', kind: null, target: null, rename: 'my-alias' },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      assert.ok(
        deps.includes('actual-crate'),
        `expected the real crate name 'actual-crate'; got ${JSON.stringify(deps)}`,
      );
      assert.ok(!deps.includes('my-alias'), 'the local alias must never appear in the result');
    },
  },
  {
    name: 'a dependency listed under both the plain table and a per-target table is counted once',
    fn: () => {
      const meta = makeMetadata([
        { name: 'serde', kind: null, target: null, rename: null },
        { name: 'serde', kind: null, target: 'cfg(target_os = "macos")', rename: null },
      ]);
      const deps = directDepsFromCargoMetadata(meta, FIXTURE_MANIFEST_PATH);
      const occurrences = deps.filter((d) => d === 'serde').length;
      assert.equal(
        occurrences,
        1,
        `expected 'serde' to appear exactly once; got ${occurrences} in ${JSON.stringify(deps)}`,
      );
    },
  },
  {
    name: 'a lookup for a manifest_path cargo never reported throws rather than silently returning an empty list',
    fn: () => {
      const meta = makeMetadata([{ name: 'serde', kind: null, target: null, rename: null }]);
      assert.throws(
        () => directDepsFromCargoMetadata(meta, '/some/other/Cargo.toml'),
        /no package with manifest_path/,
        'a mismatched manifest_path must be a loud error, never an empty result that looks like ' +
          '"this project has zero dependencies"',
      );
    },
  },
  {
    name: 'INTEGRATION: the real cargo metadata --no-deps on src-tauri/Cargo.toml finds libc (macOS-only, #1224\'s exact real-world case) and excludes tempfile (a real [dev-dependencies] entry)',
    fn: () => {
      // This shells out to the real `cargo` on PATH against the real
      // repo file -- proving Cargo's own reading actually does cover
      // the per-target table shape #1224 was about, not just a fixture
      // built to describe it. If `cargo` is not on PATH, this test
      // fails loudly (readDirectRustDeps calls process.exit(1) with a
      // clear message) rather than being silently skipped -- consistent
      // with the "must fail, never silently degrade" rule for every
      // caller of this module.
      const deps = readDirectRustDeps(REAL_CARGO_TOML);
      assert.ok(
        deps.includes('libc'),
        `expected 'libc' (declared only under [target.'cfg(target_os = "macos")'.dependencies] ` +
          `in the real src-tauri/Cargo.toml) to be found; got ${deps.length} deps total`,
      );
      assert.ok(
        !deps.includes('tempfile'),
        "'tempfile' is a real [dev-dependencies] entry in src-tauri/Cargo.toml and must not be " +
          'treated as a direct (shipped) dependency',
      );
      assert.ok(
        !deps.includes('tauri-build'),
        "'tauri-build' is a real [build-dependencies] entry in src-tauri/Cargo.toml and must not " +
          'be treated as a direct (shipped) dependency',
      );
    },
  },
];

let failures = 0;
for (const { name, fn } of tests) {
  try {
    fn();
    console.log(`  ok - ${name}`);
  } catch (err) {
    failures += 1;
    console.error(`  FAIL - ${name}`);
    console.error(`    ${err.message}`);
  }
}

if (failures) {
  console.error(`\n✗ ${failures} of ${tests.length} test(s) failed.`);
  process.exit(1);
}
console.log(`\n✓ All ${tests.length} test(s) passed.`);
process.exit(0);
