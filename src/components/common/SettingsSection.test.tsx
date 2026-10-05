// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Settings section titles are real headings that screen readers find.
 *
 * The title used to be an <h3> inside the toggle <button>. A heading inside
 * a button is flattened into the button's name, so screen readers found no
 * headings at all in Settings. The heading now holds the button.
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsSection } from './SettingsSection';

describe('SettingsSection', () => {
  it('is an <h2> heading that contains the toggle button, never the reverse', () => {
    const { container } = render(
      <SettingsSection title="Output" description="Where downloads go.">
        <p>Body</p>
      </SettingsSection>
    );
    const heading = screen.getByRole('heading', { level: 2, name: 'Output' });
    expect(heading.querySelector('button')).not.toBeNull();
    expect(container.querySelector('button h1, button h2, button h3, button h4, button h5, button h6')).toBeNull();
  });

  it("names the button by the title alone; the description is not part of the button", () => {
    render(
      <SettingsSection title="Output" description="Where downloads go.">
        <p>Body</p>
      </SettingsSection>
    );
    expect(screen.getByRole('button', { name: 'Output' })).toBeInTheDocument();
    expect(screen.getByText('Where downloads go.').closest('button')).toBeNull();
  });

  it('still opens and closes', () => {
    render(
      <SettingsSection title="Output">
        <p>Body</p>
      </SettingsSection>
    );
    const toggle = screen.getByRole('button', { name: 'Output' });
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Body')).toBeInTheDocument();
    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('Body')).not.toBeInTheDocument();
  });
});
