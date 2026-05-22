# GAMDL v3.5.1 Compatibility Audit

**Date**: 2026-05-08
**GAMDL release audited**: 3.5.1 (released 2026-05-07)
**Diff range**: `3.5..3.5.1` (1 commit, 1 file)
**Predecessor audit**: [`gamdl-v3.4-v3.5-audit.md`](./gamdl-v3.4-v3.5-audit.md)
**Tracking issue**: #711

## TL;DR

Single-commit upstream bug fix targeting music-video m3u8 URL handling (HTTP 403). **No MeedyaDL code change is required.** Bump `maximum_tested_version` and `recommended_version` in [`tool-versions.toml`](../../src-tauri/tool-versions.toml) from `"3.5"` → `"3.5.1"`. Same shape as v3.3 (playlist fix) and v3.5 (iTunes lookup fix) — zero-code-change admission to the support window.

## Methodology

Identical to the v3.4/v3.5 audit:

1. Inspected the upstream diff via `gh api repos/glomatico/gamdl/compare/3.5...3.5.1`.
2. Cross-referenced the change against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates)
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`)
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task)
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature flags)
   - `src-tauri/src/utils/process.rs` (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `classify_error`, `parse_gamdl_output`)
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window)

## v3.5.1 — `3.5..3.5.1` change set

One commit:

| Commit | Subject |
| --- | --- |
| `34a397e` | Fixed an issue with music video m3u8 URL returning error 403. |

### Finding 3.5.1-A — Music-video m3u8 URL HTTP 403 fix

The release notes describe a single fix: GAMDL's music-video downloader now successfully resolves the HLS master playlist URL for music-video assets where the previous version returned an `error 403` from the upstream CDN. The change is contained inside GAMDL's URL-resolution layer and does not surface as any:

- New / removed / renamed CLI flag.
- New / removed / renamed INI key.
- Output-line format change (no track-info / error-line regex impact).
- Exception class rename (`extract_python_exception` regex unchanged).
- Logging stream change (still stdout per v3.4's `CustomOutputWriter`).
- Subprocess-failure message format change (still the v3.4 enriched format).
- Database schema or `--database-path` semantics shift.

User-visible benefit: standalone music-video URLs and music-video companions (the `music_video_companion` flow plus `enrichment.fetch_music_video_relations` in `apple_music_api.rs`) that previously hard-failed at `error 403` will succeed once the user upgrades. The fix is silent — no opt-in required, no setting change, no UI toggle.

### MeedyaDL surface impact: none

Verified by inspection that:

- **No `GamdlFeature` gate is required.** The capability cache (`gamdl_capabilities::supports`) only gates flag emission, and no flag was added or removed.
- **No `to_cli_args` / `audio_cli_args` / `video_cli_args` change.** The `--song-codec-priority` / `--video-quality` / `--wrapper-m3u8-ip` emission paths are unaffected.
- **No INI emission change.** `ini_audio_section`, `ini_metadata_section`, `ini_template_section` — all untouched.
- **No output parser change.** `TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `PYTHON_EXCEPTION_REGEX`, `is_python_traceback_noise` — all unaffected.
- **No support-window test threshold change.** The existing `gamdl_capabilities::tests` suite already classifies any 3.x-series version `<= maximum_tested_version` as `Supported`, so bumping the ceiling automatically covers 3.5.1 without test edits.

### Conclusion

Admit GAMDL v3.5.1 to the support window via a single-file change to [`tool-versions.toml`](../../src-tauri/tool-versions.toml):

```toml
maximum_tested_version = "3.5.1"
recommended_version    = "3.5.1"
```

This causes:

- The in-app updater UI to drop the **Untested** badge from `3.5.1` (the `is_above_tested_ceiling` check returns `false`).
- The setup wizard's "recommended version" pin to land on `3.5.1` for fresh installs.
- The startup capability log to classify `3.5.1` as `VersionSupport::Supported` rather than `VersionSupport::Untested`.

No regression risk — the change is a documentation-shaped one-liner protecting the same code paths that already supported `3.5`.

## Cross-version cumulative posture

| GAMDL | Status | Audit |
| --- | --- | --- |
| `2.9.1` | Minimum supported | (legacy) |
| `3.0` | Supported | inline `tool-versions.toml` notes |
| `3.1` | Supported | inline `tool-versions.toml` notes (#604) |
| `3.2` | Supported | [`gamdl-v3.2-audit.md`](./gamdl-v3.2-audit.md) |
| `3.3` | Supported | inline `tool-versions.toml` notes |
| `3.4` | Supported | [`gamdl-v3.4-v3.5-audit.md`](./gamdl-v3.4-v3.5-audit.md) |
| `3.5` | Supported | [`gamdl-v3.4-v3.5-audit.md`](./gamdl-v3.4-v3.5-audit.md) |
| **`3.5.1`** | **Supported (this audit)** | **this document** |
