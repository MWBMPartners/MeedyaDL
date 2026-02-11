<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024-2026 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :question: Frequently Asked Questions

Answers to the most commonly asked questions about gamdl-GUI.

---

## General Questions

### What is gamdl-GUI?

gamdl-GUI is a multiplatform graphical user interface for GAMDL, built with Tauri 2.0, React, and TypeScript. It provides a user-friendly way to download songs, albums, playlists, artist discographies, and music videos from Apple Music, with options for quality, format, lyrics, and metadata. gamdl-GUI is open-source software developed by MWBM Partners Ltd and released under the MIT License.

For more information, see [Getting Started](getting-started.md).

### What platforms does gamdl-GUI support?

gamdl-GUI supports the following platforms:

- **macOS** 11.0 (Big Sur) or later -- Apple Silicon (M-series)
- **Windows** -- x64 and ARM64
- **Linux** -- x64
- **Raspberry Pi** -- ARM64

See [Getting Started](getting-started.md) for installation instructions on each platform.

### Do I need an Apple Music subscription?

Yes, a valid Apple Music subscription is required to use gamdl-GUI. Any subscription tier that grants access to the Apple Music catalog will work. The subscription is used for authentication via cookies -- without an active subscription, the app cannot access content from Apple's servers.

### Is gamdl-GUI free?

Yes. gamdl-GUI is free and open-source software licensed under the MIT License. It is free to use, modify, and distribute. There are no paid tiers, subscriptions, or in-app purchases. You do, however, need your own Apple Music subscription separately.

---

## Account and Authentication

### Why do I need to provide cookies?

Apple Music requires authentication to access its content catalog. Cookies are session tokens exported from your web browser after you sign in to Apple Music. By providing these cookies, the app can authenticate with Apple's servers on your behalf. Importantly, your Apple ID password is never stored or transmitted by the app -- only the browser session tokens are used.

For full details on how to export and import cookies, see [Cookie Management](cookie-management.md).

### How often do I need to refresh my cookies?

Cookies typically remain valid for 1 to 12 months, depending on your browser and session settings. gamdl-GUI displays expiry warnings within the app when your cookies are approaching expiration or have already expired. When that happens, simply re-export your cookies from your browser and re-import them into gamdl-GUI.

See [Cookie Management](cookie-management.md) for detailed guidance on the export and import process.

### Is my Apple ID password stored anywhere?

No. Never. gamdl-GUI never sees, stores, or transmits your Apple ID password. Authentication is handled entirely through browser cookies, which contain session tokens rather than your credentials. Your password remains solely within your browser and Apple's servers.

---

## Downloads

### What can I download with gamdl-GUI?

gamdl-GUI supports downloading the following content types from Apple Music:

- Individual songs
- Full albums
- Playlists
- Artist discographies
- Music videos

See [Downloading Music](downloading-music.md) and [Downloading Videos](downloading-videos.md) for details on each content type.

### What audio formats are supported?

gamdl-GUI supports the following audio formats:

- **AAC** -- 256 kbps lossy compression, the standard Apple Music format
- **AAC-HE** -- High Efficiency AAC for lower bitrate streaming
- **AAC Binaural** -- Binaural rendering of spatial audio for headphone listening
- **AAC Legacy** -- Legacy AAC encoding for older device compatibility
- **ALAC** -- Apple Lossless Audio Codec, lossless up to 24-bit/192kHz
- **Atmos** -- Dolby Atmos spatial audio
- **AC3** -- Dolby Digital 5.1 surround sound

For a full comparison of formats and quality levels, see [Quality Settings](quality-settings.md).

### What video formats are supported?

Music videos are downloaded in the MP4 container format. Supported resolutions range from 240p up to 4K (2160p), depending on content availability. Not all music videos are available at every resolution.

For full details on video quality options, see [Quality Settings](quality-settings.md).

### Where are my downloaded files saved?

The download location is configurable in **Settings > Paths** tab. By default, files are saved to your system's music directory. Downloaded files are organized using GAMDL's template system in an Artist/Album/Track folder structure, which you can customize in **Settings > Templates**.

See [Getting Started](getting-started.md) for initial configuration.

### Can I download content from regions other than my own?

Content availability depends on your Apple Music account's region. The app downloads whatever content is available to your account. If a song, album, or music video is not available in your region's Apple Music catalog, it will not be accessible for download through gamdl-GUI.

---

## Quality and Formats

### What is the best quality I can download?

The maximum quality levels available are:

- **Audio**: ALAC at 24-bit/192kHz (Hi-Res Lossless). This provides the highest fidelity audio reproduction available on Apple Music.
- **Video**: 2160p (4K) resolution.

Note that not all content is available at maximum quality. When the highest quality is not available for a particular track or video, the fallback system handles this automatically by selecting the next best option.

See [Quality Settings](quality-settings.md) for a full comparison of all quality tiers.

### What happens if my preferred quality is not available?

gamdl-GUI includes a fallback quality system that automatically selects the next best available quality when your preferred option is unavailable. The fallback chain is configurable, so you control which alternatives the app tries and in what order. See [Fallback Quality](fallback-quality.md) for details on how this works and how to configure it.

### What is the difference between AAC and ALAC?

- **AAC** (Advanced Audio Coding) is a lossy codec that compresses audio by discarding data deemed less perceptible to human hearing. At Apple Music's standard 256 kbps, quality is excellent for everyday listening. Files are relatively small at roughly 2 MB per minute of audio.
- **ALAC** (Apple Lossless Audio Codec) is a lossless codec that preserves the original audio data perfectly with no quality loss. Files are significantly larger at roughly 5-15 MB per minute (depending on bit depth and sample rate), but the audio is an exact reproduction of the master. Best suited for audiophile listening and archival purposes.

For a full comparison of all supported formats, see [Quality Settings](quality-settings.md).

---

## Lyrics and Metadata

### Does gamdl-GUI download lyrics?

Yes, gamdl-GUI can download synchronized lyrics in several formats including LRC, SRT, and TTML. Lyric download preferences are configurable in **Settings > Lyrics** tab. See [Lyrics and Metadata](lyrics-and-metadata.md) for details on configuring lyric downloads and choosing the right format.

### Is metadata automatically added to downloaded files?

Yes, gamdl-GUI automatically embeds metadata into downloaded files, including title, artist, album, album artwork, track numbers, disc numbers, genre, release date, and more. See [Lyrics and Metadata](lyrics-and-metadata.md) for a full list of embedded metadata fields.

### Can I edit metadata after downloading?

gamdl-GUI does not include a built-in metadata editor. If you need to modify metadata after downloading, use a third-party metadata editing tool such as:

- **[MusicBrainz Picard](https://picard.musicbrainz.org/)** -- Free, open-source, cross-platform music tagger with database lookup
- **[Mp3tag](https://www.mp3tag.de/)** -- Powerful metadata editor for Windows (also available on macOS)
- **[Kid3](https://kid3.kde.org/)** -- Free, cross-platform audio tag editor

---

## Technical Questions

### What is Tauri?

[Tauri](https://tauri.app/) is the application framework used to build gamdl-GUI. Similar in concept to Electron, but significantly lighter and more efficient, Tauri uses a Rust backend combined with a web-based frontend (React and TypeScript in gamdl-GUI's case). This approach results in small, fast, native desktop applications with low memory and disk usage compared to Electron-based alternatives.

### What is GAMDL?

GAMDL is a command-line Apple Music download tool created by glomatico. It handles the core download functionality -- authentication, content fetching, decryption, and file writing. gamdl-GUI provides a friendly graphical interface on top of GAMDL's capabilities. GAMDL is installed automatically during gamdl-GUI's first-run setup, so you do not need to install it separately.

### Can I use gamdl-GUI and the GAMDL CLI at the same time?

This is not recommended. Running the GUI and CLI simultaneously may cause conflicts over shared cookie files or output directories, leading to authentication errors or corrupted downloads. Use one at a time to avoid issues.

### How do I update gamdl-GUI?

gamdl-GUI checks for updates automatically in two ways:

- **App updates**: The application checks GitHub Releases for new versions of gamdl-GUI itself. When an update is available, a banner appears in the app with upgrade and dismiss actions.
- **GAMDL updates**: The app checks PyPI for new versions of the GAMDL backend. When an update is available, GAMDL can be upgraded with one click directly from the update banner.

No manual intervention is needed -- simply follow the prompts when the update banner appears.

---

## Troubleshooting Quick Reference

### My download keeps failing. What should I do?

Try these steps in order:

1. Verify your cookies are still valid (see [Cookie Management](cookie-management.md))
2. Check your internet connection
3. Try a different quality setting (see [Quality Settings](quality-settings.md))
4. Check the log files for specific error messages (see [Troubleshooting](troubleshooting.md))
5. If the issue persists, report it as a bug (see [Troubleshooting](troubleshooting.md#reporting-a-bug))

### Where can I get more help?

If your question is not answered here, check the full [Troubleshooting](troubleshooting.md) guide. You can also open an issue on the project's [GitHub Issues](https://github.com/MWBM-Partners-Ltd/gamdl-GUI/issues) page for support.

---

## Related Topics

- [Getting Started](getting-started.md) -- First-time setup guide
- [Quality Settings](quality-settings.md) -- Detailed quality and format information
- [Cookie Management](cookie-management.md) -- Authentication and cookie setup
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Lyric formats and metadata fields
- [Troubleshooting](troubleshooting.md) -- Error resolution and diagnostics

---

[Back to Help Index](index.md)
