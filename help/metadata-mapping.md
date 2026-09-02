<!--
  MeedyaDL Help Documentation
  Copyright (c) 2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# :bookmark_tabs: Metadata Mapping Reference

This page documents **every** metadata field MeedyaDL writes into downloaded files. It is the canonical reference for music-library tooling (MusicBrainz Picard, beets, Mp3tag, foobar2000) and for anyone scripting against MeedyaDL's output.

Last updated: 2026-05-17 (v1.7 bundle).

---

## :compass: Table of Contents

1. [How metadata gets written](#how-metadata-gets-written)
2. [Tag namespaces](#tag-namespaces)
3. [Standard M4A/MP4 atoms (4-char)](#standard-m4amp4-atoms-4-char)
4. [Apple proprietary atoms](#apple-proprietary-atoms)
5. [iTunes freeform atoms (`com.apple.iTunes:*`)](#itunes-freeform-atoms-comappleitunes)
6. [MeedyaMeta freeform atoms (`MeedyaMeta:*`)](#meedyameta-freeform-atoms-meedyameta)
7. [Per-format support matrix](#per-format-support-matrix)
8. [API source mapping](#api-source-mapping)
9. [Industry-standard alternative names](#industry-standard-alternative-names)
10. [How to inspect tags on disk](#how-to-inspect-tags-on-disk)

---

## How metadata gets written

After GAMDL writes the standard iTunes tags, MeedyaDL runs a **12-stage enrichment pipeline** in [`services/metadata_tag_service.rs`](../src-tauri/src/services/metadata_tag_service.rs). The pipeline fetches data from two APIs and merges results, then writes ~30 freeform atoms per file using the `mp4ameta` crate's `set_data()` + `write_to_path()` (preserves existing tags — never destructive).

Tag definitions are declarative — they live in [`src-tauri/tags.toml`](../src-tauri/tags.toml) as a registry that maps Apple Music API JSON paths to MP4 atom targets. The Rust module that loads and queries the registry is [`models/tag_registry.rs`](../src-tauri/src/models/tag_registry.rs).

A small number of tags are written by **hardcoded** Rust functions instead of the registry — these have conditional logic that doesn't fit a declarative TOML model:

- **Codec-specific**: `isLossless` (ALAC only), `SpatialType` (Atmos only)
- **Always-on local**: `SourceStore`, `EncodeSource`, `iTunesMediaType`, `isMedley`
- **Channel detection**: `ChannelConfig` (via ffprobe, not API)

---

## Tag namespaces

Every freeform atom MeedyaDL writes lives in one of two namespaces:

| Namespace | Reverse-DNS | Why |
|---|---|---|
| `itunes` | `com.apple.iTunes` | **Player-compatible** — recognised by iTunes, Music.app, Apple Music app, and most third-party players (Picard, beets, Mp3tag, foobar2000). Use for anything that needs to be readable by other apps. |
| `meedya` | `MeedyaMeta` | **MeedyaDL-branded** — namespaced under our own reverse-DNS so we can never collide with iTunes or third-party tools. Use for MeedyaDL-attribution metadata (provenance, processing decisions, internal flags). |

Most fields are written to **both** namespaces simultaneously (dual-write strategy) so player compatibility and MeedyaDL attribution don't conflict. See the registry for each field's atom list.

### Industry-standard names

Some fields use **industry-standard names** in the iTunes namespace where the standard is well-established. These are recognised by tools beyond just iTunes:

| MeedyaDL field | Industry-standard atom | Recognised by |
|---|---|---|
| `record_label` | `LABEL` | MusicBrainz Picard, Mp3tag, foobar2000, beets |
| `copyright` | `COPYRIGHT` | Mp3tag, foobar2000 |
| `upc` | `UPC` + `Barcode` | MusicBrainz Picard, beets |
| `is_compilation` | `COMPILATION` | MusicBrainz Picard, Mp3tag, beets |
| `track_count` | `TOTALTRACKS` (alongside `AlbumTrackCount`) | MusicBrainz Picard, foobar2000 |

---

## Standard M4A/MP4 atoms (4-char)

These are written by GAMDL during the primary download. MeedyaDL doesn't touch them — they're shown here for completeness so the full picture is in one place.

| Atom | Standard name | Type | Notes |
|---|---|---|---|
| `©nam` | Title | string | Track title. |
| `©ART` | Artist | string | Track artist. |
| `aART` | Album Artist | string | Album-level artist. |
| `©alb` | Album | string | Album title. |
| `©gen` | Genre (custom) | string | Free-text genre. |
| `gnre` | Genre (ID3v1) | u8 | Numeric genre code (legacy). |
| `©day` | Year/Date | string | Release year or full date. |
| `©wrt` | Composer | string | Composer name. |
| `trkn` | Track number | u16 pair | `(current, total)`. |
| `disk` | Disc number | u16 pair | `(current, total)`. |
| `covr` | Cover artwork | binary | Embedded image (JPEG/PNG). |
| `tmpo` | BPM | u16 | Beats per minute (optional). |
| `cpil` | Compilation flag | bool | Native compilation marker. |
| `©lyr` | Lyrics | string | Embedded lyrics (Enhanced LRC if available). |
| `pgap` | Gapless | bool | Gapless-playback hint. |
| `rtng` | Content rating | u8 | 0=none, 2=clean, 4=explicit. |
| `stik` | Media kind | u8 | 1=movie, 6=music video, etc. |

---

## Apple proprietary atoms

Numeric IDs from the Apple Music ecosystem, written by GAMDL. **Read** by MeedyaDL during enrichment and re-surfaced in freeform form. Useful for cross-referencing with the Apple Music API and iTunes Store.

| Atom | Full name | Source | Example | Notes |
|---|---|---|---|---|
| `cnID` | Content (Song) ID | GAMDL/CDN | `1649434005` | Track-level Apple Music catalogue ID. |
| `cmID` | Composer ID | GAMDL/CDN | `429153007` | Composer entity ID. |
| `atID` | Artist ID | GAMDL/CDN | `159260351` | Artist entity ID. Combine with `https://music.apple.com/{storefront}/artist/{atID}` for the canonical artist URL. |
| `plID` | Playlist/Album ID | GAMDL/CDN | `6750434` | Album entity ID. |
| `geID` | Genre ID | GAMDL/CDN | `21` (= Rock) | Numeric genre code from Apple Music's taxonomy. |
| `sfID` | Storefront ID | GAMDL/CDN | `143444` (= UK) | Apple Music storefront — see [storefront list](https://help.apple.com/itc/appsreference/#/itc6deb35a05). |
| `akID` | Account kind | GAMDL/CDN | `1` | Audio kind classification. |

Inside `MeedyaMeta:*` the same IDs are re-exposed with friendly names (e.g., `MeedyaMeta:ArtistID`, `MeedyaMeta:AlbumID`) for tools that don't speak Apple's 4-char codes.

---

## iTunes freeform atoms (`com.apple.iTunes:*`)

Written by the registry. **Player-compatible** — iTunes, Music.app, Picard, Mp3tag, foobar2000, and beets all read these by default.

### Album scope

Same value on every track in the album. JSON paths are relative to `data[0]` of the Apple Music Catalog API response.

| Atom | JSON path | Type |
|---|---|---|
| `AlbumAdvisory` | `attributes.contentRating` | string |
| `AlbumArtistID` | `relationships.artists.data[0].id` | string |
| `AlbumArtistSort` | `attributes.artistName` | string |
| `AlbumGenre` | `attributes.genreNames[0]` | string |
| `UPC` + `Barcode` | `attributes.upc` | string |
| `LABEL` | `attributes.recordLabel` | string |
| `COPYRIGHT` | `attributes.copyright` | string |
| `AlbumReleaseDate` | `attributes.releaseDate` | string |
| `COMPILATION` | `attributes.isCompilation` | bool |
| `AlbumIsSingle` | `attributes.isSingle` | bool |
| `AlbumIsComplete` | `attributes.isComplete` | bool |
| `AlbumMasteredForItunes` | `attributes.isMasteredForItunes` | bool |
| `AlbumTrackCount` + `TOTALTRACKS` | `attributes.trackCount` | u32 |
| `AlbumEditorialNote` | `attributes.editorialNotes.short` | string |
| `MotionArtURL` | `attributes.editorialVideo.motionDetailSquare.video` | string |
| `MotionArtPortraitURL` | `attributes.editorialVideo.motionDetailTall.video` | string |
| `AlbumLastModified` | `attributes.lastModifiedDate` | string |

### Track scope

Per-track values, matched to each M4A file by track/disc number. JSON paths are relative to each track object in `data[0].relationships.tracks.data[*]`.

| Atom | JSON path | Type |
|---|---|---|
| `ISRC` | `attributes.isrc` | string |
| `iTunesAdvisory` | `attributes.contentRating` | string |
| `iTunesArtistID` | `relationships.artists.data[0].id` | string |
| `iTunesCatalogID` + `StoreID/AppleMusic` | `id` | string |
| `AppleDigitalMaster` | `attributes.isAppleDigitalMaster` | bool |
| `ReleaseDate` | `attributes.releaseDate` | string |
| `Composer` | `attributes.composerName` | string |
| `DurationMs` | `attributes.durationInMillis` | u64 |
| `HasLyrics` | `attributes.hasLyrics` | bool |
| `PlayParamsId` | `attributes.playParams.id` | string |
| `TrackUrl` | `attributes.url` | string |
| `PreviewUrl` | `attributes.previews[0].url` | string |
| `Genre` | `attributes.genreNames[]` | array |

---

## MeedyaMeta freeform atoms (`MeedyaMeta:*`)

MeedyaDL's branded namespace. **All** of the album-scope and track-scope tags above are also written under `MeedyaMeta:` with friendlier names (e.g., `MeedyaMeta:AppleRecordLabel`, `MeedyaMeta:AppleUPC`, `MeedyaMeta:AppleAudioTraits`). Use these when you want a tool to know the data came from MeedyaDL specifically — useful for filtering in mixed-source libraries.

### MeedyaDL-only fields

These have no `com.apple.iTunes:*` counterpart — they're MeedyaDL-attributed only:

| Atom | What it records | Set by |
|---|---|---|
| `MeedyaMeta:AppleAudioTraits` | The track's `audioTraits` array — e.g., `["atmos","lossless","lossy-stereo"]` | Apple Music Catalog API |
| `MeedyaMeta:SourceStore` | Where the file came from (`AppleMusic`, `Spotify`, etc.) | Hardcoded per service |
| `MeedyaMeta:EncodeSource` | Which engine produced the audio (`gamdl`, `votify`, …) | Hardcoded per engine |
| `MeedyaMeta:SpatialAudioCodec` | Atmos / Spatial encoding variant | ffprobe detection |
| `MeedyaMeta:ChannelConfig` | `5.1.4`, `7.1`, etc. | ffprobe detection |
| `MeedyaMeta:ReplayGainTrack` / `ReplayGainAlbum` | EBU R128 loudness values | Internal analysis (opt-in) |
| `MeedyaMeta:AcoustIDFingerprint` + `AcoustIDID` | Chromaprint fingerprint + canonical recording ID | Internal fingerprinting (opt-in) |
| `MeedyaMeta:MusicBrainzRecordingID` | MB recording UUID | MusicBrainz lookup (opt-in; planned — not yet written by current releases) |
| `MeedyaMeta:MusicBrainzExternalUrls` | Cross-platform URLs (Spotify, YouTube, Tidal, Deezer…) | MusicBrainz lookup (opt-in; planned — not yet written by current releases) |
| `MeedyaMeta:AppleLastModifiedDate` | When the album was last updated on Apple Music (drives smart re-download detection) | Apple Music Catalog API |

---

## Per-format support matrix

| Field class | M4A / M4V (MP4 atoms) | FLAC (Vorbis comments) | MP3 (ID3v2 frames) | OGG / Opus (Vorbis comments) |
|---|---|---|---|---|
| Standard tags (title/artist/album/…) | :white_check_mark: | :white_check_mark: | :white_check_mark: | :white_check_mark: |
| `covr` (cover artwork) | :white_check_mark: | :white_check_mark: (`METADATA_BLOCK_PICTURE`) | :white_check_mark: (APIC) | :white_check_mark: |
| iTunes freeform atoms | :white_check_mark: | :grey_question: (some via Vorbis fallback) | :grey_question: (TXXX) | :grey_question: |
| `MeedyaMeta:*` freeform atoms | :white_check_mark: | :white_check_mark: (Vorbis) | :white_check_mark: (TXXX) | :white_check_mark: (Vorbis) |
| ReplayGain | :white_check_mark: (via `mp4ameta`) | :white_check_mark: (`lofty`) | :white_check_mark: (`lofty`) | :white_check_mark: (`lofty`) |
| BPM (`tmpo` / `BPM` / `TBPM`) | :white_check_mark: (`tmpo`) | :white_check_mark: (`BPM` Vorbis) | :white_check_mark: (`TBPM` ID3v2) | :white_check_mark: |
| Enhanced LRC lyrics (`©lyr`) | :white_check_mark: | :grey_question: (text only, no `<mm:ss.xx>` parsing in most players) | :grey_question: | :grey_question: |

GAMDL produces M4A (audio) and M4V (music videos) by default. The other formats appear when MeedyaDL is configured to use a non-GAMDL engine (e.g., yt-dlp for YouTube → MP3, votify for Spotify → Ogg Vorbis).

---

## API source mapping

Which API contributes each field. Enrichment fetches both APIs and merges results — Apple Music Catalog **overwrites** iTunes Lookup when both provide the same field (Catalog data is richer).

| Source | Auth | Provides |
|---|---|---|
| **iTunes Lookup API** (`itunes.apple.com/lookup`) | None (public) | Country, DiscCount, iTunesTrackURL. Runs first. |
| **Apple Music Catalog API** (`amp-api.music.apple.com/v1/catalog/...`) | MusicKit JWT | `editorialNotes`, `audioTraits`, animated artwork URLs, `contentRating`, `lastModifiedDate`, `isAppleDigitalMaster`, `recordLabel`, `copyright`, `releaseDate`, `composerName`, `durationInMillis`, `hasLyrics`, `playParams.id`, `previews[].url`, `genreNames`, plus all album-scope tags. Runs second and supersedes iTunes Lookup for shared fields. |
| **MusicKit `/syllable-lyrics`** | MusicKit JWT + Music-User-Token from cookies | Word-level TTML for Enhanced LRC upgrade (fallback when GAMDL's TTML lacks word timing). |
| **GAMDL / Apple CDN** | Cookies or wrapper | All standard 4-char atoms (`©nam`, `©ART`, …), proprietary IDs (`cnID`, `atID`, …), `covr` artwork. |
| **MusicBrainz API** (`musicbrainz.org/ws/2/`) | None (public, rate-limited 1 req/sec) | Cross-platform external URLs (Spotify, YouTube, Tidal, Deezer, Bandcamp, SoundCloud), MB recording ID. Opt-in via Settings > Quality > Video Quality. |
| **AcoustID** (`api.acoustid.org`) | Embedded API key | Chromaprint fingerprint + AcoustID recording ID. Opt-in via Settings > Metadata. |
| **ffprobe** (local binary) | None | `SpatialAudioCodec`, `ChannelConfig`, codec confirmation when native priority is active. |

---

## Industry-standard alternative names

For maximum compatibility with non-Apple tooling, MeedyaDL writes these fields under **both** an Apple-style name and an industry-standard alias. If you're scripting metadata extraction, prefer the industry-standard atom name — your code will work across more formats.

| Apple-style | Industry-standard | Recognised by |
|---|---|---|
| `iTunesCatalogID` | `StoreID/AppleMusic` | beets `apple_music` plugin |
| `AlbumArtist` | (native `aART`) | All MP4 readers |
| `cnID` / `iTunesCatalogID` | `MUSICBRAINZ_TRACKID` (planned — not yet written by current releases) | Picard, beets |
| `LABEL` | (same) | Picard, Mp3tag, foobar2000, beets |
| `COPYRIGHT` | (same) | Mp3tag, foobar2000 |
| `UPC` + `Barcode` | (same) | Picard, beets |
| `COMPILATION` | (same) | Picard, Mp3tag, beets |
| `AlbumTrackCount` | `TOTALTRACKS` | Picard, foobar2000 |
| `ISRC` | (same) | Picard, Mp3tag, foobar2000, beets |

---

## How to inspect tags on disk

### Quick check (any platform)

```bash
ffprobe -loglevel quiet -show_format -show_streams "song.m4a" | grep -i "tag\|metadata"
```

### Full freeform-atom dump (recommended for debugging)

```bash
# Requires `mp4info` (part of mp4v2 / gpac)
mp4info "song.m4a"

# Or via `AtomicParsley` (macOS / Linux)
AtomicParsley "song.m4a" -T
```

### Picard / beets / Mp3tag

Open the file in MusicBrainz Picard, beets shell, or Mp3tag and look at the full tag list. Industry-standard names show up under their canonical name; `com.apple.iTunes:*` shows up as namespaced freeform; `MeedyaMeta:*` shows up under the `MeedyaMeta` namespace.

### Programmatic (Python)

```python
from mutagen.mp4 import MP4

f = MP4("song.m4a")
for key, value in f.tags.items():
    print(f"{key!r}: {value!r}")
```

Look for keys starting with `----:com.apple.iTunes:` (iTunes freeform) and `----:MeedyaMeta:` (MeedyaDL freeform).

---

## See also

- [Lyrics and Metadata](lyrics-and-metadata.md) — narrative overview of the enrichment pipeline and per-format lyrics support
- [Quality Settings](quality-settings.md) — codec choices that affect which `MeedyaMeta:*` audio-trait tags get written
- [`src-tauri/tags.toml`](../src-tauri/tags.toml) — the canonical declarative registry (this page is generated from it)
- [`src-tauri/src/services/metadata_tag_service.rs`](../src-tauri/src/services/metadata_tag_service.rs) — the hardcoded-tag implementations
