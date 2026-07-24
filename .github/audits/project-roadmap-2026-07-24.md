# MeedyaDL — Project-State & Roadmap Analysis (2026-07-24)

**Baseline:** `alpha` @ 1.12.0-alpha.32 (freshly reconciled per EPIC #1040 Phases 1–2; branch
`claude/pr-1037-alpha-setup-p3ob3y` = alpha tip + 1 handoff commit). Content-complete: all of
main's substantive fixes verified present (`.github/audits/alpha-main-drift-content-analysis-2026-07-24.md`).
`cargo test --lib` 1516/0 · vitest 560/560 · clippy clean (per handoff).

**Method:** evidence-based codebase survey (all `file:line` refs verified against the working tree),
cross-checked against `.claude/CLAUDE.md`, `Project_Plan.md`, `.github/HANDOFF.md`,
`.claude/memory/project_multi_service_ui_direction.md`, and the drift audits. GitHub API was
unavailable this session — "Has issue?" is derived from issue numbers referenced in code/docs/handoff.

**Legend:** Effort S (< half-day) / M (half-day–2 days) / L (> 2 days) · Impact High/Med/Low ·
Model = suggested implementer (Haiku = mechanical, Sonnet = standard, Fable/Opus = design-heavy).

---

## Milestone completion picture (canonical numbering per #949: M8 = BBC iPlayer v2.0.0, M9 = Spotify v2.1.0, M10 = YouTube v2.2.0)

| Milestone | True state | Est. complete | Evidence |
| --- | --- | --- | --- |
| **M9 Spotify (votify)** | **Deeply implemented on alpha, dev-access-gated.** Waves M9-1…M9-7 + M9-UI all landed: pip install/version/support-window (`services/spotify_service.rs` 392 LOC, `votify_capabilities.rs` 648 LOC), anti-ban engine (throttle, daily cap, counter persistence — `spotify_anti_ban.rs` 498 LOC + `commands/spotify_anti_ban.rs`), 4-outcome dispatch gate incl. session artifacts (desktop DLL + Widevine `.wvd`, `commands/spotify_anti_ban.rs:425`), a **full end-to-end queue dispatch arm** with cancellation polling, progress events, partial-success detection, counter increment, and manifest write (`download_queue.rs:7347-7357`, `run_spotify_dispatch_arm` at `:11506`), Settings > Spotify tab (`SpotifyTab.tsx`), first-run consent modal (`SpotifyConsentModal.tsx`, `App.tsx:1104`), platform `enabled = true` in `engines.toml:183` (votify engine gated behind `dev_access_enabled` per `engines.toml:112`). | **~75%** | Remaining: live validation with a real Premium account + `.wvd`; enrichment/metadata parity decisions (Spotify items skip the Apple-only enrichment pipeline by design); un-gate from dev-access for GA; votify ceiling audits ride `upstream-engine-watch.yml`. |
| **M8 BBC iPlayer (get_iplayer / yt-dlp)** | **Pure stub.** Every public fn returns "not yet implemented" (`services/bbc_iplayer_service.rs:43,64,80`); engine builder stub errors "planned for v2.2.0" [sic — wrong number, see #949] (`engine_runner.rs:486`); platform `enabled = false` (`engines.toml:216`). Scaffolding that DOES exist: `models/get_iplayer_options.rs`, `BBCiPlayerTab.tsx` (under `@ts-nocheck`), platform icon + brand token, engine-fallback chain design, GPL obligations pre-analysed (#802, `.claude/memory/project_third_party_licence_obligations.md`). | **~10%** | |
| **M10 YouTube (yt-dlp)** | **Pure stub.** Same shape (`services/youtube_service.rs:42-85`, `engine_runner.rs:464`); platforms youtube + youtube-music `enabled = false` (`engines.toml:193,206`); `models/ytdlp_options.rs` + `YouTubeTab.tsx` (`@ts-nocheck`) exist. | **~10%** | |

**#911 multi-service UI:** Phase 1 substantially **landed on alpha** — responsive column system
(`QueueListVirtualized.tsx`, `QueueItemExpandPanel.tsx`), `StatusPill.tsx`/`RiskPill.tsx`,
`PlatformIcon.tsx` + `lib/platform-config.ts`, WCAG-AA brand tokens incl. a11y variants
(#911-7, `types/index.ts:1817-1860`, `styles/themes/*.css`), per-service download preview
(#911-9, `ServiceDownloadPreview.tsx`), undo affordances (#911-8, `downloadStore.ts:633/675` —
accounting honesty tracked in #991). Phase 2 (filter chips, Accounts page, connection strip,
first-launch picker) and Phase 3 (Cmd-K palette, bulk-actions toolbar) **not started** — correctly
gated on ≥ 2 GA services.

---

## Tier A — Correctness / infra gaps (ranked)

### A1. `ci.yml` never runs on alpha PRs or alpha pushes — the single highest-value infra fix
- **What:** `on.push.branches: [main, develop]` and `on.pull_request.branches: [main]`
  (`.github/workflows/ci.yml:51-55`). ALL active development happens on `alpha`/prep branches, so
  PRs into alpha get only actionlint + static-security + pr-security (pr-security.yml:37-41 covers
  alpha; ci.yml does not). Full Backend/Frontend 3-OS matrix validation currently happens only
  locally or *after* merge when the tag-triggered `release.yml` build burns a release on failure
  (the two "CI-rot clippy fixes" needed in Phase 2 of #1040 are direct evidence of drift this gap allows).
- **Fix (exact):** in `ci.yml` set `push.branches: [main, develop, alpha, beta, release-candidate]`
  and `pull_request.branches: [main, alpha, beta, release-candidate]` (mirroring pr-security.yml's list).
- **Blast radius:** more Actions minutes (2 jobs × 3 OS per event), mitigated by the existing
  concurrency cancel-in-progress group (`ci.yml:85-87`) and rust-cache. Note the *push* trigger on
  alpha will run CI concurrently with `alpha-release.yml` on every alpha push — acceptable
  (independent concurrency groups), or restrict to `pull_request` only if minutes are a concern
  (PR-time validation is the load-bearing part).
- Effort **S** · Impact **High** · Model **Haiku** · Issue: **likely none yet** (flagged only in
  HANDOFF.md "To resume" §; file one).

### A2. GAMDL 3.8.4 pre-stable live smoke-test gate (blocks promotion)
- **What:** ceiling 3.8.4 has never had the live gate run: real song download decrypt+mux via
  `_ammuxer` on the bundled cp312 per platform, wrapper-v2 0.0.2 round-trip (local + LAN),
  **song-ending integrity** check (3.8.2/3.8.3 shipped a data-corruption bug fixed in 3.8.4), MV
  download, pip resolution on 5 platforms + ARMv7 3.8.1 fallback. HANDOFF.md §3 + audit
  `.github/audits/gamdl-v3.8.3-v3.8.4-audit.md`.
- Effort **M** (manual, needs real accounts/hardware) · Impact **High** (data-integrity + release
  gate) · Model **human-led** (Sonnet can script harness) · Issue: covered by #1009/#1018 threads.

### A3. Stale `beta` and `release-candidate` branches poison the promotion flow
- **What:** `beta` parked at v1.9.4 (last commit 2026-05-20, 0 ahead / 26 behind main — a strict
  ancestor); `release-candidate` at 1.0.0-rc.21 (2026-05-08, 247 behind). Both have push-driven
  release workflows (`beta-release.yml`, `release-candidate-release.yml`) that would cut *stale*
  builds on any accidental push; beta-channel users' newest beta is months old (channel promotion
  in `check_all_updates` means they still see stables — no user harm, but the channel is dead weight).
  Also: neither branch has the #944 concurrency guard nor any post-April fix.
- **Fix:** at the next stable cut execute #1040 Phase 4 (alpha → beta fast-forward-shaped merge — the
  drift audit §Phase 4 confirms beta's ancestry makes this clean), and decide: refresh `release-candidate`
  from beta or retire the branch (rulesets + workflow removal) until an RC is actually staged.
- Effort **M** · Impact **Med-High** · Model **human-led + Sonnet** · Issue: **EPIC #1040** (Phase 4).

### A4. `pr-security.yml` heuristic noise — false positives on tests, doc comments, annotated `unsafe`
- **What (verified):** checks 2/3/5 (`.github/workflows/pr-security.yml:177-207`) grep raw lines, so
  they flag (a) path literals inside inline `#[cfg(test)] mod tests` blocks — the repo's dominant
  test pattern, which the filename-based `CHANGED_RUST_NONTEST` filter (`:169`) cannot exclude;
  (b) doc-comment / comment lines (e.g. a `///` example mentioning `sh -c` or `/Users/...`);
  (c) `unsafe` blocks that already carry the `// SAFETY:` justification the section title itself
  requests (`:188`).
- **Fix:** (1) pre-filter comment lines (`grep -vE '^\s*[0-9]+:\s*(//|///|//!|\*|#)'` applied to the
  grep -n output); (2) replace the per-file grep for Rust with a small awk/python scanner that skips
  regions after `#[cfg(test)]`/`mod tests {` (state-machine over brace depth — or pragmatically: drop
  hits whose line lies below the file's first `#[cfg(test)]`); (3) suppress an `unsafe` hit when any
  of the 3 preceding lines matches `// SAFETY:`; (4) same comment-line filter for the TS sink check.
  Keep everything advisory; add a negative-test fixture per the `tools/audit-checks/` convention.
- Effort **M** · Impact **Med** (advisory noise → alert fatigue → real findings ignored) ·
  Model **Sonnet** · Issue: **likely none** (standing task in HANDOFF; file one).

### A5. Channel release workflows re-resolve the whole Cargo tree (`cargo generate-lockfile`) — ships untested deps
- **What:** `alpha-release.yml:148` (and its beta/rc twins) run `cargo generate-lockfile` during the
  version bump, discarding the tested `Cargo.lock` resolution. Fix: `cargo update -p meedyadl` (or
  `--offline` bump). Pre-analysed in HANDOFF Plan A item B.
- Effort **S** · Impact **Med-High** (supply-chain / correctness of shipped artefacts) ·
  Model **Haiku** · Issue: **#995** (open).

### A6. Offline-installer bundles an unpinned GAMDL
- **What:** `release.yml` Step 8.5 (`bundle_engines=true` path, ~L794) pip-installs GAMDL without the
  `tool-versions.toml` window, so a bundle cut after an upstream release could ship an unaudited
  GAMDL. Fix: parse `tool-versions.toml` → `pip install --only-binary=gamdl 'gamdl>=3.0,<=3.8.4'`.
  Verified not yet landed (no `only-binary`/bounded spec in release.yml).
- Effort **S** · Impact **Med** · Model **Sonnet** · Issue: **#984** (open).

### A7. `is_service_implemented()` contradicts reality (stale gate helper)
- **What:** `service_dispatch.rs:151-153` returns `true` only for AppleMusic while the live Spotify
  dispatch arm exists (`download_queue.rs:7347`) — Spotify acceptance actually flows through the
  M9-5 gate in `commands/gamdl.rs:195-211`, so this helper is dead-or-misleading. Either make it the
  single source of truth (delegate to `engines.toml` `enabled` + dev-access) or delete it and its
  `not_implemented_error` twin.
- Effort **S** · Impact **Low-Med** (future-bug landmine for M8/M10 implementers) · Model **Haiku** ·
  Issue: **likely none**.

### A8. Remaining #1040 tasks: Phase 3 ancestry closure + bundle-ID change
- **What:** (a) Phase 3 `-s ours` merge of `origin/main` into alpha to kill the "681 behind" illusion
  (`git rev-list --count origin/alpha..origin/main` = 681 today) — content-no-op, owner-gated,
  resurrection guard mandatory (runbook §4; NEVER naive-merge, NEVER realign-alpha);
  (b) bundle ID → `com.meedyasuite.meedyadl` (owner-confirmed, sequenced after cleanup — touches
  `tauri.conf.json`, updater identity, macOS notification permission continuity, deep-link scheme
  registration; treat as its own mini-audit).
- Effort (a) **S** (mechanical + verification) / (b) **M** · Impact **High** (repo hygiene / release
  identity) · Model **Fable plan → Sonnet execute** · Issue: **EPIC #1040**.

### A9. Backlog correctness fixes already triaged-and-validated (implement as a batch)
Pre-validated in HANDOFF Plan A/deferred list — each has an open issue:
| Item | Evidence | Effort | Impact | Model |
| --- | --- | --- | --- | --- |
| #981 Linux x64 FFmpeg tar.xz declared TarGz → add `TarXz` support | `dependency_manager.rs:782`, `archive.rs:402` | M | Med (Linux installs broken/fragile) | Opus/Sonnet |
| #982 GPAC NSIS `/D=` quoting breaks spaced Windows usernames | `dependency_manager.rs:1657` | S-M (needs `cargo check --target x86_64-pc-windows-msvc`) | Med | Sonnet |
| #965 codec "(Experimental)" labels stale on GAMDL 3.8+ → "(May require wrapper)" | `gamdl_options.rs:264-273` (verified still present) | S | Low-Med | Haiku |
| #991 honest batch/undo accounting | `downloadStore.ts` | S | Low-Med | Sonnet |
| #1012 dead `fetch_syllable_lyrics` IPC (zero callers) | `commands/gamdl.rs` + `lib.rs` registration | S | Low | Haiku |
| #997 sudo-no-TTY actionable message (Linux ARM GPAC) | `dependency_manager.rs:1729` | S | Low | Haiku |
| #1011 `extend=audioTraits` on catalog fetch (dead tag path; wants live confirm) | `apple_music_api.rs:1042` | S | Low-Med | Sonnet |
| #987 tool checksum verification (needs mirror-published hashes) | `archive.rs` infra already supports `expected_sha256` | M | Med (supply chain) | Sonnet |
| #949 milestone renumber (docs + code strings) — see D1 | multiple | S | Low | Haiku |

### A10. `@ts-nocheck` staged files are type-safety debt
- `serviceStatusStore.ts:2`, `YouTubeTab.tsx`, `BBCiPlayerTab.tsx`, `ServiceStatusBanner.tsx` are
  excluded from `npm run type-check`. Fine while stubbed, but they silently rot (imports/types drift).
  Fix types now or add a lint rule cap. Effort **S-M** · Impact **Low-Med** · Model **Sonnet** ·
  Issue: **likely none**.

### A11. `download_queue.rs` is 16,979 lines
- The god-module (queue + dispatch + companions + enrichment + manifest + Spotify arm + tests).
  Every feature lands here; merge conflicts concentrate here (drift audit: 44-hunk QueueItem rewrite
  was easy, but the queue file is the recurring conflict hotspot). Propose an incremental extraction
  plan (companion supervisor already exists — `companion_supervisor.rs`; next: manifest writer,
  Spotify arm → `spotify_dispatch.rs`, enrichment task → own module), one module per PR, no behaviour
  change, tests move with code. Effort **L** (phased) · Impact **Med** (velocity + review quality) ·
  Model **Fable plan → Sonnet execute** · Issue: **likely none**.

---

## Tier B — Milestone completion (concrete remaining work)

### B1. M9 Spotify → GA (finish the ~25%)
1. **Live validation pass** (real Premium account, desktop DLL + `.wvd`): dispatch gate outcomes,
   real download, anti-ban throttle timing, daily-cap rollover, cancel mid-download, partial-success
   path. (M, human-led + Sonnet harness; part of EPIC #101.)
2. **Un-gate from dev-access** once validated: flip the votify gate (`engines.toml:112` comment
   documents the EPIC design), first-run consent modal already handles the legal surface. (S, Sonnet.)
3. **Post-download parity decisions:** Spotify items bypass Apple-only enrichment by design
   (`smart_download.rs:136` notes quality settings stub) — decide + document what Spotify gets
   (ReplayGain? lyrics via votify's own output? history/Library-scan integration — manifest write
   already exists at `download_queue.rs:11796`). (M, Fable decision → Sonnet.)
4. **votify support-window audit cadence:** ensure `upstream-engine-watch.yml` tickets votify releases
   the way GAMDL is audited; add the audit-doc convention for votify bumps. (S, Haiku.)
5. **#911 Phase 2 unlock:** once Spotify is GA there are 2 real services → service filter chips +
   multi-service empty state become eligible (see C3).

### B2. M8 BBC iPlayer (the actual v2.0.0 gate)
1. Implement `bbc_iplayer_service.rs` for real: get_iplayer subprocess builder + output parser
   (get_iplayer has a distinctive progress format), PVR-mode vs URL mode decision, quality mapping
   from `GetIplayerOptions`. (L, Sonnet with Fable design for the parser/fallback semantics.)
2. Wire `try_engine_fallback()` get_iplayer → yt-dlp for the first real multi-engine platform —
   this exercises engine-chain code that has never run in production. (M, Sonnet.)
3. Dependency story: get_iplayer is Perl (needs a managed Perl or system-dep approach — NOT covered
   by the pip engine service; design decision required) + shared yt-dlp install (see B3.1). (Fable design.)
4. **GPL compliance:** get_iplayer is GPL — `release.yml` Step 8.5 must ship corresponding source or
   the three-year written offer when `bundle_engines=true` (#802; matrix in
   `.claude/memory/project_third_party_licence_obligations.md`). (M, Sonnet; blocks bundling only.)
5. Flip `engines.toml` `[platforms.bbc-iplayer] enabled`, un-`@ts-nocheck` `BBCiPlayerTab.tsx`,
   region-restriction UX (UK VPN guidance surfaced at queue time). (M, Sonnet.)

### B3. M10 YouTube (+ YouTube Music #103)
1. **Shared yt-dlp dependency management first** (Project_Plan.md:533): install once via
   `pip_engine_service.rs`, version-window + capability cache mirroring `votify_capabilities.rs`
   (yt-dlp's monthly release cadence makes the bounded-window + "Untested" pattern essential). (M, Sonnet.)
2. `youtube_service.rs` real implementation: command builder from `YtdlpOptions`, progress parser
   (`[download] x% of y at z`), format-selection mapping, SponsorBlock passthrough. (L, Sonnet.)
3. YouTube Music (#103) rides the same engine with audio-extraction defaults. (M, after M10 core.)

---

## Tier C — Enhancements aligned with the vision

| # | Item | Rationale / evidence | Effort | Impact | Model | Issue? |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | **Server-side MusicKit token service** (Cloudflare Workers → internal API) | CLAUDE.md "Future Ideas"; removes the user-supplied Apple Developer credential requirement (DPLA 2.1/2.8 bars embedding `.p8`); `TokenSource` 3-tier chain in `apple_music_api.rs` is already shaped to accept a 4th remote tier; owner has CF accounts wired | L | High (biggest onboarding friction today) | Fable design → Sonnet | Dev_Notes architecture exists; likely EPIC-less — file one |
| C2 | **Release-notes robustness remainder (Plan B, #1033)** | body-lint tripwire in `release.yml ensure-release`, quality-lint in `release-note-gate.yml`, backfills (v1.10.0-alpha.16, v1.1.x/v1.2.0/v1.4.0) | M | Med | Sonnet | **#1033** open |
| C3 | **#911 Phase 2** (service filter chips, multi-service empty state, Accounts page, connection strip, first-launch picker) | Gated on ≥ 2 GA services (Spotify GA unlocks); brand tokens + `SERVICE_BRAND_BACKGROUNDS` already anticipate chips (`types/index.ts:1854+`) | M-L | Med | Sonnet | **#911** open (sub-items 10–14) |
| C4 | **Accessibility scan + fixes (WCAG)** | HANDOFF roadmap item 3; a11y EPIC #125; strong foundation exists (a11y CSS themes, ARIA, focus trap) — needs a systematic pass (axe-core in vitest?) | M | Med-High | Sonnet | **#125** EPIC |
| C5 | **i18n completion** | Only en/de/fr locales; most newer components hardcode English strings (grep `useTranslation` coverage is thin vs component count) | L (incremental) | Med | Haiku (mechanical extraction) + community | likely none |
| C6 | **MV filename Tiers 2 & 3** (Catalog `music-videos/{id}?include=albums`, parent-album context) | `download_queue.rs:4797-4801` "not yet wired — tracked in #537" | M | Med | Sonnet | **#537** open |
| C7 | **Per-track-per-codec Library-Scan granularity** | #667 Phase 3 (out of scope note in CLAUDE.md Library Scan section) | M | Low-Med | Sonnet | **#667** open |
| C8 | **Cross-platform smart-download search** | `smart_download.rs:86` "Phase 1: return early — cross-platform search not yet implemented"; pairs with MusicBrainz `external_urls` + odesli_service.rs already present | M-L | Med | Sonnet | likely part of an existing EPIC (#100) |
| C9 | **#911 Phase 3** — Cmd/Ctrl+K palette + bulk-actions toolbar | Medium-term per memory doc; absorbs Abort All (#620) into toolbar | M-L | Med | Sonnet | **#911** (15/16) |
| C10 | **Cloud upload EPIC** (#858–#861) + Touch Bar (#386) + SwiftUI shell | Separate surfaces, explicitly out of #911 scope; SwiftUI is a strategic rewrite — keep parked until M8–M10 ship | L/XL | Low-Med now | — | issues exist |

---

## Tier D — Quick wins (each ≤ half a day, ranked by value-per-effort)

| # | Item | Evidence | Model | Issue? |
| --- | --- | --- | --- | --- |
| D1 | **#949 milestone renumber sweep** — Project_Plan.md self-contradicts (table `:361-366` says M8=BBC/M9=Spotify/M10=YouTube; section headings `:379` "Milestone 8 — Spotify", `:426` "Milestone 9 — YouTube", `:475` "Milestone 10 — BBC iPlayer"); code strings inverted too (`engine_runner.rs:464` yt-dlp "planned for v2.1.0", `:486` get_iplayer "v2.2.0" — canonical: BBC=v2.0.0, YouTube=v2.2.0); also `types/index.ts:936`, `HelpViewer.tsx`, `help/supported-services.md`, `settings.rs:589` | Haiku | **#949** open |
| D2 | **ci.yml alpha triggers** (see A1 — listed here because it is genuinely a 2-line diff) | Haiku | file one |
| D3 | **#965 codec label copy fix** (`gamdl_options.rs:264-273`) | Haiku | **#965** |
| D4 | **#1012 dead IPC removal** | Haiku | **#1012** |
| D5 | **`is_service_implemented` fix/removal** (A7) | Haiku | file one |
| D6 | **#964 help wrapper phrasing for GAMDL 3.8+** (only ALAC is wrapper-dependent now) | Haiku | **#964** |
| D7 | **Issue hygiene:** close shipped-but-open #925 / #962 / #999; sweep #1040's task list; reconcile Project_Plan.md "🔲 Planned" for M9 (`:365`) against its actual ~75% state | Haiku | HANDOFF item 8 |
| D8 | **#998 CSP comment** + **#997 sudo message** (bundle with any Plan A batch) | Haiku | open |
| D9 | **Docs refresh batch:** README/CHANGELOG/SECURITY/help to alpha reality (HANDOFF items 6–7); CLAUDE.md "7 total" workflows count is stale (22 exist in `.github/workflows/`) | Sonnet | HANDOFF items 6–7, 9 |

---

## Suggested execution order (next 4–6 working sessions)

1. **D2/A1 ci.yml alpha triggers** + **A5 #995 lockfile** + **A6 #984 pin** — one infra PR, immediate safety.
2. **A4 pr-security refinements** + negative tests.
3. **Plan A batch (A9 + D1/D3–D8)** — sequential Haiku/Sonnet agents, one commit each, per-issue closes.
4. **A2 GAMDL 3.8.4 live smoke-test** (human) → unlocks **A3/#1040 Phase 4 promotion** and Phase 3 ancestry closure + **A8 bundle-ID** in the agreed order.
5. **B1 Spotify GA punch list** → then C3 (#911 Phase 2) becomes eligible.
6. **B2 M8 BBC iPlayer design spike** (Fable: get_iplayer Perl dependency + parser + GPL bundling decision) — the real v2.0.0 critical path.

