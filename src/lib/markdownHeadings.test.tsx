// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Markdown shown inside a screen fits under the screen's own headings.
 *
 * Help pages start with "# Title", which used to become a second <h1> under
 * the Help screen's title; release notes jump from "#" to "###".
 */

import { renderToStaticMarkup } from 'react-dom/server';
import ReactMarkdown from 'react-markdown';
import { remarkFitHeadings } from './markdownHeadings';

const headingsIn = (markdown: string, top: number): string[] =>
  [
    ...renderToStaticMarkup(
      <ReactMarkdown remarkPlugins={[[remarkFitHeadings, { top }]]}>{markdown}</ReactMarkdown>
    ).matchAll(/<(h[1-6])>/g),
  ].map((m) => m[1]);

describe('remarkFitHeadings', () => {
  it('turns a help page "# Title" into an <h2>, and its sections into <h3>', () => {
    expect(headingsIn('# Title\n\n## Part\n\n### Detail\n\n## Part two', 2)).toEqual([
      'h2',
      'h3',
      'h4',
      'h3',
    ]);
  });

  it('closes a skipped level: "#" then "###" become consecutive levels', () => {
    expect(headingsIn('# MeedyaDL 1.2.3\n\n### What is new\n\n### What is fixed', 3)).toEqual([
      'h3',
      'h4',
      'h4',
    ]);
  });

  it('never goes deeper than <h6>', () => {
    expect(headingsIn('# a\n\n## b\n\n### c', 5)).toEqual(['h5', 'h6', 'h6']);
  });
});
