---
name: project-alpha-main-drift
description: alpha↔main↔beta divergence — RESOLVED at the content level 2026-07-24 (EPIC #1040 Phases 1-2 merged; alpha is content-complete). Ancestry closure (Phase 3, `-s ours` merge of main) and promotion to main (Phase 4) remain gated on owner go-ahead. Do NOT run realign-alpha before Phase 4 promotion.
metadata:
  type: project
---

# Alpha / main drift (updated 2026-07-24 — Phases 1-2 DONE, content reconciled)

**Status (2026-07-24): CONTENT RECONCILED.** EPIC #1040 Phases 1–2 landed on
`alpha` this session (PR #1041 — 13 missing fragments F1–F13; PR #1044 — the
Phase-2 prep rebase, 64 commits of GAMDL 3.8.2–3.8.4 + #1034 security work +
docs, bundled with the `ci.yml` alpha/beta/rc PR-gating fix; plus follow-on
hardening PRs #1047/#1048/#1049). `alpha` is now content-complete at
`1.12.0-alpha.35`, verified against the full CI matrix (`cargo test --lib`
1516/0, `clippy --all-targets` clean, `npm test` 560).

**What's still open:** Phase 3 (ancestry closure — a content-no-op `-s ours`
merge of `origin/main` into `alpha` to restore an honest merge-base and kill
the "N behind" illusion for good) and Phase 4 (promotion `alpha` → `beta` →
`main` at the next stable cut) are **both gated on explicit owner go-ahead** —
not urgent, since the content gap that made this drift dangerous is already
closed. See `.github/HANDOFF.md` "★★ LATEST — Session 2026-07-24" for the
live phase tracker, and the audit trail below for the analysis that
established content parity.

**Audit trail (2026-07-24):**
`.github/audits/alpha-main-drift-content-analysis-2026-07-24.md` (the content
probe — of main's 130 substantive commits: 119 present on alpha, 9
superseded, 1 N/A, 1 partial; the pivotal finding that "alpha 681 behind
main" was a **git-ancestry illusion**, not a real content gap — alpha forked
from main 2026-04-20 and absorbed main's content via squash-imports rather
than merges, so `git cherry`/commit-counting lied) and
`.github/audits/alpha-main-realignment-runbook-2026-07-24.md` (the executable
Phase 0–4 plan). Rollback anchors if anything needs undoing:
`backup/{alpha,prep}-pre-realign-2026-07-24`, `backup/prep-pre-rebase-2026-07-24`.

**Still true — do NOT violate:**
1. **NEVER run `realign-alpha`** before Phase 4 promotion — it fast-forwards
   `alpha` onto `main` and would clobber alpha's unique commits (Spotify,
   #911 UI, Profile Bundle, Lyricsfile, SQLite index, GAMDL 3.6–3.8.4, brand).
2. **NEVER let a naive `git merge main` land** on alpha outside the Phase 3
   runbook — it silently resurrects deleted files (nightly/weekly/monthly
   cron workflows, `upstream-gamdl-watch.yml`, `protected-cron-channels.json`).

---

## Historical status (superseded by the above — kept for context)

The section below documents the drift as it stood 2026-07-03, before the
2026-07-24 realignment. Kept for archaeology; do not treat as current state.

## Current topology (2026-07-03)

```text
main (1.10.1) ⊇ beta (1.9.4)          [beta ahead of main: 0]
alpha (1.11.0-alpha.25) = +40 feature commits neither beta nor main have
main/beta = 655-679 commits ahead of alpha
```

- The 40 alpha-only commits = M9 Spotify, brand refresh, #911 multi-service
  UI, Profile Bundle, Lyricsfile, SQLite index, GAMDL 3.6/3.7 gates. These
  must flow **alpha→beta→main** at the next stable cut.
- **DO NOT run `realign-alpha`** (fast-forwards alpha onto main) before those
  land in main — it would clobber the 40 commits.
- The full alpha↔main merge has a **~69-file conflict surface** (version
  strings, workflows, docs, Rust sources) — a dedicated multi-session
  workstream, still the biggest pending item.
- Two consolidation prep PRs exist (2026-07-03, NOT merged): **#967** (alpha —
  gamdl v3.8/v3.8.1, security fixes, artwork, syllable-lyrics fix, dep mirror,
  v2-drop) and **#968** (main — dep bumps + dependabot.yml fix routing future
  dep PRs through alpha). Merge #968 then #967; then tackle the big reconciliation.

## Historical origin (2026-05-23)

**Status:** OPEN — tracked in #873 as a dedicated rationalisation PR.

## What happened

PR #855 (GAMDL 3.6 EPIC) merged to `alpha` on 2026-05-22 — wrapper-v2, aac-web codec rename, native muxing, four new `GamdlFeature` variants, settings UI changes. It never landed on `main`.

On 2026-05-22 / 2026-05-23, `main` accumulated 10 commits of its own: rclone bundling + multi-endpoint updater fallback (#863), v1.10.0 release commits (#864), version bump (#865), release-please manifest fix (#866), plus auto CHANGELOG/SECURITY updates.

**v1.10.0 stable was released from main on 2026-05-22 WITHOUT the GAMDL 3.6 code.** Users on v1.10.0 with GAMDL ≤3.5.x are fine; users who upgrade GAMDL to 3.6+ on v1.10.0 will see broken behaviour (no aac-web rename, no wrapper-v2, etc.).

## Why merge to alpha will conflict heavily

Attempting `git merge origin/main` into an alpha-based branch surfaces ~60 conflict blocks across 19 files. Both sides touched the same surfaces:

- `gamdl_capabilities.rs` — alpha added 4 v3.6 variants, main is unchanged
- `dependency_manager.rs` — alpha added wrapper-v2 health checks, main added rclone TOOL entry
- `tool-versions.toml` — alpha bumped GAMDL ceiling, main added `[rclone]` section
- `settings.rs` / `tauri-commands.ts` / `types/index.ts` / `AdvancedTab.tsx` / `settingsStore.ts` — both sides added new fields/types

## Why this is a separate PR (not bundled with v3.7)

1. **Risk isolation.** Mixing 60 conflict resolutions with new feature work makes regression bisection nearly impossible.
2. **Reviewability.** A "rationalisation" PR is reviewable for merge correctness without also reviewing feature additions.
3. **Sequencing flexibility.** The drift PR can land before OR after the v3.7 PR — order doesn't matter. Either way, the next stable cut includes both.

## Recommended approach (see #873 for full options)

**Option A (preferred):** manual merge with careful conflict resolution. Preserve BOTH sides everywhere they add different things. Resolve version stamps to main's `1.10.0`. Verify with cargo check + npm type-check + cargo test.

**Option B:** cherry-pick only PR #863 (the substantive code work) — accept that the release-please housekeeping commits never make it to alpha. Faster but loses git ancestry.

## Why this drift happened (preventive)

PR #863 (rclone + multi-endpoint) was opened against `main` and merged there. PR #855 was opened against `alpha` and merged there. Neither was followed by a forward-port. Then #864/#865/#866 stacked on top of #863 on main, and v1.10.0 was tagged from main — but main never got #855.

The pre-existing `realign-alpha.yml` workflow handles `main → alpha` fast-forward AFTER a stable cut, but only if alpha has no commits ahead of main. Alpha had #855 ahead, so realign-alpha would have lost it. The workflow needs an update to support "alpha catches up to main while preserving alpha's commits" (a true merge, not a fast-forward reset).

## How to apply

- When reasoning about what's in v1.10.0 stable: assume GAMDL 3.6 code is NOT there yet (until #873 lands)
- When working on alpha-branch features: rebase carefully — alpha is behind main on the rclone/updater work
- When this drift is finally resolved: confirm via `git log --oneline origin/alpha..origin/main` returns empty (or just the channel version bumps)

## Related

- #873 — the rationalisation PR
- #855 — GAMDL 3.6 EPIC that merged to alpha only
- #863 — rclone + multi-endpoint that merged to main only
- [[project_release_pipeline_gotchas]] — release-please gotchas; this drift is a new gotcha worth adding
- [[project_gamdl_v37_audit]] — v3.7 work that surfaced this drift
