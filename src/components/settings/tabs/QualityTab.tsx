/**
 * Copyright (c) 2024-2026 MeedyaDL
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
 *   - **Companion Downloads** -- Controls automatic multi-format downloads.
 *     When enabled, specialist format downloads (Dolby Atmos, ALAC) also
 *     download companion versions in other formats. Maps to
 *     `settings.companion_mode`.
 *
 *   - **Artist Auto-Select** -- When downloading from an artist URL,
 *     controls which content type is automatically selected (main albums,
 *     compilations, singles, etc.). Maps to `settings.artist_auto_select`.
 *     Requires GAMDL 2.9.1+.
 *
 *   - **Default Video Resolution** -- The preferred resolution for music
 *     video downloads (e.g., 2160p for 4K). Maps to
 *     `settings.default_video_resolution`.
 *
 *   - **Video Codec Priority** -- A reorderable list of video codecs
 *     tried in order (H.265/HEVC and H.264/AVC). Stored internally as a
 *     comma-separated string in `settings.default_video_codec_priority`;
 *     the UI converts to/from an array at the boundary.
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
 * @see {@link @/types/index.ts}           -- SongCodec, VideoResolution, CompanionMode types
 */

// Zustand store for reading/writing quality settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components.
import { Select, Toggle, FallbackChainList, CheckboxGroup } from '@/components/common';

// Label maps and type definitions for audio codecs, video resolutions, video codecs, and companion modes.
// These Record<T, string> maps are used to populate the <Select> dropdown options and reorderable lists.
import {
  SONG_CODEC_LABELS,
  VIDEO_RESOLUTION_LABELS,
  VIDEO_CODEC_LABELS,
  COMPANION_MODE_LABELS,
  ARTIST_AUTO_SELECT_LABELS,
} from '@/types';
import type {
  SongCodec,
  VideoResolution,
  VideoCodec,
  CompanionMode,
  ArtistAutoSelect,
} from '@/types';

/** All valid video codec identifiers, used for type-guarding parsed strings. */
const VALID_VIDEO_CODECS: VideoCodec[] = ['h265', 'h264'];

/**
 * Parses a comma-separated video codec priority string into a typed array.
 * Filters out any invalid values and falls back to the default order if
 * the result is empty (e.g., stored value was blank or entirely invalid).
 */
function parseVideoCodecPriority(raw: string): VideoCodec[] {
  const parsed = raw
    .split(',')
    .map((s) => s.trim())
    .filter((s): s is VideoCodec => VALID_VIDEO_CODECS.includes(s as VideoCodec));
  return parsed.length > 0 ? parsed : [...VALID_VIDEO_CODECS];
}

/**
 * QualityTab -- Renders the Quality settings tab.
 *
 * Organised into two visual sections:
 *   1. "Audio Quality" -- codec selection, fallback toggle, companion mode
 *   2. "Video Quality" -- resolution, codec priority, and remux format
 *
 * Each control's `onChange` calls `updateSettings` with a partial patch,
 * using type assertions (`as SongCodec`, `as VideoResolution`, etc.) to
 * narrow the string from the native <select> element to the expected union type.
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
   * Transform companion mode labels into <Select> options.
   */
  const companionModeOptions = Object.entries(COMPANION_MODE_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  /** Whether custom companion mode is active (shows codec checkboxes). */
  const isCustomCompanion = settings.companion_mode === 'custom';

  /**
   * Same transformation for video resolution labels.
   */
  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

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
        <h3 className="text-base font-semibold text-content-primary mb-4">Audio Quality</h3>

        {/* Default audio codec */}
        <Select
          label="Default Audio Codec"
          description={
            <>
              The preferred codec for song downloads. Only <strong>AAC Legacy</strong> and{' '}
              <strong>AAC-HE Legacy</strong> are reliably downloadable with cookie-based
              authentication. All other codecs are marked <em>(Experimental)</em> and may fail
              intermittently without the Wrapper service. See Help &gt; Audio Codecs for details.
              <br />
              <br />
              <strong>ALAC</strong> = lossless (perfect quality, larger files);{' '}
              <strong>Dolby Atmos</strong> = immersive spatial audio; <strong>AC3</strong> =
              surround sound (5.1 home theatre); <strong>AAC</strong> = standard quality (smallest
              files, plays everywhere).
            </>
          }
          options={codecOptions}
          value={settings.default_song_codec}
          onChange={(e) => updateSettings({ default_song_codec: e.target.value as SongCodec })}
          helpTopic="audio-codecs"
        />

        {/* Fallback toggle */}
        <Toggle
          label="Enable Fallback Chain"
          description="When the preferred codec is unavailable, automatically try the next codec in the fallback chain"
          checked={settings.fallback_enabled}
          onChange={(checked) => updateSettings({ fallback_enabled: checked })}
        />

        {/* Companion download mode */}
        <Select
          label="Companion Downloads"
          description="Automatically download additional format versions alongside the primary download. Specialist formats get a suffix ([Dolby Atmos], [Lossless]); the most compatible companion uses a clean filename. Select 'Custom...' to pick exact codecs."
          options={companionModeOptions}
          value={settings.companion_mode}
          onChange={(e) => updateSettings({ companion_mode: e.target.value as CompanionMode })}
          helpTopic="audio-codecs"
        />

        {/* Custom companion codec checkboxes (only visible when Custom mode is selected) */}
        {isCustomCompanion && (
          <CheckboxGroup<SongCodec>
            label="Custom Companion Codecs"
            description="Select which codecs to download as companions alongside the primary format. Each selected codec runs as a separate download. The last codec in the list gets a clean filename; others get a suffix."
            options={SONG_CODEC_LABELS}
            selected={settings.custom_companion_codecs}
            onChange={(selected) => updateSettings({ custom_companion_codecs: selected })}
          />
        )}

        {/* Artist auto-select mode (multi-select) */}
        <CheckboxGroup<ArtistAutoSelect>
          label="Artist Auto-Select"
          description="When downloading from an artist URL, automatically download these content types. Select multiple to download each type separately (MeedyaDL creates one download per selected mode). Leave empty to use GAMDL's default behaviour. Requires GAMDL 2.9.1+."
          options={ARTIST_AUTO_SELECT_LABELS}
          selected={settings.artist_auto_select_multi}
          onChange={(selected) => {
            updateSettings({
              artist_auto_select_multi: selected,
              // Keep legacy field in sync for backwards compat
              artist_auto_select: selected.length > 0 ? selected[0] : null,
            });
          }}
        />
      </div>

      {/* Section: Video */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">Video Quality</h3>

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

        {/* Video codec priority (reorderable list) */}
        <div>
          <label className="block text-sm font-medium text-content-primary mb-1">
            Video Codec Priority
          </label>
          <p className="text-xs text-content-secondary mb-2">
            Order of preferred video codecs for music video downloads (top = tried first)
          </p>
          <FallbackChainList<VideoCodec>
            items={parseVideoCodecPriority(settings.default_video_codec_priority)}
            labels={VIDEO_CODEC_LABELS}
            onChange={(codecs) =>
              updateSettings({ default_video_codec_priority: codecs.join(',') })
            }
          />
        </div>

        {/* Remux format */}
        <Select
          label="Video Remux Format"
          description="Container format for remuxed video files"
          options={remuxOptions}
          value={settings.default_video_remux_format}
          onChange={(e) => updateSettings({ default_video_remux_format: e.target.value })}
        />
      </div>
    </div>
  );
}
