// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service modules containing the core business logic.
// =====================================================
//
// This module aggregates all service modules that implement the
// application's business logic. Services are called by `commands`
// handlers and encapsulate all interactions with:
//   - External processes (Python, pip, GAMDL CLI, FFmpeg, etc.)
//   - The filesystem (reading/writing config files, extracting archives)
//   - HTTP APIs (downloading releases from GitHub, checking PyPI versions)
//   - Tauri managed state (the download queue)
//
// Architectural pattern:
//   Command handlers (`commands/`) are thin IPC wrappers. They extract
//   arguments and managed state, then delegate to a service function here.
//   Services contain the actual logic: subprocess orchestration, error
//   handling, retry/fallback strategies, and state mutations.
//
// Module map:
//   services/
//   +-- python_manager.rs        -- Install/verify portable Python runtime
//   +-- gamdl_service.rs         -- Install/run GAMDL, parse subprocess output
//   +-- dependency_manager.rs    -- Install external tools (FFmpeg, mp4decrypt, ...)
//   +-- config_service.rs        -- Load/save settings, sync to GAMDL config.ini
//   +-- download_queue.rs        -- Queue management, concurrent downloads, fallback
//   +-- update_checker.rs        -- Version checking from PyPI and GitHub Releases
//   +-- cookie_service.rs        -- Browser cookie extraction and import
//   +-- login_window_service.rs  -- Embedded Apple Music login webview
//   +-- animated_artwork_service -- Animated cover art via MusicKit API
//   +-- apple_music_api.rs       -- Shared MusicKit JWT, URL parsing, API client
//   +-- metadata_tag_service.rs  -- Post-download metadata enrichment (codec + API tags)
//   +-- acoustid_service.rs      -- AcoustID fingerprinting via embedded Chromaprint (opt-in)
//   +-- replaygain_service.rs    -- ReplayGain loudness analysis via FFmpeg (opt-in)
//   +-- enhanced_lyrics_service  -- TTML → Enhanced LRC with word-by-word timestamps
//   +-- webvtt_service.rs        -- WebVTT subtitle generation from TTML/SRT/LRC (opt-in)
//   +-- rich_srt_service.rs      -- Rich SRT from TTML with styling + subtitle embedding
//   +-- musicbrainz_service.rs   -- MusicBrainz 3-tier recording lookup (opt-in)
//
// Thread safety:
//   Services that access shared state (like the download queue) use
//   `Arc<Mutex<T>>` for interior mutability. Tauri's `.manage()` stores
//   state behind an `Arc`, so services receive `State<'_, T>` which is
//   `Send + Sync` and safe to access from any async task.

/// Python runtime manager: download, install, and verify the portable
/// Python runtime from [python-build-standalone](https://github.com/indygreg/python-build-standalone)
/// GitHub releases.
///
/// Handles platform/architecture detection to select the correct release
/// asset, extraction via `utils::archive`, and verification by executing
/// `python3 --version`.
pub mod python_manager;

/// GAMDL CLI wrapper: install GAMDL via pip into the portable Python
/// environment, execute downloads as subprocesses, and parse stdout/stderr
/// into structured `GamdlOutputEvent` values for the frontend.
///
/// Uses `utils::process::parse_gamdl_output()` for line-by-line parsing
/// and emits events to the frontend via Tauri's event system.
pub mod gamdl_service;

/// Dependency manager: download and install external tool binaries
/// (`FFmpeg`, mp4decrypt, N_m3u8DL-RE, `MP4Box`) from their official
/// GitHub release pages.
///
/// Handles platform/architecture selection, archive download and
/// extraction via `utils::archive`, and permission setting via
/// `utils::archive::set_executable()`.
pub mod dependency_manager;

/// Settings and configuration service: load/save the application's
/// JSON settings (via tauri-plugin-store), and synchronise them to
/// GAMDL's `config.ini` format for CLI compatibility.
///
/// The sync step translates JSON keys (e.g., `outputFormat`) into
/// INI keys (e.g., `output_format`) that GAMDL's `--config-path`
/// flag can read.
pub mod config_service;

/// Download queue manager.
///
/// Manages the ordered queue of download requests with support for
/// concurrent execution, automatic quality-chain fallback retries
/// (e.g., AAC-HE -> AAC-LC), and per-item cancellation.
///
/// The queue state is stored as Tauri managed state
/// (`State<'_, QueueHandle>`) and accessed from both command handlers
/// and background download tasks.
pub mod download_queue;

/// Update checker.
///
/// Queries `PyPI` (for GAMDL/gamdl version) and GitHub Releases (for
/// Python, `FFmpeg`, mp4decrypt, etc.) to determine whether newer
/// versions are available, and provides an upgrade function for GAMDL.
pub mod update_checker;

/// Browser cookie extraction service: detects installed browsers,
/// extracts Apple Music cookies using the `rookie` crate, converts
/// them to Netscape format, and saves them to the app data directory.
///
/// Handles platform-specific concerns: macOS Keychain access for
/// Chromium browsers, Full Disk Access detection for Safari, Windows
/// DPAPI decryption, and Linux D-Bus Secret Service integration.
pub mod cookie_service;

/// Embedded Apple Music login window service.
///
/// Manages a secondary webview window where users can sign in to Apple
/// Music directly. Uses Tauri's native `cookies_for_url()` API to extract
/// authentication cookies (including `HttpOnly`) from the webview after
/// login, converts them to Netscape format, and saves them for GAMDL.
///
/// Addresses the scenario where users have no existing browser cookies
/// to auto-import. The login window loads `https://music.apple.com` and
/// auto-detects successful authentication via the `media-user-token` cookie.
pub mod login_window_service;

/// Animated artwork (motion cover art) download service: queries the
/// Apple Music catalog API for animated album covers (`editorialVideo`)
/// and downloads them via `FFmpeg` HLS-to-MP4 conversion.
///
/// Saves `FrontCover.mp4` (square, 1:1) and `PortraitCover.mp4`
/// (portrait, 3:4) alongside downloaded album files. Requires user-provided
/// `MusicKit` credentials (Team ID, Key ID, private key in OS keychain).
pub mod animated_artwork_service;

/// Shared Apple Music (`MusicKit`) API client and authentication module.
///
/// Provides JWT generation, URL parsing, keychain access, and the enriched
/// catalog API call that returns album metadata (ISRC, UPC, genre, advisory,
/// artist IDs) plus animated artwork URLs in a single request.
///
/// Used by: `animated_artwork_service`, `metadata_tag_service`
pub mod apple_music_api;

/// Post-download metadata enrichment service.
///
/// Injects comprehensive custom metadata into downloaded M4A files.
/// Codec tags (isLossless, `SpatialType`), source tags (`SourceStore`,
/// `EncodeSource`, `ChannelConfig`), and Apple Music API metadata (ISRC,
/// UPC, genre, advisory, artist IDs, artwork URLs) are written as freeform
/// atoms. Channel detection uses ffprobe; API metadata requires `MusicKit`
/// credentials configured in settings.
pub mod metadata_tag_service;

/// `AcoustID` fingerprinting service.
///
/// Generates Chromaprint audio fingerprints using the embedded
/// rusty-chromaprint library (pure Rust) and looks up `AcoustID`
/// identifiers via the acoustid.org web service. Writes `Acoustid Id`
/// and `Acoustid Fingerprint` freeform atoms to M4A files. Opt-in
/// feature with no external binary dependencies.
///
/// Used by: `download_queue` (post-download enrichment, when `acoustid_enabled`)
pub mod acoustid_service;

/// `ReplayGain` loudness analysis service.
///
/// Analyses audio loudness using `FFmpeg`'s EBU R128 filter and writes
/// non-destructive `ReplayGain` metadata tags (`replaygain_track_gain`,
/// `replaygain_track_peak`). Enables volume normalisation in media
/// players that support `ReplayGain`. Opt-in feature.
///
/// Used by: `download_queue` (post-download enrichment, when `replaygain_enabled`)
pub mod replaygain_service;

/// Enhanced LRC lyrics conversion service.
///
/// Post-processes Apple Music TTML lyrics files to produce Enhanced LRC
/// with word-by-word synchronized timestamps. Parses TTML XML using
/// `roxmltree`, extracts `<span>` word timing from `itunes:timing="Word"`
/// documents, and generates backward-compatible Enhanced LRC format with
/// inline `<mm:ss.xx>` word timestamps.
///
/// Also embeds the resulting Enhanced LRC in M4A/M4V audio metadata via
/// the `©lyr` atom. Songs without word-level timing in their TTML
/// gracefully fall back to standard line-level LRC.
///
/// Used by: `download_queue` (post-download enrichment, when `enhanced_lrc` enabled)
pub mod enhanced_lyrics_service;

/// Pre-flight health check service.
///
/// Provides reusable health check functions for internet connectivity,
/// cookie validation, and wrapper service health. Called by
/// `download_queue.rs` before queue processing begins, and by
/// `commands/settings.rs` for on-demand validation.
///
/// Each check returns `Option<PreflightWarning>`:
/// - `None` = check passed
/// - `Some(warning)` = issue detected, shown as a persistent toast
///
/// Used by: `download_queue` (pre-flight checks), `commands/settings` (cookie validation)
pub mod health_check_service;

/// Crash report service.
///
/// Provides CRUD operations for crash report JSON files stored in
/// `{app_data_dir}/crashes/`. Supports listing, reading, deleting,
/// exporting (as Markdown), and saving frontend error reports. Also
/// provides automatic cleanup of reports older than 30 days.
///
/// Used by: `commands/crash_reports` (IPC handlers), `lib.rs` (startup cleanup)
pub mod crash_report_service;

/// WebVTT subtitle generation service.
///
/// Generates WebVTT (`.vtt`) subtitle files from existing lyrics sidecars
/// (TTML, SRT, or LRC). Source priority: TTML (richest timing data),
/// SRT (has start+end times), LRC (start times only). Opt-in feature
/// controlled by the `generate_webvtt` setting.
///
/// Used by: `download_queue` (post-download enrichment Step 2c, when `generate_webvtt` enabled)
pub mod webvtt_service;

/// Rich SRT subtitle generation and embedding service.
///
/// Generates format-rich SRT files from TTML or WebVTT sources that
/// preserve styling (bold, italic, underline, colours) using HTML-like
/// tags. Source priority: TTML (richest), WebVTT (also supports styling).
/// Also provides subtitle embedding into MP4/M4A/M4V containers as
/// freeform atoms for future multi-service support.
///
/// Used by: `download_queue` (post-download enrichment Steps 2d and 2e)
pub mod rich_srt_service;

/// ASS (Advanced SubStation Alpha) subtitle generation service.
///
/// Generates ASS subtitle files from TTML or WebVTT sources with full
/// styling support: colours (BGR format), bold, italic, underline,
/// dynamic positioning (`\pos`), and background vocal styles. Reuses
/// TTML style resolution from `rich_srt_service`.
///
/// Used by: `download_queue` (post-download enrichment Step 2f, when `generate_ass` enabled)
pub mod ass_subtitle_service;

/// MusicBrainz recording lookup service.
///
/// Queries the MusicBrainz database to discover recording metadata,
/// cross-platform URLs, and music video links. Uses a 3-tier priority
/// chain: (1) Apple Music URL search in MB external links, (2) ISRC code
/// search, (3) AcoustID recording ID direct lookup. Serves as a fallback
/// for music video discovery (no MusicKit credentials needed) and
/// groundwork for cross-platform song discovery.
///
/// Used by: `download_queue` (post-download enrichment Step 6b, when `musicbrainz_lookup` enabled)
pub mod musicbrainz_service;

/// MediaInfo CLI service — accurate codec detection via `mediainfo --Output=JSON`.
///
/// Provides definitive Dolby Atmos detection via `Format_AdditionalFeatures: "JOC"`.
/// Falls back to ffprobe when MediaInfo is not installed (optional dependency).
///
/// Used by: `metadata_tag_service` (codec detection in enrichment Step 1)
pub mod mediainfo_service;

/// Download history persistence service.
///
/// Records completed and failed downloads to `{app_data_dir}/history.json`
/// for user review. Provides list, search, and clear operations. Maximum
/// 1000 entries; oldest are trimmed on save.
///
/// Used by: `download_queue` (post-completion/error recording), `commands/history` (IPC)
pub mod history_service;

/// API field audit service — diagnostic tool for discovering new API fields.
///
/// Fetches a real album from the Apple Music API and diffs the raw JSON
/// response against known tag definitions in `tags.toml`. Reports new/
/// unknown fields for human review. Does NOT auto-embed unknown fields.
pub mod api_audit_service;

/// Generic pip-based engine management service.
///
/// Provides install, version-check, and uninstall functions for any Python
/// package that MeedyaDL manages as a download engine. Generalises the
/// pattern from `gamdl_service.rs` so new pip-based engines (votify,
/// ofscraper, yt-dlp) can be managed with zero new service code.
///
/// Used by: `commands/dependencies` (IPC handlers for engine install/check)
pub mod pip_engine_service;

/// End-to-end integration tests for the enrichment pipeline services.
///
/// Tests the subtitle/lyrics services with actual files on disk: creates
/// temp directories with sample TTML/SRT/LRC/VTT sources, calls the
/// directory-level public APIs, and verifies output files exist with
/// correct content. Covers Rich SRT, WebVTT, Enhanced LRC, and ASS
/// generation, including Unicode filename handling and source priority.
///
/// Engine registry — runtime query layer for `engines.toml`.
///
/// Provides typed access to engine and platform configuration compiled
/// into the binary. The download queue, dependency manager, and frontend
/// all query this registry instead of hardcoding engine knowledge.
///
/// Used by: `commands/system` (IPC), `download_queue` (engine resolution)
pub mod engine_registry;

/// Only compiled in test mode (`cargo test`).
#[cfg(test)]
mod integration_tests;
