/**
 * Copyright (c) 2026 MeedyaSuite
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

// Audit v2 #6 — per-field Zustand binding.
import { useSettingsField } from '@/hooks/useSettingsField';

// Version-aware GAMDL capability flags (#963, #1002) — drives the
// wrapper-dependency prose below without touching the codec labels (#965).
import { useGamdlCapabilities } from '@/hooks/useGamdlCapabilities';

// Shared form components.
import { Select, Toggle, FallbackChainList, CheckboxGroup, SettingsSection } from '@/components/common';

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
  DuplicateDetectionScope,
  DedupKeyStrategy,
} from '@/types';

/** Display labels for the duplicate-detection scope setting. */
const DEDUP_SCOPE_LABELS: Record<DuplicateDetectionScope, string> = {
  off: 'Off — download everything',
  intra_session: 'Within this artist URL only',
  intra_and_queued: 'This URL + already-queued items (recommended)',
  intra_and_queued_and_history:
    'This URL + queue + download history (scans manifest files)',
};

/** Display labels for the dedup key strategy. */
const DEDUP_KEY_LABELS: Record<DedupKeyStrategy, string> = {
  song_id_isrc_fallback: 'Song ID (with ISRC fallback) — recommended',
  isrc_only: 'ISRC only — catches remasters as duplicates',
  song_id_only: 'Song ID only — strictest match',
};

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
  // Per-field Zustand bindings (audit v2 #6).
  const songCodec = useSettingsField('default_song_codec');
  const fallbackEnabled = useSettingsField('fallback_enabled');
  const companionMode = useSettingsField('companion_mode');
  const customCompanionCodecs = useSettingsField('custom_companion_codecs');
  const artistAutoSelectMulti = useSettingsField('artist_auto_select_multi');
  const artistAutoSelect = useSettingsField('artist_auto_select');
  const dupDetect = useSettingsField('duplicate_detection');
  const videoResolution = useSettingsField('default_video_resolution');
  const videoCodecPriority = useSettingsField('default_video_codec_priority');
  const videoRemuxFormat = useSettingsField('default_video_remux_format');
  const musicVideoCompanion = useSettingsField('music_video_companion');
  const musicbrainzLookup = useSettingsField('musicbrainz_lookup');
  // #963/#1002: on GAMDL 3.8+ only ALAC needs the Wrapper; swap the codec
  // description prose accordingly. The (Experimental) labels stay (#965).
  const { capabilities: gamdlCaps } = useGamdlCapabilities();

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
  const isCustomCompanion = companionMode.value === 'custom';

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
    <div className="space-y-3">
      {/* Section: Audio */}
      <SettingsSection title="Audio Quality">

        {/* Default audio codec */}
        <Select
          label="Default Audio Codec"
          description={
            <>
              {gamdlCaps.assets_api_unlocks_lossy_codecs ? (
                <>
                  The preferred codec for song downloads. On your installed GAMDL (3.8+), every
                  codec except <strong>Lossless (ALAC)</strong> downloads with cookie-based
                  authentication alone — ALAC still requires the Wrapper service. Codecs marked{' '}
                  <em>(Experimental)</em> may still fail intermittently. See Help &gt; Audio Codecs
                  for details.
                </>
              ) : (
                <>
                  The preferred codec for song downloads. Only <strong>AAC Legacy</strong> and{' '}
                  <strong>AAC-HE Legacy</strong> are reliably downloadable with cookie-based
                  authentication. All other codecs are marked <em>(Experimental)</em> and may fail
                  intermittently without the Wrapper service. See Help &gt; Audio Codecs for details.
                </>
              )}
              <br />
              <br />
              <strong>ALAC</strong> = lossless (perfect quality, larger files);{' '}
              <strong>Dolby Atmos</strong> = immersive spatial audio; <strong>AC3</strong> =
              surround sound (5.1 home theatre); <strong>AAC</strong> = standard quality (smallest
              files, plays everywhere).
            </>
          }
          options={codecOptions}
          value={songCodec.value}
          onChange={(e) => songCodec.set(e.target.value as SongCodec)}
          helpTopic="audio-codecs"
        />

        {/* Fallback toggle */}
        <Toggle
          label="Enable Fallback Chain"
          description="When the preferred codec is unavailable, automatically try the next codec in the fallback chain"
          checked={fallbackEnabled.value}
          onChange={fallbackEnabled.set}
        />

        {/* Companion download mode */}
        <Select
          label="Companion Downloads"
          description="Automatically download additional format versions alongside the primary download. Specialist formats get a suffix ([Dolby Atmos], [Lossless]); the most compatible companion uses a clean filename. Select 'Custom...' to pick exact codecs."
          options={companionModeOptions}
          value={companionMode.value}
          onChange={(e) => companionMode.set(e.target.value as CompanionMode)}
          helpTopic="audio-codecs"
        />

        {/* Custom companion codec checkboxes (only visible when Custom mode is selected) */}
        {isCustomCompanion && (
          <CheckboxGroup<SongCodec>
            label="Custom Companion Codecs"
            description="Select which codecs to download as companions alongside the primary format. Each selected codec runs as a separate download. The last codec in the list gets a clean filename; others get a suffix."
            options={SONG_CODEC_LABELS}
            selected={customCompanionCodecs.value}
            onChange={customCompanionCodecs.set}
          />
        )}

        {/* Artist auto-select mode (multi-select). Two writes per change
            because the legacy `artist_auto_select` scalar is kept in
            sync with the multi-select array for backwards compat. */}
        <CheckboxGroup<ArtistAutoSelect>
          label="Artist Auto-Select"
          description="When downloading from an artist URL, automatically download these content types. Select multiple to download each type separately (MeedyaDL creates one download per selected mode). Leave empty to use GAMDL's default behaviour. Requires GAMDL 2.9.1+."
          options={ARTIST_AUTO_SELECT_LABELS}
          selected={artistAutoSelectMulti.value}
          onChange={(selected) => {
            artistAutoSelectMulti.set(selected);
            artistAutoSelect.set(selected.length > 0 ? selected[0] : null);
          }}
        />
      </SettingsSection>

      {/* Section: Duplicate detection (#510) */}
      <SettingsSection title="Duplicate Detection">
        <p className="text-sm text-content-secondary">
          When an artist URL is downloaded with multiple Artist Auto-Select modes
          (e.g. main albums + singles/EPs + compilations), the same song can
          appear in multiple modes. Duplicate detection queries the Apple
          Music catalog API before queueing, then skips duplicates so each
          song is only downloaded once. This does <strong>not</strong> affect
          companion-format downloads — the winning song still runs the full
          ALAC/Atmos/AAC companion chain you've configured.
        </p>

        <Select
          label="Scope"
          description="How far to look when deciding whether a track is a duplicate."
          options={(['off', 'intra_session', 'intra_and_queued', 'intra_and_queued_and_history'] as DuplicateDetectionScope[]).map((v) => ({
            value: v,
            label: DEDUP_SCOPE_LABELS[v],
          }))}
          value={dupDetect.value.scope}
          onChange={(e) =>
            dupDetect.set({
              ...dupDetect.value,
              scope: e.target.value as DuplicateDetectionScope,
            })
          }
        />

        <Select
          label="Match key"
          description="Which identifier to match tracks on. Song ID is the strictest — it matches only the exact same master file. ISRC also matches remasters and re-releases of the same recording."
          options={(['song_id_isrc_fallback', 'isrc_only', 'song_id_only'] as DedupKeyStrategy[]).map((v) => ({
            value: v,
            label: DEDUP_KEY_LABELS[v],
          }))}
          value={dupDetect.value.key_strategy}
          onChange={(e) =>
            dupDetect.set({
              ...dupDetect.value,
              key_strategy: e.target.value as DedupKeyStrategy,
            })
          }
        />

        <div>
          <label className="block text-sm font-medium text-content-primary mb-1">
            Version preference
          </label>
          <p className="text-sm text-content-secondary mb-2">
            When a song appears in multiple Artist Auto-Select modes, the
            version from the highest-priority mode wins and the others are
            skipped. Reorder to change which version is kept.
          </p>
          <FallbackChainList<ArtistAutoSelect>
            items={dupDetect.value.preference_order}
            labels={ARTIST_AUTO_SELECT_LABELS}
            onChange={(items) =>
              dupDetect.set({
                ...dupDetect.value,
                preference_order: items,
              })
            }
          />
        </div>
      </SettingsSection>

      {/* Section: Video */}
      <SettingsSection title="Video Quality">

        {/* Default video resolution */}
        <Select
          label="Default Video Resolution"
          description="The preferred resolution for music video downloads"
          options={resolutionOptions}
          value={videoResolution.value}
          onChange={(e) => videoResolution.set(e.target.value as VideoResolution)}
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
            items={parseVideoCodecPriority(videoCodecPriority.value)}
            labels={VIDEO_CODEC_LABELS}
            onChange={(codecs) => videoCodecPriority.set(codecs.join(','))}
          />
        </div>

        {/* Remux format */}
        <Select
          label="Video Remux Format"
          description="Container format for remuxed video files"
          options={remuxOptions}
          value={videoRemuxFormat.value}
          onChange={(e) => videoRemuxFormat.set(e.target.value)}
        />

        {/* Music video companion downloads */}
        <Toggle
          label="Music Video Companions (Experimental)"
          description="When downloading audio, also download the music video for each track (if available on Apple Music). Uses the video quality settings above. Discovery via Apple Music API (requires MusicKit credentials) and/or MusicBrainz ISRC lookup (no credentials needed)."
          checked={musicVideoCompanion.value}
          onChange={musicVideoCompanion.set}
        />

        {musicVideoCompanion.value && (
          <div className="p-3 rounded-lg bg-status-warning-bg border border-status-warning">
            <p className="text-xs font-semibold text-status-warning mb-1">
              Experimental Feature
            </p>
            <p className="text-xs text-status-warning">
              Music video discovery uses two sources: the Apple Music API (if MusicKit credentials are
              configured in Settings &gt; Advanced &gt; API Credentials) and MusicBrainz ISRC lookups
              (no credentials needed). Results may vary — not all tracks have music videos, and
              MusicBrainz coverage depends on community contributions.
            </p>
          </div>
        )}

        {/* MusicBrainz video lookup (no credentials needed) */}
        <Toggle
          label="MusicBrainz Video Lookup"
          description="Use MusicBrainz database to discover music videos and cross-platform URLs via ISRC codes. No credentials required. Also used by Music Video Companions when MusicKit credentials are not configured."
          checked={musicbrainzLookup.value}
          onChange={musicbrainzLookup.set}
        />
      </SettingsSection>
    </div>
  );
}
