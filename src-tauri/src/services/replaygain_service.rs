// Copyright (c) 2024-2026 MeedyaDL
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

/// Apple iTunes freeform atom namespace (standard for ReplayGain in M4A files).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// ReplayGain reference level in LUFS (EBU R128 standard).
const REFERENCE_LEVEL: f64 = -18.0;

// ============================================================
// Public Types
// ============================================================

/// Result of a ReplayGain loudness analysis for a single track.
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

/// Process all M4A files in the output directory for ReplayGain analysis.
///
/// For each file: analyses loudness using FFmpeg's ebur128 filter, calculates
/// the ReplayGain adjustment, and writes the gain and peak tags.
///
/// # Arguments
/// * `app` - Tauri AppHandle for tool path resolution
/// * `output_path` - Download output path (file or album directory)
///
/// # Returns
/// * `Ok(count)` - Number of files successfully analysed and tagged
/// * `Err(message)` - FFmpeg not installed or output path invalid
pub async fn process_replaygain_for_directory(
    app: &AppHandle,
    output_path: &str,
) -> Result<usize, String> {
    let ffmpeg_path = get_ffmpeg_path(app)?;

    let m4a_files = collect_m4a_files(output_path);
    if m4a_files.is_empty() {
        return Ok(0);
    }

    let mut tagged_count = 0;

    for file_path in &m4a_files {
        match analyse_and_tag(&ffmpeg_path, file_path).await {
            Ok(result) => {
                log::debug!(
                    "ReplayGain: {} → gain={:.2} dB, peak={:.6}",
                    file_path.display(),
                    result.gain_db,
                    result.true_peak
                );
                tagged_count += 1;
            }
            Err(e) => {
                log::debug!("ReplayGain failed for {}: {}", file_path.display(), e);
            }
        }
    }

    if tagged_count > 0 {
        log::info!(
            "Analysed {} of {} file(s) for ReplayGain",
            tagged_count,
            m4a_files.len()
        );
    }

    Ok(tagged_count)
}

// ============================================================
// Internal: Per-File Analysis and Tagging
// ============================================================

/// Analyse a single file's loudness and write ReplayGain tags.
async fn analyse_and_tag(
    ffmpeg_path: &Path,
    file_path: &Path,
) -> Result<ReplayGainResult, String> {
    // Analyse loudness
    let result = analyse_track_loudness(ffmpeg_path, file_path).await?;

    // Write tags
    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read M4A: {}", e))?;

    // replaygain_track_gain — e.g., "-4.20 dB"
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_track_gain"),
        Data::Utf8(format!("{:.2} dB", result.gain_db)),
    );

    // replaygain_track_peak — e.g., "0.933254" (linear scale)
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "replaygain_track_peak"),
        Data::Utf8(format!("{:.6}", result.true_peak)),
    );

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write M4A: {}", e))?;

    Ok(result)
}

// ============================================================
// Internal: Loudness Analysis (via FFmpeg ebur128)
// ============================================================

/// Resolve the managed FFmpeg binary path.
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_bin.exists() {
        return Err("FFmpeg not installed — required for ReplayGain analysis".to_string());
    }
    Ok(ffmpeg_bin)
}

/// Analyse a single audio file's loudness using FFmpeg's ebur128 filter.
///
/// Runs `ffmpeg -i file -af ebur128=peak=true -f null -` and parses the
/// Summary section of stderr for integrated loudness and true peak values.
///
/// # Returns
/// * `Ok(ReplayGainResult)` - Loudness measurements and calculated gain
/// * `Err(message)` - FFmpeg execution or parsing failed
async fn analyse_track_loudness(
    ffmpeg_path: &Path,
    file_path: &Path,
) -> Result<ReplayGainResult, String> {
    let output = Command::new(ffmpeg_path)
        .args([
            "-i",
        ])
        .arg(file_path)
        .args([
            "-af", "ebur128=peak=true",
            "-f", "null",
            "-",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

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

/// Parse FFmpeg ebur128 filter output to extract loudness measurements.
///
/// Looks for the "Summary:" section in stderr and extracts:
/// - Integrated loudness (I: value in LUFS)
/// - True peak (Peak: value in dBFS, converted to linear scale)
fn parse_ebur128_output(stderr: &str) -> Result<ReplayGainResult, String> {
    // Find the Summary section
    let summary_start = stderr.find("Summary:")
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

    // Calculate ReplayGain: reference_level - integrated_loudness
    let gain_db = REFERENCE_LEVEL - integrated_loudness;

    Ok(ReplayGainResult {
        integrated_loudness,
        true_peak,
        gain_db,
    })
}

/// Extract a LUFS value from ebur128 output text.
///
/// Looks for a line matching: `{key}  {spaces}  {number} LUFS`
fn parse_lufs_value(text: &str, key: &str) -> Option<f64> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            // Extract the numeric value before "LUFS"
            let after_key = trimmed[key.len()..].trim();
            let value_str = after_key.trim_end_matches("LUFS").trim();
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
        if trimmed.starts_with(key) {
            let after_key = trimmed[key.len()..].trim();
            let value_str = after_key.trim_end_matches("dBFS").trim();
            return value_str.parse::<f64>().ok();
        }
    }
    None
}

// ============================================================
// Internal: File Collection
// ============================================================

/// Collect all M4A file paths from the output path.
fn collect_m4a_files(output_path: &str) -> Vec<PathBuf> {
    let path = Path::new(output_path);
    let mut files = Vec::new();

    if path.is_file() {
        if is_m4a(path) {
            files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        collect_m4a_recursive(path, &mut files);
    }

    files
}

/// Recursively collect M4A file paths from a directory tree.
fn collect_m4a_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_m4a_recursive(&path, files);
        } else if is_m4a(&path) {
            files.push(path);
        }
    }
}

/// Checks whether a file path has an `.m4a` extension (case-insensitive).
fn is_m4a(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("m4a"))
        .unwrap_or(false)
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

    const SAMPLE_EBUR128_OUTPUT: &str = r#"
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
"#;

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
        // Reference: -18.0 LUFS, integrated: -14.2 LUFS
        // Gain = -18.0 - (-14.2) = -3.8 dB
        assert!((result.gain_db - (-3.8)).abs() < 0.01);
    }

    #[test]
    fn parse_quiet_track() {
        let output = r#"
[Parsed_ebur128_0 @ 0x0] Summary:

  Integrated loudness:
    I:         -24.5 LUFS
    Threshold: -34.5 LUFS

  True peak:
    Peak:        -6.0 dBFS
"#;
        let result = parse_ebur128_output(output).unwrap();
        assert!((result.integrated_loudness - (-24.5)).abs() < 0.01);
        // Gain = -18.0 - (-24.5) = 6.5 dB (positive = turn up)
        assert!((result.gain_db - 6.5).abs() < 0.01);
    }

    #[test]
    fn parse_missing_summary_returns_error() {
        let output = "some random ffmpeg output without summary";
        assert!(parse_ebur128_output(output).is_err());
    }

    #[test]
    fn parse_lufs_value_extracts_correctly() {
        assert_eq!(parse_lufs_value("  I:         -14.2 LUFS", "I:"), Some(-14.2));
        assert_eq!(parse_lufs_value("  I:         0.0 LUFS", "I:"), Some(0.0));
        assert_eq!(parse_lufs_value("  I:         -70.0 LUFS", "I:"), Some(-70.0));
    }

    #[test]
    fn parse_dbfs_value_extracts_correctly() {
        assert_eq!(parse_dbfs_value("  Peak:        -0.6 dBFS", "Peak:"), Some(-0.6));
        assert_eq!(parse_dbfs_value("  Peak:        0.0 dBFS", "Peak:"), Some(0.0));
        assert_eq!(parse_dbfs_value("  Peak:        -12.3 dBFS", "Peak:"), Some(-12.3));
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
}
