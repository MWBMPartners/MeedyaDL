/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file SettingsPage.tsx -- Settings page container with 9-tab navigation.
 *
 * This is the top-level settings component rendered when the user navigates to
 * the "Settings" page via the application sidebar. It provides:
 *
 *   1. A **left sidebar** listing all 9 settings tabs with icons.
 *   2. A **content area** on the right that renders the active tab's component.
 *   3. A **header bar** with "Reset" and "Save Changes" action buttons.
 *
 * ## 9-Tab Layout
 *
 * The settings are organised into 9 categories, each implemented as a
 * separate React component inside the `./tabs/` directory:
 *
 * | Tab          | Component        | Purpose                                      |
 * |--------------|------------------|----------------------------------------------|
 * | General      | GeneralTab       | Output path, language, overwrite, auto-update |
 * | Quality      | QualityTab       | Audio codec, video resolution, remux format   |
 * | Fallback     | FallbackTab      | Drag-to-reorder fallback chains               |
 * | Paths        | PathsTab         | External tool binary paths                    |
 * | Cookies      | CookiesTab       | Cookie file import and validation             |
 * | Lyrics       | LyricsTab        | Synced lyrics format preferences              |
 * | Cover Art    | CoverArtTab      | Cover art saving, format, and size            |
 * | Templates    | TemplatesTab     | File/folder naming templates                  |
 * | Advanced     | AdvancedTab      | Download mode, remux mode, wrapper config     |
 *
 * ## Active Tab State
 *
 * The currently active tab is tracked by a local `useState('general')` hook.
 * Clicking a tab button in the sidebar calls `setActiveTab(id)`, which
 * triggers a re-render to display the corresponding tab component.
 *
 * ## Settings Save Flow
 *
 * 1. On mount, `loadSettings()` fetches settings from the Rust backend via
 *    Tauri IPC (see {@link @/stores/settingsStore}).
 * 2. The user modifies settings within any tab -- each tab calls
 *    `updateSettings(patch)` on the Zustand store, which sets `isDirty = true`.
 * 3. Clicking "Save Changes" calls `saveSettings()`, which persists the
 *    settings to disk via the Rust backend and resets `isDirty = false`.
 * 4. A toast notification is displayed via `useUiStore.addToast()` on success
 *    or failure.
 * 5. Clicking "Reset" calls `resetToDefaults()`, which restores all settings
 *    to their default values and sets `isDirty = true`.
 *
 * ## Store Connections
 *
 * - **settingsStore** (Zustand): Provides `loadSettings`, `saveSettings`,
 *   `resetToDefaults`, `isDirty`, and `isLoading`. All tab components also
 *   read from and write to this store.
 * - **uiStore** (Zustand): Provides `addToast` for displaying save/reset
 *   feedback notifications.
 *
 * @see {@link https://react.dev/reference/react/useState}  -- React useState hook
 * @see {@link https://react.dev/reference/react/useEffect} -- React useEffect hook
 * @see {@link https://zustand.docs.pmnd.rs/}               -- Zustand state management
 * @see {@link https://v2.tauri.app/}                        -- Tauri 2.0 framework
 * @see {@link https://lucide.dev/}                          -- Lucide icon library
 */

// React hooks: useState for active tab tracking, useEffect for loading settings on mount.
// @see https://react.dev/reference/react/useState
// @see https://react.dev/reference/react/useEffect
import { useEffect, useState } from 'react';

// Lucide icon components used for the tab sidebar and header action buttons.
// Each tab has a corresponding icon for visual identification.
// @see https://lucide.dev/icons/
import {
  Settings as SettingsIcon, // General tab icon (aliased to avoid clash with component name)
  Music,                    // Quality tab icon
  ArrowDownUp,              // Fallback tab icon (up/down arrows representing reordering)
  FolderOpen,               // Paths tab icon
  Cookie,                   // Cookies tab icon
  FileText,                 // Lyrics tab icon
  Image,                    // Cover Art tab icon
  Code,                     // Templates tab icon (code brackets for template syntax)
  Wrench,                   // Advanced tab icon
  Save,                     // Save button icon
  RotateCcw,                // Reset button icon (counter-clockwise rotation)
} from 'lucide-react';

// Zustand stores: settingsStore holds the application settings state;
// uiStore provides toast notification capabilities.
// @see https://zustand.docs.pmnd.rs/getting-started/introduction
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';

// Shared UI components used in the header action bar.
import { Button } from '@/components/common';
import { PageHeader } from '@/components/layout';

// Individual tab components -- each renders its own section of settings.
// They are imported directly here (not re-exported) because they are
// internal to the settings module.
import { GeneralTab } from './tabs/GeneralTab';
import { QualityTab } from './tabs/QualityTab';
import { FallbackTab } from './tabs/FallbackTab';
import { PathsTab } from './tabs/PathsTab';
import { CookiesTab } from './tabs/CookiesTab';
import { LyricsTab } from './tabs/LyricsTab';
import { CoverArtTab } from './tabs/CoverArtTab';
import { TemplatesTab } from './tabs/TemplatesTab';
import { AdvancedTab } from './tabs/AdvancedTab';

/**
 * Shape of a single tab entry in the TABS configuration array.
 *
 * @property id        - Unique identifier used as the key for `activeTab` state
 *                       and the React `key` prop in the sidebar list.
 * @property label     - Human-readable label displayed in the sidebar button.
 * @property icon      - Lucide icon component rendered next to the label.
 *                       Typed as `typeof SettingsIcon` (all Lucide icons share
 *                       the same component signature).
 * @property component - The React functional component to render when this tab
 *                       is active. Receives no props; it reads state directly
 *                       from the settingsStore via Zustand selectors.
 */
interface SettingsTab {
  id: string;
  label: string;
  icon: typeof SettingsIcon;
  component: React.FC;
}

/**
 * Static configuration array defining all 9 settings tabs in the order they
 * appear in the sidebar. The order here directly controls the visual layout.
 *
 * Each entry maps a tab ID to its display properties (label, icon) and the
 * component that renders its settings form. All tab components follow the
 * same pattern: they read from `useSettingsStore` and call `updateSettings`
 * to mutate the shared settings state.
 */
const TABS: SettingsTab[] = [
  { id: 'general', label: 'General', icon: SettingsIcon, component: GeneralTab },
  { id: 'quality', label: 'Quality', icon: Music, component: QualityTab },
  { id: 'fallback', label: 'Fallback', icon: ArrowDownUp, component: FallbackTab },
  { id: 'paths', label: 'Paths', icon: FolderOpen, component: PathsTab },
  { id: 'cookies', label: 'Cookies', icon: Cookie, component: CookiesTab },
  { id: 'lyrics', label: 'Lyrics', icon: FileText, component: LyricsTab },
  { id: 'cover-art', label: 'Cover Art', icon: Image, component: CoverArtTab },
  { id: 'templates', label: 'Templates', icon: Code, component: TemplatesTab },
  { id: 'advanced', label: 'Advanced', icon: Wrench, component: AdvancedTab },
];

/**
 * SettingsPage -- Top-level settings page component.
 *
 * Renders the full settings page with a vertical tab sidebar on the left
 * and a scrollable content area on the right. The header includes "Reset"
 * and "Save Changes" action buttons that operate on the global settings
 * state managed by {@link useSettingsStore}.
 *
 * **Rendering flow:**
 * 1. On mount, `loadSettings()` is called to hydrate the Zustand store
 *    from the Rust backend (Tauri IPC command `load_settings`).
 * 2. The sidebar renders one button per entry in the TABS array.
 * 3. Clicking a tab updates `activeTab`, which selects the corresponding
 *    component to render in the content area.
 * 4. The "Save Changes" button is disabled when `isDirty` is false
 *    (no unsaved changes) or `isLoading` is true (save in progress).
 *
 * @see {@link https://react.dev/reference/react/useState}  -- active tab state
 * @see {@link https://react.dev/reference/react/useEffect} -- settings load on mount
 */
export function SettingsPage() {
  /**
   * Tracks which settings tab is currently active (displayed in the content area).
   * Defaults to 'general' so the General tab is shown on first render.
   * @see {@link https://react.dev/reference/react/useState}
   */
  const [activeTab, setActiveTab] = useState('general');

  // --- Zustand store selectors ---
  // Each selector extracts a single slice of state to minimize re-renders.
  // @see https://zustand.docs.pmnd.rs/guides/prevent-rerenders-with-use-shallow

  /** Fetches settings from the Rust backend and populates the store */
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  /** Persists the current in-memory settings to disk via Tauri IPC */
  const saveSettings = useSettingsStore((s) => s.saveSettings);
  /** Restores all settings to their compiled-in defaults */
  const resetToDefaults = useSettingsStore((s) => s.resetToDefaults);
  /** True when the user has modified settings but not yet saved */
  const isDirty = useSettingsStore((s) => s.isDirty);
  /** True while a load or save operation is in-flight */
  const isLoading = useSettingsStore((s) => s.isLoading);
  /** Pushes a toast notification to the UI toast queue */
  const addToast = useUiStore((s) => s.addToast);

  /**
   * Load settings from the backend on component mount.
   * This ensures the settings UI always reflects the latest persisted state.
   * The dependency array contains only `loadSettings` (a stable function
   * reference from Zustand), so this effect runs exactly once.
   */
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  /**
   * Persists settings to disk by calling the Zustand store's `saveSettings`
   * action, which in turn invokes the Tauri IPC command `save_settings`.
   * Displays a success or error toast notification based on the outcome.
   */
  const handleSave = async () => {
    try {
      await saveSettings();
      addToast('Settings saved successfully', 'success');
    } catch {
      addToast('Failed to save settings', 'error');
    }
  };

  /**
   * Resets all settings to their compiled-in defaults by calling the
   * Zustand store's `resetToDefaults` action. Note that this only updates
   * the in-memory state (sets `isDirty = true`) -- the user must still
   * click "Save Changes" to persist the defaults to disk.
   */
  const handleReset = () => {
    resetToDefaults();
    addToast('Settings reset to defaults', 'info');
  };

  /**
   * Resolve the active tab's React component from the TABS configuration.
   * Uses Array.find to locate the matching tab by ID, then extracts its
   * `component` property. Falls back to GeneralTab if the activeTab ID
   * does not match any entry (defensive guard, should not happen in practice).
   */
  const ActiveComponent =
    TABS.find((t) => t.id === activeTab)?.component || GeneralTab;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Settings"
        subtitle="Configure download options, paths, and preferences"
        actions={
          <div className="flex gap-2">
            {/* Reset to defaults */}
            <Button
              variant="ghost"
              size="sm"
              icon={<RotateCcw size={14} />}
              onClick={handleReset}
            >
              Reset
            </Button>

            {/* Save button (shows unsaved indicator) */}
            <Button
              variant="primary"
              size="sm"
              icon={<Save size={14} />}
              onClick={handleSave}
              disabled={!isDirty || isLoading}
            >
              {isDirty ? 'Save Changes' : 'Saved'}
            </Button>
          </div>
        }
      />

      {/* ================================================================
          Tab navigation (sidebar) + content area
          Uses a horizontal flex layout: fixed-width nav on the left,
          flex-1 content on the right. `overflow-hidden` on the parent
          prevents double scrollbars; each child manages its own scroll.
          ================================================================ */}
      <div className="flex flex-1 overflow-hidden">
        {/* ----------------------------------------------------------------
            Tab sidebar (left column)
            Fixed width of w-44 (176px). Lists all 9 tabs as buttons.
            The active tab receives an accent background and font weight.
            The sidebar scrolls independently if the window is too short
            to display all tabs (unlikely at 9 items, but defensive).
            ---------------------------------------------------------------- */}
        <nav className="w-44 flex-shrink-0 border-r border-border-light overflow-y-auto p-2 space-y-0.5">
          {TABS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveTab(id)}
              className={`
                w-full flex items-center gap-2.5 px-3 py-2
                rounded-platform text-sm transition-colors
                ${
                  activeTab === id
                    ? 'bg-accent-light text-accent font-medium'   /* Active tab styling */
                    : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary' /* Inactive tab styling */
                }
              `}
            >
              {/* Tab icon -- flex-shrink-0 prevents it from collapsing */}
              <Icon size={16} className="flex-shrink-0" />
              {/* Tab label text */}
              {label}
            </button>
          ))}
        </nav>

        {/* ----------------------------------------------------------------
            Tab content area (right column)
            Renders the currently active tab's React component via dynamic
            component selection. The content scrolls vertically and has
            p-6 padding for visual breathing room.
            ---------------------------------------------------------------- */}
        <div className="flex-1 overflow-y-auto p-6">
          <ActiveComponent />
        </div>
      </div>
    </div>
  );
}
