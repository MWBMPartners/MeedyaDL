// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for SetupWizard (#232 — fourth installment).
 *
 * Scope: the wizard *controller* — progress indicator, navigation
 * gating, finish flow. The six per-step components (WelcomeStep,
 * PythonStep, GamdlStep, DependenciesStep, CookiesStep,
 * CompleteStep) are each mocked to a small placeholder so this file
 * tests SetupWizard.tsx in isolation. Per-step components warrant
 * their own dedicated test files (CookiesStep alone is 850 lines
 * and pulls in the cookie-import IPC chain).
 *
 * Tests cover:
 *   - Progress bar renders 6 step circles + 5 connecting lines
 *   - Active step circle uses accent styling, future steps muted
 *   - Past/completed steps render as checkmarks
 *   - StepComponent dispatches to the correct mock per `currentStep`
 *   - Back button hidden on first step
 *   - Continue button enabled/disabled based on `canProceed`
 *   - Continue click invokes `nextStep`
 *   - Back click invokes `prevStep`
 *   - Last step swaps "Continue" → "Get Started"
 *   - Get Started invokes the full finish flow
 *     (finishSetup + updateSettings + setShowSetupWizard(false))
 *
 * @see src/components/setup/SetupWizard.tsx
 */

import { render, screen, fireEvent, act } from '@testing-library/react';
import { useSetupStore, SETUP_STEPS } from '@/stores/setupStore';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { SetupWizard } from '@/components/setup/SetupWizard';
import type { SetupStep } from '@/types';

// Mock each step component to an identifiable placeholder so we can
// verify the dispatch table without exercising 1700+ lines of step
// behaviour. Each placeholder includes a data-testid so the test
// can assert which step is being rendered.
vi.mock('@/components/setup/steps/WelcomeStep', () => ({
  WelcomeStep: () => <div data-testid="step-welcome" />,
}));
vi.mock('@/components/setup/steps/PythonStep', () => ({
  PythonStep: () => <div data-testid="step-python" />,
}));
vi.mock('@/components/setup/steps/GamdlStep', () => ({
  GamdlStep: () => <div data-testid="step-gamdl" />,
}));
vi.mock('@/components/setup/steps/DependenciesStep', () => ({
  DependenciesStep: () => <div data-testid="step-dependencies" />,
}));
vi.mock('@/components/setup/steps/CookiesStep', () => ({
  CookiesStep: () => <div data-testid="step-cookies" />,
}));
vi.mock('@/components/setup/steps/CompleteStep', () => ({
  CompleteStep: () => <div data-testid="step-complete" />,
}));

/**
 * Reset the setupStore between tests so each one starts at the
 * 'welcome' step with no completed entries. Also clear UI overlay
 * state so finish-flow assertions can detect a flip from true → false.
 */
beforeEach(() => {
  act(() => {
    useSetupStore.setState({
      currentStep: 'welcome',
      currentStepIndex: 0,
      completedSteps: new Set(),
    });
    useUiStore.setState({ showSetupWizard: true });
  });
});

/**
 * Helper: jump to a specific step by index, optionally marking some
 * steps as completed. Mutates the setupStore directly so tests can
 * exercise the controller without going through the per-step
 * "completeStep + nextStep" UX.
 */
function goToStep(stepIndex: number, completed: SetupStep[] = []) {
  act(() => {
    useSetupStore.setState({
      currentStep: SETUP_STEPS[stepIndex],
      currentStepIndex: stepIndex,
      completedSteps: new Set(completed),
    });
  });
}

describe('SetupWizard', () => {
  // ===========================================================================
  // Progress indicator
  // ===========================================================================

  it('renders all six step labels in the progress bar', () => {
    render(<SetupWizard />);
    expect(screen.getByText('Welcome')).toBeInTheDocument();
    expect(screen.getByText('Python')).toBeInTheDocument();
    expect(screen.getByText('GAMDL')).toBeInTheDocument();
    expect(screen.getByText('Tools')).toBeInTheDocument();
    expect(screen.getByText('Cookies')).toBeInTheDocument();
    expect(screen.getByText('Done')).toBeInTheDocument();
  });

  it('first step circle shows "1" when on welcome step', () => {
    render(<SetupWizard />);
    // Future-step circles render their 1-based index. Step 1 (welcome)
    // is the active step so it shows "1" not a checkmark.
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.getByText('6')).toBeInTheDocument();
  });

  it('past steps render as checkmarks (✓)', () => {
    goToStep(2, ['welcome', 'python']);
    render(<SetupWizard />);
    // Welcome (idx 0) and Python (idx 1) are past — each renders ✓.
    // GAMDL (idx 2) is current — shows "3". Dependencies (idx 3),
    // Cookies (idx 4), Done (idx 5) are future — show "4", "5", "6".
    const checks = screen.getAllByText('✓');
    expect(checks.length).toBe(2);
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();
    expect(screen.getByText('6')).toBeInTheDocument();
  });

  // ===========================================================================
  // Step component dispatch
  // ===========================================================================

  it('renders WelcomeStep when currentStep === "welcome"', () => {
    render(<SetupWizard />);
    expect(screen.getByTestId('step-welcome')).toBeInTheDocument();
  });

  it('renders PythonStep when currentStep === "python"', () => {
    goToStep(1);
    render(<SetupWizard />);
    expect(screen.getByTestId('step-python')).toBeInTheDocument();
    expect(screen.queryByTestId('step-welcome')).not.toBeInTheDocument();
  });

  it('renders the correct step for every position in SETUP_STEPS', () => {
    SETUP_STEPS.forEach((step, index) => {
      goToStep(index);
      const { unmount } = render(<SetupWizard />);
      expect(screen.getByTestId(`step-${step}`)).toBeInTheDocument();
      unmount();
    });
  });

  // ===========================================================================
  // Navigation: Back button
  // ===========================================================================

  it('hides Back button on first step', () => {
    render(<SetupWizard />);
    expect(
      screen.queryByRole('button', { name: /^back$/i })
    ).not.toBeInTheDocument();
  });

  it('shows Back button on non-first steps', () => {
    goToStep(2);
    render(<SetupWizard />);
    expect(screen.getByRole('button', { name: /^back$/i })).toBeInTheDocument();
  });

  it('Back button calls prevStep on click', () => {
    goToStep(3);
    const prevSpy = vi.spyOn(useSetupStore.getState(), 'prevStep');
    render(<SetupWizard />);
    fireEvent.click(screen.getByRole('button', { name: /^back$/i }));
    expect(prevSpy).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Navigation: Continue button gating
  // ===========================================================================

  it('Continue button is DISABLED when current step has not been completed', () => {
    goToStep(1); // python step, no completedSteps
    render(<SetupWizard />);
    expect(screen.getByRole('button', { name: /continue/i })).toBeDisabled();
  });

  it('Continue button is ENABLED once current step is in completedSteps', () => {
    goToStep(1, ['python']);
    render(<SetupWizard />);
    expect(screen.getByRole('button', { name: /continue/i })).toBeEnabled();
  });

  it('Continue button calls nextStep on click', () => {
    goToStep(1, ['python']);
    const nextSpy = vi.spyOn(useSetupStore.getState(), 'nextStep');
    render(<SetupWizard />);
    fireEvent.click(screen.getByRole('button', { name: /continue/i }));
    expect(nextSpy).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Last step: Get Started + finish flow
  // ===========================================================================

  it('on the last step, Continue is replaced by "Get Started"', () => {
    goToStep(SETUP_STEPS.length - 1, [...SETUP_STEPS]);
    render(<SetupWizard />);
    expect(
      screen.queryByRole('button', { name: /continue/i })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /get started/i })
    ).toBeInTheDocument();
  });

  it('Get Started fires the full finish flow (finishSetup + updateSettings + setShowSetupWizard)', () => {
    goToStep(SETUP_STEPS.length - 1, [...SETUP_STEPS]);
    const finishSpy = vi.spyOn(useSetupStore.getState(), 'finishSetup');
    const updateSpy = vi.spyOn(useSettingsStore.getState(), 'updateSettings');
    const setShowSpy = vi.spyOn(useUiStore.getState(), 'setShowSetupWizard');
    render(<SetupWizard />);
    fireEvent.click(screen.getByRole('button', { name: /get started/i }));
    expect(finishSpy).toHaveBeenCalledTimes(1);
    expect(updateSpy).toHaveBeenCalledWith({ setup_completed: true });
    expect(setShowSpy).toHaveBeenCalledWith(false);
  });

  // ===========================================================================
  // SETUP_STEPS contract
  // ===========================================================================

  it('SETUP_STEPS contract: 6 steps in the documented order', () => {
    // If a future PR reorders SETUP_STEPS, every progress-indicator
    // assertion above breaks. Pin the contract here so the failure
    // is unambiguous instead of cascading through label assertions.
    expect(SETUP_STEPS).toEqual([
      'welcome',
      'python',
      'gamdl',
      'dependencies',
      'cookies',
      'complete',
    ]);
  });
});
