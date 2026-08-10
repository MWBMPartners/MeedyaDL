// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Package-manager abstraction.
//
// MeedyaDL prefers reusing an external tool (FFmpeg, mp4decrypt, MP4Box, …)
// that the user already installed via a system package manager, instead of
// downloading a second managed copy (see #1081 + the 2026-08-10 follow-up).
// Detection + in-place reuse already work; this module generalises the
// previously Homebrew-only *attribution* and *update-routing* so the same
// pattern covers every package manager MeedyaDL can recognise.
//
// ## What this module owns
//
// - [`PackageManagerKind`] — the closed set of package managers MeedyaDL can
//   attribute an install to and (where safe) drive an update through.
// - [`PackageRef`] — a `(package-manager, package-name)` pair. Serialises
//   to/from the per-tool `.source` marker written by `dependency_manager`
//   (`homebrew:ffmpeg`, `pipx:gamdl`, `apt:ffmpeg`, …).
// - [`detect_owner`] — given a binary path, work out which package manager
//   owns it (Homebrew formula, pipx venv, dpkg/rpm package, …).
// - [`PackageManagerKind::upgrade`] — update a package *through its owning
//   manager*. No-elevation managers (Homebrew, pipx, Scoop) run directly;
//   root-requiring managers (apt, dnf, snap, MacPorts) run through the same
//   non-interactive elevation tiers as the #997 installer (`sudo -n` →
//   `pkexec` → actionable error), so an update can never hang a TTY-less
//   desktop process.
//
// ## Security invariants (mirrored from `dependency_manager`)
//
// - Every subprocess is a fixed-argv `Command::new().arg()` — never `sh -c`,
//   never a string-built command line.
// - A package name only ever originates from package-manager output or from a
//   component of a filesystem path, and is validated by
//   [`is_safe_package_name`] (no separators, whitespace, or leading `-`) before
//   it can reach an `upgrade` argv — so it can never be mistaken for a flag.
// - No `Command::env` mutation; the only environment reads are of MeedyaDL's
//   own process (`HOME`, `PIPX_HOME`, `SCOOP`, `USERPROFILE`, `LOCALAPPDATA`).

use std::path::{Path, PathBuf};

use crate::services::dependency_manager;

/// The closed set of package managers MeedyaDL can attribute an install to.
///
/// Deliberately an `enum` (not a trait object): the set is small, closed, and
/// platform-partitioned, so exhaustive `match` forces every call site to make
/// a decision when a new manager is added, and there is no `async fn`-in-`dyn`
/// friction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManagerKind {
    /// macOS + Linuxbrew. No elevation to upgrade.
    Homebrew,
    /// macOS MacPorts. Querying is free; mutation needs root.
    MacPorts,
    /// pipx (all platforms). Per-user; no elevation to upgrade.
    Pipx,
    /// Scoop (Windows). Per-user; no elevation to upgrade.
    Scoop,
    /// Debian/Ubuntu apt/dpkg. Querying is free; mutation needs root.
    Apt,
    /// Fedora/RHEL dnf/rpm. Querying is free; mutation needs root.
    Dnf,
    /// Linux snap. Mutation needs root (snaps usually self-refresh anyway).
    Snap,
}

/// Whether MeedyaDL may auto-invoke an update for a manager, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCapability {
    /// Per-user, no elevation required (Homebrew, pipx, Scoop) — run directly.
    Auto,
    /// Root required. Auto-invoked through the non-interactive elevation tiers
    /// (`sudo -n` → `pkexec` → actionable error), never a bare `sudo`.
    Elevated,
}

/// A package-manager-attributed package. Round-trips through the `.source`
/// marker grammar (`<prefix>:<package>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRef {
    /// The owning package manager.
    pub pm: PackageManagerKind,
    /// The package/formula/port name as the manager knows it.
    pub package: String,
}

impl PackageManagerKind {
    /// Every recognised marker prefix, for parse/serialise round-tripping.
    const ALL: [PackageManagerKind; 7] = [
        PackageManagerKind::Homebrew,
        PackageManagerKind::MacPorts,
        PackageManagerKind::Pipx,
        PackageManagerKind::Scoop,
        PackageManagerKind::Apt,
        PackageManagerKind::Dnf,
        PackageManagerKind::Snap,
    ];

    /// The lowercase prefix used in the `.source` marker (`homebrew:ffmpeg`).
    #[must_use]
    pub fn marker_prefix(self) -> &'static str {
        match self {
            PackageManagerKind::Homebrew => "homebrew",
            PackageManagerKind::MacPorts => "macports",
            PackageManagerKind::Pipx => "pipx",
            PackageManagerKind::Scoop => "scoop",
            PackageManagerKind::Apt => "apt",
            PackageManagerKind::Dnf => "dnf",
            PackageManagerKind::Snap => "snap",
        }
    }

    /// The human-readable label shown in the setup wizard badge / Updates page.
    #[must_use]
    pub fn display_label(self) -> &'static str {
        match self {
            PackageManagerKind::Homebrew => "Homebrew",
            PackageManagerKind::MacPorts => "MacPorts",
            PackageManagerKind::Pipx => "pipx",
            PackageManagerKind::Scoop => "Scoop",
            PackageManagerKind::Apt => "APT",
            PackageManagerKind::Dnf => "DNF",
            PackageManagerKind::Snap => "Snap",
        }
    }

    /// Whether MeedyaDL may auto-invoke an upgrade, and whether it needs
    /// elevation to do so.
    #[must_use]
    pub fn update_capability(self) -> UpdateCapability {
        match self {
            PackageManagerKind::Homebrew
            | PackageManagerKind::Pipx
            | PackageManagerKind::Scoop => UpdateCapability::Auto,
            PackageManagerKind::MacPorts
            | PackageManagerKind::Apt
            | PackageManagerKind::Dnf
            | PackageManagerKind::Snap => UpdateCapability::Elevated,
        }
    }

    /// The exact command a user could run themselves to upgrade `package`.
    /// Shown in the UI as transparency and surfaced as the fallback when an
    /// elevated upgrade cannot obtain privileges. `{pkg}` is substituted.
    #[must_use]
    pub fn manual_command(self, package: &str) -> String {
        match self {
            PackageManagerKind::Homebrew => format!("brew upgrade {package}"),
            PackageManagerKind::MacPorts => format!("sudo port upgrade {package}"),
            PackageManagerKind::Pipx => format!("pipx upgrade {package}"),
            PackageManagerKind::Scoop => format!("scoop update {package}"),
            PackageManagerKind::Apt => format!("sudo apt install --only-upgrade {package}"),
            PackageManagerKind::Dnf => format!("sudo dnf upgrade {package}"),
            PackageManagerKind::Snap => format!("sudo snap refresh {package}"),
        }
    }

    /// For an [`UpdateCapability::Elevated`] manager, the program and argument
    /// vector (excluding the `sudo -n` / `pkexec` prefix) that performs a
    /// non-interactive upgrade of `package`. Returns `None` for `Auto`
    /// managers (which never elevate).
    fn elevated_upgrade_argv(self, package: &str) -> Option<(&'static str, Vec<String>)> {
        let owned = |parts: &[&str]| parts.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        match self {
            PackageManagerKind::Apt => Some((
                "apt-get",
                owned(&["install", "-y", "--only-upgrade", package]),
            )),
            PackageManagerKind::Dnf => Some(("dnf", owned(&["upgrade", "-y", package]))),
            PackageManagerKind::Snap => Some(("snap", owned(&["refresh", package]))),
            PackageManagerKind::MacPorts => Some(("port", owned(&["upgrade", package]))),
            PackageManagerKind::Homebrew
            | PackageManagerKind::Pipx
            | PackageManagerKind::Scoop => None,
        }
    }

    /// Locate the package-manager driver binary, even when a desktop launch
    /// did not inherit the user's shell PATH.
    pub async fn locate(self) -> Option<PathBuf> {
        match self {
            PackageManagerKind::Homebrew => find_homebrew(),
            PackageManagerKind::Pipx => locate_on_path_or(&["pipx"], &[]).await,
            PackageManagerKind::Scoop => {
                // Scoop's shim is `scoop.cmd`/`scoop` on PATH, or under the
                // per-user scoop install.
                let mut extra: Vec<PathBuf> = Vec::new();
                for root in scoop_roots() {
                    extra.push(root.join("shims").join("scoop.cmd"));
                    extra.push(root.join("shims").join("scoop.ps1"));
                }
                locate_on_path_or(&["scoop", "scoop.cmd"], &extra).await
            }
            PackageManagerKind::Apt => locate_on_path_or(&["apt-get"], &[]).await,
            PackageManagerKind::Dnf => locate_on_path_or(&["dnf"], &[]).await,
            PackageManagerKind::Snap => locate_on_path_or(&["snap"], &[]).await,
            PackageManagerKind::MacPorts => {
                locate_on_path_or(&["port"], &[PathBuf::from("/opt/local/bin/port")]).await
            }
        }
    }

    /// Work out whether this manager owns `binary` (already canonicalised by
    /// the caller). Returns the [`PackageRef`] on a match.
    async fn owner_of(self, binary: &Path, original: &Path) -> Option<PackageRef> {
        match self {
            PackageManagerKind::Homebrew => find_homebrew_owner(binary)
                .await
                .map(|(_, formula)| PackageRef::new(self, formula)),
            PackageManagerKind::Pipx => {
                pkg_after_component(binary, "venvs").map(|pkg| PackageRef::new(self, pkg))
            }
            PackageManagerKind::Scoop => scoop_owner(original).map(|pkg| PackageRef::new(self, pkg)),
            PackageManagerKind::Snap => snap_owner(original).map(|pkg| PackageRef::new(self, pkg)),
            PackageManagerKind::Apt => dpkg_owner(binary).await.map(|pkg| PackageRef::new(self, pkg)),
            PackageManagerKind::Dnf => rpm_owner(binary).await.map(|pkg| PackageRef::new(self, pkg)),
            PackageManagerKind::MacPorts => macports_owner(binary)
                .await
                .map(|pkg| PackageRef::new(self, pkg)),
        }
    }

    /// Upgrade `pkg` through this manager. `Auto` managers run directly;
    /// `Elevated` managers run through the non-interactive elevation tiers.
    ///
    /// # Errors
    /// Returns an actionable message if the manager binary is missing, the
    /// upgrade command fails, or (for `Elevated` managers) no non-interactive
    /// elevation path is available — in which case the message carries the
    /// exact command the user can run themselves.
    pub async fn upgrade(self, pkg: &PackageRef) -> Result<(), String> {
        if !is_safe_package_name(&pkg.package) {
            return Err(format!("Refusing to upgrade unsafe package name: {}", pkg.package));
        }
        match self.update_capability() {
            UpdateCapability::Auto => self.upgrade_auto(&pkg.package).await,
            UpdateCapability::Elevated => self.upgrade_elevated(&pkg.package).await,
        }
    }

    /// No-elevation upgrade (Homebrew / pipx / Scoop).
    async fn upgrade_auto(self, package: &str) -> Result<(), String> {
        let driver = self
            .locate()
            .await
            .ok_or_else(|| format!("{} is not available on this system", self.display_label()))?;
        let args: Vec<&str> = match self {
            PackageManagerKind::Homebrew => vec!["upgrade", package],
            PackageManagerKind::Pipx => vec!["upgrade", package],
            PackageManagerKind::Scoop => vec!["update", package],
            _ => return Err("not an auto-updatable package manager".to_string()),
        };
        let output = tokio::process::Command::new(&driver)
            .args(&args)
            .output()
            .await
            .map_err(|e| format!("Failed to run {}: {e}", self.display_label()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{} could not update {package}: {}",
                self.display_label(),
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    /// Elevated upgrade (apt / dnf / snap / MacPorts) via the #997 tiers:
    /// passwordless `sudo -n` → graphical `pkexec` → actionable error.
    async fn upgrade_elevated(self, package: &str) -> Result<(), String> {
        let (program, args) = self
            .elevated_upgrade_argv(package)
            .ok_or_else(|| "not an elevated package manager".to_string())?;

        // The manager binary must exist before we bother elevating.
        if self.locate().await.is_none() {
            return Err(format!(
                "{} is not available on this system. To update {package}, run: {}",
                self.display_label(),
                self.manual_command(package)
            ));
        }

        let manual = self.manual_command(package);
        let elevation_error = || {
            format!(
                "Could not update {package} automatically: no non-interactive privilege \
                 elevation is available (no cached sudo credentials and no graphical \
                 PolicyKit prompt). Please run: {manual}"
            )
        };

        let output = if dependency_manager::can_sudo_without_password().await {
            let mut full: Vec<String> = vec!["-n".to_string(), program.to_string()];
            full.extend(args);
            log::info!("Running: sudo -n {program} … (update {package})");
            tokio::process::Command::new("sudo")
                .args(&full)
                .output()
                .await
                .map_err(|e| format!("Failed to run 'sudo -n {program}': {e}"))?
        } else if let Some(pkexec) = dependency_manager::find_pkexec().await {
            let mut full: Vec<String> = vec![program.to_string()];
            full.extend(args);
            log::info!("Running: pkexec {program} … (update {package}, via {pkexec})");
            tokio::process::Command::new("pkexec")
                .args(&full)
                .output()
                .await
                .map_err(|e| format!("Failed to run 'pkexec {program}': {e}"))?
        } else {
            return Err(elevation_error());
        };

        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{} could not update {package}: {}. You can run it yourself: {manual}",
                self.display_label(),
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

impl PackageRef {
    /// Construct a reference, trimming the package name.
    #[must_use]
    pub fn new(pm: PackageManagerKind, package: impl Into<String>) -> Self {
        PackageRef {
            pm,
            package: package.into().trim().to_string(),
        }
    }

    /// Serialise to the `.source` marker form (`pipx:gamdl`).
    #[must_use]
    pub fn to_marker(&self) -> String {
        format!("{}:{}", self.pm.marker_prefix(), self.package)
    }

    /// Parse a `.source` marker value. Returns `None` for `managed`, `system`,
    /// the empty string, an unknown prefix, or an unsafe package name — all of
    /// which callers must treat as a generic (non-PM-routable) system install.
    #[must_use]
    pub fn parse_marker(s: &str) -> Option<PackageRef> {
        let s = s.trim();
        let (prefix, package) = s.split_once(':')?;
        if !is_safe_package_name(package) {
            return None;
        }
        let pm = PackageManagerKind::ALL
            .into_iter()
            .find(|pm| pm.marker_prefix() == prefix)?;
        Some(PackageRef::new(pm, package))
    }

    /// The exact command a user could run to upgrade this package.
    #[must_use]
    pub fn manual_update_command(&self) -> String {
        self.pm.manual_command(&self.package)
    }
}

/// Given a binary path, determine which package manager owns it.
///
/// The cascade runs cheap pure-path classifiers first (pipx / Scoop / snap),
/// then the subprocess-heavy Homebrew owner lookup, then the cheap Linux
/// dpkg/rpm queries, then MacPorts. The first match wins — a binary lives
/// under exactly one manager's tree, so ordering is about cost, not
/// correctness. The path is canonicalised once so a symlink
/// (`/usr/local/bin/ffmpeg` → Cellar) resolves to its real owner.
pub async fn detect_owner(binary: &Path) -> Option<PackageRef> {
    let canonical = std::fs::canonicalize(binary).unwrap_or_else(|_| binary.to_path_buf());

    // Pure-path classifiers (no subprocess). `original` matters for snap/scoop
    // whose wrapper symlinks would otherwise canonicalise away the package.
    for pm in [
        PackageManagerKind::Pipx,
        PackageManagerKind::Scoop,
        PackageManagerKind::Snap,
    ] {
        if let Some(r) = pm.owner_of(&canonical, binary).await {
            return Some(r);
        }
    }

    // Homebrew (subprocess-heavy: N × `brew --prefix`), then Linux dpkg/rpm,
    // then MacPorts.
    for pm in [
        PackageManagerKind::Homebrew,
        PackageManagerKind::Apt,
        PackageManagerKind::Dnf,
        PackageManagerKind::MacPorts,
    ] {
        if let Some(r) = pm.owner_of(&canonical, binary).await {
            return Some(r);
        }
    }

    None
}

/// A package name is safe to pass to an `upgrade` argv iff it is non-empty,
/// carries no path separator or whitespace, and does not start with `-` (which
/// a package manager could misread as a flag). Names only ever come from PM
/// output or path components, so this is defence-in-depth.
#[must_use]
pub fn is_safe_package_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains(['/', '\\', ' ', '\t', '\n', '\r', ';', '&', '|', '$', '`'])
}

// ------------------------------------------------------------------
// Homebrew (moved verbatim from dependency_manager as the Homebrew arm)
// ------------------------------------------------------------------

/// Locate Homebrew even when a desktop launch does not inherit the user's
/// shell PATH. Linuxbrew is supported as well as both macOS prefixes.
fn find_homebrew() -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("brew"))
            .find(|candidate| candidate.is_file())
    });
    from_path.or_else(|| {
        [
            "/opt/homebrew/bin/brew",
            "/usr/local/bin/brew",
            "/home/linuxbrew/.linuxbrew/bin/brew",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
    })
}

fn homebrew_formulae(output: &str) -> impl Iterator<Item = &str> {
    output.lines().map(str::trim).filter(|line| !line.is_empty())
}

/// Determine which installed formula owns a binary by comparing canonical
/// Cellar prefixes. Also detects alternatives such as `ffmpeg-full` and
/// automatically applies to tools added to the dependency catalogue later.
async fn find_homebrew_owner(binary: &Path) -> Option<(PathBuf, String)> {
    let brew = find_homebrew()?;
    let list = tokio::process::Command::new(&brew)
        .args(["list", "--formula", "-1"])
        .output()
        .await
        .ok()?;
    if !list.status.success() {
        return None;
    }

    let binary = std::fs::canonicalize(binary).unwrap_or_else(|_| binary.to_path_buf());
    let formulae = String::from_utf8_lossy(&list.stdout);
    for formula in homebrew_formulae(&formulae) {
        let output = tokio::process::Command::new(&brew)
            .args(["--prefix", formula])
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let prefix = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        let prefix = std::fs::canonicalize(&prefix).unwrap_or(prefix);
        if binary.starts_with(prefix) {
            return Some((brew, formula.to_string()));
        }
    }
    None
}

// ------------------------------------------------------------------
// Pure-path classifiers
// ------------------------------------------------------------------

/// Extract the path component immediately following `marker` (e.g. the pipx
/// package name after a `venvs` segment: `…/venvs/<pkg>/bin/gamdl` → `<pkg>`).
fn pkg_after_component(path: &Path, marker: &str) -> Option<String> {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    let idx = comps.iter().position(|c| c == marker)?;
    let pkg = comps.get(idx + 1)?;
    is_safe_package_name(pkg).then(|| pkg.clone())
}

/// Scoop: a shimmed binary lives under `…/scoop/apps/<pkg>/current/<bin>`.
fn scoop_owner(path: &Path) -> Option<String> {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    // Require a "scoop" segment somewhere above an "apps" segment.
    let scoop_idx = comps.iter().position(|c| c.eq_ignore_ascii_case("scoop"))?;
    let apps_idx = comps
        .iter()
        .position(|c| c.eq_ignore_ascii_case("apps"))
        .filter(|&i| i > scoop_idx)?;
    let pkg = comps.get(apps_idx + 1)?;
    is_safe_package_name(pkg).then(|| pkg.clone())
}

/// snap: an app binary lives under `/snap/<pkg>/…`; the wrapper on PATH is
/// `/snap/bin/<name>` (best-effort: `<name>` is usually the snap name).
fn snap_owner(path: &Path) -> Option<String> {
    let comps: Vec<String> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    let snap_idx = comps.iter().position(|c| c == "snap")?;
    let next = comps.get(snap_idx + 1)?;
    let pkg = if next == "bin" {
        comps.get(snap_idx + 2)?
    } else {
        next
    };
    is_safe_package_name(pkg).then(|| pkg.clone())
}

// ------------------------------------------------------------------
// Subprocess classifiers (Linux dpkg/rpm, MacPorts)
// ------------------------------------------------------------------

async fn dpkg_owner(binary: &Path) -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let out = tokio::process::Command::new("dpkg")
        .arg("-S")
        .arg(binary)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // Format: "pkg: /path/to/binary" (or "pkg:arch: /path").
    let line = String::from_utf8_lossy(&out.stdout);
    let pkg = line.split(':').next()?.trim().to_string();
    is_safe_package_name(&pkg).then_some(pkg)
}

async fn rpm_owner(binary: &Path) -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let out = tokio::process::Command::new("rpm")
        .args(["-qf", "--qf", "%{NAME}"])
        .arg(binary)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let pkg = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // rpm prints "file … is not owned by any package" to stderr with a
    // non-zero exit, so success + a clean name is the guard.
    is_safe_package_name(&pkg).then_some(pkg)
}

async fn macports_owner(binary: &Path) -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    // Cheap prefix gate before spawning `port`.
    if !binary.starts_with("/opt/local/") {
        return None;
    }
    let port = PackageManagerKind::MacPorts.locate().await?;
    let out = tokio::process::Command::new(&port)
        .arg("provides")
        .arg(binary)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // Format: "<path> is provided by: <port>".
    let text = String::from_utf8_lossy(&out.stdout);
    let pkg = text.rsplit_once("is provided by:")?.1.trim().to_string();
    is_safe_package_name(&pkg).then_some(pkg)
}

// ------------------------------------------------------------------
// Locator helpers
// ------------------------------------------------------------------

/// Locate a driver binary via `which`/`where` (PATH), falling back to a list
/// of fixed absolute candidates. Returns the first existing candidate.
async fn locate_on_path_or(names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    for name in names {
        if let Ok(out) = tokio::process::Command::new(which_cmd)
            .arg(name)
            .output()
            .await
        {
            if out.status.success() {
                if let Some(line) = String::from_utf8_lossy(&out.stdout).lines().next() {
                    let p = PathBuf::from(line.trim());
                    if p.is_absolute() && p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }
    extra.iter().find(|p| p.is_file()).cloned()
}

/// Candidate Scoop install roots (Windows), from our own process env.
fn scoop_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(scoop) = std::env::var_os("SCOOP") {
        roots.push(PathBuf::from(scoop));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(profile).join("scoop"));
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_round_trips() {
        for (marker, pm, pkg) in [
            ("homebrew:ffmpeg", PackageManagerKind::Homebrew, "ffmpeg"),
            ("pipx:gamdl", PackageManagerKind::Pipx, "gamdl"),
            ("apt:ffmpeg", PackageManagerKind::Apt, "ffmpeg"),
            ("dnf:mediainfo", PackageManagerKind::Dnf, "mediainfo"),
            ("scoop:ffmpeg", PackageManagerKind::Scoop, "ffmpeg"),
            ("snap:ffmpeg", PackageManagerKind::Snap, "ffmpeg"),
            ("macports:ffmpeg", PackageManagerKind::MacPorts, "ffmpeg"),
        ] {
            let parsed = PackageRef::parse_marker(marker).expect("should parse");
            assert_eq!(parsed.pm, pm);
            assert_eq!(parsed.package, pkg);
            assert_eq!(parsed.to_marker(), marker);
        }
    }

    #[test]
    fn parse_marker_rejects_non_pm_and_unsafe() {
        // Non-PM markers written by the download/adoption paths.
        assert!(PackageRef::parse_marker("managed").is_none());
        assert!(PackageRef::parse_marker("system").is_none());
        assert!(PackageRef::parse_marker("").is_none());
        // Unknown prefix (e.g. a future MeedyaDL's marker).
        assert!(PackageRef::parse_marker("winget:ffmpeg").is_none());
        // Unsafe package names can never round-trip into an upgrade argv.
        assert!(PackageRef::parse_marker("apt:-rf").is_none());
        assert!(PackageRef::parse_marker("apt:a b").is_none());
        assert!(PackageRef::parse_marker("apt:../evil").is_none());
    }

    #[test]
    fn update_capability_table_is_exhaustive_and_correct() {
        // Auto = the no-elevation, per-user managers; everything else Elevated.
        for pm in PackageManagerKind::ALL {
            let expected = matches!(
                pm,
                PackageManagerKind::Homebrew
                    | PackageManagerKind::Pipx
                    | PackageManagerKind::Scoop
            );
            assert_eq!(
                pm.update_capability() == UpdateCapability::Auto,
                expected,
                "{pm:?}"
            );
        }
    }

    #[test]
    fn is_safe_package_name_guards() {
        assert!(is_safe_package_name("ffmpeg"));
        assert!(is_safe_package_name("ffmpeg-full"));
        assert!(is_safe_package_name("N_m3u8DL-RE"));
        assert!(!is_safe_package_name("-rf"));
        assert!(!is_safe_package_name(""));
        assert!(!is_safe_package_name("a/b"));
        assert!(!is_safe_package_name("a b"));
        assert!(!is_safe_package_name("a;b"));
        assert!(!is_safe_package_name("$(x)"));
    }

    #[test]
    fn manual_commands_render() {
        assert_eq!(
            PackageManagerKind::Homebrew.manual_command("ffmpeg"),
            "brew upgrade ffmpeg"
        );
        assert_eq!(
            PackageManagerKind::Apt.manual_command("ffmpeg"),
            "sudo apt install --only-upgrade ffmpeg"
        );
        assert_eq!(
            PackageManagerKind::Pipx.manual_command("gamdl"),
            "pipx upgrade gamdl"
        );
        assert_eq!(
            PackageManagerKind::Snap.manual_command("ffmpeg"),
            "sudo snap refresh ffmpeg"
        );
    }

    #[test]
    fn pipx_path_classifier() {
        // Any pipx home works — we key on the `venvs/<pkg>/` segment.
        let p = PathBuf::from("/home/u/.local/share/pipx/venvs/gamdl/bin/gamdl");
        assert_eq!(pkg_after_component(&p, "venvs").as_deref(), Some("gamdl"));
        let p2 = PathBuf::from("/home/u/.local/pipx/venvs/yt-dlp/bin/yt-dlp");
        assert_eq!(pkg_after_component(&p2, "venvs").as_deref(), Some("yt-dlp"));
        // No venvs segment → no match.
        let p3 = PathBuf::from("/usr/bin/ffmpeg");
        assert_eq!(pkg_after_component(&p3, "venvs"), None);
    }

    #[test]
    fn scoop_path_classifier() {
        let p = PathBuf::from(r"C:\Users\me\scoop\apps\ffmpeg\current\bin\ffmpeg.exe");
        // On non-Windows the backslash path is a single component, so build a
        // forward-slash equivalent to exercise the segment logic portably.
        let p_unix = PathBuf::from("/c/Users/me/scoop/apps/ffmpeg/current/bin/ffmpeg.exe");
        assert_eq!(scoop_owner(&p_unix).as_deref(), Some("ffmpeg"));
        // The raw Windows path only classifies on Windows; assert it does not
        // panic elsewhere.
        let _ = scoop_owner(&p);
    }

    #[test]
    fn snap_path_classifier() {
        let app = PathBuf::from("/snap/ffmpeg/current/usr/bin/ffmpeg");
        assert_eq!(snap_owner(&app).as_deref(), Some("ffmpeg"));
        let wrapper = PathBuf::from("/snap/bin/ffmpeg");
        assert_eq!(snap_owner(&wrapper).as_deref(), Some("ffmpeg"));
        let other = PathBuf::from("/usr/bin/ffmpeg");
        assert_eq!(snap_owner(&other), None);
    }

    #[test]
    fn homebrew_formulae_parsing_preserved() {
        assert_eq!(
            homebrew_formulae("ffmpeg-full\nowner/tap/gamdl\n\n").collect::<Vec<_>>(),
            vec!["ffmpeg-full", "owner/tap/gamdl"]
        );
    }
}
