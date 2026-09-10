---
name: project-green-release-can-miss-a-platform
description: A Release run reports success even when a platform build fails, because the ARM cross-compile jobs are deliberately continue-on-error — so a release can look entirely green and be missing a platform
metadata:
  type: project
---

# A green release can be missing a whole platform

The `Release` workflow reports **success** even when one of the platform builds
inside it has failed. The two ARM cross-compile jobs are deliberately marked
"carry on if this fails" (`continue-on-error`, because they are Tier 2 /
experimental). That is a defensible choice — one flaky experimental target
should not sink an entire release.

The cost is that **the run's green tick does not mean every platform built.**
Nothing announces the gap. You only see it by opening the run and reading the
job list, or by noticing an absence in the release assets, which is exactly the
kind of thing nobody notices.

This happened on `v1.13.0-alpha.65` (2026-09-10): Linux ARMv7 failed, the run
said success, and the release published with no ARMv7 files and no ARMv7 entries
in `latest.json`. ARMv7 users had no update.

## How to check, rather than trust the tick

Read the job list, not the run conclusion:

```bash
gh api "repos/MWBMPartners/MeedyaDL/actions/runs/<run-id>/jobs?per_page=50" \
  --jq '.jobs[] | "\(.conclusion)  \(.name)"'
```

Then count the release assets. Six platforms should give **22 files**, and
`latest.json` should carry **twelve** platform entries, every one with a
signature.

## If a platform build failed

1. **Re-run just that job** first. Both times this has bitten, the cause was
   outside the repository — Ubuntu's armhf archive briefly in a state where
   `libc6:armhf` could not be installed, which cascades into every other armhf
   package. ARMv7 had succeeded in the six releases before it, so a re-run is
   the cheap, correct first move.
2. **Re-running the build does not fix `latest.json`.** The manifest was
   assembled before the retry, so the files exist and the updater still cannot
   see them. Re-run the run's own **`Finalize Release Notes`** job, which
   rebuilds the manifest from whatever assets are actually on the release.
3. **Do not reach for `fix-updater-manifest.yml`** for this. That standalone
   repair tool has no ARMv7 handling at all, so it silently produces a manifest
   missing the platform most likely to need repairing — see **#1178**. It only
   works for the platforms it happens to know about.

## Why this keeps happening in this shape

Two separate copies of the same platform list exist, one in `release.yml` and
one in `fix-updater-manifest.yml`. One was updated for ARMv7; the other was not;
nothing compares them. That is the same failure as [[project-never-worked-pattern]]
— two sources that must agree, with nothing that would ever notice they had
stopped agreeing.

Related: [[project-release-pipeline-gotchas]], [[project-never-worked-pattern]],
[[project-comment-accuracy-hazard]].
