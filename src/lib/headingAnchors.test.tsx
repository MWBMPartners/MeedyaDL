// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';
import ReactMarkdown from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import { createSlugger, githubSlugBase, rehypeHeadingIds, scrollToHeading } from './headingAnchors';

describe('githubSlugBase', () => {
  it('lowercases, hyphenates spaces and drops punctuation like GitHub', () => {
    expect(githubSlugBase('Setup Guide')).toBe('setup-guide');
    expect(githubSlugBase('Step 5: Copy the private key')).toBe('step-5-copy-the-private-key');
    expect(githubSlugBase('Standard M4A/MP4 atoms')).toBe('standard-m4amp4-atoms');
    expect(githubSlugBase('Download Manifests (.meedyadl files)')).toBe(
      'download-manifests-meedyadl-files'
    );
    expect(githubSlugBase('Under_score and-hyphen')).toBe('under_score-and-hyphen');
  });
});

describe('createSlugger', () => {
  it('numbers repeated headings -1, -2', () => {
    const slug = createSlugger();
    expect(slug('FAQ')).toBe('faq');
    expect(slug('FAQ')).toBe('faq-1');
    expect(slug('FAQ')).toBe('faq-2');
    expect(slug('Other')).toBe('other');
  });
});

describe('scrolling to a heading', () => {
  const page = (md: string) =>
    render(
      <div data-testid="box">
        <ReactMarkdown rehypePlugins={[rehypeRaw, rehypeSanitize, rehypeHeadingIds]}>
          {md}
        </ReactMarkdown>
      </div>
    );

  it('gives rendered headings ids, including repeats', () => {
    const { container } = page('# Title\n\n## Setup Guide\n\n## Setup Guide\n');
    expect(container.querySelector('h2#setup-guide')).not.toBeNull();
    expect(container.querySelector('h2#setup-guide-1')).not.toBeNull();
  });

  it('scrolls to a heading in the container and reports it found', () => {
    const { container, getByTestId } = page('## Setup Guide\n\ntext');
    const heading = container.querySelector('#setup-guide') as HTMLElement;
    const scroll = vi.fn();
    heading.scrollIntoView = scroll;
    expect(scrollToHeading(getByTestId('box'), 'setup-guide')).toBe(true);
    expect(scroll).toHaveBeenCalledOnce();
  });

  it('reports not found for an unknown name or a non-heading element', () => {
    const { getByTestId } = page('## Setup Guide\n\ntext');
    expect(scrollToHeading(getByTestId('box'), 'nope')).toBe(false);
    expect(scrollToHeading(null, 'setup-guide')).toBe(false);
    expect(scrollToHeading(getByTestId('box'), '')).toBe(false);
  });
});
