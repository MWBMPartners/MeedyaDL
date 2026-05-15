// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL download execution IPC commands.
// Handles starting downloads, cancelling active downloads, retrying
// failed downloads, clearing completed items, and querying the download
// queue status. Downloads are managed by the download_queue service
// which handles concurrent execution, fallback quality, and retries.
//
// ## Architecture
//
// These are Tauri IPC command handlers — the bridge between the React/TypeScript
// frontend and the Rust backend. Each `#[tauri::command]` function is callable
// from the frontend via `invoke()` in `src/lib/tauri-commands.ts`.
//
// The download lifecycle is:
//   1. Frontend calls `startDownload(request)` -> `start_download()`
//   2. Download is enqueued in the `QueueHandle` (a Tokio Mutex-wrapped queue)
//   3. `process_queue()` picks up the item and spawns a GAMDL subprocess
//   4. Frontend polls `getQueueStatus()` -> `get_queue_status()` for progress
//   5. Frontend can cancel via `cancelDownload(id)` -> `cancel_download()`
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command          | TypeScript Function        | Line |
// |-----------------------|----------------------------|------|
// | start_download        | startDownload()            | ~99  |
// | cancel_download       | cancelDownload()           | ~104 |
// | retry_download        | retryDownload()            | ~109 |
// | clear_queue           | clearQueue()               | ~114 |
// | get_queue_status      | getQueueStatus()           | ~119 |
// | check_gamdl_update    | checkGamdlUpdate()         | ~124 |
// | export_activity_log   | exportActivityLog(entries)  |      |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Tauri State management: https://v2.tauri.app/develop/state-management/
// - Tauri Events (emit): https://v2.tauri.app/develop/calling-frontend/

// serde::Serialize is required for any struct returned to the frontend — Tauri
// serializes return values to JSON before sending them over the IPC bridge.
use serde::Serialize;
// AppHandle provides access to Tauri's managed state and app-level APIs.
// Emitter allows sending events from Rust to the frontend (e.g., "download-queued").
// State<'_, T> is Tauri's dependency injection for managed state (see main.rs setup).
use tauri::{AppHandle, Emitter, State};

// DownloadRequest: the deserialized JSON payload from the frontend containing
// URLs and optional per-download quality/format overrides.
// QueueItemStatus: per-item status info (id, state, progress, error message).
use crate::models::download::{DownloadRequest, QueueItemStatus, StartDownloadResult};
// download_queue module contains the queue processing logic (process_queue).
// QueueHandle is an Arc<Mutex<DownloadQueue>> shared across all command invocations.
use crate::services::download_queue::{self, QueueHandle};
use crate::utils::activity_log::{emit_app_log, emit_verbose_app_log};

/// Status of all items in the download queue.
///
/// This struct is serialized to JSON and returned to the frontend by
/// `get_queue_status()`. The frontend uses it to render the download queue
/// UI panel with progress bars, status badges, and action buttons.
///
/// Implements `Serialize` (required by Tauri for IPC return values).
/// See: <https://v2.tauri.app/develop/calling-rust/#return-types>
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    /// Total number of items in the queue (all states combined)
    pub total: usize,
    /// Number of items currently downloading (state == Active)
    pub active: usize,
    /// Number of items waiting to start (state == Queued)
    pub queued: usize,
    /// Number of items that completed successfully (state == Completed)
    pub completed: usize,
    /// Number of items that failed with errors (state == Failed)
    pub failed: usize,
    /// Detailed status for each queue item, including per-item progress,
    /// error messages, and the original download request parameters.
    pub items: Vec<QueueItemStatus>,
}

/// Starts a new download by adding it to the queue.
///
/// **Frontend caller:** `startDownload(request)` in `src/lib/tauri-commands.ts`
///
/// The download request includes the Apple Music URL(s) and optional
/// quality/format overrides. If no overrides are specified, the global
/// settings are used. The download is added to the queue and will be
/// processed when a slot becomes available (default: 1 concurrent).
///
/// Returns a unique download ID (UUID) for tracking progress and cancellation.
///
/// # Arguments
/// * `app` - Tauri `AppHandle`, injected automatically by the IPC runtime.
///   Used to access managed state, emit events, and resolve paths.
/// * `queue` - The download queue state, injected via `State<'_, QueueHandle>`.
///   This is a Tokio Mutex-wrapped `DownloadQueue` registered in `main.rs`.
///   See: <https://v2.tauri.app/develop/state-management>/
/// * `request` - The download request payload deserialized from the frontend JSON.
///   Contains `urls: Vec<String>` and optional override fields.
/// * `skip_auto_start` - When `Some(true)`, the item is queued but `process_queue()`
///   is NOT called, regardless of the `auto_start_queue` setting. Used by the
///   frontend when the device is offline — the item sits in `Queued` state until
///   the user manually starts the queue or a future online download triggers processing.
///
/// # Returns
/// * `Ok(StartDownloadResult)` - Contains the download ID (UUID v4) and an optional
///   `duplicate_warning` message when the URL is already in the active queue.
/// * `Err(String)` - Human-readable error message if the event emission fails.
///
/// # Errors
/// Returns an error if emitting the `"download-queued"` event to the frontend fails.
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful enqueue.
///   The frontend listens for this to update the queue UI immediately.
///   See: <https://v2.tauri.app/develop/calling-frontend>/
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    request: DownloadRequest,
    skip_auto_start: Option<bool>,
) -> Result<StartDownloadResult, String> {
    // Rate limit: max 10 downloads per minute
    crate::utils::rate_limiter::check_rate_limit("start_download", 10, 60)?;

    // Load current settings for merging with per-download overrides.
    // If settings can't be loaded (corrupted file, etc.), fall back to defaults
    // so the download can still proceed with sensible quality/format choices.
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Verbose: log key settings for debugging
    emit_verbose_app_log(
        &app,
        &format!(
            "Download settings: codec={:?}, wrapper={}, cookies={}, output={}",
            settings.default_song_codec,
            settings.use_wrapper,
            settings.cookies_path.as_deref().unwrap_or("(none)"),
            settings.output_path,
        ),
    );

    // Normalize non-geographic Apple Music URLs by injecting a storefront
    // code. GAMDL requires a 2-letter storefront in the URL path (e.g., /us/).
    // MeedyaDL detects URLs missing a storefront and injects one based on the
    // OS locale (or "us" as fallback). The storefront is cosmetic for GAMDL
    // (it uses the user's cookies/wrapper auth to determine the real storefront)
    // but structurally required for GAMDL's URL regex to match.
    let mut request = request;

    // Validate all URLs belong to supported domains (#459).
    // Reject any URL whose host does not exactly match an Apple Music,
    // Apple Music Classical, or legacy iTunes domain. Uses url::Url to
    // parse and extract host_str() so that substring tricks like
    // "evil.com/?next=music.apple.com/..." cannot bypass the check.
    const SUPPORTED_HOSTS: &[&str] = &[
        "music.apple.com",
        "classical.apple.com",
        "itunes.apple.com",
    ];
    for url in &request.urls {
        let parsed = url::Url::parse(url)
            .map_err(|e| format!("Invalid URL '{url}': {e}"))?;
        if parsed.scheme() == "http" || parsed.scheme() == "https" {
            let host = parsed.host_str().unwrap_or("").to_lowercase();
            // Check for an exact host match or a subdomain (e.g., "geo.music.apple.com").
            // Use strip_suffix to avoid per-iteration string allocations.
            let is_supported = SUPPORTED_HOSTS.iter().any(|&allowed| {
                host == allowed
                    || host
                        .strip_suffix(allowed)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            });
            if !is_supported {
                return Err(format!(
                    "Unsupported URL domain: {url}. Only Apple Music, Apple Music Classical, and iTunes URLs are supported."
                ));
            }
        }
    }

    let original_urls = request.urls.clone();
    request.urls = request
        .urls
        .into_iter()
        .map(|url| crate::services::apple_music_api::normalize_apple_music_url(&url))
        .collect();

    // Log when normalization occurs so the user sees what happened.
    // Two distinct rewrites can fire inside `normalize_apple_music_url`:
    //   - iTunes-host rewrite (#568): `itunes.apple.com` → `music.apple.com`
    //     plus stripping the legacy `id` prefix from numeric IDs.
    //   - Storefront injection: missing `/<storefront>/` segment filled in
    //     from the OS locale (or `us` as a last-resort default).
    // Both can apply to the same URL (e.g. a non-geographic iTunes URL
    // gets host-rewritten AND storefront-injected). Detect each
    // contribution by host comparison so the user-facing log line is
    // accurate, not just the generic "storefront auto-detected" we used
    // to emit for every change.
    for (original, normalized) in original_urls.iter().zip(request.urls.iter()) {
        if original != normalized {
            let was_itunes_rewrite = original.contains("itunes.apple.com")
                && !normalized.contains("itunes.apple.com");
            let message = if was_itunes_rewrite {
                format!(
                    "Legacy iTunes URL rewritten to music.apple.com (GAMDL doesn't accept itunes.apple.com): {normalized}"
                )
            } else {
                format!("URL normalized — storefront auto-detected: {normalized}")
            };
            log::info!("Normalized URL: {original} → {normalized}");
            emit_app_log(&app, &message);
        }
    }

    // URL audit diagnostics (#546/#547/#548/#549 — part of the #487
    // umbrella). Classify each URL and emit one log line per matching
    // class so downstream filename-path behaviour can be correlated with
    // the URL class without replaying the download. Per-URL (not batched)
    // so each offending URL lands as its own activity-log entry in
    // timestamp order. All four classes are orthogonal except library,
    // which is skipped by the #549 catch-all (library URLs already have
    // their own #546 trace).
    //
    // - #546: `/library/` URLs pass through to GAMDL unchanged; no
    //   MeedyaDL storefront injection, prefetch, or Tier 4 safety net.
    // - #547: `classical.apple.com` URLs ride the same templates as
    //   `music.apple.com` — movement-title collisions are the risk.
    // - #548: `itunes.apple.com` URLs parse cleanly after the regex gap
    //   fix, but GAMDL's own URL regex compatibility is unverified — WARN.
    // - #549: host-allowed URLs that fail `parse_apple_music_url` after
    //   normalisation (uploaded / post videos and any novel path) — WARN.
    for url in &request.urls {
        let is_library = url.contains("/library/");

        if is_library {
            log::info!("Library URL passed through to GAMDL (#546): {url}");
            emit_app_log(
                &app,
                &format!(
                    "Library URL detected (#546) — passed through to GAMDL unchanged; no MeedyaDL filename safety net applies: {url}"
                ),
            );
        }

        if url.contains("itunes.apple.com") {
            // #568 added an iTunes-host rewrite to `normalize_apple_music_url`
            // that runs BEFORE this audit pass — recognised iTunes URL
            // shapes (album / song / music-video / artist / playlist) get
            // rewritten to music.apple.com. So if we still see an
            // itunes.apple.com URL here, it didn't match any of those
            // shapes (e.g. a `/lookup`-style URL or a novel path the
            // rewrite alternation doesn't cover yet). GAMDL will reject
            // it; warn the user so they can convert manually.
            log::warn!(
                "iTunes URL didn't match the rewrite alternation (#568 / #548) — GAMDL will reject: {url}"
            );
            emit_app_log(
                &app,
                &format!(
                    "iTunes URL detected but couldn't be auto-rewritten — please copy the music.apple.com link instead: {url}"
                ),
            );
        }

        if url.contains("classical.apple.com") {
            log::info!("Classical URL routed through shared Apple Music pipeline (#547): {url}");
            emit_app_log(
                &app,
                &format!(
                    "Classical URL detected (#547) — same templates as music.apple.com; watch for movement-title collisions: {url}"
                ),
            );
        }

        if !is_library
            && crate::services::apple_music_api::parse_apple_music_url(url).is_none()
        {
            log::warn!(
                "Unrecognised Apple Music URL shape (#549) — passed to GAMDL without MeedyaDL prefetch or safety net: {url}"
            );
            emit_app_log(
                &app,
                &format!(
                    "Unrecognised Apple Music URL (#549) — no MeedyaDL metadata prefetch or filename safety net applies; GAMDL compatibility unverified: {url}"
                ),
            );
        }
    }

    // Check for duplicate URLs already in the active queue. This is a
    // non-blocking warning — the download proceeds regardless, but the
    // frontend can show a toast to let the user know.
    let duplicate_warning = {
        let q = queue.lock().await;
        if q.has_duplicate_urls(&request.urls) {
            log::info!("Duplicate URL detected in queue: {}", request.urls.join(", "));
            Some("This URL is already in the queue".to_string())
        } else {
            None
        }
    };

    // Batch cross-URL dedup (#513). When the user pastes multiple URLs
    // in one request (e.g. an album + a playlist that overlap), walk
    // every album/playlist URL's track list and apply a source-priority
    // filter (song > album > playlist) so any given track is claimed by
    // exactly one URL. URLs that end up winning nothing are dropped from
    // the request; URLs that partially win are rewritten to per-track
    // URLs. Song/artist/video URLs pass through unchanged.
    //
    // Runs BEFORE the single-URL album and playlist planners (#512 /
    // #514) so the later planners see the already-trimmed URL list.
    if !matches!(
        settings.duplicate_detection.scope,
        crate::models::settings::DuplicateDetectionScope::Off
    ) && request.urls.len() > 1
    {
        let queued_keys = {
            let snapshot = {
                let q = queue.lock().await;
                q.get_status()
            };
            crate::services::duplicate_detector::collect_queued_keys(
                &snapshot,
                settings.duplicate_detection.scope,
                settings.duplicate_detection.key_strategy,
            )
        };
        let history_keys = crate::services::duplicate_detector::collect_history_keys(
            &settings.output_path,
            settings.duplicate_detection.scope,
            settings.duplicate_detection.key_strategy,
        );
        if let Some(batch_plan) =
            crate::services::duplicate_detector::plan_batch_deduplication(
                &app,
                &request.urls,
                &settings,
                &queued_keys,
                &history_keys,
            )
            .await
        {
            for line in &batch_plan.activity_lines {
                emit_app_log(&app, line);
            }
            if batch_plan.skipped_count > 0 || batch_plan.dropped_url_count > 0 {
                emit_app_log(
                    &app,
                    &format!(
                        "Batch dedup: {} track(s) skipped, {} URL(s) dropped",
                        batch_plan.skipped_count, batch_plan.dropped_url_count
                    ),
                );
            }

            // Apply the per-URL actions, preserving original paste order.
            let mut new_urls: Vec<String> = Vec::with_capacity(request.urls.len());
            for (i, action) in batch_plan.per_url.iter().enumerate() {
                match action {
                    crate::services::duplicate_detector::BatchUrlAction::KeepOriginal => {
                        if let Some(u) = request.urls.get(i) {
                            new_urls.push(u.clone());
                        }
                    }
                    crate::services::duplicate_detector::BatchUrlAction::Replace(urls) => {
                        new_urls.extend(urls.iter().cloned());
                    }
                    crate::services::duplicate_detector::BatchUrlAction::Drop => {
                        // Skip entirely.
                    }
                }
            }

            if new_urls.is_empty() {
                emit_app_log(
                    &app,
                    "Nothing queued — every track across the batch is already downloaded or queued",
                );
                return Ok(StartDownloadResult {
                    download_id: String::new(),
                    duplicate_warning: Some(
                        "Every track in this batch is already downloaded or queued"
                            .to_string(),
                    ),
                });
            }
            request.urls = new_urls;
        }
    }

    // Multi-select artist auto-select: if the URL is an artist URL and
    // multiple modes are configured, split into N separate downloads (one
    // per mode). GAMDL only accepts a single --artist-auto-select value,
    // so MeedyaDL creates separate queue items for each selected mode.
    let is_artist_url = request.urls.iter().any(|u| u.contains("/artist/"));
    let artist_modes = &settings.artist_auto_select_multi;

    if is_artist_url && artist_modes.len() > 1 {
        // Capture URL display string before splitting
        let urls_display = request.urls.join(", ");
        let mut first_id = String::new();

        // Pre-queue duplicate detection (#510). When enabled, we fetch each
        // mode's albums/tracks via the Apple Music catalog API and rewrite
        // per-mode URL lists so that a song appearing in multiple modes
        // (e.g. on an album AND a compilation) is only downloaded once at
        // the preferred mode. Companion-format downloads are unaffected.
        //
        // Only runs for the first artist URL in the request — batch pastes
        // of multiple artist URLs fall through with per-mode dedup skipped
        // for the secondary URLs (rare case; keeps the code bounded).
        let dedup_plan = if crate::services::duplicate_detector::dedup_enabled_for(
            &settings.duplicate_detection,
            artist_modes,
        ) {
            let first_artist_url = request
                .urls
                .iter()
                .find(|u| u.contains("/artist/"))
                .cloned()
                .unwrap_or_default();
            let parsed = crate::services::apple_music_api::parse_apple_music_url(&first_artist_url);
            if let Some(parsed) = parsed.filter(|p| p.content_type == "artist") {
                // Collect queue / history keys per the configured scope.
                let queued_keys = {
                    let snapshot = {
                        let q = queue.lock().await;
                        q.get_status()
                    };
                    crate::services::duplicate_detector::collect_queued_keys(
                        &snapshot,
                        settings.duplicate_detection.scope,
                        settings.duplicate_detection.key_strategy,
                    )
                };
                let history_keys = crate::services::duplicate_detector::collect_history_keys(
                    &settings.output_path,
                    settings.duplicate_detection.scope,
                    settings.duplicate_detection.key_strategy,
                );
                emit_app_log(&app, "Checking for duplicate tracks across selected artist modes...");
                crate::services::duplicate_detector::plan_artist_deduplication(
                    &app,
                    &parsed,
                    &first_artist_url,
                    artist_modes,
                    &settings,
                    &queued_keys,
                    &history_keys,
                )
                .await
            } else {
                None
            }
        } else {
            None
        };

        // Emit per-skipped-track activity log lines. Done before the enqueue
        // block so the log messages appear in chronological order relative
        // to the queue entries that follow.
        if let Some(plan) = &dedup_plan {
            for line in &plan.activity_lines {
                emit_app_log(&app, line);
            }
            if plan.skipped_count > 0 {
                emit_app_log(
                    &app,
                    &format!(
                        "Duplicate detection: {} track(s) skipped across {} mode(s)",
                        plan.skipped_count,
                        artist_modes.len()
                    ),
                );
            }
        }

        {
            let mut q = queue.lock().await;
            for (i, mode) in artist_modes.iter().enumerate() {
                // Clone the request and set the per-download artist override
                let mut split_request = request.clone();
                let overrides = split_request.options.get_or_insert_with(Default::default);
                overrides.artist_auto_select = Some(mode.clone());

                // Apply the dedup plan for this mode, if any.
                if let Some(plan) = &dedup_plan {
                    let mode_plan = plan
                        .per_mode
                        .iter()
                        .find(|(m, _)| m == mode)
                        .map(|(_, p)| p);
                    match mode_plan {
                        Some(
                            crate::services::duplicate_detector::ModePlan::SkipEntirely,
                        ) => {
                            emit_app_log(
                                &app,
                                &format!(
                                    "Skipping artist mode {} — every track is a duplicate",
                                    mode.display_name()
                                ),
                            );
                            continue;
                        }
                        Some(crate::services::duplicate_detector::ModePlan::TrackUrls(
                            urls,
                        )) => {
                            // Replace the artist URL with explicit per-track
                            // URLs; GAMDL will download each as a single
                            // track. Drop --artist-auto-select because it's
                            // meaningless for non-artist URLs.
                            split_request.urls = urls.clone();
                            let overrides = split_request
                                .options
                                .get_or_insert_with(Default::default);
                            overrides.artist_auto_select = None;
                        }
                        Some(
                            crate::services::duplicate_detector::ModePlan::FallbackToArtistUrl,
                        )
                        | None => {
                            // Keep the original artist URL + mode override.
                        }
                    }
                }

                if q.has_duplicate_urls(&split_request.urls) {
                    crate::services::history_service::remove_entries_for_urls(
                        &app,
                        &split_request.urls,
                    );
                    emit_app_log(
                        &app,
                        &format!(
                            "Skipped duplicate queued URL(s): {}",
                            split_request.urls.join(", ")
                        ),
                    );
                    continue;
                }

                let queued_urls = split_request.urls.clone();
                let download_id = q.enqueue(split_request, &settings);
                crate::services::history_service::remove_entries_for_urls(&app, &queued_urls);
                log::info!(
                    "Download {download_id} queued (artist mode: {})",
                    mode.to_cli_string()
                );
                if i == 0 || first_id.is_empty() {
                    first_id = download_id;
                }
            }
        }

        if first_id.is_empty() {
            // Every mode was SkipEntirely — report this to the caller as a
            // no-op success with a warning. The frontend will show the
            // duplicate_warning toast and the user sees the activity log.
            emit_app_log(
                &app,
                &format!(
                    "Nothing queued for {urls_display} — every track is already downloaded or queued"
                ),
            );
            return Ok(StartDownloadResult {
                download_id: String::new(),
                duplicate_warning: Some(
                    "All tracks from this artist URL were duplicates and were not re-queued"
                        .to_string(),
                ),
            });
        }

        emit_app_log(
            &app,
            &format!(
                "Queued: {urls_display} ({} artist modes)",
                artist_modes.len()
            ),
        );

        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        app.emit("download-queued", &first_id)
            .map_err(|e| format!("Failed to emit event: {e}"))?;

        if settings.auto_start_queue && !skip_auto_start.unwrap_or(false) {
            download_queue::process_queue(app, queue_handle).await;
        }

        return Ok(StartDownloadResult {
            download_id: first_id,
            duplicate_warning: duplicate_warning.clone(),
        });
    }

    // Single-mode path: standard enqueue (also handles single artist_auto_select_multi)
    let urls_display = request.urls.join(", ");

    // If exactly one multi-mode is set and it's an artist URL, apply it as an override
    let mut request = if is_artist_url && artist_modes.len() == 1 {
        let mut req = request;
        let overrides = req.options.get_or_insert_with(Default::default);
        if overrides.artist_auto_select.is_none() {
            overrides.artist_auto_select = Some(artist_modes[0].clone());
        }
        req
    } else {
        request
    };

    // Playlist dedup (#512). When a catalog playlist URL is queued, fetch
    // its tracks via the Apple Music API and filter out any that match
    // song_ids / ISRCs already present in the queue or the download
    // history (per the configured scope). Single-URL requests only —
    // batch pastes go through the more general cross-URL dedup in #513.
    //
    // Like the artist planner, companion-format downloads for the winning
    // tracks are unaffected (dedup operates on track identity, not codec).
    let playlist_warning: Option<String> = if request.urls.len() == 1
        && !matches!(
            settings.duplicate_detection.scope,
            crate::models::settings::DuplicateDetectionScope::Off
        ) {
        let single_url = request.urls[0].clone();
        let parsed = crate::services::apple_music_api::parse_apple_music_url(&single_url);
        let is_playlist = parsed
            .as_ref()
            .is_some_and(|p| p.content_type == "playlist");
        if is_playlist {
            let parsed = parsed.unwrap();
            let queued_keys = {
                let snapshot = {
                    let q = queue.lock().await;
                    q.get_status()
                };
                crate::services::duplicate_detector::collect_queued_keys(
                    &snapshot,
                    settings.duplicate_detection.scope,
                    settings.duplicate_detection.key_strategy,
                )
            };
            let history_keys = crate::services::duplicate_detector::collect_history_keys(
                &settings.output_path,
                settings.duplicate_detection.scope,
                settings.duplicate_detection.key_strategy,
            );
            emit_app_log(&app, "Checking playlist for duplicate tracks...");
            let plan = crate::services::duplicate_detector::plan_playlist_deduplication(
                &app,
                &parsed,
                &settings,
                &queued_keys,
                &history_keys,
            )
            .await;

            if let Some(plan) = plan {
                for line in &plan.activity_lines {
                    emit_app_log(&app, line);
                }
                match plan.plan {
                    crate::services::duplicate_detector::PlaylistPlan::TrackUrls(urls) => {
                        request.urls = urls;
                        emit_app_log(
                            &app,
                            &format!(
                                "Playlist dedup: kept {} track(s), skipped {}",
                                plan.kept_count, plan.skipped_count
                            ),
                        );
                        None
                    }
                    crate::services::duplicate_detector::PlaylistPlan::SkipEntirely => {
                        emit_app_log(
                            &app,
                            "Nothing queued — every track in the playlist is already downloaded or queued",
                        );
                        return Ok(StartDownloadResult {
                            download_id: String::new(),
                            duplicate_warning: Some(
                                "All tracks from this playlist were duplicates and were not re-queued"
                                    .to_string(),
                            ),
                        });
                    }
                    crate::services::duplicate_detector::PlaylistPlan::FallbackToPlaylistUrl => {
                        // No changes to request.urls — GAMDL will handle the full playlist.
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let _ = playlist_warning; // reserved for future UI surfacing

    // Album URL vs history/queue dedup (#514). Covers the "I already
    // downloaded the standard edition, then queued the deluxe edition"
    // case — the bonus tracks get enqueued, the rest are skipped.
    // Only runs for single-URL requests (batch pastes go through #513).
    // Album URLs with `?i=song_id` (single-song selectors) are passed
    // through unchanged.
    let album_warning: Option<String> = if request.urls.len() == 1
        && !matches!(
            settings.duplicate_detection.scope,
            crate::models::settings::DuplicateDetectionScope::Off
        ) {
        let single_url = request.urls[0].clone();
        let parsed = crate::services::apple_music_api::parse_apple_music_url(&single_url);
        let is_album_without_song_selector = parsed
            .as_ref()
            .is_some_and(|p| p.content_type == "album" && p.song_id.is_none());
        if is_album_without_song_selector {
            let parsed = parsed.unwrap();
            let queued_keys = {
                let snapshot = {
                    let q = queue.lock().await;
                    q.get_status()
                };
                crate::services::duplicate_detector::collect_queued_keys(
                    &snapshot,
                    settings.duplicate_detection.scope,
                    settings.duplicate_detection.key_strategy,
                )
            };
            let history_keys = crate::services::duplicate_detector::collect_history_keys(
                &settings.output_path,
                settings.duplicate_detection.scope,
                settings.duplicate_detection.key_strategy,
            );
            // Short-circuit the expensive catalog fetch if there's
            // literally nothing to compare against.
            if queued_keys.is_empty() && history_keys.is_empty() {
                None
            } else {
                emit_app_log(
                    &app,
                    "Checking album against already-queued and previously downloaded tracks...",
                );
                let plan = crate::services::duplicate_detector::plan_album_deduplication(
                    &app,
                    &parsed,
                    &settings,
                    &queued_keys,
                    &history_keys,
                )
                .await;
                if let Some(plan) = plan {
                    for line in &plan.activity_lines {
                        emit_app_log(&app, line);
                    }
                    match plan.plan {
                        crate::services::duplicate_detector::AlbumPlan::TrackUrls(urls) => {
                            request.urls = urls;
                            emit_app_log(
                                &app,
                                &format!(
                                    "Album dedup: kept {} new track(s), skipped {}",
                                    plan.kept_count, plan.skipped_count
                                ),
                            );
                            None
                        }
                        crate::services::duplicate_detector::AlbumPlan::SkipEntirely => {
                            emit_app_log(
                                &app,
                                "Nothing queued — every track in this album is already downloaded or queued",
                            );
                            return Ok(StartDownloadResult {
                                download_id: String::new(),
                                duplicate_warning: Some(
                                    "Every track in this album is already downloaded or queued. Change scope in Settings > Quality > Duplicate Detection to download anyway."
                                        .to_string(),
                                ),
                            });
                        }
                        crate::services::duplicate_detector::AlbumPlan::FallbackToAlbumUrl => {
                            None
                        }
                    }
                } else {
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    let _ = album_warning; // reserved for future UI surfacing

    // Acquire the queue lock and enqueue the download. The lock is scoped
    // to this block to release it before the async process_queue() call,
    // avoiding potential deadlocks.
    let submitted_urls = request.urls.clone();
    let filtered_urls = {
        let q = queue.lock().await;
        q.filter_out_active_duplicate_urls(&request.urls)
    };
    if filtered_urls.is_empty() {
        crate::services::history_service::remove_entries_for_urls(&app, &submitted_urls);
        emit_app_log(
            &app,
            &format!("Nothing queued for {urls_display} — already in Queue"),
        );
        return Ok(StartDownloadResult {
            download_id: String::new(),
            duplicate_warning: Some("This URL is already in the queue".to_string()),
        });
    }
    if filtered_urls.len() != request.urls.len() {
        let skipped = request.urls.len() - filtered_urls.len();
        emit_app_log(
            &app,
            &format!("Skipped {skipped} duplicate queued URL(s) from request"),
        );
        request.urls = filtered_urls;
    }

    let download_id = {
        let mut q = queue.lock().await;
        q.enqueue(request, &settings)
    };
    crate::services::history_service::remove_entries_for_urls(&app, &submitted_urls);

    log::info!("Download {download_id} queued");
    emit_app_log(&app, &format!("Queued: {urls_display}"));

    // Persist the updated queue to disk for crash recovery.
    // This ensures the new item survives an unexpected app close/crash.
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    // Emit a Tauri event to notify the frontend that the download has been queued.
    // The frontend listens for "download-queued" events to refresh the queue UI.
    app.emit("download-queued", &download_id)
        .map_err(|e| format!("Failed to emit event: {e}"))?;

    // Trigger queue processing if auto-start is enabled AND the caller
    // didn't request skipping it. `skip_auto_start` is set by the frontend
    // when the device is offline — the item is queued but not processed
    // until the user retries or a future download triggers queue processing.
    if settings.auto_start_queue && !skip_auto_start.unwrap_or(false) {
        download_queue::process_queue(app, queue_handle).await;
    }

    Ok(StartDownloadResult {
        download_id,
        duplicate_warning,
    })
}

/// Cancels an active or queued download.
///
/// **Frontend caller:** `cancelDownload(downloadId)` in `src/lib/tauri-commands.ts`
///
/// If the download is currently active, the GAMDL subprocess is killed via
/// its stored `Child` process handle. If it's still queued (not yet started),
/// it's moved directly to the Cancelled state without ever spawning a process.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID (UUID) returned by `start_download`.
///   The frontend passes this as `downloadId` (camelCase) and Tauri
///   automatically converts it to `download_id` (`snake_case`).
///   See: <https://v2.tauri.app/develop/calling-rust/#command-arguments>
///
/// # Returns
/// * `Ok(())` - The download was successfully cancelled.
/// * `Err(String)` - The download ID was not found or the item already finished.
///
/// # Errors
/// Returns an error if the download ID was not found or the item has already
/// completed or failed (i.e., it is no longer in a cancellable state).
///
/// # Events Emitted
/// * `"download-cancelled"` - Emitted with the download ID on successful cancellation.
#[tauri::command]
pub async fn cancel_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Cancel requested for download: {download_id}");

    // Acquire lock, attempt cancellation, then release lock.
    // q.cancel() returns true if the item was found and successfully cancelled.
    let cancelled = {
        let mut q = queue.lock().await;
        q.cancel(&download_id)
    };

    if cancelled {
        // Persist the updated queue (cancelled item removed from active set)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Cancelled download [{short}]"));

        // Notify the frontend so it can update the item's UI state immediately.
        // We use `let _ =` to ignore emission errors — the cancellation itself
        // already succeeded, so a failed event is non-critical.
        let _ = app.emit("download-cancelled", &download_id);
        Ok(())
    } else {
        // The download ID was not found, or the item has already completed/failed.
        Err(format!(
            "Download {download_id} not found or already finished"
        ))
    }
}

// ============================================================
// Queue reorder commands (#782)
// ============================================================
//
// Four IPC entry points lets the frontend reorder Queued items live,
// while other items are still processing. All four:
//
//   - Acquire the queue Mutex (so they cannot interleave with
//     `next_pending` or any other queue mutation).
//   - Call the matching `DownloadQueue` method.
//   - On a non-no-op move, persist the queue to disk + emit
//     `queue-updated` so the frontend re-fetches.
//   - Emit a `[System]` activity-log entry naming the item and the
//     direction so reorders are traceable.
//
// `Ok(true)` means the queue mutated; `Ok(false)` means the move was
// a no-op (item missing, not Queued, or already at the requested
// position). Errors are reserved for queue-lock failures.

async fn run_queue_move(
    app: &AppHandle,
    queue: &State<'_, QueueHandle>,
    download_id: &str,
    direction_label: &'static str,
    op: impl FnOnce(&mut crate::services::download_queue::DownloadQueue) -> bool,
) -> Result<bool, String> {
    let (moved, label) = {
        let mut q = queue.lock().await;
        let moved = op(&mut q);
        // Capture a friendly label while we still hold the lock so
        // the activity-log entry says "Black Velvet" not just an ID.
        let label = q
            .friendly_label(download_id)
            .unwrap_or_else(|| download_id.to_string());
        (moved, label)
    };

    if moved {
        let queue_handle = queue.inner().clone();
        crate::services::download_queue::save_queue_to_disk(app, &queue_handle).await;
        let _ = app.emit("queue-updated", ());
        emit_app_log(app, &format!("Reordered: {direction_label} — \"{label}\""));
    }

    Ok(moved)
}

/// Move a `Queued` item to the top of the pending sub-sequence.
/// Returns `true` when the queue changed, `false` when it was a no-op
/// (item not found, not Queued, or already at the top).
#[tauri::command]
pub async fn move_queue_item_to_top(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<bool, String> {
    log::debug!("Queue move-to-top requested for {download_id}");
    run_queue_move(&app, &queue, &download_id, "move to top", |q| {
        q.move_to_top(&download_id)
    })
    .await
}

/// Move a `Queued` item to the bottom of the pending sub-sequence.
#[tauri::command]
pub async fn move_queue_item_to_bottom(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<bool, String> {
    log::debug!("Queue move-to-bottom requested for {download_id}");
    run_queue_move(&app, &queue, &download_id, "move to bottom", |q| {
        q.move_to_bottom(&download_id)
    })
    .await
}

/// Swap a `Queued` item with the `Queued` item immediately above it.
#[tauri::command]
pub async fn move_queue_item_up(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<bool, String> {
    log::debug!("Queue move-up requested for {download_id}");
    run_queue_move(&app, &queue, &download_id, "move up", |q| {
        q.move_up(&download_id)
    })
    .await
}

/// Swap a `Queued` item with the `Queued` item immediately below it.
#[tauri::command]
pub async fn move_queue_item_down(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<bool, String> {
    log::debug!("Queue move-down requested for {download_id}");
    run_queue_move(&app, &queue, &download_id, "move down", |q| {
        q.move_down(&download_id)
    })
    .await
}

/// Retries a failed or cancelled download.
///
/// **Frontend caller:** `retryDownload(downloadId)` in `src/lib/tauri-commands.ts`
///
/// Resets the download item to the Queued state with freshly-loaded settings
/// (in case the user changed quality/format options since the original attempt)
/// and triggers queue processing to start it.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings access and event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID of the failed/cancelled download to retry.
///
/// # Returns
/// * `Ok(())` - The download was reset to Queued and queue processing triggered.
/// * `Err(String)` - The download ID was not found, or the item is in a state
///   that cannot be retried (e.g., currently active or already completed).
///
/// # Errors
/// Returns an error if the download ID was not found or the item is not in a
/// retryable state (only Failed and Cancelled items can be retried).
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful re-queue.
#[tauri::command]
pub async fn retry_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry requested for download: {download_id}");

    // Re-load settings so retries pick up any changes the user made
    // (e.g., switching from AAC to ALAC after a failed attempt).
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Smart retry preview (#667). Peek at what the planner would produce
    // BEFORE mutating queue state, so a "nothing to retry — every track
    // is already on disk" outcome surfaces as a friendly Err message
    // instead of the generic `cannot be retried`. The peek is read-only
    // and doesn't change item state.
    let smart_preview = {
        let q = queue.lock().await;
        q.peek_smart_retry_outcome(&download_id)
    };
    if let Some(crate::services::smart_retry_planner::PlanOutcome::AllPresent {
        total_tracks,
    }) = smart_preview
    {
        emit_app_log(
            &app,
            &format!(
                "Retry refused for [{}]: every track ({total_tracks}) is already \
                 on disk — manifest diff found nothing to re-fetch",
                &download_id[..8.min(download_id.len())],
            ),
        );
        return Err(format!(
            "Nothing to retry — all {total_tracks} track(s) are already on disk. \
             If something changed (e.g. Apple released a new mix), use Settings > \
             General > Smart Re-Download Detection."
        ));
    }

    // Attempt to reset the download item to Queued state.
    // q.retry() returns true only if the item exists and is in a retryable state
    // (Failed or Cancelled).
    let retry_urls = {
        let q = queue.lock().await;
        q.get_status()
            .into_iter()
            .find(|item| item.id == download_id)
            .map(|item| item.urls)
            .unwrap_or_default()
    };

    let retried = {
        let mut q = queue.lock().await;
        q.retry(&download_id, &settings)
    };

    if retried {
        crate::services::history_service::remove_entries_for_urls(&app, &retry_urls);

        // Persist the updated queue (retried item now Queued again)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Retrying download [{short}]"));

        // Notify frontend and kick off queue processing if auto-start is enabled.
        let _ = app.emit("download-queued", &download_id);
        if settings.auto_start_queue {
            download_queue::process_queue(app, queue_handle).await;
        }
        Ok(())
    } else {
        Err(format!("Download {download_id} cannot be retried"))
    }
}

/// Retries a failed download with wrapper authentication disabled.
///
/// **Frontend caller:** `retryDownloadWithoutWrapper(downloadId)` in `src/lib/tauri-commands.ts`
///
/// Similar to `retry_download`, but explicitly disables the wrapper
/// system and clears wrapper URLs before re-queueing. This allows
/// users to fall back to cookie-based authentication when the wrapper
/// service is down or misconfigured.
///
/// Only applies to downloads that were originally attempted with wrapper
/// enabled and are in a retryable state (Failed or Cancelled).
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings access and event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID of the failed download to retry.
///
/// # Returns
/// * `Ok(())` - The download was reset with wrapper disabled and queue processing triggered.
/// * `Err(String)` - The download ID was not found, not in a retryable state, or wasn't
///   originally attempted with wrapper enabled.
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful re-queue.
#[tauri::command]
pub async fn retry_download_without_wrapper(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry without wrapper requested for download: {download_id}");

    // Re-load settings so retries pick up any changes the user made
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Attempt to reset the download with wrapper disabled.
    // Returns true only if the item exists, is retryable, and was using wrapper.
    let retry_urls = {
        let q = queue.lock().await;
        q.get_status()
            .into_iter()
            .find(|item| item.id == download_id)
            .map(|item| item.urls)
            .unwrap_or_default()
    };

    let retried = {
        let mut q = queue.lock().await;
        q.retry_without_wrapper(&download_id, &settings)
    };

    if retried {
        crate::services::history_service::remove_entries_for_urls(&app, &retry_urls);

        // Persist the updated queue (retried item now Queued again)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Retrying [{short}] without wrapper"));

        // Notify frontend and kick off queue processing if auto-start is enabled.
        let _ = app.emit("download-queued", &download_id);
        if settings.auto_start_queue {
            download_queue::process_queue(app, queue_handle).await;
        }
        Ok(())
    } else {
        Err(format!(
            "Download {download_id} cannot be retried without wrapper (not found, not retryable, or wasn't using wrapper)"
        ))
    }
}

/// Removes a single item from the queue by ID (#685).
///
/// **Frontend caller:** `deleteQueueItem(downloadId)` in `src/lib/tauri-commands.ts`
///
/// Sibling of `cancel_download` (which transitions an active item to
/// `Cancelled` but keeps the row visible) and `clear_all_queue` (bulk).
/// This is the per-item destructive removal — typically used to purge a
/// stubbornly-failing entry from the queue without nuking the rest.
///
/// Active items (`Downloading` / `Processing`) are refused — the caller
/// must `cancel_download` first. The frontend already hides the menu entry
/// for active rows, so this is defense-in-depth against a context-menu
/// race (state changed between menu open and click).
///
/// # Events Emitted
/// * `"queue-item-deleted"` — payload: the deleted download ID. The
///   frontend can listen for this to update lists without a full refetch
///   (the existing 3 s polling fallback also catches it).
#[tauri::command]
pub async fn delete_queue_item(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Delete requested for download: {download_id}");

    let result = {
        let mut q = queue.lock().await;
        q.delete(&download_id)
    };

    match result {
        Ok(true) => {
            // Persist the updated queue (item removed). Mirrors the
            // cancel/retry/clear paths so a crash or quit can't resurrect
            // a deleted row from disk on next startup.
            let queue_handle = queue.inner().clone();
            download_queue::save_queue_to_disk(&app, &queue_handle).await;

            let short = &download_id[..8.min(download_id.len())];
            emit_app_log(&app, &format!("Removed queue item [{short}]"));

            // Best-effort frontend notification; the deletion has already
            // landed regardless of whether the event reaches the UI.
            let _ = app.emit("queue-item-deleted", &download_id);
            Ok(())
        }
        Ok(false) => Err(format!("Download {download_id} not found")),
        Err(e) => Err(e),
    }
}

/// Clears all completed, failed, and cancelled items from the queue.
///
/// **Frontend caller:** `clearQueue()` in `src/lib/tauri-commands.ts`
///
/// Removes all items whose state is Completed, Failed, or Cancelled,
/// leaving only Active and Queued items. This is typically called when
/// the user clicks "Clear Finished" in the download queue panel.
///
/// # Arguments
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(usize)` - The number of items that were removed from the queue.
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn clear_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    let removed = {
        let mut q = queue.lock().await;
        // clear_finished() drains all terminal-state items and returns the count
        q.clear_finished()
    };

    // Persist the updated queue (or clear the file if nothing remains)
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    if removed > 0 {
        emit_app_log(&app, &format!("Cleared {removed} item(s) from queue"));
    }

    Ok(removed)
}

/// Clears ALL non-active items from the queue (completed, cancelled,
/// errored, and queued). Active downloads are preserved.
///
/// **Frontend caller:** `clearAllQueue()` in `src/lib/tauri-commands.ts`
///
/// # Returns
/// * `Ok(usize)` - The number of items removed.
#[tauri::command]
pub async fn clear_all_queue(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
) -> Result<usize, String> {
    let removed = {
        let mut q = queue.lock().await;
        q.clear_all()
    };

    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    if removed > 0 {
        emit_app_log(&app, &format!("Cleared all {removed} item(s) from queue"));
    }

    Ok(removed)
}

/// Aborts every active and queued download (#620).
///
/// **Frontend caller:** `abortAllDownloads()` in `src/lib/tauri-commands.ts`.
///
/// Per-item cancellation is a click per queue row and leaves the subprocess
/// running until the next cancellation poll tick. For a user who wants to
/// halt a 50-item batch *now*, that's painful. `abort_all_downloads` is the
/// "stop everything" escape hatch:
///
/// 1. Transitions every `Queued` / `Downloading` / `Processing` item to
///    `Cancelled`.
/// 2. The already-running cancellation-poll loops on each active task see
///    the state change on their next tick and kill the subprocess (`Child`
///    already has `kill_on_drop(true)`), exiting the enrichment / companion
///    / lyrics pipelines cleanly.
/// 3. Terminal items (`Complete` / `Cancelled` / `Error`) are untouched so
///    the user keeps their history.
/// 4. The updated queue is persisted to disk and a `"downloads-aborted"`
///    event is emitted carrying the [`AbortSummary`] so the frontend can
///    show a single summary toast instead of N per-item state changes.
///
/// # Arguments
/// * `app` - Tauri handle for event emission + disk persistence.
/// * `queue` - Managed download queue state.
///
/// # Returns
/// An [`AbortSummary`] with per-pre-state counts. The frontend uses this to
/// craft the user-facing toast ("Cancelled 12 queued, stopped 1 downloading").
///
/// # Errors
/// Infallible in practice; `Result` satisfies the Tauri IPC convention.
///
/// # Events Emitted
/// * `"downloads-aborted"` — payload: [`AbortSummary`].
#[tauri::command]
pub async fn abort_all_downloads(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
) -> Result<download_queue::AbortSummary, String> {
    let summary = {
        let mut q = queue.lock().await;
        q.abort_all()
    };

    // Persist the post-abort queue so a crash or accidental quit doesn't
    // leave behind a stale "Downloading" state that would confuse the
    // startup recovery path.
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    if summary.total() > 0 {
        emit_app_log(
            &app,
            &format!(
                "Aborted queue — cancelled {q} queued, stopped {d} downloading, \
                 stopped {p} processing",
                q = summary.queued_cancelled,
                d = summary.downloading_stopped,
                p = summary.processing_stopped,
            ),
        );
        // Event emission is best-effort; the state change already landed,
        // so a failed emit is non-critical.
        let _ = app.emit("downloads-aborted", &summary);
    }

    Ok(summary)
}

/// Returns the current status of all items in the download queue.
///
/// **Frontend caller:** `getQueueStatus()` in `src/lib/tauri-commands.ts`
///
/// Used by the frontend to render the download queue UI with progress
/// bars, status indicators, and action buttons. The frontend typically
/// polls this command on an interval (or after receiving a download event)
/// to keep the UI synchronized with the backend state.
///
/// # Arguments
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(QueueStatus)` - Aggregated counts plus per-item status details.
///   This struct is serialized to JSON by Tauri's IPC layer.
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn get_queue_status(queue: State<'_, QueueHandle>) -> Result<QueueStatus, String> {
    let q = queue.lock().await;
    // get_counts() returns a tuple of (total, active, queued, completed, failed)
    let (total, active, queued, completed, failed) = q.get_counts();
    // get_status() returns a Vec<QueueItemStatus> with per-item details
    let items = q.get_status();
    // Release the queue lock as soon as possible to avoid holding it while
    // assembling the response struct (reduces resource contention).
    drop(q);

    // Assemble and return the complete queue snapshot
    Ok(QueueStatus {
        total,
        active,
        queued,
        completed,
        failed,
        items,
    })
}

/// Checks the latest GAMDL version available on `PyPI`.
///
/// **Frontend caller:** `checkGamdlUpdate()` in `src/lib/tauri-commands.ts`
///
/// Used by the update checker to notify the user when a new GAMDL
/// version is available. Queries the `PyPI` JSON API at:
///   <https://pypi.org/pypi/gamdl/json>
///
/// This command takes no parameters because it only needs network access.
/// It does not require the `AppHandle` or `State` since it doesn't access
/// any local state or managed resources.
///
/// # Returns
/// * `Ok(String)` - The latest version string (e.g., "2.8.4").
/// * `Err(String)` - Network error or `PyPI` API parsing failure.
///
/// # Errors
/// Returns an error if the HTTP request to `PyPI` fails (network timeout,
/// DNS failure) or if the JSON response cannot be parsed.
#[tauri::command]
pub async fn check_gamdl_update() -> Result<String, String> {
    // Delegates to the gamdl_service which handles the HTTP request and
    // JSON parsing of the PyPI API response.
    crate::services::gamdl_service::check_latest_gamdl_version().await
}

/// Exports the current download queue to a `.meedyadl` file.
///
/// **Frontend caller:** `exportQueue()` in `src/lib/tauri-commands.ts`
///
/// Opens a native "Save As" dialog with the `.meedyadl` file filter.
/// Only non-terminal items (Queued/Downloading/Processing) are exported.
/// The exported file is a JSON document with the `QueueExportFile` schema.
///
/// # Returns
/// * `Ok(usize)` - The number of items exported.
/// * `Err(String)` - No items to export, dialog cancelled, or write error.
///
/// # Errors
/// Returns an error if:
/// - The queue has no exportable (non-terminal) items.
/// - The user cancels the native save dialog.
/// - JSON serialization fails.
/// - The file system write fails (permissions, disk full, etc.).
/// - The selected file path cannot be resolved.
#[tauri::command]
pub async fn export_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    // Import DialogExt at the top of the function body to satisfy
    // clippy::items_after_statements (items must precede statements).
    use tauri_plugin_dialog::DialogExt;

    // Get exportable items (non-terminal)
    let items = {
        let q = queue.lock().await;
        q.get_exportable_items()
    };

    if items.is_empty() {
        return Err("No items to export".to_string());
    }

    let count = items.len();

    // Build the export file structure
    let export_file = download_queue::QueueExportFile {
        version: 1,
        app: "MeedyaDL".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        items,
    };

    // Serialize to pretty-printed JSON
    let json = serde_json::to_string_pretty(&export_file)
        .map_err(|e| format!("Failed to serialize queue: {e}"))?;

    // Open a native save dialog with the .meedyadl file filter
    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Queue", &["meedyadl"])
        .set_file_name("queue.meedyadl")
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, &json)
                .map_err(|e| format!("Failed to write export file: {e}"))?;
            let filename = resolved
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("queue.meedyadl");
            log::info!("Exported {count} queue item(s) to file");
            emit_app_log(&app, &format!("Exported {count} item(s) to {filename}"));
            Ok(count)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Imports download queue items from a `.meedyadl` file.
///
/// **Frontend caller:** `importQueue()` in `src/lib/tauri-commands.ts`
///
/// Opens a native file picker dialog with the `.meedyadl` file filter.
/// Imported items are enqueued as new downloads and queue processing is
/// started. The queue is persisted to disk after import.
///
/// # Returns
/// * `Ok(usize)` - The number of items imported.
/// * `Err(String)` - Dialog cancelled, invalid file, or parse error.
///
/// # Errors
/// Returns an error if:
/// - The user cancels the native file picker dialog.
/// - The selected file path cannot be resolved.
/// - The file cannot be read (permissions, not found, etc.).
/// - The file contents are not valid `QueueExportFile` JSON.
/// - The schema version is not 1 (unsupported format).
/// - The file contains no items.
#[tauri::command]
pub async fn import_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    // Open a native file picker with the .meedyadl file filter
    use tauri_plugin_dialog::DialogExt;
    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Queue", &["meedyadl"])
        .blocking_pick_file();

    let Some(path) = file_path else {
        return Err("Import cancelled".to_string());
    };

    // Read and parse the export file
    let resolved = path
        .as_path()
        .ok_or_else(|| "Failed to resolve import file path".to_string())?;
    let json = std::fs::read_to_string(resolved)
        .map_err(|e| format!("Failed to read import file: {e}"))?;

    let export_file: download_queue::QueueExportFile =
        serde_json::from_str(&json).map_err(|e| format!("Invalid queue file format: {e}"))?;

    // Validate schema version
    if export_file.version != 1 {
        return Err(format!(
            "Unsupported queue file version: {} (expected 1)",
            export_file.version
        ));
    }

    if export_file.items.is_empty() {
        return Err("Queue file contains no items".to_string());
    }

    // Load current settings for option merging on the importing device
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Normalize non-geographic URLs in imported items (same as start_download).
    let items: Vec<_> = export_file
        .items
        .into_iter()
        .map(|mut item| {
            item.urls = item
                .urls
                .into_iter()
                .map(|url| crate::services::apple_music_api::normalize_apple_music_url(&url))
                .collect();
            item
        })
        .collect();

    // Import items into the queue. The lock is acquired inline and
    // released immediately after import_items() returns, avoiding
    // unnecessary resource contention (clippy::significant_drop_tightening).
    let count = queue.lock().await.import_items(items, &settings).len();

    // Persist the updated queue
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    // Notify the frontend that items were imported
    let _ = app.emit("queue-imported", count);

    let filename = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("queue file");
    log::info!("Imported {count} queue item(s) from file");
    emit_app_log(&app, &format!("Imported {count} item(s) from {filename}"));

    // Start processing the imported items if auto-start is enabled.
    if settings.auto_start_queue {
        download_queue::process_queue(app, queue_handle).await;
    }

    Ok(count)
}

/// Manually triggers download queue processing.
///
/// **Frontend caller:** `processQueue()` in `src/lib/tauri-commands.ts`
///
/// Used when `auto_start_queue` is disabled and the user wants to start
/// processing queued items from the Queue page. Also useful in auto mode
/// if processing stalled for any reason.
///
/// This is a no-op if no items are in the Queued state or if the
/// concurrency limit is already reached.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for subprocess management and event emission.
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(())` - Queue processing was triggered successfully.
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn process_queue_manual(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
) -> Result<(), String> {
    log::info!("Manual queue processing triggered");
    emit_app_log(&app, "Queue processing started (manual)");
    let queue_handle = queue.inner().clone();
    download_queue::process_queue(app, queue_handle).await;
    Ok(())
}

/// Entry payload for activity log export (received from frontend).
///
/// The activity log entries live in the frontend Zustand store
/// (`activityStore.ts`) and are passed to this command for formatting
/// and writing to a file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActivityLogExportEntry {
    /// Unique download ID this log line belongs to
    pub download_id: String,
    /// Output stream: "stdout" or "stderr"
    pub stream: String,
    /// Raw line content from GAMDL subprocess
    pub line: String,
    /// ISO 8601 timestamp (UTC) when the line was emitted
    pub timestamp: String,
}

/// Formats an ISO 8601 timestamp to a short HH:MM:SS string for the export file.
fn format_timestamp_short(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map_or_else(|_| iso.to_string(), |dt| dt.format("%H:%M:%S").to_string())
}

/// Exports activity log entries to a `.log` file via a native save dialog.
///
/// **Frontend caller:** `exportActivityLog(entries)` in `src/lib/tauri-commands.ts`
///
/// Activity log entries are maintained in the frontend Zustand store and
/// passed to this command for formatting and writing. Each entry is
/// formatted as: `[HH:MM:SS] [download_id_prefix] [stream] line_content`
///
/// # Arguments
/// * `app` - The Tauri app handle (for dialog access)
/// * `entries` - Vec of activity log entries from the frontend store
///
/// # Returns
/// * `Ok(usize)` - Number of lines exported
/// * `Err(String)` if the dialog is cancelled, serialization fails, or I/O fails
#[tauri::command]
pub async fn export_activity_log(
    app: AppHandle,
    entries: Vec<ActivityLogExportEntry>,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    if entries.is_empty() {
        return Err("No log entries to export".to_string());
    }

    let count = entries.len();

    // Format entries as human-readable text lines
    let mut lines = Vec::with_capacity(count + 3);
    lines.push(format!(
        "MeedyaDL Activity Log -- Exported {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!("{count} entries"));
    lines.push(String::new()); // blank separator

    for entry in &entries {
        let time = format_timestamp_short(&entry.timestamp);
        let id_prefix = if entry.download_id == "system" {
            "System"
        } else if entry.download_id.len() > 8 {
            &entry.download_id[..8]
        } else {
            &entry.download_id
        };
        lines.push(format!(
            "[{}] [{}] [{}] {}",
            time, id_prefix, entry.stream, entry.line
        ));
    }

    let text = lines.join("\n");

    // Open native save dialog with .log filter.
    // Filename includes the current date/time for easy identification.
    let default_name = chrono::Local::now()
        .format("MeedyaDL-activity-log_%Y-%m-%d_%Hh%Mm.log")
        .to_string();
    let file_path = app
        .dialog()
        .file()
        .add_filter("Log File", &["log"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, text).map_err(|e| format!("Failed to write log file: {e}"))?;
            log::info!("Exported {count} activity log entries to file");
            Ok(count)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Imports a `.meedyadl` manifest file via a native open dialog.
///
/// **Frontend caller:** `importManifest()` in `src/lib/tauri-commands.ts`
///
/// Parses the manifest and returns the source URLs for the frontend
/// to populate the download form. Supports both single-source and
/// multi-platform manifests (returns all source URLs).
///
/// # Returns
/// * `Ok(Vec<String>)` - List of download URLs from the manifest sources
/// * `Err(String)` if the dialog is cancelled, file is invalid, or I/O fails
#[tauri::command]
pub async fn import_manifest(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Manifest", &["meedyadl"])
        .blocking_pick_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve manifest file path".to_string())?;
            let contents = std::fs::read_to_string(resolved)
                .map_err(|e| format!("Failed to read manifest file: {e}"))?;
            let manifest: crate::models::manifest::ManifestFile =
                serde_json::from_str(&contents)
                    .map_err(|e| format!("Invalid manifest file: {e}"))?;

            let urls: Vec<String> = manifest
                .sources
                .iter()
                .map(|s| s.url.clone())
                .collect();

            if urls.is_empty() {
                return Err("Manifest contains no download sources".to_string());
            }

            log::info!(
                "Imported manifest with {} source(s)",
                urls.len()
            );
            Ok(urls)
        }
        None => Err("Import cancelled".to_string()),
    }
}

/// Result of scanning a folder for manifest files (#456).
///
/// Each entry represents one discovered `manifest.meedyadl` file with
/// metadata extracted for display in the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannedManifest {
    /// Path to the manifest file on disk
    pub manifest_path: String,
    /// Album directory containing the manifest
    pub album_dir: String,
    /// Source URLs extracted from the manifest
    pub urls: Vec<String>,
    /// Platform (e.g., "apple-music")
    pub platform: Option<String>,
    /// Artist name (from album directory structure)
    pub artist: Option<String>,
    /// Album name (from album directory name)
    pub album: Option<String>,
    /// When this source was last downloaded
    pub downloaded_at: Option<String>,
    /// Track count from the manifest
    pub track_count: usize,
    /// Current codec detected from files on disk (e.g., "aac", "alac") (#380)
    pub current_codec: Option<String>,
    /// Number of audio files in the album directory
    pub audio_file_count: usize,
    /// Apple Music lastModifiedDate from the manifest — used for content
    /// refresh detection (#380). If the current API response has a newer
    /// date, the album may have been updated (mix corrections, remasters,
    /// added tracks, Apple Digital Master certification).
    pub last_modified_date: Option<String>,
}

/// Recursively scan a directory for `manifest.meedyadl` files and return
/// metadata from each discovered manifest (#456).
///
/// Opens a native folder picker dialog, then walks the selected directory
/// tree looking for `manifest.meedyadl` (and legacy `.meedyadl`) files.
/// For each found manifest, extracts the source URLs, platform, download
/// date, and track count. Also infers artist/album names from the directory
/// structure (GAMDL's `Artist/Album/` convention).
///
/// Used by the "Re-download from Folder" feature to let users point at an
/// existing music library and re-queue downloads for metadata correction
/// or quality upgrades.
///
/// # Returns
/// * `Ok(Vec<ScannedManifest>)` — Manifests found (may be empty)
/// * `Err(String)` — Folder picker cancelled or I/O error
#[tauri::command]
pub async fn scan_folder_for_manifests(
    app: AppHandle,
) -> Result<Vec<ScannedManifest>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .blocking_pick_folder();

    let folder_path = match folder {
        Some(path) => {
            path.as_path()
                .ok_or_else(|| "Failed to resolve folder path".to_string())?
                .to_path_buf()
        }
        None => return Err("Folder selection cancelled".to_string()),
    };

    log::info!(
        "Scanning folder for manifests: {}",
        folder_path.display()
    );

    let results = scan_for_manifests(&folder_path);

    log::info!(
        "Found {} manifest(s) in {}",
        results.len(),
        folder_path.display()
    );

    crate::utils::activity_log::emit_app_log(
        &app,
        &format!(
            "Folder scan: found {} manifest(s) in {}",
            results.len(),
            folder_path.display()
        ),
    );

    Ok(results)
}

/// Recursively scan a folder for `manifest.meedyadl` (or legacy `.meedyadl`)
/// files, parse each into a [`ScannedManifest`], and return the collection
/// for the Library Scan UI.
///
/// Migrated to [`walk_dir_depth`] in #716 finding #1 — previously this was a
/// hand-rolled recursive walker (`scan_dir_for_manifests_recursive`) that
/// duplicated the depth-limited `read_dir` boilerplate present in 5+ other
/// callsites. The 10-level depth cap is preserved (matches the documented
/// max for library scans — see `walk_dir_depth` doc on `max_depth = 10`).
///
/// Visitor returns `None` for any entry that isn't a parseable manifest with
/// at least one source URL — this preserves the original "skip empty-source
/// manifests entirely" behaviour without introducing a sentinel variant.
fn scan_for_manifests(base: &std::path::Path) -> Vec<ScannedManifest> {
    crate::utils::fs_walk::walk_dir_depth(base, 10, parse_manifest_at_path)
}

/// Visitor for [`scan_for_manifests`]: turns a single filesystem entry into
/// `Some(ScannedManifest)` if it's a parseable manifest with at least one
/// source URL, `None` otherwise. Pulled out as a free function so the
/// closure handed to `walk_dir_depth` stays a one-liner.
fn parse_manifest_at_path(path: &std::path::Path) -> Option<ScannedManifest> {
    if !path.is_file() {
        return None;
    }
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name != "manifest.meedyadl" && file_name != ".meedyadl" {
        return None;
    }

    let contents = std::fs::read_to_string(path)
        .inspect_err(|_| log::debug!("Failed to read manifest: {}", path.display()))
        .ok()?;

    let manifest =
        serde_json::from_str::<crate::models::manifest::ManifestFile>(&contents)
            .inspect_err(|_| log::debug!("Failed to parse manifest: {}", path.display()))
            .ok()?;

    let source = manifest.sources.first();

    // Infer artist/album from directory structure:
    // base/Artist/Album/manifest.meedyadl → parent = Album, grandparent = Artist
    let album_dir = path.parent()?;
    let album_name = album_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(String::from);
    let artist_name = album_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(String::from);

    let urls: Vec<String> = manifest.sources.iter().map(|s| s.url.clone()).collect();
    if urls.is_empty() {
        return None;
    }

    let track_count = source.map(|s| s.tracks.len()).unwrap_or(0);
    let (current_codec, audio_file_count) = detect_album_codec(album_dir);

    Some(ScannedManifest {
        manifest_path: path.to_string_lossy().to_string(),
        album_dir: album_dir.to_string_lossy().to_string(),
        urls,
        platform: source.map(|s| s.platform.clone()),
        artist: artist_name,
        album: album_name,
        downloaded_at: source.map(|s| s.downloaded_at.clone()),
        track_count,
        current_codec,
        audio_file_count,
        last_modified_date: source.and_then(|s| s.last_modified_date.clone()),
    })
}

/// Checks whether a URL was previously downloaded and returns change
/// Detect the codec of the first M4A file in an album directory (#380).
///
/// Reads the `MeedyaMeta:SourceCodec` or `com.apple.iTunes:isLossless` tag
/// from the first `.m4a` file found. Returns `(codec_name, audio_file_count)`.
fn detect_album_codec(album_dir: &std::path::Path) -> (Option<String>, usize) {
    let mut count = 0;
    let mut codec: Option<String> = None;

    let Ok(entries) = std::fs::read_dir(album_dir) else {
        return (None, 0);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("m4a") && !ext.eq_ignore_ascii_case("m4v") {
            continue;
        }
        count += 1;

        // Only detect codec from the first file
        if codec.is_some() {
            continue;
        }

        if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
            // Try MeedyaMeta:SourceCodec first (written by MeedyaDL enrichment)
            let source_codec = tag
                .strings_of(&mp4ameta::FreeformIdent::new_static(
                    "MeedyaMeta",
                    "SourceCodec",
                ))
                .next()
                .map(String::from);

            if let Some(c) = source_codec {
                codec = Some(c);
            } else {
                // Fallback: check isLossless tag
                let is_lossless = tag
                    .strings_of(&mp4ameta::FreeformIdent::new_static(
                        "com.apple.iTunes",
                        "isLossless",
                    ))
                    .next()
                    .map(|s| s.to_string());

                codec = match is_lossless.as_deref() {
                    Some("true") => Some("alac".to_string()),
                    Some("false") => Some("aac".to_string()),
                    _ => None,
                };
            }
        }
    }

    (codec, count)
}

/// detection metadata for smart re-download detection (#263).
///
/// Looks up the URL in download history. If found, returns the download
/// date and title. The frontend uses this to inform the user before
/// re-downloading. The actual `lastModifiedDate` comparison happens
/// after the API metadata is fetched during enrichment.
///
/// # Returns
/// * `Ok(Some(info))` - URL was previously downloaded; includes date + title
/// * `Ok(None)` - URL not in download history (first download)
/// * `Err` - History service error
#[tauri::command]
pub async fn check_redownload_status(
    app: AppHandle,
    url: String,
) -> Result<Option<RedownloadInfo>, String> {
    let history = crate::services::history_service::list_history(&app);
    let previous = history.iter().find(|e| e.url == url && e.status == "success");

    Ok(previous.map(|entry| RedownloadInfo {
        downloaded_at: entry.completed_at.clone(),
        title: entry.title.clone(),
        album: entry.album.clone(),
    }))
}

/// Result of a Library Scan smart-retry diff (Phase 5b, #717).
///
/// Mirrors the three [`crate::services::smart_retry_planner::PlanOutcome`]
/// variants but uses a TypeScript-friendly tagged-union shape so the
/// Library Scan UI can render "X of Y missing" / "All present" /
/// "Cannot diff" badges per row without re-implementing the variant
/// inspection on every frontend re-render.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryScanDiff {
    /// Manifest produced a precise per-track URL list — at least one
    /// track is missing.
    Plan {
        missing_tracks: usize,
        total_tracks: usize,
    },
    /// Manifest was found and every track is on disk. No retry needed.
    AllPresent { total_tracks: usize },
    /// Manifest missing, malformed, or no source matched. UI should
    /// render a neutral "Cannot diff" badge.
    NotApplicable,
}

/// Diffs a single scanned manifest against its on-disk state to
/// determine whether tracks are missing (Phase 5b of #717 Library
/// Scan UX).
///
/// Wraps `services::smart_retry_planner::plan_retry`. The Library
/// Scan page calls this once per `ScannedManifest` to populate per-row
/// "missing tracks" badges. Cheap (filesystem ops only — no network),
/// safe to run synchronously per row from the frontend.
///
/// # Arguments
/// * `album_dir` - The album directory path (the parent of the
///   `manifest.meedyadl` file). Comes directly from
///   [`ScannedManifest::album_dir`].
/// * `source_url` - The manifest source URL to diff against. Comes
///   from `ScannedManifest::urls[0]`. Manifests can carry multiple
///   sources (Apple Music + Spotify of the same album); the URL
///   selects which one to plan against.
#[tauri::command]
pub async fn diff_library_scan_manifest(
    album_dir: String,
    source_url: String,
) -> Result<LibraryScanDiff, String> {
    use crate::services::smart_retry_planner::{plan_retry, PlanOutcome};
    let outcome = plan_retry(std::path::Path::new(&album_dir), &source_url);
    Ok(match outcome {
        PlanOutcome::Plan(plan) => LibraryScanDiff::Plan {
            missing_tracks: plan.missing_tracks,
            total_tracks: plan.total_tracks,
        },
        PlanOutcome::AllPresent { total_tracks } => {
            LibraryScanDiff::AllPresent { total_tracks }
        }
        PlanOutcome::NotApplicable => LibraryScanDiff::NotApplicable,
    })
}

/// Apple Music `lastModifiedDate` freshness result for a single
/// Library Scan row (Phase 5c, #717).
///
/// `manifest_date` was recorded at the time of the original download
/// (stored in `manifest.meedyadl`'s `last_modified_date` field). The
/// IPC fetches the current value via the Apple Music Catalog API and
/// returns one of three states:
///
/// - **Fresh** — manifest's date matches Apple's current value;
///   nothing has changed upstream since the user downloaded.
/// - **Updated** — Apple now reports a newer `lastModifiedDate`. The
///   album may have gained tracks, swapped a mix (Atmos added, mix
///   correction), or earned the Apple Digital Master certification.
///   UI surfaces a "Content updated" badge so the user can choose to
///   re-download.
/// - **Unknown** — MusicKit credentials missing, network failure, the
///   URL parser couldn't extract an album ID, or the manifest had no
///   recorded date to compare against. UI shows nothing (this is the
///   expected resting state for users without credentials).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryScanFreshness {
    /// Apple's current `lastModifiedDate` matches the manifest.
    Fresh { current_date: String },
    /// Apple now reports a newer `lastModifiedDate` than the manifest.
    Updated {
        manifest_date: String,
        current_date: String,
    },
    /// Compare wasn't possible — credentials missing, API error, etc.
    /// Carries a short reason for the activity log / diagnostics.
    Unknown { reason: String },
}

/// Phase 5c (#717): per-row Apple Music `lastModifiedDate` freshness
/// check for the Library Scan page.
///
/// Hits the Apple Music Catalog API for the URL's album ID and
/// compares the API's `lastModifiedDate` against the manifest-recorded
/// value (`ScannedManifest::last_modified_date` from the v1.0.1 scan).
///
/// **Cost**: one HTTP request per call. The frontend rate-limits the
/// dispatch to keep large libraries from saturating the API; see
/// `LibraryScanPage.tsx`.
///
/// Returns `Unknown` rather than `Err` for the typical "user has no
/// MusicKit credentials" case — this is the expected resting state
/// for most users and shouldn't surface as an error toast.
#[tauri::command]
pub async fn check_library_scan_freshness(
    app: AppHandle,
    source_url: String,
    manifest_date: Option<String>,
) -> Result<LibraryScanFreshness, String> {
    use crate::services::apple_music_api::{
        fetch_album_metadata_with_fallback, get_private_key_from_keychain,
        parse_apple_music_url, resolve_musickit_developer_token,
    };

    let Some(manifest_date) = manifest_date.filter(|s| !s.is_empty()) else {
        return Ok(LibraryScanFreshness::Unknown {
            reason: "manifest has no recorded lastModifiedDate".to_string(),
        });
    };

    let Some(parsed) = parse_apple_music_url(&source_url) else {
        return Ok(LibraryScanFreshness::Unknown {
            reason: "URL is not a recognised Apple Music album link".to_string(),
        });
    };

    // We need an album ID to query the Catalog API; song / playlist
    // / library URLs don't carry one and aren't supported here.
    let album_id = parsed.album_id.clone();
    let storefront = parsed.storefront.clone();

    // Resolve MusicKit credentials. The team/key IDs live in
    // settings.json; the private key PEM is stored in the OS
    // keychain. Most users won't have these configured — that's
    // fine, return `Unknown` cleanly so the UI can render no badge
    // and not toast an error.
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();
    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match get_private_key_from_keychain() {
        Ok(pk) => pk,
        Err(_) => {
            return Ok(LibraryScanFreshness::Unknown {
                reason: "MusicKit private key keychain inaccessible".to_string(),
            });
        }
    };
    let jwt = match resolve_musickit_developer_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(Some(jwt)) => jwt,
        Ok(None) => {
            return Ok(LibraryScanFreshness::Unknown {
                reason: "MusicKit credentials not configured".to_string(),
            });
        }
        Err(_) => {
            return Ok(LibraryScanFreshness::Unknown {
                reason: "MusicKit credentials present but invalid".to_string(),
            });
        }
    };

    // Fetch current metadata from Apple's Catalog API.
    match fetch_album_metadata_with_fallback(&jwt, &storefront, &album_id).await {
        Ok(Some(metadata)) => match metadata.last_modified_date {
            Some(current_date) => {
                if current_date == manifest_date {
                    Ok(LibraryScanFreshness::Fresh { current_date })
                } else {
                    Ok(LibraryScanFreshness::Updated {
                        manifest_date,
                        current_date,
                    })
                }
            }
            None => Ok(LibraryScanFreshness::Unknown {
                reason: "Apple Music API returned no lastModifiedDate".to_string(),
            }),
        },
        Ok(None) => Ok(LibraryScanFreshness::Unknown {
            reason: "album not found in any storefront".to_string(),
        }),
        Err(e) => Ok(LibraryScanFreshness::Unknown {
            reason: format!("API error: {e}"),
        }),
    }
}

/// Fetches syllable-level (word-by-word) TTML lyrics for a single song from
/// the Apple Music API.
///
/// **Frontend caller:** `fetchSyllableLyrics(storefront, songId)` in `src/lib/tauri-commands.ts`
///
/// Requires MusicKit credentials (Team ID, Key ID, private key) and a valid
/// `media-user-token` cookie for subscriber authentication. Returns the raw
/// TTML XML string if available, or `None` if the song has no syllable-level
/// lyrics.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings and keychain access.
/// * `storefront` - Apple Music storefront code (e.g., "us", "gb").
/// * `song_id` - Numeric Apple Music song ID.
///
/// # Returns
/// * `Ok(Some(String))` - TTML XML lyrics content.
/// * `Ok(None)` - No syllable-level lyrics available for this song.
/// * `Err(String)` - Credentials missing, expired cookies, or API error.
#[tauri::command]
pub async fn fetch_syllable_lyrics(
    app: AppHandle,
    storefront: String,
    song_id: String,
) -> Result<Option<String>, String> {
    use crate::services::{apple_music_api, config_service};

    // Resolve MusicKit credentials and generate JWT
    let settings = config_service::load_settings(&app).unwrap_or_default();
    let private_key = apple_music_api::get_private_key_from_keychain()
        .map_err(|e| format!("Keychain error: {e}"))?;
    let jwt = apple_music_api::resolve_musickit_developer_token(
        settings.musickit_team_id.as_deref(),
        settings.musickit_key_id.as_deref(),
        private_key.as_deref(),
    )
    .map_err(|e| format!("JWT error: {e}"))?
    .ok_or(
        "MusicKit credentials not configured. Set up Team ID, Key ID, and private key in Settings > Advanced > API Credentials."
    )?;

    // Extract media-user-token from cookies file
    let cookies_path = settings.cookies_path.as_deref().ok_or(
        "Apple Music cookies not configured. Import cookies from your browser in Settings > Authentication.",
    )?;
    let music_user_token = apple_music_api::extract_media_user_token(cookies_path)
        .map_err(|e| format!("Cookie error: {e}"))?
        .ok_or(
            "Apple Music subscriber token not found or expired. Re-import cookies from your browser in Settings > Authentication."
        )?;

    // Fetch syllable lyrics from the Apple Music API
    apple_music_api::fetch_syllable_lyrics(&jwt, &storefront, &song_id, &music_user_token).await
}

/// Information about a previous download of the same URL.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RedownloadInfo {
    /// When the URL was last downloaded (ISO 8601).
    pub downloaded_at: String,
    /// Track/album title from the previous download.
    pub title: Option<String>,
    /// Album name from the previous download.
    pub album: Option<String>,
}
