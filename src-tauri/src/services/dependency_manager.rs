// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency manager service.
// Downloads, installs, and manages external tool dependencies required
// by GAMDL: FFmpeg, mp4decrypt, N_m3u8DL-RE, and MP4Box (all required).
// Each tool is downloaded from its official release source and installed
// to {app_data}/tools/{tool_name}/.
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
// Tool version requirements (compiled from tool-versions.toml)
// ============================================================

/// The tool-versions.toml file, compiled into the binary at build time.
/// This avoids runtime file I/O and ensures version requirements always
/// match the application version.
const TOOL_VERSIONS_TOML: &str = include_str!("../../tool-versions.toml");

/// Parsed version requirement for a single tool from tool-versions.toml.
#[derive(Debug, serde::Deserialize)]
struct ToolVersionConfig {
    minimum_version: String,
    binary_name: String,
    #[allow(dead_code)]
    version_flag: String,
}

/// Loads the tool version config for a specific tool from the compiled TOML.
fn load_tool_version_config(tool_id: &str) -> Option<ToolVersionConfig> {
    let config: toml::Value = toml::from_str(TOOL_VERSIONS_TOML).ok()?;
    let tool_table = config.get(tool_id)?;
    toml::from_str(&toml::to_string(tool_table).ok()?).ok()
}

/// Configuration for the fallback tool mirror repository.
/// Parsed from the [mirror] section of tool-versions.toml.
///
/// The mirror hosts pre-built binaries with standardized naming at a GitHub
/// repository. When primary upstream sources fail (404, URL changes, etc.),
/// the dependency manager queries this repo's releases for matching assets.
#[derive(Debug, serde::Deserialize)]
struct MirrorConfig {
    /// GitHub repository in "owner/name" format (e.g., "MWBMPartners/meedyadl-tools")
    github_repo: String,
    /// Release tag to query for downloadable assets (e.g., "latest")
    release_tag: String,
}

/// Loads the mirror repository configuration from the compiled TOML.
///
/// Returns `None` if the [mirror] section is missing or malformed,
/// which disables mirror fallback silently.
fn load_mirror_config() -> Option<MirrorConfig> {
    let config: toml::Value = toml::from_str(TOOL_VERSIONS_TOML).ok()?;
    let mirror_table = config.get("mirror")?;
    toml::from_str(&toml::to_string(mirror_table).ok()?).ok()
}

/// Parses a version string into (major, minor, patch) components.
///
/// Handles various formats:
///   - "6.1.2" → (6, 1, 2)
///   - "5.0" → (5, 0, 0)
///   - "2.4-DEV" → (2, 4, 0)
///   - "N-112479-..." → None (`FFmpeg` nightly builds without numeric version)
fn parse_version_tuple(version_str: &str) -> Option<(u32, u32, u32)> {
    // Use regex to extract the first version-like pattern (digits.digits[.digits])
    let re = regex::Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").ok()?;
    let caps = re.captures(version_str)?;

    let major: u32 = caps.get(1)?.as_str().parse().ok()?;
    let minor: u32 = caps.get(2)?.as_str().parse().ok()?;
    let patch: u32 = caps
        .get(3)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);

    Some((major, minor, patch))
}

/// Extracts a version number from a tool's version output.
///
/// Each tool has a different version output format. This function applies
/// tool-specific parsing to extract the semver-like version.
///
/// Examples:
///   - `FFmpeg`: "ffmpeg version 6.1.2-..." → "6.1.2"
///   - `FFmpeg` nightly: "ffmpeg version N-112479-..." → skipped (returns None)
///   - `MP4Box`: "`MP4Box` - GPAC version 2.4-DEV-rev18-..." → "2.4"
///   - mp4decrypt: "mp4decrypt version 1.6.0.641" → "1.6.0"
///   - N_m3u8DL-RE: "N_m3u8DL-RE version 0.5.1-beta" → "0.5.1"
fn extract_version_from_output(output: &str, tool_id: &str) -> Option<String> {
    let first_line = output.lines().next()?.trim();

    match tool_id {
        "ffmpeg" => {
            // FFmpeg: "ffmpeg version 6.1.2-..." or "ffmpeg version N-112479-..."
            // Skip nightly builds starting with "N-" (no numeric version)
            let after_version = first_line.strip_prefix("ffmpeg version ")?;
            if after_version.starts_with('N') {
                // Nightly build — can't reliably compare, accept as compatible
                return Some("nightly".to_string());
            }
            let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
            Some(re.find(after_version)?.as_str().to_string())
        }
        "mp4box" => {
            // MP4Box: "MP4Box - GPAC version 2.4-DEV..." or "GPAC version 2.4..."
            let re = regex::Regex::new(r"(?:GPAC|version)\s+(\d+\.\d+(?:\.\d+)?)").ok()?;
            let caps = re.captures(first_line)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        "mp4decrypt" => {
            // mp4decrypt: "mp4decrypt version 1.6.0.641"
            let re = regex::Regex::new(r"(\d+\.\d+\.\d+)").ok()?;
            Some(re.find(first_line)?.as_str().to_string())
        }
        "nm3u8dlre" => {
            // N_m3u8DL-RE: "N_m3u8DL-RE version 0.5.1-beta" or just "v0.5.1-beta"
            let re = regex::Regex::new(r"v?(\d+\.\d+\.\d+)").ok()?;
            let caps = re.captures(first_line)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        _ => {
            // Generic: try to find any version-like pattern
            let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
            Some(re.find(first_line)?.as_str().to_string())
        }
    }
}

/// Checks if a detected version meets the minimum requirement.
///
/// Compares major.minor.patch tuples. Returns true if detected >= minimum.
/// Also returns true for special "nightly" versions (assumed to be bleeding edge).
fn meets_minimum_version(detected_version: &str, minimum_version: &str) -> bool {
    // Nightly/dev builds are assumed to be recent enough
    if detected_version == "nightly" || detected_version.contains("DEV") {
        return true;
    }

    let Some(detected) = parse_version_tuple(detected_version) else {
        return false; // Can't parse → assume incompatible
    };

    let Some(minimum) = parse_version_tuple(minimum_version) else {
        return true; // Can't parse minimum → accept anything
    };

    // Compare: major, then minor, then patch
    detected >= minimum
}

/// Checks if Rosetta 2 is available on macOS for running `x86_64` binaries
/// on Apple Silicon.
///
/// Returns `true` on non-macOS platforms or on Intel Macs (where Rosetta
/// isn't needed). Returns `false` only on Apple Silicon without Rosetta 2.
///
/// Detection method: checks for the Rosetta 2 runtime file at
/// `/Library/Apple/usr/share/rosetta/rosetta`, which is created when
/// Rosetta 2 is installed (either manually or auto-prompted by macOS).
fn is_rosetta2_available() -> bool {
    // Not macOS → Rosetta 2 is irrelevant
    if cfg!(not(target_os = "macos")) {
        return true;
    }

    // Intel Mac → no need for Rosetta 2
    if std::env::consts::ARCH != "aarch64" {
        return true;
    }

    // Apple Silicon → check if Rosetta 2 runtime is installed
    std::path::Path::new("/Library/Apple/usr/share/rosetta/rosetta").exists()
}

/// Searches for a tool in the system PATH and returns its path and version
/// if found and compatible with the minimum version requirement.
///
/// Uses `which` (Unix) or `where` (Windows) to locate the binary in PATH.
///
/// # Returns
/// * `Some((path, version))` if the tool is found in PATH and version could be detected
/// * `None` if the tool is not in PATH or version detection fails
pub async fn find_system_tool(tool_id: &str) -> Option<(PathBuf, String)> {
    let config = load_tool_version_config(tool_id)?;

    // Use `which` on Unix, `where` on Windows to search PATH
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    let output = tokio::process::Command::new(which_cmd)
        .arg(&config.binary_name)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // Parse the first line of output as the binary path
    let path_str = String::from_utf8_lossy(&output.stdout)
        .trim()
        .lines()
        .next()?
        .to_string();
    let path = PathBuf::from(&path_str);

    if !path.exists() {
        return None;
    }

    // Get the version using the tool's version flag
    let version_output = tokio::process::Command::new(&path)
        .arg(&config.version_flag)
        .output()
        .await
        .ok()?;

    // Combine stdout and stderr (some tools output version to stderr)
    let stdout = String::from_utf8_lossy(&version_output.stdout);
    let stderr = String::from_utf8_lossy(&version_output.stderr);
    let combined = if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };

    // Extract a version string from the output
    let version = extract_version_from_output(&combined, tool_id)?;

    log::info!(
        "Found system {} at {} (version {})",
        tool_id,
        path.display(),
        version
    );

    Some((path, version))
}

// ============================================================
// GitHub API resolution + mirror fallback infrastructure
// ============================================================

/// Queries a GitHub repository's release for an asset matching a name pattern.
///
/// This is a generic function used by both upstream GitHub API resolution
/// (e.g., N_m3u8DL-RE from nilaoda's repo) and the fallback mirror repo.
/// It queries the GitHub Releases API and searches the assets array for
/// a filename containing the specified substring.
///
/// # Arguments
/// * `repo` - GitHub repo in "owner/name" format (e.g., "nilaoda/N_m3u8DL-RE")
/// * `tag` - Release tag ("latest" uses the /releases/latest endpoint)
/// * `asset_name_contains` - Substring to match in asset filenames
///
/// # Returns
/// * `Ok((download_url, filename))` - The matching asset's URL and name
/// * `Err(message)` - If the API call fails or no matching asset is found
async fn resolve_github_release_asset(
    repo: &str,
    tag: &str,
    asset_name_contains: &str,
) -> Result<(String, String), String> {
    // Use the /releases/latest endpoint for "latest" tag,
    // or /releases/tags/{tag} for specific tags.
    let api_url = if tag == "latest" {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
    };

    // A User-Agent header is required by the GitHub API (returns 403 without it).
    // 15-second timeout prevents indefinite stalls on unresponsive GitHub API.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let response = client
        .get(&api_url)
        .header("User-Agent", "MeedyaDL")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed for {repo}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned HTTP {} for {}/releases/{}",
            response.status(),
            repo,
            tag
        ));
    }

    let release: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub release JSON from {repo}: {e}"))?;

    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| format!("No assets in {repo}/releases/{tag}"))?;

    // Search for an asset whose filename contains the specified pattern
    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        if name.contains(asset_name_contains) {
            let url = asset["browser_download_url"]
                .as_str()
                .ok_or("Missing download URL in GitHub release asset")?
                .to_string();
            return Ok((url, name.to_string()));
        }
    }

    Err(format!(
        "No asset matching '{}' in {}/releases/{}. Available: {:?}",
        asset_name_contains,
        repo,
        tag,
        assets
            .iter()
            .filter_map(|a| a["name"].as_str())
            .collect::<Vec<_>>()
    ))
}

/// Returns the standardized mirror asset name prefix for a tool
/// on the current platform.
///
/// Mirror assets follow the convention: `{tool_id}-{os}-{arch}`
/// The file extension varies (.zip for Windows, .tar.gz for Unix).
fn get_mirror_asset_prefix(tool_id: &str) -> Result<String, String> {
    let os_name = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        other => return Err(format!("Unsupported OS for mirror: {other}")),
    };
    let arch = std::env::consts::ARCH; // "x86_64" or "aarch64"

    Ok(format!("{tool_id}-{os_name}-{arch}"))
}

/// Queries the mirror repository for a tool's download URL.
///
/// The mirror at `MWBMPartners/meedyadl-tools` hosts pre-built binaries
/// with standardized naming. This function queries the repo's GitHub
/// Releases API to find the matching asset for the current platform.
///
/// This is used as a fallback when the primary upstream download source
/// fails (e.g., due to URL changes, server downtime, or naming convention
/// changes in upstream projects).
async fn get_mirror_download_url(
    tool_id: &str,
) -> Result<(String, archive::ArchiveFormat), String> {
    let mirror = load_mirror_config().ok_or(
        "Mirror not configured in tool-versions.toml. \
         Cannot fall back to mirror downloads.",
    )?;

    let asset_prefix = get_mirror_asset_prefix(tool_id)?;

    log::info!(
        "Querying mirror {}/releases/{} for asset matching '{}'",
        mirror.github_repo,
        mirror.release_tag,
        asset_prefix
    );

    // Query the mirror repo's release for our platform-specific asset
    let (url, filename) =
        resolve_github_release_asset(&mirror.github_repo, &mirror.release_tag, &asset_prefix)
            .await?;

    // Determine archive format from the matched filename extension.
    // Uses Path-based extension check for case-insensitive comparison.
    let format = if std::path::Path::new(&filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        archive::ArchiveFormat::Zip
    } else {
        archive::ArchiveFormat::TarGz // Covers .tar.gz, .tar.xz, etc.
    };

    log::info!("Mirror resolved: {asset_prefix} → {url}");
    Ok((url, format))
}

/// Downloads a tool's archive and extracts it to the tool directory,
/// with automatic fallback to the mirror repository if the primary
/// upstream source fails.
///
/// Resolution order:
///   1. Primary upstream source (hardcoded URL or upstream GitHub API)
///   2. MWBMPartners/meedyadl-tools mirror repository (fallback)
///
/// # Arguments
/// * `tool_id` - The tool identifier (e.g., "ffmpeg")
/// * `tool_dir` - The target extraction directory
async fn download_tool_with_fallback(
    tool_id: &str,
    tool_dir: &std::path::Path,
) -> Result<(), String> {
    // Prepare the directory (clean up any existing contents)
    if tool_dir.exists() {
        log::info!("Removing existing {tool_id} installation");
        std::fs::remove_dir_all(tool_dir)
            .map_err(|e| format!("Failed to remove existing {tool_id} directory: {e}"))?;
    }
    std::fs::create_dir_all(tool_dir)
        .map_err(|e| format!("Failed to create tool directory: {e}"))?;

    // Try primary upstream source
    let primary_error = match get_tool_download_url(tool_id).await {
        Ok((url, format)) => {
            log::info!("Downloading {tool_id} from primary source: {url}");
            match archive::download_and_extract(&url, tool_dir, format).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!("Primary download failed for {tool_id}: {e}");
                    e
                }
            }
        }
        Err(e) => {
            log::warn!("Primary URL resolution failed for {tool_id}: {e}");
            e
        }
    };

    // Primary failed — clean up and try mirror
    if tool_dir.exists() {
        std::fs::remove_dir_all(tool_dir).ok();
    }
    std::fs::create_dir_all(tool_dir)
        .map_err(|e| format!("Failed to recreate tool directory: {e}"))?;

    log::info!("Trying mirror fallback for {tool_id}...");
    match get_mirror_download_url(tool_id).await {
        Ok((mirror_url, mirror_format)) => {
            log::info!("Downloading {tool_id} from mirror: {mirror_url}");
            archive::download_and_extract(&mirror_url, tool_dir, mirror_format)
                .await
                .map_err(|e| {
                    format!(
                        "All download sources failed for {tool_id}.\n  Primary: {primary_error}\n  Mirror: {e}"
                    )
                })
        }
        Err(mirror_err) => Err(format!(
            "All download sources failed for {tool_id}.\n  Primary: {primary_error}\n  Mirror: {mirror_err}"
        )),
    }
}

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
    /// Human-readable display name shown in the UI (e.g., "`FFmpeg`")
    pub name: &'static str,
    /// Short machine-readable identifier used for directory names and API calls.
    /// Must match the `tool_id` parameter used in `install_tool()`, `get_tool_binary_path()`, etc.
    pub id: &'static str,
    /// Whether this tool is required for basic GAMDL functionality.
    /// The setup wizard highlights required tools and blocks completion until they're installed.
    pub required: bool,
    /// Brief description of what the tool is used for (shown in the setup wizard).
    pub description: &'static str,
}

/// All external tool dependencies and their metadata.
/// The first four tools are required for full functionality: `FFmpeg` for remuxing,
/// mp4decrypt for DRM decryption, N_m3u8DL-RE for HLS/DASH streams, and
/// `MP4Box` for MP4 muxing.
/// This list is returned by `get_all_tools()` for the setup wizard UI.
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
/// This function is async because N_m3u8DL-RE requires querying the GitHub
/// Releases API to resolve the correct asset URL (their naming convention
/// includes version and date, which can't be predicted statically).
///
/// # Arguments
/// * `tool_id` - The tool identifier (e.g., "ffmpeg", "mp4decrypt")
///
/// # Returns
/// * `Ok((url, format))` - The download URL and archive format
/// * `Err(message)` - If no pre-built binary is available for this platform
async fn get_tool_download_url(tool_id: &str) -> Result<(String, archive::ArchiveFormat), String> {
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
        "nm3u8dlre" => get_nm3u8dlre_url(os, arch).await,
        "mp4box" => get_mp4box_url(os, arch),
        _ => Err(format!("Unknown tool: {tool_id}")),
    }
}

/// Returns the `FFmpeg` download URL for the given platform.
///
/// Sources:
/// - Linux/Windows: BtbN/FFmpeg-Builds GitHub releases (latest master build)
/// - macOS: evermeet.cx static builds (`x86_64`) or osxcross builds (aarch64)
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
        ("windows", "x86_64" | "aarch64") => Ok((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
                .to_string(),
            archive::ArchiveFormat::Zip,
        )),
        // macOS (both architectures): evermeet.cx provides x86_64 static builds.
        // On Apple Silicon (aarch64), these require Rosetta 2 translation.
        // System PATH detection (checked before this function) will prefer a
        // native ARM64 FFmpeg if installed (e.g., via `brew install ffmpeg`).
        // Ref: https://evermeet.cx/ffmpeg/
        ("macos", _) => {
            if arch == "aarch64" && !is_rosetta2_available() {
                return Err(
                    "FFmpeg download requires Rosetta 2, which is not installed on this Mac. \
                     Install FFmpeg natively via Homebrew: brew install ffmpeg"
                        .to_string(),
                );
            }
            Ok((
                "https://evermeet.cx/ffmpeg/getrelease/zip".to_string(),
                archive::ArchiveFormat::Zip,
            ))
        }
        _ => Err(format!(
            "No pre-built FFmpeg available for {os}/{arch}. Install FFmpeg manually and set the path in Settings."
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
/// Ref: <https://www.bento4.com>/
fn get_mp4decrypt_url(os: &str, arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // Map OS/arch to Bento4's platform suffix naming convention
    let platform_suffix = match (os, arch) {
        // macOS: universal build (works on both x86_64 and aarch64 natively)
        ("macos", "x86_64" | "aarch64") => "universal-apple-macosx",
        // Linux: x86_64 only (ARM64 users would need to compile from source)
        // Naming convention changed at build 633: "linux-x86_64" → "x86_64-unknown-linux"
        ("linux", "x86_64" | "aarch64") => "x86_64-unknown-linux",
        // Windows: 64-bit only (32-bit builds dropped at build 633)
        // Naming convention changed: "win32" → "x86_64-microsoft-win32"
        ("windows", "x86_64" | "aarch64") => "x86_64-microsoft-win32",
        _ => return Err(format!("No pre-built mp4decrypt available for {os}/{arch}")),
    };

    // Bento4 SDK version 1.6.0-641 (latest stable as of writing).
    // The ZIP contains bin/{mp4decrypt, mp4info, mp4fragment, ...}.
    Ok((
        format!("https://www.bok.net/Bento4/binaries/Bento4-SDK-1-6-0-641.{platform_suffix}.zip"),
        archive::ArchiveFormat::Zip,
    ))
}

/// Returns the N_m3u8DL-RE download URL for the given platform.
///
/// N_m3u8DL-RE is a cross-platform HLS/DASH stream downloader that GAMDL
/// can use as an alternative to its built-in downloader. It's written in C#
/// (.NET) and provides native AOT-compiled binaries for each platform.
///
/// This function queries the GitHub Releases API via `resolve_github_release_asset()`
/// to find the correct asset URL, because N_m3u8DL-RE's release naming
/// convention includes version and date (e.g.,
/// `N_m3u8DL-RE_v0.5.1-beta_osx-arm64_20251029.tar.gz`), which can't be
/// predicted with a static URL pattern.
///
/// Ref: <https://github.com/nilaoda/N_m3u8DL-RE>
async fn get_nm3u8dlre_url(
    os: &str,
    arch: &str,
) -> Result<(String, archive::ArchiveFormat), String> {
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
                "No pre-built N_m3u8DL-RE available for {os}/{arch}"
            ))
        }
    };

    // Query the upstream GitHub Releases API to find the correct platform asset.
    // Example asset name: "N_m3u8DL-RE_v0.5.1-beta_osx-arm64_20251029.tar.gz"
    let (url, filename) =
        resolve_github_release_asset("nilaoda/N_m3u8DL-RE", "latest", rid).await?;

    // Determine archive format from the matched filename extension.
    // Uses Path-based extension check for case-insensitive comparison.
    let format = if std::path::Path::new(&filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        archive::ArchiveFormat::Zip
    } else {
        archive::ArchiveFormat::TarGz
    };

    log::info!("Resolved N_m3u8DL-RE asset: {filename}");
    Ok((url, format))
}

/// Returns the `MP4Box` (GPAC) download URL for the given platform.
///
/// `MP4Box` is the command-line tool from the GPAC multimedia framework.
/// GAMDL can use it as an alternative to `FFmpeg` for MP4 container operations
/// (muxing, demuxing, encryption handling).
///
/// GPAC provides nightly builds from their CI server at Telecom Paris.
/// Ref: <https://gpac.io>/
/// Ref: <https://github.com/gpac/gpac>
fn get_mp4box_url(_os: &str, _arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // MP4Box installation is handled entirely by platform-specific functions
    // (install_mp4box_macos, install_mp4box_windows, install_mp4box_linux)
    // that are intercepted in install_tool() before get_tool_download_url()
    // is called. This function should never be reached for MP4Box.
    Err("MP4Box installation is handled by platform-specific install functions".to_string())
}

/// Returns the path to a tool's installation directory.
///
/// Each tool gets its own subdirectory under {`app_data}/tools`/.
/// Example: {`app_data}/tools/ffmpeg`/
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier (e.g., "ffmpeg")
#[must_use]
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
#[must_use]
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
        "ffmpeg" => format!("ffmpeg{exe_ext}"),
        "mp4decrypt" => format!("mp4decrypt{exe_ext}"),
        "nm3u8dlre" => format!("N_m3u8DL-RE{exe_ext}"),
        "mp4box" => format!("MP4Box{exe_ext}"),
        _ => format!("{tool_id}{exe_ext}"),
    };

    // The binary is expected at {app_data}/tools/{tool_id}/{binary_name}
    // e.g., {app_data}/tools/ffmpeg/ffmpeg
    tool_dir.join(binary_name)
}

/// Resolves a tool display name or ID to the canonical tool ID.
///
/// The frontend sends tool display names (e.g., "`FFmpeg`", "N_m3u8DL-RE")
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
    Err(format!("Unknown tool: {name_or_id}"))
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
/// * `name_or_id` - The tool display name or identifier (e.g., "`FFmpeg`" or "ffmpeg")
///
/// # Errors
///
/// Returns `Err(String)` if the tool ID is unrecognized, the download fails,
/// archive extraction fails, or the installed binary fails verification.
///
/// # Returns
/// * `Ok(version)` - The installed version string (or "installed" if version detection fails)
/// * `Err(message)` - A descriptive error if installation failed
pub async fn install_tool(app: &AppHandle, name_or_id: &str) -> Result<String, String> {
    // Resolve display name to canonical tool ID (e.g., "FFmpeg" -> "ffmpeg")
    let tool_id = resolve_tool_id(name_or_id)?;
    log::info!("Starting installation of tool: {tool_id}");

    // Step 0: Check if a compatible version exists in the system PATH.
    // If found, copy it to our managed tools directory instead of downloading.
    // This saves bandwidth and respects existing installations.
    if let Some((system_path, system_version)) = find_system_tool(tool_id).await {
        let config = load_tool_version_config(tool_id);
        let is_compatible = config
            .as_ref()
            .is_none_or(|c| meets_minimum_version(&system_version, &c.minimum_version)); // If no config, accept any version

        if is_compatible {
            log::info!(
                "Using system {} (version {}) from {}",
                tool_id,
                system_version,
                system_path.display()
            );

            // Prepare the tool directory and copy the system binary
            let tool_dir = get_tool_dir(app, tool_id);
            if tool_dir.exists() {
                std::fs::remove_dir_all(&tool_dir).ok();
            }
            std::fs::create_dir_all(&tool_dir)
                .map_err(|e| format!("Failed to create tool directory: {e}"))?;

            let expected_binary = get_tool_binary_path(app, tool_id);
            std::fs::copy(&system_path, &expected_binary).map_err(|e| {
                format!(
                    "Failed to copy system {} to {}: {}",
                    tool_id,
                    expected_binary.display(),
                    e
                )
            })?;

            archive::set_executable(&expected_binary)?;

            // Write a .source marker file so check_all_dependencies knows this
            // tool came from the system PATH rather than being downloaded.
            let source_marker = tool_dir.join(".source");
            std::fs::write(&source_marker, "system").ok();

            log::info!(
                "Copied system {} to managed directory: {}",
                tool_id,
                expected_binary.display()
            );
            return Ok(format!("{system_version} (system)"));
        }
        log::info!(
            "System {tool_id} version {system_version} does not meet minimum requirement, downloading fresh copy"
        );
    }

    // Special case: MP4Box requires platform-specific installation on all platforms.
    // GPAC discontinued ZIP/tar.gz downloads — each platform now uses different formats:
    //   macOS:   Homebrew (ARM64 native) with .pkg fallback (x86_64 via Rosetta 2)
    //   Windows: NSIS .exe installer (silent install + binary extraction)
    //   Linux:   .deb package (extracted using ar + tar without installation)
    // If the platform-specific installer fails, falls back to the mirror.
    if tool_id == "mp4box" {
        return install_mp4box_with_fallback(app).await;
    }

    // Step 1-3: Download with automatic mirror fallback.
    // Tries the primary upstream source first (hardcoded URL or GitHub API),
    // then falls back to the MWBMPartners/meedyadl-tools mirror repository.
    let tool_dir = get_tool_dir(app, tool_id);
    download_tool_with_fallback(tool_id, &tool_dir).await?;

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
                format!("Failed to copy {tool_id} binary to expected location: {e}")
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

    // Write a .source marker file so check_all_dependencies knows this
    // tool was downloaded and managed by the app.
    let source_marker = tool_dir.join(".source");
    std::fs::write(&source_marker, "managed").ok();

    // Step 6: Try to get the version (best-effort)
    let version = get_tool_version(&expected_binary, tool_id)
        .await
        .unwrap_or_else(|_| "installed".to_string());

    log::info!("{tool_id} {version} installed successfully");
    Ok(version)
}

/// Installs `MP4Box` on macOS using a layered strategy.
///
/// GPAC (the project that provides `MP4Box`) doesn't publish downloadable
/// ZIP/tar.gz archives for macOS. This function tries two approaches:
///
/// 1. **Primary**: Homebrew (`brew install gpac`) — gives a native ARM64
///    build on Apple Silicon, handles all dependencies automatically.
/// 2. **Fallback**: Extract from GPAC's official `.pkg` installer — works
///    for everyone without needing Homebrew. The `.pkg` contains `x86_64`
///    binaries that run on Apple Silicon via Rosetta 2. **Note**: Rosetta 2
///    may be deprecated in a future macOS release, so the Homebrew path
///    should be preferred on Apple Silicon.
///
/// # Arguments
/// * `app` - The Tauri app handle (for resolving the tools directory)
///
/// # Returns
/// * `Ok(version)` - The installed version string
/// * `Err(message)` - If both installation methods fail
async fn install_mp4box_macos(app: &AppHandle) -> Result<String, String> {
    log::info!("Installing MP4Box on macOS");

    // Try Homebrew first — gives native ARM64 build on Apple Silicon
    let brew_path = if std::path::Path::new("/opt/homebrew/bin/brew").exists() {
        Some("/opt/homebrew/bin/brew")
    } else if std::path::Path::new("/usr/local/bin/brew").exists() {
        Some("/usr/local/bin/brew")
    } else {
        None
    };

    if let Some(brew) = brew_path {
        log::info!("Found Homebrew at {brew}, attempting brew install gpac");
        match install_mp4box_via_homebrew(app, brew).await {
            Ok(version) => return Ok(version),
            Err(e) => {
                log::warn!(
                    "Homebrew installation failed: {e}. Falling back to GPAC .pkg extraction."
                );
            }
        }
    } else {
        log::info!("Homebrew not found, using GPAC .pkg extraction");
    }

    // Fallback: download and extract from GPAC's official .pkg installer
    install_mp4box_from_pkg(app).await
}

/// Attempts to install `MP4Box` via Homebrew (`brew install gpac`).
///
/// Runs `brew install gpac`, then locates the installed `MP4Box` binary
/// and copies it to `MeedyaDL`'s tool directory.
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `brew_path` - Absolute path to the `brew` binary
async fn install_mp4box_via_homebrew(app: &AppHandle, brew_path: &str) -> Result<String, String> {
    // Run `brew install gpac` to install MP4Box.
    // GPAC is the multimedia framework that includes the MP4Box binary.
    let install_output = tokio::process::Command::new(brew_path)
        .args(["install", "gpac"])
        .output()
        .await
        .map_err(|e| format!("Failed to run 'brew install gpac': {e}"))?;

    if !install_output.status.success() {
        let stderr = String::from_utf8_lossy(&install_output.stderr);
        return Err(format!("'brew install gpac' failed: {}", stderr.trim()));
    }

    log::info!("'brew install gpac' completed successfully");

    // Find the installed MP4Box binary using `brew --prefix gpac`.
    // Returns the Homebrew cellar path (e.g., /opt/homebrew/opt/gpac).
    let prefix_output = tokio::process::Command::new(brew_path)
        .args(["--prefix", "gpac"])
        .output()
        .await
        .map_err(|e| format!("Failed to get gpac prefix: {e}"))?;

    let prefix = String::from_utf8_lossy(&prefix_output.stdout)
        .trim()
        .to_string();

    if prefix.is_empty() {
        return Err("Could not determine gpac installation prefix".to_string());
    }

    let brew_binary = std::path::PathBuf::from(&prefix).join("bin/MP4Box");
    if !brew_binary.exists() {
        // Also check lowercase variant (some Homebrew versions use "mp4box")
        let brew_binary_lower = std::path::PathBuf::from(&prefix).join("bin/mp4box");
        if !brew_binary_lower.exists() {
            return Err(format!(
                "MP4Box binary not found at {} or {} after brew install",
                brew_binary.display(),
                brew_binary_lower.display()
            ));
        }
        return copy_and_verify_mp4box(app, &brew_binary_lower, "Homebrew").await;
    }

    copy_and_verify_mp4box(app, &brew_binary, "Homebrew").await
}

/// Installs `MP4Box` by downloading and extracting GPAC's official macOS `.pkg`.
///
/// The `.pkg` installer from GPAC's CI server contains a full `GPAC.app`
/// bundle with `MP4Box` and its required dynamic libraries under
/// `Contents/MacOS/lib/`. The binary uses `@executable_path/lib/` references,
/// so it works from any location as long as the `lib/` directory is a sibling.
///
/// Note: The `.pkg` currently provides `x86_64` binaries only. On Apple Silicon,
/// these require Rosetta 2 for translation. If Rosetta 2 is not installed,
/// this function returns an error directing the user to install via Homebrew.
///
/// Source: <https://gpac.io/downloads>/
/// Nightly permalink: <https://download.tsi.telecom-paristech.fr/gpac/new_builds>/
async fn install_mp4box_from_pkg(app: &AppHandle) -> Result<String, String> {
    // On Apple Silicon, the .pkg only contains x86_64 binaries — Rosetta 2 is required
    if std::env::consts::ARCH == "aarch64" && !is_rosetta2_available() {
        return Err(
            "MP4Box .pkg fallback requires Rosetta 2, which is not installed on this Mac. \
             Install GPAC natively via Homebrew: brew install gpac"
                .to_string(),
        );
    }
    log::info!("Installing MP4Box from GPAC official .pkg");

    // Create temp directory for download and extraction
    let temp_dir = std::env::temp_dir().join("meedyadl_gpac_install");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    // Run the extraction in an inner function so we can clean up regardless of result
    let result = install_mp4box_from_pkg_inner(app, &temp_dir).await;

    // Always clean up temp files
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    result
}

/// Inner implementation for .pkg extraction (separated for cleanup handling).
async fn install_mp4box_from_pkg_inner(
    app: &AppHandle,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    // Download the GPAC .pkg from the nightly builds permalink.
    // This URL always points to the latest master branch build.
    let pkg_url =
        "https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_macos.pkg";
    let pkg_path = temp_dir.join("gpac.pkg");

    log::info!("Downloading GPAC .pkg from {pkg_url}");
    archive::download_file(pkg_url, &pkg_path).await?;

    // Step 1: Expand the .pkg using pkgutil (standard macOS tool).
    // This unpacks the XAR archive into a directory structure containing
    // sub-packages, each with a Payload (gzip-compressed cpio archive).
    let expanded_dir = temp_dir.join("expanded");
    let expand_status = tokio::process::Command::new("pkgutil")
        .args([
            "--expand",
            &pkg_path.to_string_lossy(),
            &expanded_dir.to_string_lossy(),
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run pkgutil --expand: {e}"))?;

    if !expand_status.status.success() {
        let stderr = String::from_utf8_lossy(&expand_status.stderr);
        return Err(format!("pkgutil --expand failed: {}", stderr.trim()));
    }

    // Step 2: Find the Payload file inside the expanded package.
    // Structure: expanded/{sub-package}/Payload
    let payload_path = find_pkg_payload(&expanded_dir)?;

    // Step 3: Extract the Payload (gzip-compressed cpio archive).
    // Uses macOS system tools: gunzip decompresses, cpio extracts the archive.
    let payload_dir = temp_dir.join("payload");
    std::fs::create_dir_all(&payload_dir)
        .map_err(|e| format!("Failed to create payload directory: {e}"))?;

    let extract_status = tokio::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "gunzip -c '{}' | cpio -id 2>/dev/null",
                payload_path.to_string_lossy()
            ),
        ])
        .current_dir(&payload_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to extract GPAC payload: {e}"))?;

    if !extract_status.status.success() {
        let stderr = String::from_utf8_lossy(&extract_status.stderr);
        return Err(format!(
            "Failed to extract GPAC .pkg payload: {}",
            stderr.trim()
        ));
    }

    // Step 4: Locate MP4Box and its lib/ directory in the extracted GPAC.app bundle.
    // The .pkg installs GPAC.app to /Applications, so the extracted layout is:
    //   payload/GPAC.app/Contents/MacOS/MP4Box
    //   payload/GPAC.app/Contents/MacOS/lib/ (dynamic libraries)
    let macos_dir = payload_dir.join("GPAC.app/Contents/MacOS");
    let mp4box_src = macos_dir.join("MP4Box");
    let lib_src = macos_dir.join("lib");

    if !mp4box_src.exists() {
        return Err(format!(
            "MP4Box binary not found in extracted GPAC package at {}",
            mp4box_src.display()
        ));
    }

    // Step 5: Prepare the tool directory and copy MP4Box + libs.
    // MP4Box links against @executable_path/lib/libgpac.dylib, so the lib/
    // directory must be a sibling of the binary for it to find its dependencies.
    let tool_dir = get_tool_dir(app, "mp4box");
    if tool_dir.exists() {
        std::fs::remove_dir_all(&tool_dir)
            .map_err(|e| format!("Failed to remove existing mp4box directory: {e}"))?;
    }
    std::fs::create_dir_all(&tool_dir)
        .map_err(|e| format!("Failed to create mp4box tool directory: {e}"))?;

    // Copy the MP4Box binary
    let mp4box_dest = get_tool_binary_path(app, "mp4box");
    std::fs::copy(&mp4box_src, &mp4box_dest)
        .map_err(|e| format!("Failed to copy MP4Box binary: {e}"))?;

    // Copy the lib/ directory (contains libgpac.dylib and its transitive deps)
    if lib_src.exists() && lib_src.is_dir() {
        let lib_dest = tool_dir.join("lib");
        copy_dir_all(&lib_src, &lib_dest)
            .map_err(|e| format!("Failed to copy GPAC libraries: {e}"))?;
        log::info!(
            "Copied GPAC lib/ directory ({} entries) to {}",
            std::fs::read_dir(&lib_dest)
                .map(std::iter::Iterator::count)
                .unwrap_or(0),
            lib_dest.display()
        );
    }

    // Step 6: Set executable permissions and verify
    archive::set_executable(&mp4box_dest)?;

    let version = get_tool_version(&mp4box_dest, "mp4box")
        .await
        .unwrap_or_else(|_| "installed".to_string());

    log::info!("MP4Box {version} installed from GPAC .pkg successfully");
    Ok(version)
}

/// Finds the Payload file inside an expanded .pkg directory.
///
/// macOS .pkg files contain sub-packages, each with a `Payload` file that
/// holds the actual installable content as a gzip-compressed cpio archive.
fn find_pkg_payload(expanded_dir: &std::path::Path) -> Result<PathBuf, String> {
    // Walk one level deep looking for a Payload file inside sub-packages
    if let Ok(entries) = std::fs::read_dir(expanded_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let payload = path.join("Payload");
                if payload.exists() {
                    return Ok(payload);
                }
            }
        }
    }

    // Also check for a Payload directly in the expanded dir (flat pkg format)
    let direct_payload = expanded_dir.join("Payload");
    if direct_payload.exists() {
        return Ok(direct_payload);
    }

    Err(format!(
        "No Payload found in expanded .pkg at {}",
        expanded_dir.display()
    ))
}

/// Recursively copies a directory and all its contents to a new location.
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create directory {}: {}", dst.display(), e))?;

    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {}: {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| {
                format!(
                    "Failed to copy {} to {}: {}",
                    src_path.display(),
                    dst_path.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

/// Copies an `MP4Box` binary to `MeedyaDL`'s tool directory and verifies it works.
///
/// Used by both the Homebrew and .pkg installation paths to finalize the install.
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `source_binary` - Path to the `MP4Box` binary to copy
/// * `source_label` - Label for log messages (e.g., "Homebrew", "GPAC .pkg")
async fn copy_and_verify_mp4box(
    app: &AppHandle,
    source_binary: &std::path::Path,
    source_label: &str,
) -> Result<String, String> {
    let tool_dir = get_tool_dir(app, "mp4box");

    // Clean up existing installation if present
    if tool_dir.exists() {
        std::fs::remove_dir_all(&tool_dir)
            .map_err(|e| format!("Failed to remove existing mp4box directory: {e}"))?;
    }

    std::fs::create_dir_all(&tool_dir)
        .map_err(|e| format!("Failed to create mp4box tool directory: {e}"))?;

    let expected_binary = get_tool_binary_path(app, "mp4box");
    std::fs::copy(source_binary, &expected_binary).map_err(|e| {
        format!(
            "Failed to copy MP4Box from {} to {}: {}",
            source_binary.display(),
            expected_binary.display(),
            e
        )
    })?;

    log::info!(
        "Copied MP4Box from {} to {}",
        source_binary.display(),
        expected_binary.display()
    );

    // Set executable permissions and verify
    archive::set_executable(&expected_binary)?;

    // Write a .source marker so check_all_dependencies knows the install origin.
    let source_marker = tool_dir.join(".source");
    std::fs::write(&source_marker, "managed").ok();

    let version = get_tool_version(&expected_binary, "mp4box")
        .await
        .unwrap_or_else(|_| "installed".to_string());

    log::info!("MP4Box {version} installed via {source_label} successfully");
    Ok(version)
}

/// Installs `MP4Box` on Windows by downloading and silently running GPAC's NSIS installer.
///
/// GPAC discontinued ZIP archives for Windows — only NSIS `.exe` installers remain.
/// This function downloads the installer, runs it with `/S` (silent) and `/D=` (custom
/// install directory) to extract files without user interaction, then copies MP4Box.exe
/// to `MeedyaDL`'s tool directory.
///
/// The NSIS `/D=` flag installs to a user-specified directory without requiring admin
/// privileges (as long as the directory is user-writable).
///
/// Source: <https://download.tsi.telecom-paristech.fr/gpac/new_builds>/
async fn install_mp4box_windows(app: &AppHandle) -> Result<String, String> {
    log::info!("Installing MP4Box on Windows via GPAC NSIS installer");

    // Create temp directory for the installer
    let temp_dir = std::env::temp_dir().join("meedyadl_gpac_install");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    let result = install_mp4box_windows_inner(app, &temp_dir).await;

    // Always clean up temp files
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    result
}

/// Inner implementation for Windows GPAC NSIS installer extraction.
async fn install_mp4box_windows_inner(
    app: &AppHandle,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    // Download the GPAC NSIS installer from the nightly builds permalink
    let installer_url =
        "https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_win64.exe";
    let installer_path = temp_dir.join("gpac_installer.exe");

    log::info!("Downloading GPAC installer from {installer_url}");
    archive::download_file(installer_url, &installer_path).await?;

    // Run the NSIS installer silently with /S (silent) and /D= (install directory).
    // NSIS installers support these flags natively. The /D= flag must be the last
    // parameter and must not be quoted.
    let install_dir = temp_dir.join("gpac");
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create GPAC install directory: {e}"))?;

    log::info!(
        "Running GPAC installer silently to {}",
        install_dir.display()
    );
    let install_status = tokio::process::Command::new(&installer_path)
        .arg("/S")
        .arg(format!("/D={}", install_dir.display()))
        .output()
        .await
        .map_err(|e| format!("Failed to run GPAC installer: {e}"))?;

    if !install_status.status.success() {
        let stderr = String::from_utf8_lossy(&install_status.stderr);
        return Err(format!("GPAC silent install failed: {}", stderr.trim()));
    }

    // Find MP4Box.exe in the installed directory tree
    let mp4box_src = find_binary_recursive(&install_dir, "mp4box");
    let mp4box_src = mp4box_src.ok_or_else(|| {
        format!(
            "MP4Box.exe not found in GPAC installation at {}",
            install_dir.display()
        )
    })?;

    log::info!("Found MP4Box at {}", mp4box_src.display());

    // Copy to MeedyaDL's tool directory and verify
    copy_and_verify_mp4box(app, &mp4box_src, "GPAC NSIS installer").await
}

/// Installs `MP4Box` on Linux by downloading and extracting GPAC's `.deb` package.
///
/// GPAC provides nightly `.deb` packages (not tar.gz archives) for Linux.
/// This function downloads the `.deb` and extracts it without installing it
/// system-wide, using standard tools (`ar` to unpack the deb, `tar` to
/// extract the data payload).
///
/// A `.deb` file is an `ar` archive containing:
///   - `debian-binary` (version string)
///   - `control.tar.*` (package metadata)
///   - `data.tar.*` (actual installed files)
///
/// Source: <https://download.tsi.telecom-paristech.fr/gpac/new_builds>/
async fn install_mp4box_linux(app: &AppHandle) -> Result<String, String> {
    log::info!("Installing MP4Box on Linux from GPAC .deb package");

    // Create temp directory
    let temp_dir = std::env::temp_dir().join("meedyadl_gpac_install");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    let result = install_mp4box_linux_inner(app, &temp_dir).await;

    // Always clean up temp files
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    result
}

/// Inner implementation for Linux GPAC .deb extraction.
async fn install_mp4box_linux_inner(
    app: &AppHandle,
    temp_dir: &std::path::Path,
) -> Result<String, String> {
    // Download the GPAC .deb from the nightly builds permalink
    let deb_url =
        "https://download.tsi.telecom-paristech.fr/gpac/new_builds/gpac_latest_head_linux64.deb";
    let deb_path = temp_dir.join("gpac.deb");

    log::info!("Downloading GPAC .deb from {deb_url}");
    archive::download_file(deb_url, &deb_path).await?;

    // Extract the .deb using `ar` (part of binutils, standard on Linux)
    let ar_status = tokio::process::Command::new("ar")
        .args(["x", &deb_path.to_string_lossy()])
        .current_dir(temp_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to run 'ar x' on .deb: {e}"))?;

    if !ar_status.status.success() {
        let stderr = String::from_utf8_lossy(&ar_status.stderr);
        return Err(format!("Failed to extract .deb archive: {}", stderr.trim()));
    }

    // Find the data archive (could be data.tar.xz, data.tar.gz, or data.tar.zst)
    let data_dir = temp_dir.join("data");
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {e}"))?;

    // Use shell globbing to find and extract the data archive
    let extract_status = tokio::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "tar xf '{}/data.tar.'* -C '{}'",
                temp_dir.to_string_lossy(),
                data_dir.to_string_lossy()
            ),
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to extract data archive: {e}"))?;

    if !extract_status.status.success() {
        let stderr = String::from_utf8_lossy(&extract_status.stderr);
        return Err(format!(
            "Failed to extract GPAC data archive: {}",
            stderr.trim()
        ));
    }

    // Find MP4Box binary in the extracted directory tree
    // Typical path: usr/bin/MP4Box or usr/local/bin/MP4Box
    let mp4box_src = find_binary_recursive(&data_dir, "mp4box");
    let mp4box_src = mp4box_src.ok_or_else(|| {
        format!(
            "MP4Box binary not found in extracted GPAC .deb at {}",
            data_dir.display()
        )
    })?;

    log::info!("Found MP4Box at {}", mp4box_src.display());

    // Copy to MeedyaDL's tool directory and verify
    copy_and_verify_mp4box(app, &mp4box_src, "GPAC .deb package").await
}

/// Installs `MP4Box` using platform-specific installers with mirror fallback.
///
/// First tries the platform's native installation method:
///   - macOS: Homebrew → .pkg extraction
///   - Windows: NSIS silent installer
///   - Linux: .deb extraction
///
/// If the platform-specific method fails, falls back to the
/// MWBMPartners/meedyadl-tools mirror repository for a generic binary archive.
async fn install_mp4box_with_fallback(app: &AppHandle) -> Result<String, String> {
    // Try platform-specific installer first
    let platform_result = match std::env::consts::OS {
        "macos" => install_mp4box_macos(app).await,
        "windows" => install_mp4box_windows(app).await,
        "linux" => install_mp4box_linux(app).await,
        _ => Err(format!(
            "MP4Box installation not supported on {}",
            std::env::consts::OS
        )),
    };

    match platform_result {
        Ok(version) => Ok(version),
        Err(primary_err) => {
            log::warn!("Platform-specific MP4Box install failed: {primary_err}. Trying mirror...");

            // Fall back to mirror directly (skip get_tool_download_url which
            // returns Err for MP4Box since it uses platform-specific installers)
            let tool_dir = get_tool_dir(app, "mp4box");
            if tool_dir.exists() {
                std::fs::remove_dir_all(&tool_dir).ok();
            }
            std::fs::create_dir_all(&tool_dir)
                .map_err(|e| format!("Failed to create tool directory: {e}"))?;

            let (mirror_url, mirror_format) =
                get_mirror_download_url("mp4box").await.map_err(|e| {
                    format!(
                        "All sources failed for MP4Box.\n  Platform: {primary_err}\n  Mirror: {e}"
                    )
                })?;

            log::info!("Downloading MP4Box from mirror: {mirror_url}");
            archive::download_and_extract(&mirror_url, &tool_dir, mirror_format)
                .await
                .map_err(|e| {
                    format!(
                        "All sources failed for MP4Box.\n  Platform: {primary_err}\n  Mirror download: {e}"
                    )
                })?;

            // Find binary in extracted mirror archive
            let expected_binary = get_tool_binary_path(app, "mp4box");
            if !expected_binary.exists() {
                if let Some(found) = find_binary_recursive(&tool_dir, "mp4box") {
                    std::fs::copy(&found, &expected_binary)
                        .map_err(|e| format!("Failed to copy MP4Box binary: {e}"))?;
                } else {
                    return Err(format!(
                        "All sources failed for MP4Box.\n  Platform: {primary_err}\n  Mirror: binary not found in archive"
                    ));
                }
            }

            archive::set_executable(&expected_binary)?;

            // Write .source marker for the mirror-sourced install
            let source_marker = tool_dir.join(".source");
            std::fs::write(&source_marker, "managed").ok();

            let version = get_tool_version(&expected_binary, "mp4box")
                .await
                .unwrap_or_else(|_| "installed".to_string());

            log::info!("MP4Box {version} installed from mirror");
            Ok(version)
        }
    }
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
        // Both FFmpeg and MP4Box use single-dash "-version" (non-standard)
        "ffmpeg" | "mp4box" => "-version",
        _ => "--version", // Standard GNU-style flag
    };

    // Run the binary with the version flag and capture output.
    // This serves as both a version check and a basic health check
    // (verifying the binary is executable and not corrupt).
    let output = tokio::process::Command::new(binary_path)
        .arg(version_flag)
        .output()
        .await
        .map_err(|e| format!("Failed to run {tool_id} --version: {e}"))?;

    // Extract the first line of stdout as the version string.
    // Most tools output their version on the first line of stdout.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("").trim().to_string();

    if first_line.is_empty() {
        // Some tools output version info to stderr (e.g., FFmpeg logs to stderr).
        // Fall back to the first line of stderr.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first_err_line = stderr
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string();
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
#[must_use]
pub fn is_tool_installed(app: &AppHandle, tool_id: &str) -> bool {
    get_tool_binary_path(app, tool_id).exists()
}

/// Returns the list of all tool dependencies with their metadata.
///
/// Used by the setup wizard and dependency status UI to display
/// the full list of tools with their installation requirements.
#[must_use]
pub const fn get_all_tools() -> &'static [ToolInfo] {
    TOOLS
}

/// Removes a tool's installation directory and all its contents.
///
/// Used when the user wants to reinstall a tool or when the installation
/// is detected as corrupt. Uses async filesystem operations to avoid
/// blocking the Tokio runtime.
/// Ref: <https://docs.rs/tokio/latest/tokio/fs/fn.remove_dir_all.html>
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `tool_id` - The tool identifier (e.g., "ffmpeg", "mp4decrypt")
///
/// # Errors
///
/// Returns `Err(String)` if the tool directory cannot be removed.
pub async fn uninstall_tool(app: &AppHandle, tool_id: &str) -> Result<(), String> {
    let tool_dir = get_tool_dir(app, tool_id);

    if tool_dir.exists() {
        log::info!(
            "Removing {} installation at {}",
            tool_id,
            tool_dir.display()
        );
        tokio::fs::remove_dir_all(&tool_dir)
            .await
            .map_err(|e| format!("Failed to remove {tool_id} directory: {e}"))?;
    }

    Ok(())
}
