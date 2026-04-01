<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo.svg"><img src="assets/brand/logo.png" alt="MeedyaDL Logo" height="96">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: light)" srcset="assets/brand/logotype.svg"><img src="assets/brand/logotype.png" alt="MeedyaDL" height="96">
  </picture>
</p>
<p align="center">
  <strong>A multiplatform media downloader</strong>
</p>
<p align="center">
  Download songs, albums, playlists, music videos, and more from Apple Music, Spotify, and other services.
</p>

<p align="center">
  <a href="https://github.com/MWBMPartners/MeedyaDL/releases"><img src="https://img.shields.io/github/v/release/MWBMPartners/MeedyaDL?include_prereleases&style=flat-square&label=Version" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License: MIT"></a>
  <a href="https://github.com/MWBMPartners/MeedyaDL/actions/workflows/ci.yml"><img src="https://github.com/MWBMPartners/MeedyaDL/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI Status"></a>
  <a href="https://github.com/MWBMPartners/MeedyaDL/actions/workflows/codeql.yml"><img src="https://github.com/MWBMPartners/MeedyaDL/actions/workflows/codeql.yml/badge.svg?branch=main" alt="CodeQL"></a>
  <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platforms">
</p>

---

## ✨ Features

### 🎶 Music Downloads

- **Songs, Albums, Playlists, Artists, Music Videos** — download anything from Apple Music. Non-geographic URLs (without a storefront code) are auto-detected and normalized
- **Quality selection with smart fallback chain**: ALAC → Atmos → AC3 → AAC Binaural → AAC → AAC Legacy
- **Companion downloads** — configurable multi-format downloads: automatically download additional codec versions alongside the primary download. Choose from 5 preset modes (Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy, All Formats) or use **Custom** mode with multi-select checkboxes to pick exactly which codecs to download as companions. **Music video companions** — optionally download the music video for each track alongside audio (requires MusicKit credentials)
- **Persistent download queue** — queue survives app close/crash; auto-resumes on restart, failed downloads persist for manual retry. Queue header shows live statistics (active/queued/completed/failed counts) with an aggregate progress bar; album downloads display a "Track N of M" counter
- **Queue export/import** — save queue to `.meedyadl` file, transfer to another device
- **Animated cover art** — automatically download motion artwork (FrontCover.mp4 / PortraitCover.mp4) via MusicKit API, with optional OS-level file hiding to keep folders clean

### 📝 Metadata & Extras

- **Enhanced LRC with word-by-word sync** — automatically converts Apple Music's TTML lyrics to Enhanced LRC with word-level synchronized timestamps for karaoke-style highlighting in compatible players (foobar2000, Poweramp, AIMP). Falls back to standard line-level LRC for songs without word-level data. Companion lyrics formats (LRC, SRT) remain selectable alongside Enhanced LRC.
- **Lyrics format fallback** — if the primary lyrics format isn't available for some tracks, automatically tries alternatives (Audio: TTML → LRC → SRT; Video: TTML → SRT → LRC)
- **Lyrics embed + sidecar** — embed lyrics in file metadata AND save as separate LRC, SRT, or TTML files
- **Cover art** — save artwork as JPG, PNG, or raw format at full resolution
- **Rich metadata tagging** powered by GAMDL
- **Comprehensive metadata enrichment** — 30+ freeform atoms per file including: codec/source/channel tags, ISRC, UPC, genre, advisory ratings, artist IDs, record label, copyright, release dates, Apple Digital Master flag, composer, editorial notes, and animated artwork URLs. All sourced from the Apple Music API and written in dual namespaces (`com.apple.iTunes` for player compatibility + `MeedyaMeta` for MeedyaDL attribution) with industry-standard alternative names (`LABEL`, `COPYRIGHT`, `COMPILATION`, `TOTALTRACKS`)
- **Config-driven tag system** — tag definitions live in `tags.toml`; adding new metadata tags requires only editing the TOML file — zero code changes
- **AcoustID fingerprinting** (opt-in) — Chromaprint audio fingerprints with acoustid.org lookup for MusicBrainz identification. Built-in API key included in release builds; no registration required
- **ReplayGain analysis** (opt-in) — non-destructive loudness metadata for volume normalisation in compatible players
- **Rich SRT subtitles** (opt-in, default: on) — converts TTML to SRT with styling tags (`<b>`, `<i>`, `<u>`, `<font color>`)
- **ASS subtitles** (opt-in) — converts TTML/WebVTT to Advanced SubStation Alpha with colours, positioning, and background vocal styling
- **WebVTT subtitles** (opt-in) — generates `.vtt` subtitle files from lyrics sidecars (TTML, SRT, or LRC)
- **Subtitle embedding** (opt-in) — embeds SRT and WebVTT content directly in MP4 containers as freeform atoms
- **MusicBrainz video discovery** (opt-in) — discovers music videos via MusicBrainz database using a 3-tier lookup (Apple Music URL → ISRC → AcoustID recording ID). No credentials required. Also discovers cross-platform URLs (YouTube, Spotify, etc.)
- **API field audit tool** — developer diagnostic in Settings > Metadata that compares real API responses against known tag definitions, surfacing new fields for review

### 🔐 Authentication & Security

- **Browser cookie auto-import** — detect installed browsers and import Apple Music cookies automatically
- **Built-in Apple Music login** — sign in directly within the app to extract cookies (no browser extension needed)
- **Cookie file import** — manual Netscape-format cookie import with domain/expiry validation
- **Pre-download validation** — multi-provider internet connectivity check (Cloudflare, Google, Apple Music API), output path writability probe (catches disconnected cloud mounts), and cookie validation are checked before every download; offline downloads are queued but deferred until connectivity returns; expired cookies block the download with a link to Settings > Cookies
- **Wrapper support** — alternative authentication via a locally-running wrapper service for more reliable Dolby Atmos and DRM-protected format access, with optional **auto-retry without wrapper** when wrapper downloads fail (see [Wrapper Authentication](#wrapper-authentication) section below)
- **Secure credential storage** via OS-native keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service)

### 🖥️ Platform-Adaptive UI

- **macOS** — Liquid Glass-inspired design with native vibrancy
- **Windows** — Fluent Design System with Mica/Acrylic effects
- **Linux** — Adwaita-inspired styling for GNOME integration

### ⚙️ Quality of Life

- **Auto-update checking** — stay on the latest version with full release notes in the Updates page
- **Auto-start queue** — downloads start immediately by default, or toggle off to batch-add URLs and start manually from the Queue page
- **Configurable temp directory** — intermediate files stored in `{OS temp}/MeedyaDL` by default, customizable in Settings > Paths
- **First-run setup wizard** — installs Python and GAMDL automatically; detects existing tools from system PATH
- **Built-in help documentation** — 12 topics with search, accessible in-app
- **System tray support** for background operation
- **Smart notifications** — toast notifications deduplicate automatically (no more stacking identical messages) and auto-dismiss when their condition resolves (e.g., wrapper warning clears when wrapper becomes reachable)
- **Crash reporting** — local crash report logging with optional Sentry telemetry and one-click GitHub Issues reporting (pre-filled issue opened in your browser with privacy preview)
- **Graceful shutdown** — background tasks (enrichment, companion downloads, lyrics) stop cleanly on window close or tray quit instead of being abruptly terminated
- **Supply chain hardening** — all CI/CD GitHub Actions pinned to immutable commit SHAs, SHA-256 checksum verification for dependency downloads, `cargo-deny` licence scanning in CI
- **Accessibility** — ARIA labels on interactive elements, `aria-live` regions for dynamic content updates, `prefers-reduced-motion` support, skip navigation, high-contrast mode, colour-blind themes (deuteranopia, protanopia, tritanopia)
- **i18n groundwork** — translation infrastructure with OS language detection and manual language selection (English, German, French)
- **Pre-release version handling** — verbose activity logging persists across restarts during pre-release versions (v0.x.x) for easier debugging; first-load notice modal warns users when a new pre-release version is launched
- **Component version info** — Help > About screen displays a Component Library table with installed versions of all tools (Python, GAMDL, FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box) via the `get_component_versions` IPC command
- **Collapsible Help > About sections** — Credits, License, Links, Open Source Acknowledgements, and Component Library are wrapped in `<details>`/`<summary>` elements for clean, scannable layout
- **CodeQL workflow** — GitHub CodeQL static analysis for Actions YAML and JavaScript/TypeScript. Rust is intentionally excluded (build hangs indefinitely) since Rust code quality is covered by clippy and cargo test in CI
- **Brand assets** — vinyl/reel icon design with animated SVG logo and logotype. Brand kit page at `assets/brand/brandkit.html`. Colour mode support: light, dark, and 3 colour-blind variants (deuteranopia, protanopia, tritanopia) plus dark variants of each. Sidebar uses animated SVG logo + logotype via `<object>` tags with static PNG/text fallbacks

---

## 💻 Supported Platforms

| Platform | Architecture | Format | Notes |
| -------- | ------------ | ------ | ----- |
| 🍎 **macOS** | Apple Silicon (ARM64) | `.dmg` | Requires macOS 13.3 (Ventura) or later |
| 🪟 **Windows** | x64 (64-bit) | `.exe` (NSIS) | Also works on ARM64 via emulation |
| 🪟 **Windows** | ARM64 | `.exe` (NSIS) | Native ARM64 build |
| 🐧 **Linux** | x64 | `.deb`, `.AppImage` | Also works on ChromeOS via Crostini |
| 🐧 **Linux** | ARM64 | `.deb` | Raspberry Pi 4/5, ARM servers |
| 🐧 **Linux** | ARMv7 | `.deb` | Raspberry Pi 32-bit (experimental) |

---

## Wrapper Authentication

The **wrapper** is an alternative authentication method for advanced users. Instead of using browser cookies, it connects to a locally-running server that handles Apple ID authentication and DRM key exchange directly.

### When to Use It

Most users should stick with **cookie-based authentication** (the default). The wrapper is useful if you:

- Need more reliable access to **Dolby Atmos** or other DRM-protected formats
- Experience frequent cookie expiration issues
- Are comfortable running local server software

### Setup

1. **Obtain and run the wrapper service** — the wrapper is a separate application (not bundled with MeedyaDL) that listens on `http://127.0.0.1:30020` by default
2. **Enable in MeedyaDL** — go to **Settings > Advanced** and toggle **Use Wrapper** on
3. **Configure the URL** — update the **Wrapper Account URL** if the wrapper runs on a different host or port

### Auto-Retry without Wrapper

When a wrapper download fails (all retries exhausted), MeedyaDL normally shows a "Retry without Wrapper" button on the failed item. If you'd prefer this to happen automatically:

1. Go to **Settings > Advanced > Wrapper**
2. Enable **Auto-Retry without Wrapper**

When enabled, failed wrapper downloads are automatically re-queued with cookie-based authentication — no manual intervention needed. This is useful if your wrapper is intermittently unavailable and you want downloads to fall back gracefully.

### Verifying Connectivity

MeedyaDL checks wrapper connectivity in two ways:

- **Manual test** — click **Test Connection** in Settings > Advanced. Shows "Connected (Xms)" on success, or a specific error (timeout, connection refused) on failure.
- **Automatic pre-flight check** — every time the download queue starts processing, MeedyaDL pings the wrapper and shows a yellow toast notification if it's unreachable (e.g., `Wrapper service at http://192.168.3.179:30020 timed out — check that it is running`). The notification auto-dismisses when the wrapper becomes reachable again. Downloads still proceed — the check is advisory.

### Troubleshooting (Remote / Docker)

If MeedyaDL reports the wrapper is unreachable — especially when the wrapper runs on a separate device (e.g. a Raspberry Pi, VPS, or Docker container) — you can diagnose the issue from a terminal on the machine running MeedyaDL:

1. **Test with curl** — `curl -v http://192.168.x.x:30020` (use your actual wrapper URL). "Connection refused" means nothing is listening; "timed out" means the host is unreachable.
2. **Check the wrapper is running** on the host — `docker ps | grep wrapper` (Docker) or `ps aux | grep wrapper` (native). Check logs with `docker logs <container> --tail 50`.
3. **Check the port is accessible** — on the wrapper host, run `ss -tlnp | grep 30020`. If it shows `127.0.0.1:30020`, the wrapper only accepts local connections — configure it to bind to `0.0.0.0`. For Docker, verify port mapping with `docker port <container>`.
4. **Check firewalls** — `sudo ufw allow 30020/tcp` (Linux), or check your OS firewall settings. Devices on the same LAN usually don't need router port forwarding.

For the full step-by-step troubleshooting guide, see the in-app help (**Help > Wrapper > Troubleshooting Wrapper Connectivity**).

### Platform Support

The Wrapper service only provides native binaries for **Linux x86_64**. On other platforms, you can run the wrapper remotely on a Linux server or in a Docker container and point MeedyaDL to it via a custom URL. See the in-app help (**Help > Wrapper**) for detailed remote setup instructions.

---

## 🏗️ Architecture

MeedyaDL is built with a modern, performance-first tech stack:

```text
┌─────────────────────────────────────────┐
│           React 19 + TypeScript         │  ← Frontend UI
│       Tailwind CSS v4 + Zustand         │
├─────────────────────────────────────────┤
│              Tauri 2.0 IPC              │  ← Bridge
├─────────────────────────────────────────┤
│            Rust Backend (Tokio)         │  ← Native Layer
│  Commands · Models · Services · Utils   │
├─────────────────────────────────────────┤
│          Engine Registry (TOML)         │  ← Per-platform routing
│  engines.toml · codecs.toml · tags.toml │
├─────────────────────────────────────────┤
│       Embedded Python + pip Engines     │  ← Download Engines
│      GAMDL · votify · yt-dlp · ...     │
└─────────────────────────────────────────┘
```

| Layer | Technology | Purpose |
| ----- | ---------- | ------- |
| **Frontend** | React 19, TypeScript, Tailwind CSS v4, Zustand | Reactive UI with platform-adaptive themes |
| **Framework** | Tauri 2.0 | Lightweight native shell, IPC, plugins |
| **Backend** | Rust, Tokio, Reqwest | Async process management, downloads, credential storage |
| **Registry** | TOML configs (engines, codecs, tags) | Declarative per-platform engine routing and metadata |
| **Engines** | Python (standalone), GAMDL, votify, yt-dlp | Service-specific download and decryption |

---

## 🚀 Quick Start

### Installation

1. **Download** the latest release for your platform from the [Releases](https://github.com/MWBMPartners/MeedyaDL/releases) page.
2. **Install** using your platform's standard method:
   - **macOS**: Open the `.dmg` and drag MeedyaDL to Applications
   - **Windows**: Run the `.exe` installer
   - **Linux**: Install the `.deb` or run the `.AppImage`
3. **Launch** the application.

### First-Run Setup

On first launch, the setup wizard will guide you through:

1. 📦 **Dependency installation** — automatically downloads and installs a standalone Python, GAMDL, votify, and external tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, MediaInfo). If compatible versions are already installed on your system, those are used instead of downloading fresh copies.
2. 🍪 **Cookie configuration** — import your Apple Music cookies for authentication
3. 📂 **Output directory** — choose where downloaded music will be saved
4. 🎚️ **Quality preferences** — select your preferred audio codec and fallback chain

> 💡 The setup takes a few minutes on first run. Dependencies are sandboxed within the app's data directory, or reused from your system PATH if already installed.

---

## 🔨 Building from Source

### Prerequisites

| Tool | Version | Notes |
| ---- | ------- | ----- |
| **Node.js** | LTS (20+) | Frontend build toolchain |
| **npm** | 10+ | Comes with Node.js |
| **Rust** | Stable (1.77+) | Backend compilation |
| **Tauri CLI** | 2.x | `npm install` handles this |

#### Linux Additional Dependencies

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev \
  libgtk-3-dev
```

### Build Steps

```bash

# Clone the repository

git clone https://github.com/MWBMPartners/MeedyaDL.git
cd MeedyaDL

# Install frontend dependencies

npm install

# Build the application (debug)

npm run tauri build -- --debug

# Build the application (release)

npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

---

For the full project structure, see [DEV_NOTES.md](DEV_NOTES.md#-project-structure).

---

## 🛠️ Development

### Running in Dev Mode

```bash

# Start the frontend dev server + Tauri window with hot reload

npm run tauri dev
```

This launches:

- **Vite dev server** on `http://localhost:1420` with HMR
- **Tauri native window** that loads the dev server
- **Rust backend** with debug logging (set `RUST_LOG=debug` for verbose output)

### Available Scripts

| Command | Description |
| ------- | ----------- |
| `npm run dev` | Start Vite dev server only |
| `npm run build` | Build frontend (TypeScript + Vite) |
| `npm run tauri dev` | Full dev mode (frontend + backend) |
| `npm run tauri build` | Production build |
| `npm run type-check` | TypeScript type checking |
| `npm run lint` | ESLint for `src/` |
| `npm run format` | Prettier formatting |
| `npm run format:check` | Check formatting without changes |
| `npm run test` | Run frontend tests (Vitest) |
| `npm run test:watch` | Run tests in watch mode |

### Rust Backend

```bash

# Check compilation

cd src-tauri && cargo check

# Run clippy linter

cargo clippy -- -D warnings

# Run Rust unit tests

cargo test
```

---

## 🤝 Contributing

Contributions are welcome! Please follow these guidelines:

### Commit Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/) for automated version bumps and changelog generation:

```text
type(scope): description

# Examples:
feat(download): add fallback quality chain support   # → minor bump, in changelog
fix(settings): resolve cookie validation edge case   # → patch bump, in changelog
refactor(backend): simplify dependency management    # → no bump, in changelog as "Improvements"
chore(deps): update dependencies                     # → no bump, hidden from changelog
```

| Type | Version Bump | Changelog |
| ---- | ------------ | --------- |
| `feat` | Minor (0.**X**.0) | Features |
| `fix` | Patch (0.0.**X**) | Bug Fixes |
| `refactor` | None | Improvements |
| `perf` | None | Improvements |
| `test` | None | Improvements |
| `chore` | None | Hidden |
| `docs` | None | Hidden |
| `ci` | None | Hidden |

**Important:** Use `feat:` only for **user-visible** new functionality. Internal infrastructure, tooling, and config changes should use `chore:` or `refactor:` to avoid unnecessary version bumps.

### Development Workflow

1. 🍴 Fork the repository
2. 🌿 Create a feature branch: `git checkout -b feat/my-feature`
3. 💾 Commit changes using conventional commits
4. ✅ Ensure all checks pass: `npm run type-check && npm run test`
5. 📬 Open a pull request against `main`

---

## 🗺️ Roadmap

### v1.x — Current (v0.27.0) <!-- x-release-please-version -->

- ✅ Tauri 2.0 + React 19 foundation with platform-adaptive UI
- ✅ Full Apple Music download workflow with queue, fallback quality, and retry
- ✅ Automatic dependency management with first-run setup wizard
- ✅ CI/CD pipeline with release-please, pre-release channel, and bundled dependencies
- ✅ Settings UI with 10 configuration tabs
- ✅ Cookie import (browser auto-detect, built-in login, manual import)
- ✅ Auto-update checker with in-app download, install, and rollback
- ✅ System tray integration
- ✅ Animated cover art via MusicKit API with OS-level file hiding
- ✅ Configurable companion downloads (5 preset modes + Custom multi-select)
- ✅ Multi-select artist auto-select (download multiple content types from artist URLs)
- ✅ Metadata enrichment (codec/source/channel tags, Apple Music API, AcoustID, ReplayGain)
- ✅ Enhanced LRC with word-by-word synchronized lyrics (TTML → Enhanced LRC conversion)
- ✅ Lyrics embed + sidecar (LRC, SRT, TTML)
- ✅ WebVTT subtitle generation from lyrics sidecars (TTML, SRT, LRC)
- ✅ MusicBrainz video discovery with 3-tier lookup (URL → ISRC → AcoustID recording ID)
- ✅ Queue persistence, crash recovery, and export/import
- ✅ Updates page with rendered release notes
- ✅ In-app help viewer with 12 topics and search
- ✅ i18n infrastructure (i18next, OS language detection, English)
- ✅ Smart re-download detection — checks download history and Apple Music `lastModifiedDate` to detect album changes
- ✅ Per-track activity log separators with codec and auth info
- ✅ MediaInfo CLI integration for accurate Atmos/AC3 codec detection
- ✅ Engine registry (`engines.toml`) for per-platform tool priority and fallback

### v2.x — Multi-Service Expansion

| Milestone | Version | Service | Engine | Issue | Status |
| --- | --- | --- | --- | --- | --- |
| — | v2.0.0 | Multi-service architecture | — | [#107](https://github.com/MWBMPartners/MeedyaDL/issues/107) | 🔲 Prerequisite |
| — | v2.0.0 | Engine priority system | — | [#268](https://github.com/MWBMPartners/MeedyaDL/issues/268) | 🔲 Prerequisite |
| M8 | v2.0.0 | Spotify | [votify](https://github.com/glomatico/votify) | [#101](https://github.com/MWBMPartners/MeedyaDL/issues/101) | 🔲 Planned |
| M9 | v2.1.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | [#104](https://github.com/MWBMPartners/MeedyaDL/issues/104) | 🔲 Planned |
| M10 | v2.2.0 | BBC iPlayer | [get_iplayer](https://github.com/get-iplayer/get_iplayer) / yt-dlp | [#102](https://github.com/MWBMPartners/MeedyaDL/issues/102) | 🔲 Planned |

Each milestone adds a new media service with its own CLI subprocess engine, URL parser, settings tab, and help documentation. Engine priority per platform is defined in `engines.toml`. See [Project Plan](Project_Plan.md) for full milestone details.

### v3.x — Advanced Features

- 🔮 **Smart Download** ([#110](https://github.com/MWBMPartners/MeedyaDL/issues/110)) — cross-platform quality optimisation (search all services for the same content, download the best quality)
- 🔮 **YouTube Music** ([#103](https://github.com/MWBMPartners/MeedyaDL/issues/103)) via [gytmdl](https://github.com/glomatico/gytmdl) for music-specific features beyond yt-dlp
- 🔮 **Full i18n** ([#111](https://github.com/MWBMPartners/MeedyaDL/issues/111)) — complete translations for German, French, and additional languages
- 🔮 **Enhanced MusicKit Integration** ([#108](https://github.com/MWBMPartners/MeedyaDL/issues/108)) — server-side token generation to remove Apple Developer credential requirement
- 🔮 **Stable rollback** ([#267](https://github.com/MWBMPartners/MeedyaDL/issues/267)) — option to roll back from pre-release to latest stable version

### Future

- 🔮 **Remote Service Status** ([#106](https://github.com/MWBMPartners/MeedyaDL/issues/106)) — developer-controlled kill switch for individual media services
- 🔮 **Anonymous Crash Reporting** ([#44](https://github.com/MWBMPartners/MeedyaDL/issues/44)) — PHP relay for crash submission without GitHub account
- 🔮 **Native SwiftUI UI for macOS** ([#109](https://github.com/MWBMPartners/MeedyaDL/issues/109)) — fully native frontend on Apple Silicon

---

## 📄 License

```text
MIT License

Copyright (c) 2026 MeedyaDL
```

This project is licensed under the **MIT License** — see the [LICENSE](LICENSE) file for full details.

---

## 🙏 Credits & Acknowledgements

| Project | Role |
| ------- | ---- |
| [**GAMDL**](https://github.com/glomatico/gamdl) | Apple Music download engine |
| [**votify**](https://github.com/glomatico/votify) | Spotify download engine |
| [**yt-dlp**](https://github.com/yt-dlp/yt-dlp) | YouTube / general-purpose download engine |
| [**get_iplayer**](https://github.com/get-iplayer/get_iplayer) | BBC iPlayer specialist engine |
| [**Tauri**](https://tauri.app/) | Lightweight, secure framework for building native apps with web tech |
| [**python-build-standalone**](https://github.com/indygreg/python-build-standalone) | Portable, self-contained Python builds bundled with the app |
| [**MediaInfo**](https://mediaarea.net/en/MediaInfo) | Accurate codec detection for enrichment pipeline |
| [**React**](https://react.dev/) | Frontend UI library |
| [**Zustand**](https://github.com/pmndrs/zustand) | Lightweight state management |
| [**Tailwind CSS**](https://tailwindcss.com/) | Utility-first CSS framework |
| [**Lucide**](https://lucide.dev/) | Beautiful, consistent icon set |

---

## 📖 Additional Documentation

For the full implementation plan, project status, architecture decisions, and development phases, see the [Project Plan](Project_Plan.md).

---

<p align="center">
  Made with ❤️ by MeedyaDL
</p>
