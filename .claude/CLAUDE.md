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
  commands/             # IPC command handlers (system, dependencies, settings, gamdl, credentials)
  models/               # Data structures (download, settings, gamdl_options, dependency)
  services/             # Business logic (placeholder for Phase 2+)
  utils/                # Platform, archive, process utilities
src/                    # React frontend
  components/           # UI components (placeholder for Phase 3+)
  hooks/                # React hooks (usePlatform)
  styles/themes/        # Platform CSS (base, macos, windows, linux)
help/                   # Markdown help documentation (10 topics)
scripts/                # Build utilities (copyright year updater)
.github/workflows/      # CI, Release, Changelog workflows
```

## Implementation Phases

- **Phase 1** (COMPLETE): Project foundation - scaffold, config, CI/CD, docs
- **Phase 2** (NEXT): Core backend - Python manager, GAMDL service, dependency manager, settings, credentials
- **Phase 3**: Core UI - Zustand stores, layout, download form, settings pages, setup wizard
- **Phase 4**: Download system - Queue, fallback quality, progress tracking
- **Phase 5**: Advanced features - Cookie import, auto-updates, help system, tray
- **Phase 6**: Polish & release - Icons, testing, docs, release workflow

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
- Icons in `src-tauri/icons/` are placeholder RGBA PNGs - replace with real SVG-derived icons in Phase 6
- All settings stored as JSON in platform app data directory
- GAMDL config.ini is synced from GUI settings
