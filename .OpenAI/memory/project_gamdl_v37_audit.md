---
name: project-gamdl-v37-audit
description: GAMDL v3.7 (2026-05-23) — library URL expansion, --ffmpeg-path REINSTATED post-v3.6 removal, DRM-free tracks. New FFmpegPath capability gate (true on <3.6 OR >=3.7).
metadata:
  type: project
---

# GAMDL v3.7 audit + MeedyaDL adaptations (2026-05-23)

**Status:** Audit doc + capability-gate code landed on `feat/gamdl-3.7-support-on-alpha` (commits `dada73d3` + `1ce581c7`). PR to alpha pending final docs sweep.

**Upstream**: https://github.com/glomatico/gamdl/releases/tag/3.7 — 22 commits, 17 files touched between `3.6...3.7`. Plus v3.7.1 already 8 commits ahead on upstream `main` (bug-fix-only).

## Headline changes + MeedyaDL response

1. **`--ffmpeg-path` REINSTATED (#867 / #869).** v3.6 dropped all three tool-path CLI options when switching to native muxing. v3.7 brings `--ffmpeg-path` BACK because N_m3u8DL-RE depends on FFmpeg for HLS streaming. The other two (`--mp4box-path`, `--mp4decrypt-path`) stay removed.

   New `GamdlFeature::FFmpegPath` capability gate: `true` on `<3.6` OR `>=3.7` — only `false` on the `3.6.x` line. Three-version emission table:
   - ≤ 3.5.x: all three tool paths emitted
   - 3.6.x: none emitted (Click crashes on "no such option")
   - ≥ 3.7: only `--ffmpeg-path` emitted

   The existing `NativeMuxing` gate continues to govern the two still-removed paths.

2. **Library URL support extended (#870 / #871).** GAMDL now natively downloads from the user's personal Apple Music library across all media types — `/library/{albums,playlists,songs,music-videos}/` with `{p.,l.,i.}*` ID prefixes. MeedyaDL's frontend URL parser already routes `/library/` URLs via substring match — no code change for routing.

   Open follow-up: full enrichment skip (#871) deferred — existing 404-fallback handles library catalog calls correctly, just noisily. Will be done post-v3.7-PR.

3. **DRM-free tracks (#872).** Uploaded music videos / library uploads now marked `drm_free=True`; decrypt step skipped server-side. Verification follow-up; no code change expected.

## v3.7.1 readiness

Already 8 commits on upstream `main` past v3.7 — pure bug-fix release. The FFmpegPath gate's `>=3.7` check accepts 3.7.1 transparently (`is_version_at_least("3.7.1", "3.7") == true`). When upstream tags 3.7.1, admit via one-line `tool-versions.toml` PR — same zero-code-change shape as v3.3 / v3.5 / v3.5.1 / v3.5.2.

## Codebase impact summary

- `src-tauri/src/services/gamdl_capabilities.rs` — new `FFmpegPath` variant + `is_available_on` arm + `active_capabilities_summary` entry + unit test
- `src-tauri/src/models/gamdl_options.rs::path_cli_args()` — split tool-path emission into FFmpegPath-gated vs NativeMuxing-gated blocks
- `src-tauri/src/services/config_service.rs::ini_tool_path_section()` — mirrors the CLI split for INI key emission
- `src-tauri/src/commands/dependencies.rs::GamdlCapabilities` DTO — new `ffmpeg_path: bool` field
- `src/lib/tauri-commands.ts::GamdlCapabilities` interface — frontend DTO mirror
- `src-tauri/tool-versions.toml` — `maximum_tested_version` / `recommended_version` bumped to `3.7`; v3.7 + v3.7.1-readiness blocks appended

## How to apply

When investigating GAMDL output / classifying users in support:

- Check installed GAMDL via `gamdl --version` (or look at MeedyaDL's startup activity log — version + capabilities summary line)
- Three eras: `<3.6` (legacy tool paths), `3.6.x` (native muxing, all 3 paths suppressed), `>=3.7` (FFmpeg path back)
- On 3.7.1+ ("Untested" badge), users still get the FFmpeg path emitted automatically

## Related

- [[project_gamdl_release_cadence]] — historical audit pattern; v3.7 follows the same shape
- Audit doc: `.github/audits/gamdl-v3.7-audit.md`
- Parent EPIC: #867
- Drift issue: #873 — main is behind alpha (v1.10.0 stable shipped without 3.6 work) — separate rationalisation PR pending
