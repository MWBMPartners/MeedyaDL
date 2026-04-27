# GAMDL v3.4 + v3.5 Compatibility Audit

**Date**: 2026-04-27
**GAMDL releases audited**: 3.4 (released 2026-04-27 09:38 UTC), 3.5 (released 2026-04-27 12:21 UTC)
**Diff range**: `3.3..3.4` (11 commits, 9 files), `3.4..3.5` (4 commits, 3 files)
**Predecessor audit**: [`gamdl-v3.2-audit.md`](./gamdl-v3.2-audit.md), v3.3 inline notes in `tool-versions.toml`

This document captures the audit findings for GAMDL v3.4 and v3.5 compatibility against MeedyaDL's integration surface. Both releases are bug-fix only — no CLI flags added or removed, no INI keys changed, no output-format regressions. The conclusion is that **no MeedyaDL code change is required**; the support window can be bumped from `[2.9.1, 3.3]` to `[2.9.1, 3.5]` in a single-file PR.

## Methodology

1. Pulled the full diff of each release pair via `gh api repos/glomatico/gamdl/compare/{base}...{head}` and inspected every patch hunk.
2. Cross-referenced the changes against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates)
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`)
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task)
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature flags)
   - `src-tauri/src/utils/process.rs` (`TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `classify_error`, `parse_gamdl_output`)
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window)
3. Verified each finding against the pre-3.4 source (`gh api repos/glomatico/gamdl/contents/...?ref=3.3`) so the diff interpretations are not based on commit-message wording alone.

## v3.4 — `3.3..3.4` change set

Eleven commits in upstream order:

| Commit | Subject |
| --- | --- |
| `a922749` | Include subprocess output in async errors |
| `df23276` | Improve subprocess error message |
| `939520b` | Stringify subprocess args in error message |
| `63ad0f2` | Respect `skip_cleanup` when removing temp files |
| `716112c` | Use `default_factory` for `DownloadItem` uuid |
| `e5675f8` | Use `CustomOutputWriter` for structlog output |
| `5d242c8` | Remove `'level'` and `'event'` from `event_dict` |
| `2e57216` | Strip size suffix from Apple Music cover URLs |
| `37ede65` | Add overwrite flag to `Database` |
| `64b1974` | Include filter result in exclusion error message |
| `a009071` | Bump version to 3.4 |

### Finding 3.4-A — Logging output stream swap (stderr → stdout)

The most behaviourally significant change in v3.4. Before:

```python
# gamdl/cli/cli.py — pre-3.4
root_logger = logging.getLogger(__name__.split(".")[0])
root_logger.setLevel(config.log_level)
stream_handler = logging.StreamHandler()  # defaults to sys.stderr
stream_handler.setFormatter(logging.Formatter("%(message)s"))
root_logger.addHandler(stream_handler)
…
structlog.configure(
    processors=[
        structlog.processors.add_log_level,
        structlog.processors.ExceptionPrettyPrinter(),
        custom_structlog_formatter,
    ],
    logger_factory=structlog.stdlib.LoggerFactory(),
)
```

After:

```python
# gamdl/cli/cli.py — 3.4
log_output = CustomOutputWriter()
if config.log_file:
    log_output.add_file(config.log_file)
structlog.configure(
    processors=[
        structlog.processors.add_log_level,
        structlog.processors.ExceptionPrettyPrinter(),
        custom_structlog_formatter,
    ],
    logger_factory=structlog.PrintLoggerFactory(file=log_output),
    wrapper_class=structlog.make_filtering_bound_logger(config.log_level),
)
```

Where `CustomOutputWriter` (added in `gamdl/cli/utils.py`) defaults to `streams=[sys.stdout]`.

**Net effect**: every GAMDL log line — `[INFO    HH:MM:SS] [Track 1/14 ] Downloading "…"`, `[ERROR    HH:MM:SS] Error processing …`, `Finished with N error(s)`, etc. — now lands on **stdout** instead of stderr.

**MeedyaDL impact**: ✅ Benign. Both reader tasks in `run_download_with_events()` (`download_queue.rs:7686`–`7991`) call `parse_gamdl_output()` identically and feed the same shared structures:

- `last_activity_ms` watchdog bumped on every line, regardless of stream.
- `update_item_progress(&download_id, &event)` called for every parsed event (`TrackInfo`, `Error`, `Saved`, `ProcessingStep`, …).
- `collected_errors` collects `Error` events from either reader.
- `seen_lines` cross-stream deduplication set already exists, anticipating exactly this kind of output-stream drift (added when GAMDL/yt-dlp briefly double-printed to both streams).

The only stream-asymmetric code path is the **cosmetic `──── [Track N/M] Downloading "Title" ────` separator** at `download_queue.rs:7849`, which is only emitted from the stdout reader. Pre-3.4 the separator was a latent no-op because TrackInfo lines arrived on stderr; on 3.4+ it now starts firing reliably. **Free UX improvement**, not a regression.

`extract_python_exception()` reads `raw_stderr_lines` only. Post-3.4, Python tracebacks rendered via structlog's `ExceptionPrettyPrinter` will arrive on stdout, not stderr, so `raw_stderr_lines` may not capture them. Tracebacks raised by GAMDL still come through the catch-all `except` in `cli.py` and are formatted by structlog → routed via the new `CustomOutputWriter` to stdout. **However**, raw subprocess tracebacks (e.g., when the Python process itself crashes uncaught) still go to stderr via the Python interpreter's default `sys.excepthook`. That is the only path `extract_python_exception` realistically catches anyway, and it's untouched by this change. **No code update required**.

### Finding 3.4-B — Subprocess error message format change

`gamdl/utils.py::async_subprocess()` now captures stdout/stderr from `silent` invocations (previously routed to `DEVNULL`) and embeds them in the exception message:

```python
# Pre-3.4
raise Exception(f'"{args[0]}" exited with code {proc.returncode}')

# 3.4
msg = f"Exited with code {proc.returncode}: {' '.join(str(arg) for arg in args)}"
if stdout:
    msg += f"\nstdout:\n{stdout.decode()}"
if stderr:
    msg += f"\nstderr:\n{stderr.decode()}"
raise Exception(msg)
```

This is the wrapper around every subtool spawn (ffmpeg, mp4decrypt, MP4Box, N_m3u8DL-RE, etc.) that GAMDL launches internally. The message is what propagates back through the GAMDL log line when a subtool fails.

**MeedyaDL impact**: ✅ Strictly improved.

- `process::classify_error()` (`utils/process.rs:636`) is purely substring-based: it scans for `"ffmpeg"`, `"mp4decrypt"`, `"mp4box"`, `"timeout"`, `"timed out"`, `"connection"`, `"dns"`, `"httpx"`, etc. The new format **embeds the subtool name in the args** (`Exited with code 1: ffmpeg -i …`) and the **subtool's own stderr output** (which contains the actual root-cause keyword). Pre-3.4 the user got `"ffmpeg" exited with code 1` and we matched only on the literal `"ffmpeg"` token; post-3.4 we get all of that **plus** the subtool's own diagnostic (e.g., `"could not find codec parameters for stream 0"`), which lets the classifier reach better verdicts.
- A `grep -rn 'exited with code'` across `src-tauri/src/` finds no string-equality match against the old format. All references are MeedyaDL's own messages (`"GAMDL process exited with code N"` in `download_queue.rs:8115` and `"{engine_id} process exited with code {code}"` in `engine_runner.rs:214`).

**No code update required.**

### Finding 3.4-C — `Database(overwrite)` constructor + sentinel return

`gamdl/cli/database.py::Database.__init__` now takes an `overwrite: bool` kwarg, and `flat_filter()` returns the sentinel string `"Registered in database"` (instead of a path) when the media is in the DB AND the file exists AND `not overwrite`.

**MeedyaDL impact**: ✅ N/A. MeedyaDL never sets `--database-path`. `gamdl_capabilities.rs` references `--database-path` only in module-level documentation as a known v3.0+-only flag we deliberately don't expose (#523 declined). The new constructor signature, the new return-value semantics, and the rephrased exception message (`GamdlInterfaceFlatFilterExcludedError`: `"Media excluded by flat filter (media ID: {media_id}): {result}"`) are all invisible to us.

### Finding 3.4-D — `DownloadItem.uuid_` factory fix

```python
# Pre-3.4 (bug):
uuid_: str = uuid.uuid4().hex[:8]      # evaluated ONCE at class definition

# 3.4 (correct):
uuid_: str = field(default_factory=lambda: uuid.uuid4().hex[:8])
```

Pre-3.4, every `DownloadItem` instance in a single GAMDL process shared the same UUID, which collided in the temp-folder path template (`TEMP_PATH_TEMPLATE.format(folder_tag)`). That collision is what the v3.4 cleanup-refactor (Finding 3.4-E) is paired with.

**MeedyaDL impact**: ✅ Internal to GAMDL. No surface change.

### Finding 3.4-E — `skip_cleanup` location refactor (no behavioural change)

Moved the `skip_cleanup` short-circuit from the body of `_cleanup_temp` into the call site:

```python
# Pre-3.4
async def download(self, item):
    try:
        …
    finally:
        self._cleanup_temp(item.uuid_)            # always called

def _cleanup_temp(self, folder_tag):
    temp_path = …
    if temp_path.exists() and temp_path.is_dir() and not self.skip_cleanup:
        shutil.rmtree(temp_path, ignore_errors=True)

# 3.4
async def download(self, item):
    try:
        …
    finally:
        if not self.skip_cleanup:
            self._cleanup_temp(item.uuid_)        # gated at caller

def _cleanup_temp(self, folder_tag):
    temp_path = …
    if temp_path.exists() and temp_path.is_dir():
        shutil.rmtree(temp_path, ignore_errors=True)
```

Net behaviour identical. ✅ No MeedyaDL impact.

### Finding 3.4-F — Raw cover URL: strip `/{w}x{h}bb.jpg` suffix

`_get_raw_cover_url` in `gamdl/interface/base.py` now strips the trailing `/{w}x{h}bb.jpg` from the templated cover URL before returning it. This matters when GAMDL is asked for the raw, unsized cover.

**MeedyaDL impact**: ✅ N/A. MeedyaDL doesn't consume GAMDL's cover URL output. The enrichment pipeline fetches cover artwork directly via `apple_music_api.rs::fetch_artwork` against the Apple Music API; GAMDL is responsible for writing its own cover files (`Cover.{ext}`) which MeedyaDL then renames via `rename_cover_art()` based on filename, not URL.

### Finding 3.4-G — `custom_structlog_formatter` `pop` instead of `get`

```python
level = event_dict.pop("level", "INFO").upper()   # was event_dict.get(…)
…
message = event_dict.pop("event", "")             # was event_dict.get(…)
```

Side-effect: when the formatter falls through to the `else: return f"{prefix} {event_dict}"` branch (DEBUG-level structured fields), the dict no longer contains `level` and `event` keys, so the rendered output is cleaner. Cosmetic — affects only the rare DEBUG-level structured emissions.

**MeedyaDL impact**: ✅ Cosmetic. We don't run GAMDL at DEBUG level by default (`LogLevel::Info`) so this is unobservable in practice.

### Finding 3.4-H — `--log-file` codepath migrated to `CustomOutputWriter.add_file()`

```python
if config.log_file:
    log_output.add_file(config.log_file)   # was logging.FileHandler(config.log_file, encoding="utf-8")
```

`CustomOutputWriter.add_file()` opens the path in `"a"` mode (append), registers an `atexit` handler to close the file, and adds the stream to the multiplexer.

**MeedyaDL impact**: ✅ N/A. MeedyaDL doesn't pass `--log-file` (only `--log-level` — see `gamdl_options.rs:1049`). The file-logging path is dormant for us.

### v3.4 — Summary

- **Behavioural changes that reach MeedyaDL**: 1 (logging stream swap stderr → stdout). Benign; reader tasks already symmetric.
- **Behavioural changes invisible to MeedyaDL**: 7 (subprocess error format, Database overwrite, DownloadItem UUID, cleanup refactor, raw cover URL, formatter pop, log-file file-handler swap, filter exclusion message).
- **CLI flag changes**: 0.
- **INI key changes**: 0.
- **Output-format regressions affecting `TRACK_INFO_V2_REGEX` or `ERROR_PREFIX_REGEX`**: 0.
- **`GamdlFeature` gates that need to flip on/off at v3.4**: 0.

## v3.5 — `3.4..3.5` change set

Four commits, all in the iTunes lookup path used for music-video metadata enrichment:

| Commit | Subject |
| --- | --- |
| `4e28b7e` | Enable redirects and use correct storefront header |
| `3765ef0` | Set `storefront_id` `None` for non-US iTunes API |
| `8f184fc` | Remove `-28` from `X-Apple-Store-Front` header |
| `f670fe8` | Bump version to 3.5 |

### Finding 3.5-A — `httpx.AsyncClient(follow_redirects=True)` for iTunes API

```python
# gamdl/api/itunes.py
client = httpx.AsyncClient(
    timeout=60.0,
    follow_redirects=True,    # NEW in 3.5
)
```

Apple's iTunes lookup endpoint sometimes 30x-redirects across regional CDNs, especially for newly-added music-video pages. Pre-3.5, `httpx` defaulted to `follow_redirects=False`, so any redirect surfaced as an empty response and broke the lookup. v3.5 unblocks those affected music videos.

**MeedyaDL impact**: ✅ Pure upstream win. We get more music-video metadata coverage for free.

### Finding 3.5-B — `X-Apple-Store-Front` header strip

```python
# Pre-3.5
"X-Apple-Store-Front": f"{self.storefront_id}-1,32 t:music31"

# 3.5
"X-Apple-Store-Front": f"{self.storefront_id},32 t:music31"
```

The `-1` (or `-28`, depending which commit message you read — the diff strips `-1`, the commit subject says `-28`, no consequential difference because both are storefront-modifier suffixes Apple now expects callers to omit) is removed. Internal HTTP detail; the header value drives Apple's storefront routing for the unauthenticated iTunes page endpoint.

**MeedyaDL impact**: ✅ N/A. MeedyaDL never constructs `X-Apple-Store-Front` itself; we only see the result of GAMDL's own iTunes calls.

### Finding 3.5-C — `storefront_id=None` for non-US storefronts

```python
# gamdl/interface/base.py
itunes_api = itunes_api or await ItunesApi.create(
    storefront=apple_music_api.storefront,
    language=apple_music_api.language,
    **(
        {"storefront_id": None}
        if apple_music_api.storefront.lower() != "us"
        else {}
    ),
)
```

Forces a storefront-ID lookup for any non-US user. The storefront ID is what the X-Apple-Store-Front header in 3.5-B is built from, so this is the second half of the same fix.

**MeedyaDL impact**: ✅ Pure upstream win. Apple's iTunes lookup now works correctly for the GB/AU/JP/etc. storefronts MeedyaDL users routinely run with.

### v3.5 — Summary

- **Behavioural changes that reach MeedyaDL**: 0 directly. All changes are confined to GAMDL's internal HTTP layer and improve music-video metadata coverage as a side effect.
- **CLI flag changes**: 0.
- **INI key changes**: 0.
- **Output-format regressions**: 0.
- **`GamdlFeature` gates that need to flip on/off at v3.5**: 0.

## Capability gate matrix — verified against 3.4 + 3.5

| `GamdlFeature` | v2.9.1 | v2.9.3 | v3.0 | v3.1 | v3.2 | v3.3 | v3.4 | v3.5 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `FetchExtraTags` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `NativeCodecPriority` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `WrapperM3u8Ip` | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `PlaylistFolderTemplate` | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `NoExceptionsFlag` (effective) | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |

The `is_version_at_least` thresholds in `gamdl_capabilities.rs::GamdlFeature::is_available_on` already produce the right answers for 3.4 and 3.5 — every gate is anchored at the version it was introduced/removed and the underlying CLI declarations didn't shift in 3.4 or 3.5. **No `GamdlFeature` enum addition or threshold adjustment needed.**

## Tool-versions ceiling bump

```diff
-maximum_tested_version = "3.3"
-recommended_version = "3.3"
+maximum_tested_version = "3.5"
+recommended_version = "3.5"
```

The bump is the entire MeedyaDL-side change set. `pip_version_spec()` will resolve to `gamdl>=2.9.1,<=3.5` after this lands, so the routine "Update GAMDL" path picks v3.5 directly. Above-ceiling users (4.0 etc.) continue to flow through `pip_target_spec()` → "Untested" amber badge.

## Floor analysis (no change)

`minimum_version = "2.9.1"` remains correct. Every capability MeedyaDL depends on still exists across the full window:

- Native `--song-codec-priority` for albums (2.9.1+): present.
- `--artist-auto-select` (2.9.1+): present.
- `structlog`-wrapped errors (3.0+): captured by `ERROR_PREFIX_REGEX` (`utils/process.rs:175`), which now also benefits from the cleaner stdout-stream provenance from 3.4+.
- `--wrapper-m3u8-ip` (3.1+): gated.
- `--no-exceptions` is a no-op (3.1+): gated.
- `--playlist-folder-template` (3.0+): gated.

## Conclusion

GAMDL v3.4 and v3.5 are **safe to admit to MeedyaDL's support window with no code changes**. The full audit-driven update is:

1. `tool-versions.toml`: `maximum_tested_version` 3.3 → 3.5, `recommended_version` 3.3 → 3.5, plus the inline change-set narrative.
2. `README.md`: GAMDL support range table updated to `2.9.1 – 3.5` and recommended to `3.5`.
3. `CLAUDE.md`: version-aware GAMDL dispatch section appends the 3.4 and 3.5 audit notes inline (mirroring the 3.2 / 3.3 paragraphs).
4. This audit document.

No `GamdlFeature` gates were added, removed, or re-thresholded. No regex was retuned. No reader-task logic was changed. The latent fix for the cosmetic `──── [Track N/M] Downloading "Title" ────` separator (was a no-op pre-3.4 because TrackInfo lines came from stderr; now fires reliably because they come from stdout) is a free improvement that ships the moment the user upgrades to GAMDL 3.4+.
