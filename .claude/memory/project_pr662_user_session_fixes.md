---
name: PR #662 — six user-reported fixes (2026-04-29 → 2026-04-30 session)
description: In-flight PR bundling six user-reported defects discovered in one v0.50.1 session — toast persistence, native-notification silence, fallback-chain rigidity, Python-traceback noise, terminal-state revival, post-abort companion-task chatter
type: project
---

## In-flight PR

**[PR #662](https://github.com/MWBMPartners/MeedyaDL/pull/662)** — branch `fix/toasts-notifications-fallback-tracebacks-revival`. CI green on all 10 checks (Backend macOS / Ubuntu / Windows, Frontend ×3, Analyze ×2, CodeQL) as of 2026-04-29 evening. Awaiting review/merge.

Six issues closed by the PR:

- **#657** — Duplicate-URL toast was typed `'warning'` (persistent); switched to `'info'` so it auto-dismisses. Frontend-only.
- **#658** — Native macOS notifications never fired with "Native + In-app" selected. Three-layer fix: startup permission preflight in `App.tsx` Effect 2, `console.warn` instead of silent `.catch(() => {})` in `uiStore.ts`, backend `send_desktop_notification` now gates on `notification_style != "in_app_only"`. New "Send Test Notification" button in Settings > General > Notifications.
- **#659** — Audio + Video fallback chains were reorder-only. Extended `FallbackChainList<T>` with optional `allItems: readonly T[]` prop; when supplied, rows get `X` (remove) buttons and an "Available (not in chain)" panel renders below with `+` (re-add) buttons. No settings schema migration needed.
- **#660** — Python tracebacks (`Traceback (most recent call last):` header / `File "..."` frames / caret highlight lines) leaked through to the activity log as misclassified Errors. New `GamdlOutputEvent::TracebackFrame` variant + cheap `process::is_python_traceback_noise()` helper used by stdout/stderr readers in `download_queue.rs` to gate the per-line `activity-log` event in non-verbose mode. The `traceback` keyword removed from Priority 7 — the explicit variant supersedes it. Disk mirror still records everything.
- **#661** — `set_complete()` and `set_error()` blindly overwrote terminal states; an `Error → Complete` transition triggered by the late completion-task pass was silently reviving failed items. Added guards: `set_complete` refuses when state is `Error`/`Cancelled`; `set_error` refuses when state is `Complete`/`Cancelled`. Misleading "marking complete" timeout text replaced with "skipping remaining companions; final tag pass still to run". New `Final tag pass: applying [Explicit]/[Clean] suffixes…` log entry makes the otherwise-silent post-companion advisory pass visible.
- **#663** — User predicted post-timeout symptom and captured evidence: 11+ minutes after `handle.abort()` fired at the deadline, the activity log "sprang back to life" with a burst of `Companion: converted N TTML file(s)` events. Root cause: `run_companion_lyrics_conversion` is a synchronous `fn` called from inside an async tokio task; `JoinHandle::abort()` only takes effect at `.await` points, so multi-minute sync work runs to completion regardless. Fixed via a new `CompanionTaskHandle { handle, aborted: Arc<AtomicBool> }` wrapper; `CompanionTaskHandle::abort()` sets the cooperative flag *and* aborts the async task. The tier loop and `run_companion_lyrics_conversion` check the flag at every loop boundary.

## Live state at session end

- **Repo at `main` HEAD `2afea4b`** (release-please / docs commits past v0.49.2).
- **Branch HEAD `67ac4fa`** carries the six commits + clippy doc-list lint fix.
- **App version on disk:** v0.50.1 (the user's running build) — they were observing the bugs against this. The PR will land in the next release.
- **No release-channel impact** — fixes are user-facing only, no schema migration, no CLI surface change.

## What was tricky / instructive

1. **`requestPermission()` once-per-bundle-id quirk on macOS.** Calling it lazily inside `addToast` means the user only sees the prompt buried in some random toast firing — and if they dismiss it, every later call returns `'default'` silently. Resolved by running the preflight at app startup and surfacing the resolved status.
2. **Async-task abort cannot preempt sync code.** Discovered when the user's "spring back to life" prediction was confirmed by the captured logs (22:08 timeout fires → silence → 22:19 `Companion: converted N TTML…` burst). The fix pattern (`Arc<AtomicBool>` cooperative-cancel flag checked at every loop boundary) is reusable — any new sync function called from inside an async task and longer than the parent's expected lifetime needs the same treatment. Recorded in CLAUDE.md.
3. **Doc-list lint trap.** `cargo clippy -- -D warnings` (CI) rejects unindented `3c.` / `4b.` numbered list items as `clippy::doc_lazy_continuation`. Sub-bullets must be indented (`   - 3c. …`) or renumbered. Local `cargo check` passed but CI failed.
4. **Test-mock alignment.** Adding new `lucide-react` icons (`Bell`, `Plus`) requires updating the `vi.mock('lucide-react', …)` block in `src/components/settings/tabs/SettingsTabs.test.tsx`, otherwise unrelated GeneralTab tests fail at import time.
5. **The user's predictive QA cycle.** Lance described what would happen *next* before it happened ("MeedyaDL will eventually start logging errors again") — and was right. When users describe an architectural symptom prospectively, take it as evidence for the next-deeper layer rather than waiting for proof.

## Why save this

These six fixes are not derivable from the code alone — the rationale (especially the macOS notification permission quirk and the sync-in-async cancellation pattern) lives in PR descriptions and commit bodies that get less searchable over time. Pinning the architectural pattern + the live PR state in shared memory means next-session Claude (or any teammate) can pick this up without re-reading every commit.

## How to apply

When the user references "the toast/notification/fallback/traceback/revival/companion fixes", default to assuming PR #662. If they say "still seeing the timeout error" against v0.50.1, gently remind them the fix has not yet shipped. Once the PR merges and a release lands, archive this memory file (don't delete — keeps the architectural pattern findable).
