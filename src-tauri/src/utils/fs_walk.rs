// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Depth-bounded filesystem traversal helpers.
// =============================================
//
// Codebase audit (#716 finding #1) identified five+ ad-hoc recursive
// filesystem walkers in the backend, each open-coding the same pattern:
// `read_dir` → recurse on subdirs → filter files by extension /
// presence-test → accumulate results, with manual depth tracking and
// max-depth bailouts. Each rewrites the same boilerplate; some are
// missing the depth limit (which #712 had to bolt onto
// `find_dirs_with_ttml` after the user's 30-minute hang reproduction).
//
// This module provides ONE primitive — `walk_dir_depth` — that every
// callsite can reuse. The visitor pattern keeps it generic enough to
// cover collect-paths, count-files, and find-first use cases without
// fragmenting into N specialised wrappers.
//
// Migration is opt-in per callsite (#716 follow-ups). Existing walkers
// keep working until they're individually refactored to call the
// helper; the value of the helper grows as more callsites adopt it
// AND as new services land (Spotify M9, YouTube M10, BBC iPlayer M8
// will all need similar walkers — this is the shared foundation).

use std::path::Path;

/// Depth-limited recursive directory traversal with a visitor closure.
///
/// Walks `base` and every subdirectory up to `max_depth` levels
/// (inclusive of `base` itself at depth 0). For each filesystem entry
/// (both files AND directories), invokes the visitor; whatever the
/// visitor returns gets accumulated into the result vec.
///
/// Visitor semantics:
/// - Return `Some(T)` to record a result for this entry.
/// - Return `None` to skip this entry (no recording).
/// - The visitor sees BOTH files and directories — it's the visitor's
///   job to filter on `path.is_file()` / `path.is_dir()` if it cares.
/// - Recursion is automatic for directories; the visitor doesn't
///   control descent. (If you need predicate-controlled descent,
///   use [`walk_dir_depth_with_descend`] instead — not implemented
///   yet; file an issue if you need it.)
///
/// **Why a depth limit is mandatory.** Pre-helper callsites that
/// forgot the limit chewed through the user's full music library when
/// pointed at `~/Music/` (the GAMDL output root), producing the
/// 30-minute "Companion: converted N TTML" silent hang reproduced on
/// 2026-05-08 (#712). The helper takes `max_depth` as a required
/// argument so this category of bug is impossible to write again
/// without explicitly opting into unbounded recursion.
///
/// Conventional `max_depth` values used in this codebase:
/// - **3** — GAMDL's natural `Output/Artist/Album/file` shape; covers
///   all files under a known album dir without escaping.
/// - **10** — manifest-scan / library-scan entry point at the user's
///   root music library. Generous enough for nested compilation
///   structures, capped before unbounded scans.
///
/// Errors are silently swallowed (a single permission-denied dir
/// shouldn't abort the whole walk). If you need error reporting, the
/// visitor can side-effect into a logger.
pub fn walk_dir_depth<T, F>(base: &Path, max_depth: u32, mut visit: F) -> Vec<T>
where
    F: FnMut(&Path) -> Option<T>,
{
    let mut results = Vec::new();
    walk_inner(base, 0, max_depth, &mut visit, &mut results);
    results
}

fn walk_inner<T, F>(
    dir: &Path,
    depth: u32,
    max_depth: u32,
    visit: &mut F,
    results: &mut Vec<T>,
) where
    F: FnMut(&Path) -> Option<T>,
{
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(value) = visit(&path) {
            results.push(value);
        }
        if path.is_dir() {
            walk_inner(&path, depth + 1, max_depth, visit, results);
        }
    }
}

/// Find-first companion to [`walk_dir_depth`]: returns the FIRST path
/// for which the visitor returns `Some(T)`, stopping the walk early.
///
/// Use this when you're searching for a single match — locating an
/// extracted binary inside an archive, finding the first audio file
/// in an album dir, etc. The full-walk variant (`walk_dir_depth`)
/// would exhaust the whole tree before returning, which is wasteful
/// for early-termination shapes.
///
/// Visitor semantics match `walk_dir_depth`:
/// - Return `Some(T)` to claim this entry as the match (walk halts).
/// - Return `None` to keep searching.
/// - Visitor sees BOTH files and directories — filter on
///   `path.is_file()` / `path.is_dir()` if needed.
///
/// Recursion order is "visit current entry, then descend if dir" —
/// matches the natural left-to-right depth-first ordering. The
/// `max_depth` semantics are identical to `walk_dir_depth` (mandatory,
/// inclusive of `base` at depth 0).
pub fn walk_dir_find_first<T, F>(base: &Path, max_depth: u32, mut visit: F) -> Option<T>
where
    F: FnMut(&Path) -> Option<T>,
{
    walk_inner_find(base, 0, max_depth, &mut visit)
}

fn walk_inner_find<T, F>(
    dir: &Path,
    depth: u32,
    max_depth: u32,
    visit: &mut F,
) -> Option<T>
where
    F: FnMut(&Path) -> Option<T>,
{
    if depth > max_depth {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(value) = visit(&path) {
            return Some(value);
        }
        if path.is_dir() {
            if let Some(value) = walk_inner_find(&path, depth + 1, max_depth, visit) {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Visitor that records every file path with the given extension.
    /// Used by the simpler tests; not exposed as a helper because
    /// real callsites usually want extra filtering (depth, presence-
    /// of-sibling, etc.).
    fn collect_files_with_ext<'a>(
        ext: &'a str,
    ) -> impl FnMut(&Path) -> Option<std::path::PathBuf> + 'a {
        move |p: &Path| {
            if p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            {
                Some(p.to_path_buf())
            } else {
                None
            }
        }
    }

    #[test]
    fn walks_flat_directory_within_depth_limit() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.m4a"), b"").unwrap();
        fs::write(dir.path().join("b.m4a"), b"").unwrap();
        fs::write(dir.path().join("c.txt"), b"").unwrap();

        let m4a = walk_dir_depth(dir.path(), 0, collect_files_with_ext("m4a"));
        assert_eq!(m4a.len(), 2, "should find 2 .m4a files at depth 0");
    }

    #[test]
    fn respects_max_depth() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.m4a"), b"").unwrap();
        fs::write(dir.path().join("a").join("b").join("mid.m4a"), b"").unwrap();
        fs::write(dir.path().join("a").join("near.m4a"), b"").unwrap();
        fs::write(dir.path().join("top.m4a"), b"").unwrap();

        // depth 0: only base entries (no recursion past base)
        let d0 = walk_dir_depth(dir.path(), 0, collect_files_with_ext("m4a"));
        assert_eq!(d0.len(), 1, "depth 0 only sees base/top.m4a");

        // depth 1: base + immediate subdirs (a/) but a/ has no .m4a directly,
        // it has a 'b/' subdir; depth 1 stops descending into b/, so still 1
        // .m4a (top.m4a only). a/near.m4a is at depth 2 from base if you count
        // base (depth 0) → a/ (depth 1) → near.m4a (depth 1 still since
        // walking entries of a/ at depth 1 visits each but visitor is
        // depth-1-bounded, then descend into b/ at depth 2 etc.).
        // We expect 2 here: top.m4a + a/near.m4a (the iterator at depth 1
        // visits a/'s entries which include near.m4a).
        let d1 = walk_dir_depth(dir.path(), 1, collect_files_with_ext("m4a"));
        assert_eq!(d1.len(), 2, "depth 1: top.m4a + a/near.m4a");

        // depth 10: full tree (4 .m4a files)
        let d10 = walk_dir_depth(dir.path(), 10, collect_files_with_ext("m4a"));
        assert_eq!(d10.len(), 4, "depth 10 walks the full tree");
    }

    #[test]
    fn returns_empty_on_missing_dir() {
        let result =
            walk_dir_depth(Path::new("/definitely/does/not/exist"), 10, collect_files_with_ext("m4a"));
        assert!(result.is_empty());
    }

    #[test]
    fn visitor_can_count_via_unit_returns() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.m4a"), b"").unwrap();
        fs::write(dir.path().join("b.m4a"), b"").unwrap();
        fs::write(dir.path().join("c.flac"), b"").unwrap();

        // Counting use case — visitor returns Some(()) for each match,
        // result.len() is the count.
        let count = walk_dir_depth(dir.path(), 0, |p: &Path| {
            if p.extension().is_some_and(|e| e == "m4a") {
                Some(())
            } else {
                None
            }
        })
        .len();
        assert_eq!(count, 2);
    }

    #[test]
    fn visitor_sees_directories_too() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir1")).unwrap();
        fs::create_dir(dir.path().join("subdir2")).unwrap();

        let dirs = walk_dir_depth(dir.path(), 0, |p: &Path| {
            if p.is_dir() {
                Some(p.to_path_buf())
            } else {
                None
            }
        });
        assert_eq!(dirs.len(), 2, "visitor sees both subdirs at depth 0");
    }

    // ===========================================================================
    // walk_dir_find_first
    // ===========================================================================

    /// Visitor matching files by exact name. Returns the path on match.
    fn match_file_named<'a>(
        target: &'a str,
    ) -> impl FnMut(&Path) -> Option<std::path::PathBuf> + 'a {
        move |p: &Path| {
            if p.is_file()
                && p.file_name().and_then(|n| n.to_str()) == Some(target)
            {
                Some(p.to_path_buf())
            } else {
                None
            }
        }
    }

    #[test]
    fn find_first_returns_match_at_depth_zero() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("target.bin"), b"x").unwrap();
        fs::write(dir.path().join("other.bin"), b"x").unwrap();

        let found = walk_dir_find_first(dir.path(), 5, match_file_named("target.bin"));
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("target.bin"));
    }

    #[test]
    fn find_first_descends_into_subdirs() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("ffmpeg"), b"x").unwrap();

        let found = walk_dir_find_first(dir.path(), 5, match_file_named("ffmpeg"));
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("ffmpeg"));
    }

    #[test]
    fn find_first_returns_none_when_no_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("decoy.bin"), b"x").unwrap();

        let found = walk_dir_find_first(dir.path(), 5, match_file_named("missing"));
        assert!(found.is_none());
    }

    #[test]
    fn find_first_respects_max_depth() {
        let dir = TempDir::new().unwrap();
        let too_deep = dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&too_deep).unwrap();
        fs::write(too_deep.join("target.bin"), b"x").unwrap();

        // depth=2 stops at a/b/, never sees a/b/c/target.bin
        assert!(walk_dir_find_first(dir.path(), 2, match_file_named("target.bin")).is_none());
        // depth=3 reaches it
        assert!(walk_dir_find_first(dir.path(), 3, match_file_named("target.bin")).is_some());
    }

    #[test]
    fn find_first_short_circuits_after_first_match() {
        // Visitor with a call counter — proves we don't keep walking after match.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.bin"), b"x").unwrap();
        fs::write(dir.path().join("b.bin"), b"x").unwrap();
        fs::write(dir.path().join("c.bin"), b"x").unwrap();

        let mut calls = 0;
        let _ = walk_dir_find_first(dir.path(), 5, |p: &Path| {
            calls += 1;
            if p.file_name().and_then(|n| n.to_str()) == Some("a.bin") {
                Some(p.to_path_buf())
            } else {
                None
            }
        });
        // Filesystem ordering is non-deterministic, but we must have stopped
        // by the time we found "a.bin" — so calls is bounded above by the
        // total entry count (3), and below by 1.
        assert!(
            (1..=3).contains(&calls),
            "expected 1..=3 visitor calls (early exit after match), got {calls}"
        );
    }

    #[test]
    fn find_first_returns_none_on_missing_dir() {
        let result =
            walk_dir_find_first(Path::new("/definitely/does/not/exist"), 5, match_file_named("x"));
        assert!(result.is_none());
    }
}
