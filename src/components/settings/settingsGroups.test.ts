// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Which Settings tabs are listed for whom.
 *
 * Spotify is a developer-only preview. Until October 2026 its tab, under a
 * "Services" heading, was listed for everyone, although Spotify downloads
 * were refused to everyone without developer access.
 */

import { visibleSettingsGroups, SETTINGS_GROUPS } from './settingsGroups';

const allTabs = (groups: { tabs: string[] }[]) => groups.flatMap((g) => g.tabs);

describe('visibleSettingsGroups', () => {
  it('lists no Spotify tab, and no empty Services heading, without developer access', () => {
    const groups = visibleSettingsGroups(false);
    expect(allTabs(groups)).not.toContain('spotify');
    expect(groups.map((g) => g.id)).not.toContain('services');
    expect(groups.every((g) => g.tabs.length > 0)).toBe(true);
  });

  it('lists every tab, Spotify included, with developer access', () => {
    expect(allTabs(visibleSettingsGroups(true))).toEqual(allTabs(SETTINGS_GROUPS));
    expect(allTabs(visibleSettingsGroups(true))).toContain('spotify');
  });

  it('hides nothing but the developer-only tabs', () => {
    const hidden = allTabs(SETTINGS_GROUPS).filter((t) => !allTabs(visibleSettingsGroups(false)).includes(t));
    expect(hidden).toEqual(['spotify']);
  });
});
