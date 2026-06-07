// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Inline-SVG renderer for a service-platform icon (#911-1).
 *
 * Fetches the SVG once per icon path, sanitises (defence-in-depth
 * script + event-handler stripping — Tauri's CSP already blocks
 * inline scripts), then caches in module memory. The SVG is rendered
 * via `dangerouslySetInnerHTML` so `currentColor` inherits the parent
 * CSS context's text colour.
 *
 * Co-located with `platform-config.ts` (helpers / cache) but kept in
 * its own `.tsx` file so Vite's fast-refresh doesn't trip on a
 * component+utility co-export.
 *
 * **Brand-colour separation (#911 anti-pattern 2):** this component
 * does NOT apply a service brand colour. Status pills (green/amber/red)
 * and brand colours must use distinct hue ranges so deuteranopia users
 * can tell "Spotify queued" from "Spotify failed" — see
 * `.claude/memory/project_multi_service_ui_direction.md`. Wrap this
 * component in a brand-coloured container when (and only when) you
 * specifically want the brand tint.
 */

import { useEffect, useState } from 'react';

import type { PlatformEntry } from './platform-config';

const svgCache = new Map<string, string>();

/**
 * Render the SVG icon for `platform`. Returns `null` when `platform` is
 * undefined (no match) or the icon hasn't loaded yet.
 *
 * `size` sets both width and height in pixels. Defaults to `16` (the
 * size GlobalProgressBar uses); queue rows pass `20`.
 */
export function PlatformIcon({
  platform,
  size = 16,
}: {
  platform: PlatformEntry | undefined;
  size?: number;
}) {
  const [svgHtml, setSvgHtml] = useState<string | null>(
    () => (platform?.icon ? svgCache.get(platform.icon) ?? null : null)
  );
  const [useFallback, setUseFallback] = useState(false);

  useEffect(() => {
    if (!platform || !platform.icon) {
      if (platform) setUseFallback(true);
      return;
    }
    if (svgCache.has(platform.icon)) {
      setSvgHtml(svgCache.get(platform.icon)!);
      return;
    }
    const iconUrl = platform.icon.startsWith('/')
      ? platform.icon
      : `/${platform.icon}`;
    fetch(iconUrl)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.text();
      })
      .then((text) => {
        if (text.includes('<svg')) {
          const parser = new DOMParser();
          const doc = parser.parseFromString(text, 'image/svg+xml');
          const svgEl = doc.querySelector('svg');
          if (!svgEl) {
            setUseFallback(true);
            return;
          }
          doc.querySelectorAll('script').forEach((s) => s.remove());
          doc.querySelectorAll('*').forEach((el) => {
            for (const attr of [...el.attributes]) {
              if (attr.name.startsWith('on')) {
                el.removeAttribute(attr.name);
              }
            }
          });
          svgEl.setAttribute('width', '100%');
          svgEl.setAttribute('height', '100%');
          svgEl.setAttribute('style', 'display:block');
          const sanitized = svgEl.outerHTML;
          svgCache.set(platform.icon, sanitized);
          setSvgHtml(sanitized);
        } else {
          setUseFallback(true);
        }
      })
      .catch(() => setUseFallback(true));
  }, [platform]);

  if (!platform) return null;

  if (svgHtml) {
    return (
      <span
        className="flex-shrink-0 inline-flex items-center justify-center text-content-secondary [&>svg]:w-full [&>svg]:h-full"
        style={{ width: size, height: size }}
        aria-label={platform.name}
        title={platform.name}
        dangerouslySetInnerHTML={{ __html: svgHtml }}
      />
    );
  }

  if (useFallback && platform.icon) {
    return (
      <img
        src={platform.icon}
        alt={platform.name}
        title={platform.name}
        width={size}
        height={size}
        className="flex-shrink-0"
      />
    );
  }

  return null;
}
