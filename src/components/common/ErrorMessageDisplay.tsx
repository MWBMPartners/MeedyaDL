// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file ErrorMessageDisplay — truncated error text with tooltip + actions.
 *
 * Long download errors (notably GAMDL bug reports — see the
 * `"GAMDL bug — …"` prefix emitted by `download_queue.rs`) used to
 * truncate at the right edge of the History row with no way to read
 * the full text without resizing the window. This component:
 *
 * - Truncates the visible text to the configured number of lines via
 *   Tailwind `line-clamp-N`.
 * - Wraps in a {@link Tooltip} that surfaces the FULL message on
 *   hover/focus — works on the keyboard too.
 * - Adds a right-click context menu with:
 *   - **Copy error message** (always)
 *   - **Report this bug to GAMDL** (only when the message looks like
 *     a known upstream defect — recognised via the `"GAMDL bug "`
 *     prefix that `download_queue.rs` emits). Opens the upstream
 *     repo's `issues/new` URL with the title and body pre-filled
 *     from the failed URL + the error text. The pre-fill is
 *     written from a normal-GAMDL-user perspective: no MeedyaDL
 *     branding, no "via MeedyaDL" attribution — upstream
 *     maintainers want a clean repro on bare GAMDL.
 *
 * Used by HistoryPage and QueueItem.
 */

import { useCallback, useState, type MouseEvent, type ReactElement } from 'react';
import { Bug, Copy } from 'lucide-react';
import { open as openExternal } from '@tauri-apps/plugin-shell';
// Sibling imports (NOT via the @/components/common barrel) to avoid a
// circular dependency: the barrel re-exports this very component, so
// going through it would resolve to `undefined` at module evaluation
// time and produce a runtime "Element type is invalid" crash.
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import { Tooltip } from './Tooltip';
import { useUiStore } from '@/stores/uiStore';

export interface ErrorMessageDisplayProps {
  /** The error message to render. */
  message: string;
  /**
   * The source URL of the failed download, when known. Included in
   * the pre-filled GitHub issue body so upstream can reproduce.
   */
  sourceUrl?: string;
  /**
   * Number of lines to show before truncating with an ellipsis.
   * Defaults to `2`. Pass `null` to disable truncation entirely
   * (full text wraps inline; tooltip + context menu still work).
   */
  truncateLines?: number | null;
  /**
   * Tailwind classes for the visible text. Defaults to
   * `"text-xs text-status-error"` to match the existing inline
   * pattern. Override when the host context needs different
   * sizing (e.g. larger error in a detail panel).
   */
  className?: string;
}

/**
 * Detects messages that look like a known upstream GAMDL defect.
 * The "GAMDL bug " prefix is emitted by MeedyaDL's
 * `download_queue.rs` when a specific upstream pattern matches —
 * see `process::is_mv_cover_template_bug_error` and adjacent
 * classifiers. Future GAMDL-bug detectors should keep using this
 * prefix so this UI surfaces them automatically.
 */
function isGamdlBug(message: string): boolean {
  return message.startsWith('GAMDL bug');
}

/**
 * Build a `https://github.com/glomatico/gamdl/issues/new?…`
 * URL with the title + body pre-filled.
 *
 * No MeedyaDL branding by design — the upstream maintainer should
 * see a normal GAMDL-user bug report. The body is a
 * markdown-friendly template the user can edit before submitting.
 */
function buildGamdlIssueUrl(message: string, sourceUrl?: string): string {
  // Strip the "GAMDL bug — " prefix so the title reads like a
  // user-authored summary, not MeedyaDL's classifier output.
  const stripped = message.replace(/^GAMDL bug\s*[—\-:]\s*/, '');
  // Take the first sentence (or first ~80 chars) for the title.
  const firstSentence = stripped.split(/(?<=[.!?])\s/)[0] ?? stripped;
  const title = firstSentence.length > 80
    ? firstSentence.slice(0, 77).trimEnd() + '…'
    : firstSentence;

  const bodyLines: string[] = [];
  bodyLines.push('### What happened');
  bodyLines.push('');
  bodyLines.push('<!-- Briefly describe what you were trying to download and what went wrong. -->');
  bodyLines.push('');
  if (sourceUrl) {
    bodyLines.push('### URL');
    bodyLines.push('');
    bodyLines.push('```');
    bodyLines.push(sourceUrl);
    bodyLines.push('```');
    bodyLines.push('');
  }
  bodyLines.push('### Error output');
  bodyLines.push('');
  bodyLines.push('```');
  bodyLines.push(message);
  bodyLines.push('```');
  bodyLines.push('');
  bodyLines.push('### Environment');
  bodyLines.push('');
  bodyLines.push('- GAMDL version: <!-- run `gamdl --version` -->');
  bodyLines.push('- Operating system: ');
  bodyLines.push('- Python version: <!-- `python --version` -->');

  const params = new URLSearchParams({
    title,
    body: bodyLines.join('\n'),
  });
  return `https://github.com/glomatico/gamdl/issues/new?${params.toString()}`;
}

export function ErrorMessageDisplay({
  message,
  sourceUrl,
  truncateLines = 2,
  className = 'text-xs text-status-error',
}: ErrorMessageDisplayProps): ReactElement | null {
  const addToast = useUiStore((s) => s.addToast);
  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback((e: MouseEvent) => {
    e.preventDefault();
    setMenuPos({ x: e.clientX, y: e.clientY });
  }, []);
  const closeMenu = useCallback(() => setMenuPos(null), []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(message);
      addToast('Error message copied to clipboard', 'info');
    } catch {
      addToast('Could not copy to clipboard', 'error');
    }
  }, [addToast, message]);

  const handleReportToGamdl = useCallback(async () => {
    const url = buildGamdlIssueUrl(message, sourceUrl);
    try {
      await openExternal(url);
    } catch {
      addToast('Could not open browser', 'error');
    }
  }, [addToast, message, sourceUrl]);

  if (!message) return null;

  const items: ContextMenuItem[] = [
    {
      label: 'Copy error message',
      icon: <Copy size={14} />,
      onClick: handleCopy,
    },
  ];
  if (isGamdlBug(message)) {
    items.push({
      label: 'Report this bug to GAMDL',
      icon: <Bug size={14} />,
      onClick: handleReportToGamdl,
      separator: true,
    });
  }

  // `line-clamp-2` / `line-clamp-3` etc. — Tailwind's standard
  // multi-line truncation utility. `null` disables truncation.
  // Lookup table (not template-literal interpolation) so Tailwind's
  // content scanner sees each literal class string.
  const CLAMP_CLASSES: Record<number, string> = {
    1: 'line-clamp-1',
    2: 'line-clamp-2',
    3: 'line-clamp-3',
    4: 'line-clamp-4',
    5: 'line-clamp-5',
    6: 'line-clamp-6',
  };
  const clampClass = truncateLines === null ? '' : (CLAMP_CLASSES[truncateLines] ?? 'line-clamp-2');

  return (
    <>
      <Tooltip content={message} position="top">
        <p
          className={`${className} ${clampClass} cursor-default select-text`}
          onContextMenu={handleContextMenu}
        >
          {message}
        </p>
      </Tooltip>
      {menuPos && (
        <ContextMenu items={items} x={menuPos.x} y={menuPos.y} onClose={closeMenu} />
      )}
    </>
  );
}
