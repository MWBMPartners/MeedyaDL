// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Votify CLI option models.
// Defines typed Rust representations of the most commonly used votify
// command-line options for Spotify downloads.
//
// All fields are `Option<T>` to support the same merge pattern used by
// `GamdlOptions`: per-download overrides are merged on top of global
// defaults from `AppSettings::services::spotify`.
//
// ## References
//
// - votify documentation: <https://github.com/glomatico/votify>
// - serde: <https://docs.rs/serde/latest/serde/>

use serde::{Deserialize, Serialize};

/// Votify CLI options for Spotify downloads.
///
/// Maps to votify command-line flags. Each field corresponds to a
/// specific CLI option. All fields are `Option<T>` so they can be
/// used as per-download overrides on top of global settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VotifyOptions {
    /// Audio quality level (e.g., `"vorbis-high"`, `"vorbis-medium"`,
    /// `"vorbis-low"`). Maps to votify's quality selection flag.
    pub audio_quality: Option<String>,

    /// Output directory path for downloaded tracks.
    pub output_path: Option<String>,

    /// Output filename template using votify's template syntax.
    pub output_template: Option<String>,

    /// Whether to save cover art as a separate image file alongside
    /// the downloaded audio.
    pub save_cover: Option<bool>,

    /// Cover art dimensions in pixels (square). Higher values request
    /// larger artwork from Spotify's CDN.
    pub cover_size: Option<u32>,

    /// Whether to embed lyrics in the audio file metadata.
    pub embed_lyrics: Option<bool>,

    /// Whether to overwrite existing files.
    pub overwrite: Option<bool>,

    /// Custom FFmpeg binary path for post-processing.
    pub ffmpeg_path: Option<String>,

    /// Path to a cookies file for Spotify authentication.
    pub cookies_path: Option<String>,
}
