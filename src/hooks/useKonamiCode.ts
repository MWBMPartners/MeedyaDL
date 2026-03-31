// Copyright (c) 2026 MeedyaDL
/**
 * @file useKonamiCode.ts -- Hidden gesture detector for developer access
 * @license MIT -- See LICENSE file in the project root.
 *
 * Listens for the Konami code key sequence (Up Up Down Down Left Right
 * Left Right B A) on any page. When detected, calls the provided callback.
 *
 * The sequence buffer resets after 2 seconds of inactivity to prevent
 * accidental activation from normal keyboard usage.
 *
 * ## Usage
 *
 * ```tsx
 * useKonamiCode(() => setShowDevModal(true));
 * ```
 */

import { useEffect, useRef } from 'react';

/** The Konami code key sequence. */
const KONAMI_SEQUENCE = [
  'ArrowUp',
  'ArrowUp',
  'ArrowDown',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'ArrowLeft',
  'ArrowRight',
  'KeyB',
  'KeyA',
];

/** Reset the sequence buffer after this many ms of inactivity. */
const RESET_TIMEOUT_MS = 2000;

/**
 * Registers a global keydown listener that detects the Konami code.
 *
 * @param onActivate - Callback fired when the full sequence is entered.
 */
export function useKonamiCode(onActivate: () => void): void {
  const bufferRef = useRef<string[]>([]);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Reset the inactivity timer on every key press.
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
      timerRef.current = setTimeout(() => {
        bufferRef.current = [];
      }, RESET_TIMEOUT_MS);

      // Append the key code to the buffer.
      bufferRef.current.push(e.code);

      // Keep only the last N keys (where N = sequence length).
      if (bufferRef.current.length > KONAMI_SEQUENCE.length) {
        bufferRef.current = bufferRef.current.slice(-KONAMI_SEQUENCE.length);
      }

      // Check for a match.
      if (
        bufferRef.current.length === KONAMI_SEQUENCE.length &&
        bufferRef.current.every((key, i) => key === KONAMI_SEQUENCE[i])
      ) {
        bufferRef.current = [];
        onActivate();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [onActivate]);
}
