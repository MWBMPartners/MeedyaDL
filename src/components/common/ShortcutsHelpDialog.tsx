// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file ShortcutsHelpDialog.tsx -- Keyboard shortcuts reference modal (#465).
 *
 * Triggered globally via `Cmd/Ctrl + Shift + ?` (the standard "what
 * shortcuts does this app have" binding across most macOS / Linux
 * desktop apps). Renders a Modal listing every shortcut handled by
 * `useKeyboardShortcuts.ts` plus the component-local ones
 * (Modal's Escape, DownloadForm's Cmd+Enter).
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
  return /Mac/i.test(navigator.platform);
}

/** Render a key glyph as a small kbd-styled chip. */
function Key({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="inline-block min-w-[1.75rem] text-center px-1.5 py-0.5 rounded border border-border bg-surface-secondary text-content-primary text-xs font-mono leading-none">
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

export function ShortcutsHelpDialog(): React.JSX.Element {
  const open = useUiStore((s) => s.shortcutsHelpOpen);
  const setOpen = useUiStore((s) => s.setShortcutsHelpOpen);
  const mod = isMacPlatform() ? '⌘' : 'Ctrl';

  return (
    <Modal
      open={open}
      onClose={() => setOpen(false)}
      title="Keyboard Shortcuts"
      maxWidth="md"
    >
      <div className="space-y-4 text-content-primary">
        <section>
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">
            Navigation
          </h3>
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
                <Key>Q</Key>
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
                <Key>H</Key>
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
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">
            Downloads
          </h3>
          <ShortcutRow
            label="Submit current URL (in Download input)"
            keys={
              <>
                <Key>{mod}</Key>
                <span>+</span>
                <Key>↵</Key>
              </>
            }
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
          <h3 className="text-xs uppercase tracking-wide text-content-tertiary mb-1">
            General
          </h3>
          <ShortcutRow
            label="Close active dialog"
            keys={<Key>Esc</Key>}
          />
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
