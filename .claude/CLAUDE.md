# gamdl-GUI - Claude Code Project Context

## Project Overview

A multiplatform desktop GUI for [GAMDL](https://github.com/glomatico/gamdl) (Apple Music downloader) built with **Tauri 2.0 + React + TypeScript**. Targets macOS (Apple Silicon), Windows (x64/ARM64), Linux (x64/ARM64/ARMv7), and ChromeOS (via Linux `.deb`).

## Architecture

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS + Zustand state management
- **Backend**: Rust (Tauri 2.0) with IPC command handlers
- **GAMDL Integration**: CLI subprocess calls (`python -m gamdl ...`) - never imported as a Python library
- **Dependencies**: Self-contained in app data dir (Python via python-build-standalone, GAMDL via pip, tools)
- **Theming**: Platform-adaptive CSS custom properties (macOS/Windows/Linux themes)
- **Error Handling**: ErrorBoundary in main.tsx catches React crashes; unhandled rejection handler logs async errors

## Key Directories

```
src-tauri/src/          # Rust backend
  commands/             # IPC command handlers (system, dependencies, settings, gamdl, credentials, updates)
  models/               # Data structures (download, settings, gamdl_options, dependency, music_service)
  services/             # Business logic (python_manager, gamdl_service, dependency_manager, config_service, download_queue, update_checker)
  utils/                # Platform, archive, process utilities
src/                    # React frontend
  components/           # UI components (common, layout, download, settings, setup, help)
  hooks/                # React hooks (usePlatform, useTheme)
  stores/               # Zustand state stores (ui, settings, download, dependency, setup, update)
  lib/                  # Utilities (tauri-commands, url-parser, quality-chains)
  types/                # TypeScript type definitions mirroring Rust models
  styles/themes/        # Platform CSS (base, macos, windows, linux)
help/                   # Markdown help documentation (10 topics)
scripts/                # Build utilities (copyright year updater, version bump)
.github/workflows/      # CI, Release, Release Please, Changelog workflows
```

## Implementation Phases (All Complete)

- **Phase 1**: Project foundation - scaffold, config, CI/CD, docs
- **Phase 2**: Core backend - Python manager, GAMDL service, dependency manager, settings, credentials
- **Phase 3**: Core UI - Zustand stores, layout, download form, settings pages, setup wizard, help viewer
- **Phase 4**: Download system - Queue manager, fallback quality chain, progress tracking, retry/clear
- **Phase 5**: Advanced features - Cookie import, auto-updates, help search, system tray, service architecture
- **Phase 6**: Polish & release - Icons, CI fixes, testing, docs, release workflow, release-please integration

## Conventions

- **Copyright header**: Every source file starts with `// Copyright (c) 2024-2026 MWBM Partners Ltd` + MIT license reference
- **Comments**: Every function and significant code block gets detailed comments
- **Conventional commits**: Required for automated changelog generation (release-please)
- **GAMDL options**: All 11 audio codecs, 8 video resolutions, all CLI flags typed as Rust enums in `models/gamdl_options.rs`
- **Fallback quality chains**: Music: ALAC→Atmos→AC3→AacBinaural→Aac→AacLegacy; Video: 2160p→...→240p
- **Git operations**: Do NOT auto-commit or auto-push. Only edit files — let the user control git operations.

## Release Workflow

```text
Push fix:/feat: commits directly to main
  → release-please creates/updates a Release PR (bumps versions)
  → User reviews and merges the Release PR
  → release-please creates tag (e.g., v0.1.3) using RELEASE_PAT
  → release.yml triggers → 6 platform builds → draft GitHub Release
  → changelog.yml triggers → git-cliff regenerates CHANGELOG.md
```

Manual override: `version-bump.yml` + `scripts/bump-version.mjs` for non-standard releases.

## Build Targets

| Platform | Architecture | Format | Notes |
| -------- | ------------ | ------ | ----- |
| macOS | Apple Silicon (ARM64) | `.dmg`, `.app` | Needs `xattr -cr` for unsigned builds |
| Windows | x64 (64-bit) | `.exe` (NSIS) | Also works on ARM64 via emulation |
| Windows | ARM64 | `.exe` (NSIS) | Native ARM64 build |
| Linux | x64 | `.deb`, `.AppImage` | Also works on ChromeOS via Crostini |
| Linux | ARM64 | `.deb` | Experimental; Pi 4/5, ARM servers |
| Linux | ARMv7 | `.deb` | Experimental; Raspberry Pi 32-bit |

## Build Commands

```bash
npm run dev          # Start frontend dev server
npm run build        # Build frontend
npm run type-check   # TypeScript type checking
npm run test         # Run Vitest tests
cargo check          # Check Rust compilation (in src-tauri/)
cargo tauri dev      # Run full Tauri dev mode
cargo tauri build    # Build release binary
```

## Important Notes

- Rust env: `export PATH="$HOME/.cargo/bin:$PATH"` (not `source "$HOME/.cargo/env"` — fails in zsh sandbox)
- Icons generated from `assets/icons/app-icon.svg` via `scripts/generate-icons.mjs` (requires `sharp` — install temporarily with `npm i sharp`)
- All settings stored as JSON in platform app data directory
- GAMDL config.ini is synced from GUI settings
- CSP in `tauri.conf.json` must include `connect-src ipc: http://ipc.localhost` for IPC
- Vite build config uses `TAURI_ENV_PLATFORM` for platform-specific JS targets (safari13 / chrome105)
- `devtools` Cargo feature enabled for WebView inspection in release builds
