// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MediaInfo service — accurate codec detection via `mediainfo --Output=JSON`.
//
// MediaInfo provides definitive Dolby Atmos detection via `Format_AdditionalFeatures: "JOC"`
// which ffprobe's channel-count heuristics cannot reliably achieve. Falls back to ffprobe
// when MediaInfo is not installed.
//
// Detection fields:
//   Atmos:  Format="E-AC-3" + Format_AdditionalFeatures contains "JOC"
//   AC-3:   Format="AC-3"
//   ALAC:   Format="ALAC"
//   AAC-LC: Format="AAC", Format_AdditionalFeatures="LC"
//   HE-AAC: Format="AAC", Format_AdditionalFeatures contains "SBR" or "HE"
//
// Note: Binaural/Downmix cannot be detected by MediaInfo (Apple Music API rendering modes).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::models::gamdl_options::SongCodec;

// ============================================================
// JSON deserialization structs for `mediainfo --Output=JSON`
// ============================================================

#[derive(Debug, Deserialize)]
struct MediaInfoOutput {
    media: Option<MediaInfoMedia>,
}

#[derive(Debug, Deserialize)]
struct MediaInfoMedia {
    track: Vec<MediaInfoTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)] // Full JSON schema — not all fields used yet
struct MediaInfoTrack {
    #[serde(rename = "@type")]
    track_type: String,
    format: Option<String>,
    #[serde(rename = "Format_AdditionalFeatures")]
    format_additional_features: Option<String>,
    #[serde(rename = "Format_Commercial_IfAny")]
    format_commercial: Option<String>,
    channels: Option<String>,
    #[serde(rename = "Compression_Mode")]
    compression_mode: Option<String>,
    #[serde(rename = "SamplingRate")]
    sampling_rate: Option<String>,
    #[serde(rename = "BitDepth")]
    bit_depth: Option<String>,
}

/// Result of MediaInfo codec detection for a single audio file.
#[derive(Debug, Clone)]
pub struct MediaInfoResult {
    /// The detected codec.
    pub codec: SongCodec,
    /// The raw format string from MediaInfo (e.g., "ALAC", "AAC", "E-AC-3").
    pub format: String,
    /// Additional features (e.g., "LC", "JOC").
    pub additional_features: Option<String>,
    /// Channel count as reported by MediaInfo.
    pub channels: Option<u32>,
}

/// Detect the audio codec of a file using MediaInfo CLI.
///
/// Runs `mediainfo --Output=JSON <file>` and parses the output to determine
/// the actual codec. Returns `None` if MediaInfo is not available or the
/// file cannot be analysed.
///
/// # Arguments
/// * `mediainfo_path` - Path to the `mediainfo` binary
/// * `file_path` - Path to the audio file to analyse
pub async fn detect_codec(
    mediainfo_path: &Path,
    file_path: &Path,
) -> Option<MediaInfoResult> {
    let output = tokio::process::Command::new(mediainfo_path)
        .arg("--Output=JSON")
        .arg(file_path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        log::debug!(
            "MediaInfo failed for {}: exit code {}",
            file_path.display(),
            output.status
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_mediainfo_json(&stdout)
}

/// Parse MediaInfo JSON output and extract the codec information.
///
/// Looks for the first "Audio" track in the output and classifies the codec
/// based on the Format and Format_AdditionalFeatures fields.
fn parse_mediainfo_json(json: &str) -> Option<MediaInfoResult> {
    let parsed: MediaInfoOutput = serde_json::from_str(json).ok()?;
    let media = parsed.media?;

    // Find the first audio track
    let audio_track = media
        .track
        .iter()
        .find(|t| t.track_type == "Audio")?;

    let format = audio_track.format.as_deref().unwrap_or("");
    let additional = audio_track.format_additional_features.as_deref().unwrap_or("");
    let channels: Option<u32> = audio_track
        .channels
        .as_deref()
        .and_then(|c| c.parse().ok());

    // Classify the codec based on MediaInfo fields
    let codec = match format {
        "ALAC" => SongCodec::Alac,
        "E-AC-3" => {
            // E-AC-3 with JOC = Dolby Atmos; plain E-AC-3 = Dolby Digital Plus
            if additional.contains("JOC") {
                SongCodec::Atmos
            } else {
                SongCodec::Ac3
            }
        }
        "AC-3" => SongCodec::Ac3,
        "AAC" => {
            // Distinguish AAC variants by additional features and channel count
            if additional.contains("SBR") || additional.contains("HE") {
                // HE-AAC (SBR = Spectral Band Replication)
                SongCodec::AacHe
            } else if channels == Some(2) {
                // Standard stereo AAC
                // Note: Binaural/Downmix look identical at codec level
                SongCodec::Aac
            } else {
                SongCodec::Aac
            }
        }
        _ => {
            log::debug!("MediaInfo: unknown format '{format}' with features '{additional}'");
            return None;
        }
    };

    Some(MediaInfoResult {
        codec,
        format: format.to_string(),
        additional_features: if additional.is_empty() {
            None
        } else {
            Some(additional.to_string())
        },
        channels,
    })
}

/// Get the path to the MediaInfo binary if it's installed.
///
/// Priority: (1) user-configured custom path from settings, (2) managed
/// tools directory, (3) system PATH, (4) common platform install locations.
pub fn get_mediainfo_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    // 1. Check user-configured custom path from settings
    if let Ok(settings) = super::config_service::load_settings(app) {
        if let Some(ref custom) = settings.mediainfo_path {
            if !custom.is_empty() {
                let p = PathBuf::from(custom);
                if p.exists() {
                    return Some(p);
                }
                log::warn!("Custom MediaInfo path does not exist: {custom}");
            }
        }
    }

    // 2. Check managed tools directory
    let managed = super::dependency_manager::get_tool_binary_path(app, "mediainfo");
    if managed.exists() {
        return Some(managed);
    }
    // Check system PATH using the which/where command
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    if let Ok(output) = std::process::Command::new(which_cmd)
        .arg("mediainfo")
        .output()
    {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(&path_str);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // Check common platform-specific installation locations
    let extra_paths: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/opt/homebrew/bin/mediainfo"),
            PathBuf::from("/usr/local/bin/mediainfo"),
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            PathBuf::from("/usr/bin/mediainfo"),
            PathBuf::from("/usr/local/bin/mediainfo"),
        ]
    } else {
        vec![]
    };
    extra_paths.into_iter().find(|p| p.exists())
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alac() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"ALAC","Channels":"2","Compression_Mode":"Lossless","SamplingRate":"44100","BitDepth":"16"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::Alac));
        assert_eq!(result.format, "ALAC");
        assert_eq!(result.channels, Some(2));
    }

    #[test]
    fn parse_atmos() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"E-AC-3","Format_AdditionalFeatures":"JOC","Channels":"16"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::Atmos));
        assert_eq!(result.additional_features.as_deref(), Some("JOC"));
    }

    #[test]
    fn parse_ac3() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"AC-3","Channels":"6"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::Ac3));
    }

    #[test]
    fn parse_aac_lc() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"AAC","Format_AdditionalFeatures":"LC","Channels":"2","SamplingRate":"44100"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::Aac));
    }

    #[test]
    fn parse_he_aac() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"AAC","Format_AdditionalFeatures":"LC SBR","Channels":"2"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::AacHe));
    }

    #[test]
    fn parse_eac3_without_joc_is_ac3() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"E-AC-3","Channels":"6"}]}}"#;
        let result = parse_mediainfo_json(json).unwrap();
        assert!(matches!(result.codec, SongCodec::Ac3));
    }

    #[test]
    fn parse_unknown_format_returns_none() {
        let json = r#"{"media":{"track":[{"@type":"General"},{"@type":"Audio","Format":"FLAC","Channels":"2"}]}}"#;
        assert!(parse_mediainfo_json(json).is_none());
    }

    #[test]
    fn parse_no_audio_track_returns_none() {
        let json = r#"{"media":{"track":[{"@type":"General"}]}}"#;
        assert!(parse_mediainfo_json(json).is_none());
    }
}
