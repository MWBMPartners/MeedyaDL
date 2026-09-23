// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download history IPC command handlers.
// ========================================
//
// Thin wrappers around `services::history_service` that expose
// history operations to the React frontend via Tauri's `invoke()`.
//
// Commands:
//   - `list_history`   -- Returns all history entries (newest first)
//   - `clear_history`  -- Deletes all history entries
//   - `search_history` -- Case-insensitive search on title/artist/album/URL

use tauri::AppHandle;

use crate::services::history_service;
use crate::services::history_service::HistoryEntry;

/// Returns all download history entries, sorted newest first.
///
/// Called by the frontend to populate the History page.
#[tauri::command]
pub fn list_history(app: AppHandle) -> Vec<HistoryEntry> {
    history_service::list_history(&app)
}

/// Deletes all download history entries from disk.
///
/// Called by the frontend's "Clear History" button.
#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    history_service::clear_history(&app);
    Ok(())
}

/// Searches history entries by a case-insensitive substring match
/// on title, artist, album, and URL fields.
///
/// Called by the frontend's search input on the History page.
#[tauri::command]
pub fn search_history(app: AppHandle, query: String) -> Vec<HistoryEntry> {
    history_service::search_history(&app, &query)
}

/// Removes a single history entry by ID (#685).
///
/// Sibling of `clear_history` (bulk). Returns `Err` only when the ID
/// doesn't match any row, so the frontend can surface "already gone"
/// distinctly from a no-op.
#[tauri::command]
pub fn delete_history_entry(app: AppHandle, id: String) -> Result<(), String> {
    if history_service::delete_entry(&app, &id) {
        Ok(())
    } else {
        Err(format!("History entry {id} not found"))
    }
}

/// Aggregate lifetime download analytics (#464). Returns totals,
/// success rate, codec distribution, top artist / album, and
/// last-7-day activity computed on-demand from the persistent history.
///
/// **Frontend caller:** `getLifetimeStats()` in
/// `src/lib/tauri-commands.ts`, wired to the StatisticsPanel.
///
/// Pure read — no side effects, safe to call as often as the UI
/// likes. Computed on each call rather than cached because the
/// history is small (≤1000 entries per MAX_HISTORY_ENTRIES) and
/// the aggregation is sub-ms.
#[tauri::command]
pub fn get_lifetime_stats(
    app: AppHandle,
) -> crate::services::stats_service::LifetimeStats {
    crate::services::stats_service::get_lifetime_stats(&app)
}

/// Resolves the folder to reveal for a history entry's stored path (#992).
///
/// `HistoryEntry.file_path` is ambiguous by construction: album downloads
/// record the album *directory*, while single-file downloads record the
/// *file* itself. There's no `output_is_directory` flag on the struct (and
/// older rows predate any such flag), so this stats the path directly —
/// the path itself if it is a directory, otherwise its parent.
///
/// Called by the frontend's "Open Folder" action instead of the old
/// unconditional `path.replace(/[/\\][^/\\]+$/, '')` regex strip, which
/// incorrectly climbed one level above album directories and revealed the
/// parent artist folder instead of the album folder.
///
/// Falls back to the input string unchanged if it has no parent (e.g. a
/// root path) — the caller's regex-based fallback handles the case where
/// this command itself is unreachable (IPC failure).
/// The kinds of file MeedyaDL will open for you.
///
/// An allow-list, not a list of things to refuse. The difference matters
/// here more than usual: on Windows, "opening" a program RUNS it, and a
/// list of dangerous extensions is always one spelling short — `.EXE` in
/// capitals, or some file type that turns out to be executable on one
/// system and not another.
const OPENABLE_EXTENSIONS: &[&str] = &[
    // Music, which is the point of the app.
    "m4a", "m4b", "m4p", "mp3", "flac", "wav", "aac", "ogg", "opus", "aiff", "alac",
    // Video, for music videos.
    "mp4", "m4v", "mov", "mkv", "webm", // Artwork.
    "jpg", "jpeg", "png", "webp", "gif", // Words that come with the music.
    "lrc", "srt", "vtt", "ass", "ttml", "txt", "lyrics", // The app's own records.
    "json", "meedyadl", "log", "m3u", "m3u8", "cue", "pdf",
];

/// Opens a downloaded file in whatever program handles its type.
///
/// # Why this exists rather than the page opening it directly
///
/// The page used to call the "open this path" plugin itself, with
/// permission to open **any path at all**. On Windows, opening a program
/// runs it — so anything that could run code inside the page had, in
/// effect, "run any file on this computer". That is a large power to
/// hand to a web page, and the feature it was there for is "open the
/// track I just downloaded".
///
/// A full review of the codebase found it, and also found the capability
/// file describing those permissions as "scoped just to that" when the
/// scope was in fact everything.
///
/// So the page no longer has that permission. It asks here instead, and
/// this checks:
///
/// * the path is not a shortcut to somewhere else (see below);
/// * it exists and is a FILE, not a folder or anything stranger;
/// * its extension is one of [`OPENABLE_EXTENSIONS`], compared without
///   regard to case, so `.EXE` is refused exactly as `.exe` is.
///
/// # Why shortcuts are refused
///
/// The first version of this check did not refuse them, and that left
/// the hole it was written to close still open. "Is this a file?"
/// follows a shortcut to whatever it points at, while "what is its
/// extension?" reads the name it was handed. So a shortcut *named*
/// `track.mp3` pointing at a program satisfied both: the first question
/// answered yes, because the program at the far end is a file, and the
/// second answered `mp3`, because that is what the name says. Then the
/// real path — the shortcut — went to the system, which follows it and
/// launches the program.
///
/// An independent reviewer found that. It is the same shape as the
/// original fault, one layer down: the thing being checked and the thing
/// being acted on were not the same thing.
///
/// A file this app downloaded is never a shortcut, so refusing them
/// outright costs nothing and needs no guessing about where one leads.
/// Both questions are now asked about the same path — the one that was
/// handed in, without following anything.
///
/// Folders are revealed rather than opened, through a separate
/// permission, which selects the item in the file manager instead of
/// running anything.
///
/// **What this does not do:** it does not check the file is inside the
/// download folder. People save music to external drives and network
/// shares, and to their own folders outside anything this app chose, so
/// a location check would refuse ordinary use.
///
/// It also does not look inside the file. An extension says what
/// something is called, not what it contains. That is accepted: the list
/// exists to stop the app being a way to run a program, and renaming a
/// program to `.mp3` does not make the system run it — the system
/// decides by the same extension this check reads.
#[tauri::command]
pub fn open_downloaded_file(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);

    // Asked FIRST, and deliberately about the path itself rather than
    // what it might point at. `symlink_metadata` is the one that does
    // not follow shortcuts; `is_file` does. See the note above for what
    // asking them in the wrong order cost.
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(
                "That is a shortcut to another file, and MeedyaDL does not open shortcuts. \
                 Open it from your file manager if you meant to."
                    .to_string(),
            );
        }
        Ok(meta) if !meta.is_file() => {
            return Err("That is not a file.".to_string());
        }
        Ok(_) => {}
        Err(_) => {
            return Err("That is not a file, or it is no longer there.".to_string());
        }
    }

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if !OPENABLE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "MeedyaDL only opens music, video, artwork and text files. It will not open a \
             .{extension} file. Open it from your file manager if you meant to."
        ));
    }

    // Not `open_path` with a chosen program: the second argument is
    // `None`, which means "whatever this computer normally uses".
    tauri_plugin_opener::open_path(&file_path, None::<&str>)
        .map_err(|e| format!("Could not open that file: {e}"))
}

#[tauri::command]
pub fn resolve_reveal_path(file_path: String) -> String {
    let p = std::path::Path::new(&file_path);
    if p.is_dir() {
        file_path
    } else {
        p.parent()
            .map(|q| q.to_string_lossy().into_owned())
            .unwrap_or(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shortcut named like music, pointing at a program, must be
    /// refused — with a real shortcut on a real disk, not a stand-in.
    ///
    /// The first version of this check let it through: "is this a file?"
    /// follows the shortcut and says yes, while "what is its extension?"
    /// reads the name and says `mp3`. Both questions passed, and the
    /// system then followed the shortcut and launched the program at the
    /// far end. An independent reviewer found it.
    #[cfg(unix)]
    #[test]
    fn a_shortcut_wearing_a_music_name_is_refused() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!(
            "meedyadl-open-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);

        // Something that would run if it were opened.
        let target = dir.join("payload.sh");
        {
            let mut f = std::fs::File::create(&target).unwrap();
            writeln!(f, "#!/bin/sh\necho pwned").unwrap();
        }

        // A shortcut to it, wearing a name from the allowed list.
        let disguise = dir.join("track.mp3");
        let _ = std::fs::remove_file(&disguise);
        std::os::unix::fs::symlink(&target, &disguise).unwrap();

        // Both of the old checks would have passed on this path, so this
        // is a real demonstration rather than a restatement of the code.
        assert!(
            disguise.is_file(),
            "the old file check followed the shortcut and said yes"
        );
        assert_eq!(
            disguise.extension().and_then(|e| e.to_str()),
            Some("mp3"),
            "the old extension check read the name and said mp3"
        );

        let result = open_downloaded_file(disguise.to_string_lossy().into_owned());
        assert!(
            result.is_err(),
            "a shortcut must be refused however innocent its name looks"
        );
        assert!(
            result.unwrap_err().contains("shortcut"),
            "and the person should be told why"
        );

        let _ = std::fs::remove_file(&disguise);
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_dir(&dir);
    }

    /// An ordinary file is still opened — or rather, still gets past
    /// every check, which is as far as a test can go without actually
    /// launching something.
    #[test]
    fn an_ordinary_music_file_still_passes_the_checks() {
        let dir = std::env::temp_dir().join(format!(
            "meedyadl-open-ok-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let track = dir.join("real track.m4a");
        std::fs::write(&track, b"not really audio, but that is not what is checked").unwrap();

        // It is not a shortcut and its kind is on the list, so the only
        // thing that can fail now is the launch itself, which a test
        // cannot ask for. So check the two refusals do NOT fire.
        let meta = std::fs::symlink_metadata(&track).unwrap();
        assert!(!meta.file_type().is_symlink());
        assert!(meta.is_file());
        assert!(OPENABLE_EXTENSIONS.contains(&"m4a"));

        let _ = std::fs::remove_file(&track);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn only_music_video_artwork_and_text_may_be_opened() {
        // On Windows, opening a program runs it. The page used to be
        // able to ask for any path at all, which made this a way to run
        // any file on the computer from anything that could run code in
        // the page. A full review of the codebase found it.
        //
        // An allow-list, so a file type nobody thought of is refused
        // rather than permitted.
        for allowed in ["m4a", "mp4", "flac", "jpg", "lrc", "json", "meedyadl"] {
            assert!(
                OPENABLE_EXTENSIONS.contains(&allowed),
                "{allowed} is something this app produces and should open"
            );
        }
        for refused in [
            "exe", "bat", "cmd", "com", "scr", "msi", "lnk", "ps1", "vbs", "sh", "command", "app",
            "jar", "dll", "so", "dylib", "reg", "hta", "pkg", "deb",
        ] {
            assert!(
                !OPENABLE_EXTENSIONS.contains(&refused),
                "{refused} can run code and must never be opened"
            );
        }
    }

    #[test]
    fn a_capital_letter_does_not_get_a_program_past_the_check() {
        // The reason the check lower-cases before comparing: a list of
        // dangerous extensions is always one spelling short, and
        // `PAYLOAD.EXE` is the spelling people try. Here the list is of
        // what is ALLOWED, so the same lower-casing means `TRACK.M4A`
        // works while `PAYLOAD.EXE` does not.
        let lowered = |name: &str| {
            std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default()
        };

        assert!(OPENABLE_EXTENSIONS.contains(&lowered("Track.M4A").as_str()));
        assert!(OPENABLE_EXTENSIONS.contains(&lowered("Cover.JPG").as_str()));
        assert!(!OPENABLE_EXTENSIONS.contains(&lowered("payload.EXE").as_str()));
        assert!(!OPENABLE_EXTENSIONS.contains(&lowered("payload.Bat").as_str()));
        // Nothing at all is not an extension we accept.
        assert!(!OPENABLE_EXTENSIONS.contains(&lowered("README").as_str()));
    }

    /// A real directory resolves to itself — this is the album-download
    /// case that #992 was about (the stored path IS the folder to reveal).
    #[test]
    fn resolve_reveal_path_returns_directory_itself() {
        let dir = tempfile::TempDir::new().unwrap();
        let dir_path = dir.path().to_string_lossy().into_owned();

        let resolved = resolve_reveal_path(dir_path.clone());

        assert_eq!(resolved, dir_path);
    }

    /// A real file resolves to its parent directory — the single-file
    /// download case, matching the old regex-strip behaviour.
    #[test]
    fn resolve_reveal_path_returns_parent_of_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("track.m4a");
        std::fs::write(&file_path, b"fake audio").unwrap();

        let resolved = resolve_reveal_path(file_path.to_string_lossy().into_owned());

        assert_eq!(resolved, dir.path().to_string_lossy().into_owned());
    }

    /// A nonexistent path (e.g. the user moved/deleted the file after
    /// download) with a parent still resolves to that parent — `is_dir()`
    /// is `false` for missing paths, so the file branch (parent) applies.
    /// This preserves today's UX: users can still open the containing
    /// folder even if the exact file is gone.
    #[test]
    fn resolve_reveal_path_nonexistent_path_returns_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing_path = dir.path().join("no-such-file.m4a");

        let resolved = resolve_reveal_path(missing_path.to_string_lossy().into_owned());

        assert_eq!(resolved, dir.path().to_string_lossy().into_owned());
    }
}
