/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Individual download queue item component.
 * Displays the status, progress, and controls for a single download.
 * Shows URL, content state, progress bar, speed/ETA, and action buttons
 * for cancel/retry/remove.
 */

import {
  Clock,
  Download,
  CheckCircle,
  XCircle,
  Loader2,
  X,
  RotateCcw,
  FolderOpen,
  AlertTriangle,
} from 'lucide-react';
import { ProgressBar } from '@/components/common';
import type { QueueItemStatus, DownloadState } from '@/types';

interface QueueItemProps {
  /** The queue item data */
  item: QueueItemStatus;
  /** Called when the cancel button is clicked */
  onCancel: (id: string) => void;
  /** Called when the retry button is clicked for failed/cancelled downloads */
  onRetry: (id: string) => void;
}

/** Icon and color mapping for each download state */
const STATE_CONFIG: Record<
  DownloadState,
  { icon: typeof Clock; colorClass: string; label: string }
> = {
  queued: { icon: Clock, colorClass: 'text-content-tertiary', label: 'Queued' },
  downloading: {
    icon: Download,
    colorClass: 'text-status-info',
    label: 'Downloading',
  },
  processing: {
    icon: Loader2,
    colorClass: 'text-status-warning',
    label: 'Processing',
  },
  complete: {
    icon: CheckCircle,
    colorClass: 'text-status-success',
    label: 'Complete',
  },
  error: { icon: XCircle, colorClass: 'text-status-error', label: 'Error' },
  cancelled: {
    icon: XCircle,
    colorClass: 'text-content-tertiary',
    label: 'Cancelled',
  },
};

/**
 * Renders a single item in the download queue with status icon,
 * progress tracking, and action buttons.
 *
 * @param item - Queue item status data
 * @param onCancel - Handler for cancel button
 * @param onRemove - Handler for remove button
 */
export function QueueItem({ item, onCancel, onRetry }: QueueItemProps) {
  const config = STATE_CONFIG[item.state];
  const StateIcon = config.icon;
  const isActive = item.state === 'downloading' || item.state === 'processing';

  /**
   * Opens the output folder in the native file manager.
   * Uses dynamic import to avoid errors outside Tauri.
   */
  const handleOpenFolder = async () => {
    if (!item.output_path) return;
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      /* Open the parent directory of the output file */
      const parentDir = item.output_path.substring(
        0,
        item.output_path.lastIndexOf('/'),
      );
      await open(parentDir);
    } catch {
      /* Shell API unavailable */
    }
  };

  return (
    <div className="px-4 py-3 border-b border-border-light last:border-b-0 hover:bg-surface-secondary transition-colors">
      {/* Top row: status icon, URL, action buttons */}
      <div className="flex items-start gap-3">
        {/* Status icon */}
        <div className={`mt-0.5 flex-shrink-0 ${config.colorClass}`}>
          <StateIcon
            size={18}
            className={item.state === 'processing' ? 'animate-spin' : ''}
          />
        </div>

        {/* URL and track info */}
        <div className="flex-1 min-w-0">
          <p className="text-sm text-content-primary truncate">
            {item.urls[0]}
          </p>
          {item.current_track && (
            <p className="text-xs text-content-secondary mt-0.5 truncate">
              {item.current_track}
            </p>
          )}

          {/* Fallback indicator */}
          {item.fallback_occurred && (
            <div className="flex items-center gap-1 mt-1 text-xs text-status-warning">
              <AlertTriangle size={12} />
              <span>Fallback used (codec: {item.codec_used})</span>
            </div>
          )}
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-1 flex-shrink-0">
          {/* Show output folder button for completed downloads */}
          {item.state === 'complete' && item.output_path && (
            <button
              onClick={handleOpenFolder}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
              title="Open folder"
            >
              <FolderOpen size={14} />
            </button>
          )}

          {/* Cancel button for active or queued downloads */}
          {(isActive || item.state === 'queued') && (
            <button
              onClick={() => onCancel(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-status-error hover:bg-surface-elevated transition-colors"
              title="Cancel"
            >
              <X size={14} />
            </button>
          )}

          {/* Retry button for failed or cancelled downloads */}
          {(item.state === 'error' || item.state === 'cancelled') && (
            <button
              onClick={() => onRetry(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
              title="Retry"
            >
              <RotateCcw size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Progress bar (shown for active downloads) */}
      {isActive && (
        <div className="mt-2 pl-7">
          <ProgressBar
            value={item.state === 'downloading' ? item.progress : null}
          />
          {/* Speed and ETA info */}
          {item.speed && (
            <div className="flex gap-3 mt-1 text-[11px] text-content-tertiary">
              {item.speed && <span>{item.speed}</span>}
              {item.eta && <span>ETA: {item.eta}</span>}
            </div>
          )}
        </div>
      )}

      {/* Error message */}
      {item.state === 'error' && item.error && (
        <p className="mt-1.5 pl-7 text-xs text-status-error">{item.error}</p>
      )}
    </div>
  );
}
