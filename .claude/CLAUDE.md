# gamdl-GUI - Claude Code Project Context

## Project Overview

A multiplatform desktop GUI for [GAMDL](https://github.com/glomatico/gamdl) (Apple Music downloader) built with **Tauri 2.0 + React + TypeScript**. Targets macOS (Apple Silicon), Windows (x86/x64/ARM), Linux, and Raspberry Pi.

## Architecture

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS + Zustand state management
- **Backend**: Rust (Tauri 2.0) with IPC command handlers
- **GAMDL Integration**: CLI subprocess calls (`python -m gamdl ...`) - never imported as a Python library
- **Dependencies**: Self-contained in app data dir (Python via python-build-standalone, GAMDL via pip, tools)
- **Theming**: Platform-adaptive CSS custom properties (macOS/Windows/Linux themes)

## Key Directories

```
src-tauri/src/          # Rust backend
  commands/             # IPC command handlers (system, dependencies, settings, gamdl, credentials, updates)
  models/               # Data structures (download, settings, gamdl_options, dependency, music_service)
  services/             # Business logic (python_manager, gamdl_service, dependency_manager, config_service, download_queue, update_checker)
  utils/                # Platform, archive, process utilities
src/                    # React frontend
  components/           # UI components (common, layout, download, settings, setup, help)
  hooks/                # React hooks (usePlatform)
  stores/               # Zustand state stores (ui, settings, download, dependency, setup, update)
  lib/                  # Utilities (tauri-commands, url-parser, quality-chains)
  types/                # TypeScript type definitions mirroring Rust models
  styles/themes/        # Platform CSS (base, macos, windows, linux)
help/                   # Markdown help documentation (10 topics)
scripts/                # Build utilities (copyright year updater)
.github/workflows/      # CI, Release, Changelog workflows
```

## Implementation Phases

- **Phase 1** (COMPLETE): Project foundation - scaffold, config, CI/CD, docs
- **Phase 2** (COMPLETE): Core backend - Python manager, GAMDL service, dependency manager, settings, credentials
- **Phase 3** (COMPLETE): Core UI - Zustand stores, layout, download form, settings pages, setup wizard, help viewer
- **Phase 4** (COMPLETE): Download system - Queue manager, fallback quality chain, progress tracking, retry/clear
- **Phase 5** (COMPLETE): Advanced features - Cookie import, auto-updates, help search, system tray, service architecture
- **Phase 6** (IN PROGRESS): Polish & release - Icons, CI fixes, testing, docs, release workflow

## Conventions

- **Copyright header**: Every source file starts with `// Copyright (c) 2024-2026 MWBM Partners Ltd` + MIT license reference
- **Comments**: Every function and significant code block gets detailed comments
- **Conventional commits**: Required for automated changelog generation
- **GAMDL options**: All 11 audio codecs, 8 video resolutions, all CLI flags typed as Rust enums in `models/gamdl_options.rs`
- **Fallback quality chains**: Music: ALAC→Atmos→AC3→AacBinaural→Aac→AacLegacy; Video: 2160p→...→240p

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

- Rust env may need: `source "$HOME/.cargo/env"`
- Icons generated from `assets/icons/app-icon.svg` via `scripts/generate-icons.mjs` (requires `sharp` — install temporarily with `npm i sharp`)
- All settings stored as JSON in platform app data directory
- GAMDL config.ini is synced from GUI settings
