# GAMDL v3.2 Compatibility Audit

**Branch**: `claude/audit-gamdl-v3.2-eI87q`
**Date**: 2026-04-24
**GAMDL releases audited**: 2.9.1, 2.9.2, 2.9.3, 3.0, 3.1, 3.2
**Umbrella issue**: #613

This document captures the audit findings for GAMDL v3.2 compatibility against
MeedyaDL's integration surface. Each section below corresponds to a filed
GitHub issue and records the investigation, verified facts, and resulting
decision.

## Methodology

1. Fetched every GAMDL source tarball from PyPI (`pip download --no-binary :all:`)
   for versions 2.9.1 through 3.2 and diffed them pairwise.
2. Cross-referenced every CLI flag in `gamdl/cli/cli_config.py` and every INI
   key emitted by MeedyaDL's `config_service.rs::settings_to_ini` against the
   Click parameter set each version actually recognises.
3. Verified behaviour of `dataclass_click` (which sets `click.Parameter.name`
   from the Python field name, not the `--flag` name) by running a minimal
   repro locally.
4. Audited MeedyaDL's integration sites:
   - `src-tauri/src/models/gamdl_options.rs` (`to_cli_args`)
   - `src-tauri/src/services/config_service.rs` (`settings_to_ini`)
   - `src-tauri/src/services/download_queue.rs` (`merge_options`)
   - `src-tauri/src/services/gamdl_capabilities.rs` (feature gating)
   - `src-tauri/src/utils/process.rs` (output parsing)
   - `src-tauri/tool-versions.toml` (`[gamdl]` support window)

## Issue 614 — `--song-codec` rejected on v3.0+ (and never existed on v2.9.1+)

Per user request, this finding was re-verified against the full support window
(not just v3.x). The `else if` branch in `audio_cli_args()` that emits
`--song-codec <codec>` when `song_codec_priority` is unset crashes GAMDL on
**every** release in our support window, not only v3.0+.

### Upstream CLI declaration, grepped across every tarball

```text
v2.9.1: line 378 — song_codec_piority: Annotated[list[SongCodec], option("--song-codec-priority", …)]
v2.9.2: line 378 — (identical)
v2.9.3: line 378 — (identical)
v3.0:   line 223 — (module relocated, same declaration)
v3.1:   line 239 — (padded structlog changes only)
v3.2:   line 239 — (same as 3.1)
```

**No release in our support window declares `--song-codec` as a Click option.**
The single-codec flag was removed when GAMDL split `cli.py` into the
`cli/cli_config.py` / `cli/config_file.py` structure in the 2.9.1 refactor.

### Fallback-strategy outcome

The user asked: if v2.9.1–v2.9.3 doesn't support the fix, consider raising the
support floor to v3.x only (last resort). **Not necessary.** The proposed fix
(always emit `--song-codec-priority`) works on every release from v2.9.1
onward because:

1. `--song-codec-priority` exists in v2.9.1.
2. Its type is `Csv(SongCodec)` — a one-element CSV is valid, so `alac` is
   accepted identically to `alac,atmos,aac`.

### Resolution

Filed as #614 with Option B (always emit `--song-codec-priority`). Support
window remains `[2.9.1, 3.2]`.

