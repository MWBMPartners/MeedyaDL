# Release notes drafts — RETIRED, historical record only

**This folder and the workflow that read it are no longer used.** Do not
write a new file here. The current mechanism lives in
[`.github/release-notes/`](../release-notes/) instead — see that folder's
own `README.md` and `STYLE_GUIDE.md`.

## Why this folder stopped being used

The files below were the input to `preserve-release-pr-body.yml`, which
rewrote the release-please pull request's body (and, as a fallback, the
published GitHub Release body) from a hand-written Markdown file each time
release-please force-pushed its branch. The last file written here was for
v1.9.4. Every release since — well over twenty of them — has used
`.github/release-notes/` instead, so nothing here has been read by anything
for a long time.

The replacement mechanism (issue #1028) works differently, and does not
try to make the release-please PR body itself look nice:

- The curated notes file (`.github/release-notes/v<version>.md`) is written
  and merged to `main` **as its own pull request**, reviewed there like any
  other change.
- `release-note-gate.yml`'s `release-pr-notes-file` check then **requires**
  that file to exist before the release-please PR is allowed to merge at
  all — so the review already happened by the time anyone needs to look at
  the release-please PR.
- `release.yml`'s `ensure-release` job applies the curated file to the
  actual **GitHub Release** body once the tag is cut — footer-preserving,
  idempotent, and able to detect and fix a prerelease body that still looks
  like raw commit text. A stable release with no curated file is refused
  outright rather than silently shipping something unreadable.

That is a stronger guarantee than this folder's workflow ever gave — it
used to happily report success while doing nothing, for ten months,
because nobody noticed the folder had gone stale. See the header comment
in `.github/workflows/preserve-release-pr-body.yml` for the full account.

## What is kept, and why

The individual `vX.Y.Z.md` files below are left in place as a historical
record of what those release bodies actually said — deleting them would
throw away real, previously-published wording for no benefit. The
`preserve-release-pr-body.yml` workflow itself is also kept, gutted down to
a manual-only, no-op explainer, purely so that running it out of old habit
tells you where the real mechanism lives instead of failing with a bare
"workflow not found".

## If you are here to write release notes today

Use `.github/release-notes/` instead:

```bash
scripts/release-notes/draft-notes.sh vX.Y.Z
# polish the resulting .draft file per .github/release-notes/STYLE_GUIDE.md,
# then drop the .draft suffix and commit .github/release-notes/vX.Y.Z.md to main
```
