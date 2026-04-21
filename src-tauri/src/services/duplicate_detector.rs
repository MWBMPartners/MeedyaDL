// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// duplicate_detector.rs — Pre-queue duplicate detection (#510)
// ============================================================
//
// When an Apple Music artist URL is queued with multiple
// `artist_auto_select_multi` modes (e.g. main-albums + singles-eps +
// compilation-albums), GAMDL would otherwise download the same song
// multiple times at the same quality: once as a studio-album track,
// once as a single, once on a "Greatest Hits" compilation.
//
// This service resolves that pre-queue by:
//   1. Enumerating every album for the artist via the Apple Music
//      catalog API.
//   2. Bucketing each album into a GAMDL auto-select category (main,
//      singles/EPs, compilation, live) using API flags + name
//      heuristics.
//   3. Fetching track metadata per album — retrieving `song_id` + ISRC
//      per track.
//   4. Walking the user's preference-ordered mode list and assigning
//      each track to the first mode that contains it. Later modes
//      have the duplicate skipped.
//   5. Also matching against song keys already in the queue / prior
//      downloads (per the configured scope), so a user who queues
//      `singles-eps` today after already downloading the album
//      yesterday doesn't re-download the shared tracks.
//
// ## Scope guarantee
//
// This service operates on **track identity** (song_id / ISRC) only.
// Companion-format downloads (ALAC / Atmos / AAC, etc., controlled by
// `companion_mode`) are untouched — a song chosen by dedup still runs
// the user's full companion chain. The feature eliminates same-quality
// duplicates, not format-variant duplicates.
//
// ## Failure handling
//
// Any API failure (no credentials, rate-limit, network error) produces
// a `DedupPlan::FallbackToArtistUrl` outcome for the affected modes,
// falling back to the pre-feature behaviour. The download is never
// blocked by this service.

use std::collections::{HashMap, HashSet};

use tauri::AppHandle;

use crate::models::gamdl_options::ArtistAutoSelect;
use crate::models::settings::{
    AppSettings, DedupKeyStrategy, DuplicateDetectionScope, DuplicateDetectionSettings,
};
use crate::services::apple_music_api::{self, ArtistAlbumRef, ParsedAppleMusicUrl, TrackMetadata};

// ============================================================
// Public output types
// ============================================================

/// The dedup plan for a single artist-auto-select mode.
#[derive(Debug, Clone)]
pub enum ModePlan {
    /// Replace the original artist URL with this explicit list of
    /// track URLs (format: `{album_url}?i={song_id}`). Empty vec is
    /// represented by [`Self::SkipEntirely`] instead.
    TrackUrls(Vec<String>),
    /// Dedup was not usable for this mode (API failure, no credentials,
    /// classifier couldn't resolve the mode). Fall through to the
    /// original artist URL — behaviour is unchanged vs pre-feature.
    FallbackToArtistUrl,
    /// Every track this mode would have produced is a duplicate of a
    /// higher-priority mode or an existing queue/history entry. Don't
    /// enqueue a download for this mode at all.
    SkipEntirely,
}

/// The combined plan produced by [`plan_artist_deduplication`].
#[derive(Debug, Clone)]
pub struct DedupPlan {
    /// One plan per mode the user selected. Order matches the input
    /// `modes` slice passed in.
    pub per_mode: Vec<(ArtistAutoSelect, ModePlan)>,
    /// Human-readable lines to emit to the activity log ("Duplicate
    /// skipped: {title} — kept {A}, skipped {B}"). Emitted by the
    /// caller in whatever context is appropriate.
    pub activity_lines: Vec<String>,
    /// Total number of tracks deduplicated across all modes (diagnostic).
    pub skipped_count: usize,
}

// ============================================================
// Entry point
// ============================================================

/// Plan duplicate detection for a multi-mode artist URL download.
///
/// Returns `None` when dedup is disabled (`scope: Off`), the URL isn't
/// an artist URL, MusicKit credentials aren't configured, or the mode
/// list has fewer than two modes (nothing to deduplicate against).
///
/// When `Some(plan)` is returned, the caller should use
/// `plan.per_mode` to decide what to enqueue per mode. See the
/// [`ModePlan`] variants.
///
/// # Arguments
/// * `_app` — Reserved for future activity-log emission; currently
///   unused because the caller emits using the returned
///   `activity_lines` with proper context.
/// * `parsed` — Parsed artist URL (content_type must be "artist").
/// * `artist_url` — Original URL string (used as fallback identifier).
/// * `modes` — User-selected `artist_auto_select_multi` values, in the
///   order the fan-out iterates them.
/// * `settings` — Full settings bundle (for MusicKit creds, dedup
///   config, and scope-history output_path).
/// * `queued_keys` — Track keys already present in the active queue.
///   Supply an empty set if the scope excludes the queue.
/// * `history_keys` — Track keys from prior download manifests. Supply
///   an empty set if the scope excludes history.
pub async fn plan_artist_deduplication(
    _app: &AppHandle,
    parsed: &ParsedAppleMusicUrl,
    artist_url: &str,
    modes: &[ArtistAutoSelect],
    settings: &AppSettings,
    queued_keys: &HashSet<String>,
    history_keys: &HashSet<String>,
) -> Option<DedupPlan> {
    let cfg = &settings.duplicate_detection;

    // Fast-path: feature disabled or not enough modes to possibly dedupe.
    if matches!(cfg.scope, DuplicateDetectionScope::Off) {
        log::debug!("Dedup: scope=Off, skipping");
        return None;
    }
    if modes.len() < 2 && queued_keys.is_empty() && history_keys.is_empty() {
        log::debug!("Dedup: only {} mode(s) selected and no queue/history, skipping", modes.len());
        return None;
    }
    if parsed.content_type != "artist" || parsed.artist_id.is_none() {
        log::debug!("Dedup: URL is not an artist URL, skipping");
        return None;
    }

    let artist_id = parsed.artist_id.as_deref().unwrap_or("");
    let storefront = &parsed.storefront;

    // Resolve the MusicKit developer token — same priority chain as
    // enrichment (user creds → embedded → web-session). If nothing
    // resolves, we can't fetch from the catalog, so fall back.
    let jwt = match resolve_jwt(settings) {
        Some(jwt) => jwt,
        None => {
            log::info!(
                "Dedup: no MusicKit token available for artist {artist_id}, falling back to per-mode artist URLs"
            );
            return Some(fallback_plan(modes));
        }
    };

    // Fetch every album for this artist once — the list is the same
    // regardless of which subset of modes the user picked.
    let albums = match apple_music_api::fetch_artist_albums(&jwt, storefront, artist_id).await {
        Ok(albums) => albums,
        Err(e) => {
            log::warn!("Dedup: artist-albums fetch failed for {artist_id}: {e}");
            return Some(fallback_plan(modes));
        }
    };

    if albums.is_empty() {
        log::info!("Dedup: artist {artist_id} returned no albums — nothing to dedupe");
        return Some(fallback_plan(modes));
    }

    // For each user-selected mode, select the albums that belong to it.
    // `music-videos` is out of scope (not audio) — always falls through.
    // `top-songs` is also out of scope for this iteration (it's a
    // separate endpoint and would require per-song metadata — deferred
    // as a follow-up).
    let mut mode_to_albums: HashMap<ArtistAutoSelect, Vec<&ArtistAlbumRef>> = HashMap::new();
    let mut unhandled_modes: HashSet<ArtistAutoSelect> = HashSet::new();

    for mode in modes {
        match mode {
            ArtistAutoSelect::MusicVideos | ArtistAutoSelect::TopSongs => {
                unhandled_modes.insert(mode.clone());
            }
            ArtistAutoSelect::AllAlbums => {
                mode_to_albums.insert(mode.clone(), albums.iter().collect());
            }
            bucket_mode => {
                let bucketed: Vec<&ArtistAlbumRef> = albums
                    .iter()
                    .filter(|a| &apple_music_api::classify_album_bucket(a) == bucket_mode)
                    .collect();
                mode_to_albums.insert(mode.clone(), bucketed);
            }
        }
    }

    // Fetch track metadata per album, lazily + cached: the same album
    // could appear under several modes (e.g. all-albums + main-albums).
    let mut album_tracks_cache: HashMap<String, Vec<TrackMetadata>> = HashMap::new();
    let mut mode_tracks: HashMap<ArtistAutoSelect, Vec<TrackInMode>> = HashMap::new();

    for (mode, mode_albums) in &mode_to_albums {
        let mut tracks: Vec<TrackInMode> = Vec::new();
        for album in mode_albums {
            let cached = if let Some(cached) = album_tracks_cache.get(&album.album_id) {
                cached.clone()
            } else {
                match apple_music_api::fetch_album_metadata_with_fallback(
                    &jwt,
                    storefront,
                    &album.album_id,
                )
                .await
                {
                    Ok(Some(meta)) => {
                        album_tracks_cache.insert(album.album_id.clone(), meta.tracks.clone());
                        meta.tracks
                    }
                    Ok(None) => {
                        log::debug!(
                            "Dedup: album {} returned no data, skipping",
                            album.album_id
                        );
                        Vec::new()
                    }
                    Err(e) => {
                        log::warn!("Dedup: album {} fetch failed: {e}", album.album_id);
                        Vec::new()
                    }
                }
            };
            for track in cached {
                if let Some(key) = build_track_key(&track, cfg.key_strategy) {
                    tracks.push(TrackInMode {
                        key,
                        song_id: track.song_id.clone(),
                        album_id: album.album_id.clone(),
                        album_name: album.name.clone(),
                        title: track.name.clone(),
                    });
                }
            }
        }
        mode_tracks.insert(mode.clone(), tracks);
    }

    // Walk modes in preference order (including fallback to natural
    // order for modes not present in the preference list). For each
    // track key, the first mode to contain it wins; later modes have
    // that key added to a skip set.
    let preference = preference_order(&cfg.preference_order, modes);
    let mut claimed: HashSet<String> = HashSet::new();
    let mut activity_lines: Vec<String> = Vec::new();
    let mut skipped_count = 0usize;
    // Per-mode kept track set → used below to build the output URLs.
    let mut kept_per_mode: HashMap<ArtistAutoSelect, Vec<TrackInMode>> = HashMap::new();

    for mode in &preference {
        let Some(tracks) = mode_tracks.get(mode) else {
            continue;
        };
        let mut kept = Vec::new();
        let mut seen_in_this_mode: HashSet<String> = HashSet::new();
        for track in tracks {
            // Deduplicate within a single mode too — rare, but possible
            // if the same song appears on multiple albums GAMDL would
            // have downloaded under the same mode.
            if !seen_in_this_mode.insert(track.key.clone()) {
                skipped_count += 1;
                activity_lines.push(format!(
                    "Dedup: skipped duplicate \"{}\" inside {} (same song on multiple albums)",
                    track.title,
                    mode.display_name()
                ));
                continue;
            }
            // Cross-session: does it already exist in the queue or on
            // disk (depending on scope)?
            if queued_keys.contains(&track.key) {
                skipped_count += 1;
                activity_lines.push(format!(
                    "Dedup: skipped \"{}\" ({}) — already in download queue",
                    track.title,
                    mode.display_name()
                ));
                continue;
            }
            if history_keys.contains(&track.key) {
                skipped_count += 1;
                activity_lines.push(format!(
                    "Dedup: skipped \"{}\" ({}) — previously downloaded (history)",
                    track.title,
                    mode.display_name()
                ));
                continue;
            }
            // Intra-session: did an earlier (higher-priority) mode
            // already claim this song?
            if !claimed.insert(track.key.clone()) {
                skipped_count += 1;
                activity_lines.push(format!(
                    "Dedup: skipped \"{}\" ({}) — kept higher-priority version",
                    track.title,
                    mode.display_name()
                ));
                continue;
            }
            kept.push(track.clone());
        }
        kept_per_mode.insert(mode.clone(), kept);
    }

    // Build the output plan per user-selected mode.
    let mut per_mode: Vec<(ArtistAutoSelect, ModePlan)> = Vec::new();
    for mode in modes {
        if unhandled_modes.contains(mode) {
            // Classifier couldn't handle this mode → unchanged behaviour.
            per_mode.push((mode.clone(), ModePlan::FallbackToArtistUrl));
            continue;
        }
        let Some(kept) = kept_per_mode.get(mode) else {
            // No albums matched this mode's bucket — keep behaviour
            // unchanged (GAMDL will emit its own "no content" message).
            per_mode.push((mode.clone(), ModePlan::FallbackToArtistUrl));
            continue;
        };
        if kept.is_empty() {
            per_mode.push((mode.clone(), ModePlan::SkipEntirely));
            continue;
        }
        // Build per-track URLs in the "{album_url}?i={song_id}" form
        // that GAMDL recognises for single-track downloads.
        let urls: Vec<String> = kept
            .iter()
            .filter_map(|t| build_track_url(storefront, &t.album_id, &t.song_id))
            .collect();
        if urls.is_empty() {
            per_mode.push((mode.clone(), ModePlan::FallbackToArtistUrl));
        } else {
            per_mode.push((mode.clone(), ModePlan::TrackUrls(urls)));
        }
    }

    log::info!(
        "Dedup: artist {artist_id} — {} mode(s) planned, {} duplicate track(s) skipped",
        per_mode.len(),
        skipped_count
    );
    log::debug!("Dedup: input artist URL was {artist_url}");

    Some(DedupPlan {
        per_mode,
        activity_lines,
        skipped_count,
    })
}

// ============================================================
// Playlist dedup (#512)
// ============================================================

/// Dedup outcome for a single playlist URL.
#[derive(Debug, Clone)]
pub enum PlaylistPlan {
    /// Replace the playlist URL with this explicit list of per-track URLs.
    /// Only the non-duplicate subset of the playlist will be downloaded.
    TrackUrls(Vec<String>),
    /// Every track on the playlist matched a queued / history key.
    /// Don't enqueue the playlist at all.
    SkipEntirely,
    /// Dedup couldn't operate (no MusicKit token, API error, library
    /// playlist, playlist returned zero usable tracks). Keep the original
    /// playlist URL — behaviour is unchanged vs pre-feature.
    FallbackToPlaylistUrl,
}

/// Result of [`plan_playlist_deduplication`].
#[derive(Debug, Clone)]
pub struct PlaylistDedupPlan {
    /// What to do with the playlist URL.
    pub plan: PlaylistPlan,
    /// Human-readable "Duplicate skipped" lines — emitted by the caller
    /// once it has download-event context.
    pub activity_lines: Vec<String>,
    /// Count of tracks skipped (diagnostic).
    pub skipped_count: usize,
    /// Count of tracks kept (diagnostic).
    pub kept_count: usize,
}

/// Plan duplicate detection for a catalog playlist URL.
///
/// Returns `None` when the feature is disabled, the URL isn't a catalog
/// playlist, or there's nothing to compare against (empty queue + empty
/// history). The caller falls through to the default enqueue path in
/// those cases.
///
/// When `Some(plan)` is returned, the caller should consult
/// [`PlaylistDedupPlan::plan`] to decide what URL list to enqueue. See
/// the [`PlaylistPlan`] variants.
///
/// # Arguments
/// * `_app` — Reserved for future activity-log plumbing; callers emit
///   `activity_lines` directly with the appropriate context.
/// * `parsed` — Parsed URL; `content_type` must be `"playlist"` and
///   `playlist_id` must be `Some(pl.xxxx)`.
/// * `settings` — Full settings bundle (for the MusicKit token chain
///   and the dedup configuration).
/// * `queued_keys` — Song keys already present in the active queue.
/// * `history_keys` — Song keys from prior download manifests.
pub async fn plan_playlist_deduplication(
    _app: &AppHandle,
    parsed: &ParsedAppleMusicUrl,
    settings: &AppSettings,
    queued_keys: &HashSet<String>,
    history_keys: &HashSet<String>,
) -> Option<PlaylistDedupPlan> {
    let cfg = &settings.duplicate_detection;

    // Fast-path: disabled, not a playlist URL, or nothing to dedupe against.
    if matches!(cfg.scope, DuplicateDetectionScope::Off) {
        return None;
    }
    if parsed.content_type != "playlist" {
        return None;
    }
    let Some(playlist_id) = parsed.playlist_id.as_deref() else {
        return None;
    };
    if queued_keys.is_empty() && history_keys.is_empty() {
        // Without external comparison sources there's nothing for a
        // playlist to dedupe against (unlike the artist planner which
        // dedupes across modes in the same request).
        log::debug!(
            "Playlist dedup: no queued/history keys — nothing to compare against for {playlist_id}"
        );
        return None;
    }

    // Resolve JWT. Without it we can't fetch the playlist's tracks.
    let Some(jwt) = resolve_jwt(settings) else {
        log::info!(
            "Playlist dedup: no MusicKit token available for {playlist_id}, keeping original URL"
        );
        return Some(PlaylistDedupPlan {
            plan: PlaylistPlan::FallbackToPlaylistUrl,
            activity_lines: Vec::new(),
            skipped_count: 0,
            kept_count: 0,
        });
    };

    // Fetch the playlist's tracks (paginated). On failure, graceful
    // fallback — never block the download.
    let tracks = match apple_music_api::fetch_playlist_tracks(&jwt, &parsed.storefront, playlist_id)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            log::warn!("Playlist dedup: fetch_playlist_tracks failed for {playlist_id}: {e}");
            return Some(PlaylistDedupPlan {
                plan: PlaylistPlan::FallbackToPlaylistUrl,
                activity_lines: Vec::new(),
                skipped_count: 0,
                kept_count: 0,
            });
        }
    };

    if tracks.is_empty() {
        log::info!(
            "Playlist dedup: playlist {playlist_id} returned zero usable tracks — falling back"
        );
        return Some(PlaylistDedupPlan {
            plan: PlaylistPlan::FallbackToPlaylistUrl,
            activity_lines: Vec::new(),
            skipped_count: 0,
            kept_count: 0,
        });
    }

    let strategy = cfg.key_strategy;
    let mut kept_urls: Vec<String> = Vec::with_capacity(tracks.len());
    let mut activity_lines: Vec<String> = Vec::new();
    let mut skipped_count: usize = 0;

    // Intra-playlist dedup (a song listed twice in the same playlist
    // shouldn't be downloaded twice either). Track the keys we've already
    // kept so we can flag second occurrences.
    let mut seen_in_playlist: HashSet<String> = HashSet::new();

    for t in &tracks {
        let Some(key) = build_track_key_from_parts(
            Some(&t.song_id),
            t.isrc.as_deref(),
            strategy,
        ) else {
            // Track lacks the fields required by the configured strategy
            // — can't dedupe it. Keep it so we don't silently drop tracks.
            if let Some(url) = build_track_url(&parsed.storefront, &t.album_id, &t.song_id) {
                kept_urls.push(url);
            }
            continue;
        };

        if queued_keys.contains(&key) {
            skipped_count += 1;
            activity_lines.push(format!(
                "Duplicate skipped from playlist: {} — already in download queue",
                t.name
            ));
            continue;
        }
        if history_keys.contains(&key) {
            skipped_count += 1;
            activity_lines.push(format!(
                "Duplicate skipped from playlist: {} — already in download history",
                t.name
            ));
            continue;
        }
        if !seen_in_playlist.insert(key) {
            skipped_count += 1;
            activity_lines.push(format!(
                "Duplicate skipped from playlist: {} — same track listed twice in playlist",
                t.name
            ));
            continue;
        }

        if let Some(url) = build_track_url(&parsed.storefront, &t.album_id, &t.song_id) {
            kept_urls.push(url);
        }
    }

    let kept_count = kept_urls.len();
    let plan = if kept_urls.is_empty() {
        PlaylistPlan::SkipEntirely
    } else {
        PlaylistPlan::TrackUrls(kept_urls)
    };

    log::info!(
        "Playlist dedup: {playlist_id} — {kept_count} kept, {skipped_count} skipped"
    );

    Some(PlaylistDedupPlan {
        plan,
        activity_lines,
        skipped_count,
        kept_count,
    })
}

// ============================================================
// Internal helpers
// ============================================================

/// A track enumerated under a specific mode.
#[derive(Debug, Clone)]
struct TrackInMode {
    /// Dedup key (song_id or ISRC, per strategy).
    key: String,
    /// Numeric Apple Music song ID (used when building the track URL).
    song_id: String,
    /// Album it belongs to.
    album_id: String,
    /// Album title (for diagnostics only).
    #[allow(dead_code)]
    album_name: Option<String>,
    /// Track title, used for activity-log messages.
    title: String,
}

/// Build the dedup key for a track based on the configured strategy.
/// Returns `None` when the track lacks whatever the strategy requires
/// (e.g. `IsrcOnly` but the API didn't return an ISRC).
fn build_track_key(track: &TrackMetadata, strategy: DedupKeyStrategy) -> Option<String> {
    match strategy {
        DedupKeyStrategy::SongIdOnly => {
            if track.song_id.is_empty() {
                None
            } else {
                Some(format!("song:{}", track.song_id))
            }
        }
        DedupKeyStrategy::IsrcOnly => track
            .isrc
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("isrc:{}", s.to_uppercase())),
        DedupKeyStrategy::SongIdIsrcFallback => {
            if !track.song_id.is_empty() {
                Some(format!("song:{}", track.song_id))
            } else {
                track
                    .isrc
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("isrc:{}", s.to_uppercase()))
            }
        }
    }
}

/// Build the same dedup key from a (song_id, isrc) pair. Used when
/// deriving keys from queue / history sources where we don't have a
/// full `TrackMetadata`.
#[must_use]
pub fn build_track_key_from_parts(
    song_id: Option<&str>,
    isrc: Option<&str>,
    strategy: DedupKeyStrategy,
) -> Option<String> {
    let song_id = song_id.filter(|s| !s.is_empty());
    let isrc = isrc.filter(|s| !s.is_empty());
    match strategy {
        DedupKeyStrategy::SongIdOnly => song_id.map(|s| format!("song:{s}")),
        DedupKeyStrategy::IsrcOnly => isrc.map(|s| format!("isrc:{}", s.to_uppercase())),
        DedupKeyStrategy::SongIdIsrcFallback => song_id
            .map(|s| format!("song:{s}"))
            .or_else(|| isrc.map(|s| format!("isrc:{}", s.to_uppercase()))),
    }
}

/// Produce a priority-ordered mode list for dedup iteration.
///
/// Starts with the user's preference order (`preference_cfg`) filtered
/// down to modes actually in `modes`, then appends any user-selected
/// mode that wasn't in the preference list in its natural position.
fn preference_order(
    preference_cfg: &[ArtistAutoSelect],
    modes: &[ArtistAutoSelect],
) -> Vec<ArtistAutoSelect> {
    let selected: HashSet<&ArtistAutoSelect> = modes.iter().collect();
    let mut out: Vec<ArtistAutoSelect> = Vec::with_capacity(modes.len());
    let mut placed: HashSet<ArtistAutoSelect> = HashSet::new();
    for m in preference_cfg {
        if selected.contains(m) && placed.insert(m.clone()) {
            out.push(m.clone());
        }
    }
    for m in modes {
        if placed.insert(m.clone()) {
            out.push(m.clone());
        }
    }
    out
}

/// Construct an Apple Music single-track URL in the form
/// `https://music.apple.com/{sf}/album/_/{album_id}?i={song_id}`.
/// Returns `None` when either component is empty.
fn build_track_url(storefront: &str, album_id: &str, song_id: &str) -> Option<String> {
    if storefront.is_empty() || album_id.is_empty() || song_id.is_empty() {
        return None;
    }
    Some(format!(
        "https://music.apple.com/{storefront}/album/_/{album_id}?i={song_id}"
    ))
}

/// Build a "no-op" dedup plan where every mode falls back to the
/// original artist URL. Used when the feature is enabled but cannot
/// operate for some reason (no credentials, API failure, etc.).
fn fallback_plan(modes: &[ArtistAutoSelect]) -> DedupPlan {
    DedupPlan {
        per_mode: modes
            .iter()
            .map(|m| (m.clone(), ModePlan::FallbackToArtistUrl))
            .collect(),
        activity_lines: Vec::new(),
        skipped_count: 0,
    }
}

/// Resolve a MusicKit developer token using the shared priority chain
/// already implemented in `apple_music_api`. Returns `None` if no token
/// source is configured.
fn resolve_jwt(settings: &AppSettings) -> Option<String> {
    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Dedup: keychain read for MusicKit private key failed: {e}");
            None
        }
    };
    match apple_music_api::resolve_musickit_developer_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(Some(jwt)) => Some(jwt),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Dedup: MusicKit JWT generation failed: {e}");
            None
        }
    }
}

// ============================================================
// Scope helpers (queue + history key collection)
// ============================================================

/// Collect song keys from the currently-active queue items (Queued,
/// Downloading, Processing). Returns an empty set when the scope
/// excludes the queue. The queue's `song_id` / ISRC source is the
/// per-item `status` captured at enqueue time when available; items
/// that pre-date #510 will not contribute keys (graceful degradation).
#[must_use]
pub fn collect_queued_keys(
    queue_snapshot: &[crate::models::download::QueueItemStatus],
    scope: DuplicateDetectionScope,
    strategy: DedupKeyStrategy,
) -> HashSet<String> {
    if matches!(
        scope,
        DuplicateDetectionScope::Off | DuplicateDetectionScope::IntraSession
    ) {
        return HashSet::new();
    }
    let mut keys = HashSet::new();
    for item in queue_snapshot {
        // Only consider items that are still going to run.
        if !matches!(
            item.state,
            crate::models::download::DownloadState::Queued
                | crate::models::download::DownloadState::Downloading
                | crate::models::download::DownloadState::Processing
        ) {
            continue;
        }
        // Parse URLs for a trailing `?i={song_id}` component — the URL
        // form we emit for dedup'd queue items. Older items without
        // that form contribute no keys, which is safe (worst case we
        // re-download an older queue entry's song, matching pre-#510
        // behaviour).
        for url in &item.urls {
            if let Some(song_id) = extract_song_id_from_url(url) {
                if let Some(key) = build_track_key_from_parts(
                    Some(&song_id),
                    None,
                    strategy,
                ) {
                    keys.insert(key);
                }
            }
        }
    }
    keys
}

/// Walk the configured output directory for `manifest.meedyadl` files
/// and collect every song key recorded across them. Used when the
/// scope is `IntraAndQueuedAndHistory`. Bounded by depth (10) and
/// file-count to avoid pathological scans on very large music
/// libraries.
#[must_use]
pub fn collect_history_keys(
    output_path: &str,
    scope: DuplicateDetectionScope,
    strategy: DedupKeyStrategy,
) -> HashSet<String> {
    if !matches!(scope, DuplicateDetectionScope::IntraAndQueuedAndHistory) {
        return HashSet::new();
    }
    let root = std::path::Path::new(output_path);
    if !root.is_dir() {
        return HashSet::new();
    }
    let mut keys = HashSet::new();
    walk_manifests(root, &mut keys, 0, 10, strategy);
    log::debug!(
        "Dedup: collected {} history key(s) from {}",
        keys.len(),
        output_path
    );
    keys
}

fn walk_manifests(
    dir: &std::path::Path,
    keys: &mut HashSet<String>,
    depth: u32,
    max_depth: u32,
    strategy: DedupKeyStrategy,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_manifests(&path, keys, depth + 1, max_depth, strategy);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name != "manifest.meedyadl" && name != ".meedyadl" {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<crate::models::manifest::ManifestFile>(&contents)
        else {
            continue;
        };
        for source in &manifest.sources {
            for track in &source.tracks {
                if let Some(key) = build_track_key_from_parts(
                    track.song_id.as_deref(),
                    track.isrc.as_deref(),
                    strategy,
                ) {
                    keys.insert(key);
                }
            }
        }
    }
}

/// Extract the `song_id` from a `?i={id}` query parameter, case-insensitively.
fn extract_song_id_from_url(url: &str) -> Option<String> {
    let idx = url.find("?i=").or_else(|| url.find("&i="))?;
    let rest = &url[idx + 3..];
    let end = rest.find('&').unwrap_or(rest.len());
    let candidate = &rest[..end];
    if candidate.chars().all(|c| c.is_ascii_digit()) && !candidate.is_empty() {
        Some(candidate.to_string())
    } else {
        None
    }
}

// ============================================================
// Predicate for when we should run dedup at all
// ============================================================

/// Convenience predicate: should we *attempt* dedup for this
/// artist-URL fan-out? Centralises the on/off logic so the caller in
/// `commands/gamdl.rs` stays tidy.
#[must_use]
pub fn dedup_enabled_for(cfg: &DuplicateDetectionSettings, modes: &[ArtistAutoSelect]) -> bool {
    !matches!(cfg.scope, DuplicateDetectionScope::Off) && !modes.is_empty()
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn track(song_id: &str, isrc: Option<&str>, name: &str) -> TrackMetadata {
        TrackMetadata {
            song_id: song_id.to_string(),
            isrc: isrc.map(str::to_string),
            content_rating: None,
            artist_id: None,
            artist_name: None,
            name: name.to_string(),
            track_number: 1,
            disc_number: 1,
            audio_traits: Vec::new(),
            is_apple_digital_master: None,
            release_date: None,
            composer_name: None,
            duration_in_millis: None,
            has_lyrics: None,
            play_params_id: None,
            url: None,
            preview_url: None,
            genre_names: Vec::new(),
            raw_json: serde_json::Value::Null,
        }
    }

    #[test]
    fn track_key_song_id_fallback() {
        let t = track("123", Some("USRC12345678"), "Song");
        assert_eq!(
            build_track_key(&t, DedupKeyStrategy::SongIdIsrcFallback),
            Some("song:123".into())
        );
        let t_no_id = track("", Some("USRC12345678"), "Song");
        assert_eq!(
            build_track_key(&t_no_id, DedupKeyStrategy::SongIdIsrcFallback),
            Some("isrc:USRC12345678".into())
        );
    }

    #[test]
    fn track_key_isrc_only_case_normalises() {
        let t = track("123", Some("usrc12345678"), "Song");
        assert_eq!(
            build_track_key(&t, DedupKeyStrategy::IsrcOnly),
            Some("isrc:USRC12345678".into())
        );
    }

    #[test]
    fn track_key_song_id_only_returns_none_when_missing() {
        let t = track("", Some("USRC12345678"), "Song");
        assert_eq!(build_track_key(&t, DedupKeyStrategy::SongIdOnly), None);
    }

    #[test]
    fn preference_order_puts_user_preference_first() {
        let cfg = vec![
            ArtistAutoSelect::MainAlbums,
            ArtistAutoSelect::SinglesEps,
            ArtistAutoSelect::CompilationAlbums,
        ];
        let modes = vec![
            ArtistAutoSelect::CompilationAlbums,
            ArtistAutoSelect::MainAlbums,
        ];
        let ordered = preference_order(&cfg, &modes);
        assert_eq!(
            ordered,
            vec![
                ArtistAutoSelect::MainAlbums,
                ArtistAutoSelect::CompilationAlbums,
            ]
        );
    }

    #[test]
    fn preference_order_appends_unlisted_modes() {
        let cfg = vec![ArtistAutoSelect::MainAlbums];
        let modes = vec![
            ArtistAutoSelect::MusicVideos,
            ArtistAutoSelect::MainAlbums,
        ];
        let ordered = preference_order(&cfg, &modes);
        assert_eq!(
            ordered,
            vec![
                ArtistAutoSelect::MainAlbums,
                ArtistAutoSelect::MusicVideos,
            ]
        );
    }

    #[test]
    fn track_url_builds_with_query_marker() {
        let u = build_track_url("gb", "1234", "9876").unwrap();
        assert_eq!(
            u,
            "https://music.apple.com/gb/album/_/1234?i=9876"
        );
    }

    #[test]
    fn song_id_extraction_handles_various_positions() {
        assert_eq!(
            extract_song_id_from_url(
                "https://music.apple.com/gb/album/_/1?i=42"
            ),
            Some("42".into())
        );
        assert_eq!(
            extract_song_id_from_url(
                "https://music.apple.com/gb/album/x/1?foo=bar&i=42"
            ),
            Some("42".into())
        );
        // No ?i=
        assert_eq!(
            extract_song_id_from_url("https://music.apple.com/gb/album/x/1"),
            None
        );
        // Non-numeric
        assert_eq!(
            extract_song_id_from_url("https://music.apple.com/gb/album/x/1?i=abc"),
            None
        );
    }

    // ----- Playlist dedup (#512) -----

    #[test]
    fn playlist_url_parsed_as_playlist_content_type() {
        use crate::services::apple_music_api::parse_apple_music_url;
        let parsed = parse_apple_music_url(
            "https://music.apple.com/gb/playlist/pink-floyd-essentials/pl.u-abc123",
        )
        .expect("playlist should parse");
        assert_eq!(parsed.content_type, "playlist");
        assert_eq!(parsed.playlist_id.as_deref(), Some("pl.u-abc123"));
        assert_eq!(parsed.storefront, "gb");
    }

    #[test]
    fn library_playlist_url_is_not_parsed_as_catalog_playlist() {
        // Library playlists (`/library/playlist/...`) must not match the
        // catalog playlist regex — they need Music-User-Token auth and a
        // different endpoint, so falling through to the generic path is
        // the correct behaviour.
        use crate::services::apple_music_api::parse_apple_music_url;
        assert!(parse_apple_music_url(
            "https://music.apple.com/gb/library/playlist/p.abc123"
        )
        .is_none());
    }
}
