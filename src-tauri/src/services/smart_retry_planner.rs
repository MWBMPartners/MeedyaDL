// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Smart manifest-driven retry planner (#667).
//
// When a queue item is retried after a partial-success failure (the
// dominant `GAMDL reported N per-track error(s) even though the
// process exited 0` shape), today's plain `retry()` re-runs the full
// album URL and depends on GAMDL's `overwrite=false` to skip the
// tracks that landed first time. That works for correctness but burns
// wall time on:
//   * a fresh metadata fetch for every track (rate-limit risk),
//   * re-evaluating the whole companion-tier loop,
//   * re-running every enrichment stage (ReplayGain, AcoustID,
//     MusicBrainz) against the full set of files we already tagged.
//
// This planner reads the `manifest.meedyadl` written at end-of-
// pipeline, enumerates the tracks that *should* be on disk for the
// queue item's source, scans the album directory to see which tracks
// landed first time, and returns a `RetryPlan` describing exactly
// which `album_url?i={song_id}` URLs need to re-run. The retry path
// then rewrites the queue item's URL list with the precise per-track
// set, so GAMDL only revisits the tracks that actually failed.
//
// Failure modes the planner deliberately does *not* try to handle —
// it returns `None` and the caller falls back to the existing
// "dumb retry" path:
//   * No `output_path` on the queue item (we never produced any
//     output to diff against — re-running the whole URL is correct).
//   * No manifest at the output path (manifest is written near the
//     end of enrichment, so a failure before that point genuinely
//     leaves us nothing to plan from).
//   * Manifest exists but no source matches the queue item's URL
//     (multi-platform manifest where the queue item was a different
//     platform — out of scope for this MVP).
//   * Every expected track is present on disk (nothing to retry —
//     the previous run effectively succeeded; the caller should
//     surface "nothing to do" rather than re-enqueueing).
//   * No `track.url` was recorded for any of the missing tracks
//     (older manifest written before per-track URLs were stored,
//     or a non-Apple-Music source — falling back to the album URL
//     loses no work and is what today's retry does anyway).
//
// Phase 2 (#766, landed): per-codec presence — manifest now records the
// `companion_tiers` plan (`Vec<Vec<String>>` of registry codec IDs per
// tier) at download time. The planner cross-references this against
// codec-suffix-detected on-disk files and emits `PartialCodecs` when
// the primary tier landed but companion tiers timed out.
//
// Out of scope for this MVP (still tracked in #667 as Phase 3):
//   * Atmos-update / new-track detection via fresh `lastModifiedDate`
//     comparison — adds a network call to the retry hot path.
//     (Phase 5c #717 already provides this signal as a parallel rail.)
//   * Per-track enrichment-stage skip — the planner currently only
//     scopes the GAMDL invocation, not the post-download stages.
//   * Per-track-per-codec granularity (e.g. "tracks 12–45 missing AAC,
//     1–11 have it"). Today's `PartialCodecs` reports at album-level
//     codec granularity: any tier with zero matching files is reported
//     as missing.

use std::collections::BTreeSet;
use std::path::Path;

use crate::models::codec_registry::CODEC_REGISTRY;
use crate::models::manifest::{ManifestFile, ManifestSource, ManifestTrack};

/// Audio file extensions the planner recognises as a downloaded track.
///
/// Mirrors the codecs MeedyaDL's enrichment pipeline knows about: M4A
/// (ALAC / AAC / Atmos / AC3 in MP4 container), FLAC, OGG/Opus, MP3,
/// and M4V (used for music videos that land alongside audio in some
/// album contexts). Case-insensitive comparison happens at match time.
const AUDIO_EXTS: &[&str] = &["m4a", "flac", "ogg", "opus", "mp3", "m4v"];

/// Outcome of a successful planning pass.
///
/// `urls_to_fetch` is empty only when [`PlanOutcome::AllPresent`] would
/// have been returned instead — every emitter of `RetryPlan` guarantees
/// at least one URL.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPlan {
    /// The per-track URLs (typically `album_url?i={song_id}`) that the
    /// retry should pass to GAMDL. The order mirrors the manifest's
    /// track order so GAMDL's progress UI stays in album order.
    pub urls_to_fetch: Vec<String>,
    /// How many tracks we need to re-fetch.
    pub missing_tracks: usize,
    /// How many tracks the manifest said we should have ended up with.
    pub total_tracks: usize,
    /// The album URL the manifest source was downloaded from. Surfaced
    /// in the activity log so the user can correlate the retry with
    /// the original request.
    pub source_url: String,
}

/// Album-level codec-tier gap — every track is present in *some* codec
/// variant, but at least one tier the user requested at download time
/// (e.g. an AAC companion) has zero matching files on disk.
///
/// Surfaced after companion-tier timeouts where the primary codec
/// landed (45 Atmos files) but later tiers never completed (#766).
#[derive(Debug, Clone, PartialEq)]
pub struct PartialCodecsPlan {
    /// Album URL to re-queue. GAMDL's `overwrite=false` semantics
    /// preserve the successful primary files; only the missing
    /// companion variants will be re-fetched.
    pub source_url: String,
    /// Canonical codec-registry IDs (e.g. `"aac-hq"`, `"aac-mq"`) of
    /// tiers that have no matching files on disk. One entry per
    /// missing tier — for multi-codec tiers (fallback chains), the
    /// first codec in the tier is reported as the representative.
    pub missing_codecs: Vec<String>,
    /// Total expected tracks (from the manifest). Surfaced so the UI
    /// can say "N of M present, but missing the AAC variant".
    pub total_tracks: usize,
}

/// Top-level planner outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanOutcome {
    /// Manifest was found, diffed cleanly, and produced a precise
    /// per-track URL list. Caller should use `plan.urls_to_fetch`
    /// in place of the queue item's original URLs.
    Plan(RetryPlan),
    /// All expected tracks are present in *at least one* codec, but
    /// the manifest records additional codec tiers (companion downloads)
    /// that have no matching files on disk. Re-queueing the album URL
    /// will fill the codec gaps without re-downloading the primary
    /// files (#766, Phase 2 of #717/5b).
    PartialCodecs(PartialCodecsPlan),
    /// Manifest was found but every expected track is already on
    /// disk. Caller should *not* re-enqueue — emit a "nothing to
    /// retry" log line instead. Includes the total track count so
    /// the message can be specific.
    AllPresent { total_tracks: usize },
    /// No usable manifest was found, or no source matched the queue
    /// item's URL, or some other condition the planner can't help
    /// with. Caller should fall through to the existing dumb retry.
    NotApplicable,
}

/// Plan a smart retry for a queue item that lives at `output_path`
/// and was downloaded from `item_url`.
///
/// `output_path` is the album directory (where audio files + the
/// `manifest.meedyadl` file co-exist). When the queue item's
/// `output_is_directory` is `false` (a single-file download such as
/// a music-video URL), pass the file's parent directory instead —
/// the planner is a no-op (`NotApplicable`) for single-file outputs
/// because they don't carry a manifest of their own.
///
/// Returns [`PlanOutcome`] describing whether a smart retry can
/// proceed. The caller (in `download_queue.rs`'s `retry()` path)
/// inspects the variant and either rewrites the URL list, logs
/// "nothing to do", or falls back to dumb retry.
#[must_use]
pub fn plan_retry(output_path: &Path, item_url: &str) -> PlanOutcome {
    if !output_path.is_dir() {
        return PlanOutcome::NotApplicable;
    }

    let manifest_path = output_path.join("manifest.meedyadl");
    let manifest = match read_manifest(&manifest_path) {
        Some(m) => m,
        None => return PlanOutcome::NotApplicable,
    };

    // Find the source that matches the queue item's URL. Multi-platform
    // manifests can hold several sources (e.g., the same album was once
    // downloaded from Apple Music and later from Spotify); the queue
    // item we're retrying maps to exactly one of them.
    let source = match find_matching_source(&manifest, item_url) {
        Some(s) => s,
        None => return PlanOutcome::NotApplicable,
    };

    if source.tracks.is_empty() {
        // Manifest doesn't have track-level info (older format, or a
        // single-track source). No safe diff possible.
        return PlanOutcome::NotApplicable;
    }

    let present = scan_present_track_numbers(output_path);
    let missing = collect_missing_tracks(&source.tracks, &present);

    if !missing.is_empty() {
        // Track-level gaps take precedence over codec-level gaps —
        // a missing track is the more severe gap. Build the per-track
        // URL list. Skip tracks that don't have a recorded `track.url`
        // — without it we have no way to address the single track, so
        // we'd have to fall back to the album URL anyway. If *every*
        // missing track lacks a URL, there's no smart plan to be had
        // and we return NotApplicable.
        let urls_to_fetch: Vec<String> = missing
            .iter()
            .filter_map(|t| t.url.clone())
            .collect();

        if urls_to_fetch.is_empty() {
            return PlanOutcome::NotApplicable;
        }

        return PlanOutcome::Plan(RetryPlan {
            urls_to_fetch,
            missing_tracks: missing.len(),
            total_tracks: source.tracks.len(),
            source_url: source.url.clone(),
        });
    }

    // Every track is present in *some* codec variant. Check whether
    // the manifest's companion-tier plan has unsatisfied tiers — this
    // is the companion-codec-timeout case (#766): primary landed,
    // companion AAC / AAC-Legacy / etc. never completed. Manifests
    // written before this field existed have `companion_tiers: None`
    // and fall through to today's `AllPresent` behaviour.
    if let Some(tiers) = source.companion_tiers.as_ref() {
        let missing_codecs = find_missing_tier_codecs(output_path, tiers);
        if !missing_codecs.is_empty() {
            return PlanOutcome::PartialCodecs(PartialCodecsPlan {
                source_url: source.url.clone(),
                missing_codecs,
                total_tracks: source.tracks.len(),
            });
        }
    }

    PlanOutcome::AllPresent {
        total_tracks: source.tracks.len(),
    }
}

/// Walk the album directory and return the canonical codec IDs of
/// every tier in `tiers` that has **no** matching audio file on disk.
///
/// A tier is "satisfied" when at least one of its `codecs_to_try`
/// entries (canonical registry IDs) has a corresponding file —
/// matched by the codec's filename suffix from `codecs.toml`.
/// Empty-suffix codecs (e.g. `aac-hq`, which produces clean
/// filenames) are matched by detecting an audio file whose stem
/// carries no known codec bracket (`[Dolby Atmos]`, `[Lossless]`,
/// etc.). The first codec listed in a missing tier is reported as
/// the representative ID — multi-codec fallback chains
/// (`["aac-hq", "aac-mq"]`) report the first entry.
///
/// Tiers with no recognised codec IDs (e.g. a stale manifest from a
/// future MeedyaDL version that introduced a codec ID our local
/// `codecs.toml` doesn't know about yet) are skipped silently —
/// reporting them as missing would force a redundant re-download.
fn find_missing_tier_codecs(album_dir: &Path, tiers: &[Vec<String>]) -> Vec<String> {
    let suffix_inventory = scan_suffix_inventory(album_dir);
    let mut missing = Vec::new();
    for tier in tiers {
        if tier.is_empty() {
            continue;
        }
        let mut tier_satisfied = false;
        let mut representative: Option<&str> = None;
        for codec_id in tier {
            // Resolve the registry entry for this codec ID. Unknown
            // IDs (forward-compat) are silently ignored.
            let Some(entry) = CODEC_REGISTRY.audio.iter().find(|c| c.id == *codec_id) else {
                continue;
            };
            if representative.is_none() {
                representative = Some(codec_id);
            }
            // Empty / missing suffix → clean filename — a tier
            // codec with no bracket is satisfied by any file that
            // carries no known suffix bracket. Non-empty suffix
            // requires the bracket text to appear in the inventory.
            let suffix = entry.suffix.as_deref().unwrap_or("");
            if suffix.is_empty() {
                if suffix_inventory.contains(&String::new()) {
                    tier_satisfied = true;
                    break;
                }
            } else if suffix_inventory.contains(suffix) {
                tier_satisfied = true;
                break;
            }
        }
        if !tier_satisfied {
            if let Some(rep) = representative {
                missing.push(rep.to_string());
            }
        }
    }
    missing
}

/// Build the set of distinct codec suffixes observed in the album
/// directory. Each present audio file maps to either a known suffix
/// bracket (e.g. `"[Dolby Atmos]"`, `"[Lossless]"`) or the empty
/// string (clean filename, no known bracket).
///
/// Used by `find_missing_tier_codecs` to decide whether each planned
/// tier is satisfied. We pre-compute the set once per `plan_retry`
/// call instead of re-walking the directory for every tier.
fn scan_suffix_inventory(album_dir: &Path) -> BTreeSet<String> {
    let mut inventory = BTreeSet::new();
    let entries = match std::fs::read_dir(album_dir) {
        Ok(e) => e,
        Err(_) => return inventory,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Multi-disc subdirs: recurse one level into recognised
            // disc folders so codec suffixes on those files count too.
            if parse_disc_dir_name(&path).is_some() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    for inner in sub.flatten() {
                        accumulate_suffix(&inner.path(), &mut inventory);
                    }
                }
            }
            continue;
        }
        accumulate_suffix(&path, &mut inventory);
    }
    inventory
}

/// Inspect a single file path; if it's an audio file, record its
/// codec suffix (empty string for clean filenames). Helper for
/// `scan_suffix_inventory`.
fn accumulate_suffix(path: &Path, inventory: &mut BTreeSet<String>) {
    if !is_audio_file(path) {
        return;
    }
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        inventory.insert(detect_codec_suffix(stem));
    }
}

/// Return the longest known codec suffix bracket found in `stem`,
/// or an empty `String` when none of the registered suffixes match
/// (i.e. a clean filename).
///
/// "Longest match wins" handles the (currently hypothetical) case
/// where one codec's suffix is a substring of another's — without it
/// a future `"[AAC]"` suffix would shadow `"[AAC Legacy]"`. Today's
/// `codecs.toml` doesn't have any such overlap but the safeguard is
/// cheap.
fn detect_codec_suffix(stem: &str) -> String {
    let mut best = String::new();
    for codec in &CODEC_REGISTRY.audio {
        if let Some(suffix) = codec.suffix.as_deref() {
            if !suffix.is_empty() && stem.contains(suffix) && suffix.len() > best.len() {
                best = suffix.to_string();
            }
        }
    }
    best
}

/// Read + parse a manifest file. Returns `None` for any I/O or parse
/// failure (the file may be missing on a fresh download, or corrupt
/// from an interrupted previous write).
fn read_manifest(path: &Path) -> Option<ManifestFile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Pick the manifest source whose `url` matches `item_url`.
///
/// Tries an exact match first (fast, common case), then a normalised
/// match that ignores trailing slashes and the `?i={song_id}` query
/// fragment. The query fragment is stripped because the queue item
/// may carry a per-track URL (`/album/...?i=123`) while the manifest
/// recorded the album-level URL.
fn find_matching_source<'a>(
    manifest: &'a ManifestFile,
    item_url: &str,
) -> Option<&'a ManifestSource> {
    if let Some(s) = manifest.sources.iter().find(|s| s.url == item_url) {
        return Some(s);
    }
    let normalised_item = normalise_url(item_url);
    manifest
        .sources
        .iter()
        .find(|s| normalise_url(&s.url) == normalised_item)
}

/// Strip trailing slashes and `?i={song_id}` query so per-track URLs
/// match album-level URLs. Hash fragments are also dropped.
fn normalise_url(url: &str) -> String {
    let no_fragment = url.split('#').next().unwrap_or(url);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    no_query.trim_end_matches('/').to_string()
}

/// Walk the album directory and collect the set of `(disc, track_num)`
/// tuples that have an audio file present.
///
/// Filename heuristic mirrors the dominant GAMDL output template:
/// `01 Title.m4a`, `02 Title.flac`, multi-disc albums use a `D-T` or
/// `Disc D/T` style. We accept anything where the filename starts
/// with one or two digits followed by a separator. When disc info is
/// absent we default to disc 1 (matches the manifest's default).
fn scan_present_track_numbers(album_dir: &Path) -> BTreeSet<(u32, u32)> {
    let mut present = BTreeSet::new();
    let entries = match std::fs::read_dir(album_dir) {
        Ok(e) => e,
        Err(_) => return present,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Multi-disc layouts: GAMDL writes `Disc 1/`, `CD 02/`,
            // etc. as subdirectories. Recurse one level so we pick
            // up tracks regardless of layout choice.
            if let Some(disc) = parse_disc_dir_name(&path) {
                for (_disc_from_file, track) in scan_audio_files(&path) {
                    // Use the disc derived from the directory name as
                    // canonical — files inside `Disc 02/` may or may
                    // not have a `02-01` filename prefix depending on
                    // the user's template.
                    present.insert((disc, track));
                }
            }
            continue;
        }
        if !is_audio_file(&path) {
            continue;
        }
        if let Some((disc, track)) = parse_track_number_from_filename(&path) {
            present.insert((disc, track));
        }
    }

    present
}

/// Scan a single directory for audio files and return their parsed
/// (disc, track) numbers. Used by the multi-disc subdirectory walker.
fn scan_audio_files(dir: &Path) -> Vec<(u32, u32)> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    entries
        .flatten()
        .filter(|e| {
            let p = e.path();
            p.is_file() && is_audio_file(&p)
        })
        .filter_map(|e| parse_track_number_from_filename(&e.path()))
        .collect()
}

/// Returns `true` when `path` ends in one of the [`AUDIO_EXTS`].
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_ascii_lowercase();
            AUDIO_EXTS.iter().any(|x| *x == lower)
        })
        .unwrap_or(false)
}

/// Try to parse a `(disc, track)` pair from a filename like:
///   `01 Title.m4a` -> (1, 1)
///   `02-05 Title.flac` -> (2, 5)
///   `1-12 Foo.m4a` -> (1, 12)
///   `Disc 02/03 Title.m4a` -> caller should provide disc=2 from the dir
///
/// Returns `None` for filenames that don't start with at least one
/// digit (e.g. cover art that snuck in, hidden dotfiles).
fn parse_track_number_from_filename(path: &Path) -> Option<(u32, u32)> {
    let stem = path.file_stem()?.to_str()?;
    let stem = stem.trim_start();

    // Try `D-T ` shape first (multi-disc explicit prefix).
    if let Some(dash_pos) = stem.find('-') {
        let (a, b) = stem.split_at(dash_pos);
        let b = &b[1..]; // drop the '-'
        let a = a.trim();
        // `b` continues with the track number, then a separator and title.
        let track_part: String = b.chars().take_while(char::is_ascii_digit).collect();
        if !a.is_empty()
            && a.chars().all(|c| c.is_ascii_digit())
            && !track_part.is_empty()
        {
            if let (Ok(d), Ok(t)) = (a.parse::<u32>(), track_part.parse::<u32>()) {
                if d > 0 && t > 0 {
                    return Some((d, t));
                }
            }
        }
    }

    // Fall back to single-disc `T ` shape.
    let track_part: String = stem.chars().take_while(char::is_ascii_digit).collect();
    if track_part.is_empty() {
        return None;
    }
    let track_num: u32 = track_part.parse().ok()?;
    if track_num == 0 {
        return None;
    }
    Some((1, track_num))
}

/// Extract a disc number from a subdirectory name like `Disc 1`,
/// `Disc 02`, `CD 3`, `Disc-2`. Returns `None` for anything that
/// doesn't match — the caller skips those subdirectories rather
/// than confusing them with the per-album track set.
fn parse_disc_dir_name(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let trimmed = lower
        .strip_prefix("disc")
        .or_else(|| lower.strip_prefix("cd"))?;
    let digits: String = trimmed
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    let disc: u32 = digits.parse().ok()?;
    (disc > 0).then_some(disc)
}

/// Diff the manifest's expected tracks against the on-disk presence
/// set and return the manifest entries that are missing.
fn collect_missing_tracks<'a>(
    expected: &'a [ManifestTrack],
    present: &BTreeSet<(u32, u32)>,
) -> Vec<&'a ManifestTrack> {
    expected
        .iter()
        .filter(|t| !present.contains(&(t.disc, t.number)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn track(number: u32, disc: u32, song_id: &str, album_url: &str) -> ManifestTrack {
        ManifestTrack {
            number,
            disc,
            title: format!("Track {number}"),
            url: Some(format!("{album_url}?i={song_id}")),
            codec: None,
            isrc: None,
            song_id: Some(song_id.to_string()),
        }
    }

    fn write_manifest(dir: &Path, source: ManifestSource) {
        let manifest = ManifestFile::new(source);
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        fs::write(dir.join("manifest.meedyadl"), json).unwrap();
    }

    fn touch(dir: &Path, name: &str) {
        fs::File::create(dir.join(name)).unwrap();
    }

    #[test]
    fn plans_retry_for_missing_tracks_only() {
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: url.to_string(),
                storefront: Some("gb".to_string()),
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![
                    track(1, 1, "1001", url),
                    track(2, 1, "1002", url),
                    track(3, 1, "1003", url),
                    track(4, 1, "1004", url),
                ],
                enrichment: None,
                cross_platform_urls: None,
            is_library: false,
            },
        );
        // Tracks 1 and 3 made it; 2 and 4 are missing.
        touch(dir.path(), "01 Track 1.m4a");
        touch(dir.path(), "03 Track 3.m4a");

        let outcome = plan_retry(dir.path(), url);
        match outcome {
            PlanOutcome::Plan(plan) => {
                assert_eq!(plan.missing_tracks, 2);
                assert_eq!(plan.total_tracks, 4);
                assert_eq!(plan.source_url, url);
                assert_eq!(
                    plan.urls_to_fetch,
                    vec![
                        format!("{url}?i=1002"),
                        format!("{url}?i=1004"),
                    ],
                );
            }
            other => panic!("expected Plan, got {other:?}"),
        }
    }

    #[test]
    fn returns_all_present_when_disk_matches_manifest() {
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: url.to_string(),
                storefront: None,
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![track(1, 1, "1001", url), track(2, 1, "1002", url)],
                enrichment: None,
                cross_platform_urls: None,
            is_library: false,
            },
        );
        touch(dir.path(), "01 Track 1.m4a");
        touch(dir.path(), "02 Track 2.flac");

        assert_eq!(
            plan_retry(dir.path(), url),
            PlanOutcome::AllPresent { total_tracks: 2 },
        );
    }

    #[test]
    fn returns_not_applicable_when_no_manifest() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            plan_retry(dir.path(), "https://music.apple.com/gb/album/foo/123"),
            PlanOutcome::NotApplicable,
        );
    }

    #[test]
    fn returns_not_applicable_when_url_does_not_match_manifest_source() {
        let dir = TempDir::new().unwrap();
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: "https://music.apple.com/gb/album/foo/123".to_string(),
                storefront: None,
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![track(1, 1, "1001", "https://music.apple.com/gb/album/foo/123")],
                enrichment: None,
                cross_platform_urls: None,
            is_library: false,
            },
        );
        // Different album URL than what the manifest recorded.
        let outcome = plan_retry(
            dir.path(),
            "https://music.apple.com/gb/album/bar/999",
        );
        assert_eq!(outcome, PlanOutcome::NotApplicable);
    }

    #[test]
    fn matches_url_when_query_differs() {
        // The queue item URL may carry `?i=` (per-track retry) while
        // the manifest source recorded the album-level URL. Normalisation
        // strips the query before comparing.
        let dir = TempDir::new().unwrap();
        let album = "https://music.apple.com/gb/album/foo/123";
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: album.to_string(),
                storefront: None,
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![track(1, 1, "1001", album), track(2, 1, "1002", album)],
                enrichment: None,
                cross_platform_urls: None,
            is_library: false,
            },
        );
        // Disk has track 1 only.
        touch(dir.path(), "01 foo.m4a");
        let outcome = plan_retry(dir.path(), &format!("{album}?i=1002"));
        match outcome {
            PlanOutcome::Plan(plan) => assert_eq!(plan.urls_to_fetch, vec![format!("{album}?i=1002")]),
            other => panic!("expected Plan, got {other:?}"),
        }
    }

    #[test]
    fn returns_not_applicable_when_missing_tracks_have_no_url() {
        // Older manifest (pre per-track URL): `track.url` is None so
        // the planner has no per-track address. Falling back to the
        // album URL would lose nothing vs a dumb retry, so we punt.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        let mut t = track(1, 1, "1001", url);
        t.url = None;
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: url.to_string(),
                storefront: None,
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![t],
                enrichment: None,
                cross_platform_urls: None,
            is_library: false,
            },
        );
        assert_eq!(plan_retry(dir.path(), url), PlanOutcome::NotApplicable);
    }

    #[test]
    fn parses_multi_disc_filename_prefix() {
        let path = Path::new("/tmp/02-05 Track Title.m4a");
        assert_eq!(parse_track_number_from_filename(path), Some((2, 5)));

        let path = Path::new("/tmp/1-12 Foo.flac");
        assert_eq!(parse_track_number_from_filename(path), Some((1, 12)));
    }

    #[test]
    fn parses_single_disc_filename_prefix() {
        let path = Path::new("/tmp/01 Title.m4a");
        assert_eq!(parse_track_number_from_filename(path), Some((1, 1)));

        let path = Path::new("/tmp/12 Title.flac");
        assert_eq!(parse_track_number_from_filename(path), Some((1, 12)));
    }

    #[test]
    fn rejects_filenames_without_digit_prefix() {
        assert_eq!(parse_track_number_from_filename(Path::new("Cover.jpg")), None);
        assert_eq!(parse_track_number_from_filename(Path::new("manifest.meedyadl")), None);
        assert_eq!(parse_track_number_from_filename(Path::new(".DS_Store")), None);
    }

    #[test]
    fn rejects_zero_track_or_disc_numbers() {
        // Edge case: a leading "0" track number (real-world: hidden bonus
        // intro tracks numbered as 0) is accepted as track 0 by the parser
        // but our domain rule says manifest track numbers are 1-based, so
        // we treat 0 as invalid to avoid spurious matches against tests.
        assert_eq!(parse_track_number_from_filename(Path::new("00 Hidden.m4a")), None);
    }

    #[test]
    fn parses_disc_subdirectory_names() {
        assert_eq!(parse_disc_dir_name(Path::new("/x/Disc 1")), Some(1));
        assert_eq!(parse_disc_dir_name(Path::new("/x/Disc 02")), Some(2));
        assert_eq!(parse_disc_dir_name(Path::new("/x/disc-3")), Some(3));
        assert_eq!(parse_disc_dir_name(Path::new("/x/CD 4")), Some(4));
        assert_eq!(parse_disc_dir_name(Path::new("/x/Bonus")), None);
    }

    // ----------------------------------------------------------
    // Codec-tier gap detection (#766, Phase 2 of #717/5b)
    // ----------------------------------------------------------

    /// Build a manifest source with the full Atmos→all-formats tier
    /// plan for tests. Matches `CompanionMode::AtmosToAllFormats` with
    /// `primary_codec == "atmos"` from `download_queue::plan_companions`:
    /// primary Atmos, then AC3, then ALAC, then [AAC, AAC-Legacy].
    fn atmos_all_formats_source(url: &str) -> ManifestSource {
        ManifestSource {
            platform: "apple-music".to_string(),
            url: url.to_string(),
            storefront: None,
            downloaded_at: "2026-05-13T00:00:00Z".to_string(),
            codec: Some("eac3-atmos".to_string()),
            last_modified_date: None,
            companion_tiers: Some(vec![
                vec!["eac3-atmos".to_string()],
                vec!["ac3".to_string()],
                vec!["alac".to_string()],
                vec!["aac-hq".to_string(), "aac-mq".to_string()],
            ]),
            tracks: vec![
                track(1, 1, "1001", url),
                track(2, 1, "1002", url),
            ],
        }
    }

    #[test]
    fn partial_codecs_when_primary_landed_but_companions_missing() {
        // The exact companion-timeout scenario from the original
        // user report: primary Atmos files all landed (suffixed
        // `[Dolby Atmos]`); AC3 + ALAC + AAC companion tiers never
        // produced files. Track diff says all-present, but the
        // codec-tier diff should fire.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(dir.path(), atmos_all_formats_source(url));
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Atmos].m4a");

        match plan_retry(dir.path(), url) {
            PlanOutcome::PartialCodecs(plan) => {
                assert_eq!(plan.source_url, url);
                assert_eq!(plan.total_tracks, 2);
                // Tier 0 (Atmos) is satisfied. Tiers 1/2/3 are all
                // missing — AC3, ALAC, and the AAC/AAC-Legacy fallback
                // chain. Representative IDs are the first codec of
                // each missing tier.
                assert_eq!(
                    plan.missing_codecs,
                    vec![
                        "ac3".to_string(),
                        "alac".to_string(),
                        "aac-hq".to_string(),
                    ],
                );
            }
            other => panic!("expected PartialCodecs, got {other:?}"),
        }
    }

    #[test]
    fn partial_codecs_satisfied_by_clean_filenames_for_empty_suffix_tier() {
        // The AAC tier has `aac-hq` (suffix "") and `aac-mq` (suffix
        // "[AAC Legacy]"). A clean filename satisfies the tier via the
        // `aac-hq` codec; the tier should NOT be reported as missing.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(dir.path(), atmos_all_formats_source(url));
        // Atmos landed, AC3 + ALAC landed, AAC landed with clean
        // filenames (no bracket suffix) — every tier satisfied.
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Atmos].m4a");
        touch(dir.path(), "01 Track 1 [Dolby Digital].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Digital].m4a");
        touch(dir.path(), "01 Track 1 [Lossless].m4a");
        touch(dir.path(), "02 Track 2 [Lossless].m4a");
        touch(dir.path(), "01 Track 1.m4a");
        touch(dir.path(), "02 Track 2.m4a");

        assert_eq!(
            plan_retry(dir.path(), url),
            PlanOutcome::AllPresent { total_tracks: 2 },
        );
    }

    #[test]
    fn partial_codecs_satisfied_by_aac_legacy_fallback() {
        // The AAC tier carries the [aac-hq, aac-mq] fallback chain.
        // If only AAC-Legacy landed (suffix "[AAC Legacy]"), the tier
        // is still satisfied via the second codec — no gap reported.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(dir.path(), atmos_all_formats_source(url));
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Atmos].m4a");
        touch(dir.path(), "01 Track 1 [Dolby Digital].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Digital].m4a");
        touch(dir.path(), "01 Track 1 [Lossless].m4a");
        touch(dir.path(), "02 Track 2 [Lossless].m4a");
        touch(dir.path(), "01 Track 1 [AAC Legacy].m4a");
        touch(dir.path(), "02 Track 2 [AAC Legacy].m4a");

        assert_eq!(
            plan_retry(dir.path(), url),
            PlanOutcome::AllPresent { total_tracks: 2 },
        );
    }

    #[test]
    fn track_level_plan_takes_precedence_over_codec_gap() {
        // If both a track is missing AND companion tiers are missing,
        // the Plan variant wins — a missing track is the more severe
        // gap and the existing dumb retry handles both at once.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(dir.path(), atmos_all_formats_source(url));
        // Only track 1 landed (track 2 missing) and only in Atmos
        // (companion tiers also missing).
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");

        match plan_retry(dir.path(), url) {
            PlanOutcome::Plan(plan) => {
                assert_eq!(plan.missing_tracks, 1);
                assert_eq!(plan.total_tracks, 2);
            }
            other => panic!("expected Plan (track-level takes priority), got {other:?}"),
        }
    }

    #[test]
    fn backward_compat_no_companion_tiers_falls_through_to_all_present() {
        // Older manifests written before #766 have
        // `companion_tiers: None`. The planner must NOT report a
        // PartialCodecs gap for these — it has no idea what was
        // planned. Falls through to today's `AllPresent` semantics.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: url.to_string(),
                storefront: None,
                downloaded_at: "2026-04-30T00:00:00Z".to_string(),
                codec: None,
                last_modified_date: None,
                companion_tiers: None,
                tracks: vec![
                    track(1, 1, "1001", url),
                    track(2, 1, "1002", url),
                ],
            },
        );
        // Atmos primary landed; no other codecs. With None
        // companion_tiers, we can't diff codec presence → AllPresent.
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");
        touch(dir.path(), "02 Track 2 [Dolby Atmos].m4a");

        assert_eq!(
            plan_retry(dir.path(), url),
            PlanOutcome::AllPresent { total_tracks: 2 },
        );
    }

    #[test]
    fn unknown_codec_id_in_companion_tiers_skipped_silently() {
        // Forward-compat: a manifest written by a future MeedyaDL
        // build with a codec the local `codecs.toml` doesn't know
        // about must NOT be reported as missing — reporting it would
        // force a redundant re-download.
        let dir = TempDir::new().unwrap();
        let url = "https://music.apple.com/gb/album/foo/123";
        write_manifest(
            dir.path(),
            ManifestSource {
                platform: "apple-music".to_string(),
                url: url.to_string(),
                storefront: None,
                downloaded_at: "2026-05-13T00:00:00Z".to_string(),
                codec: Some("eac3-atmos".to_string()),
                last_modified_date: None,
                companion_tiers: Some(vec![
                    vec!["eac3-atmos".to_string()],
                    // Hypothetical future codec the local registry
                    // doesn't recognise — skipped silently.
                    vec!["dts-x".to_string()],
                ]),
                tracks: vec![track(1, 1, "1001", url)],
            },
        );
        touch(dir.path(), "01 Track 1 [Dolby Atmos].m4a");

        assert_eq!(
            plan_retry(dir.path(), url),
            PlanOutcome::AllPresent { total_tracks: 1 },
        );
    }

    #[test]
    fn detect_codec_suffix_picks_longest_match() {
        // Sanity check on the longest-match safeguard. None of the
        // current codec suffixes are substrings of each other, so
        // this test demonstrates the contract rather than catching
        // a current overlap.
        assert_eq!(detect_codec_suffix("01 Foo [Dolby Atmos]"), "[Dolby Atmos]");
        assert_eq!(detect_codec_suffix("01 Foo [Lossless]"), "[Lossless]");
        assert_eq!(detect_codec_suffix("01 Foo [AAC Legacy]"), "[AAC Legacy]");
        assert_eq!(detect_codec_suffix("01 Foo"), "");
    }
}
