/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CrashReportDialog.tsx -- Preview/consent modal for GitHub crash reporting.
 *
 * Displays a modal dialog that shows the user exactly what crash data will
 * be shared when reporting to GitHub Issues. The user reviews the Markdown
 * preview, then clicks "Open GitHub Issue" to open a pre-filled issue form
 * in their default browser via the Tauri shell plugin.
 *
 * ## Privacy Design
 *
 * The dialog is intentionally transparent about what data is shared:
 * - A privacy notice at the top explains what will be included
 * - The full Markdown export is shown in a read-only preview
 * - The user must explicitly click to proceed (consent mechanism)
 * - The GitHub issue page itself provides a second chance to review
 *   before submitting
 *
 * ## Component Hierarchy
 *
 * AdvancedTab > CrashReportSection > CrashReportDialog (this file)
 *
 * @see {@link ./CrashReportSection.tsx} -- Parent component that manages dialog state
 * @see {@link @/lib/tauri-commands.ts}  -- exportCrashReport, getGitHubIssueUrl wrappers
 * @see {@link @/components/common/Modal.tsx} -- Underlying modal component
 */

import { useState, useEffect } from 'react';
import { ExternalLink } from 'lucide-react';

import { Modal, Button, LoadingSpinner } from '@/components/common';
import { exportCrashReport, getGitHubIssueUrl } from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';
import type { CrashReport } from '@/types';

/**
 * Props for the CrashReportDialog component.
 */
interface CrashReportDialogProps {
  /** Controls visibility of the modal */
  open: boolean;
  /** Callback invoked when the modal should close */
  onClose: () => void;
  /** The crash report to preview and potentially report */
  report: CrashReport;
  /** Optional callback after the GitHub URL is successfully opened */
  onReported?: () => void;
}

/**
 * CrashReportDialog -- Modal for reviewing and reporting a crash to GitHub.
 *
 * On open, loads the Markdown export of the crash report via IPC. Displays
 * a privacy notice, the preview, and action buttons. The "Open GitHub Issue"
 * button constructs a pre-filled URL and opens it in the default browser.
 */
export function CrashReportDialog({ open, onClose, report, onReported }: CrashReportDialogProps) {
  /** Markdown preview text loaded from the backend */
  const [preview, setPreview] = useState<string>('');
  /** Whether the preview is currently loading */
  const [isLoading, setIsLoading] = useState(true);
  /** Whether the GitHub URL is being constructed/opened */
  const [isOpening, setIsOpening] = useState(false);

  const addToast = useUiStore((s) => s.addToast);

  // Load the Markdown preview when the dialog opens
  useEffect(() => {
    if (open && report) {
      setIsLoading(true);
      exportCrashReport(report.id)
        .then(setPreview)
        .catch(() => setPreview('Failed to load crash report preview.'))
        .finally(() => setIsLoading(false));
    }
  }, [open, report]);

  /**
   * Handles the "Open GitHub Issue" button click.
   * Constructs the pre-filled URL via IPC and opens it in the browser.
   */
  const handleOpenGitHub = async () => {
    setIsOpening(true);
    try {
      const url = await getGitHubIssueUrl(report.id);
      const { open: openUrl } = await import('@tauri-apps/plugin-shell');
      await openUrl(url);
      addToast('Opening GitHub Issues in your browser...', 'info');
      onReported?.();
      onClose();
    } catch {
      addToast('Failed to open GitHub Issues', 'error');
    } finally {
      setIsOpening(false);
    }
  };

  const isDownloadError = report.source === 'download_error';
  const dialogTitle = isDownloadError ? 'Report Error to Developer' : 'Report Crash to Developer';

  return (
    <Modal open={open} onClose={onClose} title={dialogTitle} maxWidth="max-w-2xl">
      {/* Privacy notice */}
      <div className="mb-4 p-3 rounded-platform bg-surface-secondary text-xs text-content-secondary leading-relaxed">
        The following crash report data will be shared when you open the GitHub issue. Review the
        information below before proceeding. You will need a{' '}
        <span className="font-medium text-content-primary">GitHub account</span> to submit the
        issue. No personal data, download history, or account information is included.
      </div>

      {/* Markdown preview */}
      {isLoading ? (
        <div className="flex items-center justify-center py-8">
          <LoadingSpinner />
        </div>
      ) : (
        <pre className="max-h-80 overflow-auto rounded-platform bg-surface-tertiary border border-border-light p-3 text-xs text-content-secondary font-mono whitespace-pre-wrap break-words">
          {preview}
        </pre>
      )}

      {/* Action buttons */}
      <div className="flex items-center justify-end gap-2 mt-4 pt-3 border-t border-border-light">
        <Button variant="secondary" size="sm" onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant="primary"
          size="sm"
          icon={<ExternalLink size={14} />}
          onClick={handleOpenGitHub}
          disabled={isLoading || isOpening}
          loading={isOpening}
        >
          Open GitHub Issue
        </Button>
      </div>
    </Modal>
  );
}
