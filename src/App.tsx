/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/App.tsx - Root application component for MeedyaDL
 *
 * This is the top-level React component rendered by main.tsx. It orchestrates
 * the entire application lifecycle from first paint to steady-state operation.
 *
 * Architecture overview:
 * - State management: Zustand stores (uiStore, settingsStore, dependencyStore,
 *   downloadStore, updateStore) provide global state without React Context overhead.
 *   Each store is accessed via selector hooks to minimize re-renders.
 *   @see {@link https://docs.pmnd.rs/zustand/getting-started/introduction} - Zustand docs
 *
 * - IPC bridge: The Tauri event system (`@tauri-apps/api/event`) delivers
 *   real-time events from the Rust backend (progress updates, download lifecycle).
 *   @see {@link https://v2.tauri.app/develop/calling-frontend/} - Tauri calling frontend
 *
 * - Routing: A lightweight page router based on `currentPage` state in uiStore,
 *   avoiding the need for react-router. Pages are swapped via a switch statement.
 *
 * Startup sequence (in order):
 * 1. Platform detection via usePlatform() hook (async, detects OS)
 * 2. Platform-specific CSS theme loaded dynamically
 * 3. Settings loaded from Rust backend via IPC
 * 4. Dependency checks run in parallel (Python, GAMDL, tools)
 * 5. Setup wizard shown if dependencies are missing; main UI if ready
 * 6. Auto-update check fires if enabled in settings
 * 7. Event listeners registered for progress, download lifecycle, and tray
 *
 * @see {@link https://react.dev/reference/react/useEffect} - useEffect lifecycle
 * @see {@link https://react.dev/reference/react/useState} - useState hook
 * @see {@link https://v2.tauri.app/develop/calling-frontend/} - Tauri event system
 */

/**
 * React hooks for component lifecycle and local state.
 * - `useEffect`: runs side effects after render (event listeners, async init)
 * - `useState`: manages local component state (isReady flag)
 * @see {@link https://react.dev/reference/react/useEffect}
 * @see {@link https://react.dev/reference/react/useState}
 */
import { useEffect, useState } from 'react';

/**
 * Tauri event listener API for receiving events emitted from the Rust backend.
 * `listen<T>(event, handler)` returns an unlisten function for cleanup.
 * Used here to receive: gamdl-output, download-complete, download-error,
 * download-cancelled, download-queued, and tray-check-updates events.
 * @see {@link https://v2.tauri.app/reference/javascript/api/namespacevent/} - Tauri event API
 */
import { listen } from '@tauri-apps/api/event';

/* ─── Hooks ──────────────────────────────────────────────────────────── */

/**
 * Custom hook for runtime OS detection (macOS / Windows / Linux).
 * Returns the platform identifier and loading state, enabling the App
 * component to defer rendering until the correct theme CSS is loaded.
 * @see ./hooks/usePlatform.ts for implementation details
 */
import { usePlatform } from './hooks/usePlatform';

/**
 * Custom hook for dark/light/auto theme override management.
 * Syncs the `theme_override` setting to CSS classes on <html>, enabling
 * manual override of the OS color scheme preference.
 * @see ./hooks/useTheme.ts for implementation details
 */
import { useTheme } from './hooks/useTheme';

/* ─── Zustand Stores ─────────────────────────────────────────────────── */
/*
 * Each store is a Zustand slice providing global state and actions.
 * Selectors `(s) => s.property` are used to subscribe to specific slices
 * of state, preventing unnecessary re-renders when unrelated state changes.
 * @see {@link https://docs.pmnd.rs/zustand/guides/prevent-rerenders-with-use-shallow}
 */

/** UI state: current page, sidebar collapse, setup wizard visibility */
import { useUiStore } from './stores/uiStore';

/** Persistent settings: loaded from and saved to the Rust backend */
import { useSettingsStore } from './stores/settingsStore';

/** Dependency status: tracks Python, GAMDL, and tool installation states */
import { useDependencyStore } from './stores/dependencyStore';

/** Download queue: manages queue items, progress events, and lifecycle */
import { useDownloadStore } from './stores/downloadStore';

/** Update checker: polls for new versions of app components */
import { useUpdateStore } from './stores/updateStore';

/* ─── Layout ─────────────────────────────────────────────────────────── */

/**
 * MainLayout provides the persistent chrome: sidebar navigation,
 * status bar, and content area. All page components render inside it.
 * @see ./components/layout/ for layout component implementations
 */
import { MainLayout } from './components/layout';

/* ─── Page Components ────────────────────────────────────────────────── */

/**
 * DownloadForm: URL input and option configuration for new downloads.
 * DownloadQueue: Real-time view of all queued/active/completed downloads.
 */
import { DownloadForm, DownloadQueue } from './components/download';

/** SettingsPage: Full application settings editor with save/reset */
import { SettingsPage } from './components/settings';

/** HelpViewer: In-app help documentation and FAQ */
import { HelpViewer } from './components/help';

/**
 * SetupWizard: Multi-step guided setup shown on first run or when
 * required dependencies (Python, GAMDL) are missing.
 * @see ./components/setup/ for wizard step implementations
 */
import { SetupWizard } from './components/setup';

/* ─── Common Components ──────────────────────────────────────────────── */

/** LoadingSpinner: Animated spinner shown during async loading states */
/** UpdateBanner: Dismissible banner shown when updates are available */
import { LoadingSpinner, UpdateBanner } from './components/common';

/* ─── Styles ─────────────────────────────────────────────────────────── */

/**
 * Base theme CSS: defines CSS custom properties (--color-*, --spacing-*, etc.)
 * shared across all platform themes. Platform-specific themes (macos.css,
 * windows.css, linux.css) are loaded dynamically in the platform useEffect.
 */
import './styles/themes/base.css';

/* ─── Types ──────────────────────────────────────────────────────────── */

/**
 * GamdlProgress: TypeScript interface for the `gamdl-output` event payload.
 * Contains a download_id and a discriminated union GamdlOutputEvent.
 * Mirrors the Rust struct `GamdlProgress` serialized via serde.
 * @see ./types/index.ts for the full type definition
 */
import type { GamdlProgress } from './types';

/**
 * The root component that serves as the entry point for the application UI.
 *
 * On mount, it:
 * 1. Detects the current platform (macOS/Windows/Linux)
 * 2. Loads the appropriate platform theme CSS
 * 3. Loads settings from the backend
 * 4. Checks if dependencies are installed (Python, GAMDL)
 * 5. Shows the setup wizard if dependencies are missing, or the main UI if ready
 * 6. Auto-checks for updates if enabled in settings
 * 7. Listens for GAMDL progress events from the backend
 * 8. Listens for system tray events (update check trigger)
 */
function App() {
  /*
   * ─── Platform Detection ────────────────────────────────────────────
   * usePlatform() asynchronously detects the OS via Tauri's OS plugin.
   * `platform` is 'macos' | 'windows' | 'linux'; `platformLoading` is
   * true until detection completes. We block rendering until this resolves
   * so that the correct theme CSS is loaded before any UI paints.
   * @see ./hooks/usePlatform.ts
   */
  const { platform, isLoading: platformLoading } = usePlatform();

  /*
   * ─── Theme Override ─────────────────────────────────────────────────
   * useTheme() reads the `theme_override` setting from the settings store
   * and applies the appropriate CSS class ('theme-dark' or 'theme-light')
   * to the <html> element. When set to null (auto), no class is applied
   * and the OS media query controls dark/light mode.
   * @see ./hooks/useTheme.ts
   */
  useTheme();

  /*
   * ─── Local State ───────────────────────────────────────────────────
   * `isReady` gates the entire UI. It becomes `true` only after:
   * 1. Platform detection completes
   * 2. Platform-specific theme CSS is dynamically imported
   * This prevents a flash of unstyled content (FOUC).
   * @see {@link https://react.dev/reference/react/useState}
   */
  const [isReady, setIsReady] = useState(false);

  /*
   * ─── UI Store Selectors ────────────────────────────────────────────
   * Zustand selector pattern: `useStore((s) => s.field)` subscribes only
   * to that specific field, so this component only re-renders when
   * `currentPage` or `showSetupWizard` changes -- not on any UI store update.
   * @see {@link https://docs.pmnd.rs/zustand/guides/prevent-rerenders-with-use-shallow}
   */
  /** The currently active navigation page (drives the page router below) */
  const currentPage = useUiStore((s) => s.currentPage);
  /** Whether the setup wizard overlay is visible (first-run or missing deps) */
  const showSetupWizard = useUiStore((s) => s.showSetupWizard);
  /** Action to show/hide the setup wizard */
  const setShowSetupWizard = useUiStore((s) => s.setShowSetupWizard);

  /*
   * ─── Settings Store Selectors ──────────────────────────────────────
   * `loadSettings` triggers an IPC call to the Rust backend to fetch
   * persisted settings from disk (JSON file in the app data directory).
   * `sidebarCollapsed` is the only reactive setting App.tsx needs --
   * subscribing to the entire `settings` object would cause App (and
   * its entire subtree) to re-render on any settings change.
   * @see ./stores/settingsStore.ts
   */
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  const sidebarCollapsedSetting = useSettingsStore((s) => s.settings.sidebar_collapsed);

  /*
   * ─── Dependency Store Selectors ────────────────────────────────────
   * `checkAll` runs parallel IPC calls to check Python, GAMDL, and tool
   * installation status. The dependency readiness check is done imperatively
   * via `useDependencyStore.getState()` inside Effect 2, rather than via
   * reactive subscriptions, because App only needs to check this once at
   * startup. The setup wizard visibility is controlled by `showSetupWizard`.
   * @see ./stores/dependencyStore.ts
   */
  const checkAll = useDependencyStore((s) => s.checkAll);

  /*
   * ─── Update Store Selector ─────────────────────────────────────────
   * `checkForUpdates` triggers an IPC call to check all component versions
   * against their latest available versions (PyPI for GAMDL, GitHub for app).
   * @see ./stores/updateStore.ts
   */
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);

  /*
   * ─── Download Store Selectors ──────────────────────────────────────
   * These handlers process real-time events from the Rust backend:
   * - handleProgressEvent: updates progress bars, track info, speed/ETA
   * - handleDownloadComplete: marks a queue item as 'complete'
   * - handleDownloadError: marks a queue item as 'error' with message
   * - handleDownloadCancelled: marks a queue item as 'cancelled'
   * - refreshQueue: re-fetches the full queue status from the backend
   * @see ./stores/downloadStore.ts
   */
  const handleProgressEvent = useDownloadStore((s) => s.handleProgressEvent);
  const handleDownloadComplete = useDownloadStore((s) => s.handleDownloadComplete);
  const handleDownloadError = useDownloadStore((s) => s.handleDownloadError);
  const handleDownloadCancelled = useDownloadStore((s) => s.handleDownloadCancelled);
  const refreshQueue = useDownloadStore((s) => s.refreshQueue);

  /*
   * ─── Effect 1: Platform Theme Application ──────────────────────────
   *
   * Once platform detection completes, this effect:
   * 1. Adds a CSS class `platform-{macos|windows|linux}` to the <html> element,
   *    enabling platform-conditional CSS selectors throughout the app
   *    (e.g., `.platform-macos .sidebar { ... }`)
   * 2. Dynamically imports the platform-specific CSS theme file using
   *    Vite's code-splitting (`import()` expression). This ensures only
   *    the relevant theme CSS is downloaded, not all three.
   * 3. Sets `isReady = true` to unblock the rest of the UI
   *
   * Dependencies: [platform, platformLoading] -- re-runs if platform changes
   * (practically runs once since platform is detected once on startup).
   *
   * @see {@link https://react.dev/reference/react/useEffect} - useEffect docs
   * @see {@link https://vite.dev/guide/features.html#dynamic-import} - Vite dynamic imports
   */
  useEffect(() => {
    if (!platformLoading && platform) {
      /* Add the platform class to <html> for CSS targeting */
      document.documentElement.classList.add(`platform-${platform}`);

      /*
       * Dynamically import the platform-specific theme CSS.
       * Vite splits these into separate chunks so only the needed theme loads.
       * The import() call is fire-and-forget; CSS is applied as a side effect
       * once the module is evaluated by the browser.
       */
      switch (platform) {
        case 'macos':
          import('./styles/themes/macos.css');
          break;
        case 'windows':
          import('./styles/themes/windows.css');
          break;
        default:
          /* Linux, FreeBSD, and other Unix-like platforms use the Linux theme */
          import('./styles/themes/linux.css');
          break;
      }

      /* Mark the app as ready -- this unblocks the loading screen below */
      setIsReady(true);
    }
  }, [platform, platformLoading]);

  /*
   * ─── Effect 2: Application Initialization ──────────────────────────
   *
   * Runs once `isReady` becomes true (i.e., after the platform theme loads).
   * This is the core startup sequence:
   *
   * 1. `loadSettings()` -- IPC call to Rust `get_settings` command.
   *    Returns persisted AppSettings or defaults if first run.
   *    Populates the settingsStore so all components have access.
   *
   * 2. `checkAll()` -- IPC calls to check Python, GAMDL, and tool status.
   *    Runs checks in parallel for faster startup.
   *
   * 3. `isReady_deps()` -- derived check: returns false if any required
   *    dependency is missing. If false, the setup wizard is shown.
   *
   * The async IIFE pattern (`const initialize = async () => { ... }; initialize()`)
   * is used because useEffect callbacks cannot be async directly.
   *
   * Dependencies: stable references from Zustand stores (won't cause re-runs).
   *
   * @see {@link https://react.dev/reference/react/useEffect#fetching-data-with-effects}
   */
  useEffect(() => {
    /* Gate: don't initialize until platform theme is loaded */
    if (!isReady) return;

    const initialize = async () => {
      /* Step 1: Load settings from the Rust backend via IPC (get_settings command) */
      await loadSettings();

      /* Step 2: Check all dependency statuses in parallel via IPC */
      await checkAll();

      /*
       * Step 3: Show setup wizard if any required dependency is missing.
       * We read the latest state imperatively via getState() rather than using
       * the reactive selectors, because at this point the async `checkAll()`
       * has just completed and we need the freshest snapshot.
       */
      const depState = useDependencyStore.getState();
      const requiredToolsReady =
        depState.tools.length > 0 &&
        depState.tools.filter((t) => t.required).every((t) => t.installed);
      const depsReady = !!(
        depState.python?.installed &&
        depState.gamdl?.installed &&
        requiredToolsReady
      );
      if (!depsReady) {
        setShowSetupWizard(true);
      }
    };

    initialize();
  }, [isReady, loadSettings, checkAll, setShowSetupWizard]);

  /*
   * ─── Effect 3: Sync Sidebar State from Settings ────────────────────
   *
   * After settings load, sync the sidebar collapsed/expanded state from
   * the persisted settings into the UI store. This uses `useUiStore.getState()`
   * to access the store imperatively (outside the React render cycle),
   * which is a valid Zustand pattern for one-off state synchronization.
   *
   * Dependency: [sidebarCollapsedSetting] -- re-runs when the setting changes.
   *
   * @see {@link https://docs.pmnd.rs/zustand/guides/practice-with-no-store-actions}
   */
  useEffect(() => {
    if (sidebarCollapsedSetting) {
      useUiStore.getState().setSidebarCollapsed(sidebarCollapsedSetting);
    }
  }, [sidebarCollapsedSetting]);

  /*
   * ─── Effect 4: Auto-Update Check and Tray Listener ─────────────────
   *
   * Two responsibilities:
   *
   * A) Auto-check for updates on startup:
   *    Reads `auto_check_updates` from settings imperatively (getState())
   *    to avoid subscribing to the full settings object. If enabled,
   *    fires `checkForUpdates()` which calls the Rust `check_all_updates`
   *    command. Failures are silently caught -- update checking is non-critical.
   *
   * B) System tray "Check for Updates" listener:
   *    The Rust backend emits a `tray-check-updates` event when the user
   *    clicks the "Check for Updates" item in the OS system tray context menu.
   *    This listener triggers the same update check flow.
   *    @see {@link https://v2.tauri.app/develop/calling-frontend/} - Tauri events
   *
   * Cleanup: The returned function unsubscribes the tray event listener
   * to prevent memory leaks if the component unmounts.
   *
   * Dependencies: [isReady, checkForUpdates]
   */
  useEffect(() => {
    /* Gate: wait until the app is fully initialized */
    if (!isReady) return;

    /*
     * Read auto_check_updates imperatively from the store snapshot.
     * Using getState() avoids adding `settings` to the dependency array,
     * which would cause this effect to re-run whenever any setting changes.
     */
    const autoCheck = useSettingsStore.getState().settings.auto_check_updates;
    if (autoCheck) {
      checkForUpdates().catch(() => {
        /* Non-fatal: silently ignore update check failures on startup */
      });
    }

    /*
     * Register a listener for the 'tray-check-updates' Tauri event.
     * The `listen()` call is async, so we store the unlisten function
     * in a closure variable for cleanup.
     */
    let unlistenTray: (() => void) | undefined;
    const setupTrayListener = async () => {
      try {
        unlistenTray = await listen('tray-check-updates', () => {
          checkForUpdates().catch(() => {});
        });
      } catch {
        /* Tauri API unavailable (running in browser dev mode) */
      }
    };
    setupTrayListener();

    /* Cleanup: unsubscribe from tray events on unmount */
    return () => unlistenTray?.();
  }, [isReady, checkForUpdates]);

  /*
   * ─── Effect 5: GAMDL Progress Event Listener ──────────────────────
   *
   * Subscribes to the `gamdl-output` Tauri event, which the Rust backend
   * emits whenever the GAMDL subprocess (Python process) writes to stdout/stderr.
   *
   * Event payload type: `GamdlProgress` (see src/types/index.ts)
   * - download_id: unique ID linking this event to a queue item
   * - event: discriminated union (track_info | download_progress |
   *   processing_step | error | complete | unknown)
   *
   * The `listen<T>()` generic parameter provides type safety for the payload.
   * `event.payload` is automatically deserialized from the Rust struct by
   * Tauri's serde-based serialization bridge.
   *
   * The async pattern with a stored unlisten function handles the fact that
   * `listen()` returns a Promise, but useEffect cleanup must be synchronous.
   *
   * @see {@link https://v2.tauri.app/develop/calling-frontend/#listening-to-events}
   * @see {@link https://v2.tauri.app/reference/javascript/api/namespacevent/}
   */
  useEffect(() => {
    /** Stores the unlisten function returned by `listen()` for cleanup */
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        /* Subscribe to gamdl-output events with typed payload */
        unlisten = await listen<GamdlProgress>('gamdl-output', (event) => {
          /* Delegate to the download store to update queue item state.
           * Wrapped in try/catch because event handler errors are NOT caught
           * by React's ErrorBoundary (they run outside the render cycle). */
          try {
            handleProgressEvent(event.payload);
          } catch (err) {
            console.error('Error in gamdl-output handler:', err);
          }
        });
      } catch {
        /* Tauri API unavailable (running in browser dev mode) */
      }
    };

    setupListener();
    /* Cleanup: unsubscribe when component unmounts or handler changes */
    return () => unlisten?.();
  }, [handleProgressEvent]);

  /*
   * ─── Effect 6: Download Lifecycle Event Listeners ──────────────────
   *
   * Subscribes to four Tauri events that track the lifecycle of each
   * download queue item. These are emitted by the Rust download queue
   * manager when a download transitions between states.
   *
   * Events handled:
   *
   * 1. `download-complete` (payload: string = download_id)
   *    Fired when GAMDL finishes downloading all tracks for a queue item.
   *    Updates the item state to 'complete' and refreshes the queue.
   *
   * 2. `download-error` (payload: { download_id: string, error: string })
   *    Fired when GAMDL exits with an error or the process crashes.
   *    Updates the item state to 'error' with the error message.
   *
   * 3. `download-cancelled` (payload: string = download_id)
   *    Fired when the user cancels a download via the queue UI.
   *    Updates the item state to 'cancelled'.
   *
   * 4. `download-queued` (no typed payload)
   *    Fired when a new download is added or an existing one is retried.
   *    Triggers a full queue refresh to pick up the new item.
   *
   * All four listeners are registered together and cleaned up together
   * to ensure consistent state.
   *
   * @see {@link https://v2.tauri.app/develop/calling-frontend/#listening-to-events}
   */
  useEffect(() => {
    /** Unlisten functions for each event -- all cleaned up on unmount */
    let unlistenComplete: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenCancelled: (() => void) | undefined;
    let unlistenQueued: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        /* 1. Download completed successfully.
         * All lifecycle handlers are wrapped in try/catch because event handler
         * errors run outside React's render cycle and won't be caught by the
         * ErrorBoundary. Without this, a single handler crash would silently
         * break download status updates for the rest of the session. */
        unlistenComplete = await listen<string>('download-complete', (event) => {
          try {
            handleDownloadComplete(event.payload);
            refreshQueue();
          } catch (err) {
            console.error('Error in download-complete handler:', err);
          }
        });

        /* 2. Download failed with an error */
        unlistenError = await listen<{ download_id: string; error: string }>(
          'download-error',
          (event) => {
            try {
              handleDownloadError(event.payload.download_id, event.payload.error);
              refreshQueue();
            } catch (err) {
              console.error('Error in download-error handler:', err);
            }
          },
        );

        /* 3. Download was cancelled by the user */
        unlistenCancelled = await listen<string>(
          'download-cancelled',
          (event) => {
            try {
              handleDownloadCancelled(event.payload);
            } catch (err) {
              console.error('Error in download-cancelled handler:', err);
            }
          },
        );

        /* 4. New download queued or existing one retried */
        unlistenQueued = await listen('download-queued', () => {
          try {
            refreshQueue();
          } catch (err) {
            console.error('Error in download-queued handler:', err);
          }
        });
      } catch {
        /* Tauri API unavailable (running in browser dev mode) */
      }
    };

    setupListeners();

    /*
     * Cleanup: unsubscribe all four listeners.
     * The optional chaining (?.) handles the case where listen() hasn't
     * resolved yet when the component unmounts.
     */
    return () => {
      unlistenComplete?.();
      unlistenError?.();
      unlistenCancelled?.();
      unlistenQueued?.();
    };
  }, [refreshQueue, handleDownloadComplete, handleDownloadError, handleDownloadCancelled]);

  /*
   * ─── Render: Loading State ─────────────────────────────────────────
   * While platform detection is in progress, show a centered loading spinner.
   * This prevents FOUC (flash of unstyled content) by blocking rendering
   * until the correct platform theme CSS has been loaded.
   */
  if (!isReady) {
    return (
      <div className="flex items-center justify-center h-screen bg-surface-primary">
        <LoadingSpinner size="lg" label="Loading MeedyaDL..." />
      </div>
    );
  }

  /*
   * ─── Render: Setup Wizard ──────────────────────────────────────────
   * If required dependencies (Python, GAMDL) are missing, render the
   * full-screen setup wizard instead of the main application.
   * The wizard guides the user through installation and returns here
   * once all dependencies are satisfied (setShowSetupWizard(false)).
   * @see ./components/setup/SetupWizard.tsx
   */
  if (showSetupWizard) {
    return <SetupWizard />;
  }

  /*
   * ─── Render Helper: Page Router ────────────────────────────────────
   * A lightweight client-side router that maps the `currentPage` string
   * from uiStore to the corresponding page component. This avoids the
   * overhead of react-router for a simple single-level navigation model.
   *
   * Navigation is driven by sidebar clicks which call
   * `useUiStore.getState().setCurrentPage(page)`.
   *
   * The `AppPage` type (see src/types/index.ts) constrains valid page values.
   */
  const renderPage = () => {
    switch (currentPage) {
      case 'download':
        return <DownloadForm />;   // URL input form and download options
      case 'queue':
        return <DownloadQueue />;  // Real-time download queue with progress
      case 'settings':
        return <SettingsPage />;   // Full settings editor
      case 'help':
        return <HelpViewer />;     // In-app help documentation
      default:
        return <DownloadForm />;   // Fallback to download form
    }
  };

  /*
   * ─── Render: Main Application ──────────────────────────────────────
   * The main UI consists of:
   * - MainLayout: provides sidebar navigation and status bar chrome
   * - UpdateBanner: dismissible notification when updates are available
   * - renderPage(): the currently active page component
   *
   * MainLayout uses children composition, so the content area is flexible.
   * @see ./components/layout/MainLayout.tsx
   */
  return (
    <MainLayout>
      <UpdateBanner />
      {renderPage()}
    </MainLayout>
  );
}

export default App;
