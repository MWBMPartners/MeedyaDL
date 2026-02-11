/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file FallbackTab.tsx -- Drag-to-reorder fallback chain settings tab.
 *
 * Renders the "Fallback" tab within the {@link SettingsPage} component.
 * When a preferred audio codec or video resolution is unavailable for a
 * given track, GAMDL automatically tries the next option in the fallback
 * chain. This tab lets users reorder the chains to control retry priority.
 *
 * ## Fallback Chain Concept
 *
 * Each chain is an ordered array of codec/resolution identifiers stored in
 * the settings:
 *   - `settings.music_fallback_chain: SongCodec[]` -- audio fallback order
 *   - `settings.video_fallback_chain: VideoResolution[]` -- video fallback order
 *
 * Items at the top of the list are tried first. When the user clicks the
 * up/down arrow buttons, the item swaps position with its neighbour and
 * the new order is persisted to the store.
 *
 * ## Implementation Note
 *
 * The original design called for @dnd-kit drag-and-drop support (see
 * {@link https://docs.dndkit.com/}), but the current implementation uses
 * simple up/down buttons for reordering. The grip handle icon
 * (`GripVertical`) remains as a visual affordance indicating that the
 * items are reorderable. A future iteration may add full drag-and-drop
 * via @dnd-kit's `useSortable` hook.
 *
 * ## Sub-component
 *
 * `FallbackChainList<T>` is a generic reorderable list component used for
 * both the audio and video chains. It is parameterised on the item type
 * (`SongCodec` or `VideoResolution`) and receives the label map for
 * display text.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore` via:
 *   - `settings.music_fallback_chain` / `settings.video_fallback_chain`
 *   - `updateSettings({ music_fallback_chain: ... })` / `updateSettings({ video_fallback_chain: ... })`
 *
 * @see {@link https://docs.dndkit.com/}            -- @dnd-kit documentation (future integration)
 * @see {@link ../SettingsPage.tsx}                  -- Parent container
 * @see {@link @/stores/settingsStore.ts}            -- Zustand store
 * @see {@link @/types/index.ts}                     -- SongCodec, VideoResolution types
 */

// React useState for tracking which chain section (audio/video) is active.
import { useState } from 'react';

// Lucide icons: GripVertical for the drag handle, ArrowUp/ArrowDown for reorder buttons.
import { GripVertical, ArrowUp, ArrowDown } from 'lucide-react';

// Zustand store for reading and writing the fallback chain settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Label maps that convert codec/resolution identifiers to human-readable names.
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import type { SongCodec, VideoResolution } from '@/types';

// Shared Button component used for the audio/video chain toggle tabs.
import { Button } from '@/components/common';

/**
 * FallbackChainList -- Generic reorderable list for a fallback chain.
 *
 * Renders a vertical list of items with numbered priority indicators and
 * up/down arrow buttons for reordering. The component is generic over
 * `T extends string`, allowing it to work with both `SongCodec` and
 * `VideoResolution` union types.
 *
 * @typeParam T - The union type of the chain items (e.g., SongCodec).
 *
 * @param items    - The ordered array of chain items (highest priority first).
 * @param labels   - A Record mapping each item value to its display name.
 * @param onChange - Callback invoked with the new array whenever an item
 *                   is moved. The parent is responsible for persisting the
 *                   updated order to the settings store.
 */
function FallbackChainList<T extends string>({
  items,
  labels,
  onChange,
}: {
  items: T[];
  labels: Record<T, string>;
  onChange: (items: T[]) => void;
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

  return (
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
          <span className="flex-1 text-sm text-content-primary">
            {labels[item]}
          </span>

          {/* Reorder buttons */}
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
          </div>
        </div>
      ))}
    </div>
  );
}

/**
 * FallbackTab -- Main exported component for the Fallback settings tab.
 *
 * Contains two sub-sections accessible via toggle buttons:
 *   1. **Audio Fallback** -- Reorderable list of `SongCodec` values
 *      stored in `settings.music_fallback_chain`.
 *   2. **Video Fallback** -- Reorderable list of `VideoResolution` values
 *      stored in `settings.video_fallback_chain`.
 *
 * Only one chain is displayed at a time, controlled by the `activeChain`
 * local state. This keeps the UI focused and prevents the tab from
 * becoming too tall.
 *
 * The top-of-tab description paragraph explains the fallback concept to
 * the user: items at the top of the chain are tried first.
 */
export function FallbackTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting chain reorders */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /**
   * Tracks which chain section is currently visible: 'music' (audio codecs)
   * or 'video' (video resolutions). Defaults to 'music'.
   */
  const [activeChain, setActiveChain] = useState<'music' | 'video'>('music');

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <p className="text-sm text-content-secondary mb-4">
          When the preferred codec or resolution is unavailable, GAMDL will
          automatically try the next option in the chain. Drag items to
          reorder priority (top = highest priority).
        </p>
      </div>

      {/* Chain selector tabs */}
      <div className="flex gap-2 border-b border-border-light pb-2">
        <Button
          variant={activeChain === 'music' ? 'primary' : 'ghost'}
          size="sm"
          onClick={() => setActiveChain('music')}
        >
          Audio Fallback
        </Button>
        <Button
          variant={activeChain === 'video' ? 'primary' : 'ghost'}
          size="sm"
          onClick={() => setActiveChain('video')}
        >
          Video Fallback
        </Button>
      </div>

      {/* Music fallback chain */}
      {activeChain === 'music' && (
        <div>
          <h3 className="text-sm font-semibold text-content-primary mb-3">
            Audio Codec Fallback Chain
          </h3>
          <FallbackChainList<SongCodec>
            items={settings.music_fallback_chain}
            labels={SONG_CODEC_LABELS}
            onChange={(chain) => updateSettings({ music_fallback_chain: chain })}
          />
        </div>
      )}

      {/* Video fallback chain */}
      {activeChain === 'video' && (
        <div>
          <h3 className="text-sm font-semibold text-content-primary mb-3">
            Video Resolution Fallback Chain
          </h3>
          <FallbackChainList<VideoResolution>
            items={settings.video_fallback_chain}
            labels={VIDEO_RESOLUTION_LABELS}
            onChange={(chain) =>
              updateSettings({ video_fallback_chain: chain })
            }
          />
        </div>
      )}
    </div>
  );
}
