// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file An error shown on the page rather than in a toast (polish pass M8):
 * a plain message saying what went wrong and what to do, with the
 * technical text folded under "Details".
 *
 * Several pages used to print the backend's error string by itself
 * ("Keychain error: ...", "pip install gamdl failed: ERROR: ..."). Pair
 * this with `explainError` (lib/errorMessages.ts), which keeps the
 * backend's own words when they are already a plain sentence:
 *
 *   {error && <InlineError {...explainError('MeedyaDL could not ...', error)} />}
 */

import { ErrorDetails } from './ErrorDetails';

interface InlineErrorProps {
  /** What went wrong and what to do next. */
  message: string;
  /** The technical text, folded away; omitted when it would repeat the message. */
  details?: string | null;
  /** 'box' draws the bordered error box the setup steps use; 'text' is a plain line. */
  look?: 'box' | 'text';
  /** Extra classes on the outer element (spacing). */
  className?: string;
}

export function InlineError({ message, details, look = 'text', className = '' }: InlineErrorProps) {
  const box = look === 'box' ? 'p-3 rounded-platform border border-status-error bg-status-error-bg' : '';
  return (
    <div role="alert" className={`${box} ${className}`}>
      <p className={`${look === 'box' ? 'text-sm' : 'text-xs'} text-status-error-text`}>{message}</p>
      <ErrorDetails details={details && details !== message ? details : null} />
    </div>
  );
}
