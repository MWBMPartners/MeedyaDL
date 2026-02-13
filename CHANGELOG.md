# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

## [0.3.0] - 2026-02-13

### ✨ Features

- **Configurable companion downloads** — new `CompanionMode` setting in Settings > Quality with four modes: **Disabled** (no companions), **Atmos → Lossless** (default: Atmos downloads also get ALAC), **Atmos → Lossless + Lossy** (Atmos gets ALAC + AAC; ALAC gets AAC), and **Specialist → Lossy** (Atmos or ALAC get AAC). Specialist formats get filename suffixes (`[Lossless]` for ALAC, `[Dolby Atmos]` for Atmos); the most universal companion uses clean filenames. Replaces the previous boolean toggle with a granular dropdown.

- **Codec-based filename suffix system** — generic suffix system via `codec_suffix()` and `apply_codec_suffix()` in `download_queue.rs`. ALAC files get `[Lossless]`, Dolby Atmos files get `[Dolby Atmos]`, and lossy codecs use clean filenames. Applied during both primary downloads and fallback chain retries.

- **Custom metadata tagging** — after GAMDL writes standard Apple Music metadata, MeedyaDL injects codec-identification freeform atoms into downloaded M4A files via the `mp4ameta` crate. Now also applies to companion downloads (e.g., ALAC companions get `isLossless=Y`):
  - ALAC files: `----:com.apple.iTunes:isLossless` → `Y`
  - Dolby Atmos files: `----:com.apple.iTunes:SpatialType` → `Dolby Atmos` and `----:MeedyaMeta:SpatialType` → `Dolby Atmos`

- **Lyrics embed + sidecar** — new "Embed Lyrics and Keep Sidecar" toggle in Settings > Lyrics. When enabled (default), lyrics are both embedded in audio file metadata AND saved as separate sidecar files. Overrides the "Disable Synced Lyrics" toggle when active.

### 🐛 Bug Fixes

- Fix ESLint `no-unused-vars` warning: `defaultVideoResolution` in DownloadForm.tsx was only used as a type reference — replaced with direct `VideoResolution` type import.
- Fix ESLint `no-require-imports` errors: converted `require()` calls in `main.tsx` global error handlers to `import()` dynamic imports.

- **Queue persistence and crash recovery** — the download queue is now automatically saved to disk (`queue.json`) after every state change. If the app closes or crashes while downloads are queued or active, those items are restored and auto-resumed on next launch. Completed, failed, and cancelled items are cleared on restart (only pending items are persisted).

- **Queue export/import** — export the current download queue to a `.meedyadl` file (JSON-based, custom extension) and import it on another MeedyaDL instance. Useful for transferring download lists between devices. Exported items contain only URLs and per-download overrides; the importing device merges them with its own global settings.

### 🧹 Maintenance

- Added `mp4ameta` v0.13 dependency for M4A metadata tag writing.
- Added `metadata_tag_service.rs` service module for post-download custom tagging.
- Bumped version from 0.1.4 to 0.3.0.

### 📚 Documentation

- Updated `help/quality-settings.md` with configurable companion mode documentation.
- Updated `help/downloading-music.md` with companion download modes.
- Updated `help/lyrics-and-metadata.md` with embed + sidecar toggle details.
- Updated `README.md` feature list with configurable companions and lyrics embed + sidecar.
- Updated `PROJECT_STATUS.md` with new feature entries.
- Updated `.claude/CLAUDE.md` architecture notes with companion mode, codec suffix, and metadata tagging.

## [0.1.4] - 2026-02-12

### ✨ Features

- Add browser cookie extraction service and auto-import functionality

- Introduced `cookie_service` module for extracting Apple Music cookies from installed browsers.
  - Implemented auto-import feature in `CookiesTab` and `CookiesStep` components, allowing users to extract cookies with a single click.
  - Added platform-specific handling for macOS (Keychain access and Full Disk Access for Safari).
  - Enhanced user interface with loading indicators, error handling, and validation results for cookie imports.
  - Updated TypeScript types to support new cookie import functionalities, including `DetectedBrowser` and `CookieImportResult`.
  - Refactored existing components to accommodate the new auto-import feature and improve user experience.

- Add embedded Apple Music login window service and UI integration

- Introduced `login_window_service` to manage Apple Music authentication via an embedded webview.
  - Updated `CookiesTab` and `CookiesStep` components to support direct login, including event handling for cookie extraction.
  - Enhanced user experience with loading states and manual extraction options.
  - Added Tauri commands for opening, extracting cookies from, and closing the login window.

- Add support for fetching extra metadata tags and update cover size to max resolution
- Add animated artwork download service for Apple Music

- Implemented `animated_artwork_service` to download animated cover art (motion artwork) from Apple Music's catalog API.
  - Added functionality to parse Apple Music URLs, generate MusicKit Developer Tokens, and download HLS streams using FFmpeg.
  - Integrated animated artwork download into the download queue process, allowing for background downloading after album downloads.
  - Updated settings UI to include options for enabling animated artwork downloads and entering MusicKit credentials (Team ID, Key ID, and private key).
  - Enhanced settings store to manage new animated artwork settings and added corresponding TypeScript types.
  - Added unit tests for URL parsing and JWT generation related to animated artwork functionality.


### 🐛 Bug Fixes

- Enhance error handling and improve cookie import feedback

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update version to 0.1.3 in Cargo.lock and enhance project documentation

## [0.1.3] - 2026-02-12

### 🐛 Bug Fixes

- Resolve blank screen on macOS/Windows release builds

Fix React infinite re-render loop (error #185) that caused the UI to
  flash briefly then go blank in production builds. Three root causes:

  1. UpdateBanner: Zustand selector called getActiveUpdates() which uses
     .filter(), creating a new array reference on every store change.
     Zustand's Object.is() equality check always saw a new reference,
     triggering cascading re-renders. Fixed by subscribing to raw data
     (lastResult, dismissed) and deriving via useMemo.

  2. Sidebar: Subscribed to isReady function reference (always stable)
     instead of actual dependency state. The status dot never updated.
     Fixed by subscribing to python/gamdl status objects directly.

  3. App.tsx: Subscribed to entire settings object, causing full subtree
     re-renders on any settings change. Narrowed to sidebar_collapsed.
     Also replaced reactive isReady subscription with imperative
     getState() check in initialization effect.

  Additional changes:
  - Add CSP connect-src for Tauri IPC (ipc: http://ipc.localhost)
  - Add ErrorBoundary to main.tsx for visible crash diagnostics
  - Add Vite build config (target, envPrefix) per Tauri 2.0 guide
  - Enable devtools Cargo feature for WebView inspection
  - Open DevTools automatically in debug builds
  - Simplify Windows release: drop x86, produce only NSIS .exe (no .msi)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.1.2] - 2026-02-12

### ✨ Features

- Integrate release-please for automated release PRs

Add Google's release-please to automatically create Release PRs when
  conventional commits land on main. When merged, the PR creates a tag
  that triggers the existing 7-platform release build. git-cliff continues
  to own CHANGELOG.md (release-please has skip-changelog: true). The
  manual version-bump workflow is preserved as an override for non-standard
  releases.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add macOS Gatekeeper fix to release notes

Unsigned apps trigger macOS Gatekeeper's "damaged" warning. Add
  instructions to run xattr -cr to remove the quarantine flag.

- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Add auth/ to .gitignore to prevent secret leaks

## [0.1.1] - 2026-02-11

### ✨ Features

- Add release automation and expand to 7 platform targets

Add one-command release automation via Version Bump workflow
  (workflow_dispatch) that bumps versions across all source files,
  commits, tags, and triggers the release build. Expand the release
  build matrix from 3 to 7 platform targets: macOS ARM64, Windows
  x64/x86/ARM64, Linux x64/ARM64/ARMv7 (Raspberry Pi).


### 🐛 Bug Fixes

- Make usePlatform fallback test deterministic across CI runners

Mock navigator.userAgent with a known Windows UA string instead of
  relying on the host platform's default jsdom userAgent. This fixes
  the test failure on Ubuntu runners where the userAgent contains
  "linux" instead of "darwin".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.1.0] - 2026-02-11

### ✨ Features

- Initialize GAMDL GUI application with Tauri and React

- Add Tauri configuration file for application settings and build options.
  - Create main application component with platform detection and theme loading.
  - Implement custom hook for platform detection using Tauri's OS plugin.
  - Set up entry point for React application and global styles with Tailwind CSS.
  - Define base and platform-specific themes for macOS, Windows, and Linux.
  - Configure Tailwind CSS for platform-adaptive design tokens and styles.
  - Remove legacy test files and Python dependencies.
  - Add TypeScript configuration for Vite and Node environments.
  - Set up Vite configuration for React and Tauri integration.

- Add setup wizard components and state management

- Implement WelcomeStep component for the setup wizard, providing an introduction and overview of the setup process.
  - Create tauri-commands.ts for type-safe IPC calls to the Rust backend, covering system commands, dependency management, settings, downloads, and credential storage.
  - Introduce url-parser.ts to parse Apple Music URLs and detect content types.
  - Establish dependencyStore.ts to manage the installation status of Python, GAMDL, and external tools.
  - Create downloadStore.ts to handle download queue management, URL validation, and progress tracking.
  - Implement settingsStore.ts for managing application settings with load/save operations.
  - Add setupStore.ts to manage the setup wizard flow and completion status.
  - Introduce uiStore.ts for transient UI state management, including page navigation and toast notifications.
  - Update globals.css to include keyframe animations for UI components.
  - Define TypeScript types in index.ts to ensure type safety across the application, mirroring Rust backend models.

- Enhance CookiesTab with detailed browser export instructions and validation feedback

- Added step-by-step instructions for exporting cookies from various browsers (Chrome, Firefox, Edge, Safari).
  - Implemented a status badge to indicate the current cookie state (valid, invalid, expired).
  - Introduced a warning banner for cookie expiry with estimated days remaining.
  - Enhanced validation results display to include detected domains and additional warnings.
  - Improved user experience with a "Copy Cookie Path" button and loading states for validation.
  - Updated tauri-commands to support new download management features (retry, clear queue).
  - Created a new updateStore to manage application update checks and notifications.
  - Expanded types to include music service capabilities and update status for components.

- Implement icon generation script, ESLint configuration, and Vitest setup for testing
- Automate copyright year updates across all source files and enhance script functionality
- Implement theme management with useTheme hook and update styles for dark/light modes

### 🐛 Bug Fixes

- Resolve ESLint no-explicit-any error in Modal.test.tsx

Replace `any` type with `Record<string, unknown>` for the lucide-react
  X icon mock props to satisfy @typescript-eslint/no-explicit-any rule.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

---
*Generated with [git-cliff](https://git-cliff.org/)*
