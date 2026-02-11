<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024-2026 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :pencil2: Lyrics and Metadata

This guide explains how gamdl-GUI handles lyrics in various formats and how metadata is embedded into downloaded files.

---

## Overview

gamdl-GUI can download synchronized lyrics alongside your music and embed rich metadata (title, artist, album art, etc.) directly into downloaded files. This ensures your media library is well-organized and your music player can display lyrics in real time.

---

## Lyric Formats

### LRC (LyRiCs)

LRC is a time-stamped text format and the default lyric format for songs downloaded with gamdl-GUI. Each line in an LRC file pairs a timestamp with the corresponding lyric text:

```
[00:12.34] First line of the song
[00:17.89] Second line of the song
[01:23.45] Another line later in the track
```

**Key details:**

- LRC is one of the most widely supported lyric formats. It works with music players such as foobar2000, MusicBee, Poweramp, and Apple Music (via third-party plugins).
- When downloaded, LRC files are saved as sidecar files (e.g., `Song Title.lrc`) in the same directory as the corresponding audio file, sharing the same base filename.
- The format stores line-level synchronized timestamps, allowing your music player to scroll lyrics in time with playback.

### SRT (SubRip Subtitle)

SRT is a numbered subtitle format with start and end timestamps for each entry. It is the standard subtitle format for video content:

```
1
00:00:12,340 --> 00:00:17,890
First line of the song

2
00:00:17,890 --> 00:01:23,450
Second line of the song
```

**Key details:**

- SRT is the most common subtitle format and is supported by virtually all video players, including VLC, MPV, IINA, and Windows Media Player.
- Each entry contains a sequence number, a time range (start and end), and the subtitle text.
- SRT files are saved as sidecar files (e.g., `Video Title.srt`) alongside the downloaded video file. See also [Downloading Videos](downloading-videos.md) for video-specific output information.
- SRT is an excellent choice if you plan to use lyrics or subtitles with video players or subtitle editors.

### TTML (Timed Text Markup Language)

TTML is an XML-based timed text format used natively by Apple Music. It is the default lyric format for music videos downloaded with gamdl-GUI:

```xml
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:12.340" end="00:00:17.890">First line of the song</p>
      <p begin="00:00:17.890" end="00:01:23.450">Second line of the song</p>
    </div>
  </body>
</tt>
```

**Key details:**

- TTML is the format Apple Music uses internally for its synchronized lyrics, so it preserves the richest timing data available from the source.
- TTML files are saved as sidecar files (e.g., `Song Title.ttml`) alongside the downloaded media file.
- Because TTML is an XML-based standard, it can carry more detailed timing and styling information than simpler text formats.
- TTML has more limited support among third-party music players compared to LRC or SRT. It is best suited for workflows that specifically require Apple's native lyric format or for further processing and conversion.

---

## Configuring Lyric Downloads

### Enabling and Disabling Lyrics

Lyric downloads are controlled from **Settings > Lyrics**. In this tab you will find a toggle to enable or disable lyric downloads globally. When enabled, gamdl-GUI will attempt to fetch synchronized lyrics for every track it downloads. When disabled, no lyric files will be created and no lyrics will be embedded.

### Choosing a Lyric Format

In **Settings > Lyrics**, you can select your preferred default lyric output format from the following options:

| Format | Best For | Notes |
|--------|----------|-------|
| **LRC** | Music / audio files | Widest music player support. Default for songs. |
| **SRT** | Videos / subtitle workflows | Universal video player support. |
| **TTML** | Apple Music native workflows | Richest timing data. Limited third-party support. |

**Guidance:** Choose **LRC** if you primarily listen to music on desktop or mobile players. Choose **SRT** if you download music videos and want subtitles that work everywhere. Choose **TTML** if you need the original Apple Music lyric format for specialized processing.

The Lyrics tab also provides an option to **embed lyrics directly into the media file** rather than (or in addition to) saving them as a sidecar file. When embedding is enabled, the lyric text is written into the file's metadata tags so that players can display lyrics without needing a separate file.

### Lyric File Placement

Lyric sidecar files are saved in the same directory as the downloaded audio or video file. The lyric file uses the same base filename as the media file but with the appropriate extension for the chosen format:

```
/Music/Artist - Song Title.m4a
/Music/Artist - Song Title.lrc      (LRC lyric sidecar)
```

```
/Videos/Artist - Video Title.m4v
/Videos/Artist - Video Title.srt    (SRT subtitle sidecar)
```

This convention ensures that most media players will automatically detect and load the lyrics or subtitles when you play the corresponding file.

---

## Metadata Embedding

### What Metadata Is Embedded

gamdl-GUI embeds the following metadata fields into every downloaded file:

| Field | Description |
|-------|-------------|
| **Title** | The track or video title |
| **Artist** | The performing artist(s) |
| **Album Artist** | The primary album artist |
| **Album** | The album name |
| **Track Number** | The track's position on the album |
| **Disc Number** | The disc number for multi-disc albums |
| **Genre** | The genre classification from Apple Music |
| **Release Date / Year** | The original release date of the track or album |
| **Album Artwork** | Embedded cover art image (see [Album Artwork](#album-artwork) below) |
| **Composer** | The songwriter or composer |
| **Copyright** | Copyright information from Apple Music |
| **Apple Music Catalog ID** | The unique Apple Music identifier for the track |
| **Lyrics** | Synchronized lyrics (when the embed option is enabled in Settings > Lyrics) |

### Audio File Metadata

Metadata embedding differs by audio container format:

- **M4A files** use MP4/iTunes atom tagging. Tags are written as iTunes-style metadata atoms (e.g., `©nam` for title, `©ART` for artist, `covr` for artwork). This is the same tagging system used by iTunes and Apple Music, ensuring full compatibility.
- **FLAC files** use Vorbis comments for text metadata. Tags follow the standard Vorbis comment field names (e.g., `TITLE`, `ARTIST`, `ALBUM`). Album artwork is embedded as a `METADATA_BLOCK_PICTURE` binary image block.

In both formats, artwork is embedded as a binary image atom directly within the file, so your media player can display it without needing a separate image file.

### Video File Metadata

MP4 and M4V video files use the same MP4 atom tagging system as M4A audio files. All metadata fields listed above are embedded in the video container using iTunes-style atoms. See also [Downloading Videos](downloading-videos.md) for video-specific output information.

---

## Album Artwork

### Artwork Resolution

gamdl-GUI downloads album artwork at the full resolution available from Apple Music, which can be up to **3000x3000 pixels**. This ensures your library has the highest quality cover art possible.

Artwork configuration is found in **Settings > Cover Art**, where you can choose:

- **Format:** JPG, PNG, or RAW
  - **JPG** -- Smaller file size with lossy compression. Best for saving storage space while maintaining good visual quality.
  - **PNG** -- Lossless compression. Larger file size but preserves every pixel of the original artwork without compression artifacts.
  - **RAW** -- The original format as delivered by Apple, which is typically JPEG. No re-encoding is applied.
- **Embedding:** Enable or disable embedding artwork directly into the media file's metadata.

### Artwork as Separate Files

In **Settings > Cover Art**, you can enable saving cover art as a standalone image file in addition to (or instead of) embedding it in the media file. When enabled, the artwork is saved as `cover.jpg` or `cover.png` (depending on your chosen format) in the same directory as the downloaded media.

This is useful for media players and library managers (such as Plex, Jellyfin, or Kodi) that look for a `cover.jpg` or `folder.jpg` file in the album directory to display artwork.

---

## Troubleshooting Lyrics and Metadata

### Lyrics Not Available

Not all tracks on Apple Music have synchronized lyrics. If gamdl-GUI does not download a lyric file for a particular track, it is most likely because lyrics are not available for that track in Apple Music's catalog. There is no workaround for this within gamdl-GUI.

### Album Artwork Not Displaying

If embedded artwork is not showing in your media player:

- Verify that your media player supports embedded artwork for the file format you are using (M4A, FLAC, MP4, M4V).
- Try refreshing your media library or clearing your player's metadata cache.
- Check **Settings > Cover Art** to confirm that artwork embedding is enabled.
- Some older players may not support high-resolution artwork. If artwork fails to display, try using JPG format which produces smaller embedded images.

### Metadata Fields Appearing Blank

If metadata fields appear empty in your media player, ensure that the player supports the tagging standard used by your file format (MP4 atoms for M4A/MP4/M4V, Vorbis comments for FLAC). Most modern players handle both standards, but some niche or legacy players may not read all fields.

### Encoding Issues with Special Characters

gamdl-GUI handles UTF-8 encoding automatically for all lyrics and metadata. If you see garbled or incorrect characters, the issue is likely with your media player's text encoding settings rather than with the downloaded files. Check that your player is configured to use UTF-8 encoding for metadata display.

For general troubleshooting, see [Troubleshooting](troubleshooting.md).

---

## Related Topics

- [Downloading Music](downloading-music.md) -- How audio downloads work
- [Downloading Videos](downloading-videos.md) -- How video downloads and subtitles work
- [Quality Settings](quality-settings.md) -- Audio and video format options that affect metadata capabilities
- [Troubleshooting](troubleshooting.md) -- General error resolution

---

[Back to Help Index](index.md)
