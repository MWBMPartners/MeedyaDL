// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Enrichment gap detection for downloaded albums (#759 Phase 1).
//!
//! Inspects an album directory + its `manifest.meedyadl` to determine
//! which enrichment stages are missing. The deliverable for Phase 1
//! is **detection only** — the Library Scan UI surfaces the result
//! so users can see which albums have incomplete enrichments.
//!
//! The actual gap-FILL runner that re-executes missing stages is
//! deferred to Phase 2 (separate follow-up PR) because each stage's
//! service needs to be refactored to be re-entrant in isolation
//! (current pipeline assumes a fresh download). Phase 1 lays the
//! manifest-schema + detector foundation cleanly; Phase 2 wires the
//! runner on top.
//!
//! ## Detection signals
//!
//! Hybrid: the manifest's [`EnrichmentRecord`] is authoritative when
//! present, otherwise fall back to **file-based heuristics** so
//! historical downloads (with no `enrichment` block in the manifest)
//! still surface useful gap data.
//!
//! | Stage | File-based signal |
//! |---|---|
//! | Enhanced LRC | Every audio file has a sibling `.lrc` |
//! | Rich SRT | Every audio file has a sibling `.srt` |
//! | WebVTT | Every audio file has a sibling `.vtt` |
//! | ASS subtitles | Every audio file has a sibling `.ass` |
//! | Animated artwork (square) | `FrontCover.mp4` exists in the album dir |
//! | Animated artwork (portrait) | `FrontCoverPortrait.mp4` exists |
//! | Album spotlight video | `AlbumSpotlightCover.mp4` exists |
//! | Static cover fallback | `Cover.{jpg,png}` exists |
//!
//! For tag-embedded enrichments (AcoustID, ReplayGain, MusicBrainz,
//! API metadata, BPM) the manifest's record is the only reliable
//! signal — they're inside MP4 atoms and require ffprobe / a tag
//! library to check per file. Phase 1 trusts the manifest record;
//! Phase 2 will add an ffprobe-based heuristic for manifests that
//! pre-date the schema extension.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::models::manifest::ManifestFile;

/// Canonical name for every enrichment stage. Lives here (not in
/// each stage's service) so the registry stays the single source of
/// truth — adding a new stage in a future feature means adding a
/// new variant here and registering its detector below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentStage {
    AppleMusicApiMetadata,
    ItunesLookup,
    SyllableLyrics,
    EnhancedLrc,
    LyricsFallback,
    WebVtt,
    RichSrt,
    SubtitleEmbed,
    AssSubtitles,
    AnimatedArtworkSquare,
    AnimatedArtworkPortrait,
    AlbumSpotlight,
    ArtistSpotlight,
    StaticCoverFallback,
    AcoustId,
    ReplayGain,
    MusicBrainz,
    MusicVideoCompanion,
    BpmAnalysis,
    ContentAdvisorySuffix,
}

impl EnrichmentStage {
    /// Stable string identifier used as the manifest key. Adding new
    /// variants must NOT change existing identifiers — manifests
    /// already on disk reference these strings.
    pub fn manifest_key(self) -> &'static str {
        match self {
            Self::AppleMusicApiMetadata => "apple_music_api",
            Self::ItunesLookup => "itunes_lookup",
            Self::SyllableLyrics => "syllable_lyrics",
            Self::EnhancedLrc => "enhanced_lrc",
            Self::LyricsFallback => "lyrics_fallback",
            Self::WebVtt => "webvtt",
            Self::RichSrt => "rich_srt",
            Self::SubtitleEmbed => "subtitle_embed",
            Self::AssSubtitles => "ass_subtitles",
            Self::AnimatedArtworkSquare => "animated_artwork_square",
            Self::AnimatedArtworkPortrait => "animated_artwork_portrait",
            Self::AlbumSpotlight => "album_spotlight",
            Self::ArtistSpotlight => "artist_spotlight",
            Self::StaticCoverFallback => "static_cover_fallback",
            Self::AcoustId => "acoustid",
            Self::ReplayGain => "replaygain",
            Self::MusicBrainz => "musicbrainz",
            Self::MusicVideoCompanion => "music_video_companion",
            Self::BpmAnalysis => "bpm_analysis",
            Self::ContentAdvisorySuffix => "content_advisory_suffix",
        }
    }

    /// Human-friendly label for the UI.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::AppleMusicApiMetadata => "Apple Music API metadata",
            Self::ItunesLookup => "iTunes Lookup metadata",
            Self::SyllableLyrics => "Word-level syllable lyrics",
            Self::EnhancedLrc => "Enhanced LRC lyrics",
            Self::LyricsFallback => "Lyrics fallback (TTML→LRC)",
            Self::WebVtt => "WebVTT subtitles",
            Self::RichSrt => "Rich SRT subtitles",
            Self::SubtitleEmbed => "Embedded subtitle atoms",
            Self::AssSubtitles => "ASS subtitles",
            Self::AnimatedArtworkSquare => "Animated artwork (square)",
            Self::AnimatedArtworkPortrait => "Animated artwork (portrait)",
            Self::AlbumSpotlight => "Album spotlight video",
            Self::ArtistSpotlight => "Artist spotlight video",
            Self::StaticCoverFallback => "Static cover fallback",
            Self::AcoustId => "AcoustID fingerprint",
            Self::ReplayGain => "ReplayGain loudness",
            Self::MusicBrainz => "MusicBrainz cross-platform URLs",
            Self::MusicVideoCompanion => "Music video companions",
            Self::BpmAnalysis => "BPM analysis",
            Self::ContentAdvisorySuffix => "Content advisory suffixes",
        }
    }

    /// Every stage in canonical iteration order. Adding new stages
    /// here makes them visible to the gap detector automatically.
    pub const ALL: &'static [EnrichmentStage] = &[
        Self::AppleMusicApiMetadata,
        Self::ItunesLookup,
        Self::SyllableLyrics,
        Self::EnhancedLrc,
        Self::LyricsFallback,
        Self::WebVtt,
        Self::RichSrt,
        Self::SubtitleEmbed,
        Self::AssSubtitles,
        Self::AnimatedArtworkSquare,
        Self::AnimatedArtworkPortrait,
        Self::AlbumSpotlight,
        Self::ArtistSpotlight,
        Self::StaticCoverFallback,
        Self::AcoustId,
        Self::ReplayGain,
        Self::MusicBrainz,
        Self::MusicVideoCompanion,
        Self::BpmAnalysis,
        Self::ContentAdvisorySuffix,
    ];
}

/// Whether a stage's file-based detector is reliable (i.e. the
/// detector can give a confident "missing" verdict without trusting
/// the manifest record). Stages backed by sidecar files have
/// reliable file detection; stages backed by tag atoms only do not
/// (Phase 1 — Phase 2 adds ffprobe-based tag detection).
fn has_reliable_file_detector(stage: EnrichmentStage) -> bool {
    matches!(
        stage,
        EnrichmentStage::EnhancedLrc
            | EnrichmentStage::RichSrt
            | EnrichmentStage::WebVtt
            | EnrichmentStage::AssSubtitles
            | EnrichmentStage::AnimatedArtworkSquare
            | EnrichmentStage::AnimatedArtworkPortrait
            | EnrichmentStage::AlbumSpotlight
            | EnrichmentStage::StaticCoverFallback
    )
}

/// One row of the gap-detection report.
#[derive(Debug, Clone, Serialize)]
pub struct StageStatus {
    pub stage: EnrichmentStage,
    /// `"complete" | "missing" | "unknown"`. `"unknown"` means the
    /// stage's file-detector is unreliable AND the manifest doesn't
    /// carry a record — Phase 1 can't tell; the UI should display
    /// these as "not assessed" rather than "missing" to avoid a
    /// false-positive on legacy downloads.
    pub status: &'static str,
    /// Display label for the UI (avoid frontend duplicating the map).
    pub label: &'static str,
    /// Optional: stage's manifest key for cross-reference.
    pub key: &'static str,
}

/// Complete gap-detection report for one album directory.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentGapReport {
    /// Absolute path of the inspected album directory.
    pub album_dir: String,
    /// Count of audio files found (drives the per-track sidecar
    /// detectors — Enhanced LRC etc. need to know how many tracks
    /// should have a `.lrc`).
    pub audio_file_count: usize,
    /// Per-stage status. Order matches [`EnrichmentStage::ALL`] so
    /// the UI can render without sorting.
    pub stages: Vec<StageStatus>,
    /// Count of stages explicitly marked `"missing"`. Drives the
    /// row badge in Library Scan (`12/15`).
    pub missing_count: usize,
    /// Count of stages with a `"complete"` verdict.
    pub complete_count: usize,
    /// Count of stages whose status couldn't be determined (Phase 1
    /// limitation for tag-embedded stages without a manifest record).
    pub unknown_count: usize,
}

/// Inspect an album directory + optional manifest and report
/// per-stage status.
///
/// Manifest record is authoritative when present. Falls back to
/// file-based heuristics for stages with sidecar outputs. Stages
/// whose output is embedded in audio-file tags (AcoustID, ReplayGain,
/// MusicBrainz, BPM, …) return `"unknown"` when the manifest record
/// is absent — Phase 2 will add an ffprobe-based detector for these.
pub fn scan_album_dir(
    album_dir: impl AsRef<Path>,
    manifest: Option<&ManifestFile>,
) -> EnrichmentGapReport {
    let dir = album_dir.as_ref();
    let audio_files = collect_audio_files(dir);
    let audio_count = audio_files.len();

    // Gather the latest enrichment block across all sources — most
    // manifests have a single source, but a multi-source manifest
    // (e.g. re-downloaded from a different platform) uses the union
    // of completion records as the authoritative state.
    let mut combined: BTreeMap<&'static str, bool> = BTreeMap::new();
    if let Some(mf) = manifest {
        for source in &mf.sources {
            if let Some(records) = &source.enrichment {
                for (key, rec) in records {
                    let complete = rec.completed_at.is_some();
                    combined
                        .entry(key_as_static_str(key))
                        .and_modify(|c| *c = *c || complete)
                        .or_insert(complete);
                }
            }
        }
    }

    let mut stages = Vec::with_capacity(EnrichmentStage::ALL.len());
    let mut missing_count = 0usize;
    let mut complete_count = 0usize;
    let mut unknown_count = 0usize;

    for &stage in EnrichmentStage::ALL {
        let key = stage.manifest_key();
        let status = if let Some(&complete) = combined.get(key) {
            if complete {
                "complete"
            } else {
                "missing"
            }
        } else if has_reliable_file_detector(stage) {
            if file_detector(stage, dir, &audio_files) {
                "complete"
            } else {
                "missing"
            }
        } else {
            "unknown"
        };

        match status {
            "complete" => complete_count += 1,
            "missing" => missing_count += 1,
            _ => unknown_count += 1,
        }

        stages.push(StageStatus {
            stage,
            status,
            label: stage.display_name(),
            key,
        });
    }

    EnrichmentGapReport {
        album_dir: dir.to_string_lossy().into_owned(),
        audio_file_count: audio_count,
        stages,
        missing_count,
        complete_count,
        unknown_count,
    }
}

/// Resolve a manifest-key string back to a `'static str` matching
/// the canonical [`EnrichmentStage::manifest_key`]. Strings not in
/// the registry fall through to a leaked-`'static` (one-time
/// allocation) — handles forward-compat with manifests from a
/// future MeedyaDL that registered new stages we don't know yet.
fn key_as_static_str(key: &str) -> &'static str {
    for &stage in EnrichmentStage::ALL {
        if stage.manifest_key() == key {
            return stage.manifest_key();
        }
    }
    Box::leak(key.to_string().into_boxed_str())
}

/// Reliable file-based detector for sidecar/cover-bearing stages.
/// Returns `true` when the stage's output is present.
fn file_detector(
    stage: EnrichmentStage,
    album_dir: &Path,
    audio_files: &[std::path::PathBuf],
) -> bool {
    match stage {
        EnrichmentStage::EnhancedLrc => all_have_sibling(audio_files, "lrc"),
        EnrichmentStage::RichSrt => all_have_sibling(audio_files, "srt"),
        EnrichmentStage::WebVtt => all_have_sibling(audio_files, "vtt"),
        EnrichmentStage::AssSubtitles => all_have_sibling(audio_files, "ass"),
        EnrichmentStage::AnimatedArtworkSquare => album_dir.join("FrontCover.mp4").exists(),
        EnrichmentStage::AnimatedArtworkPortrait => album_dir.join("FrontCoverPortrait.mp4").exists(),
        EnrichmentStage::AlbumSpotlight => album_dir.join("AlbumSpotlightCover.mp4").exists(),
        EnrichmentStage::StaticCoverFallback => {
            album_dir.join("Cover.jpg").exists()
                || album_dir.join("Cover.png").exists()
                || album_dir.join("FrontCover.jpg").exists()
                || album_dir.join("FrontCover.png").exists()
                || album_dir.join("Folder.jpg").exists()
                || album_dir.join("Folder.png").exists()
        }
        // All other stages have no reliable file-based detector in
        // Phase 1. Caller short-circuits these via
        // `has_reliable_file_detector` and returns "unknown".
        _ => false,
    }
}

/// Does every audio file have a sibling with the given extension?
/// (e.g. `01 Song.m4a` → expect `01 Song.lrc`.) Empty audio list →
/// `true` (vacuously) so empty album dirs aren't flagged as missing.
fn all_have_sibling(audio_files: &[std::path::PathBuf], ext: &str) -> bool {
    if audio_files.is_empty() {
        return true;
    }
    audio_files.iter().all(|path| {
        let sidecar = path.with_extension(ext);
        sidecar.exists()
    })
}

/// Collect every audio file directly under `album_dir` (non-recursive).
fn collect_audio_files(album_dir: &Path) -> Vec<std::path::PathBuf> {
    const AUDIO_EXTS: &[&str] = &["m4a", "flac", "mp3", "ogg", "opus", "wav"];
    let Ok(entries) = std::fs::read_dir(album_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| AUDIO_EXTS.iter().any(|a| a.eq_ignore_ascii_case(e)))
                    .unwrap_or(false)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::manifest::{
        EnrichmentRecord, ManifestFile, ManifestSource, ManifestTrack,
    };
    use std::fs::File;

    fn make_manifest(records: Vec<(&str, Option<&str>)>) -> ManifestFile {
        let mut enrichment = BTreeMap::new();
        for (key, completed_at) in records {
            enrichment.insert(
                key.to_string(),
                EnrichmentRecord {
                    completed_at: completed_at.map(String::from),
                    stage_version: 1,
                },
            );
        }
        ManifestFile {
            version: 1,
            app: "MeedyaDL".to_string(),
            created_at: "2026-05-18T00:00:00Z".to_string(),
            updated_at: "2026-05-18T00:00:00Z".to_string(),
            sources: vec![ManifestSource {
                platform: "apple-music".to_string(),
                url: "https://music.apple.com/test".to_string(),
                storefront: Some("us".to_string()),
                downloaded_at: "2026-05-18T00:00:00Z".to_string(),
                codec: Some("alac".to_string()),
                last_modified_date: None,
                tracks: vec![ManifestTrack {
                    number: 1,
                    disc: 1,
                    title: "T".to_string(),
                    url: None,
                    codec: None,
                    isrc: None,
                    song_id: None,
                }],
                enrichment: Some(enrichment),
                cross_platform_urls: None,
            }],
        }
    }

    #[test]
    fn manifest_key_round_trip_via_key_as_static_str() {
        for &stage in EnrichmentStage::ALL {
            let key = stage.manifest_key();
            let returned = key_as_static_str(key);
            assert_eq!(key, returned);
        }
    }

    #[test]
    fn unknown_key_in_manifest_is_preserved_as_leaked_static() {
        // A manifest from a future MeedyaDL might record a stage
        // we don't know yet. We accept the string forward so the
        // detector doesn't crash; the unknown stage just shows
        // up nowhere (since our ALL iteration only checks known
        // stages). Sanity test that the helper at least returns
        // the same string.
        let unknown = key_as_static_str("future_stage_we_dont_know");
        assert_eq!(unknown, "future_stage_we_dont_know");
    }

    #[test]
    fn manifest_complete_record_reports_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let report = scan_album_dir(
            tmp.path(),
            Some(&make_manifest(vec![("replaygain", Some("2026-05-18T00:00:00Z"))])),
        );
        let rg = report
            .stages
            .iter()
            .find(|s| s.stage == EnrichmentStage::ReplayGain)
            .unwrap();
        assert_eq!(rg.status, "complete");
    }

    #[test]
    fn manifest_record_with_null_completed_at_reports_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let report = scan_album_dir(
            tmp.path(),
            Some(&make_manifest(vec![("replaygain", None)])),
        );
        let rg = report
            .stages
            .iter()
            .find(|s| s.stage == EnrichmentStage::ReplayGain)
            .unwrap();
        assert_eq!(rg.status, "missing");
    }

    #[test]
    fn tag_stage_without_manifest_record_is_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let report = scan_album_dir(tmp.path(), None);
        // AcoustID has no reliable file detector; without a manifest
        // record it must be "unknown" — Phase 1 won't false-positive
        // legacy downloads as missing.
        let aid = report
            .stages
            .iter()
            .find(|s| s.stage == EnrichmentStage::AcoustId)
            .unwrap();
        assert_eq!(aid.status, "unknown");
    }

    #[test]
    fn sidecar_stage_uses_file_detector_when_no_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let audio = tmp.path().join("01 Song.m4a");
        File::create(&audio).unwrap();
        // No `.lrc` sibling → Enhanced LRC should be "missing".
        let report = scan_album_dir(tmp.path(), None);
        let lrc = report
            .stages
            .iter()
            .find(|s| s.stage == EnrichmentStage::EnhancedLrc)
            .unwrap();
        assert_eq!(lrc.status, "missing");

        // Add the sibling → should flip to "complete".
        File::create(tmp.path().join("01 Song.lrc")).unwrap();
        let report2 = scan_album_dir(tmp.path(), None);
        let lrc2 = report2
            .stages
            .iter()
            .find(|s| s.stage == EnrichmentStage::EnhancedLrc)
            .unwrap();
        assert_eq!(lrc2.status, "complete");
    }

    #[test]
    fn cover_sidecar_alternatives_all_count() {
        let tmp = tempfile::tempdir().unwrap();
        // None of the alternates exist → missing.
        let r0 = scan_album_dir(tmp.path(), None);
        assert_eq!(
            r0.stages
                .iter()
                .find(|s| s.stage == EnrichmentStage::StaticCoverFallback)
                .unwrap()
                .status,
            "missing"
        );
        // Folder.png alone counts.
        File::create(tmp.path().join("Folder.png")).unwrap();
        let r1 = scan_album_dir(tmp.path(), None);
        assert_eq!(
            r1.stages
                .iter()
                .find(|s| s.stage == EnrichmentStage::StaticCoverFallback)
                .unwrap()
                .status,
            "complete"
        );
    }

    #[test]
    fn counts_aggregate_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        File::create(tmp.path().join("01.m4a")).unwrap();
        File::create(tmp.path().join("01.lrc")).unwrap();
        let report = scan_album_dir(
            tmp.path(),
            Some(&make_manifest(vec![
                ("replaygain", Some("2026-05-18T00:00:00Z")),
                ("acoustid", None),
            ])),
        );
        // ReplayGain → complete; AcoustID → missing.
        // Enhanced LRC → complete via file detector.
        // All other tag stages → unknown.
        assert!(report.complete_count >= 2);
        assert!(report.missing_count >= 1);
        assert!(report.unknown_count > 0);
        assert_eq!(report.audio_file_count, 1);
    }
}
