# MeedyaDL — Session Handoff

**Last updated:** 2026-07-10
**Working branch:** `prep/alpha-gamdl-3.8.2-plus-2026-07-10` (off `alpha` @ 1.11.0-alpha.27)

Read top-to-bottom before continuing. Supersedes the earlier 2026-07-03 handoff.

---

## 1. Standing constraints (READ FIRST)

- **Do NOT open a PR yet.** Single PR at the end (no stacking / merge-race).
  Keep committing to the working branch; hold the PR until told.
- **Do NOT push / force-push / reset-hard / modify remotes** without explicit
  instruction. Committing to the local working branch IS authorised this session.
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
