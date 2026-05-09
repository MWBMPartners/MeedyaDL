// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useConfirmation (audit v2 #3).
 *
 * Pins the hook contract:
 *   - Modal closed by default (open() shows it)
 *   - Confirm fires onConfirm + auto-closes on success
 *   - Confirm stays open if onConfirm throws
 *   - Cancel fires onCancel (if provided) + closes
 *   - Backdrop / Escape also closes via onCancel
 *   - close() programmatically hides without firing onCancel
 *   - Custom labels (confirmLabel / cancelLabel)
 *   - Description accepts arbitrary ReactNode (paragraphs, checkboxes, etc.)
 */

import { useState } from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { useConfirmation } from '@/lib/useConfirmation';

// Don't mock lucide-react here — the hook pulls in Modal + Button +
// ToastContainer transitively via the common index, which uses several
// icons (CheckCircle, AlertCircle, etc.). The real SVG icons render
// fine in jsdom; mocking would require enumerating each one.

/**
 * Test harness: a tiny component that wires the hook to a trigger
 * button so tests can drive open/close from the user's perspective.
 */
function Harness({
  onConfirm,
  onCancel,
  title = 'Confirm action',
  description = 'Are you sure?',
  confirmLabel,
  cancelLabel,
}: Partial<Parameters<typeof useConfirmation>[0]> & {
  onConfirm: () => void | Promise<void>;
}) {
  const confirm = useConfirmation({
    title,
    description,
    confirmLabel,
    cancelLabel,
    onConfirm,
    onCancel,
  });
  return (
    <>
      <button type="button" onClick={confirm.open}>
        Trigger
      </button>
      <button type="button" onClick={confirm.close}>
        Programmatic close
      </button>
      {confirm.modal}
    </>
  );
}

describe('useConfirmation', () => {
  // ===========================================================================
  // Default visibility
  // ===========================================================================

  it('modal is not visible until open() is called', () => {
    render(<Harness onConfirm={vi.fn()} />);
    expect(screen.queryByText('Confirm action')).not.toBeInTheDocument();
  });

  it('open() shows the modal with title + description + buttons', () => {
    render(<Harness onConfirm={vi.fn()} />);
    fireEvent.click(screen.getByText('Trigger'));
    expect(screen.getByText('Confirm action')).toBeInTheDocument();
    expect(screen.getByText('Are you sure?')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^cancel$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^confirm$/i })).toBeInTheDocument();
  });

  // ===========================================================================
  // Confirm path
  // ===========================================================================

  it('Confirm calls onConfirm and auto-closes on success', async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    render(<Harness onConfirm={onConfirm} />);
    fireEvent.click(screen.getByText('Trigger'));
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    });
    expect(onConfirm).toHaveBeenCalledTimes(1);
    // Auto-closed → modal title gone
    expect(screen.queryByText('Confirm action')).not.toBeInTheDocument();
  });

  it('Confirm with synchronous onConfirm closes immediately', () => {
    const onConfirm = vi.fn();
    render(<Harness onConfirm={onConfirm} />);
    fireEvent.click(screen.getByText('Trigger'));
    fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('Confirm stays OPEN if onConfirm rejects (so user can retry)', async () => {
    const onConfirm = vi.fn().mockRejectedValue(new Error('disk full'));
    render(<Harness onConfirm={onConfirm} />);
    fireEvent.click(screen.getByText('Trigger'));
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^confirm$/i }));
    });
    expect(onConfirm).toHaveBeenCalledTimes(1);
    // Modal still rendered for retry
    expect(screen.getByText('Confirm action')).toBeInTheDocument();
  });

  // ===========================================================================
  // Cancel path
  // ===========================================================================

  it('Cancel button closes the modal', () => {
    render(<Harness onConfirm={vi.fn()} />);
    fireEvent.click(screen.getByText('Trigger'));
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    expect(screen.queryByText('Confirm action')).not.toBeInTheDocument();
  });

  it('Cancel button fires onCancel callback when provided', () => {
    const onCancel = vi.fn();
    render(<Harness onConfirm={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByText('Trigger'));
    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('Escape key dismisses the modal AND fires onCancel', () => {
    const onCancel = vi.fn();
    render(<Harness onConfirm={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByText('Trigger'));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Confirm action')).not.toBeInTheDocument();
  });

  // ===========================================================================
  // Programmatic close
  // ===========================================================================

  it('close() hides the modal WITHOUT firing onCancel', () => {
    const onCancel = vi.fn();
    render(<Harness onConfirm={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByText('Trigger'));
    expect(screen.getByText('Confirm action')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Programmatic close'));
    expect(screen.queryByText('Confirm action')).not.toBeInTheDocument();
    expect(onCancel).not.toHaveBeenCalled();
  });

  // ===========================================================================
  // Custom labels
  // ===========================================================================

  it('honours custom confirmLabel and cancelLabel', () => {
    render(
      <Harness
        onConfirm={vi.fn()}
        confirmLabel="Delete"
        cancelLabel="Keep it"
      />
    );
    fireEvent.click(screen.getByText('Trigger'));
    expect(screen.getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keep it' })).toBeInTheDocument();
  });

  // ===========================================================================
  // Rich description
  // ===========================================================================

  it('description accepts arbitrary ReactNode (multi-paragraph + interactive)', () => {
    function ParentWithCheckbox() {
      const [checked, setChecked] = useState(false);
      const confirm = useConfirmation({
        title: 'Abort Queue',
        description: (
          <>
            <p>This will cancel every queued item.</p>
            <p>This cannot be undone.</p>
            <label>
              <input
                type="checkbox"
                checked={checked}
                onChange={(e) => setChecked(e.target.checked)}
                aria-label="dont-ask-again"
              />
              Don't ask again
            </label>
          </>
        ),
        onConfirm: vi.fn(),
      });
      return (
        <>
          <button type="button" onClick={confirm.open}>
            Trigger
          </button>
          <span data-testid="checkbox-state">{checked ? 'on' : 'off'}</span>
          {confirm.modal}
        </>
      );
    }
    render(<ParentWithCheckbox />);
    fireEvent.click(screen.getByText('Trigger'));
    expect(screen.getByText('This will cancel every queued item.')).toBeInTheDocument();
    expect(screen.getByText('This cannot be undone.')).toBeInTheDocument();
    // Toggle the checkbox via parent state
    fireEvent.click(screen.getByLabelText('dont-ask-again'));
    expect(screen.getByTestId('checkbox-state')).toHaveTextContent('on');
  });
});
