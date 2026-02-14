// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency manager service.
// Downloads, installs, and manages external tool dependencies required
// by GAMDL: FFmpeg, mp4decrypt, N_m3u8DL-RE, and MP4Box (all required
// for full functionality). Each tool is downloaded from its official release source
// and installed to {app_data}/tools/{tool_name}/.
//
// ## Architecture Overview
//
// External tools are binary dependencies that GAMDL invokes as subprocesses
// during the download pipeline. This service handles their lifecycle:
//
// ```
// Setup Wizard UI --> install_tool("ffmpeg")
//                        |
//                     get_tool_download_url() --> platform-specific URL
//                        |
//                     archive::download_and_extract() --> {app_data}/tools/ffmpeg/
//                        |
//                     find_binary_recursive() --> locate binary in extracted dir
//                        |
//                     set_executable() + get_tool_version() --> verify working
// ```
//
// ## Tool Inventory
//
// | Tool        | Required | Source                        | Purpose                     |
// |-------------|----------|-------------------------------|-----------------------------|
// | FFmpeg      | Yes      | BtbN/FFmpeg-Builds, evermeet  | Audio/video remuxing        |
// | mp4decrypt  | Yes      | Bento4 SDK                    | DRM decryption              |
// | N_m3u8DL-RE | Yes      | nilaoda/N_m3u8DL-RE           | HLS/DASH stream downloading |
// | MP4Box      | Yes      | GPAC project                  | MP4 muxing and remuxing     |
//
// ## Cross-Platform URL Selection
//
// Each tool has a dedicated URL resolver function (get_ffmpeg_url, etc.) that
// maps (OS, architecture) to the correct pre-built binary archive URL. The
// functions handle platform-specific quirks (e.g., macOS FFmpeg from evermeet.cx,
// MP4Box requiring Homebrew on macOS).
//
// ## References
//
// - Reqwest HTTP client for downloads: https://docs.rs/reqwest/latest/reqwest/
// - FFmpeg builds: https://github.com/BtbN/FFmpeg-Builds (Linux/Windows), https://evermeet.cx/ffmpeg/ (macOS)
// - Bento4 (mp4decrypt): https://www.bento4.com/
// - N_m3u8DL-RE: https://github.com/nilaoda/N_m3u8DL-RE
// - GPAC (MP4Box): https://gpac.io/
// - Tokio async filesystem operations: https://docs.rs/tokio/latest/tokio/fs/

use std::path::PathBuf;
use tauri::AppHandle;

// `archive` provides download_and_extract() for streaming HTTP download + archive extraction,
// and set_executable() for chmod +x on Unix systems.
// `platform` provides get_tools_dir() for resolving the {app_data}/tools/ directory.
use crate::utils::{archive, platform};

// ============================================================
// Tool metadata: describes each external dependency
// ============================================================

/// Metadata for a downloadable tool dependency.
///
/// This struct describes a tool that GAMDL may need at runtime.
/// The metadata is used by the setup wizard UI to display tool names,
/// descriptions, and required/optional status. The `id` field is used
/// as the tool's directory name and identifier in all API calls.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Human-readable display name shown in the UI (e.g., "FFmpeg")
    pub name: &'static str,
    /// Short machine-readable identifier used for directory names and API calls.
    /// Must match the tool_id parameter used in install_tool(), get_tool_binary_path(), etc.
    pub id: &'static str,
    /// Whether this tool is required for basic GAMDL functionality.
    /// The setup wizard highlights required tools and blocks completion until they're installed.
    pub required: bool,
    /// Brief description of what the tool is used for (shown in the setup wizard).
    pub description: &'static str,
}

/// All external tool dependencies and their metadata.
/// All four tools are required for full functionality: FFmpeg for remuxing,
/// mp4decrypt for DRM decryption, N_m3u8DL-RE for HLS/DASH streams, and
/// MP4Box for MP4 muxing. This list is returned by get_all_tools() for the setup wizard UI.
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
        required: true,
        description: "Decryption of DRM-protected content (Bento4)",
    },
    ToolInfo {
        name: "N_m3u8DL-RE",
        id: "nm3u8dlre",
        required: true,
        description: "HLS/DASH stream downloader",
    },
    ToolInfo {
        name: "MP4Box",
        id: "mp4box",
        required: true,
        description: "MP4 muxing and remuxing tool (GPAC)",
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
    // Detect the current OS and architecture at compile time via std::env::consts.
    // OS values: "macos", "windows", "linux"
    // ARCH values: "x86_64", "aarch64"
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Dispatch to the tool-specific URL resolver.
    // Each resolver handles platform-specific URL construction and format selection.
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
        // Linux x86_64: BtbN/FFmpeg-Builds provides GPL-licensed static builds.
        // These are self-contained binaries with no external dependencies.
        // The "latest" tag always points to the most recent master build.
        // Ref: https://github.com/BtbN/FFmpeg-Builds
        ("linux", "x86_64") => Ok((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
                .to_string(),
            archive::ArchiveFormat::TarGz, // NOTE: actually tar.xz, handled by the extraction utility
        )),
        // Windows x86_64 and aarch64: BtbN builds (x64 binary, runs on ARM64 via emulation).
        // The ZIP archive contains ffmpeg.exe, ffprobe.exe, and ffplay.exe.
        ("windows", "x86_64") | ("windows", "aarch64") => Ok((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
                .to_string(),
            archive::ArchiveFormat::Zip,
        )),
        // macOS (both architectures): evermeet.cx provides x86_64 static builds.
        // On Apple Silicon (aarch64), these run via Rosetta 2 translation.
        // For native ARM64 builds, users can alternatively install via Homebrew
        // (`brew install ffmpeg`) and set the path in Settings.
        // Ref: https://evermeet.cx/ffmpeg/
        ("macos", _) => {
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
/// mp4decrypt is part of the Bento4 SDK, used for decrypting MPEG-CENC
/// encrypted content. GAMDL uses it to decrypt DRM-protected Apple Music tracks.
///
/// Bento4 provides pre-built binaries hosted at bok.net (the Bento4 author's site).
/// The SDK ZIP contains multiple tools; we only need the mp4decrypt binary.
/// Ref: https://www.bento4.com/
fn get_mp4decrypt_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // Map OS/arch to Bento4's platform suffix naming convention
    let platform_suffix = match (os, arch) {
        // macOS: universal build (works on both x86_64 and aarch64 natively)
        ("macos", "x86_64") | ("macos", "aarch64") => "universal-apple-macosx",
        // Linux: x86_64 only (ARM64 users would need to compile from source)
        ("linux", "x86_64") | ("linux", "aarch64") => "linux-x86_64",
        // Windows: 32-bit suffix but the binary works on 64-bit Windows
        ("windows", "x86_64") | ("windows", "aarch64") => "win32",
        _ => {
            return Err(format!(
                "No pre-built mp4decrypt available for {}/{}",
                os, arch
            ))
        }
    };

    // Bento4 SDK version 1.6.0-641 (latest stable as of writing).
    // The ZIP contains bin/{mp4decrypt, mp4info, mp4fragment, ...}.
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
/// N_m3u8DL-RE is a cross-platform HLS/DASH stream downloader that GAMDL
/// can use as an alternative to its built-in downloader. It's written in C#
/// (.NET) and provides native AOT-compiled binaries for each platform.
///
/// Ref: https://github.com/nilaoda/N_m3u8DL-RE
fn get_nm3u8dlre_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // N_m3u8DL-RE uses .NET Runtime Identifiers (RIDs) in their release asset names.
    // RID format: {os}-{arch} (e.g., "osx-arm64", "linux-x64", "win-x64").
    // Ref: https://learn.microsoft.com/en-us/dotnet/core/rid-catalog
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

    // Windows releases use ZIP; Unix releases use tar.gz
    let format = if os == "windows" {
        archive::ArchiveFormat::Zip
    } else {
        archive::ArchiveFormat::TarGz
    };

    // The "latest" redirect ensures we always get the newest beta release.
    // The archive contains a single binary: N_m3u8DL-RE (or N_m3u8DL-RE.exe on Windows).
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
/// MP4Box is the command-line tool from the GPAC multimedia framework.
/// GAMDL can use it as an alternative to FFmpeg for MP4 container operations
/// (muxing, demuxing, encryption handling).
///
/// GPAC provides nightly builds from their CI server at Telecom Paris.
/// Ref: https://gpac.io/
/// Ref: https://github.com/gpac/gpac
fn get_mp4box_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // MP4Box is part of the GPAC project; pre-built binaries vary by platform.
    // The download URLs point to nightly builds from the GPAC CI server.
    match (os, arch) {
        // Windows: x64 build from the GPAC CI server (latest master branch)
        ("windows", "x86_64") | ("windows", "aarch64") => Ok((
            "https://download.tsi.telecom-paristech.fr/gpac/latest_builds/windows/x64/gpac-latest-master-x64.zip"
                .to_string(),
            archive::ArchiveFormat::Zip,
        )),
        // macOS: GPAC distributes DMG installers (not ZIP/tar.gz archives)
        // which our archive utility can't handle. Users should install via
        // Homebrew instead: `brew install gpac`
        ("macos", _) => {
            Err("MP4Box on macOS: install via Homebrew (`brew install gpac`)".to_string())
        }
        // Linux x86_64: tar.gz build from the GPAC CI server
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
    // On Windows, executables require the .exe extension
    let exe_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    // Map tool_id to the actual binary filename.
    // Note: some tools have case-sensitive names that differ from the tool_id:
    // - nm3u8dlre -> N_m3u8DL-RE (the binary has uppercase/mixed case)
    // - mp4box -> MP4Box (the binary has uppercase)
    let binary_name = match tool_id {
        "ffmpeg" => format!("ffmpeg{}", exe_ext),
        "mp4decrypt" => format!("mp4decrypt{}", exe_ext),
        "nm3u8dlre" => format!("N_m3u8DL-RE{}", exe_ext),
        "mp4box" => format!("MP4Box{}", exe_ext),
        _ => format!("{}{}", tool_id, exe_ext),
    };

    // The binary is expected at {app_data}/tools/{tool_id}/{binary_name}
    // e.g., {app_data}/tools/ffmpeg/ffmpeg
    tool_dir.join(binary_name)
}

/// Resolves a tool display name or ID to the canonical tool ID.
///
/// The frontend sends tool display names (e.g., "FFmpeg", "N_m3u8DL-RE")
/// while the backend URL resolver expects tool IDs (e.g., "ffmpeg", "nm3u8dlre").
/// This function accepts either form and returns the canonical ID.
///
/// # Arguments
/// * `name_or_id` - Either a tool display name or internal ID
///
/// # Returns
/// * `Ok(id)` - The canonical tool ID
/// * `Err(message)` - If no tool matches the given name or ID
fn resolve_tool_id(name_or_id: &str) -> Result<&'static str, String> {
    for tool in TOOLS {
        if tool.id == name_or_id || tool.name == name_or_id {
            return Ok(tool.id);
        }
    }
    Err(format!("Unknown tool: {}", name_or_id))
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
/// * `name_or_id` - The tool display name or identifier (e.g., "FFmpeg" or "ffmpeg")
///
/// # Returns
/// * `Ok(version)` - The installed version string (or "installed" if version detection fails)
/// * `Err(message)` - A descriptive error if installation failed
pub async fn install_tool(app: &AppHandle, name_or_id: &str) -> Result<String, String> {
    // Resolve display name to canonical tool ID (e.g., "FFmpeg" -> "ffmpeg")
    let tool_id = resolve_tool_id(name_or_id)?;
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

    // Step 4: Find the binary in the extracted contents.
    // Archives often contain nested directory structures. For example:
    // - FFmpeg: ffmpeg-master-latest-linux64-gpl/bin/ffmpeg
    // - Bento4: Bento4-SDK-1-6-0-641.macosx/bin/mp4decrypt
    // We first check the expected flat location, then search recursively.
    let expected_binary = get_tool_binary_path(app, tool_id);
    if !expected_binary.exists() {
        // Binary not at the expected top-level location — search recursively
        // through the extracted directory tree to find it.
        if let Some(found) = find_binary_recursive(&tool_dir, tool_id) {
            // Copy the found binary to the expected location for consistent access.
            // We use copy instead of rename to handle cross-filesystem scenarios.
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

    // Build the list of possible binary filenames to search for.
    // Some tools may have different casing in their release archives compared
    // to what we expect, so we check multiple variants.
    let search_names: Vec<String> = match tool_id {
        "ffmpeg" => vec![format!("ffmpeg{}", exe_ext)],
        "mp4decrypt" => vec![format!("mp4decrypt{}", exe_ext)],
        // N_m3u8DL-RE: check both the expected case and lowercase variant
        "nm3u8dlre" => vec![
            format!("N_m3u8DL-RE{}", exe_ext),
            format!("n_m3u8dl-re{}", exe_ext),
        ],
        // MP4Box: check both the expected case and lowercase variant
        "mp4box" => vec![format!("MP4Box{}", exe_ext), format!("mp4box{}", exe_ext)],
        _ => vec![format!("{}{}", tool_id, exe_ext)],
    };

    // Walk the directory tree depth-first looking for any matching binary.
    // This handles archives with arbitrary nesting (e.g., Bento4 SDK has bin/ subdir).
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                // Check if this file matches any of our search names
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if search_names.iter().any(|s| s == name) {
                        return Some(path);
                    }
                }
            } else if path.is_dir() {
                // Recurse into subdirectories (depth-first search)
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
    // Different tools use different version flags:
    // - FFmpeg and MP4Box use single-dash "-version" (non-standard but that's how they work)
    // - Most other tools use double-dash "--version" (GNU convention)
    let version_flag = match tool_id {
        "ffmpeg" => "-version",   // e.g., "ffmpeg version N-112479-..."
        "mp4box" => "-version",   // e.g., "MP4Box - GPAC version 2.4-DEV..."
        _ => "--version",         // Standard GNU-style flag
    };

    // Run the binary with the version flag and capture output.
    // This serves as both a version check and a basic health check
    // (verifying the binary is executable and not corrupt).
    let output = tokio::process::Command::new(binary_path)
        .arg(version_flag)
        .output()
        .await
        .map_err(|e| format!("Failed to run {} --version: {}", tool_id, e))?;

    // Extract the first line of stdout as the version string.
    // Most tools output their version on the first line of stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("").trim().to_string();

    if first_line.is_empty() {
        // Some tools output version info to stderr (e.g., FFmpeg logs to stderr).
        // Fall back to the first line of stderr.
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

/// Removes a tool's installation directory and all its contents.
///
/// Used when the user wants to reinstall a tool or when the installation
/// is detected as corrupt. Uses async filesystem operations to avoid
/// blocking the Tokio runtime.
/// Ref: https://docs.rs/tokio/latest/tokio/fs/fn.remove_dir_all.html
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier (e.g., "ffmpeg", "mp4decrypt")
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
