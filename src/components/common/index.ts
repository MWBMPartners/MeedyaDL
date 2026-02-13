// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Barrel export file for the common UI component library.
 *
 * This module re-exports every shared/common component so that consuming code
 * can import from a single path rather than reaching into individual files:
 *
 *   import { Button, Input, Select } from '@/components/common';
 *
 * This pattern keeps import statements short and decouples consumers from the
 * internal file structure of the common/ directory. If a component is renamed
 * or reorganised, only this barrel file needs to change.
 *
 * @see https://react.dev/learn/importing-and-exporting-components
 *      React documentation on importing / exporting components.
 *
 * **Adding a new component:**
 * 1. Create the component file in src/components/common/.
 * 2. Add a named export here (keep alphabetical order).
 * 3. Re-export any public TypeScript types the component exposes.
 */

/* -------------------------------------------------------------------------- */
/*  Interactive form controls                                                  */
/* -------------------------------------------------------------------------- */

/**
 * Platform-adaptive button with primary/secondary/ghost/danger variants.
 * Used in: DownloadForm, DownloadQueue, SettingsPage, SetupWizard steps,
 *          UpdateBanner, FilePickerButton (internal), CookiesTab, FallbackTab.
 */
export { Button } from './Button';

/**
 * Text input with label, description, error state, and optional icon/suffix.
 * Used in: AdvancedTab, QualityTab, CoverArtTab, TemplatesTab.
 */
export { Input } from './Input';

/**
 * Native <select> dropdown with label, description, and error state.
 * Used in: DownloadForm, AdvancedTab, GeneralTab, QualityTab, CoverArtTab,
 *          LyricsTab.
 */
export { Select } from './Select';

/**
 * TypeScript interface for individual select options ({ value, label, disabled? }).
 * Re-exported as a type so consumers can strongly type their option arrays.
 */
export type { SelectOption } from './Select';

/**
 * Boolean on/off toggle switch (iOS / macOS style).
 * Used in: AdvancedTab, GeneralTab, QualityTab, CoverArtTab, LyricsTab.
 */
export { Toggle } from './Toggle';

/* -------------------------------------------------------------------------- */
/*  Overlay / dialog components                                                */
/* -------------------------------------------------------------------------- */

/**
 * Centered modal dialog with backdrop, Escape-to-close, and click-outside dismiss.
 * Available for any feature that requires a dialog overlay.
 */
export { Modal } from './Modal';

/* -------------------------------------------------------------------------- */
/*  Feedback / status components                                               */
/* -------------------------------------------------------------------------- */

/**
 * Fixed-position toast notification stack (top-right corner).
 * Rendered once in MainLayout; individual toasts are managed via useUiStore.
 */
export { ToastContainer } from './ToastContainer';

/**
 * Circular spinning indicator for async/loading states.
 * Used in: GamdlStep, PythonStep, DependenciesStep, and the App root loading screen.
 */
export { LoadingSpinner } from './LoadingSpinner';

/**
 * Hover/focus tooltip that appears after a short delay.
 * Used in: Sidebar navigation, CookiesTab.
 */
export { Tooltip } from './Tooltip';

/* -------------------------------------------------------------------------- */
/*  Tauri-specific / native integration components                             */
/* -------------------------------------------------------------------------- */

/**
 * File/directory picker that opens the native OS dialog via the Tauri dialog plugin.
 * Used in: CookiesStep (setup wizard), CookiesTab, GeneralTab, PathsTab.
 * @see https://v2.tauri.app/plugin/dialog/
 */
export { FilePickerButton } from './FilePickerButton';

/* -------------------------------------------------------------------------- */
/*  Progress / update components                                               */
/* -------------------------------------------------------------------------- */

/**
 * Horizontal progress bar supporting determinate (0-100%) and indeterminate modes.
 * Used in: QueueItem to show per-track download progress.
 */
export { ProgressBar } from './ProgressBar';

/**
 * Dismissible banner displayed when component updates (GAMDL, app, Python) are available.
 * Rendered in App.tsx above the main content area.
 */
export { UpdateBanner } from './UpdateBanner';
