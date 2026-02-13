<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :rocket: Getting Started

This guide walks you through the first-time setup of MeedyaDL, from system requirements to your first download.

---

## Prerequisites

### System Requirements

MeedyaDL is available on the following platforms:

- **macOS 11.0+** (Big Sur or later) -- Apple Silicon (.dmg)
- **Windows x64 / ARM64** -- Installer (.msi) or portable executable (.exe)
- **Linux x64** -- Debian package (.deb) or AppImage (.AppImage)
- **Raspberry Pi** -- ARM64 (64-bit Raspberry Pi OS)

Additional requirements:

- **Disk Space:** Sufficient storage for downloaded media files. Lossless and Hi-Res Lossless tracks are significantly larger than standard AAC.
- **Internet Connection:** Required for downloading content from Apple Music and for the first-run setup wizard to install dependencies.

### Required Software

MeedyaDL has **no special software prerequisites**. All dependencies -- including Python, GAMDL, FFmpeg, mp4decrypt, N_m3u8DL-RE, and MP4Box -- are automatically downloaded and installed into the application's sandboxed data directory on first launch. You do not need to install any of these tools yourself.

The only things you need before using MeedyaDL are:

- A valid Apple Music subscription for accessing content
- A supported web browser (Safari, Chrome, Firefox, Edge, etc.) for exporting your Apple Music cookies (see [Cookie Management](cookie-management.md))

---

## Installation

### macOS

1. Download the `.dmg` file for your Mac.
2. Open the `.dmg` file by double-clicking it.
3. Drag the **MeedyaDL** application into your **Applications** folder.
4. Eject the `.dmg` disk image.
5. Open MeedyaDL from your Applications folder.

**Gatekeeper warning:** Because MeedyaDL is not signed with an Apple Developer certificate, macOS may block the application the first time you open it. To bypass this:

- **Right-click** (or Control-click) the MeedyaDL application and select **Open** from the context menu, then click **Open** in the confirmation dialog.
- Alternatively, go to **System Settings > Privacy & Security**, scroll down, and click **Allow** next to the message about MeedyaDL being blocked.

You only need to do this once. Subsequent launches will work normally.

### Windows

1. Download the `.msi` installer (recommended) or the standalone `.exe` for your architecture (x64 or ARM64).
2. Run the installer and follow the on-screen prompts.
3. Launch MeedyaDL from the Start Menu or desktop shortcut.

**SmartScreen warning:** Windows may display a SmartScreen warning because the application is not signed with an Extended Validation certificate. To proceed:

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

The first time you launch MeedyaDL, a setup wizard will guide you through the initial configuration. The wizard consists of **6 steps**:

1. **Welcome** -- Introduction and overview of the setup process.
2. **Python Installation** -- The application downloads and installs a sandboxed Python environment into its data directory.
3. **GAMDL Installation** -- The GAMDL command-line tool is downloaded and configured automatically.
4. **Dependency Installation** -- Additional tools are installed: FFmpeg, mp4decrypt, N_m3u8DL-RE, and MP4Box. All dependencies are placed in the application's sandboxed data directory and do not affect your system.
5. **Cookie Import** -- You will be prompted to import your Apple Music cookies (see below).
6. **Complete** -- Setup is finished and MeedyaDL is ready to use.

The wizard handles everything automatically. Simply follow the prompts and wait for each step to complete.

### Setting Up Cookies

Before you can download any content, you must provide your Apple Music cookies. These cookies authenticate MeedyaDL with Apple Music on your behalf.

You need to export your Apple Music cookies from your web browser in **Netscape cookie format**. This is a standard format supported by browser extensions such as "Get cookies.txt LOCALLY" or "cookies.txt".

To export your cookies:

1. Log in to [Apple Music](https://music.apple.com) in your web browser.
2. Use a cookie export extension to save your cookies in Netscape format.
3. Import the cookie file when prompted by the setup wizard, or at any time via the application's cookie management interface.

For detailed instructions, supported browsers, and troubleshooting cookie issues, see [Cookie Management](cookie-management.md).

### Choosing an Output Directory

By default, MeedyaDL saves downloaded files to your system's default music directory (e.g., `~/Music` on macOS and Linux, or the Music folder on Windows).

To change the output directory:

1. Open **Settings** from the application menu or toolbar.
2. Navigate to the **Paths** tab.
3. Set your preferred output directory using the folder picker or by entering a path directly.

### Configuring Default Quality

The default audio codec is **AAC**, which provides good quality at reasonable file sizes and is widely compatible.

To change the default quality:

1. Open **Settings** from the application menu or toolbar.
2. Navigate to the **Quality** tab.
3. Select your preferred audio codec and quality level.

For a full explanation of available quality options, including lossless and Hi-Res Lossless formats, see [Quality Settings](quality-settings.md).

---

## Your First Download

Once setup is complete and your cookies are configured, downloading music is straightforward:

1. **Open MeedyaDL** if it is not already running.
2. **Paste an Apple Music URL** into the URL input field. The application auto-detects the content type -- whether it is a song, album, playlist, music video, or artist page.
3. **Select quality overrides** if you want to download at a quality different from your default settings. This is optional; your configured defaults will be used otherwise.
4. **Click "Add to Queue"** to begin the download.
5. **Monitor progress** in the download queue, which shows the status of each item.

You can paste multiple URLs and add them to the queue one after another. The queue processes downloads sequentially so you can continue adding items while downloads are in progress.

For more details on downloading music, see [Downloading Music](downloading-music.md). For music videos, see [Downloading Videos](downloading-videos.md).

---

## Next Steps

Once you have MeedyaDL set up and running, explore these topics to make the most of the application:

- [Downloading Music](downloading-music.md) -- Learn about downloading songs, albums, and playlists
- [Quality Settings](quality-settings.md) -- Fine-tune your audio and video quality preferences
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Configure lyric downloads and metadata embedding
- [Cookie Management](cookie-management.md) -- Manage, refresh, and troubleshoot your Apple Music cookies
- [Troubleshooting](troubleshooting.md) -- Solutions for common issues

---

[Back to Help Index](index.md)
