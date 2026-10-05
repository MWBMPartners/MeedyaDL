// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The small shared controls added in the polish pass (M17).
 *
 * Before the polish pass, about fifty buttons, a few dropdowns and a handful
 * of checkboxes outside components/common were drawn by hand, each with its
 * own padding, colours, focus ring (or none) and corner rounding. They now go
 * through these components. The tests pin the parts a hand-made copy most
 * often got wrong: a name for screen readers, `type="button"` (so a button in
 * a form never submits it), and the open/current state being announced.
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Checkbox } from './Checkbox';
import { DisclosureButton } from './DisclosureButton';
import { IconButton } from './IconButton';
import { NavListItem } from './NavListItem';
import { TextButton } from './TextButton';

describe('IconButton', () => {
  it('is named for screen readers and shows the same words on hover', () => {
    render(<IconButton icon={<svg />} label="Clear search" onClick={() => {}} />);
    const button = screen.getByRole('button', { name: 'Clear search' });
    // aria-label itself, not only the hover title: a title alone is not
    // read out by every screen reader and never shows on a touch screen.
    expect(button).toHaveAttribute('aria-label', 'Clear search');
    expect(button).toHaveAttribute('title', 'Clear search');
    expect(button).toHaveAttribute('type', 'button');
  });

  it('runs its action when clicked', () => {
    const onClick = vi.fn();
    render(<IconButton icon={<svg />} label="Remove" onClick={onClick} />);
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});

describe('TextButton', () => {
  it('is a real button that does not submit a form', () => {
    const onSubmit = vi.fn((e: { preventDefault: () => void }) => e.preventDefault());
    render(
      <form onSubmit={onSubmit}>
        <TextButton>Apple Developer account</TextButton>
      </form>
    );
    const button = screen.getByRole('button', { name: 'Apple Developer account' });
    expect(button).toHaveAttribute('type', 'button');
    fireEvent.click(button);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('uses the error colour token for a destructive action, never a raw red', () => {
    render(<TextButton tone="danger">Clear</TextButton>);
    const cls = screen.getByRole('button', { name: 'Clear' }).className;
    expect(cls).toContain('text-status-error-text');
    expect(cls).not.toMatch(/text-red-\d/);
  });
});

describe('Checkbox', () => {
  it('reports the new state as true or false and is named by its label', () => {
    const onChange = vi.fn();
    render(<Checkbox checked={false} onChange={onChange} label="Don't ask again" />);
    const box = screen.getByRole('checkbox', { name: "Don't ask again" });
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('can stand alone with only a screen-reader name', () => {
    render(<Checkbox checked onChange={() => {}} aria-label="Select this download" />);
    expect(screen.getByRole('checkbox', { name: 'Select this download' })).toBeChecked();
  });
});

describe('DisclosureButton', () => {
  it('says whether its section is open, and turns the chevron to match', () => {
    const { rerender } = render(<DisclosureButton open={false}>API Field Audit</DisclosureButton>);
    const button = screen.getByRole('button', { name: 'API Field Audit' });
    expect(button).toHaveAttribute('aria-expanded', 'false');
    expect(button).toHaveAttribute('type', 'button');
    const chevron = button.querySelector('svg')!;
    expect(chevron).toHaveAttribute('aria-hidden', 'true');
    expect(chevron.getAttribute('class')).not.toContain('rotate-90');
    rerender(<DisclosureButton open>API Field Audit</DisclosureButton>);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    expect(button.querySelector('svg')!.getAttribute('class')).toContain('rotate-90');
  });
});

describe('NavListItem', () => {
  it('marks only the current page, tab or topic as current', () => {
    render(
      <>
        <NavListItem icon={<svg />} active>
          General
        </NavListItem>
        <NavListItem icon={<svg />} active={false}>
          Tools
        </NavListItem>
      </>
    );
    expect(screen.getByRole('button', { name: 'General' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Tools' })).not.toHaveAttribute('aria-current');
  });

  it('keeps every label left-aligned, however many lines it wraps to (polish pass L3)', () => {
    render(
      <NavListItem icon={<svg />} active={false}>
        Codec &amp; Resolution
      </NavListItem>
    );
    expect(screen.getByRole('button', { name: 'Codec & Resolution' }).className).toContain('text-left');
  });
});
