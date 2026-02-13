<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaDL
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :musical_note: Downloading Music

This guide explains how to download songs, albums, playlists, and artist discographies from Apple Music using MeedyaDL.

---

## Overview

MeedyaDL supports downloading audio content from Apple Music by accepting URLs and processing them through the GAMDL backend. You can download individual songs, full albums, entire playlists, or an artist's complete catalog. Simply paste a URL from `music.apple.com` into the download form, choose your preferred audio quality, and the app handles the rest -- including metadata embedding, lyrics, and automatic quality fallback when a codec is unavailable.

---

## Supported URL Types

MeedyaDL auto-detects the content type from the URL path. The following URL types from `music.apple.com` are supported:

### Songs

URLs containing `/song/` download a single track. To get a song URL, open the track in Apple Music (web or app), click the share/copy-link option, and paste the URL into MeedyaDL. The app will fetch metadata, download the audio in your selected codec, embed tags and artwork, and save the file to your configured output directory.

**Example URL format:** `https://music.apple.com/us/song/track-name/1234567890`

### Albums

URLs containing `/album/` download all tracks in the album as a batch. Each track is processed sequentially within the album, and the output is organized into an album folder under the artist directory. Album artwork is embedded into every track. This is the most efficient way to download complete releases, as metadata is fetched once for the entire album.

**Example URL format:** `https://music.apple.com/us/album/album-name/1234567890`

### Playlists

URLs containing `/playlist/` download every track in the playlist. Playlists can contain tracks from different artists and albums, so each track is saved according to its own artist/album metadata. Large playlists are processed track-by-track, and if any individual track fails (for example, due to regional unavailability), the remaining tracks continue downloading.

**Example URL format:** `https://music.apple.com/us/playlist/playlist-name/pl.1234567890`

### Artists

URLs containing `/artist/` download the artist's full catalog. This can be a very large operation depending on the artist's discography. Each album is processed as a separate batch within the queue.

**Example URL format:** `https://music.apple.com/us/artist/artist-name/1234567890`

---

## Using the Download Interface

### Entering URLs

Paste an Apple Music URL into the download form's URL input field. The app automatically detects the content type (song, album, playlist, or artist) from the URL path -- there is no need to manually specify what you are downloading. Only URLs from `music.apple.com` are accepted; other domains will be rejected with a validation error.

To download multiple items, submit each URL individually. Each submission adds the content to the download queue, so you can paste and submit several URLs in succession without waiting for earlier downloads to complete.

### Selecting Quality

Before downloading, you can override the default audio codec using the quality selector on the download form. The available codecs are:

| Codec | Description |
| --- | --- |
| **AAC** | 256 kbps lossy -- the standard Apple Music streaming format. Good balance of quality and file size. |
| **AAC-HE** | High Efficiency AAC -- lower bitrate encoding optimized for constrained bandwidth. |
| **AAC Binaural** | Spatial stereo rendering -- a binaural downmix of spatial audio for headphone listening. |
| **AAC Downmix** | Stereo downmix of multichannel content. |
| **AAC Legacy** | Legacy AAC encoding for maximum compatibility with older devices and software. |
| **ALAC** | Apple Lossless Audio Codec -- lossless compression at various sample rates up to 24-bit/192 kHz. Ideal for archival-quality downloads. |
| **Atmos** | Dolby Atmos spatial audio -- immersive multichannel format for supported playback systems. |
| **AC3** | Dolby Digital surround sound -- 5.1 channel surround encoding. |

If you do not select a codec, the default configured in [Quality Settings](quality-settings.md) is used.

### Managing the Download Queue

Downloads are added to a queue when you submit a URL. The queue processes items sequentially by default, though the concurrency limit is configurable in Settings if you want multiple simultaneous downloads.

Each item in the queue displays:

- **Progress bar** -- real-time download progress for the active item
- **Status** -- the current stage of processing (fetching metadata, downloading, tagging, complete, or failed)
- **Fallback indicator** -- shown if the requested codec was unavailable and the app automatically switched to a different quality

The following queue actions are available:

- **Cancel** -- stops the active download immediately and marks it as cancelled
- **Retry** -- re-queues a failed download so it can be attempted again
- **Clear Finished** -- removes all completed and failed items from the queue list, keeping only pending and active items

---

## Download Progress and Status

MeedyaDL provides real-time progress tracking by parsing output from the GAMDL CLI backend. While a download is active, you can see:

- **Current track** -- the name of the track being processed, updated as the queue moves through an album or playlist
- **Download percentage** -- a progress bar showing how far the current item has progressed
- **Processing stage** -- status messages indicating whether the app is fetching metadata, downloading audio, decrypting, or embedding tags

When a download completes successfully, the item is marked as finished in the queue. If an error occurs, the item is marked as failed with a descriptive error message. Common error types include:

| Error Type | Cause | Resolution |
| --- | --- | --- |
| **auth** | Cookie is expired or invalid | Re-authenticate by updating your cookie in Settings. See [Cookie Management](cookie-management.md). |
| **network** | Connection timeout or server error | Network errors automatically retry 3 times with exponential backoff. If all retries fail, check your internet connection and use Retry. |
| **codec** | Selected audio quality is unavailable for this track | The fallback chain runs automatically. If all codecs fail, the track may not be available in any downloadable format. |
| **not_found** | Content has been removed from Apple Music | The song, album, or playlist no longer exists on Apple Music. No action can resolve this. |
| **rate_limit** | Too many requests sent to Apple Music servers | Wait a few minutes before retrying. Reduce concurrency in Settings if this occurs frequently. |

---

## Output Files

### File Naming

Downloaded files are saved to the output directory configured in Settings. By default, files are organized by **Artist / Album / Track** using GAMDL's template system. You can customize the naming pattern in **Settings > Templates tab** to change the folder hierarchy and file naming scheme.

For example, the default template produces a structure like:

```text
Output Directory/
  Artist Name/
    Album Name/
      01 Track Title.m4a
      02 Track Title.m4a
      ...
```

The file extension depends on the codec used (`.m4a` for AAC and ALAC, `.ec3` for Atmos, `.ac3` for AC3).

### Metadata and Lyrics

GAMDL automatically embeds full metadata into every downloaded file, including:

- Track title, artist, and album artist
- Album name and disc/track number
- Release year and genre
- High-resolution album artwork
- Copyright and label information

Lyrics downloading is configurable in **Settings > Lyrics tab**. The default format is **LRC** (synced lyrics) for songs. Available lyrics formats are:

- **LRC** -- timestamped lyrics for synced playback
- **SRT** -- SubRip subtitle format
- **TTML** -- Timed Text Markup Language (Apple's native lyrics format)

For full details on metadata and lyrics configuration, see [Lyrics and Metadata](lyrics-and-metadata.md).

---

## Fallback Quality

If the codec you selected is not available for a particular track, MeedyaDL automatically tries alternative codecs using a fallback chain. The default music fallback chain is:

ALAC -> Atmos -> AC3 -> AAC Binaural -> AAC -> AAC Legacy

When a fallback occurs, the queue item displays a fallback indicator so you know the final codec differs from your original selection. For full details on configuring fallback behavior, see [Fallback Quality](fallback-quality.md).

---

## Tips and Best Practices

- **Check cookie validity before large batch downloads.** If your authentication cookie has expired mid-way through a large playlist or artist download, all remaining tracks will fail with an auth error. Verify your cookie is current before starting. See [Cookie Management](cookie-management.md).
- **Use ALAC for archival, AAC for everyday listening.** ALAC provides lossless quality but produces larger files (typically 30--50 MB per track). AAC at 256 kbps is effectively transparent for most listeners and uses roughly 7--10 MB per track.
- **Albums download all tracks as a batch.** Submitting an album URL is more efficient than submitting individual song URLs, because metadata is fetched once for the whole album rather than per-track.
- **Monitor the fallback indicator.** If you see frequent fallbacks, the codec you selected may not be widely available. Consider switching your default codec in [Quality Settings](quality-settings.md).
- **Reduce concurrency if you encounter rate limits.** Downloading many items simultaneously can trigger Apple Music's rate limiting. Lowering the concurrency limit in Settings helps avoid this.

---

## Related Topics

- [Quality Settings](quality-settings.md) -- Configure audio codec and quality preferences
- [Fallback Quality](fallback-quality.md) -- Understand automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Configure lyric and metadata options
- [Downloading Videos](downloading-videos.md) -- Download music videos instead of audio
- [Troubleshooting](troubleshooting.md) -- Resolve common download errors

---

[Back to Help Index](index.md)
