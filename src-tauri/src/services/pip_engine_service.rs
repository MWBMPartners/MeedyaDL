// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Generic pip-based engine management service.
// =============================================
//
// Provides install and version-check functions for any Python package
// that MeedyaDL manages as a download engine (e.g., GAMDL, votify,
// OF-Scraper). Uses the managed Python runtime from python_manager.rs.
//
// This module generalises the pattern from gamdl_service.rs so new
// pip-based engines can be added with zero new Rust service code —
// just call `install_pip_engine("votify")` or `get_pip_engine_version("ofscraper")`.
//
// The engines.toml registry defines which packages are available,
// their required/optional status, and whether they're enabled.

use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::platform;

/// True only for a bare PyPI package name (no URLs, VCS refs, paths, or flags).
///
/// PyPI package names are restricted to ASCII letters, digits, `.`, `-`,
/// and `_`. But pip's "install target" grammar accepts far more than a
/// bare package name — direct URLs (`pkg @ https://evil/x.whl`), VCS
/// refs (`git+https://...`), local paths (`./local`, `/abs/path`), and
/// arbitrary CLI-flag-shaped strings (`--index-url=http://evil`) — any
/// of which can point pip at attacker-controlled code (including build
/// hooks that execute at install time) if forwarded verbatim from an
/// untrusted caller (#985).
///
/// This function is a strict allowlist: non-empty, at most 80 chars,
/// first AND last character ASCII-alphanumeric, and every character
/// drawn from `[A-Za-z0-9._-]`. This rejects `@`, `:`, `/`, whitespace,
/// and a leading `-` (which pip/argparse would otherwise interpret as
/// a flag).
#[must_use]
pub fn is_safe_pip_package_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 80 {
        return false;
    }

    let first_is_alnum = name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
    let last_is_alnum = name.chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric());
    if !first_is_alnum || !last_is_alnum {
        return false;
    }

    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Installs a pip package into the managed Python environment.
///
/// Runs `python -m pip install --upgrade <package>` and returns
/// the installed version on success.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name (e.g., "votify", "ofscraper", "yt-dlp")
///
/// # Returns
/// * `Ok(version)` - The installed version string
/// * `Err(message)` - If Python is missing, pip install failed, or
///   `package` fails the [`is_safe_pip_package_name`] allowlist check.
pub async fn install_pip_engine(app: &AppHandle, package: &str) -> Result<String, String> {
    // Security guard (#985): `package` may originate from an IPC call
    // (`upgrade_pip_engine`). Refuse anything that isn't a bare PyPI
    // package name before it ever reaches a subprocess argv.
    if !is_safe_pip_package_name(package) {
        return Err(format!(
            "Refusing to run pip on an unsafe package spec: {package}"
        ));
    }

    log::info!("Installing {package} via pip...");

    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix): a reused system Python is
    // provisioned as a venv, which on Windows puts `python.exe` under
    // `Scripts/` rather than at the portable root. The pure
    // `get_python_binary_path` only understands the portable layout, so
    // using it here made every pip-engine install (votify, yt-dlp,
    // ofscraper) silently fail with "Python is not installed" on Windows
    // whenever the user was running a system-venv Python.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Err(format!(
            "Cannot install {package}: Python is not installed. Run the setup wizard first."
        ));
    }

    let output = Command::new(&python_bin)
        .args(["-m", "pip", "install", "--upgrade", package])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install {package}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pip install {package} failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install {package} output: {}", stdout.trim());

    let version = get_pip_engine_version(app, package)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    // Post-install integrity verification: log the install location for audit trail
    if let Ok(show_output) = Command::new(&python_bin)
        .args(["-m", "pip", "show", package, "--verbose"])
        .output()
        .await
    {
        let show_text = String::from_utf8_lossy(&show_output.stdout);
        if let Some(loc_line) = show_text.lines().find(|l| l.starts_with("Location:")) {
            let location = loc_line.trim_start_matches("Location:").trim();
            log::info!("{package} v{version} installed at: {location}");
        }
    }

    log::info!("{package} v{version} installed and verified successfully");
    Ok(version)
}

/// Checks whether a pip package is installed and returns its version.
///
/// Runs `python -m pip show <package>` and parses the "Version:" line.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name
///
/// # Returns
/// * `Ok(Some(version))` - Package is installed with the given version
/// * `Ok(None)` - Package is not installed
/// * `Err(message)` - Python is not available or pip failed
pub async fn get_pip_engine_version(
    app: &AppHandle,
    package: &str,
) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix) — see `install_pip_engine` above.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Ok(None);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(&python_bin)
            .args(["-m", "pip", "show", package])
            .output(),
    )
    .await
    .map_err(|_| {
        format!("pip show {package} timed out (10s) — Python environment may be unresponsive")
    })?
    .map_err(|e| format!("Failed to run pip show {package}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "Version: X.Y.Z" from pip show output
    let version = stdout
        .lines()
        .find(|line| line.starts_with("Version:"))
        .map(|line| line.trim_start_matches("Version:").trim().to_string());

    Ok(version)
}

/// Uninstalls a pip package from the managed Python environment.
///
/// Runs `python -m pip uninstall -y <package>`.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name
///
/// # Errors
/// Returns an error if Python is missing, pip uninstall failed, or
/// `package` fails the [`is_safe_pip_package_name`] allowlist check.
pub async fn uninstall_pip_engine(app: &AppHandle, package: &str) -> Result<(), String> {
    // Security guard (#985): same rationale as `install_pip_engine`.
    if !is_safe_pip_package_name(package) {
        return Err(format!(
            "Refusing to run pip on an unsafe package spec: {package}"
        ));
    }

    log::info!("Uninstalling {package} via pip...");

    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix) — see `install_pip_engine` above.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Err("Python is not installed".to_string());
    }

    let output = Command::new(&python_bin)
        .args(["-m", "pip", "uninstall", "-y", package])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip uninstall {package}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pip uninstall {package} failed: {}",
            stderr.trim()
        ));
    }

    log::info!("{package} uninstalled successfully");
    Ok(())
}

/// Queries the PyPI JSON API for the latest version of a package.
///
/// Returns `Ok(Some(version))` if the package exists on PyPI,
/// `Ok(None)` if the API call succeeds but the package is not found,
/// `Err` on network/parsing failure.
pub async fn check_latest_pypi_version(package: &str) -> Result<Option<String>, String> {
    let url = format!("https://pypi.org/pypi/{package}/json");

    let client = crate::utils::http_client::build_simple(10)?;

    let response = client
        .get(&url)
        .header("User-Agent", crate::utils::http_client::BROWSER_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("PyPI request failed for {package}: {e}"))?;

    if response.status().as_u16() == 404 {
        return Ok(None); // Package not found on PyPI
    }
    if !response.status().is_success() {
        return Err(format!(
            "PyPI returned HTTP {} for {package}",
            response.status()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PyPI response for {package}: {e}"))?;

    let version = json["info"]["version"]
        .as_str()
        .map(std::string::ToString::to_string);

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real-world engine package names (from engines.toml) must all
    /// pass the allowlist — this guard must never block a legitimate
    /// upgrade/install of a known engine.
    #[test]
    fn accepts_known_engine_package_names() {
        for name in ["votify", "yt-dlp", "ofscraper", "gamdl"] {
            assert!(
                is_safe_pip_package_name(name),
                "expected {name:?} to be accepted"
            );
        }
    }

    /// #985 — a raw `package` string forwarded verbatim to
    /// `pip install --upgrade <package>` lets an attacker point pip at
    /// a URL, VCS ref, local path, or extra CLI flags. None of these
    /// shapes may pass the allowlist.
    #[test]
    fn rejects_unsafe_package_specs() {
        let unsafe_specs = [
            "foo @ https://evil/x.whl",
            "--index-url=http://evil",
            "git+https://x",
            "./local",
            "gamdl; rm -rf ~",
            "",
        ];
        for spec in unsafe_specs {
            assert!(
                !is_safe_pip_package_name(spec),
                "expected {spec:?} to be rejected"
            );
        }
    }

    /// The 80-character length cap rejects pathologically long specs
    /// even if every individual character would otherwise be allowed.
    #[test]
    fn rejects_overlong_package_name() {
        let long_name = "a".repeat(200);
        assert!(!is_safe_pip_package_name(&long_name));
    }
}
