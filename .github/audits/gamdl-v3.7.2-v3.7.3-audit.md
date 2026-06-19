# GAMDL v3.7.2 + v3.7.3 Compatibility Audit

**Date**: 2026-05-28
**GAMDL releases audited**: 3.7.2 + 3.7.3 (shipped same day)
**Diff range**: `3.7.1..3.7.3` (3 functional commits + version metadata, ~4 lines functional change across both releases)
**Predecessor audit**: [`gamdl-v3.5.2-audit.md`](./gamdl-v3.5.2-audit.md) (3.6 / 3.7 / 3.7.1 were rolled into a single bridging admission with no surface inspection — pure refactor / bug-fix releases)
**Tracking issue**: #898

## TL;DR

Pair of bug-fix releases shipped the same day with only ~4 functional lines changed across both. **One MeedyaDL code change required** — a new `media_not_streamable` classifier bucket in [`utils/process.rs`](../../src-tauri/src/utils/process.rs) to surface the now-reachable `GamdlInterfaceMediaNotStreamableError` message cleanly. CLI / INI / wrapper-protocol surface unchanged.

Highlights:

- **3.7.2** uncensors music-video title / album metadata. MeedyaDL writes neither sort-order atom Apple now routes the censored variants into, so no collision.
- **3.7.2 + 3.7.3** harden `interface/song.py` and `interface/music_video.py` against a bare `KeyError: 'playParams'` traceback for unstreamable / library-only items by introducing defensive `.get('playParams', {})` access. The streamability gate now reaches the `is_media_streamable` check reliably and surfaces `GamdlInterfaceMediaNotStreamableError("Media is not streamable: <id>")` to downstream callers — the message itself predates 3.5.2; what changed is that users now see it instead of the masked KeyError.

## Methodology

Identical to the v3.4 / v3.5 / v3.5.1 / v3.5.2 audits:

1. Cloned `glomatico/gamdl`, materialised both tags, ran `git diff --stat 3.7.1..3.7.3` and `git diff 3.7.1..3.7.3`.
2. Cross-referenced each hunk against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates).
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`).
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task).
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature flags).
   - `src-tauri/src/utils/process.rs` (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `classify_error`, `parse_gamdl_output`).
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window).

## v3.7.2 — `3.7.1..3.7.2` change set

| Commit | Subject |
|---|---|
| (interface/music_video.py) | Use uncensored title / album for music-video tags |
| (interface/music_video.py) | Defensive `.get('playParams')` access for unstreamable MVs |
| (version bump) | Bump version to 3.7.2 |

### Hunk 1 — Uncensored MV title + album

`interface/music_video.py::get_tags` swaps the title (`@nam`) and album (`@alb`) tag sources from `trackCensoredName` / `collectionCensoredName` to the uncensored `trackName` / `collectionName`. The censored variants are routed into the standard sort-order atoms `sonm` (title sort) and `soal` (album sort).

**MeedyaDL impact**: zero collision. MeedyaDL writes neither `sonm` nor `soal` — see `metadata_tag_service.rs` enrichment + `tags.toml` audit. The only user-visible effect is that MV filename templates containing `{title}` or `{album}` now produce uncensored filenames after upgrade. Songs are unaffected (the change is scoped to the music-video interface).

### Hunk 2 — `interface/music_video.py::get_media` defensive playParams access

Replaces `media['playParams']['isLibrary']` with `media.get('playParams', {}).get('isLibrary')`. Prevents `KeyError: 'playParams'` tracebacks when Apple's response omits the field for unstreamable / library-only / region-locked content. The control-flow effect is that affected music videos now reach the existing `is_media_streamable` check and surface `GamdlInterfaceMediaNotStreamableError("Media is not streamable: <id>")` reliably.

**MeedyaDL impact**: the error class predates 3.5.2 but its message was previously masked by the `KeyError` raised before reaching the streamability gate. Now the message lands in stderr. See §"Required MeedyaDL change" below.

## v3.7.3 — `3.7.2..3.7.3` change set

| Commit | Subject |
|---|---|
| (interface/song.py) | Defensive `.get('playParams')` access for unstreamable songs |
| (version bump) | Bump version to 3.7.3 |

### Hunk 1 — `interface/song.py::get_media` mirrors v3.7.2's MV fix

The song interface now applies the same defensive `.get('playParams', {}).get('isLibrary')` pattern. Same control-flow effect — affected songs reach the streamability gate and surface `GamdlInterfaceMediaNotStreamableError("Media is not streamable: <id>")`.

**MeedyaDL impact**: same as v3.7.2's MV path — the now-reachable error message lands in stderr.

## Required MeedyaDL change

Add an `is_media_not_streamable_error()` helper + dedicated `media_not_streamable` classifier bucket in [`src-tauri/src/utils/process.rs`](../../src-tauri/src/utils/process.rs), ordered **before** the generic `not_found` substring fallback in `classify_error()`. Rationale: the actionable "removed / region-locked / library-only" guidance wins over the broader "content not found" message that the generic bucket would otherwise emit.

Match pattern: `"Media is not streamable"` (case-sensitive substring on the stripped error line).

6 unit tests in `process::tests`:

1. Bare `Media is not streamable: 12345` classifies as `media_not_streamable`.
2. Same with a track number prefix from the parser classifies as `media_not_streamable`.
3. Generic 404-not-found message still classifies as `not_found`.
4. Generic `Resource Not Found` still classifies as `not_found`.
5. Empty / unrelated error message returns `unknown`.
6. Ordering: an error line containing both `"Media is not streamable"` and `"not found"` classifies as `media_not_streamable` (the more specific bucket wins).

## CLI / INI / wrapper / regex surface

| Surface | v3.7.2 + v3.7.3 status |
|---|---|
| `to_cli_args` (CLI flag emission) | unchanged |
| `settings_to_ini` (INI emission) | unchanged |
| `wrapper_url` / `wrapper_m3u8_ip` / `wrapper_decrypt_ip` | unchanged |
| `TRACK_INFO_V2_REGEX` | unchanged |
| `ERROR_PREFIX_REGEX` | unchanged |
| `PYTHON_EXCEPTION_REGEX` | unchanged |
| `GamdlFeature` gates | unchanged |
| `tool-versions.toml` support window | bumped 3.5.2 → 3.7.4 in #947 (covers admission of 3.6 / 3.7 / 3.7.1 / 3.7.2 / 3.7.3 / 3.7.4 — this audit + the v3.7.4 sibling audit) |

## Verdict — admission with one MeedyaDL code change

3.7.2 + 3.7.3 admitted to the support window via PR #947's `tool-versions.toml` ceiling bump (3.5.2 → 3.7.4). The required `media_not_streamable` classifier addition + 6 unit tests landed alongside the admission as part of #898. Same release-class (defensive Apple-API access patterns) as v3.5.1 (MV m3u8 403 fix) and v3.5 (iTunes lookup follow-redirects fix).
