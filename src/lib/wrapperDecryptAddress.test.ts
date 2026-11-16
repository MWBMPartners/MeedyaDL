import { describe, it, expect } from 'vitest';
import { deriveWrapperDecryptAddress } from './wrapperDecryptAddress';

// The same cases as `utils::wrapper_address` tests on the Rust side.
describe('deriveWrapperDecryptAddress', () => {
  it.each([
    ['http://192.168.1.20:80', '192.168.1.20:10020'],
    ['http://192.168.1.20', '192.168.1.20:10020'],
    ['http://127.0.0.1', '127.0.0.1:10020'],
    ['http://localhost:30020', 'localhost:10020'],
    ['https://pi.local/', 'pi.local:10020'],
    ['192.168.1.20:80', '192.168.1.20:10020'],
    ['http://[::1]:80', '[::1]:10020'],
    ['http://[fe80::1]', '[fe80::1]:10020'],
  ])('%s -> %s', (input, expected) => {
    expect(deriveWrapperDecryptAddress(input)).toBe(expected);
  });

  it.each(['', '   ', 'http://', 'ftp://host', 'not a url at all'])('%j is invalid', (input) => {
    expect(deriveWrapperDecryptAddress(input)).toBeNull();
  });
});
