// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file setupStore.ts -- Setup Wizard State Machine Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Manages the first-run setup wizard as a **linear state machine** with the
 * following step progression:
 *
 *   ```
 *   welcome -> python -> gamdl -> dependencies -> cookies -> complete
 *     [0]       [1]      [2]        [3]           [4]        [5]
 *   ```
 *
 * **Step semantics**:
 *   - `welcome`      -- Informational splash; no completion condition.
 *   - `python`       -- User installs the portable Python runtime. Step is
 *                        marked complete when `dependencyStore.python.installed`
 *                        becomes `true`.
 *   - `gamdl`        -- User installs the GAMDL package. Marked complete when
 *                        `dependencyStore.gamdl.installed` is `true`.
 *   - `dependencies` -- User installs external tools (FFmpeg, mp4decrypt, etc.).
 *                        Marked complete when all `required` tools are installed.
 *   - `cookies`      -- User provides an Apple Music cookies file. Marked
 *                        complete when `settingsStore.settings.cookies_path` is
 *                        non-null and validates successfully.
 *   - `complete`     -- Summary/confirmation screen; user clicks "Finish" to
 *                        call `finishSetup()` which sets `isComplete = true`.
 *
 * **Navigation model**:
 *   - `nextStep()` / `prevStep()` move linearly through the `SETUP_STEPS` array.
 *   - `goToStep()` allows jumping to any step (used when clicking step indicators).
 *   - Step indices are clamped to `[0, SETUP_STEPS.length - 1]` to prevent overflow.
 *
 * **Completion tracking**:
 *   - `completedSteps` is a `Set<SetupStep>` that tracks which steps the user
 *     has successfully completed. Each step component calls `completeStep()`
 *     when its completion condition is met.
 *   - `isComplete` is a separate flag set only when the user finishes the entire
 *     wizard. `<App>` reads this to decide whether to hide the wizard overlay.
 *
 * Consumed by: `<SetupWizard>`, `<SetupStepIndicator>`, `<App>`.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- Zustand state updates
 */

// Zustand store factory. Returns a React hook with built-in subscription.
import { create } from 'zustand';

// SetupStep -- union literal type: 'welcome' | 'python' | 'gamdl' | 'dependencies' | 'cookies' | 'complete'
import type { SetupStep } from '@/types';

/**
 * Ordered list of setup wizard steps.
 *
 * This array defines the canonical step sequence. The array index of each step
 * is used as `currentStepIndex` for efficient next/prev navigation.
 *
 * Exported so that other components (e.g., `<SetupStepIndicator>`) can render
 * a progress bar showing all steps and the user's current position.
 *
 * The order matters: Python must be installed before GAMDL (since GAMDL is a
 * pip package), and both must exist before external tools make sense.
 */
export const SETUP_STEPS: SetupStep[] = [
  'welcome',      // Step 0: Informational welcome splash
  'python',       // Step 1: Install portable Python runtime
  'gamdl',        // Step 2: Install GAMDL pip package (requires Python)
  'dependencies', // Step 3: Install external tools (FFmpeg, mp4decrypt, etc.)
  'cookies',      // Step 4: Provide Apple Music cookies file for authentication
  'complete',     // Step 5: Setup complete -- summary and "Finish" button
];

/**
 * Combined state + actions interface for the setup wizard store.
 *
 * This store models a linear state machine where the user progresses through
 * a fixed sequence of steps. The combination of `currentStep` (the step name)
 * and `currentStepIndex` (its array position) allows both human-readable
 * checks (`if (step === 'python')`) and efficient arithmetic navigation
 * (`nextIndex = currentStepIndex + 1`).
 *
 * The `completedSteps` Set allows non-linear completion: a step can be
 * completed without being the current step (e.g., if the user installs
 * Python from the dependencies step, the python step is retroactively
 * marked complete).
 */
interface SetupState {
  // ---------------------------------------------------------------------------
  // State fields
  // ---------------------------------------------------------------------------

  /**
   * The step name currently displayed in the wizard.
   * One of: 'welcome', 'python', 'gamdl', 'dependencies', 'cookies', 'complete'.
   * Used by `<SetupWizard>` to conditionally render the appropriate step component.
   */
  currentStep: SetupStep;

  /**
   * Zero-based array index of `currentStep` within `SETUP_STEPS`.
   * Maintained alongside `currentStep` for efficient next/prev arithmetic.
   * Invariant: `SETUP_STEPS[currentStepIndex] === currentStep`.
   */
  currentStepIndex: number;

  /**
   * Set of step names that have been successfully completed.
   * Used by `<SetupStepIndicator>` to show checkmarks on completed steps
   * and by individual step components to decide whether to allow advancement.
   *
   * Note: Uses ES6 `Set` rather than an array for O(1) `has()` lookups.
   * Zustand detects changes because `completeStep()` creates a new Set instance.
   */
  completedSteps: Set<SetupStep>;

  /**
   * `true` when the user has clicked "Finish" on the final step.
   * `<App>` reads this flag to hide the setup wizard overlay and reveal
   * the main application UI. This flag is not persisted -- the app relies
   * on `dependencyStore.isReady()` to determine if setup is needed on
   * subsequent launches.
   */
  isComplete: boolean;

  /**
   * Error message specific to the current wizard step.
   * Displayed by the step component's error banner. Cleared automatically
   * when the user navigates away from the step (`nextStep/prevStep/goToStep`).
   */
  stepError: string | null;

  // ---------------------------------------------------------------------------
  // Navigation actions
  // ---------------------------------------------------------------------------

  /**
   * Advance to the next step in the sequence.
   * Uses `Math.min()` to clamp the index, preventing overflow past the
   * last step ('complete'). Clears `stepError` on navigation.
   */
  nextStep: () => void;

  /**
   * Go back to the previous step in the sequence.
   * Uses `Math.max()` to clamp the index, preventing underflow below step 0
   * ('welcome'). Clears `stepError` on navigation.
   */
  prevStep: () => void;

  /**
   * Jump directly to a specific step by name.
   * Looks up the step's index in `SETUP_STEPS`; does nothing if the step
   * name is not found (defensive guard). Clears `stepError` on navigation.
   * Used by clickable step indicators in the wizard header.
   * @param step -- The target step name
   */
  goToStep: (step: SetupStep) => void;

  // ---------------------------------------------------------------------------
  // Completion actions
  // ---------------------------------------------------------------------------

  /**
   * Mark a specific step as completed.
   * Creates a new `Set` instance (cloned from the previous set, plus the
   * new step) to ensure Zustand detects the reference change.
   *
   * Called by individual step components when their completion condition is met
   * (e.g., the Python step calls `completeStep('python')` after a successful
   * Python installation).
   *
   * @param step -- The step to mark as completed
   */
  completeStep: (step: SetupStep) => void;

  /**
   * Set or clear the error message for the current step.
   * Pass `null` to clear the error (e.g., after a retry succeeds).
   * @param error -- The error message string, or `null` to clear
   */
  setStepError: (error: string | null) => void;

  /**
   * Mark the entire setup wizard as complete.
   * Sets `isComplete = true`, which `<App>` reads to dismiss the wizard
   * overlay and transition to the main application interface.
   */
  finishSetup: () => void;

  /**
   * Reset the entire wizard state to its initial values.
   * Used if the user needs to re-run setup (e.g., after a factory reset
   * of dependencies). Creates fresh `Set` and resets all fields.
   */
  resetSetup: () => void;
}

/**
 * Zustand store hook for the setup wizard state machine.
 *
 * Usage in components:
 *   const step = useSetupStore((s) => s.currentStep);
 *   const { nextStep, completeStep } = useSetupStore();
 *
 * This store only uses `set` (no `get`) because all actions derive state
 * from the updater-function form `set((prev) => next)` or set simple values.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useSetupStore = create<SetupState>((set) => ({
  // -------------------------------------------------------------------------
  // Initial state -- wizard starts at the welcome step
  // -------------------------------------------------------------------------
  currentStep: 'welcome',       // Begin at the welcome splash screen
  currentStepIndex: 0,          // Index 0 in the SETUP_STEPS array
  completedSteps: new Set(),    // No steps completed yet
  isComplete: false,            // Wizard not yet finished
  stepError: null,              // No error on the current step

  // -------------------------------------------------------------------------
  // Navigation actions
  // -------------------------------------------------------------------------

  /**
   * Move forward one step. Uses updater-function form of `set()` to read
   * the current index and compute the next one.
   *
   * `Math.min()` clamps the result to the last valid index, so calling
   * `nextStep()` on the 'complete' step is a no-op (stays on 'complete').
   *
   * `stepError` is cleared on navigation so stale errors from the previous
   * step do not carry over.
   */
  nextStep: () =>
    set((state) => {
      // Clamp to the last step index to prevent array out-of-bounds.
      const nextIndex = Math.min(
        state.currentStepIndex + 1,
        SETUP_STEPS.length - 1,
      );
      return {
        currentStep: SETUP_STEPS[nextIndex],  // Look up step name by index
        currentStepIndex: nextIndex,
        stepError: null, // Clear any error from the previous step
      };
    }),

  /**
   * Move backward one step. Uses `Math.max()` to clamp at index 0,
   * so calling `prevStep()` on 'welcome' is a no-op.
   */
  prevStep: () =>
    set((state) => {
      // Clamp to index 0 to prevent negative indices.
      const prevIndex = Math.max(state.currentStepIndex - 1, 0);
      return {
        currentStep: SETUP_STEPS[prevIndex],  // Look up step name by index
        currentStepIndex: prevIndex,
        stepError: null, // Clear any error from the current step
      };
    }),

  /**
   * Jump to a specific step by name. Performs a linear search in
   * `SETUP_STEPS` to find the index. If the step name is not found
   * (index === -1), the state is not updated (defensive guard).
   */
  goToStep: (step) => {
    const index = SETUP_STEPS.indexOf(step);
    // Only update if the step exists in the ordered list.
    if (index >= 0) {
      set({ currentStep: step, currentStepIndex: index, stepError: null });
    }
  },

  // -------------------------------------------------------------------------
  // Completion actions
  // -------------------------------------------------------------------------

  /**
   * Mark a specific step as completed by adding it to the `completedSteps` Set.
   *
   * Creates a new `Set` instance from the previous one, then adds the step.
   * This ensures Zustand detects the change via reference inequality
   * (since `Set` is mutable, modifying the existing set in place would not
   * trigger a re-render).
   *
   * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- immutable updates
   */
  completeStep: (step) =>
    set((state) => {
      // Clone the existing Set to produce a new reference for Zustand.
      const completed = new Set(state.completedSteps);
      completed.add(step);
      return { completedSteps: completed };
    }),

  /** Set or clear the error message for the current wizard step. */
  setStepError: (error) => set({ stepError: error }),

  /**
   * Finalize the setup wizard. Sets `isComplete = true`, which is read by
   * `<App>` to dismiss the wizard overlay and reveal the main UI.
   */
  finishSetup: () => set({ isComplete: true }),

  /**
   * Reset the wizard to its initial state for a fresh run.
   * Creates a new empty `Set` and resets all scalar fields.
   * Useful if the user uninstalls dependencies and needs to re-run setup.
   */
  resetSetup: () =>
    set({
      currentStep: 'welcome',
      currentStepIndex: 0,
      completedSteps: new Set(), // Fresh empty Set (new reference)
      isComplete: false,
      stepError: null,
    }),
}));
