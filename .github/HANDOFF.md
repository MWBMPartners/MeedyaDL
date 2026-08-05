# MeedyaDL — Session Handoff

**Last updated:** 2026-08-05
**Working branch:** `claude/gamdl-v3-8-5-review-gs36zl` (2026-08-03 session; forked at `feat/alpha-consolidated` HEAD `dc47a43` — identical content, zero divergence — so it cleanly continues the ONE-branch / ONE-PR-to-`alpha` model; the single eventual PR to `alpha` must be cut from THIS branch, not `feat/alpha-consolidated`, to avoid stacking). Prior context: `feat/alpha-consolidated` = 30 commits on top of `alpha` @ `243e8a2a` (1.12.0-alpha.42), HEAD was `b5924ae5` before the 2026-07-27 remote feature-control commits — ONE branch, ONE eventual PR to `alpha`.

Read top-to-bottom before continuing. Supersedes the earlier 2026-07-10 handoff.

---

## ★★★ LATEST — Session 2026-08-05: Dependabot consolidation + security-alert sweep + issues/docs reconciliation

**Focus:** Forward-port the two outstanding Dependabot **security** fixes onto this alpha-bound branch, resolve GitHub's 8 dependency + 2 secret-scanning alerts, then a full GitHub-issues sweep + documentation/memory refresh. **No new PR** (no-stacking rule — the single PR to `alpha` is cut later from this branch). Model routing per maintainer: sequential **Fable 5** for deep analysis/planning, Sonnet/Haiku for implementation.

**Concurrency note:** a sibling session (same session id) pushed `47633ee` (`forward-port-security.yml` + docs) and the ip-address cherry-pick `07d909f` to this same branch while this session was working. Reconciled by **rebasing** this session's undici commit onto `47633ee` (disjoint files — no conflict). Remote HEAD `19bc476` == local. The corrected push-retry loop checks git's own exit code (the earlier version read `tail`'s exit through a pipe and falsely reported success).

**Dependency consolidation — DONE & pushed (`19bc476`).**
- **undici** override `^7.28.0` → `^7.29.0` (lock 7.28.0 → **7.29.0**) — cherry-picked from Dependabot PR **#1079** (`acb420e`), rebased onto the branch tip. Resolves all **5** undici advisories (High CVE-2026-13697 degenerate private-cache directives + 4 Moderate: Cache-Control whitespace, cookie-attr injection, retry-interceptor response desync, blob-`type` CRLF). undici is **dev-only** (`"dev": true`, via `jsdom`).
- **ip-address** lock 10.2.0 → **10.4.0** — already on-branch as `07d909f` (Dependabot **#1078**, sibling session). Resolves all **3** ip-address advisories (High leading-zero-octet SSRF + 2 Moderate: IPv4-mapped/NAT64, CIDR-suffix). Dev-only (via `socks`).
- Both are **dev/CI supply-chain hygiene** — the shipped app's HTTP is Rust `reqwest`, so end-user runtime exposure is nil (being independently verified by the Fable 5 pass). The upstream Dependabot branches/PRs (#1078/#1079, targeting `main`) are **kept intact** per instruction.

**Security-alert investigation — DONE & pushed (`71be8f4`).** Fable 5 deep analysis complete (report archived below in gist form). Findings:
- **All 8 Dependabot alerts confirmed resolved** — every fixed-version verified against its GHSA page: undici 7.29.0 is the exact 7.x patch for all 5 undici advisories (GHSA-4cwx-7wf7-3272/CVE-2026-13697, -jr45-8vmc-qm54, -v3r7-h72x-cjcm, -8xcm-r25x-g524, -m8rv-5g2x-5cg5); ip-address 10.4.0 exceeds the highest ip-address fix (10.3.1) for GHSA-mwp4-54f8-5fhr / -22jq-vg5j-6vgg / -4xrf-jv44-h6hh. **No gaps.**
- **Reachability: dev/CI-only, zero end-user runtime exposure.** undici via `jsdom` (Vitest DOM env); ip-address via `socks`←`puppeteer` (icon scripts). Both `"dev": true`, single copy each, zero `src/` references. Shipped-app HTTP is Rust `reqwest`. These are contributor/CI supply-chain hygiene, not user-facing patches — no release note warranted.
- **2 EXTRA High dev-advisories** surfaced by `npm audit` (not yet in the Dependabot alert set), fixed proactively (GIRFT): **fast-uri** 3.1.4→3.1.5 (CVE-2026-18446 host confusion, via `ajv`) by ratcheting the existing override `^3.1.1`→`^3.1.5`; **brace-expansion** 5.0.8→5.0.9 (CVE-2026-69152 ReDoS, via `minimatch`) via `npm update` **without** a global override (brace-expansion 1.x/2.x consumers elsewhere would break; `^5.x` already admits 5.0.9 so the update is durable). Also added **`ip-address ^10.4.0`** to `overrides` for ratchet parity. `npm audit` now → **0 vulnerabilities**. Validated: type-check + eslint clean, **vitest 597 passed**.
- **Secret-scanning: confirmed FALSE POSITIVE, no real secret in-tree.** `tauri.conf.json:108` `plugins.updater.pubkey` = a minisign **public** key (decodes to `untrusted comment: minisign public key: FE03A1F781F9D761…`), required by the updater to verify signatures (SECURITY.md:83). Fable grep found **zero** committed private-key material (no tracked `.p8`/`.pem`/`.key`/`.env`; every `-----BEGIN` is docs/placeholder/`format!`-assembled test fixture; the signing key lives only in `TAURI_SIGNING_PRIVATE_KEY` Actions secret). **Disposition: dismiss alerts #1 & #2 as false-positive in the GitHub UI — MAINTAINER ACTION** (no MCP tool dismisses secret-scanning alerts). Already tracked by closed **#1032**. Do NOT add `tauri.conf.json` to `.github/secret_scanning.yml` `paths-ignore` (would permanently blind scanning on the config file most likely to gain a real credential later).
- **Audit scripts all PASS**: `check_user_agent.py` (135 files, 0 hardcoded UAs), `check_ipc_commands.py` (137=137 + 129 invoke targets valid), `check_codec_registry.py` (20 sections, all `resolves_to` resolve).

**Progress:** consolidation + all security hardening **DONE & pushed** (`19bc476` deps, `71be8f4` ratchets); **#1077 closed** (download_queue split verified in-tree `d299c8a`). **Remaining:** GitHub-issues sweep (delta vs the 2026-08-03 reconciliation, which already took open issues 97→42) → docs/in-app-help/memory refresh (OpenAPI stays N/A — Tauri IPC only, per `project_api_surface_determination.md`) → keep this handoff current.

**MAINTAINER ACTIONS NEEDED (blocking nothing on-branch):**
1. Dismiss the 2 secret-scanning alerts (#1, #2) as false-positive (updater public key) in Security → Secret scanning.
2. The 8 Dependabot alerts clear on `main` when #1078/#1079 merge; on the channels they clear via `forward-port-security.yml` (future merges) or when this branch reaches `alpha`.

---

## Session 2026-08-03: GAMDL 3.8.5 admitted (zero-code-change ceiling bump)

**GAMDL v3.8.5 admission — DONE (#1074).** ADMITTED, ceiling 3.8.4 → 3.8.5, committed on `claude/gamdl-v3-8-5-review-gs36zl` (this session's branch; forked at `feat/alpha-consolidated` HEAD `dc47a43`, per the one-branch rule — the eventual single `alpha` PR is cut from here). Same zero-code-change shape as 3.8.3/3.8.4 (#1018): the 2-commit / 4-file `3.8.4..3.8.5` delta's only functional change (`20e1b76d`) is private DRM key-extraction methods inside `gamdl/interface/song.py` (drops the base64 session-key fast-path; keys now always come from the media m3u8's `#EXT-X-KEY` tags via the pre-existing `_get_drm_uri_from_m3u8_keys`). No CLI/INI/exception/output/wrapper/ammuxer change; `wrapper.py` untouched → wrapper-v2 lockstep stays 0.0.2. Wheels identical (5× cp310-abi3, no ARMv7 → ARMv7 stays on 3.8.1). Edits: `tool-versions.toml` ceiling+recommended → 3.8.5; docs (README / help/wrapper.md / smoke-test README+script / CLAUDE.md / cadence memory); `"3.8.5"` added to the `WrapperDecryptHostPort` gate-test true-list; NEW audit `.github/audits/gamdl-v3.8.5-audit.md`. **The pre-stable live smoke-test gate (`scripts/smoke-tests/gamdl_live_smoke.py`) is RETARGETED at 3.8.5 and still not yet run** — song-ending integrity check kept; the wrapper-less `aac` leg now also exercises the rewritten m3u8 key path.

**GitHub-issues sweep — DONE.** All 97 open issues reconciled against the actual codebase (evidence-backed, spot-checked); full record in [`.github/audits/issue-reconciliation-2026-08-03.md`](audits/issue-reconciliation-2026-08-03.md). Result: **40 close-as-done · 1 obsolete (#386) · 4 relabel/narrow (#1034→F10, #1069, #1033, #1012) · 4 duplicate pairs · 42 genuinely-open · 10 confirmed-live bugs** (kept open: #981 #978 #983 #991 #949 #987 #982 #997 #1011 #998). Closes executed via a Sonnet agent (comment + state) referencing the reconciliation doc.

**Ranked new-work proposals — DONE (presented + recorded).** 18 ranked items in [`.github/audits/alpha-work-proposals-2026-08-03.md`](audits/alpha-work-proposals-2026-08-03.md), awaiting maintainer go/no-go. Top 5: #991, #983+#978, #949 (done), #981, #1011. Decisions needed before starting: #987 (checksum strategy), #8/#963 (label wording), #7 (HelpViewer codegen). **#981 confirmed genuinely broken** (Linux-x64 FFmpeg `.tar.xz` declared `TarGz`; no xz decoder → primary always fails, mirror is silent SPOF).

**Documentation pass — DONE.** Fixed #949 (scrambled service-milestone numbers) in BOTH `help/supported-services.md` and the `HelpViewer.tsx` inline twin (M8=BBC/v2.0.0, M9=Spotify/v2.1.0, M10=YouTube/v2.2.0). Recorded the **OpenAPI determination** — MeedyaDL is a Tauri desktop app with no HTTP API (only in-process IPC); no OpenAPI/Swagger applies here; the native-app-facing API is a separate MeedyaSuite backend repo (`DEV_NOTES.md` → "Programmatic Interface / API Surface" + `.claude/memory/project_api_surface_determination.md`).

**Alpha-cycle implementation IN PROGRESS (2026-08-03, second session).** The maintainer selected a large batch of proposals to implement (see `.github/audits/alpha-work-proposals-2026-08-03.md`). `feat/alpha-consolidated` is now **DELETED** (content fully in this branch); `claude/gamdl-v3-8-5-review-gs36zl` is the sole work branch and the single eventual `alpha` PR is cut from it. New tracking issue **#1075** filed (build-time HELP_TOPICS codegen). **#1013** confirmed still-valid (GAMDL maturin matrix = x86_64+aarch64 only, no ARMv7 wheel) and kept open. **Local Rust builds now work here** (GTK dev libs installed) — every backend batch is `cargo test`-validated locally.

Method (per maintainer instruction): sequential Fable 5 for deep analysis+planning (one at a time), Sonnet for implementation, independent `cargo test --lib` + `npm run test` re-validation before each commit. Batches pipelined: Fable analysis of batch N+1 runs while batch N is implemented/committed (analysis is read-only, no build-lock contention).

Progress:
- **Batch A — DONE & committed** (#991 undo re-queue batching; #983 Spotify URL input reaches the dispatch gate; #1011 `extend=audioTraits`; #973 `&l=` locale on syllable/promo calls; #972 `animated_artwork_resolution` setting + HLS rendition selection, settings schema **v7→v8**). Validated: `cargo test --lib` 1612 passed / 0 failed; npm 597 passed; clippy + type-check clean.
- **Batch B — DONE & committed** (#981 xz decoder via already-present `lzma-rs` + honest `detect_archive_format_from_url` + `TarXz`; #982 NSIS `raw_arg` cfg-gated; #997 sudo no-TTY → `sudo -n`/`pkexec`/actionable error; #987 exclude GPAC nightly, `[gpac.windows_installer]` + `[mirror.asset_hashes]` pin mechanism, wire `download_and_extract_verified`). Ships SAFE defaults (no pin → mirror-first; nightly no longer executed unverified). Validated: `cargo test --lib` 1619 passed / 0 failed; clippy + `npm run check:legal` clean. Live-only flagged: BtbN extract on Linux-x64, Windows path-with-spaces, Pi elevation, GPAC pin values.
- **Batch C — DONE & committed** (#963/#1002/#965: new `GamdlFeature::AssetsApiUnlocksLossyCodecs` (≥3.8) + `SongCodec::is_wrapper_dependent_runtime()` gates gap-fill; "(Experimental)" labels UNCONDITIONAL (#965); version-conditional wrapper prose in BOTH `FallbackTab` and `QualityTab` via new `useGamdlCapabilities` hook + `assets_api_unlocks_lossy_codecs` DTO field. #1000: dead `fetch_extra_tags` plumbing REMOVED across backend+frontend. #1014: `[gamdl.platform_ceilings]` (ARMv7→3.8.1) + `current_platform_id()`/`effective_maximum_tested()`/`classify_for_platform()` wired into the startup diagnostic. #1001: backend primitive `recommended_upgrade_target()` (wrapper-v1 users → `LAST_WRAPPER_V1_VERSION` = "3.5.2", else recommended).) Validated: `cargo test --lib` 1630 passed + `gamdl_capabilities` 42/42; npm type-check + 597 tests; clippy + audit scripts clean.
  - ⚠️ **Process note:** the Batch C implementer's plan file wasn't on disk when it started (a dispatch race), so it reconstructed the spec from HANDOFF + the GitHub issues and diverged in two spots I then corrected on top: QualityTab prose (was missing) + `LAST_WRAPPER_V1_VERSION` "3.5"→"3.5.2".
  - **Deferred (keep open + follow-ups):** #1002 stays open pending live-QA of real 3.8.x wrapper-less Atmos/AC3 downloads; **#1001** stays open — only the backend primitive landed; the guided-migration UX (Updates banner + one-click, `use_wrapper`-aware) is genuinely "decision-pending" per the issue. **#1014** fuller clamp (propagating the platform ceiling into the update-banner/pip-spec, not just the startup diagnostic) is optional — the existing platform-aware `no_compatible_wheel` guard already prevents an ARMv7 user from upgrading to a wheel-less release.
- **Batch D — DONE & committed** (#934 new `test_lyrics_connection` IPC + LyricsTab button, using the PREMIUM token resolver so web-player-token users get an accurate test; #971 `#HttpOnly_` cookie-parse fix landed; #1021 post-download ffprobe integrity guard at the completion site — all-suspect→`set_error`, partial→warn+complete, ≤12-file sample; #961 partial — `fallback_storefront` geo-lock warnings + album-cover cross-variant fallback + not-silently-failing key logging; #974 partial — native fMP4 init+segment concat fallback for the animated-artwork HLS path, FFmpeg-primary). **web-dev-token fallback VERIFIED preserved** for #961 + #934 (`resolve_premium_feature_token`/keychain fns in zero diff hunks; new IPC uses the premium resolver). Validated: `cargo test --lib` 1655 passed; clippy + npm type-check + 597 tests + IPC/codec audits all clean.
  - **Deferred (keep #961/#974 open):** #961 amp-api fallback fetch path + setting, 3-way log split, Plex-aware Linux hide; #974 native-primary + `+faststart` remux + parallel fetch + **live playback verification** (VLC/QuickTime/Plex without FFmpeg). #971 full Media-User-Token threading still open (only the `#HttpOnly_` fix landed).
- **Version bump + docs pass — DONE (this commit).** Minor bump **1.12.0-alpha.42 → 1.13.0-alpha.0** across all five version-bearing files (`package.json`, `package-lock.json` ×2, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`; `.release-please-manifest.json` intentionally left at stable `1.11.0`). Docs updated for the batch A–D user-facing surface and verified 1:1 against the shipped code: **README** (animated-artwork resolution ceiling; lyrics connection-test button); **help/animated-artwork.md** + HelpViewer twin ("Choosing a Resolution" — Standard `Fhd` ~1080p default / High `Uhd` ~2160p / Maximum `Max` uncapped, labels matched to `CoverArtTab.tsx` verbatim); **help/quality-settings.md** + HelpViewer twin (GAMDL 3.8+ unlocks all non-web codecs except ALAC without wrapper); **help/lyrics-and-metadata.md** + HelpViewer twin (Settings > Lyrics "Test word-level lyrics connection", premium-token resolver behaviour); **help/downloading-music.md** + HelpViewer `downloading` topic + **help/supported-services.md** + HelpViewer Spotify topic (`open.spotify.com` accepted at the form, routed through Spotify's eligibility checks); **CLAUDE.md** (per-platform ceiling #1014 + v2→v3 target #1001). Every `help/*.md` edit has its matching inline `HELP_TOPICS` twin in `HelpViewer.tsx` (the trap). Feature existence re-verified before documenting: `AnimatedArtworkResolution`/`target_height`, `test_lyrics_connection` IPC, `platform_ceilings`/`recommended_upgrade_target`/`LAST_WRAPPER_V1_VERSION` all present.
- **`download_queue.rs` submodule split — DONE & committed (`d299c8a`).** The 17,353-line single file is now a directory module: `download_queue/{mod.rs 2130, processing.rs 5713, tests.rs 4314, companions.rs 3268, helpers.rs 973, options.rs 459, notifications.rs 334, persistence.rs 304}`. Method: **byte-verbatim `sed` extractions** (no hand-transcription), deterministic `pub(crate)` visibility bumps + module-root re-exports; external call paths (`download_queue::process_queue`, `::save_queue_to_disk`, …) preserved. The `process_queue` module is named **`processing`** to avoid colliding with the `crate::utils::process` import. Sibling `services::*` modules the moved code reached via `super::X` are re-exported into the module root (the old file was `services::download_queue`, so its `super` meant `services`; submodules are one level deeper). Validated: `cargo check --lib --tests` 0/0 warnings, `clippy --all-targets -D warnings` clean, `cargo test --lib` **1655 passed / 0 failed** (identical to pre-split), full-crate `cargo check --all-targets` clean, and a deterministic **byte-identity diff** vs `git HEAD` proving the only non-mechanical change is `mod tests { … }` → `mod tests;`. (An adversarial verify-workflow was attempted but all 4 agents hit a subagent-sandbox tool-permission failure and couldn't examine code — the split's correctness rests on the hard gates above, which are stronger than an LLM audit.)
- **Dependabot #1078 cherry-picked (`07d909f`).** Security bump of the transitive dev-dep `ip-address` 10.2.0 → 10.4.0 (lockfile-only) forward-ported from `main` onto this branch with `git cherry-pick -x` (Dependabot preserved as author).
- **NEW `.github/workflows/forward-port-security.yml` — closes the security-update routing gap.** Dependabot *security* updates always target the default branch (`main`) and ignore `dependabot.yml`'s `target-branch: alpha` (which routes *version* updates only) — no GitHub config can redirect them, so #1078 hit `main` alone and left `alpha`/`beta`/`rc` exposed. The new workflow, on a Dependabot PR merging to `main` (`pull_request_target: closed` guarded on `merged && author==dependabot[bot]`, + `workflow_dispatch` manual entry), cherry-picks the fix onto each channel branch via a matrix and opens a PR there (or a `[forward-port]` tracking issue on conflict). Idempotent; uses `RELEASE_PAT`; `actions/checkout` SHA-pinned; handles squash + merge-commit (`-m 1`). Validated with actionlint 1.7.12 + shellcheck 0.10.0 (0 findings). Cross-referenced in `dependabot.yml` + CLAUDE.md. Scope chosen by maintainer: alpha + beta + release-candidate. **Requires no action to work, but only fires on FUTURE merges** — the already-open #1078 was handled by the manual cherry-pick above; `alpha`/`beta`/`rc` themselves still need #1078 applied (either merge this branch, or `workflow_dispatch` the workflow with pr_number 1078 once it's on `main`).

Maintainer decisions still surfaced (non-blocking, code ships safe defaults): **#987** GPAC pin URL+SHA-256 values and mirror per-release SHA256SUMS strategy (mirror republishes daily); the git proxy blocks remote branch deletion so any future branch cleanup needs the maintainer.

---

## Session 2026-07-27: branch consolidation + remote feature-control programme

### Working branch — do not fragment it again

All in-flight work now lives on **`feat/alpha-consolidated`**. The standing no-PR-stacking rule was tightened: commit further work to this one branch and open a **single** PR to `alpha` when ready. Three predecessor branches were consolidated into it and PR **#1067 was closed unmerged** with its commit cherry-picked in verbatim.

Verified green before push (all run at the consolidated head):

| Check | Result |
|---|---|
| `cargo check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo test` | 1546 passed, 0 failed, 1 ignored |
| `npm run type-check` | pass |
| `npm run test` | 564 passed / 37 files |

The 12 commits: `additionalDirectories` path fix · four Spotify/engine fixes (service-aware reachability probe, Spotify manifest, votify version window, venv-aware Python resolution for pip engines) · GAMDL 3.8.4 live smoke-test harness · four CI fixes (channel-branch gating, release-please Cargo.lock resolution, mirror-tool checksum verification, release-body-audit self-correcting checkout) · permission allowlist · the analysis document below.

### Remote feature control — the programme

Full analysis: **`.claude/analysis/remote-feature-flags-analysis.md`** (~50KB, every claim tied to a named file at a named revision). Issues: **MeedyaDL#1069**, **intAppsAPI#107**, **MeedyaConverter#465**, **MeedyaManager#195**.

**The single most important finding:** the existing service-status mechanism has **never run end-to-end in a shipped build**. Independently verified — the IPC command has no frontend caller, the banner component is never rendered, the enforcement helpers have zero call sites, and the hard-coded URL points at `main` where the payload file does not exist (it exists only on `alpha`). Consequence: **there is no installed base on the interim transport**, so the cutover needs no bridge and no flag-day.

Framing that matters: the static-file transport was a **deliberate interim solution** adopted while the API was built — not a defect. Do not write it up as one. Its payload shape informs, but does not define, the API's model.

The transport seam was built for the swap (one isolated fetch function; model, cache, fallback and UI are all transport-neutral), so the swap itself is small. The bulk of the remaining work — polling lifecycle, UI wiring, enforcement call sites at finer-than-service granularity — was never finished for the interim transport either.

**API-side gaps, all verified in code:** flag mutations write no audit trail (the audit helper exists with zero controller call sites — disqualifying for a legal kill switch, and the first thing being fixed); no response signing; no version/platform/channel targeting on `main` (a rollout branch carrying migration 015 is unmerged, so settle its disposition before writing migration 016 or the numbers collide); the flag-key sanitiser rejects dots, blocking a dot-namespaced scheme; responses are all `no-store` with no `ETag`.

**On authentication — describe it honestly.** App identifier, User-Agent prefix and hashed key with fail-closed scopes *are* enforced on every app-facing route. But the key and User-Agent ship inside every client binary and are extractable by anyone holding the app: they are attribution and abuse filtering, **not** a security boundary. And client-side enforcement on a user's own hardware is fundamentally **advisory** — if a legal obligation needs a hard guarantee that a feature is off, this design cannot provide it. Say so rather than implying otherwise.

**Suite shape:** MeedyaConverter is Swift, MeedyaManager is Rust (and already consumes MeedyaSuite-core), MeedyaDL is Rust. Recommendation is **contract-first, crate-later** — specify a language-neutral wire contract, let each app implement it, and only extract a shared Rust crate once two working Rust implementations exist to factor out. Conditions are evaluated client-side specifically so no install identifier is ever transmitted. `MeedyaSuite` and `Skriptey` orgs were **not** in session scope and were not verified.

### Environment limits discovered — do not burn time rediscovering

- **Remote branch deletion silently no-ops.** `git push origin --delete <branch>` reports `Everything up-to-date` and changes nothing. The GitHub MCP toolset has `create_branch` but **no delete**, and there is no `gh` CLI here. Branch deletion is a human action.
- **Wiki pushes are refused.** `git-upload-pack` (read) is allowed on `MeedyaDL.wiki.git`; `git-receive-pack` (write) returns 403. There is no REST API for wikis. Wiki changes must be pushed from a workstation.
- Both are policy denials, not transient failures. Do not retry or route around them.

### Cron channels are still live on `main`

`nightly-release.yml`, `weekly-release.yml` and `monthly-release.yml` were removed on `alpha` (#879) but **still exist on `main`**. Scheduled workflows only fire from the default branch, so they are still cutting releases — `v1.10.2-nightly.20260727` was produced during this session. Removing them means a targeted commit to `main`, which is the one change that cannot ride the alpha branch. **Awaiting owner approval.**

### Decisions awaiting the owner

1. Approve the one-commit `main` fix deleting the three cron workflow files.
2. Is the API actually deployed and reachable in production? (Unverifiable from a session; a hosting error was flagged.)
3. Ratify fail-open behaviour and the no-opt-out privacy posture.
4. Disposition of the unmerged API rollout branch and its migration 015.
5. Signing-key custody — on the API host, or isolated.
6. Ratify **server-side flag evaluation** (decision B below) — this reverses the client-side-evaluation position this same section originally recorded; the excised sentence and its replacement now live in `.claude/memory/project_remote_feature_control.md`.
7. Ratify the intAppsAPI branch-consolidation call (decision C below) — integrate the unmerged rollout branch into `feat/feature-targeting-consolidated` rather than rebuilding its fixes from scratch.

### Session continued — User-Agent standardisation shipped; remote feature control moves server-side (checkpoint, no code beyond UA)

Two commits landed on this branch, plus a decision that changes the shape of the remaining programme. This is a **checkpoint** — the flags client itself is not yet written.

**Landed (MeedyaDL, `feat/alpha-consolidated`):**
- `6c90ecf5` + `b037560c` — User-Agent standardisation. **Two strings by design, not an oversight:**
  - `APP_USER_AGENT` (compile-time const): `"MeedyaDL/{version} (+https://github.com/MWBMPartners/MeedyaDL)"` → GitHub, PyPI, MusicBrainz, Odesli, and the Apple JWT paths. 17 call sites repointed.
  - `full_user_agent()` (runtime `static LazyLock<String>` in `utils/http_client.rs`): `"MeedyaDL/{version} ({OSName} {Arch}/{OSVersion})"`, e.g. `"MeedyaDL/1.12.0-alpha.42 (MacOS ARM64/26.6)"` → **MWBM-IntAppsAPI only**, no consumers wired yet.
  - Why the split: OS/arch/version sent to *third parties* (GitHub, PyPI, MusicBrainz, Odesli) is a fingerprinting increment that buys those services nothing; only our own endpoint has a legitimate use for it (version/platform feature targeting + analytics).
  - A reference into a `static LazyLock<String>` is `&'static str`, so `ClientConfig.user_agent: Option<&'static str>` needed no type change.
  - OS/arch use **closed vocabularies** (`MacOS`/`Windows`/`Linux`, `ARM64`/`x64`/`ARMv7`) so server-side targeting rules see stable values. OS version comes from `tauri-plugin-os` (already a direct dependency — no new crate).
  - `APPLE_BROWSER_USER_AGENT` in `apple_music_api.rs` and its 3 consumers were **deliberately left untouched** — Apple's endpoints 403 non-browser UAs.
  - New guard `tools/audit-checks/check_user_agent.py`, wired into check 8 of `.github/workflows/pr-security.yml`. Zero findings on a clean tree.
  - Verified before push: `cargo clippy --all-targets -- -D warnings` clean, `cargo test` 1550 passed / 0 failed, all 3 audit scripts clean.

**Landed (MWBM-intAppsAPI, separate repo):** new branch `feat/feature-targeting-consolidated`, commit `c92a2d0` (doc note only, no functional code yet). This is the **consolidated branch for all IntAppsAPI work** on this programme — mirrors the "one branch per repo, no PR stacking" rule already in force for MeedyaDL. MeedyaDL work stays on `feat/alpha-consolidated`; intAppsAPI work stays on `feat/feature-targeting-consolidated`.

**Issues opened this leg:** MeedyaDL **#1070** (User-Agent — still open, pending API-side prefix registration before it can close), **#1071** (flags client + notice UI, not started). IntAppsAPI **#108** (version/platform targeting), **#109** (SemVer comparator bugs), **#110** (admin guides + `X-App-ID` docs). All roll up under the existing umbrella **MeedyaDL#1069**.

**Key findings this leg (all verified in code, not inferred — preserve verbatim, they are load-bearing for the next session):**

1. **`api.mwbmpartners.ltd` has no DNS record.** The apex `mwbmpartners.ltd` resolves via Cloudflare; the `api.` subdomain does not. The API is **not deployed**. The client we're about to write is safe to ship regardless (silent-on-failure, cached, fail-open) but will be inert until the record exists.
2. **IntAppsAPI has no version/platform targeting today.** Migration 015 — which adds percentage / user-allow / user-deny / segment rollout **only** (no version/platform dimension) — lives on an unmerged branch, `claude/feature-gating-readiness-yQisQ`.
3. **Bug on intAppsAPI `main`:** `MigrationRunner`'s file-discovery regex also matches `*_rollback.sql`, and lexical sort places each rollback file immediately after its forward file — so a fresh `migrate.php` run applies a migration and then immediately rolls it back. **Fresh installs are broken on `main` today.** Already fixed on `claude/feature-gating-readiness-yQisQ` (commit `1925be0`).
4. **Bug:** `SemVerComparator` compares the prerelease segment with `strcmp`, so `"alpha.10"` sorts below `"alpha.9"` — every MeedyaDL alpha build after `.9` hits this. Separately, `normalize()` requires 3-part version strings, but macOS reports `"26.6"`, Ubuntu `"24.04"`, Debian `"12"` — platform-version targeting rules would silently never match. Both filed as intAppsAPI **#109**.
5. **Branch `claude/feature-gating-readiness-yQisQ` (tip `4b8f2aa`) is not just migration 015** — it also carries the `schema.sql` cumulative-snapshot fix (`13e4de0`) and a CI DB Check workflow (`7f8c81e`). No other branch has those commits. Merging it now has a **real conflict** in `web/src/Controllers/Admin/FeatureController.php` (both sides independently wire up `AuditLogger`) — an earlier "zero conflicts" note about this branch is **stale**, do not trust it without re-diffing.
6. **Trap:** `src/components/help/HelpViewer.tsx` does **not** read `help/*.md` at runtime — help content is inline template literals in a `HELP_TOPICS` array (`src/components/help/HelpViewer.tsx:170`). Any help-doc change must land in **both** the `.md` file and the matching inline `HELP_TOPICS` entry, or the two silently diverge.
7. **`CLAUDE.md` said "12 help topics" — stale.** There are 16 (`help/*.md`: `animated-artwork`, `cookie-management`, `downloading-music`, `downloading-videos`, `fallback-quality`, `faq`, `getting-started`, `index`, `keyboard-shortcuts`, `lyrics-and-metadata`, `metadata-mapping`, `quality-settings`, `release-channels`, `supported-services`, `troubleshooting`, `wrapper`) and 16 matching `HELP_TOPICS` entries [**correction, 2026-07-27 docs-sweep session:** this "16 matching `HELP_TOPICS` entries" claim was wrong — `HELP_TOPICS` has **15** entries, and its `id`s are not 1:1 with the 16 `.md` filenames (`help/index.md` has no inline twin, and neither does `help/faq.md`, by design). See `grep -c "label: '" src/components/help/HelpViewer.tsx` = 15]. **Fixed in this checkpoint's CLAUDE.md edit.**

**Decisions taken as assumptions this leg — asked, not answered by the maintainer; all reversible; recorded here so no one mistakes them for ratified design:**

- **A. Split User-Agent** (platform detail to our own endpoint only, reduced everywhere else). Low-risk, already shipped in code (§ Landed above).
- **B. Server-side flag evaluation** — the client sends `app_version` / `platform` / `platform_version`; the server returns an already-resolved boolean, rather than the client fetching raw rule conditions and evaluating them locally. **This reverses a position ratified earlier in this same document and in `.claude/memory/project_remote_feature_control.md`** ("Conditions are evaluated client-side precisely so no install identifier is ever transmitted"). Justification: `full_user_agent()` already transmits that exact class of data to the same endpoint, so the privacy delta of server-side evaluation is zero — and client-side evaluation would freeze rule semantics into every already-shipped binary, defeating the point of a remote kill switch. **The old sentence has been excised, not merely contradicted**, from `.claude/memory/project_remote_feature_control.md`; see that file for the replacement privacy wording, which must also propagate to README/TERMS when the flags client ships.
- **C. Integrate `claude/feature-gating-readiness-yQisQ` into intAppsAPI** rather than rebuilding its fixes (migration-runner bug, schema.sql snapshot, CI DB Check) from scratch on the new consolidated branch. Real conflict in `FeatureController.php` (finding 5) still needs resolving when this happens.

**Remaining chain — in order:**

1. ✅ **DONE — Flags client** (MeedyaDL, commit `c4a2185b`) — silent-failure semantics: a fetch failure keeps the last known verdicts, **no user-visible notice**, an `emit_app_log` Activity Log entry only (`"Feature availability refresh failed — keeping last known status"`). Notices are still shown for a feature that is genuinely resolved **disabled** by the server.
2. ✅ **DONE — Notice UI** (MeedyaDL, commit `9884e669`) — the banner a user sees when a feature the server has switched off would otherwise be reachable (`FeatureNoticeBanner.tsx` + `featureFlagStore.ts`).
3. **IntAppsAPI #109** (SemVer comparator + `normalize()` 3-part fix) — must land before #108, since #108's version/platform targeting depends on comparisons being correct.
4. **IntAppsAPI #108** (version/platform targeting — migration 016, informed by but not copying migration 015's shape).
5. **IntAppsAPI #110** (admin guides + `X-App-ID` docs).
6. ✅ **DONE — Full docs sweep** (MeedyaDL, commits `132dbeb4` help copy + `60f36d98` root docs + this commit's `.claude/` checkpoint) — root `.md` files, `help/` topics **and their inline `HELP_TOPICS` twins** (finding 6 — do not update one without the other), `DEV_NOTES.md` including the `INTAPPS_*` env-var names and the `option_env!()` injection pattern (no other transport detail). See the dated subsection immediately below for what did and did not land.
7. **`.claude/` refresh** — once the client + notices exist, `.claude/memory/project_remote_feature_control.md` and this handoff both need a "shipped" pass. (Partially folded into item 6's commit — see below; the open items from that pass are recorded there too.)

### Session continued — 2026-07-27 (docs sweep for the shipped flags client)

Three more commits landed on `feat/alpha-consolidated`, documenting the client + notice UI shipped as `c4a2185b` and `9884e669` above:

- `132dbeb4` — `docs(help)`: new "temporarily unavailable" guidance in `help/troubleshooting.md`, `help/supported-services.md`, `help/faq.md`, plus the matching inline `HELP_TOPICS` twins in `HelpViewer.tsx` for `troubleshooting` and `supported-services` (no twin added for `faq`, matching the pre-existing pattern where `faq` has none).
- `60f36d98` — `docs`: README (Quality of Life bullet, 12→15 help-topic count fix, Roadmap row flipped to shipped), TERMS.md (Data Collection paragraph + Last-updated bump), SECURITY.md (three new Security Measures bullets), DEV_NOTES.md (`INTAPPS_*` secrets table + new "Remote Feature Availability (Developer Notes)" section + corrected v2 feature-status row + corrected Help Topics file-count row), Project_Plan.md (two roadmap rows flipped from "🔮 Future" to "🚧 Partially shipped").
- This commit — `docs(claude)`: `.claude/CLAUDE.md` new architecture bullet (full technical detail) + Key Directories insertions, this handoff's own updates, `.claude/memory/project_remote_feature_control.md` dated append, and a correction banner on `.claude/analysis/remote-feature-flags-analysis.md` flagging its evaluation-model description as superseded.

**Still not done after this sweep** (do not assume otherwise from the "docs sweep" checkmark above):
- No enforcement call sites exist anywhere — the client resolves and caches verdicts; nothing in the app gates behaviour on them yet.
- `notice.url` still has no scheme validation and is still never rendered.
- Decision B (server-side evaluation) is still unratified by the maintainer — the docs sweep documents it as shipped fact because that's what the code does, but nobody has gone back for explicit sign-off since the reversal recorded earlier in this file.
- Chain items 3–5 and 7 (the IntAppsAPI-side work) remain untouched.

### Session continued — 2026-07-27 (enforcement shipped, two defects fixed, docs updated for enforcement)

Two more commits landed on `feat/alpha-consolidated`, on top of the docs sweep above:

- `9042e7d3` — `fix(flags)`: the client's `verdicts` deserializer was map-only, but the live server's `FeatureController::list()` answers with `data.features` as a JSON **array** — every real fetch had been silently failing to parse, invisible forever behind the refresh silent-failure contract. Fixed with an untagged `VerdictsWire` enum accepting either shape. Separately, `UNGATEABLE_KEYS` used dotted keys (`"core.remote-config"`, `"core.updater"`) that the backend's slug sanitiser (`^[a-zA-Z0-9_-]+$`) can never create or serve — so the "a compromised admin account cannot blind the fleet" containment never actually held. Renamed to kebab-case (`"core-remote-config"`, `"core-updater"`); every flag key in the system (`MediaServiceId::flag_key()`'s five service keys included) now follows the same `^[a-z0-9-]+$` grammar, namespaced by prefix (`core-`/`service-`/`feature-`) rather than by dot.
- `747c8cda` — `feat(flags)`: enforcement itself. `feature_flag_service::service_gate()` is wired at four enqueue seams only — `start_download`, `retry_download`, `retry_failed_bulk`, `import_queue` (per-item) — and deliberately absent from `process_queue`, startup recovery, `try_fallback`, gap-fill, companions, enrichment, and `retry_download_without_wrapper`. A pause stops new work starting and is never retroactive; nothing already in flight can be stranded by a flag flip. Dead `service_dispatch::is_service_remotely_enabled()`/`service_disabled_error()` (zero call sites, wrong/superseded transport) were deleted rather than left as a trap.
- This commit — `docs`: updated the enforcement-era wording across `help/troubleshooting.md`, `help/supported-services.md`, `help/faq.md` and the matching `HelpViewer.tsx` inline twins (`faq` still has none, by design), the README feature-availability bullet, `.claude/CLAUDE.md` (fixed the pre-download-checks bullet — it's five gates now, not three — and corrected the remote-feature-availability bullet's stale dotted `UNGATEABLE_KEYS` example, plus documented the dual wire-shape deserializer), and a dated append to `.claude/memory/project_remote_feature_control.md`. TERMS.md and SECURITY.md were reviewed and left unchanged — their existing wording was already accurate for the enforcement behaviour, not just the notice behaviour.

**What remains** (unchanged from the "still not done" list above except where noted):
- `notice.url` still has no scheme validation and is still never rendered.
- Decision B (server-side evaluation) is now **more load-bearing than before, not less** — enforcement means a server-side evaluation bug can incorrectly refuse or incorrectly admit a real download, not just mis-render a banner. Still unratified by the maintainer.
- On the API repo side (per the maintainer, not independently verified this session): admin rules UI and in-console guides are still being built; a suite client-integration doc for the other two apps (MeedyaManager, MeedyaConverter) does not exist yet; production deployment/provisioning is still outstanding.
- A gap was found while documenting this, not fixed: `start_download`'s URL classification loop only inspects `http`/`https` schemes, so a bare `spotify:` URI (e.g. `spotify:track:...`) skips the domain allowlist, the feature-availability gate, and the M9 Spotify anti-ban dispatch gate entirely — see the dated append in `.claude/memory/project_remote_feature_control.md` for the exact code path. Needs a decision (recognise the `spotify:` scheme explicitly, or reject any non-`http(s)` scheme up front) before Spotify (M9) ships broadly.
- Chain items 3–5 and 7 (the IntAppsAPI-side work referenced further up) remain untouched from this session's perspective.

**Maintainer decisions still open:** the same list as "Decisions awaiting the owner" near the top of this file, plus Decision B (server-side evaluation architecture) above — none of these were resolved by shipping enforcement; enforcement was built on top of the existing assumptions, not a resolution of them.

### Session continued — 2026-07-27 (FINAL checkpoint this leg): bare `spotify:` URI gap closed; intAppsAPI companion fixes landed on its own branch

One more commit landed on `feat/alpha-consolidated`, closing the gap flagged at the end of the previous subsection, plus four commits landed on the sibling repo's own consolidated branch (one branch per repo — no PR stacking, same rule as this repo's).

**Landed (MeedyaDL, `feat/alpha-consolidated`):**
- `b5924ae5` — `fix(flags)`: the classification `if` in `start_download` had no `else`, so a bare `spotify:album:...`/`spotify:track:...` URI (no host to check) fell through with no `has_spotify` flip and no rejection — it evaded the feature-availability gate AND the entire M9 anti-ban dispatch gate (dev-access, consent, DLL/`.wvd` presence, daily cap — all skipped), then enqueued with `service: None`, which `process_queue()`'s legacy fallback treats as Apple Music, dispatching a Spotify URI to GAMDL, which rejected it with an error naming neither Spotify nor the cause. Classification is now factored into a pure, unit-tested `classify_batch_urls()` helper with an explicit `spotify:`-scheme branch, and a new `reject_bare_spotify_uris()` helper rejects the bare-URI shape by name AFTER both gates run — so a paused Spotify service still shows the pause message rather than a generic scheme error. 9 new unit tests. See `.claude/CLAUDE.md`'s "Remote feature availability" bullet for the full mechanism and `.claude/memory/project_remote_feature_control.md`'s dated append for the analysis trail.

**Landed (MWBM-intAppsAPI, sibling repo, branch `feat/feature-targeting-consolidated`):**
- `0f70813` — dual-domain deployment move: the API is now served from both `service.api.<domain>/` (app at root) and `api.<domain>/service/` (same files under a path segment), one brand per domain (`meedyasuite.com` for Meedya-branded apps incl. MeedyaDL, `mwbm.io` for the rest).
- `b057b80` — dropped the hardcoded `RewriteBase /service/` from `web/.htaccess` because it broke the subdomain form (relative substitution expanded to a non-existent `/service/index.php`); the previous value's rationale (avoiding Apache's per-directory base-guessing misresolving through a symlink) is preserved as a documented fallback in the file itself, since neither form has been verified against the real host yet.
- `d4945cf` — admin UI for disable rules with constraint validation and an effect preview.
- `a2205da` — in-console guides for connecting an app and configuring feature flags.
- `24ed917b` — **percentage-rollout fail-open fix**: `Feature::evaluateRollout()` returned `false` for any caller without `user_id`, before consulting the percentage at all. Desktop clients (MeedyaDL, MeedyaConverter, MeedyaManager) never send `user_id`, so a 50% rollout was silently removing the feature from 100% of them — no error, no failing test, the admin UI and audit log both looked correct. Percentage was the sole outlier: deny-list/allow-list/segment checks are each guarded by a presence test and already fall through to enabled on missing context. Fixed to follow the same fail-open rule; an allow-list-only strategy deliberately keeps failing open too (an allow-list says "these are in", not "everyone else is out" — that needs a deny-list or a disable rule).

**Findings worth preserving from this leg (all verified in code, not inferred):**
1. Four URL forms, one deployment — see the `0f70813`/`b057b80` bullets above. `RewriteBase` is deliberately absent from `web/.htaccess`; hardcoding `/service/` breaks the subdomain form. Unverified against the real host: Apache's per-directory base inference can mis-resolve through a symlinked directory, and the documented fallback lives in the `.htaccess` itself.
2. The wire shape is correct against the API source and parses in tests, but **no request has ever reached a running instance of the API** — still true after this leg. Because fetch failure is silent by design (see `project_remote_feature_control.md`'s silent-failure contract), a remaining mismatch will present as nothing happening, not a visible error — diagnose via the Activity Log line `"Feature availability refresh failed — keeping last known status"`.

**Open — maintainer decisions (none blocking, all recorded as assumptions):**
- Server-side flag evaluation (Decision B, above) is still unratified and is now load-bearing for enforcement. `TERMS.md` already carries the public data commitment that follows from it (app version, OS type/version, CPU architecture; never an install identifier, account, locale or settings data).
- ~~Canonical URL form per brand~~ — **DECIDED 2026-07-28**: client applications connect via the **subdomain** form, `https://service.api.<domain>/` (app served at domain root) — not the path form, `https://api.<domain>/service/` (same files under a `/service/` segment). The path form stays valid/reachable for the identical deployment (useful for browser/admin access) but is not what apps are built against. Brand mapping: Meedya-branded apps (MeedyaDL, MeedyaConverter, MeedyaManager, MeedyaPlayer, MeedyaSubtitler) → `https://service.api.meedyasuite.com/`; all other apps (CueRCode, Go2My.Link, etc.) → `https://service.api.mwbm.io/`. Full technical rationale (root-mount vs `/service/`-segment mount, why the root-mounted form makes the `.htaccess` `RewriteBase` caveat secondary rather than primary) in `.claude/memory/project_remote_feature_control.md`'s 2026-07-28 dated append. No application code changes — this is a build-secret VALUE, injected via `option_env!("INTAPPS_BASE_URL")` in MeedyaDL.
- Whether `api.mwbmpartners.ltd` redirects to the new hosts or retires — currently treated as plain replacement.

**Open — provisioning (maintainer, no code):** four DNS records and four TLS certificates (all four hostnames — both subdomain and path forms, both brand domains — still need records and certificates even though only the subdomain form is what apps target); PHP 8.4 selected per hostname; each app registered with its exact `user_agent_prefix` (e.g. `"MeedyaDL/"`) — a wrong prefix 403s every request silently; keys minted; the three `INTAPPS_*` build secrets set per app repo. **MeedyaDL's `INTAPPS_BASE_URL` build secret is `https://service.api.meedyasuite.com/`** (subdomain form, per the decision above) because it is a Meedya-branded app — not `mwbm.io`, and not the `/service/` path form.

**Open — not built:**
- A suite client-integration doc (`docs/CLIENT_INTEGRATION.md`) belongs in the private IntAppsAPI repo, capturing the language-neutral contract for MeedyaConverter (Swift), MeedyaManager (Rust), CueRCode and Go2My.Link. Those four repos are now attached to the session but sit on `main` with no feature-availability work started.
- `notice.url` is parsed but deliberately never rendered — needs URL scheme validation first.
- `Feature::applySchedules()` invalidates with `deletePattern('features:*')`, which does NOT reach `feature_rules:app:*` — harmless today (a schedule flip changes no rules) but a trap if that changes.
- The dormant interim `service_status` transport (model, commands, store, banner) is superseded and should be removed.

**Verified before this checkpoint:** `npm run type-check` clean; the doc-confidentiality grep (`api\.meedyasuite\.com|api\.mwbm\.io|api\.mwbmpartners\.ltd|service\.api\.|X-App-ID|X-API-Key|/v1/features` across README/TERMS/SECURITY/`help/`) prints nothing; `grep -c "label: '" src/components/help/HelpViewer.tsx` = 15.

### To resume

Read this subsection top-to-bottom, then `.claude/memory/project_remote_feature_control.md` (privacy wording was just rewritten there — read the current text, not memory of the old client-side sentence). The flags client, its enforcement layer, and the bare-`spotify:` classification fix are now all shipped (see the "Session continued" subsections above) — do not assume any is still pending. Do not start IntAppsAPI #108 before #109 (SemVer must be fixed first or version-targeting rules will misbehave on every alpha build) — #109 is now landed per the sibling-repo log above, so re-verify before assuming #108 is still blocked. Decisions A/B/C above are assumptions, not sign-off — flag them to the maintainer, since B (server-side evaluation) is now enforcement-load-bearing rather than display-only. Nothing in this programme has ever been run against a live server; the API's DNS/TLS/provisioning state is the long pole, not code.

---

## ★★ LATEST — Session 2026-07-24: alpha↔main REALIGNMENT (EPIC #1040, PHASES 1–2 DONE)

**Goal:** clean up alpha↔main drift without losing work, then bundle-ID change, then full issue-sweep + docs refresh + vision analysis. Autonomous run; Phase 3 gated on owner go-ahead.

**Model tiering (owner-mandated):** deep analysis/planning/orchestration = **sequential (not parallel) Fable 5** (fall back to Opus if Fable unavailable, but retry Fable next time); implementation = **Sonnet/Haiku** (Opus only if unavoidable). GIRFT.

### The pivotal finding — "alpha 681 behind main" is a git-ancestry ILLUSION
alpha forked from main 2026-04-20 and never merged back, but absorbed main's content via squash-imports (`674967f` #854 ≡ main v1.9.4 tree; #877 rclone; #967 API half of v1.10.1). `git cherry` = 0 patch-id matches → commit-counting lies. **Content probe: of main's 130 substantive commits, 119 present + 9 superseded + 1 N/A + 1 partial (#947). ALL critical fixes verified present in alpha.** Evidence: `.github/audits/alpha-main-drift-content-analysis-2026-07-24.md`. Runbook: `.github/audits/alpha-main-realignment-runbook-2026-07-24.md`.

**RESULT: Phases 1 and 2 are COMPLETE and merged.** alpha is content-complete/reconciled at **1.12.0-alpha.35**, validated by the full CI matrix.

### THREE HARD WARNINGS (still apply — Phase 3/4 not yet run)
1. **NEVER run `realign-alpha`** — clobbers alpha's unique commits (Spotify, #911 UI, Profile Bundle, Lyricsfile, SQLite index, GAMDL 3.6–3.8.4, brand).
2. **NEVER let a naive `git merge main` land** — silently resurrects 5 deleted files (nightly/weekly/monthly-release.yml, upstream-gamdl-watch.yml, protected-cron-channels.json). Phase 3 has an explicit re-deletion guard.
3. Missing **#944 concurrency guard** was a live release-race on alpha — fixed in Phase 1.

### Phase status
- **Phase 0 ✅** backups `backup/{alpha,prep}-pre-realign-2026-07-24` + `backup/prep-pre-rebase-2026-07-24` (branches — proxy blocks tag/delete pushes but ALLOWS commit + force-with-lease pushes to any branch); port branch `port/main-v1.10.1-fragments`.
- **Phase 1 ✅ DONE — PR #1041 MERGED** into alpha (rebase). 9 commits, F1–F13 fragments. All gates green. alpha auto-cut 1.11.0-alpha.31.
- **Phase 2 ✅ DONE — merged via PR #1044** into alpha (rebase; bundled with the ci.yml gate below, commit `be441fc7`). 64 commits (60 prep + audit docs + 2 CI-rot clippy fixes). Rebased prep onto Phase-1-reconciled alpha (1 conflict stop = version stamps; download_queue.rs/docs auto-merged disjoint). `cargo test --lib` 1516/0, `clippy --all-targets` clean, `npm test` 560. **alpha reached 1.12.0-alpha.32, content-complete** (fragments + prep GAMDL 3.8.2–3.8.4 + #1034 security + docs all present & verified).
- **Post-Phase-2 hardening — DONE, also merged to alpha this session:**
  - **#1041** (Phase 1 fragments — see above).
  - **#1044** — the Phase-2 prep rebase merge, bundled with `ci.yml` now gating alpha/beta/rc PRs with the full build/test matrix (previously main-only; alpha PRs got only actionlint/static-security/pr-security).
  - **#1047** — `pr-security.yml` heuristic false-positive refinement (comments, inline `cfg(test)`, `// SAFETY:` no longer flagged).
  - **#1048** — supply-chain hardening: **#995 closed** (channel release workflows pin the lockfile to the `meedyadl` package instead of a full re-resolution) + **#984 partial** (bundled-GAMDL pin to the tested ceiling + mirror-tool integrity verification; cross-repo mirror checksum still open, see below).
  - **#1049** — ELI5 release-notes self-heal gate (prerelease bodies that regress to commit-speak now auto-repair) + backfill of curated notes for alpha.30–32.
  - alpha now sits at **1.12.0-alpha.35**.
- **Phase 3 ⛔ GATED — awaiting owner go-ahead.** Ancestry closure: content-no-op `-s ours` merge of `origin/main` into alpha (runbook §4) to restore honest merge-base + kill the "681 behind" illusion. Zero content change; effectively one-way (revert poisons future merges); resurrection guard mandatory. Will also carry #1044/#1047/#1048/#1049's fixes forward to main. NOT urgent — content already reconciled; safe to defer.
- **Phase 4 ⏳** promotion alpha→beta→main at next stable cut (human-led).

### STILL OPEN
- **#984** — cross-repo mirror checksum verification (only the tested-ceiling pin + local mirror-tool integrity landed in #1048; the cross-repo checksum half remains).
- **#1046** — historical release-body backfill for older tags; needs a maintainer to run `scripts/release-notes/apply-notes.sh` (the mechanism is built and self-healing; only the manual backfill invocation is outstanding).
- Bundle ID → `com.meedyasuite.meedyadl` (owner-confirmed, queued for after cleanup).
- Full GitHub issue sweep (open+closed) + refresh of remaining `.claude/` docs.
- Branch cleanup (backup branches, stale prep branches) once Phase 3/4 land.

### Decisions locked
aria-label = main's #945 form; Phase-1 merge = rebase-merge; bundle-ID last; env has GTK/webkit installed (cargo builds locally); **delivery via `git push` works for any branch** (proxy only 403s tag-push + branch-delete), GitHub MCP API also works for branches/PRs.

### To resume
Phases 1–2 are DONE (merged to alpha; content-complete at 1.12.0-alpha.35, full CI matrix green). Remaining: (a) **Phase 3** ancestry closure — awaiting owner go-ahead (runbook §4, Approach A `-s ours` + resurrection guard); (b) **bundle ID → com.meedyasuite.meedyadl** after cleanup; (c) **#984** cross-repo mirror checksum (remaining half); (d) **#1046** historical release-body backfill (maintainer-run `apply-notes.sh`); (e) Fable issue-sweep + docs refresh; (f) Fable vision analysis → next steps + enhancements; (g) STANDING pr-security monitoring. Read EPIC #1040 for live state. Do NOT run realign-alpha; do NOT naive-merge main. Rollback anchors if anything needs undoing: `backup/{alpha,prep}-pre-realign-2026-07-24`, `backup/prep-pre-rebase-2026-07-24`.

---

## ★ LATEST — Session 2026-07-19 part 2: large autonomous program (IN PROGRESS)

A big multi-workstream autonomous run before the prep→alpha PR. **Model tiering in force:** sequential Fable 5 for deep analysis (fallback Opus); Sonnet/Haiku for implementation (Opus for complex). After each chunk: update issue + commit + update this handoff. **PR still NOT opened** — hold until the program's final step (owner: "STAGE COMMIT PUSH" is the last step).

### DONE this part (all committed + pushed)
- **Org-wide actionlint CI rollout — DONE.** `.github/workflows/lint.yml` (actionlint, `SHELLCHECK_OPTS=--severity=error` so cosmetic style never fails) rolled out to **17 repos** across MeedyaSuite / MWBMPartners / Skriptey / Salem874 via PRs (alpha→beta→main priority); all 17 actionlint checks green, no deploys fired, tracking issue in each. **3 held repos fixed** (real bugs): Go2My.Link `if:false`→`vars.SFTP_DEPLOY_ENABLED` gate (#154→PR #155), WebMS-Intra broken `release.yml` heredoc + empty choice option + `head_commit.message` injection (#366→PR #367), iHymns empty option + injection (#1563→PR #1564). **2 main-targeted:** MeedyaSuite-core (#63→#64), NetPLAYERapp (#180→#181). Reusable tooling in scratchpad: `lint.yml` + `rollout-actionlint.sh` (idempotent, DRY_RUN=1). Not yet committed to a durable home (offer: MeedyaDL-Tools).
- **Rebase — DONE.** Rebased `prep` onto `origin/alpha` (was 52 ahead / 5 behind → **52 ahead / 0 behind**). Kept the session's `1.12.0-alpha.28` (resolved 4 version-manifest conflicts to prep's side, preserved alpha's dep bumps). `cargo check` clean. Force-pushed; **backup at `backup/prep-pre-rebase-2026-07-19`**. NOTE: next alpha auto-computes `1.12.0-alpha.30` (base from manifest 1.12.0 + counter max-tag+1) — verified; nothing to do.
- **Security secret-scanning — DONE (#1032, `9f6fcc61`).** 2 "Generic" alerts = false positives, neutralised at source: JWT test key assembled via `format!("-----BEGIN {pem_kind}-----…")` (no contiguous PEM literal); `DEV_ACCESS_HASH` → `Option<&str>` with runtime `SHA-256("")` fallback (no hash literal). `.github/secret_scanning.yml` (paths-ignore help/**). CodeQL: **0 open** (16 fixed, 1 dismissed test-cookie); UI warning is staleness (last scan `main` Mar 20). **CAVEAT:** the secret-scanning alerts API returns `[]` with a classic `repo` token — couldn't enumerate/dismiss the exact 2; source fixes should auto-resolve on the scanned branch, else a 1-click UI dismiss closes them.
- **Release-notes cumulative-template leak — DONE (#1033, `72e7dbfe`).** The cumulative template had a Tera `set` (loop-local) vs `set_global` bug → it ALWAYS rendered raw commit subjects, ignoring `Release-Note:` trailers (would leak commit-speak + internal method names even after merge). Rewrote `.github/cliff-cumulative-body.tera`: untrailered commits collapse to one "Under the hood: N internal changes ([#PR]…)" line; trailers bucket into What's new/fixed/Performance/Notes; PR-number links kept. Validated over `v1.10.1..v1.11.0-alpha.29` (zero commit-speak). **Backfilled v1.11.0-alpha.29** live (was raw git-cliff — #1028 isn't on `alpha` yet, that's WHY it leaked).
- **Full-codebase SECURITY AUDIT + fixes — DONE (#1034, `0881cd1e`).** Fable audit (Opus-verified). Subprocess/INI/SQL/XSS-CSP/updater/keychain surfaces are solid; risk was in the IMPORT surfaces. Fixed F1 (HIGH — profile-bundle import RCE: no longer anchors FS writes / tool-exec paths to attacker `settings.json`; pre-import snapshot + `sanitize_imported_settings` + clamp security fields), F2 (HIGH — Apple-credential exfil via attacker `wrapper_url`: import preserves `wrapper_url`/`wrapper_decrypt_ip`; `wrapper_sign_in` refuses non-loopback/non-private hosts, DNS-resolved fail-closed), F3 (dev-access bypass — `save_settings`/import can't set `dev_access_enabled`), F4 (bundle export scrubs settings), F5 (0600 cookies/queue/history + settings temp), F6 (redact `user:pass@`), F7 (redact username paths in crash issue), F8 (bundle DoS clamps), F11 (--locked comment). `cargo test --lib` 1516 pass. **Owner decisions RESOLVED + done (`ebe239da`):** F9 → devtools gated behind non-default `devtools` cargo feature (release omits it; `--features devtools` for dev/alpha); F10 → embedded MusicKit token tier removed (resolution now user-JWT → web-player keychain only). All 11 audit findings addressed.

### Fable PLAN A — pre-merge improvement batch (analysed, ready to implement)
Bundle into commits: **A** clippy `useless_borrows_in_formatting` at `download_queue.rs:7691` + delete dead `fetch_syllable_lyrics` IPC (#1012, `commands/gamdl.rs:2878`+`lib.rs:1221`, zero callers). **B** #995 channel builds run `cargo generate-lockfile` (re-resolves whole tree → ships untested deps; `alpha/beta/rc-release.yml`) → `cargo update -p meedyadl`; #984 pin offline-installer GAMDL (`release.yml:794`, parse `tool-versions.toml` → `gamdl>=3.0,<=3.8.4`). **C** #1011 add `extend=…,audioTraits` at `apple_music_api.rs:1042` (dead tag path). **D** docs: #949 M8/M9/M10 renumber (`engine_runner.rs:447/469`, `types/index.ts:936`, `HelpViewer.tsx:982/1001/1019`, `help/supported-services.md`, `settings.rs:589`; canonical M8=BBC=v2.0.0, M9=Spotify=v2.1.0, M10=YouTube=v2.2.0) + CLAUDE.md staleness (meedya-core dep-flip already done `Cargo.toml:414`; nightly/weekly/monthly cron removed by #879) + #998 CSP comment. **E** #982 GPAC NSIS `/D=` → `raw_arg` (`dependency_manager.rs:1657`, needs `rustup target add x86_64-pc-windows-msvc` + `cargo check --target`) + #997 sudo-no-TTY message (`dependency_manager.rs:1729`). **F** #981 Linux x64 FFmpeg tar.xz declared TarGz (`dependency_manager.rs:782`, `archive.rs:402`) → add `TarXz` + `lzma-rs`/`xz2` dep (run cargo-deny). **G** #965 codec "(Experimental)" labels → "(May require wrapper)" (`gamdl_options.rs:262`). **H** #991 honest batch/undo accounting (`downloadStore.ts:984/591/697`). **I** #964 help wrapper phrasing 3.8+. **Defer:** #1000, #987, #963/#1002, #983/#978, #1014, #934, #961 items 1&3. **Housekeeping:** close shipped-but-open #925/#962/#999.

### Fable PLAN B — release-notes robustness remainder (#1033)
Body-lint tripwire in `release.yml ensure-release` (strip `<details>`, grep banned shapes `^## \[` / `**(scope)**` / conventional prefixes / git-cliff footer → stable=`exit 1`, prerelease=degrade to safe static body + `::warning::`) at ~L424 + tier-1 path L216-227; fix missing preamble at `release.yml:361-368`; quality-lint trailers+notes-file in `release-note-gate.yml` (reject backticks/snake_case/`--flags`/paths/jargon); backfill **v1.10.0-alpha.16** (placeholder), **v1.1.0/v1.1.1/v1.2.0/v1.4.0** (commit-speak); optional `baseline-1.11.0-alpha.md`; subjects-only `<details>` via new `cliff-technical-body.tera`; ~145 v0.x archive = scripted one-liner or defer. Validated `cumulative-v3.tera` in scratchpad. Owner defaults chosen: subjects-only details, adopt baseline, defer v0.x, gate chore PRs advisory.

### REMAINING ROADMAP (user-queued, priority order — resume here)
1. ~~**Full codebase security audit** → DONE (#1034, all 11 findings fixed incl. F9/F10).~~
2. **Full lint/syntax sweep** (`cargo clippy`, actionlint, shellcheck, eslint, `type-check`, rustfmt touched-only) → fix all.
3. **Accessibility** scan + fixes (WCAG / standards-compliant) across React UI.
4. **Improvement batch** (Plan A above).
5. **Release-notes robustness** remainder (Plan B / #1033).
6. **All repo .md** to current state: README, SECURITY, CHANGELOG, CONTRIBUTING, ACKNOWLEDGEMENTS, DEV_NOTES, TERMS, CODE_OF_CONDUCT, THIRD_PARTY_LICENSES, LICENSE.
7. **In-app help/*.md** → plain-speak, accurate to current app.
8. **Issue hygiene** — close completed / reopen closed-in-error / update all to reflect state.
9. **Claude** memory + CLAUDE.md; **GitHub Wiki + Project + Milestones** to current state.
10. **FINAL:** handoff refresh (last step) → STAGE/COMMIT/PUSH. **Then** owner opens the prep→alpha PR (rebase-merge, per #1027).

Tooling installed + persists: Rust 1.97.1, Node v26.5.0, git-cliff 2.13.1, ripgrep 15.2.0, actionlint 1.7.12, shellcheck, cargo-deny 0.20.2, jq. `gh` token lacks `project` scope (item-add fails; `gh auth refresh -s project`) and secret-scanning alert enumeration.

---

## 0. Current session (2026-07-18) — IN PROGRESS

Picking up after the GAMDL 3.8.2 admission (§2–§3). Agenda + status:

1. **Git hygiene — DONE (`d0790557`).** Working tree showed 722 phantom
   `100644→100755` executable-bit flips (accidental repo-wide `chmod`; committed
   mode `644`, zero content delta). Neutralised via `git config core.fileMode
   false` (local, non-destructive). `.claude/settings.local.json` now gitignored.
2. **Python detection UX — DONE (#1017, `7fb07ed7` + help `dddbf4d9`).** Setup
   wizard now DETECTS a compatible system Python (PATH / Homebrew / python.org /
   pyenv / Windows `py`; floor 3.10) and REUSES it by building a venv at
   `{app_data}/python/` (PEP-668-safe), keeping the portable download as the
   fallback. Venv-aware `platform::resolve_managed_python_binary` (Windows
   `Scripts/`); provenance marker suppresses the portable "update" nag for
   system-venv installs. 8 unit tests. See CLAUDE.md "System-Python reuse".
3. **GAMDL v3.8.4 admission — DONE (#1018, `59dbaefa`).** ADMITTED, ceiling
   3.8.2 → 3.8.4 (covers 3.8.3). ZERO-code-change bump: the whole 3.8.2→3.8.4
   delta has no Python-source change (pyo3 0.27 for Py3.14 sdist builds + CI
   wheels + one `_ammuxer` Rust fix `e4887d34` for corrupted song endings on
   wrapper-decrypt — a bug in 3.8.2/3.8.3). Wheels re-verified live (5× cp310-abi3,
   no ARMv7). Audit `.github/audits/gamdl-v3.8.3-v3.8.4-audit.md`. **3.8.4 fixes
   a data-corruption bug in the currently-recommended 3.8.2 → real user win.**
4. **GAMDL open-issues mitigation sweep — DONE.** Surveyed all 12 open upstream
   GAMDL issues (Fable-5 agent, Opus-validated). 7 mitigable, 5 rejected (3
   because MeedyaDL already handles them). Shipped 5 mitigations + docs:
   - **#1019** (`89dedb2d`, `c538c247`) — rate-limit guard (seed gamdl#306): a
     429 now pauses the queue (stops the serial cascade extending Apple's ban),
     skips the error report + companions + wrapper auto-retry, and gets honest
     "hours not minutes" guidance. Folds gamdl#307 (`license_declined`).
   - **#1020** (`e977d509`) — wrapper-v2 `playback_ready=false` preflight warning
     + `wrapper_decrypt_unavailable` classifier for the 503 (gamdl#319).
   - **#1021** (`b25a546c`) — silent-corruption guard: warn when both probes
     find no audio stream (gamdl#328); #847-safe (result-keyed, not stderr).
   - **#1022** (`1a652cc4`) — transient `[Errno 13]` on GAMDL temp files →
     retriable `io_transient` + AV-exclusion guidance (gamdl#323).
   - **#1023** (`f604e91e`) — correct multi-artist Album Artist (`aART`) from the
     catalog `artistName` (gamdl#326).
   - **#1024** (`c538c247`) — troubleshooting note for the save-playlist
     first-track bug (gamdl#322, docs-only).
5. **Minor version bump — DONE (`35617b99`).** 1.11.0 → **1.12.0-alpha.28** across
   all 5 manifests (package.json, package-lock, tauri.conf.json, Cargo.toml,
   Cargo.lock). Reflects the session's feature work; stays on the alpha channel
   (counter 27→28, no reset). `bump-version.mjs` handles 4 files; package-lock
   synced via `npm install --package-lock-only`.
6. **Backlog triage + fix batch — DONE.** Fable-5-triaged the 83 open issues into
   a validated batch (security + self-contained correctness), Opus-gated each,
   implemented via **sequential Sonnet agents** (each `cargo test`/`type-check`-
   verified locally). Shipped **13 fixes + closed 2**:
   - **Security:** #975 (`0d71deaa`, credential-redact tracebacks before crash
     reports / GitHub issues), #985 (`ef13bd0a`, validate pip package name +
     engine-registry allowlist), #977 (`26f403db`, release.yml tag-shape
     validation → block shell-injection / secret exfil), #988 (`ad6e0e9b`, pin
     cargo-binstall + git-cliff in the contents:write job).
   - **Correctness/perf:** #980 (`d7f9a3f2`, MV cover sidecar dotted-stem),
     #989 + #990 (`d8a00067`, stop `Box::leak`ing manifest keys + detect hidden
     animated art on Linux), #1010 (`87d4b78a`, expire stale web-player token),
     #994 (`b09e40fd`, cache `humanise_codec_skip_line` regexes), #992
     (`2238f2d4`, History "Open Folder" reveals the album dir not the parent),
     #996 (`4b8601d6`, tool reinstall stage-and-swap — failed upgrade keeps the
     old binary).
   - **Frontend:** #993 (`4622864c`, dedup native OS notifications).
   - **Closed as already-fixed:** #951 (console.debug), #950 (QueueItem aria).
7. Per-unit: GitHub issue (create/update) + individual commit + push. **No PR.**

### ✅ VERIFIED LOCALLY (toolchain installed mid-session)

The device had no Rust/Node toolchain (fresh clone). Installed **rustup (Rust
1.97.1 stable)** + **Node v26.5.0** (both persist: `~/.cargo` + Homebrew), then:

- **`cargo test --lib` → 1504 passed, 0 failed, 1 ignored** (baseline 1465; the
  ~39 new tests are this session's — Python detection, the GAMDL-mitigation
  classifiers, and the backlog batch). Everything compiles clean — incl. the two
  new `download_queue.rs` match arms (`rate_limit`, `io_transient`), the
  stage-and-swap in `dependency_manager.rs`, and all `process.rs` /
  `python_manager.rs` / `crash_report_service.rs` additions.
- **`npm run type-check` → clean** and **`npm run test` (vitest) → 560/560**
  (incl. the 4 new #993 notification-dedup tests).
- The full run surfaced ONE pre-existing flaky test (`config_service::
  ini_includes_wrapper_when_enabled`, unrelated) — fixed under **#1025**
  (`03c8e393`, added a `VersionGuard`) + closed. `package-lock.json` version
  staleness synced (`99cb69ba`, then again in the 1.12.0 bump).
- **Known non-blocker:** a PRE-EXISTING clippy lint `useless_borrows_in_formatting`
  at `download_queue.rs:7691` (untouched this session) — a 1-line fix if CI
  clippy-gates; left for a future pass.

So the whole session is now **locally verified**, not just CI-gated. Future
sessions on this machine have the toolchain; the standard cheatsheet in §7 works.

### Deferred / next (from the backlog triage — not yet done)

Fable-5 validated but NOT implemented this session (good next-session targets):
`#981` (Linux x64 FFmpeg tar.xz declared TarGz — needs a `TarXz` archive format +
`lzma-rs` dep; **Opus**), `#982` (GPAC NSIS `/D=` quoting for spaced Windows
usernames — needs `cargo check --target x86_64-pc-windows-msvc`), `#1011`
(`extend=audioTraits` — static + additive but wants a live confirm), `#949`
(reduced — M8/M9/M10 numbering still wrong in `engine_runner.rs:448/470`,
`types/index.ts:936`, `HelpViewer.tsx`, `help/supported-services.md`), `#984`
(offline-installer pip-pin the gamdl spec from `tool-versions.toml`), `#987`
(tool checksum verification — needs mirror-published hashes), `#983`/`#991`/`#997`/
`#998` (need design/UX decisions first). Full validated specs + the "explicitly
dropped" list are in the triage record (this session's second Fable-5 agent).

Model tiering: Fable 5 (sequential, one at a time) for deep analysis → fallback
Opus; Sonnet/Haiku for implementation; Opus for the hardest. Push IS authorised
this session; still **no PR**.

**Toolchain (installed mid-session, persists):** Rust 1.97.1 (rustup, `~/.cargo`)
+ Node v26.5.0 (Homebrew) + git-cliff 2.13.1 (Homebrew). So `cargo test --lib`
(1504 pass), `npm run type-check` / `npm run test` (560 pass), and local
git-cliff dry-runs all work now. (`gh` token still lacks `read:project` — issues
are filed/updated but not added to project 6.)

### Session continued — wrapper-v1 audit + release-notes fix

8. **GAMDL 3.0–3.5.x wrapper-v1 audit — DONE (owner request).** Verified the
   support window `[3.0, 3.8.4]`, all capability gates (WrapperUrl≥3.6 → v1 for
   ≤3.5.x, WrapperM3u8Ip 3.1–3.5.x, WrapperDecryptHostPort≥3.8.2, NativeMuxing/
   AacWebCodecRename≥3.6), CLI emission (all three v1 sockets), and preflights
   are correct for wrapper-v1. One gap fixed: **#1026** (`701430dc`) — the
   wrapper-v1 INI branch was missing `wrapper_decrypt_ip` (CLI had it; #743/#744
   never added the INI twin). Low-impact (CLI is authoritative) but closes the
   inconsistency for remote/LAN wrapper-v1.
9. **Prerelease release-notes fix — DONE (owner report: alpha releases list no
   changes). #1027** (`3d4b8b6c` + docs `b1a7dd9f`). Root cause: cliff.toml
   didn't skip the `chore(alpha): X.Y.Z-alpha.N` version-bump commit (rendered as
   `🧹 Maintenance — (alpha) X` noise), and a dep-only alpha had nothing else.
   Fix (Fable-5-designed, Opus-validated empirically with git-cliff 2.13.1):
   cliff.toml skips the machine pure-bump shape (+ `chore(version)` + internal
   `docs(handoff)`) while KEEPING human `chore(alpha)` housekeeping; release.yml
   prerelease tier-2 assembles **"New in this build"** (full) + compact
   **"All changes since <last stable>"** (`.github/cliff-cumulative-body.tera`
   + `--ignore-tags` collapse). **Backfilled `v1.11.0-alpha.19…28`** via
   `gh release edit` (footers preserved; .27/.28 are genuinely deps-only →
   honest preamble + cumulative). **⚠ Merge caveat:** rebase-merge (not squash)
   the prep→alpha PR so the ~30 conventional commits group individually in the
   next alpha's notes.

### Session continued (2026-07-19) — ELI5 release notes (all) + wrapper sign-in

10. **ELI5 release notes — durable process + full backfill — DONE (#1028 +
    backfill).** Owner: notes were still too generic/technical; wanted plain
    "ELI5" language on EVERY release and a mechanism so it happens on every
    future one. Two parts:
    - **Process (#1028, `6242eb7f`, pushed + validated).** A `Release-Note:`
      git-trailer convention is the single source of truth. `.github/cliff-eli5-body.tera`
      renders trailers into four sections (What's new / What's fixed / Performance
      / Notes); `release.yml` `ensure-release` is ELI5-first (technical changelog
      demoted into `<details>`), with a tier-1 curated-file OVERWRITE branch so
      `.github/release-notes/<TAG>.md` self-heals a live body. Guardrails:
      `.github/workflows/release-note-gate.yml` (PR gate — feat/fix/perf PRs need a
      `Release-Note:` line; release-please PR needs a `v<ver>.md` notes file),
      `STYLE_GUIDE.md` (hard bans + code→user glossary), `scripts/release-notes/
      apply-notes.sh` (applies a curated file to a live release, PRESERVING the
      "Choose your download" footer) + `draft-notes.sh`. Empirically validated
      with git-cliff 2.13.1.
    - **Backfill (`0eb6f97c`, pushed).** Rewrote **all 24 historical release
      bodies** — 14 stables (v1.0.0–v1.10.1) + the v1.11.0-alpha.19–28 line — into
      the four-section ELI5 format, authored by two sequential Sonnet agents
      (Opus-gate-reviewed), applied live via `apply-notes.sh` (footers preserved),
      and committed under `.github/release-notes/` so they self-heal. One
      correctness catch: v1.4.1 had been mis-attributed a big feature list via a
      phantom `v1.3.2` tag polluting the GitHub compare link — corrected to its
      true 4-commit delta (the feature set belonged to v1.3.0). Deps-only releases
      use the style guide's one-line housekeeping fallback.
    - **⚠ Next time:** just add a `Release-Note: <plain-english line>` trailer to
      each feat/fix/perf PR body (the squash-merge carries it into the commit
      footer). The gate + template do the rest. `apply-notes.sh <TAG>` is the tool
      to fix any already-published body.
11. **Wrapper-v2 in-app sign-in — DONE (#1029, 3 commits).** Owner asked how
    MeedyaDL can authenticate the wrapper (hoped: macOS-Keychain auto-detect).
    Sequential Fable-5 investigation (source-verified vs wrapper-v2 `100e0a8` +
    our Docker image `b3ffb1b`) **refuted the premise**: wrapper-v2 is a
    self-authenticating daemon — it runs Apple's own sign-in and mints its own
    tokens; its ONLY input is `POST /login` (Apple ID + app-specific password) →
    `POST /login/2fa`. It accepts **no** cookie / Music-User-Token / Keychain
    handoff, and Apple auth is **not** surfaced via the GAMDL CLI. Keychain
    auto-detect is a dead end twice over (the daemon needs the *password* not a
    token; and Apple's media-token keychain items sit behind `com.apple.private.*`
    access-group ACLs no third-party app can read). Owner chose "build the full
    modal." Shipped:
    - `26258045` — doc fix: `WrapperV2AuthBlock`/`WrapperV2Me` now list the
      source-verified five states (`logged_out | in_progress | awaiting_2fa |
      authenticated | failed`); old comment had a fictional `"logging_in"`.
    - `7f5262e8` — **backend** (Opus): `health_check_service::WrapperV2LoginResult` +
      `wrapper_v2_login`/`_submit_2fa`/`_logout` (shared 60s POST driver, verified
      HTTP-code mapping, never logs the body); `commands/wrapper.rs` (new) IPC
      `wrapper_sign_in` (5/min) / `wrapper_submit_2fa` (10/min) / `wrapper_sign_out`
      / `wrapper_auth_status`, registered in `mod.rs` + `lib.rs`.
    - `144f739d` — **frontend** (Sonnet, gate-reviewed): Settings › Advanced ›
      Wrapper (v2) "Sign in to wrapper" button + status line + Sign-out, two-step
      modal (Apple ID + app-specific password → 6-digit 2FA). Password/code live
      only in component state, wiped on every close path. `WrapperV2LoginResult`
      type + 4 `tauri-commands.ts` wrappers.
    - **Verified:** `cargo test --lib` → **1504 pass, 0 fail** (no regression),
      `cargo check` clean, `npm run type-check` clean, **560/560** frontend tests.
    - #1029 left OPEN with a completion comment — the `Closes #1029` trailer
      auto-closes it when the prep branch merges. Interim fallback (the
      `wrapper-account.sh` terminal helper) still works.

---

## 1. Standing constraints (READ FIRST)

- **Do NOT open a PR yet.** Single PR at the end (no stacking / merge-race).
  Keep committing to the working branch; hold the PR until told.
- **Do NOT force-push / reset-hard / modify remotes** without explicit
  instruction. Committing AND pushing to the local working branch IS authorised
  this session (owner instruction 2026-07-18: "STAGE, COMMIT and PUSH … commit &
  PUSH each change individually"). Still **no PR** — hold until told.
- **Model tiering:** Fable 5 for deep planning (sequential, no parallel agents);
  Sonnet/Haiku for implementation; Opus for orchestration/verification.
- Per-unit: detailed GitHub issue + individual commit + security-review the diff.
- **`cargo fmt`:** CI does NOT gate on it; the repo has pre-existing drift.
  rustfmt ONLY your touched files (never whole-crate `cargo fmt`) or it pollutes
  the diff with unrelated reformatting.

---

## 2. This session — 12 commits, all verified (full lib suite 1465/1465, type-check clean)

| Commit | What | Issue |
| --- | --- | --- |
| `bc9e8212` | #969 word-timing keyed on `<span begin>` presence, not the `itunes:timing` label | #969 |
| `abfe689a` | Gap-A: enrichment metadata via 3-tier premium resolver → syllable lyrics on the **web-dev-key** path | #1008 |
| `44acdc88` | Gap-B: attempt syllable fetch for `hasLyrics == None` tracks | #1008 |
| `0e06350e` | #970: shared `apple_music_headers` helper + harden `fetch_music_video_relations` + nested promo extraction | #970 |
| `256e4868` | GAMDL `no_compatible_wheel` guard + pip `--only-binary=gamdl` | #1009 |
| `3f7e0302` | `wrapper_version_mismatch` error classifier + `WrapperV2Me.version` capture | #1009 |
| `a3541c42` | (superseded) v3.8.2 audit doc — original HOLD decision | #1009 |
| `ff0b212c` | handoff refresh | — |
| `b4e4e59e` | housekeeping (meedyadl-v2 deleted, Swagger dropped) | — |
| `a504fcb4` | **wrapper-v2 0.0.2 native TCP decrypt** — `WrapperDecryptHostPort` gate + CLI/INI emission + `wrapper_url` thread + TCP preflight + `/me` version warn + guidance flip | #1009 |
| `e4ea8620` | platform-aware `no_compatible_wheel` (flag only ARMv7) | #1009 |
| `c8823f78` | **admit GAMDL 3.8.2** (ceiling → 3.8.2) + flip all docs hold→admit | #1009 |

---

## 3. GAMDL v3.8.2 — DECISION: **ADMITTED** (ceiling → 3.8.2)

> An initial pass HELD at 3.8.1 on **stale PyPI data**. A cache-busted re-check +
> live `pip download` found 3.8.2 ships **`cp310-abi3`** wheels for **5 of 6**
> platforms (macOS universal2, Win x64/ARM64, Linux x64/aarch64). Only **Linux
> ARMv7** lacks a wheel. Reversed to ADMITTED. Lesson recorded in the cadence memory.

- **abi3 verified genuine:** `import gamdl._ammuxer` + `python -m gamdl --version`
  run on CPython **3.14** (≠ the cp310 tag) ⇒ loads on the bundled 3.12.
- **Per-platform install:** `gamdl>=3.0,<=3.8.2` + `--only-binary=gamdl` →
  3.8.2 on the 5 wheel platforms, **auto-fallback to 3.8.1 on ARMv7**.
- **Wrapper-v2 0.0.2** is the one real code change: decrypt moved from HTTP
  `POST /decrypt` to a native TCP host/port. MeedyaDL now emits
  `--wrapper-decrypt-host`/`--wrapper-decrypt-port` (CLI+INI) from `wrapper_decrypt_ip`
  on 3.8.2+ (`GamdlFeature::WrapperDecryptHostPort`), runs the TCP decrypt
  preflight for wrapper-v2, and warns via a `/me` version preflight. No settings-schema bump.
- Audit doc: `.github/audits/gamdl-v3.8.2-audit.md`.

### ⏳ PRE-STABLE GATE — live smoke-test (NOT done; can't be static)
Before this ceiling reaches **stable**, on each shipping platform:
1. `import gamdl._ammuxer` + a **real song download** decrypts+muxes on the
   bundled cp312 (import verified on 3.14; the decrypt path wants a live run,
   esp. Windows + Linux-arm).
2. Real **wrapper-v2 0.0.2** round-trip — local + remote/LAN (decrypt host/port).
3. Version-skew: GAMDL 3.8.2 + wrapper-v2 ≤ 0.0.1 aborts → the new preflight warns.
4. A **music-video** download (local-key decrypt via ammuxer).
5. `pip install --only-binary=gamdl gamdl==3.8.2` resolves on 5 platforms; range
   falls back to 3.8.1 on ARMv7.

---

## 4. Animated art + syllable lyrics (ITAMenhancer cross-verified)

- Square + portrait art work on both auth paths (#970 hardened header consistency).
- Syllable/word lyrics: #969 (label→span) + Gap-A (web-key path) fixed → work on
  both paths. **⚠ Gap-A needs live validation** with a web-player-only account.
- Follow-ups (ITAM specs added; most need LIVE testing): #971 (MUT on catalog —
  foundation wired), #972 (HLS resolution), #973 (`&l={locale}`), #974 (native
  fMP4 concat), #1010 (web-token expiry — safe, no live test), #1011
  (`extend=audioTraits`), #1012 (dead `fetch_syllable_lyrics` IPC — `for consideration`).

---

## 5. Branch state

Local `main` fast-forwarded; `meedyadl-v2` deleted (owner decision); no stray
feature/prep branches. Local aligned with GitHub (alpha, main, + this branch).

---

## 6. Remaining / deferred

1. **Live smoke-test** of GAMDL 3.8.2 (§3 gate) before the ceiling ships stable.
2. **Live-validation** of the art/lyrics follow-ups (#971–#974, Gap-A).
3. **Open the single PR** (`prep/alpha-gamdl-3.8.2-plus-2026-07-10` → `alpha`)
   when the user says go. Monitor CI, fix as issues appear.
4. Strategy: #1013 retargeted to **ARMv7 wheel only** (upstream already ships
   abi3 for 5/6); #1014 (per-platform window) — no longer urgent (range +
   `--only-binary` handles ARMv7 today).
5. Swagger/OpenAPI — DROPPED (owner decision).

---

## 7. Verification cheatsheet

```bash
cd src-tauri && export PATH="$HOME/.cargo/bin:$PATH"
cargo test --lib                                    # 1465/1465
cargo test --lib services::gamdl_capabilities       # 33/33 (incl. WrapperDecryptHostPort)
cargo test --lib services::update_checker           # 15/15 (platform-aware wheel)
cd .. && npm run type-check                          # clean
# GAMDL 3.8.2 abi3 verification (done, passed on CPython 3.14):
python3 -m venv /tmp/v && /tmp/v/bin/pip install --only-binary=gamdl 'gamdl==3.8.2' \
  && /tmp/v/bin/python -c "import gamdl._ammuxer; print('ok')" \
  && /tmp/v/bin/python -m gamdl --version
```

## 8. Issues this session

Fixed: #969, #970, #1008, #1009 (all the code above). Filed: #1010, #1011,
#1012, #1013 (retargeted ARMv7), #1014. Enriched: #971–#974.

---

## 9. 2026-07-27 — Outbound User-Agent policy corrected (#1070)

Same-day follow-up correction to #1070. The first pass of that work sent
`APP_USER_AGENT` (MeedyaDL's own identity string) to every third party —
GitHub, PyPI, MusicBrainz, Odesli, the Apple Music JWT paths. The maintainer
corrected this: third parties should receive a genuine browser UA instead,
with MusicBrainz as the sole exception (licensed API, ToS requires an
identifying UA).

Landed: `BROWSER_USER_AGENT` (the Safari 17.6 string, unchanged value, moved
from `apple_music_api.rs`'s old `APPLE_BROWSER_USER_AGENT` into
`utils/http_client.rs` as the canonical shared constant) now goes to every
third party except MusicBrainz. `APP_USER_AGENT` is now MusicBrainz-only.
`full_user_agent()` is untouched — still MWBM-IntAppsAPI-only. A single
*fixed* browser UA (not per-platform) is deliberate: every install looks
byte-identical to third parties, leaking neither platform nor app version —
strictly less than the previous policy leaked. `tools/audit-checks/check_user_agent.py`
docstring updated for the three-constant policy; its no-hardcoded-literal
rule is unchanged. `.claude/CLAUDE.md`'s "Outbound User-Agent" bullet
rewritten to match. Reservation on record: GitHub's own guidance frames the
UA as its contact channel for a misbehaving integration rather than a
silent-block trigger, and both the updater and dependency downloader hit
GitHub — accepted, revisit if GitHub calls start failing.

Full verify: `cargo check` / `cargo clippy --all-targets -- -D warnings` /
`cargo test` all clean (1575 passed); `check_user_agent.py --strict` and
`--self-test` both 0 findings / all cases pass; `check_ipc_commands.py
--strict` and `check_codec_registry.py` both clean.

## 10. 2026-07-27 — Outbound User-Agent policy revised again: four groups (#1070)

Same-day second follow-up. §9's two-constant correction (browser UA to
everyone except MusicBrainz) has itself been refined by the maintainer into
a four-way split — GitHub goes back to identifying itself, and the "browser
UA" third parties get an OS-plausible browser UA instead of one fixed
Safari string.

Landed in `utils/http_client.rs`: `BROWSER_USER_AGENT` renamed to
`SAFARI_MACOS_USER_AGENT` (same Safari 17.6 value, unchanged) and scoped
down to Apple Music endpoints only (`apple_music_api.rs`,
`commands/credentials.rs`, `animated_artwork_service.rs`'s ffmpeg UA) —
always macOS Safari regardless of host OS, since Apple's edges expect it.
New `browser_user_agent() -> &'static str` matches on
`std::env::consts::OS` (macOS → the Safari string; Windows → a Chrome 131
Windows UA; Linux and unknown → a Chrome 131 Linux UA) — deliberately OS
*family* only, no architecture branch, because real Chrome on ARM
Windows/Linux still reports x64/x86_64 anyway. `odesli_service.rs` and
`pip_engine_service.rs` now call this instead of the old fixed string.
GitHub call sites (`update_checker.rs` x6, `dependency_manager.rs` x2) and
`service_status.rs` (first-party `raw.githubusercontent.com/MWBMPartners`)
moved back to `APP_USER_AGENT` — GitHub's guidance wants integrations to
identify themselves, and it's the channel GitHub uses to reach out to a
misbehaving integration rather than silently block it; first-party traffic
has no reason to look like a browser at all. `APP_USER_AGENT`'s doc comment
rewritten for the Group A definition (first-party + GitHub + MusicBrainz).
MusicBrainz (`musicbrainz_service.rs`) and MWBM-IntAppsAPI's
`full_user_agent()` (`feature_flag_service.rs`) are unchanged — already
correct.

`tools/audit-checks/check_user_agent.py` docstring, finding message, and
self-test fixtures updated for the four-constant policy and the
`SAFARI_MACOS_USER_AGENT` rename; scan rule itself (identifier-vs-literal
regex) untouched and still 0 findings. `.claude/CLAUDE.md`'s "Outbound
User-Agent" bullet rewritten for the four-group table. This note
supersedes §9 above — §9's two-constant description is now historical.

Full verify: `cargo check` / `cargo clippy --all-targets -- -D warnings` /
`cargo test` all clean (1577 passed, +2 new `http_client` tests);
`check_user_agent.py --strict` and `--self-test` both clean;
`check_ipc_commands.py --strict` and `check_codec_registry.py` both clean;
`npm run type-check` clean.

## 11. 2026-07-27 — Build-time Chrome UA major-version resolution (#1070 follow-up)

Same-day third follow-up to §9/§10. Group C's Chrome UA strings
(`browser_user_agent()`, Windows + Linux branches) were still hand-pinned
at a literal "131" in Rust source — this note lands the mechanism that
keeps that number current without ever touching the code again by hand.

**Design (settled, no debate left)**: only the Chrome **major version
number** is injected at build time, never a full UA string and never a
per-OS value. `std::env::consts::OS` is compile-time-constant per target
and Chrome's major release train is identical across desktop platforms, so
one OS-agnostic number correctly serves every build target — it is
structurally impossible for a Windows build to ship a macOS (or any other
platform's) UA token, because OS-token selection (a `match` on
`std::env::consts::OS`) and version injection (`option_env!` → a
`LazyLock<String>` format!) are two entirely separate mechanisms that never
cross-contaminate. Windows presents as **Chrome, not Edge** — Chrome's
Windows share is several times Edge's (the less remarkable, more
genuine-looking client), and Edge's UA is Chrome's plus an extra `Edg/`
token, i.e. strictly more identifying and a second version number to keep
in sync for zero benefit.

Landed in `src-tauri/src/utils/http_client.rs`: `CHROME_MAJOR_FALLBACK`
("131", compiled in, hand-refresh periodically), `CHROME_MAJOR`
(`option_env!("MEEDYADL_CHROME_MAJOR")` with fallback), and
`sanitise_chrome_major()` (defence-in-depth — a malformed/garbage env
value silently degrades to the fallback rather than shipping a broken UA
or panicking; 2-4 ASCII-digit check). `WINDOWS_CHROME_UA` / `LINUX_CHROME_UA`
are now `LazyLock<String>` built from the sanitised major, and
`browser_user_agent()` dereferences them instead of returning a literal.
`SAFARI_MACOS_USER_AGENT` is deliberately **left out of this mechanism** —
it's the Group B string Apple Music's own edges must accept, so tying it to
a network fetch is too high a blast radius (a broken/rate-limited fetch
could ship a build Apple's servers reject outright), and Safari's major
moves ~annually versus Chrome's ~4-week train, so the staleness pressure
that motivates resolving Chrome barely applies. **Do not "finish the job"
by making Safari dynamic too** — this was evaluated and rejected, not
overlooked.

`release.yml`'s `build` job gained one step ("Resolve current Chrome major
for browser UA (best-effort)") between "Install npm dependencies" and
"Pre-bundle engines", which queries Google's public VersionHistory API
(`versionhistory.googleapis.com/v1/chrome/platforms/win/channels/stable/versions`)
and exports `MEEDYADL_CHROME_MAJOR` via `$GITHUB_ENV` (propagates to every
later step in the job — no `env:` added to the `cargo tauri build` steps
themselves). **Never-fail contract**: `set +e` + a trailing `exit 0` mean
this step cannot fail the build under any response shape — a failed fetch,
timeout, or malformed body just leaves the env var unset and the compiled
fallback takes over identically to a local dev build. A `>=3-majors-behind`
advisory `::notice::` nudges a manual `CHROME_MAJOR_FALLBACK` bump when the
live value has drifted far from the compiled-in one; this is advisory only,
never blocking. `ci.yml` deliberately does **not** get this step — CI/PR
builds intentionally stay on the fallback for build determinism and cache
hygiene, per explicit design instruction.

Docs: `.claude/CLAUDE.md`'s "Outbound User-Agent" bullet's Group C
paragraph rewritten for the new mechanism (also corrected two facts that
had gone stale: the Chrome UAs are now `LazyLock`, not literals, and the
Windows-vs-Edge rationale is now spelled out). `DEV_NOTES.md` gained a new
"Build-time (non-secret) environment variables" section documenting
`MEEDYADL_CHROME_MAJOR` — explicitly safe to name publicly (unlike
`INTAPPS_*`) since it's a public, unauthenticated Google API and the
resulting value ships in plaintext in every binary's UA header anyway.

Full verify: `cargo test utils::http_client` clean with the env var unset,
set to `140`, and set to the deliberately-invalid `garbage;rm` (all three
pass identically — proves the sanitiser and the fallback both work);
`cargo clippy --all-targets -- -D warnings` clean; full `cargo test` clean
(1580 passed, +3 new `http_client` tests over §10's count);
`check_user_agent.py --strict` and `--self-test` both clean (script
untouched — the new code passes by construction, matching neither scanned
pattern); `check_ipc_commands.py` clean; `npm run type-check` clean. Live
`curl` dry-run of the VersionHistory API from this sandbox succeeded
(returned major 151 at time of writing), confirming the response shape the
workflow step parses.

**Decisions on record for future sessions** — do not relitigate: (1) Safari
stays pinned/manual, never dynamic; (2) Windows uses Chrome, never Edge;
(3) only the major crosses the build boundary, never a full string or a
per-OS value.
