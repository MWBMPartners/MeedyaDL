<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :pencil2: Lyrics and Metadata

This guide explains how gamdl-GUI handles lyrics in various formats and how metadata is embedded into downloaded files.

---

## Overview

gamdl-GUI can download synchronized lyrics alongside your music and embed rich metadata (title, artist, album art, etc.) directly into downloaded files. This ensures your media library is well-organized and your music player can display lyrics in real time.

---

## Lyric Formats

### LRC (LyRiCs)

> *Details on the LRC lyric format and how gamdl-GUI uses it.*

Placeholder for information on:

- What the LRC format is and which music players support it
- How LRC files are generated and saved alongside audio files
- The structure of LRC files (timestamps, text lines)
- Whether enhanced LRC (word-level sync) is supported

### SRT (SubRip Subtitle)

> *Details on the SRT subtitle format and its use with music videos.*

Placeholder for information on:

- What the SRT format is and its typical use cases
- How SRT files relate to video downloads (see [Downloading Videos](downloading-videos.md))
- The structure of SRT files (sequence numbers, timestamps, text)
- Compatibility with common video players

### TTML (Timed Text Markup Language)

> *Details on the TTML format and Apple's use of it.*

Placeholder for information on:

- What TTML is and why Apple Music uses it
- How TTML lyrics are processed by gamdl-GUI
- Whether TTML files are saved directly or converted to other formats
- The XML structure of TTML files

---

## Configuring Lyric Downloads

### Enabling and Disabling Lyrics

> *How to control whether lyrics are downloaded with your music.*

Placeholder for details on the lyrics toggle in gamdl-GUI settings and how to enable or disable lyric downloads on a per-download basis.

### Choosing a Lyric Format

> *How to select your preferred lyric output format.*

Placeholder for details on selecting between LRC, SRT, and TTML formats, and guidance on which format to choose based on your music player or workflow.

### Lyric File Placement

> *Where lyric files are saved relative to the downloaded media.*

Placeholder for details on:

- Default lyric file location (same directory as the audio/video file)
- File naming conventions for lyric files
- Options for embedding lyrics directly into the media file versus saving as a sidecar file

---

## Metadata Embedding

### What Metadata Is Embedded

> *A list of all metadata fields that gamdl-GUI embeds into downloaded files.*

Placeholder for a comprehensive list, which may include:

- Title
- Artist / Album Artist
- Album
- Track Number / Disc Number
- Genre
- Release Date / Year
- Album Artwork (cover art)
- Composer / Songwriter
- Copyright information
- Apple Music catalog identifiers
- Lyrics (when embedded directly)

### Audio File Metadata

> *How metadata is embedded in audio files.*

Placeholder for details on metadata tagging for different audio formats (M4A, FLAC, etc.) and which tagging standards are used (e.g., MP4 atoms, Vorbis comments).

### Video File Metadata

> *How metadata is embedded in video files.*

Placeholder for details on metadata tagging for video files. See also [Downloading Videos](downloading-videos.md) for video-specific output information.

---

## Album Artwork

### Artwork Resolution

> *Details on the resolution and quality of embedded album artwork.*

Placeholder for information on artwork resolution options, file size considerations, and how artwork is sourced from Apple Music.

### Artwork as Separate Files

> *Whether album artwork can be saved as standalone image files.*

Placeholder for details on saving cover art as a separate image file (e.g., `cover.jpg`) in addition to embedding it in the media file.

---

## Troubleshooting Lyrics and Metadata

> *Common issues with lyrics and metadata and how to resolve them.*

Placeholder for common scenarios such as:

- Lyrics not available for a particular track
- Metadata fields appearing blank or incorrect
- Album artwork not displaying in your music player
- Encoding issues with special characters in lyrics

For general troubleshooting, see [Troubleshooting](troubleshooting.md).

---

## Related Topics

- [Downloading Music](downloading-music.md) -- How audio downloads work
- [Downloading Videos](downloading-videos.md) -- How video downloads and subtitles work
- [Quality Settings](quality-settings.md) -- Audio and video format options that affect metadata capabilities
- [Troubleshooting](troubleshooting.md) -- General error resolution

---

[Back to Help Index](index.md)
