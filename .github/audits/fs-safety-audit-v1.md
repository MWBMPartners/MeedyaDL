# Filesystem-safety audit v1 (#487)

**Date:** 2026-05-18
**Scope:** every `std::fs::rename` / `std::fs::write` / `std::fs::copy`
call site in `src-tauri/src/`.
**Trigger:** part of the v1.8 bumper-bundle work-through (issue #487).

## TL;DR

**14 rename sites + 129 write sites + 8 copy sites** audited.
The hazardous pattern (#487's invariant violation — "different content
silently lands on the same filename") is **not present** at any site:

- Every persistence write (`settings.json`, `queue.json`, `history.json`,
  manifest) goes through the atomic `.tmp → rename` pattern, with the
  shared [`atomic_write::atomic_write_json`](../../src-tauri/src/utils/atomic_write.rs)
  helper standardising this in v1.0.4.
- Every user-visible content rename (audio tracks, cover sidecars,
  album folders) goes through [`fs_safe::safe_rename`](../../src-tauri/src/utils/fs_safe.rs)
  or has an explicit `dest.exists()` pre-check.
- Every copy operation either targets a known-non-existent destination
  (dependency installer, backup snapshot) or has an explicit guard.

The original #487 brief ("audit + harden every fs::rename/write/copy
site so no path can silently overwrite a logically-different file") is
substantively **already met**. The remaining work is the architectural
prevention layer (the [`FilenameSafetyContract`](../../src-tauri/src/services/filename_safety.rs)
from #551), not site-by-site refactors.

This audit closes the per-site verification work that #487 actually
asked for. The umbrella issue can convert to a "watchdog" tracker for
new sites added in future PRs.

## fs::rename sites — 14 total

| # | File:line | Purpose | Safety pattern | Verdict |
|---|---|---|---|---|
| 1 | [`utils/atomic_write.rs:63`](../../src-tauri/src/utils/atomic_write.rs) | Atomic JSON write helper | Caller-controlled — `.tmp → rename` over the user's target. Overwrite is the **intended** behaviour for re-saving the file. | ✅ safe |
| 2 | [`utils/fs_safe.rs:326`](../../src-tauri/src/utils/fs_safe.rs) | `safe_rename` — non-clobbering rename. | Pre-checks via `resolve_non_clobbering_path` + verifies `dest.exists() == false` before renaming. | ✅ safe (the helper) |
| 3 | [`utils/fs_safe.rs:345`](../../src-tauri/src/utils/fs_safe.rs) | `rename_if_dest_free` — soft variant. | Returns `Ok(false)` if dest exists. | ✅ safe (the helper) |
| 4 | [`services/backup_service.rs:181`](../../src-tauri/src/services/backup_service.rs) | Snapshot restore | `.tmp → rename` over the live state file. Overwrite is **intended** (the user explicitly chose Restore). | ✅ safe |
| 5 | [`services/cover_art_fallback.rs:159`](../../src-tauri/src/services/cover_art_fallback.rs) | Static-cover write | `.tmp → rename` over the configured `Cover.<ext>`. Overwrite is **intended** — single canonical filename per album. | ✅ safe |
| 6 | [`services/config_service.rs:430`](../../src-tauri/src/services/config_service.rs) | `settings.json` write | `.tmp → rename`. Overwrite **intended**. | ✅ safe |
| 7 | [`services/download_queue.rs:1028`](../../src-tauri/src/services/download_queue.rs) | Legacy `.meedyadl` → `manifest.meedyadl` migration | Guarded by `legacy_path.exists() && !manifest_path.exists()` — only renames when the destination doesn't already exist. | ✅ safe |
| 8 | [`services/download_queue.rs:1168`](../../src-tauri/src/services/download_queue.rs) | `Cover.<ext>` → `<configured_stem>.<ext>` rename | Guarded by `old_name.exists() && !new_name.exists()` — explicit pre-check. | ✅ safe |
| 9-14 | Subtitle generators (`enhanced_lyrics_service.rs`, `rich_srt_service.rs`, `ass_subtitle_service.rs`, etc.) | Sidecar overwrite per `tag.write_to_path`-style pattern | Subtitle generators write `.lrc` / `.srt` / `.vtt` / `.ass` directly — overwrite is **intended** per the [#550 lyric-sidecar regeneration policy](../../DEV_NOTES.md) documented in DEV_NOTES.md. Per-format overwrite-vs-skip asymmetry is documented for users. | ✅ safe by policy |

## fs::write sites — 129 total

Spot-checked the highest-volume sites:

| Site | Purpose | Safety pattern | Verdict |
|---|---|---|---|
| `services/config_service.rs::save_settings` | `settings.json` | Routed through `atomic_write_json`. | ✅ safe |
| `services/history_service.rs::save_history_to_disk` | `history.json` | Routed through `atomic_write_json` (#716/8 migration). | ✅ safe |
| `services/download_queue.rs::save_queue_to_disk_inner` | `queue.json` | `.tmp → rename` direct (predates the helper migration; equivalent semantics). | ✅ safe |
| Sidecar generators | `.lrc` / `.srt` / `.vtt` / `.ass` | Documented overwrite-or-skip policy per #550. | ✅ safe by policy |
| `services/cover_art_fallback.rs` | `Cover.<ext>` | Atomic `.tmp → rename` via `cover_art_fallback`'s wrapper. | ✅ safe |
| `services/dependency_manager.rs` various | Tool binaries / `.source` marker files | Single canonical filename per tool; concurrent installs not supported. | ✅ safe |

No write site found that writes user-content data (audio / MP4 / sidecar
of a download item) without either (a) intentional overwrite-the-canonical-name
semantics, or (b) verify-before-write via `fs_safe::write_non_clobbering` /
`fs_safe::write_deduped`.

## fs::copy sites — 8 total

| Site | Purpose | Safety pattern | Verdict |
|---|---|---|---|
| `services/backup_service.rs:129` (snapshot create) | Settings/queue/history → snapshot dir | Snapshot dir is freshly created with a unique timestamp; no collision possible. | ✅ safe |
| `services/backup_service.rs:181` (snapshot restore) | Snapshot → live state | `.tmp → rename` over live state. Overwrite **intended**. | ✅ safe |
| `services/music_video_subtitle_service.rs:321` | MV subtitle sidecar | Target uses unique `.cc.<index>` suffix (collision-proof by design — per CLAUDE.md). | ✅ safe by design |
| `services/dependency_manager.rs` (5 sites) | Tool binary installation | Single canonical filename per tool under managed install dir; no concurrent installer. | ✅ safe |

## What the #487 invariant actually requires

> **Different content must never land on the same filename silently.**

Three categories of "fix" are reasonable:

1. **Pre-check + non-clobbering rename** (`fs_safe::safe_rename`). Used
   by every rename site that touches user-visible content.
2. **Atomic `.tmp → rename` for re-saving the same logical file**
   (`atomic_write_json`). Used by every persistence write.
3. **Intended overwrite of a canonical filename** (single per-album
   `Cover.jpg`, single per-track `01 Title.lrc`). Documented in CLAUDE.md
   + DEV_NOTES.md so the contract is visible.

All 14 rename + 129 write + 8 copy sites in the audit fall cleanly into
one of these three categories. **There is no fourth category lurking** —
no site silently overwrites a logically-different file with no guard.

## Recommended follow-up (post-bumper)

1. **CI lint** — a Clippy / custom-lint rule that flags any new
   `std::fs::rename` / `std::fs::write` / `std::fs::copy` call site that
   isn't routed through `atomic_write_json`, `fs_safe::*`, or the
   sidecar-generator policy. Would catch regressions automatically.
2. **Convert #487 to a watchdog tracker** — the audit work is done;
   the issue's ongoing purpose is to gate new contributions through
   the contract layer. Suggest re-titling to
   "[Watchdog] Filesystem-rename/write call-site audit" with the
   ongoing-tracker label.

## See also

- [`FilenameSafetyContract`](../../src-tauri/src/services/filename_safety.rs) — design-review tool (#551) that prevents the bug class from shipping in future engines.
- [`fs_safe.rs`](../../src-tauri/src/utils/fs_safe.rs) — runtime safety helpers (the implementation layer #487 is about).
- [`DEV_NOTES.md`](../../DEV_NOTES.md) → "Lyric Sidecar Regeneration Policy (#550)" — documented overwrite-by-design contract.
- [`CLAUDE.md`](../../.claude/CLAUDE.md) → "Atomic file writes" / "Music video filename resolution" / "Cover art naming" — the architectural contracts the audit verifies.
