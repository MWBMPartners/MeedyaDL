// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Keeping a file that could not be read, before something overwrites it.
//!
//! MeedyaDL keeps three things on disk between runs: your settings, the queue,
//! and your download history. If one of them cannot be read at startup — a
//! crash part-way through saving, a bad patch of disk, an edit by hand that
//! broke the format — the app carried on with nothing: default settings, an
//! empty queue, or an empty history.
//!
//! From the outside that looks exactly like your download history vanishing.
//! The only trace was a line in a log file, and for the history it was written
//! at debug level, which most people never see even if they go looking.
//!
//! **The worse part was what happened next.** Nothing had been said, so the
//! person carried on using the app, and the first thing that saved wrote a
//! fresh file over the unreadable one. Whatever was in it was then gone for
//! good. Backups exist, but the newest is taken when the app closes, so it can
//! already contain the damage.
//!
//! This module makes that survivable. When a file cannot be read it is moved
//! aside first, under a name that says what it was and when, so nothing can
//! overwrite it. Then the person is told plainly, in the activity log where
//! they can actually see it.
//!
//! It deliberately does **not** try to repair anything, and does not restore a
//! backup on the person's behalf. An older backup could quietly undo settings
//! they changed since. Offering is right; deciding for them is not.

use std::path::{Path, PathBuf};

use tauri::AppHandle;

/// Moves a file that could not be read out of the way, so nothing overwrites it.
///
/// The copy keeps its original name with `.damaged` and a number added, so it
/// is obvious what it was and several can sit side by side without clashing.
///
/// Renaming rather than copying, because it is one step: there is never a
/// moment where both the original and the copy exist and something could write
/// to the original in between.
///
/// # Arguments
///
/// * `path` -- The file that could not be read.
///
/// # Returns
///
/// Where it was moved to, or `None` if it could not be moved — in which case
/// nothing has been lost that was not already lost, and the caller carries on.
pub fn preserve_damaged_file(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        // Nothing to preserve. A missing file is not damage — it is a first
        // run, or someone deliberately clearing things out.
        return None;
    }

    let target = next_available_name(path)?;
    match std::fs::rename(path, &target) {
        Ok(()) => {
            log::warn!(
                "could not read {}, kept a copy at {}",
                path.display(),
                target.display()
            );
            Some(target)
        }
        Err(e) => {
            // Worth knowing about, but not worth stopping for. The file was
            // already unreadable; failing to move it leaves things exactly as
            // they were before this existed.
            log::warn!("could not set aside the unreadable file {}: {e}", path.display());
            None
        }
    }
}

/// Finds a name to move the file to that is not already taken.
///
/// Numbered rather than timestamped so the name stays short and readable, and
/// so this needs no clock — which also makes it testable.
fn next_available_name(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let directory = path.parent()?;

    // Try the plain name first; it is the one people will see most often.
    let first = directory.join(format!("{name}.damaged"));
    if !first.exists() {
        return Some(first);
    }

    // Then numbered, for repeat occurrences. Bounded so a directory that
    // somehow fills up with these cannot spin here for ever.
    (2..=99)
        .map(|n| directory.join(format!("{name}.damaged-{n}")))
        .find(|candidate| !candidate.exists())
}

/// Tells the user, in ordinary words, that a saved file could not be read.
///
/// Goes to the activity log rather than a pop-up: it happens at startup, when
/// a dialogue would be in the way, and the activity log is where MeedyaDL
/// already explains itself.
///
/// # Arguments
///
/// * `app` -- Used to reach the activity log.
/// * `what` -- What the file holds, in the words a person would use:
///   "settings", "download queue", "download history".
/// * `kept_at` -- Where the unreadable file was moved to, if it could be moved.
/// * `consequence` -- What the person will notice, in their words. For example
///   "Your history looks empty" — said plainly, because that is the thing they
///   are about to see and be alarmed by.
pub fn report_damaged_file(
    app: &AppHandle,
    what: &str,
    kept_at: Option<&Path>,
    consequence: &str,
) {
    let mut message = format!("Your saved {what} could not be read. {consequence}");
    if let Some(path) = kept_at {
        // Naming the file matters: it is the difference between "your history
        // is gone" and "your history is in this file and can be recovered".
        message.push_str(&format!(
            " The unreadable file has been kept at {} so nothing overwrites it.",
            path.display()
        ));
    }
    message.push_str(
        " You can restore an earlier copy from Settings, under Backups. Nothing has been \
         restored automatically, because an older copy could undo changes you made since.",
    );
    crate::utils::activity_log::emit_app_log(app, &message);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes a temporary directory that cleans itself up.
    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("should be able to make a temporary directory")
    }

    #[test]
    fn a_missing_file_is_not_treated_as_damage() {
        // A first run, or someone clearing things out deliberately. Nothing to
        // preserve and nothing to worry anyone about.
        let dir = temp_dir();
        assert_eq!(preserve_damaged_file(&dir.path().join("history.json")), None);
    }

    #[test]
    fn an_unreadable_file_is_moved_out_of_the_way() {
        // The point of the whole module: the next save must not be able to
        // overwrite it.
        let dir = temp_dir();
        let original = dir.path().join("history.json");
        std::fs::write(&original, "{ this is not valid json").unwrap();

        let kept = preserve_damaged_file(&original).expect("should have been moved");

        assert!(!original.exists(), "the original must be out of the way");
        assert!(kept.exists(), "the copy must exist");
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "{ this is not valid json",
            "the contents must survive untouched — this is the whole point"
        );
    }

    #[test]
    fn the_name_says_what_it_was() {
        let dir = temp_dir();
        let original = dir.path().join("settings.json");
        std::fs::write(&original, "broken").unwrap();

        let kept = preserve_damaged_file(&original).unwrap();
        let name = kept.file_name().unwrap().to_str().unwrap();

        assert!(name.starts_with("settings.json"), "got {name}");
        assert!(name.contains("damaged"), "got {name}");
    }

    #[test]
    fn a_second_failure_does_not_overwrite_the_first() {
        // Someone whose disk is failing could hit this repeatedly. Losing the
        // first copy to the second would defeat the purpose.
        let dir = temp_dir();
        let original = dir.path().join("queue.json");

        std::fs::write(&original, "first damage").unwrap();
        let first = preserve_damaged_file(&original).unwrap();

        std::fs::write(&original, "second damage").unwrap();
        let second = preserve_damaged_file(&original).unwrap();

        assert_ne!(first, second, "the two copies must have different names");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "first damage");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "second damage");
    }

    #[test]
    fn many_repeat_failures_still_each_get_their_own_name() {
        let dir = temp_dir();
        let original = dir.path().join("history.json");
        let mut seen = std::collections::HashSet::new();

        for i in 0..10 {
            std::fs::write(&original, format!("damage {i}")).unwrap();
            let kept = preserve_damaged_file(&original).expect("should keep each one");
            assert!(seen.insert(kept.clone()), "name {kept:?} was reused");
        }
    }

    #[test]
    fn preserving_never_destroys_what_it_cannot_move() {
        // If the move fails the file must still be there. Nothing is worse
        // than a recovery step that loses the thing it was recovering.
        let dir = temp_dir();
        let original = dir.path().join("history.json");
        std::fs::write(&original, "content").unwrap();

        // Fill every candidate name so no move is possible.
        std::fs::write(dir.path().join("history.json.damaged"), "x").unwrap();
        for n in 2..=99 {
            std::fs::write(dir.path().join(format!("history.json.damaged-{n}")), "x").unwrap();
        }

        assert_eq!(preserve_damaged_file(&original), None);
        assert!(original.exists(), "the original must not be destroyed");
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "content");
    }
}
