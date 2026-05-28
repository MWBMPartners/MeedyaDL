// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Bounded ring-buffer for accumulating subprocess output lines without
// unbounded memory growth (#893).
//
// The subprocess reader tasks in `services::download_queue` need to
// retain *some* historical output to support post-mortem analysis —
// Python traceback extraction (#660 friendly-message generation),
// storefront-mismatch detection (#666 fallback path), and traceback
// diagnostic file emission (#758). But the pre-#893 buffers were
// `Vec<String>` with `push`-only behaviour, which meant a single
// verbose multi-album download could accumulate 100+ MB of string
// allocations before the subprocess exited.
//
// `BoundedLineBuffer` caps both:
//
//   1. Total number of retained lines (newest-N policy)
//   2. The byte length of each individual line
//
// Both caps protect against pathological inputs: a malformed GAMDL
// release that spams millions of progress lines (cap 1), or a single
// binary-corrupted stdout line that's tens of MB long (cap 2).
//
// The newest-N policy is correct for the consumers we have today:
//
// - **Python traceback extraction** keys off the LAST `Traceback (most
//   recent call last):` block in the buffer. As long as the cap is
//   larger than the typical traceback frame depth (~30-50 frames),
//   the diagnostic still fires.
// - **Storefront-mismatch detection** scans for "AMP Resource Not
//   Found" — emitted within the first few error lines, so as long as
//   we retain the last few hundred lines we'll see it.
// - **`emit_inner` activity-log emission** doesn't use the buffer at
//   all — it emits each line directly as it arrives.

use std::collections::VecDeque;

/// Maximum number of individual subprocess output lines retained by
/// default. Sized for the realistic worst case (50-track album with
/// verbose mode + per-track companion-codec retries + tracebacks);
/// the diagnostic consumer only ever scans the last N lines anyway.
pub const DEFAULT_LINE_CAP_RAW: usize = 2_000;

/// Tighter cap for the error-message buffer. Error messages are
/// shorter and the post-mortem error-extraction only looks at the
/// last few lines, so 200 is plenty.
pub const DEFAULT_LINE_CAP_ERRORS: usize = 200;

/// Maximum bytes per individual line. A single line larger than this
/// gets truncated with a "...(truncated, N bytes total)" marker so
/// the reader can still classify it (e.g. spotting `Traceback` in
/// the prefix) without holding the full payload.
pub const DEFAULT_LINE_BYTE_CAP: usize = 64 * 1024;

/// A bounded ring buffer of subprocess output lines.
///
/// Stores at most [`Self::line_cap`] lines, each at most
/// [`Self::byte_cap`] bytes. Older lines are evicted FIFO when the
/// line cap is reached; oversize lines are truncated on insert with
/// a tail marker that preserves the original byte count.
///
/// `push` always succeeds — there is no error path. Truncation and
/// eviction are silent (the cap behaviour is the contract).
#[derive(Debug)]
pub struct BoundedLineBuffer {
    lines: VecDeque<String>,
    line_cap: usize,
    byte_cap: usize,
    /// Total number of lines ever pushed, including evicted ones.
    /// Used by tests and metrics; not currently surfaced to users
    /// but handy for "did the buffer overflow?" debugging.
    total_pushed: u64,
    /// Total number of lines evicted due to the line cap.
    /// `total_evicted == total_pushed - lines.len()` always.
    total_evicted: u64,
}

impl BoundedLineBuffer {
    /// Create a buffer with the given caps.
    #[must_use]
    pub fn new(line_cap: usize, byte_cap: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(line_cap.min(1024)),
            line_cap,
            byte_cap,
            total_pushed: 0,
            total_evicted: 0,
        }
    }

    /// Create a buffer for the raw-output use case (large line cap).
    #[must_use]
    pub fn for_raw_output() -> Self {
        Self::new(DEFAULT_LINE_CAP_RAW, DEFAULT_LINE_BYTE_CAP)
    }

    /// Create a buffer for the error-message use case (tighter cap).
    #[must_use]
    pub fn for_errors() -> Self {
        Self::new(DEFAULT_LINE_CAP_ERRORS, DEFAULT_LINE_BYTE_CAP)
    }

    /// Push a line into the buffer. Oversize lines are truncated;
    /// when the line cap is reached, the oldest line is evicted.
    pub fn push(&mut self, line: String) {
        let bounded_line = if line.len() > self.byte_cap {
            // Truncate at the byte cap, but back off to a UTF-8
            // boundary so we don't produce invalid String content.
            // `floor_char_boundary` is unstable on stable Rust as of
            // 1.95, so we hand-roll it: walk back at most 4 bytes
            // (max UTF-8 char length) to find a valid boundary.
            let mut cut = self.byte_cap;
            while cut > 0 && !line.is_char_boundary(cut) {
                cut -= 1;
            }
            let original_len = line.len();
            let mut truncated = line[..cut].to_string();
            truncated.push_str(&format!(
                " …(truncated, {original_len} bytes total)"
            ));
            truncated
        } else {
            line
        };

        if self.lines.len() >= self.line_cap {
            self.lines.pop_front();
            self.total_evicted += 1;
        }
        self.lines.push_back(bounded_line);
        self.total_pushed += 1;
    }

    /// Snapshot the current buffer contents as a `Vec<String>`.
    /// Cloned — caller can drop the buffer without invalidating the
    /// snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }

    /// Get the last line, if any.
    #[must_use]
    pub fn last(&self) -> Option<&String> {
        self.lines.back()
    }

    /// Get the number of lines currently retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// `true` when no lines have been retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Total number of lines pushed (including evicted ones).
    #[must_use]
    pub fn total_pushed(&self) -> u64 {
        self.total_pushed
    }

    /// Total number of lines evicted by the line cap.
    /// Non-zero indicates the buffer overflowed at some point.
    #[must_use]
    pub fn total_evicted(&self) -> u64 {
        self.total_evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_below_cap_retains_all_lines() {
        let mut buf = BoundedLineBuffer::new(10, 1024);
        for i in 0..5 {
            buf.push(format!("line {i}"));
        }
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.total_pushed(), 5);
        assert_eq!(buf.total_evicted(), 0);
        let snap = buf.snapshot();
        assert_eq!(snap[0], "line 0");
        assert_eq!(snap[4], "line 4");
    }

    #[test]
    fn push_past_cap_evicts_oldest() {
        let mut buf = BoundedLineBuffer::new(3, 1024);
        for i in 0..5 {
            buf.push(format!("line {i}"));
        }
        // Cap = 3. After pushing 0..5, only the last 3 (2, 3, 4) remain.
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.total_pushed(), 5);
        assert_eq!(buf.total_evicted(), 2);
        let snap = buf.snapshot();
        assert_eq!(snap[0], "line 2");
        assert_eq!(snap[2], "line 4");
    }

    #[test]
    fn oversize_line_is_truncated_with_marker() {
        let mut buf = BoundedLineBuffer::new(10, 32);
        let huge: String = "x".repeat(100);
        buf.push(huge);
        assert_eq!(buf.len(), 1);
        let snap = buf.snapshot();
        // Truncated prefix + ellipsis marker.
        assert!(snap[0].starts_with("x"));
        assert!(snap[0].contains("…(truncated, 100 bytes total)"));
        // The retained line is shorter than the original 100 bytes.
        assert!(snap[0].len() < 100);
    }

    #[test]
    fn truncation_respects_utf8_boundary() {
        // A 4-byte char ("🦀") right at the byte cap. Truncation
        // must not split it; should back off to a boundary BEFORE
        // the char (since the cap itself falls mid-char).
        let mut buf = BoundedLineBuffer::new(10, 6);
        // 4 bytes of "🦀" + 4 bytes of "🦀" = 8 bytes total; cap is 6
        let s = "🦀🦀".to_string();
        assert_eq!(s.len(), 8);
        buf.push(s);
        let snap = buf.snapshot();
        // The retained string is valid UTF-8 (Rust's String invariant
        // — this would panic on the to_string() if not).
        assert!(snap[0].contains("…(truncated, 8 bytes total)"));
    }

    #[test]
    fn last_returns_most_recent() {
        let mut buf = BoundedLineBuffer::new(10, 1024);
        buf.push("a".into());
        buf.push("b".into());
        buf.push("c".into());
        assert_eq!(buf.last().map(String::as_str), Some("c"));
    }

    #[test]
    fn empty_after_construction() {
        let buf = BoundedLineBuffer::new(10, 1024);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.last().is_none());
    }

    #[test]
    fn for_raw_output_uses_documented_cap() {
        let buf = BoundedLineBuffer::for_raw_output();
        // Construction should reflect the public DEFAULT_LINE_CAP_RAW.
        // We can't read `line_cap` directly, but we can verify by
        // pushing N+1 and observing the eviction.
        let mut buf = buf;
        for i in 0..(DEFAULT_LINE_CAP_RAW + 1) {
            buf.push(format!("{i}"));
        }
        assert_eq!(buf.len(), DEFAULT_LINE_CAP_RAW);
        assert_eq!(buf.total_evicted(), 1);
    }

    #[test]
    fn for_errors_uses_tighter_cap() {
        let mut buf = BoundedLineBuffer::for_errors();
        for i in 0..(DEFAULT_LINE_CAP_ERRORS + 5) {
            buf.push(format!("err {i}"));
        }
        assert_eq!(buf.len(), DEFAULT_LINE_CAP_ERRORS);
        assert_eq!(buf.total_evicted(), 5);
    }
}
