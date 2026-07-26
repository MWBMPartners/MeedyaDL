// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file PrereleaseNoticeModal.tsx -- First-load notice for pre-release versions.
 *
 * Displays a modal dialog on the first launch of each new pre-release version
 * (v0.x.x) to inform the user that:
 *   - They are running a pre-release build
 *   - It may contain bugs (some critical)
 *   - Error logging may contain more detail than normal
 *   - They can install a stable release if they prefer
 *
 * ## Display Logic
 *
 * The modal is shown when ALL of these conditions are true:
 *   1. The current version is a pre-release (major version == 0)
 *   2. The `last_seen_version` differs from the current version (new install/upgrade)
 *   3. The modal hasn't been dismissed yet in this session
 *
 * The version detection and `last_seen_version` update happens in the Rust
 * backend (`config_service.rs` / `load_settings()`). The frontend reads the
 * `showPrereleaseNotice` flag from `uiStore` which is set by `App.tsx` during
 * the initialization sequence.
 *
 * ## Component Hierarchy
 *
 * App.tsx > PrereleaseNoticeModal (this file)
 *
 * @see {@link @/stores/uiStore.ts}            -- showPrereleaseNotice state
 * @see {@link @/components/common/Modal.tsx}   -- Underlying modal component
 */

import { useState, useEffect, useCallback } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { useUiStore } from '../../stores/uiStore';
import { useUpdateStore, APP_COMPONENT_NAME } from '../../stores/updateStore';
import type { ComponentUpdate } from '../../types';
import { Modal } from './Modal';

/**
 * Pre-release first-load notice modal.
 *
 * Warns the user about pre-release status and offers to install a stable
 * release if one is available. Dismissed via the "I Understand" button or
 * "Install Stable Release" action.
 */
export default function PrereleaseNoticeModal() {
  // ── State ──────────────────────────────────────────────────────────
  const [appVersion, setAppVersion] = useState<string>('');
  const showPrereleaseNotice = useUiStore((s) => s.showPrereleaseNotice);
  const setShowPrereleaseNotice = useUiStore((s) => s.setShowPrereleaseNotice);

  // Check if there's a stable (non-prerelease) update available
  const latestUpdate = useUpdateStore((s): ComponentUpdate | undefined =>
    s.lastResult?.components.find(
      (c: ComponentUpdate) =>
        c.name === APP_COMPONENT_NAME && c.update_available && c.is_compatible && !c.is_prerelease
    )
  );

  // ── Effects ────────────────────────────────────────────────────────

  // Fetch the current app version for display
  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('unknown'));
  }, []);

  // ── Handlers ───────────────────────────────────────────────────────

  /** Dismiss the modal — user acknowledges the pre-release notice */
  const handleDismiss = useCallback(() => {
    setShowPrereleaseNotice(false);
  }, [setShowPrereleaseNotice]);

  /** Navigate to the updates page to install a stable release */
  const handleInstallStable = useCallback(() => {
    setShowPrereleaseNotice(false);
    useUiStore.getState().setPage('updates');
  }, [setShowPrereleaseNotice]);

  // ── Render ─────────────────────────────────────────────────────────

  return (
    <Modal
      open={showPrereleaseNotice}
      onClose={handleDismiss}
      title="Pre-Release Version"
      maxWidth="max-w-xl"
    >
      <div className="space-y-4 text-sm text-content-secondary">
        {/* Version badge */}
        <div className="flex items-center gap-2">
          <span className="inline-flex items-center rounded-full bg-status-warning-bg px-3 py-1 text-xs font-semibold text-status-warning border border-status-warning">
            v{appVersion} — Pre-Release
          </span>
        </div>

        {/* Warning message */}
        <div className="space-y-3">
          <p>
            You are running a <strong className="text-content-primary">pre-release version</strong> of
            MeedyaDL. Please be aware of the following:
          </p>

          <ul className="list-disc pl-5 space-y-2">
            <li>
              This version <strong className="text-content-primary">may contain bugs</strong>, including
              some that could be critical or cause data loss.
            </li>
            <li>
              Features may be <strong className="text-content-primary">incomplete or unstable</strong> and
              could change without notice.
            </li>
            <li>
              Error logging may contain <strong className="text-content-primary">more detail than
              normal</strong> to assist with issue tracking and debugging. The Verbose Activity Log setting
              is preserved across restarts in pre-release builds.
            </li>
            <li>
              If you encounter issues, please report them via{' '}
              <strong className="text-content-primary">Settings &gt; Advanced &gt; Error Reporting</strong>.
            </li>
          </ul>
        </div>

        {/* Stable release offer (only if available) */}
        {latestUpdate && (
          <div className="p-3 rounded-lg bg-status-success-bg border border-status-success">
            <p className="text-xs font-semibold text-status-success mb-1">
              Stable Release Available
            </p>
            <p className="text-xs text-content-secondary">
              A stable release ({latestUpdate.latest_version}) is available. If you prefer a
              more reliable experience, you can switch to the stable version.
            </p>
          </div>
        )}

        {/* What's New section — shows key changes in this version */}
        <div className="p-3 rounded-lg bg-surface-secondary border border-border">
          <p className="text-xs font-semibold text-content-primary mb-2">
            What&apos;s New in v{appVersion}
          </p>
          <p className="text-xs text-content-tertiary">
            View the full release notes on the{' '}
            <button
              type="button"
              className="text-accent-primary hover:underline"
              onClick={() => {
                setShowPrereleaseNotice(false);
                useUiStore.getState().setPage('updates');
              }}
            >
              Updates page
            </button>
            .
          </p>
        </div>

        {/* Action buttons */}
        <div className="flex justify-end gap-3 pt-2">
          {latestUpdate && (
            <button
              type="button"
              className="px-4 py-2 rounded-lg text-sm font-medium bg-status-success text-white hover:opacity-90 transition-opacity"
              onClick={handleInstallStable}
            >
              Install Stable Release
            </button>
          )}
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium bg-interactive-accent text-white hover:opacity-90 transition-opacity"
            onClick={handleDismiss}
          >
            I Understand
          </button>
        </div>
      </div>
    </Modal>
  );
}
