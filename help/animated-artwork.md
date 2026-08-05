<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
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
| **Portrait** | `FrontCoverPortrait.mp4` | 3:4 | 2048 x 2732 | Tall/vertical artwork used on mobile |

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

If you don't already have one, sign up at [developer.apple.com](https://developer.apple.com).

> **Important:** A free Apple Developer account is all you need. You do **not** need the paid Apple Developer Program membership ($99/year). The free tier provides full access to the MusicKit API.

1. Go to [developer.apple.com](https://developer.apple.com) and click **Account**
2. Sign in with your Apple Account (formerly Apple ID). Any Apple Account will work -- the same one you use for iCloud, the App Store, etc.
3. If prompted, accept the Apple Developer Agreement

### Step 2: Create a MusicKit Key

A MusicKit key is a cryptographic credential that lets MeedyaDL authenticate with the Apple Music API. You create it once in the Apple Developer portal.

1. Sign in to the [Apple Developer Portal](https://developer.apple.com/account)
2. You need to navigate to the **Keys** section. There are two ways to get there depending on your portal layout:
   - **Option A (sidebar):** In the left sidebar, look under **Program resources** and click **Certificates, Identifiers & Profiles**. Then in the left sidebar of that page, click **Keys**.
   - **Option B (direct URL):** Go directly to [developer.apple.com/account/resources/authkeys/list](https://developer.apple.com/account/resources/authkeys/list)
3. Click the **+** (plus) button in the top-right corner to create a new key. If you don't see a **+** button, look for a **Create a key** or **Register a New Key** link instead.
4. On the **"Register a New Key"** page:
   - **Key Name:** Enter a descriptive name (e.g., "MeedyaDL"). This is just a label for your own reference — it doesn't affect functionality.
   - **Key Services:** You need to enable MusicKit access. The checkbox label varies depending on your account type:
     - **Free account:** Look for **MusicKit** as a standalone checkbox
     - **Paid Developer Program:** Look for **Media Services (MusicKit, ShazamKit, Apple Music Feed)**. This is a bundled option — checking it enables MusicKit along with related services. Either label works for MeedyaDL.
   - **Check the box** next to whichever MusicKit option appears for your account
   - If you see a **"Configure"** button next to the MusicKit/Media Services option after checking it: click it. You may be asked to select or create an **App ID**. If so, you can select **any existing App ID** from the dropdown, or create a minimal one (any bundle identifier like `com.example.meedyadl` will work). The App ID does not need to match a real app — it is just a required association in Apple's system. Click **Save** to return to the key registration page.
   - If no "Configure" button appears, you can skip this — not all account types require it.

> **Tip:** If you don't see any MusicKit-related checkbox at all, make sure you're on the correct page ("Register a New Key") and that you've accepted all Apple Developer agreements. Free accounts may need to accept an updated agreement at [developer.apple.com/account](https://developer.apple.com/account) before the MusicKit option appears.

5. Click **Continue** to proceed to the confirmation screen
6. Review the details (key name and enabled services) and click **Register** to create the key

### Step 3: Download Your Private Key

This is the most critical step. Apple generates a private key file (`.p8` format) that you must download immediately.

1. After clicking Register, you will see a confirmation page with a **Download** button
2. Click **Download** to save the `.p8` file (e.g., `AuthKey_ABC1234DEF.p8`)
3. Save the `.p8` file somewhere safe and memorable (e.g., a dedicated folder like `~/Documents/MeedyaDL Keys/`)
4. Note the **Key ID** shown on this page -- it is a 10-character alphanumeric string (e.g., `ABC1234DEF`). You can also find it later on the Keys list page

> :warning: **Apple only lets you download the `.p8` file once.** If you navigate away from this page without downloading, or if you lose the file, you cannot re-download it. You would need to revoke the key and create a new one (Step 2 again).

### Step 4: Find Your Team ID

Your Team ID is a 10-character alphanumeric code (e.g., `ABCDE12345`) that identifies your Apple Developer account. You can find it in several places:

- **Membership page:** In the Apple Developer portal, click **Membership** (or **Membership details**) in the left sidebar. Your Team ID is listed on this page.
- **Top-right corner:** On some portal pages, your Team ID appears next to your name in the top-right.
- **Key ID page:** It may also appear on the Key details page from Step 3.

### Step 5: Extract the Private Key Content

The `.p8` file you downloaded in Step 3 is a plain-text file containing your private key in PEM format. You need to copy its contents into MeedyaDL.

1. Locate the `.p8` file you downloaded (e.g., `AuthKey_ABC1234DEF.p8`)
2. Open it in any **text editor**:
   - **macOS:** TextEdit (set to plain text mode: Format > Make Plain Text), or use Terminal: `cat ~/path/to/AuthKey_ABC1234DEF.p8`
   - **Windows:** Notepad (right-click the file > Open with > Notepad)
   - **Linux:** Any text editor (gedit, nano, kate, etc.)
3. The file contents will look something like this:

   ```text
   -----BEGIN PRIVATE KEY-----
   MIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg... (base64 data)
   ...several lines of base64-encoded data...
   -----END PRIVATE KEY-----
   ```

4. **Select all** the content (`Ctrl+A` / `Cmd+A`), including the `-----BEGIN PRIVATE KEY-----` and `-----END PRIVATE KEY-----` header/footer lines, and **copy** it (`Ctrl+C` / `Cmd+C`)

> **Tip:** The key is typically 4-6 lines long. Make sure you copy everything -- including the "BEGIN" and "END" lines. Missing even one character will cause authentication to fail.

### Step 6: Configure MeedyaDL

1. Open MeedyaDL and go to **Settings** > **Cover Art** tab
2. Enable **"Download Animated Cover Art"**
3. Go to **Settings** > **Advanced** > **API Credentials** section
4. Enter your **Team ID** (from Step 4) in the "MusicKit Team ID" field
5. Enter your **Key ID** (from Step 3) in the "MusicKit Key ID" field
6. Paste the private key content you copied in Step 5 into the **"MusicKit Private Key"** textarea
7. Click **"Save to Keychain"** -- the key is stored securely in your OS's native keychain (macOS Keychain, Windows Credential Manager, or Linux Secret Service). Once saved, the raw key text is discarded from memory and settings
8. The status message should change to **"Private key is stored in OS keychain"**
9. Click **"Test Credentials"** to verify everything works -- the button generates a JWT and makes a test API call to Apple Music, showing success or a specific error message
10. Click **Save** to apply your settings

> **If you lost your `.p8` file:** You will need to revoke the old key and create a new one. In the Apple Developer portal, go to Keys, click on the key you created, click **Revoke**, then repeat from Step 2.

---

## How It Works

After you configure your MusicKit credentials, animated artwork downloading happens automatically:

1. You download an album as usual (paste URL, click Download)
2. After the album download completes successfully, MeedyaDL queries the Apple Music API
3. If animated artwork is available for that album, the HLS video streams are downloaded via FFmpeg
4. The files are saved in the same folder as your downloaded music:
   - `FrontCover.mp4` (square format)
   - `FrontCoverPortrait.mp4` (portrait format)

The artwork download runs in the background and does **not** block your download queue -- other downloads continue processing normally.

---

## Choosing a Resolution

Apple's animated artwork is delivered as an HLS stream with several resolution renditions, similar to how a video streaming service offers multiple quality tiers of the same clip. The **Animated Artwork Resolution** setting in **Settings > Cover Art** (shown once "Download Animated Cover Art" is enabled) controls which rendition MeedyaDL requests:

| Option | Target | Notes |
|--------|--------|-------|
| **Standard (~1080p, recommended)** | Caps at ~1080p | Default. Smallest files -- indistinguishable from higher renditions at the sizes artwork is normally displayed |
| **High (~2160p / 4K)** | Caps at ~2160p | Noticeably larger files for a quality difference most people won't notice |
| **Maximum (highest available, largest files)** | No cap | Always downloads the highest-resolution rendition Apple offers, regardless of size -- MeedyaDL's behaviour before this setting existed |

Higher resolution means a larger file -- a few MB difference per video adds up quickly across a large library, so **Standard** is the recommended default unless you have a specific reason to want the largest available rendition.

---

## Artist Promo Video

Some artists on Apple Music have an animated background video on their artist page (sometimes called "editorial video" or "artist highlight"). When **"Download Artist Promo Video"** is enabled in Settings > Cover Art, MeedyaDL will:

1. Look up the artist's Apple Music page for a promotional video
2. If available, download it as `ArtistSpotlightCover.mp4` to the **artist folder** (the parent of the album directory)
3. Skip the download if `ArtistSpotlightCover.mp4` already exists (idempotent -- won't re-download for every album by the same artist)

> **Note:** Not all artists have a promo video. This feature requires MusicKit credentials (same as animated artwork). The file is hidden automatically if "Hide Animated Artwork Files" is enabled.

---

## Output Files

The animated artwork files are placed alongside the album's audio files. For example:

```
Taylor Swift/
  ArtistSpotlightCover.mp4           <-- Artist promo video (in the artist folder)
  Midnights/
    01 Lavender Haze.m4a
    02 Maroon.m4a
    03 Anti-Hero.m4a
    ...
    FrontCover.mp4          <-- Square animated cover
    FrontCoverPortrait.mp4  <-- Portrait animated cover
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

- **Check credentials**: Ensure Team ID, Key ID, and private key are all configured correctly in Settings > Advanced > API Credentials
- **Verify the key is saved**: The status should show "Private key is stored in OS keychain"
- **Not all albums have it**: Try a popular recent album (e.g., a top-charting album) to verify your setup works

### "Invalid MusicKit private key"

- Make sure you copied the **entire** `.p8` file content, including the `-----BEGIN PRIVATE KEY-----` and `-----END PRIVATE KEY-----` header/footer lines (see [Step 5](#step-5-extract-the-private-key-content) above)
- Do not add extra spaces, newlines, or characters before or after the key text
- The key must be a valid PKCS#8 PEM-encoded EC private key (P-256 curve) -- this is the standard format Apple provides
- If you opened the `.p8` file in a rich-text editor (e.g., Word, Pages), invisible formatting characters may have been inserted. Always use a plain-text editor (see Step 5)
- If you lost your `.p8` file, you must revoke the key in the Apple Developer portal and create a new one (see [Step 2](#step-2-create-a-musickit-key))

### "Apple Music API returned HTTP 401"

- Your MusicKit key may have been revoked in the Apple Developer portal -- check the Keys page to verify the key is still active
- The Team ID or Key ID may be incorrect -- double-check them in the Developer portal (see [Step 3](#step-3-download-your-private-key) and [Step 4](#step-4-find-your-team-id))
- Use the **"Test Credentials"** button in **Settings > Advanced > API Credentials** to validate your credentials. It generates a JWT and makes a test API call, showing exactly what went wrong (expired key, wrong permissions, etc.)

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
