# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### ✨ Features

- Config-driven platform icons in progress bar with favicon fallback

Replaces the hardcoded Apple Music inline SVG with a data-driven
  platform icon system:

  1. engines.toml: Added `icon` field to each platform pointing to
     local SVG/PNG in public/icons/platforms/. Documentation explains
     how to add icons for new platforms.

  2. GlobalProgressBar.tsx: PLATFORM_CONFIG array maps URL hostnames to
     platform IDs, icon paths, and favicon fallback hosts. detectPlatform()
     uses hostname matching. PlatformIcon component loads the local SVG
     first, falls back to Google Favicon API (returns PNG) on error.

  3. Platform icon assets: apple-music.svg and spotify.svg added to
     public/icons/platforms/. Other platforms will use favicon fallback
     until custom icons are created.

  To add a new platform icon: save a 16x16 SVG/PNG to
  public/icons/platforms/{id}.svg and set the path in engines.toml.

- Theme-adaptive platform icons using currentColor + inline SVG rendering

Platform SVG icons now use fill="currentColor" instead of hardcoded
  colours. PlatformIcon component fetches the SVG and renders it inline
  (not as <img>) so currentColor inherits from the parent CSS context,
  automatically adapting to light, dark, and colour-blind themes.

  SVG content is cached in a module-level Map to avoid re-fetching.
  Fallback: Google Favicon API (PNG) when local SVG unavailable.

  Updated apple-music.svg and spotify.svg to use currentColor.
  Added platform icon documentation to DEV_NOTES.md covering the
  theme adaptability approach, fallback chain, SVG template, and
  step-by-step guide for adding new platform icons.

- Add BBC Sounds platform icon and path-based platform detection

Adds bbc-sounds.svg (headphones icon, currentColor for theme
  adaptability). Platform detection now supports pathContains for
  disambiguating services on the same host (e.g., bbc.co.uk/sounds
  vs bbc.co.uk/iplayer).


### 🐛 Bug Fixes

- Move CORE_COMPONENTS to module scope to resolve ESLint exhaustive-deps warnings

Also adds BBC Sounds as a separate platform in engines.toml with its
  own icon path, sharing the same engine priority as BBC iPlayer
  (get_iplayer primary, yt-dlp fallback).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.20.0] - 2026-03-29

### ✨ Features

- Auto-update checking for all enabled pip engines (#272)

Extends the update checker to monitor all pip-based engines defined
  in engines.toml, not just GAMDL. On each update check, parses
  engines.toml for enabled engines with install_method="pip", queries
  PyPI for the latest version, and compares against installed version.

  New components:
  - pip_engine_service::check_latest_pypi_version() — PyPI JSON API query
  - update_checker::get_enabled_pip_engines() — engines.toml parser
  - update_checker::check_pip_engine_update() — per-engine update check
  - commands::updates::upgrade_pip_engine() — generic pip upgrade IPC
  - upgradePipEngine() TypeScript binding

  Currently checks: votify (enabled=true). yt-dlp, get_iplayer, and
  OF-Scraper are disabled in engines.toml and skipped automatically.
  GAMDL retains its own check with compatibility gating.

- Aggregate engine updates into generic UI message, hide individual names

Engine/component updates (votify, yt-dlp, etc.) are now shown as a
  single "Component updates available" card instead of individual rows
  with version details. This avoids revealing specific tool names to
  end users and keeps the UI simple.

  - UpdatesPage: core updates (MeedyaDL, GAMDL, Python) shown with full
    detail; engine updates aggregated into one card with "Update All"
  - UpdateBanner: engine updates shown as "Component updates are also
    available" with a link to the Updates page
  - No changelog/release body shown for engine updates (already None)


### 🐛 Bug Fixes

- Use /releases/tags/ for deterministic mirror asset resolution

GitHub's /releases/latest endpoint returns the "most recently created"
  release, which differs from the release explicitly tagged "latest" when
  a repo has multiple releases. This caused MediaInfo macOS assets to not
  be found — they existed on the 'latest' tagged release but the API was
  returning a different release (2026-03-27) that lacked macOS assets.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Add SVG logo+logotype to README, update roadmap and architecture

- Logo: Added animated SVG logo (logo.svg) and logotype (logotype.svg)
    to the README header for crisp rendering at any resolution
  - Roadmap: Added M11 (OnlyFans/OF-Scraper), engine priority system
    (#268), smart re-download detection, MediaInfo, stable rollback (#267)
  - Architecture: Updated diagram to show engine registry layer and all
    5 download engines (GAMDL, votify, yt-dlp, get_iplayer, OF-Scraper)
  - Credits: Added votify, yt-dlp, get_iplayer, OF-Scraper, MediaInfo
  - Setup: Updated first-run to mention 5 required tools + MediaInfo
  - engines.toml: Added required/enabled fields per engine and platform

- Update CHANGELOG.md [skip ci]
- Update in-app help with MediaInfo, votify, smart re-download detection

- index.md: Updated project description to mention multi-service plans
  - getting-started.md: Added votify and MediaInfo to dependency lists
  - downloading-music.md: Added smart re-download detection section
  - faq.md: Added smart re-download detection Q&A entry

- Update CHANGELOG.md [skip ci]
- Remove OnlyFans/OF-Scraper references from all public documentation

OnlyFans support remains as an internal/private roadmap item but should
  not appear in public-facing documentation due to the platform's
  controversial nature.

  Removed from: README.md (roadmap, architecture, credits), DEV_NOTES.md
  (engine table), and code comments (dependencies.rs, tauri-commands.ts).

  The engines.toml entry and Rust/TypeScript code remain (compiled into
  binary, hidden when enabled=false) for infrastructure readiness.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.19.0] - 2026-03-29

### ✨ Features

- Add engines.toml for per-platform engine priority registry (#268)

New config-driven registry defining available download engines and
  their per-platform priority ordering. Follows the same pattern as
  codecs.toml and tags.toml — compiled into binary via include_str!,
  editable without code changes.

  Defines 5 engines (GAMDL, votify, yt-dlp, get_iplayer, OF-Scraper)
  and 6 platforms (Apple Music, Spotify, YouTube, YouTube Music,
  BBC iPlayer, OnlyFans). BBC iPlayer uses get_iplayer as primary
  with yt-dlp as fallback.

  Runtime parsing and Rust model will be implemented as part of #107
  (multi-service architecture).

- Embed Votify and OF-Scraper as pip engines with required/enabled flags (#268)

Adds engine lifecycle management for pip-based download engines:

  1. engines.toml: Added `required` and `enabled` fields to both engines
     and platforms. Votify is required+enabled, OF-Scraper is optional+
     disabled (hidden until OnlyFans support is implemented). yt-dlp and
     get_iplayer are also defined but disabled.

  2. pip_engine_service.rs: Generic service for install/version-check/
     uninstall of any pip package. Generalises the gamdl_service pattern
     so new engines need zero new Rust service code.

  3. IPC commands: check_votify_status, install_votify, check_ofscraper_status,
     install_ofscraper — registered in lib.rs with TypeScript bindings.

  4. Frontend: checkVotifyStatus(), installVotify(), checkOfscraperStatus(),
     installOfscraper() in tauri-commands.ts.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Add meedyadl-v2 branch archive section to DEV_NOTES.md

Documents the closed PR #24 (meedyadl-v2 branch), mapping each v2
  feature to its status on main (reimplemented, superseded, or tracked
  as open issue). Includes recommendations for future multi-service work:
  use fresh feature branches, adapt v2 patterns don't copy code, and
  use mirror-based tool management instead of bundled deps.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add engines.toml editing guide to DEV_NOTES.md (#268, #270)

Documents the engine registry file structure, priority system, and
  step-by-step guides for adding engines, adding platforms, changing
  priority, and removing engines. Includes current registry table and
  implementation status tracking.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.18.1] - 2026-03-27

### 🐛 Bug Fixes

- Mark MediaInfo as required tool for automatic installation

MediaInfo was marked as optional (required: false) so it was skipped
  during setup wizard and "Check All". Since MeedyaDL actively uses
  MediaInfo for codec detection in the enrichment pipeline, it should
  be auto-installed alongside FFmpeg, mp4decrypt, N_m3u8DL-RE, and
  MP4Box. Now 5 required tools instead of 4.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.18.0] - 2026-03-27

### ✨ Features

- Smart re-download detection via Apple Music API lastModifiedDate (#263)

Detects whether an album has changed since the user's last download
  by comparing the Apple Music API's lastModifiedDate timestamp against
  the value stored in the .meedyadl manifest.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Add smart re-download detection to in-app help (#263)

- downloading-music.md: new "Smart Re-Download Detection" section
    covering feature overview, settings toggle, detectable changes,
    and limitations
  - faq.md: new Q&A entry cross-referencing the full help section

- Update CLAUDE.md with smart re-download detection and recent fixes
- Update CHANGELOG.md [skip ci]
- Add smart re-download detection section to DEV_NOTES.md (#263)

Documents the full implementation: API field extraction, manifest
  storage, tag embedding, IPC command, frontend integration, detectable
  vs non-detectable changes, and key files reference.

- Update CHANGELOG.md [skip ci]

## [0.17.1] - 2026-03-27

### 🐛 Bug Fixes

- Remove hardcoded mp4decrypt version URL, use MeedyaDL-Tools mirror

mp4decrypt (Bento4) was pinned to version 1.6.0-641 via a hardcoded
  bok.net URL. Bento4 has no GitHub Releases API or "latest" tag, so
  the URL would go stale on future updates.

  Changed to mirror-only distribution (same approach as MediaInfo and
  MP4Box). The MeedyaDL-Tools mirror already has mp4decrypt assets for
  all 3 platforms (macos-aarch64, linux-x86_64, windows-x86_64).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.17.0] - 2026-03-27

### ✨ Features

- Add per-track separators and enhanced download headers in activity log

Improves activity log readability with three changes:

  1. Download start separator now includes codec and auth method:
     "Starting download: {URL}" + "Codec: atmos | Auth: wrapper"

  2. Per-track markers emitted when GAMDL starts each track:
     "──── Track 1/28: The Virginia Company ────"
     These appear as internal (accent-coloured) lines between the
     noisy [download] HLS fragment progress, making it easy to
     identify which track's progress lines belong to which song.

  3. Companion and enrichment phase separators:
     "──── Companion downloads (mode: Custom) ────"
     "──── Enrichment starting (lrc: on, artwork: on, ...) ────"


### 🐛 Bug Fixes

- MediaInfo install via MeedyaDL-Tools mirror instead of upstream DMG

The upstream macOS MediaInfo download is a .dmg containing a .pkg
  installer, which our archive module cannot extract. Changed the
  primary URL resolver to always fall through to the MeedyaDL-Tools
  mirror, which hosts repackaged CLI binaries as tar.gz/zip.

  Mirror assets uploaded:
  - mediainfo-macos-aarch64.tar.gz (universal binary, arm64+x86_64)
  - mediainfo-macos-x86_64.tar.gz (same universal binary)
  - mediainfo-windows-x86_64.zip (MediaInfo.exe + LIBCURL.DLL)
  - mediainfo-windows-aarch64.zip (MediaInfo.exe + LIBCURL.DLL)
  - mediainfo-linux-x86_64.tar.gz (static CLI binary)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.16.2] - 2026-03-27

### 🐛 Bug Fixes

- Activity log file logging, binaural/downmix companion tags, progress bar UX

Four fixes:

  1. Activity log entries (emit_download_log, emit_app_log) now also write
     to the tracing file log via log::info!. Previously they only emitted
     Tauri events to the frontend, making enrichment progress invisible in
     the on-disk log file when the UI was unresponsive.

  2. Companion downloads (apply_codec_metadata_tags) now clear inherited
     isBinaural/isDownmix tags for all codecs that aren't binaural/downmix.
     GAMDL's --fetch-extra-tags writes these from Apple Music API audioTraits
     regardless of the actual downloaded codec. Previously only the primary
     enrichment pipeline cleared them; companion files retained stale tags.

  3. Queue-level progress bar now includes error and cancelled items in
     both the total and completed counts, preventing it from appearing
     stuck at 0% for single-item queues.

  4. Progress bar text increased from 10px to 12px and bar height from
     4px to 6px for better readability.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.16.1] - 2026-03-26

### 🐛 Bug Fixes

- Run lyrics conversion (TTML → LRC/SRT/VTT/ASS) on companion downloads

Companion downloads inherited TTML as the lyrics format (forced by
  Enhanced LRC), but the enrichment pipeline only ran for the primary
  download. This left TTML sidecars unconverted for companion tiers.

  Adds run_companion_lyrics_conversion() which runs after each successful
  companion tier: Enhanced LRC, Rich SRT, WebVTT, and ASS generation —
  matching the same conversion steps as the primary enrichment pipeline.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.16.0] - 2026-03-26

### ✨ Features

- Aggregate release notes across multi-version jumps in update checker [skip ci]

When a user jumps multiple versions (e.g., v0.13.0 → v0.15.0), the Updates
  page now shows combined release notes from all intermediate versions, not
  just the latest. Fetches up to 20 releases from GitHub API and filters to
  those newer than current_version. Bodies are concatenated newest-first with
  horizontal rule separators.

  Also adds Animated Cover Art developer documentation to DEV_NOTES.md.


### 🐛 Bug Fixes

- Use URL hostname parsing instead of substring matching in detectPlatform [skip ci]

Resolves CodeQL alerts #13, #14, #15 (Incomplete URL substring sanitization).
  `url.includes('music.apple.com')` could match crafted hostnames like
  `evil-music.apple.com.attacker.com`. Now uses `new URL().hostname` for
  exact hostname comparison.

- Bundle English translations inline to prevent raw i18n keys on first render [skip ci]

The sidebar was briefly showing raw keys like "sidebar.ready" and
  "sidebar.checkForUpdates" because i18n resources were loaded via async
  fetch() inside a useEffect, which completes after the first render.

- Detect actual codec before planning companion downloads with native priority [skip ci]

When native priority is used (--song-codec-priority atmos,alac,aac,...),
  GAMDL may silently fall back to ALAC when Atmos is unavailable. Previously,
  companion downloads were planned against the REQUESTED codec ("atmos"),
  causing a redundant ALAC companion download when primary was already ALAC.

- Clear inherited binaural/downmix tags on non-binaural codecs, add activity log for codec detection fallback

Two fixes:

  1. isBinaural/isDownmix tags (MeedyaDL-specific MeedyaMeta namespace)
  were persisting on AAC Legacy and other non-binaural/downmix files.
  When effective codec is not binaural/downmix, enrichment now explicitly
  removes these tags via clear_binaural_downmix_tags(). Prevents stale
  tags from prior enrichment passes or overwrite scenarios.

  2. Codec detection fallback chain (MediaInfo -> ffprobe -> requested)
  now emits activity log entries (not just verbose/debug logs) so users
  see when detection falls back. Passes dl_id for per-download logging.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Remove [skip ci] convention — all pushes must trigger CI

Updated CLAUDE.md to explicitly prohibit [skip ci] in commit messages
  unless the user explicitly requests it. Every push to main must trigger
  CI, Release Please, and CodeQL workflows for proper validation.


## [0.15.0] - 2026-03-26

### ✨ Features

- Integrate MediaInfo CLI for accurate codec detection (#246)

- Added MediaInfo as 5th managed tool in dependency_manager (optional)
  - tool-versions.toml: [mediainfo] section (min v22.0)
  - New mediainfo_service.rs: JSON parser for mediainfo --Output=JSON
    with definitive Atmos detection (Format_AdditionalFeatures: "JOC")
  - metadata_tag_service.rs: MediaInfo primary, ffprobe fallback
    for codec detection in enrichment Step 1
  - URL resolver for macOS (DMG/.pkg), Windows (ZIP), Linux (mirror)
  - 8 unit tests for codec classification (Atmos, AC3, ALAC, AAC, HE-AAC)
  - Setup Wizard auto-detects MediaInfo via get_all_tools()

- Add SpatialAudioCodec ISRC annotation for Atmos/AC3 tracks (#121)

When the detected codec is Atmos, AC3, or Binaural, writes a
  MeedyaDL:SpatialAudioCodec freeform atom to the file. This marks
  the ISRC as belonging to the spatial version of the track, enabling
  future cross-platform ISRC matching for spatial audio variants.

- Enhance empty state messages with icons and improved guidance (#251)
- Add copy-to-clipboard button for Activity Log entries (#255)

Each log entry now shows a small copy icon on hover (top-right corner).
  Clicking copies the entry's line content to the clipboard. Uses
  group-hover opacity transition for non-intrusive discoverability.

- Add keyboard shortcuts help page (#252)

New help/keyboard-shortcuts.md documenting all navigation (Cmd+D,
  Cmd+comma, Cmd+Q) and action shortcuts (Enter, Shift+Enter, Escape).
  Added to help index under new "Reference" section.

- Add i18n translation keys for download, queue, activity, history (#111)

Added translation keys for:
  - Download page: URL label, content types, import manifest, validation
  - Queue page: empty state, clear all/completed, start/export/refresh
  - Activity Log: empty state, export, pause/resume, copy entry
  - History page: empty state, clear history

  Components still use hardcoded English — wiring useTranslation() to
  these keys is incremental follow-up work.

- Wire useTranslation() to Sidebar navigation and footer (#111)

Nav item labels now use t('nav.{page}') with fallback to static label.
  Footer status text ("Ready"/"Setup Required") and update button text
  ("Check for Updates"/"Checking..."/"N Updates") use translation keys.

  First component to use react-i18next — establishes the pattern for
  incremental i18n wiring across the rest of the UI.


### 🐛 Bug Fixes

- Add loading state during preflight checks to prevent duplicate submissions (#249)

Wraps handleSubmit with isChecking state that disables the "Add to
  Queue" button and shows a spinner while preflight checks (internet,
  output path, cookies) are running. Prevents users from clicking
  multiple times on slow networks.

- Add debounced save to prevent concurrent settings write race (#250)

Added debouncedSave() to settingsStore — batches rapid save calls
  within 300ms into a single disk write. Auto-save callers (toggle
  switches) should use this instead of saveSettings() directly.
  Manual "Save" button still uses saveSettings() for instant feedback.

- Add aria-labels to context menu and queue items (#254)

- ContextMenu: aria-label="Actions menu" on the role="menu" container
  - QueueItem: role="listitem" + aria-label with download URL on each item

  WCAG 2.1 compliance — screen readers can now identify context menus
  and queue items.

- Remove placeholder Sentry DSN, use env var for real DSN (#231)

Both JS (VITE_SENTRY_DSN) and Rust (SENTRY_DSN) now read the DSN
  from environment variables at build time. Without a configured DSN,
  Sentry is a no-op with a debug log message. Removes the placeholder
  examplePublicKey@o0.ingest.sentry.io/0 that was sending data nowhere.

- Add role=list to queue items container for screen reader navigation (#125)

QueueItem children already have role="listitem". Parent container now
  has role="list" + aria-label="Download queue items" so screen readers
  can identify the list structure.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update help topics for manifest files, codec detection, queue management (#256)

- downloading-music.md: .meedyadl manifest files, Import button,
    drag-and-drop, library URL support
  - quality-settings.md: custom companion mode, ffprobe/MediaInfo codec
    detection, codec suffix accuracy
  - faq.md: .meedyadl files, queue Clear All, library URLs
  - troubleshooting.md: false failure fix, auth mode logging, log export

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with missing modules and ASS enrichment stage [skip ci]

Add 4 missing services (health_check, api_audit, ass_subtitle, mediainfo),
  2 missing commands (api_audit, history), 1 missing model (tag_registry) to
  Key Directories. Add ASS subtitle generation as enrichment step 2f.


### 🧪 Testing

- Add library URL parser tests and update content type label tests (#232)

Added 4 new tests for library URL parsing (albums, songs, playlists,
  recently-added). Updated getContentTypeLabel test to cover the new
  'library' content type. Total: 260 tests (was 256).

- Add activityStore unit tests (#232)

6 tests covering: initial state, entry addition, ordering, no entry
  cap (verifies old 5000 limit was removed), clearEntries, paused state.
  Total: 266 tests across 19 test files.


## [0.14.0] - 2026-03-26

### ✨ Features

- Accept Apple Music personal library URLs (#243)

Library URLs (e.g., music.apple.com/library/albums/l.8zPXbAv) were
  rejected by the frontend URL parser. Added 'library' content type with
  /library/ path detection, Library icon, and label. URLs pass through
  to GAMDL as-is; enrichment naturally skips non-catalog URLs.

- Enhance GlobalProgressBar to display download percentage alongside speed and ETA
- Improve activity log — remove entry cap, timestamped export filename

- Remove 5,000 entry cap; log grows unbounded per session, resets on restart
  - Export filename now includes date/time: MeedyaDL-activity-log_YYYY-MM-DD_HHhMMm.log

- Add platform icon to GlobalProgressBar

Shows an Apple Music icon next to the track name in the per-item
  progress bar. Uses inline SVG with a detectPlatform() helper and
  PLATFORM_ICONS lookup, extensible for future services.

- Embed .meedyadl manifest file in album download folders (#245)

After enrichment, writes a `.meedyadl` JSON manifest to each album
  output directory. Records source URL, platform, storefront, codec,
  and per-track metadata (ISRC, title, individual URLs). Supports
  multi-platform source merging — new platforms append to the existing
  manifest without overwriting.

- Generate .meedyadl document type icon in PNG/ICO/ICNS formats

Generated from assets/brand/icon-doc.svg (split disc/reel design).
  Icon files ready for platform-specific file association wiring:
  - icon-doc.png (512px) — cross-platform fallback
  - icon-doc.icns — macOS CFBundleTypeIconFile
  - icon-doc.ico — Windows registry association

  Tauri v2 doesn't expose fileAssociations.icon yet — platform-specific
  wiring deferred to follow-up issue.

- Add .meedyadl document type icon SVG source

Split disc/reel design matching the MeedyaDL brand. Source file for
  the PNG/ICO/ICNS variants already committed. Brand asset (proprietary).

- Wire up .meedyadl manifest import UI (#247)

- "Import" button on Download page: opens native file picker for
    .meedyadl files, populates URL textarea with source URLs
  - Drag-and-drop: .meedyadl files dropped on the app are parsed and
    URLs populated (alongside existing Apple Music URL drop support)
  - Deep link / file association: multi-source manifests now emit all
    URLs joined by newlines (not just the first) for batch queueing

- Clear all queue with confirmation, wrapper logging, CVD debugging (#248)

- "Clear All" button on Queue page with confirmation modal. Removes all
    non-active items (queued, completed, cancelled, errored). Active
    downloads are preserved. Uses clear_all() in DownloadQueue + IPC.
  - Wrapper authentication status now emitted to user-visible Activity Log
    at download start: "Authentication: Wrapper ({url})" or
    "Authentication: Cookie-based (no wrapper)".
  - CVD (colour blind) modes verified working — CSS bundle confirmed to
    contain all 9 CVD selectors. Added console.debug logging to useTheme
    hook for easier troubleshooting.
  - Animated artwork confirmed independent of wrapper (uses MusicKit JWT).


### 🐛 Bug Fixes

- Resolve false 'no output files' failure on GAMDL 2.9.x album downloads (#242)

GAMDL 2.9.x with native --song-codec-priority does not emit "Saved to:"
  lines for album downloads. The success path only set output_path via that
  event, and the disk-scan fallback (find_album_directory) only ran inside
  codec/IO error branches — the clean-exit path was unhandled.

  Added a general disk-scan fallback before the terminal failure check that
  runs for ALL cases where output_path is None after GAMDL exits 0. This
  prevents the cascading bug where the false failure triggered auto-retry
  without wrapper, which overwrote successful Atmos files with ALAC.

- Export activity log as .log instead of .txt

Changes the native save dialog filter and default filename from
  meedyadl-activity-log.txt to meedyadl-activity-log.log.

- Include all user-selected codecs in Custom companion tiers

plan_companions() previously filtered out codecs matching the primary
  setting. With native priority the actual codec GAMDL picks may differ,
  so the user's explicit Custom selections are now always respected.

  Also adds a visual separator (═══) in the activity log when each new
  queue item starts processing, making it easy to distinguish boundaries.

- Apply codec suffix based on ffprobe-detected codec, not requested

enrich_single_file() now returns the effective SongCodec detected via
  ffprobe. The post-enrichment suffix rename uses this per-file detected
  codec instead of the requested codec from settings.

  Previously, requesting Atmos with native priority could apply a
  [Dolby Atmos] suffix to files that actually contained ALAC (when
  GAMDL silently fell back). Now the suffix accurately reflects the
  file's actual content.

- Manifest tweaks — download start time, null codec, vendor MIME (#245)

- downloaded_at now captures when the download starts processing
    (not when enrichment finishes or the manifest is written)
  - codec fields default to null at both source and track level —
    the manifest is a metafile for re-downloading, not a quality spec
  - MIME type changed to vendor convention per RFC 6838 §3.2:
    application/vnd.mwbmpartners.meedyadl.download+json

- **(deps)** Resolve picomatch ReDoS vulnerability (GHSA-c2c7-rcm5-vvqj)

npm audit fix: picomatch 4.0.3 → 4.0.4. Fixes high-severity ReDoS
  via extglob quantifiers and method injection in POSIX character classes.

- Resolve ESLint react-hooks/exhaustive-deps warning in App.tsx

Read showSetupWizard imperatively via getState() inside the async
  initialize() function instead of using the reactive selector. The
  value is only needed once after all awaits complete.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with manifest files, library URLs, codec suffix, queue clear, wrapper logging [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- Upgrade pinned GitHub Actions from Node.js 20 to Node.js 24 (#241)

actions/checkout v4→v6.0.2, actions/setup-node v4→v6.3.0,
  Swatinem/rust-cache v2.8.2→v2.9.1 — all now use Node.js 24 runtime,
  resolving the deprecation warning before the June 2, 2026 deadline.


## [0.13.0] - 2026-03-24

### ✨ Features

- Double sidebar logo and logotype size, expand logotype to fill width

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.12.1] - 2026-03-20

### 🐛 Bug Fixes

- Re-sync config.ini before each GAMDL invocation to prevent stale config

GAMDL 2.9.3 overwrites config.ini with its own defaults when run,
  causing our storefront and other settings to be lost. The storefront
  being None causes: AttributeError: 'NoneType' has no attribute 'upper'

- Correct logotype static fallback positions for <img> rendering

When loaded as <img> (in the app sidebar), JavaScript doesn't execute,
  so the dynamic layout script can't reposition elements. The hardcoded
  fallback positions were set for the old uppercase "MEEDYA" layout,
  leaving a 76px gap with the current mixed-case "Meedya".

  Updated static positions to match the script's calculated values:
  - Dots: cx 418 -> 342
  - Suffix: x 434 -> 345
  - Bracket: x 524 -> 473
  - ViewBox: 600 -> 487

  The dynamic script still runs in browser contexts and will override
  these for other product names (Manager, DB). But for "DL", the static
  positions now render correctly without JavaScript.

- Revert master logotype.svg, keep tight positions only in public/

Master (assets/brand/logotype.svg) restored to original wide fallback
  positions (viewBox 600, dots cx=418, suffix x=434). The dynamic JS
  script handles positioning at runtime in browser contexts.

  Only public/logotype.svg retains the tight static positions (viewBox
  487, dots cx=342, suffix x=345) for the app sidebar where JS doesn't
  execute inside <img> tags.

- Resolve doc_lazy_continuation clippy warning in sync_gamdl_config

Add blank /// line between the parameter list and the function
  description paragraph. Clippy 1.94 treats the continuation as a
  malformed doc list item without the separator.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md and memory with security hardening details [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.12.0] - 2026-03-20

### ✨ Features

- Dynamic brand theming — sidebar respects colour-blind mode (closes #220)

Read colour_blind_mode from settings store and pass as ?mode= query
  parameter to the sidebar logo.svg and logotype.svg <img> tags.

  When a colour-blind mode is active (deuteranopia/protanopia/tritanopia),
  the SVGs render with the corresponding accessible palette. The mode
  parameter is processed by the SVGs' embedded JavaScript.

  Dark mode continues to be handled automatically via @media
  (prefers-color-scheme: dark) in the SVG CSS.


### 🐛 Bug Fixes

- Implement atomic file writes for settings and queue (closes #230)

Replace std::fs::write with write-to-temp-then-rename pattern for both
  settings.json and queue.json. This prevents file corruption if the
  process crashes or loses power during a write operation.

  The rename() syscall is atomic on all major filesystems (APFS, ext4,
  NTFS), so the file is either fully written or unchanged — never
  partially written/corrupt.

  Files affected:
  - config_service.rs: settings.json -> settings.json.tmp -> rename
  - download_queue.rs: queue.json -> queue.json.tmp -> rename

- Debounce queue saves, create SECURITY.md, CSP-safe SVG embeds

#233 (closes): Debounce queue persistence to max once per 500ms.
  Uses AtomicU64 timestamp to skip rapid sequential saves, with a
  delayed follow-up save to ensure final state is always persisted.

  #234 (closes): Create SECURITY.md with vulnerability reporting
  instructions, supported versions, and security measures list.

  #221 (closes): Switch sidebar SVG embeds from <object> to <img> tags.
  <img> blocks SVG script execution (CSP defence-in-depth) while still
  rendering CSS animations. Added onError fallback for logotype.

- Resolve clippy warnings from CI (collapsible_str_replace, needless_borrow)

Fix 3 clippy warnings that failed CI on Windows (Rust 1.94.0):
  - config_service.rs: .replace('\n', "").replace('\r', "") -> .replace(['\n', '\r'], "")
  - settings.rs: same collapsible_str_replace fix
  - dependency_manager.rs: remove needless & on read_dir(temp_dir)

- Add focus trap, ARIA dialog role, and focus management to Modal (closes #218, #182 partial)

Modal accessibility improvements:
  - Added role="dialog" and aria-modal="true" to the panel element
  - Added aria-labelledby linking to the modal title
  - Focus trap: Tab/Shift+Tab cycle within focusable elements inside
    the modal, preventing focus from escaping to background content
  - Auto-focus: moves focus to the first focusable element on open
  - Focus restore: returns focus to the triggering element on close
  - Panel has tabIndex={-1} so it can receive programmatic focus

  These are the critical accessibility fixes from the audit (#218).
  Manual QA testing (#182) still needed for VoiceOver/NVDA/Orca.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update README, Dev_Notes, Claude memory with brand asset docs
- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with brand assets, directory structure, conventions [skip ci]

- Added assets/brand/ and public/ to Key Directories
  - Updated scripts/ description to include icon/APNG generators
  - Added CodeQL to workflows list
  - Added brand assets convention (proprietary license, SVG sources,
    sidebar usage, regeneration scripts, copyright year)

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### Security

- Sanitize newlines in INI config values (closes #226)

Add sanitize_ini_value() that strips \n and \r from all user-provided
  string values before writing to GAMDL's config.ini. Prevents INI
  injection via crafted settings import files where a newline in a
  path/URL/template value could inject arbitrary configparser keys.

  Applied to: cookies_path, output_path, temp_path, wrapper_account_url,
  all 6 template strings, all 4 tool paths.

- Add rehype-sanitize to HelpViewer markdown rendering (closes #227)

Add rehype-sanitize alongside rehype-raw to strip dangerous HTML
  elements (script, iframe, event handlers) while preserving safe ones
  (details, summary, strong, em, etc.).

  Custom schema extends GitHub's default to allowlist <details> and
  <summary> elements needed for collapsible About sections.

- Replace sh -c format! with direct process invocations (closes #228)

Eliminated two sh -c shell command constructions in dependency_manager.rs:

  1. GPAC .pkg extraction: replaced gunzip|cpio pipe with two-step
     process (gunzip to temp file, then cpio -F). No shell involved.

  2. Debian .deb data extraction: replaced tar with shell glob
     (data.tar.*) with Rust read_dir + find to locate the archive
     file, then direct tar invocation with arg(). No shell involved.

  Both changes prevent potential shell injection if paths ever contain
  special characters. Paths are now passed as OS arguments, never
  interpolated into shell strings.

- Add field-level validation to settings import (closes #229)

Add sanitize_imported_settings() that validates/cleans all user-provided
  fields after JSON deserialization:
  - Truncate paths to 1024 chars, URLs to 2048, templates to 512
  - Strip \n and \r from all string values (INI injection prevention)
  - Truncate language/storefront to 20/10 chars
  - Limit exclude_tags array to 50 entries, each 100 chars max

  Applied before merging with current settings so crafted import files
  cannot inject excessively long strings or control characters.


## [0.11.0] - 2026-03-20

### ✨ Features

- Add pre-release verbose log persistence, collapsible About sections, component versions, and fix release table formatting

- Version-aware verbose_activity_log: pre-release (v0.x) preserves setting
    across restarts; full releases reset to false on startup (closes #216)
  - Pre-release first-load notice modal shown on each new pre-release version
    launch with option to install stable release if available
  - last_seen_version field in AppSettings for version change detection
  - Collapsible Help > About sub-sections using <details>/<summary> HTML
    elements, collapsed by default (closes #214)
  - Component Library section in About shows dynamic version table for all
    installed components (Python, GAMDL, FFmpeg, etc.) (closes #215)
  - get_component_versions IPC command returns version info for all components
  - Component versions logged to Activity Log at app startup
  - Fix release.yml finalize-release job to include platform emojis and
    direct download links in release tables (was missing since v0.6.5)
  - Add rehype-raw dependency for HTML-in-markdown support
  - CSS styles for collapsible <details>/<summary> disclosure elements
  - GitHub Issues created: #213 (auto-delete crash reports), #214 (collapsible
    About), #215 (component versions), #216 (verbose log), #217 (logo redesign)

- Add MeedyaDL logo SVGs, fix help version, close duplicate issues

- New logo.svg and logotype.svg in assets/brand/new/ (closes #217)
    - Animated SVG with CSS custom properties for customisation
    - prefers-reduced-motion support
    - Descriptively named elements
  - Fix help/index.md version from 0.1.3 to 0.10.0
  - Closed duplicate/superseded GitHub issues: #205, #208, #209, #210, #211

- Logo crossfades between vinyl disc and film projector

Redesign the logo to alternate between two media symbols:
    - Vinyl disc (audio): rotating grooves, label area, centre hole
    - Film projector (video): dual reels with spinning spokes,
      lens barrel, flickering beam cone, film strip detail

  The two symbols crossfade on an 8s cycle (customisable via
  --logo-transition-speed). Each is visible for ~40% of the cycle
  with smooth 10% crossfade overlaps. When reduced-motion is active,
  the vinyl disc is shown statically.

  Natural animations:
    - Disc grooves rotate continuously
    - Projector reels spin (top and bottom at different speeds)
    - Projector beam flickers with irregular steps
    - Download arrow bounces

  Same colour/mode system as logotype.svg:
    - CSS custom properties for all colours
    - ?mode= URL parameter (light/dark/cb-deutan/etc.)
    - prefers-color-scheme and prefers-reduced-motion
    - Drop shadows per layer

- Rebuild logo.svg with full colour mode system and drop shadows

Promoted concept D to main logo.svg with complete implementation:
  - Disc/reel at r=195, vinyl internals r=190
  - Drop shadows on: emblem (disc-shadow), outer glow (for dark bg),
    chevron groups (chev-shadow) - all use CSS var(--logo-shadow/glow)
  - Full colour mode system matching logotype.svg:
    light (default), dark (@media + .dark class),
    cb-deutan/protan/tritan (light + dark variants)
  - ?mode= URL parameter support via embedded script
  - SVG has no fixed width/height - expands to fill container
  - All colours use CSS custom properties (no hardcoded colours
    outside the vinyl black surface)
  - Vinyl and reel each in their own wrapper group with clip-path
  - prefers-reduced-motion disables all animations

- Generate APNG animations from logo and logotype SVGs

New script scripts/svg-to-apng.mjs:
  - Renders SVG animations frame-by-frame via headless Chromium (puppeteer)
  - Captures with omitBackground for full alpha transparency
  - Assembles frames into APNG via ffmpeg
  - 15 FPS, 8-second cycle (120 frames per animation)

  Output files:
  - assets/brand/new/logo.apng (15 MB, 512x512, vinyl/reel crossfade)
  - assets/brand/new/logotype.apng (4 MB, 600x130, gradient shimmer)

  Both have full alpha transparency and loop infinitely.
  Run: node scripts/svg-to-apng.mjs to regenerate.

- Promote logo_new2 to logo.svg, align logotype dark/CB colours, add test page
- Generate animated PNG for all 8 colour modes, .png extension

Replaces the old .apng files with .png-extension animated PNGs for
  compatibility. Generates 16 files total (2 SVGs x 8 modes):

  Logo (512x512, 15fps, 8s cycle, vinyl/reel crossfade + chevrons):
    logo.png, logo-dark.png, logo-cb-deutan.png, logo-cb-protan.png,
    logo-cb-tritan.png, logo-cb-deutan-dark.png, logo-cb-protan-dark.png,
    logo-cb-tritan-dark.png

  Logotype (485x99 trimmed, 15fps, 8s cycle, gradient shimmer):
    logotype.png, logotype-dark.png, logotype-cb-deutan.png,
    logotype-cb-protan.png, logotype-cb-tritan.png,
    logotype-cb-deutan-dark.png, logotype-cb-protan-dark.png,
    logotype-cb-tritan-dark.png

  All files have full alpha transparency, content-aware trimming,
  and infinite looping. Mode colours applied via inline styles in
  the puppeteer renderer for reliable cross-browser support.

- Add split disc/reel app icon with all platform formats

New icon.svg: static split design with left-half vinyl record and
  right-half film reel, clipped via SVG clipPath. No animations or
  chevrons — designed for app icons, favicons, and tray icons.

  Generated formats:
    icon.png              — 1024x1024 static PNG (281 KB)
    icon.ico              — Windows ICO, 16-256px (62 KB)
    favicon.ico           — Web favicon, 16/32/48px (6 KB)
    icon.icns             — macOS ICNS via iconutil (798 KB)
    icon-liquidglass.png  — Apple Liquid Glass, 10% inset (332 KB)
    icon-liquidglass.icns — Apple Liquid Glass ICNS (790 KB)

  All have full alpha transparency. Regenerate with:
    node scripts/generate-icons.mjs

- Add brand kit page and icon previews to test page

brandkit.html — comprehensive brand reference including:
    - Logo section with all 8 mode variants (light/dark/CB)
    - Logotype section with all 8 mode variants
    - App icon section (PNG, ICO, ICNS, Liquid Glass)
    - Full colour palette (light, dark, 3x colour-blind)
    - Typography reference (Orbitron + Rajdhani)
    - MeedyaSuite product name variants
    - Complete file reference table with sizes and use cases
    - Customisation methods (URL param, hash, class, JS)
    - Regeneration script commands
    - Adapts to system dark mode via prefers-color-scheme

  logo.html — added icon section with:
    - icon.svg on light and dark backgrounds
    - icon.png static preview
    - Liquid Glass on light and dark backgrounds
    - Favicon size previews (48/32/16px)

- Generate icon variants for all 8 colour modes, update copyright to 2026
- Restructure brand assets, wire new icons, proprietary license

Brand restructure:
  - Copied brand assets from assets/brand/new/ to assets/brand/
  - Deleted logo.html (replaced by brandkit.html)
  - Updated SVG license headers from MIT to proprietary:
    "All rights reserved. MeedyaDL brand assets are proprietary."

  Tauri icons regenerated from new icon.svg:
  - All standard sizes (32-512px) + @2x variants
  - Windows Store logos (Square30-310px + StoreLogo)
  - iOS AppIcon set (20-512@2x)
  - Android mipmap set (mdpi-xxxhdpi)
  - icon.ico and icon.icns replaced

  Web integration:
  - New favicon.ico and app-icon.svg copied to public/
  - index.html: added ICO fallback alongside SVG favicon
  - tauri.conf.json icon paths unchanged (already correct)

- Consolidate brand assets, wire animated SVGs into sidebar

Brand asset consolidation:
  - Removed assets/brand/new/ (duplicate of assets/brand/)
  - Removed assets/icons/app-icon.svg (replaced by assets/brand/)
  - Updated scripts/svg-to-apng.mjs and generate-icons.mjs to use
    assets/brand/ instead of assets/brand/new/

  Sidebar branding:
  - Replaced static <img> icon with animated <object> logo.svg
    (vinyl/reel crossfade, auto dark mode, colour-blind aware)
  - Replaced text "MeedyaDL" with animated <object> logotype.svg
    (gradient shimmer, bracket flash, dot pulse)
  - Both use <object> for full SVG animation support with fallback
    content (static icon PNG / text) for non-SVG contexts
  - pointer-events-none prevents interference with drag regions
  - SVGs auto-detect dark mode via @media(prefers-color-scheme)

  Public assets:
  - Copied logo.svg and logotype.svg to public/ for web runtime access


### 🐛 Bug Fixes

- Redesign logotype SVG as text-only wordmark for MeedyaSuite

- Remove icon from logotype (text-only as requested)
  - Switch from Inter to Poppins Black (900) for more character
  - Design as MeedyaSuite brand template:
    - "Meedya" prefix is the brand constant (id="brand-prefix")
    - Product suffix is swappable (id="product-suffix")
    - Works for MeedyaDL, MeedyaManager, MeedyaDB
  - Animated gradient shimmer on brand text
  - Decorative dot separator between brand and suffix
  - CSS custom properties for theming
  - prefers-reduced-motion support
  - Google Fonts @import for Poppins with fallback stack

- Switch logotype to Orbitron + Rajdhani for techy/futuristic feel

Replace Poppins (generic geometric sans) with:
  - Orbitron Black (900) for brand prefix — sharp geometric display
    face with clipped corners, sci-fi/tech aesthetic
  - Rajdhani SemiBold (600) for product suffix — angular condensed
    sans with digital readout quality

  Add decorative tech elements:
  - Square bracket frames flanking the wordmark
  - Vertical circuit-dot separator (3-dot data bus motif)
  - Horizontal scan line animation (HUD/terminal sweep)
  - Dashed accent underline (circuit trace)
  - Neon glow filter on brand text
  - All uppercase for sharper tech feel

- Remove double-hyphens from XML comment in logotype.svg

XML forbids '--' inside comments. The CSS custom property names
  listed in the header comment contained '--' prefixes which caused
  a parse error in browsers (Edge, Chrome, Firefox). Removed the
  '--' prefixes from the comment text — the actual CSS properties
  in the <style> block are unaffected.

- Embed Orbitron + Rajdhani fonts as base64 WOFF2 in logotype SVG

Replace the external Google Fonts @import with four self-contained
  @font-face declarations using base64-encoded WOFF2 data:
    - Orbitron 700 (brand prefix, bold)
    - Orbitron 900 (brand prefix, black)
    - Rajdhani 600 (product suffix, semibold)
    - Rajdhani 700 (product suffix, bold)

  The SVG is now fully self-contained (68 KB) and renders correctly
  without any network requests. Fonts can be edited/changed by
  replacing the base64 data or converting text to outlines.

- Tighten logotype spacing and reduce canvas width

Move circuit dots, product suffix, and right bracket ~60px left to
  eliminate the excess gap between "MEEDYA" and the separator dots.
  Reduce viewBox from 720x130 to 600x130 to match the tighter layout.

- Dynamic canvas width, mixed-case brand, respect suffix casing

- Change "MEEDYA" to "Meedya" for brand prefix
  - Remove text-transform: uppercase from both text styles so casing
    respects the actual text content per product:
      MeedyaDL, MeedyaDB, MeedyaManager
  - Add embedded <script> that dynamically measures text widths and
    repositions circuit dots, suffix, bracket, underline, and resizes
    the viewBox on load — canvas auto-fits any suffix length
  - Remove hardcoded width attribute; viewBox drives sizing
  - Uses document.fonts.ready API for accurate post-font measurement

- Tighten dot separator to colon-like spacing (Meedya:DL)

Reduce GAP and DOT_GAP from 16px/12px to 3px/3px so the circuit
  dots sit tight against the brand prefix and suffix, reading as
  "Meedya:DL" rather than "Meedya  :  DL".

- Heavier suffix weight, drop shadows, dark/colour-blind palettes

Suffix text ("DL"):
  - Switch from Rajdhani 600 to Orbitron 900 (matches prefix weight)
  - Add 1.5px stroke for extra visual heft
  - Now reads as one cohesive word with the prefix

  Drop shadows:
  - New dual-layer text-shadow filter on both prefix and suffix
  - Layer 1: dark directional shadow for legibility on any background
  - Layer 2: coloured neon glow for brand feel
  - Shadow colour/opacity driven by CSS custom properties

  Colour adaptation:
  - Automatic dark mode via @media(prefers-color-scheme: dark)
  - Manual .dark class override for app-controlled themes
  - Colour-blind palettes: .cb-deutan, .cb-protan, .cb-tritan
  - All colours overridable via CSS custom properties or JS

- Embed full font character sets, match dot height to cap height
- Switch to slate/steel palette, add ?mode= URL parameter

Colour palette:
  - Replace blue/purple/cyan AI-vibe colours with slate/steel gradient
    (dark slate #475569 -> steel #64748B -> silver #94A3B8)
  - Dark mode: light silver/near-white for visibility on dark backgrounds
  - Colour-blind palettes updated with dark variants for all 3 types

  URL parameter mode switching:
  - ?mode=light (default slate/steel)
  - ?mode=dark (silver/white for dark backgrounds)
  - ?mode=cb-deutan, ?mode=cb-protan, ?mode=cb-tritan (light bg)
  - ?mode=cb-deutan-dark, ?mode=cb-protan-dark, ?mode=cb-tritan-dark
  - Script reads window.location.search and applies CSS classes on load
  - Also still supports CSS class application and direct JS property override

- Extend animated gradient to suffix and dots

- Suffix ("DL") now uses its own animated gradient
    (logotype-grad-suffix-anim) with offset timing from the prefix,
    so the shimmer flows across the entire wordmark
  - Circuit dots use a separate animated gradient
    (logotype-grad-dots-anim) with a faster independent rhythm
    (0.6x the base animation speed) via dot-shimmer keyframes
  - All three animations are coordinated but distinct:
    prefix shimmer, suffix shimmer (offset), dots shimmer (faster)
  - Reduced motion media query updated to disable all three

- Variable fonts, thicker brackets with flash control, Dev_Notes docs
- Re-embed full character set fonts (207 + 465 glyphs)

Replace Latin-subset fonts with full character sets downloaded from
  the canonical Google Fonts repository:
  - Orbitron variable (400-900): 207 glyphs, 15 KB (full Latin Extended)
  - Rajdhani Bold: 465 glyphs, 102 KB (full Latin Extended + Devanagari)

  SVG size: 179 KB (was 49 KB subset / 308 KB with 4 static files)

- Redesign logo — simplified, distinct layers, slate/steel palette

Simplified from 6 overlapping same-colour elements to 3 clearly
  separated layers with distinct colours:
    1. Vinyl disc (dark slate) — background, with subtle grooves
    2. Base tray (accent steel gradient) — anchors the composition
    3. Download arrow (light steel/silver gradient) — foreground, high contrast

- Full-size projector with realistic detail, download arrow as watermark

Projector redesign (fills same space as vinyl disc):
  - Dual full-size reels (r=72) with 6 spokes each, spinning opposite
  - Film gate between reels with frame aperture detail
  - Film threading path connecting reels through gate
  - Multi-ring lens assembly (barrel, mount, glass, highlight)
  - Light beam cone from lens with flickering animation
  - Soft beam glow ellipse with pulsing animation
  - Ventilation slots and feet for realism
  - Body, detail lines, proportions all match vinyl disc scale

  Download arrow:
  - Moved to background as a subtle watermark (8% opacity)
  - Includes arrow shaft, chevron head, and small base tray
  - Visible but doesn't compete with media symbols
  - Removed the foreground arrow and separate base tray

- Lighter disc/reel colours, match reel speed to vinyl, dynamic sizing
- Remove dashed accent underline from logotype

Remove the dotted/dashed bottom line (accent-underline element) and
  all script references to it. The line was a decorative tech element
  but appeared as a stray visual artefact.

- Trim APNG to actual content bounds, remove excess whitespace

The svg-to-apng script now reads the SVG's viewBox after the dynamic
  layout script has run, then resizes the viewport to match. This trims
  the logotype APNG from 600px to ~487px wide (matching the actual text
  width after font measurement).

  - Replaced ffmpeg cropdetect with puppeteer viewBox measurement
  - Logotype trimmed: 600x130 -> 487x130 (no more side padding)
  - Logo unchanged: 512x512 (viewBox doesn't resize)
  - Added 1.5s wait before measurement for font loading

- Content-aware trim for both APNGs, removes whitespace on all sides

Replace viewBox-only measurement with union bounding box of all
  rendered SVG elements (circle, path, line, rect, text, etc.).
  This correctly trims both:
  - logo.apng: 512x512 -> 472x447 (disc/chevrons only, no empty edges)
  - logotype.apng: 600x130 -> 485x99 (text only, no side/top padding)

  Both retain full alpha transparency via puppeteer omitBackground.

- Add drop shadow to bracket decorations in logotype

New bracket-shadow filter applied to both [ ] polylines for
  visibility on any background. Uses --logotype-shadow colour variable.

- Update brandkit with icon variants, clean up old icon assets

- brandkit.html: added dark mode and colour-blind icon variant
    sections with preview cards and download links
  - Cleaned up assets/icons/variants/ (old concept SVGs removed)
  - Updated assets/icons/app-icon.svg and public/app-icon.svg
    with the new split disc/reel icon
  - SVG license headers updated to proprietary format with full
    copyright year


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CLAUDE.md with new features and architecture details [skip ci]

- Add pre-release version handling, collapsible About, component versions
  - Add drag-and-drop, batch paste, download history, notifications, deep links
  - Update enrichment pipeline count (11→12 stages)
  - Add keyboard shortcuts, settings sidebar, accessibility, storefront config
  - Add ISRC handling, codec suffix rename, activity log search documentation
  - Update key directories with missing entries (hooks, styles, history_service)

- Add MeedyaSuite logotype customisation guide to Dev_Notes [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔄 CI/CD

- Add CodeQL workflow excluding Rust analysis

Override GitHub's dynamic "Default setup" CodeQL configuration with an
  explicit workflow that analyses only actions and javascript-typescript.

  Rust analysis is excluded because CodeQL's Rust extractor requires a
  full Cargo build, which routinely hangs for 6+ hours on this project
  (see Actions run #500). Rust code quality is already covered by
  cargo clippy, cargo test, and cargo-deny in ci.yml.


### Revert

- Restore original pulsating dot size and vertical positions

Revert the dot height/radius changes from the previous commit.
  Dots return to cy=58/72/86, r=3 (original colon-like positions).
  Dynamic script only repositions dots horizontally, not vertically.


## [0.10.1] - 2026-03-20

### 🐛 Bug Fixes

- Ensure all v0.x releases are marked as pre-release

Add "prerelease": true to release-please-config.json so release-please
  creates GitHub releases with the pre-release flag for all v0.x versions.

  Fixed v0.10.0 and v0.8.0 which were incorrectly marked as full releases.

  Also created GitHub issues for upcoming tasks:
  - #208: auto-delete crash reports after submission
  - #209: verbose logging persistence for pre-release versions
  - #210: component library versions on About screen + startup log
  - #211: restore platform emojis + download links in release tables


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.10.0] - 2026-03-20

### ✨ Features

- Add meedyadl:// deep link URL scheme (closes #200)

Register custom URL scheme via tauri-plugin-deep-link:
  - meedyadl://download?url=<apple_music_url>&codec=<optional>
  - Handles both running-app (on_open_url) and cold-start (get_current)
  - Pre-fills download form URL input and navigates to Download page
  - Brings main window to foreground on deep link receipt
  - Activity log entry for received deep links

- Add activity log search and category filtering (closes #199)

- Search input with clear button for case-insensitive text filtering
  - Category toggles: System (on), Download (on), Verbose (off by default)
  - Filtered count shown in subtitle when filters active
  - Empty state message when no entries match
  - Export still exports all entries regardless of filter
  - ARIA role="checkbox" with aria-checked on filter toggles

- Add duplicate URL detection in download queue (closes #197)

- normalize_url_for_dedup(): lowercase domain, strip trailing slashes,
    fragments, and non-essential query params (keeps ?i= for track IDs)
  - has_duplicate_urls(): checks against active/queued items only
  - StartDownloadResult struct replaces plain string return from start_download
  - Frontend shows warning toast for duplicates (non-blocking)
  - 13 new unit tests for normalisation and duplicate detection

- Add persistent download history page (closes #196)

JSON-based history database at {app_data_dir}/history.json:
  - Records URL, title, artist, album, codec, file path, timestamps
  - Max 1000 entries with oldest trimmed
  - Search via Rust backend (case-insensitive on title/artist/album/URL)

  New History page (sidebar nav between Queue and Activity):
  - Search input with 300ms debounce
  - Status icons (success/failed), codec badges, dates
  - "Open Folder" action for successful downloads
  - "Clear History" button
  - 3 new Rust unit tests (639 total)

- Add drag-and-drop URL input from browser (closes #195)

Drag Apple Music URLs from any browser directly into MeedyaDL:
  - Drop-zone overlay with semi-transparent backdrop and dashed border
  - Extracts URL from text/uri-list or text/plain data transfer
  - Validates via parseAppleMusicUrl, navigates to Download page
  - Nested dragenter/dragleave counter prevents overlay flicker
  - Success/error toasts for valid/invalid URLs

- Add batch URL paste — queue multiple Apple Music URLs at once (closes #194)

Replace the single-line URL input with an auto-resizing textarea that
  supports pasting multiple Apple Music URLs (one per line). When multiple
  URLs are detected, each is validated individually and submitted as a
  separate queue item. The badge shows "N URLs" count instead of content
  type. Summary toast reports queued/failed/skipped counts. Quality
  overrides apply to all URLs in the batch. Single-URL flow is unchanged.

- Add native OS desktop notifications (closes #193)

Integrate tauri-plugin-notification for download events:
  - "Download Complete" notification on successful download
  - "Download Failed" notification on terminal failure
  - Suppressed when app window is focused (background only)
  - desktop_notifications setting (default: true) in Settings > General
  - Backend-driven via send_desktop_notification() helper

- Add settings sidebar sub-categories (closes #207)

Group settings tabs under 4 section headers:
  - General: General
  - Download: Quality, Fallback, Lyrics, Cover Art, Metadata, Templates
  - Authentication: Cookies
  - System: Tools, Advanced

  Section headers: 10px uppercase, muted colour, non-interactive.
  Prepares for per-service settings groups in multi-service architecture.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.9.0] - 2026-03-20

### ✨ Features

- Log key settings on startup and include in crash reports (closes #203)

Activity Log: emit 3 concise [System] entries on every startup:
  - Config: codec, video resolution, companion mode, storefront, download mode
  - Features: enhanced_lrc, advisory_suffixes, acoustid, replaygain, musicbrainz
  - Auth: wrapper status, cookies presence, musickit configuration

  Crash Reports: add settings_snapshot_for_context() helper that populates
  crash report context with redacted settings (no paths/credentials).
  Integrated into both error handler sites in download_queue.rs.

- Add settings export/import for backup and device transfer (closes #202)
- Add keyboard shortcuts help topic (closes #201)

Add 'Keyboard Shortcuts' to the in-app HelpViewer with:
  - Full shortcuts table (Cmd/Ctrl+D, Cmd+,, Cmd+Q, Escape, Cmd+Enter)
  - Platform-specific modifier key notes (Cmd on macOS, Ctrl on Win/Linux)
  - Modal shortcuts (Escape, Tab focus trapping)
  - Accessibility navigation (Tab, Shift+Tab, skip link)

- Add download statistics panel on Activity page (closes #198)

Session-based stats derived from queue items via useMemo:
  - Total downloads, success rate (green/amber/red), top codec
  - Active/Queued/Completed/Failed counts with status colours
  - Collapsed by default, hidden when queue empty

  Full historical stats will follow #196 (download history database).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Add storefront config.ini generation tests

Verify storefront is written to config.ini:
  - Auto-detect from language (en-US → us)
  - Explicit override when set (gb)


## [0.8.1] - 2026-03-19

### ✨ Features

- Add storefront as user-configurable setting

Add explicit storefront field to AppSettings so users can set their
  Apple Music region (e.g., gb, us, jp) directly.


### 🐛 Bug Fixes

- Add storefront to GAMDL config, codec suffix rename, ISRC logic

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.8.0] - 2026-03-19

### ✨ Features

- Enhance codec handling by adding AC3 support and refining suffix application

### 🐛 Bug Fixes

- Parse GAMDL 2.9.x track format for progress bar display

GAMDL 2.9.x changed its output from "Getting track N of M: Title"
  to "[Track N/M] Downloading \"Title\"". Add TRACK_INFO_V2_REGEX to
  parse the new format and extract track number/total for progress
  calculation.

  - Add track_number/track_total optional fields to TrackInfo event
  - Compute approximate progress from track counts (N-1/M percentage)
  - Update TypeScript types and download store handler
  - Progress bars now show fill and track names during Apple Music downloads

- Update ISRC reconciliation logic for Vendor tag extraction

Update extract_isrc_from_vendor() with 3-case logic:
  1. ISRC blank → copy from Apple Vendor tag (Label:isrc:CODE)
  2. ISRC set + Vendor differs → store both (API / Vendor format)
  3. ISRC set + identical or no Vendor → no-op


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.7.0] - 2026-03-19

### ✨ Features

- Migrate to Tailwind CSS v4 and update documentation

Migrate from Tailwind CSS v3.4.17 to v4.2.2 (closes #174):
  - Replace tailwindcss PostCSS plugin with @tailwindcss/postcss
  - Remove autoprefixer (built into v4's LightningCSS)
  - Replace @tailwind directives with @import "tailwindcss" + @config + @plugin
  - Load @tailwindcss/typography via @plugin in CSS instead of require() in JS
  - Bump macOS minimum from 11.0 to 13.3 (Safari 16.4+ required by v4)
  - Update Vite targets: safari13 → safari16.4, chrome105 → chrome111

  Documentation updates:
  - CHANGELOG.md: add all unreleased changes (security, stability, CI)
  - README.md: update Tailwind version and macOS minimum
  - Project_Plan.md: update status and add post-release entries
  - DEV_NOTES.md: update project structure references
  - CLAUDE.md: update architecture, build targets, Vite config
  - help/faq.md, help/getting-started.md: update macOS version

- Add cargo-deny for licence scanning and security advisory auditing in CI

Add cargo-deny configuration (deny.toml) and CI step to scan the Rust
  dependency tree for licence compliance and known security advisories.
  The config allows MIT-compatible licences, ignores Tauri's unmaintained
  GTK3 transitive dependencies, and pins the GitHub Action to a commit SHA.

- Core accessibility improvements (partial #125)

High-impact a11y improvements across the UI:
  - ARIA labels on icon-only buttons (Sidebar, UpdateBanner, QueueItem)
  - aria-live regions for toasts, activity log, and progress bars
  - prefers-reduced-motion media query disabling animations
  - Skip navigation link for keyboard users (WCAG 2.1 SC 2.4.1)
  - ProgressBar role="progressbar" with proper value attributes

- Upgrade dependencies and add queue progress indicators

Dependency upgrades (closes #117):
  - @vitejs/plugin-react 4.7.0 → 5.2.0
  - @commitlint/cli 19.8.1 → 20.5.0
  - react-markdown 9.1.0 → 10.1.0
  - All semver-compatible updates applied

  Queue progress indicators (closes #178):
  - Add queue header statistics bar with active/queued/completed/failed
    counts and aggregate progress bar
  - Add "Track N of M" counter in QueueItem for album downloads
  - Both derived from existing store data, no backend changes needed

- Add global keyboard shortcuts (closes #179)

Add useKeyboardShortcuts hook with application-wide shortcuts:
  - Cmd/Ctrl+D: navigate to Download page and focus URL input
  - Cmd/Ctrl+,: navigate to Settings
  - Cmd/Ctrl+Q: navigate to Queue
  - Escape and Cmd+Enter already handled by Modal and DownloadForm

  Shortcuts suppressed when focus is in input/textarea/select fields.
  Uses imperative store access to avoid unnecessary re-renders.

- Add high-contrast accessibility theme (closes #180)

Add a toggleable high-contrast theme for users with low vision:
  - Pure black/white text with WCAG AA+ contrast ratios
  - Strong opaque borders replacing translucent ones
  - Saturated status colours for clear differentiation
  - 3px focus-visible outlines on all interactive elements
  - Supports both light and dark mode simultaneously
  - Auto-detects OS prefers-contrast: high media query
  - Toggle in Settings > General > Appearance

- Add colour blindness accessibility themes (closes #181)

Add three colour vision deficiency (CVD) theme variants:
  - Deuteranopia (red-green): success→blue, error→orange, warning→yellow
  - Protanopia (red-green): same palette as deuteranopia
  - Tritanopia (blue-yellow): warning→pink, info→teal

  Each variant overrides status colours in both light and dark mode.
  Select in Settings > General > Appearance > Colour Vision dropdown.

- Move progress bars to global layout — visible on all pages

Add GlobalProgressBar component to MainLayout, rendered between <main>
  and StatusBar. Always visible regardless of which page the user is on:
  - Upper bar: per-item progress (current track name, speed, ETA)
  - Lower bar: queue-level progress (completed / total items)
  - Auto-hides when no downloads are active or queued

  Remove duplicate ProgressBar from DownloadQueue page header (text
  stats retained for context on the Queue page).


### 🐛 Bug Fixes

- Security hardening, dependency updates, and stability improvements

Security fixes (closes #175, #176, #177):
  - Fix TAR extraction path traversal vulnerability — iterate entries
    individually and reject paths with `..` components or absolute paths
  - Add explicit timeouts to all reqwest HTTP clients (Apple Music API,
    AcoustID, GitHub API, update checker) preventing indefinite blocking
  - Redact wrapper account URL from GAMDL CLI args log line to prevent
    credential tokens from persisting in plaintext log files

  Dependency updates:
  - Fix npm audit vulnerabilities (flatted < 3.4.0, undici 7.0.0-7.23.0)
  - Update lz4_flex 0.11.5 → 0.11.6 (memory leak fix, closes Dependabot
    security alert #17)

  CI/DX improvements:
  - Add monthly Dependency Report workflow for major version visibility
  - Fix Dependabot config to actually ignore major version bumps (the
    comment said it did but the ignore rule was missing)
  - Fix ESLint errors in Node.js build scripts (add globals for console,
    process, Buffer; remove unused deflateSync import)
  - Fix flaky Windows CI test (probe_nonexistent_directory_with_valid_parent)

  Crash report improvements:
  - Add delete_all_crash_reports command + "Clear All" UI button
  - Promote delete logging from debug to info for production visibility
  - Show actual error messages in frontend delete failure toasts

  Stability improvements:
  - Fix Tooltip setTimeout cleanup on unmount (useRef + useEffect)
  - Fix CookiesTab copy-success timeout cleanup on unmount

- Move codec filename suffixes to codecs.toml registry (closes #118)

Move hardcoded codec suffix strings from download_queue.rs to the
  codecs.toml registry, preventing filename collisions when users select
  multiple lossy codecs in Custom companion mode.

  New suffixes: AAC Binaural → [Binaural], AAC Downmix → [Downmix],
  AAC Legacy → [AAC Legacy], HE-AAC → [HE-AAC], and variants.
  Standard AAC 256 keeps clean filenames (empty suffix).

  Existing suffixes preserved: ALAC=[Lossless], Atmos=[Dolby Atmos],
  AC3=[Dolby Digital].

- Enable minor version bumps for feat: commits pre-1.0

Change bump-patch-for-minor-pre-major from true to false so that
  feat: commits correctly bump the minor version (0.6.x → 0.7.0)
  instead of only the patch version (0.6.x → 0.6.y).

  The previous setting was treating all feat: commits as patch bumps
  while the project is pre-1.0, which didn't reflect the significance
  of changes like Tailwind v4 migration, accessibility themes, etc.

- Resolve VS Code Problems — CSS prefix order and inline style

Fix 5 linter warnings:
  - globals.css: reorder user-select after -webkit-user-select (2 instances)
  - macos.css: reorder backdrop-filter after -webkit-backdrop-filter (2 instances)
  - SettingsSection.tsx: replace inline transform style with Tailwind rotate class

  Remaining 24 Problems are documented unfixable:
  - ARIA attribute values: Edge DevTools false positives on JSX expressions
  - Inline styles: dynamic runtime values (progress bar widths)
  - main.tsx: intentional ErrorBoundary styles (must work without CSS)

- **(ci)** Restrict cargo-deny to Linux runners only

cargo-deny-action is a Docker container action which is only supported
  on Linux runners. It was failing on macOS and Windows with:
  "Container action is only supported on Linux"

  Add `if: runner.os == 'Linux'` condition since licence/advisory checks
  are platform-independent — running once on Linux is sufficient.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update changelog, readme, and project context for latest changes
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Add enrichment pipeline integration tests (closes #113)

Add 30 end-to-end integration tests across 4 subtitle/lyrics services:
  - Rich SRT: TTML→SRT conversion, styling, multi-track, unicode filenames
  - WebVTT: TTML/SRT/LRC→VTT conversion, source priority, fallbacks
  - Enhanced LRC: word-level timing, line-level fallback, multi-track
  - ASS: TTML→ASS conversion, styling override tags, VTT fallback
  - Cross-service pipeline tests and CJK/emoji filename edge cases

  Total Rust tests: 579 → 609 (+30)

- Add React component rendering tests for settings tabs (closes #114)

Add 20 Vitest tests for GeneralTab, QualityTab, and AdvancedTab covering
  toggle rendering, toggle click handling, conditional visibility, and select
  dropdown rendering. Mocks lucide-react icons, Tauri IPC commands, and the
  shell plugin to enable jsdom testing without the Tauri runtime.


### 🧹 Maintenance

- Add .hintrc to suppress false-positive webhint warnings

Disable three webhint rules that produce false positives on React/JSX:
  - axe/aria: can't evaluate JSX ternary expressions for ARIA attributes
  - no-inline-styles: dynamic runtime values and ErrorBoundary styles
  - css-prefix-order: fixed where possible, remaining are intentional


## [0.6.13] - 2026-03-06

### 🐛 Bug Fixes

- Resolve MusicKit 401 validation flow and add embedded token fallback

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Streamline macOS menu setup in run function

## [0.6.12] - 2026-03-06

### ✨ Features

- Make verbose logging a session-only setting that resets on restart (#157)

Verbose logging can expose sensitive data (auth tokens, cookies, API
  responses, MusicKit credentials). As a safety measure, it now always
  resets to off on app startup — users must re-enable it each session.

  - Reset verbose_activity_log to false in load_settings() on startup
  - Add session-only note to toggle description and warning box in UI
  - Update settings.rs doc comment documenting session-only behavior
  - Update help/troubleshooting.md with session-only callout

- Add Linux app menu integration and suppress release-build terminal output (#159)

- Add custom .desktop file with proper Categories, Keywords, and
    Terminal=false for Linux application menu discoverability
  - Reference desktopTemplate in tauri.conf.json deb config
  - Suppress stderr tracing layer in release builds unless RUST_LOG
    is explicitly set — prevents terminal flooding on Raspberry Pi
    and other Linux systems when launched from command line

- Remove MusicKit credential gate from Music Video Companions (#160)

Music Video Companions no longer requires MusicKit credentials.
  MusicBrainz ISRC lookup (Step 6b) now serves as a credential-free
  discovery and download path for Apple Music videos. Step 6 (MusicKit
  API) still runs when credentials are available but gracefully skips
  when they are not.

  - Remove disabled prop and conditional description from toggle
  - Mark feature as Experimental with warning box when enabled
  - Step 6b now downloads Apple Music videos found via MusicBrainz
  - Step 6b runs when either musicbrainz_lookup OR music_video_companion
    is enabled
  - Extract download_music_video_by_url() shared helper
  - Update settings model docs and enrichment pipeline comments


### 🐛 Bug Fixes

- Improve line wrapping on MusicKit credential validation result text

The validation message next to the Test Credentials button was being
  squeezed onto one line. Use items-start alignment, shrink-0 on the
  button, and leading-relaxed on the result text for cleaner wrapping.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Fix clickable URL in wrapper help and expand MusicKit setup guide

- Replace clickable http://192.168.3.179:30020 in Help > Wrapper >
    Automatic Pre-Flight Check with non-clickable backtick-wrapped
    http://127.0.0.1:30020
  - Significantly expand Step 2 (Create a MusicKit Key) in Help >
    Animated Artwork with detailed instructions covering: free vs paid
    account checkbox differences (MusicKit vs Media Services), the
    Configure/App ID flow, direct URL for the Keys page, and a tip for
    when the MusicKit option doesn't appear

- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Reorganise Advanced settings tab section order

- Move File Options above Error Reporting
  - Move API Credentials just above Setup
  - Move API Field Audit from Metadata tab into Advanced tab (below
    AcoustID, within the API Credentials section)

  New order: Processing → Wrapper → File Options → Error Reporting →
  Diagnostics → API Credentials (MusicKit, AcoustID, API Field Audit) →
  Setup

- Add collapsible SettingsSection component to all settings tabs

Create a reusable SettingsSection component with bordered card styling,
  clickable header with rotating chevron, and collapsible content. Apply
  it across all 10 settings tabs (General, Quality, CoverArt, Lyrics,
  Metadata, Tools, Fallback, Templates, Cookies, Advanced) for consistent
  visual distinction between sections. Tighten inter-section spacing from
  space-y-6 to space-y-3 for a more compact layout.


## [0.6.11] - 2026-03-06

### ✨ Features

- Change default remux mode to MP4Box for better subtitle handling and update app behavior on first launch

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update mirror repo reference to MeedyaDL/MeedyaDL-Tools

Renamed mirror repository from MWBMPartners/meedyadl-tools to
  MeedyaDL/MeedyaDL-Tools across code, config, and documentation.
  Also fixed example asset extension (.zip → .tar.gz) in tool-versions.toml.

- Update CHANGELOG.md [skip ci]

## [0.6.10] - 2026-03-05

### ✨ Features

- Bump minimum compatible GAMDL version to 2.9.2

GAMDL 2.9.2 fixes artist download pagination. Bump MIN_COMPATIBLE_GAMDL
  from 2.0.0 to 2.9.2 so the update checker prompts users on older versions
  to upgrade.


### 🐛 Bug Fixes

- WebKitGTK rendering corruption on Raspberry Pi and tray deprecation warning

- Add setup_linux_rendering_env() that detects Raspberry Pi via
    /proc/device-tree/model and sets WEBKIT_DISABLE_DMABUF_RENDERER=1 and
    WEBKIT_DISABLE_COMPOSITING_MODE=1 before the WebView is created, forcing
    software rendering to fix garbled UI over remote desktop (RPi Connect)
  - Only applies on Raspberry Pi — desktop Linux retains GPU acceleration
  - Respects user-set env vars (won't override if already defined)
  - Update .deb dependency to accept libayatana-appindicator3-1 as
    alternative to deprecated libappindicator3-1

- Update test_is_gamdl_compatible for new minimum version 2.9.2

The test was asserting 2.8.4 and 2.0.0 as compatible, which no longer
  holds after bumping MIN_COMPATIBLE_GAMDL to 2.9.2.

- Set MIN_COMPATIBLE_GAMDL back to 2.9.1
- Resolve clippy doc comment lints on Ubuntu CI

Move run() doc comment directly above pub fn run() to fix
  empty_line_after_doc_comments, and rewrap setup_linux_rendering_env
  doc comment to fix doc_lazy_continuation.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.9] - 2026-03-05

### ✨ Features

- Verbose settings logging and move API credentials to Advanced tab

- Verbose activity log now tracks which settings changed (key: old → new)
    with sensitive fields redacted (cookies, wrapper URL, MusicKit, AcoustID)
  - Verbose mode dumps key settings summary at startup for diagnostics
  - Move MusicKit credentials (Team ID, Key ID, Private Key, Test button)
    from Settings > Cover Art to Settings > Advanced > API Credentials
  - Move AcoustID API Key from Settings > Metadata to Settings > Advanced
    > API Credentials, with note linking from Metadata tab
  - Update all help file references to point to new credential locations


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.8] - 2026-03-05

### ✨ Features

- Add content advisory suffixes ([Explicit]/[Clean]) to filenames and folder names

After metadata enrichment, album folders and track files are renamed with
  [Explicit] or [Clean] suffixes based on Apple Music content ratings. Per-track
  granularity (individual tracks can differ from album rating). Advisory suffix
  inserted before codec suffix (e.g., "01 Title [Explicit] [Lossless].m4a").
  Idempotent on re-download. Toggle in Settings > Metadata (default: enabled).


### 🐛 Bug Fixes

- Gap-fill retry for partial downloads with native priority

When GAMDL's --song-codec-priority skips tracks because experimental
  codecs (Atmos, AC3) are unavailable without wrapper auth, MeedyaDL now
  automatically re-runs GAMDL with non-experimental codecs and
  overwrite=false to fill the gaps. This recovers skipped tracks in
  lossless/lossy formats without overwriting successful Atmos/AC3 files.

  Added SongCodec::from_cli_string() and is_wrapper_dependent() methods.
  Helpers: count_codec_skip_warnings, build_gapfill_priority_chain,
  count_audio_files_in_directory. 11 new unit tests.

- Companion downloads never apply filename suffixes

apply_codec_suffix() only checked options.song_codec, but companion
  downloads set song_codec=None and use song_codec_priority instead
  (for GAMDL >= 2.9.1). This meant no companion ever got a suffix like
  [Lossless] or [Dolby Atmos], causing each companion tier to overwrite
  the previous tier's files (identical filenames).

  Fixed by falling back to parsing song_codec_priority via
  SongCodec::from_cli_string() when song_codec is None.

- Error report deletion now persists across app restarts

delete_crash_report() returned Ok(()) even when the report wasn't found
  during directory scan, so the frontend optimistically removed it from
  state while the file stayed on disk. On restart, reports reappeared.

  Now returns Err when not found, added debug logging at each scan step.

- Add missing permissions for issue closure and project item addition
- Add read permissions for project directory in settings.json

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Fix markdownlint warnings in CHANGELOG.md [skip ci]

Fix double blank lines (MD012) and inconsistent indentation (MD007)
  in manually-edited [Unreleased] and [0.6.7] sections. Deduplicate
  repeated "Update CHANGELOG.md" entries in [0.6.6]. Add issue numbers
  to changelog entries.

- Update CHANGELOG.md [skip ci]

## [0.6.7] - 2026-03-04

### 🐛 Bug Fixes

- Detect actual codec via ffprobe for correct metadata tags with native priority

When using GAMDL >= 2.9.1's --song-codec-priority, codec_used was set to
  the requested codec at enqueue time, not the actual codec GAMDL selected.
  This caused enrichment to write incorrect tags (SpatialType, isBinaural,
  isDownmix) on ALL files regardless of their actual codec.

- Warn in activity log when ffprobe unavailable with native priority

Non-verbose activity log now alerts users when ffprobe is unavailable
  or fails for a file while native priority is active, since codec tags
  may be inaccurate without it. Previously this was only logged at debug
  level via RUST_LOG=debug.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.6] - 2026-03-03

### 🐛 Bug Fixes

- README badge URLs and header logo

- Fix badge URLs: MeedyaDL/MeedyaDL → MWBMPartners/MeedyaDL (404 fix)
  - Version badge: use dynamic GitHub release API instead of hardcoded
  - CI badge: add ?branch=main for accurate status
  - Replace emoji header with app logo (src-tauri/icons/128x128.png)
  - Add .markdownlint.jsonc: allow inline HTML (standard for GitHub READMEs)

- Use MeedyaDL logo in README header with dark/light theme support

- Replace app icon with proper MeedyaDL logo (assets/logo/meedyadl-logo.svg)
  - Use <picture> element with prefers-color-scheme for dark/light variants
  - Remove h1 heading (logo serves as the header)
  - Expand markdownlint config: disable MD013 (line length), MD041 (first
    line heading), MD060 (compact table style) — all standard for GitHub READMEs

- Resolve markdownlint warnings across all documentation files

- README.md: add blank lines around all 34 headings (MD022), add language
    to 3 fenced code blocks (MD040)
  - DEV_NOTES.md: add blank lines around headings and after list blocks
  - help/cookie-management.md: add blank line before heading
  - .markdownlint.jsonc: only suppress truly unfixable rules (MD033 inline
    HTML for logos/badges, MD041 first-line heading, MD013 line length for
    URLs) — removed MD060 suppression since tables are now clean


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.5] - 2026-03-03

### ✨ Features

- Add internal codec and format registry infrastructure

Adds an internal registry for managing audio/video codecs and
  lyrics/subtitle formats via a TOML configuration file. Includes
  MIME types, format categories, and extensible mapping structure.
  Background preparation work for future planned features.

- Add terser-based JS obfuscation for production builds

Switches production minification from esbuild to terser with aggressive
  name mangling and code compression. Makes the compiled JavaScript in
  release builds significantly harder to reverse-engineer.

  Terser options:
  - Mangle top-level names and _-prefixed properties
  - Drop console.log/debugger statements in production
  - Two-pass compression for maximum size reduction
  - Strip all comments

  Zero runtime performance impact — all processing happens at build time.
  Debug builds are unaffected (minification disabled entirely).

- Add WebVTT subtitle generation from TTML, SRT, and LRC lyrics

Opt-in feature (Settings > Lyrics > Generate WebVTT Subtitles) that
  creates .vtt sidecar files from existing lyrics. Source priority:
  TTML (richest timing data), SRT (has start+end times), LRC (start
  times only, end times estimated from next cue).

  New webvtt_service.rs with ttml_to_webvtt(), srt_to_webvtt(), and
  lrc_to_webvtt() conversion functions. Integrated as enrichment Step
  2c (after lyrics fallback, before animated artwork). Skips tracks
  that already have .vtt files. 18 new unit tests.

- Mark all releases as pre-release until v1.0

All 50 existing GitHub releases marked as pre-release. Future releases
  from release.yml also default to prerelease: true. Users on the default
  setting (check_pre_releases: false) won't receive update notifications
  until a full release is published. Users who enable "Include Pre-Release
  Versions" in Settings > General will continue receiving updates.

  Added detailed pre-release vs full release workflow guide to Dev_Notes
  covering: standard pre-release pipeline, three methods to publish a
  full release (GitHub UI, workflow edit, CLI), and how the app update
  checker chooses between stable and pre-release channels.

- Add direct download links to release download table

Updated release.yml to generate download table with clickable links
  to each platform's asset file instead of plain text references. Added
  version extraction step (strips 'v' prefix from tag) for asset URLs.

  Also updated all 48 existing releases with download links via
  gh release edit. Links point directly to the release assets:
  https://github.com/REPO/releases/download/TAG/FILENAME

- Add platform emojis to release download table

Added platform identification emojis to the download table:
  - 🍎 Mac
  - 🪟 Windows
  - 🐧 Linux
  - 💻 Chromebook

  Updated release.yml template and all existing releases.

- Add MusicBrainz lookup service for video discovery and cross-platform groundwork

New musicbrainz_service.rs queries MusicBrainz database via ISRC codes
  to discover music videos and cross-platform URLs (Apple Music, YouTube,
  Spotify, Deezer, Tidal). No credentials required (free public API).

  Integrated as enrichment Step 6b — runs as fallback when MusicKit-based
  video lookup finds no results. Cross-platform URLs are logged for future
  use when additional service engines are added.

  Service is intentionally generic: returns all discovered platform URLs
  via HashMap, not just Apple Music. Groundwork for future "if unavailable
  on one platform, try another" cross-platform routing.

  New setting: musicbrainz_lookup (default: false). Toggle in Settings >
  Quality > Video Quality. Rate-limited to 1 req/sec per MusicBrainz ToS.
  10 new unit tests for URL classification and struct serialization.

- Enhance MusicBrainz with storefront awareness, ID lookup, AcoustID bridge

Three enhancements to the MusicBrainz discovery service:

  1. Storefront-aware Apple Music URLs: rewrite_apple_music_storefront()
     detects and replaces storefront codes (e.g., /de/ → /gb/) when
     MusicBrainz returns URLs for a different region.

  2. Direct recording-by-ID lookup: lookup_recording_by_id() enables
     MusicBrainz lookups when the recording ID is already known (e.g.,
     from AcoustID), skipping the ISRC search step entirely.

  3. AcoustID → MusicBrainz bridge: the AcoustID lookup now extracts
     MusicBrainz recording IDs from the API response (they were already
     present but not parsed).

  Also refactored relationship parsing into shared parse_recording_relations()
  function used by both ISRC and direct ID lookup paths. 8 new unit tests.

- 3-tier MusicBrainz discovery: URL → ISRC → AcoustID recording ID

Enhanced lookup_videos_for_tracks with a 3-tier priority chain:
  1. Apple Music URL search (most direct — searches MB external links)
  2. ISRC code search (reliable standard identifier)
  3. MusicBrainz recording ID direct lookup (from AcoustID fingerprinting)

  New functions:
  - lookup_recording_by_url() — searches MB for recordings with a specific
    external URL link (e.g., Apple Music song URL)
  - lookup_videos_for_tracks_enhanced() — uses TrackLookupInfo struct
    carrying all three discovery identifiers
  - TrackLookupInfo struct — carries song URL, ISRC, and MB recording ID

  The legacy lookup_videos_for_tracks() still works (converts to
  TrackLookupInfo internally). Each tier only fires if the previous
  tier found no results. Rate limiting enforced between all requests.

- Support non-geographic Apple Music URLs with storefront auto-detection

URLs without a storefront code (e.g., music.apple.com/album/...) are now
  automatically normalized by injecting a storefront based on OS locale
  (or "us" fallback). GAMDL requires a storefront in the URL path for its
  regex to match, but ignores it for API calls (uses cookies/wrapper auth).

  Two-layer approach:
  1. URL normalization at enqueue — normalize_apple_music_url() injects
     storefront before the URL enters the queue or reaches GAMDL
  2. Storefront fallback for enrichment — fetch_album_metadata_with_fallback()
     retries with alternative storefronts (OS locale, "us") when the primary
     returns HTTP 404, handling cross-region shared links

  Also normalizes URLs in queue imports (.meedyadl files) and logs
  normalization events to the activity log.

- Add rich SRT generation from TTML and subtitle embedding

Two new enrichment steps:

  Step 2d - Rich SRT generation (generate_rich_srt, default: true):
  Converts Apple Music TTML to format-rich SRT with HTML-like styling
  tags (<b>, <i>, <u>, <font color="...">). Extracts tts:fontWeight,
  tts:fontStyle, tts:textDecoration, tts:color attributes from both
  inline styles and named style definitions in <head><styling>. Style
  inheritance from <p> to <span> children supported. Background vocals
  (ttm:role="x-bg") wrapped in parentheses. Rich SRT overwrites any
  existing plain SRT since TTML has richer data.

  Step 2e - Subtitle embedding (embed_subtitles, default: false):
  Embeds SRT and WebVTT sidecar content into MP4/M4A/M4V containers
  as freeform atoms (com.apple.iTunes:subtitles-srt/subtitles-vtt).
  Uses existing mp4ameta pattern. Groundwork for multi-service support.

  New service: rich_srt_service.rs with 34 unit tests covering styling,
  colour normalization, timestamps, background vocals, style inheritance,
  named styles, and edge cases.

- Support WebVTT as rich SRT source alongside TTML

Rich SRT generation now uses a dual-source priority chain:
  1. TTML (richest — Apple Music, has tts:* styling attributes)
  2. WebVTT (also supports <b>, <i>, <u>, CSS class tags)

  This enables future services (YouTube/yt-dlp, BBC iPlayer) that provide
  WebVTT with styling to produce rich SRT output. The directory function
  now scans media files and finds matching source sidecars (like WebVTT
  service pattern) instead of scanning for .ttml files directly.

  New functions:
  - webvtt_to_rich_srt() — parses WebVTT cues, preserves SRT-compatible
    tags, strips VTT-only constructs (<c>, <v>, timestamps)
  - clean_vtt_tags() — filters tags by SRT compatibility
  - try_rich_srt_from_ttml/webvtt() — per-source helpers

  15 new unit tests (WebVTT conversion, tag cleaning, edge cases).

- Generate ASS subtitles from TTML and WebVTT with rich styling

New enrichment Step 2f generates ASS (Advanced SubStation Alpha) subtitle
  files from TTML or WebVTT sources with full styling support:

  - Colours: RGB #RRGGBB → ASS BGR &HBBGGRR& conversion
  - Text styling: bold ({\b1}), italic ({\i1}), underline ({\u1})
  - Dynamic positioning: tts:origin → {\pos(x,y)} override tags
  - Background vocals: ttm:role="x-bg" → dedicated "BgVocals" style
    (semi-transparent, italic, slightly smaller font)
  - Named style resolution from <head><styling> definitions
  - Style inheritance from <p> to <span> children

  Source priority: TTML first (richest, with tts:* attributes and
  positioning), then WebVTT (supports <b>, <i>, <u> inline tags).

  WebVTT tags are converted to ASS override equivalents:
    <b>text</b> → {\b1}text{\b0}
    <i>text</i> → {\i1}text{\i0}
  VTT-only tags (<c>, <v>, timestamps) are stripped.

  Reuses TTML style resolution from rich_srt_service via pub(crate)
  shared types and functions (TtmlStyle, resolve_named_styles, etc.).

  New service: ass_subtitle_service.rs with 37 unit tests.
  New setting: generate_ass: bool (default: false, opt-in).
  Toggle in Settings > Lyrics.

- Add verbose activity log toggle for detailed debugging

New `verbose_activity_log` setting (default: false) enables detailed
  [VERBOSE] messages in the Activity Log for issue tracking. When enabled,
  emits sensitive debugging information including full URLs, CLI arguments,
  error classification details, wrapper URLs (unredacted), cookie paths,
  and download settings.

- Parse and embed audioTraits from Apple Music API (#121 Phase 1)

Extract the audioTraits field from Apple Music API track responses
  and write it as metadata tags. This field is returned by default
  (no extend parameter needed) and indicates which audio formats are
  available for each track: lossy-stereo, lossless, hi-res-lossless,
  dolby-atmos, spatial.

- Comprehensive Apple Music metadata extraction, dual-namespace tags, config-driven tag system (tags.toml), and API field audit tool

- Extract all available Apple Music API metadata fields (20 track-level + 11 album-level)
  - Dual-namespace tagging: com.apple.iTunes (player-compatible) + MeedyaMeta (MeedyaDL-branded)
  - Industry standard alternative names: LABEL, COPYRIGHT, COMPILATION, TOTALTRACKS
  - Album scope uses Album* prefix; track scope uses no prefix (default context)
  - Config-driven tag definitions via tags.toml (28 entries) — zero Rust code changes for new tags
  - Tag registry module (tag_registry.rs) with JSON path extraction and value conversion
  - API field audit tool: fetch album, flatten JSON, diff against tags.toml, report unknown fields
  - Audit UI in Settings > Metadata tab (collapsible, requires MusicKit credentials)
  - 35 new tests (25 tag registry + 10 audit service), 551 total Rust tests passing

- Add isBinaural and isDownmix codec identification tags (#119)

Binaural (AAC Binaural, AAC-HE Binaural) and Downmix (AAC Downmix,
  AAC-HE Downmix) codec variants now get identification tags written
  to both com.apple.iTunes and MeedyaMeta namespaces:

  - isBinaural = Y (binaural spatial audio for headphones)
  - isDownmix = Y (stereo downmix of spatial/surround master)

  These codecs produce standard 2-channel AAC indistinguishable from
  regular stereo by audio analysis — codec identity at download time
  is the only way to classify them.

  Tags written in both apply_codec_metadata_tags() (companion downloads)
  and enrich_single_file() (enrichment pipeline Layer 1).


### 🐛 Bug Fixes

- Reorder update check interval options in ascending frequency order

Move "Startup only" from first to last position in the Settings >
  General > Check Interval dropdown. Options now listed from most
  frequent to least frequent: Every hour → Every 6/12/24 hours →
  Startup only.

- Use version tag only as GitHub release title (no app name prefix)

Renamed all 50 existing releases from "MeedyaDL vX.X.X" to just "vX.X.X".
  Updated release.yml releaseName and ARMv7 fallback gh release create
  to use the tag directly. Keeps release page clean and consistent.

- Address GitHub Code Scanning security alerts

Three categories of fixes:

  1. Incomplete URL substring sanitization (Alert #11):
     CookiesTab.tsx used `domain.includes('apple.com')` which could match
     unrelated domains. Now uses exact domain matching:
     `domain === 'apple.com' || domain.endsWith('.apple.com')`

  2. Insecure cookie test fixtures (Alerts #4-10):
     Test cookies that don't specifically verify insecure behavior now set
     `.secure(true)`. The one test that intentionally tests insecure cookies
     (`insecure_cookie_has_false_flag`) is annotated with a comment.

  3. Missing workflow permissions (Alerts #1-2):
     CI workflow now declares `permissions: { contents: read }` following
     the principle of least privilege for the GITHUB_TOKEN.

- Use explicit ARIA string values and update CHANGELOG

- Toggle.tsx: aria-checked now uses "true"/"false" strings instead of
    boolean expression (fixes Edge DevTools axe/aria warning)
  - CookiesTab.tsx: aria-expanded now uses "true"/"false" strings at both
    outer collapsible and per-browser accordion levels
  - CHANGELOG.md: add isBinaural/isDownmix tags and ARIA fix entries


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Add codecs.toml editing guide to Dev_Notes.md

Comprehensive guide for developers on how to add/modify entries in
  codecs.toml: audio codecs, video codecs, lyrics formats, and meta
  codecs. Includes how to find service mapping values from each engine's
  CLI help, practical examples (MP3, unmapped codecs), and when code
  changes are vs aren't required.

- Standing tasks sweep — update Project_Plan and CLAUDE.md

Add codec registry infrastructure and JS obfuscation to Project_Plan
  completed features list. Update CLAUDE.md Key Directories to include
  codec_registry module, template-parser lib, and codec-registry types.

- Add Raspberry Pi GDebi installation note to release pages

Raspberry Pi users may need GDebi to install .deb packages with
  dependencies resolved. Added note to release.yml template and all
  existing releases: sudo apt install gdebi-core && sudo gdebi ...

- Update documentation for WebVTT, MusicBrainz, and v0.6.3+ features

Update all documentation to reflect recent features:
  - CLAUDE.md: 9-stage enrichment pipeline, WebVTT/MusicBrainz services
  - CHANGELOG.md: Full [Unreleased] entries for WebVTT, MusicBrainz 3-tier
    discovery, codec registry, terser, pre-release flag, download links
  - Project_Plan.md: 5 new completed features
  - README.md: WebVTT and MusicBrainz feature bullets + checklist items
  - help/lyrics-and-metadata.md: WebVTT and MusicBrainz help sections,
    updated enrichment stage list (2c, 6b)
  - services/mod.rs: Updated module map and MusicBrainz doc comment

- Link roadmap features to GitHub Issues and organize project tracking

- Created 5 new GitHub Issues for planned/future features:
    #107 (multi-service architecture), #108 (enhanced MusicKit),
    #109 (native SwiftUI), #110 (smart download), #111 (full i18n)
  - Closed #105 (Apple Music support — already fully implemented)
  - Added all open issues (#44, #100-104, #106-111) to GitHub Project
  - Updated README.md roadmap tables with Issue column and links
  - Updated Project_Plan.md roadmap overview with Issue column,
    added issue links to Milestone 8/9/10 headers, added rows for
    Enhanced MusicKit, i18n, remote disable, SwiftUI, crash relay

- Add GitHub Issue tracking as formal standing task

Standing task #4 now requires creating/closing/linking GitHub Issues
  for every task (features, bugs, enhancements, security) and adding
  them to the "MeedyaDL Development" project. Parent/child dependencies
  must be cross-referenced. Follow-up work must get its own issue.

  Updated in both CLAUDE.md (project instructions) and memory/MEMORY.md
  (session persistence).

- Comprehensive documentation update for metadata, subtitles, and tags.toml

- CHANGELOG.md: Add ASS subtitles, verbose logging, comprehensive API metadata,
    dual-namespace tagging, tags.toml, API audit tool, Dependabot entries
  - DEV_NOTES.md: Add tags.toml editing guide (schema, JSON path syntax, value
    types, namespace conventions, step-by-step "Adding a New Tag" section), subtitle
    and lyrics generation section (6-step pipeline, format comparison, embedding atoms)
  - Project_Plan.md: Add 11 recently delivered features to post-release list
  - README.md: Expand Metadata & Extras section with Rich SRT, ASS, subtitle
    embedding, config-driven tags, API audit tool, comprehensive enrichment details
  - help/lyrics-and-metadata.md: Update enrichment pipeline to 12 stages, expand
    API tag table with all 30+ atoms, add tags.toml cross-reference
  - Fix pre-existing pedantic clippy suggestions: needless raw string hashes
    (replaygain_service), map_unwrap_or (gamdl), redundant closures (download_queue)
  - MeedyaManager#11: Mirror issue for subtitle/lyrics format support

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Add complex integration tests for TTML and WebVTT rich SRT conversion

Two new end-to-end tests exercising real-world scenarios:

  - ttml_to_rich_srt_complex_mixed_styling: plain text, italic verse,
    named style (bold+colour), mixed spans with bold+background vocals,
    underline+named colour — verifies all 5 cue types in one test

  - webvtt_to_rich_srt_complex_mixed_tags: plain text, preserved <b>/<i>,
    stripped <c> class tags, stripped inline timestamps, stripped <v> voice
    tags — verifies SRT-compatible tag preservation and VTT-only stripping


### 🔄 CI/CD

- Add npm audit security scanning to CI pipeline

Adds `npm audit --audit-level=high` step to the frontend CI job,
  running after npm ci install. Fails the build only on high/critical
  severity vulnerabilities in npm dependencies.

  Also created GitHub Issues for project recommendations:
  - #112: cargo deny licence scanning for Rust dependencies
  - #113: end-to-end integration tests for enrichment pipeline
  - #114: React component rendering tests for settings tabs
  - #115: dependency freshness checks (npm outdated, cargo outdated)
  - #116: Wiki sync with in-app help documentation

  All issues added to MeedyaDL Development project.

- Add Dependabot version updates for automated dependency freshness

Configures Dependabot to create weekly PRs for semver-compatible
  (minor + patch) updates to both npm and Cargo dependencies. Major
  version jumps are excluded (tracked separately in #117).


### 🧹 Maintenance

- Update dependencies (npm + cargo semver-compatible)

npm updates (8 packages, all semver-compatible):
  - @eslint/js 9.39.2→9.39.3, eslint 9.39.2→9.39.3
  - @sentry/browser 10.40.0→10.42.0
  - @types/react 19.2.13→19.2.14
  - @typescript-eslint/eslint-plugin 8.54.0→8.56.1
  - @typescript-eslint/parser 8.54.0→8.56.1
  - autoprefixer 10.4.24→10.4.27
  - i18next 25.8.10→25.8.13, postcss 8.5.6→8.5.8

  Cargo updates (7 packages):
  - tokio 1.49.0→1.50.0
  - aws-lc-rs 1.16.0→1.16.1, aws-lc-sys 0.37.1→0.38.0
  - getrandom 0.4.1→0.4.2, ipnet 2.11.0→2.12.0
  - minisign-verify 0.2.4→0.2.5

  Major version jumps deferred (tailwindcss 4, vite 7, eslint 10,
  commitlint 20, etc.) — require migration effort.

  All 516 Rust + 231 frontend tests pass. 0 vulnerabilities.

- Add markdownlint ignore for auto-generated and internal files

Exclude CHANGELOG.md (auto-generated by git-cliff) and .claude/
  (internal development context) from markdownlint checks. Also added
  .vscode/settings.json (gitignored) with workspace-level markdownlint
  ignore config and documentation of Edge DevTools false positives.

- **(deps-dev)** Bump vite from 6.4.1 to 7.3.1

Bumps [vite](https://github.com/vitejs/vite/tree/HEAD/packages/vite) from 6.4.1 to 7.3.1.
  - [Release notes](https://github.com/vitejs/vite/releases)
  - [Changelog](https://github.com/vitejs/vite/blob/main/packages/vite/CHANGELOG.md)
  - [Commits](https://github.com/vitejs/vite/commits/v7.3.1/packages/vite)

  ---
  updated-dependencies:
  - dependency-name: vite
    dependency-version: 7.3.1
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump @commitlint/config-conventional

Bumps [@commitlint/config-conventional](https://github.com/conventional-changelog/commitlint/tree/HEAD/@commitlint/config-conventional) from 19.8.1 to 20.4.3.
  - [Release notes](https://github.com/conventional-changelog/commitlint/releases)
  - [Changelog](https://github.com/conventional-changelog/commitlint/blob/master/@commitlint/config-conventional/CHANGELOG.md)
  - [Commits](https://github.com/conventional-changelog/commitlint/commits/v20.4.3/@commitlint/config-conventional)

  ---
  updated-dependencies:
  - dependency-name: "@commitlint/config-conventional"
    dependency-version: 20.4.3
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump eslint-plugin-react-hooks from 5.2.0 to 7.0.1

Bumps [eslint-plugin-react-hooks](https://github.com/facebook/react/tree/HEAD/packages/eslint-plugin-react-hooks) from 5.2.0 to 7.0.1.
  - [Release notes](https://github.com/facebook/react/releases)
  - [Changelog](https://github.com/facebook/react/blob/main/packages/eslint-plugin-react-hooks/CHANGELOG.md)
  - [Commits](https://github.com/facebook/react/commits/HEAD/packages/eslint-plugin-react-hooks)

  ---
  updated-dependencies:
  - dependency-name: eslint-plugin-react-hooks
    dependency-version: 7.0.1
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...

- **(deps-dev)** Bump jsdom from 25.0.1 to 28.1.0

Bumps [jsdom](https://github.com/jsdom/jsdom) from 25.0.1 to 28.1.0.
  - [Release notes](https://github.com/jsdom/jsdom/releases)
  - [Changelog](https://github.com/jsdom/jsdom/blob/main/Changelog.md)
  - [Commits](https://github.com/jsdom/jsdom/compare/25.0.1...28.1.0)

  ---
  updated-dependencies:
  - dependency-name: jsdom
    dependency-version: 28.1.0
    dependency-type: direct:development
    update-type: version-update:semver-major
  ...


## [0.6.4] - 2026-03-03

### 🐛 Bug Fixes

- Use --song-codec-priority instead of removed --song-codec flag

GAMDL 2.9.1 removed the --song-codec flag entirely, causing ALL
  companion tier downloads and fallback retries to fail with:
  "Error: No such option: --song-codec Did you mean --song-codec-priority?"

- Allow companion lyrics formats when Enhanced LRC is enabled

When Enhanced Lyrics (Word-by-Word Sync) was on, the Synced Lyrics
  Formats checkboxes were completely disabled, preventing selection of
  LRC and SRT as companion formats. Now TTML remains locked as the
  primary format (required for word-level timing data) but LRC and SRT
  checkboxes are enabled for companion downloads. The description text
  adapts to explain the behavior.

  Also updates handleFormatToggle() to always keep TTML as primary and
  route other selected formats to companion_lyrics_formats when
  enhanced_lrc is active.

- File picker Browse button now starts at the currently configured path

The native file/directory picker dialog was not setting defaultPath,
  so it opened at the OS-remembered last-used directory (which could be
  wrong after exporting an activity log to a different folder). Now passes
  the current value as defaultPath so Browse always starts at the
  configured path (e.g., the output directory in Settings > General).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Comprehensive documentation update for v0.6.2-v0.6.3 features

Update all documentation to reflect recent changes:

  - help/lyrics-and-metadata.md: 7-stage enrichment pipeline, lyrics
    format fallback chain, Enhanced LRC companion format selection
  - help/troubleshooting.md: FUSE mount/cloud mount hang documentation
  - help/quality-settings.md: native priority codec suffix behavior,
    --song-codec-priority technical note
  - Project_Plan.md: v0.6.2 and v0.6.3 completed features (7 items)
  - Dev_Notes.md: GAMDL 2.9.1 CLI flag changes, enrichment blocking
    I/O fix documentation
  - README.md: lyrics format fallback feature bullet
  - CLAUDE.md: 7-stage enrichment pipeline with lyrics fallback (Step 2b)

  Also saves standing tasks to .claude/ memory for session persistence.

- Update CHANGELOG.md [skip ci]

## [0.6.3] - 2026-03-02

### ✨ Features

- Add lyrics format fallback chain for incomplete lyrics coverage

When the primary lyrics format (TTML) doesn't produce lyrics for all
  tracks, automatically retries with fallback formats. Content-type-aware
  ordering: Audio (TTML → LRC → SRT), Video (TTML → SRT → LRC). Each
  fallback uses --synced-lyrics-only to avoid re-downloading media. Chain
  stops when lyrics coverage matches media file count.

  New setting: lyrics_fallback_enabled (default: true). Toggle in
  Settings > Lyrics. Integrated as enrichment Step 2b between Enhanced
  LRC conversion and Animated Artwork download.

- Add per-endpoint logging to pre-flight internet connectivity check

Each endpoint tested during the multi-tier internet check now logs its
  result with the endpoint name, URL, HTTP status (or failure reason), and
  response time. Tier progression is logged too (Tier 1 pass/fail, Tier 2
  skipped/tested). Example output:

    Pre-flight internet check: Cloudflare (https://1.1.1.1/) → reachable (200 OK, 12ms)
    Pre-flight internet check: Google (google.com) → skipped (Cloudflare passed)
    Pre-flight internet check: Apple Music API → reachable (401 Unauthorized, 45ms)

  Helps diagnose connectivity issues from log files without needing to
  reproduce the problem.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.2] - 2026-03-02

### 🐛 Bug Fixes

- Prevent UI stall on FUSE mounts and fix wrong codec suffix with native priority

Bug 1 — UI stall: The enrichment pipeline (Steps 1-5) called blocking
  mp4ameta Tag I/O directly on tokio async worker threads. On slow FUSE
  mounts (CloudMounter, NFS), this starved the runtime, freezing the UI
  for minutes. Fix: wrap Tag::read/write in spawn_blocking() in 4
  services (metadata, lyrics, AcoustID, ReplayGain). Change enhanced
  lyrics from async fn to fn (had zero .await calls). Add yield_now()
  between all 6 enrichment steps.

  Bug 2 — Wrong codec suffix: apply_codec_suffix() used the REQUESTED
  codec, not the ACTUAL one GAMDL selected via native priority chain.
  Files named [Dolby Atmos] could contain AAC. Fix: skip suffix when
  native priority is active (actual codec unknown until GAMDL finishes).
  Force all companion tiers to use suffixes via new force_all_suffixes
  parameter, preventing filename collisions with the primary's clean
  filenames.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.6.1] - 2026-03-01

### ✨ Features

- Enhance application stability and security

- Pin all release-critical GitHub Actions to immutable commit SHAs to prevent supply chain attacks.
  - Implement SHA-256 checksum verification for dependency downloads to ensure integrity.
  - Add graceful shutdown signal for background tasks to prevent orphaned processes on app exit.
  - Improve error handling in various components, including better logging and user notifications for failures.
  - Optimize regex usage in Apple Music URL parsing by using static instances to avoid recompilation.
  - Introduce log file cleanup for entries older than 7 days to manage disk space.
  - Enhance pre-download validation with multi-provider internet connectivity checks and cookie validation.
  - Update documentation and project plans to reflect new features and improvements.

- Expand activity log coverage and enhance logging throughout the app

- Expanded Activity Log to include app-wide events such as update checks, dependency installs, settings saves, cookie imports, queue operations, login window events, pre-flight check results, and app startup messages.
  - Implemented logging for cookie imports, Python and GAMDL installations, dependency installations, and queue operations.
  - Added system-level logging for app startup and pre-flight checks.
  - Introduced utility functions for emitting activity log events, centralizing logging logic.
  - Updated Activity Log component to display both download-specific and system-level events, improving user visibility into application activity.

- Add custom companion downloads and multi-select artist auto-select

Add Custom Companion mode (6th CompanionMode variant) with multi-select
  codec checkboxes, letting users pick exactly which audio formats to
  download as companions. Add multi-select artist auto-select that creates
  N separate queue items for artist URLs when multiple content types are
  selected. New CheckboxGroup<T> reusable component. Bump version to 0.6.0.

- Embed AcoustID API key in release builds for seamless fingerprinting
- Implement TemplateBuilder component for interactive GAMDL template editing
- Add music video companion downloads and visual template builder

Add music video companion downloads as enrichment Step 6: when enabled
  and MusicKit credentials are configured, queries Apple Music API for
  music video relationships after each audio download. Tracks with music
  videos get companion GAMDL downloads using video quality settings.
  Toggle in Settings > Quality > Video Quality, gated behind MusicKit
  credentials. Deduplicated by video ID.

  Add visual TemplateBuilder component replacing 7 plain text inputs in
  Settings > Templates with interactive chip/pill UI. Variables selected
  from dropdown menu; raw-edit toggle for power users; live preview.

  Update all documentation (CHANGELOG, Dev_Notes, Project_Plan, README,
  help files, CLAUDE.md, GitHub Wiki Features page). Close GitHub issue
  #81. Enhance inline code comments on new Rust functions.


### 🐛 Bug Fixes

- Update documentation and code references to use '4K UHD' terminology
- Correct AcousticID → AcoustID spelling and upgrade vulnerable dependencies

Fix incorrect "AcousticID" spelling to "AcoustID" across 86 instances
  in 15 files (comments, UI text, docs, error messages). Upgrade
  jsonwebtoken from v9 to v10.3.0 (fixes CVE type confusion auth bypass,
  uses aws_lc_rs crypto backend). Update rollup to 4.59.0 (fixes path
  traversal CVE). Dismiss glib 0.18 alert (transitive via Tauri GTK
  stack, not directly used).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.8] - 2026-03-01

### ✨ Features

- Add output path writability check before downloads

- Implemented `check_output_path_before_download` command to verify that the output directory is writable, catching issues like disconnected cloud mounts, full disks, and permission errors.
  - Integrated the new check into the download process in `DownloadForm.tsx`, ensuring downloads are only queued if the output path is accessible.
  - Updated settings model to include `update_check_interval_hours`, allowing users to specify how often to check for updates while the app is running.
  - Added UI components in the settings to configure the update check interval, visible only when auto-check for updates is enabled.
  - Enhanced logging and error handling in the download queue to provide more informative messages regarding fallback attempts and network errors.
  - Updated tests to cover new functionality and ensure existing features remain intact.

- Enhance internet connectivity check with multi-provider, multi-tier approach

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.7] - 2026-02-28

### 🐛 Bug Fixes

- Improve error handling for Python exceptions and traceback frames

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.6] - 2026-02-28

### ✨ Features

- Enhance toast notifications with deduplication and clearing for preflight checks
- Add auto-retry without wrapper option for failed downloads
- Add pre-download connectivity check, toast notification deduplication, auto-retry without wrapper, and network error report suppression

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.5] - 2026-02-28

### ✨ Features

- Add pre-download internet connectivity check to prevent queuing downloads without internet
- Implement pre-download checks for internet connectivity and cookie validation, update queue processing behavior

### 🐛 Bug Fixes

- **(docs)** Add wrapper connectivity troubleshooting guide for remote and Docker setups

Reclassified from docs: to fix: to trigger a patch release. The wrapper
  troubleshooting content (Help > Wrapper, README) addresses user-facing
  issues with diagnosing wrapper connectivity failures on remote devices.

- Improve audio format fallback so downloads try all available formats

When your preferred audio format (like Dolby Atmos) isn't available for
  a track, MeedyaDL now reliably tries the next format in your fallback
  list instead of giving up after the first failure.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Enhance README and HelpViewer with wrapper authentication details and connectivity troubleshooting
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Unify error reporting for crashes and download failures, update UI and documentation

## [0.5.4] - 2026-02-27

### ✨ Features

- Add cookie validation before download to enhance user feedback

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.3] - 2026-02-27

### ✨ Features

- **(crash-reports)** Implement crash reporting system with Sentry integration

- Added `CrashReport` model to represent crash/error reports.
  - Created `crash_report_service` for managing crash report files.
  - Implemented IPC commands for listing, retrieving, deleting, exporting, and logging frontend errors.
  - Integrated `tracing` for structured logging and added support for Sentry error tracking.
  - Updated application settings to include `sentry_enabled` for opt-in telemetry.
  - Enhanced frontend error handling to persist errors to the Rust crash report system.
  - Added UI toggle in settings for enabling/disabling anonymous crash reporting.
  - Implemented automatic cleanup of old crash reports older than 30 days.

- Implement GitHub Issues crash reporting system

- Added a new crash reporting feature that allows users to report crashes directly to GitHub Issues from the app.
  - Introduced `CrashReportSection` and `CrashReportDialog` components for managing crash reports and user consent.
  - Implemented `get_github_issue_url` command to generate pre-filled GitHub issue URLs with crash report data.
  - Updated documentation to reflect the new crash reporting functionality and usage instructions.
  - Enhanced localization for crash reporting features in English, German, and French.
  - Added IPC commands for listing, deleting, and exporting crash reports.


### 🐛 Bug Fixes

- Resolve startup crash caused by missing Tokio runtime in setup

The app was crashing on launch with "there is no reactor running, must
  be called from the context of a Tokio 1.x runtime" because the queue
  recovery code assumed a Tokio runtime was active during the setup
  closure. On macOS, this closure runs inside the `did_finish_launching`
  callback where the Tokio runtime isn't registered as "current".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update documentation for startup crash fix and external link handling

- CHANGELOG.md: Add entries for both bug fixes in [Unreleased] section
  - CLAUDE.md: Update queue persistence convention (blocking_lock, async
    runtime spawn) and Updates page convention (shell plugin for links)
  - Project_Plan.md: Note external link handling on Updates page entry
  - help/troubleshooting.md: Add "Crash on Launch (Queue Recovery Panic)"
    section with cause, fix version, and workaround for older versions

- Update CHANGELOG.md [skip ci]

## [0.5.2] - 2026-02-27

### 🐛 Bug Fixes

- Update handleViewRelease to use Tauri shell plugin for opening URLs

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.5.1] - 2026-02-27

### ✨ Features

- Add pre-flight health checks and retry-without-wrapper

Add pre-flight health checks that run before queue processing begins:
  - Internet connectivity check (pings apple.com with 5s timeout)
  - Cookie validation (checks for valid, non-expired Apple Music cookies)
  - Wrapper health check (pings wrapper URL when enabled)

  Warnings are emitted as persistent toasts — non-blocking, queue proceeds
  regardless. Checks run once per batch with a 60-second cooldown.

  Add "Retry without Wrapper" action for failed downloads that used wrapper
  authentication, allowing users to fall back to cookie-based auth:
  - Pill button below error message + right-click context menu option
  - New retry_download_without_wrapper Tauri command
  - used_wrapper field on QueueItemStatus for conditional UI display

  Also fixes LyricsTab test failures (enhanced_lrc default + /LRC/ regex)
  and bumps version to 0.5.0.


### 🐛 Bug Fixes

- Add missing used_wrapper field to Rust test initializers

cargo test compiles #[cfg(test)] modules that cargo check skips,
  causing CI to fail with missing field errors in 3 QueueItemStatus
  serde roundtrip tests.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.4.1] - 2026-02-27

### ✨ Features

- Add Enhanced LRC with word-by-word synchronized lyrics

Convert Apple Music TTML lyrics to Enhanced LRC format with inline
  word-level timestamps (<mm:ss.xx>) for karaoke-style highlighting.

  - New enhanced_lyrics_service.rs: TTML XML parser (roxmltree), word
    timestamp extraction, Enhanced LRC generation, M4A/M4V embedding
  - New enhanced_lrc setting (default: true) with TTML as default
    primary lyrics format and SRT as companion format
  - merge_options() Layer 4: forces TTML when Enhanced LRC is enabled
  - Enrichment pipeline Step 2: TTML → Enhanced LRC conversion
  - Frontend: Enhanced Lyrics toggle in Settings > Lyrics tab
  - Falls back to standard line-level LRC for songs without word data
  - Handles both iTunes namespace URIs and background vocals
  - 20 unit tests, all 339 tests passing, clippy clean
  - Version bump to v0.4.0


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧪 Testing

- Update LyricsTab tests to use regex for format labels

## [0.3.33] - 2026-02-26

### ✨ Features

- **(download)** Implement partial-success recovery for codec errors
- **(download)** Implement companion and lyrics downloads as background tasks
- **(activity-log)** Implement export functionality and wrapper connection test

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.32] - 2026-02-26

### ✨ Features

- Enhance queue persistence to include failed downloads for manual retry

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.31] - 2026-02-26

### ✨ Features

- Enhance persistence of download queue items to include failed states

### 🐛 Bug Fixes

- Update remux mode flag and clean up unused options in GamdlOptions

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.30] - 2026-02-26

### ✨ Features

- Add new app icon variants and previews

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.29] - 2026-02-26

### ✨ Features

- Update app icons and logos

- Updated the MeedyaDL logo in both light and dark variants (SVG and PNG formats) with a new design and color scheme.
  - Added a new application icon (app-icon.svg) that combines a clapperboard, download arrow, and music note.
  - Updated the Sidebar component to use the new app icon instead of a placeholder.
  - Updated various icon sizes for Android and iOS platforms to reflect the new branding.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.28] - 2026-02-26

### 🐛 Bug Fixes

- Add use import to doc test for is_version_at_least

The doc test example needed `use meedyadl::services::gamdl_service::is_version_at_least`
  to resolve the function in cargo test --doc (which runs examples as standalone crates).


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.27] - 2026-02-26

### ✨ Features

- Integrate GAMDL v2.9.1 native codec priority, artist auto-select, and Apple Music Classical URLs

- Add version-aware codec fallback: GAMDL >= 2.9.1 uses native --song-codec-priority
    (all codecs tried in one process); older versions fall back to MeedyaDL's try_fallback system
  - Add ArtistAutoSelect enum (7 variants) with CLI arg and config.ini support
  - Add classical.apple.com URL support in frontend parser and backend regex patterns
  - Write dual config.ini keys (song_codec + song_codec_priority) for cross-version compatibility
  - Cache GAMDL version in DownloadQueue to avoid repeated pip show calls
  - Skip try_fallback() on both success and error paths when native priority was used
  - Clear song_codec_priority on companion and lyrics companion downloads (single-codec mode)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.26] - 2026-02-25

### ✨ Features

- Add platform asset validation for GitHub releases
- Refactor UpdateBanner integration in MainLayout and App components

### 🐛 Bug Fixes

- Improve asset manifest check in has_platform_assets function

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.25] - 2026-02-25

### ✨ Features

- Add sys-locale dependency for localized Apple Music storefront detection

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.24] - 2026-02-25

### 🐛 Bug Fixes

- Sign bundled macOS dependencies with Developer ID for notarization [skip ci]

Apple's notarization service inspects all Mach-O binaries inside the
  .app bundle, including those inside tar.gz archives. Third-party binaries
  (Python, Perl, FFmpeg, MP4Box libs, etc.) from bundled-deps must be
  re-signed with our Developer ID certificate before Tauri packages them.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update HelpViewer.tsx with links for cookie export and Apple Developer keys
- Update HelpViewer.tsx to clarify MusicKit key creation instructions
- Update CHANGELOG.md [skip ci]
- Update DEV_NOTES and CHANGELOG to document macOS codesign timestamp workaround and future MusicKit integration
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update dependencies and add new icons

- Updated `vitest` from `^2.1.8` to `^4.0.18` in `package.json`.
  - Added `sharp` dependency with version `^0.34.5`.
  - Updated various icon files in `src-tauri/icons` for different resolutions and platforms, including:
    - New icons for Android adaptive launcher and various mipmap resolutions.
    - New iOS app icons for multiple sizes.
    - Updated existing icon files for various resolutions.


## [0.3.23] - 2026-02-22

### ✨ Features

- Enhance link handling in HelpViewer for internal and external navigation
- Implement custom macOS menu and update About section to display app version

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Simplify TitleBar component to return null for all platforms

### 🧹 Maintenance

- Update milestone versions in project documentation and roadmap

## [0.3.22] - 2026-02-18

### ✨ Features

- Add multi-format lyrics support with companion downloads and update settings

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update quality settings and codec reliability information in documentation and UI
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.21] - 2026-02-18

### 🐛 Bug Fixes

- **(ToolsTab)** Install only missing required tools and update UI for optional tools

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.20] - 2026-02-17

### ✨ Features

- Enhance download error handling and output processing

- Introduced codec and I/O error recovery strategies in process_queue.
  - Added ANSI escape code stripping for cleaner Activity Log output.
  - Implemented new utility functions to classify codec and I/O errors.
  - Updated tests to cover new error classification logic.

- Reorganize settings tabs and enhance tool management functionality

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.19] - 2026-02-17

### 🐛 Bug Fixes

- Update default cover format to JPEG to prevent crashes in GAMDL 2.8.4; add file opening functionality in QueueItem component

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.18] - 2026-02-17

### ✨ Features

- Add non-fatal warnings to download items and update UI to display them

### 🐛 Bug Fixes

- Add text selection capability to ActivityLog component

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.17] - 2026-02-16

### ✨ Features

- **(i18n)** Add internationalization support with language detection and translations

- Added i18next and react-i18next for internationalization.
  - Implemented language detection and dynamic loading of translation files.
  - Created translation files for English, German, and French.
  - Updated AppSettings to include a UI language setting.
  - Enhanced settings UI to allow users to select their preferred language.
  - Introduced UpdatesPage component to display detailed update information with release notes.
  - Modified UpdateBanner to link to the UpdatesPage for more details.
  - Updated Sidebar navigation to include an Updates section.
  - Adjusted update checking logic to handle new update structures.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🔧 Refactoring

- Update status color classes and add animated artwork help content

## [0.3.16] - 2026-02-16

### 🐛 Bug Fixes

- **(config_service)** Improve settings loading and sync to GAMDL config.ini

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.15] - 2026-02-16

### ✨ Features

- Add Activity Log component for live subprocess output and update download queue behavior

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.14] - 2026-02-16

### 🐛 Bug Fixes

- Settings interpretation, affecting downloading ability

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.13] - 2026-02-16

### ✨ Features

- Integrate embedded Chromaprint for AcousticID fingerprinting

- Replace external fpcalc dependency with the embedded rusty-chromaprint library for generating Chromaprint audio fingerprints.
  - Update documentation and comments to reflect the removal of external dependencies.
  - Modify settings and UI components to indicate the new fingerprinting method.
  - Implement fingerprint generation using Symphonia for audio decoding.
  - Enhance error handling for Python exceptions in the download queue process.
  - Add manual update check functionality in the settings UI.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.12] - 2026-02-16

### ✨ Features

- Implement manual queue processing and add auto-start settings
- Add temp directory setting and auto-start queue functionality

### 🐛 Bug Fixes

- Temo folder bug fix

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.11] - 2026-02-16

### ✨ Features

- Add ReplayGain analysis and AcousticID fingerprinting services

- Introduced `replaygain_service` for analyzing audio loudness using FFmpeg's EBU R128 filter, writing non-destructive ReplayGain metadata tags.
  - Added `acoustid_service` for generating Chromaprint audio fingerprints and looking up AcousticID identifiers.
  - Updated `metadata_tag_service` to include new metadata enrichment features.
  - Enhanced `apple_music_api` for improved metadata retrieval from MusicKit.
  - Added new settings tab for metadata enrichment options, including toggles for AcousticID and ReplayGain.
  - Updated Zustand store to manage new settings for AcousticID and ReplayGain.
  - Added unit tests for new features and ensured existing tests cover new functionality.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Enhance quality settings recommendations for audio codecs
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.10] - 2026-02-15

### 🐛 Bug Fixes

- Fixed build generation bugs

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.9] - 2026-02-15

### 🐛 Bug Fixes

- **(updater)** Update public key for the updater plugin

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.8] - 2026-02-15

### ✨ Features

- Add developer notes and update tauri configuration for updater plugin

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.7] - 2026-02-15

### ✨ Features

- Enhance audio codec handling and add help button for contextual assistance

- Added support for Dolby Digital (AC3) codec suffix in download queue.
  - Introduced new companion mode for Atmos to download all formats (AC3, ALAC, AAC).
  - Updated setup wizard to skip if dependencies are missing but setup has been completed.
  - Implemented HelpButton component for contextual help in Input, Select, and Toggle components.
  - Enhanced various settings tabs with help topics for better user guidance.
  - Improved validation and user feedback for cookie settings and sign-in processes.
  - Updated application branding from GAMDL to MeedyaDL in the sidebar and status bar.
  - Fetched application version dynamically from Tauri configuration.
  - Added setup_completed flag to settings store for persistent setup state.

- Add updater functionality for app updates with pre-release support

- Introduced updater permission set in macOS schema for frontend access.
  - Implemented `download_and_install_app_update` command to handle app updates.
  - Enhanced `check_all_updates` to include pre-release versions based on user settings.
  - Updated settings model to allow toggling of pre-release version checks.
  - Modified update checker to query GitHub Releases for both stable and pre-release versions.
  - Added UI components for downloading and installing updates, including progress tracking.
  - Integrated event listeners for real-time download progress updates in the frontend.
  - Updated settings UI to include a toggle for pre-release version checks.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.6] - 2026-02-15

### ✨ Features

- Add FallbackChainList component for reorderable priority lists

- Introduced a new generic component, FallbackChainList, for managing reorderable lists with up/down buttons.
  - Updated FallbackTab and QualityTab to utilize FallbackChainList for audio/video fallback chains and video codec priority respectively.
  - Enhanced type definitions for video codecs and added corresponding labels for UI representation.
  - Added support for displaying the source of installed tools in the DependenciesStep component.
  - Created tool-versions.toml to define minimum version requirements for external tools.
  - Added settings.json for permission configurations.


### 🐛 Bug Fixes

- Improve error logging in download_tool_with_fallback function

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.5] - 2026-02-15

### ✨ Features

- Release 0.3.5 with macOS signing validation and updated dependencies

- Added validation for required Apple signing secrets in the release workflow to prevent publishing unsigned binaries.
  - Updated version to 0.3.5 across various files including package.json, Cargo.toml, and tauri.conf.json.
  - Introduced Entitlements.plist for macOS hardened runtime permissions.
  - Enhanced Help documentation with a disclaimer regarding third-party dependencies.
  - Updated Tailwind CSS configuration to include typography plugin for improved styling.


### 🐛 Bug Fixes

- Add release-please version annotations for auto-managed docs [skip ci]

Added x-release-please-version markers to README.md (version badge,
  roadmap heading) and Project_Plan.md (version header). Registered both
  as generic extra-files in release-please-config.json so version numbers
  are updated automatically in Release Please PRs.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.3.4] - 2026-02-14

### 🐛 Bug Fixes

- Documentation

### 📚 Documentation

- Update CHANGELOG.md [skip ci]

## [0.3.3] - 2026-02-14

### 📚 Documentation

- Update changelog and docs with CI/workflow fixes [skip ci]

Document the release-please state fix, Linux ARM cross-compilation
  apt fix, release workflow manual dispatch with tag input, Windows
  PowerShell shell fix, and git remote URL update.

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update version to 0.3.2 and enhance documentation
- Bump version to 0.3.3 and update changelog with bug fixes and changes

## [0.3.1] - 2026-02-14

### ✨ Features

- Add metadata tagging service for M4A files

- Implemented `metadata_tag_service.rs` to inject custom codec metadata tags into downloaded M4A files.
  - Added tagging for ALAC (`isLossless = Y`) and Dolby Atmos (`SpatialType = Dolby Atmos`) in both Apple iTunes and MeedyaMeta namespaces.
  - Updated `mod.rs` to include the new metadata tagging service.
  - Bumped version to 0.2.1 in `tauri.conf.json`.
  - Enhanced `DownloadForm.tsx` to support new codec and video resolution types.
  - Introduced "Embed Lyrics and Keep Sidecar" toggle in `LyricsTab.tsx` for better lyrics management.
  - Added companion download mode settings in `QualityTab.tsx` to control automatic multi-format downloads.
  - Updated settings store to include new settings for companion mode and lyrics embedding.
  - Expanded type definitions in `index.ts` to include `CompanionMode` and associated labels.
  - Updated tests in `settingsStore.test.ts` to reflect new default settings.

- Implement queue persistence and export/import functionality

- Added queue persistence to save the download queue to disk after every mutation, enabling crash recovery.
  - Introduced export/import features for the download queue using a `.meedyadl` file format, allowing users to transfer their queue between devices.
  - Updated relevant documentation and user interface to reflect new features.
  - Enhanced the download queue management with improved state handling and user notifications.

- Enhance workflows with manual dispatch and update changelog for queue features
- Update project documentation with planned service integrations and milestones for Spotify, YouTube, and BBC iPlayer
- Add multi-track muxing feature to project plan and README
- Implement hidden animated artwork files feature with OS-level hiding options

### 🐛 Bug Fixes

- Update release-please branch reference to match actual branch naming
- Restrict default apt sources to amd64 for ARM cross-compilation [skip ci]

Ubuntu 24.04's default sources (security.ubuntu.com, archive.ubuntu.com)
  don't host ARM packages. When dpkg --add-architecture adds arm64/armhf,
  apt-get update tries to fetch ARM indices from these mirrors and gets
  404 errors, causing the build to fail with exit code 100.

  Fix by adding Architectures: amd64 to the default deb822 sources file
  before adding the ARM ports repository. This ensures ARM packages are
  only fetched from ports.ubuntu.com.

- Support manual dispatch in release workflow with tag input [skip ci]

When triggered via workflow_dispatch, github.ref_name resolves to the
  branch name (e.g., "main") instead of a tag. This caused tauri-action
  to try creating a release with tag "main", which failed with
  "Resource not accessible by integration".

  Fix by adding a required 'tag' input for workflow_dispatch and resolving
  the effective tag name in a dedicated step. The checkout also uses the
  tag ref to ensure the correct code version is built.

- Use bash shell for tag resolution step on Windows runners [skip ci]

Windows runners default to PowerShell which can't parse bash syntax
  (if [ -n ... ]). Adding shell: bash ensures the step works on all
  platforms via Git Bash.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Add temporary PAT diagnostic workflow [skip ci]

Temporary workflow to verify RELEASE_PAT permissions.
  Run via: gh workflow run "Check PAT" --ref main
  Delete after verification.

- Add workflow_dispatch to release workflow [skip ci]

Allow manual trigger for re-running builds when tag push events
  are missed (e.g., after billing blocks or tag re-pushes).

- Remove PAT diagnostic workflow [skip ci]

RELEASE_PAT verified working — the original failure was caused by
  billing/spending limit, not token permissions.


## [0.1.4] - 2026-02-13

### ✨ Features

- Add browser cookie extraction service and auto-import functionality

- Introduced `cookie_service` module for extracting Apple Music cookies from installed browsers.
  - Implemented auto-import feature in `CookiesTab` and `CookiesStep` components, allowing users to extract cookies with a single click.
  - Added platform-specific handling for macOS (Keychain access and Full Disk Access for Safari).
  - Enhanced user interface with loading indicators, error handling, and validation results for cookie imports.
  - Updated TypeScript types to support new cookie import functionalities, including `DetectedBrowser` and `CookieImportResult`.
  - Refactored existing components to accommodate the new auto-import feature and improve user experience.

- Add embedded Apple Music login window service and UI integration

- Introduced `login_window_service` to manage Apple Music authentication via an embedded webview.
  - Updated `CookiesTab` and `CookiesStep` components to support direct login, including event handling for cookie extraction.
  - Enhanced user experience with loading states and manual extraction options.
  - Added Tauri commands for opening, extracting cookies from, and closing the login window.

- Add support for fetching extra metadata tags and update cover size to max resolution
- Add animated artwork download service for Apple Music

- Implemented `animated_artwork_service` to download animated cover art (motion artwork) from Apple Music's catalog API.
  - Added functionality to parse Apple Music URLs, generate MusicKit Developer Tokens, and download HLS streams using FFmpeg.
  - Integrated animated artwork download into the download queue process, allowing for background downloading after album downloads.
  - Updated settings UI to include options for enabling animated artwork downloads and entering MusicKit credentials (Team ID, Key ID, and private key).
  - Enhanced settings store to manage new animated artwork settings and added corresponding TypeScript types.
  - Added unit tests for URL parsing and JWT generation related to animated artwork functionality.


### 🐛 Bug Fixes

- Enhance error handling and improve cookie import feedback

### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Update version to 0.1.3 in Cargo.lock and enhance project documentation

## [0.1.3] - 2026-02-12

### 🐛 Bug Fixes

- Resolve blank screen on macOS/Windows release builds

Fix React infinite re-render loop (error #185) that caused the UI to
  flash briefly then go blank in production builds. Three root causes:

  1. UpdateBanner: Zustand selector called getActiveUpdates() which uses
     .filter(), creating a new array reference on every store change.
     Zustand's Object.is() equality check always saw a new reference,
     triggering cascading re-renders. Fixed by subscribing to raw data
     (lastResult, dismissed) and deriving via useMemo.

  2. Sidebar: Subscribed to isReady function reference (always stable)
     instead of actual dependency state. The status dot never updated.
     Fixed by subscribing to python/gamdl status objects directly.

  3. App.tsx: Subscribed to entire settings object, causing full subtree
     re-renders on any settings change. Narrowed to sidebar_collapsed.
     Also replaced reactive isReady subscription with imperative
     getState() check in initialization effect.

  Additional changes:
  - Add CSP connect-src for Tauri IPC (ipc: http://ipc.localhost)
  - Add ErrorBoundary to main.tsx for visible crash diagnostics
  - Add Vite build config (target, envPrefix) per Tauri 2.0 guide
  - Enable devtools Cargo feature for WebView inspection
  - Open DevTools automatically in debug builds
  - Simplify Windows release: drop x86, produce only NSIS .exe (no .msi)


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.1.2] - 2026-02-12

### ✨ Features

- Integrate release-please for automated release PRs

Add Google's release-please to automatically create Release PRs when
  conventional commits land on main. When merged, the PR creates a tag
  that triggers the existing 7-platform release build. git-cliff continues
  to own CHANGELOG.md (release-please has skip-changelog: true). The
  manual version-bump workflow is preserved as an override for non-standard
  releases.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Add macOS Gatekeeper fix to release notes

Unsigned apps trigger macOS Gatekeeper's "damaged" warning. Add
  instructions to run xattr -cr to remove the quarantine flag.

- Update CHANGELOG.md [skip ci]

### 🧹 Maintenance

- Add auth/ to .gitignore to prevent secret leaks

## [0.1.1] - 2026-02-11

### ✨ Features

- Add release automation and expand to 7 platform targets

Add one-command release automation via Version Bump workflow
  (workflow_dispatch) that bumps versions across all source files,
  commits, tags, and triggers the release build. Expand the release
  build matrix from 3 to 7 platform targets: macOS ARM64, Windows
  x64/x86/ARM64, Linux x64/ARM64/ARMv7 (Raspberry Pi).


### 🐛 Bug Fixes

- Make usePlatform fallback test deterministic across CI runners

Mock navigator.userAgent with a known Windows UA string instead of
  relying on the host platform's default jsdom userAgent. This fixes
  the test failure on Ubuntu runners where the userAgent contains
  "linux" instead of "darwin".


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

## [0.1.0] - 2026-02-11

### ✨ Features

- Initialize GAMDL GUI application with Tauri and React

- Add Tauri configuration file for application settings and build options.
  - Create main application component with platform detection and theme loading.
  - Implement custom hook for platform detection using Tauri's OS plugin.
  - Set up entry point for React application and global styles with Tailwind CSS.
  - Define base and platform-specific themes for macOS, Windows, and Linux.
  - Configure Tailwind CSS for platform-adaptive design tokens and styles.
  - Remove legacy test files and Python dependencies.
  - Add TypeScript configuration for Vite and Node environments.
  - Set up Vite configuration for React and Tauri integration.

- Add setup wizard components and state management

- Implement WelcomeStep component for the setup wizard, providing an introduction and overview of the setup process.
  - Create tauri-commands.ts for type-safe IPC calls to the Rust backend, covering system commands, dependency management, settings, downloads, and credential storage.
  - Introduce url-parser.ts to parse Apple Music URLs and detect content types.
  - Establish dependencyStore.ts to manage the installation status of Python, GAMDL, and external tools.
  - Create downloadStore.ts to handle download queue management, URL validation, and progress tracking.
  - Implement settingsStore.ts for managing application settings with load/save operations.
  - Add setupStore.ts to manage the setup wizard flow and completion status.
  - Introduce uiStore.ts for transient UI state management, including page navigation and toast notifications.
  - Update globals.css to include keyframe animations for UI components.
  - Define TypeScript types in index.ts to ensure type safety across the application, mirroring Rust backend models.

- Enhance CookiesTab with detailed browser export instructions and validation feedback

- Added step-by-step instructions for exporting cookies from various browsers (Chrome, Firefox, Edge, Safari).
  - Implemented a status badge to indicate the current cookie state (valid, invalid, expired).
  - Introduced a warning banner for cookie expiry with estimated days remaining.
  - Enhanced validation results display to include detected domains and additional warnings.
  - Improved user experience with a "Copy Cookie Path" button and loading states for validation.
  - Updated tauri-commands to support new download management features (retry, clear queue).
  - Created a new updateStore to manage application update checks and notifications.
  - Expanded types to include music service capabilities and update status for components.

- Implement icon generation script, ESLint configuration, and Vitest setup for testing
- Automate copyright year updates across all source files and enhance script functionality
- Implement theme management with useTheme hook and update styles for dark/light modes

### 🐛 Bug Fixes

- Resolve ESLint no-explicit-any error in Modal.test.tsx

Replace `any` type with `Record<string, unknown>` for the lucide-react
  X icon mock props to satisfy @typescript-eslint/no-explicit-any rule.


### 📚 Documentation

- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]
- Update CHANGELOG.md [skip ci]

---
*Generated with [git-cliff](https://git-cliff.org/)*
