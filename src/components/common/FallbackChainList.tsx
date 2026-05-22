/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file FallbackChainList.tsx -- Generic reorderable priority list component.
 *
 * A shared UI component that renders a vertical list of items with numbered
 * priority indicators and up/down arrow buttons for reordering. Used for both
 * audio/video fallback chains (FallbackTab) and video codec priority (QualityTab).
 *
 * The component is generic over `T extends string`, allowing it to work with
 * any string-literal union type (SongCodec, VideoResolution, VideoCodec, etc.).
 *
 * When the optional `allItems` prop is supplied, the component additionally
 * renders an "Available" panel listing items not currently in the chain so
 * the user can remove and re-add codecs/resolutions on demand (#659).
 *
 * @see {@link ../settings/tabs/FallbackTab.tsx} -- Audio/video fallback chains
 * @see {@link ../settings/tabs/QualityTab.tsx}  -- Video codec priority
 */

// Lucide icons: GripVertical for the drag handle, ArrowUp/ArrowDown for reorder
// buttons, X for remove, Plus for re-add.
import { GripVertical, ArrowUp, ArrowDown, X, Plus } from 'lucide-react';

/**
 * FallbackChainList -- Generic reorderable list for a priority/fallback chain.
 *
 * Renders a vertical list of items with numbered priority indicators and
 * up/down arrow buttons for reordering. The component is generic over
 * `T extends string`, allowing it to work with both `SongCodec` and
 * `VideoResolution` union types.
 *
 * @typeParam T - The union type of the chain items (e.g., SongCodec).
 *
 * @param items     - The ordered array of chain items (highest priority first).
 * @param labels    - A Record mapping each item value to its display name.
 * @param onChange  - Callback invoked with the new array whenever an item
 *                    is moved, removed, or re-added. The parent persists the
 *                    updated order to the settings store.
 * @param allItems  - Optional full universe of available items. When supplied,
 *                    the component renders remove buttons on each row and an
 *                    "Available" panel listing items not currently in `items`.
 *                    Omit to render a pure-reorder list (legacy callers).
 */
export function FallbackChainList<T extends string>({
  items,
  labels,
  onChange,
  allItems,
}: {
  items: T[];
  labels: Record<T, string>;
  onChange: (items: T[]) => void;
  allItems?: readonly T[];
}) {
  /**
   * Moves the item at `index` one position up (towards higher priority).
   * No-op if the item is already at the top of the list.
   * Uses array destructuring swap to avoid a temporary variable.
   */
  const moveUp = (index: number) => {
    if (index === 0) return; // Already at highest priority; nothing to do
    const newItems = [...items]; // Shallow clone to avoid mutating the prop
    [newItems[index - 1], newItems[index]] = [newItems[index], newItems[index - 1]]; // Swap
    onChange(newItems); // Notify parent of the new order
  };

  /**
   * Moves the item at `index` one position down (towards lower priority).
   * No-op if the item is already at the bottom of the list.
   */
  const moveDown = (index: number) => {
    if (index === items.length - 1) return; // Already at lowest priority
    const newItems = [...items];
    [newItems[index], newItems[index + 1]] = [newItems[index + 1], newItems[index]];
    onChange(newItems);
  };

  // Refuse the last removal — an empty chain is meaningless and would block
  // every download. The remove button on the only row stays disabled.
  const removeAt = (index: number) => {
    if (items.length <= 1) return;
    onChange(items.filter((_, i) => i !== index));
  };

  // Append a previously-removed item to the bottom of the chain.
  const addItem = (item: T) => {
    if (items.includes(item)) return; // Defensive — should never trigger from the UI
    onChange([...items, item]);
  };

  // Compute the disabled set only when `allItems` is supplied. Preserves the
  // declared order from `allItems` so the "Available" list reads naturally.
  const disabled: T[] = allItems
    ? allItems.filter((entry) => !items.includes(entry))
    : [];

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        {items.map((item, index) => (
          <div
            key={item}
            className="flex items-center gap-2 px-3 py-2 rounded-platform border border-border-light bg-surface-secondary"
          >
            {/* Drag handle indicator */}
            <GripVertical size={14} className="text-content-tertiary flex-shrink-0" />

            {/* Priority number */}
            <span className="w-6 text-center text-xs font-mono text-content-tertiary">
              {index + 1}
            </span>

            {/* Item label */}
            <span className="flex-1 text-sm text-content-primary">{labels[item]}</span>

            {/* Reorder + remove buttons */}
            <div className="flex gap-0.5">
              <button
                onClick={() => moveUp(index)}
                disabled={index === 0}
                className="p-1 rounded text-content-tertiary hover:text-content-primary disabled:opacity-30 transition-colors"
                aria-label="Move up"
              >
                <ArrowUp size={14} />
              </button>
              <button
                onClick={() => moveDown(index)}
                disabled={index === items.length - 1}
                className="p-1 rounded text-content-tertiary hover:text-content-primary disabled:opacity-30 transition-colors"
                aria-label="Move down"
              >
                <ArrowDown size={14} />
              </button>
              {allItems && (
                <button
                  onClick={() => removeAt(index)}
                  disabled={items.length <= 1}
                  className="p-1 rounded text-content-tertiary hover:text-status-error disabled:opacity-30 transition-colors"
                  aria-label={`Remove ${labels[item]} from fallback chain`}
                  title={
                    items.length <= 1
                      ? 'At least one item must remain in the chain'
                      : `Remove ${labels[item]} from chain`
                  }
                >
                  <X size={14} />
                </button>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Available items pool — only rendered when allItems is supplied
          AND there are items currently outside the chain (#659). */}
      {allItems && disabled.length > 0 && (
        <div>
          <h5 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">
            Available (not in chain)
          </h5>
          <div className="space-y-1">
            {disabled.map((item) => (
              <div
                key={item}
                className="flex items-center gap-2 px-3 py-2 rounded-platform border border-dashed border-border-light bg-surface-primary opacity-70"
              >
                <span className="w-6 text-center text-xs font-mono text-content-tertiary">
                  &mdash;
                </span>
                <span className="flex-1 text-sm text-content-secondary">{labels[item]}</span>
                <button
                  onClick={() => addItem(item)}
                  className="p-1 rounded text-content-tertiary hover:text-content-primary transition-colors"
                  aria-label={`Add ${labels[item]} back to fallback chain`}
                  title={`Add ${labels[item]} to the bottom of the chain`}
                >
                  <Plus size={14} />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
