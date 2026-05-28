// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// replaygain_service.rs -- ReplayGain loudness analysis service
// =============================================================
//
// Analyses audio loudness using FFmpeg's EBU R128 loudness meter and writes
// non-destructive ReplayGain metadata tags. This enables volume normalisation
// in media players that support ReplayGain (foobar2000, Kodi, VLC, etc.)
// without altering the actual audio data.
//
// ## How it works
//
// 1. For each M4A file, runs FFmpeg with the `ebur128` audio filter and
//    `peak=true` to measure integrated loudness (LUFS) and true peak (dBFS).
// 2. Calculates the ReplayGain adjustment: `gain = -18.0 - integrated_loudness`
//    where -18.0 LUFS is the standard reference level.
// 3. Writes two freeform atoms:
//    - `----:com.apple.iTunes:replaygain_track_gain` → e.g., "-4.20 dB"
//    - `----:com.apple.iTunes:replaygain_track_peak` → e.g., "0.933254"
//
// ## Reference Level
//
// The standard ReplayGain reference level is -18.0 LUFS (per EBU R128).
// A file at exactly -18.0 LUFS gets a gain of 0.0 dB. Louder files get
// negative gain (turn down), quieter files get positive gain (turn up).
//
// ## Non-destructive
//
// ReplayGain tags are metadata-only. The audio bitstream is not modified.
// Players that don't understand ReplayGain simply ignore the tags.
//
// ## Opt-in
//
// ReplayGain analysis is opt-in (`replaygain_enabled` in settings) because
// FFmpeg must decode and analyse the entire audio file, which takes time
// proportional to the file's duration.
//
// @see https://wiki.hydrogenaud.io/index.php?title=ReplayGain_specification
// @see https://ffmpeg.org/ffmpeg-filters.html#ebur128-1

use std::path::{Path, PathBuf};

use mp4ameta::{Data, FreeformIdent, Tag};
use tauri::AppHandle;

use crate::services::dependency_manager;

/// Apple iTunes freeform atom namespace (standard for `ReplayGain` in M4A files).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// Default `ReplayGain` reference level in LUFS (EBU R128 standard).
/// Used when the user hasn't configured a custom level.
pub const DEFAULT_REFERENCE_LEVEL: f64 = -18.0;

// ============================================================
// Public Types
// ============================================================

/// Result of a `ReplayGain` loudness analysis for a single track.
#[derive(Debug, Clone)]
pub struct ReplayGainResult {
    /// Integrated loudness in LUFS (e.g., -14.2)
    pub integrated_loudness: f64,
    /// True peak in linear scale (e.g., 0.933254)
    pub true_peak: f64,
    /// Calculated gain adjustment in dB (e.g., -3.80)
    pub gain_db: f64,
}

// ============================================================
// Public API
// ============================================================

/// Process all M4A files in the output directory for `ReplayGain` analysis.
///
/// Analyses every track individually for track gain, then optionally computes
/// album-level gain from the collective loudness measurements. When
/// `include_album_gain` is true, writes 4 tags per file:
/// `replaygain_track_gain`, `replaygain_track_peak`, `replaygain_album_gain`,
/// `replaygain_album_peak`. When false, only writes the 2 track-level tags.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for tool path resolution
/// * `output_path` - Download output path (file or album directory)
/// * `reference_level` - Target loudness in LUFS (e.g., -18.0 for EBU R128)
/// * `prevent_clipping` - When true, limits gain so peak × gain never exceeds 1.0
/// * `include_album_gain` - When true, computes and writes album-level gain tags
/// * `on_progress` - Called BEFORE each file is analysed with
///   `(current_index_one_based, total_files)`. Lets the
///   enrichment task surface live "ReplayGain: track 5 of 19"
///   captions on the per-item progress bar (#574). Pass
///   `|_, _| {}` if you don't need progress updates.
/// * `file_locks` - Optional shared per-file write-coordination map
///   (#779 Option 2). When supplied, each per-file tag-write
///   acquires the lock for that file so it serialises with any
///   other stage (notably AcoustID) writing to the same file.
///   Pass `None` for standalone use.
///
/// # Returns
/// * `Ok(count)` - Number of files successfully analysed and tagged
/// * `Err(message)` - `FFmpeg` not installed or output path invalid
pub async fn process_replaygain_for_directory(
    app: &AppHandle,
    output_path: &str,
    reference_level: f64,
    prevent_clipping: bool,
    include_album_gain: bool,
    on_progress: impl Fn(usize, usize) + Send,
    file_locks: Option<&std::sync::Arc<crate::utils::file_locks::FileWriteLocks>>,
) -> Result<usize, String> {
    let ffmpeg_path = get_ffmpeg_path(app)?;

    let audio_files = collect_audio_files(output_path);
    if audio_files.is_empty() {
        return Ok(0);
    }

    // Phase 1: Analyse all tracks individually
    let mut track_results: Vec<(PathBuf, ReplayGainResult)> = Vec::new();

    let total_files = audio_files.len();
    for (idx, file_path) in audio_files.iter().enumerate() {
        on_progress(idx + 1, total_files);
        log::info!(
            "ReplayGain: analysing file {}/{} — {}",
            idx + 1,
            total_files,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
        match analyse_track_loudness(&ffmpeg_path, file_path, reference_level).await {
            Ok(mut result) => {
                // gain_db is already computed against `reference_level` by the
                // shared crate's analyser; only apply clipping prevention here.

                // Clipping prevention: limit gain so peak × gain_linear ≤ 1.0
                if prevent_clipping && result.true_peak > 0.0 {
                    let max_gain_db = -20.0 * result.true_peak.log10();
                    if result.gain_db > max_gain_db {
                        log::debug!(
                            "ReplayGain clipping prevention: {} clamped from {:.2} to {:.2} dB",
                            file_path.display(),
                            result.gain_db,
                            max_gain_db
                        );
                        result.gain_db = max_gain_db;
                    }
                }

                track_results.push((file_path.clone(), result));
            }
            Err(e) => {
                log::debug!("ReplayGain analysis failed for {}: {}", file_path.display(), e);
            }
        }
    }

    if track_results.is_empty() {
        return Ok(0);
    }

    // Phase 2: Compute album-level gain (when enabled)
    // Album integrated loudness = average of all track loudness values (in linear power domain)
    // Formula: -0.691 + 10*log10(mean(10^((Li+0.691)/10))) for each track loudness Li
    // Simplified: average the linear power, convert back to LUFS
    let (album_gain_db, album_peak) = if include_album_gain {
        let album_loudness = {
            let sum: f64 = track_results
                .iter()
                .map(|(_, r)| 10.0_f64.powf(r.integrated_loudness / 10.0))
                .sum();
            let mean = sum / track_results.len() as f64;
            10.0 * mean.log10()
        };
        let mut gain = reference_level - album_loudness;

        // Album peak = highest true peak across all tracks
        let peak = track_results
            .iter()
            .map(|(_, r)| r.true_peak)
            .fold(0.0_f64, f64::max);

        // Clipping prevention for album gain
        if prevent_clipping && peak > 0.0 {
            let max_album_gain = -20.0 * peak.log10();
            if gain > max_album_gain {
                gain = max_album_gain;
            }
        }

        log::info!(
            "ReplayGain album: {:.2} dB gain, {:.6} peak ({} tracks, ref={:.1} LUFS)",
            gain,
            peak,
            track_results.len(),
            reference_level
        );

        (Some(gain), Some(peak))
    } else {
        log::info!("ReplayGain album gain disabled — writing track-level tags only");
        (None, None)
    };

    // Phase 3: Write tags to each file (4 tags when album gain enabled, 2 when not)
    let mut tagged_count = 0;
    for (file_path, result) in &track_results {
        match write_replaygain_tags(
            file_path,
            result.gain_db,
            result.true_peak,
            album_gain_db,
            album_peak,
            file_locks,
        )
        .await
        {
            Ok(()) => {
                if let Some(ag) = album_gain_db {
                    log::debug!(
                        "ReplayGain: {} → track={:.2} dB, album={:.2} dB",
                        file_path.display(),
                        result.gain_db,
                        ag
                    );
                } else {
                    log::debug!(
                        "ReplayGain: {} → track={:.2} dB",
                        file_path.display(),
                        result.gain_db,
                    );
                }
                tagged_count += 1;
            }
            Err(e) => {
                log::debug!("ReplayGain tagging failed for {}: {}", file_path.display(), e);
            }
        }
    }

    if tagged_count > 0 {
        log::info!(
            "Analysed {} of {} file(s) for ReplayGain (ref={:.1} LUFS, clipping_prevention={}, album_gain={})",
            tagged_count,
            audio_files.len(),
            reference_level,
            prevent_clipping,
            include_album_gain
        );
    }

    Ok(tagged_count)
}

// ============================================================
// Internal: Per-File Analysis and Tagging
// ============================================================

/// Write `ReplayGain` tags to a single audio file.
///
/// Dispatches to the appropriate tagging mechanism based on file format:
/// - **MP4-family** (M4A, M4V, MP4, M4P, M4B): iTunes freeform atoms via `mp4ameta`
/// - **Vorbis Comment** (FLAC, OGG, OGA, Opus): Standard ReplayGain Vorbis Comment fields via `lofty`
/// - **ID3v2** (MP3): TXXX user-defined text frames via `lofty`
///
/// Always writes track-level tags. Album-level tags are only written
/// when the album gain values are provided (`Some`).
///
/// Blocking file I/O is offloaded to `spawn_blocking` to prevent starving
/// the tokio runtime on slow filesystems (FUSE mounts, NFS, cloud storage).
async fn write_replaygain_tags(
    file_path: &Path,
    track_gain_db: f64,
    track_peak: f64,
    album_gain_db: Option<f64>,
    album_peak: Option<f64>,
    file_locks: Option<&std::sync::Arc<crate::utils::file_locks::FileWriteLocks>>,
) -> Result<(), String> {
    let format = detect_format(file_path).ok_or_else(|| {
        format!(
            "Unsupported format for ReplayGain tagging: {}",
            file_path.display()
        )
    })?;

    // Acquire the per-file write lock (#779 Option 2) BEFORE the
    // backend dispatch — whether it's mp4ameta or lofty doing the
    // write, it's the SAME file on disk that AcoustID might also
    // be writing to. Held for the full read-modify-write cycle of
    // the format-specific writer.
    let _write_guard = match file_locks {
        Some(locks) => Some(locks.lock(file_path).await),
        None => None,
    };

    match format {
        AudioFormat::Mp4 => {
            write_replaygain_mp4(file_path, track_gain_db, track_peak, album_gain_db, album_peak)
                .await
        }
        AudioFormat::VorbisComment | AudioFormat::Id3v2 => {
            write_replaygain_lofty(
                file_path,
                track_gain_db,
                track_peak,
                album_gain_db,
                album_peak,
            )
            .await
        }
    }
}

/// Write `ReplayGain` tags to an MP4-family file via `mp4ameta`.
///
/// Uses iTunes freeform atoms under the `com.apple.iTunes` namespace,
/// the de facto standard for ReplayGain in MP4/M4A containers.
async fn write_replaygain_mp4(
    file_path: &Path,
    track_gain_db: f64,
    track_peak: f64,
    album_gain_db: Option<f64>,
    album_peak: Option<f64>,
) -> Result<(), String> {
    let tag_path = file_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut tag =
            Tag::read_from_path(&tag_path).map_err(|e| format!("Failed to read MP4: {e}"))?;

        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_track_gain"),
            Data::Utf8(format!("{track_gain_db:.2} dB")),
        );
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_track_peak"),
            Data::Utf8(format!("{track_peak:.6}")),
        );

        if let (Some(ag), Some(ap)) = (album_gain_db, album_peak) {
            tag.set_data(
                FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_album_gain"),
                Data::Utf8(format!("{ag:.2} dB")),
            );
            tag.set_data(
                FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_album_peak"),
                Data::Utf8(format!("{ap:.6}")),
            );
        }

        tag.write_to_path(&tag_path)
            .map_err(|e| format!("Failed to write MP4: {e}"))?;

        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("ReplayGain MP4 tag task panicked: {e}"))??;

    Ok(())
}

/// Write `ReplayGain` tags to FLAC/OGG/Opus/MP3 files via `lofty`.
///
/// - FLAC/OGG/Opus: Vorbis Comment fields (the ReplayGain specification's native format)
/// - MP3: ID3v2 TXXX user-defined text frames
///
/// Uses uppercase key names (`REPLAYGAIN_TRACK_GAIN`) per the ReplayGain spec.
/// `lofty` automatically selects the correct tag type based on the file format.
async fn write_replaygain_lofty(
    file_path: &Path,
    track_gain_db: f64,
    track_peak: f64,
    album_gain_db: Option<f64>,
    album_peak: Option<f64>,
) -> Result<(), String> {
    let tag_path = file_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use lofty::config::WriteOptions;
        use lofty::prelude::*;

        let mut tagged_file = lofty::read_from_path(&tag_path)
            .map_err(|e| format!("Failed to read {}: {e}", tag_path.display()))?;

        // Get or create the primary tag for this format.
        // lofty picks the correct tag type: VorbisComments for FLAC/OGG, ID3v2 for MP3.
        let tag = match tagged_file.primary_tag_mut() {
            Some(t) => t,
            None => {
                // No existing tag — insert a new primary tag
                let tag_type = tagged_file.primary_tag_type();
                tagged_file.insert_tag(lofty::tag::Tag::new(tag_type));
                tagged_file
                    .primary_tag_mut()
                    .ok_or_else(|| "Failed to create primary tag".to_string())?
            }
        };

        // Write track-level tags
        tag.insert_text(ItemKey::ReplayGainTrackGain, format!("{track_gain_db:.2} dB"));
        tag.insert_text(ItemKey::ReplayGainTrackPeak, format!("{track_peak:.6}"));

        // Write album-level tags (when enabled)
        if let (Some(ag), Some(ap)) = (album_gain_db, album_peak) {
            tag.insert_text(ItemKey::ReplayGainAlbumGain, format!("{ag:.2} dB"));
            tag.insert_text(ItemKey::ReplayGainAlbumPeak, format!("{ap:.6}"));
        }

        tagged_file
            .save_to_path(&tag_path, WriteOptions::default())
            .map_err(|e| format!("Failed to write {}: {e}", tag_path.display()))?;

        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("ReplayGain lofty tag task panicked: {e}"))??;

    Ok(())
}

// ============================================================
// Internal: Loudness Analysis (via FFmpeg ebur128)
// ============================================================

/// Resolve the managed `FFmpeg` binary path.
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_bin.exists() {
        return Err("FFmpeg not installed — required for ReplayGain analysis".to_string());
    }
    Ok(ffmpeg_bin)
}

/// Analyse a single audio file's loudness via the shared
/// [`meedya_fingerprint::ReplayGainAnalyzer`] (#353 Phase 2).
///
/// Spawns `ffmpeg -i file -af ebur128=peak=true -f null -` inside the shared
/// crate, parses the `Summary:` block, and returns a MeedyaDL-local
/// [`ReplayGainResult`] with `gain_db` already computed against
/// `reference_level`. The shared crate emits its own 4-field result type
/// (it carries `reference_level` for round-trip serialisation); we drop that
/// field here because MeedyaDL's per-file tag writers don't need it.
async fn analyse_track_loudness(
    ffmpeg_path: &Path,
    file_path: &Path,
    reference_level: f64,
) -> Result<ReplayGainResult, String> {
    let analyzer = meedya_fingerprint::ReplayGainAnalyzer::new(
        ffmpeg_path.to_string_lossy().into_owned(),
    )
    .with_reference_level(reference_level);
    map_shared_replaygain_result(analyzer.analyze_track(file_path).await)
}

/// Convert a shared-crate [`meedya_fingerprint::ReplayGainResult`] into
/// MeedyaDL's local 3-field [`ReplayGainResult`], and map the shared error
/// variants back to the historical `String` error messages so call-sites
/// (and the `log::debug!` line in the caller) keep their existing surface.
///
/// Extracted as a pure helper so it can be unit-tested without spawning
/// FFmpeg — mirrors the Phase 1 `map_shared_acoustid_result` pattern.
fn map_shared_replaygain_result(
    result: Result<meedya_fingerprint::ReplayGainResult, meedya_fingerprint::FingerprintError>,
) -> Result<ReplayGainResult, String> {
    match result {
        Ok(r) => Ok(ReplayGainResult {
            integrated_loudness: r.integrated_loudness,
            true_peak: r.true_peak,
            gain_db: r.gain_db,
        }),
        Err(meedya_fingerprint::FingerprintError::FfmpegNotFound(path)) => {
            Err(format!("FFmpeg not found at expected path: {path}"))
        }
        Err(meedya_fingerprint::FingerprintError::FfmpegError(msg)) => {
            Err(format!("Failed to spawn FFmpeg: {msg}"))
        }
        Err(meedya_fingerprint::FingerprintError::LoudnessParseError(msg)) => Err(msg),
        Err(other) => Err(format!("ReplayGain analysis failed: {other}")),
    }
}

// ============================================================
// Internal: File Collection
// ============================================================

/// Audio format families for `ReplayGain` tag writing.
///
/// Each family uses a different tagging mechanism:
/// - `Mp4`: iTunes freeform atoms via `mp4ameta` (M4A, M4V, MP4, M4P, M4B)
/// - `VorbisComment`: Vorbis Comment fields via `lofty` (FLAC, OGG, OGA, Opus)
/// - `Id3v2`: TXXX user-defined text frames via `lofty` (MP3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioFormat {
    Mp4,
    VorbisComment,
    Id3v2,
}

/// All file extensions supported for `ReplayGain` analysis and tagging.
///
/// The second element indicates which tagging mechanism to use.
const SUPPORTED_EXTENSIONS: &[(&str, AudioFormat)] = &[
    // MP4-family (Apple Music: audio + video)
    ("m4a", AudioFormat::Mp4),
    ("m4v", AudioFormat::Mp4),
    ("mp4", AudioFormat::Mp4),
    ("m4p", AudioFormat::Mp4),
    ("m4b", AudioFormat::Mp4),
    // FLAC (future: Spotify/YouTube)
    ("flac", AudioFormat::VorbisComment),
    // OGG Vorbis / Opus (future: Spotify via votify, YouTube via yt-dlp)
    ("ogg", AudioFormat::VorbisComment),
    ("oga", AudioFormat::VorbisComment),
    ("opus", AudioFormat::VorbisComment),
    // MP3 (future: YouTube via yt-dlp)
    ("mp3", AudioFormat::Id3v2),
    // Matroska / WebM / OGV video containers (#329)
    // ReplayGain tags written via Vorbis Comments (lofty supports these)
    ("mkv", AudioFormat::VorbisComment),
    ("webm", AudioFormat::VorbisComment),
    ("ogv", AudioFormat::VorbisComment),
];

/// Determine the audio format of a file by its extension.
fn detect_format(path: &Path) -> Option<AudioFormat> {
    let ext = path.extension()?.to_str()?;
    SUPPORTED_EXTENSIONS
        .iter()
        .find(|(e, _)| e.eq_ignore_ascii_case(ext))
        .map(|(_, fmt)| *fmt)
}

/// Collect all supported audio/video file paths from the output path.
fn collect_audio_files(output_path: &str) -> Vec<PathBuf> {
    let path = Path::new(output_path);
    let mut files = Vec::new();

    if path.is_file() {
        if detect_format(path).is_some() {
            files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        // Migrated to walk_dir_depth in v1.0.8 (#716/1). max_depth=3
        // matches GAMDL's natural Output/Artist/Album/file shape;
        // filesystem sidecars (._*, .DS_Store, Thumbs.db) skipped to
        // avoid FFmpeg loudness-analysis noise on non-audio binaries
        // (#577).
        files.extend(crate::utils::fs_walk::walk_dir_depth(path, 3, |p| {
            if crate::utils::fs_safe::is_filesystem_sidecar(p) {
                return None;
            }
            if p.is_file() && detect_format(p).is_some() {
                Some(p.to_path_buf())
            } else {
                None
            }
        }));
    }

    files
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Shared-crate adapter tests (#353 Phase 2)
    //
    // ebur128 *parsing* is now owned by `meedya-fingerprint` and exercised
    // by its own test suite. The tests below pin the MeedyaDL-side adapter
    // — error-variant mapping and field-projection from the shared
    // 4-field result to the local 3-field result — so future shared-crate
    // bumps that quietly change either side surface here, not at runtime.
    // ----------------------------------------------------------

    #[test]
    fn adapter_projects_shared_result_into_local_struct() {
        // Shared crate returns 4 fields (carries `reference_level`); the
        // adapter drops it. Verifies field-by-field projection.
        let shared = meedya_fingerprint::ReplayGainResult {
            integrated_loudness: -14.2,
            true_peak: 0.933254,
            gain_db: -3.8,
            reference_level: -18.0,
        };
        let mapped = map_shared_replaygain_result(Ok(shared)).expect("Ok variant");
        assert!((mapped.integrated_loudness - (-14.2)).abs() < f64::EPSILON);
        assert!((mapped.true_peak - 0.933254).abs() < f64::EPSILON);
        assert!((mapped.gain_db - (-3.8)).abs() < f64::EPSILON);
    }

    #[test]
    fn adapter_maps_ffmpeg_not_found_with_path() {
        let err = map_shared_replaygain_result(Err(
            meedya_fingerprint::FingerprintError::FfmpegNotFound("/opt/ffmpeg".into()),
        ))
        .expect_err("Err variant");
        assert!(
            err.contains("FFmpeg not found"),
            "expected `FFmpeg not found` prefix, got: {err}"
        );
        assert!(
            err.contains("/opt/ffmpeg"),
            "missing path in error: {err}"
        );
    }

    #[test]
    fn adapter_maps_ffmpeg_spawn_error() {
        let err = map_shared_replaygain_result(Err(
            meedya_fingerprint::FingerprintError::FfmpegError("permission denied".into()),
        ))
        .expect_err("Err variant");
        assert!(
            err.contains("Failed to spawn FFmpeg"),
            "expected historical message, got: {err}"
        );
        assert!(err.contains("permission denied"));
    }

    #[test]
    fn adapter_passes_through_loudness_parse_error_verbatim() {
        // The shared crate's parser error string is already user-readable;
        // we preserve it verbatim so existing log scrapers stay aligned.
        let err = map_shared_replaygain_result(Err(
            meedya_fingerprint::FingerprintError::LoudnessParseError(
                "Could not find integrated loudness (I:) in FFmpeg output".into(),
            ),
        ))
        .expect_err("Err variant");
        assert!(err.contains("integrated loudness"), "got: {err}");
    }

    #[test]
    fn adapter_wraps_unrelated_errors_with_fallback_prefix() {
        // Network/AcoustID variants shouldn't appear from a ReplayGain call,
        // but if a future shared-crate refactor produces one, the fallback
        // arm should still produce a parseable error string.
        let err = map_shared_replaygain_result(Err(
            meedya_fingerprint::FingerprintError::NetworkError("DNS failure".into()),
        ))
        .expect_err("Err variant");
        assert!(
            err.starts_with("ReplayGain analysis failed:"),
            "expected fallback prefix, got: {err}"
        );
        assert!(err.contains("DNS failure"));
    }

    #[test]
    fn shared_crate_public_api_surface_unchanged() {
        // Pins the bits of `meedya-fingerprint` we depend on. If upstream
        // renames `analyze_track`, drops `with_reference_level`, or changes
        // either result type's fields, compilation here breaks first.
        let analyzer = meedya_fingerprint::ReplayGainAnalyzer::new("ffmpeg")
            .with_reference_level(-18.0);
        // Pin `analyze_track`'s existence and arity via a local `async fn`.
        // We never invoke it (FFmpeg isn't on $PATH in CI); if upstream
        // renames the method or changes the parameter list, this stops
        // compiling.
        #[allow(dead_code)]
        async fn pin_analyze_track(
            a: &meedya_fingerprint::ReplayGainAnalyzer,
            p: &Path,
        ) -> Result<
            meedya_fingerprint::ReplayGainResult,
            meedya_fingerprint::FingerprintError,
        > {
            a.analyze_track(p).await
        }
        let _: fn(_, _) -> _ = pin_analyze_track; // suppress unused-fn warning

        // `compute_album_gain` is the public album aggregation entry point;
        // pin its presence even though Phase 2 still uses MeedyaDL's local
        // computation (kept to preserve clipping-prevention semantics).
        let album = analyzer.compute_album_gain(&[]);
        assert!(album.is_none(), "empty input must yield None");

        // Pin the fields we project in the adapter.
        let r = meedya_fingerprint::ReplayGainResult {
            integrated_loudness: 0.0,
            true_peak: 0.0,
            gain_db: 0.0,
            reference_level: 0.0,
        };
        let _ = (r.integrated_loudness, r.true_peak, r.gain_db, r.reference_level);
    }

    #[test]
    fn default_reference_level_matches_shared_crate() {
        // The local constant exists for backwards compatibility with
        // existing call-sites; if the shared crate ever changes its
        // EBU R128 default, we want to find out at test time.
        assert!(
            (DEFAULT_REFERENCE_LEVEL - meedya_fingerprint::DEFAULT_REFERENCE_LEVEL).abs()
                < f64::EPSILON
        );
    }

    // ----------------------------------------------------------
    // Gain formatting tests
    // ----------------------------------------------------------

    #[test]
    fn gain_format_negative() {
        let gain = -3.8_f64;
        assert_eq!(format!("{:.2} dB", gain), "-3.80 dB");
    }

    #[test]
    fn gain_format_positive() {
        let gain = 6.5_f64;
        assert_eq!(format!("{:.2} dB", gain), "6.50 dB");
    }

    #[test]
    fn peak_format_linear() {
        let peak = 0.933254_f64;
        assert_eq!(format!("{:.6}", peak), "0.933254");
    }

    // ----------------------------------------------------------
    // Format detection tests
    // ----------------------------------------------------------

    #[test]
    fn detect_mp4_family() {
        assert_eq!(detect_format(Path::new("track.m4a")), Some(AudioFormat::Mp4));
        assert_eq!(detect_format(Path::new("video.m4v")), Some(AudioFormat::Mp4));
        assert_eq!(detect_format(Path::new("video.mp4")), Some(AudioFormat::Mp4));
        assert_eq!(detect_format(Path::new("book.m4b")), Some(AudioFormat::Mp4));
        assert_eq!(detect_format(Path::new("drm.m4p")), Some(AudioFormat::Mp4));
        // Case-insensitive
        assert_eq!(detect_format(Path::new("TRACK.M4A")), Some(AudioFormat::Mp4));
        assert_eq!(detect_format(Path::new("video.MP4")), Some(AudioFormat::Mp4));
    }

    #[test]
    fn detect_vorbis_comment_formats() {
        assert_eq!(detect_format(Path::new("track.flac")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("track.ogg")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("track.oga")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("track.opus")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("TRACK.FLAC")), Some(AudioFormat::VorbisComment));
    }

    #[test]
    fn detect_id3v2_formats() {
        assert_eq!(detect_format(Path::new("track.mp3")), Some(AudioFormat::Id3v2));
        assert_eq!(detect_format(Path::new("TRACK.MP3")), Some(AudioFormat::Id3v2));
    }

    #[test]
    fn detect_unsupported_returns_none() {
        assert_eq!(detect_format(Path::new("readme.txt")), None);
        assert_eq!(detect_format(Path::new("image.png")), None);
    }

    #[test]
    fn detect_video_containers_returns_vorbis_comment() {
        // MKV/WebM/OGV video containers use VorbisComment tags (#329)
        assert_eq!(detect_format(Path::new("video.mkv")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("video.webm")), Some(AudioFormat::VorbisComment));
        assert_eq!(detect_format(Path::new("video.ogv")), Some(AudioFormat::VorbisComment));
    }
}
