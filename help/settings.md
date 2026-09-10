<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Settings

MeedyaDL's Settings screen has 11 tabs, grouped into five sections in the sidebar: General, Download (Codec & Resolution, Codec Fallback Order, Lyrics, Cover Art, Metadata, Templates), Authentication (Cookies), Services (Spotify), and System (Tools, Advanced). This page covers what each one actually controls.

## General

- **Output** -- Where downloaded files are saved (default: an "Apple Music" folder inside your system's music directory).
- **Preferences** -- Auto-start queue, clipboard monitoring, notifications (desktop notifications on/off, the three-way notification style, and how long auto-dismissing toasts stay on screen), smart re-download detection, and after-queue actions (do nothing, open the output folder, play a sound, close the app, or put/restart/shut down the computer).
- **Appearance** -- Theme, language, and other display preferences.
- **Backup** -- Export your settings, queue, and other app state to a single file.
- **Restore from .meedyabundle** -- Import a previously exported backup file.
- **Profile Bundle** -- Diagnostic bundle export for sharing your configuration with support, with credentials handled carefully.

## Codec & Resolution

- **Audio Quality** -- Default audio codec (ALAC lossless by default), Companion Downloads mode (automatically download extra format versions alongside your primary download, e.g. Atmos + a lossless companion), and duplicate-detection preferences for artist downloads.
- **Video Quality** -- Default video resolution (2160p/4K by default, treated as a ceiling -- Apple Music returns the closest quality at or below it), music video companion downloads (requires MusicKit credentials), and MusicBrainz video discovery as a fallback when Apple Music's own API doesn't find a music video.
- **Enable Fallback Chain** -- One switch that covers both songs and music videos. When it's on, an unavailable codec steps down through your configured chain (see below); when it's off, only the first choice in each chain is ever tried.

## Codec Fallback Order

Reorder the audio codec and video codec fallback chains using the up/down arrow buttons on each row, or remove an option entirely so it's never used as a fallback (useful if you never want, say, AAC Binaural mixed into your library, or H.264 music videos). The chain persists across restarts and applies automatically whenever your preferred codec isn't available for a track or video. There is no resolution fallback chain here -- video resolution is a ceiling, not something with a fallback order; see [Fallback Quality](fallback-quality.md) for why.

## Tools

- **Core Dependencies** -- Status, install, and update controls for Python and GAMDL, including GAMDL version management (which validated version range MeedyaDL supports, and whether to upgrade).
- **External Tools** -- The five required tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, MediaInfo) plus the optional rclone tool (only needed if you turn on direct-to-cloud upload), each with status, install, and a **Configure custom binary path** option if you'd rather point MeedyaDL at your own copy.
- **Directories** -- The temporary working directory GAMDL uses during a download (default: your OS temp folder plus `MeedyaDL`).
- **Backups / Restore from snapshot** -- Tool-related backup and restore actions, separate from the General tab's whole-app backup.

## Cookies

**Authentication Cookies** -- Import your Apple Music cookies (via the built-in login window, automatic browser detection, or manual file import), check expiry, and re-authenticate when they run out. This is how MeedyaDL proves to Apple Music that it's you, without a wrapper.

## Lyrics

**Synced Lyrics** -- Which lyric formats to generate (Enhanced LRC, Rich SRT, WebVTT, ASS, and an experimental `.lyrics` YAML sidecar format used by tools such as LRCGET), whether to embed them in the audio file as well as saving a sidecar file, and a **Test word-level lyrics connection** button next to the Enhanced Lyrics toggle. That button checks whether MeedyaDL can currently fetch word-level (syllable) lyrics from Apple Music -- without waiting for a full download. It resolves your MusicKit developer token the same way a real download does (your own MusicKit credentials, falling back to the developer token captured from your Apple Music web-player session if you haven't configured your own), reads the Media-User-Token from your imported cookies, and probes Apple's syllable-lyrics endpoint against a known song. A green result means word-level timing came back and Enhanced LRC will work (noting when it succeeded via your web-player session rather than configured credentials); an amber result means the endpoint responded but only with line-level timing; anything else comes with guidance on what to fix -- signing in to Apple Music, configuring MusicKit credentials in Settings > Advanced, or re-importing cookies.

## Cover Art

- **Cover Art** -- Whether to save cover art at all, its format and size, filename (Front Cover / Cover / Folder), and **Upgrade Cover Art From Other Services** -- an opt-in switch that, once a download already has a saved cover image, checks whether another service (currently Deezer, matched by the release's barcode) has a bigger picture and replaces the saved file if so. It only ever swaps the saved image file for a bigger one; it never touches the artwork already embedded in a track. Off by default, because it means contacting a third-party service for every album.
- **Animated Artwork** -- Downloading Apple Music's animated cover art and artist promo videos via MusicKit, and whether to hide those files from normal file browsing (they're implementation detail, not something most people want cluttering a music folder).

## Metadata

- **Automatic Tags** -- Codec, source, and channel-configuration tags, plus the dual Apple Music API metadata fetch (iTunes Lookup, then the richer Apple Music Catalog API).
- **AcoustID Fingerprinting** -- Opt-in audio fingerprinting and lookup against the AcoustID database.
- **ReplayGain Analysis** -- Opt-in loudness analysis so your library plays back at a consistent volume.
- **Links on Other Music Services** -- An opt-in **Links on Other Music Services** toggle. When it's on, MeedyaDL asks song.link (a lookup service run by a company called Odesli) where else each downloaded album is available -- Spotify, YouTube Music, Tidal, and others -- and saves what it finds into your files. It needs an access key from Odesli, entered under Settings > Advanced > API Credentials.

## Templates

- **Folder Templates** -- How output folders are named, using variables like `{artist}`, `{album}`, `{platform}`.
- **File Templates** -- How individual files are named, using variables like `{title}`, `{track:02d}`.

## Spotify

Spotify support is largely built but sits behind a hidden developer-only preview switch until it's ready for regular users -- pasting a Spotify link is only accepted as far as the safety checks described here; it does not yet produce a finished download for a normal user.

- **Session** -- Spotify sign-in (cookie-based, the same shape as Apple Music, not OAuth).
- **Risk acknowledgement / Anti-ban safeguards / Daily cap status** -- Spotify enforces stricter anti-automation rules than Apple Music. This tab surfaces a first-run consent step, a daily download cap, and status on where you stand against that cap, so MeedyaDL doesn't put your Spotify account at risk.

## Advanced

- **Setup** -- Download mode (which underlying tool fetches encrypted streams from Apple's CDN) and remux mode (which tool repackages the finished file) -- both apply to Apple Music downloads.
- **Processing** -- File-handling options that affect how downloads are processed.
- **File Options** -- Additional file-handling preferences.
- **Wrapper / Sign in to wrapper** -- Optional alternative authentication for Apple Music that doesn't rely on cookies. MeedyaDL supports two wrapper generations depending on your GAMDL version -- see [Wrapper Authentication](wrapper.md) for the full picture, including the three separate addresses wrapper-v1 uses and the single address wrapper-v2 uses, plus auto-retry-without-wrapper and connectivity testing.
- **API Credentials** -- Your own MusicKit developer credentials (Team ID, Key ID, private key) for premium features, an AcoustID key override, and the song.link (Odesli) access key used by the Metadata tab's cross-platform links feature.
- **Error Reporting** -- Local crash reports are always kept on your own machine. This section's toggle switches on optional, anonymous crash reporting to help us fix bugs -- off by default, no personal data, download history, or library information ever included -- alongside one-click reporting of a specific error to GitHub Issues (which opens a pre-filled issue in your browser after showing you exactly what would be sent).
- **Diagnostics** -- Verbose activity logging, the on-disk activity log's location and export, and other troubleshooting aids.
- **Developer Tools** -- Only visible after the hidden developer-access unlock; shows internal diagnostic information not needed for normal use.

---

## Related Topics

- [Getting Started](getting-started.md) -- First-time setup guide
- [Quality Settings](quality-settings.md) -- Codec and resolution details
- [Fallback Quality](fallback-quality.md) -- How the fallback chain works
- [Cookie Management](cookie-management.md) -- Cookie import and troubleshooting
- [Wrapper Authentication](wrapper.md) -- Wrapper setup and troubleshooting
- [Tools](tools.md) -- The external programs MeedyaDL manages

[Back to Help Index](index.md)
