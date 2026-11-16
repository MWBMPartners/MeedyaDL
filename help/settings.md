<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Settings

MeedyaDL's Settings screen has 10 tabs, grouped into four sections in the sidebar: General, Download (Codec & Resolution, Codec Fallback Order, Lyrics, Cover Art, Metadata, Templates), Authentication (Cookies), and System (Tools, Advanced). This page covers what each one controls and when you might want to change it. The defaults suit most people, so you can leave most of it alone.

**Saving.** Changes are not kept until you press **Save Changes** at the top of the Settings screen. The button reads **Saved** when nothing is waiting. If saving fails, MeedyaDL says so and your changes stay on screen, still unsaved.

**Reset** puts the settings on these tabs back to how they were when MeedyaDL was first installed. It asks first, and lists what you would lose that is hard to set up again: where your cookies file and helper tools are, your Apple Music credentials and API keys, your wrapper addresses, and your download folder. A reset is not saved until you press **Save Changes**. Until then, closing Settings and opening it again gets your old settings back.

<details>
<summary>More detail: choices that save straight away</summary>

A few choices are saved the moment you make them, without pressing Save Changes, and do not count as unsaved changes:

- the one-off after-queue action from the Download page (Reset does not cancel it)
- your answer to the crash-reporting question
- **Don't ask again** in the Abort Queue window
- in the setup wizard, choosing where a tool is and finishing setup

</details>

## General

- **Output** — Where downloaded files are saved (default: an "Apple Music" folder inside your system's music directory).
- **Appearance** — Theme, High Contrast, Colour Vision, and **Language** — the language MeedyaDL's screens are shown in. A new choice takes effect straight away, with no restart. **Auto (System)** follows your computer's own language, and falls back to English if MeedyaDL has no complete translation for it. Only translations that cover the whole app are offered. German and French are not finished yet — most screens are still in English — so they are not offered; if you chose one of them before, it stays chosen and is marked in the list as an incomplete translation. (Both were translated by a machine and have not yet been checked by a native speaker, and the app says so while you are using one of them.)
- **Preferences** — Metadata language, retrying a failed link with your own region, overwriting existing files, **Auto-Start Downloads**, **Confirm before aborting the queue** (turn this back on here if you ticked **Don't ask again** in the Abort Queue window and want the question back), **After Queue Completes**, **Smart Re-Download Detection**, **Clipboard Monitoring**, and the update settings (**Auto-Check for Updates**, how often to check, and **Update Channel** — see [Release Channels](release-channels.md)). **After Queue Completes** is what happens every time the queue finishes: do nothing, open the output folder, play a sound, close MeedyaDL, or restart, sleep/hibernate or shut down the computer. For a one-time choice instead, see [After the queue finishes](downloading-music.md#after-the-queue-finishes).
- **Notifications** — Messages always appear inside MeedyaDL. These settings decide when your computer also shows them as system notifications (the ones outside the app). Changes apply once you press Save Changes. The main switch is **System Notifications**: off means no system notifications at all.

<details>
<summary>More detail: all the notification choices</summary>

  - **Show System Notifications** — when: **When MeedyaDL is not in front, or the pointer is on another screen** (the default: the window is behind another app, minimised, hidden in the tray, or your mouse pointer is on a different screen from MeedyaDL's window), **Only when the pointer is on another screen** (with one screen this never shows any), or **Always**.
  - **Which Messages** — **Important messages only** (the default: a download finished or failed, the queue finished, a copied link was found, your computer is about to shut down or restart, an app update is ready), **Important messages and errors**, or **Every message**.
  - **Notification Style** — **In-app only**, **System and in-app** (the default), or **System only**. With **System only**, a plain message that your system has shown is not repeated inside the app. Errors, warnings, messages with a button, and anything the system was not asked to show still appear inside the app, so nothing is lost.
  - **A Download Finished**, **A Download Failed**, **The Queue Finished**, **A Copied Link Was Found** — switch the system notification for each of these on or off. The message inside the app still appears.
  - **Send Test Notification** — sends one system notification straight away, using the choices on screen even before you save them. If nothing appears, see [Notifications don't appear](troubleshooting.md#notifications-dont-appear).
  - **Message Auto-Dismiss** — how long plain messages inside the app stay before going away. Errors and warnings stay until you close them. A message stays open while your pointer rests on it or it has keyboard focus.

</details>

- **Settings File** — **Export Settings** and **Import Settings**: save just your settings to a file, or load them from one, to keep a copy or to set up another computer the same way. Sensitive details such as cookies and credentials are left out of the export. For your safety, an imported file cannot choose which programs MeedyaDL runs or loads, or set anything that would shut your computer down — those stay as they are on this computer.
- **Full Copy of MeedyaDL** — **Export Full Copy** and **Import Full Copy**: everything this copy of MeedyaDL holds, in a single `.meedyabundle` file, for moving to a new computer or keeping before a big change. When you open MeedyaDL for the first time, it also offers to restore any full copy it finds in your Downloads, Desktop or home folder. You choose which extra sections to include (queue, history, activity logs and so on). Credentials can be included too, protected by a password you choose. When you restore one, your settings are always restored and you tick which other sections to replace. Restoring does not change where your downloads are saved on this computer.

## Codec & Resolution

- **Audio Quality** — Default audio codec (ALAC lossless by default), Companion Downloads mode (automatically download extra format versions alongside your primary download, e.g. Atmos + a lossless companion), and duplicate-detection preferences for artist downloads.
- **Video Quality** — Maximum Video Resolution (2160p/4K by default, treated as a ceiling — Apple Music returns the closest quality at or below it), the container format videos are repackaged into, music video companion downloads (these work with or without your own Apple Music developer credentials), and the MusicBrainz lookup, which you can also turn on on its own.
- **Enable Fallback Chain** — One switch that covers both songs and music videos. When it's on, an unavailable codec steps down through your configured chain (see below); when it's off, only your preferred codec is tried, and nothing steps down further.

## Codec Fallback Order

Reorder the audio codec and video codec fallback chains using the up/down arrow buttons on each row, or remove an option entirely so it's never used as a fallback (useful if you never want, say, AAC Binaural mixed into your library, or H.264 music videos). The chain persists across restarts and applies automatically whenever your preferred codec isn't available for a track or video. There is no resolution fallback chain here — video resolution is a ceiling, not something with a fallback order; see [Fallback Quality](fallback-quality.md) for why.

## Tools

- **Core Dependencies** — Status, install, and update controls for Python and GAMDL, including GAMDL version management (which validated version range MeedyaDL supports, and whether to upgrade).
- **External Tools** — The five required tools (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, MediaInfo), each with status, install, and a **Your Own Copy** option (open it with the arrow on the tool's right) if you'd rather point MeedyaDL at a copy you installed yourself.
- **Directories** — The temporary working directory GAMDL uses during a download (default: your OS temp folder plus `MeedyaDL`).
- **Automatic Snapshots** — Snapshots of your settings, queue and download history, kept on this computer. MeedyaDL takes one automatically each time it closes, and you can press **Create snapshot now** at any time; only the 10 most recent are kept. Restoring a snapshot replaces your current settings, queue and history, and deleting one cannot be undone — both ask you to confirm first.

## Cookies

**Authentication Cookies** — Import your Apple Music cookies (via the built-in login window, automatic browser detection, or manual file import), check expiry, and re-authenticate when they run out. This is how MeedyaDL proves to Apple Music that it's you, without a wrapper.

## Lyrics

**Synced Lyrics** — Which lyric formats to generate (Enhanced LRC, Rich SRT, WebVTT, ASS, and an experimental `.lyrics` YAML sidecar format used by tools such as LRCGET), whether to embed them in the audio file as well as saving a sidecar file, and a **Test word-level lyrics connection** button next to the Enhanced Lyrics toggle. That button checks whether word-by-word lyrics are available to you right now, without waiting for a full download. Green means they are and Enhanced LRC will work. Amber means only line-by-line lyrics came back. Anything else comes with a hint about what to fix: signing in to Apple Music, adding your credentials in Settings > Advanced, or importing your cookies again.

## Cover Art

- **Cover Art** — Whether to save cover art at all, its format and size, filename (Front Cover / Cover / Folder), and **Upgrade Cover Art From Other Services** — an opt-in switch that, once a download already has a saved cover image, checks whether another service (currently Deezer, matched by the release's barcode) has a bigger picture and replaces the saved file if so. It only ever swaps the saved image file for a bigger one; it never touches the artwork already embedded in a track. Off by default, because it means contacting a third-party service for every album.
- **Animated Artwork** — Downloading Apple Music's animated cover art and artist promo videos, and whether to hide those files from normal file browsing so they do not clutter a music folder.

## Metadata

- **Automatic Tags** — Codec, source, and channel-configuration tags, plus extra details from Apple Music such as record label and release information.
- **AcoustID Fingerprinting** — Opt-in audio fingerprinting and lookup against the AcoustID database.
- **ReplayGain Analysis** — Opt-in loudness analysis so your library plays back at a consistent volume.
- **Links on Other Music Services** — A switch called **Look up where else each album is available**, in the **Links on Other Music Services** section. It is off by default. When it's on, MeedyaDL asks song.link (a lookup service run by a company called Odesli) where else each downloaded album is available — Spotify, YouTube Music, Tidal, and others — and saves what it finds into your files. It needs an access key from Odesli, entered under Settings > Advanced > API Credentials.

## Templates

- **Folder Templates** — How output folders are named, using variables like `{artist}`, `{album}`, `{platform}`.
- **File Templates** — How individual files are named, using variables like `{title}`, `{track:02d}`.

## Advanced

- **Processing** — Which helper tools MeedyaDL uses for the download and for repackaging the finished file, and how long MeedyaDL waits with no progress before giving up on a download. All three apply to Apple Music downloads.
- **File Options** — Shorten long filenames to a maximum length, and list any metadata tags you want left out of downloaded files.
- **Setup** — A single **Re-run Setup Wizard** button that re-runs the first-time setup wizard, so your Python, GAMDL and tool installs are re-checked and repaired. Your saved settings are kept. Starting it reloads MeedyaDL, so if you have changes on the Settings screen that you have not saved yet, it warns you first — press Cancel and then Save Changes if you want to keep them.
- **Wrapper / Sign in to wrapper** — An optional extra program that lets you sign in without cookies. Most people do not need it. See [Wrapper Authentication](wrapper.md) for when it helps, how to set it up, and how to check it works.
- **Unlocking Method** — A rarely needed option. Most people should leave it on the default. It only appears when your installed GAMDL version offers it. If you chose a non-default method and later went back to an older GAMDL, the option stays visible with a note saying that the default is being used until you upgrade.
- **API Credentials** — Your own Apple Music developer credentials (Team ID, Key ID, private key) for extra features, an AcoustID key override, and the song.link (Odesli) access key used by the Metadata tab's links feature.
- **Error Reporting** — Local crash reports are always kept on your own machine. The **Send Anonymous Crash Reports** switch turns on optional, anonymous crash reporting to help us fix bugs — off unless you say yes, no personal data, download history, or library information ever included, and a change takes effect after you restart MeedyaDL — alongside one-click reporting of a specific error to GitHub Issues (which opens a pre-filled issue in your browser after showing you exactly what would be sent).
- **Diagnostics** — Verbose activity logging, the on-disk activity log's location and export, and other troubleshooting aids.
- **Developer Tools** — Only visible to people who have turned on developer access. It shows diagnostic information that normal use does not need.

---

## Related Topics

- [Getting Started](getting-started.md) — First-time setup guide
- [Quality Settings](quality-settings.md) — Codec and resolution details
- [Fallback Quality](fallback-quality.md) — How the fallback chain works
- [Cookie Management](cookie-management.md) — Cookie import and troubleshooting
- [Wrapper Authentication](wrapper.md) — Wrapper setup and troubleshooting
- [Tools](tools.md) — The external programs MeedyaDL manages

[Back to Help Index](index.md)
