# GAMDL v3.7.4 Compatibility Audit

**Date**: 2026-06-14
**GAMDL release audited**: 3.7.4 (released 2026-06-12)
**Diff range**: `3.7.3..3.7.4` (5 commits, 7 files)
**Predecessor audit**: [`gamdl-v3.7.2-v3.7.3-audit.md`](./gamdl-v3.7.2-v3.7.3-audit.md)
**Tracking issue**: #925

## TL;DR

Pure upstream reliability patch — five commits, all internal to GAMDL's HLS pipeline, token-extraction layer, and exception-formatting helper. **No MeedyaDL code change is required.** Bump `maximum_tested_version` and `recommended_version` in [`tool-versions.toml`](../../src-tauri/tool-versions.toml) from `"3.7.3"` → `"3.7.4"`. Same shape as v3.3 (playlist fix), v3.5 (iTunes lookup fix), v3.5.1 (music-video 403 fix), and v3.5.2 (pagination + edge-host migration fix) — zero-code-change admission to the support window.

## Methodology

Identical to the v3.7.2/v3.7.3 audit pattern:

1. Inspected the upstream diff via `gh api repos/glomatico/gamdl/compare/3.7.3...3.7.4` and confirmed five commits, seven files.
2. For each commit, identified the upstream file/lines touched and cross-referenced against MeedyaDL's integration surface:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`, INI emission gates)
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`, `sanitize_ini_value`)
   - `src-tauri/src/services/download_queue.rs` (stdout/stderr readers, `extract_python_exception`, completion task)
   - `src-tauri/src/services/gamdl_capabilities.rs` (`GamdlFeature` enum + `supports()`)
   - `src-tauri/src/services/gamdl_service.rs` (`install_gamdl`, `build_gamdl_command`, tool-path injection)
   - `src-tauri/src/utils/process.rs` (regexes: `TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, `PYTHON_EXCEPTION_REGEX`; classifiers: `classify_error`, `is_io_error`, `is_python_traceback_noise`, `humanise_codec_skip_line`, `is_storefront_mismatch_error`, `is_media_not_streamable_error`)
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window)
3. Verified no commit touches the upstream CLI surface, INI key set, stdout/stderr log shape, exception class hierarchy, or HTTP request/response shape that MeedyaDL inspects.

## v3.7.4 — `3.7.3..3.7.4` change set

Five commits:

| Commit | Subject | Files |
| --- | --- | --- |
| `a9e7538` | Add method to switch m3u8 master URL to default and update playback handling | `gamdl/interface/song.py` |
| `b66c06a` | Fix token extraction | `gamdl/api/apple_music.py` |
| `fb143ad` | Cover art timeout | `gamdl/interface/base.py` |
| `69c2a8a` | Refactor `GamdlApiResponseError` to accept Any type for content and improve message formatting | `gamdl/api/exceptions.py` |
| `b0c5335` | Bump version to 3.7.4 | `gamdl/__init__.py`, `pyproject.toml`, `uv.lock` |

### Finding 3.7.4-A — m3u8 master URL default-variant rewrite

`gamdl/interface/song.py` gains a private helper `_switch_m3u8_master_url_to_default(url)` that applies a regex rewrite `(P\d+)_[^/]+(\.m3u8)` → `\1_default\2`. Both upstream call sites — `_get_m3u8_from_playback` and `_get_m3u8_master_url_from_metadata` — now route through it. `_get_m3u8_from_playback` is further refactored into a logging-aware single-URL fetcher that emits a structlog `m3u8_master_url=…` debug event when `--log-level Debug` is set.

**Why**: forces GAMDL onto the canonical default-variant playlist for each track even when Apple's API returns a variant-suffixed master URL.

**MeedyaDL surface impact: none.** MeedyaDL never inspects the m3u8 master URL — the only repository references to `m3u8_master_url` are *comments* (in `download_queue.rs:3576` and `models/settings.rs:1526`) describing GAMDL's debug-log key for the user's information. The rewrite is internal to GAMDL's HLS download path; we observe the resulting audio file outputs, not URLs. The new debug-log line only fires at `--log-level Debug` (MeedyaDL's `gamdl_log_level` defaults to `INFO`) and doesn't match any of MeedyaDL's regex parsers in `utils/process.rs`.

### Finding 3.7.4-B — Token extraction regex hardening

`gamdl/api/apple_music.py` updates two regexes inside GAMDL's own developer-token-fetch path:

* Line 96: `r"/(assets/index-legacy[~-][^/\"]+\.js)"` → `r"/(assets/index[~-][^/\"]+\.js)"`. Apple renamed the home-page bundle from `index-legacy-*.js` to `index-*.js`; the previous regex required the `-legacy` suffix and broke after Apple's rename.
* Line 119: token regex tightened from `'(?=eyJh)(.*?)(?=")'` to `r'"(eyJ[A-Za-z0-9\-_]+\.eyJ[A-Za-z0-9\-_]+\.[A-Za-z0-9\-_]+)"'`. The new pattern requires the captured string to be a structurally valid JWT (three base64url segments separated by `.`) enclosed in double-quotes — stricter, less prone to false matches.

**MeedyaDL surface impact: none.** MeedyaDL doesn't shell `get_token()` — the developer-token resolution chain in `apple_music_api.rs::TokenSource` is entirely independent of GAMDL's internal token extractor. MeedyaDL's three-tier chain (user-provided MusicKit JWT > compile-time `APPLE_DEVELOPER_TOKEN` > web-player keychain) is reached without ever touching GAMDL's regex. GAMDL's own developer-token use is internal to its subprocess and feeds operations MeedyaDL doesn't observe directly.

### Finding 3.7.4-C — Cover-art HTTP client hardening

`gamdl/interface/base.py::get_cover_bytes` changes:

* `httpx.AsyncClient()` → `httpx.AsyncClient(timeout=30.0)` — adds an explicit 30-second timeout where the previous version inherited httpx's default (unbounded).
* `client.get(cover_url)` → `client.get(cover_url, follow_redirects=True)` — explicitly follows redirects.

**Why**: prior versions could hang indefinitely on slow Apple CDN responses and silently fail when Apple's CDN returned a 30x redirect.

**MeedyaDL surface impact: none.** MeedyaDL's downstream cover-art pipeline (`cover_art_fallback.rs`, `metadata_tag_service.rs::apply_cover_art_rename`, `animated_artwork_service.rs`) observes the resulting cover-image *files* on disk — the HTTP path that produced them is opaque. The change is a reliability win that may reduce sporadic "missing cover art" outcomes the user sees today, but doesn't change the file format / filename / output path. MeedyaDL's own static cover-art fallback chain (#756) — which exists precisely to handle the case where GAMDL fails to write the cover bytes — is unaffected.

### Finding 3.7.4-D — `GamdlApiResponseError.content` accepts `Any`

`gamdl/api/exceptions.py::GamdlApiResponseError` signature change:

* `content: str | None` → `content: Any | None`
* Non-string content is now JSON-serialised via `json.dumps(content)` when assembling the error message, with a `TypeError` fallback to `str(content)`.

**Why**: upstream Apple Music API responses sometimes return JSON objects (not string bodies) for error payloads, and the previous version called `str()` on a dict which produced Python-repr output (`"{'errors': [{'code': '40404', ...}]}"`) that's less greppable than canonical JSON.

**MeedyaDL surface impact: none — and arguably positive.** The user-facing string shape of `GamdlApiResponseError` is preserved: `"<message> (Status code: N): <content_text>"`. The only change is that `content_text` is now `json.dumps(dict)` instead of `str(dict)`. Crucially:

* MeedyaDL's `process::is_storefront_mismatch_error` (case-insensitive substring `"resource not found"`) — `json.dumps({"errors": [{"detail": "Resource Not Found"}]})` still contains the literal `Resource Not Found`. Verified.
* MeedyaDL's `download_queue::extract_python_exception` (regex matching `gamdl.api.exceptions.GamdlApiResponseError`) keys on the *class name in the traceback*, not on the content format. Unchanged.
* MeedyaDL's `process::PYTHON_EXCEPTION_REGEX` keys on the exception's leading class-name prefix, not the content. Unchanged.

If anything, `json.dumps` yields a more parseable representation than the previous `str(dict)` — Python-repr's apostrophe-wrapped keys (`{'errors': [...]}`) are valid Python but not valid JSON, so MeedyaDL's content matchers actually benefit from the change.

### Finding 3.7.4-E — Version bump

`gamdl/__init__.py`, `pyproject.toml`, `uv.lock` — `3.7.3` → `3.7.4`. Pure metadata.

**MeedyaDL surface impact: none.** `tool-versions.toml` bump only.

## MeedyaDL surface impact: none

Verified by inspection that v3.7.4:

- **Adds no new CLI flag** — `gamdl_options.rs::to_cli_args()` and `config_service.rs::ini_*` sections need no new fields or gates.
- **Removes no CLI flag** — every flag MeedyaDL emits on v3.7.3 remains valid on v3.7.4. No `GamdlFeature` gate threshold needs to advance.
- **Adds no INI key** — `config_service.rs::settings_to_ini` covers the same key set.
- **Changes no output-line format** — `TRACK_INFO_V2_REGEX`, `ERROR_PREFIX_REGEX`, and the structlog `[LEVEL HH:MM:SS]` prefix all match identically on v3.7.4 output. The new debug-mode `m3u8_master_url=…` line is the only new emission and it doesn't trip any existing matcher (MeedyaDL defaults to `INFO`).
- **Adds no exception class** — `PYTHON_EXCEPTION_REGEX` and `extract_python_exception` continue to recognise the same set of `gamdl.*Error` classes. `GamdlApiResponseError` retains its class name and `(Status code: N)` substring; only the rendering of `content` changed and the substrings MeedyaDL keys on are preserved.
- **Changes no log level / stream** — still structlog stdout per v3.4's `CustomOutputWriter`.
- **Changes no HTTP request shape** that MeedyaDL inspects.
- **Changes no `--database-path` semantics** — feature MeedyaDL doesn't expose anyway.

## Recommended `tool-versions.toml` change

```toml
[gamdl]
minimum_version       = "2.9.1"   # unchanged
maximum_tested_version = "3.7.4"  # was 3.7.3
recommended_version    = "3.7.4"  # was 3.7.3
```

Plus an audit-trail comment block mirroring the v3.7.2/v3.7.3 entries, pointing back at this audit doc and at issue #925.

## No GitHub follow-up issues

The four findings are all "zero-code-change" admissions. No `for consideration` or `enhancement` issues need to be opened.

If a future GAMDL release does break MeedyaDL's surface, the lesson the v3.7.4 audit reinforces — also recorded in the `project_gamdl_release_cadence.md` memory — is that GAMDL ships fast, the bug-fix-only releases dominate, and the audit cost per release is low (~30 minutes of cross-referencing). Continue running the same checklist on every tag.
