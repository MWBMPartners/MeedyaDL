/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/layout/StatusBar.test.tsx
 *
 * Unit tests for the StatusBar component. The StatusBar subscribes to the
 * download store's `queueItems` array and derives display counters from
 * the items' states (downloading, queued, completed). It also shows the
 * app version string.
 *
 * These tests manipulate the Zustand download store directly via
 * `useDownloadStore.setState()` to simulate various queue configurations.
 *
 * @see src/components/layout/StatusBar.tsx - The component under test
 * @see src/stores/downloadStore.ts - Source of queue data
 */

import { render, screen, within } from '@testing-library/react';

import { StatusBar } from '@/components/layout/StatusBar';
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';

import type { QueueItemStatus } from '@/types';

/**
 * Factory helper to create a mock queue item with a given state.
 * Provides reasonable defaults for all required fields so tests
 * can focus on the state field that drives StatusBar rendering.
 */
function createItem(state: QueueItemStatus['state'], id?: string): QueueItemStatus {
  return {
    id: id || `item-${Math.random().toString(36).slice(2)}`,
    urls: ['https://music.apple.com/us/album/test/123'],
    state,
    progress: state === 'complete' ? 100 : 0,
    current_track: null,
    total_tracks: null,
    completed_tracks: null,
    speed: null,
    eta: null,
    processing_label: null,
  processing_progress: null,
    error: state === 'error' ? 'Test error' : null,
    output_path: state === 'complete' ? '/tmp/output' : null,
    codec_used: 'alac',
    fallback_occurred: false,
    used_wrapper: false,
    output_is_directory: false,
    warnings: [],
    audio_traits: [],
    mv_companion_count: null,
    created_at: new Date().toISOString(),
  };
}

/** Reset the download store to a clean state before each test. */
beforeEach(() => {
  useDownloadStore.setState({
    queueItems: [],
  });
});

/**
 * Scopes a query to the VISIBLE counters (not the hidden live-region
 * copy added by the a11y fix below).
 *
 * The fix adds a second, screen-reader-only element that legitimately
 * contains the exact same words as the visible counters (see the long
 * comment on `statusSummary` in `StatusBar.tsx`) -- a screen reader
 * needs to hear "2 downloading" exactly once when it changes, in
 * plain language, which means reusing the same phrase rather than
 * inventing a second wording just to keep it unique. That is correct
 * for a real screen reader, but it means a plain `screen.getByText(...)`
 * in a test can no longer tell the two copies apart and throws
 * "multiple elements found". Scoping to the visible counters' own
 * `data-testid` (added purely for this reason -- see the comment next
 * to it in `StatusBar.tsx`) keeps these tests checking the same thing
 * they always checked: what's actually painted on screen.
 */
function visibleCounters() {
  return within(screen.getByTestId('status-bar-visible-counters'));
}

describe('StatusBar', () => {
  // =========================================================================
  // Empty queue
  // =========================================================================

  /** When no items are in the queue, "No downloads" should be displayed. */
  it('shows "No downloads" when queue is empty', () => {
    render(<StatusBar />);

    expect(visibleCounters().getByText('No downloads')).toBeInTheDocument();
  });

  // =========================================================================
  // Active downloads
  // =========================================================================

  /** Items in 'downloading' state should be counted as active. */
  it('shows active download count for downloading items', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading'), createItem('downloading')],
    });

    render(<StatusBar />);

    expect(visibleCounters().getByText(/2 downloading/)).toBeInTheDocument();
  });

  /**
   * Per #817: downloading and processing are surfaced as TWO
   * distinct counters so the serial-queue invariant is visible.
   * The pre-#817 behaviour lumped them as "2 downloading" which
   * was misleading when an item was stuck in post-processing
   * (#815 surfaced exactly this confusion in the user's screenshot).
   */
  it('splits downloading and processing into distinct counters (#817)', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading'), createItem('processing')],
    });

    render(<StatusBar />);

    expect(visibleCounters().getByText(/1 downloading/)).toBeInTheDocument();
    expect(visibleCounters().getByText(/1 processing/)).toBeInTheDocument();
  });

  // =========================================================================
  // Queued count
  // =========================================================================

  /** Items waiting in the 'queued' state should be displayed. */
  it('shows queued count', () => {
    useDownloadStore.setState({
      queueItems: [createItem('queued'), createItem('queued'), createItem('queued')],
    });

    render(<StatusBar />);

    expect(visibleCounters().getByText(/3 queued/)).toBeInTheDocument();
  });

  // =========================================================================
  // Completed count
  // =========================================================================

  /** Successfully finished items should show the completed count. */
  it('shows completed count', () => {
    useDownloadStore.setState({
      queueItems: [createItem('complete')],
    });

    render(<StatusBar />);

    expect(visibleCounters().getByText(/1 completed/)).toBeInTheDocument();
  });

  // =========================================================================
  // Mixed states
  // =========================================================================

  /** When the queue has items in multiple states, all counters appear. */
  it('shows multiple counters for mixed queue states', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('downloading'),
        createItem('queued'),
        createItem('queued'),
        createItem('complete'),
        createItem('complete'),
        createItem('complete'),
      ],
    });

    render(<StatusBar />);

    expect(visibleCounters().getByText(/1 downloading/)).toBeInTheDocument();
    expect(visibleCounters().getByText(/2 queued/)).toBeInTheDocument();
    expect(visibleCounters().getByText(/3 completed/)).toBeInTheDocument();
  });

  // =========================================================================
  // Non-counted states
  // =========================================================================

  /**
   * Error and cancelled items are in the queue but not shown as specific
   * counters — only downloading, queued, and completed have dedicated displays.
   * The "No downloads" placeholder should NOT appear since items exist.
   */
  it('does not show "No downloads" when only error/cancelled items exist', () => {
    useDownloadStore.setState({
      queueItems: [createItem('error'), createItem('cancelled')],
    });

    render(<StatusBar />);

    /* Items exist, so "No downloads" should not appear.
     * But no specific counters for error/cancelled are shown either. */
    expect(screen.queryByText('No downloads')).not.toBeInTheDocument();
  });

  // =========================================================================
  // Version string
  // =========================================================================

  /** The app version should always be displayed on the right side. */
  it('shows the app version string', () => {
    render(<StatusBar />);

    expect(screen.getByText(/MeedyaDL v/)).toBeInTheDocument();
  });

  // =========================================================================
  // Live region scope (review fix -- "the whole status bar is announced
  // as it changes")
  // =========================================================================

  /**
   * Before the fix, `role="status"` and `aria-live="polite"` sat on the
   * OUTER bar -- the same element holding the counters, the "Abort
   * Queue" button, the after-queue indicator, and the version string.
   * Because the counters change every time a download moves between
   * states, a screen reader would re-read the ENTIRE bar each time,
   * including "Abort" and "MeedyaDL v1.13.0" -- neither of which had
   * anything to do with what actually changed, and a button living
   * inside a live region can be announced to someone who never moved
   * focus there at all.
   *
   * This block checks the fix from the outside: there is now exactly
   * one live region, it is a dedicated element separate from the
   * visible bar, and it carries only the counter summary -- not the
   * button's label, not the version string.
   */
  it('the outer bar is no longer its own live region', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading'), createItem('queued')],
    });

    const { container } = render(<StatusBar />);

    // The outer bar (the flex row holding everything) must not itself
    // carry aria-live -- it is now an ordinary, non-live container.
    const outerBar = container.firstElementChild as HTMLElement;
    expect(outerBar).not.toHaveAttribute('aria-live');
  });

  /**
   * There must be exactly one live region on the page (the dedicated
   * one), and it must contain only the plain-English counter summary --
   * neither the Abort button's own label nor the version string, which
   * a screen reader would previously have re-announced on every single
   * counter change purely because they happened to share the same
   * live-region ancestor.
   */
  it('the live region announces only the counters, not the Abort button or the version', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading'), createItem('queued'), createItem('queued')],
    });

    render(<StatusBar />);

    // Exactly one element on the page carries the "status" role now --
    // if the fix regressed and the outer bar also became a live region
    // again, this would find two and throw.
    const liveRegion = screen.getByRole('status');

    expect(liveRegion.textContent).toContain('1 downloading');
    expect(liveRegion.textContent).toContain('2 queued');
    // The Abort button's own text and the version string must NOT be
    // part of what gets read aloud when a counter changes.
    expect(liveRegion.textContent).not.toMatch(/abort/i);
    expect(liveRegion.textContent).not.toMatch(/MeedyaDL v/);
  });

  /**
   * The "Abort Queue" button is an interactive control. It must not sit
   * inside any element that carries `aria-live`, because an element
   * inside a live region can be announced to a screen reader user who
   * never moved focus anywhere near it -- confusing on its own, and
   * exactly what happened here before the fix (every counter tick
   * re-announced "Abort" along with everything else in the bar).
   */
  it('the Abort button is not inside a live region', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading')],
    });

    render(<StatusBar />);

    const abortButton = screen.getByRole('button', { name: /abort queue/i });
    expect(abortButton.closest('[aria-live]')).toBeNull();
  });

  /**
   * Polish pass (L5): the Abort button used to sit in the MIDDLE of the
   * counts ("1 queued [Abort] 1 completed"), where it read as one more
   * count. It must come after every count.
   */
  it('puts the Abort button after every count, not among them', () => {
    useDownloadStore.setState({
      queueItems: [createItem('downloading'), createItem('queued'), createItem('complete')],
    });

    render(<StatusBar />);

    const bar = screen.getByTestId('status-bar-visible-counters');
    const abortButton = within(bar).getByRole('button', { name: /abort queue/i });
    const completed = within(bar).getByText(/1 completed/);
    // DOCUMENT_POSITION_FOLLOWING: the button comes after the last count.
    expect(completed.compareDocumentPosition(abortButton) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(abortButton.className).toContain('px-1.5 py-0.5');
  });

  // =========================================================================
  // After-queue indicator reads the SAVED action, never the Settings
  // screen's unsaved edit (#1222)
  // =========================================================================

  describe('after-queue indicator', () => {
    /**
     * The default `beforeEach` above only resets the download store.
     * This block also needs the settings store's `settings` and
     * `savedSettings` at a known, matching starting point --
     * "nothing armed, nothing unsaved" -- before every test.
     */
    beforeEach(() => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, after_queue_action: 'do_nothing', after_queue_once: null },
        savedSettings: {
          ...state.savedSettings,
          after_queue_action: 'do_nothing',
          after_queue_once: null,
        },
      }));
    });

    /**
     * Nothing is armed on disk and the screen has no unsaved edit --
     * the indicator should render nothing at all, same as before #1222.
     */
    it('shows nothing when the saved standing action is "do nothing"', () => {
      render(<StatusBar />);

      expect(screen.queryByText(/after queue/i)).not.toBeInTheDocument();
    });

    /**
     * The heart of #1222: the SAVED action is "Shut down", but the
     * Settings screen currently shows an unsaved edit that cleared it
     * to "do nothing". The bar must still say "Shut down" -- that is
     * what the queue will actually do -- never the screen's unsaved
     * "nothing".
     */
    it('shows the SAVED standing action even while an unsaved edit on the Settings screen would clear it', () => {
      useSettingsStore.setState((state) => ({
        savedSettings: { ...state.savedSettings, after_queue_action: 'shutdown_computer' },
        // The Settings screen has an in-progress, unsaved edit setting
        // it back to "do nothing" -- nobody has pressed Save.
        settings: { ...state.settings, after_queue_action: 'do_nothing' },
      }));

      render(<StatusBar />);

      expect(screen.getByText(/after queue: shut down/i)).toBeInTheDocument();
      expect(screen.queryByText(/do nothing/i)).not.toBeInTheDocument();
    });

    /**
     * When the Settings screen holds an unsaved edit that DIFFERS from
     * what is saved, the maintainer's decision was: show the saved
     * value, plus a short note saying there is an unsaved change --
     * never silently show one or the other with no indication.
     */
    it('adds an "(unsaved change)" note when the screen holds an edit to the standing action', () => {
      useSettingsStore.setState((state) => ({
        savedSettings: { ...state.savedSettings, after_queue_action: 'shutdown_computer' },
        settings: { ...state.settings, after_queue_action: 'restart_computer' },
      }));

      render(<StatusBar />);

      expect(screen.getByText(/after queue: shut down/i)).toBeInTheDocument();
      expect(screen.getByText(/unsaved change/i)).toBeInTheDocument();
    });

    /**
     * The reverse of the above: when the screen's copy and the saved
     * copy already agree (whether because nothing was edited, or the
     * edit was saved), no note should appear -- there is nothing
     * unsaved to warn about.
     */
    it('shows no "(unsaved change)" note when the edited and saved standing action match', () => {
      useSettingsStore.setState((state) => ({
        savedSettings: { ...state.savedSettings, after_queue_action: 'play_sound' },
        settings: { ...state.settings, after_queue_action: 'play_sound' },
      }));

      render(<StatusBar />);

      expect(screen.getByText(/after queue: play sound/i)).toBeInTheDocument();
      expect(screen.queryByText(/unsaved change/i)).not.toBeInTheDocument();
    });

    /**
     * A one-off ("do this once") always takes precedence over the
     * standing action when both are shown -- and that stays true even
     * when the Settings screen has an unrelated unsaved edit to the
     * standing action sitting underneath it.
     */
    // The one-off decides WHAT is shown; the unsaved edit to the standing
    // action underneath it still earns the note, because that is the value
    // that takes over once the one-off has run and cleared itself.
    it('shows the saved one-off action, with the note for an unsaved standing-action edit underneath it', () => {
      useSettingsStore.setState((state) => ({
        savedSettings: {
          ...state.savedSettings,
          after_queue_once: 'hibernate_computer',
          after_queue_action: 'do_nothing',
        },
        settings: { ...state.settings, after_queue_action: 'shutdown_computer' },
      }));

      render(<StatusBar />);

      expect(screen.getByText(/after queue: hibernate \(once\)/i)).toBeInTheDocument();
      expect(screen.getByText(/unsaved change/i)).toBeInTheDocument();
      expect(screen.queryByText(/shut down/i)).not.toBeInTheDocument();
    });
  });
});
