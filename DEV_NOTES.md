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

## How to Trigger a Pre-Release Build

The `pre-release.yml` workflow builds installers for all 6 platforms from the `meedyadl-v2` feature branch and publishes them as a **pre-release** GitHub Release (not a draft — immediately visible, marked as pre-release).

### Option A: Push a Tag (Automated — Recommended)

The easiest way. Create a pre-release tag on `meedyadl-v2` and push it. The workflow triggers automatically.

#### In VS Code

1. Make sure you're on the `meedyadl-v2` branch (check the bottom-left status bar).

2. Open the integrated terminal (`` Ctrl+` `` or **Terminal → New Terminal**).

3. Create and push the tag:
   ```bash
   git tag v0.4.0-alpha.1
   git push origin v0.4.0-alpha.1
   ```

4. Go to **GitHub → Actions → Pre-Release** to watch the build progress.

#### One-Liner

```bash
git tag v0.4.0-alpha.1 && git push origin v0.4.0-alpha.1
```

### Option B: Manual Dispatch (via CLI)

No tag needed — just run the workflow directly:

```bash
gh workflow run "Pre-Release" --ref meedyadl-v2 -f version=0.4.0-alpha.1
```

### Option C: Manual Dispatch (via GitHub UI)

1. Go to **GitHub → Actions → Pre-Release**.
2. Click **"Run workflow"**.
3. Select the `meedyadl-v2` branch.
4. Enter the version (e.g., `0.4.0-alpha.1`) — without the `v` prefix.
5. Click **"Run workflow"**.

### Version Naming Convention

| Stage | Example | When to use |
| ----- | ------- | ----------- |
| Alpha | `v0.4.0-alpha.1` | Early development, features incomplete |
| Alpha | `v0.4.0-alpha.2` | Increment for each new alpha build |
| Beta | `v0.4.0-beta.1` | Feature-complete, still testing |
| RC | `v0.4.0-rc.1` | Release candidate, nearly ready |
| Stable | `v0.4.0` | Merged to `main`, handled by `release.yml` |

The version must match the format `X.Y.Z-(alpha|beta|rc).N`. The workflow validates this and will fail if the format is wrong.

### Deleting a Tag (If You Make a Mistake)

```bash
# Delete locally
git tag -d v0.4.0-alpha.1

# Delete from GitHub
git push origin --delete v0.4.0-alpha.1
```

Then delete the GitHub Release manually (if one was created), recreate the tag with the correct name, and push again.

### How It Works Under the Hood

- Pre-release tags (`v*-alpha.*`, `v*-beta.*`, `v*-rc.*`) trigger `pre-release.yml` automatically.
- These same tags are **excluded** from `release.yml` (via `!v*-alpha.*` etc.) so only one workflow runs.
- The workflow uses the same 6-platform build matrix and signing secrets as the main release.
- Builds are published as `prerelease: true` (not draft), so they appear on the Releases page immediately.
- The app's "Include Pre-Release Versions" setting controls whether users see these versions in the update checker.

---

## Bundled Dependencies

Release and pre-release builds bundle all external dependencies into the installer so users get a zero-download first-run experience. The installer size increases from ~10-15MB to ~200-300MB.

### How It Works

1. **CI download step**: `scripts/download-bundled-deps.sh` runs during CI builds (after `npm ci`, before `tauri build`). It downloads platform-specific binaries to `src-tauri/bundled-deps/`:
   - Python 3.12.8 (portable runtime from python-build-standalone)
   - GAMDL (pip-installed into the bundled Python)
   - FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box (binary tools)

2. **Tauri bundling**: `tauri.conf.json` includes `bundled-deps/**/*` in the `resources` section. Tauri copies them into the platform-specific installer.

3. **First-launch extraction**: `bundled_deps_service::extract_bundled_deps()` copies files from the resource directory to the app data directory, writes `.source` markers as "bundled", and creates a `.bundled_deps_extracted` marker to prevent re-extraction.

4. **Setup wizard auto-skip**: The wizard detects installed dependencies and auto-advances to the first incomplete step (usually cookies).

### Dev Builds

Dev builds (`cargo tauri dev`) don't have bundled deps — the `src-tauri/bundled-deps/` directory is gitignored and only created by CI. The normal setup wizard download flow handles dependencies for development.

### Testing Locally

To test the bundled deps flow locally:

```bash
# Download deps for your platform
bash scripts/download-bundled-deps.sh --os macos --arch aarch64 --output src-tauri/bundled-deps

# Build the app (will include bundled deps)
cargo tauri build

# Clear app data to test first-launch extraction
rm -rf ~/Library/Application\ Support/io.github.meedyadl/

# Launch the built app — deps should extract silently
```

### Manifest

The script writes a `manifest.json` recording which dependencies were successfully downloaded. The extraction service reads this to skip unavailable tools gracefully (e.g., if MP4Box wasn't available for ARM).

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

### macOS "The signature does not include a secure timestamp"

**Cause**: Tauri's `tauri-macos-sign` crate omits `--timestamp` from `codesign` calls. Per Apple's codesign man page, without this flag the behavior is non-deterministic — sometimes a timestamp is included, sometimes not. Apple's notarization service requires secure timestamps on all signed binaries.

**Fix**: Both `release.yml` and `pre-release.yml` include a PATH-based codesign wrapper (Step 8.9) that intercepts `/usr/bin/codesign` and injects `--timestamp` automatically. Remove when the upstream Tauri fix lands.

**Upstream**: [tauri-apps/tauri#11992](https://github.com/tauri-apps/tauri/issues/11992)

### Linux `native-tls` compile error: "non-exhaustive patterns"

**Cause**: `native-tls` v0.2.17 doesn't handle `Some(Protocol::Tlsv13)` on newer OpenSSL versions, causing `error[E0004]` on all Linux targets (x64, ARM64, ARMv7).

**Fix**: Update `Cargo.lock` to `native-tls` v0.2.18+ (`cargo update -p native-tls`).

### Windows bundled-deps hang: "Extracting MP4Box from NSIS installer..."

**Cause**: GPAC's NSIS installer ignores the `/S` (silent) flag and opens a GUI dialog. In headless CI, the process hangs indefinitely waiting for user input. Both Windows x64 and ARM64 builds are affected.

**Fix**: `scripts/download-bundled-deps.sh` now uses `7z` extraction instead of running the NSIS installer. 7-Zip can extract NSIS `.exe` archives directly and is pre-installed on all GitHub Windows runners.

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

## Remote Service Status (Kill Switch)

### Overview

MeedyaDL includes a remote service status system that allows developers to dynamically enable or disable individual media services (Apple Music, YouTube, BBC iPlayer, Spotify) across all deployed app instances. This is useful for:

- **Broken service backends** — Disable a service while a fix is deployed (e.g., API change, dependency update).
- **Legal takedowns** — Immediately suspend a service in response to a DMCA or cease-and-desist.
- **Planned maintenance** — Disable a service with a user-facing message explaining the downtime.
- **Global announcements** — Display an informational banner to all users (e.g., upcoming version requirement).

### How It Works

1. The app fetches `service-status.json` from the `main` branch of this repository on every launch and every 4 hours.
2. The fetched config is cached locally so the app works offline.
3. If the remote endpoint is unreachable and no cache exists, the app **fails open** (all services enabled).
4. Disabled services are blocked at two levels:
   - **Frontend**: The download form shows a warning and disables the "Add to Queue" button.
   - **Backend**: The download queue rejects items for disabled services before spawning any subprocess.

### Config File: `service-status.json`

Located at the repository root. Hosted at:

```
https://raw.githubusercontent.com/MWBMPartners/MeedyaDL/main/service-status.json
```

#### Schema

```json
{
  "version": 1,
  "updated_at": "2026-02-22T00:00:00Z",
  "services": {
    "AppleMusic": { "enabled": true, "message": null },
    "YouTube": { "enabled": true, "message": null },
    "BBCiPlayer": { "enabled": true, "message": null },
    "Spotify": { "enabled": true, "message": null }
  },
  "global_message": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | `number` | Schema version (currently `1`). Reserved for future backward-incompatible changes. |
| `updated_at` | `string` | ISO 8601 timestamp of when the config was last modified. |
| `services.<Name>.enabled` | `boolean` | Whether the service is available. Set to `false` to disable. |
| `services.<Name>.message` | `string \| null` | Optional user-facing message explaining why the service is disabled. |
| `global_message` | `string \| null` | Optional info banner shown to all users regardless of service status. |

#### Service Key Names

Service keys in the JSON must use PascalCase and match these exact strings:

| Key | Service |
|-----|---------|
| `AppleMusic` | Apple Music |
| `YouTube` | YouTube |
| `BBCiPlayer` | BBC iPlayer |
| `Spotify` | Spotify |

### How to Disable a Service

1. Edit `service-status.json` on the `main` branch.
2. Set the service's `enabled` field to `false`.
3. Optionally provide a `message` explaining the reason.
4. Commit and push to `main`.

**Example** — Disabling Apple Music:

```json
{
  "version": 1,
  "updated_at": "2026-02-22T12:00:00Z",
  "services": {
    "AppleMusic": {
      "enabled": false,
      "message": "Apple Music downloads are temporarily disabled while we update the backend. Expected fix: 24 hours."
    },
    "YouTube": { "enabled": true, "message": null },
    "BBCiPlayer": { "enabled": true, "message": null },
    "Spotify": { "enabled": true, "message": null }
  },
  "global_message": null
}
```

### How to Add a Global Announcement

Set the `global_message` field to a non-null string. This displays a blue info banner at the top of the app for all users.

```json
{
  "global_message": "MeedyaDL v0.5.0 will be required starting March 1st. Please update."
}
```

### How to Re-enable a Service

Set `enabled` back to `true` and clear the `message` (set to `null`). Commit and push to `main`.

### Propagation Timing

- Changes are live on `raw.githubusercontent.com` within minutes of pushing to `main`.
- Running app instances check every **4 hours** via `setInterval`.
- New app launches check **immediately on startup**.
- **Worst case**: A user sees the change within 4 hours of the push.

### Fail-Open Design

| Scenario | Behavior |
|----------|----------|
| Remote fetch succeeds | Use remote config, update local cache |
| Remote fetch fails, cache exists | Use cached config |
| Remote fetch fails, no cache | All services enabled (fail-open) |

### Architecture

```
service-status.json (GitHub main branch)
        |
        v
[Rust] services/service_status.rs
  - fetch_service_status() → remote → cache → all-enabled default
  - is_service_disabled() / get_service_message()
  - load_cached_status() (sync, for queue gate)
        |
        v
[Rust] commands/service_status.rs
  - check_service_status → IPC command
        |
        v
[TS] stores/serviceStatusStore.ts
  - checkStatus() calls Tauri command
  - isServiceDisabled() / getServiceMessage() / getGlobalMessage()
        |
        +→ App.tsx: startup + 4-hour interval
        +→ ServiceStatusBanner.tsx: amber/blue banners
        +→ DownloadForm.tsx: disables button + shows warning
        +→ download_queue.rs: rejects disabled services at enqueue
```

### Schema Versioning

The `version` field is reserved for future backward-incompatible schema changes. The current app expects `version: 1`. If the schema needs to change:

1. Bump `version` to `2`.
2. Update the Rust `ServiceStatusConfig` struct to handle both schemas.
3. Ship the app update before switching the remote config to the new schema.

### Testing Locally

To test the kill switch locally without modifying the remote config:

1. Build and run the app with `cargo tauri dev`.
2. Find the cached config at `{app_data_dir}/service-status-cache.json`.
3. Edit the cached file to disable a service.
4. Restart the app — it will use the modified cache until the next successful remote fetch.

Alternatively, temporarily modify `service-status.json` on a feature branch and update the fetch URL in `services/service_status.rs` to point to your branch.

---

## Internationalization (i18n)

### Current Status

The i18n **infrastructure** is fully set up (i18next, language detection, dynamic locale loading), but **no UI components currently use translation keys**. All user-facing strings are still hardcoded in English within the component files. The i18n system is ready to be adopted incrementally, component by component.

### Translation File Locations

```
public/locales/
├── en/translation.json    ← English (default/fallback) — AUTHORITATIVE
├── de/translation.json    ← German (AI-generated stub, ~5% UI coverage)
└── fr/translation.json    ← French (AI-generated stub, ~5% UI coverage)
```

- **English** (`en`) is the source of truth. All keys must exist here first.
- **German** (`de`) and **French** (`fr`) were AI-generated during initial i18n setup. They cover ~93 keys across 6 sections (`app`, `nav`, `sidebar`, `settings.general`, `updates`, `common`) — roughly 5% of the full UI. These files are **not exposed to users** in the language dropdown (removed in v0.3.22) but are kept as starting points for future translation work.

### Key Source Files

| File | Purpose |
| --- | --- |
| `src/lib/i18n.ts` | i18next initialization, language detection, locale loading |
| `src/components/settings/tabs/GeneralTab.tsx` | Language dropdown (`UI_LANGUAGE_OPTIONS`) |
| `public/locales/{lang}/translation.json` | Translation strings per language |

### Translation File Format

Each locale file is a flat-ish JSON with nested namespaces:

```json
{
  "nav": {
    "download": "Download",
    "queue": "Queue"
  },
  "settings": {
    "general": {
      "theme": "Theme",
      "themeDesc": "Choose between light and dark mode..."
    }
  },
  "common": {
    "cancel": "Cancel",
    "save": "Save"
  }
}
```

Keys use camelCase. Namespaces match the UI area (`nav`, `sidebar`, `settings`, `updates`, `common`). In components, keys are accessed as `t('nav.download')` or `t('settings.general.theme')`.

#### Plurals

i18next handles plurals with `_one` / `_other` suffixes:

```json
{
  "sidebar": {
    "updatesAvailable_one": "{{count}} Update",
    "updatesAvailable_other": "{{count}} Updates"
  }
}
```

Usage: `t('sidebar.updatesAvailable', { count: 3 })` → `"3 Updates"`

#### Interpolation

Use `{{variable}}` for dynamic values:

```json
{
  "updates": {
    "currentVersion": "Current version: v{{version}}"
  }
}
```

Usage: `t('updates.currentVersion', { version: '0.3.22' })` → `"Current version: v0.3.22"`

### How to Add a New Language

1. **Create the translation file**:

   ```text
   public/locales/{code}/translation.json
   ```

   Copy `public/locales/en/translation.json` as a starting point and translate every value (leave keys untouched).

2. **Register the locale** in `src/lib/i18n.ts`:

   ```typescript
   export const AVAILABLE_LOCALES = ['en', 'de', 'fr', '{code}'] as const;
   ```

3. **Add to the language dropdown** in `src/components/settings/tabs/GeneralTab.tsx`:

   ```typescript
   const UI_LANGUAGE_OPTIONS = [
     { value: 'auto', label: 'Auto (System)' },
     { value: 'en', label: 'English' },
     { value: '{code}', label: '{Native Language Name}' },
   ];
   ```

   Only add a language here when its translation file has **comprehensive coverage** (all user-facing strings translated). Partial translations result in a mixed-language UI.

4. **Test**: Set the language in Settings > General > Appearance > Language. The app requires a restart for full effect (the setting is cached in `localStorage` under `meedyadl-ui-language`).

### How to Add New Translation Keys

When adding new user-facing text to a component:

1. **Add the key to `public/locales/en/translation.json`** first (English is the source of truth).

2. **Add the same key to all other locale files** (`de`, `fr`, and any future languages) with translated values. If you don't have a translation, use the English string as a placeholder — the app falls back to English for missing keys anyway.

3. **Use the key in your component**:

   ```tsx
   import { useTranslation } from 'react-i18next';

   function MyComponent() {
     const { t } = useTranslation();
     return <h1>{t('mySection.myKey')}</h1>;
   }
   ```

### How to Translate an Existing Component

To migrate a component from hardcoded English strings to i18n:

1. Identify all user-facing strings in the component (labels, descriptions, button text, error messages, placeholders).

2. Add keys to `public/locales/en/translation.json` under an appropriate namespace.

3. Add the `useTranslation` hook:

   ```tsx
   import { useTranslation } from 'react-i18next';

   export function MyPage() {
     const { t } = useTranslation();
     // Replace: <h1>Settings</h1>
     // With:    <h1>{t('settings.title')}</h1>
   }
   ```

4. Add translated values to `de/translation.json`, `fr/translation.json`, etc.

5. **Do not translate**: brand names ("MeedyaDL"), technical identifiers, log messages, or developer-facing strings.

### Completing German / French Translations

The existing `de` and `fr` files only cover these sections:

- `app` — App name and subtitle
- `nav` — Sidebar navigation labels
- `sidebar` — Status text, update badges
- `settings.general` — General settings tab labels and descriptions
- `updates` — Updates page text
- `common` — Generic button labels (Cancel, Save, etc.)

To complete them, you need to add keys for every other section of the UI (all other settings tabs, download form, queue page, activity log, setup wizard, help viewer, error messages, toast notifications, etc.). Once a language covers the full UI, re-add it to `UI_LANGUAGE_OPTIONS` in `GeneralTab.tsx`.

---

## Enhanced Apple Music (MusicKit) Integration — Future Feature

### Target Version

v2.x or v3.x (TBD). This feature is tabled for a future release pending internal API infrastructure readiness.

### Current State

MeedyaDL uses Apple's MusicKit API for:

- **Animated cover art** — downloading motion artwork (FrontCover.mp4 / PortraitCover.mp4) via MusicKit catalog API + FFmpeg HLS conversion
- **Metadata enrichment** — ISRC, UPC, genre, advisory ratings, artist IDs, and artwork URLs via the Apple Music catalog API
- **Cross-platform content matching** — ISRC/UPC lookup for Smart Download's cross-service quality comparison

**Current authentication**: Users must provide their own MusicKit credentials (Team ID, Key ID, and `.p8` private key) from their own Apple Developer account. The app generates ES256-signed JWTs (developer tokens) locally.

### Why We Cannot Embed Our Own MusicKit Key

As a paid Apple Developer Programme member organisation, we investigated embedding our MusicKit `.p8` private key in the application for a seamless user experience. **This is not permitted.**

The [Apple Developer Program License Agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/) explicitly prohibits distributing authentication credentials:

- **Section 2.1** — Authentication credentials (keys, tokens) must be safeguarded and must not be shared with anyone who is not an Authorized Developer on your team.
- **Section 2.8** — You must not share access to mechanisms provided by Apple for the use of Services with any third party (except formal Service Providers under Section 2.9).

Shipping the `.p8` key inside the application binary constitutes distributing it to every user, violating both sections.

### Permitted Architecture: Server-Side Token Generation

Apple's intended architecture for non-Apple-platform apps is **server-side token generation**:

```
MeedyaDL desktop app  →  GET https://api.meedyadl.io/musickit/token  →  Server signs JWT  →  returns short-lived token
         ↓
    Uses token to call Apple Music API (amp-api.music.apple.com)
```

The `.p8` private key stays on the server. The app receives only a short-lived JWT (developer token) designed for client use.

### Implementation Plan

#### Phase 1: Serverless Token Service (Initial deployment)

Deploy a lightweight serverless function to generate MusicKit developer tokens:

- **Cloudflare Workers** (recommended initially — free tier: 100K requests/day)
- Alternatively: AWS Lambda, Vercel Edge Functions, or similar

The worker stores the `.p8` key as an encrypted environment secret, generates ES256-signed JWTs with short expiry (e.g., 1 hour), includes rate limiting and origin validation.

#### Phase 2: Internal API Integration (Long-term)

Migrate the token endpoint from Cloudflare to MWBM Partners' internal API infrastructure when ready. The app-side code remains the same — only the endpoint URL changes.

#### Phase 3: Dual-Mode Fallback

The app should support both modes:

| Mode | When used |
|------|-----------|
| **Server token** (default) | App fetches tokens from the MeedyaDL token API |
| **User-provided key** (fallback) | User provides own credentials in Settings, as today |

If the token API is unreachable, the app falls back to user-provided credentials if configured. This mirrors the fail-open pattern used by the Remote Service Status system.

### App-Side Changes Required

1. **New setting**: `musickit_token_source` — enum: `"server"` (default) | `"user_provided"`
2. **Token fetcher service**: `src-tauri/src/services/musickit_token_service.rs` — fetch tokens from API, cache until near-expiry, fall back to local generation
3. **Update `apple_music_api.rs`**: Use the token fetcher instead of always generating locally
4. **Settings UI**: Show "Using MeedyaDL's built-in access" by default, with expandable "Advanced: use your own credentials" section
5. **Setup wizard**: MusicKit credential step becomes optional (no longer required for animated artwork)

### iTunes Search API (Free Alternative for Basic Metadata)

The [iTunes Search API](https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/iTuneSearchAPI/) is free, public, and requires no authentication. It can supplement MusicKit for simpler lookups:

| Data | iTunes Search API | Apple Music API (MusicKit) |
|------|-------------------|---------------------------|
| Album artwork (static) | Yes | Yes |
| Track metadata (title, artist, album) | Yes | Yes |
| ISRC codes | **No** | Yes |
| Animated/motion artwork | **No** | Yes |
| Lossless/Atmos availability flags | **No** | Yes |

### References

- [Apple Developer Program License Agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/) — Sections 2.1, 2.8, 2.9
- [Generating Developer Tokens](https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens)
- [Apple Music API Documentation](https://developer.apple.com/documentation/applemusicapi)
- [iTunes Search API](https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/iTuneSearchAPI/)

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
