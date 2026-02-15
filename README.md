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
  <a href="https://github.com/MeedyaDL/MeedyaDL/releases"><img src="https://img.shields.io/badge/Version-0.3.5-blue?style=flat-square" alt="Version"></a> <!-- x-release-please-version -->
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License: MIT"></a>
  <a href="https://github.com/MeedyaDL/MeedyaDL/actions/workflows/ci.yml"><img src="https://github.com/MeedyaDL/MeedyaDL/actions/workflows/ci.yml/badge.svg" alt="CI Status"></a>
  <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platforms">
</p>

---

## 📸 Screenshots

> 🚧 **Coming soon** — Screenshots will be added once the UI reaches beta.

<!--
<p align="center">
  <img src="assets/screenshots/macos-light.png" width="45%" alt="macOS Light Mode">
  <img src="assets/screenshots/windows-dark.png" width="45%" alt="Windows Dark Mode">
</p>
-->

---

## ✨ Features

### 🎶 Music Downloads
- **Songs, Albums, Playlists, Artists, Music Videos** — download anything from Apple Music
- **Quality selection with smart fallback chain**: ALAC → Atmos → AC3 → AAC Binaural → AAC → AAC Legacy
- **Companion downloads** — configurable multi-format downloads: automatically download ALAC and/or lossy AAC companions alongside Dolby Atmos or ALAC primary downloads (4 modes: Disabled, Atmos→Lossless, Atmos→Lossless+Lossy, Specialist→Lossy)
- **Persistent download queue** — queue survives app close/crash; auto-resumes on restart
- **Queue export/import** — save queue to `.meedyadl` file, transfer to another device
- **Animated cover art** — automatically download motion artwork (FrontCover.mp4 / PortraitCover.mp4) via MusicKit API, with optional OS-level file hiding to keep folders clean

### 📝 Metadata & Extras
- **Lyrics embed + sidecar** — embed lyrics in file metadata AND save as separate LRC, SRT, or TTML files
- **Cover art** — save artwork as JPG, PNG, or raw format at full resolution
- **Rich metadata tagging** powered by GAMDL
- **Custom codec metadata** — ALAC files tagged `isLossless=Y`; Dolby Atmos files tagged `SpatialType=Dolby Atmos` for programmatic identification

### 🔐 Authentication & Security
- **Browser cookie auto-import** — detect installed browsers and import Apple Music cookies automatically
- **Built-in Apple Music login** — sign in directly within the app to extract cookies (no browser extension needed)
- **Cookie file import** — manual Netscape-format cookie import with domain/expiry validation
- **Secure credential storage** via OS-native keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service)

### 🖥️ Platform-Adaptive UI
- **macOS** — Liquid Glass-inspired design with native vibrancy
- **Windows** — Fluent Design System with Mica/Acrylic effects
- **Linux** — Adwaita-inspired styling for GNOME integration

### ⚙️ Quality of Life
- **Auto-update checking** — stay on the latest version
- **First-run setup wizard** — installs Python and GAMDL automatically
- **Built-in help documentation** — 11 topics with search, accessible in-app
- **System tray support** for background operation

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

1. 📦 **Dependency installation** — automatically downloads and installs a standalone Python and GAMDL (no system Python required)
2. 🍪 **Cookie configuration** — import your Apple Music cookies for authentication
3. 📂 **Output directory** — choose where downloaded music will be saved
4. 🎚️ **Quality preferences** — select your preferred audio codec and fallback chain

> 💡 The setup takes a few minutes on first run. All dependencies are sandboxed within the app's data directory.

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

## 📁 Project Structure

```
MeedyaDL/
├── src/                        # React Frontend
│   ├── App.tsx                 #    Root component with routing & event listeners
│   ├── main.tsx                #    Entry point
│   ├── components/             #    UI components
│   │   ├── common/             #    Shared: Button, Input, Modal, Toast, etc.
│   │   ├── layout/             #    Sidebar, TitleBar, StatusBar, MainLayout
│   │   ├── download/           #    DownloadForm, DownloadQueue, QueueItem
│   │   ├── settings/           #    SettingsPage + 9 tab components
│   │   ├── setup/              #    SetupWizard + 6 step components
│   │   └── help/               #    HelpViewer with markdown rendering
│   ├── stores/                 #    Zustand state stores
│   │   ├── uiStore.ts          #    Navigation, toasts, sidebar state
│   │   ├── settingsStore.ts    #    App settings load/save
│   │   ├── downloadStore.ts    #    Queue, progress, cancel/retry/clear
│   │   ├── dependencyStore.ts  #    Tool installation status
│   │   ├── setupStore.ts       #    Setup wizard step tracking
│   │   └── updateStore.ts      #    Update checking and notification
│   ├── lib/                    #    Utility modules
│   │   ├── tauri-commands.ts   #    Type-safe IPC wrappers
│   │   ├── url-parser.ts       #    Apple Music URL detection
│   │   └── quality-chains.ts   #    Fallback codec/resolution chains
│   ├── types/                  #    TypeScript types (mirrors Rust models)
│   ├── hooks/                  #    Custom React hooks
│   │   └── usePlatform.ts      #    Platform detection
│   └── styles/themes/          #    Platform-adaptive CSS
│       ├── base.css            #    Shared design tokens
│       ├── macos.css           #    macOS Liquid Glass
│       ├── windows.css         #    Windows Fluent
│       └── linux.css           #    Linux Adwaita
├── src-tauri/                  # Rust Backend
│   ├── Cargo.toml              #    Rust dependencies
│   ├── tauri.conf.json         #    Tauri configuration
│   └── src/
│       ├── main.rs             #    Application entry point
│       ├── lib.rs              #    Plugin, state & command registration
│       ├── commands/           #    IPC command handlers
│       │   ├── system.rs       #    Platform info
│       │   ├── dependencies.rs #    Python/GAMDL/tool management
│       │   ├── settings.rs     #    App settings
│       │   ├── gamdl.rs        #    Download queue orchestration
│       │   ├── credentials.rs  #    Secure keychain storage
│       │   ├── updates.rs      #    Update checking commands
│       │   ├── cookies.rs      #    Browser cookie extraction
│       │   ├── login_window.rs #    Embedded Apple Music login
│       │   └── artwork.rs      #    Animated artwork download
│       ├── models/             #    Data structures
│       │   ├── download.rs     #    Download request, state, queue status
│       │   ├── gamdl_options.rs#    All GAMDL CLI options as typed enums
│       │   ├── settings.rs     #    App configuration with defaults
│       │   ├── dependency.rs   #    Dependency status tracking
│       │   └── music_service.rs#    Service trait (extensibility)
│       ├── services/           #    Business logic
│       │   ├── python_manager.rs    # Portable Python download/install
│       │   ├── gamdl_service.rs     # GAMDL CLI wrapper & subprocess
│       │   ├── dependency_manager.rs# Tool download/install per platform
│       │   ├── config_service.rs    # JSON settings + INI sync
│       │   ├── download_queue.rs    # Queue manager with fallback/retry
│       │   ├── update_checker.rs    # Version update checker
│       │   ├── cookie_service.rs    # Browser cookie extraction
│       │   ├── login_window_service.rs # Embedded Apple Music login
│       │   ├── animated_artwork_service.rs # MusicKit animated cover art
│       │   └── metadata_tag_service.rs    # Custom M4A codec metadata tagging
│       └── utils/              #    Utility modules
│           ├── platform.rs     #    OS detection & paths
│           ├── archive.rs      #    ZIP/tar extraction
│           └── process.rs      #    GAMDL output parser & error classifier
├── help/                       # Markdown help documentation (11 topics)
├── .github/workflows/          # CI/CD
│   ├── ci.yml                  #    Test & lint on push/PR
│   ├── release.yml             #    Build & publish releases
│   ├── release-please.yml      #    Automated version bumps & release PRs
│   └── changelog.yml           #    Auto-generate changelogs
├── scripts/                    # Utility scripts
├── index.html                  #    Vite entry HTML
├── package.json                #    Node.js config
├── tailwind.config.js          #    Tailwind CSS config
├── vite.config.ts              #    Vite bundler config
├── tsconfig.json               #    TypeScript config
├── cliff.toml                  #    Changelog generation config
├── commitlint.config.js        #    Conventional commits config
└── LICENSE                     #    MIT License
```

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

### Current (v0.3.5) <!-- x-release-please-version -->

- [x] Tauri 2.0 + React 19 foundation
- [x] Platform-adaptive UI themes (macOS, Windows, Linux)
- [x] Rust backend with IPC command system
- [x] Dependency management (Python, GAMDL, FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box)
- [x] CI/CD pipeline (GitHub Actions + release-please)
- [x] Full download workflow with queue, fallback quality, and retry
- [x] Settings UI with 9 configuration tabs
- [x] First-run setup wizard (6 steps)
- [x] In-app help viewer with 11 topics and search
- [x] Cookie import with validation UI (step-by-step instructions, domain/expiry display)
- [x] Browser cookie auto-import (detect installed browsers, extract cookies automatically)
- [x] Built-in Apple Music login window (sign in directly, extract cookies from webview)
- [x] Auto-update checker (GAMDL, app, Python) with notification banner
- [x] System tray integration (show, status, updates, quit)
- [x] Animated cover art download via Apple MusicKit API (FrontCover.mp4 / PortraitCover.mp4)
- [x] Hidden animated artwork files (OS-level hidden attribute: macOS `chflags hidden`, Windows `attrib +H`, Linux `.` prefix)
- [x] Configurable companion downloads (4 modes: Disabled, Atmos to Lossless, Atmos to Lossless+Lossy, Specialist to Lossy)
- [x] Custom codec metadata tagging (ALAC: isLossless=Y; Atmos: SpatialType=Dolby Atmos)
- [x] Lyrics embed + sidecar (both embedded in file and saved as separate LRC/SRT/TTML)
- [x] Queue persistence and crash recovery (auto-save to disk, auto-resume on restart)
- [x] Queue export/import (transfer queue between devices via `.meedyadl` files)
- [x] Manual workflow dispatch (`workflow_dispatch` on all CI/CD workflows for conserving Actions minutes)

### Planned Milestones

| Milestone | Version | Service | Engine | Status |
| --------- | ------- | ------- | ------ | ------ |
| **M7** | v0.4.0 | Spotify | [votify](https://github.com/glomatico/votify) | Planned |
| **M8** | v0.5.0 | YouTube | [yt-dlp](https://github.com/yt-dlp/yt-dlp) | Planned |
| **M9** | v0.6.0 | BBC iPlayer | yt-dlp / [get_iplayer](https://github.com/get-iplayer/get_iplayer) | Planned |

Each milestone adds a new media service behind the existing `MusicService` trait (to be renamed `MediaService`), with its own CLI subprocess engine, URL parser, settings tab, and help documentation. See [Project Plan](Project_Plan.md) for full milestone details.

### Future (Beyond v0.6.0)

- 🎵 **YouTube Music** via [gytmdl](https://github.com/glomatico/gytmdl) integration
- 🔌 **Integration API** for third-party scripts and automation
- 🌍 **Localization** (i18n) for multiple languages
- 📊 **Download history** and statistics
- 🎨 **Custom themes** and accent color picker
- 🎚️ **Multi-track muxing** — combine companion downloads (Atmos + AC3 + AAC) into a single MP4 with multiple audio streams
- 🍎 **Native SwiftUI UI for macOS** — replace the web-based frontend on Apple Silicon with a fully native SwiftUI interface for tighter macOS integration and performance

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
