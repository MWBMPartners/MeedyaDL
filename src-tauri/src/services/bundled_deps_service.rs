// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Bundled dependencies extraction service.
// ==========================================
//
// This service handles first-launch extraction of dependencies that were
// bundled into the application installer at build time by CI. When the
// `scripts/download-bundled-deps.sh` script runs during CI builds, it
// downloads platform-specific binaries (Python, GAMDL, FFmpeg, mp4decrypt,
// N_m3u8DL-RE, MP4Box) and places them under `src-tauri/bundled-deps/`.
// Tauri's resource bundling (configured in `tauri.conf.json`) includes
// these files in the platform installer.
//
// On first launch, this service:
//   1. Checks whether bundled deps exist in the app's resource directory
//   2. Reads the `manifest.json` to see which deps were successfully bundled
//   3. Copies each bundled dep to the app data directory (same paths that
//      the runtime installer would use)
//   4. Sets executable permissions on Unix platforms
//   5. Writes `.source` marker files as "bundled" for each extracted tool
//   6. Writes a `.bundled_deps_extracted` marker to prevent re-extraction
//
// Key design decisions:
//   - **No-overwrite**: Skips deps that already exist in the target location.
//     This respects user-updated tools and manual installations.
//   - **Idempotent**: The `.bundled_deps_extracted` marker prevents wasteful
//     re-extraction on subsequent launches.
//   - **Graceful fallback**: If bundled deps don't exist (dev builds, older
//     installs), the function returns false and the normal download flow
//     handles dependency installation via the setup wizard.
//
// Resource directory locations (where Tauri places bundled resources):
//   - macOS:   `MeedyaDL.app/Contents/Resources/bundled-deps/`
//   - Windows: `{install_dir}/resources/bundled-deps/`
//   - Linux:   `{install_dir}/resources/bundled-deps/`
//
// App data directory locations (where deps are extracted to):
//   - macOS:   `~/Library/Application Support/io.github.meedyadl/`
//   - Windows: `%APPDATA%\io.github.meedyadl\`
//   - Linux:   `~/.local/share/io.github.meedyadl/`
//
// @see scripts/download-bundled-deps.sh -- CI download orchestrator
// @see tauri.conf.json -- resource bundling configuration
// @see services/dependency_manager.rs -- runtime dependency installer (fallback)
// @see services/python_manager.rs -- runtime Python installer (fallback)

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::utils::platform;

/// Manifest structure matching the `manifest.json` written by the CI
/// download script (`scripts/download-bundled-deps.sh`).
///
/// Each field records whether the corresponding dependency was successfully
/// downloaded and staged during the CI build.
#[derive(Debug, serde::Deserialize)]
struct BundledManifest {
    /// Whether the Python runtime was bundled
    #[serde(default)]
    python: bool,
    /// Whether GAMDL was installed into the bundled Python.
    /// Not read explicitly because GAMDL lives inside the Python directory
    /// (pip installs it into site-packages), so extracting Python extracts GAMDL too.
    #[serde(default)]
    #[allow(dead_code)]
    gamdl: bool,
    /// Whether FFmpeg was bundled
    #[serde(default)]
    ffmpeg: bool,
    /// Whether mp4decrypt (Bento4) was bundled
    #[serde(default)]
    mp4decrypt: bool,
    /// Whether N_m3u8DL-RE was bundled
    #[serde(default)]
    nm3u8dlre: bool,
    /// Whether MP4Box (GPAC) was bundled
    #[serde(default)]
    mp4box: bool,
}

/// Resolves the path to the bundled-deps directory inside the app's
/// resource directory.
///
/// Returns `None` if the resource directory cannot be resolved (e.g.,
/// running in dev mode without bundled deps).
fn get_bundled_deps_dir(app: &AppHandle) -> Option<PathBuf> {
    // Tauri 2.0: app.path().resource_dir() returns the base resource dir.
    // Our bundled deps are in the `bundled-deps/` subdirectory.
    let resource_dir = app.path().resource_dir().ok()?;
    let bundled_dir = resource_dir.join("bundled-deps");

    if bundled_dir.exists() {
        Some(bundled_dir)
    } else {
        None
    }
}

/// Reads the bundled manifest to determine which deps were successfully
/// bundled during CI.
///
/// Returns `None` if the manifest doesn't exist or is unreadable.
fn read_manifest(bundled_dir: &Path) -> Option<BundledManifest> {
    let manifest_path = bundled_dir.join("manifest.json");
    let content = std::fs::read_to_string(manifest_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Checks whether bundled deps extraction has already been performed.
///
/// Returns `true` if the `.bundled_deps_extracted` marker file exists
/// in the app data directory.
fn already_extracted(app: &AppHandle) -> bool {
    let marker = platform::get_app_data_dir(app).join(".bundled_deps_extracted");
    marker.exists()
}

/// Writes the extraction marker file to prevent re-extraction on
/// subsequent launches.
fn write_extraction_marker(app: &AppHandle) -> Result<(), String> {
    let marker = platform::get_app_data_dir(app).join(".bundled_deps_extracted");
    std::fs::write(&marker, "extracted").map_err(|e| {
        format!("Failed to write extraction marker: {}", e)
    })
}

/// Recursively copies a directory tree from `src` to `dst`.
///
/// Creates `dst` and all intermediate directories. Skips files that
/// already exist in the destination (no-overwrite policy).
///
/// On Unix platforms, sets executable permissions on files in `bin/`
/// directories and on known binary files.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64, String> {
    let mut files_copied = 0u64;

    if !src.exists() {
        return Ok(0);
    }

    // Create the destination directory
    std::fs::create_dir_all(dst).map_err(|e| {
        format!("Failed to create directory {}: {}", dst.display(), e)
    })?;

    let entries = std::fs::read_dir(src).map_err(|e| {
        format!("Failed to read directory {}: {}", src.display(), e)
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if src_path.is_dir() {
            // Recurse into subdirectories
            files_copied += copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            // Skip if destination file already exists (no-overwrite policy)
            if dst_path.exists() {
                log::debug!(
                    "Skipping existing file: {}",
                    dst_path.display()
                );
                continue;
            }

            // Copy the file
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "Failed to copy {} -> {}: {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                )
            })?;
            files_copied += 1;

            // Set executable permissions on Unix for binaries
            #[cfg(unix)]
            {
                set_executable_if_needed(&dst_path, dst);
            }
        }
    }

    Ok(files_copied)
}

/// On Unix, sets executable permissions on files that should be executable.
///
/// Heuristics: files in `bin/` directories, or known binary names
/// (ffmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, python3, pip3).
#[cfg(unix)]
fn set_executable_if_needed(file_path: &Path, parent_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let should_be_executable = {
        // Files in a bin/ directory are always executable
        let parent_name = parent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if parent_name == "bin" || parent_name == "Scripts" {
            true
        } else {
            // Check known binary names
            let file_name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            matches!(
                file_name,
                "ffmpeg"
                    | "ffprobe"
                    | "mp4decrypt"
                    | "N_m3u8DL-RE"
                    | "MP4Box"
                    | "python3"
                    | "python3.12"
                    | "pip3"
                    | "pip3.12"
            )
        }
    };

    if should_be_executable {
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let mut perms = metadata.permissions();
            // Add execute permission for owner, group, and others (0o755)
            perms.set_mode(perms.mode() | 0o111);
            let _ = std::fs::set_permissions(file_path, perms);
        }
    }
}

/// Writes a `.source` marker file indicating the tool was installed from
/// the bundled deps rather than downloaded at runtime.
fn write_source_marker(tool_dir: &Path, source: &str) -> Result<(), String> {
    let marker_path = tool_dir.join(".source");
    std::fs::write(&marker_path, source).map_err(|e| {
        format!(
            "Failed to write source marker at {}: {}",
            marker_path.display(),
            e
        )
    })
}

/// Extracts bundled dependencies from the app's resource directory to the
/// app data directory on first launch.
///
/// This is the main entry point called from the IPC command layer.
///
/// # Returns
///
/// * `Ok(true)` -- Extraction was performed (first launch with bundled deps)
/// * `Ok(false)` -- No extraction needed (already extracted, no bundled deps,
///   or running a dev build without bundled deps)
/// * `Err(String)` -- Fatal error during extraction
///
/// # Behaviour
///
/// 1. If the `.bundled_deps_extracted` marker exists, returns `Ok(false)`.
/// 2. If the bundled-deps resource directory doesn't exist, returns `Ok(false)`.
/// 3. Reads `manifest.json` to determine which deps were bundled.
/// 4. For each bundled dep:
///    - Copies from resource dir to app data dir (skipping existing files)
///    - Writes `.source` marker as "bundled"
/// 5. Writes the `.bundled_deps_extracted` marker.
/// 6. Returns `Ok(true)`.
pub fn extract_bundled_deps(app: &AppHandle) -> Result<bool, String> {
    // Step 1: Check if already extracted
    if already_extracted(app) {
        log::info!("Bundled deps already extracted, skipping");
        return Ok(false);
    }

    // Step 2: Check if bundled deps exist in the resource directory
    let bundled_dir = match get_bundled_deps_dir(app) {
        Some(dir) => dir,
        None => {
            log::info!("No bundled deps found in resource directory (dev build or older install)");
            return Ok(false);
        }
    };

    log::info!(
        "Found bundled deps at: {}",
        bundled_dir.display()
    );

    // Step 3: Read the manifest
    let manifest = read_manifest(&bundled_dir).unwrap_or_else(|| {
        log::warn!("No manifest.json found in bundled deps, assuming all deps present");
        BundledManifest {
            python: true,
            gamdl: true,
            ffmpeg: true,
            mp4decrypt: true,
            nm3u8dlre: true,
            mp4box: true,
        }
    });

    let app_data_dir = platform::get_app_data_dir(app);
    std::fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!("Failed to create app data directory: {}", e)
    })?;

    let mut total_files = 0u64;

    // Step 4a: Extract Python runtime
    if manifest.python {
        let src = bundled_dir.join("python");
        let dst = app_data_dir.join("python");
        if src.exists() && !dst.exists() {
            log::info!("Extracting bundled Python runtime...");
            let count = copy_dir_recursive(&src, &dst)?;
            total_files += count;
            log::info!("Extracted Python: {} files", count);
        } else if dst.exists() {
            log::info!("Python already exists at {}, skipping", dst.display());
        }
    }

    // Step 4b: Extract tool dependencies
    let tools = [
        ("ffmpeg", manifest.ffmpeg),
        ("mp4decrypt", manifest.mp4decrypt),
        ("nm3u8dlre", manifest.nm3u8dlre),
        ("mp4box", manifest.mp4box),
    ];

    for (tool_id, bundled) in &tools {
        if !bundled {
            log::info!("Tool '{}' was not bundled, skipping", tool_id);
            continue;
        }

        let src = bundled_dir.join("tools").join(tool_id);
        let dst = app_data_dir.join("tools").join(tool_id);

        if !src.exists() {
            log::warn!(
                "Manifest says '{}' is bundled but directory not found at {}",
                tool_id,
                src.display()
            );
            continue;
        }

        if dst.exists() {
            log::info!(
                "Tool '{}' already exists at {}, skipping",
                tool_id,
                dst.display()
            );
            continue;
        }

        log::info!("Extracting bundled tool '{}'...", tool_id);
        let count = copy_dir_recursive(&src, &dst)?;
        total_files += count;
        log::info!("Extracted '{}': {} files", tool_id, count);

        // Write the .source marker as "bundled"
        write_source_marker(&dst, "bundled")?;
    }

    // Step 5: Write the extraction marker
    write_extraction_marker(app)?;

    log::info!(
        "Bundled deps extraction complete: {} files extracted",
        total_files
    );

    Ok(true)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a unique temporary directory under the system temp dir.
    /// Returns the path; the caller is responsible for cleanup.
    fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("meedyadl_test")
            .join(format!("{}_{}", suffix, std::process::id()));
        let _ = fs::remove_dir_all(&dir); // Clean up from previous runs
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Tests that copy_dir_recursive correctly copies a directory tree.
    #[test]
    fn test_copy_dir_recursive() {
        let src_dir = make_temp_dir("copy_src");
        let dst_dir = make_temp_dir("copy_dst");

        // Create source structure
        let sub_dir = src_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(src_dir.join("file1.txt"), "hello").unwrap();
        fs::write(sub_dir.join("file2.txt"), "world").unwrap();

        let dst = dst_dir.join("output");
        let count = copy_dir_recursive(&src_dir, &dst).unwrap();

        assert_eq!(count, 2);
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("subdir").join("file2.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("file1.txt")).unwrap(), "hello");
        assert_eq!(
            fs::read_to_string(dst.join("subdir").join("file2.txt")).unwrap(),
            "world"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }

    /// Tests that copy_dir_recursive skips existing files (no-overwrite).
    #[test]
    fn test_copy_dir_no_overwrite() {
        let src_dir = make_temp_dir("nooverwrite_src");
        let dst_dir = make_temp_dir("nooverwrite_dst");

        // Create source file
        fs::write(src_dir.join("existing.txt"), "new content").unwrap();

        // Pre-create the destination with different content
        let dst = dst_dir.join("output");
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("existing.txt"), "original content").unwrap();

        let count = copy_dir_recursive(&src_dir, &dst).unwrap();

        // Should skip the existing file
        assert_eq!(count, 0);
        // Original content should be preserved
        assert_eq!(
            fs::read_to_string(dst.join("existing.txt")).unwrap(),
            "original content"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }

    /// Tests that copy_dir_recursive handles nonexistent source gracefully.
    #[test]
    fn test_copy_dir_nonexistent_source() {
        let dst_dir = make_temp_dir("nonexistent_dst");
        let nonexistent = PathBuf::from("/tmp/this-path-does-not-exist-12345");
        let count = copy_dir_recursive(&nonexistent, &dst_dir).unwrap();
        assert_eq!(count, 0);

        // Cleanup
        let _ = fs::remove_dir_all(&dst_dir);
    }

    /// Tests that write_source_marker creates the correct marker file.
    #[test]
    fn test_write_source_marker() {
        let dir = make_temp_dir("source_marker");
        write_source_marker(&dir, "bundled").unwrap();

        let marker = dir.join(".source");
        assert!(marker.exists());
        assert_eq!(fs::read_to_string(marker).unwrap(), "bundled");

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    /// Tests BundledManifest deserialization with defaults.
    #[test]
    fn test_manifest_partial_deserialization() {
        let json = r#"{"python": true, "ffmpeg": true}"#;
        let manifest: BundledManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.python);
        assert!(manifest.ffmpeg);
        assert!(!manifest.gamdl);
        assert!(!manifest.mp4decrypt);
        assert!(!manifest.nm3u8dlre);
        assert!(!manifest.mp4box);
    }
}
