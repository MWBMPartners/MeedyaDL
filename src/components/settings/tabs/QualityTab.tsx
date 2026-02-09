/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Quality settings tab.
 * Configures default audio codec, video resolution, video codec priority,
 * and remux format for downloads.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Select, Toggle, Input } from '@/components/common';
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import type { SongCodec, VideoResolution } from '@/types';

/**
 * Renders the Quality settings tab with audio codec, video resolution,
 * video codec priority, and remux format configuration.
 */
export function QualityTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /* Build select options from label maps */
  const codecOptions = Object.entries(SONG_CODEC_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(
    ([value, label]) => ({ value, label }),
  );

  const remuxOptions = [
    { value: 'm4v', label: 'M4V (Apple standard)' },
    { value: 'mp4', label: 'MP4 (Universal)' },
    { value: 'mkv', label: 'MKV (Matroska)' },
  ];

  return (
    <div className="space-y-6 max-w-xl">
      {/* Section: Audio */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Audio Quality
        </h3>

        {/* Default audio codec */}
        <Select
          label="Default Audio Codec"
          description="The preferred codec for song downloads. ALAC provides lossless quality."
          options={codecOptions}
          value={settings.default_song_codec}
          onChange={(e) =>
            updateSettings({ default_song_codec: e.target.value as SongCodec })
          }
        />

        {/* Fallback toggle */}
        <Toggle
          label="Enable Fallback Chain"
          description="When the preferred codec is unavailable, automatically try the next codec in the fallback chain"
          checked={settings.fallback_enabled}
          onChange={(checked) => updateSettings({ fallback_enabled: checked })}
        />
      </div>

      {/* Section: Video */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Video Quality
        </h3>

        {/* Default video resolution */}
        <Select
          label="Default Video Resolution"
          description="The preferred resolution for music video downloads"
          options={resolutionOptions}
          value={settings.default_video_resolution}
          onChange={(e) =>
            updateSettings({
              default_video_resolution: e.target.value as VideoResolution,
            })
          }
        />

        {/* Video codec priority */}
        <Input
          label="Video Codec Priority"
          description="Comma-separated list of preferred video codecs (e.g., h265,h264)"
          value={settings.default_video_codec_priority}
          onChange={(e) =>
            updateSettings({ default_video_codec_priority: e.target.value })
          }
        />

        {/* Remux format */}
        <Select
          label="Video Remux Format"
          description="Container format for remuxed video files"
          options={remuxOptions}
          value={settings.default_video_remux_format}
          onChange={(e) =>
            updateSettings({ default_video_remux_format: e.target.value })
          }
        />
      </div>
    </div>
  );
}
