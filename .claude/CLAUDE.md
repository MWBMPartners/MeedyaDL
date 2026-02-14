# MeedyaDL - Claude Code Project Context

## Project Overview

A multiplatform media downloader desktop application built with **Tauri 2.0 + React + TypeScript**. Currently supports Apple Music via [GAMDL](https://github.com/glomatico/gamdl), with planned support for additional services. Targets macOS (Apple Silicon), Windows (x64/ARM64), Linux (x64/ARM64/ARMv7), and ChromeOS (via Linux `.deb`).

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
  commands/             # IPC command handlers (system, dependencies, settings, gamdl, credentials, updates, cookies, login_window, artwork)
  models/               # Data structures (download, settings, gamdl_options, dependency, music_service)
  services/             # Business logic (python_manager, gamdl_service, dependency_manager [all 4 tools required], config_service, download_queue, update_checker, cookie_service, login_window_service, animated_artwork_service, metadata_tag_service)
  utils/                # Platform, archive, process utilities
src/                    # React frontend
  components/           # UI components (common, layout, download, settings, setup, help)
  hooks/                # React hooks (usePlatform, useTheme)
  stores/               # Zustand state stores (ui, settings, download, dependency, setup, update)
  lib/                  # Utilities (tauri-commands, url-parser, quality-chains)
  types/                # TypeScript type definitions mirroring Rust models
  styles/themes/        # Platform CSS (base, macos, windows, linux)
help/                   # Markdown help documentation (11 topics)
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

- **Copyright header**: Every source file starts with `// Copyright (c) 2024-2026 MeedyaDL` + MIT license reference
- **Comments**: Every function and significant code block gets detailed comments
- **Conventional commits**: Required for automated changelog generation (release-please)
- **GAMDL options**: All 11 audio codecs, 8 video resolutions, all CLI flags typed as Rust enums in `models/gamdl_options.rs`
- **Fallback quality chains**: Music: ALAC→Atmos→AC3→AacBinaural→Aac→AacLegacy; Video: 2160p→...→240p
- **Companion downloads**: Configurable via `CompanionMode` enum (Disabled / AtmosToLossless / AtmosToLosslessAndLossy / SpecialistToLossy) in `settings.rs`. Default: `AtmosToLossless` (Atmos → also download ALAC). The `plan_companions()` function in `download_queue.rs` returns a list of `CompanionTier` structs; each tier's codecs are tried in order. When companions exist, primary gets suffix (`[Dolby Atmos]` or `[Lossless]`); most universal companion uses clean filenames. Fire-and-forget background task (like animated artwork). Suffix system via `codec_suffix()`, `apply_codec_suffix()`, and `needs_primary_suffix()` in `download_queue.rs`.
- **Custom metadata tagging**: After GAMDL writes standard tags, `metadata_tag_service.rs` injects freeform MP4 atoms via `mp4ameta` crate. ALAC → `isLossless=Y` (iTunes namespace); Atmos → `SpatialType=Dolby Atmos` (both iTunes and MeedyaMeta namespaces). Called in both the primary download success path and companion download success path.
- **Lyrics embed + sidecar**: When enabled in settings, `merge_options()` forces `no_synced_lyrics=false` and removes `"lyrics"` from `exclude_tags` to ensure both embedded lyrics and sidecar files are created.
- **Queue persistence**: The download queue is saved to `{app_data_dir}/queue.json` after every mutation (enqueue, cancel, retry, clear, completion, error, fallback). On startup, `load_queue_from_disk()` restores items and `process_queue()` is called after a 2-second delay (to let frontend event listeners initialise). Only non-terminal items (Queued/Downloading/Processing) are persisted; terminal items are cleared on restart. Uses clone-then-release pattern: clone persistable items from lock, release lock, then write to disk.
- **Queue export/import**: Export via `export_queue` command opens native save dialog with `.meedyadl` filter; writes `QueueExportFile` JSON (version, app, exported_at, items). Import via `import_queue` opens native file picker, validates schema version == 1, re-enqueues items with fresh settings merge. Exported items contain only URLs + per-download overrides (not merged options), so the importing device uses its own settings as base.
- **Hidden animated artwork**: After downloading FrontCover.mp4/PortraitCover.mp4, files are hidden via OS-native mechanisms if `hide_animated_artwork` is `true` (default). macOS: `chflags hidden` (preserves filename); Windows: `attrib +H` (preserves filename); Linux: `.` prefix rename (changes filename). Logic in `animated_artwork_service::hide_file()`, called from the artwork background task in `download_queue.rs`.
- **Git operations**: Do NOT auto-commit or auto-push. Only edit files — let the user control git operations.
- **Documentation maintenance**: When adding features, modifying settings, changing commands/services, or altering UI — update ALL affected markdown files (README.md, PROJECT_STATUS.md, Project_Plan.md, CLAUDE.md, help/*.md). This includes version numbers, file counts, feature lists, project structure trees, and help topic cross-references.

## Release Workflow

```text
Push fix:/feat: commits directly to main
  → release-please creates/updates a Release PR (bumps versions)
  → User reviews and merges the Release PR
  → release-please creates tag (e.g., v0.3.0) using RELEASE_PAT
  → release.yml triggers → 4 Tier 1 + 2 Tier 2 platform builds → draft GitHub Release
  → changelog.yml triggers → git-cliff regenerates CHANGELOG.md
```

Manual override: `version-bump.yml` + `scripts/bump-version.mjs` for non-standard releases.

### Conserving GitHub Actions Minutes

All workflows (CI, Changelog, Release Please, Release) support both automatic (`on: push`) and manual (`workflow_dispatch`) triggers.

During rapid development, add `[skip ci]` to commit messages to prevent auto-triggering:

```bash
git commit -m "feat: add queue persistence [skip ci]"
```

When ready to validate, manually trigger via CLI or GitHub UI:

```bash
gh workflow run "CI" --ref main
gh workflow run "Release Please" --ref main
gh workflow run "Changelog" --ref main
gh workflow run "Release" -f tag=v0.3.3  # Release requires a tag input
```

### Release Please Branch Naming

Release-please v4 creates PR branches with the format:
`release-please--branches--{target}--components--{component}`

For this project (component name from `package.json` `name` field):
`release-please--branches--main--components--meedyadl`

The `.release-please-manifest.json` must match the current version to avoid release-please trying to create releases from an old version.

## Planned Service Integrations

| Milestone | Version | Service | Engine | Key Notes |
|-----------|---------|---------|--------|-----------|
| M7 | v0.4.0 | Spotify | [votify](https://github.com/glomatico/votify) | pip install, subprocess calls like GAMDL; adds Ogg Vorbis codec support |
| M8 | v0.5.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | pip install, shared with BBC iPlayer; video-first service with format selection |
| M9 | v0.6.0 | BBC iPlayer | yt-dlp / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | Reuses yt-dlp from M8; region-restricted (UK VPN may be required) |

Architectural changes planned across milestones:
- **Rename `MusicService` → `MediaService`** (trait, enum, types) since BBC iPlayer and YouTube aren't music-only
- **Service-aware URL parser** that detects which service a URL belongs to and routes to the correct engine
- **Per-service settings tabs** in the Settings page (separate credentials, quality, paths per service)
- **Shared dependency management** — yt-dlp installed once, shared by YouTube and BBC iPlayer

### Future Ideas

- **Native SwiftUI UI for macOS** — replace the web-based Tauri frontend on Apple Silicon with a fully native SwiftUI interface for tighter macOS integration and performance (no target version)

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
