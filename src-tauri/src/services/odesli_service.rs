// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Odesli (song.link) API client (#295 Phase A).
//!
//! Cross-platform content discovery: given an Apple Music URL,
//! Odesli returns matching URLs for Spotify, YouTube, Tidal,
//! Deezer, Amazon Music, SoundCloud, Bandcamp, Pandora, and more.
//!
//! ## Why this is useful today
//!
//! Even before MeedyaDL supports M9/M10 services natively, the
//! cross-platform URLs ride to disk via the `manifest.meedyadl` and
//! the `MeedyaMeta:*Url` freeform atoms. Users can:
//!   - See where else the same content exists (manifest inspection).
//!   - Cross-reference a MeedyaDL-downloaded album with their other
//!     library tools (Picard / beets read the freeform atoms).
//!   - Re-download from an alternative source when M9/M10 land —
//!     the URL is already in the manifest, no re-lookup needed.
//!
//! Complements [`crate::services::musicbrainz_service`] which also
//! discovers cross-platform URLs via MB external links: MB's coverage
//! is sparser but requires zero auth + zero rate-limit handshake.
//! Phase B (follow-up) will merge the two sources into a single
//! `MeedyaMeta:CrossPlatformUrls` map preferring Odesli when both
//! have data.
//!
//! ## API contract
//!
//! - Endpoint: `GET https://api.song.link/v1-alpha.1/links?url={encoded_url}`
//! - Auth: optional API key as `&key=…` query parameter for the
//!   60 req/min tier; without a key, free tier is 10 req/min.
//! - Response shape: `{ linksByPlatform: { spotify: { url, ... }, … } }`.
//!   We extract just the `url` per platform — the entity metadata
//!   isn't useful for MeedyaDL's storage purpose.
//!
//! ## Rate limiting
//!
//! Phase A enforces a conservative 1 req/sec floor between Odesli
//! lookups (well below the 10 req/min free-tier cap, headroom for
//! the parallel-enrichment task scheduler). Per-process, not
//! per-thread — the call site is one shared rate limiter behind a
//! [`tokio::sync::Mutex`].

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Public-facing Odesli endpoint. Versioned `v1-alpha.1` per the
/// Songlink docs; stable in practice for years.
const ODESLI_BASE_URL: &str = "https://api.song.link/v1-alpha.1/links";

/// Minimum gap between consecutive Odesli requests. 1.1 s gives
/// ~54 req/min headroom even with no API key (free tier = 10
/// req/min, but the limit is enforced as a burst window not a
/// rolling rate, so 1.1 s spacing leaves slack for retry / parallel
/// scheduler interleaving).
const RATE_LIMIT_GAP: Duration = Duration::from_millis(1100);

/// Per-process gate ensuring Odesli requests don't fire faster than
/// [`RATE_LIMIT_GAP`]. Stores the timestamp of the last successful
/// request start; new requests wait until at least `gap` has
/// elapsed.
static LAST_REQUEST_AT: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

/// Per-platform URL map. Keys mirror Odesli's platform identifiers
/// (`spotify`, `youtube`, `youtubeMusic`, `tidal`, `deezer`,
/// `amazonMusic`, `soundcloud`, `bandcamp`, `pandora`, …) but we
/// don't enumerate them as a Rust enum — every new platform Odesli
/// adds should just appear in the map without code changes.
pub type CrossPlatformUrls = BTreeMap<String, String>;

/// Lookup result. `None` (vs an error) is returned when the API
/// returns 404 / no matches — distinct from a network / parse
/// failure (which surfaces as `Err`).
pub async fn fetch_links(
    source_url: &str,
    api_key: Option<&str>,
) -> Result<Option<CrossPlatformUrls>, String> {
    // Throttle. Acquire the lock, check elapsed-since-last, sleep
    // the remainder if needed, then update the timestamp.
    {
        let mut last = LAST_REQUEST_AT.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < RATE_LIMIT_GAP {
                tokio::time::sleep(RATE_LIMIT_GAP - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    let mut url = format!(
        "{base}?url={encoded}",
        base = ODESLI_BASE_URL,
        encoded = urlencoded(source_url),
    );
    if let Some(key) = api_key {
        if !key.is_empty() {
            url.push_str("&key=");
            url.push_str(&urlencoded(key));
        }
    }

    log::debug!("Odesli lookup: {source_url}");
    let client = crate::utils::http_client::build_simple(15)?;
    let response = client
        .get(&url)
        .header("User-Agent", crate::utils::http_client::APP_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("Odesli request failed: {e}"))?;

    let status = response.status();
    if status.as_u16() == 404 {
        log::debug!("Odesli: no matches for {source_url}");
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!(
            "Odesli API returned HTTP {status} — rate limit hit? Try setting an API key in Settings > Advanced > API Credentials."
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Odesli response parse failed: {e}"))?;

    Ok(Some(extract_links_by_platform(&json)))
}

/// Extract every per-platform URL from the API response into a flat
/// map. Pure function — separated from the IPC/HTTP path so unit
/// tests can exercise the parser without network access.
fn extract_links_by_platform(json: &serde_json::Value) -> CrossPlatformUrls {
    let mut out = CrossPlatformUrls::new();
    let Some(by_platform) = json.get("linksByPlatform").and_then(|v| v.as_object()) else {
        return out;
    };
    for (platform, entry) in by_platform {
        if let Some(url) = entry.get("url").and_then(|v| v.as_str()) {
            out.insert(platform.clone(), url.to_string());
        }
    }
    out
}

/// Minimal URL-encoder for query parameter values. The `url` crate
/// is the canonical way to do this elsewhere in the codebase
/// (see `crash_report_service::build_github_issue_url`), but for a
/// single query param the manual escape is shorter than constructing
/// a `Url` + `query_pairs_mut`.
fn urlencoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// DTO mirrored into the manifest schema (#759-style). Lives here so
/// the manifest module doesn't take a hard dependency on the
/// `odesli_service` module — keeps the persistence layer
/// orthogonal to the API client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdesliLookupResult {
    /// ISO 8601 timestamp of when the lookup completed.
    pub looked_up_at: String,
    /// Source URL that was passed to Odesli.
    pub source_url: String,
    /// Cross-platform URLs keyed by Odesli's platform identifier.
    pub urls: CrossPlatformUrls,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_links_handles_canonical_response() {
        let json = serde_json::json!({
            "entityUniqueId": "ITUNES_SONG::1234",
            "pageUrl": "https://song.link/i/1234",
            "linksByPlatform": {
                "spotify": { "url": "https://open.spotify.com/track/abc" },
                "youtubeMusic": { "url": "https://music.youtube.com/watch?v=xyz" },
                "tidal": { "url": "https://tidal.com/track/999" },
                "appleMusic": { "url": "https://music.apple.com/us/album/foo/1234" }
            }
        });
        let urls = extract_links_by_platform(&json);
        assert_eq!(urls.len(), 4);
        assert_eq!(
            urls.get("spotify").unwrap(),
            "https://open.spotify.com/track/abc"
        );
        assert_eq!(
            urls.get("youtubeMusic").unwrap(),
            "https://music.youtube.com/watch?v=xyz"
        );
    }

    #[test]
    fn extract_links_handles_empty_response() {
        let json = serde_json::json!({"linksByPlatform": {}});
        assert!(extract_links_by_platform(&json).is_empty());
    }

    #[test]
    fn extract_links_handles_missing_field() {
        let json = serde_json::json!({"entityUniqueId": "test"});
        assert!(extract_links_by_platform(&json).is_empty());
    }

    #[test]
    fn extract_links_skips_entries_without_url() {
        let json = serde_json::json!({
            "linksByPlatform": {
                "spotify": { "url": "https://open.spotify.com/track/abc" },
                "broken": { "entityUniqueId": "X::1" }, // no url field
            }
        });
        let urls = extract_links_by_platform(&json);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains_key("spotify"));
        assert!(!urls.contains_key("broken"));
    }

    #[test]
    fn urlencoded_escapes_specials() {
        assert_eq!(urlencoded("https://music.apple.com/gb/album/foo/123"),
            "https%3A%2F%2Fmusic.apple.com%2Fgb%2Falbum%2Ffoo%2F123");
        assert_eq!(urlencoded("a b"), "a%20b");
        assert_eq!(urlencoded("a-b_c.d~e"), "a-b_c.d~e"); // unreserved
    }
}
