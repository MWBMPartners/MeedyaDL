// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Best-cover-art service (M9-3).
//! ==============================
//!
//! Selects the highest-resolution static cover art available across every
//! supported music platform — *regardless* of which engine actually
//! downloaded the audio — and exposes the winner to the enrichment
//! pipeline for embedding.
//!
//! ## Why cross-platform
//!
//! In practice Apple Music's artwork CDN serves cover images up to
//! 3000 × 3000 for modern releases, while Spotify's oEmbed surface
//! caps at 640 × 640. So today the resolution race is almost always
//! won by Apple Music. **But that's an Apple Music vs. Spotify
//! property, not an architectural one.** When MeedyaDL adds Tidal
//! (typically 1280 × 1280) or Bandcamp (often 1500 × 1500), and as
//! upstream catalogues evolve, the right answer is "ask everyone,
//! pick the biggest." Hard-coding "Apple Music always wins" would
//! quietly regress every future platform.
//!
//! ## Tie-break
//!
//! When two candidates report the same `width × height`, Apple Music
//! wins. Rationale: Apple's source-of-truth artwork tends to be
//! mastered from a higher-fidelity original than aggregator
//! re-uploads at matching dimensions, and the user feedback that
//! prompted this design said *"if quality… are equal use Apple
//! Music"* verbatim.
//!
//! ## Anti-ban posture
//!
//! Best-cover-art does **not** issue downloads through any service's
//! download engine — only public metadata / oEmbed endpoints. So the
//! Spotify account-ban surface (M9-4's anti-ban stack) is unaffected
//! whether this feature is on or off.
//!
//! ## Privacy
//!
//! When the user disables `best_cover_art_enabled`, no cross-platform
//! fetch fires at all. The originating engine's cover art is used
//! exactly as-is. There is no implicit Spotify hit on Apple Music
//! downloads (or vice versa) without the toggle being on.
//!
//! ## Integration point
//!
//! This module is pure infrastructure — it returns a `Best` value
//! describing the winning candidate. Plumbing the winner into the
//! post-download embed pass lives with the queue refactor in M9-4.
//! Keeping the integration out of M9-3 means this PR is self-contained
//! and ships zero user-visible behaviour change until M9-4 turns it
//! on.

use serde::{Deserialize, Serialize};

use crate::services::apple_music_api::AlbumMetadata;
use crate::utils::http_client;

// ============================================================
// Candidate types
// ============================================================

/// Which catalogue / API a cover-art candidate came from.
///
/// The variant set grows as new music services land; the tie-break
/// rule in [`pick_best`] folds new variants in via [`Self::tiebreak_priority`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverArtSource {
    /// Apple Music catalog (`amp-api.music.apple.com`).
    AppleMusic,
    /// Spotify public oEmbed endpoint (`open.spotify.com/oembed`).
    Spotify,
}

impl CoverArtSource {
    /// Lower priority value = preferred on equal-pixel ties.
    ///
    /// Apple Music wins all ties (priority 0). Spotify and future
    /// platforms slot in behind. Keep this stable across releases —
    /// changing the priority shifts which file ends up embedded for
    /// historical re-downloads.
    fn tiebreak_priority(self) -> u8 {
        match self {
            Self::AppleMusic => 0,
            Self::Spotify => 1,
        }
    }

    /// Stable kebab-case identifier for logging / activity events.
    #[must_use]
    pub fn kebab_id(self) -> &'static str {
        match self {
            Self::AppleMusic => "apple-music",
            Self::Spotify => "spotify",
        }
    }
}

/// A single platform's candidate for "best cover art for this album."
///
/// `width` × `height` is in pixels at the **maximum natural resolution**
/// the platform advertises for this asset — we don't request the URL
/// to confirm. Lying about dimensions just produces a wrong-but-
/// non-crashing winner pick; the field is documented as best-effort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverArtCandidate {
    /// Which platform returned this candidate.
    pub source: CoverArtSource,
    /// Direct URL the consumer should `GET` to fetch the image bytes.
    /// For platforms with templated URLs (Apple Music's `{w}x{h}`),
    /// this is the already-rendered concrete URL.
    pub url: String,
    /// Width in pixels at the URL above.
    pub width: u32,
    /// Height in pixels at the URL above.
    pub height: u32,
}

impl CoverArtCandidate {
    /// Pixel area = `width × height`, in `u64` to avoid overflow at
    /// 3000² and beyond. Used as the primary sort key by [`pick_best`].
    #[must_use]
    pub fn pixel_area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// Request payload describing the album whose cover art we want to
/// compare across platforms.
///
/// Every per-platform identifier is `Option` so the caller can supply
/// only what they have. The Apple Music branch needs an
/// [`AlbumMetadata`] (the existing enrichment pipeline already fetches
/// it). The Spotify branch needs the `open.spotify.com/album/...` URL.
/// At least one of the two must yield a candidate, or `pick_best`
/// returns `None`.
#[derive(Debug, Clone, Default)]
pub struct BestCoverArtRequest<'a> {
    /// Apple Music metadata for the album, if available.
    pub apple_metadata: Option<&'a AlbumMetadata>,
    /// Spotify album URL (e.g. `https://open.spotify.com/album/ABC123`),
    /// if available.
    pub spotify_url: Option<&'a str>,
}

// ============================================================
// Comparator
// ============================================================

/// Pick the highest-resolution candidate from `candidates`, with
/// Apple Music as the equal-pixel tie-breaker.
///
/// Returns `None` if `candidates` is empty.
///
/// Sorting is stable — when two equal-area, equal-source candidates
/// appear (shouldn't happen in practice, since each source contributes
/// at most one entry), insertion order is preserved.
#[must_use]
pub fn pick_best(mut candidates: Vec<CoverArtCandidate>) -> Option<CoverArtCandidate> {
    candidates.sort_by(|a, b| {
        // Primary key: pixel area, descending (larger wins).
        b.pixel_area()
            .cmp(&a.pixel_area())
            // Secondary key: source priority, ascending (lower → better).
            // Apple Music's priority is 0 so it wins ties.
            .then_with(|| a.source.tiebreak_priority().cmp(&b.source.tiebreak_priority()))
    });
    candidates.into_iter().next()
}

// ============================================================
// Apple Music adapter
// ============================================================

/// Build the Apple Music candidate from an already-fetched
/// [`AlbumMetadata`].
///
/// Apple's `artwork.url` is a template containing `{w}`, `{h}`, `{f}`
/// placeholders — we render it with the maximum native dimensions the
/// API reported. Returns `None` when the metadata lacks either a
/// template or both dimensions (defensive — happens on very old
/// catalogue entries).
#[must_use]
pub fn apple_music_candidate(metadata: &AlbumMetadata) -> Option<CoverArtCandidate> {
    let template = metadata.artwork_url_template.as_ref()?;
    let width = metadata.artwork_width?;
    let height = metadata.artwork_height?;
    let url = template
        .replace("{w}", &width.to_string())
        .replace("{h}", &height.to_string())
        // Format placeholder: prefer `jpg`. JPEG is universally
        // tag-embeddable and is what GAMDL writes today; staying on
        // it keeps the embed flow unchanged.
        .replace("{f}", "jpg")
        // Crop / scaling placeholder. `bb` = "best fit, black bars"
        // — matches Apple's default behaviour for square album art
        // and avoids cropping artwork that isn't perfectly square.
        // Some catalogue entries don't have this placeholder; that's
        // a no-op replace.
        .replace("{c}", "bb");
    Some(CoverArtCandidate {
        source: CoverArtSource::AppleMusic,
        url,
        width,
        height,
    })
}

// ============================================================
// Spotify adapter (public oEmbed)
// ============================================================

/// Fetch the Spotify candidate via the public oEmbed endpoint.
///
/// `https://open.spotify.com/oembed?url=<album_url>` returns JSON of
/// shape `{ "thumbnail_url": "...", "thumbnail_width": N,
/// "thumbnail_height": N }`. The endpoint is public — no auth, no
/// Spotify Web API client credentials needed — and rate-limited
/// generously enough for one call per album download.
///
/// Returns `Ok(None)` on:
/// * empty / non-`open.spotify.com` URL
/// * 4xx response (typically a removed album)
/// * missing `thumbnail_url` / `thumbnail_*` fields
///
/// Returns `Err` on network failure or unparseable response — but
/// callers should treat that as "no Spotify candidate" rather than a
/// hard error, since the user can still proceed with whatever Apple
/// Music returned.
pub async fn fetch_spotify_candidate(
    spotify_album_url: &str,
) -> Result<Option<CoverArtCandidate>, String> {
    if spotify_album_url.is_empty() {
        return Ok(None);
    }
    if !spotify_album_url.contains("open.spotify.com") {
        return Ok(None);
    }

    // Use the shared HTTP-client builder so timeouts and TLS settings
    // stay in lockstep with the rest of the codebase. 10-second
    // timeout is generous for a 500-byte JSON response.
    let client = http_client::build_simple(10)?;

    let oembed_url = format!(
        "https://open.spotify.com/oembed?url={}",
        urlencode(spotify_album_url)
    );

    let resp = client
        .get(&oembed_url)
        .send()
        .await
        .map_err(|e| format!("Spotify oEmbed request failed: {e}"))?;

    if !resp.status().is_success() {
        // 4xx / 5xx → no candidate, but not a hard error for the
        // caller — they can still proceed with Apple Music's pick.
        return Ok(None);
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Spotify oEmbed body wasn't JSON: {e}"))?;

    parse_spotify_oembed_json(&json)
}

/// Pure parser separated from the HTTP fetch so it's unit-testable
/// without network access.
fn parse_spotify_oembed_json(
    json: &serde_json::Value,
) -> Result<Option<CoverArtCandidate>, String> {
    let url = json.get("thumbnail_url").and_then(|v| v.as_str());
    let width = json.get("thumbnail_width").and_then(serde_json::Value::as_u64);
    let height = json
        .get("thumbnail_height")
        .and_then(serde_json::Value::as_u64);

    match (url, width, height) {
        (Some(u), Some(w), Some(h)) => Ok(Some(CoverArtCandidate {
            source: CoverArtSource::Spotify,
            url: u.to_string(),
            width: u32::try_from(w).unwrap_or(u32::MAX),
            height: u32::try_from(h).unwrap_or(u32::MAX),
        })),
        _ => Ok(None),
    }
}

/// Minimal URL-component encoder — just enough to safely embed a
/// Spotify album URL into the `?url=` query parameter. Avoids pulling
/// in an extra crate for one call site.
fn urlencode(s: &str) -> String {
    // Encode the same set `URLEncoder.encode` would: anything that's
    // not unreserved per RFC 3986 §2.3.
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
// Orchestrator
// ============================================================

/// Fan out to every platform in `req`, collect candidates in parallel,
/// and return the winner per [`pick_best`].
///
/// Failure modes are folded silently into "no candidate from this
/// platform" — see the per-adapter doc — so the orchestrator only
/// returns `Err` on programmer errors (e.g. malformed request that
/// doesn't even allow a fetch). Today there are no such error paths,
/// hence the consistently-`Ok` signature.
///
/// This function is the integration point the queue layer calls in
/// M9-4. Today nothing in MeedyaDL calls it yet — that's deliberate
/// (see module docs on the integration-point boundary).
pub async fn find_best_cover_art(
    req: &BestCoverArtRequest<'_>,
) -> Result<Option<CoverArtCandidate>, String> {
    // Apple Music branch — synchronous because metadata is already
    // in hand. Spotify branch is the async network hit.
    let apple = req.apple_metadata.and_then(apple_music_candidate);

    let spotify = match req.spotify_url {
        Some(url) => fetch_spotify_candidate(url).await.unwrap_or(None),
        None => None,
    };

    let mut candidates = Vec::new();
    if let Some(c) = apple {
        candidates.push(c);
    }
    if let Some(c) = spotify {
        candidates.push(c);
    }

    Ok(pick_best(candidates))
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(source: CoverArtSource, w: u32, h: u32) -> CoverArtCandidate {
        CoverArtCandidate {
            source,
            url: format!("https://example.com/{}-{w}x{h}.jpg", source.kebab_id()),
            width: w,
            height: h,
        }
    }

    #[test]
    fn pick_best_returns_none_on_empty_input() {
        assert!(pick_best(Vec::new()).is_none());
    }

    #[test]
    fn pick_best_returns_only_candidate() {
        let c = candidate(CoverArtSource::Spotify, 640, 640);
        let picked = pick_best(vec![c.clone()]).unwrap();
        assert_eq!(picked, c);
    }

    #[test]
    fn pick_best_prefers_larger_pixel_area() {
        let apple = candidate(CoverArtSource::AppleMusic, 1000, 1000);
        let spotify = candidate(CoverArtSource::Spotify, 3000, 3000);
        let picked = pick_best(vec![apple, spotify.clone()]).unwrap();
        // Larger image wins regardless of source identity.
        assert_eq!(picked, spotify);
    }

    #[test]
    fn pick_best_tiebreaks_to_apple_music_on_equal_area() {
        let apple = candidate(CoverArtSource::AppleMusic, 640, 640);
        let spotify = candidate(CoverArtSource::Spotify, 640, 640);
        let picked = pick_best(vec![spotify, apple.clone()]).unwrap();
        assert_eq!(
            picked, apple,
            "Apple Music must win equal-pixel tie-breaks (per #911 EPIC design)"
        );
    }

    #[test]
    fn pick_best_handles_non_square_areas() {
        // 1500×1000 = 1.5M pixels; 1200×1200 = 1.44M pixels. Larger area wins.
        let landscape = candidate(CoverArtSource::Spotify, 1500, 1000);
        let square = candidate(CoverArtSource::AppleMusic, 1200, 1200);
        let picked = pick_best(vec![square, landscape.clone()]).unwrap();
        assert_eq!(picked, landscape);
    }

    fn metadata_with_artwork(template: Option<&str>, w: Option<u32>, h: Option<u32>) -> AlbumMetadata {
        // Centralised fixture builder so adding a new AlbumMetadata
        // field doesn't require editing every test in this module.
        AlbumMetadata {
            album_id: "1".to_string(),
            album_name: Some("Test Album".to_string()),
            upc: None,
            content_rating: None,
            genre_names: vec![],
            artist_id: None,
            artist_name: None,
            record_label: None,
            copyright: None,
            release_date: None,
            is_compilation: None,
            is_single: None,
            is_complete: None,
            is_mastered_for_itunes: None,
            track_count: None,
            editorial_notes: None,
            last_modified_date: None,
            tracks: vec![],
            artwork_square_url: None,
            artwork_tall_url: None,
            album_spotlight_url: None,
            artwork_url_template: template.map(String::from),
            artwork_width: w,
            artwork_height: h,
            raw_json: serde_json::Value::Null,
        }
    }

    #[test]
    fn apple_music_candidate_renders_template_with_max_dimensions() {
        let metadata = metadata_with_artwork(
            Some("https://is1-ssl.mzstatic.com/abc/{w}x{h}{c}.{f}"),
            Some(3000),
            Some(3000),
        );
        let candidate = apple_music_candidate(&metadata).expect("candidate produced");
        assert_eq!(candidate.source, CoverArtSource::AppleMusic);
        assert_eq!(candidate.width, 3000);
        assert_eq!(candidate.height, 3000);
        assert!(
            candidate.url.contains("3000x3000bb.jpg"),
            "rendered URL '{}' should substitute max dimensions",
            candidate.url
        );
    }

    #[test]
    fn apple_music_candidate_skips_when_template_missing() {
        let mut metadata = metadata_with_artwork(None, Some(3000), Some(3000));
        assert!(
            apple_music_candidate(&metadata).is_none(),
            "missing template should return None"
        );
        // Also: dimensions missing.
        metadata.artwork_url_template = Some("https://x/{w}x{h}.{f}".to_string());
        metadata.artwork_width = None;
        assert!(apple_music_candidate(&metadata).is_none());
    }

    #[test]
    fn parse_spotify_oembed_picks_thumbnail_fields() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "thumbnail_url": "https://i.scdn.co/image/abc",
                "thumbnail_width": 640,
                "thumbnail_height": 640,
                "title": "Some Album"
            }"#,
        )
        .unwrap();
        let candidate = parse_spotify_oembed_json(&json).unwrap().unwrap();
        assert_eq!(candidate.source, CoverArtSource::Spotify);
        assert_eq!(candidate.width, 640);
        assert_eq!(candidate.height, 640);
        assert_eq!(candidate.url, "https://i.scdn.co/image/abc");
    }

    #[test]
    fn parse_spotify_oembed_returns_none_on_missing_fields() {
        let json: serde_json::Value = serde_json::from_str(r#"{"title": "x"}"#).unwrap();
        assert!(parse_spotify_oembed_json(&json).unwrap().is_none());
    }

    #[test]
    fn urlencode_preserves_unreserved() {
        assert_eq!(urlencode("AbZ-09._~"), "AbZ-09._~");
    }

    #[test]
    fn urlencode_percent_encodes_reserved() {
        assert_eq!(
            urlencode("https://open.spotify.com/album/1?x=1"),
            "https%3A%2F%2Fopen.spotify.com%2Falbum%2F1%3Fx%3D1"
        );
    }

    #[test]
    fn cover_art_source_tiebreak_priority_apple_lowest() {
        assert!(
            CoverArtSource::AppleMusic.tiebreak_priority()
                < CoverArtSource::Spotify.tiebreak_priority(),
            "Apple Music must hold the lowest tiebreak priority — see #911"
        );
    }

    #[test]
    fn pixel_area_no_overflow_at_3000_squared() {
        let c = CoverArtCandidate {
            source: CoverArtSource::AppleMusic,
            url: String::new(),
            width: 3000,
            height: 3000,
        };
        assert_eq!(c.pixel_area(), 9_000_000);
    }

    #[tokio::test]
    async fn find_best_cover_art_returns_none_when_no_inputs() {
        let req = BestCoverArtRequest::default();
        let result = find_best_cover_art(&req).await.unwrap();
        assert!(result.is_none());
    }
}
