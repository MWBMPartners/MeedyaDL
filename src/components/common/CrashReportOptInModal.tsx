// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// First-launch crash report opt-in modal.
// Asks the user if they want to enable anonymous crash reporting (Sentry).
// Only shown once — the crash_report_prompt_shown flag prevents re-prompting.

import { useCallback } from 'react';
import { Modal } from '@/components/common';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * Crash report opt-in modal shown on first launch (after setup wizard).
 */
export default function CrashReportOptInModal() {
  const show = useUiStore((s) => s.showCrashReportPrompt);

  const handleAccept = useCallback(() => {
    useSettingsStore.getState().updateSettings({
      sentry_enabled: true,
      crash_report_prompt_shown: true,
    });
    useUiStore.getState().setShowCrashReportPrompt(false);
  }, []);

  const handleDecline = useCallback(() => {
    useSettingsStore.getState().updateSettings({
      sentry_enabled: false,
      crash_report_prompt_shown: true,
    });
    useUiStore.getState().setShowCrashReportPrompt(false);
  }, []);

  return (
    <Modal
      open={show}
      onClose={handleDecline}
      title="Help Improve MeedyaDL"
      maxWidth="max-w-md"
    >
      <div className="space-y-4 text-sm text-content-secondary">
        <p>
          Send anonymous crash reports when something goes wrong? This helps us
          fix bugs faster.
        </p>
        <p className="text-xs text-content-tertiary">
          No personal data, download history, or music library information is
          ever collected. You can change this anytime in Settings &gt; Advanced.
        </p>
        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium text-content-secondary hover:text-content-primary transition-colors"
            onClick={handleDecline}
          >
            No thanks
          </button>
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium bg-accent text-white hover:opacity-90 transition-opacity"
            onClick={handleAccept}
          >
            Yes, send crash reports
          </button>
        </div>
      </div>
    </Modal>
  );
}
