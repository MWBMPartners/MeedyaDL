// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Checks, by reading the real theme CSS files, that every place a
 * button/pill/label's text sits ON TOP of a coloured fill (the accent
 * colour, or one of the four status colours) actually has enough
 * contrast to be read.
 *
 * Why this exists: a fill colour is a different hex value in almost
 * every theme -- macOS's accent is not Windows' accent, and the
 * colour-blind palettes recolour the status fills entirely. A single
 * hard-coded text colour that happens to work against ONE theme's fill
 * can silently fail against another's, and nothing in TypeScript or
 * Tailwind will catch that -- CSS custom properties are just strings to
 * the build. This test is the guard: it reads each theme file the same
 * way the browser will, pulls out every place a fill colour and its
 * matching text colour are declared together, and computes the actual
 * WCAG contrast ratio between them.
 *
 * How pairing works: this app deliberately keeps a fill's colour and
 * its text colour declared in the SAME CSS rule block wherever either
 * one is overridden (see the long comment in base.css for why -- in
 * short, a single colour cannot be correct against every fill it might
 * sit on, so each theme states its own pairing explicitly rather than
 * silently inheriting one half of the pair from somewhere else). That
 * means this test does not need to simulate the full CSS cascade across
 * files -- it only has to look inside one rule block at a time for a
 * fill property (e.g. `--accent`) and its matching text property (e.g.
 * `--text-on-accent`), and check the two together whenever both appear
 * in that same block. A theme that doesn't override a pairing (most
 * platforms don't touch the status colours at all, for example) is
 * still covered: the SAME pairing is checked wherever it IS declared --
 * base.css's default block -- and every other theme either restates
 * that same pairing explicitly (self-documenting; checked again there)
 * or leaves both halves alone (nothing to silently drift out of sync).
 *
 * WCAG AA requires 4.5:1 for ordinary text, or 3:1 for large text
 * (18.66px bold, or 24px regular). Every real usage of these tokens in
 * this app's components is ordinary-sized button/pill/label text (an
 * icon on a fill only strictly needs the 3:1 "graphic" rule, but this
 * test holds every pairing to the stricter 4.5:1 anyway -- a colour
 * that passes text-contrast always passes icon-contrast too, and
 * holding one bar for the whole token system is simpler than tracking
 * which specific components use a token for text versus for an icon).
 */

import { describe, it, expect, beforeAll } from 'vitest';
import postcss from 'postcss';

/**
 * Node's `fs`/`path` modules, loaded through a plain dynamic `import()`
 * rather than a typed `import ... from 'node:fs'`, for the same reason
 * `tailwindColorClasses.test.ts` does this -- see the comment on its
 * own `fs`/`path` declarations for the full explanation (this project's
 * tsconfig deliberately carries no `@types/node`, and this is a test
 * file, so it only runs under Node regardless).
 */
let fs: {
  readFileSync: (path: string, encoding: string) => string;
  readdirSync: (path: string) => string[];
};
let path: {
  join: (...parts: string[]) => string;
  resolve: (...parts: string[]) => string;
  basename: (p: string) => string;
  dirname: (p: string) => string;
};

/** Directory holding every theme CSS file this test reads. */
let THEME_DIR: string;

async function loadNodeModules(): Promise<void> {
  const importByName = (name: string) => import(/* @vite-ignore */ name);
  fs = (await importByName('fs')) as unknown as typeof fs;
  const pathModule = (await importByName('path')) as unknown as typeof path & {
    default?: typeof path;
  };
  path = pathModule.default ?? pathModule;
  const urlModule = (await importByName('url')) as unknown as {
    fileURLToPath: (url: string) => string;
    default?: { fileURLToPath: (url: string) => string };
  };
  const fileURLToPath = urlModule.fileURLToPath ?? urlModule.default!.fileURLToPath;
  THEME_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)));
}

/**
 * Converts one sRGB channel (0-255) to its linear-light value, per the
 * WCAG 2 relative luminance formula.
 * @see https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
 */
function srgbChannelToLinear(channel255: number): number {
  const s = channel255 / 255;
  return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

/** Parses a `#rrggbb` string into its WCAG relative luminance (0-1). */
function relativeLuminance(hex: string): number {
  const clean = hex.trim().replace('#', '');
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  const R = srgbChannelToLinear(r);
  const G = srgbChannelToLinear(g);
  const B = srgbChannelToLinear(b);
  return 0.2126 * R + 0.7152 * G + 0.0722 * B;
}

/**
 * WCAG contrast ratio between two colours. Symmetric by definition --
 * it only depends on which of the two luminances is lighter, not which
 * colour is "the text" and which is "the background".
 * @see https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio
 */
function contrastRatio(hexA: string, hexB: string): number {
  const lA = relativeLuminance(hexA);
  const lB = relativeLuminance(hexB);
  const lighter = Math.max(lA, lB);
  const darker = Math.min(lA, lB);
  return (lighter + 0.05) / (darker + 0.05);
}

/** Matches a plain `#rrggbb` literal -- the only shape these particular
 * tokens are ever declared as in this codebase (never `var()`, never
 * `rgba()`). A value that doesn't match this shape is simply not a pair
 * this test can check, so it's skipped rather than treated as a hex
 * colour it isn't. */
const HEX_RE = /^#[0-9a-fA-F]{6}$/;

/** The minimum ratio ordinary-sized text (or an icon, which needs even
 * less) must clear. See the file header for why this test applies the
 * stricter of the two WCAG AA thresholds everywhere. */
const MIN_TEXT_CONTRAST = 4.5;

/**
 * Every (fill property, its matching text property) pair this app
 * defines. `--accent` only ever has one text partner because a theme
 * only ever has one accent fill active at a time; the four status
 * colours each get their OWN text partner because -- unlike accent -- a
 * colour-blind theme can need a different answer for one status colour
 * than for another within the very same theme (see the long comment
 * on `--text-on-status-success` in base.css).
 */
const FILL_TEXT_PAIRS: ReadonlyArray<readonly [fillProp: string, textProp: string]> = [
  ['--accent', '--text-on-accent'],
  ['--status-success', '--text-on-status-success'],
  ['--status-warning', '--text-on-status-warning'],
  ['--status-error', '--text-on-status-error'],
  ['--status-info', '--text-on-status-info'],
];

/** One occurrence of a fill+text pairing found declared together inside
 * a single CSS rule block, with enough location info to report a
 * useful failure. */
interface FoundPair {
  file: string;
  selector: string;
  fillProp: string;
  fillValue: string;
  textProp: string;
  textValue: string;
}

/**
 * Parses one theme CSS file with postcss (a real CSS parser -- not a
 * regex over the text) and returns every fill+text pairing declared
 * together inside the same rule block, wherever both halves of a pair
 * from `FILL_TEXT_PAIRS` appear as plain hex literals in that block.
 * Walks every rule regardless of nesting depth, so rules inside
 * `@media` blocks are covered exactly like top-level ones.
 */
function extractPairsFromFile(filePath: string): FoundPair[] {
  const css = fs.readFileSync(filePath, 'utf8');
  const root = postcss.parse(css);
  const found: FoundPair[] = [];

  root.walkRules((rule) => {
    const declared = new Map<string, string>();
    for (const node of rule.nodes) {
      if (node.type === 'decl' && node.prop.startsWith('--')) {
        declared.set(node.prop, node.value.trim());
      }
    }
    for (const [fillProp, textProp] of FILL_TEXT_PAIRS) {
      const fillValue = declared.get(fillProp);
      const textValue = declared.get(textProp);
      if (fillValue && textValue && HEX_RE.test(fillValue) && HEX_RE.test(textValue)) {
        found.push({
          file: path.basename(filePath),
          selector: rule.selector,
          fillProp,
          fillValue,
          textProp,
          textValue,
        });
      }
    }
  });

  return found;
}

describe('every "text sitting on a fill" pairing in the theme CSS clears WCAG AA contrast', () => {
  let allPairs: FoundPair[];

  beforeAll(async () => {
    await loadNodeModules();
    const files = fs
      .readdirSync(THEME_DIR)
      .filter((f) => f.endsWith('.css'))
      .map((f) => path.join(THEME_DIR, f));
    allPairs = files.flatMap(extractPairsFromFile);
  });

  it('found a non-trivial number of pairings to check (sanity check on the scanner itself)', () => {
    // If this is ever near zero, the scanner broke (a CSS restructure
    // changed how these tokens are declared together), not the app --
    // every platform, both light/dark modes, high-contrast, and all
    // three colour-blind variants each declare several of these pairs.
    expect(allPairs.length).toBeGreaterThan(20);
  });

  it('every pairing reaches at least 4.5:1 contrast', () => {
    const failures = allPairs
      .map((p) => ({ ...p, ratio: contrastRatio(p.fillValue, p.textValue) }))
      .filter((p) => p.ratio < MIN_TEXT_CONTRAST);

    const report = failures
      .map(
        (f) =>
          `  ${f.file} ${f.selector}\n` +
          `    ${f.fillProp}: ${f.fillValue}  vs  ${f.textProp}: ${f.textValue}` +
          `  ->  ${f.ratio.toFixed(2)}:1 (needs ${MIN_TEXT_CONTRAST}:1)`
      )
      .join('\n');

    expect(failures, `Contrast failure(s) found:\n${report}`).toHaveLength(0);
  });

  describe('the checker itself (proves the maths can tell pass from fail)', () => {
    it('agrees with the known-good pairing: black on the ordinary light-mode success fill', () => {
      // #34c759 is the ordinary theme's success fill; base.css computes
      // this exact pair at 9.46:1 in its own comments.
      expect(contrastRatio('#000000', '#34c759')).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
    });

    it('agrees with the known-bad pairing: black on the deuteranopia/protanopia success fill', () => {
      // #0077b6 is the darker blue deuteranopia/protanopia remap success
      // to. This is exactly the near-miss failure the four-separate-
      // tokens design in base.css exists to avoid -- black measures
      // 4.31:1 here, just under the 4.5:1 floor, which is why that
      // theme's --text-on-status-success is white, not black.
      expect(contrastRatio('#000000', '#0077b6')).toBeLessThan(MIN_TEXT_CONTRAST);
    });

    it('is symmetric -- "A on B" and "B on A" give the same ratio', () => {
      expect(contrastRatio('#000000', '#34c759')).toBeCloseTo(contrastRatio('#34c759', '#000000'), 10);
    });
  });
});
