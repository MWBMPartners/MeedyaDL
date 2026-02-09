/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Status bar component.
 * Renders a thin bar at the bottom of the main content area showing
 * current application state: active downloads, queue count, and version info.
 */

import { useDownloadStore } from '@/stores/downloadStore';

/**
 * Renders a status bar at the bottom of the main content area.
 * Shows active download count and queue summary information.
 */
export function StatusBar() {
  const queueItems = useDownloadStore((s) => s.queueItems);

  /* Count items by state for status display */
  const activeCount = queueItems.filter(
    (i) => i.state === 'downloading' || i.state === 'processing',
  ).length;
  const queuedCount = queueItems.filter((i) => i.state === 'queued').length;
  const completedCount = queueItems.filter((i) => i.state === 'complete').length;

  return (
    <div className="flex items-center justify-between px-4 py-1.5 bg-surface-secondary border-t border-border-light text-[11px] text-content-tertiary">
      {/* Left: download activity summary */}
      <div className="flex items-center gap-3">
        {activeCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-status-info animate-pulse" />
            {activeCount} downloading
          </span>
        )}
        {queuedCount > 0 && <span>{queuedCount} queued</span>}
        {completedCount > 0 && <span>{completedCount} completed</span>}
        {queueItems.length === 0 && <span>No downloads</span>}
      </div>

      {/* Right: app version */}
      <span>gamdl-GUI v0.1.0</span>
    </div>
  );
}
