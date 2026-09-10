# Acknowledgements

MeedyaDL is built on top of many open-source projects. We are grateful to the developers and maintainers of these libraries and tools.

---

## Download Engines

| Engine | Licence | Purpose |
|--------|---------|---------|
| [GAMDL](https://github.com/glomatico/gamdl) | MIT | Apple Music download engine |
| [votify](https://github.com/glomatico/votify) | MIT | Spotify download engine (in development, behind a hidden developer-only preview switch — not available to regular users yet) |
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | Unlicense | YouTube / BBC iPlayer download engine (planned) |
| [get_iplayer](https://github.com/get-iplayer/get_iplayer) | GPL-3.0 | BBC iPlayer download engine (planned) |

## External Tools

| Tool | Licence | Purpose |
|------|---------|---------|
| [FFmpeg](https://ffmpeg.org/) | LGPL-2.1+ upstream — **the copy MeedyaDL downloads is GPL** (see note below) | Audio/video processing, remuxing, ReplayGain analysis |
| [mp4decrypt / Bento4](https://www.bento4.com/) | GPL-2.0 with linking exception | MP4 DRM decryption |
| [N_m3u8DL-RE](https://github.com/nilaoda/N_m3u8DL-RE) | MIT | HLS/DASH stream downloader |
| [MP4Box / GPAC](https://gpac.io/) | LGPL-2.1 | Media container toolkit |
| [MediaInfo](https://mediaarea.net/en/MediaInfo) | BSD-2-Clause | Media file analysis and codec detection |
| [Python](https://www.python.org/) | PSF | Runtime for pip-based download engines |
| [python-build-standalone](https://github.com/astral-sh/python-build-standalone) | MPL-2.0 (packaging only — the Python it packages is still PSF-licensed) | Portable CPython distribution bundled by the offline installer |
| [rclone](https://rclone.org/) | MIT | Cloud-storage transport for direct-to-cloud downloads (optional, installed on-demand) |

**mp4decrypt / Bento4 is copyleft (GPL-2.0), not permissive — do not read
the row above as MIT.** MeedyaDL never builds Bento4 into its own code;
it only runs the finished `mp4decrypt` program as a separate process,
the same way it runs FFmpeg or MP4Box. Because nothing of Bento4 is
compiled or linked into MeedyaDL, MeedyaDL's own MIT licence is not
affected by mp4decrypt's GPL terms. The full licence text, and the
written offer for mp4decrypt's source code (needed because the
offline-installer build copies the `mp4decrypt` program onto your
machine), are in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md#mp4decrypt--bento4).

**FFmpeg**: the FFmpeg project's own default posture is LGPL-2.1+, but
the specific pre-built copy MeedyaDL downloads (on every platform) is a
"gpl" build — treat it as GPL. See the FFmpeg entry in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md#ffmpeg) for why, and
why that's fine for how MeedyaDL uses it.

---

## Rust Dependencies (Direct)

Version numbers below are major.minor snapshots, not exact resolved
versions — the exact version actually built is whatever
[`src-tauri/Cargo.lock`](src-tauri/Cargo.lock) (Rust) or
[`package-lock.json`](package-lock.json) (npm, next section) says at
build time. We show the major.minor line rather than the exact patch so
this table doesn't need editing every time a routine dependency bump
lands; treat the lock files as the authoritative record if you need an
exact number.

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
| lzma-rs | 0.3 | MIT | Pure-Rust XZ decompression (.tar.xz tool archives) |
| meedya-core | (git, rev-pinned) | MIT | Shared platform primitives (metadata + codecs + fingerprint + lyrics + providers + tags + library-import + db, full feature set) from [MWBMPartners/MeedyaSuite-core](https://github.com/MWBMPartners/MeedyaSuite-core) |
| meedya-fingerprint | (git, branch=main) | MIT | Shared audio-fingerprint primitives (Chromaprint + ebur128) from [MWBMPartners/MeedyaSuite-core](https://github.com/MWBMPartners/MeedyaSuite-core). Its optional `chromaprint` feature pulls in `rusty-chromaprint` (MIT) and `symphonia` (MPL-2.0 — the one licence anywhere in this dependency tree that isn't MIT/Apache-2.0/BSD/ISC/Unlicense) as transitive dependencies; neither is a direct MeedyaDL dependency, so neither gets its own row here. |
| meedya-lyrics | (git, branch=main) | MIT | Shared lyrics primitives (TTML parser + classifier + Lyricsfile YAML + LRC offset round-trip) from [MWBMPartners/MeedyaSuite-core](https://github.com/MWBMPartners/MeedyaSuite-core) |
| mp4ameta | 0.13 | MIT/Apache-2.0 | M4A metadata reading/writing |
| pbkdf2 | 0.12 | MIT/Apache-2.0 | Password-based key derivation (Profile Bundle export passphrase) |
| rand | 0.8 | MIT/Apache-2.0 | Random number generation (salt + nonce derivation, retry jitter) |
| regex | 1.12 | MIT/Apache-2.0 | Regular expression parsing |
| reqwest | 0.12 | MIT/Apache-2.0 | HTTP client |
| rookie | 0.5 | MIT | Browser cookie extraction |
| roxmltree | 0.21 | MIT/Apache-2.0 | XML parsing (TTML lyrics) |
| rusqlite | 0.31 | MIT | SQLite bindings (Library Index database + Profile Bundle export manifests) |
| sentry | 0.46 | MIT | Crash reporting SDK |
| sentry-tracing | 0.46 | MIT | Sentry integration for `tracing` events |
| serde | 1.0 | MIT/Apache-2.0 | Serialization framework |
| serde_json | 1.0 | MIT/Apache-2.0 | JSON serialization |
| sha2 | 0.10 | MIT/Apache-2.0 | SHA-256 hashing |
| sys-locale | 0.3 | MIT/Apache-2.0 | OS locale detection (storefront auto-derivation) |
| tar | 0.4 | MIT/Apache-2.0 | TAR archive extraction |
| tauri | 2.11 | MIT/Apache-2.0 | Desktop application framework |
| thiserror | 2.0 | MIT/Apache-2.0 | Derive macro for ergonomic error types |
| tokio | 1.51 | MIT | Async runtime |
| toml | 0.9 | MIT/Apache-2.0 | TOML configuration parsing |
| tracing | 0.1 | MIT | Structured logging |
| tracing-appender | 0.2 | MIT | Daily-rotating file output for `tracing` logs |
| tracing-subscriber | 0.3 | MIT | `tracing` event collector and formatter (stderr + file) |
| url | 2.5 | MIT/Apache-2.0 | URL parsing |
| uuid | 1.23 | MIT/Apache-2.0 | UUID generation |
| zip | 2.4 | MIT | ZIP archive extraction |

### Tauri Plugins

Tauri and its plugins are dual-licensed under **Apache-2.0 OR MIT**
upstream, written below as `MIT/Apache-2.0` to match every other
dual-licensed row in this file. The matching npm packages
(`@tauri-apps/api`, `@tauri-apps/plugin-*`, next section) carry the
same dual licence.

| Plugin | Licence | Purpose |
|--------|---------|---------|
| tauri-plugin-deep-link | MIT/Apache-2.0 | `meedyadl://` URL scheme |
| tauri-plugin-dialog | MIT/Apache-2.0 | Native file/folder pickers |
| tauri-plugin-fs | MIT/Apache-2.0 | File system access |
| tauri-plugin-notification | MIT/Apache-2.0 | Native OS notifications |
| tauri-plugin-opener | MIT/Apache-2.0 | Opening/revealing files and folders on disk |
| tauri-plugin-os | MIT/Apache-2.0 | OS detection |
| tauri-plugin-process | MIT/Apache-2.0 | Process management |
| tauri-plugin-shell | MIT/Apache-2.0 | External command execution (and opening web addresses) |
| tauri-plugin-updater | MIT/Apache-2.0 | In-app auto-updates |

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

Rust dependencies (the full transitive tree, not just the direct ones listed above) are checked by `cargo-deny check licenses` against the permissive-only allowlist in `src-tauri/deny.toml`, run in CI on every push. The npm side no longer relies on manual review alone: `node scripts/check-acknowledgements.mjs` confirms every direct npm/Rust dependency is named somewhere in this file, and `node scripts/check-upstream-licences.mjs` confirms each direct dependency's actual upstream-declared licence string matches what this file claims (catching a re-licensing upstream before it ships to users). Both run via `npm run check:legal` and are wired into the `Licences` CI workflow. Neither script can see the download engines and external tools in the tables above — those aren't Cargo or npm dependencies, so there's no machine-readable licence metadata to check automatically; their entries are sourced from each project's own upstream LICENSE file and verified by hand.

For questions about licence compliance, please open an issue on the [GitHub repository](https://github.com/MWBMPartners/MeedyaDL/issues).

---

*Last updated: 2026-09-10.*
