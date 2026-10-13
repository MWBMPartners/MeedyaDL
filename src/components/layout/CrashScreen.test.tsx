// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the screen shown when a component fails to draw.
 *
 * It replaced a developer dump ("React Error Caught", raw stack traces,
 * nothing to click). These tests pin the four things that make it a way
 * forward rather than a dead end: plain words, a Reload button, a Report
 * button once a report is saved, and the technical details folded away.
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { CrashScreen } from './CrashScreen';

// The report dialog loads its preview from the backend; answer it so the
// dialog can open in the test.
vi.mock('@/lib/tauri-commands', () => ({
  exportCrashReport: vi.fn().mockResolvedValue('# Error report'),
  getGitHubIssueUrl: vi.fn(),
  deleteCrashReport: vi.fn(),
}));

const boom = new TypeError("Cannot read properties of null (reading 'filter')");

describe('CrashScreen', () => {
  it('explains what happened in plain words, with no developer heading', () => {
    render(<CrashScreen error={boom} reportId={null} />);
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
      'Something went wrong on this screen'
    );
    expect(screen.queryByText(/React Error Caught/)).not.toBeInTheDocument();
  });

  it('offers a Reload button that reloads', () => {
    const onReload = vi.fn();
    render(<CrashScreen error={boom} reportId={null} onReload={onReload} />);
    fireEvent.click(screen.getByRole('button', { name: /Reload MeedyaDL/ }));
    expect(onReload).toHaveBeenCalledTimes(1);
  });

  it('keeps the technical details inside a closed <details>, not on the page', () => {
    const { container } = render(<CrashScreen error={boom} reportId={null} />);
    const details = container.querySelector('details');
    expect(details).not.toBeNull();
    expect(details).not.toHaveAttribute('open');
    // The raw error text exists, but only inside the folded section.
    const raw = screen.getByText(/Cannot read properties of null/);
    expect(details!.contains(raw)).toBe(true);
  });

  it('offers Report only once a report has been saved, and opens the preview first', async () => {
    const { rerender } = render(<CrashScreen error={boom} reportId={null} />);
    expect(screen.queryByRole('button', { name: /Report this problem/ })).not.toBeInTheDocument();

    rerender(<CrashScreen error={boom} reportId="abc-123" />);
    fireEvent.click(screen.getByRole('button', { name: /Report this problem/ }));
    // The existing consent dialog: nothing is sent until the person has
    // seen the preview and chosen to open the GitHub issue.
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });
});
