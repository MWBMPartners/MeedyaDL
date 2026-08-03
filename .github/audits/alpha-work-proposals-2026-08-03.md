# Alpha-Cycle New-Work Proposals — 2026-08-03

Decision-grade menu produced from a fresh codebase review + the open-issue set (see
`issue-reconciliation-2026-08-03.md`). Ranked by value-per-effort. **Awaiting maintainer
go/no-go — nothing here is implemented yet** except #949 (a pure doc-correctness fix, done
in the same session's documentation pass).

**Clean-bill notes (no action):** `check_ipc_commands.py` / `check_codec_registry.py` pass
(136/136 commands registered); en/de/fr locales key-identical (133 each); no `console.log`
in `src/`; no non-test `.unwrap()`/`.expect()` in `src-tauri/src/commands/`; Spotify daily
cap persisted atomically. The list below is the genuine residue.

## Ranked (best value-per-effort first)

| Rank | Effort | Type | Issue | Summary |
|---|---|---|---|---|
| 1 | XS | bug | #991 | Clear/Abort undo re-queues per-URL then always toasts "Re-queued N"; >10 URLs eaten by the 10/min limiter. Fix = one batched `startDownload`. (`downloadStore.ts:588`) |
| 2 | XS | bug | #949 | **DONE (docs pass)** — in-app Help milestone numbers were scrambled vs README. Fixed in `help/supported-services.md` + `HelpViewer.tsx`. |
| 3 | S | bug | #983 | `DownloadForm` validates single URLs with `parseAppleMusicUrl` only → Spotify never reaches the dispatch gate. Use `detectService`/`parseMediaUrl`. (`DownloadForm.tsx:354`) |
| 4 | S | bug | #981 | **Confirmed broken**: BtbN Linux-x64 FFmpeg is `.tar.xz` declared `TarGz`; `archive.rs` has no xz decoder → primary extract 100% fails on Linux x64, mirror is silent SPOF. Add `TarXz` + extension→format mapping. (`archive.rs:379`, `dependency_manager.rs:782`) |
| 5 | S | bug | #978 | votify `from_settings` never sets `output_path`/`temp_path` → clean Spotify runs marked failed. (`votify_options.rs:330`) |
| 6 | XS | bug | #1011 | Album fetch omits `extend=audioTraits` while parser reads it → ADM/lossless tags depend on API volunteering. (`apple_music_api.rs:1031`) |
| 7 | S | tech-debt | NEW | Generate `HELP_TOPICS` from `help/*.md` at build time — kills the dual-source trap that caused #949. **Decision:** Vite `?raw` vs prebuild codegen. |
| 8 | M | enhancement | #963/#1002/#965 | De-stale wrapper-dependency model for GAMDL 3.8+ (Atmos/AC3 are cookie-only now); new `GamdlFeature::WrapperlessNonWebCodecs (>=3.8)` + fix 3 "(Experimental)" UI surfaces. **Decision:** soften wording vs flip until live smoke-test passes. |
| 9 | S | bug | #982 | NSIS `/D=` gets quoted → breaks install paths with spaces. Use `raw_arg()` on Windows. |
| 10 | M | enhancement | #1021 | Post-download ffprobe integrity guard (decodable + nonzero duration) → silent corruption becomes actionable error. |
| 11 | M | enhancement | #987 | Wire the unused `expected_sha256` for GPAC nightly `.exe` (runs unverified). **Decision:** pinned hashes vs moving "latest" tags. |
| 12 | S | bug | #997 | `sudo apt-get install gpac` no-TTY on Linux ARM → replace with detection + guidance. |
| 13 | S | enhancement | #961 (partial) | Animated-artwork fallback + geo-lock warning (defer Plex/log-split halves). |
| 14 | S | enhancement | #934 | Lyrics "test connection" IPC (also the hook to investigate #971 Media-User-Token=None). |
| 15 | XS | tweak | #1000 | Delete dead `fetch_extra_tags` plumbing (v2-only, inert). |
| 16 | XS | tweak | #998 | Add Sentry ingest host to CSP `connect-src` (rider on whichever PR wires the DSN). |
| 17 | S | enhancement | #973/#972 | `&l=` locale param on syllable/editorialVideo calls; `animated_artwork_resolution` setting. |
| 18 | L | tech-debt | NEW | Split `download_queue.rs` (17,104 lines) — **not during active alpha**; schedule after a stable cut. |

**Deferred (out of scope for alpha):** M8–M10 service milestones, cloud upload, SwiftUI,
#1001, #1013/#1014 (ARMv7 wheel upstream-blocked), #974 (fMP4 concat).

## Top 5 to do first for alpha
1. **#991** (XS) — testers hit the undo bug immediately.
2. **#983 + #978** (S+S) — the pair that makes the Spotify/M9 path testable end-to-end.
3. **#949** (XS) — **done** (docs pass).
4. **#981** (S) — restore Linux-x64 FFmpeg primary; kill the mirror SPOF.
5. **#1011** (XS) — one-line metadata-quality win.

## Needs a maintainer decision before starting
- **#987** — checksum source strategy (pin mirror hashes vs runtime `.sha256`; GPAC nightly moving target).
- **#8 (#963/#965)** — soften "(Experimental)" wording vs flip labels now (pre/post live smoke-test).
- **#7** — HelpViewer codegen approach (build-time raw import vs prebuild script).
