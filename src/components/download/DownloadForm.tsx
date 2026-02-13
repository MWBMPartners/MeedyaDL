// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file Download form component.
 *
 * Provides the URL input field with real-time Apple Music URL validation,
 * a content-type detection badge, and an "Add to Queue" button. An
 * expandable "Quality Overrides" panel allows users to override the
 * global audio codec and video resolution settings on a per-download basis.
 *
 * ## Data flow
 *
 * 1. User types (or pastes) a URL into the input field.
 * 2. `setUrlInput(url)` is called on every keystroke, which:
 *    a. Stores the raw URL string in `downloadStore.urlInput`.
 *    b. Runs `parseAppleMusicUrl(url)` (from `@/lib/url-parser.ts`) to
 *       validate the URL format and detect the content type (song, album,
 *       playlist, music-video, artist, or unknown).
 *    c. Updates `downloadStore.urlIsValid` and `downloadStore.urlContentType`.
 * 3. When the URL is valid, a coloured badge appears inside the input
 *    showing the detected content type with a matching Lucide icon.
 * 4. Clicking "Add to Queue" (or pressing Enter) calls
 *    `downloadStore.submitDownload()`, which:
 *    a. Sends the URL (and optional quality overrides) to the Rust backend
 *       via the `startDownload` Tauri command.
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
 * ## URL validation
 *
 * Valid Apple Music URLs match the pattern:
 *   `https://music.apple.com/{storefront}/{type}/{name}/{id}`
 * where `{type}` is one of: song, album, playlist, music-video, artist.
 * The regex-based parser lives in `@/lib/url-parser.ts`.
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
 *  - `Music`      -> song         (@see https://lucide.dev/icons/music)
 *  - `Disc3`      -> album        (@see https://lucide.dev/icons/disc-3)
 *  - `ListMusic`  -> playlist     (@see https://lucide.dev/icons/list-music)
 *  - `Video`      -> music-video  (@see https://lucide.dev/icons/video)
 *  - `User`       -> artist       (@see https://lucide.dev/icons/user)
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
  HelpCircle,
  ChevronDown,
  ChevronUp,
  Plus,
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

/** Reusable UI primitives from the common component library. */
import { Button, Select } from '@/components/common';

/**
 * Type imports for Apple Music content types and audio codecs.
 * @see AppleMusicContentType in @/types/index.ts -- 'song' | 'album' | ... | 'unknown'
 * @see SongCodec in @/types/index.ts             -- 'alac' | 'atmos' | 'ac3' | ...
 */
import type { AppleMusicContentType, SongCodec, VideoResolution } from '@/types';

/**
 * Human-readable label maps used to populate quality-override dropdowns.
 * @see SONG_CODEC_LABELS in @/types/index.ts        -- e.g., { alac: 'Lossless (ALAC)' }
 * @see VIDEO_RESOLUTION_LABELS in @/types/index.ts   -- e.g., { '2160p': '4K (2160p)' }
 */
import { SONG_CODEC_LABELS, VIDEO_RESOLUTION_LABELS } from '@/types';

/** Page header component for consistent page-level headings. */
import { PageHeader } from '@/components/layout';

/**
 * Icon mapping for detected Apple Music content types.
 *
 * When the URL parser detects the content type from the URL path
 * (e.g., `/album/` -> 'album'), the corresponding Lucide icon component
 * is looked up from this record and rendered inside the content-type badge.
 *
 * Keyed by {@link AppleMusicContentType}; values are Lucide React
 * component constructors (typed as `typeof Music` for consistency).
 * @see https://lucide.dev/icons/  -- full Lucide icon reference.
 */
const CONTENT_TYPE_ICONS: Record<AppleMusicContentType, typeof Music> = {
  song: Music,           // Single song URL
  album: Disc3,          // Album URL
  playlist: ListMusic,   // Playlist URL
  'music-video': Video,  // Music video URL
  artist: User,          // Artist page URL
  unknown: HelpCircle,   // Unrecognised or unparseable URL
};

/**
 * Human-readable labels for each content type, displayed in the
 * content-type badge that appears inside the URL input when the URL
 * is valid. E.g., a valid album URL shows a badge reading "Album".
 */
const CONTENT_TYPE_LABELS: Record<AppleMusicContentType, string> = {
  song: 'Song',
  album: 'Album',
  playlist: 'Playlist',
  'music-video': 'Music Video',
  artist: 'Artist',
  unknown: 'Unknown',
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
   * Whether `urlInput` is a syntactically valid Apple Music URL.
   * Set automatically by `setUrlInput()` via `parseAppleMusicUrl()`.
   * @see parseAppleMusicUrl in @/lib/url-parser.ts
   */
  const urlIsValid = useDownloadStore((s) => s.urlIsValid);
  /**
   * Detected content type string (e.g., 'album', 'song', 'playlist').
   * Used to look up the icon and label for the in-input badge.
   */
  const urlContentType = useDownloadStore((s) => s.urlContentType);
  /** True while the `submitDownload()` promise is pending (disables button). */
  const isSubmitting = useDownloadStore((s) => s.isSubmitting);
  /**
   * Updates `urlInput`, re-validates, and re-detects content type.
   * Called on every keystroke in the URL input (`onChange`).
   * Internally calls `parseAppleMusicUrl()` from `@/lib/url-parser.ts`.
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

  // ---------------------------------------------------------------
  // Derived values
  // ---------------------------------------------------------------

  /**
   * Cast the raw content type string to the `AppleMusicContentType` union
   * so it can be used as a lookup key in the icon/label records.
   */
  const contentType = urlContentType as AppleMusicContentType;

  /**
   * Resolve the Lucide icon component for the detected content type.
   * Falls back to `HelpCircle` if the type is not in the map (defensive).
   */
  const ContentIcon = CONTENT_TYPE_ICONS[contentType] || HelpCircle;

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
  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(
    ([value, label]) => ({ value, label }),
  );

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
        subtitle="Enter an Apple Music URL to download music or videos"
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
          <label
            htmlFor="url-input"
            className="block text-sm font-medium text-content-primary"
          >
            Apple Music URL
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
               *   in real-time via `parseAppleMusicUrl()` from
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

              {/*
               * Content-type badge -- absolutely positioned inside the
               * input (right-aligned, vertically centred).
               *
               * Only shown when `urlIsValid` is true. Displays the
               * detected content type icon + label (e.g., "Album").
               * The icon component is resolved from `CONTENT_TYPE_ICONS`
               * and the label from `CONTENT_TYPE_LABELS`.
               *
               * `rounded-full` makes it pill-shaped.
               * `bg-accent-light text-accent` uses the accent colour
               * at a light tint for the background.
               */}
              {urlIsValid && (
                <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-light text-accent text-xs font-medium">
                  <ContentIcon size={12} />
                  {CONTENT_TYPE_LABELS[contentType]}
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
              disabled={!urlIsValid}
              onClick={handleSubmit}
            >
              Add to Queue
            </Button>
          </div>

          {/*
           * Validation feedback messages (mutually exclusive):
           *  - Red error text when the input has text but URL is invalid.
           *  - Grey helper text when the input is empty (shows supported types).
           */}
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

        {/* =========================================================
         * Section 2: Quality Overrides (collapsible)
         * =========================================================
         * Allows users to override the global audio codec and video
         * resolution for this specific download. When set, the
         * overrides are passed as `GamdlOptions` to the backend.
         * When null, global defaults from settingsStore are used.
         */}
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
            {showOverrides ? (
              <ChevronUp size={16} />
            ) : (
              <ChevronDown size={16} />
            )}
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
                  setOverrideOptions(
                    codec
                      ? { ...overrideOptions, song_codec: codec }
                      : null,
                  );
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
                      : null,
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
