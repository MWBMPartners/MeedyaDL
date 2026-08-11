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

use std::path::{Path, PathBuf};
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

/// Loads a pinned SHA-256 hash for a specific mirror asset filename, from
/// the compiled `tool-versions.toml`'s optional `[mirror.asset_hashes]`
/// table (#987).
///
/// Returns `None` when the table is absent, malformed, or simply doesn't
/// list this asset — every one of those cases means "unverified",
/// preserving pre-#987 behaviour (mirror downloads proceed without
/// checksum verification unless explicitly pinned).
fn load_mirror_asset_hash(asset_filename: &str) -> Option<String> {
    let hash = parse_mirror_asset_hash(TOOL_VERSIONS_TOML, asset_filename);
    if let Some(ref h) = hash {
        log::info!("Found pinned SHA-256 for mirror asset '{asset_filename}': {h}");
    }
    hash
}

/// Pure parsing core for [`load_mirror_asset_hash()`], factored out so it
/// can be unit tested against inline TOML fixtures without depending on
/// the compiled `tool-versions.toml` file.
///
/// Filename matching is case-insensitive (mirror asset names are produced
/// by CI tooling and casing can vary by platform/runner).
fn parse_mirror_asset_hash(toml_src: &str, asset_filename: &str) -> Option<String> {
    let config: toml::Value = toml::from_str(toml_src).ok()?;
    let hashes_table = config.get("mirror")?.get("asset_hashes")?.as_table()?;
    let target_lower = asset_filename.to_lowercase();
    hashes_table.iter().find_map(|(key, value)| {
        if key.to_lowercase() == target_lower {
            value.as_str().map(str::to_string)
        } else {
            None
        }
    })
}

/// A pinned Windows GPAC NSIS installer URL + expected SHA-256, parsed
/// from the compiled `tool-versions.toml`'s optional
/// `[gpac.windows_installer]` table (#987).
struct GpacWindowsInstallerPin {
    url: String,
    sha256: String,
}

/// Loads the pinned Windows GPAC NSIS installer configuration, if present.
///
/// Returns `None` when the `[gpac.windows_installer]` section is absent
/// (the default) or malformed — in both cases `install_mp4box_windows()`
/// skips straight to the mirror fallback rather than executing an
/// unverifiable download.
fn load_gpac_windows_installer_pin() -> Option<GpacWindowsInstallerPin> {
    parse_gpac_windows_installer_pin(TOOL_VERSIONS_TOML)
}

/// Pure parsing core for [`load_gpac_windows_installer_pin()`], factored
/// out for unit testing against inline TOML fixtures.
///
/// Requires both `url` to be non-empty AND `sha256` to look like a
/// well-formed SHA-256 hex digest (exactly 64 hex characters) — anything
/// short of that is treated as "not configured" (with a `log::warn!` so a
/// typo'd pin doesn't fail silently) rather than passed through to the
/// downloader, since a malformed hash could never successfully verify
/// anyway.
fn parse_gpac_windows_installer_pin(toml_src: &str) -> Option<GpacWindowsInstallerPin> {
    let config: toml::Value = toml::from_str(toml_src).ok()?;
    let table = config.get("gpac")?.get("windows_installer")?;

    let url = table.get("url")?.as_str()?.to_string();
    let sha256 = table.get("sha256")?.as_str()?.to_lowercase();

    if url.is_empty() {
        log::warn!("[gpac.windows_installer] is present but 'url' is empty — ignoring pin");
        return None;
    }
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        log::warn!(
            "[gpac.windows_installer] 'sha256' is not a well-formed 64-character hex digest — ignoring pin"
        );
        return None;
    }

    Some(GpacWindowsInstallerPin { url, sha256 })
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

/// The ordered absolute directories to probe for a system package-manager
/// (Homebrew, MacPorts, apt/dnf, snap, Linuxbrew) install, on top of the
/// inherited PATH. Needed because a Finder-launched macOS `.app` inherits
/// launchd's minimal `/usr/bin:/bin:/usr/sbin:/sbin` (no `/opt/homebrew/bin`),
/// so a bare `which` misses Homebrew tools. Windows returns empty — Choco/Scoop
/// shims live on PATH and are covered by `where`.
pub(crate) fn system_tool_search_dirs() -> Vec<PathBuf> {
    let base: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/opt/homebrew/bin", // Homebrew (Apple Silicon)
            "/usr/local/bin",    // Homebrew (Intel) / manual
            "/opt/local/bin",    // MacPorts
            "/usr/bin",
            "/bin",
            "/usr/sbin",
        ]
    } else if cfg!(target_os = "linux") {
        &[
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/snap/bin",
            "/home/linuxbrew/.linuxbrew/bin", // Linuxbrew (multi-user)
        ]
    } else {
        &[]
    };
    // `mut` is only exercised by the Linux-only per-user Linuxbrew push below;
    // on macOS/Windows that cfg block is compiled out, so allow unused_mut there
    // (keep the lint active on Linux, where the mutation is real).
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut dirs: Vec<PathBuf> = base.iter().map(PathBuf::from).collect();
    // Per-user Linuxbrew (~/.linuxbrew/bin), Linux only. Reading our own HOME
    // (not a subprocess env) preserves the zero-`Command::env` invariant.
    #[cfg(target_os = "linux")]
    if let Ok(home) = std::env::var("HOME") {
        let per_user = PathBuf::from(home).join(".linuxbrew/bin");
        if per_user.is_absolute() {
            dirs.push(per_user);
        }
    }
    dirs
}

/// Defence-in-depth: on Unix, rejects a candidate whose resolved target (or its
/// containing directory) is world-writable — so detection never adopts a binary
/// an unprivileged process could have planted. Homebrew/system dirs are not
/// world-writable, so legitimate installs pass. Always `true` on Windows.
#[cfg(unix)]
pub(crate) fn is_trusted_binary(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let world_writable =
        |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o002 != 0);
    !world_writable(&real) && real.parent().is_none_or(|parent| !world_writable(parent))
}

#[cfg(not(unix))]
pub(crate) fn is_trusted_binary(_path: &Path) -> bool {
    true
}

/// Searches for a tool in the system PATH and returns its path and version
/// if found and compatible with the minimum version requirement.
///
/// Uses `which` (Unix) or `where` (Windows) to locate the binary in PATH, then
/// falls back to probing [`system_tool_search_dirs`] directly (Homebrew,
/// MacPorts, Linuxbrew, snap, base dirs) so a Finder-launched macOS app with a
/// minimal PATH still finds `brew install`ed tools. All candidates are absolute,
/// existing, and [`is_trusted_binary`].
///
/// # Returns
/// * `Some((path, version))` if the tool is found and version could be detected
/// * `None` if the tool is not found or version detection fails
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
        // Parse the first line of output as the binary path.
        let path_str = String::from_utf8_lossy(&output.stdout)
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let p = PathBuf::from(&path_str);
        // Require an absolute, existing, trusted path (reject a relative or
        // world-writable `which`/`where` result).
        if p.is_absolute() && p.exists() && is_trusted_binary(&p) {
            Some(p)
        } else {
            None
        }
    } else {
        None
    };

    // If not on the inherited PATH — minimal for a Finder-launched macOS app,
    // so `which` misses Homebrew — probe the known package-manager install dirs
    // directly (Homebrew, MacPorts, Linuxbrew, snap, base dirs).
    let path = path.or_else(|| {
        system_tool_search_dirs().into_iter().find_map(|dir| {
            let candidate = dir.join(&config.binary_name);
            (candidate.exists() && is_trusted_binary(&candidate)).then_some(candidate)
        })
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

/// Detects an existing compatible system/package-manager install of `tool_id`
/// and adopts it IN PLACE — writing the `.external-path` reference pointer + the
/// `.source` marker (no copy), exactly like `install_tool`'s system tier — so
/// both the wizard status and every runtime consumer (which resolve via
/// `get_tool_binary_path`) see it WITHOUT the user having to click Install.
///
/// This closes the #1081 gap where detection only ran on an explicit install.
/// It adopts the version as found and deliberately does NOT trigger a
/// `brew upgrade` (that stays an explicit, install-time action). Returns
/// `(path, source_label)` on success; `None` if no compatible, trusted system
/// binary exists (in which case the caller falls back to the download pipeline).
pub async fn adopt_system_tool_if_available(
    app: &AppHandle,
    tool_id: &str,
) -> Option<(PathBuf, String)> {
    let (system_path, system_version) = find_system_tool(tool_id).await?;

    // Defence-in-depth: never adopt a world-writable binary.
    if !is_trusted_binary(&system_path) {
        log::warn!(
            "Skipping world-writable system {tool_id} at {}",
            system_path.display()
        );
        return None;
    }

    // Minimum-version gate (mirrors install_tool's system tier).
    let config = load_tool_version_config(tool_id);
    let is_compatible = config
        .as_ref()
        .is_none_or(|c| meets_minimum_version(&system_version, &c.minimum_version));
    if !is_compatible {
        return None;
    }

    // Provenance: attribute to the owning package manager when possible
    // (Homebrew formula, pipx venv, dpkg/rpm package, …), else a generic
    // "system" marker. Status-time adoption is detection only and never
    // triggers a package-manager upgrade.
    let source = crate::services::package_manager::detect_owner(&system_path)
        .await
        .map(|r| r.to_marker())
        .unwrap_or_else(|| "system".to_string());

    // Adopt in place: reference pointer + source marker, no copy.
    let tool_dir = get_tool_dir(app, tool_id);
    std::fs::create_dir_all(&tool_dir).ok()?;
    std::fs::write(
        tool_dir.join(".external-path"),
        system_path.to_string_lossy().as_bytes(),
    )
    .ok()?;
    std::fs::write(tool_dir.join(".source"), &source).ok();

    log::info!(
        "Adopted system {tool_id} in place from {} ({source})",
        system_path.display()
    );
    Some((system_path, source))
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
        .header("User-Agent", crate::utils::http_client::APP_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed for {repo}: {e}"))?;

    let release: serde_json::Value = if response.status().as_u16() == 404 && tag == "latest" {
        // Tag "latest" doesn't exist as a git tag — fall back to /releases/latest
        log::debug!("No 'latest' tag in {repo}, falling back to /releases/latest");
        let fallback_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let fallback_resp = client
            .get(&fallback_url)
            .header("User-Agent", crate::utils::http_client::APP_USER_AGENT)
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
) -> Result<(String, archive::ArchiveFormat, Option<String>), String> {
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
    // Uses the honest `detect_archive_format_from_url()` (#981) instead of
    // the old "anything non-.zip is .tar.gz" guess, which silently
    // mislabeled `.tar.xz` assets (e.g. BtbN's Linux FFmpeg build) and fed
    // XZ bytes into the gzip decoder.
    let format = archive::detect_archive_format_from_url(&filename).unwrap_or_else(|| {
        log::warn!(
            "Unrecognised archive extension on mirror asset '{filename}' — assuming tar.gz"
        );
        archive::ArchiveFormat::TarGz
    });

    // Optional per-asset SHA-256 pin (#987). `None` when unconfigured —
    // the download proceeds unverified, matching pre-#987 behaviour.
    let expected_sha256 = load_mirror_asset_hash(&filename);

    log::info!("Mirror resolved: {asset_prefix} → {url}");
    Ok((url, format, expected_sha256))
}

/// Downloads a tool's archive and extracts it to the tool directory,
/// with automatic fallback to the mirror repository if the primary
/// upstream source fails.
///
/// Resolution order:
///   1. Primary upstream source (hardcoded URL or upstream GitHub API)
///   2. MeedyaSuite/MeedyaDL-Tools mirror repository (fallback)
///
/// ## Stage-and-swap (#996)
///
/// Downloads extract into a *sibling staging directory*
/// (`{tool_dir}.staging`), never directly into `tool_dir`. The real
/// `tool_dir` is only touched — via [`promote_staged_install`] — once a
/// download has fully succeeded. Previously this function deleted
/// `tool_dir` *before* attempting any download, so a reinstall that failed
/// on both primary and mirror (e.g. the #981 Linux x86_64 tar.xz bug,
/// where primary can never extract) destroyed the user's previously
/// working installation, leaving GAMDL unable to run at all. Under
/// stage-and-swap, a total failure leaves `tool_dir` byte-for-byte
/// unchanged.
///
/// # Arguments
/// * `tool_id` - The tool identifier (e.g., "ffmpeg")
/// * `tool_dir` - The target installation directory
async fn download_tool_with_fallback(
    tool_id: &str,
    tool_dir: &std::path::Path,
) -> Result<(), String> {
    // Sibling staging directory — never the real tool_dir. Named
    // `{tool_dir}.staging` so it lives alongside (not inside) the real
    // install and can't collide with a legitimate tool subdirectory.
    let staging = tool_dir.with_file_name(format!(
        "{}.staging",
        tool_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tool")
    ));
    // Clear out any leftover staging dir from a prior crashed/interrupted
    // attempt, then create a fresh one.
    std::fs::remove_dir_all(&staging).ok();
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("Failed to create staging directory: {e}"))?;

    // Try primary upstream source, extracting into staging (not tool_dir).
    let primary_error = match get_tool_download_url(tool_id).await {
        Ok((url, format)) => {
            log::info!("Downloading {tool_id} from primary source: {url}");
            match archive::download_and_extract(&url, &staging, format).await {
                Ok(()) => return promote_staged_install(&staging, tool_dir),
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

    // Primary failed — reset staging (tool_dir is untouched) and try mirror.
    std::fs::remove_dir_all(&staging).ok();
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("Failed to recreate staging directory: {e}"))?;

    log::info!("Trying mirror fallback for {tool_id}...");
    match get_mirror_download_url(tool_id).await {
        Ok((mirror_url, mirror_format, mirror_sha256)) => {
            log::info!("Downloading {tool_id} from mirror: {mirror_url}");
            match archive::download_and_extract_verified(
                &mirror_url,
                &staging,
                mirror_format,
                mirror_sha256.as_deref(),
            )
            .await
            {
                Ok(()) => promote_staged_install(&staging, tool_dir),
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&staging);
                    Err(format!(
                        "All download sources failed for {tool_id}.\n  Primary: {primary_error}\n  Mirror: {e}"
                    ))
                }
            }
        }
        Err(mirror_err) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(format!(
                "All download sources failed for {tool_id}.\n  Primary: {primary_error}\n  Mirror: {mirror_err}"
            ))
        }
    }
}

/// Atomically-ish swaps a freshly-staged install into place (#996).
///
/// Two-step so it works on Windows, where renaming onto an existing
/// directory fails: move the old `tool_dir` aside to `{tool_dir}.old`,
/// move `staging` into `tool_dir`'s place, then delete the old dir. If the
/// final rename fails (e.g. cross-device on some exotic setup), the old
/// dir is best-effort restored so the user isn't left with neither.
///
/// # Arguments
/// * `staging` - The sibling staging directory containing the fresh install
/// * `tool_dir` - The real installation directory to replace
///
/// # Errors
///
/// Returns `Err(String)` if the old install can't be moved aside or the
/// staged install can't be promoted. In both cases the function tries to
/// leave the filesystem in a recoverable state (old install restored when
/// possible) rather than a half-swapped one.
fn promote_staged_install(
    staging: &std::path::Path,
    tool_dir: &std::path::Path,
) -> Result<(), String> {
    let backup = tool_dir.with_file_name(format!(
        "{}.old",
        tool_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tool")
    ));
    if backup.exists() {
        std::fs::remove_dir_all(&backup).ok();
    }
    if tool_dir.exists() {
        std::fs::rename(tool_dir, &backup)
            .or_else(|_| std::fs::remove_dir_all(tool_dir))
            .map_err(|e| format!("Failed to move aside existing install {}: {e}", tool_dir.display()))?;
    }
    if let Some(parent) = tool_dir.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::rename(staging, tool_dir).map_err(|e| {
        if backup.exists() {
            let _ = std::fs::rename(&backup, tool_dir);
        }
        format!("Failed to promote staged install {}: {e}", tool_dir.display())
    })?;
    let _ = std::fs::remove_dir_all(&backup);
    Ok(())
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
            // BtbN only publishes this asset as `.tar.xz` -- no `.tar.gz`
            // variant exists. The pre-#981 mislabel (`ArchiveFormat::TarGz`
            // here) fed a real XZ stream into `flate2::GzDecoder`, which
            // cannot parse it, causing 100% primary-extract failure on
            // Linux x86_64 (silently falling through to the mirror on
            // every install). `ArchiveFormat::TarXz` routes this through
            // `extract_tar_xz()` (lzma-rs), matching the actual bytes.
            archive::ArchiveFormat::TarXz,
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
    // Uses the honest `detect_archive_format_from_url()` (#981) instead of
    // the old "anything non-.zip is .tar.gz" guess, which silently
    // mislabeled `.tar.xz` assets and fed XZ bytes into the gzip decoder.
    let format = archive::detect_archive_format_from_url(&filename).unwrap_or_else(|| {
        log::warn!(
            "Unrecognised archive extension on N_m3u8DL-RE asset '{filename}' — assuming tar.gz"
        );
        archive::ArchiveFormat::TarGz
    });

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

fn read_external_tool_path(tool_dir: &Path) -> Option<PathBuf> {
    let path = std::fs::read_to_string(tool_dir.join(".external-path")).ok()?;
    let path = PathBuf::from(path.trim());
    (path.is_absolute() && path.exists()).then_some(path)
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
    // System tools are referenced directly rather than copied. This small
    // pointer file keeps the existing managed-tool API intact without creating
    // a duplicate binary or relying on platform-specific link privileges.
    if let Some(path) = read_external_tool_path(&tool_dir) {
        return path;
    }
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

    // Step 0: Check if a compatible version already exists on the system. A
    // package manager is never required: when no suitable binary is present we
    // continue into the original managed download/install pipeline below.
    if let Some((mut system_path, mut system_version)) = find_system_tool(tool_id).await {
        let tool_dir = get_tool_dir(app, tool_id);
        let previous_source = std::fs::read_to_string(tool_dir.join(".source")).unwrap_or_default();

        // Attribute the system binary to its owning package manager (Homebrew
        // formula, pipx venv, dpkg/rpm package, snap, …).
        let previous_ref =
            crate::services::package_manager::PackageRef::parse_marker(&previous_source);
        let mut owner = crate::services::package_manager::detect_owner(&system_path).await;

        // Only delegate an update when MeedyaDL was already referencing this
        // exact package via this exact manager. Initial setup adopts the
        // compatible version exactly as found and does not unexpectedly mutate
        // the user's system. No-elevation managers (Homebrew/pipx/Scoop) run
        // directly; root-requiring ones (apt/dnf/snap/MacPorts) run through the
        // #997 non-interactive elevation tiers. A failed or un-elevatable
        // update is non-fatal: we adopt the already-compatible version as-found
        // and surface the actionable command in the Activity Log, rather than
        // blocking the install.
        if let (Some(prev), Some(cur)) = (previous_ref.as_ref(), owner.as_ref()) {
            if prev == cur {
                match cur.pm.upgrade(cur).await {
                    Ok(()) => {
                        if let Some((path, version)) = find_system_tool(tool_id).await {
                            system_path = path;
                            system_version = version;
                            owner =
                                crate::services::package_manager::detect_owner(&system_path).await;
                        }
                    }
                    Err(e) => {
                        log::warn!("Package-manager update of {tool_id} did not run: {e}");
                        crate::utils::activity_log::emit_app_log(
                            app,
                            &format!("Could not auto-update {tool_id}: {e}"),
                        );
                    }
                }
            }
        }
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

            // Store a reference, not a copy: every runtime call resolves the
            // original system binary through get_tool_binary_path().
            if tool_dir.exists() {
                std::fs::remove_dir_all(&tool_dir).ok();
            }
            std::fs::create_dir_all(&tool_dir)
                .map_err(|e| format!("Failed to create tool directory: {e}"))?;

            std::fs::write(
                tool_dir.join(".external-path"),
                system_path.to_string_lossy().as_bytes(),
            )
            .map_err(|e| format!("Failed to save system tool path: {e}"))?;

            // Write a .source marker file so check_all_dependencies knows this
            // tool came from a package manager / system PATH rather than being
            // downloaded (`<pm>:<pkg>` when attributed, else generic "system").
            let source_marker = tool_dir.join(".source");
            let source = owner
                .as_ref()
                .map(crate::services::package_manager::PackageRef::to_marker)
                .unwrap_or_else(|| "system".to_string());
            std::fs::write(&source_marker, source).ok();

            log::info!(
                "Using system {} directly from {}",
                tool_id,
                system_path.display()
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

/// Installs `MP4Box` on Windows by downloading and silently running a
/// **pinned** GPAC NSIS installer.
///
/// GPAC discontinued ZIP archives for Windows — only NSIS `.exe` installers remain.
/// This function downloads the installer, runs it with `/S` (silent) and `/D=` (custom
/// install directory) to extract files without user interaction, then copies MP4Box.exe
/// to `MeedyaDL`'s tool directory.
///
/// The NSIS `/D=` flag installs to a user-specified directory without requiring admin
/// privileges (as long as the directory is user-writable).
///
/// ## Pinned installer + SHA-256 verification (#987)
///
/// Earlier versions of this function downloaded and executed GPAC's
/// **nightly** Windows build (`gpac_latest_head_win64.exe`) with no
/// checksum check — nightlies have no stable published hash to verify
/// against, since the artifact changes on every upstream CI run, so an
/// executed installer could never actually be confirmed to be the binary
/// GPAC built. The download URL + expected SHA-256 now come from the
/// optional `[gpac.windows_installer]` table in `tool-versions.toml`
/// (see [`load_gpac_windows_installer_pin()`]), which is expected to
/// point at a specific, stable GPAC release rather than the nightly
/// permalink. When that section is absent (the shipped default), this
/// function returns an error immediately and [`install_mp4box_with_fallback`]
/// falls through to the MeedyaSuite/MeedyaDL-Tools mirror instead — the
/// unverifiable nightly is never executed.
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
    // Load the pinned, checksum-verifiable installer (#987). This function
    // used to download and SILENTLY EXECUTE GPAC's Windows *nightly* build
    // (`gpac_latest_head_win64.exe`) with no checksum verification at
    // all — nightlies have no published, stable checksum to verify
    // against (the binary changes on every upstream CI run), so there was
    // never a way to confirm the downloaded installer was the one GPAC
    // actually built before running it with elevated file-write access.
    // MeedyaDL no longer executes an unverifiable installer: absent a pin,
    // bail out immediately so the caller (`install_mp4box_with_fallback`)
    // falls through to the MeedyaSuite/MeedyaDL-Tools mirror instead,
    // which hosts a vetted, checksummed binary archive.
    let Some(pin) = load_gpac_windows_installer_pin() else {
        return Err(
            "GPAC NSIS installer skipped: no pinned installer configured \
             ([gpac.windows_installer]) and MeedyaDL no longer executes the \
             unverifiable nightly (#987). Falling back to the mirror."
                .to_string(),
        );
    };

    let installer_path = temp_dir.join("gpac_installer.exe");

    log::info!("Downloading pinned GPAC installer from {}", pin.url);
    let (_bytes, actual_sha256) = archive::download_file(&pin.url, &installer_path).await?;

    if actual_sha256 != pin.sha256 {
        let _ = std::fs::remove_file(&installer_path);
        return Err(format!(
            "SHA-256 checksum mismatch for pinned GPAC installer {}\n  Expected: {}\n  Actual:   {}",
            pin.url, pin.sha256, actual_sha256
        ));
    }
    log::info!("SHA-256 checksum verified for pinned GPAC installer");

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
    // NSIS requires the `/D=` install-directory flag to be the LAST
    // argument on the command line, and it must be UNQUOTED (NSIS parses
    // it with its own rules, not standard argv quoting). `std::process`
    // (and therefore `tokio::process`) automatically wraps any argument
    // containing whitespace in double quotes, which breaks installs to a
    // path with spaces -- e.g. `C:\Users\John Smith\AppData\...` (#982).
    // `raw_arg()` (Windows-only) appends the text to the command line
    // completely verbatim, bypassing that quoting. On non-Windows targets
    // this function is unreachable at runtime (`install_mp4box_windows`
    // is only called when `std::env::consts::OS == "windows"`), so the
    // `.arg()` fallback below exists purely to keep the file
    // cross-compiling everywhere MeedyaDL builds.
    let mut installer_cmd = tokio::process::Command::new(&installer_path);
    installer_cmd.arg("/S");
    #[cfg(windows)]
    installer_cmd.raw_arg(format!("/D={}", install_dir.display()));
    #[cfg(not(windows))]
    installer_cmd.arg(format!("/D={}", install_dir.display()));
    let install_status = installer_cmd
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

/// Probes whether `sudo` can run non-interactively (cached credentials /
/// passwordless NOPASSWD rule) without ever prompting.
///
/// `sudo -n` ("non-interactive") fails immediately instead of blocking on
/// a password prompt if one would be required. This is the load-bearing
/// check for #997: MeedyaDL's dependency installer can run with no
/// controlling TTY (e.g. launched from a desktop icon, or headlessly over
/// SSH without a pty), where a bare `sudo apt-get install` would hang
/// forever (or fail with "a terminal is required to read the password")
/// instead of surfacing an actionable error.
///
/// # Returns
/// `true` if `sudo -n true` exits successfully (no password needed right
/// now); `false` otherwise, including if `sudo` itself is missing.
///
/// `pub(crate)` so `services::package_manager` reuses the same
/// non-interactive elevation probe for its elevated package upgrades.
pub(crate) async fn can_sudo_without_password() -> bool {
    tokio::process::Command::new("sudo")
        .args(["-n", "true"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Locates a `pkexec` binary on `PATH`, but only when a graphical session
/// is actually present.
///
/// `pkexec` (PolicyKit) pops a native GUI authentication dialog, which
/// requires a display server to render into. Without `DISPLAY` (X11) or
/// `WAYLAND_DISPLAY` (Wayland) set, invoking `pkexec` would either fail
/// outright or (worse) hang waiting for a dialog nobody can see — the
/// same class of silent-hang failure this whole elevation strategy exists
/// to avoid. Checking for a display server first means we only attempt
/// `pkexec` when it has a real chance of showing its prompt.
///
/// # Returns
/// `Some(path)` if a graphical session is present AND `which pkexec`
/// resolves to a non-empty path; `None` otherwise (headless session, or
/// `pkexec` not installed).
///
/// `pub(crate)` so `services::package_manager` reuses the same graphical
/// elevation tier for its elevated package upgrades.
pub(crate) async fn find_pkexec() -> Option<String> {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !has_display {
        return None;
    }

    let which_pkexec = tokio::process::Command::new("which")
        .arg("pkexec")
        .output()
        .await
        .ok()?;

    if !which_pkexec.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&which_pkexec.stdout)
        .trim()
        .to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Installs `MP4Box` on ARM Linux via `apt` (the system package manager).
///
/// GPAC doesn't publish ARM `.deb` packages on their nightly build server,
/// but `gpac` is available in the Debian/Ubuntu/Raspberry Pi OS ARM
/// repositories. This function elevates and runs `apt-get install -y
/// gpac`, then copies the installed `MP4Box` binary to `MeedyaDL`'s
/// managed tool directory.
///
/// ## 3-tier elevation strategy (#997)
///
/// A plain `sudo apt-get install -y gpac` blocks forever — or fails with
/// an unhelpful "a terminal is required to read the password" — when
/// MeedyaDL is launched without a controlling TTY (desktop icon, systemd
/// unit, or headless SSH session without a pty), which is common on
/// Raspberry Pi / ARM Linux setups that this codepath specifically
/// targets. To surface an actionable outcome instead of a silent hang:
///
///   1. **Passwordless `sudo`** — [`can_sudo_without_password()`] probes
///      via `sudo -n true`; if it succeeds (cached credentials, or a
///      NOPASSWD rule), run `sudo -n apt-get install -y gpac`. Still
///      non-interactive, so it can never block on a password prompt.
///   2. **`pkexec` (PolicyKit GUI prompt)** — only attempted when a
///      display server is present ([`find_pkexec()`]); shows a native
///      graphical authentication dialog instead of a terminal prompt,
///      appropriate for a desktop-launched GUI app.
///   3. **Neither available** — returns an actionable error telling the
///      user to run `sudo apt-get install gpac` in a terminal themselves
///      (or use the "Browse" button to point at an existing MP4Box).
///
/// If `apt-get` is not available or the install fails, returns an error
/// so the caller can fall back to the mirror.
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

    // Actionable message shared by every elevation failure path below —
    // whether we never had a way to elevate, or an elevation attempt
    // failed because no TTY/password/authorization was available.
    let actionable_elevation_error = || {
        "Could not install GPAC automatically: no non-interactive privilege \
         elevation is available on this system (no cached sudo credentials \
         and no graphical PolicyKit prompt). Please open a terminal and run \
         'sudo apt-get install gpac' manually, or use the \"Browse\" button \
         to point MeedyaDL at an existing MP4Box installation."
            .to_string()
    };

    // Tier 1: passwordless sudo (cached credentials or a NOPASSWD rule).
    // Tier 2: pkexec, only when a graphical session is present.
    // Tier 3: neither — bail out with actionable guidance rather than
    // attempting `sudo apt-get install` and risking an indefinite hang
    // on a password prompt nobody can answer.
    let apt_output = if can_sudo_without_password().await {
        log::info!("Running: sudo -n apt-get install -y gpac");
        tokio::process::Command::new("sudo")
            .args(["-n", "apt-get", "install", "-y", "gpac"])
            .output()
            .await
            .map_err(|e| format!("Failed to run 'sudo -n apt-get install -y gpac': {e}"))?
    } else if let Some(pkexec_path) = find_pkexec().await {
        log::info!("Running: pkexec apt-get install -y gpac (via {pkexec_path})");
        tokio::process::Command::new("pkexec")
            .args(["apt-get", "install", "-y", "gpac"])
            .output()
            .await
            .map_err(|e| format!("Failed to run 'pkexec apt-get install -y gpac': {e}"))?
    } else {
        return Err(actionable_elevation_error());
    };

    if !apt_output.status.success() {
        let stderr = String::from_utf8_lossy(&apt_output.stderr);
        // These substrings indicate the elevation itself was refused
        // (no TTY to prompt in, no cached password, PolicyKit denied the
        // request) rather than a genuine apt/package failure — surface
        // the actionable guidance instead of the raw (often cryptic)
        // sudo/pkexec error text in that case.
        let stderr_lower = stderr.to_lowercase();
        if stderr_lower.contains("a terminal is required")
            || stderr_lower.contains("a password is required")
            || stderr_lower.contains("not authorized")
        {
            return Err(actionable_elevation_error());
        }
        return Err(format!("'apt-get install -y gpac' failed: {}", stderr.trim()));
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

            let (mirror_url, mirror_format, mirror_sha256) =
                get_mirror_download_url("mp4box").await.map_err(|e| {
                    format!(
                        "All sources failed for MP4Box.\n  Platform: {primary_err}\n  Mirror: {e}"
                    )
                })?;

            log::info!("Downloading MP4Box from mirror: {mirror_url}");
            archive::download_and_extract_verified(
                &mirror_url,
                &tool_dir,
                mirror_format,
                mirror_sha256.as_deref(),
            )
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

// NOTE: the former `copy_companion_ffprobe_from_dir` was removed with #1081's
// switch from copying system ffmpeg to referencing it in place. For an in-place
// system ffmpeg, `metadata_tag_service::get_ffprobe_path` derives the `ffprobe`
// sibling in the same dir (e.g. /opt/homebrew/bin/ffprobe) for free; downloaded
// (managed) ffmpeg still fetches ffprobe via `install_companion_ffprobe`.

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Every probed system dir must be absolute (a relative CWD can never inject
    /// a candidate), Homebrew's dir present on macOS, and Windows probes nothing
    /// (it relies on PATH shims via `where`).
    #[test]
    fn system_tool_search_dirs_are_absolute_and_platform_correct() {
        let dirs = system_tool_search_dirs();
        assert!(dirs.iter().all(|d| d.is_absolute()));
        #[cfg(target_os = "macos")]
        {
            assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
            assert!(dirs.contains(&PathBuf::from("/opt/local/bin")));
        }
        #[cfg(target_os = "linux")]
        {
            assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
            assert!(dirs.contains(&PathBuf::from("/snap/bin")));
            assert!(dirs.contains(&PathBuf::from("/home/linuxbrew/.linuxbrew/bin")));
        }
        #[cfg(target_os = "windows")]
        assert!(dirs.is_empty());
    }

    /// Defence-in-depth: a world-writable candidate (something an unprivileged
    /// process could have planted) is untrusted; a normal 0755 binary is trusted.
    #[cfg(unix)]
    #[test]
    fn world_writable_binary_is_untrusted() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let safe = dir.path().join("safe");
        let evil = dir.path().join("evil");
        std::fs::write(&safe, b"x").unwrap();
        std::fs::write(&evil, b"x").unwrap();
        std::fs::set_permissions(&safe, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&evil, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(is_trusted_binary(&safe));
        assert!(!is_trusted_binary(&evil));
    }

    // NOTE: the Homebrew formula-list parser (`homebrew_formulae`) moved to
    // `services::package_manager` (the Homebrew arm of the package-manager
    // abstraction); its parsing test now lives in `package_manager::tests`.

    #[test]
    fn external_tool_pointer_requires_an_existing_absolute_path() {
        let base = TempDir::new().unwrap();
        let tool_dir = base.path().join("ffmpeg");
        std::fs::create_dir_all(&tool_dir).unwrap();
        let binary = base.path().join("system-ffmpeg");
        std::fs::write(&binary, b"fixture").unwrap();

        std::fs::write(
            tool_dir.join(".external-path"),
            binary.to_string_lossy().as_bytes(),
        )
        .unwrap();
        assert_eq!(read_external_tool_path(&tool_dir), Some(binary));

        std::fs::write(tool_dir.join(".external-path"), "relative/ffmpeg").unwrap();
        assert_eq!(read_external_tool_path(&tool_dir), None);
    }

    /// Promoting over an EXISTING install replaces its contents entirely —
    /// this is the #996 regression scenario (reinstall must not leave the
    /// old binary lying around next to / mixed with the new one). Also
    /// asserts staging is consumed and no `.old` backup dir is left behind
    /// on the happy path.
    #[test]
    fn promote_staged_install_replaces_existing_tool_dir() {
        let base = TempDir::new().unwrap();
        let tool_dir = base.path().join("ffmpeg");
        let staging = base.path().join("ffmpeg.staging");

        std::fs::create_dir_all(&tool_dir).unwrap();
        std::fs::write(tool_dir.join("old.txt"), b"old binary").unwrap();

        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("new.txt"), b"new binary").unwrap();

        promote_staged_install(&staging, &tool_dir).unwrap();

        // New content is in place, old content is gone.
        assert!(tool_dir.join("new.txt").exists());
        assert!(!tool_dir.join("old.txt").exists());

        // Staging is consumed.
        assert!(!staging.exists());

        // No leftover backup dir.
        let backup = base.path().join("ffmpeg.old");
        assert!(!backup.exists());
    }

    /// Promoting to a FRESH path (tool_dir doesn't exist yet) — the
    /// first-ever install case. Staging contents become tool_dir directly.
    #[test]
    fn promote_staged_install_to_fresh_path() {
        let base = TempDir::new().unwrap();
        let tool_dir = base.path().join("mp4decrypt");
        let staging = base.path().join("mp4decrypt.staging");

        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("mp4decrypt"), b"binary contents").unwrap();

        assert!(!tool_dir.exists());

        promote_staged_install(&staging, &tool_dir).unwrap();

        assert!(tool_dir.join("mp4decrypt").exists());
        assert_eq!(
            std::fs::read(tool_dir.join("mp4decrypt")).unwrap(),
            b"binary contents"
        );
    }

    /// The staging directory is always consumed (renamed away) after a
    /// successful promote, regardless of whether tool_dir pre-existed.
    #[test]
    fn promote_staged_install_consumes_staging_dir() {
        let base = TempDir::new().unwrap();
        let tool_dir = base.path().join("mp4box");
        let staging = base.path().join("mp4box.staging");

        std::fs::create_dir_all(&tool_dir).unwrap();
        std::fs::write(tool_dir.join("existing.txt"), b"v1").unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("existing.txt"), b"v2").unwrap();

        assert!(staging.exists());

        promote_staged_install(&staging, &tool_dir).unwrap();

        assert!(!staging.exists());
        assert_eq!(
            std::fs::read(tool_dir.join("existing.txt")).unwrap(),
            b"v2"
        );
    }

    /// The compiled `tool-versions.toml` (the exact bytes shipped in the
    /// binary via `include_str!`) must always parse as valid TOML — a
    /// regression here would mean every downstream `load_*` helper that
    /// reads `TOOL_VERSIONS_TOML` silently degrades to `None` at runtime.
    /// This guards the file itself, independent of any specific table.
    #[test]
    fn shipped_tool_versions_toml_parses() {
        let parsed: Result<toml::Value, _> = toml::from_str(TOOL_VERSIONS_TOML);
        assert!(
            parsed.is_ok(),
            "tool-versions.toml failed to parse: {:?}",
            parsed.err()
        );
    }

    /// `parse_mirror_asset_hash()` finds a pinned hash by exact (and
    /// case-insensitive) filename match, and returns `None` for any
    /// asset not listed in the table — the "unverified by default" #987
    /// contract.
    #[test]
    fn parse_mirror_asset_hash_reads_pinned_entry() {
        let toml_src = r#"
[mirror]
github_repo = "MeedyaSuite/MeedyaDL-Tools"
release_tag = "latest"

[mirror.asset_hashes]
"ffmpeg-linux-x86_64.tar.gz" = "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
"#;
        // Exact match.
        assert_eq!(
            parse_mirror_asset_hash(toml_src, "ffmpeg-linux-x86_64.tar.gz"),
            Some("abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234".to_string())
        );
        // Case-insensitive match.
        assert_eq!(
            parse_mirror_asset_hash(toml_src, "FFMPEG-LINUX-X86_64.TAR.GZ"),
            Some("abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234".to_string())
        );
        // Unlisted asset -> None (unverified, not an error).
        assert_eq!(
            parse_mirror_asset_hash(toml_src, "mp4box-windows-x86_64.zip"),
            None
        );
    }

    /// When `[mirror.asset_hashes]` is entirely absent — the shipped
    /// default — every lookup must return `None` rather than erroring,
    /// both for an inline fixture and for the real compiled TOML.
    #[test]
    fn parse_mirror_asset_hash_absent_table_returns_none() {
        let toml_src = r#"
[mirror]
github_repo = "MeedyaSuite/MeedyaDL-Tools"
release_tag = "latest"
"#;
        assert_eq!(
            parse_mirror_asset_hash(toml_src, "ffmpeg-linux-x86_64.tar.gz"),
            None
        );

        // The shipped tool-versions.toml has no [mirror.asset_hashes]
        // table populated (only the commented-out documentation block),
        // so every real asset lookup against it must also be None.
        assert_eq!(
            parse_mirror_asset_hash(TOOL_VERSIONS_TOML, "ffmpeg-linux-x86_64.tar.gz"),
            None
        );
    }

    /// `parse_gpac_windows_installer_pin()` only returns `Some` for a
    /// well-formed pin (non-empty URL + exactly-64-hex-char SHA-256);
    /// a missing section, a missing/short/non-hex hash all degrade to
    /// `None` rather than passing a broken pin through to the downloader.
    #[test]
    fn parse_gpac_pin_requires_wellformed_url_and_hash() {
        let good = r#"
[gpac.windows_installer]
url = "https://download.tsi.telecom-paristech.fr/gpac/release/2.6/gpac-2.6.0-rev0-g.exe"
sha256 = "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
"#;
        let pin = parse_gpac_windows_installer_pin(good);
        assert!(pin.is_some());
        let pin = pin.unwrap();
        assert_eq!(
            pin.url,
            "https://download.tsi.telecom-paristech.fr/gpac/release/2.6/gpac-2.6.0-rev0-g.exe"
        );
        assert_eq!(
            pin.sha256,
            "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
        );

        // Missing section entirely (the shipped default) -> None.
        let missing = r#"
[ffmpeg]
minimum_version = "5.0"
binary_name = "ffmpeg"
version_flag = "-version"
"#;
        assert!(parse_gpac_windows_installer_pin(missing).is_none());

        // Hash too short -> None.
        let short_hash = r#"
[gpac.windows_installer]
url = "https://example.com/gpac.exe"
sha256 = "abcd1234"
"#;
        assert!(parse_gpac_windows_installer_pin(short_hash).is_none());

        // Hash contains non-hex characters -> None.
        let non_hex = r#"
[gpac.windows_installer]
url = "https://example.com/gpac.exe"
sha256 = "zzzz1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
"#;
        assert!(parse_gpac_windows_installer_pin(non_hex).is_none());
    }
}
