// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Guards against "phantom" Tailwind colour classes.
 *
 * Tailwind CSS v4 does not warn or error when a class name references a
 * colour it does not know about. `bg-accent-primary` -- a colour that was
 * never defined anywhere in `tailwind.config.js` or any theme file --
 * simply generates no CSS rule at all, silently. The element still
 * renders; it just has no background, no text colour, whatever the class
 * was meant to set. An accessibility audit of this app found six
 * different shapes of exactly this defect (`bg-interactive-accent`,
 * `text-accent-primary`, `bg-surface-tertiary` before it was defined,
 * `bg-input-bg`, `border-border-default`, `bg-surface-base`, and more) --
 * each one an invisible-because-undefined control: a button with no
 * background, a link with no colour, a focus ring nobody could see.
 *
 * How this test tells a real colour class from a typo, without hand
 * -maintaining a list of "every colour MeedyaDL defines" that would drift
 * out of date the next time someone adds one: it runs the SAME Tailwind
 * build this app's own `npm run build` runs -- the real
 * `tailwind.config.js`, the real theme CSS, scanning the real `src/`
 * tree -- and gets back the real compiled CSS. A class that compiles to
 * an actual rule is fine, whatever its name (this also means a
 * legitimate non-colour utility that happens to share a colour utility's
 * prefix, like `bg-cover` or `text-center`, is never mistaken for a
 * broken colour class -- it compiles to a real rule too, so it is never
 * flagged). A class that does NOT appear anywhere in that compiled CSS
 * is exactly the shape of bug this test exists to catch.
 *
 * Separately, this file scans every `.ts`/`.tsx` file under `src/` for
 * anything that LOOKS like a colour-utility class (`bg-`, `text-`,
 * `border-`, `ring-`, `ring-offset-`, `outline-`, `accent-`,
 * `decoration-`, `divide-`, `caret-`, `fill-`, `stroke-` followed by a
 * name) inside a quoted string -- covering both inline `className="..."`
 * JSX attributes AND plain string constants that get assembled into a
 * className elsewhere (the shape `StatusPill.tsx`'s `colorClasses` table
 * uses, which is exactly where several of the real bugs were hiding).
 * Restricting to quoted strings (rather than the whole file's raw text)
 * is deliberate: several of this file's own review comments mention the
 * OLD, broken class names as history ("was `bg-interactive-accent`, now
 * `bg-accent`") -- scanning comment prose would flag those as if they
 * were still in use.
 */

import { describe, it, expect, beforeAll } from 'vitest';
import postcss from 'postcss';
import tailwindPostcss from '@tailwindcss/postcss';

/**
 * Node's `fs`/`path`/`url` modules, loaded through a plain dynamic
 * `import()` rather than a typed `import ... from 'node:fs'`.
 *
 * This project's tsconfig deliberately carries no `@types/node` -- it's
 * a browser/WebView app, and its own `types` array is an explicit,
 * short allowlist (`vitest/globals`) rather than TypeScript's default
 * "pick up everything under @types/*". Adding `@types/node` just so
 * this one test file can read the filesystem would apply Node's
 * ambient globals to the WHOLE program, not just this file -- and
 * Node's lib redefines things the DOM lib already defines differently
 * (`setTimeout`'s return type is the classic one), which is a real risk
 * to introduce project-wide for one test file's convenience. Casting
 * the dynamic imports to `any` keeps that risk contained here, at the
 * cost of losing type-checking on these specific calls -- an
 * acceptable trade for a test that only ever runs under Node anyway.
 */
let fs: {
  readFileSync: (path: string, encoding: string) => string;
  readdirSync: (path: string) => string[];
  statSync: (path: string) => { isDirectory: () => boolean };
};
let path: {
  join: (...parts: string[]) => string;
  resolve: (...parts: string[]) => string;
  relative: (from: string, to: string) => string;
  dirname: (p: string) => string;
};

/** Project root -- two directories up from this file (src/lib/*.test.ts). */
let PROJECT_ROOT: string;
let SRC_ROOT: string;

/**
 * This guard's own file is deliberately excluded from the corpus it
 * scans. Two reasons: it names deliberately-fake class names in its own
 * "prove the checker can tell real from fake" tests (see below), which
 * are not application bugs to report; and `extractQuotedStrings`'s own
 * regex LITERAL contains raw `"` / `'` / `` ` `` characters as regex
 * syntax, not as real string delimiters -- `stripComments` (a simple
 * character-by-character scanner, not a real JS parser) cannot tell
 * that apart from an actual string starting, and scanning this file's
 * own source with it corrupts its quote-tracking for everything after
 * that line. Excluding this one file sidesteps needing a real parser
 * just to check itself.
 */
let SELF_PATH: string;

/**
 * Loads the Node modules and resolves the paths above. Done once, in
 * `beforeAll`, rather than at module load time -- top-level dynamic
 * `import()` plus the untyped `any` casts read awkwardly at the very
 * top of the file, whereas every consumer of these values below only
 * ever runs from inside (or after) `beforeAll`, so an ordinary async
 * setup step is all this needs.
 */
async function loadNodeModules(): Promise<void> {
  // The module name is passed through a variable, not a literal, so
  // TypeScript treats the whole expression as `Promise<any>` and never
  // tries to resolve type declarations for it -- a plain
  // `import('fs')` literal makes `tsc` go looking for an ambient module
  // declaration for "fs", which does not exist without @types/node (see
  // the comment above `fs`'s declaration for why that is not added).
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

  SELF_PATH = path.resolve(fileURLToPath(import.meta.url));
  PROJECT_ROOT = path.resolve(path.dirname(SELF_PATH), '..', '..');
  SRC_ROOT = path.join(PROJECT_ROOT, 'src');
}

/**
 * Recursively lists every `.ts` / `.tsx` file under `dir`.
 * A hand-rolled walk rather than a glob dependency, so this test has no
 * extra package to keep installed -- `fs.readdirSync` is all Node needs.
 */
function listSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of fs.readdirSync(dir)) {
    const full = path.join(dir, entry);
    const stat = fs.statSync(full);
    if (stat.isDirectory()) {
      out.push(...listSourceFiles(full));
    } else if (/\.(ts|tsx)$/.test(entry) && path.resolve(full) !== SELF_PATH) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Removes `/* ... *\/` and `// ...` comments from `source`, while leaving
 * string/template literals completely alone (so a URL like
 * `'https://example.com'` inside a real string is never mistaken for the
 * start of a line comment, and so line numbers stay put -- a block
 * comment's content is blanked out character-by-character rather than
 * deleted, keeping every `\n` exactly where it was).
 *
 * Why this matters here specifically: several of the comments THIS file
 * and the fixes it verifies add explain a bug by naming the broken class
 * in backticks, e.g. "`bg-interactive-accent` was never a defined
 * colour". Without stripping comments first, the string-extraction pass
 * below would see that backtick pair as if it were a real template
 * literal and "discover" the very class name the comment says NOT to
 * use, as though it were still live in the code.
 */
function stripComments(source: string): string {
  let out = '';
  let i = 0;
  const n = source.length;
  while (i < n) {
    const ch = source[i];
    const two = source.slice(i, i + 2);
    if (ch === '"' || ch === "'" || ch === '`') {
      // Copy the whole string/template literal verbatim (quotes
      // included) so the extraction pass below still sees it -- only
      // comments get removed, not code.
      const quote = ch;
      out += ch;
      i++;
      while (i < n) {
        if (source[i] === '\\') {
          out += source[i] + (source[i + 1] ?? '');
          i += 2;
          continue;
        }
        out += source[i];
        const isClosingQuote = source[i] === quote;
        i++;
        if (isClosingQuote) break;
      }
      continue;
    }
    if (two === '/*') {
      i += 2;
      while (i < n && source.slice(i, i + 2) !== '*/') {
        out += source[i] === '\n' ? '\n' : ' ';
        i++;
      }
      i += 2;
      continue;
    }
    if (two === '//') {
      while (i < n && source[i] !== '\n') i++;
      continue;
    }
    out += ch;
    i++;
  }
  return out;
}

/**
 * Extracts the inner content of every single-quoted, double-quoted, or
 * backtick-delimited string in `source`. Class names -- whether an inline
 * JSX `className="..."` or a plain string constant like
 * `colorClasses: '...'` -- always live inside one of these three quote
 * styles.
 */
function extractQuotedStrings(source: string): string[] {
  const strings: string[] = [];
  const re = /"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(source)) !== null) {
    strings.push(match[1] ?? match[2] ?? match[3] ?? '');
  }
  return strings;
}

/**
 * The Tailwind utility prefixes that take a COLOUR as their value.
 * `ring-offset` is listed before `ring` so the alternation tries the
 * longer prefix first (otherwise `ring-offset-accent` would match as
 * prefix `ring` + name `offset-accent`, which is still fine for the
 * membership check below, but keeping the real prefix makes failure
 * messages read correctly).
 */
const COLOR_PREFIXES = [
  'ring-offset',
  'bg',
  'text',
  'border',
  'ring',
  'outline',
  'accent',
  'decoration',
  'divide',
  'caret',
  'fill',
  'stroke',
];

/**
 * Matches a complete class token: an optional chain of Tailwind variant
 * prefixes (`dark:`, `hover:`, `focus-visible:`, ...), one of the colour
 * prefixes above, a name starting with a letter (so a bare numeric
 * utility like `border-2` can never match -- there is no letter for
 * `[a-zA-Z]` to land on), and an optional `/NN` opacity suffix.
 *
 * The whole thing is required to be bounded by whitespace or the start/
 * end of the string it's found in -- a real `className` string is a
 * space-separated list of tokens, so a genuine class is always bounded
 * that way. This is deliberately stricter than a plain `\b` word
 * boundary: `\b` would also fire in the middle of an ordinary English
 * phrase like "plain-text-editable" (there IS a word boundary right
 * before "text", since "-" isn't a word character) and wrongly treat it
 * as the class `text-editable`. Requiring whitespace/edge on both sides
 * rules that out, because compound English words like that don't have
 * spaces around their middle segment.
 */
const CANDIDATE_RE = new RegExp(
  `(?<=^|\\s)(?:[a-zA-Z][\\w-]*:)*(?:${COLOR_PREFIXES.join('|')})-[a-zA-Z][a-zA-Z0-9-]*(?:/\\d+)?(?=\\s|$)`,
  'g'
);

/** One occurrence of a candidate colour class, with enough location info
 * to report a useful failure. */
interface Candidate {
  className: string;
  file: string;
  line: number;
}

/** Finds every candidate colour class in every source file. */
function findCandidates(files: string[]): Candidate[] {
  const candidates: Candidate[] = [];
  for (const file of files) {
    // Comments are stripped from the WHOLE file before splitting into
    // lines -- a block comment can span many lines, so stripping had to
    // happen before line-splitting, not per-line.
    const text = stripComments(fs.readFileSync(file, 'utf8'));
    const lines = text.split('\n');
    for (let i = 0; i < lines.length; i++) {
      for (const str of extractQuotedStrings(lines[i])) {
        const matches = str.match(CANDIDATE_RE);
        if (!matches) continue;
        for (const className of matches) {
          candidates.push({ className, file: path.relative(PROJECT_ROOT, file), line: i + 1 });
        }
      }
    }
  }
  return candidates;
}

/**
 * Builds the app's real Tailwind CSS: the real `tailwind.config.js`,
 * scanning the real `src/` tree, exactly like `npm run build` does. The
 * `from` path just needs to sit somewhere Tailwind can resolve
 * `@config`'s relative path from -- it never has to exist on disk.
 */
async function buildRealTailwindCss(): Promise<string> {
  const configPath = path.join(PROJECT_ROOT, 'tailwind.config.js');
  const input = `@import "tailwindcss";\n@config "${configPath}";\n`;
  const from = path.join(SRC_ROOT, 'lib', '__tailwind-guard-input.css');
  const result = await postcss([tailwindPostcss()]).process(input, { from });
  return result.css;
}

/**
 * Checks whether `className` compiled to a real rule in `compiledCss`.
 * Tailwind/LightningCSS escape characters like `/`, `[`, `]`, `(`, `)`,
 * `:` and `.` inside the selector with a backslash (e.g. `bg-accent\/10`).
 * Rather than reproducing that escaping to build the exact selector to
 * search for, this strips every backslash-escape from the compiled CSS
 * first (`\/` -> `/`, `\[` -> `[`, and so on) and searches the
 * now-plain text instead -- simpler, and exactly as accurate.
 *
 * The lookahead `(?![\w-])` after the class name stops `bg-accent` from
 * being reported as "found" merely because `bg-accent-hover` exists
 * somewhere in the file -- a plain substring search would get that
 * wrong.
 */
function isKnownTailwindClass(className: string, compiledCss: string): boolean {
  const plain = compiledCss.replace(/\\(.)/g, '$1');
  const escaped = className.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`\\.${escaped}(?![\\w-])`).test(plain);
}

describe('Tailwind colour classes actually exist (Fix 4 guard)', () => {
  let compiledCss: string;
  let candidates: Candidate[];

  beforeAll(async () => {
    await loadNodeModules();
    compiledCss = await buildRealTailwindCss();
    candidates = findCandidates(listSourceFiles(SRC_ROOT));
  }, 30_000);

  it('found a non-trivial number of candidate colour classes to check (sanity check on the scanner itself)', () => {
    // If this is ever near zero, the scanner broke, not the app --
    // there are hundreds of legitimate `bg-*`/`text-*`/etc. usages
    // across the component tree.
    expect(candidates.length).toBeGreaterThan(50);
  });

  it('every colour class used anywhere in src/ actually compiles to a real Tailwind rule', () => {
    const missing = candidates.filter((c) => !isKnownTailwindClass(c.className, compiledCss));
    // De-duplicate by class name for a readable failure message, but
    // still show every distinct file:line an offending class appears at.
    const byClass = new Map<string, string[]>();
    for (const m of missing) {
      const locations = byClass.get(m.className) ?? [];
      locations.push(`${m.file}:${m.line}`);
      byClass.set(m.className, locations);
    }
    const report = [...byClass.entries()]
      .map(([className, locations]) => `  ${className}\n    ${locations.join('\n    ')}`)
      .join('\n');
    expect(missing, `Undefined colour class(es) found:\n${report}`).toHaveLength(0);
  });

  describe('the checker itself (proves the guard can tell real from fake)', () => {
    it('recognises a real, currently-used colour class', () => {
      expect(isKnownTailwindClass('bg-accent', compiledCss)).toBe(true);
      expect(isKnownTailwindClass('text-status-error-text', compiledCss)).toBe(true);
      expect(isKnownTailwindClass('bg-surface-tertiary', compiledCss)).toBe(true);
    });

    it('rejects a colour class that was never defined', () => {
      // Built from two pieces so this literal string never appears
      // whole in the source text -- otherwise the whole-repo scan
      // above would trip over this very line.
      const neverDefined = 'bg-' + 'definitely-not-a-real-colour-token-xyz';
      expect(isKnownTailwindClass(neverDefined, compiledCss)).toBe(false);
    });

    it('does not get confused by a class name that is a prefix of a real one', () => {
      // `bg-accent` is real; `bg-accent-doesnotexist` must not be
      // reported as found just because `bg-accent` is a substring of it.
      const lookalike = 'bg-accent-' + 'doesnotexist';
      expect(isKnownTailwindClass(lookalike, compiledCss)).toBe(false);
    });
  });
});
