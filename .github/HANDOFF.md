# MeedyaDL — Session Handoff

**Last updated:** 2026-07-10
**Working branch:** `prep/alpha-gamdl-3.8.2-plus-2026-07-10` (off `alpha` @ 1.11.0-alpha.27)
**Branch context:** alpha (1.11.0-alpha.27), main (1.10.1), beta (1.9.4)

Read top-to-bottom before continuing. Supersedes the 2026-07-03 handoff (that
session's #967/#968 have since merged; #967 is commit `c8530014` on alpha).

---

## 1. Standing constraints (READ FIRST)

- **Do NOT open a PR yet.** The user is queueing more work and wants a SINGLE
  PR at the end (not stacked PRs — avoids merge-race conditions). Keep
  committing to the working branch; hold the PR until explicitly told.
- **Do NOT push / force-push / reset-hard / modify remotes** without an explicit
  instruction. Committing to the local working branch IS authorised this
  session ("commit individually"). Local branch ops (ff, delete) are fine.
- **Model tiering:** Fable 5 for deep planning (sequential, not parallel — the
  user explicitly said no parallel agents); Sonnet/Haiku for implementation;
  Opus only when necessary. Fable was available this session.
- Per-unit workflow: detailed GitHub issue + individual commit + doc/memory
  update. Security-review each diff before committing.

---

## 2. This session's work (branch `prep/alpha-gamdl-3.8.2-plus-2026-07-10`)

8 commits, all verified (full lib suite **1459/1459**, frontend type-check
clean, no `fmt`/clippy regressions in touched files):

| Commit | What | Issue |
| --- | --- | --- |
| `bc9e8212` | #969 word-timing keyed on `<span begin>` presence, not the `itunes:timing` label (both sites + `ttml_has_word_timing` + 6 tests) | #969 |
| `abfe689a` | Gap-A: enrichment metadata via 3-tier premium resolver → syllable lyrics work on the **web-dev-key** path | #1008 |
| `44acdc88` | Gap-B: attempt syllable fetch for tracks with absent `hasLyrics` flag | #1008 |
| `0e06350e` | #970: shared `apple_music_headers` helper + harden `fetch_music_video_relations` + nested promo-video extraction (MUT param wired `None` for #971) | #970 |
| `256e4868` | G2: `ComponentUpdate.no_compatible_wheel` guard + "Not Installable" badge/disabled button + pip `--only-binary=gamdl` | #1009 |
| `3f7e0302` | G3: `wrapper_version_mismatch` error classifier (both skews) + `WrapperV2Me.version` capture + stale `amdecrypt.py` doc | #1009 |
| `a3541c42` | docs: v3.8.2 audit doc + tool-versions.toml audit trail + CLAUDE.md + memory + README + help/wrapper.md | #1009 |

---

## 3. GAMDL v3.8.2 — DECISION: HOLD at 3.8.1

**v3.8.2 (2026-07-09) is NOT admitted; ceiling stays 3.8.1.** It replaced
Python `amdecrypt.py` with a compiled Rust/PyO3 extension (`gamdl._ammuxer`)
and publishes only a `cp310-cp310-manylinux_2_34_x86_64` wheel + sdist —
MeedyaDL's bundled CPython 3.12.8 (no Rust toolchain) can install it on **zero**
of 6 platforms. Verified against PyPI + source. Full analysis + admission plan:
`.github/audits/gamdl-v3.8.2-audit.md` (issue #1009).
- **Wheel-availability is now a first-class audit gate** (see the cadence memory).
- v3.8.2 also hard-requires **wrapper-v2 0.0.2** (native TCP decrypt); GAMDL and
  wrapper-v2 must be upgraded in lockstep (help/wrapper.md documents this).
- Admission plan (when wheels appear): bump ceiling; add
  `GamdlFeature::WrapperDecryptHostPort` (≥3.8.2) emitting
  `wrapper_decrypt_host`/`wrapper_decrypt_port` from the existing
  `wrapper_decrypt_ip` split; re-enable TCP decrypt preflight for wrapper-v2;
  add `/me` version preflight (uses the newly-captured `WrapperV2Me.version`).

GAMDL v3+ support (v2 dropped; wrapper-v1 for 3.0–3.5, wrapper-v2 for 3.6+) is
verified intact — gates + 32/32 `gamdl_capabilities` tests unchanged.

---

## 4. Animated art + syllable lyrics — state (ITAMenhancer cross-verified)

- **Square + portrait animated art:** works on both auth paths (MusicKit JWT +
  web-dev-key). #970 hardened the header consistency.
- **Syllable/word lyrics:** #969 (label-vs-span) + Gap-A (web-key path) fixed →
  now works on both paths. **⚠ Gap-A needs live validation** with a
  web-player-only account (does `api.music.apple.com` accept the AMPWebPlay
  token for the album catalog call? the artwork path's 2026-06-21 comments say
  yes).
- **Open follow-ups (enriched with ITAM specs, most need LIVE testing):**
  #971 (MUT on catalog — foundation wired), #972 (HLS resolution select),
  #973 (`&l={locale}`), #974 (native fMP4 concat), #1010 (web-token expiry —
  safe, no live test), #1011 (`extend=audioTraits`), #1012 (dead
  `fetch_syllable_lyrics` IPC — `for consideration`).

---

## 5. Branch state

- **Local `main`** fast-forwarded to `origin/main` (aligned).
- **Remote branches** are all structural channel branches (alpha/beta/nightly/
  release-candidate/main) — no stray feature/prep branches (already pruned).
  Nothing to delete/consolidate. (beta/rc look stale — a release-process note,
  not a cleanup action; deleting channel branches needs explicit go-ahead.)
- **`meedyadl-v2`** deleted 2026-07-10 (owner decision; tip `f8326bf8`). It was
  a stale local-only copy of the April-2026-deleted v2 branch — no unrecovered
  work (modules already extracted; see `project_meedyadl_v2_archive.md`). Local
  is now fully aligned with GitHub (alpha, main, + this working branch).

---

## 6. Remaining / deferred (this session)

1. **Swagger/OpenAPI docs — DROPPED** (owner decision 2026-07-10: "none,
   ignore"). No repo spec exists; MeedyaDL is a Tauri IPC app; the claude.ai
   Swagger connector needs auth. Not pursued.
2. **Live-validation pass** for #971–#974 + Gap-A (needs real Apple Music
   credentials + a running app — can't be done statically). #1010 (web-token
   expiry) is safe to implement without live testing.
3. **Open the single PR** (`prep/alpha-gamdl-3.8.2-plus-2026-07-10` → `alpha`)
   when the user says go — held for now so more queued work folds into ONE PR
   (no stacking). Monitor CI, fix as issues appear. Rewrite the release-please
   PR body later per the CLAUDE.md gold-standard format.
4. **Strategy `for consideration`:** #1013 (upstream GAMDL wheel matrix /
   abi3 — the path to unblocking 3.8.2+) and #1014 (per-platform support
   window). These gate the next GAMDL ceiling advance.

---

## 7. Verification cheatsheet

```bash
cd src-tauri && export PATH="$HOME/.cargo/bin:$PATH"
cargo test --lib                                   # 1459/1459
cargo test --lib services::gamdl_capabilities      # 32/32
cargo test --lib services::enhanced_lyrics_service # 26/26
cargo test --lib utils::process                    # 142/142
cargo test --lib services::apple_music_api          # 134/134
cargo test --lib services::update_checker           # 15/15
cd .. && npm run type-check                          # clean
# Note: CI does NOT gate on `cargo fmt --check`; repo has pre-existing fmt drift.
# When editing, rustfmt ONLY your touched files (not whole-crate cargo fmt).
```

## 8. Issues opened this session

#1008 (web-key syllable + hasLyrics filter — fixed), #1009 (GAMDL v3.8.2 hold +
hardening — fixed), #1010 (web-token expiry), #1011 (audioTraits extend), #1012
(dead IPC cleanup, `for consideration`). Enriched: #970 (fixed), #969 (fixed),
#971/#972/#973/#974 (ITAM specs added, live-test-gated).
