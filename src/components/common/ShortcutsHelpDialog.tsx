// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file ShortcutsHelpDialog.tsx -- Keyboard shortcuts reference modal (#465).
 *
 * Triggered globally via `Cmd/Ctrl + Shift + ?` (the standard "what
 * shortcuts does this app have" binding across most macOS / Linux
 * desktop apps). Renders a Modal listing every shortcut handled by
 * `useKeyboardShortcuts.ts` plus the component-local ones
 * (Modal's Escape, DownloadForm's Enter-to-submit).
 *
 * Two responsibilities:
 *
 *   1. Subscribes to a global `ui.shortcutsHelpOpen` boolean in the
 *      `uiStore`. The keyboard handler flips that boolean; the
 *      dialog reacts.
 *   2. Renders a single-modal table grouped by category so the
 *      user can scan visually. Each row is `<kbd>` + description.
 *
 * Keeping this in `components/common/` (alongside Modal itself)
 * because it has no business-logic dependencies — it's a pure
 * display surface keyed off `ui.shortcutsHelpOpen`.
 */

import { Modal } from './Modal';
import { useUiStore } from '@/stores/uiStore';

/**
 * Detect macOS to render `⌘` instead of `Ctrl` in the shortcut
 * labels. Falls back to `Ctrl` on unknown platforms so the user
 * still sees a sensible string. The actual handler in
 * `useKeyboardShortcuts.ts` accepts both `metaKey` and `ctrlKey`
 * so the labels are presentation only.
 */
function isMacPlatform(): boolean {
  if (typeof navigator === 'undefined') return false;
  // `navigator.platform` is deprecated but still the simplest cue for choosing a label.
  // The shortcuts themselves work on either key, so a wrong guess only changes the text.
  // See: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/platform
  return /Mac/i.test(navigator.platform);
}

/** Render a key glyph as a small kbd-styled chip. */
// `<kbd>` is the HTML element for keyboard input.
// See: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/kbd
function Key({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="inline-block min-w-[1.75rem] text-center px-1.5 py-0.5 rounded-platform-sm border border-border bg-surface-secondary text-content-primary text-xs font-mono leading-none">
      {children}
    </kbd>
  );
}

/** One row in the shortcut table — chord on the left, label on the right. */
function ShortcutRow({ keys, label }: { keys: React.ReactNode; label: string }) {
  return (
    <div className="flex items-center justify-between py-1 gap-3">
      <span className="text-sm text-content-secondary">{label}</span>
      <span className="flex items-center gap-1">{keys}</span>
    </div>
  );
}

/**
 * The "Keyboard Shortcuts" help dialog. Whether it is open lives in the UI
 * store (`shortcutsHelpOpen`), so the global key handler can open it from
 * anywhere. The list below is typed by hand: when a shortcut is added or
 * changed in `useKeyboardShortcuts.ts`, this list has to be updated too.
 */
export function ShortcutsHelpDialog(): React.JSX.Element {
  const open = useUiStore((s) => s.shortcutsHelpOpen);
  const setOpen = useUiStore((s) => s.setShortcutsHelpOpen);
  // The key label: the Command symbol on a Mac, "Ctrl" elsewhere.
  const mod = isMacPlatform() ? '⌘' : 'Ctrl';

  return (
    <Modal open={open} onClose={() => setOpen(false)} title="Keyboard Shortcuts" maxWidth="md">
      <div className="space-y-4 text-content-primary">
        <section>
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">Navigation</h3>
          <ShortcutRow
            label="Go to Download"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>D</Key>
              </>
            }
          />
          <ShortcutRow
            label="Go to Queue"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>J</Key>
              </>
            }
          />
          <ShortcutRow
            label="Go to Library"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>L</Key>
              </>
            }
          />
          <ShortcutRow
            label="Go to History"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>Y</Key>
              </>
            }
          />
          <ShortcutRow
            label="Go to Activity"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>K</Key>
              </>
            }
          />
          <ShortcutRow
            label="Go to Settings"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>,</Key>
              </>
            }
          />
        </section>

        <section>
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">Downloads</h3>
          <ShortcutRow
            label="Submit current URL (in Download input; Shift+Enter for a new line instead)"
            keys={<Key>↵</Key>}
          />
          <ShortcutRow
            label="Abort all active and queued downloads"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>⇧</Key>
                <span>+</span>
                <Key>.</Key>
              </>
            }
          />
        </section>

        <section>
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">General</h3>
          <ShortcutRow label="Close active dialog" keys={<Key>Esc</Key>} />
          <ShortcutRow
            label="Show this dialog"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>⇧</Key>
                <span>+</span>
                <Key>?</Key>
              </>
            }
          />
        </section>

        <p className="text-xs text-content-tertiary pt-2 border-t border-border-light">
          Shortcut handling is suppressed while typing in text fields so editing isn't interrupted.
        </p>
      </div>
    </Modal>
  );
}
