// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MusicBrainz guarded search fallback tier (S1 recording search, S2
// once-per-album URL search) + the advisory search-era probe. Part of
// the search-upgrade readiness work tracked in #1120.
//
// Adopts SEARCH-452 (recording-URL relationships become searchable after
// the 2026-11-30 Solr 9 -> 10 upgrade) without violating the governing
// contract in `mod.rs`'s module doc-comment: search responses are
// consumed ONLY as identifier discovery (recording MBIDs, url-entity
// resource strings) — every relation this tier eventually acts on is
// re-fetched through the existing lookup/browse pipeline in `mod.rs`,
// parsed via `relations::collect_relations`. Recording search output
// carries no relations at all (probe B5); url-entity search hit
// relations are never read for payload here, only for shape-sanity
// test fixtures (`extract_resources_from_url_search` reads `resource`
// only).
//
// The era probe (`/ws/2/genre/?query=…`, 501 pre-upgrade -> 200
// post-upgrade per SEARCH-681) is advisory-only: it decides whether the
// once-per-album S2 request is worth sending, never which parser runs.
// SEARCH-764's staged SolrCloud rollout means consecutive responses can
// come from differently-versioned nodes, so a cached era can never be a
// parsing branch — see the "DO NOT DO" list in the readiness plan.
//
// BLOCKER B1: S2's proven query shape (`build_url_search_query`) uses a
// trailing wildcard that can substring-match an album ID longer than
// the one being searched for. `validate_s2_resource` is the mandatory
// guard between a search hit and the existing browse/lookup pipeline —
// without it, a wrong-album music video could reach a real GAMDL
// download. See its own doc-comment and the
// `validate_s2_resource_rejects_longer_id_substring_match` test below.

use super::*;

/// Per-album cap on S1 attempts — a pathological no-ISRC 100-track
/// playlist must not become a 200-request MusicBrainz session. Applies
/// to SEARCH requests only (accepted or rejected candidates both count);
/// S2 is separately capped at once per album by construction (it runs
/// once, after the per-track loop, not inside it).
pub(super) const MAX_SEARCH_ATTEMPTS_PER_ALBUM: usize = 10;

/// Minimum Lucene relevance score (0-100) for accepting S1's top hit.
/// MusicBrainz's own search ranking is trusted below this only in
/// combination with the artist-credit match (`extract_mbid_from_recording_search`)
/// — score alone is not sufficient proof of identity.
pub(super) const MIN_SEARCH_SCORE: i64 = 90;

/// Non-2xx classifier for the S1/S2 search HTTP call sites only (#1120
/// F5 — conformance review, closes the tension against plan §0.1/§0.5's
/// "503 is a transport failure … never an anomaly"). The live probe
/// (`01-live-probe.md`) showed MusicBrainz throttling burstily and
/// returning 503 even at the full `RATE_LIMIT_DELAY` (1.1 s) spacing —
/// an ordinary rate-limit condition on these two NEW endpoints, not the
/// "server answered with the wrong shape" signature `endpoint_anomaly_error`
/// exists to flag. 503 and 429 are therefore reported as a plain
/// transport-style `Err` string that `is_endpoint_anomaly` does NOT
/// match, so a burst-throttle response can't consume the album's one
/// non-verbose anomaly warning; the tier still verbose-logs and falls
/// through exactly like a connection error, and is never retried.
/// Every other non-2xx status is still a genuine endpoint anomaly.
///
/// Deliberately scoped to `search_recording_mbid` / `search_url_resources`
/// alone — the legacy lookup/browse endpoints' existing 503-is-anomaly
/// behaviour (pinned by `endpoint_anomaly_error("isrc", 503)`) is an
/// established, separately pinned decision this work does not touch.
fn search_status_error(endpoint_kind: &str, status: u16) -> String {
    if status == 503 || status == 429 {
        format!("MusicBrainz {endpoint_kind} request throttled (HTTP {status})")
    } else {
        endpoint_anomaly_error(endpoint_kind, status, None)
    }
}

// ============================================================
// S1 — recording search (artist + title)
// ============================================================

/// S1: recording search by artist + title.
///
/// `GET /ws/2/recording/?query=<lucene>&limit=3&fmt=json` where
/// `<lucene>` = `artist:"{escaped}" AND recording:"{escaped}"` — the
/// exact clause shape live-validated 2026-09-01
/// (`{MB}/prod-B6-search-lucene.json`, `01-live-probe.md`'s B6 note).
/// `limit=3` is deliberately small: only the top hit is ever consulted
/// (`extract_mbid_from_recording_search`), so there is no use asking
/// for more.
///
/// Returns the top hit's MBID iff its score is >= [`MIN_SEARCH_SCORE`]
/// AND its artist-credit matches `artist` (M2b rules, see
/// `extract_mbid_from_recording_search`'s doc-comment). NEVER parses
/// relations out of the response — recording search output carries no
/// relations at all (probe B5); the MBID is handed to the caller, which
/// re-resolves the full recording (relations included) through the
/// existing, unaffected `lookup_recording_by_id` lookup endpoint.
pub(super) async fn search_recording_mbid(
    artist: &str,
    title: &str,
) -> Result<Option<String>, String> {
    let query = format!(
        "artist:\"{}\" AND recording:\"{}\"",
        escape_lucene(artist),
        escape_lucene(title)
    );
    let url = format!(
        "{MB_API_BASE}/recording/?query={}&limit=3&fmt=json",
        percent_encode_component(&query)
    );

    log::debug!("MusicBrainz: S1 — recording search");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    mb_rate_limit().await;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz recording search request failed: {e}"))?;

    let status = response.status().as_u16();

    if !response.status().is_success() {
        return Err(search_status_error("recording-search", status));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_mbid_from_recording_search(&json, artist)
}

/// Pure half of S1 — parses a `/ws/2/recording/?query=…` response and
/// decides whether the top hit is trustworthy enough to hand to the
/// existing MBID lookup.
///
/// - Missing `recordings` key on a 200 body → endpoint-anomaly `Err`
///   (endpoint kind `"recording-search"`) — the signature of a
///   server-side shape change, not a legitimate result.
/// - `recordings` present but empty → `Ok(None)`. This is search's
///   LEGITIMATE miss answer (200 + empty array), unlike lookup's 404 —
///   quiet, not an anomaly.
/// - Top hit's `score` below [`MIN_SEARCH_SCORE`], or its `id` field
///   absent/empty → `Ok(None)` (rejected candidate, zero follow-up
///   requests spent).
/// - Artist-credit match (M2b, §0.2): trim + `str::to_lowercase()` both
///   sides (no Unicode NFC/NFD normalisation — both Apple Music and
///   MusicBrainz emit NFC in practice, and a genuine mismatch is
///   verbose-logged at the call site rather than silently swallowed);
///   `expected_artist` is compared against EACH `artist-credit[].name`
///   AND against the joined credit string (every credit's `name` +
///   its `joinphrase`, concatenated in array order — MusicBrainz's own
///   convention for rendering a multi-artist credit, e.g. "Artist A
///   feat. Artist B"); accepted on any equality.
pub(super) fn extract_mbid_from_recording_search(
    json: &serde_json::Value,
    expected_artist: &str,
) -> Result<Option<String>, String> {
    let Some(recordings) = json.get("recordings").and_then(|r| r.as_array()) else {
        return Err(endpoint_anomaly_error(
            "recording-search",
            200,
            Some("response body is missing the expected 'recordings' array"),
        ));
    };

    let Some(top_hit) = recordings.first() else {
        // 200 + empty array is search's legitimate "no match" answer
        // (unlike lookup's 404) — a quiet miss, not an anomaly.
        return Ok(None);
    };

    let score = top_hit.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
    if score < MIN_SEARCH_SCORE {
        return Ok(None);
    }

    let id = top_hit.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if id.is_empty() {
        return Ok(None);
    }

    let artist_credit: &[serde_json::Value] = top_hit
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .map(std::vec::Vec::as_slice)
        .unwrap_or(&[]);

    if !artist_credit_matches(artist_credit, expected_artist) {
        return Ok(None);
    }

    Ok(Some(id.to_string()))
}

/// M2b artist-credit match, factored out of [`extract_mbid_from_recording_search`]
/// so the comparison rule has one place to read/test independent of the
/// score/id gating around it.
fn artist_credit_matches(artist_credit: &[serde_json::Value], expected_artist: &str) -> bool {
    let expected = expected_artist.trim().to_lowercase();
    if expected.is_empty() {
        return false;
    }

    let mut joined = String::new();
    for credit in artist_credit {
        let name = credit.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.trim().to_lowercase() == expected {
            return true;
        }
        joined.push_str(name);
        if let Some(joinphrase) = credit.get("joinphrase").and_then(|v| v.as_str()) {
            joined.push_str(joinphrase);
        }
    }

    joined.trim().to_lowercase() == expected
}

/// Escape every Lucene special byte in a user-derived string BEFORE it
/// is embedded in a query clause: `+ - & | ! ( ) { } [ ] ^ " ~ * ? : \ /`
/// and space. Lucene's `&&` / `||` boolean operators are two-character
/// tokens, but escaping every lone `&` and `|` byte (rather than trying
/// to detect the pair) is the safe implementation — an unescaped `&` or
/// `|` can still combine with an adjacent character from unrelated text
/// to form an unintended operator, so escaping unconditionally closes
/// that gap regardless of pairing (§0.2 nits).
///
/// Used for S1's caller-derived `artist:"…"` / `recording:"…"` clauses
/// (`search_recording_mbid`). S2's own fixed `music.apple.com` URL
/// prefix is deliberately NOT run through this function — see
/// [`build_url_search_query`]'s doc-comment for why.
pub(super) fn escape_lucene(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '+' | '-' | '&' | '|' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '"'
            | '~' | '*' | '?' | ':' | '\\' | '/' | ' ' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

// ============================================================
// S2 — once-per-album URL search (SEARCH-452 adoption)
// ============================================================

/// Build the S2 clause from the album's numeric ID. PROVEN SHAPE (live
/// probe M3-iv, 2026-09-01, count 2 — the `/us/` and `/gb/` storefront
/// variants of exactly this one album,
/// `{MB}/prod-M3iv-url-applemusic-id-anchored.json`):
///
/// ```text
/// url:https\://music.apple.com/*/album/*{album_id}*
/// ```
///
/// The fixed `https://music.apple.com` prefix is embedded as a literal,
/// hand-escaped string — deliberately NOT passed through
/// [`escape_lucene`] — because it is a compile-time-known constant, not
/// user-derived input, and `escape_lucene`'s general contract (needed
/// for S1's arbitrary artist/title text) also escapes `/`, which would
/// turn every `/` in this URL into a literal-match-only token and
/// silently narrow the query away from the exact shape validated
/// against production Solr: probes M3-ii/M3-iii proved an *unescaped*
/// `*` spans `/`, `?`, and `=` in this field — escaping the surrounding
/// slashes as well was never tested and isn't what this function
/// reproduces. Only the `:` after `https` is escaped, matching the
/// clause that returned exactly the known hit in probe M3-i.
///
/// `album_id` is digits-only (caller guarantees — resolved via
/// `apple_music_api::parse_apple_music_url` upstream in `mod.rs`) and
/// needs no escaping of its own. Wildcards are injected around it
/// directly (never through `escape_lucene`), so the `*` characters stay
/// real Lucene wildcards instead of being escaped into literal
/// asterisks.
///
/// The trailing `*` is required to catch `?i=` track variants of the
/// same album and any slug/query suffix on the resource — the
/// substring-overmatch risk that same wildcard creates (a longer album
/// ID that merely starts with these digits) is exactly what
/// [`validate_s2_resource`] exists to reject before any hit reaches the
/// browse/lookup pipeline (BLOCKER B1).
pub(super) fn build_url_search_query(album_id: &str) -> String {
    format!("url:https\\://music.apple.com/*/album/*{album_id}*")
}

/// S2: `GET /ws/2/url/?query=<clause>&limit=10&fmt=json`. Returns hit
/// `resource` strings ONLY — relations present on individual hits are
/// NEVER consumed here (the IDs-only search contract; see this file's
/// module doc-comment). Callers re-fetch relations through the existing
/// browse (`browse_recording_id_by_resource`) → lookup
/// (`lookup_recording_by_id`) pipeline, exactly as S1 does.
pub(super) async fn search_url_resources(album_id: &str) -> Result<Vec<String>, String> {
    let query = build_url_search_query(album_id);
    let url = format!(
        "{MB_API_BASE}/url/?query={}&limit=10&fmt=json",
        percent_encode_component(&query)
    );

    log::debug!("MusicBrainz: S2 — url entity search");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    mb_rate_limit().await;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz URL search request failed: {e}"))?;

    let status = response.status().as_u16();

    if !response.status().is_success() {
        return Err(search_status_error("url-search", status));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_resources_from_url_search(&json)
}

/// Pure half of S2. Reads only `urls[].resource` (`count`/`score`/
/// relations are never inspected — the IDs-only contract). Identical
/// output for the pre-SEARCH-444 nested `relation-list` shape and the
/// post-flatten flat `relations` shape, because it never looks at
/// either — pinned by the both-era fixture pair
/// (`extract_resources_url_search_pre_shape` /
/// `extract_resources_url_search_post_shape_flat`).
///
/// Missing `urls` key on a 200 body → endpoint-anomaly `Err` (endpoint
/// kind `"url-search"`) — the same "200 but not the documented shape"
/// signature used throughout this module.
pub(super) fn extract_resources_from_url_search(
    json: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let Some(urls) = json.get("urls").and_then(|u| u.as_array()) else {
        return Err(endpoint_anomaly_error(
            "url-search",
            200,
            Some("response body is missing the expected 'urls' array"),
        ));
    };

    let resources = urls
        .iter()
        .filter_map(|hit| hit.get("resource").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    Ok(resources)
}

/// B1 GUARD (BLOCKER fix): accept an S2 hit `resource` ONLY when
/// `apple_music_api::normalize_apple_music_url` →
/// `apple_music_api::parse_apple_music_url` succeeds on it AND the
/// resulting numeric album ID equals `expected_album_id` **exactly**
/// (string equality). `?i={song_id}` track-variant resources of the
/// SAME album pass — `parse_apple_music_url`'s album-ID capture group
/// stops at the digits before any `?i=` query, so the query string
/// never affects this comparison.
///
/// Without this guard, `build_url_search_query`'s trailing wildcard
/// (`*{album_id}*`) can substring-match a resource whose album ID is
/// merely PREFIXED by the expected digits but is actually a longer,
/// different ID — e.g. expected `"1558533900"` must NOT match a hit
/// resource carrying `"15585339001234"`. That would hand a wrong
/// album's video to the existing browse → lookup → download pipeline,
/// which GAMDL would then actually fetch into the user's library
/// (`validate_s2_resource_rejects_longer_id_substring_match` pins
/// this). The 2-resource cap on S2 hits is applied by the caller AFTER
/// this filter, never before.
pub(super) fn validate_s2_resource(expected_album_id: &str, resource: &str) -> bool {
    let normalised = crate::services::apple_music_api::normalize_apple_music_url(resource);
    let Some(parsed) = crate::services::apple_music_api::parse_apple_music_url(&normalised) else {
        return false;
    };
    parsed.album_id == expected_album_id
}

// ============================================================
// Advisory search-era probe (SEARCH-681) — M4
// ============================================================

/// The two states worth acting on, plus `Unknown` for every
/// inconclusive outcome (a 503, a timeout, any other transport or
/// status-code shape). NEVER consulted by any relation parser — see
/// this file's module doc-comment and the readiness plan's "DO NOT DO"
/// list, item 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchEra {
    /// `/ws/2/genre/?query=…` answered 501 — the pre-2026-11-30 state
    /// (probe B10). S2 is skipped while the world is provably
    /// pre-upgrade: recording-URL relationships aren't indexed by
    /// search yet (SEARCH-452), so the request would be near-certain
    /// wasted traffic.
    PreSolr10,
    /// `/ws/2/genre/?query=…` answered 2xx — genre search exists, which
    /// per SEARCH-681 only becomes true after the Solr 10 upgrade.
    PostSolr10,
    /// The probe itself failed or answered something other than 501/2xx
    /// (503, timeout, transport error, unexpected status). The safe
    /// default is to still allow S2 — see [`should_run_url_search`].
    Unknown,
}

/// Advisory-only era probe: `GET /ws/2/genre/?query=rock&limit=1&fmt=json`
/// — byte-identical to the URL probe B10 was captured against
/// (`{MB}/prod-B10-search-genre.json`, a 501 body today). 501 →
/// [`SearchEra::PreSolr10`]; any 2xx → [`SearchEra::PostSolr10`];
/// anything else (503, timeout, connection failure) →
/// [`SearchEra::Unknown`].
///
/// Probe failures are NEVER routed through the endpoint-anomaly warning
/// machinery (`endpoint_anomaly_error` / `should_emit_endpoint_warning`)
/// — this function returns [`SearchEra`], not `Result<_, String>`, so
/// there is no error string that could reach it; a failed probe is
/// simply [`SearchEra::Unknown`] and S2 still runs (fail open, per M4).
pub(super) async fn probe_search_era() -> SearchEra {
    let url = format!("{MB_API_BASE}/genre/?query=rock&limit=1&fmt=json");

    log::debug!("MusicBrainz: advisory search-era probe");

    let Ok(client) = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    ) else {
        return SearchEra::Unknown;
    };

    mb_rate_limit().await;

    match client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) => classify_era_probe(response.status().as_u16()),
        // Transport failure (timeout, DNS, connection refused, …) — an
        // ordinary network condition, not an anomaly signal.
        Err(_) => SearchEra::Unknown,
    }
}

/// Pure half of the probe: classify an HTTP status code into a
/// [`SearchEra`]. Split out purely so the 501/2xx/other decision is
/// unit-testable without a live client.
pub(super) fn classify_era_probe(status: u16) -> SearchEra {
    match status {
        501 => SearchEra::PreSolr10,
        200..=299 => SearchEra::PostSolr10,
        _ => SearchEra::Unknown,
    }
}

/// TTL for a cached [`SearchEra::PreSolr10`] verdict — bounds how stale
/// a "still pre-upgrade" answer can get. A probe made hours before the
/// actual 2026-11-30 cutover would otherwise wrongly suppress S2 for up
/// to a full day after the real upgrade lands.
const PRE_SOLR10_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// M4 cache rules (§0.2): [`SearchEra::PostSolr10`] latches for the
/// process lifetime — a 501 → 200 flip is one-way, so once genre search
/// exists there is nothing left to re-check.
/// [`SearchEra::PreSolr10`] is cached with [`PRE_SOLR10_CACHE_TTL`].
/// [`SearchEra::Unknown`] is NEVER cached — the next S2-eligible album
/// simply re-probes; caching an inconclusive answer would be the exact
/// defect this rule closes (a bad 24 h stretch of Unknown would either
/// wrongly suppress S2's whole existence, or — if Unknown were treated
/// as "run" and then cached as if conclusive — mask a genuine
/// persistent server issue as settled).
///
/// Deliberately WITH a TTL, unlike the feature-flags disk cache's
/// no-TTL rule (`services::feature_flag_service`) — that cache's
/// failure economics are the opposite of this one's: an EXPIRING
/// feature-flag cache could silently re-enable a feature an operator
/// deliberately paused, so it never expires. Here, an expiring
/// `PreSolr10`/never-cached `Unknown` costs at most one extra advisory
/// HTTP request sooner than strictly necessary — never a behaviour
/// change, since the probe only gates a request-economy decision, not
/// a parser (this file's governing contract).
///
/// In-memory only, this process's own cache — never persisted to disk,
/// never shared across app restarts.
static ERA_CACHE: std::sync::LazyLock<tokio::sync::Mutex<Option<(SearchEra, std::time::Instant)>>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));

/// Pure cache-READ half of the M4 rules above (#1120 F4). Extracted out
/// of `current_search_era` so the three cache rules are unit-testable
/// without going through the `static` `ERA_CACHE`/an actual `sleep`.
///
/// `now` is passed in rather than read via `Instant::elapsed()` for a
/// PORTABILITY reason, not a purity one: on Windows `Instant` is an
/// unsigned QPC tick count since boot, so a test that fabricated an
/// old stamp as `Instant::now() - TTL * 10` would underflow and PANIC
/// on a freshly-booted CI runner (`checked_sub` -> `None` -> the `Sub`
/// impl panics), while passing on Unix where `Instant` is a signed
/// timespec. With `now` injected, tests move the *future* side forward
/// by addition (`stamped_at + TTL * 10`), which can never underflow on
/// any platform. Elapsed time is measured with `saturating_duration_since`
/// so a non-monotonic or reordered pair degrades to "zero elapsed"
/// (treat the entry as fresh) instead of panicking.
///
/// `Some(era)` = the cached verdict is still trustworthy, return it
/// without probing. `None` = re-probe (no entry yet, an expired
/// `PreSolr10`, or — structurally unreachable given
/// [`store_verdict`] never writes `Unknown`, but handled the same way
/// regardless — a cached `Unknown`).
fn consult_cache(
    entry: Option<(SearchEra, std::time::Instant)>,
    now: std::time::Instant,
) -> Option<SearchEra> {
    let (era, stamped_at) = entry?;
    match era {
        SearchEra::PostSolr10 => Some(era),
        SearchEra::PreSolr10
            if now.saturating_duration_since(stamped_at) < PRE_SOLR10_CACHE_TTL =>
        {
            Some(era)
        }
        _ => None,
    }
}

/// Pure cache-WRITE half of the M4 rules above (#1120 F4). `Unknown` is
/// NEVER written — returns `None`, meaning "leave the cache as it is"
/// — so an inconclusive probe can never be mistaken for a settled
/// verdict on the next read. `PostSolr10`/`PreSolr10` are written
/// stamped with `now`; [`consult_cache`] is what makes the `PostSolr10`
/// stamp irrelevant in practice (it latches regardless of age), so
/// storing it uniformly here (rather than special-casing "don't bother
/// stamping Post") keeps this function a straight mirror of the doc
/// comment above instead of encoding the latch behaviour twice.
fn store_verdict(
    era: SearchEra,
    now: std::time::Instant,
) -> Option<(SearchEra, std::time::Instant)> {
    if era == SearchEra::Unknown {
        None
    } else {
        Some((era, now))
    }
}

/// Cache-consulting wrapper around [`probe_search_era`] — what
/// `mod.rs`'s S2 wiring actually calls, at most once per album,
/// immediately before deciding (via [`should_run_url_search`]) whether
/// to spend the once-per-album S2 request.
pub(super) async fn current_search_era() -> SearchEra {
    if let Some(era) = consult_cache(*ERA_CACHE.lock().await, std::time::Instant::now()) {
        return era;
    }

    let era = probe_search_era().await;
    if let Some(entry) = store_verdict(era, std::time::Instant::now()) {
        *ERA_CACHE.lock().await = Some(entry);
    }
    era
}

// ============================================================
// Pure gates
// ============================================================

/// Pure gate for S2: `enabled && !attempted_this_album && era !=
/// PreSolr10`. `Unknown` RUNS — the safe direction is "never silently
/// lose capability" for an inconclusive probe; the pre-Nov-30 waste of
/// running S2 when the world actually IS still pre-upgrade is bounded
/// at exactly 1 request per album (`search_url_resources` alone —
/// there will be nothing to validate/browse/lookup, since
/// recording-URL relationships aren't indexed by search until
/// SEARCH-452 lands).
pub(super) fn should_run_url_search(
    era: SearchEra,
    attempted_this_album: bool,
    enabled: bool,
) -> bool {
    enabled && !attempted_this_album && era != SearchEra::PreSolr10
}

/// Pure gate for S1: `enabled && !found && attempts_this_album <
/// MAX_SEARCH_ATTEMPTS_PER_ALBUM && has_artist_and_title`.
///
/// `found` is §0.2 M1's definition — EXACT-IDENTIFIER resolution (any
/// of T1/T2/T3 returning `Ok(Some(_))`), regardless of whether the
/// resolved recording carried video relations. A track whose ISRC
/// resolved to a videoless recording must NOT re-run S1: with the
/// merge-across-all-ISRC-recordings fix (`relations.rs` §7.3), every
/// relation MusicBrainz has for that recording has already been
/// inspected, so a text search would spend two more requests to
/// rediscover the same recording for ~zero yield.
///
/// `has_artist_and_title` is the caller's pre-computed, trimmed,
/// non-blank check on both `track.artist` and `track.title` (m5:
/// `Some("")` and whitespace-only both collapse to "absent" at the
/// call site, mirrored by
/// `should_attempt_search_tier_requires_nonblank_artist_and_title`
/// below) — this function only ANDs the already-computed bool in.
pub(super) fn should_attempt_search_tier(
    found: bool,
    attempts_this_album: usize,
    has_artist_and_title: bool,
    enabled: bool,
) -> bool {
    enabled && !found && attempts_this_album < MAX_SEARCH_ATTEMPTS_PER_ALBUM && has_artist_and_title
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Fixtures — real MusicBrainz captures, 2026-09-01
    // ----------------------------------------------------------

    /// Trimmed subset of the real capture `{MB}/prod-B5-search-isrc.json`
    /// (`GET /ws/2/recording/?query=isrc:GBAYE0601498`) — the same
    /// result shape B6's artist+title lucene search returns
    /// (`{MB}/prod-B6-search-lucene.json`, query
    /// `artist:"The Beatles" AND recording:"Yellow Submarine"`, per
    /// `01-live-probe.md`'s B6 note "same result shape as B5; no
    /// relations; scored"). Every kept value (id/score/title/artist
    /// name) is copied byte-for-byte from the real capture; the
    /// enormous per-artist `aliases` array (40+ locale variants of "The
    /// Beatles") is trimmed — it plays no role in any assertion below.
    /// Confirms probe B5's structural claim: recording search output
    /// carries NO `relations` key at all.
    fn b5_recording_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-09-01T20:17:56.052Z",
            "count": 1,
            "offset": 0,
            "recordings": [
                {
                    "id": "b2181aae-5cba-496c-bb0c-b4cc0109ebf8",
                    "score": 100,
                    "title": "Yellow Submarine",
                    "length": 160000,
                    "artist-credit": [
                        {
                            "name": "The Beatles",
                            "artist": {
                                "id": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
                                "name": "The Beatles"
                            }
                        }
                    ]
                }
            ]
        })
    }

    /// Full real capture, `{MB}/prod-B7-search-url.json`
    /// (`GET /ws/2/url/?query=url:"…"`) — pre-Nov-30 url SEARCH shape:
    /// relations nested under `relation-list[].relations[]`
    /// (SEARCH-444), no `target-type` at all (SEARCH-751/753). Same
    /// fixture body `relations.rs` uses to test nesting tolerance
    /// (isolated there to just the hit object); here it's the full
    /// search-envelope shape `extract_resources_from_url_search`
    /// actually parses.
    fn b7_search_url_fixture() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-09-01T20:20:30.153Z",
            "count": 1,
            "offset": 0,
            "urls": [
                {
                    "id": "29566add-95ae-45e8-9bbd-a77fbd14094f",
                    "score": 100,
                    "resource": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "relation-list": [
                        {
                            "relations": [
                                {
                                    "type": "free streaming",
                                    "type-id": "08445ccf-7b99-4438-9f9a-fb9ac18099ee",
                                    "direction": "backward",
                                    "release": {
                                        "id": "657a3c08-d22b-4f10-b7ab-becf05bdf3e9",
                                        "title": "With Skin Like Silverfish (demo)"
                                    }
                                },
                                {
                                    "type": "free streaming",
                                    "type-id": "08445ccf-7b99-4438-9f9a-fb9ac18099ee",
                                    "direction": "backward",
                                    "release": {
                                        "id": "dbb9ee96-4b20-42a3-a326-5b250a22c5f9",
                                        "title": "Never Gonna Give You Up",
                                        "disambiguation": "7\" vinyl single"
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    }

    /// SYNTHETIC post-Solr-10 shape from ticket text — replace with a
    /// real capture once beta/test.musicbrainz.org flips (tracked in
    /// this work's GitHub Issue). Byte-for-byte the same
    /// resource/id/score values as [`b7_search_url_fixture`], reshaped
    /// per SEARCH-444 (flat `relations[]` instead of nested
    /// `relation-list[].relations[]`) and SEARCH-751/753 (explicit
    /// `target-type` on each relation). Demonstrates
    /// `extract_resources_from_url_search` is shape-agnostic across
    /// both eras: it never even looks at the relations, only
    /// `urls[].resource`.
    fn b7_search_url_post_upgrade_synthetic_fixture() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-09-01T20:20:30.153Z",
            "count": 1,
            "offset": 0,
            "urls": [
                {
                    "id": "29566add-95ae-45e8-9bbd-a77fbd14094f",
                    "score": 100,
                    "resource": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "relations": [
                        {
                            "type": "free streaming",
                            "type-id": "08445ccf-7b99-4438-9f9a-fb9ac18099ee",
                            "target-type": "release",
                            "direction": "backward",
                            "release": {
                                "id": "657a3c08-d22b-4f10-b7ab-becf05bdf3e9",
                                "title": "With Skin Like Silverfish (demo)"
                            }
                        },
                        {
                            "type": "free streaming",
                            "type-id": "08445ccf-7b99-4438-9f9a-fb9ac18099ee",
                            "target-type": "release",
                            "direction": "backward",
                            "release": {
                                "id": "dbb9ee96-4b20-42a3-a326-5b250a22c5f9",
                                "title": "Never Gonna Give You Up",
                                "disambiguation": "7\" vinyl single"
                            }
                        }
                    ]
                }
            ]
        })
    }

    /// Full real capture, `{MB}/prod-M3iv-url-applemusic-id-anchored.json`
    /// — THE live probe that proves S2's query shape (§0.2 M3):
    /// `url:https\://music.apple.com/*/album/*1456105020*` returned
    /// exactly these two hits, one per storefront, of exactly this one
    /// album.
    fn m3iv_id_anchored_fixture() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-09-01T20:53:19.616Z",
            "count": 2,
            "offset": 0,
            "urls": [
                {
                    "id": "9b2438a8-b492-4efb-a972-577da8d48f39",
                    "score": 100,
                    "resource": "https://music.apple.com/us/album/1456105020",
                    "relation-list": [
                        {
                            "relations": [
                                {
                                    "type": "purchase for download",
                                    "type-id": "98e08c20-8402-4163-8970-53504bb6a1e4",
                                    "direction": "backward",
                                    "release": {
                                        "id": "d19079d2-6f50-449e-a47c-ba1216357e98",
                                        "title": "You Can Do It / Back in the Days"
                                    }
                                }
                            ]
                        }
                    ]
                },
                {
                    "id": "03f8781e-9758-4350-996c-dd9e2908b8fd",
                    "score": 100,
                    "resource": "https://music.apple.com/gb/album/1456105020",
                    "relation-list": [
                        {
                            "relations": [
                                {
                                    "type": "streaming",
                                    "type-id": "320adf26-96fa-4183-9045-1f5f32f833cb",
                                    "direction": "backward",
                                    "release": {
                                        "id": "5be15886-dfb7-4cbf-a9c3-c68a3172d534",
                                        "title": "You Can Do It / Back in the Days"
                                    }
                                },
                                {
                                    "type": "purchase for download",
                                    "type-id": "98e08c20-8402-4163-8970-53504bb6a1e4",
                                    "direction": "backward",
                                    "release": {
                                        "id": "5be15886-dfb7-4cbf-a9c3-c68a3172d534",
                                        "title": "You Can Do It / Back in the Days"
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    }

    // ----------------------------------------------------------
    // S1 — extract_mbid_from_recording_search
    // ----------------------------------------------------------

    #[test]
    fn extract_mbid_from_recording_search_happy() {
        let result = extract_mbid_from_recording_search(&b5_recording_search_fixture(), "The Beatles");
        assert_eq!(
            result,
            Ok(Some("b2181aae-5cba-496c-bb0c-b4cc0109ebf8".to_string()))
        );
    }

    #[test]
    fn extract_mbid_rejects_low_score() {
        let mut fixture = b5_recording_search_fixture();
        fixture["recordings"][0]["score"] = serde_json::json!(60);
        assert_eq!(
            extract_mbid_from_recording_search(&fixture, "The Beatles"),
            Ok(None)
        );
    }

    #[test]
    fn extract_mbid_rejects_artist_mismatch() {
        assert_eq!(
            extract_mbid_from_recording_search(&b5_recording_search_fixture(), "Oasis"),
            Ok(None)
        );
    }

    #[test]
    fn extract_mbid_accepts_case_differing_artist() {
        // M2b: trim + str::to_lowercase() on both sides.
        let result =
            extract_mbid_from_recording_search(&b5_recording_search_fixture(), "the beatles");
        assert_eq!(
            result,
            Ok(Some("b2181aae-5cba-496c-bb0c-b4cc0109ebf8".to_string()))
        );
    }

    #[test]
    fn extract_mbid_accepts_multi_credit_joined_artist() {
        // M2b: the JOINED credit string (name + joinphrase per entry,
        // concatenated in order) is a second acceptance path alongside
        // per-credit name equality — needed for genuine multi-artist
        // recordings where no single credit entry equals the full
        // expected display string on its own.
        let fixture = serde_json::json!({
            "recordings": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "score": 100,
                    "artist-credit": [
                        { "name": "Artist A", "joinphrase": " feat. " },
                        { "name": "Artist B" }
                    ]
                }
            ]
        });
        assert_eq!(
            extract_mbid_from_recording_search(&fixture, "Artist A feat. Artist B"),
            Ok(Some("11111111-1111-1111-1111-111111111111".to_string()))
        );
    }

    #[test]
    fn extract_mbid_accepts_accented_artist_exact_bytes() {
        // M2b: str::to_lowercase() is Unicode-aware (no NFC/NFD
        // normalisation crate needed) — an uppercase-accented credit
        // name still matches a differently-cased expected artist once
        // both sides are lowercased.
        let fixture = serde_json::json!({
            "recordings": [
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "score": 100,
                    "artist-credit": [ { "name": "BEYONCÉ" } ]
                }
            ]
        });
        assert_eq!(
            extract_mbid_from_recording_search(&fixture, "Beyoncé"),
            Ok(Some("22222222-2222-2222-2222-222222222222".to_string()))
        );
    }

    #[test]
    fn extract_mbid_empty_recordings_is_quiet_none() {
        let fixture = serde_json::json!({ "count": 0, "recordings": [] });
        assert_eq!(extract_mbid_from_recording_search(&fixture, "Anyone"), Ok(None));
    }

    #[test]
    fn extract_mbid_missing_recordings_key_is_anomaly() {
        let fixture = serde_json::json!({ "count": 0 });
        let result = extract_mbid_from_recording_search(&fixture, "Anyone");
        let Err(e) = result else {
            panic!("expected Err, got {result:?}");
        };
        assert!(is_endpoint_anomaly(&e));
    }

    #[test]
    fn recording_search_output_has_no_relations() {
        // Documents the structural fact the IDs-only search contract
        // rests on (probe B5): recording search never returns
        // relations at all, so S1 has no relations to accidentally
        // consume even if a future edit tried to.
        let fixture = b5_recording_search_fixture();
        assert!(fixture["recordings"][0].get("relations").is_none());
    }

    // ----------------------------------------------------------
    // S2 — extract_resources_from_url_search
    // ----------------------------------------------------------

    #[test]
    fn extract_resources_url_search_pre_shape() {
        assert_eq!(
            extract_resources_from_url_search(&b7_search_url_fixture()),
            Ok(vec!["https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string()])
        );
    }

    #[test]
    fn extract_resources_url_search_post_shape_flat() {
        // The both-era pair: byte-identical output for the pre- and
        // post-upgrade envelope shapes, because this function never
        // reads the relations at all — only `urls[].resource`.
        assert_eq!(
            extract_resources_from_url_search(&b7_search_url_post_upgrade_synthetic_fixture()),
            extract_resources_from_url_search(&b7_search_url_fixture())
        );
    }

    #[test]
    fn extract_resources_id_anchored_fixture() {
        assert_eq!(
            extract_resources_from_url_search(&m3iv_id_anchored_fixture()),
            Ok(vec![
                "https://music.apple.com/us/album/1456105020".to_string(),
                "https://music.apple.com/gb/album/1456105020".to_string(),
            ])
        );
    }

    #[test]
    fn extract_resources_missing_urls_key_is_anomaly() {
        let fixture = serde_json::json!({ "count": 0 });
        let result = extract_resources_from_url_search(&fixture);
        let Err(e) = result else {
            panic!("expected Err, got {result:?}");
        };
        assert!(is_endpoint_anomaly(&e));
    }

    // ----------------------------------------------------------
    // escape_lucene / build_url_search_query
    // ----------------------------------------------------------

    #[test]
    fn escape_lucene_specials_and_quotes() {
        assert_eq!(escape_lucene("AC/DC"), r#"AC\/DC"#);
        assert_eq!(
            escape_lucene("\"Weird Al\" Yankovic"),
            r#"\"Weird\ Al\"\ Yankovic"#
        );
        assert_eq!(escape_lucene("R&B || Soul"), r#"R\&B\ \|\|\ Soul"#);
    }

    #[test]
    fn build_url_search_query_matches_proven_shape() {
        // M3-iv proof: this exact string returned count 2 — both
        // storefront variants of exactly this one album.
        assert_eq!(
            build_url_search_query("1456105020"),
            r#"url:https\://music.apple.com/*/album/*1456105020*"#
        );
    }

    // ----------------------------------------------------------
    // validate_s2_resource — the B1 guard
    // ----------------------------------------------------------

    #[test]
    fn validate_s2_resource_accepts_both_storefronts() {
        assert!(validate_s2_resource(
            "1456105020",
            "https://music.apple.com/us/album/1456105020"
        ));
        assert!(validate_s2_resource(
            "1456105020",
            "https://music.apple.com/gb/album/1456105020"
        ));
    }

    #[test]
    fn validate_s2_resource_rejects_longer_id_substring_match() {
        // THE B1 pin: S2's trailing wildcard can substring-match a
        // resource whose album ID merely starts with the expected
        // digits but is actually longer/different. Without this guard
        // a wrong-album video would reach the browse -> lookup ->
        // GAMDL-download pipeline.
        assert!(!validate_s2_resource(
            "1456105020",
            "https://music.apple.com/us/album/x/14561050201234"
        ));
    }

    #[test]
    fn validate_s2_resource_accepts_track_variant_of_same_album() {
        assert!(validate_s2_resource(
            "1456105020",
            "https://music.apple.com/us/album/slug/1456105020?i=1456105021"
        ));
    }

    #[test]
    fn validate_s2_resource_rejects_unparseable_and_foreign_urls() {
        assert!(!validate_s2_resource(
            "1456105020",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
        assert!(!validate_s2_resource("1456105020", "not a url at all"));
    }

    // ----------------------------------------------------------
    // Era probe classification (M4)
    // ----------------------------------------------------------

    #[test]
    fn classify_era_probe_501_is_pre() {
        assert_eq!(classify_era_probe(501), SearchEra::PreSolr10);
    }

    #[test]
    fn classify_era_probe_200_is_post() {
        assert_eq!(classify_era_probe(200), SearchEra::PostSolr10);
    }

    #[test]
    fn classify_era_probe_5xx_is_unknown() {
        assert_eq!(classify_era_probe(503), SearchEra::Unknown);
    }

    // ----------------------------------------------------------
    // M4 era-probe cache rules (#1120 F4)
    // ----------------------------------------------------------

    #[test]
    fn consult_cache_post_solr10_latches_past_the_ttl() {
        // PostSolr10 must return from cache for the process lifetime —
        // a 501 -> 200 flip is one-way, so it never re-probes, even
        // when the stamp is far older than PRE_SOLR10_CACHE_TTL (which
        // only bounds PreSolr10).
        //
        // Age is fabricated by moving `now` FORWARD from a real stamp,
        // never by subtracting from `Instant::now()`: on Windows
        // `Instant` is an unsigned tick count since boot, so
        // `Instant::now() - 240h` panics on a freshly-booted CI runner
        // (ci.yml runs the Backend job on windows-latest). Addition is
        // underflow-free on every platform. Same rule in the sibling
        // test below.
        let stamped_at = std::time::Instant::now();
        let now = stamped_at + PRE_SOLR10_CACHE_TTL * 10;
        let entry = Some((SearchEra::PostSolr10, stamped_at));
        assert_eq!(consult_cache(entry, now), Some(SearchEra::PostSolr10));
    }

    #[test]
    fn consult_cache_pre_solr10_expires_after_ttl() {
        let stamped_at = std::time::Instant::now();

        // Still within the TTL -> cached verdict returned, no re-probe.
        let fresh = stamped_at + std::time::Duration::from_secs(60);
        assert_eq!(
            consult_cache(Some((SearchEra::PreSolr10, stamped_at)), fresh),
            Some(SearchEra::PreSolr10)
        );

        // Past the TTL -> None, so `current_search_era` falls through
        // and re-probes. Without this rule a probe made hours before
        // the real 2026-11-30 cutover would wrongly suppress S2 for up
        // to a full day after the upgrade actually lands.
        let stale = stamped_at + PRE_SOLR10_CACHE_TTL + std::time::Duration::from_secs(1);
        assert_eq!(
            consult_cache(Some((SearchEra::PreSolr10, stamped_at)), stale),
            None
        );
    }

    #[test]
    fn store_verdict_never_caches_unknown() {
        // The rule this test pins: an inconclusive probe (503,
        // timeout, …) must never be written to the cache, or a bad
        // stretch of Unknown answers would either wrongly suppress
        // S2's whole existence (if read as "cached miss") or mask a
        // genuine persistent server issue as a settled verdict (if
        // read as "cached conclusive").
        let now = std::time::Instant::now();
        assert_eq!(store_verdict(SearchEra::Unknown, now), None);

        // Both conclusive verdicts ARE written, stamped with `now`.
        assert_eq!(
            store_verdict(SearchEra::PostSolr10, now),
            Some((SearchEra::PostSolr10, now))
        );
        assert_eq!(
            store_verdict(SearchEra::PreSolr10, now),
            Some((SearchEra::PreSolr10, now))
        );
    }

    // ----------------------------------------------------------
    // Pure gates
    // ----------------------------------------------------------

    #[test]
    fn should_run_url_search_unknown_era_runs() {
        // The safe-default pin: an inconclusive probe must never
        // silently lose S2's capability — the pre-Nov-30 cost is
        // bounded at one wasted request per album.
        assert!(should_run_url_search(SearchEra::Unknown, false, true));
    }

    #[test]
    fn should_run_url_search_pre_era_skips_and_once_per_album() {
        assert!(!should_run_url_search(SearchEra::PreSolr10, false, true));
        assert!(!should_run_url_search(SearchEra::PostSolr10, true, true));
        assert!(!should_run_url_search(SearchEra::PostSolr10, false, false));
    }

    #[test]
    fn search_tier_skipped_when_isrc_resolved_even_without_videos() {
        // The M1 semantics pin: `found` means EXACT-IDENTIFIER
        // resolution, not "a video was found". A track whose ISRC
        // resolved to a videoless MusicBrainz recording must NOT
        // re-run S1 — every relation MusicBrainz has for that
        // recording was already inspected (relations.rs §7.3's
        // merge-across-all-recordings fix); a text search would just
        // rediscover the same recording for ~zero yield.
        assert!(!should_attempt_search_tier(true, 0, true, true));
    }

    #[test]
    fn should_attempt_search_tier_caps_per_album() {
        assert!(!should_attempt_search_tier(
            false,
            MAX_SEARCH_ATTEMPTS_PER_ALBUM,
            true,
            true
        ));
        assert!(should_attempt_search_tier(
            false,
            MAX_SEARCH_ATTEMPTS_PER_ALBUM - 1,
            true,
            true
        ));
    }

    #[test]
    fn should_attempt_search_tier_requires_nonblank_artist_and_title() {
        // m5: mirrors mod.rs's call-site blank check — `Some("")` and
        // whitespace-only strings collapse to "absent" via a trim
        // check, not merely "field is None". `has_artist_and_title`
        // below is this test's stand-in for that computation; the gate
        // itself just ANDs the already-computed bool in.
        fn has_artist_and_title(artist: Option<&str>, title: Option<&str>) -> bool {
            artist.is_some_and(|s| !s.trim().is_empty())
                && title.is_some_and(|s| !s.trim().is_empty())
        }

        assert!(!should_attempt_search_tier(
            false,
            0,
            has_artist_and_title(Some(""), Some("Yellow Submarine")),
            true
        ));
        assert!(!should_attempt_search_tier(
            false,
            0,
            has_artist_and_title(Some("   "), Some("Yellow Submarine")),
            true
        ));
        assert!(!should_attempt_search_tier(
            false,
            0,
            has_artist_and_title(None, Some("Yellow Submarine")),
            true
        ));
        assert!(should_attempt_search_tier(
            false,
            0,
            has_artist_and_title(Some("The Beatles"), Some("Yellow Submarine")),
            true
        ));
    }

    #[test]
    fn search_anomaly_string_triggers_warn_once() {
        // Pins that `should_emit_endpoint_warning` (mod.rs, shared
        // across every tier) correctly recognises the two endpoint
        // kinds this tranche introduces, and that the once-per-album
        // suppression is global (not per-kind): a second anomaly, even
        // of a DIFFERENT kind, stays suppressed once the flag has
        // fired once this album. Uses 500, not 503, for the second
        // anomaly — since #1120 F5, a real 503 from either search
        // endpoint no longer classifies as an anomaly at all (see
        // `search_status_error_treats_503_and_429_as_throttle_not_anomaly`
        // below); this test is only exercising the generic
        // `should_emit_endpoint_warning` string-matching/suppression
        // behaviour, so any genuine anomaly status works.
        let anomaly = endpoint_anomaly_error("recording-search", 200, None);
        let mut flag = false;
        assert!(should_emit_endpoint_warning(&anomaly, &mut flag));
        assert!(!should_emit_endpoint_warning(&anomaly, &mut flag));

        let anomaly2 = endpoint_anomaly_error("url-search", 500, None);
        assert!(!should_emit_endpoint_warning(&anomaly2, &mut flag));
    }

    #[test]
    fn search_status_error_treats_503_and_429_as_throttle_not_anomaly() {
        // #1120 F5: the live probe showed MusicBrainz burst-throttling
        // even at RATE_LIMIT_DELAY spacing, returning 503 on an
        // ordinary S1/S2 request. That must NOT consume the album's
        // one non-verbose `is_endpoint_anomaly` warning — it's a
        // transport condition, not a server-shape-change signal.
        let err_503 = search_status_error("recording-search", 503);
        assert!(!is_endpoint_anomaly(&err_503));
        assert!(err_503.contains("throttled"));
        assert!(err_503.contains("503"));

        let err_429 = search_status_error("url-search", 429);
        assert!(!is_endpoint_anomaly(&err_429));
        assert!(err_429.contains("throttled"));
        assert!(err_429.contains("429"));
    }

    #[test]
    fn search_status_error_other_non_2xx_still_anomaly() {
        // A genuine shape-change signature — anything other than the
        // two throttle statuses — must still be classified as an
        // endpoint anomaly on the new search endpoints, exactly as it
        // is on the legacy lookup/browse endpoints (that discipline is
        // deliberately unchanged by #1120 F5).
        let err = search_status_error("recording-search", 500);
        assert!(is_endpoint_anomaly(&err));

        let err2 = search_status_error("url-search", 404);
        assert!(is_endpoint_anomaly(&err2));
    }
}
