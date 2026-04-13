// Copyright (c) 2026 MeedyaDL
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
 * React hooks used for local component state and side effects.
 * @see https://react.dev/reference/react/useState
 * @see https://react.dev/reference/react/useRef
 * @see https://react.dev/reference/react/useCallback
 * @see https://react.dev/reference/react/useMemo
 */
import { useState, useRef, useCallback, useMemo, type MouseEvent } from 'react';

/**
 * Lucide React icons used for content-type badges and UI controls.
 *
 * Content-type icon mapping:
 *  - `Music`      -> song         (@see https://lucide.dev/icons/music)
 *  - `Disc3`      -> album        (@see https://lucide.dev/icons/disc-3)
 *  - `ListMusic`  -> playlist     (@see https://lucide.dev/icons/list-music)
 *  - `Video`      -> music-video  (@see https://lucide.dev/icons/video)
 *  - `User`       -> artist       (@see https://lucide.dev/icons/user)
 *  - `Library`    -> library      (@see https://lucide.dev/icons/library)
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
  Library,
  HelpCircle,
  ChevronDown,
  ChevronUp,
  Plus,
  Layers,
  FileDown,
  FolderSearch,
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
import { Button, Select, ContextMenu } from '@/components/common';
import type { AfterQueueAction } from '@/types';

/** Tauri IPC commands for pre-download checks (internet, cookies). */
import {
  checkCookiesBeforeDownload,
  checkInternetBeforeDownload,
  checkOutputPathBeforeDownload,
  checkRedownloadStatus,
  importManifest,
} from '@/lib/tauri-commands';

/** Apple Music URL parser for multi-URL validation. */
import { parseAppleMusicUrl } from '@/lib/url-parser';

/**
 * Type imports for Apple Music content types and audio codecs.
 * @see AppleMusicContentType in @/types/index.ts -- 'song' | 'album' | ... | 'unknown'
 * @see SongCodec in @/types/index.ts             -- 'alac' | 'atmos' | 'ac3' | ...
 */
import type { AppleMusicContentType, SongCodec, VideoResolution } from '@/types';

/**
 * Human-readable label maps used to populate quality-override dropdowns.
 * @see SONG_CODEC_LABELS in @/types/index.ts        -- e.g., { alac: 'Lossless (ALAC)' }
 * @see VIDEO_RESOLUTION_LABELS in @/types/index.ts   -- e.g., { '2160p': '4K UHD (2160p)' }
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
  song: Music, // Single song URL
  album: Disc3, // Album URL
  playlist: ListMusic, // Playlist URL
  'music-video': Video, // Music video URL
  artist: User, // Artist page URL
  library: Library, // Personal library URL
  unknown: HelpCircle, // Unrecognised or unparseable URL
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
  library: 'Library',
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
   * Submits multiple URLs as individual downloads to the Rust backend.
   * Used when the user pastes multiple Apple Music URLs (one per line).
   * @see downloadStore.submitBatchDownload in @/stores/downloadStore.ts
   */
  const submitBatchDownload = useDownloadStore((s) => s.submitBatchDownload);
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
  const smartRedownloadDetection = useSettingsStore((s) => s.settings.smart_redownload_detection);
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

  /**
   * Cookie validation error message shown when cookies are expired or
   * missing. Set by `handleSubmit()` before attempting to queue a download.
   * Cleared on the next successful check or when the URL input changes.
   */
  const [cookieError, setCookieError] = useState<string | null>(null);

  /** Whether preflight checks are running (disables submit button). */
  const [isChecking, setIsChecking] = useState(false);

  /** Navigation to the Settings page (for the "Go to Settings" link). */
  const setPage = useUiStore((s) => s.setPage);

  /**
   * Ref for the textarea element, used for auto-resize.
   * The textarea dynamically adjusts its height to fit content.
   */
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  /**
   * Auto-resize the textarea to fit its content.
   * Resets height to 'auto' first to correctly measure scrollHeight,
   * then sets height to the actual content height, clamped to a maximum
   * of 200px to avoid the textarea consuming too much vertical space.
   */
  const autoResizeTextarea = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      const maxHeight = 200;
      const newHeight = Math.min(textarea.scrollHeight, maxHeight);
      textarea.style.height = `${newHeight}px`;
      // Switch to scrollable overflow when content exceeds max height
      textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden';
    }
  }, []);

  /**
   * Parse the URL input into individual lines, filtering out empty lines.
   * Memoised to avoid re-computing on every render when urlInput hasn't changed.
   * Returns an array of trimmed, non-empty lines from the input.
   */
  const parsedLines = useMemo(() => {
    return urlInput
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
  }, [urlInput]);

  /**
   * Detect whether the input contains multiple URLs (multi-line mode).
   * A single line is handled by the existing single-URL flow.
   */
  const isMultiUrl = parsedLines.length > 1;

  /**
   * For multi-URL mode: validate each line individually and collect
   * the valid URLs, invalid count, and content type summary.
   * Memoised to avoid re-parsing on every render.
   */
  const multiUrlInfo = useMemo(() => {
    if (!isMultiUrl) return null;

    const validUrls: string[] = [];
    let invalidCount = 0;

    for (const line of parsedLines) {
      const parsed = parseAppleMusicUrl(line);
      if (parsed.isValid) {
        validUrls.push(parsed.url);
      } else {
        invalidCount++;
      }
    }

    return { validUrls, invalidCount, totalLines: parsedLines.length };
  }, [isMultiUrl, parsedLines]);

  /**
   * Determine whether the submit button should be enabled.
   * - Single URL mode: uses the store's `urlIsValid` flag.
   * - Multi-URL mode: requires at least one valid URL in the batch.
   */
  const canSubmit = isMultiUrl
    ? (multiUrlInfo?.validUrls.length ?? 0) > 0
    : urlIsValid;

  // ---------------------------------------------------------------
  // Event handlers
  // ---------------------------------------------------------------

  /**
   * Handles the "Add to Queue" action.
   *
   * Before queuing, runs two pre-download checks via the Rust backend:
   *  1. Internet connectivity — if offline, the download is still queued
   *     but auto-start is skipped so it waits until connectivity returns.
   *  2. Cookie validity — blocks if cookies are expired, missing, or invalid
   *     (only checked when online, since cookies are moot without internet).
   *
   * Calls `submitDownload()` which:
   *  1. Validates the URL (throws if invalid).
   *  2. Calls the Rust `startDownload` command via Tauri IPC.
   *  3. Clears the input and resets overrides on success.
   *
   * Shows a success toast on success, a warning toast when offline, or an
   * error toast on failure.
   */
  const handleSubmit = async () => {
    setIsChecking(true);
    try {
      await runPreflightAndSubmit();
    } finally {
      setIsChecking(false);
    }
  };

  /** Internal: run preflight checks then submit. Separated so setIsChecking wraps the full flow. */
  const runPreflightAndSubmit = async () => {
    // Check internet connectivity — if offline, we still queue the download
    // but skip auto-start so it waits until the user retries or connectivity
    // returns and a future download triggers queue processing.
    let isOffline = false;
    try {
      const internetCheck = await checkInternetBeforeDownload();
      if (!internetCheck.ready) {
        isOffline = true;
      }
    } catch {
      // If the check itself fails (e.g., IPC error), assume online and proceed.
    }

    // Check output path writability — catches disconnected cloud mounts,
    // full disks, and permission issues before queuing. This is a blocking
    // check: there's no point queuing a download that will definitely fail
    // because the output directory is unreachable.
    try {
      const outputCheck = await checkOutputPathBeforeDownload();
      if (!outputCheck.ready) {
        addToast(outputCheck.message ?? 'Output directory is not writable', 'error');
        return;
      }
    } catch {
      // If the check fails (e.g., IPC error), proceed —
      // the queue pre-flight will catch it later.
    }

    // Check cookie readiness before queuing (catches mid-session expiry).
    // Skipped when offline — cookies are moot without internet, and the
    // download won't start until connectivity returns anyway.
    if (!isOffline) {
      try {
        const cookieCheck = await checkCookiesBeforeDownload();
        if (!cookieCheck.ready) {
          setCookieError(cookieCheck.message ?? 'Cookies are not valid for downloading.');
          return;
        }
        setCookieError(null);
      } catch {
        // If the check itself fails (e.g., IPC error), proceed anyway —
        // the download will fail later with a more specific error.
        setCookieError(null);
      }
    }

    // Smart re-download detection (#263): check if this URL was previously
    // downloaded and inform the user. Non-blocking — download proceeds regardless.
    if (smartRedownloadDetection && !isMultiUrl && urlInput.trim()) {
      try {
        const redownloadInfo = await checkRedownloadStatus(urlInput.trim());
        if (redownloadInfo) {
          const downloadDate = new Date(redownloadInfo.downloaded_at).toLocaleDateString();
          const label = redownloadInfo.title ?? redownloadInfo.album ?? 'This album';
          addToast(
            `${label} was previously downloaded on ${downloadDate}. Downloading again.`,
            'info'
          );
        }
      } catch {
        // Non-critical — proceed with download if check fails
      }
    }

    // Branch: multi-URL batch submission vs single-URL submission
    if (isMultiUrl && multiUrlInfo && multiUrlInfo.validUrls.length > 0) {
      // --- Multi-URL batch mode ---
      try {
        const result = await submitBatchDownload(multiUrlInfo.validUrls, isOffline);

        // Show duplicate warnings (batched into a single toast if multiple)
        if (result.duplicateWarnings.length > 0) {
          addToast(
            result.duplicateWarnings.length === 1
              ? result.duplicateWarnings[0]
              : `${result.duplicateWarnings.length} duplicate URLs detected in queue`,
            'warning'
          );
        }

        // Build summary toast message
        const parts: string[] = [];
        if (result.queued > 0) {
          parts.push(`${result.queued} download${result.queued !== 1 ? 's' : ''} added to queue`);
        }
        if (result.failed > 0) {
          parts.push(`${result.failed} failed to queue`);
        }
        if (multiUrlInfo.invalidCount > 0) {
          parts.push(`${multiUrlInfo.invalidCount} invalid URL${multiUrlInfo.invalidCount !== 1 ? 's' : ''} skipped`);
        }

        const summaryMsg = parts.join(', ');

        if (isOffline) {
          addToast(`${summaryMsg} — will start when internet is available`, 'warning');
        } else if (result.failed > 0 || multiUrlInfo.invalidCount > 0) {
          addToast(summaryMsg, 'warning');
        } else {
          addToast(summaryMsg, 'success');
        }

        // Reset textarea height after clearing input
        if (textareaRef.current) {
          textareaRef.current.style.height = 'auto';
        }
      } catch {
        addToast('Failed to add downloads', 'error');
      }
    } else {
      // --- Single-URL mode (existing flow) ---
      try {
        const result = await submitDownload(isOffline);

        // Show duplicate URL warning if the backend detected a match.
        // This is non-blocking — the download is still queued.
        if (result.duplicate_warning) {
          addToast(result.duplicate_warning, 'warning');
        }

        if (isOffline) {
          addToast('Download queued — will start when internet is available', 'warning');
        } else {
          addToast('Download added to queue', 'success');
        }

        // Reset textarea height after clearing input
        if (textareaRef.current) {
          textareaRef.current.style.height = 'auto';
        }
      } catch {
        addToast('Failed to add download', 'error');
      }
    }
  };

  /**
   * Handles keyboard events in the URL textarea.
   *
   * - **Enter** (without Shift): submits the form if valid and not submitting.
   *   `preventDefault()` stops the newline from being inserted.
   * - **Shift+Enter**: inserts a newline (default textarea behaviour) for
   *   multi-URL entry. No special handling needed.
   *
   * @see https://react.dev/learn/responding-to-events
   */
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && canSubmit && !isSubmitting && !isChecking) {
      e.preventDefault();
      handleSubmit();
    }
  };

  /**
   * Handles the "Import Manifest" action.
   *
   * Opens a native file picker filtered to `.meedyadl` files, parses
   * the manifest, and populates the URL textarea with the source URLs
   * (one per line for batch queueing).
   */
  const handleImportManifest = async () => {
    try {
      const urls = await importManifest();
      if (urls.length > 0) {
        setUrlInput(urls.join('\n'));
        addToast(`Imported ${urls.length} URL${urls.length !== 1 ? 's' : ''} from manifest`, 'success');
      }
    } catch {
      // User cancelled the dialog — silently ignore
    }
  };

  /**
   * Scan a folder for manifest files and populate the URL input (#456).
   * Opens a native folder picker, recursively scans for manifest.meedyadl
   * files, and populates the textarea with discovered URLs.
   */
  const handleScanFolder = async () => {
    try {
      const { scanFolderForManifests } = await import('@/lib/tauri-commands');
      const manifests = await scanFolderForManifests();
      if (manifests.length === 0) {
        addToast('No manifest files found in the selected folder', 'info');
        return;
      }
      // Collect all unique URLs from all manifests
      const allUrls = [...new Set(manifests.flatMap((m) => m.urls))];
      setUrlInput(allUrls.join('\n'));
      const albumCount = manifests.length;
      const trackTotal = manifests.reduce((sum, m) => sum + m.track_count, 0);
      addToast(
        `Found ${albumCount} album${albumCount !== 1 ? 's' : ''} (${trackTotal} tracks, ${allUrls.length} URLs) — review and submit to re-download`,
        'success',
      );
    } catch {
      // User cancelled the dialog — silently ignore
    }
  };

  /**
   * Import URLs from a .txt file (one URL per line).
   * Lines starting with # are treated as comments and skipped.
   * Each valid URL is added to the input textarea.
   */
  const handleImportTxtFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const { readTextFile } = await import('@tauri-apps/plugin-fs');
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Text Files', extensions: ['txt'] }],
      });
      if (!selected) return; // User cancelled
      const filePath = String(selected);
      const content = await readTextFile(filePath);
      const lines = content.split('\n')
        .map((l: string) => l.trim())
        .filter((l: string) => l.length > 0 && !l.startsWith('#'));
      if (lines.length > 0) {
        setUrlInput(lines.join('\n'));
        addToast(`Loaded ${lines.length} URL${lines.length !== 1 ? 's' : ''} from file`, 'success');
      } else {
        addToast('No URLs found in file', 'warning');
      }
    } catch {
      // User cancelled the dialog — silently ignore
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
   * (e.g., '2160p' -> '4K UHD (2160p)').
   */
  const resolutionOptions = Object.entries(VIDEO_RESOLUTION_LABELS).map(([value, label]) => ({
    value,
    label,
  }));

  // ---------------------------------------------------------------
  // Context Menu: After-Queue Actions (one-shot)
  // ---------------------------------------------------------------
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback((e: MouseEvent) => {
    // Allow the native context menu (paste/cut/copy) on form elements —
    // only show the After-Queue menu on blank space.
    const target = e.target as HTMLElement;
    const tag = target.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    // Also allow native menu on contentEditable elements
    if (target.isContentEditable) return;
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const setAfterQueueOnce = useCallback((action: AfterQueueAction) => {
    const { updateSettings } = useSettingsStore.getState();
    updateSettings({ after_queue_once: action === 'do_nothing' ? null : action });
    const label = action === 'do_nothing' ? 'cleared' : action.replace(/_/g, ' ');
    useUiStore.getState().addToast(`After queue (once): ${label}`, 'info');
    setContextMenu(null);
  }, []);

  const afterQueueMenuItems = [
    { label: 'After Queue: Do nothing', onClick: () => setAfterQueueOnce('do_nothing') },
    { label: 'After Queue: Open output folder', onClick: () => setAfterQueueOnce('open_output_folder') },
    { label: 'After Queue: Play sound', onClick: () => setAfterQueueOnce('play_sound') },
    { label: 'After Queue: Close MeedyaDL', onClick: () => setAfterQueueOnce('close_meedyadl'), separator: true },
    { label: 'After Queue: Restart computer', onClick: () => setAfterQueueOnce('restart_computer'), separator: true },
    { label: 'After Queue: Hibernate / Sleep', onClick: () => setAfterQueueOnce('hibernate_computer') },
    { label: 'After Queue: Shut down', onClick: () => setAfterQueueOnce('shutdown_computer') },
  ];

  // ---------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------
  return (
    <div onContextMenu={handleContextMenu}>
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
       * Form body -- fills available width responsively.
       * `space-y-6` (24px) separates the URL input section from the
       * quality overrides section.
       */}
      <div className="p-6 space-y-6">
        {/* =========================================================
         * Section 1: URL Input
         * =========================================================
         * Contains the text input, content-type badge, "Add to Queue"
         * button, and validation feedback text.
         */}
        <div className="space-y-2">
          {/* Accessible <label> linked to the input via `htmlFor` */}
          <label htmlFor="url-input" className="block text-sm font-medium text-content-primary">
            Apple Music URL
          </label>

          {/* Input row: textarea + submit button side-by-side.
              `items-start` keeps the button top-aligned when textarea grows. */}
          <div className="flex gap-2 items-start">
            {/*
             * URL text input with inline content-type badge.
             * `relative` positioning on the wrapper allows the badge
             * to be absolutely positioned inside the input.
             */}
            <div className="flex-1 relative">
              {/*
               * Controlled textarea bound to `downloadStore.urlInput`.
               *
               * Supports both single-URL and multi-URL (one per line) input.
               * Auto-resizes vertically to fit content, capped at 200px.
               *
               * onChange: calls `setUrlInput()` which validates the URL
               *   in real-time via `parseAppleMusicUrl()` from
               *   `@/lib/url-parser.ts`. Also triggers auto-resize.
               * onKeyDown: Enter submits (Shift+Enter inserts newline).
               *
               * Border colour changes based on validation state:
               *  - Red (`border-status-error`) when input is non-empty
               *    but neither single-URL nor multi-URL is valid.
               *  - Default (`border-border`) otherwise, with accent
               *    colour on focus (`focus:border-accent`).
               *
               * @see https://react.dev/reference/react-dom/components/textarea
               */}
              <textarea
                ref={textareaRef}
                id="url-input"
                value={urlInput}
                onChange={(e) => {
                  setUrlInput(e.target.value);
                  // Clear cookie error when URL changes (user is retrying)
                  if (cookieError) setCookieError(null);
                  // Auto-resize textarea to fit content after React updates the value
                  requestAnimationFrame(autoResizeTextarea);
                }}
                onKeyDown={handleKeyDown}
                placeholder="Paste one or more Apple Music URLs (one per line)"
                rows={1}
                className={`
                  w-full px-3 py-2 text-sm rounded-platform border
                  bg-surface-secondary text-content-primary
                  placeholder-content-tertiary
                  transition-colors resize-none overflow-hidden
                  focus:ring-1 focus:ring-accent
                  ${urlInput && !canSubmit ? 'border-status-error focus:border-status-error' : 'border-border focus:border-accent'}
                `}
                style={{ minHeight: '38px' }}
              />

              {/*
               * Content-type badge -- absolutely positioned inside the
               * textarea (right-aligned, top-aligned for multi-line).
               *
               * Single-URL mode: shows the detected content type icon +
               * label (e.g., "Album") when `urlIsValid` is true.
               *
               * Multi-URL mode: shows a count badge (e.g., "3 URLs")
               * with the valid/invalid breakdown.
               *
               * `rounded-full` makes it pill-shaped.
               * `bg-accent-light text-accent` uses the accent colour
               * at a light tint for the background.
               */}
              {isMultiUrl && multiUrlInfo && multiUrlInfo.validUrls.length > 0 && (
                <div className="absolute right-2 top-2 flex items-center gap-1 px-2 py-0.5 rounded-full bg-accent-light text-accent text-xs font-medium">
                  <Layers size={12} />
                  {multiUrlInfo.validUrls.length} URL{multiUrlInfo.validUrls.length !== 1 ? 's' : ''}
                  {multiUrlInfo.invalidCount > 0 && (
                    <span className="text-status-warning ml-1">
                      ({multiUrlInfo.invalidCount} invalid)
                    </span>
                  )}
                </div>
              )}
              {!isMultiUrl && urlIsValid && (
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
              loading={isSubmitting || isChecking}
              disabled={!canSubmit || isChecking}
              onClick={handleSubmit}
            >
              Add to Queue
            </Button>
            <Button
              variant="secondary"
              icon={<FileDown size={16} />}
              onClick={handleImportManifest}
              title="Import a .meedyadl manifest file"
            >
              Import
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={handleImportTxtFile}
              title="Import URLs from a .txt file (one URL per line)"
            >
              .txt
            </Button>
            <Button
              variant="secondary"
              icon={<FolderSearch size={16} />}
              onClick={handleScanFolder}
              title="Scan a folder for manifest files to re-download"
            >
              Scan
            </Button>
          </div>

          {/*
           * Validation feedback messages (mutually exclusive):
           *  - Red error text when the input has text but URL is invalid.
           *  - Grey helper text when the input is empty (shows supported types).
           */}
          {urlInput && !canSubmit && !isMultiUrl && (
            <p className="text-xs text-status-error">Please enter a valid Apple Music URL</p>
          )}
          {urlInput && !canSubmit && isMultiUrl && (
            <p className="text-xs text-status-error">No valid Apple Music URLs found</p>
          )}
          {!urlInput && (
            <p className="text-xs text-content-tertiary">
              Supports songs, albums, playlists, music videos, and artist pages. Paste multiple URLs (one per line) to queue them all.
            </p>
          )}

          {/* Cookie validation warning -- shown when cookies are expired,
              missing, or invalid. Blocks downloads until cookies are fixed. */}
          {cookieError && (
            <div className="flex items-start gap-2 p-3 rounded-platform bg-status-warning/10 border border-status-warning/30">
              <span className="text-status-warning text-sm mt-0.5">&#9888;</span>
              <div className="flex-1 text-xs text-content-primary">
                <p>{cookieError}</p>
                <button
                  type="button"
                  className="mt-1 text-accent hover:text-accent-hover underline transition-colors"
                  onClick={() => setPage('settings')}
                >
                  Go to Settings
                </button>
              </div>
            </div>
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
               * Populated from `VIDEO_RESOLUTION_LABELS` (e.g., '2160p' -> '4K UHD (2160p)').
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
      </div>

      {/* After-queue one-shot context menu (right-click) */}
      {contextMenu && (
        <ContextMenu
          items={afterQueueMenuItems}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
