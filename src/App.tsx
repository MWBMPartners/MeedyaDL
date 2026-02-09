/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Root application component for gamdl-GUI.
 * Handles initial application setup including:
 * - Platform detection and theme loading
 * - Settings initialization
 * - Dependency status checking
 * - Routing between the setup wizard and main application UI
 */

import { useEffect, useState } from 'react';

// Import the platform detection hook
import { usePlatform } from './hooks/usePlatform';

// Import platform-specific theme stylesheets
import './styles/themes/base.css';

/**
 * The root component that serves as the entry point for the application UI.
 *
 * On mount, it:
 * 1. Detects the current platform (macOS/Windows/Linux)
 * 2. Loads the appropriate platform theme CSS
 * 3. Checks if dependencies are installed (Python, GAMDL)
 * 4. Shows the setup wizard if dependencies are missing, or the main UI if ready
 */
function App() {
  // Detect the current platform for theme selection
  const { platform, isLoading: platformLoading } = usePlatform();

  // Track whether the application has completed initial loading
  const [isReady, setIsReady] = useState(false);

  // Apply the platform-specific CSS class to the document root
  // This drives all platform-adaptive styling via CSS custom properties
  useEffect(() => {
    if (!platformLoading && platform) {
      // Add the platform class to <html> for CSS targeting
      document.documentElement.classList.add(`platform-${platform}`);

      // Dynamically import the platform-specific theme CSS
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

      // Mark the app as ready after theme is loaded
      setIsReady(true);
    }
  }, [platform, platformLoading]);

  // Show a loading state while detecting platform and loading theme
  if (!isReady) {
    return (
      <div className="flex items-center justify-center h-screen bg-surface-primary">
        <div className="text-center">
          {/* Application name displayed during loading */}
          <h1 className="text-2xl font-semibold text-content-primary mb-2">
            GAMDL
          </h1>
          {/* Loading indicator */}
          <p className="text-sm text-content-secondary">Loading...</p>
        </div>
      </div>
    );
  }

  // Main application UI
  // TODO: Phase 3 - Replace with MainLayout + Router + SetupWizard
  return (
    <div className="flex h-screen bg-surface-primary text-content-primary">
      {/* Sidebar navigation placeholder */}
      <aside className="w-56 bg-sidebar-bg border-r border-sidebar-border flex flex-col">
        {/* App title in sidebar header */}
        <div className="p-4 border-b border-sidebar-border">
          <h1 className="text-lg font-semibold text-sidebar-text-active">
            GAMDL
          </h1>
          <p className="text-xs text-content-secondary mt-0.5">
            Apple Music Downloader
          </p>
        </div>

        {/* Navigation items */}
        <nav className="flex-1 p-2 space-y-1">
          {/* Download page - active by default */}
          <button className="w-full flex items-center gap-3 px-3 py-2 rounded-platform text-sm bg-sidebar-active text-sidebar-text-active">
            <span className="w-5 h-5 flex items-center justify-center text-base">
              &#8595;
            </span>
            Download
          </button>

          {/* Queue page */}
          <button className="w-full flex items-center gap-3 px-3 py-2 rounded-platform text-sm text-sidebar-text hover:bg-sidebar-hover">
            <span className="w-5 h-5 flex items-center justify-center text-base">
              &#9776;
            </span>
            Queue
          </button>

          {/* Settings page */}
          <button className="w-full flex items-center gap-3 px-3 py-2 rounded-platform text-sm text-sidebar-text hover:bg-sidebar-hover">
            <span className="w-5 h-5 flex items-center justify-center text-base">
              &#9881;
            </span>
            Settings
          </button>

          {/* Help page */}
          <button className="w-full flex items-center gap-3 px-3 py-2 rounded-platform text-sm text-sidebar-text hover:bg-sidebar-hover">
            <span className="w-5 h-5 flex items-center justify-center text-base">
              ?
            </span>
            Help
          </button>
        </nav>

        {/* Status bar at bottom of sidebar */}
        <div className="p-3 border-t border-sidebar-border text-xs text-content-tertiary">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-status-warning" />
            Setup Required
          </div>
        </div>
      </aside>

      {/* Main content area */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Content header */}
        <header className="px-6 py-4 border-b border-border-light">
          <h2 className="text-xl font-semibold">Download</h2>
          <p className="text-sm text-content-secondary mt-0.5">
            Enter an Apple Music URL to download music or videos
          </p>
        </header>

        {/* Content body */}
        <div className="flex-1 overflow-y-auto p-6">
          {/* URL input section */}
          <div className="max-w-2xl">
            <label
              htmlFor="url-input"
              className="block text-sm font-medium text-content-primary mb-2"
            >
              Apple Music URL
            </label>
            <div className="flex gap-2">
              <input
                id="url-input"
                type="text"
                placeholder="https://music.apple.com/..."
                className="flex-1 px-3 py-2 rounded-platform border border-border bg-surface-secondary text-content-primary placeholder-content-tertiary text-sm focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent"
              />
              <button className="px-4 py-2 rounded-platform bg-accent text-text-inverse text-sm font-medium hover:bg-accent-hover transition-colors">
                Add to Queue
              </button>
            </div>
            <p className="mt-2 text-xs text-content-tertiary">
              Supports songs, albums, playlists, music videos, and artist pages
            </p>
          </div>

          {/* Setup notice for first run */}
          <div className="mt-8 p-4 rounded-platform-lg bg-surface-elevated border border-border-light max-w-2xl">
            <h3 className="text-sm font-semibold text-content-primary mb-2">
              First-Time Setup Required
            </h3>
            <p className="text-sm text-content-secondary mb-3">
              Before you can download music, the application needs to install
              Python and GAMDL. This is a one-time setup that takes a few
              minutes.
            </p>
            <button className="px-4 py-2 rounded-platform bg-accent text-text-inverse text-sm font-medium hover:bg-accent-hover transition-colors">
              Start Setup
            </button>
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;
