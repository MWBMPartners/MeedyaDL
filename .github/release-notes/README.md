# Pre-staged Release Notes

Drop a file named `<tag>.md` in this directory (e.g. `v1.10.0.md`) **before** the tag is pushed and the `release.yml` workflow's `ensure-release` job will use its content as the GitHub Release body verbatim.

This is **tier 1** of the release-notes-source fallback chain introduced by #857, extended by #1028 to write ELI5, user-facing prose (see [`STYLE_GUIDE.md`](STYLE_GUIDE.md) for the writing rules). The other tiers:

| Tier | Source | When used |
|---|---|---|
| 1 | `.github/release-notes/<tag>.md` | When this file exists for the tag being released. Also runs as a corrective pass (`scripts/release-notes/apply-notes.sh`) that overwrites the body of a release object that already exists — including one **pre-created** by release-please or `version-bump.yml` before the file was staged. Tier 1 always wins, however the release object came to exist. |
| 2 | ELI5 render from `Release-Note:` git trailers (`.github/cliff-eli5-body.tera` / `cliff-cumulative-body.tera`), with the full technical `git-cliff` diff collapsed into a `<details>` block underneath | When tier 1 is absent and `git-cliff` is installable on the runner |
| 3 | Static "See CHANGELOG.md" stub | When tiers 1 and 2 both fail |

## When to use tier 1

- **Every stable release.** `.github/workflows/release-note-gate.yml`'s `release-pr-notes-file` job **enforces** this — the release-please PR cannot merge without a populated `.github/release-notes/v<version>.md` already on `main`. Bootstrap one with `scripts/release-notes/draft-notes.sh v<version>`.
- **Milestone prereleases** (e.g. the first alpha on a new minor line, or one that needs specific framing) — stage a tier-1 file the same way; routine day-to-day alphas can rely on tier 2.
- **Releases that need a specific upgrade note** (e.g., schema migration that requires a one-shot manual step before launching the new build).
- **Compliance-critical releases** (security advisories where the wording matters).

Tier 2 is ELI5 whenever the PRs in range carried `Release-Note:` trailers — it is no longer just a technical commit dump. Reach for it as the default for routine channel-driven cuts (alpha / beta / rc) where no tier-1 file has been staged; reach for tier 1 whenever the release is a stable cut (mandatory, CI-enforced) or a milestone prerelease that needs hand-written framing.

## File format

Pure Markdown. The `finalize-release` job appends the platform-specific "Choose your download" table + `.sig` explanations underneath whatever you put here. So write what the **end-user** should see at the top of the GitHub Release page — typically:

**Confidentiality constraint:** describe capabilities, never mechanisms. This file is our loudest advertisement surface (also served inside the app by the in-app updater), so never state, name, or paraphrase — anywhere in the file, including inside HTML comments or a collapsed `<details>` block — how a feature obtains its data, credentials, or media (tokens, endpoints, acquisition paths, protocol/crypto/storage internals). See [`STYLE_GUIDE.md`](STYLE_GUIDE.md) → "Never reveal how a feature is delivered" for the full rule and the allowed-vocabulary test.

```markdown
# MeedyaDL <tag>

<one-sentence summary of what changed>

### What's new

- ...

### What's fixed

- ...

### Performance

- ...

### Notes

- ...
```

Section headings must be `###` (H3) — the `release-note-gate.yml` CI check and `scripts/release-notes/draft-notes.sh` both key on a line starting with `###` followed by a space. Write the bullets in plain English per [`STYLE_GUIDE.md`](STYLE_GUIDE.md); `scripts/release-notes/draft-notes.sh <tag>` will scaffold this shape for you, pre-seeded from any `Release-Note:` trailers already merged.

See `v1.11.0-alpha.21.md` / `v1.9.1.md` in this directory for the canonical gold standard.

## Filename rules

- Must match the tag name exactly, including the leading `v`. So `v1.10.0.md`, not `1.10.0.md` or `meedyadl-1.10.0.md`.
- One file per release. Two files for the same tag is an error — the workflow uses `.github/release-notes/${TAG}.md` literally.

## Lifecycle

Pre-staged files are **kept** in the repository after release. They serve as the durable record of the release notes that shipped, alongside the in-repo CHANGELOG.md (which git-cliff regenerates on every push).

Tier 1 isn't limited to a "run once before the tag exists" window. `scripts/release-notes/apply-notes.sh <tag>` can be (re-)run at any point — right after staging, mid-build, or as a post-publish fix-up — and it always wins: it overwrites whatever body the release object currently has (a `release.yml`-generated draft, a release-please/`version-bump.yml` placeholder, an earlier tier-2 render) while preserving the "Choose your download" footer verbatim. It's idempotent — re-running it against an already-current release is a no-op.

## Why not just rely on git-cliff?

git-cliff now renders ELI5 lines too — when a PR's squash-merge commit carries a `Release-Note:` trailer, both `cliff-eli5-body.tera` (per-release) and `cliff-cumulative-body.tera` (since-last-stable) use it instead of the raw commit subject. So for a lot of routine cuts, tier 2 alone is enough. Tier 1 still earns its keep for what trailers can't do:

- **Narrative framing.** A trailer is one line per PR; it can't say "this release is the first to support GAMDL v3.7" or open with a summary sentence tying several changes together.
- **External links** to upstream issues, regression-test results, migration guides, etc. — trailers deliberately carry no links (see `STYLE_GUIDE.md`), so anything beyond a single `([details](url))` per bullet needs a hand-written file.
- **Coverage gaps.** Not every historical commit carries a trailer (the feature only exists from #1028 onward), and a maintainer may want the full narrative even when every PR did comply.
- **Mandatory for stables.** `release-note-gate.yml` requires a tier-1 file before a stable release-please PR can merge, regardless of trailer coverage.

When those matter, drop a tier-1 file (`scripts/release-notes/draft-notes.sh <tag>` gets you started, pre-seeded from any trailers that already exist). The rest of the time, let git-cliff's ELI5 render do the job.
