<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024-2026 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :arrows_counterclockwise: Fallback Quality

This guide explains how gamdl-GUI's quality fallback system works when your preferred audio or video quality is not available for a particular piece of content.

---

## Overview

Not all content on Apple Music is available in every quality option. For example, a track might be available in AAC but not in ALAC, or a music video might be available in 1080p but not in 4K. The fallback quality system ensures that gamdl-GUI can still download content even when your top preference is unavailable, by automatically trying the next best option in a configurable chain.

When GAMDL reports a "codec unavailable" error for a track, gamdl-GUI automatically retries the download using the next codec or resolution in your configured fallback chain. This process repeats, moving down the chain, until either a successful download occurs or all options in the chain have been exhausted.

---

## How Fallback Chains Work

### The Concept

A fallback chain is an ordered list of codecs (for audio) or resolutions (for video) that gamdl-GUI will try, in sequence, when your preferred quality is not available for a given track or video. Your preferred quality is always the first item in the chain. If it fails with a "codec unavailable" error, gamdl-GUI moves to the second item, then the third, and so on.

```
Preferred: ALAC
    |
    v  (codec unavailable)
Fallback 1: Atmos
    |
    v  (codec unavailable)
Fallback 2: AC3
    |
    v  (codec unavailable)
Fallback 3: AAC
    |
    v  (available!)
Downloaded in: AAC
```

Only "codec unavailable" errors trigger the fallback mechanism. Other error types are handled differently:

- **Network errors** -- Automatic retry (up to 3 attempts with exponential backoff)
- **Authentication errors** -- Require a cookie refresh; no fallback attempted
- **Not found errors** -- The content does not exist; no fallback attempted
- **Rate limit errors** -- Automatic retry after the rate limit window expires

### Audio Fallback Chain

The default audio fallback chain, in order of priority, is:

1. **ALAC** -- Apple Lossless Audio Codec (lossless)
2. **Atmos** -- Dolby Atmos spatial audio
3. **AC3** -- Dolby Digital surround sound
4. **AAC Binaural** -- AAC with binaural spatial rendering
5. **AAC** -- Standard AAC encoding
6. **AAC Legacy** -- Legacy AAC encoding (widest compatibility)

gamdl-GUI attempts each codec in this order. If your preferred codec (the first item) is unavailable for a track, it tries the next codec in the chain, continuing until a successful download or the end of the chain is reached.

### Video Fallback Chain

The default video fallback chain, in order of priority, is:

1. **2160p** (4K)
2. **1440p** (2K)
3. **1080p** (Full HD)
4. **720p** (HD)
5. **576p** (SD PAL)
6. **480p** (SD NTSC)
7. **360p**
8. **240p**

When a requested resolution is not available, gamdl-GUI falls to the next lower resolution in the chain until it finds one that is available.

---

## Configuring Fallback Priorities

### Accessing Fallback Settings

To configure the fallback chains:

1. Open **Settings** from the application menu or toolbar.
2. Navigate to the **Fallback** tab.
3. The Fallback tab displays two lists side by side:
   - **Audio codecs** -- The ordered list of audio codecs used for fallback.
   - **Video resolutions** -- The ordered list of video resolutions used for fallback.

Changes made in this tab take effect immediately for all subsequent downloads.

### Reordering the Fallback Chain

You can fully customize the order and contents of each fallback chain:

- **Drag-and-drop reordering** -- Click and hold any codec or resolution in the list, then drag it to a new position. The first item in the list is your preferred quality; all subsequent items are tried in order if the preferred quality is unavailable.
- **Adding codecs/resolutions** -- Use the add button to include a codec or resolution that is not currently in the chain. Adding an option makes it available as a potential fallback.
- **Removing codecs/resolutions** -- Select a codec or resolution and remove it from the chain. Removing an option means it will never be used as a fallback, even if it is the only available quality for a given track. This is useful if you want to exclude a specific codec entirely (for example, removing AAC Legacy if you never want legacy-encoded files).

Your customized chain is saved automatically and persists across application restarts.

### Disabling Fallback

To disable automatic fallback entirely, remove all items from the chain except your single preferred quality. With only one codec (or resolution) in the chain:

- If that quality is available, the download proceeds normally.
- If that quality is unavailable, the download fails with a codec error and no fallback is attempted.

This approach is useful when you require an exact quality match and would rather have a failed download than a lower-quality alternative.

---

## Fallback Behavior by Content Type

### Songs and Albums

Fallback is applied **per-track**, not per-album. When downloading an album or playlist, each individual track is independently subject to the fallback chain. This means:

- Some tracks in an album may download in your preferred codec (e.g., ALAC) while others fall back to a different codec (e.g., AAC), depending on what is available for each track.
- The download queue reflects the actual quality used for each track individually.
- A single album download may result in a mix of codecs across its tracks if availability varies.

This per-track approach maximizes the number of successfully downloaded tracks rather than failing the entire album when one track lacks your preferred codec.

See [Downloading Music](downloading-music.md) for general audio download information.

### Music Videos

Video fallback works the same way, stepping through resolutions in the configured chain until an available resolution is found. For example, requesting 4K for a music video that is only available up to 1080p will cause gamdl-GUI to try 2160p, then 1440p, then 1080p, downloading at the first resolution that succeeds.

See [Downloading Videos](downloading-videos.md) for general video download information.

---

## Notifications and Logging

### Fallback Notifications

gamdl-GUI keeps you informed when a fallback occurs:

- **Fallback indicator badge** -- In the download queue UI, any item that used a fallback displays a fallback indicator badge. This badge shows both the originally requested quality and the quality that was actually used for the download (e.g., "Requested: ALAC | Downloaded: AAC").
- **Per-track visibility** -- Because fallback is per-track, you can scan the download queue to see exactly which tracks downloaded at your preferred quality and which required a fallback.
- **Queue summary** -- The download queue provides an at-a-glance summary of fallback activity for the current session.

### Log File Details

Fallback decisions are fully logged for troubleshooting purposes. The logs record:

- Which quality was originally requested for each track.
- Each fallback attempt, including which codec/resolution was tried and whether it succeeded or failed.
- The final quality used for the successful download, or the full chain of failures if all options were exhausted.

When troubleshooting quality-related issues, check the download queue's error and status messages to see which quality was attempted and which succeeded. See [Troubleshooting](troubleshooting.md) for log file locations and how to read them.

---

## Examples

### Example 1: Audio Fallback

A user requests ALAC for a 6-track album. Here is what happens during the download:

1. **Tracks 1-5** -- ALAC is available. All five tracks download successfully in ALAC. No fallback is needed.
2. **Track 6** -- ALAC is not available. gamdl-GUI moves down the fallback chain:
   - Tries **Atmos** -- codec unavailable.
   - Tries **AC3** -- codec unavailable.
   - Tries **AAC** -- available! Track 6 downloads successfully in AAC.
3. **Result** -- The album download completes with tracks 1-5 in ALAC and track 6 in AAC. The download queue shows a fallback indicator badge on track 6, indicating "Requested: ALAC | Downloaded: AAC".

### Example 2: Video Fallback

A user requests 4K (2160p) for a music video. The video is only available up to 1080p. Here is what happens:

1. gamdl-GUI attempts **2160p** -- resolution unavailable.
2. Falls back to **1440p** -- resolution unavailable.
3. Falls back to **1080p** -- available! The video downloads successfully at 1080p.
4. **Result** -- The download queue shows a fallback indicator badge on the video, indicating "Requested: 2160p | Downloaded: 1080p".

---

## Related Topics

- [Quality Settings](quality-settings.md) -- Full details on all available quality options
- [Downloading Music](downloading-music.md) -- Audio download workflow
- [Downloading Videos](downloading-videos.md) -- Video download workflow
- [Troubleshooting](troubleshooting.md) -- Resolving quality-related download issues

---

[Back to Help Index](index.md)
