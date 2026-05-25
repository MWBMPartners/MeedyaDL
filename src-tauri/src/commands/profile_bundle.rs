// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// IPC surface for the .meedyabundle profile export/import format
// (#876 P2 + P3 + P5).
//
// P2 (this commit): the `export_profile` command — opens a save
// dialog, builds the bundle from the current install's state, writes
// it. Granular content selection via `ExportProfileOptions`.
//
// P3 lands the `import_profile` command + the import wizard
// component (multi-step React flow). The Settings UI gets BOTH
// Export and Import buttons (per maintainer note: "allow the user
// to manually import a .meedyabundle file in Settings" — covered
// here by the Settings-driven path; P5 adds the first-launch
// auto-detect convenience flow).
//
// Credentials encryption (P4) is NOT in this commit. The
// `include_credentials` flag is accepted for forward compatibility
// but currently routes to a stub that adds an empty placeholder.
// P4 will wire AES-256-GCM + PBKDF2 + password entry behind that
// flag without changing the IPC shape.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::services::profile_bundle::{
    entry, BundleMeta, BundleReader, BundleSection, BundleWriter,
};
use crate::utils::activity_log::emit_app_log;

/// Frontend-supplied options controlling which OPTIONAL sections the
/// bundle should include. The REQUIRED `settings.json` +
/// `settings.json.sha256` are always present.
///
/// Defaults are conservative: queue + history included (small,
/// commonly wanted), everything else opt-in.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExportProfileOptions {
    #[serde(default = "default_true")]
    pub include_queue: bool,
    #[serde(default = "default_true")]
    pub include_history: bool,
    /// Include the SQLite download index (#875 EPIC A foundation).
    /// When `true` and the DB exists, the entire `meedyadl.db` file
    /// is embedded — the receiving install gets the full indexed
    /// download record + activity log without needing to re-ingest.
    #[serde(default)]
    pub include_database: bool,
    /// Reserved for P4. When set, P4's encryption flow will collect
    /// a password from the user and embed encrypted credentials.
    /// Today: no-op (bundle ships without credentials).
    #[serde(default)]
    pub include_credentials: bool,
    /// Include the on-disk JSONL activity-log files. Opt-in
    /// because the forensic record can be MBs.
    #[serde(default)]
    pub include_activity_log: bool,
    /// Include every `manifest.meedyadl` under the configured
    /// output directory. Opt-in because a 50,000-track library's
    /// manifest set is substantial.
    #[serde(default)]
    pub include_manifests: bool,
    /// Optional free-form note set by the user (e.g.,
    /// "Pre-MacBook-Pro replacement export, 2026-05-25").
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Result returned to the frontend on a successful export.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportProfileResult {
    /// Absolute path the bundle was written to.
    pub path: String,
    /// Size of the resulting `.meedyabundle` file, in bytes.
    pub size_bytes: u64,
    /// List of optional section names that were actually included.
    pub sections: Vec<String>,
}

/// Export the current install's profile to a `.meedyabundle` file
/// (#876 P2). Opens a native save dialog, builds the bundle, writes
/// it, emits a `[System]` activity-log line.
#[tauri::command]
pub async fn export_profile(
    app: AppHandle,
    options: ExportProfileOptions,
) -> Result<ExportProfileResult, String> {
    let app_data_dir = crate::utils::platform::get_app_data_dir(&app);
    let crate_version = env!("CARGO_PKG_VERSION");

    let mut meta = BundleMeta::new_for_export(crate_version);
    meta.note = options.note.clone();
    let mut writer = BundleWriter::new(meta);

    // settings.json + sha256 sidecar — REQUIRED.
    let settings_path = app_data_dir.join("settings.json");
    let settings_bytes = std::fs::read(&settings_path)
        .map_err(|e| format!("Failed to read settings.json: {e}"))?;
    let sha256_path = app_data_dir.join("settings.json.sha256");
    let sha256_hex = std::fs::read_to_string(&sha256_path).unwrap_or_default();
    writer.add_settings(settings_bytes, sha256_hex.trim().to_string());

    // queue.json — OPTIONAL, default ON.
    if options.include_queue {
        if let Ok(bytes) = std::fs::read(app_data_dir.join("queue.json")) {
            writer.add_optional_file(entry::QUEUE, bytes);
            writer.declare_section(BundleSection::Queue);
        }
    }

    // history.json — OPTIONAL, default ON.
    if options.include_history {
        if let Ok(bytes) = std::fs::read(app_data_dir.join("history.json")) {
            writer.add_optional_file(entry::HISTORY, bytes);
            writer.declare_section(BundleSection::History);
        }
    }

    // SQLite database — OPTIONAL, default OFF (#875 M5 + #876 P6).
    if options.include_database {
        let db_path = app_data_dir.join("meedyadl.db");
        if db_path.exists() {
            if let Err(e) = writer.add_database_from_disk(&db_path) {
                log::warn!("export_profile: failed to embed DB (continuing without): {e}");
            }
        }
    }

    // Activity-log JSONL files — OPTIONAL, default OFF.
    if options.include_activity_log {
        let log_dir = app_data_dir.join("logs");
        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let name = match p.file_name().and_then(|n| n.to_str()) {
                    Some(n) if n.starts_with("activity-") && n.ends_with(".log") => n.to_string(),
                    _ => continue,
                };
                if let Ok(bytes) = std::fs::read(&p) {
                    writer.add_optional_file(
                        format!("{}{name}", entry::ACTIVITY_LOG_PREFIX),
                        bytes,
                    );
                }
            }
            writer.declare_section(BundleSection::ActivityLog);
        }
    }

    // Manifests — OPTIONAL, default OFF (can be huge).
    if options.include_manifests {
        let settings = crate::services::config_service::load_settings(&app)?;
        let root = std::path::Path::new(&settings.output_path);
        if root.is_dir() {
            collect_manifests_into_bundle(root, &mut writer);
        }
    }

    // Credentials encryption deferred to P4. For now, declare
    // nothing — the bundle ships without credentials.
    if options.include_credentials {
        log::warn!(
            "export_profile: include_credentials=true requested but credential encryption is not yet implemented (#876 P4). Credentials will NOT be in this bundle."
        );
    }

    // Snapshot the section list BEFORE consuming the writer so we
    // can return it to the frontend.
    let sections: Vec<String> = writer
        .meta()
        .contents
        .iter()
        .map(|s| s.as_str().to_string())
        .collect();

    let bytes = writer
        .finalize_to_bytes()
        .map_err(|e| format!("Failed to seal bundle: {e}"))?;

    // Native save dialog.
    let default_name = format!(
        "MeedyaDL-{}-{}.meedyabundle",
        crate_version,
        chrono::Utc::now().format("%Y-%m-%d")
    );
    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Bundle", &["meedyabundle"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file_path {
        Some(p) => {
            let resolved: PathBuf = p
                .as_path()
                .ok_or_else(|| "Failed to resolve bundle path".to_string())?
                .to_path_buf();
            std::fs::write(&resolved, &bytes)
                .map_err(|e| format!("Failed to write bundle: {e}"))?;
            let size_bytes = bytes.len() as u64;
            let path_str = resolved.to_string_lossy().to_string();
            let filename = resolved
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("MeedyaDL.meedyabundle")
                .to_string();
            log::info!(
                "Profile bundle exported to {} ({} bytes, sections: {})",
                filename,
                size_bytes,
                sections.join(", ")
            );
            emit_app_log(
                &app,
                &format!(
                    "Profile bundle exported to {filename} ({} bytes; sections: {})",
                    format_size(size_bytes),
                    if sections.is_empty() {
                        "settings only".to_string()
                    } else {
                        sections.join(", ")
                    },
                ),
            );
            Ok(ExportProfileResult {
                path: path_str,
                size_bytes,
                sections,
            })
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Recursively walk `root` (depth-limited via the existing fs_walk
/// helper) and add every `manifest.meedyadl` we find to the bundle,
/// preserving the relative path under `manifests/`.
fn collect_manifests_into_bundle(root: &std::path::Path, writer: &mut BundleWriter) {
    let mut count = 0;
    let _ = crate::utils::fs_walk::walk_dir_depth(root, 10, |path| {
        if !path.is_file() {
            return None;
        }
        let name = path.file_name().and_then(|n| n.to_str())?;
        if name != "manifest.meedyadl" && name != ".meedyadl" {
            return None;
        }
        let rel = path.strip_prefix(root).ok()?;
        let archive_path = format!(
            "{}{}",
            entry::MANIFESTS_PREFIX,
            rel.to_string_lossy()
        );
        let bytes = std::fs::read(path).ok()?;
        writer.add_optional_file(archive_path, bytes);
        count += 1;
        None::<()>
    });
    if count > 0 {
        writer.declare_section(BundleSection::Manifests);
        log::info!("export_profile: included {count} manifest file(s)");
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

// ============================================================
// P3 — import flow (peek + import)
// ============================================================

/// Lightweight summary of a bundle's metadata + present sections.
/// Returned by [`peek_profile_bundle`] so the import wizard can
/// show the user what's about to be imported before they confirm.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProfileBundleSummary {
    pub bundle_version: u32,
    pub meedyadl_version: String,
    pub platform: String,
    pub exported_at: String,
    pub exported_by: Option<String>,
    pub note: Option<String>,
    /// OPTIONAL sections present in the bundle, in the canonical
    /// snake_case names (matches `BundleSection::as_str()`).
    pub sections: Vec<String>,
    /// Resolved absolute path of the inspected bundle. Echoed back
    /// so the frontend can pass it to `import_profile` without
    /// re-prompting.
    pub source_path: String,
}

/// Open a `.meedyabundle` for inspection without extracting anything.
/// Used by the import wizard's first step to show metadata + the
/// section checklist before the user confirms.
#[tauri::command]
pub async fn peek_profile_bundle(
    path: Option<String>,
    app: AppHandle,
) -> Result<ProfileBundleSummary, String> {
    let resolved = match path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => {
            // No path supplied — open a native picker.
            let picked = app
                .dialog()
                .file()
                .add_filter("MeedyaDL Bundle", &["meedyabundle"])
                .blocking_pick_file();
            match picked {
                Some(fp) => fp
                    .as_path()
                    .ok_or_else(|| "Failed to resolve bundle path".to_string())?
                    .to_path_buf(),
                None => return Err("Import cancelled".to_string()),
            }
        }
    };

    let bytes = std::fs::read(&resolved)
        .map_err(|e| format!("Failed to read bundle file: {e}"))?;
    let reader = BundleReader::from_bytes(bytes)
        .map_err(|e| format!("Failed to open bundle: {e}"))?;
    let meta = reader.meta().clone();

    Ok(ProfileBundleSummary {
        bundle_version: meta.bundle_version,
        meedyadl_version: meta.meedyadl_version,
        platform: meta.platform,
        exported_at: meta.exported_at,
        exported_by: meta.exported_by,
        note: meta.note,
        sections: meta
            .contents
            .iter()
            .map(|s| s.as_str().to_string())
            .collect(),
        source_path: resolved.to_string_lossy().to_string(),
    })
}

/// Conflict-resolution choice for sections that have an existing
/// equivalent in the destination install. **Settings** is always
/// `Replace` (we don't merge JSON schema-blindly). For other
/// sections the user picks per-section.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportConflictAction {
    /// Don't touch the existing file. The bundle's payload is
    /// ignored for this section.
    Skip,
    /// Overwrite the existing file with the bundle's payload.
    Replace,
}

/// Options for `import_profile`. Mirrors the per-section structure
/// of `ExportProfileOptions` but each switch is a
/// [`ImportConflictAction`] so the user can choose Skip vs Replace
/// per section.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ImportProfileOptions {
    /// Path to the bundle file. Required — call
    /// `peek_profile_bundle` first to surface this from the picker.
    pub source_path: String,
    /// Always `Replace` for settings; the field exists for shape
    /// symmetry with the rest of the section toggles.
    #[serde(default = "default_replace")]
    pub settings: ImportConflictAction,
    #[serde(default = "default_skip")]
    pub queue: ImportConflictAction,
    #[serde(default = "default_skip")]
    pub history: ImportConflictAction,
    #[serde(default = "default_skip")]
    pub database: ImportConflictAction,
    #[serde(default = "default_skip")]
    pub activity_log: ImportConflictAction,
    #[serde(default = "default_skip")]
    pub manifests: ImportConflictAction,
    /// Reserved for P4: when set + the bundle has credentials,
    /// the user is prompted for the password and the credentials
    /// get re-injected into the OS keychain.
    #[serde(default = "default_skip")]
    pub credentials: ImportConflictAction,
}

fn default_skip() -> ImportConflictAction {
    ImportConflictAction::Skip
}
fn default_replace() -> ImportConflictAction {
    ImportConflictAction::Replace
}

/// Per-section result returned by `import_profile`. Lets the
/// frontend show "restored 3 sections, skipped 2".
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportProfileResult {
    pub settings_restored: bool,
    pub queue_restored: bool,
    pub history_restored: bool,
    pub database_restored: bool,
    pub activity_log_files_restored: usize,
    pub manifests_restored: usize,
    /// True when the bundle declared credentials but P4 hasn't
    /// landed yet — the frontend shows a warning that credentials
    /// were skipped.
    pub credentials_skipped_p4: bool,
}

/// Import a `.meedyabundle` into the current install (#876 P3).
///
/// Per-section behaviour is controlled by [`ImportProfileOptions`].
/// Settings is always restored when present in the bundle (the
/// shape symmetry exists for future P4-credential interaction).
/// Manifests + activity-log files are written back to their
/// expected locations (output dir + logs dir). The SQLite
/// database is restored via the M5 helper.
///
/// The caller is responsible for ensuring the app's background
/// tasks (queue processor, activity-log writer) won't trip over
/// in-flight writes during restore — the import wizard surfaces
/// this with a "restart MeedyaDL to apply" toast on completion.
#[tauri::command]
pub async fn import_profile(
    app: AppHandle,
    options: ImportProfileOptions,
) -> Result<ImportProfileResult, String> {
    let app_data_dir = crate::utils::platform::get_app_data_dir(&app);
    let bytes = std::fs::read(&options.source_path)
        .map_err(|e| format!("Failed to read bundle file: {e}"))?;
    let mut reader =
        BundleReader::from_bytes(bytes).map_err(|e| format!("Failed to open bundle: {e}"))?;

    let mut result = ImportProfileResult {
        settings_restored: false,
        queue_restored: false,
        history_restored: false,
        database_restored: false,
        activity_log_files_restored: 0,
        manifests_restored: 0,
        credentials_skipped_p4: false,
    };

    // Settings (REQUIRED entry; restore by default).
    if matches!(options.settings, ImportConflictAction::Replace) {
        if let Some(settings_bytes) = reader
            .read_entry(entry::SETTINGS)
            .map_err(|e| format!("Failed to read settings.json from bundle: {e}"))?
        {
            std::fs::write(app_data_dir.join("settings.json"), &settings_bytes)
                .map_err(|e| format!("Failed to write settings.json: {e}"))?;
            if let Some(sha) = reader
                .read_entry(entry::SETTINGS_SHA256)
                .map_err(|e| format!("Failed to read settings.json.sha256: {e}"))?
            {
                std::fs::write(app_data_dir.join("settings.json.sha256"), &sha)
                    .map_err(|e| format!("Failed to write settings sha256: {e}"))?;
            }
            result.settings_restored = true;
        }
    }

    // Queue.
    if matches!(options.queue, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::Queue)
    {
        if let Some(bytes) = reader
            .read_entry(entry::QUEUE)
            .map_err(|e| format!("Failed to read queue.json: {e}"))?
        {
            std::fs::write(app_data_dir.join("queue.json"), &bytes)
                .map_err(|e| format!("Failed to write queue.json: {e}"))?;
            result.queue_restored = true;
        }
    }

    // History.
    if matches!(options.history, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::History)
    {
        if let Some(bytes) = reader
            .read_entry(entry::HISTORY)
            .map_err(|e| format!("Failed to read history.json: {e}"))?
        {
            std::fs::write(app_data_dir.join("history.json"), &bytes)
                .map_err(|e| format!("Failed to write history.json: {e}"))?;
            result.history_restored = true;
        }
    }

    // SQLite database (#875 M5 helper).
    if matches!(options.database, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::Database)
    {
        let dest = app_data_dir.join("meedyadl.db");
        match reader.extract_database_to(&dest) {
            Ok(true) => {
                result.database_restored = true;
            }
            Ok(false) => {} // Bundle declared section but entry missing — silent skip.
            Err(e) => {
                return Err(format!("Failed to restore database: {e}"));
            }
        }
    }

    // Activity-log JSONL files.
    if matches!(options.activity_log, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::ActivityLog)
    {
        let log_dir = app_data_dir.join("logs");
        std::fs::create_dir_all(&log_dir).ok();
        let names = collect_archive_entries_with_prefix(&mut reader, entry::ACTIVITY_LOG_PREFIX);
        for name in names {
            if let Some(bytes) = reader
                .read_entry(&name)
                .map_err(|e| format!("Failed to read {name}: {e}"))?
            {
                let basename = name
                    .strip_prefix(entry::ACTIVITY_LOG_PREFIX)
                    .unwrap_or(&name);
                std::fs::write(log_dir.join(basename), &bytes)
                    .map_err(|e| format!("Failed to write activity-log file: {e}"))?;
                result.activity_log_files_restored += 1;
            }
        }
    }

    // Manifests.
    if matches!(options.manifests, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::Manifests)
    {
        let settings = crate::services::config_service::load_settings(&app)?;
        let root = std::path::PathBuf::from(&settings.output_path);
        if !root.exists() {
            std::fs::create_dir_all(&root).ok();
        }
        let names = collect_archive_entries_with_prefix(&mut reader, entry::MANIFESTS_PREFIX);
        for name in names {
            if let Some(bytes) = reader
                .read_entry(&name)
                .map_err(|e| format!("Failed to read {name}: {e}"))?
            {
                let rel = name.strip_prefix(entry::MANIFESTS_PREFIX).unwrap_or(&name);
                let dest = root.join(rel);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(&dest, &bytes)
                    .map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;
                result.manifests_restored += 1;
            }
        }
    }

    // Credentials — P4 placeholder.
    if matches!(options.credentials, ImportConflictAction::Replace)
        && reader.meta().has(BundleSection::Credentials)
    {
        log::warn!(
            "import_profile: credentials section present in bundle but encryption-at-rest (P4) is not yet implemented; skipping. The user will need to re-import cookies / re-enter MusicKit credentials manually on this install."
        );
        result.credentials_skipped_p4 = true;
    }

    emit_app_log(
        &app,
        &format!(
            "Profile bundle imported: settings={}, queue={}, history={}, database={}, activity_log_files={}, manifests={}, credentials_skipped_p4={}",
            result.settings_restored,
            result.queue_restored,
            result.history_restored,
            result.database_restored,
            result.activity_log_files_restored,
            result.manifests_restored,
            result.credentials_skipped_p4,
        ),
    );

    Ok(result)
}

/// Helper: enumerate every entry name in the open archive that
/// starts with `prefix`. Used by the activity-log + manifests
/// restore loops which deal with N files under a single section
/// directory.
fn collect_archive_entries_with_prefix(reader: &mut BundleReader, prefix: &str) -> Vec<String> {
    reader
        .file_names_starting_with(prefix)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_buckets_correctly() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(2 * 1024 * 1024 + 512 * 1024), "2.50 MB");
    }
}
