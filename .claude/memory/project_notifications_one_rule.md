---
name: Notifications — one rule, one owner (#1431)
description: How in-app and system notifications are decided since the 16 Nov 2026 rebuild, why the old fallback could never work, and what still needs a real machine.
type: project
---
Rebuilt on 16 Nov 2026 (#1431; commits 5130cba4 → cc0bad35 on `work-in-progress`, reviewed over 7 rounds by fresh Claude agents, Codex unavailable). Full technical description: the "Notifications — one rule, one owner" bullet in `.claude/CLAUDE.md`.

**Why it was rebuilt.** Two systems with different rules: every in-app message (~217 call sites) was mirrored as a system notification even while MeedyaDL was in front, yet finished/failed downloads never showed in the app. The "batching" still sent every notification. "Play a sound" played none. The page read unsaved settings, the backend saved ones.

**The trap worth remembering.** `tauri-plugin-notification` 2.8.1 on desktop always answers "granted" and never reports a failed send (`desktop.rs` permission_state/request_permission return Granted; JS `sendNotification` is fire-and-forget; Rust `show()` drops the result). So any "fall back to in-app when notifications are blocked" logic is dead code by construction, and tests that mock a refusal test behaviour that cannot happen. Do not rebuild that fallback; explain where to look per OS instead. Also: the plugin injects an init script that calls `plugin:notification|is_permission_granted` with no catch — removing every notification permission from the page causes an unhandled rejection on each launch on macOS/Linux, so the page keeps exactly `notification:allow-is-permission-granted`.

**Maintainer decisions (16 Nov 2026).** When: background/hidden/minimised OR on a screen the pointer is not on (default), only-other-screen, or always. Which messages: important events by default. Keep three styles; in "System only" anything with a button, details, error/warning, key, or an unsent system copy still shows in-app. Per-event switches for finished, failed, queue finished, copied link. Real queue-finished sound regardless of focus. Bursts within 10 s merge into one summary (first one immediate).

**Needs a real machine.** The three sounds; pointer/other-screen detection (mixed-scale monitors; Wayland); Windows/macOS development-build labelling; burst timing with real downloads.

**17 Nov 2026 note:** settings are now schema v13 (v12 added the notification settings; v13 is the automatic wrapper decryption address, unrelated). The page-side notification permission is guarded by `tools/audit-checks/check_capabilities_open_path.py`.

**Left for #1432.** An old download run can act on an item after it was retried (needs attempt numbering); one route frees the queue slot early; the stall check releases after a refused set_error; a stalled item gets no History entry.
