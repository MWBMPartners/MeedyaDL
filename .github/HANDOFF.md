# MeedyaDL — Session Handoff

**Last updated:** 2026-07-18
**Working branch:** `prep/alpha-gamdl-3.8.2-plus-2026-07-10` (off `alpha` @ 1.11.0-alpha.27; **now bumped to 1.12.0-alpha.28**)

Read top-to-bottom before continuing. Supersedes the earlier 2026-07-10 handoff.

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
