// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! IPC commands for macOS self-relocation (#1057).
//!
//! Thin wrappers around `services::app_relocation`. `check_app_relocation`
//! is a synchronous, cheap read of the state computed once in
//! `lib.rs::run()`'s `.setup()` hook; `relocate_app_bundle` performs the
//! actual (potentially privilege-elevating, definitely blocking-I/O)
//! move on a blocking thread, mirroring the pattern already used by
//! `commands::backup`.

use tauri::{AppHandle, State};

use crate::services::app_relocation::{self, AppRelocationState};

/// Returns whether MeedyaDL should offer to relocate itself into
/// `/Applications/MeedyaSuite`, and the proposed destination path if so.
///
/// Reads the [`AppRelocationState`] computed once at startup (managed
/// state) rather than recomputing it on every call — see the module doc
/// on why the startup check must stay cheap and why this command is a
/// plain synchronous read.
///
/// **Frontend caller:** `checkAppRelocation()` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub fn check_app_relocation(state: State<'_, AppRelocationState>) -> AppRelocationState {
    state.inner().clone()
}

/// Performs the actual relocation: moves the running app bundle into
/// `/Applications/MeedyaSuite`, then relaunches from the new location.
///
/// On success this call never resolves from the frontend's point of
/// view — the backend process exits after relaunching itself at the new
/// path. On failure it resolves with `Err(reason)` and MeedyaDL keeps
/// running from its original location; the caller is expected to show
/// the user an error toast and leave the offer available to retry
/// later (see `AppRelocationModal.tsx`).
///
/// Runs on a blocking thread because the move is synchronous filesystem
/// I/O (and, on the admin-fallback path, blocks on an OS credentials
/// prompt) — the same `spawn_blocking` pattern `commands::backup` uses
/// for its own filesystem work.
///
/// **Frontend caller:** `relocateAppBundle()` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn relocate_app_bundle(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app_relocation::perform_relocation(&app))
        .await
        .map_err(|e| format!("Relocation task panicked: {e}"))?
}
