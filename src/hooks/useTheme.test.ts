/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/hooks/useTheme.test.ts - Unit tests for the theme override hook
 *
 * Tests that the useTheme hook correctly manages CSS classes and the
 * color-scheme property on the <html> element based on the theme_override
 * setting from the settings store.
 *
 * Uses @testing-library/react's renderHook() to test the hook in isolation
 * without mounting a full component tree.
 *
 * @see src/hooks/useTheme.ts - The hook under test
 * @see src/stores/settingsStore.ts - Source of the theme_override setting
 */

import { renderHook, act } from '@testing-library/react';

import { useTheme } from '@/hooks/useTheme';
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * Helper to get the <html> element for assertions.
 * In jsdom, document.documentElement is always available.
 */
const getHtml = () => document.documentElement;

/**
 * Clean up the <html> element between tests so class/style mutations
 * from one test don't leak into the next.
 */
beforeEach(() => {
  getHtml().classList.remove('theme-dark', 'theme-light');
  getHtml().style.colorScheme = '';

  /* Reset the settings store to default (theme_override: null = auto) */
  useSettingsStore.setState({
    settings: {
      ...useSettingsStore.getState().settings,
      theme_override: null,
    },
  });
});

describe('useTheme', () => {
  // =========================================================================
  // Auto Mode (theme_override = null)
  // =========================================================================
  it('adds no theme classes in auto mode', () => {
    renderHook(() => useTheme());

    expect(getHtml().classList.contains('theme-dark')).toBe(false);
    expect(getHtml().classList.contains('theme-light')).toBe(false);
  });

  it('clears color-scheme in auto mode', () => {
    /* Pre-set a value to verify it gets cleared */
    getHtml().style.colorScheme = 'dark';

    renderHook(() => useTheme());

    expect(getHtml().style.colorScheme).toBe('');
  });

  // =========================================================================
  // Dark Mode Override
  // =========================================================================
  it('adds theme-dark class when override is dark', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'dark' },
    });

    renderHook(() => useTheme());

    expect(getHtml().classList.contains('theme-dark')).toBe(true);
    expect(getHtml().classList.contains('theme-light')).toBe(false);
  });

  it('sets color-scheme to dark', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'dark' },
    });

    renderHook(() => useTheme());

    expect(getHtml().style.colorScheme).toBe('dark');
  });

  // =========================================================================
  // Light Mode Override
  // =========================================================================
  it('adds theme-light class when override is light', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'light' },
    });

    renderHook(() => useTheme());

    expect(getHtml().classList.contains('theme-light')).toBe(true);
    expect(getHtml().classList.contains('theme-dark')).toBe(false);
  });

  it('sets color-scheme to light', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'light' },
    });

    renderHook(() => useTheme());

    expect(getHtml().style.colorScheme).toBe('light');
  });

  // =========================================================================
  // Reactivity: Switching between modes
  // =========================================================================
  it('switches from dark to light when setting changes', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'dark' },
    });

    renderHook(() => useTheme());
    expect(getHtml().classList.contains('theme-dark')).toBe(true);

    /* Simulate the user changing the theme in settings */
    act(() => {
      useSettingsStore.setState({
        settings: { ...useSettingsStore.getState().settings, theme_override: 'light' },
      });
    });

    expect(getHtml().classList.contains('theme-light')).toBe(true);
    expect(getHtml().classList.contains('theme-dark')).toBe(false);
    expect(getHtml().style.colorScheme).toBe('light');
  });

  it('switches from light to auto when setting changes', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'light' },
    });

    renderHook(() => useTheme());
    expect(getHtml().classList.contains('theme-light')).toBe(true);

    /* Switch to auto */
    act(() => {
      useSettingsStore.setState({
        settings: { ...useSettingsStore.getState().settings, theme_override: null },
      });
    });

    expect(getHtml().classList.contains('theme-light')).toBe(false);
    expect(getHtml().classList.contains('theme-dark')).toBe(false);
    expect(getHtml().style.colorScheme).toBe('');
  });

  // =========================================================================
  // Cleanup on unmount
  // =========================================================================
  it('removes theme classes when the hook unmounts', () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, theme_override: 'dark' },
    });

    const { unmount } = renderHook(() => useTheme());
    expect(getHtml().classList.contains('theme-dark')).toBe(true);

    unmount();

    expect(getHtml().classList.contains('theme-dark')).toBe(false);
    expect(getHtml().style.colorScheme).toBe('');
  });
});
