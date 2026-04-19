// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Music video subtitle / caption extraction service (#483).
// =========================================================
//
// After GAMDL downloads a music video, this service probes the output
// file for subtitle / closed-caption streams and extracts them into
// sidecar files (`.vtt` / `.srt`) next to the video. Apple Music HLS
// playlists often expose CC tracks (`mov_text`, `tx3g`, `webvtt`,
// `eia_608`) that GAMDL currently remuxes into the output container
// without extracting — media players that don't auto-detect embedded
// subtitle streams benefit from the sidecar copy.
//
// ## Flow
// 1. Run ffprobe to enumerate subtitle streams in the downloaded video.
// 2. For each subtitle stream, invoke ffmpeg to copy/convert it into a
//    sidecar next to the video. Preserves the BCP-47 language tag in the
//    filename when present (e.g. `Title.en.vtt`, `Title.fr.srt`).
// 3. Best-effort: failures are logged but never affect the success of
//    the music video download itself.
//
// ## Dependencies
// - `ffprobe` (shipped with FFmpeg in the managed tools dir)
// - `ffmpeg`  (shipped likewise)

use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Metadata about a single subtitle / caption stream inside a video file.
#[derive(Debug)]
struct SubtitleStream {
    /// Stream index within the container (0-based, as ffmpeg sees it).
    index: u32,
    /// Codec name (e.g. `mov_text`, `webvtt`, `tx3g`, `eia_608`).
    codec: String,
    /// BCP-47 language tag (e.g. `en`, `fr-CA`, `und` for unknown).
    language: String,
}

/// Probe the given video file and extract every subtitle/caption stream
/// to a sidecar file alongside it.
///
/// Returns `Ok(count)` where `count` is the number of sidecars produced
/// (may be zero if the video has no subtitle streams). Errors are reported
/// only when probing fundamentally fails — per-stream failures are logged
/// and skipped.
pub async fn extract_subtitles_to_sidecars(
    ffprobe_path: &Path,
    ffmpeg_path: &Path,
    video_path: &Path,
) -> Result<usize, String> {
    if !video_path.is_file() {
        return Err(format!("Video file not found: {}", video_path.display()));
    }

    let streams = probe_subtitle_streams(ffprobe_path, video_path).await?;
    if streams.is_empty() {
        log::debug!(
            "No subtitle streams in {}",
            video_path.display()
        );
        return Ok(0);
    }

    let mut extracted = 0;
    for stream in streams {
        match extract_single_stream(ffmpeg_path, video_path, &stream).await {
            Ok(sidecar_path) => {
                log::info!(
                    "Extracted subtitle stream #{} ({}, {}) → {}",
                    stream.index,
                    stream.codec,
                    stream.language,
                    sidecar_path.display()
                );
                extracted += 1;
            }
            Err(e) => {
                log::warn!(
                    "Failed to extract subtitle stream #{} from {}: {e}",
                    stream.index,
                    video_path.display()
                );
            }
        }
    }

    Ok(extracted)
}

/// Run ffprobe and parse every subtitle stream out of the video file.
async fn probe_subtitle_streams(
    ffprobe_path: &Path,
    video_path: &Path,
) -> Result<Vec<SubtitleStream>, String> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "s",
        ])
        .arg(video_path)
        .output()
        .await
        .map_err(|e| format!("ffprobe spawn failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe exited with {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("ffprobe returned invalid JSON: {e}"))?;

    let Some(streams) = json.get("streams").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };

    let mut out = Vec::with_capacity(streams.len());
    for stream in streams {
        let Some(index) = stream.get("index").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let codec = stream
            .get("codec_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let language = stream
            .get("tags")
            .and_then(|t| t.get("language"))
            .and_then(|l| l.as_str())
            .unwrap_or("und")
            .to_string();
        out.push(SubtitleStream {
            index: u32::try_from(index).unwrap_or(0),
            codec,
            language,
        });
    }

    Ok(out)
}

/// Extract a single subtitle stream into a sidecar file.
///
/// Chooses the sidecar extension based on the source codec:
/// - `webvtt`          → `.vtt` (copy)
/// - `mov_text`, `tx3g`, `eia_608`, `subrip`, other text subs → `.srt`
/// - anything else         → `.srt` (fallback, ffmpeg will convert)
async fn extract_single_stream(
    ffmpeg_path: &Path,
    video_path: &Path,
    stream: &SubtitleStream,
) -> Result<PathBuf, String> {
    let parent = video_path
        .parent()
        .ok_or_else(|| "Video has no parent directory".to_string())?;
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Video has no filename stem".to_string())?;

    let (ext, codec_args): (&str, &[&str]) = if stream.codec.eq_ignore_ascii_case("webvtt") {
        // Copy WebVTT as-is — no transcode.
        ("vtt", &["-c:s", "copy"])
    } else {
        // Everything else lands as SRT (ffmpeg handles the conversion).
        ("srt", &["-c:s", "srt"])
    };

    // Append the language tag when it's something meaningful so multiple
    // subtitle tracks don't overwrite each other's sidecars.
    let lang_suffix = if stream.language == "und" || stream.language.is_empty() {
        String::new()
    } else {
        format!(".{}", stream.language)
    };
    let sidecar_name = format!("{stem}{lang_suffix}.{ext}");
    let sidecar_path = parent.join(&sidecar_name);

    // Skip if a sidecar with this exact name already exists (idempotent).
    if sidecar_path.exists() {
        return Ok(sidecar_path);
    }

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg(format!("0:{}", stream.index));
    cmd.args(codec_args);
    cmd.arg(&sidecar_path);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg exited with {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    Ok(sidecar_path)
}

/// Copy existing audio lyrics sidecars (TTML, LRC, SRT, VTT, ASS) alongside
/// a freshly downloaded music video, when the video's matching song was
/// already downloaded as part of the primary album.
///
/// The pairing strategy is deliberately permissive: we look for any lyrics
/// file under `album_dir` whose stem is a substring match of the video's
/// stem (or vice versa) and copy it next to the video with the video's
/// stem. This catches the common case where GAMDL names the song and the
/// music video similarly (typically identical title, different extension).
///
/// Silent no-op when no matches are found.
pub fn pair_song_lyrics_with_music_video(album_dir: &Path, video_path: &Path) -> usize {
    let Some(video_stem) = video_path.file_stem().and_then(|s| s.to_str()) else {
        return 0;
    };
    let Some(video_parent) = video_path.parent() else {
        return 0;
    };
    let normalised_video_stem = normalise_stem(video_stem);

    let Ok(entries) = std::fs::read_dir(album_dir) else {
        return 0;
    };

    let lyric_exts = ["ttml", "lrc", "srt", "vtt", "ass"];
    let mut copied = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !lyric_exts.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let normalised_lyric_stem = normalise_stem(stem);
        let matches = normalised_video_stem.contains(&normalised_lyric_stem)
            || normalised_lyric_stem.contains(&normalised_video_stem);
        if !matches {
            continue;
        }

        let target = video_parent.join(format!("{video_stem}.{ext}"));
        if target.exists() {
            continue;
        }
        if let Err(e) = std::fs::copy(&path, &target) {
            log::debug!(
                "Failed to pair {} → {}: {e}",
                path.display(),
                target.display()
            );
        } else {
            log::debug!(
                "Paired song lyrics with music video: {} → {}",
                path.display(),
                target.display()
            );
            copied += 1;
        }
    }
    copied
}

/// Lowercase + strip typical decoration suffixes (codec / advisory) so
/// song and music video stems compare cleanly.
fn normalise_stem(stem: &str) -> String {
    let mut s = stem.to_ascii_lowercase();
    // Strip any `[tag]` bracket suffixes (codec / advisory) that only
    // appear on one side of the pairing.
    while let Some(open) = s.rfind('[') {
        if let Some(close) = s[open..].find(']') {
            let end = open + close + 1;
            if end == s.len() || s[end..].trim().is_empty() {
                s.truncate(open);
                s = s.trim_end().to_string();
                continue;
            }
        }
        break;
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_stem_strips_codec_bracket() {
        assert_eq!(normalise_stem("01 Title [Lossless]"), "01 title");
        assert_eq!(
            normalise_stem("Song [Explicit] [Dolby Atmos]"),
            "song"
        );
    }

    #[test]
    fn normalise_stem_leaves_plain_title_alone() {
        assert_eq!(normalise_stem("Song Title"), "song title");
    }
}
