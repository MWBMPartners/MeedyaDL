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

/// Is Finder's "this is an alias" flag set on this file?
///
/// Reads `com.apple.FinderInfo`, the 32 bytes Finder keeps beside a file.
/// For a file, bytes 8 and 9 are its flags, most significant byte first,
/// and 0x8000 is the alias flag. Finder sets it on every alias it makes,
/// whatever the format of the alias itself — which is the point: it is
/// one question that does not depend on knowing every format.
///
/// Returns `Ok(false)` for exactly one reason: there is no such
/// attribute, which is the ordinary case for an ordinary file. Every
/// other outcome — including an attribute too short to hold the flags —
/// comes back as an error, so the caller refuses rather than assumes.
/// "Cannot tell" and "no" are never the same answer here.
#[cfg(target_os = "macos")]
fn macos_alias_flag(path: &std::path::Path) -> Result<bool, String> {
    use std::os::unix::ffi::OsStrExt;

    const FINDER_INFO: &std::ffi::CStr = c"com.apple.FinderInfo";
    const ALIAS_FLAG: u16 = 0x8000;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "the file name contains a zero byte".to_string())?;

    let mut buf = [0u8; 32];
    // `XATTR_NOFOLLOW` matters: ask about THIS file, not about whatever
    // it might point at. Asking about the far end is the mistake that
    // made the original hole.
    let read = unsafe {
        libc::getxattr(
            c_path.as_ptr(),
            FINDER_INFO.as_ptr(),
            buf.as_mut_ptr().cast::<libc::c_void>(),
            buf.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };

    if read < 0 {
        let err = std::io::Error::last_os_error();
        // "There is no such attribute" is the normal answer for a normal
        // file, not a failure — most files have no Finder information at
        // all. That is the ONE failure that counts as a genuine no.
        //
        // The name to use is ENOATTR. A previous version of this comment
        // said it shares a number with ENODATA; it does not. Compiled and
        // printed on a Mac rather than assumed: ENOATTR is 93 and ENODATA
        // is 96. They are different errors and swapping them would stop
        // ordinary files being recognised as ordinary.
        if err.raw_os_error() == Some(libc::ENOATTR) {
            return Ok(false);
        }
        return Err(err.to_string());
    }

    let read = read as usize;
    if read < 10 {
        // Present, but too short to hold the flags. That is "cannot
        // tell", not "no" — and this function's whole promise is that
        // those two are never confused. A reviewer caught the first
        // version answering "no" here, which contradicted the rule the
        // rest of the function follows.
        return Err(format!(
            "the file's Finder information is only {read} bytes, too short to read"
        ));
    }

    let flags = u16::from_be_bytes([buf[8], buf[9]]);
    Ok(flags & ALIAS_FLAG != 0)
}

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
/// handed in, rather than whatever it might lead to.
///
/// macOS has a second kind that the first check cannot see: a Finder
/// alias is an ORDINARY FILE, so it answers "yes, a file" and "no, not a
/// shortcut", and macOS follows it anyway. That is checked for
/// separately, by its contents. A reviewer found that gap too.
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

    // macOS has a second kind of shortcut the check above cannot see: a
    // Finder alias. Unlike a symbolic link, an alias is an ORDINARY FILE
    // — so "is this a file?" says yes and "is this a shortcut?" says no
    // — and macOS follows it to whatever it points at and opens that.
    // An independent reviewer found the gap.
    //
    // # Why the Finder flag, and not the file's contents
    //
    // The first attempt at this looked for the twelve bytes a modern
    // alias begins with. That was checked against a real alias, and it
    // was right about modern ones — but the same reviewer pointed out it
    // only describes ONE format. An older alias keeps its target
    // somewhere else entirely and can have no contents at all, so
    // reading its first twelve bytes finds nothing and it walks straight
    // through. Describing formats is the losing game: the list is always
    // one entry short, which is the same reason the extension check
    // below is a list of what IS allowed.
    //
    // So this asks the question Finder itself answers: **is the alias
    // flag set?** Finder sets it on every alias it makes, whatever the
    // format inside, which is what makes this independent of format.
    // Verified on a real alias on a real Mac rather than assumed — its
    // `com.apple.FinderInfo` came back 32 bytes, type code `alis`,
    // creator `MACS`, and the flag at 0x8000 set.
    //
    // # It refuses when it cannot tell
    //
    // Not being able to read the answer is not the same as the answer
    // being no. A file that cannot be inspected is refused, because the
    // alternative is that anything unreadable gets the benefit of the
    // doubt from a check written to withhold exactly that.
    #[cfg(target_os = "macos")]
    {
        match macos_alias_flag(path) {
            Ok(true) => {
                return Err(
                    "That is an alias to another file, and MeedyaDL does not open aliases. \
                     Open it from Finder if you meant to."
                        .to_string(),
                );
            }
            Ok(false) => {}
            Err(why) => {
                return Err(format!(
                    "MeedyaDL could not check whether that is an alias, so it has not opened \
                     it ({why}). Open it from Finder if you meant to."
                ));
            }
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

    /// A macOS alias wearing a music name must be refused.
    ///
    /// An alias is NOT a symbolic link: it is an ordinary file, so the
    /// check above says "yes, a file" and "no, not a shortcut" — and
    /// macOS follows it anyway. An independent reviewer found that gap.
    ///
    /// # Why this makes its own alias instead of asking Finder
    ///
    /// It used to ask Finder, and print a message and return early when
    /// Finder could not be reached. A reviewer pointed out that is a
    /// **silently passing test**: the runner hides output from tests
    /// that pass, so on a machine with no desktop it went green while
    /// proving nothing — exactly the thing its own comment promised not
    /// to do.
    ///
    /// So it sets the flag itself and always runs. The flag's layout was
    /// confirmed against a REAL alias made by Finder on a real Mac: 32
    /// bytes, type code `alis`, creator `MACS`, flag 0x8000 set. This
    /// writes that same shape, so it tests the real check against the
    /// real thing Finder marks.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_macos_alias_wearing_a_music_name_is_refused() {
        use std::os::unix::ffi::OsStrExt;

        let dir = std::env::temp_dir().join(format!("meedyadl-alias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // An ordinary file, wearing a name from the allowed list.
        let disguise = dir.join("track.m4a");
        std::fs::write(&disguise, b"whatever an alias happens to hold").unwrap();

        // Before the flag: it passes every other check, which is the
        // whole problem this test exists for.
        let meta = std::fs::symlink_metadata(&disguise).unwrap();
        assert!(!meta.file_type().is_symlink(), "an alias is not a symbolic link");
        assert!(meta.is_file(), "and it answers yes to being a file");
        assert!(OPENABLE_EXTENSIONS.contains(&"m4a"), "and its name is allowed");

        // Now mark it the way Finder marks an alias.
        let mut finder_info = [0u8; 32];
        finder_info[0..4].copy_from_slice(b"alis");
        finder_info[4..8].copy_from_slice(b"MACS");
        finder_info[8..10].copy_from_slice(&0x8000u16.to_be_bytes());

        let c_path = std::ffi::CString::new(disguise.as_os_str().as_bytes()).unwrap();
        let set = unsafe {
            libc::setxattr(
                c_path.as_ptr(),
                c"com.apple.FinderInfo".as_ptr(),
                finder_info.as_ptr().cast::<libc::c_void>(),
                finder_info.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        assert_eq!(
            set,
            0,
            "could not set the flag, so this test would have proved nothing: {}",
            std::io::Error::last_os_error()
        );

        // The check should now read that flag and refuse.
        assert!(
            macos_alias_flag(&disguise).unwrap(),
            "the flag was set, so the check must see it"
        );

        let result = open_downloaded_file(disguise.to_string_lossy().into_owned());
        assert!(result.is_err(), "an alias must be refused however it is named");
        assert!(
            result.unwrap_err().contains("alias"),
            "and the person should be told why"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that cannot be looked at is refused, not waved through.
    ///
    /// Not being able to read the answer is not the same as the answer
    /// being no. A reviewer pointed out the first version treated every
    /// failure to inspect as "not an alias", which hands the benefit of
    /// the doubt to exactly the files a check like this exists to
    /// withhold it from.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_file_that_cannot_be_checked_is_refused() {
        // A name with a zero byte in it cannot be asked about at all.
        // That is the simplest way to reach the "could not tell" branch
        // without depending on how a particular machine handles
        // permissions (a test run as an administrator can read anything,
        // so a permissions-based test would pass for the wrong reason).
        let bad = std::path::PathBuf::from("/tmp/meedyadl\0alias.m4a");
        assert!(
            macos_alias_flag(&bad).is_err(),
            "a name it cannot even ask about must be an error, not a no"
        );
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
