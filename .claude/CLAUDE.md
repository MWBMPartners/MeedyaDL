# MeedyaDL - Claude Code Project Context

## Project Overview

A multiplatform media downloader desktop application built with **Tauri 2.0 + React + TypeScript**. Currently supports Apple Music via [GAMDL](https://github.com/glomatico/gamdl), with planned support for additional services. Targets macOS (Apple Silicon), Windows (x64/ARM64), Linux (x64/ARM64/ARMv7), and ChromeOS (via Linux `.deb`).

## Architecture

- **Frontend**: React 19 + TypeScript + Vite + Tailwind CSS + Zustand state management
- **Backend**: Rust (Tauri 2.0) with IPC command handlers
- **GAMDL Integration**: CLI subprocess calls (`python -m gamdl ...`) - never imported as a Python library
- **Dependencies**: Self-contained in app data dir (Python via python-build-standalone, GAMDL via pip, tools)
- **Theming**: Platform-adaptive CSS custom properties (macOS/Windows/Linux themes)
- **Error Handling**: ErrorBoundary in main.tsx catches React crashes; unhandled rejection handler logs async errors. All frontend errors are persisted to Rust crash reports via `log_frontend_error` IPC command.
- **Logging**: `tracing` ecosystem (replaces `env_logger`) with dual output: stderr + daily-rotating file in `{app_data_dir}/logs/`. Compatible with `log` facade -- all `log::*!()` macros work unchanged.
- **Error Reports**: JSON files in `{app_data_dir}/crashes/`. Captures Rust panics (`setup_panic_handler()`), frontend errors (IPC), and download errors (terminal failures after all fallbacks/retries exhausted). Auto-cleanup of reports older than 30 days. Users can report errors to GitHub Issues via `build_github_issue_url()` in `crash_report_service.rs` -- constructs a pre-filled issue URL with percent-encoded data, opened in the system browser. Title prefix and labels adapt to source type (`[Crash Report]` + `crash-report` label for panics/frontend errors; `[Error Report]` + `error-report` label for download errors). Privacy-first: `CrashReportDialog` shows a consent modal before submission. Backtrace truncated at 3500 chars for URL length safety. Issue template at `.github/ISSUE_TEMPLATE/crash-report.yml`.
- **Sentry** (opt-in): `sentry_enabled` setting (default: `false`). When enabled, Rust SDK + JS SDK send anonymous crash data. Toggle in Settings > Advanced. Exists as a parallel path alongside the GitHub Issues reporting.

## Key Directories

```
src-tauri/src/          # Rust backend
  commands/             # IPC command handlers (system, dependencies, settings, gamdl, credentials, updates, cookies, login_window, artwork, crash_reports [includes get_github_issue_url])
  models/               # Data structures (download, settings, gamdl_options, dependency, music_service, crash_report)
  services/             # Business logic (python_manager, gamdl_service, dependency_manager [4 required tools], config_service, download_queue, update_checker, cookie_service, login_window_service, animated_artwork_service, apple_music_api, metadata_tag_service, acoustid_service, replaygain_service, enhanced_lyrics_service, crash_report_service)
  utils/                # Platform, archive, process utilities
src/                    # React frontend
  components/           # UI components (common, layout, download, settings [includes CrashReportSection, CrashReportDialog], setup, help, updates)
  hooks/                # React hooks (usePlatform, useTheme)
  stores/               # Zustand state stores (ui, settings, download, dependency, setup, update)
  lib/                  # Utilities (tauri-commands, url-parser, quality-chains, i18n)
  types/                # TypeScript type definitions mirroring Rust models
  styles/themes/        # Platform CSS (base, macos, windows, linux)
public/locales/         # i18n translation files ({lang}/translation.json)
help/                   # Markdown help documentation (12 topics)
scripts/                # Build utilities (copyright year updater, version bump)
.github/ISSUE_TEMPLATE/ # Issue templates (crash-report.yml)
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
- **GAMDL options**: All 11 audio codecs, 8 video resolutions, all CLI flags typed as Rust enums in `models/gamdl_options.rs`. Includes `song_codec_priority` (for GAMDL >= 2.9.1 native multi-codec support) and `artist_auto_select` (7 variants: main-albums, compilation-albums, live-albums, singles-eps, all-albums, top-songs, music-videos).
- **Fallback quality chains**: Version-aware strategy with safety net. For GAMDL >= 2.9.1: first attempt uses native `--song-codec-priority` flag to pass the entire fallback chain (e.g., `atmos,alac,aac,...`) in a single process invocation. If native priority fails, MeedyaDL's own `try_fallback` system kicks in as a safety net, retrying each codec individually via `--song-codec-priority <single-codec>`. For GAMDL < 2.9.1: MeedyaDL's `try_fallback` is the only mechanism. `try_fallback()` overrides `song_codec_priority` to a single codec on each retry (prevents chain rebuild and overrides config.ini). Music chain: ALAC→Atmos→AC3→AacBinaural→Aac→AacLegacy; Video: 2160p→...→240p. Config.ini dual-key strategy writes both `song_codec` (for < 2.9.1) and `song_codec_priority` (for >= 2.9.1) to ensure compatibility across GAMDL versions.
- **Apple Music URL support**: Three domains accepted: `music.apple.com` (standard), `classical.apple.com` (Apple Music Classical), and `itunes.apple.com` (legacy). Validated in both the frontend URL parser (`src/lib/url-parser.ts`) and backend regex patterns in the Rust services.
- **Companion downloads**: Configurable via `CompanionMode` enum (Disabled / AtmosToLossless / AtmosToLosslessAndLossy / SpecialistToLossy) in `settings.rs`. Default: `AtmosToLossless` (Atmos → also download ALAC). The `plan_companions()` function in `download_queue.rs` returns a list of `CompanionTier` structs; each tier's codecs are tried in order. When companions exist, primary gets suffix (`[Dolby Atmos]` or `[Lossless]`); most universal companion uses clean filenames. Fire-and-forget background task (like animated artwork). Suffix system via `codec_suffix()`, `apply_codec_suffix()`, and `needs_primary_suffix()` in `download_queue.rs`.
- **Metadata enrichment**: After GAMDL writes standard tags, a unified background task runs 5 enrichment stages via `metadata_tag_service.rs`, `enhanced_lyrics_service.rs`, `acoustid_service.rs`, and `replaygain_service.rs`: (1) codec/source/channel tags + Apple Music API metadata (always-on), (2) Enhanced LRC conversion (opt-in, `enhanced_lrc`, default on), (3) animated artwork download (reuses API response), (4) AcousticID fingerprinting (opt-in, `acoustid_enabled`), (5) ReplayGain analysis (opt-in, `replaygain_enabled`). All use `mp4ameta` crate freeform atoms via `set_data()` + `write_to_path()` which preserves existing tags. Shared API logic in `apple_music_api.rs` (MusicKit JWT, URL parsing, catalog fetch). The `AlbumMetadata` struct is fetched once and shared across enrichment and artwork to avoid duplicate API calls.
- **Enhanced LRC**: When `enhanced_lrc` is enabled (default: true), `merge_options()` Layer 4 forces TTML as the primary lyrics format so GAMDL preserves Apple Music's word-level timing data. After download, `enhanced_lyrics_service.rs` parses the TTML via `roxmltree`, extracts `<span>` word timestamps from `itunes:timing="Word"` documents, and generates Enhanced LRC with inline `<mm:ss.xx>` word timing. The `.lrc` sidecar is saved and embedded in M4A/M4V via the `©lyr` atom. Songs without word-level timing gracefully fall back to standard line-level LRC.
- **Lyrics embed + sidecar**: When enabled in settings, `merge_options()` forces `no_synced_lyrics=false` and removes `"lyrics"` from `exclude_tags` to ensure both embedded lyrics and sidecar files are created.
- **Queue auto-start**: Controlled by `auto_start_queue` setting (default: `true`). When enabled, `process_queue()` is called immediately after enqueue in `start_download()`, `retry_download()`, and `import_queue()`. When disabled, items stay in `Queued` state until the user clicks "Start Queue" in the Queue page (calls `process_queue_manual` command). Startup queue recovery always runs regardless of this setting.
- **Temp directory**: `temp_path` setting (default: empty string, resolved to `{OS temp dir}/MeedyaDL`). Resolved at runtime in both `merge_options()` and `settings_to_ini()` via `std::env::temp_dir().join("MeedyaDL")`. Avoids GAMDL's default of `.` which is unwritable on macOS from `/Applications`. Configurable in Settings > Tools.
- **Queue persistence**: The download queue is saved to `{app_data_dir}/queue.json` after every mutation (enqueue, cancel, retry, clear, completion, error, fallback). On startup, `load_queue_from_disk()` restores items and `process_queue()` is called after a 2-second delay (to let frontend event listeners initialise). Active items (Queued/Downloading/Processing) and failed items (Error) are persisted; only Completed and Cancelled items are cleared on restart. Failed items are restored in their Error state with the original error message so the user can review the failure and manually retry — they are not auto-retried. `PersistedQueueItem` includes an optional `error` field (`#[serde(default)]` for backwards compatibility with older `queue.json` files). Uses clone-then-release pattern: clone persistable items from lock, release lock, then write to disk. **Important**: Queue restoration in `setup_queue_recovery()` uses `blocking_lock()` (not `block_on(lock().await)`) and `tauri::async_runtime::spawn` (not `tokio::spawn`) because the Tokio runtime may not be registered as "current" during the `setup` closure on macOS.
- **Queue export/import**: Export via `export_queue` command opens native save dialog with `.meedyadl` filter; writes `QueueExportFile` JSON (version, app, exported_at, items). Import via `import_queue` opens native file picker, validates schema version == 1, re-enqueues items with fresh settings merge. Exported items contain only URLs + per-download overrides (not merged options), so the importing device uses its own settings as base.
- **Hidden animated artwork**: After downloading FrontCover.mp4/PortraitCover.mp4, files are hidden via OS-native mechanisms if `hide_animated_artwork` is `true` (default). macOS: `chflags hidden` (preserves filename); Windows: `attrib +H` (preserves filename); Linux: `.` prefix rename (changes filename). Logic in `animated_artwork_service::hide_file()`, called from the artwork background task in `download_queue.rs`.
- **Dependency manager**: 4 required external tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box). `tool-versions.toml` defines minimum supported versions per tool and mirror config (compiled into binary via `include_str!()`). Download resolution follows a three-tier fallback chain: **System PATH** → **Primary upstream** → **Mirror repo** (`MWBMPartners/meedyadl-tools`) → Error with guidance. `resolve_github_release_asset()` is the generic GitHub API resolver used by both upstream (N_m3u8DL-RE) and mirror queries. `install_tool()` first checks system PATH for a compatible version; if found, copies it to the managed tools dir and writes a `.source` marker file ("system"); otherwise downloads (with mirror fallback) and writes "managed". `DependencyStatus.source` field (Rust + TypeScript) drives the "System" badge in the setup wizard. MP4Box uses platform-specific installers (macOS Homebrew/`.pkg`, Windows NSIS `.exe`, Linux `.deb`) with mirror fallback via `install_mp4box_with_fallback()`. Mirror assets use standardized naming: `{tool_id}-{os}-{arch}.{ext}`. AcousticID fingerprinting uses the embedded `rusty-chromaprint` library (pure Rust) — no external fpcalc binary needed.
- **Updates page**: Dedicated sidebar page (`src/components/updates/UpdatesPage.tsx`) showing full release notes rendered via `react-markdown`. Strips the "Choose your download" section from release bodies since the in-app updater handles downloads. Connected to the update banner's "View Details" link and the sidebar footer's update button. External links in rendered release notes use a custom `ReactMarkdown` `components.a` handler that intercepts HTTP(S) clicks and opens them in the system default browser via `@tauri-apps/plugin-shell` (same pattern as `HelpViewer.tsx`). The "View on GitHub" buttons in both `UpdatesPage.tsx` and `UpdateBanner.tsx` also use the shell plugin instead of `window.open()`.
- **i18n groundwork**: Uses `i18next` + `react-i18next` + `i18next-browser-languagedetector`. Translation files in `public/locales/{lang}/translation.json` (currently en, de, fr). Language setting `ui_language` in AppSettings (empty = auto-detect from OS). Language dropdown in Settings > General (Appearance section). `initI18n()` called during app startup in App.tsx. To translate a component: `const { t } = useTranslation()` → `t('key')`. To add a language: create locale JSON, add to `AVAILABLE_LOCALES` in `i18n.ts` and `UI_LANGUAGE_OPTIONS` in `GeneralTab.tsx`.
- **Git operations**: Do NOT auto-commit or auto-push. Only edit files — let the user control git operations.
- **Documentation maintenance**: When adding features, modifying settings, changing commands/services, or altering UI — update ALL affected markdown files (README.md, Project_Plan.md, CHANGELOG.md, CLAUDE.md, help/*.md). This includes version numbers, file counts, feature lists, project structure trees, and help topic cross-references. Project_Plan.md serves as both the plan and status tracker (PROJECT_STATUS.md was consolidated into it).

## Release Workflow

```text
Push fix:/feat: commits directly to main
  → release-please creates/updates a Release PR (bumps versions)
  → User reviews and merges the Release PR
  → release-please creates tag (e.g., v0.3.0) using RELEASE_PAT
  → release.yml triggers → 6 platform builds → draft GitHub Release
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
| M8 | v2.0.0 | Spotify | [votify](https://github.com/glomatico/votify) | pip install, subprocess calls like GAMDL; adds Ogg Vorbis codec support |
| M9 | v2.1.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | pip install, shared with BBC iPlayer; video-first service with format selection |
| M10 | v2.2.0 | BBC iPlayer | yt-dlp / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | Reuses yt-dlp from M9; region-restricted (UK VPN may be required) |

Architectural changes planned across milestones:
- **Rename `MusicService` → `MediaService`** (trait, enum, types) since BBC iPlayer and YouTube aren't music-only
- **Service-aware URL parser** that detects which service a URL belongs to and routes to the correct engine
- **Per-service settings tabs** in the Settings page (separate credentials, quality, paths per service)
- **Shared dependency management** — yt-dlp installed once, shared by YouTube and BBC iPlayer

### Future Ideas

- **Native SwiftUI UI for macOS** — replace the web-based Tauri frontend on Apple Silicon with a fully native SwiftUI interface for tighter macOS integration and performance (no target version)
- **Enhanced Apple Music (MusicKit) Integration** — server-side token generation via MeedyaDL API (Cloudflare Workers initially, internal API long-term), removing the requirement for users to provide their own Apple Developer credentials. Apple DPLA Sections 2.1/2.8 prohibit embedding `.p8` private keys in distributed apps. Target: v2.x/v3.x. See `Dev_Notes.md` for architecture details.

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
- **macOS codesign `--timestamp` workaround**: Tauri's `tauri-macos-sign` crate omits `--timestamp` from `codesign` calls, causing non-deterministic notarization failures ([tauri#11992](https://github.com/tauri-apps/tauri/issues/11992)). Both `release.yml` and `pre-release.yml` include a PATH-based wrapper script (Step 8.9) that intercepts `/usr/bin/codesign` and injects `--timestamp` automatically. Remove the wrapper once the upstream Tauri fix lands.
