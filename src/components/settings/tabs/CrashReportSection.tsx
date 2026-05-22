/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CrashReportSection.tsx -- Error report list with GitHub reporting.
 *
 * Renders a list of the 5 most recent error reports within the "Error
 * Reporting" section of {@link AdvancedTab}. Each report row displays:
 *   - Formatted timestamp
 *   - Source badge (Rust Panic / Frontend Error / Unhandled Rejection / Download Error)
 *   - Truncated error message
 *   - "Report" button → opens {@link CrashReportDialog}
 *   - Delete button → removes the error report from disk
 *
 * When no crash reports exist, an empty state message is shown.
 *
 * ## Component Hierarchy
 *
 * AdvancedTab > CrashReportSection (this file) > CrashReportDialog
 *
 * @see {@link ./CrashReportDialog.tsx} -- Preview/consent modal
 * @see {@link @/lib/tauri-commands.ts}  -- listCrashReports, deleteCrashReport
 * @see {@link ./AdvancedTab.tsx}        -- Parent tab that embeds this section
 */

import { useState, useEffect, useCallback } from 'react';
import { Trash2, AlertTriangle } from 'lucide-react';

import { Button } from '@/components/common';
import { listCrashReports, deleteCrashReport, deleteAllCrashReports } from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';
import { withErrorToast } from '@/lib/withErrorToast';
import type { CrashReport } from '@/types';

import { CrashReportDialog } from './CrashReportDialog';

/** Maximum number of crash reports to display in the list. */
const MAX_DISPLAY_COUNT = 5;

/**
 * Maps crash report source strings to human-readable labels and colours.
 * Used to render colour-coded badges in the report list rows.
 */
const SOURCE_BADGES: Record<string, { label: string; className: string }> = {
  rust_panic: {
    label: 'Rust Panic',
    className: 'bg-status-error/15 text-status-error',
  },
  frontend_error: {
    label: 'Frontend Error',
    className: 'bg-orange-500/15 text-orange-500',
  },
  unhandled_rejection: {
    label: 'Unhandled Rejection',
    className: 'bg-yellow-500/15 text-yellow-500',
  },
  download_error: {
    label: 'Download Error',
    className: 'bg-blue-500/15 text-blue-500',
  },
};

/**
 * Formats an ISO 8601 timestamp into a concise, locale-aware display string.
 * Falls back to the raw timestamp if parsing fails.
 */
function formatTimestamp(iso: string): string {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return iso;
  }
}

/**
 * CrashReportSection -- Renders the recent crash reports list with actions.
 *
 * Loads crash reports on mount via the `listCrashReports` IPC command.
 * Provides "Report" (opens CrashReportDialog) and "Delete" actions per row.
 */
export function CrashReportSection() {
  /** List of crash reports to display (max 5, newest first) */
  const [reports, setReports] = useState<CrashReport[]>([]);
  /** The report currently selected for the report dialog */
  const [selectedReport, setSelectedReport] = useState<CrashReport | null>(null);
  /** Whether the CrashReportDialog is open */
  const [isDialogOpen, setIsDialogOpen] = useState(false);

  const addToast = useUiStore((s) => s.addToast);

  /** Loads crash reports from the backend. */
  const loadReports = useCallback(async () => {
    try {
      const all = await listCrashReports();
      setReports(all.slice(0, MAX_DISPLAY_COUNT));
    } catch {
      // Silently fail -- crash reports are supplementary, not critical
    }
  }, []);

  // Load reports on mount
  useEffect(() => {
    loadReports();
  }, [loadReports]);

  /** Opens the CrashReportDialog for a specific report. */
  const handleReport = (report: CrashReport) => {
    setSelectedReport(report);
    setIsDialogOpen(true);
  };

  /** Deletes a crash report and removes it from the displayed list. */
  const handleDelete = async (id: string) => {
    const ok = await withErrorToast(() => deleteCrashReport(id), {
      successMsg: 'Crash report deleted',
      successVariant: 'info',
      errorMsg: (err) => `Failed to delete crash report: ${err}`,
    });
    if (ok !== undefined) {
      setReports((prev) => prev.filter((r) => r.id !== id));
    }
  };

  /** Deletes all crash reports and clears the displayed list. */
  const handleDeleteAll = async () => {
    const count = await withErrorToast(() => deleteAllCrashReports(), {
      errorMsg: (err) => `Failed to clear error reports: ${err}`,
    });
    if (count !== undefined) {
      setReports([]);
      addToast(`Deleted ${count} error report(s)`, 'info');
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-medium text-content-secondary">Recent Error Reports</h4>
        {reports.length > 1 && (
          <button
            type="button"
            onClick={handleDeleteAll}
            className="text-[10px] text-content-tertiary hover:text-status-error transition-colors"
          >
            Clear All
          </button>
        )}
      </div>

      {reports.length === 0 ? (
        /* Empty state */
        <p className="text-xs text-content-tertiary py-2">No error reports found.</p>
      ) : (
        /* Report list */
        <div className="space-y-1.5">
          {reports.map((report) => {
            const badge = SOURCE_BADGES[report.source] ?? {
              label: report.source,
              className: 'bg-surface-secondary text-content-tertiary',
            };

            return (
              <div
                key={report.id}
                className="flex items-center gap-2 p-2 rounded-platform bg-surface-secondary text-xs"
              >
                {/* Timestamp */}
                <span className="text-content-tertiary whitespace-nowrap min-w-[90px]">
                  {formatTimestamp(report.timestamp)}
                </span>

                {/* Source badge */}
                <span
                  className={`px-1.5 py-0.5 rounded text-[10px] font-medium whitespace-nowrap ${badge.className}`}
                >
                  {badge.label}
                </span>

                {/* Error message (truncated) */}
                <span className="text-content-secondary truncate flex-1 min-w-0">
                  {report.panic_message || 'No message'}
                </span>

                {/* Action buttons */}
                <div className="flex items-center gap-1 flex-shrink-0">
                  <Button
                    variant="primary"
                    size="sm"
                    icon={<AlertTriangle size={12} />}
                    onClick={() => handleReport(report)}
                  >
                    Report
                  </Button>
                  <button
                    onClick={() => handleDelete(report.id)}
                    className="p-1.5 rounded-platform text-content-tertiary hover:text-status-error hover:bg-surface-tertiary transition-colors"
                    aria-label="Delete crash report"
                    title="Delete crash report"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* CrashReportDialog modal -- rendered when a report is selected */}
      {selectedReport && (
        <CrashReportDialog
          open={isDialogOpen}
          onClose={() => {
            setIsDialogOpen(false);
            setSelectedReport(null);
          }}
          report={selectedReport}
          onReported={loadReports}
        />
      )}
    </div>
  );
}
