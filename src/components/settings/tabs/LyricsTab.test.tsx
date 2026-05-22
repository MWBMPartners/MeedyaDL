/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file LyricsTab.test.tsx -- Unit tests for the Lyrics settings tab.
 *
 * Tests the multi-select checkbox behavior for lyrics format selection,
 * including primary/companion format assignment and interaction with the
 * "Disable Synced Lyrics" toggle.
 *
 * @see src/components/settings/tabs/LyricsTab.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';

import { LyricsTab } from './LyricsTab';
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * Reset the settings store to default state before each test.
 */
beforeEach(() => {
  useSettingsStore.setState({
    settings: {
      ...useSettingsStore.getState().settings,
      synced_lyrics_format: 'lrc',
      companion_lyrics_formats: [],
      no_synced_lyrics: false,
      embed_lyrics_and_sidecar: true,
      enhanced_lrc: false,
    },
  });
});

describe('LyricsTab', () => {
  // =========================================================================
  // Checkbox rendering
  // =========================================================================
  it('renders three format checkboxes', () => {
    render(<LyricsTab />);

    const checkboxes = screen.getAllByRole('checkbox');
    /* 3 format checkboxes + 3 toggle switches (embed, disable, lyrics-only) */
    expect(checkboxes.length).toBeGreaterThanOrEqual(3);

    /* Verify format labels are present */
    expect(screen.getByText(/^LRC/)).toBeInTheDocument();
    expect(screen.getByText(/^SRT/)).toBeInTheDocument();
    expect(screen.getByText(/^TTML/)).toBeInTheDocument();
  });

  it('checks only LRC by default', () => {
    render(<LyricsTab />);

    const lrcCheckbox = screen.getByRole('checkbox', { name: /LRC/ });
    const srtCheckbox = screen.getByRole('checkbox', { name: /SRT/ });
    const ttmlCheckbox = screen.getByRole('checkbox', { name: /TTML/ });

    expect(lrcCheckbox).toBeChecked();
    expect(srtCheckbox).not.toBeChecked();
    expect(ttmlCheckbox).not.toBeChecked();
  });

  // =========================================================================
  // Format selection behavior
  // =========================================================================
  it('adds a companion format when checking a second box', () => {
    render(<LyricsTab />);

    const srtCheckbox = screen.getByRole('checkbox', { name: /SRT/ });
    fireEvent.click(srtCheckbox);

    const settings = useSettingsStore.getState().settings;
    /* LRC stays as primary (first in canonical order) */
    expect(settings.synced_lyrics_format).toBe('lrc');
    /* SRT becomes a companion */
    expect(settings.companion_lyrics_formats).toEqual(['srt']);
  });

  it('assigns primary to first checked format in canonical order', () => {
    render(<LyricsTab />);

    /* Check TTML as well */
    const ttmlCheckbox = screen.getByRole('checkbox', { name: /TTML/ });
    fireEvent.click(ttmlCheckbox);

    const settings = useSettingsStore.getState().settings;
    /* LRC is still primary (first in LRC -> SRT -> TTML order) */
    expect(settings.synced_lyrics_format).toBe('lrc');
    expect(settings.companion_lyrics_formats).toEqual(['ttml']);
  });

  it('updates primary when the current primary is unchecked', () => {
    /* Start with LRC + SRT selected */
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        synced_lyrics_format: 'lrc',
        companion_lyrics_formats: ['srt'],
      },
    });

    render(<LyricsTab />);

    /* Uncheck LRC — SRT should become primary */
    const lrcCheckbox = screen.getByRole('checkbox', { name: /LRC/ });
    fireEvent.click(lrcCheckbox);

    const settings = useSettingsStore.getState().settings;
    expect(settings.synced_lyrics_format).toBe('srt');
    expect(settings.companion_lyrics_formats).toEqual([]);
  });

  it('prevents unchecking the last remaining format', () => {
    render(<LyricsTab />);

    /* Only LRC is checked — try to uncheck it */
    const lrcCheckbox = screen.getByRole('checkbox', { name: /LRC/ });
    fireEvent.click(lrcCheckbox);

    /* Should still be checked */
    const settings = useSettingsStore.getState().settings;
    expect(settings.synced_lyrics_format).toBe('lrc');
  });

  it('supports all three formats selected', () => {
    render(<LyricsTab />);

    fireEvent.click(screen.getByRole('checkbox', { name: /SRT/ }));
    fireEvent.click(screen.getByRole('checkbox', { name: /TTML/ }));

    const settings = useSettingsStore.getState().settings;
    expect(settings.synced_lyrics_format).toBe('lrc');
    expect(settings.companion_lyrics_formats).toEqual(['srt', 'ttml']);
  });

  // =========================================================================
  // Primary badge
  // =========================================================================
  it('shows "(Primary)" badge when multiple formats are selected', () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        synced_lyrics_format: 'lrc',
        companion_lyrics_formats: ['srt'],
      },
    });

    render(<LyricsTab />);

    expect(screen.getByText('(Primary)')).toBeInTheDocument();
  });

  it('does not show "(Primary)" badge when only one format is selected', () => {
    render(<LyricsTab />);

    expect(screen.queryByText('(Primary)')).not.toBeInTheDocument();
  });

  // =========================================================================
  // Disabled state
  // =========================================================================
  it('disables format checkboxes when lyrics are disabled and embed is off', () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        no_synced_lyrics: true,
        embed_lyrics_and_sidecar: false,
      },
    });

    render(<LyricsTab />);

    const lrcCheckbox = screen.getByRole('checkbox', { name: /LRC/ });
    expect(lrcCheckbox).toBeDisabled();
  });

  it('keeps format checkboxes enabled when lyrics disabled but embed is on', () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        no_synced_lyrics: true,
        embed_lyrics_and_sidecar: true,
      },
    });

    render(<LyricsTab />);

    const lrcCheckbox = screen.getByRole('checkbox', { name: /LRC/ });
    expect(lrcCheckbox).not.toBeDisabled();
  });
});
