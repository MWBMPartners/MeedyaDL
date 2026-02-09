// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL CLI option models.
// This module defines typed Rust representations of every command-line
// option supported by GAMDL. These types ensure type safety when
// constructing CLI commands and are shared with the frontend via
// serialization for the settings and download option UIs.

use serde::{Deserialize, Serialize};

/// All audio codec options supported by GAMDL's --song-codec flag.
/// Listed in the order recommended for the default fallback chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SongCodec {
    /// Apple Lossless Audio Codec - highest quality (24-bit/192kHz)
    Alac,
    /// Dolby Atmos spatial audio (requires wrapper for reliable access)
    Atmos,
    /// Dolby Digital AC-3 codec
    Ac3,
    /// AAC with binaural spatial processing (256kbps)
    AacBinaural,
    /// Standard AAC (256kbps at up to 48kHz)
    Aac,
    /// Legacy AAC (256kbps at up to 44.1kHz) - most compatible
    AacLegacy,
    /// AAC-HE (High Efficiency) - 64kbps at 44.1kHz
    AacHeLegacy,
    /// AAC-HE variant (experimental)
    AacHe,
    /// AAC downmix variant (experimental)
    AacDownmix,
    /// AAC-HE with binaural processing (experimental)
    AacHeBinaural,
    /// AAC-HE downmix variant (experimental)
    AacHeDownmix,
}

impl SongCodec {
    /// Converts the enum variant to the exact CLI string GAMDL expects.
    /// These must match GAMDL's --song-codec accepted values exactly.
    pub fn to_cli_string(&self) -> &str {
        match self {
            SongCodec::Alac => "alac",
            SongCodec::Atmos => "atmos",
            SongCodec::Ac3 => "ac3",
            SongCodec::AacBinaural => "aac-binaural",
            SongCodec::Aac => "aac",
            SongCodec::AacLegacy => "aac-legacy",
            SongCodec::AacHeLegacy => "aac-he-legacy",
            SongCodec::AacHe => "aac-he",
            SongCodec::AacDownmix => "aac-downmix",
            SongCodec::AacHeBinaural => "aac-he-binaural",
            SongCodec::AacHeDownmix => "aac-he-downmix",
        }
    }

    /// Human-readable display name for the UI dropdown/selector.
    pub fn display_name(&self) -> &str {
        match self {
            SongCodec::Alac => "Lossless (ALAC)",
            SongCodec::Atmos => "Dolby Atmos",
            SongCodec::Ac3 => "Dolby Digital (AC3)",
            SongCodec::AacBinaural => "AAC (256kbps) Binaural",
            SongCodec::Aac => "AAC (256kbps at up to 48kHz)",
            SongCodec::AacLegacy => "AAC Legacy (256kbps at up to 44.1kHz)",
            SongCodec::AacHeLegacy => "AAC-HE Legacy (64kbps)",
            SongCodec::AacHe => "AAC-HE (Experimental)",
            SongCodec::AacDownmix => "AAC Downmix (Experimental)",
            SongCodec::AacHeBinaural => "AAC-HE Binaural (Experimental)",
            SongCodec::AacHeDownmix => "AAC-HE Downmix (Experimental)",
        }
    }
}

/// Video resolution options for GAMDL's --music-video-resolution flag.
/// Listed from highest to lowest quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoResolution {
    /// 4K Ultra HD (2160p) - requires H.265 codec
    #[serde(rename = "2160p")]
    P2160,
    /// Quad HD (1440p) - requires H.265 codec
    #[serde(rename = "1440p")]
    P1440,
    /// Full HD (1080p) - available with both H.264 and H.265
    #[serde(rename = "1080p")]
    P1080,
    /// HD (720p) - H.264 only
    #[serde(rename = "720p")]
    P720,
    /// qHD (540p) - H.264 only
    #[serde(rename = "540p")]
    P540,
    /// Standard definition (480p) - H.264 only
    #[serde(rename = "480p")]
    P480,
    /// Low definition (360p) - H.264 only
    #[serde(rename = "360p")]
    P360,
    /// Lowest quality (240p) - H.264 only
    #[serde(rename = "240p")]
    P240,
}

impl VideoResolution {
    /// Converts to the CLI string GAMDL expects for --music-video-resolution.
    pub fn to_cli_string(&self) -> &str {
        match self {
            VideoResolution::P2160 => "2160p",
            VideoResolution::P1440 => "1440p",
            VideoResolution::P1080 => "1080p",
            VideoResolution::P720 => "720p",
            VideoResolution::P540 => "540p",
            VideoResolution::P480 => "480p",
            VideoResolution::P360 => "360p",
            VideoResolution::P240 => "240p",
        }
    }
}

/// Synced lyrics format options for GAMDL's --synced-lyrics-format flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsFormat {
    /// LRC format - timestamped lyrics (default for music)
    Lrc,
    /// SRT subtitle format
    Srt,
    /// TTML format - Timed Text Markup Language (default for videos)
    Ttml,
}

impl LyricsFormat {
    /// Converts to the CLI string GAMDL expects.
    pub fn to_cli_string(&self) -> &str {
        match self {
            LyricsFormat::Lrc => "lrc",
            LyricsFormat::Srt => "srt",
            LyricsFormat::Ttml => "ttml",
        }
    }
}

/// Cover art image format options for GAMDL's --cover-format flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverFormat {
    /// JPEG format - smaller file size, lossy compression
    Jpg,
    /// PNG format - lossless, larger file size
    Png,
    /// Raw format - original format as provided by the artist
    Raw,
}

impl CoverFormat {
    /// Converts to the CLI string GAMDL expects.
    pub fn to_cli_string(&self) -> &str {
        match self {
            CoverFormat::Jpg => "jpg",
            CoverFormat::Png => "png",
            CoverFormat::Raw => "raw",
        }
    }
}

/// Download mode options for GAMDL's --download-mode flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadMode {
    /// Use yt-dlp for downloading (default, most compatible)
    Ytdlp,
    /// Use N_m3u8DL-RE for downloading (faster alternative)
    Nm3u8dlre,
}

/// Remux mode options for GAMDL's --remux-mode flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemuxMode {
    /// Use FFmpeg for remuxing (default)
    Ffmpeg,
    /// Use MP4Box for remuxing (alternative)
    Mp4box,
}

/// Log level options for GAMDL's --log-level flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Complete set of GAMDL CLI options.
///
/// This struct maps to every flag and argument GAMDL supports.
/// All fields are Optional because:
/// 1. For global settings, only user-configured fields are set (rest use GAMDL defaults)
/// 2. For per-download overrides, only overridden fields are set (rest inherit from globals)
///
/// The `to_cli_args()` method converts this struct into a Vec<String> of
/// CLI arguments that can be passed directly to GAMDL's subprocess.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GamdlOptions {
    // --- Audio Quality ---
    /// Audio codec for music downloads
    pub song_codec: Option<SongCodec>,

    // --- Video Quality ---
    /// Comma-separated codec priority for music videos (e.g., "h265,h264")
    pub music_video_codec_priority: Option<String>,
    /// Maximum video resolution
    pub music_video_resolution: Option<VideoResolution>,
    /// Video container format ("mp4" or "m4v")
    pub music_video_remux_format: Option<String>,
    /// Uploaded/post video quality ("best" or "ask")
    pub uploaded_video_quality: Option<String>,
    /// Whether to skip music videos in album/playlist downloads
    pub disable_music_video_skip: Option<bool>,

    // --- Lyrics ---
    /// Format for synced lyrics download
    pub synced_lyrics_format: Option<LyricsFormat>,
    /// Skip downloading synced lyrics entirely
    pub no_synced_lyrics: Option<bool>,
    /// Download only lyrics (no audio/video)
    pub synced_lyrics_only: Option<bool>,

    // --- Cover Art ---
    /// Save cover art as a separate image file
    pub save_cover: Option<bool>,
    /// Image format for saved cover art
    pub cover_format: Option<CoverFormat>,
    /// Cover art dimensions in pixels (e.g., 1200)
    pub cover_size: Option<u32>,

    // --- Output ---
    /// Download output directory
    pub output_path: Option<String>,
    /// Temporary file directory
    pub temp_path: Option<String>,
    /// Overwrite existing files
    pub overwrite: Option<bool>,
    /// Maximum filename length
    pub truncate: Option<u32>,

    // --- Authentication ---
    /// Path to Netscape-format cookies file
    pub cookies_path: Option<String>,
    /// Whether to use the wrapper/amdecrypt system
    pub use_wrapper: Option<bool>,
    /// Wrapper server URL
    pub wrapper_account_url: Option<String>,
    /// Decryption server address
    pub wrapper_decrypt_ip: Option<String>,

    // --- Metadata ---
    /// Language for metadata (ISO 639-1 code, e.g., "en-US")
    pub language: Option<String>,
    /// Comma-separated list of tags to exclude from embedding
    pub exclude_tags: Option<String>,
    /// Use album release date for all tracks
    pub use_album_date: Option<bool>,
    /// Fetch extra metadata (normalization, smooth playback)
    pub fetch_extra_tags: Option<bool>,
    /// Date format for metadata tags
    pub date_tag_template: Option<String>,

    // --- Templates ---
    /// Folder template for album downloads
    pub album_folder_template: Option<String>,
    /// Folder template for compilation albums
    pub compilation_folder_template: Option<String>,
    /// Folder template for non-album tracks
    pub no_album_folder_template: Option<String>,
    /// File template for single-disc albums
    pub single_disc_file_template: Option<String>,
    /// File template for multi-disc albums
    pub multi_disc_file_template: Option<String>,
    /// File template for non-album tracks
    pub no_album_file_template: Option<String>,
    /// Folder/file template for playlists
    pub playlist_file_template: Option<String>,

    // --- Tool Paths ---
    /// Path to FFmpeg binary
    pub ffmpeg_path: Option<String>,
    /// Path to mp4decrypt binary
    pub mp4decrypt_path: Option<String>,
    /// Path to MP4Box binary
    pub mp4box_path: Option<String>,
    /// Path to N_m3u8DL-RE binary
    pub nm3u8dlre_path: Option<String>,
    /// Path to amdecrypt binary
    pub amdecrypt_path: Option<String>,
    /// Path to .wvd (Widevine Device) file
    pub wvd_path: Option<String>,

    // --- Modes ---
    /// Download mode selection (yt-dlp or N_m3u8DL-RE)
    pub download_mode: Option<DownloadMode>,
    /// Remux mode selection (FFmpeg or MP4Box)
    pub remux_mode: Option<RemuxMode>,

    // --- Other ---
    /// Log verbosity level
    pub log_level: Option<LogLevel>,
    /// Suppress exception printing
    pub no_exceptions: Option<bool>,
    /// Generate M3U8 playlist file
    pub save_playlist: Option<bool>,
    /// Read URLs from text files instead of command line
    pub read_urls_as_txt: Option<bool>,
    /// Skip using GAMDL's own config file
    pub no_config_file: Option<bool>,
}

impl GamdlOptions {
    /// Converts the options struct into a vector of CLI argument strings.
    ///
    /// Only fields that are Some() generate CLI flags. This allows
    /// selective overriding: global settings set all fields, per-download
    /// overrides only set changed fields, and the merge happens before
    /// calling this method.
    ///
    /// The returned vector can be passed directly to Command::args().
    pub fn to_cli_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // --- Audio Quality ---
        if let Some(ref codec) = self.song_codec {
            args.push("--song-codec".to_string());
            args.push(codec.to_cli_string().to_string());
        }

        // --- Video Quality ---
        if let Some(ref priority) = self.music_video_codec_priority {
            args.push("--music-video-codec-priority".to_string());
            args.push(priority.clone());
        }
        if let Some(ref resolution) = self.music_video_resolution {
            args.push("--music-video-resolution".to_string());
            args.push(resolution.to_cli_string().to_string());
        }
        if let Some(ref format) = self.music_video_remux_format {
            args.push("--music-video-remux-format".to_string());
            args.push(format.clone());
        }
        if let Some(ref quality) = self.uploaded_video_quality {
            args.push("--uploaded-video-quality".to_string());
            args.push(quality.clone());
        }
        if self.disable_music_video_skip == Some(true) {
            args.push("--disable-music-video-skip".to_string());
        }

        // --- Lyrics ---
        if let Some(ref format) = self.synced_lyrics_format {
            args.push("--synced-lyrics-format".to_string());
            args.push(format.to_cli_string().to_string());
        }
        if self.no_synced_lyrics == Some(true) {
            args.push("--no-synced-lyrics".to_string());
        }
        if self.synced_lyrics_only == Some(true) {
            args.push("--synced-lyrics-only".to_string());
        }

        // --- Cover Art ---
        if self.save_cover == Some(true) {
            args.push("--save-cover".to_string());
        }
        if let Some(ref format) = self.cover_format {
            args.push("--cover-format".to_string());
            args.push(format.to_cli_string().to_string());
        }
        if let Some(size) = self.cover_size {
            args.push("--cover-size".to_string());
            args.push(format!("{}x{}", size, size));
        }

        // --- Output ---
        if let Some(ref path) = self.output_path {
            args.push("--output-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.temp_path {
            args.push("--temp-path".to_string());
            args.push(path.clone());
        }
        if self.overwrite == Some(true) {
            args.push("--overwrite".to_string());
        }
        if let Some(truncate) = self.truncate {
            args.push("--truncate".to_string());
            args.push(truncate.to_string());
        }

        // --- Authentication ---
        if let Some(ref path) = self.cookies_path {
            args.push("--cookies-path".to_string());
            args.push(path.clone());
        }
        if self.use_wrapper == Some(true) {
            args.push("--use-wrapper".to_string());
        }
        if let Some(ref url) = self.wrapper_account_url {
            args.push("--wrapper-account-url".to_string());
            args.push(url.clone());
        }
        if let Some(ref ip) = self.wrapper_decrypt_ip {
            args.push("--wrapper-decrypt-ip".to_string());
            args.push(ip.clone());
        }

        // --- Metadata ---
        if let Some(ref lang) = self.language {
            args.push("--language".to_string());
            args.push(lang.clone());
        }
        if let Some(ref tags) = self.exclude_tags {
            args.push("--exclude-tags".to_string());
            args.push(tags.clone());
        }
        if self.use_album_date == Some(true) {
            args.push("--use-album-date".to_string());
        }
        if self.fetch_extra_tags == Some(true) {
            args.push("--fetch-extra-tags".to_string());
        }
        if let Some(ref template) = self.date_tag_template {
            args.push("--date-tag-template".to_string());
            args.push(template.clone());
        }

        // --- Templates ---
        if let Some(ref t) = self.album_folder_template {
            args.push("--album-folder-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.compilation_folder_template {
            args.push("--compilation-folder-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.no_album_folder_template {
            args.push("--no-album-folder-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.single_disc_file_template {
            args.push("--single-disc-file-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.multi_disc_file_template {
            args.push("--multi-disc-file-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.no_album_file_template {
            args.push("--no-album-file-template".to_string());
            args.push(t.clone());
        }
        if let Some(ref t) = self.playlist_file_template {
            args.push("--playlist-file-template".to_string());
            args.push(t.clone());
        }

        // --- Tool Paths ---
        if let Some(ref path) = self.ffmpeg_path {
            args.push("--ffmpeg-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.mp4decrypt_path {
            args.push("--mp4decrypt-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.mp4box_path {
            args.push("--mp4box-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.nm3u8dlre_path {
            args.push("--nm3u8dlre-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.amdecrypt_path {
            args.push("--amdecrypt-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.wvd_path {
            args.push("--wvd-path".to_string());
            args.push(path.clone());
        }

        // --- Modes ---
        if let Some(ref mode) = self.download_mode {
            args.push("--download-mode".to_string());
            args.push(match mode {
                DownloadMode::Ytdlp => "ytdlp",
                DownloadMode::Nm3u8dlre => "nm3u8dlre",
            }.to_string());
        }
        if let Some(ref mode) = self.remux_mode {
            args.push("--remux-mode".to_string());
            args.push(match mode {
                RemuxMode::Ffmpeg => "ffmpeg",
                RemuxMode::Mp4box => "mp4box",
            }.to_string());
        }

        // --- Other ---
        if let Some(ref level) = self.log_level {
            args.push("--log-level".to_string());
            args.push(match level {
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO",
                LogLevel::Warning => "WARNING",
                LogLevel::Error => "ERROR",
            }.to_string());
        }
        if self.no_exceptions == Some(true) {
            args.push("--no-exceptions".to_string());
        }
        if self.save_playlist == Some(true) {
            args.push("--save-playlist".to_string());
        }
        if self.no_config_file == Some(true) {
            args.push("--no-config-file".to_string());
        }

        args
    }
}
