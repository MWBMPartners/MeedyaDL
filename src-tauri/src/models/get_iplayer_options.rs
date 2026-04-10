// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// get_iplayer CLI option models.
// Defines typed Rust representations of the most commonly used get_iplayer
// command-line options for BBC iPlayer downloads.
//
// get_iplayer is the primary engine for BBC iPlayer content, with yt-dlp
// available as a fallback. All fields are `Option<T>` to support the same
// merge pattern used by `GamdlOptions`.
//
// ## References
//
// - get_iplayer documentation: <https://github.com/get-iplayer/get_iplayer>
// - serde: <https://docs.rs/serde/latest/serde/>

use serde::{Deserialize, Serialize};

/// get_iplayer CLI options for BBC iPlayer downloads.
///
/// Maps to get_iplayer command-line flags. Each field corresponds to a
/// specific CLI option. All fields are `Option<T>` so they can be
/// used as per-download overrides on top of global settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GetIplayerOptions {
    /// Recording quality (e.g., `"best"`, `"better"`, `"good"`,
    /// `"worst"`). Maps to `--quality` / `--tv-quality` / `--radio-quality`.
    pub quality: Option<String>,

    /// Output directory path for downloaded content.
    pub output_path: Option<String>,

    /// Output filename template using get_iplayer's template syntax.
    pub output_template: Option<String>,

    /// Whether to download subtitles alongside the video.
    /// Maps to `--subtitles`.
    pub subtitles: Option<bool>,

    /// Whether to download the programme thumbnail.
    /// Maps to `--thumbnail`.
    pub thumbnail: Option<bool>,

    /// Whether to overwrite existing files.
    /// Maps to `--overwrite`.
    pub overwrite: Option<bool>,

    /// Custom FFmpeg binary path for remuxing.
    /// Maps to `--ffmpeg`.
    pub ffmpeg_path: Option<String>,

    /// Content type filter (`"tv"` or `"radio"`).
    /// Maps to `--type`.
    pub type_filter: Option<String>,
}
