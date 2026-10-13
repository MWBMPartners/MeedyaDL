// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file markdownHeadings.ts -- renumber the headings of Markdown shown
 * inside a page, so they fit under the page's own headings.
 *
 * Every screen's title is its one `<h1>` (the shared page header). Markdown
 * shown inside a screen brings headings of its own: each help page starts
 * with "# Title", which became a second `<h1>` under the Help screen's
 * title, and release notes jump from "#" straight to "###". Screen readers
 * use heading levels to move around a page, so a second `<h1>` or a skipped
 * level makes the page harder to find your way around.
 *
 * This remark plugin keeps the order of the headings and maps the levels a
 * document actually uses onto consecutive levels starting at `top`. For a
 * help page with `top: 2`: "#" becomes `<h2>`, "##" `<h3>`, and so on. For
 * release notes using "#" and "###" with `top: 3`: `<h3>` and `<h4>`.
 * Levels never go past 6, the deepest HTML has.
 *
 * It only changes heading levels; the words, and everything else in the
 * document, are left exactly as written.
 */

/** The small part of a Markdown syntax tree node this plugin reads. */
interface MarkdownNode {
  type: string;
  depth?: number;
  children?: MarkdownNode[];
}

/** Options for {@link remarkFitHeadings}. */
export interface FitHeadingsOptions {
  /** The level the document's highest heading should become (1-6). */
  top: number;
}

/**
 * Remark plugin: renumber headings to start at `options.top`, with no
 * level skipped. Use as `remarkPlugins={[[remarkFitHeadings, { top: 2 }]]}`.
 */
export function remarkFitHeadings(options: FitHeadingsOptions) {
  return (tree: MarkdownNode): void => {
    const headings: MarkdownNode[] = [];
    const walk = (node: MarkdownNode): void => {
      if (node.type === 'heading' && typeof node.depth === 'number') headings.push(node);
      node.children?.forEach(walk);
    };
    walk(tree);

    const levels = [...new Set(headings.map((h) => h.depth as number))].sort((a, b) => a - b);
    const newLevel = new Map(levels.map((level, i) => [level, Math.min(6, options.top + i)]));
    for (const heading of headings) {
      heading.depth = newLevel.get(heading.depth as number);
    }
  };
}
