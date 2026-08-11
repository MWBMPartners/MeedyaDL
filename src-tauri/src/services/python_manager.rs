// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Python runtime manager service.
// Downloads, installs, and manages a portable Python runtime from the
// python-build-standalone project (https://github.com/indygreg/python-build-standalone).
// The portable Python is installed into the app data directory and is
// completely self-contained, avoiding conflicts with any system Python.
//
// ## Architecture Overview
//
// This service is one of the first things invoked during the setup wizard flow.
// The setup wizard (frontend) calls the `install_python` Tauri command, which
// delegates here. Once installed, the Python binary is used by `gamdl_service.rs`
// to run `python -m gamdl` and by `gamdl_service::install_gamdl` to run
// `python -m pip install gamdl`.
//
// ## Flow Summary
//
// 1. `install_python()` - Main entry: downloads tar.gz, extracts, verifies binary
// 2. `check_python_status()` - Quick health check: binary exists + runs --version
// 3. `get_installed_python_version()` - Lightweight version query for update checks
// 4. `uninstall_python()` - Clean removal for reinstall scenarios
//
// ## References
//
// - python-build-standalone releases: https://github.com/indygreg/python-build-standalone/releases
// - Reqwest HTTP client (used by archive utilities): https://docs.rs/reqwest/latest/reqwest/
// - Tokio async runtime and process spawning: https://docs.rs/tokio/latest/tokio/
// - Tauri app handle and path resolution: https://v2.tauri.app/develop/calling-rust/

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::AppHandle;

use serde::{Deserialize, Serialize};

// `archive` provides download_and_extract() and set_executable() helpers.
// `platform` provides cross-platform path resolution for Python directories and binaries.
use crate::utils::{archive, platform};

// ============================================================
// Python version constants
// Update these when a new Python release is available.
// The release tag and version must match an actual release on GitHub.
// ============================================================

/// The Python version to install (used in the download URL and version checks).
/// This must correspond to a `CPython` version available in the python-build-standalone
/// release specified by `RELEASE_TAG`. Check the release assets at:
/// <https://github.com/indygreg/python-build-standalone/releases/tag/{RELEASE_TAG>}
const PYTHON_VERSION: &str = "3.12.8";

/// The python-build-standalone release tag on GitHub (date-based, format: YYYYMMDD).
/// Each release bundles multiple Python versions as pre-compiled archives.
/// Browse available releases: <https://github.com/indygreg/python-build-standalone/releases>
const RELEASE_TAG: &str = "20250106";

/// The base URL for python-build-standalone release downloads on GitHub.
/// The full URL is constructed as: {base}/{tag}/cpython-{version}+{tag}-{triple}-install_only.tar.gz
/// See: <https://github.com/indygreg/python-build-standalone#available-distributions>
const DOWNLOAD_BASE_URL: &str =
    "https://github.com/indygreg/python-build-standalone/releases/download";

/// Constructs the download URL for the portable Python archive matching
/// the current platform and architecture.
///
/// Uses the python-build-standalone project's `install_only` variant, which
/// provides a minimal Python installation suitable for running pip and
/// Python packages like GAMDL.
///
/// # URL format
/// `{base}/{tag}/cpython-{version}+{tag}-{triple}-install_only.tar.gz`
///
/// # Returns
/// * `Ok(url)` - The download URL for the current platform
/// * `Err(message)` - If the current platform is unsupported
fn get_python_download_url() -> Result<String, String> {
    // Determine the platform triple for python-build-standalone.
    // These triples match the naming convention used in python-build-standalone releases.
    // The "install_only" variant is a minimal distribution containing just the Python
    // interpreter, standard library, and pip — ideal for running GAMDL as a pip package.
    // Ref: https://gregoryszorc.com/docs/python-build-standalone/main/running.html
    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        // macOS: Apple Silicon (M1/M2/M3) and Intel
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        // Linux: x86_64 and ARM64 with glibc (musl not supported)
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        // Windows: x86_64 and ARM64 built with MSVC toolchain
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        (os, arch) => {
            return Err(format!(
                "Unsupported platform for portable Python: {os}/{arch}"
            ))
        }
    };

    // Build the full download URL following python-build-standalone naming convention:
    // Example: https://github.com/indygreg/python-build-standalone/releases/download/
    //          20250106/cpython-3.12.8+20250106-aarch64-apple-darwin-install_only.tar.gz
    let url = format!(
        "{DOWNLOAD_BASE_URL}/{RELEASE_TAG}/cpython-{PYTHON_VERSION}+{RELEASE_TAG}-{triple}-install_only.tar.gz"
    );

    log::info!("Python download URL: {url}");
    Ok(url)
}

/// Downloads and installs the portable Python runtime.
///
/// This is the main entry point called by the setup wizard's Python
/// installation step. It performs the following:
/// 1. Determines the correct download URL for the current platform
/// 2. Cleans up any previous installation in the Python directory
/// 3. Downloads the tar.gz archive from python-build-standalone
/// 4. Extracts the archive to the app data directory
/// 5. Verifies the Python binary exists and works
/// 6. Returns the installed Python version string
///
/// The archive extracts to a `python/` subdirectory, which matches
/// the path returned by `platform::get_python_dir()`.
///
/// # Arguments
/// * `app` - The Tauri app handle, used for path resolution
///
/// # Errors
///
/// Returns `Err(String)` if the download URL cannot be determined, the archive
/// download or extraction fails, or the installed Python binary fails verification.
///
/// # Returns
/// * `Ok(version)` - The installed Python version (e.g., "3.12.8")
/// * `Err(message)` - A descriptive error if installation failed
pub async fn install_python(app: &AppHandle) -> Result<String, String> {
    log::info!("Starting Python installation...");

    // Step 1: Determine the download URL for this platform
    let url = get_python_download_url()?;

    // Step 2: Resolve installation paths
    let python_dir = platform::get_python_dir(app);
    let app_data_dir = platform::get_app_data_dir(app);

    // Step 3: Clean up any existing Python installation
    if python_dir.exists() {
        log::info!(
            "Removing existing Python installation at {}",
            python_dir.display()
        );
        std::fs::remove_dir_all(&python_dir).map_err(|e| {
            format!(
                "Failed to remove existing Python directory {}: {}",
                python_dir.display(),
                e
            )
        })?;
    }

    // Ensure the app data directory exists (archive extracts into it)
    std::fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!(
            "Failed to create app data directory {}: {}",
            app_data_dir.display(),
            e
        )
    })?;

    // Step 4: Download and extract the Python archive.
    // The archive contains a `python/` directory at the top level, which
    // extracts to `app_data_dir/python/` — exactly our python_dir path.
    // The download_and_extract utility uses reqwest to stream the download
    // and flate2/tar to extract the tar.gz archive.
    // Ref: https://docs.rs/reqwest/latest/reqwest/ (HTTP streaming download)
    log::info!(
        "Downloading and extracting Python to {}",
        app_data_dir.display()
    );
    archive::download_and_extract(&url, &app_data_dir, archive::ArchiveFormat::TarGz).await?;

    // Step 5: Verify the installation by checking the binary exists
    let python_bin = platform::get_python_binary_path(&python_dir);
    if !python_bin.exists() {
        return Err(format!(
            "Python installation failed: binary not found at {}",
            python_bin.display()
        ));
    }

    // Ensure the binary is executable on Unix (chmod +x).
    // On Windows this is a no-op since executability is determined by file extension.
    archive::set_executable(&python_bin)?;

    // Step 6: Get the installed version by running `python --version`.
    // This validates that the binary is not just present but actually functional.
    // Uses tokio::process::Command for async subprocess execution.
    // Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html
    let version = get_python_version_from_binary(&python_bin).await?;
    log::info!(
        "Python {} installed successfully at {}",
        version,
        python_dir.display()
    );

    // Record the provenance so the rest of the app knows this is the managed
    // portable runtime (as opposed to a venv built from a system Python, #1017).
    // Best-effort — a missing marker just falls back to "portable" semantics.
    write_source_marker(
        &python_dir,
        &PythonSourceRecord {
            source: PYTHON_SOURCE_PORTABLE.to_string(),
            interpreter: None,
            version: version.clone(),
        },
    );

    Ok(version)
}

/// Gets the Python version string by running the binary with --version.
///
/// Executes `python --version` and parses the output "Python X.Y.Z"
/// to extract just the version number "X.Y.Z".
///
/// # Arguments
/// * `python_bin` - Path to the Python binary to run
///
/// # Returns
/// * `Ok(version)` - The version string (e.g., "3.12.8")
/// * `Err(message)` - If the binary couldn't be executed or output couldn't be parsed
async fn get_python_version_from_binary(python_bin: &PathBuf) -> Result<String, String> {
    // Spawn the Python process asynchronously using tokio::process::Command.
    // The .output() method captures both stdout and stderr and waits for completion.
    // Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.output
    let output = tokio::process::Command::new(python_bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to run Python binary: {e}"))?;

    // Parse "Python 3.12.8" -> "3.12.8"
    // CPython always outputs "Python X.Y.Z" to stdout. We strip that prefix
    // to get the bare version string for display and comparison purposes.
    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = version_str
        .trim()
        .strip_prefix("Python ")
        .unwrap_or_else(|| version_str.trim())
        .to_string();

    if version.is_empty() {
        // Some Python builds or error conditions output to stderr instead of stdout.
        // Report the stderr content to aid in debugging installation issues.
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Python --version returned empty output. stderr: {}",
            stderr.trim()
        ));
    }

    Ok(version)
}

/// Checks whether the portable Python runtime is installed and working.
///
/// Verifies that the Python binary exists at the expected path and
/// can successfully execute `--version`. Returns the version if installed.
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Errors
///
/// Returns `Err(String)` if an unexpected error occurs while checking the
/// Python binary (e.g., path resolution failure).
///
/// # Returns
/// * `Ok(Some(version))` - Python is installed with the given version
/// * `Ok(None)` - Python is not installed
/// * `Err(message)` - An error occurred while checking
pub async fn check_python_status(app: &AppHandle) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    // Runtime resolution: tolerant of both the portable layout AND a venv
    // provisioned from a system Python (#1017) — matters on Windows where the
    // venv interpreter lives under `Scripts/` rather than the portable root.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    // If the binary doesn't exist, Python is not installed
    if !python_bin.exists() {
        return Ok(None);
    }

    // Try to get the version by actually running the binary.
    // This catches cases where the binary file exists but is corrupt,
    // has missing shared libraries, or is otherwise non-functional.
    match get_python_version_from_binary(&python_bin).await {
        Ok(version) => Ok(Some(version)),
        Err(e) => {
            log::warn!("Python binary exists but failed to get version: {e}");
            // Binary exists but isn't working — treat as not installed so that
            // the setup wizard will offer to reinstall.
            Ok(None)
        }
    }
}

/// Returns the expected Python version string for display in the UI.
///
/// This is the version that will be installed (or should be installed).
/// Called by the frontend's setup wizard to show "Python X.Y.Z will be installed".
#[must_use]
pub const fn expected_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the target Python version constant.
///
/// Used by `update_checker.rs` to compare the installed version against the
/// version we expect, enabling the update UI to prompt for a Python update
/// when a newer python-build-standalone release is configured.
#[must_use]
pub const fn get_target_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the installed Python version by running the binary.
/// Returns None if Python is not installed or the binary fails.
///
/// This is a lightweight query used by `update_checker.rs` to determine
/// the currently installed version without needing the full `AppHandle`.
///
/// # Arguments
/// * `python_dir` - The directory where Python is installed (e.g., {`app_data}/python`/)
pub async fn get_installed_python_version(python_dir: &std::path::Path) -> Option<String> {
    // Resolve the binary path (portable OR system-venv layout, #1017).
    let python_bin = crate::utils::platform::resolve_managed_python_binary(python_dir);
    if !python_bin.exists() {
        return None;
    }
    // Silently return None on any error (binary corrupt, permissions, etc.)
    get_python_version_from_binary(&python_bin).await.ok()
}

/// Removes the portable Python installation entirely.
///
/// Used when the user wants to reinstall Python or the installation
/// is detected as corrupt. This also effectively uninstalls GAMDL and
/// all pip packages, since they live inside the portable Python directory.
///
/// Uses `tokio::fs::remove_dir_all` for async directory removal, which
/// avoids blocking the Tokio runtime on potentially slow filesystem operations.
/// Ref: <https://docs.rs/tokio/latest/tokio/fs/fn.remove_dir_all.html>
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Errors
///
/// Returns `Err(String)` if the Python directory cannot be removed.
pub async fn uninstall_python(app: &AppHandle) -> Result<(), String> {
    let python_dir = platform::get_python_dir(app);

    if python_dir.exists() {
        log::info!("Removing Python installation at {}", python_dir.display());
        tokio::fs::remove_dir_all(&python_dir)
            .await
            .map_err(|e| format!("Failed to remove Python directory: {e}"))?;
    }

    Ok(())
}

// ============================================================
// System Python detection + reuse (#1017)
//
// Rather than always downloading the portable runtime, MeedyaDL can detect a
// compatible Python already on the machine and provision a `venv` from it into
// `{app_data}/python/`. On Unix that venv exposes exactly `bin/python3` +
// `bin/pip3` — the same paths the portable runtime uses — so every existing
// `{python} -m gamdl` / `-m pip` call site keeps working unchanged. A venv is
// required (not a bare interpreter reference) because modern system Pythons
// (Homebrew 3.12+, most distros) are PEP 668 "externally-managed" and reject
// `pip install` into the base environment.
// ============================================================

/// Minimum acceptable CPython `(major, minor)` for a *reused* system Python.
///
/// Forced by GAMDL 3.8.2+: its compiled `_ammuxer` decrypt/mux extension ships
/// as a `cp310-abi3` wheel, which loads on any CPython >= 3.10 (the stable ABI
/// is forward-compatible across minors) but NOT on 3.9 or earlier. A reused
/// system Python only has to clear this floor; it need not match the bundled
/// portable [`PYTHON_VERSION`] (3.12).
const MIN_SYSTEM_PYTHON: (u32, u32) = (3, 10);

/// `source` value written to the provenance marker for the managed portable runtime.
const PYTHON_SOURCE_PORTABLE: &str = "portable";
/// `source` value written to the provenance marker for a venv built from a system Python.
const PYTHON_SOURCE_SYSTEM_VENV: &str = "system-venv";

/// A Python interpreter discovered on the user's system, offered as an
/// alternative to downloading the portable runtime (#1017).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPython {
    /// Canonical interpreter path (`sys.executable`), suitable for passing to
    /// [`provision_venv_from_system_python`].
    pub path: String,
    /// Detected version string, e.g. `"3.14.6"`.
    pub version: String,
    /// Whether `version` meets [`MIN_SYSTEM_PYTHON`].
    pub meets_floor: bool,
    /// Human-readable provenance label for the UI (Homebrew / python.org / etc.).
    pub source: String,
}

/// Provenance marker persisted at `{python_dir}/.meedya-python-source.json`.
///
/// Lets the rest of the app distinguish the managed portable runtime from a
/// venv built out of a system Python — most importantly so the Updates page
/// does NOT offer to "update" a user-provided Python to the portable
/// [`PYTHON_VERSION`] (which would silently replace their chosen interpreter).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonSourceRecord {
    /// Either [`PYTHON_SOURCE_PORTABLE`] or [`PYTHON_SOURCE_SYSTEM_VENV`].
    pub source: String,
    /// For a system venv: the source interpreter the venv was built from.
    #[serde(default)]
    pub interpreter: Option<String>,
    /// The resolved interpreter version at provision time.
    pub version: String,
}

impl PythonSourceRecord {
    /// True when this managed Python is a venv built from a user's system Python.
    #[must_use]
    pub fn is_system_venv(&self) -> bool {
        self.source == PYTHON_SOURCE_SYSTEM_VENV
    }
}

/// One line of interpreter probe output: version + real executable path.
struct ProbeResult {
    version: String,
    exe: String,
}

/// Tiny Python snippet that prints `MAJOR.MINOR.MICRO|<sys.executable>` on one
/// line. Using `sys.executable` (rather than the invoked name) gives us the
/// REAL interpreter path for dedup and for building the venv, so `python3`,
/// `python3.14`, and an absolute Homebrew path that all resolve to the same
/// binary collapse to a single candidate.
const PROBE_SCRIPT: &str =
    "import sys;print(f\"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}|{sys.executable}\")";

/// Parses a dotted version string into `(major, minor, micro)`.
///
/// Accepts `"3.14.6"`, `"3.12"` (micro defaults to 0), and tolerates trailing
/// junk on the micro component (`"3.11.0rc1"` → `(3, 11, 0)`). Returns `None`
/// when there are fewer than two numeric components.
fn parse_version_tuple(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().split('.');
    let major = parts.next()?.trim().parse::<u32>().ok()?;
    let minor = parts.next()?.trim().parse::<u32>().ok()?;
    // Micro is optional; strip any non-digit suffix (e.g. "0rc1").
    let micro = parts
        .next()
        .map(|s| {
            let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().unwrap_or(0)
        })
        .unwrap_or(0);
    Some((major, minor, micro))
}

/// Whether a `(major, minor)` clears [`MIN_SYSTEM_PYTHON`].
fn version_meets_floor(major: u32, minor: u32) -> bool {
    (major, minor) >= MIN_SYSTEM_PYTHON
}

/// Orders two version strings; unparsable strings sort lowest.
fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa = parse_version_tuple(a).unwrap_or((0, 0, 0));
    let pb = parse_version_tuple(b).unwrap_or((0, 0, 0));
    pa.cmp(&pb)
}

/// Classifies a resolved interpreter path into a UI-friendly provenance label.
fn classify_source(exe: &str) -> String {
    let lower = exe.to_lowercase();
    if lower.contains("/opt/homebrew") || lower.contains("/usr/local/cellar") || lower.contains("homebrew") {
        "Homebrew".to_string()
    } else if lower.contains("python.framework") || lower.contains("/library/frameworks") {
        "python.org".to_string()
    } else if lower.contains(".pyenv") {
        "pyenv".to_string()
    } else if lower.contains("windowsapps") {
        "Microsoft Store".to_string()
    } else {
        "System".to_string()
    }
}

/// Runs `<program> [extra_args...] -c <PROBE_SCRIPT>` with a short timeout and
/// parses the `version|exe` line. Returns `None` on spawn failure, non-zero
/// exit, timeout, or unparsable output. No shell is involved and all arguments
/// are fixed/parameterised.
async fn probe_interpreter(program: &str, extra_args: &[&str]) -> Option<ProbeResult> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(extra_args).arg("-c").arg(PROBE_SCRIPT);
    let output = tokio::time::timeout(Duration::from_secs(2), cmd.output())
        .await
        .ok()? // timed out
        .ok()?; // spawn/IO error
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();
    let (version, exe) = line.split_once('|')?;
    // Reject anything whose version we can't parse (guards against surprise output).
    parse_version_tuple(version)?;
    Some(ProbeResult {
        version: version.to_string(),
        exe: exe.trim().to_string(),
    })
}

/// Builds the ordered list of interpreter candidates `(program, extra_args)` to
/// probe for the current platform. Absolute paths are pre-filtered by existence
/// to avoid pointless spawns; bare names are always tried (PATH resolves them).
fn candidate_interpreters() -> Vec<(String, Vec<String>)> {
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
    // Minor versions to probe by explicit name, newest first.
    const MINORS: &[&str] = &["3.14", "3.13", "3.12", "3.11", "3.10"];

    if cfg!(target_os = "windows") {
        // The `py` launcher is the canonical way to find Pythons on Windows.
        candidates.push(("py".to_string(), vec!["-3".to_string()]));
        for m in MINORS {
            candidates.push(("py".to_string(), vec![format!("-{m}")]));
        }
        // PATH fallbacks.
        candidates.push(("python".to_string(), vec![]));
        candidates.push(("python3".to_string(), vec![]));
    } else {
        // Bare PATH names (newest-specific first, then the generic python3).
        for m in MINORS {
            candidates.push((format!("python{m}"), vec![]));
        }
        candidates.push(("python3".to_string(), vec![]));

        // Well-known absolute install locations. Only add those that exist.
        let mut abs: Vec<String> = Vec::new();
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            abs.push(format!("{dir}/python3"));
            for m in MINORS {
                abs.push(format!("{dir}/python{m}"));
            }
        }
        // pyenv shims.
        if let Some(home) = std::env::var_os("HOME") {
            let shim = Path::new(&home).join(".pyenv/shims/python3");
            abs.push(shim.to_string_lossy().to_string());
        }
        // macOS python.org framework builds: /Library/Frameworks/Python.framework/Versions/*/bin/python3
        let versions_dir = Path::new("/Library/Frameworks/Python.framework/Versions");
        if let Ok(entries) = std::fs::read_dir(versions_dir) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin/python3");
                if bin.exists() {
                    abs.push(bin.to_string_lossy().to_string());
                }
            }
        }
        for p in abs {
            if Path::new(&p).exists() {
                candidates.push((p, vec![]));
            }
        }
    }
    candidates
}

/// Detects compatible Python interpreters already installed on the system,
/// deduplicated by their real (`sys.executable`) path.
///
/// Candidates that fail to run, time out, or produce unparsable output are
/// silently skipped. The result is sorted so the best choice is first:
/// floor-meeting interpreters ahead of too-old ones, then highest version.
/// Interpreters BELOW the floor are still returned (with `meets_floor = false`)
/// so the UI can explain *why* an otherwise-present Python can't be reused.
pub async fn detect_system_pythons() -> Vec<SystemPython> {
    let candidates = candidate_interpreters();
    let mut seen: HashSet<String> = HashSet::new();
    let mut results: Vec<SystemPython> = Vec::new();

    for (program, extra) in candidates {
        let extra_refs: Vec<&str> = extra.iter().map(String::as_str).collect();
        let Some(pr) = probe_interpreter(&program, &extra_refs).await else {
            continue;
        };
        // Dedup by canonicalised real path.
        let canonical = std::fs::canonicalize(&pr.exe)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| pr.exe.clone());
        if !seen.insert(canonical) {
            continue;
        }
        let (major, minor, _) = parse_version_tuple(&pr.version).unwrap_or((0, 0, 0));
        results.push(SystemPython {
            source: classify_source(&pr.exe),
            path: pr.exe,
            meets_floor: version_meets_floor(major, minor),
            version: pr.version,
        });
    }

    // Best first: floor-meeting before too-old, then newest version.
    results.sort_by(|a, b| {
        b.meets_floor
            .cmp(&a.meets_floor)
            .then_with(|| cmp_versions(&b.version, &a.version))
    });
    results
}

/// Path to the provenance marker inside a managed Python directory.
fn source_marker_path(python_dir: &Path) -> PathBuf {
    python_dir.join(".meedya-python-source.json")
}

/// Best-effort write of the provenance marker. A failure here is non-fatal —
/// the app falls back to portable semantics when the marker is absent.
fn write_source_marker(python_dir: &Path, record: &PythonSourceRecord) {
    match serde_json::to_string_pretty(record) {
        Ok(json) => {
            if let Err(e) = std::fs::write(source_marker_path(python_dir), json) {
                log::warn!("Failed to write Python source marker: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialise Python source marker: {e}"),
    }
}

/// Reads the provenance marker for a managed Python directory, if present.
///
/// Path-based core of [`get_python_source`], split out so it can be driven
/// directly against a `tempfile` directory in unit tests without needing a
/// live `AppHandle` — mirrors the `_for`/`_at` splitting pattern already used
/// by [`platform::resolve_managed_python_binary_for`] for the same reason.
///
/// Returns `None` when Python isn't provisioned yet, the marker is missing (a
/// portable install from before #1017), or the file is unreadable/corrupt — all
/// of which the caller should treat as the portable default.
fn read_source_marker(python_dir: &Path) -> Option<PythonSourceRecord> {
    let data = std::fs::read_to_string(source_marker_path(python_dir)).ok()?;
    serde_json::from_str(&data).ok()
}

/// Reads the provenance marker for the managed Python, if present.
///
/// Returns `None` when Python isn't provisioned yet, the marker is missing (a
/// portable install from before #1017), or the file is unreadable/corrupt — all
/// of which the caller should treat as the portable default.
#[must_use]
pub fn get_python_source(app: &AppHandle) -> Option<PythonSourceRecord> {
    read_source_marker(&platform::get_python_dir(app))
}

/// Provisions the managed Python by building a `venv` from a user-chosen system
/// interpreter, instead of downloading the portable runtime (#1017).
///
/// Steps: validate the interpreter + version floor → wipe any existing managed
/// Python → `<interpreter> -m venv {app_data}/python` → verify the venv
/// interpreter runs → write the provenance marker. GAMDL is installed into the
/// venv later by the normal `install_gamdl` flow.
///
/// # Errors
/// Returns a user-facing `Err(String)` when the interpreter can't be run, is
/// below [`MIN_SYSTEM_PYTHON`], or `venv`/`ensurepip` is unavailable (common on
/// Debian/Ubuntu, which split `python3-venv` into a separate package) — in
/// which case the caller should fall back to the portable download.
pub async fn provision_venv_from_system_python(
    app: &AppHandle,
    interpreter: &str,
) -> Result<String, String> {
    log::info!("Provisioning managed Python from system interpreter: {interpreter}");

    // Step 1: probe the interpreter (also re-validates it's a real Python 3).
    let probe = probe_interpreter(interpreter, &[]).await.ok_or_else(|| {
        format!(
            "Could not run the selected Python at '{interpreter}'. Make sure it is a valid Python 3 executable."
        )
    })?;
    let (major, minor, _) = parse_version_tuple(&probe.version).ok_or_else(|| {
        format!("Unrecognised Python version reported by '{interpreter}': {}", probe.version)
    })?;
    if !version_meets_floor(major, minor) {
        return Err(format!(
            "Python {} is too old — GAMDL needs Python {}.{}+ (its compiled decrypt module is a cp310-abi3 wheel). Choose a newer Python or download the portable runtime.",
            probe.version, MIN_SYSTEM_PYTHON.0, MIN_SYSTEM_PYTHON.1
        ));
    }

    // Step 2: wipe any existing managed Python so the venv starts clean.
    let python_dir = platform::get_python_dir(app);
    let app_data_dir = platform::get_app_data_dir(app);
    if python_dir.exists() {
        std::fs::remove_dir_all(&python_dir).map_err(|e| {
            format!("Failed to remove existing Python directory {}: {e}", python_dir.display())
        })?;
    }
    std::fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!("Failed to create app data directory {}: {e}", app_data_dir.display())
    })?;

    // Step 3: create the venv. Uses the interpreter's own `venv` module — no
    // shell, fixed arguments.
    let output = tokio::process::Command::new(interpreter)
        .arg("-m")
        .arg("venv")
        .arg(&python_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to launch '{interpreter} -m venv': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Debian/Ubuntu ship Python without ensurepip/venv unless python3-venv
        // is installed; surface that specifically so the message is actionable.
        let hint = if stderr.contains("ensurepip") || stderr.contains("No module named venv") {
            " This Python is missing the 'venv'/'ensurepip' module — on Debian/Ubuntu install the 'python3-venv' package, or download the portable runtime instead."
        } else {
            ""
        };
        // Best-effort cleanup of a partial venv.
        let _ = std::fs::remove_dir_all(&python_dir);
        return Err(format!(
            "Failed to create a virtual environment from '{interpreter}'.{hint}\n{}",
            stderr.trim()
        ));
    }

    // Step 4: verify the venv interpreter actually runs.
    let venv_bin = platform::resolve_managed_python_binary(&python_dir);
    if !venv_bin.exists() {
        let _ = std::fs::remove_dir_all(&python_dir);
        return Err(format!(
            "Virtual environment created but no Python binary was found at {}",
            venv_bin.display()
        ));
    }
    let version = get_python_version_from_binary(&venv_bin).await?;

    // Step 5: record provenance (suppresses portable-Python "update" prompts).
    write_source_marker(
        &python_dir,
        &PythonSourceRecord {
            source: PYTHON_SOURCE_SYSTEM_VENV.to_string(),
            interpreter: Some(interpreter.to_string()),
            version: version.clone(),
        },
    );

    log::info!(
        "Provisioned system-Python venv ({version}) from {interpreter} at {}",
        python_dir.display()
    );
    Ok(version)
}

// ============================================================
// Broken system-venv diagnosis (#1017 follow-up)
//
// A venv provisioned from a system Python (see above) embeds a symlink /
// reference back to that system interpreter's Cellar/framework/pyenv path.
// When the user later runs `brew upgrade python` (or similarly relocates /
// removes the interpreter the venv was built from), Homebrew deletes the old
// versioned Cellar directory — the venv's own interpreter link dangles, and
// running it fails outright. `check_python_status` (above) can only see
// "binary exists but won't run" and reports `Ok(None)`, which the setup
// wizard renders as an undifferentiated "Python Not Found" — mystifying for
// a user who previously had everything working.
//
// `diagnose_python_venv` distinguishes that specific, recoverable case (a
// system-venv whose recorded interpreter has moved) from a genuinely fresh
// "nothing installed yet" state, so the wizard can offer a one-click rebuild
// instead of just the portable-download flow.
// ============================================================

/// Result of diagnosing the managed Python's health.
///
/// Distinct from [`check_python_status`]'s `Result<Option<String>, String>`
/// contract, which existing callers depend on unchanged — this is an
/// additive, richer diagnosis consumed only by the new
/// `diagnose_python_venv` IPC command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PythonVenvHealth {
    /// The managed Python binary exists and runs.
    Ok { version: String },
    /// The managed Python binary exists but fails to run, AND the
    /// provenance marker says it was built from a system Python — almost
    /// always a moved/removed system interpreter (e.g. a Homebrew upgrade
    /// that relocated the Cellar path the venv's interpreter symlink points
    /// at).
    BrokenSystemVenv {
        /// The interpreter path recorded at provision time, if the marker
        /// has one.
        recorded_interpreter: Option<String>,
        /// The Python version recorded at provision time.
        recorded_version: Option<String>,
        /// Whether `recorded_interpreter` still exists as a file on disk —
        /// determines whether a one-click rebuild from the SAME interpreter
        /// is offered, or whether the user must pick a different one.
        recorded_interpreter_exists: bool,
    },
    /// The binary is missing outright, or exists but fails to run with no
    /// system-venv provenance to explain it (a broken portable install, or a
    /// pre-#1017 install with no marker at all) — handled by a plain
    /// reinstall, same as today.
    NotInstalled,
}

impl PythonVenvHealth {
    /// Projects this internal diagnosis onto the wire-serializable
    /// [`PythonVenvHealthDto`] returned by the `diagnose_python_venv` IPC
    /// command.
    #[must_use]
    pub fn to_dto(&self) -> PythonVenvHealthDto {
        match self {
            Self::Ok { version } => PythonVenvHealthDto {
                status: PythonVenvHealthStatus::Ok,
                version: Some(version.clone()),
                recorded_interpreter: None,
                recorded_version: None,
                recorded_interpreter_exists: None,
            },
            Self::BrokenSystemVenv {
                recorded_interpreter,
                recorded_version,
                recorded_interpreter_exists,
            } => PythonVenvHealthDto {
                status: PythonVenvHealthStatus::BrokenSystemVenv,
                version: None,
                recorded_interpreter: recorded_interpreter.clone(),
                recorded_version: recorded_version.clone(),
                recorded_interpreter_exists: Some(*recorded_interpreter_exists),
            },
            Self::NotInstalled => PythonVenvHealthDto {
                status: PythonVenvHealthStatus::NotInstalled,
                version: None,
                recorded_interpreter: None,
                recorded_version: None,
                recorded_interpreter_exists: None,
            },
        }
    }
}

/// Discriminant for [`PythonVenvHealthDto::status`], serialized as a
/// lowercase-snake-case string so the frontend can switch on a plain string
/// literal union (`"ok" | "broken_system_venv" | "not_installed"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonVenvHealthStatus {
    Ok,
    BrokenSystemVenv,
    NotInstalled,
}

/// Wire DTO for the `diagnose_python_venv` IPC command.
///
/// A flat, all-fields-present struct (rather than an internally-tagged Rust
/// enum) so every variant serializes to a uniform, predictable JSON shape —
/// straightforward for the frontend to switch on `status` and read whichever
/// of the optional fields apply. Mirrors `PythonVenvHealthDto` in
/// `src/types/index.ts`.
///
/// # Example JSON
/// ```json
/// { "status": "ok", "version": "3.12.8", "recordedInterpreter": null, "recordedVersion": null, "recordedInterpreterExists": null }
/// { "status": "broken_system_venv", "version": null, "recordedInterpreter": "/opt/homebrew/bin/python3.14", "recordedVersion": "3.14.6", "recordedInterpreterExists": false }
/// { "status": "not_installed", "version": null, "recordedInterpreter": null, "recordedVersion": null, "recordedInterpreterExists": null }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonVenvHealthDto {
    pub status: PythonVenvHealthStatus,
    pub version: Option<String>,
    pub recorded_interpreter: Option<String>,
    pub recorded_version: Option<String>,
    pub recorded_interpreter_exists: Option<bool>,
}

/// Diagnoses the health of the managed Python at `{app_data}/python/`.
///
/// # Arguments
/// * `app` - The Tauri app handle, used to resolve the Python directory.
///
/// # Returns
/// See [`PythonVenvHealth`] for the three possible outcomes.
pub async fn diagnose_python_venv(app: &AppHandle) -> PythonVenvHealth {
    diagnose_python_venv_at(&platform::get_python_dir(app)).await
}

/// Path-based core of [`diagnose_python_venv`], split out for direct
/// testability against a `tempfile` directory (mirrors the `_for`/`_at`
/// pattern used elsewhere in this module and in `utils/platform.rs`).
async fn diagnose_python_venv_at(python_dir: &Path) -> PythonVenvHealth {
    let python_bin = platform::resolve_managed_python_binary(python_dir);

    // Binary missing outright: nothing to diagnose beyond "not installed".
    if !python_bin.exists() {
        return PythonVenvHealth::NotInstalled;
    }

    // Binary exists — try to actually run it, exactly like check_python_status.
    match get_python_version_from_binary(&python_bin).await {
        Ok(version) => PythonVenvHealth::Ok { version },
        Err(e) => {
            log::warn!(
                "Python binary exists but failed to run while diagnosing venv health: {e}"
            );
            // The binary exists but won't run. Only a system-venv provenance
            // marker turns this into an explainable, recoverable state —
            // anything else (portable, or no marker) falls back to the
            // existing "not installed" treatment.
            match read_source_marker(python_dir) {
                Some(record) if record.is_system_venv() => {
                    let recorded_interpreter_exists = record
                        .interpreter
                        .as_deref()
                        .map(|p| Path::new(p).is_file())
                        .unwrap_or(false);
                    PythonVenvHealth::BrokenSystemVenv {
                        recorded_interpreter: record.interpreter,
                        recorded_version: Some(record.version),
                        recorded_interpreter_exists,
                    }
                }
                _ => PythonVenvHealth::NotInstalled,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_version() {
        assert_eq!(parse_version_tuple("3.14.6"), Some((3, 14, 6)));
        assert_eq!(parse_version_tuple(" 3.12.8 "), Some((3, 12, 8)));
    }

    #[test]
    fn parses_two_component_version() {
        assert_eq!(parse_version_tuple("3.10"), Some((3, 10, 0)));
    }

    #[test]
    fn tolerates_prerelease_micro_suffix() {
        assert_eq!(parse_version_tuple("3.11.0rc1"), Some((3, 11, 0)));
    }

    #[test]
    fn rejects_garbage_version() {
        assert_eq!(parse_version_tuple(""), None);
        assert_eq!(parse_version_tuple("python"), None);
        assert_eq!(parse_version_tuple("3"), None); // needs at least major.minor
    }

    #[test]
    fn floor_is_310() {
        assert!(version_meets_floor(3, 10));
        assert!(version_meets_floor(3, 12));
        assert!(version_meets_floor(4, 0));
        assert!(!version_meets_floor(3, 9));
        assert!(!version_meets_floor(2, 7));
    }

    #[test]
    fn version_ordering_is_numeric_not_lexical() {
        // "3.9" must sort BELOW "3.10" (lexical string compare would get this wrong).
        assert_eq!(cmp_versions("3.9.0", "3.10.0"), std::cmp::Ordering::Less);
        assert_eq!(cmp_versions("3.14.6", "3.12.8"), std::cmp::Ordering::Greater);
        assert_eq!(cmp_versions("3.12.0", "3.12.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn classifies_provenance_labels() {
        assert_eq!(classify_source("/opt/homebrew/bin/python3.14"), "Homebrew");
        assert_eq!(
            classify_source("/Library/Frameworks/Python.framework/Versions/3.12/bin/python3"),
            "python.org"
        );
        assert_eq!(classify_source("/Users/x/.pyenv/versions/3.11.0/bin/python3"), "pyenv");
        assert_eq!(classify_source("/usr/bin/python3"), "System");
    }

    #[test]
    fn source_record_roundtrips_and_flags_venv() {
        let venv = PythonSourceRecord {
            source: PYTHON_SOURCE_SYSTEM_VENV.to_string(),
            interpreter: Some("/opt/homebrew/bin/python3.14".to_string()),
            version: "3.14.6".to_string(),
        };
        assert!(venv.is_system_venv());
        let json = serde_json::to_string(&venv).unwrap();
        let back: PythonSourceRecord = serde_json::from_str(&json).unwrap();
        assert!(back.is_system_venv());
        assert_eq!(back.interpreter.as_deref(), Some("/opt/homebrew/bin/python3.14"));

        let portable = PythonSourceRecord {
            source: PYTHON_SOURCE_PORTABLE.to_string(),
            interpreter: None,
            version: "3.12.8".to_string(),
        };
        assert!(!portable.is_system_venv());
    }

    // ============================================================
    // diagnose_python_venv_at (#1017 follow-up: broken system-venv detection)
    // ============================================================

    /// Writes a file at the platform-resolved managed-Python binary path that
    /// EXISTS but cannot successfully execute `--version` — simulating a
    /// dangling venv interpreter (e.g. after `brew upgrade python` relocates
    /// the Cellar path the venv's interpreter linked against). Garbage bytes
    /// with default (non-executable) permissions fail to spawn on Unix
    /// (no +x bit) and fail to launch on Windows (not a valid PE image) —
    /// both surface as an `Err` from `Command::output()`, exactly like a real
    /// dangling symlink would.
    fn write_broken_binary(python_dir: &Path) -> PathBuf {
        let bin = platform::resolve_managed_python_binary(python_dir);
        if let Some(parent) = bin.parent() {
            std::fs::create_dir_all(parent).expect("failed to create fake binary's parent dir");
        }
        std::fs::write(&bin, b"not a real interpreter").expect("failed to write fake binary");
        bin
    }

    #[tokio::test]
    async fn diagnose_reports_not_installed_when_binary_missing() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        // Even with a system-venv marker present, a missing binary short-
        // circuits to NotInstalled — existence is checked before the marker.
        write_source_marker(
            tmp.path(),
            &PythonSourceRecord {
                source: PYTHON_SOURCE_SYSTEM_VENV.to_string(),
                interpreter: Some("/opt/homebrew/bin/python3.14".to_string()),
                version: "3.14.6".to_string(),
            },
        );

        let health = diagnose_python_venv_at(tmp.path()).await;
        assert_eq!(health, PythonVenvHealth::NotInstalled);
    }

    #[tokio::test]
    async fn diagnose_reports_broken_system_venv_with_recorded_interpreter_gone() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        write_broken_binary(tmp.path());
        write_source_marker(
            tmp.path(),
            &PythonSourceRecord {
                source: PYTHON_SOURCE_SYSTEM_VENV.to_string(),
                // A plausible-looking but nonexistent path — simulates the
                // post-`brew upgrade python` Cellar relocation.
                interpreter: Some("/opt/homebrew/Cellar/python@3.14/3.14.6/bin/python3.14".to_string()),
                version: "3.14.6".to_string(),
            },
        );

        let health = diagnose_python_venv_at(tmp.path()).await;
        assert_eq!(
            health,
            PythonVenvHealth::BrokenSystemVenv {
                recorded_interpreter: Some(
                    "/opt/homebrew/Cellar/python@3.14/3.14.6/bin/python3.14".to_string()
                ),
                recorded_version: Some("3.14.6".to_string()),
                recorded_interpreter_exists: false,
            }
        );
    }

    #[tokio::test]
    async fn diagnose_reports_recorded_interpreter_exists_true_when_still_present() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        write_broken_binary(tmp.path());

        // A separate file standing in for the still-present system interpreter
        // (only its existence as a file matters to the diagnosis).
        let still_here = tmp.path().join("still-here-python3");
        std::fs::write(&still_here, b"").expect("failed to write stand-in interpreter");

        write_source_marker(
            tmp.path(),
            &PythonSourceRecord {
                source: PYTHON_SOURCE_SYSTEM_VENV.to_string(),
                interpreter: Some(still_here.to_string_lossy().to_string()),
                version: "3.12.0".to_string(),
            },
        );

        let health = diagnose_python_venv_at(tmp.path()).await;
        match health {
            PythonVenvHealth::BrokenSystemVenv {
                recorded_interpreter_exists,
                ..
            } => assert!(recorded_interpreter_exists),
            other => panic!("expected BrokenSystemVenv, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn diagnose_reports_not_installed_for_broken_portable_marker() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        write_broken_binary(tmp.path());
        // A broken PORTABLE install (not a system-venv) has no reusable
        // interpreter to rebuild from — falls back to plain "not installed".
        write_source_marker(
            tmp.path(),
            &PythonSourceRecord {
                source: PYTHON_SOURCE_PORTABLE.to_string(),
                interpreter: None,
                version: "3.12.8".to_string(),
            },
        );

        let health = diagnose_python_venv_at(tmp.path()).await;
        assert_eq!(health, PythonVenvHealth::NotInstalled);
    }

    #[tokio::test]
    async fn diagnose_reports_not_installed_when_binary_broken_and_no_marker() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        // A pre-#1017 portable install has no marker file at all.
        write_broken_binary(tmp.path());

        let health = diagnose_python_venv_at(tmp.path()).await;
        assert_eq!(health, PythonVenvHealth::NotInstalled);
    }

    #[test]
    fn dto_serialises_ok_variant() {
        let dto = PythonVenvHealth::Ok {
            version: "3.12.8".to_string(),
        }
        .to_dto();
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "status": "ok",
                "version": "3.12.8",
                "recordedInterpreter": null,
                "recordedVersion": null,
                "recordedInterpreterExists": null,
            })
        );
    }

    #[test]
    fn dto_serialises_broken_system_venv_variant() {
        let dto = PythonVenvHealth::BrokenSystemVenv {
            recorded_interpreter: Some("/opt/homebrew/bin/python3.14".to_string()),
            recorded_version: Some("3.14.6".to_string()),
            recorded_interpreter_exists: false,
        }
        .to_dto();
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "status": "broken_system_venv",
                "version": null,
                "recordedInterpreter": "/opt/homebrew/bin/python3.14",
                "recordedVersion": "3.14.6",
                "recordedInterpreterExists": false,
            })
        );
    }

    #[test]
    fn dto_serialises_not_installed_variant() {
        let dto = PythonVenvHealth::NotInstalled.to_dto();
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "status": "not_installed",
                "version": null,
                "recordedInterpreter": null,
                "recordedVersion": null,
                "recordedInterpreterExists": null,
            })
        );
    }
}
