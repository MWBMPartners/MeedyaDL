import { describe, it, expect } from 'vitest';
import {
  deriveWrapperDecryptAddress,
  nextWrapperDecryptAddress,
} from './wrapperDecryptAddress';

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

describe('nextWrapperDecryptAddress', () => {
  it('replaces the old default', () => {
    expect(nextWrapperDecryptAddress('http://192.168.1.20:80', '127.0.0.1:10020', null)).toBe('192.168.1.20:10020');
  });
  it('replaces an empty value', () => {
    expect(nextWrapperDecryptAddress('http://10.0.0.5', '', null)).toBe('10.0.0.5:10020');
  });
  it('follows the URL while the stored value was automatic', () => {
    expect(nextWrapperDecryptAddress('http://10.0.0.6', '10.0.0.5:10020', '10.0.0.5:10020')).toBe('10.0.0.6:10020');
  });
  it('keeps a value the person typed', () => {
    expect(nextWrapperDecryptAddress('http://10.0.0.6', 'mybox:4444', '10.0.0.5:10020')).toBeNull();
  });
  it('leaves the stored value alone for an invalid URL', () => {
    expect(nextWrapperDecryptAddress('http://', '127.0.0.1:10020', null)).toBeNull();
  });
  it('does nothing when already equal', () => {
    expect(nextWrapperDecryptAddress('http://10.0.0.5', '10.0.0.5:10020', null)).toBeNull();
  });
});
