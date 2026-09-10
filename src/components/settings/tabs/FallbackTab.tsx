/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file FallbackTab.tsx -- Drag-to-reorder fallback chain settings tab.
 *
 * Renders the "Fallback" tab within the {@link SettingsPage} component.
 * It holds two ordered lists: which audio codec to try first for songs,
 * and which video codec to try first for music videos. This tab lets
 * users reorder each list, and remove/re-add entries from it.
 *
 * ## Fallback Chain Concept
 *
 * Both chains are ordered arrays of codec identifiers stored in settings:
 *   - `settings.music_fallback_chain: SongCodec[]` -- audio codec order
 *   - `settings.video_codec_fallback_chain: VideoCodec[]` -- video codec order
 *
 * Items at the top of the list are tried first. When the user clicks the
 * up/down arrow buttons, the item swaps position with its neighbour and
 * the new order is persisted to the store.
 *
 * For audio, "stepping down" a codec chain is split between this app and
 * GAMDL: GAMDL tries the whole chain itself in one run when it can, and
 * this app's own retry logic restarts GAMDL one codec at a time as a
 * safety net for whatever that first attempt didn't catch. For video,
 * GAMDL walks the whole codec chain itself in a single run -- there is
 * nothing left for this app to step in for.
 *
 * There is deliberately no video *resolution* chain here. A resolution is
 * a ceiling GAMDL never fails to meet -- see the comment on `VideoCodec`
 * in `@/types/index.ts` for the full explanation of why a codec, not a
 * resolution, is what can genuinely be unavailable.
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
 * (extracted to `@/components/common/FallbackChainList.tsx`) used for the
 * audio and video codec chains here, and for the duplicate-detection
 * preference order in QualityTab. It is parameterised on the item type
 * (`SongCodec` or `VideoCodec`) and receives the label map for display
 * text.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore` via:
 *   - `settings.music_fallback_chain` / `settings.video_codec_fallback_chain`
 *   - `updateSettings({ music_fallback_chain: ... })` / `updateSettings({ video_codec_fallback_chain: ... })`
 *
 * @see {@link https://docs.dndkit.com/}            -- @dnd-kit documentation (future integration)
 * @see {@link ../SettingsPage.tsx}                  -- Parent container
 * @see {@link @/stores/settingsStore.ts}            -- Zustand store
 * @see {@link @/types/index.ts}                     -- SongCodec, VideoCodec types
 */

// React useState for tracking which chain section (audio/video) is active.
import { useState } from 'react';

// Audit v2 #6 — per-field Zustand binding.
import { useSettingsField } from '@/hooks/useSettingsField';

// Version-aware GAMDL capability flags (#963, #1002) — drives the
// wrapper-dependency prose below without touching the codec labels
// themselves (#965).
import { useGamdlCapabilities } from '@/hooks/useGamdlCapabilities';

// Label maps that convert codec identifiers to human-readable names.
import { SONG_CODEC_LABELS, VIDEO_CODEC_LABELS, ALL_VIDEO_CODECS } from '@/types';
import type { SongCodec, VideoCodec } from '@/types';

/**
 * Full universe of audio codecs that may appear in the fallback chain (#659).
 * Used to populate the "Available" panel below the active chain so users can
 * remove and re-add codecs on demand. Order here is the order shown in the
 * "Available" pool — it does NOT influence chain priority, which is driven
 * by the user-managed `music_fallback_chain` array.
 */
const ALL_SONG_CODECS = Object.keys(SONG_CODEC_LABELS) as SongCodec[];

// Shared components: Button for toggle tabs, FallbackChainList for reorderable lists.
import { Button, FallbackChainList, SettingsSection } from '@/components/common';

/**
 * FallbackTab -- Main exported component for the Fallback settings tab.
 *
 * Contains two sub-sections accessible via toggle buttons:
 *   1. **Audio Fallback** -- Reorderable list of `SongCodec` values
 *      stored in `settings.music_fallback_chain`.
 *   2. **Video Fallback** -- Reorderable list of `VideoCodec` values
 *      stored in `settings.video_codec_fallback_chain`.
 *
 * Only one chain is displayed at a time, controlled by the `activeChain`
 * local state. This keeps the UI focused and prevents the tab from
 * becoming too tall.
 *
 * The top-of-tab description paragraph explains the fallback concept to
 * the user: items at the top of the chain are tried first.
 */
export function FallbackTab() {
  // Per-field Zustand bindings (audit v2 #6).
  const musicChain = useSettingsField('music_fallback_chain');
  const videoChain = useSettingsField('video_codec_fallback_chain');

  // Version-aware GAMDL capabilities (#963, #1002) — used below to swap
  // in accurate wrapper-dependency prose for the installed GAMDL release
  // instead of the blanket "(Experimental)" note, which is stale on 3.8+.
  const { capabilities: gamdlCaps } = useGamdlCapabilities();

  /**
   * Tracks which chain section is currently visible: 'music' (audio codecs)
   * or 'video' (video codecs). Defaults to 'music'.
   */
  const [activeChain, setActiveChain] = useState<'music' | 'video'>('music');

  return (
    <div className="space-y-3">
      <SettingsSection
        title="Fallback Chain"
        description="Both of these are lists of codecs, tried in order when the first choice is unavailable. Audio is stepped down partly by this app and partly by the download tool: the tool tries a whole codec chain itself in one run where it can, and this app restarts it one codec at a time as a safety net for whatever that first attempt missed. Video is stepped down by the tool alone, in a single run. Use the up/down arrows to reorder priority (top = highest), the × button to remove a codec from the chain, and the + button under Available to put a removed codec back. Note: audio codecs marked (Experimental) may fail intermittently without the Wrapper service — only AAC Legacy and AAC-HE Legacy are reliably downloadable with cookies alone."
      >
        {/* Chain selector tabs. Fix 6 (a11y audit): aria-pressed says
            which of the two is currently selected -- these behave like
            a two-way toggle, not a pair of ordinary buttons. */}
        <div className="flex gap-2 border-b border-border-light pb-2">
        <Button
          variant={activeChain === 'music' ? 'primary' : 'ghost'}
          size="sm"
          onClick={() => setActiveChain('music')}
          aria-pressed={activeChain === 'music'}
        >
          Audio Fallback
        </Button>
        <Button
          variant={activeChain === 'video' ? 'primary' : 'ghost'}
          size="sm"
          onClick={() => setActiveChain('video')}
          aria-pressed={activeChain === 'video'}
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
            {/* Version-aware wrapper-dependency note (#963, #1002). The
                codec dropdown's (Experimental) labels stay unconditional
                (#965) -- this paragraph is the accurate, version-specific
                explanation instead. */}
            {gamdlCaps.assets_api_unlocks_lossy_codecs ? (
              <p className="text-xs text-content-tertiary mb-3">
                Your installed GAMDL release can download Atmos, AC3, and every AAC variant
                without the Wrapper service — only <strong>ALAC</strong> still requires it.
              </p>
            ) : (
              <p className="text-xs text-content-tertiary mb-3">
                Codecs marked (Experimental) may fail intermittently without the Wrapper service
                on your installed GAMDL release — only AAC Legacy and AAC-HE Legacy are reliably
                downloadable with cookies alone.
              </p>
            )}
            <FallbackChainList<SongCodec>
              items={musicChain.value}
              labels={SONG_CODEC_LABELS}
              allItems={ALL_SONG_CODECS}
              onChange={musicChain.set}
            />
          </div>
        )}

        {/* Video codec fallback chain */}
        {activeChain === 'video' && (
          <div>
            <h4 className="text-sm font-semibold text-content-primary mb-2">
              Video Codec Fallback Chain
            </h4>
            <p className="text-xs text-content-tertiary mb-3">
              Music video resolution is a ceiling, not a request: whatever
              maximum resolution is set in Quality, the download tool always
              finds the closest quality at or below it, so resolution can
              never be unavailable and there is nothing to step down through
              — which is why there is no resolution list here. Codec is
              different. <strong>H.265</strong> makes smaller files at the
              same picture quality, and is the only codec offered for videos
              above 1080p. <strong>H.264</strong> plays on almost every
              device and app, but Apple Music never offers it above 1080p.
              The tool tries this list in order and downloads the video
              using the first codec it is actually offered in — if you
              remove H.264, a video that is only offered in H.264 is
              skipped rather than downloaded.
            </p>
            <FallbackChainList<VideoCodec>
              items={videoChain.value}
              labels={VIDEO_CODEC_LABELS}
              allItems={ALL_VIDEO_CODECS}
              onChange={videoChain.set}
            />
          </div>
        )}
      </SettingsSection>
    </div>
  );
}
