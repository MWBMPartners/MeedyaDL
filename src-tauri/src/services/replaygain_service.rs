// Copyright (c) 2026 MeedyaDL
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
use tokio::process::Command;

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
        log::info!(
            "ReplayGain: analysing file {}/{} — {}",
            idx + 1,
            total_files,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
        match analyse_track_loudness(&ffmpeg_path, file_path).await {
            Ok(mut result) => {
                // Calculate track gain from the reference level
                result.gain_db = reference_level - result.integrated_loudness;

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
) -> Result<(), String> {
    let format = detect_format(file_path).ok_or_else(|| {
        format!(
            "Unsupported format for ReplayGain tagging: {}",
            file_path.display()
        )
    })?;

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

/// Analyse a single audio file's loudness using `FFmpeg`'s ebur128 filter.
///
/// Runs `ffmpeg -i file -af ebur128=peak=true -f null -` and parses the
/// Summary section of stderr for integrated loudness and true peak values.
///
/// # Returns
/// * `Ok(ReplayGainResult)` - Loudness measurements and calculated gain
/// * `Err(message)` - `FFmpeg` execution or parsing failed
async fn analyse_track_loudness(
    ffmpeg_path: &Path,
    file_path: &Path,
) -> Result<ReplayGainResult, String> {
    let output = Command::new(ffmpeg_path)
        .args(["-i"])
        .arg(file_path)
        .args(["-af", "ebur128=peak=true", "-f", "null", "-"])
        .output()
        .await
        .map_err(|e| format!("Failed to spawn FFmpeg: {e}"))?;

    // ebur128 writes its output to stderr (FFmpeg convention)
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse the Summary section from ebur128 output.
    // The output format is:
    //   [Parsed_ebur128_0 @ ...] Summary:
    //
    //     Integrated loudness:
    //       I:         -14.2 LUFS
    //       Threshold: -24.2 LUFS
    //
    //     True peak:
    //       Peak:       -0.6 dBFS
    parse_ebur128_output(&stderr)
}

/// Parse `FFmpeg` ebur128 filter output to extract loudness measurements.
///
/// Looks for the "Summary:" section in stderr and extracts:
/// - Integrated loudness (I: value in LUFS)
/// - True peak (Peak: value in dBFS, converted to linear scale)
fn parse_ebur128_output(stderr: &str) -> Result<ReplayGainResult, String> {
    // Find the Summary section
    let summary_start = stderr
        .find("Summary:")
        .ok_or("No Summary section in ebur128 output")?;
    let summary = &stderr[summary_start..];

    // Extract integrated loudness: "I:         -14.2 LUFS"
    let integrated_loudness = parse_lufs_value(summary, "I:")
        .ok_or("Failed to parse integrated loudness (I:) from ebur128 output")?;

    // Extract true peak: "Peak:       -0.6 dBFS"
    let peak_dbfs = parse_dbfs_value(summary, "Peak:")
        .ok_or("Failed to parse true peak (Peak:) from ebur128 output")?;

    // Convert peak from dBFS to linear scale: 10^(dBFS/20)
    let true_peak = 10.0_f64.powf(peak_dbfs / 20.0);

    // gain_db is calculated by the caller with the user's reference level.
    // Return raw measurements only.
    Ok(ReplayGainResult {
        integrated_loudness,
        true_peak,
        gain_db: 0.0, // Placeholder — caller sets this from reference_level
    })
}

/// Extract a LUFS value from ebur128 output text.
///
/// Looks for a line matching: `{key}  {spaces}  {number} LUFS`
fn parse_lufs_value(text: &str, key: &str) -> Option<f64> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(after_key) = trimmed.strip_prefix(key) {
            // Extract the numeric value before "LUFS"
            let value_str = after_key.trim().trim_end_matches("LUFS").trim();
            return value_str.parse::<f64>().ok();
        }
    }
    None
}

/// Extract a dBFS value from ebur128 output text.
///
/// Looks for a line matching: `{key}  {spaces}  {number} dBFS`
fn parse_dbfs_value(text: &str, key: &str) -> Option<f64> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(after_key) = trimmed.strip_prefix(key) {
            let value_str = after_key.trim().trim_end_matches("dBFS").trim();
            return value_str.parse::<f64>().ok();
        }
    }
    None
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
        collect_audio_recursive(path, &mut files);
    }

    files
}

/// Recursively collect supported audio/video file paths from a directory tree.
///
/// Skips filesystem sidecars (macOS `._*` AppleDouble, `.DS_Store`,
/// Windows `Thumbs.db`, etc.) — running FFmpeg loudness analysis on
/// a non-audio binary fails and contributes noise to the activity
/// log (#577).
fn collect_audio_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if crate::utils::fs_safe::is_filesystem_sidecar(&path) {
            continue;
        }
        if path.is_dir() {
            collect_audio_recursive(&path, files);
        } else if detect_format(&path).is_some() {
            files.push(path);
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
    // ebur128 output parsing tests
    // ----------------------------------------------------------

    const SAMPLE_EBUR128_OUTPUT: &str = r"
[Parsed_ebur128_0 @ 0x7f8b8c000000] Summary:

  Integrated loudness:
    I:         -14.2 LUFS
    Threshold: -24.2 LUFS

  Loudness range:
    LRA:         7.1 LU
    Threshold:  -34.2 LUFS
    LRA low:    -18.8 LUFS
    LRA high:   -11.7 LUFS

  True peak:
    Peak:        -0.6 dBFS
";

    #[test]
    fn parse_integrated_loudness() {
        let result = parse_ebur128_output(SAMPLE_EBUR128_OUTPUT).unwrap();
        assert!((result.integrated_loudness - (-14.2)).abs() < 0.01);
    }

    #[test]
    fn parse_true_peak() {
        let result = parse_ebur128_output(SAMPLE_EBUR128_OUTPUT).unwrap();
        // -0.6 dBFS → linear: 10^(-0.6/20) ≈ 0.933254
        assert!((result.true_peak - 0.933254).abs() < 0.001);
    }

    #[test]
    fn calculate_gain_correctly() {
        let result = parse_ebur128_output(SAMPLE_EBUR128_OUTPUT).unwrap();
        // gain_db is now calculated by the caller, not the parser.
        // Verify the caller's formula: reference_level - integrated_loudness
        let gain = DEFAULT_REFERENCE_LEVEL - result.integrated_loudness;
        // Reference: -18.0 LUFS, integrated: -14.2 LUFS
        // Gain = -18.0 - (-14.2) = -3.8 dB
        assert!((gain - (-3.8)).abs() < 0.01);
    }

    #[test]
    fn parse_quiet_track() {
        let output = r"
[Parsed_ebur128_0 @ 0x0] Summary:

  Integrated loudness:
    I:         -24.5 LUFS
    Threshold: -34.5 LUFS

  True peak:
    Peak:        -6.0 dBFS
";
        let result = parse_ebur128_output(output).unwrap();
        assert!((result.integrated_loudness - (-24.5)).abs() < 0.01);
        // Gain calculated by caller: -18.0 - (-24.5) = 6.5 dB (positive = turn up)
        let gain = DEFAULT_REFERENCE_LEVEL - result.integrated_loudness;
        assert!((gain - 6.5).abs() < 0.01);
    }

    #[test]
    fn parse_missing_summary_returns_error() {
        let output = "some random ffmpeg output without summary";
        assert!(parse_ebur128_output(output).is_err());
    }

    #[test]
    fn parse_lufs_value_extracts_correctly() {
        assert_eq!(
            parse_lufs_value("  I:         -14.2 LUFS", "I:"),
            Some(-14.2)
        );
        assert_eq!(parse_lufs_value("  I:         0.0 LUFS", "I:"), Some(0.0));
        assert_eq!(
            parse_lufs_value("  I:         -70.0 LUFS", "I:"),
            Some(-70.0)
        );
    }

    #[test]
    fn parse_dbfs_value_extracts_correctly() {
        assert_eq!(
            parse_dbfs_value("  Peak:        -0.6 dBFS", "Peak:"),
            Some(-0.6)
        );
        assert_eq!(
            parse_dbfs_value("  Peak:        0.0 dBFS", "Peak:"),
            Some(0.0)
        );
        assert_eq!(
            parse_dbfs_value("  Peak:        -12.3 dBFS", "Peak:"),
            Some(-12.3)
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
