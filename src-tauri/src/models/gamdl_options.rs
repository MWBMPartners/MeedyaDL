// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL CLI option models.
// This module defines typed Rust representations of every command-line
// option supported by GAMDL. These types ensure type safety when
// constructing CLI commands and are shared with the frontend via
// serialization for the settings and download option UIs.
//
// ## Architecture
//
// The types in this module serve three roles:
// 1. **Settings persistence** -- serialized to/from JSON via serde for
//    the `AppSettings` struct in `settings.rs`.
// 2. **Frontend communication** -- exposed over the Tauri IPC bridge so
//    the React UI can present dropdowns/options with correct values.
// 3. **CLI argument generation** -- the `to_cli_args()` method on
//    `GamdlOptions` converts typed Rust values into the exact strings
//    that the `gamdl` Python CLI expects on the command line.
//
// ## References
//
// - GAMDL CLI source and docs: <https://github.com/glomatico/gamdl>
// - serde derive macros: <https://docs.rs/serde/latest/serde/>
// - serde rename_all attribute: <https://serde.rs/container-attrs.html#rename_all>

use serde::{Deserialize, Serialize};

/// All audio codec options supported by GAMDL's `--song-codec` flag.
///
/// These codecs correspond to the stream types available on Apple Music.
/// Listed in the order recommended for the default fallback chain (highest
/// quality first). The fallback chain is configured in `AppSettings::music_fallback_chain`
/// (see `settings.rs`) and controls automatic retry with a lower-quality codec
/// when the preferred one is unavailable for a given track.
///
/// ## Codec categories
///
/// | Category      | Variants                                             | Typical use case                |
/// |---------------|------------------------------------------------------|---------------------------------|
/// | Lossless      | `Alac`                                               | Audiophiles, archival           |
/// | Spatial/Atmos | `Atmos`, `Ac3`                                       | Surround sound systems          |
/// | AAC (standard)| `Aac`, `AacLegacy`, `AacBinaural`                    | General listening                |
/// | AAC-HE        | `AacHe`, `AacHeLegacy`, `AacHeBinaural`, etc.        | Low bandwidth / experimental    |
///
/// ## Serialization
///
/// `#[serde(rename_all = "kebab-case")]` means `AacBinaural` serializes to
/// `"aac-binaural"` in JSON -- matching both the GAMDL CLI flag values and
/// the frontend's expectation. See <https://serde.rs/container-attrs.html#rename_all>.
///
/// ## Reference
///
/// - GAMDL `--song-codec` flag: <https://github.com/glomatico/gamdl#usage>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SongCodec {
    /// Apple Lossless Audio Codec (ALAC) -- the highest-quality option.
    /// Delivers up to 24-bit/192 kHz lossless audio. Files are larger but
    /// bit-perfect. Requires Apple Music lossless tier. This is the default
    /// codec in `AppSettings` because the project brief prioritises quality.
    Alac,

    /// Dolby Atmos spatial audio stream. Produces immersive multi-channel
    /// audio encoded with Dolby's object-based format. Note: reliable access
    /// typically requires the wrapper/amdecrypt authentication pathway
    /// (`GamdlOptions::use_wrapper`).
    Atmos,

    /// Dolby Digital (AC-3) codec. A legacy surround-sound format that is
    /// widely supported by home theatre receivers. Lower quality than Atmos
    /// but broader hardware compatibility.
    Ac3,

    /// AAC at 256 kbps with Apple's binaural spatial processing applied.
    /// Simulates surround sound over standard stereo headphones using
    /// head-related transfer functions (HRTF).
    AacBinaural,

    /// Standard AAC (Advanced Audio Coding) at 256 kbps, sampled at up to
    /// 48 kHz. This is the default lossy codec Apple Music uses for streaming
    /// and is a good balance of quality and file size.
    Aac,

    /// Legacy AAC at 256 kbps, capped at 44.1 kHz sample rate. Provided
    /// for maximum compatibility with older devices and players that do
    /// not support 48 kHz AAC.
    AacLegacy,

    /// AAC High Efficiency (HE-AAC) legacy variant at 64 kbps / 44.1 kHz.
    /// Uses Spectral Band Replication (SBR) to achieve acceptable quality
    /// at very low bitrates. Primarily useful for bandwidth-constrained use.
    AacHeLegacy,

    /// AAC-HE (High Efficiency) -- experimental variant. Not widely tested;
    /// may not be available for all tracks. Use with caution.
    AacHe,

    /// AAC downmix variant (experimental). Folds surround channels down
    /// to stereo. Useful when the source is multi-channel but the listener
    /// only has stereo playback.
    AacDownmix,

    /// AAC-HE with binaural spatial processing (experimental). Combines
    /// the low-bitrate HE-AAC codec with Apple's HRTF binaural rendering.
    AacHeBinaural,

    /// AAC-HE downmix variant (experimental). Combines HE-AAC encoding
    /// with a stereo downmix of multi-channel sources.
    AacHeDownmix,
}

impl SongCodec {
    /// Converts the enum variant to the exact CLI string that the GAMDL
    /// Python CLI expects as the argument to `--song-codec`.
    ///
    /// These strings are defined in GAMDL's source at
    /// <https://github.com/glomatico/gamdl> and must be kept in sync
    /// whenever GAMDL adds or renames codec identifiers.
    ///
    /// Note: although serde's `rename_all = "kebab-case"` produces
    /// identical strings for JSON serialization, we maintain an explicit
    /// mapping here so that CLI generation is decoupled from serde config.
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
    ///
    /// These labels are shown in the React frontend's codec selection
    /// dropdown (see `src/components/settings/AudioQuality.tsx`).
    /// They include the bitrate and sample-rate characteristics so the
    /// user can make an informed choice without needing to look up the
    /// codec specifications.
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

/// Video resolution options for GAMDL's `--music-video-resolution` flag.
///
/// Listed from highest to lowest quality. Resolutions above 1080p require
/// the H.265 (HEVC) codec; lower resolutions are available with H.264 (AVC).
/// The video codec priority is controlled separately via
/// `GamdlOptions::music_video_codec_priority`.
///
/// The fallback chain in `AppSettings::video_fallback_chain` (see `settings.rs`)
/// tries these resolutions in descending order when the preferred resolution
/// is not available for a given music video.
///
/// ## Serialization
///
/// Each variant uses `#[serde(rename = "...")]` to produce the exact string
/// GAMDL expects (e.g., `"2160p"`) because serde's `rename_all = "lowercase"`
/// would yield `"p2160"` instead.
///
/// ## Reference
///
/// - GAMDL `--music-video-resolution` flag: <https://github.com/glomatico/gamdl#usage>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoResolution {
    /// 4K Ultra HD (2160p / 3840x2160). Requires H.265 codec. Produces the
    /// highest quality video output but also the largest file sizes.
    #[serde(rename = "2160p")]
    P2160,

    /// Quad HD (1440p / 2560x1440). Requires H.265 codec. A middle ground
    /// between 4K and Full HD.
    #[serde(rename = "1440p")]
    P1440,

    /// Full HD (1080p / 1920x1080). Available with both H.264 and H.265.
    /// This is the highest resolution that H.264 supports on Apple Music.
    #[serde(rename = "1080p")]
    P1080,

    /// HD (720p / 1280x720). H.264 only. Standard HD quality suitable for
    /// most screens.
    #[serde(rename = "720p")]
    P720,

    /// qHD (540p / 960x540). H.264 only. A step below standard HD.
    #[serde(rename = "540p")]
    P540,

    /// Standard definition (480p / 854x480). H.264 only. DVD-equivalent
    /// quality.
    #[serde(rename = "480p")]
    P480,

    /// Low definition (360p / 640x360). H.264 only. Suitable for very
    /// small screens or bandwidth-constrained situations.
    #[serde(rename = "360p")]
    P360,

    /// Lowest quality (240p / 426x240). H.264 only. Minimal bandwidth
    /// usage; only useful for previewing content.
    #[serde(rename = "240p")]
    P240,
}

impl VideoResolution {
    /// Converts to the CLI string GAMDL expects for `--music-video-resolution`.
    ///
    /// The returned value (e.g., `"1080p"`) is passed directly as the argument
    /// to the GAMDL subprocess. These strings are identical to the serde
    /// rename values but maintained explicitly for the same decoupling reason
    /// as `SongCodec::to_cli_string()`.
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

/// Synced lyrics format options for GAMDL's `--synced-lyrics-format` flag.
///
/// GAMDL can download time-synced lyrics alongside audio. The format
/// controls how those lyrics are stored on disk. The default in
/// `AppSettings` is `Lrc` for songs and `Ttml` for music videos
/// (the video download path overrides this at download time).
///
/// ## Reference
///
/// - GAMDL lyrics options: <https://github.com/glomatico/gamdl#usage>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsFormat {
    /// LRC format -- the most common timestamped lyrics format, widely
    /// supported by music players (foobar2000, MusicBee, etc.). Each line
    /// has a `[mm:ss.xx]` timestamp prefix. Default for song downloads.
    Lrc,

    /// SRT (SubRip) subtitle format. Numbered entries with
    /// `HH:MM:SS,mmm --> HH:MM:SS,mmm` timestamps. More common in
    /// video contexts; included here for users who prefer SRT tooling.
    Srt,

    /// TTML (Timed Text Markup Language) -- an XML-based subtitle format
    /// standardised by the W3C. Apple Music natively provides lyrics in
    /// TTML, so this option downloads the raw format without conversion.
    /// Default for music video downloads.
    Ttml,
}

impl LyricsFormat {
    /// Converts to the CLI string GAMDL expects for `--synced-lyrics-format`.
    pub fn to_cli_string(&self) -> &str {
        match self {
            LyricsFormat::Lrc => "lrc",
            LyricsFormat::Srt => "srt",
            LyricsFormat::Ttml => "ttml",
        }
    }
}

/// Cover art image format options for GAMDL's `--cover-format` flag.
///
/// Controls the format of the album artwork saved alongside downloads.
/// The default in `AppSettings` is `Raw` (original quality), matching
/// the project brief's preference for maximum fidelity.
///
/// ## Reference
///
/// - GAMDL `--cover-format` flag: <https://github.com/glomatico/gamdl#usage>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverFormat {
    /// JPEG format -- lossy compression, smaller file size (~100-300 KB
    /// for a 1200x1200 image). Good default for space-conscious users.
    Jpg,

    /// PNG format -- lossless compression, larger file size (~1-3 MB).
    /// Preserves every pixel but requires more storage.
    Png,

    /// Raw format -- downloads the artwork in whatever format Apple Music
    /// serves (typically JPEG at very high quality). No conversion is
    /// applied. This is the project default because it preserves the
    /// original artwork fidelity.
    Raw,
}

impl CoverFormat {
    /// Converts to the CLI string GAMDL expects for `--cover-format`.
    pub fn to_cli_string(&self) -> &str {
        match self {
            CoverFormat::Jpg => "jpg",
            CoverFormat::Png => "png",
            CoverFormat::Raw => "raw",
        }
    }
}

/// Download mode options for GAMDL's `--download-mode` flag.
///
/// Controls which external tool GAMDL uses to fetch HLS/DASH streams
/// from Apple Music's CDN. The choice affects download speed, reliability,
/// and which optional dependencies are required.
///
/// ## Reference
///
/// - yt-dlp: <https://github.com/yt-dlp/yt-dlp>
/// - N_m3u8DL-RE: <https://github.com/nilaoda/N_m3u8DL-RE>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadMode {
    /// Use yt-dlp for downloading. This is the default and most compatible
    /// option. yt-dlp is a Python-based tool that handles HLS stream
    /// downloading and is installed automatically as a GAMDL dependency.
    /// See `DependencyInfo` in `dependency.rs` for installation tracking.
    Ytdlp,

    /// Use N_m3u8DL-RE for downloading. A compiled binary alternative that
    /// can be faster than yt-dlp for HLS streams. Requires separate
    /// installation (tracked as an optional dependency in `dependency.rs`).
    Nm3u8dlre,
}

/// Remux mode options for GAMDL's `--remux-mode` flag.
///
/// After downloading encrypted stream segments, GAMDL decrypts and remuxes
/// them into the final container format. This enum controls which tool
/// performs that remuxing step.
///
/// ## Reference
///
/// - FFmpeg: <https://ffmpeg.org/>
/// - MP4Box (GPAC): <https://github.com/gpac/gpac/wiki/MP4Box>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemuxMode {
    /// Use FFmpeg for remuxing (default). FFmpeg is a required dependency
    /// (see `dependency.rs`) and handles both audio and video remuxing
    /// reliably. It is also used for format conversion when needed.
    Ffmpeg,

    /// Use MP4Box (from GPAC) for remuxing. An alternative to FFmpeg that
    /// some users prefer for MP4 container manipulation. MP4Box is tracked
    /// as an optional dependency in `dependency.rs`.
    Mp4box,
}

/// Log level options for GAMDL's `--log-level` flag.
///
/// Controls the verbosity of GAMDL's stdout/stderr output, which the
/// download manager in `commands/download.rs` parses for progress events.
/// Higher verbosity levels produce more output and can slow down parsing.
///
/// ## Serialization
///
/// `#[serde(rename_all = "UPPERCASE")]` ensures these serialize to
/// `"DEBUG"`, `"INFO"`, etc. -- matching Python's standard logging levels
/// that GAMDL uses internally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    /// Most verbose. Logs every HTTP request, decryption step, and internal
    /// state change. Useful for troubleshooting download failures.
    Debug,

    /// Standard operational messages. Logs track names, progress, and
    /// completion. This is the recommended level for normal use.
    Info,

    /// Only logs warnings and errors. Suppresses normal progress output.
    Warning,

    /// Only logs fatal errors. Minimal output.
    Error,
}

/// Complete set of GAMDL CLI options.
///
/// This struct is the central data structure for constructing GAMDL CLI
/// invocations. It maps 1:1 to the flags and arguments that the `gamdl`
/// Python CLI accepts on the command line.
///
/// ## Why all fields are `Option<T>`
///
/// Every field is `Option` to support a two-layer configuration model:
///
/// 1. **Global settings** (`AppSettings` in `settings.rs`) -- the user's
///    default preferences. When converting `AppSettings` into a
///    `GamdlOptions`, all configured fields become `Some(...)`.
/// 2. **Per-download overrides** (`DownloadRequest::options` in `download.rs`)
///    -- the user can tweak individual options for a specific download. Only
///    the overridden fields are `Some(...)`; the rest are `None`, meaning
///    "inherit from global settings".
///
/// Before spawning the GAMDL subprocess, the download manager merges the
/// per-download overrides on top of the global options (per-download wins),
/// then calls `to_cli_args()` on the merged result.
///
/// ## Serialization
///
/// The struct derives both `Serialize` and `Deserialize` via serde
/// (<https://docs.rs/serde/latest/serde/>) so it can be:
/// - Persisted as part of `AppSettings` JSON.
/// - Passed over the Tauri IPC bridge to/from the React frontend.
/// - Included in `DownloadRequest` payloads.
///
/// `#[derive(Default)]` initializes all fields to `None`, which is the
/// correct starting state for a blank per-download override.
///
/// ## Reference
///
/// - GAMDL CLI usage and all flags: <https://github.com/glomatico/gamdl#usage>
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
    /// Only fields that are `Some(...)` generate CLI flags. `None` fields
    /// are silently skipped, allowing GAMDL to use its own built-in defaults
    /// for those options. This design supports the two-layer merge strategy
    /// described in the struct-level documentation above.
    ///
    /// ## Mapping rules
    ///
    /// | Rust type                | CLI pattern                        | Example                                 |
    /// |--------------------------|------------------------------------|------------------------------------------|
    /// | `Option<SomeEnum>`       | `--flag <enum.to_cli_string()>`    | `--song-codec alac`                      |
    /// | `Option<String>`         | `--flag <value>`                   | `--language en-US`                       |
    /// | `Option<u32>`            | `--flag <value.to_string()>`       | `--truncate 200`                         |
    /// | `Option<bool>` = `true`  | `--flag` (presence = enabled)      | `--overwrite`                            |
    /// | `Option<bool>` = `false` | *(omitted entirely)*               | *(GAMDL's default is used)*              |
    ///
    /// The returned `Vec<String>` is passed directly to
    /// `std::process::Command::args()` when spawning the GAMDL subprocess.
    ///
    /// ## Reference
    ///
    /// - `std::process::Command::args`: <https://doc.rust-lang.org/std/process/struct.Command.html#method.args>
    pub fn to_cli_args(&self) -> Vec<String> {
        // Pre-allocate with a reasonable capacity to avoid frequent reallocation.
        // Most invocations produce 10-30 arguments.
        let mut args = Vec::new();

        // --- Audio Quality ---
        // Enum-valued option: push the flag name, then the CLI string representation.
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
        // Boolean flag pattern: only emit the flag when the value is explicitly
        // `Some(true)`. `Some(false)` and `None` both result in omission,
        // meaning GAMDL uses its default behavior (music video skip enabled).
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
        // Special formatting: GAMDL expects cover size as "WIDTHxHEIGHT" but
        // we store a single u32 because cover art is always square. The
        // format!() call duplicates the value to produce e.g. "1200x1200".
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
        // Inline match: for enums with only two variants and trivial string
        // mappings, we use an inline match instead of calling a to_cli_string()
        // method. This keeps the CLI string right next to the flag name for
        // easy verification against GAMDL's docs.
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
        // Log level uses Python's standard level names in UPPERCASE.
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
        // When set, GAMDL ignores its own ~/.gamdl/config.json. We typically
        // enable this so that the GUI's settings are the sole source of truth
        // and do not conflict with a user's pre-existing GAMDL config.
        if self.no_config_file == Some(true) {
            args.push("--no-config-file".to_string());
        }

        args
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // SongCodec::to_cli_string
    // ----------------------------------------------------------

    #[test]
    fn song_codec_cli_strings() {
        assert_eq!(SongCodec::Alac.to_cli_string(), "alac");
        assert_eq!(SongCodec::Atmos.to_cli_string(), "atmos");
        assert_eq!(SongCodec::Ac3.to_cli_string(), "ac3");
        assert_eq!(SongCodec::AacBinaural.to_cli_string(), "aac-binaural");
        assert_eq!(SongCodec::Aac.to_cli_string(), "aac");
        assert_eq!(SongCodec::AacLegacy.to_cli_string(), "aac-legacy");
        assert_eq!(SongCodec::AacHeLegacy.to_cli_string(), "aac-he-legacy");
        assert_eq!(SongCodec::AacHe.to_cli_string(), "aac-he");
        assert_eq!(SongCodec::AacDownmix.to_cli_string(), "aac-downmix");
        assert_eq!(SongCodec::AacHeBinaural.to_cli_string(), "aac-he-binaural");
        assert_eq!(SongCodec::AacHeDownmix.to_cli_string(), "aac-he-downmix");
    }

    // ----------------------------------------------------------
    // VideoResolution::to_cli_string
    // ----------------------------------------------------------

    #[test]
    fn video_resolution_cli_strings() {
        assert_eq!(VideoResolution::P2160.to_cli_string(), "2160p");
        assert_eq!(VideoResolution::P1440.to_cli_string(), "1440p");
        assert_eq!(VideoResolution::P1080.to_cli_string(), "1080p");
        assert_eq!(VideoResolution::P720.to_cli_string(), "720p");
        assert_eq!(VideoResolution::P540.to_cli_string(), "540p");
        assert_eq!(VideoResolution::P480.to_cli_string(), "480p");
        assert_eq!(VideoResolution::P360.to_cli_string(), "360p");
        assert_eq!(VideoResolution::P240.to_cli_string(), "240p");
    }

    // ----------------------------------------------------------
    // LyricsFormat::to_cli_string
    // ----------------------------------------------------------

    #[test]
    fn lyrics_format_cli_strings() {
        assert_eq!(LyricsFormat::Lrc.to_cli_string(), "lrc");
        assert_eq!(LyricsFormat::Srt.to_cli_string(), "srt");
        assert_eq!(LyricsFormat::Ttml.to_cli_string(), "ttml");
    }

    // ----------------------------------------------------------
    // CoverFormat::to_cli_string
    // ----------------------------------------------------------

    #[test]
    fn cover_format_cli_strings() {
        assert_eq!(CoverFormat::Jpg.to_cli_string(), "jpg");
        assert_eq!(CoverFormat::Png.to_cli_string(), "png");
        assert_eq!(CoverFormat::Raw.to_cli_string(), "raw");
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- empty (all None)
    // ----------------------------------------------------------

    #[test]
    fn empty_options_produce_no_args() {
        let options = GamdlOptions::default();
        assert!(options.to_cli_args().is_empty());
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- enum-valued options
    // ----------------------------------------------------------

    #[test]
    fn song_codec_option() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Alac),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--song-codec", "alac"]);
    }

    #[test]
    fn video_resolution_option() {
        let options = GamdlOptions {
            music_video_resolution: Some(VideoResolution::P1080),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--music-video-resolution", "1080p"]);
    }

    #[test]
    fn lyrics_format_option() {
        let options = GamdlOptions {
            synced_lyrics_format: Some(LyricsFormat::Ttml),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--synced-lyrics-format", "ttml"]);
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- boolean flags
    // ----------------------------------------------------------

    #[test]
    fn boolean_true_emits_flag() {
        let options = GamdlOptions {
            overwrite: Some(true),
            ..Default::default()
        };
        assert!(options.to_cli_args().contains(&"--overwrite".to_string()));
    }

    #[test]
    fn boolean_false_omits_flag() {
        let options = GamdlOptions {
            overwrite: Some(false),
            ..Default::default()
        };
        assert!(!options.to_cli_args().contains(&"--overwrite".to_string()));
    }

    #[test]
    fn boolean_none_omits_flag() {
        let options = GamdlOptions {
            overwrite: None,
            ..Default::default()
        };
        assert!(!options.to_cli_args().contains(&"--overwrite".to_string()));
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- cover size formatting
    // ----------------------------------------------------------

    #[test]
    fn cover_size_formatted_as_wxh() {
        let options = GamdlOptions {
            cover_size: Some(1200),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--cover-size".to_string()));
        assert!(args.contains(&"1200x1200".to_string()));
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- string-valued options
    // ----------------------------------------------------------

    #[test]
    fn output_path_option() {
        let options = GamdlOptions {
            output_path: Some("/tmp/music".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--output-path", "/tmp/music"]);
    }

    #[test]
    fn language_option() {
        let options = GamdlOptions {
            language: Some("ja-JP".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--language", "ja-JP"]);
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- mode enums
    // ----------------------------------------------------------

    #[test]
    fn download_mode_ytdlp() {
        let options = GamdlOptions {
            download_mode: Some(DownloadMode::Ytdlp),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--download-mode", "ytdlp"]);
    }

    #[test]
    fn download_mode_nm3u8dlre() {
        let options = GamdlOptions {
            download_mode: Some(DownloadMode::Nm3u8dlre),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--download-mode", "nm3u8dlre"]);
    }

    #[test]
    fn log_level_debug() {
        let options = GamdlOptions {
            log_level: Some(LogLevel::Debug),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--log-level", "DEBUG"]);
    }

    // ----------------------------------------------------------
    // GamdlOptions::to_cli_args -- multiple options combined
    // ----------------------------------------------------------

    #[test]
    fn multiple_options_combined() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Aac),
            save_cover: Some(true),
            cover_format: Some(CoverFormat::Jpg),
            overwrite: Some(true),
            language: Some("en-US".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();

        // Verify all expected flags are present
        assert!(args.contains(&"--song-codec".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(args.contains(&"--save-cover".to_string()));
        assert!(args.contains(&"--cover-format".to_string()));
        assert!(args.contains(&"jpg".to_string()));
        assert!(args.contains(&"--overwrite".to_string()));
        assert!(args.contains(&"--language".to_string()));
        assert!(args.contains(&"en-US".to_string()));
    }

    // ----------------------------------------------------------
    // Serde roundtrip for SongCodec
    // ----------------------------------------------------------

    #[test]
    fn song_codec_serde_roundtrip() {
        let codec = SongCodec::AacBinaural;
        let json = serde_json::to_string(&codec).unwrap();
        assert_eq!(json, "\"aac-binaural\"");

        let deserialized: SongCodec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, codec);
    }

    #[test]
    fn video_resolution_serde_roundtrip() {
        let res = VideoResolution::P1080;
        let json = serde_json::to_string(&res).unwrap();
        assert_eq!(json, "\"1080p\"");

        let deserialized: VideoResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, res);
    }
}
