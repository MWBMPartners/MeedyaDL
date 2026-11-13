<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# External Tools

MeedyaDL relies on several external command-line tools for downloading, decrypting, and processing media. You can check their status and install or update them from **Settings > Tools** at any time.

## Required Tools

All five tools below are required for full functionality. On GAMDL **3.6 and newer**, GAMDL itself no longer needs MP4Box or mp4decrypt — it mixes and decrypts music videos natively. FFmpeg, N_m3u8DL-RE, and MediaInfo remain necessary no matter which GAMDL version you're on: FFmpeg is used by MeedyaDL's own ReplayGain loudness analysis (and by N_m3u8DL-RE itself when fetching HLS streams), N_m3u8DL-RE fetches the streams in the first place, and MediaInfo helps MeedyaDL detect the real codec of a finished file. MeedyaDL still asks the Setup Wizard to install all five, since it supports a wide range of GAMDL versions, including older ones that still need MP4Box and mp4decrypt.

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

## Trying GAMDL 4

GAMDL 4.0.0, a rewrite of GAMDL, came out on 12 November 2026. MeedyaDL can use it, but it is **not yet tested on real downloads**, so it is not installed unless you ask for it. MeedyaDL keeps recommending GAMDL 3.9.1.

**How to try it.** Go to **Settings > Tools > GAMDL Version Management**, type `4.0.0` under **Advanced: install specific version**, and press **Install**. To go back, press **Install recommended** in the same place. While you are on GAMDL 3, the **Updates** page keeps offering GAMDL 3 updates only, so nobody is moved to GAMDL 4 by accident. Once you are on GAMDL 4, it offers GAMDL 4 updates, and running the setup wizard again keeps the GAMDL 4 version you chose rather than moving you back.

**Not every computer can use it.** On Linux on 32-bit ARM (for example an older Raspberry Pi), GAMDL 4 has no installable package. On Windows on ARM, MeedyaDL stays on GAMDL 3.8.5 for now. On both, MeedyaDL will not install GAMDL 4 and says why.

**Your settings carry over.** Nothing needs converting: MeedyaDL turns your settings into instructions for GAMDL each time a download starts, in whichever form the installed GAMDL understands. A few settings work differently, or not at all, with GAMDL 4, and **Settings** says so beside each one:

- **Cover art.** GAMDL 4 would save a picture beside every song, so MeedyaDL saves one album picture itself instead, when it can get the album's details from Apple Music; the **Save Cover Art** switch has no effect. The **Cover Size** setting is not used, and the "original file" format choice gives a JPEG picture inside each song.
- **Synced Lyrics Only.** GAMDL 4 cannot download lyrics on their own, so downloads will not start while this is on. Turn it off to download as usual.
- **Download Mode** and **Remux Mode** (Settings > Advanced) are not used; GAMDL 4 fetches and puts files together itself.
- **PlayReady** is not used with GAMDL 4 yet. Downloads use the built-in Widevine unlocking instead, and the Activity Log says so.
- **mp4decrypt**, **MP4Box** and **N_m3u8DL-RE** are not used for Apple Music downloads with GAMDL 4. Keep them installed if you might go back to GAMDL 3. MeedyaDL itself still uses FFmpeg and MediaInfo.

**Your download history is safe.** GAMDL 4 can keep its own list of what it has downloaded and skip anything on it. MeedyaDL switches that list off, so re-downloading an album, or filling in a missing format, still works.

If something goes wrong with GAMDL 4, please [report it](troubleshooting.md#reporting-a-bug) and go back to 3.9.1 meanwhile.

## Installation & Management

Tools are automatically downloaded during first-time setup. After setup, go to **Settings > Tools** to:

- **Check All** — refresh the status of all tools
- **Install missing tools** — individually or all at once
- **Reinstall** — every installed tool has a **Reinstall** button. It downloads the tool again, or uses a suitable copy already on your computer. If your copy came from a package manager (for example Homebrew or APT), it may ask that package manager to update it instead, which can ask for your password. It asks you first. If your current copy starts and reports a version MeedyaDL recognises, and the new copy does not, your current copy is kept.
- **Use your own copy** — open any tool with the arrow on its right and choose a copy you installed yourself (for example, one installed for your whole computer that you prefer)

If new tools are added in a future update, the Tools tab will show them as missing so you can install them.

## Keeping the tools up to date

When MeedyaDL checks for updates, it checks all five required tools too, and lists the updates it finds on the **Updates** page. Each tool is compared against the place a new copy would actually come from, so you are never offered a "new" version from somewhere MeedyaDL would not install it from:

- **N_m3u8DL-RE** — its own releases on GitHub.
- **FFmpeg** — the same source your copy was installed from. FFmpeg builds are compared by the date they were built, because many of them have no ordinary version number, and an update is only offered once the newer build is more than 30 days newer than yours (new builds appear very often, so smaller gaps are not worth a download).
- **mp4decrypt** and **MediaInfo** — the MeedyaSuite mirror, a copy of the tools kept by the MeedyaDL project.
- **MP4Box** — a copy from the MeedyaSuite mirror is compared with the mirror. A copy from GPAC's own installer is compared with the GPAC version MeedyaDL is set up to install; at the moment none is set, so it shows as "could not check".

**Copies you installed yourself are left alone.** If MeedyaDL is using a copy of a tool that a package manager put on your computer (for example Homebrew or apt), or one it simply found there, it does not check it or try to update it. Update it the same way you installed it.

**"Could not check" is shown, not hidden.** Sometimes MeedyaDL cannot tell whether a tool is up to date — for example when there is no internet connection, when the tool will not start and so cannot report its version, when nobody recorded who installed that copy, or when the source does not publish a version MeedyaDL can compare. When that happens, the Updates page lists the tool under "could not check" with the reason, and the heading says **No updates found** rather than **You're up to date!**. Usually you do not need to do anything. If the reason says MeedyaDL **could not run** the tool, it will also show as missing in **Settings > Tools**, with an **Install** button. If the reason says the tool **started but reported something other than a version** (usually an error), it still shows as installed; press its **Reinstall** button in **Settings > Tools** to replace it.

**What an update does and does not protect.** Pressing **Upgrade** on the Updates page never swaps in a copy it happens to find elsewhere on your computer, and only falls back to the MeedyaSuite mirror when the mirror is what the check compared against. (FFmpeg may still come from its usual download site rather than the mirror; that copy is the same age or newer.) If the new copy cannot be downloaded or unpacked, your working copy is left as it is and MeedyaDL tells you. And if your current copy works, MeedyaDL keeps it until the new copy is fully in place and has started and reported its version. Here "works" means the copy starts and, within 30 seconds, reports a version MeedyaDL recognises. For FFmpeg it also covers ffprobe, the part of FFmpeg that MeedyaDL uses to look inside the files it downloads: if your current copy's ffprobe works, the new copy's must too. If the new copy does not work on your computer — a missing part, or a build for a different kind of processor — MeedyaDL removes it, puts your previous copy back, and tells you. This is true for every tool, and for **Install** and **Reinstall** in **Settings > Tools** as well as for updates. If your current copy does not work either — including one that starts but prints something MeedyaDL does not recognise as a version — MeedyaDL has nothing it can safely go back to, so the new copy is used. What MeedyaDL cannot undo is a change made outside its own folders: reinstalling a tool that came from a package manager (for example Homebrew) can ask that package manager to update it, and MeedyaDL cannot put the package manager's previous version back. If that copy no longer starts afterwards, MeedyaDL says so rather than reporting success.
