# GitHub Issue Reconciliation — 2026-08-03

**Scope:** all **97 open** issues on `mwbmpartners/meedyadl` reconciled against the
**actual codebase** at branch `claude/gamdl-v3-8-5-review-gs36zl` (= `feat/alpha-consolidated`
HEAD + the 3.8.5 admission commit `6fda618`). Every "close" verdict below is backed by
`file:line` evidence from the current tree, spot-checked by the maintainer session before
execution. Method: pull all open issues → triage by title/label → deep-verify each
"likely done / stale" candidate against real source (grep + read), never against commit
titles or docs.

> **Branch-reach caveat (EPIC #1040):** these fixes live on the alpha development line.
> The project's model is alpha-first — issues are closed when implemented on that line, not
> only when promoted to stable `main`. Closes are worded "verified on the alpha development
> line". If a fix is later found not to have been promoted, reopen.

**Result:** 40 close-as-done · 1 obsolete · 4 relabel/narrow · 4 duplicate pairs ·
42 genuinely-open · **10 confirmed-live bugs** (kept open, fed into the new-work proposals).

## Close-as-done (40) — code-proven

**Tier 1 — explicit in-code `#NNNN` markers (spot-checked: #994, #996, #1026, #980 all confirmed):**
#1074 (ceiling→3.8.5), #1026 (wrapper_decrypt_ip INI twin, `config_service.rs:1048`),
#994 (LazyLock regexes, `process.rs`), #993 (dedup-before-native, `uiStore.ts:468`),
#990 (dot-hidden artwork gap, `enrichment_gaps.rs:311`), #989 (no Box::leak, `enrichment_gaps.rs:442`),
#996 (stage-and-swap install, `dependency_manager.rs:531`), #980 (dotted-stem MV cover, `music_video_cover_embed.rs:149`),
#985 (pip-engine allowlist, `commands/updates.rs:242`), #984 (offline bundle pin+checksum, `release.yml`),
#977 (tag-input injection, `release.yml:165`), #1010 (stale webplayer token, `apple_music_api.rs:782`),
#1029 (wrapper sign-in modal, `lib.rs:1284`), #1017 (system-Python reuse, `python_manager.rs:596`),
#970 (header-hardened promo video, `apple_music_api.rs:3008`), #1045 (pr-security FPs, `pr-security.yml`),
#995 (channel-workflow lockfile, `alpha-release.yml:147`), #1043 (ci gates channel PRs, `ci.yml:59`).

**Tier 2 — exact ask implemented, verified:**
#1019 (429 auto-pause), #1020 (wrapper-v2 playback_ready+503), #1022 (Errno-13 transient retry),
#1023 (aART atom), #1008 (syllable token chain + hasLyrics=None), #992 (History reveal path),
#969 (word-timing on span presence), #975 (traceback redaction), #988 (pinned binstall/git-cliff),
#1032 (secret-scan FPs — 2 live alerts may need one UI dismiss), #1031 (actionlint), #1028 (ELI5 notes),
#1027 (prerelease notes), #1046 (ELI5 self-heal).

**Tier 3 — GAMDL audit cycle / superseded:**
#1018 (3.8.4, superseded by 3.8.5), #999 (3.8.1 + v2 drop), #962 (3.8), #925 (3.7.4),
#1009 (hold-at-3.8.1 overtaken + hardening landed), #1071 (feature-flag client), #1070 (4-way UA policy),
#1024 (save-playlist doc note — landed; in-app HelpViewer copy sync is an optional follow-up).

## Obsolete → close as not_planned (1)
- **#386** macOS Touch Bar — hardware discontinued (2023); no Tauri Touch Bar API. Dead-end.

## Relabel / narrow scope (4)
- **#1034** security F1-F10 → narrow to **F10 only** (F1-F9 verified fixed in tree).
- **#1069** service-status → intAppsAPI → the "activate" half shipped; narrow to a cleanup issue
  (remove dead static-file `check_service_status` transport; `ServiceStatusBanner` never rendered).
- **#1033** release-notes follow-up → largely absorbed by #1046; only historic backfill of
  v1.1.0/v1.1.1/v1.2.0/v1.4.0 bodies remains (unverifiable from repo) — close referencing #1046.
- **#1012** unused `fetch_syllable_lyrics` IPC (registered `lib.rs:1228`, no frontend caller) →
  `good first issue`, 10-min remove-or-wire decision.

## Duplicate pairs (4)
- **#963 ⇄ #1002** — both "relax `is_wrapper_dependent()` on 3.8+". Merge.
- **#182 ⇄ #125** — #182 (font scaling + SR QA) is a subset of the #125 a11y epic.
- **#1033 ⇄ #1046** — same goal; #1046 delivered #1033's items.
- **#1071 ⇄ #1069** — #1071 is the done client slice of the #1069 umbrella.

## Genuinely open — keep (42)
**Roadmap/epics (21):** #100 #101 #102 #103 #104 #108 #109 #110 #537 #856 #858 #859 #860 #861
#862 #907 #908 #909 #911 #924 #1040.
**Follow-ups/for-consideration (15):** #934 #961 #963/#1002 #964 #965 #971 #972 #973 #974 #1000
#1001 #1013 #1014 #1021.
**QA/tracking/investigation (6):** #111 #125 #182 #696 #847 #872.

## Confirmed-live bugs (10) — kept open, prioritised for new-work
1. **#981** — BtbN FFmpeg linux64 is `.tar.xz` but declared `ArchiveFormat::TarGz`
   (`dependency_manager.rs:784`); `archive.rs` has **no xz decoder** → Linux x64 FFmpeg primary
   extract always fails, mirror is a silent SPOF. **[VERIFIED]**
2. **#978** — votify `from_settings` leaves `output_path`/`temp_path` `None`
   (`votify_options.rs:339` `..Self::default()`); success check counts files in `settings.output_path`
   → clean votify runs marked failed. **[VERIFIED]**
3. **#983** — `DownloadForm.tsx` single-URL validation uses `parseAppleMusicUrl` only → Spotify
   dispatch-gate preview unreachable from the form.
4. **#991** — Clear/Abort undo loops `startDownload` per-URL then unconditionally toasts "Re-queued N";
   >10 URLs silently dropped by the 10/min limiter (`downloadStore.ts:591`).
5. **#949** — milestone numbering scrambled: `help/supported-services.md` + in-app `HelpViewer.tsx`
   say Spotify=M8 / BBC=v2.2.0, contradicting README (M8=BBC, M9=Spotify, M10=YouTube). User-visible.
6. **#987** — `expected_sha256` plumbing exists (`archive.rs:519`) but zero call sites; GPAC nightly
   `.exe` downloaded+executed unverified.
7. **#982** — `.arg(format!("/D={}", dir))` breaks NSIS unquoted-tail requirement for paths with spaces.
8. **#997** — `sudo apt-get install -y gpac` spawned with no TTY (mirror fallback mitigates).
9. **#1011** — no `extend=audioTraits` on album fetch while code reads `audioTraits` → tag population
   depends on API volunteering the field.
10. **#998** — `tauri.conf.json` CSP `connect-src` lists no Sentry ingest host (latent until a DSN is wired).

No closed-but-regressed items were noticed.
