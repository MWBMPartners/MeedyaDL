/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Download form component.
 * Provides the URL input field with real-time validation, content type
 * detection badge, and "Add to Queue" button. Supports per-download
 * quality override options via an expandable section.
 */

import { useState } from 'react';
import {
  Music,
  Disc3,
  ListMusic,
  Video,
  User,
  HelpCircle,
  ChevronDown,
  ChevronUp,
  Plus,
} from 'lucide-react';
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { Button, Select } from '@/components/common';
import type { AppleMusicContentType, SongCodec } from '@/types';
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import { PageHeader } from '@/components/layout';

/** Icon mapping for detected content types */
const CONTENT_TYPE_ICONS: Record<AppleMusicContentType, typeof Music> = {
  song: Music,
  album: Disc3,
  playlist: ListMusic,
  'music-video': Video,
  artist: User,
  unknown: HelpCircle,
};

/** Human-readable labels for content types */
const CONTENT_TYPE_LABELS: Record<AppleMusicContentType, string> = {
  song: 'Song',
  album: 'Album',
  playlist: 'Playlist',
  'music-video': 'Music Video',
  artist: 'Artist',
  unknown: 'Unknown',
};

/**
 * Renders the download page with URL input, content type detection,
 * optional quality overrides, and the "Add to Queue" action.
 */
export function DownloadForm() {
  /* State from stores */
  const urlInput = useDownloadStore((s) => s.urlInput);
  const urlIsValid = useDownloadStore((s) => s.urlIsValid);
  const urlContentType = useDownloadStore((s) => s.urlContentType);
  const isSubmitting = useDownloadStore((s) => s.isSubmitting);
  const setUrlInput = useDownloadStore((s) => s.setUrlInput);
  const submitDownload = useDownloadStore((s) => s.submitDownload);
  const overrideOptions = useDownloadStore((s) => s.overrideOptions);
  const setOverrideOptions = useDownloadStore((s) => s.setOverrideOptions);
  const settings = useSettingsStore((s) => s.settings);
  const addToast = useUiStore((s) => s.addToast);

  /* Local UI state for the quality override panel */
  const [showOverrides, setShowOverrides] = useState(false);

  /** Handle form submission */
  const handleSubmit = async () => {
    try {
      await submitDownload();
      addToast('Download added to queue', 'success');
    } catch {
      addToast('Failed to add download', 'error');
    }
  };

  /** Handle Enter key in the URL input */
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && urlIsValid && !isSubmitting) {
      handleSubmit();
    }
  };

  /* Detect content type for badge display */
  const contentType = urlContentType as AppleMusicContentType;
  const ContentIcon = CONTENT_TYPE_ICONS[contentType] || HelpCircle;

  /* Build codec options for the override select */
  const codecOptions = Object.entries(SONG_CODEC_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(
    ([value, label]) => ({ value, label }),
  );

  return (
    <div>
      <PageHeader
        title="Download"
        subtitle="Enter an Apple Music URL to download music or videos"
      />

      <div className="p-6 max-w-2xl space-y-6">
        {/* URL Input Section */}
        <div className="space-y-2">
          <label
            htmlFor="url-input"
            className="block text-sm font-medium text-content-primary"
          >
            Apple Music URL
          </label>

          <div className="flex gap-2">
            {/* URL text input */}
            <div className="flex-1 relative">
              <input
                id="url-input"
                type="text"
                value={urlInput}
                onChange={(e) => setUrlInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="https://music.apple.com/..."
                className={`
                  w-full px-3 py-2 text-sm rounded-platform border
                  bg-surface-secondary text-content-primary
                  placeholder-content-tertiary
                  transition-colors
                  focus:ring-1 focus:ring-accent
                  ${urlInput && !urlIsValid ? 'border-status-error focus:border-status-error' : 'border-border focus:border-accent'}
                `}
              />

              {/* Content type badge (shown when URL is valid) */}
              {urlIsValid && (
                <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-light text-accent text-xs font-medium">
                  <ContentIcon size={12} />
                  {CONTENT_TYPE_LABELS[contentType]}
                </div>
              )}
            </div>

            {/* Add to Queue button */}
            <Button
              variant="primary"
              icon={<Plus size={16} />}
              loading={isSubmitting}
              disabled={!urlIsValid}
              onClick={handleSubmit}
            >
              Add to Queue
            </Button>
          </div>

          {/* Validation feedback */}
          {urlInput && !urlIsValid && (
            <p className="text-xs text-status-error">
              Please enter a valid Apple Music URL
            </p>
          )}
          {!urlInput && (
            <p className="text-xs text-content-tertiary">
              Supports songs, albums, playlists, music videos, and artist pages
            </p>
          )}
        </div>

        {/* Quality Override Section */}
        <div className="rounded-platform border border-border-light">
          {/* Toggle header */}
          <button
            onClick={() => setShowOverrides(!showOverrides)}
            className="w-full flex items-center justify-between px-4 py-3 text-sm text-content-secondary hover:text-content-primary transition-colors"
          >
            <span>
              Quality Overrides{' '}
              <span className="text-content-tertiary">
                (default: {SONG_CODEC_LABELS[settings.default_song_codec]})
              </span>
            </span>
            {showOverrides ? (
              <ChevronUp size={16} />
            ) : (
              <ChevronDown size={16} />
            )}
          </button>

          {/* Expandable override panel */}
          {showOverrides && (
            <div className="px-4 pb-4 space-y-4 border-t border-border-light pt-4">
              {/* Audio codec override */}
              <Select
                label="Audio Codec"
                description="Override the default audio codec for this download"
                options={codecOptions}
                value={overrideOptions?.song_codec || ''}
                onChange={(e) => {
                  const codec = e.target.value as SongCodec;
                  setOverrideOptions(
                    codec
                      ? { ...overrideOptions, song_codec: codec }
                      : null,
                  );
                }}
                placeholder="Use default"
              />

              {/* Video resolution override */}
              <Select
                label="Video Resolution"
                description="Override the default video resolution for this download"
                options={resolutionOptions}
                value={overrideOptions?.music_video_resolution || ''}
                onChange={(e) => {
                  const res = e.target.value;
                  setOverrideOptions(
                    res
                      ? {
                          ...overrideOptions,
                          music_video_resolution: res as typeof settings.default_video_resolution,
                        }
                      : null,
                  );
                }}
                placeholder="Use default"
              />

              {/* Clear overrides button */}
              {overrideOptions && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setOverrideOptions(null)}
                >
                  Clear overrides
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
