// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Clipboard monitoring service.
// ==============================
//
// Provides cross-platform clipboard text reading for the clipboard
// monitoring feature. The frontend polls this service at a configurable
// interval to detect when the user copies a supported URL (e.g., Apple
// Music) to the system clipboard.
//
// Privacy-first design:
//   - Only reads clipboard text; never writes to or modifies the clipboard.
//   - The raw clipboard text is returned to the frontend for URL validation;
//     non-URL content is never stored or logged.
//   - The frontend handles all URL pattern matching and deduplication.
//
// Platform backends (via `arboard` crate):
//   - macOS: NSPasteboard
//   - Windows: Win32 clipboard API
//   - Linux: X11 selections / Wayland data offers
//
// @see https://docs.rs/arboard/ -- arboard crate documentation

use arboard::Clipboard;

/// Read the current text content from the system clipboard.
///
/// Returns `Ok(Some(text))` if the clipboard contains text, `Ok(None)` if
/// the clipboard is empty or contains non-text data (images, files, etc.),
/// and `Err` if the clipboard cannot be accessed.
///
/// # Errors
///
/// Returns an error string if the clipboard backend fails to initialise
/// or read (e.g., no display server on headless Linux).
pub fn read_clipboard_text() -> Result<Option<String>, String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {e}"))?;

    match clipboard.get_text() {
        Ok(text) if text.is_empty() => Ok(None),
        Ok(text) => Ok(Some(text)),
        // ContentNotAvailable means the clipboard has non-text content (image, etc.)
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(format!("Failed to read clipboard: {e}")),
    }
}
