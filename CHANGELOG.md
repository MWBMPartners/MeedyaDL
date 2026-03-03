# Changelog

All notable changes to **MeedyaDL** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

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
