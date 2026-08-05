# MeedyaDL Developer Notes

Important notes for development, releasing, and CI/CD workflows.

---

## Programmatic Interface / API Surface (why there is no OpenAPI spec)

**MeedyaDL exposes no HTTP/REST API, so it has no OpenAPI/Swagger specification —
this is by design, not an omission.**

- The app is a **Tauri 2.0 desktop application**. Its only programmatic surface is
  the in-process **Tauri IPC command set** (`#[tauri::command]` functions invoked
  from the bundled React WebView via `invoke()`), enumerated in
  `src-tauri/src/lib.rs`'s `generate_handler![]`. IPC is transport-internal (no URLs,
  HTTP verbs, or status codes) and is only reachable from the app's own WebView — it
  is not a network endpoint, so OpenAPI cannot meaningfully describe it and there is
  no server on which to host Swagger UI. (There is no `express`/`fastify`/`axum`/
  `actix`/`utoipa` dependency anywhere — confirmed.)
- The IPC contract is instead enforced by `tools/audit-checks/check_ipc_commands.py`
  (every `#[tauri::command]` is registered and every frontend `invoke('x')` targets a
  registered command) and mirrored in the TypeScript wrappers in `src/lib/tauri-commands.ts`.
- **The API that the native apps (Apple/iOS, Android, …) will consume is a separate
  first-party backend — the MeedyaSuite / MWBM-IntAppsAPI service** (the same backend
  family behind the remote feature-availability flags and the future server-issued
  MusicKit token architecture; see "Remote Feature Availability" and "Recommended
  Production Architecture" below). That backend lives in its **own repository**, and
  any OpenAPI/Swagger spec + hostable (shared-hosting, no-Docker) Swagger UI belongs
  **there**, not in this desktop-app repo. Do not generate an OpenAPI document for the
  Tauri IPC surface — it would misrepresent an in-process interface as a web API.

---

## Package Manifests

MeedyaDL has two package manifests that define dependencies for different layers:

### `src-tauri/Cargo.toml` — Rust Backend Dependencies

Defines all Rust crates used by the Tauri backend. These are compiled into the native binary. Key categories:

- **Tauri framework & plugins** — application shell, IPC, native capabilities
- **Serialisation** (serde, serde_json) — JSON for IPC and file persistence
- **Async runtime** (tokio) — concurrent I/O for downloads and subprocess management
- **HTTP client** (reqwest) — downloading tools, querying APIs
- **Cryptography** (sha2, jsonwebtoken) — checksum verification, MusicKit JWT signing
- **Audio** (mp4ameta, rusty-chromaprint, symphonia) — metadata tagging, fingerprinting
- **Logging** (tracing, sentry) — structured diagnostics, optional error tracking

Version bumped automatically by release-please. Do not edit the version manually.

### `package.json` — Frontend Dependencies

Defines npm packages used by the React/TypeScript frontend. Key categories:

- **UI framework** (react, react-dom, zustand) — component rendering, state management
- **Styling** (tailwindcss, postcss) — utility-first CSS
- **Content** (react-markdown, rehype-sanitize, remark-gfm) — help page rendering
- **Internationalisation** (i18next, react-i18next) — translation system
- **Build tools** (vite, typescript, vitest) — in devDependencies, not shipped

Version bumped automatically by release-please. Do not edit the version manually.

### `ACKNOWLEDGEMENTS.md` — Auto-Generated

Lists all currently enabled/shipping dependencies with licence links. Generated from both manifests plus `engines.toml`:

```bash
node scripts/generate-acknowledgements.mjs
```

Run this whenever engines are enabled/disabled or dependencies change.

---

## Release Workflow

### There Are 7 Separate Workflows — Don't Confuse Them

| Workflow | Trigger | What It Does | Produces Binaries? |
| -------- | ------- | ------------ | ------------------ |
| **CI** (`ci.yml`) | Every push to `main` | Runs `cargo check`, `cargo test`, `npm test`, `npm type-check` | **No** — just checks code compiles and tests pass |
| **Release Please** (`release-please.yml`) | Every push to `main` | Creates or updates a "Release PR" that bumps version numbers | **No** — just creates/updates a PR |
| **Release** (`release.yml`) | Tag push (`v*`) or manual `workflow_dispatch` | Builds the app on all 6 platforms | **Yes** — this is the only workflow that produces installable binaries |
| **Changelog** (`changelog.yml`) | Tag push (`v*`) or manual `workflow_dispatch` | Regenerates `CHANGELOG.md` via git-cliff | **No** — just updates the changelog file |
| **Nightly Release** (`nightly-release.yml`) | Cron `0 0 * * *` (daily 00:00 UTC) or manual `workflow_dispatch` | Merges `feat/*` branches into `nightly`, bumps version to `X.Y.Z-nightly.YYYYMMDD`, pushes tag to trigger `release.yml` | **Yes** — via the tag it pushes |
| **Apply Branch Rulesets** (`apply-branch-rulesets.yml`) | Push to `.github/rulesets/*.json` or manual `workflow_dispatch` | Idempotently applies every ruleset in `.github/rulesets/` via the GitHub API | **No** — repo-config only |
| **Auto-Delete Merged Branches** (`auto-delete-merged-branches.yml`) | `pull_request` closed (merged) | Deletes merged PR head branches except the protected channels | **No** — repo-config only |

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

   | Type | Version Bump | Changelog | When to use |
   |------|-------------|-----------|-------------|
   | `feat:` | Minor (0.X.0) | Features | User-visible new functionality only |
   | `fix:` | Patch (0.0.X) | Bug Fixes | Bug fixes and internal changes worth releasing |
   | `refactor:` | None | Improvements | Internal restructuring, ships with next fix/feat |
   | `perf:` | None | Improvements | Performance improvements |
   | `test:` | None | Improvements | Test additions/changes |
   | `chore:` | None | Hidden | Build, deps, infrastructure, config, icons |
   | `docs:` | None | Hidden | Documentation only |
   | `ci:` | None | Hidden | CI/CD workflow changes |

   **Key rule:** `feat:` is ONLY for user-visible features. Internal work uses `fix:` (if it should trigger a patch release) or `chore:`/`refactor:` (if it can ship with the next release).

   **Do NOT use `[skip ci]`** in commit messages unless explicitly instructed.

2. **Push to main**. Two things happen automatically:
   - CI runs (verifies code is good)
   - Release Please creates/updates a Release PR (you can ignore it until ready)

3. **When ready to release**: go to GitHub and **merge the Release PR**. This triggers the full build pipeline.

4. **Wait for builds** (~15-20 minutes). Check the Actions tab to monitor progress.

5. **Publish the draft release** on GitHub once all builds succeed.

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

### Feature Availability (Optional)

| Secret | Description |
|--------|-------------|
| `INTAPPS_BASE_URL` | Base URL of the feature-availability backend, embedded at compile time via `option_env!(...)` (same pattern as `ACOUSTID_API_KEY`). |
| `INTAPPS_APP_ID` | Application identifier for the feature-availability backend, embedded at compile time via `option_env!(...)` (same pattern as `ACOUSTID_API_KEY`). |
| `INTAPPS_API_KEY` | API key for the feature-availability backend, embedded at compile time via `option_env!(...)` (same pattern as `ACOUSTID_API_KEY`). The key is extractable from shipped binaries by anyone holding the app, so it functions as attribution / abuse-filtering, not as a security boundary. |

If **any** of the three is unset at build time, the feature-availability client is completely inert: no network call is attempted, no error is logged, and every feature resolves as enabled. Forks and local builds therefore work fully with zero configuration. **Never put real values, hostnames, or wire header names in this file — env-var names and the `option_env!()` mechanism only.**

### MusicKit (Optional, End-User Enablement)

| Secret | Description |
|--------|-------------|
| `MUSICKIT_DEVELOPER_TOKEN` | Apple Music developer token embedded at compile time via `option_env!("MUSICKIT_DEVELOPER_TOKEN")`. Enables MusicKit-powered metadata/artwork features for end users who do not have Apple Developer credentials. Prefer embedding a pre-generated developer token, not Team ID/Key ID/private key. Rotate before expiry and treat as sensitive (extractable from binaries). |

### Release Please

| Secret | Description |
|--------|-------------|
| `RELEASE_PAT` | Personal Access Token with `repo` scope. Used instead of `GITHUB_TOKEN` so that tag pushes trigger the Release workflow (GitHub's `GITHUB_TOKEN` doesn't trigger other workflows). |

### Build-time (non-secret) environment variables

Not everything embedded via `option_env!()` at compile time is sensitive — the table below covers variables that are safe to name publicly (unlike the `INTAPPS_*` confidentiality rule above, which applies specifically to endpoint/key material).

| Variable | Description |
|----------|-------------|
| `MEEDYADL_CHROME_MAJOR` | Chrome major version number (e.g. `"131"`) embedded into the Group C browser User-Agent strings (`browser_user_agent()` in `src-tauri/src/utils/http_client.rs`) used for generic third-party requests (Odesli, PyPI, etc.) on Windows and Linux. Set by a best-effort step in `release.yml` that queries Google's public [VersionHistory API](https://versionhistory.googleapis.com/v1/chrome/platforms/win/channels/stable/versions?pageSize=1) for the current stable Windows Chrome release, extracts the major version, and exports it via `$GITHUB_ENV`. If the fetch fails, times out, or returns something that doesn't look like a clean 2-4 digit number, the step logs a warning and moves on — it can never fail the build. Local dev builds, forks, and CI/PR builds never set this variable, and fall back to the compiled-in `CHROME_MAJOR_FALLBACK` constant, so the mechanism is entirely opt-in and zero-config by default. This is safe to document here (unlike `INTAPPS_*`) because it queries a public Google API with no auth, and the resulting value ships visibly in plaintext inside every binary's User-Agent header anyway. |

---

## Remote Feature Availability (Developer Notes)

MeedyaDL can resolve, per-feature, whether a first-party backend has temporarily marked a feature unavailable. See `INTAPPS_*` in [Required GitHub Secrets](#required-github-secrets) above for the build-time credentials; without them the client is entirely inert.

### Consuming a flag

- **Backend**: `services::feature_flag_service::current(&app)` returns the resolved `FeatureFlagsSnapshot` synchronously without touching the network. `is_enabled(&snapshot, key)` and `notice_for(&snapshot, key)` are the two accessors call sites should use.
- **Frontend**: `useFeatureFlagStore` (`src/stores/featureFlagStore.ts`) holds the snapshot; `selectNoticeEntries(snapshot)` is the pure selector that turns it into user-facing notice entries, consumed today by `FeatureNoticeBanner` (`src/components/common/FeatureNoticeBanner.tsx`, mounted in `MainLayout.tsx`).

### Invariants — do not regress these

1. **Derive UI only from `snapshot.verdicts`, never `snapshot.meta`.** `meta` is diagnostics (source / failure count / last error) and must never drive a toast, banner, or any other on-screen artefact.
2. **Render server notice text as plain text only** — never `dangerouslySetInnerHTML`, never a markdown renderer, never assigned to `.innerHTML`. Untrusted remote content.
3. **Enforce at operation start, never mid-operation.** A feature check belongs at the point an operation begins, not partway through it.
4. **The disk cache deliberately never expires — do not add a TTL.** Compiled defaults are all-enabled, so an expiring cache would silently re-enable a feature that was deliberately switched off the moment a user went offline long enough. Staleness is the correct trade-off here, not a bug.
5. **Two capabilities are structurally ungateable**: the availability check itself, and the updater. Neither can be switched off by a server payload, by design — otherwise a compromised publishing account could permanently blind installed copies with no way to recover.

### Flags never gate licensing or entitlement

This mechanism exists for operational and legal pauses (e.g. investigating an upstream change), never for licensing or entitlement decisions. Nothing in MeedyaDL is designed to be "unlocked" or "locked" by a remote flag. Client-side enforcement on a user's own machine is advisory by nature — it should never be documented or designed as though it were a guarantee against a determined user.

### Confidentiality rule for docs

User-facing documentation (README, TERMS, SECURITY, `help/*`) must never contain a hostname, endpoint path, wire header name, key material, cache filename, refresh interval, or the phrase "kill switch" — the vocabulary is "we pause a feature" / "the app shows a notice" / "it comes back automatically". This file (`DEV_NOTES.md`) is public and may name the `INTAPPS_*` env vars and the `option_env!()` mechanism, and nothing else about the transport.

---

## Signing Keys & Auto-Updates

---

## MusicKit Credential Validation (Issue #161)

### Problem

Users reported repeated `HTTP 401` failures when testing MusicKit credentials in **Settings > Advanced > API Credentials**, even after regenerating private keys. This created the perception that MusicKit integration was broken end-to-end.

### Why It Was a Problem

1. The **Test Credentials** action could validate stale persisted IDs instead of the exact values currently typed into the form.
2. Team ID / Key ID normalization was weak (whitespace/casing could pass UI entry but fail auth semantics).
3. Runtime MusicKit features depended on user credentials only, which is a poor default for non-developer end users.

### What Changed

1. Credential testing now validates the current UI Team ID/Key ID values passed directly to the backend command.
2. Team ID and Key ID are normalized (trim + uppercase) and validated as strict 10-character alphanumeric IDs.
3. Validation now probes both `amp-api.music.apple.com` and `api.music.apple.com` for clearer auth diagnostics.
4. Runtime MusicKit API callers now resolve tokens via:
   - user Team ID + Key ID + private key, or
   - embedded `MUSICKIT_DEVELOPER_TOKEN` fallback.
5. Settings UI now surfaces when a build-time MusicKit token is embedded so Apple Developer credentials are optional for most end users.

### Recommended Production Architecture (Option 2: Server-Issued Token)

If legal/compliance review permits MusicKit usage for this app, the safest
operational model is to mint developer tokens on a backend and send short-lived
tokens to clients. Do not ship `.p8` private keys in desktop builds.

#### Implementation Outline

1. Build a token service (single endpoint, for example `POST /musickit/token`).
2. Store Team ID, Key ID, and `.p8` private key only in server-side secret storage.
3. On request, server signs a MusicKit JWT and returns `token` + `expires_at`.
4. App caches token in memory and refreshes before expiry (e.g. 5-10 minute buffer).
5. All MusicKit API callers (artwork, metadata enrichment, music-video lookup, validator)
   use one shared token provider abstraction.
6. Add abuse controls: rate limiting, telemetry, and key/token rotation runbook.

#### Fallback Strategy

- Dev/local: user Team ID + Key ID + private key in OS keychain.
- Release without backend: optional embedded `MUSICKIT_DEVELOPER_TOKEN`
  (higher extraction/abuse risk, rotate aggressively).
- Preferred release: server-issued short-lived token.

#### 401 Scope Clarification

Issue #161 was not only the **Test Credentials** workflow. Runtime MusicKit
paths were also updated (`animated_artwork_service`, `metadata_tag_service`,
`download_queue` music-video relation lookup) so token resolution is consistent
during real downloads/enrichment.

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

## Release Channels

MeedyaDL ships across six long-lived channels, ordered from least to most stable:

| Channel | Branch | Tag format | Cadence | Audience |
| ------- | ------ | ---------- | ------- | -------- |
| Nightly | `nightly` | `vX.Y.Z-nightly.YYYYMMDD` | Daily 00:00 UTC | Developers validating today's `feat/*` integrations |
| Weekly | `weekly` | `vX.Y.Z-weekly.YYYYWW` | Weekly Sunday 00:00 UTC (planned) | Testers willing to trial a week's worth of nightlies |
| Monthly | `monthly` | `vX.Y.Z-monthly.YYYYMM` | Monthly 1st 00:00 UTC (planned) | Early adopters wanting monthly preview builds |
| Alpha | `alpha` | `vX.Y.Z-alpha.N` | Ad-hoc | Feature-complete previews |
| Beta | `beta` | `vX.Y.Z-beta.N` | Ad-hoc | Release candidates |
| Stable | `main` | `vX.Y.Z` | Release-please PR merges | End users |

All six channel branches are **protected against deletion and non-fast-forward pushes** via `.github/rulesets/protected-release-branches.json`.

### Channel auto-merge pipeline

Each channel's source branch is refreshed from the one directly below it plus any ready `feat/*` branches, preserving the stability ladder:

```
feat/* ─→ nightly ─→ weekly ─→ monthly ─→ alpha ─→ beta ─→ main (stable)
```

`nightly-release.yml` is the live implementation of the first hop: it resets `nightly` to `main`, merges every `origin/feat/*` branch (skipping any that conflict and opening an issue listing them), bumps the version in `package.json` / `tauri.conf.json` / `Cargo.toml`, force-pushes `nightly`, and creates an annotated tag. The tag push triggers `release.yml`, which produces the platform installers. Weekly and monthly use the same pattern with their own crons (`0 0 * * 0` and `0 0 1 * *`).

### In-app update guard (option 2)

The app filters and enforces the channel on the client:

- `UpdateChannel` enum in `src-tauri/src/models/settings.rs` — ordered `Nightly < Weekly < Monthly < Alpha < Beta < Stable`.
- `UpdateChannel::from_tag()` parses the pre-release suffix of any tag (`"-nightly.20260420"` → `Nightly`, `"-beta.1"` → `Beta`, no suffix → `Stable`).
- `update_channel: UpdateChannel` is persisted in `AppSettings`, exposed as the **Update Channel** dropdown under *Settings > General > Updates*.
- `check_all_updates` filters the GitHub releases list to the user's channel, so a Stable user never sees Nightly entries and vice-versa. A Stable user fetches `releases/latest`; any other channel fetches `releases?per_page=20` and picks the first entry matching the selection.
- `download_and_install_app_update` refuses to install a tag whose channel is **less stable** than the user's current selection. This is the enforcement point: even if a cross-channel URL reaches the installer (deep link, stale cache, or manifest tampering), the installer returns a clear error instead of downgrading the user's stability tier. Switching to a less-stable channel is always an explicit action in Settings.

The legacy `check_pre_releases: bool` setting still exists and is implicitly enabled whenever `update_channel != Stable` — it controls which GitHub endpoint the checker hits, but the channel drives which release is actually surfaced and installable.

### Branch protection + auto-delete

- `.github/rulesets/protected-release-branches.json` blocks deletion and non-fast-forward pushes on `main`, `beta`, `alpha`, `monthly`, `weekly`, `nightly`. Apply (or re-apply) with `gh workflow run "Apply Branch Rulesets" --ref main`, or through **Actions → Apply Branch Rulesets → Run workflow** in the GitHub UI.
- `auto-delete-merged-branches.yml` deletes merged PR head branches (so `feat/*` and `fix/*` don't accumulate), while the same six channel names in its `case` are exempted as a soft guardrail. The ruleset is the hard guarantee — the workflow is just quieter.
- Requires a `RELEASE_PAT` with `administration:write` to apply rulesets.

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
| `src/components/help/HelpViewer.tsx` | All help topics defined in the `HELP_TOPICS` array (15 entries; ids are not 1:1 with filenames) |
| `help/*.md` | 16 files (15 topics + `index.md`) — kept for reference but **not** rendered by the app. Any content change must ALSO be made in the matching inline `HELP_TOPICS` entry. |

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

│   │   ├── download/           #    DownloadForm, DownloadQueue, ActivityLog, QueueItem, HistoryPage, GlobalProgressBar

│   │   ├── settings/           #    SettingsPage + 10 tab components + CrashReportSection, CrashReportDialog, DevToolsSection

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

│   │   ├── useTheme.ts         #    Dark/light/auto theme override

│   │   ├── useKeyboardShortcuts.ts # Cmd/Ctrl+D, Cmd+,, Cmd+Q

│   │   ├── useKonamiCode.ts    #    Dev access mode activation

│   │   └── useClipboardMonitor.ts # Clipboard URL detection

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

│       │   ├── credentials.rs  #    Secure keychain storage + dev access

│       │   ├── updates.rs      #    Update checking commands

│       │   ├── cookies.rs      #    Browser cookie extraction

│       │   ├── login_window.rs #    Embedded Apple Music login

│       │   ├── artwork.rs      #    Animated artwork download

│       │   ├── crash_reports.rs#    Crash report management + frontend error logging

│       │   ├── api_audit.rs    #    API field audit diagnostic

│       │   ├── history.rs      #    Download history queries

│       │   └── clipboard.rs    #    System clipboard reading

│       ├── models/             #    Data structures (15 files)

│       │   ├── download.rs     #    Download request, state, queue status

│       │   ├── gamdl_options.rs#    All GAMDL CLI options as typed enums

│       │   ├── settings.rs     #    App configuration with defaults

│       │   ├── dependency.rs   #    Dependency status tracking

│       │   ├── media_service.rs#    MediaServiceId enum (5 services)

│       │   ├── crash_report.rs #    Crash report data model

│       │   ├── codec_registry.rs#   Universal codec definitions (codecs.toml)

│       │   ├── tag_registry.rs #    Metadata tag definitions (tags.toml)

│       │   ├── manifest.rs     #    .meedyadl manifest schema

│       │   ├── content_match.rs#    Content matching/search results

│       │   ├── service_status.rs#   Service health/status state

│       │   ├── votify_options.rs#   Spotify (votify) CLI options stub

│       │   ├── ytdlp_options.rs#    yt-dlp CLI options stub

│       │   └── get_iplayer_options.rs# BBC iPlayer CLI options stub

│       ├── services/           #    Business logic (32 files)

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

│       │   ├── crash_report_service.rs # Crash report CRUD + export

│       │   ├── webvtt_service.rs    # WebVTT subtitle generation

│       │   ├── rich_srt_service.rs  # Rich SRT with styling tags

│       │   ├── ass_subtitle_service.rs # ASS subtitle generation

│       │   ├── musicbrainz_service.rs # MusicBrainz video discovery

│       │   ├── history_service.rs   # Download history persistence

│       │   ├── health_check_service.rs # System health monitoring

│       │   ├── api_audit_service.rs # API field audit diagnostic

│       │   ├── mediainfo_service.rs # MediaInfo CLI integration

│       │   ├── pip_engine_service.rs# Python pip installation management

│       │   ├── clipboard_service.rs # System clipboard operations

│       │   ├── engine_registry.rs   # Download engine registry (engines.toml)

│       │   ├── engine_runner.rs     # Service-agnostic subprocess spawning

│       │   ├── bpm_service.rs       # BPM/tempo detection

│       │   ├── smart_download.rs    # Intelligent download orchestration

│       │   ├── service_status.rs    # Service operational status

│       │   └── integration_tests.rs # Backend integration tests

│       └── utils/              #    Utility modules

│           ├── platform.rs     #    OS detection & paths

│           ├── archive.rs      #    ZIP/tar extraction + SHA-256 verification

│           ├── process.rs      #    GAMDL output parser & error classifier

│           ├── activity_log.rs #    Shared activity log emission helpers

│           └── rate_limiter.rs #    Sliding-window IPC rate limiter

├── public/locales/             # i18n translation files

│   ├── en/translation.json     #    English (default/fallback)

│   ├── de/translation.json     #    German (stub)

│   └── fr/translation.json     #    French (stub)

├── help/                       # Markdown help documentation (12 topics)

├── assets/screenshots/         # App screenshots for README

├── .github/
│   ├── ISSUE_TEMPLATE/         # GitHub issue templates

│   │   └── crash-report.yml    #    Crash report issue form

│   └── workflows/              # CI/CD (7 workflows)

│       ├── ci.yml              #    Test & lint on push/PR

│       ├── release.yml         #    Build & publish releases

│       ├── release-please.yml  #    Automated version bumps & release PRs

│       ├── changelog.yml       #    Auto-generate changelogs

│       ├── codeql.yml          #    CodeQL static analysis

│       ├── dependency-report.yml#   Monthly dependency audit

│       └── fix-updater-manifest.yml # Standalone updater manifest repair

├── scripts/                    # Utility scripts

├── index.html                  #    Vite entry HTML

├── package.json                #    Node.js config

├── tailwind.config.js          #    Tailwind CSS v4 config (referenced via @config in globals.css)

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

### Music-Video Filename & Folder Resolution (#527 #531 #537)

Music video placement on disk is resolved through a four-tier cascade, highest priority first. This section is the spec — any change must update the precedence, the constants, AND the unit tests.

| Tier | Source | Where it plays in | Folder path | Filename pattern | Unique? |
|------|--------|-------------------|-------------|------------------|---------|
| 1 | GAMDL's **iTunes Lookup** (`interface_music_video.py:80`) with album linkage | Inside GAMDL (`tags.album` populated → `album_folder_template`) | `{album_artist}/{album}/` | `{track:02d} {title}` or `{disc}-{track:02d} {title}` | Yes (disc/track disambiguate) |
| 2 | **Apple Music Catalog API** `music-videos/{id}?include=albums` | Not yet wired — tracked as follow-up in #537 | `{album_artist}/{album}/` (pre-filled into `no_album_folder_template` before GAMDL runs) | `{title} ({title_id})` (same last-resort file template; album-aware override planned) | Yes |
| 3 | **MeedyaDL-known parent album** — when the MV is discovered via `fetch_music_video_relations()` for album X, we already know it belongs to album X | Not yet wired — tracked in #537 | `{album_artist}/{album}/` from our known parent context | `{title} ({title_id})` | Yes |
| 4 | **Fallback** (`MV_NO_ALBUM_FOLDER_TEMPLATE` + `MV_NO_ALBUM_FILE_TEMPLATE` in `download_queue.rs`) | Reached only when all three above fail or the MV genuinely has no album (standalone promo MV) | `{artist}/Music Videos/` | `{title} ({title_id})` — `{title_id}` is Apple Music's numeric MV ID, **guaranteed unique** and **deterministic across re-downloads** | Yes |

**Why `{title_id}` and not a datetime?** `{title_id}` is the Apple Music MV numeric ID — the same ID every time, so re-downloads of the same MV dedupe correctly under GAMDL's `overwrite=false`. A datetime suffix would cause every re-download to create a new file, silently multiplying on-disk copies.

**Why override `no_album_*` templates for MVs only (not globally)?** The user's `no_album_folder_template` and `no_album_file_template` are audio-oriented. Legacy MeedyaDL installs (pre-v0.38 settings) shipped `"{artist}/[Unknown]"` + `"{disc} - "` as those defaults — which for MVs without `{disc}` produces the catastrophic `Artist/[Unknown]/-.mp4` output that motivated #527 in the first place. Forcing a fixed MV-safe pair means an audio download still honours the user's no-album choices while MVs get guaranteed-sane output regardless of settings hygiene.

**Tiers 2 and 3 are scoped out of the initial RC fix.** The current PR covers Tier 4 only (the safety net). Tier 1 already works natively in GAMDL. Tiers 2 and 3 are tracked in #537 and will land as a separate PR once the Apple Music Catalog MV endpoint and the parent-album threading are implemented. Landing Tier 4 alone already resolves the RC blocker — MVs that fell into the buggy `[Unknown]` path now land in a predictable `{artist}/Music Videos/` folder with unique `{title} ({title_id}).mp4` filenames.

**Key constants & locations:**

| Location | Role |
|----------|------|
| `src-tauri/src/services/download_queue.rs` → `MV_NO_ALBUM_FOLDER_TEMPLATE` | Tier 4 folder template |
| `src-tauri/src/services/download_queue.rs` → `MV_NO_ALBUM_FILE_TEMPLATE` | Tier 4 file template (includes `{title_id}` for uniqueness) |
| `src-tauri/src/services/download_queue.rs` → `download_music_video_by_url()` | Applies Tier 4 overrides to the GAMDL invocation for MVs |
| `src-tauri/src/services/config_service.rs` → `migrate_settings()` v2→v3 | Heals legacy broken `no_album_*` defaults on upgrade |
| `src-tauri/src/services/apple_music_api.rs` → `fetch_music_video_relations()` | Currently returns MV ID + name only; to be extended for Tier 2 (`include=albums`) |

---

## Animated Cover Art (Motion Artwork)

MeedyaDL downloads animated cover art (motion artwork) from Apple Music after album downloads complete. These are short looping HEVC H.265 videos that Apple displays on the "Now Playing" screen.

### Output Files

| File | Aspect Ratio | Max Resolution | Source API Field |
|------|-------------|----------------|-----------------|
| `FrontCover.mp4` | 1:1 (square) | 3840x3840 | `editorialVideo.motionDetailSquare.video` |
| `FrontCoverPortrait.mp4` | 3:4 (portrait) | 2048x2732 | `editorialVideo.motionDetailTall.video` |
| `ArtistSpotlightCover.mp4` | 16:9 (landscape) | artist-dependent | `editorialVideo.motionArtistFullscreen16x9.video` (preferred) or `editorialVideo.motionArtistWide16x9.video` (fallback) — queried from the artist endpoint, saved to the artist folder |

> **Filename rationale:** `FrontCover` + `FrontCoverPortrait` are the two orientations of the same album cover and sort adjacent alphabetically, so they stay visually paired in any file browser. `ArtistSpotlightCover` is intentionally narrower than the `motionArtist*` fallback chain used in earlier versions — lower-tier fallbacks (`motionDetailSquare` / `motionDetailTall`) are tightly cropped around cover art and look visually wrong as an artist-page hero, so we skip the download rather than substitute a mismatched source.

> **Legacy filename migration:** Pre-v0.39 releases wrote the portrait variant as `PortraitCover.mp4`. We do **not** auto-rename existing files — renaming without consent is risky (the user may have built scripts or media-player presets around the legacy name). Freshly downloaded albums get the new name; re-downloads leave the old file untouched (GAMDL-style `overwrite=false`). Users who want a clean sweep can delete `PortraitCover.mp4` before re-running animated artwork on an album.

Both are saved as sidecar files alongside downloaded audio in the album directory.

### Authentication — No Wrapper Required

Animated artwork uses the **Apple Music catalog API**, which authenticates via MusicKit Developer Tokens (ES256-signed JWTs). This is completely independent of the wrapper, which provides alternative Apple ID login for audio DRM decryption.

**Two-tier credential resolution** (`resolve_musickit_developer_token()` in `apple_music_api.rs`):

1. **User credentials (priority):** Team ID + Key ID (in `settings.json`) + private key (in OS keychain) → generates fresh 1-hour JWT
2. **Embedded token (fallback):** Compile-time `MUSICKIT_DEVELOPER_TOKEN` env var → allows users without Apple Developer accounts to use the feature

### API Flow

```
GET https://api.music.apple.com/v1/catalog/{storefront}/albums/{album_id}
    ?include=tracks,artists&extend=editorialVideo
Authorization: Bearer {JWT}
```

The `editorialVideo` extension returns HLS M3U8 playlist URLs for square and portrait variants. FFmpeg downloads these streams:

```bash
ffmpeg -i {m3u8_url} -c copy -movflags +faststart -y -loglevel warning {output_path}
```

- `-c copy` — stream copy, no re-encoding (preserves original HEVC quality)
- `-movflags +faststart` — moov atom at start for fast playback

### File Hiding

When `hide_animated_artwork` is `true` (default), downloaded files are hidden via OS-native mechanisms:

| Platform | Mechanism | Original Filename Preserved |
|----------|-----------|---------------------------|
| macOS | `chflags hidden` | Yes |
| Windows | `attrib +H` | Yes |
| Linux | `.` prefix rename | No (becomes `.FrontCover.mp4`) |

**Linux limitation:** Media players looking for `FrontCover.mp4` by exact name won't find `.FrontCover.mp4`.

### Enrichment Pipeline Integration

Animated artwork is **Step 3** (stage 8) in the enrichment pipeline. The `AlbumMetadata` response (with `extend=editorialVideo`) is fetched **once** in Step 1 and shared across metadata tagging, artwork download, and music video companion lookup — no duplicate API calls.

Runs in a separate `tokio::spawn` task (non-blocking). Shutdown-aware (checked between enrichment stages).

### Graceful Degradation

The feature silently succeeds with no output (returns `Ok(empty_result())`) when:

- Feature disabled in settings (`animated_artwork_enabled: false`)
- No MusicKit credentials configured (and no embedded token)
- URL is not an album (single track, playlist, music video)
- Album has no animated artwork available (most older/lower-profile albums)
- FFmpeg not installed

Actual errors (API failures, network issues) are logged at `warn!` level.

### Settings

| Setting | Type | Default | Location |
|---------|------|---------|----------|
| `animated_artwork_enabled` | `bool` | `false` | Settings > Cover Art |
| `hide_animated_artwork` | `bool` | `true` | Settings > Cover Art (conditional) |
| `musickit_team_id` | `Option<String>` | `None` | Settings > Advanced > API Credentials |
| `musickit_key_id` | `Option<String>` | `None` | Settings > Advanced > API Credentials |
| Private key | OS keychain | — | Settings > Advanced > "Save to Keychain" |

### Key Files

| File | Role |
|------|------|
| `src-tauri/src/services/animated_artwork_service.rs` | Main service: credential check, API query, HLS download, file hiding |
| `src-tauri/src/services/apple_music_api.rs` | Shared: JWT generation, keychain access, catalog API client |
| `src-tauri/src/services/download_queue.rs` | Enrichment integration (Step 3 of pipeline) |
| `src-tauri/src/commands/artwork.rs` | IPC command: manual artwork download |
| `src/components/settings/tabs/CoverArtTab.tsx` | UI: toggles and settings |
| `src/components/settings/tabs/AdvancedTab.tsx` | UI: MusicKit credentials, "Test Credentials" button |
| `help/animated-artwork.md` | User documentation: setup and troubleshooting |

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

---

## GAMDL 2.9.1 CLI Flag Changes

GAMDL 2.9.1 **removed** the `--song-codec` CLI flag. Only `--song-codec-priority` exists now (accepts a comma-separated codec list).

**Impact on MeedyaDL:**

- Companion tier downloads: changed from `--song-codec ac3` to `--song-codec-priority ac3` (single-element priority)
- Fallback retries (`try_fallback()`): same change — `song_codec = None`, `song_codec_priority = Some(single_codec)`
- Primary downloads: already used `--song-codec-priority` for native priority chains
- The `song_codec` field in `GamdlOptions` is kept for internal codec tracking (suffix determination, enrichment tags) but is no longer emitted as a CLI flag when `song_codec_priority` is also set

**CLI arg generation** (`gamdl_options.rs` `audio_cli_args()`):

```rust
if let Some(ref priority) = self.song_codec_priority {
    args.push("--song-codec-priority");  // Takes precedence
} else if let Some(ref codec) = self.song_codec {
    args.push("--song-codec");  // Legacy fallback (GAMDL < 2.9.1 only)
}
```

---

## Enrichment Pipeline — Blocking I/O Fix (v0.6.2)

The post-download enrichment pipeline runs inside `tokio::spawn(async move {...})`. Four services (`metadata_tag_service`, `enhanced_lyrics_service`, `acoustid_service`, `replaygain_service`) called `mp4ameta` Tag::read_from_path / Tag::write_to_path — blocking synchronous file I/O that starved the tokio async runtime on slow FUSE mounts (CloudMounter, NFS), freezing the UI.

**Fix (two layers):**

1. Tag I/O wrapped in `tokio::task::spawn_blocking()` in all 4 services
2. `tokio::task::yield_now().await` added between all enrichment steps

`enhanced_lyrics_service::process_enhanced_lyrics_for_directory()` was changed from `async fn` to `fn` (had zero `.await` calls). Its call site in `download_queue.rs` wraps it in `spawn_blocking()`.

---

## Codec Registry (`codecs.toml`)

**File location:** `src-tauri/codecs.toml`

The codec registry defines all audio/video codecs and lyrics/subtitle formats that MeedyaDL supports or plans to support. It is compiled into the binary at build time via `include_str!` — no runtime file I/O.

### File Structure

The TOML file has four top-level sections:

| Section | Purpose | Example ID |
| ------- | ------- | ---------- |
| `[audio.<id>]` | Audio codecs (AAC, ALAC, Atmos, etc.) | `eac3-atmos`, `alac`, `aac-hq` |
| `[meta.<id>]` | Abstract quality tiers that resolve per-service | `lossless`, `atmos`, `best-lossy` |
| `[video.<id>]` | Video codecs (H.265, H.264, VP9, etc.) | `h265`, `h264`, `vp9` |
| `[lyrics.<id>]` | Lyrics/subtitle/caption formats | `ttml`, `lrc`, `srt`, `vtt` |

### Adding a New Audio Codec

```toml
[audio.my-new-codec]
name = "My New Codec"           # Display name shown in the UI

category = "lossy"              # "spatial", "lossless", or "lossy"

lossless = false                # true if codec preserves original quality

mimetype = "audio/mp4"          # MIME type for the audio format

[audio.my-new-codec.services]
gamdl = "my-flag"               # Exact CLI flag string for GAMDL

votify = "other-flag"           # Exact CLI flag string for Votify

```

### Adding a New Video Codec

```toml
[video.my-video-codec]
name = "My Video Codec"
category = "modern"             # "modern" or "compatible"

mimetype = "video/mp4"
[video.my-video-codec.services]
gamdl = "my-flag"
```

### Adding a New Lyrics Format

```toml
[lyrics.my-format]
name = "My Format"
category = "text"               # "xml" or "text"

extension = "myf"               # File extension without dot

mimetype = "text/x-myformat"
word_timing = false             # true if supports word-level timestamps

[lyrics.my-format.services]
gamdl = "myf"
```

### Adding a Meta Codec

Meta codecs are abstract quality tiers that resolve to concrete codecs per service:

```toml
[meta.best-quality]
name = "Best Quality"
category = "lossless"
resolves_to = { gamdl = "alac", votify = "flac" }
```

### How to Find Service Mapping Values

Each service mapping value is the **exact string** the download engine's CLI expects as a flag argument. To find the correct value:

| Service Engine | CLI Tool | How to Find Codec Values |
| -------------- | -------- | ------------------------ |
| `gamdl` | GAMDL (Apple Music) | Run `python -m gamdl --help` or check `gamdl/cli_config.py` in the GAMDL source. Values listed under `--song-codec-priority`. |
| `votify` | Votify (Spotify) | Run `python -m votify --help` or check Votify's documentation. Values like `flac`, `aac-high`, `aac-medium`, `ogg-vorbis`. |
| `ytdlp` | yt-dlp (YouTube) | Run `yt-dlp --help` (format selection section) or check the yt-dlp docs. Values like `opus`, `aac`, `vorbis` (audio) and `vp9`, `av1` (video). |

The service engine IDs (`gamdl`, `votify`, `ytdlp`) are defined by MeedyaDL — they're the keys used in the download routing code.

### If No Service Supports a Codec Yet

Omit the `[*.services]` table entirely. The codec exists in the registry for future use, and `resolve_audio()` returns `None` for all services:

```toml
[audio.ac4-atmos]
name = "Dolby Atmos (AC-4)"
category = "spatial"
lossless = false
mimetype = "audio/ac4"

# No [audio.ac4-atmos.services] table — not downloadable yet

```

### Practical Example: Adding MP3 Support

If a future service engine (say `example-dl`) supported MP3 downloads with a CLI flag `--audio-format mp3-320`:

```toml
[audio.mp3-320]
name = "MP3 320kbps"
category = "lossy"
lossless = false
mimetype = "audio/mpeg"
[audio.mp3-320.services]

# gamdl doesn't support MP3, so no gamdl mapping

example-dl = "mp3-320"         # Exact CLI flag for example-dl

```

### No Code Changes Required

Adding new entries to `codecs.toml` requires **zero Rust or TypeScript code changes**. The registry uses `HashMap<String, ...>` for dynamic parsing, so new entries are automatically available via the query functions (`resolve_audio()`, `codecs_for_service()`, etc.).

The only exceptions that need code changes:

1. **New top-level section type** (e.g., `[container.mp4]`) — needs a new struct in `codec_registry.rs`
2. **New SongCodec bridge mapping** — if the codec maps to the existing `SongCodec` Rust enum, update `song_codec_to_registry_id()` / `registry_id_to_song_codec()` in `codec_registry.rs`

### Related Files

| File | Role |
| ---- | ---- |
| `src-tauri/codecs.toml` | Codec definitions (edit this file to add/modify codecs) |
| `src-tauri/src/models/codec_registry.rs` | Rust registry module (parses TOML, provides query functions) |
| `src/types/codec-registry.ts` | TypeScript type mirrors for frontend use |

---

## Metadata Tag Registry (`tags.toml`)

**File location:** `src-tauri/tags.toml`

The tag registry defines which Apple Music API JSON fields are extracted and embedded as MP4 freeform atoms in downloaded files. It is compiled into the binary at build time via `include_str!` — no runtime file I/O. **Adding new tags requires only editing `tags.toml` — zero Rust code changes.**

### File Structure

The TOML file has two top-level scopes:

| Section | Description | Count |
|---------|-------------|-------|
| `[album.<tag_id>]` | Per-album tags (same value on every track in the album) | 16 |
| `[track.<tag_id>]` | Per-track tags (matched to each file by track/disc number) | 14 |

### Tag Entry Format

Each tag entry defines three things:

```toml
[album.record_label]
json_path = "attributes.recordLabel"     # Dot-separated path into raw API JSON

value_type = "string"                     # How to convert the value

atoms = [                                 # Which freeform atoms to write
    { namespace = "itunes", name = "LABEL" },
    { namespace = "meedya", name = "AppleRecordLabel" },
]
```

### JSON Path Syntax

| Pattern | Example | Meaning |
|---------|---------|---------|
| Simple | `attributes.name` | `json["attributes"]["name"]` |
| Nested | `attributes.editorialNotes.short` | Deep object traversal |
| Array index | `attributes.previews[0].url` | First element of array |
| Relationship | `relationships.artists.data[0].id` | Across relationships |
| Top-level | `id` | Direct field on the JSON root |

Album tags use paths relative to `data[0]` (the album object).
Track tags use paths relative to each track in `data[0].relationships.tracks.data[*]`.

### Value Types

| Type | JSON Input | String Output | Example |
|------|-----------|---------------|---------|
| `string` | `"Republic Records"` | `"Republic Records"` | Record label |
| `bool` | `true` | `"true"` | Digital Master flag |
| `u32` | `13` | `"13"` | Track count |
| `u64` | `202395` | `"202395"` | Duration in ms |
| `array` | `["Pop", "Music"]` | `"Pop, Music"` | Genre names |
| `first_of_array` | `["Pop", "Music"]` | `"Pop"` | Primary genre |

### Namespace Shortcuts

| TOML Value | Full Namespace | Use Case |
|------------|---------------|----------|
| `"itunes"` | `com.apple.iTunes` | Player-compatible, industry standard |
| `"meedya"` | `MeedyaMeta` | MeedyaDL-branded, Apple-sourced |

### Naming Conventions

- **Album scope:** `Album*` prefix in iTunes namespace (e.g., `AlbumReleaseDate`, `AlbumMasteredForItunes`)
- **Track scope:** No prefix — track is the assumed default (e.g., `ReleaseDate`, `Composer`)
- **Industry standard:** Established names where recognised (e.g., `LABEL`, `COPYRIGHT`, `COMPILATION`, `TOTALTRACKS`)
- **MeedyaMeta:** `Apple*` prefix (e.g., `AppleRecordLabel`, `AppleReleaseDate`)

### Adding a New Tag

1. Edit `src-tauri/tags.toml`
2. Add a new `[album.<id>]` or `[track.<id>]` section
3. Set `json_path` to the API JSON path
4. Set `value_type` to the appropriate conversion
5. Add `atoms` with namespace and name for each atom to write
6. Rebuild the app — no Rust code changes needed

Example — adding a hypothetical `audioLocale` field:

```toml
[track.audio_locale]
json_path = "attributes.audioLocale"
value_type = "string"
atoms = [
    { namespace = "itunes", name = "AudioLocale" },
    { namespace = "meedya", name = "AppleAudioLocale" },
]
```

### Key Files

| File | Purpose |
|------|---------|
| `src-tauri/tags.toml` | Tag definitions (edit this to add new tags) |
| `src-tauri/src/models/tag_registry.rs` | TOML parsing, JSON path extraction, value conversion |
| `src-tauri/src/services/metadata_tag_service.rs` | `write_tags_from_registry()` — the generic tag-writing loop |
| `src-tauri/src/services/apple_music_api.rs` | `raw_json` fields on `AlbumMetadata`/`TrackMetadata` |

### API Field Audit Tool

A developer diagnostic tool is available in **Settings > Metadata > API Field Audit**. It fetches a real album from the Apple Music API and compares all JSON attribute paths against the known tag definitions in `tags.toml`. Reports:

- **Known** (green): Fields mapped in tags.toml
- **Unknown** (amber): Fields in the API but not in tags.toml — candidates for new tag entries
- **Missing** (grey): Fields defined in tags.toml but absent from this particular album

Requires MusicKit credentials (Team ID, Key ID, private key in keychain).

---

## Subtitle and Lyrics Generation

MeedyaDL's enrichment pipeline includes 6 subtitle/lyrics processing steps (Steps 2-2f):

### Processing Pipeline

| Step | Feature | Setting | Default | Service |
|------|---------|---------|---------|---------|
| 2 | Enhanced LRC (word-by-word sync) | `enhanced_lrc` | ON | `enhanced_lyrics_service.rs` |
| 2b | Lyrics format fallback | `lyrics_fallback_enabled` | ON | `download_queue.rs` |
| 2c | WebVTT generation | `generate_webvtt` | OFF | `webvtt_service.rs` |
| 2d | Rich SRT generation | `generate_rich_srt` | ON | `rich_srt_service.rs` |
| 2e | Subtitle embedding | `embed_subtitles` | OFF | `rich_srt_service.rs` |
| 2f | ASS generation | `generate_ass` | OFF | `ass_subtitle_service.rs` |

### Subtitle Formats

| Format | Extension | Styling | Source Priority |
|--------|-----------|---------|-----------------|
| Enhanced LRC | `.lrc` | Inline `<mm:ss.xx>` word timestamps | TTML only |
| WebVTT | `.vtt` | Plain text (no styling) | TTML → SRT → LRC |
| Rich SRT | `.srt` | `<b>`, `<i>`, `<u>`, `<font color>` | TTML → WebVTT |
| ASS | `.ass` | BGR colours, override tags, positioning, BgVocals style | TTML → WebVTT |

### Subtitle Embedding Atoms

When `embed_subtitles` is enabled, SRT and WebVTT content is embedded as freeform atoms:

- `com.apple.iTunes:subtitles-srt` — Rich SRT content
- `com.apple.iTunes:subtitles-vtt` — WebVTT content

Enhanced LRC is always embedded via the native `©lyr` atom when `enhanced_lrc` is enabled.

### Shared TTML Style Resolution

`rich_srt_service.rs` exports `pub(crate)` style resolution functions shared with `ass_subtitle_service.rs`:

- `resolve_named_styles(doc)` — Parse `<head><styling><style>` definitions
- `resolve_element_style(node, named_styles)` — Merge named + inline styles
- `TtmlStyle { bold, italic, underline, color }` — Shared style struct

### Sidecar Overwrite Behaviour (#550)

All sidecar writers (`.lrc`, `.srt`, `.vtt`, `.ass`) unconditionally overwrite their target path on every enrichment run. This is **intentional, not a bug**:

- `enhanced_lyrics_service.rs` writes `.lrc` via `std::fs::write()` (no existence check).
- `rich_srt_service.rs` writes `.srt` via `std::fs::write()` — deliberately replaces any plain SRT that GAMDL wrote natively, because the rich variant carries styling tags (`<b>`, `<i>`, colour) that the plain SRT lacks.
- `webvtt_service.rs` writes `.vtt` via `std::fs::write()`.
- `ass_subtitle_service.rs` writes `.ass` via `std::fs::write()`.
- The syllable-lyrics upgrade path in `download_queue.rs` also overwrites GAMDL's TTML with the richer `/syllable-lyrics` API response.

The design assumption is that these services are pure generators: same source TTML + same renderer version produces byte-identical output, so overwriting is a no-op in the common case. **Manual edits to any of these sidecar files WILL be silently clobbered** the next time the item is enriched (re-download, quality upgrade, manifest re-import). If you need to preserve hand-tweaked lyrics, copy the edited sidecar out of the output directory before re-running.

Follow-up work (content-hash skip, opt-in preservation flag, `.bak` backups) tracked in the Option B/C/D discussion on #550 — deliberately out of scope for the current design contract.

---

## Pre-Release vs Full Release Workflow

All versions before v1.0 are published as **pre-releases** on GitHub. This means:

- Users with `check_pre_releases: false` (default) will NOT receive update notifications
- Users who enable "Include Pre-Release Versions" in Settings > General will receive updates
- The `release.yml` workflow sets `prerelease: true` on all builds

### Publishing a Pre-Release (Current Default)

No special action needed. The standard release pipeline produces pre-releases automatically:

```bash

# 1. Push conventional commits to main

git push origin main

# 2. Release Please creates a Release PR (automatic)

# 3. Merge the Release PR on GitHub

# 4. Release Please creates a tag (e.g., v0.7.0)

# 5. release.yml builds all platforms → draft pre-release on GitHub

# 6. Manually review and publish the draft release on GitHub

```

The published release will have the "Pre-release" badge on GitHub and will only be picked up by users who have opted into pre-release updates.

### Publishing a Full (Stable) Release

When ready for v1.0 or a stable milestone:

**Option A: Edit release on GitHub (one-time)**

1. Follow the normal release pipeline (push, merge Release PR, wait for builds)
2. Go to the draft release on GitHub
3. Uncheck the "Set as a pre-release" checkbox
4. Click "Publish release"

**Option B: Change the workflow (permanent)**

1. Edit `.github/workflows/release.yml`
2. Change `prerelease: true` to `prerelease: false` (line ~545)
3. Also remove `--prerelease` from the ARMv7 `gh release create` command (line ~581)
4. Commit and push

**Option C: Via CLI after publishing**

```bash

# Mark a specific release as stable (removes pre-release flag)

gh release edit v1.0.0 --prerelease=false

# Mark a release back to pre-release

gh release edit v1.0.0 --prerelease
```

### How the App Update Checker Works

The update checker in `update_checker.rs` uses two different GitHub API endpoints:

| Setting | API Endpoint | Behaviour |
| ------- | ------------ | --------- |
| `check_pre_releases: false` (default) | `releases/latest` | GitHub auto-filters to the newest non-pre-release |
| `check_pre_releases: true` | `releases?per_page=5` | Returns newest releases including pre-releases |

Since all current releases are pre-releases, users on the default setting will see "no updates available" until a full release is published.

---

## Brand Asset Structure

### Directory Layout

All brand assets live in `assets/brand/`:

```text
assets/brand/
├── icon.svg               # Source SVG icon (vinyl/reel design with CSS colour modes)
├── logo.svg               # Animated SVG logo (vinyl/reel crossfade animation)
├── wordtype.svg           # Animated SVG wordtype (gradient shimmer wordmark)
├── brandkit.html          # Self-contained brand kit page (previews all assets)
├── icon[-mode].png        # Rendered icon PNGs (1024x1024, 8 modes)
├── icon[-mode].ico        # Windows ICO files (16-256px, 8 modes)
├── icon[-mode].icns       # macOS ICNS files (16-1024px, 8 modes)
├── icon[-mode]-liquidglass.png   # Liquid Glass variant PNGs (10% inset, 8 modes)
├── icon[-mode]-liquidglass.icns  # Liquid Glass variant ICNS (8 modes)
├── favicon[-mode].ico     # Favicon ICOs (16-48px, 8 modes)
├── logo[-mode].png        # Animated logo as APNG (8 modes)
└── wordtype[-mode].png    # Animated wordtype as APNG (8 modes)
```

**Colour modes** (8 total): default (light), dark, cb-deutan, cb-protan, cb-tritan, cb-deutan-dark, cb-protan-dark, cb-tritan-dark.

**Naming convention**: `-mode` suffix is omitted for the default (light) variant (e.g., `icon.png` = light, `icon-dark.png` = dark).

### Icon Generation (`scripts/generate-icons.mjs`)

Renders `icon.svg` in all 8 colour modes using Puppeteer (headless browser), then generates platform-specific formats:

- `.png` (1024x1024) -- copied from Puppeteer screenshot
- `.ico` (16-256px multi-size) -- assembled via Python Pillow
- `.icns` (16-1024px) -- assembled via macOS `iconutil`
- Liquid Glass variants (10% padding inset) -- assembled via Pillow

**Requirements**: `puppeteer` (npm), `python3` with `Pillow`, `iconutil` (macOS only for `.icns`).

**Usage**: `node scripts/generate-icons.mjs`

**Output**: 8 modes x 6 formats = 48 files in `assets/brand/`.

### APNG Generation (`scripts/svg-to-apng.mjs`)

Renders animated SVGs (logo.svg and wordtype.svg) frame-by-frame in Puppeteer, then assembles frames into Animated PNG files using ffmpeg:

- Captures frames at 15 fps for 8 seconds (120 frames per variant)
- Auto-trims content bounds with 4px padding
- Generates all 8 colour mode variants for both logo and wordtype

**Requirements**: `puppeteer` (npm), `ffmpeg` (for APNG assembly).

**Usage**: `node scripts/svg-to-apng.mjs`

**Output**: 2 SVGs x 8 modes = 16 APNG files in `assets/brand/`.

### Copyright Year Update (`scripts/update-copyright-year.sh`)

Updates the copyright end-year across all source files to the current calendar year. Covers: `.rs`, `.ts`, `.tsx`, `.css`, `.yml`, `.md`, `.svg`, `.html`, `LICENSE`, `tauri.conf.json`, and config files.

- Auto-detects macOS vs Linux for correct `sed -i` syntax
- Start year is 2026; uses `"Copyright (c) 2026"` for 2026, `"Copyright (c) 2026-YYYY"` for future years
- Excludes itself from bulk find/sed to prevent self-corruption (its own header is updated via a targeted line-number sed)

**Usage**: `./scripts/update-copyright-year.sh`

Run at the start of each new calendar year or automate in CI.

### Brand Asset License

Brand assets (logo, wordtype, icon) in `assets/brand/` are **proprietary** to MeedyaDL. They are NOT covered by the MIT license that applies to the source code. Do not redistribute, modify, or use the brand assets outside of MeedyaDL without written permission.

---

## MeedyaSuite Wordtype SVG — Customisation Guide

The wordtype SVG at `assets/brand/new/wordtype.svg` is a fully self-contained, animated wordmark designed for use across the MeedyaSuite product family (MeedyaDL, MeedyaManager, MeedyaDB).

### Changing Colours / Gradients

All colours are controlled via CSS custom properties in the SVG's `<style>` block. There are **four colour modes** built in, each with its own set of properties:

| Mode | Trigger | Properties |
|------|---------|-----------|
| **Light** (default) | Default / `?mode=light` | `--wordtype-primary: #475569` (dark slate), `--wordtype-secondary: #94A3B8` (silver), `--wordtype-accent: #64748B` (steel), `--wordtype-glow: #94A3B8` |
| **Dark** | `@media (prefers-color-scheme: dark)` / `.dark` class / `?mode=dark` | `--wordtype-primary: #CBD5E1`, `--wordtype-secondary: #F1F5F9`, `--wordtype-accent: #94A3B8`, `--wordtype-glow: #CBD5E1` |
| **Colour-blind (light)** | `.cb-deutan` / `.cb-protan` / `.cb-tritan` class or `?mode=cb-deutan` etc. | Uses IBM's colour-blind-safe palette (blue/amber/pink variants) |
| **Colour-blind (dark)** | `.dark.cb-deutan` or `?mode=cb-deutan-dark` etc. | Brighter versions of the CB palettes for dark backgrounds |

**To change colours for any mode**, edit the CSS custom properties in the corresponding block inside `<style>`:

```css
:root {
  --wordtype-primary: #475569;     /* Gradient start — darkest colour */
  --wordtype-secondary: #94A3B8;   /* Gradient end — lightest colour */
  --wordtype-accent: #64748B;      /* Suffix, dots, brackets, underline */
  --wordtype-glow: #94A3B8;        /* Neon glow filter colour */
  --wordtype-shadow: rgba(0,0,0,0.3); /* Drop shadow colour */
}
```

**At runtime** (e.g., from the app), override via:
- URL parameter: `wordtype.svg?mode=dark`
- CSS class: `<svg class="dark">` or `<div class="dark"><img src="wordtype.svg"></div>`
- JavaScript: `svgElement.style.setProperty('--wordtype-primary', '#ff0000')`

### Changing Animation Speed

Two CSS custom properties control all animation timing:

| Property | Default | Controls |
|----------|---------|----------|
| `--wordtype-animation-speed` | `4s` | Gradient shimmer on brand prefix, suffix, and circuit dots |
| `--wordtype-bracket-flash-speed` | `3s` | Bracket flicker/flash frequency |

To make animations faster, reduce the values; slower, increase them:
```css
:root {
  --wordtype-animation-speed: 2s;        /* Faster shimmer */
  --wordtype-bracket-flash-speed: 1.5s;  /* Faster bracket flash */
}
```

The dot pulse animation runs at `0.6×` the base animation speed. The suffix shimmer is offset by half a cycle from the prefix so the gradient appears to flow across the full wordmark.

### Changing the Product Name

Edit the `<text id="product-suffix">` element:
```xml
<text id="product-suffix" ...>DL</text>       <!-- MeedyaDL -->
<text id="product-suffix" ...>Manager</text>   <!-- MeedyaManager -->
<text id="product-suffix" ...>DB</text>        <!-- MeedyaDB -->
```

The embedded JavaScript auto-repositions the dots, brackets, underline, and resizes the canvas (`viewBox`) on load. No manual width adjustment needed.

### Fonts

Two Google Fonts are embedded as base64 WOFF2 inside the SVG (no network requests needed):

| Font | Style | Used For |
|------|-------|----------|
| **Orbitron** (variable, 400–900) | Geometric, sharp-cornered, sci-fi | Brand prefix ("Meedya") at weight 900 |
| **Rajdhani** (Bold 700) | Angular, condensed, digital readout | Product suffix ("DL") at weight 900 + 1.5px stroke |

Font weight is controlled via CSS `font-weight` in the `#brand-prefix` and `#product-suffix` rules. Orbitron is a variable font so any weight from 400–900 works.

### SVG Element IDs

All elements have descriptive IDs for manual editing or JavaScript access:

| ID | Element |
|----|---------|
| `brand-prefix` | "Meedya" text |
| `product-suffix` | "DL" / "Manager" / "DB" text |
| `circuit-dots` | Three pulsating vertical dots (group) |
| `bracket-left` / `bracket-right` | Decorative square brackets |
| `accent-underline` | Dashed bottom line |
| `scan-line` | Horizontal scan sweep (decorative) |
| `wordmark-group` | Container for all visible elements |

### File Size

~49 KB with two embedded fonts. The Orbitron variable font replaced 2 static weights, and Rajdhani uses a single weight — reduced from ~308 KB (4 separate static fonts) to ~49 KB.

---

## Smart Re-Download Detection (#263)

When a user re-downloads an album they have previously downloaded, MeedyaDL can detect whether the content has changed since the last download using Apple Music's `lastModifiedDate` API field.

### How It Works

1. **Pre-download history check**: When the user submits a URL, `DownloadForm.tsx` calls the `check_redownload_status` IPC command. This queries `history_service` for a previous successful download of the same URL. If found, an info toast shows the album title and previous download date. The download proceeds regardless (non-blocking).

2. **API field extraction**: During enrichment, `apple_music_api.rs` extracts the `lastModifiedDate` ISO 8601 timestamp from the album's `attributes` in the Apple Music catalog API response. This is stored on the `AlbumMetadata` struct.

3. **Manifest storage**: When `write_manifest()` runs in `download_queue.rs`, the `lastModifiedDate` is persisted as `ManifestSource.last_modified_date` in the `.meedyadl` JSON manifest file. This allows future downloads to compare the stored date against a fresh API response.

4. **Tag embedding**: The `lastModifiedDate` is also embedded as MP4 freeform atoms via `tags.toml`: `com.apple.iTunes:AlbumLastModified` and `MeedyaMeta:AppleLastModifiedDate`.

### Setting

| Setting | Type | Default | Location |
|---------|------|---------|----------|
| `smart_redownload_detection` | `bool` | `true` | Settings > General > Preferences |

When disabled, the pre-download history check is skipped entirely.

### What Changes Are Detectable

- Metadata updates (title, artist, artwork, editorial notes, track listing)
- Track additions or removals (deluxe editions, bonus tracks)
- Remastering or re-releases (Apple updates `lastModifiedDate` when content changes)
- Audio quality upgrades (Atmos, Lossless, Apple Digital Master certification)

### What Changes Are NOT Detectable

- Server-side audio re-encoding without metadata change (same `lastModifiedDate`)
- Changes to availability or regional restrictions
- DRM or delivery mechanism changes

### Key Files

| File | Role |
|------|------|
| `src-tauri/src/commands/gamdl.rs` | `check_redownload_status` IPC command, `RedownloadInfo` struct |
| `src-tauri/src/services/apple_music_api.rs` | `lastModifiedDate` extraction into `AlbumMetadata` |
| `src-tauri/src/services/download_queue.rs` | `last_modified_date` passed to `write_manifest()` |
| `src-tauri/src/models/manifest.rs` | `ManifestSource.last_modified_date` field |
| `src-tauri/src/models/settings.rs` | `smart_redownload_detection` setting |
| `src-tauri/tags.toml` | `[album.last_modified_date]` tag definition |
| `src/components/download/DownloadForm.tsx` | Frontend integration (history check + info toast) |
| `src/lib/tauri-commands.ts` | `checkRedownloadStatus()` IPC wrapper |
| `src/components/settings/tabs/GeneralTab.tsx` | Settings toggle UI |

---

## MeedyaDL-v2 Branch Archive (PR #24, closed 2026-03-27)

The `meedyadl-v2` branch (24 commits, Feb 20–25 2026) was an early prototype for multi-service support. It diverged too far from `main` (~100+ commits behind) and was closed as unmergeable. The branch is **preserved** (not deleted) for reference.

### Feature Status Mapping

| v2 Feature | Status on `main` | Notes |
|-----------|------------------|-------|
| Multi-service URL parser (YouTube/Spotify/iPlayer) | Not on main | Tracked by #100–#104, #107. Key reference: commit `9bcf848` |
| Smart Download cross-platform quality | Not on main | Tracked by #110. Reference: commit `fb887d98` |
| Remote feature availability / service status | Shipped | Client + in-app notice UI landed via #1069/#1071 (originally tracked by #106); per-service enforcement at enqueue time also shipped — a paused service declines new downloads with an explanation while anything already downloading finishes normally |
| Stable rollback from pre-release | Not on main | New issue #267 created |
| macOS codesign `--timestamp` wrapper | Reimplemented | release.yml Step 8.9 |
| 7z GPAC extraction (CI fix) | Reimplemented | dependency_manager.rs |
| i18n infrastructure | Reimplemented | #111 |
| Update preferences (auto-check, pre-release toggles) | Reimplemented | update_check_interval_hours, checkPreReleases |
| Bundled deps extraction on first launch | Superseded | Mirror-based tool management (MeedyaDL-Tools) |
| Perl runtime + get_iplayer | Superseded | BBC iPlayer deferred to M10 (#102) |
| aria2c / fpcalc bundling | Superseded | fpcalc replaced by embedded rusty-chromaprint |
| Signed bundled deps for notarization | Not applicable | No bundled deps approach on main |

### Recommendations for Future Multi-Service Work

1. **Don't cherry-pick from `meedyadl-v2`** — the code targets a different architecture (bundled deps, different settings schema, different type definitions). Create fresh feature branches from current `main`.
2. **The v2 URL parser pattern is useful reference** — `9bcf848` has the multi-service detection logic (Apple Music, YouTube, BBC iPlayer, Spotify) with content type classification. Adapt the pattern, don't copy the code.
3. **The v2 `bundled-deps` approach is obsolete** — main uses mirror-based tool management via MeedyaDL-Tools repo. New service engines (yt-dlp, votify) should be installed via pip (like GAMDL) or downloaded from mirrors, not bundled in the installer.
4. **The `DownloadOptions` refactor in v2** (commit `96266e3`) has a useful `service_id` field pattern for routing downloads to the correct engine. Worth adapting when implementing #107.
5. **Start each service milestone on a fresh branch** — `feat/spotify` from `main` for M8, `feat/youtube` for M9, etc. Keep PRs focused and mergeable.

---

## Engine Registry (`engines.toml`) — #268, #270

Defines available download engines (external tools) and their per-platform priority ordering. Located at `src-tauri/engines.toml`, compiled into the binary via `include_str!` — same pattern as `codecs.toml` and `tags.toml`.

### File Structure

The file has two sections:

**`[engines.<id>]`** — one entry per external tool:

```toml
[engines.get_iplayer]
name = "get_iplayer"              # Display name in UI
install_method = "system"         # "pip" | "binary" | "system"
# pip_package = "..."             # PyPI name (omit for non-pip tools)
cli_command = "get_iplayer"       # How MeedyaDL invokes the tool
homepage = "https://github.com/get-iplayer/get_iplayer"
description = "BBC iPlayer specialist with PVR scheduling and rich metadata"
```

**`[platforms.<id>]`** — one entry per media service:

```toml
[platforms.bbc-iplayer]
name = "BBC iPlayer"
url_patterns = ["bbc.co.uk/iplayer", "bbc.co.uk/sounds"]
engines = ["get_iplayer", "ytdlp"]   # priority order: first = primary
content_types = ["tv", "radio", "podcasts"]
```

### How Priority Works

The `engines` array in each platform section is an **ordered priority list**:

1. First entry = **primary engine** (used by default)
2. Subsequent entries = **fallback engines** (tried in order if primary fails or isn't installed)

Example: BBC iPlayer uses `get_iplayer` as primary because it's purpose-built for BBC content. If get_iplayer isn't installed, MeedyaDL falls back to `yt-dlp`.

Users can override the default priority per-platform in Settings. User overrides are stored in `AppSettings.engine_priority_overrides` and take precedence over the TOML defaults.

### Bundled vs External Engines

Each engine has a `bundled` field that determines how it's distributed:

**Bundled engines (`bundled = true`):**
- Installed via `pip install` into the managed Python environment during setup
- Packaged with MeedyaDL during CI release builds (release.yml reads this field)
- No custom path setting in Settings > Tools — always uses the managed version
- Updated via the in-app update checker (PyPI version comparison)
- Must not be installed separately by the user

**External engines (`bundled = false`):**
- Installed by the user via system package manager (Homebrew, apt, etc.)
- User can set a custom binary path in Settings > Tools
- Auto-detected from system PATH and common install locations at runtime
- Not packaged in the MeedyaDL installer

### Current Registry

| Engine | Bundled | Install | Platforms | Custom path |
|--------|---------|---------|-----------|-------------|
| GAMDL | Yes | pip | Apple Music | No |
| votify | Yes | pip | Spotify | No |
| yt-dlp | Yes | pip | YouTube, YouTube Music, BBC iPlayer (fallback) | No |
| get_iplayer | Yes | binary (mirror) | BBC iPlayer (primary) | No |

### CI Packaging — Tiny vs Offline Installers

MeedyaDL supports two installer types, controlled by a `workflow_dispatch` input:

**Tiny installer (default):**
- App binary only (~30MB)
- Engines and tools downloaded on first launch via setup wizard
- Produced by every tag-push release and the default manual trigger

**Offline installer (manual trigger):**
- App + Python + pip engines + all binary tools (~300MB)
- Zero-setup: everything pre-bundled, no downloads on first launch
- Triggered manually: `gh workflow run "Release" -f tag=vX.X.X -f bundle_engines=true`
- Step 8.5 in `release.yml` reads `engines.toml` to determine what to install:
  - `bundled=true, enabled=true, install_method=pip` → `pip install` into bundled-deps
  - `bundled=true, enabled=true, install_method=binary` → download from MeedyaDL-Tools mirror
- Binary tools (FFmpeg, mp4decrypt, etc.) also downloaded from mirror
- Writes `manifest.json` with `offline_installer: true` so setup wizard skips downloading

When a new engine is enabled in `engines.toml`, both installer types pick it up automatically — no workflow YAML changes needed.

### Editing Guide

#### Adding a new engine

1. Add a `[engines.<id>]` section with all required fields
2. Add the engine ID to every platform's `engines` list where it applies
3. Position it in the `engines` array according to desired priority
4. Implement the engine adapter in Rust (`services/<engine>_service.rs`)

#### Adding a new platform

1. Add a `[platforms.<id>]` section
2. List `url_patterns` — hostnames the URL parser uses for auto-detection
3. List `engines` in priority order (at least one engine required)
4. List `content_types` for UI display hints
5. Add URL parsing support in the frontend (`src/lib/url-parser.ts`)

#### Changing engine priority for a platform

Edit the `engines` array order. Example — to make yt-dlp primary for BBC iPlayer:

```toml
# Before (get_iplayer primary):
engines = ["get_iplayer", "ytdlp"]

# After (yt-dlp primary):
engines = ["ytdlp", "get_iplayer"]
```

#### Removing an engine from a platform

Remove its ID from the `engines` array. The engine definition can stay in `[engines.*]` — unused engines are simply not offered for that platform.

### Implementation Status

- `engines.toml` file: **Done** (commit `f20fe9b`)
- Rust parser (`engine_registry.rs`): **Pending** (#270)
- Engine selection/fallback logic: **Pending** (#270)
- Frontend types + Settings UI: **Pending** (#270)
- Per-engine adapters: **Pending** (one per milestone: #101, #102, #103, #104)

---

## Platform Icons (`public/icons/platforms/`)

Each media service has a small icon displayed in the progress bar during downloads. Icons are stored as SVGs in `public/icons/platforms/` and referenced by the `icon` field in `engines.toml`.

### Theme Adaptability

Platform icons use `currentColor` instead of hardcoded fill colours. The `PlatformIcon` component in `GlobalProgressBar.tsx` fetches the SVG and renders it **inline** (not as `<img>`) so that `currentColor` inherits from the parent CSS context. This means icons automatically adapt to:

- **Light mode** — inherits dark text colour
- **Dark mode** — inherits light text colour
- **Colour-blind modes** — inherits the theme's adjusted text colour
- **High-contrast mode** — inherits the boosted contrast colour

SVG content is cached in a module-level `Map` to avoid re-fetching on re-renders.

### Fallback Chain

1. **Local SVG** from `public/icons/platforms/{id}.svg` — rendered inline, theme-adaptive
2. **Google Favicon API** — `https://www.google.com/s2/favicons?domain={host}&sz=32` returns a PNG. Not theme-adaptive but always available for any domain.

### Adding a New Platform Icon

1. Create a 16x16 SVG file at `public/icons/platforms/{platform-id}.svg`
2. Use `fill="currentColor"` for all paths (NOT hardcoded hex colours)
3. Use `fill-opacity` for visual weight variation (e.g., `0.7` for primary, `0.15` for backgrounds)
4. Set the `icon` field in `engines.toml`: `icon = "icons/platforms/{id}.svg"`
5. Add the platform to `PLATFORM_CONFIG` in `GlobalProgressBar.tsx`

### SVG Template

```xml
<svg width="16" height="16" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
  <!--
    {Service Name} icon.
    Uses currentColor for theme adaptability.
  -->
  <path d="..." fill="currentColor" fill-opacity="0.7"/>
</svg>
```

### Current Icons

| Platform | File | Status |
|----------|------|--------|
| Apple Music | `apple-music.svg` | Done (music note) |
| Spotify | `spotify.svg` | Done (circle + waves) |
| YouTube | `youtube.svg` | Pending (uses favicon fallback) |
| YouTube Music | `youtube-music.svg` | Pending (uses favicon fallback) |
| BBC iPlayer | `bbc-iplayer.svg` | Pending (uses favicon fallback) |

---

## Progress Tracking Architecture (#294)

The download progress system tracks ALL activity — primary downloads, companion downloads, enrichment, and post-processing — through a unified event and state model.

### Queue Item State Machine

```text
Queued → Downloading → Processing → Complete
                  ↓           ↓
                Error      Error/Cancelled
```

- **Downloading**: Primary GAMDL download is active. Speed/ETA/percentage shown.
- **Processing**: Post-download work (enrichment + companions). Item stays in this state until ALL background tasks finish (JoinHandle tracking).
- **Complete**: Only set by the completion task after enrichment AND companion JoinHandles resolve.

### Event Flow

1. **Primary download** emits `gamdl-output` events → `handleProgressEvent()` sets state to `downloading`, updates speed/ETA/progress.
2. **Processing step** event → sets state to `processing`, clears speed/ETA.
3. **Companion downloads** emit `gamdl-output` events → update speed/ETA/progress on the item but do NOT change state back to `downloading` (preserves `processing` state stability).
4. **Enrichment stages** set `processing_label` (e.g., "Enriching metadata tags...", "Converting lyrics...") but don't emit speed/ETA.
5. **Completion task** awaits all JoinHandles → sets state to `complete`, sends desktop notification.

### Key Design Decisions

- **State stability**: `download_progress` and `track_info` events check `item.state !== 'processing'` before transitioning to `downloading`. This prevents the queue-level progress bar from oscillating between "done" and "not done" during companion downloads.
- **Speed/ETA clearing**: `processing_step` events clear `speed` and `eta` to prevent stale companion data lingering during enrichment.
- **Partial credit**: Queue-level bar counts `processing`, `error`, and `cancelled` items alongside `complete` in the "done" tally, so the queue bar advances after the primary download succeeds.
- **Determinate vs indeterminate**: GlobalProgressBar shows determinate progress (percentage) during `processing` if `speed` data exists (companion downloading); otherwise shows indeterminate animation (enrichment).

### Processing Labels

The `processing_label` field on `QueueItemStatus` is set at enrichment stage boundaries:

| Stage | Label |
|-------|-------|
| Metadata tagging | "Enriching metadata tags..." |
| Enhanced LRC | "Converting lyrics..." |
| Animated artwork | "Downloading animated artwork..." |
| AcoustID | "Fingerprinting audio..." |
| ReplayGain | "Analysing loudness..." |
| Companions | "Companion: {codec} {track}/{total}" |

### Per-File Progress

- **ReplayGain**: Emits "analysing file N/M — filename.m4a" per file
- **AcoustID**: Emits "fingerprinting file N/M — filename.m4a" per file
- **Companion stdout**: Streamed line-by-line via `AsyncBufReadExt`; each line parsed by `parse_gamdl_output()` and emitted as `gamdl-output` event

### Files

- `src-tauri/src/services/download_queue.rs` — state management, JoinHandle tracking, companion stdout streaming
- `src-tauri/src/models/download.rs` — `processing_label: Option<String>` on `QueueItemStatus`
- `src/stores/downloadStore.ts` — event handling with state stability guards
- `src/components/layout/GlobalProgressBar.tsx` — dual bar rendering with companion speed/ETA support

## Activity Log Memory Optimization (#370)

During multi-item download sessions the `tauri://localhost` WebView process grew to **14+ GB RAM** and froze. Root cause: the activity log accumulated 7,500+ entries with no cap, each `addEntry()` call created a full array copy via spread (`[...entries, entry]`), and all entries were rendered as real DOM nodes without virtualization. Combined with ~20,000–40,000 events emitted per album download from the Rust backend, this caused exponential memory pressure and GC thrashing.

### Changes (PR #364)

1. **RAF-batched event listener** (`App.tsx`) — incoming `activity-log` events are buffered in a plain array and flushed in a single `addEntries()` call per `requestAnimationFrame`. Reduces Zustand state updates from 200+/s to ~60/s max.

2. **Capped activity store** (`activityStore.ts`) — `MAX_ENTRIES = 10_000`. When exceeded, the oldest entries are trimmed via `slice()`. `addEntries()` batch method appends in a single `set()` call. Auto-incrementing `_id` on each entry for stable React keys.

3. **Virtualized ActivityLog** (`ActivityLog.tsx`) — `useVirtualizer` from `@tanstack/react-virtual` renders only visible rows + 50-row overscan buffer. DOM nodes drop from ~37,500 to ~150 regardless of entry count. Auto-scroll uses `virtualizer.scrollToIndex()` for accurate positioning with dynamic row heights.

4. **Backend `\r` segment coalescing** (`download_queue.rs`) — for `\r`-split progress lines (yt-dlp style), only the **last non-empty segment** is emitted to `activity-log`. Intermediate segments are still parsed for `gamdl-output` progress tracking. Reduces activity-log event volume by 5-10x.

5. **Download store optimization** (`downloadStore.ts`) — `handleProgressEvent` uses `map()` pattern returning same reference for non-matching items, instead of `[...arr]` + `findIndex` + splice.

### Files

- `src/stores/activityStore.ts` — capped store with `addEntries()` batch method and `_id` assignment
- `src/App.tsx` — RAF-batched event listener for `activity-log`
- `src/components/download/ActivityLog.tsx` — virtualized rendering via `@tanstack/react-virtual`
- `src-tauri/src/services/download_queue.rs` — `\r` coalescing in stdout/stderr reader tasks
- `src/stores/downloadStore.ts` — `map()` pattern for progress events
- `src/types/index.ts` — `_id?: number` field on `ActivityLogEntry`

### New dependency

- `@tanstack/react-virtual` — headless virtualization (supports dynamic row heights, React 19 compatible)

## macOS In-App Updater Fix (#368)

The macOS in-app updater periodically failed to download new releases. The update banner appeared correctly (detecting the new version via `update_checker.rs` GitHub API query), but clicking "Download & Install" returned *"No update found for this platform"*.

### Root Cause

**Filename mismatch in `release.yml`**: Tauri 2.x names the macOS updater bundle `MeedyaDL.app.tar.gz` (after the `.app` bundle, no arch/version suffix). The upload step (line 791) looked for `MeedyaDL_aarch64.app.tar.gz` — a file that doesn't exist. The `.app.tar.gz` and `.app.tar.gz.sig` were never uploaded, so the `finalize-release` job's `latest.json` rebuild silently omitted `darwin-aarch64`.

The `download_and_install_app_update` Rust command fetches `latest.json` from the specific release tag (`/releases/download/{tag}/latest.json`). Without a `darwin-aarch64` entry, `updater.check()` returns `None`.

### Fix

Corrected all three filename references in `release.yml` (upload step + `latest.json` rebuild) and `fix-updater-manifest.yml` to use `MeedyaDL.app.tar.gz`.

### Files

- `.github/workflows/release.yml` — upload step (lines 791-806) and manifest rebuild (line 940-949)
- `.github/workflows/fix-updater-manifest.yml` — macOS sig filename (lines 85-86)

## cargo-deny Org-Level Source Allowlist (#365)

`deny.toml` uses `[sources.allow-org]` to allow git dependencies from both GitHub organisations:

```toml
[sources.allow-org]
github = ["MWBMPartners", "MeedyaDL"]
```

This covers `MWBMPartners/MeedyaSuite-core` (shared Rust crates), `MeedyaSuite/MeedyaDL-Tools` (dependency mirrors), and any future repos under either org — regardless of branch, tag, or rev qualifiers.

## Updater Signing Key Rotation (#401)

See `SECURITY.md` → "Updater Signing Key Rotation Plan" for the full procedure. Key points:

- The Tauri signing key (`TAURI_SIGNING_PRIVATE_KEY`) is stored only in GitHub Actions Secrets
- If compromised: revoke immediately, generate new key pair, publish a manual recovery release
- Users must manually download the recovery release (the old auto-updater can't verify the new key)
- The public key in `tauri.conf.json` → `plugins.updater.pubkey` must match the new private key

## IPC Rate Limiting (#395)

Sensitive IPC commands are rate-limited via a sliding-window limiter in `utils/rate_limiter.rs`:

| Command | Limit | Window |
|---------|-------|--------|
| `start_download` | 10 calls | 60 seconds |
| `check_all_updates` | 1 call | 60 seconds |
| `download_and_install_app_update` | 1 call | 60 seconds |
| `import_cookies_from_browser` | 3 calls | 60 seconds |

Returns "Too many requests. Please wait N seconds" when exceeded.

## Settings File Integrity (#396)

On save: SHA-256 digest written to companion `settings.json.sha256` file.
On load: digest verified. Mismatch logs a warning but settings are still loaded (user may have intentionally edited). Missing checksum file (pre-upgrade settings) is accepted and a checksum generated for next time.

## Engine Filename Safety Contract (#551)

A **design-review tool** — not a runtime guard. Every new download-engine integration (votify for Spotify, yt-dlp for YouTube, get_iplayer for BBC iPlayer, ...) is expected to implement `services::filename_safety::FilenameSafetyContract` in a companion `impl` block so that the review PR demonstrates compile-time + unit-test conformance to the invariants that #527 / #531 / #537 chased across the Apple Music pipeline.

Runtime enforcement of safe paths remains the job of `utils/fs_safe.rs` and the #487 umbrella. This contract prevents the bug from *landing* in the first place.

### Failure modes the contract guards against

1. **Punctuation-only filename** — every template placeholder resolves to the empty string, so the final filename is something like `"-.mp4"` (the original #527 MV bug, where `no_album_file_template = "{disc} - "` rendered to `" - "` because `{disc}` was empty).
2. **`[Unknown]`-sentinel folder** — unknown content gets routed to a literal folder called `[Unknown]` / `Unknown Album` / `(no album)`, silently colliding two distinct unknown items in one directory. Legacy pre-v2 GAMDL default.
3. **Stable-ID-less dedup collision** — two items with different stable IDs (Clean/Explicit cuts of the same track, remix variants, regional re-releases) produce the same filename, triggering `MediaFileExists` skips under `overwrite=false` with no user-visible warning.

### The trait

```rust
pub trait FilenameSafetyContract {
    fn engine_id(&self) -> &str;
    fn fallback_file_template(&self) -> &str;
    fn fallback_folder_template(&self) -> &str;
    fn supported_placeholders(&self) -> &[&str];
    fn stable_id_placeholder(&self) -> Option<&str>;
    fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String;

    // Default-provided conformance checks (no per-engine override needed):
    fn must_reference_stable_id(&self)       -> Result<(), FilenameSafetyViolation>;
    fn must_not_be_unknown_sentinel(&self)   -> Result<(), FilenameSafetyViolation>;
    fn must_survive_empty_metadata(&self)    -> Result<(), FilenameSafetyViolation>;
    fn must_disambiguate_by_stable_id(&self) -> Result<(), FilenameSafetyViolation>;
    fn run_all_checks(&self) -> Vec<FilenameSafetyViolation>;
}
```

### Reviewer checklist — every new engine PR must tick all five

- [ ] Fallback file template includes a stable-unique ID placeholder — `{title_id}` (Apple Music MVs), `{spotify_id}` (Spotify), `{id}` (YouTube), `{pid}` (BBC iPlayer).
- [ ] Fallback folder template contains no literal `[Unknown]` / `Unknown Album` / `(no album)` segments as the entire path. Route unknown content to a stable named folder instead (`{artist}/Singles/`, `{channel}/`, `{programme_name}/`, ...).
- [ ] Template-builder UI (`src/lib/template-parser.ts::TEMPLATE_VARIABLES` + `TemplateBuilder.tsx`) exposes the engine's placeholders.
- [ ] Engine's conformance `impl` is added to `registered_contracts()` in `services/filename_safety.rs::tests` so `all_registered_contracts_conform` covers it in CI.
- [ ] Engine-specific regression test reproduces the "empty metadata" failure mode in a synthetic fixture (mirror the `GamdlMusicVideoFallback` example).

### Canonical implementation example

`services::filename_safety::GamdlMusicVideoFallback` — pairs with `MV_NO_ALBUM_FILE_TEMPLATE` / `MV_NO_ALBUM_FOLDER_TEMPLATE` in `services/download_queue.rs`. New engines should structure their `impl` the same way: mirror the runtime template constants as string literals inside the contract module to keep the safety-check module free of cross-module coupling.

### Out of scope

- **Runtime enforcement** — the contract does not abort a download when the invariant is violated. A failing conformance test breaks the build; a malformed runtime template (via user override of the default) is caught by `utils/fs_safe.rs`.
- **Per-placeholder validation** — whether `{artist}` correctly resolves during rendering is the engine's own business; the contract only cares about the no-metadata failure mode.
- **Forbidden-segment exhaustiveness** — the sentinel-check list (`[Unknown]`, `Unknown`, `(no album)`, ...) is deliberately short; extend it if a new legacy default surfaces, but don't let it become a style rulebook.

## Lyric Sidecar Regeneration Policy (#550)

Lyric/subtitle generators run on every enrichment pass (first download, companion pass, retry, manifest re-import). The write policy is non-uniform today:

| Generator                  | File               | Source (`src-tauri/src/services/`)          | Existing file behaviour                                                                 |
| -------------------------- | ------------------ | ------------------------------------------- | --------------------------------------------------------------------------------------- |
| Enhanced LRC converter     | `.lrc`             | `enhanced_lyrics_service.rs:191`            | **Overwrites** — no `exists()` guard                                                    |
| Rich SRT generator         | `.srt`             | `rich_srt_service.rs:132`                   | **Overwrites** by design (including GAMDL's plain `.srt`)                               |
| Syllable-lyrics upgrade    | `.ttml`            | `download_queue.rs:~5594` and `~5618`       | **Overwrites** when Apple Music's `/syllable-lyrics` returns a word-level version       |
| WebVTT generator           | `.vtt`             | `webvtt_service.rs:85-87`                   | **Skips** (`if vtt_path.exists() { continue; }`)                                        |
| ASS generator              | `.ass`             | `ass_subtitle_service.rs:91-95`             | **Skips** (`if ass_path.exists() { continue; }`)                                        |

**Status: documented, not changed.** The audit in #550 considered four options (status quo with docs / content-hash skip / opt-in preservation / `.bak` backup) and settled on documented-status-quo. The overwriting generators are idempotent converters whose inputs (TTML from GAMDL, TTML from `/syllable-lyrics`) are themselves refreshed from upstream, so overwriting is the correct default for the 95% case — first-time generation and upstream content updates. The asymmetry between `.lrc`/`.srt` (overwrite) and `.vtt`/`.ass` (skip) is historical; it's called out in `help/lyrics-and-metadata.md` so users with hand-edited sidecars can work around it (rename to a non-generator extension, disable the generator, or copy the file before re-running enrichment).

**If that policy changes**, the canonical touch points are the `std::fs::write` calls above and the two syllable-lyrics upgrade sites in `download_queue.rs`. A future hash-skip guard would live inline at each site (the existing idempotency means a content hash compare would be cheap); a preserve-user-edits toggle would need a new setting keyed off file mtime vs. an internal "generated-by-MeedyaDL" sentinel.
<<<<<<< HEAD

## GAMDL 3.6 — Wrapper-v2, Native Muxing, Codec Rename (#853)

GAMDL **v3.6** (2026-05-20) is the largest upstream release MeedyaDL has absorbed since the 2.x → 3.0 transition. Implementation lives across the seven steps documented in PR linked to issue #853:

1. **`GamdlFeature` gates** (`services/gamdl_capabilities.rs`). Four new variants pinned to ≥ 3.6: `WrapperUrl`, `AacWebCodecRename`, `NativeMuxing`, and a re-thresholded `WrapperM3u8Ip` (now 3.1 – 3.5.x only). Plus `MusicVideoRemuxMode` which inverts at the 3.6 boundary (true ≤ 3.5.x, false ≥ 3.6).
2. **Codec emission** (`models/gamdl_options.rs::SongCodec::to_runtime_cli_string`). `AacLegacy` / `AacHeLegacy` Rust variants stay; their CLI string flips to `aac-web` / `aac-he-web` on ≥ 3.6. Updated 3 call sites in `services/download_queue.rs` (gap-fill priority chain, companion-task tier loop, primary fallback retry).
3. **Settings model** (`models/settings.rs`). New `wrapper_url: String` field with `default_wrapper_url() = "http://127.0.0.1"`. CURRENT_SETTINGS_VERSION 5 → 6. Migration `migrate_settings()` v5→v6 is additive — both wrapper-v1 (`wrapper_account_url` + `wrapper_m3u8_ip` + `wrapper_decrypt_ip`) and wrapper-v2 (`wrapper_url`) fields coexist; emission picks one per CLI invocation.
4. **CLI / INI dispatch** (`models/gamdl_options.rs::path_cli_args` + `flag_cli_args`, `services/config_service.rs::ini_advanced_section` + `ini_tool_path_section`). Same gate everywhere: `supports(WrapperUrl) → emit wrapper-v2 family`, else emit wrapper-v1 family (with sub-gate for the v3.1 `wrapper_m3u8_ip` addition). External tool path options (`--ffmpeg-path` / `--mp4box-path` / `--mp4decrypt-path`) gated behind `!supports(NativeMuxing)`. `--music-video-remux-mode` gated behind `supports(MusicVideoRemuxMode)`.
5. **Wrapper-v2 preflight** (`services/health_check_service.rs`). New `check_wrapper_v2_health()` does `GET /health` with a 3-second timeout. New `check_wrapper_v2_auth()` does `GET /me` and inspects `auth.state` — surfaces a yellow toast when reachable-but-logged-out. Both are called from `download_queue.rs::run_preflight_checks` under the `use_wrapper_v2` branch (mutually exclusive with the wrapper-v1 three-socket branch).
6. **Companion planner** (`services/download_queue.rs::lossy_chain_for_runtime`). Returns `[AacLegacy, Aac]` (aac-web first) on ≥ 3.6, `[Aac, AacLegacy]` on ≤ 3.5.x. Wired into every CompanionTier that builds a lossy AAC fallback chain — Atmos→Lossless+Lossy, Atmos→AllFormats, SpecialistToLossy, and the ALAC-primary case of Atmos→Lossless+Lossy.
7. **Frontend** (`commands/dependencies.rs::get_gamdl_capabilities` IPC + `src/components/settings/tabs/AdvancedTab.tsx`). The Settings UI's Wrapper section conditionally renders the v1 (three fields) or v2 (one field) layout based on the IPC response. Capabilities are loaded on mount.

### Fetch-path semantics (the key 3.6 insight)

The `aac-legacy` → `aac-web` rename isn't cosmetic. In upstream's `interface/song.py`:

```python
for codec in self.codec_priority:
    if codec.is_web:
        stream_info = await self._get_web_stream_info(webplayback, codec)
    else:
        stream_info = await self._get_stream_info(m3u8_master_url, codec)
```

- `aac-web` / `aac-he-web` → `apple_music_api.get_webplayback()` (MusicKit JWT only, **no wrapper required**)
- Every other codec → m3u8 master URL (FairPlay-encrypted, **needs wrapper-v2** on 3.6)

This is why the companion planner now prefers `aac-web` first on 3.6 — it's the only codec that reliably works in cookie-only mode on the new release. Pre-3.6 the same codec was called "legacy" but went through the same web path, so the behavioural difference is upstream's naming catching up to reality.

### Wrapper-v2 deployment reality

Wrapper-v2 is a [C++ daemon](https://github.com/glomatico/wrapper-v2) built with the Android NDK and run inside a Linux chroot. It depends on Apple Music for Android's `.so` libraries which the user must extract from the Android APK themselves — neither MeedyaDL nor the wrapper-v2 upstream redistributes Apple's binaries. On macOS / Windows the canonical path is **Docker Desktop** running the `compose.yaml`-provided container; on native Linux the daemon needs `SYS_ADMIN` / `SYS_CHROOT` / `SYS_PTRACE` privileges. **MeedyaDL does not bundle wrapper-v2** (Apple-`.so` licensing, privileged caps, NDK build complexity).

A second gotcha: GAMDL 3.6 added an InquirerPy interactive credential prompt in `cli/interactive_prompts.py`. If MeedyaDL's GAMDL subprocess hits an unauthenticated wrapper-v2 daemon, it would block waiting on stdin forever. The preflight `check_wrapper_v2_auth` exists specifically to surface the logged-out state via a yellow toast BEFORE the queue starts processing — so the deadlock window never opens.

### Backwards compatibility

MeedyaDL still supports every release in the 3.0 – 3.5.x window. The emission gates are runtime — they read from the in-memory version cache populated by the dependency probe — so a single MeedyaDL build serves both wrapper-v1 and wrapper-v2 users without behaviour drift. Settings file forward-compat: v6 adds `wrapper_url` non-destructively; downgrading the GAMDL install just re-shows the v1 UI without losing the v1 socket addresses.

### Adding the next GAMDL major (v3.7+ / v4.0)

Same drill as 2.9.1 → 3.x → 3.6:

1. Read the upstream diff and audit the four CLI / INI surfaces (`cli/cli_config.py`, `interface/enums.py`, `interface/constants.py`, `api/`).
2. For each ADDED / REMOVED / RENAMED CLI option or INI key, add a `GamdlFeature` variant. Pin its threshold to the release that introduced the change. Add a per-variant `is_available_on` arm and a per-variant unit test in `services::gamdl_capabilities::tests`.
3. For each renamed codec / value, follow the `to_runtime_cli_string()` pattern — keep the Rust variant for settings backwards-compat, runtime-dispatch the on-the-wire string.
4. Bump `tool-versions.toml` → `[gamdl] maximum_tested_version` + `recommended_version`. Add a per-release audit block to the file's comment header documenting the four CLI / INI / output / regex surface deltas (zero-code-change is the happy path; the 3.6 entry shows the full-blown audit shape).
5. Update `help/wrapper.md`, `help/quality-settings.md`, `README.md` "Component Support Matrix", `SECURITY.md` "Wrapper service" section, and `DEV_NOTES.md` (this section).
6. Ship as a pre-release on the `alpha` channel; hand-test before promoting to `beta` / Latest.
