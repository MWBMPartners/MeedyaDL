<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Settings

## Configuration Guide

### General
- **Output Directory** - Where files are saved (default: ~/Music/Apple Music)
- **Language** - Metadata language preference
- **Overwrite** - Whether to replace existing files

### Quality
- **Audio Codec** - Default: ALAC (lossless). Options range from lossless to compressed AAC variants
- **Video Resolution** - Default: 2160p (4K). Treated as a maximum: Apple Music returns the closest quality at or below it
- **Fallback** - Enable/disable trying the next audio codec when your preferred one isn't available

### Lyrics

Settings > Lyrics includes a **Test word-level lyrics connection** button next to the Enhanced Lyrics toggle. It checks whether MeedyaDL can currently fetch word-level (syllable) lyrics from Apple Music -- without waiting for a full download. The test resolves your MusicKit developer token the same way a real download does (your own MusicKit credentials, falling back to the developer token captured from your Apple Music web-player session if you haven't configured your own), reads the Media-User-Token from your imported cookies, and probes Apple's syllable-lyrics endpoint against a known song. A green result means word-level timing came back and Enhanced LRC will work (noting when it succeeded via your web-player session rather than configured credentials); an amber result means the endpoint responded but only with line-level timing; anything else comes with guidance on what to fix -- signing in to Apple Music, configuring MusicKit credentials in Settings > Advanced, or re-importing cookies.

### Metadata

Settings > Metadata includes an opt-in **Links on Other Music Services** toggle. When it's on, MeedyaDL asks song.link (a lookup service run by a company called Odesli) where else each downloaded album is available -- Spotify, YouTube Music, Tidal, and others -- and saves what it finds into your files. It needs an access key from Odesli, entered under Settings > Advanced > API Credentials.

### Paths
Override paths to external tools. Leave empty to use the managed (auto-installed) versions.

### Templates
Customize how files and folders are named using template variables like `{artist}`, `{album}`, `{title}`, `{track:02d}`.
