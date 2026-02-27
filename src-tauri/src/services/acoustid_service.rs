// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// acoustid_service.rs -- AcousticID fingerprinting and lookup service
// ===================================================================
//
// Generates Chromaprint audio fingerprints using the embedded
// rusty-chromaprint library (pure Rust) and looks up AcousticID
// identifiers via the acoustid.org API. This enables music
// identification compatible with MusicBrainz Picard and other tools
// that use the AcousticID ecosystem.
//
// ## How it works
//
// 1. For each M4A file, decodes the audio to raw PCM using Symphonia
//    (pure Rust audio decoder), then generates a Chromaprint fingerprint
//    using rusty-chromaprint's Fingerprinter.
// 2. Compresses the fingerprint using FingerprintCompressor and encodes
//    it in URL-safe base64 format for the AcousticID API.
// 3. Sends the fingerprint + duration to the AcousticID lookup API
//    (`https://api.acoustid.org/v2/lookup`) to find a matching AcousticID.
// 4. Writes two freeform atoms to the M4A file:
//    - `----:com.apple.iTunes:Acoustid Id` — the AcousticID UUID
//    - `----:com.apple.iTunes:Acoustid Fingerprint` — encoded fingerprint
//
// ## No External Dependencies
//
// All fingerprinting is done in pure Rust via rusty-chromaprint and
// Symphonia. No external fpcalc binary is required, which also enables
// AcousticID support on ARM Linux (where no fpcalc binaries exist).
//
// ## Rate limiting
//
// The AcousticID API allows ~3 requests/second for free API keys.
// A 334ms delay is enforced between lookup requests to stay within limits.
//
// ## Opt-in
//
// AcousticID processing is opt-in (`acoustid_enabled` in settings) because
// it's CPU-intensive (decoding + fingerprinting each file) and requires
// network requests per track.
//
// @see https://acoustid.org/webservice
// @see https://docs.rs/rusty-chromaprint/
// @see https://docs.rs/symphonia/

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use mp4ameta::{Data, FreeformIdent, Tag};
use rusty_chromaprint::{Configuration, FingerprintCompressor, Fingerprinter};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tokio::time::sleep;

/// Apple iTunes freeform atom namespace (matches `MusicBrainz` Picard's convention).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// `AcousticID` API endpoint for fingerprint lookups.
const ACOUSTID_API_URL: &str = "https://api.acoustid.org/v2/lookup";

/// Delay between `AcousticID` API requests (~3 req/sec rate limit).
const API_RATE_LIMIT_DELAY: Duration = Duration::from_millis(334);

// The AcousticID API key is provided by the user via Settings > Metadata.
// Users register a free application key at https://acoustid.org/new-application.

// ============================================================
// Public API
// ============================================================

/// Process all M4A files in the output directory for `AcousticID` fingerprinting.
///
/// For each file: generates a Chromaprint fingerprint using the embedded
/// rusty-chromaprint library, looks up the `AcousticID` via the web service,
/// and writes both the fingerprint and `AcousticID` UUID as metadata tags.
///
/// # Arguments
/// * `output_path` - Download output path (file or album directory)
/// * `api_key` - AcousticID application API key (registered at acoustid.org)
///
/// # Errors
///
/// Returns `Err(String)` if the output path is invalid or file processing fails.
///
/// # Returns
/// * `Ok(count)` - Number of files successfully fingerprinted and tagged
/// * `Err(message)` - Output path invalid or no M4A files found
pub async fn process_acoustid_for_directory(
    output_path: &str,
    api_key: &str,
) -> Result<usize, String> {
    if api_key.is_empty() || api_key.contains("PLACEHOLDER") {
        return Err("AcousticID API key not configured".to_string());
    }
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

        match process_single_file(file_path, api_key).await {
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

/// Generate fingerprint, look up `AcousticID`, and write tags for a single file.
///
/// Returns `Ok(true)` if tags were written, `Ok(false)` if no match found.
async fn process_single_file(
    file_path: &Path,
    api_key: &str,
) -> Result<bool, String> {
    // Step 1: Generate Chromaprint fingerprint (embedded, no external binary).
    // This is CPU-intensive (decodes the entire audio file), so we run it
    // on a blocking thread to avoid starving the async runtime.
    let fp_path = file_path.to_path_buf();
    let (fingerprint, duration) = tokio::task::spawn_blocking(move || {
        generate_fingerprint(&fp_path)
    })
    .await
    .map_err(|e| format!("Fingerprint task panicked: {e}"))??;

    // Step 2: Look up AcousticID
    let Some(acoustid) = lookup_acoustid(&fingerprint, duration, api_key).await? else {
        return Ok(false); // No match found
    };

    // Step 3: Write tags
    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read M4A: {e}"))?;

    // Acoustid Id — the UUID from acoustid.org
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Id"),
        Data::Utf8(acoustid),
    );

    // Acoustid Fingerprint — encoded Chromaprint fingerprint string
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Fingerprint"),
        Data::Utf8(fingerprint),
    );

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write M4A: {e}"))?;

    log::debug!("AcousticID tagged: {}", file_path.display());
    Ok(true)
}

// ============================================================
// Internal: Fingerprint Generation (embedded Chromaprint)
// ============================================================

/// Generate a Chromaprint fingerprint for an audio file.
///
/// Uses Symphonia to decode the M4A file to raw interleaved i16 PCM
/// samples, then feeds them to rusty-chromaprint's Fingerprinter.
/// The resulting fingerprint is compressed and encoded in URL-safe base64
/// format compatible with the `AcousticID` API.
///
/// # Returns
/// * `Ok((fingerprint, duration))` - Encoded fingerprint and duration in seconds
/// * `Err(message)` - Decoding or fingerprinting failed
#[allow(clippy::similar_names)] // `decoded` and `decoder` are standard audio terminology
fn generate_fingerprint(file_path: &Path) -> Result<(String, u32), String> {
    // Open the audio file and wrap in a MediaSourceStream
    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open audio file: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    // Provide a file extension hint for format detection
    let mut hint = Hint::new();
    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    // Probe the format (auto-detect M4A/MP4 container)
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Failed to probe audio format: {e}"))?;

    let mut format_reader = probed.format;

    // Find the first audio track and get its codec parameters
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio tracks found in file")?;

    let codec_params = &track.codec_params;
    let sample_rate = codec_params
        .sample_rate
        .ok_or("Audio track has no sample rate")?;
    let channels = codec_params
        .channels
        .map(|ch| u32::try_from(ch.count()).unwrap_or(0))
        .ok_or("Audio track has no channel info")?;
    let track_id = track.id;

    // Create a decoder for the audio track
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create audio decoder: {e}"))?;

    // Initialize the Chromaprint fingerprinter with the standard preset
    // (preset_test2 matches fpcalc's default algorithm)
    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer
        .start(sample_rate, channels)
        .map_err(|e| format!("Failed to start fingerprinter: {e:?}"))?;

    // Decode all audio packets and feed interleaved i16 samples to the fingerprinter
    let mut total_samples: u64 = 0;
    let mut sample_buf: Option<SampleBuffer<i16>> = None;

    loop {
        let packet = match format_reader.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break; // End of stream
            }
            Err(e) => return Err(format!("Error reading audio packet: {e}")),
        };

        // Skip packets from other tracks (unlikely for M4A but defensive)
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                // Initialize or resize the sample buffer as needed.
                // SampleBuffer::new() takes u64 duration; capacity() returns usize.
                let num_frames = decoded.frames();
                if sample_buf.is_none()
                    || sample_buf.as_ref().is_some_and(|b| b.capacity() < num_frames)
                {
                    let spec = *decoded.spec();
                    sample_buf = Some(SampleBuffer::new(num_frames as u64, spec));
                }

                if let Some(ref mut buf) = sample_buf {
                    buf.copy_interleaved_ref(decoded);
                    let samples = buf.samples();
                    total_samples += samples.len() as u64;
                    printer.consume(samples);
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                // Skip corrupted packets
            }
            Err(e) => return Err(format!("Audio decode error: {e}")),
        }
    }

    // Finalize the fingerprint
    printer.finish();
    let raw_fingerprint = printer.fingerprint();

    if raw_fingerprint.is_empty() {
        return Err("Generated empty fingerprint (file too short?)".to_string());
    }

    // Compress the fingerprint using Chromaprint's internal compression format
    let compressor = FingerprintCompressor::from(&config);
    let compressed = compressor.compress(raw_fingerprint);

    // Encode in URL-safe base64 (no padding) — matches Chromaprint's convention
    let encoded = URL_SAFE_NO_PAD.encode(&compressed);

    // Calculate duration from total decoded samples.
    // u64::from() used for lossless widening; try_from() guards against truncation.
    let duration_seconds = u32::try_from(
        total_samples / u64::from(channels) / u64::from(sample_rate)
    )
    .unwrap_or(u32::MAX);

    Ok((encoded, duration_seconds))
}

// ============================================================
// Internal: AcousticID API Lookup
// ============================================================

/// Look up an `AcousticID` by fingerprint and duration.
///
/// Sends a POST request to the `AcousticID` web service with the Chromaprint
/// fingerprint. Returns the `AcousticID` UUID if a match is found with
/// sufficient confidence (score >= 0.5).
///
/// # Returns
/// * `Ok(Some(uuid))` - `AcousticID` found
/// * `Ok(None)` - No match with sufficient confidence
/// * `Err(message)` - API request or parsing failed
async fn lookup_acoustid(
    fingerprint: &str,
    duration: u32,
    api_key: &str,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();

    let response = client
        .post(ACOUSTID_API_URL)
        .form(&[
            ("client", api_key),
            ("fingerprint", fingerprint),
            ("duration", &duration.to_string()),
            ("meta", "recordings"),
        ])
        .send()
        .await
        .map_err(|e| format!("AcousticID API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!("AcousticID API returned HTTP {status}"));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse AcousticID response: {e}"))?;

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
        return Err(format!("AcousticID API error: {error_msg}"));
    }

    // Find the best matching result with score >= 0.5
    let results = json
        .get("results")
        .and_then(|v| v.as_array());

    let best_match = results
        .and_then(|arr| {
            arr.iter().find(|r| {
                r.get("score")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) >= 0.5
            })
        });

    best_match.map_or(Ok(None), |result| {
        let acoustid = result
            .get("id")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);
        Ok(acoustid)
    })
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
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
        .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"))
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use rusty_chromaprint::{Configuration, FingerprintCompressor};

    // ----------------------------------------------------------
    // Fingerprint compression + encoding pipeline tests
    // ----------------------------------------------------------

    #[test]
    fn compressed_fingerprint_produces_valid_base64() {
        let config = Configuration::preset_test2();
        let compressor = FingerprintCompressor::from(&config);

        // A minimal fake fingerprint to test the compression + encoding pipeline
        let fake_fp: Vec<u32> = vec![0x12345678, 0xABCDEF01, 0x98765432];
        let compressed = compressor.compress(&fake_fp);
        let encoded = URL_SAFE_NO_PAD.encode(&compressed);

        // Verify it's valid base64 by decoding it back
        assert!(URL_SAFE_NO_PAD.decode(&encoded).is_ok());
        assert!(!encoded.is_empty());
        // URL-safe base64 should not contain + or / (uses - and _ instead)
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
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
