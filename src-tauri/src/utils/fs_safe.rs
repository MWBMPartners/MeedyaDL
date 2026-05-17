// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Collision-proof filesystem helpers.
// ===================================
//
// Centralises the "never silently overwrite a different file" invariant
// that applies to every rename / copy / extract path in the app — not
// just music-video subtitle sidecars (#483) where it was originally
// introduced.
//
// ## Why this module exists
//
// `std::fs::rename` on **Unix** silently overwrites an existing
// destination, and on **Windows** it errors out — so every naive
// rename is either a data-loss risk or a platform-inconsistent bug.
// Likewise `std::fs::write` / `std::fs::copy` can clobber existing
// files without warning when the destination path happens to match.
//
// Real cases where the destination could carry *different* content:
//   - advisory / codec rename re-runs where the metadata updated
//     (e.g. track flipped from explicit → clean between runs)
//   - album folders in the same artist directory that happen to end
//     up sharing a post-suffix name
//   - companion-codec downloads landing next to a primary that
//     already has the suffixed name from a previous session
//   - animated-artwork hide rename on Linux where `.FrontCover.mp4`
//     already exists from a previous run
//   - API JSON dumps where the album name sanitises to the same stem
//
// ## Public surface
//
// - `same_file(a, b)`  — canonicalised same-path check; tolerates
//   symlinks, case-insensitive filesystems, and redundant `./`.
// - `resolve_non_clobbering_path(dir, name)` — returns a free path
//   by appending `.1`, `.2`, ... before the extension until a free
//   slot is found (up to 100 tries).
// - `safe_rename(src, dest)` — rename `src` to `dest`, or to an
//   auto-disambiguated sibling if `dest` is already taken by a
//   different file. Returns the final path the file now lives at.
//   Idempotent when `src == dest`.

use std::path::{Path, PathBuf};

/// Two paths refer to the same on-disk file.
///
/// Uses canonicalisation so that symlinks, case-insensitive filesystems,
/// and `./` redundancy all collapse correctly. When either path cannot
/// be canonicalised (e.g. the destination doesn't exist yet) we fall
/// back to lexical equality — safe side of the invariant, since a
/// non-existent destination cannot collide with anything.
#[must_use]
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ac), Ok(bc)) => ac == bc,
        _ => a == b,
    }
}

/// Return a path in `dir` that is guaranteed not to overwrite any
/// existing file.
///
/// If `{dir}/{name}` is free, that path is returned verbatim.
/// Otherwise appends `.1`, `.2`, ... before the final extension until
/// a free slot is found. Caps at 100 attempts — beyond that the input
/// path is returned as-is (the caller's own existence guard will catch
/// it). 100 collisions for one logical stem is pathological and
/// implies something else is wrong.
#[must_use]
pub fn resolve_non_clobbering_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    // Split `name` into (stem, ext) so the numeric suffix lives
    // before the extension: `foo.vtt` → `foo.1.vtt`, not `foo.vtt.1`.
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s, Some(e)),
        None => (name, None),
    };

    for n in 1..100 {
        let alt_name = match ext {
            Some(e) => format!("{stem}.{n}.{e}"),
            None => format!("{stem}.{n}"),
        };
        let alt = dir.join(&alt_name);
        if !alt.exists() {
            return alt;
        }
    }

    candidate
}

/// Rename `src` → `dest`, never overwriting a different file.
///
/// Semantics:
/// Classification of a candidate filename — emitted by
/// [`classify_filename`] and consumed by the activity-log emitter
/// when a write site wants to flag (or quarantine) suspicious
/// outputs from upstream tools (#532).
///
/// The variants are ordered from "clearly fine" to "clearly
/// degenerate" so the caller can decide whether to proceed, warn,
/// or quarantine based on the severity. None of the variants
/// mutate state — `classify_filename` is a pure read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilenameClassification {
    /// The name looks like real content. No action required.
    Ok,
    /// The name is suspicious but not a clear corruption — e.g.
    /// it contains an `Unknown *` sentinel from GAMDL's fallback
    /// template path. Warn-worthy; the caller should consider
    /// surfacing the source URL so the user can investigate.
    Suspicious { reason: String },
    /// The name is clearly broken — empty stem, punctuation-only,
    /// `[Unknown]` marker, etc. Writing it would either silently
    /// overwrite another degenerate file or clutter the library
    /// with files the user cannot identify. The caller should
    /// quarantine the write to a disambiguated fallback name.
    Degenerate { reason: String },
}

impl FilenameClassification {
    /// Convenience: true when the name is at least suspicious
    /// (i.e. not `Ok`). Useful for callers that want a one-line
    /// "should I warn?" check rather than matching every arm.
    #[must_use]
    pub fn is_problem(&self) -> bool {
        !matches!(self, Self::Ok)
    }
}

/// Classify a filename for the degenerate-name guard (#532).
///
/// Inspects the leaf filename's stem (everything before the
/// last `.`) and returns a [`FilenameClassification`]. Caller
/// is responsible for deciding what to do with the classification
/// — the helper has no side effects.
///
/// ## Rejection rules
///
/// - **Empty stem** (`".mp4"`, `".m4a"`) → `Degenerate`. A file
///   whose stem is just the extension is almost always the result
///   of a template substitution where the title and disc/track
///   placeholders all resolved to empty strings.
/// - **Punctuation-only stem** (`"-.mp4"`, `"--.mp4"`, `"_.mp4"`,
///   `"   .mp4"`) → `Degenerate`. Same root cause; the visible
///   characters are template separators rather than real content.
/// - **Stem contains `[Unknown]`** → `Degenerate`. This was the
///   exact symptom of #527's RC blocker; GAMDL's legacy
///   `no_album_folder_template` would resolve to literal
///   `[Unknown]` text in some configurations.
/// - **Stem contains an `Unknown *` sentinel** (`Unknown Album`,
///   `Unknown Title`, `Unknown Artist`, etc.) → `Suspicious`.
///   GAMDL's fallback values for missing tags; not strictly
///   broken, but worth flagging so the user can verify they
///   intended the no-album path.
/// - Anything else → `Ok`.
///
/// Case-insensitive on the `Unknown` markers. The `[Unknown]`
/// brackets are matched as a literal substring; any other
/// bracketed sentinel a future GAMDL version might introduce
/// won't trigger until added to this list.
#[must_use]
pub fn classify_filename(path: &Path) -> FilenameClassification {
    // Operate on the leaf filename — parent components like
    // `/Music/[Unknown]/track.mp4` should be caught by
    // `classify_path_components` (separate helper) rather than
    // this leaf-only check.
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return FilenameClassification::Degenerate {
            reason: "filename is empty or non-UTF-8".to_string(),
        };
    };

    // Compute the stem ourselves rather than relying on
    // `Path::file_stem()`. Rust treats a leading dot as a hidden-
    // file marker, so `.mp4` returns stem `".mp4"` and extension
    // `None` — exactly the opposite of what we want here. The
    // degenerate-name guard needs `.mp4` to be classified as an
    // empty stem with extension `mp4`, so we split on the last
    // `.` manually. The dot-prefixed-hidden case (`.FrontCover.mp4`
    // from the Linux hide-file path) still has a non-empty stem
    // because it contains more than one `.`.
    let stem = match filename.rsplit_once('.') {
        Some((before_ext, _ext)) => before_ext,
        // No `.` at all — the whole filename is the stem.
        None => filename,
    };

    // Empty stem — `.mp4` / `.jpg` etc. A file whose name is just
    // an extension is almost always the result of a template
    // substitution where every placeholder resolved to empty.
    if stem.is_empty() {
        return FilenameClassification::Degenerate {
            reason: format!("empty stem in '{filename}'"),
        };
    }

    // Punctuation/whitespace-only stem. After stripping all
    // punctuation + whitespace, the stem should still carry
    // some alphanumeric content. The character set covers the
    // hyphen / underscore / dot / space / parenthesis shapes
    // that show up in real degenerate filenames.
    let punctuation_only = stem.chars().all(|c| {
        c.is_whitespace() || matches!(c, '-' | '_' | '.' | '(' | ')' | '[' | ']' | ',' | ';')
    });
    if punctuation_only {
        return FilenameClassification::Degenerate {
            reason: format!("stem '{stem}' is punctuation/whitespace only"),
        };
    }

    // Literal `[Unknown]` marker — the #527 RC-blocker symptom.
    // Case-insensitive on the word; brackets must be literal.
    let lower = stem.to_ascii_lowercase();
    if lower.contains("[unknown]") {
        return FilenameClassification::Degenerate {
            reason: format!("stem '{stem}' contains the [Unknown] marker (#527)"),
        };
    }

    // GAMDL `Unknown *` sentinels for missing tags. These are
    // legal output of the fallback template path but still worth
    // flagging — the user may have a misconfigured template.
    for sentinel in &[
        "unknown album",
        "unknown title",
        "unknown artist",
        "unknown composer",
        "unknown year",
    ] {
        if lower.contains(sentinel) {
            return FilenameClassification::Suspicious {
                reason: format!("stem '{stem}' contains GAMDL fallback sentinel '{sentinel}'"),
            };
        }
    }

    FilenameClassification::Ok
}

/// Same as [`classify_filename`] but walks every path component
/// (not just the leaf), so `/Music/[Unknown]/track.mp4` is
/// caught even though `track.mp4` itself is fine.
///
/// Returns the **worst** classification across all components,
/// with the offending component name embedded in the reason.
/// Empty components (e.g. a trailing `/`) are skipped.
#[must_use]
pub fn classify_path_components(path: &Path) -> FilenameClassification {
    let mut worst = FilenameClassification::Ok;
    for component in path.components() {
        let std::path::Component::Normal(part) = component else {
            continue;
        };
        // Build a "fake file path" carrying just this component so
        // we can reuse the leaf classifier. Wrapping is cheaper
        // than duplicating the rules.
        let synthetic = Path::new(part);
        let cls = classify_filename(synthetic);
        match (&worst, &cls) {
            (FilenameClassification::Ok, _) => worst = cls,
            (FilenameClassification::Suspicious { .. }, FilenameClassification::Degenerate { .. }) => {
                worst = cls;
            }
            _ => {}
        }
    }
    worst
}

/// - If `src` does not exist, returns `Err`.
/// - If `src` and `dest` resolve to the same file (canonicalised),
///   returns `Ok(dest)` without touching the filesystem.
/// - If `dest` is free, performs `fs::rename` and returns `Ok(dest)`.
/// - If `dest` is taken, picks an auto-disambiguated sibling via
///   `resolve_non_clobbering_path` and renames to that, returning
///   the actual final path.
///
/// Use this anywhere the caller would previously have written
/// `std::fs::rename(src, dest)` without first checking `dest.exists()`.
/// The returned path MUST be captured by the caller when the
/// disambiguator was exercised — the file may not be where they
/// originally asked.
pub fn safe_rename(src: &Path, dest: &Path) -> std::io::Result<PathBuf> {
    if !src.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("source does not exist: {}", src.display()),
        ));
    }

    // Same-file guard: a rename onto itself is a no-op.
    if same_file(src, dest) {
        return Ok(dest.to_path_buf());
    }

    let final_dest = if dest.exists() {
        let Some(parent) = dest.parent() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination has no parent directory",
            ));
        };
        let Some(name) = dest.file_name().and_then(|n| n.to_str()) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination has no filename",
            ));
        };
        let alt = resolve_non_clobbering_path(parent, name);
        log::warn!(
            "safe_rename: {} already exists — disambiguating to {}",
            dest.display(),
            alt.display()
        );
        alt
    } else {
        dest.to_path_buf()
    };

    std::fs::rename(src, &final_dest)?;
    Ok(final_dest)
}

/// Rename `src` → `dest` only when `dest` is free. If `dest` exists,
/// does nothing and returns `Ok(false)`; the caller decides whether
/// to treat that as success or failure.
///
/// Useful for whole-directory renames (e.g. the album-folder advisory
/// rename) where "auto-disambiguate to `Album [Explicit].1`" is the
/// wrong semantic — two different albums should NOT get merged under
/// one suffixed name.
pub fn rename_if_dest_free(src: &Path, dest: &Path) -> std::io::Result<bool> {
    if same_file(src, dest) {
        return Ok(true);
    }
    if dest.exists() {
        return Ok(false);
    }
    std::fs::rename(src, dest)?;
    Ok(true)
}

/// Write `contents` to `{dir}/{name}`, choosing an auto-disambiguated
/// sibling name if the original path is already taken by a different
/// file.
///
/// Returns the actual path written. Does NOT perform an atomic
/// write-then-rename — callers that need crash safety should combine
/// this with the temp-file pattern themselves.
pub fn write_non_clobbering(
    dir: &Path,
    name: &str,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<PathBuf> {
    let path = resolve_non_clobbering_path(dir, name);
    std::fs::write(&path, contents)?;
    Ok(path)
}

/// Write `contents` to `{dir}/{name}` with content-aware deduplication.
///
/// Behaviour:
/// - **Target absent** → normal write; returns the target path.
/// - **Target present + identical bytes** → no-op; returns the existing
///   path. Caller can treat this as "already saved". Avoids the
///   `.1`, `.2`, ... sprawl when the same content is written repeatedly
///   (e.g., re-downloading an album whose Apple Music API response
///   hasn't changed).
/// - **Target present + different bytes** → disambiguate via
///   `resolve_non_clobbering_path` and write the new content to
///   `{name}.1.{ext}` (or further). Preserves the prior file.
///
/// Use this instead of `write_non_clobbering` when the write is
/// idempotent-in-content (the common case for API response dumps,
/// cached metadata, and other deterministic outputs) so the disk
/// doesn't accumulate redundant duplicates on every re-run.
///
/// # Errors
/// Returns any I/O error from reading the existing file or writing the
/// new one.
pub fn write_deduped(
    dir: &Path,
    name: &str,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    let bytes = contents.as_ref();

    if path.exists() {
        // Compare content byte-for-byte. For the API-dump use case the
        // files are on the order of 10–100 KB, so a synchronous read
        // here is cheaper than the wasted write + eventual disk bloat.
        match std::fs::read(&path) {
            Ok(existing) if existing.as_slice() == bytes => {
                // Identical — no-op.
                return Ok(path);
            }
            Ok(_) | Err(_) => {
                // Different content, or unreadable (in which case the
                // old file is effectively garbage anyway). Fall through
                // to the disambiguating write so the old file is
                // preserved on its original path.
                let alt = resolve_non_clobbering_path(dir, name);
                std::fs::write(&alt, bytes)?;
                return Ok(alt);
            }
        }
    }

    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Returns `true` if `path`'s file name is a known filesystem-sidecar
/// artifact that must be excluded from audio-file enrichment walkers
/// (#577).
///
/// # Why this exists
///
/// When MeedyaDL's output path lives on a non-native filesystem
/// (exFAT / FAT32 / HFS on an external drive, SMB / NFS share, etc.),
/// the host OS can create hidden sidecar files alongside every real
/// file to store metadata the underlying filesystem can't natively
/// represent:
///
/// - **macOS** creates `._{filename}` **AppleDouble** files on every
///   non-HFS+/APFS volume. These store extended attributes, Finder
///   tags, resource forks, and the `com.apple.quarantine` flag. They
///   share the audio file's extension (`._track.m4a`) but contain
///   binary metadata, not audio.
/// - **macOS** Finder writes `.DS_Store` files in every directory it
///   visits.
/// - **Windows** creates `Thumbs.db` thumbnail caches and
///   `desktop.ini` folder-customisation files.
///
/// Without filtering, every enrichment walker that iterates `.m4a` /
/// `.mp4` / `.m4v` / `.flac` / `.mp3` files (codec detection,
/// ReplayGain, AcoustID, BPM, subtitle embed, advisory rename,
/// codec-suffix rename, etc.) processes the AppleDouble files too —
/// running `ffprobe` on a non-audio binary, erroring, logging a warning,
/// and contributing hundreds of noisy lines per album to the activity
/// log (on a 100-track album: 100 spurious failures). Captured live
/// 2026-04-23 on a 200-track Beethoven box set download to an exFAT
/// USB drive.
///
/// This predicate is the single point of truth for the "what should
/// audio walkers ignore" question so that future sidecar patterns
/// (Linux `.fuse_hidden*`, network-share `@eaDir/`, etc.) can be
/// added in one place.
///
/// # Returns
///
/// `true` when the basename matches one of the known sidecar patterns,
/// `false` otherwise (including when the path has no basename — those
/// callers should discard the path on their own path-is-valid check
/// before reaching here).
#[must_use]
pub fn is_filesystem_sidecar(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // macOS AppleDouble on non-APFS/HFS+ filesystems (exFAT, FAT32,
    // SMB shares, etc.). Any file whose basename starts with `._`
    // is an AppleDouble sidecar — by convention, real filenames
    // never start with `._` (dot-underscore is reserved).
    if name.starts_with("._") {
        return true;
    }

    // Known non-AppleDouble metadata sidecars across platforms.
    matches!(
        name,
        ".DS_Store"         // macOS Finder folder metadata
            | "Thumbs.db"   // Windows XP/Vista thumbnail cache
            | "thumbs.db"   // case-insensitive filesystems
            | "desktop.ini" // Windows folder-customisation metadata
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_returns_original_when_free() {
        let dir = tempfile::tempdir().unwrap();
        let p = resolve_non_clobbering_path(dir.path(), "track.m4a");
        assert_eq!(p, dir.path().join("track.m4a"));
    }

    #[test]
    fn resolve_appends_numeric_suffix_before_extension() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("track.m4a"), "first").unwrap();
        let p = resolve_non_clobbering_path(dir.path(), "track.m4a");
        assert_eq!(p, dir.path().join("track.1.m4a"));
    }

    #[test]
    fn resolve_steps_past_multiple_collisions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.srt"), "").unwrap();
        fs::write(dir.path().join("a.1.srt"), "").unwrap();
        fs::write(dir.path().join("a.2.srt"), "").unwrap();
        let p = resolve_non_clobbering_path(dir.path(), "a.srt");
        assert_eq!(p, dir.path().join("a.3.srt"));
    }

    #[test]
    fn resolve_handles_names_without_extension() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("noext"), "").unwrap();
        let p = resolve_non_clobbering_path(dir.path(), "noext");
        assert_eq!(p, dir.path().join("noext.1"));
    }

    #[test]
    fn same_file_detects_self() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.ttml");
        fs::write(&p, "").unwrap();
        assert!(same_file(&p, &p));
    }

    #[test]
    fn same_file_detects_distinct_paths() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, "").unwrap();
        fs::write(&b, "").unwrap();
        assert!(!same_file(&a, &b));
    }

    #[test]
    fn safe_rename_free_destination() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.m4a");
        let dest = dir.path().join("dest.m4a");
        fs::write(&src, "content").unwrap();
        let final_path = safe_rename(&src, &dest).unwrap();
        assert_eq!(final_path, dest);
        assert!(dest.exists());
        assert!(!src.exists());
    }

    #[test]
    fn safe_rename_disambiguates_when_dest_taken() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.m4a");
        let dest = dir.path().join("dest.m4a");
        fs::write(&src, "NEW content").unwrap();
        fs::write(&dest, "OLD content — must not be clobbered").unwrap();

        let final_path = safe_rename(&src, &dest).unwrap();

        // Source moved, original destination preserved.
        assert!(!src.exists());
        assert_eq!(
            fs::read_to_string(&dest).unwrap(),
            "OLD content — must not be clobbered"
        );
        // New content is under a disambiguated name.
        assert_ne!(final_path, dest);
        assert_eq!(fs::read_to_string(&final_path).unwrap(), "NEW content");
    }

    #[test]
    fn safe_rename_noop_when_src_equals_dest() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.m4a");
        fs::write(&p, "content").unwrap();
        let final_path = safe_rename(&p, &p).unwrap();
        assert_eq!(final_path, p);
        assert_eq!(fs::read_to_string(&p).unwrap(), "content");
    }

    #[test]
    fn safe_rename_errors_when_src_missing() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("does-not-exist");
        let dest = dir.path().join("dest");
        assert!(safe_rename(&src, &dest).is_err());
    }

    #[test]
    fn rename_if_dest_free_refuses_when_taken() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        fs::write(&src, "NEW").unwrap();
        fs::write(&dest, "OLD").unwrap();

        let renamed = rename_if_dest_free(&src, &dest).unwrap();
        assert!(!renamed);
        // Both survive, nothing overwritten.
        assert_eq!(fs::read_to_string(&src).unwrap(), "NEW");
        assert_eq!(fs::read_to_string(&dest).unwrap(), "OLD");
    }

    #[test]
    fn rename_if_dest_free_renames_when_free() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        fs::write(&src, "NEW").unwrap();

        let renamed = rename_if_dest_free(&src, &dest).unwrap();
        assert!(renamed);
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "NEW");
    }

    #[test]
    fn write_non_clobbering_disambiguates() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("dump.json"), "EXISTING").unwrap();

        let path = write_non_clobbering(dir.path(), "dump.json", "NEW").unwrap();
        assert_ne!(path, dir.path().join("dump.json"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "NEW");
        assert_eq!(
            fs::read_to_string(dir.path().join("dump.json")).unwrap(),
            "EXISTING"
        );
    }

    // ------------------------------------------------------------
    // write_deduped (#553)
    // ------------------------------------------------------------

    #[test]
    fn write_deduped_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let out = write_deduped(dir.path(), "data.json", b"hello").unwrap();
        assert_eq!(out, dir.path().join("data.json"));
        assert_eq!(fs::read(out).unwrap(), b"hello");
    }

    #[test]
    fn write_deduped_is_noop_when_content_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"hello").unwrap();

        let out = write_deduped(dir.path(), "data.json", b"hello").unwrap();

        // Returned path is the original, no `.1.json` sprawl.
        assert_eq!(out, path);
        // Original content preserved.
        assert_eq!(fs::read(&path).unwrap(), b"hello");
        // Directory has exactly one file (no duplicate).
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn write_deduped_disambiguates_when_content_differs() {
        let dir = tempfile::tempdir().unwrap();
        let orig = dir.path().join("data.json");
        fs::write(&orig, b"old content").unwrap();

        let out = write_deduped(dir.path(), "data.json", b"new content").unwrap();

        // New content landed on a disambiguated path.
        assert_eq!(out, dir.path().join("data.1.json"));
        // Original preserved.
        assert_eq!(fs::read(&orig).unwrap(), b"old content");
        // New file has the new content.
        assert_eq!(fs::read(&out).unwrap(), b"new content");
    }

    #[test]
    fn write_deduped_handles_repeat_identical_writes_without_sprawl() {
        // Simulates the real use case: repeated re-downloads of an album
        // whose API response hasn't changed between runs. After 5
        // identical writes the directory should still contain exactly
        // one file.
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..5 {
            write_deduped(dir.path(), "dump.json", b"same bytes every time").unwrap();
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    // -----------------------------------------------------------
    // is_filesystem_sidecar (#577)
    // -----------------------------------------------------------

    #[test]
    fn appledouble_sidecar_is_detected() {
        // The single most impactful case: macOS `._*` files on
        // exFAT / FAT32 / HFS external drives.
        assert!(is_filesystem_sidecar(std::path::Path::new("._1 - 01 Track.m4a")));
        assert!(is_filesystem_sidecar(std::path::Path::new("/full/path/._Cover.jpg")));
    }

    #[test]
    fn ds_store_is_detected() {
        assert!(is_filesystem_sidecar(std::path::Path::new(".DS_Store")));
        assert!(is_filesystem_sidecar(std::path::Path::new("/Users/bob/Music/.DS_Store")));
    }

    #[test]
    fn thumbs_db_both_cases_detected() {
        assert!(is_filesystem_sidecar(std::path::Path::new("Thumbs.db")));
        assert!(is_filesystem_sidecar(std::path::Path::new("thumbs.db")));
    }

    #[test]
    fn desktop_ini_is_detected() {
        assert!(is_filesystem_sidecar(std::path::Path::new("desktop.ini")));
    }

    #[test]
    fn real_audio_files_are_not_sidecars() {
        // Regression canary: real audio filenames, including those
        // that START with legal single dots or underscores, must NOT
        // be misclassified. Only `._` (dot-underscore together at the
        // start) is reserved.
        assert!(!is_filesystem_sidecar(std::path::Path::new("1 - 01 Track.m4a")));
        assert!(!is_filesystem_sidecar(std::path::Path::new("Cover.jpg")));
        assert!(!is_filesystem_sidecar(std::path::Path::new(".hidden_file.m4a")));
        assert!(!is_filesystem_sidecar(std::path::Path::new("_underscore_start.m4a")));
        assert!(!is_filesystem_sidecar(std::path::Path::new("Ds_Store.m4a")));
    }

    #[test]
    fn paths_without_filename_component_return_false() {
        // Defensive: path consisting of `/` only has no file_name.
        assert!(!is_filesystem_sidecar(std::path::Path::new("/")));
    }

    // ============================================================
    // classify_filename — defensive degenerate-name guard (#532)
    // ============================================================

    #[test]
    fn classify_real_filenames_are_ok() {
        assert_eq!(
            classify_filename(Path::new("01 Track.m4a")),
            FilenameClassification::Ok,
        );
        assert_eq!(
            classify_filename(Path::new("Anne-Marie/01 Saxobeat.m4a")),
            FilenameClassification::Ok,
        );
        assert_eq!(
            classify_filename(Path::new("Cover.jpg")),
            FilenameClassification::Ok,
        );
        // Dot-prefixed hidden files have a stem like `.FrontCover` —
        // explicitly NOT empty, must classify as Ok.
        assert_eq!(
            classify_filename(Path::new(".FrontCover.mp4")),
            FilenameClassification::Ok,
        );
    }

    #[test]
    fn classify_empty_stem_is_degenerate() {
        assert!(matches!(
            classify_filename(Path::new(".mp4")),
            FilenameClassification::Degenerate { .. }
        ));
        assert!(matches!(
            classify_filename(Path::new(".m4a")),
            FilenameClassification::Degenerate { .. }
        ));
    }

    #[test]
    fn classify_punctuation_only_stem_is_degenerate() {
        // The #527 RC-blocker shape: `-.mp4` from a template
        // substitution where `{title}` and `{disc}` both resolved
        // to empty strings.
        assert!(matches!(
            classify_filename(Path::new("-.mp4")),
            FilenameClassification::Degenerate { .. }
        ));
        assert!(matches!(
            classify_filename(Path::new("--.mp4")),
            FilenameClassification::Degenerate { .. }
        ));
        assert!(matches!(
            classify_filename(Path::new("_.mp4")),
            FilenameClassification::Degenerate { .. }
        ));
        assert!(matches!(
            classify_filename(Path::new("   .mp4")),
            FilenameClassification::Degenerate { .. }
        ));
    }

    #[test]
    fn classify_unknown_marker_is_degenerate() {
        // The literal `[Unknown]` marker that #527 caught in
        // production. Case-insensitive — `[UNKNOWN]` and
        // `[unknown]` are both flagged.
        let cases = ["[Unknown] track.mp4", "[unknown].mp4", "[UNKNOWN].mp4"];
        for case in cases {
            let result = classify_filename(Path::new(case));
            assert!(
                matches!(result, FilenameClassification::Degenerate { .. }),
                "expected Degenerate for {case:?}, got {result:?}",
            );
        }
    }

    #[test]
    fn classify_unknown_sentinel_is_suspicious() {
        // GAMDL's `Unknown Album` / `Unknown Artist` fallbacks
        // aren't strictly broken — they're the legitimate output
        // of the no-album template path — but still worth a warn.
        let suspicious_cases = [
            "Unknown Album.m4a",
            "Unknown Title.mp4",
            "Unknown Artist - Song.m4a",
            // Composer / Year variants per the rule list.
            "Unknown Composer.m4a",
        ];
        for case in suspicious_cases {
            let result = classify_filename(Path::new(case));
            assert!(
                matches!(result, FilenameClassification::Suspicious { .. }),
                "expected Suspicious for {case:?}, got {result:?}",
            );
        }
    }

    #[test]
    fn classify_path_components_catches_parent_dir_problems() {
        // The MV path `{artist}/[Unknown]/-.mp4` from #527 — leaf
        // is `-.mp4` (Degenerate) and parent is `[Unknown]`
        // (also Degenerate). The walker returns Degenerate either
        // way; this asserts both halves of the path are scanned.
        assert!(matches!(
            classify_path_components(Path::new("Anne-Marie/[Unknown]/track.mp4")),
            FilenameClassification::Degenerate { .. },
        ));
        assert!(matches!(
            classify_path_components(Path::new("Music/Unknown Album/track.mp4")),
            FilenameClassification::Suspicious { .. },
        ));
        // All-Ok path stays Ok.
        assert_eq!(
            classify_path_components(Path::new("Music/Anne-Marie/Album/track.m4a")),
            FilenameClassification::Ok,
        );
    }

    #[test]
    fn is_problem_distinguishes_ok_from_warn_or_block() {
        assert!(!FilenameClassification::Ok.is_problem());
        assert!(FilenameClassification::Suspicious {
            reason: "x".to_string()
        }
        .is_problem());
        assert!(FilenameClassification::Degenerate {
            reason: "x".to_string()
        }
        .is_problem());
    }
}
