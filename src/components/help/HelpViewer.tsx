/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Help viewer component.
 * Renders a sidebar navigation of help topics and a Markdown content viewer.
 * Help content is loaded from bundled markdown files.
 * Uses react-markdown with remark-gfm for GitHub-flavored markdown rendering.
 */

import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  BookOpen,
  Download,
  Settings,
  Cookie,
  Wrench,
  Music,
  Video,
  HelpCircle,
  FileText,
} from 'lucide-react';
import { PageHeader } from '@/components/layout';

/** Help topic definition */
interface HelpTopic {
  id: string;
  label: string;
  icon: typeof BookOpen;
  content: string;
}

/** Built-in help topics with inline content */
const HELP_TOPICS: HelpTopic[] = [
  {
    id: 'getting-started',
    label: 'Getting Started',
    icon: BookOpen,
    content: `# Getting Started

## Welcome to GAMDL

GAMDL is a graphical interface for downloading music and videos from Apple Music. This guide will help you get started.

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

## Required Tools

### FFmpeg
Used for audio/video processing and container remuxing. Required for most download operations.

### mp4decrypt
Used for decrypting DRM-protected streams. Essential for downloading protected content.

## Optional Tools

### N_m3u8DL-RE
Alternative HLS stream downloader. Can be used instead of yt-dlp by changing the download mode in Advanced settings.

### MP4Box
Alternative container remuxing tool. Can be used instead of FFmpeg by changing the remux mode in Advanced settings.

### AMDecrypt
Alternative decryption tool for Apple Music content.

## Installation

Tools are automatically downloaded during first-time setup. You can also install or update them from **Settings > Paths** or by re-running the setup wizard.`,
  },
  {
    id: 'audio-codecs',
    label: 'Audio Codecs',
    icon: Music,
    content: `# Audio Codecs

## Available Codecs

### Lossless
- **ALAC** - Apple Lossless Audio Codec. CD quality (16-bit/44.1kHz) or Hi-Res (24-bit/up to 192kHz)

### Spatial Audio
- **Atmos** - Dolby Atmos spatial audio tracks
- **AC3** - Dolby Digital surround sound

### AAC Variants
- **AAC** - 256kbps at up to 48kHz (standard quality)
- **AAC Binaural** - 256kbps binaural rendering of spatial tracks
- **AAC Legacy** - 256kbps at up to 44.1kHz
- **AAC-HE** - High Efficiency AAC (experimental)
- **AAC Downmix** - Downmixed from spatial (experimental)

## Recommendation

For best quality, use **ALAC** (lossless) with the fallback chain enabled. This ensures you get the highest quality available, falling back to compressed formats only when lossless isn't available.`,
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
- Check if FFmpeg is properly installed in **Settings > Paths**

### "Tool Not Found" Errors
- Re-run the setup wizard from **Settings > General**
- Or manually set tool paths in **Settings > Paths**

### Application Won't Start
- Delete the settings file from the app data directory and restart
- On macOS: ~/Library/Application Support/com.mwbm.gamdl-gui/
- On Windows: %APPDATA%/com.mwbm.gamdl-gui/
- On Linux: ~/.config/com.mwbm.gamdl-gui/`,
  },
  {
    id: 'about',
    label: 'About',
    icon: FileText,
    content: `# About GAMDL GUI

## Version
v0.1.0

## Credits
- **GAMDL** by glomatico - The Apple Music download engine
- **Tauri** - Cross-platform desktop framework
- Built with React, TypeScript, and Rust

## License
Copyright (c) 2024-2026 MWBM Partners Ltd
Licensed under the MIT License.

## Links
- GitHub: github.com/MWBM-Partners-Ltd/gamdl-GUI
- GAMDL: github.com/glomatico/gamdl`,
  },
];

/**
 * Renders the help page with a topic sidebar and markdown content viewer.
 */
export function HelpViewer() {
  const [activeTopic, setActiveTopic] = useState('getting-started');

  const topic = HELP_TOPICS.find((t) => t.id === activeTopic) || HELP_TOPICS[0];

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Help"
        subtitle="Documentation and guides for using GAMDL"
      />

      <div className="flex flex-1 overflow-hidden">
        {/* Topic sidebar */}
        <nav className="w-48 flex-shrink-0 border-r border-border-light overflow-y-auto p-2 space-y-0.5">
          {HELP_TOPICS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveTopic(id)}
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
              <Icon size={16} className="flex-shrink-0" />
              {label}
            </button>
          ))}
        </nav>

        {/* Markdown content viewer */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-2xl prose prose-sm dark:prose-invert">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {topic.content}
            </ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}
