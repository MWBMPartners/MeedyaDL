# GAMDL v3.2 release smoke-test procedure

This document describes the manual verification steps to run **before cutting
the first MeedyaDL release that includes the v3.2 support-window bump (#619)**.
The audit sandbox could not exercise Tauri + GAMDL directly; these checks
must be performed on a real developer machine with an Apple Music
subscription before the release ships.

## Prerequisites

1. Build the release candidate from `claude/audit-gamdl-v3.2-eI87q` (or the
   merged `main`) on each target platform: macOS (Apple Silicon), Windows
   (x64), Linux (x64).
2. Install into a fresh profile / user account so existing `settings.json`
   and `queue.json` don't mask migration behaviour.
3. Have a valid Apple Music cookies file ready for import.

## Scenarios

### A. Fresh install resolves `gamdl==3.2`

**Goal:** confirm the installer honours the updated support window.

1. Launch MeedyaDL on a fresh profile.
2. Complete the setup wizard, allowing MeedyaDL to install Python + GAMDL.
3. Open **Settings → About → Component versions** and confirm `GAMDL: 3.2.x`.
4. Open the activity log and confirm the startup `[System]` entry reads
   `GAMDL version 3.2.x — Supported` (not `Untested` or `NotInstalled`).

**Pass criteria:** step 3 reports `3.2.x`; step 4 reports `Supported`.

### B. Existing v3.1 user sees the upgrade offer

**Goal:** confirm the update banner surfaces v3.2 to users already on v3.1.

1. On a second test profile, downgrade manually:
   ```bash
   ${MEEDYADL_APPDATA}/python/bin/python -m pip install 'gamdl==3.1'
   ```
2. Launch MeedyaDL and open the **Updates** page (or wait for the update
   banner).
3. Confirm the page lists `GAMDL 3.2.x` with `is_compatible=true` and a
   one-click upgrade control.
4. Click upgrade and verify GAMDL is bumped to `3.2.x`.

**Pass criteria:** update is offered, one-click upgrade succeeds.

### C. Existing v2.9.x user remains Supported

**Goal:** confirm the floor stays valid.

1. On a third profile, pin GAMDL to `2.9.3`:
   ```bash
   ${MEEDYADL_APPDATA}/python/bin/python -m pip install 'gamdl==2.9.3'
   ```
2. Launch MeedyaDL.
3. Activity log `[System]` entry should read `GAMDL version 2.9.3 — Supported`.
4. Attempt a single-song download; confirm it completes without emitting
   any of: `--playlist-folder-template`, `--wrapper-m3u8-ip`,
   `--song-codec`, `--no-exceptions` (v3.1+ no-op gate), or `fetch_extra_tags`
   (should still be present since capability gates allow it on v2.x).

**Pass criteria:** v2.9.3 still downloads cleanly; no v3.0+ CLI flags leaked.

### D. Smart codec selection — fallback-disabled path (#614 regression guard)

**Goal:** confirm the pre-existing latent bug in `--song-codec` is fixed.

1. On any profile (v3.2 preferred), open **Settings → Quality** and disable
   **Fallback Quality Chain** OR clear the chain.
2. Queue a single song URL.
3. Watch the activity log. Confirm the spawned command line contains
   `--song-codec-priority alac` (or whichever codec was configured) and
   NOT `--song-codec`.
4. Confirm the download completes.

**Pass criteria:** `--song-codec-priority` in the spawned args, download
succeeds. Prior to #614, step 3 would have emitted `--song-codec` and
crashed Click with `Error: No such option: --song-codec`.

### E. Playlist folder template (#618)

**Goal:** confirm the new `--playlist-folder-template` wire reaches GAMDL
on v3.0+ and is skipped on v2.9.x.

1. On a v3.2 profile, set **Settings → Templates → Playlist Folder** to
   `MyPlaylists/{playlist_artist}`.
2. Download a small Apple Music playlist URL.
3. Confirm the output folder layout is `MyPlaylists/<artist>/…` and that
   the spawned command line contains `--playlist-folder-template`.
4. Downgrade to GAMDL 2.9.3 (per Scenario C), keep the template setting,
   repeat the playlist download.
5. Confirm the spawned command line does NOT contain
   `--playlist-folder-template` (capability gate suppressed it) and that
   GAMDL falls back to its own default layout without errors.

**Pass criteria:** v3.2 honours the template; v2.9.3 skips the flag
silently and downloads.

### F. Abort queue (#620)

**Goal:** confirm the new destructive action works end-to-end.

1. Queue 5–10 URLs (mix of albums and singles).
2. Start the queue, let item 3 begin downloading.
3. Click the red **"Abort Queue"** button in the queue header.
4. Confirm the modal, click Abort.
5. Verify:
   - Items 1 + 2 stay `Complete`.
   - Item 3 transitions to `Cancelled`, GAMDL subprocess exits within a
     poll interval (check Activity Monitor / Task Manager).
   - Items 4–10 go to `Cancelled`.
   - A toast shows `Aborted — stopped 1 downloading, 7 queued`.
   - Activity log has a `[System] Aborted queue …` entry.
   - Post-queue action (if configured — e.g., "Play sound", "Shutdown
     computer") does NOT fire.
6. Re-queue a different URL; confirm the post-queue action fires this
   time (suppression is one-shot, auto-clears).
7. Test the global status-bar **Abort** button from the Settings page —
   it should fire the same abort flow.
8. Test the **Cmd/Ctrl+Shift+.** keyboard shortcut — ditto.
9. Tick **"Don't ask again"** on the modal, re-abort; the modal should not
   appear on the next trigger until **Settings → General → Preferences**
   re-enables it.

**Pass criteria:** all nine sub-steps pass; no items leak subprocesses.

### G. Sequential metadata fetch observability (#616)

**Goal:** confirm the user-facing docs explain the v3.2 behaviour change.

1. Launch MeedyaDL on a v3.2 profile.
2. Open **Help → FAQ** and scroll to the GAMDL section.
3. Confirm the "Why did my album's initial metadata phase get slower
   after upgrading GAMDL?" entry is present and references the sequential
   fetch change.

**Pass criteria:** FAQ entry renders correctly; links / formatting intact.

## Reporting

If any scenario fails, file a follow-up issue referencing #619 with:

- The MeedyaDL build version.
- The GAMDL version (from Settings → About).
- The platform (macOS / Windows / Linux + arch).
- The observed failure vs. the documented pass criteria.
- A redacted excerpt from the activity log covering the failure window.

## Sign-off

When all scenarios pass on macOS + one non-macOS platform (Windows or
Linux), the release is clear to ship. Record the sign-off as a comment on
#619 with the platforms tested and the tester's handle.
