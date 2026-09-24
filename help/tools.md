<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# External Tools

MeedyaDL relies on several external command-line tools for downloading, decrypting, and processing media. You can check their status and install or update them from **Settings > Tools** at any time.

## Required Tools

All five tools below are required for full functionality. On GAMDL **3.6 and newer**, GAMDL itself no longer needs MP4Box or mp4decrypt -- it mixes and decrypts music videos natively. FFmpeg, N_m3u8DL-RE, and MediaInfo remain necessary no matter which GAMDL version you're on: FFmpeg is used by MeedyaDL's own ReplayGain and BPM analysis (and by N_m3u8DL-RE itself when fetching HLS streams), N_m3u8DL-RE fetches the streams in the first place, and MediaInfo helps MeedyaDL detect the real codec of a finished file. MeedyaDL still asks the Setup Wizard to install all five, since it supports a wide range of GAMDL versions, including older ones that still need MP4Box and mp4decrypt.

### FFmpeg
Used for audio/video processing and container remuxing. Required for most download operations.

### mp4decrypt
Part of the Bento4 toolkit. Used for decrypting DRM-protected streams. Essential for downloading protected content.

### N_m3u8DL-RE
HLS/DASH stream downloader. Used for downloading segmented media streams from Apple Music's CDN.

### MP4Box
Part of the GPAC toolkit. Used for MP4 muxing and remuxing operations.

### MediaInfo
Used to accurately detect the codec of a downloaded file, so the app can tell what quality it actually got.

## Optional Tools

### rclone
Powers direct-to-cloud upload. Unlike the five required tools above, rclone is **not** installed during first-time setup -- MeedyaDL only downloads and installs it the moment you turn on a cloud destination in Settings. If you never use that feature, rclone never lands on your machine.

## Installation & Management

Tools are automatically downloaded during first-time setup. After setup, go to **Settings > Tools** to:

- **Check All** — refresh the status of all tools
- **Install missing tools** — individually or all at once
- **Override paths** — click the chevron on any tool to set a custom binary path (e.g., if you have a system-wide installation you prefer)

If new tools are added in a future update, the Tools tab will show them as missing so you can install them.

## Keeping the tools up to date

When MeedyaDL checks for updates, it checks all five required tools too, and lists any newer version on the **Updates** page. Each tool is compared against the place a new copy would actually come from, so you are never offered a "new" version from somewhere MeedyaDL would not install it from:

- **N_m3u8DL-RE** -- its own releases on GitHub.
- **FFmpeg** -- the same source your copy was installed from. FFmpeg builds are compared by the date they were built, because many of them have no ordinary version number.
- **mp4decrypt** and **MediaInfo** -- the MeedyaSuite mirror, a copy of the tools kept by the MeedyaDL project.
- **MP4Box** -- wherever your copy came from: GPAC's own releases, or the MeedyaSuite mirror.

**Copies you installed yourself are left alone.** If MeedyaDL is using a copy of a tool that a package manager put on your computer (for example Homebrew or apt), or one it simply found there, it does not check it or try to update it. Update it the same way you installed it.

**"Could not check" is shown, not hidden.** Sometimes MeedyaDL cannot tell whether a tool is up to date -- for example when there is no internet connection, when the tool will not start and so cannot report its version, when nobody recorded who installed that copy, or when the source does not publish a version MeedyaDL can compare. When that happens, the Updates page lists the tool under "could not check" with the reason, and the heading says **No updates found** rather than **You're up to date!**. Usually you do not need to do anything. If the reason says the tool will not start, reinstalling it is the usual fix: use **Install** in **Settings > Tools** if it shows as missing there, or press **Re-run Setup Wizard** in **Settings > Advanced > Setup**.

**An update never leaves you worse off.** Pressing **Upgrade** on the Updates page only installs from the source that was checked. If that source is not answering, your working copy is left as it is and MeedyaDL tells you, rather than quietly installing a copy from somewhere else that could be the same age or older. If a new copy of MP4Box turns out not to start, your previous copy is put back.
