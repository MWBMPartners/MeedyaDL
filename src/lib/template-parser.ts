/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file template-parser.ts -- GAMDL template string parser and serializer.
 *
 * Converts between GAMDL Python format strings (e.g., `{album_artist}/{album}`)
 * and an array of typed segments used by the visual {@link TemplateBuilder}
 * component. Also exports the registry of available template variables and
 * common literal separators.
 *
 * The parser uses a single-pass scanner that correctly handles:
 * - Simple variables: `{artist}` -> variable segment
 * - Format specifiers: `{track:02d}` -> variable segment (value includes spec)
 * - Literal text: `Compilations/` -> literal segment
 * - Path separators: `/` is just literal text
 * - Adjacent variables: `{disc}-{track:02d}` -> variable, literal, variable
 * - Empty input: `""` -> `[]`
 * - Unclosed braces: `{broken` -> literal segment (graceful degradation)
 *
 * @see {@link src/components/common/TemplateBuilder.tsx} -- Visual consumer
 * @see {@link https://github.com/glomatico/gamdl#usage} -- GAMDL template docs
 */

// ============================================================
// Types
// ============================================================

/** A single segment in a parsed template. */
export interface TemplateSegment {
  /** Discriminator: 'variable' for GAMDL placeholders, 'literal' for static text. */
  type: 'variable' | 'literal';
  /**
   * For variables: the placeholder name including any format spec (e.g., `track:02d`).
   * For literals: the raw text (e.g., `Compilations/`, ` - `, ` `).
   */
  value: string;
}

// ============================================================
// Variable & Literal Registries
// ============================================================

/** Metadata for a GAMDL template variable, used by the add-segment menu. */
export interface TemplateVariable {
  /** The value inserted into the template (without braces), e.g., `track:02d`. */
  value: string;
  /** Human-readable label for the chip pill, e.g., `Track #`. */
  label: string;
  /** Short description shown in the add menu. */
  description: string;
  /** Category for grouping in the add menu. */
  category: 'track' | 'album' | 'playlist';
}

/** All available GAMDL template variables. */
export const TEMPLATE_VARIABLES: TemplateVariable[] = [
  { value: 'artist', label: 'Artist', description: 'Track artist name', category: 'track' },
  {
    value: 'album_artist',
    label: 'Album Artist',
    description: 'Album artist name',
    category: 'album',
  },
  { value: 'album', label: 'Album', description: 'Album title', category: 'album' },
  { value: 'title', label: 'Title', description: 'Track title', category: 'track' },
  {
    value: 'track:02d',
    label: 'Track #',
    description: 'Zero-padded track number',
    category: 'track',
  },
  { value: 'disc', label: 'Disc #', description: 'Disc number', category: 'track' },
  { value: 'year', label: 'Year', description: 'Release year', category: 'album' },
  { value: 'genre', label: 'Genre', description: 'Genre', category: 'album' },
  {
    value: 'playlist_artist',
    label: 'Playlist Artist',
    description: 'Playlist curator name',
    category: 'playlist',
  },
  {
    value: 'playlist_title',
    label: 'Playlist Title',
    description: 'Playlist name',
    category: 'playlist',
  },
];

/** Common literal segments offered as quick-insert options in the add menu. */
export const COMMON_LITERALS = [
  { value: '/', label: 'Path Separator (/)' },
  { value: ' ', label: 'Space' },
  { value: ' - ', label: 'Dash Separator ( - )' },
  { value: '-', label: 'Hyphen (-)' },
  { value: '_', label: 'Underscore (_)' },
];

/** Sample metadata values used for the live preview. */
export const SAMPLE_METADATA: Record<string, string> = {
  artist: 'Taylor Swift',
  album_artist: 'Taylor Swift',
  album: "1989 (Taylor's Version)",
  title: 'Style',
  'track:02d': '03',
  disc: '1',
  year: '2023',
  genre: 'Pop',
  playlist_artist: 'Apple Music',
  playlist_title: "Today's Hits",
};

// ============================================================
// Parser
// ============================================================

/**
 * Parse a GAMDL template string into an ordered array of segments.
 *
 * Uses a single-pass scanner. Variables are delimited by `{` and `}`.
 * Everything else is accumulated as literal text. Unclosed braces are
 * treated as literal text for graceful degradation.
 *
 * @param template - The raw template string (e.g., `{album_artist}/{album}`)
 * @returns Array of segments in order
 */
export function parseTemplate(template: string): TemplateSegment[] {
  if (!template) return [];

  const segments: TemplateSegment[] = [];
  let i = 0;
  let literal = '';

  while (i < template.length) {
    if (template[i] === '{') {
      // Flush any accumulated literal text
      if (literal) {
        segments.push({ type: 'literal', value: literal });
        literal = '';
      }

      // Find the matching closing brace
      const closeIdx = template.indexOf('}', i + 1);
      if (closeIdx === -1) {
        // Unclosed brace — treat the rest as literal text
        literal += template.slice(i);
        break;
      }

      // Extract the variable name (may include format spec like track:02d)
      const varName = template.slice(i + 1, closeIdx);
      segments.push({ type: 'variable', value: varName });
      i = closeIdx + 1;
    } else {
      literal += template[i];
      i++;
    }
  }

  // Flush any remaining literal text
  if (literal) {
    segments.push({ type: 'literal', value: literal });
  }

  return segments;
}

// ============================================================
// Serializer
// ============================================================

/**
 * Serialize a segment array back to a GAMDL template string.
 *
 * @param segments - Array of template segments
 * @returns The template string (e.g., `{album_artist}/{album}`)
 */
export function serializeTemplate(segments: TemplateSegment[]): string {
  return segments.map((s) => (s.type === 'variable' ? `{${s.value}}` : s.value)).join('');
}

// ============================================================
// Helpers
// ============================================================

/**
 * Get the human-readable label for a variable value.
 * Falls back to the raw value wrapped in braces for unknown variables.
 *
 * @param varValue - The variable value (e.g., `track:02d`, `artist`)
 * @returns Display label (e.g., `Track #`, `Artist`)
 */
export function getVariableLabel(varValue: string): string {
  const found = TEMPLATE_VARIABLES.find((v) => v.value === varValue);
  return found ? found.label : `{${varValue}}`;
}

/**
 * Render a live preview of a template using sample metadata.
 *
 * @param segments - Parsed template segments
 * @returns Preview string with sample values substituted
 */
export function renderPreview(segments: TemplateSegment[]): string {
  return segments
    .map((s) => (s.type === 'variable' ? (SAMPLE_METADATA[s.value] ?? `{${s.value}}`) : s.value))
    .join('');
}
