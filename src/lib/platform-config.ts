// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Shared service-platform detection helpers for queue rows, the
 *       global progress bar, history rows, and future cross-store
 *       search results (#911-1).
 *
 * Loads platform metadata from `engines.toml` once via IPC, then exposes
 *  - {@link detectPlatform} — match a URL against the registered patterns
 *  - {@link loadPlatformConfig} — kick off (or refresh) the IPC load
 *  - {@link subscribeToPlatformConfig} — re-render when the config arrives
 *
 * The matching SVG-icon renderer lives at `./PlatformIcon.tsx` so this
 * module stays a pure-utility `.ts` file (avoids Vite's
 * `react-refresh/only-export-components` warning when a `.tsx` file
 * exports both helpers and a component).
 *
 * Extracted from `GlobalProgressBar.tsx` (which previously owned the only
 * copy of `detectPlatform` + `PlatformIcon`) so the same identification
 * vocabulary works in every per-row context. See `.claude/memory/
 * project_multi_service_ui_direction.md` for the per-row vocabulary policy.
 *
 * Naming note: this file is `platform-config` (service / engine platform)
 * to avoid collision with the existing `usePlatform` hook (OS detection
 * — macOS / Windows / Linux). Per #911 verification report.
 */

/**
 * A single registered service platform loaded from `engines.toml`.
 *
 * The fields mirror the subset of `EngineRegistry::PlatformConfig`
 * (Rust) that the frontend needs for display:
 *
 *  - `id` — kebab-case identifier (e.g. `apple-music`, `spotify`)
 *  - `name` — human-readable label (e.g. `"Apple Music"`)
 *  - `icon` — absolute path to the inline SVG asset; empty string
 *    when no icon ships for this platform
 *  - `patterns` — substring patterns matched against
 *    `hostname + pathname` of the URL under test
 */
export interface PlatformEntry {
  id: string;
  name: string;
  icon: string;
  patterns: string[];
}

let platformConfig: PlatformEntry[] = [];
let configLoaded = false;
const configSubscribers: Array<() => void> = [];

/**
 * Lazily fetches `engines.toml` via IPC and populates the module-level
 * cache. Idempotent — subsequent calls during a successful load are
 * no-ops; subsequent calls AFTER a failed load retry the IPC.
 *
 * Notifies every {@link subscribeToPlatformConfig} listener once the
 * cache populates so React components re-render with the resolved data.
 */
export async function loadPlatformConfig(): Promise<void> {
  if (configLoaded) return;
  configLoaded = true;
  try {
    const { getPlatformConfig } = await import('@/lib/tauri-commands');
    const platforms = await getPlatformConfig();
    platformConfig = platforms
      .filter((p) => p.enabled)
      .map((p) => ({
        id: p.id,
        name: p.name,
        icon: p.icon ? `/${p.icon}` : '',
        patterns: p.url_patterns,
      }));
    for (const cb of configSubscribers) cb();
  } catch {
    // IPC not ready yet — leave configLoaded=false so the next caller retries.
    configLoaded = false;
  }
}

/**
 * Subscribe to the one-time "config loaded" event. Returns an
 * unsubscribe function suitable for `useEffect` cleanup.
 */
export function subscribeToPlatformConfig(cb: () => void): () => void {
  configSubscribers.push(cb);
  return () => {
    const idx = configSubscribers.indexOf(cb);
    if (idx >= 0) configSubscribers.splice(idx, 1);
  };
}

/**
 * Matches the first URL in a queue item against the loaded platform
 * patterns. Returns `undefined` when the URL doesn't parse, when no
 * pattern matches, or when the config hasn't loaded yet.
 *
 * The match runs on `hostname + pathname` to handle patterns like
 * `bbc.co.uk/iplayer` (which need the pathname portion to disambiguate
 * BBC iPlayer from BBC Sounds).
 */
export function detectPlatform(urls?: string[]): PlatformEntry | undefined {
  const raw = urls?.[0] ?? '';
  try {
    const parsed = new URL(raw);
    const urlStr = parsed.hostname + parsed.pathname;
    return platformConfig.find((p) =>
      p.patterns.some((pattern) => urlStr.includes(pattern))
    );
  } catch {
    return undefined;
  }
}
