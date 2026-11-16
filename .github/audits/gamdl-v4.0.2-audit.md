<!-- Copyright (c) 2024-2026 MeedyaSuite. MIT licence. -->
# GAMDL 4.0.2 Audit — 16 Nov 2026

**Verdict: admitted as the version to try (opt-in, still untested on real downloads); 4.0.1 is now refused.** Nothing here changes the recommended version, which stays 3.9.1.

**Source read:** the full `v4.0.1..v4.0.2` diff of glomatico/gamdl (9 commits, 38 files). PyPI: 4.0.2 (uploaded 16 Nov 2026) ships the identical set of six files as 4.0.1 (five wheel platforms plus the source package), and still has no 32-bit ARM Linux wheel. Requirements are unchanged, so no platform ceiling moves.

## What changed, and what it means for MeedyaDL

| Change in 4.0.2 | Effect on MeedyaDL | Done |
| --- | --- | --- |
| `enhanced-ttml` added to the lyrics file and embedded lyrics formats (asks Apple for the original word-timed TTML) | **Not used for ordinary downloads.** If a track has no word timing, no `.ttml` file is written at all, and every lyrics sidecar MeedyaDL builds (Enhanced LRC, WebVTT, styled SRT, ASS, Lyricsfile) is made from that file. Plain `ttml` always gives something. **Used since 16 Nov 2026 by one extra pass** after a download, only for tracks MeedyaDL's own word-level lookup could not check (no developer key, no usable sign-in — e.g. wrapper only — a refused request, or a file it could not place): GAMDL is run again with overwrite off, so every existing track is skipped and only its `.ttml` is refreshed (read in 4.0.2's `downloader.py`: a skipped track still goes through `_save_sidecars`, which refreshes a `.ttml`; `enhanced-ttml` with no word timing leaves the file untouched). | Ordinary downloads stay on `ttml`. `GamdlFeature::GamdlFourWordTimedLyricsFile` (>= 4.0.2); the pass is `download_queue/lyrics_pass.rs` (never sends `--overwrite`, `--save-cover`, `--file-cover-size`, `--save-playlist` or `--no-lyrics-file`; refuses if a primary file has moved; never fails the item). |
| Full version in the banner and generated config comments | No effect: MeedyaDL reads the version from `pip show`, not the banner. | None needed. |
| Shared album cover restored (`Cover.jpg`/`Cover.png` in the album folder; videos keep a picture named after the video; an existing album cover is reused when overwrite is off) | Saved cover art works as in GAMDL 3, so follow-up 4 of the 4.0 audit is resolved from 4.0.2. | `GamdlFeature::GamdlFourSharedAlbumCover` (>= 4.0.2). `to_cli_args_v4` sends `--save-cover` when Save Cover Art is on, plus `--file-cover-size` (a number, never `max`; 0 is not sent) when the 4.0.1 gate also holds. Capability `gamdl_four_saves_album_cover` hides the Cover Art tab's "no effect" note. The music-video-only suppression in `options.rs` is kept. Both options exist in 4.0.2's own `cli/options.py`. |
| `organize` relocates shared album artwork | MeedyaDL never runs `organize`. | None needed. |
| Generic key address (`P000000000/s1/e1`) recognised again, and decrypted locally with the built-in key | **This is why 4.0.0 and 4.0.1 are both refused (for this fault).** Upstream's filter that discards that address came in with commit 9a78275, already in 4.0.0, and 4.0.0 has the same pass-through test; so neither release recognises it when no track-specific one exists, and the encrypted audio could be passed to the file writer unchanged. 4.0.2 fixes it (commits a06ea3cd and fd725b6c). | 4.0.1 added to `KNOWN_BAD_VERSIONS` (fixed in 4.0.2); 4.0.0's reason now names this fault too and its `fixed_in` moved from 4.0.1 to 4.0.2 so its advice no longer points at a refused version. |
| 32-bit `stco` chunk offsets and 8-byte `mdat` headers again | No effect: the tagging library MeedyaDL uses reads both layouts. | None needed. |
| Interactive menus (Enter / Space / B) | Not used: MeedyaDL never opens GAMDL's menus. | None needed. |

## The 4.0.1 refusal

Evidence is the source and the tests that changed, not a real download: in 4.0.1 the stream code drops the generic key address, and 4.0.2 restores it with a new local-key path and a test for it. The user-visible wording is "some short songs can be saved still locked, so they will not play, while reported as downloaded". `refuse_unsupported_target("4.0.1")` now fails naming 4.0.2; a plain reinstall (`routine_install_spec`) moves 4.0.0 and 4.0.1 to 4.0.2; the Tools screen marks either as a known problem. Tests cover the advice, the upgrade target, the install spec and the refusal.

## TTML refresh

With overwrite off and a `ttml` lyrics format, 4.0.2 re-reads an existing `.ttml` sidecar and refreshes it if the content differs. In a companion run (second format of the same album) this could replace a word-timed TTML MeedyaDL already upgraded. Needs a real machine to confirm; if it happens, the answer is to run companions without refreshing, or to re-run MeedyaDL's own syllable step after.

The same refresh is what the word-level lyrics pass (16 Nov 2026) relies on, deliberately: it asks for `enhanced-ttml` with overwrite off, so a track whose audio already exists is skipped and only its `.ttml` is rewritten, and a track with no word timing is left alone. It runs before ANY rename, the album folder's included, with the primary run's own templates, its resolved download folder and only on the GAMDL version the primary run used; the extra-format copies are held until the renames are done, so nothing else lands in the folder during the pass. Real-machine checks: that no audio is downloaded again (file times), a wrapper-only sign-in, a multi-disc album, and per-track `?i=` links as a possible speed-up.

## Not checked, stated plainly

- No real download was run on 4.0.2.
- The 4.0.1 fault is inferred from source and tests; which tracks are affected is unmeasured.
- Whether the TTML refresh really replaces a word-timed file in a companion run is not confirmed.
- Where the shared `Cover` file lands for a single song, an album and a playlist, and whether the companion run adds a duplicate, is read from source only.

## Follow-ups (real machine)

1. A 4.0.1 download of a short song (is it locked?) against the same song on 4.0.2.
2. Cover layout for a song, an album and a playlist; no duplicate from the companion run; a music video's `<name>.jpg`.
3. The companion TTML refresh above.
4. Optional: `ffprobe` and tag writing on a 4.0.2 file.
5. Extra-format copies on a mixed-format album are requested as per-track song links (`album/_/{album_id}?i={song_id}`): check they land in the album folder with the expected file names (same template as the album run), on 3.x and 4.0.2.
