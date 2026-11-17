// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The loading state shown while a page that loads on demand (Help,
 * Updates, Settings) arrives (polish pass M12).
 */

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PageLoading } from './PageLoading';

describe('PageLoading', () => {
  it("shows the page's own heading and the app's spinner, announced as loading", () => {
    render(<PageLoading title="Help" />);
    expect(screen.getByRole('heading', { level: 1, name: 'Help' })).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Loading…');
  });
});
