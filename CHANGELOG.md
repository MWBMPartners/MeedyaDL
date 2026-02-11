# Changelog

All notable changes to **GAMDL GUI** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### ✨ Features

<<<<<<< HEAD
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
=======
#### Icons & Tooling

- App icon generated from SVG source with proper RGBA PNGs, ICO, and ICNS
- ESLint 9.x flat config for TypeScript + React linting
- Vitest test framework with jsdom environment, Tauri API mocks, and path aliases
- URL parser unit tests (13 tests covering all content types)
- Fixed 4 Rust clippy warnings (new_without_default, field_reassign_with_default, ptr_arg)
- CI workflow updated with ESLint lint step
- Changelog workflow fixed (removed unnecessary GitHub API metadata fetching)
- Icon generation script (`scripts/generate-icons.mjs`) for reproducible builds

#### Code Documentation

- Detailed comments added to all 70+ source files across the entire codebase
- Rust backend: entry points, models, services, commands, and utilities fully documented
- React frontend: core files, stores, common components, layout/download components, settings/setup/help components
- Configuration files, build scripts, GitHub Actions workflows, and CSS theme files documented
- Every function, struct, enum, component, hook, and significant code block has explanatory comments

#### Help Documentation

- 10 help topics written with full, production-quality content replacing all placeholder stubs
- Getting Started: system requirements, installation per platform, 6-step setup wizard walkthrough
- Downloading Music: URL types, codec table, queue management, error types, file naming
- Downloading Videos: resolutions, video codecs, fallback chain, subtitles, metadata
- Lyrics and Metadata: LRC/SRT/TTML formats with examples, 13 metadata fields, cover art options
- Quality Settings: all 9 audio codecs, 8 video resolutions, comparison tables, recommendations
- Fallback Quality: chain concept, default chains, drag-to-reorder UI, per-track behavior, examples
- Cookie Management: browser-specific export (Chrome/Firefox/Safari/Edge), import, validation, security
- Troubleshooting: 7 error categories with causes/solutions, platform launch issues, log files, verbose logging
- FAQ: 20+ Q&A entries covering general, authentication, downloads, quality, lyrics, technical topics

#### README

- Fixed version badge (static badge for private repo compatibility)
- Fixed CI badge (GitHub-native workflow badge URL)
- Fixed broken `docs/Project_Plan.md` link (now points to root `Project_Plan.md`)

### Phase 5: Advanced Features
>>>>>>> 118f489 (Enhance documentation and comments across configuration files)

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

<<<<<<< HEAD
- Implement icon generation script, ESLint configuration, and Vitest setup for testing
=======
### Phase 3: Core UI

- 10 common UI components (Button, Input, Select, Toggle, Modal, Toast, LoadingSpinner, Tooltip, FilePickerButton, ProgressBar)
- Main layout with collapsible sidebar, custom title bar, and status bar
- Download form with Apple Music URL validation and content-type detection
- Quality override panel for per-download codec/resolution selection
- Download queue page with progress bars, cancel/retry actions, fallback indicators
- 9 settings tabs (General, Quality, Fallback, Paths, Cookies, Lyrics, Cover Art, Templates, Advanced)
- Drag-to-reorder fallback chains in Settings > Fallback tab
- 6-step first-run setup wizard (Welcome, Python, GAMDL, Dependencies, Cookies, Complete)
- Help viewer with 9 topics and sidebar navigation using ReactMarkdown + remarkGfm
- 5 Zustand state stores (ui, settings, download, dependency, setup)
- Type-safe Tauri command wrappers for all IPC calls
- URL parser detecting song/album/playlist/music-video/artist content types
- Platform detection hook with dynamic CSS theme loading

### Phase 2: Core Backend

- Python runtime manager: download/install/verify portable Python from python-build-standalone
- GAMDL CLI wrapper: install via pip, build typed commands, spawn subprocesses
- Dependency manager: download/install FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box per platform
- Settings service: JSON load/save with GAMDL config.ini sync
- Credential store: OS keychain integration (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Process output parser: regex-based GAMDL stdout/stderr parsing into structured events
- Complete GAMDL options model with all 11 audio codecs, 8 video resolutions, and all CLI flags
- Application settings model with fallback quality chain defaults
- Cookie file validation (Netscape format parsing with expiry detection)
- IPC command handlers for system, dependencies, settings, gamdl, and credentials
- Platform utilities (OS/arch detection, app data directory resolution)
- Archive utilities (ZIP/TAR.GZ/TAR.XZ extraction with progress)

### Phase 1: Foundation

- Initial project scaffold with Tauri 2.0 + React + TypeScript
- Platform-adaptive CSS themes (macOS Liquid Glass, Windows Fluent, Linux Adwaita)
- Secure credential storage via OS keychain
- IPC command framework bridging React frontend to Rust backend

#### Build System

- GitHub Actions CI workflow (lint, type-check, test on macOS/Windows/Linux)
- GitHub Actions Release workflow (build .dmg/.msi/.deb/.AppImage on tag push)
- Automated CHANGELOG generation via git-cliff
- Conventional commit linting via commitlint
- Copyright year automation script

#### Documentation

- Comprehensive README with feature list, architecture overview, and build instructions
- Project Plan with 6-phase implementation roadmap
- Project Status tracker with phase checkboxes
- Help documentation stubs (10 topics, expanded to full content in Phase 6)
>>>>>>> 118f489 (Enhance documentation and comments across configuration files)

---
*Generated with [git-cliff](https://git-cliff.org/)*
