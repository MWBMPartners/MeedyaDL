/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Root application component for gamdl-GUI.
 * Handles initial application setup including:
 * - Platform detection and theme loading
 * - Settings initialization from the backend
 * - Dependency status checking
 * - Routing between the setup wizard and main application UI
 * - GAMDL progress event listening
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

/* Layout */
import { MainLayout } from './components/layout';

/* Pages */
import { DownloadForm, DownloadQueue } from './components/download';
import { SettingsPage } from './components/settings';
import { HelpViewer } from './components/help';
import { SetupWizard } from './components/setup';

/* Common */
import { LoadingSpinner } from './components/common';

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
 * 6. Listens for GAMDL progress events from the backend
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

  /* Download store for handling progress events */
  const handleProgressEvent = useDownloadStore((s) => s.handleProgressEvent);
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
   * Listen for download-complete and download-error events to refresh
   * the queue status when a download finishes.
   */
  useEffect(() => {
    let unlistenComplete: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    const setupListeners = async () => {
      try {
        unlistenComplete = await listen('download-complete', () => {
          refreshQueue();
        });
        unlistenError = await listen('download-error', () => {
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
    };
  }, [refreshQueue]);

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

  /* Main application UI with sidebar, content, and status bar */
  return <MainLayout>{renderPage()}</MainLayout>;
}

export default App;
