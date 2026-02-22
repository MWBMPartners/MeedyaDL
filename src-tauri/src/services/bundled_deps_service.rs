// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Bundled dependencies extraction service.
// ==========================================
//
// This service handles first-launch extraction of dependencies that were
// bundled into the application installer at build time by CI. During CI
// builds, `scripts/download-bundled-deps.sh` downloads platform-specific
// binaries (Python, GAMDL, FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box),
// places them under `src-tauri/bundled-deps/`, and creates a
// `bundled-deps.tar.gz` archive. Tauri bundles this single archive file
// as a resource (configured in `tauri.conf.json`).
//
// On first launch, this service:
//   1. Checks whether bundled deps have already been extracted
//   2. Locates the `bundled-deps.tar.gz` archive in the app's resource dir
//   3. Extracts the archive to a temporary directory
//   4. Reads `manifest.json` to see which deps were successfully bundled
//   5. Copies each bundled dep to the app data directory (same paths that
//      the runtime installer would use)
//   6. Sets executable permissions on Unix platforms
//   7. Writes `.source` marker files as "bundled" for each extracted tool
//   8. Writes a `.bundled_deps_extracted` marker to prevent re-extraction
//   9. Cleans up the temporary extraction directory
//
// Why tar.gz instead of direct directory bundling?
// Tauri v2's resource glob (`bundled-deps/**/*`) flattens all matched
// files into a single directory, destroying the subdirectory structure.
// By bundling a single tar.gz archive, we preserve the full directory
// tree and extract it at runtime.
//
// Key design decisions:
//   - **No-overwrite**: Skips deps that already exist in the target location.
//     This respects user-updated tools and manual installations.
//   - **Idempotent**: The `.bundled_deps_extracted` marker prevents wasteful
//     re-extraction on subsequent launches.
//   - **Graceful fallback**: If the archive doesn't exist (dev builds, older
//     installs), the function returns false and the normal download flow
//     handles dependency installation via the setup wizard.
//   - **Placeholder detection**: build.rs creates a tiny (~20 byte) empty
//     gzip for dev builds so `cargo build` succeeds. We detect and skip
//     these placeholders by checking the file size.
//
// Resource directory locations (where Tauri places bundled resources):
//   - macOS:   `MeedyaDL.app/Contents/Resources/bundled-deps.tar.gz`
//   - Windows: `{install_dir}/resources/bundled-deps.tar.gz`
//   - Linux:   `{install_dir}/resources/bundled-deps.tar.gz`
//
// App data directory locations (where deps are extracted to):
//   - macOS:   `~/Library/Application Support/io.github.meedyadl/`
//   - Windows: `%APPDATA%\io.github.meedyadl\`
//   - Linux:   `~/.local/share/io.github.meedyadl/`
//
// @see scripts/download-bundled-deps.sh -- CI download orchestrator
// @see src-tauri/build.rs -- placeholder archive creation for dev builds
// @see tauri.conf.json -- resource bundling configuration
// @see services/dependency_manager.rs -- runtime dependency installer (fallback)
// @see services/python_manager.rs -- runtime Python installer (fallback)

use std::path::{Path, PathBuf};
use flate2::read::GzDecoder;
use tar::Archive;
use tauri::{AppHandle, Manager};

use crate::utils::platform;

/// Minimum file size (in bytes) for a real bundled-deps archive.
/// The build.rs placeholder is ~20 bytes. Real archives are 100MB+.
/// We use a generous threshold to distinguish the two.
const MIN_ARCHIVE_SIZE: u64 = 1024;

/// Top-level manifest file structure matching the `manifest.json` written
/// by the CI download script (`scripts/download-bundled-deps.sh`).
///
/// The manifest contains metadata about the build and a nested
/// `dependencies` object recording which deps were successfully bundled.
///
/// Example manifest.json:
/// ```json
/// {
///   "bundled_at": "2026-02-22T14:30:55Z",
///   "target_os": "macos",
///   "target_arch": "aarch64",
///   "python_version": "3.12.8",
///   "dependencies": {
///     "python": true,
///     "ffmpeg": true,
///     "mp4decrypt": true,
///     "nm3u8dlre": true,
///     "mp4box": true
///   }
/// }
/// ```
#[derive(Debug, serde::Deserialize)]
struct ManifestFile {
    /// Nested dependency availability flags
    #[serde(default)]
    dependencies: BundledDeps,
}

/// Per-dependency availability flags from the manifest.
///
/// Each field records whether the corresponding dependency was successfully
/// downloaded and staged during the CI build.
#[derive(Debug, Default, serde::Deserialize)]
struct BundledDeps {
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

/// Locates the bundled-deps.tar.gz archive in the app's resource directory.
///
/// Returns `None` if:
/// - The resource directory cannot be resolved (dev mode edge case)
/// - The archive doesn't exist
/// - The archive is a placeholder (< MIN_ARCHIVE_SIZE bytes)
fn get_bundled_archive_path(app: &AppHandle) -> Option<PathBuf> {
    let resource_dir = app.path().resource_dir().ok()?;
    let archive_path = resource_dir.join("bundled-deps.tar.gz");

    if !archive_path.exists() {
        return None;
    }

    // Check file size to distinguish real archives from the build.rs placeholder.
    // The placeholder is a minimal valid gzip (~20 bytes) created so that
    // `cargo build` succeeds without actual bundled deps.
    let metadata = std::fs::metadata(&archive_path).ok()?;
    if metadata.len() < MIN_ARCHIVE_SIZE {
        log::debug!(
            "Bundled deps archive is too small ({} bytes), likely a dev placeholder",
            metadata.len()
        );
        return None;
    }

    Some(archive_path)
}

/// Extracts the bundled-deps.tar.gz archive to a temporary directory.
///
/// The archive contains a top-level `bundled-deps/` directory with:
/// ```text
/// bundled-deps/
/// ├── manifest.json
/// ├── python/
/// │   ├── bin/
/// │   ├── lib/
/// │   └── ...
/// └── tools/
///     ├── ffmpeg/
///     ├── mp4decrypt/
///     ├── nm3u8dlre/
///     └── mp4box/
/// ```
///
/// Returns the path to the extracted `bundled-deps/` directory inside
/// the temp directory.
fn extract_archive_to_temp(archive_path: &Path) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("meedyadl_bundled_extract");

    // Clean up any previous extraction attempt
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        format!("Failed to create temp extraction dir: {}", e)
    })?;

    log::info!("Extracting bundled-deps archive to temp dir...");

    let file = std::fs::File::open(archive_path).map_err(|e| {
        format!("Failed to open bundled-deps archive: {}", e)
    })?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // Extract with full path preservation
    archive.unpack(&temp_dir).map_err(|e| {
        format!("Failed to extract bundled-deps archive: {}", e)
    })?;

    // The archive was created with:
    //   tar czf bundled-deps.tar.gz -C <parent> bundled-deps
    // So all paths are prefixed with `bundled-deps/`
    let extracted_dir = temp_dir.join("bundled-deps");
    if extracted_dir.exists() {
        log::info!("Archive extracted successfully to {}", extracted_dir.display());
        Ok(extracted_dir)
    } else {
        Err("Extracted archive doesn't contain expected bundled-deps/ directory".to_string())
    }
}

/// Reads the bundled manifest to determine which deps were successfully
/// bundled during CI.
///
/// Parses the top-level `ManifestFile` and returns the inner `BundledDeps`.
/// Returns `None` if the manifest doesn't exist or is unreadable.
fn read_manifest(bundled_dir: &Path) -> Option<BundledDeps> {
    let manifest_path = bundled_dir.join("manifest.json");
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let manifest: ManifestFile = serde_json::from_str(&content).ok()?;
    Some(manifest.dependencies)
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

/// Extracts bundled dependencies from the app's resource archive to the
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
/// 2. If the bundled-deps.tar.gz archive doesn't exist or is a placeholder,
///    returns `Ok(false)`.
/// 3. Extracts the archive to a temporary directory.
/// 4. Reads `manifest.json` to determine which deps were bundled.
/// 5. For each bundled dep:
///    - Copies from temp extraction to app data dir (skipping existing files)
///    - Writes `.source` marker as "bundled"
/// 6. Writes the `.bundled_deps_extracted` marker.
/// 7. Cleans up the temporary extraction directory.
/// 8. Returns `Ok(true)`.
pub fn extract_bundled_deps(app: &AppHandle) -> Result<bool, String> {
    // Step 1: Check if already extracted
    if already_extracted(app) {
        log::info!("Bundled deps already extracted, skipping");
        return Ok(false);
    }

    // Step 2: Check if the archive exists in the resource directory
    let archive_path = match get_bundled_archive_path(app) {
        Some(path) => path,
        None => {
            log::info!(
                "No bundled deps archive found in resource directory (dev build or older install)"
            );
            return Ok(false);
        }
    };

    log::info!(
        "Found bundled deps archive at: {} ({} bytes)",
        archive_path.display(),
        std::fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0)
    );

    // Step 3: Extract the archive to a temporary directory
    let extracted_dir = extract_archive_to_temp(&archive_path)?;

    // Step 4: Read the manifest
    let manifest = read_manifest(&extracted_dir).unwrap_or_else(|| {
        log::warn!("No manifest.json found in bundled deps, assuming all deps present");
        BundledDeps {
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

    // Step 5a: Extract Python runtime
    if manifest.python {
        let src = extracted_dir.join("python");
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

    // Step 5b: Extract tool dependencies
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

        let src = extracted_dir.join("tools").join(tool_id);
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

    // Step 6: Write the extraction marker
    write_extraction_marker(app)?;

    // Step 7: Clean up the temporary extraction directory
    let temp_dir = std::env::temp_dir().join("meedyadl_bundled_extract");
    if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
        log::warn!("Failed to clean up temp extraction dir: {}", e);
        // Non-fatal -- deps were extracted successfully
    }

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

    /// Tests ManifestFile deserialization with nested dependencies and defaults.
    #[test]
    fn test_manifest_partial_deserialization() {
        let json = r#"{"dependencies": {"python": true, "ffmpeg": true}}"#;
        let manifest: ManifestFile = serde_json::from_str(json).unwrap();
        assert!(manifest.dependencies.python);
        assert!(manifest.dependencies.ffmpeg);
        assert!(!manifest.dependencies.gamdl);
        assert!(!manifest.dependencies.mp4decrypt);
        assert!(!manifest.dependencies.nm3u8dlre);
        assert!(!manifest.dependencies.mp4box);
    }

    /// Tests ManifestFile deserialization with the full format produced by the
    /// download script, including metadata fields.
    #[test]
    fn test_manifest_full_deserialization() {
        let json = r#"{
            "bundled_at": "2026-02-22T14:30:55Z",
            "target_os": "macos",
            "target_arch": "aarch64",
            "python_version": "3.12.8",
            "dependencies": {
                "python": true,
                "gamdl": true,
                "ffmpeg": true,
                "mp4decrypt": true,
                "nm3u8dlre": true,
                "mp4box": true
            }
        }"#;
        let manifest: ManifestFile = serde_json::from_str(json).unwrap();
        assert!(manifest.dependencies.python);
        assert!(manifest.dependencies.gamdl);
        assert!(manifest.dependencies.ffmpeg);
        assert!(manifest.dependencies.mp4decrypt);
        assert!(manifest.dependencies.nm3u8dlre);
        assert!(manifest.dependencies.mp4box);
    }

    /// Tests tar.gz extraction to temporary directory.
    #[test]
    fn test_extract_archive_to_temp() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let test_dir = make_temp_dir("archive_test");

        // Create a tar.gz archive with the expected structure
        let archive_path = test_dir.join("bundled-deps.tar.gz");
        let file = fs::File::create(&archive_path).unwrap();
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar_builder = tar::Builder::new(enc);

        // Add manifest.json with the nested format produced by download script
        let manifest_content = br#"{"dependencies": {"python": true, "ffmpeg": true}}"#;
        let mut header = tar::Header::new_gnu();
        header.set_path("bundled-deps/manifest.json").unwrap();
        header.set_size(manifest_content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder
            .append(&header, &manifest_content[..])
            .unwrap();

        // Add a tool binary
        let binary_content = b"fake-ffmpeg-binary";
        let mut header = tar::Header::new_gnu();
        header.set_path("bundled-deps/tools/ffmpeg/ffmpeg").unwrap();
        header.set_size(binary_content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar_builder.append(&header, &binary_content[..]).unwrap();

        // Finalize
        let enc = tar_builder.into_inner().unwrap();
        enc.finish().unwrap();

        // Extract and verify
        let extracted = extract_archive_to_temp(&archive_path).unwrap();
        assert!(extracted.join("manifest.json").exists());
        assert!(extracted.join("tools").join("ffmpeg").join("ffmpeg").exists());

        let manifest = read_manifest(&extracted).unwrap();
        assert!(manifest.python);
        assert!(manifest.ffmpeg);

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
        let temp_dir = std::env::temp_dir().join("meedyadl_bundled_extract");
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
