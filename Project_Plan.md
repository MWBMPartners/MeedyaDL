<!-- Copyright (c) 2024-2026 MeedyaDL -->
<!-- Licensed under the MIT License. See LICENSE file in the project root. -->

# 📋 MeedyaDL - Project Plan & Status

> A multiplatform media downloader built with Tauri 2.0 + React + TypeScript

---

## 📌 Current Version

**v0.3.3** (2026-02-14) — All 6 phases complete + post-release features

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

| Platform | Architecture | Status | Format |
|----------|-------------|--------|--------|
| macOS | Apple Silicon (ARM64) | ✅ Complete | `.dmg` |
| Windows | x64 (64-bit) | ✅ Complete | `.exe` (NSIS) |
| Windows | ARM64 | ✅ Complete | `.exe` (NSIS) |
| Linux | x64 | ✅ Complete | `.deb`, `.AppImage` |
| Linux | ARM64 | ✅ Complete | `.deb` |
| Linux | ARMv7 | ✅ Complete | `.deb` |

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
- ✅ Documentation framework (README, Project_Plan, CHANGELOG, help/)
- ✅ Code tooling (ESLint, Prettier, commitlint, git-cliff)
- ✅ Copyright automation script

---

## 🔧 Phase 2: Core Backend (Rust/Tauri)

**Status:** ✅ Complete

Build the Rust services that power the application: Python management, GAMDL installation, dependency downloads, CLI command construction, settings, and credential storage.

### Key Deliverables

#### 2.1 Python Runtime Manager

- ✅ Download portable Python from [python-build-standalone](https://github.com/indygreg/python-build-standalone)
- ✅ Platform-specific builds (macOS ARM, Windows x64, Linux x64, etc.)
- ✅ Install to self-contained app data directory
- ✅ Version tracking and upgrade support

#### 2.2 GAMDL Installation

- ✅ Install GAMDL via `pip install gamdl` into portable Python
- ✅ Version checking via PyPI API
- ✅ Upgrade support with compatibility verification

#### 2.3 Dependency Manager

- ✅ Download and manage: FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box (all required)
- ✅ Platform-specific download URLs and extraction
- ✅ Version tracking and binary verification
- ✅ Display name → tool ID resolution (`resolve_tool_id()`)

#### 2.4 GAMDL CLI Wrapper

- ✅ Construct CLI commands from typed `GamdlOptions` struct
- ✅ Spawn subprocess with stdout/stderr capture
- ✅ Parse output for progress tracking
- ✅ Process lifecycle management (start, monitor, cancel)

#### 2.5 Settings Service

- ✅ App settings persisted as JSON
- ✅ GAMDL config.ini sync
- ✅ Default fallback quality chains
- ✅ Path resolution and validation

#### 2.6 Credential Store

- ✅ OS keychain integration via `keyring` crate
- ✅ Secure storage for wrapper URLs, future API keys
- ✅ Platform: macOS Keychain, Windows Credential Manager, Linux Secret Service

---

## 🎨 Phase 3: Core UI

**Status:** ✅ Complete

Build the React frontend with platform-adaptive styling, navigation, download form, settings, and first-run setup wizard.

### Key Deliverables

#### 3.1 Main Layout

- ✅ Sidebar navigation (Download, Queue, Settings, Help, About)
- ✅ Platform-adaptive title bar (overlay on macOS, standard elsewhere)
- ✅ Status bar showing GAMDL version and connection status

#### 3.2 Download Form

- ✅ URL input with Apple Music content type auto-detection
- ✅ Quality selector with per-download override capability
- ✅ Support for multiple URLs (batch downloads)

#### 3.3 Settings Pages (9 tabs)

1. ✅ **General** - Output path, language, overwrite, updates
2. ✅ **Quality** - Default audio codec, video resolution, format
3. ✅ **Fallback** - Drag-to-reorder fallback chains for music and video
4. ✅ **Paths** - Tool binary paths (FFmpeg, mp4decrypt, etc.)
5. ✅ **Cookies** - Cookie file import, validation, expiry warnings
6. ✅ **Lyrics** - Synced lyrics format (LRC/SRT/TTML)
7. ✅ **Cover Art** - Format (JPG/PNG/Raw), size
8. ✅ **Templates** - Folder and file naming templates
9. ✅ **Advanced** - Wrapper, WVD, download/remux modes

#### 3.4 First-Run Setup Wizard

- ✅ 6-step wizard: Welcome → Python Install → GAMDL Install → Dependencies → Cookie Import → Complete

---

## ⬇️ Phase 4: Download System

**Status:** ✅ Complete

Implement the download queue, fallback quality architecture, progress tracking, and error handling.

### Key Deliverables

#### 4.1 Download Queue

- ✅ Queue-based execution with configurable concurrency
- ✅ Auto-process next item on completion
- ✅ Cancel, retry, remove actions per item

#### 4.2 Fallback Quality Architecture

✅ Default music fallback chain:

1. 🎵 Lossless (ALAC) - 24-bit/192kHz
2. 🎵 Dolby Atmos - Spatial audio
3. 🎵 Dolby Digital (AC3)
4. 🎵 AAC (256kbps) Binaural
5. 🎵 AAC (256kbps at up to 48kHz)
6. 🎵 AAC Legacy (256kbps at up to 44.1kHz)

✅ Default video fallback chain:

1. 🎬 H.265 2160p (4K)
2. 🎬 H.265 1440p
3. 🎬 H.265/H.264 1080p
4. 🎬 H.264 720p → 540p → 480p → 360p → 240p

#### 4.3 Progress Tracking

- ✅ Real-time GAMDL output parsing
- ✅ Per-track progress for albums/playlists
- ✅ Speed and ETA display

#### 4.4 Error Handling

- ✅ Authentication errors → Cookie Settings redirect
- ✅ Codec errors → Automatic fallback
- ✅ Network errors → Auto-retry (3x exponential backoff)
- ✅ Clear error messages with actionable guidance

---

## 🚀 Phase 5: Advanced Features

**Status:** ✅ Complete

### Key Deliverables

- ✅ **Cookie Import UI** - Step-by-step instructions, validation, expiry warnings
- ✅ **Auto-Update Checker** - GAMDL (PyPI), Python, tools, app self-update
- ✅ **In-App Help System** - Markdown renderer, search, 11 help topics
- ✅ **System Tray** - Minimize to tray, download count badge
- ✅ **Service Architecture** - Extensible pattern for future YouTube Music / Spotify support

---

## ✨ Phase 6: Polish & Release

**Status:** ✅ Complete

### Key Deliverables

- ✅ SVG icon set (app icon + UI icons)
- ✅ Platform testing (macOS, Windows, Linux)
- ✅ Complete help documentation (11 topics)
- ✅ Release workflow verification (release-please v4)
- ✅ README with badges and project structure

---

## 🆕 Post-Release Features (v0.1.1 — v0.3.3)

**Status:** ✅ Complete

### Deliverables

- ✅ **Browser cookie auto-import** - Detect installed browsers, extract Apple Music cookies automatically
- ✅ **Embedded Apple Music login window** - Sign in directly within the app to extract cookies (no browser extension needed)
- ✅ **Enhanced error handling** - Improved cookie import feedback and error messages
- ✅ **Animated cover art download** - MusicKit API integration for downloading animated (motion) cover art (FrontCover.mp4, PortraitCover.mp4) via FFmpeg HLS conversion
- ✅ **MusicKit credential management** - Team ID and Key ID in settings, private key in OS keychain, ES256 JWT generation
- ✅ **Animated artwork documentation** - Setup guide, troubleshooting, privacy info
- ✅ **Hidden animated artwork files** - OS-level hidden attribute on downloaded FrontCover.mp4/PortraitCover.mp4 (macOS: `chflags hidden`, Windows: `attrib +H`, Linux: `.` prefix rename). Configurable toggle in Settings > Cover Art, default on.
- ✅ **Configurable companion downloads** - 4 modes (Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy) with [Lossless]/[Dolby Atmos] file suffixes
- ✅ **Lyrics embed + sidecar** - Both embedded in file metadata AND saved as separate sidecar files (LRC/SRT/TTML)
- ✅ **Custom codec metadata tagging** - ALAC: `isLossless=Y`; Atmos: `SpatialType=Dolby Atmos` via mp4ameta freeform atoms
- ✅ **Queue persistence and crash recovery** - Auto-save to `queue.json` after every mutation; auto-resume on startup
- ✅ **Queue export/import** - `.meedyadl` file format with native save/open dialogs for cross-device transfer
- ✅ **Manual workflow dispatch** - `workflow_dispatch` on CI, Changelog, Release Please, Release for conserving Actions minutes
- ✅ **Release-please branch fix** - Corrected branch naming to `release-please--branches--main--components--meedyadl`
- ✅ **Release-please state fix** - Retroactive v0.1.4 tag/release, label update, v0.3.1 tag alignment
- ✅ **Fix Linux ARM cross-compilation** - Restrict default apt sources to amd64, add `ports.ubuntu.com` for ARM packages
- ✅ **Fix release workflow manual dispatch** - Tag input parameter, `shell: bash` for Windows compatibility, checkout by tag ref
- ✅ **Fix tool installation failures on macOS** - Frontend sent display names, backend expected IDs; added `resolve_tool_id()`
- ✅ **Fix mp4decrypt (Bento4) download 404 on macOS** - URL suffix changed to `universal-apple-macosx`
- ✅ **Fix Linux ARM builds** - Skip AppImage (exec format error on x86_64 runners), only produce .deb and .rpm
- ✅ **Mark all four external tools as required** - FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box

---

## 🔮 Future Roadmap

### Overview

| Milestone | Version | Service | Backend Tool | Status |
|-----------|---------|---------|-------------|--------|
| Milestone 7 | v0.4.0 | Spotify | [votify](https://github.com/glomatico/votify) | 🔲 Planned |
| Milestone 8 | v0.5.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | 🔲 Planned |
| Milestone 9 | v0.6.0 | BBC iPlayer | [yt-dlp](https://github.com/yt-dlp/yt-dlp) / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | 🔲 Planned |
| Future | TBD | YouTube Music | [gytmdl](https://github.com/glomatico/gytmdl) | 🔲 Planned |
| Future | TBD | Integration API | Custom | 🔲 Planned |

The architecture is designed with a `MusicService` trait pattern (`src-tauri/src/models/music_service.rs`) to support adding new platforms without restructuring the codebase. Each service follows the same subprocess pattern: a Python CLI tool installed via pip into the portable Python runtime.

---

### Milestone 7 — Spotify Support (v0.4.0)

**Status:** 🔲 Planned

Spotify integration via [votify](https://github.com/glomatico/votify), a Python CLI tool by the same developer as GAMDL. Follows the identical subprocess pattern (`python -m votify ...`), making it the natural first service to add.

#### Spotify Architecture Changes

- Add `Spotify` variant to `MusicServiceId` enum
- Update `url_domains()` to match `open.spotify.com`
- Update `pip_package()` to return `"votify"`
- Generalise download queue to route by `MusicServiceId` (currently hardcoded for GAMDL)

#### Spotify Backend

- 🔲 `services/votify_service.rs` — votify CLI wrapper (install, version check, subprocess execution)
- 🔲 `commands/spotify.rs` — Spotify-specific IPC commands
- 🔲 votify installation in dependency manager (pip install alongside GAMDL)
- 🔲 Spotify OAuth authentication flow (votify uses OAuth, not cookies)
- 🔲 Spotify quality options: OGG Vorbis 320kbps, AAC 256kbps, AAC 128kbps
- 🔲 Spotify fallback quality chain
- 🔲 Spotify URL parsing (tracks, albums, playlists, artists, podcasts)
- 🔲 Multi-service queue routing (service detection from URL → correct CLI tool)

#### Spotify Frontend

- 🔲 Update URL parser to detect `open.spotify.com` URLs
- 🔲 Spotify-specific quality selector (no lossless, no spatial, no video options)
- 🔲 Spotify authentication UI (OAuth flow, not cookie import)
- 🔲 Service indicator in download form showing detected service
- 🔲 Settings tab additions for Spotify-specific options
- 🔲 Update setup wizard to optionally install votify

#### Spotify Capabilities

| Feature        | Supported                                    |
| -------------- | -------------------------------------------- |
| Lossless audio | No                                           |
| Spatial audio  | No                                           |
| Music videos   | No                                           |
| Synced lyrics  | Yes                                          |
| Cover art      | Yes                                          |
| Auth method    | OAuth                                        |
| Content types  | Songs, Albums, Playlists, Artists, Podcasts  |

---

### Milestone 8 — YouTube Support (v0.5.0)

**Status:** 🔲 Planned

YouTube integration via [yt-dlp](https://github.com/yt-dlp/yt-dlp), the most widely-used media download tool. Supports YouTube videos, shorts, playlists, channels, and audio extraction. yt-dlp also serves as the shared backend for BBC iPlayer in Milestone 9.

#### YouTube Architecture Changes

- Add `YouTube` variant to `MusicServiceId` enum (or introduce a broader `MediaServiceId`)
- Update `url_domains()` to match `youtube.com`, `youtu.be`, `music.youtube.com`
- yt-dlp is not a pip package in the same pattern as GAMDL/votify — it's a standalone binary (or pip-installable). Decide: pip install or binary download via dependency manager
- Extend download queue to handle video-only, audio-only, and video+audio downloads

#### YouTube Backend

- 🔲 `services/ytdlp_service.rs` — yt-dlp CLI wrapper (install, version check, subprocess execution)
- 🔲 `commands/youtube.rs` — YouTube-specific IPC commands
- 🔲 yt-dlp installation (pip install or binary download per platform)
- 🔲 YouTube authentication (optional; cookies for age-restricted/private content)
- 🔲 Video quality options: 2160p, 1440p, 1080p, 720p, 480p, 360p, 240p (H.264/H.265)
- 🔲 Audio quality options: best audio, Opus, AAC, MP3 (yt-dlp format selection)
- 🔲 Audio-only extraction mode (download audio stream without video)
- 🔲 YouTube URL parsing (videos, shorts, playlists, channels, mixes)
- 🔲 Progress tracking (yt-dlp stdout parsing for download percentage)
- 🔲 Thumbnail/artwork download

#### YouTube Frontend

- 🔲 Update URL parser to detect `youtube.com`, `youtu.be`, `music.youtube.com` URLs
- 🔲 YouTube-specific quality selector (video resolution + codec + audio format)
- 🔲 Audio-only toggle in download form (extract audio without video container)
- 🔲 YouTube authentication UI (optional cookie import for restricted content)
- 🔲 Settings tab additions for YouTube-specific options (preferred format, audio extraction default)
- 🔲 Update setup wizard to optionally install yt-dlp

#### YouTube Capabilities

| Feature                | Supported                                               |
| ---------------------- | ------------------------------------------------------- |
| Lossless audio         | No (Opus up to 251kbps)                                 |
| Spatial audio          | No                                                      |
| Music videos           | Yes                                                     |
| Synced lyrics          | No (auto-generated subtitles via yt-dlp)                |
| Cover art / thumbnails | Yes                                                     |
| Auth method            | Cookies (optional)                                      |
| Content types          | Videos, Shorts, Playlists, Channels, Music, Mixes       |

---

### Milestone 9 — BBC iPlayer Support (v0.6.0)

**Status:** 🔲 Planned

BBC iPlayer integration for downloading TV programmes, films, and radio shows. Reuses yt-dlp from Milestone 8 (which already supports BBC iPlayer) or uses [get_iplayer](https://github.com/get-iplayer/get_iplayer) as a dedicated alternative.

**Important:** BBC iPlayer content is geographically restricted to the United Kingdom. Users outside the UK will need a VPN or BBC account with UK access.

#### BBC iPlayer Architecture Changes

- Add `BbcIPlayer` variant to `MusicServiceId` (or broader `MediaServiceId` if refactored in Milestone 8)
- Update `url_domains()` to match `bbc.co.uk/iplayer`, `bbc.co.uk/sounds`
- Extend content type detection for TV-specific models (series, episodes, categories)
- Consider renaming `MusicService` trait to `MediaService` to reflect non-music services

#### BBC iPlayer Backend

- 🔲 BBC iPlayer service module (wrapper around yt-dlp with iPlayer-specific options, or get_iplayer)
- 🔲 `commands/iplayer.rs` — BBC iPlayer-specific IPC commands
- 🔲 BBC iPlayer URL parsing (programmes, series, episodes, films, radio/sounds)
- 🔲 Video quality options: HD (720p/1080p), SD (576p) — limited by BBC encoding
- 🔲 Audio/radio download support (BBC Sounds / Radio programmes)
- 🔲 Subtitle download (SRT — BBC provides subtitles for most content)
- 🔲 BBC iPlayer authentication (BBC account sign-in for full access)
- 🔲 Geographic availability detection and user warnings

#### BBC iPlayer Frontend

- 🔲 Update URL parser to detect `bbc.co.uk/iplayer` and `bbc.co.uk/sounds` URLs
- 🔲 BBC iPlayer-specific quality selector (HD/SD for video, audio bitrate for radio)
- 🔲 BBC iPlayer authentication UI (account sign-in)
- 🔲 Subtitle toggle for BBC programmes
- 🔲 Geographic restriction warning banner
- 🔲 Settings tab additions for BBC iPlayer-specific options

#### BBC iPlayer Capabilities

| Feature                | Supported                                                     |
| ---------------------- | ------------------------------------------------------------- |
| HD video               | Yes (720p/1080p)                                              |
| 4K video               | No (not available on iPlayer)                                 |
| Radio / audio          | Yes (BBC Sounds)                                              |
| Subtitles              | Yes (SRT)                                                     |
| Cover art / thumbnails | Yes                                                           |
| Auth method            | BBC account                                                   |
| Content types          | TV Programmes, Films, Series, Episodes, Radio, Podcasts       |
| Geographic restriction | UK only                                                       |

---

### Cross-Cutting Architectural Work

These tasks span multiple milestones and should be addressed incrementally:

- 🔲 **Multi-service download queue** — generalise `download_queue.rs` to dispatch to the correct CLI tool based on detected service
- 🔲 **Service registry** — dynamic service registration in `lib.rs` setup instead of hardcoded GAMDL references
- 🔲 **Per-service settings** — migrate flat `AppSettings` to `Vec<ServiceConfig>` for per-service output paths, auth, and quality defaults
- 🔲 **Rename MusicService → MediaService** — reflect that BBC iPlayer and YouTube are not music-only services
- 🔲 **Shared dependency management** — yt-dlp used by both YouTube (M8) and BBC iPlayer (M9); install once, share across services
- 🔲 **Service-aware fallback chains** — each service defines its own quality fallback chain based on available codecs
- 🔲 **Help documentation** — add per-service help topics (e.g., `help/spotify.md`, `help/youtube.md`, `help/bbc-iplayer.md`)

---

### Future (Beyond v0.6.0)

| Feature | Description | Status |
|---------|-------------|--------|
| **YouTube Music** | Dedicated YouTube Music support via [gytmdl](https://github.com/glomatico/gytmdl) for music-specific features (albums, playlists, lyrics) beyond what yt-dlp provides | 🔲 Planned |
| **Integration API** | REST or IPC API for external apps to trigger downloads programmatically | 🔲 Planned |
| **Localization (i18n)** | Multi-language UI support | 🔲 Planned |
| **Download history** | Persistent download history and statistics dashboard | 🔲 Planned |
| **Custom themes** | User-defined accent colours and theme presets | 🔲 Planned |
| **Multi-track muxing** | Mux companion downloads (e.g. Atmos + AC3 + AAC) into a single MP4 with multiple audio streams and alternate-group metadata for codec-based fallback. Power-user option — requires player support for MP4 alternate audio tracks (standard for video, limited for music players) | 🔲 Planned |
| **Native SwiftUI UI for macOS** | Replace the web-based frontend on Apple Silicon with a fully native SwiftUI interface for tighter macOS integration and performance | 🔲 Idea |

---

## ⚠️ Known Issues / Blockers

None at this time.

---

## 📝 Notes

- **All CLI tools are called as subprocesses** (`python -m gamdl`, `python -m votify`, `yt-dlp`, etc.) to maintain license compatibility
- **All dependencies are self-contained** in the app data directory — no system-wide installations
- **Conventional commits** are used throughout for automated changelog generation
- **Every source file** includes copyright headers with automated year updates
- **yt-dlp is shared** between YouTube (M8) and BBC iPlayer (M9) — install once, configure per-service

---

*Last updated: 2026-02-14*

(c) 2024-2026 MeedyaDL
