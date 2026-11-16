<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Getting Started

This guide takes you from installing MeedyaDL to your first download. In short: install the app, let the setup wizard run, sign in to Apple Music, then paste a link.

---

## Prerequisites

### System Requirements

MeedyaDL is available on the following platforms:

- **macOS 13.3+** (Ventura or later) — Apple Silicon (.dmg)
- **Windows x64 / ARM64** — NSIS installer (.exe)
- **Linux x64** — Debian package (.deb), RPM package (.rpm), or AppImage (.AppImage)
- **Linux ARM64** — Debian or RPM package (.deb / .rpm) — Raspberry Pi 4/5, ARM servers
- **Linux ARMv7** — Debian or RPM package (.deb / .rpm) — 32-bit Raspberry Pi (experimental)

Additional requirements:

- **Disk Space:** Sufficient storage for downloaded media files. Lossless and Hi-Res Lossless tracks are significantly larger than standard AAC.
- **Internet Connection:** Required for downloading content from Apple Music and for the first-run setup wizard to install dependencies.

### Required Software

You do not need to install anything else first. The setup wizard downloads everything MeedyaDL needs (Python, GAMDL, FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box and MediaInfo) into the app's own folder. None of it touches the rest of your computer.

The only things you need before using MeedyaDL are:

- A valid Apple Music subscription for accessing content
- A way to give MeedyaDL your Apple Music sign-in details (called cookies). You can use the built-in login window, import them from your browser, or export them with a browser extension. See [Cookie Management](cookie-management.md).

---

## Installation

### macOS

1. Download the `.dmg` file for your Mac.
2. Open the `.dmg` file by double-clicking it.
3. Drag the **MeedyaDL** application into your **Applications** folder.
4. Eject the `.dmg` disk image.
5. Open MeedyaDL from your Applications folder.

Official MeedyaDL releases are signed and approved by Apple, so macOS should open them normally.

<details>
<summary>More detail: if macOS shows a warning anyway</summary>

If you see a "can't be opened" or "cannot check it for malicious software" message, it is almost certainly one of these:

- **You have an older download.** Disk images published before September 2026 were signed but not fully approved, so macOS warned about the disk image even though the app inside was fine. Download the current release.
- **You built it yourself.** Builds made outside our release process are unsigned, and macOS will warn about them.
- **The download was interrupted or altered.** Download it again.

If you would rather not update yet, right-click (or Control-click) the app and choose **Open**, then **Open** again in the dialog. That tells macOS you trust this one app, and you only do it once.

**Please do not run `xattr -cr`.** It strips information from the app that macOS relies on, not just the one thing behind this warning. Right-click and **Open** does the same job for one app and nothing more.

</details>

### Windows

1. Download the `.exe` installer for your architecture (x64 or ARM64).
2. Run the installer and follow the on-screen prompts.
3. Launch MeedyaDL from the Start Menu or desktop shortcut.

**SmartScreen warning:** Windows builds are not code-signed yet, so Windows SmartScreen shows a warning each time you install a new version. To continue:

- Click **"More info"** in the SmartScreen dialog.
- Click **"Run anyway"** to continue with the installation.

### Linux

**Debian / Ubuntu (.deb):**

1. Download the `.deb` package for your architecture (x64).
2. Install it using your package manager or by running `sudo dpkg -i meedyadl_*.deb` in a terminal.
3. Launch MeedyaDL from your application menu or by running `meedyadl` in a terminal.

**AppImage:**

1. Download the `.AppImage` file.
2. Make it executable: `chmod +x MeedyaDL_*.AppImage`
3. Run it: `./MeedyaDL_*.AppImage`

**Raspberry Pi (ARM64):**

Follow the same Debian / Ubuntu instructions above using the ARM64 `.deb` package. Ensure you are running a 64-bit Raspberry Pi OS.

---

## Initial Configuration

### First-Run Setup Wizard

The first time you launch MeedyaDL, a setup wizard will guide you through the initial configuration. The wizard has **6 steps**:

1. **Welcome** — Introduction and overview of the setup process.
2. **Python** — MeedyaDL needs Python 3.10 or newer. If you already have a compatible Python installed (for example via Homebrew or python.org), the wizard detects it and offers a **"Use this"** button — MeedyaDL then sets up its own separate copy from it (your system Python is never changed), so there is nothing to download. Otherwise, it downloads and installs a sandboxed Python runtime into its data directory. A **Browse…** button lets you point at a specific interpreter.
3. **Engine** — GAMDL, the free download tool MeedyaDL uses for Apple Music, is installed automatically.
4. **Dependency Installation** — Additional tools are installed: FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box, and MediaInfo. They go into MeedyaDL's own folder and do not affect your system. If you already have one of them installed, you can point MeedyaDL at it with **Browse**.
5. **Cookie Import** — You will be prompted to import your Apple Music cookies (see below).
6. **Complete** — Setup is finished and MeedyaDL is ready to use.

The wizard handles everything automatically. Simply follow the prompts and wait for each step to complete.

### Setting Up Cookies

Before you can download, MeedyaDL needs your Apple Music cookies. They let it sign in as you. There are three ways to provide them:

1. **Built-in Apple Music login** (easiest) — Sign in to Apple Music directly within the app. MeedyaDL opens a login window, you sign in with your Apple ID, and the cookies are picked up automatically. No browser extensions needed.

2. **Browser auto-import** — MeedyaDL can find your installed browsers (Chrome, Firefox, Edge, Safari and others) and take your Apple Music cookies from them. You must already be signed in to music.apple.com in that browser.

3. **Manual cookie file import** — Export your cookies from your browser in **Netscape cookie format** using a browser extension such as "Get cookies.txt LOCALLY" or "cookies.txt", then import the file into MeedyaDL.

For step-by-step instructions on all three, and help with cookie problems, see [Cookie Management](cookie-management.md). Most people never need the optional [wrapper](wrapper.md).

### Choosing an Output Directory

By default, MeedyaDL saves downloaded files to an **"Apple Music" folder inside** your system's music directory (e.g., `~/Music/Apple Music` on macOS and Linux, or `Music\Apple Music` on Windows) — not directly into the music folder itself.

To change the output directory:

1. Open **Settings** from the sidebar.
2. Go to the **General** tab.
3. Under **Output Directory**, pick a folder or type a path.

### Configuring Default Quality

The default audio codec is **ALAC** (Apple Lossless), which provides the highest quality lossless audio. If you prefer smaller file sizes, you can switch to AAC.

To change the default quality:

1. Open **Settings** from the sidebar.
2. Go to the **Codec & Resolution** tab.
3. Select your preferred audio codec and quality level.

For a full explanation of available quality options, including lossless and Hi-Res Lossless formats, see [Quality Settings](quality-settings.md).

---

## Your First Download

Once setup is complete and your cookies are configured, downloading music is straightforward:

1. **Open MeedyaDL** if it is not already running.
2. **Paste an Apple Music URL** into the URL input field. The application auto-detects the content type — whether it is a song, album, playlist, music video, or artist page.
3. **Select quality overrides** if you want to download at a quality different from your default settings. This is optional; your configured defaults will be used otherwise.
4. **Click "Add to Queue"** to begin the download.
5. **Monitor progress** in the download queue, which shows the status of each item.

You can paste multiple URLs and add them to the queue one after another. The queue processes downloads sequentially so you can continue adding items while downloads are in progress.

**Tip:** MeedyaDL also monitors your clipboard. If you copy an Apple Music URL from a browser or messaging app, a message will appear in MeedyaDL offering to download it — clicking **Download** adds it directly to the queue. While MeedyaDL is not in front, your computer can also show a system notification so you don't miss it. Clipboard monitoring can be toggled off in **Settings > General > Preferences**.

**If a download fails:** every failed item shows a **Retry** button on the right of its row in both the Queue and History pages. Right-click any row to access the same actions plus Copy URL and Open Folder. To re-run multiple failed items at once, use **Retry All Failed (N)** in the History page header, or **Retry all failed (N)** in the Queue page's **More** menu. MeedyaDL is smart about retries — it reads the album's manifest and only re-fetches the tracks that actually failed, rather than re-running the whole album.

For more details on downloading music, see [Downloading Music](downloading-music.md). For music videos, see [Downloading Videos](downloading-videos.md).

---

## Next Steps

Once you have MeedyaDL set up and running, explore these topics to make the most of the application:

- [Downloading Music](downloading-music.md) — Learn about downloading songs, albums, and playlists
- [Quality Settings](quality-settings.md) — Fine-tune your audio and video quality preferences
- [Lyrics and Metadata](lyrics-and-metadata.md) — Configure lyric downloads and metadata embedding
- [Cookie Management](cookie-management.md) — Manage, refresh, and troubleshoot your Apple Music cookies
- [Troubleshooting](troubleshooting.md) — Solutions for common issues

---

[Back to Help Index](index.md)
