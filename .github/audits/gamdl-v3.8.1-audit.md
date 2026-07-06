# GAMDL v3.8.1 Compatibility Audit + v2-support drop

**Date**: 2026-07-03
**GAMDL release audited**: 3.8.1 (released 2026-07-03)
**Diff range**: `3.8..3.8.1` (2 commits, 2 files)
**Predecessor audit**: [`gamdl-v3.8-audit.md`](./gamdl-v3.8-audit.md)
**Tracking issue**: TBD

## TL;DR

**Zero-code-change admission** for the v3.8.1 upstream delta — plus a
deliberate, separate policy change in the same PR: **GAMDL v2 support is
dropped** (`minimum_version` 2.9.1 → 3.0).

- **v3.8.1 upstream**: one functional commit (`30f02156`, "Support
  non-enhanced song HLS playlists") + a version bump. It fixes an upstream
  issue that prevented some songs from downloading with non-web codecs — a
  follow-up to v3.8's `/v1/play/assets` endpoint change. Entirely internal
  to GAMDL's HLS stream-info selection + its interactive codec prompt.
  MeedyaDL never parses m3u8 / stream-info and never uses interactive mode,
  so **no MeedyaDL code change is required**. It's a free reliability win
  that reinforces v3.8's wrapper-less non-web-codec support.
- **v2 drop**: the supported line is now **GAMDL v3.x only**, split across
  two wrapper generations that both remain fully supported — v3.0–v3.5.x on
  [wrapper-v1](https://github.com/WorldObservationLog/wrapper) (three local
  sockets) and v3.6+ on [wrapper-v2](https://github.com/glomatico/wrapper-v2)
  (single HTTP daemon). No capability-gate predicates change; the gates
  keyed on the old 2.9.1 floor go always-true inside the window but keep
  their exact version-math (still correct + unknown-version-safe).

Bump `maximum_tested_version` + `recommended_version` 3.8 → 3.8.1 and
`minimum_version` 2.9.1 → 3.0 in
[`tool-versions.toml`](../../src-tauri/tool-versions.toml).

## Methodology

Same six-surface cross-reference as the v3.7.4 / v3.8 audits:

1. `src-tauri/src/models/gamdl_options.rs` — CLI flag / INI emission.
2. `src-tauri/src/services/config_service.rs` — INI generation.
3. `src-tauri/src/services/download_queue.rs` — subprocess + stdout/stderr
   parsing + `merge_options` + completion task.
4. `src-tauri/src/services/gamdl_capabilities.rs` — `GamdlFeature` gates +
   support window.
5. `src-tauri/src/services/gamdl_service.rs` — install / spawn.
6. `src-tauri/src/utils/process.rs` — regexes + classifiers.

Plus the v3.8.1 upstream patch was read in full (both files) and scanned
for CLI (`@click` / `add_option`), exception (`class …Error`), log
(`log.info/warning/error`, `print`, `track_log`), and output-shape changes.

## v3.8.1 — `3.8..3.8.1` change set

| Commit | Subject | Files |
| --- | --- | --- |
| `30f02156` | Support non-enhanced song HLS playlists | `gamdl/interface/song.py`, `gamdl/cli/interactive_prompts.py` |
| `e46f637f` | Bump version to 3.8.1 | `gamdl/__init__.py`, `pyproject.toml`, `uv.lock` |

### Finding 3.8.1-A — non-enhanced HLS stream-info selection

`gamdl/interface/song.py`: the private `_get_stream_info` is renamed to
`_get_stream_info_nonweb` and split so it branches on whether the m3u8
master is an "enhanced" playlist:

- new `_is_enhanced_m3u8_master(m3u8_master_data)` predicate,
- new `_get_stream_info_enhanced(...)` / `_get_stream_info_nonenhanced(...)`,
- new `_get_stream_info_from_playlist(...)` +
  `_get_playlist_from_codec_enhanced/_nonenhanced(...)` helpers.

**Why**: some songs expose only a *non-enhanced* HLS master for their
non-web codecs; the v3.8 code path assumed the enhanced shape and failed to
pick a stream, so those songs wouldn't download with non-web codecs. The new
branch handles both playlist shapes.

**MeedyaDL surface impact: none.** All new methods are private (`_`-prefixed)
and internal to GAMDL's HLS stream-info selection. MeedyaDL hands GAMDL a URL
and observes the resulting audio files on disk — it never inspects the m3u8
master, the stream-info dict, or the playlist selection. The only added log
statements are `logger.bind(action="get_song_stream_info_enhanced" | "…_nonenhanced")`
structlog **context binds** that fire at DEBUG level only (MeedyaDL runs at
INFO and doesn't set `--log-level Debug`); they are not new log *lines* that
`utils/process.rs` regex-parses. No CLI flag, INI key, exception class, or
`Saved to:` / TrackInfo output-shape change.

### Finding 3.8.1-B — interactive codec-choice label

`gamdl/cli/interactive_prompts.py`: `ask_song_codec` now labels each codec
choice via a new `_get_song_codec_choice_name(playlist)` helper that falls
back from `stream_info["audio"]` to a `codecs | bandwidth | uri` string when
`audio` is absent (which is exactly the non-enhanced-playlist case from
Finding A).

**MeedyaDL surface impact: none.** `ask_song_codec` is only reached in
GAMDL's *interactive* prompt flow. MeedyaDL never runs GAMDL interactively —
it passes explicit codecs via `--song-codec-priority` and pre-authenticates
the wrapper so the subprocess never blocks on stdin. This code path is
unreachable from MeedyaDL.

## v2-support drop (policy change, same PR)

`minimum_version` 2.9.1 → 3.0. The supported line is now v3.x only:

- **v3.0 – v3.5.x** — wrapper-v1 (three sockets: HTTP account 30020, TCP
  m3u8 20020, TCP decrypt 10020). Gated by `WrapperM3u8Ip` (>= 3.1) etc.
- **v3.6+** — wrapper-v2 (single `--wrapper-url` HTTP daemon). Gated by
  `WrapperUrl` (>= 3.6).

Both wrapper generations remain fully supported; the Settings UI renders the
correct fields for the installed GAMDL. No `GamdlFeature` predicate changes:

- The **v2.9.1-keyed gates** — `NativeCodecPriority`,
  `ClassicalMusicHostRequired`, `StorefrontIniKeyStripped` — are now
  always-true inside `[3.0, 3.8.1]`. Their `is_version_at_least(v, "2.9.1")`
  math is unchanged (always true for v ≥ 3.0) and still returns `false` for
  the unknown-version case, so no code edit is needed.
- **`FetchExtraTags`** (`!is_version_at_least(v, "3.0")`, i.e. v2.x only) is
  now permanently `false` inside the window. `merge_options` leaves
  `options.fetch_extra_tags = None` and the INI writer skips the key, so
  emission is already correctly suppressed — the plumbing just goes inert.
  Removing the now-dead `fetch_extra_tags` setting / UI is a follow-up
  cleanup (tracked separately), not required for correctness.

No test changes were needed: the `classify_*` tests read
`support_window().minimum` / `.maximum_tested` dynamically (so they adapt to
3.0 / 3.8.1 automatically), and the capability-gate tests exercise
version-comparison math with explicit inputs (independent of the support
floor), so they continue to pass. `cargo test services::gamdl_capabilities`
→ 32/32 pass after the change.

## MeedyaDL surface-impact summary

| Surface | v3.8.1 | v2-drop |
| --- | --- | --- |
| CLI flag encoding (`gamdl_options.rs`) | none | none |
| INI emission (`config_service.rs`) | none | `fetch_extra_tags` inert (already suppressed) |
| stdout/stderr parsing + classifiers (`process.rs`) | none | none |
| capability gates (`gamdl_capabilities.rs`) | ceiling → 3.8.1 | floor → 3.0; predicates unchanged |
| install / spawn (`gamdl_service.rs`) | none | `pip …'gamdl>=3.0,<=3.8.1'` |
| HTTP / auth / artwork / lyrics (`apple_music_api.rs`) | none | none |

## Actions taken

- `src-tauri/tool-versions.toml`: `minimum_version` 2.9.1 → 3.0,
  `maximum_tested_version` + `recommended_version` 3.8 → 3.8.1, plus the
  v3.8.1 audit-trail block and the v2-drop rationale.
- `src-tauri/src/services/gamdl_capabilities.rs`: module-doc + `pip_version_spec`
  doc-comment refreshed for the v3-only / 3.0–3.8.1 range (no logic change).
- `README.md`: support-matrix GAMDL row → **3.0 – 3.8.1**, recommended 3.8.1,
  with the wrapper-v1/v2 split called out.
- `.claude/CLAUDE.md`: "Version-aware GAMDL dispatch" extended with v3.8.1 +
  the v2-drop note.

## Out-of-scope / follow-ups

- **Remove dead v2-only plumbing** (`fetch_extra_tags` setting + UI +
  `FetchExtraTags` gate) — inert now that v2 is unsupported; tracked as a
  `for consideration` cleanup issue.
- Wrapper-v2 protocol surface — untouched by v3.8.1.
