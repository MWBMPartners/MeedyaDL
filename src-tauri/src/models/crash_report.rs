// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Crash report data model.
// =========================
//
// Defines the `CrashReport` struct used to represent a single crash or
// error report. Crash reports are written as JSON files to
// `{app_data_dir}/crashes/` by the Rust panic handler and by the
// `log_frontend_error` IPC command (for frontend errors).
//
// Each report is self-contained with all diagnostic information needed
// to investigate the issue: error message, backtrace, app version,
// platform, and additional context.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single crash or error report.
///
/// Created by the Rust panic handler (`setup_panic_handler()` in `lib.rs`)
/// or by the `log_frontend_error` IPC command when the React frontend
/// catches an unhandled error.
///
/// Reports are stored as `crash-{timestamp}.json` files in the
/// `{app_data_dir}/crashes/` directory and can be listed, viewed,
/// exported, or deleted via the crash report IPC commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// ISO 8601 timestamp of when the crash occurred
    pub timestamp: String,
    /// Application version at the time of the crash (e.g., "0.5.3")
    pub app_version: String,
    /// Operating system (e.g., "macos", "windows", "linux")
    pub os: String,
    /// CPU architecture (e.g., "aarch64", "x86_64")
    pub arch: String,
    /// Origin of the error report
    /// - `"rust_panic"` -- Rust panic handler
    /// - `"frontend_error"` -- React ErrorBoundary or window.onerror
    /// - `"unhandled_rejection"` -- Unhandled promise rejection
    pub source: String,
    /// The error/panic message (if available)
    pub panic_message: Option<String>,
    /// Source code location where the panic/error occurred
    pub location: Option<String>,
    /// Stack trace / backtrace (if available)
    pub backtrace: Option<String>,
    /// Additional key-value context (e.g., component stack, URL, settings)
    #[serde(default)]
    pub context: HashMap<String, String>,
}
