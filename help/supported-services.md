<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Supported Services

MeedyaDL supports downloading from multiple online media services. Each service uses a specialised download engine that handles authentication, content discovery, and file downloading.

---

## Currently Available

### Apple Music

| Feature | Status |
|---------|--------|
| Songs | Supported |
| Albums | Supported |
| Playlists | Supported |
| Music Videos | Supported |
| Artist Pages | Supported |
| Personal Library | Supported |
| Lossless Audio (ALAC) | Supported |
| Dolby Atmos | Supported |
| Synced Lyrics (LRC/TTML) | Supported |
| Animated Artwork | Supported |

**Engine:** [GAMDL](https://github.com/glomatico/gamdl)
**Authentication:** Browser cookies (Netscape format) or wrapper service
**Quality:** Up to lossless ALAC and Dolby Atmos spatial audio

See [Downloading Music](downloading-music.md) for detailed usage instructions.

---

## Coming Soon

### Spotify (Milestone M9 — v2.1.0)

| Feature | Planned |
|---------|---------|
| Songs | Yes |
| Albums | Yes |
| Playlists | Yes |
| Podcasts | Yes |
| Ogg Vorbis audio | Yes |
| Synced Lyrics | Yes |

**Engine:** [votify](https://github.com/glomatico/votify)
**Authentication:** OAuth (cookie-based auth as fallback)
**Note:** Spotify does not offer lossless audio — maximum quality is Ogg Vorbis 320kbps.

### YouTube (Milestone M10 — v2.2.0)

| Feature | Planned |
|---------|---------|
| Videos | Yes |
| Music | Yes |
| Playlists | Yes |
| Subtitles/Captions | Yes |
| Multiple resolutions | Yes |

**Engine:** [yt-dlp](https://github.com/yt-dlp/yt-dlp)
**Authentication:** Optional (cookies for age-restricted/member content)
**Note:** Also covers YouTube Music. Audio extraction available.

### BBC iPlayer (Milestone M8 — v2.0.0)

| Feature | Planned |
|---------|---------|
| TV programmes | Yes |
| Radio programmes | Yes |
| Subtitles | Yes |
| Multiple quality levels | Yes |

**Engine:** [get_iplayer](https://github.com/get-iplayer/get_iplayer) / yt-dlp (fallback)
**Authentication:** None required (UK IP or VPN needed)
**Note:** Region-restricted to the UK.

---

## Service Detection

MeedyaDL automatically detects which service a URL belongs to when you paste it into the download form. Supported URL patterns:

| Service | URL Pattern |
|---------|------------|
| Apple Music | `music.apple.com`, `classical.apple.com`, `itunes.apple.com` |
| Spotify | `open.spotify.com` |
| YouTube | `youtube.com`, `youtu.be` |
| YouTube Music | `music.youtube.com` |
| BBC iPlayer | `bbc.co.uk/iplayer` |

If you paste a URL for a service that isn't yet available, MeedyaDL will show a "support coming soon" message.

---

## When a Service or Feature Is Temporarily Paused

Very occasionally we may need to pause a service or feature for all users at short notice — for example, while we investigate a change on the provider's side. When that happens, MeedyaDL shows an in-app notice so the change reads as what it is: deliberate and temporary, not a fault in your installation. A paused service won't accept new downloads until it returns — anything already downloading finishes normally, and other services are unaffected. The feature comes back automatically once it is re-enabled — there is nothing you need to do. See the Troubleshooting topic ("A Feature Says It Is Temporarily Unavailable") for details.

---

## Multi-Service Features (Planned)

### Smart Download

When multiple services are available, Smart Download will search across all enabled services for the same content and recommend the best quality source. For example, if a song is available in lossless on Apple Music but only in Ogg Vorbis on Spotify, Smart Download will suggest the Apple Music version.

### Cross-Platform Content Matching

Uses industry-standard identifiers (ISRC for tracks, UPC/EAN for albums) to match content across services, with fuzzy title/artist matching as a fallback.

---

[Back to Help Index](index.md)
