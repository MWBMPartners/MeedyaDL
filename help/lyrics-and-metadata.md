<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Lyrics and Metadata

This page explains how MeedyaDL handles lyrics, cover art and the information stored inside your downloaded files (title, artist, album and so on).

---

## Overview

MeedyaDL can save song lyrics next to your music, put them inside the music files, and fill each file with tidy information. That way your player can show the right title, the cover and the words, in time with the song.

Most people never need to change anything. The defaults give you word-by-word lyrics where they exist, lyrics inside the file, and a saved cover picture. The rest of this page covers the choices you can make.

- Lyrics options live in **Settings > Lyrics**.
- Cover art options live in **Settings > Cover Art**.
- Extra tags (loudness, song fingerprints, links to other services) live in **Settings > Metadata**.

---

## Lyric Formats

You can save lyrics in several formats. A format is just the way the words and their timing are written down. Different players understand different formats.

| Format | Best for | Notes |
| --- | --- | --- |
| **LRC** | Music players | Works almost everywhere. Lyrics scroll line by line. |
| **Enhanced LRC** | Karaoke-style players | Highlights each word as it is sung. Still works as normal LRC in players that do not know about word timing. |
| **SRT** | Videos and subtitle tools | Works in nearly every video player. |
| **TTML** | Keeping the original | The full-detail original lyrics. Fewer players can open it. |
| **Lyricsfile (`.lyrics`)** | Lyrics editing tools | An open, plain-text format. Still experimental. |

If you are not sure, leave the defaults. Enhanced LRC is on by default, and it gives you word-by-word lyrics where they are available and ordinary line-by-line lyrics everywhere else.

### LRC

LRC is a plain text file. Each line of the song has a time next to it:

```
[00:12.34] First line of the song
[00:17.89] Second line of the song
```

- It works with players such as foobar2000, MusicBee and Poweramp.
- The file is saved next to the song with the same name, for example `Song Title.lrc`.

### Enhanced LRC (word-by-word)

Enhanced LRC adds a time before every word. Players that support it can highlight each word as it is sung, like a karaoke screen.

```
[00:12.45]<00:12.45>Midnight <00:13.20>rain <00:14.10>on <00:14.50>my <00:15.00>window
```

- Players that do not understand word timing simply show normal line-by-line lyrics.
- Background vocals are shown in brackets.
- Songs that only have line-by-line timing get ordinary LRC instead.
- It is on by default. Turn it on or off in **Settings > Lyrics > Enhanced Lyrics (Word-by-Word Sync)**.
- While it is on, TTML is kept as the main format, because the word timing comes from it. You can still tick LRC and SRT as extra formats.
- Word-by-word lyrics are looked up first by MeedyaDL itself (this needs your Apple Music developer details, see [Animated Artwork](animated-artwork.md), and your sign-in). If that is not possible for a song, and you are using GAMDL 4.0.2 or later (see [Trying GAMDL 4](tools.md#trying-gamdl-4)), the word-by-word file can come from GAMDL instead.
- The order is: word-by-word lyrics first, then the other lyric files made from them, and file renaming last.

**Test word-level lyrics connection.** This button, next to the toggle, checks whether word-by-word lyrics can be fetched for you right now, without waiting for a real download.

- Green means it works.
- Amber means lyrics came back, but only line by line.
- Anything else comes with a hint. Usually it asks you to sign in again (see [Cookie Management](cookie-management.md)) or to check your credentials in **Settings > Advanced**.

### SRT

SRT is a numbered list of subtitles, each with a start and an end time. It is the standard subtitle format for video.

- It works in VLC, MPV, IINA, Windows Media Player and many more.
- The file is saved next to the video, for example `Video Title.srt`. See [Downloading Videos](downloading-videos.md).
- When **Generate Rich SRT from TTML** is on (the default), SRT files include simple styling such as bold and italic, where the original lyrics have it.

### TTML

TTML is the original, most detailed lyrics format. It is the default format for music videos.

- It keeps the richest timing, including word timing.
- Few third-party players can open it. That is why MeedyaDL turns it into Enhanced LRC for you while that option is on.
- The file is saved next to the song, for example `Song Title.ttml`.

### Lyricsfile (`.lyrics`)

Lyricsfile is a newer, open, plain-text lyrics format. Lyrics tools such as LRCGET and LRCLIB can read it. You can open and edit it in any text editor, and it can hold word-by-word timing.

- It is **experimental**. The format may change, so MeedyaDL is cautious and never overwrites a `.lyrics` file that already exists. Any edits you make in LRCGET are safe.
- To turn it on: **Settings > Lyrics > Generate Lyricsfile (.lyrics)**. It is off by default.

<details>
<summary>More detail: what a Lyricsfile looks like</summary>

```yaml
version: "1.0"
metadata:
  title: Hello
  artist: Adele
  album: "25"
  duration_ms: 295000
  language: en
lines:
  - text: "Hello, it's me"
    start_ms: 1000
    end_ms: 3500
    words:
      - text: "Hello,"
        start_ms: 1000
        end_ms: 1800
```

The format version MeedyaDL writes is `1.0`, so you can tell which version of the format any file uses if it changes later.

</details>

---

## Choosing Lyrics Settings

### Turning lyrics on and off

Open **Settings > Lyrics**. By default MeedyaDL saves synced lyrics (lyrics with timing) for every track that has them. To stop saving lyrics, turn on **Disable Synced Lyrics**. That switch is greyed out while **Embed Lyrics / Captions** is on, because putting lyrics inside the file needs them to be downloaded first.

### Picking formats

Under **Synced Lyrics Formats** you can tick **LRC**, **SRT** and **TTML**.

- The first one you tick is the main format. It is saved together with the song.
- Every other one you tick is fetched afterwards, in a quick extra step that gets the lyrics only and does not download the song again.
- While Enhanced LRC is on, TTML is always the main format and cannot be unticked. Your other ticks become extras next to it.

For example, tick LRC and SRT, and you get the song, a `.lrc` file, and an `.srt` file, all with the same name apart from the ending.

### Lyrics Format Fallback

With **Lyrics Format Fallback** on (the default), MeedyaDL tries another format when the main one is missing for some tracks.

- Music tries TTML, then LRC, then SRT.
- Videos try TTML, then SRT, then LRC.
- It stops as soon as every track has lyrics.

### Putting lyrics inside the file, and saving separate files

- **Embed Lyrics / Captions** (on by default) puts the lyrics inside each music file. Most players read them from there.
- **Save Lyrics/Captions as Separate Files** (on by default, and shown only while Embed is on) also saves lyrics as their own files next to the music, for players that look for separate files. Turn it off to keep your folders tidy. The lyrics are still inside the files. Files that are already saved are never removed.
- **Synced Lyrics Only** downloads only the lyrics and not the song. Use it to add lyrics to music you already have.

### Extra subtitle files

These are all in **Settings > Lyrics** and are off unless noted:

- **Generate WebVTT Subtitles** makes `.vtt` files, used by web video players and media servers such as Plex and Jellyfin.
- **Generate Rich SRT from TTML** (on by default) makes SRT files with bold, italic and colours.
- **Generate ASS Subtitles** makes `.ass` files with colours and positions, used by VLC, mpv and MPC-HC.
- **Embed Subtitles in Media Files** stores SRT and WebVTT text inside MP4, M4A and M4V files.

### Which songs get lyric files

Lyric and subtitle files are made only for the songs and videos of the download that just ran, including any in disc folders. Older songs that happen to be in the same folder are not converted, embedded or changed.

### Where lyric files go

Lyric files are saved in the same folder as the song or video, with the same name and a different ending:

```
/Music/Artist - Song Title.m4a
/Music/Artist - Song Title.lrc
```

Most players find files named like this by themselves.

### Extra format versions of the same song

If you use **Companion Downloads** (see [Quality Settings](quality-settings.md#companion-downloads)), each extra version of a song gets its own lyric files, named to match, for example `01 Song Title [Lossless].lrc` next to `01 Song Title [Lossless].m4a`.

An extra version is never made over a file that is already there, and it is made only after the main download's lyrics and file names are finished. Each extra version gets the same lyrics as the main one, including word-by-word timing where the main download has it, and the same lyric and subtitle files you chose in **Settings > Lyrics** (the Lyricsfile and the lyrics stored inside the file included). If an extra version came with better lyrics than the main one, it keeps them, and the main version is given them too. Only lyric and subtitle files made during the same download are replaced this way; files that were already in your folder before are left alone.

<details>
<summary>More detail: will my edits to lyric files be overwritten?</summary>

Each time MeedyaDL finishes a download, or you download the same thing again, it makes the lyric and subtitle files again. Some replace an existing file. Others leave it alone.

| File type | If the file already exists |
| --- | --- |
| `.lrc` (Enhanced LRC) | Replaced |
| `.srt` (rich SRT) | Replaced, including a plain `.srt` |
| `.ttml` | Replaced when a more detailed word-by-word version is available |
| `.vtt` | Left alone |
| `.ass` | Left alone |
| `.lyrics` | Left alone |

If you edit a `.lrc`, `.srt` or `.ttml` file by hand, MeedyaDL may replace your edit the next time it runs on that folder. To keep your version, do one of these:

- Rename it, for example `Song Title.user.lrc`.
- Turn off the matching option in **Settings > Lyrics** before you download again.
- Copy it somewhere else first.

`.vtt`, `.ass` and `.lyrics` files are safe to edit in place.

</details>

---

## Metadata in Your Files

### What is stored

Every downloaded file is filled in with:

| Field | What it is |
| --- | --- |
| **Title** | The track or video title |
| **Artist** | The performing artist or artists |
| **Album Artist** | The main artist of the album |
| **Album** | The album name |
| **Track Number** | Where the track sits on the album |
| **Disc Number** | The disc, for albums with more than one |
| **Genre** | The genre |
| **Release Date / Year** | When it came out |
| **Album Artwork** | The cover picture, stored inside the file |
| **Composer** | The writer or composer |
| **Copyright** | The copyright notice |
| **Lyrics** | The lyrics, when **Embed Lyrics / Captions** is on |

Beyond these basics, MeedyaDL adds extra tags after each download. These say how good the file is, where it came from, and more. They never remove or change anything that is already there. The next section lists them.

For music files (M4A) and video files (MP4, M4V), the information is stored in the same way iTunes and Apple Music store it, so all of these players read it. FLAC files use the standard FLAC tag names.

<details>
<summary>More detail: how the information is stored in each file type</summary>

- **M4A, MP4 and M4V** files use iTunes-style tags (for example `©nam` for the title, `©ART` for the artist and `covr` for the picture).
- **FLAC** files use standard tag names such as `TITLE`, `ARTIST` and `ALBUM`, and the picture is stored inside the file.
- In every case the cover is stored inside the file, so your player does not need a separate image.

</details>

### What MeedyaDL adds after each download

After a download finishes, MeedyaDL works through a short list of extra jobs. Some are always on, and some you switch on.

1. **Quality and source tags** (always on). Marks lossless files, Dolby Atmos files, binaural and downmix versions, the speaker layout (for example 2.0 or 5.1), and where the file came from.
2. **Extra album and track details** (always on when your Apple Music developer credentials are set up in **Settings > Advanced > API Credentials**). Adds things such as the song's ISRC code, the album barcode, the record label, copyright notice, content advisory (explicit or clean) and release dates.
3. **Lyrics files** (on or off in **Settings > Lyrics**): Enhanced LRC, rich SRT, WebVTT, ASS and Lyricsfile, as described above.
4. **Cover art** (see below).
5. **Animated artwork** (optional, needs your Apple Music developer credentials). See [Animated Artwork](animated-artwork.md).
6. **Song fingerprint** (optional). See **AcoustID** below.
7. **Loudness tags** (optional). See **ReplayGain** below.
8. **Music video companions** (optional). Finds music videos for your songs. See [Downloading Videos](downloading-videos.md).
9. **Links on other music services** (optional). See below.

The full list of tags is in the [Metadata Mapping Reference](metadata-mapping.md).

<details>
<summary>More detail: the tags MeedyaDL writes</summary>

Tags that do not have a standard place are written as custom tags under two names: `com.apple.iTunes` (the name players already know) and `MeedyaMeta` (MeedyaDL's own name). Common names such as `LABEL`, `COPYRIGHT`, `COMPILATION` and `TOTALTRACKS` are also written, because tools such as MusicBrainz Picard, Mp3tag and foobar2000 look for them.

**Quality tags (always on)**

| Tag | Value |
| --- | --- |
| `com.apple.iTunes:isLossless` | `Y` for lossless (ALAC) |
| `SpatialType` | `Dolby Atmos` |
| `isBinaural` | `Y` for binaural versions |
| `isDownmix` | `Y` for stereo downmix versions |

**Source and layout tags (always on)**

| Tag | Value |
| --- | --- |
| `SourceStore` | `Apple Music` |
| `iTunesMediaType` | `Music` or `Music Video` |
| `isMedley` | `Y`, only if the title contains "Medley" |
| `ChannelConfig` | The speaker layout, such as `2.0`, `5.1` or `7.1` |

**Album and track tags (written when your Apple Music developer credentials are set up)**

| Tag | Value |
| --- | --- |
| `ISRC` | The song's recording code, for example `USRC12345678` |
| `UPC` and `Barcode` | The album barcode |
| `AlbumAdvisory` and `iTunesAdvisory` | `explicit` or `clean` |
| `AlbumArtistID`, `iTunesArtistID`, `iTunesCatalogID` | Apple's ID numbers |
| `AlbumGenre` and `Genre` | The genre or genres |
| `AppleAudioTraits` | For example `lossy-stereo, lossless, dolby-atmos` |
| `AppleDigitalMaster` | `true` or `false` |
| `ReleaseDate` and `AlbumReleaseDate` | For example `2022-10-21` |
| `Composer` | The writer's name |
| `DurationMs` | The length in milliseconds |
| `HasLyrics` | `true` or `false` |
| `TrackUrl` and `PreviewUrl` | The song's web address and its 30-second preview address |
| `LABEL` | The record label |
| `COPYRIGHT` | The copyright notice |
| `COMPILATION` | `true` or `false` |
| `AlbumIsSingle` and `AlbumIsComplete` | `true` or `false` |
| `AlbumMasteredForItunes` | `true` or `false` |
| `AlbumTrackCount` and `TOTALTRACKS` | The number of tracks on the album |
| `AlbumEditorialNote` | A short editorial summary |

Each of these is also written under the `MeedyaMeta` name with an `Apple` prefix, for example `MeedyaMeta:AppleRecordLabel`.

</details>

### AcoustID song fingerprint (optional)

Turn it on in **Settings > Metadata > Enable AcoustID Fingerprinting**. MeedyaDL listens to each song, works out a "fingerprint" of it, and looks it up on [acoustid.org](https://acoustid.org) to find the song's identity. The result is saved as tags. Tools such as MusicBrainz Picard can use those tags to recognise your music.

- You do not need to install anything. Release builds already include a key. You can use your own in **Settings > Advanced > API Credentials**.
- Each file is processed one at a time, so it adds a little time to every download.

### ReplayGain loudness (optional)

Turn it on in **Settings > Metadata > Enable ReplayGain Analysis**. MeedyaDL measures how loud each song is and saves that as tags. Players that understand them (foobar2000, VLC, Kodi, AIMP, Poweramp and others) use the tags to even out the volume. The music itself is never changed.

- **Track gain** evens out single songs. It suits shuffle.
- **Album gain** keeps the quiet and loud tracks of an album in balance. It suits listening to a whole album.

Options in the same section:

- **Reference Level** is the loudness every song is steered towards. The default (-18 LUFS) suits most players. -14 gives louder playback, similar to some streaming services. -23 is the quiet broadcast level. -16 matches Apple Music.
- **Prevent Clipping** (on by default) stops a loud song being turned up so far that it distorts.
- **Include Album Gain** (on by default) also writes the album-level values. Turn it off if you only ever shuffle.

<details>
<summary>More detail: which files support loudness tags</summary>

Loudness tags can be written to M4A, M4V, MP4, M4P, M4B, FLAC, OGG, OGA, Opus and MP3 files. Four tags are written: track gain, track peak, album gain and album peak (for example `-4.20 dB` and `0.933254`). Only the tags change. The audio itself is not touched.

</details>

### Content advisory in file names

**Settings > Metadata > Content Advisory in Filenames** adds `[Explicit]` or `[Clean]` to album folder names and track file names. It helps you tell the explicit and clean versions of the same album apart.

### Links on other music services (optional)

Turn on **Look up where else each album is available** in **Settings > Metadata > Links on Other Music Services**. After a download, MeedyaDL asks [song.link](https://song.link) where else the album can be found: Spotify, YouTube Music, Tidal, Deezer, Amazon Music and more. Each link found is saved in every track's tags, and in the album's `manifest.meedyadl` file.

- Only the album's Apple Music link is sent. Nothing else about you or your library goes with it.
- It needs an access key. song.link is run by a company called Odesli, and it stopped free access in 2026. Apply for a key at [Odesli's help pages](https://odesli.co/help), then paste it into **Settings > Advanced > API Credentials > song.link (Odesli)**. Without a key the switch does nothing, and the Activity page says so once.
- Answers are remembered for 30 days, so downloading the same album again does not ask again.

<details>
<summary>More detail: how the link tags are named</summary>

Each link is saved as a tag named `MeedyaMeta:<Service>Url`, for example `MeedyaMeta:SpotifyUrl`, `MeedyaMeta:TidalUrl` or `MeedyaMeta:DeezerUrl`. The name comes from whatever song.link calls the service, so a service that song.link adds later gets a tag with no update to MeedyaDL. The only cost is the spelling: YouTube Music becomes `YoutubeMusicUrl`.

</details>

### Finding music videos with MusicBrainz (optional)

Turn it on in **Settings > Codec & Resolution > Video Quality**. If MeedyaDL cannot find a music video for a song in the usual way, it can look the song up in the free, public MusicBrainz database. No credentials are needed.

- A search fallback, on by default and shown under the same switch, tries again using the artist and the title when the first look-up finds nothing. Matches are checked before they are used.
- MusicBrainz allows one request per second, so MeedyaDL spaces its requests out a little.

---

## Album Artwork

MeedyaDL saves album artwork at the largest size available. Settings are in **Settings > Cover Art**.

- **Save Cover Art** saves the picture as its own file in the album folder, for players and media servers (Plex, Jellyfin, Kodi) that look for one.
- **Cover Format** can be **Raw** (the original file, with no re-saving), **JPEG** (smaller files) or **PNG** (larger files, no quality loss).
- **Cover Size (pixels)** sets the width and height. You can choose anything from 100 to 10000.
- **Cover Art Filename** sets the file's name: **FrontCover** (the default), **Cover** or **Folder**.
- **Embed Music Video Cover Thumbnail** puts a music video's thumbnail inside the video file and removes the loose picture file.

The cover is stored inside each file as well, so your player can show it without a separate file.

### Checking other services for a bigger cover (optional)

Turn on **Upgrade Cover Art From Other Services**. After a download, MeedyaDL asks Deezer's public catalogue for its picture of the same album. It matches by the album's release barcode, never by name, so it cannot return another album's cover.

- If Deezer's picture has more pixels than the saved cover, the saved cover file is replaced. If it is the same size or smaller, nothing changes. MeedyaDL only counts pixels. It does not judge which picture looks better.
- Only the separate cover file is replaced. The picture inside each track is left as it is.
- It is off by default, because it sends the album's barcode to Deezer, a different company, for every album.

---

## Troubleshooting Lyrics and Metadata

### Lyrics are missing

Not every track has synced lyrics. If a track has none, nothing can be done in MeedyaDL. If many tracks are missing word-by-word lyrics, press **Test word-level lyrics connection** in **Settings > Lyrics**.

### The cover does not show in my player

- Check that your player supports covers stored inside the file type you use (M4A, FLAC, MP4, M4V).
- Refresh your library, or clear the player's cache.
- Check **Settings > Cover Art** and make sure the cover is being saved.
- Some older players cannot show very large pictures. Choose **JPEG** and a smaller **Cover Size**.

### Some fields are blank

Some players cannot read every kind of tag. Most modern players read the tags used for M4A, MP4, M4V and FLAC files, but a few small or old ones do not.

### Strange characters

MeedyaDL saves all text as UTF-8, which handles every language. If characters look wrong, check the text settings in your player. The files themselves are fine.

For anything else, see [Troubleshooting](troubleshooting.md).

---

## Related Topics

- [Downloading Music](downloading-music.md) — How audio downloads work
- [Downloading Videos](downloading-videos.md) — How video downloads and subtitles work
- [Quality Settings](quality-settings.md) — Audio and video options that affect what a file can hold
- [Metadata Mapping Reference](metadata-mapping.md) — Every tag MeedyaDL writes
- [Troubleshooting](troubleshooting.md) — General help with problems

---

[Back to Help Index](index.md)
