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
//! the `MeedyaMeta:<Platform>Url` freeform atoms (one per service —
//! see [`atom_name_for_platform`]). Users can:
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
//! ## What song.link allows
//!
//! The endpoint is `GET https://api.song.link/v1-alpha.1/links?url={encoded_url}`.
//! When we have an access key it rides along as one extra query
//! parameter, `&key=…` — nothing else about the request changes.
//!
//! Odesli closed free public access to this endpoint at some point in
//! 2026. Two live probes made from this machine on 2026-09-09
//! (repeated twice, identical result both times) show it plainly:
//!   - No key at all: `HTTP 401` with body
//!     `{"statusCode":401,"code":"PUBLIC_API_ACCESS_DEPRECATED"}`.
//!   - A made-up key: `HTTP 401` with body
//!     `{"statusCode":401,"code":"INVALID_ACCESS_KEY"}`.
//!
//! So a real access key is now the only way this works at all — there
//! is no keyless fallback any more. Odesli grants keys by application;
//! see <https://odesli.co/help>.
//!
//! Before it closed, the keyless allowance was 10 requests a minute.
//! We know that exact figure because it came straight from the API's
//! own "slow down" reply, not because Odesli published it as a
//! documented limit anywhere. The keyed figure of 60 a minute has
//! **never** been published by Odesli at all — it's this project's own
//! assumption, carried over from issue #295. Every constant and
//! comment below that leans on it says so, rather than presenting it
//! as a confirmed fact.
//!
//! ## Pacing
//!
//! Only one Odesli request is ever in flight for the whole process at
//! a time, no matter how many albums happen to be enriching in
//! parallel — there is one shared limiter, not one per album. Without
//! a key we wait 6.5 seconds between requests (nine a minute,
//! comfortably under the ten we know are allowed); with a key we wait
//! 1.1 seconds (about fifty-four a minute, comfortably under the
//! sixty we're assuming). [`request_gap`] is the single place that
//! decides which of the two applies — nothing else in this file works
//! out a wait time on its own. A unit test pins the no-key figure at
//! no more than ten requests a minute, so a future change can't
//! quietly widen it back past the limit we actually know about.
//!
//! When song.link answers "too many requests", we wait exactly as
//! long as its `Retry-After` header asks for — one minute when it
//! doesn't say, and never more than one minute even if it asks for
//! longer — then try exactly once more. If that second attempt is
//! also refused, we give up and report it rather than trying a third
//! time (see [`OdesliError::RateLimited`]). Crucially, that wait
//! happens while still holding the shared limiter: this is
//! deliberate, not an oversight, because it's what stops a second
//! album's lookup from firing straight into the same cool-down
//! instead of queueing politely behind the first one.
//!
//! ## Remembering answers
//!
//! Every answer we get back — including "no matches found" — is kept
//! for 30 days in `odesli-cache.json` in the app's data folder. That
//! means retrying a download that failed partway through, fetching a
//! different codec of an album we've already looked up, or filling a
//! gap a library scan found all cost no extra request the second
//! time. This matters more now than it used to: every request we
//! send counts against a quota that has to be granted by application,
//! not a free tap we can lean on. The cache is capped at 1000 albums;
//! once full, the oldest looked-up entry is dropped to make room for
//! a new one.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use mp4ameta::{Data, FreeformIdent, Tag};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Public-facing Odesli endpoint. Versioned `v1-alpha.1` per the
/// Songlink docs; stable in practice for years.
const ODESLI_BASE_URL: &str = "https://api.song.link/v1-alpha.1/links";

/// Gap between requests when we have no API key. 60 seconds split
/// nine ways, not ten — one request of headroom under the keyless
/// limit song.link itself told us about (10 requests a minute), so
/// ordinary scheduler jitter never tips us over it.
const RATE_LIMIT_GAP_NO_KEY: Duration = Duration::from_millis(6500);

/// Gap between requests when we do have a key. 60 seconds split a
/// little over fifty-four ways. Odesli has never told us the keyed
/// limit — 60 requests a minute is this project's own assumption
/// (carried over from issue #295), not a number Odesli published. If
/// a real key ever arrives with a stated limit, replace this constant
/// with that number instead of guessing again.
const RATE_LIMIT_GAP_WITH_KEY: Duration = Duration::from_millis(1100);

/// The longest we will ever wait because song.link told us to slow
/// down. Used whenever its `Retry-After` header is missing or can't
/// be read as whole seconds; when it names a shorter wait we honour
/// that instead, and we never wait longer than this even if the
/// header asks for more.
const RATE_LIMIT_BACKOFF_MAX: Duration = Duration::from_secs(60);

/// How many goes one lookup gets: the first, and one more only if
/// song.link asked us to slow down. Pushing harder than that is how a
/// temporary "slow down" turns into a lasting block.
const MAX_ATTEMPTS: u8 = 2;

/// Name of the on-disk cache file, inside the app's data folder.
pub const CACHE_FILENAME: &str = "odesli-cache.json";

/// How many days a cached answer stays usable before we're willing to
/// ask song.link again for the same album.
const CACHE_MAX_AGE_DAYS: i64 = 30;

/// Largest number of albums the cache will hold at once. Beyond this,
/// the oldest looked-up entry is dropped to make room for the newest
/// one.
const CACHE_MAX_ENTRIES: usize = 1000;

/// The shared, per-process pacing state. `last_request_at` is when
/// the most recent request *started* (used to enforce the ordinary
/// gap between requests); `blocked_until` is set only after a "too
/// many requests" reply and holds the moment we're next allowed to
/// try again.
struct Limiter {
    last_request_at: Option<Instant>,
    blocked_until: Option<Instant>,
}

/// One limiter for the whole process, so every album's Odesli lookup
/// — however many are enriching in parallel — queues through the same
/// gate instead of each keeping its own clock and racing the others
/// into the same rate limit.
static LIMITER: LazyLock<Mutex<Limiter>> = LazyLock::new(|| {
    Mutex::new(Limiter {
        last_request_at: None,
        blocked_until: None,
    })
});

/// Set once song.link tells us keyless access is closed
/// (`PUBLIC_API_ACCESS_DEPRECATED`), so we stop spending a request
/// finding that out again for the rest of this run. Worst case this
/// costs one wasted request per session; it resets on the next app
/// launch, so if Odesli ever reopens keyless access we'd notice
/// within one restart rather than staying blocked forever.
static PUBLIC_ACCESS_CLOSED: AtomicBool = AtomicBool::new(false);

/// The exact access key song.link most recently turned down
/// (`INVALID_ACCESS_KEY`), so we stop sending that same rejected key
/// on every subsequent album's lookup. A *different* key — for
/// example after the user fixes a typo in Settings — won't match what
/// is stored here, so it's tried immediately rather than being
/// blocked by an old rejection.
static REJECTED_KEY: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

/// The in-memory copy of the on-disk answer cache. Loaded lazily on
/// first use and then kept in memory for the rest of the process, so
/// we don't re-read the file from disk on every single lookup.
static CACHE: LazyLock<Mutex<Option<OdesliCacheFile>>> = LazyLock::new(|| Mutex::new(None));

/// Per-platform URL map. Keys mirror Odesli's platform identifiers
/// (`spotify`, `youtube`, `youtubeMusic`, `tidal`, `deezer`,
/// `amazonMusic`, `soundcloud`, `bandcamp`, `pandora`, …) but we
/// don't enumerate them as a Rust enum — every new platform Odesli
/// adds should just appear in the map without code changes.
pub type CrossPlatformUrls = BTreeMap<String, String>;

/// Everything that can go wrong asking song.link for cross-platform
/// URLs. Kept as a closed enum rather than a bare `String` so a
/// caller — today the enrichment pipeline, later maybe a Settings
/// page — can tell "the user needs to do something about this" apart
/// from "this was a one-off network hiccup" without parsing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OdesliError {
    /// song.link no longer accepts requests without an access key at
    /// all.
    PublicAccessClosed,
    /// The access key we sent was refused.
    KeyRejected,
    /// song.link asked us to slow down twice in a row.
    RateLimited,
    /// Any other non-2xx HTTP status.
    Http(u16),
    /// We couldn't reach song.link at all — DNS failure, timeout,
    /// connection refused, and so on. Carries `reqwest`'s `Display`
    /// text, which — unlike its `Debug` text — never includes the
    /// request URL, so the access key riding along in the query
    /// string can't end up leaking into a log line.
    Network(String),
    /// song.link answered, but the body wasn't the JSON shape we
    /// expected.
    Parse(String),
}

impl OdesliError {
    /// True for the two problems only the user can actually do
    /// anything about — a missing key or a rejected one. The
    /// enrichment pipeline uses this to decide whether a failure
    /// belongs in the user-visible activity log (something to act
    /// on) or just the debug log (a one-off blip likely to clear up
    /// on its own).
    #[must_use]
    pub fn is_user_actionable(&self) -> bool {
        matches!(self, OdesliError::PublicAccessClosed | OdesliError::KeyRejected)
    }
}

impl std::fmt::Display for OdesliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OdesliError::PublicAccessClosed => write!(
                f,
                "song.link no longer answers requests without an access key (Odesli closed free public access in 2026). Add a key in Settings > Advanced > API Credentials, or switch the lookup off in Settings > Metadata."
            ),
            OdesliError::KeyRejected => write!(
                f,
                "song.link rejected the access key. Check it in Settings > Advanced > API Credentials."
            ),
            OdesliError::RateLimited => write!(
                f,
                "song.link asked us to slow down, and the one retry was refused too."
            ),
            OdesliError::Http(n) => write!(f, "song.link answered with HTTP {n}."),
            OdesliError::Network(e) => write!(f, "could not reach song.link: {e}"),
            OdesliError::Parse(e) => write!(f, "song.link's reply could not be read: {e}"),
        }
    }
}

impl std::error::Error for OdesliError {}

/// How long to wait between two consecutive Odesli requests. This is
/// the single place that decides — see the "Pacing" section of the
/// module doc above for why the two numbers are what they are.
pub(crate) fn request_gap(has_api_key: bool) -> Duration {
    if has_api_key {
        RATE_LIMIT_GAP_WITH_KEY
    } else {
        RATE_LIMIT_GAP_NO_KEY
    }
}

/// Whether an API key is actually usable: `Some` and not just
/// whitespace once trimmed. Settings can hold an empty string, or a
/// few stray spaces left over from copy-pasting; both should be
/// treated exactly like "no key", not sent to song.link as though
/// they were one.
fn has_usable_key(api_key: Option<&str>) -> bool {
    api_key.is_some_and(|k| !k.trim().is_empty())
}

/// Reads the `Retry-After` header from a "too many requests" reply
/// and turns it into a wait time. song.link is only ever expected to
/// send a whole number of seconds, so that's all we try to parse.
/// Anything else — missing, an HTTP-date string, garbage — falls back
/// to the longest wait we're willing to do, which is always the safe
/// direction to guess wrong in: waiting a little too long never
/// causes a problem, waiting too little can trip the same limit
/// again immediately.
fn retry_after_delay(header: Option<&str>) -> Duration {
    let Some(raw) = header else {
        return RATE_LIMIT_BACKOFF_MAX;
    };
    let Ok(seconds) = raw.trim().parse::<u64>() else {
        return RATE_LIMIT_BACKOFF_MAX;
    };
    Duration::from_secs(seconds).clamp(Duration::from_secs(1), RATE_LIMIT_BACKOFF_MAX)
}

/// Works out the earliest moment a new request is allowed to fire:
/// whichever is later out of "the last request plus the ordinary gap"
/// and "the end of a rate-limit cool-down". Taking the later of the
/// two means a cool-down from a 429 can never be shortened by the
/// ordinary pacing gap running out first.
fn next_allowed_at(
    last: Option<Instant>,
    blocked_until: Option<Instant>,
    gap: Duration,
) -> Option<Instant> {
    let from_gap = last.map(|t| t + gap);
    match (from_gap, blocked_until) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Reads the `code` field out of a 401 body and turns it into the
/// matching error. Anything we don't recognise — an empty object, a
/// body that isn't JSON at all, a `code` we've never seen — becomes a
/// plain `Http(401)` rather than us guessing at a cause we can't
/// actually confirm.
fn classify_unauthorised(body: &str) -> OdesliError {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return OdesliError::Http(401);
    };
    match json.get("code").and_then(|v| v.as_str()) {
        Some("PUBLIC_API_ACCESS_DEPRECATED") => OdesliError::PublicAccessClosed,
        Some("INVALID_ACCESS_KEY") => OdesliError::KeyRejected,
        _ => OdesliError::Http(401),
    }
}

/// Turns an Odesli platform key (`spotify`, `youtubeMusic`, …) into
/// the freeform atom name we write its URL under (`SpotifyUrl`,
/// `YoutubeMusicUrl`, …).
///
/// This is deliberately mechanical — capitalise the first letter,
/// leave the rest exactly as Odesli sent it, add `Url` — rather than
/// a hand-written table mapping every known platform to its atom
/// name. A table needs updating every time Odesli adds a service;
/// this doesn't, so a platform we've never even heard of still lands
/// on disk under a sensible name with zero code changes. The
/// trade-off is spelling: a platform whose own brand capitalises
/// differently than "just the first letter" comes out slightly off —
/// this produces `YoutubeMusicUrl`, not `YouTubeMusicUrl`. That's
/// judged an acceptable, purely cosmetic cost for never having to
/// maintain a table by hand.
///
/// Only names made entirely of ASCII letters and digits are accepted.
/// Anything else — empty, punctuation, spaces, non-ASCII characters —
/// is refused outright, so we can never build a freeform atom name
/// that looks confusing or trips up something downstream that reads
/// these atoms.
pub(crate) fn atom_name_for_platform(platform: &str) -> Option<String> {
    if platform.is_empty() || !platform.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let mut chars = platform.chars();
    let first = chars.next()?.to_ascii_uppercase();
    let rest: String = chars.collect();
    Some(format!("{first}{rest}Url"))
}

/// Whether a URL is safe to write into a file as metadata: an actual
/// `http(s)` link, not absurdly long, and free of control characters
/// that could do something unexpected wherever the tag later gets
/// displayed or parsed.
fn url_is_writable(url: &str) -> bool {
    (url.starts_with("http://") || url.starts_with("https://"))
        && url.len() <= 2048
        && !url.chars().any(|c| c.is_control())
}

/// One cached answer for a single source URL. Reused as both "what a
/// fresh lookup handed back" and "what's stored on disk" — the "when
/// did we last ask" and "what did we get" fields matter equally in
/// both places, so one struct does both jobs instead of two
/// near-identical ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdesliLookupResult {
    /// RFC 3339 timestamp (UTC) of when this answer was looked up.
    pub looked_up_at: String,
    /// Source URL that was passed to Odesli.
    pub source_url: String,
    /// Cross-platform URLs keyed by Odesli's platform identifier.
    /// Empty means "we asked, and there were no matches" — itself a
    /// cacheable answer, not the absence of one.
    pub urls: CrossPlatformUrls,
}

/// The whole on-disk cache: every album we've looked up before, keyed
/// by the (trimmed) source URL we asked song.link about.
#[derive(Debug, Default, Serialize, Deserialize)]
struct OdesliCacheFile {
    #[serde(default)]
    entries: BTreeMap<String, OdesliLookupResult>,
}

/// Reads the cache file from disk. A missing or unreadable file just
/// gives an empty cache, logged at debug and nothing more. There's no
/// checksum guard here the way there is for the feature-flags cache
/// — the worst a broken `odesli-cache.json` can do is cost one repeat
/// lookup, and it's never used to gate anything or trusted as an
/// instruction, so a corrupt file simply isn't worth that extra
/// machinery.
fn load_cache(path: &Path) -> OdesliCacheFile {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::debug!(
                "Odesli cache at {} could not be read, starting fresh: {e}",
                path.display()
            );
            OdesliCacheFile::default()
        }),
        Err(e) => {
            log::debug!(
                "Odesli cache at {} not found, starting fresh: {e}",
                path.display()
            );
            OdesliCacheFile::default()
        }
    }
}

/// Writes the cache file back to disk via the shared atomic-write
/// helper (write-to-temp-then-rename), so a crash mid-write can never
/// leave a half-written, unreadable cache behind. A failed write is
/// logged and otherwise ignored: losing one cache write just means
/// the next lookup for that album asks song.link again, a harmless
/// (if request-costing) fallback.
fn save_cache(path: &Path, file: &OdesliCacheFile) {
    if let Err(e) = crate::utils::atomic_write::atomic_write_json(path, file, "Odesli cache") {
        log::warn!("Failed to save Odesli cache: {e}");
    }
}

/// Looks up a cached answer, but only if it's still fresh — the
/// `looked_up_at` timestamp both has to parse *and* has to be within
/// [`CACHE_MAX_AGE_DAYS`] of `now`. An unparseable timestamp counts as
/// "not fresh" rather than "assume it's fine", since a broken
/// timestamp is itself a sign something is wrong with that entry.
fn cache_get<'a>(
    file: &'a OdesliCacheFile,
    url: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<&'a CrossPlatformUrls> {
    let entry = file.entries.get(url)?;
    let looked_up_at: chrono::DateTime<chrono::Utc> = entry.looked_up_at.parse().ok()?;
    if now - looked_up_at > chrono::Duration::days(CACHE_MAX_AGE_DAYS) {
        return None;
    }
    Some(&entry.urls)
}

/// Stores an answer, then — if that pushed the cache over its cap —
/// drops entries with the oldest `looked_up_at` until it's back under
/// the cap again. An entry whose timestamp can't be parsed is treated
/// as the oldest of all, so it's evicted before anything with a
/// readable date.
fn cache_put(
    file: &mut OdesliCacheFile,
    url: &str,
    urls: CrossPlatformUrls,
    now: chrono::DateTime<chrono::Utc>,
    max_entries: usize,
) {
    file.entries.insert(
        url.to_string(),
        OdesliLookupResult {
            looked_up_at: now.to_rfc3339(),
            source_url: url.to_string(),
            urls,
        },
    );

    while file.entries.len() > max_entries {
        let oldest_key = file
            .entries
            .iter()
            .min_by(|(_, a), (_, b)| {
                let a_ts = a.looked_up_at.parse::<chrono::DateTime<chrono::Utc>>();
                let b_ts = b.looked_up_at.parse::<chrono::DateTime<chrono::Utc>>();
                match (a_ts, b_ts) {
                    (Ok(a), Ok(b)) => a.cmp(&b),
                    (Err(_), Ok(_)) => std::cmp::Ordering::Less,
                    (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Err(_)) => std::cmp::Ordering::Equal,
                }
            })
            .map(|(k, _)| k.clone());
        let Some(key) = oldest_key else { break };
        file.entries.remove(&key);
    }
}

/// Waits until it's this request's turn, claims it, and hands back the
/// still-locked limiter so the caller keeps holding it.
///
/// The sleep happens while holding the lock, so a second album's lookup
/// cannot slip in and fire during the wait. That is what turns "every
/// album shares one clock" into "every album genuinely queues behind
/// whichever one is already waiting".
///
/// The caller then keeps holding it for the request itself. Only
/// [`paced_attempt`] calls this, and it never lets the lock escape, so
/// the turn's whole life is one short function you can read at a
/// glance — see that function for why that matters.
async fn claim_turn(gap: Duration) -> tokio::sync::MutexGuard<'static, Limiter> {
    let mut limiter = LIMITER.lock().await;
    if let Some(target) = next_allowed_at(limiter.last_request_at, limiter.blocked_until, gap) {
        let now = Instant::now();
        if target > now {
            tokio::time::sleep(target - now).await;
        }
    }
    let now = Instant::now();
    limiter.last_request_at = Some(now);
    if limiter.blocked_until.is_some_and(|b| b <= now) {
        limiter.blocked_until = None;
    }
    limiter
}

/// Asks song.link for the cross-platform URLs of one source URL,
/// paced and retried per the rules in the module doc above. `Ok(None)`
/// means "song.link has no matches for this" — a real, cacheable
/// answer, distinct from an `Err`, which means we couldn't get an
/// answer at all.
///
/// This always talks to the network — it does not consult the cache.
/// [`lookup`] is the cache-aware entry point most callers should use
/// instead; this stays public because `lookup` itself needs to call
/// it, and because a caller that genuinely wants to bypass the cache
/// (a hypothetical "check again now" action) should be able to.
/// What one paced attempt came back with.
///
/// `SlowDown` means song.link asked us to wait; by the time this is
/// returned the wait has already been written down, so the next
/// attempt honours it without anyone having to remember to.
enum AttemptOutcome {
    Links(CrossPlatformUrls),
    NoMatches,
    SlowDown,
    Failed(OdesliError),
}

/// Makes exactly one request to song.link, at the right moment, and
/// deals with whatever comes back.
///
/// This function owns the turn from start to finish, and that is the
/// whole reason it exists separately rather than being written inline
/// inside [`fetch_links`]. The turn is taken here and given up here,
/// so there is no way for a caller to let go of it early and leave a
/// gap. That gap was the original bug: an album told to slow down had
/// to queue up all over again just to write down how long to wait, and
/// by then every album already waiting had started — straight into the
/// limit that had just been hit, each burning its one retry for
/// nothing. Keeping the turn's whole life inside one short function
/// means that cannot come back by accident.
async fn paced_attempt(
    gap: Duration,
    source_url: &str,
    trimmed_key: Option<&str>,
    attempt: u8,
) -> AttemptOutcome {
    // Taking the turn. Everything below happens before we give it up.
    let mut limiter = claim_turn(gap).await;

    let mut url = format!(
        "{base}?url={encoded}",
        base = ODESLI_BASE_URL,
        encoded = urlencoded(source_url),
    );
    if let Some(key) = trimmed_key {
        url.push_str("&key=");
        url.push_str(&urlencoded(key));
    }

    log::debug!("song.link lookup (attempt {attempt}): {source_url}");
    let client = match crate::utils::http_client::build_simple(15) {
        Ok(c) => c,
        Err(e) => return AttemptOutcome::Failed(OdesliError::Network(e)),
    };
    let response = match client
        .get(&url)
        .header("User-Agent", crate::utils::http_client::browser_user_agent())
        .send()
        .await
    {
        Ok(r) => r,
        // Display, never Debug. Debug would print the whole request
        // address, and the access key is part of it.
        Err(e) => return AttemptOutcome::Failed(OdesliError::Network(format!("{e}"))),
    };

    let status = response.status();

    if status.as_u16() == 429 {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok());
        let delay = retry_after_delay(retry_after);
        // Written down while we still hold the turn, so nobody else can
        // start before they know about it.
        limiter.blocked_until = Some(Instant::now() + delay);
        log::debug!("song.link asked us to wait {delay:?} before trying {source_url} again");
        return AttemptOutcome::SlowDown;
    }

    if status.as_u16() == 401 {
        let body = response.text().await.unwrap_or_default();
        let error = classify_unauthorised(&body);
        match &error {
            OdesliError::PublicAccessClosed => {
                PUBLIC_ACCESS_CLOSED.store(true, Ordering::Relaxed);
            }
            OdesliError::KeyRejected => {
                if let Some(key) = trimmed_key {
                    *REJECTED_KEY.lock().await = Some(key.to_string());
                }
            }
            _ => {}
        }
        return AttemptOutcome::Failed(error);
    }

    if status.as_u16() == 404 {
        log::debug!("song.link: no matches for {source_url}");
        return AttemptOutcome::NoMatches;
    }

    if !status.is_success() {
        return AttemptOutcome::Failed(OdesliError::Http(status.as_u16()));
    }

    match response.json::<serde_json::Value>().await {
        Ok(json) => AttemptOutcome::Links(extract_links_by_platform(&json)),
        Err(e) => AttemptOutcome::Failed(OdesliError::Parse(format!("{e}"))),
    }
    // The turn is given up here, where this function ends.
}

pub async fn fetch_links(
    source_url: &str,
    api_key: Option<&str>,
) -> Result<Option<CrossPlatformUrls>, OdesliError> {
    let has_key = has_usable_key(api_key);
    let trimmed_key = if has_key {
        api_key.map(|k| k.trim().to_string())
    } else {
        None
    };

    // Both of these are things song.link has already told us this
    // session, so there is nothing to gain by asking again.
    if !has_key && PUBLIC_ACCESS_CLOSED.load(Ordering::Relaxed) {
        return Err(OdesliError::PublicAccessClosed);
    }
    if has_key {
        let rejected = REJECTED_KEY.lock().await;
        if rejected.as_deref() == trimmed_key.as_deref() {
            return Err(OdesliError::KeyRejected);
        }
    }

    let gap = request_gap(has_key);

    // The first go, and one more only if song.link asked us to slow
    // down. A second refusal is reported rather than fought.
    for attempt in 1..=MAX_ATTEMPTS {
        match paced_attempt(gap, source_url, trimmed_key.as_deref(), attempt).await {
            AttemptOutcome::Links(urls) => return Ok(Some(urls)),
            AttemptOutcome::NoMatches => return Ok(None),
            AttemptOutcome::Failed(e) => return Err(e),
            AttemptOutcome::SlowDown => continue,
        }
    }
    Err(OdesliError::RateLimited)
}

/// Cache-aware entry point — this is what the enrichment pipeline
/// should actually call, not [`fetch_links`] directly. Checks the
/// on-disk cache first and only reaches for the network on a miss,
/// which is the entire point of caching under a quota that has to be
/// applied for (see the "Remembering answers" section of the module
/// doc above).
///
/// `cache_path` is the full path to `odesli-cache.json`, typically
/// `{app_data_dir}/odesli-cache.json`. Passing `None` skips the cache
/// entirely — every call goes to the network — which is useful for
/// tests and for any future caller that deliberately wants a fresh
/// answer.
pub async fn lookup(
    source_url: &str,
    api_key: Option<&str>,
    cache_path: Option<&Path>,
) -> Result<Option<CrossPlatformUrls>, OdesliError> {
    let key = source_url.trim().to_string();

    let Some(path) = cache_path else {
        return fetch_links(&key, api_key).await;
    };

    let now = chrono::Utc::now();

    // Check the cache first. The lock is held only long enough to
    // read it — released at the end of this block, before any
    // network call happens below — so a slow song.link response can
    // never hold up every other album's cache lookups.
    {
        let mut guard = CACHE.lock().await;
        let file = guard.get_or_insert_with(|| load_cache(path));
        if let Some(cached) = cache_get(file, &key, now) {
            log::debug!("Odesli: cache hit for {key}");
            return Ok(if cached.is_empty() {
                None
            } else {
                Some(cached.clone())
            });
        }
    }

    let result = fetch_links(&key, api_key).await;

    if let Ok(urls) = &result {
        let mut guard = CACHE.lock().await;
        let file = guard.get_or_insert_with(|| load_cache(path));
        cache_put(file, &key, urls.clone().unwrap_or_default(), now, CACHE_MAX_ENTRIES);
        save_cache(path, file);
    }

    result
}

/// Writes every cross-platform URL Odesli found onto disk as freeform
/// atoms, one atom per service (`SpotifyUrl`, `YoutubeUrl`, …), on
/// every M4A file found under `album_dir`.
///
/// This never returns an error and can never fail a download —
/// writing this metadata is a nice extra, not something any download
/// depends on. A file we can't write to (locked, read-only, mid-write
/// by something else) is quietly skipped and logged at debug, never
/// surfaced to the user. An atom for a service we already had is
/// replaced with the new URL; an atom for a service that no longer
/// appears in `urls` (Odesli's answer changed between lookups) is
/// simply left as it was — this only ever adds or updates atoms,
/// never removes one.
///
/// `file_locks`, when supplied, serialises this write against any
/// other enrichment stage (AcoustID, ReplayGain, the tag registry
/// pass) touching the same file at the same time, the same way those
/// stages already coordinate with each other.
///
/// `should_stop` is checked before starting on each file, so a
/// cancelled or shutting-down download stops partway through the file
/// list instead of carrying on regardless.
///
/// Returns how many files were written to. This is purely
/// informational, for an activity-log line — callers should never
/// treat a low or zero count as an error condition.
pub async fn write_url_atoms(
    album_dir: &str,
    urls: &CrossPlatformUrls,
    file_locks: Option<&std::sync::Arc<crate::utils::file_locks::FileWriteLocks>>,
    should_stop: impl Fn() -> bool + Send,
) -> usize {
    let pairs: Vec<(String, String)> = urls
        .iter()
        .filter_map(|(platform, url)| {
            let atom_name = atom_name_for_platform(platform)?;
            url_is_writable(url).then(|| (atom_name, url.clone()))
        })
        .collect();

    if pairs.is_empty() {
        return 0;
    }

    let files = crate::services::metadata_tag_service::collect_m4a_files(album_dir);
    let mut written = 0usize;

    for file_path in files {
        if should_stop() {
            break;
        }

        let _write_guard = match file_locks {
            Some(locks) => Some(locks.lock(&file_path).await),
            None => None,
        };

        let pairs_for_task = pairs.clone();
        let path_for_task = file_path.clone();
        let outcome = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let mut tag = Tag::read_from_path(&path_for_task)
                .map_err(|e| format!("Failed to read M4A: {e}"))?;
            for (name, url) in &pairs_for_task {
                tag.set_data(
                    FreeformIdent::new_borrowed(
                        crate::services::metadata_tag_service::MEEDYADL_NAMESPACE,
                        name,
                    ),
                    Data::Utf8(url.clone()),
                );
            }
            tag.write_to_path(&path_for_task)
                .map_err(|e| format!("Failed to write M4A: {e}"))?;
            Ok(())
        })
        .await;

        match outcome {
            Ok(Ok(())) => written += 1,
            Ok(Err(e)) => log::debug!(
                "Odesli: could not write cross-platform URLs to {}: {e}",
                file_path.display()
            ),
            Err(e) => log::debug!(
                "Odesli: tag-write task panicked for {}: {e}",
                file_path.display()
            ),
        }
    }

    written
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

    #[tokio::test]
    async fn claiming_a_turn_keeps_the_lock_until_the_caller_lets_go() {
        // `claim_turn` hands the lock back rather than releasing it, so
        // that the request and whatever it comes back with all happen
        // before anybody else can take a turn. The bigger guarantee —
        // that nobody can let go of it early — is enforced by the
        // compiler, because the lock never leaves `paced_attempt`. This
        // test pins the smaller half: while the caller holds what it
        // was given, nobody else gets through.
        let guard = claim_turn(Duration::from_millis(0)).await;
        assert!(
            LIMITER.try_lock().is_err(),
            "another lookup was able to start while a turn was still in progress"
        );
        drop(guard);
        assert!(
            LIMITER.try_lock().is_ok(),
            "letting go of the turn should let the next lookup through"
        );
    }

    #[test]
    fn urlencoded_escapes_specials() {
        assert_eq!(urlencoded("https://music.apple.com/gb/album/foo/123"),
            "https%3A%2F%2Fmusic.apple.com%2Fgb%2Falbum%2Ffoo%2F123");
        assert_eq!(urlencoded("a b"), "a%20b");
        assert_eq!(urlencoded("a-b_c.d~e"), "a-b_c.d~e"); // unreserved
    }

    /// This is the test that fails against the old 1.1-second gap:
    /// that gap allowed roughly 54 requests a minute against a known
    /// limit of 10 — more than five times over. `request_gap(false)`
    /// must leave at most 10 requests fitting in any 60-second span.
    #[test]
    fn request_gap_without_key_allows_at_most_ten_a_minute() {
        assert!(60_000 / request_gap(false).as_millis() <= 10);
    }

    #[test]
    fn request_gap_with_key_is_shorter_but_at_least_a_second() {
        let with_key = request_gap(true);
        let without_key = request_gap(false);
        assert!(with_key < without_key);
        assert!(with_key >= Duration::from_secs(1));
    }

    #[test]
    fn has_usable_key_ignores_blank_and_whitespace() {
        assert!(!has_usable_key(None));
        assert!(!has_usable_key(Some("")));
        assert!(!has_usable_key(Some("   ")));
        assert!(has_usable_key(Some(" k ")));
    }

    #[test]
    fn retry_after_delay_parses_seconds_and_clamps() {
        assert_eq!(retry_after_delay(None), Duration::from_secs(60));
        assert_eq!(retry_after_delay(Some("5")), Duration::from_secs(5));
        assert_eq!(retry_after_delay(Some("999")), Duration::from_secs(60));
        assert_eq!(retry_after_delay(Some("0")), Duration::from_secs(1));
        assert_eq!(
            retry_after_delay(Some("Wed, 21 Oct 2026 07:28:00 GMT")),
            Duration::from_secs(60)
        );
    }

    #[tokio::test]
    async fn next_allowed_at_uses_the_later_of_gap_and_block() {
        // Neither set: no constraint at all.
        assert_eq!(next_allowed_at(None, None, Duration::from_secs(1)), None);

        let now = Instant::now();
        let gap = Duration::from_secs(5);

        // Only the gap is set.
        assert_eq!(next_allowed_at(Some(now), None, gap), Some(now + gap));

        // A block ending later than the gap wins.
        let later_block = now + Duration::from_secs(30);
        assert_eq!(
            next_allowed_at(Some(now), Some(later_block), gap),
            Some(later_block)
        );

        // A block ending before the gap does NOT win — the gap does.
        let earlier_block = now + Duration::from_millis(1);
        assert_eq!(
            next_allowed_at(Some(now), Some(earlier_block), gap),
            Some(now + gap)
        );
    }

    #[test]
    fn classify_unauthorised_recognises_both_song_link_codes() {
        assert_eq!(
            classify_unauthorised(r#"{"statusCode":401,"code":"PUBLIC_API_ACCESS_DEPRECATED"}"#),
            OdesliError::PublicAccessClosed
        );
        assert_eq!(
            classify_unauthorised(r#"{"statusCode":401,"code":"INVALID_ACCESS_KEY"}"#),
            OdesliError::KeyRejected
        );
        assert_eq!(classify_unauthorised("{}"), OdesliError::Http(401));
        assert_eq!(classify_unauthorised("not json"), OdesliError::Http(401));
    }

    #[test]
    fn atom_name_for_platform_capitalises_and_appends_url() {
        assert_eq!(atom_name_for_platform("spotify").as_deref(), Some("SpotifyUrl"));
        assert_eq!(
            atom_name_for_platform("youtubeMusic").as_deref(),
            Some("YoutubeMusicUrl")
        );
        assert_eq!(
            atom_name_for_platform("amazonMusic").as_deref(),
            Some("AmazonMusicUrl")
        );
        assert_eq!(
            atom_name_for_platform("appleMusic").as_deref(),
            Some("AppleMusicUrl")
        );
    }

    #[test]
    fn atom_name_for_platform_rejects_unsafe_keys() {
        assert_eq!(atom_name_for_platform(""), None);
        assert_eq!(atom_name_for_platform("bad-key"), None);
        assert_eq!(atom_name_for_platform("with space"), None);
        assert_eq!(atom_name_for_platform("ünïcode"), None);
        assert_eq!(atom_name_for_platform("a/b"), None);
    }

    #[test]
    fn url_is_writable_rejects_control_chars_length_and_non_http() {
        assert!(url_is_writable("https://open.spotify.com/track/abc"));
        assert!(!url_is_writable("ftp://example.com/track"));
        assert!(!url_is_writable("https://example.com/\u{7}bell"));
        let too_long = format!("https://example.com/{}", "a".repeat(2048));
        assert!(!url_is_writable(&too_long));
    }

    #[test]
    fn cache_get_returns_only_fresh_entries() {
        let now = chrono::Utc::now();
        let mut file = OdesliCacheFile::default();
        file.entries.insert(
            "fresh".to_string(),
            OdesliLookupResult {
                looked_up_at: now.to_rfc3339(),
                source_url: "fresh".to_string(),
                urls: CrossPlatformUrls::new(),
            },
        );
        file.entries.insert(
            "stale".to_string(),
            OdesliLookupResult {
                looked_up_at: (now - chrono::Duration::days(31)).to_rfc3339(),
                source_url: "stale".to_string(),
                urls: CrossPlatformUrls::new(),
            },
        );
        file.entries.insert(
            "broken".to_string(),
            OdesliLookupResult {
                looked_up_at: "not a timestamp".to_string(),
                source_url: "broken".to_string(),
                urls: CrossPlatformUrls::new(),
            },
        );

        assert!(cache_get(&file, "fresh", now).is_some());
        assert!(cache_get(&file, "stale", now).is_none());
        assert!(cache_get(&file, "broken", now).is_none());
        assert!(cache_get(&file, "missing", now).is_none());
    }

    #[test]
    fn cache_put_evicts_oldest_beyond_cap() {
        let now = chrono::Utc::now();
        let mut file = OdesliCacheFile::default();
        cache_put(
            &mut file,
            "a",
            CrossPlatformUrls::new(),
            now - chrono::Duration::minutes(2),
            2,
        );
        cache_put(
            &mut file,
            "b",
            CrossPlatformUrls::new(),
            now - chrono::Duration::minutes(1),
            2,
        );
        assert_eq!(file.entries.len(), 2);

        cache_put(&mut file, "c", CrossPlatformUrls::new(), now, 2);
        assert_eq!(file.entries.len(), 2);
        assert!(!file.entries.contains_key("a"));
        assert!(file.entries.contains_key("b"));
        assert!(file.entries.contains_key("c"));
    }

    #[test]
    fn cache_round_trips_through_atomic_write() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        let now = chrono::Utc::now();
        let mut file = OdesliCacheFile::default();
        let mut urls = CrossPlatformUrls::new();
        urls.insert(
            "spotify".to_string(),
            "https://open.spotify.com/track/abc".to_string(),
        );
        cache_put(
            &mut file,
            "https://music.apple.com/us/album/x/1",
            urls,
            now,
            CACHE_MAX_ENTRIES,
        );

        save_cache(&path, &file);
        let loaded = load_cache(&path);

        assert_eq!(loaded.entries.len(), 1);
        let entry = loaded
            .entries
            .get("https://music.apple.com/us/album/x/1")
            .unwrap();
        assert_eq!(
            entry.urls.get("spotify").unwrap(),
            "https://open.spotify.com/track/abc"
        );
    }
}
