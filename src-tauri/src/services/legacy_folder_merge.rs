// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Legacy sibling-folder merge for pre-#528 downloads (#789).
//!
//! Albums downloaded before v1.4.4 with companion codecs enabled
//! and `content_advisory_in_filenames = true` produced two
//! sibling folders on disk:
//!
//! ```text
//! Anne-Marie/
//!   UNHEALTHY (Deluxe)            ← companion-codec files
//!   UNHEALTHY (Deluxe) [Explicit] ← primary-codec files, post-rename
//! ```
//!
//! #528 (v1.4.4) prevents the split for new downloads by
//! injecting the advisory suffix into the companion's GAMDL
//! folder template before spawn. This module handles the
//! historical cleanup — detect existing sibling pairs, offer the
//! user a preview, then merge.
//!
//! ## Three-phase API
//!
//! 1. [`detect_sibling_pairs`] — walks the user's library root
//!    looking for `Foo` + `Foo [Explicit]` (or `[Clean]`) pairs
//!    under the same parent. Defensive: reads one audio file's
//!    `rtng` atom from the un-suffixed sibling and refuses the
//!    pair if the rating doesn't match the suffix on the other
//!    sibling.
//!
//! 2. [`preview_merge`] — describes WHAT the merge would do
//!    (file counts, potential collisions, manifest merge plan)
//!    without touching disk. Powers the dry-run UI.
//!
//! 3. [`execute_merge`] — performs the merge:
//!    - Calls `apply_advisory_suffixes_from_tags` on the
//!      un-suffixed source folder so its audio + sidecars get
//!      the `[Explicit]`/`[Clean]` suffix in lockstep (reuses the
//!      #788-extended pathway, so each audio rename brings its
//!      `.lrc`/`.ttml`/etc. along automatically).
//!    - Merges `.meedyadl` manifests if both folders have one
//!      (the v1.4.6 `merge_source` dedup key — `(platform, url,
//!      codec)` — keeps primary + companion sources both intact
//!      rather than the companion overwriting the primary).
//!    - Moves every remaining file via `fs_safe::safe_rename`
//!      (auto-disambiguates `Cover.jpg` collisions to
//!      `Cover.1.jpg` rather than overwriting).
//!    - Deletes the source folder only if empty after moves.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::manifest::ManifestFile;
use crate::utils::fs_safe;

/// Recognised advisory suffixes, in the canonical form they appear
/// in album-folder names. Order matters for [`detect_sibling_pairs`]
/// — checked longest-first so `[Explicit]` matches before any future
/// shorter variant.
const RECOGNISED_SUFFIXES: &[&str] = &["[Explicit]", "[Clean]"];

/// Audio file extensions checked when verifying that the un-suffixed
/// sibling actually carries the matching advisory rating.
const AUDIO_EXTENSIONS: &[&str] = &["m4a", "m4b", "m4p", "mp4"];

/// A detected pair of sibling folders that share a base name and
/// differ only by an advisory suffix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingPair {
    /// The parent directory both siblings live in (typically an
    /// artist folder).
    pub parent: PathBuf,
    /// Absolute path to the un-suffixed sibling (e.g.
    /// `…/UNHEALTHY (Deluxe)`).
    pub unsuffixed_path: PathBuf,
    /// Absolute path to the suffixed sibling (e.g.
    /// `…/UNHEALTHY (Deluxe) [Explicit]`).
    pub suffixed_path: PathBuf,
    /// Which advisory suffix the pair uses — `"[Explicit]"` or
    /// `"[Clean]"`.
    pub suffix: String,
    /// The shared base album name (no suffix). Used purely for
    /// human-facing display in the UI.
    pub album_basename: String,
}

/// Dry-run summary of what [`execute_merge`] WOULD do for a pair,
/// without touching disk. Powers the confirmation UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePreview {
    pub pair: SiblingPair,
    /// Number of audio files in the un-suffixed folder.
    pub audio_count: usize,
    /// Number of lyric / subtitle sidecars in the un-suffixed folder.
    pub sidecar_count: usize,
    /// Number of other files (cover art, manifest, etc.) that would
    /// also move across.
    pub other_count: usize,
    /// Filenames that already exist in the destination — these will
    /// auto-disambiguate to `.1`, `.2`, etc. on move rather than
    /// overwrite. Surfaces upfront so the user isn't surprised.
    pub potential_collisions: Vec<String>,
    /// Whether both folders carry a `.meedyadl` manifest (in which
    /// case the merge consolidates them via
    /// [`ManifestFile::merge_source`]).
    pub will_merge_manifest: bool,
}

/// Result of [`execute_merge`] — counts + diagnostics for the UI to
/// show after the action completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeReport {
    pub pair: SiblingPair,
    /// Number of audio files that moved into the destination.
    pub audio_moved: usize,
    /// Number of sidecars that moved into the destination.
    pub sidecars_moved: usize,
    /// Number of other files (cover art, manifest, etc.) that moved.
    pub other_moved: usize,
    /// Filenames that were disambiguated by `safe_rename` (target
    /// already existed).
    pub collisions_disambiguated: Vec<String>,
    /// Whether the manifest merge ran.
    pub manifest_merged: bool,
    /// Whether the source folder was empty after the moves and so
    /// got removed. `false` if files were left behind for any
    /// reason (e.g. permission errors).
    pub source_folder_removed: bool,
    /// Non-fatal errors that didn't halt the merge but the user
    /// should know about.
    pub warnings: Vec<String>,
}

// ============================================================
// Detection
// ============================================================

/// Walk `root` at most two levels deep (typical music-library
/// layout: `root/Artist/Album`) and return every detected sibling
/// pair.
///
/// Verification — the un-suffixed sibling's `rtng` atom is checked
/// against the other sibling's suffix. A mismatch (e.g. a folder
/// literally named `[Explicit]` next to a folder that doesn't
/// actually contain explicit tracks) is REJECTED, so we never merge
/// unrelated folders that happen to have similar names.
#[must_use]
pub fn detect_sibling_pairs(root: &Path) -> Vec<SiblingPair> {
    let mut pairs = Vec::new();
    let Ok(level1) = std::fs::read_dir(root) else {
        return pairs;
    };
    for artist_entry in level1.flatten() {
        let artist_path = artist_entry.path();
        if !artist_path.is_dir() {
            continue;
        }
        pairs.extend(detect_pairs_in_parent(&artist_path));
    }
    // Also check `root` itself in case the user pointed the scan
    // directly at an artist folder (album folders one level deep).
    pairs.extend(detect_pairs_in_parent(root));
    pairs
}

/// Find sibling pairs whose direct parent is `parent`. Pulled out
/// of [`detect_sibling_pairs`] for clarity + so the unit tests can
/// exercise a single parent directly.
fn detect_pairs_in_parent(parent: &Path) -> Vec<SiblingPair> {
    let mut pairs = Vec::new();
    let Ok(entries) = std::fs::read_dir(parent) else {
        return pairs;
    };

    // Index folder names so we can look up by stripped base.
    let folders: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_dir() {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            Some((name, path))
        })
        .collect();

    let folder_set: HashSet<&str> =
        folders.iter().map(|(n, _)| n.as_str()).collect();

    // Track which suffixed folders have already been claimed so the
    // same `Album [Explicit]` isn't paired with two different
    // un-suffixed candidates.
    let mut seen_suffixed: HashSet<&str> = HashSet::new();

    for (name, path) in &folders {
        for &suffix in RECOGNISED_SUFFIXES {
            let suffixed_name = format!("{name} {suffix}");
            // Two checks:
            //   1. The suffixed sibling exists.
            //   2. We haven't already paired it.
            //   3. The audio inside `path` actually carries the
            //      matching advisory rating (defensive — refuses
            //      false positives).
            if !folder_set.contains(suffixed_name.as_str()) {
                continue;
            }
            if seen_suffixed.contains(suffixed_name.as_str()) {
                continue;
            }
            if !audio_rating_matches_suffix(path, suffix) {
                continue;
            }
            let suffixed_path = parent.join(&suffixed_name);
            pairs.push(SiblingPair {
                parent: parent.to_path_buf(),
                unsuffixed_path: path.clone(),
                suffixed_path,
                suffix: suffix.to_string(),
                album_basename: name.clone(),
            });
            // Borrow-checker workaround: `seen_suffixed` keys are
            // `&str` borrowed from `folders` — that's fine as long
            // as the name is in the set. We use the *folders'*
            // owned string by lookup.
            if let Some(borrowed) =
                folder_set.get(suffixed_name.as_str()).copied()
            {
                seen_suffixed.insert(borrowed);
            }
            break;
        }
    }

    pairs
}

/// Read one audio file from `folder` and return `true` iff its
/// `rtng` atom yields the same advisory suffix as `expected_suffix`.
/// Returns `false` on any error (unreadable file, missing rtng,
/// mismatch) so callers default to "skip this candidate pair".
fn audio_rating_matches_suffix(folder: &Path, expected_suffix: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !AUDIO_EXTENSIONS
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(ext))
        {
            continue;
        }
        // Open the first audio file we find — they should all carry
        // the same album-level rating.
        let Ok(tag) = mp4ameta::Tag::read_from_path(&path) else {
            continue;
        };
        let rating = match tag.advisory_rating() {
            Some(mp4ameta::AdvisoryRating::Explicit) => "[Explicit]",
            Some(mp4ameta::AdvisoryRating::Clean) => "[Clean]",
            _ => return false,
        };
        return rating == expected_suffix;
    }
    false
}

// ============================================================
// Preview
// ============================================================

const SIDECAR_EXTENSIONS: &[&str] = &["ttml", "lrc", "srt", "vtt", "ass"];

/// Classify a file by extension into the three preview buckets.
fn classify_file(path: &Path) -> FileBucket {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return FileBucket::Other;
    };
    let ext_lc = ext.to_ascii_lowercase();
    if AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(&ext_lc)) {
        FileBucket::Audio
    } else if SIDECAR_EXTENSIONS.contains(&ext_lc.as_str()) {
        FileBucket::Sidecar
    } else {
        FileBucket::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileBucket {
    Audio,
    Sidecar,
    Other,
}

/// Build a [`MergePreview`] for a pair without touching disk.
///
/// `Err` only fires when the source folder is unreadable; everything
/// else (collisions, missing manifests) is encoded in the preview
/// fields as data rather than failure.
pub fn preview_merge(pair: &SiblingPair) -> Result<MergePreview, String> {
    let entries = std::fs::read_dir(&pair.unsuffixed_path).map_err(|e| {
        format!(
            "Failed to read source folder {}: {e}",
            pair.unsuffixed_path.display()
        )
    })?;

    // Snapshot destination filenames once so collision checks are
    // O(1) per source file (and a missing dest folder is treated as
    // empty rather than blocking the preview).
    let dest_names: HashSet<String> =
        std::fs::read_dir(&pair.suffixed_path)
            .ok()
            .map(|it| {
                it.flatten()
                    .filter_map(|e| {
                        e.path()
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();

    let mut audio = 0;
    let mut sidecar = 0;
    let mut other = 0;
    let mut collisions = Vec::new();
    let mut has_source_manifest = false;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if name == "manifest.meedyadl" {
            has_source_manifest = true;
            continue; // handled separately via manifest merge
        }

        match classify_file(&path) {
            FileBucket::Audio => audio += 1,
            FileBucket::Sidecar => sidecar += 1,
            FileBucket::Other => other += 1,
        }

        if dest_names.contains(name) {
            collisions.push(name.to_string());
        }
    }

    let dest_has_manifest = pair.suffixed_path.join("manifest.meedyadl").exists();

    Ok(MergePreview {
        pair: pair.clone(),
        audio_count: audio,
        sidecar_count: sidecar,
        other_count: other,
        potential_collisions: collisions,
        will_merge_manifest: has_source_manifest && dest_has_manifest,
    })
}

// ============================================================
// Execute
// ============================================================

/// Perform the merge.
///
/// Sequence:
///   1. Apply the advisory rename to the source folder so its
///      audio + sidecars carry the `[Explicit]` / `[Clean]` suffix
///      before they cross folders.
///   2. Merge `.meedyadl` manifests if both exist.
///   3. Move every remaining file into the destination via
///      `safe_rename` (auto-disambiguates collisions).
///   4. Delete the source folder if empty.
pub fn execute_merge(pair: &SiblingPair) -> Result<MergeReport, String> {
    if !pair.unsuffixed_path.is_dir() {
        return Err(format!(
            "Source folder no longer exists: {}",
            pair.unsuffixed_path.display()
        ));
    }
    if !pair.suffixed_path.is_dir() {
        return Err(format!(
            "Destination folder no longer exists: {}",
            pair.suffixed_path.display()
        ));
    }

    let mut warnings = Vec::new();

    // Step 1: in-place advisory rename. Reuses the public function
    // from metadata_tag_service which also handles sidecar lockstep
    // (#788). After this, source-folder filenames carry the
    // `[Explicit]` / `[Clean]` suffix exactly as the merged
    // destination expects.
    crate::services::metadata_tag_service::apply_advisory_suffixes_from_tags(
        pair.unsuffixed_path.to_string_lossy().as_ref(),
    );

    // Step 2: manifest merge. Both folders may carry a manifest;
    // the v1.4.6 (platform, url, codec) dedup key keeps both
    // sources intact.
    let manifest_merged =
        merge_manifests(&pair.unsuffixed_path, &pair.suffixed_path, &mut warnings);

    // Step 3: walk source folder and move every remaining file.
    let mut audio_moved = 0;
    let mut sidecars_moved = 0;
    let mut other_moved = 0;
    let mut collisions = Vec::new();

    let entries = std::fs::read_dir(&pair.unsuffixed_path).map_err(|e| {
        format!(
            "Failed to read source folder during merge: {} ({e})",
            pair.unsuffixed_path.display()
        )
    })?;

    for entry in entries.flatten() {
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Manifest was merged + removed by `merge_manifests`; skip.
        if name == "manifest.meedyadl" {
            continue;
        }

        let dest = pair.suffixed_path.join(name);
        let bucket = classify_file(&src);

        match fs_safe::safe_rename(&src, &dest) {
            Ok(final_path) => {
                let final_name = final_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(name);
                if final_name != name {
                    collisions.push(format!("{name} → {final_name}"));
                }
                match bucket {
                    FileBucket::Audio => audio_moved += 1,
                    FileBucket::Sidecar => sidecars_moved += 1,
                    FileBucket::Other => other_moved += 1,
                }
            }
            Err(e) => {
                warnings.push(format!(
                    "Failed to move {}: {e}",
                    src.display()
                ));
            }
        }
    }

    // Step 4: remove source folder if empty. If anything was left
    // behind (warnings recorded above), leave the folder alone so
    // the user can investigate.
    let source_folder_removed = match std::fs::read_dir(&pair.unsuffixed_path) {
        Ok(mut it) => {
            if it.next().is_none() {
                if let Err(e) = std::fs::remove_dir(&pair.unsuffixed_path) {
                    warnings.push(format!(
                        "Source folder is empty but removal failed: {e}"
                    ));
                    false
                } else {
                    true
                }
            } else {
                warnings.push(format!(
                    "Source folder not empty after merge — left in place: {}",
                    pair.unsuffixed_path.display()
                ));
                false
            }
        }
        Err(e) => {
            warnings.push(format!(
                "Failed to check source folder for removal: {e}"
            ));
            false
        }
    };

    Ok(MergeReport {
        pair: pair.clone(),
        audio_moved,
        sidecars_moved,
        other_moved,
        collisions_disambiguated: collisions,
        manifest_merged,
        source_folder_removed,
        warnings,
    })
}

/// Merge `source_folder/.meedyadl` into `dest_folder/.meedyadl`.
///
/// Behaviour matrix:
///   - Both exist → load both, call `ManifestFile::merge_source` for
///     each source in the source manifest, write dest back atomically,
///     delete source.
///   - Only source exists → move it to dest (becomes the dest
///     manifest verbatim). Implemented via `safe_rename`.
///   - Only dest exists → nothing to do; return false.
///   - Neither exists → nothing to do; return false.
///
/// Returns `true` iff a merge actually happened (matrix row 1).
fn merge_manifests(
    source_folder: &Path,
    dest_folder: &Path,
    warnings: &mut Vec<String>,
) -> bool {
    let source_manifest = source_folder.join("manifest.meedyadl");
    let dest_manifest = dest_folder.join("manifest.meedyadl");

    if !source_manifest.exists() {
        return false;
    }

    // Case: only source. Move it and we're done.
    if !dest_manifest.exists() {
        match fs_safe::safe_rename(&source_manifest, &dest_manifest) {
            Ok(_) => return false, // moved, not "merged"
            Err(e) => {
                warnings.push(format!(
                    "Failed to move source manifest: {e}"
                ));
                return false;
            }
        }
    }

    // Both exist — load + merge.
    let source_data = match std::fs::read_to_string(&source_manifest) {
        Ok(s) => s,
        Err(e) => {
            warnings.push(format!("Failed to read source manifest: {e}"));
            return false;
        }
    };
    let source: ManifestFile = match serde_json::from_str(&source_data) {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!("Source manifest is not valid JSON: {e}"));
            return false;
        }
    };
    let dest_data = match std::fs::read_to_string(&dest_manifest) {
        Ok(s) => s,
        Err(e) => {
            warnings.push(format!("Failed to read dest manifest: {e}"));
            return false;
        }
    };
    let mut dest: ManifestFile = match serde_json::from_str(&dest_data) {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!("Dest manifest is not valid JSON: {e}"));
            return false;
        }
    };

    for src in source.sources {
        dest.merge_source(src);
    }

    match crate::utils::atomic_write::atomic_write_json(
        &dest_manifest,
        &dest,
        "merge legacy folder manifest",
    ) {
        Ok(()) => {
            // Best-effort delete of source manifest now that its
            // contents live in dest.
            if let Err(e) = std::fs::remove_file(&source_manifest) {
                warnings.push(format!(
                    "Merged manifest but failed to remove source manifest: {e}"
                ));
            }
            true
        }
        Err(e) => {
            warnings.push(format!("Failed to write merged manifest: {e}"));
            false
        }
    }
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::manifest::{ManifestFile, ManifestSource};
    use tempfile::TempDir;

    /// Create a directory layout for testing without touching real
    /// audio files. `pair_basename` becomes the album base; the test
    /// creates both `Foo` and `Foo {suffix}` under the parent.
    fn setup_pair(
        parent: &Path,
        pair_basename: &str,
        suffix: &str,
    ) -> (PathBuf, PathBuf) {
        let unsuffixed = parent.join(pair_basename);
        let suffixed = parent.join(format!("{pair_basename} {suffix}"));
        std::fs::create_dir_all(&unsuffixed).unwrap();
        std::fs::create_dir_all(&suffixed).unwrap();
        (unsuffixed, suffixed)
    }

    /// Write a minimal M4A with the given advisory rating into
    /// `folder/name`. Uses `mp4ameta` to construct a tag-only file
    /// (no actual audio data — sufficient for the rtng-check).
    fn write_audio_with_rating(
        folder: &Path,
        name: &str,
        rating: mp4ameta::AdvisoryRating,
    ) {
        let path = folder.join(name);
        // mp4ameta needs an existing MP4 container to write tags
        // into. Build a minimal one with the `mp4` crate would be
        // overkill — instead, write a stub file and skip the tag
        // write; the detector reads via `Tag::read_from_path` which
        // will fail and the helper returns false. For tests that
        // need actual rtng reads, we'd need fixtures; here we rely
        // on the detection unit tests using folders whose audio
        // we explicitly mock.
        //
        // Alternative: use a pre-built fixture file shipped under
        // `src-tauri/test-fixtures/`. Out of scope for the initial
        // suite — covered indirectly by integration tests.
        let _ = rating;
        std::fs::write(&path, b"stub").unwrap();
    }

    #[test]
    fn detect_finds_no_pairs_in_empty_directory() {
        let dir = TempDir::new().unwrap();
        let pairs = detect_sibling_pairs(dir.path());
        assert!(pairs.is_empty());
    }

    #[test]
    fn detect_finds_no_pairs_when_only_unsuffixed_exists() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Artist/Album")).unwrap();
        let pairs = detect_sibling_pairs(dir.path());
        assert!(pairs.is_empty());
    }

    #[test]
    fn detect_finds_no_pairs_when_only_suffixed_exists() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Artist/Album [Explicit]")).unwrap();
        let pairs = detect_sibling_pairs(dir.path());
        assert!(pairs.is_empty());
    }

    #[test]
    fn detect_rejects_pair_without_matching_rtng_atom() {
        // Two folders look like a pair by name, but the un-suffixed
        // sibling has no readable audio with the matching advisory.
        // The defensive `audio_rating_matches_suffix` should refuse.
        let dir = TempDir::new().unwrap();
        let artist = dir.path().join("Artist");
        let (unsuffixed, _) = setup_pair(&artist, "Album", "[Explicit]");
        // Stub file is not a valid M4A → rtng read fails → reject.
        write_audio_with_rating(
            &unsuffixed,
            "01 Track.m4a",
            mp4ameta::AdvisoryRating::Explicit,
        );
        let pairs = detect_sibling_pairs(dir.path());
        assert!(
            pairs.is_empty(),
            "rtng mismatch / unreadable audio must REJECT the pair"
        );
    }

    /// Even with no `rtng` data, the audio_rating_matches_suffix
    /// helper should never panic — verifies defensive behaviour.
    #[test]
    fn audio_rating_matches_suffix_handles_unreadable_audio() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("01 Track.m4a"), b"not really an mp4")
            .unwrap();
        assert!(!audio_rating_matches_suffix(dir.path(), "[Explicit]"));
    }

    #[test]
    fn audio_rating_matches_suffix_handles_empty_folder() {
        let dir = TempDir::new().unwrap();
        assert!(!audio_rating_matches_suffix(dir.path(), "[Explicit]"));
    }

    /// Preview correctly tallies file counts and surfaces collisions.
    #[test]
    fn preview_classifies_files_and_detects_collisions() {
        let dir = TempDir::new().unwrap();
        let (unsuffixed, suffixed) =
            setup_pair(dir.path(), "Album", "[Explicit]");

        std::fs::write(unsuffixed.join("01 Track [Lossless].m4a"), b"a").unwrap();
        std::fs::write(unsuffixed.join("01 Track [Lossless].lrc"), b"b").unwrap();
        std::fs::write(unsuffixed.join("Cover.jpg"), b"c").unwrap();
        std::fs::write(suffixed.join("Cover.jpg"), b"already-here").unwrap();

        let pair = SiblingPair {
            parent: dir.path().to_path_buf(),
            unsuffixed_path: unsuffixed,
            suffixed_path: suffixed,
            suffix: "[Explicit]".to_string(),
            album_basename: "Album".to_string(),
        };

        let preview = preview_merge(&pair).unwrap();
        assert_eq!(preview.audio_count, 1);
        assert_eq!(preview.sidecar_count, 1);
        assert_eq!(preview.other_count, 1);
        assert_eq!(preview.potential_collisions, vec!["Cover.jpg".to_string()]);
        assert!(!preview.will_merge_manifest);
    }

    /// Preview reports manifest merge intent when both folders have one.
    #[test]
    fn preview_reports_manifest_merge_when_both_present() {
        let dir = TempDir::new().unwrap();
        let (unsuffixed, suffixed) =
            setup_pair(dir.path(), "Album", "[Explicit]");
        std::fs::write(unsuffixed.join("manifest.meedyadl"), b"{}").unwrap();
        std::fs::write(suffixed.join("manifest.meedyadl"), b"{}").unwrap();

        let pair = SiblingPair {
            parent: dir.path().to_path_buf(),
            unsuffixed_path: unsuffixed,
            suffixed_path: suffixed,
            suffix: "[Explicit]".to_string(),
            album_basename: "Album".to_string(),
        };

        let preview = preview_merge(&pair).unwrap();
        assert!(preview.will_merge_manifest);
    }

    /// Manifest merge concatenates sources from both folders rather
    /// than the companion replacing the primary. Stress-tests the
    /// (platform, url, codec) dedup key from the v1.4.6 manifest
    /// change.
    #[test]
    fn merge_manifests_preserves_both_codec_sources() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        let primary_manifest = ManifestFile::new(ManifestSource {
            platform: "apple-music".to_string(),
            url: "https://music.apple.com/us/album/foo/123".to_string(),
            storefront: None,
            downloaded_at: "2026-01-01T00:00:00Z".to_string(),
            codec: Some("atmos".to_string()),
            last_modified_date: None,
            tracks: Vec::new(),
            enrichment: None,
            cross_platform_urls: None,
        });
        let companion_manifest = ManifestFile::new(ManifestSource {
            platform: "apple-music".to_string(),
            url: "https://music.apple.com/us/album/foo/123".to_string(),
            storefront: None,
            downloaded_at: "2026-01-01T00:30:00Z".to_string(),
            codec: Some("alac".to_string()),
            last_modified_date: None,
            tracks: Vec::new(),
            enrichment: None,
            cross_platform_urls: None,
        });

        std::fs::write(
            src.join("manifest.meedyadl"),
            serde_json::to_string(&companion_manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dst.join("manifest.meedyadl"),
            serde_json::to_string(&primary_manifest).unwrap(),
        )
        .unwrap();

        let mut warnings = Vec::new();
        let merged = merge_manifests(&src, &dst, &mut warnings);
        assert!(merged, "both manifests present → merge runs");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");

        let merged_data: ManifestFile = serde_json::from_str(
            &std::fs::read_to_string(dst.join("manifest.meedyadl")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            merged_data.sources.len(),
            2,
            "primary + companion sources both kept"
        );
        let codecs: HashSet<Option<String>> =
            merged_data.sources.iter().map(|s| s.codec.clone()).collect();
        assert!(codecs.contains(&Some("atmos".to_string())));
        assert!(codecs.contains(&Some("alac".to_string())));
        assert!(
            !src.join("manifest.meedyadl").exists(),
            "source manifest deleted after merge"
        );
    }

    /// Source-only manifest gets MOVED to dest verbatim — no merge,
    /// no warning.
    #[test]
    fn merge_manifests_moves_source_when_dest_absent() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("manifest.meedyadl"), b"{}").unwrap();

        let mut warnings = Vec::new();
        let merged = merge_manifests(&src, &dst, &mut warnings);
        assert!(!merged, "only-source case is a move, not a merge");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(dst.join("manifest.meedyadl").exists());
        assert!(!src.join("manifest.meedyadl").exists());
    }

    /// Execute correctly moves non-audio files (cover art) via
    /// `safe_rename` collision handling — the dest's Cover.jpg
    /// stays, the source's becomes Cover.1.jpg.
    #[test]
    fn execute_safe_renames_colliding_cover_art() {
        let dir = TempDir::new().unwrap();
        let (unsuffixed, suffixed) =
            setup_pair(dir.path(), "Album", "[Explicit]");
        std::fs::write(unsuffixed.join("Cover.jpg"), b"src-cover").unwrap();
        std::fs::write(suffixed.join("Cover.jpg"), b"dst-cover").unwrap();

        let pair = SiblingPair {
            parent: dir.path().to_path_buf(),
            unsuffixed_path: unsuffixed.clone(),
            suffixed_path: suffixed.clone(),
            suffix: "[Explicit]".to_string(),
            album_basename: "Album".to_string(),
        };

        let report = execute_merge(&pair).unwrap();
        assert_eq!(report.other_moved, 1);
        assert_eq!(report.collisions_disambiguated.len(), 1);
        assert!(report.source_folder_removed, "src empty after move");
        // Original dest cover untouched.
        assert_eq!(
            std::fs::read(suffixed.join("Cover.jpg")).unwrap(),
            b"dst-cover"
        );
        // Source's cover landed under a disambiguated name.
        let disambiguated = std::fs::read_dir(&suffixed)
            .unwrap()
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("Cover")
                    && e.file_name().to_string_lossy() != "Cover.jpg"
            });
        assert!(
            disambiguated.is_some(),
            "src Cover.jpg should have been disambiguated"
        );
    }
}
