// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Classify a wrapper-v2 URL by where its host falls on the network
// trust hierarchy. Used by Settings → Advanced → Wrapper to surface
// a security hint when the user is pointing MeedyaDL at a non-
// loopback wrapper instance (#891 follow-up).
//
// Wrapper-v2 has NO network-layer authentication — any client that
// can reach its HTTP port can use it for FairPlay decryption. Running
// it on loopback is safe (only this machine can reach it). Running
// it on a LAN address is fine on a trusted home network but risky
// on shared networks (coffee shop, dorm, work). Running it on a
// public IP is almost always a mistake.
//
// We surface a short hint that scales with the risk level, so users
// see the relevant security concern without being shouted at when
// they're on the default loopback config.

/**
 * Classification of a wrapper URL's host on the network trust
 * hierarchy.
 *
 * - `'loopback'` — host is `127.0.0.1` / `::1` / `localhost`. Safe;
 *   the wrapper is only reachable from this machine.
 * - `'private'` — host is in an RFC 1918 private IPv4 range
 *   (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`) or an IPv6
 *   ULA (`fc00::/7`). Reachable from other devices on the same LAN.
 *   Fine on a trusted home network; surface a security note.
 * - `'public'` — host parses as an IP that is neither loopback nor
 *   in a private range. The wrapper is reachable from the public
 *   internet — almost always a misconfiguration; surface a strong
 *   warning.
 * - `'hostname'` — host is a DNS name rather than a literal IP. We
 *   can't tell from the string alone whether it resolves to a
 *   private or public address; treat as the soft-warning case
 *   (same as `'private'`) since LAN hostnames (`raspberrypi.local`,
 *   `nas.home`) are the common shape here.
 * - `'invalid'` — URL didn't parse. The Wrapper field is free-form
 *   text input, so users can paste garbage; report rather than
 *   throw.
 */
export type WrapperUrlClass =
  | 'loopback'
  | 'private'
  | 'public'
  | 'hostname'
  | 'invalid';

/**
 * Classify the host of a wrapper URL. See {@link WrapperUrlClass}.
 *
 * Returns `'invalid'` for empty strings, garbage, and URLs whose
 * host is missing. Never throws.
 */
export function classifyWrapperUrl(rawUrl: string): WrapperUrlClass {
  const trimmed = rawUrl.trim();
  if (trimmed === '') return 'invalid';
  let host: string;
  try {
    // Tolerate users who paste just a hostname (no scheme); the
    // URL constructor needs a scheme, so prepend http:// when
    // absent. We only need the host, not the scheme.
    const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `http://${trimmed}`;
    host = new URL(withScheme).hostname;
  } catch {
    return 'invalid';
  }
  if (host === '') return 'invalid';

  // The URL spec wraps IPv6 hostnames in `[...]`; strip the
  // brackets so range checks below match the bare address. Both
  // platforms (browsers + Node 22 jsdom) preserve the brackets in
  // `new URL().hostname`, so this is necessary, not defensive.
  if (host.startsWith('[') && host.endsWith(']')) {
    host = host.slice(1, -1);
  }

  // Loopback names + IPv4 loopback + IPv6 loopback.
  if (host === 'localhost') return 'loopback';
  if (host === '127.0.0.1') return 'loopback';
  // The URL constructor strips IPv6 brackets, so we see the bare
  // address. ::1 is the only IPv6 loopback.
  if (host === '::1') return 'loopback';

  // IPv4 literal? Range check it.
  const ipv4 = parseIpv4(host);
  if (ipv4 !== null) {
    const [a, b] = ipv4;
    // 10.0.0.0/8
    if (a === 10) return 'private';
    // 172.16.0.0/12  →  172.16 .. 172.31
    if (a === 172 && b >= 16 && b <= 31) return 'private';
    // 192.168.0.0/16
    if (a === 192 && b === 168) return 'private';
    // 169.254.0.0/16 (link-local) — treat as private for hint purposes
    if (a === 169 && b === 254) return 'private';
    // Anything else parsing as IPv4 is public.
    return 'public';
  }

  // IPv6 ULA range: fc00::/7. The first byte is fc or fd.
  if (/^fc[0-9a-f]{2}:|^fd[0-9a-f]{2}:/i.test(host)) {
    return 'private';
  }

  // Not a recognised IP literal — must be a DNS name. We can't
  // know without resolving whether it points at a private or
  // public address, so treat as the soft-warning case.
  return 'hostname';
}

/**
 * Whether the classification should surface a security hint in the
 * Settings UI. `loopback` and `invalid` don't get a hint
 * (loopback because it's the safe default; invalid because the
 * input field's own validation should handle malformed URLs).
 */
export function shouldShowSecurityHint(c: WrapperUrlClass): boolean {
  return c === 'private' || c === 'public' || c === 'hostname';
}

/**
 * Parse a string as a dotted-quad IPv4 address. Returns `null` if
 * the string isn't a valid IPv4 literal. We only need the first
 * two octets for the private-range checks.
 */
function parseIpv4(host: string): [number, number, number, number] | null {
  const parts = host.split('.');
  if (parts.length !== 4) return null;
  const nums: number[] = [];
  for (const p of parts) {
    if (!/^\d+$/.test(p)) return null;
    const n = parseInt(p, 10);
    if (n < 0 || n > 255) return null;
    nums.push(n);
  }
  return [nums[0], nums[1], nums[2], nums[3]];
}
