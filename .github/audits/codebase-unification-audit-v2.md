# Codebase Unification Audit — v2

**Date:** 2026-05-09
**Scope:** Second consolidation pass after v1 closed out (#716). Looks at patterns that emerged or remained unaddressed across `v1.0.4`–`v1.0.9` work, with a forward-looking lens toward M8 (BBC iPlayer), M9 (Spotify), M10 (YouTube).

## v1 status recap

| # | Finding                                            | Status                                |
| - | -------------------------------------------------- | ------------------------------------- |
| 1 | Recursive directory walkers                        | **Done** — `utils/fs_walk.rs` (v1.0.9) |
| 2 | HTTP client builder boilerplate                    | **Done** — `utils/http_client.rs` (v1.0.4) |
| 3 | CLI argument builder pattern                       | **Deferred** (well-understood, not pressing) |
| 4 | Per-service settings structure                     | **Partial** — `PerServiceSettings` landed in #107; UI tabs deferred to M8/M9/M10 |
| 5 | Frontend Zustand store load/save factory           | **Done** — `src/lib/createAsyncResourceStore.ts` (v1.0.7) |
| 6 | TypeScript Tauri IPC wrappers (codegen)            | **Deferred** (high effort, low priority) |
| 7 | Engine command builder + pip-engine sharing        | **Deferred** to M9/M10 |
| 8 | JSON manifest atomic write                         | **Done** — `utils/atomic_write.rs` (v1.0.4) |

Findings #3, #4, #6, #7 remain documented in v1; this v2 audit does not relitigate them.

---

## 1. Async error → toast emission shape — issue: TBD

**Locations:**

- [`components/settings/SettingsPage.tsx:233-239`](../../src/components/settings/SettingsPage.tsx#L233) — save flow
- [`components/settings/tabs/GeneralTab.tsx:323-350`](../../src/components/settings/tabs/GeneralTab.tsx#L323) — import/export flows
- [`components/settings/tabs/CrashReportSection.tsx:119-134`](../../src/components/settings/tabs/CrashReportSection.tsx#L119) — delete flow
- [`components/layout/StatusBar.tsx:161-162`](../../src/components/layout/StatusBar.tsx#L161) — abort flow (chained `.catch` variant)
- ~30 other call sites across the component tree

**Pattern:** `try { await ipc(); addToast('success', 'success') } catch (e) { addToast(msg(e), 'error') }`. Variants include the chained `.catch(e => addToast(...))` form and a `finally { setLoading(false) }` block. Per-call boilerplate is 3-5 lines that say nothing the call site cares about.

**Consolidation:** Frontend hook `useAsyncWithToast(fn, { successMsg, errorContext })` or a free helper `withErrorToast<T>(fn, opts) -> Promise<T | undefined>` that wraps try/catch + toast emission. Either signature short-circuits to `undefined` on error so the caller's success branch can early-return.

**Size:** ~20 line helper + ~3 lines per call site (saves ~80 LOC across ~30 call sites). **Risk:** low — wraps existing pattern, zero behavioural change.

**Multi-service relevance:** **High** — every per-service settings tab (M8/M9/M10) will hit this shape for credential validation, "test connection" buttons, etc.

---

## 2. Local `isLoading` + `error` state outside `createAsyncResourceStore` — issue: TBD

**Locations:**

- [`components/download/DownloadForm.tsx`](../../src/components/download/DownloadForm.tsx) — local `isChecking` + `cookieError` state for preflight chain
- [`components/settings/tabs/AdvancedTab.tsx:~452-459`](../../src/components/settings/tabs/AdvancedTab.tsx#L452) — local async-action state for one-shot operations
- [`components/settings/tabs/GeneralTab.tsx:~283-330`](../../src/components/settings/tabs/GeneralTab.tsx#L283) — channel-switch pending + error
- ~8-10 other components with `useState(false)` + `useState<string | null>(null)` for the same pair

**Pattern:** The `createAsyncResourceStore` factory landed in v1.0.7 covers store-level resources, but components that only need a one-shot `isLoading + error + try/finally` shape still hand-roll it.

**Consolidation:** A lightweight companion hook `useAsyncTask<T>(fn, opts) -> { run, isRunning, error }` that's the single-operation equivalent of `createAsyncResourceStore`. Or: document in a `src/lib/createAsyncResourceStore.ts` header comment when each pattern applies (store for shared, hook for component-local).

**Size:** ~25 line hook + ~5 lines per component (saves ~150 LOC across ~10 sites). **Risk:** medium — needs case-by-case review to ensure local `isLoading` isn't coupled with other component state in ways that break under the hook's lifecycle.

**Multi-service relevance:** **Medium** — service-tab one-off actions (validate API key, test connection) fit this shape exactly.

---

## 3. Confirmation modal factory — issue: TBD

**Locations:**

- [`components/download/DownloadQueue.tsx:740-803`](../../src/components/download/DownloadQueue.tsx#L740) — Retry All / Clear All / Abort Queue / Delete Item modals (4 modals, ~250 LOC of inline JSX)
- [`components/library/MvGapFillModal.tsx`](../../src/components/library/MvGapFillModal.tsx) — gap-fill confirmation (already extracted as a per-feature modal)
- [`components/settings/tabs/CrashReportSection.tsx`](../../src/components/settings/tabs/CrashReportSection.tsx) — crash-report send confirmation
- Implicit "Are you sure?" patterns inline in onClick handlers across other tabs

**Pattern:** Every destructive action shows a confirmation modal with the same shape: title, description, primary "Yes, do it" button, secondary "Cancel" button, optional "don't ask again" checkbox. Each one is owned and re-implemented by the parent component.

**Consolidation:** `useConfirmation({ title, description, destructive?, dontAskAgainKey?, onConfirm }) -> { show, modal }` — call the hook once per action; render the returned `modal` JSX once in the component; call `show()` from the trigger button. Centralises the chrome (title styling, button order, accessibility, focus-trap inheritance from `Modal.tsx`).

**Size:** ~50 line hook + Modal wrapper, eliminates ~30-40 LOC per call site (~150 LOC across DownloadQueue alone). **Risk:** low — isolated UI primitive, zero backend touch.

**Multi-service relevance:** **Medium** — service-specific destructive actions (revoke credentials, clear service cache, abort service downloads) all need the same gate.

---

## 4. Test fixture builders — issue: TBD

**Locations:**

- [`components/download/DownloadQueue.test.tsx:57-79`](../../src/components/download/DownloadQueue.test.tsx#L57) — `makeItem(overrides)` for `QueueItemStatus`
- [`components/download/ActivityLog.test.tsx:82-90`](../../src/components/download/ActivityLog.test.tsx#L82) — `makeEntry(overrides)` for `ActivityLogEntry`
- [`components/library/MvGapFillModal.test.tsx`](../../src/components/library/MvGapFillModal.test.tsx) — inline `baseManifest` for `ScannedManifest`
- Implicit pattern in store tests

**Pattern:** Each test file with a fixture defines its own builder factory with hardcoded defaults. The builders work fine but duplicate the type's defaults across files; updating a type means walking N test files.

**Consolidation:** `src/testing/fixtures.ts` (or `src/__tests__/fixtures.ts`) re-exporting `makeQueueItem`, `makeActivityEntry`, `makeScannedManifest`, plus a generic `makeFixture<T>(defaults: T): (overrides?: Partial<T>) => T` helper for ad-hoc fixtures.

**Size:** ~80 line module + removal of per-file factories (~150 LOC across current tests, scales with future test count). **Risk:** low — test-only infrastructure, additive.

**Multi-service relevance:** **Low-medium** — each new service's tests will need their own fixture builders; centralised generic helper makes this 5 LOC instead of 25.

---

## 5. Subprocess output reader duplication — issue: TBD

**Locations:**

- [`services/engine_runner.rs:148-196`](../../src-tauri/src/services/engine_runner.rs#L148) — stdout + stderr readers, two near-identical `tokio::spawn` blocks
- [`services/companion_supervisor.rs:144-186`](../../src-tauri/src/services/companion_supervisor.rs#L144) — stdout + stderr readers, same shape
- [`services/download_queue.rs`](../../src-tauri/src/services/download_queue.rs) — primary download readers, same shape (3rd location)

**Pattern:** Each subprocess that's spawned in MeedyaDL has a stdout reader task and a stderr reader task. Both tasks: read line, call `parse_gamdl_output(line)`, dispatch on `GamdlOutputEvent` variants, emit Tauri events, update atomic flags, check shutdown signal. The variance between stdout and stderr is purely the stream label.

**Consolidation:** `spawn_subprocess_reader(stream_label, stream, parser, emitter, shutdown_signal) -> JoinHandle<()>` in `utils/subprocess_io.rs` (or extend `engine_runner.rs`). Trait-based parser callback so future engines (yt-dlp, Votify) plug in their own output classifier without re-implementing the reader-task chrome.

**Size:** ~60 line abstraction + 10-15 lines per reader pair (saves ~120 LOC across the 3 current sites). **Risk:** medium — affects subprocess hot path; needs cancellation-flag + shutdown-signal regression tests.

**Multi-service relevance:** **High** — yt-dlp (M8 BBC iPlayer + M10 YouTube) and Votify (M9 Spotify) both need their own output parsers; the reader chrome is identical.

---

## 6. Settings tab boilerplate — issue: TBD

**Locations:**

- All 10 tabs in [`components/settings/tabs/`](../../src/components/settings/tabs/) — GeneralTab, QualityTab, MetadataTab, LyricsTab, FallbackTab, CoverArtTab, AdvancedTab, ToolsTab, CookiesTab, AuthenticationTab

**Pattern:** Every tab opens with the same three lines:

```ts
const settings = useSettingsStore((s) => s.settings);
const updateSettings = useSettingsStore((s) => s.updateSettings);
// ... and every form control wires onChange={(v) => updateSettings({ field: v })}
```

10 tabs × 3-5 lines of boilerplate × ~20 form controls per tab = significant duplicated wiring. The `useSettingsStore` selector pattern itself is correct — the issue is the per-control `updateSettings({ key: v })` lambda.

**Consolidation:** A `useSettingsField<K extends keyof AppSettings>(key: K)` hook returning `[value, setValue]`. Form controls become `<Toggle {...useSettingsField('auto_check_updates')} />` instead of `<Toggle checked={settings.auto_check_updates} onChange={(v) => updateSettings({ auto_check_updates: v })} />`. Or: a `<SettingsField field="auto_check_updates" as={Toggle} />` wrapper.

**Size:** ~30 line hook + ~5-10 lines saved per form control (~400-800 LOC across 10 tabs × ~20 controls). **Risk:** low — additive, opt-in per call site.

**Multi-service relevance:** **Very High** — M8/M9/M10 each land a per-service settings tab (BBC iPlayer session, Spotify credentials, YouTube API config). Each tab will replicate the same wiring; the hook makes per-service tabs trivial to write.

---

## 7. Tauri command error-wrapping pattern — issue: TBD

**Locations:**

- [`commands/credentials.rs:63, 67, 74, 169, 176`](../../src-tauri/src/commands/credentials.rs) — `.map_err(|e| format!("Failed to ...: {e}"))?` (5+ instances)
- [`commands/system.rs`](../../src-tauri/src/commands/system.rs) — same pattern, ~6 instances
- ~30-40 instances across `commands/` total

**Pattern:** Command handlers consistently wrap errors with a context message via `.map_err(|e| format!("Context: {e}"))?` to satisfy Tauri's `Result<T, String>` ergonomics. The pattern is correct and consistent — but it's verbose, and the wrap-with-context idiom is one line of plumbing per failable call.

**Consolidation:** A `context_err!("message", err_or_result)` macro — expands to `.map_err(|e| format!("message: {e}"))?`. Or migrate commands from `Result<T, String>` to `Result<T, CommandError>` where `CommandError` is a thin enum with `From<E>` impls; serialise to a structured object on the wire.

**Size:** ~15 line macro definition; saves ~5 LOC across each error site. Enum approach is larger (~50 LOC infra + per-command refactor) but unlocks structured logging. **Risk:** macro is low-risk; enum migration is medium.

**Multi-service relevance:** **Medium** — every new service command will hit the same shape. Macro applied uniformly keeps things consistent.

---

## 8. State&lt;T&gt; declaration density in command signatures — issue: TBD (documentation-only)

**Locations:**

- [`commands/gamdl.rs:97-122`](../../src-tauri/src/commands/gamdl.rs#L97) — `queue: State<'_, QueueHandle>` repeated across ~15 commands
- Multiple-state commands declare `queue: State<'_, ...>, config: State<'_, ...>` etc.

**Pattern:** Tauri's DI is per-parameter, so every command declares its state injections explicitly. This is necessary boilerplate, not consolidatable through refactoring (Tauri's macro requires the parameter signature).

**Consolidation:** **Documentation, not refactor.** Add a `commands/README.md` (or top-of-`commands/mod.rs` doc) that documents the canonical State injection pattern + naming conventions, so M8/M9/M10 commands stay consistent. Optionally, prototype a `#[mwbm_command(state = [queue, config])]` proc macro that expands to the full signature — but the cost-benefit is poor for the LOC saved.

**Size:** 0 LOC (docs only). **Risk:** none. **Multi-service relevance:** **Very High** — keeps new service commands stylistically aligned without enforcement burden.

---

## Recommended implementation order

| Order | Finding                                  | Why                                                 | Cycle target |
| ----- | ---------------------------------------- | --------------------------------------------------- | ------------ |
| 1     | #4 Test fixture builders                 | Backward-compat, test-only, immediate quality win   | next cycle   |
| 2     | #1 `useAsyncWithToast` helper            | Highest LOC impact, lowest risk, broadest reuse     | next cycle   |
| 3     | #3 Confirmation modal factory            | Isolated UI primitive, immediate DownloadQueue win  | next cycle   |
| 4     | #6 Settings field hook                   | Direct enabler for M8/M9/M10 service tabs           | before M8    |
| 5     | #5 Subprocess reader abstraction         | Direct enabler for yt-dlp / Votify reader plumbing  | before M9    |
| 6     | #8 State-injection docs                  | Knowledge base for new backends; trivial            | before M8    |
| 7     | #2 `useAsyncTask` hook                   | Useful but needs scope decision (vs documenting split) | TBD       |
| 8     | #7 `context_err!` macro                  | Nice-to-have polish; current pattern is sound        | maintenance |

## Summary

| #   | Title                                  | LOC impact | Risk        | Multi-service relevance |
| --- | -------------------------------------- | ---------- | ----------- | ----------------------- |
| 1   | useAsyncWithToast                      | ~80        | Low         | High                    |
| 2   | useAsyncTask hook                      | ~150       | Medium      | Medium                  |
| 3   | Confirmation modal factory             | ~150       | Low         | Medium                  |
| 4   | Test fixture builders                  | ~150       | Low         | Low-medium              |
| 5   | Subprocess reader abstraction          | ~120       | Medium      | High                    |
| 6   | Settings field hook                    | ~400-800   | Low         | **Very High**           |
| 7   | context_err! macro                     | ~30        | Low-medium  | Medium                  |
| 8   | State-injection docs                   | 0 (docs)   | None        | **Very High**           |
