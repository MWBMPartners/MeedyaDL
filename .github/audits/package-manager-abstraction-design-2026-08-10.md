# Package-Manager Abstraction — Deep Analysis & Phase-2 Design

**Date:** 2026-08-10
**Branch:** `claude/gamdl-v3-8-5-review-gs36zl` (one-branch model — no new PRs, no stacking)
**Status:** ✅ **Phase 2a IMPLEMENTED** (see the update note below) — Phase 2b+ deferred (§5).

> **Implementation note (2026-08-10).** Phase 2a shipped on this branch. The maintainer resolved the two load-bearing decisions via AskUserQuestion: **A → GAMDL detect-and-inform only** (recommended; a read-only `detect_external_gamdl` IPC surfaces the note in the wizard's GAMDL step, MeedyaDL never consumes/updates an external GAMDL), and **D1 → root-requiring PMs auto-update WITH elevation** (the maintainer chose auto-elevation over detect-only). Consequently the `UpdateCapability` model is `{Auto, Elevated}` (not `{Auto, DetectOnly}`): apt/dnf/snap/MacPorts upgrades run through the #997 `sudo -n`/`pkexec` tiers, degrading to an actionable manual command with no privilege path; a failed/un-elevatable update is non-fatal (adopt-as-found). Everything else landed as specified in §4: `services/package_manager.rs` (`PackageManagerKind`/`PackageRef`/`UpdateCapability`/`detect_owner`/`upgrade`, Homebrew arm moved in), generalised `.source` markers + Step-0 delegation, `ComponentUpdate.{managed_by, manual_update_command}`, `src/lib/pm-source.ts` badge helper + per-tool Updates rows, and read-only external-GAMDL detection. Validated: backend clippy clean + `cargo test --lib` 1667/1667; frontend type-check + eslint clean + 610/610 vitest; IPC/UA/codec audits clean. D5 (Scoop) is code-complete but needs a real-Windows validation pass before stable; D6 (yt-dlp detect-inform) deferred to M8/M10 as recommended. **Original design (unchanged) follows.**

**Original status:** Design only — no source code changed by this pass
**Prior art:** #1081 (`7395672`) tool detection + reuse, `ca6566b` status-time adoption, #1017 system-Python reuse, #997 elevation tiers, #522 GAMDL pin/downgrade

---

## 1. Executive summary

The maintainer's goal has two halves: **(1)** every current and future external component should detect a package-manager-installed (Homebrew or similar) copy and reuse it instead of installing a duplicate; **(2)** when a PM-owned component needs an update, MeedyaDL should route the update *through the owning package manager* rather than performing a managed re-download.

Half (1) is **already substantially done** for the two component classes where it matters most:

- **Binary tools** (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, MediaInfo, rclone) detect, adopt in place, and badge system/Homebrew installs — at both install time and status time — and already delegate updates of a previously-adopted Homebrew formula to `brew upgrade`.
- **Python** detects system interpreters (Homebrew, python.org, pyenv, Microsoft Store), reuses one via a venv, records provenance, and suppresses the portable-version update nag for a system venv.

The **real remaining work** is:

1. **Generalising the Homebrew-only machinery into a `PackageManager` abstraction** so the same detect/attribute/update pattern covers MacPorts, Linuxbrew (already partially), pipx, Scoop, apt/dpkg, dnf/rpm, and snap — with a hard split between PMs that are *safe to auto-invoke* (no elevation: Homebrew, pipx, Scoop) and PMs that are **detect-only** (apt, dnf, snap, MacPorts — all need root to mutate).
2. **GAMDL**: the answer to "should MeedyaDL reuse a pipx/user-pip GAMDL?" is **no by default, and detect-only in Phase 2a** (§4.A). The dedupe win is tens of megabytes of venv site-packages; the cost is the entire `gamdl_capabilities` version-control story (bounded pip specs, wheel-ABI checks, forced downgrades, capability cache). Isolation *is* the correct dedupe boundary for pip packages; the expensive shared layer — the interpreter — is already deduped by #1017.
3. **Python**: there is **no legitimate "update via PM" action** for Python (§4.A-Python). MeedyaDL consumes a system interpreter via venv; `brew upgrade python` is the user's business and can *break* the venv — the real Phase-2 Python work is venv-liveness detection and guided re-provision, not PM invocation.

Phase 2a (§5) is deliberately small: one new module (`services/package_manager.rs`), a generalised `.source` grammar (`<pm>:<pkg>`), generalised adoption/update seams in `dependency_manager.rs`, a `managed_by` field on `ComponentUpdate` with "update it yourself with `<command>`" guidance for detect-only PMs, a badge-label map in the wizard, and read-only external-GAMDL detection. Everything riskier (elevated PM invocation, external-GAMDL consumption, PM-native outdated queries, winget) is deferred to 2b+ behind explicit maintainer decisions (§7).

---

## 2. Current-state map (verified against code)

### 2.1 Binary tools — the reference pattern (DONE, #1081 + `ca6566b`)

All in `src-tauri/src/services/dependency_manager.rs` unless noted. Six tools are registered (`TOOLS` at lines 981–1015: `ffmpeg`, `mp4decrypt`, `nm3u8dlre`, `mp4box`, `mediainfo`, `rclone`).

| Concern | Mechanism | Location |
|---|---|---|
| PATH-independent search dirs | `system_tool_search_dirs()` — Homebrew (`/opt/homebrew/bin`, `/usr/local/bin`), MacPorts (`/opt/local/bin`), Linuxbrew (system + per-user `~/.linuxbrew/bin`), `/snap/bin`, base dirs. **Windows returns `&[]`** — relies on `where` PATH search only. | `dependency_manager.rs:348-380` |
| Trust gate | `is_trusted_binary()` — rejects world-writable file or parent dir (Unix `mode & 0o002`); always `true` on Windows. | `dependency_manager.rs:387-398` |
| Detection | `find_system_tool(tool_id)` — `which`/`where`, then direct dir probe; candidates must be absolute + existing + trusted; version extracted via the tool's configured flag. | `dependency_manager.rs:412-487` |
| Homebrew location | `find_homebrew()` — PATH scan + 3 fixed absolute candidates (incl. Linuxbrew). | `dependency_manager.rs:491-507` |
| Owner attribution | `find_homebrew_owner(binary)` — enumerates `brew list --formula -1`, canonical-prefix-compares against `brew --prefix <formula>`. Catalogue-free: auto-covers `ffmpeg-full`-style alternates and future tools. | `dependency_manager.rs:519-548` |
| Update delegation | `upgrade_homebrew_formula(brew, formula)` — fixed argv `brew upgrade <formula>`. | `dependency_manager.rs:550-564` |
| Status-time adoption | `adopt_system_tool_if_available()` — detect + adopt **in place** (`.external-path` pointer + `.source` marker, no copy), min-version gated, never triggers `brew upgrade`. Called from `check_all_dependencies` when neither pointer nor managed binary exists. | `dependency_manager.rs:577-622`; `commands/dependencies.rs:603-612` |
| Install-time adoption + update | `install_tool()` Step 0 — re-detects; **delegates `brew upgrade` only when the persisted `.source` was already `homebrew:<f>` AND that formula still owns the binary** (initial adoption never mutates the system); writes `.external-path` + `.source`; incompatible system version falls through to the managed download pipeline. | `dependency_manager.rs:1367-1441` (delegation 1382-1394, fall-through 1438-1441) |
| Provenance grammar | `.source` ∈ `managed` \| `system` \| `homebrew:<formula>`; `managed` written by the download path at 1501-1502 and mirror path at 2452-2454. | see lines above |
| Runtime resolution | `get_tool_binary_path()` honours `.external-path` first (in-place reuse — no copy, no dupe). ffprobe resolves as the *sibling* of an adopted ffmpeg (e.g. `/opt/homebrew/bin/ffprobe`) — `copy_companion_ffprobe_from_dir` was removed. | `dependency_manager.rs:1277-1281, 1291-1298, 2590-2594` |
| Elevation precedent (#997) | Linux-ARM MP4Box path: tiered `sudo -n` probe → `pkexec` (GUI sessions only) → actionable "run `sudo apt-get install gpac` yourself" error. **This is install, not update, and is the only place MeedyaDL touches a root-owned PM.** | `dependency_manager.rs:2097-2230` |
| Status display | `DependencyStatus.source: Option<String>` (`commands/dependencies.rs:85`), resolved at `commands/dependencies.rs:668-681`; frontend badge is a *binary* split: `tool.source.startsWith('homebrew') ? 'Homebrew' : 'System'` | `src/components/setup/steps/DependenciesStep.tsx:202-208` |
| Update checks | `check_all_updates` only checks two tools against GitHub (`ffmpeg`→BtbN, `nm3u8dlre`→nilaoda), `update_checker.rs:804-818`; `check_github_tool_update` compares installed vs GitHub-latest semver and sets `tool_id` so the frontend routes the Upgrade click to `install_dependency` → `install_tool` → Step 0 → (if brew-owned) `brew upgrade`. | `update_checker.rs:1338-1431` |

**Conclusion:** for tools, detection + in-place reuse + brew-owned update routing is complete *for Homebrew*. The gaps: (a) owner attribution and update routing for every other PM (MacPorts/apt/dnf/snap/scoop produce a generic `system` marker today, so their updates always fall into the managed re-download path — the exact thing the maintainer wants avoided); (b) Windows has no PM attribution at all; (c) the Updates page has no way to say "this is PM-owned — here's how to update it".

### 2.2 Python (detection DONE, #1017)

- `detect_system_pythons()` probes PATH names, `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`, python.org framework dirs, pyenv shims, and the Windows `py` launcher, deduped by canonical `sys.executable` (`python_manager.rs:535-629`).
- `classify_source()` already produces PM-ish labels: `Homebrew` / `python.org` / `pyenv` / `Microsoft Store` / `System` (`python_manager.rs:492-505`).
- Reuse = venv: `provision_venv_from_system_python()` (`python_manager.rs:674+`); provenance marker `PythonSourceRecord { source: "portable"|"system-venv", interpreter, version }` at `{python_dir}/.meedya-python-source.json` (`python_manager.rs:400-443, 632-659`).
- `check_python_update` **suppresses** the portable-`PYTHON_VERSION` nag for a system venv and reports the venv's own version as "latest" (`update_checker.rs:1449-1464`).

### 2.3 GAMDL (the real gap in the maintainer's framing)

- Installed by `pip install --upgrade --only-binary=gamdl '<spec>'` **into the managed venv** (`gamdl_service.rs:108-216`); routine spec is the bounded window `gamdl>=3.0,<=3.8.5` (`gamdl_capabilities::pip_version_spec`, `gamdl_capabilities.rs:418-425`; window values `tool-versions.toml:173, 644, 648`), explicit above-ceiling opt-in pins `gamdl==<target>` (`pip_target_spec`, `gamdl_capabilities.rs:440-442`; consumed at `gamdl_service.rs:147-150`).
- Downgrades: `install_gamdl_version()` uses `--force-reinstall` + exact pin (`gamdl_service.rs:253-321`, #522).
- Invoked as `{managed_python} -m gamdl` (`gamdl_service.rs:483-488`) with managed tool paths injected (`inject_tool_paths`, `gamdl_service.rs:579+`).
- Version probe `pip show gamdl` **feeds the process-global capability cache** (`gamdl_service.rs:380-387`) that every CLI-arg/INI emission consults (wrapper eras, `GamdlFeature` gates).
- Update flow: `check_gamdl_update` (PyPI JSON, `update_checker.rs:890-976`) → `is_untested` via `is_above_tested_ceiling` (`gamdl_capabilities.rs:385-393`) → `upgrade_gamdl(target?)` IPC (`commands/updates.rs:157-185`) → `install_gamdl`.
- **Wheel-ABI safety is interpreter-coupled:** `no_compatible_wheel` is computed against the *managed* interpreter's CPython tag (`derive_cpython_tag(python_manager::get_target_python_version())`, `update_checker.rs:927-939`) — this check is meaningless for a foreign (pipx-owned) interpreter.
- Per-platform ceilings: `effective_maximum_tested` / `classify_for_platform` (`gamdl_capabilities.rs:306-351`; `linux-armv7 = "3.8.1"` at `tool-versions.toml:680-681`).
- **There is no Homebrew formula for gamdl** (homebrew-core does not carry it; it is a PyPI-only project — re-verify at implementation time). "GAMDL via Homebrew" therefore realistically means **pipx** (`pipx install gamdl`) or a user-wide `pip install --user gamdl`.
- Other pip engines (votify, yt-dlp, ofscraper) install into the **same managed venv** (`pip_engine_service.rs:68-108`; `spotify_service::install_votify` mirrors GAMDL's bounded-spec shape per `commands/updates.rs:193-199`) — so the managed venv must exist regardless of what happens with GAMDL.

---

## 3. Answers to the design questions

### A. Should MeedyaDL reuse a system/pipx/brew-installed GAMDL?

**Recommendation: NO as default behaviour; detect-and-inform only in Phase 2a; opt-in consumption is a deferred 2b+ item gated on a maintainer decision (and my recommendation there is "probably never").**

Reasoning, in order of weight:

1. **Isolation is the correct dedupe boundary for pip packages.** The expensive duplicated artefact in the Python world is the *interpreter* (30–80 MB + shared libs) — already deduped by #1017's venv reuse. A gamdl install inside a venv is tens of MB of site-packages (httpx, pywidevine, mutagen, the `_ammuxer` wheel). Sharing it saves little and creates a *version-requirement conflict between two consumers*: MeedyaDL wants `>=3.0,<=3.8.5` (`tool-versions.toml:173,644`); the pipx user typically wants latest. Two consumers with different constraints on one install is exactly the problem venvs and pipx exist to prevent. Deduping here would *reintroduce* the anti-pattern the ecosystem's tooling is built to avoid.
2. **The whole reliability story depends on MeedyaDL owning the version.** The bounded spec (`pip_version_spec`), the untested-ceiling opt-in (`pip_target_spec`), the forced-downgrade path (#522, `install_gamdl_version`), the wheel-ABI gate (`no_compatible_wheel` computed against *our* interpreter, `update_checker.rs:930`), and platform ceilings (`classify_for_platform`) all assume MeedyaDL can pin, upgrade, and downgrade at will. A pipx-owned GAMDL can be `pipx upgrade`d past the ceiling — or past a wrapper-era boundary (v3.5.x wrapper-v1 → v3.6+ wrapper-v2, `LAST_WRAPPER_V1_VERSION` at `gamdl_capabilities.rs:466`) — at any moment, out of band. The capability cache would track it (probing still works), but MeedyaDL's *remedies* (downgrade, bounded re-install) would now mutate an environment the user owns for their own CLI workflows. Either MeedyaDL silently changes the user's `gamdl` CLI (unacceptable — violates the "initial adoption never mutates the system" principle at `dependency_manager.rs:1379-1382`), or MeedyaDL loses its remedies (unacceptable — that machinery exists because upstream breakage is routine; see the v3.0 `--fetch-extra-tags` fire cited at `gamdl_service.rs:132-135`).
3. **Invocation and probing would need a parallel path.** Consumption would replace `{managed_python} -m gamdl` (`gamdl_service.rs:487-488`) with a bare `gamdl` entry point; version probing would move from `pip show` (`gamdl_service.rs:338-388`) to parsing `gamdl --version`; the `pip show --verbose` integrity logging (`gamdl_service.rs:199-212`) and `--only-binary` protections disappear; `inject_tool_paths` still works (CLI flags), but config.ini sync assumptions would need re-validation. All of that is a second code path to test for a marginal saving.
4. **ARMv7 nuance:** on the one platform where wheel availability actually bites (`linux-armv7 = 3.8.1` ceiling), a user-installed GAMDL is *more* likely to be a broken sdist attempt, not less.

**What Phase 2a ships instead (satisfies the "don't duplicate blindly" intent honestly):** a read-only `detect_external_gamdl()` that finds a `gamdl` entry point on PATH / in pipx venvs, attributes it (`pipx:gamdl` / `system`), probes its version (2 s timeout, trusted-binary gate), classifies it against the support window (`gamdl_capabilities::classify`), and surfaces one informational line in the setup wizard / Updates page: *"GAMDL X.Y.Z is also installed via pipx. MeedyaDL keeps its own copy so downloads stay on a tested version; your pipx copy is not touched."* This tells the user we saw their install and explains *why* the duplicate exists, instead of looking oblivious.

**Corollary — the "update GAMDL through Homebrew/pipx" half of the goal:** because MeedyaDL does not consume the pipx GAMDL, MeedyaDL must **never update it** (updating an environment we don't consume is pure mutation of user property with zero benefit to us). The update-routing requirement is therefore *vacuous for GAMDL by design*, unless the maintainer later opts into external-GAMDL consumption (§7-D2). This should be stated openly in the doc/PR rather than left implicit.

**Python sub-question (task item 2):** *is there any "update via PM" action for Python?* **No — and one should never be added.** MeedyaDL reuses a system Python by snapshotting it into a venv; it does not own the base interpreter. `brew upgrade python@3.x` is (a) the user's decision, (b) frequently *destructive to existing venvs* — Homebrew relocates/removes old Cellar paths on minor bumps, leaving `{app_data}/python/bin/python3` a dangling symlink. The correct Phase-2 Python work is the inverse: **venv-liveness detection** — `check_python_status` already fails on a dead venv (it runs the binary, `python_manager.rs:284+`), but the resulting UX is "Python not installed" with no explanation. A 2b item should detect the marker saying `system-venv` + a dead interpreter and offer "your system Python moved (probably a Homebrew upgrade) — rebuild the environment from <new interpreter>?" via the existing `provision_venv_from_system_python`. No PM invocation anywhere.

### B. The package-manager abstraction

**Recommendation: a closed `enum PackageManagerKind` with inherent async methods + a `PackageRef` value type, in a new module `src-tauri/src/services/package_manager.rs` — not a trait object.** The PM set is small, closed, and platform-partitioned; enum dispatch avoids `async fn`-in-dyn-trait friction, matches the codebase's free-function style, and makes the capability table exhaustively `match`-checked by the compiler (adding a PM forces every seam to decide its policy).

```rust
// services/package_manager.rs  (new file, ~350 lines incl. tests)

/// Closed set of package managers MeedyaDL can attribute installs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManagerKind {
    Homebrew,   // macOS + Linuxbrew — no elevation
    MacPorts,   // macOS — query free, mutation needs root
    Pipx,       // all platforms — per-user, no elevation
    Scoop,      // Windows — per-user, no elevation
    Apt,        // Debian/Ubuntu — query free (dpkg -S), mutation needs root
    Dnf,        // Fedora/RHEL — query free (rpm -qf), mutation needs root
    Snap,       // Linux — mutation needs root; snaps self-update anyway
}

/// A PM-attributed package: serialises to/from the `.source` marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRef { pub pm: PackageManagerKind, pub package: String }

/// What MeedyaDL is allowed to do about an update for this PM.
pub enum UpdateCapability {
    /// Safe to auto-invoke: per-user, no elevation (Homebrew, Pipx, Scoop).
    Auto,
    /// Never auto-invoke: root required or self-updating. Carry the exact
    /// command the user should run themselves.
    DetectOnly { manual_command_template: &'static str }, // "{pkg}" placeholder
}

impl PackageManagerKind {
    pub fn marker_prefix(self) -> &'static str;      // "homebrew" | "macports" | "pipx" | "scoop" | "apt" | "dnf" | "snap"
    pub fn display_label(self) -> &'static str;       // "Homebrew" | "MacPorts" | "pipx" | "Scoop" | "APT" | "DNF" | "Snap"
    pub fn update_capability(self) -> UpdateCapability;
    pub async fn locate(self) -> Option<PathBuf>;     // the PM binary itself (fixed abs candidates + PATH; find_homebrew() moves here)
    pub async fn owner_of(self, binary: &Path) -> Option<PackageRef>;
    pub async fn upgrade(self, pkg: &PackageRef) -> Result<(), String>; // Err for DetectOnly kinds
}

impl PackageRef {
    pub fn to_marker(&self) -> String;                        // "pipx:gamdl"
    pub fn parse_marker(s: &str) -> Option<PackageRef>;       // None for "managed"/"system"/unknown prefixes
    pub fn manual_update_command(&self) -> Option<String>;    // rendered template for DetectOnly
}

/// Cascade: cheap pure-path classification first, then per-PM owner lookup.
pub async fn detect_owner(binary: &Path) -> Option<PackageRef>;
```

**Per-PM owner detection (all fixed argv, package names only ever originate from PM output or path components — never user input):**

| PM | Classification (pure path, canonicalised) | Owner lookup | Upgrade argv |
|---|---|---|---|
| Homebrew | target under a Cellar/`$(brew --prefix)` | existing `find_homebrew_owner` logic **moves here verbatim** (`dependency_manager.rs:519-548`) | `brew upgrade <formula>` (moves from :550-564) |
| MacPorts | target under `/opt/local/` | `port provides <path>` → parse "provided by: <port>" | — DetectOnly: `sudo port upgrade {pkg}` |
| pipx | target under `{PIPX_HOME}/venvs/<pkg>/` (candidates: `$PIPX_HOME`, `~/.local/pipx`, `~/.local/share/pipx`; Windows `%USERPROFILE%\pipx`, `%LOCALAPPDATA%\pipx` — confirm against pipx docs at impl time); `<pkg>` = path component after `venvs/` | path-derived; optional `pipx list --json` confirmation deferred | `pipx upgrade <pkg>` |
| Scoop | binary under `%USERPROFILE%\scoop\` (or `%SCOOP%`); shims carry a sibling `<name>.shim` text file whose `path = ...` line points into `scoop\apps\<pkg>\` | parse `<pkg>` from the apps path segment | `scoop update <pkg>` |
| apt | Linux + `/usr/bin`-ish path + `dpkg` present | `dpkg -S <path>` → "pkg: path" (no root) | — DetectOnly: `sudo apt install --only-upgrade {pkg}` |
| dnf | Linux + `rpm` present | `rpm -qf --qf %{NAME} <path>` (no root) | — DetectOnly: `sudo dnf upgrade {pkg}` |
| snap | target under `/snap/<pkg>/` | path-derived | — DetectOnly: `sudo snap refresh {pkg}` (note in UI: snaps normally auto-refresh) |

**Cascade order in `detect_owner`:** pure-path classifiers (pipx, scoop, snap, MacPorts prefix) → Homebrew (subprocess-heavy: N×`brew --prefix`) → dpkg/rpm (cheap subprocess, Linux only). First hit wins. Every candidate path is canonicalised first (symlink `/usr/local/bin/ffmpeg` → Cellar is already handled this way in `find_homebrew_owner:530`).

**Deliberately out of scope for the enum:** `winget` (no binary→package reverse query exists; owner attribution is guesswork — 2b research spike at best), `choco` (same problem + typically elevated), `pacman` (add later if requested — `pacman -Qo` is trivial to slot in, which is the point of the enum).

**`.source` marker grammar generalisation:** `managed` and `system` stay as-is; `homebrew:<formula>` stays byte-identical (zero migration — existing markers keep working); new writers may produce `macports:<port>`, `pipx:<pkg>`, `scoop:<pkg>`, `apt:<pkg>`, `dnf:<pkg>`, `snap:<pkg>`. Readers MUST treat an unknown `<prefix>:<pkg>` as `system` (display) + no update delegation (behaviour) — forwards compatibility for markers written by a newer MeedyaDL. The current frontend ternary (`DependenciesStep.tsx:207`) already degrades unknown prefixes to "System", confirming the grammar is backwards-safe.

### C. Update routing — where are the seams?

**Seam 1 — `install_tool` Step 0 (`dependency_manager.rs:1375-1441`), the only place a PM update is currently auto-invoked.** Generalise the hardcoded `strip_prefix("homebrew:")` block (1383-1394) to:

```
previous = PackageRef::parse_marker(read .source)
if let Some(prev) = previous {
    if let Some(owner) = package_manager::detect_owner(&system_path).await {
        if owner == prev {                      // same PM, same package — provenance holds
            match owner.pm.update_capability() {
                Auto        => owner.pm.upgrade(&owner).await?;  // brew/pipx/scoop
                DetectOnly{..} => log + emit_app_log the manual command; adopt as-found
            }
            re-probe find_system_tool → refresh path/version
        }
    }
}
```

The two existing invariants are preserved *exactly*: initial adoption never mutates the system (delegation requires a pre-existing matching marker), and an incompatible system version still falls through to the managed download (1438-1441). Marker writes at 1416-1429 and in `adopt_system_tool_if_available` (602-615) switch from the inline Homebrew match to `detect_owner(...).map(|r| r.to_marker()).unwrap_or("system")`.

**Seam 2 — `check_github_tool_update` (`update_checker.rs:1338-1431`), the update *display* path.** Two problems today for PM-owned tools: (a) it compares an apt/brew-versioned binary against upstream GitHub tags — a category error for distro-patched versions; (b) the Upgrade button routes to `install_dependency`, which for a detect-only PM would silently adopt-as-found (a no-op the user reads as a broken button). Phase 2a fix (minimal): read the `.source` marker (cheap fs read via `get_tool_dir`), add `managed_by: Option<String>` + `manual_update_command: Option<String>` (both `#[serde(default)]`, mirrored in `src/types/index.ts`) to `ComponentUpdate` (`update_checker.rs:92-167`); frontend (`UpdatesPage.tsx` / `UpdateBanner.tsx`): when `manual_update_command` is set, render the command as copyable guidance *instead of* the Upgrade button; when `managed_by` is an Auto PM, relabel the button "Update via Homebrew" (routing unchanged — Step 0 already delegates). PM-native staleness queries (`brew outdated --json`) are 2b (§6).

**Seam 3 — GAMDL (`check_gamdl_update` + `upgrade_gamdl` → `install_gamdl`).** **No change in 2a** per recommendation A — GAMDL remains managed-venv pip, and the bounded/pinned spec logic (`gamdl_capabilities.rs:418-442`) remains the sole update mechanism. If external-GAMDL consumption ever ships (2b+, maintainer-gated): the routing rule must be *"MeedyaDL never mutates a foreign GAMDL"* — `check_gamdl_update` would classify the external version (`classify_for_platform`) and render pipx guidance (`pipx upgrade gamdl`, or `pipx install --force 'gamdl==X'` when the ceiling requires a pin), never auto-invoke. This keeps `is_above_tested_ceiling` enforcement intact: the ceiling governs what MeedyaDL *recommends*, and the untested-badge machinery (`ComponentUpdate.is_untested`, `update_checker.rs:916-918`) governs what it *warns about* — same as today, just with the execution step handed to the user.

**Seam 4 — Python.** None. Deliberately no PM routing (§3.A). The `PythonSourceRecord` marker is *not* migrated to the `<pm>:<pkg>` grammar — it answers a different question ("portable vs system-venv"), and `classify_source`'s label (`python_manager.rs:492-505`) is display-only provenance, not update-routing provenance.

### D. Provenance + status display

- **Marker grammar:** §3.B. No settings-schema change, no migration — `.source` files are per-tool sidecar files, not `settings.json` fields, and old values remain valid.
- **`DependencyStatus.source`** (`commands/dependencies.rs:85`): already a free-form `Option<String>` — carries the new markers with zero backend change beyond what Step 0/adoption writes.
- **Frontend badge** (`DependenciesStep.tsx:202-208`): replace the binary ternary with a small pure helper (new `src/lib/pm-source.ts`, unit-testable): `sourceLabel(source: string): string` — split on the first `:`, map `homebrew→Homebrew, macports→MacPorts, pipx→pipx, scoop→Scoop, apt→APT, dnf→DNF, snap→Snap`, unknown-prefix/`system`→`System`. Tooltip stays "Using your existing install — no duplicate download".
- **`ComponentUpdate`**: add `managed_by` + `manual_update_command` (§3.C Seam 2). Additive + `serde(default)` → no wire-compat risk.
- **External GAMDL info line** (§3.A): suggest surfacing in the wizard's GAMDL step and/or the Updates page GAMDL card; exact placement is implementer's choice — it is informational text, not a control.
- **CLAUDE.md**: the dependency-manager bullet (currently ending "…the full multi-PM abstraction … are Phase 2") must be updated when 2a lands, per the documentation-maintenance convention. `help/*.md` likely untouched in 2a (no user-workflow change); if any help topic is edited, its inline twin in `HelpViewer.tsx` must be edited too (known trap).

### E. Scope + phasing

Ranked by value ÷ risk:

1. **Highest value, low risk (Phase 2a):** the abstraction module + generalised markers + detect-only guidance on the Updates page. This closes the actual user-visible gap — an apt/MacPorts/scoop user today gets a generic "System" badge and a managed re-download on update — with zero new elevation surface and zero behaviour change for the already-working Homebrew path.
2. **Medium value, low risk (2a):** external-GAMDL detect-and-inform. Cheap, honest, prevents "why does MeedyaDL ignore my pipx gamdl?" confusion, and creates the provenance hook any future opt-in would need.
3. **Medium value, medium risk (2b):** PM-native outdated queries; scoop end-to-end validation on real Windows; venv-liveness detection + guided re-provision.
4. **Low value or high risk (2b+/never, maintainer-gated):** elevated auto-updates (apt/dnf/snap/MacPorts); external-GAMDL consumption; winget/choco attribution.

---

## 4. Phase-2a increment (concrete implementation plan)

**Goal:** generalise Homebrew-only machinery to the multi-PM abstraction; detect-only guidance for root-requiring PMs; external-GAMDL detection. **Non-goals:** any elevated PM invocation; any change to what MeedyaDL executes for GAMDL/Python; PM-native staleness queries.

### 4.1 New files

- `src-tauri/src/services/package_manager.rs` — everything in §3.B: `PackageManagerKind`, `PackageRef`, `UpdateCapability`, `locate()`, `owner_of()`, `upgrade()`, `detect_owner()`, marker parse/format. `find_homebrew` (:491-507), `homebrew_formulae` (:509-514), `find_homebrew_owner` (:519-548), `upgrade_homebrew_formula` (:550-564) **move** here (dependency_manager keeps thin `pub(crate)` re-export shims only if needed to avoid churn in tests). Register in `services/mod.rs`.
- `src/lib/pm-source.ts` — `sourceLabel()` helper + label map (frontend).

### 4.2 Modified files

| File | Change |
|---|---|
| `services/dependency_manager.rs` | Step 0 of `install_tool` (1375-1441): replace the Homebrew-only delegation with the capability-dispatched block in §3.C Seam 1. `adopt_system_tool_if_available` (601-615): marker via `detect_owner`. Both paths log the manual command for DetectOnly kinds via `emit_app_log`. |
| `services/update_checker.rs` | `ComponentUpdate`: add `managed_by: Option<String>`, `manual_update_command: Option<String>` (`serde(default)`). `check_github_tool_update`: read `.source`, populate both fields; keep GitHub compare as-is. Every other `ComponentUpdate` construction site sets them `None` (compiler will enumerate the sites). |
| `commands/dependencies.rs` | No structural change — `check_all_dependencies` already passes markers through (668-681). |
| `src/types/index.ts` | Mirror the two new optional `ComponentUpdate` fields. |
| `src/components/setup/steps/DependenciesStep.tsx` | Badge label via `sourceLabel()` (line 207). |
| `src/components/updates/UpdatesPage.tsx`, `src/components/common/UpdateBanner.tsx` | When `manual_update_command` present → copyable guidance text instead of Upgrade button; when `managed_by` is an Auto PM → "Update via <label>" button text. |
| `services/gamdl_service.rs` (or a small new fn in `package_manager.rs`) | `detect_external_gamdl(app) -> Option<ExternalGamdlInfo { path, version, source_marker, classification }>`: `which gamdl` + `system_tool_search_dirs`-style probe + pipx path classification; `is_trusted_binary` gate; `gamdl --version` with 2 s timeout; classify via `gamdl_capabilities::classify_for_platform`. **Read-only.** |
| `commands/dependencies.rs` + `lib.rs` `generate_handler![]` + `src/lib/tauri-commands.ts` | New IPC `detect_external_gamdl` (remember: `tools/audit-checks/check_ipc_commands.py` enforces registration — run it). |
| `.claude/CLAUDE.md` | Update the dependency-manager Phase-2 sentence; add the GAMDL-not-reused decision one-liner. |

### 4.3 Security invariants (restated for the implementer — all enforced by pr-security checks)

- Every PM invocation is `Command::new(<located_pm_binary>).args([...fixed...])` — **no `sh -c`**, no string-built commands (pr-security check 2).
- Package names passed to `upgrade` come only from `PackageRef`s produced by `owner_of()` (PM output / filesystem path components) and round-tripped through our own marker files — never from user input, and never as a leading-`-` string (defence: reject package names starting with `-` in `parse_marker`).
- Every adopted binary and every located PM binary passes `is_trusted_binary` (world-writable rejection, `dependency_manager.rs:387-398`).
- No `Command::env` usage; env reads limited to our own process (`HOME`, `PIPX_HOME`, `SCOOP`), matching the existing convention (`dependency_manager.rs:370-372`).
- DetectOnly kinds' `upgrade()` returns `Err` unconditionally — the type system carries the policy, so a future call site cannot accidentally auto-invoke apt.

### 4.4 Tests

- `package_manager::tests` (Rust): marker round-trip (`homebrew:ffmpeg`, `pipx:gamdl`, `apt:ffmpeg`); `parse_marker` rejects `managed`, `system`, empty, unknown prefix, and leading-`-` package names; capability table (exhaustive `match` — Auto = {Homebrew, Pipx, Scoop}, everything else DetectOnly); pure-path classifiers against `tempfile`-built fake layouts (pipx `venvs/<pkg>/bin/x`, scoop `scoop/apps/<pkg>/current/x` + `.shim` parse, snap `/`-relative fake root parameterised so the test doesn't need a real `/snap`); manual-command template rendering.
- `dependency_manager::tests`: Step-0 marker generalisation (extend the existing `.external-path` tests at 2793-2799); DetectOnly delegation adopts-as-found without invoking anything (assert via marker + returned version).
- Frontend (vitest): `pm-source.test.ts` label map incl. unknown-prefix fallback; `DependenciesStep` badge rendering for `pipx:gamdl` / `apt:ffmpeg`; UpdatesPage guidance-vs-button branch.
- Run `python3 tools/audit-checks/check_ipc_commands.py` after wiring the new IPC.

**Estimated size:** ~600–800 LOC Rust (half of it moved code + tests), ~150 LOC TS. One reviewable commit series on the existing branch.

---

## 5. Deferred — Phase 2b and later

1. **PM-native staleness queries** — `brew outdated --json=v2 <formula>`, `scoop status`, `pipx list --json` version compare — replacing the GitHub compare for PM-owned tools in `check_github_tool_update`. Fixes the distro-version/upstream-tag category error properly.
2. **Venv-liveness detection + guided re-provision** (§3.A-Python): dead `system-venv` interpreter → explain + one-click rebuild via `provision_venv_from_system_python`. Small, real user pain; first candidate for 2b.
3. **Opt-in external-GAMDL consumption** — full second invocation/probing path (§3.A points 2–3). Only if maintainer decision D2 says yes; my recommendation is it stays permanently in "detect + inform".
4. **Elevated PM auto-updates** (apt/dnf/snap/MacPorts) reusing the #997 `sudo -n`/`pkexec` tiers (`dependency_manager.rs:2097-2230`). Only if maintainer decision D1 says yes; my recommendation is no.
5. **winget/choco attribution research spike** (Windows). winget has no binary→package reverse mapping; Scoop covers the realistic power-user population that co-installs ffmpeg.
6. **pacman support** — one enum arm (`pacman -Qo <path>`; DetectOnly: `sudo pacman -S {pkg}`) if Arch users ask.
7. **Multi-PM detection for other pip engines** (votify/yt-dlp): same analysis as GAMDL — yt-dlp *does* have a Homebrew formula and is very commonly pipx/brew-installed, so a detect-and-inform line for yt-dlp is worth folding into whichever milestone (M8/M10) wires those engines' setup UI.

---

## 6. Maintainer decisions required

- **D1 — Elevated PM updates: should MeedyaDL ever auto-run `sudo -n`/`pkexec` `apt`/`dnf`/`snap`/`port` upgrades for an adopted tool?** Recommendation: **No** — detect-only + copyable command. (#997's elevation precedent is a one-shot *install* of a missing requirement, not a recurring background-adjacent *update* of a shared system package that other software depends on.) Yes / **No**.
- **D2 — External GAMDL: should MeedyaDL ever *consume* (execute) a pipx/user-pip GAMDL, even as an advanced opt-in?** Recommendation: **No** — detect + inform permanently; isolation is the correct dedupe boundary and the version-control machinery is the product's reliability core. Yes, as 2b opt-in / **No, detect-only permanently**.
- **D3 — If D2 is ever "yes": may MeedyaDL run `pipx upgrade gamdl` / `pipx install --force gamdl==X` on the user's pipx environment?** Recommendation: **No** — guidance only, MeedyaDL never mutates a foreign GAMDL. (Moot under D2 = No.)
- **D4 — Updates-page behaviour for detect-only PM-owned tools:** replace the Upgrade button with the manual command (recommended), or keep the button falling through to a managed re-download (which un-adopts the PM copy and creates the duplicate the maintainer wants to avoid)? **Replace with guidance** / keep managed fallback as a secondary "switch to MeedyaDL-managed copy" action.
- **D5 — Scoop in 2a scope:** include Scoop attribution + `scoop update` delegation now (needs a real-Windows validation pass before release), or land it code-complete but behind detection-only until validated? Recommendation: **include, validated via the existing Windows CI/build channel before stable**.
- **D6 — yt-dlp detect-and-inform:** fold into 2a alongside GAMDL detection (cheap, same helper), or defer to the M8/M10 engine-setup UI work? Recommendation: **defer** — no setup UI exists yet to surface it.

---

## 7. Appendix — load-bearing citations index

| Claim | Location |
|---|---|
| `.source` grammar writers (`managed`/`system`/`homebrew:<f>`) | `dependency_manager.rs:1416-1429, 1499-1502, 602-615, 2452-2454` |
| Brew update delegation requires prior matching marker | `dependency_manager.rs:1377-1394` |
| Status-time adoption never upgrades | `dependency_manager.rs:572-576` (doc comment), `commands/dependencies.rs:596-612` |
| In-place reuse, no copy; ffprobe sibling | `dependency_manager.rs:1291-1298, 2590-2594` |
| Windows search dirs empty (PATH-only) | `dependency_manager.rs:346-347, 366-368` |
| Elevation tiers (#997) | `dependency_manager.rs:2097-2230` |
| Frontend badge binary split | `src/components/setup/steps/DependenciesStep.tsx:202-208` |
| Python provenance + update-nag suppression | `python_manager.rs:400-443, 632-659`; `update_checker.rs:1449-1464` |
| GAMDL bounded/pinned pip specs | `gamdl_capabilities.rs:418-442`; `gamdl_service.rs:126-150` |
| GAMDL invocation + capability cache | `gamdl_service.rs:483-488, 380-387` |
| Wheel check bound to managed interpreter | `update_checker.rs:927-939` |
| Support window + per-platform ceilings | `tool-versions.toml:157-173, 644-648, 680-681`; `gamdl_capabilities.rs:276-351` |
| Untested-ceiling surfacing | `gamdl_capabilities.rs:373-393`; `update_checker.rs:910-918`; `commands/updates.rs:141-147` |
| Downgrade path (#522) | `gamdl_service.rs:253-321` |
| Pip engines share the managed venv | `pip_engine_service.rs:68-108`; `commands/updates.rs:193-199` |
| Tool update-check → Upgrade routing | `update_checker.rs:804-818, 1338-1431`; `commands/dependencies.rs:703-728` |
