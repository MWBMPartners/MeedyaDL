// Copyright (c) 2026 MeedyaSuite
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

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::gamdl_options::{
    ArtistAutoSelect, CoverFormat, DownloadMode, LogLevel, LyricsFormat, RemuxMode, SongCodec,
    VideoResolution,
};

/// Serde default helper that returns `true`. Used for boolean settings
/// that should default to enabled when the field is missing from an
/// older settings.json (backward compatibility during upgrades).
fn default_true() -> bool {
    true
}

/// Serde default helper for the `gamdl_log_level` field (#768). Returns
/// `LogLevel::Info`, which matches GAMDL's compiled-in default. Keeps
/// settings.json files written by older builds (where the field was
/// absent) loading unchanged on upgrade.
fn default_gamdl_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_replaygain_reference() -> f64 {
    -18.0
}

/// Update channel (stability tier) the user is tracking.
///
/// Channels are ordered from least to most stable. The user selects which
/// channel they want updates from; the update checker filters GitHub
/// releases so the user only sees updates matching their channel, and
/// `download_and_install_app_update` refuses to install a release whose
/// channel is less stable than the user's selection (guard against
/// accidentally sliding down the stability ladder via a spoofed URL).
///
/// The channel is derived from a release tag's pre-release suffix:
///   - `v0.32.0-alpha.1` → Alpha
///   - `v0.32.0-beta.1`  → Beta
///   - `v0.32.0-rc.1`    → Rc
///   - `v0.32.0`         → Stable
///
/// **Nightly/Weekly/Monthly removed from the producer pipeline in v1.11.0** —
/// alpha is now the bleeding-edge channel (push-driven on the alpha branch).
/// The old cron-driven channels generated more conflict-issue noise than user
/// signal and overlapped functionally with the alpha branch's workflow. The
/// enum variants are kept for **backwards-compatible settings.json
/// deserialisation only** (an in-the-wild install with
/// `"update_channel": "nightly"` must still load cleanly so the v6 → v7
/// migration can promote them to Alpha). They are HIDDEN FROM THE UI and
/// `from_tag()` no longer returns them — any tag with those suffixes
/// classifies as Alpha instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Deprecated (#873-era cron-channel removal). Kept ONLY for legacy
    /// settings.json compatibility; the v6 → v7 migration promotes it
    /// to Alpha on load. Not exposed in the UI; from_tag() never returns it.
    Nightly,
    /// Deprecated — same as Nightly above.
    Weekly,
    /// Deprecated — same as Nightly above.
    Monthly,
    Alpha,
    Beta,
    Rc,
    Stable,
}

impl UpdateChannel {
    /// Parses a release tag (with or without leading `v`) and returns the
    /// channel implied by its pre-release suffix. Tags with no suffix or
    /// an unrecognised suffix are treated as Stable.
    ///
    /// Legacy `-nightly.*` / `-weekly.*` / `-monthly.*` tags (from before
    /// the cron channels were removed in v1.11.0) classify as `Alpha` — so
    /// in-the-wild installs running an old nightly build still see updates
    /// even though their tag's suffix is no longer a recognised channel.
    pub fn from_tag(tag: &str) -> Self {
        let trimmed = tag.trim_start_matches('v');
        let suffix = match trimmed.split_once('-') {
            Some((_, s)) => s,
            None => return Self::Stable,
        };
        // Match on the first dotted segment so "alpha.11" → "alpha".
        let label = suffix.split('.').next().unwrap_or("").to_ascii_lowercase();
        match label.as_str() {
            "alpha" | "nightly" | "weekly" | "monthly" => Self::Alpha,
            "beta" => Self::Beta,
            "rc" => Self::Rc,
            _ => Self::Stable,
        }
    }

    /// Maps deprecated cron channels (Nightly/Weekly/Monthly) to Alpha;
    /// passes everything else through unchanged. Called during the
    /// v6 → v7 settings migration so users on a removed channel get
    /// gracefully promoted to the closest active equivalent (Alpha).
    pub fn migrate_deprecated_to_alpha(self) -> Self {
        match self {
            Self::Nightly | Self::Weekly | Self::Monthly => Self::Alpha,
            other => other,
        }
    }

    /// True for any channel less stable than `Stable`. Used by the UI to
    /// surface the pre-release stability warning before a user switches.
    pub fn is_pre_release(self) -> bool {
        self != Self::Stable
    }

    /// True for the channels that are gated behind Dev Access in the UI.
    /// Currently just Alpha — Beta and Rc are freely selectable (with a
    /// confirmation warning); Stable is the default.
    pub fn requires_dev_access(self) -> bool {
        matches!(self, Self::Alpha)
    }
}

/// Default update channel: Stable. New installs only receive production
/// releases unless the user opts into a pre-release channel in Settings.
const fn default_update_channel() -> UpdateChannel {
    UpdateChannel::Stable
}

/// Companion download mode configuration.
///
/// Controls whether `MeedyaDL` automatically downloads additional format
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

    /// When downloading Dolby Atmos, also download AC3 (Dolby Digital),
    /// ALAC (lossless), and AAC companions — 4 files per track total.
    /// This gives the user every available quality tier.
    ///
    /// File naming:
    /// - Atmos: `01 Song Title [Dolby Atmos].m4a`
    /// - AC3: `01 Song Title [Dolby Digital].m4a`
    /// - ALAC: `01 Song Title [Lossless].m4a`
    /// - AAC: `01 Song Title.m4a` (clean filename)
    AtmosToAllFormats,

    /// User-defined custom companion codecs. The companion codecs are
    /// specified in `AppSettings::custom_companion_codecs`. Each selected
    /// codec is downloaded as a separate companion tier. The most
    /// universally compatible codec (lowest quality) in the selection gets
    /// a clean filename; higher-quality codecs get a suffix.
    ///
    /// This mode is triggered for **any** primary codec, unlike the preset
    /// modes which only trigger for specific primary codecs (Atmos/ALAC).
    Custom,
}

impl Default for CompanionMode {
    /// Defaults to `AtmosToLossless` — the most common use case where
    /// Atmos users also want a lossless stereo version for universal playback.
    fn default() -> Self {
        Self::AtmosToLossless
    }
}

/// Filename for saved cover art images (without extension).
///
/// GAMDL writes `Cover.<ext>` by default. MeedyaDL renames the file after
/// download to match this setting. Default: `FrontCover` for consistency
/// with animated artwork naming (FrontCover.mp4, FrontCoverPortrait.mp4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverArtName {
    /// Keep GAMDL's default filename: `Cover.<ext>`
    Cover,
    /// Rename to `FrontCover.<ext>` (consistent with animated artwork)
    FrontCover,
    /// Rename to `Folder.<ext>` (Windows Media Player convention)
    Folder,
}

impl CoverArtName {
    /// Returns the filename stem (without extension) for this cover art name.
    #[must_use]
    pub fn to_filename_stem(&self) -> &str {
        match self {
            Self::Cover => "Cover",
            Self::FrontCover => "FrontCover",
            Self::Folder => "Folder",
        }
    }
}

fn default_cover_art_name() -> CoverArtName {
    CoverArtName::FrontCover
}

/// User-configurable zero-padding for the `{track}` placeholder in
/// filename templates (#587).
///
/// `Auto` is the preferred default: padding width derives from the
/// album's `track_total` at download time so a 12-track album gets
/// `01`-`12` and a 200-track box set gets `001`-`200` without user
/// intervention. Fixed widths are offered for users who want
/// library-wide filename consistency regardless of album size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackNumberPadding {
    /// Auto-derive padding width from album's track_total. Produces
    /// `01` for <100-track albums, `001` for <1000, `0001` for larger.
    Auto,
    /// No padding: `1`, `2`, ..., `9`, `10`, `100`.
    None,
    /// 2 digits: `01`, `02`, ..., `99`, `100`. (Pre-#587 default
    /// behaviour — sorts wrong on albums >99 tracks.)
    TwoDigits,
    /// 3 digits: `001`, `002`, ..., `999`, `1000`.
    ThreeDigits,
    /// 4 digits: `0001`, ..., `9999`.
    FourDigits,
}

impl TrackNumberPadding {
    /// Resolve to a concrete padding width for the given album
    /// `track_total`. `Auto` is the only mode that consults the
    /// album metadata; the fixed modes ignore the argument entirely.
    ///
    /// Returns the number of digits in the format specifier
    /// (e.g. `3` → `{track:03d}` in Python-style templates).
    #[must_use]
    pub fn resolve_width(&self, track_total: Option<u32>) -> usize {
        match self {
            Self::None => 0,
            Self::TwoDigits => 2,
            Self::ThreeDigits => 3,
            Self::FourDigits => 4,
            Self::Auto => match track_total {
                Some(n) if n <= 99 => 2,
                Some(n) if n <= 999 => 3,
                Some(_) => 4,
                // No album metadata available yet — match the
                // pre-#587 `{track:02d}` default so single-track
                // downloads don't regress.
                None => 2,
            },
        }
    }
}

fn default_track_number_padding() -> TrackNumberPadding {
    TrackNumberPadding::Auto
}

/// Mirrors GAMDL v3.0+'s upstream default for `--playlist-folder-template`
/// (`gamdl/downloader/base.py::playlist_folder_template`). Kept in sync here
/// so users who haven't customised the template see the same layout
/// regardless of whether the flag is emitted (v3.0+) or not (v2.9.x
/// silently falls back to its own equivalent default).
fn default_playlist_folder_template() -> String {
    "Playlists/{playlist_artist}".to_string()
}

/// User-configurable zero-padding for the `{disc}` placeholder in
/// filename templates (#587). Mirrors `TrackNumberPadding` but scoped
/// to disc numbers (typically much smaller than track counts).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscNumberPadding {
    /// Auto-derive padding width from `disc_total`. 1-digit for <10
    /// discs, 2-digit for <100, 3-digit for >99 (pathological). Most
    /// real-world albums hit the 1-digit branch.
    Auto,
    /// No padding: `1`, `2`, `10`. (Pre-#587 behaviour.)
    None,
    /// 1 digit (same as `None` for values < 10).
    OneDigit,
    /// 2 digits: `01`, `02`, `10`, `99`.
    TwoDigits,
}

impl DiscNumberPadding {
    /// Resolve to a concrete padding width for the given
    /// `disc_total`. See `TrackNumberPadding::resolve_width`.
    #[must_use]
    pub fn resolve_width(&self, disc_total: Option<u32>) -> usize {
        match self {
            Self::None | Self::OneDigit => 0,
            Self::TwoDigits => 2,
            Self::Auto => match disc_total {
                Some(n) if n <= 9 => 0,
                Some(n) if n <= 99 => 2,
                Some(_) => 3,
                None => 0,
            },
        }
    }
}

fn default_disc_number_padding() -> DiscNumberPadding {
    DiscNumberPadding::Auto
}

// ============================================================
// Duplicate Detection (#510)
// ============================================================

/// Scope of the pre-queue duplicate-detection pass.
///
/// Controls how far we look when deciding whether a track fetched from the
/// Apple Music API is already "claimed" by another download. Broader scopes
/// do more I/O (reading manifest files on disk) but catch more duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateDetectionScope {
    /// Feature disabled — no duplicate detection is performed.
    Off,
    /// Dedupe only within the fan-out of the current artist URL
    /// (across the N modes in `artist_auto_select_multi`).
    IntraSession,
    /// Intra-session plus songs already present in other queue items
    /// (Queued / Downloading / Processing states).
    #[default]
    IntraAndQueued,
    /// Intra-session, queue, AND songs recorded in existing manifest files
    /// under the configured output directory (prior download history).
    /// Walks the output directory once per artist-URL enqueue.
    IntraAndQueuedAndHistory,
}

/// Dedup-key strategy when comparing two tracks.
///
/// Apple Music's internal `song_id` is the most reliable key (unique per
/// master). ISRC is shared across re-releases, which is either desired
/// (catching remasters) or too aggressive (collapsing distinct masters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DedupKeyStrategy {
    /// Primary key: Apple Music `song_id`. Fall back to ISRC only when
    /// `song_id` is absent. Recommended default.
    #[default]
    SongIdIsrcFallback,
    /// Use ISRC exclusively — catches remasters/re-releases as duplicates.
    IsrcOnly,
    /// Use `song_id` exclusively — most conservative, won't collapse
    /// different masters of the same recording.
    SongIdOnly,
}

/// Settings governing the pre-queue duplicate-detection pipeline.
///
/// Applied when an Apple Music artist URL is fanned out into multiple
/// queue items (one per `artist_auto_select_multi` mode). Before those
/// items are enqueued, each mode's track list is fetched via the Apple
/// Music catalog API and filtered against the user's preference order so
/// that a given song is downloaded exactly once.
///
/// Disabling this (via `scope: Off`) makes the feature a no-op.
///
/// ## Scope (important)
///
/// This operates on **track identity** only — the same song appearing in
/// multiple artist-auto-select modes (album, single, compilation, etc.).
/// Companion downloads that produce multiple **format** versions of the
/// same song (ALAC / Atmos / AAC / AC3 etc., governed by `companion_mode`)
/// are NOT touched. A song chosen from, say, the main-album mode still
/// triggers the user's full companion chain. Dedup only prevents the
/// same-quality copy from being fetched 3 times because it happens to
/// appear in 3 different albums under an artist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DuplicateDetectionSettings {
    /// Which scopes to consult when deciding if a track is a duplicate.
    pub scope: DuplicateDetectionScope,
    /// Ordered priority of artist-auto-select modes. When the same song
    /// appears in multiple fetched modes, the mode earliest in this list
    /// wins and keeps the track; later modes have the duplicate skipped.
    /// Modes not present in this list fall to the end in default order.
    pub preference_order: Vec<ArtistAutoSelect>,
    /// How strict to be when matching tracks (see [`DedupKeyStrategy`]).
    pub key_strategy: DedupKeyStrategy,
}

impl Default for DuplicateDetectionSettings {
    fn default() -> Self {
        Self {
            scope: DuplicateDetectionScope::IntraAndQueued,
            // User-agreed default order: full album > singles/EPs >
            // compilations > live albums > top-songs. Music videos are
            // intentionally omitted (not an audio track) and won't be
            // deduplicated even when selected.
            preference_order: vec![
                ArtistAutoSelect::MainAlbums,
                ArtistAutoSelect::SinglesEps,
                ArtistAutoSelect::CompilationAlbums,
                ArtistAutoSelect::LiveAlbums,
                ArtistAutoSelect::TopSongs,
            ],
            key_strategy: DedupKeyStrategy::SongIdIsrcFallback,
        }
    }
}

// ============================================================
// Per-service settings (#319)
// ============================================================

/// Per-service settings for Apple Music downloads.
///
/// These fields are Apple Music-specific and will eventually be
/// the sole location for Apple Music configuration. During the
/// migration period, the top-level `AppSettings` flat fields
/// are still read by existing code; these nested fields are
/// available for new code to adopt incrementally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppleMusicSettings {
    /// Apple Music storefront code (e.g., "gb", "us").
    /// Auto-detected from language region code when empty.
    pub storefront: String,
    /// Path to Netscape cookies.txt file for Apple Music auth.
    pub cookies_path: Option<String>,
    /// Apple Developer Team ID for MusicKit API (10-char).
    pub musickit_team_id: Option<String>,
    /// Apple MusicKit Key ID (10-char).
    pub musickit_key_id: Option<String>,
    /// Download animated motion artwork from Apple Music.
    pub animated_artwork_enabled: bool,
    /// Hide animated artwork files from OS file browsers.
    pub hide_animated_artwork: bool,
    /// Convert TTML lyrics to Enhanced LRC with word-by-word sync.
    pub enhanced_lrc: bool,
    /// Append [Explicit]/[Clean] suffixes to filenames.
    pub content_advisory_in_filenames: bool,
}

impl Default for AppleMusicSettings {
    fn default() -> Self {
        Self {
            storefront: String::new(),
            cookies_path: None,
            musickit_team_id: None,
            musickit_key_id: None,
            animated_artwork_enabled: true,
            hide_animated_artwork: false,
            enhanced_lrc: true,
            content_advisory_in_filenames: true,
        }
    }
}

/// Per-service settings for Spotify downloads.
///
/// Populated through milestones M9-1 .. M9-6 as the votify
/// integration lands. The `anti_ban` block is the safety-critical
/// surface shipped in M9-4 — see
/// [`crate::models::spotify_anti_ban::AntiBanSettings`] for the
/// individual knobs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SpotifySettings {
    /// Spotify cookies path (for premium auth).
    pub cookies_path: Option<String>,

    /// Anti-ban configuration. Safety-on defaults — see the model
    /// for rationale.
    pub anti_ban: crate::models::spotify_anti_ban::AntiBanSettings,
}

/// Per-service settings for YouTube/YouTube Music downloads (stub).
///
/// Will be populated in milestone M9 (v2.1.0) when yt-dlp
/// integration is implemented.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct YouTubeSettings {
    /// YouTube cookies path (for age-restricted/member content).
    pub cookies_path: Option<String>,
}

/// Per-service settings container, keyed by service ID string.
///
/// Wraps service-specific settings in a flat map for JSON
/// serialization: `{ "apple-music": { ... }, "spotify": { ... } }`.
/// Uses `#[serde(default)]` for backwards compatibility — older
/// `settings.json` files without this field will get empty defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PerServiceSettings {
    /// Apple Music-specific settings.
    #[serde(rename = "apple-music", default)]
    pub apple_music: AppleMusicSettings,
    /// Spotify-specific settings (stub).
    #[serde(default)]
    pub spotify: SpotifySettings,
    /// YouTube/YouTube Music-specific settings (stub).
    #[serde(default)]
    pub youtube: YouTubeSettings,
    /// User-overridden engine priority per platform.
    /// Keys are platform IDs (e.g., "bbc-iplayer"), values are ordered
    /// engine IDs (first = primary). When empty, the default order from
    /// engines.toml is used. Only needed for platforms with multiple
    /// engines (e.g., BBC iPlayer: get_iplayer → yt-dlp).
    #[serde(default)]
    pub engine_priority: HashMap<String, Vec<String>>,
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
#[allow(clippy::struct_excessive_bools)] // User-facing settings with genuinely independent boolean flags
pub struct AppSettings {
    // ================================================================
    // General
    // ================================================================
    /// Settings schema version for migration support.
    /// Incremented when backwards-incompatible changes are made to the settings
    /// structure (field renames, default value changes, field removal).
    /// On load, the migration function runs any needed upgrades sequentially.
    #[serde(default)]
    pub settings_version: u32,

    /// Output directory for downloaded music and videos.
    /// An empty string means "use the platform's default Music directory",
    /// which is resolved at runtime (e.g., `~/Music` on macOS).
    pub output_path: String,

    /// Temporary directory for intermediate files during download and
    /// processing. An empty string means "use a `MeedyaDL` subdirectory
    /// within the OS default temp directory" (e.g., `/var/folders/.../MeedyaDL`
    /// on macOS, `%TEMP%\MeedyaDL` on Windows, `/tmp/MeedyaDL` on Linux),
    /// resolved at runtime via `std::env::temp_dir().join("MeedyaDL")`.
    /// Maps to `GamdlOptions::temp_path` / GAMDL `--temp-path`.
    #[serde(default)]
    pub temp_path: String,

    /// Metadata language as an IETF BCP 47 language tag (e.g., `"en-US"`,
    /// `"ja-JP"`). Passed to GAMDL's `--language` flag to control the
    /// language of track/album names and artist metadata returned by the
    /// Apple Music API.
    pub language: String,

    /// Apple Music storefront code (e.g., `"gb"`, `"us"`, `"jp"`).
    /// Required by GAMDL >= 2.9.3 for API requests.
    ///
    /// When empty (default), auto-detected from the language setting's
    /// region code (e.g., `"en-GB"` → `"gb"`). When set explicitly,
    /// the user's value is always used and never overwritten.
    ///
    /// Controlled in Settings > General > Storefront.
    #[serde(default)]
    pub storefront: String,

    /// Whether to overwrite existing files during download. When `false`,
    /// GAMDL skips tracks that already exist in the output directory.
    /// Maps to `GamdlOptions::overwrite` / GAMDL `--overwrite`.
    pub overwrite: bool,

    /// UI display language code (e.g., `"en"`, `"de"`, `"fr"`). An empty
    /// string means "auto-detect from the OS locale". Used by the React
    /// frontend's i18next setup to load the corresponding translation file
    /// from `public/locales/{code}/translation.json`.
    #[serde(default)]
    pub ui_language: String,

    /// Whether to automatically check for GAMDL/tool updates on startup.
    /// When enabled, the app queries `PyPI` and GitHub releases for newer
    /// versions of GAMDL and its dependencies (see `dependency.rs`).
    pub auto_check_updates: bool,

    /// Whether to include pre-release versions when checking for app updates.
    /// When enabled, the update checker also considers beta/RC releases from
    /// GitHub. Pre-release versions may contain bugs or incomplete features.
    #[serde(default)]
    pub check_pre_releases: bool,

    /// Release channel the user is subscribed to for app updates.
    /// Defaults to `Stable` (production releases only). Users can opt into
    /// less-stable channels (Beta/Alpha/Monthly/Weekly/Nightly) to preview
    /// upcoming features. The channel acts as a guard in
    /// `download_and_install_app_update`: the installer refuses tags whose
    /// channel is less stable than the user's selection, so a stable
    /// install can't be tricked into downgrading to a nightly build.
    #[serde(default = "default_update_channel")]
    pub update_channel: UpdateChannel,

    /// How often (in hours) to periodically check for updates while the app
    /// is running. Value `0` = check on startup only (no periodic timer).
    /// Only effective when `auto_check_updates` is `true`. Default: 6 hours,
    /// providing frequent checks during early development.
    #[serde(default = "default_update_interval")]
    pub update_check_interval_hours: u32,

    /// Maximum minutes a GAMDL child process may sit silent (no
    /// stdout / stderr output) while still in the active-download phase
    /// before the companion supervisor kills it (#505). The watchdog
    /// pauses automatically once the post-processing phase is detected
    /// (#503), so a slow remux/decrypt over a network volume will not
    /// trip the killswitch. Default: 5 minutes.
    #[serde(default = "default_gamdl_idle_timeout")]
    pub gamdl_idle_timeout_minutes: u32,

    /// Whether to start processing the download queue immediately when items
    /// are enqueued. When `true` (the default), downloads begin as soon as
    /// a concurrency slot is available. When `false`, items are added in
    /// `Queued` state and the user must manually start processing from the
    /// Queue page.
    #[serde(default = "default_auto_start_queue")]
    pub auto_start_queue: bool,

    /// Whether to show a confirmation modal before the "Abort Queue"
    /// action fires (#620). Default: `true`. Users who've grown
    /// comfortable with the destructive action can tick "Don't ask
    /// again" on the modal to flip this to `false` and invoke the
    /// abort via a single click (keyboard shortcut or button).
    ///
    /// Exposed by the modal's "Don't ask again" checkbox and in
    /// Settings > General > Preferences for explicit re-enable.
    #[serde(default = "default_true")]
    pub abort_queue_confirm: bool,

    /// Whether to send native OS desktop notifications for download events
    /// (completion and terminal failure). Notifications are only sent when
    /// the main application window is not focused, so they do not interrupt
    /// users who are actively watching the queue. Default: `true`.
    ///
    /// Controlled in Settings > General > Preferences.
    #[serde(default = "default_true")]
    pub desktop_notifications: bool,

    /// Notification delivery style controlling how notifications are shown.
    /// "in_app_only" = in-app toasts only.
    /// "native_and_in_app" = both native OS notifications and in-app toasts (default).
    /// "native_only" = native OS notifications only (no in-app toasts).
    #[serde(default = "default_notification_style")]
    pub notification_style: String,

    /// Auto-dismiss duration for transient notifications (seconds).
    /// Range: 3-60 seconds. Default: 5 seconds.
    /// Affects both in-app toasts and native OS notifications.
    /// Persistent notifications (errors, warnings) are not affected.
    #[serde(default = "default_notification_dismiss")]
    pub notification_auto_dismiss_seconds: u32,

    /// Smart re-download detection using Apple Music API `lastModifiedDate`.
    /// When enabled and a user queues a URL that was previously downloaded,
    /// MeedyaDL compares the stored `lastModifiedDate` from the `.meedyadl`
    /// manifest against the current API value. If unchanged, shows an info
    /// toast suggesting the content hasn't changed. Users can still proceed.
    /// Controlled in Settings > General > Preferences.
    #[serde(default = "default_true")]
    pub smart_redownload_detection: bool,

    /// Clipboard monitoring for supported URLs.
    /// When enabled, MeedyaDL periodically reads the system clipboard and
    /// prompts the user when a supported URL (e.g., Apple Music) is detected.
    /// Privacy-first: only checks for URL patterns, never stores clipboard
    /// contents. Default: `true`.
    /// Controlled in Settings > General > Preferences.
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,

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
    /// Controls whether and how `MeedyaDL` downloads companion format
    /// versions alongside the primary download. When companions are enabled,
    /// `MeedyaDL` triggers additional GAMDL invocations after the primary
    /// download succeeds, downloading the same content in different codecs.
    /// Specialist format files receive a codec suffix in their filenames
    /// (e.g., `[Dolby Atmos]`, `[Lossless]`) while the most universally
    /// compatible companion uses a clean filename. All versions are saved
    /// in the same album folder. See `CompanionMode` for available modes.
    pub companion_mode: CompanionMode,

    /// User-selected codecs for `CompanionMode::Custom`. Each codec in this
    /// list is downloaded as a separate companion tier alongside the primary
    /// download. Ignored when `companion_mode` is not `Custom`.
    ///
    /// Order matters: codecs are tried in the order listed. The last codec
    /// that has no natural suffix (i.e., a lossy AAC variant) receives a
    /// clean filename; all others receive a codec suffix.
    #[serde(default)]
    pub custom_companion_codecs: Vec<SongCodec>,

    /// Whether to automatically download music videos as companions when
    /// downloading audio tracks. **Experimental.** After each download
    /// completes, MeedyaDL discovers music videos via two sources:
    ///
    /// 1. **Apple Music API** (Step 6) — queries music video relationships
    ///    for each track. Requires MusicKit credentials (Team ID, Key ID,
    ///    and private key in OS keychain). Gracefully skipped when
    ///    credentials are not configured.
    /// 2. **MusicBrainz ISRC lookup** (Step 6b) — discovers videos via
    ///    ISRC codes. No credentials required. Apple Music URLs found
    ///    here are downloaded via GAMDL.
    ///
    /// Music videos use the video quality settings from Settings > Quality
    /// (resolution, codec priority, remux format).
    #[serde(default)]
    pub music_video_companion: bool,

    /// When enabled, uses the MusicBrainz database to discover music
    /// videos and cross-platform links for downloaded tracks via ISRC
    /// codes. No credentials required (free public API). Also used by
    /// Music Video Companions (when enabled) as a fallback or sole
    /// discovery source when MusicKit credentials are not configured.
    /// Stores discovered platform URLs (Spotify, YouTube, etc.) as
    /// metadata for future cross-platform features.
    #[serde(default)]
    pub musicbrainz_lookup: bool,

    /// **Odesli (song.link) cross-platform URL lookup** (#295 Phase A).
    ///
    /// When enabled, after the primary download MeedyaDL queries
    /// Odesli's API (`api.song.link/v1-alpha.1/links?url=…`) with
    /// the album URL and stores the returned per-platform URLs
    /// (Spotify / YouTube / Tidal / Deezer / Amazon Music /
    /// SoundCloud / Bandcamp / Pandora / …) in the manifest's
    /// `ManifestSource.cross_platform_urls` field.
    ///
    /// **Rate limit**: free tier is 10 req/min (one album = one
    /// request). Set [`odesli_api_key`] for the 60 req/min tier if
    /// you regularly download many albums in a short window. Without
    /// a key, MeedyaDL's per-process limiter throttles to ~54
    /// req/min so a free-tier user can't burst-trip the cap.
    #[serde(default)]
    pub odesli_lookup_enabled: bool,

    /// **Odesli API key** (#295 Phase A — optional).
    ///
    /// Free-tier requests have no auth requirement. Setting this
    /// field bumps your account to the 60 req/min tier — useful for
    /// power users with large libraries. Get a key at
    /// <https://songlink.notion.site/API-d0ebe08a5e304a55928405eb682f6741>.
    ///
    /// Empty string ⇒ free tier (default).
    #[serde(default)]
    pub odesli_api_key: String,

    // ================================================================
    // Lyrics
    // ================================================================
    /// When enabled, lyrics/captions are embedded in the audio file's
    /// metadata tags (`©lyr` atom for M4A). This removes `"lyrics"` from
    /// `exclude_tags` in merge_options so GAMDL embeds them, and also
    /// triggers MeedyaDL's Enhanced LRC embedding in the enrichment pipeline.
    ///
    /// When disabled, lyrics are still downloaded as sidecar files (if
    /// `keep_lyrics_sidecar` is true) but not embedded in the audio.
    pub embed_lyrics_and_sidecar: bool,

    /// When enabled alongside `embed_lyrics_and_sidecar`, sidecar lyrics
    /// files (LRC/SRT/TTML) are kept on disk after embedding. When disabled,
    /// sidecar files are still created during download (GAMDL needs them)
    /// but could be cleaned up. Currently defaults to true for maximum
    /// player compatibility.
    ///
    /// Only meaningful when `embed_lyrics_and_sidecar` is true. When embed
    /// is off, sidecar behavior is controlled by `no_synced_lyrics`.
    #[serde(default = "default_true")]
    pub keep_lyrics_sidecar: bool,

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

    /// Additional lyrics formats to download as lightweight companions
    /// after the primary download completes. Each format in this list
    /// triggers a separate `--synced-lyrics-only` GAMDL invocation.
    /// The primary format (`synced_lyrics_format`) is NOT included here.
    /// Defaults to `[Srt]` so users get SRT alongside the primary format.
    #[serde(default)]
    pub companion_lyrics_formats: Vec<LyricsFormat>,

    /// When enabled, MeedyaDL post-processes TTML lyrics files to produce
    /// Enhanced LRC with word-by-word synchronized timestamps. The TTML
    /// is automatically set as the primary lyrics download format, and the
    /// resulting Enhanced LRC is saved as a `.lrc` sidecar and embedded
    /// in the audio file's metadata.
    ///
    /// Enhanced LRC uses inline `<mm:ss.xx>` word timestamps within standard
    /// `[mm:ss.xx]` line timestamps, enabling karaoke-style word-by-word
    /// highlighting in compatible players (foobar2000, Poweramp, AIMP, etc.).
    /// It is fully backward-compatible with standard LRC players.
    ///
    /// Requires: Apple Music TTML lyrics with `itunes:timing="Word"`.
    /// Songs without word-level timing gracefully fall back to standard
    /// line-level LRC.
    #[serde(default = "default_true")]
    pub enhanced_lrc: bool,

    /// When enabled and the primary lyrics format (typically TTML when
    /// Enhanced LRC is active) fails to produce sidecar files for some
    /// tracks, automatically retry with fallback formats.
    ///
    /// Fallback chains differ by content type:
    /// - **Audio** (`.m4a`): TTML → LRC → SRT
    /// - **Video** (`.m4v`/`.mp4`): TTML → SRT → LRC
    ///
    /// Each fallback attempt runs GAMDL with `--synced-lyrics-only` to
    /// download just the lyrics without re-downloading media. The chain
    /// stops as soon as lyrics coverage matches the number of media files.
    #[serde(default = "default_true")]
    pub lyrics_fallback_enabled: bool,

    /// When enabled, generates WebVTT (`.vtt`) subtitle files from
    /// downloaded lyrics sidecars (TTML, SRT, or LRC). WebVTT is the
    /// standard format for web-based video players.
    ///
    /// Source priority: TTML first (richest timing data), then SRT
    /// (has start+end times), then LRC (start times only; end times
    /// estimated from the next cue). Skips if `.vtt` already exists.
    #[serde(default)]
    pub generate_webvtt: bool,

    /// When enabled, generates format-rich SRT subtitle files from TTML
    /// that preserve styling (bold, italic, underline, colours) using
    /// HTML-like tags. If a plain SRT already exists (from GAMDL or
    /// lyrics fallback), the rich SRT replaces it since TTML has richer
    /// data. If no TTML exists, any downloaded plain SRT is kept.
    #[serde(default = "default_true")]
    pub generate_rich_srt: bool,

    /// When enabled, embeds SRT and WebVTT subtitle content into
    /// MP4/M4A/M4V containers as freeform atoms. Subtitles travel with
    /// the file rather than requiring separate sidecar files.
    #[serde(default)]
    pub embed_subtitles: bool,

    /// When enabled, generates ASS (Advanced SubStation Alpha) subtitle
    /// files from TTML or WebVTT sources. ASS supports rich styling
    /// (colours, bold, italic, positioning, background vocal styles)
    /// and is preferred by advanced media players (VLC, mpv, MPC-HC).
    #[serde(default)]
    pub generate_ass: bool,

    /// When enabled, generates Lyricsfile (`.lyrics`) YAML sidecars from
    /// TTML during enrichment Step 2g. Lyricsfile is the open,
    /// extensible lyrics format jointly endorsed by LRCGET and LRCLIB
    /// (released in LRCGET v2.0.0). It supports word-level timing,
    /// overlapping vocal lines, and is plain-text-editable in any
    /// editor — the YAML alternative to Apple's TTML.
    ///
    /// **Default: off.** The format is officially marked experimental
    /// by its upstream maintainers ("expect breaking changes in future
    /// versions as the specification is refined"). Opt-in users
    /// understand the format may churn until LRCGET 2.x stabilises.
    ///
    /// When MeedyaDL has both TTML and word-level timing data, the
    /// Lyricsfile sidecar preserves that fidelity in a vendor-neutral
    /// format that LRCGET and LRCLIB consume directly. Implemented
    /// via the shared `meedya-lyrics` crate (MeedyaSuite-core#34).
    #[serde(default)]
    pub generate_lyricsfile: bool,

    /// When enabled, appends `[Explicit]` or `[Clean]` to album folder
    /// names and individual track filenames based on Apple Music content
    /// ratings. This helps distinguish Explicit and Clean versions of
    /// the same album on disk. Applied during metadata enrichment after
    /// download completes (requires MusicKit credentials for API access).
    /// Only affects file/folder naming — embedded metadata tags are
    /// written separately via tags.toml (`AlbumAdvisory`, `iTunesAdvisory`).
    #[serde(default = "default_true")]
    pub content_advisory_in_filenames: bool,

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
    /// a value of 10000 requests the highest available resolution — Apple
    /// Music's CDN returns what it has (typically up to ~3000x3000).
    /// Maps to `GamdlOptions::cover_size` / GAMDL `--cover-size`.
    pub cover_size: u32,

    /// Filename for the saved cover art image (without extension). GAMDL
    /// writes `Cover.<ext>` by default; MeedyaDL renames the file after
    /// download to match this setting. Default: `FrontCover` for consistency
    /// with animated artwork naming (FrontCover.mp4). (#448)
    #[serde(default = "default_cover_art_name")]
    pub cover_art_name: CoverArtName,

    /// Embed the music-video cover thumbnail as a `covr` atom in the
    /// `.mp4` and delete the sidecar `.jpg`/`.png` (#533 / #569).
    /// Default `true` — most users see the embedded poster frame in
    /// every modern player and never need the sidecar. Flip to `false`
    /// to keep the sidecar on disk (e.g. for tooling that expects a
    /// visible thumbnail next to the video).
    #[serde(default = "default_true")]
    pub music_video_embed_cover_sidecar: bool,

    // ================================================================
    // Animated Artwork (Motion Cover Art)
    // ================================================================
    /// Whether to download animated cover art (motion artwork) from Apple
    /// Music after each album download. When enabled, `MeedyaDL` queries the
    /// Apple Music catalog API (`extend=editorialVideo`) and saves
    /// `FrontCover.mp4` (square, 1:1) and `FrontCoverPortrait.mp4` (portrait,
    /// 3:4) alongside the audio files, if animated artwork is available.
    ///
    /// Requires valid `MusicKit` credentials (`musickit_team_id`,
    /// `musickit_key_id`, and a private key stored in the OS keychain).
    pub animated_artwork_enabled: bool,

    /// Whether to set the OS "hidden" attribute on downloaded animated
    /// artwork files (FrontCover.mp4, FrontCoverPortrait.mp4). When `true`
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

    /// When enabled, downloads the artist's promotional video (editorial
    /// motion art) from Apple Music and saves it as `ArtistSpotlightCover.mp4` in
    /// the artist folder (parent of the album directory). These are the
    /// animated backgrounds shown on Apple Music artist pages. Requires
    /// valid `MusicKit` credentials. Default: `false`.
    #[serde(default)]
    pub artist_promo_video_enabled: bool,

    /// When enabled, MeedyaDL fetches the static cover art for an
    /// album from every supported platform in parallel (Apple Music,
    /// Spotify, future MusicBrainz / Tidal / Bandcamp) and embeds the
    /// **highest-resolution** candidate into the audio file —
    /// regardless of which platform the download itself came from.
    ///
    /// Tie-break (equal pixel area): Apple Music wins, since its
    /// maximum native artwork is consistently higher quality than
    /// the fall-back sources that match its dimensions in practice.
    ///
    /// Off by default — the feature is opt-in because it issues an
    /// extra HTTP call per platform, and most users are happy with
    /// the cover art the originating engine already wrote. Surface
    /// lives at Settings > Cover Art.
    ///
    /// See `services/best_cover_art_service.rs` for the comparator
    /// + tie-break logic (M9-3).
    #[serde(default)]
    pub best_cover_art_enabled: bool,

    /// Apple `MusicKit` Team ID for API authentication. This is the
    /// 10-character team identifier from the Apple Developer portal
    /// (e.g., `"ABCDE12345"`). Required when `animated_artwork_enabled`
    /// is `true`.
    pub musickit_team_id: Option<String>,

    /// Apple `MusicKit` Key ID for API authentication. This is the
    /// 10-character identifier for the `MusicKit` private key created in
    /// the Apple Developer portal (e.g., `"ABC123DEFG"`). Required when
    /// `animated_artwork_enabled` is `true`.
    ///
    /// **Note:** The private key itself (`.p8` file content) is stored
    /// securely in the OS keychain under the key `"musickit_private_key"`,
    /// NOT in this settings struct.
    pub musickit_key_id: Option<String>,

    // ================================================================
    // Metadata Enrichment (Opt-In)
    // ================================================================
    /// Enable `AcoustID` fingerprinting for downloaded tracks. When enabled,
    /// `MeedyaDL` generates Chromaprint audio fingerprints using the embedded
    /// rusty-chromaprint library and looks up `AcoustID` identifiers from
    /// acoustid.org after each download. No external tools required.
    /// CPU-intensive: each audio file must be fully decoded to generate the
    /// fingerprint, and each lookup requires a network request.
    #[serde(default)]
    pub acoustid_enabled: bool,

    /// Application API key for `AcoustID` fingerprint lookups. Register a free
    /// application key at <https://acoustid.org/new-application>. When empty,
    /// `AcoustID` lookups are skipped (fingerprints are not generated).
    /// This is a public application identifier (not a secret).
    #[serde(default)]
    pub acoustid_api_key: String,

    /// Enable `ReplayGain` loudness analysis for downloaded tracks. When enabled,
    /// `MeedyaDL` analyses each audio file's loudness using `FFmpeg`'s EBU R128
    /// filter and writes non-destructive `ReplayGain` metadata tags
    /// (`replaygain_track_gain`, `replaygain_track_peak`). This enables volume
    /// normalisation in media players that support `ReplayGain` (foobar2000,
    /// Kodi, VLC, etc.) without altering the actual audio data. Uses `FFmpeg`
    /// (already installed). CPU-intensive: `FFmpeg` must decode each file.
    #[serde(default)]
    pub replaygain_enabled: bool,

    /// `ReplayGain` reference loudness level in LUFS. Default: -18.0 (EBU R128).
    /// Common alternatives: -14.0 (Spotify-style), -23.0 (broadcast).
    #[serde(default = "default_replaygain_reference")]
    pub replaygain_reference_level: f64,

    /// When true, limits `ReplayGain` gain so that peak × gain never exceeds 1.0,
    /// preventing digital clipping on tracks that are already near 0 dBFS.
    #[serde(default = "default_true")]
    pub replaygain_prevent_clipping: bool,

    /// When true, computes and writes album-level `ReplayGain` tags
    /// (`replaygain_album_gain`, `replaygain_album_peak`) alongside track
    /// tags. Album gain preserves the intended dynamic range between quiet
    /// and loud tracks when listening to a full album in order. When false,
    /// only track-level `ReplayGain` tags are written. Default: true.
    #[serde(default = "default_true")]
    pub replaygain_album_gain: bool,

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

    /// Folder naming template for playlist downloads (#618).
    ///
    /// Default: `"Playlists/{playlist_artist}"` — matches GAMDL's own
    /// upstream default (`gamdl/downloader/base.py::playlist_folder_template`)
    /// so users who haven't customised the template see the same layout
    /// MeedyaDL has always produced.
    ///
    /// Gated on GAMDL ≥ v3.0 (when the corresponding CLI flag
    /// `--playlist-folder-template` was introduced). On v2.9.x the flag
    /// does not exist and emission would crash the subprocess with
    /// `no such option`. The gate lives in
    /// [`gamdl_capabilities::GamdlFeature::PlaylistFolderTemplate`] and is
    /// consulted by both `GamdlOptions::to_cli_args` and
    /// `config_service::ini_template_section`.
    #[serde(default = "default_playlist_folder_template")]
    pub playlist_folder_template: String,

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

    /// User-configurable zero-padding width for `{track}` placeholders
    /// (#587). `Auto` (the default) derives the width from the album's
    /// `track_total` at download time, producing sort-correct filenames
    /// for albums of any size. Fixed widths available for users who want
    /// uniform padding across their entire library.
    #[serde(default = "default_track_number_padding")]
    pub track_number_padding: TrackNumberPadding,

    /// User-configurable zero-padding width for `{disc}` placeholders
    /// (#587). Mirrors `track_number_padding` but scoped to disc
    /// numbers. `Auto` (the default) keeps the pre-#587 unpadded
    /// format for the common <10-disc case.
    #[serde(default = "default_disc_number_padding")]
    pub disc_number_padding: DiscNumberPadding,

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

    /// Custom `FFmpeg` binary path. `None` = use the managed `FFmpeg`
    /// installation (see `dependency.rs` and `commands/dependency.rs`).
    pub ffmpeg_path: Option<String>,

    /// Custom mp4decrypt binary path (from Bento4 toolkit). Used to
    /// decrypt Widevine-protected content.
    pub mp4decrypt_path: Option<String>,

    /// Custom `MP4Box` binary path (from GPAC). Alternative remuxer.
    pub mp4box_path: Option<String>,

    /// Custom N_m3u8DL-RE binary path. Alternative HLS downloader.
    pub nm3u8dlre_path: Option<String>,

    /// Custom MediaInfo CLI binary path. Used for accurate codec
    /// detection in the enrichment pipeline. `None` = use the managed
    /// installation or system PATH.
    pub mediainfo_path: Option<String>,

    // ================================================================
    // Advanced
    // ================================================================
    /// Download tool selection. See `DownloadMode` in `gamdl_options.rs`.
    /// Default: `Ytdlp` (yt-dlp) because it requires no additional binary.
    pub download_mode: DownloadMode,

    /// Remux tool selection. See `RemuxMode` in `gamdl_options.rs`.
    /// Default: `Mp4box` — better subtitle/CC handling in music videos.
    /// FFmpeg can fail with "Invalid data found when processing input" on
    /// videos with embedded subtitles/CC tracks.
    pub remux_mode: RemuxMode,

    /// Whether to use the wrapper authentication system for accessing
    /// DRM-protected content. When `false` (default), standard cookie-based
    /// authentication is used. Maps to `GamdlOptions::use_wrapper`.
    pub use_wrapper: bool,

    /// When `true` and a download that used wrapper authentication fails
    /// terminally (all retries exhausted), automatically re-queue the
    /// download with wrapper disabled (falls back to cookie-based auth).
    /// Only relevant when `use_wrapper` is `true`. Default: `false`.
    #[serde(default)]
    pub auto_retry_without_wrapper: bool,

    /// When `true`, a download that fails with the AMP API "Resource Not
    /// Found" shape against the URL's storefront is automatically retried
    /// once with the user's account-region storefront (`storefront`
    /// setting, falling back to OS locale, then `"us"`). Useful when the
    /// user pastes a `/us/album/X` link while their account region is
    /// `gb` and the album either isn't in the US catalog or their account
    /// can't license it from there. Default: `true` — it only fires when
    /// the primary attempt has *already* failed, so it can't downgrade an
    /// otherwise-working download. (#666)
    #[serde(default = "default_true")]
    pub storefront_fallback_on_failure: bool,

    /// Wrapper server URL used when `use_wrapper` is `true`. The wrapper
    /// server handles account authentication and key exchange. Default:
    /// `"http://127.0.0.1:30020"` (local server).
    pub wrapper_account_url: String,

    /// m3u8 server address (`host:port`) used by GAMDL v3.1+ to fetch the
    /// HLS master playlist from the wrapper service instead of Apple's
    /// API. Required for wrapper downloads on GAMDL 3.1+. Emitted as
    /// `--wrapper-m3u8-ip` / `wrapper_m3u8_ip` only when the detected
    /// GAMDL version supports it. Default: `"127.0.0.1:20020"` (matches
    /// upstream GAMDL's default).
    #[serde(default = "default_wrapper_m3u8_ip")]
    pub wrapper_m3u8_ip: String,

    /// Decryption server address (`host:port`) used by GAMDL when
    /// `use_wrapper` is `true`. GAMDL opens an outbound TCP connection
    /// to this address to send encrypted samples for FairPlay decryption
    /// (see `gamdl/downloader/amdecrypt.py::decrypt_samples` —
    /// `asyncio.open_connection(host, port)`). Required for the
    /// wrapper to be reachable when it's NOT running on the same host
    /// as MeedyaDL/GAMDL — the third leg of the wrapper triangle
    /// alongside `wrapper_account_url` and `wrapper_m3u8_ip`. Without
    /// this exposed, remote-wrapper LAN setups silently fail at the
    /// decryption stage because GAMDL falls back to its compile-time
    /// default of `127.0.0.1:10020` (issue #743). Default:
    /// `"127.0.0.1:10020"` (matches upstream GAMDL's default).
    ///
    /// **GAMDL ≥ v3.6 (#853):** unused. The three v1 sockets above
    /// (`wrapper_account_url`, `wrapper_m3u8_ip`, `wrapper_decrypt_ip`)
    /// are replaced by the single [`Self::wrapper_url`] field pointing
    /// at the wrapper-v2 HTTP daemon. These fields remain in the
    /// settings file for users still running GAMDL ≤ 3.5.x.
    #[serde(default = "default_wrapper_decrypt_ip")]
    pub wrapper_decrypt_ip: String,

    /// Wrapper-v2 HTTP base URL (#853). Used when `use_wrapper` is
    /// `true` AND the detected GAMDL release is ≥ 3.6. Replaces the
    /// three wrapper-v1 socket addresses above with a single REST
    /// endpoint exposing `/health`, `/me`, `/playback`, `/decrypt`,
    /// `/login`, `/login/2fa`, `DELETE /login` per the
    /// [wrapper-v2 spec](https://github.com/glomatico/wrapper-v2).
    ///
    /// Default: `"http://127.0.0.1"` (matches upstream GAMDL v3.6's
    /// default and the wrapper-v2 `compose.yaml`'s `${HTTP_PORT:-80}:80`
    /// port mapping — i.e. the implicit `:80` after the host).
    ///
    /// MeedyaDL emits exactly one of v1 (three URLs) or v2 (this one
    /// URL) per CLI invocation, gated on `GamdlFeature::WrapperUrl`.
    #[serde(default = "default_wrapper_url")]
    pub wrapper_url: String,

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
    // Artist
    // ================================================================
    /// Default artist auto-selection mode when downloading from artist URLs.
    /// Controls which content type is automatically downloaded. New in GAMDL 2.9.1.
    /// Default: `None` (omit the flag, let GAMDL use its own default behavior).
    #[serde(default)]
    pub artist_auto_select: Option<ArtistAutoSelect>,

    /// Multiple artist auto-selection modes for artist URL downloads.
    /// When non-empty, this takes precedence over `artist_auto_select` and
    /// causes MeedyaDL to create one download queue item per selected mode.
    /// Each item runs GAMDL with `--artist-auto-select <mode>` separately.
    ///
    /// This is a MeedyaDL-internal feature (GAMDL only accepts a single
    /// `--artist-auto-select` value). MeedyaDL splits the request into N
    /// independent downloads to achieve multi-mode behavior.
    ///
    /// Default: empty (falls back to `artist_auto_select` for single-mode).
    #[serde(default)]
    pub artist_auto_select_multi: Vec<ArtistAutoSelect>,

    /// Pre-queue duplicate-detection settings (#510).
    ///
    /// When a multi-mode artist URL is fanned out into N downloads, songs
    /// that exist in multiple modes (e.g. the same track on an album AND a
    /// compilation) would otherwise be downloaded multiple times at the
    /// same quality. This setting controls the dedup strategy.
    ///
    /// Does NOT affect companion-format downloads — a song chosen from one
    /// mode still runs the full `companion_mode` chain.
    #[serde(default)]
    pub duplicate_detection: DuplicateDetectionSettings,

    // ================================================================
    // Crash Reporting & Telemetry
    // ================================================================
    /// Whether to send anonymous crash reports to Sentry (opt-in telemetry).
    /// When `true`, the Sentry SDK is initialised at startup and captures
    /// panics, `tracing::error!()` events, and breadcrumbs. When `false`
    /// (the default), no data is ever sent -- crash reports are only saved
    /// locally to `{app_data_dir}/crashes/`.
    ///
    /// Controlled in Settings > Advanced > Crash Reporting.
    #[serde(default)]
    pub sentry_enabled: bool,

    /// When enabled, emits detailed diagnostic messages to the Activity Log
    /// including URLs, file paths, API responses, error details, and
    /// enrichment step internals. Prefixed with `[VERBOSE]` in the log.
    ///
    /// **WARNING**: Verbose output may contain sensitive information such as
    /// cookie file paths, wrapper URLs with authentication tokens, Apple Music
    /// API responses, and MusicKit credentials. Only enable when collecting
    /// detailed logs for issue tracking; disable before sharing logs.
    ///
    /// **Reset behaviour** (version-aware):
    /// - **Pre-release versions** (v0.x.x): The setting is **preserved** across
    ///   restarts because verbose logging is critical for debugging pre-release
    ///   issues. Users can still toggle it off manually.
    /// - **Full/public releases** (v1.0.0+): The setting is **reset to `false`**
    ///   on every startup as a safety measure to prevent sensitive data from
    ///   being logged permanently by accident.
    /// - **Upgrade from pre-release → full release**: The setting is reset to
    ///   `false` on the first launch of the full release version.
    ///
    /// Controlled in Settings > Advanced > Diagnostics.
    #[serde(default)]
    pub verbose_activity_log: bool,

    /// When `true`, GAMDL prints full Python tracebacks to stderr on
    /// uncaught exceptions. When `false` (default), MeedyaDL passes
    /// `--no-exceptions` to every GAMDL invocation so only the final
    /// one-line error message reaches the activity log.
    ///
    /// Default is `false` because GAMDL v3.0's structlog migration
    /// interleaves structured log lines with raw multi-line tracebacks,
    /// which turns the activity log into an unreadable blob and makes
    /// `classify_error()` match the wrong keyword (e.g. picking up
    /// "Error" from a traceback filepath like
    /// `httpx/_transports/default.py`).
    ///
    /// Flip this on when filing upstream bug reports against GAMDL —
    /// you lose a clean activity log but gain the full call stack.
    ///
    /// **GAMDL v3.1 compatibility note (#606):** Upstream commit
    /// `dc6f2e8` removed every `traceback.print_exc()` site and routes
    /// exceptions through `structlog.ExceptionPrettyPrinter` instead.
    /// `--no-exceptions` is a no-op on v3.1+, so flipping this setting
    /// does not change activity-log verbosity on that release. The
    /// MeedyaDL output parser handles the new format; see #607.
    ///
    /// Controlled in Settings > Advanced > Diagnostics.
    #[serde(default)]
    pub verbose_gamdl_exceptions: bool,

    /// GAMDL subprocess log level (`--log-level <LEVEL>`).
    ///
    /// Surfaced behind the Developer Tools panel only (gated on
    /// `dev_access_enabled` via the Konami sentinel), so it doesn't
    /// confuse end users with a knob that mostly just creates activity-
    /// log noise.
    ///
    /// **Why this exists (#768):** GAMDL v3.5.2 (commit `dec4a22`,
    /// "Bind logger and log m3u8 master URL extraction") added a
    /// `log.debug("success", m3u8_master_url=...)` call inside the
    /// music-video pipeline that is silent at GAMDL's default `INFO`
    /// level. Future v3.x releases follow the same pattern as upstream
    /// invests more in structlog instrumentation. Without this field a
    /// developer hitting "music videos download to the wrong folder"
    /// on v3.5.2+ has no in-app way to enable DEBUG output — they
    /// have to fork settings.json or shell out to gamdl directly.
    ///
    /// **Default `Info`** matches GAMDL's compiled-in default, so this
    /// field is a no-op for users who never open Developer Tools.
    /// `merge_options()` in `download_queue.rs` copies the value into
    /// `GamdlOptions.log_level`, which `to_cli_args()` then emits as
    /// `--log-level <LEVEL>` regardless of GAMDL version (the flag
    /// exists on every release in our support window). The serde
    /// `default = "default_gamdl_log_level"` helper makes settings.json
    /// files written by pre-#768 builds load unchanged.
    #[serde(default = "default_gamdl_log_level")]
    pub gamdl_log_level: LogLevel,

    /// Optional user-chosen directory for the persistent on-disk
    /// activity log (`activity-YYYY-MM-DD.log` files, #541).
    ///
    /// When empty / absent, the writer falls back to the default
    /// `{app_data_dir}/logs/` location alongside the tracing and
    /// session logs. When set, the writer opens log files under the
    /// configured directory instead — useful for pointing logs at an
    /// external drive to save space on the system disk.
    ///
    /// Changes apply on the next app restart because the writer is
    /// started once during `setup()` and owns the file handle for
    /// the process lifetime. Relocating mid-session would require
    /// tearing down the writer, which is not worth the complexity.
    ///
    /// Controlled in Settings > Advanced > Diagnostics.
    #[serde(default)]
    pub activity_log_path_override: String,

    // ================================================================
    // Internal / Developer
    // ================================================================
    /// Internal developer access mode. When enabled, unlocks enhanced
    /// features such as token status dashboard, debug diagnostics, and
    /// experimental capabilities. Not visible in the normal Settings UI —
    /// activated via a hidden gesture or keychain sentinel value.
    #[serde(default)]
    pub dev_access_enabled: bool,

    /// Spotify-download consent acknowledgment (M9-4).
    ///
    /// Spotify's terms of service prohibit automated downloads, and
    /// accounts have been suspended in the wild for obvious bot
    /// behaviour. Even with `dev_access_enabled` on, the first
    /// Spotify queue attempt prompts the user to acknowledge the
    /// account-ban risk; that acknowledgment is persisted here so
    /// the modal doesn't recur.
    ///
    /// Defaults to `false`. The acknowledge IPC flips it to `true`;
    /// there's no UI affordance to flip it back, so users who want
    /// to disable Spotify must use the Settings > Services >
    /// Spotify toggle (which is independent of this flag).
    #[serde(default)]
    pub spotify_consent_acknowledged: bool,

    // ================================================================
    // Application State
    // ================================================================
    /// The last app version the user launched. Compared against the current
    /// `CARGO_PKG_VERSION` on startup to detect version changes (e.g., for
    /// showing a pre-release first-load notice or resetting verbose logging
    /// when upgrading from pre-release → full release).
    ///
    /// Empty string = first run (no previous version recorded).
    #[serde(default)]
    pub last_seen_version: String,

    /// Whether the first-run setup wizard has been completed at least once.
    /// When `true`, the app skips the wizard on startup even if some
    /// dependencies are missing (shows a warning banner instead). This
    /// prevents the wizard from re-appearing after app updates that might
    /// temporarily break tool detection.
    #[serde(default)]
    pub setup_completed: bool,

    /// Whether the user has accepted the Terms of Service / EULA.
    /// Shown as a modal on first launch before the setup wizard.
    #[serde(default)]
    pub terms_accepted: bool,

    /// Whether the crash report opt-in prompt has been shown to the user.
    /// Set to `true` after the first-launch prompt, regardless of the user's
    /// choice — prevents re-prompting.
    #[serde(default)]
    pub crash_report_prompt_shown: bool,

    /// Whether anonymous usage analytics are enabled (opt-in).
    /// When true, anonymised feature usage data is sent to help prioritise development.
    /// No personal data, URLs, or content information is ever collected.
    #[serde(default)]
    pub analytics_enabled: bool,

    /// Whether BPM (tempo) analysis is enabled during enrichment.
    /// When true, audio files are analysed for BPM and the result is
    /// written as a metadata tag. Default: false (opt-in).
    #[serde(default)]
    pub bpm_analysis_enabled: bool,

    // ================================================================
    // Per-Service Settings (#319)
    // ================================================================
    // Service-specific configuration nested under a single field.
    // During the migration period, existing flat fields (storefront,
    // cookies_path, musickit_*, etc.) are still the primary source.
    // New code should read from service_settings where possible.
    /// Per-service settings (Apple Music, Spotify, YouTube).
    /// Backwards-compatible: missing from older settings.json files,
    /// defaults to empty service settings.
    #[serde(default)]
    pub service_settings: PerServiceSettings,

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

    /// When enabled, applies a high-contrast accessibility theme that
    /// increases visual contrast across all UI elements. Works in both
    /// light and dark modes. Text uses pure black/white, borders are
    /// stronger, status colours are more saturated, and focus indicators
    /// are thick and visible. Also auto-activates when the OS-level
    /// `prefers-contrast: high` media query is detected.
    ///
    /// Controlled in Settings > General > Appearance.
    #[serde(default)]
    pub high_contrast: bool,

    /// Colour vision deficiency (CVD) simulation mode. Remaps status
    /// colours (success, error, warning, info) to palettes that are
    /// distinguishable for users with specific types of colour blindness.
    ///
    /// Valid values:
    /// - `""` (empty string) -- Disabled; use standard status colours.
    /// - `"deuteranopia"` -- Red-green colour blindness (most common, ~6% of males).
    /// - `"protanopia"` -- Red-green colour blindness (reduced red sensitivity).
    /// - `"tritanopia"` -- Blue-yellow colour blindness (rare).
    ///
    /// Controlled in Settings > General > Appearance.
    #[serde(default)]
    pub colour_blind_mode: String,

    // ================================================================
    // After-Queue Actions
    // ================================================================

    /// Persistent after-queue action (applies to every queue completion).
    /// Default: `DoNothing`.
    #[serde(default)]
    pub after_queue_action: AfterQueueAction,

    /// One-shot after-queue action (applies to the next queue completion only,
    /// then resets to `None`). Overrides `after_queue_action` when set.
    #[serde(default)]
    pub after_queue_once: Option<AfterQueueAction>,
}

/// Action to perform after the download queue finishes processing.
///
/// Each variant maps to an OS-level command. Unsupported actions on a given
/// platform should be greyed out in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AfterQueueAction {
    /// Take no action after queue completion.
    #[default]
    DoNothing,
    /// Open the output folder in the system file manager.
    OpenOutputFolder,
    /// Play a system notification sound.
    PlaySound,
    /// Quit the MeedyaDL application.
    CloseMeedyadl,
    /// Restart the computer.
    RestartComputer,
    /// Hibernate the computer (suspend to disk). Not available on all platforms.
    HibernateComputer,
    /// Shut down the computer.
    ShutdownComputer,
}

/// Serde default helper: returns `true` for `auto_start_queue`.
/// Used by `#[serde(default = "default_auto_start_queue")]` because
/// `bool::default()` returns `false`, but we need `true` for backward
/// compatibility (existing users expect downloads to start automatically).
const fn default_auto_start_queue() -> bool {
    true
}

/// Default periodic update check interval: every 6 hours.
/// A relatively frequent default is appropriate during early development
/// when updates may contain important fixes. Value `0` = startup only.
const fn default_update_interval() -> u32 {
    6
}

/// Default auto-dismiss duration for transient notifications: 5 seconds.
const fn default_notification_dismiss() -> u32 {
    5
}

/// Default GAMDL idle-output timeout: 5 minutes. The companion
/// supervisor kills the child after this many minutes of stdout/stderr
/// silence while still in the download phase. The watchdog stands down
/// once a `100% of` line is observed so the silent post-processing
/// phase doesn't trigger a false kill.
const fn default_gamdl_idle_timeout() -> u32 {
    5
}

/// Default notification style: native + in-app (both).
fn default_notification_style() -> String {
    "native_and_in_app".to_string()
}

/// Default wrapper m3u8 service address — matches upstream GAMDL v3.1's
/// `AppleMusicBaseInterface.create(wrapper_m3u8_ip="127.0.0.1:20020")`.
fn default_wrapper_m3u8_ip() -> String {
    "127.0.0.1:20020".to_string()
}

/// Default wrapper-v2 HTTP base URL (#853).
///
/// Matches upstream GAMDL v3.6's `WrapperApi.create(base_url=
/// "http://127.0.0.1")` default and the [wrapper-v2](https://github.com/glomatico/wrapper-v2)
/// `compose.yaml` port mapping `${HTTP_PORT:-80}:80` (i.e. the implicit
/// `:80` after `127.0.0.1`).
fn default_wrapper_url() -> String {
    "http://127.0.0.1".to_string()
}

/// Default wrapper decryption service address — matches upstream
/// GAMDL's `AppleMusicSongInterface.create(wrapper_decrypt_ip=
/// "127.0.0.1:10020")` and the `WorldObservationLog/wrapper`
/// service's default decrypt port (10020).
fn default_wrapper_decrypt_ip() -> String {
    "127.0.0.1:10020".to_string()
}

/// Current settings schema version.
/// Increment this when making backwards-incompatible changes to AppSettings.
pub const CURRENT_SETTINGS_VERSION: u32 = 7;

impl Default for AppSettings {
    /// Creates default settings that match the project brief requirements.
    ///
    /// ## Design rationale for key defaults
    ///
    /// - **`default_song_codec: Alac`** -- The project brief prioritises
    ///   maximum audio quality, so we default to lossless ALAC.
    /// - **`default_video_resolution: P2160`** -- Same reasoning: highest
    ///   available quality (4K UHD).
    /// - **`cover_format: Jpg`** -- JPEG is the safest default because
    ///   GAMDL 2.8.4 has a bug in `get_cover_file_extension()` that crashes
    ///   when `cover_format` is `Raw`. JPEG still provides high-quality
    ///   artwork at the requested resolution. Users can switch to Raw once
    ///   the upstream bug is fixed.
    /// - **`fallback_enabled: true`** -- Ensures downloads succeed even
    ///   when the preferred codec/resolution is not available. The project
    ///   brief explicitly defines the fallback chains below.
    /// - **`music_fallback_chain`** -- ALAC -> Atmos -> AC3 -> AAC Binaural
    ///   -> AAC -> AAC Legacy. This descends from lossless through spatial
    ///   audio to standard lossy, matching the project brief's order.
    /// - **`video_fallback_chain`** -- 2160p -> 1440p -> ... -> 240p.
    ///   Every resolution Apple Music offers, in descending order.
    /// - **`synced_lyrics_format: Ttml`** -- TTML preserves Apple Music's
    ///   word-level timing data for Enhanced LRC conversion. For music
    ///   videos, the download manager also uses TTML.
    /// - **`output_path: ""`** -- An empty string signals the app to use
    ///   the platform's default Music directory at runtime (resolved by
    ///   `dirs::audio_dir()` or equivalent).
    /// - **Templates** -- Use GAMDL's own default templates so that files
    ///   are organized identically to a standalone GAMDL installation.
    #[allow(clippy::literal_string_with_formatting_args)] // GAMDL template strings, not Rust format args
    fn default() -> Self {
        Self {
            // Schema version for migration support
            settings_version: CURRENT_SETTINGS_VERSION,

            // --- General ---
            // Empty string = resolve to platform Music dir at runtime.
            output_path: String::new(),
            // Empty string = resolve to {OS temp dir}/MeedyaDL at runtime.
            // Users can override in Settings > Paths.
            temp_path: String::new(),
            // English (US) metadata by default; users in other regions
            // can change this to get localized track/album names.
            language: "en-US".to_string(),
            // Empty = auto-detect from language region code (e.g., en-US → us).
            storefront: String::new(),
            // Do not overwrite by default to prevent accidental data loss.
            overwrite: false,
            // Auto-detect UI language from OS locale by default.
            ui_language: String::new(),
            // Check for updates on launch so users get security/bug fixes.
            auto_check_updates: true,
            // Only show stable releases by default. Pre-releases may have
            // incomplete features or bugs and are for testers/developers.
            check_pre_releases: false,
            // Default to the stable channel; users opt into less-stable
            // channels explicitly from Settings > General > Updates.
            update_channel: UpdateChannel::Stable,
            // Check for updates every 6 hours during early development.
            // Users can change to 1/12/24 hours or 0 (startup only).
            update_check_interval_hours: 6,
            // 5-minute idle window matches GAMDL's typical worst-case
            // segment-download latency before something is genuinely wedged.
            gamdl_idle_timeout_minutes: default_gamdl_idle_timeout(),
            // Auto-start queue processing by default so downloads begin
            // immediately. When disabled, items stay queued until the user
            // manually triggers processing from the Queue page.
            auto_start_queue: true,
            abort_queue_confirm: true,
            // Desktop notifications enabled by default — OS-native alerts
            // for download completion and failure when the window is not focused.
            desktop_notifications: true,
            notification_style: "native_and_in_app".to_string(),
            notification_auto_dismiss_seconds: 5,
            smart_redownload_detection: true,
            clipboard_monitoring: true,

            // --- Audio quality ---
            // Default to the highest-quality codec (lossless ALAC).
            default_song_codec: SongCodec::Alac,

            // --- Video quality ---
            // Default to 4K with H.265 preferred, H.264 as fallback codec.
            default_video_resolution: VideoResolution::P2160,
            default_video_codec_priority: "h265,h264".to_string(),
            // m4v is Apple's preferred container on macOS; mp4 is more
            // universally compatible on Windows and Linux.
            default_video_remux_format: if cfg!(target_os = "macos") {
                "m4v"
            } else {
                "mp4"
            }
            .to_string(),

            // --- Fallback chains (as specified in the project brief) ---
            fallback_enabled: true,
            music_fallback_chain: vec![
                SongCodec::Alac,        // 1. Lossless (ALAC) -- highest quality
                SongCodec::Atmos,       // 2. Dolby Atmos -- spatial audio
                SongCodec::Ac3,         // 3. Dolby Digital (AC3) -- surround
                SongCodec::AacBinaural, // 4. AAC (256kbps) Binaural -- spatial stereo
                SongCodec::Aac,         // 5. AAC (256kbps at up to 48kHz) -- standard lossy
                SongCodec::AacLegacy, // 6. AAC Legacy (256kbps at up to 44.1kHz) -- broadest compat
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
            // No custom companion codecs — only relevant in Custom mode.
            custom_companion_codecs: Vec::new(),
            // Music video companions disabled by default — experimental feature.
            music_video_companion: false,
            // MusicBrainz lookup disabled by default — opt-in for video discovery fallback.
            musicbrainz_lookup: false,
            odesli_lookup_enabled: false,
            odesli_api_key: String::new(),

            // --- Lyrics ---
            // Enabled by default: embed lyrics in audio metadata.
            embed_lyrics_and_sidecar: true,
            // Keep sidecar files alongside embedded lyrics by default.
            keep_lyrics_sidecar: true,
            // TTML is the primary format because it preserves Apple Music's
            // word-level timing data for Enhanced LRC conversion.
            synced_lyrics_format: LyricsFormat::Ttml,
            // Download lyrics by default (they are small and useful).
            no_synced_lyrics: false,
            // Download audio + lyrics, not lyrics-only.
            synced_lyrics_only: false,
            // SRT as companion format for video player subtitle compatibility.
            companion_lyrics_formats: vec![LyricsFormat::Srt],
            // Enabled by default: converts TTML to Enhanced LRC with
            // word-by-word synchronised timestamps. Falls back to
            // line-level LRC for songs without word-level timing.
            enhanced_lrc: true,
            // Lyrics fallback enabled by default — if TTML isn't available,
            // try LRC (audio) or SRT (video) automatically.
            lyrics_fallback_enabled: true,
            // WebVTT generation off by default — opt-in for users who need it.
            generate_webvtt: false,
            // Rich SRT on by default — strictly improves SRT quality from TTML.
            generate_rich_srt: true,
            // Subtitle embedding off by default — opt-in for users who want
            // subtitles embedded in media containers.
            embed_subtitles: false,
            // ASS subtitle generation off by default — niche format.
            generate_ass: false,
            // Lyricsfile (.lyrics) generation off by default — the
            // format is experimental per LRCGET 2.0 release notes;
            // opt-in until upstream spec stabilises (#596).
            generate_lyricsfile: false,
            // Content advisory suffixes on by default — helps distinguish
            // Explicit vs Clean versions of the same album on disk.
            content_advisory_in_filenames: true,

            // --- Cover art ---
            // Save cover art by default -- most users want artwork files.
            save_cover: true,
            // JPEG = high-quality artwork. Raw is preferred but GAMDL 2.8.4
            // has a bug in get_cover_file_extension() that crashes with Raw format.
            cover_format: CoverFormat::Jpg,
            // 10000px requests the highest available resolution from Apple Music's
            // CDN. The CDN returns the largest version it has (typically 3000x3000),
            // so this effectively means "give me the best you have".
            cover_size: 10000,
            // Rename GAMDL's "Cover" to "FrontCover" for consistency with
            // animated artwork (FrontCover.mp4, FrontCoverPortrait.mp4).
            cover_art_name: CoverArtName::FrontCover,
            // #569: embed MV cover sidecar into the .mp4 as a covr
            // atom and delete the sidecar on success — keeps the
            // poster frame, removes the redundant loose .jpg files.
            music_video_embed_cover_sidecar: true,

            // --- Animated artwork ---
            // Enabled by default (#449): animated artwork is downloaded when
            // available and credentials are configured. Falls back gracefully
            // when MusicKit credentials are missing or album has no artwork.
            animated_artwork_enabled: true,
            // Show animated artwork files by default (#449) so users can see
            // FrontCover.mp4/FrontCoverPortrait.mp4 in their album folders.
            hide_animated_artwork: false,
            // Enabled by default (#453): downloads artist promo video to artist
            // folder when available. Gracefully skips when no credentials or
            // no promo video exists. Skipped for compilation albums.
            artist_promo_video_enabled: true,
            // Off by default (M9-3): opt-in cross-platform cover-art
            // resolution race. The feature issues one extra HTTP call
            // per non-Apple platform per album, so we don't enable it
            // on every download by default — users who want the
            // highest-fidelity artwork can flip it on in Settings >
            // Cover Art.
            best_cover_art_enabled: false,
            musickit_team_id: None,
            musickit_key_id: None,

            // --- Metadata enrichment (opt-in) ---
            // Both disabled by default: they are CPU-intensive post-processing
            // features that decode each audio file. Users enable them in the
            // Metadata settings tab when they want the extra tags.
            acoustid_enabled: false,
            acoustid_api_key: String::new(),
            replaygain_enabled: false,
            replaygain_reference_level: default_replaygain_reference(),
            replaygain_prevent_clipping: true,
            replaygain_album_gain: true,

            // --- Templates ---
            // These match GAMDL's built-in defaults for familiar organization,
            // with two divergences where the upstream default lacks
            // uniqueness:
            //   - `playlist_file_template` adds `{playlist_id}` so two
            //     playlists with the same artist + title don't clobber
            //     each other's `.m3u8` file (#545).
            //   - `compilation_folder_template` adds `{album_id}` so two
            //     Various-Artists compilations with the same title don't
            //     intermix in one folder (#552).
            // Both IDs are Apple Music's numeric identifiers — unique per
            // release/playlist, deterministic across re-downloads, no
            // datetime foot-guns (same rationale as the MV `{title_id}`
            // fix in #531).
            album_folder_template: "{album_artist}/{album}".to_string(),
            compilation_folder_template: "Compilations/{album} ({album_id})".to_string(),
            no_album_folder_template: "{artist}/Unknown Album".to_string(),
            playlist_folder_template: default_playlist_folder_template(),
            single_disc_file_template: "{track:02d} {title}".to_string(),
            multi_disc_file_template: "{disc}-{track:02d} {title}".to_string(),
            no_album_file_template: "{title}".to_string(),
            playlist_file_template: "Playlists/{playlist_artist}/{playlist_title} ({playlist_id})".to_string(),
            track_number_padding: TrackNumberPadding::Auto,
            disc_number_padding: DiscNumberPadding::Auto,

            // --- Tool paths ---
            // All None = use managed (auto-installed) tools from the app's
            // data directory. See `commands/dependency.rs` for the management logic.
            cookies_path: None,
            ffmpeg_path: None,
            mp4decrypt_path: None,
            mp4box_path: None,
            nm3u8dlre_path: None,
            mediainfo_path: None,

            // --- Advanced ---
            // yt-dlp is the default downloader because it is installed as
            // a Python dependency alongside GAMDL (no extra binary needed).
            download_mode: DownloadMode::Ytdlp,
            // MP4Box is the default remuxer — handles subtitle/CC tracks in
            // music videos better than FFmpeg (avoids "Invalid data" errors).
            remux_mode: RemuxMode::Mp4box,
            // Wrapper is disabled by default. Most users use cookie-based
            // auth. The wrapper is an advanced feature for accessing
            // certain DRM-protected streams.
            use_wrapper: false,
            // Off by default — user must opt in to automatic wrapper fallback.
            auto_retry_without_wrapper: false,
            // On by default (#666) — only fires after the primary URL fails,
            // so it can never downgrade a working download.
            storefront_fallback_on_failure: true,
            // Default wrapper URL assumes a locally-running server.
            wrapper_account_url: "http://127.0.0.1:30020".to_string(),
            // Default wrapper m3u8 service address (GAMDL v3.1+). Matches
            // upstream's default port 20020.
            wrapper_m3u8_ip: default_wrapper_m3u8_ip(),
            // Default wrapper decryption service address. Matches upstream
            // GAMDL's default port 10020. Override in Settings > Advanced
            // when the wrapper runs on a different host (#743).
            wrapper_decrypt_ip: default_wrapper_decrypt_ip(),
            // Default wrapper-v2 HTTP base URL (#853). Used in place of
            // the three v1 sockets above when the detected GAMDL release
            // is ≥ 3.6.
            wrapper_url: default_wrapper_url(),
            // No filename truncation by default (OS limits still apply).
            truncate: None,
            // Fetch extra metadata (normalization, smooth playback info, etc.)
            // by default. Richer metadata is worth the small extra API overhead.
            fetch_extra_tags: true,
            // No tags excluded by default -- embed all available metadata.
            exclude_tags: Vec::new(),

            // --- Artist ---
            // No auto-selection by default: omit the flag so GAMDL uses its
            // own default behavior when the user provides an artist URL.
            artist_auto_select: None,
            // No multi-mode artist selection by default.
            artist_auto_select_multi: Vec::new(),

            // --- Duplicate detection (#510) ---
            // Default: on, with preference main-albums > singles-eps > compilation-albums
            // > live-albums > top-songs; scope covers intra-session + already-queued items.
            // Applies only to multi-mode artist URL expansion; other downloads unaffected.
            duplicate_detection: DuplicateDetectionSettings::default(),

            // --- Crash reporting ---
            // Sentry is disabled by default (opt-in). No data is sent until
            // the user explicitly enables it in Settings > Advanced.
            sentry_enabled: false,
            // Verbose activity log disabled by default — may expose sensitive data.
            verbose_activity_log: false,
            // GAMDL tracebacks suppressed by default — structlog-wrapped
            // stderr is unreadable with raw tracebacks interleaved. Users
            // debugging upstream issues can flip this in Settings > Advanced.
            verbose_gamdl_exceptions: false,
            // GAMDL log level — matches GAMDL's compiled-in default.
            // Only relevant to users who flip this from Developer Tools
            // (#768); for everyone else it's a no-op that GAMDL would
            // pick anyway.
            gamdl_log_level: LogLevel::Info,
            // Empty string = use default {app_data_dir}/logs/. Users can
            // point the on-disk activity log at an external drive via
            // Settings > Advanced > Diagnostics.
            activity_log_path_override: String::new(),

            // --- Internal / Developer ---
            // Developer access is disabled by default; activated via hidden gesture.
            dev_access_enabled: false,
            // M9-4: first-run consent for Spotify downloads — off by default;
            // the IPC `acknowledge_spotify_consent` flips it to true after
            // the user accepts the account-ban-risk modal.
            spotify_consent_acknowledged: false,

            // --- Application state ---
            // No previous version on first run; populated by load_settings().
            last_seen_version: String::new(),
            // Setup wizard has not been completed yet on a fresh install.
            setup_completed: false,
            terms_accepted: false,
            crash_report_prompt_shown: false,
            analytics_enabled: false,
            bpm_analysis_enabled: false,

            // --- Per-service settings ---
            // Default service settings (all services use their own defaults).
            service_settings: PerServiceSettings::default(),

            // --- UI state ---
            // Sidebar expanded by default for discoverability.
            sidebar_collapsed: false,
            // Auto-detect theme from OS settings.
            theme_override: None,
            // High-contrast accessibility theme disabled by default.
            // Auto-activates via the prefers-contrast: high media query.
            high_contrast: false,
            // Colour vision deficiency mode disabled by default.
            // Users can enable deuteranopia, protanopia, or tritanopia
            // in Settings > General > Appearance to remap status colours.
            colour_blind_mode: String::new(),

            // After-Queue Actions
            after_queue_action: AfterQueueAction::DoNothing,
            after_queue_once: None,
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
        assert_eq!(
            deserialized.default_video_resolution,
            settings.default_video_resolution
        );
        assert_eq!(
            deserialized.default_video_codec_priority,
            settings.default_video_codec_priority
        );
        assert_eq!(
            deserialized.default_video_remux_format,
            settings.default_video_remux_format
        );

        // Fallback
        assert_eq!(deserialized.fallback_enabled, settings.fallback_enabled);
        assert_eq!(
            deserialized.music_fallback_chain.len(),
            settings.music_fallback_chain.len()
        );
        assert_eq!(
            deserialized.video_fallback_chain.len(),
            settings.video_fallback_chain.len()
        );

        // Companion downloads
        assert_eq!(deserialized.companion_mode, settings.companion_mode);

        // Lyrics
        assert_eq!(
            deserialized.synced_lyrics_format,
            settings.synced_lyrics_format
        );
        assert_eq!(deserialized.no_synced_lyrics, settings.no_synced_lyrics);
        assert_eq!(deserialized.synced_lyrics_only, settings.synced_lyrics_only);
        assert_eq!(
            deserialized.companion_lyrics_formats,
            settings.companion_lyrics_formats
        );
        assert_eq!(deserialized.enhanced_lrc, settings.enhanced_lrc);

        // Cover art
        assert_eq!(deserialized.save_cover, settings.save_cover);
        assert_eq!(deserialized.cover_format, settings.cover_format);
        assert_eq!(deserialized.cover_size, settings.cover_size);

        // Animated artwork
        assert_eq!(
            deserialized.animated_artwork_enabled,
            settings.animated_artwork_enabled
        );
        assert_eq!(
            deserialized.hide_animated_artwork,
            settings.hide_animated_artwork
        );
        assert_eq!(deserialized.musickit_team_id, settings.musickit_team_id);
        assert_eq!(deserialized.musickit_key_id, settings.musickit_key_id);

        // Templates
        assert_eq!(
            deserialized.album_folder_template,
            settings.album_folder_template
        );
        assert_eq!(
            deserialized.compilation_folder_template,
            settings.compilation_folder_template
        );
        assert_eq!(
            deserialized.playlist_file_template,
            settings.playlist_file_template
        );

        // Advanced
        assert_eq!(deserialized.download_mode, settings.download_mode);
        assert_eq!(deserialized.remux_mode, settings.remux_mode);
        assert_eq!(deserialized.use_wrapper, settings.use_wrapper);
        assert_eq!(
            deserialized.wrapper_account_url,
            settings.wrapper_account_url
        );
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
        assert!(deserialized.truncate.is_none());
        assert!(deserialized.theme_override.is_none());
    }

    /// Verifies that `AppSettings` with all optional fields set to
    /// `Some(...)` values survives a serde roundtrip, ensuring custom
    /// tool paths and overrides are correctly persisted.
    #[test]
    fn app_settings_serde_handles_optional_fields_as_some() {
        let settings = AppSettings {
            cookies_path: Some("/path/to/cookies.txt".to_string()),
            ffmpeg_path: Some("/usr/local/bin/ffmpeg".to_string()),
            mp4decrypt_path: Some("/usr/local/bin/mp4decrypt".to_string()),
            mp4box_path: Some("/usr/local/bin/mp4box".to_string()),
            nm3u8dlre_path: Some("/usr/local/bin/N_m3u8DL-RE".to_string()),
            truncate: Some(200),
            theme_override: Some("dark".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.cookies_path,
            Some("/path/to/cookies.txt".to_string())
        );
        assert_eq!(
            deserialized.ffmpeg_path,
            Some("/usr/local/bin/ffmpeg".to_string())
        );
        assert_eq!(
            deserialized.mp4decrypt_path,
            Some("/usr/local/bin/mp4decrypt".to_string())
        );
        assert_eq!(
            deserialized.mp4box_path,
            Some("/usr/local/bin/mp4box".to_string())
        );
        assert_eq!(
            deserialized.nm3u8dlre_path,
            Some("/usr/local/bin/N_m3u8DL-RE".to_string())
        );
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

    // ----------------------------------------------------------
    // companion_lyrics_formats -- backward compatibility
    // ----------------------------------------------------------

    /// Verifies that companion_lyrics_formats defaults to `[Srt]`.
    #[test]
    fn default_companion_lyrics_formats_contains_srt() {
        let settings = AppSettings::default();
        assert_eq!(settings.companion_lyrics_formats.len(), 1);
        assert_eq!(settings.companion_lyrics_formats[0], LyricsFormat::Srt);
    }

    /// Verifies that JSON without companion_lyrics_formats deserializes
    /// successfully with the field defaulting to an empty vec. This
    /// ensures backward compatibility with existing settings.json files
    /// that predate the companion lyrics feature.
    #[test]
    fn companion_lyrics_formats_missing_from_json_defaults_to_empty() {
        let json = r#"{"synced_lyrics_format": "lrc"}"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert!(settings.companion_lyrics_formats.is_empty());
    }

    // ----------------------------------------------------------
    // enhanced_lrc -- Enhanced LRC word-by-word sync
    // ----------------------------------------------------------

    /// Verifies that enhanced_lrc defaults to true (enabled by default
    /// for new installations).
    #[test]
    fn default_enhanced_lrc_is_true() {
        let settings = AppSettings::default();
        assert!(settings.enhanced_lrc);
    }

    /// Verifies that JSON without the enhanced_lrc field deserializes
    /// with enhanced_lrc defaulting to true. This ensures users upgrading
    /// from v0.3.x automatically get Enhanced LRC enabled.
    #[test]
    fn enhanced_lrc_missing_from_json_defaults_to_true() {
        let json = r#"{"synced_lyrics_format": "lrc"}"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        assert!(settings.enhanced_lrc);
    }

    /// Verifies that enhanced_lrc can be explicitly set to false and
    /// round-trips correctly through serde.
    #[test]
    fn enhanced_lrc_false_roundtrip() {
        let settings = AppSettings {
            enhanced_lrc: false,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.enhanced_lrc);
    }

    /// Verifies that the default synced_lyrics_format is Ttml (for
    /// Enhanced LRC conversion with word-level timing preservation).
    #[test]
    fn default_synced_lyrics_format_is_ttml() {
        let settings = AppSettings::default();
        assert_eq!(settings.synced_lyrics_format, LyricsFormat::Ttml);
    }

    /// Verifies that companion_lyrics_formats round-trips through serde
    /// correctly when populated with multiple formats.
    #[test]
    fn companion_lyrics_formats_serde_roundtrip() {
        let settings = AppSettings {
            companion_lyrics_formats: vec![LyricsFormat::Srt, LyricsFormat::Ttml],
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.companion_lyrics_formats.len(), 2);
        assert_eq!(deserialized.companion_lyrics_formats[0], LyricsFormat::Srt);
        assert_eq!(deserialized.companion_lyrics_formats[1], LyricsFormat::Ttml);
    }
}
