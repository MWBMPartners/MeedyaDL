<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root for details.
-->

# Quality Settings

This page explains the audio and video quality choices in MeedyaDL, and how to pick the right ones for you.

---

## Overview

You choose the format and quality of what you download. The choice is a trade-off between sound or picture quality, file size, and whether your devices can play the file.

If you just want a good answer:

- **Music for everyday listening:** choose **AAC**. Files are small and play everywhere.
- **Best possible sound:** choose **ALAC (Lossless)**.
- **Surround or 3D sound:** choose **Dolby Atmos**, and keep the fallback list on so tracks without an Atmos mix still download.
- **Videos:** choose **1080p**.

Set your default in **Settings > Codec & Resolution** (see [Configuring Quality in MeedyaDL](#configuring-quality-in-meedyadl)).

---

## About the "(Experimental)" Label

Most audio formats are marked **(Experimental)** in the format list. This means a download in that format may sometimes fail, even though it works most of the time. If one fails:

1. **Retry it.** These failures are often one-off.
2. **Turn on the fallback list.** In **Settings > Codec & Resolution**, switch on **Enable Fallback Chain**, so MeedyaDL tries the next format by itself.
3. **Use the wrapper if you need ALAC.** See [Wrapper Authentication](wrapper.md) for what it is and whether you need it.

**AAC Legacy** and **AAC-HE Legacy** are the most reliable choices.

<details>
<summary>More detail: which formats need the wrapper</summary>

What needs the wrapper depends on the GAMDL version you have installed:

- **GAMDL 3.8 and newer:** only **ALAC (Lossless)** needs the wrapper to download reliably. Dolby Atmos, AC3 and the AAC variants work without it.
- **GAMDL 3.0 to 3.7:** the special formats (ALAC, Atmos, AC3, and the AAC variants other than the two Legacy ones) need the wrapper to download reliably. Without it they may fail now and then.

The **(Experimental)** label stays on every GAMDL version. [Wrapper Authentication](wrapper.md) has the full version-by-version list. The wrapper runs directly on Linux. On macOS and Windows it needs Docker Desktop (GAMDL 3.6 and newer).

</details>

---

## Audio Formats

### AAC (Advanced Audio Coding)

AAC is Apple Music's standard format. It gives good sound in small files.

- **Quality:** 256 kbps (lossy)
- **File type:** `.m4a`
- **Size:** about 2 MB per minute
- **Plays on:** almost everything, including all Apple devices, Android, Windows and web browsers
- **Best for:** everyday listening and phones. For most people, AAC at 256 kbps sounds the same as lossless.

### AAC-HE (High Efficiency AAC)

A smaller, lower-quality version of AAC.

- **Quality:** lower than standard AAC (about 48 to 96 kbps)
- **Plays on:** most modern devices and players
- **Best for:** when keeping files as small as possible matters more than sound. It is noticeably worse than AAC 256, so it is not good for careful listening.

### AAC Binaural

A version of spatial music made for headphones. It turns a Dolby Atmos or other spatial mix into two-channel stereo that feels like it comes from all around you.

- **Output:** stereo
- **File type:** `.m4a`
- **Best for:** hearing spatial music on ordinary headphones, with no Atmos equipment needed.

### AAC Downmix

A normal stereo version of music that was made in surround or spatial sound.

- **Output:** stereo
- **File type:** `.m4a`
- **Best for:** a plain stereo copy for speakers or headphones. Unlike Binaural, it does not try to make sound seem to come from around you.

### AAC Legacy

An older AAC style made for maximum compatibility.

- **File type:** `.m4a`
- **Best for:** very old devices and players. Choose it only if standard AAC does not play on your older equipment.

### ALAC (Apple Lossless)

ALAC loses no sound at all. The music is exactly as the original, only packed into a smaller file. It is the best-sounding choice.

- **File type:** `.m4a`
- **Quality levels and approximate size per minute:**
  - **16-bit / 44.1 kHz:** CD quality, about 5 MB
  - **24-bit / 48 kHz:** studio quality, about 7 MB
  - **24-bit / 96 kHz:** hi-res, about 10 MB
  - **24-bit / 192 kHz:** the highest available, about 15 MB
- **Plays on:** all Apple devices and iTunes, and many other players. Some non-Apple devices need the file converted to FLAC.
- **Best for:** careful listening on good equipment, and keeping a lasting copy when storage is not a concern.

### Dolby Atmos

Dolby Atmos places sounds all around and above you instead of just left and right.

- **File type:** `.m4a`
- **Size:** varies with the mix
- **To hear it properly:** you need Atmos-ready equipment, such as AirPods Pro, AirPods Max, AirPods (3rd generation and later), or Atmos speakers, soundbars and receivers. On other devices it plays as stereo or surround.
- **Best for:** immersive listening when you have the right equipment. Not every track has an Atmos mix.

### AC3 (Dolby Digital)

A surround-sound format common in home cinemas.

- **Sound:** up to 5.1 surround (five main channels plus one for deep bass)
- **Best for:** home cinema systems with a surround receiver or soundbar. It is the standard DVD sound format.

### Ask (Automatic)

Lets MeedyaDL's download tool pick the most suitable format for each item.

- **Best for:** when you have no preference.

---

## Companion Downloads

Companion downloads get you more than one format of the same music in one go. You might want a special format (such as Dolby Atmos) for good equipment, and a plain one that works on every device. Choose the mode from the **Companion Downloads** dropdown in **Settings > Codec & Resolution**.

### Companion Modes

| Mode | What it does |
| ---- | ------------ |
| **Disabled (no companions)** | No extra copies. You only get the format you chose. |
| **Atmos → Lossless (ALAC)** *(default)* | When you download Dolby Atmos, you also get an ALAC (lossless) copy. Downloads in other formats get no extra copy. |
| **Atmos → Lossless + Lossy; ALAC → Lossy** | Atmos also gets an ALAC copy and an AAC copy. An ALAC download also gets an AAC copy. |
| **Specialist → Lossy (AAC)** | Atmos or ALAC downloads also get an AAC copy. |
| **Atmos → All Formats (AC3 + ALAC + AAC)** | Atmos also gets a copy in every other format (AC3, ALAC, AAC). |
| **Custom…** | You pick exactly which formats to get as extra copies, using tick boxes. |

Extra copies are made after the main download's lyrics and file names are finished, and before the next item in the queue starts. If a format is not offered for a track, it is skipped (the Activity page says so), and your main download is not affected.

- **Extra copies never replace a file.** If a file with the same name is already in the folder, that copy is skipped and one line on the **Activity** page says so. This also means downloading an album again does not refresh extra copies that are already there.
- **Mixed albums.** If some tracks of the main download already came back in a format, only the other tracks are fetched in that format.
- **Only this download is renamed.** Renaming applies only to the files this download made, and each extra copy's format details are written only to the copies in that format. Older files in the same folder keep their names.
- **Lyrics.** Each extra copy gets the same lyric files as the main download (see [Lyrics and Metadata](lyrics-and-metadata.md#extra-format-versions-of-the-same-song)).

### File Names

MeedyaDL adds a short tag to file names, such as `[Dolby Atmos]` or `[Lossless]`, so copies of the same song in different formats can sit side by side. Some files, such as standard AAC, have no tag. All versions go in the same album folder.

- Dolby Atmos: `01 Song Title [Dolby Atmos].m4a`
- ALAC (lossless): `01 Song Title [Lossless].m4a`
- AAC Legacy: `01 Song Title [AAC Legacy].m4a`

Example, with "Atmos → Lossless + Lossy; ALAC → Lossy" and a Dolby Atmos download:

```text
Artist/Album/
  01 Song Title [Dolby Atmos].m4a   ← Main copy (spatial audio)
  01 Song Title [Lossless].m4a      ← ALAC copy
  01 Song Title [AAC Legacy].m4a    ← AAC copy (the tag, if any, depends on the AAC type and your settings)
```

MeedyaDL checks each main file after it downloads, so a file that came back as ALAC when you asked for Atmos is named `[Lossless]`, not `[Dolby Atmos]`. The exact tags can vary with your settings, so use them as a guide rather than a guarantee.

### Custom Mode

With **Custom…** you see a list of tick boxes, one per format. Every format you tick is downloaded as its own extra copy, even if it is the same as your main format. Most copies get a tag in their names, such as `[Lossless]` or `[Dolby Atmos]`, so they can sit side by side in the album folder. Some, such as standard AAC, have none.

Choose Custom for a mix the other modes don't cover, such as AAC Binaural and ALAC alongside Atmos, or every format for a full archive.

<details>
<summary>More detail: extra information saved inside the files</summary>

MeedyaDL also saves extra information inside special-format files, so other programs can tell what they are. ALAC files get `isLossless = Y`. Dolby Atmos files get `SpatialType = Dolby Atmos`. See [Lyrics and Metadata](lyrics-and-metadata.md#what-meedyadl-adds-after-each-download) for more.

</details>

<details>
<summary>More detail: how MeedyaDL tells which format a file is (MediaInfo)</summary>

MeedyaDL checks each finished file to find its real format, using FFmpeg and **MediaInfo**. MediaInfo is installed with the other helper tools during setup, so you do not need to do anything. It is better at telling Dolby Atmos from ordinary Dolby Digital, so it matters most if you download a lot of Atmos.

When you ask for several formats in one go, MeedyaDL only knows which one came back once the download is finished, so the tag is added after it checks the file.

</details>

---

## Audio Comparison

| Format | Type | Typical bitrate | Size per minute | Best for |
| ------- | ------ | ----------------- | --------------------- | ---------- |
| AAC 256 | Lossy | 256 kbps | ~2 MB | Everyday listening, plays everywhere |
| AAC Binaural | Lossy, spatial | 256 kbps | ~2 MB | Spatial sound on ordinary headphones |
| ALAC 16/44.1 | Lossless | ~800 kbps | ~5 MB | CD-quality listening |
| ALAC 24/96 | Lossless | ~1200 kbps | ~10 MB | Hi-res on good headphones or speakers |
| ALAC 24/192 | Lossless | ~1600 kbps | ~15 MB | Highest quality, for keeping |
| AC3 (Dolby Digital) | Lossy, 5.1 surround | ~288 kbps | ~2 MB | Home cinema surround |
| Atmos | Spatial | Varies | Varies | 3D sound with the right equipment |

Sizes are rough. They change with the music. Lossless sizes vary most, because quiet or simple passages pack smaller.

### Pros and Cons

| Format | Good | Not so good |
| ----- | ---- | ---- |
| **ALAC (Lossless)** | Perfect quality, up to 24-bit/192 kHz, ideal for keeping | Files 2.5 to 7 times bigger than AAC. The difference can be hard to hear on basic equipment. |
| **Dolby Atmos** | Immersive 3D sound on the right equipment | Needs Atmos equipment (AirPods Pro or Max, an Atmos soundbar). Plays as plain stereo elsewhere. Not every track has an Atmos mix. |
| **AC3** | True 5.1 surround. Works with receivers and soundbars. | Lossy, and not useful on phones or ordinary headphones. Fewer tracks offer it. |
| **AAC 256** | Plays everywhere, small files, and most people can't tell it from lossless | Lossy, so some sound is lost for good |
| **AAC Binaural** | Spatial feel on any headphones, small files | The effect is only an approximation. Only helps with headphones, not speakers. |
| **AAC Legacy** | Plays on very old devices | No benefit on modern equipment |

---

## Video Formats

### H.264 (AVC)

The most widely supported video format. Almost every device can play it.

- **File size:** average.
- **Plays on:** everything modern, and most old devices.
- **Best for:** playing on many different devices, including older ones.

### H.265 (HEVC)

The newer format. It gives the same picture in files that are roughly a third to a half smaller.

- **File size:** about 30 to 50 percent smaller than H.264 for the same picture.
- **Plays on:** most devices made since about 2017. Some older devices and players can't play it without extra software.
- **Best for:** saving space, especially at 1080p and above, if your devices support it.

---

## Video Resolution

The resolution you pick is a **ceiling**, not a promise. MeedyaDL gets the best picture the video has, up to your choice. If a video does not reach your resolution, you get what it does have. The download does not fail. See [Fallback Quality](fallback-quality.md#why-there-is-no-resolution-fallback-chain) for why resolution has no fallback list, unlike video format.

From best to smallest:

- **2160p (4K Ultra HD):** the sharpest, and the largest files. For big 4K screens.
- **1440p (2K):** a step up from Full HD on large monitors.
- **1080p (Full HD):** the best choice for most people. Great on screens up to about 27 inches.
- **720p (HD):** good on phones and tablets, with much smaller files.
- **540p:** between 720p and 480p, for smaller files without dropping to standard definition.
- **480p:** standard definition in widescreen shape. For small screens or little storage.
- **360p:** low quality, very small. For previews.
- **240p:** the smallest. Only for thumbnails or very little storage.

For how to download videos, see [Downloading Videos](downloading-videos.md).

### Typical Video File Sizes

Based on a 4-minute music video:

| Resolution | Format | Typical size | Best for |
| ------------ | ------- | --------------------------- | ---------- |
| 1080p | H.264 | ~150 MB | Plays on every device |
| 1080p | HEVC | ~100 MB | Smaller files on modern devices |
| 4K (2160p) | HEVC | ~400 MB | Best picture on large screens |

Sizes change with how busy the picture is, because fast, detailed scenes make bigger files. 4K with H.264 is uncommon, because HEVC is much better at high resolutions. For most people, 1080p with HEVC gives the best mix of quality and size.

---

## Configuring Quality in MeedyaDL

### Setting Your Default Quality

1. Open **Settings**.
2. Go to the **Codec & Resolution** tab.
3. Under **Audio Quality**, pick your preferred audio format.
4. Under **Video Quality**, pick your preferred video resolution.
5. Click **Save Changes**.

Every new download uses these unless you change them for one download (see below).

### Companion Downloads Setting

The same tab has the **Companion Downloads** dropdown. The default is **Atmos → Lossless (ALAC)**. **Custom…** shows tick boxes for all 11 audio formats. See [Companion Downloads](#companion-downloads) above.

### Artist Auto-Select

When you download an **artist link**, **Artist Auto-Select** (in the **Codec & Resolution** tab) decides which kinds of release to get:

- **Main Albums:** studio albums and standard releases
- **Compilation Albums:** greatest hits and collections
- **Live Albums:** concert recordings
- **Singles & EPs:** singles and short releases
- **All Albums:** every album type
- **Top Songs:** the artist's most popular tracks
- **Music Videos:** the artist's music videos

You can tick several. MeedyaDL then makes one download for each kind you ticked. Leave all of them empty to use the download tool's own default. This needs GAMDL 3.0 or later.

### Download Notifications

When a download finishes, fails, or finishes with warnings, MeedyaDL shows a message inside the app naming it, for example "Daft Punk — Discovery finished." A failed download says in a few plain words what probably went wrong. When the whole queue is done, you see how it went, for example "Queue finished: 5 downloaded, 1 failed."

By default your computer also shows these as system notifications, but only while MeedyaDL is not in front (behind another app, minimised or hidden in the tray) or while your pointer is on a different screen. The first download to finish is shown straight away, by name. If more finish or fail within the next ten seconds, your system shows one notification that sums them up ("2 more downloads finished, 1 failed") instead of one each. Each of them still appears inside MeedyaDL. When the whole queue finishes, its summary covers any that were still waiting.

Choose when and for which messages in **Settings > General > Notifications**:

| Style | What it does |
|------|----------|
| **In-app only** | Messages inside MeedyaDL only |
| **System and in-app** (default) | System notifications as well as messages inside MeedyaDL |
| **System only** | A plain message your system has shown is not repeated inside MeedyaDL. Errors, warnings and anything the system was not asked to show still appear inside MeedyaDL |

You can also switch the system notification off for each kind of event (a download finished, a download failed, the queue finished, a copied link was found). The message inside the app still appears. See [Settings](settings.md) for every option. If notifications do not appear, see [Troubleshooting](troubleshooting.md#notifications-dont-appear).

### Quality for One Download

You can change the quality for one download without changing your defaults.

1. Paste your link into the download form as usual.
2. Open the **Quality Overrides** panel in the form.
3. Pick the audio format and/or video resolution you want.
4. Start the download.

That choice applies to this download only. The next one goes back to your defaults.

### When Your Format Isn't Available

When your preferred **format** isn't offered for a track or video, MeedyaDL moves to the next one on your fallback list. Audio formats and video formats each have their own list in Settings.

Video **resolution** works differently. It is a ceiling. If you ask for 4K and the video only goes up to 1080p, you get the 1080p version straight away, because nothing failed.

See [Fallback Quality](fallback-quality.md) for how the lists work.

---

## Which Quality Should I Choose?

### Our Top Recommendations

#### Best sound: ALAC (Lossless)

If you want the highest quality, an exact copy of the studio recording, choose **ALAC** and switch on **Enable Fallback Chain** in **Settings > Codec & Resolution**. ALAC keeps every detail and goes up to 24-bit/192 kHz. The fallback list makes sure that if lossless isn't offered for a track, MeedyaDL tries the next format, so you still get the track.

**Choose ALAC if** you use good speakers or headphones, or you want a lasting archive. Make sure you have the space. ALAC files are 2.5 to 7 times bigger than AAC.

#### Immersive sound: Dolby Atmos

If you want sound placed all around you, choose **Dolby Atmos** and switch on the fallback list. On AirPods Pro or Max, an Atmos soundbar or a compatible receiver, it feels much more spacious. Not every track has an Atmos mix, so the fallback list matters. When Atmos isn't offered, MeedyaDL tries AC3 (5.1 surround), then AAC Binaural (spatial for ordinary headphones), then AAC.

**Choose Atmos if** you have the right equipment. Consider a **Companion Download** of ALAC too, so every track also has a lossless copy.

#### Want both?

Set Dolby Atmos as your default with a Companion Download of ALAC. You get the spatial version where there is one, and a lossless copy of everything.

### Other Common Situations

- **Listening on a phone:** choose **AAC 256 kbps**. Files are small and the quality is excellent for most listeners, especially on the move.
- **Home audio system:** choose **ALAC 16-bit/44.1 kHz** (CD quality, about 5 MB a minute) or **ALAC 24-bit/96 kHz** if your equipment supports hi-res (about 10 MB a minute).
- **Home cinema without Atmos:** choose **AC3** for 5.1 surround.
- **Keeping a lasting archive:** choose **ALAC 24-bit/192 kHz** (about 15 MB a minute). You can always convert it to a smaller format later.
- **Spatial sound on ordinary headphones:** choose **AAC Binaural**.
- **Watching videos:** choose **1080p**. Most screens don't gain from 4K. Add HEVC if your devices support it for smaller files.
- **Little storage:** choose **AAC 256 kbps** and **720p**. An album is about 80 to 100 MB, and a music video about 50 to 75 MB.

---

## Related Topics

- [Downloading Music](downloading-music.md) — Audio download workflow
- [Downloading Videos](downloading-videos.md) — Video download workflow
- [Fallback Quality](fallback-quality.md) — Automatic quality fallback behavior
- [Lyrics and Metadata](lyrics-and-metadata.md) — How format choice affects metadata capabilities
- [Getting Started](getting-started.md) — Initial quality configuration

---

[Back to Help Index](index.md)
