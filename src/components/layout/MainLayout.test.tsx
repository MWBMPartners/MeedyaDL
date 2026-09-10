/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/layout/MainLayout.test.tsx - Skip-link focus test (a11y audit)
 *
 * Before the fix, activating the "Skip to main content" link moved the
 * page's scroll position (because it's a plain `#main-content` fragment
 * link) but did NOT move actual keyboard focus, because `<main>` was not
 * a focusable element. A keyboard-only user who used the link to jump
 * past the sidebar would then press Tab and land back wherever focus
 * already was -- not inside the content they'd just "skipped" to. This
 * test checks the one thing that actually matters: that
 * `document.activeElement` really is the main content region after the
 * link is activated.
 *
 * @see src/components/layout/MainLayout.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { MainLayout } from '@/components/layout/MainLayout';

describe('MainLayout skip link (a11y audit)', () => {
  it('moves real keyboard focus to the main content region, not just the page scroll position', () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    const skipLink = screen.getByText(/skip to main content/i);
    fireEvent.click(skipLink);

    const main = document.getElementById('main-content');
    expect(main).not.toBeNull();
    expect(document.activeElement).toBe(main);
  });

  it('the main content region is reachable via script focus (tabIndex=-1) but not part of the normal Tab order', () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    const main = document.getElementById('main-content');
    // tabIndex={-1}: focusable via .focus()/script, but explicitly
    // excluded from sequential (Tab-key) navigation.
    expect(main).toHaveAttribute('tabindex', '-1');
  });
});
