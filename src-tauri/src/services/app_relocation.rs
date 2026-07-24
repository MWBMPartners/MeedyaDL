// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! macOS self-relocation: "tidy MeedyaDL into `/Applications/MeedyaSuite`" (#1057).
//!
//! ## Background
//!
//! Every MeedyaSuite app installs to the root of `/Applications` by
//! default (that's where the `.dmg`'s `/Applications` symlink drops it).
//! As more MeedyaSuite apps ship, users end up with a flat pile of
//! unrelated-looking icons instead of a recognisable family. Two
//! DMG-side fixes were considered and rejected:
//!
//!   - Repointing the DMG's `/Applications` symlink at a subfolder
//!     breaks silently on any Mac that doesn't already have that
//!     subfolder (Finder can't drag onto a symlink target that
//!     doesn't exist).
//!   - Shipping a `MeedyaSuite` folder *inside* the DMG and asking the
//!     user to drag that folder onto `/Applications` risks Finder's
//!     "An item with the same name already exists — Replace?" prompt
//!     silently deleting every *other* MeedyaSuite app already
//!     installed there, if the user says yes without reading closely.
//!
//! Instead, the app relocates **itself**, once, the first time it
//! launches from a bare `/Applications/*.app` install. This module is
//! the whole implementation: a pure, unit-testable eligibility
//! predicate, and the (macOS-only) move + relaunch logic.
//!
//! ## Why moving is safe
//!
//! - macOS code signatures are path-independent — moving a signed and
//!   notarized `.app` bundle does not invalidate its signature.
//! - `tauri-plugin-updater`'s `extract_path_from_executable()` derives
//!   the install path it updates in place from `current_exe()` at
//!   *update* time, walking up past `Contents/MacOS` to the `.app`
//!   bundle root. Once we've relocated, every future auto-update
//!   naturally lands at the new path — no separate manifest or
//!   settings change is needed to keep the updater working.
//!
//! ## Failure philosophy
//!
//! A failed relocation must never brick the app. [`perform_relocation`]
//! only ever exits the process on the SUCCESS path (where it
//! deliberately relaunches from the new location). Every failure path
//! logs the reason and returns an `Err` so the caller can surface it
//! and the app keeps running from its original location, exactly as
//! before the user clicked "Move".

use std::path::Path;

/// Name of the shared subfolder inside `/Applications` that MeedyaSuite
/// apps consolidate into.
const MEEDYASUITE_SUBDIR: &str = "MeedyaSuite";

/// Result of the startup relocation-eligibility check (#1057), computed
/// once in `lib.rs::run()`'s `.setup()` hook — immediately AFTER
/// `services::bundle_migration::run()` — and stored as Tauri managed
/// state via `.manage()`. The frontend reads it through the
/// `check_app_relocation` IPC command (`commands::app_relocation`)
/// rather than the setup hook doing any UI work itself.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppRelocationState {
    /// Whether MeedyaDL should offer to relocate itself this launch.
    /// Always `false` on non-macOS builds. On macOS, `false` whenever
    /// [`is_eligible_for_relocation`] rejects the current bundle path
    /// (already relocated, translocated by Gatekeeper, running from a
    /// mounted DMG, or installed somewhere other than a bare
    /// `/Applications/*.app`).
    pub eligible: bool,
    /// The proposed destination path (e.g.
    /// `/Applications/MeedyaSuite/MeedyaDL.app`), always present when
    /// `eligible` is `true` and always absent otherwise.
    pub destination: Option<String>,
}

/// Pure eligibility predicate — takes a path, returns a bool, touches
/// no filesystem or OS state. Kept free of any `#[cfg(target_os = ...)]`
/// gate so it compiles AND is exercised by `cargo test` on every CI
/// platform (Linux, Windows, macOS), not just the macOS runner that
/// happens to be able to execute the real relocation.
///
/// Offers relocation if and only if ALL of the following hold:
///   - `bundle_path`'s parent is exactly `/Applications` (not a
///     subfolder, not `~/Applications`, not anywhere else).
///   - `bundle_path` is not already under `/Applications/MeedyaSuite/`.
///   - `bundle_path` does not contain `/AppTranslocation/` (Gatekeeper's
///     quarantine path-randomisation — moving a translocated decoy path
///     would not move the real, installed `.app`).
///   - `bundle_path` does not start with `/Volumes/` (running straight
///     off a mounted DMG is a first install, not a "tidy up" step).
#[must_use]
pub fn is_eligible_for_relocation(bundle_path: &Path) -> bool {
    // The dominant check: must be a DIRECT child of /Applications.
    // Anything nested deeper (a subfolder, including MeedyaSuite/
    // itself), shallower (~/Applications), or on a different root
    // entirely fails here. This single condition already implies
    // "not already relocated" (see the explicit restatement below for
    // defence-in-depth) and "not a subfolder anywhere".
    if bundle_path.parent() != Some(Path::new("/Applications")) {
        return false;
    }

    // Explicit, independent restatement of "not already consolidated"
    // -- kept as its own check rather than relying solely on the
    // parent-equality check above, in case that check is ever loosened
    // to accept nested paths.
    if bundle_path.starts_with(Path::new("/Applications").join(MEEDYASUITE_SUBDIR)) {
        return false;
    }

    let path_str = bundle_path.to_string_lossy();

    // Gatekeeper path randomisation: a freshly-downloaded, quarantined
    // app can be re-executed from a throwaway path under
    // `.../AppTranslocation/<uuid>/d/` instead of its real install
    // location. Relocating from there would move a decoy, not the real
    // `.app` on disk.
    if path_str.contains("/AppTranslocation/") {
        return false;
    }

    // Running directly off a mounted DMG (double-clicked straight out
    // of the disk image, nothing dragged anywhere yet). That's a first
    // install, not a self-tidy step -- and the DMG volume isn't
    // writable in the way `rename()` needs anyway.
    if path_str.starts_with("/Volumes/") {
        return false;
    }

    true
}

// ============================================================
// macOS-only: computing the offer + performing the actual move.
// Every other target gets a no-op stub below so the crate builds
// (and the pure predicate above still runs its unit tests) on
// Linux and Windows CI.
// ============================================================

/// Computes the startup [`AppRelocationState`] from the CURRENT
/// process's own executable path. Cheap (a handful of `stat`-free path
/// operations, no filesystem I/O, no OS prompts) — safe to call
/// unconditionally from `.setup()`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn compute_startup_state() -> AppRelocationState {
    let Some(bundle) = current_bundle_path() else {
        return AppRelocationState { eligible: false, destination: None };
    };
    if !is_eligible_for_relocation(&bundle) {
        return AppRelocationState { eligible: false, destination: None };
    }
    match proposed_destination(&bundle) {
        Some(dest) => AppRelocationState {
            eligible: true,
            destination: Some(dest.display().to_string()),
        },
        None => AppRelocationState { eligible: false, destination: None },
    }
}

/// Non-macOS stub: relocation is never offered.
#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn compute_startup_state() -> AppRelocationState {
    AppRelocationState { eligible: false, destination: None }
}

/// Performs the actual relocation: creates `/Applications/MeedyaSuite`
/// if needed, moves the running app bundle into it, then relaunches
/// from the new location and exits this process.
///
/// Re-derives the bundle path and re-checks eligibility fresh (rather
/// than trusting the cached startup [`AppRelocationState`]), since this
/// is a user-initiated action that may happen a while after startup.
///
/// On success this function DOES NOT RETURN — it calls
/// [`relaunch_and_exit`], which relaunches the app from its new
/// location and terminates the current process via
/// `std::process::exit(0)`.
///
/// On ANY failure it logs the reason via [`emit_app_log`] and returns
/// `Err` — the caller (the `relocate_app_bundle` IPC command) surfaces
/// that to the frontend, and the app simply keeps running from its
/// original location. See the module-level "Failure philosophy" doc.
#[cfg(target_os = "macos")]
pub fn perform_relocation(app: &tauri::AppHandle) -> Result<(), String> {
    use crate::utils::activity_log::emit_app_log;

    let current_bundle = current_bundle_path()
        .ok_or_else(|| "Could not determine the running app's bundle path.".to_string())?;

    if !is_eligible_for_relocation(&current_bundle) {
        let msg = format!(
            "{} is no longer eligible for relocation (already moved, or launched from an \
             unexpected location).",
            current_bundle.display()
        );
        emit_app_log(app, &format!("App relocation declined by eligibility check: {msg}"));
        return Err(msg);
    }

    let destination = proposed_destination(&current_bundle)
        .ok_or_else(|| "Could not compute a destination path for the move.".to_string())?;
    let Some(destination_parent) = destination.parent() else {
        return Err("Destination has no parent directory.".to_string());
    };

    emit_app_log(
        app,
        &format!(
            "Relocating MeedyaDL from {} to {}...",
            current_bundle.display(),
            destination.display()
        ),
    );

    // Try the plain, no-privileges-needed path first: this succeeds for
    // the common case of a single-user Mac where the logged-in account
    // is an admin (macOS's default for the primary account) and
    // therefore already owns write access to /Applications.
    let standard_attempt: std::io::Result<()> = (|| {
        std::fs::create_dir_all(destination_parent)?;
        std::fs::rename(&current_bundle, &destination)?;
        Ok(())
    })();

    match standard_attempt {
        Ok(()) => {
            emit_app_log(
                app,
                &format!("Relocated MeedyaDL to {}. Relaunching...", destination.display()),
            );
            relaunch_and_exit(&destination)
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Standard (non-admin) user on this Mac: /Applications isn't
            // writable by them directly. Fall back to an
            // administrator-privileged move, mirroring
            // tauri-plugin-updater 2.10.1's own PermissionDenied
            // fallback in `updater.rs` (`install_inner()`).
            log::debug!(
                "Standard relocation move hit PermissionDenied ({e}) -- falling back to an \
                 administrator-privileged move."
            );
            match move_with_admin_privileges(&current_bundle, &destination) {
                Ok(()) => {
                    emit_app_log(
                        app,
                        &format!(
                            "Relocated MeedyaDL to {} with administrator approval. Relaunching...",
                            destination.display()
                        ),
                    );
                    relaunch_and_exit(&destination)
                }
                Err(admin_err) => {
                    let msg = format!(
                        "Moving MeedyaDL requires administrator approval, which failed or was \
                         declined: {admin_err}"
                    );
                    emit_app_log(app, &format!("App relocation failed: {msg}"));
                    Err(msg)
                }
            }
        }
        Err(e) => {
            let msg = format!("Failed to relocate the app bundle: {e}");
            emit_app_log(app, &format!("App relocation failed: {msg}"));
            Err(msg)
        }
    }
}

/// Non-macOS stub: relocation can never be performed.
#[cfg(not(target_os = "macos"))]
pub fn perform_relocation(_app: &tauri::AppHandle) -> Result<(), String> {
    Err("App relocation is only supported on macOS.".to_string())
}

/// Derives the `.app` bundle root from the currently running
/// executable's path (e.g.
/// `/Applications/MeedyaDL.app/Contents/MacOS/MeedyaDL` ->
/// `/Applications/MeedyaDL.app`), the same technique
/// `tauri-plugin-updater`'s `extract_path_from_executable()` uses.
#[cfg(target_os = "macos")]
fn current_bundle_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

/// Computes `/Applications/MeedyaSuite/<bundle file name>` for a given
/// current bundle path.
#[cfg(target_os = "macos")]
fn proposed_destination(bundle_path: &Path) -> Option<std::path::PathBuf> {
    let file_name = bundle_path.file_name()?;
    Some(Path::new("/Applications").join(MEEDYASUITE_SUBDIR).join(file_name))
}

/// Relaunches MeedyaDL from `new_bundle_path` and terminates the
/// current process. Never returns.
///
/// # Why not `tauri_plugin_process::relaunch()`?
///
/// That API re-executes the CURRENT process's executable path, captured
/// at process start -- but by the time this function runs, the `.app`
/// bundle has just been renamed out from under that path, so it no
/// longer exists there. Calling it here would simply fail to relaunch.
/// `relaunch()` remains the CORRECT choice for the in-place update flow
/// in `src/components/common/UpdateBanner.tsx`, where an update
/// overwrites files in place and the executable path never changes --
/// this relocation flow is the one situation in the codebase where the
/// executable path itself moves, hence the explicit `open -n <path>`
/// here instead.
#[cfg(target_os = "macos")]
fn relaunch_and_exit(new_bundle_path: &Path) -> ! {
    if let Err(e) = std::process::Command::new("open").arg("-n").arg(new_bundle_path).spawn() {
        log::error!(
            "Failed to relaunch MeedyaDL at its new location {}: {e}. The app has still been \
             moved successfully -- it can be launched manually from there.",
            new_bundle_path.display()
        );
    }
    std::process::exit(0);
}

/// Moves `src` to `dst` with elevated privileges via an AppleScript
/// `do shell script ... with administrator privileges` invocation --
/// the same pattern `tauri-plugin-updater` 2.10.1 uses for its own
/// `PermissionDenied` fallback (`updater.rs`, `install_inner()`),
/// triggering the native macOS admin-credentials prompt.
///
/// Invoked as a single `Command::new("osascript").arg(...)` call (no
/// `sh -c` on our side) -- the embedded shell command only runs inside
/// the AppleScript string, which is macOS's own mechanism for
/// elevating a privileged operation (the same one System Events and
/// Installer.app use), not a shell interpolation of untrusted input:
/// both `src` and `dst` are paths this module itself computed, never
/// user-supplied text.
#[cfg(target_os = "macos")]
fn move_with_admin_privileges(src: &Path, dst: &Path) -> Result<(), String> {
    let Some(dst_parent) = dst.parent() else {
        return Err("destination has no parent directory".to_string());
    };
    let script = format!(
        "do shell script \"mkdir -p '{parent}' && mv -f '{src}' '{dst}'\" with administrator privileges",
        parent = dst_parent.display(),
        src = src.display(),
        dst = dst.display(),
    );

    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map_err(|e| format!("failed to invoke osascript: {e}"))?;

    if !status.success() {
        return Err("the administrator-privileged move was cancelled or failed".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare `/Applications/MeedyaDL.app` install -- the common case
    /// this whole feature exists for -- must be offered.
    #[test]
    fn eligible_path_is_offered() {
        assert!(is_eligible_for_relocation(Path::new("/Applications/MeedyaDL.app")));
    }

    /// Already living under the consolidated folder: nothing to do.
    #[test]
    fn already_in_meedyasuite_is_not_offered() {
        assert!(!is_eligible_for_relocation(Path::new(
            "/Applications/MeedyaSuite/MeedyaDL.app"
        )));
    }

    /// Gatekeeper's quarantine path randomisation -- moving this path
    /// would move a throwaway decoy, not the real install.
    #[test]
    fn translocated_path_is_not_offered() {
        assert!(!is_eligible_for_relocation(Path::new(
            "/private/var/folders/zz/000000000000/T/AppTranslocation/12345678-ABCD/d/MeedyaDL.app"
        )));
    }

    /// Running straight off a mounted DMG is a first install, not a
    /// tidy-up step.
    #[test]
    fn mounted_dmg_volume_is_not_offered() {
        assert!(!is_eligible_for_relocation(Path::new("/Volumes/MeedyaDL/MeedyaDL.app")));
    }

    /// A per-user `~/Applications` install is not the system-wide
    /// `/Applications` this feature targets.
    #[test]
    fn user_applications_folder_is_not_offered() {
        assert!(!is_eligible_for_relocation(Path::new(
            "/Users/alice/Applications/MeedyaDL.app"
        )));
    }

    /// Any other subfolder of /Applications (not MeedyaSuite) is also
    /// left alone -- only a bare, direct `/Applications/*.app` install
    /// qualifies.
    #[test]
    fn nested_deeper_subfolder_is_not_offered() {
        assert!(!is_eligible_for_relocation(Path::new(
            "/Applications/Utilities/MeedyaDL.app"
        )));
    }
}
