---
name: GAMDL 4.0 preparation (dormant until switched on)
description: GAMDL 4.0 is a rewrite; MeedyaDL refuses 4.x and carries a ready-but-unused 4.x command builder. Raising HIGHEST_DRIVABLE_MAJOR is the switch.
type: project
---
On 2 Nov 2026 the unreleased upstream GAMDL `4.0` branch (commit `0c884ecc`, version `4.0.0a0`) was read file by file. It is a rewrite: no `python -m gamdl` (no `__main__`), downloading is the `rip` subcommand, YAML settings instead of `config.ini`, renamed options, `rich` text output, no helper programs, Widevine moved to an optional `gamdl[widevine]` extra, artwork saved per track, lyrics only `.lrc`/`.txt` (no `.ttml`).

**What MeedyaDL now has (commit `06e5b53` on `work-in-progress`):**
- `gamdl_capabilities::HIGHEST_DRIVABLE_MAJOR = 3` — the on/off switch. `refuse_unsupported_target` refuses any 4.x install; the update check marks 4.x `is_compatible = false` so it is never offered.
- `GamdlCliGeneration` / `cli_generation_of` / `detected_cli_generation`, read from the major number only (so `4.0.0a0` counts).
- `GamdlOptions::to_cli_args_v4` + `gamdl_service::gamdl_v4_args` / `build_gamdl_v4_command`: `python -c "<launcher>" --no-config rip <options> <links>`, stdin closed, `COLUMNS=4096`, `NO_COLOR`, UTF-8 output, `GAMDL_*` env vars removed. Only reached when the installed GAMDL is 4+.
- `process::parse_gamdl_v4_line` for `✓/↷ title: outcome → path`, `✗ …`, `Finished: …`.
- `FFmpegPath` and `NoExceptionsFlag` gates are off from 4.0.

**Why:** without the refusal, the "Untested" upgrade button would install 4.0 the day it is published and every download would fail on the first line.

**How to apply:** when 4.0 is released, re-diff the release against the audit (`.github/audits/gamdl-v4.0-preview-audit.md`), work through its follow-up list (lyrics `.ttml`, artwork, progress, PlayReady extra, settings screen, docs), test on real machines, then raise the switch to 4 and update its test. Do not raise it on code reading alone.
