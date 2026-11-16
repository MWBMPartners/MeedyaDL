<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root.
-->

# Fallback Quality

Not every song or video is offered in every format. This page explains what MeedyaDL does when your first choice is not available, and how to change the order it tries.

---

## The Short Version

- You pick a **preferred codec** (a format, such as ALAC or H.265).
- If a track or video is not offered in it, MeedyaDL tries the **next codec on your list**, then the next, until one works.
- There are **two lists**: one for audio and one for video.
- Video **resolution** has no list. It is a ceiling, and you always get the closest quality at or below it.
- You can reorder the lists, remove codecs you never want, or turn fallback off.

---

## Overview

A track might be offered in AAC but not in ALAC. A music video might be offered in H.264 but not in the newer H.265. Fallback lets MeedyaDL still download the item, using the next best format on your list, instead of failing.

The two lists work in slightly different ways:

- **Audio.** If a track is not offered in a codec, MeedyaDL starts again and asks for the next codec on your list. It repeats until one works or the list runs out.
- **Video.** The download tool goes through your whole video list in one go and uses the first codec the video is offered in. MeedyaDL does not need to start again. See [Video Codec Fallback Chain](#video-codec-fallback-chain).

---

## How Fallback Chains Work

A **fallback chain** is an ordered list of codecs. The first one is your preferred codec. The others are tried in order only when the one before is not available.

```text
Preferred: ALAC
    |
    v  (not available)
Fallback 1: Atmos
    |
    v  (not available)
Fallback 2: AC3
    |
    v  (not available)
Fallback 3: AAC
    |
    v  (available!)
Downloaded in: AAC
```

For audio, only "this codec is not available" moves to the next codec. Other problems are handled differently:

- **Network errors:** MeedyaDL tries again automatically, up to four tries in total (the first one plus three more), before it reports a failure.
- **Sign-in problems:** You need to refresh your cookies. No fallback is tried. See [Cookie Management](cookie-management.md).
- **Not found:** The content does not exist. No fallback is tried.
- **Rate limits:** Apple limits how many requests one account can make, and the block can last for hours. MeedyaDL marks the item as failed and pauses the whole queue so it does not make things worse. Files already downloaded are kept. When you resume from the Queue page later, only the missing tracks are fetched.

### Audio Fallback Chain

The default audio order is:

1. **ALAC** — Apple Lossless (full quality)
2. **Atmos** — Dolby Atmos spatial audio
3. **AC3** — Dolby Digital surround sound
4. **AAC Binaural** — AAC with a headphone-style spatial effect
5. **AAC** — Standard AAC
6. **AAC Legacy** — Older AAC (plays almost anywhere)

If the first codec is not offered for a track, MeedyaDL tries the next, and so on to the end.

### Video Codec Fallback Chain

Apple Music offers each music video in one or both of two codecs. The default order is:

1. **H.265 (HEVC)** — Smaller files for the same picture, and the only codec Apple Music offers above 1080p.
2. **H.264 (AVC)** — Plays on almost anything, but is never offered above 1080p.

If the video is offered in H.265, that is what you get. If not, the download tool moves on to H.264 in the same attempt.

If the video is not offered in any codec still on your list, the download fails and is reported as not available. There is nothing left to try unless you add another codec back.

Because only H.265 is offered above 1080p, **removing H.265 from your list means no video is ever higher than 1080p**, whatever resolution ceiling you set.

### Why There Is No Resolution Fallback Chain

The resolution you choose in Settings is a **ceiling**, not a request. You get the best quality available at or below your ceiling, in a single attempt.

So a resolution can never be "not available" the way a codec can. Nothing fails, so there is nothing to step down from. Reordering such a list would change nothing, so it was removed. The resolution setting is described as a maximum instead.

---

## Configuring Fallback Priorities

### Opening the Fallback Settings

1. Open **Settings**.
2. Choose **Codec Fallback Order** in the sidebar.
3. Use the **Audio Fallback** and **Video Fallback** buttons at the top to switch lists. You edit them the same way.

Changes only take effect when you click **Save Changes**. Editing a list changes what you see, not what MeedyaDL uses, until you save.

### Changing a List

- **Reorder.** Use the up and down arrows on each row. The first row is your preferred codec.
- **Remove a codec.** Click the × on its row. It moves to the **Available (not in chain)** panel below. A removed codec is never used, even if it is the only one on offer. This is useful if, for example, you do not want AAC Binaural in your library, or never want an H.264 video. (A video offered only in H.264 will then fail instead of downloading.)
- **Add a codec back.** Click the + next to it in the **Available** panel. It goes to the bottom of the list. Use the up arrow to move it higher.
- **You cannot empty a list.** The × on the last remaining row is switched off. What you see is always what is used.

<details>
<summary>More detail: empty lists</summary>

An empty audio list would leave nothing to try, so every audio download would fail. That is why the settings screen stops you from emptying it. An empty video list is handled more gently: if it is ever empty for some other reason (for example an old settings file), MeedyaDL uses the recommended video order instead of failing.

</details>

### Turning Fallback Off

Use **Enable Fallback Chain** in **Settings > Codec & Resolution**. One switch covers songs and music videos. When it is off, only your preferred codec is tried:

- **Audio:** If your preferred codec is available, the download works. If not, it fails with a codec error. The rest of the list is not tried.
- **Video:** Only your preferred codec is requested, so it cannot step down to a second choice.

Turn it off when you want an exact codec and would rather have a failed download than a different format.

---

## Fallback by Content Type

### Songs and Albums

Fallback happens **per track**, not per album. Each track is checked on its own, so:

- Some tracks in an album may arrive in your preferred codec (for example ALAC) and others in a fallback (for example AAC).
- The queue item shows **Fallback used** with the format that was used.
- One failing codec never fails the whole album.

See [Downloading Music](downloading-music.md).

### Music Videos

Video steps down through codecs, as described in [Video Codec Fallback Chain](#video-codec-fallback-chain). Resolution does not step down. It is a ceiling, and you always get the closest quality at or below it. See [Why There Is No Resolution Fallback Chain](#why-there-is-no-resolution-fallback-chain).

See [Downloading Videos](downloading-videos.md).

---

## Notifications and Logging

### Fallback Notifications

When an **audio** fallback happens, the queue shows it:

- **Fallback used.** The queue item shows **Fallback used** with the format that was actually used, for example "Fallback used (codec: aac)".

MeedyaDL cannot tell which video format was used, so it shows no badge.

### Log Details

For audio, fallback decisions are written to the activity log. It records:

- The codec first requested for each track.
- Each fallback attempt, and whether it worked.
- The codec finally used, or the list of failures if every option ran out.

If you are checking a quality problem, look at the queue's status and error messages and at the activity log. See [Troubleshooting](troubleshooting.md).

---

## Examples

### Example 1: Audio Fallback

You ask for ALAC on a 6-track album.

1. **Tracks 1-5:** ALAC is available. They download in ALAC.
2. **Track 6:** ALAC is not available. MeedyaDL tries **Atmos** (not available), then **AC3** (not available), then **AAC** (available).
3. **Result:** Tracks 1-5 are ALAC and track 6 is AAC. The queue item shows **Fallback used** with the format that was used.

### Example 2: A Music Video

Your video list is the default (H.265, then H.264) and your resolution ceiling is 2160p (4K). You queue a video that Apple Music offers only in H.264, up to 1080p.

1. H.265 is tried first. It is not offered.
2. H.264 is tried next. It is offered.
3. Within H.264, you get the closest quality at or below 2160p. The video only goes up to 1080p, so that is what you get.
4. **Result:** The video downloads in H.264 at 1080p. No **Fallback used** note appears (see [Fallback Notifications](#fallback-notifications)).

**What if H.264 had been removed from your list?** With only H.265 left and the video not offered in it, the download would fail as not available. The resolution ceiling never comes into it, because the failure happens when the codec is chosen.

---

## Related Topics

- [Quality Settings](quality-settings.md) — All the quality options
- [Downloading Music](downloading-music.md) — Audio downloads
- [Downloading Videos](downloading-videos.md) — Video downloads
- [Troubleshooting](troubleshooting.md) — Fixing quality-related problems

---

[Back to Help Index](index.md)
