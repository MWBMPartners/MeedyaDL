// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Queue persistence: settings snapshot, save/load/clear of queue.json.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;

/// Loads the current app settings for use during queue processing decisions.
///
/// This is called during the error handling path of `process_queue()` to
/// access the fallback chain configuration. It uses `config_service::load_settings()`
/// rather than cached settings to ensure the latest user preferences are used
/// (the user might change settings while downloads are running).
///
/// Returns `AppSettings::default()` on load failure to avoid blocking queue processing.
///
/// Post-#690: reads from the `SettingsCache` Tauri-managed state when
/// available (lazy-populated on first access, refreshed by the
/// `save_settings` IPC after each write). On cache miss — or when the
/// cache isn't registered at all (test contexts that don't set up the
/// full app state) — falls through to the original disk-read path so
/// the function stays correct in every caller environment.
pub(crate) fn load_settings_for_queue(app: &AppHandle) -> AppSettings {
    use tauri::Manager as _;
    // Fast path: read from the cache when registered.
    if let Some(cache) = app.try_state::<super::settings_cache::SettingsCache>() {
        return cache.get_or_load(app);
    }

    // Fallback path (test contexts + the rare boot-order edge case
    // where a queue task fires before AppState is fully managed):
    // load directly from disk with the same default-on-error shape
    // as pre-#690.
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
pub(crate) fn settings_snapshot_for_context(app: &AppHandle) -> std::collections::HashMap<String, String> {
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
pub(crate) async fn save_queue_to_disk_inner(app: &AppHandle, queue: &QueueHandle) {
    // Clone persistable items and get counts while holding the lock
    let (items, active, queued, completed) = {
        let q = queue.lock().await;
        let (_, active, queued, completed, _) = q.get_counts();
        (q.get_persistable_items(), active, queued, completed)
    };

    // Update the system tray tooltip with current queue status
    crate::update_tray_tooltip(app, active, queued, completed);

    // Write to disk after releasing the lock. Atomic via the shared
    // `utils::atomic_write::atomic_write_json` helper (#716 finding #8).
    // Atomicity guarantee inherited from `std::fs::rename` per #230.
    // Errors are warn-logged (not propagated) — same fail-quiet
    // behaviour as the pre-migration site since queue.json corruption
    // is recoverable from the in-memory state on next save.
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    if let Err(e) =
        crate::utils::atomic_write::atomic_write_json(&queue_path, &items, "queue")
    {
        log::warn!("{e}");
    }

    // Restrict queue.json to owner-only read/write on Unix — queue items
    // can embed the configured output path and, indirectly, wrapper
    // endpoint details, so it shouldn't be world-readable (mirrors the
    // settings.json 0600 hardening, #459).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) =
            std::fs::set_permissions(&queue_path, std::fs::Permissions::from_mode(0o600))
        {
            log::debug!("Failed to set queue.json permissions: {e}");
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
                let original_len = items.len();
                let items = dedupe_persisted_queue_items(items);
                if items.len() != original_len {
                    log::info!(
                        "Cleaned up {} duplicate persisted queue item(s)",
                        original_len - items.len()
                    );
                    match serde_json::to_string_pretty(&items) {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&queue_path, json) {
                                log::warn!("Failed to write cleaned queue.json: {e}");
                            }
                        }
                        Err(e) => log::warn!("Failed to serialize cleaned queue: {e}"),
                    }
                }
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

pub(crate) fn dedupe_persisted_queue_items(items: Vec<PersistedQueueItem>) -> Vec<PersistedQueueItem> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(items.len());

    for item in items.into_iter().rev() {
        let urls: Vec<String> = item
            .request
            .urls
            .iter()
            .map(|url| normalize_url_for_dedup(url))
            .collect();
        if urls.iter().any(|url| seen.contains(url)) {
            continue;
        }
        seen.extend(urls);
        deduped.push(item);
    }

    deduped.reverse();
    deduped
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
