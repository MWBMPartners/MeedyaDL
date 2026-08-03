# GAMDL v3.8.5 Compatibility Audit — DECISION: ADMITTED (ceiling → 3.8.5)

**Date**: 2026-08-03
**GAMDL release audited**: 3.8.5 (released 2026-08-03)
**Diff range**: `3.8.4..3.8.5` (2 commits, 4 files)
**Predecessor audit**: [`gamdl-v3.8.3-v3.8.4-audit.md`](./gamdl-v3.8.3-v3.8.4-audit.md)
**Tracking issue**: #1074

## TL;DR

**Admit 3.8.5; bump the ceiling 3.8.4 → 3.8.5.** This is a **zero-code-change
ceiling bump** — the same shape as v3.3 / v3.5 / v3.5.1 / v3.5.2 / v3.7.4 /
v3.8.1 / v3.8.3+v3.8.4. The entire delta is 2 commits touching 4 files, and the
only functional change is a refactor of **private, `_`-prefixed DRM
key-extraction methods inside `gamdl/interface/song.py`**: the base64
audio-session-key-metadata fast-path is dropped, and DRM keys are now **always**
read from the media m3u8's `#EXT-X-KEY` tags via the pre-existing
`_get_drm_uri_from_m3u8_keys`. No CLI / INI / exception-class / log / output /
wrapper-protocol / `_ammuxer` change. The wheel gate re-verified live: 3.8.5
ships the identical 5-platform `cp310-abi3` matrix (no ARMv7). Only edits
needed: `tool-versions.toml` + docs.

## Verified facts

- **Compare `3.8.4...3.8.5`**: 2 commits, 4 files. The only file under `gamdl/`
  besides `gamdl/__init__.py` (version string) is `gamdl/interface/song.py`;
  the remaining two files are the release-bump pair (`pyproject.toml`,
  `uv.lock`). **No `cli/` or CLI/INI argument-definition `.py` changed.**
- **`gamdl/interface/song.py` delta (the sole functional change, `20e1b76d`):**
  removes the `m3u8_master_data` parameter from `_get_stream_info_from_playlist`
  (and its two callers `_get_stream_info_enhanced` /
  `_get_stream_info_nonenhanced`), deletes the base64
  audio-session-key-metadata fast-path — the private helpers
  `_get_m3u8_metadata`, `_get_audio_session_key_metadata`,
  `_get_asset_metadata`, and `_get_drm_uri_from_session_key` are removed, along
  with the now-unused `base64` and `json` imports. DRM key extraction now
  always goes through the **pre-existing** `_get_drm_uri_from_m3u8_keys`,
  reading `#EXT-X-KEY` tags from the media m3u8. All touched methods are
  `_`-prefixed internals of GAMDL's HLS pipeline. Net: −74 / +15 in song.py.
- **`gamdl/api/wrapper.py` is untouched** ⇒ `TARGET_WRAPPER_API_VERSION` stays
  `"0.0.2"`. MeedyaDL's `/me` preflight expected-version literal
  (`health_check_service.rs`) and `wrapper_version_mismatch` guidance
  (`process.rs`) remain correct.
- **`ammuxer/` is untouched** — the 3.8.4 song-ending corruption fix
  (`e4887d34`) is carried forward unchanged; 3.8.5 does not re-touch the
  decrypt/mux pipeline.
- **Wheel matrix (PyPI JSON API, live, 2026-08-03):** 3.8.5 ships — identically
  to 3.8.2/3.8.3/3.8.4 — `cp310-abi3` wheels for macOS universal2
  (x86_64+arm64), Linux x86_64 (`manylinux_2_34`), Linux aarch64
  (`manylinux_2_34`), Windows amd64, Windows arm64, plus an sdist. **No Linux
  ARMv7 (`armv7l`/`armhf`) wheel.** `requires-python` = `>=3.10`.

## Change-set (2 commits)

| SHA | Message | Files | MeedyaDL-facing? |
| --- | --- | --- | --- |
| `20e1b76d` | Fix non-web song key extraction (rework song DRM key extraction to read keys from the media m3u8) | `gamdl/interface/song.py` | **No code action** — private `_`-prefixed methods inside GAMDL's HLS key selection; MeedyaDL never parses m3u8s or session-key metadata. Sole functional change in the range. |
| `478c3f26` | Bump version to 3.8.5 | `gamdl/__init__.py`, `pyproject.toml`, `uv.lock` | No — version bump |

## Findings

### 3.8.5-A — DRM key extraction moves entirely to the m3u8 `#EXT-X-KEY` path

Before 3.8.5, `song.py` tried a base64 audio-session-key-metadata fast-path
first and fell back to reading `#EXT-X-KEY` tags from the media m3u8. 3.8.5
drops the fast-path: `_get_drm_uri_from_m3u8_keys` (which **already existed**
and was already exercised as the fallback / else-branch since 3.8.1) is now the
only key source. **MeedyaDL surface impact: none** — MeedyaDL invokes GAMDL as
a subprocess and observes files + stdout/stderr; it never inspects m3u8 URLs,
session-key metadata, or DRM keys. The repository's only references to GAMDL's
HLS internals are comments. No flag, INI key, or output line changes. This is
the fix behind the release note "Fixed an issue when extracting content key
from some songs on non-web codecs" — the deleted fast-path's session-key
metadata was wrong/absent for some non-web-codec songs.

### 3.8.5-B — simpler, stricter error surface (positive side-effect)

Deleting the fast-path also deletes the exception shapes it could raise — a
`KeyError` on `asset_metadata[variant_id]["AUDIO-SESSION-KEY-IDS"]`, plus any
base64/JSON decode error, all came from a code path that no longer exists. All
key-extraction failures now funnel through the single pre-existing m3u8 path, so
error output is more uniform across tracks. MeedyaDL's classifiers are
unaffected either way: `PYTHON_EXCEPTION_REGEX` keys on the exception
class-name prefix generically, and no GAMDL exception **class** was added,
removed, or renamed in this range. No `classify_error` / `is_*_error` change
needed. The `'NoneType' … 'stream_info'` codec-not-available translation in
`process.rs` remains valid — `StreamInfoAv` / `stream_info.codec` /
`stream_info.widevine_pssh` all survive 3.8.5.

## Per-platform install behaviour (unchanged from 3.8.2/3.8.3/3.8.4)

| Platform | 3.8.5 wheel | Installs |
| --- | --- | --- |
| macOS Apple Silicon | `cp310-abi3` universal2 | 3.8.5 |
| Windows x64 | `cp310-abi3-win_amd64` | 3.8.5 |
| Windows ARM64 | `cp310-abi3-win_arm64` | 3.8.5 |
| Linux x64 | `cp310-abi3-manylinux_2_34_x86_64` | 3.8.5 |
| Linux ARM64 | `cp310-abi3-manylinux_2_34_aarch64` | 3.8.5 |
| Linux ARMv7 | **none** | **3.8.1** (range fallback via `--only-binary`) |

`gamdl>=3.0,<=3.8.5` + `--only-binary=gamdl` resolves 3.8.5 on 5/6 targets and
falls back to 3.8.1 on ARMv7 (its universal `py3-none-any` wheel is still in
range). The `no_compatible_wheel` UI guard stays platform-aware (flags only
ARMv7), computed live from the 3.8.5 file list at check time.

## Nothing else needs changing (swept)

Checked each coupling surface against the actual Rust source; all inherit 3.8.5
via existing version math:

1. **`models/gamdl_options.rs`** (`to_cli_args`, `audio_cli_args`) — no flag
   added/removed/renamed upstream (3.8.5 has zero `cli/` change).
2. **`services/config_service.rs`** (`ini_*`) — no INI key change.
3. **`services/download_queue.rs`** (`merge_options`, companions, gap-fill,
   stdout/stderr readers) — no behavioural or output-shape change;
   `WrapperDecryptHostPort` (`>= 3.8.2`) still drives the wrapper-v2 TCP
   decrypt emission + preflight; `NoExceptionsFlag` (`>= 3.8`) unchanged.
4. **`services/gamdl_capabilities.rs`** — no `GamdlFeature` add/re-threshold;
   all 3.8.2-keyed gates go true on 3.8.5 by version math. (Added `"3.8.5"` to
   the `WrapperDecryptHostPort` test true-list purely for documentation.)
5. **`services/gamdl_service.rs`** — `--only-binary=gamdl` install spec
   unchanged (both call sites); the range ceiling comes from
   `tool-versions.toml`, so `pip_version_spec()` now renders
   `gamdl>=3.0,<=3.8.5`.
6. **`utils/process.rs`** — no exception class, log-line shape, error prefix,
   or classifier substring change (`ERROR_PREFIX_REGEX`,
   `PYTHON_EXCEPTION_REGEX`, `TRACK_INFO_V2_REGEX`, `classify_error`,
   `wrapper_version_mismatch`, `is_media_not_streamable_error`,
   `is_storefront_mismatch_error` all still accurate).
7. **`services/update_checker.rs`** — `is_wheel_compatible` matches
   `cpython_tag` OR `abi3` + platform; 3.8.5 filenames are shape-identical to
   the 3.8.2 fixtures. `no_compatible_wheel` stays ARMv7-only.
8. **`services/health_check_service.rs`** — `/me` expected version `"0.0.2"`
   still correct (`wrapper.py` untouched); all three wrapper preflights
   unchanged.
9. **`services/apple_music_api.rs`** — MeedyaDL's own token chain / catalog
   API / syllable-lyrics paths are fully independent of GAMDL's key-extraction
   internals; nothing shared with `song.py`.
10. **Settings schema / migrations / i18n / help UI** — no new setting, no
    migration, no `HelpViewer.tsx` literal (verified: zero `3.8.4` literals in
    the component; the wrapper-lockstep table lives in `help/wrapper.md` only).

Grep evidence: `_get_audio_session_key_metadata` / `_get_asset_metadata` /
`_get_drm_uri_from_session_key` / `_get_m3u8_metadata` / `AUDIO-SESSION-KEY` /
`session_key` / `stable_variant_id` — **zero matches** across the entire
MeedyaDL repository. MeedyaDL references none of the deleted GAMDL symbols.

## Actions taken

- `src-tauri/tool-versions.toml` — `maximum_tested_version` +
  `recommended_version` → `3.8.5`; ARMv7 comment refreshed; 3.8.5 audit-trail
  block appended.
- `README.md` support matrix — GAMDL range → 3.0–3.8.5, recommended 3.8.5;
  "supported" bullet retargeted at this audit; wrapper blockquote → 3.8.5.
- `help/wrapper.md` — recommended latest → 3.8.5; wrapper-v2 lockstep row
  extended to 3.8.5 (3.8.4 corruption-fix note kept as history).
- `scripts/smoke-tests/README.md` + `gamdl_live_smoke.py` — harness `--install`
  ceiling (`DEFAULT_GAMDL_SPEC_CEILING`) and forward-looking references
  retargeted at 3.8.5 (historical 3.8.4-fix references preserved).
- `.claude/CLAUDE.md` — ceiling → 3.8.5; v3.8.5 capability note appended.
- `.claude/memory/project_gamdl_release_cadence.md` — dated 3.8.5 entry.
- `.github/HANDOFF.md` — 2026-08-03 LATEST section.
- `services/gamdl_capabilities.rs` — `WrapperDecryptHostPort` test true-list
  gains `"3.8.5"` (documentation value).

## Test impact

- **`services::gamdl_capabilities`** — one test literal added (`"3.8.5"` in the
  `WrapperDecryptHostPort` true-list); everything else reads `support_window()`
  dynamically (`classify_*`, `support_window_*`,
  `pip_version_spec_bounds_the_range`, `is_above_tested_ceiling_*`,
  `should_offer_upgrade_*`).
- **`services::update_checker`** — no edits. Wheel fixtures are representative
  shape data, not ceiling-keyed.
- **`utils::process`** — no edits.
- Expected: `cargo test --lib` green with only the `tool-versions.toml` string
  change (the `support_window_has_recommended_inside_range` invariant holds:
  3.0 ≤ 3.8.5 ≤ 3.8.5; `pip_version_spec` now renders `gamdl>=3.0,<=3.8.5`).

## Pre-release gate (carried forward, retargeted at 3.8.5)

Before this ceiling reaches **stable**, on each shipping platform
(`scripts/smoke-tests/gamdl_live_smoke.py`):

1. `import gamdl._ammuxer` + a real song download decrypts + muxes on the
   bundled cp312.
2. Real wrapper-v2 0.0.2 round-trip — local + remote/LAN (decrypt host/port).
3. **Song-ending integrity** — a wrapper-decrypted ALAC song's final seconds
   play cleanly (validates that `e4887d34` is still carried; 3.8.2/3.8.3
   remain known-bad).
4. **NEW for 3.8.5 — wrapper-less non-web-codec song leg**: a cookie-only
   `aac` song download must succeed end-to-end. On 3.8.5 this leg exercises
   the **rewritten m3u8 `#EXT-X-KEY` key-extraction path** (the deleted
   session-key fast-path can no longer mask a broken m3u8 path). The harness's
   existing non-wrapper `aac` leg covers this — run it on 3.8.5 specifically.
5. A music-video download (local-key decrypt via `_ammuxer`).
6. `pip install --only-binary=gamdl 'gamdl==3.8.5'` resolves on the 5 wheel
   platforms; the range falls back to 3.8.1 on ARMv7.

## `for consideration` follow-ups (non-blocking)

1. **Carried forward:** `SongCodec::is_wrapper_dependent()` + `(Experimental)`
   codec labels are conceptually stale since 3.8's assets API.
2. **Carried forward:** dead `fetch_extra_tags` plumbing removal.
3. **#1013 stands** ("Linux ARMv7 wheel only") — 3.8.5 again ships no `armv7l`
   wheel; an upstream feature request remains the only path to closing it.
4. **Carried forward:** an `expected_wrapper_v2_version(gamdl_version) -> &str`
   helper next to the gates would make a future wrapper-v2 0.0.3 a one-line
   change (the `"0.0.2"` literal still lives in `health_check_service.rs`).
