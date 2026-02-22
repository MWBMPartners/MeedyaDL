/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file HelpViewer.tsx -- Help documentation viewer with search.
 *
 * Renders the "Help" page within the main application. The component
 * provides a two-column layout:
 *
 *   - **Left sidebar** -- Searchable list of help topics, each with an
 *     icon and label. Clicking a topic displays its content in the viewer.
 *   - **Right content area** -- Renders the selected topic's Markdown
 *     content using `react-markdown` with the `remark-gfm` plugin.
 *
 * ## Search Feature
 *
 * The sidebar includes a text input that filters topics in real-time:
 *   - Filters by topic label AND topic content (case-insensitive).
 *   - Matched portions of the label are highlighted with `<mark>` elements
 *     via the `HighlightedLabel` sub-component.
 *   - A result count is displayed below the search input.
 *   - A clear (X) button resets the search when text is entered.
 *   - A keyboard shortcut hint (Cmd+K / Ctrl+K) is shown as a visual
 *     placeholder for future shortcut implementation.
 *
 * ## Help Topics
 *
 * Topics are defined as a static `HELP_TOPICS` array of `HelpTopic`
 * objects, each containing an ID, label, icon, and inline Markdown string.
 * Topics cover: Getting Started, Downloading, Settings, Cookies, Tools,
 * Audio Codecs, Music Videos, Troubleshooting, and About.
 *
 * ## Markdown Rendering
 *
 * Content is rendered using:
 *   - `react-markdown` (v9+) -- Core Markdown-to-React renderer
 *     @see {@link https://www.npmjs.com/package/react-markdown}
 *   - `remark-gfm` -- Plugin for GitHub Flavored Markdown support
 *     (tables, strikethrough, task lists, autolinks)
 *     @see {@link https://www.npmjs.com/package/remark-gfm}
 *
 * Tailwind CSS `prose` classes from `@tailwindcss/typography` provide
 * typographic styling with automatic dark mode support via `dark:prose-invert`.
 *
 * ## Sub-components (file-private)
 *
 * - `isMacPlatform()` -- Detects macOS for modifier key display.
 * - `escapeRegExp()` -- Escapes regex special characters in search queries.
 * - `HighlightedLabel` -- Renders a label with search matches highlighted.
 *
 * ## Store Connections
 *
 * This component does NOT connect to any Zustand stores. It is entirely
 * self-contained with local state for the active topic and search query.
 *
 * @see {@link https://www.npmjs.com/package/react-markdown}  -- react-markdown
 * @see {@link https://www.npmjs.com/package/remark-gfm}      -- remark-gfm plugin
 * @see {@link https://react.dev/reference/react/useState}     -- React useState
 * @see {@link https://react.dev/reference/react/useMemo}      -- React useMemo
 * @see {@link https://react.dev/reference/react/useCallback}  -- React useCallback
 * @see {@link https://lucide.dev/}                            -- Lucide icon library
 */

// React hooks: useState for active topic and search state, useMemo for
// memoized filtering and platform detection, useCallback for stable handlers.
import { useState, useEffect, useMemo, useCallback } from 'react';

/**
 * react-markdown -- Renders Markdown strings as React components.
 * Used to display help topic content in the right-side viewer pane.
 * @see https://www.npmjs.com/package/react-markdown
 * @see https://github.com/remarkjs/react-markdown
 */
import ReactMarkdown from 'react-markdown';

/**
 * remark-gfm -- Remark plugin that adds support for GitHub Flavored
 * Markdown (GFM) extensions: tables, strikethrough (~text~), task
 * lists (- [x] item), and autolinks. Passed to ReactMarkdown's
 * `remarkPlugins` prop.
 * @see https://www.npmjs.com/package/remark-gfm
 * @see https://github.github.com/gfm/
 */
import remarkGfm from 'remark-gfm';

// Lucide icons for each help topic in the sidebar.
// Each topic has a dedicated icon for quick visual identification.
import {
  BookOpen, // "Getting Started" topic
  Download, // "Downloading" topic
  Settings, // "Settings" topic
  Cookie, // "Cookies" topic
  Wrench, // "Tools" topic
  Shield, // "Wrapper / AMdecrypt" topic
  Music, // "Audio Codecs" topic
  Video, // "Music Videos" topic
  Film, // "Animated Artwork" topic
  HelpCircle, // "Troubleshooting" topic
  FileText, // "About" topic
  ShieldAlert, // "Disclaimer" topic
  Scale, // "Licenses" topic
  FileCheck, // "Terms of Use" topic
  ShieldCheck, // "Privacy Policy" topic
  Search, // Search icon in the sidebar search bar
  X, // Clear search button icon
} from 'lucide-react';

// Shared layout component for the page header.
import { PageHeader } from '@/components/layout';

// UI store for reading/clearing the help deep-link topic.
import { useUiStore } from '@/stores/uiStore';

/**
 * Shape of a single help topic entry.
 *
 * @property id      - Unique identifier used for the React `key` prop and
 *                     for tracking the active topic in component state.
 * @property label   - Short display name shown in the sidebar navigation.
 *                     Also searched when the user types in the search bar.
 * @property icon    - Lucide icon component rendered next to the label in
 *                     the sidebar. Typed as `typeof BookOpen` (all Lucide
 *                     icons share the same component signature).
 * @property content - Full Markdown content string rendered in the viewer
 *                     pane when this topic is selected. Also searched
 *                     when the user types in the search bar.
 */
interface HelpTopic {
  id: string;
  label: string;
  icon: typeof BookOpen;
  content: string;
}

/**
 * Static array of all built-in help topics.
 *
 * Each topic contains inline Markdown content rather than loading from
 * external files. This approach keeps help content bundled with the
 * application and eliminates the need for async file loading.
 *
 * Topics are displayed in the sidebar in the order they appear in this
 * array. The order is intentional: Getting Started and Downloading come
 * first as the most common entry points, followed by reference material
 * (Settings, Cookies, Tools, Codecs, Videos), troubleshooting, and About.
 */
const HELP_TOPICS: HelpTopic[] = [
  {
    id: 'getting-started',
    label: 'Getting Started',
    icon: BookOpen,
    content: `# Getting Started

## Welcome to MeedyaDL

MeedyaDL is a media downloader application for downloading music and videos. This guide will help you get started.

### First-Time Setup

When you first launch the app, you'll be guided through a setup wizard that:

1. **Installs Python** - A portable Python runtime is downloaded (no system changes)
2. **Installs GAMDL** - The download tool is installed into the portable Python
3. **Installs Tools** - Required tools like FFmpeg are downloaded automatically
4. **Imports Cookies** - You provide your Apple Music authentication cookies

### Downloading Music

1. Copy an Apple Music URL from your browser or the Apple Music app
2. Paste it into the URL field on the Download page
3. (Optional) Adjust quality settings using the override panel
4. Click **Add to Queue**
5. Monitor progress on the Queue page

### Supported Content Types

- **Songs** - Individual tracks
- **Albums** - Complete albums with all tracks
- **Playlists** - User or editorial playlists
- **Music Videos** - Music videos in up to 4K
- **Artist Pages** - Downloads the artist's top songs`,
  },
  {
    id: 'downloading',
    label: 'Downloading',
    icon: Download,
    content: `# Downloading

## How to Download

### Supported URLs

GAMDL supports the following Apple Music URL formats:

- \`https://music.apple.com/{country}/album/{name}/{id}\`
- \`https://music.apple.com/{country}/album/{name}/{id}?i={track_id}\`
- \`https://music.apple.com/{country}/playlist/{name}/{id}\`
- \`https://music.apple.com/{country}/music-video/{name}/{id}\`
- \`https://music.apple.com/{country}/artist/{name}/{id}\`

### Quality Overrides

By default, downloads use the settings from the Quality settings tab. You can override the codec and resolution for individual downloads using the "Quality Overrides" panel on the Download page.

### Fallback Chain

When the preferred codec or resolution is unavailable, GAMDL automatically tries the next option in the fallback chain. Configure the chain order in **Settings > Fallback**.`,
  },
  {
    id: 'settings-help',
    label: 'Settings',
    icon: Settings,
    content: `# Settings

## Configuration Guide

### General
- **Output Directory** - Where files are saved (default: ~/Music/Apple Music)
- **Language** - Metadata language preference
- **Overwrite** - Whether to replace existing files

### Quality
- **Audio Codec** - Default: ALAC (lossless). Options range from lossless to compressed AAC variants
- **Video Resolution** - Default: 2160p (4K). Falls back to lower resolutions if unavailable
- **Fallback** - Enable/disable automatic fallback when preferred quality isn't available

### Paths
Override paths to external tools. Leave empty to use the managed (auto-installed) versions.

### Templates
Customize how files and folders are named using template variables like \`{artist}\`, \`{album}\`, \`{title}\`, \`{track:02d}\`.`,
  },
  {
    id: 'cookies-help',
    label: 'Cookies',
    icon: Cookie,
    content: `# Cookie Authentication

## Why Cookies Are Needed

Apple Music requires authentication to access content. GAMDL uses browser cookies from your Apple Music subscription to authenticate download requests.

## How to Export Cookies

1. Install a **cookies.txt** browser extension:
   - Chrome: "Get cookies.txt LOCALLY" extension
   - Firefox: "cookies.txt" extension
2. Go to **music.apple.com** and log in with your Apple ID
3. Click the extension icon and choose **Export** or **Download**
4. Save the file somewhere accessible

## Importing Cookies

1. Go to **Settings > Cookies** or use the Setup Wizard
2. Click **Browse** and select your cookies.txt file
3. Click **Validate Cookies** to verify they work
4. Save your settings

## Cookie Expiry

Cookies expire after some time. If downloads start failing with authentication errors, export fresh cookies from your browser.`,
  },
  {
    id: 'tools',
    label: 'Tools',
    icon: Wrench,
    content: `# External Tools

MeedyaDL relies on several external command-line tools for downloading, decrypting, and processing media. You can check their status and install or update them from **Settings > Tools** at any time.

## Required Tools

All four tools below are required for full functionality.

### FFmpeg
Used for audio/video processing and container remuxing. Required for most download operations.

### mp4decrypt
Part of the Bento4 toolkit. Used for decrypting DRM-protected streams. Essential for downloading protected content.

### N_m3u8DL-RE
HLS/DASH stream downloader. Used for downloading segmented media streams from Apple Music's CDN.

### MP4Box
Part of the GPAC toolkit. Used for MP4 muxing and remuxing operations.

## Optional Tools

### AMDecrypt
Apple Music DRM decryption tool used with the **wrapper** authentication system. Not required for standard cookie-based authentication. See the **Wrapper / AMdecrypt** help topic for details.

## Installation & Management

Tools are automatically downloaded during first-time setup. After setup, go to **Settings > Tools** to:

- **Check All** — refresh the status of all tools
- **Install missing tools** — individually or all at once
- **Override paths** — click the chevron on any tool to set a custom binary path (e.g., if you have a system-wide installation you prefer)

If new tools are added in a future update, the Tools tab will show them as missing so you can install them.`,
  },
  {
    id: 'wrapper',
    label: 'Wrapper / AMdecrypt',
    icon: Shield,
    content: `# Wrapper / AMdecrypt

The **wrapper** is an alternative authentication method for accessing Apple Music content. Instead of using browser cookies, it uses a locally-running server that handles Apple ID authentication and DRM key exchange.

**Note:** Full managed support (setup wizard, auto-install) is only available on **Linux x86_64**. On other platforms, the Wrapper settings are hidden from the UI but can be configured manually via the settings JSON file. See the Platform Support section below.

## When to Use It

Most users should use **cookie-based authentication** (the default). The wrapper is an advanced option for users who:

- Need more reliable access to **Dolby Atmos** or other DRM-protected formats
- Experience frequent cookie expiration issues
- Are familiar with running local server software

## How It Works

1. A **wrapper service** runs on your computer (typically at \`http://127.0.0.1:30020\`)
2. MeedyaDL connects to the wrapper instead of using cookies
3. The wrapper handles Apple ID login and DRM key exchange on your behalf
4. **AMDecrypt** is the companion decryption tool that works with the wrapper

## Platform Support

The Wrapper service and AMdecrypt have limited platform availability:

| Platform | Wrapper | AMdecrypt | MeedyaDL Integration |
|----------|---------|-----------|---------------------|
| Linux x86_64 | Available | Available | Full managed support (setup wizard, auto-install) |
| macOS (Apple Silicon) | Not available | Available | Manual setup only (settings hidden in UI) |
| macOS (Intel) | Not available | Available | Manual setup only (settings hidden in UI) |
| Windows x64 | Not available | Available | Manual setup only (settings hidden in UI) |
| Windows ARM64 | Not available | Available | Manual setup only (settings hidden in UI) |
| Linux ARM64 | Not available | Available | Manual setup only (settings hidden in UI) |
| Linux ARMv7 | Not available | Not available | Not supported |

### Why Only Linux x86_64?

The Wrapper service only provides Linux x86_64 binaries. It requires the Android NDK and LLVM to build, which are heavily Linux-oriented. On other platforms, MeedyaDL hides the Wrapper settings from the UI to avoid confusion.

### Manual Setup on Other Platforms

Power users on unsupported platforms can still use the Wrapper by:

1. **Running Wrapper remotely** — Run the Wrapper service on a Linux x86_64 server (or VPS) and point MeedyaDL to it via a custom URL
2. **Docker** — Run the Wrapper in a Docker container on any host OS (the Wrapper provides a Docker-based setup)
3. **Edit settings directly** — Open the MeedyaDL settings JSON file (in the app data directory) and set \`"use_wrapper": true\` and \`"wrapper_account_url"\` to the URL of your remote Wrapper service

## Setup

### 1. Obtain and run the wrapper service
The wrapper is a separate application that you run locally. It listens on \`http://127.0.0.1:30020\` by default. You will need to source this separately — it is not bundled with MeedyaDL.

### 2. Install AMDecrypt (optional)
Go to **Settings > Tools** and install AMDecrypt, or set a custom path to an existing AMDecrypt binary.

### 3. Enable the wrapper in MeedyaDL
Go to **Settings > Advanced** and enable the **Use Wrapper** toggle. The default URL (\`http://127.0.0.1:30020\`) should work if the wrapper is running locally with default settings.

### 4. Configure the URL (if needed)
If your wrapper runs on a different port or host, update the **Wrapper Account URL** field in Settings > Advanced.

## Cookie Auth vs Wrapper

| Feature | Cookie Auth | Wrapper |
|---------|-------------|---------|
| Setup difficulty | Easy (browser extension export) | Advanced (local server) |
| Dolby Atmos access | Sometimes unreliable | More reliable |
| Session duration | Cookies expire periodically | Persistent while server runs |
| Dependencies | None (cookies file only) | Wrapper service + AMDecrypt |
| Platform support | All platforms | Linux x86_64 (managed) or manual setup |
| Recommended for | Most users | Advanced users on Linux x86_64 |

## Settings Reference

| Setting | Location | Default |
|---------|----------|---------|
| Use Wrapper | Settings > Advanced | Off |
| Wrapper Account URL | Settings > Advanced | \`http://127.0.0.1:30020\` |
| AMDecrypt path | Settings > Tools > AMDecrypt | Not configured |`,
  },
  {
    id: 'audio-codecs',
    label: 'Audio Codecs',
    icon: Music,
    content: `# Audio Codecs

Understanding the differences between audio codecs helps you choose the right balance between quality, file size, and device compatibility. This guide explains each option in plain language.

---

## Reliability Notice

Most audio codecs are marked **(Experimental)** in the codec selector. This means they may fail intermittently when using cookie-based authentication. Only two codecs are reliably downloadable without the Wrapper service:

- **AAC Legacy** (256kbps at 44.1kHz) — reliable with cookies
- **AAC-HE Legacy** (64kbps) — reliable with cookies

All other codecs — including ALAC (Lossless), Dolby Atmos, AC3, AAC, and AAC Binaural — depend on DRM key exchange that cookies don't always handle correctly. If you experience download failures with experimental codecs, consider:

1. **Retrying** — failures are intermittent, a retry may succeed
2. **Enabling the fallback chain** — Settings > Fallback lets MeedyaDL automatically try the next codec
3. **Using the Wrapper service** — provides more reliable access (Linux x86_64 only, see Help > Wrapper / AMdecrypt)

---

## The Main Codecs Explained

### ALAC — Lossless (Apple Lossless Audio Codec)

ALAC is the highest-quality audio option. It compresses audio without losing any data — the decoded audio is identical to the original studio master. Think of it like a ZIP file for music: smaller than the raw source, but nothing is thrown away.

- **Quality:** Bit-for-bit identical to the source. Available in CD quality (16-bit/44.1kHz), studio quality (24-bit/48kHz), Hi-Res (24-bit/96kHz), and maximum resolution (24-bit/192kHz)
- **File size:** ~5 MB/min (CD quality) to ~15 MB/min (24-bit/192kHz) — roughly 2.5–7× larger than AAC
- **Compatibility:** All Apple devices, iTunes, and many third-party players. Some non-Apple devices may need conversion to FLAC
- **Best for:** Audiophile listening, high-quality speakers/headphones, archival. If you want the absolute best quality and have the storage space, this is the one to choose

### Dolby Atmos — Spatial Audio

Dolby Atmos is an immersive audio format that places sounds in 3D space around you. Instead of traditional stereo (left/right), Atmos positions individual instruments and sounds as "objects" that your playback system renders all around and above you. The result is a more enveloping, cinematic listening experience.

- **Quality:** Depends on the spatial mix — can be stunning on compatible hardware. Encoded as Enhanced AC-3 (EC-3)
- **File size:** Varies by complexity of the spatial mix
- **Compatibility:** Requires Atmos-compatible hardware for the full experience — AirPods Pro, AirPods Max, AirPods 3rd gen+, Dolby Atmos soundbars, AV receivers, and supported speakers. On unsupported devices, it plays as a standard stereo or surround downmix
- **Best for:** Listening through AirPods Pro/Max or a Dolby Atmos home theatre. If you have compatible headphones, Atmos tracks can sound dramatically more spacious and immersive than stereo

### AC3 — Dolby Digital (Surround Sound)

AC3 (also called Dolby Digital) is the classic surround-sound format used in DVDs and home theatres since the 1990s. It delivers up to 5.1 channels: front left, centre, front right, surround left, surround right, plus a subwoofer channel.

- **Quality:** Lossy compression, but designed for surround sound with up to 5.1 channels
- **File size:** Moderate — roughly comparable to AAC
- **Compatibility:** Universally supported by AV receivers, soundbars, and home theatre systems. Less common on phones and portable devices
- **Best for:** Playing through a traditional surround-sound speaker setup (5.1 or 7.1). If you have an AV receiver or soundbar, AC3 will give you multichannel audio without needing Atmos hardware

### AAC — Standard (256 kbps)

AAC (Advanced Audio Coding) at 256 kbps is Apple Music's standard lossy format. It discards audio data that is theoretically inaudible to achieve much smaller file sizes. At 256 kbps, most listeners cannot distinguish it from lossless in a blind test.

- **Quality:** Very good. Transparent to most listeners in everyday environments
- **File size:** ~2 MB/min — the smallest files of the main codecs
- **Compatibility:** Universal. Plays on every device, operating system, browser, and media player
- **Best for:** Everyday listening, phones, portable devices, limited storage. This is the sensible default if you don't have strong feelings about audio quality

### AAC Binaural

AAC Binaural takes a Dolby Atmos or spatial audio mix and renders it as a two-channel stereo signal specifically processed for headphone listening. It simulates the 3D positioning of Atmos using psychoacoustic techniques (head-related transfer functions), so you hear spatial depth and width through ordinary stereo headphones.

- **Quality:** 256 kbps lossy, but with spatial processing applied. Not the same as standard stereo — it's designed to trick your ears into perceiving surround sound
- **File size:** Similar to standard AAC (~2 MB/min)
- **Compatibility:** Plays on any device as a standard stereo .m4a file
- **Best for:** Experiencing spatial audio through regular wired or wireless headphones that don't support Atmos natively. If you want the "immersive" feel but your headphones aren't AirPods Pro/Max, this is the next best thing

### AAC Legacy (256 kbps, 44.1 kHz)

An older AAC encoding profile capped at 44.1 kHz sample rate. Functionally identical to standard AAC for most content, but uses a legacy encoding path designed for maximum compatibility with vintage hardware.

- **Best for:** Older iPods, early-generation media players, or any device that struggles with standard AAC. Only use this if you have playback issues on older equipment

### Experimental Codecs

All codecs except **AAC Legacy** and **AAC-HE Legacy** are marked as **(Experimental)**. This includes the main codecs above (ALAC, Dolby Atmos, AC3, AAC, AAC Binaural) as well as the following niche variants:

- **AAC-HE** — High Efficiency AAC at ~48–96 kbps. Much smaller files but audibly lower quality
- **AAC Downmix** — Surround-to-stereo downmix without binaural processing (a "flat" stereo fold-down)
- **AAC-HE Binaural** — AAC-HE combined with binaural rendering
- **AAC-HE Downmix** — AAC-HE combined with stereo downmix

The "Experimental" label indicates that these codecs may fail intermittently when using cookie-based authentication. The Wrapper service provides more reliable access to all codec types — see Help > Wrapper / AMdecrypt for details.

---

## Pros & Cons Comparison

| Codec | Pros | Cons |
| ----- | ---- | ---- |
| **ALAC (Lossless)** | Perfect quality, no data lost; supports Hi-Res up to 24-bit/192kHz; great for archival | Large files (2.5–7× bigger than AAC); requires more storage; overkill for casual listening |
| **Dolby Atmos** | Immersive 3D spatial audio; stunning on compatible hardware; reveals details stereo cannot | Requires Atmos-compatible headphones/speakers for full effect; falls back to flat stereo on unsupported devices; not all tracks have Atmos mixes |
| **AC3 (Dolby Digital)** | True multichannel surround (5.1); universally supported by home theatre gear | Lossy compression; limited to 5.1 channels; not useful on phones/headphones; fewer tracks available in AC3 than AAC |
| **AAC (256 kbps)** | Universal compatibility; tiny files (~2 MB/min); indistinguishable from lossless for most listeners | Lossy — discards some audio data permanently; not ideal for archival or high-end listening |
| **AAC Binaural** | Simulated spatial audio through any stereo headphones; same small file size as AAC | Lossy; spatial simulation is approximate — not as good as native Atmos on compatible hardware; only useful with headphones, not speakers |

---

## Which Should I Choose?

There are two main goals when choosing a codec, and each has its own recommended setup:

### Recommendation 1: Best Raw Audio Quality → ALAC (Lossless)

If your priority is **pure audio fidelity** — the highest quality, bit-for-bit identical reproduction of the original studio master — choose **ALAC** as your default codec and enable the **fallback chain** in Settings > Quality.

ALAC preserves every detail of the original recording with no data lost. It supports Hi-Res up to 24-bit/192kHz, making it ideal for audiophile listening, high-quality speakers and headphones, and archival. The fallback chain ensures that when lossless isn't available for a particular track, MeedyaDL automatically tries the next codec in your chain (e.g., AAC 256 kbps) so you always get a download.

**Choose ALAC if:** you listen on quality speakers or headphones, you want the best your equipment can reproduce, or you want to build a future-proof archive. ALAC files are larger (2.5–7× bigger than AAC), so make sure you have the storage space.

### Recommendation 2: Immersive Spatial/Multichannel Audio → Dolby Atmos

If your priority is **immersive, three-dimensional audio** — hearing instruments and sounds placed all around you in 3D space — choose **Dolby Atmos** as your default codec and enable the **fallback chain**.

Dolby Atmos uses object-based mixing to position sounds in 3D space rather than just left/right stereo. On compatible hardware (AirPods Pro, AirPods Max, Atmos soundbars, compatible AV receivers), the result is a dramatically more spacious and enveloping listening experience. The fallback chain is especially important here because not every track has an Atmos mix — when Atmos isn't available, MeedyaDL will automatically fall back through AC3 (5.1 surround), AAC Binaural (simulated spatial for regular headphones), and then standard AAC.

**Choose Atmos if:** you have AirPods Pro/Max, a Dolby Atmos soundbar, or a compatible home theatre system, and you want the most immersive listening experience available. Consider enabling a **Companion Download** of ALAC (in Settings > Quality) so you also get a lossless copy of every track alongside the Atmos version.

### Which One Is Right for Me?

| Priority | Recommended Codec | Why |
| -------- | ----------------- | --- |
| Raw quality, perfect reproduction | **ALAC** | Bit-for-bit identical to the studio master. No data lost. Best for quality speakers, headphones, and archival |
| Immersive spatial/3D audio | **Dolby Atmos** | 3D object-based positioning. Instruments surround you. Best for AirPods Pro/Max and Atmos systems |

If you care about **both**, set Dolby Atmos as your default with a Companion Download of ALAC. You'll get the spatial experience on compatible tracks and a lossless backup for everything.

### Other Scenarios

- **"I have a surround-sound system but not Atmos"** → Choose **AC3** (Dolby Digital) for 5.1 multichannel content.
- **"I just want it to work everywhere"** → Choose **AAC** (256 kbps). Smallest files, plays on everything, sounds great.
- **"I want spatial audio but my headphones aren't AirPods"** → Choose **AAC Binaural**. You'll get simulated 3D audio through any standard headphones.

For more details on fallback behaviour, see the Fallback Quality section.`,
  },
  {
    id: 'video',
    label: 'Music Videos',
    icon: Video,
    content: `# Music Videos

## Video Quality

Available resolutions (highest to lowest):
- **2160p** (4K Ultra HD)
- **1440p** (QHD)
- **1080p** (Full HD)
- **720p** (HD)
- **540p** (qHD)
- **480p** (SD)
- **360p** (Low)
- **240p** (Lowest)

## Video Codecs

Configure codec priority in **Settings > Quality**:
- **H.265/HEVC** - Better quality at smaller file sizes (recommended)
- **H.264/AVC** - More compatible, larger file sizes

## Remux Format

Choose the container format in **Settings > Quality**:
- **M4V** - Apple standard format
- **MP4** - Universal compatibility
- **MKV** - Matroska (supports more features)`,
  },
  {
    id: 'animated-artwork',
    label: 'Animated Artwork',
    icon: Film,
    content: `# Animated Artwork

MeedyaDL can automatically download **animated cover art** (motion artwork) from Apple Music. These are short looping videos used as album artwork.

## Requirements

1. **A free Apple Developer account** (no paid membership required)
2. **A MusicKit key** created in the Apple Developer portal
3. **FFmpeg** installed (handled by MeedyaDL's setup wizard)

## Setup Guide

### Step 1: Create an Apple Developer Account

Sign up at [developer.apple.com](https://developer.apple.com) using any Apple Account. Accept the Apple Developer Agreement when prompted. The **free tier** is all you need.

### Step 2: Create a MusicKit Key

1. Sign in to the [Apple Developer Portal](https://developer.apple.com/account)
2. Go to **Certificates, Identifiers & Profiles** (under Program resources)
3. Click **Keys** in the left sidebar
4. Click the **+** button to create a new key
5. Enter a name (e.g., "MeedyaDL"), check **MusicKit**, then click **Continue** > **Register**

### Step 3: Download Your Private Key

After registering, click **Download** to save the \`.p8\` file. Note the **Key ID** shown on this page.

> **Warning:** Apple only lets you download the \`.p8\` file **once**. Save it somewhere safe. If lost, you must revoke and recreate the key.

### Step 4: Find Your Team ID

Your **Team ID** is a 10-character code found on the **Membership** page in the Apple Developer portal, or in the top-right corner of some portal pages.

### Step 5: Extract the Private Key Content

1. Open the \`.p8\` file in any plain text editor (TextEdit, Notepad, etc.)
2. The content looks like:

\`\`\`text
-----BEGIN PRIVATE KEY-----
MIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg...
-----END PRIVATE KEY-----
\`\`\`

3. **Select all** and **copy** the entire content, including the BEGIN/END lines

### Step 6: Configure MeedyaDL

1. Go to **Settings > Cover Art**
2. Enable **"Download Animated Cover Art"**
3. Enter your **Team ID** and **Key ID**
4. Paste the private key content into the **"MusicKit Private Key"** textarea
5. Click **"Save to Keychain"** -- the key is stored securely in your OS keychain
6. Click **Save**

## Troubleshooting

- **"Invalid MusicKit private key"** -- Make sure you copied the complete key including the \`-----BEGIN PRIVATE KEY-----\` and \`-----END PRIVATE KEY-----\` lines
- **Lost your \`.p8\` file** -- Revoke the old key in the Developer portal and create a new one (Step 2)
- **No animated artwork downloaded** -- Not all albums have animated artwork. MeedyaDL silently skips albums without it`,
  },
  {
    id: 'troubleshooting',
    label: 'Troubleshooting',
    icon: HelpCircle,
    content: `# Troubleshooting

## Common Issues

### "Authentication Failed" / Cookie Errors
- Your cookies may have expired. Export fresh cookies from your browser
- Make sure you're logged into music.apple.com before exporting
- Verify cookies using **Settings > Cookies > Validate**

### "Codec Not Available"
- Not all tracks are available in all codecs
- Enable the fallback chain in **Settings > Quality**
- ALAC (lossless) has the widest availability

### Downloads Stuck at 0%
- Check your internet connection
- Try cancelling and re-adding the download
- Check if FFmpeg is properly installed in **Settings > Tools**

### "Tool Not Found" Errors
- Re-run the setup wizard from **Settings > General**
- Or manually set tool paths in **Settings > Tools**

### Application Won't Start
- Delete the settings file from the app data directory and restart
- On macOS: ~/Library/Application Support/io.github.meedyadl/
- On Windows: %APPDATA%/io.github.meedyadl/
- On Linux: ~/.config/io.github.meedyadl/`,
  },
  {
    id: 'disclaimer',
    label: 'Disclaimer',
    icon: ShieldAlert,
    content: `# Disclaimer

## Important Notice

MeedyaDL is provided "as is" without warranty of any kind, express or implied.

### Third-Party Dependencies

MeedyaDL relies on several third-party libraries and services to function, including but not limited to:

- **GAMDL** — the core download engine
- **Python** — runtime environment for GAMDL
- **FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box** — media processing tools

These are independent projects maintained by their respective developers. Changes to these projects may affect MeedyaDL's functionality.

### No Guarantees

- Quality of service, features, and performance are **not guaranteed**
- Third-party services may change, become unavailable, or cease to function at any time
- While we endeavour to provide updates and fixes, this **cannot be guaranteed**
- The developers accept **no liability** for loss of functionality, data, or service

### Your Responsibility

By using MeedyaDL, you acknowledge and accept that:

- You are responsible for complying with all applicable laws and terms of service
- Downloaded content is for personal use in accordance with your existing subscriptions
- The developers are not responsible for how the software is used

### License

MeedyaDL is licensed under the MIT License. See the LICENSE file for full details.`,
  },
  {
    id: 'licenses',
    label: 'Licenses',
    icon: Scale,
    content: `# Third-Party Licenses

MeedyaDL relies on a number of open-source tools, libraries, and runtimes. This page lists the key third-party components and their respective licenses.

## External Tools

| Tool | Purpose | License |
|------|---------|---------|
| **FFmpeg** | Audio/video processing and conversion | LGPL 2.1+ / GPL 2+ |
| **mp4decrypt** (Bento4) | Media decryption | GPL 2 |
| **N_m3u8DL-RE** | HLS/DASH stream downloading | MIT |
| **MP4Box** (GPAC) | MP4 container remuxing | LGPL 2.1 |
| **aria2** | Download acceleration (optional) | GPL 2+ |
| **Chromaprint / fpcalc** | Audio fingerprinting (optional) | LGPL 2.1 |
| **yt-dlp** | YouTube and video platform downloads | Unlicense |
| **get_iplayer** | BBC iPlayer downloads | GPL 3 |

## Python Packages

| Package | Purpose | License |
|---------|---------|---------|
| **GAMDL** | Apple Music download engine | MIT |
| **votify** | Spotify download engine | MIT |
| **gytmdl** | YouTube Music download engine | MIT |
| **yt-dlp** | YouTube/BBC iPlayer download engine | Unlicense |

## Runtimes

| Runtime | Purpose | License |
|---------|---------|---------|
| **Python** (python-build-standalone) | Python runtime for GAMDL, votify, gytmdl, yt-dlp | PSF License |
| **Perl** (relocatable-perl) | Perl runtime for get_iplayer | Artistic License 2.0 / GPL 1+ |

## Key Rust Libraries

| Library | Purpose | License |
|---------|---------|---------|
| **Tauri** | Cross-platform desktop framework | MIT / Apache 2.0 |
| **tokio** | Async runtime | MIT |
| **reqwest** | HTTP client | MIT / Apache 2.0 |
| **serde** | Serialization framework | MIT / Apache 2.0 |
| **mp4ameta** | M4A metadata tagging | MIT |
| **rusty-chromaprint** | Audio fingerprinting (embedded) | MIT |
| **symphonia** | Audio decoding | MPL 2.0 |
| **rookie** | Browser cookie extraction | MIT |
| **keyring** | OS credential storage | MIT / Apache 2.0 |
| **flate2 / tar / zip** | Archive extraction | MIT / Apache 2.0 |

## Key JavaScript Libraries

| Library | Purpose | License |
|---------|---------|---------|
| **React** | UI framework | MIT |
| **Zustand** | State management | MIT |
| **Tailwind CSS** | Utility-first CSS framework | MIT |
| **Lucide** | Icon library | ISC |
| **react-markdown** | Markdown rendering | MIT |
| **i18next** | Internationalisation | MIT |

## Notice

Full license texts are available in each project's respective repository. The binaries and libraries listed above are redistributed under their original licenses. MeedyaDL itself is licensed under the MIT License.`,
  },
  {
    id: 'terms-of-use',
    label: 'Terms of Use',
    icon: FileCheck,
    content: `# Terms of Use

*Last updated: February 2026*

## 1. License

MeedyaDL is free, open-source software distributed under the **MIT License**. You may use, copy, modify, and distribute the software subject to the terms of that license. The software is provided "as is", without warranty of any kind.

## 2. Acceptable Use

MeedyaDL is intended for **personal use** in conjunction with your own valid, paid subscriptions to supported media services (Apple Music, Spotify, YouTube, BBC iPlayer). By using MeedyaDL, you agree to:

- Use the software only in compliance with all applicable laws and regulations in your jurisdiction
- Respect the terms of service of the media platforms you access through MeedyaDL
- Use downloaded content only for personal, non-commercial purposes
- Not redistribute, resell, or publicly share downloaded content

## 3. No Warranty

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NON-INFRINGEMENT.

## 4. Limitation of Liability

IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## 5. Third-Party Dependencies

MeedyaDL relies on third-party tools, libraries, and services (see **Help > Licenses** for a full list). These are independent projects maintained by their respective developers:

- Their availability, functionality, and compatibility are **not guaranteed**
- Changes to upstream projects may affect MeedyaDL's functionality
- Media service APIs may change, become restricted, or cease to function without notice

## 6. Pre-Release Versions

Pre-release versions (alpha, beta, release candidate) are provided for **testing and evaluation purposes**:

- They may contain **bugs, incomplete features, or instability**
- Data loss, configuration corruption, or unexpected behaviour **may occur**
- **No support guarantees** are provided for pre-release versions
- Pre-release versions should **not be relied upon** for critical workflows
- You can roll back to the latest stable release at any time via **Updates > Roll Back to Official Release**

## 7. User Responsibility

You are solely responsible for:

- Ensuring your use of MeedyaDL complies with applicable laws
- Maintaining valid subscriptions to the services you access
- Backing up your data and configuration
- Any consequences arising from your use of the software

## 8. Service Availability

MeedyaDL does not operate any servers or online services. The application connects directly to third-party media service APIs. We make no guarantees about the availability, performance, or continued operation of these services.

## 9. Changes to Terms

These terms may be updated with new versions of the application. Significant changes will be noted in release notes. Continued use of the software after updates constitutes acceptance of any revised terms.`,
  },
  {
    id: 'privacy-policy',
    label: 'Privacy Policy',
    icon: ShieldCheck,
    content: `# Privacy Policy

*Last updated: February 2026*

## Overview

MeedyaDL is a **local-first application**. We do not collect, store, or transmit any user data. There are no analytics, no telemetry, no tracking, and no MeedyaDL-operated servers.

## Data Stored Locally

All application data is stored on your device in the platform app data directory:

- **Settings** — Your configuration preferences (JSON file)
- **Download queue** — Pending and completed download history (JSON file)
- **Cookies** — Media service authentication cookies (Netscape format text file)
- **Log files** — Application logs for debugging (text files)

None of this data is transmitted to MeedyaDL developers or any third party.

## Cookies

MeedyaDL stores media service cookies (Apple Music, Spotify, YouTube, BBC iPlayer) **locally on your device only**. These cookies are:

- Used exclusively for authenticating with the respective media service APIs
- Stored in a standard Netscape-format text file in your app data directory
- Never transmitted to MeedyaDL developers or servers
- Never shared with third parties
- Under your control — you can delete them at any time via **Settings > Cookies**

## Credentials

Service credentials (API keys, tokens) are stored in your operating system's native secure credential storage:

- **macOS**: Keychain
- **Windows**: Credential Manager
- **Linux**: Secret Service (via D-Bus)

These credentials are never transmitted to MeedyaDL developers and are protected by your operating system's security mechanisms.

## Network Requests

MeedyaDL makes network requests only to the following destinations:

| Destination | Purpose | Data Sent |
|-------------|---------|-----------|
| Media service APIs (Apple Music, Spotify, YouTube, BBC iPlayer) | Downloading content | Authentication cookies, content URLs |
| GitHub API (api.github.com) | Checking for app updates | No user data (only IP address visible to GitHub) |
| PyPI (pypi.org) | Checking for GAMDL updates | No user data |

MeedyaDL does **not** operate any proprietary servers. All requests go directly to third-party services.

## Update Checks

When checking for updates, MeedyaDL queries the GitHub Releases API and PyPI. These requests contain no user-identifiable information beyond the requesting IP address (which is standard for any HTTP request). Update checks can be disabled in **Settings > General**.

## Log Files

Application logs are stored locally and may contain:

- File paths on your system
- Error messages and stack traces
- Media service URLs and response codes

Logs are **never transmitted** to MeedyaDL developers. They exist solely for local debugging purposes.

## Third-Party Services

When you use MeedyaDL to access media services, those services' own privacy policies apply. MeedyaDL has no control over how Apple Music, Spotify, YouTube, or BBC iPlayer handle your data.

## Children's Privacy

MeedyaDL is not directed at children under the age of 13 and does not knowingly collect personal information from children.

## Changes to This Policy

This privacy policy may be updated with new versions of the application. Changes will be noted in release notes.

## Contact

MeedyaDL is an open-source project. For privacy concerns or questions, please open an issue on the project's GitHub repository.`,
  },
  {
    id: 'about',
    label: 'About',
    icon: FileText,
    content: `# About MeedyaDL

## Version
v0.3.5

## Credits
- **GAMDL** by glomatico - The Apple Music download engine
- **Tauri** - Cross-platform desktop framework
- Built with React, TypeScript, and Rust

## License
Copyright (c) 2024-2026 MeedyaDL
Licensed under the MIT License.

## Links
- GitHub: github.com/MeedyaDL/MeedyaDL
- GAMDL: github.com/glomatico/gamdl`,
  },
];

/**
 * Detects whether the user is on macOS so we can display the correct
 * modifier key hint (Cmd on macOS, Ctrl on everything else).
 * Uses navigator.platform with a fallback to navigator.userAgent for
 * broader browser compatibility.
 */
function isMacPlatform(): boolean {
  if (typeof navigator !== 'undefined') {
    return (
      navigator.platform?.toUpperCase().includes('MAC') ||
      navigator.userAgent?.toUpperCase().includes('MAC')
    );
  }
  return false;
}

/**
 * Escapes special regex characters in a user-supplied string so it can
 * be safely used inside a RegExp constructor without unintended pattern matching.
 * For example, a search query containing "C++" would be escaped to "C\\+\\+"
 * so the plus signs are matched literally.
 *
 * @param str - The raw string to escape
 * @returns The escaped string safe for RegExp construction
 */
function escapeRegExp(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Renders a label string with search match segments highlighted.
 * Splits the label on case-insensitive matches of the query, then wraps
 * every matched segment in a styled <mark> element to visually distinguish
 * it from the surrounding text.
 *
 * If the query is empty or there are no matches, the label is returned as
 * plain text with no extra markup.
 *
 * @param label - The full label text to render
 * @param query - The current search query to highlight within the label
 * @returns A React fragment containing text nodes and <mark> elements
 */
function HighlightedLabel({ label, query }: { label: string; query: string }) {
  /* When there is no search query, render the label as plain text */
  if (!query.trim()) {
    return <>{label}</>;
  }

  /**
   * Build a case-insensitive regex that captures the matched portion.
   * The capturing group ensures that String.prototype.split retains
   * the matched segments in the resulting array (interleaved between
   * the non-matching parts).
   */
  const regex = new RegExp(`(${escapeRegExp(query.trim())})`, 'gi');
  const parts = label.split(regex);

  return (
    <>
      {parts.map((part, index) => {
        /**
         * Check whether this segment is a match by comparing it
         * case-insensitively against the query. Matched segments
         * receive highlight styling; non-matched segments render
         * as ordinary text.
         */
        const isMatch = part.toLowerCase() === query.trim().toLowerCase();
        return isMatch ? (
          <mark key={index} className="bg-yellow-300/40 text-inherit rounded-sm px-0.5">
            {part}
          </mark>
        ) : (
          <span key={index}>{part}</span>
        );
      })}
    </>
  );
}

/**
 * Renders the help page with a searchable topic sidebar and markdown content viewer.
 *
 * The component maintains two pieces of state:
 * - activeTopic: the ID of the currently selected help topic
 * - searchQuery: the current text in the search input
 *
 * When the user types a search query, the sidebar filters help topics by checking
 * whether the query appears (case-insensitively) in the topic label or markdown
 * content. Matching portions of the label are highlighted inline. A result count
 * is shown below the search input when a query is active.
 */
export function HelpViewer() {
  /** Tracks which help topic is currently displayed in the content viewer */
  const [activeTopic, setActiveTopic] = useState('getting-started');

  /** Tracks the current search input value for filtering the sidebar topics */
  const [searchQuery, setSearchQuery] = useState('');

  /* ---- Store bindings for help deep-linking ---- */
  /** Deep-link topic ID set by HelpButton clicks (null when no deep-link) */
  const helpActiveTopic = useUiStore((s) => s.helpActiveTopic);
  /** Action to clear the deep-link after consuming it */
  const clearHelpActiveTopic = useUiStore((s) => s.clearHelpActiveTopic);

  /**
   * Consume the helpActiveTopic deep-link from the UI store.
   * When a HelpButton sets a topic and navigates here, this effect
   * auto-selects the requested topic and clears the deep-link so
   * subsequent visits to the Help page start on the last-viewed topic.
   */
  useEffect(() => {
    if (helpActiveTopic) {
      // Only navigate if the topic exists in our list
      const exists = HELP_TOPICS.some((t) => t.id === helpActiveTopic);
      if (exists) {
        setActiveTopic(helpActiveTopic);
      }
      clearHelpActiveTopic();
    }
  }, [helpActiveTopic, clearHelpActiveTopic]);

  /**
   * Determine the platform-appropriate modifier key label once.
   * On macOS we show the Cmd symbol; on other platforms we show "Ctrl".
   * This is memoized because isMacPlatform() accesses navigator, and
   * we only need to evaluate it once per component mount.
   */
  const modifierKey = useMemo(() => (isMacPlatform() ? '\u2318' : 'Ctrl'), []);

  /**
   * Filters HELP_TOPICS based on the current searchQuery.
   *
   * Matching logic:
   * - If the query is empty or whitespace-only, all topics are returned.
   * - Otherwise, a topic matches if the query appears anywhere in its
   *   label OR its markdown content (case-insensitive).
   *
   * The result is memoized so the filter only re-runs when the search
   * query actually changes, avoiding unnecessary array iterations on
   * every render.
   */
  const filteredTopics = useMemo(() => {
    const trimmed = searchQuery.trim().toLowerCase();

    /* No query: return the full topic list unfiltered */
    if (!trimmed) {
      return HELP_TOPICS;
    }

    /* Filter topics whose label or content contains the query substring */
    return HELP_TOPICS.filter(
      (topic) =>
        topic.label.toLowerCase().includes(trimmed) || topic.content.toLowerCase().includes(trimmed)
    );
  }, [searchQuery]);

  /**
   * Look up the currently active topic object.
   * Falls back to the first topic in the full list if the active ID
   * is not found (e.g. on initial render or after a state reset).
   */
  const topic = HELP_TOPICS.find((t) => t.id === activeTopic) || HELP_TOPICS[0];

  /**
   * Handles selecting a topic from the sidebar.
   * Updates the active topic state so the content viewer shows
   * the selected topic's markdown.
   */
  const handleTopicSelect = useCallback((topicId: string) => {
    setActiveTopic(topicId);
  }, []);

  /**
   * Handles changes to the search input.
   * Updates the searchQuery state which triggers re-filtering
   * of the sidebar topics via the filteredTopics memo.
   */
  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
  }, []);

  /**
   * Clears the search input and resets the filtered view to show
   * all topics. Called when the user clicks the clear (X) button.
   */
  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  /**
   * Determines whether a search is actively filtering the topic list.
   * Used to conditionally render the result count and clear button.
   */
  const isSearchActive = searchQuery.trim().length > 0;

  return (
    <div className="flex flex-col h-full">
      {/* Page header with title and description */}
      <PageHeader title="Help" subtitle="Documentation and guides for using MeedyaDL" />

      <div className="flex flex-1 overflow-hidden">
        {/* ----------------------------------------------------------------
         * Topic sidebar
         * Contains the search bar and the scrollable list of help topics.
         * The sidebar has a fixed width and does not shrink when the
         * content area needs more space.
         * ---------------------------------------------------------------- */}
        <nav className="w-56 flex-shrink-0 border-r border-border-light overflow-y-auto flex flex-col">
          {/* --------------------------------------------------------------
           * Search bar section
           * Positioned at the top of the sidebar with sticky behavior so
           * it remains visible as the user scrolls through topics.
           * -------------------------------------------------------------- */}
          <div className="sticky top-0 bg-surface-primary z-10 p-2 pb-1 border-b border-border-light">
            {/* Search input wrapper: contains the icon, input, keyboard
                hint, and clear button in a single horizontal row */}
            <div className="relative flex items-center">
              {/* Search icon on the left side of the input */}
              <Search
                size={14}
                className="absolute left-2.5 text-content-tertiary pointer-events-none"
                aria-hidden="true"
              />

              {/* The search text input. Padded on the left to make room
                  for the search icon, and on the right for the keyboard
                  shortcut hint and clear button. */}
              <input
                type="text"
                value={searchQuery}
                onChange={handleSearchChange}
                placeholder="Search topics..."
                aria-label="Search help topics"
                className="
                  w-full pl-8 pr-16 py-1.5
                  text-xs rounded-platform
                  bg-surface-secondary
                  border border-border-light
                  text-content-primary
                  placeholder:text-content-tertiary
                  focus:outline-none focus:ring-1 focus:ring-accent
                  transition-colors
                "
              />

              {/* Right-side controls positioned absolutely within the input.
                  Shows the keyboard shortcut hint when idle, or the clear
                  button when a search query is entered. */}
              <div className="absolute right-2 flex items-center gap-1">
                {isSearchActive ? (
                  /* Clear search button: visible only when there is text
                     in the search input. Resets the query on click. */
                  <button
                    onClick={handleClearSearch}
                    className="
                      p-0.5 rounded
                      text-content-tertiary
                      hover:text-content-primary
                      hover:bg-surface-tertiary
                      transition-colors
                    "
                    aria-label="Clear search"
                    title="Clear search"
                  >
                    <X size={12} />
                  </button>
                ) : (
                  /* Keyboard shortcut hint: shown when the input is empty.
                     Displays Cmd+K on macOS or Ctrl+K on other platforms.
                     This is a visual placeholder for future keyboard
                     shortcut support (the actual shortcut handler is not
                     yet implemented). */
                  <kbd
                    className="
                      hidden sm:inline-flex items-center gap-0.5
                      px-1 py-0.5 rounded
                      text-[10px] leading-none
                      font-mono
                      text-content-tertiary
                      bg-surface-tertiary
                      border border-border-light
                    "
                    title={`${modifierKey}+K to focus search (coming soon)`}
                    aria-label={`Keyboard shortcut: ${modifierKey} plus K (coming soon)`}
                  >
                    {modifierKey}+K
                  </kbd>
                )}
              </div>
            </div>

            {/* Result count: displayed below the search input when the user
                has entered a search query. Shows the number of matching
                topics to give immediate feedback on the search scope. */}
            {isSearchActive && (
              <div className="mt-1 px-1 text-[10px] text-content-tertiary">
                {filteredTopics.length === 1 ? '1 result' : `${filteredTopics.length} results`}
              </div>
            )}
          </div>

          {/* --------------------------------------------------------------
           * Topic list
           * Renders a button for each help topic that passes the current
           * search filter. Each button shows the topic's icon and label.
           * When a search is active, matching portions of the label text
           * are highlighted.
           * -------------------------------------------------------------- */}
          <div className="p-2 space-y-0.5 flex-1">
            {filteredTopics.length > 0 ? (
              filteredTopics.map(({ id, label, icon: Icon }) => (
                <button
                  key={id}
                  onClick={() => handleTopicSelect(id)}
                  className={`
                    w-full flex items-center gap-2.5 px-3 py-2
                    rounded-platform text-sm transition-colors
                    ${
                      activeTopic === id
                        ? 'bg-accent-light text-accent font-medium'
                        : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary'
                    }
                  `}
                >
                  {/* Topic icon: fixed size to maintain alignment across
                      all sidebar entries regardless of label length */}
                  <Icon size={16} className="flex-shrink-0" />

                  {/* Topic label: rendered with search match highlighting
                      when a query is active, or as plain text otherwise */}
                  <span className="truncate">
                    <HighlightedLabel label={label} query={searchQuery} />
                  </span>
                </button>
              ))
            ) : (
              /* Empty state: shown when the search query matches no topics.
                 Provides a visual cue that the filter returned zero results
                 and encourages the user to modify their search. */
              <div className="flex flex-col items-center justify-center py-8 text-center">
                <Search size={24} className="text-content-tertiary mb-2 opacity-50" />
                <p className="text-xs text-content-tertiary">No matching topics found.</p>
                <button
                  onClick={handleClearSearch}
                  className="
                    mt-2 text-xs text-accent
                    hover:text-accent-hover
                    transition-colors
                  "
                >
                  Clear search
                </button>
              </div>
            )}
          </div>
        </nav>

        {/* ----------------------------------------------------------------
         * Markdown content viewer
         * Displays the full markdown content of the currently selected
         * help topic. Uses react-markdown with the remark-gfm plugin
         * to support GitHub-flavored markdown features such as tables,
         * strikethrough, and task lists. The prose classes provide
         * typographic styling with dark mode support.
         * ---------------------------------------------------------------- */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-2xl prose prose-sm dark:prose-invert">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{topic.content}</ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}
