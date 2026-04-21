// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download queue manager service.
// Manages a queue of download jobs with concurrent execution limits,
// automatic processing of queued items, fallback quality retries,
// and child process tracking for cancellation support.
//
// ## Architecture Overview
//
// The download queue is the central orchestrator for all GAMDL downloads.
// It sits between the frontend (React) and the GAMDL subprocess execution:
//
// ```
// Frontend (React)                  Download Queue                    GAMDL Process
// ================                  ==============                    =============
// "Add to Queue" button  -->  enqueue() --> QueueItem (Queued)
//                             process_queue() --> next_pending()
//                                                    |
//                             run_download_with_events() --> spawn GAMDL
//                                    |                          |
//                             update_item_progress() <-- parse stdout/stderr
//                                    |
//                             emit("gamdl-output") --> frontend listener
//                                    |
//                             on_task_finished() --> process_queue() (cascade)
// ```
//
// ## Key Design Decisions
//
// 1. **Arc<Mutex<DownloadQueue>>**: The queue is wrapped in Arc<Mutex<>> for
//    thread-safe access from multiple Tauri command handlers and background tasks.
//    Ref: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
//
// 2. **VecDeque for FIFO ordering**: Items are processed front-to-back, with
//    new items added to the back. This provides natural queue ordering.
//
// 3. **Recursive process_queue()**: After each download completes, process_queue()
//    is called again to start the next item, creating a cascade effect.
//    Uses Pin<Box<dyn Future>> to support this recursive async pattern.
//
// 4. **Fallback codec chains**: When a download fails with a codec error, the
//    queue automatically retries with the next codec in the fallback chain
//    (e.g., alac -> aac-he -> aac-binaural). Configurable in settings.
//
// 5. **Network retries**: Network errors trigger automatic retries (up to 3 by default)
//    with the same options, giving transient errors a chance to resolve.
//
// 6. **Cancellation polling**: Running downloads are checked for cancellation every
//    250ms via try_wait() + is_cancelled(). The process is killed on cancellation.
//
// ## Event Emission Pattern
//
// Real-time progress is reported to the frontend via Tauri's event system:
// - "download-started" - Emitted when a queued item begins downloading
// - "gamdl-output" - Emitted for each parsed line of GAMDL output (progress, track info, etc.)
// - "download-complete" - Emitted when a download finishes successfully
// - "download-error" - Emitted when a download fails (includes error category for UI routing)
// Ref: https://v2.tauri.app/develop/calling-rust/#events
//
// ## References
//
// - Tokio Mutex (async-aware): https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
// - Tokio process spawning: https://docs.rs/tokio/latest/tokio/process/
// - Pin and Box for recursive futures: https://doc.rust-lang.org/std/pin/
// - Tauri event system: https://v2.tauri.app/develop/calling-rust/#events

use std::collections::{HashSet, VecDeque};
// Future and Pin are needed for the recursive async pattern in process_queue().
// Recursive async functions cannot use normal `async fn` syntax because the
// compiler cannot determine the size of the future at compile time.
// Instead, we return Pin<Box<dyn Future<Output = ()> + Send>>.
// Ref: https://doc.rust-lang.org/std/pin/index.html
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
// Tokio's Mutex is used instead of std::sync::Mutex because the lock is held
// across .await points. std::sync::Mutex would block the entire thread;
// tokio::sync::Mutex yields the task instead.
// Ref: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
use tokio::sync::Mutex;

// Emitter trait provides app.emit() for sending events to the frontend.
use tauri::{AppHandle, Emitter};

// DownloadRequest: The user's download request from the frontend (URLs + optional overrides).
// DownloadState: Enum of lifecycle states (Queued, Downloading, Processing, Complete, Error, Cancelled).
// QueueItemStatus: The public-facing status struct sent to the frontend for UI rendering.
use crate::models::download::{DownloadRequest, DownloadState, QueueItemStatus};
// GamdlOptions: Typed representation of GAMDL CLI arguments, used as the "effective" options
// after merging per-download overrides with global settings.
// SongCodec: Enum of audio codec options, used for companion download planning and
// codec suffix logic.
use crate::models::codec_registry::codec_suffix_from_registry;
use crate::models::gamdl_options::{GamdlOptions, LyricsFormat, SongCodec};
// AppSettings: The full application settings, used for merging defaults and fallback chain config.
// CompanionMode: Enum controlling companion download behavior (Disabled, AtmosToLossless, etc.).
use crate::models::settings::{AppSettings, CompanionMode};
// config_service: Used to load settings during fallback decisions.
// gamdl_service: Provides build_gamdl_command_public() and GamdlProgress for subprocess execution.
// crash_report_service: Used to save download error reports for user-reportable diagnostics.
use crate::services::{config_service, crash_report_service, gamdl_service, history_service};
// CrashReport: Reused for download error reports (source: "download_error").
use crate::models::crash_report::CrashReport;
// process: Provides parse_gamdl_output() for parsing GAMDL output lines and
// classify_error() for categorizing errors (codec, network, etc.) for retry logic.
use crate::utils::process;
// Activity log helpers: emit_download_log for per-download messages,
// emit_app_log for system-level messages, ActivityLogEvent for raw stdout/stderr.
use crate::utils::activity_log::{
    emit_app_log, emit_download_log, emit_verbose_download_log, ActivityLogEvent,
};

// ============================================================
// Graceful shutdown signal
// ============================================================

/// Application-wide shutdown signal for fire-and-forget background tasks.
///
/// When the user closes the app window or clicks "Quit" in the tray menu,
/// this flag is set to `true`. Fire-and-forget tasks (companion downloads,
/// lyrics companions, and the enrichment pipeline) check this flag between
/// iterations and exit early instead of starting new work.
///
/// Uses `AtomicBool` with `Ordering::Relaxed` for minimal overhead — the
/// shutdown signal only needs to propagate eventually (within one loop
/// iteration), not with strict memory ordering guarantees.
///
/// # Cloning
/// The `Clone` derive produces a cheap `Arc` reference count increment,
/// making it safe to pass to multiple `tokio::spawn` tasks.
#[derive(Clone, Default)]
pub struct ShutdownSignal(Arc<AtomicBool>);

impl ShutdownSignal {
    /// Creates a new shutdown signal in the non-triggered state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals all background tasks to stop at their next check point.
    pub fn trigger(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Returns `true` if shutdown has been requested.
    pub fn is_triggered(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

// ============================================================
// Activity log event helpers
// ============================================================
// ActivityLogEvent, emit_download_log, and emit_app_log are now in
// crate::utils::activity_log (shared across commands and services).
// The import is at the top of this file.

/// Strips query parameters from a URL before logging to prevent credential leakage.
///
/// Wrapper URLs may contain authentication tokens as query parameters
/// (e.g., `http://host:port/?token=abc`). Logging these would persist tokens
/// in plaintext log files. This function returns the URL up to (but not
/// including) the `?` character, preserving only the scheme, host, port, and path.
fn redact_url_query(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Notification throttling state: tracks last notification time per category
/// and batched count to prevent notification spam during rapid queue processing.
static NOTIFICATION_THROTTLE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, u32)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Minimum interval between notifications of the same category (seconds).
const NOTIFICATION_THROTTLE_SECS: u64 = 10;

/// Sends a native OS desktop notification if the setting is enabled and the
/// main application window is not focused.
///
/// This avoids interrupting users who are actively watching the queue. When the
/// window is minimized, in the background, or the user has switched to another
/// app, a notification alerts them that a download has completed or failed.
///
/// Silently does nothing if:
/// - The `desktop_notifications` setting is `false`.
/// - The main window is currently focused (visible and in foreground).
/// - The notification fails to build or send (non-critical).
fn send_desktop_notification(app: &AppHandle, title: &str, body: &str) {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;

    // Check if desktop notifications are enabled in user settings
    let settings = load_settings_for_queue(app);
    if !settings.desktop_notifications {
        return;
    }

    // Only send notifications when the window is NOT focused.
    if let Some(window) = app.get_webview_window("main") {
        if window.is_focused().unwrap_or(false) {
            return;
        }
    }

    // Throttle: batch rapid notifications of the same title category.
    // If the same title was sent within the last 10 seconds, update the
    // count and modify the body to show "N downloads completed" etc.
    let throttle_key = title.to_string();
    let display_body = {
        let mut throttle = NOTIFICATION_THROTTLE.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let entry = throttle.entry(throttle_key).or_insert((now, 0));
        let elapsed = now.duration_since(entry.0);

        if elapsed < std::time::Duration::from_secs(NOTIFICATION_THROTTLE_SECS) {
            entry.1 += 1;
            if entry.1 > 1 {
                // Batch: update the body with count
                format!("{} ({} items)", body, entry.1)
            } else {
                body.to_string()
            }
        } else {
            // Reset: enough time has passed
            *entry = (now, 1);
            body.to_string()
        }
    };

    // Send the OS-native notification
    app.notification()
        .builder()
        .title(title)
        .body(&display_body)
        .show()
        .ok();
}

/// Executes the configured after-queue action when the queue becomes idle.
///
/// Checks `after_queue_once` first (one-shot override), then `after_queue_action`
/// (persistent). One-shot actions are cleared after execution. Called after
/// `on_task_finished()` when `is_idle()` returns true.
fn execute_after_queue_action(app: &AppHandle) {
    use crate::models::settings::AfterQueueAction;

    let mut settings = load_settings_for_queue(app);

    // Resolve which action to execute: one-shot overrides persistent
    let action = settings
        .after_queue_once
        .take() // consume one-shot
        .unwrap_or(settings.after_queue_action);

    // Clear one-shot in saved settings if it was set
    if settings.after_queue_once.is_some() {
        settings.after_queue_once = None;
        // Save updated settings (clear the one-shot flag)
        let data_dir = crate::utils::platform::get_app_data_dir(app);
        let settings_path = data_dir.join("settings.json");
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(settings_path, json);
        }
    }

    match action {
        AfterQueueAction::DoNothing => {}
        AfterQueueAction::OpenOutputFolder => {
            let path = if settings.output_path.is_empty() {
                crate::services::config_service::get_default_output_path()
                    .unwrap_or_else(|_| ".".to_string())
            } else {
                settings.output_path.clone()
            };
            // Open the folder in the system file manager using platform-native commands
            #[cfg(target_os = "macos")]
            { let _ = std::process::Command::new("open").arg(&path).spawn(); }
            #[cfg(target_os = "windows")]
            { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }
            #[cfg(target_os = "linux")]
            { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }
            log::info!("After-queue: opened output folder {path}");
            emit_app_log(app, &format!("After-queue action: opened output folder ({path})"));
        }
        AfterQueueAction::PlaySound => {
            send_desktop_notification(app, "Queue Complete", "All downloads finished.");
            log::info!("After-queue: played notification sound");
            emit_app_log(app, "After-queue action: notification sound");
        }
        AfterQueueAction::CloseMeedyadl => {
            log::info!("After-queue: closing MeedyaDL");
            emit_app_log(app, "After-queue action: closing MeedyaDL...");
            // Brief delay to let the activity log event propagate
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                app_clone.exit(0);
            });
        }
        AfterQueueAction::RestartComputer => {
            log::info!("After-queue: restarting computer");
            emit_app_log(app, "After-queue action: restarting computer in 30 seconds...");
            send_desktop_notification(app, "MeedyaDL", "Computer will restart in 30 seconds...");
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to restart"])
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/r", "/t", "30"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("systemctl")
                    .args(["reboot"])
                    .spawn();
            }
        }
        AfterQueueAction::HibernateComputer => {
            log::info!("After-queue: hibernating computer");
            emit_app_log(app, "After-queue action: hibernating computer...");
            #[cfg(target_os = "macos")]
            {
                // macOS uses sleep (pmset sleepnow) — true hibernate requires
                // hibernatemode 25 which most Macs don't use by default.
                let _ = std::process::Command::new("pmset").arg("sleepnow").spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/h"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                // systemctl hibernate requires swap; falls back gracefully
                let _ = std::process::Command::new("systemctl")
                    .args(["hibernate"])
                    .spawn();
            }
        }
        AfterQueueAction::ShutdownComputer => {
            log::info!("After-queue: shutting down computer");
            emit_app_log(app, "After-queue action: shutting down computer in 30 seconds...");
            send_desktop_notification(app, "MeedyaDL", "Computer will shut down in 30 seconds...");
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to shut down"])
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/s", "/t", "30"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("systemctl")
                    .args(["poweroff"])
                    .spawn();
            }
        }
    }
}

/// Normalises a URL for duplicate detection by lowercasing the domain,
/// stripping trailing slashes, and removing query parameters except
/// essential ones like `?i=` (Apple Music track IDs within album URLs).
///
/// This produces a canonical form so that cosmetically different URLs
/// pointing to the same resource are recognised as duplicates:
///   - `https://Music.Apple.Com/us/album/...` == `https://music.apple.com/us/album/...`
///   - `https://music.apple.com/us/album/foo/123/` == `https://music.apple.com/us/album/foo/123`
///   - `https://music.apple.com/us/album/foo/123?ls=1` == `https://music.apple.com/us/album/foo/123`
///   - `https://music.apple.com/us/album/foo/123?i=456` is kept distinct (track-specific)
///
/// Extract album name and artist name from an Apple Music URL at enqueue time.
///
/// Apple Music URLs have the format:
///   `https://music.apple.com/{storefront}/album/{album-slug}/{id}`
///
/// The album slug is a hyphenated lowercase version of the album name.
/// Artist name is not available from the URL alone — it will be populated
/// later from the Apple Music API during enrichment (Step 1). For artist
/// URLs the artist slug is extracted instead.
///
/// Returns `(album_name, artist_name)` — artist is always `None` from URL parsing.
fn extract_album_info_from_url(url: &str) -> (Option<String>, Option<String>) {
    let Ok(parsed) = url::Url::parse(url) else {
        return (None, None);
    };
    let segments: Vec<&str> = parsed.path_segments().map_or(vec![], |s| s.collect());

    // Find album slug: /album/{slug}/{id}
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "album" && i + 1 < segments.len() {
            let slug = segments[i + 1];
            // Convert hyphenated slug to title case: "the-platinum-collection" → "The Platinum Collection"
            let name = slug
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            return (Some(name), None);
        }
        // Artist URLs: /artist/{slug}/{id}
        if *seg == "artist" && i + 1 < segments.len() {
            let slug = segments[i + 1];
            let name = slug
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            return (None, Some(name));
        }
    }

    (None, None)
}

fn normalize_url_for_dedup(url: &str) -> String {
    // Split scheme + authority from path+query.
    // URL structure: scheme://authority/path?query#fragment
    let url = url.trim();

    // Find the end of "scheme://".
    let after_scheme = if let Some(pos) = url.find("://") {
        pos + 3
    } else {
        // Not a valid URL — return lowercased as-is for best-effort comparison.
        return url.to_lowercase();
    };

    // Split authority (domain[:port]) from the rest at the first `/` after `://`.
    let (before_path, path_and_rest) = match url[after_scheme..].find('/') {
        Some(slash_pos) => {
            let abs = after_scheme + slash_pos;
            (&url[..abs], &url[abs..])
        }
        None => {
            // No path — just the domain (e.g., `https://example.com`).
            return url.to_lowercase();
        }
    };

    // Lowercase the scheme + authority (domain is case-insensitive per RFC 3986).
    let lower_authority = before_path.to_lowercase();

    // Strip fragment (#...) first, then handle query.
    let without_fragment = path_and_rest.split('#').next().unwrap_or(path_and_rest);

    // Split path from query string.
    let (path, query) = match without_fragment.find('?') {
        Some(q) => (&without_fragment[..q], Some(&without_fragment[q + 1..])),
        None => (without_fragment, None),
    };

    // Strip trailing slashes from the path.
    let path = path.trim_end_matches('/');

    // Keep only the `i=` query parameter (Apple Music track ID within an album).
    let essential_query = query.and_then(|q| {
        q.split('&')
            .find(|param| param.starts_with("i="))
            .map(|param| format!("?{param}"))
    });

    format!(
        "{lower_authority}{path}{}",
        essential_query.unwrap_or_default()
    )
}

/// Finds the deepest (leaf) subdirectory containing audio files (.m4a/.m4v).
///
/// GAMDL creates an `Artist/Album/` directory structure under the base output
/// path. This function must return the **album** directory (where audio files
/// live), not the artist directory.
///
/// When `artist_hint` and/or `album_hint` are provided (from the early metadata
/// fetch), the search first attempts a targeted path match before falling back
/// to the generic timestamp-based scan. This prevents cross-contamination
/// between concurrent downloads (#452) where the generic scan might return
/// a different artist's most recently modified directory.
///
/// Fixed in #447/#452: previously only searched one level deep and used
/// recency-based selection that could pick the wrong artist's directory.
fn find_album_directory(
    base_dir: &std::path::Path,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
) -> Option<String> {
    // --- Targeted search: use artist/album names to find the exact directory ---
    // GAMDL's default template creates: base_dir/Artist/Album/
    if let (Some(artist), Some(album)) = (artist_hint, album_hint) {
        let targeted = base_dir.join(artist).join(album);
        if targeted.is_dir() && has_direct_audio_files(&targeted) {
            log::info!(
                "find_album_directory: targeted match at {}",
                targeted.display()
            );
            return Some(targeted.to_string_lossy().to_string());
        }
        // Try case-insensitive match on the artist/album directory names
        if let Some(found) = find_directory_case_insensitive(base_dir, artist, album) {
            log::info!(
                "find_album_directory: case-insensitive match at {}",
                found.display()
            );
            return Some(found.to_string_lossy().to_string());
        }
    }

    // --- Fallback: generic deep scan (picks most recently modified leaf dir) ---
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;

    find_deepest_audio_dir(base_dir, &mut best);

    // If no subdirectory with audio files found, check if base itself has audio
    if best.is_none() && has_direct_audio_files(base_dir) {
        return Some(base_dir.to_string_lossy().to_string());
    }

    best.map(|(_, p)| p.to_string_lossy().to_string())
}

/// Case-insensitive directory matching for Artist/Album structure.
/// Handles slight differences in filesystem naming vs. API naming
/// (e.g., special characters stripped, Unicode normalisation).
fn find_directory_case_insensitive(
    base_dir: &std::path::Path,
    artist: &str,
    album: &str,
) -> Option<std::path::PathBuf> {
    let artist_lower = artist.to_lowercase();
    let album_lower = album.to_lowercase();

    // Scan base_dir for a matching artist subdirectory
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name()?.to_string_lossy().to_lowercase();
        if dir_name != artist_lower {
            continue;
        }
        // Found artist dir — now scan for album subdirectory
        let Ok(album_entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for album_entry in album_entries.flatten() {
            let album_path = album_entry.path();
            if !album_path.is_dir() {
                continue;
            }
            let album_dir_name = album_path.file_name()?.to_string_lossy().to_lowercase();
            if album_dir_name == album_lower && has_direct_audio_files(&album_path) {
                return Some(album_path);
            }
        }
    }
    None
}

/// Recursively finds the deepest directory that directly contains audio files.
/// Prefers the most recently modified leaf directory.
fn find_deepest_audio_dir(
    dir: &std::path::Path,
    best: &mut Option<(std::time::SystemTime, std::path::PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // If this directory directly contains audio files, it's a candidate
            if has_direct_audio_files(&path) {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                            *best = Some((modified, path.clone()));
                        }
                    }
                }
            }
            // Always recurse deeper — there may be nested album directories
            find_deepest_audio_dir(&path, best);
        }
    }
}

/// Checks if the given directory directly contains audio files (non-recursive).
/// Unlike `has_audio_files()`, this does NOT recurse into subdirectories.
fn has_direct_audio_files(dir: &std::path::Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("m4a")
                        || ext.eq_ignore_ascii_case("m4v")
                        || ext.eq_ignore_ascii_case("mp4")
                        || ext.eq_ignore_ascii_case("flac")
                        || ext.eq_ignore_ascii_case("mp3")
                        || ext.eq_ignore_ascii_case("ogg")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Count GAMDL warnings indicating tracks were skipped because the
/// requested codec format was unavailable. These appear as stderr lines
/// like "Requested format is not available for song ..." when GAMDL
/// skips tracks in the priority chain without wrapper auth.
fn count_codec_skip_warnings(warnings: &[String]) -> usize {
    warnings
        .iter()
        .filter(|w| {
            let lower = w.to_lowercase();
            lower.contains("format is not available")
                || lower.contains("format not available")
                || lower.contains("requested format")
        })
        .count()
}

/// Build a gap-fill priority chain by removing wrapper-dependent codecs
/// (Atmos, AC3) from the original chain. These codecs don't reliably
/// work without wrapper authentication for per-track availability.
///
/// Returns `None` if no non-experimental codecs remain after filtering.
fn build_gapfill_priority_chain(original_chain: &str) -> Option<String> {
    let filtered: Vec<&str> = original_chain
        .split(',')
        .filter(|codec_str| {
            // Parse each codec string and check if it's wrapper-dependent
            if let Some(codec) = SongCodec::from_cli_string(codec_str.trim()) {
                !codec.is_wrapper_dependent()
            } else {
                // Unknown codec strings are kept (conservative)
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        None
    } else {
        Some(filtered.join(","))
    }
}

/// Count audio files (.m4a, .m4v, .mp4) in the output directory to
/// detect partial download success. Searches recursively through
/// Artist/Album subdirectory structure.
fn count_audio_files_in_directory(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("m4a")
                        || ext.eq_ignore_ascii_case("m4v")
                        || ext.eq_ignore_ascii_case("mp4")
                    {
                        count += 1;
                    }
                }
            } else if path.is_dir() {
                count += count_audio_files_in_directory(&path);
            }
        }
    }
    count
}

// ============================================================
// Manifest writer
// ============================================================

/// Write or update a `.meedyadl` manifest file in the album output directory.
///
/// If a manifest already exists, the new source is merged (append or
/// replace matching platform+URL). Uses atomic write-to-temp-then-rename.
fn write_manifest(
    album_dir: &str,
    urls: &[String],
    album_metadata: Option<&crate::services::apple_music_api::AlbumMetadata>,
    settings: &crate::models::settings::AppSettings,
    downloaded_at: &str,
) {
    use crate::models::manifest::{ManifestFile, ManifestSource, ManifestTrack};

    let dir = std::path::Path::new(album_dir);
    if !dir.exists() {
        log::warn!("Manifest: album dir does not exist: {album_dir}");
        return;
    }

    let manifest_path = dir.join("manifest.meedyadl");
    log::info!("Writing manifest to: {}", manifest_path.display());

    // Migration: rename legacy hidden dotfile to visible filename (#447)
    let legacy_path = dir.join(".meedyadl");
    if legacy_path.exists() && !manifest_path.exists() {
        if let Err(e) = std::fs::rename(&legacy_path, &manifest_path) {
            log::warn!("Failed to migrate legacy .meedyadl to manifest.meedyadl: {e}");
        } else {
            log::info!("Migrated legacy .meedyadl → manifest.meedyadl");
        }
    }
    let url = urls.first().cloned().unwrap_or_default();
    if url.is_empty() {
        return;
    }

    // Determine platform from the URL domain using the MediaServiceId enum
    let platform = crate::models::media_service::MediaServiceId::from_url(&url)
        .map_or_else(|| "unknown".to_string(), |svc| svc.to_string());

    // Build per-track metadata from AlbumMetadata (if available).
    // codec is intentionally null — the manifest is a metafile for
    // re-downloading, not a prescription of quality/format settings.
    let tracks: Vec<ManifestTrack> = album_metadata
        .map(|meta| {
            meta.tracks
                .iter()
                .map(|t| {
                    // Build individual track URL if we have the album URL + song ID
                    let track_url = if !t.song_id.is_empty() && url.contains("/album/") {
                        // e.g., https://music.apple.com/gb/album/slug/123?i=456
                        Some(format!("{}?i={}", url, t.song_id))
                    } else {
                        None
                    };
                    // Record Apple Music song_id when non-empty so the
                    // duplicate-detection "History" scope (#510) can match
                    // previously-downloaded tracks.
                    let song_id = if t.song_id.is_empty() {
                        None
                    } else {
                        Some(t.song_id.clone())
                    };
                    ManifestTrack {
                        number: t.track_number,
                        disc: t.disc_number,
                        title: t.name.clone(),
                        url: track_url,
                        codec: None,
                        isrc: t.isrc.clone(),
                        song_id,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let storefront = if settings.storefront.is_empty() {
        None
    } else {
        Some(settings.storefront.clone())
    };

    let source = ManifestSource {
        platform: platform.to_string(),
        url: url.clone(),
        storefront,
        downloaded_at: downloaded_at.to_string(),
        codec: None,
        last_modified_date: album_metadata.and_then(|m| m.last_modified_date.clone()),
        tracks,
    };

    // Read existing manifest or create new
    let mut manifest = if manifest_path.exists() {
        match std::fs::read_to_string(&manifest_path) {
            Ok(contents) => match serde_json::from_str::<ManifestFile>(&contents) {
                Ok(existing) => existing,
                Err(e) => {
                    log::warn!("Failed to parse existing manifest: {e}");
                    ManifestFile::new(source.clone())
                }
            },
            Err(e) => {
                log::warn!("Failed to read existing manifest: {e}");
                ManifestFile::new(source.clone())
            }
        }
    } else {
        ManifestFile::new(source.clone())
    };

    // Merge the source (appends or replaces matching platform+url)
    if manifest_path.exists() {
        manifest.merge_source(source);
    }

    // Write atomically (temp file + rename)
    match serde_json::to_string_pretty(&manifest) {
        Ok(json) => {
            // Use a sibling temp file with a simple suffix to avoid
            // Rust's `with_extension()` quirks on dotfiles (#447).
            let tmp_path = dir.join("manifest.meedyadl.tmp");
            if let Err(e) = std::fs::write(&tmp_path, &json) {
                log::warn!("Failed to write manifest temp file: {e}");
                return;
            }
            if let Err(e) = std::fs::rename(&tmp_path, &manifest_path) {
                log::warn!("Failed to rename manifest temp file: {e}");
                // Clean up temp file
                let _ = std::fs::remove_file(&tmp_path);
                return;
            }
            log::info!("Wrote download manifest to {}", manifest_path.display());
        }
        Err(e) => {
            log::warn!("Failed to serialise manifest: {e}");
        }
    }
}

/// Rename GAMDL's `Cover.<ext>` to the user's configured cover art name (#448).
///
/// GAMDL hardcodes the static cover art filename as `Cover.jpg` / `Cover.png` /
/// `Cover.raw`. The `cover_art_name` setting controls what the file should be
/// renamed to (e.g., `FrontCover`, `Folder`, or kept as `Cover`).
///
/// Idempotent: skips if target already exists or source `Cover.<ext>` is absent.
fn rename_cover_art(album_dir: &str, target_stem: &str) {
    // If the user wants to keep the default "Cover" name, nothing to do
    if target_stem == "Cover" {
        return;
    }

    let dir = std::path::Path::new(album_dir);
    if !dir.exists() {
        return;
    }

    for ext in &["jpg", "png", "raw"] {
        let old_name = dir.join(format!("Cover.{ext}"));
        let new_name = dir.join(format!("{target_stem}.{ext}"));

        if old_name.exists() && !new_name.exists() {
            match std::fs::rename(&old_name, &new_name) {
                Ok(()) => {
                    log::info!(
                        "Renamed Cover.{ext} → {target_stem}.{ext} in {}",
                        dir.display()
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Failed to rename Cover.{ext} → {target_stem}.{ext}: {e}"
                    );
                }
            }
        }
    }
}

// ============================================================
// Queue item (internal representation with extra tracking fields)
// ============================================================

/// Internal representation of a download job in the queue.
///
/// Contains the public-facing `QueueItemStatus` (sent to the frontend) plus
/// additional private tracking fields used by the queue manager for retry
/// and fallback logic. The frontend never sees `fallback_index` or
/// `network_retries_left` directly.
#[derive(Debug, Clone)]
struct QueueItem {
    /// Public status sent to the frontend via `get_status()`.
    /// This is the serializable subset of the item's state.
    pub status: QueueItemStatus,
    /// The original download request as submitted by the user.
    /// Preserved for retry operations (retry resets options from this).
    pub request: DownloadRequest,
    /// Merged GAMDL options (user overrides merged with global settings).
    /// These are the "effective" options passed to GAMDL for this download.
    /// Updated during fallback (e.g., codec changes from alac to aac-he).
    pub merged_options: GamdlOptions,
    /// Index into the `settings.music_fallback_chain` array.
    /// 0 = preferred codec (initial attempt), 1 = first fallback, etc.
    /// Incremented by `try_fallback()` on codec-related errors.
    pub fallback_index: usize,
    /// Number of network retry attempts remaining before giving up.
    /// Decremented by `try_network_retry()` on network-related errors.
    pub network_retries_left: u32,
    /// Index into the engine chain for this platform (from engines.toml).
    /// 0 = primary engine, 1 = first fallback engine, etc.
    /// Incremented by `try_engine_fallback()` on tool errors (#320).
    pub engine_fallback_index: usize,
}

// ============================================================
// Persistence types (crash recovery + export/import)
// ============================================================

/// Persistable snapshot of a queue item, saved to `queue.json` for crash recovery.
///
/// Items in active states (Queued/Downloading/Processing) and failed items (Error)
/// are persisted; only Complete and Cancelled items are discarded on restart.
/// Failed items are restored in their Error state so the user can review and
/// manually retry them — they are not auto-retried.
///
/// The original `DownloadRequest` is preserved so that on restore, options are
/// re-merged with the current device's settings (rather than using stale merged
/// options from the previous session).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedQueueItem {
    /// The unique download ID (UUID v4), matching the original `QueueItem`'s ID.
    pub id: String,
    /// The original download request as submitted by the user (URLs + optional overrides).
    pub request: DownloadRequest,
    /// ISO 8601 timestamp of when the download was originally queued.
    pub created_at: String,
    /// Error message for failed items (`None` for active/queued items).
    /// Preserved so the failure reason is visible after app restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The detected media service (e.g., "apple-music"). Added in multi-service
    /// architecture; older persisted items will have `None` (backwards compat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

/// Top-level schema for a `.meedyadl` export file (JSON content inside).
///
/// Used for cross-device queue transfer: export on one machine, import on another.
/// The `version` field enables forward-compatible schema evolution — importers
/// should reject files with `version > 1` until a newer schema is defined.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueExportFile {
    /// Schema version (always 1 for the initial format).
    pub version: u32,
    /// Application identifier (always "`MeedyaDL`").
    pub app: String,
    /// ISO 8601 timestamp of when the export was created.
    pub exported_at: String,
    /// The queue items included in the export.
    pub items: Vec<ExportedItem>,
}

/// A single item within a `.meedyadl` export file.
///
/// Contains only the URLs and per-download overrides; the importing device
/// merges these with its own global settings on import. This means an export
/// created with ALAC settings can be imported on a device configured for AAC
/// and the import will respect the importing device's defaults (unless the
/// original download had explicit per-download overrides).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedItem {
    /// Apple Music URL(s) for this download.
    pub urls: Vec<String>,
    /// Per-download quality/format overrides (None = use importing device's defaults).
    pub options: Option<GamdlOptions>,
}

// ============================================================
// Download queue manager
// ============================================================

/// The download queue manager. Wrapped in Arc<Mutex<>> for thread-safe
/// access from multiple Tauri commands and background tasks.
///
/// The queue manages the full lifecycle of downloads:
/// Queued -> Downloading -> Processing -> Complete (happy path)
/// Queued -> Downloading -> Error -> (retry/fallback) -> Queued (retry path)
/// Queued -> Cancelled (user cancellation)
///
/// Only `max_concurrent` downloads run simultaneously. When a download
/// finishes, the queue automatically starts the next queued item.
#[derive(Debug)]
pub struct DownloadQueue {
    /// The queue of download jobs (front = next to process).
    /// `VecDeque` allows efficient `push_back` (enqueue) and iteration
    /// to find the next Queued item.
    items: VecDeque<QueueItem>,
    /// Maximum number of concurrent downloads (default: 1).
    /// Limiting to 1 avoids Apple Music rate limiting and reduces
    /// memory usage from concurrent GAMDL processes.
    max_concurrent: usize,
    /// Number of currently active (Downloading/Processing) downloads.
    /// Incremented by `next_pending()`, decremented by `on_task_finished()`.
    active_count: usize,
    /// Maximum number of network retry attempts per download (default: 3).
    /// Each download starts with this many retries; decremented on network errors.
    max_network_retries: u32,
    /// Cached GAMDL version string, populated once on first `process_queue()` call.
    /// Used to determine whether to use native `--song-codec-priority` (>= 2.9.1)
    /// or `MeedyaDL`'s own `try_fallback` system for older versions.
    gamdl_version: Option<String>,
    /// Timestamp of the last pre-flight health check run. Used to avoid running
    /// checks on every `process_queue()` call during a single batch (the function
    /// is called recursively for each item). Reset to `None` when the queue drains.
    /// A 60-second cooldown prevents duplicate warnings during rapid re-processing.
    last_preflight_at: Option<std::time::Instant>,
}

/// Thread-safe handle to the download queue, stored as Tauri managed state.
///
/// This type alias is used throughout the codebase when accessing the queue.
/// Tauri's `State<QueueHandle>` injector provides this to command handlers.
/// Ref: <https://v2.tauri.app/develop/calling-rust/#accessing-managed-state>
pub type QueueHandle = Arc<Mutex<DownloadQueue>>;

/// Creates a new queue handle for use as Tauri managed state.
/// Called once during app initialization (typically in main.rs setup).
#[must_use]
pub fn new_queue_handle() -> QueueHandle {
    Arc::new(Mutex::new(DownloadQueue::new()))
}

impl Default for DownloadQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: VecDeque::new(),
            max_concurrent: 1,
            active_count: 0,
            max_network_retries: 3,
            gamdl_version: None,
            last_preflight_at: None,
        }
    }

    /// Adds a new download to the queue and returns its unique ID.
    ///
    /// The download is placed at the back of the queue in the Queued state.
    /// The caller should call `process_queue()` after adding to start
    /// processing if slots are available.
    ///
    /// # Arguments
    /// * `request` - The download request with URLs and optional overrides
    /// * `settings` - Current app settings for merging default options
    ///
    /// # Returns
    /// The unique download ID for tracking this job.
    pub fn enqueue(&mut self, request: DownloadRequest, settings: &AppSettings) -> String {
        // Generate a unique download ID using UUID v4.
        // This ID is used to track the download across the queue, events, and frontend.
        let download_id = uuid::Uuid::new_v4().to_string();

        // Merge per-download overrides (from the frontend's "custom options" UI)
        // with global settings to produce the final set of GAMDL options.
        // For example, a user might override the codec for a specific download
        // while keeping the global output path from settings.
        let merged_options = merge_options(request.options.as_ref(), settings);

        // Detect which media service this URL belongs to (Apple Music, Spotify, etc.)
        // and resolve the primary download engine from the engine registry.
        let first_url = request.urls.first().map(String::as_str).unwrap_or("");
        let detected_service = crate::models::media_service::MediaServiceId::from_url(first_url);
        let service_str = detected_service.as_ref().map(std::string::ToString::to_string);

        // Resolve the primary engine for this service via the engine registry
        let engine_str = detected_service.as_ref().and_then(|svc| {
            let registry = crate::services::engine_registry::EngineRegistry::load();
            let platform_id = svc.to_string();
            registry.resolve_engine(&platform_id).map(|e| e.id.clone())
        });

        let item = QueueItem {
            status: {
                // Extract album name and artist from URL at enqueue time for
                // immediate display in the progress bar and queue list.
                let (album_name, artist_name) = extract_album_info_from_url(
                    request.urls.first().map(String::as_str).unwrap_or(""),
                );

                QueueItemStatus {
                    id: download_id.clone(),
                    urls: request.urls.clone(),
                    service: service_str,
                    engine: engine_str,
                    state: DownloadState::Queued,
                    progress: 0.0,
                    current_track: None,
                    album_name,
                    artist_name,
                    total_tracks: None,
                    completed_tracks: None,
                    speed: None,
                    eta: None,
                    processing_label: None,
                    error: None,
                    output_path: None,
                    codec_used: Some(merged_options.song_codec.as_ref().map_or_else(
                        || settings.default_song_codec.to_cli_string().to_string(),
                        |c| c.to_cli_string().to_string(),
                    )),
                    fallback_occurred: false,
                    used_wrapper: merged_options.use_wrapper.unwrap_or(false),
                    output_is_directory: false,
                    warnings: Vec::new(),
                    audio_traits: Vec::new(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                }
            },
            request,
            merged_options,
            fallback_index: 0,
            network_retries_left: self.max_network_retries,
            engine_fallback_index: 0,
        };

        log::info!(
            "Enqueued download {} ({} URL(s))",
            download_id,
            item.status.urls.len()
        );

        self.items.push_back(item);
        download_id
    }

    /// Checks whether any of the given URLs already exist in the queue in an
    /// active or pending state (Queued, Downloading, or Processing).
    ///
    /// Returns `true` if at least one URL matches an existing queue item.
    /// Completed, cancelled, and errored items are ignored since those are
    /// effectively inert and cleared on restart.
    ///
    /// URL comparison uses [`normalize_url_for_dedup`] for case-insensitive,
    /// trailing-slash-insensitive, and query-parameter-stripped matching.
    #[must_use]
    pub fn has_duplicate_urls(&self, urls: &[String]) -> bool {
        // Build a set of normalised incoming URLs for O(n+m) comparison.
        let incoming: HashSet<String> = urls.iter().map(|u| normalize_url_for_dedup(u)).collect();

        self.items.iter().any(|item| {
            // Only check active/pending items — terminal states are irrelevant.
            matches!(
                item.status.state,
                DownloadState::Queued | DownloadState::Downloading | DownloadState::Processing
            ) && item
                .status
                .urls
                .iter()
                .any(|u| incoming.contains(&normalize_url_for_dedup(u)))
        })
    }

    /// Returns the public status of all queue items for display in the frontend.
    /// The frontend calls this (via a Tauri command) to render the queue list.
    /// Returns cloned statuses to avoid holding the lock during serialization.
    #[must_use]
    pub fn get_status(&self) -> Vec<QueueItemStatus> {
        self.items.iter().map(|item| item.status.clone()).collect()
    }

    /// Returns summary counts for the queue: (total, active, queued, completed, failed).
    /// Used by the frontend to display queue statistics in the header/badge.
    #[must_use]
    pub fn get_counts(&self) -> (usize, usize, usize, usize, usize) {
        let total = self.items.len();
        // Active includes both Downloading and Processing states
        let active = self
            .items
            .iter()
            .filter(|i| {
                i.status.state == DownloadState::Downloading
                    || i.status.state == DownloadState::Processing
            })
            .count();
        let queued = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Queued)
            .count();
        let completed = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Complete)
            .count();
        let failed = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Error)
            .count();
        (total, active, queued, completed, failed)
    }

    /// Cancels a download by ID.
    ///
    /// If the download is queued, it's moved to the Cancelled state.
    /// If it's active, we mark it cancelled (the running task will check this).
    ///
    /// # Returns
    /// `true` if the item was found, `false` otherwise.
    pub fn cancel(&mut self, download_id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match item.status.state {
                DownloadState::Queued => {
                    item.status.state = DownloadState::Cancelled;
                    log::info!("Download {download_id} cancelled (was queued)");
                    true
                }
                DownloadState::Downloading | DownloadState::Processing => {
                    item.status.state = DownloadState::Cancelled;
                    // The active_count will be decremented when the running task
                    // detects the cancellation and stops
                    log::info!("Download {download_id} marked for cancellation");
                    true
                }
                _ => {
                    log::debug!("Download {download_id} already in terminal state");
                    false
                }
            }
        } else {
            log::warn!("Download {download_id} not found in queue");
            false
        }
    }

    /// Removes completed and cancelled items from the queue.
    /// Errored items are kept so the user can review and retry them.
    ///
    /// # Returns
    /// Number of items removed.
    pub fn clear_finished(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|item| {
            !matches!(
                item.status.state,
                DownloadState::Complete | DownloadState::Cancelled
            )
        });
        let removed = before - self.items.len();
        if removed > 0 {
            log::info!("Cleared {removed} finished items from queue");
        }
        removed
    }

    /// Removes ALL non-active items from the queue (completed, cancelled,
    /// errored, and queued). Active downloads (Downloading/Processing) are
    /// kept to avoid interrupting in-progress work.
    ///
    /// # Returns
    /// Number of items removed.
    pub fn clear_all(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|item| {
            matches!(
                item.status.state,
                DownloadState::Downloading | DownloadState::Processing
            )
        });
        let removed = before - self.items.len();
        if removed > 0 {
            log::info!("Cleared all {removed} items from queue (active downloads preserved)");
        }
        removed
    }

    /// Updates the state of a queue item.
    /// Used by the download task to report progress.
    pub fn update_item_state(&mut self, download_id: &str, state: DownloadState) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = state;
        }
    }

    /// Updates progress information for a queue item based on a parsed GAMDL event.
    ///
    /// Called by the stdout/stderr reader tasks in `run_download_with_events()`
    /// each time a line is parsed from GAMDL's output. The event type determines
    /// which status fields are updated:
    ///
    /// - `DownloadProgress`: Updates percentage, speed, ETA (shown in progress bar)
    /// - `TrackInfo`: Updates current track name (shown above progress bar)
    /// - `ProcessingStep`: Transitions state to Processing (e.g., remuxing, tagging)
    /// - Complete: Sets output path and 100% progress
    /// - Error: Records the error message for display
    pub fn update_item_progress(&mut self, download_id: &str, event: &process::GamdlOutputEvent) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match event {
                process::GamdlOutputEvent::DownloadProgress {
                    percent,
                    speed,
                    eta,
                } => {
                    // Update real-time progress metrics from GAMDL's tqdm-style progress bar
                    item.status.progress = *percent;
                    item.status.speed = Some(speed.clone());
                    item.status.eta = Some(eta.clone());
                    item.status.state = DownloadState::Downloading;
                }
                process::GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                    // Format the current track as "Artist - Title" or just "Title"
                    let track_name = if artist.is_empty() {
                        title.clone()
                    } else {
                        format!("{artist} - {title}")
                    };
                    item.status.current_track = Some(track_name);
                }
                process::GamdlOutputEvent::ProcessingStep { .. } => {
                    // Processing state covers post-download steps like remuxing,
                    // metadata tagging, and cover art embedding
                    item.status.state = DownloadState::Processing;
                }
                process::GamdlOutputEvent::Complete { path } => {
                    // Set the output file/directory path for the "Open" button in the UI
                    item.status.output_path = Some(path.clone());
                    item.status.progress = 100.0;
                }
                process::GamdlOutputEvent::Error { message } => {
                    // Record the error but don't change state yet — the process
                    // may still be running and the error handling in process_queue()
                    // will determine the final state (retry, fallback, or Error).
                    item.status.error = Some(message.clone());
                }
                // Unknown events are raw GAMDL output lines that don't match
                // any recognized pattern; they're logged for debugging but
                // don't affect queue item state.
                process::GamdlOutputEvent::Unknown { .. } => {}
            }
        }
    }

    /// Marks a download as errored and sets the error message.
    pub fn set_error(&mut self, download_id: &str, error: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = DownloadState::Error;
            item.status.error = Some(error.to_string());
        }
    }

    /// Marks a download as in post-processing (enrichment, companions, etc.).
    /// The item stays in this state until all background tasks finish.
    pub fn set_processing(&mut self, download_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = DownloadState::Processing;
        }
    }

    /// Updates the processing label for a download item.
    /// Shows what's currently happening during Processing state.
    pub fn set_processing_label(&mut self, download_id: &str, label: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.processing_label = Some(label.to_string());
        }
    }

    /// Clears the processing label for a download. Called by the
    /// companion supervisor when the post-processing phase finishes
    /// (#503), so the queue UI returns to its normal caption.
    pub fn clear_processing_label(&mut self, download_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.processing_label = None;
        }
    }

    /// Marks a download as complete (#416).
    /// Clears processing label and speed/ETA to prevent stale data
    /// appearing in the UI after completion.
    pub fn set_complete(&mut self, download_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = DownloadState::Complete;
            item.status.progress = 100.0;
            item.status.processing_label = None;
            item.status.speed = None;
            item.status.eta = None;
        }
    }

    /// Appends non-fatal warnings to a download item. These are displayed
    /// in the queue UI as amber text below the URL, indicating that the
    /// download succeeded but encountered issues during the run.
    pub fn add_warnings(&mut self, download_id: &str, warnings: &[String]) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.warnings.extend(warnings.iter().cloned());
        }
    }

    /// Checks if a download should attempt a fallback codec/resolution.
    ///
    /// The fallback chain is defined in `AppSettings::music_fallback_chain`, e.g.:
    /// `[Alac, AacHe, AacBinaural]`
    ///
    /// On each codec error, we advance to the next codec in the chain.
    /// This handles the case where Apple Music doesn't offer a track in the
    /// user's preferred codec (e.g., ALAC not available for all tracks).
    ///
    /// The item is reset to Queued state so `process_queue()` will pick it up again.
    ///
    /// # Returns
    /// `Some((new_options, fallback_index, chain_len))` if fallback should be
    /// attempted. `fallback_index` is 1-indexed (first fallback = 1, since
    /// index 0 is the initial codec). `chain_len` is the total length of the
    /// fallback chain. Returns `None` if all fallbacks are exhausted.
    pub fn try_fallback(
        &mut self,
        download_id: &str,
        settings: &AppSettings,
    ) -> Option<(GamdlOptions, usize, usize)> {
        let item = self.items.iter_mut().find(|i| i.status.id == download_id)?;

        // Only attempt fallback if the user has enabled it in settings
        if !settings.fallback_enabled {
            return None;
        }

        // Advance to the next codec in the fallback chain
        item.fallback_index += 1;

        if item.fallback_index < settings.music_fallback_chain.len() {
            // Get the next codec to try from the fallback chain
            let next_codec = &settings.music_fallback_chain[item.fallback_index];
            let mut new_options = item.merged_options.clone();
            // Clear song_codec — GAMDL >= 2.9.1 removed the --song-codec flag.
            // We use --song-codec-priority with a single codec instead.
            new_options.song_codec = None;

            // Override song_codec_priority with just this single codec.
            // This serves two purposes:
            // 1. Prevents process_download_item() from rebuilding the full
            //    priority chain (the is_none() check will be false).
            // 2. Overrides the config.ini song_codec_priority (which still
            //    has the full chain) so GAMDL tries only this one codec.
            // Using --song-codec-priority with one codec is equivalent to
            // --song-codec but also overrides the config.ini key.
            new_options.song_codec_priority = Some(next_codec.to_cli_string().to_string());

            // If the companion mode would produce companions for this fallback
            // codec, apply the codec suffix to file templates so the specialist
            // format files don't collide with the companion files.
            if needs_primary_suffix(
                next_codec,
                &settings.companion_mode,
                &settings.custom_companion_codecs,
            ) {
                apply_codec_suffix(&mut new_options);
            }

            // Update tracking info for the frontend to display
            item.status.codec_used = Some(next_codec.to_cli_string().to_string());
            item.status.fallback_occurred = true;
            // Reset the item to Queued so process_queue() will start it again
            item.status.state = DownloadState::Queued;
            item.status.error = None;
            item.status.progress = 0.0;
            item.merged_options = new_options.clone();

            let chain_len = settings.music_fallback_chain.len();
            log::info!(
                "Download {} falling back to codec: {} (fallback {} of {})",
                download_id,
                next_codec.to_cli_string(),
                item.fallback_index,
                chain_len.saturating_sub(1),
            );

            Some((new_options, item.fallback_index, chain_len))
        } else {
            // All codecs in the fallback chain have been tried and failed.
            // The download will remain in the Error state.
            log::info!("Download {download_id} exhausted all fallback codecs");
            None
        }
    }

    /// Attempts to fall back to the next engine in the platform's engine chain.
    ///
    /// When the primary engine fails with a **tool error** (binary missing,
    /// crash, unsupported format), the download queue can try the next engine
    /// in the platform's priority order (defined in engines.toml).
    ///
    /// **Network and auth errors skip engine fallback** — if the network is
    /// down or credentials are invalid, a different engine won't help.
    ///
    /// # Returns
    /// `Some((engine_id, fallback_index, chain_len))` if another engine is
    /// available. Returns `None` if all engines have been tried or the
    /// platform has no fallback engines.
    pub fn try_engine_fallback(
        &mut self,
        download_id: &str,
    ) -> Option<(String, usize, usize)> {
        let item = self.items.iter_mut().find(|i| i.status.id == download_id)?;

        // Resolve the engine chain for this item's service
        let service_id = item.status.service.as_deref()?;
        let registry = crate::services::engine_registry::EngineRegistry::load();
        let chain = registry.resolve_engine_chain(service_id);

        if chain.len() <= 1 {
            // Single engine or unknown platform — no fallback possible
            return None;
        }

        // Advance to the next engine in the chain
        item.engine_fallback_index += 1;

        if item.engine_fallback_index < chain.len() {
            let next_engine = &chain[item.engine_fallback_index];
            let next_engine_id = next_engine.id.clone();

            // Update the queue item's engine field
            item.status.engine = Some(next_engine_id.clone());
            // Reset the item to Queued so process_queue() will start it again
            item.status.state = DownloadState::Queued;
            item.status.error = None;
            item.status.progress = 0.0;

            let chain_len = chain.len();
            log::info!(
                "Download {} engine fallback: trying {} (fallback {} of {})",
                download_id,
                next_engine_id,
                item.engine_fallback_index,
                chain_len.saturating_sub(1),
            );

            Some((next_engine_id, item.engine_fallback_index, chain_len))
        } else {
            log::info!("Download {download_id} exhausted all engine fallbacks");
            None
        }
    }

    /// Checks if a download should retry due to a network error.
    ///
    /// # Returns
    /// `Some((attempt, total))` — 1-indexed attempt number and total attempts
    /// (initial + retries). For example, with `max_network_retries = 3`,
    /// total = 4 and attempts are 2, 3, 4 (attempt 1 was the initial try).
    /// Returns `None` if retries are exhausted or the item doesn't exist.
    pub fn try_network_retry(&mut self, download_id: &str) -> Option<(u32, u32)> {
        let max = self.max_network_retries;
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.network_retries_left > 0 {
                item.network_retries_left -= 1;
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                // Total attempts = initial try + max retries.
                // Attempt number = total - retries_left (after decrement).
                let total = max + 1;
                let attempt = total - item.network_retries_left;
                log::info!(
                    "Download {} network retry (attempt {} of {}, {} remaining)",
                    download_id,
                    attempt,
                    total,
                    item.network_retries_left
                );
                Some((attempt, total))
            } else {
                log::info!("Download {download_id} exhausted network retries");
                None
            }
        } else {
            None
        }
    }

    /// Gets the next queued item's download ID and options for execution.
    ///
    /// This is the "scheduler" — it decides whether a new download can start.
    /// Returns None if:
    /// - No items are in the Queued state
    /// - The max concurrent limit has been reached
    ///
    /// When an item is selected, it transitions from Queued -> Downloading
    /// and the active count is incremented. The caller (`process_queue`) must
    /// eventually call `on_task_finished()` when the download completes.
    pub fn next_pending(&mut self) -> Option<(String, Vec<String>, GamdlOptions, Option<String>)> {
        // Check if we're at the concurrent download limit
        if self.active_count >= self.max_concurrent {
            return None;
        }

        // Find the first Queued item (FIFO order from VecDeque front)
        let item = self
            .items
            .iter_mut()
            .find(|i| i.status.state == DownloadState::Queued)?;
        // Transition to Downloading and increment active count
        item.status.state = DownloadState::Downloading;
        self.active_count += 1;

        // Return the data needed to start the download, including the
        // detected service ID for service-aware routing (#318).
        Some((
            item.status.id.clone(),
            item.status.urls.clone(),
            item.merged_options.clone(),
            item.status.service.clone(),
        ))
    }

    /// Called when a download task finishes (success, error, or cancel).
    /// Decrements the active count so new downloads can start.
    /// This must be called exactly once per `next_pending()` call to keep
    /// the `active_count` accurate. The guard `if self.active_count > 0`
    /// prevents underflow in edge cases.
    pub const fn on_task_finished(&mut self) {
        if self.active_count > 0 {
            self.active_count -= 1;
        }
    }

    /// Returns true when the queue has no active or pending downloads.
    /// Used to trigger after-queue actions.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.active_count == 0 && !self.items.iter().any(|i| i.status.state == DownloadState::Queued)
    }

    /// Checks if a download has been cancelled by the user.
    /// Called by the cancellation polling loop in `run_download_with_events()`
    /// every 250ms to detect if the user cancelled while the process is running.
    /// If true, the caller should kill the GAMDL subprocess.
    #[must_use]
    pub fn is_cancelled(&self, download_id: &str) -> bool {
        self.items
            .iter()
            .find(|i| i.status.id == download_id)
            .is_some_and(|i| i.status.state == DownloadState::Cancelled)
    }

    /// Retries a failed or cancelled download by fully resetting it to the Queued state.
    ///
    /// This is a "full reset" — the download starts from scratch with fresh options
    /// (re-merged from the original request + current settings), a reset fallback
    /// index, and full network retry budget. This differs from automatic retries
    /// (`try_fallback`, `try_network_retry`) which only adjust specific fields.
    ///
    /// Called by the frontend's "Retry" button via a Tauri command.
    ///
    /// # Returns
    /// `true` if the item was found and reset, `false` otherwise.
    pub fn retry(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.status.state == DownloadState::Error
                || item.status.state == DownloadState::Cancelled
            {
                // Re-merge options from the original request with current settings.
                // This picks up any settings changes the user made since the original attempt.
                item.merged_options = merge_options(item.request.options.as_ref(), settings);
                // Reset fallback and retry counters to their initial values
                item.fallback_index = 0;
                item.network_retries_left = self.max_network_retries;
                // Reset status fields for a fresh start
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                item.status.fallback_occurred = false;
                item.status.used_wrapper = item.merged_options.use_wrapper.unwrap_or(false);
                item.status.codec_used = Some(item.merged_options.song_codec.as_ref().map_or_else(
                    || settings.default_song_codec.to_cli_string().to_string(),
                    |c| c.to_cli_string().to_string(),
                ));
                log::info!("Download {download_id} reset for retry");
                return true;
            }
        }
        false
    }

    /// Retries a failed download with wrapper authentication disabled.
    ///
    /// Clones the item's original request, disables wrapper in the merged
    /// options, and resets the item to Queued state. This allows users to
    /// fall back to cookie-based authentication when the wrapper service
    /// is down or misconfigured.
    ///
    /// Only applies to items that were originally attempted with wrapper
    /// enabled and are in an error or cancelled state.
    ///
    /// # Arguments
    /// * `download_id` - The unique ID of the failed download to retry
    /// * `settings` - Current app settings for option re-merging
    ///
    /// # Returns
    /// `true` if the item was found, was wrapper-enabled, and was reset.
    pub fn retry_without_wrapper(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if (item.status.state == DownloadState::Error
                || item.status.state == DownloadState::Cancelled)
                && item.status.used_wrapper
            {
                // Re-merge options from the original request with current settings
                item.merged_options = merge_options(item.request.options.as_ref(), settings);
                // Override wrapper settings: disable wrapper, clear wrapper URLs
                item.merged_options.use_wrapper = Some(false);
                item.merged_options.wrapper_account_url = None;
                item.merged_options.wrapper_decrypt_ip = None;
                // Reset counters and state
                item.fallback_index = 0;
                item.network_retries_left = self.max_network_retries;
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                item.status.fallback_occurred = false;
                item.status.used_wrapper = false;
                item.status.codec_used = Some(item.merged_options.song_codec.as_ref().map_or_else(
                    || settings.default_song_codec.to_cli_string().to_string(),
                    |c| c.to_cli_string().to_string(),
                ));
                log::info!("Download {download_id} reset for retry without wrapper");
                return true;
            }
        }
        false
    }

    // ==========================================================
    // Pre-flight health check helpers
    // ==========================================================

    /// Returns true if pre-flight checks should run.
    ///
    /// Pre-flight checks run once per queue batch (not per-item) with a
    /// 60-second cooldown to prevent duplicate warnings when `process_queue()`
    /// is called recursively for cascading items.
    #[must_use]
    pub fn should_run_preflight(&self) -> bool {
        match self.last_preflight_at {
            None => true,
            Some(t) => t.elapsed() > std::time::Duration::from_secs(60),
        }
    }

    /// Marks that pre-flight checks have been run, starting the cooldown timer.
    pub fn mark_preflight_run(&mut self) {
        self.last_preflight_at = Some(std::time::Instant::now());
    }

    // ==========================================================
    // Persistence and export/import methods
    // ==========================================================

    /// Returns persistable snapshots of all non-terminal queue items.
    ///
    /// Called by `save_queue_to_disk()` to capture queue state for crash recovery.
    /// Only items in Queued, Downloading, or Processing states are included;
    /// completed/failed/cancelled items are not persisted (they are cleared
    /// on restart per the user's preference).
    #[must_use]
    pub fn get_persistable_items(&self) -> Vec<PersistedQueueItem> {
        self.items
            .iter()
            .filter(|item| {
                // Persist active items AND failed items. Only Complete and
                // Cancelled are discarded — errored items stay so the user
                // can retry them after restarting the app.
                matches!(
                    item.status.state,
                    DownloadState::Queued
                        | DownloadState::Downloading
                        | DownloadState::Processing
                        | DownloadState::Error
                )
            })
            .map(|item| PersistedQueueItem {
                id: item.status.id.clone(),
                request: item.request.clone(),
                created_at: item.status.created_at.clone(),
                error: item.status.error.clone(),
                service: item.status.service.clone(),
            })
            .collect()
    }

    /// Restores items from persisted data, re-merging with current settings.
    ///
    /// Called during startup to recover the queue after a crash or app close.
    /// Active items (Queued/Downloading/Processing) are reset to Queued so
    /// they re-download from scratch. Failed items (those with a persisted
    /// error message) are restored in Error state so the user can review the
    /// failure reason and manually retry — they are not auto-retried.
    ///
    /// Options are re-merged with the current device's settings so any
    /// changes made since the last session are respected.
    ///
    /// # Arguments
    /// * `persisted` - The items loaded from `queue.json`
    /// * `settings` - The current app settings for option merging
    pub fn restore_items(&mut self, persisted: Vec<PersistedQueueItem>, settings: &AppSettings) {
        for p in persisted {
            // Re-merge the original request's overrides with the current settings.
            // This ensures setting changes made between sessions are respected.
            let merged_options = merge_options(p.request.options.as_ref(), settings);

            // Items with a persisted error are restored in Error state so the
            // user sees the failure reason and can choose to retry. Active
            // items are reset to Queued for automatic re-processing.
            let (state, error) = if p.error.is_some() {
                (DownloadState::Error, p.error)
            } else {
                (DownloadState::Queued, None)
            };

            // Restore or re-detect service/engine from persisted data.
            // Older persisted items may not have the service field, so
            // fall back to URL-based detection for backwards compatibility.
            let service_str = p.service.or_else(|| {
                let url = p.request.urls.first()?;
                crate::models::media_service::MediaServiceId::from_url(url)
                    .map(|svc| svc.to_string())
            });
            let engine_str = service_str.as_ref().and_then(|svc_id| {
                let registry = crate::services::engine_registry::EngineRegistry::load();
                registry.resolve_engine(svc_id).map(|e| e.id.clone())
            });

            let (album_name, artist_name) = extract_album_info_from_url(
                p.request.urls.first().map(String::as_str).unwrap_or(""),
            );

            let item = QueueItem {
                status: QueueItemStatus {
                    id: p.id.clone(),
                    urls: p.request.urls.clone(),
                    service: service_str,
                    engine: engine_str,
                    state,
                    progress: 0.0,
                    current_track: None,
                    album_name,
                    artist_name,
                    total_tracks: None,
                    completed_tracks: None,
                    speed: None,
                    eta: None,
                    processing_label: None,
                    error,
                    output_path: None,
                    codec_used: Some(merged_options.song_codec.as_ref().map_or_else(
                        || settings.default_song_codec.to_cli_string().to_string(),
                        |c| c.to_cli_string().to_string(),
                    )),
                    fallback_occurred: false,
                    used_wrapper: merged_options.use_wrapper.unwrap_or(false),
                    output_is_directory: false,
                    warnings: Vec::new(),
                    // Re-fetched from the Apple Music API on next attempt.
                    audio_traits: Vec::new(),
                    created_at: p.created_at,
                },
                request: p.request,
                merged_options,
                fallback_index: 0,
                engine_fallback_index: 0,
                network_retries_left: self.max_network_retries,
            };
            self.items.push_back(item);
        }
        if !self.items.is_empty() {
            log::info!(
                "Restored {} item(s) from queue persistence",
                self.items.len()
            );
        }
    }

    /// Returns exportable items for the `.meedyadl` export file format.
    ///
    /// Includes all non-terminal items (Queued/Downloading/Processing).
    /// Each item contains only the original URLs and per-download overrides,
    /// so the importing device will merge them with its own settings.
    #[must_use]
    pub fn get_exportable_items(&self) -> Vec<ExportedItem> {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.status.state,
                    DownloadState::Queued | DownloadState::Downloading | DownloadState::Processing
                )
            })
            .map(|item| ExportedItem {
                urls: item.request.urls.clone(),
                options: item.request.options.clone(),
            })
            .collect()
    }

    /// Imports items from an export file, enqueuing each as a new download.
    ///
    /// Each imported item is treated as a fresh download request: a new UUID
    /// is generated, options are merged with the importing device's current
    /// settings, and the item is placed at the back of the queue.
    ///
    /// # Returns
    /// The download IDs of the newly created queue items.
    pub fn import_items(
        &mut self,
        items: Vec<ExportedItem>,
        settings: &AppSettings,
    ) -> Vec<String> {
        items
            .into_iter()
            .map(|exported| {
                let request = DownloadRequest {
                    urls: exported.urls,
                    options: exported.options,
                };
                self.enqueue(request, settings)
            })
            .collect()
    }
}

// ============================================================
// Helper: merge per-download overrides with global settings
// ============================================================

/// Merges per-download option overrides with the global app settings
/// to produce the final set of GAMDL CLI options.
///
/// The merge follows a two-layer priority system:
/// 1. **Global settings** (from `AppSettings`) form the base layer
/// 2. **Per-download overrides** (from the frontend) override specific fields
///
/// This allows users to set global defaults (e.g., always use ALAC) while
/// still customizing individual downloads (e.g., this one in AAC-HE).
///
/// The resulting `GamdlOptions` struct is what actually gets passed to
/// `gamdl_service::build_gamdl_command_public()` to construct the CLI command.
#[allow(clippy::field_reassign_with_default)]
fn merge_options(overrides: Option<&GamdlOptions>, settings: &AppSettings) -> GamdlOptions {
    let mut options = GamdlOptions::default();

    // === Layer 1: Apply global settings as the base ===
    // These come from the user's saved settings (settings.json).
    options.song_codec = Some(settings.default_song_codec.clone());
    options.music_video_resolution = Some(settings.default_video_resolution.clone());
    options.music_video_codec_priority = Some(settings.default_video_codec_priority.clone());
    options.music_video_remux_format = Some(settings.default_video_remux_format.clone());
    options.synced_lyrics_format = Some(settings.synced_lyrics_format.clone());
    options.no_synced_lyrics = Some(settings.no_synced_lyrics);
    options.synced_lyrics_only = Some(settings.synced_lyrics_only);
    options.save_cover = Some(settings.save_cover);
    options.cover_format = Some(settings.cover_format.clone());
    options.cover_size = Some(settings.cover_size);
    options.overwrite = Some(settings.overwrite);
    options.language = Some(settings.language.clone());
    options.album_folder_template = Some(settings.album_folder_template.clone());
    options.compilation_folder_template = Some(settings.compilation_folder_template.clone());
    options.no_album_folder_template = Some(settings.no_album_folder_template.clone());
    options.single_disc_file_template = Some(settings.single_disc_file_template.clone());
    options.multi_disc_file_template = Some(settings.multi_disc_file_template.clone());
    options.no_album_file_template = Some(settings.no_album_file_template.clone());
    options.playlist_file_template = Some(settings.playlist_file_template.clone());
    options.use_wrapper = Some(settings.use_wrapper);
    options.wrapper_account_url = Some(settings.wrapper_account_url.clone());
    options.truncate = settings.truncate;

    if !settings.output_path.is_empty() {
        // Validate output path for traversal sequences (#459)
        match super::config_service::validate_path_safe(&settings.output_path) {
            Ok(_) => {
                options.output_path = Some(settings.output_path.clone());
            }
            Err(e) => {
                log::warn!("Output path rejected: {e}");
            }
        }
    }

    // Resolve temp_path: use the user's custom path if set, otherwise fall
    // back to a "MeedyaDL" subdirectory within the OS temp directory (e.g.
    // /var/folders/.../MeedyaDL on macOS, %TEMP%\MeedyaDL on Windows,
    // /tmp/MeedyaDL on Linux). Using a dedicated subdirectory keeps
    // intermediate files isolated and easy to clean up. This avoids GAMDL's
    // default of "." which is unwritable from /Applications on macOS.
    if settings.temp_path.is_empty() {
        options.temp_path = Some(
            std::env::temp_dir()
                .join("MeedyaDL")
                .to_string_lossy()
                .to_string(),
        );
    } else {
        options.temp_path = Some(settings.temp_path.clone());
    }

    // Apply tool paths from settings
    options.cookies_path.clone_from(&settings.cookies_path);
    options.ffmpeg_path.clone_from(&settings.ffmpeg_path);
    options
        .mp4decrypt_path
        .clone_from(&settings.mp4decrypt_path);
    options.mp4box_path.clone_from(&settings.mp4box_path);
    options.nm3u8dlre_path.clone_from(&settings.nm3u8dlre_path);

    // Set download and remux modes
    options.download_mode = Some(settings.download_mode.clone());
    options.remux_mode = Some(settings.remux_mode.clone());

    // Apply metadata options.
    //
    // `fetch_extra_tags` is version-gated: the CLI flag exists in every
    // v2.x release but was removed in GAMDL v3.0 ("Remove extra tags
    // fetching and preview parsing"). Emitting `--fetch-extra-tags` on
    // v3+ crashes the subprocess with a Click "no such option" error, so
    // we leave the field as `None` there — `GamdlOptions::to_cli_args()`
    // only emits the flag when the value is `Some(true)`.
    if super::gamdl_capabilities::supports(
        super::gamdl_capabilities::GamdlFeature::FetchExtraTags,
    ) {
        options.fetch_extra_tags = Some(settings.fetch_extra_tags);
    }

    // Default to `--no-exceptions` so GAMDL prints a single user-facing
    // line per failure instead of a full Python traceback. GAMDL v3.0
    // interleaves structlog-formatted lines with raw multi-line
    // tracebacks, which makes the activity log unreadable and confuses
    // `classify_error()` (frame paths like `httpx/_transports/default.py`
    // pick up the "Error" keyword falsely). Users debugging upstream
    // issues can flip the `verbose_gamdl_exceptions` setting to get the
    // full stack trace back; in that case we leave `no_exceptions` as
    // `None` so `to_cli_args()` never emits the flag.
    if !settings.verbose_gamdl_exceptions {
        options.no_exceptions = Some(true);
    }

    // Apply exclude tags
    if !settings.exclude_tags.is_empty() {
        options.exclude_tags = Some(settings.exclude_tags.join(","));
    }

    // Apply artist auto-selection mode (GAMDL >= 2.9.1)
    options
        .artist_auto_select
        .clone_from(&settings.artist_auto_select);

    // === Layer 2: Apply per-download overrides (highest priority) ===
    // Only non-None fields from the override replace the global values.
    // This selective merge allows partial overrides (e.g., only change codec).
    if let Some(overrides) = overrides {
        if overrides.song_codec.is_some() {
            options.song_codec.clone_from(&overrides.song_codec);
        }
        if overrides.music_video_resolution.is_some() {
            options
                .music_video_resolution
                .clone_from(&overrides.music_video_resolution);
        }
        if overrides.music_video_codec_priority.is_some() {
            options
                .music_video_codec_priority
                .clone_from(&overrides.music_video_codec_priority);
        }
        if overrides.music_video_remux_format.is_some() {
            options
                .music_video_remux_format
                .clone_from(&overrides.music_video_remux_format);
        }
        if overrides.output_path.is_some() {
            options.output_path.clone_from(&overrides.output_path);
        }
        if overrides.overwrite.is_some() {
            options.overwrite = overrides.overwrite;
        }
        if overrides.artist_auto_select.is_some() {
            options
                .artist_auto_select
                .clone_from(&overrides.artist_auto_select);
        }
    }

    // === Layer 3: Lyrics embed + sidecar enforcement ===
    // When the user has enabled "Embed Lyrics and Keep Sidecar", ensure that:
    // 1. Lyrics are NOT excluded from metadata embedding (remove "lyrics" from
    //    exclude_tags if present, so GAMDL embeds them in the audio file's tags).
    // 2. Synced lyrics sidecar files are NOT disabled (force no_synced_lyrics
    //    to false, so GAMDL creates the .lrc/.srt/.ttml sidecar alongside).
    // This provides maximum compatibility: embedded for players that read tags,
    // sidecar for those that look for external lyrics files.
    if settings.embed_lyrics_and_sidecar {
        // Remove "lyrics" from the exclude_tags comma-separated string so
        // GAMDL embeds lyrics in the audio file's metadata atoms.
        if let Some(ref tags) = options.exclude_tags {
            let filtered: Vec<&str> = tags
                .split(',')
                .map(str::trim)
                .filter(|t| !t.eq_ignore_ascii_case("lyrics"))
                .collect();
            if filtered.is_empty() {
                options.exclude_tags = None;
            } else {
                options.exclude_tags = Some(filtered.join(","));
            }
        }
        // Force sidecar lyrics creation regardless of the no_synced_lyrics setting.
        options.no_synced_lyrics = Some(false);
    }

    // === Layer 4: Enhanced LRC enforcement ===
    // When Enhanced LRC is enabled, force TTML as the primary lyrics format
    // so the raw word-level timing data is preserved in the sidecar file.
    // GAMDL's TTML output retains the full XML including <span> word timestamps,
    // which are then converted to Enhanced LRC in the enrichment pipeline.
    // This runs after Layer 3 so it overrides the lyrics format regardless
    // of what the user selected, ensuring TTML is always available for conversion.
    if settings.enhanced_lrc {
        options.synced_lyrics_format = Some(LyricsFormat::Ttml);
        // Ensure sidecar creation (needed for TTML → Enhanced LRC conversion)
        options.no_synced_lyrics = Some(false);
    }

    options
}

// ============================================================
// Helper: codec-based filename suffix system
// ============================================================

/// Returns the filename suffix for a given audio codec, or `None` if the
/// codec should use a clean (unsuffixed) filename.
///
/// Suffix rules:
/// - **Lossy codecs** (AAC, AAC-Legacy, AAC-Binaural, AC3, etc.) get no
///   suffix, as they represent the "standard" download a user would expect.
/// - **Lossless** (ALAC) gets `[Lossless]` to distinguish from lossy versions.
/// - **Spatial audio** (Dolby Atmos) gets `[Dolby Atmos]` to clearly identify
///   the immersive mix.
///
/// When companion downloads are enabled, multiple codec versions of the same
/// track can coexist in the same album folder. The suffix prevents filename
/// collisions and makes the format instantly visible in file browsers.
///
/// Suffixes are defined in `codecs.toml` under the `suffix` field of each
/// audio codec entry. This function delegates to `codec_suffix_from_registry()`
/// which looks up the suffix from the compiled-in registry data.
fn codec_suffix(codec: &SongCodec) -> Option<&'static str> {
    codec_suffix_from_registry(codec)
}

/// Determines whether the primary download's file templates should have a
/// codec suffix applied, based on the companion mode and the download's codec.
///
/// A suffix is needed when the companion mode will produce at least one
/// companion with a clean filename alongside the primary download. This
/// prevents filename collisions in the same album directory.
///
/// # Rules per mode
///
/// | Mode                       | Atmos gets suffix? | ALAC gets suffix? | Others? |
/// |----------------------------|--------------------|-------------------|---------|
/// | `Disabled`                 | No                 | No                | No      |
/// | `AtmosToLossless`          | Yes                | No                | No      |
/// | `AtmosToLosslessAndLossy`  | Yes                | Yes               | No      |
/// | `SpecialistToLossy`        | Yes                | Yes               | No      |
/// | `AtmosToAllFormats`        | Yes                | No                | No      |
fn needs_primary_suffix(
    codec: &SongCodec,
    mode: &CompanionMode,
    custom_codecs: &[SongCodec],
) -> bool {
    match mode {
        // No companions → no suffix needed (only one version exists)
        CompanionMode::Disabled => false,
        // Only Atmos gets a companion (ALAC or all formats), so only Atmos
        // needs a suffix. ALAC downloads in these modes have no companion → no suffix.
        CompanionMode::AtmosToLossless | CompanionMode::AtmosToAllFormats => {
            matches!(codec, SongCodec::Atmos)
        }
        // Both Atmos and ALAC get companions (at least a lossy one),
        // so both need suffixes to coexist with the clean-filename companion.
        // Same for SpecialistToLossy: any specialist codec gets a lossy companion.
        CompanionMode::AtmosToLosslessAndLossy | CompanionMode::SpecialistToLossy => {
            matches!(codec, SongCodec::Atmos | SongCodec::Alac)
        }
        // Custom mode: primary needs suffix if at least one companion will
        // have a clean filename (to prevent collisions). This is the case
        // when the custom codec list is non-empty and the primary codec is
        // not already in the custom list (the primary always gets a suffix
        // when custom companions exist).
        CompanionMode::Custom => {
            if custom_codecs.is_empty() {
                return false;
            }
            // Primary always gets a suffix when custom companions are active,
            // because at least one companion will use a clean filename.
            true
        }
    }
}

/// A planned companion download tier. Each tier represents one additional
/// GAMDL invocation to download the same content in a different codec.
struct CompanionTier {
    /// Codecs to try in order for this companion tier. The first codec that
    /// succeeds ends the tier (remaining codecs are skipped). If all fail,
    /// the tier is skipped silently.
    codecs_to_try: Vec<SongCodec>,
    /// Whether to apply a codec suffix to this companion's file templates.
    /// `true` means this companion gets a suffixed filename (e.g., `[Lossless]`);
    /// `false` means this companion gets the clean (unsuffixed) filename.
    apply_suffix: bool,
}

/// Plans the companion downloads to perform after a primary download
/// succeeds, based on the companion mode and the primary codec used.
///
/// Returns an ordered list of `CompanionTier` structs. Each tier is
/// processed sequentially (to avoid concurrent GAMDL processes writing to
/// the same directory). Within a tier, codecs are tried in order until one
/// succeeds.
///
/// # Examples
///
/// `AtmosToLossless` with primary `"atmos"`:
/// → `[CompanionTier { codecs: [ALAC], suffix: false }]`
///
/// `AtmosToLosslessAndLossy` with primary `"atmos"`:
/// → `[CompanionTier { codecs: [ALAC], suffix: true },
///     CompanionTier { codecs: [AAC, AacLegacy], suffix: false }]`
/// Epoch-milliseconds helper used by `run_download_with_events` for
/// the primary GAMDL idle watchdog (#508). Local copy rather than
/// importing from `companion_supervisor` to avoid a tight coupling
/// between the two modules — the helper is trivially small.
fn now_epoch_ms_primary() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Reads the union of `audioTraits` for a queue item, returning an
/// empty `Vec` when the item is missing or has no traits captured.
/// Used at companion-spawn time to feed the audioTraits-aware filter
/// in `filter_tiers_by_audio_traits`.
async fn read_audio_traits(queue: &QueueHandle, dl_id: &str) -> Vec<String> {
    let q = queue.lock().await;
    q.items
        .iter()
        .find(|i| i.status.id == dl_id)
        .map(|i| i.status.audio_traits.clone())
        .unwrap_or_default()
}

/// Filters a planned companion-tier list against the union of
/// `audioTraits` reported by the Apple Music catalog API for this
/// download's tracks (#504).
///
/// A tier is kept when at least one of its codecs has either:
///   - no `required_audio_trait()` (the codec is derived from another
///     stream, e.g. binaural — leave the decision to GAMDL), or
///   - a required trait that appears in `available_traits`.
///
/// When `available_traits` is empty (API metadata wasn't reachable) we
/// pass every tier through unchanged so the existing best-effort
/// behaviour is preserved.
fn filter_tiers_by_audio_traits(
    tiers: Vec<CompanionTier>,
    available_traits: &[String],
) -> (Vec<CompanionTier>, Vec<String>) {
    if available_traits.is_empty() {
        return (tiers, Vec::new());
    }
    let mut skipped = Vec::new();
    let kept: Vec<CompanionTier> = tiers
        .into_iter()
        .filter(|tier| {
            let any_codec_supported = tier.codecs_to_try.iter().any(|c| match c.required_audio_trait() {
                None => true,
                Some(needed) => available_traits.iter().any(|t| t == needed),
            });
            if !any_codec_supported {
                let names: Vec<&str> = tier
                    .codecs_to_try
                    .iter()
                    .map(SongCodec::to_cli_string)
                    .collect();
                skipped.push(names.join(","));
            }
            any_codec_supported
        })
        .collect();
    (kept, skipped)
}

fn plan_companions(
    mode: &CompanionMode,
    primary_codec: &str,
    custom_codecs: &[SongCodec],
) -> Vec<CompanionTier> {
    match mode {
        // No companions in any scenario
        CompanionMode::Disabled => vec![],

        // Atmos → ALAC companion (clean filename); nothing for other codecs
        CompanionMode::AtmosToLossless => {
            if primary_codec == "atmos" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Alac],
                    apply_suffix: false, // ALAC companion gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Maximum coverage:
        //   Atmos → ALAC [Lossless] + AAC (clean)
        //   ALAC → AAC (clean)
        CompanionMode::AtmosToLosslessAndLossy => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix (AAC exists too)
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                        apply_suffix: false, // Lossy AAC gets clean filename
                    },
                ]
            } else if primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Any specialist → lossy companion (clean filename)
        CompanionMode::SpecialistToLossy => {
            if primary_codec == "atmos" || primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Atmos → all formats: AC3 [Dolby Digital] + ALAC [Lossless] + AAC (clean)
        // 4 files per track total (Atmos + 3 companions)
        CompanionMode::AtmosToAllFormats => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Ac3],
                        apply_suffix: true, // AC3 gets [Dolby Digital] suffix
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                        apply_suffix: false, // AAC gets clean filename
                    },
                ]
            } else {
                vec![]
            }
        }

        // Custom: each user-selected codec becomes its own tier.
        // The last tier gets the clean filename; earlier tiers get suffixes.
        // All user-selected codecs are included — even if one matches the
        // primary setting. With native priority the actual codec GAMDL picks
        // may differ from the requested primary, so the user's explicit
        // selections must be respected.
        CompanionMode::Custom => {
            let filtered: Vec<&SongCodec> = custom_codecs.iter().collect();

            if filtered.is_empty() {
                return vec![];
            }

            let last_idx = filtered.len() - 1;
            filtered
                .into_iter()
                .enumerate()
                .map(|(i, codec)| CompanionTier {
                    codecs_to_try: vec![codec.clone()],
                    // Last companion gets clean filename; others get suffix
                    apply_suffix: i != last_idx,
                })
                .collect()
        }
    }
}

/// Appends a codec-specific suffix to all file naming templates in a
/// `GamdlOptions` struct.
///
/// When companion downloads are enabled, multiple codec versions of the
/// same track land in the same album directory. To prevent filename
/// collisions and clearly identify the format, this function appends the
/// codec's suffix (e.g., ` [Lossless]` or ` [Dolby Atmos]`) to the
/// filename portion of every file template.
///
/// The most universally compatible companion uses the original (unsuffixed)
/// template, so it gets a "clean" filename (e.g., `01 Song Title.m4a`)
/// while specialist formats get tagged filenames (e.g.,
/// `01 Song Title [Lossless].m4a` or `01 Song Title [Dolby Atmos].m4a`).
///
/// This modifies the following templates:
/// - `single_disc_file_template` (most common: `{track:02d} {title}`)
/// - `multi_disc_file_template` (`{disc}-{track:02d} {title}`)
/// - `no_album_file_template` (`{title}`)
/// - `playlist_file_template` (`Playlists/{playlist_artist}/{playlist_title}`)
///
/// Returns `true` if a suffix was applied, `false` if the codec has no suffix.
fn apply_codec_suffix(options: &mut GamdlOptions) -> bool {
    // Determine the suffix for the current codec, if any.
    // Check song_codec first, then fall back to song_codec_priority
    // (companion downloads set song_codec=None and use
    // song_codec_priority with a single codec string instead).
    let suffix = if let Some(codec) = &options.song_codec {
        match codec_suffix(codec) {
            Some(s) => s,
            None => return false, // Lossy codecs get no suffix
        }
    } else if let Some(ref priority) = options.song_codec_priority {
        // Parse the first (or only) codec from the priority string
        let first_codec_str = priority.split(',').next().unwrap_or("");
        if let Some(codec) = SongCodec::from_cli_string(first_codec_str) {
            match codec_suffix(&codec) {
                Some(s) => s,
                None => return false,
            }
        } else {
            return false;
        }
    } else {
        return false; // No codec specified
    };

    // For each file template, append the suffix to the existing value.
    // If the template is None (not set), use the GAMDL default with the suffix.
    if let Some(ref template) = options.single_disc_file_template {
        options.single_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.single_disc_file_template = Some(format!("{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.multi_disc_file_template {
        options.multi_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.multi_disc_file_template =
            Some(format!("{{disc}}-{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.no_album_file_template {
        options.no_album_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.no_album_file_template = Some(format!("{{title}} {suffix}"));
    }

    if let Some(ref template) = options.playlist_file_template {
        options.playlist_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.playlist_file_template = Some(format!(
            "Playlists/{{playlist_artist}}/{{playlist_title}} {suffix}"
        ));
    }

    true
}

// ============================================================
// Music video companion downloads
// ============================================================

/// Downloads music videos as companions via the Apple Music API (Step 6).
///
/// Called inside the enrichment pipeline's async block (after Step 5: ReplayGain).
/// Queries the Apple Music API to find music videos related to the downloaded
/// tracks, then spawns a GAMDL invocation for each available music video.
///
/// This is a fire-and-forget operation — failures are logged but do not
/// affect the primary download status or queue progression. Gracefully
/// skips if MusicKit credentials are not configured — Step 6b (MusicBrainz)
/// provides a credential-free fallback path.
///
/// # Requirements
/// - `settings.music_video_companion` must be `true`
/// - MusicKit developer token required (user credentials or embedded build token)
/// - `album_metadata` provides song IDs (reused from enrichment Step 1)
async fn spawn_music_video_companion_inner(
    app: &tauri::AppHandle,
    dl_id: &str,
    urls: &[String],
    album_metadata: Option<&super::apple_music_api::AlbumMetadata>,
    settings: &crate::models::settings::AppSettings,
    shutdown: &ShutdownSignal,
) {
    // Early exit if the original URL is already a music-video URL
    // (no self-referencing companion).
    if let Some(first_url) = urls.first() {
        if let Some(parsed) = super::apple_music_api::parse_apple_music_url(first_url) {
            if parsed.content_type == "music-video" {
                log::debug!(
                    "Music video companion skipped for {dl_id}: URL is already a music-video"
                );
                return;
            }
        }
    }

    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match super::apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        _ => None,
    };

    // Extract storefront from the original URL
    let storefront = match urls
        .first()
        .and_then(|u| super::apple_music_api::parse_apple_music_url(u))
    {
        Some(parsed) => parsed.storefront,
        None => {
            log::debug!(
                "Music video companion skipped for {dl_id}: could not parse URL storefront"
            );
            return;
        }
    };

    // Extract song IDs from the enrichment Step 1 metadata (already fetched once
    // and passed here to avoid duplicate API calls). These IDs are used to query
    // which songs have available music videos on Apple Music.
    let song_ids: Vec<String> = match album_metadata {
        Some(meta) => meta.tracks.iter().map(|t| t.song_id.clone()).collect(),
        None => {
            log::debug!("Music video companion skipped for {dl_id}: no album metadata available");
            return;
        }
    };

    if song_ids.is_empty() {
        log::debug!("Music video companion skipped for {dl_id}: no tracks in metadata");
        return;
    }

    emit_download_log(
        app,
        dl_id,
        &format!("Looking up music videos for {} track(s)...", song_ids.len()),
    );

    // Resolve MusicKit token for API call (premium feature resolver with web player fallback).
    let (jwt, token_source) = match super::apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            log::debug!(
                "Music video companion skipped for {dl_id}: no MusicKit token available"
            );
            return;
        }
        Err(e) => {
            log::debug!("Music video companion token resolution failed for {dl_id}: {e}");
            emit_download_log(app, dl_id, &format!("Music video lookup failed: {e}"));
            return;
        }
    };

    log::debug!("Music video companion: using MusicKit token from {token_source}");

    // Fetch music video relationships
    let relations =
        match super::apple_music_api::fetch_music_video_relations(&jwt, &storefront, &song_ids)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Music video relation lookup failed for {dl_id}: {e}");
                emit_download_log(app, dl_id, &format!("Music video lookup failed: {e}"));
                return;
            }
        };

    // Deduplicate by music video ID. An album may have the same music video
    // linked from multiple songs (e.g., a lead single's video referenced by
    // both the single and album versions). HashSet::insert() returns true if
    // the ID is new, false if already seen — we only keep the first occurrence.
    let mut seen_ids = std::collections::HashSet::new();
    let unique_relations: Vec<_> = relations
        .into_iter()
        .filter(|r| seen_ids.insert(r.music_video_id.clone()))
        .collect();

    if unique_relations.is_empty() {
        emit_download_log(app, dl_id, "No music videos found for this album");
        return;
    }

    emit_download_log(
        app,
        dl_id,
        &format!(
            "Found {} music video(s) — downloading as companions",
            unique_relations.len()
        ),
    );

    // Download each music video using the shared helper
    for relation in &unique_relations {
        if shutdown.is_triggered() {
            log::info!("Music video companions stopping early for {dl_id} (app shutting down)");
            return;
        }

        let mv_url =
            super::apple_music_api::build_music_video_url(&storefront, &relation.music_video_id);
        let mv_name = relation.name.as_deref().unwrap_or("unknown");

        emit_download_log(app, dl_id, &format!("Downloading music video: {mv_name}"));

        download_music_video_by_url(app, dl_id, &mv_url, mv_name, settings).await;
    }

    emit_download_log(
        app,
        dl_id,
        &format!(
            "Music video companion downloads complete ({} video(s))",
            unique_relations.len()
        ),
    );
}

/// Downloads a single music video given its Apple Music URL.
///
/// Emit the `\r`-split segments of a single raw output line to the
/// activity log, matching the main GAMDL reader's coalescing rules.
///
/// yt-dlp and N_m3u8DL-RE (used for music videos / HLS) overwrite
/// terminal progress in place with `\r` rather than `\n`, which means
/// `AsyncBufReadExt::lines()` returns a single line containing many
/// `[download]` progress segments. Emitting that line as-is produces a
/// 100KB+ unreadable blob in the activity log.
///
/// This helper splits on `\r`, strips ANSI escapes, and emits either:
/// - the **last non-empty segment only** in normal mode (keeps activity
///   log scrollable — earlier segments would be overwritten in a real
///   terminal anyway), or
/// - **every** non-empty segment in verbose mode, so users get the full
///   speed / ETA / percentage trail when debugging.
///
/// Shared by companion audio downloads and music-video companion
/// downloads so their progress renders consistently with the primary
/// GAMDL reader.
async fn emit_companion_stream_line(
    app: &tauri::AppHandle,
    dl_id: &str,
    stream: &'static str,
    raw_line: &str,
) -> Option<String> {
    let segments: Vec<&str> = raw_line.split('\r').collect();
    let verbose = crate::utils::activity_log::is_verbose_logging();
    let last_segment_idx = segments.iter().rposition(|s| !s.trim().is_empty());
    let mut last_clean = None;

    for (idx, segment) in segments.iter().enumerate() {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let clean_line = crate::utils::process::strip_ansi_codes(segment);
        last_clean = Some(clean_line.clone());

        // Drive the progress bar off EVERY segment so the speed/ETA/%
        // stay live even when we suppress earlier segments from the log.
        let event = crate::utils::process::parse_gamdl_output(&clean_line);
        let progress = gamdl_service::GamdlProgress {
            download_id: dl_id.to_string(),
            event,
        };
        let _ = app.emit("gamdl-output", &progress);

        // Emit to activity-log: last segment only (normal) or all (verbose).
        if verbose || Some(idx) == last_segment_idx {
            let _ = app.emit(
                "activity-log",
                &crate::utils::activity_log::ActivityLogEvent {
                    download_id: dl_id.to_string(),
                    stream,
                    line: clean_line,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    }

    last_clean
}

/// Recursively collect every `.mp4` / `.m4v` file under the given root.
///
/// Used to snapshot the video-file set before a music video download so we
/// can diff the new set afterwards and pinpoint which files GAMDL just
/// produced. Depth-limited at 4 levels to keep the scan bounded on large
/// music libraries.
fn snapshot_video_files(root: &str) -> std::collections::HashSet<std::path::PathBuf> {
    let mut out = std::collections::HashSet::new();
    let root_path = std::path::Path::new(root);
    if !root_path.is_dir() {
        return out;
    }
    collect_video_files_depth_limited(root_path, &mut out, 0, 4);
    out
}

fn collect_video_files_depth_limited(
    dir: &std::path::Path,
    out: &mut std::collections::HashSet<std::path::PathBuf>,
    depth: u32,
    max_depth: u32,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < max_depth {
                collect_video_files_depth_limited(&path, out, depth + 1, max_depth);
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lc = ext.to_ascii_lowercase();
            if ext_lc == "mp4" || ext_lc == "m4v" {
                out.insert(path);
            }
        }
    }
}

/// Identify freshly-created music video files (those not present in
/// `pre_existing`) and run subtitle extraction + lyrics pairing on each.
///
/// Fire-and-forget: any failure is logged but never fails the download.
async fn extract_music_video_subtitles_for_new_files(
    app: &tauri::AppHandle,
    dl_id: &str,
    output_root: &str,
    pre_existing: &std::collections::HashSet<std::path::PathBuf>,
) {
    let ffprobe_path = match super::metadata_tag_service::get_ffprobe_path(app) {
        Ok(p) => p,
        Err(e) => {
            log::debug!("Skipping music video subtitle extraction for {dl_id}: {e}");
            return;
        }
    };
    let ffmpeg_path = super::dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_path.exists() {
        log::debug!("Skipping music video subtitle extraction for {dl_id}: ffmpeg missing");
        return;
    }

    // Diff pre/post video sets to isolate the new files.
    let post_existing = snapshot_video_files(output_root);
    let new_videos: Vec<_> = post_existing
        .difference(pre_existing)
        .cloned()
        .collect();

    if new_videos.is_empty() {
        log::debug!("No new music video files detected for {dl_id}");
        return;
    }

    for video_path in &new_videos {
        // 1. Extract any embedded subtitle / caption streams to sidecars.
        match super::music_video_subtitle_service::extract_subtitles_to_sidecars(
            &ffprobe_path,
            &ffmpeg_path,
            video_path,
        )
        .await
        {
            Ok(0) => log::debug!(
                "No subtitle streams in {}",
                video_path.display()
            ),
            Ok(n) => {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Extracted {n} subtitle/caption track(s) from {}",
                        video_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("music video")
                    ),
                );
            }
            Err(e) => {
                log::warn!(
                    "Music video subtitle extraction failed for {}: {e}",
                    video_path.display()
                );
            }
        }

        // 2. Pair any matching song lyrics sidecars from the album folder
        //    (works when the music video was a companion to an album track).
        if let Some(album_dir) = video_path.parent() {
            let paired =
                super::music_video_subtitle_service::pair_song_lyrics_with_music_video(
                    album_dir, video_path,
                );
            if paired > 0 {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Paired {paired} song-lyrics file(s) with {}",
                        video_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("music video")
                    ),
                );
            }
        }
    }
}

/// Shared helper used by both the MusicKit-based video companion pipeline
/// (Step 6) and the MusicBrainz fallback discovery (Step 6b). Builds a
/// minimal `GamdlOptions` using the user's video quality settings and
/// spawns a GAMDL subprocess. Fire-and-forget: failures are logged but
/// do not affect the primary download.
///
/// Returns `true` if the download succeeded, `false` otherwise.
async fn download_music_video_by_url(
    app: &tauri::AppHandle,
    dl_id: &str,
    video_url: &str,
    video_label: &str,
    settings: &crate::models::settings::AppSettings,
) -> bool {
    // Inherit the user's filename/folder templates, tool paths, and metadata
    // settings so the music video output matches what the primary pipeline
    // produces. Without these, GAMDL falls back to its own defaults which
    // can yield empty filenames like "-.mp4" for music videos (#481).
    let opts = crate::models::gamdl_options::GamdlOptions {
        output_path: Some(settings.output_path.clone()),
        music_video_resolution: Some(settings.default_video_resolution.clone()),
        music_video_codec_priority: Some(settings.default_video_codec_priority.clone()),
        music_video_remux_format: Some(settings.default_video_remux_format.clone()),
        temp_path: Some(if settings.temp_path.is_empty() {
            std::env::temp_dir().join("MeedyaDL").to_string_lossy().to_string()
        } else {
            settings.temp_path.clone()
        }),
        cookies_path: settings.cookies_path.clone(),
        use_wrapper: Some(settings.use_wrapper),
        wrapper_account_url: if settings.use_wrapper {
            Some(settings.wrapper_account_url.clone())
        } else {
            None
        },
        // Filename / folder templates — shared with the audio pipeline so
        // videos land with `{artist}/{album}/{title}` style names.
        album_folder_template: Some(settings.album_folder_template.clone()),
        compilation_folder_template: Some(settings.compilation_folder_template.clone()),
        no_album_folder_template: Some(settings.no_album_folder_template.clone()),
        single_disc_file_template: Some(settings.single_disc_file_template.clone()),
        multi_disc_file_template: Some(settings.multi_disc_file_template.clone()),
        no_album_file_template: Some(settings.no_album_file_template.clone()),
        playlist_file_template: Some(settings.playlist_file_template.clone()),
        // Tool paths (ffmpeg, mp4decrypt, mp4box, N_m3u8DL-RE) so GAMDL can
        // resolve the managed binaries instead of relying on PATH lookup.
        ffmpeg_path: settings.ffmpeg_path.clone(),
        mp4decrypt_path: settings.mp4decrypt_path.clone(),
        mp4box_path: settings.mp4box_path.clone(),
        nm3u8dlre_path: settings.nm3u8dlre_path.clone(),
        // Metadata / language so music-video tags are localised consistently.
        language: Some(settings.language.clone()),
        truncate: settings.truncate,
        download_mode: Some(settings.download_mode.clone()),
        remux_mode: Some(settings.remux_mode.clone()),
        ..Default::default()
    };

    let urls = vec![video_url.to_string()];
    let mut cmd = match super::gamdl_service::build_gamdl_command_public(app, &urls, &opts) {
        Ok(c) => c,
        Err(e) => {
            log::debug!("Music video download command build failed for {dl_id}: {e}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video download failed ({video_label}): {e}"),
            );
            return false;
        }
    };

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Snapshot the set of video files under the output directory BEFORE
    // GAMDL runs so we can identify exactly which files were freshly
    // produced for this music video (#483). Used for subtitle extraction.
    let pre_existing_videos = snapshot_video_files(&settings.output_path);

    // Stream stdout/stderr line-by-line (with `\r` splitting) instead of
    // buffering the entire process output with `wait_with_output()`.
    // Streaming keeps the progress bar live AND prevents yt-dlp's
    // carriage-return progress blob from arriving as a single 100KB
    // activity-log row (see `emit_companion_stream_line` for rationale).
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::debug!("Failed to spawn music video download for {dl_id}: {e}");
            return false;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_task = stdout.map(|out| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                emit_companion_stream_line(&app, &dl_id, "stdout", &line).await;
            }
        })
    });

    let stderr_task = stderr.map(|err| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(err);
            let mut lines = reader.lines();
            let mut last = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(clean) =
                    emit_companion_stream_line(&app, &dl_id, "stderr", &line).await
                {
                    last = clean;
                }
            }
            last
        })
    });

    let status = child.wait().await;
    if let Some(t) = stdout_task {
        let _ = t.await;
    }
    let last_err = if let Some(t) = stderr_task {
        t.await.unwrap_or_default()
    } else {
        String::new()
    };

    match status {
        Ok(s) if s.success() => {
            log::info!("Music video downloaded for {dl_id}: {video_label}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video downloaded: {video_label}"),
            );
            // Post-process freshly downloaded music videos: extract any
            // embedded subtitle / caption streams into sidecar files and
            // (best-effort) copy the matching song's lyrics alongside.
            extract_music_video_subtitles_for_new_files(
                app,
                dl_id,
                &settings.output_path,
                &pre_existing_videos,
            )
            .await;
            true
        }
        Ok(_) => {
            let shown_err = if last_err.is_empty() {
                "unknown error".to_string()
            } else {
                last_err
            };
            log::debug!("Music video download failed for {dl_id} ({video_label}): {shown_err}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video failed ({video_label}): {shown_err}"),
            );
            false
        }
        Err(e) => {
            log::debug!("Music video process error for {dl_id}: {e}");
            false
        }
    }
}

// ============================================================
// Lyrics format fallback
// ============================================================

/// Count lyrics sidecar files in a directory.
///
/// Returns the number of files matching any supported lyrics extension
/// (`.ttml`, `.lrc`, `.srt`). Each unique stem is counted only once
/// (e.g., `01 Song.ttml` and `01 Song.lrc` count as 1 stem with lyrics).
fn count_lyrics_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut stems_with_lyrics = std::collections::HashSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "ttml" || ext == "lrc" || ext == "srt" {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                stems_with_lyrics.insert(stem.to_string());
            }
        }
    }
    stems_with_lyrics.len()
}

/// Count media files (audio or video) in a directory.
///
/// Returns `(audio_count, video_count)` for `.m4a` and `.m4v`/`.mp4` files.
fn count_media_files(dir: &std::path::Path) -> (usize, usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    let mut audio = 0;
    let mut video = 0;
    for entry in entries.flatten() {
        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "m4a" => audio += 1,
            "m4v" | "mp4" => video += 1,
            _ => {}
        }
    }
    (audio, video)
}

/// Run lyrics format fallback when the primary format didn't produce
/// lyrics for all tracks.
///
/// After the primary download (typically with `--synced-lyrics-format ttml`),
/// checks if every media file has a corresponding lyrics sidecar. If not,
/// retries with fallback formats in content-type-specific order:
/// - **Audio** (`.m4a`): TTML → LRC → SRT
/// - **Video** (`.m4v`/`.mp4`): TTML → SRT → LRC
///
/// Each fallback attempt spawns GAMDL with `--synced-lyrics-format <fmt>
/// --synced-lyrics-only`. The chain stops as soon as lyrics coverage
/// matches the number of media files (or all formats are exhausted).
async fn run_lyrics_fallback(
    app: &tauri::AppHandle,
    dl_id: &str,
    album_dir: &str,
    urls: &[String],
    settings: &crate::models::settings::AppSettings,
) {
    let dir = std::path::Path::new(album_dir);
    let (audio_count, video_count) = count_media_files(dir);
    let total_media = audio_count + video_count;

    if total_media == 0 {
        return; // No media files to match lyrics against
    }

    let lyrics_count = count_lyrics_files(dir);
    if lyrics_count >= total_media {
        log::debug!("Lyrics fallback not needed for {dl_id}: {lyrics_count}/{total_media} tracks have lyrics");
        return; // All tracks already have lyrics
    }

    // Determine fallback chain based on content type.
    // If there are more video files than audio, treat it as video content.
    let is_video = video_count > audio_count;
    let fallback_chain: Vec<LyricsFormat> = if is_video {
        // Video: TTML (already tried) → SRT → LRC
        vec![LyricsFormat::Srt, LyricsFormat::Lrc]
    } else {
        // Audio: TTML (already tried) → LRC → SRT
        vec![LyricsFormat::Lrc, LyricsFormat::Srt]
    };

    emit_download_log(
        app,
        dl_id,
        &format!(
            "Lyrics fallback: {lyrics_count}/{total_media} tracks have lyrics — trying alternative formats"
        ),
    );

    for format in &fallback_chain {
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Lyrics fallback: trying {} format...",
                format.to_cli_string()
            ),
        );

        // Build minimal options for lyrics-only download using struct init
        // syntax (clippy: field_reassign_with_default). Carries over auth
        // and template settings so lyrics land in the correct folder.
        let opts = GamdlOptions {
            synced_lyrics_format: Some(format.clone()),
            synced_lyrics_only: Some(true),
            output_path: Some(settings.output_path.clone()),
            temp_path: Some(if settings.temp_path.is_empty() {
                std::env::temp_dir().join("MeedyaDL").to_string_lossy().to_string()
            } else {
                settings.temp_path.clone()
            }),
            cookies_path: settings.cookies_path.clone(),
            use_wrapper: Some(settings.use_wrapper),
            wrapper_account_url: if settings.use_wrapper {
                Some(settings.wrapper_account_url.clone())
            } else {
                None
            },
            single_disc_file_template: Some(settings.single_disc_file_template.clone()),
            multi_disc_file_template: Some(settings.multi_disc_file_template.clone()),
            no_album_file_template: Some(settings.no_album_file_template.clone()),
            playlist_file_template: Some(settings.playlist_file_template.clone()),
            album_folder_template: Some(settings.album_folder_template.clone()),
            compilation_folder_template: Some(settings.compilation_folder_template.clone()),
            no_album_folder_template: Some(settings.no_album_folder_template.clone()),
            overwrite: Some(settings.overwrite),
            ..Default::default()
        };

        // Build and run GAMDL command
        let mut cmd = match super::gamdl_service::build_gamdl_command_public(app, urls, &opts) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Lyrics fallback command build failed for {dl_id}: {e}");
                continue;
            }
        };

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(child) => match child.wait_with_output().await {
                Ok(output) if output.status.success() => {
                    log::info!(
                        "Lyrics fallback ({}) succeeded for {dl_id}",
                        format.to_cli_string()
                    );
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Lyrics fallback: {} format downloaded successfully",
                            format.to_cli_string()
                        ),
                    );
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let last_err = stderr.lines().last().unwrap_or("unknown error");
                    log::debug!(
                        "Lyrics fallback ({}) failed for {dl_id}: {last_err}",
                        format.to_cli_string()
                    );
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Lyrics fallback: {} format failed — {}",
                            format.to_cli_string(),
                            last_err
                        ),
                    );
                    continue; // Try next format in chain
                }
                Err(e) => {
                    log::debug!("Lyrics fallback process error for {dl_id}: {e}");
                    continue;
                }
            },
            Err(e) => {
                log::debug!("Failed to spawn lyrics fallback for {dl_id}: {e}");
                continue;
            }
        }

        // Re-check lyrics coverage after this fallback attempt
        let new_lyrics_count = count_lyrics_files(dir);
        if new_lyrics_count >= total_media {
            emit_download_log(
                app,
                dl_id,
                &format!("Lyrics fallback complete: {new_lyrics_count}/{total_media} tracks now have lyrics"),
            );
            return; // Coverage complete, stop the fallback chain
        }
    }

    // All fallback formats exhausted
    let final_count = count_lyrics_files(dir);
    emit_download_log(
        app,
        dl_id,
        &format!(
            "Lyrics fallback exhausted: {final_count}/{total_media} tracks have lyrics (some tracks may not have lyrics on Apple Music)"
        ),
    );
}

// ============================================================
// Queue processing: runs downloads and handles fallback/retry
/// Spawns companion and lyrics companion downloads as background tasks.
///
/// Companions are independent downloads of the same content in different
/// Detects the actual audio codec of downloaded files by running ffprobe on
/// the first `.m4a` file in the output directory.
///
/// When native priority is used (`--song-codec-priority atmos,alac,aac,...`),
/// GAMDL silently falls back through the chain. The requested codec may be
/// "atmos" but the actual files may be ALAC. This function probes the real
/// codec so companion downloads are planned against the actual content.
///
/// Returns the CLI string of the detected codec (e.g., "alac", "atmos"),
/// or falls back to `requested_codec` if detection fails.
async fn detect_actual_primary_codec(
    app: &tauri::AppHandle,
    dl_id: &str,
    output_path: Option<&str>,
    requested_codec: &str,
) -> String {
    let Some(dir) = output_path else {
        return requested_codec.to_string();
    };

    let dir_path = std::path::Path::new(dir);
    if !dir_path.is_dir() {
        return requested_codec.to_string();
    }

    // Find the first .m4a file in the output directory
    let first_m4a = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .find(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"))
            })
            .map(|e| e.path()),
        Err(_) => None,
    };

    let Some(m4a_path) = first_m4a else {
        log::debug!("No .m4a files in output dir for codec detection — using requested codec");
        return requested_codec.to_string();
    };

    // Parse the requested codec for the ffprobe fallback path
    let requested_song_codec = crate::models::gamdl_options::SongCodec::from_cli_string(requested_codec)
        .unwrap_or(crate::models::gamdl_options::SongCodec::Aac);

    // Try MediaInfo first (more reliable, especially for Atmos/AC3 distinction)
    if let Some(mediainfo_bin) = super::mediainfo_service::get_mediainfo_path(app) {
        if let Some(result) = super::mediainfo_service::detect_codec(&mediainfo_bin, &m4a_path).await {
            let actual_str = result.codec.to_cli_string().to_string();
            if actual_str != requested_codec {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Actual codec detected via MediaInfo: {} (requested: {}) — companions adjusted",
                        actual_str, requested_codec
                    ),
                );
            }
            return actual_str;
        }
        // MediaInfo present but detection failed for this file
        emit_download_log(
            app,
            dl_id,
            "MediaInfo codec detection failed — falling back to ffprobe",
        );
    }

    // Fall back to ffprobe if MediaInfo unavailable or failed
    if let Ok(ffprobe) = super::metadata_tag_service::get_ffprobe_path(app) {
        if let Some(info) = super::metadata_tag_service::detect_audio_info(&ffprobe, &m4a_path).await {
            let actual = super::metadata_tag_service::resolve_codec_from_ffprobe(&info, &requested_song_codec);
            let actual_str = actual.to_cli_string().to_string();
            if actual_str != requested_codec {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Actual codec detected via ffprobe: {} (requested: {}) — companions adjusted",
                        actual_str, requested_codec
                    ),
                );
            }
            return actual_str;
        }
        // ffprobe present but detection failed
        emit_download_log(
            app,
            dl_id,
            "ffprobe codec detection also failed — using requested codec for companions",
        );
    } else {
        // Neither MediaInfo nor ffprobe available
        emit_download_log(
            app,
            dl_id,
            "No codec detection tools available — using requested codec for companions",
        );
    }

    requested_codec.to_string()
}

/// codecs (e.g., ALAC companion for an Atmos primary). They fire regardless
/// of whether the primary download succeeded or failed — the companion codec
/// may succeed where the primary format was unavailable.
///
/// This is called after any terminal download outcome (success or final
/// failure), but NOT after fallback/network retries (where the download
/// will be re-attempted and companions will fire on the final outcome).
/// Spawns companion downloads and returns a JoinHandle that resolves when
/// all companion tiers have completed. Returns None if no companions are needed.
// The argument list is long because this function is the seam between
// the queue's per-download state (app, queue, dl_id, urls, shutdown),
// the GAMDL invocation context (primary codec, base options,
// force_all_suffixes), and the audioTraits pre-filter from #504. All
// of them are needed inside the spawned task and a struct wrapper
// would just move the repetition to the three call sites. Suppress
// the clippy nit rather than introduce indirection.
#[allow(clippy::too_many_arguments)]
fn spawn_companion_downloads(
    app: &tauri::AppHandle,
    queue: &QueueHandle,
    dl_id: &str,
    urls: &[String],
    primary_codec_str: &str,
    companion_base_options: &GamdlOptions,
    shutdown: &ShutdownSignal,
    force_all_suffixes: bool,
    available_audio_traits: &[String],
) -> Option<tokio::task::JoinHandle<()>> {
    let companion_settings = load_settings_for_queue(app);
    let raw_tiers = plan_companions(
        &companion_settings.companion_mode,
        primary_codec_str,
        &companion_settings.custom_companion_codecs,
    );

    // Drop tiers whose codec the Apple Music API has already told us
    // isn't offered for this track (#504). When no traits are available
    // the filter is a no-op and we fall through to GAMDL as before.
    let (mut companion_tiers, skipped) =
        filter_tiers_by_audio_traits(raw_tiers, available_audio_traits);
    for codec_list in &skipped {
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Companion skipped: {codec_list} not available for this track on Apple Music \
                 (audioTraits: [{}])",
                available_audio_traits.join(", ")
            ),
        );
    }

    // When native priority was used, the primary download has clean filenames
    // (because we don't know the actual codec until GAMDL finishes). Force ALL
    // companion tiers to use suffixed filenames to prevent collisions between
    // the primary's clean filenames and the "most compatible" companion tier
    // that would normally also get clean filenames.
    if force_all_suffixes {
        for tier in &mut companion_tiers {
            tier.apply_suffix = true;
        }
    }

    let codec_handle = if companion_tiers.is_empty() {
        None
    } else {
        let comp_app = app.clone();
        let comp_queue = queue.clone();
        let comp_urls = urls.to_vec();
        let comp_base_opts = companion_base_options.clone();
        let comp_dl_id = dl_id.to_string();
        let comp_shutdown = shutdown.clone();

        emit_download_log(
            app,
            dl_id,
            &format!(
                "──── Companion downloads (mode: {:?}) ────",
                companion_settings.companion_mode,
            ),
        );
        // Log per-tier codec details before spawning the async task
        for (tier_idx, tier) in companion_tiers.iter().enumerate() {
            let codec_names: Vec<&str> = tier
                .codecs_to_try
                .iter()
                .map(SongCodec::to_cli_string)
                .collect();
            emit_download_log(
                app,
                dl_id,
                &format!(
                    "Companion tier {}: trying {}",
                    tier_idx,
                    codec_names.join(", ")
                ),
            );
        }

        let handle = tokio::spawn(async move {
            // Process each companion tier sequentially
            for (tier_idx, tier) in companion_tiers.iter().enumerate() {
                // Check for app shutdown between tiers
                if comp_shutdown.is_triggered() {
                    log::info!("Companion downloads stopping early (app shutting down)");
                    return;
                }

                let mut tier_succeeded = false;

                // Try each codec in the tier until one succeeds
                for codec in &tier.codecs_to_try {
                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!(
                            "Companion tier {}: attempting {}...",
                            tier_idx,
                            codec.to_cli_string()
                        ),
                    );

                    let mut opts = comp_base_opts.clone();
                    // Use --song-codec-priority with a single codec value.
                    // GAMDL >= 2.9.1 removed the --song-codec flag entirely;
                    // passing a single-element priority achieves the same effect.
                    opts.song_codec = None;
                    opts.song_codec_priority = Some(codec.to_cli_string().to_string());

                    // If this tier needs a suffix (e.g., ALAC
                    // companion in AtmosToLosslessAndLossy mode
                    // gets [Lossless]), apply it to the options.
                    if tier.apply_suffix {
                        apply_codec_suffix(&mut opts);
                    }
                    // If not suffixed, the base options already
                    // have clean (unsuffixed) templates.

                    // Build the GAMDL CLI command for the companion
                    let mut cmd = match gamdl_service::build_gamdl_command_public(
                        &comp_app, &comp_urls, &opts,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            log::debug!(
                                "Companion tier {}: failed to build \
                                 command ({}) for {}: {}",
                                tier_idx,
                                codec.to_cli_string(),
                                comp_dl_id,
                                e
                            );
                            continue; // Try next codec in tier
                        }
                    };

                    // Log the GAMDL CLI args so users can verify the codec request
                    emit_verbose_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!(
                            "Companion GAMDL args: --song-codec-priority {}",
                            codec.to_cli_string()
                        ),
                    );

                    // Pipe stdout/stderr and stream line-by-line to the activity log.
                    // This gives users real-time visibility into companion downloads
                    // (per-track progress, [download] fragments, errors).
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::piped());

                    let stream_codec = codec.to_cli_string().to_string();

                    // Hand the child off to the companion supervisor:
                    //   - kill_on_drop(true) so an aborted task reaps GAMDL (#501)
                    //   - parses `Finished with N error(s)` to detect soft errors (#500)
                    //   - watches for stdout/stderr silence and kills the child after
                    //     `gamdl_idle_timeout_minutes` of inactivity (#505), pausing the
                    //     watchdog once a `100% of` line marks the post-processing phase (#503)
                    let supervisor_app = comp_app.clone();
                    let supervisor_dl = comp_dl_id.clone();
                    let label_for_emit = format!("companion-tier-{tier_idx}");
                    let companion_settings = load_settings_for_queue(&comp_app);
                    let idle_timeout = std::time::Duration::from_secs(
                        u64::from(companion_settings.gamdl_idle_timeout_minutes.max(1)) * 60,
                    );
                    let emitter: super::companion_supervisor::LineEmitter =
                        std::sync::Arc::new(move |app, dl, stream, line| {
                            Box::pin(async move {
                                let kind = if stream.contains("stderr") {
                                    "stderr"
                                } else {
                                    "stdout"
                                };
                                emit_companion_stream_line(&app, &dl, kind, &line).await
                            })
                        });

                    let run_result = super::companion_supervisor::run_supervised(
                        &supervisor_app,
                        &supervisor_dl,
                        &label_for_emit,
                        cmd,
                        idle_timeout,
                        emitter,
                    )
                    .await;

                    // Surface the post-processing transition to the queue UI so
                    // the per-item bar switches from `DOWNLOADING…` to
                    // `PROCESSING (remux / decrypt)…` instead of looking frozen
                    // while mp4decrypt / ffmpeg / mp4box run silently (#503).
                    if let Ok(ref r) = run_result {
                        if r.reached_post_processing {
                            let mut q = comp_queue.lock().await;
                            q.set_processing_label(
                                &supervisor_dl,
                                &format!(
                                    "Post-processing companion ({}): remux / decrypt",
                                    stream_codec
                                ),
                            );
                        }
                    }

                    match run_result {
                        Ok(r) if r.exit_success && !r.had_soft_error => {
                            emit_download_log(
                                &comp_app,
                                &comp_dl_id,
                                &format!(
                                    "Companion download complete (tier {}, codec: {})",
                                    tier_idx, stream_codec,
                                ),
                            );
                            let _ = comp_app.emit("companion-downloaded", &comp_dl_id);

                            if let Some(ref output_dir) = opts.output_path {
                                // Rename cover art per user setting (#448)
                                let comp_settings = load_settings_for_queue(&comp_app);
                                rename_cover_art(output_dir, comp_settings.cover_art_name.to_filename_stem());

                                match super::metadata_tag_service::apply_codec_metadata_tags(
                                    output_dir, codec,
                                ) {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Tagged {} companion file(s) with {} metadata for {}",
                                            count, stream_codec, comp_dl_id
                                        );
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::debug!("Companion metadata tagging failed for {comp_dl_id}: {e}");
                                    }
                                }

                                // Scope the lyrics conversion to the album we
                                // just produced — falls back to a recursive
                                // walk only when the artist/album hints are
                                // unavailable (#502).
                                let (artist_hint, album_hint) = {
                                    let q = comp_queue.lock().await;
                                    q.items
                                        .iter()
                                        .find(|i| i.status.id == comp_dl_id)
                                        .map(|i| {
                                            (
                                                i.status.artist_name.clone(),
                                                i.status.album_name.clone(),
                                            )
                                        })
                                        .unwrap_or((None, None))
                                };
                                run_companion_lyrics_conversion(
                                    &comp_app,
                                    &comp_dl_id,
                                    output_dir,
                                    artist_hint.as_deref(),
                                    album_hint.as_deref(),
                                );
                            }

                            // Clear the post-processing label now that we're done.
                            {
                                let mut q = comp_queue.lock().await;
                                q.clear_processing_label(&supervisor_dl);
                            }

                            tier_succeeded = true;
                            break;
                        }
                        Ok(r) => {
                            // Build the most informative failure message we can:
                            //   - friendly_error wins (translated traceback, #500)
                            //   - then idle_killed (#505)
                            //   - then had_soft_error notes the GAMDL summary (#500)
                            //   - last resort: whatever stderr ended on
                            let detail = if let Some(msg) = r.friendly_error {
                                msg
                            } else if r.idle_killed {
                                format!(
                                    "GAMDL was idle for {} min — terminated by watchdog",
                                    companion_settings.gamdl_idle_timeout_minutes.max(1)
                                )
                            } else if r.had_soft_error {
                                "GAMDL exited 0 but reported per-track errors — treating as failure"
                                    .to_string()
                            } else if r.last_stderr_line.is_empty() {
                                "unknown error".to_string()
                            } else {
                                r.last_stderr_line.clone()
                            };
                            emit_download_log(
                                &comp_app,
                                &comp_dl_id,
                                &format!(
                                    "Companion tier {}: {} failed — {}",
                                    tier_idx, stream_codec, detail
                                ),
                            );
                            // Reset post-processing label on failure too.
                            {
                                let mut q = comp_queue.lock().await;
                                q.clear_processing_label(&supervisor_dl);
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to spawn companion ({stream_codec}): {e}");
                        }
                    }
                }

                if !tier_succeeded {
                    log::debug!("Companion tier {tier_idx} exhausted all codecs for {comp_dl_id}");
                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!("Companion tier {tier_idx}: all codecs exhausted"),
                    );
                }
            }
        });
        Some(handle)
    };

    /// Runs lyrics format conversion on a companion download's output directory.
    ///
    /// Companions inherit TTML as the lyrics format (forced by Enhanced LRC),
    /// but the enrichment pipeline only runs for the primary download. This
    /// function runs the same conversion steps so companion sidecars get
    /// Enhanced LRC, Rich SRT, WebVTT, and ASS conversions.
    ///
    /// The `output_dir` parameter is the top-level output path from GamdlOptions
    /// (e.g., `~/Music/`), not the album-specific directory. GAMDL creates the
    /// `Artist/Album/` structure within this path. The lyrics conversion services
    /// expect the album directory (where .ttml and .m4a files live), so this
    /// function resolves it by recursively finding directories with TTML files.
    fn run_companion_lyrics_conversion(
        app: &tauri::AppHandle,
        dl_id: &str,
        output_dir: &str,
        artist_hint: Option<&str>,
        album_hint: Option<&str>,
    ) {
        let settings = load_settings_for_queue(app);
        let base = std::path::Path::new(output_dir);

        if !base.is_dir() {
            return;
        }

        // Prefer the targeted `{output_dir}/{artist}/{album}/` resolution
        // when we have both hints, falling back to a recursive walk only
        // when hints aren't available or the targeted path doesn't exist
        // yet (#502). The recursive walk over a large library can take
        // several minutes — scoping eliminates the perceived "hang" the
        // user saw between a finished companion and the next queue item.
        let album_dirs: Vec<std::path::PathBuf> = match (artist_hint, album_hint) {
            (Some(artist), Some(album)) => {
                let scoped = base.join(artist).join(album);
                if scoped.is_dir() && find_dirs_with_ttml(&scoped).iter().any(|p| p == &scoped) {
                    log::debug!(
                        "Companion lyrics: scoped to album dir {} for {dl_id}",
                        scoped.display()
                    );
                    vec![scoped]
                } else {
                    log::debug!(
                        "Companion lyrics: scoped path {} missing TTML — falling back to recursive walk",
                        scoped.display()
                    );
                    find_dirs_with_ttml(base)
                }
            }
            _ => {
                log::debug!(
                    "Companion lyrics: no artist/album hints for {dl_id} — recursive walk over {output_dir}"
                );
                find_dirs_with_ttml(base)
            }
        };
        if album_dirs.is_empty() {
            log::debug!(
                "Companion lyrics: no TTML files found in {output_dir} — skipping conversion"
            );
            return;
        }

        for album_dir in &album_dirs {
            let dir_str = album_dir.to_string_lossy();

            // Enhanced LRC: TTML → word-by-word LRC
            if settings.enhanced_lrc {
                match super::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(&dir_str)
                {
                    Ok(count) if count > 0 => {
                        emit_download_log(
                            app,
                            dl_id,
                            &format!("Companion: converted {count} TTML file(s) to Enhanced LRC"),
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("Companion Enhanced LRC conversion failed: {e}");
                    }
                }
            }

            // Rich SRT: TTML → styled SRT with bold/italic/colour
            if settings.generate_rich_srt {
                match super::rich_srt_service::generate_rich_srt_for_directory(&dir_str) {
                    Ok(count) if count > 0 => {
                        log::debug!("Companion: generated {count} Rich SRT file(s)");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("Companion Rich SRT generation failed: {e}");
                    }
                }
            }

            // WebVTT: TTML/SRT/LRC → .vtt
            if settings.generate_webvtt {
                match super::webvtt_service::generate_webvtt_for_directory(&dir_str) {
                    Ok(count) if count > 0 => {
                        log::debug!("Companion: generated {count} WebVTT file(s)");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("Companion WebVTT generation failed: {e}");
                    }
                }
            }

            // ASS: → styled .ass subtitles
            if settings.generate_ass {
                match super::ass_subtitle_service::generate_ass_for_directory(&dir_str) {
                    Ok(count) if count > 0 => {
                        log::debug!("Companion: generated {count} ASS subtitle file(s)");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::debug!("Companion ASS generation failed: {e}");
                    }
                }
            }
        }
    }

    /// Recursively finds directories that contain `.ttml` files.
    ///
    /// GAMDL creates an `Artist/Album/` directory structure within the output
    /// path. The lyrics conversion services expect the leaf album directory
    /// where `.ttml` and `.m4a` files coexist. This helper walks the tree
    /// and collects every directory that directly contains at least one
    /// `.ttml` file.
    fn find_dirs_with_ttml(base: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut result = Vec::new();
        let mut has_ttml_here = false;

        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Recurse into subdirectories
                    result.extend(find_dirs_with_ttml(&path));
                } else if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ttml"))
                {
                    has_ttml_here = true;
                }
            }
        }

        if has_ttml_here {
            result.push(base.to_path_buf());
        }

        result
    }

    // === Lyrics companion downloads (background, fire-and-forget) ===
    // When the user has selected additional lyrics formats beyond the
    // primary, spawn a lightweight GAMDL invocation for each extra
    // format using --synced-lyrics-only. This produces sidecar lyrics
    // files (.lrc, .srt, .ttml) without re-downloading audio.
    let lyrics_settings = load_settings_for_queue(app);
    if !lyrics_settings.companion_lyrics_formats.is_empty()
        && (!lyrics_settings.no_synced_lyrics || lyrics_settings.embed_lyrics_and_sidecar)
    {
        let lyrics_app = app.clone();
        let lyrics_urls = urls.to_vec();
        let lyrics_base_opts = companion_base_options.clone();
        let lyrics_dl_id = dl_id.to_string();
        let lyrics_formats = lyrics_settings.companion_lyrics_formats.clone();
        let lyrics_shutdown = shutdown.clone();

        let format_names: Vec<&str> = lyrics_formats
            .iter()
            .map(LyricsFormat::to_cli_string)
            .collect();
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Downloading companion lyrics formats: {}",
                format_names.join(", ")
            ),
        );

        tokio::spawn(async move {
            for format in &lyrics_formats {
                // Check for app shutdown between lyrics format iterations
                if lyrics_shutdown.is_triggered() {
                    log::info!("Lyrics companion downloads stopping early (app shutting down)");
                    return;
                }
                let mut opts = lyrics_base_opts.clone();
                opts.synced_lyrics_format = Some(format.clone());
                opts.synced_lyrics_only = Some(true);

                let mut cmd = match gamdl_service::build_gamdl_command_public(
                    &lyrics_app,
                    &lyrics_urls,
                    &opts,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        log::debug!(
                            "Lyrics companion ({}) command error: {e}",
                            format.to_cli_string()
                        );
                        continue;
                    }
                };

                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                emit_download_log(
                    &lyrics_app,
                    &lyrics_dl_id,
                    &format!(
                        "Lyrics companion: downloading {}...",
                        format.to_cli_string()
                    ),
                );

                match cmd.spawn() {
                    Ok(child) => match child.wait_with_output().await {
                        Ok(output) if output.status.success() => {
                            log::info!(
                                "Lyrics companion ({}) \
                                     downloaded for {}",
                                format.to_cli_string(),
                                lyrics_dl_id
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!(
                                    "Lyrics companion ({}) downloaded",
                                    format.to_cli_string(),
                                ),
                            );
                            let _ = lyrics_app.emit(
                                "lyrics-companion-downloaded",
                                serde_json::json!({
                                    "download_id": lyrics_dl_id,
                                    "format": format.to_cli_string(),
                                }),
                            );
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let last_line = stderr.lines().last().unwrap_or("");
                            log::debug!(
                                "Lyrics companion ({}) \
                                     failed for {}: {}",
                                format.to_cli_string(),
                                lyrics_dl_id,
                                last_line
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!(
                                    "Lyrics companion ({}) failed: {}",
                                    format.to_cli_string(),
                                    last_line,
                                ),
                            );
                        }
                        Err(e) => {
                            log::debug!(
                                "Lyrics companion process \
                                     error: {e}"
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!("Lyrics companion process error: {e}"),
                            );
                        }
                    },
                    Err(e) => {
                        log::debug!("Failed to spawn lyrics companion: {e}");
                        emit_download_log(
                            &lyrics_app,
                            &lyrics_dl_id,
                            &format!("Failed to spawn lyrics companion: {e}"),
                        );
                    }
                }
            }
        });
    }

    // Return the codec companion handle if it exists.
    // (Lyrics companions are fire-and-forget — they're fast and non-critical.)
    codec_handle
}

// ============================================================

/// Processes the next queued download if a slot is available.
///
/// This function is called after enqueueing a new item, after a download
/// completes, or after a retry/fallback. It spawns a background task
/// for the download and sets up event forwarding to the frontend.
///
/// Returns a boxed future to support recursive calls from within
/// `tokio::spawn` (standard pattern for recursive async in Rust).
///
/// # Arguments
/// * `app` - Tauri app handle for event emission and path resolution
/// * `queue` - Shared queue handle
// Monolithic by necessity: this function orchestrates the entire queue lifecycle
// including async closures with complex move semantics, cancellation polling,
// fallback/retry decision trees, companion spawning, and recursive self-calls.
// Extracting helper functions would require passing many Arc/Mutex handles and
// would fragment the sequential orchestration logic without meaningful benefit.
#[allow(clippy::too_many_lines)]
pub fn process_queue(
    app: AppHandle,
    queue: QueueHandle,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        // === Pre-flight health checks (once per queue batch) ===
        // Run lightweight health checks before processing to warn the user about
        // potential issues (no internet, expired cookies, wrapper down). These are
        // non-blocking warnings — the queue proceeds regardless. A 60-second
        // cooldown prevents duplicate warnings during cascading process_queue() calls.
        {
            let should_run = {
                let q = queue.lock().await;
                q.should_run_preflight()
            };

            if should_run {
                // Mark as run first to prevent concurrent duplicate checks
                {
                    let mut q = queue.lock().await;
                    q.mark_preflight_run();
                }

                let settings = load_settings_for_queue(&app);

                // Run internet + wrapper checks concurrently (both are HTTP GETs)
                let internet_future =
                    crate::services::health_check_service::check_internet_connectivity();
                let wrapper_future = if settings.use_wrapper {
                    Some(crate::services::health_check_service::check_wrapper_health(
                        &settings.wrapper_account_url,
                    ))
                } else {
                    None
                };

                // Output path writability check — verify the resolved output directory
                // is accessible before starting downloads. Catches disconnected cloud
                // mounts, full disks, and permission issues.
                let output_path_for_check = if settings.output_path.is_empty() {
                    crate::services::config_service::get_default_output_path().ok()
                } else {
                    Some(settings.output_path.clone())
                };
                let output_path_future = output_path_for_check
                    .as_ref()
                    .map(|path| crate::services::health_check_service::check_output_path(path));

                // Cookie check is synchronous (file I/O only) — run when not using wrapper
                let cookie_warning = if !settings.use_wrapper {
                    if let Some(ref path) = settings.cookies_path {
                        crate::services::health_check_service::validate_cookies(path)
                    } else {
                        Some(crate::services::health_check_service::PreflightWarning {
                        check: crate::services::health_check_service::PreflightCheck::Cookies,
                        message: "No cookies file configured. Apple Music downloads require authentication via cookies or wrapper.".to_string(),
                    })
                    }
                } else {
                    None
                };

                // Await the async checks
                let internet_warning = internet_future.await;
                let wrapper_warning = match wrapper_future {
                    Some(fut) => fut.await,
                    None => None,
                };
                let output_path_warning = match output_path_future {
                    Some(fut) => fut.await,
                    None => None,
                };

                // Build the list of checks that were actually run, paired with their result.
                // Each entry is (check_type, warning_option). We use this to:
                //   - Emit "preflight-warning" for checks that failed
                //   - Emit "preflight-cleared" for checks that passed (so the frontend
                //     can dismiss stale warning toasts from a previous cycle)
                let checks_run: Vec<(
                    crate::services::health_check_service::PreflightCheck,
                    Option<crate::services::health_check_service::PreflightWarning>,
                )> = {
                    use crate::services::health_check_service::PreflightCheck;
                    let mut v = vec![(PreflightCheck::Internet, internet_warning)];
                    if !settings.use_wrapper {
                        v.push((PreflightCheck::Cookies, cookie_warning));
                    }
                    if settings.use_wrapper {
                        v.push((PreflightCheck::Wrapper, wrapper_warning));
                    }
                    // Always check output path (applies to all auth modes)
                    v.push((PreflightCheck::OutputPath, output_path_warning));
                    v
                };

                let mut any_warnings = false;
                for (check, warning) in checks_run {
                    if let Some(w) = warning {
                        any_warnings = true;
                        log::warn!("Pre-flight warning ({check:?}): {}", w.message);
                        emit_app_log(&app, &format!("Pre-flight: {}", w.message));
                        let _ = app.emit("preflight-warning", &w);
                    } else {
                        // Check passed — dismiss any stale toast for this check type
                        let _ = app.emit(
                            "preflight-cleared",
                            &serde_json::json!({ "check": format!("{check:?}") }),
                        );
                    }
                }
                if !any_warnings {
                    emit_app_log(&app, "Pre-flight checks passed");
                }
            }
        }

        // Acquire the queue lock briefly to check for the next pending item.
        // The lock is released immediately after to avoid holding it during the download.
        let pending = {
            let mut q = queue.lock().await;
            q.next_pending()
        };

        // If no items are pending (queue empty or max concurrent reached), exit.
        // When the queue is truly idle (no active + no pending), execute the
        // configured after-queue action (e.g., shutdown, close app).
        let Some((download_id, urls, mut options, item_service)) = pending else {
            let is_idle = {
                let q = queue.lock().await;
                q.is_idle()
            };
            if is_idle {
                execute_after_queue_action(&app);
            }
            return;
        };

        // Determine if this is an Apple Music download for service-aware routing.
        // Used to guard Apple Music-specific enrichment and companion features.
        let is_apple_music = item_service.as_deref() == Some("apple-music")
            || item_service.is_none(); // Legacy items default to Apple Music

        log::info!("Processing download {download_id}");

        // Capture download start time for the manifest file. This is when
        // the first file begins downloading, not when enrichment finishes.
        let download_started_at = chrono::Utc::now().to_rfc3339();

        // Emit a clear separator in the activity log so users can easily
        // distinguish where one queue item ends and the next begins.
        // Includes codec and quality info for debugging.
        let separator_url = urls.first().cloned().unwrap_or_default();
        let separator_codec = options
            .song_codec
            .as_ref()
            .map(|c| c.to_cli_string().to_string())
            .unwrap_or_else(|| "default".to_string());
        let separator_wrapper = if options.use_wrapper == Some(true) {
            "wrapper"
        } else {
            "cookies"
        };
        emit_download_log(
            &app,
            &download_id,
            &format!(
                "[MeedyaDL] ════════════════════════════════════════\n\
                 Starting download: {separator_url}\n\
                 Codec: {separator_codec} | Auth: {separator_wrapper}"
            ),
        );

        // === Early metadata fetch (Apple Music API) ===
        // Fetch album metadata from the Apple Music API BEFORE starting the
        // GAMDL subprocess, so artist_name and album_name are available for
        // the progress bar caption and activity log from the very first track.
        // This is a lightweight API call that completes in <1s on good networks.
        // Failures are silently ignored — the enrichment pipeline will retry later.
        if is_apple_music {
            let early_metadata = super::metadata_tag_service::try_fetch_metadata(
                &app,
                &urls,
                Some((&app, &download_id)),
            )
            .await;

            if let Some(ref meta) = early_metadata {
                let mut q = queue.lock().await;
                if let Some(item) = q
                    .items
                    .iter_mut()
                    .find(|i| i.status.id == download_id)
                {
                    if let Some(ref name) = meta.artist_name {
                        item.status.artist_name = Some(name.clone());
                    }
                    if let Some(ref name) = meta.album_name {
                        item.status.album_name = Some(name.clone());
                    }
                    // Capture the union of audioTraits across every track
                    // for the companion planner (#504). De-duplicate so
                    // the slice handed to plan_companions is compact.
                    let mut traits: Vec<String> = meta
                        .tracks
                        .iter()
                        .flat_map(|t| t.audio_traits.iter().cloned())
                        .collect();
                    traits.sort();
                    traits.dedup();
                    item.status.audio_traits = traits;
                }
                drop(q);
                // Trigger frontend queue refresh so progress bar picks up the metadata
                let _ = app.emit("download-queued", &download_id);
            }
        }

        // === GAMDL version detection (cached, runs once per queue lifetime) ===
        // Detect the installed GAMDL version on the first download so we can
        // decide whether to use native `--song-codec-priority` (>= 2.9.1) or
        // our own `try_fallback` system for older versions.
        let gamdl_version = {
            let mut q = queue.lock().await;
            if q.gamdl_version.is_none() {
                // First download in this queue session — detect version once
                match gamdl_service::get_gamdl_version(&app).await {
                    Ok(Some(ver)) => {
                        log::info!("Detected GAMDL version: {ver}");
                        q.gamdl_version = Some(ver);
                    }
                    Ok(None) => {
                        log::warn!("GAMDL not installed — version detection skipped");
                    }
                    Err(e) => {
                        log::warn!("Failed to detect GAMDL version: {e}");
                    }
                }
            }
            q.gamdl_version.clone()
        };

        // === Native codec priority (GAMDL >= 2.9.1) ===
        // When GAMDL supports `--song-codec-priority`, build the priority string
        // from the user's preferred codec + fallback chain. GAMDL tries each codec
        // in order within a single process, which is much more efficient than our
        // `try_fallback` system (which spawns one process per codec attempt).
        // If native priority fails, MeedyaDL's own `try_fallback()` still runs as
        // a safety net, retrying each codec individually via `--song-codec-priority`
        // with a single codec (see `try_fallback()` and the "codec" error handler).
        let uses_native_priority = if let Some(ref ver) = gamdl_version {
            let settings_for_priority = load_settings_for_queue(&app);
            if gamdl_service::is_version_at_least(ver, "2.9.1")
                && settings_for_priority.fallback_enabled
                && !settings_for_priority.music_fallback_chain.is_empty()
                && options.song_codec_priority.is_none()
            {
                // Build priority string: preferred codec first, then remaining
                // fallback chain entries (deduped to avoid redundant attempts)
                let mut seen = HashSet::new();
                let mut priority_codecs: Vec<String> = Vec::new();

                // Start with the preferred codec from the merged options
                if let Some(ref codec) = options.song_codec {
                    let cli_str = codec.to_cli_string().to_string();
                    seen.insert(cli_str.clone());
                    priority_codecs.push(cli_str);
                }

                // Append remaining fallback chain entries, skipping duplicates
                for codec in &settings_for_priority.music_fallback_chain {
                    let cli_str = codec.to_cli_string().to_string();
                    if seen.insert(cli_str.clone()) {
                        priority_codecs.push(cli_str);
                    }
                }

                if priority_codecs.len() > 1 {
                    let priority_str = priority_codecs.join(",");
                    log::info!(
                        "Download {download_id} using native codec priority: {priority_str}"
                    );
                    emit_download_log(
                        &app,
                        &download_id,
                        &format!(
                            "Using GAMDL native format priority: {}",
                            priority_str.replace(',', " → ")
                        ),
                    );
                    options.song_codec_priority = Some(priority_str);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // === Codec suffix: always apply codec suffix to primary download ===
        // Codec suffixes (e.g., "[Lossless]", "[Dolby Atmos]") are always
        // applied to primary download filenames so users can identify the
        // codec at a glance. Standard AAC has an empty suffix in codecs.toml,
        // so `apply_codec_suffix` is a no-op for it — filenames stay clean.
        //
        // IMPORTANT: When native `--song-codec-priority` is used, we don't know
        // which codec GAMDL will actually select from the priority chain until
        // after the download completes. Applying a suffix based on the REQUESTED
        // codec would be speculative and potentially wrong (e.g., suffixing
        // `[Dolby Atmos]` when GAMDL falls back to AAC). In that case, the
        // primary gets clean filenames and all companions get suffixed filenames
        // (via `force_all_suffixes` in `spawn_companion_downloads`).
        //
        // Keep the original (unsuffixed) options for companion downloads later.
        let companion_base_options = options.clone();
        let mut download_options = options;
        let settings_for_companion = load_settings_for_queue(&app);
        if !uses_native_priority {
            // Single-codec mode: apply codec suffix to file templates only
            // when companion downloads will produce alternative formats.
            // The suffix prevents filename collisions between primary and
            // companion files. For non-companion downloads, codec suffixes
            // are applied as a post-download rename in the enrichment pipeline
            // (see apply_codec_rename_suffixes in metadata_tag_service.rs).
            if let Some(ref codec) = download_options.song_codec {
                if needs_primary_suffix(
                    codec,
                    &settings_for_companion.companion_mode,
                    &settings_for_companion.custom_companion_codecs,
                ) {
                    apply_codec_suffix(&mut download_options);
                    log::info!(
                        "Download {} using codec with file suffix (companion mode: {:?})",
                        download_id,
                        settings_for_companion.companion_mode
                    );
                }
            }
        } else if let Some(ref codec) = download_options.song_codec {
            // Native priority mode: log that we're using clean filenames because
            // the actual codec is TBD (GAMDL picks from the priority chain).
            if needs_primary_suffix(
                codec,
                &settings_for_companion.companion_mode,
                &settings_for_companion.custom_companion_codecs,
            ) {
                log::info!(
                    "Download {} skipping suffix (native priority active, actual codec unknown until download completes)",
                    download_id
                );
                emit_verbose_download_log(
                    &app,
                    &download_id,
                    "Primary uses clean filenames (native priority — actual codec determined by GAMDL from priority chain). Companion downloads will use codec suffixes.",
                );
            }
        }

        // Notify the frontend that this download is starting.
        // The frontend uses this event to transition the download card's UI state.
        let _ = app.emit("download-started", &download_id);

        // Extract the primary codec string for companion planning. This is the
        // codec the user requested (e.g., "atmos"), used to determine which
        // companion tiers to spawn after the download attempt completes.
        let primary_codec_for_companions = download_options
            .song_codec
            .as_ref()
            .map_or_else(String::new, |c| c.to_cli_string().to_string());

        // Capture wrapper URL for logging inside the tokio::spawn block.
        // download_options will be moved into the spawn, so extract this before.
        let wrapper_url_for_logging = if download_options.use_wrapper == Some(true) {
            download_options.wrapper_account_url.clone()
        } else {
            None
        };

        // Log wrapper URL at download start for troubleshooting connectivity.
        // Redact query parameters to avoid leaking authentication tokens
        // (e.g., ?token=abc) into log files which have no automatic cleanup.
        if let Some(ref url) = wrapper_url_for_logging {
            let safe_url = redact_url_query(url);
            log::info!(
                "Download {download_id} using wrapper at {safe_url}"
            );
            // User-visible Activity Log: show auth mode clearly
            emit_download_log(
                &app,
                &download_id,
                &format!("Authentication: Wrapper ({safe_url})"),
            );
            // Verbose: show full wrapper URL (not redacted) for troubleshooting
            emit_verbose_download_log(&app, &download_id, &format!("Wrapper URL (full): {url}"));
        } else {
            emit_download_log(
                &app,
                &download_id,
                "Authentication: Cookie-based (no wrapper)",
            );
        }

        // Verbose: log the full download options
        emit_verbose_download_log(
            &app,
            &download_id,
            &format!(
                "URLs: {:?} | Codec: {} | Native priority: {}",
                &urls,
                primary_codec_for_companions,
                download_options.song_codec_priority.is_some()
            ),
        );

        // Retrieve the shutdown signal from Tauri managed state.
        // This is checked by fire-and-forget background tasks (companion
        // downloads, lyrics, enrichment) to exit early on app close.
        let shutdown_signal = {
            use tauri::Manager;
            app.state::<ShutdownSignal>().inner().clone()
        };

        // Spawn the download in a separate tokio task so it runs independently.
        // This allows process_queue() to return immediately while the download runs.
        let app_clone = app.clone();
        let queue_clone = queue.clone();
        let dl_id = download_id;
        let shutdown_clone = shutdown_signal;

        tokio::spawn(async move {
            // Run the GAMDL download with real-time event forwarding.
            // This function handles subprocess spawning, output parsing,
            // and cancellation polling. See run_download_with_events() below.
            let result = run_download_with_events(
                &app_clone,
                &dl_id,
                &urls,
                &download_options,
                &queue_clone,
            )
            .await;

            // Handle the result of the download attempt
            match result {
                Ok(warnings) => {
                    // === Success path ===
                    // GAMDL exited with code 0. Check whether output files were
                    // actually produced before declaring success.
                    let (output_path_for_artwork, completed_codec, history_track_name, history_created_at) = {
                        let mut q = queue_clone.lock().await;

                        // Check if GAMDL emitted a "Saved to:" line during the run.
                        // If not, no files were produced despite a clean exit.
                        let has_output = q
                            .items
                            .iter()
                            .find(|i| i.status.id == dl_id)
                            .and_then(|i| i.status.output_path.as_ref())
                            .is_some();

                        if !has_output {
                            // No "Saved to:" line was emitted by GAMDL despite exit 0.
                            // Classify the warnings to determine the right recovery
                            // strategy: codec fallback, IO error recovery, or terminal.
                            let has_codec_error =
                                warnings.iter().any(|w| process::is_codec_error(w));
                            let has_io_error = warnings.iter().any(|w| process::is_io_error(w));

                            if has_codec_error {
                                // Codec-related failure on success path — try fallback
                                let error_msg = warnings.last().cloned().unwrap_or_else(|| {
                                    "Requested format is not available".to_string()
                                });

                                if uses_native_priority {
                                    // GAMDL >= 2.9.1 used native --song-codec-priority.
                                    // Check for partial success: some tracks downloaded
                                    // but others skipped because experimental codecs
                                    // (Atmos, AC3) don't reliably fall back per-track
                                    // without wrapper auth.
                                    let skip_count = count_codec_skip_warnings(&warnings);
                                    let output_base = q
                                        .items
                                        .iter()
                                        .find(|i| i.status.id == dl_id)
                                        .and_then(|i| i.merged_options.output_path.clone());
                                    let existing_audio = output_base
                                        .as_ref()
                                        .map(|p| {
                                            count_audio_files_in_directory(std::path::Path::new(p))
                                        })
                                        .unwrap_or(0);
                                    let priority_chain = download_options
                                        .song_codec_priority
                                        .as_deref()
                                        .unwrap_or("");
                                    let gapfill_chain =
                                        build_gapfill_priority_chain(priority_chain);
                                    let wrapper_active =
                                        download_options.use_wrapper.unwrap_or(false);

                                    if let Some(chain) = gapfill_chain.filter(|_| {
                                        existing_audio > 0 && skip_count > 0 && !wrapper_active
                                    }) {
                                        log::info!(
                                            "Download {dl_id} partial: {existing_audio} file(s) \
                                             on disk, {skip_count} skip warning(s). \
                                             Gap-fill with: {chain}"
                                        );
                                        emit_download_log(
                                            &app_clone,
                                            &dl_id,
                                            &format!(
                                                "Partial download: {existing_audio} track(s) \
                                                 downloaded, {skip_count} skipped \
                                                 (experimental codec without wrapper). \
                                                 Re-downloading skipped tracks with \
                                                 lossless fallback..."
                                            ),
                                        );

                                        // Build gap-fill options: same as original but
                                        // with overwrite=false and the filtered chain.
                                        let mut gapfill_options = download_options.clone();
                                        gapfill_options.overwrite = Some(false);
                                        gapfill_options.song_codec_priority = Some(chain);

                                        // Release lock before async GAMDL call
                                        drop(q);

                                        match run_download_with_events(
                                            &app_clone,
                                            &dl_id,
                                            &urls,
                                            &gapfill_options,
                                            &queue_clone,
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                emit_download_log(
                                                    &app_clone,
                                                    &dl_id,
                                                    "Gap-fill complete — skipped tracks \
                                                     recovered in lossless format",
                                                );
                                                log::info!("Download {dl_id} gap-fill succeeded");
                                            }
                                            Err(e) if e.contains("cancelled") => {
                                                log::info!("Download {dl_id} gap-fill cancelled");
                                                return;
                                            }
                                            Err(e) => {
                                                log::warn!("Download {dl_id} gap-fill failed: {e}");
                                                emit_download_log(
                                                    &app_clone,
                                                    &dl_id,
                                                    &format!(
                                                        "Gap-fill pass failed ({e}) — \
                                                         continuing with partial download"
                                                    ),
                                                );
                                            }
                                        }

                                        // Re-acquire lock for downstream logic
                                        q = queue_clone.lock().await;
                                    } else {
                                        // No partial success or no viable gap-fill chain
                                        log::info!(
                                            "Download {dl_id} codec error with native priority \
                                             (existing={existing_audio}, skipped={skip_count}, \
                                             wrapper={wrapper_active}): {error_msg}"
                                        );
                                        emit_download_log(
                                            &app_clone,
                                            &dl_id,
                                            "GAMDL tried all formats in priority chain \
                                             — none available for this content",
                                        );
                                    }
                                    // Fall through to partial-success recovery below
                                } else {
                                    // GAMDL < 2.9.1 — use MeedyaDL's own fallback system
                                    log::info!(
                                        "Download {dl_id} exited 0 with codec error, \
                                     attempting fallback: {error_msg}"
                                    );
                                    let settings = load_settings_for_queue(&app_clone);
                                    q.set_error(&dl_id, &error_msg);
                                    q.on_task_finished();

                                    if let Some((new_options, fb_idx, chain_len)) =
                                        q.try_fallback(&dl_id, &settings)
                                    {
                                        let fallback_codec = new_options
                                            .song_codec
                                            .as_ref()
                                            .map(|c| c.to_cli_string().to_string())
                                            .unwrap_or_else(|| "unknown".to_string());
                                        let total_fallbacks = chain_len.saturating_sub(1);
                                        log::info!(
                                        "Download {dl_id} will retry with fallback codec: {fallback_codec} ({fb_idx} of {total_fallbacks})"
                                    );
                                        drop(q);
                                        emit_download_log(
                                        &app_clone,
                                        &dl_id,
                                        &format!("Format not available — trying {fallback_codec} (fallback {fb_idx} of {total_fallbacks})"),
                                    );
                                        save_queue_to_disk(&app_clone, &queue_clone).await;
                                        process_queue(app_clone.clone(), queue_clone.clone()).await;
                                        return;
                                    }
                                    // Fallback chain exhausted — fall through to error below
                                    emit_download_log(
                                        &app_clone,
                                        &dl_id,
                                        "All audio formats exhausted — no compatible format found",
                                    );
                                }
                            }

                            // Partial-success recovery: GAMDL exited 0 with codec
                            // skip warnings, meaning some tracks were unavailable in
                            // the requested format but others downloaded successfully.
                            // Find the actual album directory (not the base output dir)
                            // containing audio files. This handles GAMDL 2.9.1+ which
                            // doesn't emit "Saved to:" lines for album downloads.
                            if has_codec_error && !has_io_error {
                                if let Some(item) =
                                    q.items.iter_mut().find(|i| i.status.id == dl_id)
                                {
                                    if let Some(ref base_dir) =
                                        item.merged_options.output_path.clone()
                                    {
                                        let base_path = std::path::Path::new(base_dir);
                                        // Use artist/album names from the queue item for targeted search (#452)
                                        let artist_hint = item.status.artist_name.as_deref();
                                        let album_hint = item.status.album_name.as_deref();
                                        // Find the actual album directory within the base output dir
                                        if let Some(album_dir) = find_album_directory(base_path, artist_hint, album_hint) {
                                            item.status.output_path = Some(album_dir);
                                            item.status.output_is_directory = true;
                                            log::info!(
                                                "Download {dl_id} partial success: found album \
                                             directory with files despite codec skip warnings"
                                            );
                                        }
                                    }
                                }
                            }

                            if has_io_error && !has_codec_error {
                                // Filesystem I/O error recovery: GAMDL exited 0 with IO
                                // errors (e.g., cloud storage timeout writing Cover.jpg)
                                // but no codec errors. Audio files were likely downloaded
                                // to the output directory despite the IO errors on
                                // ancillary operations (cover art, lyrics sidecar).
                                //
                                // Find the actual album directory so "Open Folder" works.
                                if let Some(item) =
                                    q.items.iter_mut().find(|i| i.status.id == dl_id)
                                {
                                    if let Some(ref base_dir) =
                                        item.merged_options.output_path.clone()
                                    {
                                        let base_path = std::path::Path::new(base_dir);
                                        let artist_hint = item.status.artist_name.as_deref();
                                        let album_hint = item.status.album_name.as_deref();
                                        if let Some(album_dir) = find_album_directory(base_path, artist_hint, album_hint) {
                                            item.status.output_path = Some(album_dir);
                                            item.status.output_is_directory = true;
                                        } else {
                                            // Fallback: use base dir if no album subdir found
                                            item.status.output_path = Some(base_dir.clone());
                                            item.status.output_is_directory = true;
                                        }
                                    }
                                }
                                log::warn!(
                                    "Download {dl_id} completed with IO errors — recovering with \
                                 configured output path"
                                );
                                // Fall through to normal completion with IO warnings
                            }

                            // General fallback: GAMDL 2.9.x with native
                            // --song-codec-priority doesn't emit "Saved to:"
                            // for album downloads. Scan the output directory
                            // for audio files as a last resort before declaring
                            // failure.
                            if let Some(item) =
                                q.items.iter_mut().find(|i| i.status.id == dl_id)
                            {
                                if item.status.output_path.is_none() {
                                    if let Some(ref base_dir) =
                                        item.merged_options.output_path.clone()
                                    {
                                        let base_path =
                                            std::path::Path::new(base_dir);
                                        let artist_hint = item.status.artist_name.as_deref();
                                        let album_hint = item.status.album_name.as_deref();
                                        if let Some(album_dir) =
                                            find_album_directory(base_path, artist_hint, album_hint)
                                        {
                                            item.status.output_path =
                                                Some(album_dir);
                                            item.status.output_is_directory =
                                                true;
                                            log::info!(
                                                "Download {dl_id} recovered: \
                                                 found album directory on disk \
                                                 despite no 'Saved to:' output \
                                                 from GAMDL"
                                            );
                                        }
                                    }
                                }
                            }

                            // Re-check output: partial-success, IO recovery,
                            // or disk-scan fallback may have set output_path.
                            let has_output_now = q
                                .items
                                .iter()
                                .find(|i| i.status.id == dl_id)
                                .and_then(|i| i.status.output_path.as_ref())
                                .is_some();

                            if !has_output_now {
                                // Terminal: no output, no IO recovery, no files on
                                // disk despite codec fallback exhausted.
                                let error_msg = if has_io_error {
                                    format!(
                                        "Output path may be unreachable or too slow. \
                                     Check your storage connection and try a local \
                                     output path. Details: {}",
                                        warnings.last().cloned().unwrap_or_default()
                                    )
                                } else if let Some(last_warning) = warnings.last() {
                                    format!(
                                        "Download completed but no output files were \
                                     produced: {last_warning}"
                                    )
                                } else {
                                    "Download completed but no output files were produced. \
                                 Check the Activity Log for details."
                                        .to_string()
                                };
                                log::warn!(
                                    "Download {dl_id} exited 0 but produced no output: {error_msg}"
                                );
                                q.set_error(&dl_id, &error_msg);
                                q.on_task_finished();
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!("Download failed: {error_msg}"),
                                );

                                // Auto-retry without wrapper on success-path
                                // terminal errors (same logic as the Err path).
                                if wrapper_url_for_logging.is_some() {
                                    let ar_settings = load_settings_for_queue(&app_clone);
                                    if ar_settings.auto_retry_without_wrapper {
                                        log::info!(
                                            "Auto-retrying download {dl_id} without \
                                         wrapper (success-path terminal error)"
                                        );
                                        emit_download_log(
                                            &app_clone,
                                            &dl_id,
                                            "Wrapper failed — auto-retrying without wrapper",
                                        );
                                        let retried = {
                                            let mut q = queue_clone.lock().await;
                                            q.retry_without_wrapper(&dl_id, &ar_settings)
                                        };
                                        if retried {
                                            save_queue_to_disk(&app_clone, &queue_clone).await;
                                            let _ = app_clone.emit("download-queued", &dl_id);
                                            process_queue(app_clone.clone(), queue_clone.clone())
                                                .await;
                                            return;
                                        }
                                    }
                                }

                                // Save error report for user-reportable diagnostics.
                                // Include a redacted settings snapshot for context.
                                let mut err_ctx = settings_snapshot_for_context(&app_clone);
                                if let Some(ref url) = urls.first() {
                                    err_ctx.insert("url".to_string(), url.to_string());
                                }
                                err_ctx.insert(
                                    "error_category".to_string(),
                                    if has_io_error { "io" } else { "codec" }.to_string(),
                                );
                                let report = CrashReport {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                                    os: std::env::consts::OS.to_string(),
                                    arch: std::env::consts::ARCH.to_string(),
                                    source: "download_error".to_string(),
                                    panic_message: Some(error_msg.clone()),
                                    location: None,
                                    backtrace: None,
                                    context: err_ctx,
                                };
                                if let Err(e) =
                                    crash_report_service::save_error_report(&app_clone, report)
                                {
                                    log::debug!("Failed to save download error report: {e}");
                                }

                                // Record failed download in history
                                {
                                    let q = queue_clone.lock().await;
                                    let created = q.get_status().iter().find(|s| s.id == dl_id)
                                        .map(|s| s.created_at.clone()).unwrap_or_default();
                                    drop(q);
                                    history_service::save_history_entry(
                                        &app_clone,
                                        history_service::HistoryEntry {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            url: urls.first().cloned().unwrap_or_default(),
                                            title: None,
                                            artist: None,
                                            album: None,
                                            codec: None,
                                            file_path: None,
                                            started_at: created,
                                            completed_at: chrono::Utc::now().to_rfc3339(),
                                            status: "failed".to_string(),
                                            error_message: Some(error_msg.clone()),
                                        },
                                    );
                                }

                                save_queue_to_disk(&app_clone, &queue_clone).await;

                                // Emit error guidance to the activity log
                                let error_category = process::classify_error(&error_msg);
                                let guidance = process::error_guidance(error_category);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!("💡 {guidance}"),
                                );

                                let _ = app_clone.emit(
                                    "download-error",
                                    serde_json::json!({
                                        "download_id": dl_id,
                                        "error": error_msg,
                                        "category": error_category,
                                        "guidance": guidance,
                                    }),
                                );

                                // Send a desktop notification for the terminal failure
                                send_desktop_notification(
                                    &app_clone,
                                    "Download Failed",
                                    &format!("Download failed: {error_msg}"),
                                );

                                // Spawn companion downloads even on failure —
                                // the companion codec may succeed where the primary
                                // format was unavailable.
                                let traits = read_audio_traits(&queue_clone, &dl_id).await;
                                let _ = spawn_companion_downloads(
                                    &app_clone,
                                    &queue_clone,
                                    &dl_id,
                                    &urls,
                                    &primary_codec_for_companions,
                                    &companion_base_options,
                                    &shutdown_clone,
                                    uses_native_priority,
                                    &traits,
                                );

                                // Continue processing remaining queued items
                                process_queue(app_clone.clone(), queue_clone.clone()).await;
                                return;
                            }
                        }

                        // Mark as processing (enrichment + companions still running).
                        // The item stays in Processing state until all background
                        // tasks complete, keeping the progress bar active.
                        q.set_processing(&dl_id);
                        let mut all_warnings = warnings.clone();
                        if warnings.iter().any(|w| process::is_io_error(w)) {
                            all_warnings.push(
                                "Some files may be incomplete due to storage I/O \
                             errors. Consider using a local output path instead \
                             of cloud storage."
                                    .to_string(),
                            );
                        }
                        if !all_warnings.is_empty() {
                            q.add_warnings(&dl_id, &all_warnings);
                        }
                        q.on_task_finished();

                        // Extract output_path, codec_used, and history metadata while we have the lock
                        let status = q.get_status();
                        let item = status.iter().find(|s| s.id == dl_id);
                        let result = (
                            item.and_then(|s| s.output_path.clone()),
                            item.and_then(|s| s.codec_used.clone()),
                            item.and_then(|s| s.current_track.clone()),
                            item.map(|s| s.created_at.clone()),
                        );
                        drop(q);
                        result
                    };

                    // Verify the output path actually exists on disk. If GAMDL
                    // reported a path but the file/folder is missing, add a warning.
                    if let Some(ref path) = output_path_for_artwork {
                        if !std::path::Path::new(path).exists() {
                            log::warn!("Download {dl_id} output path does not exist: {path}");
                            let mut q = queue_clone.lock().await;
                            q.add_warnings(
                                &dl_id,
                                &["Output path reported by GAMDL does not exist on disk"
                                    .to_string()],
                            );
                            drop(q);
                        }
                    }

                    log::info!("Download {dl_id} completed successfully");

                    // Build a display name for the notification body. Prefer the
                    // track name from the queue item, fall back to the URL basename.
                    let _notification_name = history_track_name.clone().unwrap_or_else(|| {
                        urls.first()
                            .and_then(|u| u.rsplit('/').next())
                            .unwrap_or("Download")
                            .to_string()
                    });

                    // Record successful download in history
                    history_service::save_history_entry(
                        &app_clone,
                        history_service::HistoryEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            url: urls.first().cloned().unwrap_or_default(),
                            title: history_track_name,
                            artist: None,
                            album: None,
                            codec: completed_codec.clone(),
                            file_path: output_path_for_artwork.clone(),
                            started_at: history_created_at.unwrap_or_default(),
                            completed_at: chrono::Utc::now().to_rfc3339(),
                            status: "success".to_string(),
                            error_message: None,
                        },
                    );

                    // Persist queue state: completed item is now in terminal state,
                    // so it will be excluded from the persistence file (only
                    // Queued/Downloading/Processing items are persisted).
                    save_queue_to_disk(&app_clone, &queue_clone).await;

                    // Notify frontend of successful completion
                    // NOTE: download-complete event and desktop notification are
                    // deferred to the completion task (after enrichment + companions
                    // finish) so they don't fire prematurely.

                    // === Unified post-download enrichment (background, fire-and-forget) ===
                    // After a successful download, run all post-processing in a single
                    // background task:
                    //   1. Metadata enrichment (codec tags + source tags + channel
                    //      detection + API metadata)
                    //   2. Enhanced LRC conversion (TTML → word-by-word LRC, opt-in)
                    //   3. Animated artwork download (reuses API data from step 1)
                    //   4. AcoustID fingerprinting (opt-in, embedded Chromaprint)
                    //   5. ReplayGain loudness analysis (opt-in, uses FFmpeg)
                    //
                    // The enrichment fetches Apple Music API data once and shares it
                    // with animated artwork, avoiding duplicate API calls.
                    //
                    // This runs in a separate tokio task so it doesn't block the queue
                    // from processing the next download. Failures are logged but never
                    // propagate to the user or affect the download status.
                    let enrichment_handle = if let Some(output_dir) = output_path_for_artwork.clone() {
                        let enrich_app = app_clone.clone();
                        let enrich_urls = urls.clone();
                        let enrich_dl_id = dl_id.clone();
                        let enrich_codec_str = completed_codec.clone();
                        let enrich_shutdown = shutdown_clone.clone();
                        let enrich_native_priority = uses_native_priority;
                        let enrich_is_apple_music = is_apple_music;
                        let enrich_queue = queue_clone.clone();
                        let enrich_started_at = download_started_at.clone();
                        Some(tokio::spawn(async move {
                            // Determine the album directory from the output path.
                            // For single tracks, output_path is a file -- use its parent.
                            // For albums, output_path is already the directory.
                            let dir = std::path::Path::new(&output_dir);
                            let mut album_dir = if dir.is_dir() {
                                output_dir.clone()
                            } else {
                                dir.parent().map_or_else(
                                    || output_dir.clone(),
                                    |p| p.to_string_lossy().to_string(),
                                )
                            };

                            // Helper: update the processing label in the queue item
                            // so the progress bar shows what's happening.
                            // Also appends album context (Artist: Album) when available.
                            let label_queue = enrich_queue.clone();
                            let label_dl_id = enrich_dl_id.clone();
                            let set_label = move |label: &str| {
                                if let Ok(mut q) = label_queue.try_lock() {
                                    // Look up album context for richer labels
                                    let context = q
                                        .items
                                        .iter()
                                        .find(|i| i.status.id == label_dl_id)
                                        .map(|item| {
                                            let artist = item.status.artist_name.as_deref().unwrap_or("");
                                            let album = item.status.album_name.as_deref().unwrap_or("");
                                            if !artist.is_empty() && !album.is_empty() {
                                                format!(" — {artist}: {album}")
                                            } else if !album.is_empty() {
                                                format!(" — {album}")
                                            } else {
                                                String::new()
                                            }
                                        })
                                        .unwrap_or_default();
                                    let full_label = format!("{label}{context}");
                                    q.set_processing_label(&label_dl_id, &full_label);
                                }
                            };

                            // Helper: get album context for activity log messages.
                            let log_context_queue = enrich_queue.clone();
                            let log_context_id = enrich_dl_id.clone();
                            let album_context = move || -> String {
                                if let Ok(q) = log_context_queue.try_lock() {
                                    if let Some(item) = q.items.iter().find(|i| i.status.id == log_context_id) {
                                        let artist = item.status.artist_name.as_deref().unwrap_or("");
                                        let album = item.status.album_name.as_deref().unwrap_or("");
                                        if !artist.is_empty() && !album.is_empty() {
                                            return format!(" — {artist}: {album}");
                                        } else if !album.is_empty() {
                                            return format!(" — {album}");
                                        }
                                    }
                                }
                                String::new()
                            };

                            // Guard: skip Apple Music-specific enrichment for
                            // non-Apple Music services. The entire enrichment
                            // pipeline (metadata tags, lyrics, artwork, AcoustID,
                            // ReplayGain, music video companions) is currently
                            // Apple Music-specific. Other services will get their
                            // own enrichment pipelines when implemented.
                            if !enrich_is_apple_music {
                                log::info!(
                                    "Skipping enrichment for non-Apple Music download {}",
                                    enrich_dl_id,
                                );
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Enrichment skipped (not an Apple Music download)",
                                );
                                return;
                            }

                            // Log enrichment settings summary so users can see
                            // which steps will run in the Activity Log.
                            let enrich_settings = load_settings_for_queue(&enrich_app);
                            emit_download_log(
                            &enrich_app,
                            &enrich_dl_id,
                            &format!(
                                "──── Enrichment starting (lrc: {}, artwork: {}, artist_promo: {}, acoustid: {}, replaygain: {}, video: {}) ────",
                                if enrich_settings.enhanced_lrc { "on" } else { "off" },
                                if enrich_settings.animated_artwork_enabled { "on" } else { "off" },
                                if enrich_settings.artist_promo_video_enabled { "on" } else { "off" },
                                if enrich_settings.acoustid_enabled { "on" } else { "off" },
                                if enrich_settings.replaygain_enabled { "on" } else { "off" },
                                if enrich_settings.music_video_companion { "on" } else { "off" },
                            ),
                        );

                            set_label("Enriching metadata tags...");
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ Metadata enrichment started{}", album_context()));
                            // --- Step 1: Enriched metadata tagging ---
                            // Parse the codec string and run full enrichment (codec tags,
                            // source tags, channel detection, API metadata). Returns the
                            // fetched AlbumMetadata for reuse by animated artwork.
                            let codec = enrich_codec_str.as_deref().and_then(|s| {
                                let parsed = SongCodec::from_cli_string(s);
                                if parsed.is_none() {
                                    log::warn!(
                                        "Unrecognised codec string '{}' for enrichment of {}, skipping codec tags",
                                        s,
                                        enrich_dl_id,
                                    );
                                }
                                parsed
                            });

                            emit_verbose_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                &format!(
                                    "Starting enrichment: requested_codec={}, native_priority={}, output_dir={}",
                                    enrich_codec_str.as_deref().unwrap_or("unknown"),
                                    enrich_native_priority,
                                    album_dir,
                                ),
                            );
                            // --- Step 0: iTunes Lookup API enrichment (#454) ---
                            // Run FIRST (no auth required). Writes baseline metadata
                            // (country, disc count) that Apple Music API can overwrite.
                            {
                                let album_id = enrich_urls.iter()
                                    .find_map(|u| super::apple_music_api::parse_apple_music_url(u))
                                    .map(|p| p.album_id);

                                if let Some(aid) = album_id {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "iTunes API: fetching supplementary metadata...",
                                    );
                                    match super::apple_music_api::fetch_itunes_lookup(&aid).await {
                                        Ok(Some(itunes_tracks)) => {
                                            let count = super::metadata_tag_service::apply_itunes_supplementary_tags(
                                                &album_dir,
                                                &itunes_tracks,
                                            );
                                            if count > 0 {
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &format!("iTunes API: enriched {count} file(s) with supplementary metadata (country, disc count)"),
                                                );
                                            }
                                        }
                                        Ok(None) => {
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                "iTunes API: album not found in iTunes catalog",
                                            );
                                        }
                                        Err(e) => {
                                            log::debug!("iTunes Lookup failed for {enrich_dl_id}: {e}");
                                        }
                                    }
                                }
                            }

                            emit_download_log(
                            &enrich_app,
                            &enrich_dl_id,
                            "Enriching metadata (codec tags, source tags, channel detection, Apple Music API)...",
                        );
                            let album_metadata = if let Some(ref codec) = codec {
                                match super::metadata_tag_service::apply_enriched_metadata_tags(
                                    &enrich_app,
                                    &album_dir,
                                    codec,
                                    &enrich_urls,
                                    None, // No pre-fetched metadata; will fetch from API if possible
                                    Some((&enrich_app, &enrich_dl_id)),
                                    enrich_native_priority,
                                    enrich_settings.content_advisory_in_filenames,
                                )
                                .await
                                {
                                    Ok((count, metadata, renamed_path)) => {
                                        if count > 0 {
                                            let api_note = if metadata.is_some() {
                                                " (including Apple Music API metadata: ISRC, UPC, genre)"
                                            } else {
                                                " (Apple Music API metadata unavailable)"
                                            };
                                            log::info!(
                                            "Enriched {count} file(s) with metadata for {enrich_dl_id}"
                                        );
                                            emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Enriched {count} file(s) with metadata tags{api_note}"),
                                        );
                                        }
                                        // Update album_dir if advisory suffix renamed the folder
                                        if let Some(ref new_path) = renamed_path {
                                            album_dir = new_path.clone();
                                            // Update queue item's output_path so "Open Folder" works
                                            if let Ok(mut q) = enrich_queue.try_lock() {
                                                if let Some(item) = q
                                                    .items
                                                    .iter_mut()
                                                    .find(|i| i.status.id == enrich_dl_id)
                                                {
                                                    item.status.output_path =
                                                        Some(new_path.clone());
                                                }
                                            }
                                        }
                                        // Update artist_name from API metadata for progress bar display
                                        if let Some(ref meta) = metadata {
                                            if let Ok(mut q) = enrich_queue.try_lock() {
                                                if let Some(item) = q
                                                    .items
                                                    .iter_mut()
                                                    .find(|i| i.status.id == enrich_dl_id)
                                                {
                                                    if item.status.artist_name.is_none() {
                                                        item.status.artist_name =
                                                            meta.artist_name.clone();
                                                    }
                                                    if item.status.album_name.is_none() {
                                                        item.status.album_name =
                                                            meta.album_name.clone();
                                                    }
                                                }
                                            }
                                        }
                                        metadata
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Metadata enrichment failed for {enrich_dl_id}: {e}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Metadata enrichment failed: {e}"),
                                        );
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            if album_metadata.is_some() {
                                emit_download_log(&enrich_app, &enrich_dl_id, &format!("✓ Metadata enrichment completed{}", album_context()));
                            } else {
                                emit_download_log(&enrich_app, &enrich_dl_id, &format!("⚠ Metadata enrichment completed without API data — some enrichment steps may be limited{}", album_context()));
                            }

                            // --- Post-step 1: Rename cover art per user setting (#448) ---
                            // GAMDL saves static cover art as Cover.<ext>. Rename to the
                            // user's configured name (default: FrontCover for consistency
                            // with animated artwork FrontCover.mp4/PortraitCover.mp4).
                            rename_cover_art(&album_dir, enrich_settings.cover_art_name.to_filename_stem());

                            // --- Step 1a: Dump raw API response JSON (verbose diagnostics) ---
                            // When verbose logging is enabled, write the raw Apple Music API
                            // response to a JSON file in the album output directory. This lets
                            // developers confirm the API integration is returning correct data
                            // (e.g., after endpoint changes like amp-api → api.music.apple.com).
                            // File is named `<AlbumName>-applemusic-data.json` in the album dir.
                            if crate::utils::activity_log::is_verbose_logging() {
                                if let Some(ref metadata) = album_metadata {
                                    let album_name = metadata
                                        .album_name
                                        .as_deref()
                                        .unwrap_or("Unknown Album");
                                    // Sanitize album name for filesystem safety: strip characters
                                    // that are illegal or problematic on macOS/Windows/Linux.
                                    let safe_name: String = album_name
                                        .chars()
                                        .map(|c| match c {
                                            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                                            _ => c,
                                        })
                                        .collect();
                                    let json_filename = format!("{safe_name}-applemusic-data.json");
                                    let album_dir_path = std::path::Path::new(&album_dir);
                                    match serde_json::to_string_pretty(&metadata.raw_json) {
                                        Ok(json_str) => {
                                            // Non-clobbering write: if a dump
                                            // from a prior run already sits at
                                            // the same name (or two albums
                                            // sanitise to the same stem), the
                                            // new dump lands on `...data.1.json`
                                            // instead of silently replacing it.
                                            match crate::utils::fs_safe::write_non_clobbering(
                                                album_dir_path,
                                                &json_filename,
                                                json_str.as_bytes(),
                                            ) {
                                                Ok(json_path) => {
                                                    emit_verbose_download_log(
                                                        &enrich_app,
                                                        &enrich_dl_id,
                                                        &format!(
                                                            "Apple Music API response saved to: {}",
                                                            json_path.display()
                                                        ),
                                                    );
                                                }
                                                Err(e) => {
                                                    log::debug!(
                                                        "Failed to write API response JSON for {enrich_dl_id}: {e}"
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::debug!(
                                                "Failed to serialize API response for {enrich_dl_id}: {e}"
                                            );
                                        }
                                    }
                                }
                            }

                            // Yield to the async runtime between enrichment steps so UI
                            // events (sidebar clicks, activity log updates) can be processed.
                            tokio::task::yield_now().await;
                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            // --- Step 1b: Syllable-lyrics fetch (word-level TTML) ---
                            // When Enhanced LRC is enabled and album metadata is available,
                            // check if GAMDL's TTML files have word-level timing. If not,
                            // fetch syllable-lyrics directly from Apple Music API and write
                            // upgraded TTML sidecars before the Enhanced LRC conversion step.
                            if enrich_settings.enhanced_lrc {
                                if let Some(ref metadata) = album_metadata {
                                    // Resolve JWT for API calls (premium feature resolver with web player fallback).
                                    let private_key = super::apple_music_api::get_private_key_from_keychain()
                                        .ok()
                                        .flatten();
                                    let jwt_pair = super::apple_music_api::resolve_premium_feature_token(
                                        enrich_settings.musickit_team_id.as_deref(),
                                        enrich_settings.musickit_key_id.as_deref(),
                                        private_key.as_deref(),
                                    )
                                    .ok()
                                    .flatten();
                                    if let Some((_, ref src)) = jwt_pair {
                                        log::debug!("Syllable-lyrics: using MusicKit token from {src}");
                                    }
                                    let jwt = jwt_pair.map(|(t, _)| t);

                                    // Extract Music-User-Token from cookies for subscriber-only endpoint.
                                    // Emit to activity log if the token is expired so the user knows
                                    // to re-import cookies (instead of a silent skip or HTTP 401).
                                    let music_user_token = if let Some(p) = enrich_settings.cookies_path.as_deref() {
                                        match super::apple_music_api::extract_media_user_token(p) {
                                            Ok(Some(token)) => Some(token),
                                            Ok(None) => {
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    "Word-level lyrics skipped: Apple Music cookies expired or missing. Re-import from your browser.",
                                                );
                                                None
                                            }
                                            Err(_) => None,
                                        }
                                    } else {
                                        None
                                    };

                                    if let (Some(jwt), Some(token)) = (jwt, music_user_token) {
                                        // Scan existing TTML files to find which tracks lack word-level timing
                                        let dir_for_scan = album_dir.clone();
                                        let tracks_needing_upgrade: Vec<_> = metadata.tracks.iter()
                                            .filter(|t| t.has_lyrics == Some(true))
                                            .filter(|t| {
                                                // Check if a TTML file exists and already has word-level timing
                                                let ttml_path = std::path::Path::new(&dir_for_scan);
                                                let pattern = format!("{:02} ", t.track_number);
                                                if let Ok(entries) = std::fs::read_dir(ttml_path) {
                                                    for entry in entries.flatten() {
                                                        let path = entry.path();
                                                        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttml")) {
                                                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                                                if name.starts_with(&pattern) {
                                                                    // Read the file and check for word-level timing
                                                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                                                        if content.contains("itunes:timing=\"Word\"")
                                                                            || content.contains("itunes:timing='Word'")
                                                                        {
                                                                            return false; // Already has word-level timing
                                                                        }
                                                                    }
                                                                    return true; // TTML exists but no word-level timing
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                true // No TTML file found at all
                                            })
                                            .collect();

                                        if !tracks_needing_upgrade.is_empty() {
                                            set_label("Fetching word-level lyrics...");
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!(
                                                    "Fetching word-level lyrics from Apple Music API for {} track(s)...",
                                                    tracks_needing_upgrade.len()
                                                ),
                                            );

                                            let mut upgraded = 0u32;
                                            for track in &tracks_needing_upgrade {
                                                if enrich_shutdown.is_triggered() {
                                                    break;
                                                }
                                                match super::apple_music_api::fetch_syllable_lyrics(
                                                    &jwt,
                                                    &enrich_settings.storefront,
                                                    &track.song_id,
                                                    &token,
                                                )
                                                .await
                                                {
                                                    Ok(Some(ttml_xml)) => {
                                                        // Find the matching TTML file to overwrite, or create one
                                                        let pattern = format!("{:02} ", track.track_number);
                                                        let mut written = false;
                                                        if let Ok(entries) = std::fs::read_dir(&album_dir) {
                                                            for entry in entries.flatten() {
                                                                let path = entry.path();
                                                                if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttml")) {
                                                                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                                                        if name.starts_with(&pattern) {
                                                                            if let Err(e) = std::fs::write(&path, &ttml_xml) {
                                                                                log::debug!("Failed to write TTML for track {}: {e}", track.song_id);
                                                                            } else {
                                                                                upgraded += 1;
                                                                                written = true;
                                                                            }
                                                                            break;
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        // If no existing TTML file, try to find a matching audio file and create one
                                                        if !written {
                                                            if let Ok(entries) = std::fs::read_dir(&album_dir) {
                                                                for entry in entries.flatten() {
                                                                    let path = entry.path();
                                                                    if path.extension().is_some_and(|e| {
                                                                        let e = e.to_ascii_lowercase();
                                                                        e == "m4a" || e == "m4v" || e == "mp4"
                                                                    }) {
                                                                        if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                                                                            if name.starts_with(&pattern) {
                                                                                let ttml_path = path.with_extension("ttml");
                                                                                if let Err(e) = std::fs::write(&ttml_path, &ttml_xml) {
                                                                                    log::debug!("Failed to create TTML for track {}: {e}", track.song_id);
                                                                                } else {
                                                                                    upgraded += 1;
                                                                                }
                                                                                break;
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    Ok(None) => {
                                                        log::debug!(
                                                            "No syllable-lyrics available for track {} (song {})",
                                                            track.track_number,
                                                            track.song_id
                                                        );
                                                    }
                                                    Err(e) => {
                                                        log::debug!(
                                                            "Syllable-lyrics fetch failed for track {} (song {}): {e}",
                                                            track.track_number,
                                                            track.song_id
                                                        );
                                                        // Auth errors affect all tracks — stop early
                                                        if e.contains("401") || e.contains("403") {
                                                            emit_download_log(
                                                                &enrich_app,
                                                                &enrich_dl_id,
                                                                &format!("Word-level lyrics fetch stopped: {e}"),
                                                            );
                                                            break;
                                                        }
                                                    }
                                                }
                                                // Rate-limit: small delay between API requests to
                                                // avoid hitting Apple Music API rate limits.
                                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                            }

                                            if upgraded > 0 {
                                                log::info!(
                                                    "Word-level lyrics fetched for {upgraded} track(s) for {enrich_dl_id}"
                                                );
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &format!("Word-level lyrics fetched from Apple Music API for {upgraded} track(s)"),
                                                );
                                            }
                                        }
                                    } else {
                                        set_label("Skipping word-level lyrics (no credentials)");
                                        log::debug!(
                                            "Syllable-lyrics skipped for {enrich_dl_id}: MusicKit JWT or Music-User-Token unavailable"
                                        );
                                    }
                                }

                                tokio::task::yield_now().await;
                                if enrich_shutdown.is_triggered() {
                                    log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                    return;
                                }
                            }

                            set_label("Converting lyrics (Enhanced LRC)...");
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ Lyrics processing started{}", album_context()));
                            // --- Step 2: Enhanced LRC conversion (opt-in, default on) ---
                            // When enabled, converts TTML sidecar files to Enhanced LRC
                            // with word-by-word timestamps. Saves a `.lrc` sidecar file
                            // and embeds the Enhanced LRC in M4A/M4V metadata.
                            // Falls back to standard line-level LRC for songs without
                            // word-level timing in their TTML.
                            if enrich_settings.enhanced_lrc {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Converting TTML lyrics to Enhanced LRC (word-by-word sync)...",
                                );
                                // Offload Enhanced LRC processing to a blocking thread.
                                // The function is pure sync I/O (std::fs + mp4ameta Tag)
                                // that would starve tokio on slow FUSE mounts.
                                let lrc_dir = album_dir.clone();
                                match tokio::task::spawn_blocking(move || {
                                    super::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(&lrc_dir)
                                }).await.unwrap_or_else(|e| Err(format!("LRC task panicked: {e}"))) {
                                Ok(count) if count > 0 => {
                                    log::info!(
                                        "Enhanced LRC generated for {count} file(s) for {enrich_dl_id}"
                                    );
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        &format!("Enhanced LRC generated for {count} file(s)"),
                                    );
                                }
                                Ok(_) => {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "No TTML lyrics files found for Enhanced LRC conversion",
                                    );
                                }
                                Err(e) => {
                                    log::debug!(
                                        "Enhanced LRC conversion skipped for {enrich_dl_id}: {e}"
                                    );
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        &format!("Enhanced LRC conversion skipped: {e}"),
                                    );
                                }
                            }
                            }

                            // --- Step 2b: Lyrics format fallback ---
                            // If the primary lyrics format (TTML when Enhanced LRC is
                            // active) didn't produce lyrics for all tracks, retry with
                            // fallback formats. Audio: TTML → LRC → SRT.
                            // Video: TTML → SRT → LRC. The chain stops as soon as
                            // lyrics coverage matches the number of media files.
                            tokio::task::yield_now().await;
                            if enrich_settings.lyrics_fallback_enabled
                                && !enrich_settings.no_synced_lyrics
                                && !enrich_shutdown.is_triggered()
                            {
                                run_lyrics_fallback(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &album_dir,
                                    &enrich_urls,
                                    &enrich_settings,
                                )
                                .await;
                            }

                            // --- Step 2c: WebVTT subtitle generation (opt-in) ---
                            // When enabled, converts existing lyrics sidecars (TTML, SRT,
                            // or LRC) to WebVTT (.vtt) format for web video player
                            // compatibility. Source priority: TTML → SRT → LRC.
                            // Runs after lyrics fallback so all available sources are present.
                            tokio::task::yield_now().await;
                            if enrich_settings.generate_webvtt && !enrich_shutdown.is_triggered() {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Generating WebVTT subtitles...",
                                );
                                let vtt_dir = album_dir.clone();
                                match tokio::task::spawn_blocking(move || {
                                    super::webvtt_service::generate_webvtt_for_directory(&vtt_dir)
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("VTT task panicked: {e}")))
                                {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Generated {count} WebVTT file(s) for {enrich_dl_id}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Generated {count} WebVTT subtitle(s)"),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "No lyrics files found for WebVTT conversion",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "WebVTT generation skipped for {enrich_dl_id}: {e}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("WebVTT generation skipped: {e}"),
                                        );
                                    }
                                }
                            }

                            // --- Step 2d: Rich SRT generation (opt-in, default on) ---
                            // When enabled, converts TTML files to format-rich SRT with
                            // styling tags (bold, italic, underline, colour). If a plain
                            // SRT already exists (from GAMDL or lyrics fallback), the
                            // rich SRT replaces it since TTML has richer data.
                            tokio::task::yield_now().await;
                            if enrich_settings.generate_rich_srt && !enrich_shutdown.is_triggered()
                            {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Generating rich SRT subtitles from TTML...",
                                );
                                let srt_dir = album_dir.clone();
                                match tokio::task::spawn_blocking(move || {
                                    super::rich_srt_service::generate_rich_srt_for_directory(
                                        &srt_dir,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("Rich SRT task panicked: {e}")))
                                {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Generated {count} rich SRT file(s) for {enrich_dl_id}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Generated {count} rich SRT subtitle(s)"),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "No TTML files found for rich SRT generation",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Rich SRT generation skipped for {enrich_dl_id}: {e}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Rich SRT generation skipped: {e}"),
                                        );
                                    }
                                }
                            }

                            // --- Step 2e: Subtitle embedding (opt-in) ---
                            // When enabled, embeds SRT and WebVTT sidecar content into
                            // MP4/M4A/M4V containers as freeform atoms.
                            tokio::task::yield_now().await;
                            if enrich_settings.embed_subtitles && !enrich_shutdown.is_triggered() {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Embedding subtitles in media files...",
                                );
                                let embed_dir = album_dir.clone();
                                match tokio::task::spawn_blocking(move || {
                                    super::rich_srt_service::embed_subtitles_for_directory(
                                        &embed_dir,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| {
                                    Err(format!("Subtitle embed task panicked: {e}"))
                                }) {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Embedded subtitles in {count} file(s) for {enrich_dl_id}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Embedded subtitles in {count} file(s)"),
                                        );
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::debug!(
                                            "Subtitle embedding skipped for {enrich_dl_id}: {e}"
                                        );
                                    }
                                }
                            }

                            // --- Step 2f: ASS subtitle generation (opt-in) ---
                            // When enabled, generates ASS (Advanced SubStation Alpha)
                            // subtitle files from TTML or WebVTT with rich styling
                            // (colours, bold, italic, positioning, background vocals).
                            tokio::task::yield_now().await;
                            if enrich_settings.generate_ass && !enrich_shutdown.is_triggered() {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Generating ASS subtitles...",
                                );
                                let ass_dir = album_dir.clone();
                                match tokio::task::spawn_blocking(move || {
                                    super::ass_subtitle_service::generate_ass_for_directory(
                                        &ass_dir,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("ASS task panicked: {e}")))
                                {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Generated {count} ASS file(s) for {enrich_dl_id}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Generated {count} ASS subtitle(s)"),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "No source files found for ASS generation",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "ASS generation skipped for {enrich_dl_id}: {e}"
                                        );
                                    }
                                }
                            }

                            tokio::task::yield_now().await;
                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("✓ Lyrics processing completed{}", album_context()));

                            set_label("Downloading animated artwork...");
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ Animated artwork started{}", album_context()));
                            // --- Step 3: Animated artwork download ---
                            // Reuse the AlbumMetadata from enrichment to avoid a
                            // duplicate API call. Falls back to a fresh API call
                            // if enrichment didn't produce metadata.
                            if enrich_settings.animated_artwork_enabled {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Downloading animated artwork...",
                                );
                                let artwork_result = if let Some(ref metadata) = album_metadata {
                                    super::animated_artwork_service::process_album_artwork_from_metadata(
                                    &enrich_app,
                                    metadata,
                                    &album_dir,
                                ).await
                                } else {
                                    super::animated_artwork_service::process_album_artwork(
                                        &enrich_app,
                                        &enrich_urls,
                                        &album_dir,
                                    )
                                    .await
                                };

                                match artwork_result {
                                    Ok(result) => {
                                        if result.square_downloaded || result.portrait_downloaded {
                                            log::info!(
                                                "Animated artwork downloaded for {enrich_dl_id}"
                                            );
                                            emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "Animated artwork downloaded (square: {}, portrait: {})",
                                                if result.square_downloaded { "yes" } else { "no" },
                                                if result.portrait_downloaded { "yes" } else { "no" },
                                            ),
                                        );

                                            // Hide artwork files if enabled in settings
                                            if enrich_settings.hide_animated_artwork {
                                                let dir = std::path::Path::new(&album_dir);
                                                if result.square_downloaded {
                                                    if let Err(e) =
                                                        super::animated_artwork_service::hide_file(
                                                            &dir.join("FrontCover.mp4"),
                                                        )
                                                        .await
                                                    {
                                                        log::debug!(
                                                            "Failed to hide FrontCover.mp4: {e}"
                                                        );
                                                    }
                                                }
                                                if result.portrait_downloaded {
                                                    if let Err(e) =
                                                        super::animated_artwork_service::hide_file(
                                                            &dir.join("PortraitCover.mp4"),
                                                        )
                                                        .await
                                                    {
                                                        log::debug!(
                                                            "Failed to hide PortraitCover.mp4: {e}"
                                                        );
                                                    }
                                                }
                                            }

                                            let _ = enrich_app
                                                .emit("artwork-downloaded", &enrich_dl_id);
                                        } else {
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                "No animated artwork available for this album",
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Animated artwork skipped for {enrich_dl_id}: {e}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Animated artwork skipped: {e}"),
                                        );
                                    }
                                }
                            } else {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Animated artwork disabled in settings",
                                );
                            }

                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            // --- Step 3b: Artist promo video download (opt-in) ---
                            // Downloads the artist's animated background video from Apple Music
                            // and saves it as ArtistSpotlightCover.mp4 in the artist folder (parent of
                            // the album directory). Uses the artist_id from album metadata.
                            // Skipped for compilation albums (Various Artists) where there is
                            // no single primary artist (#453).
                            if enrich_settings.artist_promo_video_enabled {
                                // Skip for compilation albums — no meaningful artist to fetch
                                let is_compilation = album_metadata
                                    .as_ref()
                                    .and_then(|m| m.is_compilation)
                                    .unwrap_or(false);

                                if is_compilation {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "Artist promo video skipped (compilation album)",
                                    );
                                } else {
                                    // Extract artist ID and storefront from album metadata or URL
                                    let artist_id = album_metadata
                                        .as_ref()
                                        .and_then(|m| m.artist_id.clone());
                                    let storefront = enrich_urls
                                        .iter()
                                        .find_map(|u| super::apple_music_api::parse_apple_music_url(u))
                                        .map(|p| p.storefront)
                                        .unwrap_or_else(|| enrich_settings.storefront.clone());

                                    if let Some(aid) = artist_id {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "Downloading artist promo video...",
                                        );
                                        match super::animated_artwork_service::download_artist_promo_video(
                                            &enrich_app,
                                            &aid,
                                            &storefront,
                                            &album_dir,
                                        )
                                        .await
                                        {
                                            Ok(true) => {
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    "Artist promo video downloaded",
                                                );
                                            }
                                            Ok(false) => {
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    "No artist promo video available (or already downloaded)",
                                                );
                                            }
                                            Err(e) => {
                                                log::debug!(
                                                    "Artist promo video failed for {enrich_dl_id}: {e}"
                                                );
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &format!("Artist promo video skipped: {e}"),
                                                );
                                            }
                                        }
                                    } else {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "Artist promo video skipped (no artist ID in metadata)",
                                        );
                                    }
                                }
                            }

                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("✓ Animated artwork completed{}", album_context()));

                            set_label("AcoustID fingerprinting...");
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ AcoustID fingerprinting started{}", album_context()));
                            // --- Step 4: AcoustID fingerprinting (opt-in) ---
                            // When enabled, generates Chromaprint fingerprints using the
                            // embedded rusty-chromaprint library and looks up AcoustID
                            // identifiers from acoustid.org. The API key is resolved with
                            // priority: user override → compile-time embedded key → none.
                            if enrich_settings.acoustid_enabled {
                                let resolved_key = super::acoustid_service::resolve_api_key(
                                    &enrich_settings.acoustid_api_key,
                                );
                                if let Some(ref api_key) = resolved_key {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "Running AcoustID fingerprinting...",
                                    );
                                    match super::acoustid_service::process_acoustid_for_directory(
                                        &album_dir, api_key,
                                    )
                                    .await
                                    {
                                        Ok(count) if count > 0 => {
                                            log::info!(
                                            "AcoustID tagged {count} file(s) for {enrich_dl_id}"
                                        );
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!("AcoustID tagged {count} file(s)"),
                                            );
                                        }
                                        Ok(_) => {
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                "AcoustID: no matches found for any files",
                                            );
                                        }
                                        Err(e) => {
                                            log::debug!("AcoustID skipped for {enrich_dl_id}: {e}");
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!("AcoustID failed: {e}"),
                                            );
                                        }
                                    }
                                } else {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "AcoustID skipped: no API key available",
                                    );
                                }
                            }

                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("✓ AcoustID fingerprinting completed{}", album_context()));

                            set_label("ReplayGain loudness analysis...");
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ ReplayGain analysis started{}", album_context()));
                            // --- Step 5: ReplayGain loudness analysis (opt-in) ---
                            // When enabled, analyses each file's loudness via FFmpeg's
                            // ebur128 filter and writes non-destructive ReplayGain tags.
                            if enrich_settings.replaygain_enabled {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &format!(
                                        "Analysing loudness (ReplayGain, ref={:.1} LUFS, clipping prevention={}, album gain={})...",
                                        enrich_settings.replaygain_reference_level,
                                        if enrich_settings.replaygain_prevent_clipping { "on" } else { "off" },
                                        if enrich_settings.replaygain_album_gain { "on" } else { "off" }
                                    ),
                                );
                                match super::replaygain_service::process_replaygain_for_directory(
                                    &enrich_app,
                                    &album_dir,
                                    enrich_settings.replaygain_reference_level,
                                    enrich_settings.replaygain_prevent_clipping,
                                    enrich_settings.replaygain_album_gain,
                                )
                                .await
                                {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                        "ReplayGain analysed {count} file(s) for {enrich_dl_id}"
                                    );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("ReplayGain analysed {count} file(s)"),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "ReplayGain: no audio files to analyse",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!("ReplayGain skipped for {enrich_dl_id}: {e}");
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("ReplayGain failed: {e}"),
                                        );
                                    }
                                }
                            }

                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("✓ ReplayGain analysis completed{}", album_context()));

                            // --- Step 6: Music video companion downloads via MusicKit (opt-in) ---
                            // When `music_video_companion` is enabled, queries the Apple Music
                            // API for music videos related to the downloaded tracks (requires
                            // MusicKit credentials). Each found video is downloaded as a
                            // separate GAMDL subprocess. Reuses album_metadata from Step 1
                            // (avoids duplicate API calls). Fire-and-forget: API failures or
                            // video download failures do NOT affect the primary download status
                            // or queue progression. Gracefully skips if MusicKit credentials
                            // are not configured — Step 6b (MusicBrainz) provides a
                            // credential-free fallback path.
                            if enrich_settings.music_video_companion
                                && !enrich_shutdown.is_triggered()
                            {
                                spawn_music_video_companion_inner(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &enrich_urls,
                                    album_metadata.as_ref(),
                                    &enrich_settings,
                                    &enrich_shutdown,
                                )
                                .await;
                            }

                            // --- Step 6b: MusicBrainz video discovery and download ---
                            // Runs when `musicbrainz_lookup` OR `music_video_companion` is
                            // enabled. Uses MusicBrainz ISRC lookups to discover music videos
                            // that the MusicKit API may have missed (or as the sole discovery
                            // method when MusicKit credentials are not configured). No
                            // credentials required. When `music_video_companion` is also
                            // enabled, Apple Music video URLs discovered here are downloaded
                            // via GAMDL (same as Step 6). Cross-platform URLs (YouTube,
                            // Spotify, etc.) are logged for future reference.
                            tokio::task::yield_now().await;
                            let run_musicbrainz = (enrich_settings.musicbrainz_lookup
                                || enrich_settings.music_video_companion)
                                && !enrich_shutdown.is_triggered();
                            if run_musicbrainz {
                                // Only run MusicBrainz lookup if we have ISRC codes from Step 1
                                if let Some(ref metadata) = album_metadata {
                                    let isrc_tracks: Vec<(String, Option<String>)> = metadata
                                        .tracks
                                        .iter()
                                        .map(|t| (t.song_id.clone(), t.isrc.clone()))
                                        .collect();

                                    if !isrc_tracks.is_empty() {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "MusicBrainz: looking up {} track(s) via ISRC...",
                                                isrc_tracks.len()
                                            ),
                                        );

                                        match super::musicbrainz_service::lookup_videos_for_tracks(
                                            &isrc_tracks,
                                        )
                                        .await
                                        {
                                            Ok(videos) if !videos.is_empty() => {
                                                // Filter for Apple Music video URLs (downloadable via GAMDL)
                                                let am_videos: Vec<_> = videos
                                                    .iter()
                                                    .filter(|v| v.platform == "apple_music")
                                                    .collect();

                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &format!(
                                                        "MusicBrainz: found {} video(s) ({} on Apple Music)",
                                                        videos.len(),
                                                        am_videos.len(),
                                                    ),
                                                );

                                                // Log all discovered platform URLs for future reference
                                                for video in &videos {
                                                    log::info!(
                                                        "MusicBrainz video for {}: {} → {}",
                                                        enrich_dl_id,
                                                        video.platform,
                                                        video.url,
                                                    );
                                                }

                                                // Download Apple Music videos when music_video_companion is enabled
                                                if enrich_settings.music_video_companion
                                                    && !am_videos.is_empty()
                                                {
                                                    emit_download_log(
                                                        &enrich_app,
                                                        &enrich_dl_id,
                                                        &format!(
                                                            "MusicBrainz: downloading {} Apple Music video(s)...",
                                                            am_videos.len(),
                                                        ),
                                                    );
                                                    for video in &am_videos {
                                                        if enrich_shutdown.is_triggered() {
                                                            break;
                                                        }
                                                        let label = video
                                                            .title
                                                            .as_deref()
                                                            .unwrap_or("unknown");
                                                        download_music_video_by_url(
                                                            &enrich_app,
                                                            &enrich_dl_id,
                                                            &video.url,
                                                            label,
                                                            &enrich_settings,
                                                        )
                                                        .await;
                                                    }
                                                }
                                            }
                                            Ok(_) => {
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    "MusicBrainz: no music videos found",
                                                );
                                            }
                                            Err(e) => {
                                                log::debug!("MusicBrainz lookup failed for {enrich_dl_id}: {e}");
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &format!("MusicBrainz lookup failed: {e}"),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            // Write/update manifest.meedyadl in the album folder.
                            // Records the source URL and per-track metadata so users
                            // can re-download by importing the manifest file.
                            write_manifest(
                                &album_dir,
                                &enrich_urls,
                                album_metadata.as_ref(),
                                &enrich_settings,
                                &enrich_started_at,
                            );
                            emit_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                "Download manifest saved to album folder",
                            );

                            emit_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                "✓ All enrichment stages completed",
                            );
                        }))
                    } else {
                        None
                    };

                    // Spawn companion downloads (codec + lyrics) as background tasks.
                    // When native priority was used, GAMDL may have silently fallen
                    // back to a different codec (e.g., Atmos requested but ALAC
                    // delivered). Detect the actual codec via ffprobe so companions
                    // are planned against reality, not the request. This prevents
                    // redundant downloads (e.g., ALAC companion when primary is
                    // already ALAC after Atmos fallback).
                    let actual_codec_for_companions = if uses_native_priority {
                        detect_actual_primary_codec(
                            &app_clone,
                            &dl_id,
                            output_path_for_artwork.as_deref(),
                            &primary_codec_for_companions,
                        )
                        .await
                    } else {
                        primary_codec_for_companions.clone()
                    };

                    // When native priority was used, force all companions to
                    // use suffixed filenames (primary has clean filenames).
                    let companion_traits =
                        read_audio_traits(&queue_clone, &dl_id).await;
                    let companion_handle = spawn_companion_downloads(
                        &app_clone,
                        &queue_clone,
                        &dl_id,
                        &urls,
                        &actual_codec_for_companions,
                        &companion_base_options,
                        &shutdown_clone,
                        uses_native_priority,
                        &companion_traits,
                    );

                    // Spawn a completion task that waits for ALL background work
                    // (enrichment + companions) before marking the item as Complete.
                    // This keeps the item in Processing state and the progress bar
                    // active until everything finishes.
                    {
                        let completion_app = app_clone.clone();
                        let completion_dl_id = dl_id.clone();
                        let completion_queue = queue_clone.clone();
                        tokio::spawn(async move {
                            // Wait for enrichment to finish with a timeout (#461).
                            // If enrichment hangs (e.g., deadlock, unresponsive API),
                            // we force completion after 10 minutes to prevent the
                            // queue from stalling indefinitely.
                            // IMPORTANT: on timeout, abort() the task so it is actually
                            // cancelled rather than merely detached (dropping a JoinHandle
                            // detaches the task and lets it keep running, which can
                            // reintroduce the cross-contamination this fix aims to prevent).
                            let enrichment_timeout = std::time::Duration::from_secs(600);
                            if let Some(mut handle) = enrichment_handle {
                                if tokio::time::timeout(enrichment_timeout, &mut handle)
                                    .await
                                    .is_err()
                                {
                                    log::warn!(
                                        "Enrichment timed out after 10 minutes for {}",
                                        completion_dl_id
                                    );
                                    emit_download_log(
                                        &completion_app,
                                        &completion_dl_id,
                                        "⚠ Enrichment timed out after 10 minutes — marking complete",
                                    );
                                    handle.abort();
                                    let _ = handle.await;
                                }
                            }
                            // Wait for companion downloads with the same timeout
                            if let Some(mut handle) = companion_handle {
                                if tokio::time::timeout(enrichment_timeout, &mut handle)
                                    .await
                                    .is_err()
                                {
                                    log::warn!(
                                        "Companion downloads timed out after 10 minutes for {}",
                                        completion_dl_id
                                    );
                                    emit_download_log(
                                        &completion_app,
                                        &completion_dl_id,
                                        "⚠ Companion downloads timed out after 10 minutes — marking complete",
                                    );
                                    handle.abort();
                                    let _ = handle.await;
                                }
                            }

                            // Post-companion advisory pass (#482).
                            // Re-apply `[Explicit]` / `[Clean]` suffixes now that
                            // companion files have landed. The primary-file pass
                            // runs inside the enrichment task before companions
                            // exist, so companion files never saw it. Reads the
                            // `rtng` atom directly from each file so no extra
                            // metadata plumbing is required.
                            {
                                let completion_settings = load_settings_for_queue(&completion_app);
                                if completion_settings.content_advisory_in_filenames {
                                    let advisory_path = {
                                        let q = completion_queue.lock().await;
                                        q.items
                                            .iter()
                                            .find(|i| i.status.id == completion_dl_id)
                                            .and_then(|i| i.status.output_path.clone())
                                    };
                                    if let Some(output_dir) = advisory_path {
                                        let dir_for_advisory = {
                                            let p = std::path::Path::new(&output_dir);
                                            if p.is_dir() {
                                                output_dir.clone()
                                            } else {
                                                p.parent()
                                                    .map(|pp| pp.to_string_lossy().to_string())
                                                    .unwrap_or(output_dir.clone())
                                            }
                                        };
                                        super::metadata_tag_service::apply_advisory_suffixes_from_tags(
                                            &dir_for_advisory,
                                        );
                                    }
                                }
                            }

                            // Mark as complete now that all background work is done
                            {
                                let mut q = completion_queue.lock().await;
                                q.set_complete(&completion_dl_id);
                                drop(q);
                            }
                            save_queue_to_disk(&completion_app, &completion_queue).await;
                            emit_download_log(
                                &completion_app,
                                &completion_dl_id,
                                "All downloads and processing complete",
                            );
                            let _ = completion_app.emit("download-complete", &completion_dl_id);

                            // Desktop notification fires AFTER all work finishes
                            send_desktop_notification(
                                &completion_app,
                                "Download Complete",
                                "All downloads and processing complete",
                            );

                            // Cascade: process the next item in the queue (#455).
                            // Moved inside completion task so the entire pipeline
                            // (download + enrichment + companions) completes before
                            // the next item starts. Prevents metadata cross-contamination.
                            process_queue(completion_app, completion_queue).await;
                        });
                    }
                }
                Err(error_msg) => {
                    // === Cancellation short-circuit ===
                    // When the user cancels an active download, the cancellation loop
                    // in run_download_with_events() kills the process and returns
                    // Err("Download cancelled by user"). The queue item's state is
                    // already set to Cancelled by cancel(). We must NOT overwrite it
                    // with Error state or create a spurious error report.
                    if error_msg == "Download cancelled by user" {
                        let mut q = queue_clone.lock().await;
                        q.on_task_finished();
                        drop(q);
                        save_queue_to_disk(&app_clone, &queue_clone).await;
                        log::info!("Download {dl_id} cancelled by user");
                        emit_download_log(&app_clone, &dl_id, "Download cancelled by user");
                        // Cascade on cancel (#455)
                        process_queue(app_clone, queue_clone).await;
                        return;
                    }

                    // === Error path ===
                    // Classify the error to determine the appropriate retry strategy.
                    // process::classify_error() returns "codec", "network", or "unknown".
                    let error_category = process::classify_error(&error_msg);
                    log::error!("Download {dl_id} failed ({error_category}): {error_msg}");

                    // Verbose: full error message + classification details
                    emit_verbose_download_log(
                        &app_clone,
                        &dl_id,
                        &format!(
                            "Error classification: category={error_category}, message={error_msg}"
                        ),
                    );

                    // Add wrapper-specific context for network errors to aid troubleshooting.
                    // Surface the wrapper URL in the Activity Log (not just the debug log)
                    // so users can see which endpoint failed.
                    if error_category == "network" {
                        if let Some(ref url) = wrapper_url_for_logging {
                            let safe_url = redact_url_query(url);
                            log::error!(
                                "Wrapper URL was: {safe_url} -- check that the wrapper \
                             service is running and reachable"
                            );
                            emit_download_log(
                                &app_clone,
                                &dl_id,
                                &format!(
                                    "Network error occurred while using wrapper at {safe_url}"
                                ),
                            );
                        }
                    }

                    // Determine if we should retry or fallback based on error category
                    let should_retry = match error_category {
                        "codec" => {
                            // Codec/format error: the requested audio codec isn't
                            // available for this track. Try the next codec in the
                            // fallback chain (e.g., atmos -> alac -> aac).
                            //
                            // Even when GAMDL >= 2.9.1 native priority was used
                            // (--song-codec-priority), we still run MeedyaDL's own
                            // fallback as a safety net. GAMDL's native priority
                            // loop may fail without exhausting all codecs (e.g.,
                            // AttributeError on None stream_info, DRM issues, or
                            // partial chain traversal). Per-codec retries via
                            // --song-codec are more reliable as a fallback path.
                            if uses_native_priority {
                                log::warn!(
                                    "Download {dl_id} codec error despite native \
                                 priority — trying per-codec fallback as \
                                 safety net"
                                );
                                emit_download_log(
                                &app_clone,
                                &dl_id,
                                "GAMDL native priority failed — trying each format individually",
                            );
                            }

                            let settings = load_settings_for_queue(&app_clone);
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();

                            if let Some((new_options, fb_idx, chain_len)) =
                                q.try_fallback(&dl_id, &settings)
                            {
                                let fallback_codec = new_options
                                    .song_codec
                                    .as_ref()
                                    .map(|c| c.to_cli_string().to_string())
                                    .unwrap_or_else(|| "unknown".to_string());
                                let total_fallbacks = chain_len.saturating_sub(1);
                                log::info!(
                                    "Download {dl_id} will retry with \
                                 fallback codec: {fallback_codec} ({fb_idx} of {total_fallbacks})"
                                );
                                drop(q);
                                emit_download_log(
                                &app_clone,
                                &dl_id,
                                &format!("Format not available — trying {fallback_codec} (fallback {fb_idx} of {total_fallbacks})"),
                            );
                                true
                            } else {
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    "All audio formats exhausted — download failed",
                                );
                                false
                            }
                        }
                        "network" => {
                            // Network error: transient connection issue.
                            // Retry with the same options (up to max_network_retries times).
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();

                            // Differentiate wrapper vs direct in activity log messages
                            let mode_context = if wrapper_url_for_logging.is_some() {
                                " (via wrapper)"
                            } else {
                                " (direct to Apple Music)"
                            };

                            if let Some((attempt, total)) = q.try_network_retry(&dl_id) {
                                // try_network_retry resets the item to Queued with same options
                                log::info!("Download {dl_id} will retry (network error, attempt {attempt} of {total})");
                                drop(q);
                                emit_download_log(
                                        &app_clone,
                                        &dl_id,
                                        &format!("Network error{mode_context} — retrying (attempt {attempt} of {total})"),
                                    );
                                true
                            } else {
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!("Network error{mode_context} — all retries exhausted"),
                                );
                                false
                            }
                        }
                        "io" => {
                            // I/O error: filesystem issue (disconnected cloud mount,
                            // full disk, stale NFS handle, read-only filesystem).
                            // Not retriable — user needs to fix the underlying issue.
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();
                            drop(q);
                            emit_download_log(
                                &app_clone,
                                &dl_id,
                                &format!(
                                    "Filesystem error — check that the output directory \
                                         is accessible and writable: {error_msg}"
                                ),
                            );
                            false
                        }
                        _ => {
                            // Non-retriable error (e.g., authentication, invalid URL).
                            // Mark as failed and don't retry.
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();
                            drop(q);
                            emit_download_log(
                                &app_clone,
                                &dl_id,
                                &format!("Download failed ({error_category}): {error_msg}"),
                            );
                            false
                        }
                    };

                    // Persist queue state after error handling (whether retrying or terminal)
                    save_queue_to_disk(&app_clone, &queue_clone).await;

                    // If no retry will occur, check auto-retry-without-wrapper
                    // before falling through to the terminal error path.
                    if !should_retry {
                        // Auto-retry without wrapper: if the download used wrapper
                        // auth and the user has opted in, automatically re-queue
                        // with wrapper disabled (cookie-based auth) instead of
                        // treating this as a terminal failure.
                        log::debug!(
                            "Download {dl_id} terminal error path: \
                         wrapper_url={}, error_category={error_category}",
                            wrapper_url_for_logging.as_deref().unwrap_or("none"),
                        );
                        if wrapper_url_for_logging.is_some() {
                            let ar_settings = load_settings_for_queue(&app_clone);
                            log::debug!(
                                "Download {dl_id} auto_retry_without_wrapper={}",
                                ar_settings.auto_retry_without_wrapper,
                            );
                            if ar_settings.auto_retry_without_wrapper {
                                log::info!(
                                    "Auto-retrying download {dl_id} without wrapper \
                                 (auto_retry_without_wrapper enabled)"
                                );
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    "Wrapper failed — auto-retrying without wrapper",
                                );
                                let retried = {
                                    let mut q = queue_clone.lock().await;
                                    let item_state =
                                        q.items.iter().find(|i| i.status.id == dl_id).map(|i| {
                                            (i.status.state.clone(), i.status.used_wrapper)
                                        });
                                    log::debug!(
                                        "Download {dl_id} before retry_without_wrapper: \
                                     state={:?}",
                                        item_state,
                                    );
                                    q.retry_without_wrapper(&dl_id, &ar_settings)
                                };
                                log::debug!(
                                    "Download {dl_id} retry_without_wrapper returned {retried}"
                                );
                                if retried {
                                    save_queue_to_disk(&app_clone, &queue_clone).await;
                                    let _ = app_clone.emit("download-queued", &dl_id);
                                    // Skip the terminal error path entirely.
                                    process_queue(app_clone, queue_clone).await;
                                    return;
                                }
                                // If retry_without_wrapper returned false (item not
                                // found or not eligible), fall through to normal
                                // terminal error handling below.
                            }
                        }

                        // Save a download error report so the user can optionally
                        // report it to GitHub Issues via Settings > Advanced.
                        // Skip for network errors — these are connectivity issues,
                        // not application bugs, and would just add noise.
                        if error_category != "network" {
                            // Start with a redacted settings snapshot, then add
                            // error-specific fields on top.
                            let mut context = settings_snapshot_for_context(&app_clone);
                            context
                                .insert("error_category".to_string(), error_category.to_string());
                            if let Some(ref url) = urls.first() {
                                context.insert("url".to_string(), url.to_string());
                            }
                            if let Some(ref ver) = gamdl_version {
                                context.insert("gamdl_version".to_string(), ver.to_string());
                            }
                            context.insert(
                                "native_priority".to_string(),
                                uses_native_priority.to_string(),
                            );
                            let report = CrashReport {
                                id: uuid::Uuid::new_v4().to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                app_version: env!("CARGO_PKG_VERSION").to_string(),
                                os: std::env::consts::OS.to_string(),
                                arch: std::env::consts::ARCH.to_string(),
                                source: "download_error".to_string(),
                                panic_message: Some(error_msg.clone()),
                                location: None,
                                backtrace: None,
                                context,
                            };
                            if let Err(e) =
                                crash_report_service::save_error_report(&app_clone, report)
                            {
                                log::debug!("Failed to save download error report: {e}");
                            }
                        } // end: skip error reports for network errors

                        // Record failed download in history
                        {
                            let q = queue_clone.lock().await;
                            let created = q.get_status().iter().find(|s| s.id == dl_id)
                                .map(|s| s.created_at.clone()).unwrap_or_default();
                            drop(q);
                            history_service::save_history_entry(
                                &app_clone,
                                history_service::HistoryEntry {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    url: urls.first().cloned().unwrap_or_default(),
                                    title: None,
                                    artist: None,
                                    album: None,
                                    codec: None,
                                    file_path: None,
                                    started_at: created,
                                    completed_at: chrono::Utc::now().to_rfc3339(),
                                    status: "failed".to_string(),
                                    error_message: Some(error_msg.clone()),
                                },
                            );
                        }

                        // Emit error guidance to the activity log
                        let guidance = process::error_guidance(error_category);
                        emit_download_log(
                            &app_clone,
                            &dl_id,
                            &format!("💡 {guidance}"),
                        );

                        let _ = app_clone.emit(
                            "download-error",
                            serde_json::json!({
                                "download_id": dl_id,
                                "error": error_msg,
                                "category": error_category,
                                "guidance": guidance,
                            }),
                        );

                        // Send a desktop notification for the terminal failure
                        send_desktop_notification(
                            &app_clone,
                            "Download Failed",
                            &format!("Download failed: {error_msg}"),
                        );

                        // Spawn companion downloads on failure — unless the error
                        // is network-related (network is down, so companions would
                        // also fail and just waste time + clutter the Activity Log).
                        if error_category != "network" {
                            let traits = read_audio_traits(&queue_clone, &dl_id).await;
                            let _ = spawn_companion_downloads(
                                &app_clone,
                                &queue_clone,
                                &dl_id,
                                &urls,
                                &primary_codec_for_companions,
                                &companion_base_options,
                                &shutdown_clone,
                                uses_native_priority,
                                &traits,
                            );
                        } else {
                            emit_download_log(
                                &app_clone,
                                &dl_id,
                                "Companion downloads skipped — network unavailable",
                            );
                        }

                        // Cascade on error path (#455): process next item after failure
                        process_queue(app_clone.clone(), queue_clone.clone()).await;
                    }
                }
            }
        });
    }) // close Box::pin(async move {
}

/// Extracts the actual Python exception message from raw stderr lines.
///
/// Python tracebacks have this structure:
/// ```text
/// Traceback (most recent call last):
///   File "foo.py", line 42, in bar
///     some_call()
/// TypeError: 'NoneType' object has no attribute 'x'
/// ```
///
/// The actual exception is the LAST non-empty, non-indented line after
/// the "Traceback" header. This function finds the last traceback block
/// in the stderr output and extracts that exception line.
///
/// Returns `None` if no traceback is found in the stderr lines.
fn extract_python_exception(stderr_lines: &[String]) -> Option<String> {
    // Find the last occurrence of "Traceback" in stderr
    let traceback_idx = stderr_lines
        .iter()
        .rposition(|line| line.trim().to_lowercase().contains("traceback"))?;

    // Walk forward from the traceback line to find the exception.
    // The exception is the last non-empty, non-indented line in the traceback block.
    let mut exception_line: Option<&str> = None;
    for line in &stderr_lines[traceback_idx + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Empty line after we've found an exception means end of traceback
            if exception_line.is_some() {
                break;
            }
            continue;
        }
        // Exception lines are NOT indented (don't start with space/tab).
        // Indented lines are stack frame details (File "...", code).
        if !line.starts_with(' ') && !line.starts_with('\t') {
            exception_line = Some(trimmed);
        }
    }

    exception_line.map(std::string::ToString::to_string)
}

/// Runs a GAMDL download while forwarding parsed events to both
/// the queue item (for status tracking) and the frontend (for UI updates).
///
/// This is the queue's version of `gamdl_service::run_gamdl()`, with two
/// key differences:
/// 1. It updates the queue item's progress (for status queries)
/// 2. It polls for cancellation every 250ms (for user cancel support)
///
/// The function builds the GAMDL command, spawns it with piped stdio,
/// starts two reader tasks (stdout + stderr), and enters a poll loop
/// that alternates between checking for process exit and cancellation.
///
/// Error messages from GAMDL's output are collected in a Vec<String>
/// (behind Arc<Mutex>) so the last error can be used as the failure
/// message if the process exits with a non-zero code. On success, returns
/// any non-fatal warning messages collected during the run (error-pattern
/// lines from GAMDL output that didn't cause a non-zero exit).
// Monolithic by necessity: manages subprocess lifecycle (spawn, stdout/stderr
// readers, cancellation polling loop, exit status handling) with shared
// Arc<Mutex> state that makes extraction impractical.
#[allow(clippy::too_many_lines)]
async fn run_download_with_events(
    app: &AppHandle,
    download_id: &str,
    urls: &[String],
    options: &GamdlOptions,
    queue: &QueueHandle,
) -> Result<Vec<String>, String> {
    log::info!(
        "Starting GAMDL download {} for {} URL(s)",
        download_id,
        urls.len()
    );

    // Build the command with all arguments
    let mut cmd = gamdl_service::build_gamdl_command_public(app, urls, options)?;

    // Verbose: log CLI args for debugging
    emit_verbose_download_log(
        app,
        download_id,
        &format!("GAMDL CLI args: {:?}", options.to_cli_args()),
    );

    // Configure piped stdout/stderr for real-time parsing
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Reap the child automatically if the supervising task is aborted
    // (e.g., app shutdown, 10-min completion timeout in process_queue).
    // Prevents zombie GAMDL processes that would otherwise keep running
    // and writing log lines long after the queue item is "done" (#508).
    cmd.kill_on_drop(true);

    // Spawn the GAMDL subprocess
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start GAMDL process: {e}"))?;

    // Idle-watchdog + post-processing shared state (#508).
    //
    // - `last_activity_ms` is bumped on every stdout/stderr line so the
    //   cancellation poll loop can compute idle time against the
    //   configured `gamdl_idle_timeout_minutes` and kill the child when
    //   exceeded.
    // - `post_processing_flag` flips to true once the parser reports a
    //   `ProcessingStep` or we see a `100% of` progress line. While
    //   set, the idle watchdog stands down (remux / decrypt is silent
    //   by design). The queue UI also picks up the flag via a
    //   processing-label update so the caption flips from
    //   `DOWNLOADING…` to `Post-processing (remux / decrypt)`.
    // - `soft_error_count` tallies `Finished with N error(s)` from
    //   GAMDL's stdout so an exit-0 with N>0 is downgraded to an
    //   error at status-check time instead of being silently swallowed
    //   (the same symptom the companion supervisor guards against
    //   in #500).
    let last_activity_ms = Arc::new(std::sync::atomic::AtomicU64::new(
        now_epoch_ms_primary(),
    ));
    let post_processing_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let soft_error_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    // Take stdout/stderr handles
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stderr".to_string())?;

    // Collect error messages from GAMDL's output for post-process error reporting.
    // These are shared between the stdout and stderr reader tasks via Arc<Mutex>.
    // After the process exits, the last collected error is used as the failure message,
    // which is more informative than just the exit code.
    let collected_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Collect ALL stderr lines (raw) for Python traceback extraction.
    // When GAMDL crashes with a Python traceback, the parsed error list may
    // only contain "Traceback (most recent call last):" because intermediate
    // lines and the final exception line may not match any error keyword.
    // The raw stderr buffer lets us extract the actual exception post-mortem.
    let raw_stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Deduplication set for Activity Log emissions.
    // GAMDL and its Python dependencies (yt-dlp, tqdm) may write the same
    // output to both stdout and stderr, causing each line to appear twice
    // in the Activity Log. This shared set tracks lines already emitted
    // so the second reader skips duplicates. The set is bounded by the
    // total GAMDL output size (typically a few hundred to a few thousand
    // lines per album) so memory usage is negligible.
    let seen_lines: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Spawn stdout reader
    let stdout_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        let seen = seen_lines.clone();
        let last_activity = last_activity_ms.clone();
        let post_proc = post_processing_flag.clone();
        let soft_errors = soft_error_count.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(raw_line)) = lines.next_line().await {
                // #508: bump the idle watchdog on every line and scan
                // for GAMDL's end-of-run summary. Parsing is cheap
                // (byte search) and runs once per stdout line.
                last_activity
                    .store(now_epoch_ms_primary(), std::sync::atomic::Ordering::Relaxed);
                if let Some(n) = process::parse_gamdl_error_count(&raw_line) {
                    if n > 0 {
                        soft_errors.store(n, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                // Split on \r to handle yt-dlp download progress updates.
                // yt-dlp uses \r (carriage return) for in-place terminal
                // updates, but AsyncBufReadExt::lines() only splits on \n.
                // Without this split, all \r-separated progress updates
                // concatenate into one massive line (~127KB for albums).
                let segments: Vec<&str> = raw_line.split('\r').collect();

                // For \r-separated progress lines (e.g., yt-dlp), only
                // emit the LAST non-empty segment to activity-log since
                // earlier segments are overwritten in a real terminal.
                // This reduces activity-log event volume by 5-10x during
                // downloads. All segments are still parsed for gamdl-output
                // progress tracking.
                //
                // When verbose logging is enabled, bypass coalescing and
                // emit ALL segments so users get complete progress detail
                // for debugging (speeds, ETAs, percentages at every step).
                let verbose = crate::utils::activity_log::is_verbose_logging();
                let last_segment_idx = segments
                    .iter()
                    .rposition(|s| !s.trim().is_empty());

                for (idx, segment) in segments.iter().enumerate() {
                    let segment = segment.trim();
                    if segment.is_empty() {
                        continue;
                    }

                    // Strip ANSI escape codes (e.g., \x1b[32m) that GAMDL
                    // outputs for terminal colouring but render as garbage
                    // in the Activity Log's HTML view.
                    let clean_line = process::strip_ansi_codes(segment);
                    log::debug!("[gamdl stdout] {clean_line}");

                    // Emit to activity-log: last \r segment only (normal),
                    // or ALL segments when verbose logging is enabled for
                    // full debugging detail.
                    if verbose || Some(idx) == last_segment_idx {
                        let should_emit = if verbose {
                            // Verbose: bypass dedup — emit every line for
                            // complete progress history
                            true
                        } else {
                            let mut set = seen.lock().await;
                            set.insert(clean_line.clone())
                        };
                        if should_emit {
                            let _ = app.emit(
                                "activity-log",
                                &ActivityLogEvent {
                                    download_id: download_id.clone(),
                                    stream: "stdout",
                                    line: clean_line.clone(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                },
                            );
                        }
                    }

                    let event = process::parse_gamdl_output(&clean_line);

                    // Emit a clear per-track separator when GAMDL starts
                    // downloading a new track. This makes it easy to identify
                    // which track's [download] progress lines belong to which
                    // song in the activity log. Includes artist and album context
                    // from the queue item metadata (populated by early API fetch).
                    if let process::GamdlOutputEvent::TrackInfo {
                        ref title,
                        track_number,
                        track_total,
                        ..
                    } = event
                    {
                        let track_label = match (track_number, track_total) {
                            (Some(n), Some(t)) => format!("[Track {n}/{t}]"),
                            (Some(n), None) => format!("[Track {n}]"),
                            _ => "[Track]".to_string(),
                        };
                        // Look up artist/album from the queue item for context
                        let (artist_ctx, album_ctx) = {
                            if let Ok(q) = queue.try_lock() {
                                let item = q.items.iter().find(|i| i.status.id == download_id);
                                (
                                    item.and_then(|i| i.status.artist_name.clone())
                                        .unwrap_or_default(),
                                    item.and_then(|i| i.status.album_name.clone())
                                        .unwrap_or_default(),
                                )
                            } else {
                                (String::new(), String::new())
                            }
                        };
                        // Build context: "Artist — Album — " prefix when available
                        let context = {
                            let mut parts = Vec::new();
                            if !artist_ctx.is_empty() {
                                parts.push(artist_ctx);
                            }
                            if !album_ctx.is_empty() {
                                parts.push(album_ctx);
                            }
                            if parts.is_empty() {
                                String::new()
                            } else {
                                format!("{} — ", parts.join(" — "))
                            }
                        };
                        let _ = app.emit(
                            "activity-log",
                            &ActivityLogEvent {
                                download_id: download_id.clone(),
                                stream: "internal",
                                line: format!(
                                    "──── {track_label} Downloading {context}\"{title}\" ────"
                                ),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            },
                        );
                    }

                    // Update the queue item's progress
                    {
                        let mut q = queue.lock().await;
                        q.update_item_progress(&download_id, &event);
                    }

                    // Collect errors for fallback decisions
                    if let process::GamdlOutputEvent::Error { ref message } = event {
                        let mut errs = errors.lock().await;
                        errs.push(message.clone());
                    }

                    // #508: detect the transition into the silent
                    // post-processing phase. The parser returns
                    // `ProcessingStep` for remux/tag/decrypt steps, and
                    // yt-dlp's final `100% of` progress line signals the
                    // HLS download is complete. Either path flips the
                    // shared flag; first flip also updates the queue's
                    // processing_label so the UI caption flips to
                    // `Post-processing (remux / decrypt)`.
                    let is_processing_transition = matches!(
                        event,
                        process::GamdlOutputEvent::ProcessingStep { .. }
                    ) || clean_line.contains("100% of");
                    if is_processing_transition
                        && !post_proc.swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        let mut q = queue.lock().await;
                        q.set_processing_label(
                            &download_id,
                            "Post-processing (remux / decrypt)",
                        );
                    }

                    // Emit parsed event to frontend
                    let progress = gamdl_service::GamdlProgress {
                        download_id: download_id.clone(),
                        event,
                    };
                    let _ = app.emit("gamdl-output", &progress);
                }
            }
        })
    };

    // Spawn stderr reader
    let stderr_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        let raw_stderr = raw_stderr_lines.clone();
        let seen = seen_lines.clone();
        let last_activity = last_activity_ms.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(raw_line)) = lines.next_line().await {
                last_activity
                    .store(now_epoch_ms_primary(), std::sync::atomic::Ordering::Relaxed);
                // Split on \r for yt-dlp progress updates (same as stdout)
                let segments: Vec<&str> = raw_line.split('\r').collect();

                // Same coalescing logic as stdout: emit only last \r segment
                // in normal mode, or ALL segments when verbose logging is on.
                let verbose = crate::utils::activity_log::is_verbose_logging();
                let last_segment_idx = segments
                    .iter()
                    .rposition(|s| !s.trim().is_empty());

                for (idx, segment) in segments.iter().enumerate() {
                    let segment = segment.trim();
                    if segment.is_empty() {
                        continue;
                    }

                    // Strip ANSI escape codes before display and parsing
                    let clean_line = process::strip_ansi_codes(segment);
                    log::debug!("[gamdl stderr] {clean_line}");

                    // Emit to activity-log: last \r segment only (normal),
                    // or ALL segments when verbose logging is enabled.
                    if verbose || Some(idx) == last_segment_idx {
                        let should_emit = if verbose {
                            true
                        } else {
                            let mut set = seen.lock().await;
                            set.insert(clean_line.clone())
                        };
                        if should_emit {
                            let _ = app.emit(
                                "activity-log",
                                &ActivityLogEvent {
                                    download_id: download_id.clone(),
                                    stream: "stderr",
                                    line: clean_line.clone(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                },
                            );
                        }
                    }

                    let event = process::parse_gamdl_output(&clean_line);

                    // Collect raw stderr lines for Python traceback extraction
                    // (always, regardless of dedup — needed for exception analysis)
                    {
                        let mut raw = raw_stderr.lock().await;
                        raw.push(clean_line.clone());
                    }

                    {
                        let mut q = queue.lock().await;
                        q.update_item_progress(&download_id, &event);
                    }

                    if let process::GamdlOutputEvent::Error { ref message } = event {
                        let mut errs = errors.lock().await;
                        errs.push(message.clone());
                    }

                    let progress = gamdl_service::GamdlProgress {
                        download_id: download_id.clone(),
                        event,
                    };
                    let _ = app.emit("gamdl-output", &progress);
                }
            }
        })
    };

    // Idle watchdog configuration (#508). Read once before the loop —
    // changing it mid-download has no effect until the next item.
    // `.max(1)` clamps a pathological 0 to one minute.
    let idle_limit_ms = u64::from(
        load_settings_for_queue(app)
            .gamdl_idle_timeout_minutes
            .max(1),
    ) * 60_000;

    // Cancellation polling loop: alternate between checking for user cancellation
    // and checking if the GAMDL process has exited naturally.
    // This loop runs every 250ms and provides responsive cancellation support
    // without consuming excessive CPU.
    let status = loop {
        // Step 1: Check if the user cancelled this download.
        // The cancel() method on the queue sets the item's state to Cancelled,
        // which we detect here. The lock is held very briefly (just a read check).
        {
            let q = queue.lock().await;
            if q.is_cancelled(download_id) {
                log::info!("Download {download_id} cancelled, killing process");
                // Kill the GAMDL process and wait for cleanup
                let _ = child.kill().await;
                let _ = child.wait().await;
                // Wait for reader tasks to finish draining any buffered output
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err("Download cancelled by user".to_string());
            }
        }

        // Step 1b (#508): idle watchdog. When the post-processing flag
        // isn't set, kill the child if no stdout/stderr line has arrived
        // for `gamdl_idle_timeout_minutes`. The flag pause is important
        // — remux / decrypt can run silently for minutes on a network
        // volume without being stuck.
        if !post_processing_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let elapsed = now_epoch_ms_primary().saturating_sub(
                last_activity_ms.load(std::sync::atomic::Ordering::Relaxed),
            );
            if elapsed >= idle_limit_ms {
                let mins = idle_limit_ms / 60_000;
                log::warn!(
                    "GAMDL idle for {mins} min on download {download_id} — terminating"
                );
                emit_download_log(
                    app,
                    download_id,
                    &format!(
                        "⚠ GAMDL was idle for {mins} min — terminated by watchdog"
                    ),
                );
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err(format!(
                    "GAMDL idle for {mins} min — terminated by watchdog"
                ));
            }
        }

        // Step 2: Check if the process has exited (non-blocking check).
        // try_wait() returns Ok(Some(status)) if the process has exited,
        // Ok(None) if it's still running, or Err on OS-level error.
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                // Process still running — sleep briefly before next poll iteration.
                // 250ms provides a good balance between responsiveness and CPU usage.
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            }
            Err(e) => return Err(format!("Failed to wait for GAMDL process: {e}")),
        }
    };

    // Wait for output reader tasks to finish
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Check the exit status and construct an appropriate error message.
    //
    // #508 soft-error gate: GAMDL exits 0 even when a per-track download
    // crashed internally (e.g., `AttributeError: 'NoneType' object has
    // no attribute 'audio_track'` when a codec isn't offered for the
    // track). The `Finished with N error(s)` summary line is the
    // authoritative failure signal in that case. When soft errors
    // occurred, translate the tracebacks we can recognise and return
    // an error instead of swallowing them.
    let soft_errors = soft_error_count.load(std::sync::atomic::Ordering::Relaxed);
    if status.success() && soft_errors > 0 {
        let raw_lines = raw_stderr_lines.lock().await;
        let combined = raw_lines.join("\n");
        let friendly = process::classify_gamdl_traceback(&combined)
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| {
                format!(
                    "GAMDL reported {soft_errors} per-track error(s) even though the process exited 0"
                )
            });
        return Err(friendly);
    }

    if status.success() {
        // Return any error-pattern lines as non-fatal warnings.
        // These are issues GAMDL logged during the run but didn't consider
        // fatal enough to exit with a non-zero code.
        let warnings = collected_errors.lock().await.clone();
        Ok(warnings)
    } else {
        // Use the last collected error message from GAMDL's output for a meaningful
        // error message. This is more informative than just "exited with code N".
        // The error message is also used by classify_error() to determine the
        // retry/fallback strategy (codec error vs network error vs unknown).
        let errors = collected_errors.lock().await;
        let raw_lines = raw_stderr_lines.lock().await;

        errors.last().map_or_else(
            || {
                // Fallback to exit code if no error messages were collected
                // (e.g., GAMDL crashed without printing an error)
                let code = status.code().unwrap_or(-1);
                Err(format!("GAMDL process exited with code {code}"))
            },
            |last_error| {
                // Always try to extract the actual Python exception from raw
                // stderr. The output parser captures lines independently, so:
                //   - The "Traceback" header may be the last captured error
                //   - A traceback FRAME (e.g., `File "...map_httpcore_exceptions"`)
                //     may be the last error if the "exception" keyword matched
                //     a function name in the stack frame
                //   - The actual exception line may not have been captured at
                //     all (process killed mid-traceback, buffering issue)
                //
                // extract_python_exception scans raw stderr for the last
                // non-indented line after a "Traceback" header, which is the
                // actual exception (e.g., "httpx.ConnectError: Connection refused").
                // This gives classify_error() the real error text.
                //
                // #508: first try the friendly classifier on the combined
                // raw stderr so `NoneType.audio_track` and similar
                // "codec not available" patterns surface as a single
                // actionable line instead of a Python traceback.
                let combined = raw_lines.join("\n");
                if let Some(friendly) = process::classify_gamdl_traceback(&combined) {
                    Err(friendly.to_string())
                } else if let Some(extracted) = extract_python_exception(&raw_lines) {
                    Err(extracted)
                } else {
                    Err(last_error.clone())
                }
            },
        )
    }
}

/// Loads the current app settings for use during queue processing decisions.
///
/// This is called during the error handling path of `process_queue()` to
/// access the fallback chain configuration. It uses `config_service::load_settings()`
/// rather than cached settings to ensure the latest user preferences are used
/// (the user might change settings while downloads are running).
///
/// Returns `AppSettings::default()` on load failure to avoid blocking queue processing.
fn load_settings_for_queue(app: &AppHandle) -> AppSettings {
    match config_service::load_settings(app) {
        Ok(settings) => settings,
        Err(e) => {
            log::warn!("Failed to load settings for fallback: {e}, using defaults");
            AppSettings::default()
        }
    }
}

/// Creates a redacted settings snapshot for inclusion in crash/error reports.
///
/// Captures the most diagnostically useful settings (codec, resolution,
/// companion mode, feature flags, download mode) as a flat `HashMap`.
/// **No sensitive data** is included: no paths, no credentials, no
/// wrapper URLs, no MusicKit keys, no cookie paths. Only safe-to-share
/// configuration values that help diagnose download failures.
///
/// Merged into the crash report `context` alongside error-specific fields
/// like `error_category`, `url`, and `gamdl_version`.
fn settings_snapshot_for_context(app: &AppHandle) -> std::collections::HashMap<String, String> {
    let s = load_settings_for_queue(app);
    let mut m = std::collections::HashMap::new();

    // Core download config
    m.insert(
        "setting.codec".to_string(),
        s.default_song_codec.to_cli_string().to_string(),
    );
    m.insert(
        "setting.video_resolution".to_string(),
        s.default_video_resolution.to_cli_string().to_string(),
    );
    let companion_str = serde_json::to_value(&s.companion_mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.companion_mode));
    m.insert("setting.companion_mode".to_string(), companion_str);
    let dl_mode = serde_json::to_value(&s.download_mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.download_mode));
    m.insert("setting.download_mode".to_string(), dl_mode);
    let storefront = if s.storefront.is_empty() {
        "auto".to_string()
    } else {
        s.storefront.clone()
    };
    m.insert("setting.storefront".to_string(), storefront);

    // Feature flags (booleans as "true"/"false")
    m.insert(
        "setting.enhanced_lrc".to_string(),
        s.enhanced_lrc.to_string(),
    );
    m.insert(
        "setting.advisory_suffixes".to_string(),
        s.content_advisory_in_filenames.to_string(),
    );
    m.insert(
        "setting.acoustid".to_string(),
        s.acoustid_enabled.to_string(),
    );
    m.insert(
        "setting.replaygain".to_string(),
        s.replaygain_enabled.to_string(),
    );
    m.insert(
        "setting.replaygain_album_gain".to_string(),
        s.replaygain_album_gain.to_string(),
    );
    m.insert(
        "setting.artist_promo_video".to_string(),
        s.artist_promo_video_enabled.to_string(),
    );
    m.insert(
        "setting.musicbrainz".to_string(),
        s.musicbrainz_lookup.to_string(),
    );
    m.insert(
        "setting.fallback_enabled".to_string(),
        s.fallback_enabled.to_string(),
    );
    m.insert(
        "setting.auto_start_queue".to_string(),
        s.auto_start_queue.to_string(),
    );

    // Auth status (redacted — presence only, no values)
    m.insert(
        "setting.use_wrapper".to_string(),
        s.use_wrapper.to_string(),
    );
    m.insert(
        "setting.cookies_set".to_string(),
        s.cookies_path.is_some().to_string(),
    );
    m.insert(
        "setting.musickit_configured".to_string(),
        (s.musickit_team_id.is_some() && s.musickit_key_id.is_some()).to_string(),
    );

    m
}

// ============================================================
// Queue persistence: save/load/clear (crash recovery)
// ============================================================

/// Saves the current queue state to disk for crash recovery.
///
/// Writes only non-terminal items (Queued/Downloading/Processing) to
/// `{app_data_dir}/queue.json` as a JSON array of `PersistedQueueItem`.
///
/// Uses a clone-then-release pattern: persistable items are cloned while
/// the Mutex lock is held, then the lock is released before performing
/// file I/O. This avoids holding the lock during potentially slow disk writes.
///
/// Called after every queue mutation (enqueue, cancel, retry, clear, fallback,
/// network retry, completion, error) to ensure the on-disk state is always
/// up-to-date for crash recovery.
/// Debounced queue persistence. Saves at most once per 500ms to reduce
/// I/O pressure for rapid sequential mutations (e.g., batch enqueue).
/// See: https://github.com/MWBMPartners/MeedyaDL/issues/233
pub async fn save_queue_to_disk(app: &AppHandle, queue: &QueueHandle) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_SAVE_MS: AtomicU64 = AtomicU64::new(0);
    const DEBOUNCE_MS: u64 = 500;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_SAVE_MS.load(Ordering::Relaxed);

    if now.saturating_sub(last) < DEBOUNCE_MS {
        // Schedule a delayed save to ensure the final state is persisted
        let app_clone = app.clone();
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(DEBOUNCE_MS)).await;
            save_queue_to_disk_inner(&app_clone, &queue_clone).await;
        });
        return;
    }
    LAST_SAVE_MS.store(now, Ordering::Relaxed);

    save_queue_to_disk_inner(app, queue).await;
}

/// Internal save implementation (not debounced).
async fn save_queue_to_disk_inner(app: &AppHandle, queue: &QueueHandle) {
    // Clone persistable items and get counts while holding the lock
    let (items, active, queued, completed) = {
        let q = queue.lock().await;
        let (_, active, queued, completed, _) = q.get_counts();
        (q.get_persistable_items(), active, queued, completed)
    };

    // Update the system tray tooltip with current queue status
    crate::update_tray_tooltip(app, active, queued, completed);

    // Write to disk after releasing the lock
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    match serde_json::to_string_pretty(&items) {
        Ok(json) => {
            // Atomic write: write to temp file then rename to prevent
            // corruption on crash/power loss. See: #230
            let temp_path = queue_path.with_extension("json.tmp");
            if let Err(e) = std::fs::write(&temp_path, json) {
                log::warn!("Failed to write temp queue file: {e}");
            } else if let Err(e) = std::fs::rename(&temp_path, &queue_path) {
                log::warn!("Failed to rename temp queue file: {e}");
            }
        }
        Err(e) => {
            log::warn!("Failed to serialize queue: {e}");
        }
    }
}

/// Loads persisted queue items from disk.
///
/// Returns an empty `Vec` on missing or invalid file (graceful degradation
/// for first run or file corruption). This is intentional: the queue should
/// start empty rather than crash if persistence data is unavailable.
#[must_use]
pub fn load_queue_from_disk(app: &AppHandle) -> Vec<PersistedQueueItem> {
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    std::fs::read_to_string(&queue_path).map_or_else(
        |_| vec![], // File doesn't exist (first run) — not an error
        |json| match serde_json::from_str::<Vec<PersistedQueueItem>>(&json) {
            Ok(items) => {
                if !items.is_empty() {
                    log::info!("Loaded {} persisted queue item(s) from disk", items.len());
                }
                items
            }
            Err(e) => {
                log::debug!("Failed to parse queue.json: {e}");
                vec![]
            }
        },
    )
}

/// Deletes the `queue.json` persistence file.
///
/// Called when the queue is intentionally cleared to avoid restoring
/// stale items on next startup.
pub fn clear_queue_file(app: &AppHandle) {
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    if let Err(e) = std::fs::remove_file(&queue_path) {
        // ENOENT (file not found) is expected when no queue has been persisted yet.
        // Any other error (permission denied, I/O error) means the stale file
        // will survive and be restored on next startup — worth logging.
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Failed to remove queue.json: {e}");
        }
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::{DownloadRequest, DownloadState};
    use crate::models::gamdl_options::{GamdlOptions, SongCodec};
    use crate::models::settings::AppSettings;
    use crate::utils::process::GamdlOutputEvent;

    // ----------------------------------------------------------
    // Test Helpers
    // ----------------------------------------------------------

    /// Creates default AppSettings suitable for test use.
    /// The returned settings have sensible defaults matching AppSettings::default(),
    /// which includes fallback_enabled=true and a full music_fallback_chain.
    fn test_settings() -> AppSettings {
        AppSettings::default()
    }

    /// Creates a minimal DownloadRequest with a single URL and no overrides.
    /// The URL is a placeholder Apple Music URL for test purposes only.
    fn test_request() -> DownloadRequest {
        DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test-song/123456789".to_string()],
            options: None,
        }
    }

    /// Creates a DownloadRequest with per-download codec override.
    fn test_request_with_codec_override(codec: SongCodec) -> DownloadRequest {
        DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/999".to_string()],
            options: Some(GamdlOptions {
                song_codec: Some(codec),
                ..Default::default()
            }),
        }
    }

    /// Helper: enqueues a single item and returns its download ID.
    fn enqueue_one(queue: &mut DownloadQueue) -> String {
        let settings = test_settings();
        queue.enqueue(test_request(), &settings)
    }

    /// Helper: enqueues N items and returns their download IDs.
    fn enqueue_n(queue: &mut DownloadQueue, n: usize) -> Vec<String> {
        let settings = test_settings();
        (0..n)
            .map(|_| queue.enqueue(test_request(), &settings))
            .collect()
    }

    // ==========================================================
    // 1. new() tests
    // ==========================================================

    /// Verifies that DownloadQueue::new() creates an empty queue with no items,
    /// zero active count, and default concurrency settings.
    #[test]
    fn new_creates_empty_queue() {
        let queue = DownloadQueue::new();
        assert!(queue.items.is_empty(), "New queue should have no items");
        assert_eq!(
            queue.active_count, 0,
            "New queue should have zero active count"
        );
        assert_eq!(
            queue.max_concurrent, 1,
            "Default max_concurrent should be 1"
        );
        assert_eq!(
            queue.max_network_retries, 3,
            "Default max_network_retries should be 3"
        );
    }

    /// Verifies that the Default trait implementation delegates to new().
    #[test]
    fn default_delegates_to_new() {
        let queue = DownloadQueue::default();
        assert!(queue.items.is_empty());
        assert_eq!(queue.active_count, 0);
        assert_eq!(queue.max_concurrent, 1);
    }

    // ==========================================================
    // 2. enqueue() tests
    // ==========================================================

    /// Verifies that enqueue() returns a unique download ID (UUID v4 format)
    /// and that successive calls produce different IDs.
    #[test]
    fn enqueue_returns_unique_ids() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();

        let id1 = queue.enqueue(test_request(), &settings);
        let id2 = queue.enqueue(test_request(), &settings);

        assert!(!id1.is_empty(), "Download ID should not be empty");
        assert!(!id2.is_empty(), "Download ID should not be empty");
        assert_ne!(id1, id2, "Each enqueue should produce a unique ID");
    }

    /// Verifies that an enqueued item starts in the Queued state and appears
    /// in the status list with correct initial fields.
    #[test]
    fn enqueue_sets_queued_state() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request();
        let expected_url = request.urls[0].clone();

        let id = queue.enqueue(request, &settings);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 1, "Queue should have exactly one item");
        let status = &statuses[0];
        assert_eq!(status.id, id);
        assert_eq!(status.state, DownloadState::Queued);
        assert_eq!(status.urls, vec![expected_url]);
        assert_eq!(status.progress, 0.0);
        assert!(status.current_track.is_none());
        assert!(status.error.is_none());
        assert!(status.output_path.is_none());
        assert!(!status.fallback_occurred);
    }

    /// Verifies that enqueue() merges global settings into the item's options.
    /// Since merge_options is private, we test it indirectly: the enqueued item's
    /// codec_used field should reflect the default settings codec (ALAC).
    #[test]
    fn enqueue_merges_default_settings() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();

        let _id = queue.enqueue(test_request(), &settings);
        let statuses = queue.get_status();

        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("alac"),
            "Default codec should be ALAC from settings"
        );
    }

    /// Verifies that per-download codec overrides take precedence over
    /// global settings when enqueuing. This indirectly tests merge_options().
    #[test]
    fn enqueue_applies_per_download_overrides() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::Aac);

        let _id = queue.enqueue(request, &settings);
        let statuses = queue.get_status();

        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("aac"),
            "Per-download override should replace default ALAC with AAC"
        );
    }

    /// Verifies that multiple items can be enqueued and they all appear in
    /// the status list in FIFO order.
    #[test]
    fn enqueue_preserves_fifo_order() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 3);
        assert_eq!(
            statuses[0].id, ids[0],
            "First enqueued should be first in status"
        );
        assert_eq!(statuses[1].id, ids[1]);
        assert_eq!(
            statuses[2].id, ids[2],
            "Last enqueued should be last in status"
        );
    }

    /// Verifies that enqueue sets the network_retries_left field to
    /// the queue's max_network_retries value (tested via try_network_retry).
    #[test]
    fn enqueue_sets_network_retries_from_queue_config() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Exhaust all 3 retries
        assert!(
            queue.try_network_retry(&id).is_some(),
            "Should succeed on retry 1 of 3"
        );
        // Need to set back to Error/Queued for next retry test, but try_network_retry
        // itself resets to Queued. We need to simulate re-error.
        // Actually try_network_retry sets to Queued. Let's set back to error.
        queue.set_error(&id, "network error");
        assert!(
            queue.try_network_retry(&id).is_some(),
            "Should succeed on retry 2 of 3"
        );
        queue.set_error(&id, "network error");
        assert!(
            queue.try_network_retry(&id).is_some(),
            "Should succeed on retry 3 of 3"
        );
        queue.set_error(&id, "network error");
        assert!(
            queue.try_network_retry(&id).is_none(),
            "Should fail after 3 retries exhausted"
        );
    }

    // ==========================================================
    // 3. get_status() tests
    // ==========================================================

    /// Verifies that get_status() returns an empty vector when the queue
    /// has no items.
    #[test]
    fn get_status_empty_queue() {
        let queue = DownloadQueue::new();
        let statuses = queue.get_status();
        assert!(
            statuses.is_empty(),
            "Empty queue should return empty status vec"
        );
    }

    /// Verifies that get_status() returns all items in the queue, each with
    /// the correct state and URL information.
    #[test]
    fn get_status_returns_all_items() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 4);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 4);
        for (i, status) in statuses.iter().enumerate() {
            assert_eq!(status.id, ids[i]);
            assert_eq!(status.state, DownloadState::Queued);
        }
    }

    /// Verifies that get_status() returns cloned data (modifications to the
    /// returned vec do not affect the queue).
    #[test]
    fn get_status_returns_cloned_data() {
        let mut queue = DownloadQueue::new();
        let _id = enqueue_one(&mut queue);

        let statuses1 = queue.get_status();
        let statuses2 = queue.get_status();

        assert_eq!(statuses1.len(), statuses2.len());
        assert_eq!(statuses1[0].id, statuses2[0].id);
    }

    // ==========================================================
    // 4. get_counts() tests
    // ==========================================================

    /// Verifies that get_counts() returns all zeros for an empty queue.
    /// Returns tuple: (total, active, queued, completed, failed).
    #[test]
    fn get_counts_empty_queue() {
        let queue = DownloadQueue::new();
        assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
    }

    /// Verifies that get_counts() correctly counts items in different states.
    /// Sets up items in Queued, Downloading, Complete, and Error states
    /// and checks that each counter is accurate.
    #[test]
    fn get_counts_various_states() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 5);

        // Leave ids[0] as Queued
        // Set ids[1] to Downloading via next_pending
        let _ = queue.next_pending(); // ids[0] becomes Downloading
                                      // Set ids[2] to Complete
        queue.set_complete(&ids[2]);
        // Set ids[3] to Error
        queue.set_error(&ids[3], "test error");
        // ids[4] stays Queued

        // ids[0]=Downloading, ids[1]=Queued, ids[2]=Complete, ids[3]=Error, ids[4]=Queued
        let (total, active, queued, completed, failed) = queue.get_counts();
        assert_eq!(total, 5, "Total should be 5");
        assert_eq!(active, 1, "One item is Downloading");
        assert_eq!(queued, 2, "Two items are Queued (ids[1] and ids[4])");
        assert_eq!(completed, 1, "One item is Complete");
        assert_eq!(failed, 1, "One item is Error");
    }

    /// Verifies that get_counts() counts Processing state items as active.
    #[test]
    fn get_counts_processing_counted_as_active() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 2);

        queue.update_item_state(&ids[0], DownloadState::Processing);

        let (total, active, queued, _completed, _failed) = queue.get_counts();
        assert_eq!(total, 2);
        assert_eq!(active, 1, "Processing items should count as active");
        assert_eq!(queued, 1);
    }

    // ==========================================================
    // 5. cancel() tests
    // ==========================================================

    /// Verifies that cancelling a Queued item sets its state to Cancelled
    /// and returns true.
    #[test]
    fn cancel_queued_item() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let result = queue.cancel(&id);
        assert!(result, "cancel() should return true for Queued items");

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Cancelled,
            "Cancelled queued item should be in Cancelled state"
        );
    }

    /// Verifies that cancelling a Downloading item sets its state to Cancelled
    /// and returns true. The active_count is NOT decremented here; that happens
    /// when the running task detects cancellation and calls on_task_finished().
    #[test]
    fn cancel_downloading_item() {
        let mut queue = DownloadQueue::new();
        let _id = enqueue_one(&mut queue);

        // Move to Downloading state via next_pending
        let (dl_id, _, _, _) = queue.next_pending().expect("Should have a pending item");
        assert_eq!(queue.active_count, 1);

        let result = queue.cancel(&dl_id);
        assert!(result, "cancel() should return true for Downloading items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Cancelled);
        // active_count is NOT decremented by cancel() -- that's the task's job
        assert_eq!(
            queue.active_count, 1,
            "active_count should not be decremented by cancel()"
        );
    }

    /// Verifies that cancelling a Processing item sets its state to Cancelled
    /// and returns true.
    #[test]
    fn cancel_processing_item() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 1);
        queue.update_item_state(&ids[0], DownloadState::Processing);

        let result = queue.cancel(&ids[0]);
        assert!(result, "cancel() should return true for Processing items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Cancelled);
    }

    /// Verifies that cancel() returns false for items already in a terminal
    /// state (Complete, Error, Cancelled).
    #[test]
    fn cancel_returns_false_for_terminal_states() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        queue.set_complete(&ids[0]);
        queue.set_error(&ids[1], "some error");
        queue.cancel(&ids[2]); // First cancel succeeds

        assert!(
            !queue.cancel(&ids[0]),
            "Should return false for Complete item"
        );
        assert!(!queue.cancel(&ids[1]), "Should return false for Error item");
        assert!(
            !queue.cancel(&ids[2]),
            "Should return false for already Cancelled item"
        );
    }

    /// Verifies that cancel() returns false for a non-existent download ID.
    #[test]
    fn cancel_returns_false_for_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);

        assert!(
            !queue.cancel("nonexistent-id-12345"),
            "Should return false for unknown ID"
        );
    }

    // ==========================================================
    // 6. clear_finished() tests
    // ==========================================================

    /// Verifies that clear_finished() removes items in terminal states
    /// (Complete, Error, Cancelled) and keeps items in active/pending states.
    #[test]
    fn clear_finished_removes_terminal_keeps_active() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 5);

        // ids[0] = Queued (keep)
        // ids[1] = Downloading (keep)
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        // ids[2] = Complete (remove)
        queue.set_complete(&ids[2]);
        // ids[3] = Error (keep — errored items persist for review)
        queue.set_error(&ids[3], "error msg");
        // ids[4] = Cancelled (remove)
        queue.cancel(&ids[4]);

        let removed = queue.clear_finished();

        assert_eq!(removed, 2, "Should remove 2 items (Complete + Cancelled)");
        let statuses = queue.get_status();
        assert_eq!(statuses.len(), 3, "Should have 3 remaining items");
        assert_eq!(statuses[0].id, ids[0], "Queued item should remain");
        assert_eq!(statuses[1].id, ids[1], "Downloading item should remain");
        assert_eq!(statuses[2].id, ids[3], "Errored item should remain");
    }

    /// Verifies that clear_finished() returns 0 when there are no terminal items.
    #[test]
    fn clear_finished_returns_zero_when_nothing_to_clear() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_n(&mut queue, 3);

        let removed = queue.clear_finished();
        assert_eq!(
            removed, 0,
            "Nothing should be removed when all items are Queued"
        );
        assert_eq!(queue.get_status().len(), 3, "All items should remain");
    }

    /// Verifies that clear_finished() works correctly on an empty queue.
    #[test]
    fn clear_finished_on_empty_queue() {
        let mut queue = DownloadQueue::new();
        let removed = queue.clear_finished();
        assert_eq!(removed, 0, "Should return 0 for empty queue");
    }

    // ==========================================================
    // 7. next_pending() tests
    // ==========================================================

    /// Verifies that next_pending() returns None for an empty queue.
    #[test]
    fn next_pending_empty_queue() {
        let mut queue = DownloadQueue::new();
        assert!(
            queue.next_pending().is_none(),
            "Empty queue should return None"
        );
    }

    /// Verifies that next_pending() returns the first Queued item, transitions
    /// it to Downloading, and increments active_count.
    #[test]
    fn next_pending_returns_first_queued_item() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        let result = queue.next_pending();
        assert!(result.is_some(), "Should return Some for non-empty queue");

        let (dl_id, urls, _options, _service) = result.unwrap();
        assert_eq!(dl_id, ids[0], "Should return the first queued item");
        assert_eq!(urls.len(), 1, "Should include the URLs from the request");
        assert_eq!(
            queue.active_count, 1,
            "active_count should be incremented to 1"
        );

        // Verify the item's state changed to Downloading
        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Downloading);
        assert_eq!(
            statuses[1].state,
            DownloadState::Queued,
            "Other items should remain Queued"
        );
    }

    /// Verifies that next_pending() returns None when max_concurrent is reached.
    /// Default max_concurrent is 1, so after one next_pending(), the next call
    /// should return None even if there are queued items.
    #[test]
    fn next_pending_respects_max_concurrent() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_n(&mut queue, 3);

        // First call succeeds (active_count goes from 0 to 1)
        let first = queue.next_pending();
        assert!(first.is_some(), "First next_pending should succeed");

        // Second call should return None (active_count == max_concurrent == 1)
        let second = queue.next_pending();
        assert!(
            second.is_none(),
            "Second next_pending should return None when at max_concurrent"
        );
    }

    /// Verifies that next_pending() skips non-Queued items and finds the first
    /// Queued item in the deque.
    #[test]
    fn next_pending_skips_non_queued_items() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        // Set first item to Complete (not Queued)
        queue.set_complete(&ids[0]);
        // Set second item to Error
        queue.set_error(&ids[1], "error");

        // next_pending should skip ids[0] and ids[1], returning ids[2]
        let result = queue.next_pending();
        assert!(result.is_some());
        let (dl_id, _, _, _) = result.unwrap();
        assert_eq!(
            dl_id, ids[2],
            "Should return the first Queued item, skipping terminal items"
        );
    }

    /// Verifies that next_pending() returns the merged GamdlOptions for the item.
    #[test]
    fn next_pending_returns_merged_options() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::AacHe);
        let _id = queue.enqueue(request, &settings);

        let (_, _, options, _) = queue.next_pending().expect("Should return pending item");
        assert_eq!(
            options.song_codec,
            Some(SongCodec::AacHe),
            "Returned options should reflect the per-download override"
        );
    }

    // ==========================================================
    // 8. on_task_finished() tests
    // ==========================================================

    /// Verifies that on_task_finished() decrements active_count.
    #[test]
    fn on_task_finished_decrements_active_count() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);
        let _ = queue.next_pending(); // active_count = 1

        assert_eq!(queue.active_count, 1);
        queue.on_task_finished();
        assert_eq!(
            queue.active_count, 0,
            "active_count should be 0 after on_task_finished"
        );
    }

    /// Verifies that on_task_finished() does not underflow below zero.
    /// Calling it when active_count is already 0 should be a no-op.
    #[test]
    fn on_task_finished_does_not_underflow() {
        let mut queue = DownloadQueue::new();
        assert_eq!(queue.active_count, 0);

        queue.on_task_finished();
        assert_eq!(queue.active_count, 0, "active_count should not go below 0");

        // Call it multiple times to be sure
        queue.on_task_finished();
        queue.on_task_finished();
        assert_eq!(queue.active_count, 0);
    }

    /// Verifies that after on_task_finished(), the queue can start new items
    /// since a concurrent slot has been freed.
    #[test]
    fn on_task_finished_frees_slot_for_next_pending() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 2);

        // Start first item
        let first = queue.next_pending();
        assert!(first.is_some());
        // Verify second item can't start yet
        assert!(
            queue.next_pending().is_none(),
            "Should be at max_concurrent"
        );

        // Finish first task
        queue.on_task_finished();

        // Now second item should be startable
        let second = queue.next_pending();
        assert!(
            second.is_some(),
            "Should be able to start next item after finishing"
        );
        let (dl_id, _, _, _) = second.unwrap();
        assert_eq!(dl_id, ids[1]);
    }

    // ==========================================================
    // 9. is_cancelled() tests
    // ==========================================================

    /// Verifies that is_cancelled() returns true for cancelled items.
    #[test]
    fn is_cancelled_returns_true_for_cancelled() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);
        queue.cancel(&id);

        assert!(
            queue.is_cancelled(&id),
            "Should return true for cancelled item"
        );
    }

    /// Verifies that is_cancelled() returns false for non-cancelled items
    /// in various states (Queued, Downloading, Complete, Error).
    #[test]
    fn is_cancelled_returns_false_for_non_cancelled() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 4);

        // ids[0] = Queued
        assert!(
            !queue.is_cancelled(&ids[0]),
            "Queued item should not be cancelled"
        );

        // ids[1] = Downloading (via update_item_state)
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        assert!(
            !queue.is_cancelled(&ids[1]),
            "Downloading item should not be cancelled"
        );

        // ids[2] = Complete
        queue.set_complete(&ids[2]);
        assert!(
            !queue.is_cancelled(&ids[2]),
            "Complete item should not be cancelled"
        );

        // ids[3] = Error
        queue.set_error(&ids[3], "error");
        assert!(
            !queue.is_cancelled(&ids[3]),
            "Error item should not be cancelled"
        );
    }

    /// Verifies that is_cancelled() returns false for a non-existent download ID.
    #[test]
    fn is_cancelled_returns_false_for_nonexistent_id() {
        let queue = DownloadQueue::new();
        assert!(
            !queue.is_cancelled("does-not-exist"),
            "Should return false for unknown ID"
        );
    }

    // ==========================================================
    // 10. update_item_progress() tests
    // ==========================================================

    /// Verifies that a DownloadProgress event updates the item's progress,
    /// speed, eta, and sets state to Downloading.
    #[test]
    fn update_item_progress_download_progress() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::DownloadProgress {
            percent: 45.5,
            speed: "2.5MiB/s".to_string(),
            eta: "00:30".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        let s = &statuses[0];
        assert!((s.progress - 45.5).abs() < 0.001, "Progress should be 45.5");
        assert_eq!(s.speed.as_deref(), Some("2.5MiB/s"));
        assert_eq!(s.eta.as_deref(), Some("00:30"));
        assert_eq!(s.state, DownloadState::Downloading);
    }

    /// Verifies that a TrackInfo event updates the current_track field
    /// with the formatted "Artist - Title" string.
    #[test]
    fn update_item_progress_track_info_with_artist() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::TrackInfo {
            title: "Anti-Hero".to_string(),
            artist: "Taylor Swift".to_string(),
            album: String::new(),
            track_number: None,
            track_total: None,
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].current_track.as_deref(),
            Some("Taylor Swift - Anti-Hero"),
            "Should format as 'Artist - Title'"
        );
    }

    /// Verifies that a TrackInfo event with an empty artist just uses the title.
    #[test]
    fn update_item_progress_track_info_without_artist() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::TrackInfo {
            title: "Bohemian Rhapsody".to_string(),
            artist: String::new(),
            album: String::new(),
            track_number: None,
            track_total: None,
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].current_track.as_deref(),
            Some("Bohemian Rhapsody"),
            "Should use title only when artist is empty"
        );
    }

    /// Verifies that a ProcessingStep event sets the state to Processing.
    #[test]
    fn update_item_progress_processing_step() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::ProcessingStep {
            step: "Remuxing to M4A".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Processing,
            "ProcessingStep event should set state to Processing"
        );
    }

    /// Verifies that a Complete event sets the output_path and progress to 100%.
    #[test]
    fn update_item_progress_complete() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Complete {
            path: "/output/song.m4a".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].output_path.as_deref(),
            Some("/output/song.m4a"),
            "Complete event should set output_path"
        );
        assert!(
            (statuses[0].progress - 100.0).abs() < 0.001,
            "Complete event should set progress to 100%"
        );
    }

    /// Verifies that an Error event sets the error field on the item.
    #[test]
    fn update_item_progress_error() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Error {
            message: "Codec not available".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].error.as_deref(),
            Some("Codec not available"),
            "Error event should set the error field"
        );
        // Note: Error event does NOT change state -- that's handled by set_error()
        // after the process exits and retry/fallback logic is evaluated.
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Error event should NOT change state (that's set_error's job)"
        );
    }

    /// Verifies that an Unknown event does not change any item fields.
    #[test]
    fn update_item_progress_unknown_event_is_no_op() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Unknown {
            raw: "some random output".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Unknown event should not change state"
        );
        assert_eq!(
            statuses[0].progress, 0.0,
            "Unknown event should not change progress"
        );
    }

    /// Verifies that update_item_progress is a no-op for non-existent IDs
    /// (does not panic).
    #[test]
    fn update_item_progress_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::DownloadProgress {
            percent: 50.0,
            speed: "1MiB/s".to_string(),
            eta: "00:10".to_string(),
        };
        // Should not panic
        queue.update_item_progress("nonexistent-id", &event);

        // Original item should be unchanged
        let statuses = queue.get_status();
        assert_eq!(statuses[0].progress, 0.0);
    }

    // ==========================================================
    // 11. set_error() and set_complete() tests
    // ==========================================================

    /// Verifies that set_error() sets the state to Error and records the
    /// error message.
    #[test]
    fn set_error_sets_state_and_message() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.set_error(&id, "Network timeout occurred");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Error);
        assert_eq!(
            statuses[0].error.as_deref(),
            Some("Network timeout occurred")
        );
    }

    /// Verifies that set_error() is a no-op for non-existent IDs.
    #[test]
    fn set_error_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.set_error("nonexistent", "some error");
    }

    /// Verifies that set_complete() sets the state to Complete and progress
    /// to 100%.
    #[test]
    fn set_complete_sets_state_and_progress() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.set_complete(&id);

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Complete);
        assert!(
            (statuses[0].progress - 100.0).abs() < 0.001,
            "set_complete should set progress to 100%"
        );
    }

    /// Verifies that set_complete() is a no-op for non-existent IDs.
    #[test]
    fn set_complete_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.set_complete("nonexistent");
    }

    // ==========================================================
    // 12. try_network_retry() tests
    // ==========================================================

    /// Verifies that try_network_retry() resets the item to Queued state when
    /// retries remain, returns attempt number and total, and decrements the
    /// retry counter.
    #[test]
    fn try_network_retry_resets_to_queued_when_retries_remain() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Simulate an error state
        queue.set_error(&id, "Network timeout");

        let result = queue.try_network_retry(&id);
        // max_network_retries = 3, so total = 4. First retry = attempt 2.
        assert_eq!(result, Some((2, 4)), "First retry should be attempt 2 of 4");

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Should reset to Queued for retry"
        );
        assert!(
            statuses[0].error.is_none(),
            "Error should be cleared on retry"
        );
        assert_eq!(statuses[0].progress, 0.0, "Progress should reset to 0");
    }

    /// Verifies that try_network_retry() returns None when all retries have
    /// been exhausted (default: 3 retries), and that attempt numbers increment.
    #[test]
    fn try_network_retry_returns_false_when_exhausted() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Use up all 3 retries and verify attempt numbers
        queue.set_error(&id, "network error");
        assert_eq!(queue.try_network_retry(&id), Some((2, 4)));
        queue.set_error(&id, "network error");
        assert_eq!(queue.try_network_retry(&id), Some((3, 4)));
        queue.set_error(&id, "network error");
        assert_eq!(queue.try_network_retry(&id), Some((4, 4)));

        // 4th attempt should fail
        queue.set_error(&id, "network error");
        let result = queue.try_network_retry(&id);
        assert!(
            result.is_none(),
            "Should return None after all retries exhausted"
        );
    }

    /// Verifies that try_network_retry() returns None for a non-existent ID.
    #[test]
    fn try_network_retry_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        assert!(queue.try_network_retry("nonexistent").is_none());
    }

    // ==========================================================
    // 13. try_fallback() tests
    // ==========================================================

    /// Verifies that try_fallback() returns new options with the next codec in
    /// the fallback chain, resets the item to Queued, and marks fallback_occurred.
    #[test]
    fn try_fallback_returns_next_codec_in_chain() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Simulate an error requiring fallback
        queue.set_error(&id, "Codec not available");

        // First fallback: chain[0] = Alac (initial), so next = chain[1] = Atmos
        let result = queue.try_fallback(&id, &settings);
        assert!(result.is_some(), "First fallback should succeed");

        let (new_opts, fb_idx, chain_len) = result.unwrap();
        assert_eq!(fb_idx, 1, "First fallback should be index 1");
        assert_eq!(chain_len, 6, "Default chain has 6 codecs");
        // song_codec should be None — GAMDL >= 2.9.1 removed --song-codec.
        // We use --song-codec-priority with a single codec instead.
        assert_eq!(
            new_opts.song_codec, None,
            "song_codec should be None (--song-codec removed in GAMDL 2.9.1)"
        );
        // song_codec_priority should be set to just the single fallback codec
        // to prevent process_download_item() from rebuilding the full chain
        // and to override config.ini's full priority chain.
        assert_eq!(
            new_opts.song_codec_priority,
            Some("atmos".to_string()),
            "song_codec_priority should be overridden to single fallback codec"
        );

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Should reset to Queued"
        );
        assert!(
            statuses[0].fallback_occurred,
            "Should mark fallback_occurred as true"
        );
        assert_eq!(statuses[0].codec_used.as_deref(), Some("atmos"));
        assert!(
            statuses[0].error.is_none(),
            "Error should be cleared on fallback"
        );
        assert_eq!(statuses[0].progress, 0.0, "Progress should be reset");
    }

    /// Verifies that try_fallback() returns None when all codecs in the
    /// fallback chain have been exhausted.
    #[test]
    fn try_fallback_returns_none_when_chain_exhausted() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        // Use a short chain for testing
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac];
        let id = queue.enqueue(test_request(), &settings);

        // First fallback: Alac (0) -> Aac (1)
        queue.set_error(&id, "codec error");
        let result1 = queue.try_fallback(&id, &settings);
        assert!(result1.is_some(), "First fallback to Aac should succeed");
        let (_, idx, len) = result1.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(len, 2);

        // Second fallback: chain exhausted (index 2 >= chain.len() of 2)
        queue.set_error(&id, "codec error again");
        let result2 = queue.try_fallback(&id, &settings);
        assert!(result2.is_none(), "Should return None when chain exhausted");
    }

    /// Verifies that try_fallback() returns None when fallback is disabled
    /// in settings, regardless of chain contents.
    #[test]
    fn try_fallback_returns_none_when_disabled() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.fallback_enabled = false;
        let id = queue.enqueue(test_request(), &settings);

        queue.set_error(&id, "codec error");
        let result = queue.try_fallback(&id, &settings);
        assert!(
            result.is_none(),
            "Should return None when fallback_enabled is false"
        );
    }

    /// Verifies that try_fallback() returns None for a non-existent ID.
    #[test]
    fn try_fallback_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let result = queue.try_fallback("nonexistent", &settings);
        assert!(result.is_none());
    }

    /// Verifies that multiple successive fallbacks advance through the entire
    /// fallback chain correctly.
    #[test]
    fn try_fallback_advances_through_full_chain() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Atmos, SongCodec::Aac];
        let id = queue.enqueue(test_request(), &settings);

        // Fallback 1: Alac -> Atmos (index 1 of 3-element chain)
        queue.set_error(&id, "codec error");
        let r1 = queue.try_fallback(&id, &settings);
        let (r1_opts, r1_idx, r1_len) = r1.unwrap();
        assert_eq!(
            r1_opts.song_codec, None,
            "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
        );
        assert_eq!(r1_idx, 1, "First fallback is index 1");
        assert_eq!(r1_len, 3, "Chain length is 3");
        assert_eq!(
            r1_opts.song_codec_priority,
            Some("atmos".to_string()),
            "Priority should be single codec on fallback"
        );

        // Fallback 2: Atmos -> Aac (index 2 of 3-element chain)
        queue.set_error(&id, "codec error");
        let r2 = queue.try_fallback(&id, &settings);
        let (r2_opts, r2_idx, _) = r2.unwrap();
        assert_eq!(
            r2_opts.song_codec, None,
            "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
        );
        assert_eq!(r2_idx, 2, "Second fallback is index 2");
        assert_eq!(
            r2_opts.song_codec_priority,
            Some("aac".to_string()),
            "Priority should be single codec on fallback"
        );

        // Fallback 3: exhausted
        queue.set_error(&id, "codec error");
        let r3 = queue.try_fallback(&id, &settings);
        assert!(r3.is_none(), "Chain should be exhausted after 3 codecs");
    }

    // ==========================================================
    // 14. retry() tests
    // ==========================================================

    /// Verifies that retry() resets an errored item fully to Queued state
    /// with fresh options, reset counters, and cleared error/progress.
    #[test]
    fn retry_resets_errored_item_to_queued() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        queue.set_error(&id, "Download failed");

        let result = queue.retry(&id, &settings);
        assert!(result, "retry() should return true for Error items");

        let statuses = queue.get_status();
        let s = &statuses[0];
        assert_eq!(s.state, DownloadState::Queued, "Should be reset to Queued");
        assert!(s.error.is_none(), "Error should be cleared");
        assert_eq!(s.progress, 0.0, "Progress should be reset");
        assert!(!s.fallback_occurred, "fallback_occurred should be reset");
        assert_eq!(
            s.codec_used.as_deref(),
            Some("alac"),
            "Codec should be re-merged from settings"
        );
    }

    /// Verifies that retry() resets a cancelled item to Queued state.
    #[test]
    fn retry_resets_cancelled_item() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        queue.cancel(&id);
        assert_eq!(queue.get_status()[0].state, DownloadState::Cancelled);

        let result = queue.retry(&id, &settings);
        assert!(result, "retry() should return true for Cancelled items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Queued);
    }

    /// Verifies that retry() returns false for non-terminal states (Queued,
    /// Downloading, Processing, Complete).
    #[test]
    fn retry_returns_false_for_non_terminal_states() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let ids = enqueue_n(&mut queue, 4);

        // ids[0] = Queued
        assert!(
            !queue.retry(&ids[0], &settings),
            "Should not retry Queued item"
        );

        // ids[1] = Downloading
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        assert!(
            !queue.retry(&ids[1], &settings),
            "Should not retry Downloading item"
        );

        // ids[2] = Processing
        queue.update_item_state(&ids[2], DownloadState::Processing);
        assert!(
            !queue.retry(&ids[2], &settings),
            "Should not retry Processing item"
        );

        // ids[3] = Complete
        queue.set_complete(&ids[3]);
        assert!(
            !queue.retry(&ids[3], &settings),
            "Should not retry Complete item"
        );
    }

    /// Verifies that retry() returns false for a non-existent download ID.
    #[test]
    fn retry_returns_false_for_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        assert!(!queue.retry("nonexistent", &settings));
    }

    /// Verifies that retry() re-merges options from the original request
    /// with the current settings. This means if settings changed between the
    /// original enqueue and the retry, the retry picks up the new settings.
    #[test]
    fn retry_remerges_options_from_original_request() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::AacHe);
        let id = queue.enqueue(request, &settings);

        queue.set_error(&id, "error");

        // Retry with the same settings
        let result = queue.retry(&id, &settings);
        assert!(result);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("aac-he"),
            "Retry should preserve the original per-download override"
        );
    }

    /// Verifies that retry() resets the network retries counter and fallback
    /// index, which can be verified by subsequent try_network_retry calls
    /// succeeding again after a retry.
    #[test]
    fn retry_resets_retry_counters() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Exhaust network retries
        for _ in 0..3 {
            queue.set_error(&id, "network");
            queue.try_network_retry(&id);
        }
        queue.set_error(&id, "network");
        assert!(
            queue.try_network_retry(&id).is_none(),
            "Retries should be exhausted"
        );

        // Now use retry() to do a full reset
        queue.set_error(&id, "network");
        queue.retry(&id, &settings);

        // Network retries should be available again
        queue.set_error(&id, "network");
        assert!(
            queue.try_network_retry(&id).is_some(),
            "After retry(), network retries should be reset to max"
        );
    }

    // ==========================================================
    // update_item_state() tests
    // ==========================================================

    /// Verifies that update_item_state() changes the state of the item.
    #[test]
    fn update_item_state_changes_state() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.update_item_state(&id, DownloadState::Downloading);
        assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);

        queue.update_item_state(&id, DownloadState::Processing);
        assert_eq!(queue.get_status()[0].state, DownloadState::Processing);
    }

    /// Verifies that update_item_state() is a no-op for non-existent IDs.
    #[test]
    fn update_item_state_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.update_item_state("nonexistent", DownloadState::Downloading);
    }

    // ==========================================================
    // new_queue_handle() test
    // ==========================================================

    /// Verifies that new_queue_handle() creates an Arc<Mutex<DownloadQueue>>
    /// that can be locked and used.
    #[tokio::test]
    async fn new_queue_handle_creates_usable_handle() {
        let handle = new_queue_handle();
        let queue = handle.lock().await;
        assert!(
            queue.items.is_empty(),
            "New queue handle should wrap an empty queue"
        );
        assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
    }

    // ==========================================================
    // Integration / workflow tests
    // ==========================================================

    /// Simulates a full download lifecycle: enqueue -> next_pending (Downloading)
    /// -> progress updates -> set_complete -> on_task_finished. Verifies state
    /// transitions at each step.
    #[test]
    fn full_lifecycle_happy_path() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Step 1: Item is Queued
        assert_eq!(queue.get_status()[0].state, DownloadState::Queued);

        // Step 2: Start downloading
        let (dl_id, _, _, _) = queue.next_pending().unwrap();
        assert_eq!(dl_id, id);
        assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);
        assert_eq!(queue.active_count, 1);

        // Step 3: Progress updates
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::TrackInfo {
                title: "Test Song".to_string(),
                artist: "Test Artist".to_string(),
                album: String::new(),
                track_number: None,
                track_total: None,
            },
        );
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::DownloadProgress {
                percent: 50.0,
                speed: "5MiB/s".to_string(),
                eta: "00:10".to_string(),
            },
        );
        assert!((queue.get_status()[0].progress - 50.0).abs() < 0.001);
        assert_eq!(
            queue.get_status()[0].current_track.as_deref(),
            Some("Test Artist - Test Song")
        );

        // Step 4: Processing
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::ProcessingStep {
                step: "Remuxing to M4A".to_string(),
            },
        );
        assert_eq!(queue.get_status()[0].state, DownloadState::Processing);

        // Step 5: Complete
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::Complete {
                path: "/output/test.m4a".to_string(),
            },
        );
        queue.set_complete(&id);
        queue.on_task_finished();

        let final_status = &queue.get_status()[0];
        assert_eq!(final_status.state, DownloadState::Complete);
        assert!((final_status.progress - 100.0).abs() < 0.001);
        assert_eq!(
            final_status.output_path.as_deref(),
            Some("/output/test.m4a")
        );
        assert_eq!(queue.active_count, 0);
    }

    /// Simulates a download that fails with a codec error and successfully
    /// falls back to the next codec in the chain.
    #[test]
    fn lifecycle_with_codec_fallback() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac, SongCodec::AacLegacy];
        let id = queue.enqueue(test_request(), &settings);

        // Start and fail with codec error
        let _ = queue.next_pending();
        queue.set_error(&id, "Codec not available for ALAC");
        queue.on_task_finished();

        // Fallback to AAC (index 1 of 3-element chain)
        let fallback = queue.try_fallback(&id, &settings);
        assert!(fallback.is_some());
        let (fb_opts, fb_idx, fb_len) = fallback.unwrap();
        assert_eq!(
            fb_opts.song_codec, None,
            "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
        );
        assert_eq!(fb_opts.song_codec_priority, Some("aac".to_string()));
        assert_eq!(fb_idx, 1);
        assert_eq!(fb_len, 3);

        // Item should be re-queued
        assert_eq!(queue.get_status()[0].state, DownloadState::Queued);
        assert!(queue.get_status()[0].fallback_occurred);
    }

    /// Simulates a download that fails with a network error and retries
    /// successfully.
    #[test]
    fn lifecycle_with_network_retry() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Start and fail with network error
        let _ = queue.next_pending();
        queue.set_error(&id, "Network timeout");
        queue.on_task_finished();

        // Network retry should succeed (attempt 2 of 4)
        let retried = queue.try_network_retry(&id);
        assert!(retried.is_some());

        // Item should be re-queued
        let s = &queue.get_status()[0];
        assert_eq!(s.state, DownloadState::Queued);
        assert!(s.error.is_none());
        assert_eq!(s.progress, 0.0);
    }

    /// Verifies that multiple items can cycle through the queue when only
    /// one concurrent slot is available. The second item should start only
    /// after the first completes.
    #[test]
    fn sequential_processing_with_single_concurrency() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        // Start first item
        let (id1, _, _, _) = queue.next_pending().unwrap();
        assert_eq!(id1, ids[0]);

        // Can't start second while first is running
        assert!(queue.next_pending().is_none());

        // Finish first
        queue.set_complete(&ids[0]);
        queue.on_task_finished();

        // Now second can start
        let (id2, _, _, _) = queue.next_pending().unwrap();
        assert_eq!(id2, ids[1]);

        // Finish second
        queue.set_complete(&ids[1]);
        queue.on_task_finished();

        // Third can start
        let (id3, _, _, _) = queue.next_pending().unwrap();
        assert_eq!(id3, ids[2]);
    }

    // ----------------------------------------------------------
    // Python Traceback Extraction Tests
    // ----------------------------------------------------------

    #[test]
    fn extracts_type_error_from_traceback() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"foo.py\", line 42, in bar".to_string(),
            "    some_call()".to_string(),
            "TypeError: 'NoneType' object has no attribute 'x'".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("TypeError: 'NoneType' object has no attribute 'x'".to_string())
        );
    }

    #[test]
    fn extracts_dotted_exception_from_traceback() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"api.py\", line 10".to_string(),
            "    response.raise_for_status()".to_string(),
            "requests.exceptions.HTTPError: 403 Client Error".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("requests.exceptions.HTTPError: 403 Client Error".to_string())
        );
    }

    #[test]
    fn returns_none_without_traceback() {
        let lines = vec!["Normal output line".to_string(), "Another line".to_string()];
        assert_eq!(extract_python_exception(&lines), None);
    }

    #[test]
    fn handles_multiple_tracebacks_uses_last() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"a.py\", line 1".to_string(),
            "RuntimeError: first".to_string(),
            "".to_string(),
            "Traceback (most recent call last):".to_string(),
            "  File \"b.py\", line 2".to_string(),
            "ValueError: second".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("ValueError: second".to_string())
        );
    }

    #[test]
    fn handles_traceback_with_no_exception_after() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"foo.py\", line 1".to_string(),
            "  File \"bar.py\", line 2".to_string(),
        ];
        // All lines after Traceback are indented, so no exception found
        assert_eq!(extract_python_exception(&lines), None);
    }

    // ----------------------------------------------------------
    // Gap-fill helper tests
    // ----------------------------------------------------------

    #[test]
    fn build_gapfill_filters_experimental_codecs() {
        let chain = "atmos,ac3,alac,aac-binaural,aac,aac-legacy";
        let result = build_gapfill_priority_chain(chain);
        assert_eq!(result, Some("alac,aac-binaural,aac,aac-legacy".to_string()));
    }

    #[test]
    fn build_gapfill_preserves_non_experimental() {
        let chain = "alac,aac,aac-legacy";
        let result = build_gapfill_priority_chain(chain);
        assert_eq!(result, Some("alac,aac,aac-legacy".to_string()));
    }

    #[test]
    fn build_gapfill_returns_none_when_all_experimental() {
        let chain = "atmos,ac3";
        let result = build_gapfill_priority_chain(chain);
        assert_eq!(result, None);
    }

    #[test]
    fn build_gapfill_single_non_experimental() {
        let chain = "atmos,alac";
        let result = build_gapfill_priority_chain(chain);
        assert_eq!(result, Some("alac".to_string()));
    }

    #[test]
    fn count_codec_skip_warnings_counts_correctly() {
        let warnings = vec![
            "Requested format is not available for song 01".to_string(),
            "Requested format is not available for song 02".to_string(),
            "Some other warning".to_string(),
            "Requested format is not available for song 03".to_string(),
        ];
        assert_eq!(count_codec_skip_warnings(&warnings), 3);
    }

    #[test]
    fn count_codec_skip_warnings_zero_when_no_skips() {
        let warnings = vec![
            "Network timeout occurred".to_string(),
            "Some other warning".to_string(),
        ];
        assert_eq!(count_codec_skip_warnings(&warnings), 0);
    }

    #[test]
    fn count_codec_skip_warnings_empty() {
        let warnings: Vec<String> = vec![];
        assert_eq!(count_codec_skip_warnings(&warnings), 0);
    }

    /// Exercises `count_codec_skip_warnings` against a synthetic GAMDL
    /// v3.0 stderr capture that includes the structlog prefix.
    ///
    /// `count_codec_skip_warnings` matches by substring (lowercased),
    /// so the `[WARNING  HH:MM:SS]` prefix should not prevent a match.
    /// This test pins that invariant so a future regression in the
    /// matcher (e.g. someone anchoring the pattern at line start) is
    /// caught immediately.
    ///
    /// Fixture wording best-effort synthesis — see #521.
    #[test]
    fn count_codec_skip_warnings_handles_structlog_prefix() {
        let v3_lines = vec![
            "[WARNING  12:10:05] Skipping \"Track Two\": Requested format is not available".to_string(),
            "[INFO     12:10:06] Downloading \"Track Three\"".to_string(),
            "[WARNING  12:10:09] Skipping \"Track Four\": Requested format is not available".to_string(),
            "[INFO     12:10:10] Finished with 2 error(s)".to_string(),
        ];
        assert_eq!(
            count_codec_skip_warnings(&v3_lines),
            2,
            "count_codec_skip_warnings must see past GAMDL v3.0's \
             structlog [LEVEL HH:MM:SS] prefix"
        );
    }

    /// End-to-end: if the matcher works, the gap-fill chain builder
    /// must also produce a usable priority string when experimental
    /// codecs (wrapper-dependent) are present. Ties the two helpers
    /// together so a regression in one doesn't silently break the
    /// pipeline.
    #[test]
    fn v3_codec_skips_drive_gapfill_chain_construction() {
        let warnings = vec![
            "[WARNING  12:10:05] Skipping \"T1\": Requested format is not available".to_string(),
            "[WARNING  12:10:09] Skipping \"T2\": Requested format is not available".to_string(),
        ];
        let skip_count = count_codec_skip_warnings(&warnings);
        assert!(skip_count > 0);

        // Original chain favours Atmos (wrapper-dependent), then ALAC.
        // Gap-fill should drop Atmos and keep ALAC/AAC so the retry
        // pass can fill the missing tracks with lossless/lossy
        // fallbacks.
        let gap = build_gapfill_priority_chain("atmos,alac,aac").unwrap();
        assert_eq!(gap, "alac,aac");
    }

    /// `extract_python_exception` scans stderr for traceback lines.
    /// v3.0 leaves tracebacks unwrapped (structlog only formats
    /// `logger.error` calls, not the traceback Python itself prints),
    /// so this should still work. Pin the behaviour.
    #[test]
    fn v3_network_traceback_extraction_survives_structlog_interleaving() {
        let stderr_lines = vec![
            "[INFO     12:30:00] Processing \"https://music.apple.com/us/album/example/1234567890\"".to_string(),
            "[ERROR    12:30:05] Error processing \"https://...\": Connection timed out".to_string(),
            "Traceback (most recent call last):".to_string(),
            "  File \"gamdl/cli/cli.py\", line 142, in main".to_string(),
            "    downloader.download(url)".to_string(),
            "  File \"httpx/_transports/default.py\", line 118, in map_httpcore_exceptions".to_string(),
            "    raise mapped_exc(message) from exc".to_string(),
            "httpx.ConnectTimeout: Connection timed out".to_string(),
        ];
        let exception = extract_python_exception(&stderr_lines);
        assert!(
            exception.is_some(),
            "extract_python_exception should have captured the traceback"
        );
        let msg = exception.unwrap();
        assert!(
            msg.contains("ConnectTimeout"),
            "Expected the final exception line to be extracted; got: {msg}"
        );
    }

    // ==========================================================
    // normalize_url_for_dedup() tests
    // ==========================================================

    /// Verifies that domain case is normalised (RFC 3986 host is case-insensitive).
    #[test]
    fn normalize_url_lowercases_domain() {
        let a = normalize_url_for_dedup("https://Music.Apple.Com/us/album/test/123");
        let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
        assert_eq!(a, b);
    }

    /// Verifies that trailing slashes are stripped.
    #[test]
    fn normalize_url_strips_trailing_slash() {
        let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123/");
        let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
        assert_eq!(a, b);
    }

    /// Verifies that non-essential query parameters are removed.
    #[test]
    fn normalize_url_strips_non_essential_query() {
        let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123?ls=1&app=music");
        let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
        assert_eq!(a, b);
    }

    /// Verifies that the `?i=` query parameter (track ID) is preserved.
    #[test]
    fn normalize_url_keeps_track_id_query() {
        let url = normalize_url_for_dedup(
            "https://music.apple.com/us/album/test/123?i=456&ls=1",
        );
        assert_eq!(url, "https://music.apple.com/us/album/test/123?i=456");
    }

    /// Verifies that a URL with only `?i=` and no other params is preserved correctly.
    #[test]
    fn normalize_url_keeps_track_id_only() {
        let url = normalize_url_for_dedup(
            "https://music.apple.com/us/album/test/123?i=456",
        );
        assert_eq!(url, "https://music.apple.com/us/album/test/123?i=456");
    }

    /// Verifies that fragment identifiers are stripped.
    #[test]
    fn normalize_url_strips_fragment() {
        let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123#section");
        let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
        assert_eq!(a, b);
    }

    /// Verifies that two identical URLs produce the same normalised form.
    #[test]
    fn normalize_url_identical_urls_match() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        assert_eq!(
            normalize_url_for_dedup(url),
            normalize_url_for_dedup(url),
        );
    }

    // ==========================================================
    // has_duplicate_urls() tests
    // ==========================================================

    /// Verifies that `has_duplicate_urls` returns false for an empty queue.
    #[test]
    fn has_duplicate_urls_empty_queue() {
        let queue = DownloadQueue::new();
        let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
        assert!(!queue.has_duplicate_urls(&urls));
    }

    /// Verifies that `has_duplicate_urls` detects an exact URL match
    /// in a Queued item.
    #[test]
    fn has_duplicate_urls_detects_match() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
        };
        queue.enqueue(request, &settings);

        let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
        assert!(queue.has_duplicate_urls(&urls));
    }

    /// Verifies that `has_duplicate_urls` detects a case-insensitive match.
    #[test]
    fn has_duplicate_urls_case_insensitive() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
        };
        queue.enqueue(request, &settings);

        let urls = vec!["https://Music.Apple.Com/us/album/test/123".to_string()];
        assert!(queue.has_duplicate_urls(&urls));
    }

    /// Verifies that `has_duplicate_urls` ignores completed items.
    #[test]
    fn has_duplicate_urls_ignores_completed() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
        };
        let id = queue.enqueue(request, &settings);
        // Transition to complete
        queue.set_complete(&id);

        let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
        assert!(!queue.has_duplicate_urls(&urls));
    }

    /// Verifies that `has_duplicate_urls` ignores errored items.
    #[test]
    fn has_duplicate_urls_ignores_errored() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
        };
        let id = queue.enqueue(request, &settings);
        queue.set_error(&id, "some error");

        let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
        assert!(!queue.has_duplicate_urls(&urls));
    }

    /// Verifies that `has_duplicate_urls` returns false for a different URL.
    #[test]
    fn has_duplicate_urls_no_match() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
        };
        queue.enqueue(request, &settings);

        let urls = vec!["https://music.apple.com/us/album/other/456".to_string()];
        assert!(!queue.has_duplicate_urls(&urls));
    }

    // ============================================================
    // find_album_directory / find_deepest_audio_dir tests (#460)
    // ============================================================

    #[test]
    fn has_direct_audio_files_detects_m4a() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("track.m4a"), b"fake").unwrap();
        assert!(has_direct_audio_files(dir.path()));
    }

    #[test]
    fn has_direct_audio_files_ignores_nested() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("track.m4a"), b"fake").unwrap();
        // Parent dir has no direct audio files (only in subdir)
        assert!(!has_direct_audio_files(dir.path()));
    }

    #[test]
    fn has_direct_audio_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_direct_audio_files(dir.path()));
    }

    #[test]
    fn find_album_directory_targeted_match() {
        let base = tempfile::tempdir().unwrap();
        let album = base.path().join("Blue").join("Too Close - EP");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("01 Track.m4a"), b"fake").unwrap();

        let result = find_album_directory(base.path(), Some("Blue"), Some("Too Close - EP"));
        assert_eq!(result, Some(album.to_string_lossy().to_string()));
    }

    #[test]
    fn find_album_directory_case_insensitive() {
        let base = tempfile::tempdir().unwrap();
        let album = base.path().join("Blue").join("Too Close - EP");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("01 Track.m4a"), b"fake").unwrap();

        // Hints with different casing — on case-insensitive filesystems (macOS)
        // the original-cased path is found directly; on case-sensitive (Linux)
        // the case-insensitive fallback finds it. Either way, the paths should
        // match when compared case-insensitively.
        let result = find_album_directory(base.path(), Some("blue"), Some("too close - ep"));
        assert!(result.is_some(), "should find album directory");
        assert_eq!(
            result.unwrap().to_lowercase(),
            album.to_string_lossy().to_string().to_lowercase()
        );
    }

    #[test]
    fn find_album_directory_fallback_when_no_hints() {
        let base = tempfile::tempdir().unwrap();
        let album = base.path().join("Artist").join("Album");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("track.m4a"), b"fake").unwrap();

        let result = find_album_directory(base.path(), None, None);
        assert_eq!(result, Some(album.to_string_lossy().to_string()));
    }

    #[test]
    fn find_album_directory_returns_deepest() {
        let base = tempfile::tempdir().unwrap();
        // Create Artist/Album with audio
        let album = base.path().join("Artist").join("Album");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("track.m4a"), b"fake").unwrap();
        // Artist dir should NOT be returned (no direct audio files)
        let result = find_album_directory(base.path(), None, None);
        assert_eq!(result, Some(album.to_string_lossy().to_string()));
    }

    // ============================================================
    // rename_cover_art tests (#460)
    // ============================================================

    #[test]
    fn rename_cover_art_renames_jpg() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cover.jpg"), b"img").unwrap();
        rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");
        assert!(dir.path().join("FrontCover.jpg").exists());
        assert!(!dir.path().join("Cover.jpg").exists());
    }

    #[test]
    fn rename_cover_art_skips_when_target_cover() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cover.jpg"), b"img").unwrap();
        rename_cover_art(&dir.path().to_string_lossy(), "Cover");
        // Should keep as Cover.jpg — no rename
        assert!(dir.path().join("Cover.jpg").exists());
    }

    #[test]
    fn rename_cover_art_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("FrontCover.jpg"), b"existing").unwrap();
        std::fs::write(dir.path().join("Cover.jpg"), b"old").unwrap();
        rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");
        // FrontCover.jpg should keep existing content (not overwritten)
        let content = std::fs::read_to_string(dir.path().join("FrontCover.jpg")).unwrap();
        assert_eq!(content, "existing");
    }

    // ============================================================
    // validate_path_safe tests (#460)
    // ============================================================

    #[test]
    fn validate_path_safe_allows_normal_paths() {
        assert!(super::super::config_service::validate_path_safe("/home/user/Music").is_ok());
        assert!(super::super::config_service::validate_path_safe("C:\\Users\\Music").is_ok());
        assert!(super::super::config_service::validate_path_safe("./output").is_ok());
    }

    #[test]
    fn validate_path_safe_rejects_traversal() {
        assert!(super::super::config_service::validate_path_safe("../etc/passwd").is_err());
        assert!(super::super::config_service::validate_path_safe("/home/../root").is_err());
        assert!(super::super::config_service::validate_path_safe("foo/../../bar").is_err());
    }

    // ============================================================
    // filter_tiers_by_audio_traits (#504)
    // ============================================================

    fn tier(codecs: Vec<SongCodec>, suffix: bool) -> super::CompanionTier {
        super::CompanionTier {
            codecs_to_try: codecs,
            apply_suffix: suffix,
        }
    }

    #[test]
    fn filter_tiers_passes_through_when_no_traits() {
        let tiers = vec![tier(vec![SongCodec::Ac3], false)];
        let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &[]);
        assert_eq!(kept.len(), 1);
        assert!(skipped.is_empty());
    }

    #[test]
    fn filter_tiers_drops_unmatched_required_trait() {
        // PSYCHO single example: track has atmos / lossless / lossy-stereo / spatial
        // but NOT dolby-digital. AC3 must be dropped.
        let traits = vec![
            "atmos".to_string(),
            "lossless".to_string(),
            "lossy-stereo".to_string(),
            "spatial".to_string(),
        ];
        let tiers = vec![
            tier(vec![SongCodec::Ac3], true),
            tier(vec![SongCodec::AacBinaural], false),
        ];
        let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
        assert_eq!(kept.len(), 1, "AacBinaural tier should survive");
        assert_eq!(kept[0].codecs_to_try, vec![SongCodec::AacBinaural]);
        assert_eq!(skipped, vec!["ac3".to_string()]);
    }

    #[test]
    fn filter_tiers_keeps_tier_when_any_codec_in_chain_supported() {
        let traits = vec!["lossy-stereo".to_string()];
        // A multi-codec tier where one codec is supported and another isn't
        let tiers = vec![tier(vec![SongCodec::Ac3, SongCodec::Aac], false)];
        let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
        assert_eq!(kept.len(), 1);
        assert!(skipped.is_empty());
    }

    #[test]
    fn filter_tiers_keeps_derived_codecs_unconditionally() {
        // AacBinaural has no required_audio_trait — it should survive
        // even when the trait list is empty-ish.
        let traits = vec!["lossy-stereo".to_string()]; // intentionally minimal
        let tiers = vec![tier(vec![SongCodec::AacBinaural], false)];
        let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
        assert_eq!(kept.len(), 1);
        assert!(skipped.is_empty());
    }
}
