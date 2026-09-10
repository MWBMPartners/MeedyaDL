/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/ContextMenu.test.tsx - Unit tests for ContextMenu keyboard support
 *
 * Before the a11y audit fix, `ContextMenu` was mouse-only: opening it moved
 * focus nowhere, there were no arrow keys, and closing it (by any means)
 * left focus stranded wherever it happened to be. Since this menu is the
 * only way to reorder the download queue, delete a row, or retry without
 * the wrapper, that made those actions completely unreachable from the
 * keyboard.
 *
 * These tests drive the menu the way a keyboard-only person would --
 * Tab to a trigger, activate it, use arrow keys, press Escape -- and
 * check the actual DOM focus target (`document.activeElement`) at each
 * step, not just that the right callback fired.
 *
 * @see src/components/common/ContextMenu.tsx - The component under test
 */

import { useState } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { ContextMenu, type ContextMenuItem } from '@/components/common/ContextMenu';

/**
 * A small harness that mirrors how every real caller uses ContextMenu:
 * a trigger button opens the menu, and the menu closing (for any reason)
 * unmounts it. This is necessary to test focus RESTORATION, since that
 * happens in the component's unmount cleanup -- calling `onClose` alone
 * (without actually unmounting) wouldn't exercise that path.
 */
function Harness({ items }: { items: ContextMenuItem[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button type="button" onClick={() => setOpen(true)}>
        Open menu
      </button>
      {open && (
        <ContextMenu items={items} x={10} y={10} onClose={() => setOpen(false)} />
      )}
    </div>
  );
}

function makeItems(overrides?: Partial<ContextMenuItem>[]): ContextMenuItem[] {
  const base: ContextMenuItem[] = [
    { label: 'Move to Top', onClick: vi.fn() },
    { label: 'Move Up', onClick: vi.fn(), disabled: true },
    { label: 'Move Down', onClick: vi.fn() },
    { label: 'Delete', onClick: vi.fn() },
  ];
  if (!overrides) return base;
  return base.map((item, i) => ({ ...item, ...overrides[i] }));
}

describe('ContextMenu keyboard support (a11y audit)', () => {
  it('moves focus to the first enabled item as soon as the menu opens', () => {
    render(<Harness items={makeItems()} />);

    fireEvent.click(screen.getByText('Open menu'));

    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Move to Top' }));
  });

  it('skips disabled items and wraps around when moving with ArrowDown / ArrowUp', () => {
    render(<Harness items={makeItems()} />);
    fireEvent.click(screen.getByText('Open menu'));

    const top = screen.getByRole('menuitem', { name: 'Move to Top' });
    const down = screen.getByRole('menuitem', { name: 'Move Down' });
    const del = screen.getByRole('menuitem', { name: 'Delete' });

    // Starts on "Move to Top". ArrowDown must skip the disabled
    // "Move Up" entry and land on "Move Down".
    expect(document.activeElement).toBe(top);
    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(down);

    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(del);

    // One more ArrowDown from the last item wraps back to the first
    // (skipping the disabled entry again on the way).
    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(top);

    // ArrowUp from the first item wraps to the last.
    fireEvent.keyDown(document, { key: 'ArrowUp' });
    expect(document.activeElement).toBe(del);
  });

  it('Home and End jump to the first and last enabled item', () => {
    render(<Harness items={makeItems()} />);
    fireEvent.click(screen.getByText('Open menu'));

    fireEvent.keyDown(document, { key: 'End' });
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Delete' }));

    fireEvent.keyDown(document, { key: 'Home' });
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Move to Top' }));
  });

  it('Escape closes the menu and returns focus to whatever opened it', () => {
    render(<Harness items={makeItems()} />);
    const trigger = screen.getByText('Open menu');

    // jsdom's fireEvent.click does not replicate a real browser's
    // "clicking a button also focuses it" behaviour, so the trigger is
    // focused explicitly here -- this is what actually happens for
    // both a mouse click on a real button and a keyboard activation
    // (Enter/Space on a focused button), which is the scenario this
    // test is standing in for either way.
    trigger.focus();
    fireEvent.click(trigger);
    expect(screen.getByRole('menu')).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });

    // The menu is gone, and focus is back on the button that opened it --
    // not left on the document body or nowhere at all.
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(document.activeElement).toBe(trigger);
  });

  it('clicking a menu item runs its action, closes the menu, and restores focus', () => {
    const onClick = vi.fn();
    render(<Harness items={makeItems([{ onClick }])} />);
    const trigger = screen.getByText('Open menu');

    trigger.focus(); // see the comment in the Escape test above
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Move to Top' }));

    expect(onClick).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(document.activeElement).toBe(trigger);
  });

  it('has a real, translated accessible name rather than nothing', () => {
    render(<Harness items={makeItems()} />);
    fireEvent.click(screen.getByText('Open menu'));

    const menu = screen.getByRole('menu');
    expect(menu.getAttribute('aria-label')).toBeTruthy();
  });
});
