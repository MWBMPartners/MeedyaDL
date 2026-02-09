/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * File picker button component.
 * Uses Tauri's native dialog plugin to open a file or directory picker.
 * Displays the selected path in a text input with a "Browse" button.
 */

import { FolderOpen, File } from 'lucide-react';
import { Button } from './Button';

interface FilePickerButtonProps {
  /** Currently selected file/directory path */
  value: string | null;
  /** Callback when a file/directory is selected */
  onChange: (path: string | null) => void;
  /** Label text above the picker */
  label?: string;
  /** Description text below the picker */
  description?: string;
  /** Whether to pick a directory instead of a file (default: false) */
  directory?: boolean;
  /** File type filters (e.g., [{ name: 'Text Files', extensions: ['txt'] }]) */
  filters?: Array<{ name: string; extensions: string[] }>;
  /** Placeholder text when no path is selected */
  placeholder?: string;
  /** Whether the picker is disabled */
  disabled?: boolean;
}

/**
 * Renders a file path input with a "Browse" button that opens
 * the native OS file/directory picker dialog via Tauri.
 *
 * @param value - Current file/directory path (or null)
 * @param onChange - Called with the selected path (or null if cancelled)
 * @param directory - Whether to pick directories instead of files
 * @param filters - Optional file type filters for the dialog
 */
export function FilePickerButton({
  value,
  onChange,
  label,
  description,
  directory = false,
  filters,
  placeholder = 'No file selected',
  disabled = false,
}: FilePickerButtonProps) {
  /**
   * Opens the native file/directory picker dialog.
   * Uses dynamic import to avoid errors when running outside Tauri.
   */
  const handleBrowse = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');

      const selected = await open({
        directory,
        multiple: false,
        filters: filters,
        title: directory ? 'Select Directory' : 'Select File',
      });

      /* The dialog returns a string path or null if cancelled */
      if (typeof selected === 'string') {
        onChange(selected);
      }
    } catch {
      /* Dialog was cancelled or Tauri API unavailable - silently ignore */
    }
  };

  return (
    <div className="space-y-1.5">
      {/* Label */}
      {label && (
        <label className="block text-sm font-medium text-content-primary">
          {label}
        </label>
      )}

      {/* Path display + Browse button */}
      <div className="flex gap-2">
        {/* Path text (read-only display) */}
        <div
          className={`
            flex-1 flex items-center gap-2 px-3 py-2 text-sm
            rounded-platform border border-border
            bg-surface-secondary min-w-0
            ${!value ? 'text-content-tertiary' : 'text-content-primary'}
          `}
        >
          {directory ? (
            <FolderOpen size={16} className="flex-shrink-0 text-content-tertiary" />
          ) : (
            <File size={16} className="flex-shrink-0 text-content-tertiary" />
          )}
          <span className="truncate">{value || placeholder}</span>
        </div>

        {/* Browse button */}
        <Button
          variant="secondary"
          size="md"
          onClick={handleBrowse}
          disabled={disabled}
        >
          Browse
        </Button>
      </div>

      {/* Description text */}
      {description && (
        <p className="text-xs text-content-tertiary">{description}</p>
      )}
    </div>
  );
}
