# MeedyaDL Developer Notes

Important notes for development, releasing, and CI/CD workflows.

---

## Release Workflow

### There Are 4 Separate Workflows — Don't Confuse Them

| Workflow | Trigger | What It Does | Produces Binaries? |
| -------- | ------- | ------------ | ------------------ |
| **CI** (`ci.yml`) | Every push to `main` | Runs `cargo check`, `cargo test`, `npm test`, `npm type-check` | **No** — just checks code compiles and tests pass |
| **Release Please** (`release-please.yml`) | Every push to `main` | Creates or updates a "Release PR" that bumps version numbers | **No** — just creates/updates a PR |
| **Release** (`release.yml`) | Tag push (`v*`) or manual `workflow_dispatch` | Builds the app on all 6 platforms | **Yes** — this is the only workflow that produces installable binaries |
| **Changelog** (`changelog.yml`) | Tag push (`v*`) or manual `workflow_dispatch` | Regenerates `CHANGELOG.md` via git-cliff | **No** — just updates the changelog file |

**Key insight**: When you push code to `main`, you'll see CI and Release Please run. These are fast and lightweight — they do NOT build binaries. The Release workflow only runs after the full release pipeline completes (see below).

### The Release Pipeline: A Two-Step Process

Getting from "code pushed" to "binaries available" is a **two-step process**. Both steps are required. There are no shortcuts.

#### Step 1: Push Code → Release PR Created (automatic)

```
You push feat:/fix: commits to main
  → CI workflow runs (checks code compiles + tests pass)
  → Release Please workflow runs (creates or updates a Release PR)
  → STOP. Nothing else happens until you merge the PR.
```

At this point you will see:

- CI workflow: green checkmarks (or red if code is broken)
- A PR from `release-please` bot titled something like `chore(main): release 0.3.8`
- **No binaries. No release. No draft.** Just a PR sitting there waiting.

The Release PR contains automatic version bumps to `package.json`, `Cargo.toml`, `tauri.conf.json`, and `.release-please-manifest.json`. You don't need to bump versions yourself.

#### Step 2: Merge the Release PR → Binaries Built (requires your action)

```
You merge the Release PR on GitHub
  → Release Please creates a git tag (e.g., v0.3.8) using RELEASE_PAT
  → Tag push triggers the Release workflow
  → Release workflow builds all 6 platforms (takes ~15-20 minutes)
  → A DRAFT GitHub Release is created with the built binaries
  → Changelog workflow also triggers (regenerates CHANGELOG.md)
```

At this point you will see:

- The Release workflow running with 6 parallel jobs (one per platform)
- A **draft** GitHub Release (not published yet) with installer files attached
- You then review and **publish** the draft release when ready

### Why the PR ALWAYS Needs Merging

The Release PR is not optional. It's the **gate** between "code changes" and "published release":

1. **It bumps versions** — release-please automatically updates version numbers in 4 files. Without this, the built binaries would have the wrong version.
2. **It creates the tag** — only after merging does release-please create the `v0.3.8` tag. The Release workflow is triggered by tag pushes, so no tag = no build.
3. **It's your checkpoint** — you can push 20 commits over a week, and release-please accumulates them into one Release PR. You choose when to merge (= when to release).

**There is no scenario where binaries are built without this PR being merged** (unless you manually trigger the Release workflow via `workflow_dispatch`).

### Why Builds Might "Seem" to Work Then Fail

A common source of confusion:

1. You push code → CI runs → **CI passes** (green checkmark). This only means the code compiles and tests pass. It does NOT mean release builds will work — CI doesn't run `tauri build`, doesn't sign binaries, doesn't create installers.

2. You merge the Release PR → Release Please creates a GitHub Release **immediately with source code archives** (`.zip` and `.tar.gz`). These appear right away. The 6 platform build jobs start running in parallel but take 15-20 minutes.

3. If the build jobs fail, the GitHub Release still exists but only contains source code (no `.dmg`, `.exe`, `.deb`, etc.). **This is what "release with only source code" looks like** — the release was created, the builds failed, and no binary artifacts were uploaded.

### Normal Development Cycle

1. **Write code** and commit using [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat: ...` — new features (bumps minor version)
   - `fix: ...` — bug fixes (bumps patch version)
   - `chore: ...` / `docs: ...` — no version bump, won't appear in changelog

2. **Use `[skip ci]`** in commit messages during rapid development to conserve GitHub Actions minutes:
   ```bash
   git commit -m "feat: add queue persistence [skip ci]"
   ```

3. **Push to main**. Two things happen automatically:
   - CI runs (verifies code is good)
   - Release Please creates/updates a Release PR (you can ignore it until ready)

4. **When ready to release**: go to GitHub and **merge the Release PR**. This triggers the full build pipeline.

5. **Wait for builds** (~15-20 minutes). Check the Actions tab to monitor progress.

6. **Publish the draft release** on GitHub once all builds succeed.

### Manual Release Trigger

If builds fail and you need to retry after fixing the issue, use `workflow_dispatch`:

```bash
gh workflow run "Release" -f tag=v0.3.8
```

This bypasses the tag-push trigger and runs the Release workflow directly.

### What NOT to Do

- **Do NOT manually bump versions** in `package.json`, `tauri.conf.json`, `Cargo.toml`, or `.release-please-manifest.json`. Release-please handles this automatically via the Release PR.
- **Do NOT confuse CI passing with release builds working** — CI only runs `cargo check` + tests, not `tauri build` + signing + bundling. A green CI does not guarantee the release will build.
- **Do NOT delete and recreate tags** — GitHub doesn't re-fire tag push events for recreated tags. Use `workflow_dispatch` instead.
- **Do NOT push directly to main and expect binaries** — you must merge the Release PR first, or manually trigger the Release workflow.

---

## Required GitHub Secrets

The Release workflow requires these secrets to be configured in the repository settings (**Settings → Secrets and variables → Actions**):

### Tauri Updater Signing (Required for all platforms)

| Secret | Description |
|--------|-------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Private key for signing updater artifacts. Generated by `npx tauri signer generate`. Without this, **all builds will fail**. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the private key (can be empty if key was generated without a password) |

**Why these are required**: The `createUpdaterArtifacts: true` setting in `tauri.conf.json` tells the Tauri bundler to create signed update artifacts (`.sig` files and `latest.json` manifest) alongside each platform's installer. Without the signing key, the bundler fails with:
```
failed to build bundler settings: failed to get updater configuration: plugins > updater doesn't exist
```

**To generate keys** (one-time setup):
```bash
npx tauri signer generate -w ~/.tauri/meedyadl.key -p "" --ci
```

This creates:
- `~/.tauri/meedyadl.key` — private key (add contents as `TAURI_SIGNING_PRIVATE_KEY` secret)
- `~/.tauri/meedyadl.key.pub` — public key (already configured in `tauri.conf.json`)

### macOS Code Signing & Notarization

| Secret | Description |
|--------|-------------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` Developer ID certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` certificate |
| `APPLE_SIGNING_IDENTITY` | Certificate common name (e.g., `Developer ID Application: ...`) |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password for notarization |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

These are only used by the macOS build. If missing, the macOS build will fail at the "Validate macOS signing secrets" step.

### AcoustID (Optional)

| Secret | Description |
|--------|-------------|
| `ACOUSTID_API_KEY` | Application API key from [acoustid.org/new-application](https://acoustid.org/new-application). Embedded at compile time via `option_env!("ACOUSTID_API_KEY")` so release builds ship with a pre-configured key for audio fingerprinting. If not set, users must provide their own key in Settings > Metadata. |

### Release Please

| Secret | Description |
|--------|-------------|
| `RELEASE_PAT` | Personal Access Token with `repo` scope. Used instead of `GITHUB_TOKEN` so that tag pushes trigger the Release workflow (GitHub's `GITHUB_TOKEN` doesn't trigger other workflows). |

---

## Signing Keys & Auto-Updates

### How the Auto-Update System Works

1. **Version detection**: The app checks GitHub Releases API and PyPI for newer versions of MeedyaDL, GAMDL, and Python.

2. **Update download**: When a MeedyaDL app update is available, the user clicks "Download & Install" which:
   - Downloads the update binary from the specific GitHub Release
   - Verifies the binary's cryptographic signature using the public key in `tauri.conf.json`
   - Applies the update in-place

3. **Restart**: The user clicks "Restart Now" to load the new version.

### Key Files

| File | Purpose |
|------|---------|
| `tauri.conf.json` → `plugins.updater.pubkey` | Public key for verifying update signatures |
| `tauri.conf.json` → `bundle.createUpdaterArtifacts` | Enables `.sig` file generation during builds |
| `~/.tauri/meedyadl.key` | Private key (local only, never committed) |
| `~/.tauri/meedyadl.key.pub` | Public key (contents are in tauri.conf.json) |

### If You Need to Regenerate Keys

If the private key is lost:

1. Generate a new key pair:
   ```bash
   npx tauri signer generate -w ~/.tauri/meedyadl.key -f -p "" --ci
   ```

2. Update `tauri.conf.json` → `plugins.updater.pubkey` with the new public key:
   ```bash
   cat ~/.tauri/meedyadl.key.pub
   ```

3. Update the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret with the new private key:
   ```bash
   cat ~/.tauri/meedyadl.key
   ```

**Important**: After changing keys, users on older versions won't be able to auto-update to newer versions (signature mismatch). They'll need to download the new version manually.

---

## Pre-Release Channel

The app supports a toggle for "Include Pre-Release Versions" in General settings:

- **Off (default)**: Only checks `releases/latest` (stable releases)
- **On**: Checks `releases?per_page=5` which includes pre-release/beta/RC versions

Pre-release updates show an amber badge and disclaimer in the Update Banner. The setting is stored as `check_pre_releases: bool` in `AppSettings`.

---

## Common Build Issues

### "plugins > updater doesn't exist"
**Cause**: `createUpdaterArtifacts: true` is set but `plugins.updater` section is missing from `tauri.conf.json`.
**Fix**: Ensure `plugins.updater` exists with a valid `pubkey` and `endpoints` array.

### "No artifacts were found" (ARMv7)
**Cause**: `tauri-action` skips the build entirely when both `includeRelease: false` and `includeUpdaterJson: false`. This was the original approach for ARMv7 (because tauri-action can't match Debian's `armhf` filenames), but it prevented the build from running at all.
**Fix**: ARMv7 now bypasses tauri-action completely. The build runs directly via `npx tauri build --target armv7-unknown-linux-gnueabihf --bundles deb rpm`, and a separate step renames and uploads the artifacts manually. See the `Build ARMv7` and `Upload ARMv7 artifacts` steps in `release.yml`.

### Windows ARM64 "http status: 504"
**Cause**: Transient GitHub CDN timeout when downloading NSIS utilities (`nsis_tauri_utils.dll`). The Rust compilation succeeds but the bundling step fails.
**Fix**: Re-run the failed job. This is a network issue, not a code problem.

### Empty `TAURI_SIGNING_PRIVATE_KEY` in CI logs
**Cause**: The `TAURI_SIGNING_PRIVATE_KEY` GitHub secret hasn't been configured.
**Fix**: Add the private key contents as a repository secret. See [Required GitHub Secrets](#required-github-secrets).

### macOS "The signature does not include a secure timestamp"
**Cause**: Tauri's `tauri-macos-sign` crate omits `--timestamp` from the `codesign` command. Without it, macOS uses a non-deterministic default that may or may not produce timestamps. Apple's notarization service requires secure timestamps on all code signatures, so builds can randomly fail.
**Fix**: Both `release.yml` and `pre-release.yml` include a `codesign` wrapper step (Step 8.9) that injects `--timestamp` into every codesign invocation. The wrapper is installed to `$HOME/bin` and prepended to `$GITHUB_PATH` so it intercepts all calls before delegating to `/usr/bin/codesign`. This is a workaround for [tauri-apps/tauri#11992](https://github.com/tauri-apps/tauri/issues/11992) and can be removed once Tauri adds `--timestamp` natively. See also: [Apple: Resolving Common Notarization Issues](https://developer.apple.com/documentation/security/resolving-common-notarization-issues).

### macOS "Validate macOS signing secrets" failure
**Cause**: One or more Apple signing secrets are missing.
**Fix**: Configure all 6 Apple secrets in the repository settings.

### `cargo clean` needed after config changes
If you change `tauri.conf.json` and the build fails with cached errors, run:
```bash
cd src-tauri && cargo clean
```
This clears ~5GB of cached build artifacts and forces a fresh compilation.

---

## Version Bumping

**Normally handled by release-please**. But if you need a manual bump, these 4 files need updating:

1. `package.json` → `"version"`
2. `src-tauri/tauri.conf.json` → `"version"`
3. `src-tauri/Cargo.toml` → `version`
4. `.release-please-manifest.json` → `"."`

After updating `Cargo.toml`, regenerate the lockfile:
```bash
cd src-tauri && cargo generate-lockfile
```

Or use the automated script:
```bash
node scripts/bump-version.mjs 0.3.8
```

---

## Conserving CI Minutes

All workflows support manual triggers via `workflow_dispatch`:

```bash
# Run CI checks
gh workflow run "CI" --ref main

# Create/update Release PR
gh workflow run "Release Please" --ref main

# Regenerate changelog
gh workflow run "Changelog" --ref main

# Build release (requires tag)
gh workflow run "Release" -f tag=v0.3.8
```

During development, use `[skip ci]` in commit messages to prevent automatic workflow triggers.

---

## UI Text & Content File Locations

Where to find and edit user-facing text in the application. All labels, descriptions, and content are inline in the component files (not in separate text/resource files) unless i18n translation keys are used.

### Settings Tabs

Each settings tab is a self-contained React component. Labels, descriptions, and option arrays are defined inline.

| Tab | File |
| --- | --- |
| General | `src/components/settings/tabs/GeneralTab.tsx` |
| Quality | `src/components/settings/tabs/QualityTab.tsx` |
| Fallback | `src/components/settings/tabs/FallbackTab.tsx` |
| Paths | `src/components/settings/tabs/PathsTab.tsx` |
| Cookies | `src/components/settings/tabs/CookiesTab.tsx` |
| Lyrics | `src/components/settings/tabs/LyricsTab.tsx` |
| Cover Art | `src/components/settings/tabs/CoverArtTab.tsx` |
| Metadata | `src/components/settings/tabs/MetadataTab.tsx` |
| Templates | `src/components/settings/tabs/TemplatesTab.tsx` |
| Advanced | `src/components/settings/tabs/AdvancedTab.tsx` |

The tab list itself (names, icons, order) is the `TABS` array in `src/components/settings/SettingsPage.tsx`.

### Help Topics

Help content is embedded as JSX in the HelpViewer component, **not** loaded from external markdown files:

| File | Description |
| --- | --- |
| `src/components/help/HelpViewer.tsx` | All help topics defined in the `HELP_TOPICS` array |
| `help/*.md` | 11 external markdown files — kept for reference but **not** rendered by the app |

### Other UI Text

| Content | Location |
| --- | --- |
| Sidebar navigation labels | `src/components/layout/Sidebar.tsx` → `NAV_ITEMS` array |
| Setup wizard step content | `src/components/setup/steps/*.tsx` (6 step files) |
| Toast messages | Inline `addToast('message', 'type')` calls throughout components |
| Page headers (e.g., "Download", "Queue") | Inline in each page component's JSX |
| Update banner text | `src/components/common/UpdateBanner.tsx` |
| Updates page text | `src/components/updates/UpdatesPage.tsx` |
| Loading/error states | `src/components/common/LoadingSpinner.tsx`, inline in components |

### i18n Translation Files

When i18n is fully adopted, translatable strings live in:

| File | Description |
| --- | --- |
| `public/locales/en/translation.json` | English (default/fallback) |
| `public/locales/{lang}/translation.json` | Additional languages (e.g., `de`, `fr`) |
| `src/lib/i18n.ts` | i18next initialization and locale loading |

To translate a component: use `const { t } = useTranslation()` from `react-i18next` and replace string literals with `t('key')` calls. See the i18n section below for details.

### Capturing Screenshots

To update the README screenshots:

1. Build and run the app: `npm run tauri dev`
2. Navigate to each page and capture with your OS screenshot tool
3. Save to `assets/screenshots/` with the naming convention: `{page}-{theme}.png`
4. Expected files: `download-light.png`, `download-dark.png`, `queue-dark.png`, `settings-dark.png`, `activity-dark.png`

---

## GitHub Branch Protection

The `main` branch is protected via a GitHub Repository Ruleset (preferred over legacy branch protection rules for flexibility).

**Ruleset name:** `main-protection`

**Rules applied:**

- **Prevent force pushes** -- No one can rewrite `main` history
- **Prevent branch deletion** -- `main` cannot be deleted
- **Require CI status checks** -- PRs must pass `frontend` and `backend` CI jobs before merging

**Bypass actors:** Repository admin (allows direct pushes to `main` for the owner's workflow)

**Managing the ruleset:**

```bash
# View current rulesets
gh api repos/MWBMPartners/MeedyaDL/rulesets

# View specific ruleset details
gh api repos/MWBMPartners/MeedyaDL/rulesets/{ruleset_id}

# Or manage via GitHub UI: Settings > Rules > Rulesets
```

---

## Crash Reporting & Diagnostics

### Architecture

MeedyaDL has a three-layer crash reporting system:

1. **Local file logging** (always on) -- `tracing` ecosystem with dual output:
   - **stderr** -- Coloured, human-readable output for development
   - **Rotating file** -- `{app_data_dir}/logs/meedyadl.YYYY-MM-DD.log` (daily rotation)

2. **Local crash reports** (always on) -- JSON crash reports saved to `{app_data_dir}/crashes/`:
   - Rust panics are captured by a custom `std::panic::set_hook()` handler
   - Frontend errors (ErrorBoundary, window.onerror, unhandledrejection) are sent to Rust via the `log_frontend_error` IPC command
   - Each report includes: error message, stack trace, app version, OS, architecture, timestamp, source

3. **Sentry cloud reporting** (opt-in) -- Anonymous crash telemetry:
   - Disabled by default (`sentry_enabled: false` in settings)
   - Toggle in Settings > Advanced > Crash Reporting
   - Rust SDK (`sentry` crate) + JS SDK (`@sentry/browser`)
   - Captures panics, `tracing::error!()` events, unhandled JS exceptions
   - No personal data, download history, or account info is ever sent

### Key Crash Reporting Files

| File | Role |
| ---- | ---- |
| `src-tauri/src/lib.rs` | `setup_tracing()`, `setup_panic_handler()`, Sentry init |
| `src-tauri/src/models/crash_report.rs` | `CrashReport` struct |
| `src-tauri/src/services/crash_report_service.rs` | CRUD operations for crash report files |
| `src-tauri/src/commands/crash_reports.rs` | IPC commands (list, get, delete, export, log_frontend_error) |
| `src/main.tsx` | Frontend error handlers + `persistFrontendError()` + Sentry JS init |
| `src/components/settings/tabs/AdvancedTab.tsx` | Sentry opt-in toggle UI |

### File Locations

| Platform | Logs | Crash Reports |
| -------- | ---- | ------------- |
| macOS | `~/Library/Application Support/io.github.meedyadl/logs/` | `~/Library/Application Support/io.github.meedyadl/crashes/` |
| Windows | `%APPDATA%/io.github.meedyadl/logs/` | `%APPDATA%/io.github.meedyadl/crashes/` |
| Linux | `~/.local/share/io.github.meedyadl/logs/` | `~/.local/share/io.github.meedyadl/crashes/` |

### Tracing Configuration

The `tracing` ecosystem replaces the previous `env_logger`:

- `tracing` -- Structured diagnostics framework (compatible with `log` facade)
- `tracing-subscriber` -- Layered subscriber with `EnvFilter` for `RUST_LOG` support
- `tracing-appender` -- Non-blocking rolling file appender
- `sentry-tracing` -- Sentry integration layer (only active when opted in)

All existing `log::info!()` / `log::error!()` calls work unchanged.

### Crash Report Cleanup

On startup, crash reports older than 30 days are automatically deleted by `clear_old_reports()`.

### GitHub Issues Crash Reporting

In addition to local crash reports and optional Sentry telemetry, users can report crashes directly to the developer via GitHub Issues from **Settings > Advanced > Crash Reporting**.

#### How It Works

The `build_github_issue_url()` function in `crash_report_service.rs` constructs a pre-filled GitHub Issue URL:

1. **URL construction** -- Uses the `url` crate to build a `https://github.com/MWBMPartners/MeedyaDL/issues/new` URL with query parameters: `template=crash-report.yml`, `labels=crash-report`, `title`, and body fields pre-populated from the crash report data (error message, backtrace, app version, OS, architecture, timestamp, source).
2. **Percent-encoding** -- The `url` crate handles all percent-encoding of special characters in the URL query string automatically, ensuring the URL is valid regardless of the crash report content.
3. **Backtrace truncation** -- If the URL body would exceed **3500 characters**, the backtrace is truncated with a `... [truncated for URL length]` marker. This keeps the total URL length safely under browser and server limits (most browsers support up to ~8000 characters; GitHub accepts ~8192).
4. **Pre-filled fields** -- The issue template (`crash-report.yml`) uses YAML form fields. The URL pre-fills the title and body with structured crash data, but the user can edit everything before submitting.

#### Pre-filled URL vs. API-based Approach

The pre-filled URL approach was chosen over the GitHub API approach for several reasons:

- **No tokens required** -- The user's browser handles GitHub authentication. No OAuth tokens, personal access tokens, or server infrastructure needed.
- **Privacy-first** -- The user sees exactly what will be submitted in a `CrashReportDialog` consent modal before their browser opens. Nothing is sent without explicit user action.
- **No server dependency** -- No backend relay, no API keys to manage, no server costs.
- **Trade-off** -- Requires a GitHub account. Users without GitHub cannot submit reports this way (the PHP relay endpoint tracked in GitHub Issue [#44](https://github.com/MWBMPartners/MeedyaDL/issues/44) would address this in the future).

#### URL Length Handling Strategy

Browser and server URL length limits vary:

| Component | Approximate Limit |
| --------- | ----------------- |
| Chrome, Edge, Firefox | ~8,000-32,000 chars |
| Safari | ~80,000 chars |
| GitHub `issues/new` | ~8,192 chars (query string) |

The 3500-character truncation threshold for the backtrace ensures the total URL (base URL + title + all body fields) stays well within GitHub's limit. The truncation is applied only to the backtrace field -- the error message, metadata fields, and title are always included in full.

#### Issue Template and Label

- **Template**: `.github/ISSUE_TEMPLATE/crash-report.yml` -- YAML-based issue form with structured fields for crash data, steps to reproduce, and additional context.
- **Label**: `crash-report` -- automatically applied to all issues created via this template, enabling filtering and triage.

#### Frontend Components

- **`CrashReportSection`** (`src/components/settings/tabs/CrashReportSection.tsx`) -- Lists recent crash reports with Report and Delete buttons. Embedded in the Advanced settings tab below the Sentry toggle.
- **`CrashReportDialog`** (`src/components/settings/tabs/CrashReportDialog.tsx`) -- Privacy consent modal that shows the user exactly what data will be included in the GitHub Issue. Displays the error message, backtrace preview, and metadata. The "Open GitHub Issue" button opens the pre-filled URL in the system browser via `@tauri-apps/plugin-shell`.

#### IPC Command

- **`get_github_issue_url`** -- Tauri command in `commands/crash_reports.rs` that takes a crash report ID and returns the pre-filled GitHub Issue URL string. Registered in `lib.rs` via `generate_handler!`.

---

## 📁 Project Structure

```text
MeedyaDL/
├── src/                        # React Frontend
│   ├── App.tsx                 #    Root component with routing & event listeners
│   ├── main.tsx                #    Entry point
│   ├── components/             #    UI components
│   │   ├── common/             #    Shared: Button, Input, Modal, Toast, etc.
│   │   ├── layout/             #    Sidebar, TitleBar, StatusBar, MainLayout
│   │   ├── download/           #    DownloadForm, DownloadQueue, ActivityLog
│   │   ├── settings/           #    SettingsPage + 10 tab components + CrashReportSection, CrashReportDialog
│   │   ├── updates/            #    UpdatesPage (changelog + update actions)
│   │   ├── setup/              #    SetupWizard + 6 step components
│   │   └── help/               #    HelpViewer with inline topic rendering
│   ├── stores/                 #    Zustand state stores
│   │   ├── uiStore.ts          #    Navigation, toasts, sidebar state
│   │   ├── settingsStore.ts    #    App settings load/save
│   │   ├── downloadStore.ts    #    Queue, progress, cancel/retry/clear
│   │   ├── dependencyStore.ts  #    Tool installation status
│   │   ├── setupStore.ts       #    Setup wizard step tracking
│   │   ├── updateStore.ts      #    Update checking and notification
│   │   └── activityStore.ts    #    Subprocess output log buffer
│   ├── lib/                    #    Utility modules
│   │   ├── tauri-commands.ts   #    Type-safe IPC wrappers
│   │   ├── url-parser.ts       #    Apple Music URL detection
│   │   ├── quality-chains.ts   #    Fallback codec/resolution chains
│   │   └── i18n.ts             #    i18next initialization & locale loading
│   ├── types/                  #    TypeScript types (mirrors Rust models)
│   ├── hooks/                  #    Custom React hooks
│   │   ├── usePlatform.ts      #    Platform detection
│   │   └── useTheme.ts         #    Dark/light/auto theme override
│   └── styles/themes/          #    Platform-adaptive CSS
│       ├── base.css            #    Shared design tokens + status variables
│       ├── macos.css           #    macOS Liquid Glass
│       ├── windows.css         #    Windows Fluent
│       └── linux.css           #    Linux Adwaita
├── src-tauri/                  # Rust Backend
│   ├── Cargo.toml              #    Rust dependencies
│   ├── tauri.conf.json         #    Tauri configuration
│   └── src/
│       ├── main.rs             #    Application entry point
│       ├── lib.rs              #    Plugin, state & command registration
│       ├── commands/           #    IPC command handlers
│       │   ├── system.rs       #    Platform info
│       │   ├── dependencies.rs #    Python/GAMDL/tool management
│       │   ├── settings.rs     #    App settings
│       │   ├── gamdl.rs        #    Download queue orchestration
│       │   ├── credentials.rs  #    Secure keychain storage
│       │   ├── updates.rs      #    Update checking commands
│       │   ├── cookies.rs      #    Browser cookie extraction
│       │   ├── login_window.rs #    Embedded Apple Music login
│       │   ├── artwork.rs      #    Animated artwork download
│       │   └── crash_reports.rs#    Crash report management + frontend error logging
│       ├── models/             #    Data structures
│       │   ├── download.rs     #    Download request, state, queue status
│       │   ├── gamdl_options.rs#    All GAMDL CLI options as typed enums
│       │   ├── settings.rs     #    App configuration with defaults
│       │   ├── dependency.rs   #    Dependency status tracking
│       │   ├── music_service.rs#    Service trait (extensibility)
│       │   └── crash_report.rs #    Crash report data model
│       ├── services/           #    Business logic
│       │   ├── python_manager.rs    # Portable Python download/install
│       │   ├── gamdl_service.rs     # GAMDL CLI wrapper & subprocess
│       │   ├── dependency_manager.rs# Tool download/install per platform
│       │   ├── config_service.rs    # JSON settings + INI sync
│       │   ├── download_queue.rs    # Queue manager with fallback/retry
│       │   ├── update_checker.rs    # Version update checker
│       │   ├── cookie_service.rs    # Browser cookie extraction
│       │   ├── login_window_service.rs # Embedded Apple Music login
│       │   ├── animated_artwork_service.rs # MusicKit animated cover art
│       │   ├── apple_music_api.rs   # Shared MusicKit JWT, URL parsing, API
│       │   ├── metadata_tag_service.rs    # Post-download metadata enrichment
│       │   ├── acoustid_service.rs  # AcoustID fingerprinting (opt-in)
│       │   ├── replaygain_service.rs# ReplayGain loudness analysis (opt-in)
│       │   ├── enhanced_lyrics_service.rs # TTML → Enhanced LRC conversion
│       │   └── crash_report_service.rs # Crash report CRUD + export
│       └── utils/              #    Utility modules
│           ├── platform.rs     #    OS detection & paths
│           ├── archive.rs      #    ZIP/tar extraction
│           └── process.rs      #    GAMDL output parser & error classifier
├── public/locales/             # i18n translation files
│   ├── en/translation.json     #    English (default/fallback)
│   ├── de/translation.json     #    German (stub)
│   └── fr/translation.json     #    French (stub)
├── help/                       # Markdown help documentation (12 topics)
├── assets/screenshots/         # App screenshots for README
├── .github/
│   ├── ISSUE_TEMPLATE/         # GitHub issue templates
│   │   └── crash-report.yml    #    Crash report issue form
│   └── workflows/              # CI/CD
│       ├── ci.yml              #    Test & lint on push/PR
│       ├── release.yml         #    Build & publish releases
│       ├── release-please.yml  #    Automated version bumps & release PRs
│       └── changelog.yml       #    Auto-generate changelogs
├── scripts/                    # Utility scripts
├── index.html                  #    Vite entry HTML
├── package.json                #    Node.js config
├── tailwind.config.js          #    Tailwind CSS config
├── vite.config.ts              #    Vite bundler config
├── tsconfig.json               #    TypeScript config
├── cliff.toml                  #    Changelog generation config
├── commitlint.config.js        #    Conventional commits config
└── LICENSE                     #    MIT License
```

---

## Music Video Companion Downloads

The `music_video_companion` setting (default: `false`) enables automatic downloading of music videos as companions to audio tracks.

**Process:**

1. After each audio download completes, the enrichment pipeline's **Step 6** queries the Apple Music API
2. `fetch_music_video_relations()` in `apple_music_api.rs` batch-queries song IDs with `relate=music-videos`
3. For each song with a music video, a separate GAMDL invocation is spawned with the music video URL
4. Uses the user's video quality settings (resolution, codec priority, remux format)
5. Deduplicated by music video ID (same video linked from multiple songs won't download twice)
6. Fire-and-forget — failures logged but don't affect primary download status

**Prerequisites:** Valid MusicKit credentials (`musickit_team_id`, `musickit_key_id`, private key in OS keychain). The UI toggle in Settings > Quality > Video Quality is disabled when credentials are not configured.

**Key files:**

| File | Role |
| ---- | ---- |
| `src-tauri/src/services/apple_music_api.rs` | `MusicVideoRelation` struct, `fetch_music_video_relations()`, `build_music_video_url()` |
| `src-tauri/src/services/download_queue.rs` | `spawn_music_video_companion_inner()` — enrichment Step 6 |
| `src-tauri/src/models/settings.rs` | `music_video_companion: bool` setting field |
| `src/components/settings/tabs/QualityTab.tsx` | Toggle in Video Quality section (gated behind MusicKit credentials) |

---

## Visual Template Builder

The `TemplateBuilder` component provides an interactive chip/pill-based UI for building GAMDL file/folder naming templates, replacing plain text `<Input>` fields.

**Process:**

1. Template string (e.g., `{album_artist}/{album}`) is parsed via `parseTemplate()` into typed segments
2. Each segment renders as a removable chip (variables in accent color, literals in neutral mono)
3. `+` button opens a dropdown menu with available variables and common separators
4. "Edit Raw" toggle switches to a plain text input for power users
5. Live preview with sample metadata (e.g., "Taylor Swift/1989 (Taylor's Version)")
6. On change, segments are serialized back via `serializeTemplate()` and passed to `onChange()`

**Key files:**

| File | Role |
| ---- | ---- |
| `src/lib/template-parser.ts` | Parser, serializer, `TEMPLATE_VARIABLES`, `COMMON_LITERALS`, `SAMPLE_METADATA` |
| `src/lib/template-parser.test.ts` | 30 unit tests |
| `src/components/common/TemplateBuilder.tsx` | Visual chip builder component |
| `src/components/settings/tabs/TemplatesTab.tsx` | Consumer — 7 TemplateBuilder instances |
