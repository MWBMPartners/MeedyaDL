// Copyright (c) 2026 MeedyaSuite
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
use std::sync::LazyLock;
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
    /// GitHub repository in "owner/name" format (e.g., "MeedyaSuite/MeedyaDL-Tools")
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

// Regex caches.
//
// Compiling a regex involves parsing the pattern + building an NFA, costing
// hundreds of microseconds. Tool-version probing runs at every startup and
// on every "Check for updates" click, so per-call compilation is wasted
// work that adds up across a session. Each pattern is compiled once via
// `LazyLock` and reused for the rest of the process lifetime — same shape
// as `apple_music_api::parse_apple_music_url` and `process::ERROR_PREFIX_REGEX`.
//
// All patterns are static literals with no user input, so `.expect()` on
// `Regex::new` only fires on developer typos at boot — never at runtime.
static VERSION_TUPLE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").expect("static regex"));
static SEMVER_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").expect("static regex"));
static MP4BOX_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?:GPAC|version)\s+(\d+\.\d+(?:\.\d+)?)").expect("static regex")
});
static MP4DECRYPT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"Bento4 Version (\d+\.\d+\.\d+)").expect("static regex")
});
static NM3U8DL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"v?(\d+\.\d+\.\d+)").expect("static regex"));
static MEDIAINFO_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"v(\d+\.\d+)").expect("static regex"));

/// Parses a version string into (major, minor, patch) components.
///
/// Handles various formats:
///   - "6.1.2" → (6, 1, 2)
///   - "5.0" → (5, 0, 0)
///   - "2.4-DEV" → (2, 4, 0)
///   - "N-112479-..." → None (`FFmpeg` nightly builds without numeric version)
fn parse_version_tuple(version_str: &str) -> Option<(u32, u32, u32)> {
    let caps = VERSION_TUPLE_RE.captures(version_str)?;

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
            Some(SEMVER_RE.find(after_version)?.as_str().to_string())
        }
        "mp4box" => {
            // MP4Box: "MP4Box - GPAC version 2.4-DEV..." or "GPAC version 2.4..."
            let caps = MP4BOX_RE.captures(first_line)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        "mp4decrypt" => {
            // mp4decrypt has no --version flag. When run with no args, outputs:
            //   MP4 Decrypter - Version 1.4
            //   (Bento4 Version 1.6.0.0)
            // Extract the Bento4 version from the full output.
            let caps = MP4DECRYPT_RE.captures(output)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        "nm3u8dlre" => {
            // N_m3u8DL-RE: "N_m3u8DL-RE version 0.5.1-beta" or just "v0.5.1-beta"
            let caps = NM3U8DL_RE.captures(first_line)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        "mediainfo" => {
            // MediaInfo: "MediaInfo Command line,\nMediaInfoLib - v26.01"
            // The version is on the second line, but first_line may be the first.
            // Try to find "v{major}.{minor}" anywhere in the output.
            let caps = MEDIAINFO_RE.captures(output)?;
            Some(caps.get(1)?.as_str().to_string())
        }
        _ => {
            // Generic: try to find any version-like pattern
            Some(SEMVER_RE.find(first_line)?.as_str().to_string())
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

    let path = if output.status.success() {
        // Parse the first line of output as the binary path
        let path_str = String::from_utf8_lossy(&output.stdout)
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let p = PathBuf::from(&path_str);
        if p.exists() { Some(p) } else { None }
    } else {
        None
    };

    // If not on PATH, check common platform-specific installation locations.
    // macOS: Homebrew paths, /usr/local/bin, and common app bundle locations.
    let path = path.or_else(|| {
        let extra_paths: Vec<PathBuf> = if cfg!(target_os = "macos") {
            vec![
                PathBuf::from("/usr/local/bin").join(&config.binary_name),
                PathBuf::from("/opt/homebrew/bin").join(&config.binary_name),
                // MediaInfo-specific: Homebrew installs as lowercase
                PathBuf::from("/opt/homebrew/bin/mediainfo"),
                PathBuf::from("/usr/local/bin/mediainfo"),
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                PathBuf::from("/usr/bin").join(&config.binary_name),
                PathBuf::from("/usr/local/bin").join(&config.binary_name),
            ]
        } else {
            vec![]
        };
        extra_paths.into_iter().find(|p| p.exists())
    });

    let path = path?;

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
    // Use /releases/tags/{tag} for deterministic resolution. The /releases/latest
    // endpoint returns the "most recently created" release, which may differ from
    // the release explicitly tagged "latest" when a repo has multiple releases.
    // /tags/ ensures we always get the exact tag we asked for.
    //
    // Exception: upstream repos (non-mirror) that use "latest" as a convention
    // for "newest release" without an actual "latest" tag need the /releases/latest
    // endpoint. We detect this by trying /tags/ first and falling back.
    // A User-Agent header is required by the GitHub API (returns 403 without it).
    // 15-second timeout prevents indefinite stalls on unresponsive GitHub API.
    let client = crate::utils::http_client::build_simple(15)?;

    // Try /releases/tags/{tag} first (deterministic, exact tag match).
    // Fall back to /releases/latest if the tag doesn't exist as an explicit tag
    // (e.g., upstream repos that use "latest" as a GitHub convention, not a git tag).
    let tag_url = format!("https://api.github.com/repos/{repo}/releases/tags/{tag}");
    let response = client
        .get(&tag_url)
        .header("User-Agent", "MeedyaDL")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed for {repo}: {e}"))?;

    let release: serde_json::Value = if response.status().as_u16() == 404 && tag == "latest" {
        // Tag "latest" doesn't exist as a git tag — fall back to /releases/latest
        log::debug!("No 'latest' tag in {repo}, falling back to /releases/latest");
        let fallback_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let fallback_resp = client
            .get(&fallback_url)
            .header("User-Agent", "MeedyaDL")
            .send()
            .await
            .map_err(|e| format!("GitHub API fallback request failed for {repo}: {e}"))?;
        if !fallback_resp.status().is_success() {
            return Err(format!(
                "GitHub API returned HTTP {} for {}/releases/latest",
                fallback_resp.status(),
                repo
            ));
        }
        fallback_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse GitHub release JSON from {repo}: {e}"))?
    } else if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned HTTP {} for {}/releases/tags/{}",
            response.status(),
            repo,
            tag
        ));
    } else {
        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse GitHub release JSON from {repo}: {e}"))?
    };

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
/// The mirror at `MeedyaSuite/MeedyaDL-Tools` hosts pre-built binaries
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
///   2. MeedyaSuite/MeedyaDL-Tools mirror repository (fallback)
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
    ToolInfo {
        name: "MediaInfo",
        id: "mediainfo",
        required: true,
        description: "Media file analysis for accurate codec detection",
    },
    // rclone is OPTIONAL: only needed when the user enables direct-to-cloud
    // upload (M11, #859). `required: false` keeps the setup wizard non-blocking
    // when rclone is absent. Cloud Destination settings will trigger
    // `install_tool(app, "rclone")` on-demand when the feature is enabled.
    ToolInfo {
        name: "rclone",
        id: "rclone",
        required: false,
        description: "Direct-to-cloud upload (optional; needed only for cloud destinations)",
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
        "mediainfo" => get_mediainfo_url(os, arch),
        "rclone" => get_rclone_url(os, arch).await,
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
fn get_mp4decrypt_url(_os: &str, _arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // mp4decrypt (Bento4) is distributed exclusively via the MeedyaDL-Tools mirror.
    // The upstream source (bok.net) uses hardcoded version-specific URLs with no
    // "latest" tag or GitHub Releases API, so the URL would go stale on updates.
    // Returning Err here causes download_tool_with_fallback() to fall through
    // to the mirror, which hosts the extracted mp4decrypt binary directly.
    Err("mp4decrypt is installed from the MeedyaDL-Tools mirror".to_string())
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

/// Returns the MediaInfo CLI download URL for the given platform.
///
/// MediaInfo is available as a self-contained CLI binary:
/// - macOS: Universal binary (extracted from .pkg in DMG) via mirror
/// - Windows: ZIP with MediaInfo.exe + LIBCURL.DLL via mirror
/// - Linux: .deb packages via apt-get or mirror
///
/// All platforms fall through to the mirror fallback in `download_tool_with_fallback()`.
fn get_mediainfo_url(_os: &str, _arch: &str) -> Result<(String, archive::ArchiveFormat), String> {
    // MediaInfo is distributed exclusively via the MeedyaDL-Tools mirror.
    // The upstream .dmg (macOS) format isn't extractable by our archive module.
    // Returning Err here causes download_tool_with_fallback() to fall through
    // to the mirror, which hosts repackaged CLI binaries as tar.gz/zip.
    Err("MediaInfo is installed from the MeedyaDL-Tools mirror".to_string())
}

/// Returns the rclone download URL for the given platform.
///
/// rclone is the cloud-storage transport layer that powers MeedyaDL's
/// direct-to-cloud upload feature (M11, issue #859). Optional dependency
/// — only installed when the user enables Cloud Destinations. Source:
/// official rclone GitHub releases. Mirror fallback: MeedyaSuite/MeedyaDL-Tools
/// (planned, tracked in MeedyaSuite/MeedyaDL-Tools#17).
///
/// Asset naming convention on upstream:
///   `rclone-vX.YY.Z-osx-arm64.zip`
///   `rclone-vX.YY.Z-osx-amd64.zip`
///   `rclone-vX.YY.Z-linux-amd64.zip`
///   `rclone-vX.YY.Z-linux-arm64.zip`
///   `rclone-vX.YY.Z-linux-arm.zip`      (ARMv7)
///   `rclone-vX.YY.Z-windows-amd64.zip`
///   `rclone-vX.YY.Z-windows-arm64.zip`
///
/// The version (`vX.YY.Z`) changes per release, so we use
/// `resolve_github_release_asset()` to find the platform-matching asset
/// against the `latest` tag dynamically.
async fn get_rclone_url(
    os: &str,
    arch: &str,
) -> Result<(String, archive::ArchiveFormat), String> {
    // rclone uses its own asset-name suffix scheme distinct from .NET RIDs.
    // Map (os, arch) → rclone's platform-arch suffix.
    let platform_arch = match (os, arch) {
        ("macos", "aarch64") => "osx-arm64",
        ("macos", "x86_64") => "osx-amd64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") => "linux-arm",
        ("windows", "x86_64") => "windows-amd64",
        ("windows", "aarch64") => "windows-arm64",
        _ => {
            return Err(format!(
                "No pre-built rclone binary available for {os}/{arch}"
            ));
        }
    };

    // Query upstream's GitHub Releases API for the matching asset.
    let (url, filename) =
        resolve_github_release_asset("rclone/rclone", "latest", platform_arch).await?;

    log::info!("Resolved rclone asset: {filename}");
    // All rclone releases ship as .zip across every platform.
    Ok((url, archive::ArchiveFormat::Zip))
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
        "mediainfo" => format!("mediainfo{exe_ext}"),
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

            // For ffmpeg, also copy ffprobe from the same system directory.
            // ffprobe is a companion binary used for codec detection and BPM analysis.
            if tool_id == "ffmpeg" {
                copy_companion_ffprobe_from_dir(
                    system_path.parent().unwrap_or(std::path::Path::new("")),
                    &tool_dir,
                );
            }

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
    // then falls back to the MeedyaSuite/MeedyaDL-Tools mirror repository.
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

    // Step 5b: For ffmpeg, also extract/download the companion ffprobe binary.
    // ffprobe is used for codec detection (metadata enrichment) and BPM tag reading.
    if tool_id == "ffmpeg" {
        install_companion_ffprobe(&tool_dir).await;
    }

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

    // Use a two-step process instead of sh -c to avoid shell injection.
    // Step 3a: Decompress with gunzip to a temp file.
    // Step 3b: Extract with cpio from the decompressed file.
    // See: https://github.com/MWBMPartners/MeedyaDL/issues/228
    let decompressed = temp_dir.join("Payload.cpio");
    let gunzip_output = tokio::process::Command::new("gunzip")
        .args(["-c"])
        .arg(&payload_path)
        .output()
        .await
        .map_err(|e| format!("Failed to decompress GPAC payload: {e}"))?;

    if !gunzip_output.status.success() {
        return Err("gunzip failed to decompress GPAC payload".to_string());
    }
    tokio::fs::write(&decompressed, &gunzip_output.stdout)
        .await
        .map_err(|e| format!("Failed to write decompressed payload: {e}"))?;

    let extract_status = tokio::process::Command::new("cpio")
        .args(["-id", "--quiet"])
        .arg("-F")
        .arg(&decompressed)
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
    let arch = std::env::consts::ARCH;
    log::info!("Installing MP4Box on Linux ({arch})");

    // GPAC only publishes x86_64 .deb packages. On ARM (aarch64, arm),
    // install via the system package manager instead since GPAC is available
    // in Debian/Raspberry Pi OS/Ubuntu ARM repositories.
    if arch != "x86_64" {
        log::info!("ARM architecture detected ({arch}), trying apt install gpac");
        return install_mp4box_via_apt(app).await;
    }

    // Create temp directory for x86_64 .deb extraction
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

/// Installs `MP4Box` on ARM Linux via `apt` (the system package manager).
///
/// GPAC doesn't publish ARM `.deb` packages on their nightly build server,
/// but `gpac` is available in the Debian/Ubuntu/Raspberry Pi OS ARM
/// repositories. This function runs `sudo apt-get install -y gpac`, then
/// copies the installed `MP4Box` binary to `MeedyaDL`'s managed tool directory.
///
/// Requires `sudo` privileges. If `apt-get` is not available or the install
/// fails, returns an error so the caller can fall back to the mirror.
async fn install_mp4box_via_apt(app: &AppHandle) -> Result<String, String> {
    // Verify apt-get is available
    let which_apt = tokio::process::Command::new("which")
        .arg("apt-get")
        .output()
        .await
        .map_err(|e| format!("Failed to check for apt-get: {e}"))?;

    if !which_apt.status.success() {
        return Err("apt-get not found. Cannot install GPAC on this system. \
             Install manually: sudo apt install gpac"
            .to_string());
    }

    // Run sudo apt-get install -y gpac
    log::info!("Running: sudo apt-get install -y gpac");
    let apt_output = tokio::process::Command::new("sudo")
        .args(["apt-get", "install", "-y", "gpac"])
        .output()
        .await
        .map_err(|e| format!("Failed to run 'sudo apt-get install -y gpac': {e}"))?;

    if !apt_output.status.success() {
        let stderr = String::from_utf8_lossy(&apt_output.stderr);
        return Err(format!(
            "'sudo apt-get install -y gpac' failed: {}",
            stderr.trim()
        ));
    }

    log::info!("'apt-get install gpac' completed successfully");

    // Find the installed MP4Box binary via `which`
    let which_output = tokio::process::Command::new("which")
        .arg("MP4Box")
        .output()
        .await
        .map_err(|e| format!("Failed to locate MP4Box after apt install: {e}"))?;

    let mp4box_path = if which_output.status.success() {
        String::from_utf8_lossy(&which_output.stdout)
            .trim()
            .to_string()
    } else {
        // Also check lowercase variant
        let which_lower = tokio::process::Command::new("which")
            .arg("mp4box")
            .output()
            .await
            .map_err(|e| format!("Failed to locate mp4box after apt install: {e}"))?;

        if which_lower.status.success() {
            String::from_utf8_lossy(&which_lower.stdout)
                .trim()
                .to_string()
        } else {
            return Err("MP4Box binary not found after apt install. \
                 Try: sudo apt install gpac && which MP4Box"
                .to_string());
        }
    };

    log::info!("Found system MP4Box at {mp4box_path}");

    // Copy to MeedyaDL's managed tool directory
    copy_and_verify_mp4box(
        app,
        std::path::Path::new(&mp4box_path),
        "apt (system package manager)",
    )
    .await
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

    // Find the data archive file (data.tar.xz, data.tar.gz, or data.tar.zst)
    // without using shell globbing. See: https://github.com/MWBMPartners/MeedyaDL/issues/228
    let data_archive = std::fs::read_dir(temp_dir)
        .map_err(|e| format!("Failed to read temp directory: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("data.tar."))
                .unwrap_or(false)
        })
        .ok_or_else(|| "No data.tar.* archive found in .deb package".to_string())?;

    let extract_status = tokio::process::Command::new("tar")
        .arg("xf")
        .arg(&data_archive)
        .arg("-C")
        .arg(&data_dir)
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
///   - Linux x86_64: .deb extraction from GPAC nightly builds
///   - Linux ARM (aarch64/armv7): `apt-get install gpac`
///
/// If the platform-specific method fails, falls back to the
/// MeedyaSuite/MeedyaDL-Tools mirror repository for a generic binary archive.
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
fn find_binary_recursive(dir: &std::path::Path, tool_id: &str) -> Option<PathBuf> {
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
        // MediaInfo: check both expected case and uppercase variant
        "mediainfo" => vec![
            format!("mediainfo{}", exe_ext),
            format!("MediaInfo{}", exe_ext),
        ],
        _ => vec![format!("{}{}", tool_id, exe_ext)],
    };

    // Walk the directory tree depth-first looking for any matching binary.
    // This handles archives with arbitrary nesting (e.g., Bento4 SDK has
    // bin/ subdir). Migrated to walk_dir_find_first in v1.0.9 (#716/1)
    // which adds a max_depth bound — previously unbounded. depth=5 covers
    // every archive shape we've encountered (deepest known is BtbN's
    // ffmpeg-master-latest-linux64-gpl/bin/ffmpeg at depth 2) with
    // generous headroom; if a future archive nests deeper, raise this.
    crate::utils::fs_walk::walk_dir_find_first(dir, 5, |path| {
        if !path.is_file() {
            return None;
        }
        let name = path.file_name().and_then(|n| n.to_str())?;
        if search_names.iter().any(|s| s == name) {
            Some(path.to_path_buf())
        } else {
            None
        }
    })
}

/// Installs all companion FFmpeg binaries (ffprobe, ffplay) alongside ffmpeg.
///
/// The BtbN archives (Linux/Windows) bundle all three tools. On macOS,
/// evermeet.cx provides each tool as a separate download. This function:
/// 1. Searches the extracted archive for each companion (covers BtbN)
/// 2. If not found, downloads each separately (covers macOS evermeet.cx)
async fn install_companion_ffprobe(tool_dir: &std::path::Path) {
    // All FFmpeg companion binaries to extract/download alongside ffmpeg.
    // evermeet.cx URLs follow the pattern: https://evermeet.cx/{tool}/getrelease/zip
    let companions: &[(&str, &str)] = &[
        ("ffprobe", "https://evermeet.cx/ffprobe/getrelease/zip"),
        ("ffplay", "https://evermeet.cx/ffplay/getrelease/zip"),
    ];

    for &(base_name, macos_url) in companions {
        let binary_name = if cfg!(target_os = "windows") {
            format!("{base_name}.exe")
        } else {
            base_name.to_string()
        };
        let dest = tool_dir.join(&binary_name);

        // Already present (e.g., extracted at top level) — skip
        if dest.exists() {
            log::debug!("{base_name} already present at {}", dest.display());
            continue;
        }

        // Search the extracted archive tree (BtbN archives nest in bin/)
        if let Some(found) = find_file_recursive(tool_dir, &binary_name) {
            match std::fs::copy(&found, &dest) {
                Ok(_) => {
                    archive::set_executable(&dest).ok();
                    log::info!("Copied companion {base_name} from {} to {}", found.display(), dest.display());
                    continue;
                }
                Err(e) => log::warn!("Failed to copy {base_name} from archive: {e}"),
            }
        }

        // Not in the archive — download separately (macOS evermeet.cx case)
        if cfg!(target_os = "macos") {
            log::info!("{base_name} not in ffmpeg archive — downloading separately from evermeet.cx");
            match archive::download_and_extract(
                macos_url,
                tool_dir,
                archive::ArchiveFormat::Zip,
            )
            .await
            {
                Ok(()) => {
                    if dest.exists() {
                        archive::set_executable(&dest).ok();
                        log::info!("Downloaded companion {base_name} to {}", dest.display());
                    } else {
                        log::warn!("{base_name} download succeeded but binary not found at expected location");
                    }
                }
                Err(e) => {
                    log::warn!("Failed to download companion {base_name}: {e}");
                }
            }
        } else {
            log::warn!("{base_name} not found in ffmpeg archive");
        }
    }
}

/// Copies all companion FFmpeg binaries from a directory (used when copying system ffmpeg).
///
/// When the user has ffmpeg on their system PATH, ffprobe and ffplay typically
/// live in the same directory. This copies them to the managed tool directory.
fn copy_companion_ffprobe_from_dir(source_dir: &std::path::Path, tool_dir: &std::path::Path) {
    let companions: &[&str] = &["ffprobe", "ffplay"];

    for &base_name in companions {
        let binary_name = if cfg!(target_os = "windows") {
            format!("{base_name}.exe")
        } else {
            base_name.to_string()
        };
        let src = source_dir.join(&binary_name);
        let dest = tool_dir.join(&binary_name);

        if src.exists() {
            match std::fs::copy(&src, &dest) {
                Ok(_) => {
                    archive::set_executable(&dest).ok();
                    log::info!("Copied system {base_name} from {}", src.display());
                }
                Err(e) => log::warn!("Failed to copy system {base_name}: {e}"),
            }
        } else {
            log::debug!("System {base_name} not found at {}", src.display());
        }
    }
}

/// Searches a directory tree for a file by exact name (case-sensitive).
///
/// Unlike `find_binary_recursive`, this searches for any filename rather than
/// tool-specific names. Used to find companion binaries like ffprobe.
///
/// Migrated to `walk_dir_find_first` in v1.0.9 (#716/1) — previously
/// unbounded recursion. depth=5 matches `find_binary_recursive`; same
/// archive-shape rationale applies.
fn find_file_recursive(dir: &std::path::Path, filename: &str) -> Option<PathBuf> {
    crate::utils::fs_walk::walk_dir_find_first(dir, 5, |path| {
        if !path.is_file() {
            return None;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some(filename) {
            Some(path.to_path_buf())
        } else {
            None
        }
    })
}

/// Attempts to get the version of an installed tool binary.
///
/// Runs the binary with common version flags (--version, -version) and
/// parses the first line of output.
///
/// # Arguments
/// * `binary_path` - Path to the tool binary
/// * `tool_id` - The tool identifier (for tool-specific parsing)
pub async fn get_tool_version(binary_path: &PathBuf, tool_id: &str) -> Result<String, String> {
    // Different tools use different version flags:
    // - FFmpeg and MP4Box use single-dash "-version" (non-standard but that's how they work)
    // - mp4decrypt has no version flag — running it with no args prints usage to stderr
    // - Most other tools use double-dash "--version" (GNU convention)
    let version_flag = match tool_id {
        "ffmpeg" | "mp4box" => "-version",
        "mp4decrypt" => "", // No version flag — run with no args, parse stderr
        _ => "--version",
    };

    // Run the binary with the version flag and capture output.
    let output = if version_flag.is_empty() {
        // mp4decrypt: run with no arguments, version info is in the error output
        tokio::process::Command::new(binary_path)
            .output()
            .await
            .map_err(|e| format!("Failed to run {tool_id}: {e}"))?
    } else {
        tokio::process::Command::new(binary_path)
            .arg(version_flag)
            .output()
            .await
            .map_err(|e| format!("Failed to run {tool_id} {version_flag}: {e}"))?
    };

    // Combine stdout and stderr (some tools output to stderr)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };

    // Use the structured version parser which handles each tool's quirks
    // (mp4decrypt error output, MediaInfo multi-line, FFmpeg prefixes, etc.)
    if let Some(version) = extract_version_from_output(&combined, tool_id) {
        return Ok(version);
    }

    // Fallback: return the first non-empty line as-is
    let first_line = combined
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Ok(first_line)
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
