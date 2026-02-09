/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Custom title bar component (Windows/Linux only).
 * On macOS, the native title bar is used with traffic-light buttons
 * and titleBarStyle: 'overlay' in tauri.conf.json, so this component
 * only renders on Windows and Linux where we need custom controls.
 *
 * Provides window drag region and minimize/maximize/close buttons.
 */

import { Minus, Square, X } from 'lucide-react';
import { usePlatform } from '@/hooks/usePlatform';

/**
 * Renders a custom window title bar with minimize, maximize, and close buttons.
 * Only visible on Windows and Linux; macOS uses native decorations.
 */
export function TitleBar() {
  const { isMacOS } = usePlatform();

  /* macOS uses native title bar with traffic-light buttons - don't render */
  if (isMacOS) return null;

  /**
   * Window control handlers using Tauri's window API.
   * Uses dynamic import to avoid errors when running outside Tauri.
   */
  const handleMinimize = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().minimize();
    } catch {
      /* Tauri API unavailable - silently ignore */
    }
  };

  const handleMaximize = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* Tauri API unavailable */
    }
  };

  const handleClose = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().close();
    } catch {
      /* Tauri API unavailable */
    }
  };

  return (
    <div className="drag-region flex items-center justify-end h-8 bg-surface-primary border-b border-border-light">
      {/* Window control buttons */}
      <div className="no-drag flex items-center">
        {/* Minimize */}
        <button
          onClick={handleMinimize}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-surface-secondary transition-colors"
          aria-label="Minimize"
        >
          <Minus size={14} />
        </button>

        {/* Maximize/Restore */}
        <button
          onClick={handleMaximize}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-surface-secondary transition-colors"
          aria-label="Maximize"
        >
          <Square size={12} />
        </button>

        {/* Close */}
        <button
          onClick={handleClose}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-status-error hover:text-white transition-colors"
          aria-label="Close"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
