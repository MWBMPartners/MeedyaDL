// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for the Reinstall button on each tool row in
 * Settings > Tools (issue #1225).
 *
 * Before this button existed, a tool that was present and started but
 * reported an error instead of its version counted as "installed", so
 * its row offered no button at all -- nothing the person could press to
 * repair it. These tests exercise the real `ToolsTab` component and the
 * real dependency store, with only the Tauri commands mocked, and cover:
 *   - An installed tool gets a Reinstall button; a missing one gets
 *     Install instead.
 *   - Clicking Reinstall asks first, and installs nothing until the
 *     person confirms.
 *   - Confirming runs the same install command as Install -- WITHOUT the
 *     "this is an update" flag, which would stop the backend using a copy
 *     already on the computer or the backup download source -- then
 *     refreshes the tool list and says it worked.
 *   - A failure is shown, and the tool list is still refreshed, because
 *     the backend may have put the previous copy back.
 *   - Cancel installs nothing.
 *
 * Separate from `ToolsTab.backupDelete.test.tsx`, which deliberately
 * leaves the tool list empty.
 *
 * @see src/components/settings/tabs/ToolsTab.tsx -- the component under test
 */

import { render, screen, fireEvent, waitFor, within, act } from '@testing-library/react';
import type { Mock } from 'vitest';

import { ToolsTab } from './ToolsTab';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useUiStore } from '@/stores/uiStore';
import { checkAllDependencies, installDependency } from '@/lib/tauri-commands';
import type { DependencyStatus } from '@/types';

/**
 * Mock the Tauri command wrappers. ToolsTab imports the first six itself
 * (the Backups section calls `listBackups` on mount); the dependency
 * store's real `installTool` action calls `installDependency` and then
 * `checkAllDependencies`, which is exactly the path under test.
 */
vi.mock('@/lib/tauri-commands', () => ({
  installGamdlVersion: vi.fn(),
  getGamdlSupportWindow: vi.fn(),
  createBackup: vi.fn(),
  listBackups: vi.fn().mockResolvedValue([]),
  restoreFromBackup: vi.fn(),
  deleteBackup: vi.fn(),
  installDependency: vi.fn(),
  checkAllDependencies: vi.fn(),
}));

/** Builds one tool row's status as the backend reports it. */
function tool(name: string, installed: boolean): DependencyStatus {
  return {
    name,
    required: true,
    installed,
    version: null,
    path: installed ? `/tools/${name}` : null,
    source: installed ? 'managed' : null,
    classification: null,
    known_bad_message: null,
  };
}

const TOOLS: DependencyStatus[] = [tool('FFmpeg', true), tool('mp4decrypt', false)];

let checkAll: Mock<() => Promise<void>>;

beforeEach(() => {
  // `checkAll` normally fires real IPC calls on mount; it is replaced
  // with a spy so the tests can also see the refresh after a failure.
  // `installTool` is left as the store's real action on purpose.
  checkAll = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  act(() => {
    useDependencyStore.setState({
      python: null,
      gamdl: null,
      tools: TOOLS,
      isChecking: false,
      isInstalling: false,
      installingName: null,
      error: null,
      checkAll,
    });
    useUiStore.setState({ toasts: [] });
  });
  vi.mocked(installDependency).mockReset().mockResolvedValue('7.1');
  vi.mocked(checkAllDependencies).mockReset().mockResolvedValue(TOOLS);
});

/** The text of every toast currently held by the UI store. */
function toastMessages(): string[] {
  return useUiStore.getState().toasts.map((t) => t.message);
}

describe('ToolsTab -- Reinstall button', () => {
  it('shows Reinstall on an installed tool and Install on a missing one', () => {
    render(<ToolsTab />);

    expect(screen.getByRole('button', { name: 'Reinstall FFmpeg' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Reinstall mp4decrypt' })).not.toBeInTheDocument();
    // The missing tool keeps its ordinary Install button.
    expect(screen.getByRole('button', { name: /^Install$/ })).toBeInTheDocument();
  });

  it('asks first, and installs nothing until the person confirms', () => {
    render(<ToolsTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Reinstall FFmpeg' }));

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('Reinstall FFmpeg?')).toBeInTheDocument();
    // Pins the condition, not just the promise: the current copy is kept
    // only when IT works and the new one does not.
    expect(
      within(dialog).getByText(
        /If your current copy starts and reports a version MeedyaDL recognises, and the new copy does not, your current copy is kept\./,
      ),
    ).toBeInTheDocument();
    expect(installDependency).not.toHaveBeenCalled();
  });

  it('runs the ordinary install (not an update) once confirmed, then refreshes and says so', async () => {
    render(<ToolsTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Reinstall FFmpeg' }));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^Reinstall$/ }));

    // Exactly one argument: no "for update" flag, so the backend treats it
    // as an install or repair, like the Install button.
    await waitFor(() => expect(installDependency).toHaveBeenCalledWith('FFmpeg'));
    expect(vi.mocked(installDependency).mock.calls[0]).toHaveLength(1);
    await waitFor(() => expect(checkAllDependencies).toHaveBeenCalled());
    await waitFor(() => expect(toastMessages()).toContain('FFmpeg has been reinstalled.'));
    // The confirmation closes as soon as the install starts.
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows a failure, and still refreshes the tool list', async () => {
    vi.mocked(installDependency).mockRejectedValue(
      'The new copy of FFmpeg would not start on this computer (it printed "dyld: Library not loaded" instead of its version), so it has been removed. The copy of FFmpeg you had before has been kept, and works as it did.',
    );
    render(<ToolsTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Reinstall FFmpeg' }));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^Reinstall$/ }));

    await waitFor(() =>
      expect(toastMessages().some((m) => m.startsWith('MeedyaDL could not reinstall FFmpeg.'))).toBe(true),
    );
    expect(toastMessages().some((m) => m.includes('has been kept'))).toBe(true);
    await waitFor(() => expect(checkAll).toHaveBeenCalledTimes(2)); // on mount, and after the failure
  });

  it('says a package manager may be asked, for a copy that came from one', () => {
    // A Homebrew or APT copy is not downloaded again: the install may ask
    // that package manager to update it, which can ask for a password. The
    // window must say that, not describe a download.
    act(() => {
      useDependencyStore.setState({
        tools: [{ ...tool('FFmpeg', true), source: 'apt:ffmpeg' }],
      });
    });
    render(<ToolsTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Reinstall FFmpeg' }));

    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText(/came from APT/)).toBeInTheDocument();
    expect(within(dialog).getByText(/ask for your password/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/download/i)).not.toBeInTheDocument();
  });

  it('Cancel installs nothing', () => {
    render(<ToolsTab />);

    fireEvent.click(screen.getByRole('button', { name: 'Reinstall FFmpeg' }));
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^Cancel$/ }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(installDependency).not.toHaveBeenCalled();
  });
});
