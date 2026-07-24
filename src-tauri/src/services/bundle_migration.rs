// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// First-launch migration from the pre-2026-07-24 bundle identifier.
// ====================================================================
//
// MeedyaDL's Tauri `identifier` was renamed from `io.github.meedyadl` to
// `com.meedyasuite.meedyadl`. Because Tauri derives both the app-data
// directory AND (indirectly, via our own `SERVICE_NAME` constants) the
// convention used for OS-keychain entries from that identifier, the
// rename silently orphaned every existing user's on-disk settings, queue,
// history, logs, crash reports, portable Python runtime, external tools --
// AND every keychain-stored credential (MusicKit private key, web player
// developer token, dev-access sentinel).
//
// This module runs once, early in `lib.rs::run()`'s `.setup()` hook
// (before anything else reads/writes the new app-data directory or the
// queue/settings/history files), and copies the user's data across so
// nothing is lost. It is intentionally:
//
//   - **Non-destructive**: everything is COPIED, never MOVED. The old
//     directory and old keychain entries are left fully intact as a
//     safety net -- if this migration has a bug, the user's original
//     data is still sitting exactly where it always was.
//   - **Idempotent**: gated by a marker file written into the NEW app
//     data directory once the migration has been attempted. Re-running
//     the app never re-copies anything.
//   - **Resilient**: every individual copy/keychain operation is wrapped
//     so a single failure (permission error, disk full, keychain locked)
//     logs a warning and moves on to the next item. A failed migration
//     must never prevent the app from starting -- worst case the user
//     re-enters a credential or re-downloads the portable Python runtime.
//
// See `utils::platform::{OLD_BUNDLE_IDENTIFIER, CURRENT_BUNDLE_IDENTIFIER}`
// for the two identifier constants this module bridges between.

use crate::utils::platform::{CURRENT_BUNDLE_IDENTIFIER, OLD_BUNDLE_IDENTIFIER};
use std::path::{Path, PathBuf};

/// Marker file written into the NEW app data directory once this
/// migration has been attempted (successfully or not -- see module doc:
/// individual failures are logged and skipped, not retried forever).
/// Its presence is the sole gate for "has this already run".
const MIGRATION_MARKER_FILENAME: &str = ".migrated-from-io.github.meedyadl";

/// Top-level flat files copied from the old app-data root into the new
/// one, if the new copy doesn't already exist.
const DATA_FILES: &[&str] = &["settings.json", "settings.json.sha256", "queue.json", "history.json"];

/// Directories merged (per-file, skip-if-exists) from the old app-data
/// root into the new one. Unlike `RUNTIME_DIRS`, these are small and
/// grow incrementally, so a per-file merge (rather than an all-or-nothing
/// directory copy) is the more useful semantics here.
const MERGED_DIRS: &[&str] = &["logs", "crashes"];

/// Large, all-or-nothing directories copied wholesale from the old
/// app-data root into the new one, ONLY when the destination doesn't
/// exist at all yet (the portable Python runtime and external tools are
/// each an internally-consistent installation; a partial merge could
/// leave a broken hybrid).
const RUNTIME_DIRS: &[&str] = &["python", "tools"];

/// Keychain accounts (the `keyring::Entry` "account" half of the
/// (service, account) pair -- see `commands::credentials::SERVICE_NAME`
/// and `services::apple_music_api::SERVICE_NAME`) that this app has ever
/// written under the shared `SERVICE_NAME` keychain service. Enumerated
/// by grepping every `keyring::Entry::new(SERVICE_NAME, ...)` call site
/// in the codebase as of the 2026-07-24 rename:
/// - `musickit_private_key` (`apple_music_api::get/store_private_key_from/in_keychain`,
///   also reachable via `commands::credentials`'s generic store/get/delete
///   allowlist)
/// - `webplayer_developer_token` (`apple_music_api::get/store/clear_webplayer_token_*`)
/// - `dev_access_token` (`commands::credentials`'s Konami-code dev access
///   sentinel)
///
/// If a future PR adds another `keyring::Entry::new(SERVICE_NAME, ...)`
/// call site, add its account key here too -- this list does NOT update
/// itself.
const KEYCHAIN_ACCOUNTS: &[&str] = &[
    "musickit_private_key",
    "webplayer_developer_token",
    "dev_access_token",
];

/// Runs the one-time first-launch migration. Must be called from
/// `lib.rs::run()`'s `.setup()` hook, as early as possible -- specifically
/// BEFORE any code reads `settings.json` / `queue.json` / `history.json`
/// from the new app-data directory, so the "is this a fresh install"
/// check below can't be fooled by files this same launch already wrote.
///
/// `new_app_data_dir` is the already-created (via `create_dir_all`)
/// current app-data root, i.e. `platform::get_app_data_dir(app.handle())`
/// -- passed in rather than recomputed so the caller's existing
/// `std::fs::create_dir_all` call and log line stay the single source of
/// truth for "the new directory exists".
///
/// Returns a short human-readable summary of what was migrated (empty if
/// nothing was), so the caller can optionally surface it via the activity
/// log. Never panics; every fallible step is individually logged and
/// skipped on error.
///
/// # Fresh-install fast path
/// The very first thing this function does is check whether the OLD
/// (`io.github.meedyadl`) app-data directory genuinely contains our data
/// (concretely: has a `settings.json`). If it doesn't -- the overwhelming
/// common case, both for brand new installs and for every launch after
/// the first on an upgraded machine once the marker exists -- the
/// function returns immediately: no marker file is written, no directory
/// is created, and NO keychain probing happens. A fresh install must be a
/// true no-op, paying only the cost of a couple of `stat` calls per
/// launch.
///
/// Only when that check passes (there is real legacy data to consider)
/// do we go on to check the migration marker and, if genuinely needed,
/// perform the filesystem copy + keychain migration.
#[must_use]
pub fn run(new_app_data_dir: &Path) -> Vec<String> {
    run_inner(legacy_app_data_dir(), new_app_data_dir)
}

/// Testable core of [`run`], parameterised over the OLD app-data
/// directory rather than resolving it internally via `dirs::data_dir()`.
/// This lets unit tests exercise every branch (fresh install / legacy
/// data present with no marker / legacy data present with marker) with a
/// synthetic temp directory instead of depending on -- or risking
/// mutating -- whatever the real OS data directory and OS keychain
/// happen to contain on the machine running the tests.
fn run_inner(old_app_data_dir: Option<PathBuf>, new_app_data_dir: &Path) -> Vec<String> {
    // --- Fast path: nothing to migrate -----------------------------------
    // This MUST be the first thing the function does, and must not create
    // any directories, files, or touch the keychain. See doc comment.
    let Some(old_app_data_dir) = old_app_data_dir else {
        log::debug!("Could not determine the OS data directory -- skipping migration check.");
        return Vec::new();
    };
    if !old_app_data_dir.join("settings.json").is_file() {
        log::debug!(
            "No legacy app data at {} -- fresh install (or already migrated + cleaned up by \
             the user), migration is a no-op.",
            old_app_data_dir.display()
        );
        return Vec::new();
    }

    // From here on we know there is genuinely legacy data on disk, so
    // it's worth paying for the marker-file idempotency check.
    let marker_path = new_app_data_dir.join(MIGRATION_MARKER_FILENAME);
    if marker_path.exists() {
        // Already attempted on a previous launch (regardless of outcome).
        // Filesystem copies are one-shot by design -- see module doc.
        return Vec::new();
    }

    log::info!(
        "Legacy app data found at {} -- running one-time bundle-identifier migration \
         ('{OLD_BUNDLE_IDENTIFIER}' -> '{CURRENT_BUNDLE_IDENTIFIER}')",
        old_app_data_dir.display()
    );

    let mut summary: Vec<String> = Vec::new();

    migrate_filesystem_data(&old_app_data_dir, new_app_data_dir, &mut summary);
    migrate_keychain_entries(&mut summary);

    // Write the marker LAST and unconditionally (best-effort), so this
    // function never runs its (potentially slow, e.g. copying a portable
    // Python runtime) filesystem pass more than once per install, even if
    // individual items above failed. The old directory and old keychain
    // entries are untouched, so a user who hits a partial-migration bug
    // can always be pointed at manual recovery by support -- deleting
    // this marker file re-arms the migration for the next launch.
    let marker_contents = if summary.is_empty() {
        "Legacy directory was present but nothing new was migrated (destination already had \
         this data).\n"
            .to_string()
    } else {
        format!("{}\n", summary.join("\n"))
    };
    if let Err(e) = std::fs::write(&marker_path, marker_contents) {
        log::warn!(
            "Failed to write migration marker at {} ({e}) -- migration will be re-attempted \
             on next launch. This is non-fatal (all migration steps are idempotent / \
             skip-if-exists), just slightly wasteful.",
            marker_path.display()
        );
    }

    if summary.is_empty() {
        log::info!(
            "Bundle-identifier migration: legacy directory found but destination already \
             provisioned -- nothing new migrated."
        );
    } else {
        log::info!("Bundle-identifier migration complete: {}", summary.join("; "));
    }

    summary
}

/// Filesystem half of the migration: settings/queue/history, logs,
/// crashes, and the portable Python + tools installations. Only called
/// once `run()` has already confirmed `old_app_data_dir` genuinely has
/// our data and the migration marker is absent.
fn migrate_filesystem_data(old_app_data_dir: &Path, new_app_data_dir: &Path, summary: &mut Vec<String>) {
    // Only migrate into a genuinely fresh new-identifier install. If
    // `settings.json` already exists in the new directory, either a
    // previous launch already migrated it (but somehow lost the marker --
    // extra safety net) or the user already has independent data there;
    // either way we must not overwrite it.
    if new_app_data_dir.join("settings.json").exists() {
        log::debug!(
            "New app data directory already has settings.json -- skipping filesystem migration."
        );
        return;
    }

    log::info!(
        "Copying (not moving) legacy app data from {} into {}",
        old_app_data_dir.display(),
        new_app_data_dir.display()
    );

    // --- Flat data files -------------------------------------------------
    for name in DATA_FILES {
        let old_file = old_app_data_dir.join(name);
        let new_file = new_app_data_dir.join(name);
        if new_file.exists() || !old_file.is_file() {
            continue;
        }
        match std::fs::copy(&old_file, &new_file) {
            Ok(_) => {
                log::info!("Migrated {name} from legacy app data directory");
                summary.push(format!("copied {name}"));
            }
            Err(e) => {
                log::warn!("Failed to migrate {name} from legacy app data directory: {e} (non-fatal)");
            }
        }
    }

    // --- Merged directories (logs, crashes) ------------------------------
    for name in MERGED_DIRS {
        let old_dir = old_app_data_dir.join(name);
        if !old_dir.is_dir() {
            continue;
        }
        let new_dir = new_app_data_dir.join(name);
        match copy_dir_recursive(&old_dir, &new_dir, true) {
            Ok(0) => {}
            Ok(count) => {
                log::info!("Migrated {count} file(s) from legacy {name}/ directory");
                summary.push(format!("copied {count} file(s) in {name}/"));
            }
            Err(e) => {
                log::warn!("Failed to migrate legacy {name}/ directory: {e} (non-fatal)");
            }
        }
    }

    // --- Runtime directories (python, tools) ------------------------------
    // All-or-nothing: only copied when the destination doesn't exist yet,
    // to avoid producing a hybrid/broken installation from a partial
    // merge of two independently-provisioned runtimes.
    for name in RUNTIME_DIRS {
        let old_dir = old_app_data_dir.join(name);
        if !old_dir.is_dir() {
            continue;
        }
        let new_dir = new_app_data_dir.join(name);
        if new_dir.exists() {
            log::debug!(
                "{name}/ already present in new app data directory -- skipping runtime migration"
            );
            continue;
        }
        log::info!("Migrating legacy {name}/ runtime directory -- this may take a moment...");
        match copy_dir_recursive(&old_dir, &new_dir, false) {
            Ok(count) => {
                log::info!("Migrated legacy {name}/ runtime directory ({count} file(s))");
                summary.push(format!("copied {name}/ runtime ({count} file(s))"));
            }
            Err(e) => {
                log::warn!(
                    "Failed to migrate legacy {name}/ runtime directory: {e} (non-fatal -- \
                     the app will re-provision it if needed, at the cost of a re-download)."
                );
            }
        }
    }
}

/// Keychain half of the migration. For each known account, copies the
/// secret from the OLD service name to the NEW one, but only when the NEW
/// entry doesn't already exist (never overwrite) and the OLD entry does
/// (nothing to migrate otherwise). The OLD entry is left in place.
///
/// Only called from `run()` after it has already confirmed the legacy
/// FILESYSTEM directory genuinely contains our data. This is a deliberate
/// trade-off: it means the rare case of a user who deleted the legacy
/// app-data directory (or never had one, e.g. a synced/scripted install)
/// but still has leftover legacy keychain entries won't get those
/// migrated automatically. That's judged acceptable in exchange for the
/// stronger guarantee that a fresh install performs ZERO keychain
/// probing -- see `run()`'s doc comment for the fast-path rationale.
fn migrate_keychain_entries(summary: &mut Vec<String>) {
    for account in KEYCHAIN_ACCOUNTS {
        match migrate_single_keychain_entry(account) {
            Ok(true) => {
                log::info!("Migrated keychain entry '{account}' to the new app identifier");
                summary.push(format!("migrated keychain entry '{account}'"));
            }
            Ok(false) => {
                // Nothing to do: either no legacy entry existed, or the
                // new entry was already present.
            }
            Err(e) => {
                log::warn!(
                    "Keychain migration for '{account}' failed (non-fatal -- the user may need \
                     to re-enter this credential): {e}"
                );
            }
        }
    }
}

/// Migrates a single (service, account) keychain entry from the OLD
/// service name to the NEW one. Returns `Ok(true)` if a value was
/// actually copied, `Ok(false)` if there was nothing to do (no legacy
/// value, or a new value already present), and `Err` on an unexpected
/// keychain backend error (locked, permission denied, unavailable).
fn migrate_single_keychain_entry(account: &str) -> Result<bool, String> {
    // Check the NEW entry first -- if it already has a value we must not
    // overwrite it (the user may have re-entered credentials manually).
    let new_entry = keyring::Entry::new(CURRENT_BUNDLE_IDENTIFIER, account)
        .map_err(|e| format!("failed to open new keychain entry: {e}"))?;
    match new_entry.get_password() {
        Ok(_) => return Ok(false),
        Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(format!("failed to check new keychain entry: {e}")),
    }

    let old_entry = keyring::Entry::new(OLD_BUNDLE_IDENTIFIER, account)
        .map_err(|e| format!("failed to open legacy keychain entry: {e}"))?;
    let value = match old_entry.get_password() {
        Ok(v) => v,
        // Nothing stored under the old identifier -- nothing to migrate.
        Err(keyring::Error::NoEntry) => return Ok(false),
        Err(e) => return Err(format!("failed to read legacy keychain entry: {e}")),
    };

    // Deliberately does NOT delete `old_entry` -- see module doc
    // "non-destructive" guarantee. A stale-but-harmless duplicate in the
    // old service namespace is an acceptable trade for never being able
    // to lose a credential to a migration bug.
    new_entry
        .set_password(&value)
        .map_err(|e| format!("failed to write new keychain entry: {e}"))?;

    Ok(true)
}

/// Resolves the OLD app-data root directory (pre-2026-07-24 identifier),
/// using the same `dirs::data_dir()`-based construction that
/// `platform::get_app_data_dir`'s own fallback branch and
/// `platform::resolve_early_app_data_root()` already rely on for
/// consistency with how Tauri itself derives `app_data_dir()` on macOS /
/// Windows / Linux (identifier appended directly to the OS data-dir
/// root). Returns `None` only if the `dirs` crate can't determine the OS
/// data directory at all (exceedingly rare).
fn legacy_app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(OLD_BUNDLE_IDENTIFIER))
}

/// Recursively copies the contents of `src` into `dst`, creating
/// directories as needed. Returns the number of files actually copied.
///
/// When `skip_existing_files` is `true`, a destination file that already
/// exists is left untouched (merge semantics -- used for `logs/`,
/// `crashes/`). When `false`, every source file is copied unconditionally
/// (used for `python/`, `tools/`, which are only ever invoked against a
/// destination that doesn't exist yet -- see call site guard above).
///
/// Symlinks are skipped rather than followed, to avoid the (unlikely, but
/// possible on a hand-edited install) risk of an infinite loop or copying
/// data outside the intended tree; none of our own managed directories
/// (`logs/`, `crashes/`, `python/`, `tools/`) legitimately contain
/// symlinks.
fn copy_dir_recursive(src: &Path, dst: &Path, skip_existing_files: bool) -> std::io::Result<u64> {
    std::fs::create_dir_all(dst)?;
    let mut copied = 0u64;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_symlink() {
            continue;
        } else if file_type.is_dir() {
            copied += copy_dir_recursive(&src_path, &dst_path, skip_existing_files)?;
        } else if file_type.is_file() {
            if skip_existing_files && dst_path.exists() {
                continue;
            }
            std::fs::copy(&src_path, &dst_path)?;
            copied += 1;
        }
    }

    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal legacy-style app-data tree under a fresh temp dir
    /// for use as `src` in `copy_dir_recursive` tests.
    fn make_tree(root: &Path, files: &[&str]) {
        for rel in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, rel).unwrap();
        }
    }

    #[test]
    fn copy_dir_recursive_copies_nested_files() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let src = tmp.join("src");
        let dst = tmp.join("dst");
        make_tree(&src, &["a.txt", "sub/b.txt", "sub/deeper/c.txt"]);

        let copied = copy_dir_recursive(&src, &dst, false).unwrap();
        assert_eq!(copied, 3);
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("sub/b.txt").exists());
        assert!(dst.join("sub/deeper/c.txt").exists());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn copy_dir_recursive_skip_existing_does_not_overwrite() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let src = tmp.join("src");
        let dst = tmp.join("dst");
        make_tree(&src, &["a.txt"]);
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("a.txt"), "already-here").unwrap();

        let copied = copy_dir_recursive(&src, &dst, true).unwrap();
        assert_eq!(copied, 0);
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "already-here");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn copy_dir_recursive_no_skip_overwrites() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let src = tmp.join("src");
        let dst = tmp.join("dst");
        make_tree(&src, &["a.txt"]);
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("a.txt"), "old-value").unwrap();

        let copied = copy_dir_recursive(&src, &dst, false).unwrap();
        assert_eq!(copied, 1);
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "a.txt");

        std::fs::remove_dir_all(&tmp).ok();
    }

    // ------------------------------------------------------------------
    // `run_inner` is used (rather than the public `run`) for every test
    // below that needs to control whether a "legacy directory" exists, so
    // tests are deterministic and never depend on -- or risk reading --
    // whatever the real OS data directory happens to contain on the
    // machine running the tests.
    //
    // None of these tests exercise `migrate_keychain_entries` end-to-end
    // (i.e. none reach the "legacy dir has settings.json AND no marker"
    // branch), matching this codebase's existing convention (see
    // `commands::credentials::tests`) of not exercising real OS-keychain
    // I/O from unit tests -- CI/sandboxed runners may have no Secret
    // Service / Credential Manager / Keychain backend available at all.
    // ------------------------------------------------------------------

    #[test]
    fn run_inner_is_a_true_noop_when_old_dir_does_not_exist() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let old_dir = tmp.join("old-does-not-exist");
        let new_dir = tmp.join("new");
        std::fs::create_dir_all(&new_dir).unwrap();

        let summary = run_inner(Some(old_dir), &new_dir);

        assert!(summary.is_empty());
        // Fresh-install fast path: must not create the marker file, and
        // must not create anything else inside the new directory either.
        assert!(!new_dir.join(MIGRATION_MARKER_FILENAME).exists());
        assert_eq!(std::fs::read_dir(&new_dir).unwrap().count(), 0);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn run_inner_is_a_true_noop_when_old_dir_exists_but_has_no_settings_json() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let old_dir = tmp.join("old");
        let new_dir = tmp.join("new");
        // Old directory exists but is empty / foreign (no settings.json)
        // -- e.g. some unrelated leftover directory, not genuine legacy
        // MeedyaDL data.
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::create_dir_all(&new_dir).unwrap();

        let summary = run_inner(Some(old_dir), &new_dir);

        assert!(summary.is_empty());
        assert!(!new_dir.join(MIGRATION_MARKER_FILENAME).exists());
        assert_eq!(std::fs::read_dir(&new_dir).unwrap().count(), 0);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn run_inner_short_circuits_when_marker_already_present() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let old_dir = tmp.join("old");
        let new_dir = tmp.join("new");
        // Old directory DOES have genuine legacy data...
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("settings.json"), "{\"legacy\":true}").unwrap();
        std::fs::write(old_dir.join("queue.json"), "[]").unwrap();
        // ...but the new directory already has the marker from a prior
        // launch, so nothing should be (re-)copied.
        std::fs::create_dir_all(&new_dir).unwrap();
        std::fs::write(new_dir.join(MIGRATION_MARKER_FILENAME), "already ran").unwrap();

        let summary = run_inner(Some(old_dir), &new_dir);

        assert!(summary.is_empty());
        assert!(!new_dir.join("settings.json").exists());
        assert!(!new_dir.join("queue.json").exists());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn does_not_migrate_filesystem_when_new_settings_already_exists() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let old_dir = tmp.join("old");
        let new_dir = tmp.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("settings.json"), "{\"legacy\":true}").unwrap();
        std::fs::create_dir_all(&new_dir).unwrap();
        std::fs::write(new_dir.join("settings.json"), "{\"already\":\"here\"}").unwrap();

        let mut summary = Vec::new();
        migrate_filesystem_data(&old_dir, &new_dir, &mut summary);
        assert!(summary.is_empty());
        // The existing (new-directory) settings.json must be untouched.
        assert_eq!(
            std::fs::read_to_string(new_dir.join("settings.json")).unwrap(),
            "{\"already\":\"here\"}"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn migrate_filesystem_data_copies_flat_files_and_merged_dirs() {
        let tmp = std::env::temp_dir().join(format!("meedyadl-migration-test-{}", uuid::Uuid::new_v4()));
        let old_dir = tmp.join("old");
        let new_dir = tmp.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::create_dir_all(&new_dir).unwrap();

        std::fs::write(old_dir.join("settings.json"), "{\"legacy\":true}").unwrap();
        std::fs::write(old_dir.join("settings.json.sha256"), "deadbeef").unwrap();
        std::fs::write(old_dir.join("queue.json"), "[]").unwrap();
        std::fs::write(old_dir.join("history.json"), "[]").unwrap();
        make_tree(&old_dir.join("logs"), &["meedyadl.2026-07-01.log"]);
        make_tree(&old_dir.join("crashes"), &["crash-20260701.json"]);

        let mut summary = Vec::new();
        migrate_filesystem_data(&old_dir, &new_dir, &mut summary);

        assert!(!summary.is_empty());
        assert!(new_dir.join("settings.json").exists());
        assert!(new_dir.join("settings.json.sha256").exists());
        assert!(new_dir.join("queue.json").exists());
        assert!(new_dir.join("history.json").exists());
        assert!(new_dir.join("logs/meedyadl.2026-07-01.log").exists());
        assert!(new_dir.join("crashes/crash-20260701.json").exists());
        // Non-destructive: legacy copies must still be present too.
        assert!(old_dir.join("settings.json").exists());
        assert!(old_dir.join("logs/meedyadl.2026-07-01.log").exists());

        std::fs::remove_dir_all(&tmp).ok();
    }
}
