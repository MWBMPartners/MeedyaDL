<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Settings

MeedyaDL's Settings screen has 11 tabs, grouped into five sections in the sidebar: General, Download (Codec & Resolution, Codec Fallback Order, Lyrics, Cover Art, Metadata, Templates), Authentication (Cookies), Services (Spotify), and System (Tools, Advanced). This page covers what each one actually controls.

**Saving and resetting.** Changes you make on these tabs are not kept until you press **Save Changes** at the top of the Settings screen (the button reads **Saved** when there is nothing waiting). If saving fails, MeedyaDL says so and your changes stay on screen, still unsaved. **Reset** puts the settings on these tabs back to how they were when MeedyaDL was first installed (a one-off after-queue action you chose on the Download page is not cancelled by it). It asks first, and lists what you would lose that is hard to set up again -- where your cookies file and helper tools are, your Apple Music credentials and API keys, your wrapper addresses, and your download folder. A reset is not written to disk until you press **Save Changes**; until then, closing Settings and opening it again gets your old settings back.

A few choices are saved the moment you make them, without pressing Save Changes, and do not count as unsaved changes: the one-off after-queue action from the Download page, your answer to the crash-reporting question, **Don't ask again** in the Abort Queue window, and, in the setup wizard, choosing where a tool is and finishing setup.

## General

- **Output** -- Where downloaded files are saved (default: an "Apple Music" folder inside your system's music directory).
- **Appearance** -- Theme, High Contrast, Colour Vision, and **Language** -- the language MeedyaDL's screens are shown in. A new choice takes effect straight away, with no restart. **Auto (System)** follows your computer's own language, and falls back to English if MeedyaDL has no translation for it. German and French were translated by a machine and have not yet been checked by a native speaker, so the app says so while you are using one of them.
- **Preferences** -- Metadata language, retrying a failed link with your own region, overwriting existing files, **Auto-Start Downloads**, **After Queue Completes**, notifications (**Desktop Notifications** on/off, the three-way **Notification Style**, and how long auto-dismissing messages stay on screen), **Smart Re-Download Detection**, **Clipboard Monitoring**, and the update settings (**Auto-Check for Updates**, how often to check, and **Update Channel** -- see [Release Channels](release-channels.md)). **After Queue Completes** is what happens every time the queue finishes: do nothing, open the output folder, play a notification sound, close MeedyaDL, or restart, sleep/hibernate or shut down the computer. For a one-time choice instead, see [After the queue finishes](downloading-music.md#after-the-queue-finishes).
- **Backup** -- **Export Settings** and **Import Settings**: save your settings to a file, or load them from one, for a backup or to move them to another computer. Sensitive details such as cookies and credentials are left out of the export. For your safety, an imported file cannot choose which programs MeedyaDL runs or loads, or set anything that would shut your computer down -- those stay as they are on this computer.
- **Profile Bundle** -- **Export Profile** and **Import Profile**: a complete, portable copy of this install in a single `.meedyabundle` file, for moving to a new computer or keeping a backup before a big change. You choose which extra sections to include (queue, history, activity logs and so on); credentials can be included too, protected by a password you choose. When you restore one, your settings are always restored and you tick which other sections to replace. Restoring does not change where your downloads are saved on this computer, and your Apple Music identifiers are only changed when the key they belong to is restored with them.

## Codec & Resolution

- **Audio Quality** -- Default audio codec (ALAC lossless by default), Companion Downloads mode (automatically download extra format versions alongside your primary download, e.g. Atmos + a lossless companion), and duplicate-detection preferences for artist downloads.
- **Video Quality** -- Maximum Video Resolution (2160p/4K by default, treated as a ceiling -- Apple Music returns the closest quality at or below it), the container format videos are repackaged into, music video companion downloads (these work either way: with MusicKit credentials MeedyaDL asks the Apple Music API, without them it falls back to MusicBrainz), and the MusicBrainz lookup itself, which you can also turn on on its own.
- **Enable Fallback Chain** -- One switch that covers both songs and music videos. When it's on, an unavailable codec steps down through your configured chain (see below); when it's off, only your preferred codec is tried, and nothing steps down further.

## Codec Fallback Order

Reorder the audio codec and video codec fallback chains using the up/down arrow buttons on each row, or remove an option entirely so it's never used as a fallback (useful if you never want, say, AAC Binaural mixed into your library, or H.264 music videos). The chain persists across restarts and applies automatically whenever your preferred codec isn't available for a track or video. There is no resolution fallback chain here -- video resolution is a ceiling, not something with a fallback order; see [Fallback Quality](fallback-quality.md) for why.

## Tools

- **Core Dependencies** -- Status, install, and update controls for Python and GAMDL, including GAMDL version management (which validated version range MeedyaDL supports, and whether to upgrade).
- **External Tools** -- The five required tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, MediaInfo) plus the optional rclone tool (only needed if you turn on direct-to-cloud upload), each with status, install, and a **Configure custom binary path** option if you'd rather point MeedyaDL at your own copy.
- **Directories** -- The temporary working directory GAMDL uses during a download (default: your OS temp folder plus `MeedyaDL`).
- **Backups** -- Snapshots of your settings, queue and download history. MeedyaDL takes one automatically each time it closes, and you can press **Create snapshot now** at any time; only the 10 most recent are kept. Restoring a snapshot replaces your current settings, queue and history, and deleting one cannot be undone -- both ask you to confirm first.

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

- **Processing** -- Which tool fetches the encrypted streams from Apple's servers, which tool repackages the finished file, and how long MeedyaDL waits with no output from GAMDL before giving up on a download. All three apply to Apple Music downloads.
- **File Options** -- Shorten long filenames to a maximum length, and list any metadata tags you want left out of downloaded files.
- **Setup** -- A single **Re-run Setup Wizard** button that re-runs the first-time setup wizard, so your Python, GAMDL and tool installs are re-checked and repaired. Your saved settings are kept. Starting it reloads MeedyaDL, so if you have changes on the Settings screen that you have not saved yet, it warns you first -- press Cancel and then Save Changes if you want to keep them.
- **Wrapper / Sign in to wrapper** -- Optional alternative authentication for Apple Music that doesn't rely on cookies. MeedyaDL supports two wrapper generations depending on your GAMDL version -- see [Wrapper Authentication](wrapper.md) for the full picture, including the three separate addresses wrapper-v1 uses and the single address wrapper-v2 uses, plus auto-retry-without-wrapper and connectivity testing.
- **Unlocking Method** -- How the download engine unlocks Apple Music's copy-protected tracks before it can save them. This is a technical term you don't normally need to think about -- Apple Music calls it DRM (Digital Rights Management), and unlocking it is just what makes a download possible at all. The built-in method (Widevine) needs no setup and is what MeedyaDL has always used. GAMDL 3.9 added a second method (PlayReady), but it only works if you supply your own PlayReady device file (a `.prd` file) -- MeedyaDL doesn't provide one and can't get one for you. Most people should leave this on the built-in method. This option only appears once you have GAMDL 3.9 or newer installed -- with one exception: if you'd already chosen PlayReady before going back to an older GAMDL, the option stays visible with a note explaining that downloads are quietly using the built-in method again until you upgrade. One reason PlayReady is worth knowing about even if you never touch it: it's one of the only two ways to download 4K music videos -- see [Downloading Videos](downloading-videos.md).
- **API Credentials** -- Your own MusicKit developer credentials (Team ID, Key ID, private key) for premium features, an AcoustID key override, and the song.link (Odesli) access key used by the Metadata tab's cross-platform links feature.
- **Error Reporting** -- Local crash reports are always kept on your own machine. The **Send Anonymous Crash Reports** switch turns on optional, anonymous crash reporting to help us fix bugs -- off unless you say yes, no personal data, download history, or library information ever included, and a change takes effect after you restart MeedyaDL -- alongside one-click reporting of a specific error to GitHub Issues (which opens a pre-filled issue in your browser after showing you exactly what would be sent).
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
