---
name: Internal helpers from audits v1 + v2
description: Catalogue of project-internal primitives — withErrorToast, useConfirmation, useSettingsField, useAsyncTask, walk_dir_depth, atomic_write, http_client, subprocess_reader, context_err!, fs_walk, fixtures, createAsyncResourceStore. Reach for these instead of hand-rolling the same shape.
type: project
---
Two consolidation audits (v1: `.github/audits/codebase-unification-audit-v1.md`, v2: `.github/audits/codebase-unification-audit-v2.md`) produced 16 findings total. As of 2026-05-10 every actionable finding has shipped. The reusable primitives below are now project-standard — new code should adopt them rather than reimplement the underlying pattern.

## Backend (Rust)

| Primitive | File | Use for |
| --- | --- | --- |
| `walk_dir_depth(base, max_depth, visitor) -> Vec<T>` | `src-tauri/src/utils/fs_walk.rs` | Collect-all recursive directory walks. `max_depth` is mandatory — picks 3 for album-scoped walks, 10 for library-root scans. |
| `walk_dir_find_first(base, max_depth, visitor) -> Option<T>` | `src-tauri/src/utils/fs_walk.rs` | Find-first early-termination walks (e.g. locate a binary in an extracted archive). |
| `build_client(ClientConfig)` + `build_simple(timeout_secs)` | `src-tauri/src/utils/http_client.rs` | Centralised `reqwest::Client` construction. Don't call `Client::builder()...build()` inline. |
| `atomic_write_json(path, data, context)` | `src-tauri/src/utils/atomic_write.rs` | Durable JSON writes (settings.json, queue.json, manifest.meedyadl, etc.) — serialize, write tmp, rename. Don't roll your own. |
| `spawn_line_reader(stream, async_visitor) -> JoinHandle<()>` | `src-tauri/src/utils/subprocess_reader.rs` | The truly-common shell at every subprocess reader site (`BufReader → next_line loop → visitor`). Engine_runner uses it; Votify (M9) and yt-dlp (M10) should adopt it from day one. |
| `context_err!(result, "message {format}")?` macro | `src-tauri/src/utils/error_context.rs` | Replaces `.map_err(\|e\| format!("...: {e}"))?` in `commands/`. `#[macro_export]` so the call site is `crate::context_err!`. |

## Frontend (TypeScript / React)

| Primitive | File | Use for |
| --- | --- | --- |
| `createAsyncResourceStore<T>(config)` | `src/lib/createAsyncResourceStore.ts` | Zustand factory for IPC-backed `data` + `isLoading` + `isDirty` + `error` resources with `load`/`save`/`debouncedSave`/`update`/`reset` actions. Use for new stores; existing 6 stores opt-in incrementally. |
| `withErrorToast(fn, opts) -> Promise<T \| undefined>` | `src/lib/withErrorToast.ts` | `try { await ipc(); addToast(success) } catch { addToast(error) }` shape. Supports static / function-typed `errorMsg`, optional `successMsg` + `successVariant`, `suppressOn` substring patterns for expected-cancellation paths. |
| `useConfirmation({ title, description, confirmLabel?, onConfirm }) -> { open, modal }` | `src/lib/useConfirmation.tsx` | Confirmation modal hook — Title + body + Cancel/Confirm buttons. `description` accepts `ReactNode` so callers can include item details, "don't ask again" checkboxes bound to parent state, etc. Auto-closes on success; stays open if `onConfirm` throws. |
| `useSettingsField<K extends keyof AppSettings>(key) -> { value, set }` | `src/hooks/useSettingsField.ts` | Read + write ONE settings field. Replaces the per-control `settings.X` + `(v) => updateSettings({ X: v })` lambda pair. Direct enabler for M8/M9/M10 per-service settings tabs. |
| `useAsyncTask<TArgs, TResult>(fn) -> { run, isRunning, error }` | `src/hooks/useAsyncTask.ts` | Component-local sibling of `createAsyncResourceStore`. Single async fn with `isRunning` + `error` state. Composes with `withErrorToast` for toast + state combined. Captures fn by ref so hoisting order doesn't bite. |
| `makeFixture<T>(defaults)` + `makeQueueItem` / `makeActivityEntry` / `makeScannedManifest` | `src/testing/fixtures.ts` | Test fixture builders. New tests should reach for these instead of inline `function makeX(overrides)` boilerplate. |

## Why this catalogue exists

Without it, future session work re-discovers each pattern by grep and may re-implement the same shape inline. The 16 audit findings explicitly identified these as "I keep writing this same code" — every primitive replaces 5-50+ scattered call sites.

## How to apply

When writing new code:

- Before adding a `try { await … } catch { addToast(…) }` block, reach for `withErrorToast`.
- Before adding a confirmation modal, reach for `useConfirmation`.
- Before wiring a new settings form control, reach for `useSettingsField`.
- Before adding a new recursive directory walker, reach for `walk_dir_depth` / `walk_dir_find_first`.
- Before writing `serialize → write tmp → rename`, reach for `atomic_write_json`.
- Before calling `reqwest::Client::builder()` inline, reach for `build_client` / `build_simple`.
- Before writing a tokio reader for a subprocess stream, reach for `spawn_line_reader`.
- Before defining an inline test fixture builder, reach for `makeFixture` or one of the named builders.

Audit doc finding numbers don't matter day-to-day — the file paths above are the durable interface. The audit docs are the historical justification.

## Deferred audit work (won't auto-implement)

- v1 #3 — CLI argument builder pattern (well-understood, low priority)
- v1 #4 — Per-service settings UI tabs (waits for M8/M9/M10 service work)
- v1 #6 — TypeScript Tauri IPC wrappers codegen (high effort, low priority)
- v1 #7 — Engine command builder + pip-engine sharing (waits for M9/M10)
- Migration of EXISTING `src/stores/*.ts` to `createAsyncResourceStore` (would touch 30+ component consumers per store; deferred unless asked)
