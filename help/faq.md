<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :question: Frequently Asked Questions

Answers to the most commonly asked questions about MeedyaDL.

---

## General Questions

### What is MeedyaDL?

MeedyaDL is a multiplatform graphical user interface for GAMDL, built with Tauri 2.0, React, and TypeScript. It provides a user-friendly way to download songs, albums, playlists, artist discographies, and music videos from Apple Music, with options for quality, format, lyrics, and metadata. MeedyaDL is open-source software developed by MeedyaDL and released under the MIT License.

For more information, see [Getting Started](getting-started.md).

### What platforms does MeedyaDL support?

MeedyaDL supports the following platforms:

- **macOS** 13.3 (Ventura) or later -- Apple Silicon (M-series)
- **Windows** -- x64 and ARM64
- **Linux** -- x64
- **Raspberry Pi** -- ARM64

See [Getting Started](getting-started.md) for installation instructions on each platform.

### Do I need an Apple Music subscription?

Yes, a valid Apple Music subscription is required to use MeedyaDL. Any subscription tier that grants access to the Apple Music catalog will work. The subscription is used for authentication via cookies -- without an active subscription, the app cannot access content from Apple's servers.

### Is MeedyaDL free?

Yes. MeedyaDL is free and open-source software licensed under the MIT License. It is free to use, modify, and distribute. There are no paid tiers, subscriptions, or in-app purchases. You do, however, need your own Apple Music subscription separately.

---

## Account and Authentication

### Why do I need to provide cookies?

Apple Music requires authentication to access its content catalog. Cookies are session tokens exported from your web browser after you sign in to Apple Music. By providing these cookies, the app can authenticate with Apple's servers on your behalf. Importantly, your Apple ID password is never stored or transmitted by the app -- only the browser session tokens are used.

For full details on how to export and import cookies, see [Cookie Management](cookie-management.md).

### How often do I need to refresh my cookies?

Cookies typically remain valid for 1 to 12 months, depending on your browser and session settings. MeedyaDL displays expiry warnings within the app when your cookies are approaching expiration or have already expired. When that happens, simply re-export your cookies from your browser and re-import them into MeedyaDL.

See [Cookie Management](cookie-management.md) for detailed guidance on the export and import process.

### Is my Apple ID password stored anywhere?

No. Never. MeedyaDL never sees, stores, or transmits your Apple ID password. Authentication is handled entirely through browser cookies, which contain session tokens rather than your credentials. Your password remains solely within your browser and Apple's servers.

---

## Downloads

### What can I download with MeedyaDL?

MeedyaDL supports downloading the following content types from Apple Music:

- Individual songs
- Full albums
- Playlists
- Artist discographies
- Music videos

See [Downloading Music](downloading-music.md) and [Downloading Videos](downloading-videos.md) for details on each content type.

### What audio formats are supported?

MeedyaDL supports the following audio formats:

- **AAC** -- 256 kbps lossy compression, the standard Apple Music format
- **AAC-HE** -- High Efficiency AAC for lower bitrate streaming
- **AAC Binaural** -- Binaural rendering of spatial audio for headphone listening
- **AAC Legacy** -- Legacy AAC encoding for older device compatibility
- **ALAC** -- Apple Lossless Audio Codec, lossless up to 24-bit/192kHz
- **Atmos** -- Dolby Atmos spatial audio
- **AC3** -- Dolby Digital 5.1 surround sound

For a full comparison of formats and quality levels, see [Quality Settings](quality-settings.md).

### What video formats are supported?

Music videos are downloaded in the MP4 container format. Supported resolutions range from 240p up to 4K UHD (2160p), depending on content availability. Not all music videos are available at every resolution.

For full details on video quality options, see [Quality Settings](quality-settings.md).

### Where are my downloaded files saved?

The download location is configurable in **Settings > Paths** tab. By default, files are saved to your system's music directory. Downloaded files are organized using GAMDL's template system in an Artist/Album/Track folder structure, which you can customize in **Settings > Templates**. Intermediate files during download and processing are stored in a temporary directory (default: `{OS temp}/MeedyaDL`), also configurable in **Settings > Paths**.

See [Getting Started](getting-started.md) for initial configuration.

### Can I download content from regions other than my own?

Content availability depends on your Apple Music account's region. The app downloads whatever content is available to your account. If a song, album, or music video is not available in your region's Apple Music catalog, it will not be accessible for download through MeedyaDL.

### What is a .meedyadl file?

A `.meedyadl` file is a download manifest that MeedyaDL saves in each album's output folder after a successful download. It contains the source Apple Music URLs and per-track metadata, allowing you to re-download the same content later without looking up URLs or reconfiguring settings. You can re-import a manifest by clicking the **Import** button on the Download page, dragging the file onto the app window, or using the Queue Import feature. See [Downloading Music](downloading-music.md#download-manifests-meedyadl-files) for full details.

### How do I clear the download queue?

The Queue page provides two options for clearing items:

- **Clear Completed** -- Removes only completed and cancelled items from the queue, keeping active, queued, and failed items so you can review errors and retry.
- **Clear All** -- Removes all items from the queue regardless of status, including failed items. Active downloads are cancelled before being removed.

Both buttons are in the queue header. If you want to keep failed items visible for review, use **Clear Completed** instead of **Clear All**.

### What is smart re-download detection?

When you re-download an album you have previously downloaded, MeedyaDL automatically checks the Apple Music API to see if the album has changed since your last download. If the album was previously downloaded, an info toast shows the date of the original download. The feature detects audio quality upgrades (such as Atmos or Lossless becoming available), tracks being added, metadata corrections, and Apple Digital Master certification. It cannot detect silent audio remasters where the audio files change but the catalog metadata stays the same. This feature is enabled by default and can be toggled in **Settings > General > Preferences**. See [Downloading Music](downloading-music.md#smart-re-download-detection) for full details.

### What is clipboard monitoring?

When clipboard monitoring is enabled, MeedyaDL watches your system clipboard for supported URLs (e.g., Apple Music). If you copy an Apple Music URL from a browser or messaging app, a notification appears — clicking **Download** adds it directly to the queue. When the app window is not focused, a native OS notification is sent instead. This is a convenience feature — it only checks for URL patterns and never stores clipboard contents. Enabled by default; toggle in **Settings > General > Preferences**. See [Downloading Music](downloading-music.md#clipboard-monitoring) for details.

### How does the Activity Log work?

The Activity Log shows real-time output from all downloads and system events. It auto-scrolls to the bottom by default — if you scroll up to read earlier entries, the **Auto-scroll** checkbox in the toolbar automatically unchecks. Re-check it to jump back to the bottom and resume auto-scrolling. The log retains up to 10,000 entries per session (oldest entries are trimmed when the limit is reached). Use the **Export** button to save the full log to a file before it's trimmed. Filtering by category (System, Download, Verbose) and text search are available in the toolbar.

### Does MeedyaDL support library URLs?

Yes. MeedyaDL accepts personal library URLs that use the `music.apple.com/library/...` path format. These point to content in your own iCloud Music Library. Paste them into the download form the same way you would paste a catalog URL. This is useful for downloading content you have added to your library, including items that may have been removed from the public Apple Music catalog. See [Downloading Music](downloading-music.md#library-urls) for details.

---

## Quality and Formats

### What is the best quality I can download?

The maximum quality levels available are:

- **Audio**: ALAC at 24-bit/192kHz (Hi-Res Lossless). This provides the highest fidelity audio reproduction available on Apple Music.
- **Video**: 2160p (4K) resolution.

Note that not all content is available at maximum quality. When the highest quality is not available for a particular track or video, the fallback system handles this automatically by selecting the next best option.

See [Quality Settings](quality-settings.md) for a full comparison of all quality tiers.

### What happens if my preferred quality is not available?

MeedyaDL includes a fallback quality system that automatically selects the next best available quality when your preferred option is unavailable. The fallback chain is configurable, so you control which alternatives the app tries and in what order. See [Fallback Quality](fallback-quality.md) for details on how this works and how to configure it.

### What is the difference between AAC and ALAC?

- **AAC** (Advanced Audio Coding) is a lossy codec that compresses audio by discarding data deemed less perceptible to human hearing. At Apple Music's standard 256 kbps, quality is excellent for everyday listening. Files are relatively small at roughly 2 MB per minute of audio.
- **ALAC** (Apple Lossless Audio Codec) is a lossless codec that preserves the original audio data perfectly with no quality loss. Files are significantly larger at roughly 5-15 MB per minute (depending on bit depth and sample rate), but the audio is an exact reproduction of the master. Best suited for audiophile listening and archival purposes.

For a full comparison of all supported formats, see [Quality Settings](quality-settings.md).

---

## Lyrics and Metadata

### Does MeedyaDL download lyrics?

Yes, MeedyaDL can download synchronized lyrics in several formats including LRC, SRT, and TTML. Lyric download preferences are configurable in **Settings > Lyrics** tab. See [Lyrics and Metadata](lyrics-and-metadata.md) for details on configuring lyric downloads and choosing the right format.

### Is metadata automatically added to downloaded files?

Yes, MeedyaDL automatically embeds metadata into downloaded files, including title, artist, album, album artwork, track numbers, disc numbers, genre, release date, and more. See [Lyrics and Metadata](lyrics-and-metadata.md) for a full list of embedded metadata fields.

### Can I edit metadata after downloading?

MeedyaDL does not include a built-in metadata editor. If you need to modify metadata after downloading, use a third-party metadata editing tool such as:

- **[MusicBrainz Picard](https://picard.musicbrainz.org/)** -- Free, open-source, cross-platform music tagger with database lookup
- **[Mp3tag](https://www.mp3tag.de/)** -- Powerful metadata editor for Windows (also available on macOS)
- **[Kid3](https://kid3.kde.org/)** -- Free, cross-platform audio tag editor

---

## Technical Questions

### What is Tauri?

[Tauri](https://tauri.app/) is the application framework used to build MeedyaDL. Similar in concept to Electron, but significantly lighter and more efficient, Tauri uses a Rust backend combined with a web-based frontend (React and TypeScript in MeedyaDL's case). This approach results in small, fast, native desktop applications with low memory and disk usage compared to Electron-based alternatives.

### What is GAMDL?

GAMDL is a command-line Apple Music download tool created by glomatico. It handles the core download functionality -- authentication, content fetching, decryption, and file writing. MeedyaDL provides a friendly graphical interface on top of GAMDL's capabilities. GAMDL is installed automatically during MeedyaDL's first-run setup, so you do not need to install it separately.

### Can I use MeedyaDL and the GAMDL CLI at the same time?

This is not recommended. Running the GUI and CLI simultaneously may cause conflicts over shared cookie files or output directories, leading to authentication errors or corrupted downloads. Use one at a time to avoid issues.

### How do I update MeedyaDL?

MeedyaDL checks for updates automatically in two ways:

- **App updates**: The application checks GitHub Releases for new versions of MeedyaDL itself. When an update is available, a banner appears in the app with upgrade and dismiss actions.
- **GAMDL updates**: The app checks PyPI for new versions of the GAMDL backend. When an update is available, GAMDL can be upgraded with one click directly from the update banner.

No manual intervention is needed -- simply follow the prompts when the update banner appears.

---

## Troubleshooting Quick Reference

### How do I report a crash?

If MeedyaDL crashes, a crash report is automatically saved to your local app data directory. You can report it directly to the developer from within the app:

1. Go to **Settings > Advanced > Crash Reporting**.
2. Find the crash report in the list and click **Report**.
3. Review the data that will be shared in the preview dialog.
4. Click **Open GitHub Issue** to open a pre-filled issue in your browser.
5. Add any steps to reproduce and submit on GitHub.

A GitHub account is required. No personal data is included in the report. For full details, see [Troubleshooting > Reporting a Crash](troubleshooting.md#reporting-a-crash-via-github-issues).

### My download keeps failing. What should I do?

Try these steps in order:

1. Verify your cookies are still valid (see [Cookie Management](cookie-management.md))
2. Check your internet connection
3. Try a different quality setting (see [Quality Settings](quality-settings.md))
4. Check the log files for specific error messages (see [Troubleshooting](troubleshooting.md))
5. If the issue persists, report it as a bug (see [Troubleshooting](troubleshooting.md#reporting-a-bug))

### Where can I get more help?

If your question is not answered here, check the full [Troubleshooting](troubleshooting.md) guide. You can also open an issue on the project's [GitHub Issues](https://github.com/MeedyaSuite/MeedyaDL/issues) page for support.

---

## Release channels and updates

### What are "release channels"?

MeedyaDL ships across six channels, ordered from least to most stable: **Nightly → Weekly → Monthly → Alpha → Beta → Stable**. Pre-release channels (anything below Stable) may be incomplete, untested, or broken. Pick your channel in **Settings > General > Updates**. See [Release Channels](release-channels.md) for the full breakdown.

### Will I accidentally get a Nightly build if I'm on Stable?

No. The in-app updater only surfaces releases matching your selected channel, and the installer refuses to apply a tag from a less-stable channel than the one you're on. Switching channel is always an explicit action in Settings.

### How do I move back to Stable after trying a pre-release build?

Open **Settings > General > Updates**, pick **Stable** from the Update Channel dropdown, and save. The next update check will surface the latest Stable release. If the Stable version number is lower than the pre-release version you're currently on, you'll need to download and install Stable manually from the [Releases page](https://github.com/MWBMPartners/MeedyaDL/releases) — the updater won't auto-downgrade your version.

### How often are pre-release builds published?

- **Nightly**: every day at 00:00 UTC (if there are new changes to integrate).
- **Weekly**: every Sunday at 00:00 UTC.
- **Monthly**: on the 1st of every month at 00:00 UTC.
- **Alpha / Beta**: published ad-hoc during release preparation.

---

## Related Topics

- [Getting Started](getting-started.md) -- First-time setup guide
- [Quality Settings](quality-settings.md) -- Detailed quality and format information
- [Cookie Management](cookie-management.md) -- Authentication and cookie setup
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Lyric formats and metadata fields
- [Troubleshooting](troubleshooting.md) -- Error resolution and diagnostics

### macOS shows "MeedyaDL can't be opened because Apple cannot check it for malicious software"

This is macOS Gatekeeper protecting you from unverified software. MeedyaDL's pre-release builds are not yet signed with an Apple Developer ID certificate. To open MeedyaDL:

1. **Right-click** (or Control-click) the MeedyaDL app icon
2. Click **Open** from the context menu
3. Click **Open** again in the dialog that appears

Alternatively, run this command in Terminal after installing:

```bash
xattr -cr /Applications/MeedyaDL.app
```

This only needs to be done once. Future launches will open normally. macOS code signing and notarization are planned for the v1 stable release.

### Windows shows "Windows protected your PC" (SmartScreen)

Windows SmartScreen shows this warning for software from unverified publishers. MeedyaDL's pre-release builds are not yet signed with a code signing certificate. To proceed:

1. Click **More info** on the SmartScreen dialog
2. Click **Run anyway**

This only needs to be done once per version. The warning will not reappear for the same version after you choose "Run anyway". Windows EV code signing is planned for the v1 stable release.

---

[Back to Help Index](index.md)
