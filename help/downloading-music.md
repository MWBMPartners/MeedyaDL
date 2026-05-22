<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :musical_note: Downloading Music

This guide explains how to download songs, albums, playlists, and artist discographies from Apple Music using MeedyaDL.

---

## Overview

MeedyaDL supports downloading audio content from Apple Music by accepting URLs and processing them through the GAMDL backend. You can download individual songs, full albums, entire playlists, or an artist's complete catalog. Simply paste a URL from `music.apple.com` into the download form, choose your preferred audio quality, and the app handles the rest -- including metadata embedding, lyrics, and automatic quality fallback when a codec is unavailable.

---

## Supported URL Types

MeedyaDL auto-detects the content type from the URL path. The following URL types from `music.apple.com` are supported (including `classical.apple.com` and `itunes.apple.com`):

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

URLs containing `/artist/` download the artist's catalog. This can be a very large operation depending on the artist's discography. Each album is processed as a separate batch within the queue.

By default, GAMDL downloads the artist's full catalog. You can narrow the scope using the **Artist Auto-Select** setting in Settings > Quality, which lets you choose specific content types (Main Albums, Singles & EPs, Music Videos, etc.). When multiple content types are selected, MeedyaDL creates a separate queue item for each type. For example, selecting "Main Albums" and "Singles & EPs" creates two queue entries — one downloading main albums and one downloading singles — so each is processed independently.

**Example URL format:** `https://music.apple.com/us/artist/artist-name/1234567890`

### Library URLs

MeedyaDL also accepts personal library URLs from Apple Music. These are URLs that point to content in your own iCloud Music Library, using the path format `music.apple.com/library/...`. Library URLs work the same way as catalog URLs -- paste them into the download form and MeedyaDL will process them using your authenticated session. This is useful for downloading content that you have added to your personal library, including items that may have been removed from the public catalog but remain in your collection.

**Example URL format:** `https://music.apple.com/library/albums/l.1234567890`

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

Downloads are added to a queue when you submit a URL. By default, the queue begins processing immediately after each submission (**Auto-Start Downloads** is enabled in Settings > General). If you prefer to batch-add multiple URLs before starting, disable auto-start -- items will remain in the "Queued" state until you click the **Start Queue** button in the Queue page. The concurrency limit is also configurable in Settings if you want multiple simultaneous downloads.

Each item in the queue displays:

- **Progress bar** -- real-time download progress for the active item
- **Status** -- the current stage of processing (fetching metadata, downloading, tagging, complete, or failed)
- **Fallback indicator** -- shown if the requested codec was unavailable and the app automatically switched to a different quality

The following queue actions are available:

- **Cancel** -- stops the active download immediately and marks it as cancelled
- **Retry** -- re-queues a failed download so it can be attempted again. When a partial download exists on disk, MeedyaDL reads the album's `manifest.meedyadl` and re-runs only the tracks that actually failed (smart retry). If every expected track is already on disk, the retry is refused with a friendly message instead of pointlessly re-fetching
- **Retry without Wrapper** -- (only on items that used wrapper auth) re-runs with wrapper disabled, falling back to cookie-based auth
- **Retry All Failed** -- header button; re-queues every failed item in one click. Confirmation modal shows the count first
- **Right-click any row** -- opens a context menu with Copy Source Link, Open Folder (when output exists), Retry (when failed), and Retry without Wrapper (when applicable)
- **Clear Finished** -- removes all completed and failed items from the queue list, keeping only pending and active items
- **Export** -- saves the current queue to a `.meedyadl` file (JSON-based) that can be imported on another device or MeedyaDL instance. Only shown when there are active or pending items in the queue
- **Import** -- loads a previously exported `.meedyadl` queue file and adds the items to the current queue. The imported items use the current device's global settings as the base, with any per-download overrides from the export preserved

The History page exposes the same Retry / Retry All Failed actions for entries already moved out of the queue. Re-enqueuing from History creates a fresh queue item; the original History entry is preserved.

### Queue Persistence and Crash Recovery

MeedyaDL automatically saves the download queue to disk after every state change. If the app is closed or crashes while downloads are queued or in progress, those items are automatically restored and resumed on the next launch. Failed downloads are also preserved so you can review the error and retry them later.

**How it works:**

- The queue state is saved to a `queue.json` file in the app's data directory after every mutation (enqueue, cancel, retry, clear, completion, error, or fallback)
- Active items (queued, downloading, or processing) and failed items are persisted. Only completed and cancelled items are cleared on restart
- Failed items are restored in their error state with the original error message visible, so you can review what went wrong and retry when ready -- they are not automatically retried
- When the app launches and finds a saved queue, it restores the items and automatically begins processing queued items after a short delay (to allow the UI to initialize), regardless of the auto-start setting
- No manual action is required for active items -- recovery is fully automatic. Failed items persist until you manually retry or clear them

### Queue Export and Import

You can transfer your download queue between devices or MeedyaDL installations using the export/import feature.

**Exporting:**

1. Click the **Export** button in the queue header (shown when there are active or pending items)
2. Choose a save location in the native file dialog -- the default filename is `queue.meedyadl`
3. The exported file contains the URLs and any per-download quality overrides, but not your global settings

**Importing:**

1. Click the **Import** button in the queue header
2. Select a `.meedyadl` file from the native file picker
3. The imported items are added to your current queue and begin processing immediately (if auto-start is enabled) or remain queued until you click **Start Queue**
4. Each imported item uses your device's global settings as the base, with any per-download overrides from the export applied on top

This is useful for transferring download lists between a desktop and laptop, sharing playlists with others, or backing up a download queue before reinstalling the app.

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

#### Track and Disc Number Padding

The `{track}` and `{disc}` placeholders in your file template are zero-padded according to two settings at the bottom of the **Settings > Templates** page:

- **Track Number Padding (default: Auto)** — Auto sizes the padding to the album's track count: 2-digit for albums with up to 99 tracks (`01`, `02`, ..., `99`), 3-digit for box sets with 100–999 tracks (`001`, `002`, ..., `100`, ...), 4-digit for >999. Fixed widths (None / 2 / 3 / 4) are also offered if you want library-wide consistency regardless of album size. The default fixes a long-standing sort-order bug where a 100-track album under 2-digit padding produced `1 Track.m4a`, `10 Track.m4a`, ..., `2 Track.m4a` (alphabetical sort by leading character).
- **Disc Number Padding (default: Auto)** — Auto stays single-digit for the typical 1–9 disc case and switches to 2-digit for box sets with 10+ discs.

Existing libraries are not retroactively renamed when you change padding — the new setting only affects future downloads.

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

By default, the **Embed Lyrics and Keep Sidecar** option is enabled. This ensures lyrics are both embedded in the audio file's metadata tags and saved as a separate sidecar file (e.g., `.lrc`), providing maximum compatibility across different media players.

For full details on metadata and lyrics configuration, see [Lyrics and Metadata](lyrics-and-metadata.md).

---

## Fallback Quality

If the codec you selected is not available for a particular track, MeedyaDL automatically tries alternative codecs using a fallback chain. The default music fallback chain is:

ALAC -> Atmos -> AC3 -> AAC Binaural -> AAC -> AAC Legacy

When a fallback occurs, the queue item displays a fallback indicator so you know the final codec differs from your original selection. For full details on configuring fallback behavior, see [Fallback Quality](fallback-quality.md).

### Companion Downloads

MeedyaDL can automatically download additional format versions alongside your primary download. The **Companion Downloads** dropdown in Settings > Quality controls the behavior. By default (**Atmos → Lossless**), downloading Dolby Atmos content also downloads an ALAC (lossless) companion. Other preset modes offer additional tiers, such as downloading both ALAC and lossy AAC companions for Atmos, or downloading a lossy AAC companion alongside ALAC. The **Custom...** mode lets you pick exactly which codecs to download as companions using multi-select checkboxes. Specialist files are saved with format suffixes -- ALAC files get `[Lossless]` and Atmos files get `[Dolby Atmos]` -- while the most compatible companion uses a clean filename. Companion downloads run in the background without blocking the queue. See [Quality Settings](quality-settings.md#companion-downloads) for full mode descriptions.

Companion downloads include lyric sidecar files for every companion tier — each format version gets its own `.lrc`, `.srt`, `.vtt`, and `.ass` files (depending on your lyrics settings). You can track companion download progress in the **Activity Log**, which shows per-tier codec details, per-codec attempts, and completion status.

---

## Download Manifests (.meedyadl Files)

After each album download completes, MeedyaDL saves a `.meedyadl` manifest file in the album's output folder. This manifest records the source URLs and per-track metadata for the download, providing a convenient way to re-download the same content later.

### What the Manifest Contains

The `.meedyadl` manifest is a JSON file that stores:

- The original Apple Music URL used for the download
- Per-track metadata (title, artist, codec, quality settings)
- Any per-download overrides that were active at the time

Because manifests capture the exact parameters of the original download, re-importing one reproduces the same result without needing to look up the URL or reconfigure settings.

### Re-Downloading from a Manifest

There are three ways to re-download content from a `.meedyadl` manifest:

1. **Import button on the Download page** -- Click the **Import** button on the Download page and select a `.meedyadl` file from the native file picker. The items are added to the download queue using the manifest's stored URLs and your current device settings.

2. **Drag and drop** -- Drag a `.meedyadl` file from your file manager and drop it on the MeedyaDL application window. The app detects the manifest, parses its contents, and adds the items to the queue automatically.

3. **Queue Import** -- The **Import** button in the Queue page header also accepts `.meedyadl` files exported via the Queue Export feature.

In all cases, the imported items use your current global settings as the base, with any per-download overrides from the manifest applied on top.

### Manifest File Location

Manifests are saved alongside the downloaded tracks in the album folder:

```text
Output Directory/
  Artist Name/
    Album Name/
      01 Track Title.m4a
      02 Track Title.m4a
      Album Name.meedyadl   <- manifest file
```

---

## Smart Re-Download Detection

When you re-download an album you have previously downloaded, MeedyaDL checks the Apple Music API to determine whether the album has changed since your last download. If the album was previously downloaded, an info toast is shown with the date of the original download so you can decide whether to proceed.

This feature is useful for keeping your library up to date without manually tracking changes. It is enabled by default and can be toggled in **Settings > General > Preferences**.

### What Changes Are Detected

Smart re-download detection compares the current Apple Music catalog metadata against what was recorded at the time of your original download. It can detect:

- **Audio quality upgrades** -- such as an album gaining Dolby Atmos or Apple Lossless availability after initial release
- **Tracks added** -- bonus tracks, deluxe edition expansions, or previously missing tracks restored to the catalog
- **Metadata corrections** -- updated artist credits, corrected track titles, rewritten album descriptions, or genre reclassification
- **Apple Digital Master certification** -- an album receiving the Apple Digital Master designation after your original download

### Limitations

Smart re-download detection relies on metadata changes exposed through the Apple Music API. It **cannot** detect silent audio remasters where the underlying audio files have been replaced but the catalog metadata remains unchanged. In those cases, re-downloading manually is the only way to obtain the updated audio.

---

## Clipboard Monitoring

MeedyaDL can watch your system clipboard for supported URLs while the app is open. When you copy an Apple Music URL from a browser, messaging app, or any other source, MeedyaDL detects it and shows a notification offering to download that content.

Click **Download** on the notification to add the URL directly to the download queue (using your current quality settings). Dismiss the notification if you do not want to download.

When the MeedyaDL window is not focused (e.g., minimised or in the background), a **native OS notification** is sent instead of the in-app toast, so you never miss a detected URL. Native notifications respect the **Desktop Notifications** setting in **Settings > General**.

### Privacy

Clipboard monitoring only checks for URL patterns -- it never stores or logs clipboard contents. The check runs every 2 seconds and only triggers on Apple Music URLs (music.apple.com, classical.apple.com, itunes.apple.com). Non-URL clipboard content is immediately discarded.

### Configuration

Clipboard monitoring is enabled by default. To disable it, go to **Settings > General > Preferences** and toggle **Clipboard Monitoring** off.

The same URL will not trigger a second prompt within the same app session, even if it remains on the clipboard.

---

## Tips and Best Practices

- **Check cookie validity before large batch downloads.** If your authentication cookie has expired mid-way through a large playlist or artist download, all remaining tracks will fail with an auth error. Verify your cookie is current before starting. See [Cookie Management](cookie-management.md).
- **Use ALAC for archival, AAC for everyday listening.** ALAC provides lossless quality but produces larger files (typically 30--50 MB per track). AAC at 256 kbps is effectively transparent for most listeners and uses roughly 7--10 MB per track.
- **Albums download all tracks as a batch.** Submitting an album URL is more efficient than submitting individual song URLs, because metadata is fetched once for the whole album rather than per-track.
- **Monitor the fallback indicator.** If you see frequent fallbacks, the codec you selected may not be widely available. Consider switching your default codec in [Quality Settings](quality-settings.md).
- **Reduce concurrency if you encounter rate limits.** Downloading many items simultaneously can trigger Apple Music's rate limiting. Lowering the concurrency limit in Settings helps avoid this.
- **Your queue survives app restarts.** If you need to close the app while downloads are pending, they will automatically resume on the next launch. There is no need to manually save or re-enter URLs.
- **Use export/import to transfer queues between devices.** If you set up downloads on one machine and want to continue on another, export the queue to a `.meedyadl` file and import it on the other device. The imported items will use the destination device's quality settings.
- **Disable auto-start for batch queuing.** If you want to add multiple URLs before any downloads begin, turn off **Auto-Start Downloads** in Settings > General. Add all your URLs, then click **Start Queue** in the Queue page when ready.

---

## Related Topics

- [Quality Settings](quality-settings.md) -- Configure audio codec and quality preferences
- [Fallback Quality](fallback-quality.md) -- Understand automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) -- Configure lyric and metadata options
- [Downloading Videos](downloading-videos.md) -- Download music videos instead of audio
- [Troubleshooting](troubleshooting.md) -- Resolve common download errors

---

[Back to Help Index](index.md)
