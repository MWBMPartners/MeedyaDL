<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# MeedyaDL Help Center

Welcome to the **MeedyaDL** help documentation. This guide covers everything you need to know about using MeedyaDL, a multiplatform media downloader.

These pages can be translated: a translated page lives at `help/<language>/<same file name>.md` (for example, a German translation of this file would be `help/de/index.md`). If a page has no translation yet for your language, the app shows the English original with a note explaining that it isn't translated yet, rather than a blank page.

---

## Table of Contents

### Getting Up and Running

- [Getting Started](getting-started.md) -- First-time setup, system requirements, and initial configuration to get MeedyaDL running on your machine.

### Core Features

- [Downloading Music](downloading-music.md) -- How to download songs, albums, and playlists from Apple Music.
- [Downloading Videos](downloading-videos.md) -- How to download music videos and post videos, including quality options.
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Working with LRC, SRT, and TTML lyric formats, and embedding metadata into downloaded files.
- [Metadata Mapping Reference](metadata-mapping.md) -- Canonical reference for every tag MeedyaDL writes: standard MP4 atoms, Apple proprietary IDs, iTunes freeform, MeedyaMeta freeform, per-format support, and API source mapping.

### Configuration and Quality

- [Quality Settings](quality-settings.md) -- Understanding audio codecs, video codecs, and format differences.
- [Fallback Quality](fallback-quality.md) -- How fallback quality chains work and how to configure priority orders.
- [Cookie Management](cookie-management.md) -- Exporting cookies from your browser, importing them into MeedyaDL, and troubleshooting expiry issues.
- [Wrapper Authentication](wrapper.md) -- Optional wrapper-v1 (GAMDL ≤ 3.5.x) and wrapper-v2 (GAMDL 3.6+) for ALAC / Atmos / AC3 / spatial-audio downloads.
- [Animated Artwork](animated-artwork.md) -- Downloading animated cover art from Apple Music using MusicKit credentials.
- [Audio Codecs](audio-codecs.md) -- What ALAC, Dolby Atmos, AC3 and the AAC variants actually mean, and which one to pick.
- [Settings](settings.md) -- What each tab in the Settings screen controls.
- [Tools](tools.md) -- The external programs MeedyaDL installs for itself, and how to manage them.

### Reference

- [Supported Services](supported-services.md) -- Apple Music, Spotify, YouTube, BBC iPlayer — what's available and what's coming.
- [Release Channels](release-channels.md) -- Alpha, Beta, RC, and Stable channels, and how the in-app update guard keeps you on your selected tier.
- [Keyboard Shortcuts](keyboard-shortcuts.md) -- Navigation and action shortcuts for power users.

### Support

- [Troubleshooting](troubleshooting.md) -- Common errors, their solutions, and where to find log files.
- [FAQ](faq.md) -- Frequently asked questions about MeedyaDL.
- [Disclaimer](disclaimer.md) -- What MeedyaDL does and does not promise, and what is your responsibility.
- [About](about.md) -- What MeedyaDL is, who made it, its licence, and where to find it.

---

## How to Use This Documentation

Each help topic is a standalone page that you can read independently. Where relevant, pages cross-reference each other so you can easily navigate between related topics.

If you are new to MeedyaDL, we recommend starting with the [Getting Started](getting-started.md) guide and then reading through the topics in the order listed above.

---

## About MeedyaDL

MeedyaDL is a multiplatform media downloader built with [Tauri](https://tauri.app/) and [React](https://react.dev/). It supports multiple media services through a plugin-based engine architecture: Apple Music (via GAMDL) is fully available today; Spotify (via votify) is largely built but sits behind a hidden developer-only preview switch until it's ready for everyone; YouTube (via yt-dlp) and BBC iPlayer (via get_iplayer/yt-dlp) are planned for future releases.

- **License:** MIT
- **Author:** MeedyaSuite

---

## Need More Help?

If you encounter an issue not covered in this documentation:

1. Check the [Troubleshooting](troubleshooting.md) guide for common errors and solutions
2. Review the [FAQ](faq.md) for frequently asked questions
3. Open an issue on the [MeedyaDL GitHub repository](https://github.com/MWBMPartners/MeedyaDL/issues) with details about your problem, including your OS, app version, and any relevant log messages
