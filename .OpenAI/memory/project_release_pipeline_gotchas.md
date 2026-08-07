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

## 5. Release-please PR body ships as the user-facing release notes

**Symptom:** A new `vX.Y.Z` release publishes with a release body that is just a one-line conventional-commit subject, e.g.:

> * legacy folder merge + colour-coded activity log (closes #789, #793) (#794)

…rather than the four-section "What's new / What's fixed / Performance / Notes" gold-standard format the user expects.

**Root cause:** Release-please autofills its PR body from the squash-commit subjects of the user-facing PRs merged since the last tag. Those subjects are 50–70 chars of dev shorthand — fine for `git log`, useless as a release note for end users. When the release-please PR is merged as-is, that body becomes the GitHub Release body, and `commands/updates.rs::release_body` serves it verbatim to the in-app updater. **CHANGELOG.md is regenerated by git-cliff from the same commit messages on every push**, so manually editing CHANGELOG.md after the fact is futile — git-cliff reverts it.

The fix has to land on the release-please PR body **before it is merged**. Recovery after the fact is limited to `gh release edit vX.Y.Z --notes-file …` on the GitHub Release; the in-app updater respects the edit on its next poll.

**Hardened procedure — every time you auto-merge a `feat:` or `fix:` PR into main:**

1. Watch for the open `chore(main): release X.Y.Z` PR. It opens within ~1 min of any `feat:`/`fix:` push to main:

   ```bash
   gh pr list --state open --search 'in:title "chore(main): release"' --json number,title --limit 1
   ```

2. Rewrite the body in place with user-facing notes in the four-section format (concrete-symptom-then-plain-English-fix bullets, no `cargo clippy` style code references). Reference v1.4.3 / PR #785 for the canonical shape:

   ```bash
   gh pr edit <num> --body-file /tmp/release-notes.md
   ```

3. **Then** signal the user the release-please PR is ready to merge. Don't merge it for them.

4. **Post-publish recovery** only when step 2 was missed: `gh release edit vX.Y.Z --notes-file /tmp/notes.md` overwrites the GitHub Release body. The in-app updater serves the corrected body on its next poll.

Worked example (2026-05-17): v1.5.0 shipped with the generic `* legacy folder merge + colour-coded activity log (closes #789, #793) (#794)` line. PR #794 was auto-merged at ~12:55, release-please PR #795 opened at 12:25 and sat for **43 minutes** before the maintainer merged it at 13:08 — an entire window during which the body could have been rewritten. Recovery was via `gh release edit v1.5.0 --notes-file /tmp/v1.5.0-notes.md`. CHANGELOG.md kept the generic line because git-cliff owns it; that's fine — it's a developer artefact.

## 6. ELI5 machinery can be present on one channel branch and absent on another (#1046)

**Symptom:** `alpha`/`beta`/`release-candidate` prerelease bodies suddenly regress to raw commit-speak or bare `🧹 Maintenance — (alpha) X.Y.Z` bump noise, even though `main` (or another channel) clearly has the #1027/#1028 ELI5 machinery (`release.yml`'s two-section prerelease render, `.github/cliff-eli5-body.tera`, `.github/cliff-cumulative-body.tera`, the `Release-Note:` trailer pipeline).

**Root cause:** a tag-push-triggered workflow runs the copy of `.github/workflows/release.yml` **at the tag's own commit**, not the copy on `main`. If the ELI5 machinery was developed on a feature/working branch and only reached `alpha` (say, via a realignment merge) without ever landing on `main` — or `beta`/`release-candidate` simply went dormant before the machinery existed — every tag cut from a pre-machinery tree renders with whatever `ensure-release` logic existed at THAT commit, regardless of what `main` looks like today. This bit `v1.11.0-alpha.30`/`.31` (cut 2026-07-19/24 from a tree where #1027/#1028 existed only on `alpha`+feature branches, not `main`) — see `.github/audits/release-notes-eli5-diagnosis-2026-07-24.md` for the full evidence chain.

**Guardrails now in place (#1046):**
- `ensure-release`'s already-exists branch self-heals: if a prerelease's live body matches the commit-speak heuristic in `scripts/release-notes/detect-commit-speak.py` (old `## [x.y.z]` git-cliff header, or a bare `### Maintenance` section, with none of the new-format markers present), it's automatically regenerated and spliced back on **the next re-run of Release for that tag** — via `scripts/release-notes/splice-body.py`, shared with `apply-notes.sh`. Re-running `gh workflow run Release -f tag=vX.Y.Z-suffix.N --ref <branch-that-has-the-fix>` now repairs a bad prerelease body without any hand-editing.
- Stables are deliberately NOT self-healed this way — they still hard-require a curated `.github/release-notes/vX.Y.Z.md` file (see `release-note-gate.yml`'s `release-pr-notes-file` job), so a human always signs off on stable release copy.
- `release-note-gate.yml` gained a third job, `push-trailer-advisory` — the PR-only trailer check (`pr-trailer`) never fires for this repo's "push `fix:`/`feat:` directly to main" convention; the new job scans pushed commit ranges on `main`/`alpha`/`beta`/`release-candidate` and posts an advisory `::warning` for any feat/fix/perf commit missing a `Release-Note:` trailer.

**Still requires a human check:** before `gh workflow run "Release"` (or any manual re-run) for a channel tag, confirm `--ref <branch>` points at a branch that actually HAS the ELI5 machinery — `git cat-file -e <branch>:.github/cliff-eli5-body.tera` is a fast sanity check. `workflow_dispatch` with no `--ref` always runs the **default branch's** copy (`main`), which can silently be stale relative to `alpha`/`beta`/`release-candidate`. Before reactivating a dormant channel branch (`beta`/`release-candidate` as of 2026-07-24), realign it onto the promoted base first so it inherits the current machinery — don't just push directly onto the old tip.

## How to apply

When walking a user through a release cut:

1. After they merge a release-please PR, check `gh run list --workflow="Release Please" --limit 3` AND `git log -1 --format=%B origin/main | grep "skip ci"` — if the workflow didn't fire and the merge commit body contains `[skip ci]` strings, you've hit issue #1; trigger Release Please manually.
2. After release.yml completes, check `gh release view <tag> --json body` — if it starts with `Release in progress`, run the strip script.
3. After release.yml completes, check `gh release view <tag> --json isDraft` — if `true` for a stable tag, `gh release edit <tag> --draft=false`.
4. Don't flip `isPrerelease=true → false` without the user's go-ahead.
5. After **you** auto-merge any `feat:`/`fix:` PR, immediately check for the open release-please PR (issue #5 above) and rewrite its body before the maintainer merges it.
