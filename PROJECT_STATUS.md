# Project Status

> **MeedyaDL** — A multiplatform media downloader built with Tauri 2.0, React, and TypeScript.

---

## Current Version

**v0.3.1** (2026-02-13)

## Overall Status

### All 6 Phases COMPLETE + Post-Release Features

---

## Phase Breakdown

### Phase 1: Project Foundation (COMPLETE)

- [x] Clean up old PyQt5 code
- [x] Initialize Tauri 2.0 + React + TypeScript scaffold
- [x] Set up project directory structure
- [x] Configure npm dependencies
- [x] Configure Rust/Cargo dependencies
- [x] Set up Tailwind CSS with platform themes
- [x] Set up code tooling (ESLint, Prettier, commitlint)
- [x] Create GitHub Actions workflows (CI, Release, Changelog)
- [x] Create documentation (README, Project_Plan, PROJECT_STATUS, CHANGELOG)
- [x] Update .gitignore
- [x] Create help documentation stubs
- [x] Set up copyright automation

### Phase 2: Core Backend (COMPLETE)

- [x] Platform utilities (OS detection, path resolution)
- [x] Archive utilities (ZIP, TAR.GZ extraction)
- [x] Python runtime manager (download, install, verify)
- [x] GAMDL pip installer
- [x] Dependency manager (FFmpeg, mp4decrypt, etc.)
- [x] GAMDL CLI wrapper (command builder, subprocess execution)
- [x] Settings service (JSON load/save + GAMDL config.ini sync)
- [x] Credential store (OS keychain via keyring crate)
- [x] IPC command handlers (system, dependencies, settings, gamdl, credentials)
- [x] Process output parser (regex-based GAMDL stdout/stderr parsing)

### Phase 3: Core UI (COMPLETE)

- [x] TypeScript type definitions (mirrors all Rust models)
- [x] Typed Tauri command wrappers (type-safe invoke())
- [x] Zustand state stores (ui, settings, download, dependency, setup)
- [x] Platform detection hook (usePlatform)
- [x] Common UI components (Button, Input, Select, Toggle, Modal, Toast, LoadingSpinner, Tooltip, FilePickerButton, ProgressBar)
- [x] Main layout (Sidebar, TitleBar, StatusBar, PageHeader, MainLayout)
- [x] Download form (URL input with content-type detection, quality overrides)
- [x] Settings pages (9 tabs: General, Quality, Fallback, Paths, Cookies, Lyrics, Cover Art, Templates, Advanced)
- [x] First-run setup wizard (6 steps: Welcome, Python, GAMDL, Dependencies, Cookies, Complete)
- [x] Help viewer (11 topics with sidebar navigation, ReactMarkdown rendering)
- [x] URL parser (Apple Music content type detection)
- [x] App.tsx routing (setup wizard vs main app, event listeners)

### Phase 4: Download System (COMPLETE)

- [x] Download queue manager (VecDeque-based, concurrent execution limits, Arc<Mutex<>> thread-safe state)
- [x] Fallback quality architecture (music codec chain + video resolution chain with auto-retry)
- [x] Progress tracking (real-time stdout/stderr parsing with Tauri event emission)
- [x] Error classification (auth, network, codec, not_found, rate_limit, tool, unknown)
- [x] Network retry (3x automatic retry with exponential backoff for network errors)
- [x] Download queue UI (cancel, retry, clear finished, progress bars, fallback indicators)
- [x] Event system (download-queued, download-started, download-complete, download-error, download-cancelled)
- [x] Tauri managed state (QueueHandle injected into command handlers)

### Phase 5: Advanced Features (COMPLETE)

- [x] Cookie import UI with step-by-step instructions, domain display, expiry warnings, copy path
- [x] Auto-update checker (PyPI for GAMDL, GitHub Releases for app, Python version comparison)
- [x] Update notification banner with dismiss, upgrade GAMDL, and view release actions
- [x] Update Zustand store (checkForUpdates, upgradeGamdl, dismissUpdate)
- [x] In-app help system with search filtering and highlighted matches
- [x] System tray integration (show window, download status, check for updates, quit)
- [x] Tray event bridge (tray-check-updates Tauri event to frontend)
- [x] Future service architecture trait (MusicService trait, MusicServiceId enum, ServiceCapabilities)
- [x] Frontend extensibility types (MusicServiceId, ServiceCapabilities, MUSIC_SERVICE_LABELS)

### Phase 6: Polish & Release (COMPLETE)

- [x] SVG icon creation and PNG/ICO/ICNS generation from source SVG
- [x] ESLint 9.x flat config setup
- [x] Vitest configuration with Tauri mocks and test setup
- [x] URL parser unit tests (13 tests)
- [x] Fix Rust clippy warnings (new_without_default, field_reassign, ptr_arg)
- [x] Fix CI workflow (add lint step, fix test discovery)
- [x] Fix changelog workflow (remove unnecessary GitHub API calls)
- [x] Detailed code comments on all 70+ source files (Rust backend, React frontend, config, scripts, workflows, CSS)
- [x] Complete help documentation (11 topics with full content: Getting Started, Downloading Music/Videos, Lyrics & Metadata, Quality Settings, Fallback Quality, Cookie Management, Animated Artwork, Troubleshooting, FAQ)
- [x] README finalization (fixed badges for private repo, fixed broken Project_Plan link)
- [x] Platform testing (macOS Apple Silicon: frontend builds, Rust release compiles in 2m05s, GAMDL.app bundle created)
- [x] Release workflow review (CI and Release workflows verified: matrix builds, concurrency, caching, Linux deps, artifact upload)
- [x] Copyright year automation (expanded script to cover all 121 files, auto-detect macOS/Linux, self-corruption guard)

### Post-Release Features (v0.1.1 — v0.3.1)

- [x] Browser cookie auto-import service (detect installed browsers, extract cookies automatically)
- [x] Embedded Apple Music login window (sign in directly in-app, extract cookies from webview)
- [x] Enhanced error handling and cookie import feedback
- [x] Animated cover art download via Apple MusicKit API (FrontCover.mp4, PortraitCover.mp4)
- [x] MusicKit credential management (Team ID, Key ID in settings; private key in OS keychain)
- [x] Animated artwork help documentation page
- [x] Hidden animated artwork files (OS-level hidden attribute: macOS `chflags hidden`, Windows `attrib +H`, Linux `.` prefix rename; configurable toggle, default on)
- [x] Configurable companion downloads (4 modes: Disabled, Atmos→ALAC, Atmos→ALAC+Lossy, Specialist→Lossy; with [Lossless]/[Dolby Atmos] file suffixes)
- [x] Embed lyrics and keep sidecar (ensures both embedded lyrics and sidecar files for max compatibility)
- [x] Custom codec metadata tagging (ALAC: isLossless=Y; Atmos: SpatialType=Dolby Atmos via mp4ameta freeform atoms)
- [x] Queue persistence and crash recovery (queue.json auto-save after every mutation; auto-resume on startup)
- [x] Queue export/import (.meedyadl file format with native save/open dialogs; cross-device transfer)
- [x] Manual workflow dispatch (workflow_dispatch on CI, Changelog, Release Please, Release for conserving Actions minutes)
- [x] Fix release-please branch naming (release-please--branches--main--components--meedyadl)
- [x] Fix release-please stuck on stale PR #4 (retroactive v0.1.4 tag/release, label update, v0.3.1 tag alignment)
- [x] Fix Linux ARM cross-compilation (restrict default apt sources to amd64, add `ports.ubuntu.com` for ARM packages)
- [x] Fix release workflow manual dispatch (tag input parameter, `shell: bash` for Windows compatibility, checkout by tag ref)

### Planned Milestones

| Milestone | Version | Service | Engine | Status |
| --------- | ------- | ------- | ------ | ------ |
| **M7** | v0.4.0 | Spotify | votify | Planned |
| **M8** | v0.5.0 | YouTube | yt-dlp | Planned |
| **M9** | v0.6.0 | BBC iPlayer | yt-dlp / get_iplayer | Planned |

Each milestone adds a new media service with its own CLI subprocess engine, URL parser, settings tab, and help documentation. The existing `MusicService` trait will be renamed to `MediaService` to accommodate non-music services. See [Project Plan](Project_Plan.md) for full milestone details.

---

## Known Issues / Blockers

_None at this time._

---

## Last Updated

2026-02-14

---

(c) 2024-2026 MeedyaDL
