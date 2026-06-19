// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Engine runner — service-agnostic subprocess spawning and streaming.
// ==================================================================
//
// Provides a generic abstraction for running download engine subprocesses
// (GAMDL, Votify, yt-dlp, get_iplayer) with consistent stdout/stderr
// streaming, progress event emission, and error handling.
//
// ## Architecture
//
// Each download engine implements the `EngineCommandBuilder` trait to
// construct its CLI command. The generic `run_engine()` function handles
// the subprocess lifecycle (spawning, piped I/O, line-by-line parsing,
// event emission, exit status checking) so engine-specific code only
// needs to define how to build the command and parse output.
//
// ## Usage
//
// ```rust,ignore
// let builder = GamdlCommandBuilder;
// let cmd = builder.build_command(app, urls, options)?;
// run_engine(app, download_id, "gamdl", cmd).await?;
// ```
//
// ## References
//
// - Tokio async process: https://docs.rs/tokio/latest/tokio/process/
// - Tauri event emission: https://v2.tauri.app/develop/calling-rust/#events

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::utils::process;
use crate::utils::subprocess_reader::spawn_line_reader;

// ============================================================
// Progress event payload (generic, replaces GAMDL-specific)
// ============================================================

/// Progress event emitted during any engine subprocess execution.
///
/// The frontend listens for "engine-output" events (and the legacy
/// "gamdl-output" alias) to update the download UI. Each event is
/// tagged with a `download_id` and `engine` so the React frontend
/// can route events to the correct download card.
#[derive(Debug, Clone, Serialize)]
pub struct EngineProgress {
    /// Unique identifier for the download this event belongs to.
    pub download_id: String,
    /// Engine identifier (e.g., "gamdl", "votify", "ytdlp").
    pub engine: String,
    /// The structured event parsed from the engine's output line.
    pub event: process::GamdlOutputEvent,
}

// ============================================================
// Command builder trait
// ============================================================

/// Trait for building engine-specific subprocess commands.
///
/// Each download engine (GAMDL, Votify, yt-dlp, etc.) implements this
/// trait to construct its CLI command with appropriate arguments, tool
/// paths, and configuration. The `run_engine()` function then handles
/// the common subprocess lifecycle.
pub trait EngineCommandBuilder: Send + Sync {
    /// Returns the engine identifier (e.g., "gamdl", "votify", "ytdlp").
    fn engine_id(&self) -> &str;

    /// Builds the subprocess command for this engine.
    ///
    /// The returned `Command` should have all arguments configured but
    /// should NOT set stdio pipes — `run_engine()` handles that.
    ///
    /// # Arguments
    /// * `app` - The Tauri app handle (for path resolution)
    /// * `urls` - URLs to download
    /// * `cli_args` - Pre-built CLI argument strings
    ///
    /// # Errors
    /// Returns `Err(String)` if the command cannot be built (e.g., Python not found).
    fn build_command(
        &self,
        app: &AppHandle,
        urls: &[String],
        cli_args: &[String],
    ) -> Result<Command, String>;
}

// ============================================================
// Generic engine runner
// ============================================================

/// Runs a download engine subprocess with stdout/stderr streaming.
///
/// This is the generic equivalent of `gamdl_service::run_gamdl()`. It:
/// 1. Configures piped stdio on the command
/// 2. Spawns the subprocess
/// 3. Reads stdout/stderr concurrently via tokio tasks
/// 4. Parses each line via `process::parse_gamdl_output()`
/// 5. Emits `EngineProgress` events to the frontend
/// 6. Waits for process exit and returns the result
///
/// # Arguments
/// * `app` - Tauri app handle for event emission
/// * `download_id` - Unique download identifier for event routing
/// * `engine_id` - Engine name for logging and event tagging
/// * `cmd` - The pre-built command (stdio will be configured here)
///
/// # Errors
/// Returns `Err(String)` if the process fails to spawn or exits with non-zero status.
pub async fn run_engine(
    app: &AppHandle,
    download_id: &str,
    engine_id: &str,
    mut cmd: Command,
) -> Result<(), String> {
    log::info!(
        "Starting {engine_id} download {download_id}"
    );

    // Configure piped stdio for real-time output streaming
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the subprocess
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start {engine_id} process: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture {engine_id} stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture {engine_id} stderr"))?;

    // Spawn one reader task per stream via the shared helper. The
    // per-line work is identical between stdout and stderr — parse,
    // log, emit on the generic + legacy event channels — so we
    // capture it once in a builder and invoke `spawn_line_reader`
    // twice. Audit v2 #5: the helper is the truly-common shell
    // (BufReader → next_line loop); the visitor body is the
    // engine-specific work.
    let make_emitter = |stream_label: &'static str| {
        let download_id = download_id.to_string();
        let engine = engine_id.to_string();
        let app = app.clone();
        move |line: String| {
            let download_id = download_id.clone();
            let engine = engine.clone();
            let app = app.clone();
            async move {
                let event = process::parse_gamdl_output(&line);
                log::debug!("[{engine} {stream_label}] {line}");

                let progress = EngineProgress {
                    download_id: download_id.clone(),
                    engine,
                    event: event.clone(),
                };
                // Emit on both the generic and legacy event channels —
                // the frontend still listens to the legacy "gamdl-output"
                // alias for backwards compatibility.
                let _ = app.emit("engine-output", &progress);
                let legacy = super::gamdl_service::GamdlProgress {
                    download_id,
                    event,
                };
                let _ = app.emit("gamdl-output", &legacy);
            }
        }
    };
    let stdout_task = spawn_line_reader(stdout, make_emitter("stdout"));
    let stderr_task = spawn_line_reader(stderr, make_emitter("stderr"));

    // Wait for process exit
    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for {engine_id} process: {e}"))?;

    // Wait for readers to drain remaining output
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    if status.success() {
        log::info!("{engine_id} download {download_id} completed successfully");
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        Err(format!("{engine_id} process exited with code {code}"))
    }
}

// ============================================================
// GAMDL command builder
// ============================================================

/// Command builder for the GAMDL engine (Apple Music downloads).
///
/// Delegates to `gamdl_service::build_gamdl_command_public()` which handles
/// Python path resolution, URL validation, option conversion, tool path
/// injection, and config.ini syncing.
pub struct GamdlCommandBuilder;

impl EngineCommandBuilder for GamdlCommandBuilder {
    fn engine_id(&self) -> &str {
        "gamdl"
    }

    fn build_command(
        &self,
        app: &AppHandle,
        urls: &[String],
        _cli_args: &[String],
    ) -> Result<Command, String> {
        // GAMDL uses GamdlOptions for its full argument construction,
        // so this delegates to the existing builder. The cli_args param
        // is not used here — GamdlOptions::to_cli_args() handles it.
        // The download_queue calls build_gamdl_command_public() directly
        // for now; this adapter enables future callers to use the trait.
        use crate::models::gamdl_options::GamdlOptions;
        let options = GamdlOptions::default();
        super::gamdl_service::build_gamdl_command_public(app, urls, &options)
    }
}

// ============================================================
// Stub command builders for future engines
// ============================================================

/// Command builder stub for the Votify engine (Spotify downloads).
///
/// Not yet functional — will be implemented in milestone M9 (v2.1.0).
/// Votify is a pip-installed Python package, similar to GAMDL.
pub struct VotifyCommandBuilder;

impl EngineCommandBuilder for VotifyCommandBuilder {
    fn engine_id(&self) -> &str {
        "votify"
    }

    fn build_command(
        &self,
        _app: &AppHandle,
        _urls: &[String],
        _cli_args: &[String],
    ) -> Result<Command, String> {
        Err("Votify engine is not yet implemented (planned for v2.1.0)".to_string())
    }
}

/// Command builder stub for yt-dlp (BBC iPlayer / YouTube downloads).
///
/// Not yet functional — will be implemented in milestones M8/M10.
/// yt-dlp is a pip-installed Python package shared between BBC iPlayer
/// (fallback engine) and YouTube (primary engine).
pub struct YtdlpCommandBuilder;

impl EngineCommandBuilder for YtdlpCommandBuilder {
    fn engine_id(&self) -> &str {
        "ytdlp"
    }

    fn build_command(
        &self,
        _app: &AppHandle,
        _urls: &[String],
        _cli_args: &[String],
    ) -> Result<Command, String> {
        Err("yt-dlp engine is not yet implemented (planned for v2.0.0)".to_string())
    }
}

/// Command builder stub for get_iplayer (BBC iPlayer primary engine).
///
/// Not yet functional — will be implemented in milestone M8 (v2.0.0).
/// get_iplayer uses a different installation method (system binary,
/// not pip) and has a distinct CLI interface.
pub struct GetIplayerCommandBuilder;

impl EngineCommandBuilder for GetIplayerCommandBuilder {
    fn engine_id(&self) -> &str {
        "get_iplayer"
    }

    fn build_command(
        &self,
        _app: &AppHandle,
        _urls: &[String],
        _cli_args: &[String],
    ) -> Result<Command, String> {
        Err("get_iplayer engine is not yet implemented (planned for v2.0.0)".to_string())
    }
}

// ============================================================
// Builder factory
// ============================================================

/// Returns the appropriate command builder for an engine ID.
///
/// Maps engine IDs from `engines.toml` to their `EngineCommandBuilder`
/// implementation. Returns `None` for unknown engine IDs.
#[must_use]
pub fn get_command_builder(engine_id: &str) -> Option<Box<dyn EngineCommandBuilder>> {
    match engine_id {
        "gamdl" => Some(Box::new(GamdlCommandBuilder)),
        "votify" => Some(Box::new(VotifyCommandBuilder)),
        "ytdlp" => Some(Box::new(YtdlpCommandBuilder)),
        "get_iplayer" => Some(Box::new(GetIplayerCommandBuilder)),
        _ => None,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_command_builder_known_engines() {
        assert!(get_command_builder("gamdl").is_some());
        assert!(get_command_builder("votify").is_some());
        assert!(get_command_builder("ytdlp").is_some());
        assert!(get_command_builder("get_iplayer").is_some());
    }

    #[test]
    fn test_get_command_builder_unknown() {
        assert!(get_command_builder("unknown-engine").is_none());
    }

    #[test]
    fn test_engine_ids() {
        assert_eq!(GamdlCommandBuilder.engine_id(), "gamdl");
        assert_eq!(VotifyCommandBuilder.engine_id(), "votify");
        assert_eq!(YtdlpCommandBuilder.engine_id(), "ytdlp");
        assert_eq!(GetIplayerCommandBuilder.engine_id(), "get_iplayer");
    }

    #[test]
    fn test_stub_builders_return_error() {
        // Stub builders should return clear "not implemented" errors.
        // We can't easily test with a real AppHandle in unit tests
        // (tauri::test requires the "test" feature), so we just verify
        // the trait implementations exist and the factory function
        // returns the right types.
        let votify = get_command_builder("votify").unwrap();
        assert_eq!(votify.engine_id(), "votify");
    }
}
