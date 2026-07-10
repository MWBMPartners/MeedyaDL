# GAMDL v3.8.2 Compatibility Audit — DECISION: ADMITTED (ceiling → 3.8.2)

**Date**: 2026-07-10
**GAMDL release audited**: 3.8.2 (released 2026-07-09)
**Diff range**: `3.8.1..3.8.2` (31 commits, net state at tag `3.8.2`)
**Predecessor audit**: [`gamdl-v3.8.1-audit.md`](./gamdl-v3.8.1-audit.md)
**Tracking issue**: #1009

## Correction notice (read first)

An initial pass **HELD** at 3.8.1 on the belief that 3.8.2 shipped only a
`cp310-cp310-manylinux_2_34_x86_64` wheel + sdist and was therefore
uninstallable on MeedyaDL's bundled CPython 3.12. **That was stale PyPI data** —
the abi3 wheels were still uploading (and/or a CDN-cached JSON was served) when
first checked. A cache-busted re-check + live `pip download` per platform showed
GAMDL 3.8.2 publishes **`cp310-abi3`** wheels for 5 of 6 platforms. The decision
is reversed to **ADMITTED**. The lesson (a wheel set can be incomplete for hours
after a release — always re-check) is recorded in `project_gamdl_release_cadence`.

## TL;DR

**Admit 3.8.2; bump the ceiling 3.8.1 → 3.8.2.** GAMDL 3.8.2 ships a compiled
Rust `ammuxer` extension as **`cp310-abi3`** wheels (stable ABI, load on
CPython 3.10+, incl. the bundled 3.12) for **macOS universal2, Windows
x64/ARM64, Linux x86_64/aarch64** — installable on 5 of 6 targets. Only **Linux
ARMv7** has no wheel. The one genuine MeedyaDL code change is the **wrapper-v2
0.0.2 decrypt host/port** (decrypt moved from HTTP `POST /decrypt` to a native
TCP port; MeedyaDL previously emitted nothing for wrapper-v2 decrypt →
remote/LAN wrapper users would silently fail — the #743 bug class). Everything
else is subprocess-transparent.

## Verified facts

- **Wheels** (live PyPI + `pip download` per platform on cp312):
  `cp310-abi3` for macOS universal2 / Win amd64 / Win arm64 / manylinux_2_34
  x86_64 / manylinux_2_34 aarch64 (+ a redundant `cp310-cp310` linux wheel) +
  sdist. **No ARMv7 wheel.**
- **abi3 genuineness (smoke-test)**: in a throwaway venv on **CPython 3.14.0**
  (≠ the `cp310` tag), `pip install gamdl==3.8.2` then
  `import gamdl._ammuxer` → OK (exposes `decrypt_and_mux_wrapper_native`,
  `WrapperDecryptSession`, `mux_decrypted_*`, …) and `python -m gamdl --version`
  → `3.8.2`. Loading on a non-3.10 minor proves the wheel is genuinely abi3 ⇒
  it loads on the bundled 3.12. (Windows / Linux-arm *runtime* still want a live
  check — same CI build, lower risk.)
- **wrapper-v2 0.0.2 daemon** (`glomatico/wrapper-v2@0.0.2`): HTTP default `:80`,
  **separate raw-TCP decrypt listener default `:10020`**; `POST /decrypt` HTTP
  now 404 ("decrypt is available on the raw TCP port"); `GET /me` →
  `{version:"0.0.2", runtime, auth}`. Matches GAMDL's client defaults.
- **GAMDL 3.8.2 client** (`gamdl/api/wrapper.py`): `TARGET_WRAPPER_API_VERSION =
  "0.0.2"`, exact-match `validate_api_version` at CLI startup; decrypt via
  `--wrapper-decrypt-host`/`--wrapper-decrypt-port` (default `127.0.0.1:10020`,
  correctly-spelled INI keys), independent of `--wrapper-url`.

## Per-platform install behaviour

The routine install spec `gamdl>=3.0,<=3.8.2` + the `--only-binary=gamdl`
guardrail resolves to:
| Platform | Result |
| --- | --- |
| macOS arm64/x64, Win x64/ARM64, Linux x64/aarch64 | **3.8.2** (abi3 wheel) |
| Linux ARMv7 | **falls back to 3.8.1** (3.8.2 has no wheel; `--only-binary` excludes the sdist; 3.8.1's universal wheel is still in range) |

The platform-aware `no_compatible_wheel` guard flags **only ARMv7** for the
explicit "Untested" upgrade path (disabled Upgrade button there).

## MeedyaDL changes made (this PR)

| # | Change | File(s) |
| --- | --- | --- |
| a | `GamdlFeature::WrapperDecryptHostPort` (≥ 3.8.2) + summary entry + gate test | `gamdl_capabilities.rs` |
| b | Emit `--wrapper-decrypt-host`/`--wrapper-decrypt-port` (split `wrapper_decrypt_ip` at last `:`, IPv6-safe) on the wrapper-v2 CLI path; thread the dormant `options.wrapper_url` so `--wrapper-url` is actually emitted | `gamdl_options.rs`, `download_queue.rs` |
| c | Emit `wrapper_decrypt_host`/`wrapper_decrypt_port` in the wrapper-v2 INI (via `sanitize_ini_value`) | `config_service.rs` |
| d | Re-enable the TCP decrypt preflight for wrapper-v2 on 3.8.2+ | `download_queue.rs` |
| e | wrapper-v2 `/me` version preflight — warn (not block) when GAMDL ≥ 3.8.2 and the daemon != 0.0.2 | `health_check_service.rs` |
| f | Platform-aware `no_compatible_wheel` (flag only ARMv7) | `update_checker.rs` |
| g | Ceiling `maximum_tested_version`/`recommended_version` → 3.8.2 | `tool-versions.toml` |
| h | Flip `wrapper_version_mismatch` guidance (3.8.2 ⇔ 0.0.2; 3.6–3.8.1 ⇔ 0.0.1) | `process.rs` |

No settings-schema bump (reuses `wrapper_decrypt_ip`; `CURRENT_SETTINGS_VERSION`
unchanged). Full lib suite **1465+/… green** after each unit.

## Nothing else needs changing (swept)

`ammuxer` is internal to GAMDL (subprocess-transparent). cli/cli_config added
only the 2 decrypt options. Stream-lookup (playback-before-assets) is internal
HLS selection — zero output/log/exception surface (like v3.8.1). The
wrapper-CBCS / MV-wrapper-decrypt commits in the range were reverted (net-zero
at the tag); music videos still use local-key decrypt.

## Pre-release gate (LIVE smoke-tests — not doable statically)

Before this ceiling reaches **stable**, confirm on each shipping platform:
1. `import gamdl._ammuxer` + a **real song download** decrypts+muxes on the
   bundled cp312 (abi3 import verified on 3.14; the real decrypt path wants a
   live run, esp. Windows + Linux-arm).
2. A real **wrapper-v2 0.0.2** round-trip — local (defaults) and remote/LAN
   (decrypt host/port pointed at the daemon), ALAC song decrypts.
3. Version-skew: GAMDL 3.8.2 + wrapper-v2 ≤ 0.0.1 aborts at startup and the new
   `/me` preflight surfaces the warning.
4. A **music-video** download (local-key decrypt path through the ammuxer).
5. `pip install --only-binary=gamdl gamdl==3.8.2` resolves the abi3 wheel on the
   5 platforms and the range falls back to 3.8.1 on ARMv7.

## Follow-ups

- **#1013** (upstream wheel matrix) is now largely satisfied — upstream ships
  abi3 for 5/6; **retarget to "Linux ARMv7 wheel only."**
- **#1014** (per-platform support window) — the `--only-binary` range fallback
  gives correct ARMv7 behaviour today; a formal per-platform window is still the
  clean long-term model when platform wheel coverage diverges further.
- `SongCodec::is_wrapper_dependent()` / "(Experimental)" labels — stale since
  3.8's assets API (Atmos/AC3 now wrapper-less); pre-existing `for consideration`.
