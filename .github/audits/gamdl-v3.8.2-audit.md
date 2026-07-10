# GAMDL v3.8.2 Compatibility Audit — decision: HOLD ceiling at 3.8.1

**Date**: 2026-07-10
**GAMDL release audited**: 3.8.2 (released 2026-07-09)
**Diff range**: `3.8.1..3.8.2` (31 commits, net state audited at tag `3.8.2`)
**Predecessor audit**: [`gamdl-v3.8.1-audit.md`](./gamdl-v3.8.1-audit.md)
**Tracking issue**: #1009

## TL;DR

**v3.8.2 is NOT admitted. `maximum_tested_version` stays at 3.8.1.** This is
the first GAMDL release MeedyaDL cannot install at all. GAMDL replaced the
pure-Python `amdecrypt.py` with a compiled **Rust/PyO3 extension**
(`gamdl._ammuxer`, imported unconditionally at package import) and publishes
exactly **one binary wheel — `gamdl-3.8.2-cp310-cp310-manylinux_2_34_x86_64.whl`**
— plus an sdist. MeedyaDL runs GAMDL on bundled python-build-standalone
**CPython 3.12.8** with no Rust toolchain, so `pip install gamdl==3.8.2` finds
no compatible wheel (cp310 ≠ cp312; the wheel is Linux-x86_64-only anyway),
falls back to the sdist → maturin → cargo → **fails on all six target
platforms, including Linux x64**.

Independently, 3.8.2 also hard-requires **wrapper-v2 0.0.2** (exact-match
version check) and moves decryption to a native TCP protocol. Neither breaks
MeedyaDL while the ceiling is held at 3.8.1, but they create a version-skew
hazard and shape the eventual admission plan.

## Methodology

Two independent Fable-5 deep passes over the 3.8.1..3.8.2 diff cross-referenced
against MeedyaDL's six GAMDL surfaces, plus hand-verification of the three
load-bearing facts:

- PyPI `gamdl/3.8.2/json` `urls` array (`cp310` wheel + sdist only; 3.8.1 was
  universal `py3-none-any`). ✔ verified.
- Bundled interpreter `python_manager::PYTHON_VERSION = "3.12.8"`. ✔ verified.
- `wrapper.py@3.8.2`: `TARGET_WRAPPER_API_VERSION = "0.0.2"`, exact-match guard,
  error `"Unsupported wrapper-v2 API version. gamdl requires wrapper-v2 0.0.2"`;
  new `--wrapper-decrypt-host`/`--wrapper-decrypt-port` (correctly-spelled INI
  keys `wrapper_decrypt_host`/`wrapper_decrypt_port`). ✔ verified from source.

## Findings

### (a) `ammuxer` native Rust extension — INSTALL BLOCKER (all 6 platforms)

`gamdl/downloader/amdecrypt.py` (3,546 lines Python) deleted; replaced by a
PyO3 `cdylib` crate `gamdl/downloader/ammuxer/` (`pyo3 = "0.23.5"`,
`extension-module`, **no `abi3`** → wheels are CPython-minor-specific).
`pyproject.toml` switched to `maturin`, `module-name = "gamdl._ammuxer"`, and
the shim is imported by `gamdl/downloader/__init__.py` — so the compiled
extension is required for *every* invocation, wrapper or not.

The publish workflow (`.github/workflows/python-publish.yml@3.8.2`) is a single
`ubuntu-latest` / Python 3.10 / `maturin build --release --sdist` job — no
cibuildwheel, no matrix, no abi3. Result on PyPI: one `cp310` Linux-x86_64
wheel + an sdist.

| MeedyaDL target | Compatible wheel for bundled CPython 3.12? | `pip install gamdl==3.8.2` |
| --- | --- | --- |
| macOS aarch64 | none | sdist → maturin → **FAIL** (no cargo) |
| Windows x64 | none | **FAIL** |
| Windows ARM64 | none | **FAIL** |
| Linux x64 | no (wheel is cp310, needs glibc ≥ 2.34) | **FAIL** |
| Linux aarch64 | none | **FAIL** |
| Linux ARMv7 | none | **FAIL** |

Routine installs are already safe: `pip_version_spec()` = `gamdl>=3.0,<=3.8.1`
resolves to 3.8.1's universal wheel. The exposed path was the Updates page's
"Untested" upgrade offer (`should_offer_upgrade(3.8.2)` = true) — a
guaranteed-fail click. **Mitigated** (see Actions).

### (b) wrapper-v2 0.0.2 hard requirement

`wrapper.py`: `WrapperApi.create()` calls `validate_api_version(me)` on the
`GET /me` response (`version` field, exact string equality) at CLI startup and
after login; mismatch raises `GamdlApiResponseError("Unsupported wrapper-v2 API
version. gamdl requires wrapper-v2 0.0.2", content={"detected_version": ...})`.
The HTTP `POST /decrypt` path is removed (-117 lines); decryption now rides a
native TCP protocol (`--wrapper-decrypt-host` / `--wrapper-decrypt-port`,
default `127.0.0.1:10020`).

**Version-skew hazard (lockstep required):**
- GAMDL ≤ 3.8.1 + wrapper-v2 0.0.2 → `POST /decrypt` 404 at decrypt time.
- GAMDL 3.8.2 + wrapper-v2 ≤ 0.0.1 → startup version-mismatch error.

No MeedyaDL breakage while the ceiling is held (users on GAMDL ≤ 3.8.1 must keep
wrapper-v2 at the 0.0.1/HTTP era). **Mitigated** with an error classifier +
version capture + docs (see Actions).

### (c) cli/cli_config.py deltas — 2 options added, 0 removed/renamed

`--wrapper-decrypt-host` (str, default `127.0.0.1`) + `--wrapper-decrypt-port`
(int, default `10020`), INI keys `wrapper_decrypt_host` / `wrapper_decrypt_port`
(correctly spelled — unlike the `song_codec_piority` precedent, these INI keys
work). `--ffmpeg-path` untouched (the `FFmpegPath` gate stays correct). No
tool-path options re-removed despite native muxing going Rust.

MeedyaDL already owns a `wrapper_decrypt_ip` setting (`host:port`, #743/#744)
wired only to wrapper-v1. Threading it into the new v2 keys is part of the
future admission plan (not needed while held).

### (d) stream-lookup change (playback-before-assets)

`interface/song.py`: `get_stream_info` now tries playback metadata before the
`/v1/play/assets` API. Purely internal HLS stream selection, DEBUG-only new
logs — zero MeedyaDL surface (same shape as v3.8.1). No change.

### (e) song / music_video downloader changes

`_decrypt_amdecrypt*` → `_decrypt_ammuxer*`, single native decrypt+mux calls;
the wrapper-CBCS / wrapper-MV-decrypt commits within the range were **reverted**
at the tag (music videos still use local-key decrypt). All private, no
output/exception/log surface change. No change.

## MeedyaDL surface-impact summary

| Surface | v3.8.2 impact | MeedyaDL action |
| --- | --- | --- |
| CLI flag encoding (`gamdl_options.rs`) | new decrypt host/port (wrapper-v2 only) | deferred to admission (future `WrapperDecryptHostPort` gate) |
| INI emission (`config_service.rs`) | new decrypt keys | deferred to admission |
| stdout/stderr parsing (`process.rs`) | new wrapper-mismatch error strings | **classifier added** (`wrapper_version_mismatch`) |
| capability gates (`gamdl_capabilities.rs`) | none | ceiling held at 3.8.1 |
| install / spawn (`gamdl_service.rs`) | sdist-only would source-build | **`--only-binary=gamdl` guardrail** |
| update check (`update_checker.rs`) | doomed "Untested" upgrade | **`no_compatible_wheel` flag + disabled button** |
| wrapper health (`health_check_service.rs`) | new `/me` version field | **captured + logged** (no enforcement yet) |
| HTTP / auth / artwork / lyrics (`apple_music_api.rs`) | none | none |

## Actions taken (this PR)

- **HOLD** `tool-versions.toml` `[gamdl]` `maximum_tested_version` /
  `recommended_version` at `3.8.1` — recorded as a deliberate decision with a
  v3.8.2 audit-trail block. No version bump.
- `gamdl_service.rs`: `--only-binary=gamdl` on both pip invocations → a future
  sdist-only release fails fast/clean instead of a maturin build.
- `update_checker.rs` + frontend: `ComponentUpdate.no_compatible_wheel` (PyPI
  version-scoped `urls` check vs the bundled `cpXY` tag / `py3-none-any` /
  `abi3`), "Not Installable" badge + disabled Upgrade button. Defaults to false
  on any PyPI error.
- `process.rs`: `is_wrapper_version_mismatch_error` + `wrapper_version_mismatch`
  classifier bucket (both skew directions) + actionable guidance + 7 tests.
- `health_check_service.rs`: `WrapperV2Me.version` capture + debug log; stale
  `"0.2.0"` doc example fixed to `"0.0.2"`.
- `models/settings.rs`: `wrapper_decrypt_ip` doc refreshed (`amdecrypt.py`
  deleted in 3.8.2 → native `ammuxer`; this socket = wrapper-v1 leg).
- Docs: this audit, CLAUDE.md GAMDL paragraph, `project_gamdl_release_cadence`
  memory (wheel-availability is now a first-class audit gate), wrapper-lockstep
  guidance in help.

## Admission plan (when upstream ships compatible wheels)

Admit 3.8.2 only when PyPI publishes wheels matching MeedyaDL's runtime — ideally
`abi3-py310` (covers the Python-minor dimension) via a maturin-action /
cibuildwheel matrix for macOS arm64 + Windows x64/ARM64 + Linux
x64/aarch64/armv7 (ARMv7 may never come → a per-platform ceiling may eventually
be needed; `support_window()` is currently global). Then:

1. bump `maximum_tested_version` / `recommended_version` → 3.8.2;
2. add `GamdlFeature::WrapperDecryptHostPort` (≥ 3.8.2) emitting
   `wrapper_decrypt_host` / `wrapper_decrypt_port` by splitting the existing
   `wrapper_decrypt_ip` setting at the last `:` (reuse — no new setting);
3. re-enable the TCP decrypt preflight for wrapper-v2 (the `if use_wrapper_v1`
   gate at the queue preflight also fires when the new capability holds);
4. add the wrapper-v2 `/me` version preflight (enforce 0.0.2) using the newly
   captured `WrapperV2Me.version`;
5. gate tests + `active_capabilities_summary`.

## Follow-ups worth filing

- Upstream wheel-matrix watch (and a polite upstream issue/PR proposing
  maturin-action matrix + `abi3-py310`).
- wrapper-v2 exact-match brittleness (every future wrapper-v2 needs a matching
  GAMDL) → the lockstep matrix in docs needs maintenance.
- New PyO3 decrypt/mux error surface — re-check `classify_error` coverage during
  live admission testing.
- Pre-existing 3.8 concept-drift (`is_wrapper_dependent()` / "(Experimental)"
  labels) — unchanged by 3.8.2, already `for consideration`.
