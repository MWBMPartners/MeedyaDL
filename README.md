<p align="center">
  <h1 align="center">🎵 GAMDL GUI</h1>
  <p align="center">
    <strong>A beautiful, multiplatform graphical interface for <a href="https://github.com/glomatico/gamdl">GAMDL</a> — the Apple Music downloader</strong>
  </p>
  <p align="center">
    Download songs, albums, playlists, music videos, and entire artist discographies from Apple Music with ease.
  </p>
</p>

<p align="center">
  <a href="https://github.com/MWBMPartners/gamdl-GUI/releases"><img src="https://img.shields.io/github/v/release/MWBMPartners/gamdl-GUI?style=flat-square&label=Version&color=blue" alt="Version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License: MIT"></a>
  <a href="https://github.com/MWBMPartners/gamdl-GUI/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/MWBMPartners/gamdl-GUI/ci.yml?style=flat-square&label=CI" alt="CI Status"></a>
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
- **Quality selection with smart fallback chain**: ALAC → Atmos → AC3 → AAC
- **Download queue management** with drag-and-drop reordering
- **Concurrent downloads** for faster batch processing

### 📝 Metadata & Extras
- **Lyrics support** — embed or save as LRC, SRT, or TTML formats
- **Cover art** — save artwork as JPG, PNG, or raw format at full resolution
- **Rich metadata tagging** powered by GAMDL

### 🔐 Authentication & Security
- **Cookie management** for Apple Music authentication
- **Secure credential storage** via OS-native keychains (macOS Keychain, Windows Credential Manager, Linux Secret Service)

### 🖥️ Platform-Adaptive UI
- **macOS** — Liquid Glass-inspired design with native vibrancy
- **Windows** — Fluent Design System with Mica/Acrylic effects
- **Linux** — Adwaita-inspired styling for GNOME integration

### ⚙️ Quality of Life
- **Auto-update checking** — stay on the latest version
- **First-run setup wizard** — installs Python and GAMDL automatically
- **Built-in help documentation** accessible in-app
- **System tray support** for background operation

---

## 💻 Supported Platforms

| Platform | Architecture | Format |
|----------|-------------|--------|
| 🍎 **macOS** | Apple Silicon (arm64) | `.dmg` |
| 🪟 **Windows** | x64, ARM64 | `.msi`, `.exe` |
| 🐧 **Linux** | x64 | `.deb`, `.AppImage` |
| 🍓 **Raspberry Pi** | ARM64 | `.deb`, `.AppImage` |

> **Note:** macOS requires version 11.0 (Big Sur) or later.

---

## 🏗️ Architecture

GAMDL GUI is built with a modern, performance-first tech stack:

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

1. **Download** the latest release for your platform from the [Releases](https://github.com/MWBMPartners/gamdl-GUI/releases) page.
2. **Install** using your platform's standard method:
   - **macOS**: Open the `.dmg` and drag GAMDL to Applications
   - **Windows**: Run the `.msi` installer
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
git clone https://github.com/MWBMPartners/gamdl-GUI.git
cd gamdl-GUI

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
gamdl-GUI/
├── src/                        # 🌐 React Frontend
│   ├── App.tsx                 #    Root component
│   ├── main.tsx                #    Entry point
│   ├── hooks/                  #    Custom React hooks
│   │   └── usePlatform.ts      #    Platform detection
│   └── styles/                 #    CSS & Themes
│       ├── globals.css          #    Global styles
│       └── themes/              #    Platform-adaptive themes
│           ├── base.css         #    Shared design tokens
│           ├── macos.css        #    macOS Liquid Glass
│           ├── windows.css      #    Windows Fluent
│           └── linux.css        #    Linux Adwaita
├── src-tauri/                  # 🦀 Rust Backend
│   ├── Cargo.toml              #    Rust dependencies
│   ├── tauri.conf.json         #    Tauri configuration
│   └── src/
│       ├── main.rs             #    Application entry point
│       ├── lib.rs              #    Plugin & command registration
│       ├── commands/           #    IPC command handlers
│       │   ├── system.rs       #    Platform info
│       │   ├── dependencies.rs #    Python/GAMDL management
│       │   ├── settings.rs     #    App settings
│       │   ├── gamdl.rs        #    Download orchestration
│       │   └── credentials.rs  #    Secure storage
│       ├── models/             #    Data structures
│       │   ├── download.rs     #    Download queue items
│       │   ├── gamdl_options.rs#    GAMDL CLI options
│       │   ├── settings.rs     #    App configuration
│       │   └── dependency.rs   #    Dependency status
│       ├── utils/              #    Utility modules
│       │   ├── platform.rs     #    OS detection & paths
│       │   ├── archive.rs      #    ZIP/tar extraction
│       │   └── process.rs      #    Child process management
│       └── services/           #    Business logic services
├── .github/workflows/          # 🔄 CI/CD
│   ├── ci.yml                  #    Test & lint on push/PR
│   ├── release.yml             #    Build & publish releases
│   └── changelog.yml           #    Auto-generate changelogs
├── scripts/                    # 🛠️ Utility scripts
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

### Current (v0.1.x)
- [x] Tauri 2.0 + React 19 foundation
- [x] Platform-adaptive UI themes (macOS, Windows, Linux)
- [x] Rust backend with IPC command system
- [x] Dependency management (Python, GAMDL)
- [x] CI/CD pipeline (GitHub Actions)
- [ ] Full download workflow with queue
- [ ] Settings UI with live preview
- [ ] Setup wizard

### Future
- 🎵 **YouTube Music support** via [gytmdl](https://github.com/glomatico/gytmdl) integration
- 🟢 **Spotify support** via [votify](https://github.com/glomatico/votify) integration
- 🌍 **Localization** (i18n) for multiple languages
- 📊 **Download history** and statistics
- 🎨 **Custom themes** and accent color picker

---

## 📄 License

```
MIT License

Copyright (c) 2024-2026 MWBM Partners Ltd
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

For the full implementation plan, architecture decisions, and development phases, see the [Project Plan](docs/Project_Plan.md).

---

<p align="center">
  Made with ❤️ by <a href="https://github.com/MWBMPartners">MWBM Partners Ltd</a>
</p>
