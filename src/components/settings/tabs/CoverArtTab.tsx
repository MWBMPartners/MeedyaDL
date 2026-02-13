/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CoverArtTab.tsx -- Cover art preferences settings tab.
 *
 * Renders the "Cover Art" tab within the {@link SettingsPage} component.
 * This tab controls two main areas:
 *
 * ## Static Cover Art (GAMDL)
 *
 *   - **Save Cover Art** -- Toggle that enables/disables cover art saving.
 *     Maps to `settings.save_cover` and GAMDL's `--save-cover` flag.
 *     When disabled, the format and size controls are hidden.
 *
 *   - **Cover Format** -- The image format for saved cover art files:
 *       - Raw: Original format from Apple Music (typically JPEG)
 *       - JPEG: Compressed, smaller file size
 *       - PNG: Lossless, larger file size
 *     Maps to `settings.cover_format` and GAMDL's `--cover-format` flag.
 *
 *   - **Cover Size** -- Width and height in pixels (square) for the cover
 *     art image. Valid range: 100-3000. Maps to `settings.cover_size` and
 *     GAMDL's `--cover-size` flag.
 *
 * ## Animated Artwork (MusicKit API)
 *
 *   - **Download Animated Cover Art** -- Toggle to enable/disable automatic
 *     downloading of animated (motion) cover art after album downloads.
 *     Maps to `settings.animated_artwork_enabled`.
 *
 *   - **MusicKit Team ID** -- Apple Developer Team ID for API auth.
 *     Maps to `settings.musickit_team_id`.
 *
 *   - **MusicKit Key ID** -- MusicKit private key identifier for API auth.
 *     Maps to `settings.musickit_key_id`.
 *
 *   - **MusicKit Private Key** -- Content of the `.p8` private key file.
 *     Stored securely in the OS keychain (not in settings JSON).
 *
 * ## Conditional Rendering
 *
 * Format/size controls are shown only when `save_cover` is true.
 * MusicKit credential inputs are shown only when `animated_artwork_enabled`
 * is true.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore` for all fields except
 * the private key, which uses the `storeCredential` / `getCredential`
 * IPC commands for OS keychain storage.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- CoverFormat type definition
 */

import { useState, useEffect, useCallback } from 'react';

// Zustand store for reading/writing cover art settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for format dropdown, Toggle for the save switch,
// Input for the size number field.
import { Select, Toggle, Input } from '@/components/common';

// IPC wrappers for keychain credential storage (private key).
import { storeCredential, getCredential } from '@/lib/tauri-commands';

// TypeScript union type for cover format values.
import type { CoverFormat } from '@/types';

/**
 * Dropdown options for the cover art format selector.
 * "raw" preserves the original format served by Apple Music's CDN.
 */
const COVER_FORMAT_OPTIONS = [
  { value: 'raw', label: 'Raw (original format from Apple Music)' },
  { value: 'jpg', label: 'JPEG (compressed, smaller file size)' },
  { value: 'png', label: 'PNG (lossless, larger file size)' },
];

/**
 * CoverArtTab -- Renders the Cover Art settings tab.
 *
 * Contains two visual sections:
 * 1. "Cover Art" -- Static cover art settings (toggle, format, size)
 * 2. "Animated Artwork" -- Motion cover art settings (toggle, MusicKit credentials)
 */
export function CoverArtTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting cover art setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  // --- Private key state (stored in OS keychain, not settings JSON) ---
  /** Whether the private key has been stored in the keychain */
  const [keyStored, setKeyStored] = useState(false);
  /** Transient input value for the private key textarea (never persisted to settings) */
  const [keyInput, setKeyInput] = useState('');
  /** Status message for the private key save operation */
  const [keyStatus, setKeyStatus] = useState('');

  // Check if a private key is already stored on mount and when the tab becomes visible.
  useEffect(() => {
    if (settings.animated_artwork_enabled) {
      getCredential('musickit_private_key').then((val) => {
        setKeyStored(val !== null);
      }).catch(() => {
        setKeyStored(false);
      });
    }
  }, [settings.animated_artwork_enabled]);

  // Save the private key to the OS keychain when the user clicks "Save to Keychain".
  const handleSaveKey = useCallback(async () => {
    if (!keyInput.trim()) {
      setKeyStatus('Please paste your .p8 private key content first');
      return;
    }
    try {
      await storeCredential('musickit_private_key', keyInput.trim());
      setKeyStored(true);
      setKeyInput(''); // Clear the input after successful save
      setKeyStatus('Private key saved securely to OS keychain');
    } catch (e) {
      setKeyStatus(`Failed to save key: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, [keyInput]);

  return (
    <div className="space-y-6 max-w-xl">
      {/* ============================================================ */}
      {/* Section 1: Static Cover Art (GAMDL) */}
      {/* ============================================================ */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Cover Art
        </h3>

        <div className="space-y-4">
          {/* Save cover art */}
          <Toggle
            label="Save Cover Art"
            description="Download and save album cover art as a separate file"
            checked={settings.save_cover}
            onChange={(checked) => updateSettings({ save_cover: checked })}
          />

          {/* Cover format (only shown when save_cover is enabled) */}
          {settings.save_cover && (
            <>
              <Select
                label="Cover Format"
                description="Image format for saved cover art files"
                options={COVER_FORMAT_OPTIONS}
                value={settings.cover_format}
                onChange={(e) =>
                  updateSettings({
                    cover_format: e.target.value as CoverFormat,
                  })
                }
              />

              {/* Cover size -- numeric input with client-side validation.
                  The onChange handler parses the string to an integer and
                  only persists the value if it falls within the valid range
                  (100-3000 pixels). This prevents invalid values from
                  reaching the backend while still allowing the user to
                  type freely. The `step={100}` prop controls the increment
                  when using the browser's native spinner arrows. */}
              <Input
                label="Cover Size (pixels)"
                description="Width and height of the cover art image (max 3000)"
                type="number"
                min={100}
                max={3000}
                step={100}
                value={settings.cover_size.toString()} /* Convert number to string for the input value */
                onChange={(e) => {
                  const size = parseInt(e.target.value, 10); // Parse the input string to a base-10 integer
                  if (!isNaN(size) && size >= 100 && size <= 3000) { // Validate within acceptable range
                    updateSettings({ cover_size: size }); // Only persist valid values
                  }
                }}
              />
            </>
          )}
        </div>
      </div>

      {/* ============================================================ */}
      {/* Section 2: Animated Artwork (Apple MusicKit API) */}
      {/* ============================================================ */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Animated Artwork
        </h3>

        <div className="space-y-4">
          {/* Master toggle for animated artwork downloading */}
          <Toggle
            label="Download Animated Cover Art"
            description="Download animated (motion) cover art from Apple Music when available. Saves FrontCover.mp4 and PortraitCover.mp4 alongside album files."
            checked={settings.animated_artwork_enabled}
            onChange={(checked) =>
              updateSettings({ animated_artwork_enabled: checked })
            }
          />

          {/* MusicKit credential inputs (only shown when enabled) */}
          {settings.animated_artwork_enabled && (
            <>
              {/* Informational note about requirements */}
              <p className="text-xs text-content-secondary">
                Requires an Apple Developer account (free) with a MusicKit key.
                See the{' '}
                <span className="text-accent-primary cursor-pointer">
                  Animated Artwork help page
                </span>
                {' '}for setup instructions.
              </p>

              {/* MusicKit Team ID -- stored in settings (non-sensitive) */}
              <Input
                label="MusicKit Team ID"
                description="Your 10-character Apple Developer Team ID (found at top-right of developer.apple.com)"
                value={settings.musickit_team_id ?? ''}
                onChange={(e) =>
                  updateSettings({
                    musickit_team_id: e.target.value || null,
                  })
                }
              />

              {/* MusicKit Key ID -- stored in settings (non-sensitive) */}
              <Input
                label="MusicKit Key ID"
                description="Your 10-character MusicKit key identifier (shown when you create the key)"
                value={settings.musickit_key_id ?? ''}
                onChange={(e) =>
                  updateSettings({
                    musickit_key_id: e.target.value || null,
                  })
                }
              />

              {/* MusicKit Private Key -- stored in OS keychain (sensitive) */}
              <div className="space-y-2">
                <label className="block text-sm font-medium text-content-primary">
                  MusicKit Private Key
                </label>
                <p className="text-xs text-content-secondary">
                  Paste the content of your .p8 private key file. This is stored
                  securely in your OS keychain, not in the settings file.
                </p>

                {/* Show key status indicator */}
                {keyStored && (
                  <p className="text-xs text-green-600 dark:text-green-400">
                    Private key is stored in OS keychain
                  </p>
                )}

                {/* Textarea for pasting the private key content */}
                <textarea
                  className="w-full h-24 px-3 py-2 text-xs font-mono rounded-md
                    border border-border-primary bg-surface-secondary
                    text-content-primary placeholder-content-tertiary
                    focus:outline-none focus:ring-2 focus:ring-accent-primary
                    resize-none"
                  placeholder="-----BEGIN PRIVATE KEY-----&#10;MIGHAgEAMBMGByqGSM49...&#10;-----END PRIVATE KEY-----"
                  value={keyInput}
                  onChange={(e) => {
                    setKeyInput(e.target.value);
                    setKeyStatus(''); // Clear status on new input
                  }}
                />

                {/* Save button and status message */}
                <div className="flex items-center gap-3">
                  <button
                    type="button"
                    className="px-3 py-1.5 text-xs font-medium rounded-md
                      bg-accent-primary text-white
                      hover:bg-accent-primary/90
                      disabled:opacity-50 disabled:cursor-not-allowed
                      transition-colors"
                    onClick={handleSaveKey}
                    disabled={!keyInput.trim()}
                  >
                    Save to Keychain
                  </button>
                  {keyStatus && (
                    <span className="text-xs text-content-secondary">
                      {keyStatus}
                    </span>
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
