/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Setup wizard state store.
 * Manages the first-run setup wizard flow: tracks the current step,
 * completion status of each step, and whether setup has been completed.
 */

import { create } from 'zustand';
import type { SetupStep } from '@/types';

/** Ordered list of setup wizard steps */
export const SETUP_STEPS: SetupStep[] = [
  'welcome',
  'python',
  'gamdl',
  'dependencies',
  'cookies',
  'complete',
];

interface SetupState {
  /** Current wizard step */
  currentStep: SetupStep;
  /** Index of the current step (0-based) */
  currentStepIndex: number;
  /** Which steps have been completed */
  completedSteps: Set<SetupStep>;
  /** Whether the entire setup has been completed */
  isComplete: boolean;
  /** Error from the current step */
  stepError: string | null;

  /** Move to the next step */
  nextStep: () => void;
  /** Move to the previous step */
  prevStep: () => void;
  /** Jump to a specific step */
  goToStep: (step: SetupStep) => void;
  /** Mark a step as completed */
  completeStep: (step: SetupStep) => void;
  /** Set an error for the current step */
  setStepError: (error: string | null) => void;
  /** Mark the entire setup as complete */
  finishSetup: () => void;
  /** Reset the wizard to the beginning */
  resetSetup: () => void;
}

export const useSetupStore = create<SetupState>((set) => ({
  currentStep: 'welcome',
  currentStepIndex: 0,
  completedSteps: new Set(),
  isComplete: false,
  stepError: null,

  nextStep: () =>
    set((state) => {
      const nextIndex = Math.min(
        state.currentStepIndex + 1,
        SETUP_STEPS.length - 1,
      );
      return {
        currentStep: SETUP_STEPS[nextIndex],
        currentStepIndex: nextIndex,
        stepError: null,
      };
    }),

  prevStep: () =>
    set((state) => {
      const prevIndex = Math.max(state.currentStepIndex - 1, 0);
      return {
        currentStep: SETUP_STEPS[prevIndex],
        currentStepIndex: prevIndex,
        stepError: null,
      };
    }),

  goToStep: (step) => {
    const index = SETUP_STEPS.indexOf(step);
    if (index >= 0) {
      set({ currentStep: step, currentStepIndex: index, stepError: null });
    }
  },

  completeStep: (step) =>
    set((state) => {
      const completed = new Set(state.completedSteps);
      completed.add(step);
      return { completedSteps: completed };
    }),

  setStepError: (error) => set({ stepError: error }),

  finishSetup: () => set({ isComplete: true }),

  resetSetup: () =>
    set({
      currentStep: 'welcome',
      currentStepIndex: 0,
      completedSteps: new Set(),
      isComplete: false,
      stepError: null,
    }),
}));
