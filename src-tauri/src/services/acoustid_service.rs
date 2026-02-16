// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// acoustid_service.rs -- AcousticID fingerprinting and lookup service
// ===================================================================
//
// Generates Chromaprint audio fingerprints using fpcalc and looks up
// AcousticID identifiers via the acoustid.org API. This enables music
// identification compatible with MusicBrainz Picard and other tools
// that use the AcousticID ecosystem.
//
// ## How it works
//
// 1. For each M4A file, runs `fpcalc -json file.m4a` to generate a
//    Chromaprint audio fingerprint and measure the file's duration.
// 2. Sends the fingerprint + duration to the AcousticID lookup API
//    (`https://api.acoustid.org/v2/lookup`) to find a matching AcousticID.
// 3. Writes two freeform atoms to the M4A file:
//    - `----:com.apple.iTunes:Acoustid Id` — the AcousticID UUID
//    - `----:com.apple.iTunes:Acoustid Fingerprint` — raw fingerprint
//
// ## Prerequisites
//
// - **fpcalc binary**: Must be installed via the dependency manager.
//   Distributed from Chromaprint releases (acoustid/chromaprint on GitHub).
// - **API key**: An application API key registered at acoustid.org.
//   Currently uses an app-embedded key (standard practice, same as
//   MusicBrainz Picard).
//
// ## Rate limiting
//
// The AcousticID API allows ~3 requests/second for free API keys.
// A 334ms delay is enforced between lookup requests to stay within limits.
//
// ## Opt-in
//
// AcousticID processing is opt-in (`acoustid_enabled` in settings) because
// it's CPU-intensive (fpcalc decodes and fingerprints each file) and
// requires network requests per track.
//
// @see https://acoustid.org/webservice
// @see https://github.com/acoustid/chromaprint

use std::path::{Path, PathBuf};
use std::time::Duration;

use mp4ameta::{Data, FreeformIdent, Tag};
use tauri::AppHandle;
use tokio::process::Command;
use tokio::time::sleep;

use crate::services::dependency_manager;

/// Apple iTunes freeform atom namespace (matches MusicBrainz Picard's convention).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// AcousticID API endpoint for fingerprint lookups.
const ACOUSTID_API_URL: &str = "https://api.acoustid.org/v2/lookup";

/// Delay between AcousticID API requests (~3 req/sec rate limit).
const API_RATE_LIMIT_DELAY: Duration = Duration::from_millis(334);

/// MeedyaDL application API key for AcousticID.
/// Registered at https://acoustid.org/new-application
/// This is a public application key (not a secret) — same practice as
/// MusicBrainz Picard and other open-source music taggers.
const ACOUSTID_API_KEY: &str = "PLACEHOLDER_REGISTER_AT_ACOUSTID_ORG";

// ============================================================
// Public API
// ============================================================

/// Process all M4A files in the output directory for AcousticID fingerprinting.
///
/// For each file: generates a Chromaprint fingerprint, looks up the AcousticID,
/// and writes both the fingerprint and AcousticID UUID as metadata tags.
///
/// # Arguments
/// * `app` - Tauri AppHandle for tool path resolution
/// * `output_path` - Download output path (file or album directory)
///
/// # Returns
/// * `Ok(count)` - Number of files successfully fingerprinted and tagged
/// * `Err(message)` - fpcalc not installed or output path invalid
pub async fn process_acoustid_for_directory(
    app: &AppHandle,
    output_path: &str,
) -> Result<usize, String> {
    let fpcalc_path = get_fpcalc_path(app)?;

    // Collect all M4A files
    let m4a_files = collect_m4a_files(output_path);
    if m4a_files.is_empty() {
        return Ok(0);
    }

    let mut tagged_count = 0;

    for (i, file_path) in m4a_files.iter().enumerate() {
        // Rate limit: wait before each API call (skip first iteration)
        if i > 0 {
            sleep(API_RATE_LIMIT_DELAY).await;
        }

        match process_single_file(&fpcalc_path, file_path).await {
            Ok(true) => tagged_count += 1,
            Ok(false) => {
                log::debug!("No AcousticID match for {}", file_path.display());
            }
            Err(e) => {
                log::debug!("AcousticID failed for {}: {}", file_path.display(), e);
            }
        }
    }

    if tagged_count > 0 {
        log::info!(
            "Tagged {} of {} file(s) with AcousticID metadata",
            tagged_count,
            m4a_files.len()
        );
    }

    Ok(tagged_count)
}

// ============================================================
// Internal: Per-File Processing
// ============================================================

/// Generate fingerprint, look up AcousticID, and write tags for a single file.
///
/// Returns `Ok(true)` if tags were written, `Ok(false)` if no match found.
async fn process_single_file(
    fpcalc_path: &Path,
    file_path: &Path,
) -> Result<bool, String> {
    // Step 1: Generate Chromaprint fingerprint
    let (fingerprint, duration) = generate_fingerprint(fpcalc_path, file_path).await?;

    // Step 2: Look up AcousticID
    let acoustid = match lookup_acoustid(&fingerprint, duration).await? {
        Some(id) => id,
        None => return Ok(false), // No match found
    };

    // Step 3: Write tags
    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read M4A: {}", e))?;

    // Acoustid Id — the UUID from acoustid.org
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Id"),
        Data::Utf8(acoustid),
    );

    // Acoustid Fingerprint — raw Chromaprint fingerprint string
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Fingerprint"),
        Data::Utf8(fingerprint),
    );

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write M4A: {}", e))?;

    log::debug!("AcousticID tagged: {}", file_path.display());
    Ok(true)
}

// ============================================================
// Internal: Fingerprint Generation (via fpcalc)
// ============================================================

/// Resolve the managed fpcalc binary path.
fn get_fpcalc_path(app: &AppHandle) -> Result<PathBuf, String> {
    let fpcalc_bin = dependency_manager::get_tool_binary_path(app, "fpcalc");
    if !fpcalc_bin.exists() {
        return Err("fpcalc not installed — required for AcousticID fingerprinting".to_string());
    }
    Ok(fpcalc_bin)
}

/// Generate a Chromaprint fingerprint for an audio file using fpcalc.
///
/// Runs `fpcalc -json file.m4a` and parses the JSON output to extract
/// the fingerprint string and duration in seconds.
///
/// # Returns
/// * `Ok((fingerprint, duration))` - Fingerprint string and duration in seconds
/// * `Err(message)` - fpcalc execution or parsing failed
async fn generate_fingerprint(
    fpcalc_path: &Path,
    file_path: &Path,
) -> Result<(String, u32), String> {
    let output = Command::new(fpcalc_path)
        .arg("-json")
        .arg(file_path)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn fpcalc: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("fpcalc failed: {}", stderr.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse fpcalc output: {}", e))?;

    let fingerprint = json
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .ok_or("fpcalc output missing 'fingerprint' field")?
        .to_string();

    let duration = json
        .get("duration")
        .and_then(|v| v.as_f64())
        .ok_or("fpcalc output missing 'duration' field")?
        as u32;

    Ok((fingerprint, duration))
}

// ============================================================
// Internal: AcousticID API Lookup
// ============================================================

/// Look up an AcousticID by fingerprint and duration.
///
/// Sends a POST request to the AcousticID web service with the Chromaprint
/// fingerprint. Returns the AcousticID UUID if a match is found with
/// sufficient confidence (score >= 0.5).
///
/// # Returns
/// * `Ok(Some(uuid))` - AcousticID found
/// * `Ok(None)` - No match with sufficient confidence
/// * `Err(message)` - API request or parsing failed
async fn lookup_acoustid(
    fingerprint: &str,
    duration: u32,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();

    let response = client
        .post(ACOUSTID_API_URL)
        .form(&[
            ("client", ACOUSTID_API_KEY),
            ("fingerprint", fingerprint),
            ("duration", &duration.to_string()),
            ("meta", "recordings"),
        ])
        .send()
        .await
        .map_err(|e| format!("AcousticID API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!("AcousticID API returned HTTP {}", status));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse AcousticID response: {}", e))?;

    // Check API status
    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if status != "ok" {
        let error_msg = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown API error");
        return Err(format!("AcousticID API error: {}", error_msg));
    }

    // Find the best matching result with score >= 0.5
    let results = json
        .get("results")
        .and_then(|v| v.as_array());

    let best_match = results
        .and_then(|arr| {
            arr.iter().find(|r| {
                r.get("score")
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) >= 0.5
            })
        });

    match best_match {
        Some(result) => {
            let acoustid = result
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(acoustid)
        }
        None => Ok(None),
    }
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
    // ----------------------------------------------------------
    // Fingerprint output parsing tests
    // ----------------------------------------------------------

    #[test]
    fn parse_fpcalc_json_output() {
        let json_str = r#"{"duration": 242.573, "fingerprint": "AQADtNIyRYgS..."}"#;
        let json: serde_json::Value = serde_json::from_str(json_str).unwrap();

        let fingerprint = json.get("fingerprint").and_then(|v| v.as_str()).unwrap();
        assert_eq!(fingerprint, "AQADtNIyRYgS...");

        let duration = json.get("duration").and_then(|v| v.as_f64()).unwrap() as u32;
        assert_eq!(duration, 242);
    }

    // ----------------------------------------------------------
    // AcousticID response parsing tests
    // ----------------------------------------------------------

    #[test]
    fn parse_acoustid_response_with_match() {
        let json: serde_json::Value = serde_json::from_str(r#"{
            "status": "ok",
            "results": [
                {
                    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "score": 0.95,
                    "recordings": []
                }
            ]
        }"#).unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        let best = results.iter().find(|r| {
            r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) >= 0.5
        });
        assert!(best.is_some());
        let id = best.unwrap().get("id").and_then(|v| v.as_str()).unwrap();
        assert_eq!(id, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn parse_acoustid_response_low_score_returns_none() {
        let json: serde_json::Value = serde_json::from_str(r#"{
            "status": "ok",
            "results": [
                {
                    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "score": 0.2,
                    "recordings": []
                }
            ]
        }"#).unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        let best = results.iter().find(|r| {
            r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) >= 0.5
        });
        assert!(best.is_none());
    }

    #[test]
    fn parse_acoustid_response_empty_results() {
        let json: serde_json::Value = serde_json::from_str(r#"{
            "status": "ok",
            "results": []
        }"#).unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_acoustid_error_response() {
        let json: serde_json::Value = serde_json::from_str(r#"{
            "status": "error",
            "error": {
                "code": 2,
                "message": "invalid fingerprint"
            }
        }"#).unwrap();

        let status = json.get("status").and_then(|v| v.as_str()).unwrap();
        assert_eq!(status, "error");

        let error_msg = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(error_msg, "invalid fingerprint");
    }
}
