<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Supported Services

Today MeedyaDL downloads from Apple Music. Other services are planned. This page shows what works now, what is coming, and how MeedyaDL knows which service a link belongs to.

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

**Sign-in:** your Apple Music cookies (see [Cookie Management](cookie-management.md)), or the optional [wrapper](wrapper.md)
**Quality:** up to lossless ALAC and Dolby Atmos spatial audio

See [Downloading Music](downloading-music.md) for detailed usage instructions.

---

## Coming Soon

### Spotify

| Feature | Planned |
|---------|---------|
| Songs | Yes |
| Albums | Yes |
| Playlists | Yes |
| Podcasts | Yes |
| Ogg Vorbis audio | Yes |
| Synced Lyrics | Yes |

**Note:** Spotify does not offer lossless audio. The planned quality is Ogg Vorbis up to 320 kbps.
**Today:** pasting an `open.spotify.com` link shows that MeedyaDL cannot download from Spotify yet, and the link is not added to the queue.

### YouTube

| Feature | Planned |
|---------|---------|
| Videos | Yes |
| Music | Yes |
| Playlists | Yes |
| Subtitles/Captions | Yes |
| Multiple resolutions | Yes |

**Note:** Also covers YouTube Music. Audio-only downloads are planned too.

### BBC iPlayer

| Feature | Planned |
|---------|---------|
| TV programmes | Yes |
| Radio programmes | Yes |
| Subtitles | Yes |
| Multiple quality levels | Yes |

**Note:** BBC iPlayer is only available in the UK, so you will need to be in the UK.

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

If you paste a link to a service that isn't available yet, the download form says that MeedyaDL cannot download from that service yet (the link itself is fine) and does not add it to the queue. When you paste several links at once, the form counts how many it cannot download yet and queues the rest.

---

## When a Service or Feature Is Temporarily Paused

Very occasionally we may need to pause a service or feature for all users at short notice — for example, while we investigate a change on the provider's side. When that happens, MeedyaDL shows an in-app notice so the change reads as what it is: deliberate and temporary, not a fault in your installation. A paused service won't accept new downloads until it returns — anything already downloading finishes normally, and other services are unaffected. The feature comes back automatically once it is re-enabled — there is nothing you need to do. See the Troubleshooting topic ("A Feature Says It Is Temporarily Unavailable") for details.

---

## Planned Features

These are ideas for when more than one service is available. They do not exist yet.

### Smart Download

Smart Download will search across all enabled services for the same content and recommend the best quality source. For example, if a song is available in lossless on Apple Music but only in Ogg Vorbis on Spotify, Smart Download will suggest the Apple Music version.

### Matching Across Services

Will match the same song or album across services using the standard codes printed on releases, and fall back to matching by title and artist.

---

[Back to Help Index](index.md)
