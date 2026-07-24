# Release-Notes ELI5 Diagnosis — Issue #1046 (2026-07-24)

**Scope:** Why prerelease GitHub Releases (exemplar: `v1.11.0-alpha.30`) shipped commit-speak /
bare version-bump noise instead of the #1027/#1028 ELI5 format; whether the next auto-cut alpha
is fixed; backfill plan; durable gates. Diagnosis performed on branch `chore/release-supply-chain`
(off reconciled `alpha`), read-only.

---

## 1. Root cause (with evidence)

### 1.1 The one-sentence version

The ELI5 machinery (#1027 tier-2 rewrite, #1028 trailers, #1033 cumulative template) never
landed on `main` at all and only reached the `alpha` branch via the 2026-07-24 realignment
(#1040) — so every alpha tag up to and including `v1.11.0-alpha.31` was cut from a tree that
contained the **pre-#1027** `release.yml` + `cliff.toml`, and (because a tag-push-triggered
workflow runs the workflow file **at the tag's commit**) `ensure-release` executed the old
single-range tier-2 with no bump-commit skip rule.

### 1.2 Evidence chain

**(a) Machinery commits exist only on `alpha` (and feature branches), NOT on `main`:**

```
60d3de82 2026-07-18 fix(release): populate prerelease notes with real changes, not bump noise (#1027)
87367c2c 2026-07-19 feat(release): ELI5 release notes by default — Release-Note trailers + curated stable notes (#1028)
3c1ed0fb 2026-07-19 fix(release-notes): rewrite cumulative template to never leak commit-speak + backfill alpha.29 (#1033)
```

- `git branch -r --contains 60d3de82` → `origin/alpha`, `origin/chore/pr-security-noise`,
  `origin/chore/release-supply-chain`, `origin/claude/pr-1037-alpha-setup-p3ob3y`. **`origin/main` is absent.**
- `origin/main` tip is `docs: update CHANGELOG.md [skip ci]` dated **2026-07-06**;
  `git cat-file -e origin/main:.github/cliff-eli5-body.tera` → *does not exist*.
- `origin/beta` (tip 2026-05-20) and `origin/release-candidate` (tip 2026-05-08) also lack the
  machinery entirely.

**(b) Alpha tags up to `.31` lack the machinery:**

`git merge-base --is-ancestor` of all three machinery commits against the tags:

| Tag | Cut date | Contains #1027/#1028/#1033? |
|---|---|---|
| v1.11.0-alpha.29 | 2026-07-19 | NO (all three missing) |
| v1.11.0-alpha.30 | 2026-07-20 | NO |
| v1.11.0-alpha.31 | 2026-07-24 (pre-realignment push) | NO |
| v1.12.0-alpha.32 | 2026-07-24 (post-realignment) | **YES** |
| v1.12.0-alpha.33 | 2026-07-24 | **YES** |
| v1.12.0-alpha.34 | 2026-07-24 | **YES** |

**(c) What actually ran for alpha.30, step by step:**

1. `alpha-release.yml` (push-driven on `alpha`, `.github/workflows/alpha-release.yml:31-33`)
   committed `chore(alpha): 1.11.0-alpha.30` (`:167-177`) and pushed tag `v1.11.0-alpha.30`
   (`:179-226`). The tag points at the bump commit itself.
2. The tag push triggered `release.yml` **at the tag's tree**. That version
   (`git show v1.11.0-alpha.30:.github/workflows/release.yml`) has the #857-era
   `ensure-release`: tier-1 file check, then tier-2 = a **single-range**
   `git-cliff --strip header --tag "$TAG" "${PREV_TAG}..${TAG}"` (old line ~255) rendered with
   **cliff.toml's default body template** (which emits the `## [1.11.0-alpha.30] - 2026-07-20`
   header + `### <group>` sections). No two-section prerelease format, no ELI5 template, no
   honest deps-only preamble — all of that is #1027+ and wasn't in this tree.
3. The commit range `v1.11.0-alpha.29..v1.11.0-alpha.30` is exactly:
   ```
   ce935dca chore(alpha): 1.11.0-alpha.30
   3f7b241d chore(deps): bump the npm-minor-patch group with 11 updates (#1035)
   d0dc628a chore(deps): bump serde_json … cargo-minor-patch group (#1036)
   ```
4. `cliff.toml` **at that tag** skips `chore(deps)` (old line 140) but has **no rule skipping
   `chore(alpha): X.Y.Z-alpha.N`** (the skip rule now at current `cliff.toml:152` was added by
   #1027 and is absent at the tag). So the two deps commits vanished and the ONLY surviving
   commit was the version bump itself → rendered as `### 🧹 Maintenance / - **(alpha)**
   1.11.0-alpha.30`. That is precisely the observed body.
5. There was no tier-1 file (`.github/release-notes/v1.11.0-alpha.30.md` does not exist — the
   #1027/#1033 backfills covered `.18`–`.29` only).
6. After the platform builds, `finalize-release`'s "Append download guide" step appended the
   raw `## Choose your download` table (current `release.yml:1584-1717`; same step existed at
   the tag). With the notes content being one noise line, the table dominates the body — the
   "embedded raw download table" half of the complaint. The append is by design (it carries the
   asset links), but nothing de-emphasises it relative to (empty) notes.

**(d) alpha.31 — same root cause, different symptom.** Cut the morning of 2026-07-24 *before*
the realignment merge, from the same pre-#1027 tree (verified: machinery commits not ancestors;
`git show v1.11.0-alpha.31:cliff.toml` has no bump-skip rule). Its range
`alpha.30..alpha.31` contains ~10 real commits (#905 CI security workflow, #945 a11y labels,
#946 settings tab labels, #935/#942 activity-log detail, npm audit fix, etc.), so its body is a
**technical commit-speak dump** (old grouped format incl. its own bump line under Maintenance)
rather than bare noise — still not ELI5, still the old format.

### 1.3 Why the CLAUDE.md narrative masked this

CLAUDE.md documents #1027/#1028 as landed ("landed by 2026-07-19"), which is true **for the
branch the work was done on** — but that work never reached `main` (stale since 2026-07-06)
and only reached `alpha` on 2026-07-24 via #1040. The releases in between (alpha.30, alpha.31)
fell in the gap. This is a branch-topology failure, not a logic failure: the #1027 logic is
sound and demonstrably absent from the trees that cut the bad releases.

---

## 2. Current-state verdict: does it work NOW?

### 2.1 What the next auto-cut alpha will render — mostly YES

`v1.12.0-alpha.32/33/34` (cut 2026-07-24, post-realignment) contain the full machinery, so
`ensure-release` tier-2 (current `release.yml:222-453`) runs the #1027 prerelease branch
(`release.yml:367-423`). Simulating alpha.34 (range `alpha.33..alpha.34` = the bump commit +
`7249c01b ci(pr-security)…` which carries `Release-Note: none`):

- Bump commit `chore(alpha): 1.12.0-alpha.34` → **skipped** by `cliff.toml:152`. ✅ No more
  "🧹 Maintenance — (alpha) X.Y.Z" noise, ever, on any tag containing this rule.
- ELI5 render (`release.yml:330-334`) → empty (only `Release-Note: none`). `has_content` false.
- `INC` (technical render) → has content (`ci:` group) → the honest "no user-facing changes"
  line at `:390` is correctly **suppressed**, and the body becomes:
  `# MeedyaDL 1.12.0-alpha.34` + `## What's changed since v1.10.1 (the last stable release)`
  (cumulative, `cliff-cumulative-body.tera`, which can only emit ELI5 bullets or the aggregated
  "### Under the hood — N internal changes" line — **never raw commit subjects**, per #1033) +
  `<details>` technical changelog. Self-describing, no commit-speak above the fold. ✅
- A genuinely deps-only build renders the honest preamble (`:385-392`). ✅

**Caveat:** I could not fetch the live alpha.32–34 bodies (no `gh`, unauthenticated API
blocked). The main agent should eyeball them via GitHub-MCP; prediction: correct new format,
but with **empty ELI5 sections** (see 2.2.1) and alpha.32's genuinely user-facing content
(#1029 wrapper sign-in modal, #1034 security hardening) buried in "Under the hood".

### 2.2 Residual gaps (concrete)

1. **Direct-to-main/alpha pushes bypass the trailer gate — the dominant workflow.**
   `release-note-gate.yml:39-42` only fires on `pull_request`. This repo's convention
   (CLAUDE.md "Push fix:/feat: commits directly to main") means most user-facing commits never
   see the gate. Verified: `35710d6a feat(wrapper): in-app "Sign in to wrapper" modal (#1029)`,
   `7d29efd7`/`8b2b2ade security: … (#1034)`, `553993c8 fix(deps) (#996)`, `05b3479c
   fix(history) (#992)`, `85d13e7f fix(auth) (#1010)` — **none carry a `Release-Note:`
   footer**, so the alpha.32 ELI5 sections came out empty despite substantial user-facing
   content in range. (Today's direct commits `7249c01b`, `5b8cb440`, `be441fc7` DO carry
   `Release-Note: none` — the habit has started, but nothing enforces it.)
2. **"Already exists + no tier-1 file" path never self-heals a prerelease**
   (`release.yml:244-249`): it logs "leaving body untouched" and only raises `::error` for
   stables. A prerelease whose release object was created by an old workflow / a re-run keeps
   its commit-speak body forever unless a curated file is committed AND a build re-runs with a
   checkout containing it.
3. **`workflow_dispatch` of Release runs `main`'s stale workflow.** `gh workflow run "Release"
   -f tag=vX` executes the **default-branch** copy of `release.yml` — which on `main` today is
   the pre-#1027 version with the old tier-2. Any manual re-run for an alpha tag would
   *regenerate the bug*. Blocked on realignment Phase 3 (#1040) syncing `main`; until then,
   dispatch with `--ref alpha` (CLI) if ever needed.
4. **`beta` / `release-candidate` branches are pre-machinery** (tips 2026-05-20 / 2026-05-08).
   They're dormant, but the next push to either would cut a tag with the old workflow and
   reproduce #1046 on that channel.
5. **The "Choose your download" table is appended raw, full-height, un-collapsed**
   (`release.yml:1670-1706`). With good notes above it this is tolerable; with thin notes it
   dominates. Three consumers key on the literal heading and must stay in sync if it changes:
   `apply-notes.sh:87-94` (footer splice), `release.yml:1653` (idempotence grep), and the
   `#857` verify step below it. The in-app Updates page already strips the section.
6. **Channel-PR merge strategy vs trailers.** Trailers live in **PR bodies**; they reach commit
   footers only via **squash-merge with PR_BODY**. CLAUDE.md's #1027 note recommends
   *rebase-merging* channel PRs to preserve conventional-commit granularity — but rebase-merge
   **drops the PR body**, so a trailer that lives only there is lost. The two goals conflict;
   resolution: rebase-merge is fine **iff** the individual commits carry their own
   `Release-Note:` footers (which gap 1's enforcement would cover), otherwise squash-merge.

---

## 3. Backfill plan

### 3.1 Tag inventory (curated tier-1 files exist for: `v1.10.0-alpha.15`, `v1.11.0-alpha.18`–`.29`, plus stables)

| Priority | Tags | State | Action |
|---|---|---|---|
| **P0 — must backfill** | `v1.11.0-alpha.30` | Confirmed bare bump-noise body | Curate file + edit body |
| **P0 — must backfill** | `v1.11.0-alpha.31` | Old-format commit-speak dump (pre-machinery tree, real commits in range) | Curate file + edit body |
| **P1 — verify, likely light-touch** | `v1.12.0-alpha.32`, `.33`, `.34` | New machinery ran; predicted correct format but ELI5-empty. `.32` has real user-facing content (#1029, #1034) worth surfacing | Main agent fetches bodies via MCP; curate a file for `.32` (recommended); `.33`/`.34` acceptable as-is if they match prediction |
| **P2 — skip (recommended)** | 12 nightlies (`v1.0.10-nightly.20260509`, `v1.10.0-nightly.2026052x–0616` ×9, `v1.10.1-nightly.20260620/-0707`), `v1.9.4-alpha.9–12`, `v1.10.0-alpha.13/14/16/17`, `v1.0.0-rc.1`, `v2.0.0-alpha.1–8` | Superseded builds on removed (#879) or abandoned channels; near-zero reader traffic | Leave, or (optional) batch-prepend a one-line "superseded prerelease — see CHANGELOG" preamble via MCP |

Net: **2 mandatory + up to 3 verify/optional**; ~29 legacy tags deliberately skipped.

### 3.2 Mechanism

`scripts/release-notes/apply-notes.sh` **requires `gh`** (hard check at `:51-54`) — unusable in
this sandbox (no `gh`, unauthenticated API blocked). The **main agent has GitHub-MCP
release-edit access**, so the least-effort durable path is:

1. **Commit curated `.github/release-notes/v1.11.0-alpha.30.md` and `v1.11.0-alpha.31.md`**
   (and optionally `v1.12.0-alpha.32.md`) to `alpha` — durability: the `ensure-release`
   already-exists path (`release.yml:240-243`) re-applies committed files on any future re-run
   whose checkout contains them (self-heal).
2. **Main agent edits each release body via MCP**, replicating apply-notes.sh's splice exactly:
   keep everything from the `\n---\n\n## Choose your download` divider onward as the footer
   (regex at `apply-notes.sh:87-94`), replace everything above it with the curated file
   content + one blank line.

### 3.3 Draft curated bodies

**`v1.11.0-alpha.30.md`** (range is bump + 2 dependency PRs — honest housekeeping shape,
mirrors the `.29` gold standard):

```markdown
# MeedyaDL 1.11.0-alpha.30

A quiet housekeeping release.

### Notes

- No user-facing changes since v1.11.0-alpha.29 — this build only refreshes internal
  dependencies to keep the app secure and up to date.
  ([#1035](https://github.com/MWBMPartners/MeedyaDL/pull/1035),
  [#1036](https://github.com/MWBMPartners/MeedyaDL/pull/1036))
- This is a pre-release on the **alpha** channel — early access to work in progress. The
  in-app updater serves the latest stable unless you've opted into Alpha.

_Full technical changelog: [v1.11.0-alpha.29 → v1.11.0-alpha.30](https://github.com/MWBMPartners/MeedyaDL/compare/v1.11.0-alpha.29...v1.11.0-alpha.30) · [CHANGELOG.md](https://github.com/MWBMPartners/MeedyaDL/blob/main/CHANGELOG.md)_
```

**`v1.11.0-alpha.31.md`** (real content in range):

```markdown
# MeedyaDL 1.11.0-alpha.31

Small quality-of-life and accessibility improvements.

### What's new

- The activity log now shows more detail while music videos, lyrics, and MusicBrainz lookups
  are running, so long downloads no longer look stalled.
  ([#935](https://github.com/MWBMPartners/MeedyaDL/issues/935),
  [#942](https://github.com/MWBMPartners/MeedyaDL/issues/942))

### What's fixed

- Screen readers now correctly announce the Retry, Open File, and Open Folder buttons on
  queue items. ([#945](https://github.com/MWBMPartners/MeedyaDL/issues/945))
- Two settings tabs have clearer names, so it's easier to find quality and fallback options.
  ([#946](https://github.com/MWBMPartners/MeedyaDL/issues/946))

### Notes

- Several internal changes to keep the app healthy — build-pipeline safety checks, security
  scanning for code contributions, and dependency updates.
- This is a pre-release on the **alpha** channel — early access to work in progress.

_Full technical changelog: [v1.11.0-alpha.30 → v1.11.0-alpha.31](https://github.com/MWBMPartners/MeedyaDL/compare/v1.11.0-alpha.30...v1.11.0-alpha.31) · [CHANGELOG.md](https://github.com/MWBMPartners/MeedyaDL/blob/main/CHANGELOG.md)_
```

---

## 4. Durable gates — exact edits

### G1 (highest value): self-heal commit-speak prerelease bodies in `ensure-release`

**File:** `.github/workflows/release.yml`, the already-exists branch at **lines 240-250**.
Replace the prerelease half of the `else` (currently "leaving body untouched", `:245`) with a
commit-speak detector + regenerate-and-splice:

- Fetch the live body (`gh release view --json body`).
- Treat it as commit-speak if it matches `^## \[[0-9]` (old git-cliff version header) OR
  contains `### 🧹 Maintenance`, AND lacks all of: `### What's new|### What's fixed|
  ### Notes|### Under the hood|_No user-facing changes` (the new-format markers).
- If commit-speak: fall through to the existing tier-2 generation (`:253-443`, needs a small
  refactor so the generation block is reachable from this branch — extract it into a function
  or gate the early `exit 0` at `:250` behind the detector), then **edit** instead of create,
  preserving the footer with the same splice used by `apply-notes.sh:68-103`. **Factor that
  Python splice into `scripts/release-notes/splice-body.py`** and call it from both
  apply-notes.sh and this step so the footer regex lives in one place.
- Effect: any re-run of Release for a bad tag repairs it automatically; historical repairs
  become "re-run the workflow" instead of hand-editing.

### G2: trailer coverage for direct pushes (closes gap 1)

- **File:** `.github/workflows/release-note-gate.yml` — add a third job (or extend
  `ci.yml`) triggered `on: push: branches: [main, alpha, beta, release-candidate]` that scans
  the pushed commits (`github.event.before..github.event.after`) for `^(feat|fix|perf)`
  subjects whose bodies lack `^Release-Note: \S`. **Advisory** (`::warning` + step summary, exit
  0) — a push can't be un-pushed, but the warning lands while the author is still looking.
- **File:** `.claude/CLAUDE.md` — add to Conventions: *"Direct-pushed `feat`/`fix`/`perf`
  commits MUST end their body with `Release-Note:` trailer line(s) (or `Release-Note: none`),
  exactly like PR bodies."* (Today's commits show the habit forming; write it down.)

### G3: collapse the download table without breaking its three consumers (closes gap 5)

**File:** `.github/workflows/release.yml:1670-1706`. Keep the literal `## Choose your
download` heading (so `apply-notes.sh:87-94`, the idempotence grep at `:1653`, and the verify
step stay untouched), but wrap the table + tips that follow it in
`<details><summary>Download links for all platforms</summary> … </details>`. The `.sig` and
macOS/first-time sections can stay inside the same details block. Zero regex churn; the body
reads notes-first at every width.

### G4: unblock the branch topology (closes gaps 3 + 4)

- Complete #1040 **Phase 3**: sync `main` with the machinery (release.yml, cliff.toml, both
  `.tera` files, `scripts/release-notes/`, `release-note-gate.yml`, curated notes dir). Until
  then, never `gh workflow run "Release"` for a tag without `--ref alpha`.
- Before reactivating `beta` / `release-candidate`, realign them (or reset onto the promoted
  base per the channel-promotion flow). Add a line to the release-pipeline-gotchas memory file.

### G5: write down the merge-strategy rule (closes gap 6)

**File:** `.claude/CLAUDE.md` (#1027 section) — replace the unconditional "rebase-merge channel
PRs" advice with: *"Rebase-merge channel PRs only when every commit carries its own
`Release-Note:` footer; otherwise squash-merge (PR_BODY) so the PR-body trailer lands in the
commit footer."*

---

## 5. Prioritized implementer task list

| # | Task | Files | Effort |
|---|---|---|---|
| 1 | Verify live bodies of `v1.12.0-alpha.32/33/34` via GitHub-MCP (confirm §2.1 prediction) | — (MCP) | XS |
| 2 | Backfill: commit curated `v1.11.0-alpha.30.md` + `v1.11.0-alpha.31.md` (drafts in §3.3; optionally `.32`) to `alpha`, then footer-preserving body edits via MCP | `.github/release-notes/*`, MCP release edits | S |
| 3 | G1 self-heal: commit-speak detector + regenerate-and-splice in `ensure-release`; extract `splice-body.py` | `release.yml:240-250` (+ refactor of `:253-443`), `scripts/release-notes/splice-body.py`, `apply-notes.sh` | M |
| 4 | G3 collapse download table in `<details>` (keep heading literal) | `release.yml:1670-1706` | S |
| 5 | G2 advisory push-gate for missing trailers + CLAUDE.md convention line | `release-note-gate.yml`, `.claude/CLAUDE.md` | S |
| 6 | G4 confirm #1040 Phase 3 syncs the machinery to `main`; note beta/rc pre-reactivation realign requirement | #1040 plan, memory file | S (coordination) |
| 7 | G5 merge-strategy doc fix | `.claude/CLAUDE.md` | XS |
| 8 | Optional: P2 legacy-tag batch preamble (or explicitly wontfix in #1046) | MCP | S |

---

*Diagnosis by Claude (Fable 5), 2026-07-24. All file:line references are against
`chore/release-supply-chain` @ `18af20d6` unless a tag is named explicitly.*
