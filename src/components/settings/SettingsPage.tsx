/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file SettingsPage.tsx -- Settings page container with 10-tab grouped navigation.
 *
 * This is the top-level settings component rendered when the user navigates to
 * the "Settings" page via the application sidebar. It provides:
 *
 *   1. A **left sidebar** listing all 10 settings tabs grouped under 4
 *      section headers (General, Download, Authentication, System).
 *   2. A **content area** on the right that renders the active tab's component.
 *   3. A **header bar** with "Reset" and "Save Changes" action buttons.
 *
 * ## 10-Tab Layout
 *
 * The settings are organised into 10 categories, each implemented as a
 * separate React component inside the `./tabs/` directory:
 *
 * | Tab          | Component        | Purpose                                      |
 * |--------------|------------------|----------------------------------------------|
 * | General      | GeneralTab       | Output path, language, overwrite, auto-update |
 * | Quality      | QualityTab       | Audio codec, video resolution, remux format   |
 * | Fallback     | FallbackTab      | Drag-to-reorder fallback chains               |
 * | Tools        | ToolsTab         | Tool status, install, path overrides          |
 * | Cookies      | CookiesTab       | Cookie file import and validation             |
 * | Lyrics       | LyricsTab        | Synced lyrics format preferences              |
 * | Cover Art    | CoverArtTab      | Cover art saving, format, and size            |
 * | Metadata     | MetadataTab      | AcoustID, ReplayGain, enrichment settings   |
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
import { useEffect, useRef, useState } from 'react';

// Lucide icon components used for the tab sidebar and header action buttons.
// Each tab has a corresponding icon for visual identification.
// @see https://lucide.dev/icons/
import {
  Settings as SettingsIcon, // General tab icon (aliased to avoid clash with component name)
  Music, // Quality tab icon
  ArrowDownUp, // Fallback tab icon (up/down arrows representing reordering)
  Package, // Tools tab icon
  Cookie, // Cookies tab icon
  FileText, // Lyrics tab icon
  Image, // Cover Art tab icon
  Code, // Templates tab icon (code brackets for template syntax)
  Wrench, // Advanced tab icon
  Tag, // Metadata tab icon
  Save, // Save button icon
  RotateCcw, // Reset button icon (counter-clockwise rotation)
} from 'lucide-react';

// Zustand stores: settingsStore holds the application settings state;
// uiStore provides toast notification capabilities.
// @see https://zustand.docs.pmnd.rs/getting-started/introduction
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { withErrorToast } from '@/lib/withErrorToast';

// Shared UI components used in the header action bar.
import { Button, Modal } from '@/components/common';
import { PageHeader } from '@/components/layout';

// Individual tab components -- each renders its own section of settings.
// They are imported directly here (not re-exported) because they are
// internal to the settings module.
import { GeneralTab } from './tabs/GeneralTab';
import { QualityTab } from './tabs/QualityTab';
import { FallbackTab } from './tabs/FallbackTab';
import { ToolsTab } from './tabs/ToolsTab';
import { CookiesTab } from './tabs/CookiesTab';
import { LyricsTab } from './tabs/LyricsTab';
import { CoverArtTab } from './tabs/CoverArtTab';
import { TemplatesTab } from './tabs/TemplatesTab';
import { AdvancedTab } from './tabs/AdvancedTab';
import { MetadataTab } from './tabs/MetadataTab';

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
 * Static configuration array defining all 10 settings tabs in the order they
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
  { id: 'tools', label: 'Tools', icon: Package, component: ToolsTab },
  { id: 'cookies', label: 'Cookies', icon: Cookie, component: CookiesTab },
  { id: 'lyrics', label: 'Lyrics', icon: FileText, component: LyricsTab },
  { id: 'cover-art', label: 'Cover Art', icon: Image, component: CoverArtTab },
  { id: 'metadata', label: 'Metadata', icon: Tag, component: MetadataTab },
  { id: 'templates', label: 'Templates', icon: Code, component: TemplatesTab },
  { id: 'advanced', label: 'Advanced', icon: Wrench, component: AdvancedTab },
];

/**
 * Groups settings tabs into logical sections for the sidebar navigation.
 * Each group has a label (rendered as a section header) and a list of tab IDs
 * that belong to that group. The tab IDs must match entries in the TABS array.
 *
 * Groups are rendered as static (non-collapsible) sections with visually
 * distinct headers -- with only 4 groups, collapsible behaviour would add
 * UI complexity without meaningful benefit.
 */
const SETTINGS_GROUPS: { label: string; tabs: string[] }[] = [
  { label: 'General', tabs: ['general'] },
  { label: 'Download', tabs: ['quality', 'fallback', 'lyrics', 'cover-art', 'metadata', 'templates'] },
  { label: 'Authentication', tabs: ['cookies'] },
  { label: 'System', tabs: ['tools', 'advanced'] },
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

  // ---------------------------------------------------------------
  // Restart-required tracking for template edits (#833)
  // ---------------------------------------------------------------
  //
  // GAMDL reads its template config (`album_folder_template`, file
  // templates, etc.) from `config.ini` at process spawn. MeedyaDL
  // writes `config.ini` once at app startup (and when GAMDL config
  // is synced from settings), but the GAMDL CLI does NOT re-read the
  // file mid-process. So a template change saved while a download is
  // in flight only takes effect from the *next* app launch.
  //
  // Strategy:
  //   1. Snapshot the template-field values when settings first
  //      hydrate (the load-from-disk pass).
  //   2. On every `Save Changes`, compare current template values
  //      against the snapshot. If any TEMPLATE field changed, open a
  //      modal explaining the restart requirement and offering
  //      Restart Now / Later.
  //   3. After a successful Save, re-snapshot — so subsequent saves
  //      compare against the now-saved state rather than the
  //      original-load state.
  //
  // The list of restart-required keys is co-located here so it's
  // obvious which fields trigger the prompt. If we later identify
  // other restart-only settings (e.g. `activity_log_path_override`
  // already changes on next restart but isn't yet wired here), add
  // them to this list.
  const RESTART_REQUIRED_KEYS = [
    'album_folder_template',
    'compilation_folder_template',
    'no_album_folder_template',
    'playlist_folder_template',
    'single_disc_file_template',
    'multi_disc_file_template',
    'no_album_file_template',
    'playlist_file_template',
  ] as const;
  type RestartKey = (typeof RESTART_REQUIRED_KEYS)[number];

  /** Snapshot taken at load (and refreshed after each successful save). */
  const templateSnapshot = useRef<Record<RestartKey, string> | null>(null);
  /** True after a save where one or more template fields changed. */
  const [restartPromptOpen, setRestartPromptOpen] = useState(false);
  /**
   * Persistent banner shown after the user chose Restart Later. Cleared
   * on next app restart automatically (mounts fresh; ref starts null).
   */
  const [restartPending, setRestartPending] = useState(false);

  /**
   * Load settings from the backend on component mount.
   * This ensures the settings UI always reflects the latest persisted state.
   * The dependency array contains only `loadSettings` (a stable function
   * reference from Zustand), so this effect runs exactly once.
   *
   * Also snapshots the template field values once the load completes (#833),
   * so subsequent `Save Changes` clicks can detect a template-only edit.
   */
  useEffect(() => {
    (async () => {
      await loadSettings();
      const current = useSettingsStore.getState().settings;
      const snap = {} as Record<RestartKey, string>;
      for (const k of RESTART_REQUIRED_KEYS) {
        snap[k] = (current[k] ?? '') as string;
      }
      templateSnapshot.current = snap;
    })();
    // RESTART_REQUIRED_KEYS is a stable module-level constant; safe to
    // omit from deps.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadSettings]);

  /**
   * Persists settings to disk by calling the Zustand store's `saveSettings`
   * action, which in turn invokes the Tauri IPC command `save_settings`.
   * Displays a success or error toast notification based on the outcome.
   *
   * #833: After a successful save, compares the post-save template values
   * to the pre-save snapshot. If any template field changed, opens the
   * restart-prompt modal so the user can either Restart Now or dismiss
   * with a persistent "restart pending" pill.
   */
  const handleSave = async () => {
    // Capture pre-save template values to detect a change AFTER the save
    // returns successfully. We compare against the snapshot from
    // load-or-last-save — not against the current in-memory value, which
    // is what the user is about to persist.
    const preSaveSnapshot = templateSnapshot.current;
    await withErrorToast(() => saveSettings(), {
      successMsg: 'Settings saved successfully',
      errorMsg: 'Failed to save settings',
    });
    // saveSettings() throws on failure (withErrorToast re-surfaces),
    // so reaching this line implies a successful disk write.
    const post = useSettingsStore.getState().settings;
    const changed =
      preSaveSnapshot !== null &&
      RESTART_REQUIRED_KEYS.some(
        (k) => ((post[k] ?? '') as string) !== preSaveSnapshot[k],
      );
    // Refresh snapshot so a second click of Save Changes (with no
    // further template edits) won't re-trigger the prompt.
    const next = {} as Record<RestartKey, string>;
    for (const k of RESTART_REQUIRED_KEYS) {
      next[k] = (post[k] ?? '') as string;
    }
    templateSnapshot.current = next;
    if (changed) {
      setRestartPromptOpen(true);
    }
  };

  /**
   * Relaunches the app via the Tauri process plugin. Used by the
   * "Restart Now" button on the restart-required modal (#833) — same
   * mechanism the in-app updater uses after installing a new version
   * (UpdateBanner.tsx:187).
   */
  const handleRestartNow = async () => {
    try {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (e) {
      addToast(
        `Failed to restart: ${e instanceof Error ? e.message : String(e)}. \
Please quit and reopen MeedyaDL manually.`,
        'error',
      );
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
  const ActiveComponent = TABS.find((t) => t.id === activeTab)?.component || GeneralTab;

  return (
    <div className="flex flex-col h-full">
      {/* Restart-required modal (#833). Triggered by handleSave when one
          or more template fields differ from the pre-save snapshot. The
          "Restart Now" button calls Tauri's `relaunch()` (same mechanism
          UpdateBanner uses post-install). "Restart Later" dismisses and
          sets `restartPending`, which renders a persistent banner so
          the user doesn't forget. */}
      <Modal
        open={restartPromptOpen}
        onClose={() => {
          setRestartPromptOpen(false);
          setRestartPending(true);
        }}
        title="Restart Required"
      >
        <p className="text-sm text-content-primary mb-3">
          Your template changes have been saved, but they only take effect
          for downloads queued <em>after</em> MeedyaDL restarts. Items
          already in flight will continue using the previous templates
          until they finish.
        </p>
        <p className="text-xs text-content-tertiary mb-4">
          GAMDL reads its template configuration when it starts. MeedyaDL
          writes the config once at app launch and doesn't re-spawn GAMDL
          mid-download, so a template edit can't be picked up by an
          already-running subprocess.
        </p>
        <div className="flex justify-end gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setRestartPromptOpen(false);
              setRestartPending(true);
            }}
          >
            Restart Later
          </Button>
          <Button variant="primary" size="sm" onClick={handleRestartNow}>
            Restart Now
          </Button>
        </div>
      </Modal>

      <PageHeader
        title="Settings"
        subtitle="Configure download options, paths, and preferences"
        actions={
          <div className="flex gap-2">
            {/* Restart-pending pill (#833). Shown after the user picked
                "Restart Later" — a passive reminder until the user
                quits or restarts the app. Click to re-open the modal. */}
            {restartPending && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setRestartPromptOpen(true)}
                title="Template changes won't apply until MeedyaDL restarts"
                className="text-status-warning"
              >
                Restart pending
              </Button>
            )}

            {/* Reset to defaults */}
            <Button variant="ghost" size="sm" icon={<RotateCcw size={14} />} onClick={handleReset}>
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
            Fixed width of w-44 (176px). Tabs are grouped under 4 section
            headers (General, Download, Authentication, System). Each
            group has a small uppercase header label followed by its tab
            buttons. The active tab receives an accent background and
            font weight. The sidebar scrolls independently if the window
            is too short to display all groups.
            ---------------------------------------------------------------- */}
        <nav className="w-44 flex-shrink-0 border-r border-border-light overflow-y-auto p-2">
          {SETTINGS_GROUPS.map((group, groupIndex) => (
            <div key={group.label} className={groupIndex > 0 ? 'mt-3' : ''}>
              {/* Group section header -- uppercase, small, muted colour */}
              <div className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-content-tertiary select-none">
                {group.label}
              </div>

              {/* Tab items within this group */}
              <div className="space-y-0.5">
                {group.tabs.map((tabId) => {
                  const tab = TABS.find((t) => t.id === tabId);
                  if (!tab) return null;
                  const { id, label, icon: Icon } = tab;
                  return (
                    <button
                      key={id}
                      onClick={() => setActiveTab(id)}
                      className={`
                        w-full flex items-center gap-2.5 px-3 py-2
                        rounded-platform text-sm transition-colors
                        ${
                          activeTab === id
                            ? 'bg-accent-light text-accent font-medium' /* Active tab styling */
                            : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary' /* Inactive tab styling */
                        }
                      `}
                    >
                      {/* Tab icon -- flex-shrink-0 prevents it from collapsing */}
                      <Icon size={16} className="flex-shrink-0" />
                      {/* Tab label text */}
                      {label}
                    </button>
                  );
                })}
              </div>
            </div>
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
