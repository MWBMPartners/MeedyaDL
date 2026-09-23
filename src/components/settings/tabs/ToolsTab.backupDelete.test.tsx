// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for ToolsTab's Backups section "Delete" confirmation
 * (code review fix, September 2026).
 *
 * A review found that each backup snapshot row has two buttons side by
 * side: "Restore" (a labelled button that already opens a confirmation
 * explaining what it will overwrite) and "Delete" (an icon-only "X"
 * with no visible text, right next to it) -- and Delete removed the
 * snapshot on the very first click, with no confirmation at all. Only
 * the 10 most recent snapshots are kept, and each one holds settings,
 * queue and history together, so a mis-click aimed at Restore could
 * permanently destroy the one snapshot someone actually needed.
 *
 * These tests exercise the real `ToolsTab` component (not a
 * re-implementation of its logic) and cover exactly the fix:
 *   - Clicking Delete opens a confirmation modal instead of deleting
 *     immediately.
 *   - `delete_backup` is only called once the user confirms.
 *   - Cancelling leaves the snapshot in place.
 *
 * This is a separate file from a full `ToolsTab.test.tsx` (there isn't
 * one yet) because exercising the Backups section only needs the
 * dependency-status fields left empty -- see the `beforeEach` below --
 * which would be a strange starting point for a file that also wants
 * to cover the Core Dependencies / External Tools sections properly.
 *
 * @see src/components/settings/tabs/ToolsTab.tsx -- the component under test
 */

import { render, screen, fireEvent, waitFor, within, act } from '@testing-library/react';

import { ToolsTab } from './ToolsTab';
import { useDependencyStore } from '@/stores/dependencyStore';
import { deleteBackup, listBackups, restoreFromBackup, type BackupEntry } from '@/lib/tauri-commands';

/**
 * Mock the Tauri IPC command wrappers ToolsTab imports. Only
 * `listBackups` and `deleteBackup` matter for these tests; the rest
 * are plain stubs so the component doesn't throw if it touches them
 * (e.g. `getGamdlSupportWindow` would only be called if `gamdl.installed`
 * were true, which it never is here -- see the `beforeEach` below).
 */
vi.mock('@/lib/tauri-commands', () => ({
  installGamdlVersion: vi.fn(),
  getGamdlSupportWindow: vi.fn(),
  createBackup: vi.fn(),
  listBackups: vi.fn(),
  restoreFromBackup: vi.fn().mockResolvedValue({ snapshot_path: '', restored: [], skipped: [] }),
  deleteBackup: vi.fn().mockResolvedValue(undefined),
}));

const SNAPSHOTS: BackupEntry[] = [
  { path: '/backups/20260101-120000', name: '20260101-120000', size_bytes: 2048, file_count: 3 },
  { path: '/backups/20260102-090000', name: '20260102-090000', size_bytes: 4096, file_count: 5 },
];

beforeEach(() => {
  // ToolsTab checks Python / GAMDL / tool status on mount through the
  // dependency store's `checkAll` action, which normally fires real
  // IPC calls the test environment can't answer. These tests only
  // care about the Backups section, so `checkAll` is replaced with a
  // no-op and the status fields are left empty -- Core Dependencies
  // and External Tools then render nothing, which is fine since this
  // file doesn't assert on them.
  act(() => {
    useDependencyStore.setState({
      python: null,
      gamdl: null,
      tools: [],
      isChecking: false,
      isInstalling: false,
      installingName: null,
      error: null,
      checkAll: vi.fn().mockResolvedValue(undefined),
    });
  });
  vi.mocked(listBackups).mockReset().mockResolvedValue(SNAPSHOTS);
  vi.mocked(deleteBackup).mockReset().mockResolvedValue(undefined);
  vi.mocked(restoreFromBackup)
    .mockReset()
    .mockResolvedValue({ snapshot_path: '', restored: [], skipped: [] });
});

describe('ToolsTab -- Backups "Delete" confirmation', () => {
  it('does not delete anything on click -- opens a confirmation modal instead', async () => {
    render(<ToolsTab />);
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: /delete snapshot/i })).toHaveLength(2)
    );

    fireEvent.click(screen.getAllByRole('button', { name: /delete snapshot/i })[0]);

    expect(screen.getByText('Delete snapshot?')).toBeInTheDocument();
    expect(deleteBackup).not.toHaveBeenCalled();
  });

  it('explains the loss is permanent and that it reduces the available restore points', async () => {
    render(<ToolsTab />);
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: /delete snapshot/i })).toHaveLength(2)
    );

    fireEvent.click(screen.getAllByRole('button', { name: /delete snapshot/i })[0]);

    expect(screen.getByText(/cannot be undone/i)).toBeInTheDocument();
    // The part that makes this snapshot's loss more than routine: only
    // 10 are kept, and losing the wrong one removes the option to undo
    // whatever the user was actually trying to fix.
    expect(screen.getByText(/10 most recent snapshots/i)).toBeInTheDocument();
  });

  it('deletes the snapshot only after the user confirms, calling delete_backup with its path', async () => {
    render(<ToolsTab />);
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: /delete snapshot/i })).toHaveLength(2)
    );

    fireEvent.click(screen.getAllByRole('button', { name: /delete snapshot/i })[0]);
    const dialog = screen.getByRole('dialog');
    // The row trigger's accessible name is "Delete snapshot <name>"
    // (an aria-label, since the button has no visible text); the
    // modal's confirm button is plain "Delete" -- an exact match keeps
    // this query from also matching the row button underneath.
    fireEvent.click(within(dialog).getByRole('button', { name: /^Delete$/ }));

    await waitFor(() => expect(deleteBackup).toHaveBeenCalledWith(SNAPSHOTS[0].path));
    // Modal closes once the delete resolves.
    await waitFor(() => expect(screen.queryByText('Delete snapshot?')).not.toBeInTheDocument());
  });

  it('Cancel leaves the snapshot in place', async () => {
    render(<ToolsTab />);
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: /delete snapshot/i })).toHaveLength(2)
    );

    fireEvent.click(screen.getAllByRole('button', { name: /delete snapshot/i })[0]);
    expect(screen.getByText('Delete snapshot?')).toBeInTheDocument();

    const dialog = screen.getByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: /^Cancel$/ }));

    expect(screen.queryByText('Delete snapshot?')).not.toBeInTheDocument();
    expect(deleteBackup).not.toHaveBeenCalled();
  });
});
