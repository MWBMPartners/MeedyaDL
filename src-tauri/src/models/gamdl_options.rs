// Copyright (c) 2026 MeedyaSuite
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SongCodec {
    /// Apple Lossless Audio Codec (ALAC) -- the highest-quality option.
    /// Delivers up to 24-bit/192 kHz lossless audio. Files are larger but
    /// bit-perfect. Requires Apple Music lossless tier. This is the default
    /// codec in `AppSettings` because the project brief prioritises quality.
    Alac,

    /// Dolby Atmos spatial audio stream. Produces immersive multi-channel
    /// audio encoded with Dolby's object-based format. Note: reliable access
    /// typically requires the wrapper authentication pathway
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
    #[must_use]
    pub const fn to_cli_string(&self) -> &str {
        match self {
            Self::Alac => "alac",
            Self::Atmos => "atmos",
            Self::Ac3 => "ac3",
            Self::AacBinaural => "aac-binaural",
            Self::Aac => "aac",
            Self::AacLegacy => "aac-legacy",
            Self::AacHeLegacy => "aac-he-legacy",
            Self::AacHe => "aac-he",
            Self::AacDownmix => "aac-downmix",
            Self::AacHeBinaural => "aac-he-binaural",
            Self::AacHeDownmix => "aac-he-downmix",
        }
    }

    /// Parse a CLI string (e.g., `"atmos"`, `"aac-binaural"`) back into a
    /// `SongCodec` variant. Returns `None` for unrecognized strings.
    /// Inverse of `to_cli_string()`.
    ///
    /// **GAMDL v3.6 (#853):** accepts both the pre-3.6 strings
    /// (`aac-legacy`, `aac-he-legacy`) AND the v3.6+ renames
    /// (`aac-web`, `aac-he-web`) — they describe the same underlying
    /// codec on different upstream releases. This lets us round-trip
    /// settings files saved by either version.
    #[must_use]
    pub fn from_cli_string(s: &str) -> Option<Self> {
        match s {
            "alac" => Some(Self::Alac),
            "atmos" => Some(Self::Atmos),
            "ac3" => Some(Self::Ac3),
            "aac-binaural" => Some(Self::AacBinaural),
            "aac" => Some(Self::Aac),
            // GAMDL <3.6 names + >=3.6 renames map to the same Rust variant.
            "aac-legacy" | "aac-web" => Some(Self::AacLegacy),
            "aac-he-legacy" | "aac-he-web" => Some(Self::AacHeLegacy),
            "aac-he" => Some(Self::AacHe),
            "aac-downmix" => Some(Self::AacDownmix),
            "aac-he-binaural" => Some(Self::AacHeBinaural),
            "aac-he-downmix" => Some(Self::AacHeDownmix),
            _ => None,
        }
    }

    /// Capability-aware CLI string (#853).
    ///
    /// Identical to [`Self::to_cli_string`] for every codec **except**
    /// `AacLegacy` / `AacHeLegacy`, which GAMDL v3.6 renamed to
    /// `aac-web` / `aac-he-web`. On runtimes ≥ v3.6 this returns the
    /// new name; on older runtimes (and when the version cache hasn't
    /// been populated yet — `gamdl_capabilities::supports` returns
    /// `false` in that case) it returns the historical name.
    ///
    /// Use this at every CLI / INI emission site that hands a codec
    /// identifier to GAMDL. `to_cli_string()` stays `const fn` for
    /// display, history, tag-writing, and other non-runtime
    /// consumers.
    #[must_use]
    pub fn to_runtime_cli_string(&self) -> &'static str {
        use crate::services::gamdl_capabilities::{supports, GamdlFeature};
        // List each variant explicitly so we return `&'static str` (all
        // arms are string literals). Borrowing the result of
        // `to_cli_string(&self)` would only give us `&'a str` tied to
        // `self`'s lifetime, which doesn't satisfy callers that need
        // `&'static`.
        match self {
            Self::Alac => "alac",
            Self::Atmos => "atmos",
            Self::Ac3 => "ac3",
            Self::AacBinaural => "aac-binaural",
            Self::Aac => "aac",
            Self::AacLegacy if supports(GamdlFeature::AacWebCodecRename) => "aac-web",
            Self::AacLegacy => "aac-legacy",
            Self::AacHeLegacy if supports(GamdlFeature::AacWebCodecRename) => "aac-he-web",
            Self::AacHeLegacy => "aac-he-legacy",
            Self::AacHe => "aac-he",
            Self::AacDownmix => "aac-downmix",
            Self::AacHeBinaural => "aac-he-binaural",
            Self::AacHeDownmix => "aac-he-downmix",
        }
    }

    /// Returns `true` if this codec requires wrapper authentication for
    /// reliable per-track availability queries. Atmos (E-AC-3 JOC) and
    /// Dolby Digital (AC-3) use spatial audio API endpoints that don't
    /// properly fall back per-track without wrapper auth. When these codecs
    /// lead a native priority chain (`--song-codec-priority`) and wrapper
    /// is not enabled, GAMDL may skip tracks instead of trying later codecs.
    ///
    /// Used by the gap-fill mechanism in `download_queue.rs` to build a
    /// fallback chain that excludes these codecs for skipped tracks.
    #[must_use]
    pub const fn is_wrapper_dependent(&self) -> bool {
        matches!(self, Self::Atmos | Self::Ac3)
    }

    /// Returns the Apple Music `audioTraits` value that must be present
    /// on a track for this codec to be downloadable. Used by the
    /// companion planner (#504) to skip tiers whose codec the API has
    /// already told us isn't offered for the track, instead of letting
    /// GAMDL crash with `NoneType.audio_track`.
    ///
    /// Returns `None` for codecs that are derived from another stream
    /// (binaural / downmix / HE variants) — those are computed on top
    /// of whatever the track *does* have, so they can't be filtered
    /// purely from `audioTraits` and we still hand them to GAMDL.
    ///
    /// Mapping derived from Apple Music API field values observed on
    /// catalog responses for tracks across the codec matrix.
    #[must_use]
    pub const fn required_audio_trait(&self) -> Option<&'static str> {
        match self {
            Self::Alac => Some("lossless"),
            Self::Atmos => Some("atmos"),
            Self::Ac3 => Some("dolby-digital"),
            Self::Aac | Self::AacLegacy => Some("lossy-stereo"),
            // Derived / rendered codecs — gated by their source stream
            // (binaural and downmix are computed from the spatial mix;
            // HE variants share the lossy stereo source). Returning
            // None means "don't pre-skip on traits".
            Self::AacBinaural
            | Self::AacHe
            | Self::AacHeLegacy
            | Self::AacDownmix
            | Self::AacHeBinaural
            | Self::AacHeDownmix => None,
        }
    }

    /// Human-readable display name for the UI dropdown/selector.
    ///
    /// These labels are shown in the React frontend's codec selection
    /// dropdown (see `src/components/settings/AudioQuality.tsx`).
    /// They include the bitrate and sample-rate characteristics so the
    /// user can make an informed choice without needing to look up the
    /// codec specifications.
    #[must_use]
    pub const fn display_name(&self) -> &str {
        match self {
            Self::Alac => "Lossless (ALAC) (Experimental)",
            Self::Atmos => "Dolby Atmos (Experimental)",
            Self::Ac3 => "Dolby Digital (AC3) (Experimental)",
            Self::AacBinaural => "AAC (256kbps) Binaural (Experimental)",
            Self::Aac => "AAC (256kbps at up to 48kHz) (Experimental)",
            Self::AacLegacy => "AAC Legacy (256kbps at up to 44.1kHz)",
            Self::AacHeLegacy => "AAC-HE Legacy (64kbps)",
            Self::AacHe => "AAC-HE (Experimental)",
            Self::AacDownmix => "AAC Downmix (Experimental)",
            Self::AacHeBinaural => "AAC-HE Binaural (Experimental)",
            Self::AacHeDownmix => "AAC-HE Downmix (Experimental)",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[must_use]
    pub const fn to_cli_string(&self) -> &str {
        match self {
            Self::P2160 => "2160p",
            Self::P1440 => "1440p",
            Self::P1080 => "1080p",
            Self::P720 => "720p",
            Self::P540 => "540p",
            Self::P480 => "480p",
            Self::P360 => "360p",
            Self::P240 => "240p",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsFormat {
    /// LRC format -- the most common timestamped lyrics format, widely
    /// supported by music players (foobar2000, `MusicBee`, etc.). Each line
    /// has a `[mm:ss.xx]` timestamp prefix. Default for song downloads.
    Lrc,

    /// SRT (`SubRip`) subtitle format. Numbered entries with
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
    #[must_use]
    pub const fn to_cli_string(&self) -> &str {
        match self {
            Self::Lrc => "lrc",
            Self::Srt => "srt",
            Self::Ttml => "ttml",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[must_use]
    pub const fn to_cli_string(&self) -> &str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::Raw => "raw",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Remux mode options for GAMDL's `--music-video-remux-mode` flag.
///
/// After downloading encrypted stream segments, GAMDL decrypts and remuxes
/// them into the final container format. This enum controls which tool
/// performs that remuxing step.
///
/// ## Reference
///
/// - `FFmpeg`: <https://ffmpeg.org/>
/// - `MP4Box` (GPAC): <https://github.com/gpac/gpac/wiki/MP4Box>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemuxMode {
    /// Use `FFmpeg` for remuxing (default). `FFmpeg` is a required dependency
    /// (see `dependency.rs`) and handles both audio and video remuxing
    /// reliably. It is also used for format conversion when needed.
    Ffmpeg,

    /// Use `MP4Box` (from GPAC) for remuxing. An alternative to `FFmpeg` that
    /// some users prefer for MP4 container manipulation. `MP4Box` is tracked
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Artist content auto-selection mode for GAMDL's `--artist-auto-select` flag.
///
/// Controls what content is automatically downloaded when the user provides
/// an artist URL instead of a specific album/song URL. New in GAMDL 2.9.1.
///
/// Without this flag, GAMDL prompts interactively when given an artist URL,
/// which doesn't work in a subprocess context. This enum allows the user to
/// pre-select the content type in settings.
///
/// ## Serialization
///
/// `#[serde(rename_all = "kebab-case")]` maps `MainAlbums` to `"main-albums"`, etc.
///
/// ## Reference
///
/// - GAMDL `--artist-auto-select` flag: <https://github.com/glomatico/gamdl#usage>
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtistAutoSelect {
    /// Download main studio albums only.
    MainAlbums,
    /// Download compilation albums only.
    CompilationAlbums,
    /// Download live albums only.
    LiveAlbums,
    /// Download singles and EPs only.
    SinglesEps,
    /// Download all album types (main, compilation, live, singles).
    AllAlbums,
    /// Download the artist's top songs.
    TopSongs,
    /// Download the artist's music videos.
    MusicVideos,
}

impl ArtistAutoSelect {
    /// Converts the enum variant to the exact CLI string for GAMDL's
    /// `--artist-auto-select` flag.
    #[must_use]
    pub const fn to_cli_string(&self) -> &str {
        match self {
            Self::MainAlbums => "main-albums",
            Self::CompilationAlbums => "compilation-albums",
            Self::LiveAlbums => "live-albums",
            Self::SinglesEps => "singles-eps",
            Self::AllAlbums => "all-albums",
            Self::TopSongs => "top-songs",
            Self::MusicVideos => "music-videos",
        }
    }

    /// Human-readable display name for the UI dropdown/selector.
    #[must_use]
    pub const fn display_name(&self) -> &str {
        match self {
            Self::MainAlbums => "Main Albums",
            Self::CompilationAlbums => "Compilation Albums",
            Self::LiveAlbums => "Live Albums",
            Self::SinglesEps => "Singles & EPs",
            Self::AllAlbums => "All Albums",
            Self::TopSongs => "Top Songs",
            Self::MusicVideos => "Music Videos",
        }
    }
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
    /// Audio codec for music downloads (used with GAMDL < 2.9.1's `--song-codec`)
    pub song_codec: Option<SongCodec>,

    /// Comma-separated codec priority list for GAMDL >= 2.9.1's
    /// `--song-codec-priority` flag. When set, GAMDL tries each codec
    /// in order within a single process, using the first that returns
    /// valid stream info. Built from the fallback chain at download time.
    /// Example: `"alac,atmos,ac3,aac-binaural,aac,aac-legacy"`
    pub song_codec_priority: Option<String>,

    // --- Video Quality ---
    /// Comma-separated codec priority for music videos (e.g., "h265,h264")
    pub music_video_codec_priority: Option<String>,
    /// Maximum video resolution
    pub music_video_resolution: Option<VideoResolution>,
    /// Video container format ("mp4" or "m4v")
    pub music_video_remux_format: Option<String>,
    /// Uploaded/post video quality ("best" or "ask").
    ///
    /// **State of the uploaded-video pipeline (#549):** this field is a
    /// pass-through CLI flag only. GAMDL ships `downloader_uploaded_video.py`
    /// and `interface_uploaded_video.py` for Apple Music's label/artist-uploaded
    /// videos (behind-the-scenes clips, live sessions, interviews — distinct
    /// from catalog music videos), but MeedyaDL does not:
    /// - detect uploaded-video URLs in `parse_apple_music_url` / the
    ///   frontend `detectContentType`,
    /// - route them through `download_music_video_by_url()` (so the
    ///   MV-safe `MV_NO_ALBUM_FOLDER_TEMPLATE` / `MV_NO_ALBUM_FILE_TEMPLATE`
    ///   in `download_queue.rs:2916, 2943` are NOT applied),
    /// - expose any UI surface for uploaded-video discovery.
    ///
    /// If an uploaded-video URL is somehow submitted (deep link,
    /// drag-drop, or direct IPC call) its tag shape — per the upstream
    /// `interface_uploaded_video.py` — is `{artist, date, title,
    /// title_id, storefront}` with no `album`, `disc`, `track`, or
    /// `album_artist`. That routes straight through GAMDL's `no_album_*`
    /// templates. Post-v3 migration the default is safe
    /// (`{artist}/Unknown Album/{title}`) but loses the `{title_id}`
    /// uniqueness guarantee — two same-artist uploaded videos sharing a
    /// title ("Live Session") collide. See #549 for the pipeline plan.
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
    /// Whether to use the wrapper authentication system
    pub use_wrapper: Option<bool>,
    /// Wrapper server URL
    pub wrapper_account_url: Option<String>,
    /// Decryption server address
    pub wrapper_decrypt_ip: Option<String>,
    /// m3u8 server address (host:port) used by GAMDL v3.1+ to fetch the HLS
    /// master playlist URL from the wrapper instead of Apple's API. Only
    /// emitted when the detected GAMDL release supports it (see
    /// `GamdlFeature::WrapperM3u8Ip`). Default on upstream: `127.0.0.1:20020`.
    ///
    /// GAMDL v3.6 (#853) removed this CLI option — emission is gated.
    pub wrapper_m3u8_ip: Option<String>,
    /// Wrapper-v2 HTTP base URL (#853). Emitted as `--wrapper-url <url>`
    /// only when the detected GAMDL release supports it
    /// (`GamdlFeature::WrapperUrl`, ≥3.6) — replaces the three v1 socket
    /// addresses above on that path. Default on upstream:
    /// `http://127.0.0.1` (port 80 implied).
    pub wrapper_url: Option<String>,

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
    /// Folder template for playlists (GAMDL v3.0+). Emission is gated by
    /// [`crate::services::gamdl_capabilities::GamdlFeature::PlaylistFolderTemplate`]
    /// — `--playlist-folder-template` does not exist on v2.9.x and would
    /// crash the subprocess with "no such option". See #618.
    pub playlist_folder_template: Option<String>,
    /// File template for single-disc albums
    pub single_disc_file_template: Option<String>,
    /// File template for multi-disc albums
    pub multi_disc_file_template: Option<String>,
    /// File template for non-album tracks
    pub no_album_file_template: Option<String>,
    /// Folder/file template for playlists
    pub playlist_file_template: Option<String>,

    // --- Tool Paths ---
    /// Path to `FFmpeg` binary
    pub ffmpeg_path: Option<String>,
    /// Path to mp4decrypt binary
    pub mp4decrypt_path: Option<String>,
    /// Path to `MP4Box` binary
    pub mp4box_path: Option<String>,
    /// Path to N_m3u8DL-RE binary
    pub nm3u8dlre_path: Option<String>,
    /// Path to .wvd (Widevine Device) file
    pub wvd_path: Option<String>,

    // --- Modes ---
    /// Download mode selection (yt-dlp or N_m3u8DL-RE)
    pub download_mode: Option<DownloadMode>,
    /// Remux mode selection (`FFmpeg` or `MP4Box`)
    pub remux_mode: Option<RemuxMode>,

    // --- Artist ---
    /// Auto-selection mode for artist URL downloads (GAMDL >= 2.9.1).
    /// Controls which content type is automatically downloaded when the
    /// user provides an artist URL.
    pub artist_auto_select: Option<ArtistAutoSelect>,

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
    /// Delegates to four helper methods (`audio_cli_args`, `video_cli_args`,
    /// `path_cli_args`, `flag_cli_args`) to keep each section under the
    /// clippy `too_many_lines` threshold.
    ///
    /// ## Reference
    ///
    /// - `std::process::Command::args`: <https://doc.rust-lang.org/std/process/struct.Command.html#method.args>
    #[must_use]
    pub fn to_cli_args(&self) -> Vec<String> {
        // Pre-allocate with a reasonable capacity to avoid frequent reallocation.
        // Most invocations produce 10-30 arguments.
        let mut args = Vec::new();

        // Collect arguments from each logical group of CLI options.
        // The ordering matches GAMDL's own help output for readability.
        args.extend(self.audio_cli_args());
        args.extend(self.video_cli_args());
        args.extend(self.path_cli_args());
        args.extend(self.flag_cli_args());

        args
    }

    /// Builds CLI arguments for audio quality, lyrics, and cover art options.
    ///
    /// Covers: `--song-codec-priority`, `--synced-lyrics-format`,
    /// `--no-synced-lyrics`, `--synced-lyrics-only`, `--save-cover`,
    /// `--cover-format`, `--cover-size`.
    fn audio_cli_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // --- Audio Quality ---
        // `--song-codec-priority` has been the only codec-selection flag in
        // GAMDL since v2.9.1 (the floor of our support window) — the legacy
        // `--song-codec` single-codec flag was removed in the 2.9.1 CLI
        // restructure and crashes Click with "No such option" on every
        // subsequent release (v2.9.1 → v3.2). See #614 for the full
        // cross-version verification.
        //
        // `song_codec_priority` wins when set; otherwise we promote the
        // scalar `song_codec` field into a one-element CSV (valid per
        // GAMDL's `Csv(SongCodec)` typing). This keeps `GamdlOptions`
        // backwards-compatible with callers that still set `song_codec`
        // while emitting a command line every supported GAMDL release
        // understands.
        let priority = self.song_codec_priority.clone().or_else(|| {
            self.song_codec
                .as_ref()
                .map(|c| c.to_runtime_cli_string().to_string())
        });
        if let Some(priority) = priority {
            args.push("--song-codec-priority".to_string());
            args.push(priority);
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
        // GAMDL expects --cover-size as a single integer (pixels).
        // Cover art is always square, so one dimension suffices.
        if let Some(size) = self.cover_size {
            args.push("--cover-size".to_string());
            args.push(size.to_string());
        }

        args
    }

    /// Builds CLI arguments for video quality and music video flags.
    ///
    /// Covers: `--music-video-codec-priority`, `--music-video-resolution`,
    /// `--music-video-remux-format`, `--uploaded-video-quality`,
    /// `--disable-music-video-skip`.
    fn video_cli_args(&self) -> Vec<String> {
        let mut args = Vec::new();

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
        // NOTE: `disable_music_video_skip` is kept as a field for potential
        // future use, but GAMDL does not expose a `--disable-music-video-skip`
        // CLI flag (as of v2.9.1). Passing it would cause an "unrecognized
        // option" error, so we intentionally do not emit it.

        args
    }

    /// Builds CLI arguments for output paths, authentication, templates,
    /// and external tool paths.
    ///
    /// Covers: `--output-path`, `--temp-path`, `--cookies-path`,
    /// `--wrapper-account-url`, `--wrapper-decrypt-ip`, folder/file templates,
    /// and binary paths (`--ffmpeg-path`, `--mp4decrypt-path`, etc.).
    fn path_cli_args(&self) -> Vec<String> {
        use crate::services::gamdl_capabilities::{supports, GamdlFeature};
        let mut args = Vec::new();

        // --- Output ---
        if let Some(ref path) = self.output_path {
            args.push("--output-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.temp_path {
            args.push("--temp-path".to_string());
            args.push(path.clone());
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
        // Wrapper authentication — capability-gated per #853.
        //
        // GAMDL 2.9.x – 3.5.x: three separate v1 socket addresses
        //   --wrapper-account-url <url>      (HTTP)
        //   --wrapper-decrypt-ip <host:port> (TCP)
        //   --wrapper-m3u8-ip <host:port>    (TCP, GAMDL 3.1+ only)
        //
        // GAMDL ≥ 3.6: a single HTTP base URL pointing at the wrapper-v2
        // daemon's REST API (replaces all three above)
        //   --wrapper-url <url>
        //
        // We emit EXACTLY ONE family per CLI invocation. Mixing would
        // either silently no-op (`cleanup_unknown_params()` on v3.0+
        // INI parsing) or crash Click with "no such option" on the
        // CLI parser. (Imports hoisted to fn top.)
        if supports(GamdlFeature::WrapperUrl) {
            // GAMDL ≥ 3.6 — wrapper-v2 single endpoint.
            if let Some(ref url) = self.wrapper_url {
                args.push("--wrapper-url".to_string());
                args.push(url.clone());
            }
        } else {
            // GAMDL ≤ 3.5.x — wrapper-v1 triple.
            if let Some(ref url) = self.wrapper_account_url {
                args.push("--wrapper-account-url".to_string());
                args.push(url.clone());
            }
            if let Some(ref ip) = self.wrapper_decrypt_ip {
                args.push("--wrapper-decrypt-ip".to_string());
                args.push(ip.clone());
            }
            // --wrapper-m3u8-ip is itself GAMDL v3.1+ only.
            if let Some(ref ip) = self.wrapper_m3u8_ip {
                if supports(GamdlFeature::WrapperM3u8Ip) {
                    args.push("--wrapper-m3u8-ip".to_string());
                    args.push(ip.clone());
                }
            }
        }

        // --- Metadata (string-valued) ---
        if let Some(ref lang) = self.language {
            args.push("--language".to_string());
            args.push(lang.clone());
        }
        if let Some(ref tags) = self.exclude_tags {
            args.push("--exclude-tags".to_string());
            args.push(tags.clone());
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
        // `--playlist-folder-template` is GAMDL v3.0+ only (#618). On
        // v2.9.x the flag does not exist and emission would crash the
        // subprocess with "no such option", so we gate it the same way
        // `wrapper_m3u8_ip` is gated in `path_cli_args` above.
        if let Some(ref t) = self.playlist_folder_template {
            if crate::services::gamdl_capabilities::supports(
                crate::services::gamdl_capabilities::GamdlFeature::PlaylistFolderTemplate,
            ) {
                args.push("--playlist-folder-template".to_string());
                args.push(t.clone());
            }
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
        //
        // GAMDL ≥ v3.6 (#853) dropped FFmpeg/MP4Box/mp4decrypt for native
        // muxing + decryption. The corresponding CLI options were
        // removed; passing any of them on v3.6 crashes Click with "no
        // such option". MeedyaDL still ships these binaries for its own
        // pipeline (FFmpeg → ReplayGain / BPM analysis; MP4Box +
        // mp4decrypt were only relied on by GAMDL itself), so emission
        // is purely conditional on the GAMDL release.
        let native_muxing = supports(GamdlFeature::NativeMuxing);
        if !native_muxing {
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
        }
        if let Some(ref path) = self.nm3u8dlre_path {
            args.push("--nm3u8dlre-path".to_string());
            args.push(path.clone());
        }
        if let Some(ref path) = self.wvd_path {
            args.push("--wvd-path".to_string());
            args.push(path.clone());
        }

        args
    }

    /// Builds CLI arguments for boolean flags, mode enums, and log level.
    ///
    /// Covers: `--overwrite`, `--use-wrapper`, `--use-album-date`,
    /// `--fetch-extra-tags`, `--download-mode`, `--remux-mode`, `--log-level`,
    /// `--no-exceptions`, `--save-playlist`, `--no-config-file`.
    fn flag_cli_args(&self) -> Vec<String> {
        use crate::services::gamdl_capabilities::{supports, GamdlFeature};
        let mut args = Vec::new();

        // --- Boolean flags ---
        if self.overwrite == Some(true) {
            args.push("--overwrite".to_string());
        }
        if self.use_wrapper == Some(true) {
            args.push("--use-wrapper".to_string());
        }
        if self.use_album_date == Some(true) {
            args.push("--use-album-date".to_string());
        }
        if self.fetch_extra_tags == Some(true) {
            args.push("--fetch-extra-tags".to_string());
        }

        // --- Modes ---
        // Inline match: for enums with only two variants and trivial string
        // mappings, we use an inline match instead of calling a to_cli_string()
        // method. This keeps the CLI string right next to the flag name for
        // easy verification against GAMDL's docs.
        if let Some(ref mode) = self.download_mode {
            args.push("--download-mode".to_string());
            args.push(
                match mode {
                    DownloadMode::Ytdlp => "ytdlp",
                    DownloadMode::Nm3u8dlre => "nm3u8dlre",
                }
                .to_string(),
            );
        }
        if let Some(ref mode) = self.remux_mode {
            // GAMDL ≥ v3.6 (#853) removed --music-video-remux-mode
            // alongside native muxing — there's only one remux
            // strategy on that release. Gate emission.
            if supports(GamdlFeature::MusicVideoRemuxMode) {
                args.push("--music-video-remux-mode".to_string());
                args.push(
                    match mode {
                        RemuxMode::Ffmpeg => "ffmpeg",
                        RemuxMode::Mp4box => "mp4box",
                    }
                    .to_string(),
                );
            }
        }

        // --- Other ---
        // Log level uses Python's standard level names in UPPERCASE.
        if let Some(ref level) = self.log_level {
            args.push("--log-level".to_string());
            args.push(
                match level {
                    LogLevel::Debug => "DEBUG",
                    LogLevel::Info => "INFO",
                    LogLevel::Warning => "WARNING",
                    LogLevel::Error => "ERROR",
                }
                .to_string(),
            );
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

        // --- Artist auto-select (GAMDL >= 2.9.1) ---
        if let Some(ref mode) = self.artist_auto_select {
            args.push("--artist-auto-select".to_string());
            args.push(mode.to_cli_string().to_string());
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
    // SongCodec::display_name
    // ----------------------------------------------------------

    #[test]
    fn song_codec_display_names() {
        // Non-experimental (reliable with cookie auth)
        assert_eq!(
            SongCodec::AacLegacy.display_name(),
            "AAC Legacy (256kbps at up to 44.1kHz)"
        );
        assert_eq!(
            SongCodec::AacHeLegacy.display_name(),
            "AAC-HE Legacy (64kbps)"
        );

        // Experimental (may fail without Wrapper service)
        assert_eq!(
            SongCodec::Alac.display_name(),
            "Lossless (ALAC) (Experimental)"
        );
        assert_eq!(
            SongCodec::Atmos.display_name(),
            "Dolby Atmos (Experimental)"
        );
        assert_eq!(
            SongCodec::Ac3.display_name(),
            "Dolby Digital (AC3) (Experimental)"
        );
        assert_eq!(
            SongCodec::AacBinaural.display_name(),
            "AAC (256kbps) Binaural (Experimental)"
        );
        assert_eq!(
            SongCodec::Aac.display_name(),
            "AAC (256kbps at up to 48kHz) (Experimental)"
        );
        assert_eq!(SongCodec::AacHe.display_name(), "AAC-HE (Experimental)");
        assert_eq!(
            SongCodec::AacDownmix.display_name(),
            "AAC Downmix (Experimental)"
        );
        assert_eq!(
            SongCodec::AacHeBinaural.display_name(),
            "AAC-HE Binaural (Experimental)"
        );
        assert_eq!(
            SongCodec::AacHeDownmix.display_name(),
            "AAC-HE Downmix (Experimental)"
        );
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

    /// `song_codec` alone still yields a working CLI — but via
    /// `--song-codec-priority <single-codec>`, not the removed-in-v2.9.1
    /// `--song-codec` flag. This exercises the `or_else` promotion branch
    /// in `audio_cli_args` (the fix for #614).
    #[test]
    fn song_codec_promotes_to_priority_csv() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Alac),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--song-codec-priority", "alac"]);
        assert!(!args.iter().any(|a| a == "--song-codec"));
    }

    /// Native priority chain takes precedence over the scalar `song_codec`.
    #[test]
    fn song_codec_priority_wins_over_scalar() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Alac),
            song_codec_priority: Some("atmos,alac,aac".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert_eq!(args, vec!["--song-codec-priority", "atmos,alac,aac"]);
    }

    /// Both-`None` emits no codec arg at all (existing invariant preserved).
    #[test]
    fn song_codec_both_none_emits_nothing() {
        let options = GamdlOptions::default();
        let args = options.to_cli_args();
        assert!(!args.iter().any(|a| a == "--song-codec"));
        assert!(!args.iter().any(|a| a == "--song-codec-priority"));
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
    fn cover_size_formatted_as_integer() {
        let options = GamdlOptions {
            cover_size: Some(1200),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--cover-size".to_string()));
        assert!(args.contains(&"1200".to_string()));
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

    #[test]
    fn wrapper_m3u8_ip_option() {
        // The --wrapper-m3u8-ip CLI flag is GAMDL v3.1 — v3.5.x only.
        // Pre-3.1 didn't recognise the flag at all; v3.6 removed it
        // alongside the wrapper-v2 single-endpoint redesign (#853).
        // Set the version cache to a supporting release so the gate
        // emits the flag.
        use crate::services::gamdl_capabilities::set_detected_version;
        set_detected_version(Some("3.5.2".to_string()));
        let options = GamdlOptions {
            wrapper_m3u8_ip: Some("127.0.0.1:20020".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        set_detected_version(None);
        assert_eq!(args, vec!["--wrapper-m3u8-ip", "127.0.0.1:20020"]);
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

        // Verify all expected flags are present.
        // Post-#614, the scalar `song_codec` is promoted to a one-element
        // `--song-codec-priority` CSV on every supported GAMDL release.
        assert!(args.contains(&"--song-codec-priority".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.iter().any(|a| a == "--song-codec"));
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

    // ----------------------------------------------------------
    // song_codec_priority takes precedence over song_codec
    // ----------------------------------------------------------

    #[test]
    fn song_codec_priority_takes_precedence() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Alac),
            song_codec_priority: Some("alac,aac,aac-legacy".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--song-codec-priority".to_string()));
        assert!(args.contains(&"alac,aac,aac-legacy".to_string()));
        assert!(!args.contains(&"--song-codec".to_string()));
    }

    /// When `song_codec_priority` is `None` the scalar `song_codec` value
    /// is promoted to a one-element `--song-codec-priority` CSV. The
    /// removed-in-v2.9.1 `--song-codec` flag must NEVER be emitted (#614).
    #[test]
    fn song_codec_promotes_when_priority_unset() {
        let options = GamdlOptions {
            song_codec: Some(SongCodec::Aac),
            song_codec_priority: None,
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--song-codec-priority".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.iter().any(|a| a == "--song-codec"));
    }

    #[test]
    fn neither_codec_field_produces_no_codec_args() {
        let options = GamdlOptions {
            song_codec: None,
            song_codec_priority: None,
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(!args.contains(&"--song-codec".to_string()));
        assert!(!args.contains(&"--song-codec-priority".to_string()));
    }

    // ----------------------------------------------------------
    // playlist_folder_template capability gate (#618)
    //
    // These tests mutate the process-global capability cache so they
    // share a `Mutex` to avoid interfering with each other (same
    // pattern as the `gamdl_capabilities` module's own tests).
    // ----------------------------------------------------------

    /// Shared lock for `playlist_folder_template` tests — see
    /// `gamdl_capabilities::tests::TEST_LOCK` for the pattern.
    static PLAYLIST_TEMPLATE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn playlist_folder_template_emitted_on_v30_plus() {
        let _guard = PLAYLIST_TEMPLATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::services::gamdl_capabilities::set_detected_version(Some("3.0".to_string()));
        let options = GamdlOptions {
            playlist_folder_template: Some("MyPlaylists/{playlist_artist}".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--playlist-folder-template".to_string()));
        assert!(args.contains(&"MyPlaylists/{playlist_artist}".to_string()));
        crate::services::gamdl_capabilities::set_detected_version(None);
    }

    #[test]
    fn playlist_folder_template_suppressed_on_v29x() {
        let _guard = PLAYLIST_TEMPLATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::services::gamdl_capabilities::set_detected_version(Some("2.9.3".to_string()));
        let options = GamdlOptions {
            playlist_folder_template: Some("MyPlaylists/{playlist_artist}".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(
            !args.contains(&"--playlist-folder-template".to_string()),
            "v2.9.x must not receive --playlist-folder-template (no such option)"
        );
        crate::services::gamdl_capabilities::set_detected_version(None);
    }

    #[test]
    fn playlist_folder_template_suppressed_when_version_unknown() {
        let _guard = PLAYLIST_TEMPLATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::services::gamdl_capabilities::set_detected_version(None);
        let options = GamdlOptions {
            playlist_folder_template: Some("MyPlaylists/{playlist_artist}".to_string()),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(
            !args.contains(&"--playlist-folder-template".to_string()),
            "unknown-version default is 'no capability' per the module contract"
        );
    }

    // ----------------------------------------------------------
    // ArtistAutoSelect
    // ----------------------------------------------------------

    #[test]
    fn artist_auto_select_cli_strings() {
        assert_eq!(ArtistAutoSelect::MainAlbums.to_cli_string(), "main-albums");
        assert_eq!(
            ArtistAutoSelect::CompilationAlbums.to_cli_string(),
            "compilation-albums"
        );
        assert_eq!(ArtistAutoSelect::LiveAlbums.to_cli_string(), "live-albums");
        assert_eq!(ArtistAutoSelect::SinglesEps.to_cli_string(), "singles-eps");
        assert_eq!(ArtistAutoSelect::AllAlbums.to_cli_string(), "all-albums");
        assert_eq!(ArtistAutoSelect::TopSongs.to_cli_string(), "top-songs");
        assert_eq!(
            ArtistAutoSelect::MusicVideos.to_cli_string(),
            "music-videos"
        );
    }

    #[test]
    fn artist_auto_select_serde_roundtrip() {
        let mode = ArtistAutoSelect::TopSongs;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"top-songs\"");

        let deserialized: ArtistAutoSelect = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);
    }

    #[test]
    fn artist_auto_select_cli_arg() {
        let options = GamdlOptions {
            artist_auto_select: Some(ArtistAutoSelect::AllAlbums),
            ..Default::default()
        };
        let args = options.to_cli_args();
        assert!(args.contains(&"--artist-auto-select".to_string()));
        assert!(args.contains(&"all-albums".to_string()));
    }

    // ----------------------------------------------------------
    // SongCodec::from_cli_string
    // ----------------------------------------------------------

    #[test]
    fn from_cli_string_roundtrip() {
        // Every codec should round-trip through to_cli_string → from_cli_string
        let codecs = [
            SongCodec::Alac,
            SongCodec::Atmos,
            SongCodec::Ac3,
            SongCodec::AacBinaural,
            SongCodec::Aac,
            SongCodec::AacLegacy,
            SongCodec::AacHeLegacy,
            SongCodec::AacHe,
            SongCodec::AacDownmix,
            SongCodec::AacHeBinaural,
            SongCodec::AacHeDownmix,
        ];
        for codec in codecs {
            let cli = codec.to_cli_string().to_string();
            let parsed = SongCodec::from_cli_string(&cli);
            assert_eq!(parsed, Some(codec), "round-trip failed for {cli}");
        }
    }

    #[test]
    fn from_cli_string_unknown_returns_none() {
        assert_eq!(SongCodec::from_cli_string("flac"), None);
        assert_eq!(SongCodec::from_cli_string(""), None);
        assert_eq!(SongCodec::from_cli_string("mp3"), None);
    }

    // ----------------------------------------------------------
    // SongCodec::is_wrapper_dependent
    // ----------------------------------------------------------

    #[test]
    fn is_wrapper_dependent_atmos_ac3() {
        assert!(SongCodec::Atmos.is_wrapper_dependent());
        assert!(SongCodec::Ac3.is_wrapper_dependent());
    }

    #[test]
    fn is_wrapper_dependent_non_experimental() {
        assert!(!SongCodec::Alac.is_wrapper_dependent());
        assert!(!SongCodec::Aac.is_wrapper_dependent());
        assert!(!SongCodec::AacLegacy.is_wrapper_dependent());
        assert!(!SongCodec::AacBinaural.is_wrapper_dependent());
        assert!(!SongCodec::AacDownmix.is_wrapper_dependent());
        assert!(!SongCodec::AacHe.is_wrapper_dependent());
        assert!(!SongCodec::AacHeLegacy.is_wrapper_dependent());
        assert!(!SongCodec::AacHeBinaural.is_wrapper_dependent());
        assert!(!SongCodec::AacHeDownmix.is_wrapper_dependent());
    }

    #[test]
    fn required_audio_trait_maps_known_codecs() {
        assert_eq!(SongCodec::Alac.required_audio_trait(), Some("lossless"));
        assert_eq!(SongCodec::Atmos.required_audio_trait(), Some("atmos"));
        assert_eq!(SongCodec::Ac3.required_audio_trait(), Some("dolby-digital"));
        assert_eq!(SongCodec::Aac.required_audio_trait(), Some("lossy-stereo"));
        assert_eq!(
            SongCodec::AacLegacy.required_audio_trait(),
            Some("lossy-stereo")
        );
    }

    #[test]
    fn required_audio_trait_none_for_derived_codecs() {
        // Binaural / downmix / HE variants are computed from another
        // stream — we don't pre-skip them on traits.
        assert_eq!(SongCodec::AacBinaural.required_audio_trait(), None);
        assert_eq!(SongCodec::AacHe.required_audio_trait(), None);
        assert_eq!(SongCodec::AacDownmix.required_audio_trait(), None);
        assert_eq!(SongCodec::AacHeLegacy.required_audio_trait(), None);
        assert_eq!(SongCodec::AacHeBinaural.required_audio_trait(), None);
        assert_eq!(SongCodec::AacHeDownmix.required_audio_trait(), None);
    }
}
