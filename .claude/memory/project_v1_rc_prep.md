---
name: v1 RC preparation status
description: Tracks v1 RC milestone state — version cadence has accelerated significantly since v0.32.0
type: project
originSessionId: 2ab3d7da-8f4e-4331-8327-4ea82ab8e25f
---
**Current version:** v0.49.1 (released 2026-04-27, the same day as v0.49.0). Release cadence: ~17 patches in ~17 days from v0.32.0 (2026-04-10) to v0.49.1 (2026-04-27).

**v1.0 Release Candidate milestone (open):** 5 closed / 2 open
- #232 — Frontend tests for DownloadForm/DownloadQueue/ActivityLog/SetupWizard
- #182 — QA: font scaling and screen reader testing

**v1.0 GA milestone (open):** 7 closed / 4 open
- #386 — macOS Touch Bar support
- #125 — Comprehensive a11y (screen readers, assistive devices, colour blindness)
- #111 — Complete i18n translations for all UI components
- #109 — Native SwiftUI UI for macOS (long-term, may slip)

**Service expansion milestones (renumbered since the original plan):**
- v2.0 — BBC iPlayer (M8): #102 open
- v2.1 — Spotify (M9): #295, #110, #101 open (1 closed)
- v2.2 — YouTube (M10): #103, #104 open

**Why:** Lance has been merging changes aggressively through April; tracking the version delta + remaining RC blockers helps frame "how close is v1.0?" conversations without re-querying GitHub. Two open RC blockers means v1.0 is feasible after #232 + #182 close.

**How to apply:** When the user asks about RC status or v1 readiness, lead with the current version + the two open RC blockers. Don't quote the old M8/M9/M10 mapping — Spotify is M9 and BBC iPlayer is M8 in the current milestones (the original plan reversed them). Verify against `gh api repos/MWBMPartners/MeedyaDL/milestones` before quoting numbers — this state moves weekly.
