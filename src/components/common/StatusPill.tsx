// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Status pill (#911-4) — coloured rounded-full with icon + label,
 *       used in the queue row Tier 1 status column and reusable in
 *       History rows, Library Scan rows, and the future Cmd-K results
 *       panel (per the per-row vocabulary policy in `.claude/memory/
 *       project_multi_service_ui_direction.md`).
 *
 * Replaces the previous icon-only status indicator with a more scan-able
 * pill — colour + icon + label — at no extra horizontal cost. The pill
 * uses a soft (15% opacity) background tint paired with a solid-colour
 * icon + label, matching the existing badge patterns used by
 * `ActivityLog.tsx` (filter chips) and the "Retry without Wrapper"
 * action pill in `QueueItem.tsx`.
 *
 * **Brand-colour separation (#911 anti-pattern 2):** status pills use
 * the `--status-*` token family (green/amber/red/blue/grey) and MUST
 * NOT use service brand colours. Brand colours (Apple red, Spotify
 * green, YouTube red, BBC pink) live on `--service-*` tokens (PR C —
 * #911-7) and are only consumed by `PlatformIcon`, never by status
 * indicators. This separation is what lets deuteranopia users tell
 * "Spotify queued" from "Spotify failed."
 */

import { AlertTriangle, CheckCircle, Clock, Download, Loader2, XCircle } from 'lucide-react';

import type { DownloadState } from '@/types';

/**
 * Visual configuration per state. Each entry pins:
 *  - the Lucide icon (`icon`)
 *  - the human-readable label (`label`)
 *  - the pill's colour token classes (`colorClasses`) — a triple of
 *    text colour + soft background + border, all pulling from the
 *    `--status-*` / `--content-*` token families so the pill adapts
 *    automatically to light / dark / high-contrast / colour-blind
 *    themes via the existing theme system.
 *
 * `complete-with-warnings` is a synthetic state derived at render time
 * from `state === 'complete' && warnings.length > 0`.
 */
const STATE_CONFIG: Record<
  DownloadState | 'complete-with-warnings',
  {
    icon: typeof Clock;
    label: string;
    /**
     * Colour classes for the pill's TEXT (the visible label word) and
     * its background tint / border. Uses the `-text` sibling of each
     * status colour (e.g. `text-status-success-text`, not
     * `text-status-success`): the plain colour is tuned for small fills
     * and icons, which only need to clear a 3:1 "UI component" contrast
     * floor, and falls short of the 4.5:1 that an actual readable word
     * needs -- e.g. plain `text-status-success` is only 2.22:1 as text
     * on a white background. See base.css for the full arithmetic
     * behind the two separate token sets.
     */
    colorClasses: string;
    /**
     * Colour class for the pill's ICON specifically, kept at the plain
     * (undarkened) status colour -- an icon is a small graphic, not
     * text, so it only needs the lower 3:1 bar the plain colour already
     * clears, and darkening it for no reason would just make the icon a
     * duller version of its usual colour for no accessibility benefit.
     */
    iconColorClass: string;
    /** Whether to apply `animate-spin` to the icon (processing state). */
    spinIcon?: boolean;
  }
> = {
  queued: {
    icon: Clock,
    label: 'Queued',
    colorClasses:
      'text-content-tertiary bg-content-tertiary/10 border-content-tertiary/25',
    iconColorClass: 'text-content-tertiary',
  },
  downloading: {
    icon: Download,
    label: 'Downloading',
    colorClasses: 'text-status-info-text bg-status-info/15 border-status-info/30',
    iconColorClass: 'text-status-info',
  },
  processing: {
    icon: Loader2,
    label: 'Processing',
    colorClasses:
      'text-status-warning-text bg-status-warning/15 border-status-warning/30',
    iconColorClass: 'text-status-warning',
    spinIcon: true,
  },
  complete: {
    icon: CheckCircle,
    label: 'Complete',
    colorClasses:
      'text-status-success-text bg-status-success/15 border-status-success/30',
    iconColorClass: 'text-status-success',
  },
  'complete-with-warnings': {
    icon: AlertTriangle,
    label: 'Warnings',
    colorClasses:
      'text-status-warning-text bg-status-warning/15 border-status-warning/30',
    iconColorClass: 'text-status-warning',
  },
  error: {
    icon: XCircle,
    label: 'Error',
    colorClasses: 'text-status-error-text bg-status-error/15 border-status-error/30',
    iconColorClass: 'text-status-error',
  },
  cancelled: {
    icon: XCircle,
    label: 'Cancelled',
    colorClasses:
      'text-content-tertiary bg-content-tertiary/10 border-content-tertiary/25',
    iconColorClass: 'text-content-tertiary',
  },
};

export interface StatusPillProps {
  /**
   * Download state to render. The pill maps each state to a fixed
   * icon, label, and colour scheme via the internal `STATE_CONFIG`
   * table.
   */
  state: DownloadState;
  /**
   * Whether this `complete` item finished with non-fatal warnings.
   * When `true`, the pill switches from green ("Complete") to amber
   * ("Warnings") and surfaces an `AlertTriangle` icon. Ignored for
   * non-`complete` states.
   */
  hasWarnings?: boolean;
  /**
   * Render the label inline alongside the icon. Defaults to `true`.
   * Set `false` when rendering inside ultra-compact rows (e.g.
   * dropdown row indicators) where the icon alone suffices and the
   * label would steal width. The label still appears via the
   * `aria-label` so screen readers always announce it.
   */
  showLabel?: boolean;
  /**
   * Pixel size for the icon. Defaults to `14` — matches the existing
   * inline-icon density on the queue row. Set higher when the pill
   * needs to read at a glance from further away (e.g. a status row
   * on a settings page).
   */
  iconSize?: number;
  /**
   * Optional extra classes (e.g. `data-testid` selectors, custom
   * margins). Appended after the built-in colour and shape classes.
   */
  className?: string;
}

/**
 * Render a coloured status pill (#911-4) for a single download.
 *
 * @example
 *   <StatusPill state={item.state} hasWarnings={!!item.warnings.length} />
 */
export function StatusPill({
  state,
  hasWarnings = false,
  showLabel = true,
  iconSize = 14,
  className,
}: StatusPillProps) {
  const effectiveState =
    state === 'complete' && hasWarnings ? 'complete-with-warnings' : state;
  const config = STATE_CONFIG[effectiveState];
  const Icon = config.icon;
  return (
    <div
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full border text-xs font-medium transition-colors ${config.colorClasses}${
        className ? ` ${className}` : ''
      }`}
      role="status"
      aria-label={config.label}
      title={config.label}
    >
      <Icon
        size={iconSize}
        className={`${config.iconColorClass}${config.spinIcon ? ' animate-spin' : ''}`}
        aria-hidden="true"
      />
      {showLabel && <span>{config.label}</span>}
    </div>
  );
}
