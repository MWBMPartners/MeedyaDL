---
name: project-cron-channels-removed
description: v1.11.0 removed Nightly/Weekly/Monthly cron channels — alpha branch is now the bleeding edge. Backwards-compat shim for old settings.json.
metadata:
  type: project
---

# Cron channels (Nightly/Weekly/Monthly) removed in v1.11.0

**Status**: code change landed on branch `chore/remove-cron-channels` (off `chore/sync-alpha-with-main`, the alpha/main drift PR #877).

## What changed

- **`.github/workflows/{nightly,weekly,monthly}-release.yml` deleted.** No more cron-driven builds.
- **`.github/rulesets/protected-cron-channels.json` deleted.** The `apply-branch-rulesets.yml` reconciliation logic will auto-prune the leftover ruleset on the GitHub side on next run.
- **`UpdateChannel` enum (Rust + TS)**: Nightly/Weekly/Monthly variants kept in Rust ONLY for backwards-compat settings deserialisation; hidden from the UI; `from_tag()` classifies legacy tag suffixes as Alpha. TS type narrowed to `'alpha' | 'beta' | 'rc' | 'stable'`.
- **Settings migration v6 → v7**: `UpdateChannel::migrate_deprecated_to_alpha()` promotes any Nightly/Weekly/Monthly user to Alpha on first load post-upgrade. Logged in the activity log.
- **UI**: `ChannelSwitchWarning.tsx` label map cleaned; `GeneralTab.tsx` channel dropdown options + comments updated; only Alpha remains gated behind Dev Access.
- **Workflows**: `auto-delete-merged-branches.yml` and `apply-branch-rulesets.yml` had their cron-channel references removed.

## Why this was done

1. **Alpha branch is the new bleeding edge.** Every push to `alpha` triggers `alpha-release.yml` and produces a build. Lower-noise, more curated than nightly's "auto-merge every feat/* branch nightly".
2. **Auto-merge had been brittle.** `weekly-conflict` / `monthly-conflict` issue labels existed because the auto-merge feat/* pattern hit conflicts frequently.
3. **Daily release noise.** 25 cron tags in the release history (23 nightly + 1 weekly + 1 monthly).
4. **UI complexity.** 7-variant UpdateChannel enum with 3 variants hidden behind Konami. If a channel needs hiding, it shouldn't exist.

## How to apply (when reasoning about this in future)

- A user reporting "I can't find Nightly channel in dropdown anymore" → it was removed; they got auto-migrated to Alpha
- A user's settings.json had `"update_channel": "weekly"` → loads fine; migration converts to Alpha + logs the change
- A user installs a build from one of the deleted tags (e.g. they had a saved `.dmg`) → in-app updater treats their build as Alpha (via `from_tag` mapping)
- Branch protection rulesets / workflow files referencing nightly/weekly/monthly → audit + clean up via this branch

## Related

- [[project_release_pipeline_gotchas]] — the cron-channel removal eliminates one entire class of release-pipeline edge case
- [[project_gamdl_v37_audit]] — surfaced separately; this cleanup landed in parallel
- [[project_alpha_main_drift]] — alpha/main drift PR #877 (chore/sync-alpha-with-main) is the BASE of this branch; the two must merge in order
