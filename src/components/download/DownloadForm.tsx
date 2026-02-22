// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file Download form component.
 *
 * Provides the URL input field with real-time multi-service URL validation,
 * service detection badges, content-type detection badges, and an "Add to
 * Queue" button. An expandable "Quality Overrides" panel allows Apple Music
 * users to override the global audio codec and video resolution settings
 * on a per-download basis.
 *
 * ## Data flow
 *
 * 1. User types (or pastes) a URL into the input field.
 * 2. `setUrlInput(url)` is called on every keystroke, which:
 *    a. Stores the raw URL string in `downloadStore.urlInput`.
 *    b. Runs `parseUrl(url)` (from `@/lib/url-parser.ts`) to detect the
 *       media service and content type from the URL domain and path.
 *    c. Updates `urlIsValid`, `urlContentType`, and `detectedServiceId`.
 * 3. When the URL is valid, coloured badges appear inside the input
 *    showing the detected service and content type.
 * 4. Clicking "Add to Queue" (or pressing Enter) calls
 *    `downloadStore.submitDownload()`, which:
 *    a. Sends the URL (with service_id and optional quality overrides)
 *       to the Rust backend via the `startDownload` Tauri command.
 *    b. Clears the input on success and shows a success toast.
 *    c. Shows an error toast on failure.
 *
 * ## Store connections
 *
 *  - {@link useDownloadStore} -- URL input state, validation state,
 *    override options, and the `submitDownload()` action.
 *  - {@link useSettingsStore} -- reads `defaultSongCodec` to
 *    display the current default in the quality overrides section.
 *  - {@link useUiStore}       -- `addToast()` for success/error notifications.
 *
 * @see https://react.dev/reference/react/useState  -- local showOverrides state.
 * @see https://react.dev/learn/responding-to-events -- event handlers.
 * @see https://lucide.dev/icons/                    -- icons for content types.
 */

/**
 * React `useState` hook used for the local `showOverrides` toggle.
 * @see https://react.dev/reference/react/useState
 */
import { useState } from 'react';

/**
 * Lucide React icons used for content-type badges and UI controls.
 *
 * Content-type icon mapping:
 *  - `Music`      -> song/track   (@see https://lucide.dev/icons/music)
 *  - `Disc3`      -> album        (@see https://lucide.dev/icons/disc-3)
 *  - `ListMusic`  -> playlist     (@see https://lucide.dev/icons/list-music)
 *  - `Video`      -> music-video/video (@see https://lucide.dev/icons/video)
 *  - `User`       -> artist/channel (@see https://lucide.dev/icons/user)
 *  - `Tv`         -> episode/programme/series (@see https://lucide.dev/icons/tv)
 *  - `Radio`      -> live-stream  (@see https://lucide.dev/icons/radio)
 *  - `Scissors`   -> clip         (@see https://lucide.dev/icons/scissors)
 *  - `HelpCircle` -> unknown      (@see https://lucide.dev/icons/help-circle)
 *
 * UI controls:
 *  - `ChevronDown` / `ChevronUp` -> expand/collapse quality overrides
 *  - `Plus` -> "Add to Queue" button icon
 */
import {
  Music,
  Disc3,
  ListMusic,
  Video,
  User,
  Tv,
  Radio,
  Scissors,
  HelpCircle,
  ChevronDown,
  ChevronUp,
  Plus,
  Search,
  Zap,
} from 'lucide-react';

/**
 * Zustand store hooks for reactive state management.
 * @see useDownloadStore in @/stores/downloadStore.ts  -- URL input & submission.
 * @see useSettingsStore in @/stores/settingsStore.ts  -- global quality defaults.
 * @see useUiStore in @/stores/uiStore.ts              -- toast notifications.
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { useServiceStatusStore } from '@/stores/serviceStatusStore';

/** Reusable UI primitives from the common component library. */
import { Button, Select } from '@/components/common';

/**
 * Type imports for media content types, service IDs, and audio codecs.
 * @see MediaContentType in @/types/index.ts -- 'song' | 'album' | 'video' | ... | 'unknown'
 * @see MediaServiceId in @/types/index.ts   -- 'apple-music' | 'youtube' | ...
 * @see SongCodec in @/types/index.ts        -- 'alac' | 'atmos' | 'ac3' | ...
 */
import type {
  MediaContentType,
  MediaServiceId,
  SongCodec,
  VideoResolution,
  CrossPlatformMatch,
} from '@/types';

/**
 * Human-readable label maps used to populate quality-override dropdowns
 * and service/content-type badges.
 * @see SONG_CODEC_LABELS in @/types/index.ts        -- e.g., { alac: 'Lossless (ALAC)' }
 * @see VIDEO_RESOLUTION_LABELS in @/types/index.ts   -- e.g., { '2160p': '4K (2160p)' }
 * @see MEDIA_SERVICE_LABELS in @/types/index.ts      -- e.g., { youtube: 'YouTube' }
 * @see MEDIA_SERVICE_COLORS in @/types/index.ts      -- e.g., { youtube: '#ff0000' }
 */
import {
  SONG_CODEC_LABELS,
  VIDEO_RESOLUTION_LABELS,
  MEDIA_SERVICE_LABELS,
  MEDIA_SERVICE_COLORS,
  QUALITY_TIER_LABELS,
} from '@/types';

/**
 * URL parser helpers for human-readable content type labels.
 * @see getContentTypeLabel in @/lib/url-parser.ts
 */
import { getContentTypeLabel, getServiceLabel } from '@/lib/url-parser';
import { checkCrossPlatform } from '@/lib/tauri-commands';

/** Page header component for consistent page-level headings. */
import { PageHeader } from '@/components/layout';

/**
 * Icon mapping for detected media content types across all services.
 *
 * When the URL parser detects the content type from the URL path
 * (e.g., `/album/` -> 'album', `/watch?v=` -> 'video'), the corresponding
 * Lucide icon component is looked up from this record and rendered inside
 * the content-type badge.
 *
 * Keyed by {@link MediaContentType}; values are Lucide React
 * component constructors (typed as `typeof Music` for consistency).
 * @see https://lucide.dev/icons/  -- full Lucide icon reference.
 */
const CONTENT_TYPE_ICONS: Partial<Record<MediaContentType, typeof Music>> = {
  song: Music, // Apple Music song
  album: Disc3, // Album (Apple Music, Spotify)
  playlist: ListMusic, // Playlist (Apple Music, YouTube, Spotify)
  'music-video': Video, // Apple Music music video
  artist: User, // Artist page (Apple Music, Spotify)
  video: Video, // YouTube video
  channel: User, // YouTube channel
  'live-stream': Radio, // YouTube live stream
  track: Music, // Spotify track
  episode: Tv, // BBC iPlayer episode
  series: Tv, // BBC iPlayer series
  programme: Tv, // BBC iPlayer programme
  clip: Scissors, // BBC iPlayer clip
  unknown: HelpCircle, // Unrecognised or unparseable URL
};

/**
 * Renders the download page with URL input, content-type detection,
 * optional per-download quality overrides, and the "Add to Queue" action.
 *
 * This is the primary entry point for initiating downloads. Users paste
 * an Apple Music URL, optionally adjust quality settings, and click
 * "Add to Queue" to send the request to the Rust backend.
 *
 * ## Sections
 *
 * 1. **Page header** -- "Download" title + description.
 * 2. **URL input** -- text field with real-time validation and a
 *    content-type badge overlay.
 * 3. **Quality overrides** -- collapsible panel with audio codec and
 *    video resolution dropdowns. When collapsed, global defaults from
 *    {@link useSettingsStore} are used.
 *
 * @see https://react.dev/learn/responding-to-events  -- onClick / onKeyDown
 * @see https://react.dev/reference/react/useState     -- showOverrides toggle
 */
export function DownloadForm() {
  // ---------------------------------------------------------------
  // Store selectors (Zustand)
  // ---------------------------------------------------------------
  // Each selector subscribes to a single slice of download store
  // state to minimise unnecessary re-renders.

  /** The current text in the URL input field. */
  const urlInput = useDownloadStore((s) => s.urlInput);
  /**
   * Whether `urlInput` is a syntactically valid media URL.
   * Set automatically by `setUrlInput()` via `parseUrl()`.
   * @see parseUrl in @/lib/url-parser.ts
   */
  const urlIsValid = useDownloadStore((s) => s.urlIsValid);
  /**
   * Detected content type string (e.g., 'album', 'song', 'video', 'episode').
   * Used to look up the icon and label for the in-input badge.
   */
  const urlContentType = useDownloadStore((s) => s.urlContentType);
  /**
   * Detected media service ('apple-music', 'youtube', etc.) or null.
   * Used to display a coloured service badge and determine which
   * quality overrides to show.
   */
  const detectedServiceId = useDownloadStore((s) => s.detectedServiceId);
  /** True while the `submitDownload()` promise is pending (disables button). */
  const isSubmitting = useDownloadStore((s) => s.isSubmitting);
  /**
   * Updates `urlInput`, re-validates, and re-detects service + content type.
   * Called on every keystroke in the URL input (`onChange`).
   * Internally calls `parseUrl()` from `@/lib/url-parser.ts`.
   */
  const setUrlInput = useDownloadStore((s) => s.setUrlInput);
  /**
   * Submits the current URL (+ optional overrides) to the Rust backend
   * via the `startDownload` Tauri command. Returns a Promise that
   * resolves with the download ID on success.
   * @see downloadStore.submitDownload in @/stores/downloadStore.ts
   */
  const submitDownload = useDownloadStore((s) => s.submitDownload);
  /**
   * Per-download quality overrides (or `null` to use global defaults).
   * Contains optional fields like `song_codec` and `music_video_resolution`.
   * @see GamdlOptions in @/types/index.ts
   */
  const overrideOptions = useDownloadStore((s) => s.overrideOptions);
  /** Sets (or clears) per-download quality overrides. */
  const setOverrideOptions = useDownloadStore((s) => s.setOverrideOptions);
  /**
   * Narrow Zustand selectors for the two settings fields actually used by
   * this component. Subscribing to the full `settings` object would cause
   * DownloadForm to re-render on ANY setting change (language, lyrics,
   * output dir, etc.), which is wasteful and risks re-render loops.
   * @see debugging.md -- "Key Zustand Lesson" on selector anti-patterns
   */
  const defaultSongCodec = useSettingsStore((s) => s.settings.default_song_codec);
  /** Shows a toast notification (success/error) after submission. */
  const addToast = useUiStore((s) => s.addToast);

  // ---------------------------------------------------------------
  // Local component state
  // ---------------------------------------------------------------

  /**
   * Controls visibility of the quality override panel.
   * `false` by default -- the panel starts collapsed.
   * @see https://react.dev/reference/react/useState
   */
  const [showOverrides, setShowOverrides] = useState(false);

  /** Smart Download state: whether the panel is expanded. */
  const [showSmartDownload, setShowSmartDownload] = useState(false);
  /** Smart Download state: loading indicator while searching cross-platform. */
  const [smartDownloadLoading, setSmartDownloadLoading] = useState(false);
  /** Smart Download state: cross-platform match results (null = not yet searched). */
  const [smartDownloadMatches, setSmartDownloadMatches] = useState<CrossPlatformMatch[] | null>(
    null
  );

  /** Whether Smart Download is enabled in settings. */
  const smartDownloadEnabled = useSettingsStore((s) => s.settings.smart_download_enabled);

  // ---------------------------------------------------------------
  // Event handlers
  // ---------------------------------------------------------------

  /**
   * Handles the "Add to Queue" action.
   *
   * Calls `submitDownload()` which:
   *  1. Validates the URL (throws if invalid).
   *  2. Calls the Rust `startDownload` command via Tauri IPC.
   *  3. Clears the input and overrides on success.
   *
   * Shows a success toast on success, or an error toast on failure.
   */
  const handleSubmit = async () => {
    try {
      await submitDownload();
      addToast('Download added to queue', 'success');
    } catch {
      addToast('Failed to add download', 'error');
    }
  };

  /**
   * Handles the Enter key in the URL input field.
   * Submits the form if the URL is valid and no submission is in progress.
   * This provides a keyboard-friendly shortcut so users don't need to
   * click the "Add to Queue" button.
   *
   * @see https://react.dev/learn/responding-to-events
   */
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && urlIsValid && !isSubmitting) {
      handleSubmit();
    }
  };

  /**
   * Map from PascalCase service IDs (as used in Rust/JSON) to the
   * kebab-case MediaServiceId used by the frontend. Needed because
   * the Smart Download results use PascalCase keys.
   */
  const SERVICE_KEY_TO_ID: Record<string, string> = {
    AppleMusic: 'apple-music',
    YouTube: 'youtube',
    BBCiPlayer: 'bbc-iplayer',
    Spotify: 'spotify',
  };

  /**
   * Map from kebab-case MediaServiceId to the PascalCase key used
   * by the Rust backend's Smart Download command.
   */
  const SERVICE_ID_TO_KEY: Record<string, string> = {
    'apple-music': 'AppleMusic',
    youtube: 'YouTube',
    'bbc-iplayer': 'BBCiPlayer',
    spotify: 'Spotify',
  };

  /**
   * Handles the "Check other platforms" action in the Smart Download panel.
   * Calls the Rust `check_cross_platform` command and updates the local state.
   */
  const handleSmartDownloadCheck = async () => {
    if (!detectedServiceId || !urlIsValid) return;
    setSmartDownloadLoading(true);
    setSmartDownloadMatches(null);
    try {
      const serviceKey = SERVICE_ID_TO_KEY[detectedServiceId] || detectedServiceId;
      const result = await checkCrossPlatform(urlInput, serviceKey);
      setSmartDownloadMatches(result.matches);
    } catch {
      addToast('Smart Download check failed', 'error');
    } finally {
      setSmartDownloadLoading(false);
    }
  };

  // ---------------------------------------------------------------
  // Derived values
  // ---------------------------------------------------------------

  /**
   * Cast the raw content type string to the `MediaContentType` union
   * so it can be used as a lookup key in the icon/label records.
   */
  const contentType = urlContentType as MediaContentType;

  /**
   * Resolve the Lucide icon component for the detected content type.
   * Falls back to `HelpCircle` if the type is not in the map (defensive).
   */
  const ContentIcon = CONTENT_TYPE_ICONS[contentType] || HelpCircle;

  /**
   * Whether the detected service is Apple Music.
   * Quality overrides (codec, resolution) are only applicable to Apple Music
   * downloads via GAMDL. Other services use their own settings.
   */
  const isAppleMusic = detectedServiceId === 'apple-music';

  /**
   * Whether the detected service is currently implemented.
   * Non-Apple Music services show a "Coming Soon" indicator.
   */
  const isServiceImplemented = detectedServiceId === 'apple-music';

  /**
   * Whether the detected service has been remotely disabled via the kill-switch.
   * Reads from the serviceStatusStore (populated on app launch + every 4 hours).
   */
  const isServiceDisabled = useServiceStatusStore((s) => s.isServiceDisabled);
  const getServiceMessage = useServiceStatusStore((s) => s.getServiceMessage);
  const isDetectedServiceDisabled = detectedServiceId
    ? isServiceDisabled(detectedServiceId)
    : false;
  const disabledMessage = detectedServiceId ? getServiceMessage(detectedServiceId) : null;

  /**
   * Transform the `SONG_CODEC_LABELS` record into an array of
   * `{ value, label }` objects suitable for the `<Select>` component.
   * Each entry maps a `SongCodec` value to its human-readable label
   * (e.g., 'alac' -> 'Lossless (ALAC)').
   */
  const codecOptions = Object.entries(SONG_CODEC_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  /**
   * Transform the `VIDEO_RESOLUTION_LABELS` record into an array of
   * `{ value, label }` objects for the video resolution `<Select>`.
   * (e.g., '2160p' -> '4K (2160p)').
   */
  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  // ---------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------
  return (
    <div>
      {/*
       * Page header: "Download" title with a brief description.
       * Uses the shared PageHeader component for consistent styling.
       * @see PageHeader in @/components/layout/PageHeader.tsx
       */}
      <PageHeader
        title="Download"
        subtitle="Paste a URL from Apple Music, YouTube, Spotify, or BBC iPlayer"
      />

      {/*
       * Form body -- constrained to `max-w-2xl` (672px) for readability.
       * `space-y-6` (24px) separates the URL input section from the
       * quality overrides section.
       */}
      <div className="p-6 max-w-2xl space-y-6">
        {/* =========================================================
         * Section 1: URL Input
         * =========================================================
         * Contains the text input, content-type badge, "Add to Queue"
         * button, and validation feedback text.
         */}
        <div className="space-y-2">
          {/* Accessible <label> linked to the input via `htmlFor` */}
          <label htmlFor="url-input" className="block text-sm font-medium text-content-primary">
            Media URL
          </label>

          {/* Input row: text field + submit button side-by-side */}
          <div className="flex gap-2">
            {/*
             * URL text input with inline content-type badge.
             * `relative` positioning on the wrapper allows the badge
             * to be absolutely positioned inside the input.
             */}
            <div className="flex-1 relative">
              {/*
               * Controlled text input bound to `downloadStore.urlInput`.
               *
               * onChange: calls `setUrlInput()` which validates the URL
               *   in real-time via `parseUrl()` from
               *   `@/lib/url-parser.ts`.
               * onKeyDown: submits on Enter if URL is valid.
               *
               * Border colour changes based on validation state:
               *  - Red (`border-status-error`) when input is non-empty
               *    but the URL is invalid.
               *  - Default (`border-border`) otherwise, with accent
               *    colour on focus (`focus:border-accent`).
               *
               * @see https://react.dev/reference/react-dom/components/input
               */}
              <input
                id="url-input"
                type="text"
                value={urlInput}
                onChange={(e) => setUrlInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="https://music.apple.com/... or any supported URL"
                className={`
                  w-full px-3 py-2 text-sm rounded-platform border
                  bg-surface-secondary text-content-primary
                  placeholder-content-tertiary
                  transition-colors
                  focus:ring-1 focus:ring-accent
                  ${urlInput && !urlIsValid ? 'border-status-error focus:border-status-error' : 'border-border focus:border-accent'}
                `}
              />

              {/*
               * Service + content-type badges -- absolutely positioned
               * inside the input (right-aligned, vertically centred).
               *
               * Only shown when `urlIsValid` is true. Displays two
               * pill-shaped badges:
               *   1. Service badge (coloured by service) showing "Apple Music", "YouTube", etc.
               *   2. Content-type badge showing "Album", "Video", etc.
               */}
              {urlIsValid && detectedServiceId && (
                <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1.5">
                  {/* Service badge -- uses the service's brand colour */}
                  <div
                    className="flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium text-white"
                    style={{ backgroundColor: MEDIA_SERVICE_COLORS[detectedServiceId] }}
                  >
                    {MEDIA_SERVICE_LABELS[detectedServiceId]}
                  </div>
                  {/* Content-type badge */}
                  <div className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-light text-accent text-xs font-medium">
                    <ContentIcon size={12} />
                    {getContentTypeLabel(contentType)}
                  </div>
                </div>
              )}
            </div>

            {/*
             * "Add to Queue" button.
             *
             * Disabled when the URL is invalid (`!urlIsValid`).
             * Shows a loading spinner when `isSubmitting` is true.
             * The Plus icon prefixes the button label.
             *
             * @see Button component in @/components/common
             */}
            <Button
              variant="primary"
              icon={<Plus size={16} />}
              loading={isSubmitting}
              disabled={!urlIsValid || !isServiceImplemented || isDetectedServiceDisabled}
              onClick={handleSubmit}
            >
              {urlIsValid && isDetectedServiceDisabled
                ? 'Unavailable'
                : urlIsValid && !isServiceImplemented
                  ? 'Coming Soon'
                  : 'Add to Queue'}
            </Button>
          </div>

          {/*
           * Validation feedback messages (mutually exclusive):
           *  - Red error text when the input has text but URL is invalid.
           *  - Amber warning when the service is remotely disabled.
           *  - Amber warning when the service is not yet implemented.
           *  - Grey helper text when the input is empty (shows supported types).
           */}
          {urlInput && !urlIsValid && (
            <p className="text-xs text-status-error">
              Please enter a valid URL from Apple Music, YouTube, Spotify, or BBC iPlayer
            </p>
          )}
          {urlIsValid && isDetectedServiceDisabled && detectedServiceId && (
            <p className="text-xs text-status-warning">
              {MEDIA_SERVICE_LABELS[detectedServiceId]} downloads are temporarily unavailable.
              {disabledMessage ? ` ${disabledMessage}` : ''}
            </p>
          )}
          {urlIsValid &&
            !isDetectedServiceDisabled &&
            !isServiceImplemented &&
            detectedServiceId && (
              <p className="text-xs text-status-warning">
                {MEDIA_SERVICE_LABELS[detectedServiceId]} downloads are coming soon
              </p>
            )}
          {!urlInput && (
            <p className="text-xs text-content-tertiary">
              Supports Apple Music, YouTube, Spotify, and BBC iPlayer URLs
            </p>
          )}
        </div>

        {/* =========================================================
         * Section 2: Quality Overrides (collapsible, Apple Music only)
         * =========================================================
         * Allows users to override the global audio codec and video
         * resolution for this specific download. When set, the
         * overrides are passed as `GamdlOptions` to the backend.
         * When null, global defaults from settingsStore are used.
         * Only shown for Apple Music URLs (other services use their
         * own settings configured in Settings > [Service]).
         */}
        {(!detectedServiceId || isAppleMusic) && (
          <div className="rounded-platform border border-border-light">
            {/*
             * Collapsible header -- clicking toggles `showOverrides`.
             * Displays the current global default codec in parentheses
             * so users know what they would override.
             */}
            <button
              onClick={() => setShowOverrides(!showOverrides)}
              className="w-full flex items-center justify-between px-4 py-3 text-sm text-content-secondary hover:text-content-primary transition-colors"
            >
              <span>
                Quality Overrides{' '}
                <span className="text-content-tertiary">
                  (default: {SONG_CODEC_LABELS[defaultSongCodec]})
                </span>
              </span>
              {/* Chevron icon flips based on expand/collapse state */}
              {showOverrides ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
            </button>

            {/*
             * Expandable override panel -- only rendered when `showOverrides`
             * is true (conditional rendering). Contains:
             *  - Audio codec Select dropdown
             *  - Video resolution Select dropdown
             *  - "Clear overrides" button (shown only when overrides are set)
             */}
            {showOverrides && (
              <div className="px-4 pb-4 space-y-4 border-t border-border-light pt-4">
                {/*
                 * Audio codec override dropdown.
                 *
                 * Populated from `SONG_CODEC_LABELS` (e.g., 'alac' -> 'Lossless (ALAC)').
                 * Selecting an option updates `downloadStore.overrideOptions.song_codec`.
                 * Selecting the empty/placeholder option ("Use default") clears
                 * the override by setting `overrideOptions` to `null`.
                 *
                 * @see Select component in @/components/common
                 */}
                <Select
                  label="Audio Codec"
                  description="Override the default audio codec for this download"
                  options={codecOptions}
                  value={overrideOptions?.song_codec || ''}
                  onChange={(e) => {
                    const codec = e.target.value as SongCodec;
                    setOverrideOptions(codec ? { ...overrideOptions, song_codec: codec } : null);
                  }}
                  placeholder="Use default"
                />

                {/*
                 * Video resolution override dropdown.
                 *
                 * Populated from `VIDEO_RESOLUTION_LABELS` (e.g., '2160p' -> '4K (2160p)').
                 * Works identically to the audio codec dropdown above.
                 * The resolution value is cast to the `VideoResolution` type
                 * via `as VideoResolution`.
                 */}
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
                            music_video_resolution: res as VideoResolution,
                          }
                        : null
                    );
                  }}
                  placeholder="Use default"
                />

                {/*
                 * "Clear overrides" button.
                 * Only shown when `overrideOptions` is non-null (i.e., at
                 * least one override is set). Resets overrides to `null`,
                 * meaning global defaults will be used for the next download.
                 */}
                {overrideOptions && (
                  <Button variant="ghost" size="sm" onClick={() => setOverrideOptions(null)}>
                    Clear overrides
                  </Button>
                )}
              </div>
            )}
          </div>
        )}

        {/* =========================================================
         * Section 3: Smart Download (collapsible, all services)
         * =========================================================
         * When enabled in settings, shows a panel that searches all
         * enabled services for the same content and compares quality.
         * Only visible when a valid URL is detected and Smart Download
         * is enabled.
         */}
        {smartDownloadEnabled &&
          urlIsValid &&
          detectedServiceId &&
          isServiceImplemented &&
          !isDetectedServiceDisabled && (
            <div className="rounded-platform border border-border-light">
              <button
                onClick={() => setShowSmartDownload(!showSmartDownload)}
                className="w-full flex items-center justify-between px-4 py-3 text-sm text-content-secondary hover:text-content-primary transition-colors"
              >
                <span className="flex items-center gap-2">
                  <Zap size={14} className="text-accent" />
                  Smart Download
                  <span className="text-content-tertiary text-xs">
                    (compare quality across platforms)
                  </span>
                </span>
                {showSmartDownload ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              </button>

              {showSmartDownload && (
                <div className="px-4 pb-4 space-y-3 border-t border-border-light pt-4">
                  {/* Check button */}
                  <Button
                    variant="secondary"
                    size="sm"
                    icon={<Search size={14} />}
                    loading={smartDownloadLoading}
                    onClick={handleSmartDownloadCheck}
                  >
                    Check other platforms
                  </Button>

                  {/* Results list */}
                  {smartDownloadMatches && smartDownloadMatches.length > 0 && (
                    <div className="space-y-2">
                      <p className="text-xs text-content-tertiary font-medium">
                        Available quality by platform:
                      </p>
                      {smartDownloadMatches.map((match, i) => {
                        const kebabId = SERVICE_KEY_TO_ID[match.service_id] || match.service_id;
                        const isOriginal = match.match_method === 'original';
                        return (
                          <div
                            key={i}
                            className={`flex items-center justify-between p-2.5 rounded-md text-sm ${
                              isOriginal ? 'bg-accent-light/50' : 'bg-surface-secondary'
                            }`}
                          >
                            <div className="flex items-center gap-2">
                              <span className="font-medium text-content-primary">
                                {getServiceLabel(kebabId as MediaServiceId)}
                              </span>
                              {isOriginal && (
                                <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-accent/10 text-accent font-medium">
                                  Current
                                </span>
                              )}
                            </div>
                            <span className="text-xs px-2 py-0.5 rounded-full bg-surface-tertiary text-content-secondary font-medium">
                              {QUALITY_TIER_LABELS[match.best_quality] || match.best_quality}
                            </span>
                          </div>
                        );
                      })}
                      {smartDownloadMatches.length <= 1 && (
                        <p className="text-xs text-content-tertiary italic">
                          Cross-platform matching will find more results as additional services are
                          implemented.
                        </p>
                      )}
                    </div>
                  )}

                  {smartDownloadMatches && smartDownloadMatches.length === 0 && (
                    <p className="text-xs text-content-tertiary">
                      No cross-platform matches found.
                    </p>
                  )}
                </div>
              )}
            </div>
          )}
      </div>
    </div>
  );
}
