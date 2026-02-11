<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024-2026 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Quality Settings

This guide provides a comprehensive overview of the audio and video quality options available in gamdl-GUI, including codec differences and format trade-offs.

---

## Overview

gamdl-GUI gives you control over the quality and format of your downloaded content. Understanding the available options helps you choose the right balance between audio/video fidelity, file size, and compatibility with your playback devices.

The quality settings in gamdl-GUI map directly to the codec and resolution options supported by the underlying GAMDL command-line tool. Each audio codec and video resolution has distinct characteristics that make it better suited to certain listening or viewing scenarios.

---

## Audio Codecs

### AAC (Advanced Audio Coding)

AAC is the standard lossy audio codec used by Apple Music. It delivers high-quality audio at a fraction of the file size required by lossless formats.

- **Bitrate:** 256 kbps (constant bitrate)
- **Type:** Lossy compression
- **Container:** `.m4a`
- **File size:** Approximately 2 MB per minute of audio
- **Compatibility:** Universal -- plays on virtually every device, operating system, and media player available today, including all Apple devices, Android, Windows, and web browsers
- **Best for:** Everyday listening, portable devices, and situations where storage space or bandwidth is limited. For most listeners, AAC at 256 kbps is indistinguishable from lossless audio in typical listening environments

### AAC-HE (High Efficiency AAC)

AAC-HE is a lower-bitrate variant of the AAC codec, optimized for streaming and bandwidth-constrained scenarios. It uses Spectral Band Replication (SBR) to reconstruct high-frequency content from a smaller encoded payload.

- **Bitrate:** Lower than standard AAC (typically 48-96 kbps)
- **Type:** Lossy compression (high efficiency)
- **Compatibility:** Broadly supported on modern devices and players
- **Best for:** Streaming over slow connections or situations where minimal file size is the priority. Audio quality is noticeably lower than standard AAC 256 kbps, so this codec is not recommended for critical listening

### AAC Binaural

AAC Binaural is Apple's binaural rendering of spatial audio content. It takes Dolby Atmos or other spatial audio mixes and renders them as a two-channel stereo signal specifically designed for headphone playback.

- **Type:** Lossy compression with spatial processing
- **Output:** Stereo (two channels)
- **Container:** `.m4a`
- **Best for:** Listening to spatial audio content on standard stereo headphones. The binaural rendering simulates the immersive spatial audio experience without requiring Atmos-compatible hardware. This is ideal if you want to experience spatial mixes through regular wired or wireless headphones

### AAC Downmix

AAC Downmix produces a standard stereo output by downmixing spatial or surround-sound source material into two channels.

- **Type:** Lossy compression with stereo downmix
- **Output:** Standard stereo (two channels)
- **Container:** `.m4a`
- **Best for:** Obtaining a conventional stereo version of content that was originally mixed in surround or spatial audio. Unlike the Binaural option, the Downmix does not attempt to simulate spatial positioning -- it produces a straightforward stereo mix suitable for speakers or headphones alike

### AAC Legacy

AAC Legacy uses an older AAC encoding profile designed for maximum compatibility with legacy hardware and software.

- **Type:** Lossy compression (legacy profile)
- **Container:** `.m4a`
- **Best for:** Playback on older devices, vintage iPods, early-generation media players, or any hardware or software that may not support newer AAC encoding profiles. Choose this option only if you experience playback issues with standard AAC on older equipment

### ALAC (Apple Lossless Audio Codec)

ALAC is a lossless audio codec developed by Apple. It compresses audio data without discarding any information, meaning the decoded audio is bit-for-bit identical to the original source. This is the highest-fidelity audio option available in gamdl-GUI.

- **Type:** Lossless compression
- **Container:** `.m4a`
- **Sample rates and bit depths available:**
  - **16-bit / 44.1 kHz** -- CD quality. The standard resolution for most music. File size approximately 5 MB per minute
  - **24-bit / 48 kHz** -- Studio quality. Slightly higher resolution than CD. File size approximately 7 MB per minute
  - **24-bit / 96 kHz** -- Hi-Res Audio. Captures detail beyond the range of CD quality. File size approximately 10 MB per minute
  - **24-bit / 192 kHz** -- Maximum resolution Hi-Res Audio. The highest sample rate available from Apple Music. File size approximately 15 MB per minute
- **Compatibility:** All Apple devices (iPhone, iPad, Mac, Apple TV, HomePod), iTunes, and many third-party players that support the ALAC codec. Some non-Apple devices may require conversion to FLAC for playback
- **Best for:** Audiophile listening on high-quality audio equipment, archival purposes where you want to preserve the full quality of the source material, and any scenario where storage space is not a concern

### Atmos (Dolby Atmos)

Dolby Atmos delivers immersive spatial audio using object-based mixing. Rather than assigning sounds to fixed channels, Atmos places audio objects in three-dimensional space, allowing supported playback systems to render sound all around and above the listener.

- **Type:** Spatial / immersive audio
- **Codec:** Enhanced AC-3 (EC-3)
- **Container:** `.m4a`
- **File size:** Varies depending on the complexity of the spatial mix
- **Playback requirements:** Dolby Atmos-compatible hardware or software is required for the full spatial experience. Compatible devices include Apple AirPods Pro, AirPods Max, AirPods (3rd generation and later), Dolby Atmos-enabled speakers, soundbars, and AV receivers. On unsupported devices, the content may be played as a stereo or surround downmix
- **Best for:** Immersive listening experiences where you have compatible playback hardware. Atmos mixes can reveal new details and spatial separation in music that standard stereo cannot provide

### AC3 (Dolby Digital)

AC3, also known as Dolby Digital, is a multichannel surround-sound audio codec widely used in home theater systems.

- **Type:** Lossy compression, multichannel surround
- **Channels:** Up to 5.1 surround sound (five full-range channels plus one low-frequency effects channel)
- **Bitrate:** Approximately 48 kB/s per channel
- **Best for:** Home theater setups with a surround-sound speaker system. AC3 is the standard audio format for DVDs and is widely supported by AV receivers and soundbars. Choose this option if you are downloading music videos or concert content for playback through a traditional surround-sound system

### Ask (Auto Selection)

The "Ask" option defers codec selection to GAMDL's default behavior. When selected, GAMDL will automatically choose the most appropriate audio codec based on what is available for the specific content being downloaded.

- **Best for:** Users who do not have a strong preference for a specific codec and are happy to let the tool select the best available option automatically

---

## Audio Quality Comparison

The table below summarizes the key audio codec options and their characteristics to help you choose the right format for your needs.

| Codec | Type | Typical Bitrate | File Size (per min) | Best For |
| ------- | ------ | ----------------- | --------------------- | ---------- |
| AAC 256 | Lossy | 256 kbps | ~2 MB | General listening, broad device compatibility |
| ALAC 16/44.1 | Lossless | ~800 kbps | ~5 MB | CD-quality listening, good balance of quality and size |
| ALAC 24/96 | Lossless | ~1200 kbps | ~10 MB | Hi-Res audio on quality headphones or speakers |
| ALAC 24/192 | Lossless | ~1600 kbps | ~15 MB | Highest available quality, archival and audiophile use |
| Atmos (EC-3) | Spatial | Varies | Varies | Immersive spatial listening with compatible hardware |

**Notes:**

- File sizes are approximate and vary depending on the dynamic range and complexity of the source material.
- Lossless bitrates are variable because ALAC uses variable-rate compression -- simpler passages compress more than complex ones.
- Atmos file sizes depend on the number of audio objects and the complexity of the spatial mix.

---

## Video Codecs

### H.264 (AVC)

H.264, also known as Advanced Video Coding (AVC), is the most widely supported video codec in use today. Virtually every device, browser, and media player can decode H.264 video without difficulty.

- **Compression efficiency:** Good. H.264 provides solid quality at reasonable file sizes, though it is less efficient than newer codecs like HEVC
- **Compatibility:** Universal. Supported by all modern devices, operating systems, web browsers, smart TVs, and media players
- **Hardware decoding:** Supported on virtually all hardware manufactured in the last decade
- **Best for:** Maximum compatibility. Choose H.264 if you need to play videos on a wide range of devices, including older hardware that may not support HEVC

### H.265 (HEVC)

H.265, also known as High Efficiency Video Coding (HEVC), is the successor to H.264. It achieves significantly better compression, delivering the same visual quality at roughly half the file size, or noticeably better quality at the same file size.

- **Compression efficiency:** Excellent. Approximately 30-50% smaller files compared to H.264 at equivalent visual quality
- **Compatibility:** Supported on most modern devices manufactured from approximately 2017 onward. All recent Apple devices, most recent Android devices, and modern Windows PCs with hardware HEVC decoding support. Some older devices and software may not play HEVC content without additional codec installation
- **Hardware decoding:** Requires a relatively modern GPU or SoC. Available on Apple A9 chip and later, Intel 6th-generation (Skylake) and later, and most recent AMD and NVIDIA GPUs
- **Best for:** Saving storage space while maintaining high visual quality. Choose HEVC if your playback devices support it, especially for high-resolution (1080p and above) content where the file size savings are most significant

---

## Video Resolution Options

gamdl-GUI supports the following video resolutions, listed from highest to lowest quality:

- **2160p (4K Ultra HD)** -- The highest available resolution. Four times the pixel count of 1080p (3840x2160). Ideal for large screens and displays that support 4K. Produces the largest files
- **1440p (2K QHD)** -- Quad HD resolution (2560x1440). A step above Full HD with noticeably sharper detail on larger monitors
- **1080p (Full HD)** -- The recommended resolution for most users (1920x1080). Excellent quality on screens up to approximately 27 inches. The best balance between visual quality and file size
- **720p (HD)** -- Standard HD resolution (1280x720). Good quality on smaller screens such as tablets and phones. Significantly smaller files than 1080p
- **576p (PAL SD)** -- Standard definition at the PAL broadcast standard (720x576). Suitable for content originally produced in PAL regions
- **480p (NTSC SD)** -- Standard definition at the NTSC broadcast standard (720x480). Suitable for content originally produced in NTSC regions
- **360p** -- Low resolution. Very small file sizes. Suitable only for previewing content or extremely limited storage situations
- **240p** -- Minimum resolution. Smallest possible file sizes. Suitable only for thumbnail previews or extremely constrained bandwidth/storage

For more information on downloading videos, see [Downloading Videos](downloading-videos.md).

---

## Video Quality Comparison

The table below summarizes typical file sizes for common resolution and codec combinations, based on a standard 4-minute music video.

| Resolution | Codec | Typical File Size (4 min) | Best For |
| ------------ | ------- | --------------------------- | ---------- |
| 1080p | H.264 | ~150 MB | Broad compatibility across all devices |
| 1080p | HEVC | ~100 MB | Smaller files on modern devices |
| 4K (2160p) | HEVC | ~400 MB | Highest visual quality on large screens |

**Notes:**

- Actual file sizes vary depending on the visual complexity of the content (fast-moving scenes with many details produce larger files).
- 4K content with H.264 encoding is uncommon because HEVC is significantly more efficient at high resolutions.
- For most users, 1080p with HEVC provides the best combination of quality and file size.

---

## Configuring Quality in gamdl-GUI

### Setting Default Quality

You can set your preferred default audio codec and video resolution so that every new download uses your chosen quality settings automatically.

1. Open **Settings** from the application menu or toolbar
2. Navigate to the **Quality** tab
3. Under **Audio Quality**, select your preferred audio codec from the dropdown (AAC, ALAC, Atmos, etc.)
4. Under **Video Quality**, select your preferred video resolution from the dropdown (1080p, 720p, 4K, etc.)
5. Click **Save** or **Apply** to store your defaults

These defaults will be used for all subsequent downloads unless you override them on a per-download basis (see below).

### Per-Download Quality Selection

You can override the default quality settings for any individual download without changing your global defaults.

1. Paste or enter the URL into the download form as usual
2. Before starting the download, expand the **Quality Override** panel in the download form
3. Select the desired audio codec and/or video resolution from the quality selector dropdowns
4. Start the download -- it will use the overridden settings for this download only

The per-download override applies only to that specific download. Your global default settings remain unchanged, and the next download will revert to using your defaults.

### Quality Fallback

When your preferred quality is not available for a particular track or video, gamdl-GUI automatically falls back to the next best available option. For example, if you request ALAC 24/192 but the content is only available at 24/96, gamdl-GUI will download the 24/96 version rather than failing.

For full details on how fallback chains work and how to configure fallback behavior, see [Fallback Quality](fallback-quality.md).

---

## Format Differences and Recommendations

The right quality setting depends on your listening or viewing environment, your playback equipment, and how much storage space you are willing to use. Here are recommendations for common scenarios:

- **Casual mobile listening:** Choose **AAC 256 kbps**. At approximately 2 MB per minute, file sizes are small enough to carry a large library on your phone. The 256 kbps bitrate is transparent (indistinguishable from lossless) for the vast majority of listeners, especially in noisy mobile environments
- **Home audio system:** Choose **ALAC 16-bit/44.1 kHz** (CD quality) or **ALAC 24-bit/96 kHz** (Hi-Res) depending on your equipment. CD quality at ~5 MB per minute is sufficient for most home audio setups. If your DAC and amplifier support Hi-Res audio, 24/96 at ~10 MB per minute offers measurably higher fidelity
- **Archival and preservation:** Choose **ALAC 24-bit/192 kHz** for the highest possible quality at approximately 15 MB per minute. This preserves the full resolution of the source material and can always be converted to lower-quality formats later without re-downloading
- **Video viewing:** Choose **1080p** for the best balance of visual quality and file size. Most screens and viewing distances do not benefit from 4K. Pair with HEVC if your devices support it for smaller file sizes
- **Limited storage:** Choose **AAC 256 kbps** for audio and **720p** for video. This combination provides good quality while keeping file sizes manageable. A typical album will be approximately 80-100 MB in AAC, and a music video approximately 50-75 MB at 720p

---

## Related Topics

- [Downloading Music](downloading-music.md) -- Audio download workflow
- [Downloading Videos](downloading-videos.md) -- Video download workflow
- [Fallback Quality](fallback-quality.md) -- Automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) -- How format choice affects metadata capabilities
- [Getting Started](getting-started.md) -- Initial quality configuration

---

[Back to Help Index](index.md)
