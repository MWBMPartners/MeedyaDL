/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file TemplateBuilder.test.tsx -- The builder's label names whatever is
 * being edited, in both modes (#1439).
 *
 * The raw-text mode has one input, so the label points at it with
 * `htmlFor`. The visual (chip) mode has no single input, so the chip area
 * is a group named by the label. Before #1439 the label pointed at an id
 * that only existed in raw mode, so in visual mode it named nothing.
 *
 * @see src/components/common/TemplateBuilder.tsx
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { TemplateBuilder } from '@/components/common/TemplateBuilder';

describe('TemplateBuilder label (#1439)', () => {
  it('names the chip area in visual mode', () => {
    render(<TemplateBuilder label="Album Folder" value="{album}" onChange={() => {}} />);
    expect(screen.getByRole('group', { name: 'Album Folder' })).toBeInTheDocument();
  });

  it('points at the text box in raw mode', () => {
    render(<TemplateBuilder label="Album Folder" value="{album}" onChange={() => {}} />);
    fireEvent.click(screen.getByText('Edit Raw'));
    expect(screen.getByLabelText('Album Folder')).toHaveValue('{album}');
  });
});
