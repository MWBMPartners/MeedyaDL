/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file settings/index.ts -- Barrel export for settings components.
 *
 * Re-exports the top-level {@link SettingsPage} component so that other parts
 * of the application can import it from `@/components/settings` without
 * reaching into internal module paths. The individual tab components
 * (GeneralTab, QualityTab, FallbackTab, etc.) are intentionally kept private
 * to this module -- they are imported directly by {@link SettingsPage} and
 * are not part of the public API surface.
 *
 * Usage:
 * ```ts
 * import { SettingsPage } from '@/components/settings';
 * ```
 *
 * @see {@link ./SettingsPage.tsx} -- The main settings page container
 * @see {@link ./tabs/}           -- Individual settings tab components (internal)
 */

export { SettingsPage } from './SettingsPage';
