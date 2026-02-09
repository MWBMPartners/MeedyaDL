/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Fallback chain settings tab.
 * Allows users to reorder the audio and video fallback chains using
 * drag-and-drop. When a preferred codec/resolution is unavailable,
 * GAMDL retries with the next item in the chain.
 */

import { useState } from 'react';
import { GripVertical, ArrowUp, ArrowDown } from 'lucide-react';
import { useSettingsStore } from '@/stores/settingsStore';
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import type { SongCodec, VideoResolution } from '@/types';
import { Button } from '@/components/common';

/**
 * Reorderable list component for a fallback chain.
 * Shows numbered items with up/down buttons for reordering.
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
  /** Move an item up in the list */
  const moveUp = (index: number) => {
    if (index === 0) return;
    const newItems = [...items];
    [newItems[index - 1], newItems[index]] = [newItems[index], newItems[index - 1]];
    onChange(newItems);
  };

  /** Move an item down in the list */
  const moveDown = (index: number) => {
    if (index === items.length - 1) return;
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
 * Renders the Fallback settings tab with reorderable audio and video
 * fallback chains. Items are tried top-to-bottom when the preferred
 * codec/resolution is unavailable.
 */
export function FallbackTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /* Track which chain section is expanded */
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
