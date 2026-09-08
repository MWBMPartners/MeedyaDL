---
name: project-apple-music-safari-identity
description: Why MeedyaDL always tells Apple Music it is Safari, from every platform — a deliberate choice, not a bug
metadata:
  type: project
---

**MeedyaDL sends a Safari identity to Apple Music from every platform** — Windows,
Linux and Raspberry Pi included, not just macOS. There are six places that do this and
none of them checks which operating system is running. **This is deliberate. Do not
"fix" it.**

## The two reasons

1. It is the identity Apple Music's servers expect, so requests get a normal response
   rather than being refused.

2. **The important one, and the one that keeps getting forgotten:** Safari implies a
   Mac, and Apple Music serves its fullest feature set to a Mac running Safari.
   Presenting as anything else — even something genuine and platform-appropriate like
   Chrome on Windows — can mean a reduced experience: fewer or lower-quality assets,
   and features simply not offered to a non-Apple client. Sending Safari from every
   platform is how a Windows or Raspberry Pi user gets the same Apple Music experience
   a Mac user does.

Confirmed by the maintainer on 2026-09-08.

## Why this note exists

The reason had never been written down. The code comment only said it avoids being
refused, which reads like a workaround rather than a product decision. Anyone auditing
the code — or any tool that flags "the browser identity does not match the operating
system" — would reasonably conclude it was a leftover and make it per-platform. That
would quietly degrade the app for every non-Mac user, and nothing would fail loudly to
say so.

## What IS worth changing

The **version number** in the string is a separate matter. It said Safari 17.6 (mid
2024) well into September 2026, while current Safari was 26.6. That staleness is worth
fixing, and the agreed approach is to read the version on the **build** machine and
inject it, the same way the Chrome version already is.

Two approaches were rejected, for reasons worth keeping:

- **Do not fetch it over the network at build time.** A failed or rate-limited fetch
  could ship a build Apple Music refuses outright. (This objection stands and predates
  the rest of this note.)
- **Do not read the user's own Safari version at run time.** It only works on Macs, so
  the fleet would split into "Macs sending today's version" and "everyone else sending
  an old one", which makes the old one more conspicuous rather than less. It also adds
  ways to fail on someone else's computer for no benefit. The version number is not
  what identifies the app anyway.

One earlier argument against refreshing it was simply wrong and should not be reused:
that Safari changes about once a year so staleness hardly matters. The gap reached
nine major versions.

Tracked in issue #1072. See also [[feedback-plain-english-always]].
