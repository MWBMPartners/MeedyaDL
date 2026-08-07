---
name: project-v17-bumper-bundle
description: v1.7 bumper-bundle session (2026-05-17/18) — 22 issues processed across 7 themed bundles on `feat/v1.7-bumper-bundle`. 13 closed, 9 status-commented as deferred/ongoing trackers.
metadata:
  type: project
---

**Working session: 2026-05-17 evening → 2026-05-18 early morning.** Single
branch `feat/v1.7-bumper-bundle` carrying the entire backlog work; one PR at
the end per the user directive "do not create PRs for these unless absolutely
necessary — we'll do one big PR at the very end".

## What landed (commits, in order)

| # | Commit | Closes | What |
|---|---|---|---|
| 1 | `b030d8d` | #549 | Apple Music uploaded-video URL decision (accept + audit log + defer wiring) |
| 2 | `8f4500c` | #457 | `help/metadata-mapping.md` — comprehensive tag-by-tag reference (10 sections) |
| 3 | `3a6caf1` | #522 | GAMDL version management UI + `install_gamdl_version` (force-reinstall path for downgrades) |
| 4 | `7721147` | #462 + #463 | Queue search/filter + bulk-select-and-bulk-ops |
| 5 | `f1989b0` | #466 | Auto-backup + restore for settings/queue/history (10-snapshot cap, atomic restore) |
| 6 | `fcaac13` | #464 | Lifetime download stats — aggregated from `history.json` (no separate stats file) |
| 7 | _audit_  | #551 | Verified contract trait + GAMDL impl + DEV_NOTES.md checklist all already shipped |
| 8 | `afa72dd` | #536 | `is_likely_motion_art_url` defensive guard in `download_music_video_by_url` |
| 9 | `1510571` | #558 | MV filename Tier 2 — Apple Music Catalog `/music-videos/{id}?include=albums` |
| 10 | `088823f` | #559 | MV filename Tier 3 — parent album context override (precedence: Tier 3 > Tier 2 > Tier 4) |
| 11 | `42476be` | #572 | Phase 1 MVP diagnostic bundle composer (review modal + GitHub URL prefill) |

Plus 1 issue closed via "evaluated + rejected": #266 (Tidal/Qobuz/Amazon Music).

## What stayed open (status comments added, deferred with concrete plans)

- **#487 + #537** — EPICs for the filename-safety / video-asset audit. Concrete progress recorded; full fs-rename sweep is multi-PR.
- **#352, #353** — meedya-codecs / meedya-fingerprint integrations. Crates exist + are wired in `Cargo.toml`, but the migration needs paired enum-alignment work in MeedyaSuite-core (Phase B). Detailed 3-phase plan posted on each.
- **#596** — LyricsFile (.lyrics) format. **Hard upstream block** — no `meedya-lyrics` crate exists in MeedyaSuite-core yet. MeedyaDL side is consumer-only.
- **#111** — i18n migration. Groundwork is in place (i18next + 3 locales) but only Sidebar uses `t()`. ~6-10 follow-up PRs needed; per-area migration plan posted.
- **#125** — a11y umbrella. Substantial coverage already shipped (high-contrast, 3 colour-blind themes, reduced-motion, skip-nav, 34 components with ARIA attrs, modal focus-trap, keyboard-shortcuts dialog). Remaining checklist items (icon-only `aria-label` audit, toast `aria-live`, screen-reader smoke test) posted as concrete 3-PR follow-up.
- **#696** — Tauri GTK4 migration tracker. Quarterly audit; next checkpoint 2026-08 or any upstream GTK4 milestone.
- **#295** — Odesli (song.link) integration. ~1-2 days of work; most value unlocks post-M8/M9/M10. Phased plan posted.

## Patterns to remember

### Drafts-file workflow for release-please PR bodies (PR #812, merged earlier in same session)

Release-please force-pushes its release branch on every sync; any manual body
edit gets wiped. The `.github/workflows/preserve-release-pr-body.yml` workflow
reads `.github/release-drafts/v<version>.md` from main after every release-
please run and re-applies it. New convention: write the user-facing notes once
into the drafts file, commit to main, workflow handles the rest.

Race condition seen during v1.6.0 cut: the drafts workflow merged 44 minutes
after the release-please PR, so v1.6.0 shipped with the empty `(release notes
omitted)` body and was recovered manually via `gh release edit`. From v1.7
onward the workflow guards this automatically. Documented in
[[project-release-pipeline-gotchas]].

### Bumper-bundle scope discipline

22 issues in one branch is the limit. Larger and the PR review becomes
unmergeable. Smaller and the round-trip overhead per release dominates the
work. The 7-bundle thematic grouping (Queue UX / Filename safety / MV /
Architecture / Diagnostics / a11y+i18n / Infra) worked well — each bundle is
~30-45 minutes of focused work, commits naturally cohesive.

### Defer-with-plan vs close-as-shipped

When an issue can't be fully closed (upstream-blocked, multi-PR scope,
ongoing-tracker), the right action is a **status comment with the concrete
next-step plan** + leave open. The comment should:
1. Document what's actually in place today (with file/line refs).
2. Identify the specific blocker.
3. Propose a phased migration path the next contributor can pick up.
4. Justify the deferral.

This is more useful than either "I'll get to it later" (which loses context)
or arbitrarily closing as won't-fix.

## Verification before final PR

- `cargo check` ✓
- `cargo test` ✓ — 11 new unit tests across `backup_service` (2), `stats_service` (4), `apple_music_api::mv_album_linkage_tests` (6), `diagnostic_bundle` (6), `download_queue::tests::motion_art_*` (4), `download_queue::tests::sanitize_fs_segment_*` (1)
- `npx tsc --noEmit` ✓
- `npm test -- --run` ✓ — 33 test files / 489 tests pass
- `node scripts/check-acknowledgements.mjs` ✓ — no drift
- `node scripts/check-upstream-licences.mjs` ✓ — no drift

## See also

- [[project-release-pipeline-gotchas]] — the v1.6.0 release-body recovery + the drafts-workflow rule.
- [[project-brand-identity]] — vendor (MeedyaSuite) vs product (MeedyaDL) convention, still observed.
- [[feedback-pr-squash-titles]] — applies to the eventual v1.7 release PR title.
