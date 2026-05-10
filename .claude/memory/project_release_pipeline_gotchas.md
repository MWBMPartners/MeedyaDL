---
name: Release pipeline gotchas (skip-ci, placeholder, manual recovery)
description: Three recurring failure modes in release.yml + release-please that bit during the v1.1.0 cut — recovery patterns documented so the next person hitting them doesn't have to re-discover them
type: project
---
The v1.1.0 release cut on 2026-05-09 surfaced three failure modes that aren't obvious from reading the workflow files. PR #741 (open at the time of writing) patches the workflow side; this memory captures the recovery patterns + remaining edges.

## 1. `[skip ci]` propagates through release-please's CHANGELOG body

**Symptom:** You merge release-please's release PR. The merge succeeds. Then... nothing happens. No Release Please workflow run, no tag, no Release workflow build.

**Root cause:** release-please's auto-generated CHANGELOG body in the merge commit message includes references to historical commits whose subjects contained `[skip ci]` (e.g. previous CI-skip commits, or PRs whose titles literally said `[skip ci]`). GitHub Actions parses commit messages for `[skip ci]` ANYWHERE in the body — not just the subject — and skips every push-triggered workflow when it finds it.

**Recovery (two manual triggers, ~25 min total):**

```sh
# 1. Manually trigger Release Please to create the tag.
gh workflow run "Release Please" --ref main

# 2. Once the tag is created (~30s), manually trigger the Release
#    workflow with the tag as input.
gh workflow run "Release" -f tag=vX.Y.Z

# 3. After the build (~17-25 min), the GitHub Release will exist
#    as a DRAFT (release.yml's auto-publish step only handles
#    prereleases; stable tags rely on release-please-action).
#    Strip the placeholder + publish:
gh release view vX.Y.Z --json body --jq .body | \
  python3 -c "import re,sys;sys.stdout.write(re.sub(r'^Release in progress[^\n]*\n+(?:---\s*\n+)?', '', sys.stdin.read()))" | \
  gh release edit vX.Y.Z --notes-file -
gh release edit vX.Y.Z --draft=false
```

**Long-term fix candidate:** Either escape `[skip ci]` in the changelog generator (`[skip ci]` → `[skip_ci]`), or migrate to a CI skip mechanism that only checks the commit subject line. PR #741 doesn't address this — the placeholder fix in #741 papers over the symptom but the root mechanism still trips the workflow chain.

## 2. "Release in progress…" placeholder persistence

**Symptom:** A published GitHub Release's body starts with the literal text `Release in progress...` followed by `---` and the actual content.

**Root cause:** `release.yml`'s per-platform build steps create the GitHub Release with `gh release create --notes "Release in progress..."` as a race-guard (any of the 6 platforms may be the first to create it). The "Append download guide" step at the end then APPENDED to the existing body — so the placeholder became permanent.

**Affected releases:** 15 historical Pre-releases between 2026-04-28 and 2026-05-10 (every nightly / weekly / monthly / RC that didn't go through release-please-action).

**Recovery:** `/tmp/strip-placeholder.sh` (committed during the 2026-05-10 cleanup; see history) does a regex strip + `gh release edit --notes-file -` per tag. 13 of 15 backfilled successfully; 2 May-10 tags were inaccessible at backfill time.

**Permanent fix:** PR #741 patches the "Append download guide" step to strip the placeholder before appending. Once #741 merges, new releases can't have this issue.

## 3. Manual stable releases sit as drafts

**Symptom:** You manually tag a stable version (`vX.Y.Z` with no hyphen suffix), push the tag, the Release workflow builds 6 platforms with 20 assets attached — but the GitHub Release stays as a draft. Users can't see it.

**Root cause:** `release.yml`'s "Auto-publish prerelease draft" step at the end of `finalize-release` ONLY publishes prereleases (tags with a hyphen suffix like `-nightly.X` or `-rc.N`). Stable tags rely on `release-please-action` to publish them — but if you manually tagged (e.g., promoting an RC by tagging the same commit as `vX.Y.Z`), release-please isn't involved and nothing publishes the draft.

**Recovery:** `gh release edit vX.Y.Z --draft=false` after the build completes.

**This is by design**, not a bug — PR #645 added `version-bump.yml` pre-creating the release object before tag push to avoid the same gap there. The manual-tag path is an edge case the workflow doesn't handle automatically.

## 4. Release promotion policy (don't auto-flip stable flags)

**The user gates the GitHub "Latest" badge / `isPrerelease=false` on manual testing.** Even when `release.yml` builds a `vX.Y.Z` stable tag and `release-please-action` publishes it as `isPrerelease=false`, the user wants it set BACK to `true` until they've finished testing. Don't auto-correct an apparently-mis-flagged prerelease — assume it's intentional.

Worked example (2026-05-10): v1.1.0 was published as `isPrerelease=false` by release-please-action. I incorrectly "fixed" it to `--prerelease=false --latest`. The user corrected me: "v1.1.0 is a pre-release version, until tested." The right move was to flip back to `isPrerelease=true`; v1.0.0 (which had been promoted from rc.1 and was already tested) automatically became Latest because it was the highest semver among non-prereleases.

See `feedback_release_promotion_policy.md` in personal memory for the full policy.

## How to apply

When walking a user through a release cut:

1. After they merge a release-please PR, check `gh run list --workflow="Release Please" --limit 3` AND `git log -1 --format=%B origin/main | grep "skip ci"` — if the workflow didn't fire and the merge commit body contains `[skip ci]` strings, you've hit issue #1; trigger Release Please manually.
2. After release.yml completes, check `gh release view <tag> --json body` — if it starts with `Release in progress`, run the strip script.
3. After release.yml completes, check `gh release view <tag> --json isDraft` — if `true` for a stable tag, `gh release edit <tag> --draft=false`.
4. Don't flip `isPrerelease=true → false` without the user's go-ahead.
