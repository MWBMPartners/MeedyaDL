// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Application settings model.
// Defines the complete settings structure for the gamdl-GUI application.
// These settings are persisted as JSON in the app data directory and
// control both the GUI behavior and the default GAMDL options.

use serde::{Deserialize, Serialize};

use super::gamdl_options::{
    CoverFormat, DownloadMode, LyricsFormat, RemuxMode, SongCodec, VideoResolution,
};

/// Complete application settings, persisted as {app_data}/settings.json.
///
/// This struct contains all user-configurable preferences, organized
/// into logical sections. Default values provide sensible starting
/// points that match the project brief requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // --- General ---
    /// Output directory for downloaded music and videos
    pub output_path: String,
    /// Metadata language (ISO 639-1 code, e.g., "en-US")
    pub language: String,
    /// Whether to overwrite existing files during download
    pub overwrite: bool,
    /// Whether to automatically check for GAMDL/tool updates on startup
    pub auto_check_updates: bool,

    // --- Audio Quality Defaults ---
    /// Default audio codec for music downloads
    pub default_song_codec: SongCodec,

    // --- Video Quality Defaults ---
    /// Default maximum video resolution
    pub default_video_resolution: VideoResolution,
    /// Default video codec priority (comma-separated, e.g., "h265,h264")
    pub default_video_codec_priority: String,
    /// Default video container format ("mp4" or "m4v")
    pub default_video_remux_format: String,

    // --- Fallback Quality Chains ---
    /// Whether the fallback quality system is enabled
    pub fallback_enabled: bool,
    /// Ordered list of audio codecs to try if the preferred codec fails.
    /// The first codec is tried first; on failure, the next is attempted.
    pub music_fallback_chain: Vec<SongCodec>,
    /// Ordered list of video resolutions to try if the preferred fails.
    pub video_fallback_chain: Vec<VideoResolution>,

    // --- Lyrics ---
    /// Default format for synced lyrics
    pub synced_lyrics_format: LyricsFormat,
    /// Whether to skip downloading synced lyrics
    pub no_synced_lyrics: bool,
    /// Whether to download only lyrics (no audio/video)
    pub synced_lyrics_only: bool,

    // --- Cover Art ---
    /// Whether to save cover art as a separate image file
    pub save_cover: bool,
    /// Image format for saved cover art
    pub cover_format: CoverFormat,
    /// Cover art dimensions in pixels (square: NxN)
    pub cover_size: u32,

    // --- File/Folder Templates ---
    /// Folder naming template for album downloads
    pub album_folder_template: String,
    /// Folder naming template for compilation albums
    pub compilation_folder_template: String,
    /// Folder naming template for non-album tracks
    pub no_album_folder_template: String,
    /// File naming template for single-disc albums
    pub single_disc_file_template: String,
    /// File naming template for multi-disc albums
    pub multi_disc_file_template: String,
    /// File naming template for non-album tracks
    pub no_album_file_template: String,
    /// Folder/file naming template for playlists
    pub playlist_file_template: String,

    // --- Tool Paths (None = use managed/bundled tools) ---
    /// Path to cookies.txt file for Apple Music authentication
    pub cookies_path: Option<String>,
    /// Custom FFmpeg path (None = use managed installation)
    pub ffmpeg_path: Option<String>,
    /// Custom mp4decrypt path
    pub mp4decrypt_path: Option<String>,
    /// Custom MP4Box path
    pub mp4box_path: Option<String>,
    /// Custom N_m3u8DL-RE path
    pub nm3u8dlre_path: Option<String>,
    /// Custom amdecrypt path
    pub amdecrypt_path: Option<String>,

    // --- Advanced ---
    /// Download tool selection (yt-dlp or N_m3u8DL-RE)
    pub download_mode: DownloadMode,
    /// Remux tool selection (FFmpeg or MP4Box)
    pub remux_mode: RemuxMode,
    /// Whether to use the wrapper/amdecrypt system
    pub use_wrapper: bool,
    /// Wrapper server URL (when use_wrapper is true)
    pub wrapper_account_url: String,
    /// Maximum filename length (None = no truncation)
    pub truncate: Option<u32>,
    /// Tags to exclude from metadata embedding
    pub exclude_tags: Vec<String>,

    // --- UI State ---
    /// Whether the sidebar navigation is collapsed
    pub sidebar_collapsed: bool,
    /// Override platform theme (None = auto-detect from OS)
    pub theme_override: Option<String>,
}

impl Default for AppSettings {
    /// Creates default settings that match the project brief requirements.
    ///
    /// Key defaults:
    /// - Cover art format: Raw (highest quality, as specified in brief)
    /// - Music fallback chain: ALAC → Atmos → AC3 → AAC Binaural → AAC → AAC Legacy
    /// - Video fallback chain: 2160p → 1440p → 1080p → ... → 240p
    /// - Lyrics format: LRC (for music; TTML is used for videos at download time)
    fn default() -> Self {
        Self {
            // General
            output_path: String::new(), // Will be resolved to platform Music dir at runtime
            language: "en-US".to_string(),
            overwrite: false,
            auto_check_updates: true,

            // Audio quality - default to highest available
            default_song_codec: SongCodec::Alac,

            // Video quality - default to highest available
            default_video_resolution: VideoResolution::P2160,
            default_video_codec_priority: "h265,h264".to_string(),
            default_video_remux_format: "m4v".to_string(),

            // Fallback chains (as specified in the project brief)
            fallback_enabled: true,
            music_fallback_chain: vec![
                SongCodec::Alac,        // 1. Lossless (ALAC) - highest quality
                SongCodec::Atmos,       // 2. Dolby Atmos
                SongCodec::Ac3,         // 3. Dolby Digital (AC3)
                SongCodec::AacBinaural, // 4. AAC (256kbps) Binaural
                SongCodec::Aac,         // 5. AAC (256kbps at up to 48kHz)
                SongCodec::AacLegacy,   // 6. AAC Legacy (256kbps at up to 44.1kHz)
            ],
            video_fallback_chain: vec![
                VideoResolution::P2160, // 1. H.265 2160p (4K)
                VideoResolution::P1440, // 2. H.265 1440p
                VideoResolution::P1080, // 3. H.265/H.264 1080p
                VideoResolution::P720,  // 4. H.264 720p
                VideoResolution::P540,  // 5. H.264 540p
                VideoResolution::P480,  // 6. H.264 480p
                VideoResolution::P360,  // 7. H.264 360p
                VideoResolution::P240,  // 8. H.264 240p
            ],

            // Lyrics - default to LRC for music
            synced_lyrics_format: LyricsFormat::Lrc,
            no_synced_lyrics: false,
            synced_lyrics_only: false,

            // Cover art - default to Raw (highest quality, as per project brief)
            save_cover: true,
            cover_format: CoverFormat::Raw,
            cover_size: 1200,

            // Templates (GAMDL defaults)
            album_folder_template: "{album_artist}/{album}".to_string(),
            compilation_folder_template: "Compilations/{album}".to_string(),
            no_album_folder_template: "{artist}/Unknown Album".to_string(),
            single_disc_file_template: "{track:02d} {title}".to_string(),
            multi_disc_file_template: "{disc}-{track:02d} {title}".to_string(),
            no_album_file_template: "{title}".to_string(),
            playlist_file_template: "Playlists/{playlist_artist}/{playlist_title}".to_string(),

            // Tool paths - all None means use managed installations
            cookies_path: None,
            ffmpeg_path: None,
            mp4decrypt_path: None,
            mp4box_path: None,
            nm3u8dlre_path: None,
            amdecrypt_path: None,

            // Advanced
            download_mode: DownloadMode::Ytdlp,
            remux_mode: RemuxMode::Ffmpeg,
            use_wrapper: false,
            wrapper_account_url: "http://127.0.0.1:30020".to_string(),
            truncate: None,
            exclude_tags: Vec::new(),

            // UI state
            sidebar_collapsed: false,
            theme_override: None,
        }
    }
}
