// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Utility modules providing cross-cutting concerns.
// ==================================================
//
// This module aggregates three utility sub-modules that are used throughout
// the application by both the `commands` and `services` layers. None of
// these modules hold state; they are purely functional helpers.
//
// Module map:
//   utils/
//   +-- platform.rs   -- OS detection and path resolution
//   +-- archive.rs    -- HTTP download + archive extraction (ZIP, TAR.GZ)
//   +-- process.rs    -- GAMDL subprocess output parsing (regex-based)
//
// These utilities are imported by services like `python_manager`,
// `gamdl_service`, and `dependency_manager` to perform platform-specific
// file operations, download and unpack release archives, and parse GAMDL's
// stdout/stderr into structured events for the React frontend.

/// Platform detection, path resolution, and OS-specific utilities.
///
/// Provides functions to resolve the app data directory, Python binary
/// path, pip binary path, tools directory, and GAMDL config path for
/// the current operating system (macOS, Windows, Linux).
///
/// Used by: `services::python_manager`, `services::dependency_manager`,
///          `services::config_service`, `commands::system`
pub mod platform;

/// Archive download and extraction utilities (ZIP, TAR.GZ).
///
/// Provides an async `download_file()` that streams HTTP responses to disk
/// with progress logging, plus format-specific extractors (`extract_zip`,
/// `extract_tar_gz`) that run synchronous I/O on Tokio's blocking thread
/// pool. The high-level `download_and_extract()` combines both steps.
///
/// Used by: `services::python_manager`, `services::dependency_manager`
pub mod archive;

/// Subprocess stdout/stderr parsing for GAMDL CLI output.
///
/// Uses compiled regex patterns (via `LazyLock`) to parse yt-dlp-style
/// download progress lines, GAMDL track information, post-processing
/// steps, error messages, and file-save confirmations into a
/// `GamdlOutputEvent` enum that the frontend can render as a progress UI.
///
/// Used by: `services::gamdl_service`, `services::download_queue`
pub mod process;

/// App-wide activity log emission helpers.
///
/// Provides `emit_download_log()` and `emit_app_log()` functions for emitting
/// structured activity log events from any backend module. Centralises the
/// `ActivityLogEvent` struct that was previously private to `download_queue.rs`.
///
/// Used by: `services::download_queue`, `services::metadata_tag_service`,
///          `commands::gamdl`, `commands::updates`, `commands::dependencies`,
///          `commands::cookies`, `commands::settings`, `commands::login_window`
pub mod activity_log;

/// IPC command rate limiter.
///
/// Simple sliding-window rate limiter that prevents abuse of expensive
/// commands (downloads, API calls, updates). Each command has a configurable
/// max calls / window seconds limit.
///
/// Used by: `commands::gamdl`, `commands::updates`, `commands::cookies`
pub mod rate_limiter;

/// Collision-proof filesystem helpers.
///
/// Every `std::fs::rename` / `std::fs::write` / `std::fs::copy` path in
/// the app must use one of the helpers here rather than the raw stdlib
/// call, because `fs::rename` silently overwrites on Unix (data-loss
/// risk) and errors on Windows (platform-inconsistent). Exposes
/// `safe_rename`, `rename_if_dest_free`, `write_non_clobbering`,
/// `resolve_non_clobbering_path`, and `same_file`.
pub mod fs_safe;

/// Depth-bounded recursive directory traversal helper (#716 finding #1).
///
/// Replaces 5+ ad-hoc walkers across `services/` that each open-coded
/// `read_dir → recurse on subdirs → filter by extension/predicate →
/// accumulate`. Single primitive `walk_dir_depth(base, max_depth, visitor)`
/// covers every collect-paths / count-files / find-first use case via
/// a closure visitor. New callers should use this; existing callers
/// migrate opportunistically (#716 follow-up sub-tasks).
pub mod fs_walk;

/// Centralised reqwest::Client construction (#716 finding #2).
///
/// Replaces 13+ ad-hoc `reqwest::Client::builder()...build()` instances
/// across services/ + utils/ + commands/, each rebuilding the same
/// timeout + error-message pattern. `build_client(ClientConfig)` is
/// the single primitive; `build_simple(timeout_secs)` is the
/// convenience wrapper for the common case. New callers should use
/// these; existing callers migrate opportunistically.
pub mod http_client;

/// `context_err!` macro for Tauri command error wrapping (audit v2 #7).
///
/// Replaces ~30-40 sites of `.map_err(|e| format!("...: {e}"))?`
/// with `context_err!(result, "...")?`. The macro is exported via
/// `#[macro_export]` so it lives at crate root (`crate::context_err!`)
/// rather than under `utils::error_context::`. Real value isn't LOC
/// saved — it's having one place to evolve error formatting if we
/// ever migrate to a structured `CommandError` type.
pub mod error_context;

/// Subprocess line-reader spawn helper (audit v2 finding #5).
///
/// `spawn_line_reader(stream, visitor)` captures the truly-common
/// shell at every subprocess reader site (`BufReader → next_line
/// loop → visitor call`) without forcing the per-line work into a
/// generic shape. Direct enabler for new pip-engine implementations
/// (Votify M9, yt-dlp M10) which follow the engine_runner pattern;
/// callers with caller-specific per-stream state (companion
/// supervisor's watchdog/accumulator, download queue's last-clean
/// threading) keep their inline implementations and document the
/// choice in-source.
pub mod subprocess_reader;

/// Atomic JSON file write helper (#716 finding #8).
///
/// Replaces 5+ services that independently implement the same
/// `serialize → write to .tmp → rename` durability pattern (settings,
/// queue, history, crash reports, manifest, engine config). Single
/// primitive `atomic_write_json(path, data, context)` makes future
/// durability changes (fsync, retry-on-EBUSY, lock-file support)
/// touch one place.
pub mod atomic_write;

/// Per-file write-coordination locks for concurrent enrichment stages
/// (#779 Option 2). When AcoustID and ReplayGain both write freeform
/// atoms to the same M4A file via `mp4ameta::Tag::write_to_path`, the
/// read-modify-write cycle races at the byte level — last write wins,
/// losing the other stage's atoms. `FileWriteLocks` serialises writes
/// at the granularity of a single file so the slow per-file analyses
/// (chromaprint, FFmpeg ebur128) can run truly in parallel while the
/// fast millisecond-scale writes coordinate correctly.
pub mod file_locks;

/// Ring-buffer for subprocess output lines with per-line size cap
/// (#893). Replaces the pre-#893 `Arc<Mutex<Vec<String>>>` buffers
/// in `services::download_queue` that accumulated unbounded
/// per-GAMDL-subprocess output for the lifetime of the download —
/// the largest single contributor to RSS growth identified in the
/// 2026-05-28 memory audit (#897).
pub mod bounded_log;
