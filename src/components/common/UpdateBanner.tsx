/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Update notification banner component.
 * Displays a dismissible banner at the top of the main content area when
 * component updates are available. Shows update details for GAMDL, the app,
 * and Python, with actions to upgrade GAMDL directly or view release notes.
 */

import { ArrowUpCircle, X, ExternalLink, RefreshCw } from 'lucide-react';
import { useUpdateStore } from '@/stores/updateStore';
import { useUiStore } from '@/stores/uiStore';
import { Button } from './Button';

/**
 * Renders a banner showing available updates with upgrade/dismiss actions.
 * Only renders when there are non-dismissed, compatible updates available.
 * The GAMDL component gets a direct "Upgrade" button; other components
 * get a "View" link to their release page.
 */
export function UpdateBanner() {
  const activeUpdates = useUpdateStore((s) => s.getActiveUpdates());
  const dismissUpdate = useUpdateStore((s) => s.dismissUpdate);
  const upgradeGamdl = useUpdateStore((s) => s.upgradeGamdl);
  const isUpgrading = useUpdateStore((s) => s.isUpgrading);
  const addToast = useUiStore((s) => s.addToast);

  /* Don't render anything if there are no active updates */
  if (activeUpdates.length === 0) return null;

  /** Handle the GAMDL upgrade action */
  const handleUpgradeGamdl = async () => {
    try {
      const version = await upgradeGamdl();
      addToast(`GAMDL upgraded to v${version}`, 'success');
    } catch {
      addToast('Failed to upgrade GAMDL', 'error');
    }
  };

  /** Open a release URL in the default browser */
  const handleViewRelease = (url: string) => {
    window.open(url, '_blank');
  };

  return (
    <div className="mx-4 mt-3 mb-1 p-3 rounded-platform border border-accent/30 bg-accent-light">
      <div className="flex items-start gap-3">
        {/* Update icon */}
        <ArrowUpCircle
          size={18}
          className="text-accent flex-shrink-0 mt-0.5"
        />

        {/* Update details */}
        <div className="flex-1 space-y-2">
          {/* Header */}
          <p className="text-sm font-medium text-content-primary">
            {activeUpdates.length === 1
              ? 'Update Available'
              : `${activeUpdates.length} Updates Available`}
          </p>

          {/* Per-component update info */}
          {activeUpdates.map((update) => (
            <div
              key={update.name}
              className="flex items-center justify-between gap-2"
            >
              {/* Component name and version info */}
              <span className="text-xs text-content-secondary">
                <span className="font-medium">{update.name}</span>
                {update.current_version && (
                  <span> v{update.current_version}</span>
                )}
                {update.latest_version && (
                  <span className="text-accent">
                    {' '}
                    &rarr; v{update.latest_version}
                  </span>
                )}
              </span>

              {/* Actions */}
              <div className="flex items-center gap-1.5">
                {/* GAMDL gets a direct upgrade button */}
                {update.name === 'GAMDL' && (
                  <Button
                    variant="primary"
                    size="sm"
                    icon={<RefreshCw size={12} />}
                    loading={isUpgrading}
                    onClick={handleUpgradeGamdl}
                  >
                    Upgrade
                  </Button>
                )}

                {/* Link to release page (if available) */}
                {update.release_url && (
                  <button
                    onClick={() => handleViewRelease(update.release_url!)}
                    className="p-1 rounded hover:bg-surface-secondary transition-colors"
                    title="View release"
                  >
                    <ExternalLink
                      size={14}
                      className="text-content-tertiary"
                    />
                  </button>
                )}

                {/* Dismiss this update notification */}
                <button
                  onClick={() => dismissUpdate(update.name)}
                  className="p-1 rounded hover:bg-surface-secondary transition-colors"
                  title="Dismiss"
                >
                  <X size={14} className="text-content-tertiary" />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
