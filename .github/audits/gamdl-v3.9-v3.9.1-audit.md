# GAMDL v3.9 + v3.9.1 Compatibility Audit — DECISION: 3.9 REFUSED, 3.9.1 ADMITTED (ceiling → 3.9.1; Windows on ARM64 held at 3.8.5)

**Date**: 2026-09-22
**GAMDL releases audited**: 3.9 (tagged 2026-09-21, **never admitted**) and 3.9.1 (tagged 2026-09-22, **admitted**)
**Diff range**: `3.8.5..3.9.1` — 12 commits, 17 files, +1479 / −102 lines
**Predecessor audit**: [`gamdl-v3.8.5-audit.md`](./gamdl-v3.8.5-audit.md)
**MeedyaDL branch audited**: `work/after-1.10.8`, tested ceiling 3.8.5 at the start of this audit

## TL;DR

**GAMDL 3.9 must never be admitted. It cannot download Apple's "web" AAC
formats at all.** 3.9 changed how GAMDL finds the copy-protection key it needs
to unlock a track — the technical term is a DRM key, DRM being the general
name for the copy-protection system built into a streaming service (GAMDL's
own name for the specific system Apple Music uses is FairPlay). Where 3.8.5
simply took the first key a track's playlist offered, 3.9 insists on a key
carrying one particular label. Apple's web AAC streams present a key with no
label at all, so the lookup comes back empty and the download stops with a
"decryption not available" error. That is exactly the two formats MeedyaDL
calls AAC Legacy and AAC-HE Legacy (GAMDL's own names for them, `aac-web` and
`aac-he-web`, are new as of GAMDL 3.6) — and those two sit at the very bottom
of MeedyaDL's default audio fallback chain (ALAC → Atmos → AC3 → AAC Binaural
→ AAC → AAC Legacy). That chain is the safety net every other codec falls
back to when it isn't available, so a user on 3.9 would watch the whole chain
walk to the bottom and fail there — which looks exactly like "this track just
isn't available" rather than what it actually is, a defect in GAMDL. It also
breaks MeedyaDL's gap-fill retry, which deliberately reaches for this same
last-resort codec.

**GAMDL 3.9.1 fixes it, the next day, and is admitted.** The tested ceiling
and the recommended version both move to 3.9.1. 3.9.1 restores the old
behaviour as a fallback: look up a key by the expected label first, and if
that finds nothing, fall back to whichever key came first — exactly what
3.8.5 always did.

**Windows on ARM64 is held at 3.8.5.** GAMDL 3.9 added a genuine, unavoidable
dependency on a package called `pyplayready` (needed for the new PlayReady
support described below). That package in turn requires a package called
`cryptography` at a version that, as things stand today, publishes no build
at all for Windows on ARM64. GAMDL's own package for 3.9.1 installs fine
there — the missing piece is two layers further down, where MeedyaDL's
installer cannot see it and cannot make pip resolve around it automatically.
Installing 3.9.1 on Windows ARM64 would try to compile that missing package
from source, which needs a Rust compiler MeedyaDL's managed Python does not
carry, and the install would fail outright — on a first-time setup, that
means the app cannot finish setting itself up on that one platform at all.

**Linux on 32-bit ARM (ARMv7) stays at 3.8.1, exactly as before.** This is not
a new decision — GAMDL has never published a 32-bit ARM package for anything
newer than 3.8.1, so MeedyaDL's installer has always fallen back to that
version there by itself, for the same underlying reason (a missing package
somewhere in the chain), just one layer shallower.

**Everything else upstream changed is genuinely useful and, for the most
part, arrives for free**: a second way of unlocking protected content
(PlayReady, alongside the existing Widevine), fuller metadata for singles and
music videos pulled in from Apple's iTunes lookup service when the richer
catalogue response leaves a field blank, better handling of which audio track
a music video picks when the best one isn't offered, and a fix so two
different music videos that would have landed on the same filename no longer
overwrite one another. None of it needs a new MeedyaDL setting to take
effect, and none of it is dangerous — but two of these upstream changes touch
places where MeedyaDL already does its own work, and those are covered in
detail below because they are the parts worth watching.

## Methodology

The same six MeedyaDL integration surfaces checked in every audit since
v3.7.4 were checked again here, plus two more this release specifically
touches:

1. `src-tauri/src/models/gamdl_options.rs` — the command-line flags MeedyaDL
   builds, and the `SongCodec` traits that describe each codec's behaviour.
2. `src-tauri/src/services/config_service.rs` — the settings MeedyaDL writes
   into GAMDL's own configuration file.
3. `src-tauri/src/services/download_queue.rs` — the code that actually runs
   GAMDL, reads its output line by line, and decides what MeedyaDL does next.
4. `src-tauri/src/services/gamdl_capabilities.rs` — the single place that
   knows which GAMDL feature is available on which installed version.
5. `src-tauri/src/services/gamdl_service.rs` — how MeedyaDL installs and
   upgrades GAMDL itself.
6. `src-tauri/src/utils/process.rs` — the patterns MeedyaDL uses to recognise
   a line of GAMDL's output and decide what kind of message it is.
7. `src-tauri/src/services/metadata_tag_service.rs` — MeedyaDL's own
   after-download metadata enrichment, checked here because this release
   changes what GAMDL itself writes into the same files first.
8. `src-tauri/src/services/update_checker.rs` — the logic behind the
   in-app "you can upgrade" and "this version is untested" messages.

Verified independently for this document, not merely taken from the working
draft: the exact commit set and file/line counts between the `3.8.5` and
`3.9.1` tags; the release dates of both tags (read from git, not estimated);
the wheel files GAMDL itself publishes for 3.8.5, 3.9 and 3.9.1; the
dependency `pyplayready` pulls in and the version range it demands for
`cryptography`; and, file by file, exactly which published `cryptography`
releases carry a Windows-on-ARM64 package. All of that is reported as
confirmed below. A short list of things that could **not** be independently
confirmed — because doing so would need a real download against Apple Music,
or a real Windows-on-ARM64 machine — is kept together near the end, rather
than folded quietly into the findings, so nothing here reads as more certain
than it is.

**One correction to the working draft this document replaces**: the draft
states that 3.9.1 followed 3.9 "three days later". Git's own tag timestamps
say otherwise — 3.9 was tagged 2026-09-21 17:20 (UTC−3) and 3.9.1 was tagged
2026-09-22 09:45 (UTC−3), about sixteen hours apart, the next day rather than
three days on. It changes nothing about the decision, but the record should
say what actually happened.

## The commits

**In 3.9** (9 commits):

| Commit | Subject | Files touched |
|---|---|---|
| `76975c0` | Improve playlist generation | `downloader/base.py`, `downloader/constants.py`, `downloader/downloader.py`, `interface/base.py` |
| `a3df8eb` | Add PlayReady DRM support | `README.md`, `api/apple_music.py`, `cli/cli.py`, `cli/cli_config.py`, `interface/base.py`, `interface/enums.py`, `interface/music_video.py`, `interface/song.py`, `pyproject.toml`, `uv.lock` |
| `c89b001` | Document DRM backend requirements | `README.md` only |
| `85a6fb5` | Fill missing album metadata from iTunes | `interface/base.py` |
| `eee2351` | Fix music video sort metadata | `interface/music_video.py` |
| `463f280` | Fill missing track and disc metadata | `interface/base.py` |
| `598c88d` | Improve music video handling | `README.md`, `cli/cli.py`, `cli/database.py`, `downloader/music_video.py`, `interface/constants.py`, `interface/music_video.py` |
| `bac95b0`, `24409b9` | Version bump to 3.9 | metadata only |

**In 3.9.1** (3 commits, released the next day):

| Commit | Subject | Files touched |
|---|---|---|
| `8afc71d` | Document DRM backend options | `README.md` only |
| `d57f75d` | **Fix web AAC DRM handling** | `interface/base.py`, `interface/song.py` |
| `bc3bcd2` | Version bump to 3.9.1 | metadata only |

The shape of it: 3.9 on its own carries a defect in how it selects the
copy-protection key for ordinary "web" codec songs, and 3.9.1 is the
same-week fix. That is the whole argument for skipping straight to 3.9.1 and
never letting 3.9 itself reach a user.

## Findings

### 3.9-A — the web AAC DRM defect, and the 3.9.1 fix

This is the finding the whole decision turns on, so it is covered in full
here rather than split by which commit did what.

**How it broke.** In 3.8.5, `interface/song.py::_get_web_stream_info` read
the copy-protection key identifier off the **first** key listed in the
track's media playlist, regardless of what kind of key it was:

```python
stream_info.widevine_pssh = m3u8_obj.keys[0].uri
```

3.9 replaced that with a lookup **by label**: the identifier had to come from
a key explicitly marked with the Widevine format label
(`urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed`), or, on the new PlayReady
path, one marked `com.microsoft.playready`. Apple's web AAC playlists present
a key with **no label attached at all**, so that lookup found nothing. The
identifier came back empty, and a guard further down GAMDL's own code then
raised its "decryption not available" error. The song could not be
downloaded.

**The fix.** 3.9.1 restores the old behaviour, but only as a fallback: look
up a key with no label once, and use it for whichever backend is active when
the labelled lookup finds nothing.

```python
untyped_drm_uri = self._get_drm_uri_from_m3u8_keys(m3u8_obj, None)
stream_info.widevine_pssh = self._get_drm_uri_from_m3u8_keys(
    m3u8_obj, "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed",
) or untyped_drm_uri
```

Two supporting changes travelled in the same commit: the lookup helper
switched from a plain dictionary lookup (which raised an error when asked for
"no label") to `.get()`, so asking for the unlabelled key no longer crashes;
and a new helper, `reconstruct_playready_wrm_header`, was added because 3.9
fed the raw identifier straight into a PlayReady-specific parser that only
works on a full PSSH box — a bare 16-byte key identifier isn't one, so the
new helper builds the minimal document PlayReady expects around it by hand
when the value is exactly that short.

**Exactly which codecs this hits.** The broken code path only runs for
codecs marked as using Common Encryption (CENC) — the same key-labelling
scheme both Widevine and PlayReady use, as opposed to Apple's own FairPlay
scheme, which every other codec in GAMDL uses. Only two GAMDL codec names
carry that marking: `aac-web` and `aac-he-web`. On the current MeedyaDL
codebase, `SongCodec::AacLegacy` and `SongCodec::AacHeLegacy` are sent to
GAMDL under exactly those two names, on any installed GAMDL from 3.6 onward
(verified in `gamdl_options.rs`, gated by `GamdlFeature::AacWebCodecRename`).
Every other codec — ALAC, Atmos, AC3, and the other AAC variants — carries no
such marking and is unaffected.

**Why the position in the fallback chain matters more than the codec count.**
MeedyaDL's default audio fallback chain is ALAC → Atmos → AC3 → AAC Binaural
→ AAC → AAC Legacy (confirmed against `settings.rs`'s own default and its
test coverage). AAC Legacy — one of the two broken codecs — is the last
entry: the thing MeedyaDL tries once nothing else has worked. A user on 3.9
whose preferred codec wasn't available would have watched the whole chain
walk to the bottom and then fail there too, with nothing in the failure
message pointing at the real cause. The same logic hits MeedyaDL's gap-fill
retry (`build_gapfill_priority_chain()`), which strips wrapper-dependent
codecs and deliberately retries with what's left — including `aac-web`. On
3.9, that retry would also fail.

**Confidence.** This is read off the code and the fix commit, not reproduced
against a real Apple Music download — there is no way to fetch a real
playlist for this audit. The reasoning holds together tightly: 3.8.5 worked
by taking whichever key came first; 3.9 required a labelled key and broke;
3.9.1, released the next day and titled "Fix web AAC DRM handling", adds
back exactly an unlabelled fallback. The only way that fix makes sense is if
the web AAC key genuinely carries no label — but it was not observed
directly.

**The consequence for the support window is unambiguous: admit 3.9.1, and
never let 3.9 reach a user.** MeedyaDL's installer already resolves a bounded
range like `gamdl>=3.0,<=3.9.1` to the newest release in range when it
upgrades, so in the ordinary course of things nobody lands on 3.9 by
accident — but the defect is real and user-facing enough that it needed its
own explicit refusal rather than being left to chance. (The mechanism that
enforces this — a short list of known-bad GAMDL releases the installer
refuses to target, and that flags an already-installed 3.9 for the user —
lives in `gamdl_capabilities.rs`; how it works belongs to the "What MeedyaDL
changed" section below, once that is written up.)

### 3.9-B — a second way of unlocking protected content: PlayReady

GAMDL has always used a copy-protection system called Widevine to unlock
tracks. 3.9 adds a second one, PlayReady, as an alternative — Widevine stays
the default and nothing about it changes.

- A new setting, `--drm-backend`, chooses between `widevine` (the default,
  unchanged) and `playready`.
- A new setting, `--prd-path`, points at a device file the user must obtain
  themselves — GAMDL does not supply one, the same way it does not supply a
  Widevine device file (`.wvd`) by default either.
- Choosing `playready` without also supplying `--prd-path` makes GAMDL print
  a clear error and quit without downloading anything — it never gets far
  enough to reach a confusing failure.
- The debug log no longer prints the full response from Apple's licence
  exchange, or the copy-protection key itself, in plain text. A small, purely
  positive hardening change, unrelated to the feature itself.
- Per the README: a 4K music video needs either a Widevine device rated
  "L1" or a PlayReady device rated "SL3000" — MeedyaDL supplies neither
  today, so 4K music video download remains out of reach exactly as before.
  PlayReady doesn't close that gap by itself; it only offers a second route
  to the same kind of device requirement.

**Effect on a MeedyaDL user who changes nothing: none.** MeedyaDL never sends
`--drm-backend` or `--prd-path`, so GAMDL takes its own default (Widevine)
and behaves exactly as it did on 3.8.5.

**How it interacts with the wrapper.** GAMDL's wrapper protocol file
(`gamdl/api/wrapper.py`) is completely unchanged between 3.8.5 and 3.9.1, and
its version string is still `"0.0.2"` — so MeedyaDL's wrapper version check
and its mismatch guidance stay correct without any change. Per the README
and confirmed in the code: when the wrapper is switched on, any stream the
wrapper can unlock goes through the wrapper; only what the wrapper cannot
handle falls through to whichever backend was chosen. The wrapper still
comes first. PlayReady is an alternative fallback, not a replacement for the
wrapper — and the decision logic that chooses between them was rewritten,
but behaves identically to 3.8.5 for a user who has changed nothing, checked
across all four combinations of wrapper on/off and the two encryption
schemes.

Cookie handling is untouched anywhere in this diff.

### 3.9-C — playlist file generation (does not reach MeedyaDL)

`76975c0` reworks how GAMDL writes a playlist file when asked to: the file
extension changes from `.m3u` to `.m3u8`, a standard `#EXTM3U` header line is
now written first, the relative path from the playlist file to each track is
computed more robustly, and writing is now append-based for speed on long
playlists rather than rewriting the whole file for every track added. One
genuinely user-visible change travels with it: the `{playlist_track}`
template variable — the position of a track within its playlist — now counts
from 1 instead of from 0.

**None of this reaches a MeedyaDL user.** GAMDL only writes a playlist file
at all when told to with `--save-playlist`, and MeedyaDL never sets it — the
underlying setting (`GamdlOptions.save_playlist`) exists in the code but
nothing in MeedyaDL's settings, options-merging, or configuration-writing
code ever assigns it a value, so it stays unset and the flag is never
passed. `{playlist_track}` is also absent from MeedyaDL's own list of
template variables shown to users, so nobody could be relying on it. Worth
recording only so that if playlist-file generation is ever exposed in
MeedyaDL's own settings later, whoever does that remembers the numbering
changed underneath it.

### 3.9-D — filling in missing metadata from iTunes, and where it needs care

`85a6fb5` and `463f280` add a genuinely useful upstream improvement: when the
response GAMDL gets while downloading a track is missing a field, GAMDL now
looks the item up in Apple's public iTunes lookup service and fills the gap
from there, rather than leaving it blank. A companion change reads that
lookup result more carefully than before — it separates the "track" part of
the answer from the "collection" (album) part instead of blindly taking the
first result, which fixes a class of subtly wrong dates and counts.

**Which downloads this touches.** Tracing every call site: it affects every
**song** download, wrapper or not — not only the wrapper path, which was the
first assumption checked and found wrong. For **music videos**, it only
applies when the wrapper is switched on; without the wrapper, music videos
take a different code path that does not consult iTunes for these fields.

**The direction is safe: iTunes only fills a blank, it never overwrites
something already there.** Whatever the main response already supplied —
album name, artist, copyright, disc number, disc count, genre, track count —
is kept exactly as it was; iTunes is consulted only for the fields left
empty.

**Where files land can move — a genuinely useful change, but a quiet one.**
GAMDL decides which output folder and filename template to use for a track
based on one simple test: does it have an album name at all? Before this
release, a music video with no album context (a common case — MeedyaDL's own
Tier 4 safety-net naming exists precisely for this) had no album name and so
fell to that safety net: `{artist}/Music Videos/{title} ({title_id})`, where
the numeric ID at the end guarantees the file can never collide with another
video of the same name. From 3.9 onward, iTunes will often supply an album
name where none existed before, so that same video can now land inside the
ordinary album folder, under the ordinary filename template, **without** the
numeric-ID suffix. This is arguably the very thing MeedyaDL's own design
notes for music-video placement have wanted all along — a video landing
beside its album's audio tracks rather than off in its own folder — arriving
for free. But it moves files for existing users without any warning, and it
quietly drops the uniqueness guarantee those files used to have. GAMDL's own
new name-clash handling (3.9-E below) exists specifically to catch the
collision this creates. A second, quieter version of the same thing: a track
whose disc count is filled in from iTunes for the first time can switch from
a single-disc filename template to a multi-disc one — `01 Title.m4a` becomes
`1-01 Title.m4a` — so a re-download can produce a second file sitting beside
the first rather than replacing it.

**The part that needs real attention: MeedyaDL's own cross-contamination
guard can now fail silently on exactly the files this change touches.**
MeedyaDL already refuses to write its own richer Apple Music metadata onto a
file if that file doesn't look like it belongs to the album being enriched —
a defence against mixing up metadata between two albums downloading at once,
added under issue #452. The check is an exact, case-insensitive comparison
between the album name GAMDL wrote into the file and the album name
MeedyaDL's own Catalog API fetched independently (`metadata_tag_service.rs`,
the guard around lines 704–740). Before this GAMDL release, a single or an
album-less item simply had no album name in the file at all, so the
comparison fell through to a looser check (compare artist names instead)
and enrichment ran normally. From 3.9.1 onward, that same file now carries
an album name — but one **read from iTunes**, being compared against the
name MeedyaDL separately fetched from Apple's **Catalog** API. Those two
names don't always agree: iTunes commonly appends " - Single" or " - EP"
where the Catalog name has neither, and the two sometimes word an edition
differently, such as "(Deluxe Edition)" against "(Deluxe)". When they differ
by even one character, the comparison fails and MeedyaDL's entire layer of
richer metadata — the record label, the editorial notes, the content
rating, everything the guard protects — is silently skipped for that file.
The only trace it leaves is a line in the tracing log at warning level, not
anywhere a user would see it. The download still completes and looks
entirely normal; it simply ends up with plainer metadata than it should
have. This is scoped narrowly — an ordinary album download already had an
album name in the main response and is unaffected — but it is a real,
invisible regression path for singles, music videos, and personal-library
items. How often the two names actually disagree could not be measured for
this audit; doing that needs real downloads against both of Apple's APIs,
which was not available here.

### 3.9-E — music video handling: three changes

`eee2351` and `598c88d`, both under the same "improve music video handling"
banner:

1. **Sort-title and sort-album metadata removed on the non-wrapper path.**
   These two fields (used for correct alphabetical sorting when a title
   starts with something like "The") were added to GAMDL two releases ago
   and are now taken back out for music videos downloaded without the
   wrapper. **No effect on MeedyaDL**: MeedyaDL writes neither of these
   fields itself, so there is nothing for it to collide with — the same
   conclusion the audit for that earlier release reached, just running in
   the opposite direction this time. Wrapper users are unaffected either
   way; with the wrapper switched on, Apple's own response supplies these
   and several other fields GAMDL didn't have access to before.
2. **A proper fallback when the best audio track isn't offered.** Picking
   which audio track goes with a music video used to hard-code a single
   named option and give up entirely if that one wasn't present. There is
   now an ordered list of four options, each checked in turn, so a music
   video that only offers a lower-quality audio track now downloads
   successfully instead of failing outright.
3. **Two different music videos with the same name no longer overwrite each
   other.** Before writing a new music video file, GAMDL now checks the
   destination folder for any file with a matching name and, when one is
   found, works out whether that file is genuinely *this* video (so a
   re-download correctly reuses the same path) or a *different* one that
   happens to share a title — in which case the new file gets a number
   appended (`Name 2.m4v`, `Name 3.m4v`, and so on) instead of silently
   overwriting the earlier one.

**Does this collide with MeedyaDL's own renaming?** No. MeedyaDL renames
files after download to add codec suffixes (like `[Lossless]`) and content
advisory suffixes (like `[Explicit]`), but that renaming only ever runs on
audio files — music video files are left exactly as GAMDL wrote them
(confirmed against the `is_m4a()` gate in `metadata_tag_service.rs`). So
GAMDL's own name-matching sees exactly the filenames it produced, with
nothing from MeedyaDL in the way.

One small companion fix in the same commit set: a function that used to
crash outright when a requested key label was missing now returns nothing
instead, and the missing-key handling moved to where it properly belongs —
neither of which changes anything a MeedyaDL user would notice.

### 3.9-F — the command-line options MeedyaDL sends, and a gap found while checking (not caused by this release)

Comparing GAMDL's full list of command-line options at both tags: **3.9.1
adds exactly two options and removes none** — `--drm-backend` and
`--prd-path`, both covered under 3.9-B above. Nothing MeedyaDL currently
sends has been renamed or withdrawn by this release, and GAMDL's
configuration file gains no key MeedyaDL writes and loses none either — the
new options would appear there as `drm_backend` and `prd_path`, MeedyaDL
writes neither, and GAMDL already discards configuration-file keys it
doesn't recognise, so an older file stays valid regardless.

**A gap found while checking every option MeedyaDL can send against every
option GAMDL 3.9.1 still recognises — pre-existing, and not something this
release caused.** GAMDL removed two tool-path options, `--mp4box-path` and
`--mp4decrypt-path`, when it switched to muxing files natively (confirmed
present in GAMDL 3.2 and absent from every later release checked). MeedyaDL
has two separate places that can send those flags: one, in
`gamdl_options.rs`, is correctly switched off once the installed GAMDL no
longer accepts them; the other, a function called `inject_tool_paths` in
`gamdl_service.rs`, has **no such switch at all** — it sends both flags
whenever the user hasn't set a custom tool path and the managed copy of that
tool exists on disk, regardless of which GAMDL version is installed. That
function sits on the real download path. GAMDL declares its command
strictly, so sending it a flag it no longer recognises is a hard error, not
a warning.

**What could not be settled here: whether this actually fires today.** If it
did on every download, MeedyaDL would be visibly broken on 3.8.5 already,
and it plainly is not — so something must usually prevent it, most likely
the specific tool-path check finding nothing on disk in an ordinary install.
That could not be confirmed by reading the code alone, and running GAMDL to
observe it directly was not available for this audit. This is not a defect
introduced by 3.9 or 3.9.1 — the code has been like this since roughly
GAMDL 3.3 — but it surfaced while checking this release's option list, and
the earlier audit that fixed the equivalent problem elsewhere in the
codebase (the 3.7 audit) named three files to fix and missed this fourth
one. Recorded here as advisable follow-up A2, not blocking this release's
admission either way.

### 3.9-G — output parsing and error classification: unaffected

Every line GAMDL prints at its ordinary logging level is byte-identical
between 3.8.5 and 3.9.1 — checked against the diff of GAMDL's own
command-line entry point, which contains no change to any of the standard
"Starting", "Processing", "Downloading" or "Finished" lines MeedyaDL's
patterns match against. None of MeedyaDL's regular expressions or error
classifiers need any change.

Two new error messages exist in 3.9.1 that were checked and found
unreachable by MeedyaDL: the critical error for choosing PlayReady without a
device file (MeedyaDL never sets that option, so it can never trigger it),
and an internal check inside GAMDL's own playlist writer (which MeedyaDL
never invokes, per 3.9-C above). One error MeedyaDL already recognises
("decryption not available") simply moved to a slightly different place in
the code without changing its wording or meaning — already handled, nothing
to update.

Worth recording while checking this, though it predates this release
entirely: the literal text "Saved to" — which one of MeedyaDL's regular
expressions is written to match — does not appear anywhere in GAMDL 3.8.5 or
3.9.1. That expression has therefore already been dead on the currently
shipping version, and MeedyaDL already compensates for it with a separate
fallback that scans the output folder directly when GAMDL exits successfully
without printing any such line. Not a regression from this release, just a
fact worth having on record.

### 3.9-H — dependencies and the database option: confirmed unused, listed for completeness

3.9 adds one new direct dependency to GAMDL itself: `pyplayready`. That
package in turn pulls in a sizeable tree of its own — most of it already
present because of GAMDL's existing Widevine dependency, `pywidevine`, which
needs several of the same packages. The genuinely new arrival is a package
called `cryptography`, which is compiled rather than pure Python — this is
the root of the Windows-on-ARM64 problem covered in full in its own section
below.

`598c88d` also adds a new database index and lookup function GAMDL can use
when a `--database-path` is supplied, to help its new name-clash handling
(3.9-E above) recognise a previously downloaded video. Searching the whole
MeedyaDL codebase confirms `--database-path` is never sent to GAMDL —
MeedyaDL doesn't use that feature of GAMDL at all — so this new database
machinery never runs for a MeedyaDL user. Recorded for completeness only.

## Can GAMDL 3.9.1 actually be installed everywhere?

**No — this is the second major finding of the audit, and it is the reason
this release cannot be a plain, one-file ceiling bump.** Unlike the finding
above, this one is not visible anywhere in GAMDL's own published files — it
sits one level further down, inside a dependency of the new dependency.

### How MeedyaDL installs GAMDL, and why the two details below matter enormously here

MeedyaDL's installer runs, in essence, `pip install --upgrade
--only-binary=gamdl <version range>`. Two details of that command decide
everything that follows:

1. **`--only-binary=gamdl` names only the `gamdl` package itself.** Every
   package `gamdl` depends on is still free to be built from its source code
   if no ready-made package exists for the machine it's installing on. So a
   dependency with no ready-made package for a given platform does not
   produce a clean "this can't be installed here" message — it produces an
   attempt to compile it, silently, in the background.
2. **The version range comes from the global ceiling, not a per-platform
   one**, in the code MeedyaDL had before this audit. MeedyaDL already has a
   mechanism for pinning specific platforms to an older, known-good version
   — used today for 32-bit Linux ARM — but that mechanism only affected what
   the "is this update tested?" message told a user, not what the installer
   actually asked pip to install. This distinction is the entire reason
   32-bit Linux ARM has quietly worked correctly up to now while this new
   problem would not have: on that platform, GAMDL itself has no package to
   offer above 3.8.1, so pip has nothing to choose and falls back on its
   own, regardless of what MeedyaDL's ceiling said. On Windows ARM64, GAMDL
   **does** publish a working package for 3.9.1 — so pip has no reason to
   fall back to anything, and walks straight into the missing dependency
   three layers down instead.

MeedyaDL's own managed copy of Python is a genuine, separate build for each
processor architecture — including a dedicated one for Windows on ARM64 —
so pip running inside it correctly reports itself as a Windows-ARM64
installation and will only accept a package built for that exact
architecture, never one built for ordinary 64-bit Windows.

### GAMDL's own published files: identical, and that is the trap

Checked independently for this document: GAMDL 3.8.5, 3.9 and 3.9.1 all
publish an identical set of ready-made packages — one each for macOS
(covering both Intel and Apple Silicon in a single file), 64-bit Windows,
Windows on ARM64, 64-bit Linux, and 64-bit-ARM Linux, plus a source archive
as a last resort. **None of the three publishes anything for 32-bit ARM
Linux.** So looking at GAMDL alone, 3.9.1 looks like a completely free
upgrade on every platform that matters. That is exactly what makes the real
problem easy to miss.

### The new dependency, and the one underneath it

`pyplayready`, the package 3.9 adds, is itself pure Python and installs
anywhere without issue on its own. Its own requirements list a package
called `cryptography`, restricted to versions 45.0.6 or newer but strictly
below 46.0.0.

Checked independently for this document, file by file, against every
release `cryptography` has ever published on PyPI:

- Only two releases exist inside the range `pyplayready` allows: 45.0.6 and
  45.0.7. **Neither publishes a package for Windows on ARM64** — their
  Windows packages cover ordinary 32-bit and 64-bit Windows only.
- Windows-on-ARM64 packages of `cryptography` first appear at version 46.0.0
  and continue through 46.0.3 — **precisely the range `pyplayready`'s own
  requirement excludes.** (They then stop again at 46.0.4 and later, for
  reasons unrelated to this audit — not that it matters here, since
  `pyplayready` cannot use anything at or above 46.0.0 regardless.)

So on a Windows-on-ARM64 install, pip is left with no ready-made choice for
`cryptography` and falls back to compiling it from its published source
code. That needs a Rust compiler and certain system libraries, neither of
which exists inside MeedyaDL's managed, portable copy of Python. **The
install fails.**

### Per-platform verdict for GAMDL 3.9.1

| Platform | GAMDL package | `pyplayready` | `cryptography` (pinned to 45.0.7) | Result |
|---|---|---|---|---|
| macOS, Apple Silicon | yes | pure Python | ready-made package exists | **installs cleanly** |
| Windows, 64-bit | yes | pure Python | ready-made package exists | **installs cleanly** |
| **Windows, ARM64** | yes | pure Python | **no package — tries to compile** | **FAILS** |
| Linux, 64-bit | yes | pure Python | ready-made package exists | **installs cleanly** |
| Linux, ARM64 | yes | pure Python | ready-made package exists | **installs cleanly** |
| Linux, ARMv7 (32-bit) | **none published** | never reached | never reached | resolves down to 3.8.1, exactly as today |

### Why the existing 32-bit-ARM handling does not, on its own, cover Windows-on-ARM64

It's tempting to assume the existing per-platform mechanism already handles
this — after all, MeedyaDL already copes with a platform that can't take the
newest GAMDL. It does not, on its own, and the reason matters: 32-bit ARM
Linux has always worked by luck of the installer's own behaviour, not
because of anything MeedyaDL's per-platform ceiling actively did. Since
GAMDL itself has never published a package for that platform above 3.8.1,
pip has never had a choice to make — the per-platform ceiling entry has only
ever affected the wording of the "is this tested?" message shown to a user
on that platform, never what actually got installed. Windows-on-ARM64 is the
opposite situation: GAMDL **does** publish a working 3.9.1 package there, so
pip happily selects it and only then runs into the missing dependency.
Recording a lower ceiling for that platform changes nothing by itself unless
the part of the installer that decides what to ask pip for is also taught to
read it.

### What the failure would look like to a user

The most likely outcome: the install command exits with an error, and
MeedyaDL's own install routine reports that failure back — most likely as a
long, confusing wall of compiler output rather than anything a typical user
could act on. An already-installed copy of GAMDL is left untouched, so an
existing user keeps working but can never upgrade past whatever they already
had. The worse case is a brand-new install: a fresh setup on Windows-ARM64
has no GAMDL at all yet, so if the installer is allowed to reach for 3.9.1
there, the whole setup process cannot complete and the app is unusable on
that platform until the problem is fixed.

One possibility could not be ruled out from reading the code alone: pip
might, in principle, notice the failure and quietly try an older GAMDL
release instead of failing outright, leaving a user silently on an older
version while the app reports success. That was judged unlikely — a build
failure discovered partway through installing usually stops the whole
process rather than trying something else — but it was not proven either
way, since doing so would mean actually running the install on a real
Windows-on-ARM64 machine, which was not available for this audit. Either
way, the same fix is needed: an explicit, working per-platform ceiling that
stops the installer from ever asking pip for 3.9.1 on that one platform in
the first place.

### The existing "compatible wheel" check does not catch this

MeedyaDL already has a check that looks at which ready-made packages a new
GAMDL release publishes, to warn a user when their platform isn't covered.
That check only ever inspects the filenames GAMDL itself publishes — and
GAMDL's own Windows-ARM64 package for 3.9.1 genuinely exists, so the check
correctly reports it as available and would offer the upgrade. The
incompatibility here lives one level further down the dependency chain,
somewhere that check has never looked and was never designed to look.
Extending it to walk an entire dependency tree for every release would be a
substantial piece of work on its own, and is not recommended here — a
correctly working per-platform ceiling is the right, and far simpler, fix.

## Per-platform install behaviour, summarised

| Platform | Resolves to, once the fix below is in place |
|---|---|
| macOS, Apple Silicon | 3.9.1 |
| Windows, 64-bit | 3.9.1 |
| **Windows, ARM64** | **3.8.5** (held back — see above) |
| Linux, 64-bit | 3.9.1 |
| Linux, ARM64 | 3.9.1 |
| Linux, ARMv7 (32-bit) | 3.8.1 (falls back on its own, as before) |

## Nothing else needs changing (swept)

| Area | Finding |
|---|---|
| Every existing GAMDL-version capability check | All resolve correctly at 3.9.1 without any change to their own logic — each is either a simple "available from version X onward" threshold or a bounded window, and 3.9.1 sits correctly on the right side of every one checked, including the ones governing the wrapper decrypt address, the "web" AAC codec naming, the `--no-exceptions` flag, the FFmpeg path flag, and native muxing. |
| The wrapper protocol | GAMDL's wrapper client file is byte-identical between 3.8.5 and 3.9.1, and still expects the same wrapper version. Every wrapper-related check, address, and piece of guidance MeedyaDL shows stays correct without any change. |
| Output parsing | Every line GAMDL prints at its normal logging level is unchanged; see 3.9-G above. |
| Error classification | Unaffected; see 3.9-G above. |
| Configuration file | No key MeedyaDL writes was added or removed; see 3.9-F above. |
| Playlist file generation | Inert for MeedyaDL; see 3.9-C above. |
| The `--database-path` feature | Inert for MeedyaDL; see 3.9-H above. |
| Cover art | Nothing in this diff touches how GAMDL fetches cover art. MeedyaDL's own cover-art renaming, fallback chain, and cross-service upgrade all operate on files already on disk and are unaffected. |
| Content-advisory and codec filename suffixes | Driven entirely by MeedyaDL's own metadata and by inspecting the downloaded file directly; unaffected by anything in this release. |
| Music-video sort metadata removal | MeedyaDL writes neither field itself; nothing to collide with. See 3.9-E above. |
| Music-video name-clash handling vs MeedyaDL's own renaming | No conflict — MeedyaDL's renaming only ever touches audio files, never music-video files. See 3.9-E above. |
| The "compatible wheel" check | Left alone, deliberately. It correctly inspects only what GAMDL itself publishes, and GAMDL's own Windows-ARM64 package genuinely exists — the problem is one level further down, which a correctly working per-platform ceiling handles properly without needing this check to grow much more complicated. |

## Actions required

### Required — without these, something breaks

#### R1. Stop Windows-on-ARM64 from ever being offered 3.9.1

| | |
|---|---|
| **Why** | `pyplayready` requires `cryptography` at a version range where no release publishes a Windows-ARM64 package. Installing 3.9.1 there tries to compile it from source and fails; a first-time install on that platform would not complete at all. |
| **Risk if skipped** | High. New installs on Windows-ARM64 cannot complete; existing installs there can never move past 3.8.5. |

Three changes are needed together — any one alone would leave a hole:

1. Record a per-platform ceiling of 3.8.5 for Windows-on-ARM64, alongside
   the existing entry for 32-bit Linux ARM.
2. The function that builds the version range handed to the installer must
   read that per-platform ceiling for the machine it's actually running on,
   rather than always reading the single global ceiling. Today, the
   per-platform table only affects the "is this tested?" wording shown to a
   user — it does not yet constrain what gets installed. This is the change
   that makes it actually constrain the install, and it is also worth doing
   for 32-bit Linux ARM even though that platform has worked correctly so
   far — by luck of GAMDL's own missing package there, not by design.
3. There is a separate installer path used when a user explicitly targets an
   exact version — for example clicking "upgrade" on a specific release
   shown as available in the app. That path pins an exact version and would
   walk straight past the fix in step 2 if it is not also taught to refuse
   (or clamp down) a version above what that platform can actually take,
   with a plain-English explanation of why.

**How to confirm this works.** Most of it can be checked without a real
machine: the version-range builder should produce the lower, 3.8.5-bounded
range specifically for the Windows-ARM64 platform identifier and the normal
3.9.1-bounded range everywhere else; the explicit-version installer path
should refuse an explicit request for 3.9.1 on that same platform. The part
that genuinely needs a real machine — actually running the install on
Windows-ARM64 and watching what happens — was not available for this audit
and is worth doing before this reaches a wide release either way.

#### R2. Make the "this update is untested" warning platform-aware

| | |
|---|---|
| **Why** | The function that decides whether to show an "untested" warning on an available update currently checks only the single global ceiling. Once that global ceiling reads 3.9.1, a Windows-ARM64 user would be shown 3.9.1 as an ordinary, fully tested upgrade — with no warning at all — and clicking it would fail. |
| **Risk if skipped** | Medium. A user is offered an upgrade that cannot work on their machine, with no warning beforehand, and the failure they see is a wall of compiler output rather than an explanation. |

**Change needed.** The same function should compare a candidate version
against the ceiling for the specific platform it's running on, not the
single global one — using the same per-platform-aware lookup as R1 above.

**How to confirm this works.** On the Windows-ARM64 platform identifier,
with the latest available release read as 3.9.1, the function should report
that release as untested.

### Advisable — nothing breaks if these are skipped, but they are worth doing

#### A1. Stop the album-name guard failing silently

| | |
|---|---|
| **Why** | Covered in full under 3.9-D above. GAMDL now writes an album name pulled from iTunes where it used to write none at all. MeedyaDL compares that, by an exact, case-insensitive match, against the name its own Catalog API fetch separately found. Where the two disagree — most commonly a trailing " - Single" or " - EP", or slightly different edition wording — MeedyaDL's entire layer of richer metadata is silently skipped for that file, with nothing but a line in the tracing log to show for it. |
| **Where** | `src-tauri/src/services/metadata_tag_service.rs`, the guard around lines 704–741. |
| **Suggested change** | Keep the guard itself — it protects against a real cross-contamination defect (#452) — but stop the failure being invisible: raise the existing warning to somewhere a user's activity log actually shows it. A softer comparison could also be considered, such as trying again after stripping a trailing " - Single" or " - EP" before deciding the names genuinely disagree. |
| **Risk if skipped** | Medium, and the shape of the risk is the concerning part — it fails invisibly. Nobody would file this as a bug; singles and music videos would simply, quietly, end up with plainer metadata than an album download gets. |

#### A2. Give the ungated tool-path flags a version check

| | |
|---|---|
| **Why** | Covered in full under 3.9-F above. A pre-existing function in MeedyaDL's own installer-invocation code can send two command-line flags GAMDL stopped recognising some releases ago, with no check on the installed version at all. An earlier audit fixed the equivalent problem in three other places and missed this fourth one. Not caused by this release. |
| **Where** | `src-tauri/src/services/gamdl_service.rs`, the `inject_tool_paths` function. |
| **Suggested change** | Apply the same version check already used at the equivalent, correctly-gated site elsewhere in the codebase. |
| **Risk if skipped** | Unknown, deliberately stated as unknown rather than guessed at — it is either harmless dead code, or it breaks every download on a current GAMDL version, and reading the code alone could not settle which. The first step, before writing any fix, should be to log the actual list of arguments sent on one real download and simply look. |

#### A3. Consider a belt-and-braces install guard behind R1

| | |
|---|---|
| **Why** | If the `cryptography` package were also added to the installer's "must use a ready-made package, never compile" list, the 3.9.x branch of GAMDL's own version range would become impossible to satisfy on Windows-ARM64 at all, and pip would fall back to 3.8.5 entirely on its own — the same kind of self-correction 32-bit Linux ARM already benefits from. |
| **Caveat** | This is a safety net behind R1, never a replacement for it. It depends on pip's own fallback behaviour, which is slow and gives no explanation to the user — they would simply end up on 3.8.5 with nothing telling them why. R1 is a direct, deterministic fix that also explains itself; this is worth considering only in addition. |
| **Risk if skipped** | Low. R1 already covers the case on its own. |

#### A4. Say plainly, somewhere users will read it, that some music videos will move

| | |
|---|---|
| **Why** | Covered in full under 3.9-D above. A music video that previously had no album context will often now land inside its album's folder instead of MeedyaDL's own no-album fallback location, and lose the numeric-ID suffix that used to guarantee its filename was unique. This is the behaviour MeedyaDL's own design notes for music-video placement have wanted — arriving for free — but it moves existing users' files without any warning. |
| **Where** | Release notes; the music-video filename and folder design notes; the in-app help page covering video downloads. |
| **Risk if skipped** | Low, but expect "where did my files go?" questions from users who upgrade. |

## Support-window values, once R1 and R2 are in place

```toml
# src-tauri/tool-versions.toml
maximum_tested_version = "3.9.1"     # was "3.8.5"
recommended_version    = "3.9.1"     # was "3.8.5"

[gamdl.platform_ceilings]
linux-armv7      = "3.8.1"           # unchanged — GAMDL still publishes no package there
windows-aarch64  = "3.8.5"           # new — no Windows-ARM64 package for the pinned cryptography range
```

`minimum_version` stays `"3.0"`, unaffected by anything in this audit.

## Decisions for the maintainer

| # | Decision | Recommended answer |
|---|---|---|
| D1 | Admit 3.9.1 now, or hold the ceiling at 3.8.5? | **Admit — but only together with R1 and R2.** If there is no appetite for the platform-ceiling work this cycle, holding at 3.8.5 costs very little: nothing in 3.9.1 is urgent for a MeedyaDL user. What is not acceptable is moving the single global ceiling on its own without R1, since that would break Windows-on-ARM64 outright. |
| D2 | How should Windows-on-ARM64 be protected? | Make the existing per-platform ceiling table actually govern what the installer asks pip for (R1), rather than only the wording of a warning message. This also tidies up the 32-bit-ARM-Linux case, which has only ever worked by luck of a missing GAMDL package there, not by design. Treat A3 as a safety net behind it, not a substitute for it. |
| D3 | Expose the new `--drm-backend` / `--prd-path` options in MeedyaDL's own settings? | **Not now.** PlayReady needs a device file the user has to obtain themselves, and offers a user who sets nothing today nothing at all. Worth revisiting only if unlocking 4K music video downloads becomes a goal — that is the one thing it opens up. |
| D4 | Act on the silent metadata-enrichment skip (A1)? | **Yes — at minimum, make it visible.** A safety check that fails silently is the worst kind of safety check. Making the skip audible is cheap; softening the comparison itself can follow separately if it turns out to be needed. |
| D5 | Chase the ungated tool-path flags (A2)? | **Yes, but as its own piece of work, separate from this release.** It predates 3.9 entirely and is unrelated to it. Start by finding out whether it actually fires today before deciding how to fix it. |
| D6 | Raise the `cryptography` version pin with the `pyplayready` project upstream? | **Yes.** `cryptography` 46.x does publish Windows-ARM64 packages; asking `pyplayready` to relax its `<46.0.0` restriction would remove this problem at the source, for every project depending on it, not just MeedyaDL. Cheap to ask, and the only fix that actually makes the underlying problem go away rather than working around it. |
| D7 | Accept that some music videos will move into their album's folder (A4)? | **Accept it, and say so plainly in the release notes.** It is the behaviour MeedyaDL's own design notes for music-video placement have asked for, arriving without any MeedyaDL code change at all. |

## Pre-release gate

Before this ceiling reaches a wide release, carried forward from the 3.8.4 /
3.8.5 live testing checklist and retargeted at 3.9.1:

1. Keep the existing song-ending integrity check (the fix carried forward
   from 3.8.4, confirming a wrapper-decrypted song's final seconds are
   intact).
2. **Download one album with the codec forced to AAC Legacy.** This is the
   single most valuable test in the whole set — it is the exact path the
   3.9 defect broke and 3.9.1 is meant to have repaired.
3. **Download one music video that belongs to an album**, and confirm where
   it lands on disk; then **download one single**, and confirm its
   MeedyaDL-added metadata (the richer tags covered under 3.9-D / A1 above)
   is actually present on the file.
4. On a real Windows-ARM64 machine, confirm the installer lands on 3.8.5 and
   explains why, rather than attempting 3.9.1.

## What could not be verified

Stated plainly, because leaving this unsaid would suggest more certainty
than this audit actually has:

1. **The Windows-ARM64 install failure was not reproduced on a real
   machine.** It is derived from PyPI's own published package lists and the
   version range `pyplayready` demands. The package facts themselves are
   solid and were checked independently for this document; the exact way
   the failure presents itself — a hard, visible error, versus pip quietly
   falling back to an older GAMDL release — was not confirmed.
2. **The 3.9 "web" AAC defect was not reproduced against a real Apple Music
   download.** It is read off the code and the commit that fixes it. The
   reasoning holds together well, but it is inference from the code, not a
   direct observation of the failure happening.
3. **How often the iTunes-derived album name and MeedyaDL's own Catalog-API
   album name actually disagree was not measured.** That figure would say
   how often the silent metadata skip described in 3.9-D and A1 actually
   bites in practice, and measuring it needs real downloads against both
   Apple services.
4. **Whether the ungated tool-path flags described in 3.9-F and A2 actually
   fire on an ordinary install today is unresolved.** It depends on whether
   the specific tool path they check for exists at the exact location
   MeedyaDL looks for it in a typical install, which could not be determined
   by reading the code alone.
5. **No code was built or run to produce this document.** No compiler was
   invoked, no package installer was run, no GAMDL process was launched.
   This was a reading exercise against the source code of both releases,
   the packages they and their dependencies publish, and MeedyaDL's own
   codebase.

## What MeedyaDL changed

All of the below landed together on `work/after-1.10.8` on 2026-09-22. Each
piece was reviewed by an agent that did not build it; Codex, the usual
reviewer, was out of usage credit until 27 September, so a catch-up review by
Codex is still owed before any of this merges.

### Admitting 3.9.1 and refusing 3.9

| Change | Where |
|---|---|
| Ceiling and recommended version both `3.9.1` | `tool-versions.toml` |
| `windows-aarch64 = "3.8.5"` added beside `linux-armv7 = "3.8.1"` | `tool-versions.toml` → `[gamdl.platform_ceilings]` |
| A list of releases that are known to be broken, with the reason and the release that fixes each one, plus a matching `KnownBad` verdict | `gamdl_capabilities.rs` |
| The per-platform ceiling now governs what pip is asked to install, not only what the screen says | `gamdl_capabilities.rs::pip_version_spec`, `pip_target_spec` |
| An explicit "install exactly this version" request is refused when it is above what this platform can install | `gamdl_service.rs::refuse_unsupported_target` |
| `cryptography` added to the binary-only list, so pip cannot try to build it from source | `gamdl_service.rs` |
| A red "Known Issue" badge and a sentence saying what to do | `ToolsTab.tsx`, fed by `DependencyStatus.classification` / `known_bad_message` |

Every sentence shown to a user that names a GAMDL version is now built by one
function, `known_bad_advice`, which takes the platform. It has three cases, so
a held-back platform is never told to "update" to a version below the one it
already has, and never told "or newer" in a way that would re-permit the
broken release. An earlier version of this work got exactly that wrong on
Windows on ARM, in four places; a review caught it, and the shared function is
the answer to it happening again.

### The PlayReady setting

Offered in Settings > Advanced, switched off, and **shown only when the
installed GAMDL is 3.9 or newer** — with one deliberate exception: if it has
already been switched on and GAMDL is later downgraded, the section still
appears, with a note. A setting that is switched on but invisible is worse
than one that is visible and explained.

One function decides what actually happens, `plan_drm_backend` in
`download_queue/options.rs`. It sends both options to GAMDL or neither, never
one, because GAMDL stops without downloading anything if it is told to use
PlayReady with no device file — which reaches MeedyaDL as a download that
ended with no output, the least informative failure there is. When PlayReady
cannot be used (GAMDL too old, no file chosen, file no longer there) the
download goes ahead on the built-in unlocking and one plain sentence explains
why, next to the existing line about how the download is signing in.

Neither option is written to GAMDL's configuration file, and that is a
decision rather than an omission: GAMDL reads that file as its defaults and
lets the command line override them, so a line left there would outlive
MeedyaDL's decision to fall back and break the download it was meant to
protect.

### Two pre-existing faults, found by this audit and fixed with it

Neither was caused by 3.9.1.

**The album-name guard was about to start failing silently.** Before writing
any of its own metadata, MeedyaDL checks that the file's album name matches
the one Apple Music's catalogue gave it — a guard against one album's
metadata landing on another's tracks. Singles and music videos used to have no
album name at all, so the check fell through to comparing artists and passed.
From 3.9, GAMDL fills that name in from iTunes, which writes `"X - Single"`
where the catalogue writes `"X"` — a mismatch, so every piece of MeedyaDL's
own metadata would have been skipped for those files, with nothing on screen
to say so. The comparison now ignores case and those two suffixes, and a skip
is written to the activity log. Edition wording (`"(Deluxe Edition)"` against
`"(Deluxe)"`) is deliberately still treated as a mismatch: telling a wording
difference from a genuinely different release means guessing, and a wrong
guess is the exact fault the guard exists to prevent.

**Two command-line flags were being sent ungated.**
`gamdl_service.rs::inject_tool_paths` could pass `--mp4decrypt-path` and
`--mp4box-path`, which no GAMDL since 3.6 defines, and GAMDL treats an option
it does not recognise as a hard error rather than ignoring it. The same two
flags are emitted from a second place as well, and that one was gated
correctly during the 3.7 work; this one, on the live download path, was
missed. It is now gated the same way. **Whether it ever actually fired is
unresolved** — it needs both no user-set path and the managed tool present —
so this is either a fix for a live fault or a guard against one that was
waiting to happen.

### Written down rather than changed

That music videos now often land in the album folder instead of a separate
Music Videos folder, and that a track can gain a disc-number prefix, are
upstream behaviour changes MeedyaDL chose to accept rather than override — the
first is the behaviour MeedyaDL's own design document asked for, arriving on
its own. Both are recorded in `DEV_NOTES.md` and in the README, because the
visible effect is that files appear somewhere new after an upgrade, and that
is the kind of thing users report as a fault when nobody told them.
