# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### ✨ Features

- Legacy folder merge + colour-coded activity log (closes #789, #793) (#794)

## Summary

  Two user-facing improvements bundled for **v1.4.6**:

  ### 1. Legacy sibling-folder merge (#789)
  For users with pre-#528 downloads on disk, the **Library** page now has
  an opt-in **Merge Legacy Folders** tool that reconciles sibling album
  folders (e.g. `Album` + `Album [Explicit]`) into a single canonical
  folder, with collision-safe file moves and automatic `.meedyadl`
  manifest merging.

  - Three-phase API (`detect` → `preview` → `execute`) — no destructive
  action without explicit confirmation.
  - Defensive verification: each pair's `[Explicit]` / `[Clean]` suffix is
  cross-checked against the actual `rtng` atom in the audio files.
  Mismatches are rejected to avoid mis-merging unrelated folders that
  happen to share a name pattern.
  - `.meedyadl` manifest merge dedup key bumped from `(platform, url)` to
  `(platform, url, codec)` so a folder that contains both ALAC and Atmos
  downloads of the same album records both source entries instead of
  clobbering one.
  - Collision-safe file moves via `fs_safe::safe_rename` with
  auto-disambiguation (`Cover.jpg` + `Cover.jpg` → `Cover.jpg` + `Cover
  (1).jpg`).
  - 11 unit tests covering detection, classification, manifest merge,
  advisory verification, and collision handling.

  ### 2. Colour-coded Activity Log (#793)
  Activity Log entries now render in theme-aware colours that reflect
  their severity — **errors in red, warnings in amber**, info in the
  default content colour.

  - Uses MeedyaDL's existing design tokens (`text-status-error` /
  `text-status-warning` / `text-content-primary`), so the colour mapping
  adapts to light, dark, high-contrast, and colour-blind themes without
  any per-theme overrides.
  - New `LogSeverity` enum (`info` / `warning` / `error`) added to
  `ActivityLogEvent` as an optional field, serialised lowercase. Existing
  emit sites keep working unchanged (default: `Info`).
  - Subprocess output gets severity inferred from GAMDL's structlog prefix
  (`[WARNING HH:MM:SS]`, `[ERROR HH:MM:SS]`, `[CRITICAL …]`). Stderr
  without a prefix defaults to Warning; stdout defaults to Info.
  - Four new emit helpers (`emit_download_warn`, `emit_download_error`,
  `emit_app_warn`, `emit_app_error`) let high-impact failure sites opt in.
  Five sites migrated: filesystem errors, terminal download failures (both
  Err and success-path), and music-video lookup warnings.
  - 9 new unit tests cover severity serialisation, default behaviour,
  GAMDL prefix detection (WARNING / ERROR / CRITICAL / INFO), and the
  conservative "no false positives from keyword mentions" guarantee.
  - Exported activity logs remain plain text. A future HTML-with-CSS
  export is straightforward to add now that severity is on the event
  struct.

  ## Test plan
  - [x] `cargo check` clean
  - [x] `cargo clippy --lib -- -D warnings` clean
  - [x] `cargo test --lib` — 201 download_queue tests, 9 activity_log
  severity tests, 11 legacy_folder_merge tests, 7 manifest tests, 3
  activity_log_writer tests — all pass
  - [x] `npx tsc --noEmit` clean
  - [x] `npm test -- --run` — 482 frontend tests pass
  - [ ] Manual: open Library page → Merge Legacy Folders → pick a folder
  with mixed `[Explicit]` siblings → verify preview matches expectation →
  execute → confirm files moved and manifest merged
  - [ ] Manual: trigger a download failure (e.g. invalid URL) → confirm
  Activity Log entry renders in red
  - [ ] Manual: trigger a GAMDL warning (e.g. fallback codec) → confirm
  Activity Log entry renders in amber
  - [ ] Manual: switch UI theme (light/dark/high-contrast/colour-blind) →
  confirm severity colours remain legible in each


### 📚 Documentation

- **(security)** Update supported versions to 1.4.5 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.5.0 [skip ci]

## [1.4.5] - 2026-05-17

### 🐛 Bug Fixes

- Companion sidecar rename + accurate per-item progress bar (#788, #790) (#791)

### 📚 Documentation

- **(security)** Update supported versions to 1.4.4 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [1.4.4] - 2026-05-16

### 🐛 Bug Fixes

- Companion folder merge + fully parallel enrichment (#528, #779) (#786)

Two fixes shipping together as v1.4.4:

  | Closes | Title | Commit |
  |---|---|---|
  | #528 | fix: companion + advisory suffix no longer produces two sibling
  folders | `dc579e6` |
  | #779 | perf: enrichment fully parallel via per-file write locks
  (Option 2) | `327752a` |

  ## What's new (user-facing)

  - **Companion downloads now merge with the primary album.** When you
  download an Explicit album with companion codecs enabled (e.g. Atmos
  primary + ALAC companion), MeedyaDL no longer leaves you with two
  sibling folders (`Album/` and `Album [Explicit]/`) — both codec variants
  now land in the single `Album [Explicit]/` folder per the user's
  expectation. The per-file `[Lossless]` / `[Dolby Atmos]` codec suffixes
  already prevent filename collisions inside that one folder. (#528)

  - **Enrichment is roughly 40-50% faster on heavy albums.** AcoustID
  fingerprinting, MusicBrainz ISRC lookup, and ReplayGain analysis now run
  **fully in parallel** instead of staged. On a 19-track live album with
  both AcoustID and ReplayGain enabled, total wall time drops from ~210 s
  → ~120 s. v1.4.3 only parallelised AcoustID + MusicBrainz (Option 1);
  this PR completes the picture by adding per-file write coordination so
  ReplayGain can run alongside without racing on `mp4ameta` tag writes.
  (#779)

  ---


### 📚 Documentation

- **(security)** Update supported versions to 1.4.3 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [1.4.3] - 2026-05-15

### 🐛 Bug Fixes

- Combined MV / enrichment / queue / UX fixes (11 issues closed) (#781)

Combined PR superseding #777, #778, #780, plus a string of additional MV
  / enrichment / queue / UX fixes added in the same review window. Closes
  11 issues.

  | Closes | Title | Commit |
  |---|---|---|
  | #774 | fix(downloads): stop the no-op MV cover-art retry | `e17ea81` |
  | #775 | fix(metadata): real music-video names in activity log |
  `2660574` |
  | #776 | perf(enrichment): unified dynamic timeout (tracks + tiers +
  MVs) | `3d80315` + `0668875` |
  | #779 | perf(enrichment): parallelise AcoustID + MusicBrainz lookup
  (Option 1) | `1239b33` |
  | Cluster #5 | fix: stop claiming MV companions completed when they
  didn't | `deaef79` |
  | #771 | fix(release): version-bump.yml now creates the git tag |
  `b350ac3` |
  | #568 | fix(parser): rewrite legacy iTunes URLs so GAMDL accepts them |
  `da640c0` |
  | #782 | feat(queue): reorder pending items live via right-click |
  `d2ae5cc` |
  | #467 | perf: virtualize the queue list for large queues | `5af5654` |
  | #689 | perf: memoise QueueItem rows with field-aware comparator |
  `4081628` |
  | #574 | feat(ux): per-track captions for AcoustID + ReplayGain |
  `1d207b0` |


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.4.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Bump release-please manifest to 1.4.2 (#784)

## Why

  `.release-please-manifest.json` was stuck at `1.4.1` because v1.4.2 was
  tagged and released manually on 2026-05-15 (recovery from the
  `version-bump.yml` tag-creation gap, since fixed in #771). The manual
  tag bypassed release-please, so the manifest never got updated.

  ## What broke

  When release-please ran after PR #781 merged, it computed `1.4.1 + 1 fix
  = 1.4.2` and opened **#783** proposing a duplicate v1.4.2 release. Wrong
  on two counts:

  - **Version**: v1.4.2 is already published.
  - **Body**: just the squash-merge subject from #781 — too vague for an
  in-app changelog.

  #783 closed; this PR fixes the baseline.

  ## What this fixes

  - Bumps `.release-please-manifest.json` from `1.4.1` → `1.4.2` so
  release-please's next run computes the correct next version (`1.4.3`).
  - Source-of-truth versions in `package.json` / `tauri.conf.json` /
  `Cargo.toml` were already at `1.4.2` — only the manifest needed catching
  up.

  ## After this merges

  I'll trigger the `Release Please` workflow manually so the new v1.4.3 PR
  opens with a fresh body, then write a richer user-facing changelog
  directly into that PR's body before merging.

  ## Test plan

  - [x] Single-line manifest edit, no behavioural change.
  - [ ] CI runs (no functional code touched, but matrix runs anyway for
  hygiene).

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


## [1.4.2] - 2026-05-15

### 🐛 Bug Fixes

- **(test)** Drop rerender() that flakes on Windows CI (#765)

## Summary

  The Windows GitHub Actions runner has been intermittently timing out on
  a single ActivityLog test — `'subtitle pluralises lines correctly'` —
  while macOS and Ubuntu pass cleanly. Latest hit: the v1.4.1 release
  commit (df6549c) on main, breaking the post-merge CI run.

  ## Root cause

  The test called:

  ```tsx
  const { rerender } = render(<ActivityLog />);
  expect(screen.getByText('1 line')).toBeInTheDocument();

  act(() => { useActivityStore.setState({ entries: [...] }); });
  rerender(<ActivityLog />);
  expect(screen.getByText('2 lines')).toBeInTheDocument();
  ```

  The explicit `rerender()` forces a remount-style render. `ActivityLog`
  uses `@tanstack/react-virtual` which re-measures DOM nodes on remount.
  jsdom's DOM measurement is slower on the Windows runner than on macOS /
  Ubuntu (a known win32 Node test-harness quirk), and the remeasure
  occasionally exceeded the default 5000ms test timeout.

  ## Fix

  Drop the `rerender()` call. The component subscribes to the Zustand
  activity store via `useActivityStore`, so a `setState` inside `act()`
  triggers a normal React re-render — no need to remount. Keeps the
  virtualiser instance stable across the two assertions.

  ## Verification

  - [x] `npm test -- --run src/components/download/ActivityLog.test.tsx` —
  18 / 18 pass locally.
  - [ ] CI runs the full Windows + macOS + Ubuntu Frontend matrix on this
  PR.

  ## Why a separate PR

  Following the new "branch + PR for everything except `[skip ci]`
  doc/chore" rule established in PR #760's retrospective. The fix is
  one-test-file but the policy applies uniformly.

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(ci)** Pin Backend matrix macos slot to macos-14 + shim guard (#770)

## Summary

  Backend (macos-latest) on every push since the macos-latest → macos-15
  runner image rotation in mid-2026 has been failing instantly at `cargo
  check`. The job duration is ~1m, the failure step (`Cargo check`) takes
  0s, and the error is:

  ```
  Run cargo check
  error: error: unexpected argument 'check' found
  Usage: rustup-init[EXE] [OPTIONS]
  For more information, try '--help'.
  Error: Process completed with exit code 1.
  ```

  ## Root cause

  The macos-15 runner image ships a Homebrew `cargo` shim at
  `/opt/homebrew/bin/cargo` that proxies to `rustup-init` when rustup
  isn't fully initialised. The current `dtolnay/rust-toolchain@631a55b`
  pin installs rustup at `~/.cargo/` and adds `~/.cargo/bin` to
  `$GITHUB_PATH`, but the Homebrew shim wins the PATH race. When the
  workflow then runs `cargo check`, the shim invokes `rustup-init check`,
  which doesn't recognise `check` as a valid arg → instant failure.

  This isn't a project code issue — it's a runner-image regression. Ubuntu
  and Windows are unaffected (no Homebrew shim). Frontend (macos-latest)
  is unaffected because that matrix never runs cargo.

  ## Evidence

  - Failed on PR #769 head commit `27b7000` (Backend macOS, 1m9s) — every
  other backend platform passed against the same code.
  - Failed again on the post-merge main commit `04a9798` (CI run #1400) —
  also 1m4s, same `rustup-init` banner.
  - Two consecutive failures on different shas with the same fingerprint
  rules out one-shot flake.
  - Job log screenshot confirms `Setup Node.js` (4s ✓), `Install npm
  dependencies` (12s ✓), `Build frontend` (8s ✓), `Cargo check` (0s ❌).

  ## Fix


### 📚 Documentation

- **(security)** Update supported versions to 1.4.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- **(gamdl)** Admit v3.5.2 to support window (#769)

## [1.4.1] - 2026-05-11

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.4.0 [skip ci]

### 🔧 Refactoring

- **(settings)** Finish useSettingsField migration — closes #757 (#763)

## Summary

  Completes the **audit v2 finding #6** migration. With this PR, all 9
  user-facing settings tabs use the `useSettingsField` hook for per-field
  Zustand bindings. Replaces the `settings.X` + `updateSettings({ X: v })`
  lambda pair with `field.value` + `field.set` — each control re-renders
  only when its bound field changes.

  ## Tabs migrated in this PR (4 of the original 9)

  - **`CookiesTab.tsx`** — single field (`cookies_path`); validation +
  browser-import handlers all migrate cleanly.
  - **`QualityTab.tsx`** — 12 fields including the nested
  `duplicate_detection` object (handled via `dupDetect.set({
  ...dupDetect.value, key: v })`) and the artist-auto-select pair where
  the multi-select array AND the legacy scalar both fire `.set()` per
  change.
  - **`GeneralTab.tsx`** — 21 fields. `useSettingsStore` retained only for
  the `loadSettings` action (used by the import flow).
  - **`AdvancedTab.tsx`** — 20 fields, plus the `DevToolsSection`
  sub-component which migrates its two MusicKit reads. `useSettingsStore`
  retained for the two non-field actions: `saveSettings` (setup-wizard
  reset) and `loadSettings` (dev-access deactivate).

  Combined with the 5 tabs already migrated in 815a0ce (`TemplatesTab`,
  `FallbackTab`, `CoverArtTab`, `LyricsTab`, `ToolsTab`), all 9 settings
  tabs are now on the new pattern.

  ## Why useSettingsField wins

  - **Per-field subscriptions** — Zustand re-renders only the controls
  bound to a field that changed, not the entire tab.
  - **Type-safe key access** — typos are compile-time errors
  (`useSettingsField('xyz_typo')` won't compile).
  - **No key duplication** — the field key is supplied once at the hook
  call, not twice (read + write).
  - **Memoised setters** — `.set` is stable across renders so memoised
  consumers don't re-render on parent re-render.

  ## Verification

  - [x] `npx tsc --noEmit` clean.
  - [x] `npm test -- --run` — 465 tests pass across 31 files.
  - [ ] CI runs the full Backend + Frontend matrix on this PR.

  ## Branch base

  This branch was created off `main` BEFORE PR #762 landed the clippy fix.
  CI will fail on the same Backend clippy lint until #762 merges and this
  branch rebases. Order of operations:

  1. Merge #762 (clippy fix + Release-As trailer).
  2. Rebase or merge `main` into this branch.
  3. CI re-runs and passes.
  4. Merge this PR.


## [1.4.0] - 2026-05-11

### ✨ Features

- **(activity-log)** Emit per-download GAMDL version + capability flags (#755)

Adds a one-liner to each queue item's activity log stream identifying
  the GAMDL version and active capability flags at the moment that item
  ran. Surfaces in both the Tauri-event activity log and the on-disk
  file, so any subsequent crash report can be correlated to the exact
  GAMDL release that produced it.

  - New `active_capabilities_summary()` in `gamdl_capabilities.rs`
    returns a compact comma-separated list of currently-supported
    feature gates ("native_codec_priority, wrapper_m3u8_ip, …").
  - Reads the existing process-global version cache — no extra
    subprocess spawn per item.
  - Emission lives next to the existing "Authentication: …" line for
    consistent download-start framing.
  - Three new unit tests cover v3.5, v2.x, and uncached states.

- **(cover-art)** RAW → PNG → JPEG fallback when GAMDL cover write fails (#756)

When `cover_format = raw`, GAMDL occasionally fails the upstream
  `httpx` cover-bytes fetch and leaves the album folder with no static
  cover sidecar — though the embedded cover atom inside each M4A is
  unaffected. The Python traceback noise reported alongside that bug
  was the visible symptom; the missing `Cover.raw` was the underlying
  loss.

  This adds a deterministic post-download fallback chain that runs
  during the enrichment pipeline:

  1. **Fast path**: any `Cover.<ext>` (or user-stem `<X>.<ext>`) ≥ 4 KiB
     in any of `.raw` / `.png` / `.jpg` is treated as a successful
     GAMDL write — silent skip, no outbound request.
  2. **Fallback fetch**: when nothing valid is on disk, the Apple
     Music artwork URL template (now extracted from the API response
     into `AlbumMetadata::artwork_url_template` + `_width` + `_height`)
     is substituted with `{f}=png` and fetched. Failure → retry with
     `{f}=jpg`. Both write atomically (temp + rename).
  3. **All-failed path**: surfaced as an activity-log notice, with the
     reminder that the embedded cover atom in M4A is unaffected.

  Why post-download fetch rather than re-running GAMDL: re-running
  costs whole-album minutes; a single HTTP GET costs <1 second.

  Why RAW is excluded from the fallback chain: we cannot fabricate a
  RAW byte stream from the API (which serves PNG / JPEG depending on
  `{f}`). RAW is preserved on the fast path when GAMDL did write it.

- **(diagnostics)** Capture Python tracebacks as forensic reports (#758)

GAMDL and its Python deps (`httpx`, `async_lru`, `gamdl.interface`)
  occasionally raise multi-line tracebacks during otherwise-successful
  downloads — notably during cover-bytes fetch (especially with
  `cover_format = raw`, see #756), syllable-lyrics requests, and
  music-video relation lookups. The activity-log filter introduced in
  #660 suppresses the visual noise, but until now MeedyaDL had no way
  to aggregate or analyse these latent failures.

  This adds a forensic-capture layer that piggybacks on the existing
  crash-report infrastructure:

  - New `services/traceback_diagnostic` scans the per-download raw
    stdout/stderr buffer for traceback groups (header → frames → PEP
    657 source-code context → exception summary).
  - Identical groups are deduplicated with an occurrence count, so a
    19-track album where every track hits the same cover-bytes
    traceback reports as one entry with `count=19`, not 19 duplicates.
  - The scanner runs once per GAMDL invocation, on any exit path
    (success, error, soft-error). The healthy fast path is a single
    buffer scan + early return when no `Traceback (...)` header was
    observed — zero cost in the common case.
  - Captured tracebacks are written as a `CrashReport` with
    `source = "traceback_diagnostic"` via the existing
    `save_error_report` path. They surface in Settings → Advanced →
    Crash Reporting alongside other reports.
  - The URL stored in the report context is run through the existing
    `redact_url_query` helper so wrapper auth tokens never land in
    the diagnostic file.
  - A one-line activity-log notice is emitted on capture so users
    know to look in the Crash Reporting section.

  10 unit tests cover: single-group capture, duplicate dedup, PEP 657
  source-context lines (Python 3.11+), distinct groups stay separate,
  dangling tracebacks (process killed mid-stream), structlog interrupts
  mid-group, lone-header discard, and empty-input/no-traceback fast
  paths.

  New `is_python_exception_summary` helper exposed in `utils/process.rs`
  so the new module can recognise the closing line of a traceback
  group without re-implementing `PYTHON_EXCEPTION_REGEX`.


### 🐛 Bug Fixes

- **(enrichment)** Skip filesystem sidecars in BPM/lyrics/SRT/VTT/ASS walkers (#577)

The codec-detection (`metadata_tag_service`), ReplayGain
  (`replaygain_service`), and AcoustID (`acoustid_service`) walkers
  already filter macOS AppleDouble shadows (`._*`) and other sidecars via
  the shared `utils::fs_safe::is_filesystem_sidecar` helper. Several
  sister walkers in the same enrichment pipeline never got the same
  guard, so on exFAT/FAT32/HFS-formatted external drives they silently
  processed every `._Track.m4a` / `._Track.ttml` shadow alongside the
  real file — emitting parse failures, redundant subprocess spawns, and
  in some cases producing duplicate sidecar outputs.

  Adds the existing helper to the seven previously-unguarded walkers:

  - `bpm_service::analyze_directory_bpm` (silencedetect)
  - `enhanced_lyrics_service::process_enhanced_lyrics_for_directory`
  - `ass_subtitle_service::generate_ass_for_directory`
  - `webvtt_service::generate_webvtt_for_directory`
  - `rich_srt_service::generate_rich_srt_for_directory` + the
    embed-srt walker in the same file
  - `music_video_subtitle_service::copy_lyric_sidecars_for_video`
  - `download_queue::count_lyrics_files` (lyrics-coverage check)

  The two TTML scanners inside the syllable-lyrics upgrade path
  (download_queue.rs ~line 7099, ~7154) already filter implicitly via
  `name.starts_with("{:02} ", track_number)` — AppleDouble shadows
  start with `._` and never match the track-number prefix, so no
  explicit guard is needed there.

  No new test fixtures: the `is_filesystem_sidecar` predicate has full
  test coverage in `utils::fs_safe::tests`. Each walker now delegates to
  that single source of truth.

- Drop needless borrow on traceback url + override release to v1.3.3 (#762)

## Summary

  Two-in-one fix:

  1. **Clippy `needless_borrow` regression** breaking PR #761's three
  Backend CI checks. The `url_for_report` binding in
  [src/services/download_queue.rs:9523](src-tauri/src/services/download_queue.rs#L9523)
  is already `&str` (because `redact_url_query` returns a borrowed slice
  of its input), so passing `&url_for_report` created `&&str` and tripped
  clippy's lint under Rust 1.95 on CI. Local toolchains on rustc 1.93
  didn't catch this.

  2. **`Release-As: 1.3.3` trailer** — overrides release-please's
  automatic v1.4.0 calculation. The v1.3.2 batch contained `feat:` commits
  (#755, #756, #758) which by Conventional Commits semantics demand a
  minor bump, but the user-facing surface changes are small enough that a
  patch bump is preferred. Once this lands on main, release-please will
  recompute PR #761 to use v1.3.3 instead.

  ## Why no PR for the original v1.3.2 batch caught this

  The earlier batch landed via direct push to `main` (no PR-level CI). The
  v1.3.2 retrospective PR (#760) only diffs a markdown file, so the
  per-platform Backend matrix didn't run on the affected Rust code. Going
  forward, the new branch + PR rule should catch this kind of
  toolchain-specific regression at PR time.

  ## Test plan

  - [x] `cargo clippy -- -D warnings` clean locally on rustc 1.93.
  - [ ] CI re-runs the three Backend checks on this PR.
  - [ ] After merge, release-please refreshes PR #761 to title
  `chore(main): release 1.3.3` and updates the version files in the bot
  branch.

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


### 📚 Documentation

- **(security)** Update supported versions to 1.3.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.3.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(audits)** Retrospective sign-off for the v1.3.2 batch (#760)

## Summary

  The six commits that landed v1.3.2 (#755, #577, #756, #758, #757
  partial, version bump) were pushed fast-forward directly to `main`
  rather than through a PR. This PR is the **retrospective sign-off
  artifact** — a single reviewable surface for the batch.

  See
  [`.github/audits/v1.3.2-batch-retrospective.md`](.github/audits/v1.3.2-batch-retrospective.md)
  for the full breakdown: commit-by-commit summary, test verification,
  follow-up issues, and the process note explaining why no PR at commit
  time.

  ## Batch contents

  | SHA | Issue | Summary |
  | --- | --- | --- |
  | `169e708` | #755 | Per-download GAMDL version + capability flags in
  activity log |
  | `a891067` | #577 | Filesystem-sidecar guard extended to 7 walkers |
  | `f494a74` | #756 | Cover-art RAW → PNG → JPEG fallback |
  | `3e60285` | #758 | Python traceback diagnostic capture |
  | `815a0ce` | #757 partial | 5 of 9 settings tabs migrated to
  useSettingsField |
  | `5d30f45` | — | Version bump 1.3.1 → 1.3.2 |

  ## Diff scope

  Single new file: `.github/audits/v1.3.2-batch-retrospective.md`. No
  production code touched in this PR — the production changes are already
  on `main` (commits above).

  ## Action requested

  - [ ] Sign-off on the batch as documented.
  - [ ] Confirm the going-forward rule: branch + PR for feature work,
  direct-push reserved for `[skip ci]` doc/chore edits.

  ## Test plan

  - [x] Production code already validated on `main` (cargo check +
  targeted test suites pass — see retrospective doc).
  - [x] This PR adds only a markdown file; CI runs on it as
  belt-and-braces.

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- **(settings)** Migrate 5 small/medium tabs to useSettingsField (#757)

Audit v2 finding #6 — replaces the `settings.X` + `updateSettings({
  X: v })` lambda pair with per-field Zustand bindings via the
  `useSettingsField` hook. Each `useSettingsField('X')` call subscribes
  to `state.settings.X` only, so a change to an unrelated field no
  longer re-renders the entire tab.

  Tabs migrated in this batch:

  - `TemplatesTab.tsx` — 10 template + padding fields
  - `FallbackTab.tsx` — 2 chain bindings
  - `CoverArtTab.tsx` — 7 cover-art + animated-artwork fields
  - `LyricsTab.tsx` — 10 lyrics-related toggles (multi-key
    `handleFormatToggle` retains `updateSettings` for the single-shot
    pair update; non-trivial dependency between two keys)
  - `ToolsTab.tsx` — 1 statically-keyed `temp_path` field (the dynamic
    tool-path map at `TOOL_PATH_KEYS[toolName]` retains `useSettingsStore`
    because `useSettingsField` requires a compile-time key)

  The remaining 4 tabs (AdvancedTab, GeneralTab, QualityTab,
  CookiesTab) account for ~3.3 kLOC and ~180 settings sites. They are
  tracked as a follow-up under #757 — splitting the migration in two
  PRs keeps each reviewable. No behaviour changes — pure refactor.
  TypeScript clean.

  Partial — #757 stays open


### 🧹 Maintenance

- Bump version 1.3.1 → 1.3.2

Bundles four user-visible improvements landed in this batch:


## [1.3.1] - 2026-05-11

### 🐛 Bug Fixes

- **(ui)** Stop stale 'Finalising metadata' label + sync auto-scroll checkbox (#751)

## Summary

  Two related UX bugs reported on 2026-05-11 — both about the
  queue/activity-log surfaces showing state that doesn't match reality.

  ### Bug 1: Progress bar caption staleness

  The enrichment task ended with `set_label("Finalising metadata...", …)`
  immediately followed by the "All enrichment stages completed"
  activity-log line. That label then **persisted as the per-item caption
  through every subsequent gap** — between enrichment ending and the
  companion supervisor spawning its first GAMDL, between companion
  finishing and the post-companion advisory pass starting, etc. Your
  screenshot caught one of those gaps showing "Finalising metadata…" while
  the activity log was reporting fresh GAMDL companion track downloads.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.3.0 [skip ci]
- Update CHANGELOG.md [skip ci]

## [1.3.0] - 2026-05-11

### ✨ Features

- **(history)** Tooltip + right-click actions on long error messages (#748)

## Summary

  The History row's error text was truncated at the right edge of the
  visible area for long messages — notably upstream "GAMDL bug — …"
  classifier outputs. Reading the full text required widening the window.
  New shared `ErrorMessageDisplay` component fixes this with three layered
  affordances:

  - **Hover/focus tooltip** showing the full text
  - **Right-click → Copy error message** (always)
  - **Right-click → Report this bug to GAMDL** (only when the message
  looks like an upstream defect — recognised via the `"GAMDL bug "` prefix
  that `download_queue.rs` emits). Opens
  [`glomatico/gamdl/issues/new`](https://github.com/glomatico/gamdl/issues/new)
  with title + body pre-filled from the failed URL + the error text.

  The pre-filled GitHub URL is **intentionally MeedyaDL-free** — no
  branding, no "via MeedyaDL" attribution, no internal classifier
  metadata. Upstream maintainers get a clean repro shaped like a normal
  GAMDL-user bug report (title strips the classifier prefix so it reads as
  a user-authored summary; body template has URL / Error output /
  Environment sections to fill in).

  ## Wiring

  -
  [`HistoryPage.tsx`](../blob/feat/error-message-tooltip-actions/src/components/download/HistoryPage.tsx)
  — replaces the inline `line-clamp-2` `<p>`. The URL paragraph below it
  also gets a native `title={entry.url}` tooltip (same problem class for
  long Apple Music URLs).
  -
  [`QueueItem.tsx`](../blob/feat/error-message-tooltip-actions/src/components/download/QueueItem.tsx)
  — replaces the inline error `<p>` (no truncation since queue rows are
  full-width, but tooltip + right-click affordances apply).

  ## Tests

  12 unit tests pin: render + null-on-empty + line-clamp variants,
  right-click menu open, copy writes to clipboard, "Report" gated on GAMDL
  prefix, `URL.searchParams.get('body')` contains the error + sourceUrl,
  title strips the classifier prefix, no MeedyaDL anywhere in the report
  URL, omitted sourceUrl drops the URL section.

  ## Implementation note

  Imports siblings via `./ContextMenu` / `./Tooltip` rather than the
  `@/components/common` barrel — the barrel re-exports this very file, so
  a barrel import would resolve to `undefined` at module-evaluation time
  and crash with the React "Element type is invalid" runtime error.

  ## Test plan

  - [x] `npm run type-check` clean
  - [x] `npm run lint` clean
  - [x] `npm run test` — 465 pass (12 new, 0 regressions)
  - [ ] CI green
  - [ ] Manual: in History, hover a long error → tooltip shows full text;
  right-click → "Copy error message" works; right-click on a "GAMDL bug —
  …" entry → "Report this bug to GAMDL" appears + opens upstream issue
  form pre-filled

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.2.0 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(claude)** Refresh project context for v1.0.0/v1.1.0 + audit v2 + release-pipeline gotchas (#747)

## Summary

  The `.claude/` files had drifted after a busy 2026-04 → 2026-05 run.
  This PR brings the shared project memory + the CLAUDE.md context file in
  line with the actual project state as of 2026-05-10.

  ### Memory refreshes
  -
  [\`project_v1_rc_prep.md\`](../blob/docs/refresh-claude-context/.claude/memory/project_v1_rc_prep.md)
  — was stuck at v0.49.1 with two open RC blockers; now reflects v1.0.0 GA
  + v1.1.0 published as Pre-release pending user testing. Captures the
  post-rc.1 promotion path, the audit-v2 rollout, and the recent
  #743/#744/#741/#746 cycle. Adds an explicit pointer to the
  don't-auto-flip-stable-flags policy.

  ### Memory removals
  - `project_pr662_user_session_fixes.md` — described an in-flight PR that
  merged early May. Project memory should describe live state, not
  historical PR descriptions, so the file is removed rather than marked
  "historic".

  ### New memory files
  -
  [\`project_audit_v2_helpers.md\`](../blob/docs/refresh-claude-context/.claude/memory/project_audit_v2_helpers.md)
  — catalogue of the 12 internal primitives that landed across audits v1 +
  v2 (six backend, six frontend), each with its file path + use case.
  -
  [\`project_release_pipeline_gotchas.md\`](../blob/docs/refresh-claude-context/.claude/memory/project_release_pipeline_gotchas.md)
  — the three failure modes the v1.1.0 cut surfaced (\`[skip ci]\`
  propagation through CHANGELOG bodies, "Release in progress…" placeholder
  persistence, manual stable tags sitting as drafts) plus recovery
  patterns and the release-promotion policy.

  ### MEMORY.md index
  Updated to remove the PR662 entry, add the two new entries, and refresh
  the v1 RC prep one-liner.

  ### CLAUDE.md additions
  Three new convention bullets, no restructure of existing content:
  - "Internal helpers (audits v1 + v2)" — one-line index of the 12
  primitives with file paths so the right helper is reachable without
  grepping
  - "Release pipeline gotchas" — short summary of the three failure modes
  + recovery commands, with pointer to the dedicated memory file
  - "Wrapper triangle" — explains the three independent wrapper
  connections (account / m3u8 / decrypt), all now in AppSettings, with the
  schema-version-bump-to-5 reference

  Plus markdown lint fixes for two pre-existing list-spacing issues.

  No code changes.

  ## Test plan

  - [x] No code changes — no test gates apply
  - [x] Markdown linter satisfied (blank lines around lists, valid
  frontmatter)
  - [ ] CI green (only PR title check + CodeQL should run)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- Update CHANGELOG.md [skip ci]
- **(wrapper)** Explain re-authentication when decryption keeps failing (#750)

## Summary

  Follow-up to the wrapper-on-LAN docs
  ([#746](https://github.com/MWBMPartners/MeedyaDL/pull/746)). When users
  see every track in a download skipping with `Decryption is not available
  for media ID: …` **and** wrapper auth is enabled, the most likely cause
  is **stale wrapper credentials** — the wrapper appears healthy (sockets
  accept, pre-flight checks pass) but the Apple Music tokens it cached
  during initial login have expired and decryption requests silently fail
  upstream.

  The user can confirm this from the live state: pre-flights all green ✓,
  manifest fetches succeed, but every per-track decryption WARNING is the
  same `media ID: …` shape.

  ### What this PR adds


  [`help/troubleshooting.md`](../blob/docs/wrapper-reauth/help/troubleshooting.md)
  gets a new **"Decryption is not available" warnings (wrapper enabled,
  downloads still skipping)** subsection covering:

  - Exact symptom shape (per-track WARNING lines, every-track-skips vs
  only-some)
  - Why it happens (cached tokens go stale)
  - Step-by-step Docker re-auth: stop container → run in one-shot login
  mode → enter 2FA if prompted → Ctrl-C → restart. Native install variant
  noted briefly.
  - Diagnostic — distinguishing "stale auth" from
  "track-not-in-this-codec" by skip rate
  - Other possible causes (wrapper not enabled, withdrawn tracks)
  - **Explicit "no, Download Mode doesn't help" note** — addresses the
  common "I switched yt-dlp ↔ N_m3u8DL-RE and it worked" misconception
  (it's the reset, not the mode)

  [`README.md`](../blob/docs/wrapper-reauth/README.md) gets a mirror
  summary in the Wrapper Authentication section (between Auto-Retry and
  Verifying Connectivity) pointing at the in-app help.

  Wrapper login command sourced from upstream
  `WorldObservationLog/wrapper` README — no MeedyaDL guesswork.

  ### Tone

  Matches the prior wrapper docs PR (#746) — less technical than PR/issue
  text, step-by-step, no jargon.

  ### Out of scope

  - Detecting "stale auth" automatically and surfacing a one-click
  suggestion in the activity log — could be a follow-up issue if you want.
  - Wrapper-side changes (out of scope per project policy — wrapper is
  upstream).

  ## Test plan

  - [x] `npm run lint` clean
  - [x] Markdown linter satisfied (asterisk emphasis, blank lines around
  lists)
  - [ ] CI green (only PR title check + CodeQL should run)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- Update CHANGELOG.md [skip ci]

## [1.2.0] - 2026-05-10

### ✨ Features

- **(settings)** Expose wrapper_decrypt_ip — closes #743 (#744)

## Summary

  Implements [#743](https://github.com/MWBMPartners/MeedyaDL/issues/743).
  MeedyaDL exposed two of GAMDL's three wrapper-related connection targets
  (\`wrapper_account_url\` + \`wrapper_m3u8_ip\`) but not the third —
  \`wrapper_decrypt_ip\`. Without this field surfaced in settings,
  remote-wrapper LAN setups silently failed at the decryption stage
  because GAMDL fell back to its compile-time default of
  \`127.0.0.1:10020\` (the user's own loopback, where nothing was
  listening).

  This PR mirrors the existing \`wrapper_m3u8_ip\` shape exactly across
  every layer:

  | Layer | Change |
  |---|---|
  | Rust settings model | New field + default helper + Default impl +
  version bump 4→5 |
  | Settings migration | New v4→v5 step (version-stamp only; serde default
  fills field) + tests |
  | Merge layer | `merge_options()` propagates setting → `GamdlOptions`
  when wrapper on |
  | Preflight | New `check_wrapper_decrypt_health()` +
  `PreflightCheck::WrapperDecrypt` enum variant + wired into chain |
  | TS types | Field + `'wrapper_decrypt'` added to `PreflightCheck` union
  |
  | TS store defaults | `DEFAULT_SETTINGS.wrapper_decrypt_ip =
  '127.0.0.1:10020'` |
  | Settings UI | New `<Input>` next to the m3u8 input |
  | Misc | Added to log-redaction list; test fixtures updated |

  **Behaviour for upgrading users:** zero change. The serde default is
  `127.0.0.1:10020` — the same value GAMDL used at the CLI default.
  Local-wrapper setups are unaffected.

  **Behaviour for remote-wrapper users:** can now configure the field via
  Settings > Advanced. The new preflight check surfaces a yellow toast at
  queue time if the configured host:port is unreachable (instead of
  silently failing at decryption mid-download).

  ## Out of scope

  - Wrapper-side `compose.yaml` `ports:` section (upstream
  `WorldObservationLog/wrapper`, not ours to fix per the discussion on
  #743).
  - General "remote wrapper setup" help doc — separate issue if wanted.

  ## Test plan

  - [x] `cargo clippy --tests -- -D warnings` clean
  - [x] `cargo test --lib` — 1009 pass, 1 ignored (1 new for v4→v5
  migration)
  - [x] `npm run type-check` clean
  - [x] `npm run lint` clean
  - [x] `npm run test` — 453 pass (3 fixture snapshots updated, 0
  regressions)
  - [ ] CI green
  - [ ] Manual smoke (you have the actual Mac → RPi setup):
  - Settings > Advanced should show a new "Wrapper Decryption Address"
  input
  - Setting it to the RPi's IP, with the wrapper service exposing port
  10020, should make ALAC downloads succeed
  - Setting it to a non-listening address should produce a yellow
  preflight toast at queue time


### 📚 Documentation

- **(security)** Update supported versions to 1.1.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Explain remote-wrapper setup (the three-address pattern) (#746)

## Summary

  Follow-up to [#743](https://github.com/MWBMPartners/MeedyaDL/issues/743)
  / [#744](https://github.com/MWBMPartners/MeedyaDL/pull/744). The new
  \`wrapper_decrypt_ip\` setting needs documentation explaining when and
  how to use it. Both
  [README.md](../blob/docs/wrapper-on-lan-device/README.md) and the in-app
  help ([Help > Troubleshooting > Wrapper
  Errors](../blob/docs/wrapper-on-lan-device/help/troubleshooting.md)) now
  include a "Running the wrapper on a different device on your network"
  section.

  The new content explains:

  - The wrapper uses **three** connections, not one
  - A table mapping each setting → what it does → default port
  - A worked example for a Raspberry Pi at `192.168.1.50` showing all
  three addresses to update
  - Common gotchas (container port-forwarding, firewall, forgetting the
  third address, three-port conflict)
  - A quick SSH-tunnel alternative for users who'd rather not touch
  MeedyaDL's defaults

  Tone is intentionally less technical than the issue/PR — no
  TCP/loopback/outbound jargon. Aimed at a hobbyist who set up a Raspberry
  Pi.

  The README's existing "Troubleshooting (Remote / Docker)" section is
  preserved unchanged below the new content.

  ## Test plan

  - [x] No code changes
  - [x] \`npm run lint\` clean
  - [x] Markdown linter satisfied (table padding + valid link fragment)
  - [ ] CI green (only PR title check + CodeQL should run)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


## [1.1.1] - 2026-05-10

### 🐛 Bug Fixes

- **(release)** Pipeline cleanup — stop placeholder, halt cadence drift (#741)

## Summary

  Three CI/release fixes addressing user-reported issues:

  ### 1. Stop "Release in progress..." persistence (\`release.yml\`)

  Per-platform build steps create the GitHub Release with \`gh release
  create --notes "Release in progress..."\` as a race-guard. The "Append
  download guide" step then appended to that body — leaving the
  placeholder as the leading line forever in every release that wasn't
  pre-populated by release-please-action.


### 📚 Documentation

- **(security)** Update supported versions to 1.1.0 [skip ci]
- Update CHANGELOG.md [skip ci]

## [1.1.0] - 2026-05-09

### ✨ Features

- **(release)** V1.0.1 prep — GAMDL 3.5.1, activity-log refactor, Library Scan scaffold

16 commits delivering:

  - GAMDL v3.5.1 admission to support window (#711)
  - Activity-log media-context labels + 30-min walk hang fix + stale progress-bar caption fix (#712)
  - Phase 3.5 holistic refactor of activity-log + progress-bar emission layer (#714, 9 sub-commits): ProgressStage enum, unified emit_inner facade, shared set_stage helpers callable from both enrichment AND companion tasks, emit_subprocess_line consolidation, sub-stage labels for lyrics + finalising + companion phases, frontend caption extraction, codec-skip-line humaniser, companion file-count verification before claiming complete
  - GAMDL MV cover-URL bug status documented (not fixed in 3.5.1, workaround tracked in #715)
  - Codebase unification audit doc with 8 prioritised findings (#716, implementation PRs follow)
  - Library Scan page scaffold (#717, gap-fill UX sub-features deferred)
  - Version bump 0.53.3 → 1.0.1
  - All gates green: cargo check / clippy / fmt / test (975 pass), npm lint / type-check / test (305 pass), npm audit (0 vulns)

  Full PR description: https://github.com/MWBMPartners/MeedyaDL/pull/718

- **(release)** V1.0.2 prep — MV cover workaround + 3 unification helpers + fast-uri patch

6 commits delivering:

- **(release)** V1.0.3 prep — helper migrations + Library Scan diff badges

5 commits delivering:

- **(release)** V1.0.4 prep — per-item MV override + more helper migrations

4 commits delivering:

- **(release)** V1.0.5 prep — Library Scan gap-fill modal + Re-download action

3 commits delivering:

- **(release)** V1.0.6 prep — Library Scan freshness + helper migration (#724)

## Summary

  Sixth in the v1.0.x prep series. Three landings:

  - **#717/5c**: Apple Music \`lastModifiedDate\` freshness check per
  Library Scan row. New \`check_library_scan_freshness\` IPC +
  \`LibraryScanFreshness\` tagged union. Frontend dispatches throttled to
  5 concurrent calls; \`Sparkles\` "Content updated" badge renders
  alongside the existing diff badge. Re-download button enables for both
  \`plan\` (missing tracks) AND \`updated\` (content changed upstream —
  added tracks, Atmos mix, ADM certification).
  - **#717/5g**: 10 unit tests for \`MvGapFillModal\` covering all four
  override outcomes in the 2x2 table (ENABLED+Yes→null, ENABLED+No→false,
  DISABLED+Yes→true, DISABLED+No→null) plus null-manifest gating, prompt
  copy, and Cancel vs Confirm.
  - **#716 finding #1** (one more migration):
  \`scan_dir_for_manifests_recursive\` → \`walk_dir_depth(base, 10,
  parse_manifest_at_path)\`. ~90 lines of recursive boilerplate gone;
  behaviour preserved (empty-source manifests still skipped via \`None\`
  returns).

  Versions bumped 1.0.5 → 1.0.6 across package.json, Cargo.toml,
  tauri.conf.json, .release-please-manifest.json.

  ## Test plan

  - [x] \`cargo clippy --tests -- -D warnings\` clean
  - [x] \`cargo test --lib\` — 993 pass, 1 ignored
  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean (sole warning is pre-existing in
  updateStore.ts)
  - [x] \`npm run test\` — 315 pass (10 new from MvGapFillModal.test.tsx)
  - [ ] CI green
  - [ ] Manual: scan a folder, confirm freshness badge appears for updated
  albums (or stays absent for users without MusicKit creds)
  - [ ] Manual: confirm the new helper-migrated scanner still finds
  manifests at depth 0..10

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(release)** V1.0.7 prep — Zustand async-resource factory primitive (#725)

## Summary

  Seventh in the v1.0.x prep series. One landing:

  - **#716 finding #5**: \`createAsyncResourceStore<T extends
  object>(config)\` factory in
  [\`src/lib/createAsyncResourceStore.ts\`](../blob/feat/v1.0.7-prep/src/lib/createAsyncResourceStore.ts).
  Returns a Zustand hook with the standard
  \`data\`/\`isLoading\`/\`isDirty\`/\`error\` reactive state and
  \`load\`/\`save\`/\`debouncedSave\`/\`update\`/\`reset\` actions.
  Read-only stores (no \`save\` config) get silent no-ops on save paths.
  Default debounce window 300ms.
  - **15 unit tests** covering initial defaults, load
  happy/error/non-Error rejections, save happy/error/re-throw, debounce
  batching + default-300ms + read-only-noop + error surfacing, update
  shallow-merge + reference equality, reset.
  - Audit doc finding #5 marked **primitive landed; opt-in migration
  deferred**.

  **No existing store migrated in this PR.** Each existing store has 30+
  component consumers using per-store API names (\`settings\`,
  \`loadSettings\`, \`saveSettings\`, etc.); migrating those is a separate
  cycle whose value is debatable for already-working code. The primary
  consumer is the M8/M9/M10 per-service settings stores when those land.

  Versions bumped 1.0.6 → 1.0.7.

  ## Test plan

  - [x] \`cargo check\` clean (Cargo.lock refresh only)
  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean (sole warning is pre-existing in
  updateStore.ts)
  - [x] \`npm run test\` — 330 pass (15 new from
  createAsyncResourceStore.test.ts)
  - [ ] CI green
  - [ ] (Manual not applicable — additive primitive, no UI surface)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(release)** V1.0.8 prep — four more recursive walker migrations (#726)

## Summary

  Eighth in the v1.0.x prep series. Four more recursive walker callsites
  migrated to \`walk_dir_depth\` (#716 finding #1).

  | File | Function | Before | After | Notes |
  |---|---|---|---|---|
  | services/duplicate_detector.rs | walk_manifests | depth=10, manual |
  depth=10, walk_dir_depth | side-effects into HashSet (0..N keys per
  manifest) |
  | services/acoustid_service.rs | collect_m4a_recursive | **UNBOUNDED** |
  depth=3 | AcoustID fingerprinting is per-album |
  | services/replaygain_service.rs | collect_audio_recursive |
  **UNBOUNDED** | depth=3 | FFmpeg loudness analysis is per-album |
  | services/metadata_tag_service.rs | tag_directory_recursive |
  **UNBOUNDED** | depth=3 | covers Album/Disc N/file split |

  The three previously-unbounded walkers were latent #712 risks — if ever
  called against the user's full music root rather than an album dir,
  they'd produce the same 30-minute hang reproduction. Capping at depth 3
  makes that impossible without affecting happy-path behaviour (every
  actual album dir is well under 3 deep).

  Filesystem-sidecar skipping (\`._*\`, \`.DS_Store\`, \`Thumbs.db\`)
  preserved in all four — same #577 rationale (avoid \`mp4ameta\` /
  \`ffmpeg\` / \`chromaprint\` errors on non-audio binaries).

  Net -36 lines.

  Versions bumped 1.0.7 → 1.0.8.

  ## Test plan

  - [x] \`cargo clippy --tests -- -D warnings\` clean
  - [x] \`cargo test --lib\` — 993 pass, 1 ignored
  - [x] \`npm run type-check\` clean
  - [x] \`npm run test\` — 330 pass
  - [ ] CI green
  - [ ] (Manual smoke: depth=3 covers Artist/Album/Disc/file — check no
  regression on multi-disc albums)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(release)** V1.0.9 prep — walk_dir_find_first + last two walker migrations (#727)

## Summary

  Ninth in the v1.0.x prep series. Closes out the recursive-walker
  consolidation (#716 finding #1).

  **New helper:** \`walk_dir_find_first<T, F>(base, max_depth, visitor) ->
  Option<T>\` in
  [\`utils/fs_walk.rs\`](../blob/feat/v1.0.9-prep/src-tauri/src/utils/fs_walk.rs).
  Find-first companion to \`walk_dir_depth\` — short-circuits as soon as
  the visitor matches. 6 unit tests covering depth-zero match,
  descend-into-subdirs, no-match, max_depth, short-circuit (call counter),
  missing dir.

  **Migrations** (the last two hand-rolled recursive walkers):

  | Function | Was | Now |
  |---|---|---|
  | find_binary_recursive (dependency_manager.rs) | UNBOUNDED | depth=5,
  walk_dir_find_first |
  | find_file_recursive (dependency_manager.rs) | UNBOUNDED | depth=5,
  walk_dir_find_first |

  Both signatures cleaned: \`&PathBuf\` → \`&Path\` (clippy::ptr_arg); one
  \`&tool_dir.to_path_buf()\` call simplified.


### 🐛 Bug Fixes

- **(ci)** Stop release-channel workflow self-trigger loop ([skip ci])

The push-driven release workflows for the alpha / beta / release-candidate
  channels each commit a version bump and push it back to the same branch
  that triggered them. The job-level
  `if: github.actor != 'github-actions[bot]'` guard was meant to prevent the
  bot push from re-triggering the workflow, but it does not work in this
  repo because the bot uses `RELEASE_PAT` rather than the default
  `GITHUB_TOKEN`, and GitHub attributes PAT-authenticated pushes to the PAT
  owner (a real user account) not to `github-actions[bot]`.

  Net effect on 2026-05-08: a single human push to `release-candidate`
  spawned 21 sequential RC tags (v1.0.0-rc.1 through v1.0.0-rc.21) and
  6 published + 5 draft GitHub Releases before the loop was caught and
  killed by hand. Spurious tags and releases have been deleted; only the
  intended v1.0.0-rc.1 remains.

  The fix appends `[skip ci]` to the bot's commit message in all three
  push-driven release-channel workflows. GitHub parses this marker at the
  trigger layer, so the workflow is not even queued for the bot's own
  push. The pre-existing actor guard is kept as belt-and-braces.

  Files touched (commit-message string only, no behavioural change):
    - .github/workflows/alpha-release.yml
    - .github/workflows/beta-release.yml
    - .github/workflows/release-candidate-release.yml

  The cron-driven workflows (nightly, weekly, monthly) do NOT have `push:`
  triggers, so they were never affected — verified.

- **(ci)** Stop release-channel workflow self-trigger loop ([skip ci]) (#713)

## Incident

  Single human push to \`release-candidate\` on 2026-05-08 09:38 UTC
  spawned **21 sequential RC tags** (\`v1.0.0-rc.1\` through
  \`v1.0.0-rc.21\`) and 6 published + 5 draft GitHub Releases before the
  loop was caught and killed by hand. Spurious tags + releases have been
  deleted; only the intended \`v1.0.0-rc.1\` remains.

  ## Root cause

  The push-driven release workflows for \`alpha\` / \`beta\` /
  \`release-candidate\` each commit a version bump and push it back to the
  same branch that triggered them. The job-level guard

  \`\`\`yaml
  if: github.actor != 'github-actions[bot]' || github.event_name ==
  'workflow_dispatch'
  \`\`\`

  was meant to skip the bot's own push, **but does not work in this repo**
  because:

  - The bot uses \`secrets.RELEASE_PAT\` (not the default
  \`GITHUB_TOKEN\`) for the push.
  - GitHub attributes PAT-authenticated pushes to the **PAT owner's user
  account**, not to \`github-actions[bot]\`.
  - So \`github.actor\` is the human user; the check is always true; the
  workflow re-runs every push.

  Each iteration: \`BASE = \"1.0.0\"\`, \`MAX\` increments because the
  previous run created \`v1.0.0-rc.N\`, \`NEXT = MAX + 1\`, version
  manifests patched + committed + tagged, push to \`release-candidate\`
  triggers the workflow again. Loop only stops when the PAT runs out of
  API quota or someone cancels in-progress runs.

  ## Fix

  Append \`[skip ci]\` to the bot's commit message in all three
  push-driven release-channel workflows:

  - \`.github/workflows/alpha-release.yml\`
  - \`.github/workflows/beta-release.yml\`
  - \`.github/workflows/release-candidate-release.yml\`

  GitHub parses \`[skip ci]\` at the **trigger layer** — the workflow is
  not even queued for the bot's own push. The pre-existing actor guard is
  kept as belt-and-braces.

  ## Why this fix is safe

  - Only the bot's own commit message changes; no behavioural difference
  for any human contributor.
  - \`[skip ci]\` is parsed by GitHub natively, no custom logic required.
  - Cron-driven workflows (\`nightly\` / \`weekly\` / \`monthly\`) have
  \`on: schedule\` but no \`on: push\` — they were never affected.
  Verified via \`grep -A 5 \"^on:\"
  .github/workflows/{nightly,weekly,monthly}-release.yml\`.

  ## Verification

  - [x] Three workflow files updated — diff is one-line per file, commit
  message string only
  - [x] \`grep \"\\[skip ci\\]\"
  .github/workflows/{alpha,beta,release-candidate}-release.yml\` confirms
  presence in all three
  - [ ] After merge: smoke-test by triggering one push to \`alpha\`
  (separate, low-risk channel) and confirming exactly **one** \`alpha\`
  tag is produced
  - [ ] After successful smoke-test, can resume \`v1.0.1-prep\` work
  without fear of re-runaway

  ## Cleanup already done out-of-band

  - \`v1.0.0-rc.2\` through \`v1.0.0-rc.21\` tags deleted (\`gh release
  delete --cleanup-tag\` for rc.2-rc.11, \`git push origin --delete\` for
  rc.12-rc.21 which only had tags).
  - All in-progress / queued \`release.yml\` runs for the spurious tags
  cancelled (saves GHA minutes).
  - \`release-candidate\` branch HEAD is currently at the bot's
  \`chore(rc): 1.0.0-rc.21\` commit. Not reset (force-push protected) —
  the next legitimate RC bump will compute \`MAX=1\` from remaining tags
  and produce \`v1.0.0-rc.2\` correctly.

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 0.53.3 [skip ci]
- **(security)** Update supported versions to 1.0.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.3 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.4 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.5 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.6 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.7 [skip ci]
- **(security)** Update supported versions to 1.0.8 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.9 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 1.0.10 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(audit)** Codebase unification audit v2 — 8 new consolidation findings (#732)

## Summary

  Second consolidation pass after [audit
  v1](.github/audits/codebase-unification-audit-v1.md) closed out (#716,
  completed in v1.0.9).

  Eight new findings, each scored on LOC impact, risk, and multi-service
  relevance:

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(commands)** Authoring guide — audit v2 #8 (#738)

## Summary

  Sixth implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #8
  (state injection docs).

  Tauri's \`#[tauri::command]\` DI macro requires explicit parameter
  signatures, so \`State<'_, T>\` boilerplate is unavoidable per-command.
  The audit's recommendation was **documentation, not refactor** — codify
  the pattern so M8/M9/M10 command modules stay aligned with what already
  exists.

  **New**
  [\`src-tauri/src/commands/README.md\`](../blob/docs/audit-v2-commands-pattern/src-tauri/src/commands/README.md)
  covers:

  - Anatomy of a command (signature, doc comment, return type)
  - Parameter conventions (AppHandle first when present, \`State<'_, T>\`
  next, request payload last)
  - Adding new managed state types (tie-back to \`lib.rs::run()\`
  \`.manage()\`)
  - Async vs sync (\`spawn_blocking\` for CPU-bound work)
  - Error handling (\`Result<T, String>\` + context-bearing \`map_err\`
  prefix, no internal type names)
  - Registration in \`lib.rs::generate_handler!\` + frontend wrapper in
  \`src/lib/tauri-commands.ts\`
  - Naming conventions
  - Per-service module layout for M8/M9/M10
  - References to clean small + full-featured examples

  Pure docs — no code changes.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 withErrorToast (#734)
  3. ✅ #3 useConfirmation (#735)
  4. ✅ #6 useSettingsField (#736)
  5. ✅ #5 subprocess reader (#737)
  6. ✅ #8 commands README (this PR)
  7. #2 useAsyncTask hook (next)
  8. #7 context_err! macro

  ## Test plan

  - [x] No code changes — no test gates apply
  - [ ] CI green (only the markdown linter / PR title check should run)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- **(testing)** Centralised fixture builders — audit v2 #4 (#733)

## Summary

  First implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #4
  (test fixture builders).

  **New module**
  [\`src/testing/fixtures.ts\`](../blob/refactor/audit-v2-test-fixtures/src/testing/fixtures.ts):
  - \`makeFixture<T>(defaults)\` generic factory
  - \`makeQueueItem\` — \`QueueItemStatus\` builder
  - \`makeActivityEntry\` — \`ActivityLogEntry\` builder
  - \`makeScannedManifest\` — \`ScannedManifest\` builder

  **9 tests** pin the helper contract + per-builder default shape so a
  future change to a domain type that breaks the defaults is caught here
  before cascading.

  **Three call sites migrated:**
  - \`DownloadQueue.test.tsx\` — inline \`makeItem\` builder removed (~25
  LOC)
  - \`ActivityLog.test.tsx\` — inline \`makeEntry\` builder removed (~10
  LOC)
  - \`MvGapFillModal.test.tsx\` — 13-line \`baseManifest\` literal →
  \`makeScannedManifest()\`

  All 22 + 18 + 10 tests in the migrated files still pass without
  modification.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (this PR)
  2. #1 useAsyncWithToast helper (next)
  3. #3 confirmation modal factory hook
  4. #6 settings field hook
  5. #5 subprocess reader abstraction
  6. #8 state injection docs
  7. #2 useAsyncTask hook
  8. #7 context_err! macro

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 409 pass (9 new, 0 regressions in 3 migrated
  files)
  - [ ] CI green

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(lib)** WithErrorToast helper — audit v2 #1 (#734)

## Summary

  Second implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #1
  (async error → toast emission shape).

  **New helper**
  [\`src/lib/withErrorToast.ts\`](../blob/refactor/audit-v2-with-error-toast/src/lib/withErrorToast.ts):

  \`\`\`ts
  await withErrorToast(() => ipc(), {
    successMsg: 'Success!',
    errorMsg: (err) => \`Failed to X: \${err}\`,
  suppressOn: ['cancel'], // optional — silently swallow expected
  rejections
  });
  \`\`\`

  Reads \`addToast\` via \`useUiStore.getState()\` so it works from any
  context (component handlers, store actions, async effects) without being
  a hook itself.

  **13 unit tests** cover: success/error paths, static-string vs
  function-typed errorMsg, default vs 'info' successVariant, suppressOn
  case-insensitive matching against the displayed (post-errorMsg) text.

  **Three call sites migrated** as proof:
  - \`SettingsPage.tsx\` \`handleSave\` (7 LOC → 4)
  - \`CrashReportSection.tsx\` \`handleDelete\` + \`handleDeleteAll\` (16
  → 11)
  - \`DownloadQueue.tsx\` 5 handlers (50 → 35)

  All migrated sites' tests pass without modification.

  Remaining ~25 call sites can opt in incrementally; no big-bang refactor.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 useAsyncWithToast / withErrorToast (this PR)
  3. #3 confirmation modal factory hook (next)
  4. #6 settings field hook
  5. #5 subprocess reader abstraction
  6. #8 state injection docs
  7. #2 useAsyncTask hook
  8. #7 context_err! macro

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 422 pass (13 new, 0 regressions in 3 migrated
  files)
  - [ ] CI green

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(lib)** UseConfirmation hook — audit v2 #3 (#735)

## Summary

  Third implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #3
  (confirmation modal factory).

  **New hook**
  [\`src/lib/useConfirmation.tsx\`](../blob/refactor/audit-v2-use-confirmation/src/lib/useConfirmation.tsx):

  \`\`\`ts
  const confirmDelete = useConfirmation({
    title: 'Delete crash report',
    description: 'This cannot be undone.',
    confirmLabel: 'Delete',
    onConfirm: () => deleteCrashReport(id),
  });

  return (
    <>
      <Button onClick={confirmDelete.open}>Delete</Button>
      {confirmDelete.modal}
    </>
  );
  \`\`\`

  The hook owns open/close state. \`description\` accepts \`ReactNode\` so
  callers can include item details, secondary checkboxes (bound to parent
  state), or multi-paragraph copy. Auto-closes on successful
  \`onConfirm\`; **stays open if \`onConfirm\` throws** so the user can
  retry — \`onConfirm\` is expected to surface its own error toast
  (natural pairing with \`withErrorToast\`).

  **11 unit tests** pin: default visibility, open(), confirm + auto-close,
  stay-open-on-reject, cancel/escape fire onCancel, programmatic close(),
  custom labels, rich ReactNode description with parent-state checkbox.

  **Two DownloadQueue modals migrated** as proof:
  - "Retry All Failed" (#665) — ~25 LOC of inline modal JSX gone
  - "Clear All" — ~28 LOC gone

  Two modals deliberately deferred:
  - Per-item Delete (#685) — needs \`deleteTarget\` snapshot in
  description
  - Abort Queue (#620) — has "don't ask again" checkbox bound to separate
  state

  Both could migrate (hook supports both via rich descriptions) but are
  slightly off the happy path.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 withErrorToast (#734)
  3. ✅ #3 useConfirmation (this PR)
  4. #6 settings field hook (next, before M8)
  5. #5 subprocess reader abstraction
  6. #8 state injection docs
  7. #2 useAsyncTask hook
  8. #7 context_err! macro

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 433 pass (11 new, 0 regressions in 2 migrated
  modals)
  - [ ] CI green

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(hooks)** UseSettingsField — audit v2 #6 (#736)

## Summary

  Fourth implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #6
  (settings tab boilerplate).

  **New hook**
  [\`src/hooks/useSettingsField.ts\`](../blob/refactor/audit-v2-settings-field-hook/src/hooks/useSettingsField.ts):

  \`\`\`ts
  const acoustid = useSettingsField('acoustid_enabled');
  <Toggle checked={acoustid.value} onChange={acoustid.set} />
  \`\`\`

  The key is supplied once. TypeScript narrows \`value\` and \`set\` to
  the field's actual type (\`boolean\` / \`string\` / \`number\` / union)
  automatically. Each call subscribes to \`state.settings.X\` only —
  re-renders only on that field's change, preserving audit-v1's
  per-selector pattern. \`set\` is \`useCallback\`-stable.

  **11 unit tests** pin: read for each field type, reactive updates from
  external store mutations, set writes via updateSettings without touching
  other fields, set works for union-typed fields, set is referentially
  stable per key.

  **MetadataTab migrated** as proof — 7 controls converted from the inline
  \`(checked) => updateSettings({ X: checked })\` lambda pattern. Net ~25
  LOC saved on this single tab.

  **Multi-service relevance: very high.** M8/M9/M10 per-service settings
  tabs (BBC iPlayer session, Spotify credentials, YouTube API key) will
  land ~20 controls each. Without this hook: ~60 LOC of identical lambdas
  per service. With it: ~10 LOC. Direct enabler for M8.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 withErrorToast (#734)
  3. ✅ #3 useConfirmation (#735)
  4. ✅ #6 useSettingsField (this PR)
  5. #5 subprocess reader abstraction (next, before M9/M10)
  6. #8 state injection docs
  7. #2 useAsyncTask hook
  8. #7 context_err! macro

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 444 pass (11 new, 0 regressions)
  - [ ] CI green
  - [ ] (Manual: open Settings > Metadata, toggle every control, save,
  reload — confirm values round-trip)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(utils)** Spawn_line_reader helper — audit v2 #5 (#737)

## Summary

  Fifth implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #5
  (subprocess reader abstraction).

  **Honest scope assessment:** the audit estimated ~120 LOC saved across 4
  reader sites. After surveying the actual code, that estimate was
  optimistic — companion_supervisor and download_queue carry per-stream
  state (watchdog timestamps, mutex accumulators, last-clean return-value
  threading) that doesn't generalise into a clean visitor signature
  without making the abstraction bigger than the inline code it replaces.

  **What this PR ships:**
  - New helper
  [\`utils/subprocess_reader.rs\`](../blob/refactor/audit-v2-subprocess-reader/src-tauri/src/utils/subprocess_reader.rs)
  with one primitive: \`spawn_line_reader(stream, async_visitor) ->
  JoinHandle<()>\`
  - 4 unit tests pinning the contract: reads-until-close,
  partial-final-line, empty-stream, visitor mutable-state sharing (proves
  the helper *could* host the more complex sites if a future audit decides
  it's worth it)
  - engine_runner's two readers migrated — they were byte-identical except
  for the stream label. Net ~25 LOC saved on this single site
  - companion_supervisor and download_queue **stay inline**; the
  file-level docstring documents why honestly

  **Real long-term value:** the primitive is a clean foundation for M9
  (Votify) and M10 (yt-dlp) pip-engines, which will follow the
  engine_runner pattern. They can adopt this from day one rather than
  re-implementing the BufReader+next_line scaffold.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 withErrorToast (#734)
  3. ✅ #3 useConfirmation (#735)
  4. ✅ #6 useSettingsField (#736)
  5. ✅ #5 subprocess reader (this PR)
  6. #8 state injection docs (next)
  7. #2 useAsyncTask hook
  8. #7 context_err! macro

  ## Test plan

  - [x] \`cargo clippy --tests -- -D warnings\` clean
  - [x] \`cargo test --lib\` — 1003 pass, 1 ignored (4 new)
  - [x] \`npm run type-check\` clean
  - [x] \`npm run test\` — 444 pass (unchanged)
  - [ ] CI green
  - [ ] (Manual smoke: start a download, confirm engine output still
  streams to UI / activity log via both event channels)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(hooks)** UseAsyncTask — audit v2 #2 (#739)

## Summary

  Seventh implementation cycle of [audit
  v2](.github/audits/codebase-unification-audit-v2.md). Closes finding #2
  (component-local async lifecycle).

  **New hook**
  [\`src/hooks/useAsyncTask.ts\`](../blob/refactor/audit-v2-use-async-task/src/hooks/useAsyncTask.ts)
  — the component-local sibling of \`createAsyncResourceStore\` (which
  covers the store-level shape).

  \`\`\`ts
  const submit = useAsyncTask(runPreflight);
  <Button loading={submit.isRunning} onClick={() =>
  submit.run()}>Submit</Button>
  {submit.error && <p className="text-status-error">{submit.error}</p>}
  \`\`\`

  - Args forwarded through \`run(...args)\` — wrapped fn can take
  arbitrary parameters
  - Wrapped fn captured fresh on every render via a ref → no stale-closure
  bugs
  - \`run\` is referentially stable → safe to pass to onClick without
  \`useCallback\`
  - Composes naturally with \`withErrorToast\` when callers want a toast
  on top

  **9 unit tests** pin the contract: initial state, value/error/undefined
  returns, isRunning toggle timing, prior-error clearing on new run, args
  forwarding, run reference stability, **always invokes the latest
  closure** (the captured-by-ref pattern — captured run from render 1
  still calls render 2's fn).

  **DownloadForm migrated** as proof — replaced the hand-rolled
  \`isChecking\` boolean + handleSubmit's try/finally toggle with
  \`submitTask.isRunning\` + \`submitTask.run()\`. The lambda wrapper \`()
  => runPreflightAndSubmit()\` is required because the wrapped fn is
  declared after the hook call (TS2448 TDZ otherwise) — captures the
  binding lazily so click-time the binding is initialised.

  **Audit v2 PR plan:**
  1. ✅ #4 fixtures (#733)
  2. ✅ #1 withErrorToast (#734)
  3. ✅ #3 useConfirmation (#735)
  4. ✅ #6 useSettingsField (#736)
  5. ✅ #5 subprocess reader (#737)
  6. ✅ #8 commands README (#738)
  7. ✅ #2 useAsyncTask (this PR)
  8. #7 context_err! macro (last one)

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 453 pass (9 new, 0 regressions in
  DownloadForm's 15 tests)
  - [ ] CI green
  - [ ] (Manual smoke: paste an Apple Music URL, confirm Add to Queue
  button shows loading state during preflight + clears on completion)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(utils)** Context_err! macro — audit v2 #7 (closes audit v2) (#740)

## Summary

  **Final audit v2 cycle.** Closes finding #7 (Tauri command
  error-wrapping macro). With this PR merged, **all 8 audit v2 findings
  ship**.

  **New macro**
  [\`src-tauri/src/utils/error_context.rs\`](../blob/refactor/audit-v2-context-err-macro/src-tauri/src/utils/error_context.rs):

  \`\`\`rust
  use crate::context_err;

  let entry = context_err!(
      keyring::Entry::new(SERVICE_NAME, &key),
      "Failed to create keyring entry"
  )?;
  \`\`\`

  Expands to \`result.map_err(|e| format!("...: {e}"))?\`. Format args
  resolve against the surrounding scope. Exported via \`#[macro_export]\`
  so the call site is \`crate::context_err!\`.

  **Honest scope assessment.** Per-site savings are modest (~30 chars).
  The real value: one place to evolve error formatting (separator change,
  structured logging, migration to a \`CommandError\` enum) — without the
  macro, that's a 40-site search-and-replace.

  **5 unit tests** pin: Ok pass-through, static prefix, format args from
  scope, function-body usage with \`?\`, Display preservation.

  **Three credentials.rs sites migrated** as proof. \`commands/README.md\`
  updated to point new code at the macro.

  ## Audit v2 — final scoreboard


### 🧪 Testing

- **(downloadform)** Focused unit tests — first installment of #232 (#728)

## Summary

  First installment of #232 (frontend tests for DownloadForm,
  DownloadQueue, ActivityLog, SetupWizard) — DownloadForm covered.

  15 tests landed in
  [\`src/components/download/DownloadForm.test.tsx\`](../blob/feat/v1.0.10-prep/src/components/download/DownloadForm.test.tsx):

  - Smoke render + label/input wiring + helper hint
  - Single URL: typing → store update + validation, content-type badge,
  error message
  - Multi-URL: count badge with pluralisation, "(N invalid)" suffix,
  all-invalid error
  - Submit button gating across 4 states (empty / invalid single / valid
  single / mixed batch)
  - Quality Overrides toggle render + click

  **Deferred to a follow-up PR**: the full \`handleSubmit\` preflight
  chain (4 IPCs — internet / output path / wrapper / cookies). Each
  preflight has its own toast/error/redirect surface and warrants
  dedicated fixture setup. Submit-disabled gating is tested without
  clicking the button.

- **(downloadqueue)** Focused unit tests — #232 part 2 (#729)

## Summary

  Second installment of #232 (frontend tests for DownloadForm,
  DownloadQueue, ActivityLog, SetupWizard) — DownloadQueue covered.

  22 tests landed in
  [\`src/components/download/DownloadQueue.test.tsx\`](../blob/test/232-downloadqueue/src/components/download/DownloadQueue.test.tsx):

  - Empty state (icon + helper text + 0-items subtitle)
  - Header subtitle pluralisation (0 / 1 / 2 items)
  - Per-row rendering via QueueItem (mocked to a placeholder)
  - \`role=list\` accessibility landmark
  - Stats bar segments — non-zero only
  - Conditional action buttons:
    - Refresh / Import always
    - Start Queue: queued > 0 AND active === 0
    - Clear Completed: only when finished items exist
    - Retry All Failed: only when failed > 0
    - Abort Queue: hides when only terminal items remain
  - Export: present when queue has items, count of active+queued in label
  - Confirmation modals open on click (Retry All, Clear All, Abort)
  - \`refreshQueue\` called on mount (polling start)
  - DownloadState exhaustiveness sanity check

  QueueItem mocked as a stable placeholder so this file exercises the
  queue *controller* (header, stats, actions, modals, empty state) — not
  the row component, which warrants its own test file.

  **No version bump in this PR** — per-cycle internal version bumps were
  confusing the picture. Release-please PR #719 accumulates everything
  into the next bumper release.

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 367 pass (22 new)
  - [ ] CI green
  - [ ] (Manual not applicable — pure test addition, no behaviour change)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)

- **(activitylog)** Focused unit tests — #232 part 3 (#730)
- **(setupwizard)** Focused unit tests — #232 part 4 (closes controller scope) (#731)

## Summary

  Fourth and final installment of #232 (frontend tests for DownloadForm,
  DownloadQueue, ActivityLog, SetupWizard) — SetupWizard controller
  covered.

  15 tests landed in
  [\`src/components/setup/SetupWizard.test.tsx\`](../blob/test/232-setupwizard/src/components/setup/SetupWizard.test.tsx):

  - Progress bar renders all 6 step labels
  - Future-step circles show 1-based index
  - Past steps render as ✓ checkmarks
  - StepComponent dispatches to the correct mock per \`currentStep\`
  - Exhaustive sanity check: every SETUP_STEPS entry maps to its expected
  mock
  - Back button hidden on first step, present from step 2
  - Back click invokes \`prevStep\`
  - Continue disabled when current step ∉ \`completedSteps\`, enabled
  otherwise
  - Continue click invokes \`nextStep\`
  - Last step: Continue → "Get Started"
  - Get Started fires the full finish flow (\`finishSetup\` +
  \`updateSettings\` + \`setShowSetupWizard(false)\`)
  - SETUP_STEPS contract test pins the documented order

  **Each step component mocked** as a tiny placeholder so tests assert on
  the controller's dispatch + navigation gating without exercising 1700+
  lines of step behaviour. Per-step tests (CookiesStep, DependenciesStep,
  etc.) remain as follow-ups in their own dedicated test files.

  \`goToStep(idx, completedSteps)\` helper mutates the setupStore directly
  so tests can jump to any wizard position without going through the
  per-step UX.

  **This is the final installment of #232's controller-level scope** — all
  four target components now have focused unit test files. Per-row /
  per-step deep-dive coverage is its own follow-up.

  Frontend test count: **400 total** (15 new in this PR).

  ## Test plan

  - [x] \`npm run type-check\` clean
  - [x] \`npm run lint\` clean
  - [x] \`npm run test\` — 400 pass (15 new)
  - [ ] CI green

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


## [0.53.3] - 2026-05-08

### 🐛 Bug Fixes

- **(queue)** Identify content in codec-exhaustion activity log messages

When GAMDL exhausts the priority chain, the activity log used to say
  "none available for this content" / "All audio formats exhausted" with
  no indication of *which* queued item failed — confusing once the queue
  is more than one item deep. Each of the three exhaustion sites in
  process_queue() now appends "for {Artist — Album — Track}" (or the
  redacted URL when the API metadata fetch hadn't populated names yet),
  sourced from the queue item under its existing lock.

- **(queue)** Scale companion-phase timeout by tier count (#705)

The completion task was reusing compute_completion_timeout (sized for
  enrichment alone) as the deadline for the companion-wait branch. With
  multi-tier companion modes (e.g. Atmos→ALAC→AAC→AAC-Legacy = 4 full
  GAMDL re-downloads), the 22-minute hard timeout was firing while tier
  2 or 3 of 4 was still legitimately running.

  - New compute_companion_timeout(track_count, tier_count): adds 8 min
    per planned tier on top of the enrichment budget, same 4-hour cap.
  - CompanionTaskHandle::tier_count() exposes the already-tracked
    planned_tiers length (no new state).
  - Soft/hard companion + enrichment timeout messages now identify the
    affected item by Artist — Album (URL fallback), matching the
    pattern from c7ed212. Same fix applied via format_content_label.
  - 5 new unit tests cover zero/single/four-tier scaling, the cap, and
    monotonicity in tier count. Existing 7 enrichment timeout tests
    unchanged and still passing.

- **(queue)** Enforce strictly-serial post-processing via ActiveSlotGuard (#706)

The success path used to call q.on_task_finished() at line 6246 (pre-fix)
  — right after primary GAMDL exited but BEFORE the completion task took
  over. That early decrement freed the queue slot while companions and
  enrichment were still running, so any subsequent process_queue invocation
  (user IPC, fallback retry, lib.rs startup recovery) could pick up the
  next item and run its primary GAMDL in parallel with the previous item's
  post-processing. The status bar showed "2 downloading" with max_concurrent: 1,
  and two completion tasks could fire their hard timeouts in the same
  wall-clock second — exactly the cross-contamination scenario #455 / #452
  were designed to prevent.

  The fix moves the slot release into the completion task, atomic with
  set_complete inside the same lock acquisition. To make sure a panic,
  abort, or runtime shutdown inside the completion task cannot leak the
  slot (and stall the queue forever), the task takes ownership of an
  ActiveSlotGuard RAII guard on spawn:

    - Happy path: explicit q.on_task_finished() then guard.disarm() →
      Drop is a no-op, slot released exactly once.
    - Panic / abort path: Drop fires a fire-and-forget tokio::spawn to
      acquire the lock and decrement active_count.

  The other 7 on_task_finished() call sites (5898, 6092, 7998, 8073, 8119,
  8154, 8171) all live on terminal error paths that never spawn a
  completion task, so they keep their existing behaviour.

  3 new unit tests:
    - active_slot_guard_disarm_does_not_release
    - active_slot_guard_drop_releases_slot
    - on_task_finished_saturates_at_zero (defence in depth — even if a
      double-release ever slips through, active_count cannot underflow)

  Full suite: 965 passed (962 + 3), 1 ignored, 0 failed. Clippy clean.

- **(queue)** Serial post-processing + per-tier timeouts + content labels (#707)

## Summary

  Three independent download-queue reliability fixes, bundled because they
  were discovered in one debugging session and share the same area of
  `download_queue.rs`:

  - **#706 — Strictly-serial post-processing.** The success path used to
  call `q.on_task_finished()` immediately after primary GAMDL exit,
  decrementing the slot while companions and enrichment were still
  running. That let any concurrent `process_queue` invocation (user IPC,
  fallback retry, startup recovery) start the next item in parallel,
  violating the #455 contract and re-introducing the metadata
  cross-contamination risk #452 was designed to prevent. Fixed by moving
  the slot release into the completion task and adding an
  `ActiveSlotGuard` RAII guard so a panic / abort / shutdown cannot leak
  the slot.

  - **#705 — Companion-phase timeout scales with tier count.**
  `compute_completion_timeout()` was sized for enrichment alone (10 min
  base + 10 s/track) and reused for the companion-wait branch. With
  multi-tier modes (Atmos → ALAC → AAC → AAC-Legacy = 4 full GAMDL
  re-downloads), the 22 min hard timeout was firing while tier 2 of 4 was
  still legitimately running. New `compute_companion_timeout(track_count,
  tier_count)` adds 8 min × tier_count on top of the enrichment budget.
  `CompanionTaskHandle::tier_count()` reads the already-tracked
  `planned_tiers.len()` — no new state.

  - **Content labels in activity-log timeout / exhaustion messages.** Six
  previously ambiguous messages (3 codec-exhaustion at lines
  5728/5769/7885 pre-fix, 3 timeouts at the soft companion / hard
  companion / enrichment sites) now identify the affected item by Artist —
  Album (URL fallback) via `format_content_label(&QueueItemStatus)`.
  Useful when a 51-item queue has multiple items in flight.

  ## Commits

  | Hash | Title |
  |---|---|
  | `af7d48b` | fix(queue): identify content in codec-exhaustion activity
  log messages |
  | `58c97df` | fix(queue): scale companion-phase timeout by tier count
  (#705) |
  | `ab0214c` | fix(queue): enforce strictly-serial post-processing via
  ActiveSlotGuard (#706) |


### 📚 Documentation

- **(security)** Update supported versions to 0.53.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- Serialise changelog/release-please workflows + install git-cliff binary
- Serialise changelog/release-please workflows + install git-cliff binary (#703) (#704)

## Summary


## [0.53.2] - 2026-05-06

### 🐛 Bug Fixes

- **(queue)** Stop classifying per-track codec skips as download failures
- **(queue)** Stop classifying per-track codec skips as download failures (#698) (#699)

### 📚 Documentation

- **(security)** Update supported versions to 0.53.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- **(changelog)** Regenerate from clean state on each retry attempt
- **(changelog)** Regenerate from clean state on each retry attempt (#700) (#701)

### 🧹 Maintenance

- **(deps-dev)** Bump ip-address from 10.1.0 to 10.2.0

Bumps [ip-address](https://github.com/beaugunderson/ip-address) from 10.1.0 to 10.2.0.
  - [Commits](https://github.com/beaugunderson/ip-address/commits)

  ---
  updated-dependencies:
  - dependency-name: ip-address
    dependency-version: 10.2.0
    dependency-type: indirect
  ...

- **(deps-dev)** Bump ip-address from 10.1.0 to 10.2.0 (#695)

Bumps [ip-address](https://github.com/beaugunderson/ip-address) from
  10.1.0 to 10.2.0.
  <details>
  <summary>Commits</summary>
  <ul>
  <li>See full diff in <a
  href="https://github.com/beaugunderson/ip-address/commits">compare
  view</a></li>
  </ul>
  </details>
  <br />


  [![Dependabot compatibility
  score](https://dependabot-badges.githubapp.com/badges/compatibility_score?dependency-name=ip-address&package-manager=npm_and_yarn&previous-version=10.1.0&new-version=10.2.0)](https://docs.github.com/en/github/managing-security-vulnerabilities/about-dependabot-security-updates#about-compatibility-scores)

  Dependabot will resolve any conflicts with this PR as long as you don't
  alter it yourself. You can also trigger a rebase manually by commenting
  `@dependabot rebase`.

- **(security)** Suppress glib-0429 advisory and prune stale entries

Identified during the post-#688 security audit (#693).

  `cargo audit` flags 20 RustSec advisories; all 20 are non-exploitable
  in MeedyaDL's threat model and either already suppressed in deny.toml
  or now added in this commit.

- **(security)** Suppress glib-0429 advisory and prune stale entries (#693) (#697)

## Summary

  Post-#688 security audit. Closes #693.

  \`cargo audit\` flags 20 RustSec advisories; **all 20 are
  non-exploitable in MeedyaDL's threat model** and either
  already-suppressed in [\`deny.toml\`](src-tauri/deny.toml) or now added
  in this PR. \`npm audit\` is clean (0 vulnerabilities).

  ## What changed

  - **Added \`RUSTSEC-2024-0429\` suppression** — glib::VariantStrIter
  Iterator/DoubleEndedIterator unsoundness. Linux-only (transitive via
  \`webkit2gtk → wry → tauri\`); trigger condition (constructing a
  VariantStrIter and iterating it) is not user-reachable from MeedyaDL's
  code paths. Patch requires glib 0.20+, blocked by upstream Tauri's
  pending GTK4 migration. Tracked in #696.
  - **Removed two stale suppressions** — \`RUSTSEC-2025-0057\` (fxhash)
  and \`RUSTSEC-2026-0097\` (rand 0.7.3 build-only). Upstream
  \`tauri-utils\` dropped both chains; \`cargo deny\` now reports them as
  \`advisory-not-detected\`. Pruned to keep the suppression list honest.
  The detailed reachability block for the rand entry is removed since it's
  no longer relevant; the carve-out (b) policy comment remains as guidance
  for any future entries.

  ## Manual security review (no findings)

  A targeted Explore-agent audit verified the security baseline against
  drift:

  | Threat class | Result |
  | --- | --- |
  | Path traversal (\`validate_path_safe()\` guards on every IPC accepting
  paths) | ✅ intact |
  | Subprocess argument handling (zero \`sh -c\` patterns; all \`.arg()\`
  parameterised; URL scheme validation before GAMDL) | ✅ intact |
  | IPC rate limiting (\`start_download\` 10/min, \`check_all_updates\`
  1/min, \`download_and_install_app_update\` 1/min,
  \`import_cookies_from_browser\` 3/min) | ✅ intact |
  | Credential storage (keychain-only via \`keyring\` crate; nothing in
  \`settings.json\`) | ✅ intact |
  | Wrapper-URL redaction in logs (\`redact_url_query()\` + verbose-only
  \`[REDACTED]\` strip) | ✅ intact |
  | Settings/queue file integrity (SHA-256 checksum on \`settings.json\`;
  atomic temp+rename writes; settings migration v0→v4) | ✅ intact |
  | Tauri capability scope (\`shell:allow-open\` only — not the broader
  \`shell:default\`; \`fs:default\` is app-scoped in Tauri 2.x) | ✅ intact
  |
  | Newly-added IPC commands from #685 (\`delete_queue_item\`,
  \`delete_history_entry\`) | ✅ both accept only typed UUIDs, no
  paths/URLs/shell args |

  ## Verification

  - \`cargo deny check\` — \`advisories ok, bans ok, licenses ok, sources
  ok\`.
  - \`cargo audit\` — 0 vulnerabilities, all 20 advisories explicitly
  handled.
  - \`npm audit --audit-level=low\` — 0 vulnerabilities.
  - \`cargo clippy --all-targets -- -D warnings\` — clean.
  - \`cargo test --lib\` — 951 / 0.

  ## Out of scope (filed as follow-ups)

  - **#696** — track upstream Tauri's GTK4 migration. This is the root
  cause for 12+ of the suppressed advisories; resolving it removes them
  all in one go.

  ## Test plan

  - [ ] CI \`cargo deny\` job passes on Linux, macOS, Windows.
  - [ ] No regressions in existing security paths (path validation,
  subprocess argument handling, settings migration).

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


## [0.53.1] - 2026-05-05

### 📚 Documentation

- **(security)** Update supported versions to 0.53.0 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### ⚡ Performance

- Cache regexes on hot paths and clean clippy test warnings

Identified during the post-#685 audit (#688). Two perf bugs fixed +
  clippy clean across the test tree.

  Perf — Rust regex caching (hot paths):

  - dependency_manager.rs: 6 distinct regex::Regex::new() calls inside
    parse_version_tuple() and extract_version_from_output() compiled
    fresh on every invocation. Both run on every app startup and every
    "Check for updates" click. Hoisted to LazyLock statics
    (VERSION_TUPLE_RE, SEMVER_RE, MP4BOX_RE, MP4DECRYPT_RE, NM3U8DL_RE,
    MEDIAINFO_RE) — same pattern as apple_music_api.rs and process.rs.
  - update_checker.rs: check_component_update() compiled the same
    semver-extraction regex on every poll. Hoisted to a SEMVER_EXTRACT_RE
    LazyLock.

  Clippy — test-tree warnings (8 total, all in test code):

  - update_checker.rs::test_channel_filter_promotion: 4 nested-not asserts
    (assert!(!(x >= y))) rewritten as assert!(x < y) — what they actually
    mean.
  - process.rs: 4 single-arm `match parse_gamdl_output(...) { TrackInfo
    { .. } => panic!(...), _ => {} }` rewritten as `if let TrackInfo { .. }
    = parse_gamdl_output(...) { panic!(...) }`.

- Cache version-detection regexes on hot paths (#688) (#692)

## Summary

  Post-#685 code-quality + performance sweep. Two perf fixes on hot Rust
  paths plus a clippy cleanup so the test tree builds clean under \`-D
  warnings\`. Three larger findings deferred to dedicated follow-up issues
  with full root-cause analysis.


## [0.53.0] - 2026-05-05

### ✨ Features

- **(queue,history)** Add per-item delete to Queue and History
- **(queue,history)** Add per-item delete (#685) (#686)

## Summary

  Adds per-item delete to the Download Queue and Download History pages,
  filling the gap where the only removal options were bulk Clear Finished
  / Clear All / Clear History. Common use case: purge a stubbornly-failing
  entry without nuking the rest.


### 📚 Documentation

- **(security)** Update supported versions to 0.52.4 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.52.4] - 2026-05-04

### 🐛 Bug Fixes

- **(release)** Require conventional PR titles
- **(release)** Require conventional PR titles (#683)

## Summary
  - add a PR Title workflow that validates pull request titles with the
  existing commitlint Conventional Commit config
  - prevent squash-merge titles like 'Fix retry dedupe...' or 'Make
  companion timeout...' from being merged and then ignored by Release
  Please
  - make this fix itself a conventional commit so Release Please has a
  releasable commit after this PR lands

  ## Root Cause
  Release Please is not failing. The latest Release Please logs show it
  skipped release creation because it could not parse recent merged commit
  titles and then reported: 'No user facing commits found since v0.52.3'.
  The recent PRs were merged with non-conventional squash titles, so
  Release Please ignored them.

  ## Expected Flow After Merge
  1. This PR merges with a conventional squash title.
  2. Release Please sees fix(release): require conventional PR titles.
  3. Release Please creates or updates the release PR for the next patch
  version.
  4. Merging that release PR creates the tag/GitHub Release and triggers
  the Release workflow.

  ## Verification
  - printf '%s\n' 'fix(release): require conventional PR titles' | npx
  commitlint
  - printf '%s\n' 'Make companion timeout advisory before hard abort' |
  npx commitlint (fails as expected)
  - git diff --check


### 📚 Documentation

- **(security)** Update supported versions to 0.52.3 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.52.3] - 2026-05-03

### 🐛 Bug Fixes

- **(release)** Avoid parallel updater manifest uploads

### 📚 Documentation

- **(security)** Update supported versions to 0.52.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- **(config)** Isolate GAMDL wrapper capability checks

## [0.52.2] - 2026-05-02

### 🐛 Bug Fixes

- **(queue)** Unblock #666 storefront fallback on GAMDL v3.4+ + detect MV cover bug

Two related fixes for failure shapes seen in real user runs (2026-05-02
  session: 78-error visualizer album, 77 distinct AMP `Resource Not Found`
  hits, zero storefront-fallback firings).

- Unblock #666 storefront fallback on GAMDL v3.4+ + detect MV cover bug (#674)

## Summary

  Two fixes that came out of investigating today's user-reported failures
  (queue with 8 failed items, all `GAMDL reported N per-track error(s)
  even though the process exited 0`).

  | Issue | What | One-line fix |
  | --- | --- | --- |
  | **#672** | #666 storefront fallback was a no-op on GAMDL v3.4+ | The
  detector buffer (`raw_stderr_lines`) was only fed by stderr. GAMDL v3.4
  moved logging to stdout, so the buffer stayed empty. Renamed to
  `raw_output_lines` and have both readers append to it. |
  | **#673** | Music-video albums fail every track with a confusing
  generic error | Added `is_gamdl_mv_cover_template_bug` detector and a
  focused user-facing message replacing the generic per-track-error count.
  |

  ## Captured user evidence

  Today's session log (1-day window) shows:
  - **77 distinct AMP "Resource Not Found" 404s** — these should have
  triggered the auto-retry-with-account-region path from #666 but didn't.
  - **78 `httpx.HTTPStatusError: 400 Bad Request`** for
  `mzstatic.com/Video.../{w}x{h}mv.jpg` URLs — every track of a 78-track
  music-video album failed because GAMDL didn't substitute the cover-URL
  placeholders.
  - **Zero** activity-log lines mentioning `Storefront fallback` or
  `account region` for the same window.

  Both root causes confirmed at the source:
  - #672: `raw_stderr_lines` declared at `download_queue.rs:8088` was only
  written from the stderr task. GAMDL v3.4+'s `structlog` migration to
  stdout (documented in `CLAUDE.md` and
  `.github/audits/gamdl-v3.4-v3.5-audit.md`) means the buffer is empty
  when `is_storefront_mismatch_error` runs at line 8508.
  - #673: Real captured error: ``httpx.HTTPStatusError: Client error '400
  Bad Request' for url
  'https://a1.mzstatic.com/Video221/v4/.../%7Bw%7Dx%7Bh%7Dmv.jpg'`` —
  `%7Bw%7Dx%7Bh%7D` is URL-encoded `{w}x{h}`, the literal placeholder
  GAMDL was supposed to substitute.

  ## What changed

  - `src-tauri/src/services/download_queue.rs`
  - Renamed `raw_stderr_lines` → `raw_output_lines` and added a clone +
  push from the stdout reader so both streams feed the consumer buffer.
  - Soft-error path now checks `is_gamdl_mv_cover_template_bug` BEFORE the
  storefront detector, returning a focused upstream-bug message that names
  the cause + links to the GAMDL issue tracker + warns that no audio
  downloaded.
  - `src-tauri/src/utils/process.rs`
  - New `is_gamdl_mv_cover_template_bug(error_message)` requiring all
  three signals (`400 Bad Request` + `mzstatic.com/Video` + raw or
  URL-encoded `{w}x{h}` template). Won't match generic 400s, won't match
  the storefront 404 shape.
  - 4 new unit tests for the cover-template detector.

  ## Tests

  - 8 targeted tests pass (4 storefront-mismatch, 4 cover-template).
  - `cargo clippy -- -D warnings` clean.
  - Frontend: 303 tests pass, type-check clean.

  **Note on the parallel-test flake.** Adding any new tests to the suite
  reshuffles the scheduler and exposes a pre-existing race against
  `gamdl_capabilities`'s global version cache
  (`ini_includes_wrapper_m3u8_ip_on_v31` flakes when run in parallel with
  sibling `ini_omits_wrapper_m3u8_ip_on_v30`). Both pass in isolation;
  tracked separately for a `serial_test`-style fix.

  ## Test plan

  - [ ] **#672 verify:** Queue a `/us/album/X` URL that you know is
  region-locked away from the US, on a `gb` account. Activity log should
  now show `Storefront 'us' returned no catalog entry — retrying with your
  account region 'gb'…` and the retry should land.
  - [ ] **#673 verify:** Queue a music-video-heavy album (e.g. an
  Anniversary Edition with visualizers). When it fails, the queue item's
  error text should now read the focused `GAMDL bug — music-video cover
  URL not templated…` message instead of `GAMDL reported N per-track
  error(s) even though the process exited 0`.

  ## Closes

  - Closes #672
  - Closes #673

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


### 📚 Documentation

- **(security)** Update supported versions to 0.52.1 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.52.1] - 2026-04-30

### 🐛 Bug Fixes

- **(deps)** Bump @tauri-apps/api + cli to 2.11.0 to match Rust crate

The Release workflow for v0.52.0 failed on every platform with:

    Found version mismatched Tauri packages.
    tauri (v2.11.0) : @tauri-apps/api (v2.10.1)

  `Cargo.toml` had `tauri = "2"` which cargo resolved to the latest
  2.11.0 release. `package.json` had `@tauri-apps/api` and
  `@tauri-apps/cli` pinned to `^2.10.1`, so the npm lockfile stuck on
  2.10.1 even after Tauri 2.11.0 dropped. The newer tauri-cli's
  preflight mismatch check then refused to build.

  Bumped both npm pins to `^2.11.0` and refreshed `package-lock.json`.
  `npx tauri info` now reports tauri / @tauri-apps/api /
  @tauri-apps/cli all on 2.11.0.

  The accompanying `src-tauri/gen/schemas/*.json` updates are the
  expected 2.11.0 schema diff — adds the new
  `core:app:allow-supports-multiple-windows` permission. No
  capability/.json file in this repo references the new permission
  yet; the schema bump is metadata-only.

  Verified locally: `npm run type-check`, `npm run build`, `npm run
  test` (303 pass), `cargo test --lib` (930 pass), `cargo clippy
  -- -D warnings` clean.

  Fixes the Release workflow failure for v0.52.0.

- **(deps)** Bump @tauri-apps/api + cli to 2.11.0 (unblock v0.52 Release) (#670)

## Summary

  The **Release** workflow for **v0.52.0** failed on every platform (run
  [25191197362](https://github.com/MWBMPartners/MeedyaDL/actions/runs/25191197362))
  at the `npm run tauri build` step with:

  \`\`\`
  Found version mismatched Tauri packages.
  Make sure the NPM package and Rust crate versions are on the same
  major/minor releases:
  tauri (v2.11.0) : @tauri-apps/api (v2.10.1)
  \`\`\`

  ## Root cause

  - `Cargo.toml` had `tauri = "2"` → cargo resolved to the latest 2.11.0
  release.
  - `package.json` had `@tauri-apps/api` / `@tauri-apps/cli` pinned to
  `^2.10.1` → the npm lockfile stayed on 2.10.1 even after Tauri 2.11.0
  dropped.
  - The newer tauri-cli's preflight mismatch check refused to build.

  ## Fix

  - `package.json`: bumped both `@tauri-apps/api` and `@tauri-apps/cli`
  pins to `^2.11.0`.
  - `package-lock.json`: refreshed via `npm install`.
  - `src-tauri/gen/schemas/*.json`: regenerated by tauri-cli on the bump —
  adds the new `core:app:allow-supports-multiple-windows` permission. No
  capability/.json file in this repo references it yet; the schema bump is
  metadata-only.

  `npx tauri info` now reports tauri / @tauri-apps/api / @tauri-apps/cli
  all on **2.11.0**.

  ## Verified locally

  - `npm run type-check`: clean
  - `npm run build`: succeeds
  - `npm run test`: **303 pass**
  - `cargo test --lib`: **930 pass, 1 ignored**
  - `cargo clippy -- -D warnings`: clean

  ## Test plan

  - [ ] CI on this PR is green on all platforms.
  - [ ] After merge, manually re-trigger the Release workflow against the
  `v0.52.0` tag (or it will pick up automatically on the next tag).

  ## Follow-up — should we widen the cargo pin?

  `tauri = "2"` is intentionally permissive. The mismatch only happens
  when the upstream cuts a minor *between* our last npm-install and our
  next release build. We could pin both sides to `~2.11` for tighter
  coupling, but that just defers the problem to the next minor bump and
  adds a manual step. Alternative: a CI job on a daily cron that runs `npx
  tauri info` and opens a PR if the two sides drift. Tracked separately
  (no issue yet — file if you want me to).

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


### 📚 Documentation

- **(security)** Update supported versions to 0.52.0 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.52.0] - 2026-04-30

### ✨ Features

- **(queue)** Auto-retry failed downloads with account region storefront (#666)

When a user pastes a URL with a storefront other than their account
  region (e.g. /us/album/X on a GB account) and the download fails
  because the album either isn't in the URL's catalog or the user's
  account can't license it from there, MeedyaDL now retries once with
  the user's account-region storefront. The original-URL behaviour is
  preserved when it works — the user may legitimately want the US
  version for bonus tracks / mix variants / regional licensing.

  Captured user evidence drove this: a queue with 12 failed items, 5 of
  them /us/ URLs against a GB account, all marked failed with
  "GAMDL reported N per-track error(s) even though the process exited 0".
  The underlying tracebacks contained the AMP API "Resource Not Found"
  shape — i.e. the catalog probe found no match in the URL's storefront.

- **(queue)** Smart manifest-driven retry — only re-fetch missing tracks (#667)

Today's retry path resets the queue item to Queued and re-runs the
  full GAMDL command. GAMDL's `overwrite=false` keeps already-downloaded
  files (correct), but it wastes wall time on a fresh metadata fetch for
  every track, re-evaluates the whole companion-tier loop, and re-runs
  every enrichment stage (ReplayGain, AcoustID, MusicBrainz) against
  files we already tagged.

  This change reads the `manifest.meedyadl` written at end-of-pipeline,
  diffs the expected track set against on-disk audio files, and replaces
  the queue item's URL list with a precise per-track URL set
  (`album_url?i={song_id}`) covering only the tracks that actually
  failed. The retry call then runs a single targeted GAMDL invocation.

- **(retry)** Per-item + right-click + bulk retry UX on History and Queue (#665)

History page had no retry path at all — failed and partially-failed
  downloads were dead-ends until the user manually copied the URL back
  into the Download form. Queue page had per-item retry but no bulk
  option, so re-running 12 failed items took 12 clicks. This adds:

  History page:
    * Per-row Retry button (RotateCcw icon) on every entry whose
      `status !== 'success'`. Calls `startDownload({ urls: [entry.url] })`
      to re-enqueue (the original queue item is gone after the history
      write). Toast confirms success/failure.
    * Right-click context menu on every row with Copy URL (always),
      Retry Download (failed only), Open Folder (when path exists).
      Same set as the inline buttons — keyboard / power-user parity.
    * "Retry All Failed (N)" header button shown only when failed
      entries exist AND the user is not in a search context. Confirms
      via modal with the count, dedupes URLs (12 failed entries for the
      same URL → 1 re-enqueue), submits a single batched
      `startDownload` call. Toast summary reports duplicates skipped.

  Queue page:
    * "Retry All Failed (N)" header button. Confirms via modal,
      iterates errored items, calls `retryDownload` per item via
      `Promise.allSettled` so one bad item doesn't abort the batch.
      Each retry passes through the smart manifest planner (#667), so
      already-downloaded tracks are skipped at the planner layer —
      bulk retry of 12 album items only re-fetches the actually-failed
      tracks across them.

  Failed scope deliberately covers both:
    * Hard failures (network, auth, terminal codec exhaustion, etc.)
    * Partial-success failures (`GAMDL reported N per-track error(s)
      even though the process exited 0`) — the dominant failure mode
      in captured user evidence.

  Updated `settingsStore.test.ts` mocks to include the new
  `storefront_fallback_on_failure: true` field that landed in #666.

- **(settings)** Expose Track/Disc Number Padding controls (#587)

Settings audit found these two `AppSettings` fields had no UI surface
  at all — they were only configurable by hand-editing settings.json.
  Both govern the `{track}` / `{disc}` placeholder padding in filename
  templates and were added in #587 to fix box-set sort order
  (`100 Track.m4a` after `099 Track.m4a` instead of after `09 Track.m4a`).

  Mirrored as TypeScript types `TrackNumberPadding` and
  `DiscNumberPadding`, defaulted in settingsStore to `'auto'`, and
  surfaced as Select controls at the bottom of Settings > Templates >
  File Templates with descriptions explaining when each option matters.
  Updated settingsStore.test.ts mock to include the new fields.

  Other audit findings reviewed and confirmed already covered in UI:
  cover_art_name + cover_format (CoverArtTab), default_video_resolution
  (QualityTab), ffmpeg_path / mediainfo_path (ToolsTab),
  companion_lyrics_formats / synced_lyrics_format (LyricsTab),
  colour_blind_mode + theme_override (GeneralTab), wrapper_m3u8_ip
  (AdvancedTab), artist_auto_select_multi (QualityTab).


### 📚 Documentation

- **(security)** Update supported versions to 0.51.0 [skip ci]
- Update CHANGELOG.md [skip ci]
- **(claude)** Record PR #662 fixes in Context, Memory, and Prompts

Updates the four Claude collateral files in .claude/ to capture the
  six user-reported v0.50.1 fixes shipped on PR #662:

  - CLAUDE.md (Claude Context): six new convention bullets covering the
    notification permission preflight + style gating + Test button (#658),
    the FallbackChainList.allItems remove/re-add API (#659), the
    TracebackFrame variant + is_python_traceback_noise helper (#660),
    the set_complete/set_error terminal-state guards (#661), and the
    CompanionTaskHandle cooperative-cancel pattern (#663). Also amends
    the duplicate-URL toast note (#657) and updates the existing
    Companion downloads bullet to reference the new handle wrapper.

  - memory/project_pr662_user_session_fixes.md (Claude Memory + History):
    new project memory file recording the in-flight PR, the live state
    at session end, the architectural learnings (macOS notification
    permission once-per-bundle quirk; tokio JoinHandle::abort cannot
    preempt sync code; cooperative-cancel flag pattern; doc-list lint
    trap; lucide-react test-mock alignment) and the user's predictive QA
    cycle.

  - memory/MEMORY.md: indexes the new file with a one-line hook.

  - ProjectBrief_Chat.claude (Claude Prompts): appends a new "Session
    Prompts Archive" section below a sentinel divider, leaving the
    original frozen genesis brief intact above. Captures this session's
    user prompts in chronological order so future sessions can reload
    context without re-reading every commit.

- **(help)** Cover retry UX, storefront fallback, smart retry, padding

Bundles the help-doc updates for the features landing in this PR (#665
  retry UX, #666 storefront fallback, #667 smart manifest retry) plus
  the previously-undocumented Track/Disc Number Padding controls (#587).

  Files updated:

  - help/troubleshooting.md
    * New "Storefront Mismatch" subsection under Not Found errors,
      explaining the auto-retry-with-account-region behaviour and the
      settings escape hatch.
    * New "Retrying Failed or Partial Downloads" section covering per-
      item retry, right-click context menu, bulk Retry All Failed, and
      the smart manifest-driven retry path (with the "all tracks already
      on disk → refused" outcome explained).

  - help/faq.md
    * Expanded "Can I download content from regions other than my own?"
      to cover the auto-retry-with-account-region path and how to opt out.
    * New Q&A "How do I retry a failed download?" covering all three
      paths (per-item, History, bulk) and the smart-retry behaviour.

  - help/fallback-quality.md
    * "Reordering the Fallback Chain" rewritten to match the actual UI
      (up/down arrows, × remove, Available panel, + re-add). Old text
      described drag-and-drop which was never implemented.
    * Documents the safety guard that prevents removal of the last item.

  - help/downloading-music.md
    * New "Track and Disc Number Padding" subsection in File Naming
      explaining the Auto/None/2/3/4-digit options and the sort-order
      bug they fix on >99-track albums.
    * Queue actions list expanded: smart-retry behaviour on Retry,
      Retry without Wrapper, Retry All Failed header button, right-
      click context menu, and the History-page parity.

  - help/getting-started.md
    * Added a "If a download fails" tip after the basic-usage steps so
      new users discover the retry affordances early.

- Update CHANGELOG.md [skip ci]

## [0.51.0] - 2026-04-29

### ✨ Features

- **(settings)** Allow removing/re-adding codecs in fallback chains (#659)

The Audio Fallback and Video Fallback chains in Settings > Fallback
  only supported reordering, so users could not exclude codecs they did
  not want MeedyaDL to try (e.g. Binaural for users without binaural
  headphones, Atmos/AC3 for cookie-only users without the wrapper).

  Extends FallbackChainList with an optional `allItems` prop. When
  supplied, each row gets an X (remove) button and an "Available
  (not in chain)" panel renders below the active chain with + buttons
  to re-add previously removed entries. The remove button on the only
  remaining row is disabled — an empty chain would block every download.

  No settings schema migration needed: the chain remains a Vec<SongCodec>
  / Vec<VideoResolution> serialised as today; removed items are simply
  absent from the array. The Rust priority builder
  (merge_options/try_fallback) already handles arbitrary chain lengths.

  QualityTab callers (artist auto-select, video codec priority) do not
  pass `allItems` and continue to render as pure-reorder lists.


### 🐛 Bug Fixes

- **(toast)** Auto-dismiss duplicate-URL warning (#657)

The duplicate-URL toast emitted by DownloadForm was typed as 'warning',
  which the uiStore treats as persistent (duration = 0). Since the download
  is still queued and no user action is required, switch the type to 'info'
  so the toast picks up notification_auto_dismiss_seconds (default 5s).

- **(notifications)** Make native OS notifications actually fire (#658)

Native macOS notifications never appeared even with "Native + In-app"
  selected. Three root causes addressed:

  1. Permission silently never asked. requestPermission() only triggers
     the macOS system prompt the first time it's called per bundle ID;
     if the user dismissed the original prompt, all subsequent calls
     resolved with 'default' and sendNotification became a no-op.
     Added a startup preflight that runs once after settings load so
     the prompt appears at a predictable, visible moment.

  2. Errors swallowed. uiStore's notification path used .catch(() => {}),
     giving zero diagnostic signal. Replaced with console.warn lines that
     surface the resolved status / underlying error.

  3. Backend ignored notification_style. send_desktop_notification only
     gated on desktop_notifications:bool, so the user's in_app_only
     choice was disregarded by the Rust completion path. Added a
     notification_style != "in_app_only" gate.

  Also adds a "Send Test Notification" button to Settings > General >
  Notifications so users can verify the OS pipeline on demand.

- **(activity-log)** Suppress Python traceback noise in non-verbose mode (#660)

GAMDL (and its dependencies — httpx, async_lru, gamdl.interface) raise
  Python exceptions on certain code paths (notably music-video cover-art
  fetch), printing multi-line tracebacks to stdout. The Activity Log was
  showing two red error entries per traceback:

    1. The "Traceback (most recent call last):" header — caught by the
       legacy `traceback` keyword in Priority 7 of parse_gamdl_output.
    2. The exception summary line ("TypeError: ...") — caught by
       PYTHON_EXCEPTION_REGEX (Priority 4b).

  (1) was duplicate noise, since (2) is the meaningful one. Multiplied
  across 20+ retries on a music-video heavy album, this produced 40+
  red lines for what was actually a single, recurring upstream bug.

  The fix is layered:

  - New GamdlOutputEvent::TracebackFrame variant captures the header,
    `File "..."` stack frames, and caret highlight lines explicitly so
    the consumer can route them to a separate sink. The `traceback`
    keyword is removed from Priority 7 — the explicit variant supersedes
    it without leaving a duplicate-classification path.

  - New process::is_python_traceback_noise() is a cheap (no-regex)
    twin of the Priority 3c branch that the stdout/stderr readers use
    to gate the per-line `activity-log` Tauri event in non-verbose mode.
    The on-disk activity-log writer still records every line, so support
    requests stay debuggable.

  - The exception summary line (TypeError, ConnectError, etc.) is
    unchanged — it remains a real Error event and stays visible.

  These tracebacks originate inside upstream Python; MeedyaDL cannot
  prevent them being printed. What MeedyaDL *can* do is stop classifying
  benign noise as errors.

- **(queue)** Block terminal-state revival + clarify timeout messaging (#661)

The per-item completion task at the bottom of the download pipeline
  always called set_complete() after the post-companion advisory pass,
  even if the download itself had failed minutes earlier. Captured logs
  showed items moving Error -> Complete silently, contradicting the
  prior error toast and red activity-log entry.

  Three changes:

  1. set_complete() now refuses to overwrite Error or Cancelled. The
     completion task can call it safely; failed/cancelled items stay
     in their terminal state. Five new unit tests pin the behaviour:
     - set_complete_does_not_revive_errored_item
     - set_complete_does_not_revive_cancelled_item
     - set_error_does_not_overwrite_cancelled_item
     - set_error_does_not_overwrite_complete_item
     (plus the original happy-path tests which still pass)

  2. set_error() now refuses to overwrite Cancelled or Complete. The
     cancellation path explicitly transitions to Cancelled first; a
     late-arriving subprocess error during teardown must not flip
     that to Error and must not poison the error field.

  3. The companion-timeout activity-log message no longer claims
     "marking complete" — that wording was misleading because
     set_complete() does not actually run until after the post-
     companion advisory pass, which can take many additional minutes
     on large box sets. Replaced with "skipping remaining companions;
     final tag pass still to run". A new "Final tag pass:
     applying [Explicit]/[Clean] suffixes…" log entry fires at the
     start of the advisory pass so the long silent gap becomes visible.

  Also adds Bell + Plus to the lucide-react test mock to support the
  icons added by #658 and #659.

- **(lint)** Indent sub-bullets in parse_gamdl_output priority list

Clippy's `doc_lazy_continuation` rejected the unindented `3c.` and `4b.`
  lines I added in #660 — they were treated as continuation text of the
  preceding `3.` and `4.` items rather than separate bullets. Indented
  them as proper sub-bullets so clippy + rendered docs both stay clean.

- **(queue)** Cooperative-cancel companion task on completion-task abort (#663)

After the 10-minute companion-download deadline fired and handle.abort()
  was called, the activity log went silent — and then sprang back to life
  5–15 minutes later with a burst of "Companion: converted N TTML file(s)"
  events for the same download_id. Captured timeline:

    22:08:26  ⚠ Companion downloads timed out — handle.abort() called
    22:19:50  Companion: converted 1 TTML file(s) to Enhanced LRC
    22:19:50  Companion: converted 5 TTML file(s) to Enhanced LRC
    …       (one line per album dir, for the next several seconds)

  Root cause: run_companion_lyrics_conversion is a *synchronous* function
  called from inside the async companion task. tokio::task::JoinHandle::
  abort() only takes effect at .await points; it cannot preempt sync code.
  The conversion runs to completion (multi-minute recursive walk over the
  output library + per-album-dir conversions), emitting log lines all the
  way through.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 0.50.1 [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.50.1] - 2026-04-28

### 🐛 Bug Fixes

- **(ux)** Missing 'to' in untested-GAMDL warning message

The Updates page warning shown to users when a GAMDL release
  post-dates the last MeedyaDL validation read:

    "or wait for the next MeedyaDL version validate it."

  It should be:

    "or wait for the next MeedyaDL version to validate it."

  One-character fix (well, three-character — " to"). Spotted while
  auditing the orphan chore/claude-shared-memory branch's local-only
  commits, which had attempted earlier wording iterations on this
  same paragraph but never got pushed because upstream already
  rewrote the surrounding prose.

- **(ux)** Missing 'to' in untested-GAMDL warning message (#655)

One-word typo fix in
  [UpdatesPage.tsx#L399](src/components/updates/UpdatesPage.tsx#L399).


### 📚 Documentation

- **(security)** Update supported versions to 0.50.0 [skip ci]
- Update CHANGELOG.md [skip ci]
- Describe the seven-channel release ladder in README + Project_Plan (#635)

The previous README.md "Release channels" table was stale:
  - Claimed six channels; we have seven (RC tier missing).
  - Said Weekly tags were `-weekly.YYYYWW` and Monthly were `-monthly.YYYYMM`,
    but the actual workflows produce `YYYYMMDD` for both.
  - Said "Stable | Release-please merge" — the prose buried the actual
    trigger mechanism and ignored the version-bump.yml hotfix path.
  - Didn't mention the dev-access gating that hides the four most
    experimental tiers from the channel selector by default.
  - Didn't explain that channel discovery uses `>=` for promotion, so
    e.g. a Beta user also sees RC + Stable.

  Project_Plan.md's "Release channel ladder + nightly auto-release"
  entry was similarly stale — it said "six-tier" with weekly/monthly
  "to follow the same template" as a future item, even though the
  workflows are now shipped (PR #652 / #628). It also didn't mention
  the RC tier, the push-driven alpha/beta/rc workflows (#631), the
  split branch rulesets (#629), the channel-bump UI restrictions
  (#632), the `release.yml` auto-publish behaviour (#646), the
  `version-bump.yml` race-condition fix (#645), the security-policy
  auto-update (#633), or the realign-alpha helper (#634).

  Both docs now describe the actual current state:
  - README.md table updated to seven rows, correct suffixes, correct
    triggers, and a paragraph explaining the dev-access gating + the
    channel-promotion semantics.
  - Project_Plan.md entry rewritten as a single comprehensive
    paragraph that cross-references every shipped child issue.
  - Project_Plan.md "Last updated" bumped to 2026-04-28.

- Describe the seven-channel release ladder in README + Project_Plan (#654)

## Summary

  Closes the docs half of #635 — the CLAUDE.md half landed in PR #650, and
  PR #652 added the actual weekly/monthly workflows. With both shipped,
  README.md and Project_Plan.md can finally describe the ladder
  accurately.

  ## What changed

  ### README.md \"Release channels\" section

  | Before | After |
  | --- | --- |
  | \"six channels\" | \"seven channels\" |
  | (no RC row) | New row: \"**RC** \\| Ad-hoc \\| push to
  \`release-candidate\` branch \\| \`-rc.N\` (monotonic)\" |
  | Weekly suffix \`-weekly.YYYYWW\` (wrong) | \`-weekly.YYYYMMDD\`
  (matches what the workflow actually emits) |
  | Monthly suffix \`-monthly.YYYYMM\` (wrong) | \`-monthly.YYYYMMDD\` |
  | \"Stable \\| Release-please merge\" | \"Stable \\| Per-version \\|
  release-please-action merge or \`version-bump.yml\` \\| *no suffix*\" |
  | (no mention of dev-access gating) | New paragraph: the four
  most-experimental tiers (Nightly / Weekly / Monthly / Alpha) are gated
  behind \`dev_access_enabled\` |
  | (no mention of channel promotion semantics) | New sentence: discovery
  filter uses \`>=\`, so e.g. a Beta user also sees RC + Stable |

  ### Project_Plan.md \"Release channel ladder\" entry

  Rewritten from the original \"six-tier with weekly/monthly to follow\"
  pre-#628 wording into a single comprehensive paragraph describing the
  actual current state — seven channels, three cron-driven, three
  push-driven, plus stable. Cross-references every shipped child issue:
  #628, #629, #630, #631, #632, #633, #634, #645, #646, plus PRs #647 /
  #648 / #650 / #652.

  \"Last updated\" timestamp bumped from 2026-04-11 to 2026-04-28.

  ## Verification

  - Verified Weekly Release and Monthly Release dry-runs succeed
  end-to-end (workflow runs
  [25045099333](https://github.com/MWBMPartners/MeedyaDL/actions/runs/25045099333)
  and
  [25046735408](https://github.com/MWBMPartners/MeedyaDL/actions/runs/25046735408)
  — both completed in <40 seconds, no errors).
  - Both \`weekly\` and \`monthly\` long-lived branches now exist on
  origin at \`6b98aa4\`.

  ## Test plan

  - [x] Manual diff check — every claim cross-referenced against the
  actual workflow files / settings.rs / update_checker.rs /
  GeneralTab.tsx.
  - [x] No code changes — pure docs.
  - [ ] Maintainer eyeball review.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.50.0] - 2026-04-28

### ✨ Features

- **(release)** Weekly + monthly cron workflows + branches (#628)

The seven-tier UpdateChannel enum has had Weekly and Monthly variants
  since the channel-ladder PR landed, but the producer-side automation
  was never created — users selecting those channels in the in-app
  selector would never receive a build because no `weekly-release.yml`
  / `monthly-release.yml` existed and no `weekly` / `monthly` long-lived
  branches existed on origin.

  This commit closes that gap. The two new workflows are line-by-line
  copies of `nightly-release.yml` with channel-specific substitutions
  (name, cron schedule, branch, tag suffix, concurrency group, conflict
  label, log strings). Keeping them as parallel siblings instead of
  parametrising a single workflow keeps the diff vs nightly small and
  each cron's behaviour explicit.

- **(release)** Weekly + monthly cron workflows (#652)

## Summary

  Closes the producer-side gap for the \`Weekly\` and \`Monthly\` channel
  variants. Until now, the \`UpdateChannel\` enum had seven ordered
  variants but only five had build automation — anyone picking Weekly or
  Monthly in the in-app channel selector would never receive a build
  because no cron workflow / long-lived branch existed for them.

  ## What changed

  | File | Change |
  | --- | --- |
  | \`.github/workflows/weekly-release.yml\` | New. Line-by-line clone of
  \`nightly-release.yml\` with channel-specific substitutions. Cron \`0 0
  * * 0\` (Sundays). |
  | \`.github/workflows/monthly-release.yml\` | New. Same template. Cron
  \`0 0 1 * *\` (1st of month). |
  | \`.claude/CLAUDE.md\` | Drops the \"aspirational / tracked in #628\"
  caveat from the release-channels paragraph. |

  ## What was already in place (no change needed)

  - \`.github/rulesets/protected-cron-channels.json\` already includes
  \`refs/heads/weekly\` and \`refs/heads/monthly\` with admin-bypass for
  cron pushes.
  - \`.github/workflows/auto-delete-merged-branches.yml\` regex (line 55)
  already includes \`weekly|monthly\` in the channel exempt list.
  - \`UpdateChannel\` enum already has both variants and \`from_tag()\`
  recognises them.
  - \`release.yml\`'s #646 auto-publish step recognises any \`vX.Y.Z-*\`
  tag suffix, so the first weekly / monthly draft will auto-publish.

  ## Out-of-band

  The \`weekly\` and \`monthly\` long-lived branches don't exist on origin
  yet — they need to be created from \`main\` once this PR merges. I'll do
  that immediately post-merge with:

  \`\`\`bash
  git push origin main:refs/heads/weekly
  git push origin main:refs/heads/monthly
  \`\`\`

  The first scheduled cron run after that will force-push the integrated
  state.

  ## Why two parallel files instead of a single parametrised workflow

  Keeping nightly / weekly / monthly as parallel siblings means each
  cron's behaviour is explicit, the diff vs nightly is tiny, and a
  maintainer reading any one of them sees the full pipeline without
  chasing variables. If the three workflows ever drift in non-trivial
  ways, parametrising can come later.

  ## Test plan

  - [x] Diff verified against nightly-release.yml — only channel-specific
  substitutions changed (name, cron, branch, tag suffix, concurrency
  group, conflict label, log strings). No changes to logic, regex, or jq
  queries.
  - [ ] Smoke: next Sunday 00:00 UTC, Weekly Release fires and produces a
  published \`v0.49.X-weekly.YYYYMMDD\` release with all 20 platform
  installers.
  - [ ] Smoke: next 1st of month 00:00 UTC, Monthly Release fires and
  produces a published \`v0.49.X-monthly.YYYYMMDD\` release.
  - [ ] Manual smoke (faster): \`gh workflow run \"Weekly Release\" --ref
  main -f dry_run=true\` to validate the merge + bump dry-run path before
  the first scheduled run.


### 📚 Documentation

- **(security)** Update supported versions to 0.49.3 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.49.3] - 2026-04-28

### 🐛 Bug Fixes

- **(ci)** Auto-publish prerelease drafts at the end of release.yml (#646)

`tauri-action` creates a draft GitHub Release and never publishes it.
  For STABLE tags (`vX.Y.Z`), `release-please-action` publishes the
  release object itself before `release.yml` runs, so the draft state is
  irrelevant. For PRERELEASE tags (`vX.Y.Z-nightly.YYYYMMDD`,
  `-alpha.N`, `-beta.N`, `-rc.N`, etc.) there is no `release-please-
  action` involvement — the draft sits unpublished forever, and
  end-users see only the source-archive auto-attachments on the public
  tag page.

  This commit adds an idempotent step at the end of the
  `finalize-release` job that detects prerelease tags by hyphen suffix
  and flips `draft=false`. Stable tags are deliberately left alone —
  `version-bump.yml`'s gap is addressed separately in #645 (pre-create
  the GitHub Release object before the tag push so platform jobs can't
  race to create separate drafts).

  Verified pattern matches against the existing tag corpus:

    v0.49.2-nightly.20260428      → prerelease (auto-publishes)
    v0.49.0-nightly.20260427      → prerelease (auto-publishes)
    v0.49.0-nightly-build-46      → prerelease (auto-publishes)
    v0.35.0-nightly.20260421      → prerelease (auto-publishes)
    v0.49.2                       → stable (left alone)
    v0.49.1                       → stable (left alone)
    v0.49.0                       → stable (left alone)

  Existing draft nightlies are not retroactively published by this
  workflow change — that requires manual `gh release edit --draft=false`
  calls, which will follow in this PR's commentary.

- **(ci)** Auto-publish prerelease drafts at the end of release.yml (#647)

## Summary

  Fixes the bug where nightly (and weekly / monthly / alpha / beta / rc)
  GitHub Releases were stuck as unpublished drafts forever — end-users saw
  only \`Source code (zip/tar.gz)\` on the public tag page despite all 20
  platform installer assets being built and uploaded.

  ## Root cause

  \`release.yml\`'s \`finalize-release\` job ends by appending a download
  guide via \`gh release edit --notes\`, but it never flips the \`draft\`
  flag. For stable releases driven by \`release-please-action\`, that
  action publishes the release object itself before \`release.yml\` runs,
  so the draft state is irrelevant. For **prereleases**, there's no
  equivalent — \`nightly-release.yml\` (and friends) just push the tag and
  rely on \`release.yml\` to do the rest, and \`release.yml\` never
  publishes.

  ## Fix

  Adds an idempotent step at the end of \`finalize-release\` that:

  1. Detects prerelease tags by the hyphen suffix after \`vX.Y.Z\`
  (matches \`-nightly.YYYYMMDD\`, \`-alpha.N\`, \`-beta.N\`, \`-rc.N\`,
  \`-weekly.N\`, \`-monthly.N\`).
  2. Skips stable tags (\`vX.Y.Z\` exact) — \`release-please-action\`
  handles them on the standard path; \`version-bump.yml\`'s gap is
  addressed separately in #645.
  3. Skips already-published releases (idempotent re-runs).
  4. Calls \`gh release edit --draft=false\` to publish the prerelease.

  ## Pattern verification against the tag corpus

  | Tag | Classification | Action |
  | --- | --- | --- |
  | \`v0.49.2-nightly.20260428\` | prerelease | auto-publishes |
  | \`v0.49.0-nightly.20260427\` | prerelease | auto-publishes |
  | \`v0.35.0-nightly.20260421\` | prerelease | auto-publishes |
  | \`v0.49.2\` | stable | left alone |
  | \`v0.49.1\` | stable | left alone |
  | \`v0.49.0\` | stable | left alone |

  ## Existing draft nightlies

  This workflow change only affects the **next** nightly release. Existing
  stranded draft nightlies (\`v0.49.2-nightly.20260428\` etc.) need a
  one-time backfill via \`gh release edit \"$TAG\" --draft=false\` per
  tag, or via the GitHub UI. I'll do this manually after the PR merges.

  ## Test plan

  - [x] Pattern regex \`^v[0-9]+\\.[0-9]+\\.[0-9]+-\` verified against 6
  historical tags (4 prerelease, 3 stable).
  - [x] Idempotent — re-running on an already-published release exits
  cleanly with \"already published\" log.
  - [x] Stable tags hit the early-exit branch with a clear log message
  about why.
  - [ ] End-to-end smoke: next scheduled nightly (00:00 UTC tonight)
  produces a *published* prerelease visible on its tag page.

- **(ci)** Version-bump.yml pre-creates GitHub Release to prevent draft fragmentation (#645)

When releases are cut via `version-bump.yml` (the manual override / hotfix
  path), `release.yml`'s six platform-build jobs all run in parallel and each
  calls `tauri-action`. tauri-action's "create-or-update" logic checks for an
  existing release; if none exists, it falls back to `gh release create
  --draft "Release in progress..."`. When two jobs hit that fallback within
  the same few hundred milliseconds, GitHub's API races them and creates two
  separate draft releases for the same tag, fragmenting installer assets
  across them.

  Evidence from the v0.49.1 / v0.49.2 audit (#645):

  - v0.49.1 ended up with a published release (6 of 20 assets) plus an
    orphan draft (14 of 20 assets) that sat unmerged for ~11 hours, leaving
    Windows / Linux x64 / Linux ARM64 users with no installer.
  - v0.49.2 ended up split across THREE separate drafts (10 + 7 + 5 assets),
    none of which were published, so the public tag page showed only source
    archives until the manual consolidation on 2026-04-28.

  The fix swaps the local `git tag -a` + `git push origin <tag>` sequence
  for `gh release create <tag> --target <commit_sha> --draft`. That single
  API call atomically:

    1. Creates the tag on the remote (no `git push origin <tag>` needed).
    2. Creates the draft GitHub Release object.

  When `release.yml` triggers from the resulting tag-create event, every
  platform job's `gh release view` finds the same existing draft and
  attaches its assets to it. No race, no fragmentation.

  Notes are intentionally a short placeholder. `release.yml`'s
  `Finalize Release Notes` step appends a download guide at the end of
  the build, and `changelog.yml` regenerates CHANGELOG.md on the next
  commit. Maintainers running version-bump.yml manually can
  `gh release edit "$TAG" --notes "…"` post-build for richer notes —
  this is the manual / hotfix path, not the standard release-please flow.

  Stable releases continue to land as drafts so the maintainer can review
  platform artifacts before publishing. (Prerelease auto-publishing is
  handled separately by #646 in `release.yml`'s finalize-release job.)

- **(ci)** Version-bump.yml pre-creates GitHub Release to prevent draft fragmentation (#648)

## Summary

  Fixes the race-condition documented in #645 where manual releases cut
  via \`version-bump.yml\` end up split across two or three competing
  draft GitHub Releases — fragmenting installer assets so users see only
  some platforms (or none) on the public tag page.

  ## Root cause

  \`release.yml\` runs six platform-build jobs in parallel via
  \`tauri-action\`. Each one's \"create-or-update\" logic checks whether a
  release exists for the tag; if not, it falls back to \`gh release create
  --draft \"Release in progress...\"\`. When two jobs hit that fallback in
  the same few hundred milliseconds, GitHub's API races them and produces
  two separate draft releases, partitioning assets across them by
  platform-job timing.

  The standard release-please flow is unaffected because
  \`release-please-action\` creates the release object before
  \`release.yml\` ever runs. The manual \`version-bump.yml\` path skipped
  that pre-creation entirely.

  ## Evidence (from #645 audit)

  - **v0.49.1** — ended up with one published release (6/20 assets) and
  one orphan draft (14/20 assets) that sat unmerged for ~11 hours. Windows
  / Linux x64 / Linux ARM64 users had no installer.
  - **v0.49.2** — split across **three** separate drafts (10 + 7 + 5
  assets), none published, so the public tag page showed only source
  archives until manually consolidated on 2026-04-28.

  ## Fix

  Swap the local \`git tag -a\` + \`git push origin <tag>\` sequence for
  \`gh release create <tag> --target <commit_sha> --draft\`. That single
  API call atomically:

  1. Creates the tag on the remote (no separate tag push needed).
  2. Creates the draft GitHub Release object.

  When \`release.yml\` triggers from the tag-create event, every platform
  job's \`gh release view\` finds the same existing draft and attaches its
  assets to it. No race, no fragmentation.

  ## Why notes are a short placeholder

  \`release.yml\`'s \`Finalize Release Notes\` step appends a download
  guide. \`changelog.yml\` regenerates \`CHANGELOG.md\` with full
  git-cliff content. Maintainers running version-bump.yml manually can
  \`gh release edit \"$TAG\" --notes \"…\"\` post-build for richer notes —
  version-bump.yml is the *manual override / hotfix path*, not the
  standard automated release path. The standard \`release-please-action\`
  path produces auto-generated changelog notes already.

  ## Why drafts stay as drafts

  Stable releases continue to land as drafts so the maintainer can review
  platform artifacts before publishing. **Prerelease auto-publishing is
  handled separately by #646** in \`release.yml\`'s \`finalize-release\`
  job (auto-publishes nightlies / weeklies / monthlies / alphas / betas /
  rcs).

  ## Test plan

  - [x] Diff verified — \`git tag -a\` + \`git push origin <tag>\`
  removed; \`gh release create --target <sha> --draft\` step added.
  - [x] \`steps.commit.outputs.sha\` correctly threaded into the
  \`--target\` flag (the version-bump commit's exact SHA, before any
  subsequent \`[skip ci]\` activity).
  - [x] RELEASE_PAT used for \`gh\` (so the tag-create event triggers
  \`release.yml\`).
  - [ ] End-to-end smoke: next manual \`version-bump.yml\` run produces
  exactly **one** GitHub Release object with all 20 platform assets
  (matches v0.49.0's complete-release baseline).

  ## Related


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 0.49.2 [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(claude)** Update release-channels paragraph to reflect current state (#635)

The previous "six-tier ladder" description was stale — it claimed all
  six channels (nightly / weekly / monthly / alpha / beta / stable) had
  matching cron-driven workflows and long-lived branches. Audit (2026-04-28)
  confirmed:

  - Long-lived branches that exist: nightly, alpha, beta, release-candidate
    (RC is a real channel that wasn't even mentioned in the old text).
  - Long-lived branches that don't exist: weekly, monthly. The
    UpdateChannel enum still has Weekly and Monthly variants but
    there's no producer-side automation for them, so users selecting
    those channels in Settings will never receive a build. Tracking
    in #628.
  - Push-driven channel workflows (alpha-release.yml, beta-release.yml,
    release-candidate-release.yml) were never described.
  - Branch ruleset is now SPLIT into protected-stable-branches.json
    (no bypass) and protected-cron-channels.json (admin bypass for
    cron) — the old text referenced the singular pre-split file.
  - realign-alpha.yml and update-security-policy.yml weren't mentioned.
  - release.yml's dynamic prerelease detection and (post-#646)
    auto-publishing weren't mentioned.
  - Channel discovery uses `>=` for promotion, not exact-match — the
    old text said the opposite.

  Updated paragraph captures the actual current state without inventing
  behaviour. Project_Plan.md and README.md don't currently describe
  channels at this depth, so no updates needed there.

  Partially addresses #635 — the rest of #635 (Project_Plan.md /
  README.md updates, in-app help docs) stays open until #628's weekly/
  monthly question is decided, since the docs need to either describe
  seven channels (if weekly/monthly cron workflows ship) or five (if
  they're dropped).

- **(claude)** Update release-channels paragraph to reflect current state (#650)

## Summary

  The CLAUDE.md "Release Channels (six-tier ladder)" section was stale.
  Audit (2026-04-28) found:

  - It claimed all six channels (nightly / weekly / monthly / alpha / beta
  / stable) had matching cron workflows. **Weekly and monthly cron
  workflows don't exist**; tracking in #628.
  - It described a single combined branch ruleset, but the rules have
  since been split into \`protected-stable-branches.json\` (no bypass) and
  \`protected-cron-channels.json\` (admin bypass for cron pushes).
  - It didn't mention the \`Rc\` channel variant or the push-driven
  \`alpha-release.yml\` / \`beta-release.yml\` /
  \`release-candidate-release.yml\` workflows.
  - It didn't mention \`realign-alpha.yml\` or
  \`update-security-policy.yml\`.
  - It said channel discovery is exact-match, but \`update_checker.rs\`
  actually uses \`>=\` for channel promotion.

  This patch rewrites the section to describe the current actual state,
  marks \`Weekly\` / \`Monthly\` as aspirational variants pending #628's
  resolution, and links to the relevant follow-up issues.

  ## Why partial-close, not full-close, of #635

  #635 covers Project_Plan.md and README.md updates too. Those should
  reflect either a seven-channel or five-channel ladder depending on how
  #628 (weekly/monthly cron workflows) is resolved. Updating them now
  would just need to be redone post-#628. CLAUDE.md is the most
  actively-consulted doc and was actively misleading; the rest can wait.

  ## Test plan

  - [x] Manual diff check — every claim in the new paragraph
  cross-referenced against the actual files (\`alpha-release.yml\`,
  \`beta-release.yml\`, \`release-candidate-release.yml\`,
  \`update_checker.rs\`, \`GeneralTab.tsx\`, the two ruleset JSONs).
  - [x] No code changes — pure docs edit.
  - [ ] Maintainer eyeball review.

- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- **(claude)** Share project Claude memory across dev machines (#643)

Until now, Claude Code's per-project memory store has lived only at
  `~/.claude/projects/<sanitised-repo-path>/memory/` — a per-user,
  per-machine location that can't be loaded directly from inside the
  repo. Every contributor's Claude session has therefore been starting
  blind to project context like the v1 RC milestone state, the
  multi-service groundwork status, the macOS updater bug history, the
  GAMDL audit cadence, etc.

  This commit adds:

  - `.claude/memory/` — canonical project-scoped memory files
    (`type: project`) plus a shared `MEMORY.md` index and a
    README documenting the convention. Six files seeded from the
    current memory store: macOS updater bug, GitHub orgs,
    meedyadl-v2 archive, v1 RC prep, multi-service groundwork,
    GAMDL release cadence.
  - `scripts/sync-claude-memory.sh` — POSIX shell script that
    computes the sanitised path for the dev's clone and copies
    every shared memory file into the dev's local memory dir.
    Merges the shared `MEMORY.md` hooks under sentinel markers
    (`<!-- claude-memory:shared:start/end -->`) so re-runs are
    idempotent and personal hooks above/below the block are
    preserved. Strips outside-block references to shared
    filenames so contributors who hand-edited their index before
    this convention existed don't end up with duplicates.
  - `.claude/CLAUDE.md` — new "Shared Claude memory" convention
    entry pointing at the script and the README.

  Scope is intentionally project-only. `type: user` and
  `type: feedback` memory stays personal in each contributor's
  home dir — committing one contributor's git-workflow
  preferences or user profile would force every other
  contributor's Claude session to inherit them, which is the
  wrong scope.

  Sync direction is intentionally repo→home only. Local edits to
  the home memory don't propagate back; to share a change, edit
  the file under `.claude/memory/` in the repo and commit. This
  keeps the repo as the deliberate source of truth and avoids
  silent drift on individual machines.

  Verified locally:

  - Fresh-install path: when no personal `MEMORY.md` exists,
    the script writes the marker block as the entire file.
  - Update path: on re-runs, the personal file is filtered
    (block stripped, duplicate references to shared filenames
    removed) and the fresh block appended.
  - Idempotency: third consecutive run produces a byte-identical
    file (md5 verified).
  - Cross-shell portability: avoids `case` inside `$()`
    (POSIX-mode bash 3.x on macOS misparses) and avoids
    multi-line `awk -v` (BSD awk rejects).

- **(claude)** Share project Claude memory across dev machines (#644)

## Summary

  - Commits the `type: project` subset of Claude Code memory into
  `.claude/memory/` so every contributor's Claude session loads the same
  project context (RC milestone state, multi-service groundwork, GAMDL
  audit cadence, macOS updater bug, GitHub org structure, meedyadl-v2
  archive).
  - Adds `scripts/sync-claude-memory.sh` — a POSIX shell script that
  copies the shared memory files into the dev's local Claude memory dir
  (`~/.claude/projects/<sanitised-path>/memory/`) and merges shared
  `MEMORY.md` hooks under sentinel markers, idempotent, repo→home only.
  - Adds `.claude/memory/README.md` documenting the convention and
  `.claude/CLAUDE.md` gets a short \"Shared Claude memory\" entry pointing
  at the script.

  ## Scope choice: project-only


## [0.49.2] - 2026-04-27

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- **(security)** Update supported versions to 0.49.1 [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- **(gamdl)** Admit v3.4 and v3.5 to the support window (#641)

Audit of upstream GAMDL v3.4 (3.3..3.4: 11 commits, 9 files) and v3.5
  (3.4..3.5: 4 commits, 3 files) confirms zero MeedyaDL-facing surface
  change. No CLI flags added/removed, no INI keys changed, no
  output-format regressions, no `GamdlFeature` gate adjustments.

  v3.4 swapped GAMDL's logging output stream from stderr to stdout
  (`logging.StreamHandler` → `structlog.PrintLoggerFactory(file=
  CustomOutputWriter([sys.stdout]))`). Benign — both reader tasks in
  `download_queue.rs` parse identically via `parse_gamdl_output()`. As
  a free side effect, the latent `──── [Track N/M] Downloading
  "Title" ────` cosmetic separator (only emitted from the stdout
  reader) starts firing reliably from 3.4+ because TrackInfo lines
  now arrive on stdout instead of stderr. v3.4 also enriched the
  subprocess-failure message format to embed the failing subtool's
  own stderr, which strictly improves `process::classify_error()`
  accuracy.

  v3.5 is a 4-commit fix to GAMDL's iTunes lookup HTTP layer
  (`follow_redirects=True`, `X-Apple-Store-Front` header tweak,
  `storefront_id=None` for non-US storefronts). Pure upstream win for
  music-video metadata coverage, especially for non-US users.

- **(gamdl)** Admit v3.4 and v3.5 to the support window (#642)

## Summary

  - Audited GAMDL v3.4 (3.3..3.4: 11 commits, 9 files) and v3.5 (3.4..3.5:
  4 commits, 3 files) against MeedyaDL's integration surface — zero
  CLI/INI/output-format surface change, no `GamdlFeature` gate
  adjustments, no parser regression.
  - Bumped `tool-versions.toml` `maximum_tested_version` and
  `recommended_version` from 3.3 → 3.5; added inline 3.4 + 3.5 narratives
  mirroring the existing 3.2 / 3.3 paragraphs.
  - New audit document at `.github/audits/gamdl-v3.4-v3.5-audit.md`
  following the `gamdl-v3.2-audit.md` structure (capability gate matrix,
  finding-by-finding analysis, floor analysis, conclusion).
  - README.md Component Support Matrix bumped to `2.9.1 – 3.5`
  (recommended 3.5).
  - CLAUDE.md version-aware GAMDL dispatch paragraph appends 3.4 + 3.5
  audit notes inline.

  ## Notable findings

  **v3.4 logging stream swap stderr → stdout** (`logging.StreamHandler()`
  →
  `structlog.PrintLoggerFactory(file=CustomOutputWriter([sys.stdout]))`):
  benign because both `stdout_task` and `stderr_task` in
  `download_queue.rs` call `parse_gamdl_output()` identically. **Free UX
  win**: the cosmetic `──── [Track N/M] Downloading \"Title\" ────`
  separator (only emitted from the stdout reader) was a latent no-op
  pre-3.4 because TrackInfo log lines came from stderr; from 3.4+ it fires
  reliably.

  **v3.4 subprocess error format change** (`'\"<cmd>\" exited with code
  N'` → `'Exited with code N: <args>\\nstdout:\\n…\\nstderr:\\n…'`):
  strictly improves `process::classify_error()` accuracy because it now
  embeds the failing subtool's own stderr (network keywords, codec
  keywords, etc.).

  **v3.5 iTunes HTTP fixes** (`follow_redirects=True`,
  `X-Apple-Store-Front` header tweak, `storefront_id=None` for non-US
  storefronts): pure upstream win for music-video metadata coverage.

  Database overwrite (3.4) is scoped to `--database-path` users only —
  MeedyaDL never sets that flag.

  Full per-finding analysis lives in the audit document.

  ## Test plan

  - [x] `cargo test --lib gamdl_capabilities` — 20/20 tests pass with the
  bumped 3.5 ceiling (support window parses, recommended-inside-range
  invariant holds, classify-above-ceiling/below-floor/inside-window all
  green).
  - [ ] Manual smoke: fresh install → setup wizard installs GAMDL →
  confirm `pip install --upgrade 'gamdl>=2.9.1,<=3.5'` resolves to 3.5.
  - [ ] Manual smoke: existing 3.3 install → "Update GAMDL" → confirm
  install path lands on 3.5.
  - [ ] Manual smoke: download an album on 3.4+ → confirm the new `────
  [Track N/M] Downloading ────` separator appears in the activity log (was
  latent pre-3.4).
  - [ ] Manual smoke: download a non-US (e.g. GB / AU) artist with music
  videos → confirm 3.5 iTunes redirect fix unblocks previously-broken
  music-video metadata.


## [0.49.1] - 2026-04-27

### 🐛 Bug Fixes

- **(ci)** Remove duplicate fi in update-security-policy workflow

The Compute supported-versions table step had a stray duplicate `fi`
  after the if/else/fi block, causing the shell to exit with code 2
  ("syntax error near unexpected token").

- **(ci)** Remove duplicate `fi` in update-security-policy workflow (#638)

## Summary

  The `Compute supported-versions table` step in
  `.github/workflows/update-security-policy.yml` has a stray duplicate
  `fi` immediately after the `if/else/fi` block. With `set -euo pipefail`
  the shell exits with code 2 (`syntax error near unexpected token
  \`fi\``), failing every Update Security Policy run.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.49.0] - 2026-04-26

### ✨ Features

- **(release)** Seven-tier release-channel ladder + push-driven alpha/beta/rc

Adds a Release Candidate tier between Beta and Stable, splits the channel
  branch protection into stable (no bypass) vs. cron (admin bypass), wires
  push-driven version-bump-and-tag workflows for alpha/beta/release-candidate,
  gates Nightly/Weekly/Monthly/Alpha behind Dev Access in the settings UI, and
  auto-regenerates the SECURITY.md supported-versions table on version bumps.

  - UpdateChannel: add Rc tier between Beta and Stable; ordering Nightly <
    Weekly < Monthly < Alpha < Beta < Rc < Stable. Add is_pre_release() and
    requires_dev_access() predicates. Update tests to cover the new tier.
  - update_checker: filter releases by tag_channel >= user_channel (was ==),
    so Beta subscribers auto-promote to Rc/Stable releases without seeing
    Alpha-or-below.
  - GeneralTab: hide Nightly/Weekly/Monthly/Alpha from non-dev users; add
    ChannelSwitchWarning modal for any pre-release switch.
  - Branch rulesets: split protected-release-branches.json into
    protected-stable-branches.json (main/release-candidate/beta/alpha, no
    bypass) and protected-cron-channels.json (nightly/weekly/monthly, admin
    bypass for cron-driven force-pushes). apply-branch-rulesets.yml now
    reconciles by deleting unmanaged branch rulesets.
  - New workflows: alpha-release.yml / beta-release.yml /
    release-candidate-release.yml — push-triggered, monotonic counter across
    base versions, fast-forward push of branch + tag.
  - realign-alpha.yml: one-shot manual workflow to hard-reset alpha to main.
  - update-security-policy.yml: regenerates SECURITY.md supported-versions
    table between sentinel comments on every version bump.
  - release.yml: derive prerelease flag from tag suffix (was hard-coded true).
  - auto-delete-merged-branches.yml: add release-candidate to exempt list.
  - Bump version to 0.48.0 across package.json, tauri.conf.json, Cargo.toml,
    Cargo.lock, .release-please-manifest.json.

- **(release)** Seven-tier release-channel ladder + push-driven alpha/beta/rc (#636)

## Summary

  - Adds a **Release Candidate** tier between Beta and Stable
  (`UpdateChannel::Rc`), making the ladder seven tiers: `Nightly < Weekly
  < Monthly < Alpha < Beta < Rc < Stable`.
  - Switches update discovery from `tag_channel == user_channel` to
  `tag_channel >= user_channel`, so a Beta subscriber auto-promotes to
  Rc/Stable releases without seeing Alpha-or-below.
  - Splits branch protection: `protected-stable-branches` (main /
  release-candidate / beta / alpha — no bypass actors) vs.
  `protected-cron-channels` (nightly / weekly / monthly — admin bypass for
  cron force-pushes). `apply-branch-rulesets.yml` now reconciles by
  deleting unmanaged branch rulesets.
  - Adds push-driven `alpha-release.yml` / `beta-release.yml` /
  `release-candidate-release.yml` workflows with a monotonic counter that
  never resets across base-version bumps (`0.48.0-alpha.1`,
  `0.48.0-alpha.2`, `0.49.0-alpha.3`, …).
  - Gates **Nightly / Weekly / Monthly / Alpha** behind Dev Access in the
  settings dropdown; Beta and RC remain freely selectable but trigger a
  `ChannelSwitchWarning` confirmation modal.
  - `release.yml` now derives the GitHub Release `prerelease` flag from
  the tag suffix (was hard-coded `true`), so stable `vX.Y.Z` tags publish
  as full releases.
  - New `update-security-policy.yml` regenerates the SECURITY.md
  supported-versions table between sentinel comments on every version bump
  (pre-1.0 → "current latest 0.x.y or newer"; 1.0+ → "current full release
  only").
  - New `realign-alpha.yml` one-shot manual workflow to hard-reset `alpha`
  to `main` (used during this overhaul; remains checked in for any future
  cleanup).
  - Bumps version to **0.48.0** across `package.json`,
  `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`,
  `.release-please-manifest.json`.

  ## Test plan

  - [ ] CI green on this branch (Rust + frontend)
  - [x] `npm run type-check` clean (verified locally)
  - [x] `npm run test` — 19 files / 303 tests passing (verified locally)
  - [ ] `cargo check` — needs CI / Linux dev box with GTK system libs
  (sandbox lacks `libgtk-3-dev`, `libatk1.0-dev`, etc.)
  - [ ] After merge: trigger `Apply Branch Rulesets` workflow to install
  the split rulesets and clean up the legacy `protected-release-branches`
  ruleset
  - [ ] After merge: trigger `Realign Alpha with Main` (with the temporary
  admin bypass on `protected-stable-branches`) to put `alpha` in sync,
  then remove the bypass

  ## Notes for review

  - The `alpha` realignment must happen **after** the new rulesets land.
  Procedure documented in `realign-alpha.yml`'s header.
  - Existing channel subscribers (anyone currently on
  Nightly/Weekly/Monthly/Alpha) will keep their channel even if Dev Access
  is later disabled — the gate only affects the dropdown options, not the
  persisted setting.
  - `update_checker.rs` test suite extended to cover the new ordering, the
  `>=` filter, and `requires_dev_access()`.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.47.1] - 2026-04-26

### 🐛 Bug Fixes

- **(updates)** GAMDL upgrade follow-ups — surface real pip error + don't conflate refresh failure with upgrade failure (#626)

## Summary

  Two follow-up fixes to PR #624 (the GAMDL "Untested" upgrade flow), both
  motivated by a real reproduction in v0.47.0: the user clicked Upgrade,
  pip succeeded ("GAMDL upgraded to v3.3" appeared in the activity log),
  but the toast showed a generic "Failed to upgrade GAMDL" error.

  ### 1. Don't conflate post-upgrade refresh failure with upgrade failure
  (`008cae8`)

  The `upgradeGamdl` Zustand action wrapped both the pip upgrade and the
  post-upgrade `checkAllUpdates()` refresh in a single try/catch. The
  `check_all_updates` IPC has a 1/min rate limiter — when a user clicks
  Upgrade within 60s of the startup update check (very common), pip
  succeeds in ~1s (cached wheel) but the refresh hits the rate limiter,
  the rate-limit error gets caught by the outer try/catch, and the action
  re-throws it as if the upgrade itself failed.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.47.0] - 2026-04-25

### ✨ Features

- **(updates)** Surface above-ceiling GAMDL updates as untested + admit v3.3

The update check previously hard-capped `is_compatible` at
  `maximum_tested_version`, so any GAMDL release shipped above the
  support-window ceiling was silently filtered out of the Updates page —
  operators had no signal that a new build was waiting for validation,
  which blocked us from testing v3.2 and v3.3 against MeedyaDL until we
  shipped a new release of our own.

  Split the two concerns:
  * `gamdl_capabilities::should_offer_upgrade` now only rejects
    unparseable strings (above-ceiling versions DO surface).
  * New `gamdl_capabilities::is_above_tested_ceiling` plus
    `ComponentUpdate.is_untested` carry the warning state to the UI.
  * `UpdateBanner` and `UpdatesPage` render an amber "Untested" badge +
    disclaimer when `is_untested` is set.
  * `pip_target_spec(target)` + `install_gamdl(app, Some(target))` give
    the user-explicit Upgrade path an exact pin so pip lands on the
    version the banner advertised, instead of silently resolving down to
    the bounded `[minimum, maximum_tested]` spec.

  Also audited and admitted GAMDL v3.3 to the support window. The entire
  3.2 → 3.3 delta is one internal commit (`c83e47d`) that drops a stale
  `total=` kwarg from two `_get_*_media` calls inside
  `interface.py::_get_playlist_media` — a pure playlist-download bugfix
  with zero CLI / INI / output / regex surface changes, so admission
  needs no `GamdlFeature` gate adjustments.

  20 gamdl_capabilities tests + 10 update_checker tests + 891 total
  backend tests + 303 frontend tests all pass.

- **(updates)** Surface above-ceiling GAMDL updates as Untested + admit v3.3 (#624)

## Summary

  Two related fixes:

  1. **MeedyaDL was silently hiding GAMDL updates above the validated
  ceiling.** `update_checker::is_gamdl_compatible` was a thin wrapper over
  `gamdl_capabilities::should_offer_upgrade`, which hard-capped at
  `maximum_tested_version`. The frontend's `getActiveUpdates()` filters by
  `is_compatible`, so any PyPI release above the ceiling never reached the
  Updates page — operators had no signal that a new GAMDL build was
  waiting for validation, which created a chicken-and-egg problem (we
  couldn't validate until we shipped, and we couldn't see the upgrade
  until we'd validated).
  2. **GAMDL v3.3 audited and admitted.** Upstream shipped 3.3 the same
  day as 3.2.

  ## Fix details

  ### Architectural fix: surface untested upgrades, don't hide them

  - `gamdl_capabilities::should_offer_upgrade` now only rejects
  unparseable semver strings — above-ceiling versions DO surface.
  - New `gamdl_capabilities::is_above_tested_ceiling(version)` helper.
  - New `ComponentUpdate.is_untested: bool` field on the IPC payload
  (mirrored in TypeScript types).
  - `UpdateBanner.tsx` and `UpdatesPage.tsx` render an amber
  **"Untested"** badge + a short disclaimer paragraph for above-ceiling
  GAMDL releases.
  - The Upgrade button on those rows passes the explicit `latest_version`
  through `upgradeGamdl(targetVersion)` → `upgrade_gamdl` IPC →
  `install_gamdl(app, Some(target))` → `pip_target_spec`
  (`gamdl=={target}`), so pip lands on exactly the version the banner
  advertised instead of silently resolving down to the bounded `[min,
  maximum_tested]` spec.
  - Routine "Upgrade tested" clicks still go through `pip_version_spec`
  (no target argument), so an unaudited release can never sneak in via a
  future-version `--upgrade` resolution.

  ### GAMDL v3.3 audit

  - Reviewed the entire 3.2 → 3.3 diff: it's a single internal commit
  (`c83e47d`, "Remove total arg from media fetch calls") that drops a
  stale `total=...` kwarg from two `_get_*_media` calls inside
  `interface.py::_get_playlist_media`.
  - Pure playlist-download bugfix — **zero CLI flag changes, zero INI key
  changes, zero output/regex/format changes**.
  - `maximum_tested_version` and `recommended_version` bumped to `3.3` in
  `tool-versions.toml`. No `GamdlFeature` gate adjustments needed.

  ## Test plan

  - [x] `cargo test --lib gamdl_capabilities` — 20 tests pass (added
  `is_above_tested_ceiling_flags_future_versions`,
  `pip_target_spec_pins_exact_version`, updated
  `should_offer_upgrade_above_ceiling`)
  - [x] `cargo test --lib update_checker` — 10 tests pass
  (`test_is_gamdl_compatible` semantics updated)
  - [x] `cargo test --lib` — 891 tests pass
  - [x] `npm test` — 303 tests pass
  - [x] `cargo check` — clean
  - [x] `npm run type-check` — clean
  - [x] `npm run build` — clean
  - [ ] Manual smoke test: launch the app with GAMDL 3.0 installed,
  confirm Updates page shows GAMDL → v3.3 row with no "Untested" badge
  (3.3 is now inside the window)
  - [ ] Manual smoke test: temporarily lower `maximum_tested_version` to
  "3.2" locally, confirm the GAMDL → v3.3 row gains the amber "Untested"
  badge + disclaimer, and the Upgrade button installs exactly v3.3 (not
  v3.2)

  ## Files changed

  - `src-tauri/tool-versions.toml` — admit v3.3
  - `src-tauri/src/services/gamdl_capabilities.rs` — split
  `should_offer_upgrade` from ceiling check; add `is_above_tested_ceiling`
  and `pip_target_spec`
  - `src-tauri/src/services/gamdl_service.rs` — `install_gamdl` accepts
  optional explicit-version target
  - `src-tauri/src/services/update_checker.rs` — add
  `ComponentUpdate.is_untested`; populate for GAMDL above ceiling
  - `src-tauri/src/commands/updates.rs` — `upgrade_gamdl` IPC accepts
  `target_version`
  - `src-tauri/src/commands/dependencies.rs` — pass `None` (routine setup)
  - `src/types/index.ts` — add `is_untested` to `ComponentUpdate`
  - `src/lib/tauri-commands.ts` — `upgradeGamdl(targetVersion?)` signature
  - `src/stores/updateStore.ts` — `upgradeGamdl(targetVersion?)` action
  - `src/components/common/UpdateBanner.tsx` — "Untested" badge +
  disclaimer + target-version pin on Upgrade
  - `src/components/updates/UpdatesPage.tsx` — same as banner
  - `.claude/CLAUDE.md` — audit + fix notes


### 🐛 Bug Fixes

- **(updates)** Surface real pip error when GAMDL upgrade fails

A failed Upgrade click was collapsing into a generic "Failed to upgrade
  GAMDL" toast, hiding the actual pip stderr the Rust handler had
  returned. This made upgrade failures un-diagnosable in the field.

- **(updates)** Don't surface post-upgrade refresh failure as an upgrade failure

The `upgradeGamdl` action wrapped both `commands.upgradeGamdl()` and
  the post-upgrade `commands.checkAllUpdates()` refresh in a single
  try/catch. When pip succeeded but the refresh hit the
  `check_all_updates` IPC's 1/min rate limiter (typical when a user
  clicks "Upgrade" within 60s of the startup update check), the rate
  limit error was caught and re-thrown as if the upgrade itself had
  failed. The toast read "Failed to upgrade GAMDL" while the activity
  log showed a successful "GAMDL upgraded to v3.3" entry — confusing
  contradiction reported in #624 review feedback.

  Split the two operations:
  * The pip upgrade keeps its existing try/catch and rejection contract,
    so genuine upgrade failures still surface in the toast.
  * The post-upgrade refresh runs in its own try block. On failure we
    `console.warn`, patch `lastResult` locally (mark GAMDL's
    `current_version` to the new value and clear `update_available`),
    and clear `isUpgrading`. The next periodic refresh catches up.

  Net effect: a successful pip upgrade always reports success, and the
  Updates page reflects the new version immediately even when the
  refresh is rate-limited.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.46.0] - 2026-04-25

### ✨ Features

- **(queue)** Abort-all destructive action (#620)

Adds a one-click "Abort Queue" escape hatch that stops every active
  and queued download in a single IPC call — faster and more decisive
  than today's per-item cancel flow.

  Backend (`download_queue.rs` + `commands/gamdl.rs`):

    * New `AbortSummary` serialisable struct (queued_cancelled,
      downloading_stopped, processing_stopped).
    * New `DownloadQueue::abort_all()` method transitioning every
      non-terminal item to `Cancelled` and returning the summary.
      Terminal items (Complete/Cancelled/Error) are untouched so the
      user keeps their history.
    * New `abort_all_downloads` IPC command — persists the updated
      queue, emits a `downloads-aborted` event, writes a `[System]`
      activity-log entry. Rides the existing cancellation-poll loops
      to reap subprocesses (already `kill_on_drop(true)`).
    * Three new unit tests covering empty queue, mixed pre-states,
      and terminal-only queue.

  Frontend (`tauri-commands.ts`, `downloadStore.ts`, `DownloadQueue.tsx`):

    * `abortAllDownloads()` IPC wrapper + `AbortSummary` TS interface.
    * `downloadStore.abortAll()` action with refresh + summary toast.
    * Red-styled "Abort Queue" button in the queue header, shown only
      when there's something to abort. Confirmation modal mirrors the
      existing "Clear All" pattern.

  Remaining UX polish tracked in the issue's acceptance criteria:
  status-bar affordance, Cmd/Ctrl+Shift+. shortcut, "don't ask again",
  post-queue-action suppression, manual mid-batch abort test.

- **(gamdl)** Wire --playlist-folder-template (GAMDL v3.0+, #618)

Adds playlist-specific folder template support with mandatory version
  gating so v2.9.x users don't receive a flag that crashes Click.

- **(settings)** Settings UI for playlist_folder_template (#618)

Follow-up on #618. Adds a `TemplateBuilder` entry under Settings >
  Templates > Folder Templates for the new `playlist_folder_template`
  field, with `variableCategories={['playlist']}` so the variable-picker
  chips narrow to the playlist-scoped tokens (`{playlist_artist}`,
  `{playlist_title}`, `{playlist_id}`) already present in
  `TEMPLATE_VARIABLES`.

  The description makes the GAMDL v3.0+ requirement explicit — users on
  v2.9.x see the value persist but GAMDL falls back to the upstream
  default layout until they upgrade. This is the expectation-setting
  pattern used for `wrapper_m3u8_ip` in the Advanced tab.

- **(queue)** Abort-all UX polish — shortcut, status-bar, don't-ask, suppression (#620)

Ships the four outstanding items from #620's original acceptance
  criteria:

    1. `abort_queue_confirm` setting (default true) — user-facing
       "Don't ask again" checkbox on the confirmation modal. When
       ticked, `updateSettings({ abort_queue_confirm: false })`
       persists before the abort IPC fires, so subsequent aborts
       skip the modal entirely.

    2. Status-bar global abort affordance — red `Square` icon +
       "Abort" label, shown whenever there's a non-terminal item.
       Honours `abort_queue_confirm` via `window.confirm()` since
       StatusBar doesn't own the shared Modal component; the
       queue-page modal remains the canonical rich confirmation.

    3. Cmd/Ctrl+Shift+. keyboard shortcut. Shift-gated to avoid
       colliding with macOS Cmd+. platform interrupt. Honours the
       confirm setting the same way the StatusBar does.

    4. Post-queue-action suppression — new one-shot `recently_aborted`
       flag on DownloadQueue, armed by `abort_all()` on non-zero
       summaries, consumed by `take_recently_aborted()` at the
       post-action dispatch site. Auto-clears so subsequent
       legitimate drains still fire the configured action. New
       unit test locks the one-shot semantics.

  Type-check passes. Manual mid-batch abort test deferred to a live
  environment (sandbox can't run Tauri).


### 🐛 Bug Fixes

- **(gamdl)** Always emit --song-codec-priority, never --song-codec (#614)

The `--song-codec` single-codec flag was removed when GAMDL split
  `cli.py` into `cli/cli_config.py` in v2.9.1 — it doesn't exist in
  any release in our support window (2.9.1–3.2). MeedyaDL's
  `else if self.song_codec` fallback in `audio_cli_args()` crashed the
  subprocess with `Error: No such option: --song-codec` whenever a
  user disabled the fallback chain on any supported GAMDL version.

  Collapse the two emission paths into one: when `song_codec_priority`
  is unset, promote the scalar `song_codec` field into a one-element
  `--song-codec-priority` CSV. Safe across v2.9.1+ because the flag
  accepts `Csv(SongCodec)` and a single codec is a valid one-element
  list.

  Tests added: `song_codec_promotes_to_priority_csv`,
  `song_codec_priority_wins_over_scalar`, `song_codec_both_none_emits_nothing`,
  `song_codec_promotes_when_priority_unset`. `multiple_options_combined`
  updated to assert the new emission shape.

- **(config)** Drop vestigial song_codec / song_codec_priority INI keys (#617)

Neither key round-trips through GAMDL's INI loader on any release in
  our support window (v2.9.1 → v3.2):

    * `song_codec` was removed from the CLI in the v2.9.1 restructure; it
      never registered in the Click param set, so `cleanup_unknown_params()`
      silently drops our emission.
    * `song_codec_priority` is declared upstream as `song_codec_piority`
      (missing the `r`). `dataclass_click` propagates the Python field name
      to `click.Parameter.name`, which GAMDL uses to key INI lookups.
      Our correctly-spelled key was therefore also silently dropped.

  Codec preference reaches GAMDL via `--song-codec-priority` (emitted by
  `GamdlOptions::audio_cli_args`), which is authoritative and unaffected.
  `ini_audio_section` is now intentionally empty and kept as a section
  anchor for future audio INI keys that do round-trip through Click.

  Tests updated to assert both keys are absent (negative assertions
  covering the upstream typo form as well). File-level doc example
  updated to point at the CLI-authoritative path.

- **(config)** Drop stale song_codec_priority tests + unused import (#617, PR #621)

Two pre-existing follow-up bugs from #617 ("Drop vestigial song_codec /
  song_codec_priority INI keys") that were missed when that commit landed
  and surfaced as backend CI failures on PR #621:

  1. **Unused import** — `crate::models::gamdl_options::SongCodec` was
     left at the top of `config_service.rs` after #617 emptied
     `ini_audio_section()`. With CI running `cargo clippy -- -D warnings`,
     the unused import promoted to a hard error on every backend
     platform (macos / windows / ubuntu).

  2. **Stale `cargo test` assertions** — two tests in the
     `settings_to_ini: song_codec_priority` block still asserted the
     pre-#617 invariant ("INI must contain `song_codec_priority =`"). The
     new invariant (key never emitted regardless of `fallback_enabled`)
     is already locked in by `ini_does_not_emit_song_codec` and
     `ini_does_not_emit_song_codec_priority` further up in the file, so
     the obsolete tests are removed entirely (with a comment pointing at
     the canonical replacements).

  Verified locally: `cargo clippy -- -D warnings` clean, `cargo test --lib`
  shows 889 passed / 0 failed, all six v32_fixture_* tests green against
  the synthesised fixtures shipped earlier in this branch.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(audit)** GAMDL v3.2 audit scaffold + #614 verification

Adds `.github/audits/gamdl-v3.2-audit.md` capturing the verified facts
  behind the GAMDL v3.2 audit finding set (issues #613–#619). The #614
  section records the v2.9.1–v3.2 cross-version check that confirms
  `--song-codec` has never existed in our support window and
  `--song-codec-priority` is safe on every release we ship against, so
  no support-floor change is needed.

- **(audit)** GAMDL v3.2 #615 parser-validation findings

Records the audit verification that MeedyaDL's `TRACK_INFO_V2_REGEX` and
  `classify_error()` are unaffected by v3.2's conditional `Downloading`
  log line and the `GamdlDownloaderFlatFilterExcludedError` →
  `GamdlInterfaceFlatFilterExcludedError` rename. The rename is invisible
  to our parser (no class-name matching), so the rename is a cleanup and
  not a regression. Real-sample fixtures still need to be captured from
  a live v3.2 run.

- **(audit)** GAMDL v3.2 #616 sequential-fetch observability notes

Records the alignment between upstream's v3.2 concurrency 5 → 1 default
  flip and MeedyaDL's own serial-queue decision (#455). Docs-only follow-up:
  CHANGELOG entry + help FAQ when the tool-versions.toml 3.2 bump (#619)
  ships, no code change.

- **(audit)** GAMDL v3.2 #617 INI-typo analysis

Records verification that upstream's `song_codec_piority` (misspelled)
  dataclass field — combined with `dataclass_click`'s param-name
  propagation rule — means MeedyaDL's correctly-spelled
  `song_codec_priority` INI line has been silently dropped by every
  GAMDL release since v2.9.1. CLI emission is authoritative and remains
  unaffected. Recommended resolution is Option D (drop the INI codec
  block entirely); the CLI path has always been the one doing the work.

- **(audit)** GAMDL v3.2 #618 playlist_folder_template gating

Records the cross-version check of `--playlist-folder-template` — v3.0+
  only CLI flag — and promotes the capability-gate requirement from
  optional (as originally framed in #516's deferral) to mandatory.
  v2.9.1–v2.9.3 users would otherwise see a Click `no such option` crash.
  Feature-gate pattern mirrors `GamdlFeature::WrapperM3u8Ip` from #605.

- **(audit)** GAMDL v3.2 #619 support-window bump analysis

Records that `minimum_version = \"2.9.1\"` remains correct for v3.2 —
  every capability MeedyaDL depends on is present across the full
  2.9.1–3.2 window. The pre-existing latent bugs (#614, #617) affect
  every release in that window and don't constrain the floor, so the
  bump can proceed as soon as #614/#615 land.

- **(audit)** GAMDL v3.2 #613 umbrella roll-up

Closes the audit trail for the v3.2 compatibility umbrella. Post-audit
  verification found that #614 (`--song-codec` crash) and #617 (INI-key
  typo) are pre-existing since v2.9.1, not v3.x regressions. The v2.9.1
  floor stays intact, and the `--song-codec-priority`-only fix strategy
  unlocks both bugs in one coherent change. Aligns with MeedyaDL's own
  serial-queue decision (#455).

- **(audit)** Link abort-button feature request (#620) to v3.2 audit

Records the rationale for the user-requested abort-button feature
  (#620) in the audit trail. Scenarios uncovered by the audit — such as
  the v2.9.1+ `--song-codec` crash (#614) in the `fallback_enabled=false`
  path — are exactly when a one-click abort becomes most valuable, since
  per-item cancel doesn't scale to large batch queues. No GAMDL-side
  change; pure MeedyaDL UX. Existing `ShutdownSignal` is a model for a
  narrower queue-level `AbortSignal`.

- GAMDL v3.2 sequential metadata fetch + capability notes (#616)

Adds a user-facing FAQ entry explaining why album metadata phase
  may feel slower after upgrading to GAMDL 3.2 (upstream changed the
  AppleMusicInterface concurrency default from 5 → 1, trading
  throughput for reliability against AMP API rate-limits). Highlights
  the alignment with MeedyaDL's own serial-queue decision (#455) so
  the design consistency is discoverable.

  Extended the existing "Version-aware GAMDL dispatch" CLAUDE.md
  bullet with the v3.2 behaviour + cross-references to the new
  `song_codec` / `song_codec_priority` emission rules (#614, #617),
  the playlist_folder_template gate (#618), and the abort-all queue
  action (#620). Each pulls a thread to the audit trail in
  `.github/audits/gamdl-v3.2-audit.md` for full rationale.

  CHANGELOG.md intentionally not edited — it's generated from
  conventional commits via git-cliff.

- **(audit)** GAMDL v3.2 umbrella closure roll-up (#613)

All seven child issues of the v3.2 audit umbrella (#614–#620) have
  landed on this branch. Records the per-issue commit hash + kind, and
  lists the non-blocking follow-ups tracked in their respective children
  (real-sample fixtures, Settings UI control, abort-button UX polish,
  manual smoke test). Umbrella is ready to close pending those
  follow-ups being addressed or spun out to new tickets.

- **(audit)** GAMDL v3.2 release smoke-test procedure (#619)

Committed prescriptive manual-verification checklist for whoever cuts
  the first MeedyaDL release that includes the v3.2 support-window bump.
  Covers seven scenarios with pass criteria:

    A. Fresh install resolves gamdl==3.2
    B. Existing v3.1 user sees the upgrade offer
    C. Existing v2.9.x user remains Supported (floor intact)
    D. Fallback-disabled codec path (#614 regression guard)
    E. Playlist folder template gate (#618, v3.0+ only)
    F. Abort queue end-to-end (#620) — button, status-bar, shortcut,
       "don't ask again", post-queue-action suppression
    G. Sequential metadata fetch FAQ entry (#616)

  Plus a failure-reporting template and sign-off rubric. Document can
  be forked as a template for future GAMDL version bumps.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- **(parser)** Regression tests for GAMDL v3.2 output shapes (#615)

Synthesised tests covering the v3.2 parser-adjacent changes derived
  from the source-tree diff between the 3.1 and 3.2 tags:

    * Conditional `track_log.info(f'Downloading "{media_title}"')` —
      only fires for partial media of specific types (songs, MVs,
      uploaded videos). Wrapper entities (albums, playlists, artists)
      no longer emit it. Positive tests confirm song + MV lines still
      parse as TrackInfo; negative tests confirm banner-style lines
      don't.
    * `GamdlInterfaceFlatFilterExcludedError` rename — both the old
      and new class names classify identically via `classify_error()`
      and fall through to `unknown`, not a real error bucket.

  Also adds `.github/audits/fixtures/gamdl-3.2/` with a `README.md`
  documenting the capture workflow + redaction checklist, so real-sample
  captures can land later without re-deriving the procedure.

- **(parser)** Synthesised v3.2 fixture files + fixture-driven tests (#615)

Follow-up on #615. Previously the v3.2 regression tests used inline
  string literals that pinned exact whitespace. Extracted five realistic
  scenarios into committed `.log` files under
  `.github/audits/fixtures/gamdl-3.2/` (album, single song, music video,
  playlist, flat-filter-excluded WARNING line), derived structurally
  from the v3.2 upstream source (`cli/utils.py` +  `cli.py`).

  New test helpers load the fixtures relative to `CARGO_MANIFEST_DIR`;
  six new fixture-driven tests assert counter values and event counts
  rather than whitespace so real-sample replacements drop in cleanly.

  Original inline-string tests preserved — they still pin exact
  alignment as a belt-and-braces check. The fixtures README documents
  the drop-in replacement workflow for anyone with a live v3.2
  environment.

- **(parser)** Add missing v3.2 stderr fixture .log files (#615, PR #621)

The v3.2 fixture-driven parser tests added in PR #621 (v32_fixture_*)
  load five `.log` files from `.github/audits/fixtures/gamdl-3.2/`, but
  only the README.md actually shipped — the `.log` files were silently
  dropped on commit by `.gitignore`'s blanket `*.log` rule.


### 🔄 CI/CD

- Harden changelog push race + document GHAS posture (#544, #564)

Wrap the changelog workflow's final git push in a pull-rebase +
  bounded-retry loop so concurrent pushes to main (release-please merges,
  fast-follow changelog runs) no longer surface cosmetic non-fast-forward
  failures.

  Expand SECURITY.md's "Reporting a Vulnerability" section to advertise
  the GitHub Private Vulnerability Reporting form as the preferred
  channel, and list the newly-enabled GHAS features (PVR, secret scanning
  + push protection, Dependabot security updates) alongside the already-
  live CodeQL security-and-quality query suite.

- Harden changelog push race + document GHAS posture (#544, #564) (#622)

### 🧹 Maintenance

- **(gamdl)** Bump support window to GAMDL 3.2 (#619)

All gating child issues for the v3.2 umbrella (#613) have landed on
  this branch:

    * #614 — --song-codec crash fix
    * #615 — parser regression tests
    * #616 — sequential-fetch docs
    * #617 — INI codec-block cleanup
    * #618 — --playlist-folder-template wiring
    * #620 — abort-all queue action

  `install_gamdl()` now resolves `pip install --upgrade 'gamdl>=2.9.1,<=3.2'`,
  classify() reports Supported for v3.2, and `is_gamdl_compatible("3.2")`
  returns true so the UpdatesPage surfaces the new release.

  README Component Support Matrix bumped to reflect the new ceiling.
  CLAUDE.md was already updated in #616 with the v3.2 behaviour notes
  + cross-references to the new capability gates.

  Manual smoke test (fresh install resolves gamdl==3.2, v3.1 user sees
  the upgrade offer) is deferred to whoever cuts the release — the
  sandbox can't run Tauri.


## [0.45.0] - 2026-04-24

### ✨ Features

- **(gamdl)** Add wrapper_m3u8_ip CLI/INI/UI support for GAMDL v3.1 (#605)

GAMDL v3.1 introduced --wrapper-m3u8-ip (default 127.0.0.1:20020) and
  changed wrapper semantics: when --use-wrapper is set on v3.1+, the HLS
  master playlist URL is fetched from a TCP socket on this address instead
  of from Apple's API response. Users running a wrapper must now expose an
  m3u8 service on the configured host:port.

  - GamdlFeature::WrapperM3u8Ip capability gate (is_version_at_least 3.1).
  - GamdlOptions + AppSettings field wrapper_m3u8_ip with
    #[serde(default)] so pre-v3.1 settings.json files get the upstream
    default 127.0.0.1:20020 on load (no schema migration needed).
  - ini_advanced_section emits wrapper_m3u8_ip only when use_wrapper=true
    AND the gate passes; merge_options() propagates the CLI arg under the
    same conditions. retry_without_wrapper() clears the new field along
    with the existing wrapper fields.
  - New preflight health check (check_wrapper_m3u8_health) does a 3-second
    TCP connect, emits PreflightCheck::WrapperM3u8 toast on failure.
  - Settings > Advanced > Wrapper UI exposes the field when wrapper is on.
  - Import sanitisation truncates to 64 chars; diff-logging redacts.

  Part of #604.

- **(ux)** Surface GAMDL v3.1 track counter + suppress 1-of-1 (#609)

GAMDL v3.1 now emits `[Track 1/1]` for single-song URLs, and
  `AppleMusicMedia.index/total` are populated across every download path
  (artist buckets, single songs, music videos). MeedyaDL's parser was
  already capturing `track_number`/`track_total` from TrackInfo events,
  but `update_item_progress()` discarded them — the QueueItemStatus
  fields existed as dead letters.

  Wire the parsed counters through to the queue item and gate the
  frontend "(Track N of M)" span on `total_tracks > 1` so single-song
  downloads show the track name only (no redundant "1 of 1").

  - update_item_progress() propagates track_number → completed_tracks,
    track_total → total_tracks on TrackInfo events.
  - QueueItem.tsx gates the counter span on total_tracks > 1.
  - Two new tests cover the 3/12 album case and the 1/1 single-song case.

  Part of #604.


### 🐛 Bug Fixes

- **(gamdl)** Stop emitting --no-exceptions on GAMDL v3.1 (#606)

Upstream commit dc6f2e8 removed every traceback.print_exc() call and
  routes exceptions through structlog's ExceptionPrettyPrinter unconditionally,
  making --no-exceptions a no-op. The flag is still accepted by the CLI
  parser but nothing consumes it.

  Keep emitting the flag on v2.x / v3.0 (where it still suppresses raw
  tracebacks) and on unknown versions (safe default — the flag is accepted
  everywhere since 2.x). Drop it only when we've positively detected 3.1+.

  - GamdlFeature::NoExceptionsFlag capability gate
    (!is_version_at_least(version, "3.1")).
  - merge_options() clears options.no_exceptions = None when detected
    version is >= 3.1.
  - verbose_gamdl_exceptions setting doc updated with v3.1 note.

  Part of #604. The companion parser work for ExceptionPrettyPrinter
  output is tracked in #607.

- **(parser)** Handle GAMDL v3.1 ExceptionPrettyPrinter output ordering (#607)

GAMDL v3.1's switch from traceback.print_exc() to structlog's
  ExceptionPrettyPrinter processor changes the stderr line ordering: the
  traceback now appears BEFORE its accompanying [ERROR HH:MM:SS ...] log
  line because the processor runs earlier in structlog's pipeline than
  custom_structlog_formatter.

  extract_python_exception() previously walked forward from the last
  Traceback header capturing the "last non-empty, non-indented line", so
  on v3.1 it would pick up the trailing [ERROR ...] structlog entry
  instead of the actual exception (e.g. KeyError: 'title').

- **(parser)** Use strip_prefix in is_structlog_line_start (clippy::manual_strip)

CI failure on PR #611 — `cargo clippy -- -D warnings` flagged the
  manual `&inside[level.len()..]` prefix-strip introduced in #607's
  is_structlog_line_start helper. Swap to the idiomatic
  `inside.strip_prefix(level)` pattern. No behaviour change — the two
  is_structlog_line_start tests still pass unchanged.

  Detected in the v3.1 rollout's ExceptionPrettyPrinter parser fix (#607),
  but is a local style cleanup rather than a separate issue.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- **(parser)** Regression tests for GAMDL v3.1 Track/URL brackets + WARNING→ERROR (#608)

Adding v3.1 parser regression tests exposed a latent bug in
  TRACK_INFO_V2_REGEX: the pattern did not tolerate the trailing space
  that GAMDL's `action=f"Track {index:>3}/{total:<3}"` emits for padded
  totals (e.g. `15 ` before `]`). The regex required a bare `\]`, so the
  `Downloading "..."` line silently routed to Unknown. Existing v3.0
  tests only asserted "no Error events", so the bug went undetected.

  Extend TRACK_INFO_V2_REGEX with `\s*` tolerance around the slash and
  before the closing bracket. Works on v2.9.x, v3.0, and v3.1 alike.

  Seven new tests cover:
  - Padded bracket formats `[Track   1/15 ]` and `[Track   1/1  ]`.
  - Dash-total fallback `[Track   1/-  ]` (media_total or "-" in v3.1)
    — must NOT parse as TrackInfo with a bogus numeric total.
  - WARNING→ERROR upgrade for URL parse errors (commit fd3b621).
  - `[ERROR ...] [URL   1/1  ] ...` and `[ERROR ...] [Track   1/1  ] ...`
    captured via ERROR_PREFIX_REGEX.
  - classify_error() on URL parse errors does not fall into the
    httpx/httpcore "network" bucket (#521 regression guard).

  Part of #604. No regression in the 82 utils::process::tests.


### 🧹 Maintenance

- **(gamdl)** Bump support window to GAMDL 3.1 (#610)

Closes the final child of #604. All sibling changes have landed on this
  branch:
    * #605 wrapper_m3u8_ip CLI/INI/UI
    * #606 --no-exceptions suppressed on v3.1 (no-op upstream)
    * #607 extract_python_exception() handles ExceptionPrettyPrinter
      output ordering
    * #608 TRACK_INFO_V2_REGEX + ERROR_PREFIX_REGEX regression tests for
      v3.1 padded brackets + URL parse ERROR upgrade
    * #609 QueueItem counter wiring + single-song 1/1 suppression

- **(scripts)** Remove unused variables flagged by CodeQL

Three dead declarations flagged by CodeQL code scanning on main
  (findings #16/#17/#18, all Note-severity):

    * scripts/generate-icons.mjs:132 — const map (stale size pairing,
      superseded by const entries on the next line)
    * scripts/generate-icons.mjs:133 — const names (unused filename list)
    * scripts/svg-to-apng.mjs:103   — const modeParam (query-param
      string built but never interpolated — the HTML template below
      uses modeConfig.mode directly in an inline <script> block)

  All three were left over from earlier iterations of the build scripts
  and have zero runtime impact. Output icons + APNGs are unaffected.

- **(scripts)** Remove unused variables flagged by CodeQL (#603)

## Summary

  Cleans up three unused-variable Note-level findings raised by CodeQL on
  `main` (findings #16/#17/#18 in the Code Scanning dashboard). All three
  are stale declarations with zero runtime impact.

  | File | Line | Variable | Why it was unused |
  |---|---|---|---|
  | `scripts/generate-icons.mjs` | 132 | `const map` | Superseded by
  `const entries` on the very next line |
  | `scripts/generate-icons.mjs` | 133 | `const names` | Filename list
  that was never consumed |
  | `scripts/svg-to-apng.mjs` | 103 | `const modeParam` | Built as a
  `?mode=...` query string, but the HTML template below uses
  `modeConfig.mode` directly in an inline `<script>` block — the param was
  never interpolated |

  ## Test plan

  - [x] `node --check scripts/generate-icons.mjs` — clean
  - [x] `node --check scripts/svg-to-apng.mjs` — clean
  - [ ] Manual run of the icon generator produces identical output (not
  run in-session; these scripts need `sharp` + `iconutil` + Puppeteer)
  - [ ] CodeQL rescan on this PR clears findings #16/#17/#18

  ## Risk

  Zero. Dead locals only — no exports, no side effects, no references
  anywhere else in the repo (verified by `grep`). Output icons and APNGs
  are unaffected.


## [0.44.2] - 2026-04-24

### 🐛 Bug Fixes

- **(settings)** Make Settings panel fill horizontal space + wrap long checkbox labels (#601)

## Summary

  Settings panel now fills available horizontal space + long checkbox
  labels no longer truncate. Addresses the UI reported in-session:
  Settings > Quality > Audio Quality > Custom Companion Codecs had "AAC
  (256kbps) Binaural (Experi..." cut off with an ellipsis, and the whole
  Settings panel was capped at 576px leaving a large empty gutter on
  anything bigger than a small laptop window.

  ## Changes

  Two surgical 1-line edits, applied across 11 files:

  1. **`max-w-xl` removed from all 10 settings tab wrappers.** The
  responsive chain (`MainLayout > main (flex-1) > SettingsPage > tab area
  (flex-1)`) already fills available width — the `max-w-xl` (576px) was
  the only constraint. With it gone, the Settings panel dynamically
  expands/shrinks with the window.

  2. **`CheckboxGroup` label span changed from `truncate` → `leading-tight
  break-words`.** Long labels now wrap within the checkbox cell instead of
  being cut off with an ellipsis. This fixes the truncation at the default
  window width, and the wrap behaviour remains correct when cells expand
  on wider windows.

  ## Affected files

  - `src/components/common/CheckboxGroup.tsx` — label wrap
  -
  `src/components/settings/tabs/{Advanced,Cookies,CoverArt,Fallback,General,Lyrics,Metadata,Quality,Templates,Tools}Tab.tsx`
  — 10 × remove `max-w-xl`

  ## Test plan

  - [x] `npm run type-check` — clean
  - [x] `npm run test -- --run` — 303 passed / 0 failed
  - [ ] Visual check: Settings panel fills window width at multiple sizes
  - [ ] Visual check: Custom Companion Codecs grid shows full labels at
  default width
  - [ ] Visual check: labels wrap gracefully when cells are narrow (e.g.,
  Settings sidebar hidden or small window)
  - [ ] No regression on any other settings tab

  ## Not included

  No design-level refactor of the responsive grid breakpoints on
  `CheckboxGroup` — on ultra-wide windows, the 2-column grid will have
  very wide cells. If that looks wrong in practice we can bump the column
  count at larger breakpoints as a follow-up.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.44.1] - 2026-04-23

### 🐛 Bug Fixes

- **(settings)** Make Settings panel fill horizontal space + wrap long checkbox labels

The Settings page was capped at max-w-xl (576px), leaving a wide empty
  right-hand gutter on anything bigger than a small laptop window. On top
  of that, CheckboxGroup's labels were `truncate`d at the cell edge, so
  labels like "AAC (256kbps) Binaural (Experimental)" in Settings >
  Quality > Audio Quality > Custom Companion Codecs got cut off with an
  ellipsis even at the default window width.

  Two surgical changes:

  1. Remove `max-w-xl` from all 10 settings tab wrappers. The Settings
     content area now expands to fill the available horizontal space and
     shrinks with the window — the responsive chain of flex-1 parents
     handles everything.

  2. Replace `truncate` with `leading-tight break-words` on the
     CheckboxGroup label span. Long labels now wrap within their cell
     instead of being cut off, eliminating the ellipsis at narrow widths
     while still looking clean when cells expand.

- **(parser)** Capture GAMDL v3.0 bracketed Track/URL error lines (#521)

The live-fire capture for #521 revealed that GAMDL v3.0 emits
  per-track/per-URL errors with two stacked bracket groups:

      [ERROR    23:02:03] [Track   1/14 ] Error downloading "Lavender Haze"

  `ERROR_PREFIX_REGEX` required the error keyword to immediately follow
  the optional structlog banner, so the `[Track ...]` infix pushed the
  line out of the regex's match space. Priority-7 keyword matching
  doesn't list "error" on its own, so these lines fell through to
  `GamdlOutputEvent::Unknown` — silently losing every track-scoped error
  from the activity log.

  The regex now permits zero or more `[...]` infixes between the banner
  and the error keyword.

  Also refines the v3.0 test fixtures with verbatim patterns from the
  captures (Starting Gamdl 3.0, `[URL   1/1  ]`, `[Track   N/M ]`, double
  traceback with `During handling of the above exception, another
  exception occurred:`, `gamdl.api.exceptions.GamdlApiResponseError`),
  and adds 7 regression tests covering:

    - bracketed [Track N/M] Error downloading lines
    - bracketed [URL N/M] Error processing lines
    - nested-exception marker captured as Error
    - multi-dot-module-path GamdlApiResponseError via PYTHON_EXCEPTION_REGEX
    - full double-traceback fixture → complete Error chain
    - Finished-with-N-errors summary survives interleaved traceback
    - experimental-codec WARNING is not misclassified as Error

  The codec-skip fixture remains synthetic — none of the four live-fire
  captures exercised a real codec-unavailable scenario (all errored on
  cover-fetch or catalog 404 before reaching a track-download stage).

- **(parser)** Capture GAMDL v3.0 bracketed Track/URL error lines (#521) (#599)

## Summary

  - Fix a latent regression in every MeedyaDL release against GAMDL v3.0
  where track-scoped errors (`[ERROR HH:MM:SS] [Track N/M ] Error
  downloading "..."`) fell through to `Unknown` and silently disappeared
  from the activity log.
  - Refine v3.0 test fixtures with verbatim patterns from the #521
  live-fire capture (2026-04-23).
  - Add 7 regression tests pinning the newly observed v3.0 formatting
  invariants.

  ## The bug

  The live-fire capture on #521 revealed that GAMDL v3.0 emits per-track
  errors with **two stacked bracket groups**:

  ```
  [ERROR    23:02:03] [Track   1/14 ] Error downloading "Lavender Haze"
  ```

  `ERROR_PREFIX_REGEX` required the `Error` keyword to immediately follow
  the optional structlog banner, so the `[Track 1/14 ]` infix pushed the
  whole line out of the regex's match space. Priority-7 keyword matching
  doesn't list `error` on its own, so these lines fell through to
  `GamdlOutputEvent::Unknown` — **silently losing every track-scoped
  error** from the activity log on v3.0.

  ## The fix

  ```diff
  -r"(?i)^(?:\[[A-Z]+\s+[\d:]+\]\s*)?(?:ERROR|error|Error):?\s+(.+)"
  +r"(?i)^(?:\[[A-Z]+\s+[\d:]+\]\s*)?(?:\[[^\]]+\]\s*)*(?:ERROR|error|Error):?\s+(.+)"
  ```

  The new `(?:\[[^\]]+\]\s*)*` group allows zero or more bracketed infixes
  between the structlog banner and the error keyword. Covers both the
  `[Track N/M ]` and `[URL N/M ]` variants observed in the capture.

  ## Fixtures updated with real data

  | Fixture | Status |
  |---|---|
  | `FIXTURE_V3_SUCCESSFUL_ALBUM` | Refreshed with verbatim v3.0
  formatting (`Starting Gamdl 3.0`, `[URL 1/1 ]`, `[Track N/M ]`) |
  | `FIXTURE_V3_AUTH_ERROR` | Replaced with the full double-traceback from
  capture D (httpx.HTTPStatusError → `During handling of the above
  exception` → `gamdl.api.exceptions.GamdlApiResponseError`) |
  | `FIXTURE_V3_CODEC_SKIPS` | Still synthetic — no capture exercised a
  real codec-unavailable scenario (all four errored on cover-fetch or
  catalog 404 pre-download) |
  | `FIXTURE_V3_NETWORK_TRACEBACK` | Unchanged (synthetic; no network
  timeout observed) |

  ## New regression tests

  ```
  v3_real_bracketed_track_error_is_captured_as_error
  v3_real_bracketed_url_error_is_captured_as_error
  v3_real_nested_exception_marker_captured_by_keyword_match
  v3_real_gamdl_api_response_error_captured_by_python_regex
  v3_real_auth_fixture_produces_full_error_chain
  v3_real_finished_summary_survives_nested_traceback
  v3_real_experimental_codec_warning_is_not_misclassified_as_error
  ```

  ## Test plan

  - [x] `cargo test --lib` — 850 passed, 0 failed
  - [x] `cargo clippy --lib --tests -- -D warnings` — clean
  - [ ] CI passes on the PR
  - [ ] No spurious errors on successful v3.0 download (activity log
  check)

  ## Still open on #521

  Codec-skip / gap-fill / `find_album_directory` remain untested against
  real v3.0 — need a successful-download capture. Likely blocked by a
  separate upstream GAMDL v3.0 cover-URL template-substitution regression
  surfaced by the same captures (see #521 analysis comment).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.44.0] - 2026-04-23

### ✨ Features

- **(progress-bar)** Intra-Processing progress fraction (#576)

Queue-level progress bar now shows visible forward motion DURING the
  enrichment phase, not a flat partial-credit value for 15–40 minutes
  on large box sets. Complements #574 (per-item caption labels) — both
  halves of the "RC polish for the download-progress surface" item land
  together.

  ### Backend

  - `QueueItemStatus` gains `processing_progress: Option<f32>` (nullable,
    serde-defaulted so old persistence files load unchanged).
  - `DownloadQueue::set_processing_progress(dl_id, progress)` clamps to
    [0.0, 1.0] and stores.
  - New `PROGRESS_*_STAGE` constants near `compute_completion_timeout`
    defining cumulative weights per enrichment stage (metadata 0.05,
    word-lyrics 0.15, LRC conversion 0.25, animated artwork 0.40,
    AcoustID 0.55, ReplayGain 0.75). Deliberately monotonic; rebalance
    later as real-world timing data from #579 repros accumulates.
  - `set_label` closure signature changes from `(label)` to
    `(label, progress)`. All 7 existing call sites updated to pass the
    corresponding stage weight. Each call still emits `queue-updated`
    (shipped in #590) so the frontend refreshes in real time.

  ### Frontend

  - `QueueItemStatus` TS type gains `processing_progress: number | null`.
  - `GlobalProgressBar.tsx` queue-level aggregation rewritten: instead
    of counting `processing` items as a flat 1.0, it now sums the
    weighted contribution per state:
      - complete / error / cancelled → 1.0
      - processing → `processing_progress ?? 0.5` (clamped)
      - downloading / queued → 0.0
    Integer "N of M complete" caption kept as-is (processing still
    counts as "done" in the integer display; the fractional upgrade
    only affects the bar fill).

  ### Why processing_progress defaults to 0.5 when null

  An item in `processing` state has produced its primary files (audio
  on disk) but hasn't yet seen its first enrichment stage emit. 0.5 is
  the same flat partial-credit value the pre-#576 UI showed; the
  upgrade from 0.5 to the stage weight is additive, never regressive.

  ### Test fixtures updated

  - `src/stores/downloadStore.test.ts` — QueueItemStatus fixture.
  - `src/components/layout/StatusBar.test.tsx` — QueueItemStatus fixture.
  - 3 backend default-construction sites in models/download.rs
    and 2 in services/download_queue.rs (MembershipFixture /
    retry-insert / from_persisted).

  ### Verified locally

  - tsc --noEmit clean
  - npx vitest run = 303 passed, 0 failed
  - cargo clippy -- -D warnings clean
  - cargo test --lib = 825 passed, 0 failed

- **(naming)** User-configurable disc + track number padding (#587)

Settings gain two new enums — `TrackNumberPadding` and `DiscNumberPadding`
  — that control how bare `{track}` / `{disc}` tokens in filename templates
  are padded with leading zeros. Closes #587's "I'd like more control over
  padding" UX request from the #547 audit (100-track Beethoven box set
  where track 100 sorted between 10 and 11 lexicographically).

  ### Core change

  - Two new enum settings (both default `Auto`):
    - `TrackNumberPadding::{Auto, None, TwoDigits, ThreeDigits, FourDigits}`
    - `DiscNumberPadding::{Auto, None, OneDigit, TwoDigits}`
  - Both enums expose `resolve_width(total) -> usize` that returns the
    number of padding digits for a given total. `Auto` derives width
    from the album's `track_total` / `disc_total`; fixed modes ignore
    the argument and return their constant width.
  - New `apply_padding_to_template()` pure function in `download_queue.rs`
    that rewrites bare `{track}` / `{disc}` placeholders to
    `{track:{width}d}` / `{disc:{width}d}`. Tokens with an explicit
    format spec (`{track:02d}`) are left untouched — the user's template
    always wins. Similar-looking tokens like `{track_total}` are
    correctly distinguished.
  - `merge_options()` now applies the padding to `single_disc_file_template`
    and `multi_disc_file_template` at merge time so GAMDL sees the
    already-formatted template.

  ### Defaults preserve existing behaviour

  - `Auto` with no album metadata known yet → 2-digit track, 0-digit
    disc. Matches pre-#587 `{track:02d}` / `{disc}-` exactly.
  - Users who've customised their templates with explicit format specs
    (`{track:03d}`) keep working identically — `apply_padding_to_template`
    is a no-op on explicit specs.

  ### Auto mode and album metadata

  `merge_options()` runs before the Apple Music API prefetch returns
  `track_total` / `disc_total`, so `Auto` currently resolves with
  `None` → fallback defaults. Upgrading `Auto` to consult the actual
  album totals (producing `001` for 200-track box sets, `01` for 12-track
  albums) is a follow-up that requires threading the totals through the
  pipeline. Fixed widths (`TwoDigits` / `ThreeDigits` / `FourDigits`)
  take effect immediately for users who want library-wide consistency
  without waiting for that follow-up.

  ### Tests

  Eight new unit tests in `download_queue::tests`:

  - `padding_leaves_explicit_format_spec_untouched`
  - `padding_substitutes_bare_track_token`
  - `padding_substitutes_bare_disc_and_track_tokens`
  - `padding_width_zero_emits_bare_placeholder`
  - `padding_leaves_similar_but_distinct_tokens_alone` (e.g. `{track_total}`)
  - `padding_auto_mode_derives_width_from_track_total`
  - `padding_fixed_modes_ignore_track_total`
  - `padding_disc_auto_mode_stays_unpadded_for_small_sets`

  ### Out of scope

  - Settings UI for the new controls (radio buttons / dropdowns). Backend
    infrastructure ships first; UI follows in a separate PR so this one
    stays reviewable.
  - `Auto` mode reading real album metadata at enrichment time — requires
    plumbing that isn't strictly necessary for the user's immediate ask
    (fixed widths already solve the box-set problem).
  - Settings migration bumping the default template to use `{track:03d}`
    — left as a future micro-PR if it turns out users don't discover
    the new setting.

  ### Verified locally

  - cargo clippy -- -D warnings clean (one narrowly-scoped allow on
    merge_options itself for the field_reassign_with_default lint —
    rewriting a 50-field builder function as a struct literal would
    destroy its readability).
  - cargo test --lib = 833 passed, 0 failed (8 new + 825 existing).


### 🐛 Bug Fixes

- **(errors)** Classify GAMDL playlist-title KeyError with actionable guidance (#588)

Apple Music Classical cross-work playlists hit a GAMDL upstream
  bug (#547 scenario 4 repro, 2026-04-23) where the playlist template
  renderer unconditionally dereferences `kwargs["title"]` even when
  the track's catalog entry lacks a `name` attribute, raising
  `KeyError: 'title'` on every affected track. The error cascades
  through GAMDL's async framework and lands in MeedyaDL's stderr
  buffer as a Python traceback. Pre-#588 it was mis-classified as
  `"unknown"` and the user saw only a generic "check the log" toast.

  ### Fix

  New classifier branch in `utils::process::classify_error`:
  `is_playlist_title_keyerror(error_message)` matches the exact
  signature (both the `KeyError: 'title'` string AND a GAMDL
  playlist-renderer frame like `get_playlist_file_path` or
  `downloader_base`) so unrelated `KeyError: 'title'` failures
  don't false-positive as playlist bugs.

  New `error_guidance` arm `"playlist_title_keyerror"` emits a
  user-friendly message with:
  - Specific framing ("this is a known upstream GAMDL limitation
    with certain Apple Music Classical playlists").
  - Actionable workaround ("try downloading the individual albums
    instead").
  - Upstream escalation link (https://github.com/glomatico/gamdl/issues).

  ### Out of scope

  - Fixing the upstream bug in GAMDL itself.
  - Auto-retry or fallback to per-track downloads when the classifier
    fires (separate follow-up if desired).
  - MusicBrainz / Discogs playlist-title resolution as a workaround.

  ### Tests

  Three new unit tests in `utils::process`:
  - `classifies_gamdl_playlist_title_keyerror` — canonical traceback
    routes correctly.
  - `classifies_unrelated_keyerror_title_as_unknown` — regression
    canary: a `KeyError: 'title'` without a playlist-renderer frame
    stays in "unknown".
  - `playlist_keyerror_guidance_points_users_upstream` — validates
    the user-visible message.

  ### Verified locally

  - cargo clippy -- -D warnings clean
  - cargo test --lib utils::process = 68 passed (3 new + 65 existing)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- **(naming)** Multi-disc + padding interaction unit tests (#589)

Adds seven unit tests covering the interaction between multi-disc
  filename templates and the new #587 padding settings. Closes #589's
  test-tracker ask now that the #587 infrastructure is in place.

  Scenarios covered:

  1. **Typical 2-disc album with Auto** — baseline: unpadded disc,
     2-digit track.
  2. **10-disc box set with Auto** — disc count ≥ 10 forces
     2-digit disc padding so `10-01` sorts after `9-01` correctly.
     The originating case from the #587 discussion.
  3. **Deep classical box set (200 discs × 120 tracks each)** —
     pathological Brilliant-Classics-style case. Auto correctly
     produces 3-digit disc AND 3-digit track.
  4. **User mixes fixed ThreeDigits track + Auto disc** — settings
     can be independently configured; small-disc album with fixed
     3-digit track produces `{disc}-{track:03d}`.
  5. **User explicit `{disc:02d}` spec takes precedence** — regression
     canary: user's explicit format always wins over the setting.
  6. **Direct song URL (no album metadata)** — `None` passed to
     `resolve_width` triggers Auto's safe default (2-digit track,
     unpadded disc), matching pre-#587 behaviour.
  7. **Compilation folder template with `{album_id}`** — padding
     applies to tracks only; `{album_id}` and other non-track
     placeholders are untouched.

  ### Verified locally

  - cargo test --lib services::download_queue::tests::multidisc = 7 passed
  - cargo test --lib (full) = 843 passed, 0 failed (7 new + 836 existing)


## [0.43.0] - 2026-04-23

### ✨ Features

- **(ux)** Add "Open folder" button alongside Browse in Diagnostics (#581)

Settings → Advanced → Diagnostics → On-disk activity log location now
  has three buttons: Browse… (change the folder), Open folder (reveal
  in OS file viewer), Reset (revert to default).

  The Open folder button reuses the existing `get_logs_folder_path` IPC
  command + `@tauri-apps/plugin-shell`'s `open()` — the same pattern
  that powers the Activity Log page's "Reveal" button (line 297 of
  `src/components/download/ActivityLog.tsx`). No new Rust code needed;
  behaviour is consistent across the two entry points.

  Failures surface via a toast (`Failed to open logs folder: {error}`)
  using the existing `useUiStore.addToast()`. Same failure-handling
  shape as the Activity Log page.

  No backend changes. No new dependencies. One new import (`getLogsFolderPath`),
  one new button, one toast import.

  Verified locally: tsc --noEmit clean; npx vitest run = 303 tests.

- **(ux)** Add "Open folder" button alongside Browse in Diagnostics (#581) (#594)

## Summary

  Closes **#581**. Settings → Advanced → Diagnostics → *On-disk activity
  log location* gets a new **Open folder** button between the existing
  **Browse…** and **Reset** buttons.

  ## Reviewer ask

  > *"In Settings > Advanced > Diagnostics > On-Disk Activity Log
  location, add a button to open the on-disk activity log location in the
  OS set/native directory viewer. We have a Browse button to set the
  folder, but we also should have a simple way to open the folder for
  quick access to logs."*

  ## Implementation

  Reuses the existing `get_logs_folder_path` IPC command +
  `@tauri-apps/plugin-shell`'s `open()` — the identical pattern that
  powers the Activity Log page's "Reveal" button (see
  `src/components/download/ActivityLog.tsx:297`). No new Rust code;
  behaviour is consistent across the two entry points, which was the
  express ask.

  ```tsx
  <Button
    variant="secondary"
    size="sm"
    onClick={async () => {
      const addToast = useUiStore.getState().addToast;
      try {
        const path = await getLogsFolderPath();
        const { open } = await import('@tauri-apps/plugin-shell');
        await open(path);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        addToast(`Failed to open logs folder: ${msg}`, 'error');
      }
    }}
  >
    Open folder
  </Button>
  ```

  Failure path: errors surface as a toast via `useUiStore.addToast` with
  the same failure-handling shape as the Activity Log page. Silent success
  (the OS file viewer popping open).

  ## Placement

  Button order (left to right): **Browse…** / **Open folder** / **Reset**.
  Matches natural action order — change the folder (Browse), visit the
  folder (Open), reset back (Reset).

  ## Diff summary

  Single file changed (`src/components/settings/tabs/AdvancedTab.tsx`),
  +29 / -0:

  - 1 new import: `getLogsFolderPath` (already a public IPC).
  - 1 new import: `useUiStore` (for the error-case toast).
  - 1 new `<Button>` (Open folder).

  No backend changes. No new dependencies. No schema changes.

  ## Acceptance criteria from #581

  - [x] **Open folder** button added between Browse and Reset.
  - [x] Reuses the existing `get_logs_folder_path` IPC command.
  - [x] Reuses `@tauri-apps/plugin-shell`'s `open()`.
  - [x] Placement: Browse / Open / Reset.
  - [x] Consistent with the Activity Log page's "Reveal" button pattern.
  - [x] Honours the `activity_log_path_override` setting
  (`get_logs_folder_path` already does).
  - [x] Error toast on failure.

  Labelling choice: "Open folder" rather than "Reveal" — the Activity Log
  page uses "Reveal" to match macOS-native Finder terminology, but in a
  Settings panel "Open folder" is more immediately understandable to
  non-Mac users. This is a minor label divergence; if consistency
  preference flips the other way, trivial rename either place.

  ## Local verification

  ```
  tsc --noEmit                  ✓ clean
  npx vitest run                303 tests passed, 0 failed (no new tests; UI-only change)
  ```

  ## Test plan (post-merge)

  - [ ] Open Settings → Advanced → Diagnostics. The On-disk activity log
  location row shows three buttons in order.
  - [ ] Click **Open folder** with default path (blank override). System
  file viewer opens at the default app data logs directory.
  - [ ] Set a custom path via **Browse…**, click **Open folder**. File
  viewer opens at the custom path.
  - [ ] Set an invalid path (e.g. a removed external drive), click **Open
  folder**. Toast shows "Failed to open logs folder: ..." with a clear
  error.
  - [ ] Cross-platform: macOS Finder, Windows Explorer, Linux Nautilus /
  Files — all open the folder.

  ## Related

  - **#581** — closed by this PR.
  - **CLAUDE.md** — Activity Log section documents the
  `get_logs_folder_path` + `plugin-shell` pattern this reuses.


### 🐛 Bug Fixes

- **(ux)** Replace 'Pre-flight checks passed' with plain-English activity-log messages (#578)

User feedback on 2026-04-23: the phrase was jargon-first — "even I (as
  a dev) thought something was broke when the progress bars disappeared.
  For a moment I didn't realise this vague statement meant ALL actions
  completed successfully."

  ### Changes to `services/download_queue.rs` pre-flight block

  Three string updates:

  1. **Before the checks run**: new activity-log emit announcing what's
     about to happen — "Checking internet connection, output folder,
     and account..." So users see the app pause, know what it's doing,
     then get a matching answer.

  2. **All-clear message**: "Pre-flight checks passed" → "Ready to
     download — internet, output folder, and account all verified".
     Leads with the user-facing action (what happens next), enumerates
     the verified prerequisites in plain English, no aviation jargon.

  3. **Warning message**: "Pre-flight: {message}" → "Pre-flight warning:
     {message}". Keeps the prefix (the word "warning" gives more context
     than the technical phrase alone) but makes it explicit that this is
     a problem the user should act on.

  ### Not changed

  - The RUST log::warn! in the warning path keeps "Pre-flight warning"
    as it's aimed at log-file inspection (dev-facing), not the
    activity-log UI.
  - The existing `preflight-warning` / `preflight-cleared` event names
    are left alone — they're internal wire-protocol identifiers the
    frontend listens for.
  - Out of scope: broader audit of every [System] activity-log string
    (the issue mentions this as a follow-up; scope kept tight to the
    specific symptom reported).

  ### Verified locally

  - cargo clippy -- -D warnings clean
  - cargo test --lib services::download_queue::tests = 120 passed

- **(ux)** Replace "Pre-flight checks passed" jargon with plain-English activity log (#578) (#593)

## Summary

  Closes **#578**. Replaces the jargon-first `[System] Pre-flight checks
  passed` activity-log message with plain-English before/after pair so
  users know what was verified and what happens next.

  ## Reviewer feedback

  > *"It needs to be something meaningful to the end user! This is partly
  why even I (as a dev) thought something was broken when the progress
  bars disappeared. For a moment I didn't realise this vague statement
  meant ALL actions completed successfully."*

  ## Changes

  Three string tweaks in the pre-flight block of
  `services/download_queue.rs`:

  ### 1. New "before" message

  Emitted at the moment pre-flight checks begin, so the activity log has a
  clear question-and-answer shape:

  ```
  [System] Checking internet connection, output folder, and account...
  ```

  ### 2. "All-clear" message

  ```diff
  - [System] Pre-flight checks passed
  + [System] Ready to download — internet, output folder, and account all verified
  ```

  Leads with the user-facing action ("Ready to download"), enumerates the
  verified prerequisites in plain English, no aviation jargon.

  ### 3. Warning message prefix

  ```diff
  - [System] Pre-flight: {message}
  + [System] Pre-flight warning: {message}
  ```

  Kept the "Pre-flight" prefix because adding "warning" makes it explicit
  that this is a problem the user should act on; removing the prefix
  entirely would drop useful context. (The backend `preflight-warning`
  event that drives the user-visible toast is also left alone — it's a
  wire-protocol identifier, not a UI string.)

  ## Not in scope

  - Broader audit of every `[System]` activity-log string for similar
  jargon (the issue body mentions this as a follow-up). Done separately if
  the pattern recurs.
  - The Rust `log::warn!` line keeps the old phrasing — that's aimed at
  log-file inspection for support / dev debugging, not the user-facing
  activity log.

  ## Before / after (typical happy path)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.42.3] - 2026-04-23

### 🐛 Bug Fixes

- **(enrichment)** Skip macOS AppleDouble + known filesystem sidecars in audio walkers (#577)

When the user's output path is on a non-native filesystem (exFAT /
  FAT32 / HFS on external drives, SMB / NFS shares), macOS creates
  AppleDouble `._*` sidecar files alongside every real file to store
  resource-fork metadata the underlying filesystem can't natively
  represent. Similar sidecars exist on other platforms: `.DS_Store`,
  `Thumbs.db` / `thumbs.db`, `desktop.ini`.

  Every enrichment walker that iterates audio extensions was processing
  these sidecars too — running ffprobe / MediaInfo / Chromaprint /
  FFmpeg loudness analysis / mp4ameta on non-audio binaries, failing
  noisily, and contributing hundreds of spurious warning lines to the
  activity log on a large album. Captured live 2026-04-23 on a 200-track
  Beethoven box set download to an exFAT USB drive.

  ### Fix

  New shared predicate `utils::fs_safe::is_filesystem_sidecar(path)`
  returning `true` for:
    - `._*` prefix (macOS AppleDouble — the dominant case)
    - `.DS_Store` (macOS Finder metadata)
    - `Thumbs.db` / `thumbs.db` (Windows thumbnail cache)
    - `desktop.ini` (Windows folder customisation)

  Applied as a guard at every enrichment walker call site:

    - `download_queue::has_direct_audio_files` — non-recursive check;
      now ignores sidecars when deciding if a dir has "real" content.
      Via inheritance, `find_deepest_audio_dir` gets the same filter.
    - `download_queue::count_audio_files_in_directory` — recursive
      counter used by the completion-task timeout (#579).
    - `download_queue::count_media_files` — audio-vs-video split
      counter.
    - `acoustid_service::collect_m4a_recursive` — Chromaprint
      fingerprinting walker.
    - `replaygain_service::collect_audio_recursive` — FFmpeg loudness
      walker.
    - `metadata_tag_service::tag_directory_recursive` — mp4ameta atom
      writer.
    - `metadata_tag_service::collect_m4a_depth_limited` — codec
      detection + API-tag injection walker (#452).

  Seven new unit tests on the predicate lock in the positive cases
  (`._track.m4a`, `.DS_Store`, `Thumbs.db`, `thumbs.db`, `desktop.ini`)
  and the regression canaries (real audio files, files with legal
  leading dots/underscores not misclassified, paths without basenames
  return false).

  ### Observable result

  On a 100-track album download to an exFAT USB drive, the activity
  log previously emitted ~100 `ffprobe failed for ._N - Title.m4a` +
  MediaInfo fallback warnings during codec detection. After this fix:
  zero. Bonus: faster enrichment (no wasted ffprobe subprocess spawns)
  and less activity-log burst pressure (which complements the #575
  virtualiser keying fix).

  ### Verified locally

  - cargo clippy -- -D warnings  ✓ clean
  - cargo test --lib             825 passed, 0 failed (7 new sidecar
    tests + inheritance through every existing walker test)

- **(progress-bar)** Emit queue-updated event on enrichment label changes (#574)

The per-item progress bar kept showing `DOWNLOADING...Artist — Album — Track` for
  the entire enrichment phase of a download, even though the backend was correctly
  updating `processing_label` at every enrichment stage start (metadata, lyrics,
  artwork, AcoustID, ReplayGain) via `set_processing_label()`.

  ### Root cause

  The frontend's `refreshQueue()` in App.tsx is only wired to lifecycle events —
  `download-complete`, `download-error`, `download-queued`, `download-cancelled`.
  There is no periodic poll of `get_queue_status()`. During enrichment, the backend
  mutates `processing_label` on the queue item, but no lifecycle event fires, so
  the frontend never re-fetches. The progress-bar caption stays locked on the
  last known state (the final "DOWNLOADING..." from the primary GAMDL download)
  until `download-complete` finally fires at the very end.

  `GlobalProgressBar.tsx` already correctly reads `activeItem.processing_label`
  and prioritises it over the download caption (line 278) — the labels just
  never reach it.

  ### Fix

  Two changes, single commit:

  1. **Backend** (`services/download_queue.rs`): the `set_label` closure in the
     enrichment task now emits a `queue-updated` event after mutating the label.
     Stage transitions are low-frequency (typically <15 per download); no
     throttling needed.

  2. **Frontend** (`App.tsx`): new `queue-updated` event listener that calls
     `refreshQueue()`. The listener is registered alongside the existing
     `download-queued` listener (similar pattern, same cleanup path).

  ### Observable result

  During the enrichment phase of a download, the progress-bar caption now cycles
  through the existing labels in real time:
    - "Enriching metadata tags..."
    - "Fetching word-level lyrics..."
    - "Converting lyrics (Enhanced LRC)..."
    - "Downloading animated artwork..."
    - "AcoustID fingerprinting..."
    - "ReplayGain loudness analysis..."

  Previously these were all invisible — the caption stayed on
  "DOWNLOADING...{last track}" until `download-complete` fired.

  ### Scope

  This PR fixes only #574 (visible caption stagnation). **#576** (queue-level
  partial progress — showing >0% while in Processing state) needs a separate
  architectural change: a new `processing_progress: Option<f32>` field on
  `QueueItemStatus`, new emit sites per stage, and a rewrite of
  `GlobalProgressBar.tsx`'s queue-level aggregation to use it. Deferred to a
  follow-up PR to keep this change tight.

  ### Verified locally

  - tsc --noEmit clean
  - npx vitest run = 303 tests passed across 19 files
  - cargo clippy -- -D warnings clean

- **(progress-bar)** Emit queue-updated event so enrichment labels reach the UI (#574) (#590)

## Summary

  Closes **#574** — the per-item progress bar caption stagnating on
  `DOWNLOADING...Artist — Album — Track` for the entire enrichment phase,
  even though the backend was updating `processing_label` at every stage.

  **Not** closing **#576** (queue-level partial progress during
  enrichment) — that needs a larger architectural change I want to keep in
  its own PR.

  ## Root cause

  `GlobalProgressBar.tsx` already reads `activeItem.processing_label`
  correctly and prioritises it over the download caption (line 278). The 5
  existing enrichment stages (metadata, word-lyrics, lyrics-conversion,
  animated-artwork, AcoustID, ReplayGain) all call
  `set_processing_label()` with human-readable strings.

  **The labels weren't reaching the frontend.** `App.tsx`'s
  `refreshQueue()` is only triggered by four lifecycle events
  (`download-complete`, `download-error`, `download-queued`,
  `download-cancelled`) — there's **no periodic poll** of
  `get_queue_status()`. During enrichment, `set_processing_label()`
  mutates backend queue state but emits no event, so the frontend never
  re-fetches. The caption stays frozen on the last known state (the final
  `DOWNLOADING...{last track}` from the primary GAMDL download) until
  `download-complete` finally fires at the very end.

  Captured live 2026-04-23 on a 200-track Beethoven box set, where
  enrichment takes 20+ minutes and the user sees `DOWNLOADING...Piano
  Sonata No. 32 in C Minor, Op. 111...` frozen for the entire time.

  ## Fix

  Two surgical changes, one commit:

  ### Backend — `src-tauri/src/services/download_queue.rs`

  The `set_label` closure in the enrichment task now emits a
  `queue-updated` event after mutating the label:

  ```rust
  let label_app = enrich_app.clone();
  let set_label = move |label: &str| {
      if let Ok(mut q) = label_queue.try_lock() {
          // ...existing label mutation...
          q.set_processing_label(&label_dl_id, &full_label);
      }
      let _ = label_app.emit("queue-updated", &label_dl_id);
  };
  ```

  Stage transitions are low-frequency (~5–15 per download, driven by
  human-observable enrichment boundaries). No throttling needed.

  ### Frontend — `src/App.tsx`

  New `queue-updated` event listener alongside the existing four lifecycle
  listeners:

  ```typescript
  unlistenQueueUpdated = await listen('queue-updated', () => {
    try {
      refreshQueue();
    } catch (err) {
      console.error('Error in queue-updated handler:', err);
    }
  });
  ```

  Declared, cleaned up, and registered using the exact same pattern as the
  four existing unlisteners. Zero structural change to the listener setup.

  ## Observable result

  During enrichment, the progress-bar caption now cycles through the
  existing labels in real time:

  - "Enriching metadata tags..."
  - "Fetching word-level lyrics..."
  - "Converting lyrics (Enhanced LRC)..."
  - "Downloading animated artwork..."
  - "AcoustID fingerprinting..."
  - "ReplayGain loudness analysis..."

  Previously these were all invisible — the caption stayed on
  `DOWNLOADING...{last track}` until `download-complete` fired 10–20
  minutes later on large albums.

  ## Out of scope (deferred to follow-up)

  - **#576 queue-level partial progress** — the "0 of 1 complete, 0%"
  screen caption from the same #547 repro. Needs: new
  `processing_progress: Option<f32>` field on `QueueItemStatus`, per-stage
  progress emit sites, rewrite of `GlobalProgressBar.tsx`'s queue-level
  aggregation formula. Deferred to a follow-up PR because it changes data
  model + a non-trivial fraction of the progress-bar rendering logic.
  - **Labels for the 10+ enrichment stages that currently don't call
  `set_label`** (lyrics fallback, WebVTT, Rich SRT, ASS, subtitle embed,
  artist promo, BPM, MV companion discovery, MusicBrainz, advisory rename,
  cover rename, manifest write). Each would be a one-line addition but
  proliferation of call sites is best done alongside the #576 work where
  the stage taxonomy gets consolidated into a single source of truth.
  - **"ENRICHMENT..." prefix** proposed in #574 body. Current labels are
  already self-descriptive (`"ReplayGain loudness analysis..."`);
  prefixing would add noise. If desired, trivial follow-up — prepend in
  `GlobalProgressBar.tsx` when `activeItem.state === 'processing'`.

  ## Local verification

  ```
  tsc --noEmit                 ✓ clean
  npx vitest run (all files)   303 tests passed across 19 files
  cargo clippy -- -D warnings  ✓ clean
  ```

  ## Risk

  Low. Backend change is an additive `app.emit()` call inside an existing
  closure; errors non-fatal (swallowed with `let _ =`). Frontend change is
  an additive listener following the existing four-listener pattern
  exactly. No data model changes, no migration, no behavioural change
  beyond the intended one.

  ## Test plan (post-merge)

  - [ ] Download a typical album. During enrichment phase, progress-bar
  caption should transition through the stage labels ("Enriching metadata
  tags..." → "AcoustID fingerprinting..." → "ReplayGain loudness
  analysis..."). No more frozen-on-DOWNLOADING.
  - [ ] Download a large box set (e.g. #547's 100-track Beethoven). Over
  ~15 minutes of enrichment, the caption visibly changes at each stage
  boundary.
  - [ ] Cancel a download mid-enrichment. No spurious events, no memory
  leaks (listener cleaned up on unmount via existing pattern).
  - [ ] Queue with multiple items. Each item's enrichment updates only its
  own caption; queue progress-bar unaffected (that's #576 territory).

  ## Related

  - **#574** — this PR closes it.
  - **#576** — separate follow-up for queue-level partial progress.
  - **#582** — recently-shipped empty-output guard + timeout scaling; also
  lives in the completion-task area.
  - **CLAUDE.md** — "Global progress bars" section documents the
  `processing_label` infrastructure this PR finally makes visible.

- **(enrichment)** Skip macOS AppleDouble + known filesystem sidecars in audio walkers (#577) (#591)

## Summary

  Closes **#577**. Every enrichment walker that iterates audio-file
  extensions now skips filesystem sidecars: macOS `._*` AppleDouble files,
  `.DS_Store`, Windows `Thumbs.db` / `desktop.ini`. Captured live
  2026-04-23 on a 200-track Beethoven box set download to an exFAT USB
  drive — ~100 spurious `ffprobe failed for ._N - Title.m4a` warnings per
  album, now zero.

  ## Why this happens

  When the user's output path is on a non-native filesystem (exFAT / FAT32
  / HFS on external drives, SMB / NFS shares), macOS automatically creates
  **AppleDouble** `._{filename}` sidecar files alongside every real file.
  These sidecars hold resource-fork metadata (extended attributes, Finder
  tags, `com.apple.quarantine` flags) that the underlying filesystem can't
  natively represent. They share the real file's extension (`._track.m4a`
  sits next to `track.m4a`) but contain binary metadata, not audio.

  Every enrichment walker that filters by extension (`.m4a` / `.mp4` /
  `.m4v` / `.flac` / `.mp3`) was processing these sidecars:

  - `ffprobe` / MediaInfo tried to detect codec on them → failed noisily →
  fallback codec used.
  - Chromaprint tried to fingerprint them → failed.
  - FFmpeg loudness analysis (ReplayGain) tried to analyse them → failed.
  - `mp4ameta` tried to write atoms to them → failed.

  On a 100-track album, that's ~500 spurious errors across the enrichment
  stages, cluttering the activity log and wasting CPU on subprocess
  spawns.

  Same class of sidecar exists on Windows (`Thumbs.db`, `desktop.ini`) and
  even on macOS itself (`.DS_Store` for Finder metadata).

  ## Fix — one predicate, seven call sites

  ### New shared predicate


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.42.2] - 2026-04-23

### 🐛 Bug Fixes

- **(parser)** Accept /recording/ URLs as submittable (revert #573 rejection UX)

#573 classified Classical recording URLs as `recording` content type
  and set `isValid: false` on the grounds that GAMDL's URL vocabulary
  doesn't include `/recording/` paths, so passing them through would
  hit the misleading-success cascade documented in #567/#548 (primary
  download produces zero files → lyrics companion pipeline runs anyway
  → activity log reports fake success).

  Reviewer feedback on 2026-04-23:

    "why are we showing a notice/error for Apple Music Classical
    /recording/ URLs if theyre valid? why not just accept them?
    Asking the user to enter another is not user friendly!"

  Fair call. Two things changed since #573 that make pre-emptive
  rejection the wrong UX:

  1. PR #582 broadened #567's guard from "skip lyrics companion when
     primary produced no audio" to "skip the ENTIRE enrichment pipeline
     when primary produced zero output files". So if GAMDL rejects a
     recording URL, the user now sees ONE clean "Enrichment skipped —
     primary download produced no output files" activity-log line
     instead of 5+ cascading fake successes.

  2. Recording URLs ARE Apple Music Classical URLs — the user's mental
     model is that if they copied the link from the Apple Music
     Classical app, it should work. Forcing them to navigate back to
     the app and find the containing album URL is paternalistic.

- **(parser)** Accept Apple Music Classical `/recording/` URLs as submittable (revert #573 rejection UX) (#583)

## Summary

  Reverses the reject-at-validator UX from #573 for Apple Music Classical
  `/recording/` URLs. Recording URLs are now treated like any other
  recognised Apple Music URL — the user can paste and submit, GAMDL
  attempts the download, the pipeline gives a clean outcome.

  ## Reviewer feedback that drove this change

  > *"why are we showing a notice/error for Apple Music Classical
  /recording/ URLs if theyre valid? why not just accept them? Asking the
  user to enter another is not user friendly!"*

  Fair call. Two things changed since #573 that make pre-emptive rejection
  the wrong UX:

  1. **PR #582** (currently in CI) broadened #567's guard from *"skip
  lyrics companion when primary produced no audio"* to *"skip the ENTIRE
  enrichment pipeline when primary produced zero output files"*. So if
  GAMDL rejects a recording URL, the user now sees **one** clean
  `Enrichment skipped — primary download produced no output files`
  activity-log line instead of 5+ cascading fake "success" lines.
  2. **Recording URLs ARE Apple Music Classical URLs** — the user's mental
  model is that if they copied the link from the Apple Music Classical
  app, it should work. Forcing them to navigate back to the app and find
  the containing album URL is paternalistic UX.

  ## Changes

  ### `src/lib/url-parser.ts`

  `parseAppleMusicUrl()`: removes the `contentType !== 'recording'`
  exclusion from the `isValid` calculation. Recording URLs now return
  `isValid: true`. The docstring block explaining the old rationale is
  replaced with the new one.

  **Before** (merged in #573):
  ```ts
  const isSubmittable = contentType !== 'unknown' && contentType !== 'recording';
  return { url: trimmed, contentType, isValid: isSubmittable };
  ```

- **(activity-log)** Stable key + stable measureElement to prevent row overlap (#575)

The activity-log virtualiser renders overlapping text during dense
  log bursts — captured live 2026-04-23 on a 200-track box set
  download to an external USB volume, hundreds of ffprobe / MediaInfo
  verbose lines per second. Class of bug is the same as #442 (closed
  2026-04-12 by adding `measureElement`), but #442's fix was
  incomplete — the regression resurfaces under real workloads.

  ### Root cause (this PR fixes it)

  Two related gaps in the `useVirtualizer` config:

  1. **No `getItemKey` option** — TanStack virtual defaults to keying
     its measurement cache by positional index. Any event that shifts
     positions — the 10,000-entry trimming cap firing, filter toggles
     changing `filteredEntries.length`, RAF-batched bursts inserting
     new entries — causes cached row heights to attach to the wrong
     entries. Rows then get laid out at `translateY(start)` values
     computed from mis-keyed heights, producing the visual overlap.
  2. **Inline `measureElement` closure** — re-created on every render.
     At ~60 flushes/sec from App.tsx's RAF batching, that's a lot of
     reference thrash in the virtualiser's internal config sync.

  ### Fix

  - **`getItemKey`**: wrap in `useCallback`, return `filteredEntries[index]?._id`
    with a fallback to the positional index. `_id` is the auto-
    incrementing ID set by `activityStore.addEntries()` (per CLAUDE.md),
    so it's stable across filter toggles, trim cycles, and burst inserts.
  - **`measureElement`**: extract from the config object and wrap in
    `useCallback` with an empty dep list. One stable function for the
    component's lifetime.

  No other changes — the `estimateSize`, `overscan`, row rendering
  (JSX, refs, CSS), filter logic, and RAF batching are all left alone.

  Verified locally: tsc --noEmit clean; npx vitest run = 303 tests
  passed across 19 files.

- **(activity-log)** Stable key + stable measureElement to prevent row overlap (#575) (#585)

## Summary

  Closes **#575**. Fixes the activity-log overlapping-text regression
  captured live 2026-04-23 during the Beethoven box-set download. The
  virtualiser was laying out rows at stale `translateY()` offsets during
  burst log ingestion, producing visual overlap between consecutive
  entries whose wrapped heights didn't match what the cache thought they
  should be.

  #442 (closed 2026-04-12) was the original fix for the same symptom — it
  added `measureElement` so that dynamic row heights were actually
  measured. That fix was necessary but not sufficient; under real
  workloads the bug recurs because TanStack virtual's measurement cache is
  keyed by **index** by default, and indices shift whenever the filtered
  entry list changes.

  ## Root cause analysis

  Two related gaps in the `useVirtualizer` config in
  `src/components/download/ActivityLog.tsx`:

  ### Gap 1 — no `getItemKey`

  Without `getItemKey`, TanStack virtual uses the positional index as the
  cache key. Every time the entry list changes position:

  - **10 000-entry trimming cap** (per CLAUDE.md) — oldest entries drop,
  remaining entries shift down by N.
  - **Filter toggles** (System / Download / Verbose) —
  `filteredEntries.length` changes; a row that was index 47 in the
  all-visible set is index 12 in the filtered set.
  - **RAF-batched bursts** — App.tsx feeds entries in batches at ~60
  flushes/sec; during a dense burst (200+ entries/sec from MediaInfo codec
  detection on a 200-track album) the virtualiser sees rapid `count`
  deltas.

  Every shift invalidates the index→height mapping. Cached heights for
  entry-at-index-47 get applied to the new-entry-at-index-47, which has
  completely different content (and therefore different wrapped height).
  `translateY(row.start)` for subsequent rows computes using stale
  heights, and rows overlap.

  ### Gap 2 — inline `measureElement`

  ```typescript
  // Before:
  const virtualizer = useVirtualizer({
    ...
    measureElement: (element) => element?.getBoundingClientRect().height ?? 26,
  });
  ```

  Inline arrow function — re-created on every render. At 60 flushes/sec,
  that's 60 new function references per second, each one syncing into the
  virtualiser's internal config. TanStack's behaviour when the
  `measureElement` reference changes is to re-walk its cache; in a tight
  burst scenario this was observed to interact with the index-based keying
  to produce worse overlap than either issue alone.

  ## Fix

  - **`getItemKey`** added, wrapped in `useCallback`. Keys by
  `filteredEntries[index]?._id` (the stable auto-incrementing ID set by
  `activityStore.addEntries()` per CLAUDE.md), falls back to index when
  `_id` is absent (defensive; shouldn't trigger in normal flow).
  - **`measureElement`** extracted from the inline config and wrapped in
  `useCallback([])`. One stable function for the component's lifetime.

  Full change is **45 lines added, 1 removed, single file**:

  ```typescript
  const measureElement = useCallback(
    (element: Element | null | undefined) =>
      element?.getBoundingClientRect().height ?? 26,
    [],
  );

  const getItemKey = useCallback(
    (index: number) => filteredEntries[index]?._id ?? index,
    [filteredEntries],
  );

  const virtualizer = useVirtualizer({
    count: filteredEntries.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 26,
    overscan: 50,
    measureElement,
    getItemKey,
  });
  ```

  No other changes. `estimateSize`, `overscan`, JSX row rendering (still
  uses `ref={virtualizer.measureElement}`), CSS classes, filter logic, and
  App.tsx's RAF batching are all left alone.

  ## Why this completes #442's intended fix

  #442 added the missing `measureElement` option. That made the
  virtualiser *capable* of measuring dynamic heights, but the measurements
  were then stored against volatile keys. The fix works when the entry
  list is static, but fails under the exact workloads MeedyaDL encounters
  in practice — box-set downloads, external USB filesystems, MediaInfo
  verbose streams. This PR is the belt-and-braces completion: dynamic
  measurements stored against stable keys.

  ## Local verification

  ```
  tsc --noEmit                 ✓ clean
  npx vitest run (all files)   303 tests passed across 19 files
  ```

  ## Post-merge test plan

  Reproducing the original #575 observation requires the exact conditions
  captured 2026-04-23:

  - Output path on an external exFAT / FAT32 / HFS USB drive (forces macOS
  to create `._*` AppleDouble sidecars, which inflate the ffprobe-failure
  log volume and trigger the rapid-burst path).
  - Verbose activity log enabled.
  - 100+ track album download (forces MediaInfo codec detection to iterate
  many files rapidly).
  - Watch Activity Log during enrichment phase — no row overlap should
  occur.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.42.1] - 2026-04-23

### 🐛 Bug Fixes

- **(enrichment)** Skip all enrichments on empty output + scale timeout by track count (#567 #579)

Two related completion-task fixes that share infrastructure in
  `services/download_queue.rs`:

- **(enrichment)** Skip all enrichments on empty output + scale completion timeout by track count (#567 #579) (#582)

## Summary

  Two related completion-task fixes, one PR because they live in the same
  ~100-line region of `services/download_queue.rs`:

  - **Closes #567** (broadened): skip **all** post-GAMDL enrichment stages
  when the primary download produced zero output files.
  - **Closes #579**: scale the completion-task timeout by output track
  count so large box sets don't hit the fixed 10-minute deadline
  mid-ReplayGain.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.42.0] - 2026-04-23

### ✨ Features

- **(parser)** Recognise Apple Music Classical `/recording/` URLs with helpful error

Apple Music Classical's 2026 rollout introduced a new content type path
  segment, `/recording/`, which identifies a specific performance of a
  classical work (distinct from the album release that contains it).
  Example URL shape from the Apple Music Classical app Share → Copy Link:

    https://classical.music.apple.com/gb/recording/
      gustav-mahler-1860-pp1-1452377808?l=en-GB

  Previous behaviour: frontend `detectContentType()` did not recognise
  `/recording/`, returned `'unknown'`, and the UI showed the generic red
  error "Please enter a valid Apple Music URL" — no actionable guidance.

  New behaviour: recording URLs are classified as a new content type,
  `recording`. `parseAppleMusicUrl()` marks them `isValid: false` (since
  GAMDL's URL vocabulary doesn't include `/recording/` paths, attempting
  to download would hit the misleading-success bug documented in #567 /
  #568). The DownloadForm shows a specific actionable message:

    "Apple Music Classical *recording* URLs aren't supported yet. Open
     the recording in Apple Music Classical, then use **Go to Album**
     and share that URL instead."

  Also adds `Classical Recording` label for display in the content-type
  badge UI elements (`CONTENT_TYPE_LABELS`, `CONTENT_TYPE_ICONS`,
  `getContentTypeLabel`).

- **(parser)** Recognise Apple Music Classical `/recording/` URLs with helpful error message (#573)

## Summary

  Apple Music Classical's 2026 rollout introduced a new content type,
  `/recording/`, which represents a specific performance of a classical
  work (distinct from the album release that contains it). User
  encountered it whilst collecting #547 audit data on 2026-04-23 — the URL
  they pasted was:

  ```
  https://classical.music.apple.com/gb/recording/gustav-mahler-1860-pp1-1452377808?l=en-GB
  ```

  **Current behaviour on `main`**: frontend `detectContentType()` doesn't
  recognise `/recording/`, returns `'unknown'`, and the UI shows the
  generic red error *"Please enter a valid Apple Music URL"* — no
  actionable guidance.

  **New behaviour with this PR**: recording URLs are classified as a new
  content type, `recording`. `parseAppleMusicUrl()` marks them `isValid:
  false` so the submit button stays disabled. The DownloadForm shows a
  specific actionable message:

  > *"Apple Music Classical **recording** URLs aren't supported yet. Open
  the recording in Apple Music Classical, then use **Go to Album** and
  share that URL instead."*

  ## Why not pass to GAMDL and let it try?

  GAMDL's URL regex vocabulary does not include `/recording/`. Attempting
  to download would hit the exact misleading-success bug documented in
  #567 / #568: GAMDL emits *"Could not parse URL, skipping"*, exits 0,
  MeedyaDL's lyrics companion pipeline runs and claims success, but no
  files land on disk. Frontend rejection with a helpful message is the
  honest UX until we know how to handle recordings properly.

  If we ever add a proper `/recording/` download pipeline (e.g. by
  resolving recording → album via the Apple Music Catalog API and
  rewriting the URL), flipping `isValid: true` re-enables submission — the
  type system is already in place.

  ## Changes

  ### `src/types/index.ts`
  Add `recording` to the `AppleMusicContentType` union.

  ### `src/lib/url-parser.ts`
  - `detectContentType()`: recognise `/recording/` path segment and return
  `'recording'`.
  - `parseAppleMusicUrl()`: new `isSubmittable` guard that excludes
  `recording` (and still excludes `unknown`) from `isValid: true`.
  Detailed docstring explaining why.
  - `getContentTypeLabel()`: add `"Classical Recording"` label for the new
  type.

  ### `src/components/download/DownloadForm.tsx`
  - `CONTENT_TYPE_ICONS` / `CONTENT_TYPE_LABELS` registries: add entries
  for `recording`.
  - Validation feedback block: when `urlContentType === 'recording'`, show
  the specific actionable message; generic "Please enter a valid Apple
  Music URL" still fires for truly unrecognised URLs.

  ### `src/lib/url-parser.test.ts`
  Four new tests in a dedicated `parseAppleMusicUrl - classical recording
  URLs` describe block:

  1. `classifies recording URLs as 'recording' content type`
  2. `marks recording URLs as NOT submittable` (the critical guard)
  3. `classifies recording URL with locale query param` (real-world shape,
  `?l=en-GB`)
  4. `does not misclassify album URLs as recordings` (regression canary
  against detection-order mistakes)

  Plus an extended assertion on the existing `getContentTypeLabel`
  exhaustive test.

  ## Backend parser — intentionally NOT touched

  Recording URLs are blocked at the frontend now, so they never reach the
  Rust side during normal flow. If a recording URL bypasses the frontend
  (deep-link, drag-drop, manifest import), the existing `#549 catch-all
  WARN` in `start_download` will flag it. Adding a backend regex branch
  for `/recording/` would add complexity with no observed benefit —
  keeping the surface small.

  ## Local verification

  ```
  tsc --noEmit                   ✓ clean
  npx vitest run url-parser      49 tests passed (45 existing + 4 new)
  npx vitest run (all files)     303 tests passed across 19 files
  ```

  ## Risk

  Very low. Purely additive frontend classification:

  - Existing URL shapes still parse identically (unchanged detection
  order; `/recording/` check inserted *after* the other 6 path-type
  checks).
  - No backend changes.
  - New content type exclusion from `isValid` means one specific URL shape
  is *rejected* rather than *accepted* — strictly safer than current
  behaviour (which was also rejecting it, just with a worse message).

  ## Test plan (post-merge)

  - [ ] Paste the URL from the bug report — UI shows the specific "Go to
  Album" message instead of "Please enter a valid Apple Music URL".
  - [ ] Paste any `/album/` URL on `classical.music.apple.com` — still
  works (no regression).
  - [ ] Paste the recording URL with whitespace around it — still detected
  and rejected correctly (trim still runs first).
  - [ ] Unblocks #547 repro — users can now share-link to a recording, see
  the helpful message, tap "Go to Album" in the app, and get the album URL
  for the actual download.

  ## Out of scope

  - Downloading classical recordings. Separate future work; needs Apple
  Music Catalog API investigation
  (`/v1/catalog/{sf}/recordings/{id}?include=albums`?) to resolve
  recording → album before GAMDL handoff.
  - Other Classical-specific path segments (`/work/`, `/composer/`,
  `/ensemble/`, `/conductor/`). These haven't appeared in the wild yet; if
  they do, the #549 catch-all WARN will flag them and we can extend the
  same pattern.

  ## Related

  - **#547** — manual repro blocked on this URL shape being unparseable;
  clicking "Go to Album" per the new error message produces a URL that
  works.
  - **#567** — why we block at validator instead of passing through
  (misleading-success bug).
  - **#568** — similar rationale for rewriting iTunes URLs rather than
  passing them raw.
  - **#549** — catch-all WARN in `start_download` for any URL that
  bypasses frontend validation.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.41.1] - 2026-04-23

### 🐛 Bug Fixes

- **(parser)** Accept classical.music.apple.com + slug-less Share URLs

Apple migrated Apple Music Classical to the `classical.music.apple.com`
  subdomain in 2026 and dropped the human-readable slug segment from
  Share-link URLs. The new shape is `/{sf}/{type}/{id}` instead of the
  classic `/{sf}/{type}/{slug}/{id}`, often with a `?l=en-GB` locale
  hint appended. Both changes broke MeedyaDL's URL validators, which is
  a live production regression — classical downloads via the Apple Music
  Classical Share button were rejected with "Please enter a valid Apple
  Music URL".

- **(parser)** Accept classical.music.apple.com + slug-less Share URLs (urgent live regression) (#565)

## Urgent production regression fix

  Apple migrated Apple Music Classical to the `classical.music.apple.com`
  subdomain in 2026 and dropped the slug segment from Share-link URLs.
  **Current state on `main`**: pasting a Classical Share link into
  MeedyaDL is rejected with `"Please enter a valid Apple Music URL"` —
  classical downloads via the native Share button are unreachable.

  Captured live 2026-04-23 from the Apple Music Classical app Share → Copy
  Link:

  ```
  https://classical.music.apple.com/gb/album/1844602145?l=en-GB
  ```

  Two axes of breakage:

  1. **Domain**: `classical.apple.com` → `classical.music.apple.com` (a
  sub-subdomain of `music.apple.com`).
  2. **Path shape**: `/album/{slug}/{id}` → `/album/{id}` — the
  human-readable slug is gone.

  Cosmetically there's also a `?l=en-GB` locale hint our `?i=` capture
  group harmlessly ignores.

  ## Changes

  ### Frontend (`src/lib/url-parser.ts`)

  - `isAppleMusicUrl()` adds `classical.music.apple.com` to the accepted
  hostname list.
  - `SERVICE_DOMAINS` routing list gets the same domain so
  `detectService()` returns `apple-music`.

  ### Backend (`src-tauri/src/services/apple_music_api.rs`)

  All five entity regexes (album, song, music-video, artist,
  catalog-playlist) + `NON_GEO_RE` updated:

  - Domain alternation `(?:classical|music|itunes)` →
  `(?:classical(?:\.music)?|music|itunes)`. Covers all four hostnames:
  `music.apple.com`, `classical.apple.com`, `classical.music.apple.com`,
  `itunes.apple.com`.
  - Slug segment `[^/]+/` → `(?:[^/]+/)?`, making it optional. Both
  classic `/album/slug/id` and new `/album/id` forms parse.
  - Docstrings on `parse_apple_music_url` and `normalize_apple_music_url`
  updated.

  ### Backend (`src-tauri/src/commands/gamdl.rs`)

  **No change** — `SUPPORTED_HOSTS` already allows subdomains via
  `strip_suffix` at line 174, so `classical.music.apple.com` passes host
  validation. Only the parser regexes needed fixing.

  ## Test coverage

  **Rust** — 9 new tests in `apple_music_api::tests`:

  - `parse_new_classical_album_url_without_slug`
  - `parse_new_classical_album_url_with_locale_query` (`?l=en-GB`)
  - `parse_new_classical_album_url_with_track_id` (`?i=`)
  - `parse_new_classical_song_url_without_slug`
  - `parse_new_classical_music_video_url_without_slug`
  - `parse_new_classical_artist_url_without_slug`
  - `parse_new_classical_playlist_url_without_slug`
  - `parse_new_classical_album_url_with_slug_still_works` (defensive — if
  Apple keeps emitting slugged URLs for back-compat, we're covered)
  - `parse_classic_slugless_form_on_music_apple_com` (defensive — if Apple
  rolls slug-less out to main domain, we're covered)

  Plus 2 normalize tests:

  - `normalize_new_classical_url_without_storefront` — storefront
  injection on the new domain
  - `normalize_new_classical_url_with_storefront_unchanged` — idempotency
  check

  **TypeScript** — 4 new tests in `url-parser.test.ts`:

  - `accepts classical.apple.com URLs` (filled in missing legacy coverage
  while I was there)
  - `accepts classical.music.apple.com URLs`
  - `accepts classical.music.apple.com URLs with slug-less path + locale
  query` (the live shape)
  - `classifies new classical.music.apple.com album URLs (slug-less)`
  - `classifies new classical.music.apple.com album URL with ?l= locale
  query`
  - `classifies new classical.music.apple.com song URL with ?i= track id`

  ## Local verification

  ```
  cargo clippy -- -D warnings  ✓ clean
  cargo test --lib services::apple_music_api::tests  65 passed, 0 failed
  npx vitest run url-parser.test.ts  45 passed
  ```

  ## Risk

  Low. All changes are additive in regex alternation (existing URL shapes
  still parse identically; new shapes gain coverage). No behaviour change
  for the three hostnames that were already supported. Regex correctness
  is covered by the existing 65-test suite plus the new 9 tests.

  ## Related

  - **#547** — Apple Music Classical movement-title collision audit was
  blocked on this same regression (can't paste a Classical URL to
  reproduce). With this PR merged, the #547 repro is unblocked.
  - **#560** — independent PR with orthogonal classical-URL diagnostic
  logging. No conflict; this fix landing on main first will cleanly merge
  into #560.

  ## Test plan (post-merge)

  - [ ] Paste the URL from the original bug report into MeedyaDL —
  download starts (auth permitting)
  - [ ] Paste a slugged classical URL from the old app — still parses
  - [ ] Paste a regular `music.apple.com` URL — no regression
  - [ ] Proceed with #547 manual repro (unblocked)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- **(codeql)** Enable security-and-quality query suite (#564)

Adds CodeQL code quality queries alongside the existing security
  queries for the actions and javascript-typescript language matrix.
  Surfaces code-quality findings in the Security tab.

  Rust remains excluded from CodeQL (see workflow header); clippy
  covers Rust quality in ci.yml.


## [0.41.0] - 2026-04-23

### ✨ Features

- **(logging)** Trace Apple Music library URL submissions (#546)

Library URLs (e.g. music.apple.com/{sf}/library/albums/l.XXXX) pass the
  backend URL validator's host allowlist but are not matched by
  parse_apple_music_url or normalize_apple_music_url, so they fall through
  to GAMDL with no MeedyaDL-side metadata prefetch or filename safety net.

  Whether GAMDL's iTunes Lookup resolves l.XXXX library IDs is unverified.
  Emit a log line at start_download so downstream behaviour (album folder
  vs. no_album_* template fallback vs. outright rejection) can be
  correlated with the URL class without re-running the download.

  Investigation only; no behaviour change.

- **(logging)** Trace Apple Music Classical URL submissions (#547)

classical.apple.com URLs are treated identically to music.apple.com in
  parse_apple_music_url and normalize_apple_music_url (shared regex
  alternation at apple_music_api.rs:430-460) and share every filename
  template, metadata prefetch path, and artwork fetch.

  Classical movement titles ("Allegro", "Andante", "Adagio", "Finale",
  "Intermezzo", ...) are extremely non-unique. Within a single symphony,
  {disc}-{track:02d} prefixes disambiguate; but when album context is lost
  (direct song URL, curated cross-work playlist, no_album_* fallback),
  identical movement names collide.

  Emit a log line so support can correlate downstream filename-path
  behaviour with classical-vs-pop content without replaying the download.
  Investigation only; no behaviour change.

- **(logging)** Warn on legacy itunes.apple.com URL submissions (#548)

itunes.apple.com is in SUPPORTED_HOSTS (validator passes) and in the
  NON_GEO_RE alternation (storefront injection works). But the main
  parse_apple_music_url regexes (apple_music_api.rs:430, 437, 443, 449,
  460) only alternate `(?:classical|music)` — iTunes URLs fail every
  parser branch, so they reach GAMDL raw with no metadata prefetch.

  Whether GAMDL's own URL regex accepts iTunes Store URLs is unverified;
  if it rejects silently, the download errors mid-pipeline with no
  user-facing "this URL format is legacy" hint. Emit a WARN so the audit
  can classify outcomes and decide whether to reject at the validator
  with a clear message.

  Investigation only; no behaviour change.

- **(logging)** Warn on unrecognised Apple Music URL shapes (#549)

Catch-all for any URL that passes the host allowlist but is not matched
  by parse_apple_music_url after normalisation — i.e. neither an
  album / song / music-video / artist / catalog-playlist URL nor a
  `/library/` URL (those already have a #546 trace).

  Uploaded / "post" videos (backstage clips, live sessions, interviews)
  are the concrete case #549 tracks, and the exact URL path Apple uses
  for them isn't documented anywhere MeedyaDL can rely on. Rather than
  guess a path substring to match, catch every unrecognised shape in a
  single WARN log. Gives the audit the telemetry to decide between
  rejecting at the validator (A) and building a parser/pipeline for the
  unrecognised class (B), without having to reproduce the specific URL
  shape first.

  Investigation only; no behaviour change.

- **(filename-safety)** Engine-contract trait scaffold (#551)

Introduces `services::filename_safety` — a design-time invariant
  checker every engine integration must satisfy before being wired
  into the download pipeline.

  Trait `FilenameSafetyContract` codifies four invariants:
  1. stable_unique_id_placeholder is declared in supported_placeholders
  2. fallback_file_template contains the stable unique ID placeholder
     in the engine's native syntax (prevents the #527 empty-{title}
     class of bug)
  3. fallback_folder_template is not a bare [Unknown] / Unknown Album
     sentinel (prevents the #531 class)
  4. Neither template is empty

  `verify_contract()` enforces these statically — suitable for unit
  tests in each engine's test module. Not a runtime guard; runtime
  filename safety remains in utils::fs_safe.

  Four conformance impls ship with this commit:

  - GamdlFilenameSafety — the reference implementation, mirrors
    MV_NO_ALBUM_FILE_TEMPLATE / MV_NO_ALBUM_FOLDER_TEMPLATE in
    download_queue.rs with a lockstep test that fails if either
    constant drifts from the contract declaration.
  - VotifyFilenameSafety, YtdlpFilenameSafety,
    GetIplayerFilenameSafety — stubs for #101 / #102 / #103 / #104
    with placeholder syntax hooks (CurlyBraces / PercentParens /
    AngleBrackets). Must be tightened when each engine is actually
    wired into engine_runner::get_command_builder.

  DEV_NOTES.md gets a new "Engine Integration Checklist (#551)"
  section with the reviewer checklist from the issue body, plus
  instructions for adding a new engine's contract.

- **(filename-safety)** Engine filename-safety contract (#551)

Design-review trait every new engine integration (votify, yt-dlp,
  get_iplayer) is expected to implement. Four default conformance checks
  catch the #527/#531/#537 class of bug: stable-ID-less filenames,
  [Unknown]-sentinel folders, empty-metadata renders, same-filename dedup
  collisions. Ships GAMDL's music-video fallback as the first conformance
  example plus a reviewer checklist in DEV_NOTES.md.


### 🐛 Bug Fixes

- **(parser)** Accept itunes.apple.com in parse_apple_music_url (#548)

parse_apple_music_url's five entity regexes (album, song, music-video,
  artist, catalog playlist) only alternated `(?:classical|music)` — legacy
  `itunes.apple.com` URLs fell through every branch despite passing the
  backend host allowlist and the NON_GEO_RE storefront-injection check.
  Net effect: iTunes URLs missed metadata prefetch, missed Tier 4 safety
  net candidacy, and reached GAMDL with no MeedyaDL-side preparation.

  Extend each alternation to `(?:classical|music|itunes)` so iTunes URLs
  ride the same prefetch + normalisation rails as the other two domains.
  GAMDL still receives the iTunes URL verbatim (we don't rewrite the
  domain); the #548 WARN log in start_download stays in place to flag
  downloads where GAMDL itself may reject the legacy scheme.

  Adds six unit tests locking the fix in (album, album+track, song,
  music-video, artist, catalog playlist), mirroring the existing
  classical-domain test coverage.

- **(ci)** Clippy doc_lazy_continuation + verifier check ordering

Two CI-breaking issues from the initial #551 scaffold:

  1. `gamdl_options.rs:603` — the uploaded-video docstring started a
     line with `+ interface_uploaded_video.py` which clippy's
     doc_lazy_continuation lint interprets as an un-indented Markdown
     list continuation. Reword `+` to `and` so the bullet-list parse
     doesn't trigger.

  2. `filename_safety::verify_contract` — the empty-template check
     ran AFTER the "template contains ID placeholder" check, so an
     empty `fallback_file_template` got the confusing "does not
     contain '{id}'" error instead of the intended "must not be
     empty". The `verifier_rejects_empty_templates` test explicitly
     asserted the clearer message; it was catching a real UX bug in
     the verifier. Move the emptiness checks above the contains
     check so diagnostics are clearest.

  Verified locally: cargo clippy -- -D warnings passes; cargo test
  --lib reports 810 passed, 0 failed.

- **(filename-safety)** Scope HashSet import to tests module

clippy -D warnings rejected the top-level `use std::collections::HashSet`
  because the type is only referenced inside `#[cfg(test)] mod tests`. Move
  the import under the tests module to silence `unused_imports` without
  touching runtime code.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- **(lyrics)** Document sidecar regeneration policy (#550)

The four lyric/subtitle generators have non-uniform write behaviour:
  .lrc and .srt overwrite unconditionally, .ttml is overwritten by the
  syllable-lyrics upgrade path, while .vtt and .ass already skip when
  the target file exists. Hand-edited .lrc/.srt/.ttml sidecars are
  therefore silently replaced on the next enrichment pass.

  After considering four policies (status quo + docs / content-hash
  skip / opt-in preservation / .bak backup), the audit settled on
  Option A: document the behaviour so users with hand-edited sidecars
  know to rename or disable the generator before re-running enrichment.

  - Add "Lyric Sidecar Regeneration" section to help/lyrics-and-metadata.md
    with a per-generator behaviour table and workaround guidance.
  - Add "Lyric Sidecar Regeneration Policy (#550)" section to DEV_NOTES.md
    with file:line anchors for every write site and a note on what a
    future guard would need to change.

  No code changes — behaviour is unchanged from current releases.

- **(lyrics)** Document sidecar regeneration policy (#550) (#556)

## Summary

- Update CHANGELOG.md [skip ci]
- Document uploaded-video pipeline gap (#549)

Apple Music ships label/artist-uploaded videos (backstage, live
  sessions, interviews) with their own GAMDL entry points
  (downloader_uploaded_video.py / interface_uploaded_video.py) and a
  sparse tag shape — {artist, date, title, title_id, storefront}, no
  album/disc/track/album_artist.

  MeedyaDL has no URL detection, no routing through
  download_music_video_by_url(), and no UI surface for uploaded videos.
  The MV-safe MV_NO_ALBUM_*_TEMPLATE constants therefore never apply to
  them — if an uploaded-video URL reaches GAMDL (deep link, drag-drop,
  direct IPC), it inherits the audio-oriented no_album_* templates and
  loses the {title_id} uniqueness guarantee. Same class as #527/#531,
  different URL scheme.

  Annotate both the `uploaded_video_quality` field and the
  `MV_NO_ALBUM_FOLDER_TEMPLATE` constant so the gap is visible from the
  code; implementation follow-up tracked in #549.

  Documentation only; no behaviour change.

- **(claude)** Document URL audit diagnostics (#546/#547/#548/#549)

Records the four URL-classification logs added to start_download as
  part of the #487 audit umbrella, and notes that the parse_apple_music_url
  regex alternation now includes itunes consistently across all five
  entity patterns.

- Document lyrics sidecar overwrite behaviour (#550)

Add intentional-generator note to DEV_NOTES.md and end-user warning to
  help/lyrics-and-metadata.md. Sidecar writers (.lrc .srt .vtt .ass) and
  the syllable-lyrics TTML upgrade path all overwrite unconditionally by
  design; manual edits are not preserved across re-enrichment.

- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- **(gamdl)** Merge four URL audit loops into one (#546/#547/#548/#549)

The per-class diagnostic logs introduced earlier this session each ran
  their own `for url in &request.urls` iteration, producing four
  near-identical loops over the same slice. Consolidate into a single
  loop that classifies each URL against all four conditions in order,
  preserving the exact set of log lines emitted for every URL class
  (library, iTunes, classical, unrecognised) — same messages, same
  levels, same sequencing.

  - Reduces per-enqueue iteration count from 4×N to 1×N.
  - Caches `is_library` so the #549 catch-all guard doesn't re-check the
    path substring.
  - Replaces the "Already logged by #546 trace above" continue+comment
    with a structural `!is_library` guard inside the catch-all branch.

  No behaviour change — the set of log entries per URL is identical.


## [0.40.1] - 2026-04-22

### 🐛 Bug Fixes

- **(playlist)** Add {playlist_id} to default template + settings migration (#545)

Two playlists sharing `{playlist_artist}` + `{playlist_title}` silently
  overwrote each other's `.m3u8` file under GAMDL's default template
  `"Playlists/{playlist_artist}/{playlist_title}"`. Heal by adding Apple
  Music's stable numeric `{playlist_id}` — deterministic across
  re-downloads, unique per playlist, no datetime foot-guns (same
  rationale as the MV `{title_id}` fix in #531).

- **(compilation)** Add {album_id} to default template + extend v3→v4 migration (#552)

Two Various-Artists compilations sharing the same `{album}` name
  (e.g., two different `"Greatest Hits"`) silently intermixed in a
  shared `Compilations/Greatest Hits/` folder under GAMDL's default
  `"Compilations/{album}"` template. Tracks with different titles
  co-located; tracks with identical titles silently skipped under
  `overwrite=false`; `manifest.meedyadl` overwritten by the last
  download — breaking smart-redownload detection for both albums.

  Heal by adding `{album_id}` (Apple Music's stable numeric album ID)
  — same pattern as #545's `{playlist_id}` fix. Both are bundled under
  the same v3 → v4 migration since they close at the same version
  boundary.

- **(fs-safe)** Content-aware dedup for API JSON dumps (#553, supersedes #492)

The verbose-mode API response dump (`{album}-applemusic-data.json`) was
  wrapped in `write_non_clobbering` at `download_queue.rs:5433`, so it
  never silently overwrote a prior dump — but it did create `.1.json`,
  `.2.json`, ... on every re-download, even when the API response was
  byte-identical to the previous run. Disk bloat on repeat runs.

  Add `fs_safe::write_deduped(dir, name, contents)` — compares bytes to
  any existing file first:
  - absent                    → normal write
  - present + identical bytes → no-op, returns existing path
  - present + different bytes → disambiguates to `.1`, `.2`, ...

  This keeps the collision-proof invariant (never silently replace a
  file that differs) while avoiding the pointless-duplicate sprawl.

  Swap the API-dump call site from `write_non_clobbering` to
  `write_deduped`. Future callers with the same idempotent-content
  pattern (cached metadata, deterministic exports) can opt in too.

  Tests (4 new):
  - Creates file when absent
  - No-op when bytes identical; directory count stays at 1
  - Disambiguates to `.1.ext` when bytes differ; both files preserved
  - Repeat-identical stress case: 5 writes leave 1 file on disk

- Playlist + compilation + API-dump filename collisions (#545, #552, #553) (#554)

## Summary

  Three filename-collision fixes from the #487 audit pass, each closing
  its own ticket, unified by a single **settings schema v3 → v4**
  migration. All concrete-fix-ready risks from the audit; the remaining
  six investigation/architecture tickets (#546, #547, #548, #549, #550,
  #551) stay open for follow-up.

  3 commits, 7 files, +218 / −28.

  ## What changed

  ### `b4bf8cd` — Playlist `.m3u8` collision (#545)
  Two playlists with the same `{playlist_artist}` + `{playlist_title}`
  silently overwrote each other's `.m3u8`. Default now
  `"Playlists/{playlist_artist}/{playlist_title} ({playlist_id})"`.
  `{playlist_id}` is Apple Music's stable numeric ID — unique +
  deterministic (same pattern as MV `{title_id}` in #531).

  ### `a4bdaa0` — Compilation folder collision (#552)
  Two Various-Artists compilations with the same `{album}` name intermixed
  in a shared `Compilations/{album}/` folder (silent track skips under
  `overwrite=false`, manifest.meedyadl overwritten). Default now
  `"Compilations/{album} ({album_id})"`. `{album_id}` gives per-release
  uniqueness with the same semantics.

  ### `6d3255e` — API JSON dump dedup (#553, supersedes #492)
  Verbose-mode API response dump accumulated `.1.json`, `.2.json`, ... on
  every re-download even when bytes were identical. Added
  `fs_safe::write_deduped(dir, name, contents)`:

  | Target state | Action |
  |---|---|
  | Absent | Normal write |
  | Present + bytes identical | No-op (return existing path) |
  | Present + bytes differ | Disambiguate to `.1.{ext}` |

  Swapped the API-dump call site from `write_non_clobbering` →
  `write_deduped`. Future idempotent-content writers (cached metadata,
  deterministic exports) can opt in.

  ## Settings migration v3 → v4

  Bundled under one migration since both #545 and #552 close at the same
  version boundary. Exact-match heal on the legacy defaults; custom user
  values preserved. `CURRENT_SETTINGS_VERSION` bumped 3 → 4. Stale test
  assertions that hard-coded `version == 3` updated to
  `CURRENT_SETTINGS_VERSION` so they track future bumps.

  ## Frontend

  `TEMPLATE_VARIABLES` in `src/lib/template-parser.ts` now exposes
  `{playlist_id}` and `{album_id}` in the visual template builder with
  collision-safety descriptions. Sample-data block extended with
  representative IDs so the live preview works.

  ## Test plan

  - [x] `cargo test --lib` — 788 passed (780 pre-fix + 8 new)
  - [x] `cargo clippy --lib --all-targets` — clean
  - [x] 11 new unit tests across all three fixes (migration heal +
  preserve-custom + v0→current end-to-end + default-template invariants +
  `write_deduped` behaviour including a 5-repeat-writes stress case)
  - [ ] Manual: download two playlists with matching artist+title on macOS
  (real Apple Music account) and verify no `.m3u8` overwrite — needs
  maintainer verification
  - [ ] Manual: download two Various-Artists compilations with matching
  album name; verify separate folders
  - [ ] Manual: upgrade from a v3-era settings.json and confirm
  `playlist_file_template` + `compilation_folder_template` heal on first
  launch

  ## Follow-up

  Open on #487 umbrella:
  - #546 — library URL audit
  - #547 — Classical movement collision audit
  - #548 — iTunes legacy URL audit
  - #549 — uploaded-video pipeline
  - #550 — lyrics sidecar overwrite (docs vs. guard)
  - #551 — platform-agnostic `FilenameSafetyContract` trait (pre-flight
  before M8 BBC iPlayer work)

  Auto-closes on merge: #492, #545, #552, #553.


  ---
  _Generated by [Claude
  Code](https://claude.ai/code/session_01Piay1zSSu6z2uWMsSWNzsA)_


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.40.0] - 2026-04-22

### ✨ Features

- **(activity-log)** Persistent on-disk activity log for bug hunting (#541)

Adds a daily-rotating `activity-YYYY-MM-DD.log` file under the logs
  directory that mirrors every `ActivityLogEvent` as it happens — the
  complete forensic record for bug hunting, unaffected by the 10,000-line
  in-memory cap or the Verbose UI filter.

  Implementation highlights:

  - New `services/activity_log_writer.rs` — buffered Tokio background task
    fed via an unbounded `mpsc` channel. Uses `BufWriter<File>` with a
    500ms flush tick and UTC date rollover detection. Polls the shared
    `ShutdownSignal` to flush and drain on window close / tray quit.
  - `utils/activity_log.rs` — new `register_disk_writer()` + `write_to_disk()`
    helpers backed by a `OnceLock`. All four `emit_*` helpers fan out to
    disk after emitting the Tauri event; verbose events persist to disk
    regardless of the UI filter (the file is the forensic record).
  - `services/download_queue.rs` — the four direct-emit sites
    (`emit_companion_stream_line`, stdout/stderr readers, track separator)
    now call `write_to_disk(&event)` alongside `app.emit(...)`.
  - No change to the in-memory activity store, virtualiser, or RAF
    batching. No hot-path disk I/O. No risk of reintroducing the 14 GB
    WebView RAM leak (#370).

  Frontend UX:

  - **Export Disk** — concatenates the last 3 daily files via
    `export_disk_activity_log` IPC, opens a native save dialog.
  - **Reveal** — opens the logs folder via `@tauri-apps/plugin-shell`'s
    `open()` using the path returned by `get_logs_folder_path` IPC.
  - Existing **Export** (in-memory view, respects filters) preserved.

  User-configurable location:

  - New `activity_log_path_override: String` setting (empty = default
    `{app_data_dir}/logs/`).
  - `lib.rs::resolve_activity_log_dir()` validates the override via
    `create_dir_all`; falls back to default with `log::warn!` if
    unwritable.
  - UI: Browse + Reset buttons in Settings > Advanced > Diagnostics.
    Applies on next app restart (writer owns the file handle).
  - `clear_old_logs()` scans the override dir too, honouring the 7-day
    retention.

- **(activity-log)** Persistent on-disk activity log for bug hunting (#541) (#542)

### 🐛 Bug Fixes

- **(lyrics)** Rename sidecars alongside codec-suffixed audio (#535)

When native --song-codec-priority is active, GAMDL writes audio and
  lyrics/subtitle sidecars on a clean stem because the actual codec is
  unknown until the download finishes. The post-enrichment codec-suffix
  rename only touched the audio file, so .ttml/.lrc/.srt/.vtt/.ass
  sidecars stayed on the clean stem — leaving Dolby Atmos tracks with no
  lyrics once an overlapping companion tier with a clean-filename slot
  took over those files.

  Add rename_matching_sidecars() to move all five sidecar formats in
  lockstep with the audio rename. Idempotent: skips missing sources and
  existing suffixed targets so safe_rename's auto-disambiguation can't
  produce "[Dolby Atmos] (1).ttml" noise files when an overlapping run
  has already written a suffixed sidecar.

- **(lyrics)** Rename sidecars alongside codec-suffixed audio (#540)

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.39.0] - 2026-04-22

### ✨ Features

- **(activity-log)** Emit dedup settings in startup summary (#530)

Adds a fourth `Dedup: scope=..., key=..., preferences=...` line to
  `emit_startup_settings_summary`. Surfaces all three duplicate-detection
  configuration knobs on the `[System]` channel at every app launch:

  - `scope` — off / intra_session / intra_and_queued (default) /
  intra_and_queued_and_history
  - `key_strategy` — song_id+isrc_fallback (default) / isrc_only /
  song_id_only
  - `preference_order` — the artist-auto-select mode priority, rendered as
  a `>`-joined CLI-style list (e.g.
  `main-albums>singles-eps>compilation-albums>live-albums>top-songs`)

  Without this, diagnosing "why did my second album redownload tracks I
  already had?" requires digging into settings.json by hand. Companion to
  the per-download `Album dedup: kept N, skipped M` / `Checking album
  against already-queued ...` lines that already emit from
  `commands::gamdl::start_download` whenever dedup fires.


### 🐛 Bug Fixes

- **(download)** Force MV-safe no-album templates + heal legacy defaults (#531)

Music video companion downloads were landing as `-.mp4` inside
  `{artist}/[Unknown]/` folders for users upgrading from pre-v2 settings.
  Two causes, one bug:

  1. `download_music_video_by_url()` inherited the user's audio-oriented
     `no_album_folder_template` / `no_album_file_template`. A direct
     `/music-video/` URL has no album context, so GAMDL routes through the
     no-album template path. Override those two fields with fixed MV-safe
     values (`{artist}/Music Videos` + `{title}`) regardless of user
     settings — MVs never have a `{disc}` or `{album}` context, so audio
     templates are never the right fit.

  2. Pre-v2 MeedyaDL shipped `no_album_folder_template` as
     `"{artist}/[Unknown]"` and `no_album_file_template` as `"{disc} - "`.
     Serde only fills missing fields with defaults, so upgraders kept the
     original broken values. Add a v2 → v3 settings migration that heals
     exact matches of those legacy defaults to the current defaults
     (`{artist}/Unknown Album` + `{title}`); custom values are preserved.

  Adds 7 unit tests (762 → 769 passing).

- **(download)** MV filename uniqueness + motion-art renaming pass (#527 #536 #537)

Follow-up to the MV no-album template fix. Three coordinated changes plus
  the documentation that specifies the resolution order for future tiers.

  1. **MV filename uniqueness** — last-resort template
     `MV_NO_ALBUM_FILE_TEMPLATE` tightened from `"{title}"` to
     `"{title} ({title_id})"`. `{title_id}` is Apple Music's numeric MV ID:
     deterministic across re-downloads (dedupe survives) and unique per MV
     (same-title cuts — Clean/Explicit, remixes, live versions — no longer
     silently collide under GAMDL `overwrite=false`). Datetime was
     deliberately rejected as a disambiguator because it would cause every
     re-download to create a new file.

     The four-tier resolution spec now lives in `DEV_NOTES.md` under
     "Music-Video Filename & Folder Resolution". This PR only implements
     Tier 4 (the safety net); Tiers 2 (Apple Music Catalog `include=albums`)
     and 3 (MeedyaDL-known parent album context) are tracked in #537 and
     land in a separate PR. Tier 1 (GAMDL's native iTunes Lookup) already
     works upstream and is unchanged.

  2. **Motion artwork rename** — `PortraitCover.mp4` →
     `FrontCoverPortrait.mp4`. The two album-motion variants now sort
     adjacent (`FrontCover` + `FrontCoverPortrait`) in any alphabetical
     listing, and the portrait filename is self-describing. No
     auto-migration of legacy files on disk — renaming without consent is
     risky; users who want a clean sweep can delete `PortraitCover.mp4`
     before re-running animated artwork on an album.

  3. **Artist Spotlight priority reorder** —
     `fetch_artist_promo_video()` now consults only the two 16:9
     artist-framed feeds (`motionArtistFullscreen16x9` →
     `motionArtistWide16x9`), with Fullscreen preferred for its typically
     higher source resolution and full-bleed framing. The previous chain
     fell through to `motionDetailSquare` / `motionDetailTall`, but those
     album-detail feeds are tightly cropped around cover art and look
     visually wrong used as an artist-page hero. Prefer skipping the
     download over a mismatched fallback.

  Docs updated across DEV_NOTES.md (new "Music-Video Filename & Folder
  Resolution" section, updated motion-artwork table), CLAUDE.md
  (animated-artwork and artist-promo bullet points plus a new MV
  resolution bullet), README.md, Project_Plan.md, help/animated-artwork.md
  (including stale `ArtistCover.mp4` → `ArtistSpotlightCover.mp4` fixups),
  and inline docstrings on every touched symbol.

  Test updates: new assertion that `MV_NO_ALBUM_FILE_TEMPLATE` contains
  `{title_id}` (hard invariant — removing it re-opens the silent-collision
  regression). All 769 Rust tests pass, clippy clean.

  Follow-ups tracked:
  - #537 Tiers 2 & 3 (Apple Music Catalog + parent-album context wiring)
  - New issue (to file): `AlbumSpotlightCover.mp4` — when an album's own
    `editorialVideo` includes a `motionArtist*` 16:9 feed (as opposed to
    the artist-page feed), save it to the album folder with that filename
    so it's distinguishable from the artist-wide spotlight.

- **(download)** MV naming uniqueness + motion-art rename + settings migration (#539)

## Summary

  RC-blocker fix for #527 + motion-art polish pass (#536 partial). Two
  commits, 14 files, +293 / −49.

  - **`2fff25e`** — force MV-safe no-album templates + v2→v3 settings
  migration heals legacy broken defaults (#531)
  - **`b70e200`** — MV filename uniqueness via `{title_id}` +
  `PortraitCover.mp4` → `FrontCoverPortrait.mp4` rename + Artist Spotlight
  priority trim + full spec in `DEV_NOTES.md`

  ## What changed

  ### 1. Music-video filename bug (#527) — RC blocker, resolved


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.38.0] - 2026-04-21

### ✨ Features

- **(gamdl)** V3.0 compatibility + long-term version management (#525)

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.37.0] - 2026-04-21

### ✨ Features

- **(dedup)** Pre-queue track-level duplicate detection for artist URLs (#510)

When an Apple Music artist URL is fanned out across multiple
  artist_auto_select_multi modes (e.g. main-albums + singles-eps +
  compilation-albums), the same song would previously be downloaded
  multiple times at the same quality because each mode spawned an
  independent GAMDL subprocess with no cross-process awareness.

  Fetch each mode's track list via the Apple Music catalog API
  before enqueueing, then skip duplicates according to a
  user-configurable preference hierarchy (default: main-albums >
  singles-eps > compilation-albums > live-albums > top-songs). The
  winning mode's queue item is rewritten to explicit per-track
  URLs; modes with zero unique tracks are suppressed entirely. API
  failures fall back to the original artist URL so downloads are
  never blocked.

  Scope is configurable (off / intra-session / intra+queued /
  intra+queued+history) and match key strategy can be swapped
  between song_id (with ISRC fallback), ISRC-only, or song_id-only.
  Companion-format downloads are unaffected — a song chosen from
  one mode still runs the full ALAC/Atmos/AAC companion chain.

- **(dedup)** Skip playlist tracks that overlap queue or history (#512)

When a user queues an Apple Music playlist URL, fetch the playlist's
  tracks via the catalog API and trim it to the subset that isn't already
  present in the active queue or in existing manifest.meedyadl files
  (per the duplicate_detection.scope setting). All tracks duplicated →
  the playlist isn't enqueued at all.

  - Extend ParsedAppleMusicUrl with a playlist_id field and add a catalog
    PLAYLIST_RE matcher (library playlists deliberately skipped — they'd
    need Music-User-Token auth and a different endpoint).
  - Add fetch_playlist_tracks() to apple_music_api.rs (paginated, 50-page
    cap matching fetch_artist_albums).
  - Add plan_playlist_deduplication() + PlaylistPlan / PlaylistDedupPlan
    public types to duplicate_detector.rs, reusing the existing
    build_track_key_from_parts / build_track_url / resolve_jwt helpers.
  - Wire the planner into start_download's single-URL path (single
    playlist URLs only; batch pastes will be covered by #513).
  - Intra-playlist dedup: a song listed twice in the same playlist is
    also collapsed to one download.
  - Graceful fallback on any failure path (missing token, API error,
    library playlist, zero usable tracks) — never blocks a download.

- **(dedup)** Skip album tracks already in queue or history (#514)

When a single album URL is queued, cross-check its tracks against the
  active queue and (per duplicate_detection.scope) existing manifest.meedyadl
  files in the output directory. Tracks already present are dropped; the
  remaining subset is enqueued as per-track URLs. If every track is a
  duplicate, nothing is enqueued and the caller receives a duplicate_warning
  surfacing the "everything already downloaded" state.

  - Add plan_album_deduplication() + AlbumPlan / AlbumDedupPlan in
    duplicate_detector.rs, reusing build_track_key / build_track_url /
    resolve_jwt from the artist + playlist planners.
  - Wire into start_download's single-URL path (batch pastes will be
    handled by #513). Album URLs with ?i=song_id are passed through
    unchanged — those are explicit single-song requests.
  - Short-circuit the catalog API call when there's nothing to compare
    against (empty queue + history).
  - No dedup runs → the original album URL is kept unchanged (more
    efficient than switching to per-track URLs for no gain).

  Graceful fallback on API / credential failures; never blocks a
  download.

  Interaction with smart re-download (#263): both signals can surface
  simultaneously for now; refinement (suppressing dedup when
  lastModifiedDate differs) deferred to a follow-up.

- **(dedup)** Cross-URL batch deduplication (#513)

When multiple URLs are pasted in a single request (e.g. album + playlist
  + song URLs that overlap), walk every classifiable album and playlist
  track list and apply a source-priority filter so each track is claimed
  by exactly one URL. Albums claim tracks before playlists (an album is
  the more canonical source). Song URLs and album?i=song_id URLs are
  treated as explicit picks and claim their song_ids up-front. Artist,
  music-video, and unrecognised URLs pass through unchanged.

  - Add plan_batch_deduplication() + BatchUrlAction / BatchDedupPlan in
    duplicate_detector.rs. Reuses the existing JWT resolver, track-key
    builder, and per-track URL builder.
  - Wire into start_download BEFORE the single-URL planners (#510, #512,
    #514) so the later planners see the already-trimmed URL list.
  - Skip the expensive fetch when urls.len() < 2 or no album/playlist
    URL is present in the batch.
  - When every URL in the batch is fully deduped, return a
    duplicate_warning and don't enqueue anything.

  Rename parse_playlist_url_returns_none to parse_playlist_url_extracts_id
  (now that #512 made playlist URLs parseable) and add a matching
  parse_library_playlist_url_returns_none check.

  All 728 lib tests + 293 frontend tests pass. Rust compiles clean.

- **(dedup)** Pre-queue duplicate detection for artist, playlist, album, and batch URLs (#511)

## Summary

  Comprehensive pre-queue duplicate-detection across **every URL type**
  MeedyaDL handles. Before any URL hits GAMDL, the Apple Music catalog API
  is consulted to identify tracks that would otherwise be downloaded
  multiple times at the same quality — whether across artist auto-select
  modes, inside a playlist, inside the download history, or across a batch
  of pasted URLs. A user-configurable priority hierarchy + scope + key
  strategy decides which copy wins; the rest are skipped with an Activity
  Log entry naming the kept and skipped sources.

  **Scope guarantee (unchanged across all 4 issues):** operates on **track
  identity** only. Companion-format downloads (ALAC / Atmos / AAC etc.)
  are untouched — a song chosen by dedup still runs the full
  `companion_mode` chain.

- **(gamdl)** Version-aware CLI/INI dispatch for GAMDL v2.9.1 — v3.x

GAMDL v3.0 removed the --fetch-extra-tags CLI flag (upstream commit
  61ea24b, "Remove extra tags fetching and preview parsing") and migrated
  user-facing logging to structlog. Unconditionally emitting the old flag
  crashes the subprocess on v3+, and the new "[LEVEL    HH:MM:SS] ..."
  line prefix used to slip past Priority-4 error classification.

  Introduce a shared gamdl_capabilities module that caches the detected
  GAMDL version in a process-global RwLock and exposes supports(feature)
  queries. The cache is refreshed by install_gamdl() and get_gamdl_version().
  merge_options() and ini_metadata_section() now gate fetch_extra_tags on
  GamdlFeature::FetchExtraTags so the flag and INI key are only emitted on
  v2.x. Unknown-version queries return false so a freshly installed v3.0
  never sees the removed option. Tested across v2.9.1, v2.9.3, v3.0, v3.1.2,
  and None.

  Update ERROR_PREFIX_REGEX to optionally strip structlog's
  "[LEVEL    HH:MM:SS]" banner so v3.0 "Error processing ..." lines are
  still classified as GamdlOutputEvent::Error rather than Unknown.

- **(gamdl)** Compile-time version support window with pinned installer + gated upgrade prompts

MeedyaDL now declares an explicit `[minimum, maximum_tested, recommended]`
  range for GAMDL in `src-tauri/tool-versions.toml` → `[gamdl]`. The range
  is the single source of truth for four things:

  1. The installer. `install_gamdl()` now runs
     `pip install --upgrade 'gamdl>={min},<={max}'` instead of the
     unbounded `pip install --upgrade gamdl`, so first-time setup and
     in-app "Update GAMDL" clicks can never pull a release we haven't
     validated.

  2. The update banner. `update_checker::is_gamdl_compatible` is now a
     thin wrapper over `gamdl_capabilities::should_offer_upgrade`, which
     returns `false` for PyPI advertisements above `maximum_tested_version`.
     The frontend's `updateStore.getActiveUpdates()` already filters by
     `is_compatible`, so those updates disappear from the UI without
     any frontend plumbing.

  3. Startup diagnostics. `commands::dependencies::log_component_versions_to_activity`
     now emits a `[System]` activity-log line classifying the installed
     GAMDL version as NotInstalled / Supported / Unsupported / Untested
     against the window. Unsupported / Untested cases additionally hit
     `log::warn!` so they land in the rotated tracing log for crash reports.

  4. User-facing documentation. README has a new "Component Support
     Matrix" section listing the validated ranges for every component;
     SECURITY.md references it as the canonical support policy.

  New surface in `services::gamdl_capabilities`:
  - `GamdlSupportWindow { minimum, maximum_tested, recommended }`
  - `VersionSupport::{NotInstalled, Supported, Unsupported, Untested}` +
    `is_supported()`
  - `support_window()`, `classify(Option<&str>)`,
    `should_offer_upgrade(&str)`, `pip_version_spec()`

- **(ci)** Weekly PyPI watcher that tickets GAMDL releases above our tested ceiling

Adds `.github/workflows/upstream-gamdl-watch.yml`, a Monday 08:00 UTC cron
  that compares PyPI's latest GAMDL release against the `maximum_tested_version`
  declared in `tool-versions.toml`. When upstream ships past the ceiling, the
  workflow opens (or updates) a GitHub Issue labelled `upstream-bump` with a
  triage checklist covering release-notes review, commit diff review, local
  install + smoke test, and the ceiling-bump path.

  Dedupe is by exact title match on open `upstream-bump` tickets, so the
  weekly cron updates the same ticket instead of spamming 52 fresh ones
  across a year where upstream stays above the ceiling. Label creation is
  idempotent via `gh label create --force`, so fresh clones of the repo
  don't fail on the missing label.

- **(gamdl)** Emit --no-exceptions by default to clean up v3.0 mixed stderr

GAMDL v3.0 migrated logging to structlog but still lets Python print raw
  tracebacks when --no-exceptions is not set. The resulting stderr is an
  unreadable blob — structlog-formatted lines interleaved with multi-line
  tracebacks — that clutters the activity log and fools classify_error()
  into matching "Error" in frame paths like httpx/_transports/default.py
  line 118 in map_httpcore_exceptions.

  Default merge_options() to set options.no_exceptions = Some(true) so
  each download gets a single user-facing error line per failure. Users
  debugging upstream GAMDL issues can flip a new AppSettings field
  `verbose_gamdl_exceptions` (default false) from Settings > Advanced >
  Diagnostics to restore the full traceback.

- **(activity-log)** Emit dedup settings in startup summary

Adds a fourth `Dedup: scope=..., key=..., preferences=...` line to
  `emit_startup_settings_summary`. Surfaces all three duplicate-detection
  configuration knobs on the `[System]` channel at every app launch:

  - `scope` — off / intra_session / intra_and_queued (default) /
    intra_and_queued_and_history
  - `key_strategy` — song_id+isrc_fallback (default) / isrc_only /
    song_id_only
  - `preference_order` — the artist-auto-select mode priority, rendered
    as a `>`-joined CLI-style list (e.g.
    `main-albums>singles-eps>compilation-albums>live-albums>top-songs`)

  Without this, diagnosing "why did my second album redownload tracks I
  already had?" requires digging into settings.json by hand. Companion
  to the per-download `Album dedup: kept N, skipped M` / `Checking album
  against already-queued ...` lines that already emit from
  `commands::gamdl::start_download` whenever dedup fires.


### 🐛 Bug Fixes

- **(dedup)** Appease clippy::question_mark lint

Replace `let Some(playlist_id) = parsed.playlist_id.as_deref() else {
  return None; }` with `let playlist_id = parsed.playlist_id.as_deref()?;`
  — the function already returns Option<PlaylistDedupPlan>, so the `?`
  operator is equivalent and preferred by clippy in this codebase
  (CI runs `cargo clippy -- -D warnings`).

  Backend CI on all 3 OSes failed on this single lint; no other issues.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- **(gamdl)** Synthetic v3.0 output fixtures + parser integration coverage

Captures what we believe GAMDL v3.0 writes to stderr based on the
  upstream source at the v3.0 tag (cli/utils.py::custom_structlog_formatter
  plus the INFO/WARNING/ERROR strings in cli/cli.py), and exercises every
  parser that consumes GAMDL output end-to-end against those fixtures.

  Four scenarios are represented:

  1. Happy-path album download — pins the invariant that no structlog-
     prefixed INFO line is ever misclassified as an Error.
  2. Codec skips — the exact wording we believe triggers gap-fill retry.
     Tests lock in that count_codec_skip_warnings + is_codec_error see
     past the [WARNING  HH:MM:SS] prefix, and that build_gapfill_priority_chain
     still produces a usable fallback chain when experimental codecs are
     dropped.
  3. Auth / 404 error — the new ERROR_PREFIX_REGEX (from #517) must
     preserve URL and reason through classification, and classify_error
     must bucket "404 Not Found" as `not_found`.
  4. Network failure + traceback — covers the verbose_gamdl_exceptions
     opt-in path. Interleaved structlog + raw traceback must still land
     in the `network` classify bucket, traceback frames must not be
     captured as errors, and the final exception line must be.

  Bonus fix: PYTHON_EXCEPTION_REGEX now also accepts `Timeout` as a
  class-name suffix. Without it httpx's typed timeout hierarchy
  (ConnectTimeout, ReadTimeout, WriteTimeout, PoolTimeout) silently
  fell through to GamdlOutputEvent::Unknown — a pre-existing regression
  for every network timeout raised by GAMDL's HTTP stack.

  Fixtures are best-effort synthesis. Real v3.0 output samples should
  refine the skip-warning wording and any structlog-action prefixes —
  see issue #521 for the follow-up.

  Part of #521. Part of #516.


## [0.36.0] - 2026-04-21

### ✨ Features

- Companion-download resilience — soft errors, watchdog, scoping, audioTraits gate

Wraps every companion-tier GAMDL spawn in a new
  `services::companion_supervisor` so the queue stops being misled by
  GAMDL's per-track exception handling, and stops looking frozen during
  genuine post-processing.

  - **Soft-error detection (closes #500).** New `process::parse_gamdl_error_count`
    + `classify_gamdl_traceback` parse `Finished with N error(s)` and
    recognise the `NoneType.audio_track` traceback. Companion supervisor
    downgrades a soft-error exit-0 to a tier failure and logs
    "<codec> not available for this track on Apple Music — skipping"
    instead of dumping the raw traceback.

  - **Real abort with `kill_on_drop` (closes #501).** Supervisor sets
    `kill_on_drop(true)` on the GAMDL Command so when the 10-minute
    completion-task timeout aborts the supervising tokio task the GAMDL
    child is reaped instead of leaking as a zombie that keeps writing
    hours later. (`tokio::JoinHandle::abort()` on its own only fires at
    await points; combined with #502 the synchronous tail is now short
    enough that abort fully covers it.)

  - **Scoped lyrics conversion (closes #502).** `run_companion_lyrics_conversion`
    now takes `artist_hint` / `album_hint`. Targeted
    `{output_dir}/{artist}/{album}/` resolution wins; the previous
    recursive walk over the entire library is the fallback only when
    hints aren't available. Fixes the "TTML conversion of every album in
    the library" perceived hang.

  - **Post-processing indicator (closes #503).** Supervisor flips an
    `is_post_processing` flag once it sees a `100% of` line. New
    `DownloadQueue::clear_processing_label`. Queue UI now switches to
    "Post-processing companion (codec): remux / decrypt" while
    mp4decrypt / ffmpeg / mp4box run silently.

  - **audioTraits-aware tier filter (closes #504).**
    `SongCodec::required_audio_trait()` maps each codec to the API trait
    that must be present on the track. New `filter_tiers_by_audio_traits`
    drops tiers whose codec the catalog response says isn't offered for
    the track. `QueueItemStatus.audio_traits` carries the union across
    the download's tracks; populated during the early metadata fetch.
    No-op when API metadata isn't reachable.

  - **Idle watchdog (closes #505).** Supervisor polls `child.try_wait()`
    every 200 ms; if no stdout/stderr line has arrived for
    `gamdl_idle_timeout_minutes` (default 5) and we are NOT in
    post-processing, it kills the child and reports a watchdog failure
    to the activity log. New `gamdl_idle_timeout_minutes: u32` setting.

- Expose gamdl_idle_timeout_minutes in Settings > Advanced

Adds a 'GAMDL Idle Timeout' Select to Settings > Advanced > Processing
  with preset options (2 / 5 / 10 / 15 / 30 min) and an explanation of
  how the watchdog interacts with the post-processing phase. The control
  binds to the existing settings.gamdl_idle_timeout_minutes field added
  in #505; no backend changes needed.

- Wrap primary GAMDL spawn in supervisor safety nets

Extends the resilience net from the companion tiers to the primary
  GAMDL invocation in run_download_with_events. Same four guarantees,
  implemented inline since the primary's rich output parser (progress
  events, track info, ANSI stripping, \r coalescing, dedup) is tightly
  coupled with queue state in ways the generic supervisor module
  doesn't host today.

  - kill_on_drop(true) on the Command so app shutdown or the queue's
    10-min completion timeout reaps GAMDL instead of leaking a zombie.
  - Idle watchdog in the cancellation poll loop: tracks last
    stdout/stderr timestamp, kills the child after
    gamdl_idle_timeout_minutes (default 5) of silence, and stands down
    while the post-processing flag is set so a slow remux on a network
    volume doesn't trip it.
  - Soft-error detection: parses 'Finished with N error(s)' on stdout
    and, when status.success() && N > 0, downgrades to an error with
    a classified message.
  - Friendly traceback translation: primary error path now runs
    process::classify_gamdl_traceback on the combined raw stderr BEFORE
    falling back to extract_python_exception, so
     surfaces as 'this codec is not available for
    this track on Apple Music — skipping' instead of a Python dump.
  - Post-processing indicator: stdout reader flips a shared flag on
    ProcessingStep events or '100% of' progress lines, and sets the
    queue's processing_label to 'Post-processing (remux / decrypt)'
    so the UI caption doesn't look frozen during the silent phase.

- Companion-download resilience — soft errors, watchdog, scoping, audioTraits gate (#506)

### 🐛 Bug Fixes

- **(clippy)** Allow too_many_arguments on spawn_companion_downloads

The new function signature is 9 params after adding queue + audio traits
  for the #504 gate. All of them are genuinely needed inside the spawned
  task and a struct wrapper just moves repetition to call sites. Suppress
  the lint with an explanatory comment.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.35.0] - 2026-04-20

### ✨ Features

- Nightly release channel with channel-aware update guard

Adds the first slice of the multi-channel release pipeline:

  - New `nightly-release.yml` workflow runs daily at 00:00 UTC. It resets
    `nightly` to `main`, merges every `feat/*` branch (skipping conflicts
    and opening an issue for them), bumps to `X.Y.Z-nightly.YYYYMMDD`
    across package.json / tauri.conf.json / Cargo.toml, and pushes the
    tag to trigger the existing `release.yml`.
  - `UpdateChannel` enum + `update_channel` setting. `UpdateChannel::from_tag`
    parses release-tag suffixes (-nightly, -weekly, -monthly, -alpha, -beta,
    stable). `check_all_updates` filters GitHub releases to the user's
    channel, and `download_and_install_app_update` refuses to install a
    tag whose channel is less stable than the user's selection — the
    guard for "option 2" client-side channel safety.
  - Settings > General > Updates gains an Update Channel dropdown.
  - `.github/rulesets/protected-release-branches.json` + apply workflow
    keep `main`, `beta`, `alpha`, `monthly`, `weekly`, `nightly`
    undeletable even with repo-wide auto-delete on.
  - `auto-delete-merged-branches.yml` deletes merged PR head branches
    except the protected channels.

  Weekly/monthly channels fall out of the same pattern once this lands.

- Release-channel ladder, nightly auto-release, and update-channel guard (#498)

## Summary

  Introduces a six-tier release-channel ladder with automated nightly
  integration, branch protection, and an in-app update-channel guard.

  **Channel hierarchy** (least → most stable):

  ```
  feat/* → nightly → weekly → monthly → alpha → beta → main (stable)
  ```

  ### Pipeline

  - **`.github/workflows/nightly-release.yml`** — cron `0 0 * * *`. Resets
  `nightly` to `main`, merges every `origin/feat/*` (skips conflicts,
  opens an issue listing them), bumps version to `X.Y.Z-nightly.YYYYMMDD`
  across `package.json` / `tauri.conf.json` / `Cargo.toml`, force-pushes
  `nightly`, and creates an annotated tag that triggers the existing
  `release.yml`. `workflow_dispatch` supports a `dry_run` flag.
  Weekly/monthly follow the same template (crons `0 0 * * 0` and `0 0 1 *
  *`).
  - **`.github/rulesets/protected-release-branches.json`** +
  **`.github/workflows/apply-branch-rulesets.yml`** — ruleset committed as
  JSON; workflow idempotently applies it (PUT on match, POST otherwise).
  Blocks deletion + non-fast-forward on `main` / `beta` / `alpha` /
  `monthly` / `weekly` / `nightly`.
  - **`.github/workflows/auto-delete-merged-branches.yml`** — on merged
  PRs, deletes head branches except the six protected channel names.

  ### In-app update-channel guard (option 2)

  - `UpdateChannel` enum in `src-tauri/src/models/settings.rs` — ordered
  `Nightly < Weekly < Monthly < Alpha < Beta < Stable` (`PartialOrd`).
  - `UpdateChannel::from_tag()` parses pre-release suffixes
  (`-nightly.YYYYMMDD`, `-weekly.YYYYWW`, `-monthly.YYYYMM`, `-alpha.N`,
  `-beta.N`/`-rc.N`, stable).
  - `update_channel: UpdateChannel` persisted in `AppSettings`, exposed as
  the **Update Channel** dropdown in Settings > General > Updates.
  - `check_all_updates` filters releases to the user's channel.
  `download_and_install_app_update` refuses tags whose channel is less
  stable than the user's selection — enforcement point: even a tampered
  manifest or stale deep link cannot downgrade stability. Switching
  channel is always an explicit action in Settings.

  ### Docs

  - `DEV_NOTES.md` — workflow table expanded to 7 workflows; new "Release
  Channels" section.
  - `README.md` — roadmap bullet + new "Release channels" table in Quick
  Start.
  - `Project_Plan.md` — completed bullet for the release-channel ladder +
  guard.
  - `CONTRIBUTING.md` — new "Branching Model" section.
  - `CLAUDE.md` — "Release Workflow" + "Conserving GitHub Actions Minutes"
  updated; note about `workflow_dispatch` UI visibility.
  - `help/release-channels.md` (new) + linked from `help/index.md`, plus
  matching in-app topic in `HelpViewer.tsx` and a new FAQ section.

  ## Test plan

  - [ ] After merging, run `gh workflow run "Apply Branch Rulesets" --ref
  main` to apply the protected-branch ruleset (needs `RELEASE_PAT` with
  `administration:write`).
  - [ ] Enable repo setting **Settings → General → Automatically delete
  head branches**.
  - [ ] Manually trigger `Nightly Release` with `dry_run=true` to verify
  the merge + version-bump steps without pushing a tag.
  - [ ] Once dry-run is clean, let the cron fire (or dispatch with
  `dry_run=false`) to produce the first `-nightly.YYYYMMDD` build.
  - [ ] In an installed build, confirm Settings > General > Updates shows
  the **Update Channel** dropdown and changing it triggers an update
  check.
  - [ ] Confirm `cargo test --lib services::update_checker` passes (10
  tests incl. new `test_update_channel_from_tag`).
  - [ ] Confirm `npm test` passes (293 tests).

  ## Follow-up (separate PRs)

  - Weekly / monthly workflows (near-copies of `nightly-release.yml` with
  different cron + source branch).
  - macOS installer refinements for pre-release channel badging in the
  update banner.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Release channels + branch protection + update-channel guard

- DEV_NOTES.md: expand Release Workflow table to 7 workflows; replace
    "Pre-Release Channel" with a full "Release Channels" section covering
    the six-tier ladder, auto-merge pipeline, option 2 in-app guard, and
    branch-protection tooling.
  - README.md: roadmap bullet reflects the six-channel ladder; new
    "Release channels" section under Quick Start with the channel table.
  - Project_Plan.md: new bullet for the release-channel ladder + guard.
  - CONTRIBUTING.md: new "Branching Model" section; clarify feat/* naming
    and auto-delete of merged head branches.
  - CLAUDE.md: document the ladder, enum ordering, and per-call guard in
    the "Release Workflow" section; list new workflows under "Conserving
    GitHub Actions Minutes" with a note about workflow_dispatch UI.
  - help/: new release-channels.md page with channel table, switching
    guide, and guard explanation; linked from help/index.md.
  - help/faq.md: new FAQ section about channels, downgrade behaviour, and
    cadence.
  - HelpViewer.tsx: add matching in-app help topic (id: release-channels)
    with the same content as the sidecar markdown.

- Update CHANGELOG.md [skip ci]

## [0.34.6] - 2026-04-20

### 🐛 Bug Fixes

- **(fs)** Collision-proof every rename/write path (generalise #483 invariant) (#494)

## Summary

  Generalises the "different content must never land on the same filename
  silently" invariant that shipped for music-video subtitle sidecars in
  #483 to **every** rename / write / copy site in the Rust backend.
  Addresses 6 HIGH-severity sites where different logical content could
  silently overwrite on Unix.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.5] - 2026-04-20

### 🐛 Bug Fixes

- **(download)** Music video filenames, Explicit/Clean suffix, video subtitles

Three related fixes to the music-video / companion / filename pipeline:

  - fix(#481): music video companion downloads were producing `-.mp4`
    because `download_music_video_by_url()` only passed quality/path
    settings to GAMDL. Now inherits filename/folder templates, tool paths,
    language, truncate, download/remux modes so videos land with proper
    `{artist} - {title}` style names matching the primary pipeline.

  - fix(#482): `[Explicit]`/`[Clean]` suffixes were applied inconsistently.
    (a) `insert_advisory_before_codec_suffix` now pulls the full codec
    suffix set from the codec registry instead of the three hardcoded
    strings, so `[Binaural]`, `[Downmix]`, `[AAC Legacy]`, `[HE-AAC]`
    files get the correct ordering. (b) `advisory_suffix()` matches
    case-insensitively. (c) Idempotency checks are case-insensitive.
    (d) New `apply_advisory_suffixes_from_tags()` runs in the completion
    task after companion downloads finish, reading each file's `rtng`
    atom so companion files that land late still pick up the suffix.

  - feat(#483): music videos (direct or companion) now get
    subtitles / closed-captions extracted into sidecar files. New
    `music_video_subtitle_service` probes the video with ffprobe,
    extracts each subtitle stream to `.vtt` (WebVTT copy) or `.srt`
    (ffmpeg convert) preserving BCP-47 language tags. Companion videos
    additionally get the matching song's TTML/LRC/SRT/VTT/ASS sidecars
    mirrored alongside for media-player pickup. Diff before/after video
    file set so only freshly produced videos are processed.

  Adds 9 new unit tests (688 → 697 passing).

- **(subtitles)** Guarantee music video caption sidecars never overwrite

Harden the music video subtitle extraction naming so freshly produced
  captions can never overwrite:
  - a song's existing lyrics sidecar (`01 Title.srt` / `.vtt` / `.ttml`),
  - another music video caption track of the same language, or
  - any prior extraction.

  Changes to `music_video_subtitle_service`:
  - Extracted caption filename now embeds a `.cc.` marker and the stream
    index: `{stem}.cc.{index}[.{lang}].{ext}`. Keeps captions distinct
    from song lyrics and from each other.
  - New `resolve_non_clobbering_path()` disambiguates with `.1`, `.2`, ...
    if the computed path is somehow already taken (up to 100 attempts).
  - ffmpeg invocation switched from `-y` to `-n` so the downloader cannot
    silently overwrite an existing file even if our path resolution has
    a bug.
  - Lyrics pairing now guards with `same_file()` (canonicalised compare)
    so copying a file onto itself (identical stems in the same dir) is a
    true no-op rather than a potential truncation.

  Adds 7 new unit tests covering every guard (collision disambiguation,
  same-file detection, pair no-op when target exists, pair-copy when
  stems differ). Full suite: 688 → 695 passing.

- **(activity-log)** Split \r progress segments for companion downloads

Companion audio downloads and music-video downloads were emitting
  yt-dlp / N_m3u8DL-RE progress output to the activity log as single
  100KB+ lines because their stream readers used
  `AsyncBufReadExt::lines()` (splits on `\n` only) and did not then
  re-split on `\r`. yt-dlp uses `\r` to overwrite in place in a terminal,
  so a full download's progress arrived as one huge blob like
  `[download] 4.0% of ... (frag 0/18)[download] 3.2% of ...[download]`,
  rendering as an unreadable wall of text.

  Also, the music-video download helper used `wait_with_output()` which
  buffers the entire process output and never streams — so MV progress
  never reached the activity log or the progress bar in real time.

  Both issues are fixed:
  - New shared helper `emit_companion_stream_line()` mirrors the main
    GAMDL reader: splits on `\r`, strips ANSI, emits the last non-empty
    segment to `activity-log` in normal mode or every segment in verbose
    mode, and fires `gamdl-output` progress events for every segment so
    the progress bar stays live.
  - Companion audio reader (stdout + stderr) routed through the helper.
  - `download_music_video_by_url()` replaced `wait_with_output()` with
    spawned stdout/stderr reader tasks that also use the helper.

  Visible effect: music-video download progress now lands as individual
  scrollable rows with speed / ETA / percentage, matching the primary
  GAMDL reader's rendering.

- **(clippy)** Use checked_div for download progress (Rust 1.95)

Rust 1.95 shipped the new `clippy::manual_checked_ops` lint which
  flags `if x > 0 { (a * 100) / x }` patterns as preferring `checked_div`.
  CI (which installs the latest stable via `dtolnay/rust-toolchain@stable`)
  failed on this lint; local builds on 1.94 didn't see it yet.

  Rewrote the progress-log branch in `archive.rs` to use
  `downloaded.checked_mul(100).and_then(|n| n.checked_div(total_size))`,
  which produces the same percentage when `total_size > 0` and
  short-circuits otherwise.

- **(fs)** Collision-proof every rename/write path, not just MV subtitles

Extends the "never silently overwrite a different file" invariant —
  previously scoped to music-video subtitle sidecars — to every rename
  and write site in the app where different logical content could land
  on the same path.

  ## Why

- Music video filenames, Explicit/Clean suffix, collision-proof subtitles, activity-log progress splitting (#484)

## Summary

  - **fix(#481)**: music video companions were saving as `-.mp4`.
  `download_music_video_by_url()` now inherits the full set of
  filename/folder templates, tool paths, language, truncate, and
  download/remux modes so videos land with proper
  `{artist}/{album}/{title}` names.
  - **fix(#482)**: `[Explicit]` / `[Clean]` suffixes were applied
  inconsistently. `insert_advisory_before_codec_suffix()` now pulls the
  full codec suffix set from the registry (covers `[Binaural]`,
  `[Downmix]`, `[AAC Legacy]`, `[HE-AAC]` — not just the three hardcoded
  strings); `advisory_suffix()` + idempotency checks are case-insensitive;
  a new `apply_advisory_suffixes_from_tags()` pass runs in the completion
  task after companion downloads finish so late-landing companion files
  still pick up the suffix.
  - **feat(#483)**: music videos (direct or companion) now get subtitles /
  closed-captions extracted to sidecars. New
  `music_video_subtitle_service` probes the video with ffprobe, extracts
  each stream (`.vtt` copy for WebVTT, `.srt` convert for
  `mov_text`/`tx3g`/`eia_608`/etc.), mirrors matching song lyrics next to
  companion videos, and uses a **collision-proof** naming scheme
  (`{stem}.cc.{index}[.{lang}].{ext}`) plus belt-and-braces guards (`-n`
  on ffmpeg, canonicalised same-file compare on pairing, numeric
  disambiguation).
  - **fix(activity-log)**: companion audio downloads and music-video
  downloads were emitting yt-dlp / N_m3u8DL-RE `\r`-progress blobs as a
  single 100KB+ unreadable row. New shared helper
  `emit_companion_stream_line()` splits on `\r`, strips ANSI, coalesces to
  the last segment (or every segment in verbose mode), and drives
  `gamdl-output` progress events from every segment. Also switched
  `download_music_video_by_url()` from `wait_with_output()` to streaming
  readers so MV progress is live in the progress bar.
  - **chore(deps)**: `basic-ftp` 5.2.2 → 5.3.0 (patches
  GHSA-rp42-5vxx-qpwr to unblock `npm audit`); `tauri-plugin-deep-link`
  2.4.8 → 2.4.7 (2.4.8 was yanked — unblocks `cargo-deny`).

  ## Filename safety

  Naming is guaranteed not to overwrite:
  - song lyrics (`.ttml`/`.lrc`/`.srt`/`.vtt`/`.ass`) ← distinct from
  caption sidecars via `.cc.` marker
  - multi-track captions ← distinct by stream index
  - prior extractions ← idempotent (skip when target exists)
  - any file at all ← `resolve_non_clobbering_path()` + ffmpeg `-n` +
  `same_file()` pairing guard

  ## Tests

  - 16 new unit tests (advisory suffix cases, collision disambiguation,
  same-file detection, pair guards)
  - Full backend suite: **688 → 695 passing**, 0 failing
  - `cargo clippy --all-targets -- -D warnings` clean
  - `cargo-deny check` clean
  - `npm audit --audit-level=high` clean

  ## Test plan

  - [ ] Download an album that includes a track with a music video
  (music_video_companion enabled + MusicKit credentials) — verify the MV
  is named like `{artist} - {title}.mp4`, not `-.mp4`
  - [ ] Same album with a mix of Explicit + Clean tracks — verify every
  track file and the album folder carry the correct `[Explicit]`/`[Clean]`
  suffix regardless of codec
  - [ ] Companion codec (e.g. Atmos + ALAC) — verify the ALAC companion
  file also gets the advisory suffix after all downloads complete
  - [ ] Music video with embedded captions — verify
  `.cc.{idx}[.{lang}].vtt` / `.srt` sidecars appear alongside the video
  - [ ] Music video companion where the song's lyrics already exist in the
  album folder — verify lyrics are mirrored alongside the video and
  nothing is overwritten
  - [ ] Re-run the same download twice — verify no double-suffixing, no
  overwritten sidecars
  - [ ] Watch the Activity Log during a music-video download — verify
  progress arrives as individual scrollable rows (not one wall of text),
  and the progress bar shows live speed / ETA / percentage


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.4] - 2026-04-15

### 🐛 Bug Fixes

- False-positive tool update notifications due to version format m… (#479)

…ismatch

  The update checker compared installed tool versions (from `--version`
  output) against GitHub release tags using simple string inequality. This
  caused perpetual false "update available" for:

  - FFmpeg: installed "8.0.1" vs GitHub tag "latest" (BtbN uses rolling
  builds, not semver tags) — always different, always "update available"
  - N_m3u8DL-RE: installed "0.5.1" (digits only) vs GitHub tag
  "0.5.1-beta" (includes pre-release suffix) — never matched


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.3] - 2026-04-14

### 🐛 Bug Fixes

- Component Update All button now actually updates binary tools

The "Update All" button for component updates was silently succeeding
  without updating anything. Root cause: the 2 updates shown (FFmpeg,
  N_m3u8DL-RE) are binary tools with pip_package=null, so the pip-only
  filter excluded them — Promise.all([]) resolved as instant "success".

- False-positive tool update notifications due to version format mismatch

The update checker compared installed tool versions (from `--version`
  output) against GitHub release tags using simple string inequality.
  This caused perpetual false "update available" for:

  - FFmpeg: installed "8.0.1" vs GitHub tag "latest" (BtbN uses rolling
    builds, not semver tags) — always different, always "update available"
  - N_m3u8DL-RE: installed "0.5.1" (digits only) vs GitHub tag
    "0.5.1-beta" (includes pre-release suffix) — never matched

- Component Update All button now actually updates binary tools (#477)

The "Update All" button for component updates was silently succeeding
  without updating anything. Root cause: the 2 updates shown (FFmpeg,
  N_m3u8DL-RE) are binary tools with pip_package=null, so the pip-only
  filter excluded them — Promise.all([]) resolved as instant "success".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.2] - 2026-04-14

### 🐛 Bug Fixes

- Blank window rendering + broken component updates (#475)

## Summary

  - **Fix blank/black window on all platforms**: Remove terser
  `mangle.properties.regex: /^_/` which renamed React's internal
  underscore-prefixed properties (`._reactInternals`, `._payload`,
  `._init`, etc.), destroying the reconciler in production builds. Dev
  mode was unaffected because terser is disabled in debug builds.
  - **Fix "Update All" button doing nothing**: The frontend passed
  `e.name.toLowerCase()` (display name like "of-scraper") to
  `upgradePipEngine()` instead of the actual PyPI package name
  ("ofscraper"). Added `pip_package` field to `ComponentUpdate` so the
  backend passes the correct identifier through.

  ## Test plan

  - [ ] Build a release binary (`cargo tauri build`) and verify the app
  renders on macOS, Windows, and Linux
  - [ ] Verify dev mode (`cargo tauri dev`) still works
  - [ ] Navigate to Updates page, confirm "Update All" triggers pip
  upgrades with correct package names
  - [ ] Run `npm run type-check` — passes
  - [ ] Run `npm test` — 293/293 tests pass


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.1] - 2026-04-14

### 🐛 Bug Fixes

- Remove terser property mangling that breaks React rendering

The `mangle.properties.regex: /^_/` terser option renames ALL object
  properties starting with underscore across the entire production bundle.
  This destroys React's internal state management — React DOM uses 18+
  underscore-prefixed properties (._reactInternals, ._internalRoot,
  ._payload, ._init, ._currentValue, etc.) that must retain their exact
  names for the reconciler to function.

- Component updates fail — frontend uses display name instead of pip package name

The "Update All" button on the Updates page called `upgradePipEngine(e.name.toLowerCase())`
  which passes the human-readable display name (e.g., "OF-Scraper" → "of-scraper") instead
  of the PyPI package name ("ofscraper"). This caused pip to fail for any engine where the
  display name differs from the package name.

- Loads with blank screen (#473)

The `mangle.properties.regex: /^_/` terser option renames ALL object
  properties starting with underscore across the entire production bundle.
  This destroys React's internal state management — React DOM uses 18+
  underscore-prefixed properties (._reactInternals, ._internalRoot,
  ._payload, ._init, ._currentValue, etc.) that must retain their exact
  names for the reconciler to function.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.34.0] - 2026-04-14

### ✨ Features

- Download manifest, folder scan, enrichment fixes, and new features (#468)

## Summary

  Large feature branch spanning critical bug fixes, new features, and
  infrastructure improvements across 105+ files.

  ### Critical Fixes
  - **Metadata cross-contamination defence** (#452) — targeted
  `find_album_directory()` with artist/album hints, depth-limited file
  collection, album/artist validation before writing API metadata
  - **Serial queue processing** (#455) — entire pipeline (download →
  companions → enrichment → lyrics → manifest) completes before next item
  starts; 10-minute completion timeout (#461)
  - **Platform icon not rendering** — CSP `connect-src` was missing
  `'self'`, blocking SVG fetch; replaced external Google Favicon fallback
  with local `<img>`

  ### New Features
  - **Download manifest rename** (#447) — `.meedyadl` →
  `manifest.meedyadl` with auto-migration
  - **Folder scan** (#456) — recursive manifest discovery for
  re-downloading with quality upgrade (#380) and content refresh detection
  - **Dual API metadata enrichment** (#454) — iTunes Lookup API (Step 0)
  then Apple Music Catalog API (Step 1)
  - **MetadataProvider trait** (#351) — service-agnostic enrichment
  interface with priority registry
  - **BPM onset detection** (#418) — FFmpeg `silencedetect` filter for
  audio analysis
  - **Tool version tracking** (#273) — `--version` parsing + auto-update
  checking via GitHub API
  - **Functional tool verification** (#391) — startup binary health checks
  with 2s timeout
  - **Rollback UI for pre-release users** (#267) — downgrade to stable via
  in-app updater
  - **Browse button in setup wizard** (#278) — locate existing tool
  binaries via native file picker
  - **Cover art naming** (#448) — configurable cover art filename
  (FrontCover/Cover/Folder)
  - **Platform template variable** (#309) — `{platform}` in
  filename/folder templates
  - **ReplayGain video containers** (#329) — MKV/WebM/OGV support via
  lofty
  - **Service status** — remote kill-switch system with fail-open design
  - **Smart Download** — cross-platform quality search infrastructure
  (Phase 1)

  ### Security & Validation
  - **Input validation** (#459) — path traversal rejection, URL domain
  validation, `0o600` file permissions
  - **MusicKit JWT fix** (#161) — missing `aud` claim + validation logic

  ### Testing & Docs
  - **Frontend tests** (#232, #460) — Vitest component tests,
  multi-service URL detection, unit tests
  - **ARIA accessibility** (#182) — screen reader support on DownloadForm
  + StatusBar
  - **meedya-core migration docs** (#352, #353) — CodecDetector and
  Fingerprinter extraction plans

  ## Test plan
  - [x] TypeScript type-check passes (`npm run type-check`)
  - [x] All 293 Vitest tests pass (`npm run test`)
  - [ ] Verify platform icon renders in progress bar during active
  download
  - [ ] Test folder scan discovers manifest files and allows re-download
  - [ ] Verify serial queue processing — second item waits for first to
  fully complete
  - [ ] Confirm `manifest.meedyadl` created after download (not hidden
  `.meedyadl`)


### 🐛 Bug Fixes

- Address review feedback — abort on timeout, exact URL host validation, tool ID/version fixes, error handling
- Metadata_provider async trait + ComponentUpdate missing field

1. metadata_provider.rs: Remove `async_trait` crate dependency (not in
     Cargo.toml). Replace `async fn` in trait with pinned boxed future
     return type for dyn-compatibility (E0038).

  2. update_checker.rs: Remove nonexistent `compatibility_note` field
     from ComponentUpdate struct initialization (E0560).

- Add missing ComponentUpdate fields in tool version check

The ComponentUpdate struct requires description, release_url,
  release_body, is_prerelease, and tag_name fields — these were
  missing from the check_github_tool_update constructor.

- Clippy errors — orphaned doc comment + dead code

1. metadata_tag_service.rs: Remove orphaned doc comment before section
     header that triggered empty_line_after_doc_comments lint.

  2. download_queue.rs: Remove unused has_audio_files() function (dead
     code — superseded by has_direct_audio_files() from #452).

- Rename BbcIPlayer → BBCiPlayer + fix doc_lazy_continuation lints

1. Rename MediaServiceId::BbcIPlayer to BBCiPlayer across the codebase
     to match BBC branding (7 files).

  2. Fix 5 doc_lazy_continuation clippy warnings where /// doc comments
     were immediately followed by // regular comments without a blank
     /// separator line (lib.rs, login_window_service.rs, download_queue.rs).

- Use blank lines (not ///) to separate doc comments from regular comments

Previous fix incorrectly used blank /// lines as separators — the
  doc_lazy_continuation lint requires a completely blank line (no
  comment marker at all) between /// doc comments and // comments.

- Remove accidentally committed test binary
- Remove blank lines between /// doc comments and // comments

The empty_line_after_doc_comments lint fires when there is ANY blank
  line between a /// doc comment and the function it documents — even
  if regular // comments and #[allow] attributes sit between them.

- Update tests for video container ReplayGain + case-insensitive dirs

1. replaygain_service.rs: .mkv/.webm now correctly return
     Some(VorbisComment) after #329 added video container support.
     Split into detect_unsupported (txt, png → None) and
     detect_video_containers (mkv, webm, ogv → VorbisComment).

  2. download_queue.rs: find_album_directory_case_insensitive test now
     compares paths case-insensitively — macOS has a case-insensitive
     filesystem so the original-cased path is returned directly.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- Trigger fresh CI run after doc comment fixes

### Merge

- Integrate main v0.33.0 release

Pick up version bump to 0.33.0 and Cargo.lock update from
  release-please PR #470.


## [0.33.0] - 2026-04-13

### ✨ Features

- Dual API metadata enrichment — Apple Music + iTunes Lookup (#454)

Add iTunes Search/Lookup API as a supplementary metadata source alongside
  the existing Apple Music Catalog API. The iTunes API is public (no auth
  required) and provides fields the Apple Music API doesn't:

  - Track/album price and currency (TrackPrice, CollectionPrice, Currency)
  - Release country (Country)
  - Disc count (DiscCount)
  - iTunes track URLs (iTunesTrackURL)

- Re-download from folder scan via manifest discovery (#456)

Added `scan_folder_for_manifests` IPC command that opens a native folder
  picker and recursively scans the selected directory for `manifest.meedyadl`
  (and legacy `.meedyadl`) files. For each discovered manifest, extracts:
  - Source URLs for re-downloading
  - Artist/album names (inferred from GAMDL's Artist/Album/ dir structure)
  - Platform, download date, track count

- Functional tool verification at startup (#391)

check_all_dependencies() now runs each tool binary with --version
  (2-second timeout) instead of just checking file existence. This
  detects corrupted or non-executable binaries before showing "Ready".

  If the binary exists but fails to spawn (not executable), it's
  reported as not installed. If it spawns but --version fails (some
  tools use different flags), it's still reported as installed since
  the binary is functional.

- Quality upgrade detection in folder scan (#380)

ScannedManifest now includes `current_codec` and `audio_file_count`
  fields. When scanning a folder for manifests, the codec is detected
  from the first M4A file's MeedyaMeta:SourceCodec or isLossless tag.

  This lets the frontend show upgrade opportunities: e.g., "Currently
  AAC — ALAC/Atmos available" when the user's current codec is lower
  quality than what Apple Music offers.

  detect_album_codec() helper reads mp4ameta tags without spawning any
  subprocess — fast and lightweight for scanning large libraries.

- Auto-update checking for external tools via GitHub API (#273)

Added check_github_tool_update() that queries GitHub Releases API for
  the latest version of external tools (FFmpeg, N_m3u8DL-RE). Integrated
  into check_all_updates() — runs for installed tools only.

  Currently reports latest available version without comparison (exact
  installed version not tracked). Foundation for future "Tool Updates
  Available" UI indicator.

- Browse button in setup wizard for custom tool paths (#278)

Added "Browse" button next to "Install" for each missing tool in the
  setup wizard's Dependencies step. Opens a native file picker to locate
  an existing binary on the system. The selected path is stored in
  settings (e.g., ffmpeg_path, mp4decrypt_path) so the app uses the
  user's existing installation instead of downloading a new one.

- Add {platform} template variable for download platform name (#309)

Added 'platform' to TEMPLATE_VARIABLES and SAMPLE_METADATA in
  template-parser.ts. Renders as the download platform name (e.g.,
  "AppleMusic", "Spotify") in file/folder name templates. Enables
  organizing downloads by service: {platform}/{album_artist}/{album}/

- Stable rollback option when running pre-release (#267)

UpdateCheckResult now includes `rollback_version` and `rollback_tag`
  fields. When the current app version is a pre-release (contains '-'),
  check_all_updates() fetches the latest stable release from GitHub's
  releases/latest endpoint and populates these fields.

  The frontend can use these to show a "Rollback to stable vX.Y.Z"
  option when the user is running a beta/RC. The Tauri updater can
  then be pointed to the stable release's tag.

- BPM detection foundation with FFmpeg audio analysis (#418)

Replaced placeholder "analysis deferred" comment with actual FFmpeg
  invocation for audio pre-processing (resample to 22050Hz, bandpass
  40-300Hz for rhythm detection). Added parse_ffmpeg_duration() helper.

  Full BPM onset detection via aubio/essentia is deferred to meedya-core
  integration, but the FFmpeg pipeline is now in place. Musical key
  detection requires Python's essentia library (no Rust equivalent).

- ReplayGain support for MKV/WebM/OGV video containers (#329)

Added mkv, webm, and ogv to SUPPORTED_EXTENSIONS in replaygain_service.
  These containers support Vorbis Comment tags (same as FLAC/OGG/Opus),
  so the existing VorbisComment write path handles them via lofty.

  FFmpeg ebur128 analysis already supports any decodable audio stream,
  so the analysis side was already working — only the extension whitelist
  needed updating.

- Rollback UI for pre-release users (#267)

Added rollback_version and rollback_tag to UpdateCheckResult TypeScript
  type. When running a pre-release, the Updates page shows a card:
  "Running pre-release — stable vX.Y.Z available" with a "View Stable
  Release" button that opens the GitHub release page.

  Backend (commit ec8a249) already populates these fields via
  fetch_latest_stable_release(). This commit adds the frontend UI.

- Tool version tracking via --version for update comparison (#273)

check_github_tool_update() now runs the installed binary with --version
  to detect the current version, then compares against the latest GitHub
  release tag. Reports update_available when versions differ.

  Version extraction parses the first whitespace-delimited token containing
  a dot and digits from the combined stdout+stderr output.

- Content refresh detection via lastModifiedDate in folder scan (#380)

ScannedManifest now includes last_modified_date from the manifest's
  ManifestSource. This enables the frontend to compare the stored date
  against a fresh API response to detect content refreshes: mix
  corrections, remastered audio, added tracks, Apple Digital Master
  certification, metadata updates.

  Combined with current_codec detection (commit 1f526e2), the folder
  scan now provides complete re-download opportunity data: quality
  upgrades (AAC→ALAC) AND content refreshes (same codec, newer content).

- MetadataProvider trait and registry for multi-service enrichment (#351)

Defined the service-agnostic MetadataProvider async trait with:
  - provider_name(), service_id(), priority(), requires_auth()
  - fetch_album_metadata(album_id, storefront) -> Option<AlbumMetadata>

  MetadataProviderRegistry stores providers sorted by ProviderPriority:
  - Supplementary (iTunes) → Primary (Apple Music) → UserOverride

  This trait enables the enrichment pipeline to call multiple providers
  through a common interface. Concrete implementations (AppleMusicProvider,
  ItunesProvider) will be added as the services are migrated to use the
  trait instead of direct function calls.

- BPM onset detection via FFmpeg silencedetect filter (#418)

Replaced placeholder analysis with actual onset-based BPM detection:
  - FFmpeg silencedetect filter (noise=-30dB, duration=0.1s) identifies
    quiet gaps between beats in the low-frequency band (40-300Hz)
  - Counts silence_end markers as onset events
  - BPM = (onset_count / duration) * 60
  - Auto-corrects via halving/doubling if outside 60-200 BPM range
  - Only returns plausible results (60-200 BPM range)

  This is an approximation — for production quality, aubio/essentia
  via meedya-core (#353) will be needed. But this provides reasonable
  BPM estimates for most 4/4 time music without external dependencies.

  Musical key detection still requires Python essentia library.

- Extract all FFmpeg components and rename MeedyaDL org to MeedyaSuite (#444, #445)

- Expand companion FFmpeg extraction to include ffplay alongside ffprobe,
    ensuring the full FFmpeg suite (ffmpeg, ffprobe, ffplay) is available
    in the managed tools directory on all platforms
  - Rename all references from MeedyaDL/MeedyaDL-Tools org to
    MeedyaSuite/MeedyaDL-Tools across tool-versions.toml, deny.toml,
    engines.toml, release.yml, dependency_manager.rs, and documentation
  - CHANGELOG.md historical entries left unchanged


### 🐛 Bug Fixes

- Manifest visibility, cover art naming, and animated artwork defaults (#447, #448, #449)

- Rename manifest file from `.meedyadl` to `manifest.meedyadl` so it is
    visible in file managers on macOS/Linux (was hidden as a dotfile).
    Auto-migrates legacy `.meedyadl` files on write. Add activity log entry
    when manifest is saved. (#447)

  - Add post-download rename of GAMDL's `Cover.<ext>` to `FrontCover.<ext>`
    for consistency with animated artwork naming (`FrontCover.mp4`,
    `PortraitCover.mp4`). Applied in both primary enrichment and companion
    download paths. (#448)

  - Change `animated_artwork_enabled` default from `false` to `true` and
    `hide_animated_artwork` default from `true` to `false` so animated
    cover art is downloaded and visible by default. Upgrade skip-reason
    logging from DEBUG to INFO for better diagnostics. (#449)

- Album directory resolution, configurable cover names, lyrics settings (#447, #448, #449, #450, #451)

Root cause fix: `find_album_directory()` only searched one level deep,
  returning the artist directory instead of the album directory for
  GAMDL's Artist/Album/ structure. This caused manifests, animated artwork,
  and all enrichment to target the wrong directory. Replaced with recursive
  `find_deepest_audio_dir()` that finds the deepest directory directly
  containing audio files. Added `has_direct_audio_files()` non-recursive
  check alongside existing recursive `has_audio_files()`. (#450)

  Manifest temp file fix: replaced `Path::with_extension()` (which
  produced `.meedyadl.meedyadl.tmp` on dotfiles) with explicit
  `dir.join("manifest.meedyadl.tmp")`. (#447)

  Configurable cover art naming: added `CoverArtName` enum (FrontCover,
  Cover, Folder) with `cover_art_name` setting (default: FrontCover).
  UI dropdown in Settings > Cover Art. `rename_cover_art()` respects the
  user's choice. (#448)

  Lyrics settings restructure: split "Embed Lyrics and Keep Sidecar" into
  two controls — "Embed Lyrics / Captions" master toggle + conditional
  "Keep Sidecar Lyrics / Caption Files" sub-toggle. Added
  `keep_lyrics_sidecar` setting (default: true). (#451)

- CRITICAL metadata cross-contamination between concurrent downloads (#452)

Three-layer defence against metadata from one download being written to
  another download's files:

  1. **Depth-limited file collection**: `collect_m4a_files()` now uses
     `collect_m4a_depth_limited()` with max_depth=1, preventing recursive
     collection into sibling album directories when the enrichment path
     resolves to an artist directory.

  2. **Album-name validation**: Before writing Apple Music API metadata
     (Layer 4), `enrich_single_file()` now compares the file's embedded
     album name against the API response. Mismatches are logged and skipped,
     preventing cross-contamination even if file collection includes files
     from a different album.

  3. **Genre deduplication**: Track genres are now merged with existing
     `©gen` atom values (from GAMDL) and deduplicated case-insensitively.
     The generic "Music" entry is filtered out. Result written to standard
     `©gen`, freeform `Genre`, and `MeedyaMeta:AppleGenres` atoms.

  Also added activity log entry when animated artwork is disabled in
  settings, so users can see why artwork wasn't downloaded.

- Artist promo video not downloading to artist folders (#453)

- Add compilation album skip: checks `is_compilation` from Apple Music
    API and skips promo video download for "Various Artists" compilations.
    Activity log shows "Artist promo video skipped (compilation album)".

  - Add activity log entries for all skip reasons: disabled in settings,
    no artist_id in metadata, compilation album, no MusicKit credentials.
    Previously these were silent `log::debug!()` calls only.

  - Change `artist_promo_video_enabled` default from `false` to `true`
    (consistent with animated artwork #449). Feature gracefully skips
    when credentials are missing or no promo video exists.

  - Upgrade `log::debug!()` to `log::info!()` in
    `download_artist_promo_video()` for skip reasons (already exists,
    no MusicKit token).

  The primary fix for directory resolution was already applied in #450 —
  `find_album_directory()` now returns the album directory, so
  `parent()` correctly derives the artist directory for ArtistCover.mp4.

- CRITICAL cross-contamination — targeted directory search + strict validation (#452)

Root cause: `find_album_directory()` used most-recently-modified timestamp
  to select the album directory from the base output path. With concurrent
  downloads, this returned the WRONG artist's directory (e.g., Michael W.
  Smith's dir for Blue's enrichment) because the later download had newer
  file timestamps.

  Primary fix: `find_album_directory()` now accepts `artist_hint` and
  `album_hint` parameters (from the early metadata fetch). It first
  attempts a targeted path match (`base_dir/Artist/Album/`) with
  case-insensitive fallback before falling back to the timestamp scan.
  This ensures each download's enrichment targets the correct directory.

  Secondary fix: Tightened album-name validation guard in
  `enrich_single_file()`. When the file has no album tag but DOES have
  an artist tag, the artist name is compared against the API response.
  Previously the fallback was unconditionally `true`, allowing any
  metadata to be applied to files with missing album tags.

  Also added `find_directory_case_insensitive()` helper for filesystem
  name matching that handles slight differences between API naming and
  GAMDL's filesystem-safe naming.

- Serial queue processing, reorder iTunes-first, rename ArtistSpotlightCover (#455, #454)

Serial queue processing (#455): Moved `process_queue()` cascade from
  the main download task (which ran immediately after GAMDL exited) INTO
  the completion task (which waits for enrichment + companions to finish).
  The next queued item now starts only after the current item's full
  pipeline completes: download → enrichment → companions → lyrics → done.
  Added cascade calls to error and cancellation paths too.

  This eliminates the root cause of metadata cross-contamination (#452)
  where concurrent enrichment tasks could target wrong directories due
  to race conditions in timestamp-based directory detection.

  iTunes-first ordering (#454): Moved iTunes Lookup API call to Step 0
  (before Apple Music API enrichment). iTunes runs first (no auth needed),
  Apple Music overwrites with richer data. This ensures Apple Music data
  takes priority for any overlapping fields.

  Removed price/currency from file metadata: TrackPrice, CollectionPrice,
  and Currency are locale-dependent and change over time, so they are
  excluded from file tags. Country and DiscCount are still written.

  Renamed ArtistCover.mp4 → ArtistSpotlightCover.mp4 across all files
  for clearer naming of the artist spotlight/promo video.

- Enrichment pipeline status reporting when API metadata unavailable (#458)

Audit found all .unwrap() calls are safely in test code only — production
  code uses proper error handling (unwrap_or, ok_or?, match, if-let).

  Fixed misleading activity log: "Metadata enrichment completed" now shows
  a warning icon when API metadata is unavailable, informing users that
  downstream enrichment steps (artwork, manifest) may be limited.

- Completion task timeout prevents stuck Processing state (#461)

Added 10-minute timeout to enrichment and companion download awaits in
  the completion task. If either hangs (deadlock, unresponsive API, slow
  network), the item is forcibly marked complete with a warning in the
  activity log, and the queue cascades to the next item.

  Without this timeout, a hung enrichment task would block the entire
  queue indefinitely (since #455 made processing serial).

- Clear processing label and speed/ETA on completion (#416)

set_complete() now clears processing_label, speed, and eta fields when
  transitioning to Complete state. This prevents stale "Processing..."
  text and speed indicators from lingering in the queue UI after a
  download finishes.

  Combined with the completion task timeout (#461) which prevents
  indefinite stalls, this addresses the stuck Processing state bug.

- MusicKit JWT missing aud claim + validation logic (#161)

Root cause: The MusicKit JWT was missing the required `aud` (audience)
  claim. Apple's API requires `"aud": "https://music.apple.com"` in the
  JWT claims — without it, valid credentials get rejected with HTTP 401.

  Also fixed validation logic: changed `.all(|s| s == 401)` to
  `.any(|s| s == 401)` so a 401 from ANY host is reported as an auth
  failure. Previously, if one host returned 401 and the other had a
  network error, the error fell through to a generic "unexpected response"
  message.

- Platform icon not rendering — CSP blocked fetch + fallback

Root cause: `connect-src` in CSP was `ipc: http://ipc.localhost` without
  `'self'`, so `fetch('/icons/platforms/apple-music.svg')` was blocked by
  the Content Security Policy. When the fetch failed, the Google Favicon
  fallback <img> was also blocked because `img-src` didn't include
  `https://www.google.com`. Result: broken image outline.

- Resolve ffprobe missing, activity log text overlap, and progress bar icon (#441, #442, #443)

- Install companion ffprobe alongside ffmpeg during dependency setup:
    search extracted archives for ffprobe (Linux/Windows BtbN), download
    separately from evermeet.cx (macOS), and copy from system PATH
  - Add dynamic row height measurement to Activity Log virtualizer so
    wrapped multi-line entries no longer overlap subsequent rows
  - Add PlatformIcon to the queue-level progress bar caption so the
    service icon displays in both progress bars

- Update remaining MeedyaDL org references to MeedyaSuite (#445)

Update CHANGELOG.md and help/faq.md references from the old
  MeedyaDL GitHub org name to MeedyaSuite, completing the org rename
  across all files — no stale references remain.

- Register 9 missing module declarations for cargo check

Added missing `pub mod` declarations that caused cargo check to fail
  with exit code 101 on CI (ubuntu-latest, macos-latest):

  - services/mod.rs: +service_status, +smart_download
  - models/mod.rs: +content_match, +service_status, +votify_options,
    +ytdlp_options, +get_iplayer_options
  - commands/mod.rs: +service_status, +smart_download
  - lib.rs: register check_service_status, check_cross_platform in
    generate_handler!

  The .rs files existed on disk but were never declared in their parent
  mod.rs files, so rustc couldn't find them.

- Compile errors — wrong enum variant, missing match arms, nonexistent field

- MediaServiceId::BBCiPlayer → BbcIPlayer (3 files)
  - Add missing YouTubeMusic arm to all match expressions on MediaServiceId
  - Remove reference to nonexistent SpotifySettings.audio_quality field
    (stub service — quality settings not yet implemented)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Document meedya-core CodecDetector migration path (#352)

Added documentation block in mediainfo_service.rs describing the planned
  migration to meedya-core's CodecDetector trait. Current implementation
  (MediaInfo CLI + ffprobe fallback) is stable and performant — migration
  is low priority. The trait abstraction would enable alternative backends
  (symphonia, GStreamer) without changing the service interface.

- Document meedya-core Fingerprinter migration path (#353)

Added documentation block in acoustid_service.rs describing the planned
  migration to meedya-core's Fingerprinter trait. Current rusty-chromaprint
  + Symphonia implementation is stable — migration enables alternative
  backends (essentia for musical key detection). Low priority.

- Update CLAUDE.md with recent feature and fix context

Reflects changes from issues #161, #182, #232, #267, #273, #278, #309,
  #329, #351, #352, #353, #380, #391, #416, #447, #448, #452, #453, #454,
  #455, #456, #459, #460, #461.

  Key additions: download manifest rename + folder scan, targeted album
  directory resolution, metadata cross-contamination defence, serial queue
  processing, dual API enrichment, MetadataProvider trait, cover art
  naming, input validation, tool version tracking, functional tool
  verification, rollback UI, platform template variable, ARIA a11y,
  frontend tests, meedya-core migration docs.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Add unit tests for find_album_directory, rename_cover_art, validate_path_safe (#460)

Added 13 unit tests covering the new functions from this session:

  - find_album_directory: targeted match, case-insensitive, fallback,
    returns deepest directory (4 tests)
  - has_direct_audio_files: detects M4A, ignores nested, empty dir (3 tests)
  - rename_cover_art: renames jpg, skips when target is Cover, idempotent
    (3 tests)
  - validate_path_safe: allows normal paths, rejects traversal (3 tests)

  Total download_queue.rs tests: 103 (was 90).

- Add multi-service URL detection tests (#232)

Added 15 frontend tests for the multi-service URL parser:

  - detectService: Apple Music, Classical, iTunes, YouTube Music,
    YouTube, Spotify, BBC iPlayer, unknown, case-insensitive (9 tests)
  - isSupportedUrl: supported + unsupported (2 tests)
  - parseMediaUrl: Apple Music with content type, Spotify without content
    type, unknown URL, whitespace trimming (4 tests)

  Total frontend tests: 287 (was 272).

- Frontend component tests for activity and download stores (#232)

Added 6 tests:
  - Activity store: filter by download_id, filter by stream type,
    search by line content, serial pipeline entries distinguishable (4)
  - Download store: queue state tracking by state, queue snapshot
    replacement (2)

  Total frontend tests: 293 (was 287).


### A11y

- Add ARIA attributes for screen reader support (#182)

- DownloadForm: aria-label + aria-describedby on URL textarea
  - StatusBar: role="status" + aria-live="polite" + aria-label for
    screen readers to announce download status changes

  55 ARIA attributes were already in place across components. These
  additions cover the two most interactive areas: URL input and
  download status. Manual VoiceOver/NVDA testing still needed.


### Merge

- Resolve conflict with main (org rename + feature updates)

Conflict in CLAUDE.md resolved by combining:
  - Our branch: ArtistSpotlightCover.mp4 rename (#455), artist folder
    resolution fix (#453)
  - Main: MeedyaSuite org rename (#445) in dependency manager mirror path


### Security

- Input validation — path traversal, URL domain, file permissions (#459)

1. Path traversal: Added `validate_path_safe()` in config_service.rs
     that rejects paths containing `..` components. Applied to output_path
     in merge_options() before passing to GAMDL subprocess.

  2. URL domain validation: start_download() now rejects URLs not matching
     supported domains (music.apple.com, classical.apple.com,
     itunes.apple.com) before normalization. Prevents arbitrary URLs from
     reaching GAMDL.

  3. File permissions: settings.json now set to 0o600 (owner read/write
     only) on Unix after write. Settings contain sensitive data (cookies
     path, API credentials, wrapper URL).


## [0.32.1] - 2026-04-11

### 🐛 Bug Fixes

- Progress bar icon, caption format, and activity log context (#427, #428, #429)

Three fixes for the GlobalProgressBar and activity log:

  1. Service icon not displaying (#427): The platform config loaded
     asynchronously via IPC but stored in module-level variables without
     triggering a React re-render. Added a subscriber pattern so the
     component re-renders when config arrives.

  2. Caption format (#428): Changed from "Artist — Track" to
     "DOWNLOADING...Artist — Album — Track" with a state prefix.
     Added early Apple Music API metadata fetch at download start
     (before GAMDL subprocess) so artist_name and album_name are
     available from the first track.

  3. Activity log context (#429): Track separator now includes artist
     and album context from the queue item, matching the progress bar
     format: "[Track N/M] Downloading Artist — Album — "Track"".

  Also made try_fetch_metadata() public in metadata_tag_service.rs
  so it can be reused for the early fetch.

- Verbose mode now bypasses \r coalescing in activity log (#435)

The stdout/stderr readers in download_queue.rs coalesce \r-separated
  progress lines, emitting only the last segment per newline-delimited
  output. This reduces event volume 5-10x but eliminates all intermediate
  download progress from the Activity Log (speeds, ETAs, percentages).

  When verbose logging is enabled (Settings > Advanced > Verbose Activity
  Log), the coalescing and dedup filters are now bypassed — ALL GAMDL
  output segments are emitted to the activity log. This gives users
  complete progress history for debugging slow downloads, stalls, or
  rate limiting issues.

  Normal mode (verbose off) retains the compact coalesced view.

- Context menu on inputs, responsive content width, native notifications (#436, #437, #438)

- Right-click on input/textarea/select now shows native paste menu instead
    of the After-Queue context menu; blank-space right-click still works
  - Removed fixed max-w constraints from Download, Help, and Updates pages
    so content fills available width responsively on window resize
  - Added notification_style setting (in_app_only / native_and_in_app /
    native_only) with native OS notification routing in addToast(); UI
    control added in Settings > General > Preferences

- Resolve npm audit vulnerability and add notification_style validation

- Update basic-ftp transitive dependency to fix GHSA-6v7q-wjvx-w8wg
    (high severity CRLF injection in FTP credentials)
  - Add notification_style enum validation in sanitize_imported_settings()
    to prevent arbitrary string injection via crafted settings imports

- Resolve clippy doc_lazy_continuation warning in download_queue

Add blank doc-comment line between URL normalization examples list and
  the extract_album_info_from_url doc block. Clippy 1.94 flags consecutive
  doc-comment paragraphs after list items as lazy continuations without
  proper indentation.

- Add missing album_name and artist_name fields to test structs

The QueueItemStatus struct gained album_name and artist_name fields in
  commit cc8c0e1, but the three serde roundtrip tests in download.rs were
  not updated to include these required fields, causing cargo test to fail
  with E0063.

- Companion lyrics conversion finds album dirs recursively (#439)

The companion lyrics conversion (TTML → LRC/SRT/VTT/ASS) was only
  scanning the top-level output path with non-recursive read_dir().
  Since GAMDL creates Artist/Album/ subdirectories, the TTML files
  were never found for companion tiers, resulting in missing sidecar
  files for suffixed companions (e.g., [Lossless] variants).

  Adds find_dirs_with_ttml() helper that recursively walks the output
  directory tree to locate all directories containing .ttml files,
  then runs all four conversion services on each discovered directory.

- Progress bar, activity log, companion lyrics, CI fixes, and docs update (#434)

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Comprehensive help and project documentation update

- Create help/supported-services.md with current and planned service
    details (Apple Music, Spotify, YouTube, BBC iPlayer)
  - Fix GitHub repo URL in help/index.md
  - Update Project_Plan.md with multi-service groundwork status
  - Close 7 already-implemented GitHub issues
  - Create 5 GitHub milestones (v1.0 RC, v1.0 GA, v2.0, v2.1, v2.2)
  - Assign 23 open issues to milestones

- Update CHANGELOG.md [skip ci]
- Reorder service milestones — BBC iPlayer (M8) before Spotify (M10)

Milestone order updated:
  - M8 v2.0.0: BBC iPlayer (was Spotify)
  - M9 v2.1.0: YouTube (unchanged)
  - M10 v2.2.0: Spotify (was BBC iPlayer)

  Updated in: README.md, CLAUDE.md, Project_Plan.md
  GitHub milestones renamed and issues reassigned accordingly.

- Update CHANGELOG.md [skip ci]
- Swap Spotify and YouTube milestone order

New milestone order:
  - M8 v2.0.0: BBC iPlayer (get_iplayer / yt-dlp)
  - M9 v2.1.0: Spotify (votify)
  - M10 v2.2.0: YouTube (yt-dlp)

  Updated in: README.md, CLAUDE.md, Project_Plan.md
  GitHub milestones renamed and issues reassigned.

- Update CHANGELOG.md [skip ci]
- Comprehensive documentation update reflecting current project state

- CLAUDE.md: add notification_style setting, album/artist context in progress
    bars, companion lyrics recursive fix, updated services/models lists, verbose
    \r bypass, workflow count
  - Project_Plan.md: add v0.32.0 features (album context, notification style,
    service icon, companion lyrics fix), update cross-cutting architecture status
    (multi-service queue, engine registry, per-service settings now complete),
    mark download history as complete, update date
  - README.md: add v0.32.0 roadmap items (album context, notification style,
    download history, companion lyrics fix)
  - DEV_NOTES.md: update project structure with all 32 services, 15 models,
    13 commands, 5 hooks, 5 utils, 7 workflows
  - help/downloading-music.md: add companion lyrics and activity log tracking
  - help/lyrics-and-metadata.md: add companion lyrics subsection
  - help/quality-settings.md: add download notifications section

  GitHub Issues closed: #370 (activity log memory leak), #411 (CONTRIBUTING.md),
  #412 (CODE_OF_CONDUCT.md), #439 (companion lyrics)

- Update CHANGELOG.md [skip ci]

## [0.32.0] - 2026-04-10

### ✨ Features

- Show album context in progress bar during downloads

Parse the album name from the Apple Music URL slug and display it
  alongside the track title: "The Platinum Collection — \"Black Boy Run\""
  instead of just "Black Boy Run". Helps identify which album is being
  downloaded in multi-queue sessions.

  Falls back to track name only if album can't be extracted from URL.
  Processing labels (enrichment/companions) are unaffected.

- Show artist and album context in progress bar and queue

Add album_name and artist_name fields to QueueItemStatus:
  - album_name: extracted from Apple Music URL slug at enqueue time
    (e.g., /album/the-platinum-collection/123 → "The Platinum Collection")
  - artist_name: populated from Apple Music API during enrichment Step 1
    (AlbumMetadata.artist_name)

  Progress bar now shows: "Artist — Album — \"Track Title\""
  instead of just "Track Title", giving clear context in multi-queue
  sessions.

  The extract_album_info_from_url() helper also handles artist URLs.

- Album context in activity log entries and processing labels (#422)

All enrichment step markers now include album context:
  - "▶ Metadata enrichment started — Blue: The Platinum Collection"
  - "✓ ReplayGain analysis completed — Blue: The Platinum Collection"

  Processing labels (progress bar) also auto-append context:
  - "Enriching metadata tags... — Blue: The Platinum Collection"
  - "AcoustID fingerprinting... — Blue: The Platinum Collection"

  The set_label() closure reads artist_name/album_name from the queue
  item and appends " — Artist: Album" to every label. The album_context()
  helper does the same for emit_download_log() calls.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.31.1] - 2026-04-10

### 🐛 Bug Fixes

- Clippy empty_line_after_doc_comments — reorder doc comment blocks

Move send_desktop_notification() doc comment to directly above the
  function, after the NOTIFICATION_THROTTLE static. The previous layout
  had the function's doc comment above the static with a blank line,
  triggering clippy::empty_line_after_doc_comments.

- **(ci)** Allow pre-existing rustdoc link warnings in CI

Change RUSTDOCFLAGS from "-D warnings" to "-A rustdoc::all" for the
  cargo doc CI step. Pre-existing doc comments use backtick-wrapped words
  (Explicit, Clean, Lossless, etc.) that rustdoc interprets as unresolved
  item links. These will be cleaned up incrementally.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.31.0] - 2026-04-10

### ✨ Features

- Activity log readability + after-queue actions backend

Activity log task markers:
  - Add ▶ started / ✓ completed markers for all 6 major enrichment stages
    (metadata, lyrics, artwork, AcoustID, ReplayGain, music video)
  - Provides clear timing visibility and stall/freeze detection

  Fix temp_path for companion/video downloads (#417):
  - Companion download options (music video, lyrics fallback) passed empty
    temp_path directly from settings instead of resolving to OS temp dir
  - On macOS from /Applications, empty temp_path = unwritable CWD
  - Now uses same resolution logic as merge_options(): empty → {OS temp}/MeedyaDL

- Notification tier system with configurable auto-dismiss (#385)
- Add What's New section to version upgrade modal (#387)

The pre-release notice modal now includes a "What's New" section
  with a link to the full release notes on the Updates page. This
  gives users a quick changelog summary when they upgrade to a new
  version.

- Keyboard shortcut hints on sidebar nav tooltips (#388)

Add shortcut field to NavItem interface. Sidebar buttons now show
  platform-aware shortcut hints in their title tooltip:
  - Download (⌘D / Ctrl+D)
  - Queue (⌘Q / Ctrl+Q)
  - Settings (⌘, / Ctrl+,)

  Uses usePlatform().isMacOS to show ⌘ on macOS, Ctrl+ on other platforms.

- Enhanced empty state illustrations for Queue and Activity (#389)

- Activity Log: add ScrollText icon (32px, 40% opacity) above empty state text
  - Queue: increase icon to 40px, add descriptive text about clipboard monitoring
  - Both now match History page's empty state pattern (icon + primary + secondary text)

- Contextual error recovery guidance in download failures (#390)

Add error_guidance() helper in utils/process.rs that maps error
  categories to actionable user suggestions:
  - auth → refresh cookies or check wrapper
  - network → check internet, auto-retry will resume
  - io → check output directory access and disk space
  - codec → try different quality setting
  - not_found → content may be removed or URL incorrect
  - rate_limit → wait and retry
  - tool → check Settings > Tools

  Guidance is emitted as a 💡 activity log entry after each terminal
  failure, and included in the download-error event payload for
  frontend display.

- Crash/telemetry opt-in prompt infrastructure (#402)

Add crash_report_prompt_shown field to AppSettings (Rust + TypeScript)
  to gate a first-launch crash reporting opt-in prompt. The field prevents
  re-prompting after the user has made their choice.

  The frontend modal component will be wired up in a follow-up — this
  provides the settings infrastructure.

  Partial fix for #402.

- Explicit settings migration with version field (#392)

Add settings_version field (u32) to AppSettings with
  CURRENT_SETTINGS_VERSION constant (currently 1).

  On load, migrate_settings() runs sequential migrations:
  - v0 → v1: stamps version (no structural changes yet)
  - Future migrations add blocks for v1→v2, v2→v3, etc.

  Old settings files (without settings_version) deserialize as v0
  via serde(default), triggering the migration automatically.

- Disk space check before download — warn if < 500 MB (#408)

Add available disk space check to the output path preflight probe.
  After verifying write access, check remaining space via fs2 crate.
  If < 500 MB available, emit a PreflightWarning with the remaining
  space and a suggestion about large albums.

  New dependency: fs2 (0.4, lightweight cross-platform disk space query).

- Batch URL import from .txt file (#409)

Add "Import URLs from .txt" button on the Download page alongside
  the existing .meedyadl manifest import. Supports:
  - One URL per line
  - Lines starting with # treated as comments (skipped)
  - Empty lines skipped
  - Native file picker with .txt filter

  Uses dynamic imports of @tauri-apps/plugin-dialog and plugin-fs
  to keep the bundle lean (only loaded when the button is clicked).

- System tray tooltip shows download progress (#410)

Add TrayState managed state and update_tray_tooltip() function in lib.rs.
  The tray icon tooltip updates on every queue state change (via
  save_queue_to_disk_inner) showing:
  - "MeedyaDL — 2 downloading, 3 queued" during active downloads
  - "MeedyaDL — 5 completed" when all done
  - "MeedyaDL" when idle

  Works on all platforms (macOS, Windows, Linux). Future: macOS dock
  badge count and Windows taskbar progress overlay can be added as
  platform-specific enhancements.

- Undo queue clear with 5-second toast action (#406)

When clearing the queue (Clear All), URLs of cleared items are saved
  to an _undoBuffer in the download store. An "Undo" toast appears for
  5 seconds. Clicking "Undo" re-enqueues all cleared URLs.

  The buffer auto-expires after 5 seconds to prevent stale state.

- Download speed sparkline in Statistics panel (#407)

Collect speed samples from gamdl-output progress events (last 60
  samples, parsed from "2.5MB/s" format to numeric MB/s). Render as
  a SVG polyline sparkline in the Session Statistics panel with the
  current speed displayed alongside.

  The sparkline provides at-a-glance speed history for diagnosing
  throttling or network fluctuations during download sessions.

- WebView memory mitigation with session log persistence (#393)
- Integration test smoke tests for GAMDL and URL parsing (#414)

Add two smoke tests to the integration test suite:
  - gamdl_version_smoke_test: verifies GAMDL subprocess works (#[ignore])
  - url_parsing_smoke_test: verifies Apple Music URL parsing

  The GAMDL test is #[ignore]d by default since it requires Python + GAMDL.
  Run explicitly with: cargo test -- --ignored

  Partial fix for #414 — full download pipeline tests require CI secrets.

- Visual regression testing infrastructure with Puppeteer (#415)

Add screenshot capture script using Puppeteer (already a devDependency):
  - Captures pages at desktop (1280x800) and compact (900x600) viewports
  - Supports both dark and light theme via prefers-color-scheme emulation
  - Baseline capture mode: --baseline flag saves to baselines/ directory
  - Comparison mode scaffold (pixelmatch integration is a follow-up)

- Analytics opt-in infrastructure (#405)

Add analytics_enabled field to AppSettings (Rust + TypeScript),
  defaulting to false (opt-in). The actual analytics endpoint and
  data collection are deferred to post-v1 — this provides the
  settings field so the opt-in prompt can be built.

  Partial fix for #405.

- Recover service expansion groundwork from meedyadl-v2 (#373)

Cherry-pick recovered files from prep/refactoring/supported-service-expansion:

- Complete partial implementations (#385, #402, #405, #413, #414, #415)

#385 — Notification throttling: add 10-second batching per notification
  category. Rapid completions batch as "Download failed (3 items)" instead
  of 3 separate notifications.

  #402 — Crash report opt-in modal: CrashReportOptInModal component with
  Accept/Decline buttons. Shown after setup wizard completes if
  crash_report_prompt_shown is false. Wired into App.tsx.

  #405 — Analytics toggle: add "Anonymous Usage Analytics" toggle in
  Settings > Advanced > Error Reporting section.

  #413 — Rustdoc in CI: add `cargo doc --no-deps` step to backend CI job
  (Linux only, RUSTDOCFLAGS="-D warnings"). Fix doc-test in rate_limiter.rs.

  #414 — Fix doc-test compilation in rate_limiter.rs (add no_run + import).

  #415 — Visual regression comparison: add pixelmatch + pngjs for pixel-diff
  comparison. `node scripts/visual-regression.mjs compare` diffs screenshots
  against baselines with 0.1% threshold, saves diff images.

- BPM analysis service with metadata tagging (#418)

Add bpm_service.rs with:
  - detect_bpm(): reads existing BPM tags via ffprobe; full DSP analysis
    deferred to MeedyaSuite-core integration (see MeedyaSuite-core#16)
  - process_bpm_for_directory(): batch analysis with progress emission
  - write_bpm_m4a(): writes tmpo atom via mp4ameta
  - write_bpm_lofty(): writes TBPM/BPM via lofty (MP3/FLAC/OGG)

  Add bpm_analysis_enabled setting (default: false, opt-in).
  Registered in services/mod.rs.


### 🐛 Bug Fixes

- Toast notification text overflow with long paths (#384)

Add break-words + overflow-hidden to toast message text, and
  overflow-hidden to the toast container element. Long file paths
  without natural break points (e.g., /Users/.../Library/...) now
  wrap within the toast bounds instead of leaking outside.

- Platform icon empty square — normalize SVG fetch path (#394)

The platform icon from engines.toml uses a relative path
  (icons/platforms/apple-music.svg) which may not resolve correctly
  in Tauri production builds where the base URL is tauri://localhost/.
  Normalize to absolute path (prepend /) before fetching.

- Clippy doc_lazy_continuation, needless_borrow, eslint ban-ts-comment

- Separate doc comment blocks for NOTIFICATION_THROTTLE static
  - Remove needless & in error_guidance() call
  - Add eslint-disable for @ts-nocheck in serviceStatusStore.ts
    (staged for future, not active code)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add updater signing key rotation plan to SECURITY.md (#401)

Document the recovery procedure if the Tauri updater signing key is
  compromised: revoke, regenerate, publish manual recovery release,
  communicate via GitHub Security Advisory.

  Also document IPC rate limiting (#395), settings integrity (#396),
  and pip verification (#397) in DEV_NOTES.md.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Comprehensive third-party licence viewer (#399)

Rewrite ACKNOWLEDGEMENTS.md with complete dependency inventory:
  - 4 download engines with licences
  - 6 external tools with licences
  - 36 direct Rust crates with versions, licences, descriptions
  - 9 Tauri plugins
  - 14 direct npm packages
  - Licence compliance statement referencing cargo-deny

  Already referenced in Help > About > Open Source Acknowledgements.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- MacOS Gatekeeper, Windows SmartScreen, CONTRIBUTING, CODE_OF_CONDUCT

- FAQ: add macOS Gatekeeper workaround with xattr command (#403)
  - FAQ: add Windows SmartScreen "Run anyway" instructions (#404)
  - CONTRIBUTING.md: dev setup, conventions, PR process, release info (#411)
  - CODE_OF_CONDUCT.md: Contributor Covenant v2.1 (#412)

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add npm script for Rust API documentation generation (#413)

Add `npm run docs:rust` which runs `cargo doc --no-deps --open`
  in the src-tauri directory. Generates and opens rustdoc in the
  browser. All public functions already have doc comments per
  project convention.

  Partial fix for #413 — CI artifact generation and GitHub Pages
  hosting are planned follow-ups.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Comprehensive CLAUDE.md update with v1 RC features

Add documentation for: IPC rate limiting, settings integrity check,
  settings migration, after-queue actions, notification tiers, BPM
  analysis, disk space check, session log retention.

- Update CHANGELOG.md [skip ci]

### Legal

- Add EULA / Terms of Service (#398)

Add TERMS.md with:
  - MIT licence reference
  - Disclaimer of warranty
  - No affiliation with Apple/Spotify/Google/BBC
  - User responsibility clause
  - Data collection transparency (Sentry opt-in, clipboard monitoring)
  - Third-party dependency notice
  - Limitation of liability

  Add terms_accepted field to AppSettings (Rust + TypeScript) for
  first-launch acceptance tracking. Frontend modal to be wired up
  in a follow-up (the field + TERMS.md are the foundation).

- Add DMCA / content takedown process to SECURITY.md (#400)

Document how rights holders can submit takedown requests, what
  MeedyaDL can and cannot control, response timeline (48h ack, 7 day
  response), and counter-notification procedure.


### Security

- Add IPC command rate limiting (#395)

Add a sliding-window rate limiter in utils/rate_limiter.rs with per-command
  configurable limits. Applied to sensitive commands:
  - start_download: 10 calls/minute
  - check_all_updates: 1 call/minute
  - download_and_install_app_update: 1 call/minute
  - import_cookies_from_browser: 3 calls/minute

  Returns a user-friendly "Too many requests" error with retry-after duration
  when the limit is exceeded. Includes unit tests.

- Add settings file integrity check via SHA-256 checksum (#396)

Compute SHA-256 digest on save and write to companion .sha256 file.
  On load, verify the checksum matches. If mismatched, log a warning
  but still load the settings (user may have intentionally edited).

  Backwards compatible: settings without a checksum file are accepted
  and a checksum is generated for next time.

- Add post-install verification for pip packages (#397)

After pip install, run `pip show --verbose` to verify the installed
  package location and log it for audit trail. Applied to both
  gamdl_service::install_gamdl() and pip_engine_service::install_pip_engine().

  This provides a verifiable record of what was installed and where,
  enabling detection of tampered packages after installation.


## [0.30.0] - 2026-04-10

### ✨ Features

- Clipboard direct queue, native notifications, auto-scroll checkbox

- Clipboard toast "Download" now queues directly via startDownload()
    instead of pre-filling the URL input (#376)
  - Native OS notifications sent via @tauri-apps/plugin-notification when
    the window is not focused, so clipboard URLs are never missed (#377)
  - Activity Log Pause/Resume button replaced with Auto-scroll checkbox
    that auto-unchecks on scroll-up and re-checks to jump to bottom (#378)
  - Updated in-app help docs (getting-started, downloading-music, faq,
    troubleshooting) with new clipboard, notification, and activity log
    behaviour
  - Updated CLAUDE.md, README.md, Project_Plan.md

- Activity log readability + after-queue actions backend

Activity log readability (#382):
  - Add zebra striping (alternating subtle backgrounds) for visual separation
  - Add bottom border between entries (border-border/20)
  - Change break-all to break-words for more natural wrapping
  - Add horizontal padding (px-1) for breathing room

  After-queue actions backend (#383):
  - Add AfterQueueAction enum with 7 actions: do_nothing, open_output_folder,
    play_sound, close_meedyadl, restart_computer, hibernate_computer,
    shutdown_computer
  - Add after_queue_action (persistent) and after_queue_once (one-shot)
    fields to AppSettings with serde defaults
  - Add is_idle() method to DownloadQueue
  - Add execute_after_queue_action() with platform-specific implementations
    for macOS, Windows, Linux (restart/hibernate/shutdown)
  - Queue idle detection in process_queue() exit path
  - TypeScript AfterQueueAction type added to index.ts

- After-queue actions UI — settings dropdown, context menu, status bar

Settings > General > Preferences:
  - "After Queue Completes" dropdown with 7 actions (do nothing, open folder,
    play sound, close app, restart, hibernate, shut down)

  Download page right-click context menu:
  - "After Queue: ..." one-shot actions that apply to the next queue completion
    only, then auto-clear. Shows confirmation toast.

  Status bar:
  - AfterQueueIndicator shows the active after-queue action in the status bar
    with "(once)" suffix for one-shot overrides. Hidden when set to "Do nothing".
    Displayed in warning colour to draw attention.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.29.5] - 2026-04-10

### 🐛 Bug Fixes

- Restore activity log line spacing after virtualization

The switch to @tanstack/react-virtual absolute positioning caused rows
  to lose inherited font-mono/text-xs/leading-relaxed from the parent
  container. Add these classes directly to each virtualized row and add
  py-[1px] vertical padding for minimal line separation.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update documentation for activity log optimization and macOS updater fix

- README.md: add activity log memory optimization and macOS updater fix
    to completed features, update supply chain hardening note
  - DEV_NOTES.md: add detailed sections for activity log optimization (#370),
    macOS updater fix (#368), and cargo-deny org allowlist (#365)
  - SECURITY.md: update supported version to 0.29.x, add activity log memory
    bounds and updater artifact signing to security measures
  - Project_Plan.md: add completed items for activity log optimization,
    macOS updater fix, and cargo-deny org allowlist
  - CLAUDE.md: update activity log and macOS updater artifact naming sections

- Update documentation for activity log optimization and macOS up… (#372)

…dater fix

  - README.md: add activity log memory optimization and macOS updater fix
  to completed features, update supply chain hardening note
  - DEV_NOTES.md: add detailed sections for activity log optimization
  (#370), macOS updater fix (#368), and cargo-deny org allowlist (#365)
  - SECURITY.md: update supported version to 0.29.x, add activity log
  memory bounds and updater artifact signing to security measures
  - Project_Plan.md: add completed items for activity log optimization,
  macOS updater fix, and cargo-deny org allowlist
  - CLAUDE.md: update activity log and macOS updater artifact naming
  sections

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.29.4] - 2026-04-10

### 🐛 Bug Fixes

- Resolve merge conflict in deny.toml — keep org-level allowance

Resolve conflict between #364's per-repo allow-git entry and #367's
  org-level [sources.allow-org] approach. The org-level approach is
  superior as it covers all MWBMPartners repos regardless of branch/rev.

- **(ci)** Allow MeedyaDL org in cargo-deny source allowlist

Add the MeedyaDL GitHub org alongside MWBMPartners so git dependencies
  from repos like MeedyaDL-Tools pass cargo-deny source checks. Future-
  proofs against additional project-specific repos under that org.

- **(ci)** Update cargo-deny config to allow MeedyaSuite-core org sources (#367)

Replace `allow-git` with `[sources.allow-org]` for MWBMPartners GitHub
  org, so all git dependencies from the org pass source checks regardless
  of branch or rev qualifiers.

- **(ci)** Correct macOS updater artifact filename in release workflow

Tauri 2.x names the macOS updater bundle after the .app bundle itself
  (MeedyaDL.app.tar.gz), NOT with an arch suffix (MeedyaDL_aarch64.app.tar.gz).
  The upload step and latest.json rebuild were looking for the wrong filename,
  so the .app.tar.gz and .app.tar.gz.sig were never uploaded. This caused
  the darwin-aarch64 platform to be missing from latest.json, making in-app
  updates fail on macOS with "No update found for this platform".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update generated macOS schema

## [0.29.3] - 2026-04-09

### 🐛 Bug Fixes

- **(ci)** Allow MeedyaSuite-core git source in cargo-deny

The MeedyaSuite-core integration (d802870) added git dependencies for
  meedya-core, meedya-codecs, meedya-metadata, and meedya-fingerprint.
  These were blocked by cargo-deny's source allowlist.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### ⚡ Performance

- Fix activity log memory leak causing 14+ GB WebView RAM usage

The WebView process grew to 14+ GB during download sessions due to
  unbounded activity log accumulation, non-virtualized DOM rendering,
  and high-frequency event emission from the Rust backend.

- Fix activity log memory leak (14+ GB → <500 MB) (#364)

## Summary

  - **RAF-batched event listener** in `App.tsx` — collapses hundreds of
  per-line Zustand updates into ~60/s via `requestAnimationFrame`
  buffering
  - **Capped activity store** at 10,000 entries with batch `addEntries()`
  method and auto-incrementing `_id` for stable React keys
  - **Virtualized ActivityLog** with `@tanstack/react-virtual` — DOM nodes
  drop from ~37,500 to ~150 regardless of entry count
  - **Backend `\r` segment coalescing** in `download_queue.rs` — only
  emits the last progress segment to `activity-log` (5-10x event
  reduction)
  - **Download store optimisation** — `map()` pattern instead of
  spread+findIndex+splice for lower GC pressure

  ## Context

  During multi-item download sessions, the `tauri://localhost` WebView
  process grew to 14+ GB RAM and the app froze. Root causes: unbounded
  activity log array with O(n) spread-copy on every entry, all 7,500+
  entries rendered as real DOM nodes without virtualization, and
  ~20,000-40,000 events emitted per album download from the Rust backend.

  ## Test plan

  - [x] `npm run type-check` passes
  - [x] `npm run test` passes (272/272 tests, including updated
  activityStore tests)
  - [x] `cargo check` passes
  - [ ] Manual test: queue 3+ albums, watch Activity Monitor — WebView
  memory should stay under ~500 MB
  - [ ] Verify activity log auto-scrolls, search/filter, export, and
  pause/resume work
  - [ ] Verify log entries are trimmed at cap (queue enough downloads to
  exceed 10,000 lines)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)


## [0.29.2] - 2026-04-09

### 🐛 Bug Fixes

- **(ci)** Add macOS notarization retry logic to release workflow

Apple's notarization service occasionally returns HTTP 503 "Slow Down"
  rate-limiting errors, causing macOS release builds to fail (see #360).

  This separates the macOS build from tauri-action and handles it manually
  with retry logic:
  - Up to 3 attempts with exponential backoff (30s, 60s)
  - Only notarization failures (503/serviceUnavailable) trigger retries
  - Non-transient errors (compilation, signing) fail immediately
  - Artifacts uploaded manually via gh release upload

- **(ci)** Add macOS notarization retry and updater manifest verification (#362)

## Summary
  - Add macOS notarization retry logic to `release.yml` — up to 3 attempts
  with exponential backoff (30s, 60s) on Apple 503 "Slow Down" errors;
  non-transient errors fail immediately
  - Verify `latest.json` content before showing update banner —
  `verify_manifest_has_platform()` downloads and checks that the manifest
  contains a `platforms` entry for the current OS/arch, suppressing the
  update notification if missing
  - Separate macOS build from `tauri-action` and handle artifact upload
  manually (mirrors existing ARMv7 pattern)

  ## Test plan
  - [ ] Verify CI passes on the PR (Rust check, tests, frontend
  lint/type-check)
  - [ ] Confirm `release.yml` syntax is valid (no YAML parse errors)
  - [ ] On next release, verify macOS build succeeds with the retry
  wrapper
  - [ ] Verify `latest.json` check gracefully falls back to `true` on
  network errors (doesn't suppress updates when manifest is unreachable)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.29.1] - 2026-04-08

### ✨ Features

- Integrate MeedyaSuite-core for tag registry (phase 1)

Replace the custom tag registry implementation (~425 lines) with
  meedya-core's shared tag_registry module. This is the first phase of
  the MeedyaSuite-core integration.


### 🐛 Bug Fixes

- **(ci)** Update cargo-deny config to allow MeedyaSuite-core org sources

Replace `allow-git` with `[sources.allow-org]` for MWBMPartners GitHub org,
  so all git dependencies from the org pass source checks regardless of branch
  or rev qualifiers.

- Resolve macOS update download failure

Root cause: The `latest.json` updater manifest was missing the
  `darwin-aarch64` platform entry due to a race condition in the
  release workflow. When parallel platform builds each upload their
  own `latest.json` via tauri-action, the last build to finish
  overwrites all previous entries. For v0.29.0, the Windows build
  finished last, leaving only `windows-x86_64` entries.

- Resolve npm audit high-severity vulnerability in basic-ftp

Updates basic-ftp 5.2.0 → 5.2.1 to fix FTP Command Injection via
  CRLF (GHSA-chqc-8p9q-pq6q). Transitive dependency via puppeteer →
  proxy-agent → get-uri. Fixes CI Frontend job failure on npm audit.

- Verify latest.json content before showing update banner

The update checker previously only verified that `latest.json` existed
  as a release asset, not that it contained a download entry for the
  current platform. This caused the update banner to appear even when
  the manifest was missing the platform entry (due to CI race condition).

  Now downloads and parses `latest.json` to verify the platform key
  (e.g., `darwin-aarch64`) exists before showing the update notification.
  Gracefully falls back to showing the update if the manifest can't be
  fetched (avoids suppressing updates due to transient network errors).

- Resolve macOS update download failure (#355)

Root cause: The `latest.json` updater manifest was missing the
  `darwin-aarch64` platform entry due to a race condition in the release
  workflow. When parallel platform builds each upload their own
  `latest.json` via tauri-action, the last build to finish overwrites all
  previous entries. For v0.29.0, the Windows build finished last, leaving
  only `windows-x86_64` entries.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- Add workflow to fix updater manifest for existing releases

Adds a `workflow_dispatch` workflow that rebuilds the `latest.json`
  updater manifest for any existing GitHub Release. This fixes the race
  condition where parallel platform builds each overwrite `latest.json`,
  causing the last platform to win and missing earlier platforms.

  Triggered manually via Actions UI with a release tag input.

- Add workflow to fix updater manifest for existing releases

Adds a `workflow_dispatch` workflow that rebuilds the `latest.json`
  updater manifest for any existing GitHub Release. This fixes the race
  condition where parallel platform builds each overwrite `latest.json`,
  causing the last platform to win and missing earlier platforms.

  Triggered manually via Actions UI with a release tag input.


## [0.29.0] - 2026-04-08

### ✨ Features

- Upgrade Vite to 8.x and @vitejs/plugin-react to v6 (#340)

- Vite 7.3.x → 8.0.7 (Rolldown-based bundler, 10-30x faster builds)
  - @vitejs/plugin-react 5.x → 6.0.1 (Oxc-based, Babel-free)
  - Renamed build.rollupOptions → build.rolldownOptions (deprecated shim)
  - Added INEFFECTIVE_DYNAMIC_IMPORT to warning suppression filter
    (Rolldown's equivalent of Rollup's MIXED_IMPORTS)
  - Updated comments to reflect Oxc/Rolldown internals
  - 0 npm audit vulnerabilities
  - All 268 frontend tests pass, lint clean, type-check clean

- Upgrade i18next 25→26 and react-i18next 16→17 (#343)

Drop-in upgrade — no deprecated API usage in our codebase.
  Build, type-check, and all 268 tests pass.

- Upgrade jsdom 28→29 (Vitest test environment) (#345)

New CSSOM implementation; no impact on React component tests.
  5 packages removed (old CSS deps). All 268 tests pass.

- Upgrade ESLint 9→10 and fix preserve-caught-error lint violations (#342, #348)

- ESLint 9.39.4 → 10.2.0, @eslint/js 9.39.4 → 10.0.1
  - eslint-plugin-react-hooks updated for ESLint 10 compatibility
  - @testing-library/dom added (was missing from dependency tree)
  - Fixed 10 preserve-caught-error violations: added { cause: e } to
    all throw new Error() calls in catch blocks across 4 store files
  - Bumped tsconfig target/lib ES2021 → ES2022 (ErrorOptions.cause
    requires ES2022 types; Safari 16.4 and Chrome 111 both support it)

  All checks pass: lint clean, type-check clean, 268 tests, 0 vulnerabilities.

- Upgrade lucide-react 0.577→1.x (#344)

Upgrades lucide-react from 0.577.0 to ^1 (1.7.0). Audited all 53
  unique icon imports across the codebase — zero brand icons found,
  no breaking changes. Icons now set aria-hidden by default.

- Upgrade TypeScript 5→6 (#341)

Upgrades TypeScript from 5.9.3 to 6.0.2. Zero code changes required —
  the codebase already follows TS6 conventions (strict mode, ESNext
  module, bundler resolution, no enums/namespaces/decorators). All checks
  pass: type-check clean, ESLint clean, Vite build succeeds, 268 tests
  pass, 0 vulnerabilities.


### 🐛 Bug Fixes

- Add .npmrc with legacy-peer-deps for CI compatibility

eslint-plugin-react-hooks@7.0.1 declares peerDependencies.eslint
  "^9.0.0" which conflicts with ESLint 10. npm ci in CI fails without
  legacy-peer-deps=true. The next stable release of the plugin will
  add ESLint 10 support, at which point this file can be removed.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Cargo update for patch-level Rust dependency updates (#347)

Updates 9 transitive dependencies to latest compatible versions:
  - async-signal 0.2.13→0.2.14, cc 1.2.58→1.2.59
  - fastrand 2.3.0→2.4.1, indexmap 2.13.0→2.13.1
  - muda 0.17.1→0.17.2, notify-rust 4.12.0→4.14.0
  - semver 1.0.27→1.0.28, toml_edit 0.25.10→0.25.11
  - writeable 0.6.2→0.6.3

  All 670 tests pass. No breaking changes.


## [0.28.1] - 2026-04-08

### 🐛 Bug Fixes

- Update Vite to patch 3 high-severity vulnerabilities

npm audit fix resolves:
  - GHSA-4w7w-66w2-5vf9 (Path Traversal in Optimized Deps .map Handling)
  - GHSA-v2wj-q39q-566r (server.fs.deny bypassed with queries)
  - GHSA-p9ff-h696-f583 (Arbitrary File Read via Dev Server WebSocket)

- Add Display impl for MediaServiceId and missing QueueItem field

- Implement std::fmt::Display for MediaServiceId (delegates to
    display_name()), enabling .to_string() calls in queue restoration
  - Add missing engine_fallback_index: 0 to QueueItem initializer in
    restore_items()

  Fixes Backend CI compilation errors on all 3 platforms.

- Resolve remaining Backend CI compilation errors in tests

- download.rs: add missing service/engine fields to 3 test QueueItemStatus
    initializers (required since #318 added these fields)
  - download_queue.rs: update 3 tuple destructurings from 3-element to
    4-element to match next_pending() return type change from #318
  - engine_runner.rs: remove tauri::test::mock_builder() call that requires
    the "test" Cargo feature flag not enabled in Cargo.toml
  - media_service.rs: use kebab-case platform IDs in Display impl to match
    engines.toml keys used by resolve_engine()

  All 670 tests pass locally (668 unit + 2 doc-tests).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Updated internal docs
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Remove duplicate Display impl and update generated schemas

Linter added a second Display impl for MediaServiceId with kebab-case
  IDs; removed it in favour of the original display_name()-based impl.
  Includes auto-generated Tauri schema updates.


## [0.28.0] - 2026-04-02

### ✨ Features

- Clipboard monitoring for supported URLs (#330)

Monitor the system clipboard while MeedyaDL is open. When a supported
  URL (Apple Music) is copied, an actionable toast prompts the user to
  download. Privacy-first: only checks URL patterns, never stores
  clipboard contents. Session-scoped deduplication prevents re-prompting.

  - Backend: clipboard_service.rs via arboard crate, read_clipboard IPC
  - Frontend: useClipboardMonitor hook (2s poll), toast action buttons
  - Setting: clipboard_monitoring (default true), toggle in General tab
  - Docs: CLAUDE.md, CHANGELOG, help/downloading-music, help/faq, help/getting-started


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.27.0] - 2026-04-01

### ✨ Features

- Add ReplayGain album gain toggle and artist promo video download (#325)

- ReplayGain: new "Include Album Gain" setting (Settings > Metadata)
    controls whether album-level tags are written alongside track tags.
    Default: on. When disabled, only per-track gain is written.

  - Artist promo video: new "Download Artist Promo Video" setting
    (Settings > Cover Art) downloads the animated background from Apple
    Music artist pages as ArtistCover.mp4 to the artist folder. Requires
    MusicKit credentials. Idempotent (skips if file already exists).

  - parse_apple_music_url() now recognises artist URLs, returning a
    ParsedAppleMusicUrl with artist_id field.

- Expand ReplayGain to MP4-family, FLAC, MP3, OGG/Opus (#325, #326, #327, #328)

ReplayGain analysis was limited to .m4a files only. Now supports:

  - MP4-family (M4A, M4V, MP4, M4P, M4B): iTunes freeform atoms via mp4ameta
  - FLAC, OGG, OGA, Opus: Vorbis Comments via lofty crate
  - MP3: ID3v2 TXXX frames via lofty crate

  FFmpeg's ebur128 filter already handled all these inputs; only the file
  collection and tag writing needed expansion. Format-aware dispatch via
  AudioFormat enum routes to mp4ameta or lofty as appropriate.


### 🐛 Bug Fixes

- Prevent consecutive separator chips from collapsing in template builder

The parser's longest-first token matching greedily consumed " - " as a
  single compound token, so when a user built Space + Hyphen + Space as
  three individual chips, the serialize→re-parse roundtrip collapsed them
  into one "Dash Separator" chip. Now the parser only splits on atomic
  (single-character) tokens, preserving individual chip boundaries.

  The "Dash Separator" menu shortcut still works — it adds " - " which
  re-parses into three atomic chips (Space, Hyphen, Space).

- Rename "Component Library" to "Dependencies" and remove duplicate MeedyaDL entry in Help > About

The MeedyaDL version was shown both at the top of the About page and
  again in the component list. Removed the duplicate entry from
  get_component_versions(). Renamed the section to "Dependencies" since
  it lists external tools, not a component library.

- Add new dependencies for data-encoding, lofty, lofty_attr, ogg_pager, and paste
- Ignore RUSTSEC-2024-0436 (paste) unmaintained advisory in cargo-deny

Transitive dependency from lofty (ReplayGain tagging). Archived by
  dtolnay — no security vulnerability, just unmaintained status.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add missing activity store to CLAUDE.md stores listing
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.26.2] - 2026-04-01

### ✨ Features

- Multi-service URL parser with YouTube, Spotify, BBC iPlayer detection (#315)

Add YouTube, YouTube Music, Spotify, and BBC iPlayer variants to
  MediaServiceId with domain detection. Add generic parseMediaUrl() and
  detectService() to frontend url-parser.ts. Add detect_service IPC command.
  Existing Apple Music parsing preserved as-is.

- Service registry and engine resolver from engines.toml (#316)

Add EngineRegistry service that provides typed runtime access to the
  compiled-in engines.toml configuration. Includes engine/platform lookup,
  URL-based platform detection, and engine chain resolution (primary +
  fallbacks). Exposed via get_engine_config IPC command with TypeScript
  types in tauri-commands.ts.

- Service-aware download queue with service/engine fields (#318)

Add service and engine fields to QueueItemStatus and PersistedQueueItem.
  Detect media service at enqueue time via MediaServiceId::from_url() and
  resolve the primary engine via EngineRegistry. Guard Apple Music-specific
  enrichment pipeline behind service check so non-Apple Music downloads
  skip API metadata, lyrics, artwork, and companion downloads. Replace
  hardcoded music.apple.com domain checks with MediaServiceId for manifest
  platform detection. Backwards-compatible via serde(default) for legacy
  persisted queue items.

- Per-service settings infrastructure (#319)

Add PerServiceSettings container with typed structs for Apple Music,
  Spotify (stub), and YouTube (stub) service-specific configuration.
  Nest under AppSettings.service_settings with serde(default) for
  backwards compatibility with existing settings.json files. Apple Music
  settings mirror existing flat fields (storefront, cookies_path,
  musickit_*, animated_artwork, enhanced_lrc, content_advisory). Existing
  flat fields preserved during migration period. TypeScript types added
  for all service settings interfaces.

- Per-platform engine priority and fallback (#320)

Add try_engine_fallback() to download queue that advances through
  the engine chain (from engines.toml) when a tool error occurs.
  Network and auth errors skip engine fallback. Add engine_fallback_index
  to QueueItem for tracking position in the chain. Add engine_priority
  HashMap to PerServiceSettings for user-overridden engine ordering
  per platform (defaults to engines.toml order when empty).


### 🐛 Bug Fixes

- Prioritise Sound & Video category in Linux desktop entry

Remove Utility category from .desktop file so MeedyaDL appears under
  "Sound & Video" rather than "Internet" or "Accessories" on Linux DEs
  like Raspberry Pi OS (LXDE). AudioVideo remains the primary category
  with Network as secondary.

- Restore Utility category in Linux desktop entry

Restore Utility category — MeedyaDL is a download utility focused on
  audio/video. Keep AudioVideo as the primary category so it appears
  under "Sound & Video" on Raspberry Pi OS and similar DEs.

- Use PNG fallback for logotype in light mode README header

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Per-service help documentation and UI polish (#321)

Add "Supported Services" help topic covering Apple Music, Spotify,
  YouTube, YouTube Music, and BBC iPlayer with engine information,
  accepted URL formats, authentication requirements, and the engine
  fallback system. Update help index with new topic reference and
  multi-service architecture description.

- Update CLAUDE.md with multi-service architecture context

Add comprehensive multi-service architecture documentation to CLAUDE.md:
  - New architecture bullet covering EngineRegistry, EngineCommandBuilder,
    service-aware queue, PerServiceSettings, and engine fallback
  - Updated Key Directories with engine_registry and engine_runner services
  - Phase 7 added to Implementation Phases for #107
  - Planned Service Integrations updated to reflect completed architecture
    foundation (#314-#321) vs remaining per-milestone work

- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Rename MusicService → MediaService across codebase (#314)

Pure rename — no behaviour change. Renames MusicServiceId to MediaServiceId,
  MusicService trait to MediaService, and music_service.rs to media_service.rs
  across Rust, TypeScript, and documentation.

- Abstract subprocess spawning with EngineCommandBuilder trait (#317)

Create engine_runner.rs with generic run_engine() function and
  EngineCommandBuilder trait for service-agnostic subprocess spawning.
  Refactor gamdl_service::run_gamdl() to delegate to the engine runner.
  Add stub command builders for Votify, yt-dlp, and get_iplayer with
  factory function get_command_builder(). Emits both "engine-output"
  (new) and "gamdl-output" (legacy) events for backwards compatibility.


## [0.26.1] - 2026-04-01

### 🐛 Bug Fixes

- Always fetch full release list for multi-version changelog aggregation

The pre-release update check only fetched 5 releases (per_page=5), then
  reused that short list for changelog aggregation. When the user was more
  than ~4 versions behind, intermediate release notes were silently dropped.
  Now aggregate_intermediate_release_notes always fetches per_page=20
  independently, ensuring all intermediate changelogs are shown.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.26.0] - 2026-04-01

### ✨ Features

- Add new Dolby SVG icons for audio formats
- Add premium feature token fallback and internal dev access mode (#312)

Introduces a 3-tier MusicKit token resolution for premium API features
  (syllable-lyrics, animated artwork, music video relations): user credentials
  → embedded build token → web session token. The web session token is
  extracted opportunistically from the login window during cookie import.

  Adds an internal developer access mode gated by a hidden activation gesture
  and SHA-256 passphrase validation, with a Developer Tools section in
  Settings > Advanced showing token status and management controls.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md to include new line for improved MusicKit API fallback
- Add pip_engine_service and integration_tests to services in CLAUDE.md
- Update CHANGELOG.md [skip ci]

## [0.25.0] - 2026-03-31

### ✨ Features

- Initialize GAMDL GUI application with Tauri and React

- Add Tauri configuration file for application settings and build options.
  - Create main application component with platform detection and theme loading.
  - Implement custom hook for platform detection using Tauri's OS plugin.
  - Set up entry point for React application and global styles with Tailwind CSS.
  - Define base and platform-specific themes for macOS, Windows, and Linux.
  - Configure Tailwind CSS for platform-adaptive design tokens and styles.
  - Remove legacy test files and Python dependencies.
  - Add TypeScript configuration for Vite and Node environments.
  - Set up Vite configuration for React and Tauri integration.

- Add setup wizard components and state management

- Implement WelcomeStep component for the setup wizard, providing an introduction and overview of the setup process.
  - Create tauri-commands.ts for type-safe IPC calls to the Rust backend, covering system commands, dependency management, settings, downloads, and credential storage.
  - Introduce url-parser.ts to parse Apple Music URLs and detect content types.
  - Establish dependencyStore.ts to manage the installation status of Python, GAMDL, and external tools.
  - Create downloadStore.ts to handle download queue management, URL validation, and progress tracking.
  - Implement settingsStore.ts for managing application settings with load/save operations.
  - Add setupStore.ts to manage the setup wizard flow and completion status.
  - Introduce uiStore.ts for transient UI state management, including page navigation and toast notifications.
  - Update globals.css to include keyframe animations for UI components.
  - Define TypeScript types in index.ts to ensure type safety across the application, mirroring Rust backend models.

- Enhance CookiesTab with detailed browser export instructions and validation feedback

- Added step-by-step instructions for exporting cookies from various browsers (Chrome, Firefox, Edge, Safari).
  - Implemented a status badge to indicate the current cookie state (valid, invalid, expired).
  - Introduced a warning banner for cookie expiry with estimated days remaining.
  - Enhanced validation results display to include detected domains and additional warnings.
  - Improved user experience with a "Copy Cookie Path" button and loading states for validation.
  - Updated tauri-commands to support new download management features (retry, clear queue).
  - Created a new updateStore to manage application update checks and notifications.
  - Expanded types to include music service capabilities and update status for components.

- Implement icon generation script, ESLint configuration, and Vitest setup for testing
- Automate copyright year updates across all source files and enhance script functionality
- Implement theme management with useTheme hook and update styles for dark/light modes
- Add release automation and expand to 7 platform targets

Add one-command release automation via Version Bump workflow
  (workflow_dispatch) that bumps versions across all source files,
  commits, tags, and triggers the release build. Expand the release
  build matrix from 3 to 7 platform targets: macOS ARM64, Windows
  x64/x86/ARM64, Linux x64/ARM64/ARMv7 (Raspberry Pi).

- Integrate release-please for automated release PRs

Add Google's release-please to automatically create Release PRs when
  conventional commits land on main. When merged, the PR creates a tag
  that triggers the existing 7-platform release build. git-cliff continues
  to own CHANGELOG.md (release-please has skip-changelog: true). The
  manual version-bump workflow is preserved as an override for non-standard
  releases.

- Add browser cookie extraction service and auto-import functionality

- Introduced `cookie_service` module for extracting Apple Music cookies from installed browsers.
  - Implemented auto-import feature in `CookiesTab` and `CookiesStep` components, allowing users to extract cookies with a single click.
  - Added platform-specific handling for macOS (Keychain access and Full Disk Access for Safari).
  - Enhanced user interface with loading indicators, error handling, and validation results for cookie imports.
  - Updated TypeScript types to support new cookie import functionalities, including `DetectedBrowser` and `CookieImportResult`.
  - Refactored existing components to accommodate the new auto-import feature and improve user experience.

- Add embedded Apple Music login window service and UI integration

- Introduced `login_window_service` to manage Apple Music authentication via an embedded webview.
  - Updated `CookiesTab` and `CookiesStep` components to support direct login, including event handling for cookie extraction.
  - Enhanced user experience with loading states and manual extraction options.
  - Added Tauri commands for opening, extracting cookies from, and closing the login window.

- Add support for fetching extra metadata tags and update cover size to max resolution
- Add animated artwork download service for Apple Music

- Implemented `animated_artwork_service` to download animated cover art (motion artwork) from Apple Music's catalog API.
  - Added functionality to parse Apple Music URLs, generate MusicKit Developer Tokens, and download HLS streams using FFmpeg.
  - Integrated animated artwork download into the download queue process, allowing for background downloading after album downloads.
  - Updated settings UI to include options for enabling animated artwork downloads and entering MusicKit credentials (Team ID, Key ID, and private key).
  - Enhanced settings store to manage new animated artwork settings and added corresponding TypeScript types.
  - Added unit tests for URL parsing and JWT generation related to animated artwork functionality.

- Add metadata tagging service for M4A files

- Implemented `metadata_tag_service.rs` to inject custom codec metadata tags into downloaded M4A files.
  - Added tagging for ALAC (`isLossless = Y`) and Dolby Atmos (`SpatialType = Dolby Atmos`) in both Apple iTunes and MeedyaMeta namespaces.
  - Updated `mod.rs` to include the new metadata tagging service.
  - Bumped version to 0.2.1 in `tauri.conf.json`.
  - Enhanced `DownloadForm.tsx` to support new codec and video resolution types.
  - Introduced "Embed Lyrics and Keep Sidecar" toggle in `LyricsTab.tsx` for better lyrics management.
  - Added companion download mode settings in `QualityTab.tsx` to control automatic multi-format downloads.
  - Updated settings store to include new settings for companion mode and lyrics embedding.
  - Expanded type definitions in `index.ts` to include `CompanionMode` and associated labels.
  - Updated tests in `settingsStore.test.ts` to reflect new default settings.

- Implement queue persistence and export/import functionality

- Added queue persistence to save the download queue to disk after every mutation, enabling crash recovery.
  - Introduced export/import features for the download queue using a `.meedyadl` file format, allowing users to transfer their queue between devices.
  - Updated relevant documentation and user interface to reflect new features.
  - Enhanced the download queue management with improved state handling and user notifications.

- Enhance workflows with manual dispatch and update changelog for queue features
- Update project documentation with planned service integrations and milestones for Spotify, YouTube, and BBC iPlayer
- Add multi-track muxing feature to project plan and README
- Implement hidden animated artwork files feature with OS-level hiding options
- Release 0.3.5 with macOS signing validation and updated dependencies

- Added validation for required Apple signing secrets in the release workflow to prevent publishing unsigned binaries.
  - Updated version to 0.3.5 across various files including package.json, Cargo.toml, and tauri.conf.json.
  - Introduced Entitlements.plist for macOS hardened runtime permissions.
  - Enhanced Help documentation with a disclaimer regarding third-party dependencies.
  - Updated Tailwind CSS configuration to include typography plugin for improved styling.

- Add FallbackChainList component for reorderable priority lists

- Introduced a new generic component, FallbackChainList, for managing reorderable lists with up/down buttons.
  - Updated FallbackTab and QualityTab to utilize FallbackChainList for audio/video fallback chains and video codec priority respectively.
  - Enhanced type definitions for video codecs and added corresponding labels for UI representation.
  - Added support for displaying the source of installed tools in the DependenciesStep component.
  - Created tool-versions.toml to define minimum version requirements for external tools.
  - Added settings.json for permission configurations.

- Enhance audio codec handling and add help button for contextual assistance

- Added support for Dolby Digital (AC3) codec suffix in download queue.
  - Introduced new companion mode for Atmos to download all formats (AC3, ALAC, AAC).
  - Updated setup wizard to skip if dependencies are missing but setup has been completed.
  - Implemented HelpButton component for contextual help in Input, Select, and Toggle components.
  - Enhanced various settings tabs with help topics for better user guidance.
  - Improved validation and user feedback for cookie settings and sign-in processes.
  - Updated application branding from GAMDL to MeedyaDL in the sidebar and status bar.
  - Fetched application version dynamically from Tauri configuration.
  - Added setup_completed flag to settings store for persistent setup state.

- Add updater functionality for app updates with pre-release support

- Introduced updater permission set in macOS schema for frontend access.
  - Implemented `download_and_install_app_update` command to handle app updates.
  - Enhanced `check_all_updates` to include pre-release versions based on user settings.
  - Updated settings model to allow toggling of pre-release version checks.
  - Modified update checker to query GitHub Releases for both stable and pre-release versions.
  - Added UI components for downloading and installing updates, including progress tracking.
  - Integrated event listeners for real-time download progress updates in the frontend.
  - Updated settings UI to include a toggle for pre-release version checks.

- Add developer notes and update tauri configuration for updater plugin
- Add ReplayGain analysis and AcousticID fingerprinting services

- Introduced `replaygain_service` for analyzing audio loudness using FFmpeg's EBU R128 filter, writing non-destructive ReplayGain metadata tags.
  - Added `acoustid_service` for generating Chromaprint audio fingerprints and looking up AcousticID identifiers.
  - Updated `metadata_tag_service` to include new metadata enrichment features.
  - Enhanced `apple_music_api` for improved metadata retrieval from MusicKit.
  - Added new settings tab for metadata enrichment options, including toggles for AcousticID and ReplayGain.
  - Updated Zustand store to manage new settings for AcousticID and ReplayGain.
  - Added unit tests for new features and ensured existing tests cover new functionality.

- Implement manual queue processing and add auto-start settings
- Add temp directory setting and auto-start queue functionality
- Integrate embedded Chromaprint for AcousticID fingerprinting

- Replace external fpcalc dependency with the embedded rusty-chromaprint library for generating Chromaprint audio fingerprints.
  - Update documentation and comments to reflect the removal of external dependencies.
  - Modify settings and UI components to indicate the new fingerprinting method.
  - Implement fingerprint generation using Symphonia for audio decoding.
  - Enhance error handling for Python exceptions in the download queue process.
  - Add manual update check functionality in the settings UI.

- Add Activity Log component for live subprocess output and update download queue behavior
- **(i18n)** Add internationalization support with language detection and translations

- Added i18next and react-i18next for internationalization.
  - Implemented language detection and dynamic loading of translation files.
  - Created translation files for English, German, and French.
  - Updated AppSettings to include a UI language setting.
  - Enhanced settings UI to allow users to select their preferred language.
  - Introduced UpdatesPage component to display detailed update information with release notes.
  - Modified UpdateBanner to link to the UpdatesPage for more details.
  - Updated Sidebar navigation to include an Updates section.
  - Adjusted update checking logic to handle new update structures.

- Add non-fatal warnings to download items and update UI to display them
- Enhance download error handling and output processing

- Introduced codec and I/O error recovery strategies in process_queue.
  - Added ANSI escape code stripping for cleaner Activity Log output.
  - Implemented new utility functions to classify codec and I/O errors.
  - Updated tests to cover new error classification logic.

- Reorganize settings tabs and enhance tool management functionality
- Add multi-format lyrics support with companion downloads and update settings
- Enhance link handling in HelpViewer for internal and external navigation
- Implement custom macOS menu and update About section to display app version
- Add sys-locale dependency for localized Apple Music storefront detection
- Add platform asset validation for GitHub releases
- Refactor UpdateBanner integration in MainLayout and App components
- Integrate GAMDL v2.9.1 native codec priority, artist auto-select, and Apple Music Classical URLs

- Add version-aware codec fallback: GAMDL >= 2.9.1 uses native --song-codec-priority
    (all codecs tried in one process); older versions fall back to MeedyaDL's try_fallback system
  - Add ArtistAutoSelect enum (7 variants) with CLI arg and config.ini support
  - Add classical.apple.com URL support in frontend parser and backend regex patterns
  - Write dual config.ini keys (song_codec + song_codec_priority) for cross-version compatibility
  - Cache GAMDL version in DownloadQueue to avoid repeated pip show calls
  - Skip try_fallback() on both success and error paths when native priority was used
  - Clear song_codec_priority on companion and lyrics companion downloads (single-codec mode)

- Update app icons and logos

- Updated the MeedyaDL logo in both light and dark variants (SVG and PNG formats) with a new design and color scheme.
  - Added a new application icon (app-icon.svg) that combines a clapperboard, download arrow, and music note.
  - Updated the Sidebar component to use the new app icon instead of a placeholder.
  - Updated various icon sizes for Android and iOS platforms to reflect the new branding.

- Add new app icon variants and previews
- Enhance persistence of download queue items to include failed states
- Enhance queue persistence to include failed downloads for manual retry
- **(download)** Implement partial-success recovery for codec errors
- **(download)** Implement companion and lyrics downloads as background tasks
- **(activity-log)** Implement export functionality and wrapper connection test
- Add Enhanced LRC with word-by-word synchronized lyrics

Convert Apple Music TTML lyrics to Enhanced LRC format with inline
  word-level timestamps (<mm:ss.xx>) for karaoke-style highlighting.

  - New enhanced_lyrics_service.rs: TTML XML parser (roxmltree), word
    timestamp extraction, Enhanced LRC generation, M4A/M4V embedding
  - New enhanced_lrc setting (default: true) with TTML as default
    primary lyrics format and SRT as companion format
  - merge_options() Layer 4: forces TTML when Enhanced LRC is enabled
  - Enrichment pipeline Step 2: TTML → Enhanced LRC conversion
  - Frontend: Enhanced Lyrics toggle in Settings > Lyrics tab
  - Falls back to standard line-level LRC for songs without word data
  - Handles both iTunes namespace URIs and background vocals
  - 20 unit tests, all 339 tests passing, clippy clean
  - Version bump to v0.4.0

- Add pre-flight health checks and retry-without-wrapper

Add pre-flight health checks that run before queue processing begins:
  - Internet connectivity check (pings apple.com with 5s timeout)
  - Cookie validation (checks for valid, non-expired Apple Music cookies)
  - Wrapper health check (pings wrapper URL when enabled)

  Warnings are emitted as persistent toasts — non-blocking, queue proceeds
  regardless. Checks run once per batch with a 60-second cooldown.

  Add "Retry without Wrapper" action for failed downloads that used wrapper
  authentication, allowing users to fall back to cookie-based auth:
  - Pill button below error message + right-click context menu option
  - New retry_download_without_wrapper Tauri command
  - used_wrapper field on QueueItemStatus for conditional UI display

  Also fixes LyricsTab test failures (enhanced_lrc default + /LRC/ regex)
  and bumps version to 0.5.0.

- **(crash-reports)** Implement crash reporting system with Sentry integration

- Added `CrashReport` model to represent crash/error reports.
  - Created `crash_report_service` for managing crash report files.
  - Implemented IPC commands for listing, retrieving, deleting, exporting, and logging frontend errors.
  - Integrated `tracing` for structured logging and added support for Sentry error tracking.
  - Updated application settings to include `sentry_enabled` for opt-in telemetry.
  - Enhanced frontend error handling to persist errors to the Rust crash report system.
  - Added UI toggle in settings for enabling/disabling anonymous crash reporting.
  - Implemented automatic cleanup of old crash reports older than 30 days.

- Implement GitHub Issues crash reporting system

- Added a new crash reporting feature that allows users to report crashes directly to GitHub Issues from the app.
  - Introduced `CrashReportSection` and `CrashReportDialog` components for managing crash reports and user consent.
  - Implemented `get_github_issue_url` command to generate pre-filled GitHub issue URLs with crash report data.
  - Updated documentation to reflect the new crash reporting functionality and usage instructions.
  - Enhanced localization for crash reporting features in English, German, and French.
  - Added IPC commands for listing, deleting, and exporting crash reports.

- Add cookie validation before download to enhance user feedback
- Add pre-download internet connectivity check to prevent queuing downloads without internet
- Implement pre-download checks for internet connectivity and cookie validation, update queue processing behavior
- Enhance toast notifications with deduplication and clearing for preflight checks
- Add auto-retry without wrapper option for failed downloads
- Add pre-download connectivity check, toast notification deduplication, auto-retry without wrapper, and network error report suppression
- Add output path writability check before downloads

- Implemented `check_output_path_before_download` command to verify that the output directory is writable, catching issues like disconnected cloud mounts, full disks, and permission errors.
  - Integrated the new check into the download process in `DownloadForm.tsx`, ensuring downloads are only queued if the output path is accessible.
  - Updated settings model to include `update_check_interval_hours`, allowing users to specify how often to check for updates while the app is running.
  - Added UI components in the settings to configure the update check interval, visible only when auto-check for updates is enabled.
  - Enhanced logging and error handling in the download queue to provide more informative messages regarding fallback attempts and network errors.
  - Updated tests to cover new functionality and ensure existing features remain intact.

- Enhance internet connectivity check with multi-provider, multi-tier approach
- Enhance application stability and security

- Pin all release-critical GitHub Actions to immutable commit SHAs to prevent supply chain attacks.
  - Implement SHA-256 checksum verification for dependency downloads to ensure integrity.
  - Add graceful shutdown signal for background tasks to prevent orphaned processes on app exit.
  - Improve error handling in various components, including better logging and user notifications for failures.
  - Optimize regex usage in Apple Music URL parsing by using static instances to avoid recompilation.
  - Introduce log file cleanup for entries older than 7 days to manage disk space.
  - Enhance pre-download validation with multi-provider internet connectivity checks and cookie validation.
  - Update documentation and project plans to reflect new features and improvements.

- Expand activity log coverage and enhance logging throughout the app

- Expanded Activity Log to include app-wide events such as update checks, dependency installs, settings saves, cookie imports, queue operations, login window events, pre-flight check results, and app startup messages.
  - Implemented logging for cookie imports, Python and GAMDL installations, dependency installations, and queue operations.
  - Added system-level logging for app startup and pre-flight checks.
  - Introduced utility functions for emitting activity log events, centralizing logging logic.
  - Updated Activity Log component to display both download-specific and system-level events, improving user visibility into application activity.

- Add custom companion downloads and multi-select artist auto-select

Add Custom Companion mode (6th CompanionMode variant) with multi-select
  codec checkboxes, letting users pick exactly which audio formats to
  download as companions. Add multi-select artist auto-select that creates
  N separate queue items for artist URLs when multiple content types are
  selected. New CheckboxGroup<T> reusable component. Bump version to 0.6.0.

- Embed AcoustID API key in release builds for seamless fingerprinting
- Implement TemplateBuilder component for interactive GAMDL template editing
- Add music video companion downloads and visual template builder

Add music video companion downloads as enrichment Step 6: when enabled
  and MusicKit credentials are configured, queries Apple Music API for
  music video relationships after each audio download. Tracks with music
  videos get companion GAMDL downloads using video quality settings.
  Toggle in Settings > Quality > Video Quality, gated behind MusicKit
  credentials. Deduplicated by video ID.

  Add visual TemplateBuilder component replacing 7 plain text inputs in
  Settings > Templates with interactive chip/pill UI. Variables selected
  from dropdown menu; raw-edit toggle for power users; live preview.

  Update all documentation (CHANGELOG, Dev_Notes, Project_Plan, README,
  help files, CLAUDE.md, GitHub Wiki Features page). Close GitHub issue
  #81. Enhance inline code comments on new Rust functions.

- Add lyrics format fallback chain for incomplete lyrics coverage

When the primary lyrics format (TTML) doesn't produce lyrics for all
  tracks, automatically retries with fallback formats. Content-type-aware
  ordering: Audio (TTML → LRC → SRT), Video (TTML → SRT → LRC). Each
  fallback uses --synced-lyrics-only to avoid re-downloading media. Chain
  stops when lyrics coverage matches media file count.

  New setting: lyrics_fallback_enabled (default: true). Toggle in
  Settings > Lyrics. Integrated as enrichment Step 2b between Enhanced
  LRC conversion and Animated Artwork download.

- Add per-endpoint logging to pre-flight internet connectivity check

Each endpoint tested during the multi-tier internet check now logs its
  result with the endpoint name, URL, HTTP status (or failure reason), and
  response time. Tier progression is logged too (Tier 1 pass/fail, Tier 2
  skipped/tested). Example output:

    Pre-flight internet check: Cloudflare (https://1.1.1.1/) → reachable (200 OK, 12ms)
    Pre-flight internet check: Google (google.com) → skipped (Cloudflare passed)
    Pre-flight internet check: Apple Music API → reachable (401 Unauthorized, 45ms)

  Helps diagnose connectivity issues from log files without needing to
  reproduce the problem.

- Add internal codec and format registry infrastructure

Adds an internal registry for managing audio/video codecs and
  lyrics/subtitle formats via a TOML configuration file. Includes
  MIME types, format categories, and extensible mapping structure.
  Background preparation work for future planned features.

- Add terser-based JS obfuscation for production builds

Switches production minification from esbuild to terser with aggressive
  name mangling and code compression. Makes the compiled JavaScript in
  release builds significantly harder to reverse-engineer.

  Terser options:
  - Mangle top-level names and _-prefixed properties
  - Drop console.log/debugger statements in production
  - Two-pass compression for maximum size reduction
  - Strip all comments

  Zero runtime performance impact — all processing happens at build time.
  Debug builds are unaffected (minification disabled entirely).

- Add WebVTT subtitle generation from TTML, SRT, and LRC lyrics

Opt-in feature (Settings > Lyrics > Generate WebVTT Subtitles) that
  creates .vtt sidecar files from existing lyrics. Source priority:
  TTML (richest timing data), SRT (has start+end times), LRC (start
  times only, end times estimated from next cue).

  New webvtt_service.rs with ttml_to_webvtt(), srt_to_webvtt(), and
  lrc_to_webvtt() conversion functions. Integrated as enrichment Step
  2c (after lyrics fallback, before animated artwork). Skips tracks
  that already have .vtt files. 18 new unit tests.

- Mark all releases as pre-release until v1.0

All 50 existing GitHub releases marked as pre-release. Future releases
  from release.yml also default to prerelease: true. Users on the default
  setting (check_pre_releases: false) won't receive update notifications
  until a full release is published. Users who enable "Include Pre-Release
  Versions" in Settings > General will continue receiving updates.

  Added detailed pre-release vs full release workflow guide to Dev_Notes
  covering: standard pre-release pipeline, three methods to publish a
  full release (GitHub UI, workflow edit, CLI), and how the app update
  checker chooses between stable and pre-release channels.

- Add direct download links to release download table

Updated release.yml to generate download table with clickable links
  to each platform's asset file instead of plain text references. Added
  version extraction step (strips 'v' prefix from tag) for asset URLs.

  Also updated all 48 existing releases with download links via
  gh release edit. Links point directly to the release assets:
  https://github.com/REPO/releases/download/TAG/FILENAME

- Add platform emojis to release download table

Added platform identification emojis to the download table:
  - 🍎 Mac
  - 🪟 Windows
  - 🐧 Linux
  - 💻 Chromebook

  Updated release.yml template and all existing releases.

- Add MusicBrainz lookup service for video discovery and cross-platform groundwork

New musicbrainz_service.rs queries MusicBrainz database via ISRC codes
  to discover music videos and cross-platform URLs (Apple Music, YouTube,
  Spotify, Deezer, Tidal). No credentials required (free public API).

  Integrated as enrichment Step 6b — runs as fallback when MusicKit-based
  video lookup finds no results. Cross-platform URLs are logged for future
  use when additional service engines are added.

  Service is intentionally generic: returns all discovered platform URLs
  via HashMap, not just Apple Music. Groundwork for future "if unavailable
  on one platform, try another" cross-platform routing.

  New setting: musicbrainz_lookup (default: false). Toggle in Settings >
  Quality > Video Quality. Rate-limited to 1 req/sec per MusicBrainz ToS.
  10 new unit tests for URL classification and struct serialization.

- Enhance MusicBrainz with storefront awareness, ID lookup, AcoustID bridge

Three enhancements to the MusicBrainz discovery service:

  1. Storefront-aware Apple Music URLs: rewrite_apple_music_storefront()
     detects and replaces storefront codes (e.g., /de/ → /gb/) when
     MusicBrainz returns URLs for a different region.

  2. Direct recording-by-ID lookup: lookup_recording_by_id() enables
     MusicBrainz lookups when the recording ID is already known (e.g.,
     from AcoustID), skipping the ISRC search step entirely.

  3. AcoustID → MusicBrainz bridge: the AcoustID lookup now extracts
     MusicBrainz recording IDs from the API response (they were already
     present but not parsed).

  Also refactored relationship parsing into shared parse_recording_relations()
  function used by both ISRC and direct ID lookup paths. 8 new unit tests.

- 3-tier MusicBrainz discovery: URL → ISRC → AcoustID recording ID

Enhanced lookup_videos_for_tracks with a 3-tier priority chain:
  1. Apple Music URL search (most direct — searches MB external links)
  2. ISRC code search (reliable standard identifier)
  3. MusicBrainz recording ID direct lookup (from AcoustID fingerprinting)

  New functions:
  - lookup_recording_by_url() — searches MB for recordings with a specific
    external URL link (e.g., Apple Music song URL)
  - lookup_videos_for_tracks_enhanced() — uses TrackLookupInfo struct
    carrying all three discovery identifiers
  - TrackLookupInfo struct — carries song URL, ISRC, and MB recording ID

  The legacy lookup_videos_for_tracks() still works (converts to
  TrackLookupInfo internally). Each tier only fires if the previous
  tier found no results. Rate limiting enforced between all requests.

- Support non-geographic Apple Music URLs with storefront auto-detection

URLs without a storefront code (e.g., music.apple.com/album/...) are now
  automatically normalized by injecting a storefront based on OS locale
  (or "us" fallback). GAMDL requires a storefront in the URL path for its
  regex to match, but ignores it for API calls (uses cookies/wrapper auth).

  Two-layer approach:
  1. URL normalization at enqueue — normalize_apple_music_url() injects
     storefront before the URL enters the queue or reaches GAMDL
  2. Storefront fallback for enrichment — fetch_album_metadata_with_fallback()
     retries with alternative storefronts (OS locale, "us") when the primary
     returns HTTP 404, handling cross-region shared links

  Also normalizes URLs in queue imports (.meedyadl files) and logs
  normalization events to the activity log.

- Add rich SRT generation from TTML and subtitle embedding

Two new enrichment steps:

  Step 2d - Rich SRT generation (generate_rich_srt, default: true):
  Converts Apple Music TTML to format-rich SRT with HTML-like styling
  tags (<b>, <i>, <u>, <font color="...">). Extracts tts:fontWeight,
  tts:fontStyle, tts:textDecoration, tts:color attributes from both
  inline styles and named style definitions in <head><styling>. Style
  inheritance from <p> to <span> children supported. Background vocals
  (ttm:role="x-bg") wrapped in parentheses. Rich SRT overwrites any
  existing plain SRT since TTML has richer data.

  Step 2e - Subtitle embedding (embed_subtitles, default: false):
  Embeds SRT and WebVTT sidecar content into MP4/M4A/M4V containers
  as freeform atoms (com.apple.iTunes:subtitles-srt/subtitles-vtt).
  Uses existing mp4ameta pattern. Groundwork for multi-service support.

  New service: rich_srt_service.rs with 34 unit tests covering styling,
  colour normalization, timestamps, background vocals, style inheritance,
  named styles, and edge cases.

- Support WebVTT as rich SRT source alongside TTML

Rich SRT generation now uses a dual-source priority chain:
  1. TTML (richest — Apple Music, has tts:* styling attributes)
  2. WebVTT (also supports <b>, <i>, <u>, CSS class tags)

  This enables future services (YouTube/yt-dlp, BBC iPlayer) that provide
  WebVTT with styling to produce rich SRT output. The directory function
  now scans media files and finds matching source sidecars (like WebVTT
  service pattern) instead of scanning for .ttml files directly.

  New functions:
  - webvtt_to_rich_srt() — parses WebVTT cues, preserves SRT-compatible
    tags, strips VTT-only constructs (<c>, <v>, timestamps)
  - clean_vtt_tags() — filters tags by SRT compatibility
  - try_rich_srt_from_ttml/webvtt() — per-source helpers

  15 new unit tests (WebVTT conversion, tag cleaning, edge cases).

- Generate ASS subtitles from TTML and WebVTT with rich styling

New enrichment Step 2f generates ASS (Advanced SubStation Alpha) subtitle
  files from TTML or WebVTT sources with full styling support:

  - Colours: RGB #RRGGBB → ASS BGR &HBBGGRR& conversion
  - Text styling: bold ({\b1}), italic ({\i1}), underline ({\u1})
  - Dynamic positioning: tts:origin → {\pos(x,y)} override tags
  - Background vocals: ttm:role="x-bg" → dedicated "BgVocals" style
    (semi-transparent, italic, slightly smaller font)
  - Named style resolution from <head><styling> definitions
  - Style inheritance from <p> to <span> children

  Source priority: TTML first (richest, with tts:* attributes and
  positioning), then WebVTT (supports <b>, <i>, <u> inline tags).

  WebVTT tags are converted to ASS override equivalents:
    <b>text</b> → {\b1}text{\b0}
    <i>text</i> → {\i1}text{\i0}
  VTT-only tags (<c>, <v>, timestamps) are stripped.

  Reuses TTML style resolution from rich_srt_service via pub(crate)
  shared types and functions (TtmlStyle, resolve_named_styles, etc.).

  New service: ass_subtitle_service.rs with 37 unit tests.
  New setting: generate_ass: bool (default: false, opt-in).
  Toggle in Settings > Lyrics.

- Add verbose activity log toggle for detailed debugging

New `verbose_activity_log` setting (default: false) enables detailed
  [VERBOSE] messages in the Activity Log for issue tracking. When enabled,
  emits sensitive debugging information including full URLs, CLI arguments,
  error classification details, wrapper URLs (unredacted), cookie paths,
  and download settings.

- Parse and embed audioTraits from Apple Music API (#121 Phase 1)

Extract the audioTraits field from Apple Music API track responses
  and write it as metadata tags. This field is returned by default
  (no extend parameter needed) and indicates which audio formats are
  available for each track: lossy-stereo, lossless, hi-res-lossless,
  dolby-atmos, spatial.

- Comprehensive Apple Music metadata extraction, dual-namespace tags, config-driven tag system (tags.toml), and API field audit tool

- Extract all available Apple Music API metadata fields (20 track-level + 11 album-level)
  - Dual-namespace tagging: com.apple.iTunes (player-compatible) + MeedyaMeta (MeedyaDL-branded)
  - Industry standard alternative names: LABEL, COPYRIGHT, COMPILATION, TOTALTRACKS
  - Album scope uses Album* prefix; track scope uses no prefix (default context)
  - Config-driven tag definitions via tags.toml (28 entries) — zero Rust code changes for new tags
  - Tag registry module (tag_registry.rs) with JSON path extraction and value conversion
  - API field audit tool: fetch album, flatten JSON, diff against tags.toml, report unknown fields
  - Audit UI in Settings > Metadata tab (collapsible, requires MusicKit credentials)
  - 35 new tests (25 tag registry + 10 audit service), 551 total Rust tests passing

- Add isBinaural and isDownmix codec identification tags (#119)

Binaural (AAC Binaural, AAC-HE Binaural) and Downmix (AAC Downmix,
  AAC-HE Downmix) codec variants now get identification tags written
  to both com.apple.iTunes and MeedyaMeta namespaces:

  - isBinaural = Y (binaural spatial audio for headphones)
  - isDownmix = Y (stereo downmix of spatial/surround master)

  These codecs produce standard 2-channel AAC indistinguishable from
  regular stereo by audio analysis — codec identity at download time
  is the only way to classify them.

  Tags written in both apply_codec_metadata_tags() (companion downloads)
  and enrich_single_file() (enrichment pipeline Layer 1).

- Add content advisory suffixes ([Explicit]/[Clean]) to filenames and folder names

After metadata enrichment, album folders and track files are renamed with
  [Explicit] or [Clean] suffixes based on Apple Music content ratings. Per-track
  granularity (individual tracks can differ from album rating). Advisory suffix
  inserted before codec suffix (e.g., "01 Title [Explicit] [Lossless].m4a").
  Idempotent on re-download. Toggle in Settings > Metadata (default: enabled).

- Verbose settings logging and move API credentials to Advanced tab

- Verbose activity log now tracks which settings changed (key: old → new)
    with sensitive fields redacted (cookies, wrapper URL, MusicKit, AcoustID)
  - Verbose mode dumps key settings summary at startup for diagnostics
  - Move MusicKit credentials (Team ID, Key ID, Private Key, Test button)
    from Settings > Cover Art to Settings > Advanced > API Credentials
  - Move AcoustID API Key from Settings > Metadata to Settings > Advanced
    > API Credentials, with note linking from Metadata tab
  - Update all help file references to point to new credential locations

- Bump minimum compatible GAMDL version to 2.9.2

GAMDL 2.9.2 fixes artist download pagination. Bump MIN_COMPATIBLE_GAMDL
  from 2.0.0 to 2.9.2 so the update checker prompts users on older versions
  to upgrade.

- Change default remux mode to MP4Box for better subtitle handling and update app behavior on first launch
- Make verbose logging a session-only setting that resets on restart (#157)

Verbose logging can expose sensitive data (auth tokens, cookies, API
  responses, MusicKit credentials). As a safety measure, it now always
  resets to off on app startup — users must re-enable it each session.

  - Reset verbose_activity_log to false in load_settings() on startup
  - Add session-only note to toggle description and warning box in UI
  - Update settings.rs doc comment documenting session-only behavior
  - Update help/troubleshooting.md with session-only callout

- Add Linux app menu integration and suppress release-build terminal output (#159)

- Add custom .desktop file with proper Categories, Keywords, and
    Terminal=false for Linux application menu discoverability
  - Reference desktopTemplate in tauri.conf.json deb config
  - Suppress stderr tracing layer in release builds unless RUST_LOG
    is explicitly set — prevents terminal flooding on Raspberry Pi
    and other Linux systems when launched from command line

- Remove MusicKit credential gate from Music Video Companions (#160)

Music Video Companions no longer requires MusicKit credentials.
  MusicBrainz ISRC lookup (Step 6b) now serves as a credential-free
  discovery and download path for Apple Music videos. Step 6 (MusicKit
  API) still runs when credentials are available but gracefully skips
  when they are not.

  - Remove disabled prop and conditional description from toggle
  - Mark feature as Experimental with warning box when enabled
  - Step 6b now downloads Apple Music videos found via MusicBrainz
  - Step 6b runs when either musicbrainz_lookup OR music_video_companion
    is enabled
  - Extract download_music_video_by_url() shared helper
  - Update settings model docs and enrichment pipeline comments

- Migrate to Tailwind CSS v4 and update documentation

Migrate from Tailwind CSS v3.4.17 to v4.2.2 (closes #174):
  - Replace tailwindcss PostCSS plugin with @tailwindcss/postcss
  - Remove autoprefixer (built into v4's LightningCSS)
  - Replace @tailwind directives with @import "tailwindcss" + @config + @plugin
  - Load @tailwindcss/typography via @plugin in CSS instead of require() in JS
  - Bump macOS minimum from 11.0 to 13.3 (Safari 16.4+ required by v4)
  - Update Vite targets: safari13 → safari16.4, chrome105 → chrome111

  Documentation updates:
  - CHANGELOG.md: add all unreleased changes (security, stability, CI)
  - README.md: update Tailwind version and macOS minimum
  - Project_Plan.md: update status and add post-release entries
  - DEV_NOTES.md: update project structure references
  - CLAUDE.md: update architecture, build targets, Vite config
  - help/faq.md, help/getting-started.md: update macOS version

- Add cargo-deny for licence scanning and security advisory auditing in CI

Add cargo-deny configuration (deny.toml) and CI step to scan the Rust
  dependency tree for licence compliance and known security advisories.
  The config allows MIT-compatible licences, ignores Tauri's unmaintained
  GTK3 transitive dependencies, and pins the GitHub Action to a commit SHA.

- Core accessibility improvements (partial #125)

High-impact a11y improvements across the UI:
  - ARIA labels on icon-only buttons (Sidebar, UpdateBanner, QueueItem)
  - aria-live regions for toasts, activity log, and progress bars
  - prefers-reduced-motion media query disabling animations
  - Skip navigation link for keyboard users (WCAG 2.1 SC 2.4.1)
  - ProgressBar role="progressbar" with proper value attributes

- Upgrade dependencies and add queue progress indicators

Dependency upgrades (closes #117):
  - @vitejs/plugin-react 4.7.0 → 5.2.0
  - @commitlint/cli 19.8.1 → 20.5.0
  - react-markdown 9.1.0 → 10.1.0
  - All semver-compatible updates applied

  Queue progress indicators (closes #178):
  - Add queue header statistics bar with active/queued/completed/failed
    counts and aggregate progress bar
  - Add "Track N of M" counter in QueueItem for album downloads
  - Both derived from existing store data, no backend changes needed

- Add global keyboard shortcuts (closes #179)

Add useKeyboardShortcuts hook with application-wide shortcuts:
  - Cmd/Ctrl+D: navigate to Download page and focus URL input
  - Cmd/Ctrl+,: navigate to Settings
  - Cmd/Ctrl+Q: navigate to Queue
  - Escape and Cmd+Enter already handled by Modal and DownloadForm

  Shortcuts suppressed when focus is in input/textarea/select fields.
  Uses imperative store access to avoid unnecessary re-renders.

- Add high-contrast accessibility theme (closes #180)

Add a toggleable high-contrast theme for users with low vision:
  - Pure black/white text with WCAG AA+ contrast ratios
  - Strong opaque borders replacing translucent ones
  - Saturated status colours for clear differentiation
  - 3px focus-visible outlines on all interactive elements
  - Supports both light and dark mode simultaneously
  - Auto-detects OS prefers-contrast: high media query
  - Toggle in Settings > General > Appearance

- Add colour blindness accessibility themes (closes #181)

Add three colour vision deficiency (CVD) theme variants:
  - Deuteranopia (red-green): success→blue, error→orange, warning→yellow
  - Protanopia (red-green): same palette as deuteranopia
  - Tritanopia (blue-yellow): warning→pink, info→teal

  Each variant overrides status colours in both light and dark mode.
  Select in Settings > General > Appearance > Colour Vision dropdown.

- Move progress bars to global layout — visible on all pages

Add GlobalProgressBar component to MainLayout, rendered between <main>
  and StatusBar. Always visible regardless of which page the user is on:
  - Upper bar: per-item progress (current track name, speed, ETA)
  - Lower bar: queue-level progress (completed / total items)
  - Auto-hides when no downloads are active or queued

  Remove duplicate ProgressBar from DownloadQueue page header (text
  stats retained for context on the Queue page).

- Enhance codec handling by adding AC3 support and refining suffix application
- Add storefront as user-configurable setting

Add explicit storefront field to AppSettings so users can set their
  Apple Music region (e.g., gb, us, jp) directly.

- Log key settings on startup and include in crash reports (closes #203)

Activity Log: emit 3 concise [System] entries on every startup:
  - Config: codec, video resolution, companion mode, storefront, download mode
  - Features: enhanced_lrc, advisory_suffixes, acoustid, replaygain, musicbrainz
  - Auth: wrapper status, cookies presence, musickit configuration

  Crash Reports: add settings_snapshot_for_context() helper that populates
  crash report context with redacted settings (no paths/credentials).
  Integrated into both error handler sites in download_queue.rs.

- Add settings export/import for backup and device transfer (closes #202)
- Add keyboard shortcuts help topic (closes #201)

Add 'Keyboard Shortcuts' to the in-app HelpViewer with:
  - Full shortcuts table (Cmd/Ctrl+D, Cmd+,, Cmd+Q, Escape, Cmd+Enter)
  - Platform-specific modifier key notes (Cmd on macOS, Ctrl on Win/Linux)
  - Modal shortcuts (Escape, Tab focus trapping)
  - Accessibility navigation (Tab, Shift+Tab, skip link)

- Add download statistics panel on Activity page (closes #198)

Session-based stats derived from queue items via useMemo:
  - Total downloads, success rate (green/amber/red), top codec
  - Active/Queued/Completed/Failed counts with status colours
  - Collapsed by default, hidden when queue empty

  Full historical stats will follow #196 (download history database).

- Add meedyadl:// deep link URL scheme (closes #200)

Register custom URL scheme via tauri-plugin-deep-link:
  - meedyadl://download?url=<apple_music_url>&codec=<optional>
  - Handles both running-app (on_open_url) and cold-start (get_current)
  - Pre-fills download form URL input and navigates to Download page
  - Brings main window to foreground on deep link receipt
  - Activity log entry for received deep links

- Add activity log search and category filtering (closes #199)

- Search input with clear button for case-insensitive text filtering
  - Category toggles: System (on), Download (on), Verbose (off by default)
  - Filtered count shown in subtitle when filters active
  - Empty state message when no entries match
  - Export still exports all entries regardless of filter
  - ARIA role="checkbox" with aria-checked on filter toggles

- Add duplicate URL detection in download queue (closes #197)

- normalize_url_for_dedup(): lowercase domain, strip trailing slashes,
    fragments, and non-essential query params (keeps ?i= for track IDs)
  - has_duplicate_urls(): checks against active/queued items only
  - StartDownloadResult struct replaces plain string return from start_download
  - Frontend shows warning toast for duplicates (non-blocking)
  - 13 new unit tests for normalisation and duplicate detection

- Add persistent download history page (closes #196)

JSON-based history database at {app_data_dir}/history.json:
  - Records URL, title, artist, album, codec, file path, timestamps
  - Max 1000 entries with oldest trimmed
  - Search via Rust backend (case-insensitive on title/artist/album/URL)

  New History page (sidebar nav between Queue and Activity):
  - Search input with 300ms debounce
  - Status icons (success/failed), codec badges, dates
  - "Open Folder" action for successful downloads
  - "Clear History" button
  - 3 new Rust unit tests (639 total)

- Add drag-and-drop URL input from browser (closes #195)

Drag Apple Music URLs from any browser directly into MeedyaDL:
  - Drop-zone overlay with semi-transparent backdrop and dashed border
  - Extracts URL from text/uri-list or text/plain data transfer
  - Validates via parseAppleMusicUrl, navigates to Download page
  - Nested dragenter/dragleave counter prevents overlay flicker
  - Success/error toasts for valid/invalid URLs

- Add batch URL paste — queue multiple Apple Music URLs at once (closes #194)

Replace the single-line URL input with an auto-resizing textarea that
  supports pasting multiple Apple Music URLs (one per line). When multiple
  URLs are detected, each is validated individually and submitted as a
  separate queue item. The badge shows "N URLs" count instead of content
  type. Summary toast reports queued/failed/skipped counts. Quality
  overrides apply to all URLs in the batch. Single-URL flow is unchanged.

- Add native OS desktop notifications (closes #193)

Integrate tauri-plugin-notification for download events:
  - "Download Complete" notification on successful download
  - "Download Failed" notification on terminal failure
  - Suppressed when app window is focused (background only)
  - desktop_notifications setting (default: true) in Settings > General
  - Backend-driven via send_desktop_notification() helper

- Add settings sidebar sub-categories (closes #207)

Group settings tabs under 4 section headers:
  - General: General
  - Download: Quality, Fallback, Lyrics, Cover Art, Metadata, Templates
  - Authentication: Cookies
  - System: Tools, Advanced

  Section headers: 10px uppercase, muted colour, non-interactive.
  Prepares for per-service settings groups in multi-service architecture.

- Add pre-release verbose log persistence, collapsible About sections, component versions, and fix release table formatting

- Version-aware verbose_activity_log: pre-release (v0.x) preserves setting
    across restarts; full releases reset to false on startup (closes #216)
  - Pre-release first-load notice modal shown on each new pre-release version
    launch with option to install stable release if available
  - last_seen_version field in AppSettings for version change detection
  - Collapsible Help > About sub-sections using <details>/<summary> HTML
    elements, collapsed by default (closes #214)
  - Component Library section in About shows dynamic version table for all
    installed components (Python, GAMDL, FFmpeg, etc.) (closes #215)
  - get_component_versions IPC command returns version info for all components
  - Component versions logged to Activity Log at app startup
  - Fix release.yml finalize-release job to include platform emojis and
    direct download links in release tables (was missing since v0.6.5)
  - Add rehype-raw dependency for HTML-in-markdown support
  - CSS styles for collapsible <details>/<summary> disclosure elements
  - GitHub Issues created: #213 (auto-delete crash reports), #214 (collapsible
    About), #215 (component versions), #216 (verbose log), #217 (logo redesign)

- Add MeedyaDL logo SVGs, fix help version, close duplicate issues

- New logo.svg and logotype.svg in assets/brand/new/ (closes #217)
    - Animated SVG with CSS custom properties for customisation
    - prefers-reduced-motion support
    - Descriptively named elements
  - Fix help/index.md version from 0.1.3 to 0.10.0
  - Closed duplicate/superseded GitHub issues: #205, #208, #209, #210, #211

- Logo crossfades between vinyl disc and film projector

Redesign the logo to alternate between two media symbols:
    - Vinyl disc (audio): rotating grooves, label area, centre hole
    - Film projector (video): dual reels with spinning spokes,
      lens barrel, flickering beam cone, film strip detail

  The two symbols crossfade on an 8s cycle (customisable via
  --logo-transition-speed). Each is visible for ~40% of the cycle
  with smooth 10% crossfade overlaps. When reduced-motion is active,
  the vinyl disc is shown statically.

  Natural animations:
    - Disc grooves rotate continuously
    - Projector reels spin (top and bottom at different speeds)
    - Projector beam flickers with irregular steps
    - Download arrow bounces

  Same colour/mode system as logotype.svg:
    - CSS custom properties for all colours
    - ?mode= URL parameter (light/dark/cb-deutan/etc.)
    - prefers-color-scheme and prefers-reduced-motion
    - Drop shadows per layer

- Rebuild logo.svg with full colour mode system and drop shadows

Promoted concept D to main logo.svg with complete implementation:
  - Disc/reel at r=195, vinyl internals r=190
  - Drop shadows on: emblem (disc-shadow), outer glow (for dark bg),
    chevron groups (chev-shadow) - all use CSS var(--logo-shadow/glow)
  - Full colour mode system matching logotype.svg:
    light (default), dark (@media + .dark class),
    cb-deutan/protan/tritan (light + dark variants)
  - ?mode= URL parameter support via embedded script
  - SVG has no fixed width/height - expands to fill container
  - All colours use CSS custom properties (no hardcoded colours
    outside the vinyl black surface)
  - Vinyl and reel each in their own wrapper group with clip-path
  - prefers-reduced-motion disables all animations

- Generate APNG animations from logo and logotype SVGs

New script scripts/svg-to-apng.mjs:
  - Renders SVG animations frame-by-frame via headless Chromium (puppeteer)
  - Captures with omitBackground for full alpha transparency
  - Assembles frames into APNG via ffmpeg
  - 15 FPS, 8-second cycle (120 frames per animation)

  Output files:
  - assets/brand/new/logo.apng (15 MB, 512x512, vinyl/reel crossfade)
  - assets/brand/new/logotype.apng (4 MB, 600x130, gradient shimmer)

  Both have full alpha transparency and loop infinitely.
  Run: node scripts/svg-to-apng.mjs to regenerate.

- Promote logo_new2 to logo.svg, align logotype dark/CB colours, add test page
- Generate animated PNG for all 8 colour modes, .png extension

Replaces the old .apng files with .png-extension animated PNGs for
  compatibility. Generates 16 files total (2 SVGs x 8 modes):

  Logo (512x512, 15fps, 8s cycle, vinyl/reel crossfade + chevrons):
    logo.png, logo-dark.png, logo-cb-deutan.png, logo-cb-protan.png,
    logo-cb-tritan.png, logo-cb-deutan-dark.png, logo-cb-protan-dark.png,
    logo-cb-tritan-dark.png

  Logotype (485x99 trimmed, 15fps, 8s cycle, gradient shimmer):
    logotype.png, logotype-dark.png, logotype-cb-deutan.png,
    logotype-cb-protan.png, logotype-cb-tritan.png,
    logotype-cb-deutan-dark.png, logotype-cb-protan-dark.png,
    logotype-cb-tritan-dark.png

  All files have full alpha transparency, content-aware trimming,
  and infinite looping. Mode colours applied via inline styles in
  the puppeteer renderer for reliable cross-browser support.

- Add split disc/reel app icon with all platform formats

New icon.svg: static split design with left-half vinyl record and
  right-half film reel, clipped via SVG clipPath. No animations or
  chevrons — designed for app icons, favicons, and tray icons.

  Generated formats:
    icon.png              — 1024x1024 static PNG (281 KB)
    icon.ico              — Windows ICO, 16-256px (62 KB)
    favicon.ico           — Web favicon, 16/32/48px (6 KB)
    icon.icns             — macOS ICNS via iconutil (798 KB)
    icon-liquidglass.png  — Apple Liquid Glass, 10% inset (332 KB)
    icon-liquidglass.icns — Apple Liquid Glass ICNS (790 KB)

  All have full alpha transparency. Regenerate with:
    node scripts/generate-icons.mjs

- Add brand kit page and icon previews to test page

brandkit.html — comprehensive brand reference including:
    - Logo section with all 8 mode variants (light/dark/CB)
    - Logotype section with all 8 mode variants
    - App icon section (PNG, ICO, ICNS, Liquid Glass)
    - Full colour palette (light, dark, 3x colour-blind)
    - Typography reference (Orbitron + Rajdhani)
    - MeedyaSuite product name variants
    - Complete file reference table with sizes and use cases
    - Customisation methods (URL param, hash, class, JS)
    - Regeneration script commands
    - Adapts to system dark mode via prefers-color-scheme

  logo.html — added icon section with:
    - icon.svg on light and dark backgrounds
    - icon.png static preview
    - Liquid Glass on light and dark backgrounds
    - Favicon size previews (48/32/16px)

- Generate icon variants for all 8 colour modes, update copyright to 2026
- Restructure brand assets, wire new icons, proprietary license

Brand restructure:
  - Copied brand assets from assets/brand/new/ to assets/brand/
  - Deleted logo.html (replaced by brandkit.html)
  - Updated SVG license headers from MIT to proprietary:
    "All rights reserved. MeedyaDL brand assets are proprietary."

  Tauri icons regenerated from new icon.svg:
  - All standard sizes (32-512px) + @2x variants
  - Windows Store logos (Square30-310px + StoreLogo)
  - iOS AppIcon set (20-512@2x)
  - Android mipmap set (mdpi-xxxhdpi)
  - icon.ico and icon.icns replaced

  Web integration:
  - New favicon.ico and app-icon.svg copied to public/
  - index.html: added ICO fallback alongside SVG favicon
  - tauri.conf.json icon paths unchanged (already correct)

- Consolidate brand assets, wire animated SVGs into sidebar

Brand asset consolidation:
  - Removed assets/brand/new/ (duplicate of assets/brand/)
  - Removed assets/icons/app-icon.svg (replaced by assets/brand/)
  - Updated scripts/svg-to-apng.mjs and generate-icons.mjs to use
    assets/brand/ instead of assets/brand/new/

  Sidebar branding:
  - Replaced static <img> icon with animated <object> logo.svg
    (vinyl/reel crossfade, auto dark mode, colour-blind aware)
  - Replaced text "MeedyaDL" with animated <object> logotype.svg
    (gradient shimmer, bracket flash, dot pulse)
  - Both use <object> for full SVG animation support with fallback
    content (static icon PNG / text) for non-SVG contexts
  - pointer-events-none prevents interference with drag regions
  - SVGs auto-detect dark mode via @media(prefers-color-scheme)

  Public assets:
  - Copied logo.svg and logotype.svg to public/ for web runtime access

- Dynamic brand theming — sidebar respects colour-blind mode (closes #220)

Read colour_blind_mode from settings store and pass as ?mode= query
  parameter to the sidebar logo.svg and logotype.svg <img> tags.

  When a colour-blind mode is active (deuteranopia/protanopia/tritanopia),
  the SVGs render with the corresponding accessible palette. The mode
  parameter is processed by the SVGs' embedded JavaScript.

  Dark mode continues to be handled automatically via @media
  (prefers-color-scheme: dark) in the SVG CSS.

- Double sidebar logo and logotype size, expand logotype to fill width
- Accept Apple Music personal library URLs (#243)

Library URLs (e.g., music.apple.com/library/albums/l.8zPXbAv) were
  rejected by the frontend URL parser. Added 'library' content type with
  /library/ path detection, Library icon, and label. URLs pass through
  to GAMDL as-is; enrichment naturally skips non-catalog URLs.

- Enhance GlobalProgressBar to display download percentage alongside speed and ETA
- Improve activity log — remove entry cap, timestamped export filename

- Remove 5,000 entry cap; log grows unbounded per session, resets on restart
  - Export filename now includes date/time: MeedyaDL-activity-log_YYYY-MM-DD_HHhMMm.log

- Add platform icon to GlobalProgressBar

Shows an Apple Music icon next to the track name in the per-item
  progress bar. Uses inline SVG with a detectPlatform() helper and
  PLATFORM_ICONS lookup, extensible for future services.

- Embed .meedyadl manifest file in album download folders (#245)

After enrichment, writes a `.meedyadl` JSON manifest to each album
  output directory. Records source URL, platform, storefront, codec,
  and per-track metadata (ISRC, title, individual URLs). Supports
  multi-platform source merging — new platforms append to the existing
  manifest without overwriting.

- Generate .meedyadl document type icon in PNG/ICO/ICNS formats

Generated from assets/brand/icon-doc.svg (split disc/reel design).
  Icon files ready for platform-specific file association wiring:
  - icon-doc.png (512px) — cross-platform fallback
  - icon-doc.icns — macOS CFBundleTypeIconFile
  - icon-doc.ico — Windows registry association

  Tauri v2 doesn't expose fileAssociations.icon yet — platform-specific
  wiring deferred to follow-up issue.

- Add .meedyadl document type icon SVG source

Split disc/reel design matching the MeedyaDL brand. Source file for
  the PNG/ICO/ICNS variants already committed. Brand asset (proprietary).

- Wire up .meedyadl manifest import UI (#247)

- "Import" button on Download page: opens native file picker for
    .meedyadl files, populates URL textarea with source URLs
  - Drag-and-drop: .meedyadl files dropped on the app are parsed and
    URLs populated (alongside existing Apple Music URL drop support)
  - Deep link / file association: multi-source manifests now emit all
    URLs joined by newlines (not just the first) for batch queueing

- Clear all queue with confirmation, wrapper logging, CVD debugging (#248)

- "Clear All" button on Queue page with confirmation modal. Removes all
    non-active items (queued, completed, cancelled, errored). Active
    downloads are preserved. Uses clear_all() in DownloadQueue + IPC.
  - Wrapper authentication status now emitted to user-visible Activity Log
    at download start: "Authentication: Wrapper ({url})" or
    "Authentication: Cookie-based (no wrapper)".
  - CVD (colour blind) modes verified working — CSS bundle confirmed to
    contain all 9 CVD selectors. Added console.debug logging to useTheme
    hook for easier troubleshooting.
  - Animated artwork confirmed independent of wrapper (uses MusicKit JWT).

- Integrate MediaInfo CLI for accurate codec detection (#246)

- Added MediaInfo as 5th managed tool in dependency_manager (optional)
  - tool-versions.toml: [mediainfo] section (min v22.0)
  - New mediainfo_service.rs: JSON parser for mediainfo --Output=JSON
    with definitive Atmos detection (Format_AdditionalFeatures: "JOC")
  - metadata_tag_service.rs: MediaInfo primary, ffprobe fallback
    for codec detection in enrichment Step 1
  - URL resolver for macOS (DMG/.pkg), Windows (ZIP), Linux (mirror)
  - 8 unit tests for codec classification (Atmos, AC3, ALAC, AAC, HE-AAC)
  - Setup Wizard auto-detects MediaInfo via get_all_tools()

- Add SpatialAudioCodec ISRC annotation for Atmos/AC3 tracks (#121)

When the detected codec is Atmos, AC3, or Binaural, writes a
  MeedyaDL:SpatialAudioCodec freeform atom to the file. This marks
  the ISRC as belonging to the spatial version of the track, enabling
  future cross-platform ISRC matching for spatial audio variants.

- Enhance empty state messages with icons and improved guidance (#251)
- Add copy-to-clipboard button for Activity Log entries (#255)

Each log entry now shows a small copy icon on hover (top-right corner).
  Clicking copies the entry's line content to the clipboard. Uses
  group-hover opacity transition for non-intrusive discoverability.

- Add keyboard shortcuts help page (#252)

New help/keyboard-shortcuts.md documenting all navigation (Cmd+D,
  Cmd+comma, Cmd+Q) and action shortcuts (Enter, Shift+Enter, Escape).
  Added to help index under new "Reference" section.

- Add i18n translation keys for download, queue, activity, history (#111)

Added translation keys for:
  - Download page: URL label, content types, import manifest, validation
  - Queue page: empty state, clear all/completed, start/export/refresh
  - Activity Log: empty state, export, pause/resume, copy entry
  - History page: empty state, clear history

  Components still use hardcoded English — wiring useTranslation() to
  these keys is incremental follow-up work.

- Wire useTranslation() to Sidebar navigation and footer (#111)

Nav item labels now use t('nav.{page}') with fallback to static label.
  Footer status text ("Ready"/"Setup Required") and update button text
  ("Check for Updates"/"Checking..."/"N Updates") use translation keys.

  First component to use react-i18next — establishes the pattern for
  incremental i18n wiring across the rest of the UI.

- Aggregate release notes across multi-version jumps in update checker [skip ci]

When a user jumps multiple versions (e.g., v0.13.0 → v0.15.0), the Updates
  page now shows combined release notes from all intermediate versions, not
  just the latest. Fetches up to 20 releases from GitHub API and filters to
  those newer than current_version. Bodies are concatenated newest-first with
  horizontal rule separators.

  Also adds Animated Cover Art developer documentation to DEV_NOTES.md.

- Add per-track separators and enhanced download headers in activity log

Improves activity log readability with three changes:

  1. Download start separator now includes codec and auth method:
     "Starting download: {URL}" + "Codec: atmos | Auth: wrapper"

  2. Per-track markers emitted when GAMDL starts each track:
     "──── Track 1/28: The Virginia Company ────"
     These appear as internal (accent-coloured) lines between the
     noisy [download] HLS fragment progress, making it easy to
     identify which track's progress lines belong to which song.

  3. Companion and enrichment phase separators:
     "──── Companion downloads (mode: Custom) ────"
     "──── Enrichment starting (lrc: on, artwork: on, ...) ────"

- Smart re-download detection via Apple Music API lastModifiedDate (#263)

Detects whether an album has changed since the user's last download
  by comparing the Apple Music API's lastModifiedDate timestamp against
  the value stored in the .meedyadl manifest.

- Add engines.toml for per-platform engine priority registry (#268)

New config-driven registry defining available download engines and
  their per-platform priority ordering. Follows the same pattern as
  codecs.toml and tags.toml — compiled into binary via include_str!,
  editable without code changes.

  Defines 5 engines (GAMDL, votify, yt-dlp, get_iplayer, OF-Scraper)
  and 6 platforms (Apple Music, Spotify, YouTube, YouTube Music,
  BBC iPlayer, OnlyFans). BBC iPlayer uses get_iplayer as primary
  with yt-dlp as fallback.

  Runtime parsing and Rust model will be implemented as part of #107
  (multi-service architecture).

- Embed Votify and OF-Scraper as pip engines with required/enabled flags (#268)

Adds engine lifecycle management for pip-based download engines:

  1. engines.toml: Added `required` and `enabled` fields to both engines
     and platforms. Votify is required+enabled, OF-Scraper is optional+
     disabled (hidden until OnlyFans support is implemented). yt-dlp and
     get_iplayer are also defined but disabled.

  2. pip_engine_service.rs: Generic service for install/version-check/
     uninstall of any pip package. Generalises the gamdl_service pattern
     so new engines need zero new Rust service code.

  3. IPC commands: check_votify_status, install_votify, check_ofscraper_status,
     install_ofscraper — registered in lib.rs with TypeScript bindings.

  4. Frontend: checkVotifyStatus(), installVotify(), checkOfscraperStatus(),
     installOfscraper() in tauri-commands.ts.

- Auto-update checking for all enabled pip engines (#272)

Extends the update checker to monitor all pip-based engines defined
  in engines.toml, not just GAMDL. On each update check, parses
  engines.toml for enabled engines with install_method="pip", queries
  PyPI for the latest version, and compares against installed version.

  New components:
  - pip_engine_service::check_latest_pypi_version() — PyPI JSON API query
  - update_checker::get_enabled_pip_engines() — engines.toml parser
  - update_checker::check_pip_engine_update() — per-engine update check
  - commands::updates::upgrade_pip_engine() — generic pip upgrade IPC
  - upgradePipEngine() TypeScript binding

  Currently checks: votify (enabled=true). yt-dlp, get_iplayer, and
  OF-Scraper are disabled in engines.toml and skipped automatically.
  GAMDL retains its own check with compatibility gating.

- Aggregate engine updates into generic UI message, hide individual names

Engine/component updates (votify, yt-dlp, etc.) are now shown as a
  single "Component updates available" card instead of individual rows
  with version details. This avoids revealing specific tool names to
  end users and keeps the UI simple.

  - UpdatesPage: core updates (MeedyaDL, GAMDL, Python) shown with full
    detail; engine updates aggregated into one card with "Update All"
  - UpdateBanner: engine updates shown as "Component updates are also
    available" with a link to the Updates page
  - No changelog/release body shown for engine updates (already None)

- Config-driven platform icons in progress bar with favicon fallback

Replaces the hardcoded Apple Music inline SVG with a data-driven
  platform icon system:

  1. engines.toml: Added `icon` field to each platform pointing to
     local SVG/PNG in public/icons/platforms/. Documentation explains
     how to add icons for new platforms.

  2. GlobalProgressBar.tsx: PLATFORM_CONFIG array maps URL hostnames to
     platform IDs, icon paths, and favicon fallback hosts. detectPlatform()
     uses hostname matching. PlatformIcon component loads the local SVG
     first, falls back to Google Favicon API (returns PNG) on error.

  3. Platform icon assets: apple-music.svg and spotify.svg added to
     public/icons/platforms/. Other platforms will use favicon fallback
     until custom icons are created.

  To add a new platform icon: save a 16x16 SVG/PNG to
  public/icons/platforms/{id}.svg and set the path in engines.toml.

- Theme-adaptive platform icons using currentColor + inline SVG rendering

Platform SVG icons now use fill="currentColor" instead of hardcoded
  colours. PlatformIcon component fetches the SVG and renders it inline
  (not as <img>) so currentColor inherits from the parent CSS context,
  automatically adapting to light, dark, and colour-blind themes.

  SVG content is cached in a module-level Map to avoid re-fetching.
  Fallback: Google Favicon API (PNG) when local SVG unavailable.

  Updated apple-music.svg and spotify.svg to use currentColor.
  Added platform icon documentation to DEV_NOTES.md covering the
  theme adaptability approach, fallback chain, SVG template, and
  step-by-step guide for adding new platform icons.

- Add BBC Sounds platform icon and path-based platform detection

Adds bbc-sounds.svg (headphones icon, currentColor for theme
  adaptability). Platform detection now supports pathContains for
  disambiguating services on the same host (e.g., bbc.co.uk/sounds
  vs bbc.co.uk/iplayer).

- Dynamic platform config from engines.toml, BBC iPlayer + Sounds icons

Platform detection and icon rendering in GlobalProgressBar is now
  fully driven by engines.toml — no more hardcoded PLATFORM_CONFIG
  array. The component loads platform config once via the new
  get_platform_config IPC command, which parses the compiled-in
  engines.toml and returns enabled platforms with URL patterns and
  icon paths.

  Adding a new platform with its icon now requires ONLY:
  1. Add the [platforms.*] entry to engines.toml
  2. Drop an SVG into public/icons/platforms/

  No GlobalProgressBar.tsx changes needed.

  Also adds:
  - bbc-iplayer.svg (TV screen with play button, currentColor)
  - bbc-sounds.svg (concentric sound waves, currentColor)
  - get_platform_config() Rust command + TypeScript binding

- Add YouTube, YouTube Music, and OF-Scraper platform icons

Prepared ahead of multi-service expansion:
  - youtube.svg: rounded rectangle with play triangle
  - youtube-music.svg: circle with record disc + play triangle
  - ofscraper.svg: padlock (generic subscription content icon)

  All use currentColor with fill-opacity for theme adaptability
  (light, dark, colour-blind modes). Icons are referenced by
  engines.toml and loaded dynamically via get_platform_config IPC.

- Fix Apple Music API auth and add word-level lyrics fetching (#300)

- Fix API endpoint from amp-api.music.apple.com to api.music.apple.com for MusicKit JWT auth (Resolves #299)
  - Add direct syllable-lyrics fetching via /syllable-lyrics endpoint with dual auth (MusicKit JWT + Music-User-Token) (Resolves #298)
  - Remove unnecessary Origin header from API requests
  - New enrichment Step 1b: automatically upgrades TTML to word-level timing when available

- Fetch word-level lyrics directly from Apple Music API

Adds direct syllable-lyrics fetching via the Apple Music MusicKit API
  (/syllable-lyrics endpoint) to obtain word-by-word TTML timing data.
  When GAMDL's TTML files lack word-level timing, the enrichment pipeline
  now fetches upgraded TTML directly before Enhanced LRC conversion.

  Requires both MusicKit developer credentials and an active Apple Music
  subscription (Music-User-Token extracted from imported browser cookies).

- Syllable-lyrics follow-up enhancements (#307)

- Add 7 unit tests for extract_media_user_token() cookie parsing\n- Add activity log entries when cookies are expired or missing\n- Add progress bar label for credential skip path\n- Deduplicate media-user-token cookie name constant across modules\n- Add fetch_syllable_lyrics IPC command for standalone lyrics fetching\n\nResolves #306

- Dump raw Apple Music API response JSON when verbose logging is enabled

Writes `<AlbumName>-applemusic-data.json` to the album output directory
  during enrichment Step 1a when verbose_activity_log is on. This allows
  developers to verify the API integration is returning correct data after
  endpoint changes (e.g., amp-api → api.music.apple.com). Album names are
  sanitized for cross-platform filesystem safety.


### 🐛 Bug Fixes

- Resolve ESLint no-explicit-any error in Modal.test.tsx

Replace `any` type with `Record<string, unknown>` for the lucide-react
  X icon mock props to satisfy @typescript-eslint/no-explicit-any rule.

- Make usePlatform fallback test deterministic across CI runners

Mock navigator.userAgent with a known Windows UA string instead of
  relying on the host platform's default jsdom userAgent. This fixes
  the test failure on Ubuntu runners where the userAgent contains
  "linux" instead of "darwin".

- Resolve blank screen on macOS/Windows release builds

Fix React infinite re-render loop (error #185) that caused the UI to
  flash briefly then go blank in production builds. Three root causes:

  1. UpdateBanner: Zustand selector called getActiveUpdates() which uses
     .filter(), creating a new array reference on every store change.
     Zustand's Object.is() equality check always saw a new reference,
     triggering cascading re-renders. Fixed by subscribing to raw data
     (lastResult, dismissed) and deriving via useMemo.

  2. Sidebar: Subscribed to isReady function reference (always stable)
     instead of actual dependency state. The status dot never updated.
     Fixed by subscribing to python/gamdl status objects directly.

  3. App.tsx: Subscribed to entire settings object, causing full subtree
     re-renders on any settings change. Narrowed to sidebar_collapsed.
     Also replaced reactive isReady subscription with imperative
     getState() check in initialization effect.

  Additional changes:
  - Add CSP connect-src for Tauri IPC (ipc: http://ipc.localhost)
  - Add ErrorBoundary to main.tsx for visible crash diagnostics
  - Add Vite build config (target, envPrefix) per Tauri 2.0 guide
  - Enable devtools Cargo feature for WebView inspection
  - Open DevTools automatically in debug builds
  - Simplify Windows release: drop x86, produce only NSIS .exe (no .msi)

- Enhance error handling and improve cookie import feedback
- Update release-please branch reference to match actual branch naming
- Restrict default apt sources to amd64 for ARM cross-compilation [skip ci]

Ubuntu 24.04's default sources (security.ubuntu.com, archive.ubuntu.com)
  don't host ARM packages. When dpkg --add-architecture adds arm64/armhf,
  apt-get update tries to fetch ARM indices from these mirrors and gets
  404 errors, causing the build to fail with exit code 100.

  Fix by adding Architectures: amd64 to the default deb822 sources file
  before adding the ARM ports repository. This ensures ARM packages are
  only fetched from ports.ubuntu.com.

- Support manual dispatch in release workflow with tag input [skip ci]

When triggered via workflow_dispatch, github.ref_name resolves to the
  branch name (e.g., "main") instead of a tag. This caused tauri-action
  to try creating a release with tag "main", which failed with
  "Resource not accessible by integration".

  Fix by adding a required 'tag' input for workflow_dispatch and resolving
  the effective tag name in a dedicated step. The checkout also uses the
  tag ref to ensure the correct code version is built.

- Use bash shell for tag resolution step on Windows runners [skip ci]

Windows runners default to PowerShell which can't parse bash syntax
  (if [ -n ... ]). Adding shell: bash ensures the step works on all
  platforms via Git Bash.

- Documentation
- Add release-please version annotations for auto-managed docs [skip ci]

Added x-release-please-version markers to README.md (version badge,
  roadmap heading) and Project_Plan.md (version header). Registered both
  as generic extra-files in release-please-config.json so version numbers
  are updated automatically in Release Please PRs.

- Improve error logging in download_tool_with_fallback function
- **(updater)** Update public key for the updater plugin
- Fixed build generation bugs
- Temo folder bug fix
- Settings interpretation, affecting downloading ability
- **(config_service)** Improve settings loading and sync to GAMDL config.ini
- Add text selection capability to ActivityLog component
- Update default cover format to JPEG to prevent crashes in GAMDL 2.8.4; add file opening functionality in QueueItem component
- **(ToolsTab)** Install only missing required tools and update UI for optional tools
- Sign bundled macOS dependencies with Developer ID for notarization [skip ci]

Apple's notarization service inspects all Mach-O binaries inside the
  .app bundle, including those inside tar.gz archives. Third-party binaries
  (Python, Perl, FFmpeg, MP4Box libs, etc.) from bundled-deps must be
  re-signed with our Developer ID certificate before Tauri packages them.

- Improve asset manifest check in has_platform_assets function
- Add use import to doc test for is_version_at_least

The doc test example needed `use meedyadl::services::gamdl_service::is_version_at_least`
  to resolve the function in cargo test --doc (which runs examples as standalone crates).

- Update remux mode flag and clean up unused options in GamdlOptions
- Add missing used_wrapper field to Rust test initializers

cargo test compiles #[cfg(test)] modules that cargo check skips,
  causing CI to fail with missing field errors in 3 QueueItemStatus
  serde roundtrip tests.

- Update handleViewRelease to use Tauri shell plugin for opening URLs
- Resolve startup crash caused by missing Tokio runtime in setup

The app was crashing on launch with "there is no reactor running, must
  be called from the context of a Tokio 1.x runtime" because the queue
  recovery code assumed a Tokio runtime was active during the setup
  closure. On macOS, this closure runs inside the `did_finish_launching`
  callback where the Tokio runtime isn't registered as "current".

- **(docs)** Add wrapper connectivity troubleshooting guide for remote and Docker setups

Reclassified from docs: to fix: to trigger a patch release. The wrapper
  troubleshooting content (Help > Wrapper, README) addresses user-facing
  issues with diagnosing wrapper connectivity failures on remote devices.

- Improve audio format fallback so downloads try all available formats

When your preferred audio format (like Dolby Atmos) isn't available for
  a track, MeedyaDL now reliably tries the next format in your fallback
  list instead of giving up after the first failure.

- Improve error handling for Python exceptions and traceback frames
- Update documentation and code references to use '4K UHD' terminology
- Correct AcousticID → AcoustID spelling and upgrade vulnerable dependencies

Fix incorrect "AcousticID" spelling to "AcoustID" across 86 instances
  in 15 files (comments, UI text, docs, error messages). Upgrade
  jsonwebtoken from v9 to v10.3.0 (fixes CVE type confusion auth bypass,
  uses aws_lc_rs crypto backend). Update rollup to 4.59.0 (fixes path
  traversal CVE). Dismiss glib 0.18 alert (transitive via Tauri GTK
  stack, not directly used).

- Prevent UI stall on FUSE mounts and fix wrong codec suffix with native priority

Bug 1 — UI stall: The enrichment pipeline (Steps 1-5) called blocking
  mp4ameta Tag I/O directly on tokio async worker threads. On slow FUSE
  mounts (CloudMounter, NFS), this starved the runtime, freezing the UI
  for minutes. Fix: wrap Tag::read/write in spawn_blocking() in 4
  services (metadata, lyrics, AcoustID, ReplayGain). Change enhanced
  lyrics from async fn to fn (had zero .await calls). Add yield_now()
  between all 6 enrichment steps.

  Bug 2 — Wrong codec suffix: apply_codec_suffix() used the REQUESTED
  codec, not the ACTUAL one GAMDL selected via native priority chain.
  Files named [Dolby Atmos] could contain AAC. Fix: skip suffix when
  native priority is active (actual codec unknown until GAMDL finishes).
  Force all companion tiers to use suffixes via new force_all_suffixes
  parameter, preventing filename collisions with the primary's clean
  filenames.

- Use --song-codec-priority instead of removed --song-codec flag

GAMDL 2.9.1 removed the --song-codec flag entirely, causing ALL
  companion tier downloads and fallback retries to fail with:
  "Error: No such option: --song-codec Did you mean --song-codec-priority?"

- Allow companion lyrics formats when Enhanced LRC is enabled

When Enhanced Lyrics (Word-by-Word Sync) was on, the Synced Lyrics
  Formats checkboxes were completely disabled, preventing selection of
  LRC and SRT as companion formats. Now TTML remains locked as the
  primary format (required for word-level timing data) but LRC and SRT
  checkboxes are enabled for companion downloads. The description text
  adapts to explain the behavior.

  Also updates handleFormatToggle() to always keep TTML as primary and
  route other selected formats to companion_lyrics_formats when
  enhanced_lrc is active.

- File picker Browse button now starts at the currently configured path

The native file/directory picker dialog was not setting defaultPath,
  so it opened at the OS-remembered last-used directory (which could be
  wrong after exporting an activity log to a different folder). Now passes
  the current value as defaultPath so Browse always starts at the
  configured path (e.g., the output directory in Settings > General).

- Reorder update check interval options in ascending frequency order

Move "Startup only" from first to last position in the Settings >
  General > Check Interval dropdown. Options now listed from most
  frequent to least frequent: Every hour → Every 6/12/24 hours →
  Startup only.

- Use version tag only as GitHub release title (no app name prefix)

Renamed all 50 existing releases from "MeedyaDL vX.X.X" to just "vX.X.X".
  Updated release.yml releaseName and ARMv7 fallback gh release create
  to use the tag directly. Keeps release page clean and consistent.

- Address GitHub Code Scanning security alerts

Three categories of fixes:

  1. Incomplete URL substring sanitization (Alert #11):
     CookiesTab.tsx used `domain.includes('apple.com')` which could match
     unrelated domains. Now uses exact domain matching:
     `domain === 'apple.com' || domain.endsWith('.apple.com')`

  2. Insecure cookie test fixtures (Alerts #4-10):
     Test cookies that don't specifically verify insecure behavior now set
     `.secure(true)`. The one test that intentionally tests insecure cookies
     (`insecure_cookie_has_false_flag`) is annotated with a comment.

  3. Missing workflow permissions (Alerts #1-2):
     CI workflow now declares `permissions: { contents: read }` following
     the principle of least privilege for the GITHUB_TOKEN.

- Use explicit ARIA string values and update CHANGELOG

- Toggle.tsx: aria-checked now uses "true"/"false" strings instead of
    boolean expression (fixes Edge DevTools axe/aria warning)
  - CookiesTab.tsx: aria-expanded now uses "true"/"false" strings at both
    outer collapsible and per-browser accordion levels
  - CHANGELOG.md: add isBinaural/isDownmix tags and ARIA fix entries

- README badge URLs and header logo

- Fix badge URLs: MeedyaDL/MeedyaDL → MWBMPartners/MeedyaDL (404 fix)
  - Version badge: use dynamic GitHub release API instead of hardcoded
  - CI badge: add ?branch=main for accurate status
  - Replace emoji header with app logo (src-tauri/icons/128x128.png)
  - Add .markdownlint.jsonc: allow inline HTML (standard for GitHub READMEs)

- Use MeedyaDL logo in README header with dark/light theme support

- Replace app icon with proper MeedyaDL logo (assets/logo/meedyadl-logo.svg)
  - Use <picture> element with prefers-color-scheme for dark/light variants
  - Remove h1 heading (logo serves as the header)
  - Expand markdownlint config: disable MD013 (line length), MD041 (first
    line heading), MD060 (compact table style) — all standard for GitHub READMEs

- Resolve markdownlint warnings across all documentation files

- README.md: add blank lines around all 34 headings (MD022), add language
    to 3 fenced code blocks (MD040)
  - DEV_NOTES.md: add blank lines around headings and after list blocks
  - help/cookie-management.md: add blank line before heading
  - .markdownlint.jsonc: only suppress truly unfixable rules (MD033 inline
    HTML for logos/badges, MD041 first-line heading, MD013 line length for
    URLs) — removed MD060 suppression since tables are now clean

- Detect actual codec via ffprobe for correct metadata tags with native priority

When using GAMDL >= 2.9.1's --song-codec-priority, codec_used was set to
  the requested codec at enqueue time, not the actual codec GAMDL selected.
  This caused enrichment to write incorrect tags (SpatialType, isBinaural,
  isDownmix) on ALL files regardless of their actual codec.

- Warn in activity log when ffprobe unavailable with native priority

Non-verbose activity log now alerts users when ffprobe is unavailable
  or fails for a file while native priority is active, since codec tags
  may be inaccurate without it. Previously this was only logged at debug
  level via RUST_LOG=debug.

- Gap-fill retry for partial downloads with native priority

When GAMDL's --song-codec-priority skips tracks because experimental
  codecs (Atmos, AC3) are unavailable without wrapper auth, MeedyaDL now
  automatically re-runs GAMDL with non-experimental codecs and
  overwrite=false to fill the gaps. This recovers skipped tracks in
  lossless/lossy formats without overwriting successful Atmos/AC3 files.

  Added SongCodec::from_cli_string() and is_wrapper_dependent() methods.
  Helpers: count_codec_skip_warnings, build_gapfill_priority_chain,
  count_audio_files_in_directory. 11 new unit tests.

- Companion downloads never apply filename suffixes

apply_codec_suffix() only checked options.song_codec, but companion
  downloads set song_codec=None and use song_codec_priority instead
  (for GAMDL >= 2.9.1). This meant no companion ever got a suffix like
  [Lossless] or [Dolby Atmos], causing each companion tier to overwrite
  the previous tier's files (identical filenames).

  Fixed by falling back to parsing song_codec_priority via
  SongCodec::from_cli_string() when song_codec is None.

- Error report deletion now persists across app restarts

delete_crash_report() returned Ok(()) even when the report wasn't found
  during directory scan, so the frontend optimistically removed it from
  state while the file stayed on disk. On restart, reports reappeared.

  Now returns Err when not found, added debug logging at each scan step.

- Add missing permissions for issue closure and project item addition
- Add read permissions for project directory in settings.json
- WebKitGTK rendering corruption on Raspberry Pi and tray deprecation warning

- Add setup_linux_rendering_env() that detects Raspberry Pi via
    /proc/device-tree/model and sets WEBKIT_DISABLE_DMABUF_RENDERER=1 and
    WEBKIT_DISABLE_COMPOSITING_MODE=1 before the WebView is created, forcing
    software rendering to fix garbled UI over remote desktop (RPi Connect)
  - Only applies on Raspberry Pi — desktop Linux retains GPU acceleration
  - Respects user-set env vars (won't override if already defined)
  - Update .deb dependency to accept libayatana-appindicator3-1 as
    alternative to deprecated libappindicator3-1

- Update test_is_gamdl_compatible for new minimum version 2.9.2

The test was asserting 2.8.4 and 2.0.0 as compatible, which no longer
  holds after bumping MIN_COMPATIBLE_GAMDL to 2.9.2.

- Set MIN_COMPATIBLE_GAMDL back to 2.9.1
- Resolve clippy doc comment lints on Ubuntu CI

Move run() doc comment directly above pub fn run() to fix
  empty_line_after_doc_comments, and rewrap setup_linux_rendering_env
  doc comment to fix doc_lazy_continuation.

- Improve line wrapping on MusicKit credential validation result text

The validation message next to the Test Credentials button was being
  squeezed onto one line. Use items-start alignment, shrink-0 on the
  button, and leading-relaxed on the result text for cleaner wrapping.

- Resolve MusicKit 401 validation flow and add embedded token fallback
- Security hardening, dependency updates, and stability improvements

Security fixes (closes #175, #176, #177):
  - Fix TAR extraction path traversal vulnerability — iterate entries
    individually and reject paths with `..` components or absolute paths
  - Add explicit timeouts to all reqwest HTTP clients (Apple Music API,
    AcoustID, GitHub API, update checker) preventing indefinite blocking
  - Redact wrapper account URL from GAMDL CLI args log line to prevent
    credential tokens from persisting in plaintext log files

  Dependency updates:
  - Fix npm audit vulnerabilities (flatted < 3.4.0, undici 7.0.0-7.23.0)
  - Update lz4_flex 0.11.5 → 0.11.6 (memory leak fix, closes Dependabot
    security alert #17)

  CI/DX improvements:
  - Add monthly Dependency Report workflow for major version visibility
  - Fix Dependabot config to actually ignore major version bumps (the
    comment said it did but the ignore rule was missing)
  - Fix ESLint errors in Node.js build scripts (add globals for console,
    process, Buffer; remove unused deflateSync import)
  - Fix flaky Windows CI test (probe_nonexistent_directory_with_valid_parent)

  Crash report improvements:
  - Add delete_all_crash_reports command + "Clear All" UI button
  - Promote delete logging from debug to info for production visibility
  - Show actual error messages in frontend delete failure toasts

  Stability improvements:
  - Fix Tooltip setTimeout cleanup on unmount (useRef + useEffect)
  - Fix CookiesTab copy-success timeout cleanup on unmount

- Move codec filename suffixes to codecs.toml registry (closes #118)

Move hardcoded codec suffix strings from download_queue.rs to the
  codecs.toml registry, preventing filename collisions when users select
  multiple lossy codecs in Custom companion mode.

  New suffixes: AAC Binaural → [Binaural], AAC Downmix → [Downmix],
  AAC Legacy → [AAC Legacy], HE-AAC → [HE-AAC], and variants.
  Standard AAC 256 keeps clean filenames (empty suffix).

  Existing suffixes preserved: ALAC=[Lossless], Atmos=[Dolby Atmos],
  AC3=[Dolby Digital].

- Enable minor version bumps for feat: commits pre-1.0

Change bump-patch-for-minor-pre-major from true to false so that
  feat: commits correctly bump the minor version (0.6.x → 0.7.0)
  instead of only the patch version (0.6.x → 0.6.y).

  The previous setting was treating all feat: commits as patch bumps
  while the project is pre-1.0, which didn't reflect the significance
  of changes like Tailwind v4 migration, accessibility themes, etc.

- Resolve VS Code Problems — CSS prefix order and inline style

Fix 5 linter warnings:
  - globals.css: reorder user-select after -webkit-user-select (2 instances)
  - macos.css: reorder backdrop-filter after -webkit-backdrop-filter (2 instances)
  - SettingsSection.tsx: replace inline transform style with Tailwind rotate class

  Remaining 24 Problems are documented unfixable:
  - ARIA attribute values: Edge DevTools false positives on JSX expressions
  - Inline styles: dynamic runtime values (progress bar widths)
  - main.tsx: intentional ErrorBoundary styles (must work without CSS)

- **(ci)** Restrict cargo-deny to Linux runners only

cargo-deny-action is a Docker container action which is only supported
  on Linux runners. It was failing on macOS and Windows with:
  "Container action is only supported on Linux"

  Add `if: runner.os == 'Linux'` condition since licence/advisory checks
  are platform-independent — running once on Linux is sufficient.

- Parse GAMDL 2.9.x track format for progress bar display

GAMDL 2.9.x changed its output from "Getting track N of M: Title"
  to "[Track N/M] Downloading \"Title\"". Add TRACK_INFO_V2_REGEX to
  parse the new format and extract track number/total for progress
  calculation.

  - Add track_number/track_total optional fields to TrackInfo event
  - Compute approximate progress from track counts (N-1/M percentage)
  - Update TypeScript types and download store handler
  - Progress bars now show fill and track names during Apple Music downloads

- Update ISRC reconciliation logic for Vendor tag extraction

Update extract_isrc_from_vendor() with 3-case logic:
  1. ISRC blank → copy from Apple Vendor tag (Label:isrc:CODE)
  2. ISRC set + Vendor differs → store both (API / Vendor format)
  3. ISRC set + identical or no Vendor → no-op

- Add storefront to GAMDL config, codec suffix rename, ISRC logic
- Ensure all v0.x releases are marked as pre-release

Add "prerelease": true to release-please-config.json so release-please
  creates GitHub releases with the pre-release flag for all v0.x versions.

  Fixed v0.10.0 and v0.8.0 which were incorrectly marked as full releases.

  Also created GitHub issues for upcoming tasks:
  - #208: auto-delete crash reports after submission
  - #209: verbose logging persistence for pre-release versions
  - #210: component library versions on About screen + startup log
  - #211: restore platform emojis + download links in release tables

- Redesign logotype SVG as text-only wordmark for MeedyaSuite

- Remove icon from logotype (text-only as requested)
  - Switch from Inter to Poppins Black (900) for more character
  - Design as MeedyaSuite brand template:
    - "Meedya" prefix is the brand constant (id="brand-prefix")
    - Product suffix is swappable (id="product-suffix")
    - Works for MeedyaDL, MeedyaManager, MeedyaDB
  - Animated gradient shimmer on brand text
  - Decorative dot separator between brand and suffix
  - CSS custom properties for theming
  - prefers-reduced-motion support
  - Google Fonts @import for Poppins with fallback stack

- Switch logotype to Orbitron + Rajdhani for techy/futuristic feel

Replace Poppins (generic geometric sans) with:
  - Orbitron Black (900) for brand prefix — sharp geometric display
    face with clipped corners, sci-fi/tech aesthetic
  - Rajdhani SemiBold (600) for product suffix — angular condensed
    sans with digital readout quality

  Add decorative tech elements:
  - Square bracket frames flanking the wordmark
  - Vertical circuit-dot separator (3-dot data bus motif)
  - Horizontal scan line animation (HUD/terminal sweep)
  - Dashed accent underline (circuit trace)
  - Neon glow filter on brand text
  - All uppercase for sharper tech feel

- Remove double-hyphens from XML comment in logotype.svg

XML forbids '--' inside comments. The CSS custom property names
  listed in the header comment contained '--' prefixes which caused
  a parse error in browsers (Edge, Chrome, Firefox). Removed the
  '--' prefixes from the comment text — the actual CSS properties
  in the <style> block are unaffected.

- Embed Orbitron + Rajdhani fonts as base64 WOFF2 in logotype SVG

Replace the external Google Fonts @import with four self-contained
  @font-face declarations using base64-encoded WOFF2 data:
    - Orbitron 700 (brand prefix, bold)
    - Orbitron 900 (brand prefix, black)
    - Rajdhani 600 (product suffix, semibold)
    - Rajdhani 700 (product suffix, bold)

  The SVG is now fully self-contained (68 KB) and renders correctly
  without any network requests. Fonts can be edited/changed by
  replacing the base64 data or converting text to outlines.

- Tighten logotype spacing and reduce canvas width

Move circuit dots, product suffix, and right bracket ~60px left to
  eliminate the excess gap between "MEEDYA" and the separator dots.
  Reduce viewBox from 720x130 to 600x130 to match the tighter layout.

- Dynamic canvas width, mixed-case brand, respect suffix casing

- Change "MEEDYA" to "Meedya" for brand prefix
  - Remove text-transform: uppercase from both text styles so casing
    respects the actual text content per product:
      MeedyaDL, MeedyaDB, MeedyaManager
  - Add embedded <script> that dynamically measures text widths and
    repositions circuit dots, suffix, bracket, underline, and resizes
    the viewBox on load — canvas auto-fits any suffix length
  - Remove hardcoded width attribute; viewBox drives sizing
  - Uses document.fonts.ready API for accurate post-font measurement

- Tighten dot separator to colon-like spacing (Meedya:DL)

Reduce GAP and DOT_GAP from 16px/12px to 3px/3px so the circuit
  dots sit tight against the brand prefix and suffix, reading as
  "Meedya:DL" rather than "Meedya  :  DL".

- Heavier suffix weight, drop shadows, dark/colour-blind palettes

Suffix text ("DL"):
  - Switch from Rajdhani 600 to Orbitron 900 (matches prefix weight)
  - Add 1.5px stroke for extra visual heft
  - Now reads as one cohesive word with the prefix

  Drop shadows:
  - New dual-layer text-shadow filter on both prefix and suffix
  - Layer 1: dark directional shadow for legibility on any background
  - Layer 2: coloured neon glow for brand feel
  - Shadow colour/opacity driven by CSS custom properties

  Colour adaptation:
  - Automatic dark mode via @media(prefers-color-scheme: dark)
  - Manual .dark class override for app-controlled themes
  - Colour-blind palettes: .cb-deutan, .cb-protan, .cb-tritan
  - All colours overridable via CSS custom properties or JS

- Embed full font character sets, match dot height to cap height
- Switch to slate/steel palette, add ?mode= URL parameter

Colour palette:
  - Replace blue/purple/cyan AI-vibe colours with slate/steel gradient
    (dark slate #475569 -> steel #64748B -> silver #94A3B8)
  - Dark mode: light silver/near-white for visibility on dark backgrounds
  - Colour-blind palettes updated with dark variants for all 3 types

  URL parameter mode switching:
  - ?mode=light (default slate/steel)
  - ?mode=dark (silver/white for dark backgrounds)
  - ?mode=cb-deutan, ?mode=cb-protan, ?mode=cb-tritan (light bg)
  - ?mode=cb-deutan-dark, ?mode=cb-protan-dark, ?mode=cb-tritan-dark
  - Script reads window.location.search and applies CSS classes on load
  - Also still supports CSS class application and direct JS property override

- Extend animated gradient to suffix and dots

- Suffix ("DL") now uses its own animated gradient
    (logotype-grad-suffix-anim) with offset timing from the prefix,
    so the shimmer flows across the entire wordmark
  - Circuit dots use a separate animated gradient
    (logotype-grad-dots-anim) with a faster independent rhythm
    (0.6x the base animation speed) via dot-shimmer keyframes
  - All three animations are coordinated but distinct:
    prefix shimmer, suffix shimmer (offset), dots shimmer (faster)
  - Reduced motion media query updated to disable all three

- Variable fonts, thicker brackets with flash control, Dev_Notes docs
- Re-embed full character set fonts (207 + 465 glyphs)

Replace Latin-subset fonts with full character sets downloaded from
  the canonical Google Fonts repository:
  - Orbitron variable (400-900): 207 glyphs, 15 KB (full Latin Extended)
  - Rajdhani Bold: 465 glyphs, 102 KB (full Latin Extended + Devanagari)

  SVG size: 179 KB (was 49 KB subset / 308 KB with 4 static files)

- Redesign logo — simplified, distinct layers, slate/steel palette

Simplified from 6 overlapping same-colour elements to 3 clearly
  separated layers with distinct colours:
    1. Vinyl disc (dark slate) — background, with subtle grooves
    2. Base tray (accent steel gradient) — anchors the composition
    3. Download arrow (light steel/silver gradient) — foreground, high contrast

- Full-size projector with realistic detail, download arrow as watermark

Projector redesign (fills same space as vinyl disc):
  - Dual full-size reels (r=72) with 6 spokes each, spinning opposite
  - Film gate between reels with frame aperture detail
  - Film threading path connecting reels through gate
  - Multi-ring lens assembly (barrel, mount, glass, highlight)
  - Light beam cone from lens with flickering animation
  - Soft beam glow ellipse with pulsing animation
  - Ventilation slots and feet for realism
  - Body, detail lines, proportions all match vinyl disc scale

  Download arrow:
  - Moved to background as a subtle watermark (8% opacity)
  - Includes arrow shaft, chevron head, and small base tray
  - Visible but doesn't compete with media symbols
  - Removed the foreground arrow and separate base tray

- Lighter disc/reel colours, match reel speed to vinyl, dynamic sizing
- Remove dashed accent underline from logotype

Remove the dotted/dashed bottom line (accent-underline element) and
  all script references to it. The line was a decorative tech element
  but appeared as a stray visual artefact.

- Trim APNG to actual content bounds, remove excess whitespace

The svg-to-apng script now reads the SVG's viewBox after the dynamic
  layout script has run, then resizes the viewport to match. This trims
  the logotype APNG from 600px to ~487px wide (matching the actual text
  width after font measurement).

  - Replaced ffmpeg cropdetect with puppeteer viewBox measurement
  - Logotype trimmed: 600x130 -> 487x130 (no more side padding)
  - Logo unchanged: 512x512 (viewBox doesn't resize)
  - Added 1.5s wait before measurement for font loading

- Content-aware trim for both APNGs, removes whitespace on all sides

Replace viewBox-only measurement with union bounding box of all
  rendered SVG elements (circle, path, line, rect, text, etc.).
  This correctly trims both:
  - logo.apng: 512x512 -> 472x447 (disc/chevrons only, no empty edges)
  - logotype.apng: 600x130 -> 485x99 (text only, no side/top padding)

  Both retain full alpha transparency via puppeteer omitBackground.

- Add drop shadow to bracket decorations in logotype

New bracket-shadow filter applied to both [ ] polylines for
  visibility on any background. Uses --logotype-shadow colour variable.

- Update brandkit with icon variants, clean up old icon assets

- brandkit.html: added dark mode and colour-blind icon variant
    sections with preview cards and download links
  - Cleaned up assets/icons/variants/ (old concept SVGs removed)
  - Updated assets/icons/app-icon.svg and public/app-icon.svg
    with the new split disc/reel icon
  - SVG license headers updated to proprietary format with full
    copyright year

- Implement atomic file writes for settings and queue (closes #230)

Replace std::fs::write with write-to-temp-then-rename pattern for both
  settings.json and queue.json. This prevents file corruption if the
  process crashes or loses power during a write operation.

  The rename() syscall is atomic on all major filesystems (APFS, ext4,
  NTFS), so the file is either fully written or unchanged — never
  partially written/corrupt.

  Files affected:
  - config_service.rs: settings.json -> settings.json.tmp -> rename
  - download_queue.rs: queue.json -> queue.json.tmp -> rename

- Debounce queue saves, create SECURITY.md, CSP-safe SVG embeds

#233 (closes): Debounce queue persistence to max once per 500ms.
  Uses AtomicU64 timestamp to skip rapid sequential saves, with a
  delayed follow-up save to ensure final state is always persisted.

  #234 (closes): Create SECURITY.md with vulnerability reporting
  instructions, supported versions, and security measures list.

  #221 (closes): Switch sidebar SVG embeds from <object> to <img> tags.
  <img> blocks SVG script execution (CSP defence-in-depth) while still
  rendering CSS animations. Added onError fallback for logotype.

- Resolve clippy warnings from CI (collapsible_str_replace, needless_borrow)

Fix 3 clippy warnings that failed CI on Windows (Rust 1.94.0):
  - config_service.rs: .replace('\n', "").replace('\r', "") -> .replace(['\n', '\r'], "")
  - settings.rs: same collapsible_str_replace fix
  - dependency_manager.rs: remove needless & on read_dir(temp_dir)

- Add focus trap, ARIA dialog role, and focus management to Modal (closes #218, #182 partial)

Modal accessibility improvements:
  - Added role="dialog" and aria-modal="true" to the panel element
  - Added aria-labelledby linking to the modal title
  - Focus trap: Tab/Shift+Tab cycle within focusable elements inside
    the modal, preventing focus from escaping to background content
  - Auto-focus: moves focus to the first focusable element on open
  - Focus restore: returns focus to the triggering element on close
  - Panel has tabIndex={-1} so it can receive programmatic focus

  These are the critical accessibility fixes from the audit (#218).
  Manual QA testing (#182) still needed for VoiceOver/NVDA/Orca.

- Re-sync config.ini before each GAMDL invocation to prevent stale config

GAMDL 2.9.3 overwrites config.ini with its own defaults when run,
  causing our storefront and other settings to be lost. The storefront
  being None causes: AttributeError: 'NoneType' has no attribute 'upper'

- Correct logotype static fallback positions for <img> rendering

When loaded as <img> (in the app sidebar), JavaScript doesn't execute,
  so the dynamic layout script can't reposition elements. The hardcoded
  fallback positions were set for the old uppercase "MEEDYA" layout,
  leaving a 76px gap with the current mixed-case "Meedya".

  Updated static positions to match the script's calculated values:
  - Dots: cx 418 -> 342
  - Suffix: x 434 -> 345
  - Bracket: x 524 -> 473
  - ViewBox: 600 -> 487

  The dynamic script still runs in browser contexts and will override
  these for other product names (Manager, DB). But for "DL", the static
  positions now render correctly without JavaScript.

- Revert master logotype.svg, keep tight positions only in public/

Master (assets/brand/logotype.svg) restored to original wide fallback
  positions (viewBox 600, dots cx=418, suffix x=434). The dynamic JS
  script handles positioning at runtime in browser contexts.

  Only public/logotype.svg retains the tight static positions (viewBox
  487, dots cx=342, suffix x=345) for the app sidebar where JS doesn't
  execute inside <img> tags.

- Resolve doc_lazy_continuation clippy warning in sync_gamdl_config

Add blank /// line between the parameter list and the function
  description paragraph. Clippy 1.94 treats the continuation as a
  malformed doc list item without the separator.

- Resolve false 'no output files' failure on GAMDL 2.9.x album downloads (#242)

GAMDL 2.9.x with native --song-codec-priority does not emit "Saved to:"
  lines for album downloads. The success path only set output_path via that
  event, and the disk-scan fallback (find_album_directory) only ran inside
  codec/IO error branches — the clean-exit path was unhandled.

  Added a general disk-scan fallback before the terminal failure check that
  runs for ALL cases where output_path is None after GAMDL exits 0. This
  prevents the cascading bug where the false failure triggered auto-retry
  without wrapper, which overwrote successful Atmos files with ALAC.

- Export activity log as .log instead of .txt

Changes the native save dialog filter and default filename from
  meedyadl-activity-log.txt to meedyadl-activity-log.log.

- Include all user-selected codecs in Custom companion tiers

plan_companions() previously filtered out codecs matching the primary
  setting. With native priority the actual codec GAMDL picks may differ,
  so the user's explicit Custom selections are now always respected.

  Also adds a visual separator (═══) in the activity log when each new
  queue item starts processing, making it easy to distinguish boundaries.

- Apply codec suffix based on ffprobe-detected codec, not requested

enrich_single_file() now returns the effective SongCodec detected via
  ffprobe. The post-enrichment suffix rename uses this per-file detected
  codec instead of the requested codec from settings.

  Previously, requesting Atmos with native priority could apply a
  [Dolby Atmos] suffix to files that actually contained ALAC (when
  GAMDL silently fell back). Now the suffix accurately reflects the
  file's actual content.

- Manifest tweaks — download start time, null codec, vendor MIME (#245)

- downloaded_at now captures when the download starts processing
    (not when enrichment finishes or the manifest is written)
  - codec fields default to null at both source and track level —
    the manifest is a metafile for re-downloading, not a quality spec
  - MIME type changed to vendor convention per RFC 6838 §3.2:
    application/vnd.mwbmpartners.meedyadl.download+json

- **(deps)** Resolve picomatch ReDoS vulnerability (GHSA-c2c7-rcm5-vvqj)

npm audit fix: picomatch 4.0.3 → 4.0.4. Fixes high-severity ReDoS
  via extglob quantifiers and method injection in POSIX character classes.

- Resolve ESLint react-hooks/exhaustive-deps warning in App.tsx

Read showSetupWizard imperatively via getState() inside the async
  initialize() function instead of using the reactive selector. The
  value is only needed once after all awaits complete.

- Add loading state during preflight checks to prevent duplicate submissions (#249)

Wraps handleSubmit with isChecking state that disables the "Add to
  Queue" button and shows a spinner while preflight checks (internet,
  output path, cookies) are running. Prevents users from clicking
  multiple times on slow networks.

- Add debounced save to prevent concurrent settings write race (#250)

Added debouncedSave() to settingsStore — batches rapid save calls
  within 300ms into a single disk write. Auto-save callers (toggle
  switches) should use this instead of saveSettings() directly.
  Manual "Save" button still uses saveSettings() for instant feedback.

- Add aria-labels to context menu and queue items (#254)

- ContextMenu: aria-label="Actions menu" on the role="menu" container
  - QueueItem: role="listitem" + aria-label with download URL on each item

  WCAG 2.1 compliance — screen readers can now identify context menus
  and queue items.

- Remove placeholder Sentry DSN, use env var for real DSN (#231)

Both JS (VITE_SENTRY_DSN) and Rust (SENTRY_DSN) now read the DSN
  from environment variables at build time. Without a configured DSN,
  Sentry is a no-op with a debug log message. Removes the placeholder
  examplePublicKey@o0.ingest.sentry.io/0 that was sending data nowhere.

- Add role=list to queue items container for screen reader navigation (#125)

QueueItem children already have role="listitem". Parent container now
  has role="list" + aria-label="Download queue items" so screen readers
  can identify the list structure.

- Use URL hostname parsing instead of substring matching in detectPlatform [skip ci]

Resolves CodeQL alerts #13, #14, #15 (Incomplete URL substring sanitization).
  `url.includes('music.apple.com')` could match crafted hostnames like
  `evil-music.apple.com.attacker.com`. Now uses `new URL().hostname` for
  exact hostname comparison.

- Bundle English translations inline to prevent raw i18n keys on first render [skip ci]

The sidebar was briefly showing raw keys like "sidebar.ready" and
  "sidebar.checkForUpdates" because i18n resources were loaded via async
  fetch() inside a useEffect, which completes after the first render.

- Detect actual codec before planning companion downloads with native priority [skip ci]

When native priority is used (--song-codec-priority atmos,alac,aac,...),
  GAMDL may silently fall back to ALAC when Atmos is unavailable. Previously,
  companion downloads were planned against the REQUESTED codec ("atmos"),
  causing a redundant ALAC companion download when primary was already ALAC.

- Clear inherited binaural/downmix tags on non-binaural codecs, add activity log for codec detection fallback

Two fixes:

  1. isBinaural/isDownmix tags (MeedyaDL-specific MeedyaMeta namespace)
  were persisting on AAC Legacy and other non-binaural/downmix files.
  When effective codec is not binaural/downmix, enrichment now explicitly
  removes these tags via clear_binaural_downmix_tags(). Prevents stale
  tags from prior enrichment passes or overwrite scenarios.

  2. Codec detection fallback chain (MediaInfo -> ffprobe -> requested)
  now emits activity log entries (not just verbose/debug logs) so users
  see when detection falls back. Passes dl_id for per-download logging.

- Run lyrics conversion (TTML → LRC/SRT/VTT/ASS) on companion downloads

Companion downloads inherited TTML as the lyrics format (forced by
  Enhanced LRC), but the enrichment pipeline only ran for the primary
  download. This left TTML sidecars unconverted for companion tiers.

  Adds run_companion_lyrics_conversion() which runs after each successful
  companion tier: Enhanced LRC, Rich SRT, WebVTT, and ASS generation —
  matching the same conversion steps as the primary enrichment pipeline.

- Activity log file logging, binaural/downmix companion tags, progress bar UX

Four fixes:

  1. Activity log entries (emit_download_log, emit_app_log) now also write
     to the tracing file log via log::info!. Previously they only emitted
     Tauri events to the frontend, making enrichment progress invisible in
     the on-disk log file when the UI was unresponsive.

  2. Companion downloads (apply_codec_metadata_tags) now clear inherited
     isBinaural/isDownmix tags for all codecs that aren't binaural/downmix.
     GAMDL's --fetch-extra-tags writes these from Apple Music API audioTraits
     regardless of the actual downloaded codec. Previously only the primary
     enrichment pipeline cleared them; companion files retained stale tags.

  3. Queue-level progress bar now includes error and cancelled items in
     both the total and completed counts, preventing it from appearing
     stuck at 0% for single-item queues.

  4. Progress bar text increased from 10px to 12px and bar height from
     4px to 6px for better readability.

- MediaInfo install via MeedyaDL-Tools mirror instead of upstream DMG

The upstream macOS MediaInfo download is a .dmg containing a .pkg
  installer, which our archive module cannot extract. Changed the
  primary URL resolver to always fall through to the MeedyaDL-Tools
  mirror, which hosts repackaged CLI binaries as tar.gz/zip.

  Mirror assets uploaded:
  - mediainfo-macos-aarch64.tar.gz (universal binary, arm64+x86_64)
  - mediainfo-macos-x86_64.tar.gz (same universal binary)
  - mediainfo-windows-x86_64.zip (MediaInfo.exe + LIBCURL.DLL)
  - mediainfo-windows-aarch64.zip (MediaInfo.exe + LIBCURL.DLL)
  - mediainfo-linux-x86_64.tar.gz (static CLI binary)

- Remove hardcoded mp4decrypt version URL, use MeedyaDL-Tools mirror

mp4decrypt (Bento4) was pinned to version 1.6.0-641 via a hardcoded
  bok.net URL. Bento4 has no GitHub Releases API or "latest" tag, so
  the URL would go stale on future updates.

  Changed to mirror-only distribution (same approach as MediaInfo and
  MP4Box). The MeedyaDL-Tools mirror already has mp4decrypt assets for
  all 3 platforms (macos-aarch64, linux-x86_64, windows-x86_64).

- Mark MediaInfo as required tool for automatic installation

MediaInfo was marked as optional (required: false) so it was skipped
  during setup wizard and "Check All". Since MeedyaDL actively uses
  MediaInfo for codec detection in the enrichment pipeline, it should
  be auto-installed alongside FFmpeg, mp4decrypt, N_m3u8DL-RE, and
  MP4Box. Now 5 required tools instead of 4.

- Use /releases/tags/ for deterministic mirror asset resolution

GitHub's /releases/latest endpoint returns the "most recently created"
  release, which differs from the release explicitly tagged "latest" when
  a repo has multiple releases. This caused MediaInfo macOS assets to not
  be found — they existed on the 'latest' tagged release but the API was
  returning a different release (2026-03-27) that lacked macOS assets.

- Move CORE_COMPONENTS to module scope to resolve ESLint exhaustive-deps warnings

Also adds BBC Sounds as a separate platform in engines.toml with its
  own icon path, sharing the same engine priority as BBC iPlayer
  (get_iplayer primary, yt-dlp fallback).

- Update BBC Sounds icon to match official logo shape

Three vertical rectangles (small left, medium centre, large right)
  matching the BBC Sounds speaker/equaliser branding. Black background
  areas are now transparent; fill uses currentColor so it adapts to
  light, dark, and colour-blind themes via fill-opacity layering.

- Make platform icons dynamically expand to fill container

Removed hardcoded width="16" height="16" from all 7 platform SVGs —
  they now have only viewBox, so the parent container controls size.
  PlatformIcon wrapper uses [&>svg]:w-full [&>svg]:h-full to make the
  inline SVG expand to fill the 16x16 container. Bumped container
  from 14px to 16px for better visibility.

- Update BBC iPlayer icon to match official logo shape

Three angular segments: left vertical bar, top-right angled bar,
  bottom-right angled bar — forming the stylised play/forward symbol
  from the BBC iPlayer branding. Black background areas are transparent;
  fill uses currentColor with varying opacity for depth and theme
  adaptability across light, dark, and colour-blind modes.

- Update OF-Scraper icon to match official logo shape

Circular open 'O' base with a swept wing/feather extending right,
  matching the official branding. White areas are transparent; fill
  uses currentColor with varying opacity (0.25-0.85) for the multi-
  tone depth effect, adapting to light, dark, and colour-blind themes.

- Rename ofscraper icon to onlyfans, redesign to match official logo

Renamed ofscraper.svg → onlyfans.svg and updated engines.toml
  platform ID to match. Completely redesigned the SVG to match the
  official OnlyFans logo: open circular arc (thick C-shape), centre
  dot, and two layered wing/feather curves sweeping from the opening.
  White areas are transparent; currentColor with varying opacity for
  theme adaptability.

- Clean OnlyFans SVG — currentColor for themes, no fixed dimensions

Stripped XML declaration, Illustrator metadata, and hardcoded hex
  colours (#03A9F4, #0288D1). Replaced with currentColor + fill-opacity
  (0.7 for the circular O, 0.9 for the wing sweep). viewBox preserved
  from original (0 0 48 48), no fixed width/height so SVG dynamically
  expands to fill the parent container.

- Clean Apple Music SVG — currentColor for themes, no fixed dimensions

Stripped XML declaration, Illustrator metadata, and hardcoded colours
  (#F50057 red, #FFFFFF white). Rounded square background uses
  currentColor at fill-opacity 0.25; music note uses currentColor at
  fill-opacity 0.9. viewBox preserved (0 0 48 48), no fixed width/height.

- Check common install locations beyond PATH for system tools

find_system_tool() and get_mediainfo_path() now check platform-specific
  locations when a tool isn't on the shell PATH:

- Add custom MediaInfo path setting with priority resolution

Adds mediainfo_path to AppSettings, matching the existing pattern for
  ffmpeg_path, mp4decrypt_path, etc. Resolution priority in
  get_mediainfo_path():

  1. User-configured custom path (Settings > Tools)
  2. Managed tools directory (auto-installed)
  3. System PATH (via which/where)
  4. Common platform locations (/opt/homebrew/bin, /usr/local/bin)

  If the custom path is set but doesn't exist, logs a warning and
  falls through to the next source.

- Add MediaInfo custom path input to Settings > Tools

Added MediaInfo to TOOL_PATH_KEYS and TOOL_PATH_DESCRIPTIONS in
  ToolsTab.tsx. The expand/chevron pattern and FilePickerButton
  automatically render for MediaInfo — same UX as FFmpeg, mp4decrypt,
  N_m3u8DL-RE, and MP4Box.

  Also created #278 for setup wizard custom path fallback when tool
  install fails (future enhancement).

- Setup wizard installs all bundled pip engines (GAMDL + votify)

The GAMDL setup step now calls installBundledEngines() which installs
  GAMDL first, then votify (and any future bundled+enabled pip engines).
  Previously only GAMDL was installed during setup; votify was left for
  the user to install manually or via Settings > Tools.

  GamdlStep UI text updated to "Download Engines" to reflect it handles
  multiple engines. votify install failure is non-fatal (logged but
  doesn't block the wizard).

  Binary engines (get_iplayer, MediaInfo) are handled by the existing
  Tools step which downloads from the MeedyaDL-Tools mirror.

- Allow multiple adjacent separators in template builder

The template parser now splits literal text by known separator tokens
  (from COMMON_LITERALS: " - ", "/", " ", "-", "_") so each renders as
  its own chip in the TemplateBuilder UI. Previously, the round-trip
  parse→serialize→re-parse collapsed adjacent literals into one segment,
  making it impossible to use multi-character separators like " - ".

  Known tokens are matched longest-first to avoid partial matches
  (e.g., " - " matches before " " or "-"). Unknown text between tokens
  is preserved as-is.

  Added 2 new tests: dash separator splitting, multiple adjacent separators.

- ReplayGain album gain, configurable reference level, clipping prevention (#282)

Three enhancements to ReplayGain analysis:

  1. Album gain: After analysing all tracks individually, computes
     album-level integrated loudness (average in linear power domain)
     and highest true peak. Writes 4 tags per file: replaygain_track_gain,
     replaygain_track_peak, replaygain_album_gain, replaygain_album_peak.

  2. Configurable reference level: New setting replaygain_reference_level
     (default -18.0 LUFS / EBU R128). Dropdown options: -18 LUFS (music),
     -14 LUFS (Spotify), -23 LUFS (broadcast), -16 LUFS (Apple Music).

  3. Clipping prevention: New setting replaygain_prevent_clipping (default
     true). Limits gain so peak × gain never exceeds 0 dBFS, preventing
     digital distortion on loudly mastered tracks.

  Settings UI: reference level dropdown and clipping toggle appear
  conditionally when ReplayGain is enabled.

- Graceful fallback when settings.json parsing fails (#283)

When serde_json fails to parse settings.json (e.g., a field type changed
  between app versions), the app now falls back to defaults instead of
  returning an error to the frontend. Previously, a parse error left the
  frontend store with its initialisation defaults, making it appear as if
  all settings were reset.

  The settings file is preserved on disk — only the in-memory state uses
  defaults for the incompatible fields. The error is logged at ERROR level
  with the specific parse failure and file path for debugging.

- Replace Storefront freetext with country dropdown (#285)

Replaced the freetext input with a <select> dropdown listing ~45
  Apple Music storefronts by country name. "Auto-detect" is the default
  (derives from metadata language). The disabled separator line prevents
  accidental empty selection. Backend stores the 2-letter ISO code.

  Prevents invalid storefront codes from being entered manually.

- Update engine requirements and enable status for votify and get_iplayer
- Clarify ReplayGain settings with album gain explanation and improved descriptions

The ReplayGain settings section now includes an info box explaining
  what's written to each file (track gain for shuffle, album gain for
  album listening, peak values for clipping prevention). Reference level
  and clipping prevention descriptions are also improved with practical
  context (Spotify comparison, modern pop/EDM note).

- Replace source file references with proper ACKNOWLEDGEMENTS.md

The Help > About > Open Source Acknowledgements section was directing
  users to "see the project's Cargo.toml and package.json" — source
  files that users should never need to access.

  Created ACKNOWLEDGEMENTS.md with a comprehensive categorised list of
  all dependencies (engines, framework, tools, Rust crates, frontend
  packages, Tauri plugins) with their licence types.

  Updated HelpViewer to show a complete inline list and reference the
  ACKNOWLEDGEMENTS file instead of source code files.

- ACKNOWLEDGEMENTS.md lists only enabled/shipping dependencies

Removed votify, yt-dlp, get_iplayer from both ACKNOWLEDGEMENTS.md and
  HelpViewer — they are defined in engines.toml but not yet enabled.
  Only GAMDL ships as the active download engine.

  Added maintenance comment at top of ACKNOWLEDGEMENTS.md: file must be
  reviewed when engines are enabled/disabled in engines.toml or deps
  change in Cargo.toml/package.json.

- Link each licence individually in dual-licence entries

licenceLink() now splits "MIT / Apache-2.0" into two separate
  hyperlinks: [MIT](url) / [Apache-2.0](url). Previously both pointed
  to the MIT licence URL.

- Inject width/height/display into SVG for platform icon rendering

The inline SVG loaded via fetch + dangerouslySetInnerHTML wasn't
  displaying correctly — the <span> container showed but the SVG inside
  had no explicit dimensions. Now injects width="100%" height="100%"
  style="display:block" into the SVG root element before caching, so
  it fills the 16x16 container reliably.

- Mp4decrypt and MediaInfo version display in Component Library (#296)

Two fixes in get_tool_version():

  1. mp4decrypt: Has no --version flag. Now runs with no args and parses
     "Bento4 Version X.Y.Z" from the usage output. Previously showed
     "ERROR: missing output filename".

  2. All tools: get_tool_version() now uses extract_version_from_output()
     (the structured parser) instead of returning raw first-line output.
     This fixes MediaInfo showing "MediaInfo Command line," (line 1)
     instead of "26.01" (parsed from line 2's "MediaInfoLib - v26.01").

  Combined stdout+stderr before parsing so tools that output to either
  stream are handled consistently.

- Stream companion download output to activity log, add per-file progress (#294)

Three major improvements to download visibility:

  1. Companion downloads now stream stdout/stderr line-by-line to the
     activity log in real-time, same as the primary download. Previously
     used wait_with_output() which swallowed all output until completion,
     leaving minutes of silent activity. Users can now see per-track
     [Track N/M] and [download] progress for companions.

  2. Companion GAMDL CLI args are now logged (verbose level) so users
     can verify the codec being requested (e.g., --song-codec-priority alac).

  3. ReplayGain and AcoustID now emit per-file progress to the activity
     log: "ReplayGain: analysing file N/M — filename.m4a" and
     "AcoustID: fingerprinting file N/M — filename.m4a".

  Partially addresses #294. Remaining: progress bar tracking for
  companions/enrichment, completion timing.

- Keep queue item in Processing state until companions finish (#294)

Previously, the queue item was marked Complete immediately after the
  primary GAMDL download finished, even though enrichment and companion
  downloads were still running in background tasks. The progress bar
  showed the item as complete while files were actively being downloaded.

- Track enrichment JoinHandle, emit companion progress events (#294)

Two remaining fixes for comprehensive progress tracking:

  1. Enrichment tokio::spawn now returns a JoinHandle that the completion
     task awaits alongside the companion handle. Previously enrichment
     was fire-and-forget — the item could mark Complete before enrichment
     finished writing tags/lyrics.

  2. Companion stdout reader now parses GAMDL output lines and emits
     gamdl-output progress events (same as the primary download). This
     means the per-item progress bar can show companion download progress
     as a percentage instead of an indeterminate "Processing..." bar.

- Processing label shows current activity in progress bar (#294)

New processing_label field on QueueItemStatus carries what's happening
  during the Processing state. The progress bar and queue item card show
  the label instead of generic "Processing...":

  - "Enriching metadata tags..."
  - "Converting lyrics (Enhanced LRC)..."
  - "Downloading animated artwork..."
  - "AcoustID fingerprinting..."
  - "ReplayGain loudness analysis..."

  The enrichment task sets the label via set_processing_label() at each
  stage entry. GlobalProgressBar displays processing_label with priority
  over current_track.

- Defer desktop notification until all background work completes (#294)

Desktop notification "Download Complete" was firing immediately after
  the primary GAMDL download, before enrichment and companions finished.
  Moved send_desktop_notification() to the completion task that awaits
  both enrichment and companion handles.

- Queue-level progress counts 'processing' items as done (#294)

Items in Processing state (enrichment + companions running) now count
  as "done" in the queue-level progress bar. Previously the bar sat at
  0% during the entire post-download phase (~20 min for 28-track albums
  with companion tiers).

  The user's files are downloaded when state=processing — enrichment
  and companions are bonus background processing that shouldn't hold
  back the queue progress indicator.

  Also confirmed: companion speed/ETA already works via gamdl-output
  events emitted in commit d48ff58. No additional changes needed for
  item 4 (companion speed/ETA display).

- Use PNG logo+logotype in README with dark/light mode support

Replaced SVG logo and logotype with PNGs using GitHub's <picture>
  element for dark/light mode detection. The SVG logotype relied on
  JavaScript for dynamic text scaling which GitHub strips out, making
  it render incorrectly.

  Now uses:
  - logo.png / logo-dark.png (static icon, renders correctly)
  - logotype.png / logotype-dark.png (static wordmark, no JS needed)

- Show companion download speed/ETA in progress bar (#294)

Companion downloads now display speed, ETA, and percentage in the global
  progress bar instead of showing a generic indeterminate "Processing..."
  animation. The download store no longer regresses item state from
  'processing' to 'downloading' during companion events, preventing the
  queue bar from oscillating. Speed/ETA are cleared on processing_step
  transitions to avoid stale data during enrichment.

  Also fixes: CodeQL CI failures by adding actions:read permission,
  README org URLs (MeedyaDL → MWBMPartners), screenshots placeholder
  removal.

- Correct Apple Music API endpoint for MusicKit authentication
- Polish Apple Music API and syllable-lyrics implementation

- Add set_label() for syllable-lyrics enrichment step (UI progress)
  - Add 200ms rate limiting between syllable-lyrics API requests
  - Add cookie expiry validation in extract_media_user_token()
  - Fix Origin header in credentials validation (only for amp-api host)
  - Update stale amp-api references in Cargo.toml and DEV_NOTES.md
  - Update CLAUDE.md with syllable-lyrics documentation

- Update logo sources for better color scheme support in README.md
- Remove unnecessary line break in logo section of README.md
- Update logo source for light color scheme in README.md
- Replace redacted credential placeholders with proper fake test values

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add macOS Gatekeeper fix to release notes

Unsigned apps trigger macOS Gatekeeper's "damaged" warning. Add
  instructions to run xattr -cr to remove the quarantine flag.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update changelog and docs with CI/workflow fixes [skip ci]

Document the release-please state fix, Linux ARM cross-compilation
  apt fix, release workflow manual dispatch with tag input, Windows
  PowerShell shell fix, and git remote URL update.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Enhance quality settings recommendations for audio codecs
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update quality settings and codec reliability information in documentation and UI
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update HelpViewer.tsx with links for cookie export and Apple Developer keys
- Update HelpViewer.tsx to clarify MusicKit key creation instructions
- Update CHANGELOG.md [skip ci]
- Update DEV_NOTES and CHANGELOG to document macOS codesign timestamp workaround and future MusicKit integration
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update documentation for startup crash fix and external link handling

- CHANGELOG.md: Add entries for both bug fixes in [Unreleased] section
  - CLAUDE.md: Update queue persistence convention (blocking_lock, async
    runtime spawn) and Updates page convention (shell plugin for links)
  - Project_Plan.md: Note external link handling on Updates page entry
  - help/troubleshooting.md: Add "Crash on Launch (Queue Recovery Panic)"
    section with cause, fix version, and workaround for older versions

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Enhance README and HelpViewer with wrapper authentication details and connectivity troubleshooting
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Comprehensive documentation update for v0.6.2-v0.6.3 features

Update all documentation to reflect recent changes:

  - help/lyrics-and-metadata.md: 7-stage enrichment pipeline, lyrics
    format fallback chain, Enhanced LRC companion format selection
  - help/troubleshooting.md: FUSE mount/cloud mount hang documentation
  - help/quality-settings.md: native priority codec suffix behavior,
    --song-codec-priority technical note
  - Project_Plan.md: v0.6.2 and v0.6.3 completed features (7 items)
  - Dev_Notes.md: GAMDL 2.9.1 CLI flag changes, enrichment blocking
    I/O fix documentation
  - README.md: lyrics format fallback feature bullet
  - CLAUDE.md: 7-stage enrichment pipeline with lyrics fallback (Step 2b)

  Also saves standing tasks to .claude/ memory for session persistence.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add codecs.toml editing guide to Dev_Notes.md

Comprehensive guide for developers on how to add/modify entries in
  codecs.toml: audio codecs, video codecs, lyrics formats, and meta
  codecs. Includes how to find service mapping values from each engine's
  CLI help, practical examples (MP3, unmapped codecs), and when code
  changes are vs aren't required.

- Standing tasks sweep — update Project_Plan and CLAUDE.md

Add codec registry infrastructure and JS obfuscation to Project_Plan
  completed features list. Update CLAUDE.md Key Directories to include
  codec_registry module, template-parser lib, and codec-registry types.

- Add Raspberry Pi GDebi installation note to release pages

Raspberry Pi users may need GDebi to install .deb packages with
  dependencies resolved. Added note to release.yml template and all
  existing releases: sudo apt install gdebi-core && sudo gdebi ...

- Update documentation for WebVTT, MusicBrainz, and v0.6.3+ features

Update all documentation to reflect recent features:
  - CLAUDE.md: 9-stage enrichment pipeline, WebVTT/MusicBrainz services
  - CHANGELOG.md: Full [Unreleased] entries for WebVTT, MusicBrainz 3-tier
    discovery, codec registry, terser, pre-release flag, download links
  - Project_Plan.md: 5 new completed features
  - README.md: WebVTT and MusicBrainz feature bullets + checklist items
  - help/lyrics-and-metadata.md: WebVTT and MusicBrainz help sections,
    updated enrichment stage list (2c, 6b)
  - services/mod.rs: Updated module map and MusicBrainz doc comment

- Link roadmap features to GitHub Issues and organize project tracking

- Created 5 new GitHub Issues for planned/future features:
    #107 (multi-service architecture), #108 (enhanced MusicKit),
    #109 (native SwiftUI), #110 (smart download), #111 (full i18n)
  - Closed #105 (Apple Music support — already fully implemented)
  - Added all open issues (#44, #100-104, #106-111) to GitHub Project
  - Updated README.md roadmap tables with Issue column and links
  - Updated Project_Plan.md roadmap overview with Issue column,
    added issue links to Milestone 8/9/10 headers, added rows for
    Enhanced MusicKit, i18n, remote disable, SwiftUI, crash relay

- Add GitHub Issue tracking as formal standing task

Standing task #4 now requires creating/closing/linking GitHub Issues
  for every task (features, bugs, enhancements, security) and adding
  them to the "MeedyaDL Development" project. Parent/child dependencies
  must be cross-referenced. Follow-up work must get its own issue.

  Updated in both CLAUDE.md (project instructions) and memory/MEMORY.md
  (session persistence).

- Comprehensive documentation update for metadata, subtitles, and tags.toml

- CHANGELOG.md: Add ASS subtitles, verbose logging, comprehensive API metadata,
    dual-namespace tagging, tags.toml, API audit tool, Dependabot entries
  - DEV_NOTES.md: Add tags.toml editing guide (schema, JSON path syntax, value
    types, namespace conventions, step-by-step "Adding a New Tag" section), subtitle
    and lyrics generation section (6-step pipeline, format comparison, embedding atoms)
  - Project_Plan.md: Add 11 recently delivered features to post-release list
  - README.md: Expand Metadata & Extras section with Rich SRT, ASS, subtitle
    embedding, config-driven tags, API audit tool, comprehensive enrichment details
  - help/lyrics-and-metadata.md: Update enrichment pipeline to 12 stages, expand
    API tag table with all 30+ atoms, add tags.toml cross-reference
  - Fix pre-existing pedantic clippy suggestions: needless raw string hashes
    (replaygain_service), map_unwrap_or (gamdl), redundant closures (download_queue)
  - MeedyaManager#11: Mirror issue for subtitle/lyrics format support

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Fix markdownlint warnings in CHANGELOG.md [skip ci]

Fix double blank lines (MD012) and inconsistent indentation (MD007)
  in manually-edited [Unreleased] and [0.6.7] sections. Deduplicate
  repeated "Update CHANGELOG.md" entries in [0.6.6]. Add issue numbers
  to changelog entries.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update mirror repo reference to MeedyaDL/MeedyaDL-Tools

Renamed mirror repository from MWBMPartners/meedyadl-tools to
  MeedyaDL/MeedyaDL-Tools across code, config, and documentation.
  Also fixed example asset extension (.zip → .tar.gz) in tool-versions.toml.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Fix clickable URL in wrapper help and expand MusicKit setup guide

- Replace clickable http://192.168.3.179:30020 in Help > Wrapper >
    Automatic Pre-Flight Check with non-clickable backtick-wrapped
    http://127.0.0.1:30020
  - Significantly expand Step 2 (Create a MusicKit Key) in Help >
    Animated Artwork with detailed instructions covering: free vs paid
    account checkbox differences (MusicKit vs Media Services), the
    Configure/App ID flow, direct URL for the Keys page, and a tip for
    when the MusicKit option doesn't appear

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update changelog, readme, and project context for latest changes
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with new features and architecture details [skip ci]

- Add pre-release version handling, collapsible About, component versions
  - Add drag-and-drop, batch paste, download history, notifications, deep links
  - Update enrichment pipeline count (11→12 stages)
  - Add keyboard shortcuts, settings sidebar, accessibility, storefront config
  - Add ISRC handling, codec suffix rename, activity log search documentation
  - Update key directories with missing entries (hooks, styles, history_service)

- Add MeedyaSuite logotype customisation guide to Dev_Notes [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update README, Dev_Notes, Claude memory with brand asset docs
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with brand assets, directory structure, conventions [skip ci]

- Added assets/brand/ and public/ to Key Directories
  - Updated scripts/ description to include icon/APNG generators
  - Added CodeQL to workflows list
  - Added brand assets convention (proprietary license, SVG sources,
    sidebar usage, regeneration scripts, copyright year)

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md and memory with security hardening details [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with manifest files, library URLs, codec suffix, queue clear, wrapper logging [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update help topics for manifest files, codec detection, queue management (#256)

- downloading-music.md: .meedyadl manifest files, Import button,
    drag-and-drop, library URL support
  - quality-settings.md: custom companion mode, ffprobe/MediaInfo codec
    detection, codec suffix accuracy
  - faq.md: .meedyadl files, queue Clear All, library URLs
  - troubleshooting.md: false failure fix, auth mode logging, log export

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with missing modules and ASS enrichment stage [skip ci]

Add 4 missing services (health_check, api_audit, ass_subtitle, mediainfo),
  2 missing commands (api_audit, history), 1 missing model (tag_registry) to
  Key Directories. Add ASS subtitle generation as enrichment step 2f.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Remove [skip ci] convention — all pushes must trigger CI

Updated CLAUDE.md to explicitly prohibit [skip ci] in commit messages
  unless the user explicitly requests it. Every push to main must trigger
  CI, Release Please, and CodeQL workflows for proper validation.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add smart re-download detection to in-app help (#263)

- downloading-music.md: new "Smart Re-Download Detection" section
    covering feature overview, settings toggle, detectable changes,
    and limitations
  - faq.md: new Q&A entry cross-referencing the full help section

- Update CLAUDE.md with smart re-download detection and recent fixes
- Update CHANGELOG.md [skip ci]
- Add smart re-download detection section to DEV_NOTES.md (#263)

Documents the full implementation: API field extraction, manifest
  storage, tag embedding, IPC command, frontend integration, detectable
  vs non-detectable changes, and key files reference.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add meedyadl-v2 branch archive section to DEV_NOTES.md

Documents the closed PR #24 (meedyadl-v2 branch), mapping each v2
  feature to its status on main (reimplemented, superseded, or tracked
  as open issue). Includes recommendations for future multi-service work:
  use fresh feature branches, adapt v2 patterns don't copy code, and
  use mirror-based tool management instead of bundled deps.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add engines.toml editing guide to DEV_NOTES.md (#268, #270)

Documents the engine registry file structure, priority system, and
  step-by-step guides for adding engines, adding platforms, changing
  priority, and removing engines. Includes current registry table and
  implementation status tracking.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add SVG logo+logotype to README, update roadmap and architecture

- Logo: Added animated SVG logo (logo.svg) and logotype (logotype.svg)
    to the README header for crisp rendering at any resolution
  - Roadmap: Added M11 (OnlyFans/OF-Scraper), engine priority system
    (#268), smart re-download detection, MediaInfo, stable rollback (#267)
  - Architecture: Updated diagram to show engine registry layer and all
    5 download engines (GAMDL, votify, yt-dlp, get_iplayer, OF-Scraper)
  - Credits: Added votify, yt-dlp, get_iplayer, OF-Scraper, MediaInfo
  - Setup: Updated first-run to mention 5 required tools + MediaInfo
  - engines.toml: Added required/enabled fields per engine and platform

- Update CHANGELOG.md [skip ci]
- Update in-app help with MediaInfo, votify, smart re-download detection

- index.md: Updated project description to mention multi-service plans
  - getting-started.md: Added votify and MediaInfo to dependency lists
  - downloading-music.md: Added smart re-download detection section
  - faq.md: Added smart re-download detection Q&A entry

- Update CHANGELOG.md [skip ci]
- Remove OnlyFans/OF-Scraper references from all public documentation

OnlyFans support remains as an internal/private roadmap item but should
  not appear in public-facing documentation due to the platform's
  controversial nature.

  Removed from: README.md (roadmap, architecture, credits), DEV_NOTES.md
  (engine table), and code comments (dependencies.rs, tauri-commands.ts).

  The engines.toml entry and Rust/TypeScript code remain (compiled into
  binary, hidden when enabled=false) for infrastructure readiness.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update help docs with ReplayGain album gain, reference level, and clipping options

Updated lyrics-and-metadata.md ReplayGain section to document all 4
  tags (track gain/peak + album gain/peak), configurable reference level
  options, and clipping prevention setting.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update all documentation for #294 progress tracking and security hardening

Update CLAUDE.md, DEV_NOTES.md, Project_Plan.md, and in-app help with
  companion progress tracking architecture, processing labels, and
  CodeQL CI fix details. Add SVG sanitization (DOMParser + event handler
  stripping) to GlobalProgressBar's inline SVG rendering for
  defence-in-depth against potential XSS via tampered platform icons.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Fix markdownlint warnings in README.md

Add blank lines after headings (MD022) and before lists (MD032).
  Add spaces to table separator rows for consistent padded style (MD060).

- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Update status color classes and add animated artwork help content
- Simplify TitleBar component to return null for all platforms
- Unify error reporting for crashes and download failures, update UI and documentation
- Reorganise Advanced settings tab section order

- Move File Options above Error Reporting
  - Move API Credentials just above Setup
  - Move API Field Audit from Metadata tab into Advanced tab (below
    AcoustID, within the API Credentials section)

  New order: Processing → Wrapper → File Options → Error Reporting →
  Diagnostics → API Credentials (MusicKit, AcoustID, API Field Audit) →
  Setup

- Add collapsible SettingsSection component to all settings tabs

Create a reusable SettingsSection component with bordered card styling,
  clickable header with rotating chevron, and collapsible content. Apply
  it across all 10 settings tabs (General, Quality, CoverArt, Lyrics,
  Metadata, Tools, Fallback, Templates, Cookies, Advanced) for consistent
  visual distinction between sections. Tighten inter-section spacing from
  space-y-6 to space-y-3 for a more compact layout.

- Streamline macOS menu setup in run function

### 🧪 Testing

- Update LyricsTab tests to use regex for format labels
- Add complex integration tests for TTML and WebVTT rich SRT conversion

Two new end-to-end tests exercising real-world scenarios:

  - ttml_to_rich_srt_complex_mixed_styling: plain text, italic verse,
    named style (bold+colour), mixed spans with bold+background vocals,
    underline+named colour — verifies all 5 cue types in one test

  - webvtt_to_rich_srt_complex_mixed_tags: plain text, preserved <b>/<i>,
    stripped <c> class tags, stripped inline timestamps, stripped <v> voice
    tags — verifies SRT-compatible tag preservation and VTT-only stripping

- Add enrichment pipeline integration tests (closes #113)

Add 30 end-to-end integration tests across 4 subtitle/lyrics services:
  - Rich SRT: TTML→SRT conversion, styling, multi-track, unicode filenames
  - WebVTT: TTML/SRT/LRC→VTT conversion, source priority, fallbacks
  - Enhanced LRC: word-level timing, line-level fallback, multi-track
  - ASS: TTML→ASS conversion, styling override tags, VTT fallback
  - Cross-service pipeline tests and CJK/emoji filename edge cases

  Total Rust tests: 579 → 609 (+30)

- Add React component rendering tests for settings tabs (closes #114)

Add 20 Vitest tests for GeneralTab, QualityTab, and AdvancedTab covering
  toggle rendering, toggle click handling, conditional visibility, and select
  dropdown rendering. Mocks lucide-react icons, Tauri IPC commands, and the
  shell plugin to enable jsdom testing without the Tauri runtime.

- Add storefront config.ini generation tests

Verify storefront is written to config.ini:
  - Auto-detect from language (en-US → us)
  - Explicit override when set (gb)

- Add library URL parser tests and update content type label tests (#232)

Added 4 new tests for library URL parsing (albums, songs, playlists,
  recently-added). Updated getContentTypeLabel test to cover the new
  'library' content type. Total: 260 tests (was 256).

- Add activityStore unit tests (#232)

6 tests covering: initial state, entry addition, ordering, no entry
  cap (verifies old 5000 limit was removed), clearEntries, paused state.
  Total: 266 tests across 19 test files.


### 🔄 CI/CD

- Add npm audit security scanning to CI pipeline

Adds `npm audit --audit-level=high` step to the frontend CI job,
  running after npm ci install. Fails the build only on high/critical
  severity vulnerabilities in npm dependencies.

  Also created GitHub Issues for project recommendations:
  - #112: cargo deny licence scanning for Rust dependencies
  - #113: end-to-end integration tests for enrichment pipeline
  - #114: React component rendering tests for settings tabs
  - #115: dependency freshness checks (npm outdated, cargo outdated)
  - #116: Wiki sync with in-app help documentation

  All issues added to MeedyaDL Development project.

- Add Dependabot version updates for automated dependency freshness

Configures Dependabot to create weekly PRs for semver-compatible
  (minor + patch) updates to both npm and Cargo dependencies. Major
  version jumps are excluded (tracked separately in #117).

- Add CodeQL workflow excluding Rust analysis

Override GitHub's dynamic "Default setup" CodeQL configuration with an
  explicit workflow that analyses only actions and javascript-typescript.

  Rust analysis is excluded because CodeQL's Rust extractor requires a
  full Cargo build, which routinely hangs for 6+ hours on this project
  (see Actions run #500). Rust code quality is already covered by
  cargo clippy, cargo test, and cargo-deny in ci.yml.

- Upgrade pinned GitHub Actions from Node.js 20 to Node.js 24 (#241)

actions/checkout v4→v6.0.2, actions/setup-node v4→v6.3.0,
  Swatinem/rust-cache v2.8.2→v2.9.1 — all now use Node.js 24 runtime,
  resolving the deprecation warning before the June 2, 2026 deadline.

- Update CodeQL actions from v3 to v4 (SHA-pinned)

CodeQL Action v3 is deprecated in December 2026 and uses Node.js 20
  (also deprecated). v4 uses Node.js 22 and resolves both warnings.

  Pinned to immutable SHA d4b3ca9fa7f69d38bfcd667bdc45bc373d16277e
  per supply chain hardening convention.


### 🧹 Maintenance

- Add auth/ to .gitignore to prevent secret leaks
- Update version to 0.1.3 in Cargo.lock and enhance project documentation
- Add temporary PAT diagnostic workflow [skip ci]

Temporary workflow to verify RELEASE_PAT permissions.
  Run via: gh workflow run "Check PAT" --ref main
  Delete after verification.

- Add workflow_dispatch to release workflow [skip ci]

Allow manual trigger for re-running builds when tag push events
  are missed (e.g., after billing blocks or tag re-pushes).

- Remove PAT diagnostic workflow [skip ci]

RELEASE_PAT verified working — the original failure was caused by
  billing/spending limit, not token permissions.

- Update version to 0.3.2 and enhance documentation
- Bump version to 0.3.3 and update changelog with bug fixes and changes
- Update milestone versions in project documentation and roadmap
- Update dependencies and add new icons

- Updated `vitest` from `^2.1.8` to `^4.0.18` in `package.json`.
  - Added `sharp` dependency with version `^0.34.5`.
  - Updated various icon files in `src-tauri/icons` for different resolutions and platforms, including:
    - New icons for Android adaptive launcher and various mipmap resolutions.
    - New iOS app icons for multiple sizes.
    - Updated existing icon files for various resolutions.

- Update dependencies (npm + cargo semver-compatible)

npm updates (8 packages, all semver-compatible):
  - @eslint/js 9.39.2→9.39.3, eslint 9.39.2→9.39.3
  - @sentry/browser 10.40.0→10.42.0
  - @types/react 19.2.13→19.2.14
  - @typescript-eslint/eslint-plugin 8.54.0→8.56.1
  - @typescript-eslint/parser 8.54.0→8.56.1
  - autoprefixer 10.4.24→10.4.27
  - i18next 25.8.10→25.8.13, postcss 8.5.6→8.5.8

  Cargo updates (7 packages):
  - tokio 1.49.0→1.50.0
  - aws-lc-rs 1.16.0→1.16.1, aws-lc-sys 0.37.1→0.38.0
  - getrandom 0.4.1→0.4.2, ipnet 2.11.0→2.12.0
  - minisign-verify 0.2.4→0.2.5

  Major version jumps deferred (tailwindcss 4, vite 7, eslint 10,
  commitlint 20, etc.) — require migration effort.

  All 516 Rust + 231 frontend tests pass. 0 vulnerabilities.

- Add markdownlint ignore for auto-generated and internal files

Exclude CHANGELOG.md (auto-generated by git-cliff) and .claude/
  (internal development context) from markdownlint checks. Also added
  .vscode/settings.json (gitignored) with workspace-level markdownlint
  ignore config and documentation of Edge DevTools false positives.

- **(deps-dev)** Bump vite from 6.4.1 to 7.3.1

Bumps [vite](https://github.com/vitejs/vite/tree/HEAD/packages/vite) from 6.4.1 to 7.3.1.
  - [Release notes](https://github.com/vitejs/vite/releases)
  - [Changelog](https://github.com/vitejs/vite/blob/main/packages/vite/CHANGELOG.md)
  - [Commits](https://github.com/vitejs/vite/commits/v7.3.1/packages/vite)

  ---
  updated-dependencies:
  - dependency-name: vite
    dependency-version: 7.3.1
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump @commitlint/config-conventional

Bumps [@commitlint/config-conventional](https://github.com/conventional-changelog/commitlint/tree/HEAD/@commitlint/config-conventional) from 19.8.1 to 20.4.3.
  - [Release notes](https://github.com/conventional-changelog/commitlint/releases)
  - [Changelog](https://github.com/conventional-changelog/commitlint/blob/master/@commitlint/config-conventional/CHANGELOG.md)
  - [Commits](https://github.com/conventional-changelog/commitlint/commits/v20.4.3/@commitlint/config-conventional)

  ---
  updated-dependencies:
  - dependency-name: "@commitlint/config-conventional"
    dependency-version: 20.4.3
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump eslint-plugin-react-hooks from 5.2.0 to 7.0.1

Bumps [eslint-plugin-react-hooks](https://github.com/facebook/react/tree/HEAD/packages/eslint-plugin-react-hooks) from 5.2.0 to 7.0.1.
  - [Release notes](https://github.com/facebook/react/releases)
  - [Changelog](https://github.com/facebook/react/blob/main/packages/eslint-plugin-react-hooks/CHANGELOG.md)
  - [Commits](https://github.com/facebook/react/commits/HEAD/packages/eslint-plugin-react-hooks)

  ---
  updated-dependencies:
  - dependency-name: eslint-plugin-react-hooks
    dependency-version: 7.0.1
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump jsdom from 25.0.1 to 28.1.0

Bumps [jsdom](https://github.com/jsdom/jsdom) from 25.0.1 to 28.1.0.
  - [Release notes](https://github.com/jsdom/jsdom/releases)
  - [Changelog](https://github.com/jsdom/jsdom/blob/main/Changelog.md)
  - [Commits](https://github.com/jsdom/jsdom/compare/25.0.1...28.1.0)

  ---
  updated-dependencies:
  - dependency-name: jsdom
    dependency-version: 28.1.0
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- Add .hintrc to suppress false-positive webhint warnings

Disable three webhint rules that produce false positives on React/JSX:
  - axe/aria: can't evaluate JSX ternary expressions for ARIA attributes
  - no-inline-styles: dynamic runtime values and ErrorBoundary styles
  - css-prefix-order: fixed where possible, remaining are intentional

- Configure changelog sections and document commit conventions

- release-please-config.json: Added changelog-sections mapping so
    refactor/perf/test appear under "Improvements" and chore/docs/ci
    are hidden from the changelog.
  - README.md: Updated commit convention section with version bump
    table and guidance on when to use each prefix.
  - DEV_NOTES.md: Same table, removed obsolete [skip ci] guidance,
    fixed ordered list numbering.

- Add bundled field to engines.toml for core vs external distinction

Each engine now has an explicit `bundled` field:
  - bundled=true: core engine, pip-installed into managed Python env,
    packaged with MeedyaDL, no custom path override allowed
  - bundled=false: external tool, user-installed, supports custom path
    in Settings > Tools

  Updated DEV_NOTES.md with bundled vs external documentation, current
  registry table with bundled/custom-path columns, and CI packaging
  guidance for reading engines.toml during release builds.

- Make get_iplayer bundled via MeedyaDL-Tools mirror (not user-installed)

get_iplayer is Perl-based (not pip), but pre-built binaries already
  exist in the MeedyaDL-Tools mirror for all platforms. Changed from
  bundled=false/install_method=system to bundled=true/install_method=binary
  so it's downloaded from the mirror during setup — same as FFmpeg,
  mp4decrypt, MediaInfo.

  All engines are now bundled. No external/user-installed engines remain.

- Add offline installer option to release workflow (#280)

New workflow_dispatch input `bundle_engines` (default: false) controls
  whether the release build pre-bundles all engines and tools for a
  zero-setup offline installer (~300MB vs ~30MB tiny installer).

  When bundle_engines=true, Step 8.5 in release.yml:
  1. Parses engines.toml for bundled+enabled pip engines → pip install
  2. Parses engines.toml for bundled+enabled binary engines → mirror download
  3. Downloads binary tools (FFmpeg, mp4decrypt, etc.) from MeedyaDL-Tools mirror
  4. Writes manifest.json with offline_installer=true

- Auto-generate ACKNOWLEDGEMENTS.md with licence hyperlinks

New script scripts/generate-acknowledgements.mjs dynamically generates
  ACKNOWLEDGEMENTS.md from three source files:
  - engines.toml (only enabled engines)
  - Cargo.toml (Rust dependencies)
  - package.json (frontend dependencies)

  Each licence name is hyperlinked to the official licence text (e.g.,
  MIT links to opensource.org/licenses/MIT). Links open in the system
  browser when clicked in the HelpViewer.

  Also added Package Manifests section to DEV_NOTES.md explaining
  what Cargo.toml and package.json are used for.


### Revert

- Restore original pulsating dot size and vertical positions

Revert the dot height/radius changes from the previous commit.
  Dots return to cy=58/72/86, r=3 (original colon-like positions).
  Dynamic script only repositions dots horizontally, not vertically.


### Security

- Sanitize newlines in INI config values (closes #226)

Add sanitize_ini_value() that strips \n and \r from all user-provided
  string values before writing to GAMDL's config.ini. Prevents INI
  injection via crafted settings import files where a newline in a
  path/URL/template value could inject arbitrary configparser keys.

  Applied to: cookies_path, output_path, temp_path, wrapper_account_url,
  all 6 template strings, all 4 tool paths.

- Add rehype-sanitize to HelpViewer markdown rendering (closes #227)

Add rehype-sanitize alongside rehype-raw to strip dangerous HTML
  elements (script, iframe, event handlers) while preserving safe ones
  (details, summary, strong, em, etc.).

  Custom schema extends GitHub's default to allowlist <details> and
  <summary> elements needed for collapsible About sections.

- Replace sh -c format! with direct process invocations (closes #228)

Eliminated two sh -c shell command constructions in dependency_manager.rs:

  1. GPAC .pkg extraction: replaced gunzip|cpio pipe with two-step
     process (gunzip to temp file, then cpio -F). No shell involved.

  2. Debian .deb data extraction: replaced tar with shell glob
     (data.tar.*) with Rust read_dir + find to locate the archive
     file, then direct tar invocation with arg(). No shell involved.

  Both changes prevent potential shell injection if paths ever contain
  special characters. Paths are now passed as OS arguments, never
  interpolated into shell strings.

- Add field-level validation to settings import (closes #229)

Add sanitize_imported_settings() that validates/cleans all user-provided
  fields after JSON deserialization:
  - Truncate paths to 1024 chars, URLs to 2048, templates to 512
  - Strip \n and \r from all string values (INI injection prevention)
  - Truncate language/storefront to 20/10 chars
  - Limit exclude_tags array to 50 entries, each 100 chars max

  Applied before merging with current settings so crafted import files
  cannot inject excessively long strings or control characters.


---
*Generated with [git-cliff](https://git-cliff.org/)*
