<!-- Copyright (c) 2024-2026 MeedyaDL -->
<!-- Licensed under the MIT License. See LICENSE file in the project root. -->

# 📋 MeedyaDL - Project Plan

> A multiplatform media downloader built with Tauri 2.0 + React + TypeScript

---

## 🎯 Project Overview

**MeedyaDL** is a multiplatform media downloader providing a user-friendly graphical interface. Currently supports Apple Music via GAMDL, with planned support for additional media services. Runs on macOS, Windows, Linux, and Raspberry Pi.

### Architecture

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Frontend** | React + TypeScript | User interface components |
| **Build Tool** | Vite | Fast frontend bundling |
| **Styling** | Tailwind CSS | Platform-adaptive themes |
| **Desktop Framework** | Tauri 2.0 | Native window, IPC, plugins |
| **Backend** | Rust | Services, process management |
| **State** | Zustand | Frontend state management |
| **CI/CD** | GitHub Actions | Automated builds & releases |

### Platform Support

| Platform | Architecture | Status | Priority |
|----------|-------------|--------|----------|
| macOS | Apple Silicon (ARM64) | 🟡 In Progress | Mandatory |
| Windows | x64 | 🟡 In Progress | Mandatory |
| Windows | ARM64 | 🔲 Planned | Mandatory |
| Windows | x86 | 🔲 Planned | Mandatory |
| Linux | x64 | 🔲 Planned | Good to have |
| Raspberry Pi | ARM64 | 🔲 Planned | Good to have |

---

## 📦 Phase 1: Project Foundation

**Status:** ✅ Complete

Replaced the old PyQt5 prototype with a modern Tauri 2.0 + React + TypeScript scaffold.

### Deliverables
- ✅ Project directory structure (src-tauri, src, help, assets, scripts)
- ✅ Tauri configuration (tauri.conf.json, capabilities, plugins)
- ✅ React + TypeScript + Vite frontend scaffold
- ✅ Tailwind CSS with platform-adaptive themes (macOS, Windows, Linux)
- ✅ Rust backend with command/service/model/util module structure
- ✅ All GAMDL CLI options modeled as typed Rust enums/structs
- ✅ GitHub Actions: CI (lint+test+build), Release (multi-platform), Changelog (auto-generate)
- ✅ Documentation framework (README, Project_Plan, PROJECT_STATUS, CHANGELOG, help/)
- ✅ Code tooling (ESLint, Prettier, commitlint, git-cliff)
- ✅ Copyright automation script

---

## 🔧 Phase 2: Core Backend (Rust/Tauri)

**Status:** 🔲 Planned

Build the Rust services that power the application: Python management, GAMDL installation, dependency downloads, CLI command construction, settings, and credential storage.

### Key Deliverables

#### 2.1 Python Runtime Manager
- Download portable Python from [python-build-standalone](https://github.com/indygreg/python-build-standalone)
- Platform-specific builds (macOS ARM, Windows x64, Linux x64, etc.)
- Install to self-contained app data directory
- Version tracking and upgrade support

#### 2.2 GAMDL Installation
- Install GAMDL via `pip install gamdl` into portable Python
- Version checking via PyPI API
- Upgrade support with compatibility verification

#### 2.3 Dependency Manager
- Download and manage: FFmpeg (required), mp4decrypt, N_m3u8DL-RE, MP4Box (optional)
- Platform-specific download URLs and extraction
- Version tracking and binary verification

#### 2.4 GAMDL CLI Wrapper
- Construct CLI commands from typed `GamdlOptions` struct
- Spawn subprocess with stdout/stderr capture
- Parse output for progress tracking
- Process lifecycle management (start, monitor, cancel)

#### 2.5 Settings Service
- App settings persisted as JSON
- GAMDL config.ini sync
- Default fallback quality chains
- Path resolution and validation

#### 2.6 Credential Store
- OS keychain integration via `keyring` crate
- Secure storage for wrapper URLs, future API keys
- Platform: macOS Keychain, Windows Credential Manager, Linux Secret Service

---

## 🎨 Phase 3: Core UI

**Status:** 🔲 Planned

Build the React frontend with platform-adaptive styling, navigation, download form, settings, and first-run setup wizard.

### Key Deliverables

#### 3.1 Main Layout
- Sidebar navigation (Download, Queue, Settings, Help, About)
- Platform-adaptive title bar (overlay on macOS, standard elsewhere)
- Status bar showing GAMDL version and connection status

#### 3.2 Download Form
- URL input with Apple Music content type auto-detection
- Quality selector with per-download override capability
- Support for multiple URLs (batch downloads)

#### 3.3 Settings Pages (9 tabs)
1. **General** - Output path, language, overwrite, updates
2. **Quality** - Default audio codec, video resolution, format
3. **Fallback** - Drag-to-reorder fallback chains for music and video
4. **Paths** - Tool binary paths (FFmpeg, mp4decrypt, etc.)
5. **Cookies** - Cookie file import, validation, expiry warnings
6. **Lyrics** - Synced lyrics format (LRC/SRT/TTML)
7. **Cover Art** - Format (JPG/PNG/Raw), size
8. **Templates** - Folder and file naming templates
9. **Advanced** - Wrapper, WVD, download/remux modes

#### 3.4 First-Run Setup Wizard
6-step wizard: Welcome → Python Install → GAMDL Install → Dependencies → Cookie Import → Complete

---

## ⬇️ Phase 4: Download System

**Status:** 🔲 Planned

Implement the download queue, fallback quality architecture, progress tracking, and error handling.

### Key Deliverables

#### 4.1 Download Queue
- Queue-based execution with configurable concurrency
- Auto-process next item on completion
- Cancel, retry, remove actions per item

#### 4.2 Fallback Quality Architecture
Default music fallback chain:
1. 🎵 Lossless (ALAC) - 24-bit/192kHz
2. 🎵 Dolby Atmos - Spatial audio
3. 🎵 Dolby Digital (AC3)
4. 🎵 AAC (256kbps) Binaural
5. 🎵 AAC (256kbps at up to 48kHz)
6. 🎵 AAC Legacy (256kbps at up to 44.1kHz)

Default video fallback chain:
1. 🎬 H.265 2160p (4K)
2. 🎬 H.265 1440p
3. 🎬 H.265/H.264 1080p
4. 🎬 H.264 720p → 540p → 480p → 360p → 240p

#### 4.3 Progress Tracking
- Real-time GAMDL output parsing
- Per-track progress for albums/playlists
- Speed and ETA display

#### 4.4 Error Handling
- Authentication errors → Cookie Settings redirect
- Codec errors → Automatic fallback
- Network errors → Auto-retry (3x exponential backoff)
- Clear error messages with actionable guidance

---

## 🚀 Phase 5: Advanced Features

**Status:** 🔲 Planned

### Key Deliverables
- **Cookie Import UI** - Step-by-step instructions, validation, expiry warnings
- **Auto-Update Checker** - GAMDL (PyPI), Python, tools, app self-update
- **In-App Help System** - Markdown renderer, search, 10 help topics
- **System Tray** - Minimize to tray, download count badge
- **Service Architecture** - Extensible pattern for future YouTube Music / Spotify support

---

## ✨ Phase 6: Polish & Release

**Status:** 🔲 Planned

### Key Deliverables
- SVG icon set (app icon + UI icons)
- Platform testing (macOS, Windows, Linux)
- Complete help documentation
- Release workflow verification
- README with screenshots

---

## 🔮 Future Roadmap

| Feature | Service | Library | Status |
|---------|---------|---------|--------|
| YouTube Music downloads | YouTube Music | [gytmdl](https://github.com/glomatico/gytmdl) | 🔲 Planned |
| Spotify downloads | Spotify | [votify](https://github.com/glomatico/votify) | 🔲 Planned |
| Integration API | External apps | Custom | 🔲 Planned |

The architecture is designed with a `MusicService` trait pattern to support adding new music platforms without restructuring the codebase.

---

## 📝 Notes

- **GAMDL is called as a CLI subprocess** (`python -m gamdl ...`) to maintain MIT license compatibility
- **All dependencies are self-contained** in the app data directory - no system-wide installations
- **Conventional commits** are used throughout for automated changelog generation
- **Every source file** includes copyright headers with automated year updates

---

*Last updated: 2026-02-09*
