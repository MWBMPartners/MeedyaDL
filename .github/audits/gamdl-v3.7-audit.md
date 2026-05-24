# GAMDL v3.7 (+ v3.7.1 preview) — Compatibility Audit

**Audit date:** 2026-05-23 (v3.7 released 2026-05-23; v3.7.1 prep already on upstream `main`)
**MeedyaDL baseline:** v1.10.0 stable (GAMDL ceiling at 3.6 prior to this audit)
**Audit scope:** every commit between `3.6...3.7` (22 commits) + every commit between `3.7...main` (8 commits, v3.7.1 preview)

## TL;DR

**v3.7 is a meaningful upstream release with two user-visible features + one regression of v3.6's tool-path removal:**

1. **Library URL support extended** — GAMDL now natively downloads from the user's **personal Apple Music library** (songs, albums, playlists, music-videos; both `p.` / `l.` / `i.` ID prefixes). MeedyaDL already passes `/library/…` URLs through to GAMDL; the only adjustments needed are URL-parser whitelist updates and audit-diagnostic message refresh.
2. **`--ffmpeg-path` reinstated** — v3.6 removed all three tool-path CLI options (`--ffmpeg-path`, `--mp4box-path`, `--mp4decrypt-path`) when GAMDL switched to native muxing. v3.7 brings **`--ffmpeg-path` back** (N_m3u8DL-RE depends on FFmpeg). The other two stay removed. MeedyaDL's current `NativeMuxing` capability gate suppresses all three together; we need to **un-couple `ffmpeg_path` from the other two** via a new `FFmpegPath` capability that returns `false` ONLY on v3.6.x.
3. **DRM-free tracks** — GAMDL marks uploaded music videos (and potentially user-library uploads) as `drm_free = True` and skips the decrypt step. MeedyaDL's progress-stage / activity-log parsing is **not affected** — codec detection runs post-download from the file, not from GAMDL's log line.

**v3.7.1 preview (upstream `main`)** is a four-commit bug-fix cycle: graceful handling of missing m3u8 master URLs / missing webplayback in song stream info, music-video stream refactor, and an internal exception-keyword rename (`formats=` → `codec=`). All four are zero-MeedyaDL-surface-impact when v3.7.1 ships.

**Required MeedyaDL changes:**

- Add new `GamdlFeature::FFmpegPath` capability gate, ungate `--ffmpeg-path` from `--mp4box-path` / `--mp4decrypt-path` in `gamdl_options.rs` + `config_service.rs`.
- Bump `tool-versions.toml` GAMDL `maximum_tested_version` / `recommended_version` from `3.6` → `3.7`.
- Extend frontend URL-parser library handling to cover `i.*` IDs (in addition to `p.` / `l.`) for `/library/songs/` and `/library/music-videos/`.
- Update the URL-audit-diagnostic INFO log for library URLs to reflect that GAMDL v3.7+ supports the full expanded library URL set natively.
- Skip enrichment for `/library/` URLs (graceful — catalog API would 404 for personal-only library items; better to skip than to surface noisy 404s).
- Verification only — DRM-free tracks pass through unchanged; no new code paths.

**Not required:**

- v3.7.1 admission — wait for upstream release tag. Same zero-code-change shape as v3.3 / v3.5 / v3.5.1 / v3.5.2.
- `LibrarySongs` / `LibraryMusicVideos` capability gate — GAMDL accepts library URLs on v3.7 silently (it's URL routing, not a CLI flag).

---

## Upstream change inventory

22 commits between `3.6...3.7`, 17 files touched, +488 / -195 lines:

| Commit | Title | Category | MeedyaDL impact |
|---|---|---|---|
| `92b8220c` | Add ffmpeg path option to downloader | tool path | **REQUIRED CHANGE** — `FFmpegPath` capability gate |
| `bd59bb7c` | Add ffmpeg_path CLI option and pass to downloader | tool path | (paired with above) |
| `b5432d13` | Add library endpoints and client methods | library API | None — GAMDL-internal |
| `f8ec2367` | Add include param to library endpoints | library API | None — GAMDL-internal |
| `03fb4a25` | Add library song/video APIs and params | library API | None — GAMDL-internal |
| `6d8ecf65` | Support library tracks in get_webplayback | library API | None — GAMDL-internal |
| `a8bf884d` | Handle m3u8 and HttpFD downloads in ytdlp | downloader internals | None — MeedyaDL doesn't override ytdlp config |
| `8200ee0d` | Refactor AppleMusicBaseInterface metadata parsing | internal refactor | None |
| `622661a6` | Support songs/music-videos in library URL regex | URL regex | **REQUIRED CHANGE** — frontend URL parser + audit-diagnostic message |
| `1eba4321` | Handle DRM-free tracks in AppleMusic downloader | DRM-free | Verification only |
| `001a502a` | Support Apple Music library items | library API | None — GAMDL-internal |
| `c75249bc` | Support Apple Music library songs streaming | library API | None — GAMDL-internal |
| `76a7c792` | Use API response 'id' for media.media_id | media_id source | None — MeedyaDL doesn't construct media_id |
| `aa146939` | Add drm_free and is_library flags to types | types | None — MeedyaDL doesn't deserialise GAMDL types |
| `a7140cb8` | Use .get for playParams isLibrary checks | safety / dict access | None |
| `34357ad3` | Handle library music videos and fix logging id | library + logging | None — log line still matches `TRACK_INFO_V2_REGEX` |
| `cb367049` | Remove get_tags method from AppleMusicSongInterface | internal refactor | None |
| `4fc91bac` | Add get_m3u8_master_url helper and use it | internal refactor | None |
| `0519adf6` | Clarify supported URL types in README | docs | Mirror in our README + help |
| `4650391b` | Add FFmpeg requirement and --ffmpeg-path option to README | docs | Mirror in our README + help |
| `8f82697c` | Bump package version to 3.7 | version | **REQUIRED CHANGE** — bump support window ceiling |
| `73e0b4b4` | Mark uploaded Apple Music video as DRM-free | DRM-free | Verification only |

**v3.7.1 preview (upstream `main`, 8 commits ahead of `3.7`):**

| Commit | Title | MeedyaDL impact |
|---|---|---|
| `eb9caff8` | Await get_tags_from_asset_info call | None — async bug fix internal |
| `50f82b5d` | Refactor music video stream fetching | None — output unchanged |
| `141d9cd6` | Pass codec through music video stream selection | None — internal plumbing |
| `5a41dfbd` | Handle missing m3u8 master URL | **User-facing benefit** — fewer cryptic errors. MeedyaDL's network classifier picks up the better message automatically |
| `740cad2e` | Refactor song interface stream logic and imports | None — output unchanged |
| `7ac33228` | Handle missing webplayback in song stream info | **User-facing benefit** — `aac-web` codecs now degrade gracefully when webplayback is unavailable. No MeedyaDL code change |
| `ff3dcda5` | Bump version to 3.7.1 | Will trigger support window bump when released |
| `4f910c8e` | Use 'codec' key instead of 'formats' in error | None — internal exception keyword arg name; our error parser keys on the human substring `"format is not available"`, not the structured exception fields |

---

## Required MeedyaDL changes (detailed)

### Change 1 — New `GamdlFeature::FFmpegPath` capability gate

**Why:** v3.6 removed `--ffmpeg-path` / `--mp4box-path` / `--mp4decrypt-path`. v3.7 added back JUST `--ffmpeg-path` (N_m3u8DL-RE's dependency). The other two stay removed.

**Files to change:**

- `src-tauri/src/services/gamdl_capabilities.rs`:
  - New variant: `GamdlFeature::FFmpegPath`
  - `is_available_on()`: returns `true` for `< 3.6` OR `>= 3.7` (i.e., **only `false` on `3.6.x`**)
  - Add to `active_capabilities_summary()` array

- `src-tauri/src/models/gamdl_options.rs`:
  - `path_cli_args()`: ungate `--ffmpeg-path` emission. Use `supports(GamdlFeature::FFmpegPath)` instead of being lumped under `!supports(NativeMuxing)`
  - `--mp4box-path` and `--mp4decrypt-path` continue to use `!supports(NativeMuxing)` (still removed on 3.6+)

- `src-tauri/src/services/config_service.rs`:
  - `ini_tool_path_section()`: same split as above. `ffmpeg_path` INI key uses `FFmpegPath` gate; `mp4decrypt_path` + `mp4box_path` use `!NativeMuxing`

- `src-tauri/src/commands/dependencies.rs`:
  - `GamdlCapabilities` DTO: add `ffmpeg_path: bool` field. Frontend can use it to decide whether to surface the FFmpeg path setting

- Unit test for the gate's three-version classification (≤3.5 → true, 3.6.x → false, ≥3.7 → true)

**Behaviour confirmation:**

```
GAMDL version    | --ffmpeg-path | --mp4decrypt-path | --mp4box-path
-----------------|---------------|-------------------|---------------
2.9.x – 3.5.x    | emitted       | emitted           | emitted
3.6.x            | suppressed    | suppressed        | suppressed
3.7+             | emitted       | suppressed        | suppressed
```

### Change 2 — Bump GAMDL support window to v3.7

**Files to change:**

- `src-tauri/tool-versions.toml`:
  - `[gamdl]` → `maximum_tested_version`: `3.6` → `3.7`
  - `[gamdl]` → `recommended_version`: `3.6` → `3.7`
  - Append v3.7 validation note to the existing block comment (continues the audit-cadence pattern from v3.5.2 / v3.6)

### Change 3 — Frontend URL parser library handling

**Why:** GAMDL v3.7's library URL regex was extended:

- Was: `/library/(?P<library_type>playlist|albums)/(?P<library_id>p\.[a-zA-Z0-9]+|l\.[a-zA-Z0-9]+)`
- Now: `/library/(?P<library_type>playlist|albums|songs|music-videos)/(?P<library_id>[pli]\.[a-zA-Z0-9]+)`

MeedyaDL's frontend `src/lib/url-parser.ts` already detects `/library/` URLs as `'library'` content type and passes through to GAMDL. **No code change required** to recognise the new URL shapes — they all already match the `/library/` substring check. But:

**Files to update for clarity:**

- `src/lib/url-parser.ts`: comment block documenting which library URL shapes are recognised (currently only mentions `/library/albums/l.…`). Update the comment to reflect songs / music-videos / `i.` prefixed IDs.
- `src-tauri/src/commands/gamdl.rs`: the library-URL audit-diagnostic INFO log (#546) currently reads "no MeedyaDL safety net applies". On GAMDL v3.7+ it remains accurate (we still don't enrich library items via catalog APIs), but should mention that GAMDL natively handles the full library URL set on v3.7+.

### Change 4 — Library item enrichment skip

**Why:** Today MeedyaDL runs the full enrichment pipeline (iTunes Lookup + Apple Music Catalog API) on every downloaded item including library items. Catalog APIs return 404 for personal-only library items (e.g., user's own MP3 uploads). The existing `fetch_album_metadata_with_fallback()` handles 404 gracefully but emits noise.

**Cleanup approach:**

- Detect `/library/` in the download URL early in the completion task
- Skip Step 0 (iTunes Lookup) and Step 1 (Apple Music Catalog) for these items
- Skip Step 1b (syllable-lyrics API — catalog-only)
- Skip Step 3 (animated artwork API — catalog-only)
- Keep Step 4 (AcoustID — fingerprint-based, works on any audio file)
- Keep Step 5 (ReplayGain — local DSP)
- Skip Step 6 (music video companions — catalog-only)
- Manifest still written, with a `library: true` flag

Activity log: emit a single `[System]` line ("Library item: skipping catalog metadata enrichment") instead of running each step and silently 404ing.

**Files to update:**

- `src-tauri/src/services/download_queue.rs` — gate the enrichment pipeline behind a `!is_library_url(&item.url)` check, or pass an `EnrichmentMode { skip_catalog_apis: true }` parameter through.
- `src-tauri/src/models/manifest.rs` — add optional `is_library: bool` field to `ManifestSource` (`#[serde(default)]` for backwards-compat).
- Help doc update covering the limitation.

### Change 5 — DRM-free track handling (verification only)

**Why:** GAMDL v3.7 marks tracks as `drm_free: bool = False` (default) or `True` (uploaded MV / certain library uploads). The decrypt step is skipped server-side. MeedyaDL's stdout/stderr parsing doesn't strictly require "Decrypting…" lines to appear.

**Verification:**

- Manually download an uploaded music video URL post-v3.7
- Confirm:
  - GAMDL exits 0 successfully
  - Activity log shows the track was downloaded
  - Codec detection (`detect_audio_info()`) returns the correct codec from the file
  - Manifest is written with valid track entries

**No code change unless verification reveals a regression.**

---

## What MeedyaDL does NOT need to do

| GAMDL change | Why no MeedyaDL change |
|---|---|
| `gamdl.api.apple_music` — new library API endpoints | GAMDL-internal HTTP client. We don't call these endpoints; we use the catalog APIs for our own enrichment (which 404 cleanly on library items) |
| `gamdl.api.constants` — new library URI constants | Same — GAMDL-internal |
| `gamdl.downloader.base` — HttpFD path for DRM-free | We capture stdout/stderr; the new download path is invisible to us |
| `gamdl.interface.types` — `StreamInfo.drm_free`, `AppleMusicMedia.is_library` | Internal dataclasses; we don't deserialise them |
| `media_id` source change (API `id` field) | We don't track media_id ourselves; we observe GAMDL via filename / progress output |
| `get_tags` method removed from `AppleMusicSongInterface` | Internal API change |
| New `get_m3u8_master_url` helper | Internal refactor |
| `cleanup_unknown_params()` behaviour | Already handles legacy v2.x INI keys gracefully — adds the new tool path option transparently |
| v3.7.1 `formats` → `codec` keyword arg | Our error parser greps `"format is not available"` substring; the structured exception kwarg name is internal |
| v3.7.1 missing-m3u8 / missing-webplayback graceful degradation | These are upstream wins — fewer cryptic errors reach our parser. Existing categorisation still works |

---

## Validation plan

Once changes 1–4 are implemented, run the following manual smoke tests:

1. **GAMDL ≤3.5.x users** (validate no regression on the historical path):
   - Confirm `--ffmpeg-path` still emitted
   - Confirm `--mp4decrypt-path` still emitted
   - Confirm `--mp4box-path` still emitted
   - Confirm a normal album download completes
2. **GAMDL 3.6.x users** (validate the v3.6 EPIC path still works):
   - Confirm none of the three tool path options are emitted
   - Confirm `--wrapper-url` emitted instead of v1 wrapper triple
   - Confirm `aac-web` codec rename still emitted
3. **GAMDL 3.7+ users** (the new path):
   - Confirm `--ffmpeg-path` emitted
   - Confirm `--mp4decrypt-path` and `--mp4box-path` NOT emitted
   - Confirm a library URL download (e.g., `/library/songs/i.XXX`) routes correctly to GAMDL and succeeds
   - Confirm enrichment is skipped for library items (single "skipping catalog metadata" line in activity log)
   - Confirm `[Untested]` badge in Updates page goes away once the support window bump lands

---

## v3.7.1 readiness

When 3.7.1 ships:

- Same audit pattern as v3.5.2: bump `maximum_tested_version` / `recommended_version` to `3.7.1` in a one-line PR
- No code changes expected (all four 3.7.1 changes are internal Python refactors or graceful-degradation wins that benefit us automatically)
- Quick file the v3.7.1 audit doc as a thin shell linking back to this v3.7 audit

---

## Reference

- v3.6 EPIC audit: `.github/audits/gamdl-v3.6-audit.md` (if it exists — otherwise PR #855 is the canonical reference)
- v3.5.2 audit (template for this doc): `.github/audits/gamdl-v3.5.2-audit.md`
- GAMDL v3.6 → v3.7 diff: https://github.com/glomatico/gamdl/compare/3.6...3.7
- GAMDL v3.7 → main diff (v3.7.1 preview): https://github.com/glomatico/gamdl/compare/3.7...main
- Upstream release notes: https://github.com/glomatico/gamdl/releases/tag/3.7
