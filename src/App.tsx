/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Root application component for gamdl-GUI.
 * Handles initial application setup including:
 * - Platform detection and theme loading
 * - Settings initialization from the backend
 * - Dependency status checking
 * - Auto-update checking on startup
 * - Routing between the setup wizard and main application UI
 * - GAMDL progress event listening
 * - System tray event handling
 */

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';

/* Hooks */
import { usePlatform } from './hooks/usePlatform';

/* Stores */
import { useUiStore } from './stores/uiStore';
import { useSettingsStore } from './stores/settingsStore';
import { useDependencyStore } from './stores/dependencyStore';
import { useDownloadStore } from './stores/downloadStore';
import { useUpdateStore } from './stores/updateStore';

/* Layout */
import { MainLayout } from './components/layout';

/* Pages */
import { DownloadForm, DownloadQueue } from './components/download';
import { SettingsPage } from './components/settings';
import { HelpViewer } from './components/help';
import { SetupWizard } from './components/setup';

/* Common */
import { LoadingSpinner, UpdateBanner } from './components/common';

/* Styles */
import './styles/themes/base.css';

/* Types */
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
  /* Platform detection for theme selection */
  const { platform, isLoading: platformLoading } = usePlatform();

  /* Track whether the application has completed initial loading */
  const [isReady, setIsReady] = useState(false);

  /* UI store for page routing and setup wizard visibility */
  const currentPage = useUiStore((s) => s.currentPage);
  const showSetupWizard = useUiStore((s) => s.showSetupWizard);
  const setShowSetupWizard = useUiStore((s) => s.setShowSetupWizard);

  /* Settings store for loading saved settings */
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  const settings = useSettingsStore((s) => s.settings);

  /* Dependency store for checking if setup is needed */
  const checkAll = useDependencyStore((s) => s.checkAll);
  const isReady_deps = useDependencyStore((s) => s.isReady);

  /* Update store for checking component updates */
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);

  /* Download store for handling progress events */
  const handleProgressEvent = useDownloadStore((s) => s.handleProgressEvent);
  const handleDownloadComplete = useDownloadStore((s) => s.handleDownloadComplete);
  const handleDownloadError = useDownloadStore((s) => s.handleDownloadError);
  const handleDownloadCancelled = useDownloadStore((s) => s.handleDownloadCancelled);
  const refreshQueue = useDownloadStore((s) => s.refreshQueue);

  /*
   * Apply the platform-specific CSS class to the document root.
   * This drives all platform-adaptive styling via CSS custom properties.
   */
  useEffect(() => {
    if (!platformLoading && platform) {
      /* Add the platform class to <html> for CSS targeting */
      document.documentElement.classList.add(`platform-${platform}`);

      /* Dynamically import the platform-specific theme CSS */
      switch (platform) {
        case 'macos':
          import('./styles/themes/macos.css');
          break;
        case 'windows':
          import('./styles/themes/windows.css');
          break;
        default:
          import('./styles/themes/linux.css');
          break;
      }

      /* Mark the app as ready after theme is loaded */
      setIsReady(true);
    }
  }, [platform, platformLoading]);

  /*
   * Initialize the app on first ready: load settings, check dependencies,
   * and determine if the setup wizard needs to be shown.
   */
  useEffect(() => {
    if (!isReady) return;

    const initialize = async () => {
      /* Load settings from the backend (or use defaults) */
      await loadSettings();

      /* Check dependency statuses in parallel */
      await checkAll();

      /* Show setup wizard if required dependencies are missing */
      if (!isReady_deps()) {
        setShowSetupWizard(true);
      }
    };

    initialize();
  }, [isReady, loadSettings, checkAll, isReady_deps, setShowSetupWizard]);

  /*
   * Apply sidebar collapsed state from settings after they're loaded.
   */
  useEffect(() => {
    if (settings.sidebar_collapsed) {
      useUiStore.getState().setSidebarCollapsed(settings.sidebar_collapsed);
    }
  }, [settings.sidebar_collapsed]);

  /*
   * Auto-check for updates on startup if the setting is enabled.
   * Also listens for the 'tray-check-updates' event emitted when the
   * user clicks "Check for Updates" in the system tray context menu.
   */
  useEffect(() => {
    if (!isReady) return;

    /* Check for updates automatically if the user has the setting enabled */
    const autoCheck = useSettingsStore.getState().settings.auto_check_updates;
    if (autoCheck) {
      checkForUpdates().catch(() => {
        /* Non-fatal: silently ignore update check failures on startup */
      });
    }

    /* Listen for tray-triggered update checks */
    let unlistenTray: (() => void) | undefined;
    const setupTrayListener = async () => {
      try {
        unlistenTray = await listen('tray-check-updates', () => {
          checkForUpdates().catch(() => {});
        });
      } catch {
        /* Tauri API unavailable */
      }
    };
    setupTrayListener();

    return () => unlistenTray?.();
  }, [isReady, checkForUpdates]);

  /*
   * Listen for GAMDL progress events from the Tauri backend.
   * These events are emitted by the Rust gamdl_service when a download
   * subprocess produces output.
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<GamdlProgress>('gamdl-output', (event) => {
          handleProgressEvent(event.payload);
        });
      } catch {
        /* Tauri API unavailable (running in browser dev mode) */
      }
    };

    setupListener();
    return () => unlisten?.();
  }, [handleProgressEvent]);

  /*
   * Listen for download lifecycle events from the Tauri backend.
   * These events update individual queue items in real-time:
   * - download-complete: marks a download as complete
   * - download-error: marks a download as errored with the error message
   * - download-cancelled: marks a download as cancelled
   * - download-queued: refreshes the queue when a new item is added or retried
   */
  useEffect(() => {
    let unlistenComplete: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenCancelled: (() => void) | undefined;
    let unlistenQueued: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenComplete = await listen<string>('download-complete', (event) => {
          handleDownloadComplete(event.payload);
          refreshQueue();
        });
        unlistenError = await listen<{ download_id: string; error: string }>(
          'download-error',
          (event) => {
            handleDownloadError(event.payload.download_id, event.payload.error);
            refreshQueue();
          },
        );
        unlistenCancelled = await listen<string>(
          'download-cancelled',
          (event) => {
            handleDownloadCancelled(event.payload);
          },
        );
        unlistenQueued = await listen('download-queued', () => {
          refreshQueue();
        });
      } catch {
        /* Tauri API unavailable */
      }
    };

    setupListeners();
    return () => {
      unlistenComplete?.();
      unlistenError?.();
      unlistenCancelled?.();
      unlistenQueued?.();
    };
  }, [refreshQueue, handleDownloadComplete, handleDownloadError, handleDownloadCancelled]);

  /* Show a loading screen while detecting platform and loading theme */
  if (!isReady) {
    return (
      <div className="flex items-center justify-center h-screen bg-surface-primary">
        <LoadingSpinner size="lg" label="Loading GAMDL..." />
      </div>
    );
  }

  /* Show the setup wizard if dependencies are missing */
  if (showSetupWizard) {
    return <SetupWizard />;
  }

  /*
   * Render the active page based on the UI store's currentPage value.
   * This serves as a simple client-side router without needing react-router.
   */
  const renderPage = () => {
    switch (currentPage) {
      case 'download':
        return <DownloadForm />;
      case 'queue':
        return <DownloadQueue />;
      case 'settings':
        return <SettingsPage />;
      case 'help':
        return <HelpViewer />;
      default:
        return <DownloadForm />;
    }
  };

  /* Main application UI with sidebar, update banner, content, and status bar */
  return (
    <MainLayout>
      <UpdateBanner />
      {renderPage()}
    </MainLayout>
  );
}

export default App;
