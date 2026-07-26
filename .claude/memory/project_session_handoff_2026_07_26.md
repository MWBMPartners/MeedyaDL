---
name: Session handoff — MeedyaSuite folder, icon fix, release-notes confidentiality (2026-07-24 → 26)
description: State of play after the alpha.38 work — what landed, what is still owed, and the non-obvious traps discovered along the way (GitHub dispatch rules, main lacking the release-notes system, Tauri's RGBA icon requirement)
type: project
---

Snapshot of a long working session so it can be picked up cold. Written 2026-07-26.

## 1. What landed

| PR | Target | What |
|---|---|---|
| #1059 | `alpha` | MeedyaSuite folder relocation + small-icon fix + release-notes confidentiality policy (20 commits, rebase-merged) |
| #1060 | `main` | Cherry-picked `apply-release-notes.yml` onto main so it becomes dispatchable |
| #1061 | `alpha` | Dispatch-ref guard + the missing `v1.12.0-alpha.38` notes file — **green; merge state as of writing: check it** |
| #1062 | `main` | The same dispatch-ref guard, byte-identical to #1061's copy |

Issues **#1057** (folder grouping) and **#1058** (small icon) are closed with full write-ups. Sibling issues filed at **MeedyaConverter#464** and **MeedyaManager#194**.

Shipped in **v1.12.0-alpha.38** (published, verified).

## 2. Still owed — the live to-do list

1. **Run the retro-fix.** `gh workflow run "Apply Release Notes" --repo MWBMPartners/MeedyaDL --ref alpha -f tag=all -f dry_run=true`, review, then re-run without `dry_run`. **This cannot be done from a Claude session** — the integration token lacks workflow-dispatch permission (403). It is a human action.
2. **Delete the stuck `v1.12.0-alpha.36` draft**: `gh release delete v1.12.0-alpha.36 --repo MWBMPartners/MeedyaDL --yes`. **Do NOT pass `--cleanup-tag`** — keep the git tag; the alpha channel workflow derives its build counter from existing tags.
3. **Real-macOS verification** before promoting past `alpha`: the relocation prompt → move → relaunch cycle, that updates still work from the new location, the non-admin administrator-prompt path, and the icon at native small sizes. **Flush the icon cache first** (`sudo rm -rf /Library/Caches/com.apple.iconservices.store && killall Dock Finder`) or a stale cached icon masks the fix entirely.
4. **Sibling issues for MeedyaPlayer and MeedyaSubtitler** — they live in the `MeedyaSuite` org, and a session can only add repos from a single owner, so this needs a session started against that org.
5. **Tags `v1.12.0-alpha.33` … `.37` have no curated notes files** — the retro-fix skips them.

## 3. Five published releases currently disclose implementation detail

All have corrected files committed, so one `tag=all` pass repairs them. Ranked by severity:

| Tag | What is exposed |
|---|---|
| `v1.12.0-alpha.32` | **Worst.** A default host and port, two endpoint paths, component version requirements, and that an upstream service rate-limits a specific operation |
| `v1.11.0-alpha.31` | Credential/token resolution failures, lyrics-enrichment internals, lookup tier structure |
| `v1.10.0-alpha.15` | Storage-engine internals, portable-bundle format, encryption parameters |
| `v1.11.0-alpha.18` | The hidden-developer-mode key sequence, storage engine |
| `v1.12.0-alpha.36` | *(draft)* lyrics paths, the three wrapper connections |

**Footer preservation was verified byte-exact** on every tag using the real `splice-body.py`, so applying is safe.

## 4. Traps discovered — these cost real time, don't rediscover them

### `workflow_dispatch` requires the workflow on the DEFAULT branch
GitHub only registers a dispatch trigger for workflows present on the default branch. `--ref <other>` selects which *version* runs; it cannot make an unregistered workflow dispatchable. A brand-new workflow on a feature branch is invisible to `gh workflow run` by name *and* by filename.

### `main` carries NONE of the release-notes system
Verified: zero `.github/release-notes/*.md` on `main` (32 on `alpha`), and all three `scripts/release-notes/` helpers absent. So **`--ref alpha` is load-bearing, not cosmetic** — and the UI's Run-workflow button defaults to `main`, i.e. the obvious click is the broken one. #1061/#1062 add a guard that fails fast with the correct command. The guard checks **for the files, not the branch name**, so it needs no change once alpha promotes and main legitimately gains them.

### Tauri rejects non-RGBA bundled icons
`generate_context!()` fails at compile time with `icon ... is not RGBA`. Image libraries quantise flat artwork to indexed-palette PNGs by default. Any icon generator must force `palette: false` **and** an explicit alpha channel. This broke macOS *and* Windows builds simultaneously and passed every Linux-side check, because the icon *content* was verified but its *encoding* was not.

### The app icon has no vector source in the repo
The bundled 1024px master arrived as an external brand drop. `assets/brand/icon.svg` is *different artwork*. `scripts/generate-icons.mjs` is macOS-only and writes only to `assets/brand/` — it does **not** produce the bundled icons. The new `scripts/generate-app-icons.mjs` produces the bundled set cross-platform, but only the small/medium variants have committed vector sources; the 1024 master remains unreproducible.

### Never ship a folder inside a DMG
Considered and rejected for the MeedyaSuite grouping: when the destination folder already exists, Finder's default **Replace** deletes it and every app inside — i.e. one drag could wipe every sibling MeedyaSuite app. The sibling-repo issues lead with this warning. The app relocating itself is the safe route, and is the only one that reaches existing installs, since in-app updates never re-run an installer.

## 5. Release-notes confidentiality policy (new)

`.github/release-notes/STYLE_GUIDE.md` now bans disclosing **how** a feature is delivered — credentials, acquisition paths, protocol internals, storage/crypto internals — even in perfect plain English. Admission test for vocabulary: **a term is allowed only if it appears verbatim as a label in the shipped UI.**

Enforcement: `scripts/release-notes/lint-notes.py`, wired into `release-note-gate.yml` (blocking) and into the apply workflow (blocking even in dry-run).

Decisions taken: technical changelog renders **subjects only** (commit bodies dropped — they carried the most mechanism detail); CHANGELOG.md stays technical; legacy pre-v1.10 tags are not being retro-fixed.

**Known gap:** the self-heal path that repairs regressed bodies checks *format*, not *content*, so it cannot detect a mechanism disclosure. Mitigated by blocking bad trailers at authoring time, but not closed.

## 6. In flight at time of writing

- Deep analysis: whether MeedyaDL checks for updates **before** the first-run setup wizard. Early reading of `src/App.tsx` suggests the wizard is already gated on `!appUpdateAvailable` after an awaited update check — so this may be largely handled, and the real questions are offline behaviour, what `appUpdateAvailable` actually reflects, and the ordering of three competing first-launch interruptions (update prompt / relocation prompt / wizard).
- A ranked backlog sweep across open and closed issues for alpha-cycle candidates.

## 7. Working conventions established this session

- **Deep analysis and planning → sequential Fable agents** (fall back to Opus if unavailable, retry Fable next time). **Implementation → Sonnet or Haiku.**
- **Combine related work into one PR** rather than stacking, to avoid merge races — especially when two changes touch `release.yml`.
- **Rebase-merge, not squash**, when every commit carries its own `Release-Note:` trailer, so per-commit notes survive.
- Fan out verification **one item per agent**. A single agent cannot hold thirty release payloads; the first attempt at the retro-fix preview died that way, and the fan-out version completed with better coverage.
- Adversarial review earns its keep on user-facing text: the first `alpha.38` draft was rejected for claims that were simply false (cookies aren't migrated, the icon fix isn't Mac-only, the version threshold was wrong).
