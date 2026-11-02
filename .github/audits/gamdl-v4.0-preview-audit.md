<!-- Copyright (c) 2024-2026 MeedyaSuite. MIT licence. -->
# GAMDL 4.0 Preview Audit — PREPARATION ONLY (4.x refused until support is switched on)

**Date**: 2026-11-02
**What was read**: the upstream `4.0` branch at commit `0c884ecc` (1 Nov 2026,
"feat(cli): expose music video MP4 container selection"), compared with `main`
at `3.9.1`. Version string on the branch: `4.0.0a0`. **4.0 is not released.**
**Size of the change**: 136 commits, 229 files, about 39,700 lines added and
12,300 removed. Every Python file was moved or rewritten; the Rust extension
was replaced.
**How it was read**: the source itself — options, settings loader, output
code, error handling, lyrics, artwork, packaging — not the commit titles.
**Predecessor audit**: [`gamdl-v3.9-v3.9.1-audit.md`](./gamdl-v3.9-v3.9.1-audit.md)

## In one paragraph

GAMDL 4.0 is a new program that happens to share a name with the old one.
It is started differently, takes a different settings file, has different
option names, prints different output, saves artwork and lyrics differently,
and no longer uses FFmpeg, mp4decrypt, MP4Box or N_m3u8DL-RE at all. If a
person installed 4.0 into MeedyaDL today, **every download would fail on the
first line**, because MeedyaDL starts GAMDL with `python -m gamdl` and 4.0
cannot be started that way. So this round does two things: it stops 4.x
being installed by accident, and it builds — but does not switch on — the
pieces MeedyaDL needs to drive 4.x, so that support can follow quickly once
4.0 is released and checked on a real machine.

## What changed, and what it means for MeedyaDL

| Area | GAMDL 3.x | GAMDL 4.0 (branch) | Effect on MeedyaDL | Done in this round |
|---|---|---|---|---|
| How it starts | `python -m gamdl <links> <options>` | No `__main__` module. A command group; downloading is `gamdl rip <options> <links>`. Entry point `gamdl.cli.main:main` | `python -m gamdl` fails: "No module named gamdl.__main__" | 4.x launcher written: `python -c "…from gamdl.cli.main import main…" --no-config rip …` (`gamdl_service::gamdl_v4_args`) |
| Settings file | `config.ini` (MeedyaDL writes it, passes `--config-path`) | YAML at GAMDL's own settings folder; group option `--config` / `--no-config`; rewrites an outdated file in place | `config.ini` is ignored; a YAML file a person made by hand would silently change downloads | 4.x path never writes `config.ini` and always passes `--no-config` |
| Environment variables | none read | every option can come from a `GAMDL_…` variable | a stray variable could change downloads | 4.x path removes `GAMDL_…` variables for the child process |
| Sign-in | `--use-wrapper`, cookies by default | `--auth-mode wrapper\|cookies\|media-user-token`; **with none given, an interactive set-up wizard opens** | would hang waiting for an answer | always sends `--auth-mode`; input is closed so any question fails at once |
| Wrapper | v2 only since 3.6: `--wrapper-url`, decrypt host/port | same three, same API version check (`0.0.2`); the m3u8 and account addresses are gone | none beyond names | mapped |
| Unlocking protected tracks | `--drm-backend`, `--wvd-path`, `--prd-path` | `--drm-backend`, one `--device-path` for both | rename | mapped |
| **Widevine library** | always installed | **optional extra** (`gamdl[widevine]`); Widevine is still the default method | a plain install could not unlock anything | 4.x install spec asks for `gamdl[widevine]` (`pip_target_spec`) |
| Helper programs | FFmpeg path option (3.7+), N_m3u8DL-RE path option | none: no outside program is started at all | sending any path option ends the run | 4.x builder sends none; `FFmpegPath` gate now off from 4.0 |
| Codec choice | `--song-codec-priority` (names since 3.6) | same names; `ask` is no longer a codec | none | names re-checked and filtered |
| Video quality | `--music-video-resolution 1080p`, `--music-video-remux-format` | `--max-height 1080`, `--music-video-container m4v\|mp4` | rename | mapped |
| Uploaded videos | `best` / `ask` | Apple's own variant names | `best`/`ask` refused | not sent (default is best-first) |
| Artwork format | jpg, png, raw | jpg, png only; `--cover-size` gone | `raw` refused | `raw` not sent (4.0 uses jpg) |
| **Saved artwork** | one `Cover.<ext>` per album folder | **one picture per track, named after the track** | MeedyaDL's rename step finds no `Cover` file; a picture beside every song | 4.x builder does not ask for saved artwork; MeedyaDL's own cover step already writes the album picture |
| **Lyrics** | `.lrc`, `.srt` or `.ttml` sidecar (`--synced-lyrics-format`) | **`.lrc` (line timing) or `.txt` only**; no format option; no lyrics-only mode | MeedyaDL's Enhanced LRC, WebVTT, Rich SRT, ASS and Lyricsfile steps all start from the `.ttml` file, which 4.0 never writes | **not solved** — see follow-up 3 |
| Tags to leave out | `track`, `disc`, `date`, … | `track_number`, `disc_number`, `release_date`, plus `isrc`, `explicit`; an unknown name ends the run | rename | translated; unknown names dropped |
| Overwrite | flag | `--overwrite skip\|replace\|error` | rename | mapped |
| Other options gone | `--log-level`, `--no-exceptions`, `--use-album-date`, `--date-tag-template`, `--download-mode`, `--music-video-remux-mode`, `--read-urls-as-txt`, `--no-config-file`, `--synced-lyrics-only` | gone (`--debug` replaces the traceback option) | sending any ends the run | not sent; `NoExceptionsFlag` gate now off from 4.0 |
| Templates | same eight | same eight, same field names, **but a field not on its list ends the run** | MeedyaDL already replaces its own `{platform}` before passing templates on | passed through unchanged |
| Parallel work | one item at a time | two items at a time by default (`--concurrent-downloads`), 8 requests each | per-item results can arrive out of order | left at GAMDL's default; noted |
| **Output** | log lines (`[Track 3/12] Downloading …`, `Saved to: …`) | `rich` text: `✓ Title: downloaded → path`, `↷ Title: skipped (…) → path`, `✗ label: message`, then `Finished: N downloaded, …, N failed, …` | nothing would be recognised; the summary contains "failed" and would read as an error | 4.x lines recognised (`process::parse_gamdl_v4_line`); output forced wide, colourless and UTF-8 |
| Progress | per-track percentage lines | a live display drawn only for a terminal | no percentage while a track downloads | `--no-progress`; per-item results still arrive — see follow-up 5 |
| Exit code | 0 on success | 1 if anything failed **or** nothing was found | same meaning | none needed |
| Version | `pip show gamdl` | same; `--version` prints `Gamdl, version 4.0.0a0` | a pre-release such as `4.0.0a0` fails the strict version check, so every 3.x feature gate reads "off" | generation is read from the major number alone (`major_version_of`) |
| Python | 3.10 and later | 3.10 and later; native part built as `abi3-py310` | no change to the floor | none needed |

## What this round changed in MeedyaDL

1. **4.x cannot be installed by accident.** `HIGHEST_DRIVABLE_MAJOR = 3` in
   `gamdl_capabilities.rs`. `refuse_unsupported_target` turns away any 4.x
   install with a plain reason, and the Updates check marks a 4.x release as
   not compatible, so it is not offered. The ordinary install range already
   stopped at the tested ceiling, so it was never at risk. Before this, the
   "Untested" upgrade button would have installed 4.0 the day it appeared on
   the package index.
2. **Which kind of command line to build is one question, asked in one
   place**: `GamdlCliGeneration` / `detected_cli_generation()`, from the
   major version number only.
3. **A complete 4.x command builder**: `GamdlOptions::to_cli_args_v4` and
   `gamdl_service::build_gamdl_v4_command`, used only when the installed
   GAMDL reports 4 or more. Each option is translated, filtered or left out
   for a stated reason, and the tests check every option it can send against
   the full list 4.0 accepts.
4. **4.x output is recognised** by the existing parser, ahead of the 3.x
   rules, without changing how any 3.x line is read.
5. **Two 3.x gates corrected for 4.0**: `FFmpegPath` and `NoExceptionsFlag`
   now say "off" from 4.0, which is what the 4.0 code does.

Nothing about a 3.x download changes. Every new path is reached only when the
installed GAMDL's major version is 4 or more, and installing one is refused.

## Before support can be switched on (follow-ups)

1. **Run it against a real 4.0 release**, on each platform, including a
   wrapper download and a cookies download. The option and output shapes here
   come from an unreleased branch that was still changing the day before this
   was written.
2. **Check `pywidevine` installs everywhere MeedyaDL ships**, and decide the
   PlayReady extra: `gamdl[playready]` brings in the `cryptography` range
   that has no Windows ARM64 build (the reason that platform is held at 3.8.5
   today). Probably: add it only when the person has chosen PlayReady.
3. **Lyrics.** 4.0 writes no `.ttml` file, which is what all of MeedyaDL's
   own lyrics steps start from. MeedyaDL already fetches word-timed lyrics
   from Apple itself (enrichment step 1b) when a MusicKit key and a sign-in
   token are present; making that step write the `.ttml` file when GAMDL has
   not is the likely fix. Without it, a 4.x download gets only GAMDL's own
   line-timed `.lrc`.
4. **Artwork.** Confirm MeedyaDL's own cover step writes the album picture
   for every 4.x download, as expected, and decide whether the "save cover
   art" setting should still control it.
5. **Progress.** 4.0 reports progress to its own code as events, not as
   text. Without a live percentage the item bar will sit still until each
   track finishes. Options: accept per-track steps, or ask upstream for a
   machine-readable progress mode.
6. **Track numbering in the activity log** (`[Track N/M]`) has no 4.0
   equivalent; the per-item result lines carry the title only.
7. **Output detection.** 4.0's result lines carry full paths, which is
   better than 3.x; check `find_album_directory` and the "finished with no
   output" check behave when results arrive two at a time.
8. **Settings screen**: options that 4.0 dropped (lyrics format, cover size,
   remux mode, album date, date template) should be greyed out with a note
   when 4.x is installed, the same way older-version options are today.
9. **Help pages and README**: the supported-versions table, the tools page
   (4.x needs none of the four helper programs, though MeedyaDL's own
   enrichment still uses FFmpeg and MediaInfo), and the lyrics page.
10. **Then** raise `HIGHEST_DRIVABLE_MAJOR` to 4, update its test, add 4.0
    to the tested window in `tool-versions.toml`, and teach
    `pip_version_spec` to ask for the Widevine extra on a 4.x range.

## Not checked, stated plainly

- Nothing here has driven a real 4.0 download; there is no 4.0 release.
- `rich`'s behaviour with the `COLUMNS` setting was taken from its
  documented behaviour, not tested against 4.0's output.
- Whether 4.0 keeps the `[project.optional-dependencies] widevine` layout
  at release is unknown.
