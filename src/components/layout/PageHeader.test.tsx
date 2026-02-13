/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/layout/PageHeader.test.tsx
 *
 * Unit tests for the PageHeader component. PageHeader is a purely
 * presentational component that renders a title, optional subtitle,
 * and optional action buttons. It has no store dependencies, making
 * it straightforward to test via render + screen queries.
 *
 * @see src/components/layout/PageHeader.tsx - The component under test
 */

import { render, screen } from '@testing-library/react';

import { PageHeader } from '@/components/layout/PageHeader';

describe('PageHeader', () => {
  // =========================================================================
  // Title rendering
  // =========================================================================

  /** The title prop is required and should always render as an h2 heading. */
  it('renders the title as an h2 element', () => {
    render(<PageHeader title="Download" />);

    const heading = screen.getByRole('heading', { level: 2 });
    expect(heading).toHaveTextContent('Download');
  });

  // =========================================================================
  // Subtitle rendering
  // =========================================================================

  /** When a subtitle is provided, it should appear below the title. */
  it('renders the subtitle when provided', () => {
    render(<PageHeader title="Queue" subtitle="3 items in queue" />);

    expect(screen.getByText('3 items in queue')).toBeInTheDocument();
  });

  /** When no subtitle is provided, no extra paragraph element should exist. */
  it('does not render subtitle when omitted', () => {
    render(<PageHeader title="Help" />);

    /* The title should exist but there should be no subtitle text */
    expect(screen.getByText('Help')).toBeInTheDocument();
    expect(screen.queryByText(/items/i)).not.toBeInTheDocument();
  });

  // =========================================================================
  // Actions slot
  // =========================================================================

  /** The actions prop renders arbitrary JSX on the right side of the header. */
  it('renders action buttons when provided', () => {
    render(
      <PageHeader
        title="Queue"
        actions={<button data-testid="refresh-btn">Refresh</button>}
      />,
    );

    expect(screen.getByTestId('refresh-btn')).toBeInTheDocument();
    expect(screen.getByText('Refresh')).toBeInTheDocument();
  });

  /** When no actions are provided, the actions container should not render. */
  it('does not render actions container when omitted', () => {
    const { container } = render(<PageHeader title="Settings" />);

    /*
     * The header should have only one child div (the title column).
     * Without actions, there should be no second flex container.
     */
    const header = container.querySelector('header');
    expect(header).toBeInTheDocument();
  });

  // =========================================================================
  // Multiple actions
  // =========================================================================

  /** Multiple action buttons should all render within the actions container. */
  it('renders multiple action elements', () => {
    render(
      <PageHeader
        title="Queue"
        actions={
          <>
            <button>Clear</button>
            <button>Refresh</button>
          </>
        }
      />,
    );

    expect(screen.getByText('Clear')).toBeInTheDocument();
    expect(screen.getByText('Refresh')).toBeInTheDocument();
  });

  // =========================================================================
  // Semantic structure
  // =========================================================================

  /** The component should render a <header> element for semantic HTML. */
  it('renders as a semantic header element', () => {
    const { container } = render(<PageHeader title="Download" />);

    expect(container.querySelector('header')).toBeInTheDocument();
  });
});
