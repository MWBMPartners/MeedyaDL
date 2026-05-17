// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Legacy sibling-folder merge UI (#789).
 *
 * Surfaces the three-phase backend pipeline (detect → preview →
 * execute) for cleaning up pre-#528 downloads that left two
 * sibling folders on disk (`Album/` + `Album [Explicit]/`).
 *
 * Lives alongside the existing Library Scan view rather than
 * inside it so the manifest-scan path and the legacy-merge path
 * stay independent — a user might want to run one without the
 * other.
 */

import { useState } from 'react';
import { FolderOpen, RefreshCw, AlertTriangle, CheckCircle2, FileWarning } from 'lucide-react';

import { Button, Modal } from '@/components/common';
import { useUiStore } from '@/stores/uiStore';
import {
  detectLegacyFolderPairs,
  previewLegacyFolderMerge,
  executeLegacyFolderMerge,
  type SiblingPair,
  type MergePreview,
  type MergeReport,
} from '@/lib/tauri-commands';

/**
 * Standalone section embedded on `LibraryScanPage`. Owns its own
 * folder picker / scan state because the merge flow is independent
 * of the manifest-scan flow (a user might want to clean up legacy
 * pairs even on a library with no `.meedyadl` manifests).
 */
export function LegacyFolderMergeSection() {
  const addToast = useUiStore((s) => s.addToast);

  const [scanning, setScanning] = useState(false);
  const [pairs, setPairs] = useState<SiblingPair[] | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [activePreview, setActivePreview] = useState<MergePreview | null>(null);
  const [executing, setExecuting] = useState(false);
  const [lastReport, setLastReport] = useState<MergeReport | null>(null);

  /** Picks a folder, then asks the backend to detect sibling pairs in it. */
  const handleScan = async () => {
    setScanning(true);
    setPairs(null);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        directory: true,
        title: 'Choose your music root to scan for legacy sibling folders',
      });
      if (typeof selected !== 'string' || selected.length === 0) {
        setScanning(false);
        return; // user cancelled
      }
      const found = await detectLegacyFolderPairs(selected);
      setPairs(found);
      if (found.length === 0) {
        addToast(
          'No legacy sibling-folder pairs found. Your library is already in the post-#528 layout.',
          'success'
        );
      } else {
        addToast(
          `Found ${found.length} legacy sibling-folder pair${found.length === 1 ? '' : 's'} that can be merged.`,
          'info'
        );
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.toLowerCase().includes('cancel')) {
        addToast(`Scan failed: ${msg}`, 'error');
      }
    } finally {
      setScanning(false);
    }
  };

  /** Loads the preview for a single pair and opens the confirmation modal. */
  const handlePreview = async (pair: SiblingPair) => {
    setPreviewing(true);
    try {
      const preview = await previewLegacyFolderMerge(pair);
      setActivePreview(preview);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast(`Preview failed: ${msg}`, 'error');
    } finally {
      setPreviewing(false);
    }
  };

  /** Runs the merge for the currently-previewed pair. */
  const handleConfirmMerge = async () => {
    if (!activePreview) return;
    setExecuting(true);
    try {
      const report = await executeLegacyFolderMerge(activePreview.pair);
      setLastReport(report);
      // Drop the now-merged pair from the list so the user can
      // see the remaining work without re-scanning.
      setPairs(
        (prev) =>
          prev?.filter(
            (p) =>
              p.unsuffixed_path !== report.pair.unsuffixed_path
          ) ?? null
      );
      setActivePreview(null);
      addToast(
        `Merged "${report.pair.album_basename}" — ${report.audio_moved} audio + ${report.sidecars_moved} sidecars moved${report.warnings.length > 0 ? ` (${report.warnings.length} warning${report.warnings.length === 1 ? '' : 's'})` : ''}`,
        report.warnings.length > 0 ? 'warning' : 'success'
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast(`Merge failed: ${msg}`, 'error');
    } finally {
      setExecuting(false);
    }
  };

  return (
    <section className="mt-8 pt-6 border-t border-border-light">
      <header className="mb-3">
        <h2 className="text-lg font-semibold text-content-primary">
          Legacy folder cleanup
        </h2>
        <p className="text-sm text-content-secondary max-w-3xl mt-1">
          Albums downloaded before v1.4.4 with companion codecs enabled
          (e.g. Atmos primary + ALAC companion) and the{' '}
          <code className="font-mono text-xs">[Explicit]</code> /{' '}
          <code className="font-mono text-xs">[Clean]</code> filename
          suffix produced two sibling folders on disk. v1.4.4 prevents
          this for new downloads; this tool merges any pairs left over
          from older downloads into the single post-#528 layout.
        </p>
      </header>

      <div className="mb-4 flex items-center gap-3">
        <Button
          variant="secondary"
          onClick={handleScan}
          disabled={scanning}
          icon={scanning ? <RefreshCw size={16} className="animate-spin" /> : <FolderOpen size={16} />}
        >
          {scanning ? 'Scanning…' : 'Scan for legacy folder pairs'}
        </Button>
        {pairs !== null && (
          <span className="text-sm text-content-secondary">
            {pairs.length} pair{pairs.length === 1 ? '' : 's'} found
          </span>
        )}
      </div>

      {pairs !== null && pairs.length > 0 && (
        <div className="border border-border-light rounded-md overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-surface-secondary text-content-secondary text-xs uppercase tracking-wider">
              <tr>
                <th className="text-left px-3 py-2">Album</th>
                <th className="text-left px-3 py-2">Suffix</th>
                <th className="text-left px-3 py-2">Parent folder</th>
                <th className="text-right px-3 py-2">Action</th>
              </tr>
            </thead>
            <tbody>
              {pairs.map((pair) => (
                <tr
                  key={pair.unsuffixed_path}
                  className="border-t border-border-light hover:bg-surface-secondary/50"
                >
                  <td className="px-3 py-2 font-medium text-content-primary">
                    {pair.album_basename}
                  </td>
                  <td className="px-3 py-2 text-content-secondary font-mono text-xs">
                    {pair.suffix}
                  </td>
                  <td className="px-3 py-2 text-content-tertiary font-mono text-xs truncate max-w-md" title={pair.parent}>
                    {pair.parent}
                  </td>
                  <td className="px-3 py-2 text-right">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handlePreview(pair)}
                      disabled={previewing || executing}
                    >
                      {previewing ? 'Loading…' : 'Preview merge'}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {pairs !== null && pairs.length === 0 && lastReport === null && (
        <p className="text-content-tertiary text-sm">
          No legacy sibling-folder pairs found in the chosen folder.
        </p>
      )}

      {/* Preview / confirmation modal */}
      {activePreview && (
        <Modal
          open
          onClose={() => setActivePreview(null)}
          title={`Merge "${activePreview.pair.album_basename}"?`}
        >
          <div className="space-y-3 text-sm">
            <p className="text-content-secondary">
              Everything in{' '}
              <span className="font-mono text-content-primary">
                {activePreview.pair.unsuffixed_path}
              </span>{' '}
              will be renamed with the{' '}
              <code className="font-mono">
                {activePreview.pair.suffix}
              </code>{' '}
              advisory suffix (where appropriate), moved into{' '}
              <span className="font-mono text-content-primary">
                {activePreview.pair.suffixed_path}
              </span>
              , and the now-empty source folder removed.
            </p>

            <ul className="text-content-primary space-y-1 list-disc list-inside">
              <li>{activePreview.audio_count} audio file{activePreview.audio_count === 1 ? '' : 's'}</li>
              <li>{activePreview.sidecar_count} lyric / subtitle sidecar{activePreview.sidecar_count === 1 ? '' : 's'}</li>
              <li>{activePreview.other_count} other file{activePreview.other_count === 1 ? '' : 's'} (cover art, etc.)</li>
              {activePreview.will_merge_manifest && (
                <li>
                  <code className="font-mono text-xs">manifest.meedyadl</code> will be merged
                </li>
              )}
            </ul>

            {activePreview.potential_collisions.length > 0 && (
              <div className="rounded-md border border-status-warning/30 bg-status-warning/10 p-3 text-status-warning text-xs">
                <div className="flex items-center gap-2 font-medium mb-1">
                  <FileWarning size={14} />
                  Potential filename collisions
                </div>
                <p className="text-content-secondary mb-2">
                  These files already exist in the destination. They
                  won't be overwritten — colliders will be renamed{' '}
                  <code className="font-mono">name.1.ext</code>,{' '}
                  <code className="font-mono">name.2.ext</code>, etc.
                  so nothing is lost.
                </p>
                <ul className="font-mono text-xs space-y-0.5">
                  {activePreview.potential_collisions
                    .slice(0, 5)
                    .map((name) => (
                      <li key={name}>{name}</li>
                    ))}
                  {activePreview.potential_collisions.length > 5 && (
                    <li>
                      …{' '}
                      {activePreview.potential_collisions.length - 5}{' '}
                      more
                    </li>
                  )}
                </ul>
              </div>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <Button
                variant="secondary"
                onClick={() => setActivePreview(null)}
                disabled={executing}
              >
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={handleConfirmMerge}
                disabled={executing}
                icon={executing ? <RefreshCw size={14} className="animate-spin" /> : <CheckCircle2 size={14} />}
              >
                {executing ? 'Merging…' : 'Merge folders'}
              </Button>
            </div>
          </div>
        </Modal>
      )}

      {/* Post-merge report toast is already shown via addToast; we
          surface non-fatal warnings inline so the user has a visible
          paper trail of anything that didn't go perfectly. */}
      {lastReport && lastReport.warnings.length > 0 && (
        <div className="mt-4 rounded-md border border-status-warning/30 bg-status-warning/10 p-3 text-status-warning text-xs">
          <div className="flex items-center gap-2 font-medium mb-2">
            <AlertTriangle size={14} />
            Merge of "{lastReport.pair.album_basename}" completed with warnings
          </div>
          <ul className="space-y-0.5 text-content-secondary">
            {lastReport.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
