/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file TemplateBuilder.tsx -- Visual chip/pill builder for GAMDL templates.
 *
 * Replaces plain text inputs with an interactive chip-based UI for building
 * GAMDL file/folder naming templates. Each segment of the template (variables
 * like `{artist}` and literal text like `/` or `Compilations`) is rendered as
 * a clickable pill that can be removed. New segments are added via a dropdown
 * menu. A raw-edit toggle lets power users edit the template string directly.
 *
 * The component is controlled: it receives a template `value` string, derives
 * segments via `parseTemplate()`, and calls `onChange()` with a serialized
 * string after mutations. The parent's string is the single source of truth.
 *
 * @see {@link src/lib/template-parser.ts} -- Parser/serializer/registries
 * @see {@link src/components/settings/tabs/TemplatesTab.tsx} -- Consumer
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  COMMON_LITERALS,
  getVariableLabel,
  parseTemplate,
  renderPreview,
  serializeTemplate,
  TEMPLATE_VARIABLES,
  type TemplateVariable,
} from '@/lib/template-parser';

// ============================================================
// Props
// ============================================================

interface TemplateBuilderProps {
  /** Label text displayed above the builder. */
  label: string;
  /** Optional description text shown below the builder. */
  description?: string;
  /** The current template string value (controlled). */
  value: string;
  /** Callback when the template changes — receives the serialized string. */
  onChange: (value: string) => void;
  /**
   * Which variable categories to show in the add menu.
   *
   * Default: `['common', 'track', 'album']`. Playlist templates pass
   * `['common', 'track', 'album', 'playlist']` to include playlist variables.
   *
   * `'common'` is included implicitly in every default so multi-service-prep
   * tokens like `{platform}` (#800) are always pickable regardless of the
   * caller's category list. Callers that omit `'common'` deliberately can
   * pass an explicit list that excludes it, but doing so is rarely correct.
   */
  variableCategories?: TemplateVariable['category'][];
}

// ============================================================
// Component
// ============================================================

export function TemplateBuilder({
  label,
  description,
  value,
  onChange,
  variableCategories = ['common', 'track', 'album'],
}: TemplateBuilderProps) {
  // Derive segments from the string prop (single source of truth)
  const segments = useMemo(() => parseTemplate(value), [value]);
  const preview = useMemo(() => renderPreview(segments), [segments]);

  // UI state
  const [rawMode, setRawMode] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [customText, setCustomText] = useState('');
  const menuRef = useRef<HTMLDivElement>(null);
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  // Filter variables by allowed categories
  const filteredVariables = useMemo(
    () => TEMPLATE_VARIABLES.filter((v) => variableCategories.includes(v.category)),
    [variableCategories],
  );

  // Close menu when clicking outside
  useEffect(() => {
    if (!menuOpen) return;
    function handleClick(e: MouseEvent) {
      if (
        menuRef.current &&
        !menuRef.current.contains(e.target as Node) &&
        menuButtonRef.current &&
        !menuButtonRef.current.contains(e.target as Node)
      ) {
        setMenuOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [menuOpen]);

  // Segment mutation handlers
  const addVariable = useCallback(
    (varValue: string) => {
      const newSegments = [...segments, { type: 'variable' as const, value: varValue }];
      onChange(serializeTemplate(newSegments));
      setMenuOpen(false);
    },
    [segments, onChange],
  );

  const addLiteral = useCallback(
    (text: string) => {
      if (!text) return;
      const newSegments = [...segments, { type: 'literal' as const, value: text }];
      onChange(serializeTemplate(newSegments));
      setMenuOpen(false);
      setCustomText('');
    },
    [segments, onChange],
  );

  const removeSegment = useCallback(
    (index: number) => {
      const newSegments = segments.filter((_, i) => i !== index);
      onChange(serializeTemplate(newSegments));
    },
    [segments, onChange],
  );

  // Generate a deterministic id from the label for accessibility
  const builderId = `template-${label.toLowerCase().replace(/\s+/g, '-')}`;

  return (
    <div className="space-y-1.5">
      {/* ---- Label row ---- */}
      <div className="flex items-center gap-1.5">
        <label htmlFor={builderId} className="text-sm font-medium text-content-primary">
          {label}
        </label>
        <button
          type="button"
          onClick={() => setRawMode(!rawMode)}
          className="ml-auto text-[10px] text-content-tertiary hover:text-content-primary transition-colors"
        >
          {rawMode ? 'Visual' : 'Edit Raw'}
        </button>
      </div>

      {/* ---- Builder area ---- */}
      {rawMode ? (
        /* Raw text editing mode */
        <input
          id={builderId}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="w-full px-3 py-2 text-sm rounded-platform border border-border
            bg-surface-secondary text-content-primary placeholder-content-tertiary
            transition-colors focus:border-accent focus:ring-1 focus:ring-accent font-mono"
          placeholder="e.g., {album_artist}/{album}"
        />
      ) : (
        /* Visual chip builder mode */
        <div className="relative">
          <div
            className="flex flex-wrap items-center gap-1.5 min-h-[38px] px-3 py-2
              rounded-platform border border-border bg-surface-secondary"
          >
            {/* Chips */}
            {segments.map((seg, i) => (
              <span
                key={`${i}-${seg.value}`}
                className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs
                  ${
                    seg.type === 'variable'
                      ? 'bg-accent/10 text-accent font-medium'
                      : 'bg-surface-primary text-content-primary font-mono border border-border-light'
                  }`}
              >
                {seg.type === 'variable'
                  ? getVariableLabel(seg.value)
                  : seg.value === ' '
                    ? '\u2423' // Open box character for space visibility
                    : seg.value}
                <button
                  type="button"
                  onClick={() => removeSegment(i)}
                  className="text-current opacity-50 hover:opacity-100 transition-opacity leading-none"
                  aria-label={`Remove ${seg.type === 'variable' ? getVariableLabel(seg.value) : seg.value}`}
                >
                  &times;
                </button>
              </span>
            ))}

            {/* Add button */}
            <button
              ref={menuButtonRef}
              type="button"
              onClick={() => setMenuOpen(!menuOpen)}
              className="inline-flex items-center justify-center w-6 h-6 rounded-md
                border border-dashed border-border text-content-tertiary
                hover:border-accent hover:text-accent transition-colors text-sm leading-none"
              aria-label="Add template segment"
            >
              +
            </button>
          </div>

          {/* ---- Dropdown menu ---- */}
          {menuOpen && (
            <div
              ref={menuRef}
              className="absolute top-full left-0 mt-1 z-40
                min-w-[260px] max-h-[320px] overflow-y-auto
                bg-surface-elevated border border-border-light
                rounded-platform shadow-lg p-2 space-y-2"
            >
              {/* Variables section */}
              <div>
                <p className="text-[10px] font-semibold text-content-tertiary uppercase tracking-wider px-1 mb-1">
                  Variables
                </p>
                <div className="space-y-0.5">
                  {filteredVariables.map((v) => (
                    <button
                      key={v.value}
                      type="button"
                      onClick={() => addVariable(v.value)}
                      className="flex items-center justify-between w-full px-2 py-1.5 rounded-md
                        text-left text-xs hover:bg-surface-secondary transition-colors"
                    >
                      <span className="font-medium text-content-primary">{v.label}</span>
                      <span className="text-content-tertiary">{v.description}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Separators section */}
              <div className="border-t border-border-light pt-2">
                <p className="text-[10px] font-semibold text-content-tertiary uppercase tracking-wider px-1 mb-1">
                  Separators
                </p>
                <div className="flex flex-wrap gap-1">
                  {COMMON_LITERALS.map((lit) => (
                    <button
                      key={lit.value}
                      type="button"
                      onClick={() => addLiteral(lit.value)}
                      className="px-2 py-1 rounded-md text-xs font-mono
                        bg-surface-secondary border border-border-light
                        text-content-primary hover:border-accent transition-colors"
                    >
                      {lit.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Custom text section */}
              <div className="border-t border-border-light pt-2">
                <p className="text-[10px] font-semibold text-content-tertiary uppercase tracking-wider px-1 mb-1">
                  Custom Text
                </p>
                <div className="flex gap-1">
                  <input
                    type="text"
                    value={customText}
                    onChange={(e) => setCustomText(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        addLiteral(customText);
                      }
                    }}
                    placeholder="Type text..."
                    className="flex-1 px-2 py-1 text-xs rounded-md border border-border
                      bg-surface-secondary text-content-primary placeholder-content-tertiary
                      focus:border-accent focus:ring-1 focus:ring-accent transition-colors"
                  />
                  <button
                    type="button"
                    onClick={() => addLiteral(customText)}
                    disabled={!customText}
                    className="px-2 py-1 text-xs rounded-md font-medium
                      bg-accent text-white hover:bg-accent-hover
                      disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                  >
                    Add
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ---- Preview ---- */}
      {preview && (
        <p className="text-xs font-mono text-content-secondary truncate">
          Preview: {preview}
        </p>
      )}

      {/* ---- Description ---- */}
      {description && <p className="text-xs text-content-tertiary">{description}</p>}
    </div>
  );
}
