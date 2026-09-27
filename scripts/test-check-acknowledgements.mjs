#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

// Regression test for #1224: a dependency declared for only one
// platform, inside a `[target.'cfg(target_os = "macos")'.dependencies]`
// table in src-tauri/Cargo.toml rather than the plain `[dependencies]`
// table, used to be invisible to both `check-acknowledgements.mjs`
// (#802) and `check-upstream-licences.mjs` (#806). Both scripts parsed
// Cargo.toml with a hand-rolled reader that only ever recognised the
// literal table header `[dependencies]` — a per-target table's rows
// were skipped in exactly the same way a `[dev-dependencies]` row is
// skipped on purpose, except nobody meant to skip them.
//
// There is no existing test harness for the plain-Node scripts in this
// folder (only the frontend has Vitest), so this file is deliberately
// dependency-free: Node's own built-in `assert` module, run directly
// with `node scripts/test-check-acknowledgements.mjs`. It is not wired
// into `package.json` because none of the other check scripts' tests
// are either -- there is nothing to match the style of.
//
// Run: `node scripts/test-check-acknowledgements.mjs`
// Exit 0 = every assertion passed. Exit 1 = at least one failed (the
// failing assertion's message is printed).

import assert from 'node:assert/strict';
import { parseCargoDirectDepsFromText as parseAckDeps } from './check-acknowledgements.mjs';
import { parseCargoDirectDepsFromText as parseUpstreamDeps } from './check-upstream-licences.mjs';

// A minimal Cargo.toml shaped like the real src-tauri/Cargo.toml's tail
// end: a plain `[dependencies]` table, followed by a `[dev-dependencies]`
// table, followed by the macOS-only `libc` dependency under a
// single-quoted `cfg(...)` target table -- the exact shape that exposed
// #1224 in the first place.
const SAMPLE_SINGLE_QUOTED_CFG = `
[dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"

[target.'cfg(target_os = "macos")'.dependencies]
libc = "0.2"
`;

// The same shape, but with the target predicate written in double
// quotes instead of single quotes (with the embedded double quotes
// escaped) -- the other table-header form Cargo actually accepts.
const SAMPLE_DOUBLE_QUOTED_CFG = `
[dependencies]
serde = "1"

[target."cfg(target_os = \\"windows\\")".dependencies]
winreg = "0.52"
`;

// A per-target table keyed by a bare target triple, which needs no
// quoting at all because every character in a triple is already a
// legal bare TOML key.
const SAMPLE_BARE_TARGET_TRIPLE = `
[dependencies]
serde = "1"

[target.x86_64-pc-windows-msvc.dependencies]
winapi = "0.3"
`;

// The same dependency expressed as a single dotted-key assignment
// instead of a "[...]" table header -- valid TOML that nothing in this
// repo's real Cargo.toml uses today, but which the parser is written to
// accept anyway so a future edit can't reopen #1224 by choosing this
// form instead.
const SAMPLE_DOTTED_KEY = `
[dependencies]
serde = "1"

target.'cfg(target_os = "macos")'.dependencies.libc = "0.2"
`;

// A dependency that appears in BOTH the plain table and a per-target
// table must be counted once, not twice -- the task's explicit
// de-duplication requirement.
const SAMPLE_DUPLICATE_ACROSS_TABLES = `
[dependencies]
serde = "1"

[target.'cfg(target_os = "macos")'.dependencies]
serde = "1"
`;

// A per-target BUILD/DEV dependency table must still be ignored, the
// same way the plain [build-dependencies] / [dev-dependencies] tables
// already are -- this script's existing scope (deps that ship inside
// the binary) must not be widened by the #1224 fix, only extended to
// cover every platform.
const SAMPLE_TARGET_DEV_DEPS_IGNORED = `
[dependencies]
serde = "1"

[target.'cfg(target_os = "macos")'.dev-dependencies]
proptest = "1"

[target.'cfg(target_os = "macos")'.build-dependencies]
cc = "1"
`;

const tests = [
  {
    name: 'check-acknowledgements: single-quoted cfg(...) target table is now seen (#1224)',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_SINGLE_QUOTED_CFG);
      // Before the fix, `deps` would have been ['serde', 'tokio'] only --
      // `libc` lives entirely inside the per-target table, which the old
      // parser never recognised as a dependency table at all.
      assert.ok(
        deps.includes('libc'),
        `expected 'libc' (declared only under [target.'cfg(target_os = "macos")'.dependencies]) ` +
          `to be found, but parseCargoDirectDepsFromText() returned: ${JSON.stringify(deps)}`,
      );
      assert.ok(deps.includes('serde') && deps.includes('tokio'), 'plain [dependencies] entries must still be found');
      assert.ok(!deps.includes('tempfile'), '[dev-dependencies] must still be excluded');
    },
  },
  {
    name: 'check-acknowledgements: double-quoted cfg(...) target table is seen',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_DOUBLE_QUOTED_CFG);
      assert.ok(deps.includes('winreg'), `expected 'winreg' to be found; got ${JSON.stringify(deps)}`);
    },
  },
  {
    name: 'check-acknowledgements: bare target-triple table (no quoting) is seen',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_BARE_TARGET_TRIPLE);
      assert.ok(deps.includes('winapi'), `expected 'winapi' to be found; got ${JSON.stringify(deps)}`);
    },
  },
  {
    name: 'check-acknowledgements: dotted-key form (no [...] header at all) is seen',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_DOTTED_KEY);
      assert.ok(deps.includes('libc'), `expected 'libc' to be found; got ${JSON.stringify(deps)}`);
    },
  },
  {
    name: 'check-acknowledgements: a dep in both the plain and a target table is counted once',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_DUPLICATE_ACROSS_TABLES);
      const occurrences = deps.filter((d) => d === 'serde').length;
      assert.equal(occurrences, 1, `expected 'serde' to appear exactly once; got ${occurrences} in ${JSON.stringify(deps)}`);
    },
  },
  {
    name: 'check-acknowledgements: per-target dev/build-dependencies stay excluded',
    fn: () => {
      const deps = parseAckDeps(SAMPLE_TARGET_DEV_DEPS_IGNORED);
      assert.ok(!deps.includes('proptest'), '[target....dev-dependencies] must not be read');
      assert.ok(!deps.includes('cc'), '[target....build-dependencies] must not be read');
    },
  },
  {
    name: 'check-upstream-licences: shares the identical fix (same blind spot, same table shapes)',
    fn: () => {
      const deps = parseUpstreamDeps(SAMPLE_SINGLE_QUOTED_CFG);
      assert.ok(
        deps.includes('libc'),
        `expected 'libc' to be found by check-upstream-licences.mjs's parser too; got ${JSON.stringify(deps)}`,
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
