// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// The queue processing pump: process_queue, download execution, and Spotify dispatch.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;

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

                // Tell the user we're about to verify the prerequisites.
                // The old wording ("Pre-flight checks passed") was flagged
                // in #578 as jargon — even dev-literate users couldn't tell
                // whether something had broken or succeeded. Emitting a
                // user-facing "Checking..." line before the result gives
                // the activity log a clear question-and-answer shape.
                emit_app_log(
                    &app,
                    "Checking internet connection, output folder, and account...",
                );

                // Run internet + wrapper checks concurrently (both are HTTP GETs).
                // This is a once-per-batch, queue-wide check (not tied to a
                // single item), and a batch can mix services — so unlike the
                // per-download `check_internet_before_download` IPC command
                // (A1), there's no single unambiguous URL to detect a service
                // from here. Passing `None` preserves the pre-A1 behaviour:
                // Tier 2 always probes the Apple Music API for this
                // queue-wide check.
                let internet_future =
                    crate::services::health_check_service::check_internet_connectivity(None);
                // #853: pick wrapper-v1 vs wrapper-v2 preflights based on the
                // detected GAMDL release. v1 needs the three sockets (account
                // HTTP + m3u8 TCP + decrypt TCP); v2 (≥ 3.6) needs the single
                // HTTP /health endpoint + an authenticated session
                // verification via /me.
                use crate::services::gamdl_capabilities::{supports, GamdlFeature};
                let use_wrapper_v2 = settings.use_wrapper && supports(GamdlFeature::WrapperUrl);
                let use_wrapper_v1 = settings.use_wrapper && !supports(GamdlFeature::WrapperUrl);

                // -- wrapper-v1 (GAMDL ≤ 3.5.x) --
                let wrapper_future = if use_wrapper_v1 {
                    Some(crate::services::health_check_service::check_wrapper_health(
                        &settings.wrapper_account_url,
                    ))
                } else {
                    None
                };
                // GAMDL v3.1+ fetches HLS URLs from the wrapper's m3u8 socket
                // when `--use-wrapper` is set. Skip the probe on older
                // releases (flag is ignored there) and on v3.6+ (replaced by
                // wrapper-v2 single endpoint).
                let wrapper_m3u8_future = if use_wrapper_v1
                    && supports(GamdlFeature::WrapperM3u8Ip)
                {
                    Some(
                        crate::services::health_check_service::check_wrapper_m3u8_health(
                            &settings.wrapper_m3u8_ip,
                        ),
                    )
                } else {
                    None
                };
                // Wrapper decryption socket reachability (#743). Needed for
                // every wrapper-v1 release. Cookie downloads still skip the
                // probe (no decrypt socket consulted). wrapper-v2 had no
                // equivalent TCP probe through GAMDL 3.8.1 — decryption was
                // bundled into the single HTTP endpoint — but GAMDL 3.8.2
                // (`GamdlFeature::WrapperDecryptHostPort`) split decryption
                // back out onto its own native TCP host/port pair, so the
                // same probe now also applies to wrapper-v2 on 3.8.2+.
                let wrapper_decrypt_future = if use_wrapper_v1
                    || (use_wrapper_v2 && supports(GamdlFeature::WrapperDecryptHostPort))
                {
                    Some(
                        crate::services::health_check_service::check_wrapper_decrypt_health(
                            &settings.wrapper_decrypt_ip,
                        ),
                    )
                } else {
                    None
                };

                // -- wrapper-v2 (GAMDL ≥ 3.6, #853) --
                let wrapper_v2_health_future = if use_wrapper_v2 {
                    Some(crate::services::health_check_service::check_wrapper_v2_health(
                        &settings.wrapper_url,
                    ))
                } else {
                    None
                };
                let wrapper_v2_auth_future = if use_wrapper_v2 {
                    Some(crate::services::health_check_service::check_wrapper_v2_auth(
                        &settings.wrapper_url,
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
                let wrapper_m3u8_warning = match wrapper_m3u8_future {
                    Some(fut) => fut.await,
                    None => None,
                };
                let wrapper_decrypt_warning = match wrapper_decrypt_future {
                    Some(fut) => fut.await,
                    None => None,
                };
                let wrapper_v2_health_warning = match wrapper_v2_health_future {
                    Some(fut) => fut.await,
                    None => None,
                };
                let wrapper_v2_auth_warning = match wrapper_v2_auth_future {
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
                    if use_wrapper_v1 {
                        v.push((PreflightCheck::Wrapper, wrapper_warning));
                        // Only checked when GAMDL supports it; when the
                        // capability gate returned `None` above, this entry
                        // carries `None` and the loop emits a "cleared"
                        // event so any stale toast is dismissed.
                        v.push((PreflightCheck::WrapperM3u8, wrapper_m3u8_warning));
                    }
                    if use_wrapper_v2 {
                        // wrapper-v2 single-endpoint preflights (#853).
                        v.push((PreflightCheck::WrapperV2Health, wrapper_v2_health_warning));
                        v.push((PreflightCheck::WrapperV2Auth, wrapper_v2_auth_warning));
                    }
                    // Decrypt probe runs on every wrapper-v1 download
                    // regardless of GAMDL version — the decrypt flag exists
                    // in every supported wrapper-v1 release (#743) — and,
                    // from GAMDL 3.8.2+, on wrapper-v2 too now that
                    // decryption was split back out onto its own native TCP
                    // host/port (`GamdlFeature::WrapperDecryptHostPort`),
                    // closing the same #743 bug class (silently falling
                    // back to the compile-time 127.0.0.1 default on
                    // remote/LAN wrapper setups) for wrapper-v2 users.
                    // Single push site — `use_wrapper_v1` and
                    // `use_wrapper_v2` are mutually exclusive so this
                    // matches the `wrapper_decrypt_future` gate above
                    // exactly and avoids moving `wrapper_decrypt_warning`
                    // from two branches.
                    if use_wrapper_v1
                        || (use_wrapper_v2 && supports(GamdlFeature::WrapperDecryptHostPort))
                    {
                        v.push((PreflightCheck::WrapperDecrypt, wrapper_decrypt_warning));
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
                        // Warnings keep the "Pre-flight:" prefix — the word
                        // "warning" is more useful context than the technical
                        // phrase on the all-OK path, and the preflight-warning
                        // event below drives a user-visible toast regardless.
                        emit_app_log(&app, &format!("Pre-flight warning: {}", w.message));
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
                    // Plain-English all-clear message (#578). Describes what
                    // was verified and what happens next, so the user — dev
                    // or otherwise — doesn't have to infer the outcome from
                    // the phrase "Pre-flight checks passed". The explicit
                    // enumeration of the three verified prerequisites matches
                    // the "Checking..." line that fired at the start so the
                    // activity-log pair reads as a coherent question-and-
                    // answer.
                    emit_app_log(
                        &app,
                        "Ready to download — internet, output folder, and account all verified",
                    );
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
        // configured after-queue action (e.g., shutdown, close app) — unless
        // the drain was caused by a user-initiated abort (#620), in which
        // case `take_recently_aborted` returns `true` and we suppress the
        // post-queue action this time around.
        let Some((download_id, urls, mut options, item_service)) = pending else {
            let (is_idle, was_aborted) = {
                let mut q = queue.lock().await;
                (q.is_idle(), q.take_recently_aborted())
            };
            if is_idle {
                if was_aborted {
                    log::info!(
                        "Queue idle after abort — suppressing post-queue action (#620)"
                    );
                    emit_app_log(
                        &app,
                        "Queue drained by abort — skipping post-queue action",
                    );
                } else {
                    execute_after_queue_action(&app);
                }
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

        // M9-7: Look up this item's engine for the new top-of-loop
        // dispatch fork. Branching on engine (not service) is
        // forward-compatible with M10's shared `ytdlp` engine across
        // YouTube + BBC iPlayer; the synthesis agent's "service" key
        // was the right structural intervention point but the
        // adversarial-critique agent's "engine" key is the right
        // semantic key. See the M9-7 design doc.
        let item_engine: Option<String> = {
            let q = queue.lock().await;
            q.items
                .iter()
                .find(|i| i.status.id == download_id)
                .and_then(|i| i.status.engine.clone())
        };

        // M9-7: Spotify (votify) dispatch fork. This block OWNS the
        // entire lifetime of a Spotify queue item — from gate
        // re-validation through votify spawn to set_complete /
        // set_error + counter increment + best-effort manifest
        // write + queue cascade — and never falls through to the
        // GAMDL pipeline below.
        //
        // The four design choices the adversarial-critique agent
        // flagged as ship-blockers are all addressed here:
        //
        // 1. **Cancellation polling**: routed through
        //    `engine_runner::run_engine_with_queue` which holds the
        //    QueueHandle and runs a 250 ms poll loop. Cancel button
        //    actually cancels.
        // 2. **Queue progress updates**: same — `update_item_progress`
        //    fires on every parsed event so the queue row caption /
        //    progress / track counters tick during the download.
        // 3. **Partial-success detection**: post-run audio-file count
        //    is compared against `urls.len()`. Less than half landed
        //    → `set_error` instead of `set_complete`.
        // 4. **Dispatch-gate re-validation**: the four-outcome gate
        //    fires again here, not just at the IPC boundary. Closes
        //    the crash-restore loophole the critique surfaced.
        //
        // Returns (not continues) — `process_queue` is recursive,
        // not loop-bodied; the arm's spawned task cascades back via
        // `process_queue(app, queue).await` when it finishes.
        if item_engine.as_deref() == Some("votify") {
            run_spotify_dispatch_arm(
                app.clone(),
                queue.clone(),
                download_id.clone(),
                urls.clone(),
                download_started_at.clone(),
            )
            .await;
            return;
        }

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
        // Captured at this scope so the post-fetch companion-template
        // injection (#528) can read it. `None` when the early metadata
        // fetch failed (offline, API rate-limit) — companions then fall
        // through to the existing folder template, which will produce
        // the sibling-folder bug for that one item; better than blocking
        // the download on a metadata fetch.
        let early_album_content_rating: Option<String> = if is_apple_music {
            let early_metadata = super::metadata_tag_service::try_fetch_metadata(
                &app,
                &urls,
                Some((&app, &download_id)),
            )
            .await;

            let captured_rating = early_metadata
                .as_ref()
                .and_then(|m| m.content_rating.clone());

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
                    // Build the 120×120 JPEG thumbnail URL for the queue
                    // row Tier 1 album-art column (#911-2). Apple Music's
                    // artwork URL template uses `{w}` / `{h}` / `{c}` /
                    // `{f}` placeholders that need concrete values. The
                    // same template feeds the full-size cover write in
                    // `cover_art_fallback::build_artwork_url`; we duplicate
                    // the substitution here to avoid widening that
                    // helper's visibility for one caller. 120 covers a
                    // 60-pixel display at 2× retina.
                    if let Some(ref template) = meta.artwork_url_template {
                        item.status.artwork_url = Some(
                            template
                                .replace("{w}", "120")
                                .replace("{h}", "120")
                                .replace("{c}", "bb")
                                .replace("{f}", "jpg"),
                        );
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
            captured_rating
        } else {
            None
        };

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
        let mut companion_base_options = options.clone();
        let mut download_options = options;
        let settings_for_companion = load_settings_for_queue(&app);

        // Companion-folder advisory-suffix injection (#528).
        //
        // When `content_advisory_in_filenames` is enabled, the primary's
        // post-enrichment `apply_advisory_suffixes` will rename the album
        // folder from `Album/` → `Album [Explicit]/` (or `[Clean]/`).
        // GAMDL has no knowledge of that rename — a companion GAMDL run
        // spawned with the default folder template writes its files to
        // `Album/` again, leaving the user with two sibling folders.
        //
        // We pre-compute the suffix here from the album's content rating
        // (captured during the early metadata fetch a few lines above)
        // and inject it into the companion's `album_folder_template` BEFORE
        // the tokio::spawn that owns these options. The companion's GAMDL
        // run then writes directly into the post-rename folder.
        //
        // Why we predict rather than wait: the spawn block at the bottom
        // of `process_queue` owns the options by move, and the enrichment
        // task (which performs the rename) only kicks off inside that
        // spawn. Waiting for the rename to land would require a fresh
        // synchronisation primitive between two background tasks — A1
        // (predict) avoids that complexity for a quick, low-risk fix.
        //
        // Graceful degradation: if the early metadata fetch returned no
        // rating (offline, API failure) we leave the template alone and
        // the user gets the existing sibling-folder behaviour for that
        // one item. No worse than today.
        if settings_for_companion.content_advisory_in_filenames {
            if let Some(rating) = early_album_content_rating.as_deref() {
                if let Some(suffix) =
                    super::metadata_tag_service::advisory_suffix(rating)
                {
                    if let Some(new_template) = inject_advisory_suffix_into_template(
                        companion_base_options.album_folder_template.as_deref(),
                        suffix,
                    ) {
                        log::info!(
                            "Companion folder template for {download_id}: \
                             injecting advisory suffix `{suffix}` so companions \
                             land in the primary's renamed folder (#528)"
                        );
                        companion_base_options.album_folder_template = Some(new_template);
                    }
                }
            }
        }
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

        // Per-download GAMDL version + capability flags (#755). The
        // process-global cache populated at startup means we read it
        // here without spawning a `--version` probe. Surfaces in both
        // the Tauri-event activity log AND the on-disk file so any
        // crash report can be correlated to the exact GAMDL release
        // that produced it.
        let gamdl_version_label = crate::services::gamdl_capabilities::detected_version()
            .unwrap_or_else(|| "unknown".to_string());
        let gamdl_capabilities = crate::services::gamdl_capabilities::active_capabilities_summary();
        emit_download_log(
            &app,
            &download_id,
            &format!(
                "GAMDL {gamdl_version_label} — capabilities: {gamdl_capabilities}"
            ),
        );

        // Verbose: log the full download options
        emit_verbose_download_log(
            &app,
            &download_id,
            &format!(
                "URLs: {:?} | Codec: {} | Native priority: {}",
                urls,
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
            // Snapshot the audio-file count in the configured output base
            // BEFORE spawning GAMDL (#831). This is the primary-path twin
            // of the Phase 3.5h companion-task snapshot. After a clean
            // GAMDL exit we compare against this baseline; if no new audio
            // files have landed in the output tree, the run failed
            // silently (GAMDL exits 0 when every track is skipped due to
            // format unavailability — "Skipping … format is not
            // available" is a warning, not an error from GAMDL's view).
            //
            // Without this check, the recovery path's `find_album_directory`
            // can pick up a previously-downloaded album directory from a
            // prior run and treat its files as evidence this run
            // succeeded — producing the user-visible "Complete" badge on
            // an item where every single track was actually skipped
            // (#831).
            let pre_run_audio_count = download_options
                .output_path
                .as_deref()
                .map(std::path::Path::new)
                .map(count_audio_files_in_directory)
                .unwrap_or(0);

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
                                        let content_label = q
                                            .items
                                            .iter()
                                            .find(|i| i.status.id == dl_id)
                                            .map(|i| format_content_label(&i.status))
                                            .unwrap_or_else(|| "unknown content".to_string());
                                        emit_download_log(
                                            &app_clone,
                                            &dl_id,
                                            &format!(
                                                "GAMDL tried all formats in priority chain \
                                                 for {content_label} — none available"
                                            ),
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
                                    let content_label = q
                                        .items
                                        .iter()
                                        .find(|i| i.status.id == dl_id)
                                        .map(|i| format_content_label(&i.status))
                                        .unwrap_or_else(|| "unknown content".to_string());
                                    emit_download_log(
                                        &app_clone,
                                        &dl_id,
                                        &format!(
                                            "All audio formats exhausted for {content_label} \
                                             — no compatible format found"
                                        ),
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

                            // #831: even if `has_output_now` is true, verify
                            // that this run actually produced NEW audio files.
                            // The `find_album_directory` recovery above can
                            // pick up a previously-downloaded album folder
                            // from an earlier run and set `output_path` to
                            // it — making the rest of the pipeline treat
                            // this run as a success even though zero new
                            // tracks landed. Without this check, retrying a
                            // failed item with the same (unavailable) codec
                            // chain endlessly produces "Complete" badges on
                            // items where every track was actually Skipped.
                            //
                            // Compare against the pre-run snapshot taken at
                            // the top of this spawn (line ~7155). If the
                            // count hasn't moved, treat the run as a
                            // no-files-produced failure regardless of what
                            // the recovery path set `output_path` to.
                            let new_files_landed = if has_output_now {
                                let post_run_count = download_options
                                    .output_path
                                    .as_deref()
                                    .map(std::path::Path::new)
                                    .map(count_audio_files_in_directory)
                                    .unwrap_or(0);
                                if post_run_count <= pre_run_audio_count {
                                    log::info!(
                                        "Download {dl_id}: output_path set but no new \
                                         audio files landed (pre={pre_run_audio_count}, \
                                         post={post_run_count}) — treating as \
                                         no-files-produced failure (#831)"
                                    );
                                    // Clear the misleading output_path so the
                                    // post-error UI doesn't open a folder that
                                    // was actually pre-existing.
                                    if let Some(item) = q
                                        .items
                                        .iter_mut()
                                        .find(|i| i.status.id == dl_id)
                                    {
                                        item.status.output_path = None;
                                        item.status.output_is_directory = false;
                                    }
                                    false
                                } else {
                                    true
                                }
                            } else {
                                false
                            };

                            if !new_files_landed {
                                // Terminal: no output, no IO recovery, no files on
                                // disk despite codec fallback exhausted.
                                //
                                // Distinguish the codec-skips-only case (#698) from
                                // genuine download failures: when every collected
                                // warning is a per-track codec-availability skip,
                                // Apple Music simply doesn't offer this content in
                                // any of the user's requested formats. That's a
                                // catalog limitation, not a download infrastructure
                                // failure, and the user-facing message should say
                                // so honestly instead of "Download completed but no
                                // output files were produced: [WARNING] Skipping ...".
                                let only_codec_skips = !warnings.is_empty()
                                    && warnings
                                        .iter()
                                        .all(|w| process::is_codec_skip_message(w));
                                let error_msg = if has_io_error {
                                    format!(
                                        "Output path may be unreachable or too slow. \
                                     Check your storage connection and try a local \
                                     output path. Details: {}",
                                        warnings.last().cloned().unwrap_or_default()
                                    )
                                } else if only_codec_skips {
                                    "No audio available: Apple Music does not offer \
                                 this content in any of your requested formats. Try \
                                 alternative codecs in Settings > Quality > Music \
                                 Codec, or check that this content exists in your \
                                 storefront."
                                        .to_string()
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
                                emit_download_error(
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
                        // NOTE (#706): `on_task_finished()` deliberately
                        // does NOT fire here, even though the primary
                        // GAMDL subprocess has exited. The slot stays
                        // held by an `ActiveSlotGuard` taken at the top
                        // of the completion task spawned below, so the
                        // queue contract from #455 ("ENTIRE pipeline
                        // completes before the next item starts") is
                        // actually enforced. Releasing here would let
                        // any concurrent `process_queue` invocation
                        // (user IPC, fallback retry, startup recovery)
                        // grab the next item while this item's
                        // companions + enrichment are still running.

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
                        // Phase 5e (#717): per-item music-video-companion override
                        // captured at spawn time so the enrichment task doesn't
                        // need a fresh queue-lock acquisition just to read one
                        // bool. Set by the Library Scan re-download flow when
                        // the user opts in/out of MVs on a specific gap-fill.
                        // `None` means "inherit settings.music_video_companion".
                        let enrich_mv_override: Option<bool> = {
                            let q = queue_clone.lock().await;
                            q.items
                                .iter()
                                .find(|i| i.status.id == dl_id)
                                .and_then(|i| i.request.mv_companion_override)
                        };
                        Some(tokio::spawn(async move {
                            // Determine the album directory from the output path.
                            // For single tracks, output_path is a file -- use its parent.
                            // For albums, output_path is already the directory.
                            //
                            // **#842 (artist-URL enrichment scope).** Pre-fix,
                            // when GAMDL wrote into `~/Music/Artist/Album/`,
                            // `output_dir` was `~/Music/` (the user's root)
                            // and the `dir.is_dir()` branch set `album_dir`
                            // to that same root. Every downstream pass
                            // (codec tagger, AcoustID, ReplayGain, lyrics
                            // services) then walked the WHOLE library —
                            // for the Forrest Frank 3-track artist URL the
                            // user reported, that meant tagging 69 files,
                            // fingerprinting 69 files, ReplayGain-analysing
                            // 73 files, and lyrics-fallback-scanning 73
                            // tracks. Now we resolve a specific album dir
                            // via the shared `find_album_directory` helper
                            // (same one #839 wired into the companion lyrics
                            // path). When hints are missing — common for
                            // artist URLs whose early metadata fetch returns
                            // `None` — the helper falls back to a depth-10
                            // most-recently-modified-leaf scan via
                            // `find_deepest_audio_dir` (#844 bounded the
                            // depth, so this is fast even on big libraries)
                            // and picks up the album GAMDL just produced.
                            let dir = std::path::Path::new(&output_dir);
                            let (early_artist_hint, early_album_hint) = {
                                let q = enrich_queue.lock().await;
                                q.items
                                    .iter()
                                    .find(|i| i.status.id == enrich_dl_id)
                                    .map(|i| {
                                        (
                                            i.status.artist_name.clone(),
                                            i.status.album_name.clone(),
                                        )
                                    })
                                    .unwrap_or((None, None))
                            };
                            let resolved_album_dir = if dir.is_dir() {
                                find_album_directory(
                                    dir,
                                    early_artist_hint.as_deref().filter(|s| !s.is_empty()),
                                    early_album_hint.as_deref().filter(|s| !s.is_empty()),
                                )
                            } else {
                                None
                            };
                            let mut album_dir = match resolved_album_dir {
                                Some(found) => found,
                                None if dir.is_dir() => output_dir.clone(),
                                None => dir.parent().map_or_else(
                                    || output_dir.clone(),
                                    |p| p.to_string_lossy().to_string(),
                                ),
                            };

                            // Helper: update the processing label AND the
                            // intra-Processing progress fraction for the queue
                            // item, then emit `queue-updated` so the frontend
                            // re-fetches. Callers pass both a human-readable
                            // label (#574) and a cumulative progress weight
                            // (#576) picked from `ENRICHMENT_STAGE_WEIGHTS`
                            // below. Weights are cumulative: stage N's weight
                            // is the fraction-complete AFTER stage N finishes.
                            //
                            // Label format appends album context
                            // (Artist: Album) so the progress bar surfaces
                            // which album the stage is acting on — useful
                            // when the user has a busy queue.
                            //
                            // Emit errors are swallowed: worst case is the UI
                            // stays stale until the next stage transition, no
                            // worse than pre-#574 behaviour.
                            // Phase 3.5d: replaced the closure-local set_label
                            // with a single-line wrapper around the new shared
                            // `progress_stages::set_stage` / `set_stage_with_label`
                            // helpers. The helpers take an `AppHandle` + `QueueHandle`
                            // so the **companion task** can also drive the per-item
                            // caption (Phase 3.5g) — pre-refactor, only the enrichment
                            // closure could update the label, which is why companion
                            // lyrics conversion used to leave a stale "ReplayGain…"
                            // caption visible for 30+ minutes (#712).
                            let label_queue = enrich_queue.clone();
                            let label_dl_id = enrich_dl_id.clone();
                            let label_app = enrich_app.clone();
                            let set_label = move |label: &str, progress: f32| {
                                // Reverse-lookup the stage from its weight so we can
                                // call set_stage_with_label without disturbing the
                                // existing call sites' (label, weight) ergonomics.
                                // Stages are identified uniquely by weight thanks to
                                // the `weights_strictly_increasing` invariant.
                                let stage = ProgressStage::ALL
                                    .iter()
                                    .copied()
                                    .find(|s| (s.weight() - progress).abs() < f32::EPSILON)
                                    .unwrap_or(ProgressStage::Finalising);
                                set_stage_with_label(&label_app, &label_queue, &label_dl_id, stage, label);
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

                            // Guard: skip the ENTIRE enrichment pipeline when the
                            // primary download produced zero output files (#567).
                            //
                            // This happens in three observed scenarios:
                            //   - GAMDL rejects the URL outright (e.g. legacy
                            //     iTunes URLs, #548 before the #568 rewrite fix).
                            //   - GAMDL's webplayback API returns an unexpected
                            //     shape and every track errors (#546 library-URL
                            //     case).
                            //   - Mid-pipeline decryption / network truncation
                            //     kills every track before any file lands.
                            //
                            // Without this guard, every enrichment stage (codec
                            // detection, metadata tagging, lyrics conversion,
                            // subtitle generation, ReplayGain, AcoustID,
                            // MusicBrainz, MV companion lookup, artwork fetch,
                            // advisory rename, manifest write — 20+ stages total)
                            // iterates zero files and either no-ops silently or
                            // emits a misleading "success" message. The most
                            // visible offender historically was the lyrics
                            // companion pipeline ("Lyrics companion (lrc)
                            // downloaded" lines despite zero audio files) —
                            // documented in #548 repro and broadened here to
                            // cover every post-GAMDL enrichment stage.
                            //
                            // Uses recursive counting via `count_audio_files_in_directory`
                            // so MV direct-URL downloads (which land deeper in
                            // `{artist}/Music Videos/{title} ({title_id}).mp4`)
                            // are correctly detected as "primary succeeded".
                            {
                                let dir_path = std::path::Path::new(&album_dir);
                                let output_file_count =
                                    count_audio_files_in_directory(dir_path);
                                if output_file_count == 0 {
                                    log::info!(
                                        "Skipping enrichment for {} — primary download produced no output files",
                                        enrich_dl_id,
                                    );
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "Enrichment skipped — primary download produced no output files",
                                    );
                                    return;
                                }
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

                            set_label("Enriching metadata tags...", ProgressStage::Metadata.weight());
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
                            // #871: detect Apple Music personal-library URLs
                            // (`/library/songs/i.XXX`, `/library/albums/l.XXX`,
                            // `/library/music-videos/i.XXX`). Library IDs are
                            // NOT valid against the catalog endpoints — calls
                            // return 404 and waste round-trips. We short-circuit
                            // every catalog-dependent enrichment step (iTunes
                            // Lookup, Apple Music Catalog, syllable-lyrics,
                            // animated artwork, music-video relations, artist
                            // promo video) by gating them on this flag and by
                            // forcing `try_fetch_metadata` to return None inside
                            // `apply_enriched_metadata_tags`. AcoustID,
                            // ReplayGain, codec/channel/source tag writes, and
                            // MusicBrainz lookup are local/non-Apple-catalog
                            // and still run unaltered.
                            let is_library = enrich_urls
                                .iter()
                                .any(|u| super::apple_music_api::is_library_url(u));
                            if is_library {
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Library item detected — skipping Apple Music catalog enrichment (iTunes Lookup, catalog metadata, syllable lyrics, animated artwork). AcoustID + ReplayGain + local tags still run.",
                                );
                            }

                            // --- Step 0: iTunes Lookup API enrichment (#454) ---
                            // Run FIRST (no auth required). Writes baseline metadata
                            // (country, disc count) that Apple Music API can overwrite.
                            // Skipped for library URLs (#871) — iTunes Lookup keys
                            // off the catalog album ID, which library IDs are not.
                            if !is_library {
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
                                    None, // No pre-fetched metadata; will fetch from API if possible (skipped for library)
                                    Some((&enrich_app, &enrich_dl_id)),
                                    enrich_native_priority,
                                    enrich_settings.content_advisory_in_filenames,
                                    is_library,
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
                            // with animated artwork FrontCover.mp4/FrontCoverPortrait.mp4).
                            rename_cover_art(&album_dir, enrich_settings.cover_art_name.to_filename_stem());

                            // --- Post-step 1b: Cover-art fallback chain (#756) ---
                            // GAMDL's static cover write is fragile, especially for
                            // `cover_format = raw` where the upstream `httpx` fetch
                            // raises a Python traceback and leaves no cover on disk.
                            // The embedded cover atom inside each M4A is unaffected,
                            // but the sidecar (Cover.<ext>) is missing — so file-
                            // browser previews fail. Walk the API artwork URL down a
                            // RAW → PNG → JPEG chain to recover.
                            if let Some(ref metadata) = album_metadata {
                                let cover_stem = enrich_settings
                                    .cover_art_name
                                    .to_filename_stem();
                                let outcome = crate::services::cover_art_fallback::ensure_cover_present(
                                    std::path::Path::new(&album_dir),
                                    metadata,
                                    &enrich_settings.cover_format,
                                    cover_stem,
                                )
                                .await;
                                match outcome {
                                    crate::services::cover_art_fallback::CoverFallbackOutcome::AlreadyPresent { .. } => {
                                        // Fast path — GAMDL did its job. Silent.
                                    }
                                    crate::services::cover_art_fallback::CoverFallbackOutcome::FetchedFallback { format, written_path } => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "Cover-art fallback wrote {} ({})",
                                                written_path.file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or("cover"),
                                                format.to_cli_string(),
                                            ),
                                        );
                                    }
                                    crate::services::cover_art_fallback::CoverFallbackOutcome::NoTemplate => {
                                        // Expected for non-album items; no log noise.
                                    }
                                    crate::services::cover_art_fallback::CoverFallbackOutcome::AllFallbacksFailed { last_error } => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "Cover-art fallback exhausted (PNG + JPEG both failed: {last_error}). Embedded cover in M4A is unaffected."
                                            ),
                                        );
                                    }
                                }
                            }

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
                                            // Content-aware deduped write (#553):
                                            // - identical bytes to an existing
                                            //   dump → no-op (skips the disk
                                            //   sprawl when the API response
                                            //   hasn't changed between runs);
                                            // - different bytes → disambiguate
                                            //   to `...data.1.json` so the old
                                            //   dump is preserved rather than
                                            //   silently replaced;
                                            // - no existing file → normal write.
                                            match crate::utils::fs_safe::write_deduped(
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
                                            // Attempt tracks whose `hasLyrics` flag is true OR absent
                                            // (None). Apple omits the flag on some album tracks even
                                            // when word-level lyrics exist; only an explicit
                                            // `hasLyrics == false` is a reliable "no lyrics" signal.
                                            // A track with genuinely no lyrics simply 404s and is
                                            // skipped by fetch_syllable_lyrics (Ok(None)), so treating
                                            // None as "try anyway" only adds a cheap probe, never a
                                            // spurious write. (ITAMenhancer uses the same heuristic.)
                                            .filter(|t| t.has_lyrics != Some(false))
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
                                                                    // Read the file and check for word-level timing.
                                                                    // Uses span-presence (not the itunes:timing attribute
                                                                    // string) as the authoritative signal (#969) -- Apple
                                                                    // labels word-timed TTML inconsistently ("Word",
                                                                    // "Syllable", or the attribute omitted entirely), so a
                                                                    // substring check against itunes:timing="Word" missed
                                                                    // files that already had usable word timing and
                                                                    // triggered a needless re-fetch.
                                                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                                                        if super::enhanced_lyrics_service::ttml_has_word_timing(&content) {
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
                                            set_label(
                                                "Fetching word-level lyrics...",
                                                ProgressStage::WordLyrics.weight(),
                                            );
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!(
                                                    "Fetching word-level lyrics from Apple Music API for {} track(s)...",
                                                    tracks_needing_upgrade.len()
                                                ),
                                            );

                                            let mut upgraded = 0u32;
                                            let mut no_lyrics_available = 0u32;
                                            let mut errored = 0u32;
                                            for track in &tracks_needing_upgrade {
                                                if enrich_shutdown.is_triggered() {
                                                    break;
                                                }
                                                match super::apple_music_api::fetch_syllable_lyrics(
                                                    &jwt,
                                                    &enrich_settings.storefront,
                                                    &track.song_id,
                                                    &token,
                                                    Some(&enrich_settings.language),
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
                                                        // Apple has no word-level TTML for this
                                                        // specific track — usually a very old
                                                        // catalog entry or a track that simply
                                                        // never received syllable lyrics
                                                        // upstream. Not an error; just count
                                                        // so the post-loop summary surfaces it.
                                                        no_lyrics_available += 1;
                                                        log::debug!(
                                                            "No syllable-lyrics available for track {} (song {})",
                                                            track.track_number,
                                                            track.song_id
                                                        );
                                                    }
                                                    Err(e) => {
                                                        errored += 1;
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

                                            // Post-loop summary — #935 quick win A.
                                            //
                                            // Pre-#935 / pre-A: only the upgraded > 0 path
                                            // surfaced anything to the user. Albums where every
                                            // track had no syllable-lyrics available (the
                                            // `Ok(None)` path) or every fetch failed (the
                                            // `Err(_)` path before the 401/403 break) ran in
                                            // total silence — the user had no idea Step 1b had
                                            // even tried, much less why their LRC was line-only.
                                            //
                                            // We now always emit at least one summary entry
                                            // when ≥1 track was attempted. The three counters
                                            // distinguish the three failure modes so the user
                                            // can route their next action (re-import cookies
                                            // vs. accept that this track has no word-level
                                            // upstream vs. retry later).
                                            let total_attempted = upgraded + no_lyrics_available + errored;
                                            if total_attempted > 0 {
                                                let summary = if upgraded == total_attempted {
                                                    // Happy path — all tracks upgraded
                                                    // cleanly. Keep the pre-A wording so
                                                    // long-time users see the message they
                                                    // expect.
                                                    format!(
                                                        "Word-level lyrics fetched from Apple Music API for {upgraded} track(s)"
                                                    )
                                                } else if upgraded > 0 {
                                                    // Mixed outcome — surface all three
                                                    // counters so the user sees the breakdown.
                                                    format!(
                                                        "Word-level lyrics: {upgraded} upgraded, {no_lyrics_available} unavailable on Apple Music, {errored} failed (of {total_attempted} attempted)"
                                                    )
                                                } else if no_lyrics_available == total_attempted {
                                                    // Every track was an `Ok(None)` — Apple
                                                    // genuinely has no syllable TTML for any
                                                    // of them. Not an error; the user keeps
                                                    // GAMDL's line-level TTML.
                                                    format!(
                                                        "Word-level lyrics: not available on Apple Music for any of the {total_attempted} track(s) — keeping line-level lyrics"
                                                    )
                                                } else if errored == total_attempted {
                                                    // Every fetch failed (no 401/403 since
                                                    // those would have broken the loop).
                                                    // Almost always a network / rate-limit
                                                    // issue.
                                                    format!(
                                                        "Word-level lyrics fetch failed for all {total_attempted} track(s) — check network connectivity"
                                                    )
                                                } else {
                                                    // No upgrades + mix of unavailable + errored.
                                                    format!(
                                                        "Word-level lyrics: {no_lyrics_available} unavailable on Apple Music, {errored} failed (of {total_attempted} attempted)"
                                                    )
                                                };
                                                if upgraded > 0 {
                                                    log::info!(
                                                        "Word-level lyrics fetched for {upgraded} track(s) for {enrich_dl_id}"
                                                    );
                                                }
                                                emit_download_log(
                                                    &enrich_app,
                                                    &enrich_dl_id,
                                                    &summary,
                                                );
                                            }
                                        }
                                    } else {
                                        set_label(
                                            "Skipping word-level lyrics (no credentials)",
                                            ProgressStage::WordLyrics.weight(),
                                        );
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

                            set_label(
                                "Converting lyrics (Enhanced LRC)...",
                                ProgressStage::LyricsConversion.weight(),
                            );
                            emit_download_log(&enrich_app, &enrich_dl_id, &format!("▶ Lyrics processing started{}", album_context()));
                            // --- Step 2: Enhanced LRC conversion (opt-in, default on) ---
                            // When enabled, converts TTML sidecar files to Enhanced LRC
                            // with word-by-word timestamps. Saves a `.lrc` sidecar file
                            // and embeds the Enhanced LRC in M4A/M4V metadata.
                            // Falls back to standard line-level LRC for songs without
                            // word-level timing in their TTML.
                            if enrich_settings.enhanced_lrc {
                                set_label(
                                    "Generating Enhanced LRC…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
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
                                set_label(
                                    "Generating WebVTT subtitles…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
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
                                set_label(
                                    "Generating Rich SRT…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
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
                                set_label(
                                    "Embedding subtitle sidecars…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
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
                                set_label(
                                    "Generating ASS subtitles…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
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

                            // --- Step 2g: Lyricsfile (.lyrics) generation (opt-in, #596) ---
                            // When enabled, converts the TTML sidecar GAMDL emitted
                            // into a Lyricsfile YAML sidecar via the shared
                            // `meedya_lyrics::Lyricsfile::from_ttml` upstream crate
                            // (MeedyaSuite-core#34). Preserves Apple Music's
                            // word-level timing in a vendor-neutral format that
                            // LRCGET + LRCLIB consume directly. Default off — the
                            // format is officially experimental per LRCGET 2.0
                            // release notes.
                            if enrich_settings.generate_lyricsfile
                                && !enrich_shutdown.is_triggered()
                            {
                                set_label(
                                    "Generating Lyricsfile (.lyrics) sidecars…",
                                    ProgressStage::LyricsConversion.weight(),
                                );
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Generating Lyricsfile (.lyrics) sidecars...",
                                );
                                let lf_dir = album_dir.clone();
                                let lf_default_title = album_metadata
                                    .as_ref()
                                    .and_then(|m| m.tracks.first())
                                    .map(|t| t.name.clone())
                                    .unwrap_or_else(|| "Untitled".to_string());
                                let lf_default_artist = album_metadata
                                    .as_ref()
                                    .and_then(|m| m.artist_name.clone())
                                    .unwrap_or_else(|| "Unknown Artist".to_string());
                                match tokio::task::spawn_blocking(move || {
                                    super::lyricsfile_service::generate_lyricsfile_for_directory(
                                        &lf_dir,
                                        &lf_default_title,
                                        &lf_default_artist,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| {
                                    Err(format!("Lyricsfile task panicked: {e}"))
                                }) {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                            "Generated {count} Lyricsfile sidecar(s) for {enrich_dl_id}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "Generated {count} Lyricsfile (.lyrics) sidecar(s)"
                                            ),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "No TTML sources found for Lyricsfile generation",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Lyricsfile generation skipped for {enrich_dl_id}: {e}"
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

                            set_label(
                                "Downloading animated artwork...",
                                ProgressStage::AnimatedArtwork.weight(),
                            );
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
                                        // #529: emit one deterministic line per
                                        // variant (square + portrait) instead of
                                        // the pre-#529 ambiguous "(square: yes,
                                        // portrait: no)" summary. The new
                                        // per-variant `VariantStatus` enum lets
                                        // us distinguish "API didn't offer this"
                                        // from "API offered it but download
                                        // failed" — the pre-#529 code lied
                                        // ("not available") on real failures.
                                        emit_artwork_variant_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "square",
                                            &result.square,
                                        );
                                        emit_artwork_variant_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "portrait",
                                            &result.portrait,
                                        );
                                        // #538: album-level 16:9 spotlight video.
                                        emit_artwork_variant_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "album spotlight",
                                            &result.spotlight,
                                        );

                                        // Hide artwork files if enabled in
                                        // settings. Hide-file failures are now
                                        // surfaced as warnings instead of
                                        // silently swallowed (#529 gap #4) —
                                        // the user needs to know their files
                                        // exist but didn't get the
                                        // `hide_animated_artwork` treatment
                                        // they configured.
                                        if enrich_settings.hide_animated_artwork {
                                            let dir = std::path::Path::new(&album_dir);
                                            if result.square.is_downloaded() {
                                                let target = dir.join("FrontCover.mp4");
                                                if let Err(e) =
                                                    super::animated_artwork_service::hide_file(
                                                        &target,
                                                    )
                                                    .await
                                                {
                                                    log::warn!(
                                                        "Failed to hide FrontCover.mp4: {e}"
                                                    );
                                                    emit_download_warn(
                                                        &enrich_app,
                                                        &enrich_dl_id,
                                                        &format!(
                                                            "Animated artwork: failed to hide square cover ({target_disp}) — {e}",
                                                            target_disp = target.display(),
                                                        ),
                                                    );
                                                }
                                            }
                                            if result.portrait.is_downloaded() {
                                                let target = dir.join("FrontCoverPortrait.mp4");
                                                if let Err(e) =
                                                    super::animated_artwork_service::hide_file(
                                                        &target,
                                                    )
                                                    .await
                                                {
                                                    log::warn!(
                                                        "Failed to hide FrontCoverPortrait.mp4: {e}"
                                                    );
                                                    emit_download_warn(
                                                        &enrich_app,
                                                        &enrich_dl_id,
                                                        &format!(
                                                            "Animated artwork: failed to hide portrait cover ({target_disp}) — {e}",
                                                            target_disp = target.display(),
                                                        ),
                                                    );
                                                }
                                            }
                                            // #538: hide the album-spotlight too.
                                            if result.spotlight.is_downloaded() {
                                                let target = dir.join("AlbumSpotlightCover.mp4");
                                                if let Err(e) =
                                                    super::animated_artwork_service::hide_file(
                                                        &target,
                                                    )
                                                    .await
                                                {
                                                    log::warn!(
                                                        "Failed to hide AlbumSpotlightCover.mp4: {e}"
                                                    );
                                                    emit_download_warn(
                                                        &enrich_app,
                                                        &enrich_dl_id,
                                                        &format!(
                                                            "Animated artwork: failed to hide album spotlight ({target_disp}) — {e}",
                                                            target_disp = target.display(),
                                                        ),
                                                    );
                                                }
                                            }
                                        }

                                        // Frontend event still fires on any
                                        // actual download (either variant) so
                                        // the UI can refresh its artwork
                                        // indicator.
                                        if result.square.is_downloaded()
                                            || result.portrait.is_downloaded()
                                            || result.spotlight.is_downloaded()
                                        {
                                            log::info!(
                                                "Animated artwork downloaded for {enrich_dl_id}"
                                            );
                                            let _ = enrich_app
                                                .emit("artwork-downloaded", &enrich_dl_id);
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
                                        set_label(
                                            "Downloading artist promo video…",
                                            ProgressStage::AnimatedArtwork.weight(),
                                        );
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

                            // --- Steps 4 + 6b lookup: AcoustID + MusicBrainz lookup (parallel) ---
                            //
                            // These two stages have independent I/O domains:
                            //   - AcoustID:    chromaprint fingerprint (CPU/I/O) →
                            //                  acoustid.org HTTP lookup → freeform-atom
                            //                  write via mp4ameta.
                            //   - MusicBrainz: musicbrainz.org HTTP lookup, rate-limited
                            //                  at 1.1 sec/req. No audio file writes.
                            //
                            // Running them concurrently saves up to one stage's worth of
                            // wall-clock time on heavy albums where both are enabled
                            // (#779 Option 1). For the user-reported 19-track live
                            // album, the dominant per-track cost was ReplayGain +
                            // AcoustID running serially; this fix overlaps AcoustID with
                            // the rate-limited MusicBrainz HTTP lookup so neither has to
                            // wait for the other.
                            //
                            // ReplayGain (Step 5, below) deliberately stays sequential
                            // because it ALSO writes to the same M4A files via mp4ameta,
                            // and concurrent `Tag::write_to_path` calls would race
                            // (different atoms, but the underlying read-modify-write of
                            // the tag set conflicts). Tracked separately as Option 2/3
                            // in #779 if Option 1 isn't enough.
                            //
                            // The MusicBrainz video DOWNLOADS (separate GAMDL processes
                            // per video) are kept after Step 6 (MV companion via Apple
                            // Music API) so the per-video downloader sees the full set
                            // of discovered MV URLs in one pass.

                            // Pre-resolve mv_companion_enabled here so both the parallel
                            // pair AND the sequential MV/MusicBrainz-download stages
                            // below can share the same value. Originally lived in Step 6.
                            let mv_companion_enabled = enrich_mv_override
                                .unwrap_or(enrich_settings.music_video_companion);

                            // Pre-compute the MusicBrainz lookup inputs once so the
                            // async block doesn't re-borrow nested Option chains.
                            let run_musicbrainz_lookup = (enrich_settings.musicbrainz_lookup
                                || mv_companion_enabled)
                                && !enrich_shutdown.is_triggered();
                            // Built as `TrackLookupInfo` (not the legacy
                            // `(song_id, isrc)` pair) so the lookup task
                            // below can drive the full T1-T3 + S1/S2 chain
                            // (`lookup_videos_for_tracks_enhanced`,
                            // Tranche E migration off the now-deleted
                            // legacy compat wrapper — m2). `artist` is the
                            // per-track artist with an
                            // album-artist fallback (M2a) — using the album
                            // artist alone would reject every genuine S1
                            // match on a compilation/various-artists album,
                            // where per-track artists legitimately differ
                            // from the album artist. `apple_music_url` (T1)
                            // and `musicbrainz_recording_id` (T3) stay
                            // `None` here — both tiers remain latent until a
                            // caller threads those identifiers through,
                            // unchanged by this migration.
                            let mb_isrc_tracks: Option<Vec<super::musicbrainz_service::TrackLookupInfo>> =
                                if run_musicbrainz_lookup {
                                    album_metadata.as_ref().map(|metadata| {
                                        metadata
                                            .tracks
                                            .iter()
                                            .map(|t| super::musicbrainz_service::TrackLookupInfo {
                                                song_id: t.song_id.clone(),
                                                apple_music_url: None,
                                                isrc: t.isrc.clone(),
                                                musicbrainz_recording_id: None,
                                                artist: t
                                                    .artist_name
                                                    .clone()
                                                    .or_else(|| metadata.artist_name.clone()),
                                                title: Some(t.name.clone()),
                                            })
                                            .collect()
                                    })
                                } else {
                                    None
                                };
                            // S2's once-per-album URL search input (§0.2
                            // M3/m4) — the queue item's own Apple Music URL,
                            // mirroring the `enrich_urls.first()` idiom used
                            // for the Odesli cross-platform lookup below.
                            // `AlbumLookupContext::search_fallback` is the
                            // Tranche F kill switch: `false` makes S1/S2
                            // completely inert, bit-for-bit identical to
                            // today's T1/T2/T3-only chain.
                            let mb_album_ctx = super::musicbrainz_service::AlbumLookupContext {
                                album_url: enrich_urls.first().cloned(),
                                search_fallback: enrich_settings.musicbrainz_search_fallback,
                            };

                            set_label(
                                "AcoustID + MusicBrainz lookup + ReplayGain (parallel)…",
                                ProgressStage::AcoustId.weight(),
                            );
                            emit_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                &format!(
                                    "▶ AcoustID + MusicBrainz lookup + ReplayGain started in parallel{}",
                                    album_context()
                                ),
                            );

                            // Per-file write-coordination map (#779 Option 2).
                            // AcoustID and ReplayGain both write freeform
                            // atoms to the SAME M4A files via
                            // `mp4ameta::Tag::write_to_path`. The locks
                            // serialise their writes at the granularity of a
                            // single file so the slow per-file analyses
                            // (chromaprint, FFmpeg ebur128) can run truly in
                            // parallel without racing on the tag-write
                            // critical section. MusicBrainz doesn't touch
                            // audio files at all, so it never contends.
                            let enrich_file_locks = std::sync::Arc::new(
                                crate::utils::file_locks::FileWriteLocks::new(),
                            );
                            let acoustid_file_locks = enrich_file_locks.clone();
                            let replaygain_file_locks = enrich_file_locks.clone();

                            // --- AcoustID async block (Step 4, opt-in) ---
                            // Generates Chromaprint fingerprints via the embedded
                            // rusty-chromaprint library and looks up AcoustID
                            // identifiers from acoustid.org. API key resolution
                            // priority: user override → compile-time embedded key → none.
                            let acoustid_task = async {
                                if !enrich_settings.acoustid_enabled {
                                    return;
                                }
                                let Some(api_key) = super::acoustid_service::resolve_api_key(
                                    &enrich_settings.acoustid_api_key,
                                ) else {
                                    emit_download_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "AcoustID skipped: no API key available",
                                    );
                                    return;
                                };
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Running AcoustID fingerprinting...",
                                );
                                match super::acoustid_service::process_acoustid_for_directory(
                                    &album_dir,
                                    &api_key,
                                    // Per-track caption update (#574). Both
                                    // AcoustID and ReplayGain now race to
                                    // update the caption (they run in
                                    // parallel post #779 Option 2). User
                                    // sees whichever stage updated last —
                                    // still better than the previous static
                                    // label, and both labels name the stage
                                    // so the caption is always meaningful.
                                    |current, total| {
                                        set_label(
                                            &format!(
                                                "AcoustID fingerprinting: track {current} of {total}"
                                            ),
                                            ProgressStage::AcoustId.weight(),
                                        );
                                    },
                                    Some(&acoustid_file_locks),
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
                            };

                            // --- MusicBrainz lookup async block (Step 6b lookup, opt-in) ---
                            // Returns Option<Vec<MusicVideoUrl>> with the discovered
                            // video URLs. The actual GAMDL download of any Apple Music
                            // videos is deferred to a sequential step BELOW Step 5/6
                            // so it doesn't race with the per-track audio file writes.
                            let musicbrainz_lookup_task = async {
                                let lookup_tracks = mb_isrc_tracks?;
                                if lookup_tracks.is_empty() {
                                    return None;
                                }
                                emit_download_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &format!(
                                        "MusicBrainz: looking up {} track(s) via ISRC...",
                                        lookup_tracks.len()
                                    ),
                                );
                                // Tranche E: migrated off the deleted
                                // legacy compat wrapper (m2 — this was its
                                // only caller) onto the enhanced T1-T3 +
                                // S1/S2 chain directly.
                                match super::musicbrainz_service::lookup_videos_for_tracks_enhanced(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &lookup_tracks,
                                    &mb_album_ctx,
                                )
                                .await
                                {
                                    Ok(videos) if !videos.is_empty() => {
                                        // Log all discovered platform URLs for future reference
                                        for video in &videos {
                                            log::info!(
                                                "MusicBrainz video for {}: {} → {}",
                                                enrich_dl_id,
                                                video.platform,
                                                video.url,
                                            );
                                        }
                                        let am_count = videos
                                            .iter()
                                            .filter(|v| v.platform == "apple_music")
                                            .count();
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "MusicBrainz: found {} video(s) ({} on Apple Music)",
                                                videos.len(),
                                                am_count,
                                            ),
                                        );
                                        Some(videos)
                                    }
                                    Ok(_) => {
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "MusicBrainz: no music videos found",
                                        );
                                        None
                                    }
                                    Err(e) => {
                                        // Structurally unreachable today (m3):
                                        // `lookup_videos_for_tracks_enhanced`
                                        // handles every per-tier error
                                        // internally (verbose-logged, tier
                                        // falls through) and always returns
                                        // `Ok`. Retained as defensive
                                        // belt-and-braces in case a future
                                        // change to the fn's error contract
                                        // reintroduces an `Err` path.
                                        log::debug!(
                                            "MusicBrainz lookup failed for {enrich_dl_id}: {e}"
                                        );
                                        emit_download_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("MusicBrainz lookup failed: {e}"),
                                        );
                                        None
                                    }
                                }
                            };

                            // --- ReplayGain async block (Step 5, opt-in) ---
                            // Moved into the parallel join (#779 Option 2) so
                            // its per-file FFmpeg ebur128 decode overlaps
                            // with AcoustID's chromaprint + API work. The
                            // tag-write race against AcoustID is prevented
                            // by `enrich_file_locks` — both stages acquire
                            // the per-file lock before mp4ameta touches the
                            // file. The slow analysis (which is what
                            // actually takes 5-10 sec/track on long-form
                            // audio) runs without holding the lock.
                            let replaygain_task = async {
                                if !enrich_settings.replaygain_enabled {
                                    return;
                                }
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
                                    |current, total| {
                                        set_label(
                                            &format!(
                                                "ReplayGain analysis: track {current} of {total}"
                                            ),
                                            ProgressStage::ReplayGain.weight(),
                                        );
                                    },
                                    Some(&replaygain_file_locks),
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
                            };

                            let ((), musicbrainz_videos, ()) =
                                tokio::join!(acoustid_task, musicbrainz_lookup_task, replaygain_task);

                            if enrich_shutdown.is_triggered() {
                                log::info!("Enrichment stopping early for {enrich_dl_id} (app shutting down)");
                                return;
                            }

                            emit_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                &format!(
                                    "✓ AcoustID + MusicBrainz lookup + ReplayGain completed{}",
                                    album_context()
                                ),
                            );

                            set_label(
                                "Music video discovery...",
                                ProgressStage::MusicVideoDiscovery.weight(),
                            );

                            // Snapshot the MV file count under the user's output
                            // root BEFORE Step 6 / 6b run, so we can write the
                            // *actual* count of MV companions produced into
                            // `item.status.mv_companion_count` for the
                            // completion-task companion-wait deadline (#776).
                            // Pre-fix the deadline used a `min(track_count, 30)`
                            // estimate — fine for the conservative case but
                            // generous when the album has only a few MVs and
                            // tight when it has more than 30.
                            //
                            // Empty `output_path` ⇒ user is on the OS default;
                            // skip tracking (the heuristic estimate at the
                            // completion-task site stays in effect for those
                            // items).
                            let mv_pre_count = (!enrich_settings.output_path.is_empty())
                                .then(|| snapshot_video_files(&enrich_settings.output_path).len());

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
                            //
                            // `mv_companion_enabled` was resolved earlier (above the
                            // AcoustID || MusicBrainz join) so it could be shared between
                            // the parallel lookup gate and this sequential download stage.
                            if mv_companion_enabled && !enrich_shutdown.is_triggered() {
                                // Tier 3 (#559): pass the resolved on-disk
                                // album directory so MVs land alongside
                                // the album's audio tracks instead of in
                                // a generic Music Videos/ bucket.
                                spawn_music_video_companion_inner(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    &enrich_urls,
                                    album_metadata.as_ref(),
                                    &enrich_settings,
                                    &enrich_shutdown,
                                    Some(album_dir.as_str()),
                                )
                                .await;
                            }

                            // --- Step 6b: Download MusicBrainz-discovered videos (opt-in) ---
                            // The MusicBrainz lookup itself ran in parallel with AcoustID
                            // (Steps 4 + 6b lookup, above). This block consumes the videos
                            // it discovered and downloads any Apple Music URLs via GAMDL.
                            // Cross-platform URLs (YouTube, Spotify, etc.) were already
                            // logged at lookup time for future reference.
                            //
                            // Phase 5e (#717): per-item override applies here too — when
                            // the user opted out of MVs for THIS gap-fill, MusicBrainz-
                            // discovered URLs must not be downloaded either.
                            if let Some(videos) = musicbrainz_videos {
                                let am_videos: Vec<_> = videos
                                    .iter()
                                    .filter(|v| v.platform == "apple_music")
                                    .collect();
                                if mv_companion_enabled
                                    && !am_videos.is_empty()
                                    && !enrich_shutdown.is_triggered()
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
                                        let label =
                                            video.title.as_deref().unwrap_or("unknown");
                                        // Tier 3 (#559): MusicBrainz
                                        // fallback inherits the same parent-
                                        // album context as the MusicKit path.
                                        download_music_video_by_url(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &video.url,
                                            label,
                                            &enrich_settings,
                                            Some(album_dir.as_str()),
                                        )
                                        .await;
                                    }
                                }
                            }

                            // Snapshot the MV file count AFTER Step 6 + 6b
                            // and write the diff to the queue item so the
                            // completion task's companion-wait deadline can
                            // size against the real number of MV downloads
                            // instead of the conservative estimate (#776).
                            // Skipped when `output_path` is empty (no
                            // tracking root) — the heuristic estimate at
                            // the completion-task site applies in that
                            // case.
                            if let Some(pre) = mv_pre_count {
                                let post = snapshot_video_files(&enrich_settings.output_path).len();
                                let mv_count = post.saturating_sub(pre);
                                let mut q = enrich_queue.lock().await;
                                if let Some(item) = q
                                    .items
                                    .iter_mut()
                                    .find(|i| i.status.id == enrich_dl_id)
                                {
                                    item.status.mv_companion_count = Some(mv_count);
                                }
                                drop(q);
                            }

                            // Step 6c (#295 Phase A): Odesli cross-platform URL
                            // lookup. Opt-in via `odesli_lookup_enabled`. The
                            // call is rate-limited at ~1.1 s between requests
                            // (well below the 10 req/min free-tier cap) by
                            // `odesli_service`'s per-process limiter, so the
                            // wait time is bounded regardless of how many
                            // albums are enriching in parallel. Result is
                            // threaded into `write_manifest` below.
                            //
                            // Skipped silently on:
                            //   - feature toggle off
                            //   - empty URL list (defensive)
                            //   - API miss / network failure (logged at debug)
                            let cross_platform_urls = if enrich_settings.odesli_lookup_enabled
                                && !enrich_shutdown.is_triggered()
                            {
                                let source_url = enrich_urls
                                    .first()
                                    .cloned()
                                    .unwrap_or_default();
                                if source_url.is_empty() {
                                    None
                                } else {
                                    set_label(
                                        "Odesli: looking up cross-platform URLs…",
                                        ProgressStage::Finalising.weight(),
                                    );
                                    let key = if enrich_settings.odesli_api_key.is_empty() {
                                        None
                                    } else {
                                        Some(enrich_settings.odesli_api_key.as_str())
                                    };
                                    match crate::services::odesli_service::fetch_links(
                                        &source_url,
                                        key,
                                    )
                                    .await
                                    {
                                        Ok(Some(urls)) if !urls.is_empty() => {
                                            emit_download_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!(
                                                    "Odesli: discovered {} cross-platform URL(s)",
                                                    urls.len()
                                                ),
                                            );
                                            Some(urls)
                                        }
                                        Ok(_) => {
                                            log::debug!(
                                                "Odesli: no cross-platform matches for {source_url}"
                                            );
                                            None
                                        }
                                        Err(e) => {
                                            log::debug!(
                                                "Odesli lookup failed for {source_url}: {e}"
                                            );
                                            None
                                        }
                                    }
                                }
                            } else {
                                None
                            };

                            // Write/update manifest.meedyadl in the album folder.
                            // Records the source URL and per-track metadata so users
                            // can re-download by importing the manifest file.
                            set_label(
                                "Writing download manifest…",
                                ProgressStage::Finalising.weight(),
                            );
                            // Resolve the primary codec to its canonical
                            // registry ID and snapshot the planned companion
                            // tiers (#766). The smart-retry planner uses these
                            // to detect missing-codec-variant gaps after
                            // companion-tier timeouts. `enrich_codec_str` is the
                            // CLI flag string (e.g. "atmos") captured on the
                            // success path; map it back to a `SongCodec` to
                            // derive the registry ID, falling through to `None`
                            // for the (extremely rare) unparseable case so the
                            // manifest write still succeeds with codec-blind
                            // diff semantics.
                            let (manifest_primary_id, manifest_companion_tiers) =
                                enrich_codec_str
                                    .as_deref()
                                    .and_then(SongCodec::from_cli_string)
                                    .map_or((None, None), |c| {
                                        let id = song_codec_to_registry_id(&c).to_string();
                                        let tiers = build_manifest_companion_tiers(
                                            &enrich_settings,
                                            c.to_cli_string(),
                                        );
                                        (Some(id), tiers)
                                    });
                            write_manifest(
                                &album_dir,
                                &enrich_urls,
                                album_metadata.as_ref(),
                                &enrich_settings,
                                &enrich_started_at,
                                cross_platform_urls,
                                manifest_primary_id.as_deref(),
                                manifest_companion_tiers,
                            );
                            emit_download_log(
                                &enrich_app,
                                &enrich_dl_id,
                                "Download manifest saved to album folder",
                            );

                            // Enrichment is finished — clear the per-item label
                            // so the bar caption reverts to the track context
                            // (Path 2 in `formatActiveItemCaption`) until the
                            // next stage takes over (companion supervisor's
                            // `Companion: downloading…`, the post-companion
                            // advisory pass's `Applying [Explicit]/[Clean]
                            // suffixes…`, or `set_complete` clearing on the
                            // happy path with neither configured).
                            //
                            // Pre-fix this line called `set_label("Finalising
                            // metadata...", …)` AFTER the manifest write — but
                            // at that point enrichment is *done*, not
                            // finalising, and the label persisted as the
                            // visible caption through every subsequent gap
                            // (enrichment→companion handoff, companion→
                            // advisory handoff, etc.). The screenshot the
                            // user reported on 2026-05-11 caught one of those
                            // gaps and showed "Finalising metadata…" while
                            // the activity log was reporting fresh GAMDL
                            // companion track downloads. Clearing here keeps
                            // the caption honest.
                            {
                                let mut q = enrich_queue.lock().await;
                                q.clear_processing_label(&enrich_dl_id);
                            }

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
                        // Take ownership of the queue slot for the lifetime of
                        // the completion task (#706). The success path's early
                        // `q.on_task_finished()` was removed, so the slot
                        // remains held until either the explicit release at the
                        // bottom of this task (happy path) or the guard's Drop
                        // (panic / abort / runtime shutdown). This makes the
                        // queue *actually* serial as documented in #455.
                        let active_guard = ActiveSlotGuard::new(completion_queue.clone());
                        // Copy (bool is `Copy`) so the integrity guard below
                        // can use it after the enrichment closure captured
                        // its own copy (`enrich_is_apple_music`, see #452 /
                        // Step 1 above) by move (#1021).
                        let completion_is_apple_music = is_apple_music;
                        tokio::spawn(async move {
                            // Wait for enrichment to finish with a timeout (#461).
                            // If enrichment hangs (e.g., deadlock, unresponsive API),
                            // we force completion after the deadline to prevent
                            // the queue from stalling indefinitely.
                            //
                            // IMPORTANT: on timeout, abort() the task so it is
                            // actually cancelled rather than merely detached
                            // (dropping a JoinHandle detaches the task and lets
                            // it keep running, which can reintroduce the
                            // cross-contamination #461 was opened to prevent).
                            //
                            // The deadline scales with the number of output
                            // files (#579): a 200-track box set legitimately
                            // needs 15–20 min for ReplayGain + AcoustID + the
                            // other per-track enrichment stages, and the fixed
                            // 10 min from #461 was force-completing mid-
                            // ReplayGain with tracks missing their tags.
                            let (output_path_for_timeout, content_label) = {
                                let q = completion_queue.lock().await;
                                let item = q
                                    .items
                                    .iter()
                                    .find(|i| i.status.id == completion_dl_id);
                                let path = item.and_then(|i| i.status.output_path.clone());
                                let label = item
                                    .map(|i| format_content_label(&i.status))
                                    .unwrap_or_else(|| "unknown content".to_string());
                                (path, label)
                            };
                            let track_count = output_path_for_timeout
                                .as_deref()
                                .map(std::path::Path::new)
                                .map(|p| {
                                    if p.is_dir() {
                                        count_audio_files_in_directory(p)
                                    } else {
                                        // Single-file output (e.g. direct MV URL) —
                                        // count its parent directory.
                                        p.parent()
                                            .map(count_audio_files_in_directory)
                                            .unwrap_or(0)
                                    }
                                })
                                .unwrap_or(0);
                            // Estimate the MV-companion budget from settings (#776).
                            // The enrichment task hasn't yet fetched the
                            // music-video relations, so we don't know the
                            // exact count — but if the user has enabled MV
                            // companions, give the timeout headroom for up to
                            // `min(track_count, 30)` MVs (some tracks may
                            // have an MV, most won't, but cap at 30 so a
                            // 200-track box set doesn't propose 200 extra
                            // minutes on top). Overestimating is harmless;
                            // underestimating risks a false-positive timeout
                            // on an MV-heavy album.
                            let timeout_settings = load_settings_for_queue(&completion_app);
                            let mv_count_estimate = if timeout_settings.music_video_companion {
                                track_count.min(30)
                            } else {
                                0
                            };
                            let enrichment_timeout =
                                compute_total_timeout(track_count, 0, mv_count_estimate);
                            let timeout_mins = enrichment_timeout.as_secs() / 60;
                            log::info!(
                                "Completion timeout for {}: {} min ({} track(s), ~{} MV companion(s) estimated)",
                                completion_dl_id,
                                timeout_mins,
                                track_count,
                                mv_count_estimate,
                            );
                            if let Some(mut handle) = enrichment_handle {
                                if tokio::time::timeout(enrichment_timeout, &mut handle)
                                    .await
                                    .is_err()
                                {
                                    log::warn!(
                                        "Enrichment timed out after {} minutes for {} ({} track(s) in output) — some files may be missing ReplayGain / AcoustID / MusicBrainz tags",
                                        timeout_mins,
                                        completion_dl_id,
                                        track_count,
                                    );
                                    emit_download_log(
                                        &completion_app,
                                        &completion_dl_id,
                                        &format!(
                                            "⚠ Enrichment timed out after {timeout_mins} minutes for {content_label} ({track_count} track(s) in output) — some files may be missing ReplayGain / AcoustID / MusicBrainz tags"
                                        ),
                                    );
                                    handle.abort();
                                    let _ = handle.await;
                                }
                            }
                            // Wait for companion downloads with a timeout that
                            // scales with the planned tier count (each tier is
                            // a full GAMDL re-download). Reusing the
                            // enrichment-only deadline here was triggering
                            // hard-timeouts on multi-tier "Atmos → all formats"
                            // configurations that were still legitimately
                            // running.
                            if let Some(mut handle) = companion_handle {
                                let tier_count = handle.tier_count();
                                // Now that enrichment has finished, the
                                // exact MV-companion count is on the queue
                                // item (written by the snapshot pass at
                                // the end of enrichment Step 6 + 6b, #776).
                                // Use it if present; fall back to the
                                // pre-enrichment estimate if the item was
                                // tracked-out (empty `output_path`) or if
                                // enrichment was aborted before the count
                                // was written.
                                let mv_count_actual = {
                                    let q = completion_queue.lock().await;
                                    q.items
                                        .iter()
                                        .find(|i| i.status.id == completion_dl_id)
                                        .and_then(|i| i.status.mv_companion_count)
                                };
                                let mv_count_for_companion =
                                    mv_count_actual.unwrap_or(mv_count_estimate);
                                let companion_timeout = compute_total_timeout(
                                    track_count,
                                    tier_count,
                                    mv_count_for_companion,
                                );
                                let companion_timeout_mins = companion_timeout.as_secs() / 60;
                                let mv_source = if mv_count_actual.is_some() {
                                    "actual"
                                } else {
                                    "estimated"
                                };
                                log::info!(
                                    "Companion timeout for {}: {} min ({} tier(s) planned, {} MV companion(s) {})",
                                    completion_dl_id,
                                    companion_timeout_mins,
                                    tier_count,
                                    mv_count_for_companion,
                                    mv_source,
                                );
                                if tokio::time::timeout(
                                    companion_timeout,
                                    &mut handle.handle,
                                )
                                .await
                                .is_err()
                                {
                                    let pending = handle.describe_pending();
                                    log::warn!(
                                        "Companion downloads still running after {} minutes for {} ({})",
                                        companion_timeout_mins,
                                        completion_dl_id,
                                        pending,
                                    );
                                    emit_download_log(
                                        &completion_app,
                                        &completion_dl_id,
                                        &format!(
                                            "⚠ Companion downloads still running after {companion_timeout_mins} minutes for {content_label} — waiting instead of skipping ({pending}); final tag pass will run afterwards"
                                        ),
                                    );
                                    if tokio::time::timeout(
                                        companion_timeout,
                                        &mut handle.handle,
                                    )
                                    .await
                                    .is_err()
                                    {
                                        let hard_timeout_mins =
                                            companion_timeout_mins.saturating_mul(2);
                                        let skipped = handle.describe_pending();
                                        log::warn!(
                                            "Companion downloads hard-timed-out after {} minutes for {} — skipping {}",
                                            hard_timeout_mins,
                                            completion_dl_id,
                                            skipped,
                                        );
                                        emit_download_log(
                                            &completion_app,
                                            &completion_dl_id,
                                            &format!(
                                                "⚠ Companion downloads hard-timed-out after {hard_timeout_mins} minutes for {content_label} — skipping remaining companions: {skipped}; final tag pass still to run"
                                            ),
                                        );
                                        handle.abort();
                                    }
                                    let _ = handle.handle.await;
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
                                        // Make the long, otherwise-silent advisory
                                        // pass visible in the activity log (#661)
                                        // AND on the per-item progress bar — Phase
                                        // 3.5g, courtesy of the shared
                                        // `set_stage_with_label` helper from 3.5d
                                        // (this is the completion task, not the
                                        // enrichment task, so the closure-local
                                        // `set_label` was unreachable here pre-3.5d).
                                        // On large box sets this can run for many
                                        // minutes; without a marker, users could
                                        // not tell whether MeedyaDL was hung or
                                        // working.
                                        set_stage_with_label(
                                            &completion_app,
                                            &completion_queue,
                                            &completion_dl_id,
                                            ProgressStage::Finalising,
                                            "Applying [Explicit]/[Clean] suffixes…",
                                        );
                                        emit_download_log(
                                            &completion_app,
                                            &completion_dl_id,
                                            "Final tag pass: applying [Explicit]/[Clean] suffixes…",
                                        );
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
                                        // #815 defensive fix: the advisory pass is
                                        // a sync recursive fs walk. On a slow
                                        // network share / disconnected cloud
                                        // mount / pathological tree it can hang
                                        // for hours and block the completion
                                        // task — preventing the set_complete +
                                        // on_task_finished that release the queue
                                        // slot. Wrap in spawn_blocking + timeout
                                        // so:
                                        //   1. The sync walk runs on a dedicated
                                        //      blocking thread (doesn't block the
                                        //      runtime).
                                        //   2. If it doesn't finish in
                                        //      ADVISORY_TIMEOUT, we log a warning
                                        //      and let the completion task
                                        //      proceed. The orphaned blocking
                                        //      thread keeps running on its own
                                        //      — no leak from the runtime's
                                        //      perspective.
                                        //
                                        // Result: even if the advisory pass
                                        // hangs forever, the queue slot still
                                        // gets released and the next item
                                        // starts.
                                        const ADVISORY_TIMEOUT: std::time::Duration =
                                            std::time::Duration::from_secs(300);
                                        let dir_clone = dir_for_advisory.clone();
                                        let blocking = tokio::task::spawn_blocking(move || {
                                            super::metadata_tag_service::apply_advisory_suffixes_from_tags(
                                                &dir_clone,
                                            );
                                        });
                                        match tokio::time::timeout(ADVISORY_TIMEOUT, blocking).await {
                                            Ok(Ok(())) => {
                                                // Normal completion — advisory pass
                                                // finished in time.
                                            }
                                            Ok(Err(join_err)) => {
                                                log::warn!(
                                                    "Advisory pass panicked for {completion_dl_id}: {join_err} — proceeding to set_complete anyway"
                                                );
                                                emit_download_log(
                                                    &completion_app,
                                                    &completion_dl_id,
                                                    "⚠ Final tag pass: internal error — proceeding to completion to keep the queue moving",
                                                );
                                            }
                                            Err(_) => {
                                                log::warn!(
                                                    "Advisory pass timed out after {ADVISORY_TIMEOUT:?} for {completion_dl_id} — proceeding to set_complete (orphaned blocking task continues in background)"
                                                );
                                                emit_download_log(
                                                    &completion_app,
                                                    &completion_dl_id,
                                                    &format!(
                                                        "⚠ Final tag pass timed out after {} min — proceeding to completion to keep the queue moving. Already-completed renames are preserved on disk.",
                                                        ADVISORY_TIMEOUT.as_secs() / 60,
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            // Post-download output integrity guard (#1021).
                            // Probes a bounded sample of the item's output
                            // files for suspiciously short/empty audio
                            // streams -- the signature of a truncated write
                            // that still exits GAMDL with status 0
                            // (gamdl#328). Apple-Music-only: it's the only
                            // source whose output-path shape
                            // `verify_output_integrity` understands.
                            const INTEGRITY_PROBE_CAP: usize = 12;
                            let integrity_report = if completion_is_apple_music {
                                let output_path_for_integrity = {
                                    let q = completion_queue.lock().await;
                                    q.items
                                        .iter()
                                        .find(|i| i.status.id == completion_dl_id)
                                        .and_then(|i| i.status.output_path.clone())
                                };
                                match output_path_for_integrity {
                                    Some(path) => {
                                        super::metadata_tag_service::verify_output_integrity(
                                            &completion_app,
                                            &path,
                                            INTEGRITY_PROBE_CAP,
                                        )
                                        .await
                                    }
                                    None => None,
                                }
                            } else {
                                None
                            };

                            let integrity_failure = integrity_report
                                .as_ref()
                                .and_then(|r| integrity_failure_message(r.checked, &r.suspect_files));

                            // Partial-failure case: SOME (but not every)
                            // probed file is suspect. The item still
                            // completes -- most of the album is fine -- but
                            // a prominent warning + notification surfaces
                            // the gap instead of a silent "Complete".
                            if integrity_failure.is_none() {
                                if let Some(report) = &integrity_report {
                                    if !report.suspect_files.is_empty() {
                                        emit_download_warn(
                                            &completion_app,
                                            &completion_dl_id,
                                            &format!(
                                                "⚠ {} file(s) may be corrupted or truncated: {} — re-download with overwrite OFF to retry just those tracks (see gamdl#328).",
                                                report.suspect_files.len(),
                                                report.suspect_files.join(", "),
                                            ),
                                        );
                                        send_desktop_notification(
                                            &completion_app,
                                            "Completed With Warnings",
                                            &format!(
                                                "{} file(s) may need re-downloading",
                                                report.suspect_files.len(),
                                            ),
                                        );
                                    }
                                }
                            }

                            // Mark as complete (or errored, when every
                            // probed file came back suspect) and release
                            // the queue slot in the same lock acquisition
                            // (#706). Releasing the slot here — *not* at the
                            // early line 6246 — is what makes the queue
                            // actually serial; the next item cannot start
                            // until set_complete/set_error + the
                            // accompanying decrement land atomically. The
                            // ActiveSlotGuard is then disarmed so its Drop is
                            // a no-op and we don't double-release.
                            {
                                let mut q = completion_queue.lock().await;
                                if let Some(ref msg) = integrity_failure {
                                    q.set_error(&completion_dl_id, msg);
                                } else {
                                    q.set_complete(&completion_dl_id);
                                }
                                q.on_task_finished();
                                drop(q);
                            }
                            active_guard.disarm();
                            save_queue_to_disk(&completion_app, &completion_queue).await;

                            if let Some(msg) = integrity_failure {
                                emit_download_warn(&completion_app, &completion_dl_id, &msg);
                                let guidance = process::error_guidance("io");
                                let _ = completion_app.emit(
                                    "download-error",
                                    serde_json::json!({
                                        "download_id": completion_dl_id,
                                        "error": msg,
                                        "category": "io",
                                        "guidance": guidance,
                                    }),
                                );
                                send_desktop_notification(
                                    &completion_app,
                                    "Download Failed",
                                    &msg,
                                );
                            } else {
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
                            }

                            // Cascade: process the next item in the queue (#455).
                            // The slot was already released a few lines above,
                            // so `next_pending()` will accept the next item.
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
                                let content_label = q
                                    .items
                                    .iter()
                                    .find(|i| i.status.id == dl_id)
                                    .map(|i| format_content_label(&i.status))
                                    .unwrap_or_else(|| "unknown content".to_string());
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!(
                                        "All audio formats exhausted for {content_label} \
                                         — download failed"
                                    ),
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
                            emit_download_error(
                                &app_clone,
                                &dl_id,
                                &format!(
                                    "Filesystem error — check that the output directory \
                                         is accessible and writable: {error_msg}"
                                ),
                            );
                            false
                        }
                        "io_transient" => {
                            // Transient permission-denied on GAMDL's own temp
                            // staging file (#323) — almost always an antivirus /
                            // file-indexer lock on Windows that clears on a retry.
                            // Reuse the bounded network-retry counter so we retry a
                            // couple of times but never loop forever.
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();
                            if let Some((attempt, total)) = q.try_network_retry(&dl_id) {
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!(
                                        "Temp file was briefly locked (antivirus/indexer) — \
                                         retrying (attempt {attempt} of {total})"
                                    ),
                                );
                                true
                            } else {
                                drop(q);
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    "Temp file stayed locked after retries — add MeedyaDL's Temp \
                                     folder (Settings > Tools) to your antivirus exclusions.",
                                );
                                false
                            }
                        }
                        "rate_limit" => {
                            // Apple is throttling license-exchange requests (HTTP
                            // 429) — a server-side, per-account cooldown that
                            // clears in HOURS, not minutes (upstream gamdl#306).
                            // Retrying immediately, or letting the serial queue
                            // march on to the next item, just extends the ban. So
                            // mark this item failed AND pause the queue so the
                            // cascade stops feeding new items into an active 429.
                            // Already-downloaded files are preserved (GAMDL
                            // overwrite=false), so resuming later only fetches the
                            // gap. The `!should_retry` path below additionally
                            // skips the error report, companion spawn, and
                            // auto-retry-without-wrapper for this category.
                            let mut q = queue_clone.lock().await;
                            q.set_error(&dl_id, &error_msg);
                            q.on_task_finished();
                            q.pause();
                            drop(q);
                            emit_app_log(
                                &app_clone,
                                "Apple Music is rate-limiting license requests (HTTP 429). \
                                 Queue paused — this cooldown usually lasts 1–2+ hours. Resume \
                                 from the Queue page once it lifts; already-downloaded files are \
                                 kept, so you'll only re-fetch what's missing.",
                            );
                            let _ = app_clone.emit(
                                "queue-rate-limited",
                                serde_json::json!({ "download_id": dl_id }),
                            );
                            emit_download_error(
                                &app_clone,
                                &dl_id,
                                &format!(
                                    "Rate limited by Apple Music (HTTP 429) — queue paused: \
                                     {error_msg}"
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
                            emit_download_error(
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
                        // Storefront fallback (#666). Try BEFORE wrapper auto-
                        // retry because (a) wrong-storefront and wrapper
                        // failure are different root causes, (b) if the album
                        // simply isn't in the URL's catalog, swapping wrappers
                        // won't help, and (c) we want one user-visible
                        // recovery action per failed item — not two
                        // overlapping retry chains. The detector
                        // `is_storefront_mismatch_error` is narrow (requires
                        // both `404` AND `Resource Not Found`) so we never
                        // burn the budget on a generic 404 from elsewhere.
                        if process::is_storefront_mismatch_error(&error_msg) {
                            let sf_settings = load_settings_for_queue(&app_clone);
                            let swap = {
                                let mut q = queue_clone.lock().await;
                                q.try_storefront_fallback(&dl_id, &sf_settings)
                            };
                            if let Some((from, to)) = swap {
                                log::info!(
                                    "Auto-retrying download {dl_id} via storefront \
                                     fallback ({from} -> {to})"
                                );
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!(
                                        "Storefront '{from}' returned no catalog entry — \
                                         retrying with your account region '{to}'…"
                                    ),
                                );
                                save_queue_to_disk(&app_clone, &queue_clone).await;
                                let _ = app_clone.emit("download-queued", &dl_id);
                                // Skip the terminal error path — the rewritten
                                // URL gets a fresh shot via process_queue.
                                process_queue(app_clone, queue_clone).await;
                                return;
                            }
                        }

                        // Auto-retry without wrapper: if the download used wrapper
                        // auth and the user has opted in, automatically re-queue
                        // with wrapper disabled (cookie-based auth) instead of
                        // treating this as a terminal failure.
                        log::debug!(
                            "Download {dl_id} terminal error path: \
                         wrapper_url={}, error_category={error_category}",
                            wrapper_url_for_logging.as_deref().unwrap_or("none"),
                        );
                        // Skip the cookie auto-retry for rate limits — a 429 is a
                        // per-account server-side cooldown, so re-running via
                        // cookies just fires MORE license requests into the ban.
                        if wrapper_url_for_logging.is_some() && error_category != "rate_limit" {
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
                        // not application bugs, and would just add noise. Also skip
                        // rate_limit — a 429 is a server-side Apple throttle, not a
                        // MeedyaDL bug, so a crash report is pure noise (gamdl#306).
                        if error_category != "network" && error_category != "rate_limit" {
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
                        // also fail and just waste time + clutter the Activity Log)
                        // or a rate limit (companions would fire more license
                        // requests into an active 429 — gamdl#306).
                        if error_category != "network" && error_category != "rate_limit" {
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
/// ## GAMDL 3.1 format note (#607)
///
/// Upstream replaced `traceback.print_exc()` with structlog's
/// `ExceptionPrettyPrinter`, which prints the traceback **before** the
/// `[ERROR HH:MM:SS] …` log line (the processor runs earlier in
/// structlog's pipeline than the formatter). So the output order on
/// v3.1 is:
/// ```text
/// Traceback (most recent call last):
///   File "…/downloader.py", line 123, in download
/// KeyError: 'title'
/// [ERROR    17:09:23] [Track 1/14] Error downloading "Lavender Haze"
/// ```
///
/// Without the structlog-line detection below, the walker would pick up
/// the trailing `[ERROR …]` log line as the "last non-indented line"
/// and return it instead of the real exception. The stop rule treats
/// any line that looks like a fresh structlog entry
/// (`[LEVEL   HH:MM:SS]`) as the end of the traceback block.
///
/// Returns `None` if no traceback is found in the stderr lines.
pub(crate) fn extract_python_exception(stderr_lines: &[String]) -> Option<String> {
    // Find the last occurrence of "Traceback" in stderr
    let traceback_idx = stderr_lines
        .iter()
        .rposition(|line| line.trim().to_lowercase().contains("traceback"))?;

    // Walk forward from the traceback line to find the exception.
    // The exception is the last non-empty, non-indented line in the
    // traceback block — but we stop as soon as a line clearly belongs
    // to a new structlog log entry (see GAMDL 3.1 note above).
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
        // Structlog-formatted log line ends the traceback block (v3.1).
        // Match `[LEVEL...]` where LEVEL is one of the standard log levels
        // GAMDL emits. We don't match bare `[...]` since Python exception
        // messages can contain square brackets (`[Errno 60] Operation timed
        // out`).
        if is_structlog_line_start(trimmed) {
            break;
        }
        // Exception lines are NOT indented (don't start with space/tab).
        // Indented lines are stack frame details (File "...", code).
        if !line.starts_with(' ') && !line.starts_with('\t') {
            exception_line = Some(trimmed);
        }
    }

    exception_line.map(std::string::ToString::to_string)
}

/// Returns `true` when `trimmed` looks like the start of a GAMDL v3.x
/// structlog log entry, i.e. `[LEVEL    HH:MM:SS] …`. Used by
/// [`extract_python_exception`] to detect the end of an exception
/// block on v3.1 where the traceback appears **before** its
/// accompanying log line.
pub(crate) fn is_structlog_line_start(trimmed: &str) -> bool {
    // Fast path: must start with `[` and contain `]`.
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let Some(close_idx) = rest.find(']') else {
        return false;
    };
    let inside = &rest[..close_idx];
    // Recognised level tokens GAMDL emits via structlog.
    for level in ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"] {
        if let Some(after_level) = inside.strip_prefix(level) {
            // Ensure the next char is whitespace (structlog pads with
            // `{level:<8}`) — this guards against false positives like
            // `[ERROR] some bracketed exception text` that isn't actually
            // a log line.
            if after_level.starts_with(' ') || after_level.is_empty() {
                return true;
            }
        }
    }
    false
}

// ============================================================
// M9-7: Spotify (votify) dispatch arm
// ============================================================
//
// The Apple-Music-shaped surface of `run_download_with_events`
// (warnings vector return, idle-watchdog, GamdlOutputEvent parsing
// nuances, soft-error counting, gap-fill cascade) is a wrong fit
// for votify. M9-7 instead routes Spotify items through this
// short, focused arm — re-evaluate the dispatch gate, spawn votify
// via `engine_runner::run_engine_with_queue` (which owns the
// cancellation poll + per-event queue progress update), then on
// success do the post-run count check, increment the daily-cap
// counter, and best-effort write_manifest. Failure path is symmetric.
//
// `process_queue`'s top-of-loop fork (see line ~7180) invokes this
// arm and `continue`s; the arm itself is fire-and-forget — its
// spawned task drives the cascade back into `process_queue` so the
// next item dispatches.

/// Run the entire M9-7 Spotify dispatch lifecycle for one queue item.
///
/// Synchronous (non-spawning) work performed in the caller's task:
///
/// * Load settings.
/// * Re-evaluate the four-outcome dispatch gate. A non-`Allowed`
///   outcome marks the item Error with the gate's message (clearer
///   than a generic "votify failed" surfacing inside the spawned
///   task), releases the queue slot, and triggers the next-item
///   cascade. This closes the crash-restore loophole — restored
///   Spotify items go through the gate again, not just at IPC entry.
/// * Snapshot the pre-run audio-file count for partial-success
///   detection.
/// * Build VotifyOptions from settings.
/// * Build the votify command via `spotify_service::build_votify_command_public`.
///
/// Spawned to a tokio task (so `process_queue` can `continue`):
///
/// * `engine_runner::run_engine_with_queue` — owns the 250 ms
///   cancellation poll loop AND `update_item_progress` per parsed
///   event. The queue row caption ticks during the run.
/// * On `Ok(())`: re-snapshot the audio-file count; if zero new
///   files landed (or the post-run scan failed), mark Error;
///   otherwise mark Complete.
/// * Increment the daily-cap counter by the new-file count.
/// * Write a best-effort manifest (`album_metadata: None`).
/// * `on_task_finished()` releases the slot; `ActiveSlotGuard` is
///   disarmed; `process_queue` cascade fires the next item.
pub(crate) async fn run_spotify_dispatch_arm(
    app: AppHandle,
    queue: QueueHandle,
    download_id: String,
    urls: Vec<String>,
    download_started_at: String,
) {
    // Emit the standard download separator (Spotify-flavoured — the
    // generic separator includes a GAMDL "Codec / Auth" line that
    // would be meaningless for a Spotify item).
    let first_url = urls.first().cloned().unwrap_or_default();
    emit_download_log(
        &app,
        &download_id,
        &format!(
            "[MeedyaDL] ════════════════════════════════════════\n\
             Starting Spotify download: {first_url}\n\
             Engine: votify"
        ),
    );
    emit_download_log(
        &app,
        &download_id,
        "Enrichment skipped — Spotify items use votify-native tagging \
         (Apple Music's metadata pipeline does not apply here)",
    );

    // Load settings for the gate re-validation + VotifyOptions build.
    let settings = match crate::services::config_service::load_settings(&app) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load settings for Spotify dispatch: {e}");
            spotify_arm_terminate_error(
                &app,
                &queue,
                &download_id,
                &format!("Failed to load settings: {e}"),
            )
            .await;
            return;
        }
    };

    // Re-evaluate the dispatch gate. Crash-restored Spotify items
    // bypass the IPC gate at start_download, so the only correct
    // mitigation is to evaluate the same four-outcome gate at the
    // dispatch site too.
    let counter = crate::services::spotify_anti_ban::load_counter(&app);
    let gate_result =
        crate::commands::spotify_anti_ban::evaluate_dispatch_gate(&settings, &counter);
    use crate::commands::spotify_anti_ban::DispatchGateOutcome;
    let gate_error: Option<String> = match gate_result {
        DispatchGateOutcome::Allowed => None,
        DispatchGateOutcome::DevAccessRequired => Some(
            "Spotify dispatch blocked — developer access not enabled. \
             Restored from disk: re-enable dev access to resume."
                .to_string(),
        ),
        DispatchGateOutcome::ConsentRequired => Some(
            "Spotify dispatch blocked — first-run consent not acknowledged.".to_string(),
        ),
        DispatchGateOutcome::MissingSpotifyDll => Some(
            "Spotify dispatch blocked — `session_type=desktop` selected \
             but Spotify desktop DLL path is unset or missing on disk."
                .to_string(),
        ),
        DispatchGateOutcome::MissingWvd => Some(
            "Spotify dispatch blocked — `session_type=web` selected but \
             Widevine `.wvd` path is unset or missing on disk."
                .to_string(),
        ),
        DispatchGateOutcome::DailyCapReached { count, cap } => Some(format!(
            "Spotify dispatch blocked — daily cap reached ({count} / {cap}). \
             Counter resets at local midnight."
        )),
    };
    if let Some(msg) = gate_error {
        emit_download_log(&app, &download_id, &msg);
        spotify_arm_terminate_error(&app, &queue, &download_id, &msg).await;
        return;
    }

    // Snapshot the audio-file count in the output base BEFORE
    // running votify. Mirrors the GAMDL primary-path snapshot
    // (#831) — used to detect partial / total failure via the
    // post-run count delta.
    let output_path = settings.output_path.clone();
    let pre_run_audio_count = count_audio_files_in_directory(
        std::path::Path::new(&output_path),
    );

    // Build VotifyOptions from settings (per-download overrides are
    // M9-9 work — today's queue items dispatch with global settings).
    let votify_options =
        crate::models::votify_options::VotifyOptions::from_settings(
            &settings.service_settings.spotify,
        );

    // Build the actual command. Spawn failures (Python missing, etc.)
    // are surfaced inline rather than wrapped in the spawned task —
    // a "Python isn't installed" error shouldn't live inside an
    // async block the user can't see clearly.
    let cmd = match crate::services::spotify_service::build_votify_command_public(
        &app,
        &urls,
        &votify_options,
    ) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to build votify command: {e}");
            emit_download_log(&app, &download_id, &msg);
            spotify_arm_terminate_error(&app, &queue, &download_id, &msg).await;
            return;
        }
    };

    // Spawn the actual download + post-processing work. The arm
    // returns immediately so `process_queue` can `continue` and
    // pick up the next item.
    let app_clone = app;
    let queue_clone = queue;
    let dl_id = download_id;
    tokio::spawn(async move {
        // ActiveSlotGuard ensures `on_task_finished` is called even
        // if this task panics — preventing a permanent queue stall.
        let guard = ActiveSlotGuard::new(queue_clone.clone());

        // Run votify via the queue-aware runner (cancellation poll +
        // per-event update_item_progress).
        let result = crate::services::engine_runner::run_engine_with_queue(
            &app_clone,
            &dl_id,
            "votify",
            cmd,
            queue_clone.clone(),
        )
        .await;

        match result {
            Ok(()) => {
                // Re-scan the output tree. The delta = how many new
                // audio files actually landed.
                let post_run_audio_count = count_audio_files_in_directory(
                    std::path::Path::new(&output_path),
                );
                let new_files = post_run_audio_count
                    .saturating_sub(pre_run_audio_count);

                if new_files == 0 {
                    // Zero new files despite a clean exit — partial-
                    // success critique-amendment case. Mark Error so
                    // the user sees the failure rather than a "Complete"
                    // badge on an empty folder.
                    let msg = "votify exited cleanly but produced no new audio \
                               files — every track failed (likely auth, region \
                               lock, or premium-only content)".to_string();
                    emit_download_log(&app_clone, &dl_id, &msg);
                    let mut q = queue_clone.lock().await;
                    q.set_error(&dl_id, &msg);
                } else {
                    emit_download_log(
                        &app_clone,
                        &dl_id,
                        &format!(
                            "Spotify download complete — {new_files} new audio file(s) landed"
                        ),
                    );

                    // Increment the daily-cap counter by the actual
                    // landed-file count. The cap-check at IPC gate
                    // ran ONCE per batch and can be overshot by
                    // hundreds of tracks on a near-cap dispatch
                    // (critique amendment 3 — known limitation; rich
                    // per-track instrumentation lands in M9-8).
                    let cap = settings.service_settings.spotify.anti_ban.daily_download_cap;
                    let new_files_u32 = u32::try_from(new_files).unwrap_or(u32::MAX);
                    match crate::services::spotify_anti_ban::increment_counter(
                        &app_clone,
                        new_files_u32,
                    ) {
                        Ok(counter) => {
                            if cap != 0 && counter.count > cap {
                                log::warn!(
                                    "Spotify daily-cap overshoot: counter={} cap={cap} (M9-8 will add per-track cap enforcement)",
                                    counter.count
                                );
                                emit_download_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!(
                                        "⚠ Daily cap exceeded ({}/{cap}) — \
                                         next Spotify download will be blocked until midnight",
                                        counter.count
                                    ),
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to persist daily-cap counter: {e}");
                        }
                    }

                    // Best-effort manifest write so Library Scan sees
                    // the album. We can only resolve the album_dir
                    // via deepest-audio-dir heuristics today (votify
                    // writes to its own template); when that fails,
                    // skip the manifest with a WARN — Library Scan
                    // misses the entry but the download is still
                    // complete.
                    write_spotify_manifest_best_effort(
                        &app_clone,
                        &dl_id,
                        &urls,
                        &output_path,
                        &settings,
                        &download_started_at,
                    );

                    let mut q = queue_clone.lock().await;
                    q.set_complete(&dl_id);
                }
            }
            Err(msg) => {
                // The cancellation sentinel is short-circuited by the
                // existing set_error guard — Cancelled state is
                // preserved (#661). All other errors flip to Error.
                if msg != "Download cancelled by user" {
                    emit_download_log(
                        &app_clone,
                        &dl_id,
                        &format!("Spotify download failed: {msg}"),
                    );
                }
                let mut q = queue_clone.lock().await;
                q.set_error(&dl_id, &msg);
            }
        }

        // Release the queue slot. Match the explicit order used by
        // the GAMDL completion task: call `on_task_finished` FIRST,
        // then `guard.disarm()` so the guard's Drop is a no-op
        // (prevents double-decrement if the task panics between the
        // two calls).
        {
            let mut q = queue_clone.lock().await;
            q.on_task_finished();
        }
        guard.disarm();

        // Cascade — fire-and-forget process_queue so the next item
        // dispatches. Spawn (not await) to avoid stack-deepening
        // recursion through long Spotify queues.
        tokio::spawn(async move {
            process_queue(app_clone, queue_clone).await;
        });
    });
}

/// Helper for `run_spotify_dispatch_arm`: handle the inline
/// terminate-error path (gate block, settings load failure, command
/// build failure). Sets Error state, releases the slot, cascades.
pub(crate) async fn spotify_arm_terminate_error(
    app: &AppHandle,
    queue: &QueueHandle,
    download_id: &str,
    msg: &str,
) {
    {
        let mut q = queue.lock().await;
        q.set_error(download_id, msg);
        q.on_task_finished();
    }
    let app_clone = app.clone();
    let queue_clone = queue.clone();
    tokio::spawn(async move {
        process_queue(app_clone, queue_clone).await;
    });
}

/// Best-effort manifest write for a Spotify item.
///
/// Tries to resolve the album_dir via `find_deepest_audio_dir`
/// rooted at the output path. If resolution fails, logs at WARN and
/// returns — the download is still complete; Library Scan just
/// won't index this album.
///
/// `album_metadata: None` means the manifest carries no per-track
/// ISRC / song_id / Apple-Music-specific fields. Full Spotify
/// metadata-aware manifests land in M9-9 alongside the queue-row
/// artist/album pre-fetch.
pub(crate) fn write_spotify_manifest_best_effort(
    _app: &AppHandle,
    _download_id: &str,
    urls: &[String],
    output_path: &str,
    settings: &crate::models::settings::AppSettings,
    download_started_at: &str,
) {
    let base = std::path::Path::new(output_path);
    // find_deepest_audio_dir uses in-out style: caller declares the
    // `best` accumulator, the function recurses and mutates it.
    // Mirrors the GAMDL recovery-path locator pattern.
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    find_deepest_audio_dir(base, &mut best, 0);
    let Some((_, album_dir)) = best else {
        log::warn!(
            "Spotify manifest write skipped: could not resolve album_dir under {}",
            base.display()
        );
        return;
    };

    let Some(album_dir_str) = album_dir.to_str() else {
        log::warn!(
            "Spotify manifest write skipped: album_dir path is not valid UTF-8: {}",
            album_dir.display()
        );
        return;
    };
    // write_manifest returns `()` and logs failures internally —
    // we don't need to handle a Result here.
    write_manifest(
        album_dir_str,
        urls,
        None, // album_metadata — Spotify items have none today
        settings,
        download_started_at,
        None, // cross_platform_urls — M9-3 best-cover-art only populates this for the album-art path
        // primary_codec_id — votify ships Ogg Vorbis by default. Must be the
        // canonical codec-registry ID ("ogg-vorbis", the `[audio.ogg-vorbis]`
        // section key in codecs.toml — see its `services.votify = "ogg-vorbis"`
        // mapping), not the bare format name ("vorbis"), so any future
        // codec-registry lookup against `ManifestSource.codec` (e.g. Library
        // Scan's codec badge, a future Spotify companion-tier diff) resolves
        // instead of silently missing (A2 fix).
        Some("ogg-vorbis"),
        None, // companion_tiers — votify has no codec companions
    );
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
pub(crate) async fn run_download_with_events(
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
    let requested_format_context = requested_format_cli_values(options);
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
    //
    // **Bounded as of #893.** Pre-#893 this was an unbounded `Vec<String>`;
    // for verbose / multi-album downloads with thousands of codec-skip
    // warnings the Vec could grow into the MB range per download.
    // The bounded ring buffer retains the last
    // `DEFAULT_LINE_CAP_ERRORS` lines and truncates any individual
    // line past `DEFAULT_LINE_BYTE_CAP`.
    let collected_errors: Arc<Mutex<crate::utils::bounded_log::BoundedLineBuffer>> =
        Arc::new(Mutex::new(crate::utils::bounded_log::BoundedLineBuffer::for_errors()));

    // Collect ALL output lines (raw) from BOTH stdout and stderr for
    // post-mortem analysis: Python traceback extraction, soft-error
    // friendly-message generation, and storefront-mismatch detection
    // for the #666 fallback path.
    //
    // Originally stderr-only; expanded to cover stdout in a fix to the
    // #666 blind spot exposed by GAMDL v3.4 (2026-04-27), which moved
    // its logging output stream from stderr → stdout via
    // `structlog.PrintLoggerFactory(file=CustomOutputWriter([sys.stdout]))`.
    // Tracebacks and the AMP `Resource Not Found` shape that
    // `is_storefront_mismatch_error` keys on now arrive on stdout, so a
    // stderr-only buffer leaves the detector seeing empty input and the
    // storefront fallback never fires.
    //
    // **Bounded as of #893.** Pre-#893 this was an unbounded
    // `Vec<String>` that the 2026-05-28 memory audit identified as
    // the largest single contributor to RSS growth (~50 MB per
    // verbose multi-album download). Retains the last
    // `DEFAULT_LINE_CAP_RAW` lines; all consumers
    // (`traceback_diagnostic::write_diagnostic_if_any`,
    // `is_storefront_mismatch_error`, soft-error classifier) scan the
    // newest lines so the eviction policy doesn't hide signal.
    let raw_output_lines: Arc<Mutex<crate::utils::bounded_log::BoundedLineBuffer>> =
        Arc::new(Mutex::new(crate::utils::bounded_log::BoundedLineBuffer::for_raw_output()));

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
        // GAMDL v3.4+ logs to stdout, so stdout must also feed the
        // raw output buffer used by the soft-error friendly-message
        // generator and the storefront-mismatch detector (#666).
        let raw_output = raw_output_lines.clone();
        let requested_formats = requested_format_context.clone();
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
                    let display_line =
                        annotate_unavailable_format_line(&clean_line, &requested_formats);
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
                            // Suppress Python traceback noise from the user-
                            // facing activity-log feed when verbose is off
                            // (#660). Same gate for ffprobe demuxing noise
                            // (#847). The helper unconditionally writes to
                            // disk so support requests stay debuggable.
                            let is_known_noise =
                                process::is_python_traceback_noise(&clean_line)
                                    || process::is_ffprobe_demux_noise(&clean_line);
                            // Phase 3.5h: humanise GAMDL "codec skip" lines
                            // — strip "(media ID: NNN)" and the Python-repr
                            // codec list. Idempotent + safe on non-matching
                            // lines.
                            let humanised = process::humanise_codec_skip_line(&display_line);
                            crate::utils::activity_log::emit_subprocess_line(
                                &app,
                                &download_id,
                                "stdout",
                                humanised,
                                verbose || !is_known_noise,
                            );
                        }
                    }

                    // Mirror clean_line into the shared raw-output buffer so
                    // GAMDL v3.4+'s stdout-emitted tracebacks and AMP 404s
                    // are visible to `is_storefront_mismatch_error` (#666)
                    // and `classify_gamdl_traceback` (#660 friendly path).
                    {
                        let mut raw = raw_output.lock().await;
                        raw.push(clean_line.clone()); // #893: bounded ring buffer
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
                        // Phase 3.5e: track-separator banner now goes through
                        // `emit_subprocess_line` with `stream: "internal"` so
                        // it's consistent with every other internal event.
                        // Disk mirror happens unconditionally inside the
                        // helper.
                        crate::utils::activity_log::emit_subprocess_line(
                            &app,
                            &download_id,
                            "internal",
                            format!(
                                "──── {track_label} Downloading {context}\"{title}\" ────"
                            ),
                            true,
                        );
                    }

                    // Update the queue item's progress
                    {
                        let mut q = queue.lock().await;
                        q.update_item_progress(&download_id, &event);
                    }

                    // Collect errors for fallback decisions. CodecSkip events
                    // (#698) are also pushed so the existing `is_codec_error`
                    // / `count_codec_skip_warnings` checks in the terminal
                    // block keep working unchanged — the queue's classifier
                    // distinguishes "all errors are codec skips" via
                    // `is_codec_skip_message` at decision time.
                    match event {
                        process::GamdlOutputEvent::Error { ref message }
                        | process::GamdlOutputEvent::CodecSkip { ref message } => {
                            let mut errs = errors.lock().await;
                            errs.push(message.clone());
                        }
                        _ => {}
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
        let raw_output = raw_output_lines.clone();
        let seen = seen_lines.clone();
        let last_activity = last_activity_ms.clone();
        let requested_formats = requested_format_context.clone();
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
                    let display_line =
                        annotate_unavailable_format_line(&clean_line, &requested_formats);
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
                            // Suppress Python traceback noise (#660) and
                            // recurring ffprobe demuxing-error lines (#847)
                            // from the user-facing activity-log feed in
                            // non-verbose mode. The helper unconditionally
                            // writes to disk so support requests stay
                            // debuggable.
                            let is_known_noise =
                                process::is_python_traceback_noise(&clean_line)
                                    || process::is_ffprobe_demux_noise(&clean_line);
                            // Phase 3.5h: humanise GAMDL "codec skip" lines
                            // (strips "(media ID: NNN)" + Python-repr codec
                            // list).
                            let humanised = process::humanise_codec_skip_line(&display_line);
                            crate::utils::activity_log::emit_subprocess_line(
                                &app,
                                &download_id,
                                "stderr",
                                humanised,
                                verbose || !is_known_noise,
                            );
                        }
                    }

                    let event = process::parse_gamdl_output(&clean_line);

                    // Mirror to the shared raw output buffer (#666 detector
                    // and #660 friendly-traceback path read this on the
                    // soft-error gate). The stdout reader writes to the
                    // same Arc<Mutex>, so the buffer ends up containing
                    // both streams, which is what the consumers need.
                    {
                        let mut raw = raw_output.lock().await;
                        raw.push(clean_line.clone());
                    }

                    {
                        let mut q = queue.lock().await;
                        q.update_item_progress(&download_id, &event);
                    }

                    // Collect errors + codec-skips for fallback decisions
                    // (#698 — same compatibility-shim rationale as the stdout
                    // reader above).
                    match event {
                        process::GamdlOutputEvent::Error { ref message }
                        | process::GamdlOutputEvent::CodecSkip { ref message } => {
                            let mut errs = errors.lock().await;
                            errs.push(message.clone());
                        }
                        _ => {}
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

    // Python traceback diagnostic capture (#758). Runs once per GAMDL
    // invocation regardless of exit status — some traceback patterns
    // (notably the cover-bytes fetch failure on `cover_format = raw`)
    // fire on successful downloads too. The helper is a no-op when no
    // tracebacks are present, so the healthy-download fast path is
    // a single buffer scan + an early return. The URL is redacted to
    // strip wrapper auth tokens before being stored in the report.
    {
        let raw_snapshot = raw_output_lines.lock().await.snapshot(); // #893: bounded
        let item_state = if status.success() {
            "complete"
        } else {
            "error"
        };
        let url_for_report = urls
            .first()
            .map(|u| redact_url_query(u))
            .unwrap_or_default();
        let gamdl_version = crate::services::gamdl_capabilities::detected_version();
        // `url_for_report` is an owned `String` (redact_url_query now
        // delegates to `crash_report_service::redact_single_url`, which
        // also strips `user:pass@` userinfo and therefore must return an
        // owned value). `write_diagnostic_if_any` takes `url: &str`, so
        // borrow here.
        let outcome = crate::services::traceback_diagnostic::write_diagnostic_if_any(
            app,
            download_id,
            &url_for_report,
            gamdl_version.as_deref(),
            item_state,
            &raw_snapshot,
        );
        match outcome {
            Ok(0) => {
                // Healthy path — no tracebacks observed.
            }
            Ok(n) => {
                emit_download_log(
                    app,
                    download_id,
                    &format!(
                        "Captured {n} distinct Python traceback(s) — see Settings → Advanced → Crash Reporting for the diagnostic report"
                    ),
                );
            }
            Err(e) => {
                log::warn!(
                    "Traceback diagnostic write failed for {download_id}: {e}"
                );
            }
        }
    }

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
        // #893: snapshot the bounded ring buffer to a Vec we own,
        // then drop the lock before scanning. The traceback /
        // storefront detectors are CPU-bound on the joined string;
        // we don't need to hold the buffer lock while they run.
        let raw_lines = raw_output_lines.lock().await.snapshot();
        let combined = raw_lines.join("\n");

        // GAMDL music-video cover-template bug detection. Checked
        // BEFORE the storefront detector because the bug's symptom
        // (400 Bad Request on a literal `{w}x{h}` URL) is highly
        // specific and gives the user a clear, actionable message
        // instead of the generic per-track-error count. Storefront
        // fallback wouldn't help here — the URL works fine, GAMDL's
        // template engine is at fault.
        //
        // **v3.5.2 status (#774, 2026-05-15)**: still NOT fixed
        // upstream. The previous client-side retry that appended
        // `cover` to `--exclude-tags` (#715) was removed once we
        // verified against GAMDL 3.5.2 source that `--exclude-tags`
        // only filters which tag KEYS get embedded — it doesn't
        // skip the per-track cover URL FETCH that triggers the
        // bug (`gamdl/downloader/music_video.py:202-208` runs the
        // fetch unconditionally for non-RAW cover formats; the RAW
        // path also fetches via `_get_cover_file_extension`). No
        // GAMDL CLI flag combination can avoid the bad request, so
        // we now report the failure clearly and stop. MeedyaDL's
        // own `animated_artwork_service` still attaches the
        // album-level cover during enrichment, so the album cover
        // is preserved — only the per-track music-video frame
        // thumbnail is lost.
        if process::is_gamdl_mv_cover_template_bug(&combined) {
            return Err(format!(
                "Music video cover-art bug in GAMDL — {soft_errors} track(s) \
                 skipped. Audio for those tracks did not download. This is \
                 an upstream bug (Apple returns 400 Bad Request because \
                 GAMDL sends literal `{{w}}x{{h}}` placeholders instead of \
                 real dimensions). The album cover is still attached \
                 separately during MeedyaDL's enrichment pass. Please \
                 report at https://github.com/glomatico/gamdl/issues."
            ));
        }

        // Storefront-mismatch detection (#666). The friendly soft-error
        // message would otherwise hide the AMP "Resource Not Found" signal
        // that the storefront-fallback retry path keys on. When the raw
        // output looks like a wrong-storefront failure, prepend the
        // marker so `is_storefront_mismatch_error` matches downstream.
        // Reads from `raw_output_lines` (both stdout and stderr) since
        // GAMDL v3.4+ emits its log lines to stdout, not stderr.
        let storefront_signal = if process::is_storefront_mismatch_error(&combined) {
            " — AMP API returned 404 Resource Not Found (likely wrong storefront)"
        } else {
            ""
        };
        let friendly = process::classify_gamdl_traceback(&combined)
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| {
                format!(
                    "GAMDL reported {soft_errors} per-track error(s) even though the process exited 0{storefront_signal}"
                )
            });
        return Err(friendly);
    }

    if status.success() {
        // Return any error-pattern lines as non-fatal warnings.
        // These are issues GAMDL logged during the run but didn't consider
        // fatal enough to exit with a non-zero code.
        // #893: snapshot the bounded buffer so the return value owns
        // its strings — the buffer's lock guard can't escape this
        // function and we want the Vec<String> shape for callers.
        let warnings = collected_errors.lock().await.snapshot();
        Ok(warnings)
    } else {
        // Use the last collected error message from GAMDL's output for a meaningful
        // error message. This is more informative than just "exited with code N".
        // The error message is also used by classify_error() to determine the
        // retry/fallback strategy (codec error vs network error vs unknown).
        //
        // #893: lock both buffers in scope, then snapshot to Vec<String>
        // so we can `join` / pass slices to `extract_python_exception`
        // without retaining the locks across the analysis.
        let last_error_opt = collected_errors.lock().await.last().cloned();
        let raw_lines = raw_output_lines.lock().await.snapshot();

        last_error_opt.map_or_else(
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
                    Err(last_error)
                }
            },
        )
    }
}

