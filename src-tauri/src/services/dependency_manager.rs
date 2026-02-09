// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency manager service.
// Downloads, installs, and manages external tool dependencies required
// by GAMDL: FFmpeg (required), mp4decrypt, N_m3u8DL-RE, and MP4Box
// (optional). Each tool is downloaded from its official release source
// and installed to {app_data}/tools/{tool_name}/.

use std::path::PathBuf;
use tauri::AppHandle;

use crate::utils::{archive, platform};

// ============================================================
// Tool metadata: describes each external dependency
// ============================================================

/// Metadata for a downloadable tool dependency.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Human-readable display name (e.g., "FFmpeg")
    pub name: &'static str,
    /// Short identifier used for directory names (e.g., "ffmpeg")
    pub id: &'static str,
    /// Whether this tool is required for basic GAMDL functionality
    pub required: bool,
    /// Brief description of what the tool is used for
    pub description: &'static str,
}

/// All external tool dependencies and their metadata.
/// FFmpeg is required; the others are optional but enable additional features.
const TOOLS: &[ToolInfo] = &[
    ToolInfo {
        name: "FFmpeg",
        id: "ffmpeg",
        required: true,
        description: "Audio/video remuxing and conversion",
    },
    ToolInfo {
        name: "mp4decrypt",
        id: "mp4decrypt",
        required: false,
        description: "Decryption of DRM-protected content (Bento4)",
    },
    ToolInfo {
        name: "N_m3u8DL-RE",
        id: "nm3u8dlre",
        required: false,
        description: "Alternative HLS/DASH stream downloader",
    },
    ToolInfo {
        name: "MP4Box",
        id: "mp4box",
        required: false,
        description: "Alternative remuxing tool (GPAC)",
    },
];

/// Returns the download URL and archive format for a tool on the current platform.
///
/// Selects the appropriate pre-built binary archive from the tool's official
/// release source based on the current OS and architecture.
///
/// # Arguments
/// * `tool_id` - The tool identifier (e.g., "ffmpeg", "mp4decrypt")
///
/// # Returns
/// * `Ok((url, format))` - The download URL and archive format
/// * `Err(message)` - If no pre-built binary is available for this platform
fn get_tool_download_url(tool_id: &str) -> Result<(String, archive::ArchiveFormat), String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match tool_id {
        "ffmpeg" => get_ffmpeg_url(os, arch),
        "mp4decrypt" => get_mp4decrypt_url(os, arch),
        "nm3u8dlre" => get_nm3u8dlre_url(os, arch),
        "mp4box" => get_mp4box_url(os, arch),
        _ => Err(format!("Unknown tool: {}", tool_id)),
    }
}

/// Returns the FFmpeg download URL for the given platform.
///
/// Sources:
/// - Linux/Windows: BtbN/FFmpeg-Builds GitHub releases (latest master build)
/// - macOS: evermeet.cx static builds (x86_64) or osxcross builds (aarch64)
fn get_ffmpeg_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
                .to_string(),
            archive::ArchiveFormat::TarGz, // NOTE: actually tar.xz, handled by extraction
        )),
        ("windows", "x86_64") | ("windows", "aarch64") => Ok((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
                .to_string(),
            archive::ArchiveFormat::Zip,
        )),
        ("macos", _) => {
            // macOS: Use evermeet.cx builds for x86_64 (runs via Rosetta on aarch64)
            // For native aarch64, users should install FFmpeg via Homebrew
            Ok((
                "https://evermeet.cx/ffmpeg/getrelease/zip".to_string(),
                archive::ArchiveFormat::Zip,
            ))
        }
        _ => Err(format!(
            "No pre-built FFmpeg available for {}/{}. Install FFmpeg manually and set the path in Settings.",
            os, arch
        )),
    }
}

/// Returns the mp4decrypt (Bento4) download URL for the given platform.
///
/// Bento4 provides pre-built binaries on their website and GitHub.
fn get_mp4decrypt_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    let platform_suffix = match (os, arch) {
        ("macos", "x86_64") | ("macos", "aarch64") => "macosx",
        ("linux", "x86_64") | ("linux", "aarch64") => "linux-x86_64",
        ("windows", "x86_64") | ("windows", "aarch64") => "win32",
        _ => {
            return Err(format!(
                "No pre-built mp4decrypt available for {}/{}",
                os, arch
            ))
        }
    };

    Ok((
        format!(
            "https://www.bok.net/Bento4/binaries/Bento4-SDK-1-6-0-641.{}.zip",
            platform_suffix
        ),
        archive::ArchiveFormat::Zip,
    ))
}

/// Returns the N_m3u8DL-RE download URL for the given platform.
///
/// N_m3u8DL-RE provides pre-built binaries on their GitHub releases.
fn get_nm3u8dlre_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // N_m3u8DL-RE uses .NET runtime identifiers for their build names
    let rid = match (os, arch) {
        ("macos", "aarch64") => "osx-arm64",
        ("macos", "x86_64") => "osx-x64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("windows", "x86_64") => "win-x64",
        ("windows", "aarch64") => "win-arm64",
        _ => {
            return Err(format!(
                "No pre-built N_m3u8DL-RE available for {}/{}",
                os, arch
            ))
        }
    };

    let format = if os == "windows" {
        archive::ArchiveFormat::Zip
    } else {
        archive::ArchiveFormat::TarGz
    };

    Ok((
        format!(
            "https://github.com/nilaoda/N_m3u8DL-RE/releases/latest/download/N_m3u8DL-RE_Beta_{}.tar.gz",
            rid
        ),
        format,
    ))
}

/// Returns the MP4Box (GPAC) download URL for the given platform.
///
/// GPAC provides installers and pre-built binaries via their website and GitHub.
fn get_mp4box_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // MP4Box is part of the GPAC project; pre-built binaries vary by platform
    match (os, arch) {
        ("windows", "x86_64") | ("windows", "aarch64") => Ok((
            "https://download.tsi.telecom-paristech.fr/gpac/latest_builds/windows/x64/gpac-latest-master-x64.zip"
                .to_string(),
            archive::ArchiveFormat::Zip,
        )),
        ("macos", _) => {
            // macOS: GPAC provides DMG installers, not ZIP archives.
            // Users should install via Homebrew: `brew install gpac`
            Err("MP4Box on macOS: install via Homebrew (`brew install gpac`)".to_string())
        }
        ("linux", "x86_64") => Ok((
            "https://download.tsi.telecom-paristech.fr/gpac/latest_builds/linux/x64/gpac-latest-master-x64.tar.gz"
                .to_string(),
            archive::ArchiveFormat::TarGz,
        )),
        _ => Err(format!(
            "No pre-built MP4Box available for {}/{}",
            os, arch
        )),
    }
}

/// Returns the path to a tool's installation directory.
///
/// Each tool gets its own subdirectory under {app_data}/tools/.
/// Example: {app_data}/tools/ffmpeg/
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier (e.g., "ffmpeg")
pub fn get_tool_dir(app: &AppHandle, tool_id: &str) -> PathBuf {
    platform::get_tools_dir(app).join(tool_id)
}

/// Returns the expected path to a tool's binary executable.
///
/// The binary name varies by tool and platform (Windows adds .exe).
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier
pub fn get_tool_binary_path(app: &AppHandle, tool_id: &str) -> PathBuf {
    let tool_dir = get_tool_dir(app, tool_id);
    let exe_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    // Each tool's binary has a specific name
    let binary_name = match tool_id {
        "ffmpeg" => format!("ffmpeg{}", exe_ext),
        "mp4decrypt" => format!("mp4decrypt{}", exe_ext),
        "nm3u8dlre" => format!("N_m3u8DL-RE{}", exe_ext),
        "mp4box" => format!("MP4Box{}", exe_ext),
        _ => format!("{}{}", tool_id, exe_ext),
    };

    tool_dir.join(binary_name)
}

/// Downloads and installs a specific tool dependency.
///
/// Performs the complete installation pipeline:
/// 1. Determines the download URL for the current platform
/// 2. Downloads the archive
/// 3. Extracts to the tool's directory
/// 4. Locates the binary within the extracted contents
/// 5. Sets executable permissions (Unix)
/// 6. Verifies the binary works by running --version (if supported)
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier ("ffmpeg", "mp4decrypt", "nm3u8dlre", "mp4box")
///
/// # Returns
/// * `Ok(version)` - The installed version string (or "installed" if version detection fails)
/// * `Err(message)` - A descriptive error if installation failed
pub async fn install_tool(app: &AppHandle, tool_id: &str) -> Result<String, String> {
    log::info!("Starting installation of tool: {}", tool_id);

    // Step 1: Get the download URL for this platform
    let (url, format) = get_tool_download_url(tool_id)?;

    // Step 2: Resolve the tool's installation directory
    let tool_dir = get_tool_dir(app, tool_id);

    // Clean up existing installation if present
    if tool_dir.exists() {
        log::info!("Removing existing {} installation", tool_id);
        std::fs::remove_dir_all(&tool_dir).map_err(|e| {
            format!("Failed to remove existing {} directory: {}", tool_id, e)
        })?;
    }

    // Create the tool directory
    std::fs::create_dir_all(&tool_dir)
        .map_err(|e| format!("Failed to create tool directory: {}", e))?;

    // Step 3: Download and extract the archive
    log::info!("Downloading {} from {}", tool_id, url);
    archive::download_and_extract(&url, &tool_dir, format).await?;

    // Step 4: Find the binary in the extracted contents
    // Some archives have nested directories; try to find the binary recursively
    let expected_binary = get_tool_binary_path(app, tool_id);
    if !expected_binary.exists() {
        // Try to find the binary somewhere in the extracted directory
        if let Some(found) = find_binary_recursive(&tool_dir, tool_id) {
            // Move the found binary to the expected location
            std::fs::copy(&found, &expected_binary).map_err(|e| {
                format!(
                    "Failed to copy {} binary to expected location: {}",
                    tool_id, e
                )
            })?;
            log::info!(
                "Found {} binary at {}, copied to {}",
                tool_id,
                found.display(),
                expected_binary.display()
            );
        } else {
            return Err(format!(
                "Installation succeeded but {} binary not found in extracted archive. \
                 Expected at: {}",
                tool_id,
                expected_binary.display()
            ));
        }
    }

    // Step 5: Set executable permissions on Unix
    archive::set_executable(&expected_binary)?;

    // Step 6: Try to get the version (best-effort)
    let version = get_tool_version(&expected_binary, tool_id)
        .await
        .unwrap_or_else(|_| "installed".to_string());

    log::info!("{} {} installed successfully", tool_id, version);
    Ok(version)
}

/// Searches recursively for a tool's binary within a directory.
///
/// Archives sometimes contain nested directories (e.g., ffmpeg-master-latest-linux64-gpl/bin/ffmpeg).
/// This function walks the directory tree to find the binary regardless of nesting.
///
/// # Arguments
/// * `dir` - The directory to search in
/// * `tool_id` - The tool identifier (used to determine the binary name)
fn find_binary_recursive(dir: &PathBuf, tool_id: &str) -> Option<PathBuf> {
    let exe_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    // The binary names to search for (some tools have different names in archives)
    let search_names: Vec<String> = match tool_id {
        "ffmpeg" => vec![format!("ffmpeg{}", exe_ext)],
        "mp4decrypt" => vec![format!("mp4decrypt{}", exe_ext)],
        "nm3u8dlre" => vec![
            format!("N_m3u8DL-RE{}", exe_ext),
            format!("n_m3u8dl-re{}", exe_ext),
        ],
        "mp4box" => vec![format!("MP4Box{}", exe_ext), format!("mp4box{}", exe_ext)],
        _ => vec![format!("{}{}", tool_id, exe_ext)],
    };

    // Walk the directory tree looking for the binary
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if search_names.iter().any(|s| s == name) {
                        return Some(path);
                    }
                }
            } else if path.is_dir() {
                // Recurse into subdirectories
                if let Some(found) = find_binary_recursive(&path, tool_id) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// Attempts to get the version of an installed tool binary.
///
/// Runs the binary with common version flags (--version, -version) and
/// parses the first line of output.
///
/// # Arguments
/// * `binary_path` - Path to the tool binary
/// * `tool_id` - The tool identifier (for tool-specific parsing)
async fn get_tool_version(binary_path: &PathBuf, tool_id: &str) -> Result<String, String> {
    // Different tools use different version flags
    let version_flag = match tool_id {
        "ffmpeg" => "-version",
        "mp4box" => "-version",
        _ => "--version",
    };

    let output = tokio::process::Command::new(binary_path)
        .arg(version_flag)
        .output()
        .await
        .map_err(|e| format!("Failed to run {} --version: {}", tool_id, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("").trim().to_string();

    if first_line.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first_err_line = stderr.lines().next().unwrap_or("unknown").trim().to_string();
        Ok(first_err_line)
    } else {
        Ok(first_line)
    }
}

/// Checks whether a tool is installed and returns its status.
///
/// Verifies that the tool's binary exists at the expected path. Does NOT
/// attempt to run the binary (which would be slow for batch checks).
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier
///
/// # Returns
/// `true` if the tool binary exists at the expected path
pub fn is_tool_installed(app: &AppHandle, tool_id: &str) -> bool {
    get_tool_binary_path(app, tool_id).exists()
}

/// Returns the list of all tool dependencies with their metadata.
///
/// Used by the setup wizard and dependency status UI to display
/// the full list of tools with their installation requirements.
pub fn get_all_tools() -> &'static [ToolInfo] {
    TOOLS
}

/// Removes a tool's installation directory.
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier
pub async fn uninstall_tool(app: &AppHandle, tool_id: &str) -> Result<(), String> {
    let tool_dir = get_tool_dir(app, tool_id);

    if tool_dir.exists() {
        log::info!("Removing {} installation at {}", tool_id, tool_dir.display());
        tokio::fs::remove_dir_all(&tool_dir)
            .await
            .map_err(|e| format!("Failed to remove {} directory: {}", tool_id, e))?;
    }

    Ok(())
}
