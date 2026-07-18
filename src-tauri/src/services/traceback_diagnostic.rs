// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See the LICENSE file in the project
// root for full licence text.
//
//! Python traceback diagnostic capture (#758).
//!
//! GAMDL and its Python dependencies (`httpx`, `async_lru`,
//! `gamdl.interface`) occasionally raise multi-line Python tracebacks
//! during otherwise-successful downloads — notably during cover-bytes
//! fetch, syllable-lyrics requests, and music-video relation lookups.
//! The activity-log filter at [`crate::utils::process::is_python_traceback_noise`]
//! suppresses the visual noise but loses the diagnostic signal.
//!
//! This module reads the captured stdout/stderr buffer at item
//! completion, extracts traceback **groups** (header → frames →
//! exception summary), deduplicates identical groups, and writes a
//! diagnostic report under the existing crash-report infrastructure.
//! The report shows up in Settings → Advanced → Crash Reporting with
//! `source = "traceback_diagnostic"` so users can review it or submit
//! it upstream.
//!
//! # Distinguishing from terminal-error reports
//!
//! Terminal error reports (`source = "download_error"`) fire only
//! when a download fails after all fallbacks and retries are
//! exhausted. Traceback diagnostics fire on **any** download where
//! traceback noise was observed — including successful ones. This is
//! deliberate: cover-bytes fetch raising a traceback every time on
//! `cover_format = raw` doesn't fail the download (the embedded
//! cover in the M4A still works), but it IS a latent bug we want
//! aggregated visibility on.

use std::collections::HashMap;

use tauri::AppHandle;

use crate::models::crash_report::CrashReport;
use crate::services::crash_report_service;
use crate::utils::process::{is_python_exception_summary, is_python_traceback_noise};

/// A captured traceback with the number of times it appeared in the
/// download's output. Duplicate groups collapse so a 19-track album
/// where every track raised the same cover-bytes traceback reports
/// once with `count == 19`, not 19 separate entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedTraceback {
    /// Full traceback text — header, frames, caret lines, and the
    /// closing exception summary line, joined by `\n`.
    pub text: String,
    /// How many times this exact traceback appeared.
    pub count: usize,
}

/// Scans `lines` (typically a combined stdout/stderr buffer captured
/// during a download) for Python tracebacks and returns deduplicated
/// groups in observation order.
///
/// Each group is delimited by:
/// * **Start** — a `Traceback (most recent call last):` header,
/// * **Body** — `File "<path>", line N, in <fn>`, source-code context
///   lines, and caret highlight lines (`^^^^^`),
/// * **End** — a non-noise line. If that closing line matches the
///   Python exception-summary regex (`TypeError: …`,
///   `httpx.ConnectError: …`), it's appended to the group. Otherwise
///   the group is closed without a summary.
///
/// Groups that contain only a header (zero frames, no summary) are
/// discarded — they're not useful for forensics.
#[must_use]
pub fn extract_traceback_groups(lines: &[String]) -> Vec<CapturedTraceback> {
    let mut groups: Vec<String> = Vec::new();
    let mut current: Option<Vec<String>> = None;

    for line in lines {
        let trimmed = line.trim();

        if trimmed == "Traceback (most recent call last):" {
            // Starting a new group. Close any open one as a "dangling"
            // group — we never saw its summary line.
            if let Some(buf) = current.take() {
                if buf.len() > 1 {
                    groups.push(buf.join("\n"));
                }
            }
            current = Some(vec![line.clone()]);
            continue;
        }

        // Not inside a group → nothing to capture.
        let Some(buf) = current.as_mut() else {
            continue;
        };

        if is_python_traceback_noise(line) {
            // File frame, caret highlight, or stray "Traceback" repeats.
            buf.push(line.clone());
        } else if line.starts_with(' ') || line.starts_with('\t') {
            // PEP 657 source-code context line — indented snippet
            // pulled from the failing file. We can't detect this with
            // a regex (it's literally source code), so we use the
            // heuristic that intra-traceback non-frame lines tend to
            // be indented from the column-1 exception-summary line.
            // Keep them as part of the group.
            buf.push(line.clone());
        } else if is_python_exception_summary(line) {
            // Closing summary — finalise this group.
            buf.push(line.clone());
            let finished = std::mem::take(buf);
            current = None;
            if finished.len() > 1 {
                groups.push(finished.join("\n"));
            }
        } else if trimmed.is_empty() {
            // Blank line inside traceback (structlog sometimes emits
            // these) — just continue.
        } else {
            // An unrelated log line interrupted the traceback — close
            // the group with whatever we've captured so far.
            let abandoned = std::mem::take(buf);
            current = None;
            if abandoned.len() > 1 {
                groups.push(abandoned.join("\n"));
            }
        }
    }

    // Drain anything still buffered at end-of-input (e.g., process
    // was killed mid-traceback).
    if let Some(buf) = current {
        if buf.len() > 1 {
            groups.push(buf.join("\n"));
        }
    }

    dedup_with_counts(groups)
}

/// Collapses identical strings, preserving first-seen order. Used so
/// 19 identical cover-bytes tracebacks (one per track) report as a
/// single entry with `count == 19` instead of 19 duplicates.
fn dedup_with_counts(groups: Vec<String>) -> Vec<CapturedTraceback> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for g in groups {
        if !counts.contains_key(&g) {
            order.push(g.clone());
        }
        *counts.entry(g).or_insert(0) += 1;
    }
    order
        .into_iter()
        .map(|text| {
            let count = counts[&text];
            CapturedTraceback { text, count }
        })
        .collect()
}

/// Renders the captured groups into a single backtrace string suitable
/// for the `CrashReport.backtrace` field.
fn format_groups(groups: &[CapturedTraceback]) -> String {
    let mut out = String::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n=========================================\n\n");
        }
        out.push_str(&format!("Occurrences: {}\n\n", g.count));
        out.push_str(&g.text);
    }
    out
}

/// Writes a diagnostic crash report for the captured tracebacks, if
/// any. Returns `Ok(0)` when no tracebacks were captured (the
/// common-case fast path on healthy downloads), `Ok(n)` for the
/// number of distinct groups written, or an error if the underlying
/// file-write failed.
///
/// `url` is included verbatim in the report context — the caller is
/// responsible for redacting any sensitive query parameters before
/// passing it in.
pub fn write_diagnostic_if_any(
    app: &AppHandle,
    download_id: &str,
    url: &str,
    gamdl_version: Option<&str>,
    item_state: &str,
    lines: &[String],
) -> Result<usize, String> {
    let groups = extract_traceback_groups(lines);
    if groups.is_empty() {
        return Ok(0);
    }
    let n = groups.len();

    let mut context = HashMap::new();
    context.insert("download_id".to_string(), download_id.to_string());
    context.insert("url".to_string(), url.to_string());
    context.insert("item_state".to_string(), item_state.to_string());
    if let Some(v) = gamdl_version {
        context.insert("gamdl_version".to_string(), v.to_string());
    }
    context.insert(
        "total_occurrences".to_string(),
        groups.iter().map(|g| g.count).sum::<usize>().to_string(),
    );

    let report = CrashReport {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        source: "traceback_diagnostic".to_string(),
        panic_message: Some(format!("{n} distinct Python traceback group(s) captured")),
        location: None,
        // Redact any auth tokens / credentials embedded in URLs the
        // Python exception summary echoed back (e.g. httpx
        // ConnectError messages include the full request URL) before
        // the backtrace is ever written to disk (#975).
        backtrace: Some(crash_report_service::redact_urls_in_text(&format_groups(
            &groups,
        ))),
        context,
    };

    crash_report_service::save_error_report(app, report).map(|()| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(input: &str) -> Vec<String> {
        input.lines().map(str::to_string).collect()
    }

    #[test]
    fn empty_input_returns_no_groups() {
        let groups = extract_traceback_groups(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn input_without_traceback_returns_no_groups() {
        let groups = extract_traceback_groups(&lines(
            "[Track 1/19] Downloading\nFinished with 0 error(s)\n",
        ));
        assert!(groups.is_empty());
    }

    #[test]
    fn single_traceback_captured() {
        let input = "Traceback (most recent call last):
  File \"/path/gamdl/interface.py\", line 577, in get_cover_bytes
    response = await self.session.get(cover_url)
httpx.ConnectError: Connection refused";
        let groups = extract_traceback_groups(&lines(input));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 1);
        assert!(groups[0].text.contains("get_cover_bytes"));
        assert!(groups[0].text.contains("httpx.ConnectError"));
    }

    #[test]
    fn duplicate_tracebacks_are_deduplicated() {
        // 3 copies of the same traceback (cover-bytes fetch
        // failing for 3 tracks).
        let one = "Traceback (most recent call last):
  File \"/path/gamdl/interface.py\", line 577, in get_cover_bytes
    response = await self.session.get(cover_url)
httpx.ConnectError: Connection refused";
        let input = format!("{one}\n\n{one}\n\n{one}\n");
        let groups = extract_traceback_groups(&lines(&input));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 3);
    }

    #[test]
    fn pep657_context_lines_are_captured() {
        // Python 3.11+ tracebacks include source-code context lines
        // and caret highlights pointing at the failing expression.
        let input = "Traceback (most recent call last):
  File \"/path/gamdl/base.py\", line 42, in run
    return await asyncio.shield(task)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^
RuntimeError: Task was destroyed";
        let groups = extract_traceback_groups(&lines(input));
        assert_eq!(groups.len(), 1);
        assert!(groups[0].text.contains("asyncio.shield"));
        assert!(groups[0].text.contains("RuntimeError"));
    }

    #[test]
    fn distinct_tracebacks_kept_separate() {
        let input = "Traceback (most recent call last):
  File \"/a.py\", line 1, in foo
httpx.ConnectError: A
Traceback (most recent call last):
  File \"/b.py\", line 2, in bar
TypeError: B";
        let groups = extract_traceback_groups(&lines(input));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].count, 1);
        assert_eq!(groups[1].count, 1);
        assert!(groups[0].text.contains("httpx.ConnectError: A"));
        assert!(groups[1].text.contains("TypeError: B"));
    }

    #[test]
    fn dangling_traceback_without_summary_is_captured() {
        // Process killed mid-traceback — we should still keep what
        // we have so the forensic record isn't lost.
        let input = "Traceback (most recent call last):
  File \"/path/foo.py\", line 5, in bar
    do_thing()";
        let groups = extract_traceback_groups(&lines(input));
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn traceback_interrupted_by_unrelated_log_is_captured() {
        // Real-world case: structlog emits an unrelated INFO line
        // partway through a traceback. We should still keep the
        // captured portion.
        let input = "Traceback (most recent call last):
  File \"/path/foo.py\", line 5, in bar
[INFO    14:21:09] Some unrelated log line
Traceback (most recent call last):
  File \"/path/baz.py\", line 7, in qux
ValueError: bad";
        let groups = extract_traceback_groups(&lines(input));
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn lone_traceback_header_is_discarded() {
        let input = "Traceback (most recent call last):
[INFO    14:21:09] Recovered";
        let groups = extract_traceback_groups(&lines(input));
        assert!(groups.is_empty());
    }

    #[test]
    fn format_groups_separates_with_divider() {
        let g = vec![
            CapturedTraceback {
                text: "first".to_string(),
                count: 5,
            },
            CapturedTraceback {
                text: "second".to_string(),
                count: 1,
            },
        ];
        let formatted = format_groups(&g);
        assert!(formatted.contains("Occurrences: 5"));
        assert!(formatted.contains("Occurrences: 1"));
        assert!(formatted.contains("====="));
    }
}
