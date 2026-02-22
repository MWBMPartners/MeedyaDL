// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file Custom title bar placeholder component.
 *
 * This component currently returns `null` on all platforms because
 * `decorations: true` is set in `tauri.conf.json`, which means the
 * OS-native title bar (with minimize / maximize / close buttons) is
 * displayed on every platform:
 *  - **macOS**: traffic-light buttons in the native title bar.
 *  - **Windows**: standard Windows caption buttons.
 *  - **Linux**: window manager-provided title bar buttons.
 *
 * The component is retained as a named export so that `MainLayout`
 * does not need to be modified if a future design change requires
 * `decorations: false` and custom window controls on certain platforms.
 *
 * @see https://v2.tauri.app/reference/config/#windowconfig
 *      Tauri v2 window configuration reference (decorations flag).
 */

/**
 * Renders nothing — native OS window decorations are used on all platforms.
 *
 * If a future design iteration requires custom window chrome (e.g.,
 * `decorations: false` with frameless windows), restore the previous
 * implementation that rendered minimize / maximize / close buttons
 * using the Tauri Window API and Lucide icons.
 *
 * @returns `null` on all platforms.
 */
export function TitleBar() {
  return null;
}
