// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file Status bar component.
 *
 * Renders a thin bar at the bottom of the main content area showing
 * current application state: active downloads, queued count, completed
 * count, and the application version string.
 *
 * The status bar is always visible (pinned below the scrollable `<main>`
 * inside {@link MainLayout}) and updates reactively whenever the download
 * queue changes in the Zustand store.
 *
 * State connection:
 *  - {@link useDownloadStore} -- reads `queueItems[]` to derive counts
 *    by filtering on `QueueItemStatus.state`.
 *
 * The left section displays download activity counters:
 *  - "X downloading" (with animated pulse dot) -- items in 'downloading'
 *    or 'processing' state.
 *  - "X queued" -- items waiting to start.
 *  - "X completed" -- successfully finished items.
 *  - "No downloads" -- shown when the queue is completely empty.
 *
 * The right section displays the application version string.
 *
 * @see https://tailwindcss.com/docs/animation#pulse -- animate-pulse used for the activity dot.
 * @see https://react.dev/learn/rendering-lists       -- conditional rendering of count spans.
 */

import { useCallback } from 'react';

// The app's version number, never a placeholder (see the hook's notes).
import { useAppVersion } from '@/hooks/useAppVersion';

import { Square } from 'lucide-react';
import { Button } from '@/components/common';

// This component is visible on every screen of the app (it's pinned under
// every page), so every word in it needs to go through i18next rather than
// being typed once in English.
import { useTranslation } from 'react-i18next';

/**
 * Zustand store hook for the download queue.
 * Provides `queueItems` -- an array of `QueueItemStatus` objects whose
 * `.state` field is one of: 'queued' | 'downloading' | 'processing' |
 * 'complete' | 'error' | 'cancelled'.
 * @see useDownloadStore in @/stores/downloadStore.ts
 * @see QueueItemStatus in @/types/index.ts
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { ABORT_FAILED, showError } from '@/lib/errorMessages';

/**
 * Translation keys (under `statusBar.actions.*`) for each after-queue
 * action's human-readable label. Keyed by the same strings the settings
 * store uses (`AfterQueueAction`), so this is a lookup table, not a
 * duplicate of the wording itself -- the wording lives in the locale
 * files, same as everywhere else.
 */
const AFTER_QUEUE_LABEL_KEYS: Record<string, string> = {
  do_nothing: '',
  open_output_folder: 'statusBar.actions.openOutputFolder',
  play_sound: 'statusBar.actions.playSound',
  close_meedyadl: 'statusBar.actions.closeApp',
  restart_computer: 'statusBar.actions.restart',
  hibernate_computer: 'statusBar.actions.hibernate',
  shutdown_computer: 'statusBar.actions.shutdown',
};

/**
 * Compact indicator shown in the status bar when an after-queue action is set.
 * Shows "(once)" suffix for one-shot actions vs the persistent action.
 *
 * **Always reads `savedSettings`, never `settings` (#1222).** `settings`
 * is whatever the Settings screen currently shows, which can hold an
 * edit nobody has pressed "Save" on yet. This bar exists to tell someone
 * what is actually going to happen when the queue finishes -- and what
 * is actually going to happen is decided by the file on disk, which is
 * what the download queue itself reads, not by whatever is sitting
 * half-edited on a screen the person may not even have open. Before this
 * fix, changing the dropdown and walking away made the bar show nothing
 * was going to happen while the file still said "shut down" -- wrong in
 * exactly the dangerous direction, a screen that goes quiet right before
 * the computer turns itself off.
 *
 * When the Settings screen DOES hold an unsaved edit to the standing
 * action, a short "(unsaved change)" note is appended, so the person can
 * tell the two apart without it ever changing WHAT is shown.
 */
function AfterQueueIndicator() {
  const { t } = useTranslation();

  // What is actually on disk -- what the queue will act on. This is
  // deliberately `savedSettings`, not `settings`; see the comment above.
  const savedOnce = useSettingsStore((s) => s.savedSettings.after_queue_once);
  const savedAlways = useSettingsStore((s) => s.savedSettings.after_queue_action);

  // The Settings screen's own in-memory copy of the standing action.
  // Read ONLY to decide whether to show the "(unsaved change)" note --
  // never to decide what the bar actually says is going to happen.
  const editedAlways = useSettingsStore((s) => s.settings.after_queue_action);

  // One-shot overrides persistent; show nothing for do_nothing
  const action = savedOnce ?? savedAlways ?? 'do_nothing';
  if (action === 'do_nothing') return null;

  const labelKey = AFTER_QUEUE_LABEL_KEYS[action];
  const label = labelKey ? t(labelKey) : action;
  const text = savedOnce
    ? t('statusBar.afterQueueOnce', { label })
    : t('statusBar.afterQueue', { label });

  // True when the Settings screen has an edit to the standing action
  // that has not been saved. Checked even while a one-off is what is
  // actually being shown above: the moment that one-off runs and clears
  // itself, this is the value that takes over, and the person should
  // know it does not match what they last typed.
  const hasUnsavedStandingEdit = editedAlways !== savedAlways;
  const fullText = hasUnsavedStandingEdit
    ? `${text} ${t('statusBar.unsavedChangeNote')}`
    : text;

  return (
    <span className="text-status-warning-text" title={fullText}>
      {fullText}
    </span>
  );
}

/**
 * Renders a status bar at the bottom of the main content area.
 *
 * Derives three counters from the download store's `queueItems` array
 * by filtering on the `state` field of each {@link QueueItemStatus}:
 *  - **activeCount**: items whose state is `'downloading'` or `'processing'`.
 *  - **queuedCount**: items whose state is `'queued'`.
 *  - **completedCount**: items whose state is `'complete'`.
 *
 * These filters run on every render triggered by a `queueItems` change.
 * Because the queue is typically small (< 100 items), the O(n) filter
 * cost is negligible and memoisation is not needed.
 *
 * @returns A thin horizontal bar with activity summary (left) and version (right).
 */
export function StatusBar() {
  /** i18n translation function for every piece of text in this bar. */
  const { t } = useTranslation();

  /**
   * Application version string: the build's own version from the first
   * frame, then the desktop app's answer (see `useAppVersion`). It used to
   * show "v..." while loading and "vunknown" if the lookup failed.
   */
  const appVersion = useAppVersion();

  /**
   * Subscribe to the `queueItems` slice of the download store.
   * This component re-renders whenever the array reference changes
   * (e.g., items are added, removed, or their state is updated).
   */
  const queueItems = useDownloadStore((s) => s.queueItems);

  /*
   * Derive display counters by filtering the queue array.
   * Each `.filter()` call iterates the full array, but with typical
   * queue sizes this is efficient enough without memoisation.
   */

  /**
   * Split downloading vs processing (#817). Pre-fix, both were
   * lumped into a single "N downloading" count, which violated the
   * serial-queue invariant on screen — users would see "2 downloading"
   * when one item was actively downloading via GAMDL and another was
   * stuck in post-processing (e.g. the #815 silent-hang). Showing
   * them separately gives a truthful signal: "1 downloading · 1
   * processing" makes the stuck-post-processing state visible
   * without misleading users that two GAMDL subprocesses are racing.
   */
  const downloadingCount = queueItems.filter((i) => i.state === 'downloading').length;
  const processingCount = queueItems.filter((i) => i.state === 'processing').length;
  /** Combined for backwards-compat with downstream conditions that
   * just need to know "is anything active". */
  const activeCount = downloadingCount + processingCount;

  /** Number of items waiting in the queue that have not yet started. */
  const queuedCount = queueItems.filter((i) => i.state === 'queued').length;

  /** Number of items that have finished successfully. */
  const completedCount = queueItems.filter((i) => i.state === 'complete').length;

  /**
   * Fires the abort-all action (#620). Honours the
   * `abort_queue_confirm` setting: when enabled, uses a native
   * `window.confirm()` rather than the queue-page modal — duplicating
   * the modal here would require lifting state into a shared context,
   * and the StatusBar affordance is the "quick escape" path where a
   * lightweight confirmation is appropriate. When disabled, fires
   * immediately.
   */
  const abortAll = useDownloadStore((s) => s.abortAll);
  const abortQueueConfirm = useSettingsStore(
    (s) => s.settings.abort_queue_confirm,
  );
  const triggerAbort = useCallback(() => {
    if (abortQueueConfirm) {
      // `window.confirm` is a blocking native modal — acceptable here
      // because the action is destructive and the StatusBar doesn't
      // own the shared Modal component the Queue page uses. The Queue
      // page's richer confirmation (with "Don't ask again") remains
      // the canonical flow.
      const confirmed = window.confirm(t('statusBar.abortConfirm'));
      if (!confirmed) return;
    }
    // abortAll reports its own failures (downloadStore); this catch is for
    // anything that escapes it, worded the same way.
    void abortAll().catch((e) => showError(ABORT_FAILED, e, { key: 'abort-failed' }));
  }, [abortAll, abortQueueConfirm, t]);

  /**
   * Plain-English summary of the counters below, read aloud by a screen
   * reader through the dedicated live region rendered just below (a11y
   * review fix).
   *
   * This bar used to carry `role="status"` and `aria-live="polite"` on
   * its OUTER container -- the same element holding the counters, the
   * "Abort Queue" button, the after-queue indicator, AND the version
   * string. Because the counters change every time a download moves
   * between states, a screen reader would re-read the ENTIRE bar each
   * time -- including "Abort" and "MeedyaDL v1.13.0", neither of which
   * had anything to do with what actually changed. Worse, a button
   * sitting inside a live region can be announced to someone who never
   * moved their focus anywhere near it, which is confusing on its own.
   *
   * The project already solved this exact problem on the Queue page
   * (`DownloadQueue.tsx`'s single shared live region, and the comment on
   * `StatusPill.tsx` recording why each row's own `role="status"` was
   * removed for the same reason). The fix here follows the same shape:
   * the visible bar is now an ordinary, non-live container, and this
   * string -- built from the exact same phrases already shown on
   * screen, just without the button or the version -- is the only thing
   * inside the live region below. Because the string is derived by
   * filtering the same counts the visible spans already use, it only
   * actually changes value when a count changes, so a screen reader
   * naturally never repeats itself just because the component
   * re-rendered.
   */
  const statusSummaryParts: string[] = [];
  if (downloadingCount > 0) {
    statusSummaryParts.push(t('statusBar.downloading', { count: downloadingCount }));
  }
  if (processingCount > 0) {
    statusSummaryParts.push(t('statusBar.processing', { count: processingCount }));
  }
  if (queuedCount > 0) {
    statusSummaryParts.push(t('statusBar.queued', { count: queuedCount }));
  }
  if (completedCount > 0) {
    statusSummaryParts.push(t('statusBar.completed', { count: completedCount }));
  }
  if (queueItems.length === 0) {
    statusSummaryParts.push(t('statusBar.noDownloads'));
  }
  const statusSummary = statusSummaryParts.join(', ');

  return (
    /**
     * Status bar container.
     *
     * `px-4 py-1.5` -- compact padding (16px horizontal, 6px vertical).
     * `bg-surface-secondary` -- slightly elevated background colour.
     * `border-t border-border-light` -- thin top border separating it
     * from the scrollable content above.
     * `text-[11px]` -- 11px font size (below Tailwind's smallest preset)
     * for an unobtrusive footer feel.
     * `text-content-tertiary` -- muted text colour from the design tokens.
     *
     * @see https://tailwindcss.com/docs/font-size  -- arbitrary font size
     *
     * `role="group"` + `aria-label` (rather than `role="status"`)
     * because this container is no longer a live region -- see the
     * long comment on `statusSummary` above for why. `role="group"`
     * keeps the bar as one named, browsable landmark for assistive
     * tech without making every change inside it an announcement.
     */
    <div
      role="group"
      aria-label={t('statusBar.ariaLabel')}
      className="flex items-center justify-between px-4 py-1.5 bg-surface-secondary border-t border-border-light text-[11px] text-content-tertiary"
    >
      {/*
       * Screen-reader-only live region carrying ONLY the plain-English
       * counter summary computed above -- not the Abort button, not the
       * after-queue indicator, not the version string. This is the
       * piece that actually needs to be spoken when it changes; nothing
       * else in this bar does.
       */}
      <div role="status" aria-live="polite" className="sr-only">
        {statusSummary}
      </div>

      {/*
       * Left section: download activity summary.
       * Conditionally renders one or more count spans depending on
       * which states have items. If the queue is empty, a "No downloads"
       * placeholder is shown instead.
       *
       * `data-testid` exists only so tests can tell this VISIBLE copy of
       * the counters apart from the hidden live-region copy above --
       * both legitimately contain the same words ("2 downloading" reads
       * the same whichever one a screen reader or a test happens to
       * find), so a plain text search would otherwise match either one.
       */}
      <div className="flex items-center gap-3" data-testid="status-bar-visible-counters">
        {/*
         * Active downloads indicator.
         * The small dot (`w-1.5 h-1.5 rounded-full`) uses `bg-status-info`
         * (blue) and `animate-pulse` (Tailwind's built-in pulsing animation)
         * to draw attention to ongoing activity.
         * @see https://tailwindcss.com/docs/animation#pulse
         */}
        {downloadingCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-status-info animate-pulse" />
            {t('statusBar.downloading', { count: downloadingCount })}
          </span>
        )}
        {/* Processing count (#817) — items past GAMDL exit and in
         * the post-companion / enrichment / final-tag stages. Shown
         * separately from `downloading` so the serial-queue
         * invariant ("only 1 GAMDL subprocess active at a time") is
         * visible without lumping stuck post-processing into the
         * download count. Uses an hourglass-style amber dot to
         * distinguish at a glance. */}
        {processingCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-status-warning" />
            {t('statusBar.processing', { count: processingCount })}
          </span>
        )}
        {/* Queued count -- items waiting to start */}
        {queuedCount > 0 && <span>{t('statusBar.queued', { count: queuedCount })}</span>}
        {/* Completed count -- successfully finished items */}
        {completedCount > 0 && <span>{t('statusBar.completed', { count: completedCount })}</span>}
        {/* Empty-queue fallback message */}
        {queueItems.length === 0 && <span>{t('statusBar.noDownloads')}</span>}

        {/*
          Global "Abort Queue" affordance (#620). Always available — even
          when the user is on Settings / History pages and can't reach the
          queue-page button. Fires the same abort path (confirmation
          respects `abort_queue_confirm`).

          It used to sit in the MIDDLE of the counts ("1 queued [Abort] 1
          completed"), where it read as one more count. It now comes after
          all of them, set apart by a thin divider, and is the shared
          Button (size xs) rather than a hand-made one.

          The "Cmd/Ctrl+Shift+." keyboard hint is deliberately kept out of
          the translated sentence and passed in as `{{shortcut}}` — key
          names like Cmd and Ctrl are never translated, and pulling it out
          means a translator can't accidentally reword it.
        */}
        {(activeCount > 0 || queuedCount > 0) && (
          <span className="flex items-center border-l border-border-light pl-3">
            <Button
              variant="ghost"
              size="xs"
              icon={<Square size={12} />}
              onClick={triggerAbort}
              aria-label={t('statusBar.abortAriaLabel')}
              title={t('statusBar.abortTitle', { shortcut: 'Cmd/Ctrl+Shift+.' })}
              className="text-status-error-text hover:bg-status-error/10"
            >
              {t('statusBar.abort')}
            </Button>
          </span>
        )}
      </div>

      {/* Centre: after-queue action indicator (if non-default) */}
      <AfterQueueIndicator />

      {/* Right section: application version string (fetched from
          tauri.conf.json). "MeedyaDL v1.2.3" has no actual English words in
          it to translate -- just the product name (never translated) and a
          version-prefix convention ("v" + number) that reads the same in
          every language, so this is left as a plain string rather than
          wired through i18next. */}
      <span>MeedyaDL{appVersion ? ` v${appVersion}` : ''}</span>
    </div>
  );
}
