#!/usr/bin/env node
// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

// Shared reader for MeedyaDL's DIRECT Rust dependencies -- the crate
// names in `src-tauri/Cargo.toml`'s `[dependencies]` table AND every
// per-target `[target.<spec>.dependencies]` table, but never
// `[dev-dependencies]` or `[build-dependencies]` (plain or per-target),
// because neither of those ships inside the compiled binary. Used by
// both `check-acknowledgements.mjs` (#802, set-coverage) and
// `check-upstream-licences.mjs` (#806, licence-string drift) -- they
// both need exactly this same list, so it lives in one place rather
// than being copy-pasted twice.
//
// WHY THIS ASKS CARGO INSTEAD OF READING THE FILE ITSELF (#1224 follow-up)
// =========================================================================
// The first version of this reader (removed by this file) was a
// hand-rolled regular-expression parser of the raw Cargo.toml TEXT. It
// was extended once already, under #1224, to recognise several header
// shapes Cargo accepts for a per-target table (single-quoted,
// double-quoted, a bare target triple) plus a dotted-key form written
// with no `[...]` header at all. A Codex review of THAT fix reproduced
// two further faults, and both are inherent to hand-parsing TOML with
// regular expressions -- not two more shapes to add to a list that
// would only ever grow:
//
//   1. Valid TOML the regex still could not read: a comment trailing a
//      table header on the same line
//      (`[target.'cfg(unix)'.dependencies] # Unix only`), a dependency
//      declared as its own SUB-TABLE
//      (`[target.'cfg(unix)'.dependencies.libc]` with `version = "0.2"`
//      on the next line, rather than one `libc = "0.2"` line), spaces
//      around the dots inside a dotted key
//      (`target . 'cfg(unix)' . dependencies . libc = "1"`), a quoted
//      dependency name (`"my-crate" = "1"`), and an inline table with
//      trailing content the line-based regex never anticipated.
//
//   2. A dotted assignment ignores which TABLE it is actually inside.
//      TOML dotted keys are valid ANYWHERE, not just where this project
//      happens to use them -- so `target.'cfg(unix)'.dependencies.fake
//      = "1"` sitting under `[package.metadata]` (nothing to do with
//      real dependencies at all) was wrongly counted as one, because
//      the regex matched the LINE'S SHAPE without checking which
//      section it was actually inside. (The regression test this
//      replaced even placed its own dotted-key fixture directly below
//      `[dependencies]`, which is exactly the section the mistake would
//      hide inside -- it could never have caught this on its own.)
//
// Neither fault is a missing case to patch. Both are the fundamental
// problem with reading TOML by pattern-matching lines of text instead
// of a real parser that actually understands TOML's table-nesting and
// section-scoping rules -- there will always be another valid TOML
// shape a regex has not been taught. Cargo itself already contains
// exactly that real parser, because it has to load this same file to
// build the project. `cargo metadata` prints back the answer Cargo's
// own parser already computed, so what this script reads can never
// disagree with what Cargo itself will do with the same file. Asking
// Cargo removes the entire class of fault rather than patching one more
// instance of it.
//
// CONFIRMED NO-NETWORK: `cargo metadata --no-deps` reads only this
// manifest's own declared dependency EDGES -- crate names, version
// requirements, target predicates -- it never resolves those against
// the registry or downloads anything. Verified on this machine twice:
// once with `--offline` added (exit 0, ~0.05s, i.e. no different from
// without it), and once with `CARGO_HOME` pointed at a brand-new empty
// directory holding no registry index or cache at all -- `cargo
// metadata --no-deps` still exits 0 immediately, with or without
// `--offline`. `--offline` is passed below anyway, as a standing
// guarantee rather than a one-off observation: if a future Cargo
// release ever needed the registry for some `--no-deps` edge case this
// project doesn't use today, `--offline` turns that into a loud failure
// here rather than a silent, occasional network dependency in a check
// that is supposed to run everywhere with zero setup.

import { execFileSync } from 'child_process';
import { resolve } from 'path';

/**
 * Extract direct dependency names for ONE package out of the JSON
 * `cargo metadata --no-deps --format-version 1` prints.
 *
 * A pure function on purpose -- no file I/O, no subprocess, so a test
 * can hand it a small literal object and get a deterministic answer,
 * the same way `parseCargoDirectDepsFromText` used to take a text
 * string instead of a file path (see the two calling scripts' git
 * history). `readDirectRustDeps` below is the impure half that
 * actually runs `cargo` and hands its output to this function.
 *
 * @param {object} metadataJson - the PARSED JSON `cargo metadata`
 *   printed (the caller does `JSON.parse` once; this function never
 *   touches a string). Must have the shape cargo's own
 *   `--format-version 1` schema documents: a top-level `packages`
 *   array, each entry with `manifest_path` and a `dependencies` array
 *   of `{ name, kind, target, rename, ... }` objects.
 * @param {string} rootManifestPath - the manifest whose OWN direct
 *   dependencies we want, as an absolute path. Cargo always reports
 *   `manifest_path` as an absolute, canonicalised path in its JSON
 *   output regardless of how `--manifest-path` was spelled on the
 *   command line (relative, `~`-relative, etc.), so the caller must
 *   resolve this the same way before comparing -- `readDirectRustDeps`
 *   does that with `resolve()` from `node:path`.
 * @returns {string[]} de-duplicated crate names. Only entries whose
 *   `kind` is `null` (a NORMAL / runtime dependency, in Cargo's own
 *   vocabulary) are included, whatever their `target` (`null` for the
 *   plain `[dependencies]` table, or a `cfg(...)` predicate / target
 *   triple string for a per-target table) -- a macOS-only dependency
 *   like `libc` is just as much a shipped dependency as one with no
 *   target restriction at all; it only ships on fewer platforms.
 *   Entries whose `kind` is `"dev"` (`[dev-dependencies]`) or
 *   `"build"` (`[build-dependencies]`) are always excluded, plain or
 *   per-target, because neither ships inside the compiled binary --
 *   that was this reader's scope before #1224 and the per-target fix
 *   does not widen it.
 * @throws if no package in `metadataJson.packages` has a
 *   `manifest_path` matching `rootManifestPath` exactly -- this is a
 *   programming-usage error (the caller asked for a manifest cargo
 *   never reported), not a "this project has zero dependencies"
 *   outcome, so it must never be swallowed into an empty result.
 */
export function directDepsFromCargoMetadata(metadataJson, rootManifestPath) {
  const rootPkg = metadataJson.packages.find((pkg) => pkg.manifest_path === rootManifestPath);
  if (!rootPkg) {
    const known = metadataJson.packages.map((pkg) => pkg.manifest_path).join(', ') || '(none)';
    throw new Error(
      `cargo metadata output has no package with manifest_path ${JSON.stringify(rootManifestPath)}. ` +
        `Packages actually present: ${known}`,
    );
  }

  const deps = new Set();
  for (const dep of rootPkg.dependencies ?? []) {
    // "kind" is `null` for a normal (runtime) dependency, `"dev"` for
    // one declared under `[dev-dependencies]`, `"build"` for one under
    // `[build-dependencies]` -- in every table shape, plain or
    // per-target alike. Only `null` ships inside the compiled binary.
    if (dep.kind !== null) continue;
    // Cargo lets one Cargo.toml line rename the crate it depends on
    // (`foo = { package = "real-name", version = "1" }`), reported as
    // `dep.rename`. `dep.name` is always the REAL crate name Cargo
    // actually compiles and links, regardless of any local alias, and
    // that real name is what a licence obligation belongs to and what
    // ACKNOWLEDGEMENTS.md needs to list -- not whatever convenience
    // alias one call site chose.
    deps.add(dep.name);
  }
  return [...deps];
}

/**
 * Run `cargo metadata --no-deps` against `manifestPath` and return the
 * de-duplicated list of direct dependency names that ship inside the
 * compiled binary (see `directDepsFromCargoMetadata` above for exactly
 * which ones that is).
 *
 * This is the only place either calling script shells out to `cargo`
 * for THIS purpose (getting the direct-dependency name list); the
 * separate, FULL (non-`--no-deps`) `cargo metadata` call in
 * `check-upstream-licences.mjs` that reads every package's LICENCE
 * STRING is a different concern and stays where it is.
 *
 * MUST FAIL LOUDLY, NEVER DEGRADE TO "NO DEPENDENCIES": if `cargo` is
 * missing from PATH, or the manifest is invalid, or cargo's own output
 * cannot be parsed as JSON, this prints a plain-English explanation to
 * stderr and calls `process.exit(1)` directly -- it never returns an
 * empty array for that case. An empty array here would silently exempt
 * EVERY direct dependency from both the ACKNOWLEDGEMENTS.md coverage
 * check and the upstream-licence-string check, and the caller would
 * have no way to tell "cargo failed" apart from "this project really
 * has zero dependencies" -- exactly the "reported success after
 * checking nothing" failure mode this project has already had to
 * design against elsewhere (see `channel-security-audit.yml` and the
 * #1146 discipline it follows).
 *
 * @param {string} manifestPath - path to the Cargo.toml to read
 *   (relative or absolute; resolved to absolute before comparing
 *   against cargo's own reported `manifest_path`, and before handing
 *   to `cargo` on the command line too, so behaviour does not depend on
 *   the caller's current working directory).
 * @returns {string[]} de-duplicated direct dependency crate names.
 */
export function readDirectRustDeps(manifestPath) {
  const absoluteManifestPath = resolve(manifestPath);

  let raw;
  try {
    // `--offline`: see the file-header comment above -- confirmed on
    // this machine that `--no-deps` never needs the network or even a
    // populated registry cache, so asserting that explicitly here turns
    // any future surprise into a loud failure rather than an occasional
    // silent network call.
    raw = execFileSync(
      'cargo',
      [
        'metadata',
        '--no-deps',
        '--format-version',
        '1',
        '--manifest-path',
        absoluteManifestPath,
        '--offline',
      ],
      { maxBuffer: 32 * 1024 * 1024, encoding: 'utf8' },
    );
  } catch (e) {
    console.error('::error::cargo metadata --no-deps failed -- cannot read direct Rust dependencies.');
    console.error(`    ${e.message}`);
    console.error('    Make sure `cargo` is on PATH (e.g. `export PATH="$HOME/.cargo/bin:$PATH"`)');
    console.error(`    and that ${absoluteManifestPath} is valid TOML, then re-run. This exits`);
    console.error('    non-zero on purpose: a failed read here must never be treated as "no');
    console.error('    dependencies" -- that would silently exempt every direct dependency from');
    console.error('    both licence checks.');
    process.exit(1);
  }

  let metadataJson;
  try {
    metadataJson = JSON.parse(raw);
  } catch (e) {
    console.error('::error::cargo metadata printed output that was not valid JSON.');
    console.error(`    ${e.message}`);
    process.exit(1);
  }

  return directDepsFromCargoMetadata(metadataJson, absoluteManifestPath);
}
