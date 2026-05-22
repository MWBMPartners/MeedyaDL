# GAMDL v3.5.2 Compatibility Audit

**Date**: 2026-05-14
**GAMDL release audited**: 3.5.2 (released 2026-05-13, upstream commit `82e3cf2`)
**Diff range**: `3.5.1..3.5.2` (4 commits, 4 functional files)
**Predecessor audit**: [`gamdl-v3.5.1-audit.md`](./gamdl-v3.5.1-audit.md)
**Tracking issue**: #767

## TL;DR

Pure upstream bug-fix release. **No MeedyaDL code change is required.** Bump `maximum_tested_version` and `recommended_version` in [`tool-versions.toml`](../../src-tauri/tool-versions.toml) from `"3.5.1"` → `"3.5.2"`. Same zero-code-change shape as v3.3 (playlist fix), v3.5 (iTunes lookup fix), and v3.5.1 (music-video 403 fix).

The headline fix is a music-video m3u8 host-rewrite correction tracking Apple's migration of the master-playlist host from `itunes.apple.com` to `play-edge.itunes.apple.com`. Music-video downloads that silently broke on 3.5.1 once Apple flipped the edge host will resolve the moment users upgrade.

## Methodology

Identical to the v3.4 / v3.5 / v3.5.1 audits:

1. Cloned `glomatico/gamdl`, materialised both tags, ran `git diff --stat 3.5.1..3.5.2` and `git diff 3.5.1..3.5.2`.
2. Cross-referenced each hunk against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates).
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`).
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task).
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature flags).
   - `src-tauri/src/utils/process.rs` (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `classify_error`, `parse_gamdl_output`).
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window).

## v3.5.2 — `3.5.1..3.5.2` change set

Four commits, six files touched (two are pure version metadata):

| Commit    | Subject                                                  |
| --------- | -------------------------------------------------------- |
| `b48dbef` | Forward next_params (except limit) for pagination        |
| `dec4a22` | Bind logger and log m3u8 master URL extraction           |
| `bc4cdd1` | Open file with UTF-8 encoding in add_file                |
| `82e3cf2` | Bump version to 3.5.2                                    |

`git diff --stat 3.5.1..3.5.2`:

```
gamdl/__init__.py              | 2 +-
gamdl/api/apple_music.py       | 4 +---
gamdl/cli/utils.py             | 2 +-
gamdl/interface/music_video.py | 6 +++++-
pyproject.toml                 | 2 +-
uv.lock                        | 2 +-
6 files changed, 10 insertions(+), 8 deletions(-)
```

### Finding 3.5.2-A — `_amp_request` pagination forwards all next-link params (commit `b48dbef`)

In `gamdl/api/apple_music.py`, the pagination helper inside `AppleMusicApi._amp_request` previously hand-extracted only the `offset` parameter from the Apple Music `next` link:

```python
# 3.5.1
offset = int(next_params["offset"][0])
extended_data = await self._amp_request(
    urlparse(next_uri).path,
    {
        "offset": offset,
        **({"limit": limit} if limit else {}),
    },
)
```

3.5.2 replaces it with a generic dict spread that forwards every query param from `next_params` except `limit`:

```python
# 3.5.2
extended_data = await self._amp_request(
    urlparse(next_uri).path,
    {
        **({"limit": limit} if limit else {}),
        **{k: v for k, v in next_params.items() if k not in ["limit"]},
    },
)
```

This is a pure HTTP-layer bug fix — fixes pagination of Apple Music resources whose `next` link carries query parameters beyond `offset` (e.g., `art[url]`, `extend`, `include`, `art[format]` for playlists / artist track lists with editorial art).

**MeedyaDL impact**: none. MeedyaDL never touches GAMDL's pagination internals; we consume the final filesystem output only. User-visible benefit: any 3.5.1 pagination failure on a large playlist or artist resource silently resolves on 3.5.2.

### Finding 3.5.2-B — Music-video m3u8 host rewrite + debug log (commit `dec4a22`)

In `gamdl/interface/music_video.py::get_m3u8_master_url_from_itunes_page_metadata`, two changes:

```python
# 3.5.1
m3u8_master_url = m3u8_master_url.replace(
    "itunes.apple.com",
    "play.itunes.apple.com",
).replace(
    "MZPlayLocal.woa",
    "MZPlay.woa",
)

# 3.5.2
log = logger.bind(action="get_m3u8_master_url_from_itunes_page_metadata")
...
m3u8_master_url = m3u8_master_url.replace(
    "play-edge.itunes.apple.com",
    "play.itunes.apple.com",
).replace(
    "MZPlayLocal.woa",
    "MZPlay.woa",
)
...
log.debug("success", m3u8_master_url=m3u8_master_url)
```

1. **Host-rewrite fix** — the substring being rewritten changed from `"itunes.apple.com"` to `"play-edge.itunes.apple.com"`. Apple migrated which host returns the m3u8 master URL inside the iTunes page metadata, and GAMDL needs to canonicalise that host to `play.itunes.apple.com` before handing the URL to the HLS downloader. This is **the** user-facing fix referenced in the release ("Fixed an issue with music video m3u8 URL formation").
2. **Debug log** — new `logger.bind()` + `log.debug("success", m3u8_master_url=…)` line that fires only at `--log-level DEBUG`.

**MeedyaDL impact**:

- **Host fix**: positive indirect — music-video downloads (both standalone MV URLs and music-video companions) that silently broke on 3.5.1 due to Apple's edge-host migration succeed once users upgrade. The fix is silent — no opt-in required, no setting change, no UI toggle.
- **Debug log**: none. GAMDL's default log level is `INFO`. MeedyaDL has a `GamdlOptions.log_level: Option<LogLevel>` field in `src-tauri/src/models/gamdl_options.rs` but it is never wired to settings (no UI control, no `AppSettings` field) and is `None` in every non-test code path, so MeedyaDL never passes `--log-level DEBUG` to the GAMDL subprocess. The new debug line will not appear in stdout under normal operation. If a developer manually overrides the log level for local debugging, the line would render as a benign `GamdlOutputEvent::Unknown` event in `parse_gamdl_output` — no error-classifier match, no regex match, no UI surface change.

### Finding 3.5.2-C — `CustomOutputWriter.add_file()` opens log file as UTF-8 (commit `bc4cdd1`)

```python
# 3.5.1
file_stream = open(path, "a")

# 3.5.2
file_stream = open(path, "a", encoding="utf-8")
```

`CustomOutputWriter.add_file()` is the codepath GAMDL uses when the user passes `--log-file <path>`. The default platform encoding on Windows is `cp1252`, which raises `UnicodeEncodeError` on non-ASCII track titles. Forcing UTF-8 fixes that.

**MeedyaDL impact**: none. MeedyaDL never passes `--log-file` to the GAMDL subprocess. We capture stdout/stderr via Tokio's `BufReader::lines()`, which is independent of the `add_file()` codepath. Confirmed by `grep -rn 'log-file\|log_file' src-tauri/src/` — every hit is unrelated (MeedyaDL's own on-disk activity-log writer, or doc references).

### Finding 3.5.2-D — Version metadata bump (commit `82e3cf2`)

`gamdl/__init__.py`, `pyproject.toml`, `uv.lock` — version strings updated `3.5.1` → `3.5.2`. Trivial. The `gamdl_capabilities::detect_version` cache will pick up the new string on next invocation without code change.

## MeedyaDL surface impact: none

Verified by inspection that:

- **No `GamdlFeature` gate is required.** No CLI flag added or removed; no INI key added or removed. The capability cache only gates flag emission, and the gate set is unchanged.
- **No `to_cli_args` / `audio_cli_args` / `video_cli_args` change.** Every emission path (`--song-codec-priority`, `--video-quality`, `--wrapper-m3u8-ip`, `--wrapper-account-url`, `--wrapper-decrypt-ip`, `--playlist-folder-template`, `--no-exceptions`, `--fetch-extra-tags`) is unaffected.
- **No INI emission change.** `ini_audio_section`, `ini_metadata_section`, `ini_template_section` — all untouched.
- **No output parser change.** `TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `PYTHON_EXCEPTION_REGEX`, `is_python_traceback_noise` — all unaffected at the default `INFO` log level (the new `log.debug()` line in Finding 3.5.2-B does not reach stdout).
- **No exception class rename.** `extract_python_exception` regex unchanged; `is_storefront_mismatch_error` substring matcher unchanged.
- **No subprocess-failure message format change.** Still the v3.4 enriched `'Exited with code N: <args>\nstdout:\n…\nstderr:\n…'` format.
- **No logging stream change.** Still stdout per v3.4's `PrintLoggerFactory(file=CustomOutputWriter([sys.stdout]))`.
- **No support-window test threshold change.** The existing `gamdl_capabilities::tests` suite already classifies any 3.x-series version `<= maximum_tested_version` as `Supported`, so bumping the ceiling automatically covers 3.5.2 without test edits.

### Surface check matrix

| Surface                                      | Change? |
| -------------------------------------------- | ------- |
| CLI flags added/removed/renamed              | None    |
| INI keys added/removed/renamed               | None    |
| stdout/stderr format (default `--log-level`) | None    |
| `TRACK_INFO_V2_REGEX`                        | None    |
| `ERROR_PREFIX_REGEX`                         | None    |
| `PYTHON_EXCEPTION_REGEX`                     | None    |
| `classify_error()` substrings                | None    |
| `is_storefront_mismatch_error()` shape       | None    |
| `GamdlFeature` gates                         | None    |

## Conclusion

Admit GAMDL v3.5.2 to the support window via a single-file change to [`tool-versions.toml`](../../src-tauri/tool-versions.toml):

```toml
maximum_tested_version = "3.5.2"
recommended_version    = "3.5.2"
```

This causes:

- The in-app updater UI to drop the **Untested** badge from `3.5.2` (the `is_above_tested_ceiling` check returns `false`).
- The setup wizard's "recommended version" pin to land on `3.5.2` for fresh installs.
- The startup capability log to classify `3.5.2` as `VersionSupport::Supported` rather than `VersionSupport::Untested`.

No regression risk — the change is a documentation-shaped one-liner protecting the same code paths that already supported `3.5.1`.

## Cross-version cumulative posture

| GAMDL       | Status                       | Audit                                                            |
| ----------- | ---------------------------- | ---------------------------------------------------------------- |
| `2.9.1`     | Minimum supported            | (legacy)                                                         |
| `3.0`       | Supported                    | inline `tool-versions.toml` notes                                |
| `3.1`       | Supported                    | inline `tool-versions.toml` notes (#604)                         |
| `3.2`       | Supported                    | [`gamdl-v3.2-audit.md`](./gamdl-v3.2-audit.md)                   |
| `3.3`       | Supported                    | inline `tool-versions.toml` notes                                |
| `3.4`       | Supported                    | [`gamdl-v3.4-v3.5-audit.md`](./gamdl-v3.4-v3.5-audit.md)         |
| `3.5`       | Supported                    | [`gamdl-v3.4-v3.5-audit.md`](./gamdl-v3.4-v3.5-audit.md)         |
| `3.5.1`     | Supported                    | [`gamdl-v3.5.1-audit.md`](./gamdl-v3.5.1-audit.md)               |
| **`3.5.2`** | **Supported (this audit)**   | **this document**                                                |
