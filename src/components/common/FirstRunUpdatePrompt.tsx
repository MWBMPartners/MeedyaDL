// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file FirstRunUpdatePrompt.tsx -- First-run "an app update is available" prompt.
 *
 * Shown instead of the setup wizard when BOTH of these are true on a fresh
 * install/launch:
 *   1. `settings.setup_completed` is false (the user hasn't finished the
 *      setup wizard yet), AND
 *   2. A compatible MeedyaDL app update is available (per
 *      `findAppComponentUpdate()` in `updateStore.ts`).
 *
 * ## Why this exists
 *
 * Previously, `<App>` silently suppressed the setup wizard whenever an app
 * update was pending, on the theory that the user should update first and
 * let the wizard run on the new version after restart. But the gate that
 * detected "update available" was broken (see #935-era audit — it compared
 * against a component name the backend never emits), so this path was
 * effectively dead in practice. Fixing that gate on its own would have
 * reintroduced a real dead end: a fresh install with no dependencies
 * installed, and no setup wizard shown because an update is "pending" --
 * the user would be stuck looking at a loading screen with no way forward
 * if they didn't want to update immediately (offline, slow network,
 * deliberately testing a specific version, etc).
 *
 * This component replaces silent suppression with an explicit choice that
 * ALWAYS terminates in one of two places:
 *   - **Update now**: downloads and installs the update, then relaunches.
 *     Errors are shown inline and the prompt stays open so the user can
 *     retry or fall back to continuing setup.
 *   - **Continue setup on this version**: dismisses the prompt and shows
 *     the setup wizard immediately, on the currently-installed version.
 *
 * @see ../../stores/uiStore.ts -- showFirstRunUpdatePrompt / firstRunUpdatePromptInfo
 * @see ../../App.tsx -- the startup effect that decides whether to show this
 */

import { useCallback } from 'react';
import { Download, RotateCcw } from 'lucide-react';
import { relaunch } from '@tauri-apps/plugin-process';
import { useUiStore } from '@/stores/uiStore';
import { useUpdateStore } from '@/stores/updateStore';
import { Button } from './Button';
import { Modal } from './Modal';

export default function FirstRunUpdatePrompt() {
  const show = useUiStore((s) => s.showFirstRunUpdatePrompt);
  const info = useUiStore((s) => s.firstRunUpdatePromptInfo);
  const setShowFirstRunUpdatePrompt = useUiStore((s) => s.setShowFirstRunUpdatePrompt);
  const setFirstRunUpdateDeferred = useUiStore((s) => s.setFirstRunUpdateDeferred);
  const setShowSetupWizard = useUiStore((s) => s.setShowSetupWizard);
  const addToast = useUiStore((s) => s.addToast);

  const downloadAndInstallAppUpdate = useUpdateStore((s) => s.downloadAndInstallAppUpdate);
  const isDownloadingUpdate = useUpdateStore((s) => s.isDownloadingUpdate);
  const downloadProgress = useUpdateStore((s) => s.downloadProgress);
  const updateInstalled = useUpdateStore((s) => s.updateInstalled);
  const downloadError = useUpdateStore((s) => s.downloadError);

  /**
   * Move on to the setup wizard on the currently-installed version.
   * Records the session-local "deferred" flag so this prompt never
   * resurfaces on top of the wizard it just opened, then flips the two
   * visibility flags together so there is never a render with neither
   * overlay showing.
   */
  const handleContinueSetup = useCallback(() => {
    setFirstRunUpdateDeferred(true);
    setShowFirstRunUpdatePrompt(false);
    setShowSetupWizard(true);
  }, [setFirstRunUpdateDeferred, setShowFirstRunUpdatePrompt, setShowSetupWizard]);

  /**
   * Download, install, and relaunch into the update. On failure the
   * prompt intentionally stays open (with the error surfaced inline via
   * `downloadError` from the store) so the user can either retry or fall
   * back to "Continue setup on this version" -- never a dead end.
   */
  const handleUpdateNow = useCallback(async () => {
    if (!info) return;
    try {
      await downloadAndInstallAppUpdate(info.tagName);
      await relaunch();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      addToast(message || 'Failed to download and install update', 'error');
    }
  }, [info, downloadAndInstallAppUpdate, addToast]);

  const handleRestart = useCallback(async () => {
    try {
      await relaunch();
    } catch {
      addToast('Failed to restart. Please restart manually.', 'error');
    }
  }, [addToast]);

  if (!show || !info) return null;

  return (
    <Modal
      // Escape / backdrop-click must not strand the user with neither
      // overlay visible -- treat dismissal the same as the explicit
      // "Continue setup" choice, which is the safe (non-destructive)
      // default of the two paths.
      open={show}
      onClose={handleContinueSetup}
      title="Update Available"
      maxWidth="max-w-lg"
    >
      <div className="space-y-4 text-sm text-content-secondary">
        <p>
          A newer version of MeedyaDL{' '}
          {info.latestVersion && (
            <>
              (<span className="text-content-primary font-medium">v{info.latestVersion}</span>)
            </>
          )}{' '}
          is available. You can install it now before setting up MeedyaDL, or continue setup on
          the version you already have and update later.
        </p>

        {downloadError && (
          <div className="rounded-platform border border-status-error/30 bg-status-error/5 p-3">
            <p className="text-xs text-status-error">{downloadError}</p>
          </div>
        )}

        {isDownloadingUpdate && (
          <div className="flex items-center gap-2">
            <div className="flex-1 h-1.5 bg-surface-secondary rounded-full overflow-hidden">
              <div
                className="h-full bg-accent rounded-full transition-all duration-300"
                style={{ width: `${downloadProgress ?? 0}%` }}
              />
            </div>
            <span className="text-xs text-content-tertiary tabular-nums">
              {downloadProgress != null ? `${downloadProgress}%` : '...'}
            </span>
          </div>
        )}

        <div className="flex flex-col-reverse sm:flex-row sm:justify-end gap-3 pt-2">
          <Button variant="ghost" onClick={handleContinueSetup} disabled={isDownloadingUpdate}>
            Continue Setup on This Version
          </Button>
          {updateInstalled ? (
            <Button variant="primary" icon={<RotateCcw size={14} />} onClick={handleRestart}>
              Restart Now
            </Button>
          ) : (
            <Button
              variant="primary"
              icon={<Download size={14} />}
              loading={isDownloadingUpdate}
              onClick={handleUpdateNow}
            >
              {downloadError ? 'Retry Update' : 'Update Now'}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}
