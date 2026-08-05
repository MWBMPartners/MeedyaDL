// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for FeatureNoticeBanner.
 *
 * Pins the contract described in the component's own header comment:
 *   - A disabled feature with a server message renders that message
 *     verbatim.
 *   - A disabled feature with no (or blank) message renders the i18n
 *     fallback copy instead, using the flag's label when present.
 *   - Server-authored text is rendered as inert literal text -- markup
 *     inside a notice message must never become a DOM element.
 *   - Severity maps onto the closed `info` / `warning` / `error` status
 *     token set, with `unknown` degrading to `info`.
 *   - The silent-fetch-failure invariant: a snapshot whose `meta` reports
 *     failures but has no disabled/notice-carrying verdicts renders
 *     nothing.
 */

import { render, screen } from '@testing-library/react';
import { FeatureNoticeBanner } from '@/components/common/FeatureNoticeBanner';
import { useFeatureFlagStore } from '@/stores/featureFlagStore';
import { makeFeatureFlagsSnapshot, makeFlagVerdict } from '@/testing/fixtures';
import { initI18n } from '@/lib/i18n';

// English translations are bundled inline in i18n.ts, so this resolves
// synchronously without a network fetch -- matching real app startup.
beforeAll(async () => {
  await initI18n();
});

function setSnapshot(overrides: Parameters<typeof makeFeatureFlagsSnapshot>[0]) {
  useFeatureFlagStore.setState({
    data: makeFeatureFlagsSnapshot(overrides),
    isLoading: false,
    isDirty: false,
    error: null,
  });
}

describe('FeatureNoticeBanner', () => {
  beforeEach(() => {
    setSnapshot({});
  });

  it('renders nothing when there are no notice-worthy verdicts', () => {
    const { container } = render(<FeatureNoticeBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('renders the server message verbatim for a disabled feature with a message', () => {
    setSnapshot({
      verdicts: {
        'service.bbc-iplayer': makeFlagVerdict({
          enabled: false,
          label: 'BBC iPlayer',
          notice: {
            message: 'BBC iPlayer downloads are paused for a licensing review.',
            severity: 'maintenance',
            url: null,
          },
        }),
      },
    });

    render(<FeatureNoticeBanner />);

    expect(
      screen.getByText('BBC iPlayer downloads are paused for a licensing review.')
    ).toBeInTheDocument();
  });

  it('renders the i18n fallback when disabled with no notice, using the label', () => {
    setSnapshot({
      verdicts: {
        'service.spotify': makeFlagVerdict({ enabled: false, label: 'Spotify', notice: null }),
      },
    });

    render(<FeatureNoticeBanner />);

    expect(
      screen.getByText(
        "Spotify is temporarily unavailable. We've paused it while we look into something — nothing is wrong with your installation, and there's nothing you need to do."
      )
    ).toBeInTheDocument();
  });

  it('renders the i18n fallback using the flag key when no label is present', () => {
    setSnapshot({
      verdicts: {
        'service.youtube-music': makeFlagVerdict({ enabled: false, label: null, notice: null }),
      },
    });

    render(<FeatureNoticeBanner />);

    expect(screen.getByText(/^service\.youtube-music is temporarily unavailable\./)).toBeInTheDocument();
  });

  it('renders the i18n fallback when disabled and the notice message is blank', () => {
    setSnapshot({
      verdicts: {
        'service.spotify': makeFlagVerdict({
          enabled: false,
          label: 'Spotify',
          notice: { message: '   ', severity: 'info', url: null },
        }),
      },
    });

    render(<FeatureNoticeBanner />);

    expect(screen.getByText(/^Spotify is temporarily unavailable\./)).toBeInTheDocument();
  });

  it('renders a message containing script tags and markdown as inert literal text', () => {
    const hostile = '<script>alert(1)</script> **bold** [click me](javascript:evil())';
    setSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({
          enabled: false,
          notice: { message: hostile, severity: 'critical', url: null },
        }),
      },
    });

    const { container } = render(<FeatureNoticeBanner />);

    // The literal string is present as text...
    expect(screen.getByText(hostile)).toBeInTheDocument();
    // ...and did NOT become a real <script> element or any injected node.
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('strong')).toBeNull();
    expect(container.querySelector('a')).toBeNull();
  });

  it.each([
    ['info', 'text-status-info'],
    ['maintenance', 'text-status-warning'],
    ['deprecated', 'text-status-warning'],
    ['critical', 'text-status-error'],
    ['unknown', 'text-status-info'],
  ] as const)('maps severity %s to the %s token', (severity, expectedClass) => {
    setSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({
          enabled: true,
          notice: { message: `Notice for ${severity}`, severity, url: null },
        }),
      },
    });

    render(<FeatureNoticeBanner />);

    const banner = screen.getByTestId('feature-notice-banner');
    expect(banner.className).toContain(expectedClass.replace('text-', 'border-'));
  });

  it('renders nothing when meta reports failures but no verdict is disabled or notice-bearing', () => {
    setSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({ enabled: true, notice: null }),
      },
      meta: {
        source: 'disk_cache',
        fetched_at: null,
        consecutive_failures: 12,
        last_error: 'network unreachable',
      },
    });

    const { container } = render(<FeatureNoticeBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('clamps an overlong notice message to 500 characters', () => {
    const longMessage = 'x'.repeat(600);
    setSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({
          enabled: true,
          notice: { message: longMessage, severity: 'info', url: null },
        }),
      },
    });

    render(<FeatureNoticeBanner />);

    const banner = screen.getByTestId('feature-notice-banner');
    // 500 chars + the ellipsis character appended on truncation.
    expect(banner.textContent?.length).toBe(501);
    expect(banner.textContent?.endsWith('…')).toBe(true);
  });
});
