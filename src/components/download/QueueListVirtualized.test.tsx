// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the queue list's column header strip (polish pass, L5).
 *
 * The platform column's header used to read "Svc" (shown in capitals as
 * "SVC"); it now reads "Service", and says "Time Left" rather than "ETA"
 * to match what the row shows.
 */

import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { QueueListVirtualized } from './QueueListVirtualized';
import { makeQueueItem } from '@/testing/fixtures';

const noop = vi.fn();

describe('QueueListVirtualized column headers', () => {
  it('names the platform column "Service" and the speed column "Speed / Time Left"', () => {
    render(
      <QueueListVirtualized
        queueItems={[makeQueueItem({ id: 'a' })]}
        onCancel={noop}
        onRetry={noop}
        onRetryWithoutWrapper={noop}
        onCopyUrl={noop}
        onDelete={noop}
        onMoveToTop={noop}
        onMoveUp={noop}
        onMoveDown={noop}
        onMoveToBottom={noop}
      />,
    );
    const headers = screen.getAllByRole('columnheader').map((h) => h.textContent?.trim());
    expect(headers).toContain('Service');
    expect(headers).toContain('Speed / Time Left');
    expect(headers).not.toContain('Svc');
  });
});
