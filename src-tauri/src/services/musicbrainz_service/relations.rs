// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MusicBrainz relation parsing — the single home of the 2026-11-30
// search-upgrade shape rules (SEARCH-444/751/752/753). Part of the
// search-upgrade readiness work tracked in #1120.
//
// MusicBrainz's Solr 9 -> 10 upgrade on 2026-11-30 changes only the
// INDEXED SEARCH (`?query=`) response shape; lookup (`/ws/2/<entity>/<mbid>`,
// `/ws/2/isrc/<isrc>`) and browse (`/ws/2/url?resource=`) responses are
// untouched by the upgrade and already emit the stable, post-upgrade shape
// today (flat `relations[]`, explicit `target-type`, no scalar `target`).
// This module exists so every caller — lookup, browse, and (once wired)
// the guarded search tier's own relation-shaped fixtures — parses
// relations through ONE shape-tolerant path rather than each call site
// re-deriving its own nesting/target-type/scalar-target assumptions.
//
// Per the governing contract (see `mod.rs`'s module doc-comment): search
// responses are consumed ONLY as identifier discovery, never as a source
// of relations. Everything in this file is a relation parser for
// lookup/browse payloads; it must never be pointed at a raw search
// response body for anything beyond a shape-sanity test fixture.
//
// `classify_url` and `parse_recording_relations` moved here verbatim (the
// latter rewritten on top of `collect_relations`, plus the §7.1/§7.2/§7.4
// hardening fixes from the readiness audit) from `mod.rs` in this
// tranche. `mod.rs` re-exports both so existing call sites — its own
// production code and its own `mod tests` — keep working unqualified.

use super::*;

/// Closed set of relation-target entity keys this module knows how to
/// sniff when `target-type` is absent (SEARCH-751/753 — pre-upgrade
/// search output, per live probe B7/B8, carries no `target-type` at
/// all). Checked in this exact order; the first key present AND holding
/// a JSON object wins. Deliberately closed rather than "any unknown
/// object key" — an unrecognised entity kind should sniff to `""`
/// (unresolved) rather than silently guessing.
const ENTITY_KEYS: &[&str] = &[
    "url",
    "recording",
    "release",
    "release-group",
    "release_group",
    "artist",
    "work",
    "area",
    "label",
    "place",
    "event",
    "series",
    "instrument",
    "genre",
];

/// A single MusicBrainz relation, viewed through the Nov-30-safe lens:
/// resolved `target-type`, the entity object it names (when present),
/// and the legacy scalar `target` value kept ONLY as raw material for
/// the two guarded accessors below — never read directly by a caller.
///
/// **INVARIANT: no code outside this struct's own methods may read
/// `raw["target"]` directly anywhere in this crate.** The scalar is
/// exactly what SEARCH-752 removes from search output going forward;
/// treating it as trustworthy without the http-shape guard
/// (`url_resource()`) or the UUID-shape guard (`target_mbid()`) is how a
/// truncated, garbled, or wrong-typed value turns into a silently wrong
/// lookup instead of a rejected one. See
/// `relation_view_post_upgrade_shape_needs_no_scalar_target` and
/// `relation_view_target_mbid_requires_uuid_shape` below.
#[derive(Debug)]
pub(super) struct RelationView<'a> {
    /// Relation "type" (e.g. "free streaming", "music video", "mashes
    /// up"); `""` when the field is absent or not a string.
    pub rel_type: &'a str,
    /// Explicit `target-type` when present; otherwise the sniffed
    /// entity key (see [`ENTITY_KEYS`]); `""` when neither resolves.
    pub target_type: &'a str,
    /// The entity object named by `target_type`, when one was found.
    pub entity: Option<&'a serde_json::Value>,
    /// Scalar `target` (SEARCH-752 pre-shape), if present. NEVER
    /// consumed directly outside this struct's own accessor methods.
    pub legacy_target: Option<&'a str>,
    /// The full, unmodified relation object — for `is_ended()` and any
    /// future field a caller needs that isn't worth its own accessor.
    pub raw: &'a serde_json::Value,
}

impl<'a> RelationView<'a> {
    /// Build a view from one raw relation object. `target-type` wins
    /// when present (and non-empty — pre-upgrade search output either
    /// omits the key entirely or, per probe evidence, never emits an
    /// empty string, so treating an empty value as "absent" costs
    /// nothing and is the safer read); otherwise sniff the entity key
    /// via [`ENTITY_KEYS`].
    fn from_raw(raw: &'a serde_json::Value) -> Self {
        let rel_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let explicit_target_type = raw
            .get("target-type")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let (target_type, entity) = match explicit_target_type {
            Some(explicit) => (explicit, raw.get(explicit)),
            None => {
                let mut resolved: (&'a str, Option<&'a serde_json::Value>) = ("", None);
                for &key in ENTITY_KEYS {
                    if let Some(obj) = raw.get(key) {
                        if obj.is_object() {
                            resolved = (key, Some(obj));
                            break;
                        }
                    }
                }
                resolved
            }
        };

        let legacy_target = raw.get("target").and_then(|v| v.as_str());

        RelationView {
            rel_type,
            target_type,
            entity,
            legacy_target,
            raw,
        }
    }

    /// The URL this relation points at, resolved through the guarded
    /// entity-first / legacy-scalar-second chain (SEARCH-752): the
    /// canonical `url.resource` field on the entity object when
    /// present, else the scalar `target` — but ONLY when it is
    /// http(s)-shaped. A recording/area/artist/etc-typed relation's
    /// scalar `target` is that entity's MBID, not a URL, and must never
    /// be misread as one.
    pub fn url_resource(&self) -> Option<&'a str> {
        if let Some(resource) = self
            .entity
            .and_then(|e| e.get("resource"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(resource);
        }
        self.legacy_target
            .filter(|t| t.starts_with("http://") || t.starts_with("https://"))
    }

    /// The MBID this relation points at, resolved through the same
    /// guarded chain: the entity object's own `id` field when present,
    /// else the scalar `target` — but ONLY when it is UUID-shaped (36
    /// bytes, ASCII hex with `-` at offsets 8/13/18/23). Closes audit
    /// defect §7.11's any-string tolerance, which trusted ANY
    /// non-URL-shaped scalar as an MBID — a truncated or garbled value
    /// would have silently produced a bogus recording lookup instead of
    /// being rejected outright.
    pub fn target_mbid(&self) -> Option<&'a str> {
        if let Some(id) = self
            .entity
            .and_then(|e| e.get("id"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(id);
        }
        self.legacy_target.filter(|t| is_uuid_shaped(t))
    }

    /// The entity object's own `title` (recording/release/…) or `name`
    /// (artist/area/…) field, when present. Used for log context and
    /// for stamping a discovered video's display title.
    pub fn entity_title(&self) -> Option<&'a str> {
        let entity = self.entity?;
        entity
            .get("title")
            .and_then(|v| v.as_str())
            .or_else(|| entity.get("name").and_then(|v| v.as_str()))
            .filter(|s| !s.is_empty())
    }

    /// `true` when this relation's `ended` flag is explicitly `true`.
    /// Absent/non-boolean `ended` is treated as "still active" (`false`)
    /// — the conservative default for a field most relations simply
    /// omit when it doesn't apply.
    pub fn is_ended(&self) -> bool {
        self.raw
            .get("ended")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// True when `s` is shaped like a MusicBrainz MBID: exactly 36 bytes,
/// ASCII hex digits everywhere except `-` at byte offsets 8/13/18/23
/// (the canonical RFC 4122 UUID textual form). Byte-oriented rather
/// than char-oriented — a UUID is pure ASCII by definition, so counting
/// bytes and offsets is both correct and avoids a multi-byte-char
/// miscount on adversarial input.
fn is_uuid_shaped(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Collect every relation on `container`, tolerating both the flat
/// post-upgrade shape and the nested pre-upgrade one (SEARCH-444).
///
/// Precedence (exact): if `container["relations"]` is a JSON array,
/// iterate it — full stop. Only when that key is absent (or not an
/// array) do we fall back to `container["relation-list"]`, flattening
/// every element's own `relations` array. If BOTH keys happen to be
/// present on the same object (conceivable during the SolrCloud
/// staged-rollout window, where consecutive requests can land on
/// differently-versioned nodes), the flat `relations` key wins outright
/// and the nested wrapper is silently ignored — NOT merged, NOT
/// appended (pinned by `collect_relations_flat_wins_when_both_shapes_present`,
/// per critique finding m5).
///
/// An object with neither key returns an empty `Vec` — this is never an
/// error. Whether "no relations" is a legitimate miss or a sign of a
/// shape the caller didn't expect is the CALLER's call, not this
/// function's; `parse_recording_relations` below treats it as a
/// legitimate miss, exactly as the pre-split code did.
///
/// Returns an owned `Vec` rather than a lazy iterator: MusicBrainz
/// relation arrays run to at most a few dozen entries, and a concrete
/// `Vec` keeps the two-shape flattening trivially testable in isolation
/// from any consumer.
pub(super) fn collect_relations(container: &serde_json::Value) -> Vec<RelationView<'_>> {
    let raw_relations: Vec<&serde_json::Value> =
        if let Some(flat) = container.get("relations").and_then(|r| r.as_array()) {
            flat.iter().collect()
        } else if let Some(nested) = container.get("relation-list").and_then(|r| r.as_array()) {
            nested
                .iter()
                .filter_map(|entry| entry.get("relations").and_then(|r| r.as_array()))
                .flat_map(|arr| arr.iter())
                .collect()
        } else {
            Vec::new()
        };

    raw_relations
        .into_iter()
        .map(RelationView::from_raw)
        .collect()
}

// ============================================================
// URL classification (moved verbatim from mod.rs)
// ============================================================

/// Classify a URL by its platform based on the domain.
///
/// Returns `Some((platform_id, url))` for recognized platforms,
/// or `None` for unrecognized URLs.
///
/// Deferred cleanup (tracked in this work's GitHub Issue, not addressed
/// here): substring matching on the whole URL rather than a proper
/// host-parse, and a redundant `open.spotify.com` arm now that
/// `.contains("spotify.com")` already covers it.
pub(super) fn classify_url(url: &str) -> Option<(&'static str, &str)> {
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
// Relationship parsing (rewritten on top of collect_relations)
// ============================================================

/// Parse URL and recording relationships from a MusicBrainz recording
/// JSON. Extracts external platform URLs and music video URLs from the
/// relations array — shape-tolerant via [`collect_relations`], so this
/// is the ONE relation parser shared by ISRC lookup, MBID lookup, and
/// (via `collect_relations`'s own nesting tolerance) any future browse
/// or lookup caller.
///
/// Behaviour-compatible with the pre-split direct-array parser for
/// every existing fixture, plus three hardening fixes from the
/// readiness audit:
///
/// - **§7.2**: video detection now keys on the relationship TYPE first
///   — a url-rel whose `type` is literally `"music video"` is a video
///   regardless of what its URL looks like. The old substring check
///   (`music-video` / `/video/` in the URL) is kept as a fallback for
///   relationships whose type isn't that exact string.
/// - **§7.4**: an `ended` url-rel is a dead/delisted link. It's still
///   recorded in `external_urls` (a stale link is still useful
///   diagnostic context), but it must never seed a `video_urls` entry
///   the companion downloader would then try to fetch.
/// - **§7.1 (half-fix)**: the recording–recording branch's comparison
///   string changes from `"performance"` — a WORK–recording
///   relationship type, which could never have matched a
///   recording–recording relation — to `"music video"`, MusicBrainz's
///   actual recording–recording relationship for linking a song to its
///   video recording. Still log-only: following the hop to the video
///   recording's own url-rels costs an extra request per linked video
///   and is deliberately deferred (tracked in this work's Issue).
pub(super) fn parse_recording_relations(
    json: &serde_json::Value,
) -> (
    std::collections::HashMap<String, String>,
    Vec<MusicVideoUrl>,
) {
    let mut external_urls = std::collections::HashMap::new();
    let mut video_urls = Vec::new();

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    for rel in collect_relations(json) {
        // URL relationships — streaming/download links. `url_resource()`
        // is the ONLY place the legacy scalar `target` is read (guarded,
        // http-shape-checked) — see the struct-level invariant.
        if rel.target_type == "url" {
            let Some(resource_url) = rel.url_resource() else {
                continue;
            };

            if let Some((platform, clean_url)) = classify_url(resource_url) {
                // §7.4: recorded regardless of `ended` — a stale link is
                // still worth surfacing as an external URL.
                external_urls.insert(platform.to_string(), clean_url.to_string());

                if !rel.is_ended() {
                    // §7.2: the relationship TYPE is the authoritative
                    // video signal; the URL-substring check is a
                    // fallback for relationships not literally typed
                    // "music video".
                    let is_video = rel.rel_type == "music video"
                        || resource_url.contains("music-video")
                        || resource_url.contains("/video/");

                    if is_video {
                        video_urls.push(MusicVideoUrl {
                            platform: platform.to_string(),
                            url: clean_url.to_string(),
                            title: Some(title.clone()),
                        });
                    }
                }
            }
        }

        // Recording-recording relationships — linked performances.
        // §7.1: "music video" (not "performance", which is a
        // WORK–recording type and could never fire here) is the actual
        // recording–recording relationship MusicBrainz uses to link a
        // song recording to its video recording.
        if rel.target_type == "recording" && rel.rel_type == "music video" {
            if let Some(video_title) = rel.entity_title() {
                match rel.target_mbid() {
                    Some(video_mbid) => log::debug!(
                        "MusicBrainz: found linked video recording {video_mbid} — {video_title}"
                    ),
                    None => {
                        log::debug!("MusicBrainz: found linked video recording — {video_title}")
                    }
                }
            }
        }
    }

    (external_urls, video_urls)
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Fixtures — real MusicBrainz captures, 2026-09-01
    //
    // A1/A2/A3 fixtures below are trimmed subsets of the full live
    // captures in `{MB}/prod-A1-isrc.json` / `prod-A2-recording.json` /
    // `prod-A3-url-youtube.json` (repo-external probe-report directory;
    // not read at test time) — every id/title/URL value kept is copied
    // byte-for-byte from the capture. B7/B8 are the FULL real capture
    // bodies. None of these are post-upgrade synthesised fixtures; all
    // are real pre-upgrade lookup/browse/search responses.
    // ----------------------------------------------------------

    /// The single real url-rel from `{MB}/prod-A2-recording.json`
    /// (`GET /ws/2/recording/{mbid}`), isolated to its own container so
    /// this test can assert an exact single-element result.
    fn a2_single_url_relation_fixture() -> serde_json::Value {
        serde_json::json!({
            "title": "Yellow Submarine",
            "relations": [
                {
                    "type": "free streaming",
                    "type-id": "7e41ef12-a124-4324-afdb-fdbae687a89c",
                    "target-type": "url",
                    "ended": false,
                    "direction": "forward",
                    "url": {
                        "id": "63612e24-efed-4db5-81de-9bb1c768a715",
                        "resource": "https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT"
                    }
                }
            ]
        })
    }

    /// A representative subset (3 of the real 11 recording-recording
    /// relations, plus the 1 real url-rel) of `{MB}/prod-A2-recording.json`.
    fn a2_recording_lookup_fixture() -> serde_json::Value {
        serde_json::json!({
            "title": "Yellow Submarine",
            "id": "b2181aae-5cba-496c-bb0c-b4cc0109ebf8",
            "relations": [
                {
                    "type": "mashes up",
                    "type-id": "579d0b4c-bf77-479d-aa59-a8af1f518958",
                    "target-type": "recording",
                    "ended": false,
                    "direction": "backward",
                    "recording": {
                        "id": "60f68854-d44c-4c6e-9e23-000103b1669d",
                        "title": "Lovely NYC",
                        "video": false,
                        "length": 192000
                    }
                },
                {
                    "type": "remix",
                    "type-id": "bfbdb55a-b857-473a-8f2e-a9c09e45c3f5",
                    "target-type": "recording",
                    "ended": false,
                    "direction": "backward",
                    "recording": {
                        "id": "8417ac27-57b8-4160-b07b-32772bd897d1",
                        "title": "Yellow Submarine",
                        "disambiguation": "1999 remix",
                        "video": false,
                        "length": 159160
                    }
                },
                {
                    "type": "samples material",
                    "type-id": "9efd9ce9-e702-448b-8e76-641515e8fe62",
                    "target-type": "recording",
                    "ended": false,
                    "direction": "backward",
                    "recording": {
                        "id": "488b093f-d204-4600-80fc-d0bc5bc64506",
                        "title": "Octopus’s Garden",
                        "disambiguation": "Love version",
                        "video": false,
                        "length": 199000
                    }
                },
                {
                    "type": "free streaming",
                    "type-id": "7e41ef12-a124-4324-afdb-fdbae687a89c",
                    "target-type": "url",
                    "ended": false,
                    "direction": "forward",
                    "url": {
                        "id": "63612e24-efed-4db5-81de-9bb1c768a715",
                        "resource": "https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT"
                    }
                }
            ]
        })
    }

    /// A representative subset (2 of the real 9 recording-recording
    /// relations, plus the 1 real url-rel) of the single recording in
    /// `{MB}/prod-A1-isrc.json` (`GET /ws/2/isrc/{isrc}`).
    fn a1_isrc_first_recording_fixture() -> serde_json::Value {
        serde_json::json!({
            "title": "Yellow Submarine",
            "id": "b2181aae-5cba-496c-bb0c-b4cc0109ebf8",
            "relations": [
                {
                    "type": "mashes up",
                    "type-id": "579d0b4c-bf77-479d-aa59-a8af1f518958",
                    "target-type": "recording",
                    "ended": false,
                    "recording": {
                        "id": "60f68854-d44c-4c6e-9e23-000103b1669d",
                        "title": "Lovely NYC"
                    }
                },
                {
                    "type": "remix",
                    "type-id": "bfbdb55a-b857-473a-8f2e-a9c09e45c3f5",
                    "target-type": "recording",
                    "ended": false,
                    "recording": {
                        "id": "8417ac27-57b8-4160-b07b-32772bd897d1",
                        "title": "Yellow Submarine",
                        "disambiguation": "1999 remix"
                    }
                },
                {
                    "type": "free streaming",
                    "type-id": "7e41ef12-a124-4324-afdb-fdbae687a89c",
                    "target-type": "url",
                    "ended": false,
                    "url": {
                        "id": "63612e24-efed-4db5-81de-9bb1c768a715",
                        "resource": "https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT"
                    }
                }
            ]
        })
    }

    /// Full real capture, `{MB}/prod-A3-url-youtube.json`
    /// (`GET /ws/2/url?resource=` — browse by the Rickroll YouTube URL).
    /// 3 recording relations, two of them `video: true`.
    fn a3_url_browse_youtube_fixture() -> serde_json::Value {
        serde_json::json!({
            "resource": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "id": "29566add-95ae-45e8-9bbd-a77fbd14094f",
            "relations": [
                {
                    "target-type": "recording",
                    "type": "free streaming",
                    "ended": false,
                    "recording": {
                        "id": "8f3471b5-7e6a-48da-86a9-c1c07a0f47ae",
                        "title": "Never Gonna Give You Up",
                        "video": false,
                        "length": 212946
                    }
                },
                {
                    "target-type": "recording",
                    "type": "free streaming",
                    "ended": false,
                    "recording": {
                        "id": "cd29e7db-0e71-43e3-95f1-aa9cf4a2de32",
                        "title": "Never Gonna Give You Up",
                        "video": true,
                        "length": 207000
                    }
                },
                {
                    "target-type": "recording",
                    "type": "free streaming",
                    "ended": false,
                    "recording": {
                        "id": "ff6c55fc-6fd1-47d9-a623-dff39181e7c8",
                        "title": "Never Gonna Give You Up",
                        "disambiguation": "Official Music Video",
                        "video": true,
                        "length": 213000
                    }
                }
            ]
        })
    }

    /// Full real capture, `{MB}/prod-B7-search-url.json` — pre-Nov-30
    /// url SEARCH shape: relations nested under
    /// `relation-list[].relations[]` (SEARCH-444), and (SEARCH-751/753)
    /// no `target-type` at all — only the `release` entity key
    /// identifies what's being linked.
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

    /// Full real capture (relation-bearing portion; unrelated
    /// `aliases`/`life-span` sub-fields trimmed), `{MB}/prod-B8-search-area.json`
    /// — all three pre-upgrade symptoms in one relation: nested
    /// `relation-list` (SEARCH-444), no `target-type` (SEARCH-751/753),
    /// and a scalar `target` MBID alongside the full `area` entity
    /// object (SEARCH-752).
    fn b8_search_area_fixture() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-09-01T20:21:02.185Z",
            "count": 1,
            "offset": 0,
            "areas": [
                {
                    "id": "c2839e36-3a65-4c80-96ea-7b6e4e73dae3",
                    "type": "City",
                    "name": "Ho Chi Minh",
                    "score": 100,
                    "relation-list": [
                        {
                            "relations": [
                                {
                                    "type": "part of",
                                    "type-id": "de7cc874-8b1b-3a05-8272-f3834c968fb7",
                                    "target": "0158e991-c3c6-374a-9b9d-024bbaff6980",
                                    "direction": "backward",
                                    "area": {
                                        "id": "0158e991-c3c6-374a-9b9d-024bbaff6980",
                                        "type": "Country",
                                        "name": "Vietnam"
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
    // collect_relations — nesting tolerance + target-type sniffing
    // ----------------------------------------------------------

    #[test]
    fn collect_relations_flat_shape_lookup() {
        let fixture = a2_single_url_relation_fixture();
        let views = collect_relations(&fixture);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].target_type, "url");
        assert_eq!(
            views[0].url_resource(),
            Some("https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT")
        );
    }

    #[test]
    fn collect_relations_nested_relation_list_url_search() {
        let fixture = b7_search_url_fixture();
        let url_hit = &fixture["urls"][0];
        let views = collect_relations(url_hit);
        assert_eq!(views.len(), 2);
        assert!(views.iter().all(|v| v.target_type == "release"));
        assert_eq!(
            views[0].entity_title(),
            Some("With Skin Like Silverfish (demo)")
        );
        assert_eq!(views[1].entity_title(), Some("Never Gonna Give You Up"));
    }

    #[test]
    fn collect_relations_area_search_all_three_pre_symptoms() {
        let fixture = b8_search_area_fixture();
        let area_hit = &fixture["areas"][0];
        let views = collect_relations(area_hit);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].target_type, "area");
        // A scalar `target` AND the full `area` entity are both present
        // on this real relation — the entity object must win.
        assert_eq!(
            views[0].legacy_target,
            Some("0158e991-c3c6-374a-9b9d-024bbaff6980")
        );
        assert_eq!(
            views[0].target_mbid(),
            Some("0158e991-c3c6-374a-9b9d-024bbaff6980")
        );
    }

    #[test]
    fn collect_relations_flat_shape_browse_a3_youtube() {
        let fixture = a3_url_browse_youtube_fixture();
        let views = collect_relations(&fixture);
        assert_eq!(views.len(), 3);
        assert!(views.iter().all(|v| v.target_type == "recording"));
        assert_eq!(
            views[2].target_mbid(),
            Some("ff6c55fc-6fd1-47d9-a623-dff39181e7c8")
        );
        assert_eq!(views[2].entity_title(), Some("Never Gonna Give You Up"));
    }

    #[test]
    fn collect_relations_no_relations_key_yields_empty() {
        let json = serde_json::json!({"id": "x", "title": "No relations at all"});
        assert!(collect_relations(&json).is_empty());
    }

    #[test]
    fn collect_relations_flat_wins_when_both_shapes_present() {
        let json = serde_json::json!({
            "relations": [
                { "type": "free streaming", "target-type": "url",
                  "url": { "resource": "https://open.spotify.com/track/flat" } }
            ],
            "relation-list": [
                { "relations": [
                    { "type": "free streaming", "target-type": "url",
                      "url": { "resource": "https://open.spotify.com/track/nested-1" } },
                    { "type": "free streaming", "target-type": "url",
                      "url": { "resource": "https://open.spotify.com/track/nested-2" } }
                ]}
            ]
        });

        let views = collect_relations(&json);
        // m5: when BOTH keys are present, the flat `relations` array
        // wins outright and the nested `relation-list` wrapper is
        // ignored — not merged, not appended.
        assert_eq!(views.len(), 1);
        assert_eq!(
            views[0].url_resource(),
            Some("https://open.spotify.com/track/flat")
        );
    }

    // ----------------------------------------------------------
    // RelationView accessors — guarded legacy-scalar tolerance
    // ----------------------------------------------------------

    #[test]
    fn relation_view_url_resource_prefers_entity_over_legacy_target() {
        let json = serde_json::json!({
            "relations": [
                {
                    "type": "free streaming",
                    "target-type": "url",
                    "url": { "resource": "https://tidal.com/browse/track/real" },
                    "target": "https://open.spotify.com/track/stale-legacy-value"
                }
            ]
        });
        let views = collect_relations(&json);
        assert_eq!(
            views[0].url_resource(),
            Some("https://tidal.com/browse/track/real")
        );
    }

    #[test]
    fn relation_view_url_resource_accepts_http_shaped_legacy_target_only() {
        let json = serde_json::json!({
            "relations": [
                { "type": "free streaming", "target-type": "url",
                  "target": "https://example.com/http-shaped" },
                { "type": "free streaming", "target-type": "url",
                  "target": "not-a-url-at-all" }
            ]
        });
        let views = collect_relations(&json);
        assert_eq!(
            views[0].url_resource(),
            Some("https://example.com/http-shaped")
        );
        assert_eq!(views[1].url_resource(), None);
    }

    #[test]
    fn relation_view_post_upgrade_shape_needs_no_scalar_target() {
        // The both-era guarantee: lookup/browse responses already carry
        // zero scalar `target` values today (SEARCH-752's removal
        // target was never present here to begin with) — every
        // RelationView's legacy_target is None, and full extraction
        // still succeeds via the entity-object path alone.
        let fixture = a2_recording_lookup_fixture();
        let views = collect_relations(&fixture);
        assert_eq!(views.len(), 4);
        assert!(views.iter().all(|v| v.legacy_target.is_none()));

        let url_view = views.iter().find(|v| v.target_type == "url").unwrap();
        assert_eq!(
            url_view.url_resource(),
            Some("https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT")
        );

        let recording_views: Vec<_> = views
            .iter()
            .filter(|v| v.target_type == "recording")
            .collect();
        assert_eq!(recording_views.len(), 3);
        assert!(recording_views.iter().all(|v| v.target_mbid().is_some()));
    }

    #[test]
    fn relation_view_target_mbid_requires_uuid_shape() {
        let json = serde_json::json!({
            "relations": [
                { "type": "x", "target-type": "recording", "target": "not-a-uuid" },
                { "type": "x", "target-type": "recording",
                  "target": "5aa053a9-5b84-418f-bb3c-d61df67b3880" }
            ]
        });
        let views = collect_relations(&json);
        assert_eq!(views[0].target_mbid(), None);
        assert_eq!(
            views[1].target_mbid(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880")
        );
    }

    // ----------------------------------------------------------
    // parse_recording_relations — regression pin + hardening fixes
    // ----------------------------------------------------------

    #[test]
    fn parse_recording_relations_merges_isrc_a1_fixture() {
        // Regression pin: the rewrite on top of collect_relations must
        // produce the same output the pre-rewrite direct-array parser
        // did for this real recording — one Spotify external URL, and
        // (since "free streaming" isn't "music video" and the URL
        // contains neither "music-video" nor "/video/") zero videos.
        let (urls, videos) = parse_recording_relations(&a1_isrc_first_recording_fixture());
        assert_eq!(urls.len(), 1);
        assert_eq!(
            urls.get("spotify"),
            Some(&"https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT".to_string())
        );
        assert!(videos.is_empty());
    }

    #[test]
    fn parse_recording_relations_video_by_rel_type_music_video() {
        let json = serde_json::json!({
            "title": "Some Song",
            "relations": [
                {
                    "type": "music video",
                    "target-type": "url",
                    "url": { "resource": "https://www.youtube.com/watch?v=abcdefghijk" }
                }
            ]
        });
        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, "youtube");
    }

    #[test]
    fn parse_recording_relations_skips_ended_url_rel_for_videos() {
        let json = serde_json::json!({
            "title": "Some Song",
            "relations": [
                {
                    "type": "music video",
                    "target-type": "url",
                    "ended": true,
                    "url": { "resource": "https://www.youtube.com/watch?v=deadlinknow0" }
                }
            ]
        });
        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains_key("youtube"));
        assert!(videos.is_empty());
    }
}
