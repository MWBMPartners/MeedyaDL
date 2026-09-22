<!-- Plan for the 15 issues reopened by the 2026-09-21 issues sweep. Written 2026-09-22.
     Planned by an Opus agent standing in for Fable (Fable was out of usage credit).
     The maintainer's decisions are at the top; the original plan follows unchanged. -->

# Maintainer's decisions (22 Sept 2026) — these override the plan below where they differ

- **Closed:** #393 (not planned).
- **Kept open, NOT in this queue:** #352 (waits for MeedyaSuite-core) and #431 (the service-status
  code is **not** deleted; it waits with the multi-service work). So batch 1b below is dropped.
- **Dropped:** #216's "pre-release access control" item (Alpha already needs developer access).
- **Tempo (BPM) is kept.** #759's Fill gaps keeps the tempo step, and the tempo service, which
  nothing calls today (#1160), gets connected to the download pipeline as part of #759.
- **#95 is built now**, not after the 30 Nov MusicBrainz upgrade. Single-song enrichment becomes #1210.
- **Deferred as recommended:** #423 and #424 go with M8 (#424 is reworded to one shared yt-dlp
  version range), and #426 goes with #911 Phase 2.
- **Accepted as recommended:** questions 2, 3, 4, 8 and 9, plus #759's split. #759 part C is now
  #1209, and the mirror's signed build records are #1211.

**The queue as built:**
1. #949 wording, and #397's false comment (Haiku)
2. #984 (Sonnet)
3. #216 and #387 (Opus)
4. #273 (Sonnet)
5. #329 (Sonnet)
6. #95 (Sonnet)
7. #759 part A, plus connecting the tempo step (Opus)
8. #397 (Opus)
9. #759 part B, Fill gaps, including tempo (Opus)

---

# Plan for the 15 reopened issues

Status: COMPLETE (all 15 checked). Written 22 Sept 2026 by the planning
step (an Opus agent standing in for Fable, which is out of usage credit).
Nothing in the repository, on GitHub, or in any issue has been changed.

Checked against: branch `work/after-1.10.8`, which is `origin/alpha`
(1.13.0-alpha.71, commit 0f552a71) plus four docs-only commits. Every claim
below was checked in the code on this branch. Where the issue text or the
sweep's notes were wrong or incomplete, that is said.

How to read the sizes: S = under half a day, M = about a day, L = several
days, XL = a project in its own right.

---

## At a glance

| Issue | What it is really about | Size | Recommendation |
|---|---|---|---|
| #216 | Pre-release test uses "starts with 0."; also verbose logging is switched off by every download and every visit to Settings | M | Build now (batch 3) |
| #387 | What's New never shows: wrong pre-release test AND the old version is overwritten before the screen can read it | M | Build now, with #216 (batch 3) |
| #273 | Only 1 of 5 tools is really checked (the FFmpeg check can never fire) | M | Build now (batch 4) |
| #329 | lofty cannot write MKV/WebM/OGV at all, even in its newest version | M | Build with FFmpeg (batch 5) |
| #949 | Eight stale milestone lines in four files | S | Build now (batch 1) |
| #984 | Checksum file is called `SHA256SUMS`; the workflow never looks for that name | S | Build now (batch 2) |
| #397 | Nothing is checked; GAMDL's 13 dependencies float freely | L | Wording fix now (batch 1); hash lock later (batch 7) |
| #759 | No re-run; nothing records completed stages; a timed-out album gets no manifest at all | M + L (+ XL) | Split: A now (batch 6), B later (batch 8), C new issue |
| #95 | Three parts, none wired; single-song downloads get no enrichment at all | M (+ L) | Build after 30 Nov (batch 9); song part to a new issue |
| #431 | Service-status half is dead and replaced; cross-service half is a stub | S | Delete dead code (batch 1), then close |
| #423 | BBC iPlayer settings shape | S | With M8 |
| #424 | Shared yt-dlp: already installed once; real gap is a tested version range | S-M | With M8 |
| #426 | Per-service sign-in readout | M | With #911 Phase 2 / Spotify going public |
| #393 | Memory watching: cause fixed by #370; hard to measure reliably | L | Close as not planned |
| #352 | Shared codec detection: only a small part is shareable | M, blocked | Close as not planned |

---

## Proposed queue

Every batch is committed and pushed to `work/after-1.10.8` on its own, then
reviewed by Codex (new work only), with one PR to `alpha` later when the
maintainer says so. Every `feat`/`fix`/`perf` commit ends with a
`Release-Note:` line (or `Release-Note: none`), because the project requires
it for direct commits too. Batches 3 onwards depend on the maintainer's
answers to the questions below.

| # | Batch | Issues | Size | Model | Why this model |
|---|---|---|---|---|---|
| 1a | Wording corrections | #949 (eight lines); #397's false "computes SHA-256" comment and "installed and verified" log line | S | Haiku | Pure text edits at known lines |
| 1b | Remove the dead service-status system | #431 (then close it; also finishes #1069's clean-up) | S | Sonnet | Deletions across Rust, TypeScript, the IPC audit list and docs; the compiler and `check_ipc_commands.py` catch mistakes |
| 2 | Offline installer checksum | #984: read `SHA256SUMS`, stop on a missing checksum, fix the two wrong mirror names | S | Sonnet | Small shell change in the same shape as the existing code; security-relevant but only runs on a manual build, and Codex reviews it |
| 3 | Pre-release behaviour and What's New | #216 + #387 | M + M | Opus | Moves startup-only actions out of six settings commands; must not reopen the "verbose logs leak on full releases" hole; this area has a history of backed-out attempts (#1175) |
| 4 | Helper-program update checks | #273 | M | Sonnet | Ordinary read-only network checks and version parsing |
| 5 | Loudness tags for MKV and friends | #329 | M | Sonnet | Well-understood FFmpeg copy-with-tags, with explicit instructions: temp file, swap only on success, same file locks |
| 6 | Enrichment gaps made visible and true | #759-A | M | Opus | Changes every download's enrichment step (early manifest write, per-stage records) inside a 2,100-line task that can be aborted mid-way |
| 7 | Hash-locked pip installs | #397 | L | Opus | Security-sensitive; a mistake stops new users installing GAMDL on a whole platform |
| 8 | "Fill gaps" runner | #759-B | L | Opus | Rewrites tags and sidecars in people's existing libraries, alongside downloads that may be running |
| 9 | MusicBrainz follow-ups | #95 (region fallback, recording ID tag, AcoustID second pass) | M | Sonnet | Contained changes using existing lookups; wait until after the 30 Nov 2026 MusicBrainz search upgrade |

**Not in the build queue (deferred, blocked, or to close):**

- #423 and #424 — first tasks of M8 (#102), which has no engine code yet.
- #426 — with #911 Phase 2, once Spotify (M9) is public.
- #393 and #352 — recommended to close as not planned.
- New issues to file (if the maintainer agrees): #759-C (split the inline
  enrichment block so the Apple-Music-dependent stages can be re-run —
  XL); "enrich single-song downloads" (from #95 — L); signed build
  records on the MeedyaDL-Tools mirror (from #984); tiered session-log
  retention (only if wanted, from #393 — S).

If the maintainer wants security ahead of features, batch 7 can move up to
straight after batch 3 without affecting anything else.

---

## Questions for the maintainer (with recommended answers)

1. **#216:** Drop item 5 ("pre-release access control"), treating the
   existing developer-access gate on the Alpha channel as the answer?
   *Recommended: yes.*
2. **#387:** Show What's New after every stable upgrade, and on pre-release
   channels fold it into the pre-release notice so there is one dialog, not
   two? *Recommended: yes.*
3. **#273:** For FFmpeg, record at install time where it came from and that
   build's date, and offer an update only when the same source has a build
   more than about 30 days newer? And ask the MeedyaDL-Tools mirror to fill
   in MP4Box's version in its `versions.json` (blank today)?
   *Recommended: yes to both.*
4. **#329:** Build the FFmpeg copy-with-tags writer for MKV (and WebM, OGV,
   MKA), rather than just taking those types off the list?
   *Recommended: build it.*
5. **#423 and #424:** Do both as the first tasks of M8, and reword #424 to
   "one tested yt-dlp version range shared by every service, plus a guard
   on removal" (dropping per-service versions, which one Python cannot
   hold)? *Recommended: yes.*
6. **#426:** Wait and do it as part of #911 Phase 2, once Spotify is public?
   *Recommended: yes.*
7. **#431:** Delete the old service-status system (finishing #1069's
   clean-up), close #431, and leave the cross-service wrapper to #110?
   *Recommended: yes.*
8. **#984:** Make a missing checksum stop the offline-installer build, and
   file tamper protection for the mirror (signed build records) as a
   separate issue? *Recommended: yes to both.*
9. **#397:** Lock GAMDL's and votify's whole dependency tree with committed
   hashes (several days' work), while still allowing the deliberate
   "untested version" and downgrade installs without a check, clearly
   labelled? *Recommended: yes.*
10. **#393:** Close as not planned? *Recommended: yes.*
11. **#759:** Accept the three-part split (A and B here; C as a new issue);
    have Fill gaps use the person's current settings and only switched-on
    stages; and drop the tempo stage from the list, in line with #1160's
    recommendation to remove the tempo service? *Recommended: yes to all
    three.*
12. **#95:** Try the user's own region first, then the link as found; write
    the standard MusicBrainz recording ID tag; move single-song enrichment
    to a new issue; and start only after the 30 November MusicBrainz
    upgrade? *Recommended: yes to all four.*
13. **#352:** Close as not planned? *Recommended: yes.*

---

## What I could not verify

- Nothing was built or run: no `cargo test`, `npm run test` or type-check.
  This was planning only; every finding is from reading the code, GitHub,
  PyPI and crates.io.
- #273: the exact output of `MP4Box -version` for GPAC's new `26.x`
  numbering, and whether the mirror's `versions.json` always matches the
  files on its `latest` release (I assume the mirror's own publishing job
  writes both together, but did not read that job).
- #329: which players read loudness tags from MKV files (general knowledge:
  mpv does, many do not). Not tested.
- #393: how the WebView's memory could be measured on macOS (general
  knowledge about WebKit's separate processes). Not tested.
- #397: that a universal hash lock installs cleanly through MeedyaDL's own
  pip on every platform and Python version. Not tried.
- #95: that GAMDL actually fails on a music-video link for another
  country's catalogue. Inferred from the storefront-mismatch handling
  (#666), not reproduced.
- #984: the offline-installer build itself was not run. "Not used
  recently" is based on the last 40 Release runs only.

---

## Issue by issue

## #216 — Verbose logging and the pre-release notice ignore every 1.x pre-release

**What is missing today**

- The backend decides "is this a pre-release?" by checking whether the
  version starts with `0.` (`src-tauri/src/services/config_service.rs:479`).
  The frontend does the same (`src/App.tsx:637`). The app is 1.13.0-alpha.71,
  so every alpha, beta and release candidate is treated as a full release:
  verbose logging is switched off at every start, and the pre-release notice
  never appears.
- The update checker already uses the rule that works: a version is a
  pre-release if it contains a `-` (`src-tauri/src/services/update_checker.rs:856`).
- **A second bug the sweep did not mention.** The verbose reset lives inside
  `load_settings`, and `load_settings` is not only called at startup. The two
  download pre-checks call it too: `check_cookies_before_download`
  (`src-tauri/src/commands/settings.rs:408`) and
  `check_output_path_before_download` (`:512`), both run by
  `DownloadForm.tsx:504` and `:589` before every download. So on a full
  release, someone who switches verbose logging on in Settings and then
  starts a download has it silently switched off again in the backend
  (`config_service.rs:545` sets the live flag to false). Opening the
  Settings page does the same: it reloads through `get_settings`
  (`SettingsPage.tsx:344`, `commands/settings.rs:125`), which runs the
  reset and shows the switch as off. So on a full release, verbose logging
  only lasts until the next download or the next visit to Settings — which
  defeats it as a tool for gathering detail for a bug report. The
  project's own rule (in CLAUDE.md, "Changing one stored setting") says
  only startup should call `load_settings`; six
  command handlers break that rule: `get_settings` (`commands/settings.rs:125`),
  the previous-settings read in `save_settings` (`:183`), the two download
  pre-checks (`:408`, `:512`), `export_settings` (`:694`) and
  `import_settings` (`:801`).
- The notice's "install the stable release" offer can never appear for a
  1.13 alpha user either, even after the version rule is fixed. It only shows
  when a stable release is a *newer* version
  (`PrereleaseNoticeModal.tsx`, the `latestUpdate` selector), and stable
  (1.10.8) is older than 1.13.0-alpha.71. The app already works out the right
  thing to offer — `rollback_version` / `rollback_tag`
  (`update_checker.rs:857`), shown on the Updates page
  (`UpdatesPage.tsx:201`) — but the notice does not use it.
- Two pieces of help text still describe the old rule ("v0.x.x"):
  `AdvancedTab.tsx:539` and `:555`, and the header comment of
  `PrereleaseNoticeModal.tsx`.

**What already exists to build on:** the `last_seen_version` setting, the
notice modal, the rollback fields, and the working "contains a `-`" rule.

**What "done" means**

1. On an alpha, beta or release-candidate build, verbose logging stays on
   across restarts if the person left it on.
2. On a full release it is still switched off at each start — and ONLY at
   start. Starting a download or opening Settings no longer switches it
   off.
3. The first launch of each new pre-release version shows the notice.
4. The notice offers the stable release through the existing rollback
   option when one exists.
5. One shared "is this a pre-release" rule in Rust and one in TypeScript,
   each with tests, and all help text updated.

**Size: M.** The rule change is tiny, but doing it properly means moving the
startup-only actions out of the `load_settings` callers that are not
startup (six command handlers), which touches settings code.

**Dependencies:** shares code with #387 (both need the version from *before*
the upgrade, and both change the same startup step in `App.tsx`). Build them
together. No MeedyaSuite-core or milestone dependency.

**Risk:** settings loading. A mistake here could stop settings loading or
reintroduce the verbose-logging leak on full releases (verbose logs can
contain cookies and tokens, which is why full releases switch it off). Does
not touch downloads directly, but the pre-checks run before every download.
No release-pipeline or settings-file format change.

**How it will be verified**

- `cargo test` in `src-tauri/`: new tests for the shared rule
  (`1.13.0-alpha.71` and `1.0.0-rc.38` are pre-releases; `1.10.8` is not;
  `0.49.2` is not, because it has no `-`), and a test that the pre-check
  path no longer resets the verbose flag.
- `npm run test` and `npm run type-check`: a test for the TypeScript rule and
  for when the notice is shown.
- Manual: run an alpha build, switch verbose on, restart (stays on), start a
  download and reopen Settings (stays on). On a full-release build: switch it
  on, start a download (stays on), restart (off).

**Decisions for the maintainer**

- Item 5 of the issue, "pre-release access control", was never built. The
  Alpha channel is already hidden behind developer access
  (`GeneralTab.tsx:308`), and anyone can download a pre-release straight
  from GitHub, so real access control is not possible for a public
  repository. **Recommended: record that the existing developer-access gate
  on the Alpha channel is the answer, and drop item 5.**
- Note, not a decision: old pre-1.0 builds (`0.x`, with no `-`) would count
  as full releases under the new rule. Nobody runs those any more, so this
  is accepted.

**Recommendation: build now.** It is a real bug on every pre-release build,
and the hidden second bug affects full releases too.

---

## #387 — "What's New" after an upgrade

**What is missing today**

- The "What's New" block that closed this issue is inside
  `PrereleaseNoticeModal.tsx` and is only a link to the Updates page. It
  shows no release notes.
- It never appears, for two separate reasons, both confirmed:
  1. It is gated on the version starting with `0.` (`App.tsx:637`).
  2. Even on a `0.x` build it could not appear, because the backend has
     already overwritten `last_seen_version` with the current version before
     the frontend reads it. `load_settings` does the overwrite
     (`config_service.rs:506-534`), and startup calls it twice before the
     frontend asks (`lib.rs:242` and `lib.rs:512`). The frontend's
     "previous version" is therefore always the current version.
- There is no What's New component anywhere in `src/`.

**What already exists to build on:** release notes are already fetched and
shown on the Updates page, with the download section stripped
(`UpdatesPage.tsx:41`, `stripDownloadSection`). The backend can already
join together the notes for several versions in a row
(`aggregate_intermediate_release_notes`, used at `update_checker.rs:1165`),
and can fetch a release by tag (`dependency_manager.rs:592`). Stable
releases have curated plain-English notes; pre-releases have generated ones.

**What "done" means**

1. The backend keeps the version from before the upgrade and hands it to the
   frontend (for example a small new command, or a field returned at
   startup), instead of overwriting it first.
2. On the first launch after an upgrade, a modal shows the plain-English
   notes for the new version (the notes of every version skipped over, when
   several were skipped), with the download section removed.
3. If the notes cannot be fetched (offline), the modal still appears with a
   link to the release page, and the app does not wait on the network to
   start.
4. It is shown once per upgrade, never on a fresh install, and never on top
   of the setup wizard or another first-launch dialog.

**Size: M.** One new backend value, one fetch by tag, one modal. The notes
rendering is reused.

**Dependencies:** build with #216 (same startup step, same "previous
version" fix). No other dependency.

**Risk:** low for downloads and settings. The network fetch must never
block startup. Release notes are text from GitHub, shown through the same
markdown path the Updates page already uses, so it gets the same
link-handling rules.

**How it will be verified:** Vitest tests for when the modal shows (upgrade:
yes; same version: no; fresh install: no; wizard showing: no); a Rust test
that the previous version is kept and handed over before being replaced;
manual check by editing `last_seen_version` in a test settings file.

**Decisions for the maintainer**

- **Show What's New on every channel, or stable only?** Alpha cuts several
  versions a day (alpha.69, .70 and .71 were all on 21-22 Sept), so a modal
  on every alpha update would get tiresome. **Recommended: show it after
  every stable upgrade; on pre-release channels, fold a short version into
  the pre-release notice (#216), so there is one dialog, not two.**
- Design note, not a decision: the notes are fetched from GitHub at run
  time. They cannot be built into the app, because pre-release notes are
  only written when the release is published, after the app is built.

**Recommendation: build now, together with #216.**

---

## #273 — Update checks for the helper programs

**What is missing today**

- The only tools checked are FFmpeg and N_m3u8DL-RE
  (`update_checker.rs:830-833`). mp4decrypt, MP4Box and MediaInfo are never
  checked.
- **The FFmpeg check does not work either — the sweep missed this.** It
  compares against the version number in BtbN's latest release tag, but
  that tag is literally `latest` (checked live: `latest | Latest Auto-Build
  (2026-09-21 13:55)`). There is no number in it, so the comparison
  (`update_checker.rs:1433-1451`) always answers "no update". In practice
  only N_m3u8DL-RE (tag `v0.6.0-beta`) is really checked: 1 of 5, not 2.
- How each tool is installed matters for how it can be checked:
  - mp4decrypt and MediaInfo come only from the MeedyaDL-Tools mirror
    (`dependency_manager.rs:1047-1053` and `:1135-1140`).
  - MP4Box comes from platform installers (Homebrew, `.pkg`, apt) with the
    mirror as a fallback (`:1119-1124`).
  - FFmpeg on macOS comes from evermeet.cx, on Windows and Linux from BtbN
    "master" builds, whose version string is not a normal version number.

**What already exists to build on**

- Reading the installed version works for all five tools
  (`get_tool_version`, `dependency_manager.rs:2581`).
- **The mirror repository already publishes the version of each tool it
  holds**, in `versions.json` at its root (checked live): mp4decrypt
  `1-6-0-641`, MediaInfo `v26.05`, FFmpeg `2026-09-20T13:32:39Z` (a date, not
  a version), and MP4Box blank.
- The Updates page already shows an Update button for any tool row and
  routes it through the existing install path (`UpdatesPage.tsx:87-92`,
  `:126`). Tools adopted from a package manager already get "Update via
  Homebrew" labelling (`update_checker.rs:1453-1468`). So this is backend
  work only.

**What "done" means**

1. mp4decrypt and MediaInfo installed from the mirror are compared with
   the mirror's `versions.json`, and an update shows when the mirror holds a
   newer one.
2. MP4Box is compared with GPAC's latest release when it came from the
   mirror or GPAC's own installer; when a package manager installed it, the
   check is left to that manager (#1084's later phase).
3. FFmpeg either gets a check that can actually say "newer" (for example by
   recording the build date at install time and comparing it with the
   mirror's or BtbN's build date), or is honestly marked as not checkable.
4. The version formats are normalised and tested (`1-6-0-641` against
   `1.6.0-641`; GPAC's jump from `2.4` to `26.07.0`).

**Size: M.** Three small checks, one version-format helper, tests. FFmpeg is
the fiddly part.

**Dependencies:** none blocking. Overlaps #1084 (package-manager updates);
keep the line clear: #273 is for tools MeedyaDL installed itself. Relies on
the mirror's `versions.json` staying published (maintainer's own repository).

**Risk:** low. Read-only network checks. The one thing to avoid is offering
an "update" that reinstalls the same version, which is why mirror-sourced
tools must be compared with the mirror, not with upstream.

**How it will be verified:** `cargo test` for the version-format helper and
the comparison (including "mirror version blank" → no update offered);
manual check on the Updates page against a machine with an older tool.

**Decisions for the maintainer**

- **FFmpeg: build a date-based check, or say it cannot be checked?**
  **Recommended: build the date-based check.** Record, at install time,
  where FFmpeg came from and that source's build date (BtbN's release date
  on Windows and Linux; evermeet.cx or the mirror on macOS), and compare
  with the same source later. Only offer an update when the gap is more
  than, say, 30 days, because these builds are republished almost daily
  and a daily "update available" would be noise. FFmpeg adopted from a
  package manager is left to that manager, as for the other tools.
- Should the mirror's `versions.json` also fill in MP4Box (currently blank)?
  That is a change in the MeedyaDL-Tools repository. **Recommended: yes,
  as a small follow-up there; until then, compare MP4Box with GPAC's own
  releases.**

**Recommendation: build now.**

---

## #329 — Loudness (ReplayGain) tags for MKV, WebM and OGV

**What is missing today**

- `.mkv`, `.webm` and `.ogv` are in the ReplayGain list
  (`replaygain_service.rs:517-519`) and are sent to the lofty tag writer
  (`:301-310`). lofty 0.22.4 (`Cargo.lock`) has no Matroska support and does
  not recognise a video-in-Ogg file. **Checked further than the sweep:** the
  newest lofty (0.25.4, released 20 Sept 2026) still has no Matroska
  support. Its source tree has no Matroska module, and the upstream request
  (Serial-ATA/lofty-rs#218, "Support EBML (Matroska, WebM) files") is still
  open. So a lofty upgrade will not fix this.
- The result: FFmpeg spends time measuring loudness on the file, then the
  tag write fails, and the failure is only logged at debug level (`:236`).
- The code comment "lofty supports these" (`:516`) is false. `.mka` from the
  acceptance criteria was never added.
- **Who is affected today:** anyone who chose MKV as the music-video
  container (Settings > Codec & Resolution offers "MKV (Matroska)",
  `QualityTab.tsx:163`). WebM and OGV are never produced by anything in the
  app today; they arrive with YouTube (M10).

**What already exists to build on:** FFmpeg is a required tool and already
runs the loudness measurement. FFmpeg can also write tags into MKV, WebM and
Ogg files by copying the streams into a new file without re-encoding.

**What "done" means**

1. For MKV (and MKA, WebM and OGV), loudness tags are written by copying the
   file with FFmpeg into a temporary file with the tags added, then
   replacing the original only if FFmpeg succeeded. Nothing is re-encoded.
   Chapters, attachments and all streams are kept.
2. A failure is reported as a plain warning in the activity log, not hidden
   at debug level.
3. The false comment is corrected.

**Size: M** for the real feature (the FFmpeg copy-with-tags path, the
file-locking the other tag writers use, and tests). **S** for the honest
fallback (take the three types off the list so no time is wasted, and fix
the comment).

**Dependencies:** none for MKV. WebM and OGV only matter from M10 (YouTube).

**Risk:** this rewrites a video file the user owns. A bad copy would damage
it. That is why it must write to a temporary file and only swap it in when
FFmpeg reports success, and must take the same per-file write lock as the
fingerprint and tag stages. It also costs a full file copy (music videos are
typically 100-500 MB). It does not touch settings or the release pipeline.

**How it will be verified:** unit tests for the FFmpeg argument list (no
FFmpeg needed); a test that makes a tiny MKV with FFmpeg, tags it, and
reads the tag back, which skips itself when FFmpeg is not installed (CI has
no FFmpeg; `ci.yml` never installs it). Manual: download a music video with
MKV selected and check the tags with `ffprobe` or MediaInfo.

**Decisions for the maintainer**

- **Build the FFmpeg copy-with-tags writer, or stop processing these file
  types until lofty supports them?** Player support for loudness tags in
  video files is thin (mpv reads them; many players do not).
  **Recommended: build it for MKV now using FFmpeg (it is the only one a
  user can produce today), and let WebM, OGV and MKA use the same code.**
  If the answer is instead "do not build it", the "S" fix (take the types
  off the list, correct the comment) moves into batch 1.

**Recommendation: build now** (batch 5). Everything lands on the same
branch before one PR, so there is no need for a temporary "take them off
the list" step first.

---

## #949 — Old milestone numbers left in code comments and the project plan

**What is missing today** (all confirmed, and a repository-wide search found
no others):

- `src/types/index.ts:992` — YouTube settings called a "stub for M9"
  (should be M10).
- `src-tauri/src/models/settings.rs:638` — YouTube arrives in "milestone M9
  (v2.1.0)" (should be M10, v2.2.0).
- `src-tauri/src/services/engine_runner.rs:459` "milestones M9/M10" (should
  be M8/M10); `:475` "planned for v2.1.0" (yt-dlp is first needed in v2.0.0
  as BBC iPlayer's fallback); `:481` "milestone M10" and `:497` "planned for
  v2.2.0" for get_iplayer (should be M8, v2.0.0).
- `Project_Plan.md:538` and `:579` — "YouTube (M9) and BBC iPlayer (M10)".

The help pages users see are already right. Other hits in the repository
(`.claude/memory/project_multi_service_groundwork.md`, `RiskPill.tsx`,
`fs_walk.rs`, `commands/README.md`, `upstream-engine-watch.yml`) were checked
and are correct.

**What "done" means:** those eight lines match M8 = BBC iPlayer (v2.0.0),
M9 = Spotify (v2.1.0), M10 = YouTube (v2.2.0), and a search for the old
pairings finds nothing outside the changelog and dated audit files.

**Size: S** (minutes). Comments and two error strings that nothing in the
app calls yet.

**Dependencies:** none. **Risk:** none (the two error strings come from
placeholder code no screen reaches). **Verified by:** `cargo check`,
`npm run type-check`, and the search above.

**Decisions:** none.

**Recommendation: build now** (batch 1).

---

## #423 — BBC iPlayer entry in the per-service settings

**What is missing today:** `PerServiceSettings` has `apple_music`,
`spotify`, `youtube` and `engine_priority` only
(`src-tauri/src/models/settings.rs:655-671`). No BBC iPlayer settings in
Rust or TypeScript. The earlier attempt (commit 738dad80, 17 lines, fields
"quality" and "download_subtitles") was never merged and is five months
stale. The BBC iPlayer settings tab is a "Coming Soon" placeholder
(`BBCiPlayerTab.tsx`, 43 lines) and is not in the tab list
(`SettingsPage.tsx:200-214`).

**What already exists:** the per-service settings container, the
`engine_priority` map already keyed with `"bbc-iplayer"` in its own comment,
and the `bbc-iplayer` platform in `engines.toml:217-228`.

**What "done" means:** a `BbcIPlayerSettings` block, stored under the key
`"bbc-iplayer"`, with the options M8 actually uses, mirrored in TypeScript,
defaulting safely for older settings files.

**Size: S** for the empty shape. But what should go in it depends on how
M8 drives get_iplayer, which has not been designed. M8 has no engine code at
all yet (#102, last status 25 May 2026).

**Dependencies:** M8 (#102). **Risk:** low if new fields are optional, but a
field added now with the wrong type is expensive to change later. The
comment at `settings.rs:678-700` records how one wrong-shaped field makes
the whole settings file fail to load and reset to defaults.

**Decision for the maintainer: build the shape now, or as the first step of
M8?** **Recommended: as the first step of M8.** An empty or guessed shape
now does nothing for users and risks a settings-file field that has to be
changed later. Keep #423 open, put it in the M8 milestone, and note that it
is the first M8 task.

**Recommendation: build later, with M8.**

---

## #424 — Shared engines (yt-dlp for both YouTube and BBC iPlayer)

**What is missing today:** the `Engine` struct has no record of which
services use each engine (`engine_registry.rs:27-48`). The earlier attempt
(commit 0b0c75a6, a `used_by_platforms` list) was never merged.

**Checked further than the sweep:**

- "Install once" is already true by design. Every pip engine is installed
  into the one Python that MeedyaDL manages
  (`pip_engine_service.rs:88-97`), so there can only ever be one yt-dlp.
- The real risk the issue worries about — one service's clean-up removing
  yt-dlp from under the other — cannot happen today:
  `uninstall_pip_engine` (`pip_engine_service.rs:189`) has no callers.
- yt-dlp is installed with no version limit
  (`pip install --upgrade yt-dlp`, `:97`), unlike GAMDL and votify, which
  have tested version ranges in `tool-versions.toml` (`[gamdl]` at line 157,
  `[votify]` at 717). One Python can hold only one yt-dlp, so "YouTube needs
  one version and BBC iPlayer another" cannot be solved by installing both.
  The honest answer is one tested version range for yt-dlp that both
  services must work with.
- yt-dlp is referenced by four platforms, not two: YouTube, YouTube Music,
  BBC iPlayer and BBC Sounds (`engines.toml:204, 214, 228, 239`).

**What "done" means:** (1) the registry reports which services use each
engine; (2) yt-dlp gets a tested version range in `tool-versions.toml`,
used by every install and update, like GAMDL; (3) any future uninstall or
"disable engine" action refuses, or warns, when another enabled service
still needs the engine.

**Size: S-M** once M8 starts. The listing is small; the version range and
its tests are the real work, and they need a yt-dlp version to test against,
which only exists once M8 downloads something.

**Dependencies:** M8 (#102) is the first user of yt-dlp, then M10 (#104).
**Risk:** low now; the version-range part touches installs.

**Decision for the maintainer:** **Recommended: do this as part of M8,**
and reword the issue to what is really left: a tested version range for
yt-dlp shared by all its services, and a guard on removal. Drop the
"different versions per service" idea, because one Python cannot hold two.

**Recommendation: build later, with M8.**

---

## #426 — Sign-in status for each service

**What is missing today:** no `get_service_auth_status` command in any
branch (the earlier commit bef237b6 was never merged). The only sign-in
status command is `wrapper_auth_status` (`commands/wrapper.rs:154`), which
covers the Apple Music wrapper only. The Authentication section of Settings
holds just the Apple Music cookies tab (`SettingsPage.tsx:207`). The YouTube
and BBC iPlayer tabs are placeholders and are not in the tab list.

**What already exists to build on (more than the issue says):**

- Apple Music: cookie check (`check_cookies_before_download`,
  `commands/settings.rs:407`), wrapper sign-in status (`wrapper.rs:154`),
  and MusicKit credential validation.
- Spotify: the dispatch check already reports "allowed / consent needed /
  developer access needed / missing DLL or .wvd / daily cap reached"
  (`check_spotify_dispatch_allowed`, `commands/spotify_anti_ban.rs:284`),
  and the Spotify tab lets you choose how it signs in.
- BBC iPlayer needs no sign-in (UK location only).
- YouTube: nothing yet (M10).

**What "done" means:** one command returns, for each service that exists,
whether it is ready and why not in plain words; the Authentication section
shows that readout; each service's detail stays on its own tab.

**Size: M** for Apple Music and Spotify (joining existing checks into one
readout, plus one screen). Grows with each service after that.

**Dependencies:** overlaps #911 Phase 2 ("a unified Settings > Accounts
page"), which is explicitly held until more than one service is live.
Spotify (M9) is still behind a hidden developer-only switch, so today only
Apple Music users would see anything. YouTube needs M10.

**Risk:** low. Read-only checks. The cookie check must not log cookie
values.

**Decision for the maintainer: build a readout now for Apple Music (and
Spotify for developers), or wait until M9 is public?** **Recommended:
wait, and do it as part of #911 Phase 2 when Spotify goes public.** With one
public service, a per-service readout duplicates what the cookies tab and
the download pre-checks already say.

**Recommendation: build later** (with #911 Phase 2 / M9 going public).

---

## #431 — Frontend wrappers for the service-status and cross-service commands

**What is missing today:** no `checkServiceStatus` or `checkCrossPlatform`
in `src/lib/tauri-commands.ts`, and none of the four types in
`src/types/index.ts`. The store (`serviceStatusStore.ts`) and banner
(`ServiceStatusBanner.tsx`) both have type-checking switched off
(`@ts-nocheck`) because the pieces they import do not exist. The Rust
command files claim the wrappers exist
(`commands/service_status.rs:10-14, 23`; `commands/smart_download.rs:9-13,
22`) — wrong.

**Checked further than the sweep — the two halves are in very different
states:**

- **Service status is dead and already replaced.** The backend fetches
  `service-status.json` from `main` (`services/service_status.rs:33-34`).
  That file has never been on `main` (checked: it exists only on this
  branch, and the live address returns 404). Nothing calls the command.
  The job it was meant to do is done by the remote feature-availability
  system (`feature_flag_service`, shipped). #1069's last comment already
  says what is left is to delete this old transport, and #1160 lists the
  command as unreachable. The audit tool's exemption list records it as
  "superseded rather than unused" (`tools/audit-checks/check_ipc_commands.py:190`).
- **Cross-service search is a stub.** `smart_download::check_cross_platform`
  (`services/smart_download.rs:69-100`) returns only the link it was given;
  the search itself says "not yet implemented". The feature is #110, which
  is targeted at v3.x and needs Spotify and YouTube working first. A
  frontend wrapper for a stub adds nothing.

**What "done" means (recommended version):**

1. Delete the old service-status system: `commands/service_status.rs`,
   `services/service_status.rs`, `models/service_status.rs`,
   `service-status.json`, `serviceStatusStore.ts`, `ServiceStatusBanner.tsx`,
   its registration at `lib.rs:1314`, its entry in the audit exemption list,
   and the mention in `check_user_agent.py:16` and CLAUDE.md. This finishes
   #1069's remaining clean-up.
2. Correct the false "wrapper exists" comments in `smart_download.rs`.
3. Close #431: the service-status half is removed rather than finished, and
   the cross-service wrapper belongs to #110.

**Size: S.** Deletions and comment fixes.

**Dependencies:** none. **Risk:** low. `cargo build` and
`tools/audit-checks/check_ipc_commands.py` will catch a missed reference.
Nothing users can reach changes.

**Decision for the maintainer: delete the old service-status system and
close #431, moving the cross-service wrapper to #110?** **Recommended:
yes.** This matches the project's own precedent: the dead enforcement
helpers for this same system were deleted "so nobody wires enforcement to
the wrong backend" (CLAUDE.md, "Remote feature availability").

**Recommendation: build now (the deletion), then close.**

---

## #984 — Offline installer bundles mirror tools without checking them

**What is missing today**

- The checksum check in the offline-installer build only looks for a file
  named `<asset>.sha256` or `checksums.txt` on the mirror release
  (`.github/workflows/release.yml:1228-1253`). The mirror publishes its
  hashes as `SHA256SUMS` (checked live on the `latest` release: 68 lines,
  "hash, two spaces, file name" — the same format the `checksums.txt`
  branch already reads with `awk '$2 == name'`). So every tool takes the
  "No published checksum found" warning path (`:1270`) and is bundled
  unverified.
- **Found while checking:** two places name the mirror as
  `MWBMPartners/MeedyaDL-Tools`, which does not exist (GitHub returns 404):
  `release.yml:1455` (text written into the offline installer's
  source-offer file) and `THIRD_PARTY_LICENSES.md:426`. The real repository
  is `MeedyaSuite/MeedyaDL-Tools` (as `release.yml:1156` and
  `tool-versions.toml` already say). Both lines only describe what would
  happen if MeedyaDL ever modified a component, so this is a wording fix,
  not a live broken promise — but it is in licence text, so it should be
  right.

**What already exists:** the pip pin (done), the fail-loud download and
archive checks (done), the full verify-and-refuse code path (waiting for a
name that matches).

**What "done" means:** the offline-installer build reads `SHA256SUMS`,
refuses to bundle any mirror tool whose hash does not match, and the two
repository names are corrected.

**Size: S.** A third file name to try, in the same shape as the existing
two.

**Dependencies:** none. (#1076, still open, is the same fix for the app's
own first-run downloads; it is a separate change in Rust.)

**Risk:** release pipeline. But the offline installer is only ever built by
hand (`bundle_engines`, manual `workflow_dispatch` only, `release.yml:83`,
`:1001-1003`), and no run of it appears in the last 40 Release runs, so
ordinary alpha/beta/stable releases are not affected.

**How it will be verified:** `actionlint` (already in CI); a local dry run of
the lookup snippet against the live mirror release, confirming a matching
hash for, say, `mp4decrypt-macos-aarch64.tar.gz`; and, if the maintainer
agrees, one manual offline-installer build.

**Decisions for the maintainer**

- **Should a missing checksum stop the offline-installer build, now that
  the mirror always publishes one?** Today it only warns. **Recommended:
  yes, stop the build.** The warning path exists only for mirror releases
  from before checksums were published, and the offline installer is always
  built by hand, so a stop is noticed straight away.
- The honest limit (already noted on the issue): a checksum on the same
  release as the file protects against a damaged download, not against
  someone who can change the release. Real tamper protection would mean
  signed build records on the mirror, checked by this workflow — a change
  in the MeedyaDL-Tools repository. **Recommended: file that as its own
  follow-up issue; do not hold this fix for it.**

**Recommendation: build now** (batch 2, security quick fix).

---

## #397 — Checking what pip installs

**What is missing today**

- Nothing is checked. After installing GAMDL, the code runs
  `pip show gamdl --verbose` and logs the install folder
  (`gamdl_service.rs:196-212`). The comment above it says it "compute[s]
  SHA-256 of the installed gamdl package" — false — and the log line
  "installed and verified successfully" (`:214`) claims a check that never
  happens.
- No `--require-hashes` or hash list anywhere. The SHA-256 settings in
  `tool-versions.toml` are for binary tools only (and are commented out,
  `:47-53`).
- **The real exposure is wider than GAMDL itself.** GAMDL is held to a
  tested range (`gamdl>=3.0,<=3.8.5`, with `--only-binary=gamdl`), but all
  13 of GAMDL's own dependencies are open-ended (checked on PyPI for 3.8.5:
  `httpx>=0.28.1`, `pillow>=12.0.0`, `pywidevine>=1.8.0`,
  `yt-dlp>=2025.10.22`, and nine more). So every install pulls whatever
  version of each is newest on PyPI that day. A bad release of any of those
  lands on users' machines with no check.
- There are four places that run `pip install`, and all would need
  covering: GAMDL install (`gamdl_service.rs:166`), GAMDL exact-version
  reinstall (`:293`), generic engines (`pip_engine_service.rs:97`) and
  votify (`spotify_service.rs:98`). A fifth is the offline installer's pip
  step in `release.yml`.

**What already exists:** tested version ranges for GAMDL and votify in
`tool-versions.toml`, including a per-platform ceiling for Linux ARMv7
(stuck on GAMDL 3.8.1 because no newer wheel exists there).

**What "done" means (recommended version)**

1. For each tested GAMDL and votify release, the repository holds a lock
   file listing the exact version of every package in its dependency tree
   and the SHA-256 of every file PyPI offers for each. (Tools such as
   `uv pip compile --universal --generate-hashes` produce this, covering all
   platforms and Python 3.10 to 3.14, which matters because people can reuse
   their own Python, #1017.)
2. The routine install and upgrade use that lock with
   `pip install --require-hashes`, so pip refuses any file whose hash does
   not match.
3. A CI check fails if the lock does not match `tool-versions.toml`
   (so raising the tested ceiling cannot forget the lock).
4. The deliberate "install an untested version" path and the manual
   downgrade still work, without hashes, and say so plainly.
5. The false comment and "verified" log line are corrected (this part goes
   into the quick-fixes batch straight away).

**Size: L** (several days). Lock generation, a CI check, four install
paths, the ARMv7 ceiling, and testing an install on each platform.

**Dependencies:** best done after #984 (same idea for the offline
installer). Interacts with the GAMDL audit routine: every ceiling bump would
also regenerate the lock. Needs the maintainer's decisions below.

**Risk: high for installs.** A wrong or incomplete lock makes GAMDL fail to
install on some platform, which leaves the app unable to download at all
for new users there. Must be tested by a real fresh install on macOS,
Windows and Linux before release. Security-sensitive. Touches the release
pipeline only if the offline installer also moves to the lock.

**How it will be verified:** `cargo test` for building the install
command; a CI job that installs from the lock on Linux, macOS and Windows
runners into a clean Python; manual fresh-install checks on each platform,
including Linux ARM64, before a stable release.

**Decisions for the maintainer**

- **Lock the whole dependency tree with hashes, or keep today's approach and
  only correct the wording?** **Recommended: lock it.** The issue's own
  priority note says "must have for RC", MeedyaDL stores sign-in details,
  and the open-ended dependencies are a larger exposure than GAMDL.
- **Fetch hashes from PyPI at install time instead (the issue's
  suggestion)?** **Recommended: no.** Hashes fetched from the same place as
  the files, over the same connection, add almost nothing; hashes committed
  to the repository at review time are what protect people.
- **Should an above-ceiling "Untested" install still be allowed without a
  hash check?** **Recommended: yes**, with a plain warning that it is not
  checked. The person has already chosen to install something unreviewed.

**Recommendation:** correct the false wording now (batch 1a); build the
lock as batch 7 (it can move up to straight after batch 3 if security
should come first).

---

## #393 — Watching WebView memory

**What is missing today:** nothing measures memory anywhere (no match in
`src` or `src-tauri/src` for any memory reading, and no process-inspection
library in `Cargo.toml`). The half that landed: trimmed activity-log lines
are saved to `logs/session-<date>.log` (`activityStore.ts:31-39`) and kept
for a flat 30 days (`lib.rs:190-209`). The tiered retention in the issue
(30 days if errors, 7 if clean, never if exported) was not built.

**What has changed since the issue was written:**

- The cause of the original 14 GB incident (#370) was fixed: the activity
  log is capped at 10,000 lines (`activityStore.ts:18`), only visible rows
  are drawn (`ActivityLog.tsx`, `QueueListVirtualized.tsx` use
  `useVirtualizer`), and incoming lines are batched once per frame.
- Every activity-log line is now written to disk anyway, by the persistent
  on-disk log (#541, `utils/activity_log.rs:60`). So the "don't lose trimmed
  lines" half is largely a duplicate of #541 — it only adds a longer
  retention.
- Measuring the WebView's memory is hard to do reliably. On macOS the
  WebView runs in separate system-managed processes that are not simple
  children of the app; on Windows it is separate `msedgewebview2` processes;
  the browser's own `performance.memory` exists only on Windows' engine and
  is deprecated.

**What "done" would mean if built:** a check every minute that reads the
WebView's memory on all three platforms, trims and tells the user when it
passes a threshold, and suggests a restart if trimming does not help.

**Size: L** if built properly on three platforms, with real uncertainty on
macOS. The retention tiers alone are **S**.

**Dependencies:** none. **Risk:** low for downloads, but a false alarm that
trims the log or nags about restarting would annoy people.

**Decision for the maintainer: build it, or close it as not planned?**
**Recommended: close as not planned**, noting that #370's fixes removed the
cause, that #541 already keeps every line on disk, and that reliable memory
measurement across three platforms is costly. Reopen only if a new report of
high memory use arrives. (If you want the tiered retention, it can be a
small separate issue.)

**Recommendation: close as no longer wanted** (pending the maintainer's
answer).

---

## #759 — Re-running missing enrichment steps without re-downloading

**What is missing today**

- Detection landed (`services/enrichment_gaps.rs`, a 20-stage list at
  `:56-77`, the `scan_enrichment_gaps` command, and the badge in
  `LibraryScanPage.tsx`). Nothing re-runs a stage.
- Nothing records which stages finished: the manifest's `enrichment` field
  is always written as empty (`download_queue/helpers.rs:820`). So every
  stage stored inside the audio files (ReplayGain, AcoustID, MusicBrainz,
  Apple Music metadata) always shows as "unknown".
- **Found while checking — this undercuts the issue's own example.** The
  manifest is written near the END of the enrichment task
  (`processing.rs:3853`, inside the task started at `:1639`). When
  enrichment runs out of time, the completion task aborts that task
  (`processing.rs:4034-4056`). So for a first download whose enrichment
  times out, **no manifest is written at all**, and Library Scan — which
  finds albums by their manifest — cannot see that album. The comment at
  `helpers.rs:816-819` ("Initial manifest write happens before enrichment
  runs") is wrong.
- The stage list is out of date. It has a BPM stage, but the tempo service
  is called by nothing (#1160). It lacks three stages added since: Lyricsfile
  sidecars (Step 2g, `processing.rs:2821`), the cross-service cover upgrade
  (Post-step 1c, `:2111`), and song.link links (Steps 6c/6d, `:3669`,
  `:3757`).

**How big the runner really is:** the enrichment pipeline is one inline
block of about 2,100 lines inside the download's completion task
(`processing.rs` roughly 1,860 to 3,960), not a set of separate steps. But
many stages already have a "do this for one album folder" function that a
runner could call without touching that block: ReplayGain
(`process_replaygain_for_directory`), AcoustID
(`process_acoustid_for_directory`), Enhanced LRC, WebVTT, Rich SRT,
subtitle embedding, ASS, Lyricsfile (`generate_*_for_directory`), and the
cover fallback (`ensure_cover_present`). The stages that need Apple Music
data (metadata tags, word-level lyrics, animated artwork, artist video,
music videos, MusicBrainz, cover upgrade, song.link) live inline and would
need that block split up first.

**Suggested split (three parts):**

- **759-A (M): make the gaps visible and true.** Write a basic manifest
  before enrichment starts, so a timed-out album still appears in Library
  Scan. Record each stage's completion as it finishes (updating the same
  manifest, using the existing safe-write helper). Read ReplayGain and
  AcoustID tags from the files so older downloads stop showing "unknown".
  Bring the stage list up to date.
- **759-B (L): a "Fill gaps" runner for the folder-based stages.** A
  per-album and "fill all" action in Library Scan that re-runs only the
  missing folder-based stages listed above, respecting the user's current
  settings, with the same file locks, cancellation and activity-log lines
  as a download.
- **759-C (XL): the Apple-Music-dependent stages.** Split the inline
  enrichment block into separate steps so they can be re-run too. This is a
  refactor of the code every download runs through; it should be its own
  issue and its own project.

**What "done" means:** for this issue, 759-A plus 759-B. 759-C gets its own
new issue, which this one links to.

**Size: M + L** for A and B; **XL** for C.

**Dependencies:** the Library Scan page (#717, done). 759-C overlaps any
future work on the enrichment pipeline. The tempo stage depends on the
maintainer's #1160 decision (remove the tempo service or keep it).

**Risk:** 759-A touches every download's enrichment step (extra manifest
writes) and the manifest file format (only filling in a field that already
exists and older versions already ignore). 759-B rewrites tags and sidecar
files on albums already in the user's library, so it must use the same file
locks and never touch the audio itself. 759-C is high risk: it reshapes the
code every download runs.

**How it will be verified:** `cargo test` for the manifest write-back and
merge (including "manifest written before a timeout"), the tag readers, and
each runner stage against sample folders; Vitest for the new buttons;
manual: force a short enrichment timeout, confirm the album appears in
Library Scan with the right stages missing, click Fill gaps, confirm only
those stages ran.

**Decisions for the maintainer**

- **Accept the three-part split, with 759-C as a new issue?** **Recommended:
  yes.**
- **Should Fill gaps use the user's settings as they are now, or as they
  were when the album was downloaded?** **Recommended: as they are now**,
  and only for stages the user currently has switched on. The manifest does
  not record the old settings, and "fill what I have switched on" is what
  people expect.

**Recommendation: build 759-A now (it also fixes a real hole); 759-B after
it; 759-C later as its own project.**

---

## #95 — MusicBrainz: region-aware links, single-song lookups, and the AcoustID bridge

**What is missing today** (all confirmed)

1. **Region rewrite:** `rewrite_apple_music_storefront`
   (`musicbrainz_service/mod.rs:1395`) is only called by its own tests
   (`:1615-1649`). Apple Music video links found on MusicBrainz are passed
   to GAMDL unchanged (`processing.rs:3597-3640`, via
   `download_music_video_by_url`), so a link for another country's catalogue
   can fail for a user in a different country.
2. **Single-song lookup:** a song link gets no MusicBrainz lookup. It is
   broader than that: **a single-song download gets no Apple Music metadata
   enrichment of any kind**, because the metadata fetch only accepts album
   links (`metadata_tag_service.rs:2367-2371`,
   `.filter(|p| p.content_type == "album")`). No issue tracks that.
3. **AcoustID bridge:** the MusicBrainz recording ID that AcoustID returns
   is only written to the debug log (`acoustid_service.rs:368-369`). The
   MusicBrainz lookup is always given `musicbrainz_recording_id: None` and
   `apple_music_url: None` (`processing.rs:3240-3242`). AcoustID (Step 4) and
   the MusicBrainz lookup run side by side, so the ID is not even available
   when the lookup starts. No MusicBrainz ID tag is written to files at all
   — not even from the ISRC matches that already work (nothing in
   `tags.toml` defines one).

**What already exists:** the recording-by-ID lookup and the shared
relationship reader (`musicbrainz_service/relations.rs`), the ISRC path
that already finds recording IDs for most tracks, and per-track Apple
Music links in the album metadata.

**What "done" means (recommended version)**

- Part 3, reshaped: write the standard MusicBrainz recording ID tag (the one
  MusicBrainz Picard, Plex and beets read) for every track the existing ISRC
  lookup matches; and for tracks ISRC did not match, use AcoustID's
  recording ID in a second, small pass after both have finished.
- Part 1: before downloading a MusicBrainz-found Apple Music video, try it
  with the user's own region first, then fall back to the link as found.
- Part 2: split out into a new issue, "enrich single-song downloads", since
  it is a gap in the whole enrichment step, not a MusicBrainz feature.

**Size: M** for parts 1 and 3 (reshaped). Part 2 as its own issue is
**L** (single-song metadata, then every enrichment step learning to work on
one track).

**Dependencies:** MeedyaSuite-core#75 and MeedyaDL #1119 plan to move the
MusicBrainz lookup itself into the shared library. The work here is
app-specific (what MeedyaDL does with an ID once found), which stays in
MeedyaDL after that move, so it is not blocked; but doing it before #1119
means that migration has one more caller to carry. #1120's remaining work
is operational (the 30 November MusicBrainz search upgrade checks) and does
not conflict.

**Risk:** moderate. Adds a tag write to every matched file (through the
existing file locks) and changes the order of two enrichment steps, which
can lengthen enrichment when both AcoustID and MusicBrainz are on. The video
region fallback only affects the opt-in music-video path.

**How it will be verified:** `cargo test` for the second-pass logic
(ISRC hit: no AcoustID fallback; ISRC miss plus AcoustID ID: lookup by ID)
and the region-fallback order; a tag round-trip test on a sample M4A;
manual check of the tag in MusicBrainz Picard or MediaInfo.

**Decisions for the maintainer**

- **Keep the storefront rewrite and wire it in, or delete it?**
  **Recommended: wire it in as "user's region first, then the link as
  found"**; it is small and fixes a real failure for non-US users.
- **Write the MusicBrainz recording ID tag into files?** **Recommended:
  yes, using the standard name other tools already read.**
- **Split single-song enrichment into a new issue?** **Recommended: yes.**
- **Do this before or after the #1119 move to MeedyaSuite-core?**
  **Recommended: after the 30 November MusicBrainz upgrade has settled**
  (early December), so nobody is changing MusicBrainz code during the
  cutover.

**Recommendation: build later** — after 30 November 2026.

---

## #352 — Using the shared codec-detection code from MeedyaSuite-core

**What is missing today:** nothing in MeedyaDL uses `meedya_codecs`
(no match in `src-tauri/src`). ffprobe detection is still local
(`metadata_tag_service.rs:2071` `detect_audio_info`, `:2257`
`resolve_codec_from_ffprobe`), and `mediainfo_service.rs` (308 lines) still
parses MediaInfo itself. `meedya-core` is pinned to an old commit
(`Cargo.toml:108`, rev 0a4654ff), while `meedya-fingerprint` and
`meedya-lyrics` follow MeedyaSuite-core's `main` branch (`:435-436`).

**Checked on MeedyaSuite-core's GitHub `main` (not the local copy):**

- `meedya-codecs` exists with `ffprobe.rs`, `mediainfo.rs`, `tool_path.rs`.
  Its `AudioCodec` now includes AAC binaural, AAC downmix and HE-AAC.
- Its ffprobe result has no duration. MeedyaDL uses the duration to spot
  damaged, near-empty files (`metadata_tag_service.rs:2217`).
- MeedyaDL has 11 codec kinds (`gamdl_options.rs`); four have no shared
  equivalent: AAC Legacy, HE-AAC Legacy, HE-AAC Binaural, HE-AAC Downmix.
- **The part that matters cannot be shared as the issue imagines.** Apple's
  AAC variants (standard, binaural, downmix, legacy) look identical to
  ffprobe; MeedyaDL tells them apart by what it asked GAMDL for
  (`resolve_codec_from_ffprobe` takes the requested codec as a hint, see its
  doc comment at `:2230-2245`). A shared detector with no hint cannot do
  that. Only "run ffprobe and read its output" is truly shareable — a few
  dozen lines, not ~300.

**What "done" would mean if kept:** MeedyaDL calls the shared crate to run
ffprobe and MediaInfo and read their output; keeps its own
"which Apple variant was this" decision; loses no checks (duration); and
all existing codec and suffix tests still pass.

**Size: M** for that narrow version, plus a MeedyaSuite-core change to add
duration, plus aligning how the shared crates are pinned. **Blocked** on
that upstream change and on the pinning.

**Risk:** codec detection decides the `[Dolby Atmos]` / `[Lossless]` file
name suffixes; a mistake would mislabel people's files. The saving is small.

**Decision for the maintainer: keep, narrow, or close?** **Recommended:
close as not planned**, with a note saying why (only a small part is
shareable, the Apple-specific decision must stay in MeedyaDL, and the risk
outweighs the saving). If shared use across Meedya apps becomes a goal
later, reopen it narrowed to "run and read ffprobe/MediaInfo only", after
MeedyaSuite-core adds duration.

**Recommendation: close as not planned** (pending the maintainer's answer).

---
