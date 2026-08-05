# GitHub Issue Reconciliation — 2026-08-05 (delta)

**Scope:** delta sweep on top of [`issue-reconciliation-2026-08-03.md`](issue-reconciliation-2026-08-03.md)
(which took open issues 97 → 42). Verified the current open set against the **actual code** on
`claude/gamdl-v3-8-5-review-gs36zl` (HEAD after the dependency-security commits), via a sequential
**Fable 5** deep-analysis pass (grep+read real source, never commit titles/docs). Executed by the
maintainer session.

**Result:** 1 close · 1 relabel · 5 partial confirmed-kept-open (already commented) · rest genuinely
open. Closed-issue spot-check (12 across the 2026-08-03 tiers + the batch A–D closes) — **all hold,
zero regressions, zero wrongly-closed**.

## Actions executed
- **CLOSE #964** — help-doc wrapper/codec drift for GAMDL 3.8+ is resolved (option (b), version-conditional
  note). Verified: `help/quality-settings.md` + `HelpViewer.tsx` twin ("every codec except ALAC downloads
  with cookie-based auth alone on 3.8+"), `help/wrapper.md` + twin (full version-by-version breakdown),
  `help/faq.md` / `help/downloading-music.md` / `help/fallback-quality.md` carry no stale claims. Prose
  matches the runtime gate `is_wrapper_dependent_runtime()` (`gamdl_options.rs:242`, test
  `only_alac_on_v38_plus`). Both `.md`⇄`HelpViewer` twins updated (`fcd2d5d` + `88fd46b`).
- **RELABEL #1012** — added `good first issue`. `fetch_syllable_lyrics` IPC still registered
  (`lib.rs`) with zero `invoke()` callers under `src/`; batch-D `test_lyrics_connection` (#934) calls the
  **service-layer** fn directly, not the IPC — so the dead registration is genuinely removable. Context
  comment posted.

## Kept-open partials (verified, already carry accurate on-branch progress comments — NOT re-commented)
- **#961** artwork follow-up — landed: storefront geo-lock warnings (`apple_music_api.rs`,
  `animated_artwork_service.rs`, `download_queue/companions.rs`) + cross-variant album-cover fallback + key
  logging. Deferred: amp-api `fetch_animated_artwork_fallback` + setting, 3-way "unavailable" log split,
  Plex-aware Linux dot-rename.
- **#971** Media-User-Token — landed: `#HttpOnly_` cookie-parse fix + `apply_apple_music_headers` helper.
  Deferred: actual token threading (catalog/editorialVideo calls still pass `None`, explicit TODOs at
  `apple_music_api.rs:1069`, :1598).
- **#974** native fMP4 concat — landed: native init+segment concat as **FFmpeg-failure fallback**.
  Deferred: native-primary ordering, `+faststart` remux of the native path, parallel fetch, live playback
  verification.
- **#1001** v2→v3 guided migration — landed: backend `recommended_upgrade_target()` +
  `LAST_WRAPPER_V1_VERSION`. Deferred: the guided-migration UX (no callers yet; decision-pending on shape).
- **#1002** validate wrapper-less non-web codecs — landed: `AssetsApiUnlocksLossyCodecs` +
  `is_wrapper_dependent_runtime()` wired into gap-fill. Deferred: live QA of real 3.8.x wrapper-less
  Atmos/AC3 downloads (smoke-test retargeted at 3.8.5, not yet run).

## Genuinely open — no change (verified live in code)
- **#978** votify ignores `output_path`/`temp_path` (`votify_options.rs:330` `..Self::default()`; success
  check counts files under `settings.output_path`, `download_queue/processing.rs:4714`).
- **#998** CSP `connect-src` has no Sentry ingest host (`tauri.conf.json:29`).
- **#1013** ARMv7 GAMDL wheel pin still required (`tool-versions.toml:680`, `update_checker.rs:344`).
- **#1034** narrowed to F10 (MusicKit-token rotation) — nothing on-branch changes it.
- **#1069** service-status → intAppsAPI: dead `service_dispatch` helpers deleted, but
  `services/models/commands/service_status.rs` + never-rendered `ServiceStatusBanner.tsx` still in tree
  (cleanup pending).
- **#1075** HELP_TOPICS still a static inline array (no build-time codegen).
- **#1076** per-release SHA256SUMS on the tools mirror + dynamic verification not done.
- Roadmap/epics (no verification needed): 100 101 102 103 104 108 109 110 111 125 182 537 696 847 856 858
  859 860 861 862 872 907 908 909 911 924 1040.

## Anomalies found (and dispositions)
- **#182** ("qa: font scaling + screen reader testing") is a genuine open QA tracker under the #125 a11y
  epic (the 2026-08-03 doc flagged the #182⇄#125 duplicate pair but did not close #182). **Disposition:
  left open** — a concrete QA-verification task is more useful tracked than folded into the broad epic; not
  a true duplicate.
- **CLAUDE.md self-contradiction — FIXED (doc-only).** The batch-C bullet claimed "Atmos + AC3 stay
  wrapper-dependent on every version", contradicting both the shipped code (`is_wrapper_dependent_runtime()`
  → **ALAC-only** on GAMDL ≥3.8) and CLAUDE.md's own 3.8-audit paragraph (which lists atmos/ac3 as unlocked)
  and #1002's own progress comment. Verified against `gamdl_options.rs:242-248` + doc-comment + test
  `only_alac_on_v38_plus`; corrected the CLAUDE.md clause to match the code. **No code change** — the code
  is correct; only the doc was stale.
