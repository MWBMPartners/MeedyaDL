# Changelog

All notable changes to **GAMDL GUI** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

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
