// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file CrashScreen.tsx -- what MeedyaDL shows when a screen fails to draw.
 *
 * Rendered by the error boundary in `main.tsx` in place of the whole app
 * when a React component throws while drawing. Until October 2026 that
 * boundary showed a developer dump instead: a heading reading "React Error
 * Caught", the raw error, its name and two stack traces in monospace on
 * hard-coded colours, with no explanation and nothing to click -- the only
 * way out was to quit the app. This screen says what happened in plain
 * words, offers the two useful next steps (reload, report), and keeps the
 * technical details available but folded away.
 *
 * What it deliberately does NOT do:
 *  - It is not translated yet. Most of the app is English-only today (the
 *    translation work is tracked separately), and this screen has to work
 *    even when the thing that broke is the translation system itself.
 *  - It does not try to recover the broken screen in place. A reload is the
 *    one thing known to clear a failed render; the backend (and any
 *    download it is running) is a separate process and is not affected.
 *
 * Styled only with the app's theme tokens, so it follows light, dark and
 * high-contrast modes like every other screen.
 */

import { useState } from 'react';
import { AlertTriangle, RefreshCw, Bug } from 'lucide-react';

import { Button } from '@/components/common';
import { CrashReportDialog } from '@/components/settings/tabs/CrashReportDialog';

export interface CrashScreenProps {
  /** The error that stopped the screen from drawing. */
  error: Error | null;
  /** React's component stack for the error, when it has one. */
  componentStack?: string | null;
  /**
   * The id of the error report saved for this crash, once the backend has
   * saved it. `null` while saving, or if saving failed -- the Report
   * button is only offered when there is a saved report to send.
   */
  reportId: string | null;
  /** Reloads the window. Injected so tests can check it without reloading. */
  onReload?: () => void;
}

export function CrashScreen({ error, componentStack, reportId, onReload }: CrashScreenProps) {
  const [reporting, setReporting] = useState(false);
  const reload = onReload ?? (() => window.location.reload());

  return (
    <main className="h-screen overflow-auto bg-surface-primary text-content-primary flex items-center justify-center p-6">
      <div className="w-full max-w-xl space-y-5">
        <div className="flex items-start gap-3">
          <AlertTriangle size={28} className="text-status-warning flex-shrink-0 mt-0.5" aria-hidden="true" />
          <div className="space-y-2">
            <h1 className="text-xl font-semibold">Something went wrong on this screen</h1>
            <p className="text-sm text-content-secondary leading-relaxed">
              MeedyaDL hit a problem it could not recover from while drawing this screen. Reloading
              usually fixes it. Downloads that were already running carry on in the background,
              and your settings are not affected.
            </p>
            <p className="text-sm text-content-secondary leading-relaxed">
              If it keeps happening, please report it. You will see exactly what is sent before
              anything leaves your computer.
            </p>
          </div>
        </div>

        <div className="flex flex-wrap gap-2 pl-10">
          <Button variant="primary" icon={<RefreshCw size={16} />} onClick={reload}>
            Reload MeedyaDL
          </Button>
          {reportId && (
            <Button variant="secondary" icon={<Bug size={16} />} onClick={() => setReporting(true)}>
              Report this problem
            </Button>
          )}
        </div>

        {/* Folded away by default: useful to a developer or in a bug report,
            noise to everyone else. */}
        <details className="pl-10 text-sm">
          <summary className="cursor-pointer text-content-secondary hover:text-content-primary">
            Technical details
          </summary>
          <pre className="mt-2 p-3 rounded-platform border border-border-light bg-surface-secondary text-xs text-content-secondary whitespace-pre-wrap break-words max-h-64 overflow-auto">
            {[
              error ? `${error.name}: ${error.message}` : 'No error details were recorded.',
              error?.stack ?? '',
              componentStack ? `Component stack:${componentStack}` : '',
            ]
              .filter(Boolean)
              .join('\n\n')}
          </pre>
        </details>
      </div>

      {reportId && (
        <CrashReportDialog
          open={reporting}
          onClose={() => setReporting(false)}
          report={{ id: reportId, source: 'frontend_error' }}
        />
      )}
    </main>
  );
}
