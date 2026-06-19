# GAMDL v3.7.4 Compatibility Audit

**Date**: 2026-06-12
**GAMDL release audited**: 3.7.4 (released 2026-06-12)
**Diff range**: `3.7.3..3.7.4` (5 commits: 4 functional + 1 version bump)
**Predecessor audit**: [`gamdl-v3.7.2-v3.7.3-audit.md`](./gamdl-v3.7.2-v3.7.3-audit.md)
**Tracking issues**: #925 (audit), #910 (upstream watch — also closed)

## TL;DR

Pure upstream reliability patch. **No MeedyaDL code change required.** `tool-versions.toml` ceiling bump 3.5.2 → 3.7.4 shipped in PR #947 (merged 2026-06-18). Same zero-code-change shape as v3.3 (playlist fix), v3.5 (iTunes lookup fix), v3.5.1 (music-video 403 fix), and v3.5.2 (m3u8 host migration).

The four functional commits each tighten a different upstream surface — HLS variant rewrite, AMP token regex, cover-fetch HTTP timing, and API exception content typing. None reach the MeedyaDL integration boundary.

## Methodology

Identical to the v3.4 / v3.5 / v3.5.1 / v3.5.2 / v3.7.2-v3.7.3 audits:

1. Cloned `glomatico/gamdl`, materialised both tags, ran `git diff --stat 3.7.3..3.7.4` and `git diff 3.7.3..3.7.4`.
2. Cross-referenced each hunk against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates).
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`).
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task).
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature flags).
   - `src-tauri/src/services/apple_music_api.rs` (`TokenSource` 3-tier chain — independent of GAMDL's own token-extraction).
   - `src-tauri/src/utils/process.rs` (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `classify_error`, `parse_gamdl_output`, `is_storefront_mismatch_error`).
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window).

## v3.7.4 — `3.7.3..3.7.4` change set

| Commit | Subject |
|---|---|
| 1 | `interface/song.py::_switch_m3u8_master_url_to_default` regex rewrite for HLS variants |
| 2 | `api/apple_music.py` AMP web-player token-extraction regex update |
| 3 | `interface/base.py::get_cover_bytes` 30s timeout + `follow_redirects=True` |
| 4 | `api/exceptions.py::GamdlApiResponseError.content` typed `Any \| None` + JSON serialisation fallback |
| 5 | Version bump to 3.7.4 |

### Hunk 1 — HLS master URL `_default.m3u8` rewrite

`interface/song.py::_switch_m3u8_master_url_to_default` adds a regex that rewrites HLS variant master URLs to the canonical `_default.m3u8` form across both `_get_m3u8_from_playback` and `_get_m3u8_master_url_from_metadata`.

**MeedyaDL impact**: zero. MeedyaDL never inspects m3u8 URLs — they're consumed inside GAMDL's HLS pipeline. The only repository references to `_default.m3u8` are in comments describing GAMDL's debug-log key, not in any code path that processes the URL.

### Hunk 2 — AMP web-player token-extraction regex update

`api/apple_music.py::get_token` (GAMDL's own developer-token extraction) updated for two changes Apple shipped to the Music web client:

1. The home-page bundle was renamed `index-legacy-*.js` → `index-*.js`; the regex pattern is updated accordingly.
2. JWT capture tightened to require structurally valid three-segment tokens (`eyJ*.eyJ*.*`) inside quotes, rejecting incidental matches that would later 401 against amp-api.

**MeedyaDL impact**: zero. This fixes GAMDL's own `get_token()` path. MeedyaDL's `apple_music_api.rs::TokenSource` three-tier chain (user MusicKit JWT > embedded > web-player keychain) is independent of GAMDL's internal token extraction.

### Hunk 3 — Cover-fetch HTTP hardening

`interface/base.py::get_cover_bytes` adds an explicit 30s timeout + `follow_redirects=True` on the `httpx` client used to download cover art.

**MeedyaDL impact**: zero. MeedyaDL observes the resulting cover-image *files* on disk (jpg/png landed by GAMDL), not the HTTP path. MeedyaDL's own static cover-art fallback chain (#756) is unaffected.

### Hunk 4 — `GamdlApiResponseError.content` typing

`api/exceptions.py::GamdlApiResponseError.content` typed `Any | None` (was `str | None`). Non-string content is JSON-serialised via `json.dumps()` with `TypeError` fallback to `str()`.

**MeedyaDL impact**: zero. Two regex / substring checks consume the resulting error text:

1. `process::is_storefront_mismatch_error` keys on the `"Resource Not Found"` literal in the traceback — verified preserved by the new serialisation path (the literal appears in the message body, not in the structured `content` field).
2. `PYTHON_EXCEPTION_REGEX` keys on the class-name prefix in the traceback — also preserved (the class name appears at the start of the traceback line, independent of how `content` is serialised).

## CLI / INI / wrapper / regex surface

| Surface | v3.7.4 status |
|---|---|
| `to_cli_args` (CLI flag emission) | unchanged |
| `settings_to_ini` (INI emission) | unchanged |
| `wrapper_url` / `wrapper_m3u8_ip` / `wrapper_decrypt_ip` | unchanged |
| `TRACK_INFO_V2_REGEX` | unchanged |
| `ERROR_PREFIX_REGEX` | unchanged |
| `PYTHON_EXCEPTION_REGEX` | unchanged |
| `is_storefront_mismatch_error` substring | preserved |
| `GamdlFeature` gates | unchanged |

## Verdict — zero-code-change admission

3.7.4 admitted to the support window via PR #947's `tool-versions.toml` bump:

- `maximum_tested_version`: `"3.5.2"` → `"3.7.4"`
- `recommended_version`: `"3.5.2"` → `"3.7.4"`

CLAUDE.md GAMDL section already carries the full per-commit admission paragraph. Same release-class as v3.3 / v3.5 / v3.5.1 / v3.5.2 / v3.7.1 — pure upstream reliability bug-fix with no MeedyaDL integration surface impact.

`tracking issue #925` (this audit doc) and `#910` (upstream watch — body declared its own close-trigger as "when upstream tags 3.7.4 and MeedyaDL ships the admission PR") are both closed by the housekeeping wave that lands this doc.
