# Codebase Unification Audit — v1.0.1 prep (Phase 4)

**Date**: 2026-05-08
**Branch**: `feat/v1.0.1-prep`
**Trigger**: Project owner requested a holistic check for duplicate patterns / functions / structures ahead of v1.0 stable + multi-service expansion (M8 BBC iPlayer, M9 Spotify, M10 YouTube). Stated rationale: clean up debt **now** so the multi-service work doesn't inherit it or duplicate the same bolt-on patterns per service.

This document captures the read-only audit findings, in **value-to-effort order**. Each finding has a tracker issue (linked in the heading) for the implementation follow-up. Recommended implementation order is at the end.

---

## 1. Recursive directory walkers (depth-limited) — issue: TBD

**Locations:**

- [`download_queue.rs:556-665`](../../src-tauri/src/services/download_queue.rs#L556-L665) — `find_album_directory`, `find_deepest_audio_dir`, `has_direct_audio_files`
- [`download_queue.rs:3699-3721`](../../src-tauri/src/services/download_queue.rs#L3699-L3721) — `collect_video_files_depth_limited`
- [`metadata_tag_service.rs:1868-1884`](../../src-tauri/src/services/metadata_tag_service.rs#L1868-L1884) — `collect_m4a_depth_limited`
- [`download_queue.rs:5101+`](../../src-tauri/src/services/download_queue.rs#L5101) — `find_dirs_with_ttml` (depth-bounded in #712)
- [`gamdl.rs:1791+`](../../src-tauri/src/commands/gamdl.rs#L1791) — `scan_dir_for_manifests_recursive`

**Pattern:** Five+ functions implement the same shape: walk a filesystem tree up to `max_depth`, filter by file extension or presence-test, accumulate results. Each reimplements depth-tracking, recursion, extension matching.

**Consolidation:** Extract a generic `walk_dir_depth<T, F>(base, max_depth, visitor: F) -> Vec<T>` helper in `utils/`. Each callsite passes its specific predicate.

**Size:** 40-50 lines refactored. **Risk:** low (purely internal, test coverage exists for the highest-impact callers).

**Multi-service relevance:** **High** — BBC iPlayer (video) and Spotify (audio) will need similar scans.

---

## 2. HTTP client builder boilerplate — issue: TBD

**Locations:**

- [`apple_music_api.rs`](../../src-tauri/src/services/apple_music_api.rs) — 6+ `reqwest::Client::builder().timeout(...).build()` instances
- [`musicbrainz_service.rs`](../../src-tauri/src/services/musicbrainz_service.rs) — 3+ instances
- `archive.rs`, `commands/credentials.rs`, `acoustid_service.rs`, `service_status.rs`, `pip_engine_service.rs` — one each

**Pattern:** Each service rebuilds the reqwest client with timeout (5-30s) and optional user-agent, with the same error-message pattern.

**Consolidation:** `utils/http_client.rs` with `create_http_client(timeout_secs, user_agent) -> Result<Client, String>`. Per-service timeout constants stay local.

**Size:** 30-40 lines. **Risk:** low.

**Multi-service relevance:** **High** — Spotify, YouTube, BBC iPlayer all need HTTP clients. Centralisation enables future retry / logging policies to be applied uniformly.

---

## 3. CLI argument builder pattern (Option<T> → Vec<String>) — issue: TBD

**Locations:**

- [`gamdl_options.rs:767-841`](../../src-tauri/src/models/gamdl_options.rs#L767-L841) — `to_cli_args`, `audio_cli_args`, `video_cli_args`, `path_cli_args`, `flag_cli_args` (the canonical impl)
- [`votify_options.rs`](../../src-tauri/src/models/votify_options.rs) — struct only, no builder
- [`ytdlp_options.rs`](../../src-tauri/src/models/ytdlp_options.rs) — struct only, no builder
- [`get_iplayer_options.rs`](../../src-tauri/src/models/get_iplayer_options.rs) — struct only, no builder

**Pattern:** GAMDL has a well-structured CLI builder; the other three services need one. The "if Some, push flag and value" fold will repeat 3 times.

**Consolidation:** `CliArgsBuilder` trait with default methods (`push_string_flag`, `push_bool_flag`, `push_enum_flag`). Each service struct implements the trait.

**Size:** 60-80 lines shared infrastructure + 20-30 per service. **Risk:** medium (trait design needs to handle enum variants + heterogeneous types).

**Multi-service relevance:** **Very high** — saves ~200 lines across the three new services.

---

## 4. Per-service settings structure — issue: TBD

**Location:** `PerServiceSettings` in [`settings.rs`](../../src-tauri/src/models/settings.rs)

**Pattern:** Nested struct with sub-structs per service (`gamdl: GamdlOptions`, `spotify: VotifyOptions`, …). Grows linearly with each new service.

**Consolidation:** Either macro-generate from a service registry, or a generic `ServiceDefaults<T: Default>` map.

**Size:** 50-100 lines. **Risk:** medium-to-high (changes settings schema; needs migration).

**Multi-service relevance:** **Very high** — without consolidation, schema bloats per service.

---

## 5. Frontend Zustand store load/save/persist pattern — **primitive landed v1.0.7**

**Locations:** [`src/stores/*.ts`](../../src/stores/) — settingsStore, downloadStore, activityStore, dependencyStore, updateStore, serviceStatusStore (6+ stores).

**Pattern:** Each store does load → save → optional debounce → persist via `tauri-commands.ts` IPC. Boilerplate is nearly identical.

**Consolidation:** [`createAsyncResourceStore<T>`](../../src/lib/createAsyncResourceStore.ts) factory — config takes `defaults`, `load`, optional `save`, optional `debounceMs`. Returns a Zustand hook with `data` / `isLoading` / `isDirty` / `error` reactive state and `load` / `save` / `debouncedSave` / `update` / `reset` actions. Read-only stores (no `save` config) get silent no-ops on save paths so consumers don't need to narrow.

**Size:** 165 lines factory + 15 tests (15 pass). **Risk:** low — additive primitive, no existing store touched.

**Multi-service relevance:** **Medium** — saves ~50 lines per per-service settings page; primary consumer is the M8/M9/M10 per-service settings stores when those land.

**Migration status:** factory landed in v1.0.7; **migration of existing stores deferred** to a follow-up cycle. Each existing store has 30+ component consumers using the per-store API names (`settings`, `loadSettings`, `saveSettings`, etc.) — a full rename to the generic API (`data`, `load`, `save`) would be a high-touch refactor whose value is questionable for already-working stores. New stores should use the factory from day one.

---

## 6. TypeScript Tauri IPC wrappers (codegen opportunity) — issue: TBD

**Location:** [`src/lib/tauri-commands.ts`](../../src/lib/tauri-commands.ts) — 30+ wrapper functions, each 2-4 lines around `invoke<T>()`.

**Pattern:** Every `#[tauri::command]` Rust fn has a matching TS wrapper. Boilerplate scales with command count (currently ~100 lines, will hit 200+ by v2.0).

**Consolidation:** Build-time codegen from `#[tauri::command]` attributes (script in `build.rs` or dedicated codegen crate). One-time investment, future commands auto-generate.

**Size:** ~150 lines of generator. **Risk:** medium (codegen can be fragile).

**Multi-service relevance:** **Low** — one-time consolidation, not repeated per service.

---

## 7. Engine command builder trait + pip-engine sharing — issue: TBD

**Locations:**

- [`engine_runner.rs:69-91`](../../src-tauri/src/services/engine_runner.rs#L69-L91) — `EngineCommandBuilder` trait
- [`engine_runner.rs:227-318`](../../src-tauri/src/services/engine_runner.rs#L227-L318) — four impls (GAMDL real, three stubs)

**Pattern:** Three stub implementations return `Err("not yet implemented")`. The Votify and yt-dlp impls (when written) will both need pip-engine plumbing (Python path resolution, pip command construction).

**Consolidation:** Extract `PipEngineBuilder` base trait with shared `resolve_python_path()` / `build_pip_command()` defaults; Votify and yt-dlp inherit from it.

**Size:** 20-30 shared + 30-40 per pip engine. **Risk:** low-to-medium.

**Multi-service relevance:** **High** — Votify (M9) and yt-dlp (M10) both pip-installed.

---

## 8. JSON manifest atomic write — issue: TBD

**Locations:** download_queue.rs (manifest), commands/settings.rs (settings.json), history_service.rs (history.json), activity_log_writer.rs (activity log).

**Pattern:** Multiple services do "serialize → write tmp → atomic rename" with similar error-message strings.

**Consolidation:** `utils/atomic_write.rs::atomic_write_json<T: Serialize>(path, data) -> Result<(), String>`.

**Size:** 30-40 lines. **Risk:** low.

**Multi-service relevance:** **Medium** — Spotify/YouTube will likely need their own state files.

---

## Recommended implementation order

| # | When | Reason |
|---|---|---|
| 1, 2 | **Now** (Phase 4 of v1.0.1 prep) | Low risk, immediate wins, infrastructure for new services |
| 3, 7 | Before/with M8 (BBC iPlayer) | Design now, applied as Votify/yt-dlp impls land |
| 4 | After v1.0 stable | Needs migration strategy |
| 5, 6 | Maintenance / opportunistic | Lower priority, code-health refactors |
| 8 | When fourth state-file exists (e.g. Spotify session cache) | Premature today; clear value once it's a real pattern |

## Summary

| # | Title | LOC impact | Risk | Multi-service relevance |
|---|---|---|---|---|
| 1 | Recursive dir walkers | 40-50 | Low | High |
| 2 | HTTP client boilerplate | 30-40 | Low | High |
| 3 | CLI args builder | 60-80 + 60-90/svc | Medium | **Very High** |
| 4 | Per-service settings | 50-100 | Med-High | **Very High** |
| 5 | Frontend store pattern | 40-60 + 30-40/store | Low-Med | Medium |
| 6 | Tauri IPC wrappers | 150 (codegen) | Medium | Low |
| 7 | Engine builder + pip share | 20-30 + 60-80/pip svc | Low-Med | High |
| 8 | JSON atomic write | 30-40 | Low | Medium |
