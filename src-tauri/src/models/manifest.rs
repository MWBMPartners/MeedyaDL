// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download manifest model (.meedyadl files embedded in album folders).
//
// Each downloaded album/playlist folder contains a `manifest.meedyadl`
// JSON file recording the source URL(s) and per-track metadata. This
// enables one-click re-download by importing the file into MeedyaDL.
// (Previously named `.meedyadl` — a hidden dotfile; renamed in #447.)
//
// The `sources` array supports multi-platform entries — as MeedyaDL
// expands to Spotify, YouTube, etc., new sources are appended to the
// existing file without overwriting previous platform data.
//
// Schema version: 1
// MIME type: application/x-meedyadl+json

use serde::{Deserialize, Serialize};

/// Top-level manifest file structure.
///
/// Written to `.meedyadl` in each album/playlist output directory
/// after enrichment completes. Readable by the import handler to
/// re-queue downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Schema version (currently 1). Incremented on breaking changes.
    pub version: u32,
    /// Application identifier.
    pub app: String,
    /// ISO 8601 timestamp when the manifest was first created.
    pub created_at: String,
    /// ISO 8601 timestamp of the most recent update (source added/modified).
    pub updated_at: String,
    /// One entry per platform the content was downloaded from.
    /// Appended to (not replaced) when re-downloading from a new platform.
    pub sources: Vec<ManifestSource>,
}

/// A single platform source entry within the manifest.
///
/// Records the platform, album/playlist URL, codec used, and per-track
/// metadata from the download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSource {
    /// Platform identifier: "apple-music", "spotify", "youtube", etc.
    pub platform: String,
    /// The album/playlist URL that was downloaded.
    pub url: String,
    /// Platform-specific storefront/region (e.g., "gb", "us").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storefront: Option<String>,
    /// ISO 8601 timestamp when this source was downloaded.
    pub downloaded_at: String,
    /// Primary audio codec used for this download (e.g., "alac", "atmos").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    /// Apple Music API `lastModifiedDate` at the time of download.
    /// Used for smart re-download detection (#263) — comparing this against
    /// a fresh API response reveals if the album has changed.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_modified_date: Option<String>,
    /// Companion-codec plan captured at download time (#766, Phase 2 of #717/5b).
    ///
    /// Each inner `Vec<String>` is one tier's `codecs_to_try` list expressed
    /// as canonical codec-registry IDs (e.g. `"eac3-atmos"`, `"alac"`,
    /// `"aac-hq"`, `"aac-mq"`). Tier 0 carries the primary download's codec
    /// fallback chain; subsequent entries mirror `plan_companions()` output.
    /// A tier is considered "landed" when at least one of its codecs has a
    /// matching file on disk (matched by filename suffix); a missing tier is
    /// what the smart-retry planner reports as a `PartialCodecs` gap.
    ///
    /// `None` on manifests written before this field existed — the planner
    /// falls back to its track-number-only diff in that case.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub companion_tiers: Option<Vec<Vec<String>>>,
    /// Per-track metadata from the download.
    #[serde(default)]
    pub tracks: Vec<ManifestTrack>,

    /// Per-enrichment-stage completion record (#759). `None` for
    /// manifests written before this field existed; treat absent /
    /// `None` as "stage status unknown — assume incomplete and let
    /// the file-based gap detector make the final call". Present
    /// records carry an ISO 8601 `completed_at` and a `stage_version`
    /// so future schema bumps can re-run stages whose contract
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<std::collections::BTreeMap<String, EnrichmentRecord>>,

    /// Cross-platform URLs discovered for this album (#295 Phase A).
    /// Keyed by Odesli's platform identifier (`spotify`,
    /// `youtubeMusic`, `tidal`, `deezer`, `amazonMusic`, …). Sparse —
    /// only populated when `odesli_lookup_enabled` was on at
    /// download time AND Odesli returned matches. Absent for older
    /// manifests / Odesli misses / lookup disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_platform_urls: Option<std::collections::BTreeMap<String, String>>,

    /// Whether this download originated from the user's personal Apple
    /// Music library URL (`/library/...`) rather than from the public
    /// catalog. Library items often have NO corresponding catalog entry
    /// (e.g., user-uploaded MP3s, region-restricted items), so catalog
    /// API enrichment is skipped for them — there's nothing to fetch.
    ///
    /// Surfaced as a hint to:
    ///   * The Library Scan page UI (so it doesn't try to compare
    ///     `lastModifiedDate` against a non-existent catalog response)
    ///   * The duplicate detector (library + catalog with the same
    ///     ISRC are different download instances of the same recording)
    ///   * Future EPIC A (#875) SQLite index — `downloads.is_library`
    ///     column maps directly from this field
    ///
    /// Defaults to `false` for backwards compatibility — older manifests
    /// (pre-#871) deserialise with `is_library = false`, which is
    /// correct because pre-#871 we didn't distinguish library URLs
    /// from catalog URLs at the manifest layer. (#871)
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_library: bool,
}

/// Helper for `#[serde(skip_serializing_if = "is_false")]`. Lets the
/// `is_library` field omit itself from the JSON when `false`, keeping
/// the on-disk format unchanged for catalog (non-library) manifests.
fn is_false(b: &bool) -> bool {
    !*b
}

/// Per-stage enrichment completion record (#759).
///
/// Recorded into the manifest after each enrichment stage runs to
/// completion. The Library Scan gap-fill UI consults this to decide
/// which stages need to be re-run.
///
/// Schema: keep additive only. New fields must be `#[serde(default)]`
/// so old manifests deserialize cleanly. `stage_version = 1` is the
/// initial baseline; bump per stage when the stage's output contract
/// changes in a way that warrants forced re-run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentRecord {
    /// ISO 8601 timestamp when the stage completed successfully.
    /// `None` means "ran but failed / partial / timed out".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Stage-contract version. Increment per stage when its output
    /// changes incompatibly (e.g. ReplayGain switches from RG2 to
    /// EBU R128). Defaults to 1 for any stage that hasn't bumped.
    #[serde(default = "default_stage_version")]
    pub stage_version: u32,
}

fn default_stage_version() -> u32 {
    1
}

/// Metadata for a single track within a manifest source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestTrack {
    /// Track number within the disc (1-based).
    pub number: u32,
    /// Disc number (1-based).
    #[serde(default = "default_disc")]
    pub disc: u32,
    /// Track title.
    pub title: String,
    /// Direct URL to this track (e.g., album URL with ?i= parameter).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Actual codec detected for this track (may differ from album-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    /// ISRC (International Standard Recording Code) for cross-platform matching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    /// Apple Music song_id (numeric string) — primary dedup key when a user
    /// enables the duplicate-detection history scope (#510). Older manifests
    /// written before this field existed will deserialize with `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song_id: Option<String>,
}

fn default_disc() -> u32 {
    1
}

impl ManifestFile {
    /// Create a new manifest with a single source.
    pub fn new(source: ManifestSource) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: 1,
            app: "MeedyaDL".to_string(),
            created_at: now.clone(),
            updated_at: now,
            sources: vec![source],
        }
    }

    /// Merge a new source into an existing manifest.
    ///
    /// **Dedup key** (`platform`, `url`, `codec`) — promoted from
    /// (`platform`, `url`) in #789 so a single album folder can carry
    /// both the primary codec source (e.g. Atmos) AND a companion
    /// codec source (e.g. ALAC) without one replacing the other. The
    /// pre-#789 dedup treated both as the same "re-download" and lost
    /// the companion's per-track metadata when the legacy-folder
    /// merge appended its source.
    ///
    /// Behaviour:
    ///   - Same (`platform`, `url`, `codec`) → replace (genuine
    ///     re-download of the same content + codec; tracks may have
    ///     been remastered / added).
    ///   - Different `codec` for the same `url` → append (the
    ///     companion-codec case that #789 surfaces).
    ///   - Different `url` → append (existing multi-platform /
    ///     multi-album behaviour).
    pub fn merge_source(&mut self, source: ManifestSource) {
        self.updated_at = chrono::Utc::now().to_rfc3339();

        // Replace existing source for the same platform + URL + codec,
        // or append. `codec` is part of the key so primary + companion
        // sources for the same album both survive the merge (#789).
        if let Some(existing) = self.sources.iter_mut().find(|s| {
            s.platform == source.platform
                && s.url == source.url
                && s.codec == source.codec
        }) {
            *existing = source;
        } else {
            self.sources.push(source);
        }
    }
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(platform: &str, url: &str, codec: Option<&str>) -> ManifestSource {
        ManifestSource {
            platform: platform.to_string(),
            url: url.to_string(),
            storefront: None,
            downloaded_at: "2026-05-17T12:00:00Z".to_string(),
            codec: codec.map(String::from),
            last_modified_date: None,
            tracks: Vec::new(),
            enrichment: None,
            cross_platform_urls: None,
            is_library: false,
        }
    }

    /// Genuine re-download: same (platform, url, codec) → replace.
    /// Preserves the old behaviour for the most common case.
    #[test]
    fn merge_source_replaces_same_platform_url_codec() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        let mut second = make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        );
        second.downloaded_at = "2026-05-17T13:00:00Z".to_string();
        m.merge_source(second);
        assert_eq!(m.sources.len(), 1, "same (platform, url, codec) → replace");
        assert_eq!(m.sources[0].downloaded_at, "2026-05-17T13:00:00Z");
    }

    /// Companion-codec case (#789): same album URL but different codec
    /// → append both, don't replace. This is the change the issue
    /// surfaced: pre-fix the companion's source would overwrite the
    /// primary's source and lose its tracks.
    #[test]
    fn merge_source_appends_when_codec_differs_on_same_url() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("atmos"),
        ));
        m.merge_source(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        assert_eq!(m.sources.len(), 2);
        assert_eq!(m.sources[0].codec.as_deref(), Some("atmos"));
        assert_eq!(m.sources[1].codec.as_deref(), Some("alac"));
    }

    /// Different URL still appends regardless of codec (existing
    /// multi-album behaviour preserved).
    #[test]
    fn merge_source_appends_when_url_differs() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        m.merge_source(make_source(
            "apple-music",
            "https://music.apple.com/us/album/bar/456",
            Some("alac"),
        ));
        assert_eq!(m.sources.len(), 2);
    }

    /// Different platform always appends (multi-service support).
    #[test]
    fn merge_source_appends_when_platform_differs() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        m.merge_source(make_source(
            "spotify",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        assert_eq!(m.sources.len(), 2);
    }

    /// Both sources lack a codec field → they're still "the same"
    /// source (the dedup key matches on `None == None`).
    #[test]
    fn merge_source_replaces_when_both_codecs_are_none() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            None,
        ));
        m.merge_source(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            None,
        ));
        assert_eq!(m.sources.len(), 1);
    }

    /// One source has a codec, the other doesn't → these are
    /// distinct entries (a legacy manifest without codec metadata
    /// next to a fresh one with codec).
    #[test]
    fn merge_source_treats_some_codec_and_none_as_distinct() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            None,
        ));
        m.merge_source(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        assert_eq!(m.sources.len(), 2);
    }

    /// `updated_at` bumps on every merge call (so consumers can
    /// detect manifest staleness).
    #[test]
    fn merge_source_bumps_updated_at() {
        let mut m = ManifestFile::new(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("alac"),
        ));
        let original_updated = m.updated_at.clone();
        // Wait at least 1 ms so the new RFC3339 timestamp differs.
        std::thread::sleep(std::time::Duration::from_millis(2));
        m.merge_source(make_source(
            "apple-music",
            "https://music.apple.com/us/album/foo/123",
            Some("atmos"),
        ));
        assert_ne!(m.updated_at, original_updated);
    }
}
