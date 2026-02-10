
# Changelog

All notable changes to **GAMDL GUI** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### Phase 6: Polish & Release

- App icon generated from SVG source with proper RGBA PNGs, ICO, and ICNS
- ESLint 9.x flat config for TypeScript + React linting
- Vitest test framework with jsdom environment, Tauri API mocks, and path aliases
- URL parser unit tests (13 tests covering all content types)
- Fixed 4 Rust clippy warnings (new_without_default, field_reassign_with_default, ptr_arg)
- CI workflow updated with ESLint lint step
- Changelog workflow fixed (removed unnecessary GitHub API metadata fetching)
- Icon generation script (`scripts/generate-icons.mjs`) for reproducible builds

### Phase 5: Advanced Features

- Auto-update checker service checking GAMDL (PyPI), app (GitHub Releases), and Python versions
- Update notification banner with per-component upgrade/dismiss/view-release actions
- GAMDL one-click upgrade via pip from the update banner
- Version compatibility gate preventing upgrades to incompatible GAMDL versions
- Enhanced cookie import UI with step-by-step browser instructions (Chrome, Firefox, Edge, Safari)
- Cookie validation with domain detection, expiry warnings, and status badges
- Help viewer search with topic filtering, match highlighting, and result count
- System tray integration with context menu (Show Window, Downloads status, Check for Updates, Quit)
- Tray icon left-click to show/focus main window
- MusicService trait for future service extensibility (gytmdl, votify)
- MusicServiceId enum with URL domain detection for Apple Music, YouTube Music, Spotify
- ServiceCapabilities model describing per-service feature support
- Update Zustand store with check/upgrade/dismiss lifecycle
- Frontend types for ComponentUpdate, UpdateCheckResult, MusicServiceId, ServiceCapabilities

### Phase 4: Download System

#### Features

- Download queue manager with concurrent execution limits (default: 1)
- Fallback quality chain: automatic codec fallback on codec-unavailable errors
  - Music: ALAC -> Atmos -> AC3 -> AAC Binaural -> AAC -> AAC Legacy
  - Video: 2160p -> 1440p -> 1080p -> ... -> 240p
- Network retry with automatic 3x retry for transient network errors
- Error classification system (auth, network, codec, not_found, rate_limit, tool, unknown)
- Real-time progress tracking via Tauri event system
- Cancel, retry, and clear-finished queue actions
- Tauri managed state for thread-safe queue access (Arc<Mutex<>>)
- Download lifecycle events (queued, started, complete, error, cancelled)

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
- Help documentation stubs (10 topics)

---
*Generated with [git-cliff](https://git-cliff.org/)*
