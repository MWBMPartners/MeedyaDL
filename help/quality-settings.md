<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :control_knobs: Quality Settings

This guide provides a comprehensive overview of the audio and video quality options available in gamdl-GUI, including codec differences and format trade-offs.

---

## Overview

gamdl-GUI gives you control over the quality and format of your downloaded content. Understanding the available options helps you choose the right balance between audio/video fidelity, file size, and compatibility with your playback devices.

---

## Audio Codecs

### AAC (Advanced Audio Coding)

> *Details on the AAC audio codec option.*

Placeholder for information on:

- What AAC is and its general characteristics
- Bitrate options available (e.g., 128 kbps, 256 kbps)
- Compatibility with devices and music players
- File extension and container format (`.m4a`)

### AAC-HE (High Efficiency AAC)

> *Details on the HE-AAC audio codec option.*

Placeholder for information on:

- What HE-AAC is and how it differs from standard AAC
- When HE-AAC is a suitable choice
- Bitrate and quality characteristics

### AAC-LC (Low Complexity AAC)

> *Details on the LC-AAC codec variant.*

Placeholder for information on the most common AAC profile and its quality characteristics.

### ALAC (Apple Lossless Audio Codec)

> *Details on the ALAC lossless audio option.*

Placeholder for information on:

- What ALAC is and the benefits of lossless audio
- Typical file sizes compared to lossy formats
- Compatibility with Apple ecosystem devices
- Sample rate and bit depth options (e.g., 16-bit/44.1kHz, 24-bit/48kHz, 24-bit/96kHz, 24-bit/192kHz)

### Dolby Atmos / Spatial Audio

> *Details on Dolby Atmos and Spatial Audio options.*

Placeholder for information on:

- What Dolby Atmos and Spatial Audio are
- How they are delivered through Apple Music
- Playback requirements and compatibility
- File format details for Atmos content

---

## Audio Quality Comparison

> *A comparison table summarizing the audio codec options.*

| Codec | Type | Typical Bitrate | File Size (per min) | Best For |
|-------|------|-----------------|---------------------|----------|
| AAC 256 | Lossy | 256 kbps | *TBD* | General listening, broad compatibility |
| ALAC | Lossless | ~800-1600 kbps | *TBD* | Audiophile listening, archival |
| Atmos | Spatial | Varies | *TBD* | Immersive listening with compatible hardware |

> *This table will be filled in with accurate figures.*

---

## Video Codecs

### H.264 (AVC)

> *Details on the H.264 video codec option.*

Placeholder for information on:

- What H.264 is and its universal compatibility
- Quality and file size characteristics
- Supported resolution range

### H.265 (HEVC)

> *Details on the H.265/HEVC video codec option.*

Placeholder for information on:

- What HEVC is and its improved compression efficiency
- Quality advantages over H.264 at the same bitrate
- Device and software compatibility considerations
- Hardware decoding requirements

---

## Video Resolution Options

> *Details on available video resolution choices.*

Placeholder for a list of supported resolutions:

- **480p** -- Standard definition, smallest file size
- **720p** -- HD, good balance of quality and size
- **1080p** -- Full HD, recommended for most users
- **4K (2160p)** -- Ultra HD, highest quality available (where supported)

For more information on downloading videos, see [Downloading Videos](downloading-videos.md).

---

## Video Quality Comparison

> *A comparison table summarizing video resolution and codec combinations.*

| Resolution | Codec | Typical File Size (per min) | Best For |
|------------|-------|----------------------------|----------|
| 1080p | H.264 | *TBD* | Broad compatibility |
| 1080p | HEVC | *TBD* | Smaller files, modern devices |
| 4K | HEVC | *TBD* | Highest quality, large screens |

> *This table will be filled in with accurate figures.*

---

## Configuring Quality in gamdl-GUI

### Setting Default Quality

> *How to set your preferred default quality for all downloads.*

Placeholder for details on accessing quality settings in the gamdl-GUI preferences and setting default audio and video quality.

### Per-Download Quality Selection

> *How to override the default quality for a specific download.*

Placeholder for details on selecting quality on a per-download basis before starting a download.

### Quality Fallback

When your preferred quality is not available for a particular track or video, gamdl-GUI automatically falls back to the next best option.

For full details on how fallback chains work and how to configure them, see [Fallback Quality](fallback-quality.md).

---

## Format Differences and Recommendations

> *Guidance on choosing the right format for different use cases.*

Placeholder for recommendations based on common scenarios:

- **Casual listening on mobile:** AAC 256 kbps for small file sizes and universal compatibility
- **Home audio system:** ALAC for lossless quality
- **Archival purposes:** ALAC at the highest available sample rate
- **Video viewing:** 1080p HEVC for a good balance of quality and storage

---

## Related Topics

- [Downloading Music](downloading-music.md) -- Audio download workflow
- [Downloading Videos](downloading-videos.md) -- Video download workflow
- [Fallback Quality](fallback-quality.md) -- Automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) -- How format choice affects metadata capabilities
- [Getting Started](getting-started.md) -- Initial quality configuration

---

[Back to Help Index](index.md)
