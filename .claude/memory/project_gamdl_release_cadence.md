---
name: GAMDL release cadence is unusually fast
description: Upstream GAMDL ships multiple releases per day at times — every release needs a structured audit before the support window ceiling moves
type: project
originSessionId: 2ab3d7da-8f4e-4331-8327-4ea82ab8e25f
---
Upstream GAMDL has been shipping at an unusual cadence — four releases between 2026-04-24 and 2026-04-27 (3.2 → 3.3 same day, then 3.4 → 3.5 same day three days later). Every one has been a small bug-fix patch with a narrow change set (1–11 commits, 1–9 files), but the cumulative pace means the support window will need bumping repeatedly.

**Audit pattern that has held across v3.2, v3.3, v3.4, v3.5** (none required code changes — all four bumped only `tool-versions.toml` and docs):

1. `gh api repos/glomatico/gamdl/compare/{prev}...{next}` to enumerate commits + files.
2. For each non-trivial file, fetch the patch and check it against MeedyaDL's integration surface: `gamdl_options.rs::to_cli_args`, `config_service.rs::settings_to_ini`, `gamdl_capabilities.rs::GamdlFeature`, `process.rs::TRACK_INFO_V2_REGEX` / `ERROR_PREFIX_REGEX` / `classify_error`, and the stdout/stderr reader tasks in `download_queue.rs`.
3. Document in `.github/audits/gamdl-vX.Y-audit.md` (or a combined `gamdl-vX.Y-vX.Z-audit.md` for paired same-week releases) following the v3.2 audit's structure: per-finding analysis, capability gate matrix, floor analysis, conclusion.
4. Bump `tool-versions.toml` `maximum_tested_version` and `recommended_version`, append the inline narrative paragraph mirroring 3.2/3.3.
5. Update README.md Component Support Matrix, CLAUDE.md "Version-aware GAMDL dispatch" paragraph.
6. Run `cargo test --lib gamdl_capabilities` — the support window parser test will fail loud if the TOML edit is malformed.

**Why:** This is a recurring pattern (3 audits in ~3 weeks) that the user runs the same way each time. Capturing the workflow saves re-deriving the integration-surface checklist on every release. The fact that every audit has been zero-code-change so far is itself useful signal — the GAMDL team isn't shifting CLI/INI surface in patch releases, so the audit's primary value is verification + documentation, not finding regressions.

**How to apply:** When the user asks to audit a new GAMDL release, follow the checklist above. Default expectation is "ceiling bump only" — flag any deviation prominently. The audit document goes in `.github/audits/`, and a tracking issue should be filed (Lance always wants an issue per piece of work — see `feedback_github_issues.md`).

## v3.8.2 — the first NON-admission (2026-07-10)

GAMDL 3.8.2 broke the zero-code-change streak in a new way: it shipped a compiled **Rust/PyO3 extension** (`gamdl._ammuxer`) with only a single `cp310` Linux-x86_64 wheel + an sdist. MeedyaDL's bundled CPython 3.12.8 (no Rust toolchain) can't install it on ANY platform → **ceiling HELD at 3.8.1** (issue #1009, audit `gamdl-v3.8.2-audit.md`).

**New audit gate — wheel availability.** Add a step to the checklist: check PyPI `gamdl/{version}/json` `urls` for a wheel matching the bundled interpreter (`py3-none-any`, `cpXY`, or `abi3`). If a release ships only platform/ABI-specific wheels that don't cover MeedyaDL's runtime (or only an sdist for a package that now needs a compiler), it is **NOT admissible** regardless of CLI/INI surface cleanliness. The `no_compatible_wheel` flag (`update_checker.rs`) automates the user-facing side; the audit must still make the hold decision explicitly.

**Also new:** GAMDL↔wrapper-v2 version lockstep (3.8.2 hard-requires wrapper-v2 0.0.2). Even a held ceiling needs an error classifier for the skew (`wrapper_version_mismatch` in `process.rs`).
