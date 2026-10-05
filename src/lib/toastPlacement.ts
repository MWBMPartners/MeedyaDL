// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file Where the toast stack starts, so toasts never cover an area a
 * page marks to keep clear (polish pass M7). Used by ToastContainer.
 *
 * Kept out of ToastContainer.tsx so that file exports only its
 * component (fast refresh in development works per component file).
 */

/** The least distance from the top of the window to the first toast (it
 *  was `top-20`, which clears every page's header). */
export const DEFAULT_TOP = 80;

/**
 * A page puts this attribute on an area toasts must never cover. The
 * Download page puts it on its link box and the buttons beside it.
 */
export const KEEP_CLEAR_ATTRIBUTE = 'data-toasts-keep-clear';

/**
 * Where the toast stack starts: 8px below the lowest keep-clear area on
 * screen, and never higher than 80px from the top.
 *
 * Why the stack moves down instead of somewhere else: the top-right
 * corner is where people look for messages, and on every page but
 * Download nothing important sits there. Anchoring the stack to the
 * bottom instead was considered and rejected: at the 550px minimum
 * height, three toasts stacked up from the bottom would reach back over
 * the very same buttons.
 *
 * What it cannot do: keep clear of an area a page has not marked.
 */
export function toastStackTop(): number {
  let top = DEFAULT_TOP;
  document.querySelectorAll<HTMLElement>(`[${KEEP_CLEAR_ATTRIBUTE}]`).forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.height > 0 && r.bottom > 0) top = Math.max(top, Math.ceil(r.bottom + 8));
  });
  return top;
}
