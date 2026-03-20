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

/**
 * rehype-raw: Allows raw HTML tags (e.g., <details>, <summary>) to pass
 * through ReactMarkdown without being stripped. Required for collapsible
 * sections in the About page using native HTML disclosure elements.
 * @see {@link https://github.com/rehypejs/rehype-raw}
 */
import rehypeRaw from 'rehype-raw';

// Lucide icons for each help topic in the sidebar.
// Each topic has a dedicated icon for quick visual identification.
import {
  BookOpen, // "Getting Started" topic
  Download, // "Downloading" topic
  Settings, // "Settings" topic
  Cookie, // "Cookies" topic
  Wrench, // "Tools" topic
  Shield, // "Wrapper" topic
  Music, // "Audio Codecs" topic
  Video, // "Music Videos" topic
  Film, // "Animated Artwork" topic
  HelpCircle, // "Troubleshooting" topic
  FileText, // "About" topic
  ShieldAlert, // "Disclaimer" topic
  Keyboard, // "Keyboard Shortcuts" topic
  Search, // Search icon in the sidebar search bar
  X, // Clear search button icon
} from 'lucide-react';

// Tauri app API for reading the version from tauri.conf.json at runtime.
import { getVersion } from '@tauri-apps/api/app';

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
   - Chrome: "[Get cookies.txt LOCALLY](https://chromewebstore.google.com/detail/get-cookiestxt-locally/cclelndahbckbenkjhflpdbgdldlbecc)" extension
   - Firefox: "[cookies.txt](https://addons.mozilla.org/en-US/firefox/addon/cookies-txt/)" extension
2. Go to **[music.apple.com](https://music.apple.com)** and log in with your Apple ID
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

## Installation & Management

Tools are automatically downloaded during first-time setup. After setup, go to **Settings > Tools** to:

- **Check All** — refresh the status of all tools
- **Install missing tools** — individually or all at once
- **Override paths** — click the chevron on any tool to set a custom binary path (e.g., if you have a system-wide installation you prefer)

If new tools are added in a future update, the Tools tab will show them as missing so you can install them.`,
  },
  {
    id: 'wrapper',
    label: 'Wrapper',
    icon: Shield,
    content: `# Wrapper

The **wrapper** is an alternative authentication method for accessing Apple Music content. Instead of using browser cookies, it uses a locally-running server that handles Apple ID authentication and DRM key exchange.

**Note:** The Wrapper service only provides native binaries for **Linux x86_64**. On other platforms, the Wrapper section in Settings > Advanced shows guidance for remote or Docker-based usage.

## When to Use It

Most users should use **cookie-based authentication** (the default). The wrapper is an advanced option for users who:

- Need more reliable access to **Dolby Atmos** or other DRM-protected formats
- Experience frequent cookie expiration issues
- Are familiar with running local server software

## How It Works

1. A **wrapper service** runs on your computer (typically at \`http://127.0.0.1:30020\`)
2. MeedyaDL connects to the wrapper instead of using cookies
3. The wrapper handles Apple ID login, DRM key exchange, and decryption on your behalf

## Platform Support

| Platform | Wrapper | MeedyaDL Integration |
|----------|---------|---------------------|
| Linux x86_64 | Available | Full support (Settings > Advanced) |
| All other platforms | Not natively available | Remote or Docker setup (see below) |

### Why Only Linux x86_64?

The Wrapper service only provides Linux x86_64 binaries. On other platforms, MeedyaDL still shows the Wrapper settings but includes a note about remote usage.

### Remote Setup on Other Platforms

Power users on unsupported platforms can still use the Wrapper by:

1. **Running Wrapper remotely** — Run the Wrapper service on a Linux x86_64 server (or VPS) and point MeedyaDL to it via a custom URL
2. **Docker** — Run the Wrapper in a Docker container on any host OS (the Wrapper provides a Docker-based setup)
3. **Edit settings directly** — Open the MeedyaDL settings JSON file (in the app data directory) and set \`"use_wrapper": true\` and \`"wrapper_account_url"\` to the URL of your remote Wrapper service

## Setup

### 1. Obtain and run the wrapper service
The wrapper is a separate application that you run locally. It listens on \`http://127.0.0.1:30020\` by default. You will need to source this separately — it is not bundled with MeedyaDL.

### 2. Enable the wrapper in MeedyaDL
Go to **Settings > Advanced** and enable the **Use Wrapper** toggle. The default URL (\`http://127.0.0.1:30020\`) should work if the wrapper is running locally with default settings.

### 3. Configure the URL (if needed)
If your wrapper runs on a different port or host, update the **Wrapper Account URL** field in Settings > Advanced.

## Verifying Connectivity

MeedyaDL checks whether the wrapper is reachable in two ways:

### Manual Test

In **Settings > Advanced**, click the **Test Connection** button next to the Wrapper Account URL field. MeedyaDL sends an HTTP GET to the wrapper URL with a 5-second timeout:

- **Success** — shows "Connected" with the round-trip latency in milliseconds (e.g., "Connected (42ms)")
- **Timeout** — "Connection timed out (5s)" — the wrapper may not be running or the URL is wrong
- **Connection refused** — "Connection refused — is the wrapper running at {url}?" — the host is reachable but nothing is listening on that port
- **Other errors** — the specific error message is shown

### Automatic Pre-Flight Check

Every time the download queue starts processing, MeedyaDL runs automatic health checks for internet connectivity, cookies, and (if the wrapper is enabled) the wrapper service. If the wrapper is unreachable, a **yellow toast notification** appears with the specific error message (e.g., "Wrapper service at \`http://127.0.0.1:30020\` timed out — check that it is running").

This check is **advisory** — downloads will still be attempted, but they may fail if the wrapper is genuinely down. Wrapper notifications are **deduplicated** (only one is shown at a time) and **auto-dismiss** when the wrapper becomes reachable again on a subsequent download.

### Troubleshooting Wrapper Connectivity

If MeedyaDL reports the wrapper is unreachable (yellow toast or "Test Connection" failure), work through the steps below from a terminal on the **machine running MeedyaDL**.

#### Step 1 — Verify the URL in MeedyaDL

Open **Settings > Advanced** and check the **Wrapper Account URL**. It should look like:

- Local: \`http://127.0.0.1:30020\` (wrapper on the same machine)
- Remote: \`http://192.168.x.x:30020\` (wrapper on another device, e.g. a Raspberry Pi)

Make sure the IP address and port are correct. If the wrapper is on another device, use that device's LAN IP — not \`127.0.0.1\`.

#### Step 2 — Test from your terminal with curl

Open a terminal on the machine running MeedyaDL and run:

\`\`\`
curl -v http://192.168.x.x:30020
\`\`\`

Replace the URL with your actual wrapper URL. Possible outcomes:

- **A response (any HTTP status)** — the wrapper is running and reachable. If MeedyaDL still fails, double-check the URL matches exactly.
- **"Connection refused"** — the host is reachable but nothing is listening on that port. The wrapper process may not be running, or it's on a different port.
- **"Connection timed out" / no response** — the host is not reachable. Check network, firewall, or IP address.
- **"Could not resolve host"** — the hostname/IP is wrong. Verify the address.

#### Step 3 — Check the wrapper is running on the host

SSH into the wrapper host (or open a terminal locally) and check:

**Docker:**
\`\`\`
docker ps | grep wrapper
\`\`\`
If no output, the container isn't running. Start it with \`docker start <container_name>\` or \`docker compose up -d\`.

Check container logs for errors:
\`\`\`
docker logs <container_name> --tail 50
\`\`\`

**Native (systemd):**
\`\`\`
systemctl status wrapper
\`\`\`

**Native (manual):**
\`\`\`
ps aux | grep wrapper
\`\`\`

If the process isn't running, start it according to the wrapper's own documentation.

#### Step 4 — Check the port is open (remote setups)

If the wrapper is on a different machine, the port must be accessible over the network.

**On the wrapper host**, check the port is listening:
\`\`\`
ss -tlnp | grep 30020
\`\`\`
or:
\`\`\`
netstat -tlnp | grep 30020
\`\`\`

You should see the wrapper listening on \`0.0.0.0:30020\` (all interfaces) or your LAN IP. If it only shows \`127.0.0.1:30020\`, the wrapper is only accepting local connections — you'll need to configure it to bind to \`0.0.0.0\` or the LAN interface.

**Docker port mapping:** ensure the container maps the port to the host. Check with:
\`\`\`
docker port <container_name>
\`\`\`
You should see \`30020/tcp -> 0.0.0.0:30020\`. If not, recreate the container with \`-p 30020:30020\`.

#### Step 5 — Check firewall rules

**Linux (ufw):**
\`\`\`
sudo ufw status
sudo ufw allow 30020/tcp
\`\`\`

**Linux (iptables):**
\`\`\`
sudo iptables -L -n | grep 30020
\`\`\`

**macOS:** Check System Settings > Network > Firewall (or \`/usr/libexec/ApplicationFirewall/socketfilterfw --listapps\`).

**Windows:** Check Windows Defender Firewall > Inbound Rules for port 30020.

**Router/NAT:** If the wrapper is behind a router on a different subnet, you may need port forwarding. For devices on the same LAN, this is usually not needed.

#### Step 6 — Verify from MeedyaDL

After resolving the issue, go back to **Settings > Advanced** and click **Test Connection**. You should see "Connected" with a latency reading. If it still fails, repeat from Step 2.

## Cookie Auth vs Wrapper

| Feature | Cookie Auth | Wrapper |
|---------|-------------|---------|
| Setup difficulty | Easy (browser extension export) | Advanced (local server) |
| Dolby Atmos access | Sometimes unreliable | More reliable |
| Session duration | Cookies expire periodically | Persistent while server runs |
| Dependencies | None (cookies file only) | Wrapper service |
| Platform support | All platforms | Linux x86_64 (native) or remote/Docker |
| Recommended for | Most users | Advanced users needing reliable Atmos access |

## Auto-Retry without Wrapper

When a wrapper download fails (all retries and fallbacks exhausted), MeedyaDL normally shows a **"Retry without Wrapper"** button on the failed queue item. Clicking it re-queues the download with wrapper disabled, falling back to cookie-based authentication.

If you'd prefer this to happen **automatically**, enable **Auto-Retry without Wrapper** in **Settings > Advanced > Wrapper**. When enabled:

1. A wrapper download fails terminally (all retries exhausted)
2. MeedyaDL automatically re-queues the item with wrapper disabled
3. The Activity Log shows "Wrapper failed — auto-retrying without wrapper"
4. The download retries with cookie-based authentication — no manual intervention needed

This is particularly useful if your wrapper service is intermittently unavailable and you want downloads to proceed regardless.

## Settings Reference

| Setting | Location | Default |
|---------|----------|---------|
| Use Wrapper | Settings > Advanced | Off |
| Auto-Retry without Wrapper | Settings > Advanced | Off |
| Wrapper Account URL | Settings > Advanced | \`http://127.0.0.1:30020\` |`,
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
3. **Using the Wrapper service** — provides more reliable access (Linux x86_64 only, see Help > Wrapper)

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

The "Experimental" label indicates that these codecs may fail intermittently when using cookie-based authentication. The Wrapper service provides more reliable access to all codec types — see Help > Wrapper for details.

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
3. Click **[Keys](https://developer.apple.com/account/resources/authkeys/list)** in the left sidebar
4. Click the **+** button to create a new key
5. Enter a name (e.g., "MeedyaDL"), check **MusicKit** or **(Media Services (MusicKit, ShazamKit, Apple Music Feed))**, then click **Continue** > **Register**

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
    id: 'about',
    label: 'About',
    icon: FileText,
    // Content uses {{VERSION}} placeholder, replaced at runtime with the
    // actual app version from tauri.conf.json via getVersion().
    content: `# About MeedyaDL

**Version** v{{VERSION}}

A multiplatform media downloader desktop application. Currently supports Apple Music via GAMDL, with planned support for additional services.

<details>
<summary><strong>Credits</strong></summary>

- **GAMDL** by glomatico — The Apple Music download engine
- **Tauri** — Cross-platform desktop framework
- **React** — User interface library
- **TypeScript** — Type-safe JavaScript
- **Rust** — Backend systems language
- **Tailwind CSS** — Utility-first CSS framework
- **Zustand** — Lightweight state management
- **Vite** — Frontend build tooling

</details>

<details>
<summary><strong>License</strong></summary>

Copyright (c) 2024-2026 MeedyaDL

Licensed under the MIT License. See the LICENSE file in the project root for the full license text.

</details>

<details>
<summary><strong>Links</strong></summary>

- MeedyaDL: [g2my.link/MeedyaDL](https://g2my.link/MeedyaDL)
- GAMDL: [github.com/glomatico/gamdl](https://github.com/glomatico/gamdl)
- Report Issues: [GitHub Issues](https://github.com/MWBMPartners/MeedyaDL/issues)

</details>

<details>
<summary><strong>Open Source Acknowledgements</strong></summary>

MeedyaDL is built on top of many open-source projects. Key dependencies include:

- **GAMDL** (MIT) — Apple Music download engine
- **Tauri** (MIT/Apache-2.0) — Desktop application framework
- **React** (MIT) — UI component library
- **FFmpeg** (LGPL/GPL) — Media processing toolkit
- **mp4decrypt** (MIT) — MP4 decryption utility
- **N_m3u8DL-RE** (MIT) — HLS/DASH stream downloader
- **MP4Box/GPAC** (LGPL) — Media container toolkit

See the project's \`Cargo.toml\` and \`package.json\` for the complete dependency list.

</details>

<details>
<summary><strong>Component Library</strong></summary>

{{COMPONENT_VERSIONS}}

</details>`,
  },
  {
    id: 'keyboard-shortcuts',
    label: 'Keyboard Shortcuts',
    icon: Keyboard,
    content: `# Keyboard Shortcuts

## Application Shortcuts

MeedyaDL supports keyboard shortcuts for fast navigation and common actions.

| Shortcut | Action |
|----------|--------|
| **Cmd/Ctrl + D** | Go to Download page and focus URL input |
| **Cmd/Ctrl + ,** | Open Settings |
| **Cmd/Ctrl + Q** | Open Queue |
| **Escape** | Close active modal/dialog |
| **Cmd/Ctrl + Enter** | Start download (when URL input is focused) |

### Notes

- On **macOS**, use the **Cmd (⌘)** key as the modifier.
- On **Windows** and **Linux**, use the **Ctrl** key.
- Shortcuts are disabled when typing in text inputs, textareas, or dropdown menus to avoid interference.
- **Cmd/Ctrl + D** also selects all text in the URL input for quick replacement.

### Modal Shortcuts

When a modal dialog is open (e.g., Crash Report, file picker):
- **Escape** closes the modal and returns focus to the main content.
- Tab key cycles through focusable elements within the modal (focus trapping).

### Accessibility

- **Tab** moves focus forward through interactive elements.
- **Shift + Tab** moves focus backward.
- **Skip to main content** link appears on first Tab press (top-left corner).`,
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

  /** App version fetched from tauri.conf.json, used in the About topic */
  const [appVersion, setAppVersion] = useState('...');
  /** Component version table (markdown) for the About > Component Library section */
  const [componentVersions, setComponentVersions] = useState('*Loading...*');
  useEffect(() => {
    getVersion()
      .then((v) => setAppVersion(v))
      .catch(() => setAppVersion('unknown'));
  }, []);

  // Fetch component versions when the About topic is selected
  useEffect(() => {
    if (activeTopic !== 'about') return;
    import('@/lib/tauri-commands')
      .then(({ getComponentVersions }) => getComponentVersions())
      .then((versions) => {
        const rows = versions
          .filter((v) => v.installed)
          .map((v) => `| ${v.name} | ${v.version ?? 'unknown'} |`)
          .join('\n');
        const table = `| Component | Version |\n|-----------|--------|\n${rows}`;
        setComponentVersions(table);
      })
      .catch(() => setComponentVersions('*Unable to load component versions.*'));
  }, [activeTopic]);

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
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeRaw]}
              components={{
                // Custom link handler:
                //  - Internal links (#topic-id) navigate within the help viewer
                //  - External links (http/https) open in the system browser
                a: ({ href, children, ...props }) => (
                  <a
                    {...props}
                    href={href}
                    onClick={(e) => {
                      if (!href) return;
                      // Internal help topic link (e.g., #cookies-help)
                      if (href.startsWith('#')) {
                        e.preventDefault();
                        const topicId = href.slice(1);
                        if (HELP_TOPICS.some((t) => t.id === topicId)) {
                          setActiveTopic(topicId);
                        }
                        return;
                      }
                      // External link — open in system default browser
                      if (href.startsWith('http://') || href.startsWith('https://')) {
                        e.preventDefault();
                        import('@tauri-apps/plugin-shell')
                          .then(({ open }) => open(href))
                          .catch(() => {});
                      }
                    }}
                  >
                    {children}
                  </a>
                ),
              }}
            >
              {topic.content
                .replace('{{VERSION}}', appVersion)
                .replace('{{COMPONENT_VERSIONS}}', componentVersions)}
            </ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}
