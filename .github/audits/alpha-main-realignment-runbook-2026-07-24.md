# Alpha ↔ Main Realignment — Executable Runbook (2026-07-24)

**Author:** Claude (Fable 5), planning pass on top of the completed content audit.
**Companion analysis (READ FIRST, it is the evidence base):** `.github/audits/alpha-main-drift-content-analysis-2026-07-24.md`
**Strategy:** B-then-A hybrid (analysis §3) — Phase 1 port the missing fragments, Phase 2 land prep→alpha, Phase 3 close git ancestry with a content-no-op merge, Phase 4 promote at the next stable cut.

**Refs frozen at time of writing** (re-run `git fetch --all --prune` and re-verify before starting):

| Ref | SHA | Version stamp |
| --- | --- | --- |
| `origin/main` | `ee82063` | 1.10.1 |
| `origin/alpha` | `ce935dc` | 1.11.0-alpha.30 (manifest 1.11.0) |
| `prep/alpha-gamdl-3.8.2-plus-2026-07-10` (= `origin/prep/...`) | `a44a6a4` | 1.12.0-alpha.28 (manifest 1.11.0) |
| merge-base(main, alpha) | `e0a1ee6` (2026-04-20) | — the illusory "681 behind" root |
| merge-base(alpha, prep) | `36bc0a1` | prep is +60 / −3 vs alpha; the 3 alpha-only commits are `d0dc628`, `3f7b241`, `ce935dc` |
| `origin/beta` | `ef95c30` | strict ancestor of main (0 ahead / 26 behind, parked at v1.9.4-era) |

**Phase safety classification:**

| Phase | Mutates shared branches? | Automation-safe? |
| --- | --- | --- |
| 0 (preconditions) | No (tags only) | Yes |
| 1 (content port) | Via reviewed PR only | Yes — safe to hand to sub-agents |
| 2 (land prep) | `prep` force-push + `alpha` via rebase-merge PR | Yes with care (Sonnet) — human clicks the merge button |
| 3 (ancestry closure) | **`alpha` directly (merge commit)** | **NO — REQUIRES EXPLICIT HUMAN GO-AHEAD** |
| 4 (promotion) | `beta`, `main` | **NO — human-led at the stable cut** |

---

## 0. Ground rules (hard constraints — violating any of these loses work)

1. **NEVER run the `realign-alpha` workflow** at any point in this process (it resets `alpha` onto `main`, discarding alpha's 52+ unique commits). This ban survives Phase 3 — even after ancestry closure the workflow's reset semantics are destructive.
2. **NEVER let a naive `git merge origin/main` land on alpha.** A plain merge SILENTLY RESURRECTS five files alpha deliberately deleted (verified in the simulated merge tree `d116419`, analysis §2): `.github/workflows/nightly-release.yml`, `.github/workflows/weekly-release.yml`, `.github/workflows/monthly-release.yml`, `.github/workflows/upstream-gamdl-watch.yml`, `.github/rulesets/protected-cron-channels.json`. These are add-on-main/delete-on-alpha cases git does **not** flag as conflicts. Resurrected cron workflows would resume cutting nightly/weekly/monthly releases; the resurrected watcher would double-file upstream issues against `upstream-engine-watch.yml`. Phase 3 §4 has the explicit guard.
3. **Keep each branch's own version stamps**: main 1.10.1 · alpha 1.11.0-alpha.30 (→ auto-bumps to 1.12.0-alpha.31+ after Phase 2) · prep 1.12.0-alpha.28 · `.release-please-manifest.json` stays 1.10.1 on main and 1.11.0 on alpha/prep until the stable cut. Never "fix" a stamp mid-merge.
4. **Never hand-merge `Cargo.lock` / `package-lock.json`.** Take alpha's (newest pins), then regenerate: `npm install --package-lock-only` and `cd src-tauri && cargo check`.
5. **prep→alpha MUST be a rebase-merge** (no merge commits inside prep — verified: prep's 60 commits contain 0 merges). This is why the Phase 3 ancestry merge comes AFTER Phase 2, never inside it.
6. **Never squash-merge any cross-branch reconciliation or promotion PR** (Phases 3–4). Squash-importing is exactly what created this drift (`674967f`, analysis §0).
7. **Every push to `alpha` auto-cuts an alpha release** (`alpha-release.yml` is push-driven; its #906 guard only skips its own `chore(alpha): X.Y.Z-alpha.N` bump commits). Expect one auto-build per phase landing. This is normal — do not suppress with `[skip ci]`.
8. Per project convention: **no auto-commit/auto-push by agents** — implementers prepare commits on the work branch; the human operator (or explicitly-authorised orchestrator step) pushes.

---

## 1. Preconditions & backups

### 1.1 Verify clean state and full history

```bash
cd /home/user/MeedyaDL
git fetch --all --prune --tags

# Clean tree (untracked audit .md files are acceptable):
git status --porcelain | grep -v '^\?\?' && { echo "DIRTY TREE — stop"; exit 1; } || echo "tree clean"

# Unshallow confirmation (must print "false" or nothing):
git rev-parse --is-shallow-repository

# Confirm the frozen SHAs still hold (if any moved, STOP and re-run the drift analysis deltas):
[ "$(git rev-parse origin/main)"  = "ee82063ca11c819297692a4aa20192db63988f64" ] || echo "WARN: main moved"
[ "$(git rev-parse origin/alpha)" = "ce935dcadd8322055602fb0ab611445559805488" ] || echo "WARN: alpha moved"
[ "$(git rev-parse origin/prep/alpha-gamdl-3.8.2-plus-2026-07-10)" = "a44a6a4d84694de87f23996fc69ef78a2dc5a564" ] || echo "WARN: prep moved"
```

### 1.2 Create safety tags (and push them — tags are the rollback anchors)

```bash
git tag backup/alpha-pre-realign  ce935dc   # origin/alpha as of 2026-07-24
git tag backup/prep-pre-realign   a44a6a4   # prep tip as of 2026-07-24
git tag backup/main-pre-realign   ee82063   # main (reference only; main is never mutated before Phase 4)
git push origin backup/alpha-pre-realign backup/prep-pre-realign backup/main-pre-realign
```

Before Phase 3 additionally: `git tag backup/alpha-pre-ancestry-merge origin/alpha && git push origin backup/alpha-pre-ancestry-merge`.

### 1.3 Baseline marker snapshot (all must be MISSING now; Phase 1 flips them)

```bash
for m in 'no_lyrics_available' 'keeping line-level lyrics' 'MusicKit credentials required (Settings > Quality'; do
  git grep -c "$m" origin/alpha -- src-tauri/src/services/download_queue.rs && echo "UNEXPECTED: $m already present" || true
done
git grep -c 'group: release-' origin/alpha -- .github/workflows/release.yml || echo "concurrency guard absent (expected)"
git cat-file -e origin/alpha:.editorconfig 2>/dev/null && echo "UNEXPECTED" || echo ".editorconfig absent (expected)"
git cat-file -e origin/alpha:.github/workflows/pr-security.yml 2>/dev/null && echo "UNEXPECTED" || echo "pr-security.yml absent (expected)"
```

### 1.4 Rollback procedures (per phase)

| Phase | Rollback | Notes |
| --- | --- | --- |
| 1 (before PR merge) | delete the work branch | zero shared-state impact |
| 1 (after PR merge to alpha) | `git revert` the landed commit(s) on a new PR to alpha | alpha's ruleset blocks force-push — roll FORWARD via revert, never reset. The backup tag gives the exact pre-state for diffing: `git diff backup/alpha-pre-realign origin/alpha` |
| 2 (prep force-push went wrong) | `git push --force-with-lease origin backup/prep-pre-realign:refs/heads/prep/alpha-gamdl-3.8.2-plus-2026-07-10` | prep is a work branch — force-restore from tag is allowed |
| 2 (after rebase-merge into alpha) | revert the 60-commit range via `git revert --no-commit <first>^..<last>` on a PR, or accept and fix forward | prefer fix-forward; a 60-commit revert is itself risky |
| 3 (ancestry merge landed) | `git revert -m 1 <merge-sha>` — **WARNING:** reverting a merge poisons future merges of main (git will consider main's commits "already merged" even after the revert; re-doing Phase 3 later requires reverting the revert first). Treat Phase 3 as effectively one-way; that is why it gates on human go-ahead | tree content is unaffected either way (the merge is a content no-op) — the revert only re-breaks ancestry |
| 4 | standard release rollback (#267 rollback UI, release-please revert) | out of scope here |

---

## 2. Phase 1 — content port (SAFE, additive)

**Branch:** `port/main-v1.10.1-fragments`, created from `origin/alpha`. Delivered as a normal PR → `alpha`.

```bash
git checkout -b port/main-v1.10.1-fragments origin/alpha
```

**PR mechanics:** one conventional commit per fragment (see §6 work items). Recommend **rebase-merge** for this PR so git-cliff renders each fragment in the next alpha build's notes (#1027); squash is acceptable if the maintainer prefers — the markers don't care. The PR body MUST end with `Release-Note:` trailer(s) (#1028); suggested set is in WI-9.

Every fragment below names: the target file, the exact change, the source of truth on main, and a grep-verifiable acceptance marker (reusing analysis §4). "Verbatim-portable" = alpha's target region is byte-compatible with main's pre-#947 text (verified in planning); "hand-apply" = the target was rewritten on alpha and the change must be re-expressed in the new structure.

### F1 — #942: MV-companion token/relation failures surfaced (hand-apply-lite, Sonnet)

* **Target:** `src-tauri/src/services/download_queue.rs`, fn `spawn_music_video_companion_inner`, alpha lines ~4230–4256 (the `Ok(None)` / `Err(e)` token arms, the token-source debug line, and the relation-lookup `Err(e)` arm).
* **Source:** `git show 134e3e8c -- src-tauri/src/services/download_queue.rs` — the first 3 hunks (@3942, @3963). Alpha's current code is the identical pre-#947 shape (verified: `log::debug!("Music video companion skipped for {dl_id}: no MusicKit token available")` at alpha:4234), so the hunks apply near-verbatim; only line offsets differ.
* **Change:** (a) `Ok(None)` arm → `emit_download_log(app, dl_id, "Music video lookup skipped — MusicKit credentials required (Settings > Quality > Video Quality)")`; (b) `Err(e)` token arm → `emit_download_warn(..., "Music video lookup skipped — MusicKit token resolution failed: {e}")`, drop the `log::debug!`; (c) token-source `log::debug!` → `crate::utils::activity_log::emit_verbose_download_log(app, dl_id, &format!("Music video companion: using MusicKit token from {token_source}"))`; (d) relation `Err(e)` arm → `emit_download_warn(..., "Music video relation lookup failed: {e}")`. Carry main's explanatory comments.
* **Markers:**
  ```bash
  git grep -c 'MusicKit credentials required (Settings > Quality' <ref> -- src-tauri/src/services/download_queue.rs  # ≥1
  git grep -c 'Music video relation lookup failed'                <ref> -- src-tauri/src/services/download_queue.rs  # ≥1
  ```

### F2 — #935 quick-win A: 4-way syllable-lyrics outcome summary (hand-apply-lite, Sonnet)

* **Target:** `src-tauri/src/services/download_queue.rs`, the syllable-lyrics enrichment loop, alpha ~9005–9103 (`let mut upgraded = 0u32;` … the `if upgraded > 0 { … "Word-level lyrics fetched from Apple Music API…" }` block at alpha:9090–9103).
* **Source:** same `git show 134e3e8c` — hunks @8518, @8578, @8585, @8606.
* **Change:** (a) after `let mut upgraded = 0u32;` add `let mut no_lyrics_available = 0u32;` + `let mut errored = 0u32;`; (b) in the `Ok(None)` arm (alpha ~9050, "No syllable-lyrics available for track") add `no_lyrics_available += 1;`; (c) in the `Err(e)` arm add `errored += 1;`; (d) replace the closing `if upgraded > 0 { … }` summary with main's full 4-way `total_attempted` summary block (all-upgraded / mixed / all-unavailable "keeping line-level lyrics" / all-errored), verbatim including comments.
* **Non-conflict note for Phase 2:** prep's #969 change in this function is confined to the FILTER above the loop (alpha 8959–8981: `has_lyrics != Some(false)` + `ttml_has_word_timing`); the F2 region (9005+) is disjoint — the later prep rebase should replay clean (at worst a context-fuzz stop; resolution = keep both).
* **Markers:**
  ```bash
  git grep -c 'no_lyrics_available'        <ref> -- src-tauri/src/services/download_queue.rs  # ≥1
  git grep -c 'keeping line-level lyrics'  <ref> -- src-tauri/src/services/download_queue.rs  # ≥1
  ```

### F3 — MusicBrainz tier lookup promoted to activity log (mostly-verbatim, Sonnet)

* **Targets:** `src-tauri/src/services/musicbrainz_service.rs` (fns `lookup_videos_for_tracks` at alpha:274, `lookup_videos_for_tracks_enhanced` at alpha:297) **and** the single call-site `src-tauri/src/services/download_queue.rs:9927`.
* **Source:** `git show 134e3e8c -- src-tauri/src/services/musicbrainz_service.rs` (whole diff) + the final download_queue hunk (@9365: adds `&enrich_app, &enrich_dl_id,` args).
* **Change:** (a) add `app: &tauri::AppHandle, download_id: &str` leading params to both fns; thread through the internal call at alpha:288; (b) add `use crate::utils::activity_log::{emit_download_log, emit_verbose_download_log};` at the top of the enhanced fn; (c) convert the 9 tier `log::debug!` lines (alpha:318–443 region) to `emit_verbose_download_log` and add the 3 `emit_download_log` "MusicBrainz: matched song … (Tier N)" success lines — verbatim from main; (d) update the sole call-site at `download_queue.rs:9927` to pass `&enrich_app, &enrich_dl_id,` first.
* **Verified safe:** no other callers exist on alpha (`git grep 'lookup_videos_for_tracks' origin/alpha -- src-tauri/src` → only download_queue:9927); no test invokes either fn, so the signature change needs no test-side AppHandle. Prep's `lookup_all_platform_urls` (prep:574) is a separate fn, unaffected.
* **Markers:**
  ```bash
  git grep -c 'emit_verbose_download_log' <ref> -- src-tauri/src/services/musicbrainz_service.rs  # ≥9
  git grep -c 'MusicBrainz: matched song' <ref> -- src-tauri/src/services/musicbrainz_service.rs  # =3
  ```

### F4 — #945 aria-labels in the #911-rewritten QueueItem.tsx (HAND-APPLY, Sonnet — do NOT cherry-pick)

Alpha's #911 rewrite moved/restyled all three buttons; main's diff will not apply. Re-express by hand at these verified alpha sites:

1. **Retry-without-wrapper pill** (alpha ~838–848, `data-testid="retry-without-wrapper-pill"`): the button already carries `aria-label="Retry without wrapper authentication"` (from alpha's #890). **Replace** that value with main's #945 long form: `aria-label="Retry without wrapper (uses cookie-based authentication)"`. Keep alpha's #890 styling and `title` unchanged. (Decision: main's wording wins — it carries the cookie-vs-wrapper distinction and satisfies the audit marker. If the maintainer prefers #890's wording, the marker in this runbook must be consciously amended — flag in the PR.)
2. **Open File button** (alpha ~880–887, `onClick={handleOpenFile}` / `title="Open in default application"`): add `aria-label="Open downloaded file in default application"`.
3. **Open Folder button** (alpha ~889–896, `onClick={handleOpenFolder}` / `title="Reveal in file manager"`): add `aria-label="Reveal downloaded file's folder in file manager"`.

Optionally carry main's short `//` rationale comments (valid between JSX attributes). The #911 context-menu entries (alpha ~459, ~514) render text labels and need no change.

* **Markers:**
  ```bash
  git grep -c 'Retry without wrapper (uses cookie-based'       <ref> -- src/components/download/QueueItem.tsx  # ≥1
  git grep -c 'Open downloaded file in default application'    <ref> -- src/components/download/QueueItem.tsx  # ≥1
  git grep -c "Reveal downloaded file's folder in file manager" <ref> -- src/components/download/QueueItem.tsx  # ≥1
  ```
* Run `npm run test` — `QueueItem`-adjacent tests must stay green (labels are additive; only the pill's aria value changes — check no test asserts the #890 string: `git grep -n 'Retry without wrapper authentication' src/`).

### F5 — SettingsPage tab renames (trivial, Haiku)

* **Target:** `src/components/settings/SettingsPage.tsx` alpha lines 148–149.
* **Change:** `label: 'Quality'` → `label: 'Codec & Resolution'`; `label: 'Fallback'` → `label: 'Codec Fallback Order'`. IDs (`'quality'`, `'fallback'`) unchanged.
* **Markers:** `git grep -c "label: 'Codec & Resolution'" <ref> -- src/components/settings/SettingsPage.tsx` = 1; same for `'Codec Fallback Order'`. Then `git grep -rn "'Quality'\|'Fallback'" src/ --include='*.test.*'` → confirm no test asserts the old labels.

### F6 — release.yml #944 concurrency guard (trivial with exact anchor, Haiku)

* **Target:** `.github/workflows/release.yml`. Alpha's file has NO `concurrency:` key (verified). Insertion point is **identical to main's**: between the end of the top-level `on:` block (last `workflow_dispatch` input, ~line 87) and `jobs:` (alpha line 88).
* **Change:** insert main's 28-line block verbatim (comment + guard). Extract with:
  ```bash
  git show origin/main:.github/workflows/release.yml | sed -n '/^# Concurrency guard/,/^  cancel-in-progress: false/p'
  ```
  Semantics unchanged in alpha's evolved file: `group: release-${{ inputs.tag || github.ref }}`, `cancel-in-progress: false`. Prep's release.yml changes start at line 149+ — no overlap, Phase 2 rebase-safe.
* **Marker:** `git grep -c 'group: release-' <ref> -- .github/workflows/release.yml` = 1. Sanity: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))"` (or actionlint if available).

### F7 — `.editorconfig` (trivial, Haiku)

* **Change:** `git show origin/main:.editorconfig > .editorconfig` (56 lines, wholesale copy — file absent on alpha and prep).
* **Marker:** `git cat-file -e <ref>:.editorconfig`.

### F8 — `.gitignore` entries (trivial, Haiku)

* **Target:** alpha `.gitignore` line 96 (`.debugLogs/`).
* **Change:** `.debugLogs/` → `.debugLogs/*` and add `.examplefiles/*` on the next line (matches main's hunk). Prep's own `.gitignore` addition is at lines ~62–75 (Claude Code section) — disjoint, rebase-safe.
* **Marker:** `git grep -c '\.examplefiles/\*' <ref> -- .gitignore` = 1.

### F9 — README TTML-spec reference (trivial, Haiku)

* **Target:** `README.md` — insert main's 4-line `### Reference documentation` section (Apple Music TTML Lyrics Specification link into MeedyaSuite-core) immediately after the tech-stack table (alpha's `| **Engines** | …` row is line 271), before the following `---`. Extract: `git show origin/main:README.md | sed -n '/^### Reference documentation/,/^---$/p'` (drop the trailing `---`).
* **Marker:** `git grep -c 'APPLE_MUSIC_TTML_SPEC' <ref> -- README.md` = 1. Prep's README hunks are at lines 110–136 — disjoint.

### F10 — #905 PR-security suite (mostly verbatim copies, Sonnet for the two doc-line adaptations)

* **Verbatim copies from `origin/main`** (6 files, all absent on alpha/prep):
  ```bash
  for f in .github/workflows/pr-security.yml .github/pull_request_template.md \
           tools/audit-checks/README.md tools/audit-checks/check_codec_registry.py \
           tools/audit-checks/check_ipc_commands.py .claude/memory/project_pr_security_checks.md; do
    mkdir -p "$(dirname "$f")"; git show "origin/main:$f" > "$f"
  done
  ```
  `pr-security.yml` already targets PRs to `main`/`release-candidate`/`beta`/`alpha` — no trigger edit needed.
* **Doc-line adaptations (not verbatim):** (a) `.claude/CLAUDE.md` — insert main's #905 bullet ("PR security heuristics run per-PR: …", see `git show 06369e1 -- .claude/CLAUDE.md`) into alpha's evolved CLAUDE.md architecture/conventions list; (b) `.claude/memory/MEMORY.md` — append main's index line for `project_pr_security_checks.md`, **rewording its trailing note**: main's line references "the unresolvable `setup-python` SHA still in `upstream-gamdl-watch.yml`" — that workflow does not exist on alpha (retired for `upstream-engine-watch.yml`); adapt or drop the clause.
* **Pre-verified in planning (2026-07-24):** both audit scripts run zero-finding against BOTH alpha's tree (124 IPC commands consistent; codec registry consistent) and prep's tree. If a future tree change surfaces findings, triage: real drift → fix; false positive → note in PR (the checks are non-blocking advisory by design).
* **Markers:**
  ```bash
  git cat-file -e <ref>:.github/workflows/pr-security.yml && git cat-file -e <ref>:.github/pull_request_template.md
  git cat-file -e <ref>:tools/audit-checks/check_ipc_commands.py
  python3 tools/audit-checks/check_ipc_commands.py && python3 tools/audit-checks/check_codec_registry.py  # both exit 0
  ```

### F11 — CHANGELOG splice + SECURITY row (trivial with exact source, Haiku)

* **CHANGELOG.md:** alpha's file starts `## [1.9.4]` at line 7 and lacks `[1.10.1]`/`[1.10.0]`. Splice main's two sections — **exact source** `git show origin/main:CHANGELOG.md | sed -n '14,171p'` (line 14 = `## [1.10.1] - 2026-06-19`, line 171 = last line before `## [1.9.4]` at 172) — inserted between alpha's header (line 6) and `## [1.9.4]` (line 7), preserving one blank line on each side. Do **NOT** splice main's `[Unreleased]` block (main lines 7–13 — `[skip ci]` doc records only).
* **SECURITY.md:** alpha's supported-versions table (lines 12–13, between the sentinel comments) says `1.10.0` / `!= 1.10.0` → change to `1.10.1` / `!= 1.10.1` (matching main). Self-heals via `update-security-policy.yml` on the next tag anyway; doing it now keeps the branch honest.
* **Markers:**
  ```bash
  git grep -c '^## \[1.10.1\]' <ref> -- CHANGELOG.md  # =1
  git grep -c '^## \[1.10.0\]' <ref> -- CHANGELOG.md  # =1
  git grep -c '1.10.1'         <ref> -- SECURITY.md   # =2
  ```

### F12 — clippy 1.97 `useless_borrows_in_formatting` fix (trivial, Haiku) — see §7

* **Target:** `src-tauri/src/services/download_queue.rs` alpha line 7663 — `&urls,` as a `format!` argument inside the `"URLs: {:?} | Codec: {} | Native priority: {}"` verbose log. Change to `urls,`. (The `&urls` at alpha:7346 is a function argument — leave it.)
* **Marker:** `git show <ref>:src-tauri/src/services/download_queue.rs | grep -A2 'URLs: {:?}' | grep -c '&urls'` = 0.

### F13 — npm audit advisories (lockfile-only, Sonnet) — see §7

* Run `npm audit fix`; expect lockfile-only bumps for js-yaml (→ ≥4.2.1), brace-expansion, fast-uri (all transitive). Verify `git diff --name-only` = `package-lock.json` only; then `npm run type-check && npm run test`.
* **Marker:** `npm audit --package-lock-only --audit-level=high` exits 0.
* Housekeeping: the open `dependabot/npm_and_yarn/brace-expansion-5.0.7` branch/PR becomes redundant — close it with a comment referencing this PR.

### Phase 1 exit — full verification battery (analysis §4 + additions)

Run against the work branch (`ref=HEAD`), then again against `origin/alpha` after the PR merges:

```bash
ref=HEAD
git grep -c no_lyrics_available $ref -- src-tauri/src/services/download_queue.rs               # ≥1
git grep -c 'keeping line-level lyrics' $ref -- src-tauri/src/services/download_queue.rs       # ≥1
git grep -c 'MusicKit credentials required (Settings > Quality' $ref -- src-tauri/src/services/download_queue.rs  # ≥1
git grep -c 'MusicBrainz: matched song' $ref -- src-tauri/src/services/musicbrainz_service.rs  # =3
git grep -c 'group: release-'   $ref -- .github/workflows/release.yml                          # =1
git grep -c 'Retry without wrapper (uses cookie-based' $ref -- src/components/download/QueueItem.tsx  # ≥1
git grep -c "label: 'Codec & Resolution'" $ref -- src/components/settings/SettingsPage.tsx     # =1
git cat-file -e $ref:.github/workflows/pr-security.yml && git cat-file -e $ref:.editorconfig
git grep -c '\.examplefiles/\*' $ref -- .gitignore                                             # =1
git grep -c 'APPLE_MUSIC_TTML_SPEC' $ref -- README.md                                          # =1
git grep -c '^## \[1.10.1\]' $ref -- CHANGELOG.md                                              # =1
git ls-tree --name-only $ref .github/workflows/ | grep -cE 'nightly|weekly|monthly|upstream-gamdl'  # =0 (no resurrections)
python3 tools/audit-checks/check_ipc_commands.py && python3 tools/audit-checks/check_codec_registry.py
export PATH="$HOME/.cargo/bin:$PATH"
( cd src-tauri && cargo test --lib && cargo clippy --all-targets -- -D warnings )
npm run type-check && npm run test
```

Then: open PR `port/main-v1.10.1-fragments` → `alpha`, human review, merge (rebase-merge preferred). The merge push auto-cuts an alpha build (expected).

---

## 3. Phase 2 — land prep→alpha (rebase-merge)

**Precondition:** Phase 1 merged to `alpha`. Re-run `git fetch origin`.

**Verified facts this plan relies on:** prep has **0 merge commits** in `origin/alpha..prep`; the prep-vs-alpha conflict surface is exactly **5 files, all version stamps + lockfiles** (`package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json` — measured via `git merge-tree`); only **3 prep commits** touch those files (`0881cd1`, `ebe239d` — the #1034 security pair — and `456641e` — the 1.12.0-alpha.28 bump); alpha's 3 post-fork commits (`d0dc628` serde_json, `3f7b241` npm bumps #1035, `ce935dc` alpha.30 stamp) live on the alpha side and are **automatically preserved** by rebasing prep ON TOP of alpha — they are never rewritten.

### 3.1 Rebase prep onto the reconciled alpha (local)

GitHub's Rebase-and-merge button will be greyed out while the 5-file conflict exists, so rebase locally first:

```bash
git checkout prep/alpha-gamdl-3.8.2-plus-2026-07-10
git rebase origin/alpha
```

Expected stops (≤3, at the commits named above, plus possible context-fuzz where Phase 1's F2/F6/F8/F9 landed near prep hunks — all verified disjoint-region, so fuzz stops should be rare). **Resolution rules per file:**

| File | Rule |
| --- | --- |
| `package.json` | Keep **alpha's dependency pins** (they are newer — prep forked before #1035; e.g. `@sentry/browser ^10.66.0` beats prep's `^10.65.0`) + take **prep's `"version"` field** (`1.12.0-alpha.28`) and any dep prep genuinely added/removed |
| `src-tauri/Cargo.toml` | Take **prep's structural changes** (the #1034 `[features]` block, `devtools` removed from default `tauri` features, security comments) + prep's version field. serde_json is range-pinned in Cargo.toml, so #1036 lives only in Cargo.lock — no manifest collision |
| `src-tauri/tauri.conf.json` | Take prep's version field; no other deltas |
| `Cargo.lock`, `package-lock.json` | **NEVER hand-merge.** `git checkout origin/alpha -- src-tauri/Cargo.lock package-lock.json` (alpha's pins are newest, incl. #1035/#1036), `git add`, continue; regenerate once at the end (3.2) |

After the final `git rebase --continue`:

### 3.2 Regenerate lockfiles once, at the tip

```bash
npm install --package-lock-only          # re-syncs version field + resolves prep's manifest deltas
( cd src-tauri && cargo check )          # re-syncs meedyadl version + re-resolves the tauri feature change (devtools removal)
git status --porcelain                   # if lockfiles changed, amend them into the version-bump commit:
git add package-lock.json src-tauri/Cargo.lock && git commit --amend --no-edit   # amend 456641e's replay (or a dedicated fixup)
```

### 3.3 Pre-push invariants

```bash
# Every original prep commit replayed (patch-id equivalence; expect 60 lines, all present):
git range-diff backup/prep-pre-realign~60..backup/prep-pre-realign origin/alpha..HEAD | grep -c '^ *[0-9]*: *[0-9a-f]* [=!]' 
# No prep patch lost (all '-' = present in HEAD):
git cherry HEAD backup/prep-pre-realign | grep -c '^+' || echo "0 lost — good"   # expect 0 '+' beyond the amended stamp commits
# Alpha's 3 commits are the new base (must be ancestors):
git merge-base --is-ancestor ce935dc HEAD && echo "alpha.30 preserved"
# Phase 1 markers survived the rebase (run the §2 battery with ref=HEAD)
# Build gates:
( cd src-tauri && cargo test --lib ) && npm run type-check && npm run test
```

### 3.4 Push + merge

```bash
git push --force-with-lease origin prep/alpha-gamdl-3.8.2-plus-2026-07-10
```

Open (or refresh) the **prep→alpha PR** — if one is already held open, the force-push updates it in place; if not: `gh pr create --base alpha --head prep/alpha-gamdl-3.8.2-plus-2026-07-10 …`. Wait for CI green, then merge via **"Rebase and merge"** (mandatory — preserves the 60 conventional commits for git-cliff; a squash would collapse prep's release-note grouping, analysis §4 row 2).

### 3.5 Post-merge invariants

```bash
git fetch origin
git cherry origin/alpha <local-rebased-prep-tip> | grep -c '^+'        # =0 — every prep patch is in alpha
git merge-base --is-ancestor ce935dc origin/alpha && echo OK           # alpha.30 line intact
# Phase 1 marker battery again with ref=origin/alpha
# Version stamps: package.json on alpha now reads 1.12.0-alpha.28; the push triggers
# alpha-release.yml which computes the next monotonic counter → expect an auto tag
# v1.12.0-alpha.31 (or higher) and a release build. Do NOT hand-correct stamps.
```

---

## 4. Phase 3 — ancestry closure ⚠️ **REQUIRES EXPLICIT HUMAN GO-AHEAD — DO NOT AUTOMATE**

**Goal:** record `origin/main` (`ee82063`) as merged into `alpha` while changing **zero content** — Phases 1–2 already made alpha's tree a superset of main's genuinely-new content. After this, the merge-base jumps to `ee82063`, the "681 behind" illusion dies, and every future main↔alpha operation is cheap and honest.

**Gate:** maintainer confirms (a) Phases 1–2 landed and their invariants pass on `origin/alpha`; (b) `origin/main` has not moved past `ee82063` (if it has, STOP — re-run the fragment analysis on the new commits first: `git log --oneline ee82063..origin/main`); (c) the backup tag exists: `git tag backup/alpha-pre-ancestry-merge origin/alpha && git push origin backup/alpha-pre-ancestry-merge`.

### 4.1 Approach A — `-s ours` merge (RECOMMENDED)

Justified because §1 of the analysis proves the remaining main-side delta is fully represented in alpha post-Phase-1 (this is exactly the condition the analysis set for `-s ours` acceptability). Zero interaction, provably content-neutral:

```bash
git fetch origin
git checkout -B ancestry-closure origin/alpha
git merge -s ours --no-ff origin/main -m "chore(alpha): close git ancestry with main @ v1.10.1 (ee82063)

Content-verified no-op merge. All genuinely-missing main fragments were
ported in PR <Phase-1 PR#> and verified via the marker battery in
.github/audits/alpha-main-realignment-runbook-2026-07-24.md; the
remaining main-side delta is content-present in alpha per
.github/audits/alpha-main-drift-content-analysis-2026-07-24.md.
Tree is byte-identical to pre-merge alpha (-s ours). Restores an honest
merge-base so 'commits behind main' reporting works again."
```

### 4.2 Approach B — normal merge, resolved keep-alpha, with explicit re-deletion

Use only if the maintainer wants per-file reviewability of the ~69 false conflicts (analysis §2), or wants to union any doc content discovered missing after Phase 1:

```bash
git checkout -B ancestry-closure origin/alpha
git merge --no-ff --no-commit origin/main || true          # expect ~69 conflicted files

# 1) Resolve ALL content conflicts to alpha's side:
git diff --name-only --diff-filter=U | while read -r f; do
  git checkout --ours -- "$f" 2>/dev/null && git add "$f" || true
done
# 2) modify/delete conflicts (files alpha deleted, main modified — e.g. old assets/logo/*.svg):
#    resolve by KEEPING THE DELETION:
git status --porcelain | awk '$1=="DU"||$1=="UD"{print $2}' | xargs -r git rm -f --
# 3) THE RESURRECTION GUARD — these five arrive as clean, UNCONFLICTED adds; remove explicitly:
git rm -f .github/workflows/nightly-release.yml .github/workflows/weekly-release.yml \
          .github/workflows/monthly-release.yml .github/workflows/upstream-gamdl-watch.yml \
          .github/rulesets/protected-cron-channels.json
# 4) Audit EVERY file the merge staged as an add — each must be individually justified
#    (expected result after Phase 1: NONE remain):
git diff --cached --name-only --diff-filter=A
# 5) (optional) deliberate doc unions go here, individually, with reviewer sign-off
git commit   # merge commit message as in Approach A
```

### 4.3 Verification (BOTH approaches — all four must pass before pushing)

```bash
# 1) Tree equality — the merge changed NOTHING (empty output; Approach B with deliberate
#    doc unions: output must list ONLY those files):
git diff --stat backup/alpha-pre-ancestry-merge HEAD
# 2) Ancestry closed:
git rev-list --count HEAD..origin/main            # =0
git merge-base HEAD origin/main                   # = ee82063…
# 3) Resurrection-file guard (MUST be 0):
git ls-tree --name-only HEAD .github/workflows/ | grep -cE 'nightly-release|weekly-release|monthly-release|upstream-gamdl-watch'
git ls-tree HEAD .github/rulesets/protected-cron-channels.json | wc -l    # =0
# 4) Alpha-only files survived:
git cat-file -e HEAD:.github/workflows/lint.yml && git cat-file -e HEAD:.github/workflows/upstream-engine-watch.yml
# 5) Version stamps untouched:
git show HEAD:package.json | grep '"version"'     # still the current alpha stamp
git show HEAD:.release-please-manifest.json       # still {"." : "1.11.0"}
# 6) Marker battery one more time with ref=HEAD; then cargo test --lib / type-check / vitest
```

### 4.4 Delivery — two routes

* **Route 1 — reviewable PR (preferred):** push `ancestry-closure`, open PR → `alpha`. The PR diff will show ~nothing (tree-identical), which IS the reviewable claim; reviewers check the §4.3 outputs pasted into the PR body. **Merge with the "Create a merge commit" button ONLY.** GitHub's *squash* would flatten the ancestry merge into a normal commit (defeating the entire purpose) and *rebase-merge* drops/linearises merge commits — both are forbidden here.
* **Route 2 — direct push (maintainer):** `git push origin ancestry-closure:alpha`. This is a fast-forward-shaped push (old alpha tip is the merge's first parent) — permitted by the alpha ruleset (which blocks only force-pushes).

Either route: the push auto-cuts another alpha build (expected). Afterwards `git rev-list --count origin/alpha..origin/main` reports ~0 and `realign-alpha`-class accidents lose their teeth — **but the realign-alpha ban stays** (its reset semantics would still discard alpha's unique line).

**Rollback caveat (repeat):** `git revert -m 1` of this merge re-breaks ancestry AND poisons a future re-merge (requires revert-of-the-revert). Treat as one-way.

---

## 5. Phase 4 — promotion (next stable cut, human-led)

Timing: whenever the maintainer next cuts stable (release-please computes 1.11.0/1.12.0 from alpha's line). Sequencing, exploiting the post-Phase-3 ancestry:

1. **alpha → beta:** `origin/beta` (`ef95c30`) is a strict ancestor of `origin/main` (0 ahead / 26 behind), and after Phase 3 `origin/main` is an ancestor of `origin/alpha` — therefore beta is a strict ancestor of alpha and the promotion is a **genuine fast-forward**: `git push origin <alpha-tip-sha>:beta` (or a PR merged with a merge commit if review is wanted; never squash). `beta-release.yml` cuts a beta build on push. Soak as long as desired.
2. **beta → main:** PR `beta` → `main` (or `alpha` → `main` directly if the beta soak is skipped). **Merge-commit ONLY — NEVER squash** (a squash here would recreate this exact drift; the `674967f` squash-import is the documented root cause). Conflict expectations: near-zero after Phase 3 (main is an ancestor); the only live surfaces are release-please stamps.
3. **Version/manifest normalisation at the cut:** keep the alpha-side `.release-please-manifest.json` (1.11.0) through the merge; release-please's Release PR then proposes the stable version — review that PR's computed version before merging it. The stable cut requires the curated `.github/release-notes/vX.Y.Z.md` file (the `Release Note Gate / release-pr-notes-file` check blocks until it exists — #1028 flow, `scripts/release-notes/draft-notes.sh`).
4. **rc leg (optional):** `origin/release-candidate` is stale (parked at 1.0.0-rc.21, 247 behind main). If the rc channel is to be used for this cut, first realign it the same fast-forward way (alpha → rc after Phase 3); otherwise leave parked and go alpha→beta→main.
5. Post-promotion: `update-security-policy.yml` rewrites SECURITY.md on the tag; `changelog.yml` regenerates CHANGELOG.md — verify the Phase-1 spliced `[1.10.1]`/`[1.10.0]` sections survive regeneration (git-cliff regenerates from history; after Phase 3 the ancestry contains main's release commits, so they should render — if not, the spliced text is already in the file as the fallback record).

---

## 6. Implementer task decomposition (Phases 1–2 — what the orchestrator hands out)

All Phase-1 items commit onto the single branch `port/main-v1.10.1-fragments`. Items WI-1…WI-8 touch **pairwise-disjoint files** and may run in any order (or in parallel as patches the orchestrator applies); WI-0 first, WI-9 last. Phase-2 items are strictly ordered and follow the Phase-1 PR merge.

| # | Title | Target files | Change (summary — full spec in §2/§3) | Acceptance marker | Model | Depends on |
| --- | --- | --- | --- | --- | --- | --- |
| WI-0 | Preconditions, backup tags, branch creation | (tags, branch) | §1.1–1.3 verbatim | tags exist on origin; branch off `ce935dc`-or-newer alpha | Haiku | — |
| WI-1 | Repo hygiene trio (F7+F8+F9) | `.editorconfig`, `.gitignore`, `README.md` | copy main's .editorconfig; 2 gitignore entries; README TTML section after line 271 | F7/F8/F9 markers | Haiku | WI-0 |
| WI-2 | Settings tab renames (F5) | `src/components/settings/SettingsPage.tsx` | 2 label strings, lines 148–149 | F5 markers | Haiku | WI-0 |
| WI-3 | release.yml concurrency guard (F6) | `.github/workflows/release.yml` | 28-line verbatim block before `jobs:` (line 88) | `group: release-` =1 + YAML parses | Haiku | WI-0 |
| WI-4 | CHANGELOG splice + SECURITY row (F11) | `CHANGELOG.md`, `SECURITY.md` | splice main lines 14–171 above alpha line 7; table 1.10.0→1.10.1 | F11 markers | Haiku | WI-0 |
| WI-5 | #905 suite port (F10) | 6 new files + `.claude/CLAUDE.md` + `.claude/memory/MEMORY.md` | verbatim copies + 2 adapted doc lines (reword the `upstream-gamdl-watch` clause) + run both scripts | F10 markers; scripts exit 0 | Sonnet | WI-0 |
| WI-6 | QueueItem aria-labels (F4) | `src/components/download/QueueItem.tsx` | hand-apply 3 labels into the #911 structure (sites ~840/~882/~892); pill label REPLACES #890 wording | F4 markers; vitest green | Sonnet | WI-0 |
| WI-7 | Rust port: F1 + F2 + F3 + F12 | `src-tauri/src/services/download_queue.rs`, `src-tauri/src/services/musicbrainz_service.rs` | #942 arms; #935-A counters + 4-way summary; MusicBrainz signature + 9 verbose + 3 matched logs + call-site :9927; `&urls`→`urls` at :7663. ONE agent — all four share `download_queue.rs` | F1/F2/F3/F12 markers; `cargo test --lib` + `cargo clippy -- -D warnings` green | **Sonnet** (largest item) | WI-0 |
| WI-8 | npm audit fix (F13) | `package-lock.json` only | `npm audit fix`; verify lock-only diff; close dependabot brace-expansion PR | `npm audit --audit-level=high` exit 0 | Sonnet | WI-0 (run last of the parallel set to avoid lock churn) |
| WI-9 | Verification + PR assembly | (none — runs battery, writes PR body) | full §2 exit battery; PR body with `Release-Note:` trailers, e.g. `Release-Note: Music-video lookups now tell you when MusicKit credentials are missing instead of failing silently.` / `Release-Note: The activity log now summarises word-level lyrics results for every album, including when Apple has none.` / `Release-Note: MusicBrainz video lookups now show their progress in the activity log.` / `Release-Note: Screen readers now announce what the queue's Retry/Open/Reveal buttons do.` / `Release-Note: Two settings tabs have clearer names: "Codec & Resolution" and "Codec Fallback Order".` | battery all-green; PR opened | Sonnet | WI-1…WI-8 |
| WI-10 | Rebase prep onto reconciled alpha | `prep/alpha-gamdl-3.8.2-plus-2026-07-10` (force-push with lease) | §3.1–3.3: rebase; 5-file resolution rules; lockfile regeneration; range-diff/cherry invariants | §3.3 checks all pass | **Sonnet** (needs care) | Phase-1 PR merged |
| WI-11 | Rebase-merge prep PR + post-merge invariants | (GitHub PR; human clicks) | §3.4–3.5; **Rebase and merge** button only | §3.5 checks on `origin/alpha` | Sonnet + human | WI-10 |

Phase 3 = single maintainer-supervised item (§4, Approach A unless review demanded) — **not** to be handed to an unattended sub-agent. Phase 4 = human checklist (§5).

**Parallelism note:** if WI-1…WI-8 run as concurrent sub-agents, have each produce a patch against the branch base and let the orchestrator `git am`/apply in WI order — the file sets are disjoint so application order is immaterial, but a single shared worktree must NOT host concurrent agents.

---

## 7. Global verification & CI expectations (pre-existing CI-rot, checked on alpha AND prep)

Both known main-side CI-rot items were probed against alpha and prep during planning — **both branches are affected** and the fixes are folded into Phase 1:

1. **clippy 1.97 `useless_borrows_in_formatting`** (`&<binding>` passed to `format!`-family): main's instance is `download_queue.rs:7228`; the SAME pattern exists on **alpha at `download_queue.rs:7663`** and **prep at `:7694`** (`&urls,` inside the `"URLs: {:?} | Codec: {} | Native priority: {}"` verbose log — verified by `git grep`). CI runs `cargo clippy -D warnings`, so the moment the toolchain hits 1.97 this fails the build on all three branches. **Fix = F12 (WI-7)**: drop the `&`. Prep inherits the fix through the Phase-2 rebase (no prep commit touches that line — verified).
2. **npm audit advisories** (from `npm audit --package-lock-only` against prep's lockfile, which shares all three pins with alpha and main):
   * `js-yaml` 4.2.0 — GHSA-52cp-r559-cp3m (high; YAML merge-key quadratic CPU), transitive via `cosmiconfig`; **affects main, alpha, prep equally** (all pin 4.2.0 — verified per-ref).
   * `brace-expansion` — GHSA-3jxr-9vmj-r5cp (high; DoS). An open `dependabot/npm_and_yarn/brace-expansion-5.0.7` branch already targets this — F13 supersedes it (close the dependabot PR).
   * `fast-uri` 3.0.0–3.1.3 — GHSA-v2hh-gcrm-f6hx / GHSA-4c8g-83qw-93j6 (high; host confusion).
   All three are dev/transitive; **fix = F13 (WI-8)**: `npm audit fix`, lockfile-only. Main stays affected until Phase 4 carries the fix (acceptable — main is dormant; note it in the Phase-1 PR body).
3. **#905 audit scripts**: pre-verified zero-finding against both alpha's tree (124 IPC commands consistent, codec registry consistent) and prep's worktree — F10 will not introduce a red advisory comment on day one.

**Expected CI status per phase:** Phase 1 PR — all green including the two rot fixes; the new `pr-security.yml` runs on the Phase-1 PR itself only after it lands on the base branch (GitHub uses the base's workflow set for `pull_request` events — it takes effect for subsequent PRs). Phase 2 PR — green modulo pre-existing prep CI state; the rebase must not introduce NEW failures (compare against prep's last pre-rebase run). Phase 3 — the no-op merge must be green trivially (identical tree); any failure = environment drift, not the merge. Every alpha push cuts an alpha release build — three or more builds across the phases is normal.

---

## 8. Open decisions (could not be resolved from the analysis / this environment)

1. **The held prep→alpha PR's number/existence** could not be confirmed (`gh` unavailable in the planning environment). §3.4 covers both paths (refresh existing PR / create new).
2. **Phase-1 PR merge method**: rebase-merge recommended (per-fragment commits feed git-cliff); squash acceptable. Maintainer's call.
3. **Retry-pill aria-label wording**: this runbook adopts main's #945 long form over alpha's #890 wording (marker-compliant, more informative). If the maintainer overrules, amend the F4 marker consciously.
4. **Phase 4 timing and whether the beta soak / stale rc leg is used** — maintainer's release-planning call.

---

*Runbook generated 2026-07-24 from read-only git inspection. Every line number, SHA, conflict count, and marker in this document was verified against the live repository during planning (see the companion analysis §6 for the underlying method). No branches, tags, or files other than this runbook were created.*
