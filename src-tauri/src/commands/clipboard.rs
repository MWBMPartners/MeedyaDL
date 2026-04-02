// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Clipboard IPC command handlers.
// ================================
//
// Provides `read_clipboard` for the frontend's clipboard monitoring
// feature. The frontend polls this command at a configurable interval
// to detect supported URLs copied to the system clipboard.
//
// @see services::clipboard_service -- Business logic
// @see src/hooks/useClipboardMonitor.ts -- Frontend polling hook

use crate::services::clipboard_service;

/// Read the current text content from the system clipboard.
///
/// Returns `Ok(Some(text))` when text is present, `Ok(None)` when the
/// clipboard is empty or contains non-text data. The frontend is
/// responsible for validating the text as a supported URL.
///
/// IPC name: `read_clipboard`
/// Frontend wrapper: `readClipboard()` in `tauri-commands.ts`
#[tauri::command]
pub fn read_clipboard() -> Result<Option<String>, String> {
    clipboard_service::read_clipboard_text()
}
