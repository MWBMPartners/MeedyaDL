<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Downloading Videos

This page shows how to download music videos from Apple Music with MeedyaDL, and how to choose their quality.

---

## Quick Start

1. Open the music video on Apple Music in your browser and copy the address from the address bar.
2. Paste it into MeedyaDL's link box.
3. Click download. MeedyaDL knows it is a video, so you do not have to choose anything.
4. Watch it in the queue.

Videos use the same queue as music. You can paste songs and videos together.

---

## Supported Video Types

### Music Videos

Music videos are the official videos artists publish on Apple Music. Their links contain `/music-video/`, for example:

```text
https://music.apple.com/us/music-video/example-title/1234567890
```

MeedyaDL spots the `/music-video/` part and treats the link as a video.

A music video is saved as a `.m4v` file (an MP4 video) with sound included. It is tagged with its title, artist, album and artwork.

### Post Videos

Post videos are shorter clips, such as promotional or behind-the-scenes videos. You download them exactly like music videos: paste the link and go.

---

## Video Quality Options

### Resolution

The resolution you pick is a **ceiling**, not a request. Apple Music always gives you the best quality it has at or below your choice. A high ceiling never causes a failure. If a video only exists in a lower resolution, you simply get that one.

You set it in **Settings > Codec & Resolution > Maximum Video Resolution**.

| Resolution | Label | Good for |
|------------|-------|----------|
| 2160p | 4K | Best quality. Large files. |
| 1440p | 2K | High quality, smaller than 4K. |
| 1080p | Full HD | **Recommended.** The best balance. |
| 720p | HD | Good quality, moderate size. |
| 540p | — | A step below standard HD. |
| 480p | SD | Standard definition. Small files. |
| 360p | — | Low quality. Very small. |
| 240p | — | Lowest quality. |

**Recommendation:** Choose 1080p. With the H.265 codec it gives the best quality for the size.

**File size:** A 4K music video can be several hundred megabytes. Check your free disk space first.

**About 4K:** Not every video is offered in 4K, and 4K is not available on every set-up. If you ask for 4K and get less, you have received the highest quality your set-up could reach.

### Video Codec

The codec is how the video is squeezed into a file. Different codecs trade picture quality, file size and compatibility. See [Quality Settings](quality-settings.md) for a comparison.

If your first-choice codec is not offered for a video, MeedyaDL tries the next one on your list. You can change the order in **Settings > Codec Fallback Order > Video Fallback**. See [Fallback Quality](fallback-quality.md).

### Sound Quality

The sound in a video uses your audio codec settings. It is normally AAC. See [Quality Settings](quality-settings.md).

---

## Downloading a Video

1. **Copy the video link** from Apple Music in your browser.
2. **Paste it** into MeedyaDL's link box.
3. **Check your quality settings** in Settings first, or use the defaults.
4. **Click download.** The video joins the queue.
5. **Watch progress** in the queue. Each item shows its percentage.

Your video settings apply to every video you download from then on. Resolution is always a maximum. Asking for 4K for a video that only exists in 1080p just gives you the 1080p version, with no failure and nothing to do.

---

## Output Files

### The Video File

- A `.m4v` file (MP4 format).
- The picture at your chosen quality, with sound included.
- Title, artist, album and artwork added.

Videos go to your normal output folder, the same one used for music. File names follow the templates in **Settings > Templates**. See [Lyrics and Metadata](lyrics-and-metadata.md).

**Where a video is saved.** When Apple Music links the video to an album, MeedyaDL saves it in that album's folder, next to the songs. When there is no album, it goes in a `Music Videos` folder under the artist. In that case Apple's video number is added to the file name, so two videos with the same title never overwrite each other. With recent versions of the download tool, most videos are now found with their album. A video you download now may therefore land beside the songs where an older download went into `Music Videos`. Nothing already on disk is moved or deleted.

### Subtitles and Lyrics

Music videos can come with timed lyrics or subtitles. By default you get a **TTML** file (Apple's own subtitle format) and an **SRT** file beside it, which works in more players. Choose the formats in **Settings > Lyrics**. See [Lyrics and Metadata](lyrics-and-metadata.md).

<details>
<summary>More detail: subtitle and caption file names</summary>

If a video carries its own subtitle or caption tracks, MeedyaDL saves each one as a separate file next to the video. The name is the video's name, then the language, then the kind of track, then the file type. For a video called `Song.mp4`:

- `Song.en.srt` — English subtitles
- `Song.en.sdh.srt` — English captions for people who are deaf or hard of hearing
- `Song.fr.forced.vtt` — French subtitles shown only where needed (for example a sign in another language)
- `Song.en.commentary.srt` — an English commentary track
- `Song.und.srt` — a track that does not say which language it is in

If two tracks would get the same name, the second gets a number (`Song.en.2.srt`). The same video always gives the same names, so downloading it again does not make copies. Files saved by older versions of MeedyaDL keep their old names (such as `Song.cc.2.eng.srt`) and are never renamed. A new download recognises them, so the same track is not saved twice.

These files are never mistaken for the song's lyrics. The lyric file MeedyaDL puts beside a music video is named exactly like the video, with no language in it (`Song.srt`).

</details>

### Tags

Each video is tagged with:

- **Title** — the video's name
- **Artist** — who performs it
- **Album** — the album, when there is one
- **Artwork** — the cover or a thumbnail picture

So your videos show up properly in players and libraries.

---

## Tips

- **Check your disk space.** High-resolution videos are big, especially 4K.
- **Choose 1080p with H.265** for the best size and quality.
- **Check your cookies first.** Video downloads need a valid Apple Music sign-in. See [Cookie Management](cookie-management.md).
- **Use a steady connection.** Big files take a while.
- **Mix music and videos.** The queue handles both.

<details>
<summary>More detail: the Remux Mode setting</summary>

**Settings > Advanced > Remux Mode** chooses which helper program joins a video's picture, sound and subtitles into one file. MP4Box is the default. It keeps subtitles and captions more reliably than FFmpeg.

This setting only has an effect on GAMDL 3.5 and earlier. From GAMDL 3.6 the joining is built in, and the setting is ignored. If you are on an older GAMDL and see "Invalid data found when processing input", check that Remux Mode is set to MP4Box.

</details>

---

## Music Video Companions

MeedyaDL can fetch the music video for a song automatically when you download its audio. When you download an album or song, it checks whether each track has a music video. If one exists, it is downloaded too, using your video quality settings. This feature is marked experimental.

### Turning It On

Go to **Settings > Codec & Resolution > Video Quality** and turn on **Music Video Companions (Experimental)**.

- **With MusicKit details** (Team ID, Key ID and private key in **Settings > Advanced > API Credentials**), MeedyaDL can ask Apple Music which tracks have a video. This is the fuller source.
- **Without them**, MeedyaDL looks the tracks up in the public MusicBrainz database instead. This needs no sign-up, but it finds fewer videos because MusicBrainz is kept up by volunteers.

You can also turn on **MusicBrainz Video Lookup** on its own. See [Lyrics and Metadata](lyrics-and-metadata.md) for how Apple's details are set up. The same steps are in [Animated Artwork](animated-artwork.md).

### What to Expect

- Videos are saved in the same folder as your audio.
- The video download runs after the music and its extras are done, while the item shows as processing. The progress bar shows speed, time left and percentage.
- The "download finished" message waits until the videos and the other extras are done.
- Tracks with no music video are skipped quietly.
- The same video linked from several tracks is only downloaded once.
- Progress and results appear in the Activity Log.
- Your video resolution, video codec order and remux settings apply to these videos too.

---

## Related Topics

- [Downloading Music](downloading-music.md) — Audio-only downloads
- [Quality Settings](quality-settings.md) — Video codec and resolution options
- [Fallback Quality](fallback-quality.md) — What happens when a codec is not offered
- [Lyrics and Metadata](lyrics-and-metadata.md) — Subtitles and tags
- [Cookie Management](cookie-management.md) — Your sign-in cookies
- [Troubleshooting](troubleshooting.md) — Fixing common problems

---

[Back to Help Index](index.md)
