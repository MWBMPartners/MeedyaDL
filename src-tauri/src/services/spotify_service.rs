// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Spotify download service (votify wrapper).
// =============================================
//
// Mirrors `gamdl_service.rs`: installs / upgrades the `votify` PyPI
// package into MeedyaDL's managed Python runtime, probes its version
// (and feeds the shared capability cache in `votify_capabilities.rs`),
// builds the votify CLI command for a download request, and streams
// stdout/stderr via the engine-runner's generic line-reader.
//
// ## Subprocess shape
//
// Every votify invocation is launched as `python -m votify <urls> <flags>`,
// using the managed Python resolved via `platform::resolve_managed_python_binary`
// (venv-aware — see #1017: a reused system Python is provisioned as a venv,
// which on Windows puts `python.exe` under `Scripts/` rather than at the
// portable root). This guarantees we use the managed Python (not a system
// one) and avoids PATH lookups for the `votify` console-script entry point.
//
// ## Anti-ban posture in this file
//
// Wave 1 (M9-2 — this PR): wires the subprocess. Adds no rate limit
// of its own. The flagship anti-ban knob (`--wait-interval`) is
// already wired through `VotifyOptions`, so M9-4 only has to plumb
// it into the queue layer and Settings UI without another service
// change.
//
// Wave 2 (M9-4 — separate PR): real-time playback-speed throttle,
// daily download cap, dev-access gate, first-run modal.
//
// ## References
//
// * votify CLI: <https://github.com/glomatico/votify>
// * PyPI JSON API: <https://warehouse.pypa.io/api-reference/json.html>

use tauri::AppHandle;
use tokio::process::Command;

use crate::models::votify_options::VotifyOptions;
use crate::services::dependency_manager;
use crate::utils::platform;

// ============================================================
// pip — install
// ============================================================

/// Install (or upgrade) the votify Python package via pip.
///
/// Behaviour mirrors [`crate::services::gamdl_service::install_gamdl`]:
///
/// * `target_version = None` → bounded support-window spec
///   (e.g. `votify>=1.9.0,<=1.9.9`). Routine setup-wizard /
///   "Upgrade tested" path.
/// * `target_version = Some(v)` → exact pin (`votify==v`). Used when
///   the user has opted into installing an above-ceiling
///   "Untested" release from the Updates page.
///
/// After install completes the function probes the resolved version
/// via `get_votify_version`, which also feeds the shared capability
/// cache so every consumer (CLI-arg builder, etc.) sees the new
/// version immediately.
///
/// # Errors
///
/// * Python isn't installed (run the setup wizard first).
/// * `pip install` exits non-zero (stderr is surfaced to the caller).
/// * Post-install version probe fails.
pub async fn install_votify(
    app: &AppHandle,
    target_version: Option<&str>,
) -> Result<String, String> {
    log::info!("Installing votify via pip...");

    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix) — a reused system Python is
    // provisioned as a venv, which on Windows puts `python.exe` under
    // `Scripts/` rather than at the portable root. Using the pure
    // `get_python_binary_path` here made votify installs silently fail
    // with "Python is not installed" on Windows + system-venv setups.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Err(
            "Cannot install votify: Python is not installed. Run the setup wizard first."
                .to_string(),
        );
    }

    let pip_spec = match target_version {
        Some(v) => super::votify_capabilities::pip_target_spec(v),
        None => super::votify_capabilities::pip_version_spec(),
    };
    log::info!("Installing votify with version spec: {pip_spec}");

    let output = Command::new(&python_bin)
        .args(["-m", "pip", "install", "--upgrade", &pip_spec])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pip install votify failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install output: {}", stdout.trim());

    // Post-install probe — also refreshes the capability cache.
    let version = get_votify_version(app)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    log::info!("votify {version} installed and verified successfully");
    Ok(version)
}

// ============================================================
// pip — version probe
// ============================================================

/// Returns the installed votify version, or `Ok(None)` when not installed.
///
/// Runs `python -m pip show votify` with a 10-second timeout — same
/// guardrail GAMDL's probe uses. Surfaces both possible "not installed"
/// signals: pip exits with `Ok(None)` AND a missing `Version:` line in
/// the captured stdout.
///
/// As a side effect this refreshes
/// [`super::votify_capabilities::set_detected_version`], so callers
/// querying [`super::votify_capabilities::supports`] see the new
/// value immediately after this returns.
///
/// # Errors
///
/// * Python isn't installed and resolvable → `Ok(None)` (votify can't
///   possibly be installed in that case).
/// * `pip show` failed to spawn / timed out → `Err(_)`.
pub async fn get_votify_version(app: &AppHandle) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix) — see `install_votify` above.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Ok(None);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(&python_bin)
            .args(["-m", "pip", "show", "votify"])
            .output(),
    )
    .await
    .map_err(|_| {
        "pip show votify timed out (10s) — Python environment may be unresponsive".to_string()
    })?
    .map_err(|e| format!("Failed to run pip show: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let version = stdout
        .lines()
        .find(|line| line.starts_with("Version:"))
        .map(|line| line.trim_start_matches("Version:").trim().to_string());

    // Keep the shared capability cache in sync.
    super::votify_capabilities::set_detected_version(version.clone());

    Ok(version)
}

// ============================================================
// PyPI — latest available
// ============================================================

/// Returns the latest votify version published to PyPI.
///
/// Mirrors `check_latest_gamdl_version`. Uses the public PyPI JSON API
/// — no authentication required.
///
/// # Errors
///
/// * Network failure or non-2xx response.
/// * Response body not valid JSON.
/// * The `info.version` field is missing or non-string.
pub async fn check_latest_votify_version() -> Result<String, String> {
    let url = "https://pypi.org/pypi/votify/json";
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to check PyPI: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("PyPI returned HTTP {}", response.status()));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PyPI response: {e}"))?;

    json["info"]["version"]
        .as_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| "Could not find version in PyPI response".to_string())
}

// ============================================================
// Subprocess builder
// ============================================================

/// Public wrapper around the internal command builder.
///
/// Exposed so [`crate::services::engine_runner::VotifyCommandBuilder`]
/// can build the command without having to duplicate the construction
/// logic. Kept as a separate symbol from `build_votify_command` so the
/// internal builder stays free to evolve (e.g. add private helpers)
/// without breaking external callers.
///
/// # Errors
///
/// Returns `Err(String)` if the Python binary path cannot be resolved
/// or any URL fails the `https?://` prefix check.
pub fn build_votify_command_public(
    app: &AppHandle,
    urls: &[String],
    options: &VotifyOptions,
) -> Result<Command, String> {
    build_votify_command(app, urls, options)
}

/// Build the full votify CLI command for a download request.
///
/// Constructs a `tokio::process::Command` that runs:
/// `{python} -m votify {urls...} {--option value...}`
///
/// Tool-path auto-injection follows the same shape as gamdl_service:
/// when an executable path option is unset and a managed binary exists
/// under MeedyaDL's tools directory, the path is injected so users
/// don't have to configure it manually.
fn build_votify_command(
    app: &AppHandle,
    urls: &[String],
    options: &VotifyOptions,
) -> Result<Command, String> {
    let python_dir = platform::get_python_dir(app);
    // Venv-aware resolver (#1017 / A3 fix) — see `install_votify` above.
    // This call site matters most in practice: it's the one that builds
    // the actual `python -m votify` subprocess for every real Spotify
    // download, so a Windows + system-venv user would have every download
    // fail at this exact check without the fix.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Err("Python is not installed. Run the setup wizard first.".to_string());
    }

    let mut cmd = Command::new(&python_bin);
    cmd.args(["-m", "votify"]);

    // URLs are positional arguments. Validate the `https?://` prefix —
    // same rationale as gamdl_service: prevents an attacker-supplied
    // string starting with `--` from being misparsed as a flag, and
    // blocks non-HTTP schemes from reaching the subprocess.
    for url in urls {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(format!(
                "Invalid URL (must start with http:// or https://): {url}"
            ));
        }
        cmd.arg(url);
    }

    // Marshal the typed options into CLI args. Order grouping is
    // documented in `VotifyOptions::to_cli_args`.
    let cli_args = options.to_cli_args();
    cmd.args(&cli_args);

    // Inject managed tool paths when the user hasn't specified custom
    // ones. Mirrors gamdl_service's auto-injection — keeps the
    // out-of-the-box experience working without any tool-path config.
    inject_tool_paths(app, &mut cmd, options);

    // Redact wrapper-style credentials before logging. votify itself
    // doesn't currently support a wrapper account URL, but cookies
    // path could carry sensitive content if a future power-user feature
    // surfaces a tokenised path; redacting at the activity-log boundary
    // costs nothing and keeps us aligned with the gamdl service.
    let display_args: Vec<String> = cli_args.iter().map(|s| redact_cli_value(s)).collect();
    log::info!(
        "votify command: python -m votify {} {}",
        urls.join(" "),
        display_args.join(" ")
    );

    Ok(cmd)
}

/// Append managed-tool paths to the command when not explicitly set.
///
/// Mirrors `gamdl_service::inject_tool_paths`. votify accepts the same
/// `--{tool}-path` shape for FFmpeg, MP4Box, mp4decrypt — so we share
/// the user's already-configured tool installations rather than asking
/// votify to find them on PATH.
fn inject_tool_paths(app: &AppHandle, cmd: &mut Command, options: &VotifyOptions) {
    // FFmpeg
    if options.ffmpeg_path.is_none() {
        let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
        if ffmpeg_bin.exists() {
            cmd.arg("--ffmpeg-path").arg(&ffmpeg_bin);
        }
    }
    // MP4Box
    if options.mp4box_path.is_none() {
        let mp4box_bin = dependency_manager::get_tool_binary_path(app, "mp4box");
        if mp4box_bin.exists() {
            cmd.arg("--mp4box-path").arg(&mp4box_bin);
        }
    }
    // mp4decrypt
    if options.mp4decrypt_path.is_none() {
        let mp4decrypt_bin = dependency_manager::get_tool_binary_path(app, "mp4decrypt");
        if mp4decrypt_bin.exists() {
            cmd.arg("--mp4decrypt-path").arg(&mp4decrypt_bin);
        }
    }
}

/// Redact secrets from a CLI argument value before it lands in
/// activity-log output.
///
/// Currently a passthrough — there is no votify flag known to carry a
/// secret in its value. Kept as an extension point so adding (e.g.) a
/// `wrapper_account_url`-style option later only touches this function.
fn redact_cli_value(arg: &str) -> String {
    arg.to_string()
}

// ============================================================
// run_votify — dispatch through the generic engine runner
// ============================================================

/// Spawn votify for a download request and stream output to the frontend.
///
/// Builds the command, then delegates to
/// [`crate::services::engine_runner::run_engine`] which owns the
/// stdout/stderr line readers and emits parsed events on both the
/// generic `"engine-output"` channel and the legacy `"gamdl-output"`
/// alias that the existing frontend still subscribes to.
///
/// # Errors
///
/// * The command can't be built (Python missing, URL fails prefix check).
/// * The process exits with a non-zero status.
pub async fn run_votify(
    app: &AppHandle,
    download_id: &str,
    urls: &[String],
    options: &VotifyOptions,
) -> Result<(), String> {
    log::info!(
        "Starting votify download {} for {} URL(s)",
        download_id,
        urls.len()
    );
    let cmd = build_votify_command(app, urls, options)?;
    crate::services::engine_runner::run_engine(app, download_id, "votify", cmd).await
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_cli_value_passthrough() {
        // Current contract: passthrough. If a future flag adds secret
        // handling, this test gets a sibling that exercises the new path.
        assert_eq!(redact_cli_value("--audio-quality"), "--audio-quality");
        assert_eq!(redact_cli_value("vorbis-high"), "vorbis-high");
    }

    #[tokio::test]
    async fn check_latest_votify_version_returns_a_version_or_network_err() {
        // Best-effort integration check — succeeds on a live network,
        // returns an obviously-shaped error string otherwise. We only
        // assert the SHAPE of the success path (semver-ish string), not
        // a specific value, so this stays stable across votify releases.
        match check_latest_votify_version().await {
            Ok(v) => assert!(
                v.chars().next().is_some_and(|c| c.is_ascii_digit()),
                "PyPI returned a non-version-shaped string: {v}"
            ),
            Err(e) => assert!(
                e.contains("PyPI") || e.contains("Failed"),
                "Network-style error expected, got: {e}"
            ),
        }
    }
}
