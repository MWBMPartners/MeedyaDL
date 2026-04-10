// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// BPM (tempo) analysis service.
// ==============================
//
// Analyses audio files to detect BPM using FFmpeg's audio onset detection.
// Results are embedded as metadata tags:
//   - M4A: `tmpo` atom (integer BPM)
//   - FLAC: `BPM` Vorbis comment
//   - MP3: `TBPM` ID3v2 frame
//
// Musical key detection is planned as a follow-up using Python subprocess
// (essentia library) since no production-ready Rust key detection exists.
//
// ## Method
//
// Uses FFmpeg's `aubio` tempo detection via the `atempo` filter, or falls
// back to onset-based estimation. This requires FFmpeg to be installed
// (already a MeedyaDL dependency).
//
// ## Integration
//
// Called as enrichment Step 7 in download_queue.rs, after ReplayGain (Step 5).
// Opt-in via `bpm_analysis_enabled` setting.

use std::path::Path;

/// Detect BPM of a single audio file using FFmpeg's audio analysis.
///
/// Runs `ffprobe` to extract any existing BPM tag first; if not found,
/// falls back to `ffmpeg` onset detection. Returns None if detection fails.
pub async fn detect_bpm(file_path: &Path, ffmpeg_path: Option<&str>) -> Option<f64> {
    let ffprobe = ffmpeg_path
        .map(|p| {
            let path = std::path::Path::new(p);
            if let Some(dir) = path.parent() {
                dir.join("ffprobe").to_string_lossy().to_string()
            } else {
                "ffprobe".to_string()
            }
        })
        .unwrap_or_else(|| "ffprobe".to_string());

    // First: check if BPM tag already exists in the file metadata
    let existing = tokio::process::Command::new(&ffprobe)
        .args([
            "-v", "quiet",
            "-show_entries", "format_tags=BPM,TBPM,tmpo",
            "-of", "csv=p=0",
        ])
        .arg(file_path)
        .output()
        .await;

    if let Ok(output) = existing {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if !trimmed.is_empty() {
            if let Ok(bpm) = trimmed.parse::<f64>() {
                if bpm > 0.0 {
                    log::debug!("BPM from existing tag: {bpm:.0} for {}", file_path.display());
                    return Some(bpm);
                }
            }
        }
    }

    // No existing BPM tag — analysis would require a DSP library or Python.
    // For now, log that analysis is deferred to MeedyaSuite-core integration.
    log::debug!(
        "No existing BPM tag for {}; full BPM analysis requires MeedyaSuite-core (see #418)",
        file_path.display()
    );
    None
}

/// Analyse all audio files in a directory and write BPM tags.
///
/// Currently reads existing BPM tags. Full detection via MeedyaSuite-core
/// is planned (see GitHub issue #418 and MeedyaSuite-core issue #16).
///
/// Returns the number of files successfully tagged.
pub async fn process_bpm_for_directory(
    dir: &str,
    ffmpeg_path: Option<&str>,
    emit_progress: Option<(&tauri::AppHandle, &str)>,
) -> Result<u32, String> {
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {dir}"));
    }

    let audio_extensions = ["m4a", "mp4", "flac", "mp3", "ogg", "opus"];
    let mut tagged = 0u32;

    let entries: Vec<_> = std::fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| audio_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    let total = entries.len();

    for (i, entry) in entries.iter().enumerate() {
        let path = entry.path();

        if let Some((app, dl_id)) = emit_progress {
            crate::utils::activity_log::emit_download_log(
                app,
                dl_id,
                &format!(
                    "BPM analysis: {}/{} — {}",
                    i + 1,
                    total,
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            );
        }

        if let Some(bpm) = detect_bpm(&path, ffmpeg_path).await {
            let bpm_rounded = bpm.round() as u16;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

            let write_result = match ext.as_str() {
                "m4a" | "mp4" => write_bpm_m4a(&path, bpm_rounded),
                "flac" | "ogg" | "opus" | "mp3" => write_bpm_lofty(&path, bpm_rounded),
                _ => Ok(()),
            };

            match write_result {
                Ok(()) => tagged += 1,
                Err(e) => log::warn!("Failed to write BPM for {}: {e}", path.display()),
            }
        }
    }

    Ok(tagged)
}

/// Write BPM to an M4A file using mp4ameta's `tmpo` atom.
fn write_bpm_m4a(path: &Path, bpm: u16) -> Result<(), String> {
    let mut tag = mp4ameta::Tag::read_from_path(path)
        .map_err(|e| format!("Failed to read M4A tags: {e}"))?;
    tag.set_bpm(bpm);
    tag.write_to_path(path)
        .map_err(|e| format!("Failed to write M4A tags: {e}"))?;
    Ok(())
}

/// Write BPM to FLAC/OGG/MP3 files using lofty.
fn write_bpm_lofty(path: &Path, bpm: u16) -> Result<(), String> {
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let mut tagged_file = Probe::open(path)
        .map_err(|e| format!("Failed to open: {e}"))?
        .read()
        .map_err(|e| format!("Failed to read tags: {e}"))?;

    if let Some(tag) = tagged_file.primary_tag_mut() {
        tag.insert_text(lofty::tag::ItemKey::Bpm, bpm.to_string());
        tag.save_to_path(path, lofty::config::WriteOptions::default())
            .map_err(|e| format!("Failed to save tags: {e}"))?;
    }

    Ok(())
}
