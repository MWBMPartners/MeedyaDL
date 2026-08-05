// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file TypeScript mirror of the Rust remote feature-flag model.
 *
 * Mirrors `src-tauri/src/models/feature_flags.rs` (commit c4a2185b) field
 * for field. Keep this file in lockstep with that module — a drift here
 * is a silent runtime mismatch, not a compile error, because the Tauri
 * IPC boundary is serde JSON with no shared schema check.
 *
 * ## Server-side evaluation
 *
 * The server returns already-resolved booleans; the client performs no
 * condition evaluation of its own. See the Rust module's header comment
 * for the full rationale.
 *
 * ## State separation between `verdicts` and `meta` — CRITICAL
 *
 * `verdicts` is the only part of `FeatureFlagsSnapshot` the UI may render
 * or act on. `meta` is diagnostics ONLY (where this snapshot came from,
 * when it was fetched, how many consecutive refreshes have failed) and
 * MUST NEVER drive anything user-visible — no toast, no banner, no error
 * dialog, no disabled control. A user whose network is down must see the
 * app behave exactly as it did on the last successful fetch; the failure
 * is logged by the backend to the Activity Log, not surfaced here. See
 * `src/stores/featureFlagStore.ts` and
 * `src/components/common/FeatureNoticeBanner.tsx` for the consumers that
 * hold this invariant.
 *
 * ## Optionality on the wire
 *
 * Rust `Option<T>` fields serialise to `T | null` (never omitted — none of
 * these structs use `skip_serializing_if`), so every optional field here
 * is typed `T | null`, not `T | undefined`.
 */

/**
 * Presentation weight for a {@link FlagNotice}. Mirrors
 * `NoticeSeverity` (serialised lowercase via `#[serde(rename_all =
 * "lowercase")]`). `'unknown'` is the `#[serde(other)]` catch-all for any
 * severity string a future server invents that this build doesn't
 * recognise yet — render it as `'info'` (the mildest treatment), matching
 * `NoticeSeverity::render_as` on the Rust side. Never invent additional
 * colours/severities client-side; the a11y colour-blind themes key off
 * the closed `info` / `warning` / `error` status token set.
 */
export type NoticeSeverity = 'info' | 'maintenance' | 'deprecated' | 'critical' | 'unknown';

/**
 * Which tier of the three-tier resolution chain produced a snapshot.
 * Mirrors `FlagSource` (serialised snake_case). **Diagnostics only** —
 * see the state-separation note above. Never branch UI on this value.
 */
export type FlagSource = 'remote' | 'disk_cache' | 'defaults';

/**
 * A server-authored message about a feature, shown alongside the feature
 * when it is disabled or degraded. Mirrors `FlagNotice`.
 *
 * `message` is untrusted remote text — render it ONLY as a plain text
 * child, never through a markdown renderer or `dangerouslySetInnerHTML`.
 * `url` must not be rendered/opened by this package at all (needs scheme
 * validation first — a follow-up); see the Rust struct's doc comment.
 */
export interface FlagNotice {
  message: string;
  severity: NoticeSeverity;
  url: string | null;
}

/**
 * The server's resolved answer for a single flag key. Mirrors
 * `FlagVerdict`.
 *
 * Note the asymmetry with a missing key in
 * {@link FeatureFlagsSnapshot.verdicts}: an *absent* key means enabled
 * (fail-open), whereas a *present* verdict with `enabled: false` is a
 * deliberate instruction to switch the feature off.
 */
export interface FlagVerdict {
  enabled: boolean;
  label: string | null;
  notice: FlagNotice | null;
  updated_at: string | null;
}

/**
 * Diagnostics about how the current snapshot was obtained. Mirrors
 * `FetchMeta`.
 *
 * **Never user-visible.** See the state-separation note in this file's
 * header. Present on the IPC surface so a future developer-tools panel
 * (gated behind the Konami dev-access sentinel) can show it.
 */
export interface FetchMeta {
  source: FlagSource;
  fetched_at: string | null;
  consecutive_failures: number;
  last_error: string | null;
}

/**
 * A resolved snapshot of every feature flag this build knows about.
 * Mirrors `FeatureFlagsSnapshot`.
 *
 * See the state-separation note in this file's header: `verdicts` is the
 * only field the UI may render or act on; `meta` is diagnostics-only.
 */
export interface FeatureFlagsSnapshot {
  generated_at: string | null;
  /**
   * Flag key -> resolved verdict. Keys are dot-namespaced
   * (`"core.updater"`, `"service.apple-music"`, ...). A missing key
   * means ENABLED — the empty object is the "everything on" state.
   */
  verdicts: Record<string, FlagVerdict>;
  meta: FetchMeta;
}
