// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// First-launch crash report opt-in modal.
// Asks the user if they want to enable anonymous crash reporting (Sentry).
// Only shown once — the crash_report_prompt_shown flag prevents re-prompting.

import { useCallback } from 'react';
import { Modal } from '@/components/common';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { setStoredPreference } from '@/lib/tauri-commands';

/**
 * Crash report opt-in modal shown on first launch (after setup wizard).
 */
export default function CrashReportOptInModal() {
  const show = useUiStore((s) => s.showCrashReportPrompt);

  // Both answers go to DISK. They used to be written to the page's own
  // copy of the settings and nothing ever saved it, so the question was
  // asked again at every launch and somebody who said yes was never
  // actually opted in. Crash reporting is switched on from the file at
  // startup, so an answer that never reached the file reached nothing.
  //
  // Saying yes takes effect at the next launch, for that same reason.
  const record = useCallback(async (enabled: boolean) => {
    useSettingsStore.getState().updateSettings({
      sentry_enabled: enabled,
      crash_report_prompt_shown: true,
    });
    try {
      await setStoredPreference({ kind: 'crash_reporting_choice', enabled });
    } catch (err) {
      // If this cannot be written, the question will be asked again next
      // time — which is the safe way round for a consent question.
      console.error('Could not record the crash-reporting answer:', err);
    }
    useUiStore.getState().setShowCrashReportPrompt(false);
  }, []);

  const handleAccept = useCallback(() => void record(true), [record]);
  const handleDecline = useCallback(() => void record(false), [record]);

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
            className="px-4 py-2 rounded-lg text-sm font-medium bg-accent text-content-on-accent hover:opacity-90 transition-opacity"
            onClick={handleAccept}
          >
            Yes, send crash reports
          </button>
        </div>
      </div>
    </Modal>
  );
}
