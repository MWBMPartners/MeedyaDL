// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file Save is off, with a reason and a retry, until settings have loaded (#1237).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { SettingsPage } from '@/components/settings/SettingsPage';
import { useSettingsStore } from '@/stores/settingsStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
  initReactI18next: { type: '3rdParty', init: () => {} },
}));

vi.mock('@/components/settings/tabs/GeneralTab', () => ({
  GeneralTab: () => <div>general tab</div>,
}));

const loadSettings = vi.fn();

beforeEach(() => {
  loadSettings.mockReset();
  loadSettings.mockResolvedValue(undefined);
  useSettingsStore.setState({ hasLoaded: false, isLoading: false, isDirty: true, loadSettings });
});

describe('SettingsPage when settings could not be loaded', () => {
  it('disables Save, says why, and offers to try loading again', async () => {
    render(<SettingsPage />);
    expect(
      await screen.findByText(/Settings could not be loaded, so they cannot be saved/)
    ).toBeInTheDocument();
    expect((screen.getByRole('button', { name: /Save Changes/ }) as HTMLButtonElement).disabled).toBe(true);

    const before = loadSettings.mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: 'Try loading again' }));
    await waitFor(() => expect(loadSettings.mock.calls.length).toBe(before + 1));
  });

  it('shows no warning once settings have loaded', () => {
    useSettingsStore.setState({ hasLoaded: true });
    render(<SettingsPage />);
    expect(screen.queryByText(/cannot be saved/)).not.toBeInTheDocument();
    expect((screen.getByRole('button', { name: /Save Changes/ }) as HTMLButtonElement).disabled).toBe(false);
  });
});
