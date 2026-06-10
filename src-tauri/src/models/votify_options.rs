// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Votify CLI option models.
// =========================
//
// Typed Rust representations of the votify command-line flags MeedyaDL
// emits for Spotify downloads. Mirrors the shape of `GamdlOptions`:
// every field is `Option<T>` so per-download overrides can be layered
// on top of the global defaults sourced from `AppSettings::services::spotify`.
//
// Flag surface tracks the votify >= 1.9 CLI documented at
// <https://github.com/glomatico/votify#configuration>. The subset
// represented here is the user-facing one — engine-internal knobs
// (`--config-path`, `--log-level`, `--log-file`) are managed by
// `spotify_service.rs` rather than exposed as overrides.
//
// ## Anti-ban posture (M9-2 vs M9-4)
//
// `wait_interval` is wired here so M9-4's anti-ban stack can flow
// the user-configured throttle through `VotifyOptions::to_cli_args`
// without another model change. Default is left as `None` — the
// actual default value lives in the Settings UI surface (M9-4),
// not in the option model.

use serde::{Deserialize, Serialize};

/// Votify CLI options for Spotify downloads.
///
/// Maps to votify command-line flags. Every field is `Option<T>` so
/// the merge pattern used by the queue layer can apply per-download
/// overrides without clobbering global defaults.
///
/// Fields are grouped to match the sections in votify's own configuration
/// table (General / Spotify / Output / Template / Song-Podcast / Video).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VotifyOptions {
    // ============================================================
    // General
    // ============================================================
    /// Seconds to wait between successive downloads.
    ///
    /// Maps to votify's `--wait-interval`. The flagship anti-ban knob
    /// per the M9 EPIC — M9-4's "real-time playback-speed throttle"
    /// builds on top of this by tying the interval to track duration.
    pub wait_interval: Option<u32>,

    /// Suppress the Python traceback on exceptions.
    ///
    /// Maps to `--no-exceptions`. Mirrors the equivalent GAMDL flag —
    /// useful when piping CLI output into our activity log so a single
    /// failure doesn't pollute the panel with stack frames.
    pub no_exceptions: Option<bool>,

    /// Path to a SQLite database registering downloaded media.
    ///
    /// Maps to `--database-path`. Off by default; MeedyaDL has its
    /// own manifest system, so this is exposed for power users who
    /// want votify's own dedup layer.
    pub database_path: Option<String>,

    /// Auto-select a media subset when downloading an artist or
    /// liked-tracks shortcut.
    ///
    /// Maps to `--auto-media-option`. votify accepts values like
    /// `albums`, `singles`, `liked-tracks`. Mirrors the role of
    /// `GamdlOptions::artist_auto_select`.
    pub auto_media_option: Option<String>,

    // ============================================================
    // Spotify (authentication / session)
    // ============================================================
    /// Spotify session type to use for the download.
    ///
    /// Maps to `--session-type`. Accepted values:
    /// * `librespot` — open-source client, free-tier compatible
    ///   (Ogg Vorbis only — used as the M9-2 default)
    /// * `desktop`   — premium account + Spotify DLL 1.2.88.483
    ///   for FLAC (M9-5 gate)
    /// * `web`       — premium account + Widevine `.wvd`
    ///   for FLAC (M9-6 gate)
    pub session_type: Option<String>,

    /// Path to the Netscape-format cookies file for Spotify auth.
    ///
    /// Maps to `--cookies-path`. Resolved to MeedyaDL's managed
    /// cookies location when omitted (mirrors the gamdl flow).
    pub cookies_path: Option<String>,

    /// Path to a Widevine `.wvd` file (for `session_type = "web"`
    /// FLAC support).
    ///
    /// Maps to `--wvd-path`. The Widevine integration itself lands in M9-6.
    pub wvd_path: Option<String>,

    /// Path to the Spotify desktop DLL (for `session_type = "desktop"`
    /// FLAC support).
    ///
    /// Maps to `--spotify-dll-path`. The desktop integration lands in M9-5.
    pub spotify_dll_path: Option<String>,

    /// Prefer video streams when the requested resource has both
    /// audio and video variants.
    ///
    /// Maps to `--prefer-video`. Useful for episodes / canvases.
    pub prefer_video: Option<bool>,

    // ============================================================
    // Output
    // ============================================================
    /// Output directory.
    ///
    /// Maps to `--output`. Defaulted by the queue layer when absent.
    pub output_path: Option<String>,

    /// Temporary directory for in-progress files.
    ///
    /// Maps to `--temp`. MeedyaDL routes this to its managed temp
    /// directory unless the user overrides.
    pub temp_path: Option<String>,

    /// Save the cover as a separate file alongside the audio.
    ///
    /// Maps to `--save-cover-file`. Independent of cover embedding,
    /// which votify performs unconditionally on supported codecs.
    pub save_cover_file: Option<bool>,

    /// Save an M3U8 playlist file when downloading a playlist URL.
    ///
    /// Maps to `--save-playlist-file`.
    pub save_playlist_file: Option<bool>,

    /// Overwrite existing files instead of skipping them.
    ///
    /// Maps to `--overwrite`. Defaults to `false` (skip) so smart
    /// re-download / gap-fill flows can rely on existing files being
    /// preserved.
    pub overwrite: Option<bool>,

    /// Requested cover-art resolution.
    ///
    /// Maps to `--cover-size`. votify accepts `small`, `medium`,
    /// `large`, `extra-large`. M9-3's best-cover-art service may
    /// override this on a per-track basis when the Apple Music image
    /// wins the resolution race.
    pub cover_size: Option<String>,

    /// Comma-separated list of metadata tags to exclude from the
    /// written file.
    ///
    /// Maps to `--exclude-tags`. The same shape GAMDL uses.
    pub exclude_tags: Option<String>,

    /// Maximum length of file and folder names.
    ///
    /// Maps to `--truncate`. Useful for FAT32 / SMB shares.
    pub truncate: Option<u32>,

    // ============================================================
    // Template (filename / folder layout)
    // ============================================================
    /// Folder template for album tracks.
    ///
    /// Maps to `--album-folder-template`. Default upstream is
    /// `{album_artist}/{album}` — left `None` here so the user's
    /// global preference wins.
    pub album_folder_template: Option<String>,

    /// Folder template for compilation tracks.
    ///
    /// Maps to `--compilation-folder-template`.
    pub compilation_folder_template: Option<String>,

    /// Folder template for podcast episodes.
    ///
    /// Maps to `--podcast-folder-template`.
    pub podcast_folder_template: Option<String>,

    /// Folder template for tracks not in an album.
    ///
    /// Maps to `--no-album-folder-template`.
    pub no_album_folder_template: Option<String>,

    /// File template for single-disc album tracks.
    ///
    /// Maps to `--single-disc-file-template`.
    pub single_disc_file_template: Option<String>,

    /// File template for multi-disc album tracks.
    ///
    /// Maps to `--multi-disc-file-template`.
    pub multi_disc_file_template: Option<String>,

    /// File template for podcast episodes.
    ///
    /// Maps to `--podcast-file-template`.
    pub podcast_file_template: Option<String>,

    /// File template for tracks not in an album.
    ///
    /// Maps to `--no-album-file-template`.
    pub no_album_file_template: Option<String>,

    /// File template for M3U8 playlists.
    ///
    /// Maps to `--playlist-file-template`.
    pub playlist_file_template: Option<String>,

    /// `strftime` template for date tags.
    ///
    /// Maps to `--date-tag-template`.
    pub date_tag_template: Option<String>,

    // ============================================================
    // Song / Podcast (audio)
    // ============================================================
    /// Comma-separated audio quality priority.
    ///
    /// Maps to `--audio-quality`. votify accepts the priority list
    /// in the same CSV shape GAMDL's `--song-codec-priority` uses,
    /// so the M9-2 fallback chain plumbing slots in cleanly when
    /// it lands.
    ///
    /// Values include `vorbis-high`, `vorbis-medium`, `vorbis-low`,
    /// plus `aac-high` / `aac-medium` / `aac-low` and `flac`/`mp3`
    /// when the session type allows.
    pub audio_quality: Option<String>,

    /// Backend used to fetch the encrypted audio stream.
    ///
    /// Maps to `--audio-download-mode`. votify supports `ytdlp`,
    /// `aria2c`, and `curl`. Default upstream is `ytdlp`.
    pub audio_download_mode: Option<String>,

    /// Tool used to remux the downloaded audio into its final
    /// container.
    ///
    /// Maps to `--audio-remux-mode`. Default upstream is `ffmpeg`.
    pub audio_remux_mode: Option<String>,

    /// Skip the audio file and only fetch the synced lyrics
    /// sidecar.
    ///
    /// Maps to `--synced-lyrics-only`.
    pub synced_lyrics_only: Option<bool>,

    /// Suppress synced lyrics sidecar creation.
    ///
    /// Maps to `--no-synced-lyrics-file`. Mirrors the GAMDL
    /// `--no-synced-lyrics` semantic — opt-out, not opt-in.
    pub no_synced_lyrics_file: Option<bool>,

    // ============================================================
    // Video
    // ============================================================
    /// Output container for downloaded videos.
    ///
    /// Maps to `--video-format`. Default upstream is `mp4`.
    pub video_format: Option<String>,

    /// Video resolution to request.
    ///
    /// Maps to `--video-resolution`. Default upstream is `1080p`.
    pub video_resolution: Option<String>,

    /// Tool used to remux the downloaded video stream.
    ///
    /// Maps to `--video-remux-mode`. Default upstream is `ffmpeg`.
    pub video_remux_mode: Option<String>,

    // ============================================================
    // Executable paths (auto-injected from MeedyaDL's managed tools)
    // ============================================================
    /// Path to the FFmpeg binary.
    ///
    /// Maps to `--ffmpeg-path`. Auto-injected to MeedyaDL's
    /// managed FFmpeg by `spotify_service::inject_tool_paths`.
    pub ffmpeg_path: Option<String>,

    /// Path to the MP4Box binary.
    ///
    /// Maps to `--mp4box-path`. Auto-injected when unset.
    pub mp4box_path: Option<String>,

    /// Path to the mp4decrypt binary.
    ///
    /// Maps to `--mp4decrypt-path`. Auto-injected when unset.
    pub mp4decrypt_path: Option<String>,

    /// Path to the aria2c binary.
    ///
    /// Maps to `--aria2c-path`. Optional — used only when
    /// `audio_download_mode = "aria2c"`.
    pub aria2c_path: Option<String>,

    /// Path to the curl binary.
    ///
    /// Maps to `--curl-path`. Optional — used only when
    /// `audio_download_mode = "curl"`.
    pub curl_path: Option<String>,

    /// Path to the Shaka Packager binary.
    ///
    /// Maps to `--shaka-packager-path`. Optional video pipeline tool.
    pub shaka_packager_path: Option<String>,
}

impl VotifyOptions {
    /// Build a [`VotifyOptions`] from the global [`SpotifySettings`]
    /// block on [`crate::models::settings::AppSettings`].
    ///
    /// Per-download overrides aren't supported by the queue yet —
    /// every Spotify queue item dispatches with the global settings
    /// applied unmodified. The merge pattern that GAMDL uses
    /// (per-request `Option<T>` layers over global defaults) lands
    /// in a follow-up alongside the Settings UI; until then this
    /// constructor IS the configuration boundary.
    ///
    /// Anti-ban knobs (`wait_interval`, etc.) intentionally aren't
    /// forwarded here — `wait_interval` is wired in this struct as
    /// scaffolding for M9-8, but M9-7 doesn't yet pass any value
    /// through to votify because the daily-cap counter and
    /// dispatch-gate already provide a layered defence and adding a
    /// rough wait without per-track instrumentation would just slow
    /// downloads without actually shaping their traffic pattern.
    /// See `services::spotify_anti_ban` for the helpers M9-8 will
    /// wire in.
    #[must_use]
    pub fn from_settings(spotify: &crate::models::settings::SpotifySettings) -> Self {
        Self {
            session_type: spotify.session_type.clone(),
            cookies_path: spotify.cookies_path.clone(),
            wvd_path: spotify.wvd_path.clone(),
            spotify_dll_path: spotify.spotify_dll_path.clone(),
            // Anti-ban: M9-7 keeps wait_interval unset; M9-8 will
            // populate it from `anti_ban.inter_track_delay_seconds`
            // once per-track instrumentation lands.
            ..Self::default()
        }
    }

    /// Translate the typed option struct into a flat CLI argv that
    /// can be appended after `python -m votify <urls>`.
    ///
    /// Booleans emit their flag with no value when `true` and emit
    /// nothing when `false` (so the CLI default — usually `false`
    /// — is preserved). String / numeric options emit `--flag value`
    /// pairs.
    ///
    /// Flag ordering inside this function is grouped to mirror the
    /// struct layout above. Order on the CLI is irrelevant to votify
    /// — keeping them grouped makes the rendered command line easy
    /// to read in the activity log.
    #[allow(clippy::too_many_lines)] // mostly flat key→arg mapping
    pub fn to_cli_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // ---- General ----
        if let Some(v) = self.wait_interval {
            args.push("--wait-interval".to_string());
            args.push(v.to_string());
        }
        if self.no_exceptions == Some(true) {
            args.push("--no-exceptions".to_string());
        }
        if let Some(v) = &self.database_path {
            args.push("--database-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.auto_media_option {
            args.push("--auto-media-option".to_string());
            args.push(v.clone());
        }

        // ---- Spotify session ----
        if let Some(v) = &self.session_type {
            args.push("--session-type".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.cookies_path {
            args.push("--cookies-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.wvd_path {
            args.push("--wvd-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.spotify_dll_path {
            args.push("--spotify-dll-path".to_string());
            args.push(v.clone());
        }
        if self.prefer_video == Some(true) {
            args.push("--prefer-video".to_string());
        }

        // ---- Output ----
        if let Some(v) = &self.output_path {
            args.push("--output".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.temp_path {
            args.push("--temp".to_string());
            args.push(v.clone());
        }
        if self.save_cover_file == Some(true) {
            args.push("--save-cover-file".to_string());
        }
        if self.save_playlist_file == Some(true) {
            args.push("--save-playlist-file".to_string());
        }
        if self.overwrite == Some(true) {
            args.push("--overwrite".to_string());
        }
        if let Some(v) = &self.cover_size {
            args.push("--cover-size".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.exclude_tags {
            args.push("--exclude-tags".to_string());
            args.push(v.clone());
        }
        if let Some(v) = self.truncate {
            args.push("--truncate".to_string());
            args.push(v.to_string());
        }

        // ---- Templates ----
        push_template(&mut args, "--album-folder-template", &self.album_folder_template);
        push_template(
            &mut args,
            "--compilation-folder-template",
            &self.compilation_folder_template,
        );
        push_template(&mut args, "--podcast-folder-template", &self.podcast_folder_template);
        push_template(
            &mut args,
            "--no-album-folder-template",
            &self.no_album_folder_template,
        );
        push_template(
            &mut args,
            "--single-disc-file-template",
            &self.single_disc_file_template,
        );
        push_template(&mut args, "--multi-disc-file-template", &self.multi_disc_file_template);
        push_template(&mut args, "--podcast-file-template", &self.podcast_file_template);
        push_template(&mut args, "--no-album-file-template", &self.no_album_file_template);
        push_template(&mut args, "--playlist-file-template", &self.playlist_file_template);
        push_template(&mut args, "--date-tag-template", &self.date_tag_template);

        // ---- Audio ----
        if let Some(v) = &self.audio_quality {
            args.push("--audio-quality".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.audio_download_mode {
            args.push("--audio-download-mode".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.audio_remux_mode {
            args.push("--audio-remux-mode".to_string());
            args.push(v.clone());
        }
        if self.synced_lyrics_only == Some(true) {
            args.push("--synced-lyrics-only".to_string());
        }
        if self.no_synced_lyrics_file == Some(true) {
            args.push("--no-synced-lyrics-file".to_string());
        }

        // ---- Video ----
        if let Some(v) = &self.video_format {
            args.push("--video-format".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.video_resolution {
            args.push("--video-resolution".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.video_remux_mode {
            args.push("--video-remux-mode".to_string());
            args.push(v.clone());
        }

        // ---- Executable paths ----
        if let Some(v) = &self.ffmpeg_path {
            args.push("--ffmpeg-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.mp4box_path {
            args.push("--mp4box-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.mp4decrypt_path {
            args.push("--mp4decrypt-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.aria2c_path {
            args.push("--aria2c-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.curl_path {
            args.push("--curl-path".to_string());
            args.push(v.clone());
        }
        if let Some(v) = &self.shaka_packager_path {
            args.push("--shaka-packager-path".to_string());
            args.push(v.clone());
        }

        args
    }
}

/// Helper: push a `--flag value` pair when the option is `Some`.
///
/// Kept private so the to_cli_args body stays readable — the templates
/// section calls this ten times in a row and the mechanical repetition
/// would otherwise dominate the function.
fn push_template(args: &mut Vec<String>, flag: &str, value: &Option<String>) {
    if let Some(v) = value {
        args.push(flag.to_string());
        args.push(v.clone());
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_emit_no_args() {
        let opts = VotifyOptions::default();
        assert!(
            opts.to_cli_args().is_empty(),
            "Default VotifyOptions should produce no CLI args — every flag is None"
        );
    }

    #[test]
    fn wait_interval_emits_flag_and_value() {
        let opts = VotifyOptions {
            wait_interval: Some(30),
            ..Default::default()
        };
        let args = opts.to_cli_args();
        assert_eq!(args, vec!["--wait-interval".to_string(), "30".to_string()]);
    }

    #[test]
    fn boolean_flag_only_emits_when_true() {
        let on = VotifyOptions {
            overwrite: Some(true),
            ..Default::default()
        };
        let off = VotifyOptions {
            overwrite: Some(false),
            ..Default::default()
        };
        assert_eq!(on.to_cli_args(), vec!["--overwrite".to_string()]);
        assert!(off.to_cli_args().is_empty());
    }

    #[test]
    fn audio_quality_round_trips_csv() {
        let opts = VotifyOptions {
            audio_quality: Some("aac-high,vorbis-high".to_string()),
            ..Default::default()
        };
        let args = opts.to_cli_args();
        assert!(args.iter().any(|a| a == "--audio-quality"));
        assert!(args.iter().any(|a| a == "aac-high,vorbis-high"));
    }

    #[test]
    fn template_helpers_skip_none() {
        let opts = VotifyOptions {
            album_folder_template: Some("{album_artist}/{album}".to_string()),
            ..Default::default()
        };
        let args = opts.to_cli_args();
        let idx = args.iter().position(|a| a == "--album-folder-template").unwrap();
        assert_eq!(args[idx + 1], "{album_artist}/{album}");
        // compilation_folder_template is None — must not appear.
        assert!(!args.iter().any(|a| a == "--compilation-folder-template"));
    }

    #[test]
    fn exhaustive_value_options_emit_in_struct_order() {
        let opts = VotifyOptions {
            wait_interval: Some(15),
            session_type: Some("librespot".to_string()),
            output_path: Some("/tmp/spotify".to_string()),
            cover_size: Some("extra-large".to_string()),
            audio_quality: Some("vorbis-high".to_string()),
            video_resolution: Some("720p".to_string()),
            ..Default::default()
        };
        let args = opts.to_cli_args();
        // Order check: General → Spotify → Output → Audio → Video
        let positions: Vec<usize> = [
            "--wait-interval",
            "--session-type",
            "--output",
            "--cover-size",
            "--audio-quality",
            "--video-resolution",
        ]
        .iter()
        .map(|flag| args.iter().position(|a| a == *flag).expect("flag present"))
        .collect();
        for w in positions.windows(2) {
            assert!(
                w[0] < w[1],
                "flag ordering should mirror struct grouping; got positions {positions:?}"
            );
        }
    }

    #[test]
    fn from_settings_propagates_session_artefacts_but_not_anti_ban() {
        // M9-7 contract: session_type, cookies_path, wvd_path, and
        // spotify_dll_path flow from settings into options. Anti-ban
        // knobs (wait_interval, anti_ban.*) deliberately do NOT —
        // see the from_settings doc-comment for the M9-8 timing.
        use crate::models::settings::SpotifySettings;
        use crate::models::spotify_anti_ban::AntiBanSettings;
        let spotify = SpotifySettings {
            cookies_path: Some("/c.txt".to_string()),
            session_type: Some("web".to_string()),
            spotify_dll_path: Some("/spotify.dll".to_string()),
            wvd_path: Some("/keys.wvd".to_string()),
            anti_ban: AntiBanSettings {
                inter_track_delay_seconds: 30, // M9-7 ignores this
                ..Default::default()
            },
        };
        let opts = VotifyOptions::from_settings(&spotify);
        assert_eq!(opts.session_type.as_deref(), Some("web"));
        assert_eq!(opts.cookies_path.as_deref(), Some("/c.txt"));
        assert_eq!(opts.spotify_dll_path.as_deref(), Some("/spotify.dll"));
        assert_eq!(opts.wvd_path.as_deref(), Some("/keys.wvd"));
        assert_eq!(
            opts.wait_interval, None,
            "M9-7 must not propagate wait_interval — anti-ban knobs land in M9-8 with per-track instrumentation"
        );
    }

    #[test]
    fn serde_round_trip_preserves_all_set_fields() {
        let opts = VotifyOptions {
            wait_interval: Some(20),
            session_type: Some("desktop".to_string()),
            cookies_path: Some("/c.txt".to_string()),
            spotify_dll_path: Some("/spot.dll".to_string()),
            output_path: Some("/o".to_string()),
            overwrite: Some(true),
            cover_size: Some("large".to_string()),
            audio_quality: Some("flac".to_string()),
            video_resolution: Some("1080p".to_string()),
            ffmpeg_path: Some("/usr/bin/ffmpeg".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&opts).expect("serialize");
        let back: VotifyOptions = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.wait_interval, opts.wait_interval);
        assert_eq!(back.session_type, opts.session_type);
        assert_eq!(back.audio_quality, opts.audio_quality);
        assert_eq!(back.ffmpeg_path, opts.ffmpeg_path);
        assert_eq!(back.overwrite, opts.overwrite);
    }
}
