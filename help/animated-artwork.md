<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :film_frames: Animated Artwork

MeedyaDL can automatically download **animated cover art** (motion artwork) from Apple Music when it's available. These are short looping videos that Apple uses as album artwork in Apple Music's "Now Playing" screen.

---

## What is Animated Artwork?

Many albums on Apple Music include animated cover art -- short, looping video clips that replace the static album cover. These come in two formats:

| Format | Filename | Aspect Ratio | Max Resolution | Description |
|--------|----------|-------------|----------------|-------------|
| **Square** | `FrontCover.mp4` | 1:1 | 3840 x 3840 | Standard square artwork, animated |
| **Portrait** | `PortraitCover.mp4` | 3:4 | 2048 x 2732 | Tall/vertical artwork used on mobile |

Both files are saved as MP4 videos (HEVC H.265) alongside your downloaded album files. By default, these files are **hidden** on your filesystem to keep album folders clean (see [File Hiding](#file-hiding) below).

> **Note:** Not all albums have animated artwork. When it's not available, MeedyaDL simply skips this step -- no errors are shown.

---

## Requirements

To use this feature, you need:

1. **An Apple Developer account** (free tier is sufficient)
2. **A MusicKit key** created in the Apple Developer portal
3. **FFmpeg** installed (MeedyaDL's setup wizard handles this automatically)

---

## Setup Guide

### Step 1: Create an Apple Developer Account

If you don't already have one, sign up at [developer.apple.com](https://developer.apple.com). The free tier (Apple Developer Program membership is **not** required) is sufficient for MusicKit API access.

### Step 2: Create a MusicKit Key

1. Sign in to the [Apple Developer Portal](https://developer.apple.com/account)
2. Navigate to **Certificates, Identifiers & Profiles**
3. Click **Keys** in the left sidebar
4. Click the **+** button to create a new key
5. Give it a name (e.g., "MeedyaDL")
6. Check the **MusicKit** checkbox
7. Click **Continue**, then **Register**

### Step 3: Download Your Private Key

After creating the key:

1. Click **Download** to save the `.p8` file
2. **Save this file securely** -- Apple only lets you download it once!
3. Note the **Key ID** shown on the key details page (10-character string)

### Step 4: Find Your Team ID

Your Team ID is the 10-character alphanumeric code shown at the **top-right** of the Apple Developer portal (next to your name), or on the **Membership** page.

### Step 5: Configure MeedyaDL

1. Open MeedyaDL and go to **Settings** > **Cover Art** tab
2. Enable **"Download Animated Cover Art"**
3. Enter your **Team ID** in the "MusicKit Team ID" field
4. Enter your **Key ID** in the "MusicKit Key ID" field
5. Open your `.p8` file in a text editor, **copy all content** (including the `-----BEGIN PRIVATE KEY-----` and `-----END PRIVATE KEY-----` lines)
6. Paste it into the "MusicKit Private Key" textarea
7. Click **"Save to Keychain"** -- the key is stored securely in your OS's native keychain
8. Click **Save** to apply your settings

---

## How It Works

After you configure your MusicKit credentials, animated artwork downloading happens automatically:

1. You download an album as usual (paste URL, click Download)
2. After the album download completes successfully, MeedyaDL queries the Apple Music API
3. If animated artwork is available for that album, the HLS video streams are downloaded via FFmpeg
4. The files are saved in the same folder as your downloaded music:
   - `FrontCover.mp4` (square format)
   - `PortraitCover.mp4` (portrait format)

The artwork download runs in the background and does **not** block your download queue -- other downloads continue processing normally.

---

## Output Files

The animated artwork files are placed alongside the album's audio files. For example:

```
Taylor Swift/
  Midnights/
    01 Lavender Haze.m4a
    02 Maroon.m4a
    03 Anti-Hero.m4a
    ...
    FrontCover.mp4        <-- Square animated cover
    PortraitCover.mp4     <-- Portrait animated cover
```

---

## File Hiding

By default, MeedyaDL sets the OS "hidden" attribute on animated artwork files after downloading them. This keeps your album folders clean -- you see only your music files -- while the animated artwork remains accessible to media players and scripts that reference them by name.

### Platform Behavior

| Platform | Mechanism | Original Filename Preserved? | How to View Hidden Files |
| -------- | --------- | ---------------------------- | ------------------------ |
| **macOS** | `chflags hidden` | Yes | Finder: press `Cmd + Shift + .` |
| **Windows** | `attrib +H` | Yes | Explorer: View > Show > Hidden items |
| **Linux** | `.` prefix rename | No (e.g., `.FrontCover.mp4`) | File manager: press `Ctrl + H` or `ls -a` |

> **Note:** On Linux, the only standard mechanism for hiding files is renaming them with a `.` prefix. This means software that looks for `FrontCover.mp4` by exact name will not find the file on Linux when hiding is enabled. On macOS and Windows, the original filenames are preserved.

### Disabling File Hiding

If you prefer to keep animated artwork files visible:

1. Go to **Settings** > **Cover Art** tab
2. Enable **"Download Animated Cover Art"** (if not already enabled)
3. Disable **"Hide Animated Artwork Files"**

Files downloaded after this change will remain visible. Previously hidden files can be revealed using the platform-specific methods listed above.

---

## Limitations

- **Album-level only**: Animated artwork is an album property, not per-track. Even when downloading a single track, the artwork for the full album is fetched.
- **Not all albums have it**: Animated artwork is primarily available for newer, higher-profile albums. Older or less popular albums typically only have static cover art.
- **No metadata embedding**: There is no widely-supported standard for embedding animated cover art inside audio file metadata. The MP4/M4A `covr` atom and ID3v2 `APIC` frame only support JPEG and PNG images. Sidecar files are the industry-standard approach.
- **HEVC codec**: The animated artwork is encoded in HEVC (H.265). Most modern media players support this, but very old software may not be able to play the files.

---

## Troubleshooting

### "Animated artwork skipped" / No files appear

- **Check credentials**: Ensure Team ID, Key ID, and private key are all configured correctly in Settings > Cover Art
- **Verify the key is saved**: The status should show "Private key is stored in OS keychain"
- **Not all albums have it**: Try a popular recent album (e.g., a top-charting album) to verify your setup works

### "Invalid MusicKit private key"

- Make sure you copied the **entire** `.p8` file content, including the `-----BEGIN PRIVATE KEY-----` and `-----END PRIVATE KEY-----` header/footer lines
- The key must be a valid PKCS#8 PEM-encoded EC private key (P-256 curve)

### "Apple Music API returned HTTP 401"

- Your MusicKit key may have been revoked in the Apple Developer portal
- The Team ID or Key ID may be incorrect -- double-check them in the Developer portal

### "FFmpeg not installed"

- Animated artwork download requires FFmpeg to convert HLS streams to MP4
- Run the Setup Wizard or install FFmpeg from the Settings > Paths tab

---

## Privacy & Security

- Your MusicKit **private key** is stored in your operating system's native keychain (macOS Keychain, Windows Credential Manager, or Linux Secret Service). It is never saved in plain text, config files, or logs.
- Your **Team ID** and **Key ID** are stored in the MeedyaDL settings file (they are non-sensitive identifiers).
- API requests are made directly from your device to Apple's servers -- no data passes through MeedyaDL's servers.
- MeedyaDL generates short-lived JWT tokens (1-hour expiry) from your credentials for each API request.

---

[Back to Help Index](index.md)
