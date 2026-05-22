<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :book: MeedyaDL Help Center

Welcome to the **MeedyaDL** help documentation. This guide covers everything you need to know about using MeedyaDL, a multiplatform media downloader.

---

## Table of Contents

### Getting Up and Running

- [:rocket: Getting Started](getting-started.md) -- First-time setup, system requirements, and initial configuration to get MeedyaDL running on your machine.

### Core Features

- [:musical_note: Downloading Music](downloading-music.md) -- How to download songs, albums, and playlists from Apple Music.
- [:clapper: Downloading Videos](downloading-videos.md) -- How to download music videos and post videos, including quality options.
- [:pencil2: Lyrics and Metadata](lyrics-and-metadata.md) -- Working with LRC, SRT, and TTML lyric formats, and embedding metadata into downloaded files.
- [:bookmark_tabs: Metadata Mapping Reference](metadata-mapping.md) -- Canonical reference for every tag MeedyaDL writes: standard MP4 atoms, Apple proprietary IDs, iTunes freeform, MeedyaMeta freeform, per-format support, and API source mapping.

### Configuration and Quality

- [:control_knobs: Quality Settings](quality-settings.md) -- Understanding audio codecs, video codecs, and format differences.
- [:arrows_counterclockwise: Fallback Quality](fallback-quality.md) -- How fallback quality chains work and how to configure priority orders.
- [:cookie: Cookie Management](cookie-management.md) -- Exporting cookies from your browser, importing them into MeedyaDL, and troubleshooting expiry issues.
- [:film_frames: Animated Artwork](animated-artwork.md) -- Downloading animated cover art from Apple Music using MusicKit credentials.

### Reference

- [:globe_with_meridians: Supported Services](supported-services.md) -- Apple Music, Spotify, YouTube, BBC iPlayer — what's available and what's coming.
- [:twisted_rightwards_arrows: Release Channels](release-channels.md) -- Nightly, Weekly, Monthly, Alpha, Beta, and Stable channels, and how the in-app update guard keeps you on your selected tier.
- [:keyboard: Keyboard Shortcuts](keyboard-shortcuts.md) -- Navigation and action shortcuts for power users.

### Support

- [:wrench: Troubleshooting](troubleshooting.md) -- Common errors, their solutions, and where to find log files.
- [:question: FAQ](faq.md) -- Frequently asked questions about MeedyaDL.

---

## How to Use This Documentation

Each help topic is a standalone page that you can read independently. Where relevant, pages cross-reference each other so you can easily navigate between related topics.

If you are new to MeedyaDL, we recommend starting with the [Getting Started](getting-started.md) guide and then reading through the topics in the order listed above.

---

## About MeedyaDL

MeedyaDL is a multiplatform media downloader built with [Tauri](https://tauri.app/) and [React](https://react.dev/). It supports multiple media services through a plugin-based engine architecture: Apple Music (via GAMDL), with Spotify (via Votify), YouTube (via yt-dlp), and BBC iPlayer (via get_iplayer/yt-dlp) planned for future releases.

- **License:** MIT
- **Author:** MeedyaDL

---

## Need More Help?

If you encounter an issue not covered in this documentation:

1. Check the [Troubleshooting](troubleshooting.md) guide for common errors and solutions
2. Review the [FAQ](faq.md) for frequently asked questions
3. Open an issue on the [MeedyaDL GitHub repository](https://github.com/MWBMPartners/MeedyaDL/issues) with details about your problem, including your OS, app version, and any relevant log messages
