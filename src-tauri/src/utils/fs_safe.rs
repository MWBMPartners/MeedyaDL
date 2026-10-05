// Copyright (c) 2024-2026 MeedyaSuite
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

/// The start of every temporary subtitle file name MeedyaDL writes while
/// it extracts a music video's subtitles: `.meedyadl-partial-`. Defined
/// here, once, beside [`is_temporary_subtitle_file`], the check that
/// skips these files.
pub const TEMPORARY_SUBTITLE_PREFIX: &str = ".meedyadl-partial-";

/// True when `path`'s file name starts with [`TEMPORARY_SUBTITLE_PREFIX`]:
/// a subtitle MeedyaDL is still writing, or one a stopped run left behind.
/// Its contents may be half written, or another run's work in progress, so
/// every place that lists subtitle or lyrics files in a folder skips it
/// through this one check, rather than each keeping its own copy (round 8
/// follow-up, acting on Codex's review of rounds 6-7). The lyrics pairing
/// step had copied such a file next to a music video as its lyrics, and
/// the lyrics count counted it as a track's lyrics; round 8 stopped
/// deleting other runs' leftovers, which made both likelier.
///
/// A name proves nothing about who made a file, so this is used only to
/// skip such files and to count them, never to decide to delete one.
#[must_use]
pub fn is_temporary_subtitle_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name.as_encoded_bytes()
            .starts_with(TEMPORARY_SUBTITLE_PREFIX.as_bytes())
    })
}

// ---------------------------------------------------------------------
// Operating-system operations the standard library does not offer
// (Codex's review of round 5, findings 3 and 4)
// ---------------------------------------------------------------------
//
// `std::fs::rename` REPLACES an existing destination on macOS and Linux,
// and on Windows it asks for exactly that (`MOVEFILE_REPLACE_EXISTING`).
// Checking first and renaming second leaves a gap in which another
// process can create the name, which the rename then destroys. The only
// way to "put this file here unless something is already there" with no
// gap is to ask the operating system to do both as one step:
//
//   - macOS:   `renamex_np(from, to, RENAME_EXCL)`
//   - Linux:   `renameat2(AT_FDCWD, from, AT_FDCWD, to, RENAME_NOREPLACE)`
//   - Windows: `MoveFileExW(from, to, 0)` -- WITHOUT
//              `MOVEFILE_REPLACE_EXISTING`, so it refuses a taken name.
//
// A file system that cannot do that step reports it (see
// `is_no_replace_rename_unsupported`); callers then never fall back to a
// plain rename. They may use `copy_to_new_file`, which also never replaces
// but is not one step (read its limits), or refuse.
//
// The Windows branches are compiled for Windows (checked with
// `cargo check --target x86_64-pc-windows-msvc`) but have not been run on
// Windows: no Windows machine was available when they were written.

/// Moves `from` to `to` only if nothing is at `to`, as ONE operation the
/// operating system performs atomically -- no window in which another
/// process can create `to` and have it replaced (see the section comment
/// above for the call used on each system).
///
/// # Errors
/// - [`std::io::ErrorKind::AlreadyExists`] when `to` is taken; nothing is
///   moved and both files are left exactly as they were.
/// - An error for which [`is_no_replace_rename_unsupported`] is true when
///   the file system (or system) cannot do this; nothing is moved.
/// - Any other error from the call, unchanged.
pub fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    rename_no_replace_impl(from, to)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn rename_no_replace_impl(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let to_c = |path: &Path| {
        CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a file name contains a zero byte",
            )
        })
    };
    let (from_c, to_c) = (to_c(from)?, to_c(to)?);

    // SAFETY: both arguments are text ending in a zero byte (`CString`)
    // that outlives the call, which reads them and keeps neither pointer.
    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(from_c.as_ptr(), to_c.as_ptr(), libc::RENAME_EXCL) };

    // SAFETY: as above. `AT_FDCWD` makes both names relative to the
    // current folder exactly as `rename` would; absolute names ignore it.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from_c.as_ptr(),
            libc::AT_FDCWD,
            to_c.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };

    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn rename_no_replace_impl(from: &Path, to: &Path) -> std::io::Result<()> {
    // Both names come from ONE function the tests call directly, so taking
    // the long-path conversion out of this call fails a test (Codex's review
    // of round 8, finding 2).
    let (from_w, to_w) = windows_rename_arguments(from, to)?;
    // SAFETY: both arguments are wide text ending in a zero character
    // that outlives the call, which reads them and keeps neither pointer.
    // Flags 0: no MOVEFILE_REPLACE_EXISTING, so a taken name is refused
    // (ERROR_ALREADY_EXISTS / ERROR_FILE_EXISTS, both AlreadyExists); no
    // MOVEFILE_COPY_ALLOWED, so it never turns into copy-then-delete.
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(from_w.as_ptr(), to_w.as_ptr(), 0)
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// The two arguments `rename_no_replace` hands `MoveFileExW` on Windows:
/// [`rename_arguments_for_windows`] with Windows' own way of making a path
/// absolute (`std::path::absolute`, which calls `GetFullPathNameW`).
#[cfg(windows)]
fn windows_rename_arguments(from: &Path, to: &Path) -> std::io::Result<(Vec<u16>, Vec<u16>)> {
    rename_arguments_for_windows(from, to, |path: &Path| std::path::absolute(path))
}

/// Each of two paths as wide text ending in a zero, in the form Rust's own
/// Windows file code hands to Windows, so that a path longer than Windows'
/// old 260-character limit still works (Codex's review of rounds 6-7,
/// finding 5). `rename_no_replace` has to call `MoveFileExW` itself (Rust's
/// own rename would replace a file), and without this a subtitle whose full
/// path was too long could not be published on a drive without hard links,
/// although every other file operation on it works.
///
/// It follows the standard library's `get_long_path`
/// (`library/std/src/sys/path/windows.rs`):
///   - a path that does not need converting ([`needs_windows_long_form`]:
///     already extended, empty, or a short ordinary absolute path) is passed
///     on exactly as it is, as Rust does -- Windows takes these as they are;
///   - any other path is first made absolute by `absolute`, then given the
///     extended-length prefix ([`with_windows_long_prefix`]).
///
/// `absolute` is passed in so that this runs and is tested on every system;
/// on Windows it is `std::path::absolute`, which asks Windows itself
/// (`GetFullPathNameW`). That matters for a path such as `D:Music\V.srt`:
/// Windows means drive D's REMEMBERED folder (each drive keeps its own
/// current folder), which only Windows knows. Round 8 worked it out here
/// instead and put such a path under the drive's root, so the publish step
/// looked for the temporary file in the wrong folder (Codex's review of
/// round 8, finding 2). Windows also tidies the path (`/`, `.` and `..`,
/// trailing dots and spaces) in that same call.
///
/// # Errors
/// [`std::io::ErrorKind::InvalidInput`] for a path containing a zero
/// character; any error from `absolute`.
#[cfg_attr(not(windows), allow(dead_code))]
fn rename_arguments_for_windows(
    from: &Path,
    to: &Path,
    absolute: impl Fn(&Path) -> std::io::Result<std::path::PathBuf>,
) -> std::io::Result<(Vec<u16>, Vec<u16>)> {
    let one = |path: &Path| -> std::io::Result<Vec<u16>> {
        let units = wide_units(path);
        if units.contains(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a file name contains a zero character",
            ));
        }
        let mut long = if needs_windows_long_form(&units) {
            with_windows_long_prefix(&wide_units(&absolute(path)?))
        } else {
            units
        };
        long.push(0);
        Ok(long)
    };
    Ok((one(from)?, one(to)?))
}

/// `path` as UTF-16 units, without a closing zero: exactly on Windows; on
/// other systems (where only the tests use it) from its text.
#[cfg_attr(not(windows), allow(dead_code))]
fn wide_units(path: &Path) -> Vec<u16> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().collect()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().encode_utf16().collect()
    }
}

/// Whether Rust's own Windows file code would convert `path` (UTF-16,
/// without the closing zero) to the extended-length form (`get_long_path`):
/// not when it is empty or already extended (`\\?\…`, `\??\…`), and not
/// when it is shorter than 247 characters AND an ordinary drive path
/// (`C:\…`, `C:/…`, or just `C:`) or starts with two separators (a share,
/// `\\server\share\…`). Every other path is converted: a long one, and a
/// relative one whatever its length (`sub\V.srt`, `\Music\V.srt`,
/// `D:Music\V.srt`).
#[cfg_attr(not(windows), allow(dead_code))]
fn needs_windows_long_form(path: &[u16]) -> bool {
    // What `get_long_path` calls `LEGACY_MAX_PATH`, counted with the
    // closing zero, which `path` here does not have.
    const LEGACY_MAX_PATH: usize = 248;
    let is_sep = |c: u16| c == SEP || c == ALT_SEP;
    if path.is_empty() || path.starts_with(&VERBATIM_PREFIX) || path.starts_with(&NT_PREFIX) {
        return false;
    }
    if path.len() + 1 < LEGACY_MAX_PATH {
        match path {
            [drive, COLON] if !is_sep(*drive) => return false,
            [drive, COLON, sep, ..] if !is_sep(*drive) && is_sep(*sep) => return false,
            [a, b, ..] if is_sep(*a) && is_sep(*b) => return false,
            _ => {}
        }
    }
    true
}

/// The extended-length form of an ABSOLUTE path as Windows' own
/// `GetFullPathNameW` returns it (backslashes only, already tidied), as
/// `get_long_path` makes it: `C:\…` becomes `\\?\C:\…`; a share
/// `\\server\share\…` becomes `\\?\UNC\server\share\…`; a device path
/// `\\.\…` becomes `\\?\…`; an extended path is kept; anything else is kept.
#[cfg_attr(not(windows), allow(dead_code))]
fn with_windows_long_prefix(absolute: &[u16]) -> Vec<u16> {
    match absolute {
        [_, COLON, SEP, ..] => [&VERBATIM_PREFIX[..], absolute].concat(),
        [SEP, SEP, DOT, SEP, rest @ ..] => [&VERBATIM_PREFIX[..], rest].concat(),
        [SEP, SEP, QUERY, SEP, ..] | [SEP, QUERY, QUERY, SEP, ..] => absolute.to_vec(),
        [SEP, SEP, rest @ ..] => [&UNC_PREFIX[..], rest].concat(),
        _ => absolute.to_vec(),
    }
}

// The UTF-16 units the Windows path functions above work with (all ASCII).
const SEP: u16 = b'\\' as u16;
const ALT_SEP: u16 = b'/' as u16;
const QUERY: u16 = b'?' as u16;
const COLON: u16 = b':' as u16;
const DOT: u16 = b'.' as u16;
/// `\\?\`
const VERBATIM_PREFIX: [u16; 4] = [SEP, SEP, QUERY, SEP];
/// `\??\`
const NT_PREFIX: [u16; 4] = [SEP, QUERY, QUERY, SEP];
/// `\\?\UNC\`
const UNC_PREFIX: [u16; 8] = [
    SEP,
    SEP,
    QUERY,
    SEP,
    b'U' as u16,
    b'N' as u16,
    b'C' as u16,
    SEP,
];

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn rename_no_replace_impl(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no operating-system call to rename without replacing is known here",
    ))
}

/// Copies the file at `from` into a NEW file at `to`, never replacing
/// anything: `to` is created only if no file has that name (the
/// "create only if new" step, which every drive offers -- FAT and exFAT
/// included), then filled and flushed to the drive (`sync_all`). `from` is
/// left as it is; the caller removes it.
///
/// For a drive that can neither make hard links nor rename without
/// replacing -- an exFAT drive on a Mac (stand-in review of round 6,
/// finding 1: checked on macOS 27, which refuses `renamex_np` with
/// `RENAME_EXCL` there with "not supported" whenever the name is free).
///
/// **On a failure after `to` was created**, the new file is deleted again --
/// but only if the name still refers to the file this call created
/// (Codex's review of rounds 6-7, finding 1). Creating it only if new
/// proves it was this call's file at that moment, not afterwards: another
/// program could rename or delete it and put its own file at the name
/// before the clean-up, and the clean-up used to delete whatever was there.
/// (The comment here used to say the file was "certainly" this call's.) So
/// the handle that created it is kept open, and the clean-up is
/// [`remove_if_still_ours`]: the file is moved aside to a private name and
/// compared with the handle there, and only a file proved to be this call's
/// is deleted, never the name `to` itself (Codex's review of round 8,
/// finding 1: checking `to` and then deleting `to` left a gap between the
/// two steps). Anything else -- a different file, one that cannot be
/// checked, or a file this drive cannot move aside -- is LEFT, and the error
/// says so ([`left_a_file_in_place`]).
///
/// **What this cannot do.**
/// - Be one step. A forced stop (the app killed, the power lost) while the
///   bytes are being copied leaves a PARTLY written file under the real
///   name, and nothing can then tell it from a finished one, so later runs
///   keep it. Hard links and [`rename_no_replace`] never leave a partial
///   file under the real name; use this only when neither is available.
/// - Delete anything on a drive that cannot move a file aside at all, even
///   through an exclusive placeholder. There a failed copy leaves its partly
///   written file under the real name, reported, rather than delete by name.
///   See [`remove_if_still_ours`] for its other limits.
///
/// # Errors
/// - [`std::io::ErrorKind::AlreadyExists`] when `to` is taken: nothing is
///   created or changed.
/// - Any other error opening `from`, or creating, writing or flushing `to`.
///   If deleting the partly written file then also fails, the message says
///   so and names where it is (the kind stays that of the first failure).
///   If a file was left because it could not be proved to be this call's,
///   or could not be moved aside, [`left_a_file_in_place`] is true for the
///   error and its message names the file (both names, if it was moved
///   aside and could not be put back) and why.
pub fn copy_to_new_file(from: &Path, to: &Path) -> std::io::Result<()> {
    copy_to_new_file_with(from, to, |_| {}, &mut |_| {})
}

/// [`copy_to_new_file`], with `before_clean_up` run after a copy has failed
/// and before the clean-up starts, and `pause` passed to the clean-up
/// ([`remove_if_still_ours_with`]). The tests put a different file at the
/// name at those points; the app passes nothing (through
/// [`copy_to_new_file`]).
pub(crate) fn copy_to_new_file_with(
    from: &Path,
    to: &Path,
    before_clean_up: impl FnOnce(&Path),
    pause: &mut dyn FnMut(RemovalStep<'_>),
) -> std::io::Result<()> {
    let mut source = std::fs::File::open(from)?;
    // Opened for reading too only because Windows needs read access to say
    // which file a handle is on (see `handle_identity`).
    let mut target = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(to)?;
    let filled = std::io::copy(&mut source, &mut target).and_then(|_| target.sync_all());
    let Err(error) = filled else {
        return Ok(());
    };
    before_clean_up(to);
    // `target` is still open: the file this call made still exists, so its
    // number cannot have been handed to a file put at the name meanwhile.
    let outcome = match remove_if_still_ours_with(to, &target, pause) {
        // Deleted; or someone else removed it, and nothing else is touched.
        Removal::Removed | Removal::AlreadyGone => Err(error),
        Removal::LeftOtherFile => Err(left_in_place(
            error,
            to,
            "a different file now has that name".to_string(),
        )),
        Removal::LeftUnchecked(e) => Err(left_in_place(
            error,
            to,
            format!("it could not be checked which file now has that name ({e})"),
        )),
        Removal::NotMovedAside(e) => Err(left_in_place(
            error,
            to,
            format!(
                "it could not be moved aside to be checked ({e}), and MeedyaDL deletes a \
                 file only after moving it aside and checking it is its own"
            ),
        )),
        Removal::LeftAside { aside, why } => Err(left_in_place(
            error,
            &aside,
            format!(
                "it was moved there from {} to be deleted, but {why}; nothing was deleted, \
                 whatever is now at {} was not touched, and it can be renamed back to {} \
                 once nothing else has that name",
                to.display(),
                to.display(),
                to.display()
            ),
        )),
        Removal::DeleteFailed { at, error: left } => Err(std::io::Error::new(
            error.kind(),
            format!(
                "{error}; the partly written file, at {}, could not be deleted either ({left})",
                at.display()
            ),
        )),
    };
    drop(target);
    outcome
}

/// What [`copy_to_new_file`] reports when its copy failed after it had
/// created the new file, and it then left a file at the new name because it
/// could not prove that file was the one it created.
#[derive(Debug)]
struct LeftInPlace {
    /// The name the file was left at.
    path: PathBuf,
    /// Why it could not be proved to be this call's file.
    why: String,
    /// The failure that stopped the copy.
    copy_error: std::io::Error,
}

impl std::fmt::Display for LeftInPlace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; a file was left at {}: {}, so it was not deleted",
            self.copy_error,
            self.path.display(),
            self.why
        )
    }
}

impl std::error::Error for LeftInPlace {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.copy_error)
    }
}

/// Wraps `copy_error` as a [`LeftInPlace`], keeping its kind.
fn left_in_place(copy_error: std::io::Error, path: &Path, why: String) -> std::io::Error {
    std::io::Error::new(
        copy_error.kind(),
        LeftInPlace {
            path: path.to_path_buf(),
            why,
            copy_error,
        },
    )
}

/// True when `error` came from [`copy_to_new_file`] LEAVING a file at the
/// new name after a failed copy, because it could not prove that file was
/// the one it created (it may be another program's), rather than deleting
/// it. The caller should tell the person that something is at that name.
#[must_use]
pub fn left_a_file_in_place(error: &std::io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|inner| inner.is::<LeftInPlace>())
}

/// True when [`rename_no_replace`] failed because the file system (or the
/// system) cannot rename without replacing -- not because of anything
/// about these particular files. A caller must then NOT fall back to a
/// plain rename, which would bring back the overwrite this exists to stop
/// ([`copy_to_new_file`] is the step that never replaces).
///
/// - Any system: [`std::io::ErrorKind::Unsupported`] (the call does not
///   exist, `ENOSYS`; or `EOPNOTSUPP`).
/// - macOS: `EINVAL`, `ENOTSUP` (a volume that cannot honour
///   `RENAME_EXCL`: an exFAT drive answers `ENOTSUP` whenever the name is
///   free -- checked on macOS 27; a FAT32 drive does honour it).
/// - Linux: `EINVAL` (a file system that does not support
///   `RENAME_NOREPLACE`).
/// - Windows: `ERROR_INVALID_FUNCTION`, `ERROR_NOT_SUPPORTED`.
#[must_use]
pub fn is_no_replace_rename_unsupported(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::Unsupported {
        return true;
    }
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    #[cfg(target_os = "macos")]
    {
        code == libc::EINVAL || code == libc::ENOTSUP
    }
    #[cfg(target_os = "linux")]
    {
        code == libc::EINVAL
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED};
        u32::try_from(code).is_ok_and(|c| c == ERROR_INVALID_FUNCTION || c == ERROR_NOT_SUPPORTED)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = code;
        false
    }
}

/// True when `std::fs::hard_link` failed because the FILE SYSTEM cannot
/// make hard links (FAT and exFAT drives, many network shares) -- the one
/// case in which a caller may try [`rename_no_replace`] instead. Any other
/// failure (no permission, disk full, a missing folder) is a real failure
/// and must be reported as one, not "worked around".
///
/// - Any system: [`std::io::ErrorKind::Unsupported`] (`ENOSYS`,
///   `EOPNOTSUPP`, Windows `ERROR_CALL_NOT_IMPLEMENTED`).
/// - macOS: `ENOTSUP` ("the file system does not support links").
/// - Linux: `EPERM`, which `link(2)` documents for "the file system does
///   not support the creation of hard links" (its other meanings -- a
///   folder, an immutable file, or someone else's file under
///   `protected_hardlinks` -- cannot apply to a temporary file this app
///   has just created itself).
/// - Windows: `ERROR_INVALID_FUNCTION` (what FAT and exFAT answer) and
///   `ERROR_NOT_SUPPORTED`.
#[must_use]
pub fn is_hard_link_unsupported(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::Unsupported {
        return true;
    }
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    #[cfg(target_os = "macos")]
    {
        code == libc::ENOTSUP
    }
    #[cfg(target_os = "linux")]
    {
        code == libc::EPERM
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED};
        u32::try_from(code).is_ok_and(|c| c == ERROR_INVALID_FUNCTION || c == ERROR_NOT_SUPPORTED)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = code;
        false
    }
}

/// Which file a name refers to, and how many names that file has: enough
/// to tell that two names are the same file (hard links), and that a name
/// is not the file's only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    /// The volume the file is on.
    pub device: u64,
    /// The file's number on that volume.
    pub index: u64,
    /// How many names (hard links) the file has.
    pub links: u64,
}

impl FileIdentity {
    /// True when both describe the same file: the same volume and the same
    /// number on it. The number of names is left out on purpose, because
    /// it changes when a name is added (a hard link) or removed.
    ///
    /// What this can prove depends on when the two were read:
    ///   - a file's number can be handed to a NEW file once the old one is
    ///     gone (Linux file systems often do this at once), so a match
    ///     proves "the same file" only while the first file certainly still
    ///     exists -- for example while a handle to it is held open;
    ///   - on some drives a file's number CHANGES while it exists: on a
    ///     Mac's FAT32 and exFAT drives it changes once data is first
    ///     written into the file, and when the file is renamed (checked on
    ///     macOS 27). A number stored earlier then no longer matches the
    ///     same file. Compare two identities read at the same moment, as
    ///     [`check_name_against_handle`] does.
    #[must_use]
    pub fn is_same_file(&self, other: &FileIdentity) -> bool {
        self.device == other.device && self.index == other.index
    }
}

/// The [`FileIdentity`] of the regular file at `path`, without following
/// a symbolic link (a link's own identity, never its target's).
///
/// # Errors
/// Any error reading it; [`std::io::ErrorKind::Unsupported`] on a system
/// with no known way to ask.
pub fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
    file_identity_impl(path)
}

#[cfg(unix)]
fn file_identity_impl(path: &Path) -> std::io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    Ok(FileIdentity {
        device: meta.dev(),
        index: meta.ino(),
        links: meta.nlink(),
    })
}

#[cfg(windows)]
fn file_identity_impl(path: &Path) -> std::io::Result<FileIdentity> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    // Opened for no access at all (just to ask about it), sharing
    // everything, and not following a reparse point (a link).
    let file = std::fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    identity_of_open_windows_file(&file)
}

/// The identity of an open file on Windows, shared by [`file_identity`] and
/// [`handle_identity`].
#[cfg(windows)]
fn identity_of_open_windows_file(file: &std::fs::File) -> std::io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    // SAFETY: an all-zero BY_HANDLE_FILE_INFORMATION is a valid value
    // (plain numbers and times).
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the handle belongs to `file`, which stays open for the call;
    // `info` is writable and outlives it.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(FileIdentity {
        device: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        links: u64::from(info.nNumberOfLinks),
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity_impl(_path: &Path) -> std::io::Result<FileIdentity> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no known way to identify a file here",
    ))
}

/// The [`FileIdentity`] of the file `file` is open on, read from the handle
/// itself rather than from a name -- so it is certainly the file that was
/// opened (or created), whatever has since happened to its name.
///
/// On Windows the handle must have been opened with read access (Windows
/// asks for permission to read a file's attributes); the callers here open
/// their files for reading as well as writing for that reason.
///
/// # Errors
/// Any error reading it; [`std::io::ErrorKind::Unsupported`] on a system
/// with no known way to ask.
pub fn handle_identity(file: &std::fs::File) -> std::io::Result<FileIdentity> {
    handle_identity_impl(file)
}

#[cfg(unix)]
fn handle_identity_impl(file: &std::fs::File) -> std::io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let meta = file.metadata()?;
    Ok(FileIdentity {
        device: meta.dev(),
        index: meta.ino(),
        links: meta.nlink(),
    })
}

#[cfg(windows)]
fn handle_identity_impl(file: &std::fs::File) -> std::io::Result<FileIdentity> {
    identity_of_open_windows_file(file)
}

#[cfg(not(any(unix, windows)))]
fn handle_identity_impl(_file: &std::fs::File) -> std::io::Result<FileIdentity> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no known way to identify a file here",
    ))
}

/// What a name refers to, compared with an open file (see
/// [`check_name_against_handle`]).
#[derive(Debug)]
pub enum NameCheck {
    /// Nothing has that name.
    Gone,
    /// The name refers to the very file the handle is open on.
    SameFile,
    /// The name refers to a different file.
    OtherFile,
    /// It could not be told (the reason is given). Treat as "not proven".
    CannotTell(std::io::Error),
}

/// Whether `path` refers, at this moment, to the file `handle` is open on.
/// Both identities are read now, one from the handle and one from the name,
/// and compared.
///
/// Why "now" for both, and not a number stored when the file was created:
/// a Mac's FAT32 and exFAT drives give a file a new number once data is
/// first written into it, and when it is renamed (checked on macOS 27 with
/// disk images: a number read at creation no longer matched the same file
/// after 300 kB were written, while the handle and the name read at the
/// same moment matched every time, and never matched a different file put
/// at that name, whether the original had been renamed away or deleted).
/// And while the handle is open, the file still exists, so its number
/// cannot have been handed to a new file meanwhile.
///
/// The caller must keep `handle` open until it has acted on the answer, and
/// should know the answer can go stale: anything done by name after
/// [`NameCheck::SameFile`] is a second step. So never delete the name this
/// checked; use [`remove_if_still_ours`], which checks a private name the
/// file has first been moved to (Codex's review of round 8, finding 1).
#[must_use]
pub fn check_name_against_handle(path: &Path, handle: &std::fs::File) -> NameCheck {
    let at_name = match file_identity(path) {
        Ok(identity) => identity,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return NameCheck::Gone,
        Err(e) => return NameCheck::CannotTell(e),
    };
    match handle_identity(handle) {
        Ok(ours) if ours.is_same_file(&at_name) => NameCheck::SameFile,
        Ok(_) => NameCheck::OtherFile,
        Err(e) => NameCheck::CannotTell(e),
    }
}

// ---------------------------------------------------------------------
// Deleting a file only while it is provably this run's own
// (Codex's review of round 8, finding 1)
// ---------------------------------------------------------------------

/// 64 bits from the operating system's random source, as 16 lower-case
/// hexadecimal digits: the random part of the temporary and private names
/// MeedyaDL makes, so that no such name is ever handed out twice.
///
/// # Errors
/// When the system cannot supply random bits.
pub fn random_name_part() -> std::io::Result<String> {
    use rand::RngCore;
    let mut bytes = [0u8; 8];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes).map_err(|e| {
        std::io::Error::other(format!("could not get random bits from the system: {e}"))
    })?;
    Ok(format!("{:016x}", u64::from_le_bytes(bytes)))
}

/// A point in [`remove_if_still_ours`] at which a test can act. The app
/// passes nothing.
#[derive(Debug, Clone, Copy)]
pub enum RemovalStep<'a> {
    /// About to compare the file at `at` with the handle. On macOS and
    /// Linux `at` is the private name the file has just been moved to; on
    /// Windows it is the name itself, opened for deletion.
    BeforeCheck { at: &'a Path },
    /// Compared; about to delete the file, or to put it back.
    AfterCheck { at: &'a Path },
}

/// What [`remove_if_still_ours`] did.
#[derive(Debug)]
pub enum Removal {
    /// The file was this run's own, and it is deleted.
    Removed,
    /// Nothing had that name any more; nothing was deleted.
    AlreadyGone,
    /// The name held a different file. It is where it was: on macOS and
    /// Linux it was moved aside, found not to be this run's, and put back
    /// (only on a drive with the one-step rename; elsewhere it is kept aside,
    /// [`Removal::LeftAside`]).
    LeftOtherFile,
    /// It could not be told whether the file was this run's own, so it is
    /// where it was (put back, on macOS and Linux, on a drive with the
    /// one-step rename).
    LeftUnchecked(std::io::Error),
    /// The file could not be moved aside to be checked -- not even through
    /// an exclusive placeholder -- so nothing was moved or deleted.
    NotMovedAside(std::io::Error),
    /// Moved aside, found not to be this run's (or not checkable), and not
    /// put back -- the name was taken again meanwhile, or the drive cannot
    /// put a file back safely -- so it is KEPT under the private name
    /// `aside`, and can be renamed back. Whatever is now at the original
    /// name was not touched.
    LeftAside {
        /// The private name the file is left under.
        aside: PathBuf,
        /// Why it was not deleted and not put back.
        why: String,
    },
    /// This run's own file, but deleting it failed. It is left at `at`: the
    /// private name on macOS and Linux, the name itself on Windows.
    DeleteFailed {
        /// Where the file is left.
        at: PathBuf,
        /// Why it could not be deleted.
        error: std::io::Error,
    },
}

/// Deletes the file at `path` only if it is the very file `handle` is open
/// on -- never a file that took its name, whenever that happened.
///
/// **Why not "check the name, then delete the name"** (Codex's review of
/// round 8, finding 1): those are two steps, and another program can put
/// its own file at the name between them, which the deletion then removes.
///
/// **On macOS and Linux** the name is never deleted directly:
/// 1. the file is moved to a private name in the same folder, made of the
///    temporary prefix, fresh random bits and `.discard`
///    (`.meedyadl-partial-<pid>-<random>.discard`), with the rename that
///    refuses to replace a file ([`rename_no_replace`]), or, where that is
///    "not supported" (an exFAT drive on a Mac, whenever the new name is
///    free), through an exclusive placeholder at the private name
///    (`move_onto_private_placeholder`);
/// 2. the file now at the private name is compared with the file the handle
///    is on, both read at that moment ([`check_name_against_handle`]; this
///    works on a Mac's FAT32 drive too, which gives a file a new number when
///    it is renamed -- checked on macOS 27);
/// 3. the same file: the private name is deleted;
/// 4. a different file (or one that cannot be checked): it is put back
///    under its name with the one-step rename, only if that is free;
/// 5. if that fails, or the drive has no one-step rename (an exFAT drive on
///    a Mac), it is kept under the private name and the caller reports both
///    names, so the person can rename it back. It is never put back through
///    a placeholder (see `put_back`).
///
/// A file put at the original name at any moment is never deleted: only
/// the private name is.
///
/// **On Windows**, where a file can be deleted through a handle, there is
/// no name step between the check and the deletion: the name is opened for
/// deletion (without following a link), the file that new handle is on is
/// compared with the file `handle` is on, and only if they are the same is
/// it deleted through the new handle (`SetFileInformationByHandle`, with
/// POSIX semantics where the drive offers them, else the classic delete).
/// Whatever the name holds after the check, the deletion reaches only the
/// checked file.
///
/// **What this still does not guarantee.**
/// - The private name itself is not guarded against a program that
///   watches the folder; that is a deliberate limit of the design,
///   explained once at `move_aside`.
/// - A drive that cannot move a file aside at all, not even through a
///   placeholder, gets nothing deleted: the file is left where it is
///   ([`Removal::NotMovedAside`]). This never falls back to deleting by
///   name.
/// - In the [`Removal::LeftAside`] case another program's file can end up
///   under the private name; the caller names both.
/// - The Windows branch has been compiled for Windows but not run there.
#[must_use]
pub fn remove_if_still_ours(path: &Path, handle: &std::fs::File) -> Removal {
    remove_if_still_ours_with(path, handle, &mut |_| {})
}

/// [`remove_if_still_ours`], pausing at each [`RemovalStep`] (for tests).
pub(crate) fn remove_if_still_ours_with(
    path: &Path,
    handle: &std::fs::File,
    pause: &mut dyn FnMut(RemovalStep<'_>),
) -> Removal {
    remove_if_still_ours_impl(path, handle, pause)
}

#[cfg(not(windows))]
fn remove_if_still_ours_impl(
    path: &Path,
    handle: &std::fs::File,
    pause: &mut dyn FnMut(RemovalStep<'_>),
) -> Removal {
    // 1. Move aside, only to a free name.
    let aside = match move_aside(path) {
        Ok(Some(aside)) => aside,
        Ok(None) => return Removal::AlreadyGone,
        Err(e) => return Removal::NotMovedAside(e),
    };
    // 2. Compare, both read now.
    pause(RemovalStep::BeforeCheck { at: &aside });
    let check = check_name_against_handle(&aside, handle);
    pause(RemovalStep::AfterCheck { at: &aside });
    match check {
        // 3. Ours: delete the private name, never the original one.
        NameCheck::SameFile => match std::fs::remove_file(&aside) {
            Ok(()) => Removal::Removed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Removal::AlreadyGone,
            Err(e) => Removal::DeleteFailed {
                at: aside,
                error: e,
            },
        },
        NameCheck::Gone => Removal::AlreadyGone,
        // 4. Not ours, or not provably: put it back, only if free.
        NameCheck::OtherFile => put_back(
            &aside,
            path,
            Removal::LeftOtherFile,
            "it is not the file MeedyaDL made".to_string(),
        ),
        NameCheck::CannotTell(e) => {
            let why = format!("it could not be checked ({e})");
            put_back(&aside, path, Removal::LeftUnchecked(e), why)
        }
    }
}

/// Moves `path` to a fresh private name in its folder, never replacing a
/// file: with the one-step [`rename_no_replace`], or, where that is "not
/// supported" (an exFAT drive on a Mac, whenever the new name is free),
/// through an exclusive placeholder ([`move_onto_private_placeholder`]).
/// `Ok(None)` when nothing has the name.
///
/// **A limit of the design, accepted on purpose.** A program that watches
/// this folder could replace the file at the private name -- or, on a drive
/// without the one-step rename, the placeholder -- in the instant between
/// our check and our deletion, or between our placeholder and our rename,
/// and then lose its own file. This is not guarded, for two reasons. First,
/// any program that can write to this folder can delete or overwrite the
/// person's subtitles directly, so guarding this instant would protect
/// nothing. Second, no ordinary program can know the random name in
/// advance. (Codex's review of round 9 raised this; the lead judged it not
/// a real problem.) The names that matter -- the subtitle's own name, and
/// the name a file is put back to -- are never deleted, and nothing is ever
/// renamed over them.
#[cfg(not(windows))]
fn move_aside(path: &Path) -> std::io::Result<Option<PathBuf>> {
    const TRIES: u32 = 8;
    let folder = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    for _ in 0..TRIES {
        let aside = folder.join(format!(
            "{TEMPORARY_SUBTITLE_PREFIX}{}-{}.discard",
            std::process::id(),
            random_name_part()?
        ));
        let moved = match one_step_rename_no_replace(path, &aside) {
            Err(e) if is_no_replace_rename_unsupported(&e) => {
                move_onto_private_placeholder(path, &aside)
            }
            other => other,
        };
        match moved {
            Ok(()) => return Ok(Some(aside)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            // Practically impossible with 64 random bits: try another.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("no free private name to move the file aside to after {TRIES} tries"),
    ))
}

/// The move aside where the one-step no-replace rename is "not supported"
/// (the lead's decision after round 9 measured that an exFAT drive on a Mac
/// refuses it every time the new name is free, so that, without this, every
/// temporary file there was left behind):
/// 1. the private name `aside` (fresh random bits) is created exclusively
///    (`create_new`), so the name is certainly this run's;
/// 2. that empty placeholder is closed;
/// 3. `path` is renamed onto it with an ordinary rename, which can only
///    replace that placeholder.
///
/// If the rename fails (for example `path` has gone), nothing was moved, and
/// the empty placeholder is deleted again -- by its private name, the same
/// as the private name is deleted after the check.
///
/// The instant between steps 1 and 3 is not guarded against a program that
/// watches the folder: see the limit explained at [`move_aside`].
#[cfg(not(windows))]
fn move_onto_private_placeholder(path: &Path, aside: &Path) -> std::io::Result<()> {
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(aside)?,
    );
    std::fs::rename(path, aside).inspect_err(|_| {
        // Nothing moved: only our own empty placeholder is at `aside`.
        if let Err(e) = std::fs::remove_file(aside) {
            log::warn!(
                "Could not delete the empty placeholder {}: {e}",
                aside.display()
            );
        }
    })
}

/// Puts a file that turned out not to be this run's back under `path`,
/// only with the one-step [`rename_no_replace`], which never replaces a
/// file. `left` says what happened when that works. Otherwise the file is
/// KEPT under its private name `aside` ([`Removal::LeftAside`]): the caller
/// reports both names, and the person can rename it back. Nothing is lost,
/// because the file is kept.
///
/// Where the one-step rename is "not supported" (an exFAT drive on a Mac),
/// the file is never put back. Round 9 put it back through an exclusive
/// placeholder at `path` and an ordinary rename onto it; but a subtitle
/// another program saved at `path` in the instant between the placeholder
/// and the rename would have been overwritten, because an ordinary rename
/// replaces whatever is there (Codex's review of round 9, first finding).
/// That instant is at a name another program may legitimately be writing,
/// unlike a random private name, so it is not taken.
#[cfg(not(windows))]
fn put_back(aside: &Path, path: &Path, left: Removal, why: String) -> Removal {
    match one_step_rename_no_replace(aside, path) {
        Ok(()) => left,
        Err(e) if is_no_replace_rename_unsupported(&e) => Removal::LeftAside {
            aside: aside.to_path_buf(),
            why: format!(
                "{why}; this drive cannot put a file back without the risk of replacing one \
                 saved there meanwhile, so it was kept"
            ),
        },
        Err(e) => Removal::LeftAside {
            aside: aside.to_path_buf(),
            why: format!("{why}, and putting it back failed ({e})"),
        },
    }
}

/// [`rename_no_replace`] as the removal uses it. In tests it can be made to
/// answer "not supported" every time, as an exFAT drive on a Mac does for a
/// free name, so the placeholder path runs on any machine
/// (`as_if_one_step_rename_were_unsupported`).
#[cfg(not(windows))]
fn one_step_rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if PRETEND_ONE_STEP_RENAME_UNSUPPORTED.with(std::cell::Cell::get) {
        return Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
    }
    rename_no_replace(from, to)
}

#[cfg(all(test, not(windows)))]
thread_local! {
    /// See [`one_step_rename_no_replace`]. Per test thread.
    static PRETEND_ONE_STEP_RENAME_UNSUPPORTED: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Runs `f` with the one-step no-replace rename answering "not supported",
/// as an exFAT drive on a Mac does for a free name, so the removal takes
/// the exclusive-placeholder path on any machine. For tests only.
#[cfg(all(test, not(windows)))]
pub(crate) fn as_if_one_step_rename_were_unsupported<T>(f: impl FnOnce() -> T) -> T {
    PRETEND_ONE_STEP_RENAME_UNSUPPORTED.with(|pretend| pretend.set(true));
    let result = f();
    PRETEND_ONE_STEP_RENAME_UNSUPPORTED.with(|pretend| pretend.set(false));
    result
}

#[cfg(windows)]
fn remove_if_still_ours_impl(
    path: &Path,
    handle: &std::fs::File,
    pause: &mut dyn FnMut(RemovalStep<'_>),
) -> Removal {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    // Opened for deletion (and to say which file it is), sharing
    // everything, without following a link.
    let doomed = match std::fs::OpenOptions::new()
        .access_mode(DELETE | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Removal::AlreadyGone,
        Err(e) => return Removal::LeftUnchecked(e),
    };
    pause(RemovalStep::BeforeCheck { at: path });
    let same = match (handle_identity(&doomed), handle_identity(handle)) {
        (Ok(doomed_file), Ok(own_file)) => doomed_file.is_same_file(&own_file),
        (Err(e), _) | (_, Err(e)) => return Removal::LeftUnchecked(e),
    };
    pause(RemovalStep::AfterCheck { at: path });
    if !same {
        return Removal::LeftOtherFile;
    }
    match delete_through_handle(&doomed) {
        Ok(()) => Removal::Removed,
        Err(e) => Removal::DeleteFailed {
            at: path.to_path_buf(),
            error: e,
        },
    }
}

/// Marks the file `file` is open on for deletion: with POSIX semantics
/// (its name goes at once) where the drive offers them, else the classic
/// way (it goes when the last handle to it closes). Either way only that
/// file, whatever its name now holds.
#[cfg(windows)]
fn delete_through_handle(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, FileDispositionInfoEx, SetFileInformationByHandle,
        FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO,
        FILE_DISPOSITION_INFO_EX,
    };
    let posix = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
    };
    // SAFETY: the handle belongs to `file`, open for the call; `posix` is a
    // valid FILE_DISPOSITION_INFO_EX that outlives it, and its size is
    // passed (4 bytes, so the cast cannot truncate).
    let ok = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfoEx,
            std::ptr::from_ref(&posix).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    };
    if ok != 0 {
        return Ok(());
    }
    // Older Windows, or a drive (FAT, exFAT) without POSIX semantics.
    let posix_error = std::io::Error::last_os_error();
    let classic = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: as above, for a FILE_DISPOSITION_INFO (1 byte).
    let ok = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            std::ptr::from_ref(&classic).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        let e = std::io::Error::last_os_error();
        Err(std::io::Error::new(
            e.kind(),
            format!("{e} (and with POSIX semantics: {posix_error})"),
        ))
    }
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
    fn temporary_subtitle_files_are_recognised_by_their_prefix() {
        for path in [
            ".meedyadl-partial-123-0a1b2c3d4e5f6789.srt",
            ".meedyadl-partial-9-41.vtt",
            ".meedyadl-partial-anything",
            "/Music/Artist/Album/.meedyadl-partial-1-0.srt",
        ] {
            assert!(is_temporary_subtitle_file(Path::new(path)), "{path}");
        }
        for path in [
            "meedyadl-partial-1-0.srt",
            ".V.meedyadl-partial-1-0.srt",
            ".meedyadl-partia.srt",
            "V.en.srt",
            "/Music/.meedyadl-partial-folder/V.en.srt",
            "",
            "/",
        ] {
            assert!(!is_temporary_subtitle_file(Path::new(path)), "{path}");
        }
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

    // ── rename_no_replace (Codex's review of round 5, finding 3) ────────

    #[test]
    fn rename_no_replace_moves_to_a_free_name() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&from, "a").unwrap();
        rename_no_replace(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read_to_string(&to).unwrap(), "a");
    }

    #[test]
    fn rename_no_replace_refuses_a_taken_name_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&from, "a").unwrap();
        fs::write(&to, "b").unwrap();
        let err = rename_no_replace(&from, &to).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists, "{err}");
        assert!(!is_no_replace_rename_unsupported(&err));
        assert_eq!(fs::read_to_string(&from).unwrap(), "a");
        assert_eq!(fs::read_to_string(&to).unwrap(), "b");
    }

    // ── The Windows rename's path arguments (Codex's review of rounds 6-7,
    // ── finding 5, and of round 8, finding 2). These run on every system,
    // ── with a stand-in for Windows' own way of making a path absolute; the
    // ── `#[cfg(windows)]` tests further down use the real one and first run
    // ── in CI's Windows job.

    /// The first argument `rename_arguments_for_windows` makes from `path`,
    /// as text without its closing zero (checked to be there), with
    /// `absolute` standing in for Windows.
    fn first_argument(
        path: &str,
        absolute: impl Fn(&Path) -> std::io::Result<std::path::PathBuf>,
    ) -> String {
        let (from, _) =
            rename_arguments_for_windows(Path::new(path), Path::new(r"C:\x"), absolute).unwrap();
        assert_eq!(from.last(), Some(&0), "no closing zero");
        String::from_utf16(&from[..from.len() - 1]).unwrap()
    }

    /// A stand-in that must not be asked: the path needs no converting.
    fn never_absolute(path: &Path) -> std::io::Result<std::path::PathBuf> {
        panic!("asked to make {} absolute", path.display())
    }

    /// A stand-in that answers with `answer`, whatever it is asked.
    fn answers(answer: &str) -> impl Fn(&Path) -> std::io::Result<std::path::PathBuf> + '_ {
        move |_| Ok(std::path::PathBuf::from(answer))
    }

    /// A folder name long enough to push any path past Windows' old limit.
    fn long_part() -> String {
        "a".repeat(250)
    }

    #[test]
    fn a_short_ordinary_absolute_path_is_passed_on_as_it_is() {
        for path in [
            r"C:\Music\Artist\V.en.srt",
            "C:/Music/Artist/V.en.srt",
            r"C:\Music\.\x\..\V.en.srt",
            r"\\server\share\Music\V.en.srt",
            "C:",
        ] {
            assert_eq!(first_argument(path, never_absolute), path);
        }
    }

    #[test]
    fn an_already_extended_or_empty_path_is_passed_on_as_it_is() {
        for path in [
            format!(r"\\?\C:\x\..\{}\V.srt", long_part()),
            format!(r"\\?\UNC\server\share\{}\V.srt", long_part()),
            format!(r"\??\C:\{}\V.srt", long_part()),
            String::new(),
        ] {
            assert_eq!(first_argument(&path, never_absolute), path);
        }
    }

    /// Codex's review of round 8, finding 2: `D:Music\V.srt` means drive D's
    /// REMEMBERED folder, which only Windows knows. The path must be handed
    /// to Windows' own resolution (`absolute`), never put under the drive's
    /// root here, as round 8 did (it expected `\\?\D:\Music\V.srt`).
    #[test]
    fn a_relative_path_is_made_absolute_first() {
        // Windows, with drive D's remembered folder D:\Downloads.
        let asked = std::cell::RefCell::new(Vec::new());
        let windows = |path: &Path| {
            asked.borrow_mut().push(path.to_string_lossy().into_owned());
            Ok(std::path::PathBuf::from(r"D:\Downloads\Music\V.srt"))
        };
        assert_eq!(
            first_argument(r"D:Music\V.srt", windows),
            r"\\?\D:\Downloads\Music\V.srt"
        );
        assert_eq!(*asked.borrow(), [r"D:Music\V.srt"]);
        // An ordinary relative path, and one from the drive's root.
        assert_eq!(
            first_argument(r"sub\V.srt", answers(r"C:\work\sub\V.srt")),
            r"\\?\C:\work\sub\V.srt"
        );
        assert_eq!(
            first_argument(r"\Music\V.srt", answers(r"\\srv\sh\Music\V.srt")),
            r"\\?\UNC\srv\sh\Music\V.srt"
        );
    }

    #[test]
    fn a_long_path_is_made_absolute_by_windows_then_given_the_prefix() {
        let part = long_part();
        let tidy = format!(r"C:\Music\{part}\V.en.srt");
        assert_eq!(
            first_argument(&format!("C:/Music/./{part}/V.en.srt"), answers(&tidy)),
            format!(r"\\?\{tidy}")
        );
        let share = format!(r"\\server\share\Music\{part}\V.en.srt");
        assert_eq!(
            first_argument(&share, answers(&share)),
            format!(r"\\?\UNC\server\share\Music\{part}\V.en.srt")
        );
        let device = format!(r"\\.\C:\{part}\V.srt");
        assert_eq!(
            first_argument(&device, answers(&device)),
            format!(r"\\?\C:\{part}\V.srt")
        );
    }

    #[test]
    fn both_rename_arguments_are_converted() {
        let part = long_part();
        let (from, to) = rename_arguments_for_windows(
            Path::new(&format!(r"C:\{part}\a.srt")),
            Path::new(r"sub\b.srt"),
            |path: &Path| {
                Ok(if path.to_string_lossy().starts_with("sub") {
                    std::path::PathBuf::from(r"C:\work\sub\b.srt")
                } else {
                    path.to_path_buf()
                })
            },
        )
        .unwrap();
        assert_eq!(
            String::from_utf16(&from).unwrap(),
            format!("\\\\?\\C:\\{part}\\a.srt\0")
        );
        assert_eq!(
            String::from_utf16(&to).unwrap(),
            "\\\\?\\C:\\work\\sub\\b.srt\0"
        );
    }

    #[test]
    fn a_zero_character_or_a_failure_to_make_a_path_absolute_is_an_error() {
        let err =
            rename_arguments_for_windows(Path::new("a\0b"), Path::new(r"C:\x"), never_absolute)
                .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        let err = rename_arguments_for_windows(
            Path::new(r"sub\V.srt"),
            Path::new(r"C:\x"),
            |_: &Path| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    /// The same, with Windows' real resolution. Not run on a Mac; these
    /// first run in CI's Windows job.
    #[cfg(windows)]
    #[test]
    fn on_windows_relative_and_long_paths_get_windows_own_answer() {
        let current = std::env::current_dir().unwrap();
        let current = current.to_string_lossy().into_owned();
        let convert = |path: &str| {
            let (from, _) = windows_rename_arguments(Path::new(path), Path::new(r"C:\x")).unwrap();
            String::from_utf16(&from[..from.len() - 1]).unwrap()
        };
        // Relative to the current folder.
        assert_eq!(convert(r"sub\V.srt"), format!(r"\\?\{current}\sub\V.srt"));
        // Drive-relative on the current drive: the current folder, never
        // the drive's root (round 8, finding 2). Another drive's remembered
        // folder cannot be set up in a test.
        if current.as_bytes().get(1) == Some(&b':') {
            let drive = &current[..1];
            assert_eq!(
                convert(&format!(r"{drive}:sub\V.srt")),
                format!(r"\\?\{current}\sub\V.srt")
            );
        }
        // Long, with `/`, `.` and `..` tidied by Windows.
        let part = long_part();
        assert_eq!(
            convert(&format!(r"{current}\x\.\y\..\{part}/V.srt")),
            format!(r"\\?\{current}\x\{part}\V.srt")
        );
        // Short and ordinary: passed on as it is.
        assert_eq!(convert(r"C:\Music\V.srt"), r"C:\Music\V.srt");
    }

    /// The real rename, on a path over Windows' old 260-character limit. A
    /// test program does not declare itself long-path aware, so this fails
    /// if `rename_no_replace` stops taking its arguments from
    /// `windows_rename_arguments` (or that stops converting). First runs
    /// in CI's Windows job.
    #[cfg(windows)]
    #[test]
    fn on_windows_the_no_replace_rename_works_on_a_long_path() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join(long_part());
        fs::create_dir(&deep).unwrap();
        let (from, to) = (deep.join("a.srt"), deep.join("b.srt"));
        assert!(to.as_os_str().len() > 260, "{}", to.display());
        fs::write(&from, "a").unwrap();
        rename_no_replace(&from, &to).unwrap();
        assert_eq!(fs::read_to_string(&to).unwrap(), "a");
        assert!(!from.exists());
    }

    // ── copy_to_new_file (stand-in review of round 6, finding 1) ────────

    #[test]
    fn copy_to_new_file_copies_to_a_free_name_and_leaves_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&from, "subtitle text").unwrap();
        copy_to_new_file(&from, &to).unwrap();
        assert_eq!(fs::read_to_string(&to).unwrap(), "subtitle text");
        assert_eq!(fs::read_to_string(&from).unwrap(), "subtitle text");
    }

    #[test]
    fn copy_to_new_file_refuses_a_taken_name_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&from, "ours").unwrap();
        fs::write(&to, "theirs").unwrap();
        let err = copy_to_new_file(&from, &to).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists, "{err}");
        assert_eq!(fs::read_to_string(&to).unwrap(), "theirs");
        assert_eq!(fs::read_to_string(&from).unwrap(), "ours");
    }

    /// A failure after the new file was created deletes it again. Made to
    /// fail part way by copying from a FOLDER: opening one for reading
    /// works on macOS and Linux, and reading it then fails, after the new
    /// file exists. (Windows refuses to open a folder at all, before
    /// anything is created, so this is Unix only.)
    #[cfg(unix)]
    #[test]
    fn copy_to_new_file_deletes_the_file_it_made_when_the_copy_fails() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a folder");
        fs::create_dir(&from).unwrap();
        let to = dir.path().join("b");
        let err = copy_to_new_file(&from, &to).unwrap_err();
        assert!(!left_a_file_in_place(&err), "{err}");
        assert!(!to.exists(), "the partly written file was left behind");
    }

    /// Codex's review of rounds 6-7, finding 1: between the failed copy and
    /// its clean-up, the new file is moved away (or deleted) and another
    /// file is put at the name. That other file must survive; the error
    /// says a file was left there. Unix only, as above.
    #[cfg(unix)]
    #[test]
    fn a_file_put_at_the_name_before_a_failed_copy_cleans_up_survives() {
        for moved_away in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let from = dir.path().join("a folder");
            fs::create_dir(&from).unwrap();
            let to = dir.path().join("b");
            let moved = dir.path().join("moved");
            let err = copy_to_new_file_with(
                &from,
                &to,
                |to| {
                    if moved_away {
                        fs::rename(to, &moved).unwrap();
                    } else {
                        fs::remove_file(to).unwrap();
                    }
                    fs::write(to, "somebody else's file").unwrap();
                },
                &mut |_| {},
            )
            .unwrap_err();
            assert_eq!(
                fs::read_to_string(&to).ok().as_deref(),
                Some("somebody else's file"),
                "moved away first: {moved_away}"
            );
            assert!(left_a_file_in_place(&err), "{err}");
            let message = err.to_string();
            assert!(message.contains("a file was left at"), "{message}");
            assert!(message.contains(&to.display().to_string()), "{message}");
            assert!(
                message.contains("a different file now has that name"),
                "{message}"
            );
        }
    }

    /// A failed copy whose file cannot be moved aside leaves it where it is
    /// and says so: it is never deleted by name instead (Codex's review of
    /// round 8, finding 1). Made so by taking away permission to change the
    /// folder (Unix; skipped as the superuser, who may change it anyway).
    #[cfg(unix)]
    #[test]
    fn a_failed_copy_that_cannot_move_its_file_aside_leaves_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        fs::create_dir(&work).unwrap();
        let from = dir.path().join("a folder");
        fs::create_dir(&from).unwrap();
        let to = work.join("b");
        let mut blocked = false;
        let result = copy_to_new_file_with(
            &from,
            &to,
            |to| {
                let folder = to.parent().unwrap();
                fs::set_permissions(folder, fs::Permissions::from_mode(0o555)).unwrap();
                blocked = fs::write(folder.join("probe"), "").is_err();
            },
            &mut |_| {},
        );
        fs::set_permissions(&work, fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            eprintln!("skipped: this user may change a read-only folder");
            return;
        }
        let err = result.unwrap_err();
        assert!(left_a_file_in_place(&err), "{err}");
        assert!(
            err.to_string().contains("could not be moved aside"),
            "{err}"
        );
        assert!(to.exists());
        assert_eq!(
            fs::read_dir(&work).unwrap().count(),
            1,
            "something else was left"
        );
    }

    /// A failed copy whose file was moved aside but cannot be checked there
    /// puts it back and leaves it. Made so by taking away permission to look
    /// inside the folder while it is aside, and giving it back before the
    /// file is put back (Unix; skipped as the superuser).
    #[cfg(unix)]
    #[test]
    fn a_failed_copy_that_cannot_check_its_file_puts_it_back() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        fs::create_dir(&work).unwrap();
        let from = dir.path().join("a folder");
        fs::create_dir(&from).unwrap();
        let to = work.join("b");
        let mut blocked = false;
        let result = copy_to_new_file_with(&from, &to, |_| {}, &mut |step| match step {
            RemovalStep::BeforeCheck { at } => {
                fs::set_permissions(&work, fs::Permissions::from_mode(0o600)).unwrap();
                blocked = fs::symlink_metadata(at).is_err();
            }
            RemovalStep::AfterCheck { .. } => {
                fs::set_permissions(&work, fs::Permissions::from_mode(0o755)).unwrap();
            }
        });
        fs::set_permissions(&work, fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            eprintln!("skipped: this user may look inside a folder without permission");
            return;
        }
        let err = result.unwrap_err();
        assert!(left_a_file_in_place(&err), "{err}");
        assert!(err.to_string().contains("could not be checked"), "{err}");
        assert!(to.exists(), "the file was not put back");
        assert_eq!(
            fs::read_dir(&work).unwrap().count(),
            1,
            "something else was left"
        );
    }

    /// What another program does in the removal tests: if the name still
    /// holds a file (on Windows nothing is moved aside first), moves it
    /// away; then puts its own finished file there.
    fn put_replacement_at(name: &Path) {
        if name.exists() {
            fs::rename(name, name.with_file_name("moved away")).unwrap();
        }
        fs::write(name, "somebody else's finished file").unwrap();
    }

    /// Codex's review of round 8, finding 1: a failed copy has checked that
    /// the new name holds its own file, and before the clean-up deletes it,
    /// another program puts a finished file at that name. That file must
    /// survive. The old clean-up deleted the name after checking it.
    #[cfg(unix)]
    #[test]
    fn a_file_put_at_the_name_after_the_check_survives_a_failed_copy() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a folder");
        fs::create_dir(&from).unwrap();
        let to = dir.path().join("V.en.srt");
        let err = copy_to_new_file_with(&from, &to, |_| {}, &mut |step| {
            if let RemovalStep::AfterCheck { .. } = step {
                put_replacement_at(&to);
            }
        })
        .unwrap_err();
        assert_eq!(
            fs::read_to_string(&to).ok().as_deref(),
            Some("somebody else's finished file"),
            "the file put there after the check was deleted"
        );
        assert!(!left_a_file_in_place(&err), "{err}");
        assert!(
            !folder_has_private_names(dir.path()),
            "a private name was left behind"
        );
    }

    /// Whether `dir` holds any `.discard` name made by `move_aside`.
    fn folder_has_private_names(dir: &Path) -> bool {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".discard"))
    }

    /// A file created here, with the handle that created it kept open.
    fn own_file(path: &Path, text: &str) -> fs::File {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut file, text.as_bytes()).unwrap();
        file
    }

    #[test]
    fn remove_if_still_ours_deletes_its_own_file_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        let handle = own_file(&path, "ours");
        assert!(matches!(
            remove_if_still_ours(&path, &handle),
            Removal::Removed
        ));
        drop(handle);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn remove_if_still_ours_leaves_a_file_that_took_the_name_before() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        let handle = own_file(&path, "ours");
        put_replacement_at(&path);
        let outcome = remove_if_still_ours(&path, &handle);
        assert!(matches!(outcome, Removal::LeftOtherFile), "{outcome:?}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "somebody else's finished file"
        );
        assert!(!folder_has_private_names(dir.path()));
        assert!(matches!(
            remove_if_still_ours(&dir.path().join("nothing"), &handle),
            Removal::AlreadyGone
        ));
    }

    /// Codex's review of round 8, finding 1, at the level of the shared
    /// removal: the file is checked and found to be ours, and THEN another
    /// file is put at the name, before the deletion. The new file survives,
    /// and ours is deleted.
    #[test]
    fn a_file_put_at_the_name_after_the_check_survives_removal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        let handle = own_file(&path, "ours");
        let outcome = remove_if_still_ours_with(&path, &handle, &mut |step| {
            if let RemovalStep::AfterCheck { .. } = step {
                put_replacement_at(&path);
            }
        });
        assert!(matches!(outcome, Removal::Removed), "{outcome:?}");
        assert_eq!(
            fs::read_to_string(&path).ok().as_deref(),
            Some("somebody else's finished file"),
            "the file put there after the check was deleted"
        );
        drop(handle);
        assert!(!folder_has_private_names(dir.path()));
    }

    /// Moved aside, found not to be ours, and its name taken again before
    /// it could be put back: it stays under the private name, nothing is
    /// deleted, and the private name is given (macOS and Linux only; Windows
    /// moves nothing aside).
    #[cfg(unix)]
    #[test]
    fn a_moved_aside_file_that_is_not_ours_stays_aside_if_its_name_is_taken_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        let handle = own_file(&path, "ours");
        put_replacement_at(&path);
        let outcome = remove_if_still_ours_with(&path, &handle, &mut |step| {
            if let RemovalStep::AfterCheck { .. } = step {
                fs::write(&path, "a third file").unwrap();
            }
        });
        let Removal::LeftAside { aside, why } = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(
            fs::read_to_string(&aside).unwrap(),
            "somebody else's finished file"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "a third file");
        assert!(why.contains("putting it back failed"), "{why}");
        assert!(is_temporary_subtitle_file(&aside), "{}", aside.display());
    }

    /// A file that cannot be moved aside is left where it is, never deleted
    /// by name instead. Made so by taking away permission to change the
    /// folder (Unix; skipped as the superuser).
    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_moved_aside_is_left_where_it_is() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        let handle = own_file(&path, "ours");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let blocked = fs::write(dir.path().join("probe"), "").is_err();
        let outcome = remove_if_still_ours(&path, &handle);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            eprintln!("skipped: this user may change a read-only folder");
            return;
        }
        assert!(matches!(outcome, Removal::NotMovedAside(_)), "{outcome:?}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "ours");
    }

    // ── The exclusive-placeholder path (the lead's decision after round
    // ── 9): where the one-step no-replace rename is "not supported", as on
    // ── an exFAT drive on a Mac whenever the new name is free. These
    // ── pretend it is unsupported, so they run on any machine; the
    // ── real-drive test runs them for real on exFAT.

    #[cfg(not(windows))]
    #[test]
    fn through_a_placeholder_removal_deletes_its_own_file_and_leaves_nothing() {
        as_if_one_step_rename_were_unsupported(|| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("V.en.srt");
            let handle = own_file(&path, "ours");
            let outcome = remove_if_still_ours(&path, &handle);
            assert!(matches!(outcome, Removal::Removed), "{outcome:?}");
            // Nothing at the name: no placeholder is left behind either.
            let outcome = remove_if_still_ours(&path, &handle);
            assert!(matches!(outcome, Removal::AlreadyGone), "{outcome:?}");
            drop(handle);
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        });
    }

    #[cfg(not(windows))]
    #[test]
    fn through_a_placeholder_a_file_put_at_the_name_after_the_check_survives() {
        as_if_one_step_rename_were_unsupported(|| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("V.en.srt");
            let handle = own_file(&path, "ours");
            let outcome = remove_if_still_ours_with(&path, &handle, &mut |step| {
                if let RemovalStep::AfterCheck { .. } = step {
                    put_replacement_at(&path);
                }
            });
            assert!(matches!(outcome, Removal::Removed), "{outcome:?}");
            assert_eq!(
                fs::read_to_string(&path).ok().as_deref(),
                Some("somebody else's finished file"),
                "the file put there after the check was deleted"
            );
            drop(handle);
            assert!(!folder_has_private_names(dir.path()));
        });
    }

    /// Not ours, on a drive without the one-step rename: KEPT under its
    /// private name, never put back (Codex's review of round 9, first
    /// finding: putting it back through a placeholder at its own name could
    /// overwrite a subtitle another program saved there in that instant).
    /// The original name is left free, and the outcome names the private
    /// name, so the person can rename it back. Round 9 put it back.
    #[cfg(not(windows))]
    #[test]
    fn through_a_placeholder_a_file_that_is_not_ours_is_kept_aside_not_put_back() {
        as_if_one_step_rename_were_unsupported(|| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("V.en.srt");
            let handle = own_file(&path, "ours");
            put_replacement_at(&path);
            let outcome = remove_if_still_ours(&path, &handle);
            let Removal::LeftAside { aside, why } = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(
                fs::read_to_string(&aside).unwrap(),
                "somebody else's finished file"
            );
            assert!(!path.exists(), "the file was put back");
            assert!(why.contains("cannot put a file back"), "{why}");
        });
    }

    /// The lead's test for that finding: another program saves a file at the
    /// original name AFTER the "not ours" decision. It survives untouched,
    /// and the file that is not ours stays under its private name.
    #[cfg(not(windows))]
    #[test]
    fn through_a_placeholder_a_file_appearing_at_the_name_after_the_not_ours_decision_survives() {
        as_if_one_step_rename_were_unsupported(|| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("V.en.srt");
            let handle = own_file(&path, "ours");
            put_replacement_at(&path);
            let outcome = remove_if_still_ours_with(&path, &handle, &mut |step| {
                if let RemovalStep::AfterCheck { .. } = step {
                    fs::write(&path, "another program's finished subtitle").unwrap();
                }
            });
            let Removal::LeftAside { aside, .. } = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                "another program's finished subtitle",
                "the file saved at the name after the decision was touched"
            );
            assert_eq!(
                fs::read_to_string(&aside).unwrap(),
                "somebody else's finished file"
            );
        });
    }

    /// A failed copy, through the placeholder path: it deletes its own file
    /// and leaves nothing; and a file put at the name after the check
    /// survives. (Copying from a folder: Unix only.)
    #[cfg(unix)]
    #[test]
    fn through_a_placeholder_a_failed_copy_deletes_only_its_own_file() {
        as_if_one_step_rename_were_unsupported(|| {
            let dir = tempfile::tempdir().unwrap();
            let from = dir.path().join("a folder");
            fs::create_dir(&from).unwrap();
            let work = dir.path().join("work");
            fs::create_dir(&work).unwrap();
            let to = work.join("V.en.srt");
            let err = copy_to_new_file(&from, &to).unwrap_err();
            assert!(!left_a_file_in_place(&err), "{err}");
            assert_eq!(fs::read_dir(&work).unwrap().count(), 0);

            let err = copy_to_new_file_with(&from, &to, |_| {}, &mut |step| {
                if let RemovalStep::AfterCheck { .. } = step {
                    put_replacement_at(&to);
                }
            })
            .unwrap_err();
            assert!(!left_a_file_in_place(&err), "{err}");
            assert_eq!(
                fs::read_to_string(&to).unwrap(),
                "somebody else's finished file"
            );
            assert_eq!(fs::read_dir(&work).unwrap().count(), 1);
        });
    }

    /// The private name is the temporary prefix, the process id, 16 random
    /// hexadecimal digits and `.discard`, so the lyrics steps skip it and
    /// the leftover report counts it.
    #[cfg(unix)]
    #[test]
    fn the_private_name_has_the_temporary_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("V.en.srt");
        fs::write(&path, "x").unwrap();
        let aside = move_aside(&path).unwrap().unwrap();
        let name = aside.file_name().unwrap().to_string_lossy().into_owned();
        let random = name
            .strip_prefix(&format!(
                "{TEMPORARY_SUBTITLE_PREFIX}{}-",
                std::process::id()
            ))
            .and_then(|rest| rest.strip_suffix(".discard"))
            .unwrap_or_else(|| panic!("{name}"));
        assert_eq!(random.len(), 16, "{name}");
        assert!(is_temporary_subtitle_file(&aside));
        assert_eq!(aside.parent(), path.parent());
        assert!(
            matches!(move_aside(&path), Ok(None)),
            "the name is gone now"
        );
    }

    /// When the new file has been removed by someone else and nothing has
    /// taken its name, there is nothing to delete: only the copy's own
    /// failure is reported.
    #[cfg(unix)]
    #[test]
    fn a_failed_copy_whose_file_is_already_gone_reports_only_the_failure() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a folder");
        fs::create_dir(&from).unwrap();
        let to = dir.path().join("b");
        let err = copy_to_new_file_with(&from, &to, |to| fs::remove_file(to).unwrap(), &mut |_| {})
            .unwrap_err();
        assert!(!left_a_file_in_place(&err), "{err}");
        assert!(!to.exists());
    }

    #[test]
    fn only_not_supported_errors_count_as_not_supported() {
        use std::io::{Error, ErrorKind};
        let unsupported = Error::from(ErrorKind::Unsupported);
        assert!(is_hard_link_unsupported(&unsupported));
        assert!(is_no_replace_rename_unsupported(&unsupported));
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::NotFound,
            ErrorKind::AlreadyExists,
            ErrorKind::StorageFull,
        ] {
            assert!(!is_hard_link_unsupported(&Error::from(kind)), "{kind:?}");
            assert!(
                !is_no_replace_rename_unsupported(&Error::from(kind)),
                "{kind:?}"
            );
        }
        #[cfg(target_os = "macos")]
        {
            assert!(is_hard_link_unsupported(&Error::from_raw_os_error(
                libc::ENOTSUP
            )));
            assert!(!is_hard_link_unsupported(&Error::from_raw_os_error(
                libc::EACCES
            )));
            assert!(is_no_replace_rename_unsupported(&Error::from_raw_os_error(
                libc::EINVAL
            )));
        }
        #[cfg(target_os = "linux")]
        {
            assert!(is_hard_link_unsupported(&Error::from_raw_os_error(
                libc::EPERM
            )));
            assert!(!is_hard_link_unsupported(&Error::from_raw_os_error(
                libc::EACCES
            )));
            assert!(is_no_replace_rename_unsupported(&Error::from_raw_os_error(
                libc::EINVAL
            )));
        }
    }

    // ── file_identity (Codex's review of round 5, finding 4) ───────────

    /// The identity read from the handle that created a file matches what
    /// its name shows, until the name is given to a different file.
    #[test]
    fn handle_identity_tells_the_created_file_from_a_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let made = handle_identity(&file).unwrap();
        assert!(file_identity(&path).unwrap().is_same_file(&made));
        fs::rename(&path, dir.path().join("moved")).unwrap();
        fs::write(&path, "replacement").unwrap();
        assert!(!file_identity(&path).unwrap().is_same_file(&made));
        assert!(file_identity(&dir.path().join("moved"))
            .unwrap()
            .is_same_file(&made));
    }

    /// The four answers of `check_name_against_handle`: the same file
    /// (also after it was filled through another open, as ffmpeg fills a
    /// temporary file), a different file put at the name after the original
    /// was renamed away or deleted, and no file at all.
    #[test]
    fn check_name_against_handle_tells_own_file_replacement_and_gone() {
        let dir = tempfile::tempdir().unwrap();
        let open_new = |path: &Path| {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap()
        };
        let a = dir.path().join("a");
        let handle = open_new(&a);
        fs::write(&a, "filled through another open").unwrap();
        assert!(matches!(
            check_name_against_handle(&a, &handle),
            NameCheck::SameFile
        ));
        fs::rename(&a, dir.path().join("moved")).unwrap();
        assert!(matches!(
            check_name_against_handle(&a, &handle),
            NameCheck::Gone
        ));
        fs::write(&a, "replacement").unwrap();
        assert!(matches!(
            check_name_against_handle(&a, &handle),
            NameCheck::OtherFile
        ));

        let b = dir.path().join("b");
        let handle = open_new(&b);
        fs::remove_file(&b).unwrap();
        fs::write(&b, "replacement").unwrap();
        assert!(matches!(
            check_name_against_handle(&b, &handle),
            NameCheck::OtherFile
        ));
    }

    #[test]
    fn file_identity_sees_two_names_of_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        fs::write(&a, "x").unwrap();
        fs::write(&c, "x").unwrap();
        assert_eq!(file_identity(&a).unwrap().links, 1);
        fs::hard_link(&a, &b).unwrap();
        let (ia, ib, ic) = (
            file_identity(&a).unwrap(),
            file_identity(&b).unwrap(),
            file_identity(&c).unwrap(),
        );
        assert_eq!(ia, ib);
        assert_eq!(ia.links, 2);
        assert_ne!((ia.device, ia.index), (ic.device, ic.index));
    }
}
