# Pre-staged Release Notes

Drop a file named `<tag>.md` in this directory (e.g. `v1.10.0.md`) **before** the tag is pushed and the `release.yml` workflow's `ensure-release` job will use its content as the GitHub Release body verbatim.

This is **tier 1** of the release-notes-source fallback chain introduced by #857. The other tiers:

| Tier | Source | When used |
|---|---|---|
| 1 | `.github/release-notes/<tag>.md` | When this file exists for the tag being released |
| 2 | git-cliff diff between the previous tag of the same channel and this tag | When tier 1 is absent and `git-cliff` is installable on the runner |
| 3 | Static "See CHANGELOG.md" stub | When tiers 1 and 2 both fail |

## When to use tier 1

- **Manual stable cuts** where you want a hand-written narrative ("v1.10.0 — Profile Bundle release") instead of an auto-generated commit list.
- **Releases that need a specific upgrade note** (e.g., schema migration that requires a one-shot manual step before launching the new build).
- **Compliance-critical releases** (security advisories where the wording matters).

For most channel-driven cuts (alpha / beta / rc / nightly / weekly / monthly), tier 2 (git-cliff) is the better default — automatic, accurate, no maintainer ceremony required.

## File format

Pure Markdown. The `finalize-release` job appends the platform-specific "Choose your download" table + `.sig` explanations underneath whatever you put here. So write what the **end-user** should see at the top of the GitHub Release page — typically:

```markdown
# MeedyaDL <tag>

<one-sentence summary of what changed>

## What's new

- ...

## What's fixed

- ...

## Performance

- ...

## Notes

- ...
```

See `v1.4.3` (commit `PR #785`) for the canonical four-section gold standard.

## Filename rules

- Must match the tag name exactly, including the leading `v`. So `v1.10.0.md`, not `1.10.0.md` or `meedyadl-1.10.0.md`.
- One file per release. Two files for the same tag is an error — the workflow uses `.github/release-notes/${TAG}.md` literally.

## Lifecycle

Pre-staged files are **kept** in the repository after release. They serve as the durable record of the release notes that shipped, alongside the in-repo CHANGELOG.md (which git-cliff regenerates on every push).

## Why not just rely on git-cliff?

git-cliff is great for routine cuts but has limitations the maintainer sometimes needs to bypass:

- It enumerates commits but doesn't synthesise narrative context.
- It can't say "this release is the first to support GAMDL v3.7" — that requires the maintainer's framing.
- It can't include external links to upstream issues, regression-test results, etc.

When those matter, drop a tier-1 file. The rest of the time, let git-cliff do its job.
