// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Crash report service.
// ======================
//
// Provides CRUD operations for crash report files stored in
// `{app_data_dir}/crashes/`. Crash reports are JSON files written by
// the Rust panic handler and the frontend error logging command.
//
// Functions in this module are called by the IPC command handlers in
// `commands/crash_reports.rs`.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use tauri::AppHandle;

use crate::models::crash_report::CrashReport;
use crate::utils::platform;

/// Matches an embedded `http(s)://` URL in free text, stopping at
/// whitespace or common textual delimiters (quotes, angle brackets) so
/// a URL quoted inside a log line (`for url 'http://...'`) or wrapped in
/// Markdown backticks doesn't swallow trailing prose.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^\s'"<>]+"#).expect("Invalid URL redaction regex")
});

/// Redacts credentials from any URLs embedded in free text (crash
/// backtraces).
///
/// Python tracebacks captured from GAMDL/httpx frequently embed the
/// full request URL in the exception summary (e.g.
/// `httpx.ConnectError: ... url: 'http://127.0.0.1:30020/account?token=SECRET'`).
/// Two redactions are applied to every matched URL:
///
/// 1. **Query string** — truncated at the first `?` (mirrors
///    `download_queue::redact_url_query`), since query parameters on
///    wrapper URLs frequently carry auth tokens.
/// 2. **Userinfo** — if an `@` appears before the first `/` following
///    `://` (i.e. `scheme://user:pass@host/...`), the `user:pass`
///    segment is replaced with `[redacted]`.
///
/// Idempotent: running this on already-redacted text is a no-op, since
/// a redacted URL has no `?` and no `user:pass@` left to strip.
pub(crate) fn redact_urls_in_text(text: &str) -> String {
    URL_RE
        .replace_all(text, |caps: &regex::Captures| redact_single_url(&caps[0]))
        .into_owned()
}

/// Applies the query-string-truncation and userinfo-stripping rules
/// described on [`redact_urls_in_text`] to a single URL string.
///
/// `pub(crate)` so other services (e.g. `download_queue::redact_url_query`)
/// can reuse the same redaction logic instead of re-implementing a
/// query-string-only variant that misses `user:pass@host` userinfo.
pub(crate) fn redact_single_url(url: &str) -> String {
    // Truncate at the first '?' — drops the entire query string,
    // including any auth tokens it carries.
    let truncated = match url.find('?') {
        Some(idx) => &url[..idx],
        None => url,
    };

    // Strip userinfo: look for "://", then check whether an '@' appears
    // before the first '/' that follows it (i.e. within the authority
    // component, not later in the path).
    if let Some(scheme_end) = truncated.find("://") {
        let after_scheme = &truncated[scheme_end + 3..];
        let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if let Some(at_idx) = authority.find('@') {
            let host_and_rest = &after_scheme[at_idx + 1..];
            let scheme_prefix = &truncated[..scheme_end + 3];
            return format!("{scheme_prefix}[redacted]@{host_and_rest}");
        }
    }

    truncated.to_string()
}

/// Returns the path to the crash reports directory.
/// Creates the directory if it does not exist.
fn crashes_dir(app: &AppHandle) -> PathBuf {
    let dir = platform::get_app_data_dir(app).join("crashes");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!(
            "Failed to create crash reports directory {}: {e}",
            dir.display()
        );
    }
    dir
}

/// Lists all crash reports, sorted by timestamp (newest first).
///
/// Reads all `crash-*.json` files from the crashes directory and
/// deserializes them into `CrashReport` structs. Files that fail
/// to parse are silently skipped.
pub fn list_crash_reports(app: &AppHandle) -> Vec<CrashReport> {
    let dir = crashes_dir(app);
    let mut reports: Vec<CrashReport> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<CrashReport>(&contents) {
                        reports.push(report);
                    }
                }
            }
        }
    }

    // Sort by timestamp descending (newest first)
    reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    reports
}

/// Retrieves a single crash report by its ID.
///
/// Scans all crash report files and returns the one matching the
/// given ID, or `None` if not found.
pub fn get_crash_report(app: &AppHandle, id: &str) -> Option<CrashReport> {
    list_crash_reports(app).into_iter().find(|r| r.id == id)
}

/// Deletes a crash report by its ID.
///
/// Scans all JSON files in the crashes directory, deserializes each,
/// and removes the one whose `id` field matches. Returns `Ok(())` if
/// the file was deleted, `Err` if not found or deletion failed.
///
/// All logging uses INFO level (not DEBUG) so diagnostics are visible
/// in production log files.
pub fn delete_crash_report(app: &AppHandle, id: &str) -> Result<(), String> {
    let dir = crashes_dir(app);
    log::info!(
        "delete_crash_report: looking for id={id} in {}",
        dir.display()
    );

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            let msg = format!("Failed to read crashes directory: {e}");
            log::warn!("delete_crash_report: {msg}");
            return Err(msg);
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::info!(
                    "delete_crash_report: skipping {} (read failed: {e})",
                    path.display()
                );
                continue;
            }
        };

        let report = match serde_json::from_str::<CrashReport>(&contents) {
            Ok(r) => r,
            Err(e) => {
                log::info!(
                    "delete_crash_report: skipping {} (parse failed: {e})",
                    path.display()
                );
                continue;
            }
        };

        if report.id == id {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete crash report file: {e}"))?;
            log::info!("delete_crash_report: deleted {}", path.display());
            return Ok(());
        }
    }

    // Report wasn't found — return an error so the frontend knows
    let msg = format!("Crash report not found: {id}");
    log::warn!("delete_crash_report: {msg}");
    Err(msg)
}

/// Deletes all crash report files from the crashes directory.
///
/// Removes every `.json` file without parsing — this is a brute-force
/// cleanup that bypasses any potential deserialization issues. Returns
/// the number of files successfully deleted.
pub fn delete_all_crash_reports(app: &AppHandle) -> Result<u32, String> {
    let dir = crashes_dir(app);
    log::info!(
        "delete_all_crash_reports: clearing all reports in {}",
        dir.display()
    );

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            let msg = format!("Failed to read crashes directory: {e}");
            log::warn!("delete_all_crash_reports: {msg}");
            return Err(msg);
        }
    };

    let mut deleted = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    log::info!("delete_all_crash_reports: deleted {}", path.display());
                    deleted += 1;
                }
                Err(e) => {
                    log::warn!(
                        "delete_all_crash_reports: failed to delete {}: {e}",
                        path.display()
                    );
                }
            }
        }
    }

    log::info!("delete_all_crash_reports: deleted {deleted} file(s)");
    Ok(deleted)
}

/// Exports a crash report as a formatted Markdown string suitable for
/// pasting into a GitHub issue or sharing with a developer.
pub fn export_crash_report(app: &AppHandle, id: &str) -> Result<String, String> {
    let report =
        get_crash_report(app, id).ok_or_else(|| format!("Crash report not found: {id}"))?;

    let mut md = String::new();
    md.push_str("## MeedyaDL Crash Report\n\n");
    md.push_str(&format!("**ID:** `{}`\n", report.id));
    md.push_str(&format!("**Timestamp:** {}\n", report.timestamp));
    md.push_str(&format!("**App Version:** {}\n", report.app_version));
    md.push_str(&format!("**OS:** {} ({})\n", report.os, report.arch));
    md.push_str(&format!("**Source:** {}\n", report.source));

    if let Some(ref msg) = report.panic_message {
        md.push_str(&format!("\n### Error Message\n\n```\n{msg}\n```\n"));
    }

    if let Some(ref loc) = report.location {
        md.push_str(&format!("\n**Location:** `{loc}`\n"));
    }

    if let Some(ref bt) = report.backtrace {
        // Belt-and-braces redaction: the backtrace should already be
        // redacted at capture time (traceback_diagnostic.rs), but a
        // second pass here is idempotent and guards against any other
        // future writer of `CrashReport.backtrace` that forgets to
        // redact before saving.
        let redacted_bt = redact_urls_in_text(bt);
        md.push_str(&format!("\n### Backtrace\n\n```\n{redacted_bt}\n```\n"));
    }

    if !report.context.is_empty() {
        md.push_str("\n### Context\n\n");
        for (key, value) in &report.context {
            md.push_str(&format!("- **{key}:** {value}\n"));
        }
    }

    Ok(md)
}

/// Maximum character count for the issue body before backtrace truncation.
///
/// GitHub supports ~8192 chars in a URL, and modern browsers handle even more
/// (Chrome ~32KB, Safari ~80KB, Firefox ~65KB). However, percent-encoding can
/// roughly double the size for content with backticks, newlines, and special
/// characters. A 3500-char raw body keeps the final encoded URL well under
/// 8000 characters on all platforms.
const MAX_BODY_CHARS: usize = 3500;

/// GitHub repository path used in the issue URL.
const GITHUB_REPO: &str = "MWBMPartners/MeedyaDL";

/// Builds a pre-filled GitHub new-issue URL for a crash report.
///
/// The URL opens `github.com/MWBMPartners/MeedyaDL/issues/new` with
/// query parameters for `title`, `body`, and `labels`. The body contains
/// formatted Markdown with crash details, a "Steps to Reproduce" template
/// for the user to fill in, and a footer attribution.
///
/// If the body (including full backtrace) would exceed [`MAX_BODY_CHARS`],
/// the backtrace is truncated to the first 15 + last 5 lines with a
/// `[truncated]` marker, and a note directs the user to the locally saved
/// full report.
///
/// Uses the `url` crate's `Url::parse()` + `query_pairs_mut()` for proper
/// percent-encoding of all query parameter values.
pub fn build_github_issue_url(app: &AppHandle, id: &str) -> Result<String, String> {
    let report =
        get_crash_report(app, id).ok_or_else(|| format!("Crash report not found: {id}"))?;

    // Build the issue title -- cap error message at 80 chars for readability.
    // Use different prefix and labels for download errors vs crashes.
    let is_download_error = report.source == "download_error";
    let title_prefix = if is_download_error {
        "Error Report"
    } else {
        "Crash Report"
    };
    // Redact operator account names (`/Users/{name}/…`, `/home/{name}/…`,
    // `C:\Users\{name}\…`) out of the panic message before it's embedded
    // in a *public* GitHub issue URL. Panic messages frequently include
    // source-adjacent data (failed path operations, file-not-found
    // errors) that can carry the local username.
    let redacted_panic_message = report
        .panic_message
        .as_deref()
        .map(crate::services::diagnostic_bundle::redact_path_usernames);

    let error_summary: String = redacted_panic_message
        .as_deref()
        .unwrap_or("Unknown error")
        .chars()
        .take(80)
        .collect();
    let title = format!(
        "[{title_prefix}] {} - {} ({})",
        error_summary, report.os, report.arch
    );

    // Build the Markdown body
    let mut body = String::new();
    body.push_str(&format!("## {title_prefix}\n\n"));
    body.push_str(&format!("**App Version:** {}\n", report.app_version));
    body.push_str(&format!("**OS:** {} ({})\n", report.os, report.arch));
    body.push_str(&format!("**Source:** {}\n", report.source));
    body.push_str(&format!("**Timestamp:** {}\n", report.timestamp));
    body.push_str(&format!("**Report ID:** `{}`\n", report.id));

    if let Some(ref msg) = redacted_panic_message {
        body.push_str(&format!("\n### Error Message\n\n```\n{msg}\n```\n"));
    }

    if let Some(ref loc) = report.location {
        body.push_str(&format!("\n**Location:** `{loc}`\n"));
    }

    // Backtrace with truncation logic for URL length limits.
    // Belt-and-braces redaction (see `export_crash_report` above) before
    // this goes into a *public* GitHub issue body.
    if let Some(ref raw_bt) = report.backtrace {
        let bt = redact_urls_in_text(raw_bt);
        let lines: Vec<&str> = bt.lines().collect();
        if body.len() + bt.len() + 50 > MAX_BODY_CHARS && lines.len() > 25 {
            // Truncate: keep first 15 + last 5 lines (most diagnostic)
            let head = lines[..15].join("\n");
            let tail = lines[lines.len().saturating_sub(5)..].join("\n");
            let truncated =
                format!("{head}\n\n... [truncated — full backtrace saved locally] ...\n\n{tail}");
            body.push_str(&format!(
                "\n### Backtrace (truncated)\n\n```\n{truncated}\n```\n"
            ));
        } else {
            body.push_str(&format!("\n### Backtrace\n\n```\n{bt}\n```\n"));
        }
    }

    // Context key-value pairs. Values frequently include settings/paths
    // that can carry the operator's account name — redact before this
    // goes into a public issue body.
    if !report.context.is_empty() {
        body.push_str("\n### Context\n\n");
        for (key, value) in &report.context {
            let redacted_value = crate::services::diagnostic_bundle::redact_path_usernames(value);
            body.push_str(&format!("- **{key}:** {redacted_value}\n"));
        }
    }

    // User-fillable sections and footer
    body.push_str("\n---\n\n");
    body.push_str("### Steps to Reproduce\n\n");
    body.push_str("*Please describe what you were doing when the crash occurred:*\n\n");
    body.push_str("1. \n2. \n3. \n\n");
    body.push_str("### Additional Context\n\n");
    body.push_str("*Add any other context about the problem here.*\n\n");
    body.push_str("---\n\n");
    body.push_str(&format!(
        "> *This crash report was generated by MeedyaDL v{}. \
         Full crash report saved locally with ID: `{}`*\n",
        report.app_version, report.id
    ));

    // Final hard truncation if body is still too long after backtrace
    // truncation (e.g., very long error messages or context sections)
    if body.len() > MAX_BODY_CHARS {
        body.truncate(MAX_BODY_CHARS - 100);
        body.push_str("\n\n... [body truncated for URL length limits] ...");
    }

    // Build the URL using the `url` crate for proper percent-encoding.
    // `query_pairs_mut().append_pair()` delegates to `form_urlencoded`
    // which handles all special characters (backticks, newlines, etc.).
    let base = format!("https://github.com/{GITHUB_REPO}/issues/new");
    let mut url = url::Url::parse(&base).map_err(|e| format!("Failed to parse base URL: {e}"))?;

    let labels = if is_download_error {
        "bug,error-report"
    } else {
        "bug,crash-report"
    };
    url.query_pairs_mut()
        .append_pair("title", &title)
        .append_pair("body", &body)
        .append_pair("labels", labels);

    Ok(url.to_string())
}

/// Saves an error report (crash, download error, or other) to a JSON file.
///
/// This is the generic save function used by all report sources:
/// - Frontend errors (via `log_frontend_error` IPC command)
/// - Download errors (via download queue terminal failure)
/// - Rust panics (via `setup_panic_handler()`)
///
/// Reports are stored in `{app_data_dir}/crashes/` as JSON files and can
/// be listed, viewed, exported, or reported to GitHub Issues via the
/// Settings > Advanced > Crash Reporting section.
pub fn save_error_report(app: &AppHandle, report: CrashReport) -> Result<(), String> {
    let dir = crashes_dir(app);
    let filename = format!("crash-{}.json", report.timestamp.replace(':', "-"));
    let path = dir.join(filename);

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("Failed to serialize error report: {e}"))?;

    std::fs::write(&path, json).map_err(|e| format!("Failed to write error report: {e}"))?;

    log::info!(
        "Error report saved: {} (source: {})",
        path.display(),
        report.source
    );
    Ok(())
}

/// Saves a crash report from the frontend to a JSON file on disk.
///
/// Called by the `log_frontend_error` IPC command when the React frontend
/// catches an error via ErrorBoundary, window.onerror, or unhandledrejection.
/// Delegates to [`save_error_report`].
pub fn save_frontend_crash_report(app: &AppHandle, report: CrashReport) -> Result<(), String> {
    save_error_report(app, report)
}

/// Deletes crash reports older than 30 days.
///
/// Called during application startup to prevent the crashes directory
/// from growing indefinitely. Uses the file's modification time rather
/// than parsing the JSON timestamp for efficiency.
pub fn clear_old_reports(app: &AppHandle) {
    let dir = crashes_dir(app);
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 60 * 60); // 30 days

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                            log::info!("Cleaned old crash report: {}", path.display());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #975 — an httpx exception summary embeds the full request URL,
    /// including an auth token query parameter. The redacted text must
    /// keep enough of the URL to be diagnostically useful (host + path)
    /// while dropping the token entirely.
    #[test]
    fn redacts_query_string_token_from_url() {
        let input = "httpx.ConnectError: ... for url 'http://pi.local:30020/account?token=SECRET'";
        let out = redact_urls_in_text(input);
        assert!(
            out.contains("pi.local:30020/account"),
            "expected host+path to survive redaction, got: {out}"
        );
        assert!(!out.contains("SECRET"), "token value leaked: {out}");
        assert!(!out.contains("token="), "query key leaked: {out}");
    }

    /// A URL with embedded `user:pass` userinfo (rare, but possible for
    /// self-hosted wrapper setups behind basic auth) must have the
    /// credentials stripped while the host stays visible for diagnosis.
    #[test]
    fn redacts_userinfo_credentials_from_url() {
        let input = "http://user:pass@host/x";
        let out = redact_urls_in_text(input);
        assert!(
            out.contains("[redacted]@host"),
            "expected userinfo placeholder, got: {out}"
        );
        assert!(!out.contains("user:pass"), "credentials leaked: {out}");
    }

    /// Multiple URLs in the same line (e.g. a retry log showing both the
    /// original and fallback wrapper URL) must each be redacted
    /// independently — `replace_all` should not stop at the first match.
    #[test]
    fn redacts_multiple_urls_in_one_line() {
        let input =
            "primary http://a.example.com/x?token=AAA failed, retrying http://b.example.com/y?token=BBB";
        let out = redact_urls_in_text(input);
        assert!(!out.contains("AAA"), "first token leaked: {out}");
        assert!(!out.contains("BBB"), "second token leaked: {out}");
        assert!(out.contains("a.example.com/x"), "first host lost: {out}");
        assert!(out.contains("b.example.com/y"), "second host lost: {out}");
    }

    /// Plain text with no embedded URL must pass through unchanged — the
    /// regex should not misfire on ordinary traceback prose.
    #[test]
    fn plain_text_without_url_is_unchanged() {
        let input = "TypeError: Task was destroyed but it is pending!";
        assert_eq!(redact_urls_in_text(input), input);
    }
}
