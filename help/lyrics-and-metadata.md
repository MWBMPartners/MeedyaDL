<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :pencil2: Lyrics and Metadata

This guide explains how MeedyaDL handles lyrics in various formats and how metadata is embedded into downloaded files.

---

## Overview

MeedyaDL can download synchronized lyrics alongside your music and embed rich metadata (title, artist, album art, etc.) directly into downloaded files. This ensures your media library is well-organized and your music player can display lyrics in real time.

---

## Lyric Formats

### LRC (LyRiCs)

LRC is a time-stamped text format and one of the most widely supported lyric formats. Each line in an LRC file pairs a timestamp with the corresponding lyric text:

```
[00:12.34] First line of the song
[00:17.89] Second line of the song
[01:23.45] Another line later in the track
```

**Key details:**

- LRC is one of the most widely supported lyric formats. It works with music players such as foobar2000, MusicBee, Poweramp, and Apple Music (via third-party plugins).
- When downloaded, LRC files are saved as sidecar files (e.g., `Song Title.lrc`) in the same directory as the corresponding audio file, sharing the same base filename.
- The format stores line-level synchronized timestamps, allowing your music player to scroll lyrics in time with playback.

### Enhanced LRC (Word-by-Word Sync)

Enhanced LRC extends the standard LRC format with inline word-level timestamps, enabling karaoke-style word-by-word highlighting. MeedyaDL is one of the first tools to generate Enhanced LRC from Apple Music's word-level timing data.

```
[ar:Artist Name]
[ti:Song Title]
[la:en]
[by:MeedyaDL]
[re:MeedyaDL Enhanced LRC]

[00:12.45]<00:12.45>Midnight <00:13.20>rain <00:14.10>on <00:14.50>my <00:15.00>window
[00:17.50]<00:17.50>Memories <00:18.20>fade <00:18.90>away
```

**Key details:**

- Each line has a standard `[mm:ss.xx]` line timestamp, plus `<mm:ss.xx>` word timestamps before each word.
- Standard LRC players ignore the `<...>` word timestamps and display lyrics as normal line-by-line sync — Enhanced LRC is fully backward-compatible.
- Compatible players (foobar2000 with ESLyric, Poweramp, AIMP, Musixmatch) highlight individual words as they are sung, similar to Apple Music Sing's karaoke feature.
- Background vocals are automatically wrapped in parentheses.
- A metadata header is included with artist, title, language, and tool information.

**How it works:**

1. GAMDL downloads the raw TTML lyrics file from Apple Music (TTML preserves Apple's word-level timing data in `<span>` elements).
2. MeedyaDL's `enhanced_lyrics_service` parses the TTML XML and extracts word-by-word timestamps.
3. The Enhanced LRC is saved as a `.lrc` sidecar file AND embedded in the audio file's metadata.
4. Songs without word-level timing in their TTML gracefully fall back to standard line-level LRC.

**Enabling Enhanced LRC:**

Enhanced LRC is enabled by default. The toggle is in **Settings > Lyrics > Enhanced Lyrics (Word-by-Word Sync)**. When enabled, TTML is automatically set as the primary lyrics download format (since the raw TTML is needed for conversion). You can still select LRC and SRT as **companion formats** — they will be downloaded alongside the primary TTML. This gives you Enhanced LRC (from TTML conversion) plus standard lyrics files in other formats for maximum compatibility.

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

TTML is an XML-based timed text format used natively by Apple Music. It is the default lyric format for music videos downloaded with MeedyaDL:

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

- TTML is the format Apple Music uses internally for its synchronized lyrics, so it preserves the richest timing data available from the source — including word-level timing for Apple Music Sing.
- TTML files are saved as sidecar files (e.g., `Song Title.ttml`) alongside the downloaded media file.
- Because TTML is an XML-based standard, it can carry more detailed timing and styling information than simpler text formats.
- TTML is the default primary lyrics format when Enhanced LRC is enabled, because it preserves the word-level timing data needed for Enhanced LRC conversion.
- TTML has more limited support among third-party music players compared to LRC or SRT. However, MeedyaDL automatically converts TTML to Enhanced LRC when the feature is enabled, so you get the best of both worlds: rich timing data from TTML with broad player compatibility from LRC.

---

## Configuring Lyric Downloads

### Enabling and Disabling Lyrics

Lyric downloads are controlled from **Settings > Lyrics**. In this tab you will find a toggle to enable or disable lyric downloads globally. When enabled, MeedyaDL will attempt to fetch synchronized lyrics for every track it downloads. When disabled, no lyric files will be created and no lyrics will be embedded.

### Choosing Lyric Formats

In **Settings > Lyrics**, you can select one or more lyric output formats using the checkboxes:

| Format | Best For | Notes |
|--------|----------|-------|
| **LRC** | Music / audio files | Widest music player support. |
| **Enhanced LRC** | Music with karaoke/word sync | Word-by-word highlighting. Backward-compatible with standard LRC. |
| **SRT** | Videos / subtitle workflows | Universal video player support. |
| **TTML** | Apple Music native workflows | Richest timing data. Default primary when Enhanced LRC is enabled. |

**Guidance:** Enable **Enhanced LRC** (default) for the best experience — you get word-by-word sync where available, with automatic fallback to line-level sync. SRT is automatically downloaded as a companion format. Choose **SRT** if you download music videos and want subtitles that work everywhere. Choose **TTML** if you need the original Apple Music lyric format for specialized processing.

### Multi-Format Lyrics

You can check multiple format boxes to download lyrics in more than one format. The first checked format (in LRC, SRT, TTML order) is the **primary** format, downloaded alongside the audio during the main download pass. Any additional checked formats are downloaded as lightweight **companion passes** after the audio completes -- these use GAMDL's `--synced-lyrics-only` mode, which fetches only the lyrics file without re-downloading audio.

### Companion Lyrics

When **companion downloads** are enabled in **Settings > Quality > Companion Downloads**, MeedyaDL automatically generates lyric sidecar files for every companion format version — not just the primary download. For example, if your primary download is Dolby Atmos with ALAC as a companion, both the Atmos and ALAC audio files will get their own `.lrc`, `.srt`, `.vtt`, and `.ass` sidecar files (depending on your lyric format settings). Each companion's lyric files use the same base filename as their corresponding audio file (e.g., `01 Song Title [Lossless].lrc` alongside `01 Song Title [Lossless].m4a`), making it easy to match lyrics to each format variant.

For example, if you check both LRC and SRT:

1. The primary download produces the audio file and a `.lrc` sidecar
2. After the audio completes, a companion pass downloads the `.srt` sidecar

All lyrics files land in the same directory alongside the audio, with the same base filename but different extensions.

### Lyrics Format Fallback

When the **Lyrics Format Fallback** toggle is enabled (default: on), MeedyaDL automatically retries with alternative formats if the primary lyrics format doesn't produce sidecar files for all tracks:

- **Audio** (`.m4a`): TTML → LRC → SRT
- **Video** (`.m4v`/`.mp4`): TTML → SRT → LRC

Each fallback attempt uses GAMDL's `--synced-lyrics-only` mode to download just the lyrics without re-downloading media. The chain stops as soon as all tracks have lyrics coverage.

This is particularly useful when Enhanced LRC is enabled — TTML is the primary format, but if TTML isn't available for some tracks, the fallback ensures those tracks still get lyrics in LRC or SRT format.

### Embed Lyrics and Keep Sidecar

The Lyrics tab also provides an **Embed Lyrics and Keep Sidecar** toggle. When enabled (which is the default), MeedyaDL ensures that lyrics are both embedded in the audio file's metadata tags AND saved as a separate sidecar file. This provides maximum compatibility: players that read embedded lyrics will find them in the file's metadata, while players that look for external lyrics files will find the sidecar file alongside the audio.

When the "Embed Lyrics and Keep Sidecar" toggle is enabled, the "Disable Synced Lyrics" option is overridden (greyed out), because the feature requires sidecar files to be created. If you want manual control over embedding and sidecar behavior independently, disable this toggle.

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

### Lyric Sidecar Regeneration

Every time enrichment runs on a download folder — the first download, any companion pass, and any retry or re-import via `.meedyadl` manifest — MeedyaDL's lyric/subtitle generators run again. Whether the regenerated file replaces or preserves an existing sidecar depends on which generator produced it:

| Generator              | Extension   | Behaviour on re-run                                                                                                   |
| ---------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------- |
| Enhanced LRC converter | `.lrc`      | **Overwrites** any existing file.                                                                                     |
| Rich SRT generator     | `.srt`      | **Overwrites** any existing file (including GAMDL's plain `.srt`; the rich variant with styling tags replaces it).    |
| Syllable-lyrics upgrade | `.ttml`    | **Overwrites** GAMDL's TTML when a word-level version is fetched from Apple Music's `/syllable-lyrics` endpoint.       |
| WebVTT generator       | `.vtt`      | **Skips** if the file already exists.                                                                                 |
| ASS generator          | `.ass`      | **Skips** if the file already exists.                                                                                 |

**Implication for users who edit sidecars manually:** `.lrc`, `.srt`, and upgraded `.ttml` files written by MeedyaDL are not treated as user data — any edits you make to them may be silently replaced on the next enrichment run (re-download, companion download, or manifest re-import). If you want to keep hand-edited lyrics or subtitles, either:

- Rename the file to a variant the generators don't touch (e.g., `Song Title.user.lrc`), OR
- Disable the corresponding generator in **Settings > Lyrics** before re-running enrichment, OR
- Copy the edited file elsewhere before triggering another enrichment pass.

`.vtt` and `.ass` sidecars are safe to edit in place — the generators detect the existing file and skip it.

This behaviour is intentional: the generators are idempotent converters whose inputs (TTML, SRT) are themselves refreshed from upstream, so overwriting is the correct default for the 95% case (first-time generation and upstream content updates). The asymmetry between `.lrc`/`.srt` (overwrite) and `.vtt`/`.ass` (skip) is historical; if you'd prefer a uniform guard, file an issue.

---

## Metadata Embedding

### What Metadata Is Embedded

MeedyaDL embeds the following metadata fields into every downloaded file:

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

### MeedyaDL Metadata Enrichment

In addition to the standard Apple Music metadata written by GAMDL, MeedyaDL runs a comprehensive 12-stage metadata enrichment pipeline after each download. This writes custom freeform atoms into M4A files to identify codec quality, source information, channel configuration, and optionally Enhanced LRC lyrics, subtitle files, audio fingerprints, and loudness data. All tags are non-destructive — existing metadata is never modified or removed.

Tag definitions are driven by `tags.toml` — a config file that maps Apple Music API JSON fields to MP4 freeform atoms. All tags are written in dual namespaces: `com.apple.iTunes` (player-compatible) and `MeedyaMeta` (MeedyaDL-branded). Industry-standard alternative names are used where recognised by tools like MusicBrainz Picard, Mp3tag, and foobar2000 (`LABEL`, `COPYRIGHT`, `COMPILATION`, `TOTALTRACKS`). The complete tag-by-tag reference — including standard MP4 atoms, Apple proprietary IDs, per-format support matrix, and API source mapping — lives in the [Metadata Mapping Reference](metadata-mapping.md).

The enrichment stages run in order:

1. **Codec/Source/Channel tags + Apple Music API metadata** (always-on) — 30+ freeform atoms per file. Uses ffprobe-based codec detection for accurate tagging when native priority mode is active (GAMDL >= 2.9.1)
2. **Enhanced LRC conversion** (opt-in, default on) — converts TTML to Enhanced LRC, saves `.lrc` sidecar, embeds in `©lyr` atom
2b. **Lyrics format fallback** (opt-in, default on) — if TTML didn't produce lyrics for all tracks, retries with LRC (audio) or SRT (video)
2c. **WebVTT subtitle generation** (opt-in) — converts TTML, SRT, or LRC sidecars to `.vtt` subtitle files
2d. **Rich SRT generation** (opt-in, default on) — converts TTML/WebVTT to SRT with styling tags (`<b>`, `<i>`, `<u>`, `<font color>`)
2e. **Subtitle embedding** (opt-in) — embeds SRT and WebVTT content as freeform atoms in MP4 containers
2f. **ASS subtitle generation** (opt-in) — converts TTML/WebVTT to Advanced SubStation Alpha with colours, positioning, and background vocal styles
3. **Animated artwork download** (requires MusicKit credentials)
4. **AcoustID fingerprinting** (opt-in) — also extracts MusicBrainz recording IDs from AcoustID responses
5. **ReplayGain analysis** (opt-in)
6. **Music video companion downloads** (opt-in, requires MusicKit credentials)
6b. **MusicBrainz video discovery** (opt-in) — fallback music video discovery when Step 6 finds no results

#### Codec Tags (Always-On)

| Codec | Tag (Namespace:Name) | Value |
| ----- | -------------------- | ----- |
| **ALAC (Lossless)** | `com.apple.iTunes:isLossless` | `Y` |
| **Dolby Atmos** | `com.apple.iTunes:SpatialType` / `MeedyaMeta:SpatialType` | `Dolby Atmos` |
| **Binaural** (AAC/AAC-HE) | `com.apple.iTunes:isBinaural` / `MeedyaMeta:isBinaural` | `Y` |
| **Downmix** (AAC/AAC-HE) | `com.apple.iTunes:isDownmix` / `MeedyaMeta:isDownmix` | `Y` |

#### Source and Channel Tags (Always-On)

| Tag (Namespace:Name) | Value | Source |
| -------------------- | ----- | ------ |
| `com.apple.iTunes:SourceStore` | `Apple Music` | Hardcoded |
| `MeedyaMeta:SourceStore` | `Apple Music` | Hardcoded |
| `com.apple.iTunes:EncodeSource` | `Web` | Hardcoded |
| `com.apple.iTunes:iTunesMediaType` | `Music` or `Music Video` | Download type |
| `com.apple.iTunes:isMedley` | `Y` | Only if title contains "Medley" |
| `com.apple.iTunes:ChannelConfig` | `1.0`, `2.0`, `5.1`, `7.1`, etc. | Detected via ffprobe |

#### Apple Music API Tags (Always-On When MusicKit Configured)

These tags are written automatically when MusicKit credentials are configured in **Settings > Advanced > API Credentials**. No separate toggle is needed.

| Tag (Namespace:Name) | Value | API Field |
| -------------------- | ----- | --------- |
| `com.apple.iTunes:ISRC` | e.g., `USRC12345678` | Track `attributes.isrc` |
| `com.apple.iTunes:UPC` | Barcode string | Album `attributes.upc` |
| `com.apple.iTunes:Barcode` | Same as UPC | Album `attributes.upc` |
| `com.apple.iTunes:AlbumAdvisory` | `explicit`, `clean` | Album `contentRating` |
| `com.apple.iTunes:AlbumArtistID` | Numeric string | Album artist `id` |
| `com.apple.iTunes:AlbumArtistSort` | Artist name | Album `artistName` |
| `com.apple.iTunes:AlbumGenre` | e.g., `Pop` | `genreNames[0]` |
| `com.apple.iTunes:iTunesAdvisory` | `explicit`, `clean` | Track `contentRating` |
| `com.apple.iTunes:iTunesArtistID` | Numeric string | Track artist `id` |
| `com.apple.iTunes:iTunesCatalogID` | Numeric string | Track `id` |
| `com.apple.iTunes:StoreID/AppleMusic` | Same as CatalogID | Track `id` |
| `MeedyaMeta:AppleAudioTraits` | `lossy-stereo, lossless, dolby-atmos` | Track `audioTraits` |
| `com.apple.iTunes:AppleDigitalMaster` | `true` / `false` | Track `isAppleDigitalMaster` |
| `com.apple.iTunes:ReleaseDate` | `2022-10-21` | Track `releaseDate` |
| `com.apple.iTunes:Composer` | Songwriter name | Track `composerName` |
| `com.apple.iTunes:DurationMs` | `202395` | Track `durationInMillis` |
| `com.apple.iTunes:HasLyrics` | `true` / `false` | Track `hasLyrics` |
| `com.apple.iTunes:PlayParamsId` | Catalog ID | Track `playParams.id` |
| `com.apple.iTunes:TrackUrl` | Canonical URL | Track `url` |
| `com.apple.iTunes:PreviewUrl` | 30s preview URL | Track `previews[0].url` |
| `com.apple.iTunes:Genre` | `Pop, Music` | Track `genreNames` |
| `com.apple.iTunes:LABEL` | Record label name | Album `recordLabel` |
| `com.apple.iTunes:COPYRIGHT` | Copyright notice | Album `copyright` |
| `com.apple.iTunes:AlbumReleaseDate` | `2022-10-21` | Album `releaseDate` |
| `com.apple.iTunes:COMPILATION` | `true` / `false` | Album `isCompilation` |
| `com.apple.iTunes:AlbumIsSingle` | `true` / `false` | Album `isSingle` |
| `com.apple.iTunes:AlbumIsComplete` | `true` / `false` | Album `isComplete` |
| `com.apple.iTunes:AlbumMasteredForItunes` | `true` / `false` | Album `isMasteredForItunes` |
| `com.apple.iTunes:AlbumTrackCount` | `13` | Album `trackCount` |
| `com.apple.iTunes:TOTALTRACKS` | Same as TrackCount | Album `trackCount` |
| `com.apple.iTunes:AlbumEditorialNote` | Editorial summary | Album `editorialNotes.short` |
| `com.apple.iTunes:MotionArtURL` | HLS M3U8 URL | Animated artwork (square) |
| `MeedyaMeta:MotionArtURL` | HLS M3U8 URL | Animated artwork (square) |
| `com.apple.iTunes:MotionArtPortraitURL` | HLS M3U8 URL | Animated artwork (portrait) |
| `MeedyaMeta:MotionArtPortraitURL` | HLS M3U8 URL | Animated artwork (portrait) |

All tags above are also written under the `MeedyaMeta` namespace with `Apple*` prefix (e.g., `MeedyaMeta:AppleRecordLabel`, `MeedyaMeta:AppleReleaseDate`). Tag definitions are driven by `tags.toml` — to add new metadata fields, edit the TOML file (see [DEV_NOTES.md](https://github.com/MWBMPartners/MeedyaDL/blob/main/DEV_NOTES.md#metadata-tag-registry-tagstoml) for the editing guide).

#### AcoustID Tags (Opt-In)

Enable in **Settings > Metadata**. Generates Chromaprint audio fingerprints using MeedyaDL's built-in fingerprinting engine and looks up AcoustID identifiers from [acoustid.org](https://acoustid.org). No external tools required. Release builds include a built-in API key, so no registration is needed. You can optionally override it with your own key in Settings > Metadata.

| Tag (Namespace:Name) | Value |
| -------------------- | ----- |
| `com.apple.iTunes:Acoustid Id` | UUID from acoustid.org |
| `com.apple.iTunes:Acoustid Fingerprint` | Raw Chromaprint fingerprint |

#### ReplayGain Tags (Opt-In)

Enable in **Settings > Metadata > ReplayGain Analysis**. Uses FFmpeg (already installed) to analyse audio loudness via the EBU R128 standard. Calculates both **per-track** and **per-album** gain so players can normalise in either mode (per-track for shuffle, per-album for album listening). Tags enable volume normalisation in compatible media players (foobar2000, Kodi, VLC, AIMP, Poweramp, etc.) without altering the audio data.

**Supported formats:** M4A, M4V, MP4, M4P, M4B (iTunes freeform atoms via mp4ameta), FLAC, OGG, OGA, Opus (Vorbis Comments via lofty), and MP3 (ID3v2 TXXX frames via lofty).

| Tag (Namespace:Name) | Scope | Value |
| -------------------- | ----- | ----- |
| `com.apple.iTunes:replaygain_track_gain` | Per-track | e.g., `-4.20 dB` |
| `com.apple.iTunes:replaygain_track_peak` | Per-track | e.g., `0.933254` (linear scale) |
| `com.apple.iTunes:replaygain_album_gain` | Per-album | e.g., `-3.10 dB` (average of all tracks) |
| `com.apple.iTunes:replaygain_album_peak` | Per-album | e.g., `0.987654` (highest peak in album) |

**Configuration options** (Settings > Metadata > ReplayGain Analysis):

- **Reference Level** — target loudness. Options: -18 LUFS (EBU R128, default), -14 LUFS (Spotify/YouTube), -23 LUFS (broadcast), -16 LUFS (Apple Music/iTunes)
- **Prevent Clipping** — limits gain so peak × gain never exceeds 0 dBFS. Enabled by default. Prevents digital distortion on tracks mastered near maximum loudness
- **Include Album Gain** — when enabled (default), computes and writes album-level tags (`replaygain_album_gain`, `replaygain_album_peak`) alongside track tags. Album gain preserves the intended dynamic range when listening to a full album in order. When disabled, only per-track tags are written (better for shuffle-only listening)

**Technical details:**

- All tags are stored as MP4 freeform atoms (the `----` box type), the standard mechanism for custom metadata in the iTunes/M4A ecosystem.
- The `com.apple.iTunes` namespace follows the same convention used by Apple and third-party tools like iTunes, MusicBrainz Picard, and Mp3tag.
- The `MeedyaMeta` namespace is a MeedyaDL-branded namespace, ensuring these tags are clearly identifiable and don't collide with any future Apple-defined atoms.
- Only the M4A container metadata is modified — the audio stream data (ALAC, EC-3, AAC) is never touched.
- Enrichment runs as a background task and never blocks the download queue.

#### WebVTT Subtitle Generation (Opt-In)

Enable in **Settings > Lyrics**. Generates WebVTT (`.vtt`) subtitle files from existing lyrics sidecars after download. WebVTT is the standard subtitle format for HTML5 video players, media servers (Plex, Jellyfin), and web-based playback.

Source files are used in priority order:

1. **TTML** (best) — preserves word-level timing from Apple Music when available
2. **SRT** (good) — has start and end timestamps for each line
3. **LRC** (fallback) — has start times only; end times are estimated at 3 seconds per line

The generated `.vtt` file is saved alongside the downloaded media. Only one source is used per track (the highest-priority format found).

#### MusicBrainz Video Discovery (Opt-In)

Enable in **Settings > Quality > Video Quality**. When the Apple Music API doesn't find music videos for your downloaded tracks (Step 6), MusicBrainz provides a fallback discovery mechanism. No Apple Developer credentials or MusicKit configuration required — the MusicBrainz API is free and public.

MusicBrainz discovery uses a 3-tier priority chain for maximum coverage:

1. **Apple Music URL search** — searches MusicBrainz external links for the exact Apple Music track URL (highest fidelity)
2. **ISRC code search** — uses the ISRC identifier from Apple Music metadata (standard recording identifier)
3. **AcoustID recording ID lookup** — uses the MusicBrainz recording ID extracted during AcoustID fingerprinting (Step 4, if enabled)

Each tier is tried in order; the first successful match is used. Discovered music video URLs (Apple Music, YouTube) trigger companion downloads. The service also stores cross-platform URLs (Spotify, Deezer, Tidal, SoundCloud, Bandcamp) as groundwork for future multi-service song discovery.

**Rate limiting:** MusicBrainz enforces 1 request per second. MeedyaDL respects this with a 1.1-second delay between requests.

---

## Album Artwork

### Artwork Resolution

MeedyaDL downloads album artwork at the full resolution available from Apple Music, which can be up to **3000x3000 pixels**. This ensures your library has the highest quality cover art possible.

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

Not all tracks on Apple Music have synchronized lyrics. If MeedyaDL does not download a lyric file for a particular track, it is most likely because lyrics are not available for that track in Apple Music's catalog. There is no workaround for this within MeedyaDL.

### Album Artwork Not Displaying

If embedded artwork is not showing in your media player:

- Verify that your media player supports embedded artwork for the file format you are using (M4A, FLAC, MP4, M4V).
- Try refreshing your media library or clearing your player's metadata cache.
- Check **Settings > Cover Art** to confirm that artwork embedding is enabled.
- Some older players may not support high-resolution artwork. If artwork fails to display, try using JPG format which produces smaller embedded images.

### Metadata Fields Appearing Blank

If metadata fields appear empty in your media player, ensure that the player supports the tagging standard used by your file format (MP4 atoms for M4A/MP4/M4V, Vorbis comments for FLAC). Most modern players handle both standards, but some niche or legacy players may not read all fields.

### Encoding Issues with Special Characters

MeedyaDL handles UTF-8 encoding automatically for all lyrics and metadata. If you see garbled or incorrect characters, the issue is likely with your media player's text encoding settings rather than with the downloaded files. Check that your player is configured to use UTF-8 encoding for metadata display.

For general troubleshooting, see [Troubleshooting](troubleshooting.md).

---

## Related Topics

- [Downloading Music](downloading-music.md) -- How audio downloads work
- [Downloading Videos](downloading-videos.md) -- How video downloads and subtitles work
- [Quality Settings](quality-settings.md) -- Audio and video format options that affect metadata capabilities
- [Troubleshooting](troubleshooting.md) -- General error resolution

---

[Back to Help Index](index.md)
