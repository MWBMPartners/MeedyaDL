# Release notes drafts

User-facing release-notes bodies, one Markdown file per planned release.

## Why this exists

Release-please force-pushes its release branch on every sync. Each force-push
regenerates the PR body from raw commit subjects, wiping any manual rewrite.
That means the GitHub Release body — which the in-app updater serves to users
via [`release_body` in `update_checker.rs`](../../src-tauri/src/services/update_checker.rs) —
ends up as a wall of 70-char dev shorthand instead of the four-section
gold-standard format users actually want to read.

The [`preserve-release-pr-body.yml`](../workflows/preserve-release-pr-body.yml)
workflow runs after every "Release Please" sync and re-applies the matching
draft file from this folder as the PR body. The draft survives every
force-push for the lifetime of the release PR.

## Convention

- One file per release: `v<MAJOR>.<MINOR>.<PATCH>.md` (or `v<X.Y.Z>-<channel>.<N>.md`
  for prereleases — e.g., `v1.7.0-rc.1.md`).
- Filename must match the version that release-please picks up from the
  conventional commits since the last tag.
- Format: four-section gold-standard (What's new / What's fixed /
  Performance / Notes). See [the v1.6.0 draft](v1.6.0.md) for the canonical
  shape, and [`.claude/CLAUDE.md`](../../.claude/CLAUDE.md) > "MANDATORY:
  rewrite the release-please PR body before it merges" for the writing rules.
- The first line should be the release-please robot banner so the file is a
  drop-in replacement for the auto-generated body:
  ```
  :robot: I have created a release *beep* *boop*
  ---
  ```

## Workflow

1. Write `.github/release-drafts/vX.Y.Z.md` while the release PR is open.
2. Commit it to `main`. The next release-please sync (triggered by any push
   to main) will force-push the release branch — and within seconds the
   `Preserve Release-Please PR Body` workflow will re-apply your draft.
3. Iterate on the file in the same way; each commit to main re-triggers the
   apply step. To re-apply without waiting for release-please, dispatch the
   `Preserve Release-Please PR Body` workflow manually.
4. After the release ships, the draft file can be left in place (historical
   record) or moved into a `_shipped/` subfolder — neither is required.

## Manual re-apply

```bash
gh workflow run "Preserve Release-Please PR Body" --ref main
# Optional: target a specific PR (e.g. an alpha release that doesn't follow
# the standard `chore(main): release X.Y.Z` title pattern):
gh workflow run "Preserve Release-Please PR Body" --ref main -f pr_number=810
```
