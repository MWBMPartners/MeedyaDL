/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Cookies import step of the setup wizard.
 * Guides the user through importing their Apple Music cookies for
 * authentication. Provides file picker, validation, and instructions.
 */

import { useState, useEffect } from 'react';
import {
  Shield,
  CheckCircle,
  AlertTriangle,
} from 'lucide-react';
import { useSettingsStore } from '@/stores/settingsStore';
import { useSetupStore } from '@/stores/setupStore';
import * as commands from '@/lib/tauri-commands';
import { FilePickerButton, Button } from '@/components/common';
import type { CookieValidation } from '@/types';

/**
 * Renders the cookie import step with instructions, file picker,
 * validation, and skip option.
 */
export function CookiesStep() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const completeStep = useSetupStore((s) => s.completeStep);
  const [validation, setValidation] = useState<CookieValidation | null>(null);
  const [isValidating, setIsValidating] = useState(false);

  /* Auto-complete if cookies are already configured and valid */
  useEffect(() => {
    if (validation?.valid) {
      completeStep('cookies');
    }
  }, [validation, completeStep]);

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

  /** Skip cookie import (can be set up later in Settings) */
  const handleSkip = () => {
    completeStep('cookies');
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          Apple Music Cookies
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL needs your Apple Music cookies to authenticate downloads. You
          can skip this step and configure cookies later in Settings.
        </p>
      </div>

      {/* Instructions card */}
      <div className="p-4 rounded-platform-lg border border-border-light bg-surface-elevated space-y-3">
        <div className="flex items-start gap-3">
          <Shield size={18} className="text-accent flex-shrink-0 mt-0.5" />
          <div className="text-sm text-content-secondary space-y-2">
            <p className="font-medium text-content-primary">
              How to export cookies:
            </p>
            <ol className="list-decimal list-inside space-y-1.5 ml-1">
              <li>
                Install the{' '}
                <span className="font-medium text-content-primary">
                  cookies.txt
                </span>{' '}
                browser extension
              </li>
              <li>
                Go to{' '}
                <span className="font-mono text-xs text-accent">
                  music.apple.com
                </span>{' '}
                and log in to your account
              </li>
              <li>
                Click the cookies.txt extension icon and export cookies
              </li>
              <li>Save the file and select it below</li>
            </ol>
          </div>
        </div>
      </div>

      {/* Cookie file picker */}
      <FilePickerButton
        label="Cookies File"
        description="Select your exported cookies.txt file"
        value={settings.cookies_path}
        onChange={(path) => {
          updateSettings({ cookies_path: path });
          setValidation(null);
        }}
        placeholder="No cookies file selected"
        filters={[{ name: 'Text Files', extensions: ['txt'] }]}
      />

      {/* Validate and Skip buttons */}
      <div className="flex gap-3">
        {settings.cookies_path && (
          <Button
            variant="primary"
            loading={isValidating}
            onClick={handleValidate}
          >
            Validate Cookies
          </Button>
        )}
        <Button variant="ghost" onClick={handleSkip}>
          Skip for Now
        </Button>
      </div>

      {/* Validation results */}
      {validation && (
        <div
          className={`p-4 rounded-platform border ${
            validation.valid
              ? 'border-status-success bg-green-50 dark:bg-green-950'
              : 'border-status-error bg-red-50 dark:bg-red-950'
          }`}
        >
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
          <div className="text-xs text-content-secondary space-y-1 ml-6">
            <p>{validation.apple_music_cookies} Apple Music cookies found</p>
            {validation.warnings.map((w, i) => (
              <p key={i} className="text-status-warning">{w}</p>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
