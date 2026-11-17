// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE for details.

import { describe, expect, it } from 'vitest';
import { wrapperTestAddress } from './wrapperTestAddress';

describe('wrapperTestAddress', () => {
  it('tests the newer wrapper address when the newer wrapper is in use', () => {
    expect(wrapperTestAddress(true, 'http://old:30020', 'http://127.0.0.1')).toBe(
      'http://127.0.0.1'
    );
  });

  it('tests the account address when the older wrapper is in use', () => {
    expect(wrapperTestAddress(false, 'http://old:30020', 'http://127.0.0.1')).toBe(
      'http://old:30020'
    );
  });

  it('does not fall back to the hidden field when the shown one is blank', () => {
    expect(wrapperTestAddress(true, 'http://old:30020', '')).toBe('');
    expect(wrapperTestAddress(false, '', 'http://127.0.0.1')).toBe('');
  });

  it('trims spaces and treats missing values as blank', () => {
    expect(wrapperTestAddress(true, undefined, '  http://127.0.0.1  ')).toBe('http://127.0.0.1');
    expect(wrapperTestAddress(false, undefined, undefined)).toBe('');
  });
});
