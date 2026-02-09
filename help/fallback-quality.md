<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :arrows_counterclockwise: Fallback Quality

This guide explains how gamdl-GUI's quality fallback system works when your preferred audio or video quality is not available for a particular piece of content.

---

## Overview

Not all content on Apple Music is available in every quality option. For example, a track might be available in AAC but not in ALAC, or a music video might be available in 1080p but not in 4K. The fallback quality system ensures that gamdl-GUI can still download content even when your top preference is unavailable, by automatically trying the next best option in a configurable chain.

---

## How Fallback Chains Work

### The Concept

> *Explanation of the fallback chain mechanism.*

When you initiate a download, gamdl-GUI attempts to fetch the content in your preferred quality. If that quality is not available, it moves to the next option in the fallback chain, and continues down the chain until it finds an available quality or exhausts all options.

Placeholder for a visual diagram or example:

```
Preferred: ALAC (24-bit/192kHz)
    |
    v  (not available)
Fallback 1: ALAC (24-bit/96kHz)
    |
    v  (not available)
Fallback 2: ALAC (16-bit/44.1kHz)
    |
    v  (available!)
Downloaded in: ALAC (16-bit/44.1kHz)
```

### Audio Fallback Chain

> *Details on the default audio quality fallback order.*

Placeholder for the default audio fallback chain, listing the order in which audio codecs and quality levels are attempted.

### Video Fallback Chain

> *Details on the default video quality fallback order.*

Placeholder for the default video fallback chain, listing the order in which video resolutions and codecs are attempted.

---

## Configuring Fallback Priorities

### Accessing Fallback Settings

> *How to find and modify fallback settings in gamdl-GUI.*

Placeholder for step-by-step instructions on navigating to the fallback configuration in the gamdl-GUI settings panel.

### Reordering the Fallback Chain

> *How to customize the order of fallback quality options.*

Placeholder for details on:

- Drag-and-drop reordering of fallback priorities
- Adding or removing quality options from the chain
- Saving and applying custom fallback configurations

### Disabling Fallback

> *How to disable automatic fallback and require an exact quality match.*

Placeholder for details on disabling the fallback system entirely, which would cause downloads to fail if the exact preferred quality is not available, rather than falling back to an alternative.

---

## Fallback Behavior by Content Type

### Songs and Albums

> *How fallback works for audio downloads.*

Placeholder for details specific to how fallback is applied when downloading individual songs, full albums, or playlists. Includes whether fallback is applied per-track or per-album.

See [Downloading Music](downloading-music.md) for general audio download information.

### Music Videos

> *How fallback works for video downloads.*

Placeholder for details specific to video quality fallback, including how both video resolution and codec are handled in the fallback chain.

See [Downloading Videos](downloading-videos.md) for general video download information.

---

## Notifications and Logging

### Fallback Notifications

> *How gamdl-GUI informs you when a fallback occurs.*

Placeholder for details on:

- In-app notifications when a fallback is triggered
- How to identify which quality was ultimately used for a download
- Summary information in the download queue showing fallback activity

### Log File Details

> *What information about fallback decisions is recorded in log files.*

When troubleshooting quality-related issues, the application log files contain detailed information about fallback decisions. See [Troubleshooting](troubleshooting.md) for log file locations and how to read them.

---

## Examples

### Example 1: Audio Fallback

> *A concrete example of audio fallback in action.*

Placeholder for a walkthrough showing:

1. User requests ALAC (24-bit/192kHz) for a specific album
2. Some tracks are available at that quality, but others are not
3. The fallback chain selects the next available quality for each track
4. The final download contains a mix of qualities (with details logged)

### Example 2: Video Fallback

> *A concrete example of video fallback in action.*

Placeholder for a walkthrough showing:

1. User requests 4K HEVC for a music video
2. The video is only available in 1080p
3. The fallback chain selects 1080p HEVC (or 1080p H.264 if HEVC is unavailable)
4. The video downloads at the fallback quality

---

## Related Topics

- [Quality Settings](quality-settings.md) -- Full details on all available quality options
- [Downloading Music](downloading-music.md) -- Audio download workflow
- [Downloading Videos](downloading-videos.md) -- Video download workflow
- [Troubleshooting](troubleshooting.md) -- Resolving quality-related download issues

---

[Back to Help Index](index.md)
