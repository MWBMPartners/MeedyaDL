// Copyright (c) 2026 MeedyaSuite
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

/// Version-aware capability flags for the installed GAMDL release.
///
/// Tracks which CLI options / INI keys the currently installed GAMDL
/// version supports so we can emit a backwards-compatible command line
/// across GAMDL `>= 2.9.1` (oldest we still support) through the
/// current v3.x line.
pub mod gamdl_capabilities;

/// Dependency manager: download and install external tool binaries
/// (`FFmpeg`, mp4decrypt, N_m3u8DL-RE, `MP4Box`) from their official
/// GitHub release pages.
///
/// Handles platform/architecture selection, archive download and
/// extraction via `utils::archive`, and permission setting via
/// `utils::archive::set_executable()`.
pub mod dependency_manager;

/// Package-manager abstraction: attribute a tool binary to its owning system
/// package manager (Homebrew, MacPorts, pipx, Scoop, apt, dnf, snap) and route
/// updates through it — directly for no-elevation managers, via the #997
/// `sudo -n`/`pkexec` tiers for root-requiring ones. Generalises the
/// previously Homebrew-only detect/attribute/update machinery in
/// `dependency_manager`.
pub mod package_manager;

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

/// Companion-download supervisor.
///
/// Wraps a single GAMDL companion-tier child process with soft-error
/// detection (#500), an idle-timeout watchdog (#505),
/// post-processing detection (#503) and `kill_on_drop` so timeouts
/// in `download_queue` don't leak zombie GAMDL processes (#501).
pub mod companion_supervisor;

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
/// Saves `FrontCover.mp4` (square, 1:1) and `FrontCoverPortrait.mp4`
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

/// Static cover-art fallback chain (#756).
///
/// When GAMDL fails to write the requested static cover format
/// (typically `cover_format = raw`, where the upstream cover-bytes
/// fetch raises a Python traceback), this service walks a fallback
/// chain — RAW → PNG → JPEG — by fetching the static artwork URL
/// from `AlbumMetadata.artwork_url_template` directly, sidestepping
/// GAMDL entirely. Idempotent: skips when the cover is already
/// present and non-trivially sized.
pub mod cover_art_fallback;

/// Best-cover-art picker (M9-3).
///
/// Fans out to every supported music platform in parallel, scores
/// each platform's candidate by pixel area, and returns the
/// highest-resolution winner — with Apple Music as the tie-breaker
/// on equal-pixel matches. The cross-platform design is forward-
/// looking: today Apple Music almost always wins, but as Tidal and
/// Bandcamp adapters land their typically-higher-than-Spotify
/// resolutions will participate in the race without code change.
///
/// Opt-in via `AppSettings::best_cover_art_enabled`. Queue
/// integration lives in M9-4.
pub mod best_cover_art_service;

/// Anti-ban runtime for Spotify downloads (M9-4).
///
/// Pure-function throttle math (`compute_playback_throttle_delay`,
/// `compute_inter_track_delay`) plus a persisted `DailyCapCounter`
/// with local-midnight rollover. Companion to the
/// `models::spotify_anti_ban::AntiBanSettings` struct.
///
/// The dispatch gate that consumes these primitives lives in the
/// IPC layer — see `commands::spotify_anti_ban`.
pub mod spotify_anti_ban;

/// Python traceback diagnostic capture (#758).
///
/// Scans GAMDL stdout/stderr buffers at process completion for
/// recurring Python tracebacks, deduplicates identical groups, and
/// writes a diagnostic report under the existing crash-report
/// infrastructure. Fires on **any** download where traceback noise
/// was observed — even successful ones — because some upstream bugs
/// (cover-bytes fetch, syllable-lyrics race, music-video relations)
/// raise tracebacks but don't fail the download. The forensic record
/// gives us aggregated visibility into otherwise-silent issues.
pub mod traceback_diagnostic;

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

/// BPM (tempo) analysis service.
///
/// Detects BPM from audio files using bpm-analyzer crate (wavelet decomposition)
/// and writes metadata tags (tmpo for M4A, TBPM for MP3, BPM for FLAC).
///
/// Used by: `download_queue` (post-download enrichment, when `bpm_analysis_enabled`)
pub mod bpm_service;

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

/// Lyricsfile (`.lyrics`) YAML sidecar generation service (#596).
///
/// Wraps the shared `meedya_lyrics::Lyricsfile` upstream crate (from
/// MeedyaSuite-core#34): consumes the TTML sidecar GAMDL emits during
/// download, runs it through `Lyricsfile::from_ttml` to preserve
/// word-level timing, then writes a `.lyrics` YAML sidecar alongside
/// the audio file. Idempotent — won't clobber a file that already
/// exists (preserves user edits made in LRCGET).
///
/// Used by: `download_queue` (post-download enrichment Step 2g, when
/// `generate_lyricsfile` enabled).
pub mod lyricsfile_service;

/// Music video subtitle / caption extraction service (#483).
///
/// After a music video download lands, probes the output file for
/// subtitle / closed-caption streams (via ffprobe) and extracts each one
/// to a sidecar file (`.vtt` / `.srt`) next to the video. Also mirrors any
/// existing song lyrics (TTML/LRC/SRT/VTT/ASS) from the album directory
/// alongside the music video when the pair is a companion match.
///
/// Used by: `download_queue::download_music_video_by_url` (fire-and-forget
/// post-processing after GAMDL finishes).
pub mod music_video_subtitle_service;

/// Music-video cover-sidecar embedding (#533 / #569).
///
/// Embeds the `.jpg` / `.png` cover thumbnail GAMDL writes next to
/// each music video into the MP4 container as a `covr` atom, then
/// deletes the sidecar. Cleans up the library while preserving the
/// thumbnail as an embedded poster frame that every modern player
/// renders directly. Wired into the same MV post-download loop as
/// `music_video_subtitle_service`.
pub mod music_video_cover_embed;

/// In-process `AppSettings` cache (#690).
///
/// Eliminates redundant disk reads on the queue hot path. Lazy-
/// populated on first access, refreshed by the `save_settings` IPC
/// after each write, and read by `load_settings_for_queue` so every
/// caller path benefits without per-site changes.
pub mod settings_cache;

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

/// Download Index — SQLite-backed indexed cache (#875 EPIC A).
///
/// M1 scaffolding only: opens (or creates) `meedyadl.db` in the app
/// data dir, applies the v1 schema (7 tables: downloads, recordings,
/// recording_downloads, manifests, activity_events, known_tracks,
/// schema_version), runs the migration runner. **Not yet wired into
/// any user-facing flow** — read/write paths are deferred to M1b/M2.
///
/// `.meedyadl` manifests on disk remain the source of truth; this DB
/// is rebuildable from them via `scan_folder_for_manifests`.
pub mod download_index;

/// Profile Bundle — `.meedyabundle` export/import format (#876 EPIC B).
///
/// P1 scaffolding only: pure-Rust ZIP-based format definition +
/// (de)serialiser primitives. **No IPC, no Settings UI, no credential
/// encryption yet** — those land in P2 / P3 / P4 of the EPIC. The
/// format is forward-compatible via `meta.json.contents` enumeration
/// of OPTIONAL sections.
pub mod profile_bundle;

/// Multi-service engine scaffolding (#884, cherry-picked from
/// `prep/expanded-services-groundwork`).
///
/// Stub service modules + a dispatch layer for the planned M8 / M9
/// / M10 milestones. None of them activate any new behaviour today
/// — every public function returns a typed "not yet implemented"
/// error — but the module tree is in place so the actual engine
/// implementation work has somewhere to land.
///
/// * `service_dispatch` — `ServiceOutputEvent` enum + the
///   per-service gate (`is_service_implemented`,
///   `is_service_remotely_enabled`).
/// * `bbc_iplayer_service` — M8 stub (get_iplayer + yt-dlp fallback).
/// * `spotify_service` — M9 stub (votify).
/// * `youtube_service` — M10 stub (yt-dlp).
pub mod service_dispatch;
pub mod bbc_iplayer_service;
pub mod spotify_service;
pub mod youtube_service;

/// Version-aware capability flags for the installed `votify` CLI (#101).
///
/// Mirrors [`gamdl_capabilities`]. Lands in PR M9-1 as scaffolding —
/// the [`VotifyFeature`] enum carries a placeholder variant until PR
/// M9-2's audit produces real version-conditional gates. See the module
/// docs for the full lifecycle.
pub mod votify_capabilities;

/// External queue watchdog (#818).
///
/// Top-level tokio task — owned by the Tauri runtime, NOT spawned by
/// the queue processor — that polls every 60 s and escalates queue
/// items whose progress has been bit-identical for >10 min (WARN)
/// or >20 min (transition to Error + release queue slot). Independent
/// of the queue processor's task tree so a hang in the parent can't
/// kill the watchdog. Recovers from the #815 silent-hang failure
/// class without needing to identify each specific hang surface.
pub mod queue_watchdog;

/// Odesli (song.link) API client (#295 Phase A).
///
/// Cross-platform URL discovery — given an Apple Music URL, returns
/// matching URLs for Spotify / YouTube / Tidal / Deezer / Amazon
/// Music / SoundCloud / Bandcamp / Pandora / etc. Rate-limited per-
/// process at 1 req/~1.1 s (well below the 10 req/min free tier).
/// Phase A returns the URLs; integration into the manifest and the
/// MeedyaMeta:*Url freeform atoms is wired in the enrichment task.
pub mod odesli_service;

/// Enrichment gap detection (#759 Phase 1).
///
/// Inspects an album directory + its `manifest.meedyadl` and reports
/// which enrichment stages (ReplayGain, AcoustID, MusicBrainz,
/// Enhanced LRC, animated artwork, …) are missing. Hybrid signals:
/// manifest record when present, file-based heuristics for sidecar
/// outputs (LRC / SRT / VTT / ASS / FrontCover.mp4 / Cover.{jpg,png}).
/// Stages whose output is tag-embedded with no manifest record
/// return "unknown" — Phase 2 adds ffprobe-based tag detection.
/// Runner that re-executes missing stages is deferred to Phase 2.
pub mod enrichment_gaps;

/// Opt-in diagnostic bundle composer (#572 Phase 1).
///
/// Composes a redacted Markdown report covering system state at
/// capture time (version info, settings snapshot, recent activity-log
/// slice, output-dir structure) plus a pre-filled GitHub issue URL.
/// Privacy-first: no credentials, no file contents, no auto-submit.
/// Username paths are redacted to `/Users/{user}/`.
pub mod diagnostic_bundle;

/// Lifetime download analytics (#464).
///
/// Aggregates `history.json` into roll-up stats: total downloads,
/// success rate, codec distribution, top artist / album, last-7-day
/// activity. Computed on-demand (no separate stats file) so
/// `history_service` remains the single source of truth for terminal
/// download records. Pure function `compute_lifetime_stats` is
/// covered by unit tests.
pub mod stats_service;

/// Snapshot + restore for essential state (#466).
///
/// Bundles `settings.json`, `queue.json`, and `history.json` into a
/// timestamped directory under `{app_data_dir}/backups/`. Keeps the
/// last 10 backups (constant inside the module), pruning older ones
/// after each successful write. Restore is opt-in via the Settings >
/// Tools UI; the IPC also exposes list / delete commands. Snapshots
/// are flat directories of plain JSON for trivial inspection.
pub mod backup_service;

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

/// Clipboard monitoring service.
///
/// Reads text from the system clipboard for the clipboard monitoring
/// feature. The frontend polls this service to detect when the user
/// copies a supported URL (e.g., Apple Music) to the clipboard.
///
/// Used by: `commands/clipboard` (IPC handler)
pub mod clipboard_service;

/// End-to-end integration tests for the enrichment pipeline services.
///
/// Tests the subtitle/lyrics services with actual files on disk: creates
/// temp directories with sample TTML/SRT/LRC/VTT sources, calls the
/// directory-level public APIs, and verifies output files exist with
/// correct content. Covers Rich SRT, WebVTT, Enhanced LRC, and ASS
/// generation, including Unicode filename handling and source priority.
///
/// Engine runner — service-agnostic subprocess spawning and streaming.
///
/// Provides a generic `run_engine()` function and `EngineCommandBuilder`
/// trait for running download engine subprocesses with consistent I/O
/// streaming, progress event emission, and error handling. Engine-specific
/// command builders (GAMDL, Votify, yt-dlp, get_iplayer) implement the
/// trait; the runner handles the common subprocess lifecycle.
///
/// Used by: `download_queue` (subprocess execution), `gamdl_service` (GAMDL adapter)
pub mod engine_runner;

/// Engine registry — runtime query layer for `engines.toml`.
///
/// Provides typed access to engine and platform configuration compiled
/// into the binary. The download queue, dependency manager, and frontend
/// all query this registry instead of hardcoding engine knowledge.
///
/// Used by: `commands/system` (IPC), `download_queue` (engine resolution)
pub mod engine_registry;

/// Service-agnostic metadata provider trait (#351).
/// Enables Apple Music, iTunes, Spotify, YouTube etc. to provide metadata
/// through a common interface for the enrichment pipeline.
pub mod metadata_provider;

/// Service status — fetches remote service enable/disable configuration.
///
/// Checks whether services (Apple Music, Spotify, etc.) are currently
/// available. Used to disable UI elements for temporarily unavailable services.
pub mod service_status;

/// Remote feature-availability client (#1071).
///
/// Resolves per-feature availability verdicts from MWBM-IntAppsAPI via a
/// three-tier chain — in-memory snapshot -> sticky disk cache -> compiled
/// all-enabled defaults — mirroring `service_status`'s shape. Neither
/// `current()` nor `refresh()` can fail: an unreachable server keeps the
/// last known verdicts and emits a single activity-log line, never a toast
/// or a frontend error. A client-side sanity floor refuses any instruction
/// to disable the flag fetcher or the updater. Inert (zero network calls)
/// in builds without the `INTAPPS_*` compile-time credentials.
///
/// **Backend only** — nothing gates on it yet; enforcement call sites land
/// separately.
pub mod feature_flag_service;

/// Smart Download — cross-platform quality optimisation service.
///
/// Analyses content availability across services to recommend the best
/// quality source for a given album/track.
pub mod smart_download;

/// Smart manifest-driven retry planner (#667).
///
/// Reads the `manifest.meedyadl` written at end-of-pipeline, diffs the
/// expected track set against on-disk audio files, and returns a
/// per-track URL list so a retry only re-fetches the tracks that
/// actually failed (versus today's "re-run the whole album URL" path).
pub mod smart_retry_planner;

/// Pre-queue duplicate detector (#510).
///
/// When an Apple Music artist URL is queued with multiple
/// `artist_auto_select_multi` modes, fetches the album list for each mode,
/// applies the user's preference hierarchy (default: main > singles >
/// compilations > live > top-songs), and returns a plan indicating which
/// track URLs to skip in each mode so that any given song is downloaded
/// exactly once. Operates on track identity (song_id / ISRC) only —
/// companion format downloads (ALAC / Atmos / AAC, etc.) are unaffected.
pub mod duplicate_detector;

/// Persistent on-disk activity log writer (#541).
///
/// Streams every `ActivityLogEvent` to a daily-rotating, append-only
/// text file at `{app_data_dir}/logs/activity-YYYY-MM-DD.log`. Runs as
/// a background Tokio task fed via an unbounded channel so the emit
/// hot path never blocks on disk I/O. Retained for 7 days by
/// `clear_old_logs()` in `lib.rs`.
pub mod activity_log_writer;

/// Engine filename-safety contract (#551).
///
/// Design-review-only trait every new engine integration (votify,
/// yt-dlp, get_iplayer, ...) is expected to implement. Default
/// conformance checks prove each engine's no-album / no-collection
/// fallback templates cannot reproduce the #527 / #531 / #537 class
/// of bug (punctuation-only filenames, `[Unknown]`-sentinel folders,
/// stable-ID-less dedup collisions). GAMDL's music-video fallback is
/// bundled as the first conformance example.
pub mod filename_safety;

/// Per-item progress stage registry (#712-followup / Phase 3.5b).
///
/// Single source of truth for the stages an item passes through after
/// primary GAMDL exit and before the completion task marks it `Complete`.
/// Replaces the 8 scattered `PROGRESS_*_STAGE: f32` constants and the
/// closure-local `set_label(...)` calls inside the enrichment task.
/// Used by both the enrichment task and the companion task so the
/// per-item progress bar caption stays in sync regardless of which
/// task is active.
pub mod progress_stages;

/// Legacy sibling-folder merge for pre-#528 downloads (#789).
/// Detects `Album/` + `Album [Explicit]/` pairs left over from
/// older versions and offers a three-phase (detect → preview →
/// execute) merge to consolidate them into the single
/// post-#528 layout.
pub mod legacy_folder_merge;

/// Output-directory integrity scan (#537 chunk B).
///
/// Walks the user's configured output directory looking for historic
/// damage from pre-v1.6 broken builds: `-.mp4` / `-.jpg` filenames
/// from the empty-tag MV pipeline, `[Unknown]/` folder segments, and
/// zero-byte fixed-name covers (`FrontCover.mp4`,
/// `PortraitCover.mp4`, `ArtistSpotlightCover.mp4`) from interrupted
/// HLS downloads. User-initiated via Settings → Advanced →
/// Diagnostics; quarantine action lands as a follow-up.
pub mod integrity_scan;

/// First-launch migration from the pre-2026-07-24 bundle identifier
/// (`io.github.meedyadl` -> `com.meedyasuite.meedyadl`).
///
/// Copies (never moves) settings/queue/history/logs/crashes/python/tools
/// from the old OS app-data directory into the new one, and migrates the
/// handful of OS-keychain entries (MusicKit private key, web player
/// developer token, dev-access sentinel) forward. Idempotent via a marker
/// file; every step is individually best-effort so a failure never blocks
/// startup. Called once from `lib.rs::run()`'s `.setup()` hook, before
/// anything else touches the new app-data directory.
pub mod bundle_migration;

/// macOS self-relocation: "tidy MeedyaDL into `/Applications/MeedyaSuite`"
/// (#1057).
///
/// A pure, cross-platform-testable eligibility predicate
/// (`is_eligible_for_relocation`) plus the macOS-only move + relaunch
/// logic (`perform_relocation`). Computed once at startup via
/// `compute_startup_state()` and stored as managed state; the actual
/// move only ever happens when the user explicitly accepts the offer
/// via the `relocate_app_bundle` IPC command. Never blocks or crashes
/// the app on failure — see the module doc for the full design.
pub mod app_relocation;

/// Only compiled in test mode (`cargo test`).
#[cfg(test)]
mod integration_tests;
