<!--
  gamdl-GUI Help Documentation
  Copyright (c) 2024-2026 MWBM Partners Ltd
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :clapper: Downloading Videos

This guide explains how to download music videos and post videos from Apple Music using gamdl-GUI, including how to configure video quality options.

---

## Overview

In addition to audio content, gamdl-GUI supports downloading video content from Apple Music. This includes official music videos and post videos (short-form promotional content). Video downloads use the same queue system as music downloads, so you can mix audio and video items in a single session.

---

## Supported Video Types

### Music Videos

Music videos are full-length official videos published by artists on Apple Music. They are identified by `/music-video/` in their Apple Music URL path.

To download a music video:

1. Open Apple Music in your browser and navigate to the music video you want to download.
2. Copy the URL from the address bar. It will look something like:

   ```text
   https://music.apple.com/us/music-video/example-title/1234567890
   ```

3. Paste the URL into the gamdl-GUI URL input field.

The app auto-detects the content as a video based on the `/music-video/` segment in the URL path. No manual selection of content type is needed.

Music video downloads produce an MP4 file in the `.m4v` container format, with the video track and an embedded audio track. The resulting file is tagged with metadata including title, artist, album, and artwork.

### Post Videos

Post videos are a sub-type of music video content on Apple Music. They are typically shorter promotional clips or behind-the-scenes footage published by artists. Post videos are handled identically to music videos within gamdl-GUI -- you simply paste the URL and the app processes it using the same video pipeline.

---

## Video Quality Options

### Available Resolutions

gamdl-GUI supports the following video resolutions, listed from highest to lowest:

| Resolution | Label    | Typical Use Case                              |
|------------|----------|-----------------------------------------------|
| 2160p      | 4K       | Maximum quality; large file sizes             |
| 1440p      | 2K       | High quality with smaller footprint than 4K   |
| 1080p      | Full HD  | Recommended for most users; best balance      |
| 720p       | HD       | Good quality at moderate file sizes           |
| 576p       | --       | Standard PAL resolution                       |
| 480p       | SD       | Standard definition; small file sizes         |
| 360p       | --       | Low quality; minimal storage usage            |
| 240p       | --       | Lowest available quality                      |

**Recommendation:** 1080p is the best choice for most users, offering excellent visual quality without the storage demands of 4K. If you use HEVC (H.265) as the video codec, 1080p HEVC provides the best quality-to-size ratio.

**File size consideration:** 4K (2160p) music videos can be several hundred megabytes per file. Ensure you have sufficient disk space before downloading at this resolution.

### Video Codecs

The video codec determines how the video track is compressed. Different codecs offer different trade-offs between quality, file size, and device compatibility. For a detailed comparison of all available video codecs and their differences, see [Quality Settings](quality-settings.md).

### Audio Track Quality

The audio track embedded within a downloaded video uses the same audio codec settings available in the app. By default, the audio track for video downloads is typically AAC. You can review and adjust audio codec preferences in [Quality Settings](quality-settings.md), though the available options for video audio tracks may differ from standalone audio downloads.

---

## Downloading a Video

### Step-by-Step Process

1. **Copy the video URL** -- Open Apple Music in your browser, navigate to the music video, and copy the URL from the address bar.
2. **Paste the URL** -- In gamdl-GUI, paste the URL into the URL input field.
3. **Auto-detection** -- The app automatically detects that the URL points to video content by recognizing the `/music-video/` path segment. No manual content type selection is required.
4. **Configure quality** -- Set your preferred video resolution and codec in the Settings before downloading, or use the defaults.
5. **Start the download** -- Click the download button. The video is added to the download queue.
6. **Monitor progress** -- Track the download in the queue. Progress tracking shows the download percentage for each item.

### Selecting Quality Before Download

Video quality preferences are configured in **Settings** before you begin downloading. Set your preferred resolution and codec, and gamdl-GUI will use those settings for all subsequent video downloads.

If your preferred resolution is not available for a particular video, the app automatically falls back to the next lower resolution using this chain:

> 2160p --> 1440p --> 1080p --> 720p --> 576p --> 480p --> 360p --> 240p

For example, if you request 1440p but the video is only available up to 1080p, the app will automatically download at 1080p without requiring any manual intervention. See [Fallback Quality](fallback-quality.md) for full details on the fallback system.

---

## Output Files

### Video File Format

Downloaded videos are saved in the **MP4 container format** with the `.m4v` file extension. Each file contains:

- A video track at the selected resolution and codec
- An embedded audio track (typically AAC)
- Embedded metadata (title, artist, album, artwork)

Files are saved to your configured output directory, the same location used for music downloads. File naming follows the template system configured in **Settings > Templates**. See [Lyrics and Metadata](lyrics-and-metadata.md) for more on how templates work.

### Subtitles and Lyrics

Music video downloads can include synchronized lyrics or subtitle tracks. The default lyric format for music videos is **TTML** (Timed Text Markup Language), which is Apple's native subtitle format. An **SRT** (SubRip) file is also downloaded alongside the TTML file for broader compatibility with media players.

You can configure lyric and subtitle format preferences in **Settings > Lyrics tab**. For more information on lyric formats and options, see [Lyrics and Metadata](lyrics-and-metadata.md).

### Metadata Embedding

Downloaded video files are automatically tagged with metadata retrieved from Apple Music, including:

- **Title** -- The name of the music video
- **Artist** -- The performing artist(s)
- **Album** -- The associated album, if applicable
- **Artwork** -- The cover art or video thumbnail, embedded in the file

This metadata ensures your video files are properly organized and display correctly in media players and library applications.

---

## Tips for Video Downloads

Here are some recommendations for getting the best video download experience:

- **Ensure sufficient disk space** -- High-resolution video files are large. 4K videos can be several hundred MB each. Check your available storage before starting a batch of video downloads.
- **Use 1080p HEVC for the best balance** -- 1080p with HEVC (H.265) encoding offers the best quality-to-size ratio for most users. You get Full HD quality with significantly smaller files compared to 4K.
- **Check your cookies before starting** -- Video downloads require valid Apple Music authentication. Verify that your cookies are current and not expired before beginning a video download session. See [Cookie Management](cookie-management.md) for instructions on refreshing cookies.
- **Be mindful of bandwidth** -- Large video files take longer to download. A stable internet connection is recommended, especially for 4K content.
- **Mix audio and video in the queue** -- The download queue handles both music and video items seamlessly. You can paste a mix of song and video URLs without needing to process them separately.

---

## Related Topics

- [Downloading Music](downloading-music.md) -- Download audio-only content
- [Quality Settings](quality-settings.md) -- Full details on video codec and resolution options
- [Fallback Quality](fallback-quality.md) -- Automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Subtitle and metadata options for videos
- [Cookie Management](cookie-management.md) -- Manage authentication cookies
- [Troubleshooting](troubleshooting.md) -- Resolve common video download errors

---

[Back to Help Index](index.md)
