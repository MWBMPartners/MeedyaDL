// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Application settings model.
// Defines the complete settings structure for the MeedyaDL application.
// These settings are persisted as JSON in the app data directory and
// control both the GUI behavior and the default GAMDL options.
//
// ## Persistence
//
// The settings file is stored at:
//   - macOS:   ~/Library/Application Support/io.github.meedyadl/settings.json
//   - Windows: %APPDATA%/io.github.meedyadl/settings.json
//   - Linux:   ~/.config/io.github.meedyadl/settings.json
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

/// Companion download mode configuration.
///
/// Controls whether MeedyaDL automatically downloads additional format
/// versions alongside the primary download. This allows users to have
/// both high-fidelity (lossless/spatial) and universally compatible
/// (lossy) versions of their music without downloading separately.
///
/// When companions are enabled, the primary (specialist) format receives
/// a filename suffix (e.g., `[Dolby Atmos]`, `[Lossless]`) while the
/// most universally compatible companion uses a clean filename. This
/// prevents filename collisions and makes the format instantly visible
/// in file browsers.
///
/// ## Serialization
///
/// Uses `snake_case` for JSON field values to match the project's
/// convention for enum variants across the IPC boundary.
///
/// ## Example
///
/// In `AtmosToLossless` mode (the default), downloading an album in
/// Dolby Atmos produces:
/// ```text
/// Artist/Album/
///   01 Song Title [Dolby Atmos].m4a   ← Primary (spatial audio)
///   01 Song Title.m4a                 ← ALAC companion (clean filename)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompanionMode {
    /// No companion downloads. Only the user's selected format is downloaded.
    /// Files use clean filenames with no codec suffix.
    Disabled,

    /// **[DEFAULT]** When downloading Dolby Atmos, also download an ALAC
    /// (lossless) companion version. ALAC and lossy codec downloads do not
    /// trigger companions.
    ///
    /// File naming:
    /// - Atmos files: `01 Song Title [Dolby Atmos].m4a`
    /// - ALAC companion: `01 Song Title.m4a` (clean filename)
    AtmosToLossless,

    /// Maximum companion coverage. When downloading Dolby Atmos, also
    /// download both ALAC (lossless) AND lossy AAC companions. When
    /// downloading ALAC, also download a lossy AAC companion. Lossy
    /// codec downloads do not trigger companions.
    ///
    /// File naming for Atmos primary:
    /// - Atmos: `01 Song Title [Dolby Atmos].m4a`
    /// - ALAC: `01 Song Title [Lossless].m4a`
    /// - AAC: `01 Song Title.m4a` (clean filename)
    ///
    /// File naming for ALAC primary:
    /// - ALAC: `01 Song Title [Lossless].m4a`
    /// - AAC: `01 Song Title.m4a` (clean filename)
    AtmosToLosslessAndLossy,

    /// When downloading any specialist format (Dolby Atmos or ALAC), also
    /// download a lossy AAC companion. The specialist file gets a codec
    /// suffix; the AAC companion uses a clean filename.
    ///
    /// File naming:
    /// - Atmos: `01 Song Title [Dolby Atmos].m4a`
    /// - ALAC: `01 Song Title [Lossless].m4a`
    /// - AAC companion: `01 Song Title.m4a` (clean filename)
    SpecialistToLossy,
}

impl Default for CompanionMode {
    /// Defaults to `AtmosToLossless` — the most common use case where
    /// Atmos users also want a lossless stereo version for universal playback.
    fn default() -> Self {
        Self::AtmosToLossless
    }
}

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
#[serde(default)]
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
    // Companion Downloads
    // ================================================================

    /// Controls whether and how MeedyaDL downloads companion format
    /// versions alongside the primary download. When companions are enabled,
    /// MeedyaDL triggers additional GAMDL invocations after the primary
    /// download succeeds, downloading the same content in different codecs.
    /// Specialist format files receive a codec suffix in their filenames
    /// (e.g., `[Dolby Atmos]`, `[Lossless]`) while the most universally
    /// compatible companion uses a clean filename. All versions are saved
    /// in the same album folder. See `CompanionMode` for available modes.
    pub companion_mode: CompanionMode,

    // ================================================================
    // Lyrics
    // ================================================================

    /// When enabled, ensures that lyrics are both embedded in the audio
    /// file's metadata tags AND saved as a sidecar file (LRC/SRT/TTML).
    /// This provides maximum compatibility: embedded lyrics for players
    /// that support them, sidecar files for those that don't. When enabled,
    /// this overrides `no_synced_lyrics` (forcing sidecar creation) and
    /// removes `"lyrics"` from `exclude_tags` (forcing metadata embedding).
    /// When disabled, lyrics behavior is controlled independently by
    /// `no_synced_lyrics`, `synced_lyrics_format`, and `exclude_tags`.
    pub embed_lyrics_and_sidecar: bool,

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
    /// a value of 10000 requests 10000x10000 — Apple Music's CDN returns
    /// the highest available resolution (typically up to ~3000x3000).
    /// Maps to `GamdlOptions::cover_size` (which formats it as `"WxH"`
    /// for the CLI).
    pub cover_size: u32,

    // ================================================================
    // Animated Artwork (Motion Cover Art)
    // ================================================================

    /// Whether to download animated cover art (motion artwork) from Apple
    /// Music after each album download. When enabled, MeedyaDL queries the
    /// Apple Music catalog API (`extend=editorialVideo`) and saves
    /// `FrontCover.mp4` (square, 1:1) and `PortraitCover.mp4` (portrait,
    /// 3:4) alongside the audio files, if animated artwork is available.
    ///
    /// Requires valid MusicKit credentials (`musickit_team_id`,
    /// `musickit_key_id`, and a private key stored in the OS keychain).
    pub animated_artwork_enabled: bool,

    /// Whether to set the OS "hidden" attribute on downloaded animated
    /// artwork files (FrontCover.mp4, PortraitCover.mp4). When `true`
    /// (default), files are hidden from default file browser views but
    /// still accessible to media players and scripts that reference them
    /// by name.
    ///
    /// Platform behavior:
    /// - **macOS**: Uses `chflags hidden` — files hidden in Finder, original name preserved.
    /// - **Windows**: Uses `attrib +H` — files hidden in Explorer, original name preserved.
    /// - **Linux**: Renames files with a `.` prefix (e.g., `.FrontCover.mp4`) — the only
    ///   cross-compatible hiding mechanism on Linux.
    pub hide_animated_artwork: bool,

    /// Apple MusicKit Team ID for API authentication. This is the
    /// 10-character team identifier from the Apple Developer portal
    /// (e.g., `"ABCDE12345"`). Required when `animated_artwork_enabled`
    /// is `true`.
    pub musickit_team_id: Option<String>,

    /// Apple MusicKit Key ID for API authentication. This is the
    /// 10-character identifier for the MusicKit private key created in
    /// the Apple Developer portal (e.g., `"ABC123DEFG"`). Required when
    /// `animated_artwork_enabled` is `true`.
    ///
    /// **Note:** The private key itself (`.p8` file content) is stored
    /// securely in the OS keychain under the key `"musickit_private_key"`,
    /// NOT in this settings struct.
    pub musickit_key_id: Option<String>,

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

    /// Whether to fetch extra metadata tags (normalization, smooth playback
    /// info, etc.) from Apple Music. When `true`, GAMDL makes additional API
    /// calls to retrieve richer metadata. Maps to `GamdlOptions::fetch_extra_tags`
    /// / GAMDL `--fetch-extra-tags`.
    pub fetch_extra_tags: bool,

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

            // --- Companion downloads ---
            // Default: when Atmos is downloaded, also download an ALAC
            // (lossless) companion so the user has a universally playable
            // stereo version alongside the spatial audio version.
            companion_mode: CompanionMode::AtmosToLossless,

            // --- Lyrics ---
            // Enabled by default: embed lyrics in audio metadata AND keep
            // sidecar files for maximum player compatibility.
            embed_lyrics_and_sidecar: true,
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
            // 10000px requests the highest available resolution from Apple Music's
            // CDN. The CDN returns the largest version it has (typically 3000x3000),
            // so this effectively means "give me the best you have".
            cover_size: 10000,

            // --- Animated artwork ---
            // Disabled by default: requires Apple Developer credentials.
            // Users must configure MusicKit Team ID, Key ID, and private key
            // in Settings > Cover Art before animated artwork can be fetched.
            animated_artwork_enabled: false,
            // Hide animated artwork files by default to keep album folders clean.
            // Files remain accessible by name for media players and scripts.
            hide_animated_artwork: true,
            musickit_team_id: None,
            musickit_key_id: None,

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
            // Fetch extra metadata (normalization, smooth playback info, etc.)
            // by default. Richer metadata is worth the small extra API overhead.
            fetch_extra_tags: true,
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

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // AppSettings::default() -- audio quality defaults
    // ----------------------------------------------------------

    /// Verifies that the default song codec is ALAC (lossless), matching
    /// the project brief's requirement to default to maximum audio quality.
    #[test]
    fn default_song_codec_is_alac() {
        let settings = AppSettings::default();
        assert_eq!(settings.default_song_codec, SongCodec::Alac);
    }

    // ----------------------------------------------------------
    // AppSettings::default() -- video quality defaults
    // ----------------------------------------------------------

    /// Verifies that the default video resolution is 2160p (4K UHD),
    /// matching the project brief's requirement for highest video quality.
    #[test]
    fn default_video_resolution_is_2160p() {
        let settings = AppSettings::default();
        assert_eq!(settings.default_video_resolution, VideoResolution::P2160);
    }

    // ----------------------------------------------------------
    // AppSettings::default() -- fallback system
    // ----------------------------------------------------------

    /// Verifies that the fallback quality system is enabled by default,
    /// ensuring downloads succeed even when the preferred codec or
    /// resolution is not available for a given track.
    #[test]
    fn default_fallback_enabled_is_true() {
        let settings = AppSettings::default();
        assert!(settings.fallback_enabled);
    }

    /// Verifies that the default music fallback chain contains exactly
    /// 6 codecs (ALAC -> Atmos -> AC3 -> AAC Binaural -> AAC -> AAC Legacy),
    /// matching the project brief's specified fallback order.
    #[test]
    fn default_music_fallback_chain_has_correct_length() {
        let settings = AppSettings::default();
        assert_eq!(
            settings.music_fallback_chain.len(),
            6,
            "Music fallback chain should have 6 entries, got: {}",
            settings.music_fallback_chain.len()
        );
    }

    /// Verifies that the default music fallback chain starts with ALAC
    /// (highest quality) and ends with AAC Legacy (broadest compatibility),
    /// descending through the quality tiers as specified in the project brief.
    #[test]
    fn default_music_fallback_chain_order() {
        let settings = AppSettings::default();
        let chain = &settings.music_fallback_chain;
        assert_eq!(chain[0], SongCodec::Alac);
        assert_eq!(chain[1], SongCodec::Atmos);
        assert_eq!(chain[2], SongCodec::Ac3);
        assert_eq!(chain[3], SongCodec::AacBinaural);
        assert_eq!(chain[4], SongCodec::Aac);
        assert_eq!(chain[5], SongCodec::AacLegacy);
    }

    /// Verifies that the default video fallback chain contains exactly
    /// 8 resolutions (2160p down to 240p), covering every resolution
    /// Apple Music offers in descending order.
    #[test]
    fn default_video_fallback_chain_has_correct_length() {
        let settings = AppSettings::default();
        assert_eq!(
            settings.video_fallback_chain.len(),
            8,
            "Video fallback chain should have 8 entries, got: {}",
            settings.video_fallback_chain.len()
        );
    }

    /// Verifies that the default video fallback chain is ordered from
    /// highest resolution (2160p/4K) to lowest (240p), ensuring the
    /// download manager tries the best quality first.
    #[test]
    fn default_video_fallback_chain_order() {
        let settings = AppSettings::default();
        let chain = &settings.video_fallback_chain;
        assert_eq!(chain[0], VideoResolution::P2160);
        assert_eq!(chain[1], VideoResolution::P1440);
        assert_eq!(chain[2], VideoResolution::P1080);
        assert_eq!(chain[3], VideoResolution::P720);
        assert_eq!(chain[4], VideoResolution::P540);
        assert_eq!(chain[5], VideoResolution::P480);
        assert_eq!(chain[6], VideoResolution::P360);
        assert_eq!(chain[7], VideoResolution::P240);
    }

    // ----------------------------------------------------------
    // AppSettings::default() -- general settings
    // ----------------------------------------------------------

    /// Verifies that the default language is "en-US" (English, United States),
    /// which controls the metadata language returned by the Apple Music API.
    #[test]
    fn default_language_is_en_us() {
        let settings = AppSettings::default();
        assert_eq!(settings.language, "en-US");
    }

    /// Verifies that the default output path is an empty string, which
    /// signals the app to use the platform's default Music directory
    /// (resolved at runtime via `dirs::audio_dir()` or equivalent).
    #[test]
    fn default_output_path_is_empty() {
        let settings = AppSettings::default();
        assert!(
            settings.output_path.is_empty(),
            "Default output_path should be empty, got: {:?}",
            settings.output_path
        );
    }

    // ----------------------------------------------------------
    // AppSettings serde roundtrip
    // ----------------------------------------------------------

    /// Verifies that the complete `AppSettings::default()` struct
    /// survives a full serde roundtrip (serialize to JSON, then
    /// deserialize back), ensuring all fields are correctly preserved
    /// when persisting to disk and sending over the IPC bridge.
    #[test]
    fn app_settings_serde_roundtrip_preserves_all_fields() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        // General
        assert_eq!(deserialized.output_path, settings.output_path);
        assert_eq!(deserialized.language, settings.language);
        assert_eq!(deserialized.overwrite, settings.overwrite);
        assert_eq!(deserialized.auto_check_updates, settings.auto_check_updates);

        // Audio quality
        assert_eq!(deserialized.default_song_codec, settings.default_song_codec);

        // Video quality
        assert_eq!(deserialized.default_video_resolution, settings.default_video_resolution);
        assert_eq!(deserialized.default_video_codec_priority, settings.default_video_codec_priority);
        assert_eq!(deserialized.default_video_remux_format, settings.default_video_remux_format);

        // Fallback
        assert_eq!(deserialized.fallback_enabled, settings.fallback_enabled);
        assert_eq!(deserialized.music_fallback_chain.len(), settings.music_fallback_chain.len());
        assert_eq!(deserialized.video_fallback_chain.len(), settings.video_fallback_chain.len());

        // Companion downloads
        assert_eq!(deserialized.companion_mode, settings.companion_mode);

        // Lyrics
        assert_eq!(deserialized.synced_lyrics_format, settings.synced_lyrics_format);
        assert_eq!(deserialized.no_synced_lyrics, settings.no_synced_lyrics);
        assert_eq!(deserialized.synced_lyrics_only, settings.synced_lyrics_only);

        // Cover art
        assert_eq!(deserialized.save_cover, settings.save_cover);
        assert_eq!(deserialized.cover_format, settings.cover_format);
        assert_eq!(deserialized.cover_size, settings.cover_size);

        // Animated artwork
        assert_eq!(deserialized.animated_artwork_enabled, settings.animated_artwork_enabled);
        assert_eq!(deserialized.hide_animated_artwork, settings.hide_animated_artwork);
        assert_eq!(deserialized.musickit_team_id, settings.musickit_team_id);
        assert_eq!(deserialized.musickit_key_id, settings.musickit_key_id);

        // Templates
        assert_eq!(deserialized.album_folder_template, settings.album_folder_template);
        assert_eq!(deserialized.compilation_folder_template, settings.compilation_folder_template);
        assert_eq!(deserialized.playlist_file_template, settings.playlist_file_template);

        // Advanced
        assert_eq!(deserialized.download_mode, settings.download_mode);
        assert_eq!(deserialized.remux_mode, settings.remux_mode);
        assert_eq!(deserialized.use_wrapper, settings.use_wrapper);
        assert_eq!(deserialized.wrapper_account_url, settings.wrapper_account_url);
        assert_eq!(deserialized.fetch_extra_tags, settings.fetch_extra_tags);

        // UI state
        assert_eq!(deserialized.sidebar_collapsed, settings.sidebar_collapsed);
        assert_eq!(deserialized.theme_override, settings.theme_override);
    }

    /// Verifies that all `Option<String>` fields in `AppSettings`
    /// correctly handle the `None` case through a serde roundtrip,
    /// ensuring null JSON values are properly deserialized.
    #[test]
    fn app_settings_serde_handles_optional_fields_as_none() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        // All tool paths should be None by default
        assert!(deserialized.cookies_path.is_none());
        assert!(deserialized.ffmpeg_path.is_none());
        assert!(deserialized.mp4decrypt_path.is_none());
        assert!(deserialized.mp4box_path.is_none());
        assert!(deserialized.nm3u8dlre_path.is_none());
        assert!(deserialized.amdecrypt_path.is_none());
        assert!(deserialized.truncate.is_none());
        assert!(deserialized.theme_override.is_none());
    }

    /// Verifies that `AppSettings` with all optional fields set to
    /// `Some(...)` values survives a serde roundtrip, ensuring custom
    /// tool paths and overrides are correctly persisted.
    #[test]
    fn app_settings_serde_handles_optional_fields_as_some() {
        let mut settings = AppSettings::default();
        settings.cookies_path = Some("/path/to/cookies.txt".to_string());
        settings.ffmpeg_path = Some("/usr/local/bin/ffmpeg".to_string());
        settings.mp4decrypt_path = Some("/usr/local/bin/mp4decrypt".to_string());
        settings.mp4box_path = Some("/usr/local/bin/mp4box".to_string());
        settings.nm3u8dlre_path = Some("/usr/local/bin/N_m3u8DL-RE".to_string());
        settings.amdecrypt_path = Some("/usr/local/bin/amdecrypt".to_string());
        settings.truncate = Some(200);
        settings.theme_override = Some("dark".to_string());

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.cookies_path, Some("/path/to/cookies.txt".to_string()));
        assert_eq!(deserialized.ffmpeg_path, Some("/usr/local/bin/ffmpeg".to_string()));
        assert_eq!(deserialized.mp4decrypt_path, Some("/usr/local/bin/mp4decrypt".to_string()));
        assert_eq!(deserialized.mp4box_path, Some("/usr/local/bin/mp4box".to_string()));
        assert_eq!(deserialized.nm3u8dlre_path, Some("/usr/local/bin/N_m3u8DL-RE".to_string()));
        assert_eq!(deserialized.amdecrypt_path, Some("/usr/local/bin/amdecrypt".to_string()));
        assert_eq!(deserialized.truncate, Some(200));
        assert_eq!(deserialized.theme_override, Some("dark".to_string()));
    }

    /// Verifies that the default settings do not enable overwrite mode,
    /// preventing accidental data loss on first launch.
    #[test]
    fn default_overwrite_is_false() {
        let settings = AppSettings::default();
        assert!(!settings.overwrite);
    }

    /// Verifies that auto-update checking is enabled by default so
    /// users receive security and bug fix notifications on startup.
    #[test]
    fn default_auto_check_updates_is_true() {
        let settings = AppSettings::default();
        assert!(settings.auto_check_updates);
    }
}
