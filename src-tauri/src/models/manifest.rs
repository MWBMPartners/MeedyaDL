// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download manifest model (.meedyadl files embedded in album folders).
//
// Each downloaded album/playlist folder contains a `.meedyadl` JSON
// manifest recording the source URL(s) and per-track metadata. This
// enables one-click re-download by importing the file into MeedyaDL.
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
    /// Per-track metadata from the download.
    #[serde(default)]
    pub tracks: Vec<ManifestTrack>,
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
    /// If a source with the same `platform` and `url` already exists,
    /// it is replaced (re-download of the same content). Otherwise,
    /// the new source is appended.
    pub fn merge_source(&mut self, source: ManifestSource) {
        self.updated_at = chrono::Utc::now().to_rfc3339();

        // Replace existing source for the same platform + URL, or append
        if let Some(existing) = self
            .sources
            .iter_mut()
            .find(|s| s.platform == source.platform && s.url == source.url)
        {
            *existing = source;
        } else {
            self.sources.push(source);
        }
    }
}
