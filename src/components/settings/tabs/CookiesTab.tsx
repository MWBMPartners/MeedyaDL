/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Cookies settings tab.
 * Manages the cookies file used for Apple Music authentication.
 * Supports browsing for a Netscape-format cookies file, validating it,
 * and displaying validation results.
 */

import { useState } from 'react';
import { Shield, AlertTriangle, CheckCircle } from 'lucide-react';
import { useSettingsStore } from '@/stores/settingsStore';
import * as commands from '@/lib/tauri-commands';
import { FilePickerButton, Button } from '@/components/common';
import type { CookieValidation } from '@/types';

/**
 * Renders the Cookies settings tab with a cookie file picker,
 * validation button, and validation results display.
 */
export function CookiesTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const [validation, setValidation] = useState<CookieValidation | null>(null);
  const [isValidating, setIsValidating] = useState(false);

  /** Validate the selected cookies file */
  const handleValidate = async () => {
    if (!settings.cookies_path) return;

    setIsValidating(true);
    try {
      const result = await commands.validateCookiesFile(settings.cookies_path);
      setValidation(result);
    } catch {
      setValidation(null);
    }
    setIsValidating(false);
  };

  return (
    <div className="space-y-6 max-w-xl">
      {/* Instructions */}
      <div className="p-4 rounded-platform border border-border-light bg-surface-elevated">
        <div className="flex items-start gap-3">
          <Shield size={18} className="text-accent flex-shrink-0 mt-0.5" />
          <div className="text-sm text-content-secondary space-y-2">
            <p>
              GAMDL requires Apple Music cookies for authentication. You need a
              Netscape-format cookies file exported from your browser.
            </p>
            <p>
              Use a browser extension like{' '}
              <span className="font-medium text-content-primary">
                cookies.txt
              </span>{' '}
              to export cookies from{' '}
              <span className="font-medium text-content-primary">
                music.apple.com
              </span>{' '}
              after logging in.
            </p>
          </div>
        </div>
      </div>

      {/* Cookie file picker */}
      <FilePickerButton
        label="Cookies File"
        description="Path to the Netscape-format cookies.txt file"
        value={settings.cookies_path}
        onChange={(path) => {
          updateSettings({ cookies_path: path });
          setValidation(null);
        }}
        placeholder="No cookies file selected"
        filters={[{ name: 'Text Files', extensions: ['txt'] }]}
      />

      {/* Validate button */}
      {settings.cookies_path && (
        <Button
          variant="secondary"
          size="sm"
          loading={isValidating}
          onClick={handleValidate}
        >
          Validate Cookies
        </Button>
      )}

      {/* Validation results */}
      {validation && (
        <div
          className={`p-4 rounded-platform border ${
            validation.valid
              ? 'border-status-success bg-green-50 dark:bg-green-950'
              : 'border-status-error bg-red-50 dark:bg-red-950'
          }`}
        >
          {/* Header */}
          <div className="flex items-center gap-2 mb-2">
            {validation.valid ? (
              <CheckCircle size={16} className="text-status-success" />
            ) : (
              <AlertTriangle size={16} className="text-status-error" />
            )}
            <span className="text-sm font-medium text-content-primary">
              {validation.valid ? 'Cookies Valid' : 'Cookies Invalid'}
            </span>
          </div>

          {/* Details */}
          <div className="text-xs text-content-secondary space-y-1 ml-6">
            <p>{validation.cookie_count} total cookies found</p>
            <p>{validation.apple_music_cookies} Apple Music cookies</p>
            {validation.expired && (
              <p className="text-status-warning">
                Warning: Some cookies may be expired
              </p>
            )}
            {validation.warnings.map((warning, i) => (
              <p key={i} className="text-status-warning">
                {warning}
              </p>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
