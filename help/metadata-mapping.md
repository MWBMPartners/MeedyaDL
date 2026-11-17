<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Metadata Mapping Reference

This page lists the information MeedyaDL saves inside your downloaded music files. It helps if you use a tag editor or music library program (such as MusicBrainz Picard, beets, Mp3tag or foobar2000), or if you write your own scripts to read the files.

If you only want to listen to your music, you do not need this page. Your files already show the usual title, artist, album and cover art in every player. See [Lyrics and Metadata](lyrics-and-metadata.md) for the everyday overview.

---

## Table of Contents

1. [How metadata gets written](#how-metadata-gets-written)
2. [Tag namespaces](#tag-namespaces)
3. [How to inspect tags on disk](#how-to-inspect-tags-on-disk)
4. [Standard M4A/MP4 atoms](#standard-m4amp4-atoms)
5. [Apple ID atoms](#apple-id-atoms)
6. [iTunes freeform atoms](#itunes-freeform-atoms)
7. [MeedyaMeta freeform atoms](#meedyameta-freeform-atoms)
8. [Per-format support](#per-format-support)
9. [Where the information comes from](#where-the-information-comes-from)
10. [Industry-standard alternative names](#industry-standard-alternative-names)

---

## How metadata gets written

Each song is tagged in two stages:

1. **During the download**, the standard tags are written: title, artist, album, track number, cover art, lyrics and so on.
2. **After the download**, MeedyaDL adds extra detail from Apple Music, such as the record label, barcode, release date, composer, ISRC code, whether a track is Apple Digital Master, and more. It also adds details it works out itself, such as the surround layout or loudness.

MeedyaDL only **adds or updates** tags. It never wipes out the tags a file already has.

If you use **Companion Downloads** for extra formats, each extra copy gets the details of its own format. Those details are written only to the files that extra download added, never to your main files or to older files in the same folder.

A few tags depend on the file itself:

- **Codec-specific:** a lossless flag (ALAC only) and a spatial-audio type (Atmos only).
- **Always added:** where the file came from, how it was encoded, the media type and whether a track is a medley.
- **Surround layout:** worked out from the audio file itself.

---

## Tag namespaces

Every extra ("freeform") tag MeedyaDL writes lives in one of two groups:

| Group | Full name | What it is for |
|---|---|---|
| `itunes` | `com.apple.iTunes` | **Works with other programs.** iTunes, Music, and most other players and tag editors understand it. Use it for anything other software should read. |
| `meedya` | `MeedyaMeta` | **Marked as MeedyaDL's.** It can never clash with iTunes or other tools. Use it when you want to know the data came from MeedyaDL, for example to filter a mixed library. |

Most fields are written to **both** groups, so other programs can read them and MeedyaDL's own data stays easy to tell apart.

### Industry-standard names

Some fields also use the common names that many tools expect:

| MeedyaDL field | Standard tag | Understood by |
|---|---|---|
| Record label | `LABEL` | Picard, Mp3tag, foobar2000, beets |
| Copyright | `COPYRIGHT` | Mp3tag, foobar2000 |
| Barcode | `UPC` and `Barcode` | Picard, beets |
| Compilation | `COMPILATION` | Picard, Mp3tag, beets |
| Track count | `TOTALTRACKS` (next to `AlbumTrackCount`) | Picard, foobar2000 |

---

## How to inspect tags on disk

### Quick check (any system)

```bash
ffprobe -loglevel quiet -show_format -show_streams "song.m4a" | grep -i "tag\|metadata"
```

### Full dump of the extra tags

```bash
# Needs mp4info (part of mp4v2 / gpac)
mp4info "song.m4a"

# Or AtomicParsley (macOS / Linux)
AtomicParsley "song.m4a" -T
```

### With a tag editor

Open the file in MusicBrainz Picard, beets or Mp3tag and look at the full tag list. Standard tags show under their usual names. The iTunes group shows as `com.apple.iTunes`. MeedyaDL's own tags show under `MeedyaMeta`.

### With a script (Python)

```python
from mutagen.mp4 import MP4

f = MP4("song.m4a")
for key, value in f.tags.items():
    print(f"{key!r}: {value!r}")
```

Look for keys that begin with `----:com.apple.iTunes:` (iTunes tags) and `----:MeedyaMeta:` (MeedyaDL tags).

---

## Standard M4A/MP4 atoms

These are the ordinary tags every player knows. They are written during the download, and MeedyaDL leaves them alone.

<details>
<summary>More detail: the standard tag list</summary>

| Atom | Name | Type | Notes |
|---|---|---|---|
| `©nam` | Title | text | Track title |
| `©ART` | Artist | text | Track artist |
| `aART` | Album Artist | text | Album-level artist |
| `©alb` | Album | text | Album title |
| `©gen` | Genre (custom) | text | Free-text genre |
| `gnre` | Genre (ID3v1) | number | Old-style numeric genre code |
| `©day` | Year/Date | text | Release year or full date |
| `©wrt` | Composer | text | Composer name |
| `trkn` | Track number | number pair | (this track, total tracks) |
| `disk` | Disc number | number pair | (this disc, total discs) |
| `covr` | Cover artwork | image | Embedded JPEG or PNG |
| `tmpo` | BPM | number | Beats per minute (optional) |
| `cpil` | Compilation flag | yes/no | Marks a compilation |
| `©lyr` | Lyrics | text | Embedded lyrics (Enhanced LRC when available) |
| `pgap` | Gapless | yes/no | Gapless-playback hint |
| `rtng` | Content rating | number | 0 = none, 2 = clean, 4 = explicit |
| `stik` | Media kind | number | 1 = movie, 6 = music video, and so on |

</details>

---

## Apple ID atoms

These are Apple's own number codes, saved in each file. They let you match a file to its entry in Apple Music or the iTunes Store.

<details>
<summary>More detail: the Apple ID atoms</summary>

| Atom | Name | Example | Notes |
|---|---|---|---|
| `cnID` | Song ID | `1649434005` | The track's Apple Music number |
| `cmID` | Composer ID | `429153007` | The composer's number |
| `atID` | Artist ID | `159260351` | The artist's number. Use it in `https://music.apple.com/{storefront}/artist/{atID}` for the artist's page. |
| `plID` | Album ID | `6750434` | The album's number |
| `geID` | Genre ID | `21` (Rock) | Apple's genre number |
| `sfID` | Storefront ID | `143444` (UK) | The number of the store country the track came from |
| `akID` | Account kind | `1` | Kind of audio |

The same numbers are also saved inside `MeedyaMeta:*` with friendly names (for example `MeedyaMeta:ArtistID` and `MeedyaMeta:AlbumID`), for tools that do not read Apple's four-letter codes.

</details>

---

## iTunes freeform atoms

These are written under `com.apple.iTunes:`. They are **readable by other programs**, including iTunes, Music, Picard, Mp3tag, foobar2000 and beets.

<details>
<summary>More detail: album-level iTunes tags</summary>

The same value is written on every track of the album.

| Atom | What it holds | Type |
|---|---|---|
| `AlbumAdvisory` | Album content rating (explicit or clean) | text |
| `AlbumArtistID` | The album artist's Apple number | text |
| `AlbumArtistSort` | Album artist name | text |
| `AlbumGenre` | The album's main genre | text |
| `UPC` and `Barcode` | The album's barcode | text |
| `LABEL` | Record label | text |
| `COPYRIGHT` | Copyright line | text |
| `AlbumReleaseDate` | Release date | text |
| `COMPILATION` | Whether it is a compilation | yes/no |
| `AlbumIsSingle` | Whether it is a single | yes/no |
| `AlbumIsComplete` | Whether the album is complete | yes/no |
| `AlbumMasteredForItunes` | Whether it was Mastered for iTunes | yes/no |
| `AlbumTrackCount` and `TOTALTRACKS` | Number of tracks | number |
| `AlbumEditorialNote` | Apple's short write-up of the album | text |
| `MotionArtURL` | Link to the square animated cover | text |
| `MotionArtPortraitURL` | Link to the portrait animated cover | text |

</details>

<details>
<summary>More detail: track-level iTunes tags</summary>

Each track gets its own values. They are matched to the right file by track and disc number.

| Atom | What it holds | Type |
|---|---|---|
| `ISRC` | The track's ISRC code | text |
| `iTunesAdvisory` | Track content rating (explicit or clean) | text |
| `iTunesArtistID` | The artist's Apple number | text |
| `iTunesCatalogID` and `StoreID/AppleMusic` | The track's Apple Music number | text |
| `AppleDigitalMaster` | Whether it is an Apple Digital Master | yes/no |
| `ReleaseDate` | Release date | text |
| `Composer` | Composer name | text |
| `DurationMs` | Length in milliseconds | number |
| `HasLyrics` | Whether lyrics exist | yes/no |
| `PlayParamsId` | Apple's play number for the track | text |
| `TrackUrl` | Link to the track on Apple Music | text |
| `PreviewUrl` | Link to the 30-second preview | text |
| `Genre` | Genres | list |

</details>

---

## MeedyaMeta freeform atoms

This is MeedyaDL's own group. **Every** album-level and track-level tag above is also written under `MeedyaMeta:` with a friendlier name (for example `MeedyaMeta:AppleRecordLabel`, `MeedyaMeta:AppleUPC` and `MeedyaMeta:AppleAudioTraits`).

<details>
<summary>More detail: tags that only exist under MeedyaMeta</summary>

| Atom | What it records |
|---|---|
| `MeedyaMeta:AppleAudioTraits` | The track's audio traits, for example `["atmos","lossless","lossy-stereo"]` |
| `MeedyaMeta:SourceStore` | Where the file came from. Always "Apple Music". (Also written as `com.apple.iTunes:SourceStore`.) |
| `MeedyaMeta:LastModified` | The date the service last updated this release, as it stood when the file was downloaded, in UTC (for example `2026-11-17T04:12:35Z`). Written into the music files MeedyaDL downloads, not into older files already in the folder. MeedyaDL's own update check uses the album's download record, not this tag. |
| `MeedyaMeta:SpatialAudioCodec` | The Atmos or spatial format, worked out from the audio file |
| `MeedyaMeta:<Service>Url` (for example `MeedyaMeta:SpotifyUrl`, `MeedyaMeta:TidalUrl`, `MeedyaMeta:YoutubeMusicUrl`) | Where the album can be found on another music service, one tag for each service that has a match. Written only when you turn on the song.link links option in **Settings > Metadata** and have an access key. |

</details>

<details>
<summary>More detail: tags that only exist under iTunes</summary>

A few tags are written only as `com.apple.iTunes:*`, with no MeedyaMeta copy:

| Atom | What it records |
|---|---|
| `com.apple.iTunes:EncodeSource` | Always "Web" |
| `com.apple.iTunes:ChannelConfig` | The speaker layout, for example `5.1.4` or `7.1` |
| `com.apple.iTunes:replaygain_track_gain`, `replaygain_track_peak`, `replaygain_album_gain`, `replaygain_album_peak` | Loudness values. Only when ReplayGain is turned on. |
| `com.apple.iTunes:Acoustid Fingerprint` and `Acoustid Id` | A sound fingerprint and the matching AcoustID recording ID. Only when AcoustID is turned on. |

</details>

---

## Per-format support

MeedyaDL downloads are M4A (music) and M4V or MP4 (music videos). The table shows how well each tag type is supported if your files are converted to another format.

<details>
<summary>More detail: support by file format</summary>

| Kind of tag | M4A / M4V | FLAC | MP3 | OGG / Opus |
|---|---|---|---|---|
| Standard tags (title, artist, album and so on) | Yes | Yes | Yes | Yes |
| Cover artwork | Yes | Yes | Yes | Yes |
| iTunes freeform tags | Yes | Partly | Partly | Partly |
| `MeedyaMeta:*` tags | Yes | Yes | Yes | Yes |
| ReplayGain | Yes | Yes | Yes | Yes |
| BPM | Not written yet | Not written yet | Not written yet | Not written yet |
| Enhanced LRC lyrics | Yes | Text only | Text only | Text only |

"Text only" means the lyrics are kept but most players will not show the word-by-word timing.

</details>

---

## Where the information comes from

| Source | What it gives you |
|---|---|
| **The download itself** | The standard tags, Apple's number codes and the cover art |
| **Apple's public iTunes information** | Country, number of discs and the iTunes link. Collected first. |
| **Apple Music catalogue** | The richer detail: editorial notes, audio traits, content rating, Apple Digital Master flag, label, copyright, release date, composer, length, lyrics flag, previews and genres. Where both sources have a field, this one wins. It needs your MusicKit details for the extras (see [Animated Artwork](animated-artwork.md)). |
| **MusicBrainz** (optional) | Another way to find the music videos for your songs, used by **Music Video Companions**. On its own it adds nothing to your files. Turn it on in **Settings > Codec & Resolution > Video Quality**. |
| **song.link** (optional) | Links to the album on other services. Turn it on in **Settings > Metadata**. It needs an access key, which you apply for through [Odesli's help pages](https://odesli.co/help). Free public access closed in 2026. |
| **AcoustID** (optional) | A sound fingerprint and recording ID. Turn it on in **Settings > Metadata**. |
| **Your own computer** | The surround layout, spatial format and loudness, worked out from the audio file |

---

## Industry-standard alternative names

To work with as many programs as possible, MeedyaDL writes some fields under both an Apple-style name and a standard name. If you write scripts, prefer the standard name. It works in more formats.

| Apple-style | Standard name | Understood by |
|---|---|---|
| `iTunesCatalogID` | `StoreID/AppleMusic` | The beets `apple_music` plugin |
| `AlbumArtist` | (the built-in `aART`) | All MP4 readers |
| `LABEL` | (same) | Picard, Mp3tag, foobar2000, beets |
| `COPYRIGHT` | (same) | Mp3tag, foobar2000 |
| `UPC` and `Barcode` | (same) | Picard, beets |
| `COMPILATION` | (same) | Picard, Mp3tag, beets |
| `AlbumTrackCount` | `TOTALTRACKS` | Picard, foobar2000 |
| `ISRC` | (same) | Picard, Mp3tag, foobar2000, beets |

---

## See also

- [Lyrics and Metadata](lyrics-and-metadata.md) — the everyday overview of tags and lyrics
- [Quality Settings](quality-settings.md) — codec choices affect which audio-trait tags are written
