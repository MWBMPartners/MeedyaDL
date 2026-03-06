/**
 * Copyright (c) 2024-2026 MeedyaDL
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
 * `FallbackChainList<T>` is a shared generic reorderable list component
 * (extracted to `@/components/common/FallbackChainList.tsx`) used for
 * both the audio/video fallback chains here and the video codec priority
 * in QualityTab. It is parameterised on the item type (`SongCodec` or
 * `VideoResolution`) and receives the label map for display text.
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

// Zustand store for reading and writing the fallback chain settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Label maps that convert codec/resolution identifiers to human-readable names.
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import type { SongCodec, VideoResolution } from '@/types';

// Shared components: Button for toggle tabs, FallbackChainList for reorderable lists.
import { Button, FallbackChainList, SettingsSection } from '@/components/common';

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
    <div className="space-y-3 max-w-xl">
      <SettingsSection
        title="Fallback Chain"
        description="When the preferred codec or resolution is unavailable, GAMDL will automatically try the next option in the chain. Drag items to reorder priority (top = highest priority). Note: codecs marked (Experimental) may fail intermittently without the Wrapper service — only AAC Legacy and AAC-HE Legacy are reliably downloadable with cookies alone."
      >
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
            <h4 className="text-sm font-semibold text-content-primary mb-2">
              Audio Codec Fallback Chain
            </h4>
            <p className="text-xs text-content-tertiary mb-3">
              <strong>ALAC</strong> = lossless (perfect quality) &middot; <strong>Atmos</strong> =
              spatial 3D audio &middot; <strong>AC3</strong> = 5.1 surround sound &middot;{' '}
              <strong>AAC Binaural</strong> = spatial for regular headphones &middot;{' '}
              <strong>AAC</strong> = standard quality &middot; <strong>AAC Legacy</strong> = older
              device compatibility
            </p>
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
            <h4 className="text-sm font-semibold text-content-primary mb-3">
              Video Resolution Fallback Chain
            </h4>
            <FallbackChainList<VideoResolution>
              items={settings.video_fallback_chain}
              labels={VIDEO_RESOLUTION_LABELS}
              onChange={(chain) => updateSettings({ video_fallback_chain: chain })}
            />
          </div>
        )}
      </SettingsSection>
    </div>
  );
}
