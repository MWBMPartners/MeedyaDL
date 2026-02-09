<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :question: Frequently Asked Questions

Answers to the most commonly asked questions about gamdl-GUI.

---

## General Questions

### What is gamdl-GUI?

gamdl-GUI is a multiplatform graphical user interface for GAMDL, an Apple Music downloader. It provides a user-friendly way to download songs, albums, playlists, and music videos from Apple Music, with options for quality, format, lyrics, and metadata.

For more information, see [Getting Started](getting-started.md).

### What platforms does gamdl-GUI support?

> *Details on supported platforms will be listed here.*

gamdl-GUI is built with Tauri and supports macOS, Windows, and Linux. See [Getting Started](getting-started.md) for installation instructions on each platform.

### Do I need an Apple Music subscription?

> *Details on subscription requirements.*

Placeholder for information on whether a subscription is required, and if so, what tier of subscription is needed for different features (e.g., lossless downloads may require a specific subscription tier).

### Is gamdl-GUI free?

> *Details on pricing and licensing.*

gamdl-GUI is open-source software licensed under the MIT License. It is free to use, modify, and distribute.

---

## Account and Authentication

### Why do I need to provide cookies?

gamdl-GUI uses your Apple Music browser cookies to authenticate with Apple's servers. This allows the application to access content on your behalf without requiring you to enter your Apple ID credentials directly into the application.

For full details, see [Cookie Management](cookie-management.md).

### How often do I need to refresh my cookies?

> *Details on cookie refresh frequency.*

Placeholder for information on how long cookies typically remain valid and how to recognize when they need to be refreshed. See [Cookie Management](cookie-management.md) for detailed guidance.

### Is my Apple ID password stored anywhere?

No. gamdl-GUI never sees, stores, or transmits your Apple ID password. Authentication is handled entirely through browser cookies, which contain session tokens rather than your credentials.

---

## Downloads

### What can I download with gamdl-GUI?

> *Summary of downloadable content types.*

Placeholder for a list of supported content types:

- Individual songs
- Full albums
- Playlists
- Music videos
- Post videos

See [Downloading Music](downloading-music.md) and [Downloading Videos](downloading-videos.md) for details.

### What audio formats are supported?

> *Summary of supported audio codecs and formats.*

Placeholder for a brief list referencing the full details in [Quality Settings](quality-settings.md):

- AAC (lossy, various bitrates)
- ALAC (lossless)
- Dolby Atmos (spatial audio)

### What video formats are supported?

> *Summary of supported video codecs and resolutions.*

Placeholder for a brief list referencing the full details in [Quality Settings](quality-settings.md):

- H.264 (AVC)
- H.265 (HEVC)
- Resolutions up to 4K (where available)

### Where are my downloaded files saved?

> *Details on the default download location.*

Placeholder for information on the default output directory, how to change it, and the folder structure used for organizing downloaded files. See [Getting Started](getting-started.md) for initial configuration.

### Can I download content from regions other than my own?

> *Details on regional availability.*

Placeholder for information on regional restrictions and whether it is possible to access content from other Apple Music storefronts.

---

## Quality and Formats

### What is the best quality I can download?

> *Summary of maximum quality options.*

Placeholder for information on the highest available quality levels for both audio and video. See [Quality Settings](quality-settings.md) for a full comparison.

### What happens if my preferred quality is not available?

gamdl-GUI includes a fallback quality system that automatically selects the next best available quality when your preferred option is unavailable. See [Fallback Quality](fallback-quality.md) for details on how this works and how to configure it.

### What is the difference between AAC and ALAC?

> *Brief explanation of lossy vs. lossless.*

Placeholder for a concise explanation:

- **AAC** is a lossy codec that compresses audio by discarding some data, resulting in smaller files. At 256 kbps, quality is excellent for most listeners.
- **ALAC** is a lossless codec that preserves the original audio data perfectly, resulting in larger files but no quality loss.

For a full comparison, see [Quality Settings](quality-settings.md).

---

## Lyrics and Metadata

### Does gamdl-GUI download lyrics?

Yes, gamdl-GUI can download synchronized lyrics in several formats (LRC, SRT, TTML). See [Lyrics and Metadata](lyrics-and-metadata.md) for details on configuring lyric downloads.

### Is metadata automatically added to downloaded files?

Yes, gamdl-GUI automatically embeds metadata (title, artist, album, artwork, etc.) into downloaded files. See [Lyrics and Metadata](lyrics-and-metadata.md) for a full list of embedded metadata fields.

### Can I edit metadata after downloading?

> *Details on post-download metadata editing.*

Placeholder for information on whether gamdl-GUI supports metadata editing after download, or recommendations for third-party metadata editors.

---

## Technical Questions

### What is Tauri?

[Tauri](https://tauri.app/) is the application framework used to build gamdl-GUI. It allows the application to run as a native desktop application on macOS, Windows, and Linux while using web technologies (React, TypeScript) for the user interface.

### What is GAMDL?

GAMDL is the command-line tool that powers the download functionality in gamdl-GUI. The GUI provides a user-friendly interface on top of GAMDL's capabilities.

### Can I use gamdl-GUI and the GAMDL CLI at the same time?

> *Details on concurrent usage.*

Placeholder for information on whether the GUI and CLI can run simultaneously, and any considerations about shared configuration or cookie files.

### How do I update gamdl-GUI?

> *Details on the update process.*

Placeholder for information on:

- How to check for updates
- Whether auto-update is supported
- How to manually download and install updates
- How to update the GAMDL backend separately

---

## Troubleshooting Quick Reference

### My download keeps failing. What should I do?

Try these steps in order:

1. Verify your cookies are valid (see [Cookie Management](cookie-management.md))
2. Check your internet connection
3. Try a different quality setting (see [Quality Settings](quality-settings.md))
4. Check the log files for specific error messages (see [Troubleshooting](troubleshooting.md))
5. If the issue persists, report it as a bug (see [Troubleshooting](troubleshooting.md#reporting-a-bug))

### Where can I get more help?

If your question is not answered here, check the full [Troubleshooting](troubleshooting.md) guide. You can also open an issue on the project's GitHub repository for support.

---

## Related Topics

- [Getting Started](getting-started.md) -- First-time setup guide
- [Quality Settings](quality-settings.md) -- Detailed quality and format information
- [Cookie Management](cookie-management.md) -- Authentication and cookie setup
- [Troubleshooting](troubleshooting.md) -- Error resolution and diagnostics

---

[Back to Help Index](index.md)
