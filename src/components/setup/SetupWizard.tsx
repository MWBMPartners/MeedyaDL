/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Setup wizard container component.
 * Manages the full first-run setup flow: step navigation, progress indicator,
 * and rendering of the active step component. Steps are: Welcome, Python install,
 * GAMDL install, Dependencies, Cookie import, and Complete.
 */

import { useSetupStore, SETUP_STEPS } from '@/stores/setupStore';
import { useUiStore } from '@/stores/uiStore';
import { Button } from '@/components/common';
import { WelcomeStep } from './steps/WelcomeStep';
import { PythonStep } from './steps/PythonStep';
import { GamdlStep } from './steps/GamdlStep';
import { DependenciesStep } from './steps/DependenciesStep';
import { CookiesStep } from './steps/CookiesStep';
import { CompleteStep } from './steps/CompleteStep';
import type { SetupStep } from '@/types';

/** Map of step identifiers to their component implementations */
const STEP_COMPONENTS: Record<SetupStep, React.FC> = {
  welcome: WelcomeStep,
  python: PythonStep,
  gamdl: GamdlStep,
  dependencies: DependenciesStep,
  cookies: CookiesStep,
  complete: CompleteStep,
};

/** Human-readable labels for each step (shown in the progress indicator) */
const STEP_LABELS: Record<SetupStep, string> = {
  welcome: 'Welcome',
  python: 'Python',
  gamdl: 'GAMDL',
  dependencies: 'Tools',
  cookies: 'Cookies',
  complete: 'Done',
};

/**
 * Renders the full-screen setup wizard with a progress bar,
 * step content, and navigation buttons.
 */
export function SetupWizard() {
  const currentStep = useSetupStore((s) => s.currentStep);
  const currentStepIndex = useSetupStore((s) => s.currentStepIndex);
  const completedSteps = useSetupStore((s) => s.completedSteps);
  const nextStep = useSetupStore((s) => s.nextStep);
  const prevStep = useSetupStore((s) => s.prevStep);
  const finishSetup = useSetupStore((s) => s.finishSetup);
  const setShowSetupWizard = useUiStore((s) => s.setShowSetupWizard);

  /* Get the active step's component */
  const StepComponent = STEP_COMPONENTS[currentStep];

  /** Whether the current step has been completed (enables "Next") */
  const canProceed = completedSteps.has(currentStep);
  const isFirstStep = currentStepIndex === 0;
  const isLastStep = currentStepIndex === SETUP_STEPS.length - 1;

  /** Handle finishing the wizard */
  const handleFinish = () => {
    finishSetup();
    setShowSetupWizard(false);
  };

  return (
    <div className="flex flex-col h-screen bg-surface-primary">
      {/* Progress indicator bar */}
      <div className="px-8 pt-8 pb-4">
        <div className="flex items-center justify-between max-w-2xl mx-auto">
          {SETUP_STEPS.map((step, index) => {
            const isCompleted = completedSteps.has(step);
            const isCurrent = index === currentStepIndex;
            const isPast = index < currentStepIndex;

            return (
              <div key={step} className="flex items-center flex-1 last:flex-initial">
                {/* Step circle */}
                <div className="flex flex-col items-center">
                  <div
                    className={`
                      w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium
                      transition-colors
                      ${
                        isCurrent
                          ? 'bg-accent text-content-inverse'
                          : isCompleted || isPast
                            ? 'bg-status-success text-white'
                            : 'bg-surface-elevated text-content-tertiary border border-border-light'
                      }
                    `}
                  >
                    {isCompleted || isPast ? '\u2713' : index + 1}
                  </div>
                  <span
                    className={`
                      text-[11px] mt-1.5
                      ${isCurrent ? 'text-accent font-medium' : 'text-content-tertiary'}
                    `}
                  >
                    {STEP_LABELS[step]}
                  </span>
                </div>

                {/* Connecting line between steps */}
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
