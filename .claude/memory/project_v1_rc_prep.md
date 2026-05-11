---
name: v1 release status
description: Current state of the v1.x release line — v1.0.0 is GA, v1.1.0 is published as Pre-release pending user testing, audit v2 fully landed
type: project
originSessionId: 2ab3d7da-8f4e-4331-8327-4ea82ab8e25f
---
**Current state (2026-05-10):**

- **v1.0.0** — GA stable. Promoted from `v1.0.0-rc.1` by tagging the same commit (`31e5c97`) with bumped manifests (`1.0.0-rc.1` → `1.0.0`). Marked as GitHub "Latest". 20 platform assets published.
- **v1.1.0** — Built and published as **Pre-release**. Stays as Pre-release until the user finishes manual testing — that's the project's release-promotion policy (see `feedback_release_promotion_policy.md` in personal memory). Bundles every audit-v2 implementation PR + the recent helper migrations.
- **v1.0.0-rc.1** — Original RC, still on GitHub as Pre-release. Kept for history.

**Promotion policy (load-bearing — do NOT auto-flip):**

The user gates GitHub "Latest" / `isPrerelease=false` on manual testing. release-please-action and `release.yml` will publish a stable tag as `isPrerelease=false` by default; the user wants it set BACK to `true` until they've tested. Don't auto-correct what looks like a "wrongly-flagged prerelease" — assume it's intentional. See `feedback_release_promotion_policy.md`.

**v1 RC blockers (originally tracked here):**

- ✅ #232 — Frontend tests for DownloadForm / DownloadQueue / ActivityLog / SetupWizard. Closed 2026-05-09 with 70 new component tests across 4 PRs (#728-#731).
- 🟡 #182 — QA: font scaling + screen-reader testing. Open. User-side QA (not implementation work); not a code blocker.

**v1.0 GA milestone — historic state:**

- #386, #125, #111, #109 still open as of late April. Status not refreshed in this update — verify with `gh issue list` if asked.

**Service expansion milestones (unchanged from prior state):**

- v2.0 — BBC iPlayer (M8): #102
- v2.1 — Spotify (M9): #295, #110, #101 (1 closed)
- v2.2 — YouTube (M10): #103, #104

**Audit v2 (fully landed 2026-05-10):**

8 of 8 findings shipped across 8 PRs (#733–#740). The new internal primitives are catalogued in `project_audit_v2_helpers.md`. Each new MeedyaDL feature should reach for these helpers rather than hand-rolling the same shape.

**Recent post-v1.1.0 work:**

- #743 / #744 — `wrapper_decrypt_ip` exposed in Settings (closed). Remote-wrapper LAN setups (e.g., RPi) can now configure the decryption host:port; previously this was hard-coded to `127.0.0.1:10020` and silently failed mid-download for users running the wrapper off-host.
- #741 — Release pipeline cleanup PR (open at the time of this memory write). Disables weekly + monthly cron releases, adds change-detection gate to nightly cron, fixes the "Release in progress…" placeholder persistence bug.
- #746 — Docs PR explaining the three-address wrapper-on-LAN setup pattern (open).

**How to apply:**

- When asked about RC/release status, lead with v1.0.0 = GA + v1.1.0 = Pre-release pending testing.
- Don't claim v1.x is unreleased. Don't quote the v0.49.1 baseline from old memory — the cadence has caught up and stabilised.
- Refresh from `gh release list --limit 5` + `gh pr list --state open` if quoting specific numbers; this memory captures the shape, not the live counts.
