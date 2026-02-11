/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file SetupWizard.tsx -- Multi-step setup wizard container component.
 *
 * This component manages the full first-run setup flow for the application.
 * It renders a full-screen wizard overlay with three main areas:
 *
 *   1. **Progress indicator bar** (top) -- A horizontal sequence of numbered
 *      circles connected by lines, showing which step the user is on.
 *   2. **Step content area** (middle) -- Renders the active step component.
 *   3. **Navigation buttons** (bottom) -- "Back" and "Continue"/"Get Started"
 *      buttons for moving between steps.
 *
 * ## Step Progression (State Machine)
 *
 * The wizard follows a linear 6-step progression:
 *
 *   Welcome -> Python -> GAMDL -> Dependencies -> Cookies -> Complete
 *
 * The current step is tracked by the `setupStore` (Zustand), which exposes:
 *   - `currentStep: SetupStep` -- The active step identifier
 *   - `currentStepIndex: number` -- Zero-based index into the SETUP_STEPS array
 *   - `completedSteps: Set<SetupStep>` -- Which steps have been completed
 *   - `nextStep()` / `prevStep()` -- Navigation actions
 *   - `completeStep(step)` -- Marks a step as done (enables the "Continue" button)
 *   - `finishSetup()` -- Marks the entire wizard as complete
 *
 * ## Completion Detection
 *
 * Each step component is responsible for calling `completeStep(stepName)` when
 * its requirements are met. For example:
 *   - WelcomeStep: Auto-completes on mount (no user action required).
 *   - PythonStep: Completes when Python is detected as installed.
 *   - DependenciesStep: Completes when all required tools are installed.
 *   - CookiesStep: Completes when cookies are validated OR when the user
 *     clicks "Skip for Now".
 *   - CompleteStep: Auto-completes on mount.
 *
 * The "Continue" button is disabled (`!canProceed`) until the current step
 * is marked as completed. On the last step, the button changes to "Get Started"
 * and calls `handleFinish()` instead of `nextStep()`.
 *
 * ## Store Connections
 *
 * - **setupStore** (Zustand): Manages step state, navigation, and completion.
 *   Exports the ordered SETUP_STEPS array.
 * - **uiStore** (Zustand): `setShowSetupWizard(false)` hides the wizard
 *   overlay when the user finishes.
 *
 * @see {@link @/stores/setupStore.ts}     -- Zustand store for wizard state
 * @see {@link @/stores/uiStore.ts}        -- Controls wizard visibility
 * @see {@link https://react.dev/}         -- React documentation
 * @see {@link https://v2.tauri.app/}      -- Tauri 2.0 framework
 */

// Zustand stores: setupStore manages the wizard state machine;
// SETUP_STEPS is the ordered array of step identifiers.
import { useSetupStore, SETUP_STEPS } from '@/stores/setupStore';

// uiStore provides setShowSetupWizard to dismiss the wizard overlay.
import { useUiStore } from '@/stores/uiStore';

// Shared Button component for the navigation bar.
import { Button } from '@/components/common';

// Individual step components -- each handles its own installation/validation
// logic and calls completeStep() when its requirements are satisfied.
import { WelcomeStep } from './steps/WelcomeStep';
import { PythonStep } from './steps/PythonStep';
import { GamdlStep } from './steps/GamdlStep';
import { DependenciesStep } from './steps/DependenciesStep';
import { CookiesStep } from './steps/CookiesStep';
import { CompleteStep } from './steps/CompleteStep';

// TypeScript union type for step identifiers.
import type { SetupStep } from '@/types';

/**
 * Maps each step identifier to its React component implementation.
 * Used for dynamic component rendering: `STEP_COMPONENTS[currentStep]`
 * resolves to the component that should be displayed in the content area.
 */
const STEP_COMPONENTS: Record<SetupStep, React.FC> = {
  welcome: WelcomeStep,
  python: PythonStep,
  gamdl: GamdlStep,
  dependencies: DependenciesStep,
  cookies: CookiesStep,
  complete: CompleteStep,
};

/**
 * Human-readable labels for each step, displayed beneath the numbered
 * circles in the progress indicator bar. These are intentionally short
 * to fit within the compact circular layout.
 */
const STEP_LABELS: Record<SetupStep, string> = {
  welcome: 'Welcome',
  python: 'Python',
  gamdl: 'GAMDL',
  dependencies: 'Tools',
  cookies: 'Cookies',
  complete: 'Done',
};

/**
 * SetupWizard -- Full-screen setup wizard component.
 *
 * Renders a full-height layout divided into three sections:
 *   - Progress indicator bar at the top
 *   - Scrollable step content in the middle
 *   - Navigation buttons at the bottom
 *
 * The wizard takes over the entire viewport (`h-screen`) and uses
 * `bg-surface-primary` to match the application's theme.
 */
export function SetupWizard() {
  // --- Zustand setupStore selectors ---
  /** The identifier of the currently active wizard step */
  const currentStep = useSetupStore((s) => s.currentStep);
  /** Zero-based index of the current step within SETUP_STEPS */
  const currentStepIndex = useSetupStore((s) => s.currentStepIndex);
  /** Set of step identifiers that have been marked as completed */
  const completedSteps = useSetupStore((s) => s.completedSteps);
  /** Advances to the next step in the linear progression */
  const nextStep = useSetupStore((s) => s.nextStep);
  /** Returns to the previous step */
  const prevStep = useSetupStore((s) => s.prevStep);
  /** Marks the entire setup as complete (persisted to disk) */
  const finishSetup = useSetupStore((s) => s.finishSetup);

  // --- Zustand uiStore selectors ---
  /** Hides the wizard overlay by setting showSetupWizard to false */
  const setShowSetupWizard = useUiStore((s) => s.setShowSetupWizard);

  /**
   * Resolve the React component for the current step via the static
   * STEP_COMPONENTS lookup map. This enables dynamic rendering without
   * a switch statement.
   */
  const StepComponent = STEP_COMPONENTS[currentStep];

  /**
   * Derived navigation flags:
   * - canProceed: true when the current step is in the completedSteps set,
   *   which enables the "Continue" button.
   * - isFirstStep: hides the "Back" button on the Welcome step.
   * - isLastStep: changes "Continue" to "Get Started" and wires it to
   *   handleFinish instead of nextStep.
   */
  const canProceed = completedSteps.has(currentStep);
  const isFirstStep = currentStepIndex === 0;
  const isLastStep = currentStepIndex === SETUP_STEPS.length - 1;

  /**
   * Finishes the setup wizard by:
   * 1. Calling `finishSetup()` on the setupStore, which persists the
   *    completion flag so the wizard won't appear on the next launch.
   * 2. Calling `setShowSetupWizard(false)` on the uiStore, which
   *    removes the full-screen overlay and reveals the main application.
   */
  const handleFinish = () => {
    finishSetup();
    setShowSetupWizard(false);
  };

  return (
    <div className="flex flex-col h-screen bg-surface-primary">
      {/* ================================================================
          Progress indicator bar
          Displays a horizontal sequence of step circles connected by
          lines. Each circle shows a number (for future/current steps)
          or a checkmark (for completed/past steps).

          Visual states for each step circle:
          - Current step: accent background, white text
          - Completed/past step: green success background, checkmark
          - Future step: elevated surface, tertiary text, light border

          The connecting line between circles turns green once the
          preceding step has been passed.
          ================================================================ */}
      <div className="px-8 pt-8 pb-4">
        <div className="flex items-center justify-between max-w-2xl mx-auto">
          {SETUP_STEPS.map((step, index) => {
            // Determine the visual state of this step circle
            const isCompleted = completedSteps.has(step); // Step requirements are satisfied
            const isCurrent = index === currentStepIndex;  // This is the active step
            const isPast = index < currentStepIndex;       // User has moved past this step

            return (
              <div key={step} className="flex items-center flex-1 last:flex-initial">
                {/* Step circle and label column */}
                <div className="flex flex-col items-center">
                  {/* Numbered/checkmark circle */}
                  <div
                    className={`
                      w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium
                      transition-colors
                      ${
                        isCurrent
                          ? 'bg-accent text-content-inverse'                                       /* Active step: accent colour */
                          : isCompleted || isPast
                            ? 'bg-status-success text-white'                                       /* Done/past step: green */
                            : 'bg-surface-elevated text-content-tertiary border border-border-light' /* Future step: muted */
                      }
                    `}
                  >
                    {/* Show checkmark (Unicode \u2713) for completed/past, number for others */}
                    {isCompleted || isPast ? '\u2713' : index + 1}
                  </div>
                  {/* Step label text beneath the circle */}
                  <span
                    className={`
                      text-[11px] mt-1.5
                      ${isCurrent ? 'text-accent font-medium' : 'text-content-tertiary'}
                    `}
                  >
                    {STEP_LABELS[step]}
                  </span>
                </div>

                {/* Connecting line between step circles.
                    Not rendered after the last step (no successor to connect to).
                    The negative top margin (`mt-[-20px]`) vertically aligns the
                    line with the center of the circle, compensating for the
                    label text below. */}
                {index < SETUP_STEPS.length - 1 && (
                  <div
                    className={`
                      flex-1 h-0.5 mx-2 mt-[-20px]
                      ${isPast ? 'bg-status-success' : 'bg-border-light'}
                    `}
                  />
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Step content */}
      <div className="flex-1 overflow-y-auto px-8">
        <div className="max-w-2xl mx-auto py-6">
          <StepComponent />
        </div>
      </div>

      {/* Navigation buttons */}
      <div className="px-8 py-4 border-t border-border-light">
        <div className="max-w-2xl mx-auto flex justify-between">
          {/* Back button (hidden on first step) */}
          {!isFirstStep ? (
            <Button variant="ghost" onClick={prevStep}>
              Back
            </Button>
          ) : (
            <div />
          )}

          {/* Next / Finish button */}
          {isLastStep ? (
            <Button variant="primary" onClick={handleFinish}>
              Get Started
            </Button>
          ) : (
            <Button
              variant="primary"
              onClick={nextStep}
              disabled={!canProceed}
            >
              Continue
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
