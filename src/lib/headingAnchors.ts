// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file headingAnchors.ts -- give headings in shown Markdown the same
 * link targets GitHub gives them, and jump to one when a link asks.
 *
 * A help page may link to one of its own headings, for example
 * `[Setup Guide](#setup-guide)`. On GitHub that scrolls to the heading
 * named "Setup Guide". Inside the app the headings had no targets, so the
 * link did nothing. This file does two small jobs:
 *
 * 1. {@link githubSlug} and {@link rehypeHeadingIds} work out the target
 *    name of each heading the way GitHub does (lower case, spaces become
 *    hyphens, punctuation dropped, a repeated name gets -1, -2, ...) and
 *    set it as the heading's `id`.
 * 2. {@link scrollToHeading} finds a heading by that name inside one
 *    container and scrolls it into view.
 *
 * The same rule is repeated in `tools/audit-checks/check_help_topics.py`,
 * which reports a link to a heading that does not exist. If you change the
 * rule here, change it there too.
 */

/**
 * The target name for a piece of heading text, before any repeat is
 * numbered. Mirrors GitHub: lower case, then every character that is not a
 * letter, mark, number, underscore, hyphen or space is dropped, then each
 * space becomes a hyphen.
 */
export function githubSlugBase(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, '')
    .replace(/ /g, '-');
}

/**
 * Hands out target names for the headings of ONE page, in reading order,
 * numbering repeats the way GitHub does ("faq", "faq-1", "faq-2"). Make a
 * new one for every page.
 */
export function createSlugger(): (text: string) => string {
  const used = new Map<string, number>();
  return (text: string): string => {
    const base = githubSlugBase(text);
    let result = base;
    while (used.has(result)) {
      used.set(base, (used.get(base) ?? 0) + 1);
      result = `${base}-${used.get(base)}`;
    }
    used.set(result, 0);
    return result;
  };
}

/** The small part of a rendered-HTML tree node these helpers read. */
interface HtmlNode {
  type: string;
  tagName?: string;
  value?: string;
  properties?: Record<string, unknown>;
  children?: HtmlNode[];
}

/** All the plain text inside a node, in order. */
function textOf(node: HtmlNode): string {
  if (node.type === 'text') return node.value ?? '';
  return (node.children ?? []).map(textOf).join('');
}

/**
 * Rehype plugin: set a GitHub-style `id` on every heading that has none.
 * Place it AFTER the sanitising step, which would otherwise rename ids.
 */
export function rehypeHeadingIds() {
  return (tree: HtmlNode): void => {
    const slug = createSlugger();
    const walk = (node: HtmlNode): void => {
      if (node.type === 'element' && node.tagName && /^h[1-6]$/.test(node.tagName)) {
        node.properties = node.properties ?? {};
        if (!node.properties.id) node.properties.id = slug(textOf(node));
      }
      node.children?.forEach(walk);
    };
    walk(tree);
  };
}

/**
 * Scrolls the heading whose `id` is `id` into view, looking only inside
 * `container` (so a name that happens to match something else in the app
 * is never used). Moves keyboard focus there as well, so a screen reader
 * announces where it landed.
 *
 * @returns True when a heading was found, false when there is none (the
 *   caller then treats the link some other way).
 */
export function scrollToHeading(container: HTMLElement | null, id: string): boolean {
  if (!container || !id) return false;
  const target = Array.from(container.querySelectorAll<HTMLElement>('[id]')).find(
    (el) => el.id === id && /^H[1-6]$/.test(el.tagName)
  );
  if (!target) return false;
  if (!target.hasAttribute('tabindex')) target.setAttribute('tabindex', '-1');
  target.focus({ preventScroll: true });
  if (typeof target.scrollIntoView === 'function') target.scrollIntoView({ block: 'start' });
  return true;
}
