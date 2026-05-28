/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file WelcomeStep.tsx -- Welcome screen step of the setup wizard.
 *
 * Renders the introductory screen that the user sees when the setup wizard
 * first appears. This step serves a purely informational purpose: it
 * introduces the application, explains what the wizard will do, and lists
 * the steps ahead.
 *
 * ## Auto-completion
 *
 * Since no user action is required on this step, it automatically calls
 * `completeStep('welcome')` in a `useEffect` on mount. This enables the
 * "Continue" button in the parent {@link SetupWizard} immediately.
 *
 * ## Store Connection
 *
 * - **setupStore**: Calls `completeStep('welcome')` to unblock navigation.
 *
 * @see {@link ../SetupWizard.tsx}         -- Parent wizard container
 * @see {@link @/stores/setupStore.ts}     -- Zustand store for wizard state
 * @see {@link https://react.dev/reference/react/useEffect} -- useEffect hook
 */

// React useEffect for auto-completing the step on mount.
import { useEffect, useState } from 'react';

// Icons used in the welcome step layout.
import { Download, AlertTriangle, Package } from 'lucide-react';

// Zustand store providing the completeStep action.
import { useSetupStore } from '@/stores/setupStore';

// #876 P5: scan for bundles on first-launch + offer restore.
import { scanForBundles, importProfile } from '@/lib/tauri-commands';
import type { DiscoveredBundle } from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * WelcomeStep -- Renders the welcome screen.
 *
 * Layout:
 *   1. Large app icon (Download icon in an accent-coloured rounded square)
 *   2. Welcome heading and subtitle
 *   3. Numbered list of setup steps the wizard will perform
 *   4. Footnote about sandboxed installation
 *
 * The component calls `completeStep('welcome')` on mount to signal the
 * parent wizard that this step's requirements are satisfied (there are
 * none), enabling the "Continue" button.
 */
export function WelcomeStep() {
  /** Marks a wizard step as completed in the setupStore */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Toast helper for the bundle-restore flow. */
  const addToast = useUiStore((s) => s.addToast);

  /** #876 P5: bundles discovered on disk by `scanForBundles`. */
  const [discoveredBundles, setDiscoveredBundles] = useState<
    DiscoveredBundle[]
  >([]);
  /** Whether the restore-from-bundle handler is currently running. */
  const [isRestoringBundle, setIsRestoringBundle] = useState(false);

  /**
   * Auto-complete this step on mount.
   * The Welcome step has no requirements -- it is purely informational.
   * Calling completeStep('welcome') adds 'welcome' to the completedSteps
   * set, which enables the "Continue" button in the wizard footer.
   * @see https://react.dev/reference/react/useEffect
   */
  useEffect(() => {
    completeStep('welcome');
  }, [completeStep]);

  /**
   * #876 P5: on mount, scan Downloads + Desktop + home for any
   * `.meedyabundle` files. If any are found, surface them in a
   * banner above the welcome content so the user can short-circuit
   * the wizard by restoring their previous install state.
   *
   * Failures are silent — this is a UX nicety, not a load-bearing
   * step. The user can still trigger an import from
   * Settings > General > Profile Bundle after setup completes.
   */
  useEffect(() => {
    let cancelled = false;
    scanForBundles()
      .then((found) => {
        if (!cancelled && found.length > 0) {
          setDiscoveredBundles(found);
        }
      })
      .catch(() => {
        // Silent — see comment above.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const loadSettings = useSettingsStore((s) => s.loadSettings);

  /**
   * Restore from the supplied bundle directly inside the setup
   * wizard (#876 P5). We import every OPTIONAL section the bundle
   * declares EXCEPT credentials — credentials need a password the
   * setup wizard doesn't have a UI to collect, and the rest of
   * the bundle still restores without them. After the import we
   * reload settings so the wizard's downstream steps (which read
   * cookies_path, output_path, etc.) pick up the imported values.
   *
   * The user can re-import credentials separately from
   * Settings > General > Profile Bundle once setup completes.
   */
  const handleRestoreBundle = async (bundle: DiscoveredBundle) => {
    const confirmed = window.confirm(
      `Restore from "${bundle.path}"?\n\nMeedyaDL will overwrite its current files for: ${
        ['settings', ...bundle.summary.sections]
          .filter((s) => s !== 'credentials')
          .join(', ')
      }.\n\nCredentials (if present) are NOT restored here — re-import them later from Settings > General > Profile Bundle.`,
    );
    if (!confirmed) return;
    setIsRestoringBundle(true);
    try {
      const result = await importProfile({
        source_path: bundle.path,
        settings: 'replace',
        queue: bundle.summary.sections.includes('queue') ? 'replace' : 'skip',
        history: bundle.summary.sections.includes('history')
          ? 'replace'
          : 'skip',
        database: bundle.summary.sections.includes('database')
          ? 'replace'
          : 'skip',
        activity_log: bundle.summary.sections.includes('activity_log')
          ? 'replace'
          : 'skip',
        manifests: bundle.summary.sections.includes('manifests')
          ? 'replace'
          : 'skip',
        credentials: 'skip',
      });
      // Reload the live settings store so subsequent wizard steps
      // see the restored cookies_path / output_path / etc.
      await loadSettings();
      const restored: string[] = [];
      if (result.settings_restored) restored.push('settings');
      if (result.queue_restored) restored.push('queue');
      if (result.history_restored) restored.push('history');
      if (result.database_restored) restored.push('database');
      if (result.activity_log_files_restored > 0)
        restored.push(
          `${result.activity_log_files_restored} activity-log file(s)`,
        );
      if (result.manifests_restored > 0)
        restored.push(`${result.manifests_restored} manifest(s)`);
      addToast(
        `Restored: ${restored.join(', ') || 'nothing'}. Continue through the wizard to finish setup.`,
        'success',
      );
      // The banner stays open — the user might want to compare,
      // or restore a different bundle. They can simply ignore it
      // and click Continue.
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(`Failed to restore bundle: ${message}`, 'error');
    } finally {
      setIsRestoringBundle(false);
    }
  };

  return (
    <div className="text-center space-y-6">
      {/* #876 P5: restore-from-previous-install banner. Surfaces
          any .meedyabundle files we found in Downloads / Desktop /
          home so a returning user can short-circuit the setup. */}
      {discoveredBundles.length > 0 && (
        <div className="text-left p-4 rounded-platform border border-accent/40 bg-accent/5">
          <div className="flex items-start gap-2 mb-3">
            <Package size={18} className="text-accent flex-shrink-0 mt-0.5" />
            <div>
              <h3 className="text-sm font-semibold text-content-primary">
                Found {discoveredBundles.length} previous-install bundle
                {discoveredBundles.length === 1 ? '' : 's'}
              </h3>
              <p className="text-xs text-content-secondary mt-0.5">
                Restore your settings, queue, history, and library index in
                one click. You can still continue with a fresh setup if you'd
                rather.
              </p>
            </div>
          </div>
          <div className="space-y-2">
            {discoveredBundles.map((b) => (
              <div
                key={b.path}
                className="flex items-center justify-between gap-3 p-2 rounded border border-border bg-surface-primary text-xs"
              >
                <div className="min-w-0 flex-1">
                  <div className="font-medium truncate">{b.path}</div>
                  <div className="text-content-tertiary">
                    {b.summary.producer} {b.summary.producer_version} ·
                    exported {b.summary.exported_at.slice(0, 10)} ·{' '}
                    {b.summary.sections.length} optional section
                    {b.summary.sections.length === 1 ? '' : 's'}
                    {!b.summary.produced_by_this_app && (
                      <span className="ml-2 text-status-warning">
                        ⚠ produced by another MeedyaSuite app
                      </span>
                    )}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => handleRestoreBundle(b)}
                  disabled={isRestoringBundle}
                  className="px-3 py-1 rounded bg-accent text-content-inverse hover:opacity-90 disabled:opacity-50 flex-shrink-0"
                >
                  {isRestoringBundle ? 'Restoring...' : 'Restore'}
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* App icon */}
      <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-accent">
        <Download size={40} className="text-content-inverse" />
      </div>

      {/* Welcome heading */}
      <div>
        <h2 className="text-2xl font-bold text-content-primary">Welcome to GAMDL</h2>
        <p className="text-base text-content-secondary mt-2">Apple Music Downloader</p>
      </div>

      {/* Setup description */}
      <div className="text-left max-w-md mx-auto space-y-3 text-sm text-content-secondary">
        <p>
          This wizard will help you set up everything needed to download music and videos from Apple
          Music. Here's what we'll do:
        </p>

        <ol className="list-decimal list-inside space-y-2 ml-2">
          <li>
            <strong className="text-content-primary">Install Python</strong> - A portable Python
            runtime will be downloaded (no system changes)
          </li>
          <li>
            <strong className="text-content-primary">Install GAMDL</strong> - The Apple Music
            download tool will be installed into the portable Python
          </li>
          <li>
            <strong className="text-content-primary">Install Tools</strong> - Required tools like
            FFmpeg and mp4decrypt will be downloaded
          </li>
          <li>
            <strong className="text-content-primary">Import Cookies</strong> - You'll provide your
            Apple Music authentication cookies
          </li>
        </ol>

        <p className="text-xs text-content-tertiary">
          All files are installed to the application data directory. Nothing is installed
          system-wide.
        </p>

        {/* Disclaimer notice */}
        <div className="mt-4 p-3 rounded-platform border border-status-warning/30 bg-status-warning/5">
          <div className="flex items-start gap-2">
            <AlertTriangle size={14} className="text-status-warning flex-shrink-0 mt-0.5" />
            <div className="text-xs text-content-tertiary leading-relaxed">
              <strong className="text-content-secondary">Disclaimer:</strong> MeedyaDL relies on
              third-party libraries and services. Quality of service, features, and performance are
              not guaranteed. While we endeavour to provide updates and fixes, no liability is
              accepted for loss of functionality. By continuing, you acknowledge and accept these
              terms.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
