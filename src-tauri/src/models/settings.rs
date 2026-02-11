// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Application settings model.
// Defines the complete settings structure for the gamdl-GUI application.
// These settings are persisted as JSON in the app data directory and
// control both the GUI behavior and the default GAMDL options.
//
// ## Persistence
//
// The settings file is stored at:
//   - macOS:   ~/Library/Application Support/com.mwbm.gamdl-gui/settings.json
//   - Windows: %APPDATA%/com.mwbm.gamdl-gui/settings.json
//   - Linux:   ~/.config/com.mwbm.gamdl-gui/settings.json
//
// The `commands/settings.rs` Tauri commands handle loading and saving this
// file. On first launch (or when the file is missing/corrupt), `Default::default()`
// is used to generate a fresh settings file.
//
// ## Data flow
//
// 1. Frontend reads settings via the `get_settings` Tauri command.
// 2. User edits settings in the React settings UI.
// 3. Frontend writes settings back via `save_settings`.
// 4. Before each download, settings are converted into a `GamdlOptions`
//    struct (see `gamdl_options.rs`) which is then merged with any
//    per-download overrides from the `DownloadRequest`.
//
// ## References
//
// - serde derive macros: <https://docs.rs/serde/latest/serde/>
// - Tauri app data directory: <https://v2.tauri.app/reference/javascript/api/namespacepath/>

use serde::{Deserialize, Serialize};

use super::gamdl_options::{
    CoverFormat, DownloadMode, LyricsFormat, RemuxMode, SongCodec, VideoResolution,
};

/// Complete application settings, persisted as `{app_data}/settings.json`.
///
/// This struct contains all user-configurable preferences, organized into
/// logical sections that mirror the settings UI tabs in the React frontend.
/// Default values (via the `Default` impl below) provide sensible starting
/// points that match the project brief requirements.
///
/// ## Relationship to `GamdlOptions`
///
/// `AppSettings` is the user-facing configuration. Before a download starts,
/// the relevant fields are mapped into a `GamdlOptions` instance (see
/// `gamdl_options.rs`), which is then converted to CLI arguments via
/// `GamdlOptions::to_cli_args()`. Fields like `fallback_enabled` and
/// `music_fallback_chain` have no direct GAMDL CLI equivalent -- they are
/// consumed by the download manager's retry logic instead.
///
/// ## Serialization
///
/// Derives `Serialize` + `Deserialize` via serde so it can be:
/// - Persisted to disk as JSON.
/// - Sent to the React frontend over the Tauri IPC bridge.
///
/// See <https://docs.rs/serde/latest/serde/> for derive macro details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // ================================================================
    // General
    // ================================================================

    /// Output directory for downloaded music and videos.
    /// An empty string means "use the platform's default Music directory",
    /// which is resolved at runtime (e.g., `~/Music` on macOS).
    pub output_path: String,

    /// Metadata language as an IETF BCP 47 language tag (e.g., `"en-US"`,
    /// `"ja-JP"`). Passed to GAMDL's `--language` flag to control the
    /// language of track/album names and artist metadata returned by the
    /// Apple Music API.
    pub language: String,

    /// Whether to overwrite existing files during download. When `false`,
    /// GAMDL skips tracks that already exist in the output directory.
    /// Maps to `GamdlOptions::overwrite` / GAMDL `--overwrite`.
    pub overwrite: bool,

    /// Whether to automatically check for GAMDL/tool updates on startup.
    /// When enabled, the app queries PyPI and GitHub releases for newer
    /// versions of GAMDL and its dependencies (see `dependency.rs`).
    pub auto_check_updates: bool,

    // ================================================================
    // Audio Quality Defaults
    // ================================================================

    /// Default audio codec for music downloads. Maps to
    /// `GamdlOptions::song_codec`. See `SongCodec` in `gamdl_options.rs`
    /// for the full list of available codecs and their characteristics.
    pub default_song_codec: SongCodec,

    // ================================================================
    // Video Quality Defaults
    // ================================================================

    /// Default maximum video resolution. Maps to
    /// `GamdlOptions::music_video_resolution`. See `VideoResolution`
    /// in `gamdl_options.rs` for available resolutions.
    pub default_video_resolution: VideoResolution,

    /// Default video codec priority as a comma-separated string
    /// (e.g., `"h265,h264"`). GAMDL tries codecs left-to-right.
    /// H.265 (HEVC) offers better quality per bitrate but is not
    /// available for all content. Maps to
    /// `GamdlOptions::music_video_codec_priority`.
    pub default_video_codec_priority: String,

    /// Default video container format. Either `"mp4"` (standard) or
    /// `"m4v"` (Apple's variant, which some players handle differently).
    /// Maps to `GamdlOptions::music_video_remux_format`.
    pub default_video_remux_format: String,

    // ================================================================
    // Fallback Quality Chains
    // ================================================================

    /// Whether the fallback quality system is enabled. When `true` and a
    /// download fails with the preferred codec/resolution, the download
    /// manager automatically retries with the next option in the fallback
    /// chain. This is a GUI-only feature -- GAMDL itself does not have
    /// built-in fallback logic.
    pub fallback_enabled: bool,

    /// Ordered list of audio codecs to try if the preferred codec fails.
    /// The first entry is tried first; on failure, the next is attempted,
    /// and so on until one succeeds or the chain is exhausted. Users can
    /// reorder and prune this list in the settings UI.
    pub music_fallback_chain: Vec<SongCodec>,

    /// Ordered list of video resolutions to try if the preferred
    /// resolution is not available for a given music video. Works the
    /// same way as `music_fallback_chain`.
    pub video_fallback_chain: Vec<VideoResolution>,

    // ================================================================
    // Lyrics
    // ================================================================

    /// Default format for synced lyrics files. See `LyricsFormat` in
    /// `gamdl_options.rs`. Maps to `GamdlOptions::synced_lyrics_format`.
    pub synced_lyrics_format: LyricsFormat,

    /// Whether to skip downloading synced lyrics entirely. When `true`,
    /// no `.lrc`/`.srt`/`.ttml` file is created alongside the audio.
    /// Maps to `GamdlOptions::no_synced_lyrics`.
    pub no_synced_lyrics: bool,

    /// Whether to download only lyrics (no audio/video). Useful for
    /// users who already have the audio and just want lyrics files.
    /// Maps to `GamdlOptions::synced_lyrics_only`.
    pub synced_lyrics_only: bool,

    // ================================================================
    // Cover Art
    // ================================================================

    /// Whether to save cover art as a separate image file alongside the
    /// downloaded audio. The artwork is always embedded in the audio file
    /// metadata regardless of this setting; this controls the separate
    /// image file. Maps to `GamdlOptions::save_cover`.
    pub save_cover: bool,

    /// Image format for the separately saved cover art file. See
    /// `CoverFormat` in `gamdl_options.rs`. Maps to
    /// `GamdlOptions::cover_format`.
    pub cover_format: CoverFormat,

    /// Cover art dimensions in pixels. The image is always square, so
    /// a value of 1200 produces a 1200x1200 image. Maps to
    /// `GamdlOptions::cover_size` (which formats it as `"1200x1200"`
    /// for the CLI).
    pub cover_size: u32,

    // ================================================================
    // File/Folder Templates
    // ================================================================
    // These templates use GAMDL's placeholder syntax. Available placeholders
    // include: {album_artist}, {album}, {artist}, {title}, {track}, {disc},
    // {playlist_artist}, {playlist_title}, and others documented at
    // <https://github.com/glomatico/gamdl#usage>.

    /// Folder naming template for album downloads.
    /// Default: `"{album_artist}/{album}"` -- organizes by artist then album.
    pub album_folder_template: String,

    /// Folder naming template for compilation albums (various artists).
    /// Default: `"Compilations/{album}"` -- keeps compilations separate.
    pub compilation_folder_template: String,

    /// Folder naming template for non-album tracks (singles, loose tracks).
    /// Default: `"{artist}/Unknown Album"`.
    pub no_album_folder_template: String,

    /// File naming template for tracks on single-disc albums.
    /// Default: `"{track:02d} {title}"` -- zero-padded track number + title.
    pub single_disc_file_template: String,

    /// File naming template for tracks on multi-disc albums.
    /// Default: `"{disc}-{track:02d} {title}"` -- disc number prefix.
    pub multi_disc_file_template: String,

    /// File naming template for non-album tracks.
    /// Default: `"{title}"` -- just the track title.
    pub no_album_file_template: String,

    /// Folder/file naming template for playlist downloads.
    /// Default: `"Playlists/{playlist_artist}/{playlist_title}"`.
    pub playlist_file_template: String,

    // ================================================================
    // Tool Paths (None = use managed/bundled tools)
    // ================================================================
    // When a path is `None`, the app uses the managed installation in
    // the app data directory. Users can override with custom paths if
    // they have their own installations. These map directly to the
    // corresponding `GamdlOptions` path fields.

    /// Path to a Netscape-format `cookies.txt` file for Apple Music
    /// authentication. Required for downloading encrypted content.
    /// See GAMDL docs for how to export cookies from a browser.
    pub cookies_path: Option<String>,

    /// Custom FFmpeg binary path. `None` = use the managed FFmpeg
    /// installation (see `dependency.rs` and `commands/dependency.rs`).
    pub ffmpeg_path: Option<String>,

    /// Custom mp4decrypt binary path (from Bento4 toolkit). Used to
    /// decrypt Widevine-protected content.
    pub mp4decrypt_path: Option<String>,

    /// Custom MP4Box binary path (from GPAC). Alternative remuxer.
    pub mp4box_path: Option<String>,

    /// Custom N_m3u8DL-RE binary path. Alternative HLS downloader.
    pub nm3u8dlre_path: Option<String>,

    /// Custom amdecrypt binary path. Used with the wrapper system
    /// for decrypting certain DRM-protected streams.
    pub amdecrypt_path: Option<String>,

    // ================================================================
    // Advanced
    // ================================================================

    /// Download tool selection. See `DownloadMode` in `gamdl_options.rs`.
    /// Default: `Ytdlp` (yt-dlp) because it requires no additional binary.
    pub download_mode: DownloadMode,

    /// Remux tool selection. See `RemuxMode` in `gamdl_options.rs`.
    /// Default: `Ffmpeg` because FFmpeg is a required dependency anyway.
    pub remux_mode: RemuxMode,

    /// Whether to use the wrapper/amdecrypt authentication system for
    /// accessing DRM-protected content. When `false` (default), standard
    /// cookie-based authentication is used. Maps to
    /// `GamdlOptions::use_wrapper`.
    pub use_wrapper: bool,

    /// Wrapper server URL used when `use_wrapper` is `true`. The wrapper
    /// server handles account authentication and key exchange. Default:
    /// `"http://127.0.0.1:30020"` (local server).
    pub wrapper_account_url: String,

    /// Maximum filename length in characters. `None` = no truncation
    /// (OS limits still apply: 255 bytes on most filesystems). Useful
    /// for tracks with very long titles that would exceed filesystem
    /// limits. Maps to `GamdlOptions::truncate`.
    pub truncate: Option<u32>,

    /// Tags to exclude from metadata embedding. Each entry is a tag name
    /// (e.g., `"lyrics"`, `"comment"`). Stored as a `Vec` in settings
    /// but joined with commas when passed to GAMDL's `--exclude-tags`.
    pub exclude_tags: Vec<String>,

    // ================================================================
    // UI State
    // ================================================================
    // These fields persist UI layout preferences across sessions. They
    // have no effect on GAMDL CLI arguments.

    /// Whether the sidebar navigation panel is collapsed. Persisted so
    /// the UI remembers the user's preferred layout between sessions.
    pub sidebar_collapsed: bool,

    /// Override the platform theme. `None` = auto-detect from the OS
    /// (respects macOS/Windows dark mode). `Some("dark")` or
    /// `Some("light")` forces a specific theme. Consumed by the React
    /// frontend's `ThemeProvider` component.
    pub theme_override: Option<String>,
}

impl Default for AppSettings {
    /// Creates default settings that match the project brief requirements.
    ///
    /// ## Design rationale for key defaults
    ///
    /// - **`default_song_codec: Alac`** -- The project brief prioritises
    ///   maximum audio quality, so we default to lossless ALAC.
    /// - **`default_video_resolution: P2160`** -- Same reasoning: highest
    ///   available quality (4K UHD).
    /// - **`cover_format: Raw`** -- Preserves the original artwork as
    ///   served by Apple Music without any lossy re-encoding.
    /// - **`fallback_enabled: true`** -- Ensures downloads succeed even
    ///   when the preferred codec/resolution is not available. The project
    ///   brief explicitly defines the fallback chains below.
    /// - **`music_fallback_chain`** -- ALAC -> Atmos -> AC3 -> AAC Binaural
    ///   -> AAC -> AAC Legacy. This descends from lossless through spatial
    ///   audio to standard lossy, matching the project brief's order.
    /// - **`video_fallback_chain`** -- 2160p -> 1440p -> ... -> 240p.
    ///   Every resolution Apple Music offers, in descending order.
    /// - **`synced_lyrics_format: Lrc`** -- LRC is the most widely
    ///   supported lyrics format. For music videos, the download manager
    ///   overrides this to TTML at download time.
    /// - **`output_path: ""`** -- An empty string signals the app to use
    ///   the platform's default Music directory at runtime (resolved by
    ///   `dirs::audio_dir()` or equivalent).
    /// - **Templates** -- Use GAMDL's own default templates so that files
    ///   are organized identically to a standalone GAMDL installation.
    fn default() -> Self {
        Self {
            // --- General ---
            // Empty string = resolve to platform Music dir at runtime.
            output_path: String::new(),
            // English (US) metadata by default; users in other regions
            // can change this to get localized track/album names.
            language: "en-US".to_string(),
            // Do not overwrite by default to prevent accidental data loss.
            overwrite: false,
            // Check for updates on launch so users get security/bug fixes.
            auto_check_updates: true,

            // --- Audio quality ---
            // Default to the highest-quality codec (lossless ALAC).
            default_song_codec: SongCodec::Alac,

            // --- Video quality ---
            // Default to 4K with H.265 preferred, H.264 as fallback codec.
            default_video_resolution: VideoResolution::P2160,
            default_video_codec_priority: "h265,h264".to_string(),
            // m4v is Apple's preferred container; some players handle it
            // better than mp4 for Apple-sourced content.
            default_video_remux_format: "m4v".to_string(),

            // --- Fallback chains (as specified in the project brief) ---
            fallback_enabled: true,
            music_fallback_chain: vec![
                SongCodec::Alac,        // 1. Lossless (ALAC) -- highest quality
                SongCodec::Atmos,       // 2. Dolby Atmos -- spatial audio
                SongCodec::Ac3,         // 3. Dolby Digital (AC3) -- surround
                SongCodec::AacBinaural, // 4. AAC (256kbps) Binaural -- spatial stereo
                SongCodec::Aac,         // 5. AAC (256kbps at up to 48kHz) -- standard lossy
                SongCodec::AacLegacy,   // 6. AAC Legacy (256kbps at up to 44.1kHz) -- broadest compat
            ],
            video_fallback_chain: vec![
                VideoResolution::P2160, // 1. H.265 2160p (4K UHD)
                VideoResolution::P1440, // 2. H.265 1440p (QHD)
                VideoResolution::P1080, // 3. H.265/H.264 1080p (Full HD)
                VideoResolution::P720,  // 4. H.264 720p (HD)
                VideoResolution::P540,  // 5. H.264 540p (qHD)
                VideoResolution::P480,  // 6. H.264 480p (SD)
                VideoResolution::P360,  // 7. H.264 360p (low)
                VideoResolution::P240,  // 8. H.264 240p (lowest)
            ],

            // --- Lyrics ---
            // LRC is the most widely supported format for music players.
            synced_lyrics_format: LyricsFormat::Lrc,
            // Download lyrics by default (they are small and useful).
            no_synced_lyrics: false,
            // Download audio + lyrics, not lyrics-only.
            synced_lyrics_only: false,

            // --- Cover art ---
            // Save cover art by default -- most users want artwork files.
            save_cover: true,
            // Raw = original quality from Apple Music (no re-encoding).
            cover_format: CoverFormat::Raw,
            // 1200px is a good balance: large enough for Retina displays,
            // not so large that it wastes bandwidth/storage.
            cover_size: 1200,

            // --- Templates ---
            // These match GAMDL's built-in defaults for familiar organization.
            album_folder_template: "{album_artist}/{album}".to_string(),
            compilation_folder_template: "Compilations/{album}".to_string(),
            no_album_folder_template: "{artist}/Unknown Album".to_string(),
            single_disc_file_template: "{track:02d} {title}".to_string(),
            multi_disc_file_template: "{disc}-{track:02d} {title}".to_string(),
            no_album_file_template: "{title}".to_string(),
            playlist_file_template: "Playlists/{playlist_artist}/{playlist_title}".to_string(),

            // --- Tool paths ---
            // All None = use managed (auto-installed) tools from the app's
            // data directory. See `commands/dependency.rs` for the management logic.
            cookies_path: None,
            ffmpeg_path: None,
            mp4decrypt_path: None,
            mp4box_path: None,
            nm3u8dlre_path: None,
            amdecrypt_path: None,

            // --- Advanced ---
            // yt-dlp is the default downloader because it is installed as
            // a Python dependency alongside GAMDL (no extra binary needed).
            download_mode: DownloadMode::Ytdlp,
            // FFmpeg is the default remuxer because it is a required
            // dependency for GAMDL anyway.
            remux_mode: RemuxMode::Ffmpeg,
            // Wrapper/amdecrypt is disabled by default. Most users use
            // cookie-based auth. The wrapper is an advanced feature for
            // accessing certain DRM-protected streams.
            use_wrapper: false,
            // Default wrapper URL assumes a locally-running server.
            wrapper_account_url: "http://127.0.0.1:30020".to_string(),
            // No filename truncation by default (OS limits still apply).
            truncate: None,
            // No tags excluded by default -- embed all available metadata.
            exclude_tags: Vec::new(),

            // --- UI state ---
            // Sidebar expanded by default for discoverability.
            sidebar_collapsed: false,
            // Auto-detect theme from OS settings.
            theme_override: None,
        }
    }
}
