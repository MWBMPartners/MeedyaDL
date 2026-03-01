# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### ✨ Features

- Add output path writability check before downloads

- Implemented `check_output_path_before_download` command to verify that the output directory is writable, catching issues like disconnected cloud mounts, full disks, and permission errors.
  - Integrated the new check into the download process in `DownloadForm.tsx`, ensuring downloads are only queued if the output path is accessible.
  - Updated settings model to include `update_check_interval_hours`, allowing users to specify how often to check for updates while the app is running.
  - Added UI components in the settings to configure the update check interval, visible only when auto-check for updates is enabled.
  - Enhanced logging and error handling in the download queue to provide more informative messages regarding fallback attempts and network errors.
  - Updated tests to cover new functionality and ensure existing features remain intact.

- Enhance internet connectivity check with multi-provider, multi-tier approach

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.7] - 2026-02-28

### 🐛 Bug Fixes

- Improve error handling for Python exceptions and traceback frames

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.6] - 2026-02-28

### ✨ Features

- Enhance toast notifications with deduplication and clearing for preflight checks
- Add auto-retry without wrapper option for failed downloads
- Add pre-download connectivity check, toast notification deduplication, auto-retry without wrapper, and network error report suppression

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.5] - 2026-02-28

### ✨ Features

- Add pre-download internet connectivity check to prevent queuing downloads without internet
- Implement pre-download checks for internet connectivity and cookie validation, update queue processing behavior

### 🐛 Bug Fixes

- **(docs)** Add wrapper connectivity troubleshooting guide for remote and Docker setups

Reclassified from docs: to fix: to trigger a patch release. The wrapper
  troubleshooting content (Help > Wrapper, README) addresses user-facing
  issues with diagnosing wrapper connectivity failures on remote devices.

- Improve audio format fallback so downloads try all available formats

When your preferred audio format (like Dolby Atmos) isn't available for
  a track, MeedyaDL now reliably tries the next format in your fallback
  list instead of giving up after the first failure.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Enhance README and HelpViewer with wrapper authentication details and connectivity troubleshooting
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Unify error reporting for crashes and download failures, update UI and documentation

## [0.5.4] - 2026-02-27

### ✨ Features

- Add cookie validation before download to enhance user feedback

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.3] - 2026-02-27

### ✨ Features

- **(crash-reports)** Implement crash reporting system with Sentry integration

- Added `CrashReport` model to represent crash/error reports.
  - Created `crash_report_service` for managing crash report files.
  - Implemented IPC commands for listing, retrieving, deleting, exporting, and logging frontend errors.
  - Integrated `tracing` for structured logging and added support for Sentry error tracking.
  - Updated application settings to include `sentry_enabled` for opt-in telemetry.
  - Enhanced frontend error handling to persist errors to the Rust crash report system.
  - Added UI toggle in settings for enabling/disabling anonymous crash reporting.
  - Implemented automatic cleanup of old crash reports older than 30 days.

- Implement GitHub Issues crash reporting system

- Added a new crash reporting feature that allows users to report crashes directly to GitHub Issues from the app.
  - Introduced `CrashReportSection` and `CrashReportDialog` components for managing crash reports and user consent.
  - Implemented `get_github_issue_url` command to generate pre-filled GitHub issue URLs with crash report data.
  - Updated documentation to reflect the new crash reporting functionality and usage instructions.
  - Enhanced localization for crash reporting features in English, German, and French.
  - Added IPC commands for listing, deleting, and exporting crash reports.


### 🐛 Bug Fixes

- Resolve startup crash caused by missing Tokio runtime in setup

The app was crashing on launch with "there is no reactor running, must
  be called from the context of a Tokio 1.x runtime" because the queue
  recovery code assumed a Tokio runtime was active during the setup
  closure. On macOS, this closure runs inside the `did_finish_launching`
  callback where the Tokio runtime isn't registered as "current".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update documentation for startup crash fix and external link handling

- CHANGELOG.md: Add entries for both bug fixes in [Unreleased] section
  - CLAUDE.md: Update queue persistence convention (blocking_lock, async
    runtime spawn) and Updates page convention (shell plugin for links)
  - Project_Plan.md: Note external link handling on Updates page entry
  - help/troubleshooting.md: Add "Crash on Launch (Queue Recovery Panic)"
    section with cause, fix version, and workaround for older versions

- Update CHANGELOG.md [skip ci]

## [0.5.2] - 2026-02-27

### 🐛 Bug Fixes

- Update handleViewRelease to use Tauri shell plugin for opening URLs

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.1] - 2026-02-27

### ✨ Features

- Add pre-flight health checks and retry-without-wrapper

Add pre-flight health checks that run before queue processing begins:
  - Internet connectivity check (pings apple.com with 5s timeout)
  - Cookie validation (checks for valid, non-expired Apple Music cookies)
  - Wrapper health check (pings wrapper URL when enabled)

  Warnings are emitted as persistent toasts — non-blocking, queue proceeds
  regardless. Checks run once per batch with a 60-second cooldown.

  Add "Retry without Wrapper" action for failed downloads that used wrapper
  authentication, allowing users to fall back to cookie-based auth:
  - Pill button below error message + right-click context menu option
  - New retry_download_without_wrapper Tauri command
  - used_wrapper field on QueueItemStatus for conditional UI display

  Also fixes LyricsTab test failures (enhanced_lrc default + /LRC/ regex)
  and bumps version to 0.5.0.


### 🐛 Bug Fixes

- Add missing used_wrapper field to Rust test initializers

cargo test compiles #[cfg(test)] modules that cargo check skips,
  causing CI to fail with missing field errors in 3 QueueItemStatus
  serde roundtrip tests.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.4.1] - 2026-02-27

### ✨ Features

- Add Enhanced LRC with word-by-word synchronized lyrics

Convert Apple Music TTML lyrics to Enhanced LRC format with inline
  word-level timestamps (<mm:ss.xx>) for karaoke-style highlighting.

  - New enhanced_lyrics_service.rs: TTML XML parser (roxmltree), word
    timestamp extraction, Enhanced LRC generation, M4A/M4V embedding
  - New enhanced_lrc setting (default: true) with TTML as default
    primary lyrics format and SRT as companion format
  - merge_options() Layer 4: forces TTML when Enhanced LRC is enabled
  - Enrichment pipeline Step 2: TTML → Enhanced LRC conversion
  - Frontend: Enhanced Lyrics toggle in Settings > Lyrics tab
  - Falls back to standard line-level LRC for songs without word data
  - Handles both iTunes namespace URIs and background vocals
  - 20 unit tests, all 339 tests passing, clippy clean
  - Version bump to v0.4.0


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Update LyricsTab tests to use regex for format labels

## [0.3.33] - 2026-02-26

### ✨ Features

- **(download)** Implement partial-success recovery for codec errors
- **(download)** Implement companion and lyrics downloads as background tasks
- **(activity-log)** Implement export functionality and wrapper connection test

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.32] - 2026-02-26

### ✨ Features

- Enhance queue persistence to include failed downloads for manual retry

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.31] - 2026-02-26

### ✨ Features

- Enhance persistence of download queue items to include failed states

### 🐛 Bug Fixes

- Update remux mode flag and clean up unused options in GamdlOptions

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.30] - 2026-02-26

### ✨ Features

- Add new app icon variants and previews

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.29] - 2026-02-26

### ✨ Features

- Update app icons and logos

- Updated the MeedyaDL logo in both light and dark variants (SVG and PNG formats) with a new design and color scheme.
  - Added a new application icon (app-icon.svg) that combines a clapperboard, download arrow, and music note.
  - Updated the Sidebar component to use the new app icon instead of a placeholder.
  - Updated various icon sizes for Android and iOS platforms to reflect the new branding.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.28] - 2026-02-26

### 🐛 Bug Fixes

- Add use import to doc test for is_version_at_least

The doc test example needed `use meedyadl::services::gamdl_service::is_version_at_least`
  to resolve the function in cargo test --doc (which runs examples as standalone crates).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.27] - 2026-02-26

### ✨ Features

- Integrate GAMDL v2.9.1 native codec priority, artist auto-select, and Apple Music Classical URLs

- Add version-aware codec fallback: GAMDL >= 2.9.1 uses native --song-codec-priority
    (all codecs tried in one process); older versions fall back to MeedyaDL's try_fallback system
  - Add ArtistAutoSelect enum (7 variants) with CLI arg and config.ini support
  - Add classical.apple.com URL support in frontend parser and backend regex patterns
  - Write dual config.ini keys (song_codec + song_codec_priority) for cross-version compatibility
  - Cache GAMDL version in DownloadQueue to avoid repeated pip show calls
  - Skip try_fallback() on both success and error paths when native priority was used
  - Clear song_codec_priority on companion and lyrics companion downloads (single-codec mode)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.26] - 2026-02-25

### ✨ Features

- Add platform asset validation for GitHub releases
- Refactor UpdateBanner integration in MainLayout and App components

### 🐛 Bug Fixes

- Improve asset manifest check in has_platform_assets function

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.25] - 2026-02-25

### ✨ Features

- Add sys-locale dependency for localized Apple Music storefront detection

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.24] - 2026-02-25

### 🐛 Bug Fixes

- Sign bundled macOS dependencies with Developer ID for notarization [skip ci]

Apple's notarization service inspects all Mach-O binaries inside the
  .app bundle, including those inside tar.gz archives. Third-party binaries
  (Python, Perl, FFmpeg, MP4Box libs, etc.) from bundled-deps must be
  re-signed with our Developer ID certificate before Tauri packages them.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update HelpViewer.tsx with links for cookie export and Apple Developer keys
- Update HelpViewer.tsx to clarify MusicKit key creation instructions
- Update CHANGELOG.md [skip ci]
- Update DEV_NOTES and CHANGELOG to document macOS codesign timestamp workaround and future MusicKit integration
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update dependencies and add new icons

- Updated `vitest` from `^2.1.8` to `^4.0.18` in `package.json`.
  - Added `sharp` dependency with version `^0.34.5`.
  - Updated various icon files in `src-tauri/icons` for different resolutions and platforms, including:
    - New icons for Android adaptive launcher and various mipmap resolutions.
    - New iOS app icons for multiple sizes.
    - Updated existing icon files for various resolutions.


## [0.3.23] - 2026-02-22

### ✨ Features

- Enhance link handling in HelpViewer for internal and external navigation
- Implement custom macOS menu and update About section to display app version

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Simplify TitleBar component to return null for all platforms

### 🧹 Maintenance

- Update milestone versions in project documentation and roadmap

## [0.3.22] - 2026-02-18

### ✨ Features

- Add multi-format lyrics support with companion downloads and update settings

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update quality settings and codec reliability information in documentation and UI
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.21] - 2026-02-18

### 🐛 Bug Fixes

- **(ToolsTab)** Install only missing required tools and update UI for optional tools

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.20] - 2026-02-17

### ✨ Features

- Enhance download error handling and output processing

- Introduced codec and I/O error recovery strategies in process_queue.
  - Added ANSI escape code stripping for cleaner Activity Log output.
  - Implemented new utility functions to classify codec and I/O errors.
  - Updated tests to cover new error classification logic.

- Reorganize settings tabs and enhance tool management functionality

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.19] - 2026-02-17

### 🐛 Bug Fixes

- Update default cover format to JPEG to prevent crashes in GAMDL 2.8.4; add file opening functionality in QueueItem component

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.18] - 2026-02-17

### ✨ Features

- Add non-fatal warnings to download items and update UI to display them

### 🐛 Bug Fixes

- Add text selection capability to ActivityLog component

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.17] - 2026-02-16

### ✨ Features

- **(i18n)** Add internationalization support with language detection and translations

- Added i18next and react-i18next for internationalization.
  - Implemented language detection and dynamic loading of translation files.
  - Created translation files for English, German, and French.
  - Updated AppSettings to include a UI language setting.
  - Enhanced settings UI to allow users to select their preferred language.
  - Introduced UpdatesPage component to display detailed update information with release notes.
  - Modified UpdateBanner to link to the UpdatesPage for more details.
  - Updated Sidebar navigation to include an Updates section.
  - Adjusted update checking logic to handle new update structures.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Update status color classes and add animated artwork help content

## [0.3.16] - 2026-02-16

### 🐛 Bug Fixes

- **(config_service)** Improve settings loading and sync to GAMDL config.ini

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.15] - 2026-02-16

### ✨ Features

- Add Activity Log component for live subprocess output and update download queue behavior

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.14] - 2026-02-16

### 🐛 Bug Fixes

- Settings interpretation, affecting downloading ability

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.13] - 2026-02-16

### ✨ Features

- Integrate embedded Chromaprint for AcousticID fingerprinting

- Replace external fpcalc dependency with the embedded rusty-chromaprint library for generating Chromaprint audio fingerprints.
  - Update documentation and comments to reflect the removal of external dependencies.
  - Modify settings and UI components to indicate the new fingerprinting method.
  - Implement fingerprint generation using Symphonia for audio decoding.
  - Enhance error handling for Python exceptions in the download queue process.
  - Add manual update check functionality in the settings UI.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.12] - 2026-02-16

### ✨ Features

- Implement manual queue processing and add auto-start settings
- Add temp directory setting and auto-start queue functionality

### 🐛 Bug Fixes

- Temo folder bug fix

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.11] - 2026-02-16

### ✨ Features

- Add ReplayGain analysis and AcousticID fingerprinting services

- Introduced `replaygain_service` for analyzing audio loudness using FFmpeg's EBU R128 filter, writing non-destructive ReplayGain metadata tags.
  - Added `acoustid_service` for generating Chromaprint audio fingerprints and looking up AcousticID identifiers.
  - Updated `metadata_tag_service` to include new metadata enrichment features.
  - Enhanced `apple_music_api` for improved metadata retrieval from MusicKit.
  - Added new settings tab for metadata enrichment options, including toggles for AcousticID and ReplayGain.
  - Updated Zustand store to manage new settings for AcousticID and ReplayGain.
  - Added unit tests for new features and ensured existing tests cover new functionality.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Enhance quality settings recommendations for audio codecs
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.10] - 2026-02-15

### 🐛 Bug Fixes

- Fixed build generation bugs

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.9] - 2026-02-15

### 🐛 Bug Fixes

- **(updater)** Update public key for the updater plugin

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.8] - 2026-02-15

### ✨ Features

- Add developer notes and update tauri configuration for updater plugin

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.7] - 2026-02-15

### ✨ Features

- Enhance audio codec handling and add help button for contextual assistance

- Added support for Dolby Digital (AC3) codec suffix in download queue.
  - Introduced new companion mode for Atmos to download all formats (AC3, ALAC, AAC).
  - Updated setup wizard to skip if dependencies are missing but setup has been completed.
  - Implemented HelpButton component for contextual help in Input, Select, and Toggle components.
  - Enhanced various settings tabs with help topics for better user guidance.
  - Improved validation and user feedback for cookie settings and sign-in processes.
  - Updated application branding from GAMDL to MeedyaDL in the sidebar and status bar.
  - Fetched application version dynamically from Tauri configuration.
  - Added setup_completed flag to settings store for persistent setup state.

- Add updater functionality for app updates with pre-release support

- Introduced updater permission set in macOS schema for frontend access.
  - Implemented `download_and_install_app_update` command to handle app updates.
  - Enhanced `check_all_updates` to include pre-release versions based on user settings.
  - Updated settings model to allow toggling of pre-release version checks.
  - Modified update checker to query GitHub Releases for both stable and pre-release versions.
  - Added UI components for downloading and installing updates, including progress tracking.
  - Integrated event listeners for real-time download progress updates in the frontend.
  - Updated settings UI to include a toggle for pre-release version checks.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.6] - 2026-02-15

### ✨ Features

- Add FallbackChainList component for reorderable priority lists

- Introduced a new generic component, FallbackChainList, for managing reorderable lists with up/down buttons.
  - Updated FallbackTab and QualityTab to utilize FallbackChainList for audio/video fallback chains and video codec priority respectively.
  - Enhanced type definitions for video codecs and added corresponding labels for UI representation.
  - Added support for displaying the source of installed tools in the DependenciesStep component.
  - Created tool-versions.toml to define minimum version requirements for external tools.
  - Added settings.json for permission configurations.


### 🐛 Bug Fixes

- Improve error logging in download_tool_with_fallback function

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.5] - 2026-02-15

### ✨ Features

- Release 0.3.5 with macOS signing validation and updated dependencies

- Added validation for required Apple signing secrets in the release workflow to prevent publishing unsigned binaries.
  - Updated version to 0.3.5 across various files including package.json, Cargo.toml, and tauri.conf.json.
  - Introduced Entitlements.plist for macOS hardened runtime permissions.
  - Enhanced Help documentation with a disclaimer regarding third-party dependencies.
  - Updated Tailwind CSS configuration to include typography plugin for improved styling.


### 🐛 Bug Fixes

- Add release-please version annotations for auto-managed docs [skip ci]

Added x-release-please-version markers to README.md (version badge,
  roadmap heading) and Project_Plan.md (version header). Registered both
  as generic extra-files in release-please-config.json so version numbers
  are updated automatically in Release Please PRs.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.4] - 2026-02-14

### 🐛 Bug Fixes

- Documentation

### 📚 Documentation

- Update CHANGELOG.md [skip ci]

## [0.3.3] - 2026-02-14

### 📚 Documentation

- Update changelog and docs with CI/workflow fixes [skip ci]

Document the release-please state fix, Linux ARM cross-compilation
  apt fix, release workflow manual dispatch with tag input, Windows
  PowerShell shell fix, and git remote URL update.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update version to 0.3.2 and enhance documentation
- Bump version to 0.3.3 and update changelog with bug fixes and changes

## [0.3.1] - 2026-02-14

### ✨ Features

- Add metadata tagging service for M4A files

- Implemented `metadata_tag_service.rs` to inject custom codec metadata tags into downloaded M4A files.
  - Added tagging for ALAC (`isLossless = Y`) and Dolby Atmos (`SpatialType = Dolby Atmos`) in both Apple iTunes and MeedyaMeta namespaces.
  - Updated `mod.rs` to include the new metadata tagging service.
  - Bumped version to 0.2.1 in `tauri.conf.json`.
  - Enhanced `DownloadForm.tsx` to support new codec and video resolution types.
  - Introduced "Embed Lyrics and Keep Sidecar" toggle in `LyricsTab.tsx` for better lyrics management.
  - Added companion download mode settings in `QualityTab.tsx` to control automatic multi-format downloads.
  - Updated settings store to include new settings for companion mode and lyrics embedding.
  - Expanded type definitions in `index.ts` to include `CompanionMode` and associated labels.
  - Updated tests in `settingsStore.test.ts` to reflect new default settings.

- Implement queue persistence and export/import functionality

- Added queue persistence to save the download queue to disk after every mutation, enabling crash recovery.
  - Introduced export/import features for the download queue using a `.meedyadl` file format, allowing users to transfer their queue between devices.
  - Updated relevant documentation and user interface to reflect new features.
  - Enhanced the download queue management with improved state handling and user notifications.

- Enhance workflows with manual dispatch and update changelog for queue features
- Update project documentation with planned service integrations and milestones for Spotify, YouTube, and BBC iPlayer
- Add multi-track muxing feature to project plan and README
- Implement hidden animated artwork files feature with OS-level hiding options

### 🐛 Bug Fixes

- Update release-please branch reference to match actual branch naming
- Restrict default apt sources to amd64 for ARM cross-compilation [skip ci]

Ubuntu 24.04's default sources (security.ubuntu.com, archive.ubuntu.com)
  don't host ARM packages. When dpkg --add-architecture adds arm64/armhf,
  apt-get update tries to fetch ARM indices from these mirrors and gets
  404 errors, causing the build to fail with exit code 100.

  Fix by adding Architectures: amd64 to the default deb822 sources file
  before adding the ARM ports repository. This ensures ARM packages are
  only fetched from ports.ubuntu.com.

- Support manual dispatch in release workflow with tag input [skip ci]

When triggered via workflow_dispatch, github.ref_name resolves to the
  branch name (e.g., "main") instead of a tag. This caused tauri-action
  to try creating a release with tag "main", which failed with
  "Resource not accessible by integration".

  Fix by adding a required 'tag' input for workflow_dispatch and resolving
  the effective tag name in a dedicated step. The checkout also uses the
  tag ref to ensure the correct code version is built.

- Use bash shell for tag resolution step on Windows runners [skip ci]

Windows runners default to PowerShell which can't parse bash syntax
  (if [ -n ... ]). Adding shell: bash ensures the step works on all
  platforms via Git Bash.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Add temporary PAT diagnostic workflow [skip ci]

Temporary workflow to verify RELEASE_PAT permissions.
  Run via: gh workflow run "Check PAT" --ref main
  Delete after verification.

- Add workflow_dispatch to release workflow [skip ci]

Allow manual trigger for re-running builds when tag push events
  are missed (e.g., after billing blocks or tag re-pushes).

- Remove PAT diagnostic workflow [skip ci]

RELEASE_PAT verified working — the original failure was caused by
  billing/spending limit, not token permissions.


## [0.1.4] - 2026-02-13

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
