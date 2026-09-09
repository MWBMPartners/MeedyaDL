<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# External Tools

MeedyaDL relies on several external command-line tools for downloading, decrypting, and processing media. You can check their status and install or update them from **Settings > Tools** at any time.

## Required Tools

All five tools below are required for full functionality.

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

## Installation & Management

Tools are automatically downloaded during first-time setup. After setup, go to **Settings > Tools** to:

- **Check All** — refresh the status of all tools
- **Install missing tools** — individually or all at once
- **Override paths** — click the chevron on any tool to set a custom binary path (e.g., if you have a system-wide installation you prefer)

If new tools are added in a future update, the Tools tab will show them as missing so you can install them.
