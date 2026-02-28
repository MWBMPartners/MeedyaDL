<p align="center">
  <h1 align="center">🎵 MeedyaDL</h1>
  <p align="center">
    <strong>A multiplatform media downloader</strong>
  </p>
  <p align="center">
    Download songs, albums, playlists, music videos, and more from your favourite media services with ease.
  </p>
</p>

<p align="center">
  <a href="https://github.com/MeedyaDL/MeedyaDL/releases"><img src="https://img.shields.io/badge/Version-0.5.4?style=flat-square" alt="Version"></a> <!-- x-release-please-version -->
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License: MIT"></a>
  <a href="https://github.com/MeedyaDL/MeedyaDL/actions/workflows/ci.yml"><img src="https://github.com/MeedyaDL/MeedyaDL/actions/workflows/ci.yml/badge.svg" alt="CI Status"></a>
  <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platforms">
</p>

---

## 📸 Screenshots

> Screenshots coming soon. Run the app locally with `cargo tauri dev` to see the UI.

---

## ✨ Features

### 🎶 Music Downloads
- **Songs, Albums, Playlists, Artists, Music Videos** — download anything from Apple Music
- **Quality selection with smart fallback chain**: ALAC → Atmos → AC3 → AAC Binaural → AAC → AAC Legacy
- **Companion downloads** — configurable multi-format downloads: automatically download ALAC and/or lossy AAC companions alongside Dolby Atmos or ALAC primary downloads (4 modes: Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy)
- **Persistent download queue** — queue survives app close/crash; auto-resumes on restart, failed downloads persist for manual retry
- **Queue export/import** — save queue to `.meedyadl` file, transfer to another device
- **Animated cover art** — automatically download motion artwork (FrontCover.mp4 / PortraitCover.mp4) via MusicKit API, with optional OS-level file hiding to keep folders clean

### 📝 Metadata & Extras

- **Enhanced LRC with word-by-word sync** — automatically converts Apple Music's TTML lyrics to Enhanced LRC with word-level synchronized timestamps for karaoke-style highlighting in compatible players (foobar2000, Poweramp, AIMP). Falls back to standard line-level LRC for songs without word-level data.
- **Lyrics embed + sidecar** — embed lyrics in file metadata AND save as separate LRC, SRT, or TTML files
- **Cover art** — save artwork as JPG, PNG, or raw format at full resolution
- **Rich metadata tagging** powered by GAMDL
- **Metadata enrichment** — codec tags, source tags, channel detection, ISRC, UPC, genre, advisory ratings, artist IDs, and animated artwork URLs via Apple Music API
- **AcousticID fingerprinting** (opt-in) — Chromaprint audio fingerprints with acoustid.org lookup for MusicBrainz identification
- **ReplayGain analysis** (opt-in) — non-destructive loudness metadata for volume normalisation in compatible players

### 🔐 Authentication & Security
- **Browser cookie auto-import** — detect installed browsers and import Apple Music cookies automatically
- **Built-in Apple Music login** — sign in directly within the app to extract cookies (no browser extension needed)
- **Cookie file import** — manual Netscape-format cookie import with domain/expiry validation
- **Pre-download cookie validation** — cookies are checked before every download; expired or missing cookies block the download with a clear message and link to Settings > Cookies
- **Wrapper support** — alternative authentication via a locally-running wrapper service for more reliable Dolby Atmos and DRM-protected format access (see [Wrapper Authentication](#wrapper-authentication) section below)
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
- **Crash reporting** — local crash report logging with optional Sentry telemetry and one-click GitHub Issues reporting (pre-filled issue opened in your browser with privacy preview)
- **i18n groundwork** — translation infrastructure with OS language detection and manual language selection (English, German, French)

---

## 💻 Supported Platforms

| Platform | Architecture | Format | Notes |
|----------|-------------|--------|-------|
| 🍎 **macOS** | Apple Silicon (ARM64) | `.dmg` | Requires macOS 11.0 (Big Sur) or later |
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

### Verifying Connectivity

MeedyaDL checks wrapper connectivity in two ways:

- **Manual test** — click **Test Connection** in Settings > Advanced. Shows "Connected (Xms)" on success, or a specific error (timeout, connection refused) on failure.
- **Automatic pre-flight check** — every time the download queue starts processing, MeedyaDL pings the wrapper and shows a yellow toast notification if it's unreachable (e.g., `Wrapper service at http://192.168.3.179:30020 timed out — check that it is running`). Downloads still proceed — the check is advisory.

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

```
┌─────────────────────────────────────────┐
│           React 19 + TypeScript         │  ← Frontend UI
│         Tailwind CSS + Zustand          │
├─────────────────────────────────────────┤
│              Tauri 2.0 IPC              │  ← Bridge
├─────────────────────────────────────────┤
│            Rust Backend (Tokio)         │  ← Native Layer
│  Commands · Models · Services · Utils   │
├─────────────────────────────────────────┤
│    Embedded Python + GAMDL (pip pkg)    │  ← Download Engine
└─────────────────────────────────────────┘
```

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Frontend** | React 19, TypeScript, Tailwind CSS, Zustand | Reactive UI with platform-adaptive themes |
| **Framework** | Tauri 2.0 | Lightweight native shell, IPC, plugins |
| **Backend** | Rust, Tokio, Reqwest | Async process management, downloads, credential storage |
| **Engine** | Python (standalone), GAMDL | Apple Music interaction and decryption |

---

## 🚀 Quick Start

### Installation

1. **Download** the latest release for your platform from the [Releases](https://github.com/MeedyaDL/MeedyaDL/releases) page.
2. **Install** using your platform's standard method:
   - **macOS**: Open the `.dmg` and drag MeedyaDL to Applications
   - **Windows**: Run the `.exe` installer
   - **Linux**: Install the `.deb` or run the `.AppImage`
3. **Launch** the application.

### First-Run Setup

On first launch, the setup wizard will guide you through:

1. 📦 **Dependency installation** — automatically downloads and installs a standalone Python, GAMDL, and external tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box). If compatible versions are already installed on your system, those are used instead of downloading fresh copies.
2. 🍪 **Cookie configuration** — import your Apple Music cookies for authentication
3. 📂 **Output directory** — choose where downloaded music will be saved
4. 🎚️ **Quality preferences** — select your preferred audio codec and fallback chain

> 💡 The setup takes a few minutes on first run. Dependencies are sandboxed within the app's data directory, or reused from your system PATH if already installed.

---

## 🔨 Building from Source

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
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
git clone https://github.com/MeedyaDL/MeedyaDL.git
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
|---------|-------------|
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

This project uses [Conventional Commits](https://www.conventionalcommits.org/) enforced by [commitlint](https://commitlint.js.org/):

```
type(scope): description

# Examples:
feat(download): add fallback quality chain support
fix(settings): resolve cookie validation edge case
docs(readme): update installation instructions
refactor(backend): simplify dependency management
```

**Allowed types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

### Development Workflow

1. 🍴 Fork the repository
2. 🌿 Create a feature branch: `git checkout -b feat/my-feature`
3. 💾 Commit changes using conventional commits
4. ✅ Ensure all checks pass: `npm run type-check && npm run test`
5. 📬 Open a pull request against `main`

---

## 🗺️ Roadmap

### v1.x — Current (v0.5.4) <!-- x-release-please-version -->

- ✅ Tauri 2.0 + React 19 foundation with platform-adaptive UI
- ✅ Full Apple Music download workflow with queue, fallback quality, and retry
- ✅ Automatic dependency management with first-run setup wizard
- ✅ CI/CD pipeline with release-please, pre-release channel, and bundled dependencies
- ✅ Settings UI with 10 configuration tabs
- ✅ Cookie import (browser auto-detect, built-in login, manual import)
- ✅ Auto-update checker with in-app download, install, and rollback
- ✅ System tray integration
- ✅ Animated cover art via MusicKit API with OS-level file hiding
- ✅ Configurable companion downloads (4 modes)
- ✅ Metadata enrichment (codec/source/channel tags, Apple Music API, AcousticID, ReplayGain)
- ✅ Enhanced LRC with word-by-word synchronized lyrics (TTML → Enhanced LRC conversion)
- ✅ Lyrics embed + sidecar (LRC, SRT, TTML)
- ✅ Queue persistence, crash recovery, and export/import
- ✅ Updates page with rendered release notes
- ✅ In-app help viewer with 12 topics and search
- ✅ i18n infrastructure (i18next, OS language detection, English)

### v2.x — Multi-Service Expansion

| Milestone | Version | Service | Engine | Status |
| --- | --- | --- | --- | --- |
| M8 | v2.0.0 | Spotify | [votify](https://github.com/glomatico/votify) | 🔲 Planned |
| M9 | v2.1.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | 🔲 Planned |
| M10 | v2.2.0 | BBC iPlayer | yt-dlp / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | 🔲 Planned |

Each milestone adds a new media service with its own CLI subprocess engine, URL parser, settings tab, and help documentation. See [Project Plan](Project_Plan.md) for full milestone details.

### v3.x — Advanced Features

- 🔮 **Smart Download** — cross-platform quality optimisation (search all services for the same content, download the best quality)
- 🔮 **YouTube Music** via [gytmdl](https://github.com/glomatico/gytmdl) for music-specific features beyond yt-dlp
- 🔮 **Full i18n** — complete translations for German, French, and additional languages
- 🔮 **Download history** and statistics dashboard

### Future

- 🔮 **Remote Service Status** — developer-controlled kill switch for individual media services
- 🔮 **Integration API** for third-party scripts and automation
- 🔮 **Custom themes** and accent colour picker
- 🔮 **Multi-track muxing** — combine companion downloads into a single MP4 with multiple audio streams
- 🔮 **Native SwiftUI UI for macOS** — fully native frontend on Apple Silicon

---

## 📄 License

```
MIT License

Copyright (c) 2024-2026 MeedyaDL
```

This project is licensed under the **MIT License** — see the [LICENSE](LICENSE) file for full details.

---

## 🙏 Credits & Acknowledgements

| Project | Role |
|---------|------|
| [**GAMDL**](https://github.com/glomatico/gamdl) | The core Apple Music download engine this GUI wraps |
| [**Tauri**](https://tauri.app/) | Lightweight, secure framework for building native apps with web tech |
| [**python-build-standalone**](https://github.com/indygreg/python-build-standalone) | Portable, self-contained Python builds bundled with the app |
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
