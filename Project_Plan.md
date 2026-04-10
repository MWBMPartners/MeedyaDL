<!-- Copyright (c) 2026 MeedyaDL -->
<!-- Licensed under the MIT License. See LICENSE file in the project root. -->

# 📋 MeedyaDL - Project Plan & Status

> A multiplatform media downloader built with Tauri 2.0 + React + TypeScript

---

## 📌 Current Version

**v0.29.4** (2026-03-30) — All 6 phases complete + post-release features <!-- x-release-please-version -->

---

## 🎯 Project Overview

**MeedyaDL** is a multiplatform media downloader providing a user-friendly graphical interface. Currently supports Apple Music via GAMDL, with planned support for additional media services. Runs on macOS, Windows, Linux, and Raspberry Pi.

### Architecture

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Frontend** | React + TypeScript | User interface components |
| **Build Tool** | Vite | Fast frontend bundling |
| **Styling** | Tailwind CSS v4 | Platform-adaptive themes |
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
- ✅ Tailwind CSS v4 with platform-adaptive themes (macOS, Windows, Linux)
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
- ✅ System PATH detection — check for existing tool installs before downloading
- ✅ Minimum version requirements via `tool-versions.toml` config file

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

#### 3.3 Settings Pages (10 tabs)

1. ✅ **General** - Output path, language, overwrite, auto-start queue, updates
2. ✅ **Quality** - Default audio codec, video resolution, format
3. ✅ **Fallback** - Drag-to-reorder fallback chains for music and video
4. ✅ **Paths** - Temp directory, tool binary paths (FFmpeg, mp4decrypt, etc.)
5. ✅ **Cookies** - Cookie file import, validation, expiry warnings
6. ✅ **Lyrics** - Synced lyrics format (LRC/SRT/TTML)
7. ✅ **Cover Art** - Format (JPG/PNG/Raw), size, animated artwork
8. ✅ **Metadata** - AcoustID fingerprinting, ReplayGain analysis
9. ✅ **Templates** - Folder and file naming templates
10. ✅ **Advanced** - Wrapper, WVD, download/remux modes

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

## 🆕 Post-Release Features (v0.1.1 — v0.3.11+)

**Status:** ✅ Complete

### Deliverables

- ✅ **Browser cookie auto-import** - Detect installed browsers, extract Apple Music cookies automatically
- ✅ **Embedded Apple Music login window** - Sign in directly within the app to extract cookies (no browser extension needed)
- ✅ **Enhanced error handling** - Improved cookie import feedback and error messages
- ✅ **Animated cover art download** - MusicKit API integration for downloading animated (motion) cover art (FrontCover.mp4, PortraitCover.mp4) via FFmpeg HLS conversion
- ✅ **MusicKit credential management** - Team ID and Key ID in settings, private key in OS keychain, ES256 JWT generation
- ✅ **Animated artwork documentation** - Setup guide, troubleshooting, privacy info
- ✅ **Hidden animated artwork files** - OS-level hidden attribute on downloaded FrontCover.mp4/PortraitCover.mp4 (macOS: `chflags hidden`, Windows: `attrib +H`, Linux: `.` prefix rename). Configurable toggle in Settings > Cover Art, default on.
- ✅ **Configurable companion downloads** - 5 preset modes (Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy, Atmos→All Formats) plus Custom mode with multi-select codec checkboxes, with [Lossless]/[Dolby Atmos] file suffixes
- ✅ **Multi-select artist auto-select** - Checkbox group for selecting multiple artist content types (Main Albums, Singles & EPs, Music Videos, etc.) simultaneously. MeedyaDL creates N separate queue items for artist URLs (one per selected mode) since GAMDL only accepts a single `--artist-auto-select` value per invocation.
- ✅ **Lyrics embed + sidecar** - Both embedded in file metadata AND saved as separate sidecar files (LRC/SRT/TTML)
- ✅ **Metadata enrichment** - Comprehensive post-download enrichment: codec tags, source/channel tags, Apple Music API metadata (ISRC, UPC, genre, advisory, artist IDs, artwork URLs). Shared `apple_music_api.rs` module for MusicKit JWT, URL parsing, and catalog API.
- ✅ **AcoustID fingerprinting** (opt-in) - Chromaprint audio fingerprints via embedded rusty-chromaprint library + acoustid.org API lookup. Writes `Acoustid Id` and `Acoustid Fingerprint` tags. No external binary required. API key embedded at compile time via `ACOUSTID_API_KEY` env var; users can override with custom key in Settings > Metadata.
- ✅ **Music video companion downloads** (opt-in) - Automatically find and download music videos for audio tracks via Apple Music API `relate=music-videos`. Requires MusicKit credentials. Videos use video quality settings (resolution, codec priority, remux format). Toggle in Settings > Quality > Video Quality, disabled when MusicKit credentials not configured. Enrichment pipeline Step 6.
- ✅ **Visual template builder** - Interactive chip/pill-based UI (`TemplateBuilder` component) for building GAMDL file/folder naming templates. Replaces plain text inputs with visual segment composition. Template variables and common literals available in dropdown menu; raw-edit toggle for power users. Real-time preview rendering with sample metadata.
- ✅ **ReplayGain loudness analysis** (opt-in) - FFmpeg EBU R128 filter for non-destructive volume normalisation tags (`replaygain_track_gain`, `replaygain_track_peak`)
- ✅ **Enhanced LRC with word-by-word sync** - Automatically converts Apple Music TTML lyrics to Enhanced LRC with word-level synchronized timestamps (`<mm:ss.xx>` inline word timing). Parses TTML XML via `roxmltree`, extracts `<span>` word timing from `itunes:timing="Word"` documents, generates backward-compatible Enhanced LRC. Saves `.lrc` sidecar and embeds in M4A/M4V metadata via `©lyr` atom. Falls back to standard line-level LRC for songs without word-level data. New `enhanced_lyrics_service.rs` module.
- ✅ **Queue persistence and crash recovery** - Auto-save to `queue.json` after every mutation; auto-resume on startup. Failed downloads persist across restarts with error messages for manual retry
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
- ✅ **Fix setup re-check on reopen** - Wizard now re-appears if any required tool is missing (not just Python + GAMDL)
- ✅ **Disclaimer** - Added to both the first-run setup welcome screen and Help documentation section
- ✅ **Fix help documentation formatting** - Installed `@tailwindcss/typography` plugin; added platform-adaptive prose color overrides so headings, paragraphs, and links render correctly across all themes
- ✅ **macOS code signing entitlements** - Added `Entitlements.plist` with hardened runtime permissions for subprocess execution and network access
- ✅ **Fix N_m3u8DL-RE download 404** - Use GitHub API for dynamic asset URL resolution (naming conventions change between releases)
- ✅ **Fix MP4Box macOS auto-install** - Try Homebrew first (`brew install gpac`), fall back to GPAC `.pkg` extraction
- ✅ **Fix mp4decrypt (Bento4) download 404 on Windows/Linux** - Naming changed at build 633: `win32` → `x86_64-microsoft-win32`, `linux-x86_64` → `x86_64-unknown-linux`
- ✅ **Fix MP4Box on Windows** - GPAC discontinued ZIP archives; now downloads NSIS `.exe` installer and runs silently
- ✅ **Fix MP4Box on Linux** - GPAC discontinued tarballs; now downloads `.deb` and extracts via `ar` + `tar`
- ✅ **System PATH detection for external tools** - Checks system PATH for FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box before downloading; copies compatible system binaries to managed directory
- ✅ **Tool version requirements config** - New `tool-versions.toml` with minimum versions per tool, compiled into binary via `include_str!()`
- ✅ **Dependency source tracking** - `DependencyStatus.source` field ("system" or "managed") with "System" badge in setup wizard
- ✅ **Fix ARMv7 artifact naming** - Split build/upload for ARMv7; rename `armhf`/`armhfp` → `armv7` before uploading to release
- ✅ **Improved release page download guidance** - User-friendly download table with plain-language platform descriptions
- ✅ **UI stall prevention on FUSE mounts** (v0.6.2) - Wrapped blocking mp4ameta Tag I/O in `spawn_blocking()` across 4 enrichment services. Added yield points between enrichment steps. Prevents tokio runtime starvation on slow CloudMounter/NFS mounts.
- ✅ **Codec suffix fix for native priority** (v0.6.2) - Primary download uses clean filenames when native `--song-codec-priority` is active (actual codec unknown until GAMDL finishes). All companion tiers get forced suffixes via `force_all_suffixes` parameter.
- ✅ **Lyrics format fallback chain** (v0.6.3) - When primary lyrics format (TTML) fails, automatically retries with LRC (audio) or SRT (video). Content-type-aware ordering. New `lyrics_fallback_enabled` setting (default: true).
- ✅ **Per-endpoint pre-flight logging** (v0.6.3) - Internet connectivity check now logs each endpoint's result (name, URL, HTTP status, response time) for diagnostics.
- ✅ **Fix --song-codec flag removal** (v0.6.3) - GAMDL 2.9.1 removed `--song-codec`; companion downloads and fallback retries now use `--song-codec-priority` with single codec values.
- ✅ **Fix Enhanced LRC blocking companion lyrics** (v0.6.3) - When Enhanced LRC was enabled, lyrics format checkboxes were disabled. Now TTML stays locked as primary but LRC/SRT can be selected as companions.
- ✅ **Fix file picker Browse defaultPath** (v0.6.3) - Browse buttons now open at the currently configured path instead of the OS-remembered last-used directory.
- ✅ **Internal codec and format registry** (v0.6.3) - TOML-based codec registry (`codecs.toml`) with 16 audio codecs, 3 meta codecs, 4 video codecs, 5 lyrics formats. Per-service CLI flag mappings, MIME types, and bridge functions to existing SongCodec enum. Background preparation for future planned features.
- ✅ **JS obfuscation for release builds** (v0.6.3) - Terser-based minification with aggressive name mangling, console stripping, and dead code elimination. Zero runtime performance impact (build-time only). Debug builds unaffected.
- ✅ **WebVTT subtitle generation** (v0.6.4) - Enrichment Step 2c generates WebVTT (`.vtt`) subtitle files from existing lyrics sidecars. Source priority: TTML (richest timing data with word-level `<span>` support), SRT (has start+end times), LRC (start times only, 3s estimated duration). New `generate_webvtt` setting (default: false). Toggle in Settings > Lyrics.
- ✅ **MusicBrainz recording lookup** (v0.6.4) - Enrichment Step 6b discovers music videos and cross-platform URLs via the MusicBrainz database. 3-tier priority chain: (1) Apple Music URL search in MB external links, (2) ISRC code search, (3) AcoustID recording ID direct lookup. No credentials required (free public API). Rate-limited to 1 req/sec. Returns all discovered platform URLs (Apple Music, YouTube, Spotify, Deezer, Tidal, SoundCloud, Bandcamp) for future cross-platform song discovery. Storefront-aware URL rewriting. New `musicbrainz_lookup` setting (default: false). Toggle in Settings > Quality.
- ✅ **Pre-release flag for all releases** (v0.6.4) - All GitHub Releases marked as pre-release until v1.0. Release download table includes direct download links, platform emojis, and Raspberry Pi GDebi installation note.
- ✅ **Release title format** (v0.6.4) - GitHub Release titles use just the version tag (e.g., `v0.6.4`) without app name prefix for cleaner release listing
- ✅ **Rosetta 2 detection on Apple Silicon** - Checks if Rosetta 2 is installed before downloading x86_64 binaries (FFmpeg, MP4Box .pkg); refuses with Homebrew guidance if unavailable
- ✅ **Fallback mirror for tool downloads** - When primary upstream sources fail, falls back to `MeedyaDL/MeedyaDL-Tools` GitHub Releases with standardized asset naming (`{tool_id}-{os}-{arch}.{ext}`)
- ✅ **Generic GitHub API resolver** - Reusable `resolve_github_release_asset()` for upstream release queries and mirror fallback (refactored from N_m3u8DL-RE inline code)
- ✅ **Three-tier download fallback** - System PATH → Primary upstream → Mirror repository → Error with guidance
- ✅ **Auto-start queue setting** - `auto_start_queue` toggle in Settings > General (default: on). When disabled, items queue up and the user clicks "Start Queue" in the Queue page to begin processing. New `process_queue_manual` Tauri command for manual triggering.
- ✅ **Temp directory setting** - `temp_path` in Settings > Paths (default: `{OS temp}/MeedyaDL`). Resolves GAMDL's default `--temp-path` of `.` which is unwritable on macOS from `/Applications`.
- ✅ **Fix --cover-size parameter** - Was passing `"10000x10000"` (WxH) instead of `"10000"` (single integer) to GAMDL in both CLI args and config.ini
- ✅ **Expanded MusicKit documentation** - 6-step setup guide with detailed Apple Developer portal navigation, platform-specific instructions for extracting the `.p8` private key
- ✅ **Updates page** - Dedicated sidebar page (`Updates`) showing full release notes rendered as markdown via `react-markdown`. Strips "Choose your download" section from release bodies (irrelevant for in-app auto-update). Connected to update banner "View Details" link and sidebar footer update button. Shows "You're up to date" state with current version when no updates are available. External links in release notes and "View on GitHub" buttons open in the system default browser via `@tauri-apps/plugin-shell`.
- ✅ **i18n groundwork** - Translation infrastructure using `i18next` + `react-i18next` + `i18next-browser-languagedetector`. Translation files in `public/locales/{lang}/translation.json` (en, de, fr). `ui_language` setting in AppSettings (empty = auto-detect from OS). Language dropdown in Settings > General. Dynamic locale loading at startup. Provides migration path for translating remaining components.
- ✅ **Crash reporting system** - Three-layer diagnostics: local file logging (`tracing` ecosystem with daily-rotating log files), local JSON crash reports (`{app_data_dir}/crashes/`), and opt-in Sentry cloud reporting. Custom panic handler captures Rust panics; frontend errors (ErrorBoundary, window.onerror, unhandledrejection) persisted via `log_frontend_error` IPC command.
- ✅ **GitHub Issues crash reporting** - One-click crash reporting to GitHub Issues from Settings > Advanced > Crash Reporting. Pre-filled GitHub Issue URL opened in the user's browser (no tokens, no server needed). Privacy-first: user reviews all data in a `CrashReportDialog` consent modal before submitting. Backtrace truncated if body exceeds 3500 chars for URL length safety. New `crash-report` label and `.github/ISSUE_TEMPLATE/crash-report.yml` issue template. `build_github_issue_url()` in `crash_report_service.rs`, `get_github_issue_url` IPC command, `CrashReportSection` and `CrashReportDialog` frontend components.
- ✅ **GitHub branch protection** - Repository Ruleset on `main` preventing force pushes and branch deletion, requiring CI status checks for PRs
- ✅ **Pre-download internet connectivity check** - Non-blocking internet check before every download (`check_internet_before_download` Tauri command). When offline, the download is still queued but auto-start is skipped (`skip_auto_start` parameter on `start_download`); a warning toast is shown. Downloads wait in Queued state until the next online download triggers `process_queue()`. Cookie validation (`check_cookies_before_download`) runs only when online and is skipped for wrapper users.
- ✅ **Toast notification deduplication and auto-dismissal** - Prevents identical toast messages from stacking on screen (message-level dedup). Keyed toasts support category-based replacement and programmatic dismissal via `removeToastsByKey()`. Preflight warnings use keyed toasts with `preflight-cleared` events for auto-dismissal when checks pass (e.g., wrapper becomes reachable).
- ✅ **Auto-retry without wrapper** - `auto_retry_without_wrapper` setting (default: false). When enabled and a wrapper download fails terminally, the queue automatically re-queues the item with wrapper disabled (falls back to cookie-based auth). Toggle in Settings > Advanced > Wrapper section (only visible when Use Wrapper is enabled). Activity Log entry: "Wrapper failed — auto-retrying without wrapper".
- ✅ **Network error report suppression** - Terminal download failures with `error_category == "network"` no longer generate error reports, since network errors indicate connectivity issues rather than application bugs
- ✅ **GitHub Actions SHA pinning** - All release-critical GitHub Actions across 5 workflow files (CI, Release, Release Please, Changelog, Version Bump) pinned to immutable commit SHAs instead of mutable version tags, preventing supply chain attacks
- ✅ **SHA-256 checksum verification infrastructure** - `download_file()` in `archive.rs` computes SHA-256 hash during streaming download (zero extra I/O pass) using the `sha2` crate. New `download_and_extract_verified()` accepts an optional expected hash; mismatch deletes the temp file and returns an error
- ✅ **Graceful shutdown for background tasks** - `ShutdownSignal` (`Arc<AtomicBool>`) registered as Tauri managed state. Triggered on window close (`WindowEvent::Destroyed`) and tray quit. Checked at iteration boundaries in enrichment pipeline (9 stages), companion download loops (tiers), and lyrics companion loops (formats). Fire-and-forget tasks exit early instead of starting new work
- ✅ **Non-geographic Apple Music URL support** (v0.6.4) - URLs without a storefront code (e.g., `music.apple.com/album/...`) are automatically normalized by injecting a storefront based on OS locale (or "us" fallback). Enrichment API calls (`fetch_album_metadata`) retry with alternative storefronts when the primary returns 404 (handles cross-region shared links). Applied at enqueue time and queue import time.
- ✅ **Rich SRT subtitle generation** (v0.6.4) - Enrichment Step 2d converts TTML/WebVTT to format-rich SRT with HTML-like styling tags (`<b>`, `<i>`, `<u>`, `<font color>`). New `generate_rich_srt` setting (default: true). 51 unit tests.
- ✅ **ASS subtitle generation** (v0.6.4) - Enrichment Step 2f converts TTML/WebVTT to Advanced SubStation Alpha with BGR colours, override tags, positioning, and dedicated background vocals style. New `generate_ass` setting (default: false). 37 unit tests.
- ✅ **Subtitle embedding in MP4 containers** (v0.6.4) - Enrichment Step 2e embeds SRT and WebVTT sidecar content as freeform atoms (`subtitles-srt`, `subtitles-vtt`) in M4A/MP4/M4V files. New `embed_subtitles` setting (default: false).
- ✅ **Verbose activity log toggle** (v0.6.4) - New `verbose_activity_log` setting for detailed diagnostic output. Verbose messages prefixed with `[VERBOSE]`. Toggle in Settings > Advanced.
- ✅ **Comprehensive Apple Music API metadata** (v0.6.4) - All available fields from the Apple Music catalog API extracted and embedded: 9 new track-level fields (Digital Master, release date, composer, duration, has lyrics, play params, URL, preview URL, genres) and 11 new album-level fields (record label, copyright, release date, compilation/single/complete flags, MFiT, track count, editorial notes, UPC, content rating).
- ✅ **Dual-namespace metadata tagging** (v0.6.4) - All API-sourced tags written to both `com.apple.iTunes` (player-compatible) and `MeedyaMeta` (MeedyaDL-branded). Industry standard alternative names: `LABEL`, `COPYRIGHT`, `COMPILATION`, `TOTALTRACKS`. Album scope uses `Album*` prefix; track scope uses no prefix.
- ✅ **Config-driven tag system (tags.toml)** (v0.6.4) - 28 tag definitions (16 album + 14 track) in declarative TOML. Adding new tags = edit TOML only, zero Rust changes. JSON path walker supports dotted paths, `[N]` array indexing, nested objects. `tag_registry.rs` module. 25 unit tests.
- ✅ **API field audit tool** (v0.6.4) - Developer diagnostic in Settings > Metadata. Fetches album from Apple Music API, diffs JSON against tags.toml. Reports known/unknown/missing fields. 10 unit tests.
- ✅ **Dependabot automated dependency checks** (v0.6.4) - Weekly semver-compatible npm + Cargo dependency checks. Minor/patch grouped into single PRs per ecosystem.
- ✅ **Security fixes** (v0.6.4) - Cookie domain substring sanitization (Code Scanning #11), Secure attribute on cookies (#4-10), CI permissions block (#1-2).
- ✅ **Tailwind CSS v4 migration** - Tailwind CSS 3.4.17 → 4.2.2; `@tailwindcss/postcss` replaces `tailwindcss` as PostCSS plugin; `autoprefixer` removed (built into v4); `@tailwindcss/typography` via `@plugin` in CSS; `globals.css` migrated to `@import "tailwindcss"` + `@config` + `@plugin`; macOS minimum raised to 13.3 (Safari 16.4+ required); Vite targets updated to safari16.4/chrome111
- ✅ **Security hardening** - TAR extraction path traversal fix, HTTP client timeouts added to 4 previously-unbounded instances, wrapper URL redacted from GAMDL CLI args log, lz4_flex 0.11.5 → 0.11.6 (memory leak fix)
- ✅ **Crash report improvements** - `delete_all_crash_reports` command and "Clear All" UI button in Settings > Advanced > Crash Reporting
- ✅ **Monthly Dependency Report CI workflow** - Automated monthly dependency audit reporting
- ✅ **Frontend stability fixes** - Tooltip and CookiesTab setTimeout cleanup on unmount, flaky Windows test fix, npm audit vulnerability fixes (flatted, undici)
- ✅ **Comprehensive progress tracking** (#294) - Companion downloads stream stdout/stderr line-by-line with real-time speed/ETA/percentage in the global progress bar. Queue items stay in Processing state until all companion AND enrichment JoinHandles resolve. Processing labels display current activity ("Enriching metadata tags...", "Converting lyrics..."). Queue-level bar counts Processing items as done (partial credit). Desktop notifications deferred until all background work completes. ReplayGain/AcoustID emit per-file progress.
- ✅ **Engine registry** - `engines.toml` declarative engine registry compiled into binary. Defines platforms, engines, URL patterns, install methods, icons. Platform icons rendered inline (SVG with `currentColor`) for theme adaptability.
- ✅ **CodeQL CI fix** - Added `actions: read` permission to CodeQL workflow to resolve "Resource not accessible by integration" errors on release-please PR branches.
- ✅ **README corrections** - Fixed org URLs (`MeedyaDL/MeedyaDL` → `MWBMPartners/MeedyaDL`), removed screenshots placeholder, PNG logo with dark/light mode support.
- ✅ **Activity log memory optimization** (#370) - Capped at 10,000 entries with amortised trimming. Virtualized rendering via `@tanstack/react-virtual` (~150 DOM nodes vs 37,500). RAF-batched event ingestion collapses 200+/s Zustand updates to ~60/s. Backend `\r` segment coalescing reduces event volume 5-10x. Download store uses `map()` pattern. Stable `_id` on entries for efficient React reconciliation. Fixes 14+ GB WebView RAM bloat during long download sessions.
- ✅ **macOS in-app updater fix** (#368) - Corrected filename mismatch in `release.yml` upload step and `latest.json` rebuild: Tauri 2.x generates `MeedyaDL.app.tar.gz` (no arch suffix), not `MeedyaDL_aarch64.app.tar.gz`. Also fixed in standalone `fix-updater-manifest.yml`. `darwin-aarch64` platform now included in updater manifest.
- ✅ **cargo-deny org-level source allowlist** (#365) - `deny.toml` uses `[sources.allow-org] github = ["MWBMPartners", "MeedyaDL"]` to allow git dependencies from both GitHub orgs without per-repo URL entries.

---

## 🔮 Future Roadmap

### Overview

| Milestone | Version | Service | Backend Tool | Issue | Status |
| --- | --- | --- | --- | --- | --- |
| — | v2.0.0 | Multi-service architecture | — | [#107](https://github.com/MWBMPartners/MeedyaDL/issues/107) | 🔲 Prerequisite |
| M8 | v2.0.0 | Spotify | [votify](https://github.com/glomatico/votify) | [#101](https://github.com/MWBMPartners/MeedyaDL/issues/101) | 🔲 Planned |
| M9 | v2.1.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | [#104](https://github.com/MWBMPartners/MeedyaDL/issues/104) | 🔲 Planned |
| M10 | v2.2.0 | BBC iPlayer | [yt-dlp](https://github.com/yt-dlp/yt-dlp) / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | [#102](https://github.com/MWBMPartners/MeedyaDL/issues/102) | 🔲 Planned |
| v3.x | TBD | YouTube Music | [gytmdl](https://github.com/glomatico/gytmdl) | [#103](https://github.com/MWBMPartners/MeedyaDL/issues/103) | 🔮 Future |
| v3.x | TBD | Smart Download | Cross-platform | [#110](https://github.com/MWBMPartners/MeedyaDL/issues/110) | 🔮 Future |
| v3.x | TBD | Enhanced MusicKit | Server-side JWT | [#108](https://github.com/MWBMPartners/MeedyaDL/issues/108) | 🔮 Future |
| — | TBD | Full i18n | — | [#111](https://github.com/MWBMPartners/MeedyaDL/issues/111) | 🔮 Ongoing |
| — | TBD | Remote feature disable | — | [#106](https://github.com/MWBMPartners/MeedyaDL/issues/106) | 🔮 Future |
| — | TBD | Native SwiftUI (macOS) | — | [#109](https://github.com/MWBMPartners/MeedyaDL/issues/109) | 🔮 Future |
| — | TBD | Anonymous crash relay | PHP | [#44](https://github.com/MWBMPartners/MeedyaDL/issues/44) | 🔮 Future |

The architecture is designed with a `MediaService` trait pattern (`src-tauri/src/models/media_service.rs`) to support adding new platforms without restructuring the codebase. Each service follows the same subprocess pattern: a Python CLI tool installed via pip into the portable Python runtime.

---

### Milestone 8 — Spotify Support (v2.0.0) — [#101](https://github.com/MWBMPartners/MeedyaDL/issues/101)

**Status:** 🔲 Planned

Spotify integration via [votify](https://github.com/glomatico/votify), a Python CLI tool by the same developer as GAMDL. Follows the identical subprocess pattern (`python -m votify ...`), making it the natural first service to add.

#### Spotify Architecture Changes

- Add `Spotify` variant to `MediaServiceId` enum
- Update `url_domains()` to match `open.spotify.com`
- Update `pip_package()` to return `"votify"`
- Generalise download queue to route by `MediaServiceId` (currently hardcoded for GAMDL)

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

### Milestone 9 — YouTube Support (v2.1.0) — [#104](https://github.com/MWBMPartners/MeedyaDL/issues/104)

**Status:** 🔲 Planned

YouTube integration via [yt-dlp](https://github.com/yt-dlp/yt-dlp), the most widely-used media download tool. Supports YouTube videos, shorts, playlists, channels, and audio extraction. yt-dlp also serves as the shared backend for BBC iPlayer in Milestone 10.

#### YouTube Architecture Changes

- Add `YouTube` variant to `MediaServiceId` enum (or introduce a broader `MediaServiceId`)
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

### Milestone 10 — BBC iPlayer Support (v2.2.0) — [#102](https://github.com/MWBMPartners/MeedyaDL/issues/102)

**Status:** 🔲 Planned

BBC iPlayer integration for downloading TV programmes, films, and radio shows. Reuses yt-dlp from Milestone 9 (which already supports BBC iPlayer) or uses [get_iplayer](https://github.com/get-iplayer/get_iplayer) as a dedicated alternative.

**Important:** BBC iPlayer content is geographically restricted to the United Kingdom. Users outside the UK will need a VPN or BBC account with UK access.

#### BBC iPlayer Architecture Changes

- Add `BbcIPlayer` variant to `MediaServiceId` (or broader `MediaServiceId` if refactored in Milestone 9)
- Update `url_domains()` to match `bbc.co.uk/iplayer`, `bbc.co.uk/sounds`
- Extend content type detection for TV-specific models (series, episodes, categories)
- ~~Consider renaming `MusicService` trait to `MediaService`~~ Done in #314

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
- ✅ **Rename MusicService → MediaService** — reflect that BBC iPlayer and YouTube are not music-only services (#314)
- 🔲 **Shared dependency management** — yt-dlp used by both YouTube (M9) and BBC iPlayer (M10); install once, share across services
- 🔲 **Service-aware fallback chains** — each service defines its own quality fallback chain based on available codecs
- 🔲 **Help documentation** — add per-service help topics (e.g., `help/spotify.md`, `help/youtube.md`, `help/bbc-iplayer.md`)

---

### v3.x — Advanced Features

| Feature | Description | Status |
| --- | --- | --- |
| **Smart Download** | Cross-platform quality optimisation — search all enabled services for the same content and download the best available quality | 🔮 Future |
| **YouTube Music** | Dedicated YouTube Music support via [gytmdl](https://github.com/glomatico/gytmdl) for music-specific features (albums, playlists, lyrics) beyond what yt-dlp provides | 🔮 Future |
| **Full i18n** | Complete translations for German, French, and additional languages (groundwork done: i18next + react-i18next, OS auto-detection, English locale) | 🔮 Future |
| **Download history** | Persistent download history and statistics dashboard | 🔮 Future |

### Future (Beyond v3.x)

| Feature | Description | Status |
| --- | --- | --- |
| **Remote Service Status** | Developer-controlled kill switch to remotely enable/disable individual media services across all deployed app instances | 🔮 Future |
| **Integration API** | REST or IPC API for external apps to trigger downloads programmatically | 🔮 Future |
| **Custom themes** | User-defined accent colours and theme presets | 🔮 Future |
| **Multi-track muxing** | Mux companion downloads (e.g. Atmos + AC3 + AAC) into a single MP4 with multiple audio streams and alternate-group metadata for codec-based fallback | 🔮 Future |
| **Native SwiftUI UI for macOS** | Replace the web-based frontend on Apple Silicon with a fully native SwiftUI interface for tighter macOS integration and performance | 🔮 Future |
| **PHP relay endpoint for crash reporting** | Server-side relay that accepts anonymous crash submissions without requiring a GitHub account, creating GitHub Issues on the user's behalf (GitHub Issue [#44](https://github.com/MWBMPartners/MeedyaDL/issues/44)) | 🔮 Future |

---

## ⚠️ Known Issues / Blockers

None at this time.

---

## 📝 Notes

- **All CLI tools are called as subprocesses** (`python -m gamdl`, `python -m votify`, `yt-dlp`, etc.) to maintain license compatibility
- **All dependencies are self-contained** in the app data directory — no system-wide installations
- **Conventional commits** are used throughout for automated changelog generation
- **Every source file** includes copyright headers with automated year updates
- **yt-dlp is shared** between YouTube (M9) and BBC iPlayer (M10) — install once, configure per-service

---

*Last updated: 2026-03-19*

(c) 2024-2026 MeedyaDL
