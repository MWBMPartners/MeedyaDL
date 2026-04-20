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
}
