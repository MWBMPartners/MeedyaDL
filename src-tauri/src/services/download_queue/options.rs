// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Option merging (per-download overrides + global settings), template padding, and codec-suffix planning.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;


// ============================================================
// Helper: merge per-download overrides with global settings
// ============================================================

/// Merges per-download option overrides with the global app settings
/// to produce the final set of GAMDL CLI options.
///
/// The merge follows a two-layer priority system:
/// 1. **Global settings** (from `AppSettings`) form the base layer
/// 2. **Per-download overrides** (from the frontend) override specific fields
///
/// This allows users to set global defaults (e.g., always use ALAC) while
/// still customizing individual downloads (e.g., this one in AAC-HE).
///
/// The resulting `GamdlOptions` struct is what actually gets passed to
/// `gamdl_service::build_gamdl_command_public()` to construct the CLI command.
#[allow(clippy::field_reassign_with_default)]
/// Inject zero-padding into bare `{track}` / `{disc}` placeholders (#587).
///
/// Takes a user-provided filename template and rewrites any bare
/// `{track}` / `{disc}` token to `{track:{width}d}` / `{disc:{width}d}`
/// using the caller's preferred padding widths. Tokens that already
/// carry an explicit format spec (`{track:02d}`, `{disc:02d}`,
/// `{track:!s}`, etc.) are left untouched — the user's explicit
/// template always wins. Case-sensitive: `{Track}` is not recognised.
///
/// `track_width` / `disc_width` of 0 means "no padding" (emit bare
/// `{track}` / `{disc}`), so the Python-style format spec becomes the
/// unpadded placeholder rather than `{track:0d}` which would produce
/// garbage output.
///
/// Extracted as a pure function so it can be exercised by unit tests
/// without a full settings/queue setup.
#[must_use]
pub(crate) fn apply_padding_to_template(template: &str, track_width: usize, disc_width: usize) -> String {
    // Regex-free implementation: walk the string, find literal `{track}`
    // and `{disc}` substrings (no format spec after the name), replace
    // in-place. Robust against tokens that appear multiple times in one
    // template (e.g. `{artist} - {track} of {track_total}` — only
    // `{track}` is substituted; `{track_total}` stays untouched because
    // we match the exact bare form).
    let track_replacement = if track_width == 0 {
        "{track}".to_string()
    } else {
        format!("{{track:0{track_width}d}}")
    };
    let disc_replacement = if disc_width == 0 {
        "{disc}".to_string()
    } else {
        format!("{{disc:0{disc_width}d}}")
    };
    // Only substitute when the bare form is what's in the template.
    // Ordering: track first, since `{track}` is lexically a superset of
    // nothing that would interfere with `{disc}` processing.
    let after_track = template.replace("{track}", &track_replacement);
    after_track.replace("{disc}", &disc_replacement)
}

// Large builder-style function: assigns ~50 `GamdlOptions` fields in
// layered order (global settings → computed per-download adjustments →
// per-download overrides). Rewriting as a single struct literal per
// clippy's suggestion would produce a 400-line initialiser and bury
// the layered-assignment logic that makes the merge order readable.
#[allow(clippy::field_reassign_with_default)]
pub(crate) fn merge_options(overrides: Option<&GamdlOptions>, settings: &AppSettings) -> GamdlOptions {
    let mut options = GamdlOptions::default();

    // === Layer 1: Apply global settings as the base ===
    // These come from the user's saved settings (settings.json).
    options.song_codec = Some(settings.default_song_codec.clone());
    options.music_video_resolution = Some(settings.default_video_resolution.clone());
    options.music_video_codec_priority = Some(settings.default_video_codec_priority.clone());
    options.music_video_remux_format = Some(settings.default_video_remux_format.clone());
    options.synced_lyrics_format = Some(settings.synced_lyrics_format.clone());
    options.no_synced_lyrics = Some(settings.no_synced_lyrics);
    options.synced_lyrics_only = Some(settings.synced_lyrics_only);
    options.save_cover = Some(settings.save_cover);
    options.cover_format = Some(settings.cover_format.clone());
    options.cover_size = Some(settings.cover_size);
    options.overwrite = Some(settings.overwrite);
    options.language = Some(settings.language.clone());
    // Every template field is passed through
    // `config_service::resolve_meedyadl_template_vars` BEFORE assignment
    // so MeedyaDL-introduced placeholders (currently `{platform}`, #829)
    // are resolved to concrete values. Without this, GAMDL's Python
    // `str.format(**metadata)` raises `KeyError: 'platform'` and every
    // download fails — same path as the INI-write equivalent in
    // `config_service::ini_template_section`. CLI-arg path matters
    // because options can also be passed via `--album-folder-template`
    // etc. when overriding INI defaults.
    options.album_folder_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.album_folder_template,
    ));
    options.compilation_folder_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.compilation_folder_template,
    ));
    options.no_album_folder_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.no_album_folder_template,
    ));
    // `playlist_folder_template` is a GAMDL v3.0+ CLI flag (#618). We can
    // safely set the field on `options` unconditionally — the CLI-emission
    // path in `GamdlOptions::to_cli_args` gates the actual `--playlist-
    // folder-template` arg behind `GamdlFeature::PlaylistFolderTemplate`,
    // so v2.9.x still gets a crash-free invocation. Setting the field on
    // every version keeps `GamdlOptions` the canonical debug dump.
    options.playlist_folder_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.playlist_folder_template,
    ));
    // Apply user-configurable zero-padding (#587). Padding widths are
    // derived from the user's settings; `resolve_width(None)` passes
    // `None` because `track_total` / `disc_total` aren't known at merge
    // time — they come from the Apple Music API prefetch later in the
    // pipeline. `Auto` mode falls back to pre-#587 defaults in that
    // case; fixed widths take effect immediately. See
    // `apply_padding_to_template` for the substitution rules.
    //
    // {platform} resolution runs FIRST, then padding rewrites
    // `{track}` / `{disc}`. Order matters only insofar as one helper
    // produces literal characters the other might touch — neither
    // currently does (padding only matches `{track}` / `{disc}`;
    // platform substitution only matches `{platform}`), so the order
    // is purely cosmetic, but we keep `{platform}` first so the
    // post-resolution debug dump shows the user-visible service name.
    options.single_disc_file_template = Some(apply_padding_to_template(
        &super::config_service::resolve_meedyadl_template_vars(
            &settings.single_disc_file_template,
        ),
        settings.track_number_padding.resolve_width(None),
        settings.disc_number_padding.resolve_width(None),
    ));
    options.multi_disc_file_template = Some(apply_padding_to_template(
        &super::config_service::resolve_meedyadl_template_vars(
            &settings.multi_disc_file_template,
        ),
        settings.track_number_padding.resolve_width(None),
        settings.disc_number_padding.resolve_width(None),
    ));
    options.no_album_file_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.no_album_file_template,
    ));
    options.playlist_file_template = Some(super::config_service::resolve_meedyadl_template_vars(
        &settings.playlist_file_template,
    ));
    options.use_wrapper = Some(settings.use_wrapper);
    options.wrapper_account_url = Some(settings.wrapper_account_url.clone());
    // `wrapper_url` (#853) is the wrapper-v2 HTTP base URL, accepted by
    // GAMDL >= 3.6 via `--wrapper-url`. Harmless to set unconditionally
    // on the options struct — `to_cli_args()` only emits it when
    // `GamdlFeature::WrapperUrl` is supported by the detected release,
    // so wrapper-v1 downloads never see the flag.
    if settings.use_wrapper {
        options.wrapper_url = Some(settings.wrapper_url.clone());
    }
    // `wrapper_m3u8_ip` is a GAMDL v3.1+ CLI flag; only emit it when the
    // detected GAMDL release supports it and the user has wrapper mode on.
    // Older releases don't know the flag (would parse-error), and cookie-
    // mode downloads never consult the wrapper m3u8 socket.
    if settings.use_wrapper
        && super::gamdl_capabilities::supports(
            super::gamdl_capabilities::GamdlFeature::WrapperM3u8Ip,
        )
    {
        options.wrapper_m3u8_ip = Some(settings.wrapper_m3u8_ip.clone());
    }
    // `wrapper_decrypt_ip` (#743) is the third leg of the wrapper triangle —
    // GAMDL opens an outbound TCP connection to this address to send
    // encrypted samples for FairPlay decryption. Unlike `wrapper_m3u8_ip`,
    // this flag exists in every GAMDL release we support, so no
    // version-capability gate is needed. Cookie-mode downloads never hit
    // the decrypt socket, so we only emit when wrapper auth is on.
    if settings.use_wrapper {
        options.wrapper_decrypt_ip = Some(settings.wrapper_decrypt_ip.clone());
    }
    options.truncate = settings.truncate;

    if !settings.output_path.is_empty() {
        // Validate output path for traversal sequences (#459)
        match super::config_service::validate_path_safe(&settings.output_path) {
            Ok(_) => {
                options.output_path = Some(settings.output_path.clone());
            }
            Err(e) => {
                log::warn!("Output path rejected: {e}");
            }
        }
    }

    // Resolve temp_path: use the user's custom path if set, otherwise fall
    // back to a "MeedyaDL" subdirectory within the OS temp directory (e.g.
    // /var/folders/.../MeedyaDL on macOS, %TEMP%\MeedyaDL on Windows,
    // /tmp/MeedyaDL on Linux). Using a dedicated subdirectory keeps
    // intermediate files isolated and easy to clean up. This avoids GAMDL's
    // default of "." which is unwritable from /Applications on macOS.
    if settings.temp_path.is_empty() {
        options.temp_path = Some(
            std::env::temp_dir()
                .join("MeedyaDL")
                .to_string_lossy()
                .to_string(),
        );
    } else {
        options.temp_path = Some(settings.temp_path.clone());
    }

    // Apply tool paths from settings
    options.cookies_path.clone_from(&settings.cookies_path);
    options.ffmpeg_path.clone_from(&settings.ffmpeg_path);
    options
        .mp4decrypt_path
        .clone_from(&settings.mp4decrypt_path);
    options.mp4box_path.clone_from(&settings.mp4box_path);
    options.nm3u8dlre_path.clone_from(&settings.nm3u8dlre_path);

    // Set download and remux modes
    options.download_mode = Some(settings.download_mode.clone());
    options.remux_mode = Some(settings.remux_mode.clone());

    // Default to `--no-exceptions` so GAMDL prints a single user-facing
    // line per failure instead of a full Python traceback. Three-era
    // version matrix (see `GamdlFeature::NoExceptionsFlag` doc for the
    // full history):
    //
    //   * `< 3.1` — flag suppresses `traceback.print_exc()`; fully
    //     effective. Activity log stays clean.
    //   * `3.1..3.7.4` — flag is a no-op. Upstream commit `dc6f2e8`
    //     ("Use ExceptionPrettyPrinter and .exception logging")
    //     removed every consumer; `structlog`'s
    //     `ExceptionPrettyPrinter` is added to the processor list
    //     unconditionally so tracebacks surface regardless. MeedyaDL's
    //     activity log therefore renders pretty-printed exception
    //     blocks on this range even with `verbose_gamdl_exceptions=false`.
    //   * `>= 3.8` — flag effective again. Upstream commit `58f4548`
    //     ("Respect no exceptions option") gates the
    //     `ExceptionPrettyPrinter` on `not config.no_exceptions`,
    //     restoring the suppression behaviour.
    //
    // Users debugging upstream issues can flip
    // `verbose_gamdl_exceptions` on to get the full stack trace back;
    // in that case we leave `no_exceptions` as `None` so
    // `to_cli_args()` never emits the flag.
    //
    // We route the version gate through `GamdlFeature::NoExceptionsFlag`
    // — the capability enum is the single source of truth. Emitting a
    // flag the subprocess will silently ignore is misleading to
    // anyone reading the spawned command line; the pre-3.1 and >= 3.8
    // eras both flip the gate to `true`, the 3.1..3.7.4 window flips
    // it to `false`. Unknown version (capability cache not yet
    // populated on the first download of the session) also flips to
    // `false` — the flag is safe to omit on every release since 2.x
    // (upstream never rejects it), so omitting when uncertain is a
    // free defensive default.
    if !settings.verbose_gamdl_exceptions {
        options.no_exceptions = Some(true);
    }
    if !super::gamdl_capabilities::supports(
        super::gamdl_capabilities::GamdlFeature::NoExceptionsFlag,
    ) {
        options.no_exceptions = None;
    }

    // Apply exclude tags
    if !settings.exclude_tags.is_empty() {
        options.exclude_tags = Some(settings.exclude_tags.join(","));
    }

    // Apply artist auto-selection mode (GAMDL >= 2.9.1)
    options
        .artist_auto_select
        .clone_from(&settings.artist_auto_select);

    // GAMDL log level (#768) — `--log-level <LEVEL>` exists on every
    // release in our support window, so there's no GamdlFeature gate
    // needed here. The default (`Info`) matches GAMDL's compiled-in
    // default; flipping to `Debug` in Developer Tools is what surfaces
    // the v3.5.2+ structlog `m3u8_master_url=…` diagnostics that
    // motivated this wiring. Cloned because LogLevel doesn't impl Copy.
    options.log_level = Some(settings.gamdl_log_level.clone());

    // === Layer 2: Apply per-download overrides (highest priority) ===
    // Only non-None fields from the override replace the global values.
    // This selective merge allows partial overrides (e.g., only change codec).
    if let Some(overrides) = overrides {
        if overrides.song_codec.is_some() {
            options.song_codec.clone_from(&overrides.song_codec);
        }
        if overrides.music_video_resolution.is_some() {
            options
                .music_video_resolution
                .clone_from(&overrides.music_video_resolution);
        }
        if overrides.music_video_codec_priority.is_some() {
            options
                .music_video_codec_priority
                .clone_from(&overrides.music_video_codec_priority);
        }
        if overrides.music_video_remux_format.is_some() {
            options
                .music_video_remux_format
                .clone_from(&overrides.music_video_remux_format);
        }
        if overrides.output_path.is_some() {
            options.output_path.clone_from(&overrides.output_path);
        }
        if overrides.overwrite.is_some() {
            options.overwrite = overrides.overwrite;
        }
        if overrides.artist_auto_select.is_some() {
            options
                .artist_auto_select
                .clone_from(&overrides.artist_auto_select);
        }
    }

    // GAMDL 3.5 can fail music-video artist selections before the media
    // download starts when `--save-cover` is enabled: some Apple video
    // artwork URLs arrive as `{w}x{h}` templates and GAMDL fetches them
    // literally. Static cover sidecars are non-essential for MV-only runs,
    // so keep the user's cover settings for audio and suppress them here.
    if matches!(
        options.artist_auto_select,
        Some(ArtistAutoSelect::MusicVideos)
    ) {
        options.save_cover = None;
        options.cover_format = None;
        options.cover_size = None;
        options.no_config_file = Some(true);
    }

    // === Layer 3: Lyrics embed + sidecar enforcement ===
    // When the user has enabled "Embed Lyrics and Keep Sidecar", ensure that:
    // 1. Lyrics are NOT excluded from metadata embedding (remove "lyrics" from
    //    exclude_tags if present, so GAMDL embeds them in the audio file's tags).
    // 2. Synced lyrics sidecar files are NOT disabled (force no_synced_lyrics
    //    to false, so GAMDL creates the .lrc/.srt/.ttml sidecar alongside).
    // This provides maximum compatibility: embedded for players that read tags,
    // sidecar for those that look for external lyrics files.
    if settings.embed_lyrics_and_sidecar {
        // Remove "lyrics" from the exclude_tags comma-separated string so
        // GAMDL embeds lyrics in the audio file's metadata atoms.
        if let Some(ref tags) = options.exclude_tags {
            let filtered: Vec<&str> = tags
                .split(',')
                .map(str::trim)
                .filter(|t| !t.eq_ignore_ascii_case("lyrics"))
                .collect();
            if filtered.is_empty() {
                options.exclude_tags = None;
            } else {
                options.exclude_tags = Some(filtered.join(","));
            }
        }
        // Force sidecar lyrics creation regardless of the no_synced_lyrics setting.
        options.no_synced_lyrics = Some(false);
    }

    // === Layer 4: Enhanced LRC enforcement ===
    // When Enhanced LRC is enabled, force TTML as the primary lyrics format
    // so the raw word-level timing data is preserved in the sidecar file.
    // GAMDL's TTML output retains the full XML including <span> word timestamps,
    // which are then converted to Enhanced LRC in the enrichment pipeline.
    // This runs after Layer 3 so it overrides the lyrics format regardless
    // of what the user selected, ensuring TTML is always available for conversion.
    if settings.enhanced_lrc {
        options.synced_lyrics_format = Some(LyricsFormat::Ttml);
        // Ensure sidecar creation (needed for TTML → Enhanced LRC conversion)
        options.no_synced_lyrics = Some(false);
    }

    options
}

// ============================================================
// Helper: codec-based filename suffix system
// ============================================================

/// Returns the filename suffix for a given audio codec, or `None` if the
/// codec should use a clean (unsuffixed) filename.
///
/// Suffix rules:
/// - **Lossy codecs** (AAC, AAC-Legacy, AAC-Binaural, AC3, etc.) get no
///   suffix, as they represent the "standard" download a user would expect.
/// - **Lossless** (ALAC) gets `[Lossless]` to distinguish from lossy versions.
/// - **Spatial audio** (Dolby Atmos) gets `[Dolby Atmos]` to clearly identify
///   the immersive mix.
///
/// When companion downloads are enabled, multiple codec versions of the same
/// track can coexist in the same album folder. The suffix prevents filename
/// collisions and makes the format instantly visible in file browsers.
///
/// Suffixes are defined in `codecs.toml` under the `suffix` field of each
/// audio codec entry. This function delegates to `codec_suffix_from_registry()`
/// which looks up the suffix from the compiled-in registry data.
pub(crate) fn codec_suffix(codec: &SongCodec) -> Option<&'static str> {
    codec_suffix_from_registry(codec)
}

/// Determines whether the primary download's file templates should have a
/// codec suffix applied, based on the companion mode and the download's codec.
///
/// A suffix is needed when the companion mode will produce at least one
/// companion with a clean filename alongside the primary download. This
/// prevents filename collisions in the same album directory.
///
/// # Rules per mode
///
/// | Mode                       | Atmos gets suffix? | ALAC gets suffix? | Others? |
/// |----------------------------|--------------------|-------------------|---------|
/// | `Disabled`                 | No                 | No                | No      |
/// | `AtmosToLossless`          | Yes                | No                | No      |
/// | `AtmosToLosslessAndLossy`  | Yes                | Yes               | No      |
/// | `SpecialistToLossy`        | Yes                | Yes               | No      |
/// | `AtmosToAllFormats`        | Yes                | No                | No      |
pub(crate) fn needs_primary_suffix(
    codec: &SongCodec,
    mode: &CompanionMode,
    custom_codecs: &[SongCodec],
) -> bool {
    match mode {
        // No companions → no suffix needed (only one version exists)
        CompanionMode::Disabled => false,
        // Only Atmos gets a companion (ALAC or all formats), so only Atmos
        // needs a suffix. ALAC downloads in these modes have no companion → no suffix.
        CompanionMode::AtmosToLossless | CompanionMode::AtmosToAllFormats => {
            matches!(codec, SongCodec::Atmos)
        }
        // Both Atmos and ALAC get companions (at least a lossy one),
        // so both need suffixes to coexist with the clean-filename companion.
        // Same for SpecialistToLossy: any specialist codec gets a lossy companion.
        CompanionMode::AtmosToLosslessAndLossy | CompanionMode::SpecialistToLossy => {
            matches!(codec, SongCodec::Atmos | SongCodec::Alac)
        }
        // Custom mode: primary needs suffix if at least one companion will
        // have a clean filename (to prevent collisions). This is the case
        // when the custom codec list is non-empty and the primary codec is
        // not already in the custom list (the primary always gets a suffix
        // when custom companions exist).
        CompanionMode::Custom => {
            if custom_codecs.is_empty() {
                return false;
            }
            // Primary always gets a suffix when custom companions are active,
            // because at least one companion will use a clean filename.
            true
        }
    }
}

