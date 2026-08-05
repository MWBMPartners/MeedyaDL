// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Output-path detection, format annotation, file counting, manifest writing, and cover-art helpers.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;

/// Normalises a URL for duplicate detection by lowercasing the domain,
/// stripping trailing slashes, and removing query parameters except
/// essential ones like `?i=` (Apple Music track IDs within album URLs).
///
/// This produces a canonical form so that cosmetically different URLs
/// pointing to the same resource are recognised as duplicates:
///   - `https://Music.Apple.Com/us/album/...` == `https://music.apple.com/us/album/...`
///   - `https://music.apple.com/us/album/foo/123/` == `https://music.apple.com/us/album/foo/123`
///   - `https://music.apple.com/us/album/foo/123?ls=1` == `https://music.apple.com/us/album/foo/123`
///   - `https://music.apple.com/us/album/foo/123?i=456` is kept distinct (track-specific)
///
/// Extract album name and artist name from an Apple Music URL at enqueue time.
///
/// Apple Music URLs have the format:
///   `https://music.apple.com/{storefront}/album/{album-slug}/{id}`
///
/// The album slug is a hyphenated lowercase version of the album name.
/// Artist name is not available from the URL alone — it will be populated
/// later from the Apple Music API during enrichment (Step 1). For artist
/// URLs the artist slug is extracted instead.
///
/// Returns `(album_name, artist_name)` — artist is always `None` from URL parsing.
pub(crate) fn extract_album_info_from_url(url: &str) -> (Option<String>, Option<String>) {
    let Ok(parsed) = url::Url::parse(url) else {
        return (None, None);
    };
    let segments: Vec<&str> = parsed.path_segments().map_or(vec![], |s| s.collect());

    // Find album slug: /album/{slug}/{id}
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "album" && i + 1 < segments.len() {
            let slug = segments[i + 1];
            // Convert hyphenated slug to title case: "the-platinum-collection" → "The Platinum Collection"
            let name = slug
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            return (Some(name), None);
        }
        // Artist URLs: /artist/{slug}/{id}
        if *seg == "artist" && i + 1 < segments.len() {
            let slug = segments[i + 1];
            let name = slug
                .split('-')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            return (None, Some(name));
        }
    }

    (None, None)
}

/// Format a human-readable label identifying the content of a queue item,
/// for use in user-visible activity-log messages where "this content" would
/// otherwise be ambiguous. Prefers the cached Apple Music API names
/// (`artist_name` — `album_name` — `current_track`) populated at enqueue
/// time, and falls back to the first URL when names are unavailable.
///
/// `pub(crate)` because the activity log emitter ([`crate::utils::activity_log::emit_download_log`])
/// auto-enriches every `[MeedyaDL]` line with this label so the on-disk
/// log and UI both identify which queued item a message refers to.
pub(crate) fn format_content_label(status: &QueueItemStatus) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if let Some(artist) = status.artist_name.as_deref().filter(|s| !s.is_empty()) {
        parts.push(artist);
    }
    if let Some(album) = status.album_name.as_deref().filter(|s| !s.is_empty()) {
        parts.push(album);
    }
    if let Some(track) = status.current_track.as_deref().filter(|s| !s.is_empty()) {
        parts.push(track);
    }
    if !parts.is_empty() {
        return parts.join(" — ");
    }
    status
        .urls
        .first()
        .map(|u| redact_url_query(u))
        .unwrap_or_else(|| "unknown content".to_string())
}

pub(crate) fn normalize_url_for_dedup(url: &str) -> String {
    // Split scheme + authority from path+query.
    // URL structure: scheme://authority/path?query#fragment
    let url = url.trim();

    // Find the end of "scheme://".
    let after_scheme = if let Some(pos) = url.find("://") {
        pos + 3
    } else {
        // Not a valid URL — return lowercased as-is for best-effort comparison.
        return url.to_lowercase();
    };

    // Split authority (domain[:port]) from the rest at the first `/` after `://`.
    let (before_path, path_and_rest) = match url[after_scheme..].find('/') {
        Some(slash_pos) => {
            let abs = after_scheme + slash_pos;
            (&url[..abs], &url[abs..])
        }
        None => {
            // No path — just the domain (e.g., `https://example.com`).
            return url.to_lowercase();
        }
    };

    // Lowercase the scheme + authority (domain is case-insensitive per RFC 3986).
    let lower_authority = before_path.to_lowercase();

    // Strip fragment (#...) first, then handle query.
    let without_fragment = path_and_rest.split('#').next().unwrap_or(path_and_rest);

    // Split path from query string.
    let (path, query) = match without_fragment.find('?') {
        Some(q) => (&without_fragment[..q], Some(&without_fragment[q + 1..])),
        None => (without_fragment, None),
    };

    // Strip trailing slashes from the path.
    let path = path.trim_end_matches('/');

    // Keep only the `i=` query parameter (Apple Music track ID within an album).
    let essential_query = query.and_then(|q| {
        q.split('&')
            .find(|param| param.starts_with("i="))
            .map(|param| format!("?{param}"))
    });

    format!(
        "{lower_authority}{path}{}",
        essential_query.unwrap_or_default()
    )
}

/// Finds the deepest (leaf) subdirectory containing audio files (.m4a/.m4v).
///
/// GAMDL creates an `Artist/Album/` directory structure under the base output
/// path. This function must return the **album** directory (where audio files
/// live), not the artist directory.
///
/// When `artist_hint` and/or `album_hint` are provided (from the early metadata
/// fetch), the search first attempts a targeted path match before falling back
/// to the generic timestamp-based scan. This prevents cross-contamination
/// between concurrent downloads (#452) where the generic scan might return
/// a different artist's most recently modified directory.
///
/// Fixed in #447/#452: previously only searched one level deep and used
/// recency-based selection that could pick the wrong artist's directory.
pub(crate) fn find_album_directory(
    base_dir: &std::path::Path,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
) -> Option<String> {
    // --- Targeted search: use artist/album names to find the exact directory ---
    // GAMDL's default template creates: base_dir/Artist/Album/
    if let (Some(artist), Some(album)) = (artist_hint, album_hint) {
        let targeted = base_dir.join(artist).join(album);
        if targeted.is_dir() && has_direct_audio_files(&targeted) {
            log::info!(
                "find_album_directory: targeted match at {}",
                targeted.display()
            );
            return Some(targeted.to_string_lossy().to_string());
        }
        // Try case-insensitive match on the artist/album directory names
        if let Some(found) = find_directory_case_insensitive(base_dir, artist, album) {
            log::info!(
                "find_album_directory: case-insensitive match at {}",
                found.display()
            );
            return Some(found.to_string_lossy().to_string());
        }
    }

    // --- Fallback: generic deep scan (picks most recently modified leaf dir) ---
    // Depth-bounded to 10 levels (#844) — matches the convention used by
    // `find_dirs_with_ttml` and `scan_folder_for_manifests`. Without the cap,
    // pointing this at a 484-album library walked tens of thousands of dirs
    // and added 30-60 s of pure I/O before any actual enrichment work began.
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;

    find_deepest_audio_dir(base_dir, &mut best, 0);

    // If no subdirectory with audio files found, check if base itself has audio
    if best.is_none() && has_direct_audio_files(base_dir) {
        return Some(base_dir.to_string_lossy().to_string());
    }

    best.map(|(_, p)| p.to_string_lossy().to_string())
}

/// Case-insensitive directory matching for Artist/Album structure.
/// Handles slight differences in filesystem naming vs. API naming
/// (e.g., special characters stripped, Unicode normalisation).
pub(crate) fn find_directory_case_insensitive(
    base_dir: &std::path::Path,
    artist: &str,
    album: &str,
) -> Option<std::path::PathBuf> {
    let artist_lower = artist.to_lowercase();
    let album_lower = album.to_lowercase();

    // Scan base_dir for a matching artist subdirectory
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path.file_name()?.to_string_lossy().to_lowercase();
        if dir_name != artist_lower {
            continue;
        }
        // Found artist dir — now scan for album subdirectory
        let Ok(album_entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for album_entry in album_entries.flatten() {
            let album_path = album_entry.path();
            if !album_path.is_dir() {
                continue;
            }
            let album_dir_name = album_path.file_name()?.to_string_lossy().to_lowercase();
            if album_dir_name == album_lower && has_direct_audio_files(&album_path) {
                return Some(album_path);
            }
        }
    }
    None
}

/// Recursively finds the deepest directory that directly contains audio files.
/// Prefers the most recently modified leaf directory.
/// Maximum recursion depth for [`find_deepest_audio_dir`] (#844).
///
/// User libraries typically fit within 3 levels (`Music/Artist/Album/`); 10
/// gives generous headroom for unusual layouts while keeping the cold-scan
/// cost bounded.
pub(crate) const FIND_DEEPEST_AUDIO_DIR_MAX_DEPTH: u32 = 10;

pub(crate) fn find_deepest_audio_dir(
    dir: &std::path::Path,
    best: &mut Option<(std::time::SystemTime, std::path::PathBuf)>,
    depth: u32,
) {
    if depth >= FIND_DEEPEST_AUDIO_DIR_MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // If this directory directly contains audio files, it's a candidate
            if has_direct_audio_files(&path) {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                            *best = Some((modified, path.clone()));
                        }
                    }
                }
            }
            // Always recurse deeper — there may be nested album directories
            find_deepest_audio_dir(&path, best, depth + 1);
        }
    }
}

/// Checks if the given directory directly contains audio files (non-recursive).
/// Unlike `has_audio_files()`, this does NOT recurse into subdirectories.
///
/// Skips filesystem sidecars (macOS `._*` AppleDouble, `.DS_Store`,
/// Windows `Thumbs.db`, etc.) via `fs_safe::is_filesystem_sidecar`
/// so a directory containing only such sidecars reports `false` even
/// if some of those sidecars end in audio-file extensions like `._track.m4a`.
pub(crate) fn has_direct_audio_files(dir: &std::path::Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !crate::utils::fs_safe::is_filesystem_sidecar(&path) {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("m4a")
                        || ext.eq_ignore_ascii_case("m4v")
                        || ext.eq_ignore_ascii_case("mp4")
                        || ext.eq_ignore_ascii_case("flac")
                        || ext.eq_ignore_ascii_case("mp3")
                        || ext.eq_ignore_ascii_case("ogg")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Count GAMDL warnings indicating tracks were skipped because the
/// requested codec format was unavailable. These appear as stderr lines
/// like "Requested format is not available for song ..." when GAMDL
/// skips tracks in the priority chain without wrapper auth.
pub(crate) fn count_codec_skip_warnings(warnings: &[String]) -> usize {
    warnings
        .iter()
        .filter(|w| {
            let lower = w.to_lowercase();
            lower.contains("format is not available")
                || lower.contains("format not available")
                || lower.contains("requested format")
        })
        .count()
}

pub(crate) fn requested_format_cli_values(options: &GamdlOptions) -> Vec<String> {
    if let Some(priority) = options.song_codec_priority.as_deref() {
        return priority
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
    }

    options
        .song_codec
        .as_ref()
        .map(|codec| vec![codec.to_cli_string().to_string()])
        .unwrap_or_default()
}

pub(crate) fn extract_song_codec_values_from_line(line: &str) -> Vec<String> {
    line.split("SongCodec.")
        .skip(1)
        .filter_map(|part| {
            let value_start = part.find(": '")? + 3;
            let value_rest = &part[value_start..];
            let value_end = value_rest.find('\'')?;
            Some(value_rest[..value_end].to_string())
        })
        .collect()
}

pub(crate) fn describe_song_codec_cli_value(value: &str) -> String {
    SongCodec::from_cli_string(value).map_or_else(
        || value.to_string(),
        |codec| format!("{} [{}]", codec.display_name(), codec.to_cli_string()),
    )
}

pub(crate) fn annotate_unavailable_format_line(line: &str, requested_formats: &[String]) -> String {
    let lower = line.to_lowercase();
    let is_unavailable_format = lower.contains("format is not available")
        || lower.contains("format not available")
        || lower.contains("requested format");
    if !is_unavailable_format || line.contains("Unavailable requested format") {
        return line.to_string();
    }

    let parsed_formats = extract_song_codec_values_from_line(line);
    let formats = if parsed_formats.is_empty() {
        requested_formats.to_vec()
    } else {
        parsed_formats
    };
    if formats.is_empty() {
        return line.to_string();
    }

    let labels: Vec<String> = formats
        .iter()
        .map(|value| describe_song_codec_cli_value(value))
        .collect();
    let detail = if labels.len() == 1 {
        format!("Unavailable requested format: {}", labels[0])
    } else {
        format!(
            "Unavailable requested format candidates: {}",
            labels.join(" -> ")
        )
    };

    format!("{line} ({detail})")
}

/// Build a gap-fill priority chain by removing wrapper-dependent codecs
/// from the original chain. These codecs don't reliably work without
/// wrapper authentication for per-track availability.
///
/// Version-aware (#963, #1002): below GAMDL 3.8 this strips Atmos and
/// AC3, same as always. On a detected `>= 3.8` install (where GAMDL's
/// `/v1/play/assets` endpoint unlocks every non-web codec except ALAC
/// for wrapper-less downloads) only ALAC is stripped — see
/// `SongCodec::is_wrapper_dependent_runtime()`.
///
/// Returns `None` if no non-experimental codecs remain after filtering.
pub(crate) fn build_gapfill_priority_chain(original_chain: &str) -> Option<String> {
    let filtered: Vec<&str> = original_chain
        .split(',')
        .filter(|codec_str| {
            // Parse each codec string and check if it's wrapper-dependent
            if let Some(codec) = SongCodec::from_cli_string(codec_str.trim()) {
                !codec.is_wrapper_dependent_runtime()
            } else {
                // Unknown codec strings are kept (conservative)
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        None
    } else {
        Some(filtered.join(","))
    }
}

/// Count audio files (.m4a, .m4v, .mp4) in the output directory to
/// detect partial download success. Searches recursively through
/// Artist/Album subdirectory structure.
///
/// Migrated to the shared [`crate::utils::fs_walk::walk_dir_depth`]
/// helper (#716 finding #1, v1.0.2 prep). Pre-migration this was an
/// open-coded recursive walker without an explicit depth limit;
/// Inject an advisory suffix (`[Explicit]` / `[Clean]`) into a GAMDL
/// folder template so a companion download lands in the same album
/// folder as the post-enrichment renamed primary (#528).
///
/// Pre-fix the primary's `apply_advisory_suffixes` would rename
/// `Album/` → `Album [Explicit]/` after the primary download
/// finished; the parallel companion GAMDL run, oblivious to that
/// rename, would write its files to a fresh `Album/` sibling. This
/// helper builds the folder template the companion needs to match
/// the post-rename path.
///
/// Substitution rule: replace **every** `{album}` placeholder with
/// `{album} <suffix>`. The placeholder is the natural anchor — the
/// suffix lives on the same path component as the album-name, and
/// existing user templates always position `{album}` as the last
/// path segment (the album folder itself).
///
/// Returns:
/// - `Some(new_template)` — the template was modified.
/// - `None` — the input was missing or didn't contain `{album}`
///   (custom user template that doesn't reference the placeholder;
///   we'd rather leave it alone than guess where to splice the
///   suffix and risk a worse outcome).
#[must_use]
pub(crate) fn inject_advisory_suffix_into_template(
    template: Option<&str>,
    suffix: &str,
) -> Option<String> {
    let template = template?;
    if !template.contains("{album}") {
        return None;
    }
    let new_template = template.replace("{album}", &format!("{{album}} {suffix}"));
    Some(new_template)
}

/// `walk_dir_depth` makes the bound mandatory and enforces the same
/// `read_dir → recurse → filter` shape as the other 4+ walkers in
/// the codebase. Depth 10 matches the convention used by
/// `scan_folder_for_manifests` and `find_dirs_with_ttml` (post-#712).
pub(crate) fn count_audio_files_in_directory(dir: &std::path::Path) -> usize {
    crate::utils::fs_walk::walk_dir_depth(dir, 10, |path| {
        if !path.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(path) {
            return None;
        }
        let ext = path.extension().and_then(|e| e.to_str())?;
        if ext.eq_ignore_ascii_case("m4a")
            || ext.eq_ignore_ascii_case("m4v")
            || ext.eq_ignore_ascii_case("mp4")
        {
            Some(())
        } else {
            None
        }
    })
    .len()
}

/// Decides whether a post-download output integrity report (#1021) should
/// fail the item outright, given the number of files probed and how many
/// of those came back suspect.
///
/// Pure and side-effect-free so it's directly unit-testable.
///
/// Returns `Some(message)` — an actionable error naming the upstream
/// truncated-write bug (gamdl#328) — only when EVERY probed file is
/// suspect (`checked > 0 && suspect_files.len() == checked`). A partial
/// hit (some files fine, some suspect) is not a hard failure here — the
/// caller surfaces those as a prominent warning instead, since most of the
/// album downloaded correctly. `checked == 0` (nothing was probed — e.g.
/// ffprobe unavailable, no M4A files found) is never a failure.
pub(crate) fn integrity_failure_message(checked: usize, suspect_files: &[String]) -> Option<String> {
    if checked > 0 && suspect_files.len() == checked {
        Some(format!(
            "Output integrity check failed: all {checked} probed file(s) appear corrupted or truncated ({}) — this matches a known upstream GAMDL bug (gamdl#328). Try re-downloading.",
            suspect_files.join(", "),
        ))
    } else {
        None
    }
}

/// Unified completion-task timeout (#776).
///
/// Single source of truth for "how long is this download legitimately
/// allowed to take?" Used both for the enrichment-alone wait (pass
/// `companion_tier_count = 0`) and for the wait that includes companion
/// tiers.
///
/// **Why a timeout exists in the first place** (#461): the completion
/// task awaits the enrichment + companion `JoinHandle`s. If any stage
/// deadlocks (e.g. an API client waiting on a response without its own
/// timeout, a `Mutex` held by a dead task), the queue would stall
/// forever. The timeout is a belt-and-braces safety net that force-
/// completes the item after the deadline so the queue keeps moving.
///
/// **Scaling formula**:
/// ```text
///   base
/// + tracks   × per_track_seconds   (drives RG/AcoustID/MB scaling)
/// + tiers    × per_tier_seconds    (each companion variant = a full
///                                   GAMDL re-download + remux)
/// + mvs      × per_mv_seconds      (each MV companion = a separate
///                                   GAMDL invocation)
/// capped at 4 h
/// ```
///
/// **Why the per-track slice was raised from 10 s → 30 s** (#776):
/// the previous 10 s/track allowance was tuned for the documented
/// ~1.5 s/track ReplayGain + AcoustID cost — but ReplayGain decodes
/// the entire audio file via FFmpeg `ebur128`, so live tracks (8-15
/// min each) take 5-10 s of FFmpeg time alone. A 19-track live album
/// brushed the old 13 min budget and timed out mid-stage with
/// "some files may be missing ReplayGain / AcoustID / MusicBrainz
/// tags". The new 30 s/track gives those albums genuine headroom even
/// after #776's parallelisation roughly halves the wall-clock cost.
///
/// **Why MV count is now an input** (#776): each music-video
/// companion is a separate GAMDL invocation (decrypt + remux + tag).
/// Without it counted, an album with many MV companions could blow
/// the budget for the same reason the box-set case originally did.
///
/// | Workload | Old budget | New budget |
/// |---|---|---|
/// | 19 tracks, 0 tiers, 0 MVs | 13 min | 19.5 min |
/// | 19 tracks, 1 tier, 5 MVs | 21 min | 32.5 min |
/// | 50 tracks, 4 tiers, 0 MVs | 18 min (enrich-only) / 50 min (companions) | 67 min |
/// | 200 tracks, 4 tiers, 0 MVs | 75 min | 150 min |
/// | extreme | capped at 4 h | capped at 4 h |
///
/// The companion supervisor's per-process idle watchdog
/// (`gamdl_idle_timeout_minutes`) still kills any individual GAMDL run
/// that genuinely stalls, so this completion-level deadline only needs
/// to cover the *legitimate* total wall-clock cost.
pub(crate) fn compute_total_timeout(
    track_count: usize,
    companion_tier_count: usize,
    mv_count: usize,
) -> std::time::Duration {
    /// 10-minute base timeout (unchanged from #461's original design).
    const BASE_SECS: u64 = 600;
    /// Per-output-file slice. Raised from 10 s → 30 s in #776 to cover
    /// long-form audio (live albums) where ReplayGain's full-file
    /// FFmpeg decode dominates per-track cost.
    const PER_TRACK_SECS: u64 = 30;
    /// Per-tier overhead. 8 min covers a full GAMDL re-download +
    /// mp4decrypt + remux on a typical album over a normal connection.
    /// Generous on purpose: this is a "give up, something is wrong"
    /// threshold, not a target.
    const PER_TIER_SECS: u64 = 8 * 60;
    /// Per-MV-companion overhead. 1 min covers a typical music-video
    /// download (smaller than an album re-download but still a separate
    /// GAMDL invocation per video).
    const PER_MV_SECS: u64 = 60;
    /// Absolute cap — refuses to propose a timeout above 4 h regardless
    /// of how many files are in the directory. Protects against accidental
    /// recursion into a user's full music library if the output-path
    /// check ever mis-resolves.
    const MAX_SECS: u64 = 4 * 3600;

    let scaled = BASE_SECS
        .saturating_add(PER_TRACK_SECS.saturating_mul(track_count as u64))
        .saturating_add(PER_TIER_SECS.saturating_mul(companion_tier_count as u64))
        .saturating_add(PER_MV_SECS.saturating_mul(mv_count as u64));
    let clamped = scaled.min(MAX_SECS);
    std::time::Duration::from_secs(clamped)
}

// ============================================================
// Enrichment stage progress weights (#576 → Phase 3.5b refactor)
// ============================================================
//
// The cumulative weights AND human labels for each per-item processing
// stage now live in the [`super::progress_stages::ProgressStage`] enum
// (one source of truth, label + weight + ordering enforced together,
// covered by unit tests for monotonicity, bound, ellipsis convention,
// and `ALL`-array completeness).
//
// Pre-refactor we had 8 scattered `PROGRESS_*_STAGE: f32` constants
// here plus 9 closure-local `set_label("...", PROGRESS_*)` calls
// inside the enrichment task. Adding a new stage required edits in
// 3+ places and a stale label was an easy bug to miss — exactly the
// pattern that produced the 30-minute "ReplayGain loudness analysis…"
// hang in #712. The enum is the registry; this comment is the
// breadcrumb pointing future readers to it.
// Phase 3.5d: `set_stage_with_label` is used by the enrichment task's
// `set_label` shim. The simpler `set_stage` (which uses the enum's
// canonical label, no override) will be picked up by the companion
// task in Phase 3.5g — re-exported here for that consumer.
#[allow(unused_imports)]
use super::progress_stages::{set_label_only, set_stage, set_stage_with_label, ProgressStage};

// ============================================================
// Manifest writer
// ============================================================

/// Write or update a `.meedyadl` manifest file in the album output directory.
///
/// If a manifest already exists, the new source is merged (append or
/// replace matching platform+URL). Uses atomic write-to-temp-then-rename.
///
/// `primary_codec_id` is the canonical codec-registry ID of the primary
/// download (e.g. `"eac3-atmos"`, `"alac"`). Populates `ManifestSource.codec`
/// and seeds the tier-0 entry of `companion_tiers` so the smart-retry
/// planner can diff codec-suffixed files against the planned variants
/// (#766, Phase 2 of #717/5b).
///
/// The parameter list grew from 6 → 8 across #766 (codec-tier diff) and
/// #596 (Lyricsfile manifest field, populated from disk scan rather than
/// a new param). Acceptable for an internal config-bag function; if it
/// grows further consider a `WriteManifestRequest` struct.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_manifest(
    album_dir: &str,
    urls: &[String],
    album_metadata: Option<&crate::services::apple_music_api::AlbumMetadata>,
    settings: &crate::models::settings::AppSettings,
    downloaded_at: &str,
    cross_platform_urls: Option<std::collections::BTreeMap<String, String>>,
    primary_codec_id: Option<&str>,
    companion_tiers: Option<Vec<Vec<String>>>,
) {
    use crate::models::manifest::{ManifestFile, ManifestSource, ManifestTrack};

    let dir = std::path::Path::new(album_dir);
    if !dir.exists() {
        log::warn!("Manifest: album dir does not exist: {album_dir}");
        return;
    }

    let manifest_path = dir.join("manifest.meedyadl");
    log::info!("Writing manifest to: {}", manifest_path.display());

    // Migration: rename legacy hidden dotfile to visible filename (#447)
    let legacy_path = dir.join(".meedyadl");
    if legacy_path.exists() && !manifest_path.exists() {
        if let Err(e) = std::fs::rename(&legacy_path, &manifest_path) {
            log::warn!("Failed to migrate legacy .meedyadl to manifest.meedyadl: {e}");
        } else {
            log::info!("Migrated legacy .meedyadl → manifest.meedyadl");
        }
    }
    let url = urls.first().cloned().unwrap_or_default();
    if url.is_empty() {
        return;
    }

    // Determine platform from the URL domain using the MediaServiceId enum
    let platform = crate::models::media_service::MediaServiceId::from_url(&url)
        .map_or_else(|| "unknown".to_string(), |svc| svc.to_string());

    // Album-level Lyricsfile presence: `true` when at least one
    // `.lyrics` sidecar was written into `album_dir` (#596). Each
    // ManifestTrack inherits this bool — per-track precision would
    // require knowing the exact GAMDL filename template the user
    // configured, which isn't worth solving in v1. The
    // library-scan smart-retry planner uses this as an album-level
    // "did Step 2g run successfully?" signal.
    let album_has_any_lyricsfile = std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("lyrics"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // Build per-track metadata from AlbumMetadata (if available).
    // codec is intentionally null — the manifest is a metafile for
    // re-downloading, not a prescription of quality/format settings.
    let tracks: Vec<ManifestTrack> = album_metadata
        .map(|meta| {
            meta.tracks
                .iter()
                .map(|t| {
                    // Build individual track URL if we have the album URL + song ID
                    let track_url = if !t.song_id.is_empty() && url.contains("/album/") {
                        // e.g., https://music.apple.com/gb/album/slug/123?i=456
                        Some(format!("{}?i={}", url, t.song_id))
                    } else {
                        None
                    };
                    // Record Apple Music song_id when non-empty so the
                    // duplicate-detection "History" scope (#510) can match
                    // previously-downloaded tracks.
                    let song_id = if t.song_id.is_empty() {
                        None
                    } else {
                        Some(t.song_id.clone())
                    };
                    ManifestTrack {
                        number: t.track_number,
                        disc: t.disc_number,
                        title: t.name.clone(),
                        url: track_url,
                        codec: None,
                        isrc: t.isrc.clone(),
                        song_id,
                        has_lyricsfile: album_has_any_lyricsfile,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // `settings.storefront` is an Apple Music-only concept (region/storefront
    // code injected into `music.apple.com` URLs — see `normalize_apple_music_url`).
    // Non-Apple-Music sources (Spotify, etc.) have no storefront concept of
    // their own; writing the user's globally-configured Apple Music
    // storefront into e.g. a Spotify `ManifestSource` would be misleading —
    // it documents "Platform-specific storefront/region" but the value
    // would have nothing to do with the platform it's attached to (A2 fix).
    let storefront = if platform == crate::models::media_service::MediaServiceId::AppleMusic.to_string()
        && !settings.storefront.is_empty()
    {
        Some(settings.storefront.clone())
    } else {
        None
    };

    let source = ManifestSource {
        platform: platform.to_string(),
        url: url.clone(),
        storefront,
        downloaded_at: downloaded_at.to_string(),
        codec: primary_codec_id.map(str::to_owned),
        last_modified_date: album_metadata.and_then(|m| m.last_modified_date.clone()),
        companion_tiers,
        tracks,
        // Phase 1 (#759): per-stage enrichment record. Initial
        // manifest write happens before enrichment runs, so all
        // stages are absent here; the post-stage hooks will add
        // records as each stage completes (Phase 2 wiring).
        enrichment: None,
        cross_platform_urls,
        // #871: flag personal-library downloads so the Library Scan UI
        // (#717), the duplicate detector (#510), and the future SQLite
        // index (#875) can distinguish library items from catalog items.
        // Library items typically have no catalog counterpart, so
        // metadata flows differently.
        is_library: super::apple_music_api::is_library_url(&url),
    };

    // Read existing manifest or create new
    let mut manifest = if manifest_path.exists() {
        match std::fs::read_to_string(&manifest_path) {
            Ok(contents) => match serde_json::from_str::<ManifestFile>(&contents) {
                Ok(existing) => existing,
                Err(e) => {
                    log::warn!("Failed to parse existing manifest: {e}");
                    ManifestFile::new(source.clone())
                }
            },
            Err(e) => {
                log::warn!("Failed to read existing manifest: {e}");
                ManifestFile::new(source.clone())
            }
        }
    } else {
        ManifestFile::new(source.clone())
    };

    // Merge the source (appends or replaces matching platform+url)
    if manifest_path.exists() {
        manifest.merge_source(source);
    }

    // Atomic write via the shared `utils::atomic_write::atomic_write_json`
    // helper (#716/8, v1.0.5 prep). The helper computes the temp path
    // as `{path}.{existing_ext}.tmp` — for `manifest.meedyadl` that's
    // `manifest.meedyadl.tmp`, exactly matching the pre-migration
    // sibling-temp pattern that #447 used to dodge the dotfile quirks
    // of `Path::with_extension`. No leftover-temp cleanup-on-rename-
    // fail (the helper trusts std::fs::rename), but the rename failure
    // path is rare in practice (same-fs operation) and any orphan
    // .tmp will be overwritten on the next manifest update.
    match crate::utils::atomic_write::atomic_write_json(&manifest_path, &manifest, "manifest") {
        Ok(()) => {
            log::info!("Wrote download manifest to {}", manifest_path.display());
        }
        Err(e) => {
            log::warn!("{e}");
        }
    }
}

/// Rename GAMDL's `Cover.<ext>` to the user's configured cover art name (#448).
///
/// GAMDL hardcodes the static cover art filename as `Cover.jpg` / `Cover.png` /
/// `Cover.raw`. The `cover_art_name` setting controls what the file should be
/// renamed to (e.g., `FrontCover`, `Folder`, or kept as `Cover`).
///
/// ## Three branches (per source/target file presence)
///
/// 1. **`Cover.<ext>` exists, `<stem>.<ext>` absent** → rename source → target.
///    The standard happy path. Atomic via `fs::rename`.
/// 2. **`Cover.<ext>` exists AND `<stem>.<ext>` exists** → **delete source**
///    (#892). This happens when a companion download (or any second GAMDL
///    invocation against the same album folder) writes a fresh `Cover.<ext>`
///    AFTER the primary's rename already produced `<stem>.<ext>`. The two
///    files come from the same Apple Music album URL so the bytes are
///    identical; keeping both wastes disk space and confuses media players
///    that prefer specific cover stems. The user's explicit choice of
///    `cover_art_name` is the source of truth — the renamed file wins.
/// 3. **`Cover.<ext>` absent** → no-op (whether `<stem>.<ext>` exists or not).
pub(crate) fn rename_cover_art(album_dir: &str, target_stem: &str) {
    // If the user wants to keep the default "Cover" name, nothing to do
    if target_stem == "Cover" {
        return;
    }

    let dir = std::path::Path::new(album_dir);
    if !dir.exists() {
        return;
    }

    for ext in &["jpg", "png", "raw"] {
        let old_name = dir.join(format!("Cover.{ext}"));
        let new_name = dir.join(format!("{target_stem}.{ext}"));

        if !old_name.exists() {
            // Branch 3 — nothing to rename.
            continue;
        }

        if new_name.exists() {
            // Branch 2 (#892) — both files present. Companion downloads
            // produce a fresh Cover.<ext> after the primary's rename
            // already produced <stem>.<ext>. Delete the duplicate.
            match std::fs::remove_file(&old_name) {
                Ok(()) => {
                    log::info!(
                        "Cleaned up duplicate Cover.{ext} (kept user-configured {target_stem}.{ext}) in {} — #892",
                        dir.display()
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Failed to clean up duplicate Cover.{ext} in {}: {e}",
                        dir.display()
                    );
                }
            }
            continue;
        }

        // Branch 1 — happy path rename.
        match std::fs::rename(&old_name, &new_name) {
            Ok(()) => {
                log::info!(
                    "Renamed Cover.{ext} → {target_stem}.{ext} in {}",
                    dir.display()
                );
            }
            Err(e) => {
                log::warn!(
                    "Failed to rename Cover.{ext} → {target_stem}.{ext}: {e}"
                );
            }
        }
    }
}

/// Retroactive cleanup pass for the `Cover.<ext>` + `<stem>.<ext>`
/// duplication bug (#892) on **existing** libraries.
///
/// Conservative complement to `rename_cover_art`: where the rename
/// helper has three branches (rename / delete-duplicate / no-op),
/// this helper has only the delete-duplicate branch. A lone
/// `Cover.<ext>` is never touched — the user may have set up their
/// library intentionally with the default stem, and the library-scan
/// path should not silently rename their files.
///
/// Called from the library-scan path (`scan_folder_for_manifests`) so
/// users who downloaded under the pre-#892 code reclaim the wasted
/// disk space on their next scan.
///
/// # Returns
/// Count of duplicate `Cover.<ext>` files removed from `album_dir`.
pub(crate) fn cleanup_duplicate_cover_art(album_dir: &std::path::Path, target_stem: &str) -> usize {
    // No-op when user hasn't configured a non-default stem — under
    // the default `cover_art_name = Cover`, both files would have
    // the same path anyway, no duplication possible.
    if target_stem == "Cover" {
        return 0;
    }
    if !album_dir.is_dir() {
        return 0;
    }

    let mut cleaned = 0usize;
    for ext in &["jpg", "png", "raw"] {
        let cover = album_dir.join(format!("Cover.{ext}"));
        let target = album_dir.join(format!("{target_stem}.{ext}"));
        if cover.exists() && target.exists() {
            match std::fs::remove_file(&cover) {
                Ok(()) => {
                    cleaned += 1;
                    log::info!(
                        "Cleaned duplicate Cover.{ext} (kept {target_stem}.{ext}) in {} — #892",
                        album_dir.display()
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Failed to clean duplicate Cover.{ext} in {}: {e}",
                        album_dir.display()
                    );
                }
            }
        }
    }
    cleaned
}
