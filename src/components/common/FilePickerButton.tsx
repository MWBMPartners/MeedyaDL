// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file File / directory picker button component.
 *
 * Provides a compound control consisting of:
 * 1. A read-only display showing the currently selected file or directory path.
 * 2. A "Browse" button that opens the native OS file/directory picker dialog
 *    via the Tauri dialog plugin (`@tauri-apps/plugin-dialog`).
 *
 * The Tauri dialog plugin is dynamically imported at call time so that:
 * - The module is only loaded when the user actually clicks "Browse".
 * - The component does not crash when rendered outside the Tauri runtime
 *   (e.g. during Storybook / unit testing in a browser).
 *
 * **Usage across the application:**
 * - CookiesStep (setup wizard): selecting the cookies file path.
 * - CookiesTab (settings): updating the cookies file path.
 * - GeneralTab (settings): selecting the default output directory.
 * - PathsTab (settings): configuring various tool / binary paths.
 *
 * @see https://v2.tauri.app/plugin/dialog/
 *      Tauri v2 dialog plugin -- native open/save dialogs.
 * @see https://lucide.dev/icons/folder-open -- FolderOpen icon (directory mode)
 * @see https://lucide.dev/icons/file -- File icon (file mode)
 */

/**
 * Lucide icons for the path display:
 * - FolderOpen: shown when `directory` mode is active.
 * - File: shown when picking a single file.
 * @see https://lucide.dev/guide/packages/lucide-react
 */
import { FolderOpen, File } from 'lucide-react';

/** Internal reuse of the common Button component for the "Browse" action */
import { Button } from './Button';

/**
 * Props accepted by the {@link FilePickerButton} component.
 */
interface FilePickerButtonProps {
  /**
   * The currently selected file or directory path, or null if nothing
   * has been selected. Displayed in the read-only path area.
   */
  value: string | null;

  /**
   * Callback invoked when the user selects a path in the native dialog.
   * Receives the absolute path string, or null if the dialog is cancelled.
   */
  onChange: (path: string | null) => void;

  /** Label text rendered above the picker (same style as Input/Select labels) */
  label?: string;

  /** Small helper text rendered below the picker in muted colour */
  description?: string;

  /**
   * When true, the native dialog opens a **directory** picker instead of
   * a file picker. The display icon also switches from File to FolderOpen.
   * Defaults to false (file mode).
   */
  directory?: boolean;

  /**
   * Optional array of file type filters passed to the Tauri dialog.
   * Each entry has a human-readable `name` and an array of file `extensions`
   * (without dots). Example:
   * ```ts
   * [{ name: 'Cookie Files', extensions: ['txt', 'json'] }]
   * ```
   * @see https://v2.tauri.app/plugin/dialog/#filters -- Tauri dialog filters
   */
  filters?: Array<{ name: string; extensions: string[] }>;

  /** Placeholder text shown when no path is selected (default: 'No file selected') */
  placeholder?: string;

  /** When true, the Browse button is disabled and the picker cannot be opened */
  disabled?: boolean;
}

/**
 * Renders a file path display with a "Browse" button that opens the native
 * OS file/directory picker dialog via the Tauri dialog plugin.
 *
 * The path display area is read-only -- it shows the currently selected
 * path (or placeholder text) alongside a contextual icon (file or folder).
 *
 * @example
 * ```tsx
 * <FilePickerButton
 *   label="Output Directory"
 *   directory
 *   value={outputDir}
 *   onChange={setOutputDir}
 *   description="Where downloaded tracks will be saved"
 * />
 * ```
 *
 * @param value       - Current path (or null if none selected)
 * @param onChange     - Callback with the selected path (or null on cancel)
 * @param label       - Optional label text above the control
 * @param description - Optional helper text below the control
 * @param directory   - Pick directories instead of files (default: false)
 * @param filters     - File type filters for the native dialog
 * @param placeholder - Text shown when value is null (default: 'No file selected')
 * @param disabled    - Disable the Browse button
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
   * Opens the native OS file/directory picker dialog.
   *
   * **Dynamic import:** The `@tauri-apps/plugin-dialog` module is imported
   * at invocation time (not at the top of the file) so that:
   *   1. The Tauri runtime is only required when the user actually clicks Browse.
   *   2. If the app is running outside Tauri (e.g. unit tests), the import
   *      fails gracefully and the catch block silently ignores the error.
   *
   * The `open()` function returns:
   *   - A string path when the user selects a file/directory.
   *   - null when the user cancels the dialog.
   *   - An array of paths when `multiple: true` (not used here).
   *
   * @see https://v2.tauri.app/plugin/dialog/#open -- Tauri open dialog API
   */
  const handleBrowse = async () => {
    try {
      /* Dynamically import the Tauri dialog plugin */
      const { open } = await import('@tauri-apps/plugin-dialog');

      /* Open the native dialog with the configured options */
      const selected = await open({
        directory,       // true = directory picker, false = file picker
        multiple: false, // single selection only
        filters: filters,
        title: directory ? 'Select Directory' : 'Select File',
      });

      /*
       * The dialog returns a string path when a selection is made.
       * We only call onChange when the result is a string (not null / array).
       */
      if (typeof selected === 'string') {
        onChange(selected);
      }
    } catch {
      /*
       * This catch handles two cases:
       * 1. The user cancelled the dialog (some platforms throw on cancel).
       * 2. The Tauri API is unavailable (running outside the desktop app).
       * In both cases we silently ignore the error -- no toast or alert.
       */
    }
  };

  return (
    /* Outer wrapper -- space-y-1.5 for consistent vertical spacing */
    <div className="space-y-1.5">
      {/* Label -- same styling as Input and Select labels for consistency */}
      {label && (
        <label className="block text-sm font-medium text-content-primary">
          {label}
        </label>
      )}

      {/*
       * Horizontal layout: path display area (flex-1) + Browse button.
       * gap-2 adds 8px spacing between the two elements.
       */}
      <div className="flex gap-2">
        {/*
         * Read-only path display area.
         * - Styled to visually match the Input component (same border,
         *   background, padding, and border-radius).
         * - min-w-0 allows the flex item to shrink below its intrinsic
         *   content width, enabling the `truncate` class on the <span>
         *   to ellipsis-truncate long paths.
         * - text colour switches between tertiary (placeholder) and
         *   primary (actual path) depending on whether value is set.
         */}
        <div
          className={`
            flex-1 flex items-center gap-2 px-3 py-2 text-sm
            rounded-platform border border-border
            bg-surface-secondary min-w-0
            ${!value ? 'text-content-tertiary' : 'text-content-primary'}
          `}
        >
          {/* Contextual icon -- folder for directories, file for files */}
          {directory ? (
            <FolderOpen size={16} className="flex-shrink-0 text-content-tertiary" />
          ) : (
            <File size={16} className="flex-shrink-0 text-content-tertiary" />
          )}
          {/* Path text -- truncated with ellipsis if it overflows */}
          <span className="truncate">{value || placeholder}</span>
        </div>

        {/*
         * Browse button -- uses the common Button component in secondary
         * variant so it visually complements rather than competes with
         * the primary CTA elsewhere on the page.
         */}
        <Button
          variant="secondary"
          size="md"
          onClick={handleBrowse}
          disabled={disabled}
        >
          Browse
        </Button>
      </div>

      {/* Helper description text below the control */}
      {description && (
        <p className="text-xs text-content-tertiary">{description}</p>
      )}
    </div>
  );
}
