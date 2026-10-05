// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file A button that reads like a link inside a sentence or a line of
 * small print: "Go to Settings", "Reset", "Apple Developer account",
 * "Download manually from GitHub".
 *
 * Polish pass (M17): these were hand-made in about fifteen places with four
 * different colours (accent, accent-hover, red-400, tertiary) and two
 * underline habits. One look now: accent text, underlined, darker on hover;
 * `tone="danger"` for the few that delete something.
 *
 * A button, not an <a>, because what it does is an action (open a page in
 * the app, open the system browser through Tauri, clear a list) rather
 * than a web address the WebView should follow.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface TextButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  /** 'danger' for actions that delete something. */
  tone?: 'default' | 'danger';
  /** Text size; inherits the surrounding text when omitted. */
  size?: 'xs' | 'sm';
}

export function TextButton({ children, tone = 'default', size, className = '', type = 'button', ...rest }: TextButtonProps) {
  return (
    <button
      type={type}
      className={`inline cursor-pointer underline underline-offset-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed
        focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2
        ${tone === 'danger' ? 'text-status-error-text hover:opacity-80' : 'text-accent-hover hover:text-accent'}
        ${size === 'xs' ? 'text-xs' : size === 'sm' ? 'text-sm' : ''}
        ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
