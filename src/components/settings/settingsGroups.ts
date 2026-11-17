// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file settingsGroups.ts -- which Settings tabs are listed, and under which
 * heading, for the person using the app.
 *
 * Kept apart from SettingsPage.tsx so the "who sees which tab" rule can be
 * tested on its own, without rendering every settings tab.
 */

/** One heading in the Settings tab list, and the tabs under it. */
export interface SettingsGroup {
  /** Looks up the translated heading under `settings.groups.*`. */
  id: string;
  /** English heading, used when no translation exists. */
  label: string;
  /** Tab ids, matching entries in SettingsPage.tsx's TABS. */
  tabs: string[];
}

/**
 * Groups settings tabs into logical sections for the sidebar navigation.
 * Each group has an id (used to look up its translated heading under
 * `settings.groups.*`), an English fallback label, and a list of tab IDs
 * that belong to that group. The tab IDs must match entries in the TABS array
 * in SettingsPage.tsx.
 *
 * Groups are rendered as static (non-collapsible) sections with visually
 * distinct headers -- with only 5 groups, collapsible behaviour would add
 * UI complexity without meaningful benefit.
 */
export const SETTINGS_GROUPS: SettingsGroup[] = [
  { id: 'general', label: 'General', tabs: ['general'] },
  {
    id: 'download',
    label: 'Download',
    tabs: ['quality', 'fallback', 'lyrics', 'cover-art', 'metadata', 'templates'],
  },
  { id: 'authentication', label: 'Authentication', tabs: ['cookies'] },
  // M9-UI: Spotify tab joins a new 'Services' group between
  // Authentication and System. It is a developer-only preview, so the
  // group is only listed with developer access on (see
  // `visibleSettingsGroups()` below). YouTube + BBC iPlayer tabs will
  // land in the same group as their integrations (M10 / M8) ship.
  { id: 'services', label: 'Services', tabs: ['spotify'] },
  { id: 'system', label: 'System', tabs: ['tools', 'advanced'] },
];

/**
 * Tabs that belong to a developer-only preview, listed only when developer
 * access is on. Today that is Spotify alone. Until October 2026 the Spotify
 * tab was listed for everyone, although Spotify downloads were refused to
 * everyone without developer access -- a tab full of controls for a feature
 * the person could not use.
 */
export const DEVELOPER_ONLY_TABS: ReadonlySet<string> = new Set(['spotify']);

/**
 * The groups and tabs to list. Developer-only tabs are dropped unless
 * developer access is on, and a group left with no tabs (the "Services"
 * group, whose only tab is Spotify) is dropped with them.
 *
 * @param developerAccess -- whether developer access is on.
 */
export function visibleSettingsGroups(developerAccess: boolean): SettingsGroup[] {
  return SETTINGS_GROUPS.map((group) => ({
    ...group,
    tabs: group.tabs.filter((tab) => developerAccess || !DEVELOPER_ONLY_TABS.has(tab)),
  })).filter((group) => group.tabs.length > 0);
}
