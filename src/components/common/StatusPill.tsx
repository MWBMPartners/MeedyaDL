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
    colorClasses: string;
    /** Whether to apply `animate-spin` to the icon (processing state). */
    spinIcon?: boolean;
  }
> = {
  queued: {
    icon: Clock,
    label: 'Queued',
    colorClasses:
      'text-content-tertiary bg-content-tertiary/10 border-content-tertiary/25',
  },
  downloading: {
    icon: Download,
    label: 'Downloading',
    colorClasses: 'text-status-info bg-status-info/15 border-status-info/30',
  },
  processing: {
    icon: Loader2,
    label: 'Processing',
    colorClasses:
      'text-status-warning bg-status-warning/15 border-status-warning/30',
    spinIcon: true,
  },
  complete: {
    icon: CheckCircle,
    label: 'Complete',
    colorClasses:
      'text-status-success bg-status-success/15 border-status-success/30',
  },
  'complete-with-warnings': {
    icon: AlertTriangle,
    label: 'Warnings',
    colorClasses:
      'text-status-warning bg-status-warning/15 border-status-warning/30',
  },
  error: {
    icon: XCircle,
    label: 'Error',
    colorClasses: 'text-status-error bg-status-error/15 border-status-error/30',
  },
  cancelled: {
    icon: XCircle,
    label: 'Cancelled',
    colorClasses:
      'text-content-tertiary bg-content-tertiary/10 border-content-tertiary/25',
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
        className={config.spinIcon ? 'animate-spin' : undefined}
        aria-hidden="true"
      />
      {showLabel && <span>{config.label}</span>}
    </div>
  );
}
