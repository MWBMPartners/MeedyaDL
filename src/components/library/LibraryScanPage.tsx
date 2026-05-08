// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Library Scan page (Phase 5 scaffold, #717).
 *
 * Surfaces the existing `scan_folder_for_manifests` IPC + smart-retry
 * planner infrastructure as a first-class UI flow:
 *
 *   1. User picks a root folder (typically their Music library).
 *   2. The backend recursively scans for `manifest.meedyadl` files
 *      (depth-bounded to 10 — see `scan_folder_for_manifests`).
 *   3. For each found manifest, MeedyaDL diffs the recorded
 *      track list against what's actually on disk and against the
 *      Apple Music API's current `lastModifiedDate`.
 *   4. Items with gaps (missing tracks, format upgrades available,
 *      content updates from Apple) get a "Re-download" action.
 *   5. **MV gap-fill prompt**: when the user clicks Re-download,
 *      a modal asks whether to include music videos:
 *      - if `music_video_companion` is ENABLED in settings, the
 *        prompt confirms "Music videos may be downloaded/updated.
 *        Include?" (Yes = inherit settings; No = audio only for
 *        this item, settings unchanged for others)
 *      - if `music_video_companion` is DISABLED in settings, the
 *        prompt asks "Include music videos in this gap-fill?"
 *        (Yes = enable for this one item only; No = audio only)
 *
 * **This file is the Phase 5a scaffold** — page shell + folder
 * picker + scan results table. The smart-retry diff display
 * (5b/5c), MV gap-fill prompts (5d), per-item override plumbing
 * (5e), and queue-wiring (5f) land in subsequent commits per the
 * tracker (#717). Stub behaviour today: clicking Scan invokes the
 * existing IPC and renders the raw `ScannedManifest` records.
 *
 * @see scan_folder_for_manifests in src-tauri/src/commands/gamdl.rs
 * @see smart_retry_planner in src-tauri/src/services/smart_retry_planner.rs
 * @see check_redownload_status in src-tauri/src/commands/gamdl.rs
 */

import { useState } from 'react';
import { FolderOpen, RefreshCw } from 'lucide-react';

import {
  scanFolderForManifests,
  type ScannedManifest,
} from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';

export function LibraryScanPage() {
  const addToast = useUiStore((s) => s.addToast);
  const [scanning, setScanning] = useState(false);
  const [results, setResults] = useState<ScannedManifest[] | null>(null);

  const handleScan = async () => {
    setScanning(true);
    try {
      const manifests = await scanFolderForManifests();
      setResults(manifests);
      addToast(
        `Scan complete — found ${manifests.length} manifest(s)`,
        'success'
      );
    } catch (err) {
      // User cancelled the folder picker, or I/O failure.
      const msg = err instanceof Error ? err.message : String(err);
      // "cancelled" is the expected user-cancelled-folder-picker
      // path; suppress the toast for it (matching the convention
      // used by `useDownloadStore.startDownload`).
      if (!msg.toLowerCase().includes('cancel')) {
        addToast(`Scan failed: ${msg}`, 'error');
      }
    } finally {
      setScanning(false);
    }
  };

  return (
    <div className="flex flex-col h-full p-6 overflow-y-auto">
      <header className="mb-6">
        <h1 className="text-2xl font-semibold text-content-primary mb-2">
          Library Scan
        </h1>
        <p className="text-sm text-content-secondary max-w-3xl">
          Point MeedyaDL at an existing music library to find downloads
          that are missing tracks, have a higher quality available, or
          have been updated by Apple Music since you last fetched them.
          Re-download just the gaps without losing anything you already
          have.
        </p>
      </header>

      <div className="mb-6 flex items-center gap-3">
        <button
          type="button"
          onClick={handleScan}
          disabled={scanning}
          className="px-4 py-2 rounded-md bg-accent text-white text-sm font-medium flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-accent-hover transition-colors"
          aria-label="Choose a folder and scan for manifest files"
        >
          {scanning ? (
            <>
              <RefreshCw size={16} className="animate-spin" />
              Scanning…
            </>
          ) : (
            <>
              <FolderOpen size={16} />
              Choose folder & scan
            </>
          )}
        </button>
        {results && (
          <span className="text-sm text-content-secondary">
            {results.length} manifest(s) found
          </span>
        )}
      </div>

      {results === null ? (
        <div className="flex-1 flex items-center justify-center text-content-tertiary text-sm">
          No scan run yet. Click <em className="mx-1">Choose folder &amp; scan</em>{' '}
          to begin.
        </div>
      ) : results.length === 0 ? (
        <div className="flex-1 flex items-center justify-center text-content-tertiary text-sm">
          No <code className="font-mono text-xs">manifest.meedyadl</code> files
          were found in the selected folder. MeedyaDL only writes manifests
          for downloads it has performed — third-party downloads aren't
          recognised.
        </div>
      ) : (
        <div className="border border-border rounded-md overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-surface-secondary text-content-secondary">
              <tr>
                <th className="text-left px-4 py-2 font-medium">Artist</th>
                <th className="text-left px-4 py-2 font-medium">Album</th>
                <th className="text-right px-4 py-2 font-medium">Tracks</th>
                <th className="text-right px-4 py-2 font-medium">Files</th>
                <th className="text-left px-4 py-2 font-medium">Codec</th>
                <th className="text-left px-4 py-2 font-medium">Last download</th>
              </tr>
            </thead>
            <tbody>
              {results.map((m) => (
                <tr
                  key={m.manifest_path}
                  className="border-t border-border/40 hover:bg-surface-secondary/50"
                >
                  <td className="px-4 py-2 truncate max-w-[200px]">
                    {m.artist ?? '—'}
                  </td>
                  <td className="px-4 py-2 truncate max-w-[280px]">
                    {m.album ?? '—'}
                  </td>
                  <td className="px-4 py-2 text-right tabular-nums">
                    {m.track_count}
                  </td>
                  <td className="px-4 py-2 text-right tabular-nums">
                    {m.audio_file_count}
                  </td>
                  <td className="px-4 py-2 uppercase text-xs text-content-secondary">
                    {m.current_codec ?? '—'}
                  </td>
                  <td className="px-4 py-2 text-xs text-content-tertiary">
                    {m.downloaded_at
                      ? new Date(m.downloaded_at).toLocaleDateString()
                      : '—'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/*
        Phase 5 follow-ups tracked in #717:
        - 5b: Smart-retry diff per row (compare manifest tracks vs disk)
        - 5c: lastModifiedDate diff against Apple Music API (content-update detection)
        - 5d: MV gap-fill modal (two prompt branches per settings)
        - 5e: Per-item music_video_companion override on QueueItem
        - 5f: "Re-download gaps" action button per row → enqueue with override
      */}
    </div>
  );
}
