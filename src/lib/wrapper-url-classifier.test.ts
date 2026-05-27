// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// Unit tests for the wrapper-URL host classifier (#891 follow-up).

import { describe, expect, it } from 'vitest';

import {
  classifyWrapperUrl,
  shouldShowSecurityHint,
} from './wrapper-url-classifier';

describe('classifyWrapperUrl', () => {
  describe('loopback (no hint)', () => {
    it.each([
      ['http://127.0.0.1', 'loopback'],
      ['http://127.0.0.1:80', 'loopback'],
      ['http://127.0.0.1:30020', 'loopback'],
      ['http://localhost', 'loopback'],
      ['http://localhost:80', 'loopback'],
      ['https://localhost:443', 'loopback'],
      ['http://[::1]', 'loopback'],
      ['http://[::1]:80', 'loopback'],
    ] as const)('classifies %s as %s', (input, expected) => {
      expect(classifyWrapperUrl(input)).toBe(expected);
    });
  });

  describe('private RFC 1918 / link-local (soft hint)', () => {
    it.each([
      ['http://10.0.0.1', 'private'],
      ['http://10.255.255.255', 'private'],
      ['http://172.16.0.1', 'private'],
      ['http://172.31.255.254', 'private'],
      ['http://192.168.1.1', 'private'],
      ['http://192.168.0.50:80', 'private'],
      // Link-local 169.254.0.0/16
      ['http://169.254.1.1', 'private'],
      // IPv6 ULA
      ['http://[fc00::1]', 'private'],
      ['http://[fd12:3456:789a::1]', 'private'],
    ] as const)('classifies %s as %s', (input, expected) => {
      expect(classifyWrapperUrl(input)).toBe(expected);
    });
  });

  describe('public (strong hint)', () => {
    it.each([
      // Not loopback, not private — should warn loudly.
      ['http://8.8.8.8', 'public'],
      ['http://1.1.1.1', 'public'],
      ['http://203.0.113.42', 'public'], // RFC 5737 documentation range
      // Edge cases just outside private ranges
      ['http://11.0.0.1', 'public'], // adjacent to 10/8
      ['http://172.15.0.1', 'public'], // adjacent to 172.16/12
      ['http://172.32.0.1', 'public'], // adjacent to 172.16/12
      ['http://192.167.1.1', 'public'], // adjacent to 192.168/16
      ['http://192.169.1.1', 'public'], // adjacent to 192.168/16
    ] as const)('classifies %s as %s', (input, expected) => {
      expect(classifyWrapperUrl(input)).toBe(expected);
    });
  });

  describe('hostname (soft hint — could be LAN or public)', () => {
    it.each([
      ['http://raspberrypi.local', 'hostname'],
      ['http://nas.home', 'hostname'],
      ['http://wrapper.example.com', 'hostname'],
      ['https://my-wrapper.fly.dev', 'hostname'],
      // Even with no scheme — we synthesise http://
      ['raspberrypi.local', 'hostname'],
      ['nas.home:8080', 'hostname'],
    ] as const)('classifies %s as %s', (input, expected) => {
      expect(classifyWrapperUrl(input)).toBe(expected);
    });
  });

  describe('invalid (no hint, defer to Input field validation)', () => {
    it.each([
      ['', 'invalid'],
      ['   ', 'invalid'],
      ['http://', 'invalid'],
      ['not a url at all', 'invalid'],
    ] as const)('classifies %s as %s', (input, expected) => {
      expect(classifyWrapperUrl(input)).toBe(expected);
    });
  });

  it('rejects out-of-range IPv4 octets', () => {
    // The URL constructor itself rejects `256.256.256.256` as a
    // malformed host, so the classifier surfaces it as `invalid`
    // rather than reaching the IP-range checks below.
    expect(classifyWrapperUrl('http://256.256.256.256')).toBe('invalid');
  });

  it('non-numeric octets are rejected by the URL parser as invalid', () => {
    // `192.168.foo.1` doesn't pass URL host parsing either; same
    // resolution as out-of-range octets.
    expect(classifyWrapperUrl('http://192.168.foo.1')).toBe('invalid');
  });
});

describe('shouldShowSecurityHint', () => {
  it('returns false for loopback', () => {
    expect(shouldShowSecurityHint('loopback')).toBe(false);
  });

  it('returns false for invalid (Input field handles validation)', () => {
    expect(shouldShowSecurityHint('invalid')).toBe(false);
  });

  it('returns true for private', () => {
    expect(shouldShowSecurityHint('private')).toBe(true);
  });

  it('returns true for hostname', () => {
    expect(shouldShowSecurityHint('hostname')).toBe(true);
  });

  it('returns true for public', () => {
    expect(shouldShowSecurityHint('public')).toBe(true);
  });
});
