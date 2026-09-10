---
name: project-video-codec-stepdown
description: How music video quality actually works — the codec steps down natively, the resolution is a ceiling that never fails — and the mistake of concluding otherwise
metadata:
  type: project
---

# Music video quality: what steps down and what does not

This is written down because I got it wrong once, in a commit that removed
correct help text on the strength of the wrong conclusion (`026dee36`,
corrected in #1176).

## The two halves, and they behave differently

**The video CODEC steps down, and the download tool does it itself.** Given an
ordered list of video codecs it walks them in order and downloads the first one
the video is offered in — all inside a single run. If none is offered it
reports the video as not available. This is genuinely the same idea as the
audio codec chain, which is why the video codec order now lives beside it in
Settings > Codec Fallback Order.

There are exactly two codecs: **H.265**, which gives smaller files at the same
quality and is the only one offered above 1080p, and **H.264**, which plays on
almost anything and is never offered above 1080p. A list without H.265
therefore caps every video at 1080p.

The tool also accepts a third value, `ask`, which opens a picker in a terminal.
MeedyaDL runs the tool with no terminal, so `ask` could only fail or hang. It
is deliberately not offered and is stripped if it appears in a settings file.

**The RESOLUTION is a ceiling and can never fail.** Within whichever codec is
chosen, the tool takes the closest quality at or below your ceiling. If the
video only exists above your ceiling you still get it, at the lowest quality
above. So a resolution is never "unavailable", and a list of resolutions to
step down through cannot change any outcome. There used to be one in Settings.
It was read by no backend code at all.

Note the direction carefully, because I wrote it backwards once: the
fall-back-to-lowest-above happens when nothing exists at or **below** the
ceiling — not "at this resolution or higher".

## Why MeedyaDL does not add its own retry for video

It would be pointless. The tool already walks the whole list in one run and
only reports failure after every codec has been tried. Retrying one codec at a
time would send the same list in slices and get the same answer more slowly —
precisely the kind of feature that looks like it works and changes nothing,
which this project has shipped too many times already.

What was needed instead was a **guard**: the retry loop walks the AUDIO chain
and had no idea what it was retrying, so a music video that could not be
downloaded was re-run with a different audio codec — up to five times, with
the log saying "trying atmos" for a video, downloading nothing.

## Where to check this rather than trusting anyone

`gamdl/interface/music_video.py` in the installed copy. Note the app data
folder still uses the OLD identifier in its path: `io.github.meedyadl`, not
`com.meedyasuite.meedyadl`. Looking under the new one finds nothing and makes
it look as though the tool is not installed.

- `get_stream_info()` — the loop over the codec list
- `_get_video_playlist_from_resolution()` — returns nothing when no stream
  matches the codec, and the sort key that makes the resolution a ceiling

Related: [[project-never-worked-pattern]], [[project-comment-accuracy-hazard]]
