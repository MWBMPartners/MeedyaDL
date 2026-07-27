// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file featureFlagStore — Zustand store for the remote feature-flag
 * snapshot (see `src-tauri/src/models/feature_flags.rs`, commit c4a2185b,
 * and its TypeScript mirror `src/types/feature-flags.ts`).
 *
 * Built on `createAsyncResourceStore` (`src/lib/createAsyncResourceStore.ts`)
 * as a **read-only** resource (no `save` config supplied) — per that
 * factory's contract, `save()` / `debouncedSave()` on the resulting store
 * are no-ops, and `isDirty` never becomes meaningfully true.
 *
 * `load()` is wired to the `refresh_feature_flags` IPC (a real network
 * attempt, rate-limited to 1/min on the backend) rather than the cheap
 * `get_feature_flags` local read. That matches how `App.tsx` uses this
 * store: the startup + periodic-refresh effect calls
 * `useFeatureFlagStore.getState().load()` in the exact shape of the
 * existing `checkForUpdates()` effect (see Effect 2 / Effect 4 in
 * `App.tsx`), and that call is expected to actually reach the network.
 * `getFeatureFlags()` (the cheap local-only read) is still exported from
 * `tauri-commands.ts` for a future developer-tools panel, but nothing in
 * this store calls it — there's no cheap-read consumer yet.
 *
 * ## CRITICAL invariant — `meta` never drives UI
 *
 * Every consumer of this store (today: `FeatureNoticeBanner`) must derive
 * its rendering ONLY from `data.verdicts`. `data.meta` (source /
 * consecutive_failures / last_error) is diagnostics-only — see the
 * doc comment on `FeatureFlagsSnapshot` in the Rust model. A failed
 * `load()` call (e.g. the backend's own rate limit rejecting it, or a
 * transport error surfacing through the IPC boundary itself) must never
 * produce a toast, banner, or any other on-screen artefact; it is logged
 * to the console here for local debugging only, and the store's `data`
 * simply stays at its last-known-good value (the async resource store's
 * `load()` only replaces `data` on success — see
 * `createAsyncResourceStore.ts`).
 */

import { createAsyncResourceStore } from '@/lib/createAsyncResourceStore';
import { refreshFeatureFlags } from '@/lib/tauri-commands';
import type { FeatureFlagsSnapshot, FlagVerdict } from '@/types/feature-flags';

/**
 * Compiled-in empty snapshot — mirrors `FeatureFlagsSnapshot::default()`
 * on the Rust side (empty verdicts map => every feature enabled, `meta`
 * sourced from `FlagSource::Defaults`). Used before the first `load()`
 * resolves and by `reset()`.
 */
const DEFAULT_SNAPSHOT: FeatureFlagsSnapshot = {
  generated_at: null,
  verdicts: {},
  meta: {
    source: 'defaults',
    fetched_at: null,
    consecutive_failures: 0,
    last_error: null,
  },
};

/**
 * Read-only async resource store wrapping the feature-flag snapshot.
 * Components should subscribe via `useFeatureFlagStore((s) => s.data)`
 * and derive notice-worthy entries with {@link selectNoticeEntries} —
 * never read `.data.meta` for anything user-visible.
 */
export const useFeatureFlagStore = createAsyncResourceStore<FeatureFlagsSnapshot>({
  defaults: DEFAULT_SNAPSHOT,
  load: () => refreshFeatureFlags(),
});

/** One flag key + its resolved verdict, for entries worth surfacing to the user. */
export interface FeatureNoticeEntry {
  key: string;
  verdict: FlagVerdict;
}

/**
 * Pure selector: given a snapshot, returns the verdicts that should
 * surface a notice to the user — either the feature has been switched
 * off (`verdict.enabled === false`), or the server attached a notice to
 * an otherwise-enabled feature (`verdict.notice != null`, e.g. a
 * maintenance heads-up on a feature that still works).
 *
 * Exported as a standalone pure function (not only reachable through the
 * Zustand hook) so it is unit-testable without mounting React or the
 * store. Deliberately reads ONLY `snapshot.verdicts` — never
 * `snapshot.meta` — holding the same invariant documented above.
 */
export function selectNoticeEntries(snapshot: FeatureFlagsSnapshot): FeatureNoticeEntry[] {
  return Object.entries(snapshot.verdicts)
    .filter(([, verdict]) => verdict.enabled === false || verdict.notice != null)
    .map(([key, verdict]) => ({ key, verdict }));
}

/**
 * Pure selector: is the feature behind `flagKey` currently available?
 *
 * **A missing key means enabled.** That is the same fail-open rule as
 * `feature_flag_service::is_enabled` on the Rust side, and it is what keeps
 * a build newer than the server's flag set, a fork with no credentials, and
 * a fresh install that has never reached the network all fully functional.
 *
 * Reads ONLY `snapshot.verdicts` — never `snapshot.meta`, per the invariant
 * documented above. A background refresh that failed must be invisible here.
 *
 * Consumers pass a service's flag key (`service-apple-music`,
 * `service-spotify`, …). Note that this is a **courtesy** check: the Rust
 * command handlers re-check authoritatively at every enqueue seam, because
 * deep links, the clipboard monitor and queue import never pass through the
 * React form.
 */
export function selectServiceEnabled(
  snapshot: FeatureFlagsSnapshot,
  flagKey: string
): boolean {
  const verdict = snapshot.verdicts[flagKey];
  return verdict === undefined || verdict.enabled !== false;
}
