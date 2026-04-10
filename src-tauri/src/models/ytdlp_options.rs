// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// yt-dlp CLI option models.
// Defines typed Rust representations of the most commonly used yt-dlp
// command-line options for YouTube and BBC iPlayer downloads.
//
// All fields are `Option<T>` to support the same merge pattern used by
// `GamdlOptions`: per-download overrides are merged on top of global
// defaults from `AppSettings::services::youtube`.
//
// ## References
//
// - yt-dlp documentation: <https://github.com/yt-dlp/yt-dlp#usage-and-options>
// - serde: <https://docs.rs/serde/latest/serde/>

use serde::{Deserialize, Serialize};

/// yt-dlp CLI options for YouTube and BBC iPlayer downloads.
///
/// Maps to yt-dlp command-line flags. Each field corresponds to a
/// specific CLI option. All fields are `Option<T>` so they can be
/// used as per-download overrides on top of global settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YtdlpOptions {
    /// yt-dlp format selection string (`-f`).
    /// Examples: `"bestvideo+bestaudio/best"`, `"bestaudio[ext=m4a]"`.
    pub format: Option<String>,

    /// Maximum video resolution (e.g., `"1080"`, `"720"`, `"best"`).
    /// Used to construct format filters like `-f "bestvideo[height<=1080]+bestaudio"`.
    pub video_resolution: Option<String>,

    /// Preferred audio format (e.g., `"mp3"`, `"m4a"`, `"opus"`, `"best"`).
    /// Used with `--audio-format` when downloading audio-only content.
    pub audio_format: Option<String>,

    /// Output filename template (`-o`).
    /// Uses yt-dlp template syntax (e.g., `"%(title)s.%(ext)s"`).
    pub output_template: Option<String>,

    /// Output directory path. Combined with `output_template` to form
    /// the full output path.
    pub output_path: Option<String>,

    /// Path to a Netscape-format cookies file for YouTube authentication.
    /// Required for age-restricted content, private playlists, etc.
    pub cookies_path: Option<String>,

    /// Whether to embed thumbnail as cover art in the output file.
    /// Maps to `--embed-thumbnail`.
    pub embed_thumbnail: Option<bool>,

    /// Whether to embed metadata (title, artist, etc.) in the output file.
    /// Maps to `--embed-metadata`.
    pub embed_metadata: Option<bool>,

    /// Whether to embed subtitles in the output file.
    /// Maps to `--embed-subs`.
    pub embed_subtitles: Option<bool>,

    /// Comma-separated list of subtitle language codes to download.
    /// Maps to `--sub-langs` (e.g., `"en,de,fr"` or `"all"`).
    pub subtitle_langs: Option<String>,

    /// Whether to bypass geo-restrictions.
    /// Maps to `--geo-bypass`.
    pub geo_bypass: Option<bool>,

    /// Custom FFmpeg binary path for post-processing.
    /// Maps to `--ffmpeg-location`.
    pub ffmpeg_path: Option<String>,

    /// Whether to overwrite existing files.
    /// Maps to `--force-overwrites`.
    pub overwrite: Option<bool>,

    /// SponsorBlock categories to remove (e.g., `"sponsor,selfpromo"`).
    /// Maps to `--sponsorblock-remove`.
    pub sponsorblock_remove: Option<String>,

    /// Whether to start downloading live streams from the beginning.
    /// Maps to `--live-from-start`.
    pub live_from_start: Option<bool>,

    /// Number of concurrent download fragments.
    /// Maps to `--concurrent-fragments`. Higher values speed up downloads
    /// but use more bandwidth and connections.
    pub concurrent_fragments: Option<u32>,
}
