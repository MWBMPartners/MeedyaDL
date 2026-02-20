// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Unified download options enum.
// Wraps service-specific option types into a single tagged enum so the
// download queue can store and route options without knowing the details
// of each service's CLI tool.
//
// ## Serialization
//
// Uses serde's internally-tagged representation (`#[serde(tag = "service",
// content = "options")]`) so the JSON looks like:
// ```json
// { "service": "AppleMusic", "options": { "song_codec": "alac", ... } }
// ```
// This makes it easy for the TypeScript frontend to construct and
// discriminate on the `service` field.

use serde::{Deserialize, Serialize};

use super::gamdl_options::GamdlOptions;
use super::get_iplayer_options::GetIplayerOptions;
use super::votify_options::VotifyOptions;
use super::ytdlp_options::YtdlpOptions;

/// Unified download options enum that wraps service-specific option types.
///
/// The download queue uses this to store merged options for each queue item
/// and route them to the correct service handler when processing downloads.
///
/// ## Usage
///
/// The `enqueue()` function in `download_queue.rs` constructs this enum
/// after merging per-download overrides with global settings. The
/// `run_download_with_events()` function then matches on the variant to
/// dispatch to the correct service's subprocess builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "service", content = "options")]
pub enum DownloadOptions {
    /// Apple Music download options (GAMDL CLI flags).
    AppleMusic(GamdlOptions),

    /// YouTube download options (yt-dlp CLI flags).
    YouTube(YtdlpOptions),

    /// BBC iPlayer download options (get_iplayer CLI flags).
    BBCiPlayer(GetIplayerOptions),

    /// Spotify download options (votify CLI flags).
    Spotify(VotifyOptions),
}

impl DownloadOptions {
    /// Returns a common output path if set, regardless of service.
    ///
    /// Useful for the download queue to determine where files will be
    /// written without needing to know which service is being used.
    pub fn output_path(&self) -> Option<&str> {
        match self {
            DownloadOptions::AppleMusic(opts) => opts.output_path.as_deref(),
            DownloadOptions::YouTube(opts) => opts.output_path.as_deref(),
            DownloadOptions::BBCiPlayer(opts) => opts.output_path.as_deref(),
            DownloadOptions::Spotify(opts) => opts.output_path.as_deref(),
        }
    }
}
