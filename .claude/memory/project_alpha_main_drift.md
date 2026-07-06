---
name: project-alpha-main-drift
description: alpha↔main↔beta divergence — alpha has 40 feature commits neither beta nor main have; main/beta are 655-679 ahead of alpha. Big reconciliation still pending. Do NOT run realign-alpha before promoting alpha's commits.
metadata:
  type: project
---

# Alpha / main drift (ongoing — updated 2026-07-03)

**Status:** OPEN. The original #873 rationalisation approach is superseded;
the drift persists and grew. See `.github/HANDOFF.md` for the current
session state.

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
