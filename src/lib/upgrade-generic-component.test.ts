// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests that the Update button tells the backend it is an UPDATE.
 *
 * The backend refuses to take an update from MeedyaDL's backup download
 * source (it can hold the same or an older version), but only when it is
 * told the install is an update. If this helper stopped saying so, every
 * update would silently go back to the old behaviour -- and nothing else
 * would notice, because install and update look identical from outside.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

const installDependency = vi.fn<(name: string, forUpdate?: boolean) => Promise<string>>();

vi.mock('@/lib/tauri-commands', () => ({
  installDependency: (name: string, forUpdate?: boolean) => installDependency(name, forUpdate),
  upgradeVotify: vi.fn(),
  upgradePipEngine: vi.fn(),
}));

import { upgradeGenericComponent } from '@/lib/upgrade-generic-component';
import type { ComponentUpdate } from '@/types';

function tool(tool_id: string): ComponentUpdate {
  return {
    name: tool_id,
    current_version: '1.0.0',
    latest_version: '1.1.0',
    update_available: true,
    is_compatible: true,
    is_untested: false,
    no_compatible_wheel: false,
    description: null,
    release_url: null,
    release_body: null,
    is_prerelease: false,
    tag_name: null,
    pip_package: null,
    tool_id,
  };
}

describe('upgradeGenericComponent', () => {
  beforeEach(() => {
    installDependency.mockReset();
    installDependency.mockResolvedValue('1.1.0');
  });

  it('tells the backend a helper-tool upgrade is an UPDATE', async () => {
    await upgradeGenericComponent(tool('nm3u8dlre'));
    expect(installDependency).toHaveBeenCalledWith('nm3u8dlre', true);
  });
});
