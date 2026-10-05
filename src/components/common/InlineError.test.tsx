// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for InlineError and explainError (polish pass M8): an error
 * drawn on the page shows a plain message; the backend's technical text is
 * folded under "Details", never shown as the message.
 */

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { InlineError } from './InlineError';
import { explainError } from '@/lib/errorMessages';

const FALLBACK = 'MeedyaDL could not install GAMDL, the tool it downloads with. Check your internet connection, then try again.';

describe('InlineError with explainError', () => {
  it('shows the plain message, with a technical backend error folded under Details', () => {
    const raw = 'pip install gamdl failed: ERROR: Could not find a version that satisfies the requirement (os error 2)';
    render(<InlineError {...explainError(FALLBACK, raw)} />);
    expect(screen.getByRole('alert')).toHaveTextContent(FALLBACK);
    const details = screen.getByText('Details').closest('details');
    expect(details?.open).toBe(false);
    expect(details).toHaveTextContent(raw);
    // The raw text is not the visible message.
    expect(screen.getByText(FALLBACK).textContent).not.toContain('pip install');
  });

  it("keeps the backend's own words when they are already a plain sentence", () => {
    const plain = 'Python 3.10 or newer is needed. Install it, or let MeedyaDL download its own copy.';
    render(<InlineError {...explainError(FALLBACK, new Error(plain))} />);
    expect(screen.getByRole('alert')).toHaveTextContent(plain);
    expect(screen.queryByText('Details')).toBeNull();
  });
});
