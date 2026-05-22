/**
 * Copyright (c) 2026 MeedyaSuite
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
 * 4. Update check (unconditional, before setup wizard decision)
 * 5. Dependency checks run in parallel (Python, GAMDL, tools)
 * 6. Setup wizard shown if dependencies are missing AND no app update pending
 * 7. Periodic update timer and tray listener registered
 * 8. Event listeners registered for progress, download lifecycle, and tray
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
import { useCallback, useEffect, useState } from 'react';

/**
 * Tauri app info API for retrieving the current app version at runtime.
 * Used to detect pre-release versions (v0.x.x) for the first-load notice.
 * @see {@link https://v2.tauri.app/reference/javascript/api/namespaceapp/}
 */
import { getVersion } from '@tauri-apps/api/app';

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

/**
 * Custom hook for global keyboard shortcuts (Cmd/Ctrl+D, Cmd/Ctrl+,,
 * Cmd/Ctrl+Q, Cmd/Ctrl+Enter). Registers a single window-level keydown
 * listener with cleanup on unmount.
 * @see ./hooks/useKeyboardShortcuts.ts for shortcut definitions
 */
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { useKonamiCode } from './hooks/useKonamiCode';
import { useClipboardMonitor } from './hooks/useClipboardMonitor';
import { activateDevAccess } from './lib/tauri-commands';

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

/** Activity log: accumulates raw subprocess output lines */
import { useActivityStore } from './stores/activityStore';

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
import { DownloadForm, DownloadQueue, HistoryPage, ActivityLog } from './components/download';

/** UpdatesPage: Detailed update view with full release notes */
import { UpdatesPage } from './components/updates';

/**
 * LibraryScanPage (Phase 5 / #717): scan an existing on-disk music library
 * via the existing `scan_folder_for_manifests` IPC, then offer to re-download
 * gaps (missing tracks, format upgrades, content updates from Apple Music).
 */
import { LibraryScanPage } from './components/library/LibraryScanPage';

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
import { LoadingSpinner } from './components/common';

/**
 * PrereleaseNoticeModal: First-load notice shown on each new pre-release
 * version launch. Warns about instability and offers stable release install.
 * @see ./components/common/PrereleaseNoticeModal.tsx
 */
import { Modal } from './components/common/Modal';
import PrereleaseNoticeModal from './components/common/PrereleaseNoticeModal';
import CrashReportOptInModal from './components/common/CrashReportOptInModal';

/* ─── Styles ─────────────────────────────────────────────────────────── */

/**
 * Base theme CSS: defines CSS custom properties (--color-*, --spacing-*, etc.)
 * shared across all platform themes. Platform-specific themes (macos.css,
 * windows.css, linux.css) are loaded dynamically in the platform useEffect.
 */
import './styles/themes/base.css';

/**
 * High-contrast accessibility theme: overrides base/platform tokens with
 * maximum-contrast values when the `high-contrast` class is present on <html>.
 * Must be imported after base.css so its selectors can override the defaults.
 */
import './styles/themes/a11y-high-contrast.css';

/**
 * Colour vision deficiency (CVD) accessible theme: overrides status colour
 * tokens with CVD-friendly palettes when a `cvd-*` class is present on <html>.
 * Must be imported after base.css so its selectors can override the defaults.
 */
import './styles/themes/a11y-colour-blind.css';

/* ─── i18n ──────────────────────────────────────────────────────────── */

/**
 * Internationalization setup using i18next.
 * `initI18n` initializes the translation system with OS language detection.
 * Must be called before any component uses `useTranslation()`.
 * @see ./lib/i18n.ts for configuration details
 */
import { initI18n } from './lib/i18n';
import i18next from 'i18next';

/* ─── Types ──────────────────────────────────────────────────────────── */

/**
 * GamdlProgress: TypeScript interface for the `gamdl-output` event payload.
 * Contains a download_id and a discriminated union GamdlOutputEvent.
 * Mirrors the Rust struct `GamdlProgress` serialized via serde.
 * @see ./types/index.ts for the full type definition
 */
import type { GamdlProgress, ActivityLogEntry } from './types';

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
   * ─── Keyboard Shortcuts ────────────────────────────────────────────
   * Registers global keyboard shortcuts (Cmd/Ctrl+D, Cmd/Ctrl+,,
   * Cmd/Ctrl+Q, Cmd/Ctrl+Enter) via a single window-level keydown listener.
   * Shortcuts are suppressed when focus is in form elements to avoid
   * interfering with text editing. Cleanup is automatic on unmount.
   * @see ./hooks/useKeyboardShortcuts.ts
   */
  useKeyboardShortcuts();

  /*
   * ─── Developer Access (Konami Code) ────────────────────────────────
   * Hidden gesture: ↑↑↓↓←→←→BA triggers a passphrase prompt.
   * On correct passphrase, developer features are unlocked.
   */
  const [showDevPrompt, setShowDevPrompt] = useState(false);
  const [devPassphrase, setDevPassphrase] = useState('');
  const handleKonami = useCallback(() => setShowDevPrompt(true), []);
  useKonamiCode(handleKonami);

  const handleDevSubmit = useCallback(async () => {
    const ok = await activateDevAccess(devPassphrase);
    setShowDevPrompt(false);
    setDevPassphrase('');
    if (ok) {
      // Reload settings to pick up dev_access_enabled change.
      useSettingsStore.getState().loadSettings();
    }
  }, [devPassphrase]);

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
   * ─── Clipboard Monitor ─────────────────────────────────────────────
   *
   * Polls the system clipboard for supported URLs (e.g., Apple Music)
   * and shows an actionable toast when one is detected. Controlled by
   * the `clipboard_monitoring` setting (default: enabled).
   *
   * @see ./hooks/useClipboardMonitor.ts
   */
  useClipboardMonitor(isReady);

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
      /* Step 0: Initialize i18n translation system (loads English + detected locale) */
      await initI18n();

      /* Step 1: Load settings from the Rust backend via IPC (get_settings command) */
      await loadSettings();

      /* Step 1.5: Sync UI language from settings if explicitly set.
       * If ui_language is non-empty, override i18next's auto-detected language.
       * Empty string means "use auto-detected language" (from OS locale). */
      const uiLang = useSettingsStore.getState().settings.ui_language;
      if (uiLang) {
        i18next.changeLanguage(uiLang).catch(() => {});
      }

      /* Step 2: Check for app updates BEFORE dependency checks.
       * On first launch the installed version may already be outdated, so we
       * check for updates early. If an app update is available we skip the
       * setup wizard and let the user update first — the wizard will run on
       * the new version after restart. The check is unconditional (ignores
       * the auto_check_updates setting) because first-launch users haven't
       * configured anything yet. Failures are non-fatal. */
      let appUpdateAvailable = false;
      try {
        const updateResult = await checkForUpdates();
        appUpdateAvailable = updateResult.components.some(
          (c) => c.name === 'app' && c.update_available && c.is_compatible
        );
      } catch {
        /* Non-fatal: network may be unavailable on first launch */
      }

      /* Step 3: Check all dependency statuses in parallel via IPC */
      await checkAll();

      /*
       * Step 4: Show setup wizard if dependencies are missing AND setup
       * has never been completed AND no app update is pending. If an app
       * update is available, we skip the wizard so the user sees the
       * update banner in the main UI and can update first. After updating
       * and restarting, the wizard will run on the new version.
       *
       * If the user has completed setup before (`setup_completed: true`),
       * skip the wizard even if some deps are missing — they may have been
       * intentionally removed, or the app was updated and detection is
       * temporarily broken. The user can always re-run the wizard from Settings.
       *
       * We read the latest state imperatively via getState() rather than
       * using the reactive selectors, because at this point the async
       * `checkAll()` has just completed and we need the freshest snapshot.
       */
      const depState = useDependencyStore.getState();
      const settingsState = useSettingsStore.getState();
      const requiredToolsReady =
        depState.tools.length > 0 &&
        depState.tools.filter((t) => t.required).every((t) => t.installed);
      const depsReady = !!(
        depState.python?.installed &&
        depState.gamdl?.installed &&
        requiredToolsReady
      );
      if (!depsReady && !settingsState.settings.setup_completed && !appUpdateAvailable) {
        setShowSetupWizard(true);
      }

      /*
       * Step 5: Pre-release first-load notice.
       *
       * On the first launch of a new pre-release version (v0.x.x), show a
       * modal informing the user about the pre-release status, potential
       * bugs, and verbose logging behaviour. The version change is detected
       * by comparing the current app version against `last_seen_version`
       * (which the Rust backend updates in load_settings()).
       *
       * The notice is suppressed if the setup wizard is shown (to avoid
       * modal stacking) — it will appear on the next launch after setup.
       */
      try {
        const currentVersion = await getVersion();
        const isPrerelease = currentVersion.startsWith('0.');
        const previousVersion = settingsState.settings.last_seen_version;
        const versionChanged = previousVersion !== '' && previousVersion !== currentVersion;

        if (isPrerelease && versionChanged && !useUiStore.getState().showSetupWizard) {
          useUiStore.getState().setShowPrereleaseNotice(true);
        }

        // Show crash report opt-in prompt on first launch (after setup wizard)
        if (
          !settingsState.settings.crash_report_prompt_shown &&
          settingsState.settings.setup_completed &&
          !useUiStore.getState().showSetupWizard
        ) {
          useUiStore.getState().setShowCrashReportPrompt(true);
        }
      } catch {
        /* Non-fatal: version check failure shouldn't block app startup */
      }

      /*
       * Notification permission preflight (#658).
       *
       * On macOS, `requestPermission()` only triggers the system prompt the
       * first time it is called for a bundle ID. If the user dismissed (not
       * granted, not denied) the prompt during a prior session, every later
       * call resolves silently with `'default'` and `sendNotification()` is
       * a no-op — which is why users on Native + In-app saw no notifications.
       *
       * Requesting permission once at startup forces a predictable, visible
       * prompt and also lets us log the resolved status to the WebView
       * console for debugging. Skipped when the user picked `in_app_only`.
       */
      const notifStyle =
        useSettingsStore.getState().settings.notification_style ?? 'native_and_in_app';
      if (notifStyle !== 'in_app_only') {
        try {
          const { isPermissionGranted, requestPermission } = await import(
            '@tauri-apps/plugin-notification'
          );
          const granted = await isPermissionGranted();
          if (!granted) {
            const result = await requestPermission();
            console.info(`[notification] startup permission request resolved: ${result}`);
          }
        } catch (err) {
          console.warn('[notification] startup permission preflight failed:', err);
        }
      }
    };

    initialize().catch((err) => {
      console.error('App initialization failed:', err);
      useUiStore
        .getState()
        .addToast(
          'App initialization failed — some features may not work correctly. Try restarting.',
          'error'
        );
    });
  }, [isReady, loadSettings, checkAll, checkForUpdates, setShowSetupWizard]);

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
     * Read update settings imperatively from the store snapshot.
     * Using getState() avoids adding `settings` to the dependency array,
     * which would cause this effect to re-run whenever any setting changes.
     */
    const { auto_check_updates: autoCheck, update_check_interval_hours: intervalHours } =
      useSettingsStore.getState().settings;

    /* Startup check is now handled in Effect 2 (before setup wizard decision),
     * so we skip it here to avoid a redundant duplicate check. Effect 2 runs
     * the check unconditionally (even for first-launch users who haven't
     * configured auto_check_updates yet). This Effect only handles the
     * periodic timer and tray listener. */

    /*
     * Periodic check timer. When enabled, checks for updates every N hours
     * while the app is running. The isChecking guard prevents overlapping
     * checks if the user manually triggers one during the interval.
     * Changes to interval setting take effect on app restart.
     */
    let intervalId: ReturnType<typeof setInterval> | undefined;
    if (autoCheck && intervalHours > 0) {
      const intervalMs = intervalHours * 60 * 60 * 1000;
      intervalId = setInterval(() => {
        const { isChecking } = useUpdateStore.getState();
        if (!isChecking) {
          checkForUpdates().catch(() => {});
        }
      }, intervalMs);
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

    /* Cleanup: unsubscribe from tray events and periodic timer on unmount */
    return () => {
      unlistenTray?.();
      if (intervalId) clearInterval(intervalId);
    };
  }, [isReady, checkForUpdates]);

  /*
   * ─── Effect 4b: macOS About Menu → Help > About ────────────────────
   *
   * Listens for the 'navigate-help-about' event emitted by the custom
   * macOS app menu when the user clicks "About MeedyaDL". Navigates to
   * the in-app Help > About page instead of showing the default macOS
   * About dialog.
   */
  useEffect(() => {
    if (!isReady) return;

    let unlistenAbout: (() => void) | undefined;
    const setup = async () => {
      try {
        unlistenAbout = await listen('navigate-help-about', () => {
          useUiStore.getState().navigateToHelp('about');
        });
      } catch {
        /* Tauri API unavailable */
      }
    };
    setup();

    return () => unlistenAbout?.();
  }, [isReady]);

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
    let unlistenQueueUpdated: (() => void) | undefined;
    let unlistenPreflight: (() => void) | undefined;
    let unlistenPreflightCleared: (() => void) | undefined;

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
          }
        );

        /* 3. Download was cancelled by the user */
        unlistenCancelled = await listen<string>('download-cancelled', (event) => {
          try {
            handleDownloadCancelled(event.payload);
          } catch (err) {
            console.error('Error in download-cancelled handler:', err);
          }
        });

        /* 4. New download queued or existing one retried */
        unlistenQueued = await listen('download-queued', () => {
          try {
            refreshQueue();
          } catch (err) {
            console.error('Error in download-queued handler:', err);
          }
        });

        /* 4a. Generic queue state change — fires whenever the backend
         * mutates a queue item (currently: processing_label updates at
         * each enrichment stage). Without this listener, backend label
         * changes during enrichment stay invisible until download-complete
         * fires, and the progress bar keeps showing "DOWNLOADING..." for
         * the entire enrichment phase (#574). The emit is debounced on the
         * backend by the rate of stage transitions (typically <15 per
         * download) so no throttling needed client-side. */
        unlistenQueueUpdated = await listen('queue-updated', () => {
          try {
            refreshQueue();
          } catch (err) {
            console.error('Error in queue-updated handler:', err);
          }
        });

        /* 5. Pre-flight health check warnings.
         * Emitted by the Rust backend before queue processing begins when
         * issues are detected (no internet, expired cookies, wrapper down).
         * Displayed as persistent yellow toasts (duration = 0) so the user
         * sees them and can dismiss after reading.
         *
         * Each toast is keyed by check type (e.g., 'preflight:Wrapper') so that:
         * - Duplicate warnings for the same check are deduplicated.
         * - When the condition resolves, the toast can be dismissed by key
         *   via the 'preflight-cleared' event. */
        unlistenPreflight = await listen<{ check: string; message: string }>(
          'preflight-warning',
          (event) => {
            try {
              const key = `preflight:${event.payload.check}`;
              useUiStore.getState().addToast(event.payload.message, 'warning', 0, key);
            } catch (err) {
              console.error('Error in preflight-warning handler:', err);
            }
          }
        );

        /* 6. Pre-flight check cleared — a previously-failing check now passes.
         * Emitted by the Rust backend when a health check that previously warned
         * now succeeds. Removes the corresponding persistent toast so the user
         * isn't left with stale warnings. */
        unlistenPreflightCleared = await listen<{ check: string }>('preflight-cleared', (event) => {
          try {
            const key = `preflight:${event.payload.check}`;
            useUiStore.getState().removeToastsByKey(key);
          } catch (err) {
            console.error('Error in preflight-cleared handler:', err);
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
      unlistenQueueUpdated?.();
      unlistenPreflight?.();
      unlistenPreflightCleared?.();
    };
  }, [refreshQueue, handleDownloadComplete, handleDownloadError, handleDownloadCancelled]);

  /*
   * ─── Effect 6b: Deep Link URL Handler ──────────────────────────────
   *
   * Listens for the 'deep-link-download' event emitted by the Rust backend
   * when a `meedyadl://download?url=...` URL is opened by an external tool,
   * bookmarklet, or browser extension.
   *
   * On receiving the event:
   * 1. Navigates to the Download page
   * 2. Pre-fills the URL input with the received Apple Music URL
   * 3. Shows a toast notification confirming the URL was received
   * 4. Optionally stores the codec parameter for use in quality overrides
   *
   * @see setup_deep_link_handler() in lib.rs for the Rust-side handler
   * @see https://v2.tauri.app/plugin/deep-linking/ -- Tauri deep link plugin
   */
  useEffect(() => {
    if (!isReady) return;

    let unlistenDeepLink: (() => void) | undefined;
    const setup = async () => {
      try {
        unlistenDeepLink = await listen<{ url: string; codec: string | null }>(
          'deep-link-download',
          (event) => {
            try {
              const { url, codec } = event.payload;

              // Navigate to the Download page
              useUiStore.getState().setPage('download');

              // Pre-fill the URL input via the download store
              useDownloadStore.getState().setUrlInput(url);

              // Show a toast confirming the URL was received
              const codecSuffix = codec ? ` (codec: ${codec})` : '';
              useUiStore
                .getState()
                .addToast(`URL received from external link${codecSuffix}`, 'info');

              console.log(`Deep link received: ${url}${codec ? `, codec=${codec}` : ''}`);
            } catch (err) {
              console.error('Error in deep-link-download handler:', err);
            }
          }
        );
      } catch {
        /* Tauri API unavailable */
      }
    };
    setup();

    return () => unlistenDeepLink?.();
  }, [isReady]);

  /*
   * ─── Effect 7: Activity Log Event Listener ─────────────────────────
   *
   * Subscribes to the "activity-log" Tauri event which carries every raw
   * stdout/stderr line from GAMDL subprocesses. Lines are accumulated in
   * the activityStore regardless of which page is currently displayed,
   * so no output is lost when navigating away from the Activity page.
   *
   * Uses getState().addEntry instead of a selector to avoid re-registering
   * the listener on every store update.
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const { addEntries } = useActivityStore.getState();

    // RAF-batched accumulator: collect incoming events and flush them
    // in a single addEntries() call per animation frame. This collapses
    // hundreds of per-line Zustand updates into ~60/s, dramatically
    // reducing GC pressure and React re-renders.
    let buffer: ActivityLogEntry[] = [];
    let rafId: number | null = null;

    const flush = () => {
      rafId = null;
      if (buffer.length > 0) {
        const batch = buffer;
        buffer = [];
        addEntries(batch);
      }
    };

    const setup = async () => {
      try {
        unlisten = await listen<ActivityLogEntry>('activity-log', (event) => {
          buffer.push(event.payload);
          if (rafId === null) {
            rafId = requestAnimationFrame(flush);
          }
        });
      } catch {
        /* Tauri API unavailable in test/browser environment */
      }
    };

    setup();
    return () => {
      unlisten?.();
      if (rafId !== null) cancelAnimationFrame(rafId);
      // Flush remaining buffered entries on cleanup
      if (buffer.length > 0) {
        useActivityStore.getState().addEntries(buffer);
        buffer = [];
      }
    };
  }, []);

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
        return <DownloadForm />; // URL input form and download options
      case 'queue':
        return <DownloadQueue />; // Real-time download queue with progress
      case 'library':
        return <LibraryScanPage />; // Phase 5 (#717): scan existing on-disk library for re-download gaps
      case 'history':
        return <HistoryPage />; // Persistent download history
      case 'activity':
        return <ActivityLog />; // Live subprocess output log
      case 'updates':
        return <UpdatesPage />; // Update details with release notes
      case 'settings':
        return <SettingsPage />; // Full settings editor
      case 'help':
        return <HelpViewer />; // In-app help documentation
      default:
        return <DownloadForm />; // Fallback to download form
    }
  };

  /*
   * ─── Render: Main Application ──────────────────────────────────────
   * The main UI consists of:
   * - MainLayout: provides sidebar navigation, update banner, and status bar chrome
   * - renderPage(): the currently active page component
   *
   * MainLayout uses children composition, so the content area is flexible.
   * The UpdateBanner is rendered by MainLayout above the scrollable content
   * area so it remains visible regardless of page scroll position.
   * @see ./components/layout/MainLayout.tsx
   */
  return (
    <>
      <MainLayout>{renderPage()}</MainLayout>
      {/* Pre-release first-load notice — rendered as an overlay on top of the
       * main application. Uses the Modal component so it doesn't block the UI
       * and can be dismissed independently. */}
      <PrereleaseNoticeModal />
      <CrashReportOptInModal />
      {/* Developer access passphrase prompt (triggered by Konami code) */}
      <Modal
        open={showDevPrompt}
        onClose={() => { setShowDevPrompt(false); setDevPassphrase(''); }}
        title="Developer Access"
        maxWidth="max-w-sm"
      >
        <form onSubmit={(e) => { e.preventDefault(); handleDevSubmit(); }}>
          <input
            type="password"
            className="w-full rounded border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            placeholder="Passphrase"
            value={devPassphrase}
            onChange={(e) => setDevPassphrase(e.target.value)}
            autoFocus
          />
          <button
            type="submit"
            className="mt-3 w-full rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            Activate
          </button>
        </form>
      </Modal>
    </>
  );
}

export default App;
