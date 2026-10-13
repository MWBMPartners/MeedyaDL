// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The setup wizard installs the Spotify engine only for developer access.
 *
 * Until October 2026 the wizard installed votify (the Spotify engine) for
 * everyone and told everyone it "powers Spotify downloads", although Spotify
 * is a developer-only preview that everyone else is refused.
 */

import { useDependencyStore } from './dependencyStore';
import { useSettingsStore } from './settingsStore';

const installGamdl = vi.fn().mockResolvedValue('3.9.1');
const checkGamdlStatus = vi.fn().mockResolvedValue({ name: 'GAMDL', installed: true, version: '3.9.1' });
const installVotify = vi.fn().mockResolvedValue('1.0.0');

vi.mock('@/lib/tauri-commands', () => ({
  installGamdl: () => installGamdl(),
  checkGamdlStatus: () => checkGamdlStatus(),
  installVotify: () => installVotify(),
}));

function setDeveloperAccess(on: boolean) {
  useSettingsStore.setState((s) => ({ settings: { ...s.settings, dev_access_enabled: on } }));
}

describe('installBundledEngines', () => {
  beforeEach(() => {
    installGamdl.mockClear();
    installVotify.mockClear();
  });

  it('installs GAMDL only, without developer access', async () => {
    setDeveloperAccess(false);
    await useDependencyStore.getState().installBundledEngines();
    expect(installGamdl).toHaveBeenCalledTimes(1);
    expect(installVotify).not.toHaveBeenCalled();
  });

  it('also installs votify with developer access', async () => {
    setDeveloperAccess(true);
    await useDependencyStore.getState().installBundledEngines();
    expect(installGamdl).toHaveBeenCalledTimes(1);
    expect(installVotify).toHaveBeenCalledTimes(1);
  });
});
