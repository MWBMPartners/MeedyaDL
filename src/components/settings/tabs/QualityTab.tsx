/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file QualityTab.tsx -- Audio and video quality defaults settings tab.
 *
 * Renders the "Quality" tab within the {@link SettingsPage} component.
 * This tab lets the user configure the default quality preferences for
 * both audio (song) and video (music video) downloads:
 *
 *   - **Default Audio Codec** -- The preferred audio codec for song
 *     downloads (e.g., ALAC for lossless, AAC for compressed). Maps to
 *     `settings.default_song_codec` and GAMDL's `--song-codec` flag.
 *
 *   - **Enable Fallback Chain** -- Whether to automatically try the next
 *     codec in the fallback chain when the preferred codec is unavailable.
 *     Maps to `settings.fallback_enabled`. The actual fallback order is
 *     configured in the separate {@link FallbackTab}.
 *
 *   - **Default Video Resolution** -- The preferred resolution for music
 *     video downloads (e.g., 2160p for 4K). Maps to
 *     `settings.default_video_resolution`.
 *
 *   - **Video Codec Priority** -- A comma-separated list of video codecs
 *     tried in order (e.g., "h265,h264"). Maps to
 *     `settings.default_video_codec_priority`.
 *
 *   - **Video Remux Format** -- The output container format for remuxed
 *     video files (M4V, MP4, or MKV). Maps to
 *     `settings.default_video_remux_format`.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`, same pattern as all
 * other tab components.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link ./FallbackTab.tsx}          -- Where the fallback chain order is configured
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- SongCodec and VideoResolution type definitions
 */

// Zustand store for reading/writing quality settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for dropdowns, Toggle for switches, Input for text fields.
import { Select, Toggle, Input } from '@/components/common';

// Label maps and type definitions for audio codecs and video resolutions.
// SONG_CODEC_LABELS and VIDEO_RESOLUTION_LABELS are Record<T, string> maps
// used to populate the <Select> dropdown options.
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';
import type { SongCodec, VideoResolution } from '@/types';

/**
 * QualityTab -- Renders the Quality settings tab.
 *
 * Organised into two visual sections:
 *   1. "Audio Quality" -- codec selection and fallback toggle
 *   2. "Video Quality" -- resolution, codec priority, and remux format
 *
 * Each control's `onChange` calls `updateSettings` with a partial patch,
 * using type assertions (`as SongCodec`, `as VideoResolution`) to narrow
 * the string from the native <select> element to the expected union type.
 */
export function QualityTab() {
  /** Current settings snapshot from the Zustand store */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function that sets isDirty = true */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /**
   * Transform the SONG_CODEC_LABELS record into the array format expected
   * by the <Select> component: [{ value, label }, ...].
   * Object.entries returns [string, string][] tuples from the Record.
   */
  const codecOptions = Object.entries(SONG_CODEC_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  /**
   * Same transformation for video resolution labels.
   */
  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(
    ([value, label]) => ({ value, label }),
  );

  /**
   * Video remux format options are hardcoded here rather than derived from
   * a type map because there are only three and they require custom
   * descriptive labels.
   */
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
