# Alpha ↔ Main Drift — Deep Content-Level Analysis (2026-07-24)

**Author:** Claude (Fable 5), read-only git-content audit
**Refs analysed:** `origin/main` = `ee82063` (v1.10.1, 2026-07-06) · `origin/alpha` = `ce935dc` (v1.11.0-alpha.30, 2026-07-20) · `prep/alpha-gamdl-3.8.2-plus-2026-07-10` = `a44a6a4` (working branch, +60/−3 vs alpha) · merge-base(main, alpha) = `e0a1ee6` (2026-04-20)
**Method:** No commit-message trust. Per-commit content probes (distinctive added-line sampling, per-file corpus grep + whole-tree escalation), tree diffs from *content baselines* rather than the git merge-base, and a non-destructive `git merge-tree --write-tree` trial merge. All commands reproducible; see §6.

---

## 0. THE PIVOTAL FINDING — the "681 behind" figure is a git-ancestry illusion

Alpha commit **`674967f` "chore(alpha): realign with main (catch-up to v1.9.4) (#854)" (2026-05-22)** is a **squash import of main's entire v1.9.4 tree**. Proof (tree-level, not message-level):

```
$ git diff --stat b9857af94c 674967f       # main@v1.9.4  vs  the realign commit
 CHANGELOG.md | 7 ++++++-       (a "[Unreleased]" → "[1.9.4]" heading + 5 changelog lines)
 SECURITY.md  | 4 ++--          (supported-version table 1.9.3 → 1.9.4)
 2 files changed, 8 insertions(+), 3 deletions(-)
```

The realign tree is **byte-identical to main's v1.9.4 tree except 11 lines of changelog/security text**. Therefore *every one of main's commits up to v1.9.4 (2026-05-20) is wholesale content-present in alpha's ancestry* — regardless of what `git rev-list --count` says. Because the import was a squash (no merge parent), the merge-base stayed frozen at 2026-04-20 and git reports alpha as "681 behind" — but ~666 of those 681 commits' *content* is inside alpha's tree.

Two further content flows close most of the remainder:

1. **`42c9ec9` (#877, 2026-05-24)** forward-ported main's post-v1.9.4 substantive work: `795dd8e` (#863 rclone + multi-endpoint updater). Verified: `git grep -c rclone origin/alpha -- src-tauri/src/services/dependency_manager.rs` → 23 hits; `tool-versions.toml` → 5 hits; probe = 40/40 sampled lines present.
2. **`c853001` (#967, 2026-07-06)** ported the API-layer half of main's `134e3e8` (#947, the v1.10.1 fix bundle): `APPLE_BROWSER_USER_AGENT`, `extend=ttmlLocalizations`, `extract_syllable_ttml_from_response`, the `Origin` header (all verified present in `origin/alpha:src-tauri/src/services/apple_music_api.rs`), plus the credentials/backup hardening (probe: `commands/credentials.rs` **106/106** lines, `backup_service.rs` **35/35**, `commands/backup.rs` **6/6**).

**Corroboration:** `git cherry origin/alpha origin/main` finds **zero** patch-id-equivalent commits (0 `-`, 570 `+`) — nothing was cherry-picked verbatim; everything flowed via the three squash/port commits above. This is exactly why headline commit-counting is useless here and content probing was mandatory.

**Main's total post-v1.9.4 delta is only 39 files** (`git diff --name-only b9857af..origin/main`), from 17 non-merge commits (2 substantive: #863 ported, #947 partial; 1 CI: #905; the rest dep bumps/CHANGELOG/release stamps).

---

## 1. Question 1 — True missing-from-alpha set (of the 130 substantive main-only commits)

### 1.1 Headline result

| Verdict | Count | Detail |
| --- | --- | --- |
| **Present** (content in alpha's tree via the #854 snapshot / #877 / #967 ports) | **119** | Probe ≥ 85% for most; low scorers individually verified by symbol (§1.4) |
| **Present but superseded** (alpha deliberately evolved/removed the surface) | **9** | Cron channels (#879 removal), `--no-exceptions` capability gate, PyPI watcher generalisation, dep pins (§1.5) |
| **N/A** (version-plumbing only) | **1** | `28a8e2b` release-please manifest revert |
| **PARTIAL — genuinely missing fragments** | **1** | `134e3e8` (#947, v1.10.1 bundle) — see §1.3 |

**Not a single one of main's 130 substantive commits is *wholly* missing from alpha.** The genuine gap is a set of *fragments of one commit* (#947) plus one non-substantive `ci:` commit (#905) that falls outside the feat/fix/perf grammar but matters (§1.3b).

### 1.2 The named critical fixes — explicit confirmation (all PRESENT in alpha)

| Fix | Main commit | Present in alpha? | Evidence (in `origin/alpha` tree) | Severity if it had been missing |
| --- | --- | --- | --- | --- |
| macOS startup crash (#827/#828) | `ba73f25a` | **YES** (probe 40/40) | `src-tauri/src/lib.rs:515-520` — `blocking_lock()` + rationale comment | critical |
| `{platform}` KeyError on every download (#829) | `ba73f25a` | **YES** | `download_queue.rs:3386` — the `KeyError: 'platform'` guard comment + template strip logic | critical |
| Enrichment album-dir scoping (#842) | `7cb83ee8` | **YES** (probe 35/35) | `download_queue.rs` — verbatim `**#842 (artist-URL enrichment scope).**` block (grep-verified 3/3 sampled lines) | high |
| Companion lyrics single-run (#843) | `f0ef6102` | **YES** (probe 37/40) | `run_companion_lyrics_conversion` called from exactly one site (`download_queue.rs:6560`) | high |
| Companion folder merge + parallel enrichment (#786/#528/#779) | `845a3cba` | **YES** (probe 40/40) | `services/legacy_folder_merge.rs` exists; `mod.rs:616` | high |
| Companion sidecar rename + per-item progress (#791/#788/#790) | `c5e8d7c1` | **YES** (probe 40/40) | `metadata_tag_service.rs:1472` sidecar-pass-skip on rename failure | high |
| Strictly-serial post-processing, ActiveSlotGuard (#706) | `96d52021` | **YES** (probe 40/40) | 13 `ActiveSlotGuard` refs in alpha's `download_queue.rs` (main has 10 — alpha *extended* it) | critical |
| Companion timeout × tier count (#705) | `39711f2e` | **YES — refactor-evolved** | Both branches replaced it with the same `compute_total_timeout` (alpha `download_queue.rs:1041`, `PER_TIER_SECS = 8*60` at :1056, tests at :12844-12982 mirror main's :11934-12072). Low probe (22/40) is a both-sides-refactored-identically artifact | high |
| Cooperative-cancel companion task (#663) | `67ac4faf` | **YES** | `CompanionTaskHandle` 7 refs + `aborted: Arc<AtomicBool>` in alpha = main exactly. Low probe (21/40) = comment churn | high |
| Terminal-state revival guards (#661) | `3fac521a` | **YES** | All four guard tests present by name (`set_complete_does_not_revive_errored_item`, `set_error_does_not_overwrite_cancelled_item`, …) | critical |
| Watchdog activity-count fixes (#846/#851) | `b7019973`, `2f87df38` | **YES** (40/40, 34/34) | `evict_activity_counter` sites `download_queue.rs:1962,1971,2076` | high |
| Queue freeze recovery, v1.8 bundle (#819) | `ea48540c` | **YES** (probe 40/40) | idle-watchdog machinery `download_queue.rs:1037,3784,6486` | critical |
| GAMDL spawn supervisor safety nets | `321bc29b` | **YES** (probe 39/40) | supervisor + idle watchdog present (see #819 rows) | high |
| Playlist-title KeyError classifier (#588) | `6db9d02c` | **YES** (probe 40/40) | classifier bucket in `utils/process.rs` | med |
| False-complete guard / #661-adjacent messaging | `3fac521a` + Phase 3.5h (pre-1.9.4) | **YES** | snapshot proof §0 | high |

### 1.3 The ONE partial commit — `134e3e8c` (#947, "bundled audit hardening + #935 syllable lyrics + #937 release pipeline + repo hygiene", 2026-06-19, the v1.10.1 substance)

Per-file distinctive-added-line presence in `origin/alpha` (and re-checked against `prep` HEAD — identical verdicts unless noted):

| File | Present | Verdict |
| --- | --- | --- |
| `src-tauri/src/commands/credentials.rs` | 106/106 | ✅ ported (via #967) |
| `src-tauri/src/services/backup_service.rs` | 35/35 | ✅ ported |
| `src-tauri/src/commands/backup.rs` | 6/6 | ✅ ported |
| `src-tauri/src/services/apple_music_api.rs` | 51/109 | ✅ **functionally ported** — all 5 key #935 mechanisms present (`APPLE_BROWSER_USER_AGENT`, `?extend=ttmlLocalizations`, `Origin` header, `extract_syllable_ttml_from_response`, 15s timeout); the missing lines are doc-comments + UA applied to fewer sites (prep centralised headers via `apple_music_headers`, #970) |
| `src-tauri/src/services/download_queue.rs` | 5/66 | ❌ **MISSING: #935 quick-win A** (syllable-lyrics outcome summary — `no_lyrics_available`/`errored` counters + 4-way user-facing summary incl. "keeping line-level lyrics") **and #942** (MV-companion token/relation failures surfaced via `emit_download_log`/`emit_download_warn` instead of silent `log::debug!`). Grep for `no_lyrics_available`, `MusicKit credentials required (Settings > Quality`, `keeping line-level lyrics` → **0 hits on alpha AND prep** |
| `src-tauri/src/services/musicbrainz_service.rs` | 3/18 | ❌ **MISSING**: Tier 1/2/3 lookup progress promoted to activity-log (`emit_download_log`/`emit_verbose_download_log` + `app`/`download_id` params). Alpha still has the pre-#947 `log::debug!` form (`musicbrainz_service.rs:316-334`) |
| `src/components/download/QueueItem.tsx` | 0/13 | ❌ **MISSING: #945 a11y** — three long-form `aria-label`s (retry-without-wrapper / open-file / reveal-folder). 0 hits on alpha and prep (alpha's #911 rewrite of this file did not carry them) |
| `src/components/settings/SettingsPage.tsx` | 0/2 | ❌ **MISSING**: tab renames "Quality"→"Codec & Resolution", "Fallback"→"Codec Fallback Order" (#946 hygiene) |
| `.editorconfig` | 0/28 | ❌ **MISSING** — file does not exist on alpha or prep |
| `.github/workflows/release.yml` | 0/19 | ❌ **MISSING: #944 concurrency guard** (`concurrency: group: release-${{ inputs.tag \|\| github.ref }}` + `cancel-in-progress: false`) — 0 hits on alpha and prep. Alpha/prep releases remain exposed to the dispatch-race half-built-release failure mode |
| `.github/workflows/{alpha,beta,release-candidate}-release.yml` | 2/23 etc. | ✅ **converged** — the #937 self-trigger fix is the same subject-prefix job-level `if:` guard alpha invented *earlier and stricter* in #906 (`alpha-release.yml:82` requires `chore(alpha): ` **and** `-alpha.`); only comments differ |
| `src-tauri/tool-versions.toml` | 1/38 | ✅ **superseded** — main's delta = 3.7.2/3.7.3 audit comments + ceiling 3.7.3; alpha/prep are at min 3.0 / ceiling 3.8.x with richer audit notes |
| `README.md` | 0/2 | ❌ MISSING: "Reference documentation" link to the Apple Music TTML spec in MeedyaSuite-core |
| `.gitignore` | 0/1 | ❌ MISSING: `.debugLogs/*`, `.examplefiles/*` entries |
| `package.json` / `package-lock.json` | 2/2, 3/3 | ✅ (dep churn superseded by alpha's newer pins) |
| `.claude/agents/*.md` | 6/6, 6/6 | ✅ present |

### 1.3b Genuinely missing NON-substantive commits (outside the 130 but material)

| Commit | What | Missing pieces (verified 0-hits/absent on alpha AND prep) | Severity |
| --- | --- | --- | --- |
| `06369e1` (#905, `ci:`, 2026-06-03) | PR security heuristics workflow + cross-source audit checks | `.github/workflows/pr-security.yml`, `.github/pull_request_template.md`, `tools/audit-checks/{README.md,check_codec_registry.py,check_ipc_commands.py}`, `.claude/memory/project_pr_security_checks.md` — all MISSING as files | **Medium** (CI security process gap on the branch where all development now happens) |
| `1c36577` (#948 release 1.10.1) + `ba68bc1` | Release records | Alpha's `CHANGELOG.md` has **no `[1.10.1]` section**; alpha's `SECURITY.md` supported-version table says **1.10.0** (main: 1.10.1) | Low (records; SECURITY self-heals via `update-security-policy.yml` on next main push, but the CHANGELOG section must be spliced or regenerated) |
| `f3da955` (#968 dependabot routing) | `.github/dependabot.yml` | ✅ **present** — `git diff origin/main origin/alpha -- .github/dependabot.yml` is empty | n/a |

### 1.4 Why some pre-v1.9.4 commits probed low (all verified present — probe artifacts)

Every substantive commit scoring <90% is dated ≤ 2026-05-10, i.e. inside the proven v1.9.4 snapshot (§0). Spot verification of each low scorer's *current* functional presence in alpha:

| Commit (probe %) | Verified-present evidence in `origin/alpha` |
| --- | --- |
| `c6214491` #549 (10%) | `commands/gamdl.rs:381` — "Unrecognised Apple Music URL shape (#549)" (reworded post-snapshot → line mismatch) |
| `1162fcb7`/`48ac5f3b` #548 (11/38%) | 12 files hit `itunes.apple.com`; legacy-URL rewrite at `apple_music_api.rs:1939` (both branches later evolved #548 into #568's rewrite) |
| `bfce974d` #546 (13%) | 7 files hit "library URL" diagnostics |
| `db6244b5` #547 (25%) | 8 files hit `classical.apple.com` |
| `7d50ee8d` `/recording/` (48%) | `url-parser.ts:85-87` — the #573 revert commentary (main itself reverted; alpha tracks the final state) |
| `b4bf8cd0` #545 (65%) | `playlist_id` in 7 files incl. template defaults + migration |
| `de89c1ac` #574 (48%) | `queue-updated` emission in 8 files |
| `e1e85f67` dup-`fi` (n/a — deletion-only) | `git diff origin/main origin/alpha -- .github/workflows/update-security-policy.yml` → **empty** (identical) |
| `f73199a5` #645, `b2501c32` #741, `f32356f8` clippy (5–60%) | workflow/lint content in snapshot; later reshaped on both sides — see §5 workflows |
| `611afc99` lint #843 (50%) | cosmetic collapse; the functional #843 fix verified in §1.2 |
| `5f114d5d` #576, `1186d98a` #567/#579 (72/75%) | snapshot; progress/timeout code since refactored by both sides identically (`compute_total_timeout`, §1.2) |

### 1.5 Deliberate supersessions (present-then-replaced — a merge must NOT "restore" these)

| Main commit | Alpha's superseding state |
| --- | --- |
| `4419efb8` nightly channel, `a668e97c` weekly+monthly, `5d80f8d0` seven-tier ladder (cron tiers), `38c0bfc2` self-trigger `[skip ci]` fix | **#879 removed the cron channels** (`3a46fe0`, 2026-05-24): `nightly/weekly/monthly-release.yml` + `.github/rulesets/protected-cron-channels.json` deleted; `#906` replaced `[skip ci]` recursion break with the stricter job-level `if:` guard |
| `dbd04b7f` weekly PyPI GAMDL watcher | Generalised into `upstream-engine-watch.yml` (header: "generalised successor to upstream-gamdl-watch.yml, which was retired in PR M9-1 (#101)") |
| `6591abf1` + `b3217f47` `--no-exceptions` emission | `GamdlFeature::NoExceptionsFlag` capability gate (18 refs in alpha's `gamdl_capabilities.rs`; three-era predicate refined further on prep) |
| `1d9ccb5f` tauri dep pin | newer pins via #1007/#1015/#1035 |

**Appendix A (§7) lists all 130 commits row-by-row.**

---

## 2. Question 2 — File-level divergence classification (the 442-file diff)

Buckets computed against the **content baselines** (main side: `b9857af`=v1.9.4 → `origin/main`; alpha side: `674967f`=realign → `origin/alpha`), because the git merge-base (April) predates the content sync and would classify everything as "both-changed":

| Bucket | Count | Meaning |
| --- | --- | --- |
| **A — main-only work missing from alpha** | **8** | `.editorconfig`, `.github/pull_request_template.md`, `.github/workflows/pr-security.yml`, `tools/audit-checks/{README.md,check_codec_registry.py,check_ipc_commands.py}`, `.claude/memory/project_pr_security_checks.md`, `src-tauri/src/services/musicbrainz_service.rs` |
| **B — alpha-only work missing from main** | **401** (+10 rename shadows = 411) | See composition below |
| **C — both-diverged (true conflict surface)** | **23** | Listed below |
| **D — rename shadows** | **10** | `assets/brand/logotype*` + `public/logotype.svg` — alpha's #925 brand refresh renamed `assets/brand/ → assets/brand-old/` (+ new brand set), so `--name-only` shows the post-image path only; conceptually bucket B |

**Bucket B composition** (alpha's unique work, by area): 152 brand/asset files (`assets/brand` 89 + `assets/brand-old` 63), 58 `src-tauri/icons`, 51 Rust sources, 45 `public/`, 24 React components, 8 workflows, 8 help docs, plus stores/lib/styles/scripts/audits. The Rust inventory confirms every claimed alpha feature block as real files: **M9 Spotify** (`spotify_service.rs`, `votify_options.rs`, `votify_capabilities.rs`, `spotify_anti_ban.rs` ×3, `SpotifyConsentModal.tsx`), **SQLite download index** (`services/download_index/{mod,ingest,queries}.rs`, `schema_v1.sql`, `schema_v2.sql`), **Profile Bundle** (`commands/profile_bundle.rs`, `services/profile_bundle/{mod,credentials}.rs`), **Lyricsfile** (`lyricsfile_service.rs`), **service dispatch + M8/M10 scaffolds** (`service_dispatch.rs`, `bbc_iplayer_service.rs`, `youtube_service.rs`), **#911 UI** (`StatusPill.tsx`, `RiskPill.tsx`, `QueueListVirtualized.tsx`, …), **diagnostic bundle**, **enrichment gaps**, **integrity scan**, **best cover art**. Volume: `git diff --shortstat origin/main origin/alpha -- src-tauri/src src/` → **100 files, +19,267 / −1,629**. Prep adds another **101 files, +7,592 / −962** on top (GAMDL 3.8.2–3.8.4, #1017 python detect, #1034 security F1–F11, #1029 wrapper sign-in, #1019–#1026 mitigations, release-notes machinery #1027/#1028).

**Bucket C — the 23 true-conflict files, grouped, with per-side post-baseline change volume (hunks, ±lines):**

| Subsystem | File | main-side (v1.9.4→main) | alpha-side (realign→alpha) | Risk |
| --- | --- | --- | --- | --- |
| Rust services | `services/download_queue.rs` | 10 hunks (+101/−12) — the MISSING #942 + #935-A | 84 hunks (+1486/−95) | **HIGH** (but main's hunks are the port targets) |
| Rust services | `services/apple_music_api.rs` | 22 hunks (+192/−26) — mostly already-ported #935 | 30 hunks (+646/−37) | MED (most main hunks already present → resolve to alpha) |
| TS components | `components/download/QueueItem.tsx` | 3 hunks (+13) — #945 aria-labels | 44 hunks (+338/−205) — #911 rewrite | MED (re-apply labels by hand into the rewritten JSX) |
| TS components | `components/settings/SettingsPage.tsx` | 1 hunk (+2/−2) — tab renames | 4 hunks (+8) | LOW |
| Workflows | `release.yml` | 1 hunk (+24) — #944 concurrency | 9 hunks (+301/−19) | MED (insert the concurrency block into alpha's heavily-evolved file) |
| Workflows | `alpha/beta/rc-release.yml` | 2 hunks each — #937 guard (converged) | 4 hunks each | LOW (keep alpha; guards already equivalent-or-stronger) |
| Version stamps | `package.json`, `.release-please-manifest.json`, `Cargo.toml`, `tauri.conf.json` | version 1.10.1 | 1.11.0-alpha.30 / manifest 1.11.0 | **HIGH-mechanical** (always keep alpha's; see §5) |
| Lockfiles | `Cargo.lock` (295 vs 513 hunks), `package-lock.json` (253 vs 272) | dep bumps ≤ 2026-06-15 | dep bumps ≤ 2026-07-20 | LOW (never hand-merge; regenerate) |
| Docs | `CHANGELOG.md` | +170 (the [1.10.1] + [1.10.0] sections) | +8281 (alpha line) | MED (must splice main's [1.10.1]/[1.10.0] sections in; alpha lacks them) |
| Docs | `README.md`, `SECURITY.md`, `Project_Plan.md`, `.claude/CLAUDE.md`, `.claude/memory/MEMORY.md`, `.gitignore`, `deny.toml`, `tool-versions.toml` | small | small-to-large | LOW (union or keep-alpha + splice) |

(For **prep** the C-bucket grows to 25 — adds `commands/credentials.rs` and `services/dependency_manager.rs`, both touched by prep's #1034 security work vs #947's hardening — both sides' hardening must union.)

**Empirical merge simulation** (`git merge-tree --write-tree origin/alpha origin/main`, read-only): **69 conflicted files, 514 conflict hunks** — of which 235 are lockfiles (`Cargo.lock` 160 + `package-lock.json` 75), leaving **~279 real hunks across 67 files**. Top: `download_queue.rs` 38, `gamdl_capabilities.rs` 21, `apple_music_api.rs` 16, `smart_retry_planner.rs` 13, `activity_log_writer.rs` 11, `QueueItem.tsx` 10. The 69 exceeds bucket-C's 23 because the *ancient merge-base* makes git compare April-era text: everywhere alpha's post-realign evolution touched a region main changed April–May, git sees a three-way disagreement even though alpha's side *contains* main's change plus more. **The resolution rule is therefore near-uniform: take alpha's side everywhere except the §1.3 fragments** — which is precisely what makes a full merge high-ceremony but low-information.

---

## 3. Question 3 — Reconciliation recommendation

### Option A — true merge `main → alpha`

* **Conflict volume (measured, not estimated):** 69 files / ~279 non-lockfile hunks + 2 lockfiles to regenerate.
* **Resolution entropy:** ~95% of hunks resolve "keep alpha" (alpha ⊇ main's content + evolution). The genuinely-new main content (§1.3) sits inside a handful of those hunks — easy to lose in the noise of 279 keep-alpha resolutions. **Reviewer fatigue is the #1 regression risk.**
* **Silent-resurrection landmines (verified in the simulated merged tree `d116419`):** the merge **silently re-adds** `nightly-release.yml`, `weekly-release.yml`, `monthly-release.yml`, `upstream-gamdl-watch.yml`, and `.github/rulesets/protected-cron-channels.json` — all *deliberately deleted* by alpha (#879, M9-1). These are add/add-on-one-side cases (created on main after the fork, deleted on alpha after the realign), so git does **not** flag them as conflicts. Re-activated cron workflows would start cutting nightly releases again and the resurrected gamdl-watcher would double-file upstream issues. The 3 old `assets/logo/*.svg` (modify-on-main/delete-on-alpha) at least surface as conflicts.
* **Ancestry benefit:** records main as merged; merge-base jumps to `ee82063`; the misleading "681 behind" disappears; every future `main↔alpha` operation becomes cheap and honest. This is the one thing only a merge can buy.

### Option B — cherry-pick only the truly-missing commits

* **Volume (simulated with `git merge-tree --merge-base=<sha>^`):** cherry-pick `134e3e8` (#947) onto alpha → **6 conflicted files** (3 channel workflows, `apple_music_api.rs`, `tool-versions.toml`, `QueueItem.tsx`) — all resolve "keep alpha + take main's genuinely-new lines"; cherry-pick `06369e1` (#905) → **1 trivial conflict** (`MEMORY.md` index). Nothing else to pick.
* Even better than raw cherry-picks: a **hand-curated port commit** taking only the §1.3 missing fragments (avoids re-dragging tool-versions/workflow hunks that are superseded).
* **Risk of lost work:** near zero for content. But it leaves ancestry broken: merge-base stays 2026-04-20, git keeps lying ("681 behind" grows daily), every future tool/human repeats this whole forensic exercise, and the eventual alpha→main promotion merge inherits the same 69-file false-conflict surface *plus* whatever accrues.

### RECOMMENDATION — **B then A: port the fragments first, then close ancestry with a no-op merge** (phased)

The content gap is tiny and surgically portable; the expensive thing is ancestry. Do both, in the order that makes each trivial:

1. **Phase 1 (content, on `prep` or a short branch off `alpha`):** one or two conventional commits porting the §1.3 + §1.3b fragments: (a) #942 + #935-A in `download_queue.rs`; (b) MusicBrainz activity-log tiers; (c) #945 aria-labels into the #911 `QueueItem.tsx`; (d) SettingsPage tab renames; (e) `release.yml` #944 concurrency block (adapted to alpha's evolved file); (f) `.editorconfig`, `.gitignore` entries, README TTML link; (g) `pr-security.yml` + `tools/audit-checks/` + PR template (#905); (h) splice CHANGELOG `[1.10.1]`/`[1.10.0]` sections + SECURITY 1.10.1 row. Run `cargo test --lib` + `npm run type-check` + vitest.
2. **Phase 2 (land the prep line):** merge the held prep→alpha PR (**rebase-merge**, per the #1027 release-notes requirement — note a rebase-merge PR *cannot contain merge commits*, which is why the ancestry merge must come AFTER this, not ride inside it).
3. **Phase 3 (ancestry closure, directly on `alpha` or via a merge-commit PR):** `git merge origin/main` resolved **entirely to the already-reconciled alpha tree** (verify with `git diff <merge-result> alpha-pre-merge-tree` → expect empty except any deliberate doc unions; a `-s ours` merge is acceptable *only after* Phase 1 has landed, because §1 proves the remaining main-side delta is then fully represented). Explicitly re-delete the five resurrection files if a normal merge strategy is used. Tag `backup/alpha-pre-ancestry-merge` first. After this, `git rev-list --count origin/alpha..origin/main` → ~0 and `realign-alpha`-class accidents lose their teeth.
4. **Phase 4 (promotion flow):** alpha → beta → main at the next stable cut carries alpha's 52+ unique commits (M9 Spotify, #911 UI, Profile Bundle, Lyricsfile, SQLite index, GAMDL 3.6–3.8.4 line, brand refresh, prep's security/audit work). Note beta is currently a strict ancestor of main (0 ahead, 26 behind, parked at v1.9.4) so the promotion is a fast-forward-shaped merge on the beta leg.

Estimated effort: Phase 1 ≈ a focused half-day incl. tests; Phase 3 ≈ an hour of mechanical verification. Versus Option A alone: ~279 hunks of keep-alpha resolutions with five silent landmines and one reviewer's attention as the only safety net.

---

## 4. Question 4 — "No work lost" guarantee

### What could be silently lost, per strategy, and the countermeasure

| Asset at risk | Lost under… | Prevention |
| --- | --- | --- |
| **Alpha's 52 unique commits** (M9 Spotify, #911, Profile Bundle, Lyricsfile, SQLite index, GAMDL 3.6+ gates, brand refresh, #879 removals, #967) | `realign-alpha` workflow (fast-forward reset) or any `reset --hard origin/main` | **DO NOT run realign-alpha** (already flagged in `.claude/memory/project_alpha_main_drift.md`); protect via `backup/alpha-*` tag before Phase 3; branch ruleset already blocks force-push |
| **Prep's 60 commits** (security F1–F11, wrapper sign-in, 3.8.2–3.8.4, mitigations, #1027/#1028) | prep→alpha squash-merge (release-note grouping lost) or abandoning the branch | rebase-merge the PR (per #1027); prep is pushed to origin — already durable |
| **Alpha.30's 3 commits absent from prep** (`3f7b241` npm bumps #1035, `d0dc628` serde_json #1036, `ce935dc` version alpha.30) | force-pushing prep over alpha, or a prep rebase that drops them | prep→alpha via PR merge (keeps both); if prep is rebased again, re-verify `git rev-list prep..origin/alpha` is empty afterwards |
| **Alpha's deliberate deletions** (#879 cron workflows + ruleset, retired gamdl-watch, old logos) | **Option A merge — SILENTLY resurrected** (verified in simulated tree) | Post-merge checklist: `git ls-tree HEAD .github/workflows/` must NOT contain nightly/weekly/monthly/upstream-gamdl-watch; re-delete before committing the merge |
| **Main's genuinely-new fragments** (§1.3: #942, #935-A, #944, #945, #946, MusicBrainz logs, .editorconfig, #905 suite) | Option A resolved "keep alpha" wholesale; Option B if the curator misses a fragment | This report *is* the checklist — grep-verifiable markers per fragment (e.g. `no_lyrics_available`, `group: release-`, `aria-label="Retry without wrapper`, `pr-security.yml`) |
| **Main's release records** (CHANGELOG `[1.10.1]`/`[1.10.0]` sections, SECURITY 1.10.1) | both options (CHANGELOG take-alpha loses them) | Splice sections in Phase 1; SECURITY also self-heals via workflow |
| **Main's release-please state** (`.release-please-manifest.json` = 1.10.1) | naive take-main on alpha (would make release-please propose wrong versions from alpha) — and conversely alpha's 1.11.0 manifest must NOT reach main until the stable cut | keep per-branch values; at promotion time the release-please PR recomputes |
| **Housekeeping-only main commits** (276 docs / 126 chore) | Option B by design | Acceptable by design: they are records of main's own release mechanics; the CHANGELOG/SECURITY splice covers the user-visible part |

### Verification battery (run after any reconciliation step)

```bash
git grep -c no_lyrics_available <ref> -- src-tauri/src/services/download_queue.rs      # expect ≥1
git grep -c 'group: release-'   <ref> -- .github/workflows/release.yml                  # expect 1
git grep -c 'Retry without wrapper (uses cookie-based' <ref> -- src/components/download/QueueItem.tsx  # ≥1
git cat-file -e <ref>:.github/workflows/pr-security.yml && git cat-file -e <ref>:.editorconfig
git ls-tree --name-only <ref> .github/workflows/ | grep -cE 'nightly|weekly|monthly|upstream-gamdl'    # expect 0
git grep -c '\[1.10.1\]' <ref> -- CHANGELOG.md                                          # expect ≥1
cd src-tauri && cargo test --lib && cd .. && npm run type-check && npm run test
```

---

## 5. Question 5 — Version/release-plumbing landmines (concrete)

| Surface | main | alpha | prep | Rule |
| --- | --- | --- | --- | --- |
| `package.json` / `tauri.conf.json` / `Cargo.toml` version | 1.10.1 | 1.11.0-alpha.30 | 1.12.0-alpha.28 | Always the target branch's own stamp. **Prep vs alpha note:** prep says 1.12.0 while alpha.30 says 1.11.0 — the next alpha auto-bump computes `1.12.0-alpha.31`+ from manifest+tags after the prep PR lands (handoff verified this); do not "fix" mid-merge |
| `.release-please-manifest.json` | 1.10.1 | 1.11.0 | 1.11.0 | Keep 1.10.1 on main until the stable cut; keep 1.11.0 on alpha/prep. Never let a merge move either |
| Lockfiles (`Cargo.lock`, `package-lock.json`) | ≤ 2026-06-15 pins | ≤ 2026-07-20 pins | prep variant | Never hand-merge: take alpha's, then `cargo update -p meedyadl --precise <ver>` / `npm install --package-lock-only` to re-sync version fields |
| `.github/workflows/` file **set** | has `nightly/weekly/monthly-release.yml`, `upstream-gamdl-watch.yml`, `pr-security.yml` | has `lint.yml`, `upstream-engine-watch.yml`; cron files deleted (#879) | + release-notes gates | Merge must ADD `pr-security.yml` only; must NOT resurrect the 4 cron/watch files or `.github/rulesets/protected-cron-channels.json` (all silently re-added in the simulated merge tree — verified) |
| `release.yml` | + #944 concurrency (24 lines) | +301 lines of alpha evolution (and prep adds #977/#988/#1027/#1028) | more | Port the concurrency block by hand into alpha's shape (group key + `cancel-in-progress: false` semantics unchanged) |
| `alpha/beta/rc-release.yml` | #937 subject-prefix guard | #906 stricter guard (predates main's) | same | Keep alpha; no action |
| `tool-versions.toml` | gamdl min 2.9.1 / ceiling 3.7.3 + audit comments | min 3.0 / 3.8.x + votify sections | ceiling 3.8.4 | Keep alpha/prep wholesale (main's is strictly older); merge conflict is guaranteed here (3 main-side vs 4 alpha-side hunks) — resolve take-alpha |
| `CHANGELOG.md` | has `[1.10.0]`, `[1.10.1]` | has neither; +8,281 alpha lines | same | Splice main's two release sections into alpha's file (or let `changelog.yml` git-cliff regeneration handle it after ancestry closure — verify the cliff.toml skip rules keep it clean) |
| `SECURITY.md` supported table | 1.10.1 | 1.10.0 | 1.10.0 | Take main's row; self-heals via `update-security-policy.yml` on the next main push anyway |
| `deny.toml` | +17/−3 (#947-era) | +35/−23 (alpha) | prep more | Union by hand; then `cargo-deny check licenses` |
| `.claude/CLAUDE.md` / `MEMORY.md` / `Project_Plan.md` / `README.md` | small main deltas | large alpha deltas | larger | Keep alpha/prep; port main's TTML-spec README link (§1.3) |

---

## 6. Reproduction — key commands run (all read-only)

```bash
git rev-list --count origin/alpha..origin/main                      # 681 (570 non-merge; 130 feat/fix/perf)
git cherry origin/alpha origin/main                                  # 0 patch-id-equivalent
git diff --stat b9857af94c 674967f                                   # realign ≡ v1.9.4 (2 files, 11 lines)
git log --no-merges b9857af94c..origin/main                          # the TRUE candidate window (17 commits)
# per-commit content probe: sample ≤40 distinctive added lines, grep alpha's versions of touched
# files, escalate ≤8 misses to whole-tree `git grep -F <line> origin/alpha` (script in session scratchpad)
git diff --name-only origin/alpha origin/main                        # 442 files
comm -12/-23/-13 <(diffset) <(main-since-v1.9.4) <(alpha-since-realign)   # buckets A/B/C/D
git merge-tree --write-tree --name-only origin/alpha origin/main     # 69 conflict files; tree d116419…
git grep -c '^<<<<<<<' d116419…                                      # 514 hunks (235 lockfile)
git merge-tree --write-tree --merge-base=134e3e8c^ origin/alpha 134e3e8c   # cherry-pick sim: 6 files
git ls-tree d116419… .github/workflows/                              # resurrection proof
```

---

## 7. Appendix A — all 130 substantive main-only commits (probe % = distinctive added-line presence in `origin/alpha`)

Legend: **No (present)** = content verified in alpha (snapshot §0 + probe; low % individually re-verified in §1.4). **No (superseded)** = present-then-deliberately-replaced (§1.5). **PARTIAL** = §1.3. **N/A** = version plumbing.

| Commit | Date | Subject | Probe | Verdict |
| --- | --- | --- | --- | --- |
| `4ed3f126` | 2026-04-20 | feat: companion-download resilience — soft errors, watchdog, scoping, audioTraits gate | 87% | No (present) — content in #854 v1.9.4 snapshot; probe 87% |
| `8ae02305` | 2026-04-20 | feat: expose gamdl_idle_timeout_minutes in Settings > Advanced | 92% | No (present) — content in #854 v1.9.4 snapshot; probe 92% |
| `4419efb8` | 2026-04-20 | feat: nightly release channel with channel-aware update guard | 35% | No (superseded) — superseded — nightly workflow deliberately removed on alpha (#879); channel-aware update guard retained |
| `321bc29b` | 2026-04-20 | feat: wrap primary GAMDL spawn in supervisor safety nets | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `42a20374` | 2026-04-20 | fix(clippy): allow too_many_arguments on spawn_companion_downloads | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `2e0838fd` | 2026-04-21 | feat(activity-log): emit dedup settings in startup summary | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `dbd04b7f` | 2026-04-21 | feat(ci): weekly PyPI watcher that tickets GAMDL releases above our tested ceiling | 0% | No (superseded) — superseded — generalised into upstream-engine-watch.yml on alpha (M9-1, retires upstream-gamdl-watch.yml) |
| `f3ac53b4` | 2026-04-21 | feat(dedup): cross-URL batch deduplication (#513) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `07a1a6d4` | 2026-04-21 | feat(dedup): pre-queue track-level duplicate detection for artist URLs (#510) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `130cecf4` | 2026-04-21 | feat(dedup): skip album tracks already in queue or history (#514) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `4c51df5a` | 2026-04-21 | feat(dedup): skip playlist tracks that overlap queue or history (#512) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `daed484a` | 2026-04-21 | feat(gamdl): compile-time version support window with pinned installer + gated upgrade prompts | 92% | No (present) — content in #854 v1.9.4 snapshot; probe 92% |
| `b3217f47` | 2026-04-21 | feat(gamdl): emit --no-exceptions by default to clean up v3.0 mixed stderr | 74% | No (superseded) — superseded — same NoExceptionsFlag gate governs emission |
| `6383bcb8` | 2026-04-21 | feat(gamdl): version-aware CLI/INI dispatch for GAMDL v2.9.1 — v3.x | 92% | No (present) — content in #854 v1.9.4 snapshot; probe 92% |
| `92a31f0e` | 2026-04-21 | fix(dedup): appease clippy::question_mark lint | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `2017ab9e` | 2026-04-22 | feat(activity-log): persistent on-disk activity log for bug hunting (#541) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `722f82cd` | 2026-04-22 | feat(filename-safety): engine filename-safety contract (#551) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `db6244b5` | 2026-04-22 | feat(logging): trace Apple Music Classical URL submissions (#547) | 25% | No (present) — content in #854 v1.9.4 snapshot; probe 25% |
| `bfce974d` | 2026-04-22 | feat(logging): trace Apple Music library URL submissions (#546) | 13% | No (present) — content in #854 v1.9.4 snapshot; probe 13% |
| `1162fcb7` | 2026-04-22 | feat(logging): warn on legacy itunes.apple.com URL submissions (#548) | 11% | No (present) — content in #854 v1.9.4 snapshot; probe 11% |
| `c6214491` | 2026-04-22 | feat(logging): warn on unrecognised Apple Music URL shapes (#549) | 9% | No (present) — content in #854 v1.9.4 snapshot; probe 9% |
| `a4bdaa01` | 2026-04-22 | fix(compilation): add {album_id} to default template + extend v3→v4 migration (#552) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `b70e200c` | 2026-04-22 | fix(download): MV filename uniqueness + motion-art renaming pass (#527 #536 #537) | 95% | No (present) — content in #854 v1.9.4 snapshot; probe 95% |
| `2fff25ea` | 2026-04-22 | fix(download): force MV-safe no-album templates + heal legacy defaults (#531) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `fcf75f2a` | 2026-04-22 | fix(filename-safety): scope HashSet import to tests module | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `6d3255e2` | 2026-04-22 | fix(fs-safe): content-aware dedup for API JSON dumps (#553, supersedes #492) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `fc329d54` | 2026-04-22 | fix(lyrics): rename sidecars alongside codec-suffixed audio (#535) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `48ac5f3b` | 2026-04-22 | fix(parser): accept itunes.apple.com in parse_apple_music_url (#548) | 37% | No (present) — content in #854 v1.9.4 snapshot; probe 37% |
| `b4bf8cd0` | 2026-04-22 | fix(playlist): add {playlist_id} to default template + settings migration (#545) | 65% | No (present) — content in #854 v1.9.4 snapshot; probe 65% |
| `0a6c5841` | 2026-04-23 | feat(filename-safety): engine-contract trait scaffold (#551) | 5% | No (present) — content in #854 v1.9.4 snapshot; probe 5% |
| `1aabfda8` | 2026-04-23 | feat(naming): user-configurable disc + track number padding (#587) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `7d50ee8d` | 2026-04-23 | feat(parser): recognise Apple Music Classical `/recording/` URLs with helpful error | 47% | No (present) — content in #854 v1.9.4 snapshot; probe 47% |
| `5f114d5d` | 2026-04-23 | feat(progress-bar): intra-Processing progress fraction (#576) | 72% | No (present) — content in #854 v1.9.4 snapshot; probe 72% |
| `16e0cce1` | 2026-04-23 | feat(ux): add "Open folder" button alongside Browse in Diagnostics (#581) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `260edf75` | 2026-04-23 | fix(activity-log): stable key + stable measureElement to prevent row overlap (#575) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f32356f8` | 2026-04-23 | fix(ci): clippy doc_lazy_continuation + verifier check ordering | 9% | No (present) — content in #854 v1.9.4 snapshot; probe 9% |
| `1186d98a` | 2026-04-23 | fix(enrichment): skip all enrichments on empty output + scale timeout by track count (#567 #579) | 75% | No (present) — content in #854 v1.9.4 snapshot; probe 75% |
| `10229b99` | 2026-04-23 | fix(enrichment): skip macOS AppleDouble + known filesystem sidecars in audio walkers (#577) | 87% | No (present) — content in #854 v1.9.4 snapshot; probe 87% |
| `6db9d02c` | 2026-04-23 | fix(errors): classify GAMDL playlist-title KeyError with actionable guidance (#588) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `1ce945c6` | 2026-04-23 | fix(parser): accept /recording/ URLs as submittable (revert #573 rejection UX) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f17504a5` | 2026-04-23 | fix(parser): accept classical.music.apple.com + slug-less Share URLs | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `176f7949` | 2026-04-23 | fix(parser): capture GAMDL v3.0 bracketed Track/URL error lines (#521) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `de89c1ac` | 2026-04-23 | fix(progress-bar): emit queue-updated event on enrichment label changes (#574) | 48% | No (present) — content in #854 v1.9.4 snapshot; probe 48% |
| `3676c047` | 2026-04-23 | fix(settings): make Settings panel fill horizontal space + wrap long checkbox labels | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `16dfc71d` | 2026-04-23 | fix(ux): replace 'Pre-flight checks passed' with plain-English activity-log messages (#578) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `e3c03381` | 2026-04-24 | feat(gamdl): add wrapper_m3u8_ip CLI/INI/UI support for GAMDL v3.1 (#605) | 80% | No (present) — content in #854 v1.9.4 snapshot; probe 80% |
| `a8efb074` | 2026-04-24 | feat(gamdl): wire --playlist-folder-template (GAMDL v3.0+, #618) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `0438af29` | 2026-04-24 | feat(queue): abort-all UX polish — shortcut, status-bar, don't-ask, suppression (#620) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `090fe116` | 2026-04-24 | feat(queue): abort-all destructive action (#620) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `8ceabbcc` | 2026-04-24 | feat(settings): Settings UI for playlist_folder_template (#618) | 84% | No (present) — content in #854 v1.9.4 snapshot; probe 84% |
| `92a8f720` | 2026-04-24 | feat(ux): surface GAMDL v3.1 track counter + suppress 1-of-1 (#609) | 90% | No (present) — content in #854 v1.9.4 snapshot; probe 90% |
| `34bb4ea0` | 2026-04-24 | fix(config): drop vestigial song_codec / song_codec_priority INI keys (#617) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `4ca228fc` | 2026-04-24 | fix(gamdl): always emit --song-codec-priority, never --song-codec (#614) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `6591abf1` | 2026-04-24 | fix(gamdl): stop emitting --no-exceptions on GAMDL v3.1 (#606) | 15% | No (superseded) — superseded — GamdlFeature::NoExceptionsFlag capability gate on alpha (18 refs in gamdl_capabilities.rs) |
| `bf761180` | 2026-04-24 | fix(parser): handle GAMDL v3.1 ExceptionPrettyPrinter output ordering (#607) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `cbf85cc3` | 2026-04-24 | fix(parser): use strip_prefix in is_structlog_line_start (clippy::manual_strip) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `850fbf6a` | 2026-04-25 | feat(updates): surface above-ceiling GAMDL updates as untested + admit v3.3 | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `cb34d544` | 2026-04-25 | fix(config): drop stale song_codec_priority tests + unused import (#617, PR #621) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `008cae8d` | 2026-04-25 | fix(updates): don't surface post-upgrade refresh failure as an upgrade failure | 96% | No (present) — content in #854 v1.9.4 snapshot; probe 96% |
| `05adad63` | 2026-04-25 | fix(updates): surface real pip error when GAMDL upgrade fails | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `5d80f8d0` | 2026-04-26 | feat(release): seven-tier release-channel ladder + push-driven alpha/beta/rc | 70% | No (superseded) — partially superseded — alpha/beta/rc tiers retained on alpha; cron tiers removed (#879) |
| `e1e85f67` | 2026-04-26 | fix(ci): remove duplicate fi in update-security-policy workflow | n/a | No (present) — content in #854 v1.9.4 snapshot; probe n/a |
| `a668e97c` | 2026-04-28 | feat(release): weekly + monthly cron workflows + branches (#628) | 5% | No (superseded) — superseded — weekly/monthly workflows deliberately removed on alpha (#879) |
| `71b868e8` | 2026-04-28 | fix(ci): auto-publish prerelease drafts at the end of release.yml (#646) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f73199a5` | 2026-04-28 | fix(ci): version-bump.yml pre-creates GitHub Release to prevent draft fragmentation (#645) | 60% | No (present) — content in #854 v1.9.4 snapshot; probe 60% |
| `fbae9474` | 2026-04-28 | fix(ux): missing 'to' in untested-GAMDL warning message | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `25b4f70a` | 2026-04-29 | feat(settings): allow removing/re-adding codecs in fallback chains (#659) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `85e39268` | 2026-04-29 | fix(activity-log): suppress Python traceback noise in non-verbose mode (#660) | 90% | No (present) — content in #854 v1.9.4 snapshot; probe 90% |
| `9840ee48` | 2026-04-29 | fix(lint): indent sub-bullets in parse_gamdl_output priority list | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `56e67aa4` | 2026-04-29 | fix(notifications): make native OS notifications actually fire (#658) | 90% | No (present) — content in #854 v1.9.4 snapshot; probe 90% |
| `3fac521a` | 2026-04-29 | fix(queue): block terminal-state revival + clarify timeout messaging (#661) | 80% | No (present) — content in #854 v1.9.4 snapshot; probe 80% |
| `67ac4faf` | 2026-04-29 | fix(queue): cooperative-cancel companion task on completion-task abort (#663) | 52% | No (present) — content in #854 v1.9.4 snapshot; probe 52% |
| `75a5fcdd` | 2026-04-29 | fix(toast): auto-dismiss duplicate-URL warning (#657) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `687b9870` | 2026-04-30 | feat(queue): auto-retry failed downloads with account region storefront (#666) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `4c351eb0` | 2026-04-30 | feat(queue): smart manifest-driven retry — only re-fetch missing tracks (#667) | 92% | No (present) — content in #854 v1.9.4 snapshot; probe 92% |
| `c6bb3218` | 2026-04-30 | feat(retry): per-item + right-click + bulk retry UX on History and Queue (#665) | 77% | No (present) — content in #854 v1.9.4 snapshot; probe 77% |
| `e8a553e6` | 2026-04-30 | feat(settings): expose Track/Disc Number Padding controls (#587) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `1d9ccb5f` | 2026-04-30 | fix(deps): bump @tauri-apps/api + cli to 2.11.0 to match Rust crate | 47% | No (superseded) — superseded — alpha carries newer dep pins (#1007/#1015/#1035) |
| `361ccd99` | 2026-05-02 | fix(queue): unblock #666 storefront fallback on GAMDL v3.4+ + detect MV cover bug | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `1cedfbe4` | 2026-05-03 | fix(release): avoid parallel updater manifest uploads | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `1214f31d` | 2026-05-04 | fix(release): require conventional PR titles | 95% | No (present) — content in #854 v1.9.4 snapshot; probe 95% |
| `a8fdca50` | 2026-05-05 | feat(queue,history): add per-item delete to Queue and History | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `8a5a3f55` | 2026-05-06 | fix(queue): stop classifying per-track codec skips as download failures | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `96d52021` | 2026-05-07 | fix(queue): enforce strictly-serial post-processing via ActiveSlotGuard (#706) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `7a98dd92` | 2026-05-07 | fix(queue): identify content in codec-exhaustion activity log messages | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `39711f2e` | 2026-05-07 | fix(queue): scale companion-phase timeout by tier count (#705) | 55% | No (present) — content in #854 v1.9.4 snapshot; probe 55% |
| `8e849a79` | 2026-05-08 | feat(release): v1.0.1 prep — GAMDL 3.5.1, activity-log refactor, Library Scan scaffold | 85% | No (present) — content in #854 v1.9.4 snapshot; probe 85% |
| `2bf345a2` | 2026-05-08 | feat(release): v1.0.2 prep — MV cover workaround + 3 unification helpers + fast-uri patch | 77% | No (present) — content in #854 v1.9.4 snapshot; probe 77% |
| `11979abf` | 2026-05-08 | feat(release): v1.0.3 prep — helper migrations + Library Scan diff badges | 95% | No (present) — content in #854 v1.9.4 snapshot; probe 95% |
| `9e7cfc55` | 2026-05-08 | feat(release): v1.0.4 prep — per-item MV override + more helper migrations | 77% | No (present) — content in #854 v1.9.4 snapshot; probe 77% |
| `ce4793cf` | 2026-05-08 | feat(release): v1.0.5 prep — Library Scan gap-fill modal + Re-download action | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `9e214208` | 2026-05-08 | feat(release): v1.0.6 prep — Library Scan freshness + helper migration (#724) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `6d5a0a7d` | 2026-05-08 | feat(release): v1.0.7 prep — Zustand async-resource factory primitive (#725) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `de2ee9f1` | 2026-05-08 | feat(release): v1.0.8 prep — four more recursive walker migrations (#726) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `1409ab50` | 2026-05-08 | feat(release): v1.0.9 prep — walk_dir_find_first + last two walker migrations (#727) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `38c0bfc2` | 2026-05-08 | fix(ci): stop release-channel workflow self-trigger loop ([skip ci]) | 0% | No (superseded) — superseded — alpha uses the stronger subject-prefix if-guard (#906); main later converged in #947 |
| `b6c8f2f0` | 2026-05-10 | feat(settings): expose wrapper_decrypt_ip — closes #743 (#744) | 90% | No (present) — content in #854 v1.9.4 snapshot; probe 90% |
| `b2501c32` | 2026-05-10 | fix(release): pipeline cleanup — stop placeholder, halt cadence drift (#741) | 5% | No (present) — content in #854 v1.9.4 snapshot; probe 5% |
| `169e708f` | 2026-05-11 | feat(activity-log): emit per-download GAMDL version + capability flags (#755) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `f494a745` | 2026-05-11 | feat(cover-art): RAW → PNG → JPEG fallback when GAMDL cover write fails (#756) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `3e602857` | 2026-05-11 | feat(diagnostics): capture Python tracebacks as forensic reports (#758) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `468b70b1` | 2026-05-11 | feat(history): tooltip + right-click actions on long error messages (#748) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `a8910674` | 2026-05-11 | fix(enrichment): skip filesystem sidecars in BPM/lyrics/SRT/VTT/ASS walkers (#577) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f82b4e91` | 2026-05-11 | fix(test): drop rerender() that flakes on Windows CI (#765) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `c8fab00d` | 2026-05-11 | fix(ui): stop stale 'Finalising metadata' label + sync auto-scroll checkbox (#751) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f1701b91` | 2026-05-11 | fix: drop needless borrow on traceback url + override release to v1.3.3 (#762) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `57eb96b8` | 2026-05-15 | fix(ci): pin Backend matrix macos slot to macos-14 + shim guard (#770) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `2e17fd6f` | 2026-05-15 | fix: combined MV / enrichment / queue / UX fixes (11 issues closed) (#781) | 97% | No (present) — content in #854 v1.9.4 snapshot; probe 97% |
| `845a3cba` | 2026-05-16 | fix: companion folder merge + fully parallel enrichment (#528, #779) (#786) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `8d9e64dd` | 2026-05-17 | feat(v1.6): embed mv cover, canonical mb match, licence checks, vendor rename (30+ issues) (#809) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `0f3170bc` | 2026-05-17 | feat: legacy folder merge + colour-coded activity log (closes #789, #793) (#794) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `c5e8d7c1` | 2026-05-17 | fix: companion sidecar rename + accurate per-item progress bar (#788, #790) (#791) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `840acfcc` | 2026-05-18 | feat(v1.7): queue UX, GAMDL rollback, MV folder routing, auto-backup, diagnostics (13 closed) (#813) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `ea48540c` | 2026-05-18 | feat(v1.8): queue freeze recovery, Odesli lookup, library gaps, status bar split (10 closed) (#819) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `ba73f25a` | 2026-05-18 | fix(v1.8.1): macOS startup crash (#827) + {platform} KeyError on every download (#829) (#828) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `1afa7ab0` | 2026-05-19 | feat(v1.9): native-toast diagnostics, bulk retry, restart prompt, false-complete fix, 95% peg fix, codec labels, integrity scan (7 closed, 1 partial) (#837) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `e49086e3` | 2026-05-19 | fix(activity-log): break 'Nothing queued for ...' URL lists across lines (#845) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `056782af` | 2026-05-19 | fix(activity-log): break 'Queued: ...' URL lists across lines (#849) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `a8b76962` | 2026-05-19 | fix(activity-log): break long deduplicated-URL log entries across lines (#840) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `71062c12` | 2026-05-19 | fix(companion): never walk whole output library on missing/empty hints (#839) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f0ef6102` | 2026-05-19 | fix(companion): run lyrics conversion once per item, not once per tier (#843) | 92% | No (present) — content in #854 v1.9.4 snapshot; probe 92% |
| `7cb83ee8` | 2026-05-19 | fix(enrichment): scope all enrichment passes to a specific album dir (#842) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `611afc99` | 2026-05-19 | fix(lint): collapse nested if in #843 single-run lyrics block | 50% | No (present) — content in #854 v1.9.4 snapshot; probe 50% |
| `50b3fa3a` | 2026-05-19 | fix(lint): use let-else for output_dir extraction to satisfy clippy 1.95 | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `b7019973` | 2026-05-19 | fix(watchdog): count activity-log emissions as progress signal (#846) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `f8bf38c8` | 2026-05-19 | perf(fs): bound find_deepest_audio_dir recursion to depth 10 (#844) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `2f87df38` | 2026-05-20 | fix(watchdog): refresh tracked activity_count after own WARN emission (#851) | 100% | No (present) — content in #854 v1.9.4 snapshot; probe 100% |
| `795dd8ed` | 2026-05-22 | feat(deps): add rclone as optional bundled tool + multi-endpoint updater fallback (prep for #858) (#863) | 100% | No (superseded) — present via #877 forward-port (rclone markers: dependency_manager.rs 23 hits, tool-versions.toml 5 hits) |
| `28a8e2b8` | 2026-05-22 | fix(release-please): revert manifest 1.10.0 → 1.9.4 (corrects PR #864's 1.11.0 proposal) (#866) | n/a | N/A — N/A — release-please manifest revert; alpha manifest is 1.11.0 by design |
| `134e3e8c` | 2026-06-19 | fix: bundled audit hardening + #935 syllable lyrics + #937 release pipeline + repo hygiene (closes #935, #937, #938-#946) (#947) | 55% | **PARTIAL** — see §1.3 fragment table |

---

*Report generated 2026-07-24 by a read-only content audit. No branches, tags, or files other than this report were modified. Probe scripts and intermediate TSVs live in the session scratchpad (not committed).*
