/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/ProgressBar.test.tsx - Unit tests for the ProgressBar component
 *
 * Tests the ProgressBar component's determinate and indeterminate modes,
 * value clamping to the 0-100 range, label/percentage display behaviour,
 * and className forwarding to the outer container.
 *
 * The ProgressBar has two distinct rendering modes:
 * - Determinate (value is a number): shows a filled bar with an inline width style.
 * - Indeterminate (value is null): shows an animated sliding bar.
 *
 * @see src/components/common/ProgressBar.tsx - The component under test
 */

import { render, screen } from '@testing-library/react';
import { ProgressBar } from '@/components/common/ProgressBar';

describe('ProgressBar', () => {
  // ===========================================================================
  // Determinate Mode
  // ===========================================================================

  /**
   * Verifies that in determinate mode (value is a number), the filled bar
   * element has an inline width style matching the provided percentage.
   * The width is set via `style={{ width: '45%' }}` to control how much
   * of the track is filled.
   */
  it('renders determinate bar with correct width style', () => {
    const { container } = render(<ProgressBar value={45} />);

    /*
     * The determinate bar is the inner div with 'bg-accent' inside the
     * overflow-hidden track. It uses an inline style for the width.
     * We query for the element that has transition-all (determinate bar's
     * smooth animation class) and check its inline style.
     */
    const bar = container.querySelector('.bg-accent.transition-all');
    expect(bar).toBeTruthy();
    /* The inline width style should match the percentage value */
    expect(bar!).toHaveStyle({ width: '45%' });
  });

  // ===========================================================================
  // Value Clamping
  // ===========================================================================

  /**
   * Verifies that negative progress values are clamped to 0%.
   * The component uses Math.max(0, Math.min(100, value)) to ensure
   * the bar never renders with a negative width, which would be
   * visually meaningless and could cause layout issues.
   */
  it('clamps negative values to 0%', () => {
    const { container } = render(<ProgressBar value={-20} />);

    const bar = container.querySelector('.bg-accent.transition-all');
    expect(bar).toBeTruthy();
    /* Negative values should be clamped to 0% width */
    expect(bar!).toHaveStyle({ width: '0%' });
  });

  /**
   * Verifies that progress values exceeding 100 are clamped to 100%.
   * The component uses Math.min(100, value) to cap the bar at full width,
   * preventing it from overflowing the track container.
   */
  it('clamps values over 100 to 100%', () => {
    const { container } = render(<ProgressBar value={150} />);

    const bar = container.querySelector('.bg-accent.transition-all');
    expect(bar).toBeTruthy();
    /* Values above 100 should be clamped to 100% width */
    expect(bar!).toHaveStyle({ width: '100%' });
  });

  // ===========================================================================
  // Indeterminate Mode
  // ===========================================================================

  /**
   * Verifies that when value=null, the component enters indeterminate mode
   * and renders an animated bar instead of a static filled bar. The
   * indeterminate bar has a CSS animation applied via an inline style
   * containing the 'indeterminate' animation name. It also has the w-1/3
   * class making it 1/3 of the track width.
   */
  it('renders indeterminate animation when value=null', () => {
    const { container } = render(<ProgressBar value={null} />);

    /*
     * The indeterminate bar uses w-1/3 class (1/3 of the track width)
     * and an inline animation style. We look for the element with
     * the w-1/3 class inside the track.
     */
    const animatedBar = container.querySelector('.w-1\\/3.bg-accent');
    expect(animatedBar).toBeTruthy();
    /* The animation style should reference the 'indeterminate' keyframes */
    expect(animatedBar!.getAttribute('style')).toContain('indeterminate');
  });

  // ===========================================================================
  // Label Display
  // ===========================================================================

  /**
   * Verifies that when the label prop is provided, the label text is
   * displayed above the progress bar track. The label describes what
   * operation the bar is tracking (e.g. "Downloading", "Processing").
   */
  it('shows label text when provided', () => {
    render(<ProgressBar value={50} label="Downloading" />);

    /* The label text should be visible in the document */
    expect(screen.getByText('Downloading')).toBeInTheDocument();
  });

  // ===========================================================================
  // Percentage Display
  // ===========================================================================

  /**
   * Verifies that in determinate mode with a label, the rounded percentage
   * is shown next to the label text. The percentage gives users a precise
   * numeric indication of progress alongside the visual bar.
   */
  it('shows percentage next to label in determinate mode', () => {
    render(<ProgressBar value={73} label="Downloading" />);

    /* The percentage should be displayed as a rounded integer with % suffix */
    expect(screen.getByText('73%')).toBeInTheDocument();
  });

  /**
   * Verifies that in indeterminate mode (value=null), the percentage is
   * NOT displayed even when a label is provided. Since the progress is
   * unknown in indeterminate mode, showing "0%" would be misleading.
   */
  it('does not show percentage in indeterminate mode', () => {
    render(<ProgressBar value={null} label="Preparing..." />);

    /* The label should still appear */
    expect(screen.getByText('Preparing...')).toBeInTheDocument();
    /* But no percentage text should be rendered */
    expect(screen.queryByText('%')).not.toBeInTheDocument();
  });

  // ===========================================================================
  // className Forwarding
  // ===========================================================================

  /**
   * Verifies that the className prop is merged onto the outer container div.
   * This allows consumers to add spacing, margins, or other layout utilities
   * to the progress bar wrapper without overriding internal styles.
   */
  it('applies className to outer container', () => {
    const { container } = render(
      <ProgressBar value={50} className="mt-4 custom-class" />,
    );

    /*
     * The outer container is the root div rendered by the component.
     * The className prop should be appended to the component's own classes.
     */
    const outerDiv = container.firstElementChild;
    expect(outerDiv).toBeTruthy();
    expect(outerDiv!.className).toContain('mt-4');
    expect(outerDiv!.className).toContain('custom-class');
  });
});
