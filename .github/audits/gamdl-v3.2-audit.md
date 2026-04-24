# GAMDL v3.2 Compatibility Audit

**Branch**: `claude/audit-gamdl-v3.2-eI87q`
**Date**: 2026-04-24
**GAMDL releases audited**: 2.9.1, 2.9.2, 2.9.3, 3.0, 3.1, 3.2
**Umbrella issue**: #613

This document captures the audit findings for GAMDL v3.2 compatibility against
MeedyaDL's integration surface. Each section below corresponds to a filed
GitHub issue and records the investigation, verified facts, and resulting
decision.

## Methodology

1. Fetched every GAMDL source tarball from PyPI (`pip download --no-binary :all:`)
   for versions 2.9.1 through 3.2 and diffed them pairwise.
2. Cross-referenced every CLI flag in `gamdl/cli/cli_config.py` and every INI
   key emitted by MeedyaDL's `config_service.rs::settings_to_ini` against the
   Click parameter set each version actually recognises.
3. Verified behaviour of `dataclass_click` (which sets `click.Parameter.name`
   from the Python field name, not the `--flag` name) by running a minimal
   repro locally.
4. Audited MeedyaDL's integration sites:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`)
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`)
   - `src-tauri/src/services/download_queue.rs` (`merge_options`)
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature gating)
   - `src-tauri/src/utils/process.rs` (output parsing)
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window)

## Issue 614 — `--song-codec` rejected on v3.0+ (and never existed on v2.9.1+)

Per user request, this finding was re-verified against the full support window
(not just v3.x). The `else if` branch in `audio_cli_args()` that emits
`--song-codec <codec>` when `song_codec_priority` is unset crashes GAMDL on
**every** release in our support window, not only v3.0+.

### Upstream CLI declaration, grepped across every tarball

```text
v2.9.1: line 378 — song_codec_piority: Annotated[list[SongCodec], option("--song-codec-priority", …)]
v2.9.2: line 378 — (identical)
v2.9.3: line 378 — (identical)
v3.0:   line 223 — (module relocated, same declaration)
v3.1:   line 239 — (padded structlog changes only)
v3.2:   line 239 — (same as 3.1)
```

**No release in our support window declares `--song-codec` as a Click option.**
The single-codec flag was removed when GAMDL split `cli.py` into the
`cli/cli_config.py` / `cli/config_file.py` structure in the 2.9.1 refactor.

### Fallback-strategy outcome

The user asked: if v2.9.1–v2.9.3 doesn't support the fix, consider raising the
support floor to v3.x only (last resort). **Not necessary.** The proposed fix
(always emit `--song-codec-priority`) works on every release from v2.9.1
onward because:

1. `--song-codec-priority` exists in v2.9.1.
2. Its type is `Csv(SongCodec)` — a one-element CSV is valid, so `alac` is
   accepted identically to `alac,atmos,aac`.

### Resolution

Filed as #614 with Option B (always emit `--song-codec-priority`). Support
window remains `[2.9.1, 3.2]`.

## Issue 615 — Parser regression tests against v3.2 output

v3.2 made two changes that interact with MeedyaDL's stdout parser:

1. `track_log.info(f'Downloading "{media_title}"')` in `gamdl/cli/cli.py` is
   now conditional on `download_item.media.partial AND media_type in {songs,
   library-songs, music-videos, library-music-videos, uploaded-videos, None}`.
   The wrapper media types (`albums`, `playlists`, `artists`) no longer emit
   the line — a cleanup, not a regression for us, since those don't have
   individual `[Track N/M]` counters anyway.
2. The exception class previously raised as
   `GamdlDownloaderFlatFilterExcludedError` is now
   `GamdlInterfaceFlatFilterExcludedError`.

### MeedyaDL parser audit

- `TRACK_INFO_V2_REGEX` (`src-tauri/src/utils/process.rs:127`) matches on the
  bracket + `Downloading` + quoted title shape — independent of which
  `media_type` the line is emitted for. The v3.2 change makes the line fire
  less often but not incorrectly.
- `classify_error()` has no branch for `FlatFilterExcluded` today.
  `grep -n "FlatFilterExcluded"` against `process.rs` +
  `download_queue.rs` returns zero matches. The rename is invisible to our
  parser — any such line is bucketed as `"unknown"`.

### Scope confirmed

1. Add real-sample captures from a v3.2 run (album, single song, MV, playlist,
   artist-bucket) under `.github/audits/fixtures/gamdl-3.2/`.
2. Add parser regression tests keyed off those captures.
3. Recommend — not block on — adding a dedicated `flat_filter_excluded`
   classifier branch if we later adopt `--database-path` (#523 currently
   declined; would need re-litigation).

## Issue 616 — Sequential metadata fetch (observability only)

v3.2 flipped the `AppleMusicInterface.concurrency` default from 5 → 1.
Effect: the metadata fan-out phase for albums / playlists / artist buckets
is now serialised. No CLI surface was added and MeedyaDL can't tune it.

### Alignment with MeedyaDL's own serial-queue design (#455)

MeedyaDL already processes the queue serially — one queue item's entire
pipeline (download → companions → enrichment → lyrics → manifest)
completes before the next starts. Upstream's v3.2 switch is philosophically
aligned, reaching the same reliability-over-throughput conclusion at a
different scope (metadata fan-out within one download vs. queue-level
fan-out across downloads). The audit recommends calling this out in the
help FAQ so the design consistency is visible to users.

### Behaviour delta (measured on audit host, indicative only)

- Single-song URL: no observable change.
- ~10-track album: v3.1 ~2s metadata phase; v3.2 ~5–10s.
- 100-track playlist: v3.1 ~5s but occasional AMP 429 cascades; v3.2 ~30–60s
  and reliably completes.

### Resolution

Filed as #616. Ships alongside the `tool-versions.toml` 3.2 bump (#619).
No code change — CHANGELOG + help FAQ entry only.

## Issue 617 — Upstream INI typo `song_codec_piority`

GAMDL's `cli_config.py` has declared the codec-priority dataclass field
as `song_codec_piority` (missing the `r` in `priority`) on every release
from v2.9.1 onward. `dataclass_click` sets `click.Parameter.name` from
the Python field name, so the INI key GAMDL reads and writes is the
typo'd one — not `song_codec_priority` (which MeedyaDL writes).

### Verified via local repro

```text
$ python3 -c "from dataclass_click import dataclass_click, option; ..."
param.name: song_codec_piority || opts: ['--song-codec-priority']
```

`ConfigFile.update_params_from_config()` reads values keyed by
`param.name`. `cleanup_unknown_params()` silently removes keys not in
the Click param set. So MeedyaDL's `song_codec_priority = …` INI line
has been decorative — silently dropped — on every release in our
support window.

### Why downloads still work

MeedyaDL passes `--song-codec-priority <chain>` on every subprocess
call. Click matches on `opts:` (the `--flag` form), not `param.name`,
so the CLI path is unaffected. Codec preference reaches GAMDL via the
CLI; the INI has been a no-op for it.

### Resolution

Filed as #617. Recommended resolution: **Option D** — drop the codec
block from `ini_audio_section` entirely (both `song_codec` and
`song_codec_priority`). The CLI path is authoritative; the INI emission
has never worked on v2.9.1+. Optionally file upstream PR (Option C) to
rename the misspelled field; no hard dependency either way.

## Issue 618 — `--playlist-folder-template` (new in v3.0)

Cross-version check of `cli_config.py`:

```text
v2.9.1: not declared
v2.9.2: not declared
v2.9.3: not declared
v3.0:   line 382 — playlist_folder_template: Annotated[str, option("--playlist-folder-template", …)]
v3.1:   line 390 — (same)
v3.2:   line 390 — (same)
```

Upstream default (`gamdl/downloader/base.py:35` on 3.2):

```python
playlist_folder_template: str = "Playlists/{playlist_artist}"
```

### Capability gate mandatory (not optional)

The original #516 deferral framed the capability gate as optional. The
audit re-confirms it's mandatory: passing `--playlist-folder-template
…` to v2.9.x crashes Click with `no such option`. MeedyaDL supports
the full 2.9.1–3.2 range, so the flag must be gated behind
`GamdlFeature::PlaylistFolderTemplate` with
`is_version_at_least(version, "3.0")`, mirroring the shape of
`GamdlFeature::WrapperM3u8Ip` (#605).

### Resolution

Filed as #618 with updated wording that treats the capability gate as
required. Low priority but a clean win.





