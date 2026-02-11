// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file dependencyStore.ts -- Dependency Checking & Installation State Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Tracks the installation status of all required and optional dependencies:
 *
 *   **Required dependencies** (the app cannot download without these):
 *   - **Python** -- A portable Python runtime bundled/managed by the app.
 *     Checked via `commands.checkPythonStatus()` -> Rust `check_python_status`.
 *   - **GAMDL** -- The Python package that performs the actual Apple Music
 *     downloading. Installed into the portable Python environment via pip.
 *     Checked via `commands.checkGamdlStatus()` -> Rust `check_gamdl_status`.
 *
 *   **External tools** (optional but enhance functionality):
 *   - FFmpeg, mp4decrypt, MP4Box, N_m3u8DL-RE, amdecrypt, etc.
 *     Checked via `commands.checkAllDependencies()` -> Rust `check_all_dependencies`.
 *     Each returns a `DependencyStatus` with name, installed, version, and path.
 *
 * This store is consumed by:
 *   - `<SetupWizard>` -- Guides first-time users through installing each dependency.
 *   - `<StatusBar>` / `<DependencyIndicator>` -- Shows green/red status icons.
 *   - `<SettingsPage>` -- Displays tool paths and versions in the advanced section.
 *   - `<App>` -- Calls `checkAll()` on startup to determine if setup is needed.
 *
 * The `isReady()` computed getter is the key decision point: it returns `true`
 * only when both Python and GAMDL are installed, which is the minimum requirement
 * for the download functionality to work.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- Zustand state updates
 * @see {@link https://v2.tauri.app/develop/calling-rust/} -- Tauri IPC invoke()
 */

// Zustand store factory. Creates a React hook backed by a single store instance.
import { create } from 'zustand';

// DependencyStatus -- { name, required, installed, version, path } shape from the backend.
// Mirrors the Rust `DependencyStatus` struct serialized over the IPC boundary.
import type { DependencyStatus } from '@/types';

// Type-safe wrappers for Tauri IPC commands related to dependency management.
// Each function maps to a `#[tauri::command]` handler in the Rust backend.
import * as commands from '@/lib/tauri-commands';

/**
 * Combined state + actions interface for the dependency store.
 *
 * State is organized into three logical groups:
 *   1. **Status fields** (`python`, `gamdl`, `tools`) -- installation info for
 *      each dependency, populated by check actions. `null` means "not yet checked".
 *   2. **Operation flags** (`isChecking`, `isInstalling`, `installingName`) --
 *      track whether a check or install is in progress, used by the UI to
 *      show spinners and disable buttons.
 *   3. **Error state** (`error`) -- the last failure message.
 *
 * Actions are divided into:
 *   - **Check actions**: Query the backend for current installation status.
 *   - **Install actions**: Trigger downloads/installations and refresh status.
 *   - **Computed getter**: `isReady()` derives whether the app is functional.
 */
interface DependencyState {
  // ---------------------------------------------------------------------------
  // Status fields -- populated by check actions
  // ---------------------------------------------------------------------------

  /**
   * Installation status of the portable Python runtime.
   * `null` until the first `checkPython()` or `checkAll()` call completes.
   * When populated, contains: `{ name, required, installed, version, path }`.
   */
  python: DependencyStatus | null;

  /**
   * Installation status of the GAMDL Python package.
   * `null` until the first `checkGamdl()` or `checkAll()` call completes.
   * GAMDL is the core CLI tool that performs Apple Music downloads.
   */
  gamdl: DependencyStatus | null;

  /**
   * Array of installation statuses for external tools (FFmpeg, mp4decrypt,
   * MP4Box, N_m3u8DL-RE, amdecrypt). Each has `required: true/false` to
   * distinguish mandatory tools from optional ones.
   */
  tools: DependencyStatus[];

  // ---------------------------------------------------------------------------
  // Operation tracking flags
  // ---------------------------------------------------------------------------

  /**
   * `true` while any `checkAll/checkPython/checkGamdl` is awaiting the backend.
   * The `<SetupWizard>` shows a scanning animation while this is set.
   */
  isChecking: boolean;

  /**
   * `true` while any `installPython/installGamdl/installTool` is in progress.
   * Used to disable install buttons and show a progress indicator.
   */
  isInstalling: boolean;

  /**
   * Human-readable name of the component currently being installed
   * (e.g., 'Python', 'GAMDL', 'FFmpeg'). `null` when no installation is active.
   * Displayed in the UI next to the progress spinner.
   */
  installingName: string | null;

  /**
   * Error message from the last failed check or install operation.
   * `null` when there is no error.
   */
  error: string | null;

  // ---------------------------------------------------------------------------
  // Check actions -- query the Rust backend for current installation status
  // ---------------------------------------------------------------------------

  /**
   * Check all dependencies in parallel: Python, GAMDL, and external tools.
   * Uses `Promise.all()` for concurrent execution, minimizing total wait time.
   * IPC calls:
   *   - `commands.checkPythonStatus()` -> Rust `check_python_status`
   *   - `commands.checkGamdlStatus()` -> Rust `check_gamdl_status`
   *   - `commands.checkAllDependencies()` -> Rust `check_all_dependencies`
   */
  checkAll: () => Promise<void>;

  /**
   * Check only the Python runtime status.
   * IPC call: `commands.checkPythonStatus()` -> Rust `check_python_status`
   * Used by the setup wizard's Python step to refresh status after install.
   */
  checkPython: () => Promise<void>;

  /**
   * Check only the GAMDL package status.
   * IPC call: `commands.checkGamdlStatus()` -> Rust `check_gamdl_status`
   * Used by the setup wizard's GAMDL step to refresh status after install.
   */
  checkGamdl: () => Promise<void>;

  // ---------------------------------------------------------------------------
  // Install actions -- trigger installation and refresh status afterward
  // ---------------------------------------------------------------------------

  /**
   * Download and install the portable Python runtime.
   * IPC call: `commands.installPython()` -> Rust `install_python`
   * After installation, automatically re-checks Python status.
   * @returns The installed Python version string (e.g., '3.12.1')
   * @throws If the installation fails
   */
  installPython: () => Promise<string>;

  /**
   * Install GAMDL via pip into the portable Python environment.
   * IPC call: `commands.installGamdl()` -> Rust `install_gamdl`
   * After installation, automatically re-checks GAMDL status.
   * @returns The installed GAMDL version string
   * @throws If the installation fails
   */
  installGamdl: () => Promise<string>;

  /**
   * Install a specific external tool by name (e.g., 'ffmpeg', 'mp4decrypt').
   * IPC call: `commands.installDependency(name)` -> Rust `install_dependency`
   * After installation, re-checks ALL tool statuses to refresh the full list.
   * @param name -- The tool name matching the backend's dependency registry
   * @returns The installed version string
   * @throws If the installation fails
   */
  installTool: (name: string) => Promise<string>;

  // ---------------------------------------------------------------------------
  // Computed getter
  // ---------------------------------------------------------------------------

  /**
   * Returns `true` if the minimum required dependencies (Python + GAMDL)
   * are both installed. This is the gate that determines whether the
   * download functionality is available.
   *
   * Note: This is a synchronous getter, not a reactive selector. It reads
   * from `get()` at call time. Components should call it inside a selector
   * or re-derive after `checkAll()` completes.
   */
  isReady: () => boolean;
}

/**
 * Zustand store hook for dependency checking and installation state.
 *
 * Usage in components:
 *   const python = useDependencyStore((s) => s.python);
 *   const { checkAll, installPython } = useDependencyStore();
 *   const ready = useDependencyStore((s) => s.isReady());
 *
 * The store creator receives `set` (for state updates) and `get` (for reading
 * current state inside sync getters like `isReady()`).
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useDependencyStore = create<DependencyState>((set, get) => ({
  // -------------------------------------------------------------------------
  // Initial state -- null statuses indicate "not yet checked"
  // -------------------------------------------------------------------------
  python: null,           // Python status unknown until first check
  gamdl: null,            // GAMDL status unknown until first check
  tools: [],              // No tool statuses until first check
  isChecking: false,      // No check in progress
  isInstalling: false,    // No installation in progress
  installingName: null,   // No component being installed
  error: null,            // No error

  // -------------------------------------------------------------------------
  // Check actions
  // -------------------------------------------------------------------------

  /**
   * Check all dependencies concurrently using `Promise.all()`.
   * This fires three IPC calls in parallel to minimize total latency:
   *   1. `check_python_status` -- checks for portable Python binary
   *   2. `check_gamdl_status` -- checks for GAMDL pip package
   *   3. `check_all_dependencies` -- checks all external tools (FFmpeg, etc.)
   *
   * All three results are applied atomically in a single `set()` call,
   * ensuring the UI sees a consistent snapshot of all dependency statuses.
   */
  checkAll: async () => {
    // Signal checking in progress and clear any stale error.
    set({ isChecking: true, error: null });
    try {
      // Fire all three IPC calls concurrently for faster results.
      const [python, gamdl, tools] = await Promise.all([
        commands.checkPythonStatus(),
        commands.checkGamdlStatus(),
        commands.checkAllDependencies(),
      ]);
      // Apply all results atomically and clear the checking flag.
      set({ python, gamdl, tools, isChecking: false });
    } catch (e) {
      set({ error: String(e), isChecking: false });
    }
  },

  /**
   * Check only the Python runtime status.
   * Lighter-weight than `checkAll()` -- used after Python installation
   * to confirm it succeeded without re-checking every other dependency.
   */
  checkPython: async () => {
    try {
      const python = await commands.checkPythonStatus();
      set({ python });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  /**
   * Check only the GAMDL package status.
   * Used after GAMDL installation to confirm it succeeded.
   */
  checkGamdl: async () => {
    try {
      const gamdl = await commands.checkGamdlStatus();
      set({ gamdl });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  // -------------------------------------------------------------------------
  // Install actions -- each follows the pattern:
  //   1. Set isInstalling + installingName + clear error
  //   2. Call the Rust install command
  //   3. Re-check the dependency status to confirm
  //   4. Clear isInstalling + installingName
  //   5. On failure: store error, clear flags, re-throw
  // -------------------------------------------------------------------------

  /**
   * Download and install the portable Python runtime.
   *
   * IPC call: `commands.installPython()` -> Rust `install_python`
   * The Rust handler downloads a standalone Python build appropriate for the
   * current platform (e.g., python-build-standalone from indygreg) and extracts
   * it to the app's data directory.
   *
   * After installation, `checkPythonStatus()` is called to verify the binary
   * is accessible and populate the `python` status field with version info.
   *
   * @returns The installed Python version string (e.g., '3.12.1')
   */
  installPython: async () => {
    // Signal that an installation is starting. The UI shows "Installing Python...".
    set({ isInstalling: true, installingName: 'Python', error: null });
    try {
      // Invoke the Rust `install_python` command, which downloads and extracts Python.
      const version = await commands.installPython();
      // Re-check Python status to populate version and path information.
      const python = await commands.checkPythonStatus();
      // Clear installation flags and update Python status atomically.
      set({ python, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      // Store the error for UI display and clear installation flags.
      set({ error: msg, isInstalling: false, installingName: null });
      // Re-throw so the calling component (e.g., SetupWizard) can handle the failure.
      throw new Error(msg);
    }
  },

  /**
   * Install GAMDL via pip into the portable Python environment.
   *
   * IPC call: `commands.installGamdl()` -> Rust `install_gamdl`
   * The Rust handler runs `python -m pip install gamdl` using the portable
   * Python runtime. This installs GAMDL and all its Python dependencies.
   *
   * After installation, `checkGamdlStatus()` is called to verify the package
   * is importable and populate the `gamdl` status field.
   *
   * @returns The installed GAMDL version string
   */
  installGamdl: async () => {
    set({ isInstalling: true, installingName: 'GAMDL', error: null });
    try {
      const version = await commands.installGamdl();
      // Re-check GAMDL status to confirm the installation succeeded.
      const gamdl = await commands.checkGamdlStatus();
      set({ gamdl, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isInstalling: false, installingName: null });
      throw new Error(msg);
    }
  },

  /**
   * Install a specific external tool (e.g., 'ffmpeg', 'mp4decrypt').
   *
   * IPC call: `commands.installDependency(name)` -> Rust `install_dependency`
   * The Rust handler downloads the platform-appropriate binary and places it
   * in the app's tools directory.
   *
   * After installation, `checkAllDependencies()` is called to refresh the
   * entire tools list, since tool installations may affect other tools'
   * detection (e.g., MP4Box might be bundled with GPAC).
   *
   * @param name -- The tool identifier matching the backend's registry
   * @returns The installed version string
   */
  installTool: async (name: string) => {
    set({ isInstalling: true, installingName: name, error: null });
    try {
      const version = await commands.installDependency(name);
      // Re-check ALL tools (not just the installed one) because some tools
      // may be co-bundled or have inter-dependencies.
      const tools = await commands.checkAllDependencies();
      set({ tools, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isInstalling: false, installingName: null });
      throw new Error(msg);
    }
  },

  // -------------------------------------------------------------------------
  // Computed getter
  // -------------------------------------------------------------------------

  /**
   * Determines whether the minimum required dependencies are installed.
   *
   * Uses `get()` to read the current state synchronously. Returns `true`
   * only when both `python.installed` and `gamdl.installed` are truthy.
   *
   * The double-bang `!!` ensures the return type is `boolean` even if
   * `python` or `gamdl` is `null` (short-circuits to `false`).
   *
   * This is the primary gate used by `<App>` to decide whether to show
   * the setup wizard or the main download interface.
   */
  isReady: () => {
    const { python, gamdl } = get();
    return !!(python?.installed && gamdl?.installed);
  },
}));
