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
    entry, BundleMeta, BundleSection, BundleWriter,
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
