// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MusicBrainz recording lookup service.
// ======================================
//
// Queries the MusicBrainz database to discover recording metadata,
// relationships, and cross-platform URLs using ISRC codes. This serves
// two purposes:
//
// 1. **Music video companion fallback** — find music videos via
//    MusicBrainz when MusicKit credentials aren't configured.
//    No Apple Developer account needed.
//
// 2. **Cross-platform discovery groundwork** — MusicBrainz links
//    recordings to Apple Music, YouTube, Spotify, Deezer, Tidal etc.
//    This enables future "if unavailable on one platform, try another"
//    functionality when additional service engines are added.
//
// ## API Details
//
// - Endpoint: `https://musicbrainz.org/ws/2/`
// - Rate limit: 1 request/second (enforced via sleep between requests)
// - Authentication: None needed for read-only lookups
// - User-Agent: Required by MusicBrainz ToS (identifies the application)
// - Format: JSON (`fmt=json`)
//
// ## Endpoint policy (post 2026-11-30 search upgrade)
//
// MusicBrainz's 2026-11-30 search-service upgrade (Solr 9 → 10) changes
// the behaviour of `query=`-style SEARCH requests. This service therefore
// uses only non-search LOOKUP/BROWSE endpoints:
//
// - ISRC:      `GET /ws/2/isrc/{isrc}?inc=…`      (the live production path)
// - Recording: `GET /ws/2/recording/{mbid}?inc=…` (MBID lookup)
// - URL:       `GET /ws/2/url?resource={url}&inc=…` (browse by exact resource;
//              Tier 1 groundwork — not yet wired by any production caller)
//
// ## Lookup Chain
//
// ISRC code (from Apple Music API metadata) → MusicBrainz recording(s) →
// relationships → video recording → external URLs (Apple Music, YouTube)
//
// ## Integration
//
// Called from the enrichment pipeline (Step 6b) as a fallback when the
// MusicKit-based music video lookup (Step 6) finds no videos. Discovered
// cross-platform URLs are currently logged for diagnostics; persisting
// them as file metadata is planned follow-up work.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

// ============================================================
// Constants
// ============================================================

/// Base URL for the MusicBrainz web service API (v2).
const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";

/// Rate limit delay between MusicBrainz API requests.
/// MusicBrainz requires max 1 request/second for unauthenticated clients.
const RATE_LIMIT_DELAY: Duration = Duration::from_millis(1100);

/// HTTP request timeout for MusicBrainz API calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// User-Agent header required by MusicBrainz Terms of Service.
/// Must identify the application and provide a contact URL.
///
/// Aliased to the shared `APP_USER_AGENT` constant (was previously a
/// hardcoded `"MeedyaDL/0.6"` literal that silently drifted out of sync
/// with the app's actual version as it moved through the 1.x line —
/// exactly the stale-version ToS defect this alias fixes).
const USER_AGENT: &str = crate::utils::http_client::APP_USER_AGENT;

// ============================================================
// Internal: Endpoint-Anomaly Classification + URL Encoding
// ============================================================

/// Builds the error string for an "endpoint answered, but wrongly"
/// condition — a non-2xx status (other than a legitimate 404
/// not-found) or an HTTP 200 whose body lacks the expected top-level
/// shape. These are the two signatures of a server-side API change
/// (e.g. the 2026-11-30 search-service upgrade) as opposed to an
/// ordinary transport failure, so `lookup_videos_for_tracks_enhanced`
/// surfaces the FIRST one per album as a non-verbose warning instead
/// of letting a regression masquerade as "no music videos found".
///
/// Deliberately contains NO query string, NO ISRC, and NO looked-up
/// URL — endpoint kind + status (+ a static shape note) only, per the
/// credential-redaction norms for non-verbose log lines.
fn endpoint_anomaly_error(endpoint_kind: &str, status: u16, detail: Option<&str>) -> String {
    match detail {
        Some(d) => format!("MusicBrainz {endpoint_kind} endpoint anomaly (HTTP {status}): {d}"),
        None => format!("MusicBrainz {endpoint_kind} endpoint anomaly (HTTP {status})"),
    }
}

/// True when an error string produced inside this module represents an
/// endpoint anomaly (see [`endpoint_anomaly_error`]). Substring-based
/// classifier in the house idiom of `process::is_io_error` — transport
/// failures ("MusicBrainz API request failed: …") and JSON-parse
/// failures deliberately do NOT match: they are ordinary network
/// conditions, not API-change signals.
fn is_endpoint_anomaly(error: &str) -> bool {
    error.contains("endpoint anomaly (HTTP ")
}

/// Once-per-invocation guard for the anomaly warning. Pure so the
/// at-most-once behaviour is unit-testable without an AppHandle.
/// Returns `true` exactly when a warning should be emitted now, and
/// flips the flag.
fn should_emit_endpoint_warning(error: &str, already_emitted: &mut bool) -> bool {
    if is_endpoint_anomaly(error) && !*already_emitted {
        *already_emitted = true;
        true
    } else {
        false
    }
}

/// Minimal RFC 3986 §2.3 component encoder — everything that is not
/// unreserved (`A–Z a–z 0–9 - _ . ~`) is percent-encoded. Used to embed
/// a full URL into the `?resource=` query parameter of the MB URL
/// browse endpoint. Mirrors `best_cover_art_service::urlencode` (kept
/// private there); avoids pulling in an extra crate for one call site.
/// Closes the old hand-rolled-encoder gap where only `:` and `/` were
/// encoded and a `?i=…`/`&`/`#` in an Apple Music URL corrupted the
/// request.
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

// ============================================================
// Public Types
// ============================================================

/// A MusicBrainz recording with its metadata and discovered URLs.
///
/// Represents a single track/song as identified by MusicBrainz,
/// including all external platform URLs found in its relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicBrainzRecording {
    /// MusicBrainz recording UUID (e.g., "5aa053a9-5b84-418f-bb3c-d61df67b3880")
    pub recording_id: String,
    /// Track title from MusicBrainz
    pub title: String,
    /// Artist credit string (may differ from Apple Music's artist name)
    pub artist: Option<String>,
    /// External platform URLs found in recording relationships.
    /// Key = platform identifier, value = URL.
    /// Common keys: "apple_music", "youtube", "spotify", "deezer", "tidal"
    pub external_urls: HashMap<String, String>,
    /// Music video URLs discovered for this recording.
    pub video_urls: Vec<MusicVideoUrl>,
}

/// A discovered music video URL from MusicBrainz relationships.
///
/// Represents a video linked to a recording via MusicBrainz's
/// recording-recording "performance" relationships or URL relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicVideoUrl {
    /// Platform identifier: "apple_music", "youtube", etc.
    pub platform: String,
    /// Full URL to the music video on the platform
    pub url: String,
    /// Title of the video (from MusicBrainz, may differ from audio track)
    pub title: Option<String>,
}

// ============================================================
// Public API
// ============================================================

/// Look up a MusicBrainz recording by ISRC code.
///
/// Uses the dedicated non-search ISRC lookup endpoint
/// (`GET /ws/2/isrc/{isrc}`) — NOT the `query=isrc:` recording search,
/// which is affected by MusicBrainz's 2026-11-30 search-service
/// upgrade. The `inc=` parameters inline URL/recording relationships
/// and artist credits into each returned recording, so one request
/// yields the fully-populated result the old search + detail-lookup
/// pair needed two requests for.
///
/// # Arguments
/// * `isrc` - International Standard Recording Code (e.g., "USUG12345678")
///
/// # Returns
/// The first recording bearing the ISRC, with its relationships, or
/// `None` if MusicBrainz has no recording for it (HTTP 404, or an
/// empty `recordings` array).
pub async fn lookup_recording_by_isrc(isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
    if isrc.is_empty() {
        return Ok(None);
    }

    // Non-search ISRC lookup. All recordings in the response bear this
    // exact ISRC, so "first element" ≈ "any correct match" — the same
    // first-element behaviour the old search path had (scores were
    // never inspected; the lookup endpoint has none to inspect).
    let url = format!(
        "{MB_API_BASE}/isrc/{isrc}?inc=url-rels+recording-rels+artist-credits&fmt=json"
    );

    log::debug!("MusicBrainz: looking up ISRC {isrc}");

    // Create HTTP client with timeout and required User-Agent header
    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 is the endpoint's legitimate "this ISRC is not in the
    // database" answer (lookup semantics — unlike the old search,
    // which answered 200 + empty array). Quiet no-match, NOT an
    // anomaly.
    if status == 404 {
        log::debug!("MusicBrainz: no recording found for ISRC {isrc} (404)");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("isrc", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_recording_from_isrc_response(&json)
}

/// Pure extraction half of [`lookup_recording_by_isrc`] — parses the
/// `/ws/2/isrc/{isrc}` response body. Split out so the parse logic is
/// unit-testable from inline JSON fixtures without a live client
/// (house pattern; cf. `extract_syllable_ttml_from_response`).
///
/// Expected shape (HTTP 200):
/// `{"isrc": "…", "recordings": [ { "id", "title", "artist-credit",
/// "relations", … } ]}` — relations and artist-credit inlined per
/// recording because of the `inc=` parameters.
///
/// - `recordings` present but empty → `Ok(None)` (legitimate no-match).
/// - `recordings` missing or not an array → endpoint-anomaly `Err`
///   (a 200 that doesn't look like the documented response is the
///   signature of a server-side API change, and must be
///   distinguishable from a legitimate empty result).
fn extract_recording_from_isrc_response(
    json: &serde_json::Value,
) -> Result<Option<MusicBrainzRecording>, String> {
    let Some(recordings) = json.get("recordings").and_then(|r| r.as_array()) else {
        return Err(endpoint_anomaly_error(
            "isrc",
            200,
            Some("response body is missing the expected 'recordings' array"),
        ));
    };

    let Some(rec) = recordings.first() else {
        log::debug!("MusicBrainz: ISRC known but has no recordings attached");
        return Ok(None);
    };

    let recording_id = rec
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if recording_id.is_empty() {
        // A recording object with no usable MBID — treat as no-match
        // (mirrors the old extract_first_recording_from_search guard).
        return Ok(None);
    }

    let title = rec
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = rec
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Relations are inlined on the recording object itself (that is
    // what the inc= parameters buy us) — parse them with the shared
    // helper, exactly as the old detail-lookup response was parsed.
    let (external_urls, video_urls) = parse_recording_relations(rec);

    log::debug!(
        "MusicBrainz: found recording {} — \"{}\" by {} ({} external URL(s), {} video URL(s))",
        recording_id,
        title,
        artist.as_deref().unwrap_or("unknown"),
        external_urls.len(),
        video_urls.len()
    );

    Ok(Some(MusicBrainzRecording {
        recording_id,
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Information for looking up a track on MusicBrainz.
///
/// Carries all available identifiers for a track, used by the 3-tier
/// discovery priority chain: URL → ISRC → AcoustID recording ID.
#[derive(Debug, Clone)]
pub struct TrackLookupInfo {
    /// Internal song/download ID (for logging)
    pub song_id: String,
    /// Apple Music song URL (primary discovery path — search MB external links)
    pub apple_music_url: Option<String>,
    /// ISRC code from Apple Music API metadata (secondary path)
    pub isrc: Option<String>,
    /// MusicBrainz recording ID from AcoustID fingerprinting (tertiary path)
    pub musicbrainz_recording_id: Option<String>,
}

/// Look up music video URLs for a batch of tracks.
///
/// Uses a 3-tier discovery priority for each track:
/// 1. **Apple Music URL** — search MusicBrainz for recordings linked to
///    this URL via external links (most direct match)
/// 2. **ISRC code** — search by International Standard Recording Code
///    (reliable, standard identifier)
/// 3. **MusicBrainz recording ID** — direct lookup by ID if available
///    from AcoustID fingerprinting (skips search entirely)
///
/// Rate-limited to 1 request/second per MusicBrainz ToS.
///
/// # Returns
/// All discovered music video URLs, deduplicated by URL.
pub async fn lookup_videos_for_tracks(
    app: &tauri::AppHandle,
    download_id: &str,
    tracks: &[(String, Option<String>)],
) -> Result<Vec<MusicVideoUrl>, String> {
    // Convert legacy (song_id, isrc) pairs to TrackLookupInfo
    let infos: Vec<TrackLookupInfo> = tracks
        .iter()
        .map(|(song_id, isrc)| TrackLookupInfo {
            song_id: song_id.clone(),
            apple_music_url: None,
            isrc: isrc.clone(),
            musicbrainz_recording_id: None,
        })
        .collect();

    lookup_videos_for_tracks_enhanced(app, download_id, &infos).await
}

/// Enhanced track video lookup with 3-tier discovery priority.
///
/// Priority chain per track:
/// 1. Apple Music URL → MusicBrainz external link search
/// 2. ISRC → MusicBrainz recording search
/// 3. MusicBrainz recording ID → direct lookup (from AcoustID)
pub async fn lookup_videos_for_tracks_enhanced(
    app: &tauri::AppHandle,
    download_id: &str,
    tracks: &[TrackLookupInfo],
) -> Result<Vec<MusicVideoUrl>, String> {
    use crate::utils::activity_log::{emit_download_log, emit_download_warn, emit_verbose_download_log};

    let mut all_videos = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();
    let mut request_count = 0;
    // At most ONE non-verbose endpoint-anomaly warning per invocation
    // (== per album — this fn is called once per album by the Step 6b
    // lookup task). Everything else stays verbose-only.
    let mut endpoint_warning_emitted = false;

    for track in tracks {
        let mut found = false;

        // Tier 1: Try Apple Music URL lookup on MusicBrainz
        // (search for recordings that have this URL as an external link)
        if let Some(ref am_url) = track.apple_music_url {
            if request_count > 0 {
                tokio::time::sleep(RATE_LIMIT_DELAY).await;
            }
            request_count += 1;

            emit_verbose_download_log(
                app,
                download_id,
                &format!(
                    "MusicBrainz: Tier 1 — looking up song {} via Apple Music URL",
                    track.song_id
                ),
            );

            // Search MusicBrainz for recordings linked to this Apple Music URL
            match lookup_recording_by_url(am_url).await {
                Ok(Some(recording)) => {
                    for video in &recording.video_urls {
                        if seen_urls.insert(video.url.clone()) {
                            all_videos.push(video.clone());
                        }
                    }
                    found = true;
                    emit_download_log(
                        app,
                        download_id,
                        &format!(
                            "MusicBrainz: matched song {} via Apple Music URL (Tier 1)",
                            track.song_id
                        ),
                    );
                }
                Ok(None) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: Tier 1 — no match for URL {am_url}"),
                    );
                }
                Err(e) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: Tier 1 — URL lookup failed: {e}"),
                    );
                    if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                        emit_download_warn(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: unexpected API response — {e}. \
                                 Music-video discovery may be incomplete for this album; \
                                 the download itself is not affected."
                            ),
                        );
                    }
                }
            }
        }

        // Tier 2: Try ISRC lookup (if Tier 1 didn't find anything)
        if !found {
            if let Some(ref isrc) = track.isrc {
                if request_count > 0 {
                    tokio::time::sleep(RATE_LIMIT_DELAY).await;
                }
                request_count += 1;

                emit_verbose_download_log(
                    app,
                    download_id,
                    &format!(
                        "MusicBrainz: Tier 2 — looking up song {} via ISRC {isrc}",
                        track.song_id
                    ),
                );

                match lookup_recording_by_isrc(isrc).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        found = true;
                        emit_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: matched song {} via ISRC {isrc} (Tier 2)",
                                track.song_id
                            ),
                        );
                    }
                    Ok(None) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 2 — no match for ISRC {isrc}"),
                        );
                    }
                    Err(e) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 2 — ISRC lookup failed: {e}"),
                        );
                        if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                            emit_download_warn(
                                app,
                                download_id,
                                &format!(
                                    "MusicBrainz: unexpected API response — {e}. \
                                     Music-video discovery may be incomplete for this album; \
                                     the download itself is not affected."
                                ),
                            );
                        }
                    }
                }
            }
        }

        // Tier 3: Try direct MusicBrainz recording ID lookup (from AcoustID)
        if !found {
            if let Some(ref mb_id) = track.musicbrainz_recording_id {
                if request_count > 0 {
                    tokio::time::sleep(RATE_LIMIT_DELAY).await;
                }
                request_count += 1;

                emit_verbose_download_log(
                    app,
                    download_id,
                    &format!(
                        "MusicBrainz: Tier 3 — looking up song {} via recording ID {mb_id}",
                        track.song_id
                    ),
                );

                match lookup_recording_by_id(mb_id).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        emit_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: matched song {} via recording ID {mb_id} (Tier 3)",
                                track.song_id
                            ),
                        );
                    }
                    Ok(None) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 3 — no match for recording ID {mb_id}"),
                        );
                    }
                    Err(e) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 3 — ID lookup failed: {e}"),
                        );
                        if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                            emit_download_warn(
                                app,
                                download_id,
                                &format!(
                                    "MusicBrainz: unexpected API response — {e}. \
                                     Music-video discovery may be incomplete for this album; \
                                     the download itself is not affected."
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(all_videos)
}

/// Look up a MusicBrainz recording via the URL entity that carries the
/// given resource.
///
/// Uses the non-search URL browse endpoint
/// (`GET /ws/2/url?resource={url}&inc=recording-rels`) — NOT the old
/// `query=url:` recording search, which (a) is affected by the
/// 2026-11-30 search-service upgrade, and (b) could never have worked
/// well for recordings anyway: recording–URL relationships were not
/// indexed by the search server at all before that upgrade
/// (SEARCH-452). The browse endpoint reads the database directly, so
/// it sees every recording–URL relationship today.
///
/// #807 storefront-independence: the browse endpoint is EXACT-match on
/// the stored resource — the old leading-wildcard Lucene glob has no
/// browse equivalent. As a bounded substitute, when the exact user URL
/// misses we retry once with the storefront-less canonical form
/// (`https://music.apple.com/{type}/{id}`). Storefronted permutations
/// other than the user's own are not probed; if Tier 1 is ever wired
/// for real, extending the candidate list (e.g. `/us/…`) is the place
/// to do it.
///
/// Currently LATENT: no production caller populates
/// `TrackLookupInfo.apple_music_url` — this is groundwork hardening.
pub async fn lookup_recording_by_url(
    external_url: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if external_url.is_empty() {
        return Ok(None);
    }

    // Candidate resources, tried in order: the user's exact URL, then
    // the storefront-and-slug-less canonical form (skipped when it
    // would repeat the exact URL).
    let mut candidates: Vec<String> = vec![external_url.to_string()];
    if let Some(canonical) = super::apple_music_api::canonicalise_apple_music_url(external_url) {
        // canonicalise returns scheme-less "music.apple.com/{type}/{id}";
        // MB stores full URLs, so re-add the scheme.
        let canonical_resource = format!("https://{canonical}");
        if canonical_resource != external_url {
            log::debug!(
                "MusicBrainz: will fall back to canonical resource form if the exact URL misses (#807)"
            );
            candidates.push(canonical_resource);
        }
    }

    let mut recording_id: Option<String> = None;
    for (i, resource) in candidates.iter().enumerate() {
        if i > 0 {
            // Rate limit between consecutive browse attempts.
            tokio::time::sleep(RATE_LIMIT_DELAY).await;
        }
        if let Some(mbid) = browse_recording_id_by_resource(resource).await? {
            recording_id = Some(mbid);
            break;
        }
    }

    let Some(mbid) = recording_id else {
        return Ok(None);
    };

    // Rate limit before the follow-up recording lookup.
    tokio::time::sleep(RATE_LIMIT_DELAY).await;
    lookup_recording_by_id(&mbid).await
}

/// Browse the MB `url` entity for an exact resource and return the
/// MBID of the first related recording, if any.
///
/// Endpoint: `GET /ws/2/url?resource={percent-encoded url}&inc=recording-rels&fmt=json`.
/// The resource value is fully percent-encoded (RFC 3986 component
/// rules) — the old hand-rolled encoder only covered `:` and `/`,
/// so a `?i=…` query, `&`, `=` or `#` in an Apple Music URL corrupted
/// the request.
async fn browse_recording_id_by_resource(resource: &str) -> Result<Option<String>, String> {
    let url = format!(
        "{MB_API_BASE}/url?resource={}&inc=recording-rels&fmt=json",
        percent_encode_component(resource)
    );

    log::debug!("MusicBrainz: browsing url entity by resource");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz URL browse request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 = "no url entity stored for this resource" — the endpoint's
    // legitimate miss answer. Quiet, not an anomaly.
    if status == 404 {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("url-browse", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_recording_id_from_url_browse(&json)
}

/// Pure extraction half of [`browse_recording_id_by_resource`] —
/// parses a `/ws/2/url?resource=…` response body and returns the MBID
/// of the first related recording.
///
/// Handles BOTH documented response shapes defensively:
/// - single-entity shape (one `resource` param): the url entity at the
///   root — `{"id": …, "resource": …, "relations": […]}`;
/// - browse-list shape: `{"urls": [ {…url entity…} ], "url-count": n}`
///   → the first entity is used.
///
/// Relation scanning prefers the canonical `target-type` marker and
/// the entity object named by it (`relation["recording"]["id"]`), and
/// merely tolerates the legacy scalar `target` (SEARCH-752) as an MBID
/// fallback.
///
/// - Recognised shape but no recording relation → `Ok(None)`.
/// - A 200 body with none of `relations` / `urls` / `id` at the root →
///   endpoint-anomaly `Err` (shape mismatch, distinct from a
///   legitimate empty result).
fn extract_recording_id_from_url_browse(
    json: &serde_json::Value,
) -> Result<Option<String>, String> {
    // Resolve which JSON object is the url entity.
    let entity: &serde_json::Value = if json.get("relations").is_some() || json.get("id").is_some()
    {
        // Single-entity shape.
        json
    } else if let Some(urls) = json.get("urls").and_then(|u| u.as_array()) {
        // Browse-list shape.
        match urls.first() {
            Some(first) => first,
            None => return Ok(None), // empty list = legitimate miss
        }
    } else {
        return Err(endpoint_anomaly_error(
            "url-browse",
            200,
            Some("response body has none of the expected 'relations', 'urls', or 'id' keys"),
        ));
    };

    let Some(relations) = entity.get("relations").and_then(|r| r.as_array()) else {
        // Entity found but no relations included/present — a miss,
        // not an anomaly.
        return Ok(None);
    };

    for rel in relations {
        let target_type = rel
            .get("target-type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Prefer the canonical shape: target-type names the entity
        // object to read. Tolerate a missing target-type by falling
        // back to the presence of the `recording` object itself.
        let is_recording_rel =
            target_type == "recording" || (target_type.is_empty() && rel.get("recording").is_some());
        if !is_recording_rel {
            continue;
        }

        if let Some(id) = rel
            .get("recording")
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_str())
        {
            if !id.is_empty() {
                return Ok(Some(id.to_string()));
            }
        }

        // Legacy tolerance (SEARCH-752): older output carried the
        // target entity's MBID in a scalar `target`. Only trust it
        // when it does NOT look like a URL (a url-rel's target is the
        // URL string itself).
        if let Some(t) = rel.get("target").and_then(|v| v.as_str()) {
            if !t.is_empty() && !t.starts_with("http") {
                return Ok(Some(t.to_string()));
            }
        }
    }

    Ok(None)
}

/// Get all discovered external URLs for a batch of tracks.
///
/// Similar to `lookup_videos_for_tracks` but returns all platform URLs
/// (not just video URLs). Useful for cross-platform discovery.
///
/// # Returns
/// A map of song_id → HashMap<platform, url> for all tracks that had
/// MusicBrainz matches.
pub async fn lookup_external_urls_for_tracks(
    tracks: &[(String, Option<String>)],
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let mut results = HashMap::new();
    let mut attempted = false;

    for (song_id, isrc) in tracks {
        let Some(isrc) = isrc else {
            continue;
        };

        // Rate limit between requests — keyed on prior ATTEMPTS, not
        // prior HITS: consecutive misses must be paced too.
        if attempted {
            tokio::time::sleep(RATE_LIMIT_DELAY).await;
        }
        attempted = true;

        match lookup_recording_by_isrc(isrc).await {
            Ok(Some(recording)) if !recording.external_urls.is_empty() => {
                results.insert(song_id.clone(), recording.external_urls);
            }
            _ => {}
        }
    }

    Ok(results)
}

/// Look up a MusicBrainz recording directly by its recording ID.
///
/// Used when the recording ID is already known (e.g., from AcoustID
/// fingerprint results). Skips the ISRC search step entirely.
///
/// # Arguments
/// * `recording_id` - MusicBrainz recording UUID
///
/// # Returns
/// Recording with relationships and external URLs.
pub async fn lookup_recording_by_id(
    recording_id: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if recording_id.is_empty() {
        return Ok(None);
    }

    log::debug!("MusicBrainz: looking up recording by ID {recording_id}");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    // Fetch recording with URL and recording relationships
    let url = format!(
        "{MB_API_BASE}/recording/{recording_id}?inc=url-rels+recording-rels+artist-credits&fmt=json"
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 = recording deleted or merged away — a legitimate miss for
    // an MBID that may have come from stale AcoustID data.
    if status == 404 {
        log::debug!("MusicBrainz: recording {recording_id} not found (404)");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("recording", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = json
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Parse relationships (same logic as in lookup_recording_by_isrc)
    let (external_urls, video_urls) = parse_recording_relations(&json);

    Ok(Some(MusicBrainzRecording {
        recording_id: recording_id.to_string(),
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Rewrite an Apple Music URL's storefront code.
///
/// If the URL contains a different storefront than the user's preferred
/// one, returns a new URL with the storefront replaced. Useful when
/// MusicBrainz returns an Apple Music URL for a different region.
///
/// # Examples
/// ```ignore
/// rewrite_apple_music_storefront(
///     "https://music.apple.com/de/album/550152190",
///     "gb"
/// ) // → "https://music.apple.com/gb/album/550152190"
/// ```
#[must_use]
pub fn rewrite_apple_music_storefront(url: &str, target_storefront: &str) -> String {
    // Apple Music URL pattern: https://music.apple.com/{storefront}/{type}/{name}/{id}
    // The storefront is a 2-letter country code after the domain.
    if let Some(domain_end) = url.find("music.apple.com/") {
        let after_domain = &url[domain_end + "music.apple.com/".len()..];
        // Check if the next segment is a 2-3 letter storefront code
        if let Some(slash_pos) = after_domain.find('/') {
            let current_sf = &after_domain[..slash_pos];
            // Only rewrite if it looks like a storefront (2-3 lowercase letters)
            if current_sf.len() >= 2
                && current_sf.len() <= 3
                && current_sf.chars().all(|c| c.is_ascii_lowercase())
                && current_sf != target_storefront
            {
                let prefix = &url[..domain_end + "music.apple.com/".len()];
                let rest = &after_domain[slash_pos..]; // includes the leading /
                return format!("{prefix}{target_storefront}{rest}");
            }
        }
    }
    // Return original if no rewrite needed or URL doesn't match pattern
    url.to_string()
}

// ============================================================
// Internal: Relationship Parsing
// ============================================================

/// Parse URL and recording relationships from a MusicBrainz recording JSON.
///
/// Extracts external platform URLs and music video URLs from the
/// `relations` array. Shared between ISRC lookup and direct ID lookup.
fn parse_recording_relations(
    json: &serde_json::Value,
) -> (HashMap<String, String>, Vec<MusicVideoUrl>) {
    let mut external_urls = HashMap::new();
    let mut video_urls = Vec::new();

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(relations) = json.get("relations").and_then(|r| r.as_array()) {
        for rel in relations {
            let rel_type = rel.get("type").and_then(|v| v.as_str()).unwrap_or("");

            let target_type = rel
                .get("target-type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // URL relationships — streaming/download links.
            // Prefer the canonical `target-type` marker; when absent
            // (pre-SEARCH-751/753 output lacked it on some entities)
            // fall back to the presence of the `url` entity object.
            let is_url_rel =
                target_type == "url" || (target_type.is_empty() && rel.get("url").is_some());
            if is_url_rel {
                let mut resource_url = rel
                    .get("url")
                    .and_then(|u| u.get("resource"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Legacy tolerance (SEARCH-752): older search output
                // carried the relation target in a scalar `target`.
                // For url-rels that scalar is the URL string itself —
                // only trust it when it actually looks like one.
                if resource_url.is_empty() {
                    if let Some(t) = rel.get("target").and_then(|v| v.as_str()) {
                        if t.starts_with("http://") || t.starts_with("https://") {
                            resource_url = t;
                        }
                    }
                }

                if let Some((platform, clean_url)) = classify_url(resource_url) {
                    external_urls.insert(platform.to_string(), clean_url.to_string());

                    if resource_url.contains("music-video") || resource_url.contains("/video/")
                    {
                        video_urls.push(MusicVideoUrl {
                            platform: platform.to_string(),
                            url: clean_url.to_string(),
                            title: Some(title.clone()),
                        });
                    }
                }
            }

            // Recording-recording relationships — linked performances.
            // Same target-type-preferred / entity-object-fallback rule.
            let is_recording_rel = target_type == "recording"
                || (target_type.is_empty() && rel.get("recording").is_some());
            if is_recording_rel && rel_type == "performance" {
                if let Some(target_rec) = rel.get("recording") {
                    let video_title = target_rec
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let _video_id = target_rec.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    if let Some(ref vt) = video_title {
                        log::debug!("MusicBrainz: found linked video recording — {vt}");
                    }
                }
            }
        }
    }

    (external_urls, video_urls)
}

// ============================================================
// Internal: URL Classification
// ============================================================

/// Classify a URL by its platform based on the domain.
///
/// Returns `Some((platform_id, url))` for recognized platforms,
/// or `None` for unrecognized URLs.
fn classify_url(url: &str) -> Option<(&str, &str)> {
    if url.contains("music.apple.com") || url.contains("itunes.apple.com") {
        Some(("apple_music", url))
    } else if url.contains("youtube.com") || url.contains("youtu.be") {
        Some(("youtube", url))
    } else if url.contains("spotify.com") || url.contains("open.spotify.com") {
        Some(("spotify", url))
    } else if url.contains("deezer.com") {
        Some(("deezer", url))
    } else if url.contains("tidal.com") {
        Some(("tidal", url))
    } else if url.contains("soundcloud.com") {
        Some(("soundcloud", url))
    } else if url.contains("bandcamp.com") {
        Some(("bandcamp", url))
    } else {
        None
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // URL classification
    // ----------------------------------------------------------

    #[test]
    fn classify_apple_music_url() {
        let result = classify_url("https://music.apple.com/gb/music-video/291812351");
        assert_eq!(
            result,
            Some((
                "apple_music",
                "https://music.apple.com/gb/music-video/291812351"
            ))
        );
    }

    #[test]
    fn classify_youtube_url() {
        let result = classify_url("https://www.youtube.com/watch?v=Eo-KmOd3i7s");
        assert_eq!(
            result,
            Some(("youtube", "https://www.youtube.com/watch?v=Eo-KmOd3i7s"))
        );
    }

    #[test]
    fn classify_youtube_short_url() {
        let result = classify_url("https://youtu.be/Eo-KmOd3i7s");
        assert_eq!(result, Some(("youtube", "https://youtu.be/Eo-KmOd3i7s")));
    }

    #[test]
    fn classify_spotify_url() {
        let result = classify_url("https://open.spotify.com/track/abc123");
        assert_eq!(
            result,
            Some(("spotify", "https://open.spotify.com/track/abc123"))
        );
    }

    #[test]
    fn classify_deezer_url() {
        let result = classify_url("https://www.deezer.com/track/12345");
        assert_eq!(
            result,
            Some(("deezer", "https://www.deezer.com/track/12345"))
        );
    }

    #[test]
    fn classify_tidal_url() {
        let result = classify_url("https://tidal.com/browse/track/12345");
        assert_eq!(
            result,
            Some(("tidal", "https://tidal.com/browse/track/12345"))
        );
    }

    #[test]
    fn classify_unknown_url_returns_none() {
        let result = classify_url("https://example.com/some/path");
        assert_eq!(result, None);
    }

    // ----------------------------------------------------------
    // MusicVideoUrl struct
    // ----------------------------------------------------------

    #[test]
    fn music_video_url_serializes() {
        let mv = MusicVideoUrl {
            platform: "apple_music".to_string(),
            url: "https://music.apple.com/gb/music-video/291812351".to_string(),
            title: Some("Test Video".to_string()),
        };
        let json = serde_json::to_string(&mv).unwrap();
        assert!(json.contains("apple_music"));
        assert!(json.contains("291812351"));
    }

    // ----------------------------------------------------------
    // MusicBrainzRecording struct
    // ----------------------------------------------------------

    #[test]
    fn recording_with_empty_urls() {
        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: Some("Test Artist".to_string()),
            external_urls: HashMap::new(),
            video_urls: Vec::new(),
        };
        assert!(rec.external_urls.is_empty());
        assert!(rec.video_urls.is_empty());
    }

    #[test]
    fn recording_with_multiple_platforms() {
        let mut urls = HashMap::new();
        urls.insert(
            "apple_music".to_string(),
            "https://music.apple.com/...".to_string(),
        );
        urls.insert("youtube".to_string(), "https://youtube.com/...".to_string());
        urls.insert(
            "spotify".to_string(),
            "https://open.spotify.com/...".to_string(),
        );

        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: None,
            external_urls: urls,
            video_urls: Vec::new(),
        };
        assert_eq!(rec.external_urls.len(), 3);
        assert!(rec.external_urls.contains_key("apple_music"));
        assert!(rec.external_urls.contains_key("youtube"));
        assert!(rec.external_urls.contains_key("spotify"));
    }

    // ----------------------------------------------------------
    // Storefront rewriting
    // ----------------------------------------------------------

    #[test]
    fn rewrite_storefront_de_to_gb() {
        let result =
            rewrite_apple_music_storefront("https://music.apple.com/de/album/test/550152190", "gb");
        assert_eq!(result, "https://music.apple.com/gb/album/test/550152190");
    }

    #[test]
    fn rewrite_storefront_us_to_gb() {
        let result = rewrite_apple_music_storefront(
            "https://music.apple.com/us/music-video/test/291812351",
            "gb",
        );
        assert_eq!(
            result,
            "https://music.apple.com/gb/music-video/test/291812351"
        );
    }

    #[test]
    fn rewrite_storefront_same_no_change() {
        let url = "https://music.apple.com/gb/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_non_apple_url_unchanged() {
        let url = "https://youtube.com/watch?v=abc";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_no_storefront_segment() {
        // URL without storefront (geo-non-specific)
        let url = "https://music.apple.com/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        // "album" is not a 2-3 char lowercase code, so no rewrite
        assert_eq!(result, url);
    }

    // ----------------------------------------------------------
    // Relationship parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_relations_with_url_relationships() {
        let json = serde_json::json!({
            "title": "Test Song",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/album/test/12345"
                    }
                },
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://www.youtube.com/watch?v=abc123"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains_key("apple_music"));
        assert!(urls.contains_key("youtube"));
        assert!(videos.is_empty()); // No music-video URLs in this test
    }

    #[test]
    fn parse_relations_with_video_url() {
        let json = serde_json::json!({
            "title": "Test Video",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/music-video/test/12345"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, "apple_music");
    }

    #[test]
    fn parse_relations_empty() {
        let json = serde_json::json!({
            "title": "No Relations"
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert!(urls.is_empty());
        assert!(videos.is_empty());
    }

    // ----------------------------------------------------------
    // ISRC endpoint (Decision A) — extract_recording_from_isrc_response
    // ----------------------------------------------------------

    fn isrc_fixture() -> serde_json::Value {
        serde_json::json!({
            "isrc": "USUG12345678",
            "recordings": [
                {
                    "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880",
                    "title": "Test Song",
                    "artist-credit": [ { "name": "Test Artist" } ],
                    "relations": [
                        {
                            "type": "streaming",
                            "target-type": "url",
                            "url": { "resource": "https://music.apple.com/gb/music-video/test/291812351" }
                        },
                        {
                            "type": "streaming",
                            "target-type": "url",
                            "url": { "resource": "https://open.spotify.com/track/abc123" }
                        }
                    ]
                },
                {
                    "id": "ffffffff-0000-0000-0000-000000000000",
                    "title": "Second Recording Same ISRC",
                    "artist-credit": [ { "name": "Other Artist" } ],
                    "relations": []
                }
            ]
        })
    }

    #[test]
    fn isrc_endpoint_response_extracts_recording_and_urls() {
        let result = extract_recording_from_isrc_response(&isrc_fixture());
        let rec = result.unwrap().unwrap();
        assert_eq!(rec.recording_id, "5aa053a9-5b84-418f-bb3c-d61df67b3880");
        assert_eq!(rec.title, "Test Song");
        assert_eq!(rec.artist.as_deref(), Some("Test Artist"));
        assert_eq!(rec.external_urls.len(), 2);
        assert!(rec.external_urls.contains_key("apple_music"));
        assert!(rec.external_urls.contains_key("spotify"));
        assert_eq!(rec.video_urls.len(), 1);
        assert_eq!(rec.video_urls[0].platform, "apple_music");
        assert_eq!(rec.video_urls[0].title.as_deref(), Some("Test Song"));
    }

    #[test]
    fn isrc_endpoint_takes_first_recording() {
        let result = extract_recording_from_isrc_response(&isrc_fixture());
        let rec = result.unwrap().unwrap();
        assert_ne!(rec.recording_id, "ffffffff-0000-0000-0000-000000000000");
        assert_eq!(rec.recording_id, "5aa053a9-5b84-418f-bb3c-d61df67b3880");
    }

    #[test]
    fn isrc_endpoint_empty_recordings_returns_none_quietly() {
        let json = serde_json::json!({"isrc": "GBAAA0000001", "recordings": []});
        let result = extract_recording_from_isrc_response(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn isrc_endpoint_recording_without_id_returns_none() {
        let json = serde_json::json!({
            "isrc": "GBAAA0000001",
            "recordings": [ { "title": "No Id Here" } ]
        });
        let result = extract_recording_from_isrc_response(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn isrc_endpoint_missing_recordings_key_is_anomaly() {
        let json = serde_json::json!({"error": "something else entirely"});
        let result = extract_recording_from_isrc_response(&json);
        let e = result.unwrap_err();
        assert!(is_endpoint_anomaly(&e));
        assert!(e.contains("isrc"));
        assert!(e.contains("HTTP 200"));
        assert!(e.contains("recordings"));
    }

    // ----------------------------------------------------------
    // parse_recording_relations hardening (SEARCH-752/751/753)
    // ----------------------------------------------------------

    #[test]
    fn parse_relations_missing_target_type_falls_back_to_entity_object() {
        let json = serde_json::json!({
            "title": "Shape Shift",
            "relations": [
                { "type": "streaming", "url": { "resource": "https://music.apple.com/gb/music-video/x/999" } }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains_key("apple_music"));
        assert_eq!(videos.len(), 1);
    }

    #[test]
    fn parse_relations_tolerates_legacy_target_scalar() {
        let json = serde_json::json!({
            "title": "Legacy Shape",
            "relations": [
                { "type": "streaming", "target-type": "url",
                  "target": "https://music.apple.com/gb/music-video/y/123" },
                { "type": "streaming", "target-type": "url",
                  "target": "0f0f0f0f-1111-2222-3333-444444444444" }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
    }

    #[test]
    fn parse_relations_prefers_entity_object_over_legacy_target() {
        let json = serde_json::json!({
            "title": "Prefer Entity",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": { "resource": "https://tidal.com/browse/track/1" },
                    "target": "https://open.spotify.com/track/zzz"
                }
            ]
        });

        let (urls, _videos) = parse_recording_relations(&json);
        assert!(urls.contains_key("tidal"));
        assert!(!urls.contains_key("spotify"));
    }

    #[test]
    fn parse_relations_video_classification_unchanged() {
        let json = serde_json::json!({
            "title": "Multi",
            "relations": [
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://music.apple.com/gb/music-video/test/1" } },
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://www.youtube.com/video/abc" } },
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://music.apple.com/gb/album/test/2" } }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        // The two apple_music URLs collide on the same map key (HashMap
        // insert overwrites), so only 2 distinct platform keys survive.
        assert_eq!(urls.len(), 2);
        assert!(urls.contains_key("apple_music"));
        assert!(urls.contains_key("youtube"));
        assert_eq!(videos.len(), 2);
    }

    // ----------------------------------------------------------
    // URL browse endpoint (Decision B) — extract_recording_id_from_url_browse
    // ----------------------------------------------------------

    #[test]
    fn url_browse_single_entity_shape_yields_recording_id() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "resource": "https://music.apple.com/us/album/1729264859",
            "relations": [
                { "type": "free streaming", "target-type": "recording",
                  "recording": { "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880", "title": "Test Song" } }
            ]
        });

        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );
    }

    #[test]
    fn url_browse_list_shape_yields_recording_id() {
        let entity = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "resource": "https://music.apple.com/us/album/1729264859",
            "relations": [
                { "type": "free streaming", "target-type": "recording",
                  "recording": { "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880", "title": "Test Song" } }
            ]
        });
        let json = serde_json::json!({
            "url-count": 1,
            "url-offset": 0,
            "urls": [ entity ]
        });

        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );
    }

    #[test]
    fn url_browse_empty_list_returns_none() {
        let json = serde_json::json!({"url-count": 0, "url-offset": 0, "urls": []});
        let result = extract_recording_id_from_url_browse(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn url_browse_entity_without_recording_rel_returns_none() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "member of band", "target-type": "artist",
                  "artist": { "id": "some-artist-id", "name": "Someone" } }
            ]
        });
        let result = extract_recording_id_from_url_browse(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn url_browse_legacy_target_mbid_tolerated() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "x", "target-type": "recording",
                  "target": "5aa053a9-5b84-418f-bb3c-d61df67b3880" }
            ]
        });
        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );

        // A url-shaped target must NOT be tolerated as an MBID.
        let json_url_target = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "x", "target-type": "recording",
                  "target": "https://music.apple.com/us/album/1" }
            ]
        });
        let result2 = extract_recording_id_from_url_browse(&json_url_target);
        assert!(matches!(result2, Ok(None)));
    }

    #[test]
    fn url_browse_unrecognised_shape_is_anomaly() {
        let json = serde_json::json!({"whatever": true});
        let result = extract_recording_id_from_url_browse(&json);
        let e = result.unwrap_err();
        assert!(is_endpoint_anomaly(&e));
        assert!(e.contains("url-browse"));
    }

    // ----------------------------------------------------------
    // percent_encode_component
    // ----------------------------------------------------------

    #[test]
    fn percent_encode_component_encodes_query_reserved() {
        assert_eq!(
            percent_encode_component("https://music.apple.com/gb/album/x/123?i=456&l=en#frag"),
            "https%3A%2F%2Fmusic.apple.com%2Fgb%2Falbum%2Fx%2F123%3Fi%3D456%26l%3Den%23frag"
        );
    }

    #[test]
    fn percent_encode_component_preserves_unreserved() {
        assert_eq!(percent_encode_component("AbZ-09._~"), "AbZ-09._~");
    }

    // ----------------------------------------------------------
    // Endpoint-anomaly classification
    // ----------------------------------------------------------

    #[test]
    fn endpoint_anomaly_error_formats_and_classifies() {
        let e = endpoint_anomaly_error("isrc", 503, None);
        assert_eq!(e, "MusicBrainz isrc endpoint anomaly (HTTP 503)");
        assert!(is_endpoint_anomaly(&e));
        let e2 = endpoint_anomaly_error("url-browse", 200, Some("shape note"));
        assert_eq!(e2, "MusicBrainz url-browse endpoint anomaly (HTTP 200): shape note");
        assert!(is_endpoint_anomaly(&e2));
    }

    #[test]
    fn transport_and_parse_errors_are_not_anomalies() {
        assert!(!is_endpoint_anomaly(
            "MusicBrainz API request failed: connection refused"
        ));
        assert!(!is_endpoint_anomaly(
            "Failed to parse MusicBrainz response: EOF"
        ));
        assert!(!is_endpoint_anomaly(
            "MusicBrainz URL browse request failed: timeout"
        ));
    }

    #[test]
    fn endpoint_warning_emitted_at_most_once() {
        let mut flag = false;
        let anomaly = endpoint_anomaly_error("isrc", 500, None);
        assert!(should_emit_endpoint_warning(&anomaly, &mut flag)); // first anomaly → emit
        assert!(!should_emit_endpoint_warning(&anomaly, &mut flag)); // second anomaly → suppressed

        let mut flag2 = false;
        assert!(!should_emit_endpoint_warning(
            "MusicBrainz API request failed: x",
            &mut flag2
        ));
        assert!(!flag2); // transport error neither emits nor consumes the budget
    }
}
