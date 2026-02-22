/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CompleteStep.tsx -- Setup complete step of the setup wizard.
 *
 * Renders the final "Done" step within the {@link SetupWizard}. This step
 * serves as a confirmation screen indicating that all required components
 * have been installed and the app is ready to use.
 *
 * ## Auto-completion
 *
 * Like the {@link WelcomeStep}, this step auto-completes on mount since no
 * user action is required. This enables the "Get Started" button in the
 * wizard footer, which calls `handleFinish()` to dismiss the wizard.
 *
 * ## Store Connections
 *
 * - **setupStore**: `completeStep('complete')` for auto-completion.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link @/stores/setupStore.ts}         -- Zustand store for wizard state
 */

// React useEffect for auto-completing on mount.
import { useEffect } from 'react';

// CheckCircle icon used for the success indicator.
import { CheckCircle } from 'lucide-react';

// Zustand store for wizard state.
import { useSetupStore } from '@/stores/setupStore';

/**
 * CompleteStep -- Renders the setup completion screen.
 *
 * Layout:
 *   1. Large success icon (green rounded square with checkmark)
 *   2. "Setup Complete!" heading and subtitle
 *   3. Footer text prompting the user to click "Get Started"
 *
 * Auto-completes on mount to enable the "Get Started" button.
 */
export function CompleteStep() {
  /** Marks the 'complete' wizard step as done */
  const completeStep = useSetupStore((s) => s.completeStep);

  /** Auto-complete this step on mount (no action required from the user) */
  useEffect(() => {
    completeStep('complete');
  }, [completeStep]);

  return (
    <div className="text-center space-y-6">
      {/* Success icon */}
      <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-status-success">
        <CheckCircle size={40} className="text-white" />
      </div>

      {/* Heading */}
      <div>
        <h2 className="text-2xl font-bold text-content-primary">
          Setup Complete!
        </h2>
        <p className="text-base text-content-secondary mt-2">
          Everything is ready. You can now start downloading media.
        </p>
      </div>

      <p className="text-xs text-content-tertiary">
        Click "Get Started" below to begin using MeedyaDL
      </p>
    </div>
  );
}
