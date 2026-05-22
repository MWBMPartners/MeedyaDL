// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for MvGapFillModal (#717 sub-feature 5g).
 *
 * The modal has two prompt branches gated on
 * `mvCompanionEnabledInSettings`, and four resulting
 * `mv_companion_override` outcomes (the 2x2 of branch x button).
 * Tests pin the override-mapping table so a future copy tweak or
 * button reorder can't silently invert the semantics:
 *
 *   ENABLED  + Yes → null   (inherit settings, no per-item override)
 *   ENABLED  + No  → false  (force-disable for this item)
 *   DISABLED + Yes → true   (force-enable for this item)
 *   DISABLED + No  → null   (inherit settings, already off)
 *
 * @see MvGapFillModal in src/components/library/MvGapFillModal.tsx
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { MvGapFillModal } from '@/components/library/MvGapFillModal';
import { makeScannedManifest } from '@/testing/fixtures';

vi.mock('lucide-react', () => ({
  X: (props: Record<string, unknown>) => <span data-testid="x-icon" {...props} />,
  Music: (props: Record<string, unknown>) => (
    <span data-testid="music-icon" {...props} />
  ),
  Video: (props: Record<string, unknown>) => (
    <span data-testid="video-icon" {...props} />
  ),
  Info: (props: Record<string, unknown>) => (
    <span data-testid="info-icon" {...props} />
  ),
}));

const baseManifest = makeScannedManifest();

describe('MvGapFillModal', () => {
  // ===========================================================================
  // Rendering gates
  // ===========================================================================

  it('returns null when manifest is null (regardless of open)', () => {
    const { container } = render(
      <MvGapFillModal
        open={true}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        mvCompanionEnabledInSettings={true}
        manifest={null}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('returns null when open=false (nothing rendered)', () => {
    const { container } = render(
      <MvGapFillModal
        open={false}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        mvCompanionEnabledInSettings={true}
        manifest={baseManifest}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('includes the album name in the modal title', () => {
    render(
      <MvGapFillModal
        open={true}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        mvCompanionEnabledInSettings={true}
        manifest={baseManifest}
      />
    );
    expect(screen.getByText(/Re-download gaps/)).toBeInTheDocument();
    expect(screen.getByText(/Test Album/)).toBeInTheDocument();
  });

  // ===========================================================================
  // ENABLED branch — settings already on
  // ===========================================================================

  describe('mvCompanionEnabledInSettings = true', () => {
    it('renders the "enabled" prompt copy', () => {
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={vi.fn()}
          mvCompanionEnabledInSettings={true}
          manifest={baseManifest}
        />
      );
      expect(screen.getByText(/enabled/i)).toBeInTheDocument();
      expect(
        screen.getByText('Include music videos in this gap-fill?')
      ).toBeInTheDocument();
    });

    it('Yes → null (inherit settings)', () => {
      const onConfirm = vi.fn();
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={onConfirm}
          mvCompanionEnabledInSettings={true}
          manifest={baseManifest}
        />
      );
      fireEvent.click(screen.getByText('Yes — include videos'));
      expect(onConfirm).toHaveBeenCalledTimes(1);
      expect(onConfirm).toHaveBeenCalledWith(null);
    });

    it('No → false (force-disable for this item)', () => {
      const onConfirm = vi.fn();
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={onConfirm}
          mvCompanionEnabledInSettings={true}
          manifest={baseManifest}
        />
      );
      fireEvent.click(screen.getByText('No — audio only'));
      expect(onConfirm).toHaveBeenCalledTimes(1);
      expect(onConfirm).toHaveBeenCalledWith(false);
    });
  });

  // ===========================================================================
  // DISABLED branch — settings off
  // ===========================================================================

  describe('mvCompanionEnabledInSettings = false', () => {
    it('renders the "disabled" prompt copy', () => {
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={vi.fn()}
          mvCompanionEnabledInSettings={false}
          manifest={baseManifest}
        />
      );
      expect(screen.getByText(/currently disabled/i)).toBeInTheDocument();
      expect(
        screen.getByText('Include music videos in this gap-fill anyway?')
      ).toBeInTheDocument();
    });

    it('Yes → true (force-enable for this item)', () => {
      const onConfirm = vi.fn();
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={onConfirm}
          mvCompanionEnabledInSettings={false}
          manifest={baseManifest}
        />
      );
      fireEvent.click(screen.getByText('Yes — include videos'));
      expect(onConfirm).toHaveBeenCalledTimes(1);
      expect(onConfirm).toHaveBeenCalledWith(true);
    });

    it('No → null (inherit the disabled setting)', () => {
      const onConfirm = vi.fn();
      render(
        <MvGapFillModal
          open={true}
          onClose={vi.fn()}
          onConfirm={onConfirm}
          mvCompanionEnabledInSettings={false}
          manifest={baseManifest}
        />
      );
      fireEvent.click(screen.getByText('No — audio only'));
      expect(onConfirm).toHaveBeenCalledTimes(1);
      expect(onConfirm).toHaveBeenCalledWith(null);
    });
  });

  // ===========================================================================
  // Cancel path
  // ===========================================================================

  it('Cancel button calls onClose, not onConfirm', () => {
    const onClose = vi.fn();
    const onConfirm = vi.fn();
    render(
      <MvGapFillModal
        open={true}
        onClose={onClose}
        onConfirm={onConfirm}
        mvCompanionEnabledInSettings={true}
        manifest={baseManifest}
      />
    );
    fireEvent.click(screen.getByText('Cancel'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
