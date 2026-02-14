# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is maintained from [conventional commits](https://www.conventionalcommits.org/).

<!-- markdownlint-disable MD024 -->

## [0.3.3] - 2026-02-14

### 🐛 Bug Fixes

- Fix tool installation failures on macOS (frontend sent display names like "FFmpeg", backend expected IDs like "ffmpeg"; added `resolve_tool_id()` to accept either form)
- Fix mp4decrypt (Bento4) download 404 on macOS (URL suffix changed from `macosx` to `universal-apple-macosx`)
- Fix Linux ARM builds failing due to AppImage exec format error on x86_64 runners (skip AppImage, produce only `.deb` and `.rpm`)

### 🧹 Maintenance

- Mark all four external tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box) as required

## [0.3.2] - 2026-02-14

### 📚 Documentation

- Add Native SwiftUI UI for macOS as a future idea across project docs
- Document release-please state fix, Linux ARM cross-compilation apt fix, release workflow manual dispatch with tag input, Windows PowerShell shell fix

### 🧹 Maintenance

- Consolidate documentation and update version references

## [0.3.1] - 2026-02-14

### ✨ Features

- Add metadata tagging service for M4A files — inject custom codec metadata tags (ALAC: `isLossless=Y`, Atmos: `SpatialType=Dolby Atmos`) via mp4ameta freeform atoms
- Implement queue persistence and export/import functionality — auto-save to `queue.json` after every mutation; `.meedyadl` file format for cross-device transfer
- Enhance workflows with manual dispatch (`workflow_dispatch` on CI, Changelog, Release Please, Release)
- Update project documentation with planned service integrations and milestones for Spotify, YouTube, and BBC iPlayer
- Add multi-track muxing feature to project plan and README
- Implement hidden animated artwork files feature with OS-level hiding options
- Add companion download mode settings (4 modes: Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy)
- Add "Embed Lyrics and Keep Sidecar" toggle for dual lyrics output

### 🐛 Bug Fixes

- Update release-please branch reference to match actual branch naming
- Restrict default apt sources to amd64 for ARM cross-compilation (Ubuntu 24.04 default mirrors don't host ARM packages)
- Support manual dispatch in release workflow with tag input (fix `github.ref_name` resolving to branch name instead of tag)
- Use bash shell for tag resolution step on Windows runners (PowerShell can't parse bash syntax)

### 🧹 Maintenance

- Add `workflow_dispatch` to release workflow for re-running builds when tag push events are missed

## [0.1.4] - 2026-02-13

### ✨ Features

- Add browser cookie extraction service and auto-import functionality (detect installed browsers, extract Apple Music cookies automatically)
- Add embedded Apple Music login window service and UI integration (sign in directly within the app)
- Add support for fetching extra metadata tags and update cover size to max resolution
- Add animated artwork download service for Apple Music (MusicKit API, HLS streams via FFmpeg)

### 🐛 Bug Fixes

- Enhance error handling and improve cookie import feedback

## [0.1.3] - 2026-02-12

### 🐛 Bug Fixes

- Resolve blank screen on macOS/Windows release builds — fix React infinite re-render loop (error #185) caused by Zustand selector creating new array references, subscribing to function references instead of state, and subscribing to entire settings object
- Add CSP `connect-src` for Tauri IPC
- Add ErrorBoundary to main.tsx for visible crash diagnostics
- Add Vite build config (target, envPrefix) per Tauri 2.0 guide
- Enable devtools Cargo feature for WebView inspection
- Simplify Windows release: drop x86, produce only NSIS .exe (no .msi)

## [0.1.2] - 2026-02-12

### ✨ Features

- Integrate release-please for automated release PRs — git-cliff continues to own CHANGELOG.md, manual version-bump workflow preserved as override

### 📚 Documentation

- Add macOS Gatekeeper fix to release notes (`xattr -cr` to remove quarantine flag)

### 🧹 Maintenance

- Add auth/ to .gitignore to prevent secret leaks

## [0.1.1] - 2026-02-11

### ✨ Features

- Add release automation and expand to 7 platform targets (macOS ARM64, Windows x64/x86/ARM64, Linux x64/ARM64/ARMv7)

### 🐛 Bug Fixes

- Make usePlatform fallback test deterministic across CI runners (mock navigator.userAgent)

## [0.1.0] - 2026-02-11

### ✨ Features

- Initialize MeedyaDL application with Tauri 2.0 and React 19
- Add setup wizard components and state management (6-step first-run wizard)
- Add download form with Apple Music URL detection and quality selection
- Add settings pages (9 tabs: General, Quality, Fallback, Paths, Cookies, Lyrics, Cover Art, Templates, Advanced)
- Add help viewer with 11 topics, sidebar navigation, and search
- Implement platform-adaptive themes (macOS Liquid Glass, Windows Fluent, Linux Adwaita)
- Implement icon generation script, ESLint configuration, and Vitest setup
- Automate copyright year updates across all source files

### 🐛 Bug Fixes

- Resolve ESLint no-explicit-any error in Modal.test.tsx

---

(c) 2024-2026 MeedyaDL
