/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file setup/index.ts -- Barrel export for setup wizard components.
 *
 * Re-exports the top-level {@link SetupWizard} component so that other parts
 * of the application can import it from `@/components/setup` without reaching
 * into internal module paths. The individual step components (WelcomeStep,
 * PythonStep, GamdlStep, DependenciesStep, CookiesStep, CompleteStep) are
 * intentionally kept private to this module -- they are imported directly by
 * {@link SetupWizard} and are not part of the public API surface.
 *
 * The SetupWizard is rendered as a full-screen overlay by the root `<App>`
 * component when `uiStore.showSetupWizard` is true (typically on first-run
 * or when the user re-launches the wizard from settings).
 *
 * Usage:
 * ```ts
 * import { SetupWizard } from '@/components/setup';
 * ```
 *
 * @see {@link ./SetupWizard.tsx}   -- The multi-step wizard container
 * @see {@link ./steps/}            -- Individual step components (internal)
 * @see {@link @/stores/setupStore} -- Zustand store managing wizard state
 * @see {@link @/stores/uiStore}    -- Controls visibility via showSetupWizard flag
 */

export { SetupWizard } from './SetupWizard';
