# MeedyaDL — Session Handoff

**Last updated:** 2026-07-03
**Branch context:** alpha (1.11.0-alpha.25), main (1.10.1), beta (1.9.4)

This document captures the state of a large multi-workstream session so work
can resume cleanly. Read it top-to-bottom before continuing.

---

## 1. Standing constraints (READ FIRST)

- **Do NOT merge any PRs.** The user wants to review + merge in single,
  deliberate steps to avoid PR-stacking race conditions. Prep PRs are
  staged and left open.
- **Do NOT open new stacked PRs.** Fold additional work into the existing
  consolidation branches (`#967` for alpha-bound work, `#968` for main).
- **Monthly spend limit was hit** mid-session — multi-agent Workflows/subagents
  fail with "You've hit your monthly spend limit". Do remaining work **inline**
  until the limit resets or is raised. (Planning-with-Fable/Opus +
  implement-with-Sonnet delegation is blocked while the limit holds.)
- Git safety: never push/force-push/reset-hard/modify remote without explicit
  instruction. Local branch deletion is fine (recoverable via reflog).

---

## 2. Open PRs (staged, NOT merged)

### #967 → alpha — "consolidated alpha PR" (12 commits)
Supersedes the three original alpha PRs (#955, #960, #966), which stay open
until #967 merges. Contents:
1. GAMDL v3.8 audit + `--no-exceptions` 3-era gate + quick-xml deny.toml unblock
2. Security hardening (credential allowlist #938, backup containment #939) +
   release-yml recursion-guard tightening
3. Animated cover art reliability — browser-grade HTTP headers + expanded
   JSON path chain (square `motionSquareVideo1x1`→`motionDetailSquare`,
   portrait `motionTallVideo3x4`→`motionDetailTall`, nested `.video.url`)
4. Dependency mirror — 14 npm + 2 cargo bumps matching #958/#959 (keeps
   alpha's tree aligned with main)
5. **HIGH: syllable-lyrics regression fix** — #936 (amp-api +
   `?extend=ttmlLocalizations` + Origin + `APPLE_BROWSER_USER_AGENT` +
   `extract_syllable_ttml_from_response` fallback) ported to alpha; alpha was
   silently degrading word-by-word lyrics to line-level
6. Artist-promo-video header hardening (#970 partial)
7. **HIGH sec: Zip-Slip fix** in profile-bundle restore (`safe_bundle_dest`)
8. **HIGH: after-queue one-shot re-fire fix** (`.take()` dead-code)
9. **HIGH sec: version-bump.mjs shell-injection fix** (anchored semver regex)
10. **MED sec: symlink tar-slip fix** in archive extraction (#976, closed)
11. GAMDL v3.8.1 audit + **drop GAMDL v2 support** (min 2.9.1 → 3.0)
12. Doc updates for the v2-drop (GAMDL range 2.9.1 → 3.0 across user docs)

CI: green as of last dispatch (re-dispatch after each push — PRs to alpha
need manual `gh workflow run "CI" --ref <branch>`). Tests added this session
all pass (syllable 8/8, Zip-Slip 3/3, gamdl_capabilities 32/32, motion-url 10/10).

### #968 → main — "consolidated dependency bumps" (4 commits)
1. cargo-minor-patch (log 0.4.33, uuid 1.23.4) — from #959
2. npm-minor-patch (14 updates) — from #958
3. **dependabot.yml root-cause fix** — main's config said `target-branch: "main"`,
   contradicting the documented alpha-channel-promotion intent; now targets
   alpha so future dep PRs flow alpha→beta→main
4. deny.toml quick-xml RUSTSEC-2026-0194/0195 ignore (main lacked it)

CI: green. #956/#957 were already CLOSED (Dependabot superseded them with
#958/#959) — no action needed.

**Recommended merge order (when the user is ready):** #968 → main first,
then #967 → alpha. Then the separate big alpha→beta→main reconciliation.

---

## 3. Branch topology + the big reconciliation (NOT done)

```
main (1.10.1) ⊇ beta (1.9.4)          [beta ahead of main: 0]
alpha (1.11.0-alpha.25) = +40 feature commits neither beta nor main have
main/beta = 655–679 commits ahead of alpha (stable history + backports)
```

The release ladder (alpha→beta→main) ran backwards historically — feature
work landed on main directly instead of flowing up. The 40 alpha-only commits
(M9 Spotify, brand refresh, #911 multi-service UI, Profile Bundle, Lyricsfile,
SQLite index, GAMDL 3.6/3.7 gates) must flow **alpha→beta→main** at the next
stable cut. **Do NOT run `realign-alpha`** (fast-forwards alpha onto main)
before those land in main — it would clobber them. The full alpha↔main merge
has a ~69-file conflict surface (version strings, workflows, docs, Rust
sources) — a dedicated multi-session workstream, tracked as the biggest
pending item.

---

## 4. Issues filed this session (#969–#1000)

**Functional-break sweep (vs GAMDL v3.8):** #969 (lyrics span-begin keying),
#970 (artwork shared-header-helper refactor — partial), #971 (Media-User-Token
on catalog calls), #972 (HLS resolution selection for motion art), #973
(`&l={locale}` localized variants), #974 (native fMP4 concat — drop FFmpeg
from motion-art path).

**Security + lint sweep (30 findings, 4 HIGH fixed in #967):** #975 (traceback
credential redaction — MED, deferred, needs URL-in-text redactor), #977–#998
(medium/low: SHA-256 pinning gaps, Spotify wiring gaps, platform-install bugs,
UX correctness, CSP Sentry host, etc.). #976 (symlink tar-slip) CLOSED — fixed.

**GAMDL v3.8.1:** #999 (v3.8.1 admission + v2-drop tracker), #1000 (remove
dead `fetch_extra_tags` v2-only plumbing — `for consideration`).

Already-known / referenced: #938/#939 (fixed in #967), #955/#960/#966
(superseded by #967), #961/#962/#963/#964/#965 (prior artwork/gamdl follow-ups).

---

## 5. GAMDL support policy (current)

- **v3.x only** — minimum bumped 2.9.1 → 3.0 (v2 dropped 2026-07-03).
- Ceiling: 3.8.1 (recommended 3.8.1).
- v3.0–v3.5.x → wrapper-v1 (WorldObservationLog/wrapper, 3 sockets).
- v3.6+ → wrapper-v2 (glomatico/wrapper-v2, single `--wrapper-url` daemon).
- Both wrapper generations fully supported; Settings UI adapts to the
  installed GAMDL.
- Audit trail: `.github/audits/gamdl-v3.8-audit.md`, `gamdl-v3.8.1-audit.md`.

**Animated cover art (square + portrait) + syllable/word-by-word lyrics** are
supported ITAM-Enhancer-style via the 3-tier token resolver
(`resolve_premium_feature_token`): user MusicKit JWT (account/certs) →
embedded build token → web-player dev key. All landed/hardened in #967.

---

## 6. Branch audit result

- **59 local branches deleted** (68 → 9): 42 merged-via-PR + 16 closed-unmerged/
  superseded/stale + 1 stale prep branch. Squash-merge means `git branch
  --merged` finds nothing — cross-referenced against GitHub PR history instead.
- **Kept:** `meedyadl-v2` (local-only historic archive, 24 commits — deletion
  would lose it) and `prep/refactoring/supported-service-expansion` (Apr-2026
  multi-service planning — low-risk to keep). **Flagged for owner decision.**
- **No abandoned-but-valuable work found** to consolidate — everything was
  merged-equivalent, rejected, or stale.
- **Remote branches recommended for deletion (NOT executed — remote push):**
  `chore/docs-housekeeping-2026-06-19`, `fix/847-ffprobe-demuxing-noise-filter`
  (both closed-unmerged PRs). Awaiting explicit go-ahead.

---

## 7. Deferred work (next session)

1. **Merge #968 then #967** (when the user decides) + close superseded
   #955/#960/#966 + delete their branches.
2. **The big alpha→beta→main reconciliation** (69-file conflict; multi-session).
3. **Remaining security quick-wins** (#975, #977–#998) — fix the safe
   self-contained ones (fold into #967 while it's open, or a fresh alpha PR
   after #967 merges). #975 needs a proper URL-in-text redaction helper.
4. **Enhancement discovery** — the workflow failed on the spend limit; re-run
   (`Workflow({scriptPath: '…/enhancement-discovery-wf_073fc0db-31d.js',
   resumeFromRunId: 'wf_073fc0db-31d'})`) once the limit resets, OR do a
   lighter inline pass.
5. **`fetch_extra_tags` dead-code removal** (#1000) once v2 is fully sunset.
6. **Remote stale-branch deletion** (§6) — needs go-ahead.

---

## 8. Verification cheatsheet

```bash
# CI on the two prep PRs (re-dispatch after each push to alpha-targeted branches)
gh run list --branch prep/alpha-consolidated-955-960-966 --workflow CI --limit 1
gh run list --branch prep/main-consolidated-deps-958-959 --workflow CI --limit 1
gh workflow run "CI" --ref prep/alpha-consolidated-955-960-966   # manual dispatch

# Key test suites touched this session
cd src-tauri
cargo test --lib services::gamdl_capabilities            # 32/32
cargo test --lib services::apple_music_api::tests::extract_syllable   # 8/8
cargo test --lib commands::profile_bundle::tests::safe_bundle         # 3/3
```
