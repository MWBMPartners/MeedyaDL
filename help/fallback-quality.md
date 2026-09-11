<!--
  MeedyaDL Help Documentation
  Copyright (c) 2024-2026 MeedyaSuite
  Licensed under the MIT License. See LICENSE file in the project root.
-->

# Fallback Quality

This guide explains how MeedyaDL's quality fallback system works when your preferred audio codec or video codec is not available for a particular piece of content.

---

## Overview

Not all content on Apple Music is available in every codec. For example, a track might be available in AAC but not in ALAC, or a music video might be offered in H.264 but not in the newer H.265 codec. The fallback quality system ensures that MeedyaDL can still download content even when your top preference is unavailable, by automatically trying the next best codec in a configurable chain.

There are two separate chains -- one for audio codecs, one for video codecs -- and they are stepped through in two different ways:

- **Audio.** When GAMDL (the download tool MeedyaDL drives) reports a codec as unavailable for a track, MeedyaDL itself restarts GAMDL, asking for the next codec in your chain. This repeats, moving down the chain, until either a codec succeeds or the chain is exhausted.
- **Video.** GAMDL walks your whole video codec chain itself, inside one single run, and downloads the video using the first codec it is actually offered in. MeedyaDL never has to restart anything for a music video -- the tool does the stepping-down on its own. See [Video Codec Fallback Chain](#video-codec-fallback-chain) below for what this means in practice.

Video resolution is a different kind of setting entirely -- it is a ceiling, not something with a fallback chain. See [Why There Is No Resolution Fallback Chain](#why-there-is-no-resolution-fallback-chain) for the full explanation.

---

## How Fallback Chains Work

### The Concept

A fallback chain is an ordered list of codecs -- one list for audio, a separate list for video -- that MeedyaDL (for audio) or GAMDL (for video) will try, in sequence, when your preferred codec is not available for a given track or video. Your preferred codec is always the first item in the chain. If it is unavailable, the next item is tried, then the next, and so on.

```
Preferred: ALAC
    |
    v  (codec unavailable)
Fallback 1: Atmos
    |
    v  (codec unavailable)
Fallback 2: AC3
    |
    v  (codec unavailable)
Fallback 3: AAC
    |
    v  (available!)
Downloaded in: AAC
```

For audio, only a "codec unavailable" error triggers this. Other error types are handled differently:

- **Network errors** -- Automatic retry. MeedyaDL tries the download up to four times in total (the first attempt plus three retries), one straight after another, before reporting it as failed.
- **Authentication errors** -- Require a cookie refresh; no fallback attempted
- **Not found errors** -- The content does not exist; no fallback attempted
- **Rate limit errors** -- No automatic retry. Apple Music limits how many licence requests one account can make, and the block lasts hours rather than minutes. MeedyaDL marks the item failed and pauses the whole queue so it stops making the block worse. Files already downloaded are kept, so when you resume from the Queue page later only the missing tracks are fetched.

### Audio Fallback Chain

The default audio fallback chain, in order of priority, is:

1. **ALAC** -- Apple Lossless Audio Codec (lossless)
2. **Atmos** -- Dolby Atmos spatial audio
3. **AC3** -- Dolby Digital surround sound
4. **AAC Binaural** -- AAC with binaural spatial rendering
5. **AAC** -- Standard AAC encoding
6. **AAC Legacy** -- Legacy AAC encoding (widest compatibility)

MeedyaDL attempts each codec in this order. If your preferred codec (the first item) is unavailable for a track, it tries the next codec in the chain, continuing until a successful download or the end of the chain is reached.

### Video Codec Fallback Chain

Apple Music offers each music video in one or both of two codecs. The default order MeedyaDL tries them in is:

1. **H.265 (HEVC)** -- smaller files for the same picture quality, and the only codec Apple Music offers above 1080p.
2. **H.264 (AVC)** -- plays on almost anything, but Apple Music never offers it above 1080p.

Unlike the audio chain, MeedyaDL does not restart the download for each codec in turn. GAMDL walks your whole codec list itself, inside a single run, and downloads the video using the first codec it is actually offered in. If the video is offered in H.265, that is what you get. If it is not, GAMDL moves on to H.264 within that same run.

If a video is not offered in any codec you have allowed, the download fails and MeedyaDL reports it as not available -- there is nothing left to fall back to unless you add another codec to the chain.

Because H.265 is the only codec offered above 1080p, a chain with H.265 removed effectively caps every video at 1080p, no matter what resolution ceiling you have set. Removing H.265 from your chain is as much a "never above 1080p" choice as it is a codec preference.

### Why There Is No Resolution Fallback Chain

There used to be a resolution list here too, ordered from 2160p (4K) down to 240p, working the same way the two codec lists work. It has been removed, because it never actually did anything -- and it is worth explaining why, so the removal makes sense rather than looking like a missing feature.

The video resolution you choose in Settings is a **ceiling**, not a request. Apple Music does not answer "yes" or "no" to a specific resolution the way it does to a specific codec. Instead, within whichever codec was chosen, Apple Music hands back the best quality it has at or below your ceiling, in the same single attempt. If a video only exists above your ceiling, you still get it -- at the lowest quality that is above the ceiling, since that is the closest thing available.

That means a resolution can never be "unavailable" in the way a codec can be. There is nothing that fails, so there is nothing for MeedyaDL to notice and step down through. A resolution fallback list looked exactly as meaningful as the two codec lists sitting next to it, right up until you checked what happened when you reordered it: nothing. So it has been taken out, and the resolution setting itself is described honestly, elsewhere in Settings, as a maximum rather than a request.

---

## Configuring Fallback Priorities

### Accessing Fallback Settings

To configure the fallback chains:

1. Open **Settings** from the application menu or toolbar.
2. Navigate to **Codec Fallback Order** in the sidebar.
3. Use the **Audio Fallback** / **Video Fallback** buttons at the top of the page to switch between the two chains. Each is edited the same way:
   - **Audio Fallback** -- The ordered list of audio codecs used for fallback.
   - **Video Fallback** -- The ordered list of video codecs used for fallback.

Like every other setting on this screen, changes here only take effect once you click **Save Changes** -- reordering or editing a chain by itself only changes what you see on screen, not what MeedyaDL actually uses.

### Reordering the Fallback Chain

You can fully customize the order and contents of each fallback chain:

- **Reordering** -- use the up (↑) and down (↓) arrow buttons on the right of each row. The first item in the list is your preferred codec; all subsequent items are tried in order if the preferred codec is unavailable.
- **Removing a codec** -- click the × button on the right of the row to drop it from the active chain. The removed entry moves to the **Available (not in chain)** panel below the active list. Removing an option means it will never be used as a fallback, even if it is the only available codec for a given track or video -- useful for users who don't want, say, AAC Binaural mixed into their library, or who never want an H.264 music video (since removing H.264 means a video offered only in H.264 will simply fail rather than download in that codec).
- **Adding a codec back** -- click the + button next to any entry in the **Available** panel to append it to the bottom of the active chain (lowest priority). Use the up arrow to move it earlier if needed.
- **Safety guard** -- the × button on the only remaining row of a chain is disabled, so you can never remove every codec from either list through the settings screen. For audio, this matters because an empty chain really would leave nothing to try -- every download in that category would fail. Video is different: if its chain is ever empty for some other reason (for example, an old settings file being read in), MeedyaDL quietly falls back to the tool's own recommended codec order instead of failing -- but the × button still stops you emptying it by hand, so what you see in the list is always what is actually used.

Your customized chains persist across application restarts once you click **Save Changes** -- they are not saved automatically as you edit them.

### Disabling Fallback

The **Enable Fallback Chain** switch, in **Settings > Codec & Resolution**, covers songs and music videos together with a single toggle. Turning it off means only your preferred codec is tried, and nothing steps down further:

- For audio, if your preferred codec is available, the download proceeds normally; if it is unavailable, the download fails with a codec error and MeedyaDL does not try the rest of the chain.
- For video, GAMDL is only given your single preferred codec, instead of the whole chain, so it cannot step down to a second choice on its own either.

This is useful when you require an exact codec match and would rather have a failed download than a different codec.

---

## Fallback Behavior by Content Type

### Songs and Albums

Fallback is applied **per-track**, not per-album. When downloading an album or playlist, each individual track is independently subject to the fallback chain. This means:

- Some tracks in an album may download in your preferred codec (e.g., ALAC) while others fall back to a different codec (e.g., AAC), depending on what is available for each track.
- The download queue reflects the actual quality used for each track individually.
- A single album download may result in a mix of codecs across its tracks if availability varies.

This per-track approach maximizes the number of successfully downloaded tracks rather than failing the entire album when one track lacks your preferred codec.

See [Downloading Music](downloading-music.md) for general audio download information.

### Music Videos

Video does step down through codecs, the same way audio does -- GAMDL walks your video codec chain in one run and uses the first codec the video is actually offered in, as described in [Video Codec Fallback Chain](#video-codec-fallback-chain) above. What does **not** step down is resolution: the resolution you choose is a ceiling, and Apple Music always gives you the closest quality at or below it, within whichever codec was chosen, in that same single attempt. See [Why There Is No Resolution Fallback Chain](#why-there-is-no-resolution-fallback-chain) for why a resolution is never "unavailable" the way a codec can be.

See [Downloading Videos](downloading-videos.md) for general video download information.

---

## Notifications and Logging

### Fallback Notifications

MeedyaDL keeps you informed when an **audio** fallback occurs:

- **Fallback indicator badge** -- In the download queue UI, any track that used a fallback displays a fallback indicator badge. This badge shows both the originally requested codec and the codec that was actually used (e.g., "Requested: ALAC | Downloaded: AAC").
- **Per-track visibility** -- Because fallback is per-track, you can scan the download queue to see exactly which tracks downloaded at your preferred codec and which required a fallback.
- **Queue summary** -- The download queue provides an at-a-glance summary of fallback activity for the current session.

A music video never shows this badge, even when GAMDL had to step down from H.265 to H.264. That is not an oversight -- GAMDL chooses the codec for a music video by itself, inside one run, and does not report back to MeedyaDL which codec it ended up picking. MeedyaDL has nothing to compare against "requested", so there is nothing honest it could put on a badge.

### Log File Details

Fallback decisions are logged for troubleshooting purposes where MeedyaDL itself does the stepping-down (audio). The logs record:

- Which codec was originally requested for each track.
- Each fallback attempt, including which codec was tried and whether it succeeded or failed.
- The final codec used for the successful download, or the full chain of failures if all options were exhausted.

When troubleshooting quality-related issues, check the download queue's error and status messages to see which codec was attempted and which succeeded. See [Troubleshooting](troubleshooting.md) for log file locations and how to read them.

---

## Examples

### Example 1: Audio Fallback

A user requests ALAC for a 6-track album. Here is what happens during the download:

1. **Tracks 1-5** -- ALAC is available. All five tracks download successfully in ALAC. No fallback is needed.
2. **Track 6** -- ALAC is not available. MeedyaDL moves down the fallback chain:
   - Tries **Atmos** -- codec unavailable.
   - Tries **AC3** -- codec unavailable.
   - Tries **AAC** -- available! Track 6 downloads successfully in AAC.
3. **Result** -- The album download completes with tracks 1-5 in ALAC and track 6 in AAC. The download queue shows a fallback indicator badge on track 6, indicating "Requested: ALAC | Downloaded: AAC".

### Example 2: A Music Video

A user's video codec chain is the default -- H.265 first, then H.264 -- and their resolution ceiling is set to 2160p (4K). They queue a music video that Apple Music offers only in H.264, up to 1080p. Here is what happens, all inside GAMDL's single attempt:

1. GAMDL looks for the video in **H.265** -- not offered.
2. It moves to **H.264** -- offered.
3. Within H.264, it asks Apple Music for the closest quality at or below the **2160p ceiling**. The video only goes up to 1080p, so that is what comes back.
4. **Result** -- the video downloads in H.264 at 1080p. The queue shows no fallback badge for this track, because MeedyaDL did not do the stepping down itself and GAMDL does not report back which codec it picked (see [Fallback Notifications](#fallback-notifications) above).

**What if H.264 had been removed from the chain?** With only H.265 left in the chain, and the video not offered in H.265, the download would fail outright -- reported as not available, with no further codec left to try. The resolution ceiling would never even come into play, because the failure happens at the codec step, before a resolution is ever chosen.

---

## Related Topics

- [Quality Settings](quality-settings.md) -- Full details on all available quality options
- [Downloading Music](downloading-music.md) -- Audio download workflow
- [Downloading Videos](downloading-videos.md) -- Video download workflow
- [Troubleshooting](troubleshooting.md) -- Resolving quality-related download issues

---

[Back to Help Index](index.md)
