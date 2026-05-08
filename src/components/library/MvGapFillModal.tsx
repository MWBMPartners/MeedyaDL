// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file MV gap-fill confirmation modal (#717 sub-feature 5d).
 *
 * When the user clicks "Re-download gaps" on a Library Scan row,
 * this modal asks one of two questions before queueing — depending
 * on whether `music_video_companion` is enabled in settings:
 *
 * **Companion ENABLED in settings:**
 *
 *   "Music videos may be downloaded/updated as part of this gap-fill.
 *    Include music videos? (Yes — inherit your settings · No — audio
 *    only for this re-download)"
 *
 *   - Yes  → mv_companion_override = null (inherit settings = TRUE)
 *   - No   → mv_companion_override = false (override → audio only)
 *
 * **Companion DISABLED in settings:**
 *
 *   "Music video companion is currently disabled in settings.
 *    Include music videos in this gap-fill? (Yes — for this
 *    re-download only · No — audio only)"
 *
 *   - Yes  → mv_companion_override = true (override → MV included)
 *   - No   → mv_companion_override = null (inherit settings = FALSE)
 *
 * In both cases the user's GLOBAL setting is unchanged. The override
 * applies to THIS one queue item only via the
 * `DownloadRequest.mv_companion_override` field shipped in v1.0.4
 * (#717/5e).
 *
 * @see DownloadRequest in src/types/index.ts
 * @see set_stage_with_label in src-tauri/src/services/progress_stages.rs
 */

import { Modal } from '@/components/common/Modal';
import { Music, Video, Info } from 'lucide-react';

import type { ScannedManifest } from '@/lib/tauri-commands';

interface MvGapFillModalProps {
  /** When true, the modal is rendered. */
  open: boolean;
  /** Invoked when the modal is dismissed (Escape, backdrop, X). */
  onClose: () => void;
  /**
   * Invoked with the user's chosen override value when they click
   * a Yes/No button. `null` means "inherit settings"; `true`/`false`
   * are explicit per-item overrides.
   *
   * The Library Scan page maps this to
   * `DownloadRequest.mv_companion_override` and calls `startDownload`.
   */
  onConfirm: (override: boolean | null) => void;
  /**
   * Whether `music_video_companion` is currently enabled in the user's
   * settings. Drives which of the two prompt variants is shown.
   */
  mvCompanionEnabledInSettings: boolean;
  /** The manifest row being acted on — used for the modal title. */
  manifest: ScannedManifest | null;
}

export function MvGapFillModal({
  open,
  onClose,
  onConfirm,
  mvCompanionEnabledInSettings,
  manifest,
}: MvGapFillModalProps) {
  if (!manifest) return null;

  const titleSuffix = manifest.album
    ? ` — ${manifest.album}`
    : '';

  const handleYes = () => {
    // ENABLED + Yes → inherit settings (no override needed)
    // DISABLED + Yes → force-enable for this item
    onConfirm(mvCompanionEnabledInSettings ? null : true);
  };
  const handleNo = () => {
    // ENABLED + No → force-disable for this item
    // DISABLED + No → inherit settings (already off)
    onConfirm(mvCompanionEnabledInSettings ? false : null);
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`Re-download gaps${titleSuffix}`}
      maxWidth="max-w-xl"
    >
      <div className="space-y-4">
        <div className="flex items-start gap-3 text-sm text-content-secondary">
          <Info
            size={18}
            className="flex-shrink-0 text-accent mt-0.5"
            aria-hidden="true"
          />
          {mvCompanionEnabledInSettings ? (
            <p>
              Music video companion is <strong>enabled</strong> in your
              settings. Music videos may be downloaded or updated as
              part of this gap-fill, alongside any missing audio
              tracks.
            </p>
          ) : (
            <p>
              Music video companion is <strong>currently disabled</strong>{' '}
              in your settings. By default this gap-fill will only
              re-download missing audio tracks.
            </p>
          )}
        </div>

        <div className="text-sm text-content-primary font-medium">
          {mvCompanionEnabledInSettings
            ? 'Include music videos in this gap-fill?'
            : 'Include music videos in this gap-fill anyway?'}
        </div>

        <p className="text-xs text-content-tertiary">
          Either choice applies to <strong>this re-download only</strong>{' '}
          — your global settings are not changed.
        </p>

        <div className="flex gap-2 justify-end pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 rounded-md text-sm border border-border text-content-secondary hover:bg-surface-secondary"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleNo}
            className="px-3 py-1.5 rounded-md text-sm border border-border flex items-center gap-1.5 hover:bg-surface-secondary"
            aria-label={
              mvCompanionEnabledInSettings
                ? 'Audio only for this re-download'
                : 'Audio only (inherits your disabled setting)'
            }
          >
            <Music size={14} />
            No — audio only
          </button>
          <button
            type="button"
            onClick={handleYes}
            className="px-3 py-1.5 rounded-md text-sm bg-accent text-white flex items-center gap-1.5 hover:bg-accent-hover"
            aria-label={
              mvCompanionEnabledInSettings
                ? 'Yes, inherit your enabled setting (audio + music videos)'
                : 'Yes, include music videos for this re-download only'
            }
          >
            <Video size={14} />
            Yes — include videos
          </button>
        </div>
      </div>
    </Modal>
  );
}
