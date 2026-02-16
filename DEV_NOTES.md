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
│   │   ├── settings/           #    SettingsPage + 10 tab components
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
│       │   └── artwork.rs      #    Animated artwork download
│       ├── models/             #    Data structures
│       │   ├── download.rs     #    Download request, state, queue status
│       │   ├── gamdl_options.rs#    All GAMDL CLI options as typed enums
│       │   ├── settings.rs     #    App configuration with defaults
│       │   ├── dependency.rs   #    Dependency status tracking
│       │   └── music_service.rs#    Service trait (extensibility)
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
│       │   ├── acoustid_service.rs  # AcousticID fingerprinting (opt-in)
│       │   └── replaygain_service.rs# ReplayGain loudness analysis (opt-in)
│       └── utils/              #    Utility modules
│           ├── platform.rs     #    OS detection & paths
│           ├── archive.rs      #    ZIP/tar extraction
│           └── process.rs      #    GAMDL output parser & error classifier
├── public/locales/             # i18n translation files
│   ├── en/translation.json     #    English (default/fallback)
│   ├── de/translation.json     #    German (stub)
│   └── fr/translation.json     #    French (stub)
├── help/                       # Markdown help documentation (11 topics)
├── assets/screenshots/         # App screenshots for README
├── .github/workflows/          # CI/CD
│   ├── ci.yml                  #    Test & lint on push/PR
│   ├── release.yml             #    Build & publish releases
│   ├── release-please.yml      #    Automated version bumps & release PRs
│   └── changelog.yml           #    Auto-generate changelogs
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
