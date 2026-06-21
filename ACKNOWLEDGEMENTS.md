# Acknowledgements

MeedyaDL is built on top of many open-source projects. We are grateful to the developers and maintainers of these libraries and tools.

---

## Download Engines

| Engine | Licence | Purpose |
|--------|---------|---------|
| [GAMDL](https://github.com/glomatico/gamdl) | MIT | Apple Music download engine |
| [votify](https://github.com/glomatico/votify) | MIT | Spotify download engine (planned) |
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | Unlicense | YouTube / BBC iPlayer download engine (planned) |
| [get_iplayer](https://github.com/get-iplayer/get_iplayer) | GPL-3.0 | BBC iPlayer download engine (planned) |

## External Tools

| Tool | Licence | Purpose |
|------|---------|---------|
| [FFmpeg](https://ffmpeg.org/) | LGPL-2.1+ | Audio/video processing, remuxing, ReplayGain analysis |
| [mp4decrypt / Bento4](https://www.bento4.com/) | MIT | MP4 DRM decryption |
| [N_m3u8DL-RE](https://github.com/nilaoda/N_m3u8DL-RE) | MIT | HLS/DASH stream downloader |
| [MP4Box / GPAC](https://gpac.io/) | LGPL-2.1 | Media container toolkit |
| [MediaInfo](https://mediaarea.net/en/MediaInfo) | BSD-2-Clause | Media file analysis and codec detection |
| [Python](https://www.python.org/) | PSF | Runtime for pip-based download engines |
| [rclone](https://rclone.org/) | MIT | Cloud-storage transport for direct-to-cloud downloads (optional, installed on-demand) |

---

## Rust Dependencies (Direct)

| Crate | Version | Licence | Description |
|-------|---------|---------|-------------|
| aes-gcm | 0.10 | MIT/Apache-2.0 | AES-GCM authenticated encryption (Profile Bundle export, credential vault) |
| arboard | 3.6 | MIT/Apache-2.0 | Cross-platform clipboard access |
| base64 | 0.22 | MIT/Apache-2.0 | Base64 encoding/decoding (animated artwork, API payloads) |
| chrono | 0.4 | MIT/Apache-2.0 | Date and time library |
| configparser | 3.1 | MIT/LGPL-3.0+ | INI file parsing (GAMDL config) |
| cookie | 0.18 | MIT/Apache-2.0 | HTTP cookie parsing |
| dirs | 6.0 | MIT/Apache-2.0 | Platform-standard directories |
| flate2 | 1.1 | MIT/Apache-2.0 | Gzip compression/decompression |
| fs2 | 0.4 | MIT/Apache-2.0 | Filesystem free-space + advisory locking (disk-space preflight) |
| jsonwebtoken | 10.3 | MIT | MusicKit JWT generation |
| keyring | 3.6 | MIT/Apache-2.0 | OS keychain access |
| lofty | 0.22 | MIT/Apache-2.0 | Audio metadata reading/writing (FLAC, MP3, OGG) |
| log | 0.4 | MIT/Apache-2.0 | Logging facade |
| meedya-fingerprint | (git, branch=main) | MIT | Shared audio-fingerprint primitives (Chromaprint + ebur128) from [MWBMPartners/MeedyaSuite-core](https://github.com/MWBMPartners/MeedyaSuite-core) |
| meedya-lyrics | (git, branch=main) | MIT | Shared lyrics primitives (TTML parser + classifier + Lyricsfile YAML + LRC offset round-trip) from [MWBMPartners/MeedyaSuite-core](https://github.com/MWBMPartners/MeedyaSuite-core) |
| mp4ameta | 0.13 | MIT/Apache-2.0 | M4A metadata reading/writing |
| pbkdf2 | 0.12 | MIT/Apache-2.0 | Password-based key derivation (Profile Bundle export passphrase) |
| rand | 0.8 | MIT/Apache-2.0 | Random number generation (salt + nonce derivation, retry jitter) |
| regex | 1.12 | MIT/Apache-2.0 | Regular expression parsing |
| reqwest | 0.12-0.13 | MIT/Apache-2.0 | HTTP client |
| rookie | 0.5 | — | Browser cookie extraction |
| roxmltree | 0.21 | MIT/Apache-2.0 | XML parsing (TTML lyrics) |
| rusqlite | 0.31 | MIT | SQLite bindings (Library Index database + Profile Bundle export manifests) |
| rusty-chromaprint | 0.2 | MIT | Audio fingerprinting (AcoustID) |
| sentry | 0.46 | MIT | Crash reporting SDK |
| sentry-tracing | 0.46 | MIT | Sentry integration for `tracing` events |
| serde | 1.0 | MIT/Apache-2.0 | Serialization framework |
| serde_json | 1.0 | MIT/Apache-2.0 | JSON serialization |
| sha2 | 0.10 | MIT/Apache-2.0 | SHA-256 hashing |
| symphonia | 0.5 | MPL-2.0 | Audio decoding (codec detection) |
| sys-locale | 0.3 | MIT/Apache-2.0 | OS locale detection (storefront auto-derivation) |
| tar | 0.4 | MIT/Apache-2.0 | TAR archive extraction |
| tauri | 2.10 | MIT/Apache-2.0 | Desktop application framework |
| thiserror | 2.0 | MIT/Apache-2.0 | Derive macro for ergonomic error types |
| tokio | 1.51 | MIT | Async runtime |
| toml | 0.8-0.9 | MIT/Apache-2.0 | TOML configuration parsing |
| tracing | 0.1 | MIT | Structured logging |
| tracing-appender | 0.2 | MIT | Daily-rotating file output for `tracing` logs |
| tracing-subscriber | 0.3 | MIT | `tracing` event collector and formatter (stderr + file) |
| url | 2.5 | MIT/Apache-2.0 | URL parsing |
| uuid | 1.23 | MIT/Apache-2.0 | UUID generation |
| zip | 2.4-4.6 | MIT | ZIP archive extraction |

### Tauri Plugins

| Plugin | Licence | Purpose |
|--------|---------|---------|
| tauri-plugin-deep-link | MIT | `meedyadl://` URL scheme |
| tauri-plugin-dialog | MIT | Native file/folder pickers |
| tauri-plugin-fs | MIT | File system access |
| tauri-plugin-notification | MIT | Native OS notifications |
| tauri-plugin-os | MIT | OS detection |
| tauri-plugin-process | MIT | Process management |
| tauri-plugin-shell | MIT | External command execution |
| tauri-plugin-store | MIT | Persistent key-value storage |
| tauri-plugin-updater | MIT | In-app auto-updates |

---

## npm Dependencies (Direct)

| Package | Version | Licence | Description |
|---------|---------|---------|-------------|
| @tanstack/react-virtual | 3.13 | MIT | Virtualized list rendering |
| @sentry/browser | 10.47 | MIT | Frontend crash reporting |
| @dnd-kit/core | 6.3 | MIT | Drag-and-drop framework |
| @dnd-kit/sortable | 10.0 | MIT | Sortable preset for @dnd-kit |
| @dnd-kit/utilities | 3.2 | MIT | Shared utilities for @dnd-kit |
| i18next | 26.0 | MIT | Internationalisation framework |
| i18next-browser-languagedetector | 8.2 | MIT | OS language detection |
| lucide-react | 1.7 | ISC | Icon library |
| react | 19.0 | MIT | UI component library |
| react-dom | 19.0 | MIT | React DOM renderer |
| react-i18next | 17.0 | MIT | React i18n bindings |
| react-markdown | 10.1 | MIT | Markdown rendering |
| rehype-raw | 7.0 | MIT | Raw HTML in markdown |
| rehype-sanitize | 6.0 | MIT | HTML sanitization |
| remark-gfm | 4.0 | MIT | GitHub Flavoured Markdown |
| zustand | 5.0 | MIT | Lightweight state management |

---

## Licence Compliance

All dependencies have been verified compatible with MeedyaDL's MIT licence via `cargo-deny` (Rust) and manual review (npm). The full licence check configuration is in `src-tauri/deny.toml`.

For questions about licence compliance, please open an issue on the [GitHub repository](https://github.com/MWBMPartners/MeedyaDL/issues).

---

*Last updated: 2026-05-24.*
