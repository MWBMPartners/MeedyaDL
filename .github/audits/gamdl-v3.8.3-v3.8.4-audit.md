# GAMDL v3.8.3 + v3.8.4 Compatibility Audit — DECISION: ADMITTED (ceiling → 3.8.4)

**Date**: 2026-07-18
**GAMDL releases audited**: 3.8.3 (released 2026-07-12) + 3.8.4 (released 2026-07-16)
**Diff range**: `3.8.2..3.8.4` (14 commits, 9 files; net state at tag `3.8.4`)
**Predecessor audit**: [`gamdl-v3.8.2-audit.md`](./gamdl-v3.8.2-audit.md)
**Tracking issue**: #1018

## TL;DR

**Admit 3.8.4; bump the ceiling 3.8.2 → 3.8.4** (covers the intervening 3.8.3).
This is a **zero-code-change ceiling bump** — the same shape as v3.3 / v3.5 /
v3.5.1 / v3.5.2 / v3.7.4 / v3.8.1. The entire 3.8.2 → 3.8.4 delta contains **no
Python-source change** (only the `__version__` string moves). Everything else is
CI wheel-publishing, a pyo3 build-dependency bump, packaging hygiene, and one
internal Rust fix in the compiled `_ammuxer` extension that **fixes corrupted
song endings on wrapper-decrypt downloads** — a data-corruption bug present in
3.8.2 and 3.8.3. The wheel gate re-verified live: 3.8.3 and 3.8.4 ship the
identical 5-platform `cp310-abi3` matrix to 3.8.2 (no ARMv7). Only edits needed:
`tool-versions.toml` + docs.

> **Extra urgency to bump:** 3.8.2 (the version MeedyaDL currently recommends)
> and 3.8.3 have a wrapper-decrypt bug that can cut off / corrupt the ending of
> some songs. 3.8.4 fixes it (`e4887d34`). Users on wrapper decryption benefit
> immediately from the ceiling bump.

## Verified facts (independent Opus gate, 2026-07-18)

- **Compare `3.8.2...3.8.4`** (GitHub API): 14 commits, 9 files. The only files
  under `gamdl/` besides `gamdl/__init__.py` are the Rust `_ammuxer` extension
  sources (`Cargo.lock`, `Cargo.toml`, `src/decrypt.rs`, `src/media.rs`,
  `src/mux.rs`). **No `cli/` or CLI/INI argument-definition `.py` changed.**
- **`gamdl/api/wrapper.py` is untouched** ⇒ `TARGET_WRAPPER_API_VERSION` stays
  `"0.0.2"`. MeedyaDL's `/me` preflight expected-version literal
  (`health_check_service.rs`) and `wrapper_version_mismatch` guidance
  (`process.rs`) remain correct.
- **Wheel matrix (PyPI JSON API, live):** 3.8.3 and 3.8.4 both ship —
  identically to 3.8.2 — `cp310-abi3` wheels for macOS universal2 (x86_64+arm64),
  Linux x86_64 (`manylinux_2_34`), Linux aarch64 (`manylinux_2_34`), Windows
  amd64, Windows arm64, plus an sdist. **No Linux ARMv7 (`armv7l`/`armhf`)
  wheel.** `requires-python` = `>=3.10`.
- **pyo3 0.27 keeps `abi3-py310`** — the wheels still load on the bundled
  CPython 3.12 and on any reused system Python ≥ 3.10 (`MIN_SYSTEM_PYTHON`).

## Change-set (14 commits, condensed)

| SHA | Message | Files | MeedyaDL-facing? |
| --- | --- | --- | --- |
| `c5b63927` `2637e9e8` `7f3a840d` `cba5f475` `209975ee` `d4fb3a9c` | Cross-platform / universal2 / ARM abi3 wheel-publishing CI | `.github/workflows/python-publish.yml` | No — CI only (the workflow that produced the 3.8.2 abi3 matrix; explains the 3.8.2 "stale PyPI data" episode) |
| `68d12ef5` `dcb0d43e` | Bump pyo3 0.23.5 → 0.27 for Python 3.14 (PR #333) | `ammuxer/Cargo.toml`, `Cargo.lock` | No — build-dep only; `abi3-py310` floor unchanged |
| `55c3abc0` | Release 3.8.3 (+ pyo3 API migration) | `__init__.py`, `pyproject.toml`, `uv.lock`, `python-publish.yml`, `ammuxer/src/{decrypt,media,mux}.rs` | No — mechanical `allow_threads`→`detach` / `downcast`→`cast` renames inside `_ammuxer` |
| `234e9fac` | Simplify publish workflow | `python-publish.yml` | No — CI |
| `b16d16fc` `61801bca` | Exclude Cargo target dir from wheel builds (PR #334) | `pyproject.toml` | No — sdist packaging hygiene; MeedyaDL uses `--only-binary=gamdl` |
| **`e4887d34`** | **Fix corrupted song endings with wrapper decryption** | `ammuxer/src/media.rs` (+50/−1) | **No code action** — internal `_ammuxer` fix; MeedyaDL observes output files. Sole functional runtime change in the range. |
| `0b48f351` | Release 3.8.4 | `__init__.py`, `pyproject.toml`, `uv.lock` | No — version bump |

## Findings

### 3.8.3-A — pyo3 0.23.5 → 0.27.2 (Python 3.14 support)

Build-time only. "Python 3.14 support" means GAMDL can now **build from sdist**
on CPython 3.14 (pyo3 0.23 refused 3.14); the abi3 wheels already *loaded* on
3.14 (proven in the 3.8.2 audit by importing `_ammuxer` + running
`python -m gamdl --version` on CPython 3.14). The `abi3-py310` feature is
unchanged, so nothing about which interpreters the wheel loads on changes.
**MeedyaDL surface impact: none** — MeedyaDL never compiles `_ammuxer`; it
installs a pre-built abi3 wheel via `--only-binary=gamdl` and invokes GAMDL as a
subprocess.

### 3.8.3-B — cross-platform abi3 wheel-publishing workflow

CI formalisation of the 5-platform abi3 matrix. **MeedyaDL surface impact:
none** beyond the wheel availability already relied upon since 3.8.2.

### 3.8.4-A — `flush_then_write_immediate()` sample-ordering fix (`e4887d34`)

`decrypt_track_wrapper()` in `ammuxer/src/media.rs` batches decrypted samples;
before this fix, "immediate" (non-encrypted-block) data could be written ahead of
still-queued batched samples, so the on-disk payload order diverged from the
`stsz` sample-size table — producing corrupted / truncated song endings on
wrapper-decrypt downloads. The fix flushes the queue before immediate writes.
**MeedyaDL surface impact: none** — this is inside the compiled decrypt/mux
pipeline; MeedyaDL observes the resulting audio *files*, never the mux path. It
is a pure user-facing correctness win. **Action:** retarget the pre-stable live
smoke-test at 3.8.4 (add a song-ending integrity check on a wrapper-decrypted
song); do **not** validate on 3.8.2, which is now known-bad here.

## Per-platform install behaviour (unchanged from 3.8.2)

| Platform | 3.8.4 wheel | Installs |
| --- | --- | --- |
| macOS Apple Silicon | `cp310-abi3` universal2 | 3.8.4 |
| Windows x64 | `cp310-abi3-win_amd64` | 3.8.4 |
| Windows ARM64 | `cp310-abi3-win_arm64` | 3.8.4 |
| Linux x64 | `cp310-abi3-manylinux_2_34_x86_64` | 3.8.4 |
| Linux ARM64 | `cp310-abi3-manylinux_2_34_aarch64` | 3.8.4 |
| Linux ARMv7 | **none** | **3.8.1** (range fallback via `--only-binary`) |

`gamdl>=3.0,<=3.8.4` + `--only-binary=gamdl` resolves 3.8.4 on 5/6 targets and
falls back to 3.8.1 on ARMv7 (its universal `py3-none-any` wheel is still in
range). The `no_compatible_wheel` UI guard stays platform-aware (flags only
ARMv7), computed live from the 3.8.4 file list at check time.

## Nothing else needs changing (swept)

Checked each coupling surface; all inherit 3.8.4 via existing version math:

- **`models/gamdl_options.rs`** (`to_cli_args`, `audio_cli_args`) — no flag
  added/removed/renamed upstream.
- **`services/config_service.rs`** (`ini_*`) — no INI key change.
- **`services/download_queue.rs`** (`merge_options`, companions, gap-fill) — no
  behavioural change; `WrapperDecryptHostPort` (`>= 3.8.2`) still drives the
  wrapper-v2 TCP decrypt emission + preflight.
- **`services/gamdl_capabilities.rs`** — no `GamdlFeature` add/re-threshold; all
  3.8.2-keyed gates go true on 3.8.3/3.8.4 by version math.
- **`services/gamdl_service.rs`** — `--only-binary=gamdl` install spec unchanged
  (both call sites).
- **`utils/process.rs`** — no exception class, log-line shape, error prefix, or
  classifier substring change (`ERROR_PREFIX_REGEX`, `PYTHON_EXCEPTION_REGEX`,
  `TRACK_INFO_V2_REGEX`, `classify_error`, `wrapper_version_mismatch`,
  `is_media_not_streamable_error`, `is_storefront_mismatch_error` all still
  accurate).
- **`services/update_checker.rs`** — `is_wheel_compatible` matches
  `cpython_tag` OR `abi3` + platform; 3.8.4 filenames are shape-identical to the
  3.8.2 fixtures.
- **`services/health_check_service.rs`** — `/me` expected version `"0.0.2"` still
  correct (`wrapper.py` untouched).

## Actions taken

- `src-tauri/tool-versions.toml` — `maximum_tested_version` + `recommended_version`
  → `3.8.4`; ARMv7 comment refreshed; 3.8.3+3.8.4 audit-trail block appended.
- `README.md` support matrix — GAMDL range → 3.0–3.8.4, recommended 3.8.4.
- `.claude/CLAUDE.md` — capability note appended (v3.8.3 + v3.8.4 paragraph).
- (optional) `WrapperDecryptHostPort` gate test true-list gains `"3.8.4"` for
  documentation value.

## Test impact

- **`services::gamdl_capabilities`** — no edits required. `classify_*`,
  `support_window_*`, `pip_version_spec_bounds_the_range`, `is_above_tested_ceiling_*`,
  `should_offer_upgrade_*` read `support_window()` dynamically; gate tests use
  explicit version literals independent of the ceiling. (Added `"3.8.4"` to the
  `WrapperDecryptHostPort` true-list purely for documentation.)
- **`services::update_checker`** — no edits. Wheel fixtures are representative
  shape data, not ceiling-keyed.
- **`utils::process`** — no edits. `wrapper_version_mismatch` classifier/guidance
  key on GAMDL's unchanged startup-abort output.
- Expected: `cargo test --lib` green with only the `tool-versions.toml` string
  change (the `support_window_has_recommended_inside_range` invariant holds:
  3.0 ≤ 3.8.4 ≤ 3.8.4).

## Pre-release gate (carried forward, retargeted at 3.8.4)

Before this ceiling reaches **stable**, on each shipping platform:

1. `import gamdl._ammuxer` + a real song download decrypts + muxes on the bundled
   cp312 (abi3 import proven on 3.14; the decrypt path still wants a live run,
   esp. Windows + Linux-arm).
2. Real wrapper-v2 0.0.2 round-trip — local + remote/LAN (decrypt host/port).
3. **Song-ending integrity** — download a wrapper-decrypted ALAC song and confirm
   the final seconds play cleanly (validates `e4887d34`; 3.8.2/3.8.3 are known-bad).
4. A music-video download (local-key decrypt via `_ammuxer`).
5. `pip install --only-binary=gamdl 'gamdl==3.8.4'` resolves on the 5 wheel
   platforms; the range falls back to 3.8.1 on ARMv7.

## `for consideration` follow-ups (non-blocking)

1. **Carried forward:** `SongCodec::is_wrapper_dependent()` + `(Experimental)`
   codec labels are conceptually stale since 3.8's assets API.
2. **Carried forward:** dead `fetch_extra_tags` plumbing removal.
3. **#1013 stands** ("Linux ARMv7 wheel only") — upstream `209975ee` added Linux
   aarch64 + Windows ARM64 wheels but not `armv7l`; an upstream feature request
   for an armv7l wheel is the only path to closing it.
4. **Wrapper-version map:** the expected daemon version `"0.0.2"` is a bare
   literal in `health_check_service.rs`. Still correct, but an
   `expected_wrapper_v2_version(gamdl_version) -> &str` helper next to the gates
   would make a future wrapper-v2 0.0.3 a one-line change.
