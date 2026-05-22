// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Companion-download supervisor.
// ==============================
//
// Wraps a single GAMDL child process for a companion-download tier with
// the safety nets the bare `cmd.spawn() + child.wait()` flow lacks:
//
//   - **Soft-error detection (#500)** — GAMDL exits 0 when a per-track
//     download throws an unhandled exception. We tail stdout/stderr,
//     parse `Finished with N error(s)`, and classify known Python
//     tracebacks so callers can downgrade an "exit 0" to a real failure
//     and surface a single user-friendly activity-log line instead of a
//     raw traceback.
//
//   - **Idle watchdog (#505)** — kills the child if no stdout/stderr
//     line arrives for `idle_timeout`. Defaults are tuned so a healthy
//     post-processing pause (#503) doesn't trip the killswitch.
//
//   - **Post-processing indicator (#503)** — once the HLS download
//     reaches 100% the supervisor flips an "is post-processing" flag,
//     suppresses the idle killswitch, and returns the flag to the
//     caller so the queue UI can show `PROCESSING…` while
//     mp4decrypt / ffmpeg / mp4box are running silently.
//
//   - **Kill-on-drop (#501)** — `Command::kill_on_drop(true)` is set so
//     when the supervising tokio task is aborted (e.g., the queue's
//     10-minute completion timeout), the GAMDL child is reaped instead
//     of leaking as a zombie.
//
// The supervisor is intentionally narrow: it only owns the child
// process and the IO readers. Tier orchestration, codec planning,
// activity-log emission of progress lines, and metadata tagging stay
// in `download_queue.rs` and `services/`.

use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tauri::AppHandle;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};

use crate::utils::process::{classify_gamdl_traceback, parse_gamdl_error_count};

// ============================================================
// Public types
// ============================================================

/// Result of supervising a single GAMDL invocation.
pub struct CompanionRunResult {
    /// Did the child exit with status code 0?
    pub exit_success: bool,
    /// Did GAMDL print `Finished with N error(s)` with N > 0?
    /// Callers should treat this as a failure even when `exit_success`
    /// is true (GAMDL silently absorbs per-track exceptions and exits 0).
    pub had_soft_error: bool,
    /// True when the idle watchdog killed the child.
    pub idle_killed: bool,
    /// True when the supervisor saw a `100% of` line on stdout, meaning
    /// the HLS download reached completion and we entered the silent
    /// remux/decrypt phase. Callers can use this to distinguish
    /// "post-processing took a while" from "GAMDL hung early".
    pub reached_post_processing: bool,
    /// User-facing translation of a recognised Python traceback (e.g.,
    /// `NoneType.audio_track`). Prefer this over `last_stderr_line` for
    /// activity-log surfacing when present.
    pub friendly_error: Option<String>,
    /// Last `\r`-coalesced stderr line, for fallback error display.
    pub last_stderr_line: String,
}

/// Boxed-future return type for the [`LineEmitter`] callback.
type EmitFuture = Pin<Box<dyn Future<Output = Option<String>> + Send>>;

/// Hook callback signature: invoked for each line that survives
/// `\r`-coalescing. Returns the cleaned representation of the line so
/// the supervisor can record the most recent one for error surfacing.
///
/// Arguments to the callback are `(app, dl_id, stream_label, line)`.
pub type LineEmitter =
    Arc<dyn Fn(AppHandle, String, String, String) -> EmitFuture + Send + Sync>;

// ============================================================
// Public entry point
// ============================================================

/// Spawns the supplied command, watches its IO, and returns a
/// `CompanionRunResult` describing how the run terminated.
///
/// `cmd` should already be configured with `stdout(Stdio::piped())` and
/// `stderr(Stdio::piped())` by the caller; we apply `kill_on_drop(true)`
/// here so all callers get the same safety guarantee.
///
/// `idle_timeout` is the maximum permitted span between consecutive
/// stdout/stderr lines while the supervisor is in the "downloading"
/// phase. The watchdog pauses automatically once a `100% of` line is
/// observed (post-processing is silent by design).
///
/// `emit_line` is invoked for every line; the returned `Option<String>`
/// is recorded as `last_stderr_line` when the stream label contains
/// "stderr" (caller usually strips ANSI codes inside the closure).
pub async fn run_supervised(
    app: &AppHandle,
    dl_id: &str,
    stream_label: &str,
    mut cmd: Command,
    idle_timeout: Duration,
    emit_line: LineEmitter,
) -> Result<CompanionRunResult, String> {
    cmd.kill_on_drop(true);

    let mut child: Child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Shared state between IO readers and the watchdog poll loop.
    //
    // - `last_activity_ms` is updated on every line so the watchdog can
    //   measure idle time. Atomic so the readers don't need a mutex on
    //   the hot path.
    // - `post_processing` flips to true once we observe a `100% of`
    //   line; the watchdog stands down (post-processing is silent by
    //   design and may take minutes on slow disks).
    // - `accumulated_*` capture full output for soft-error parsing
    //   after the child exits. Bounded by GAMDL's per-track verbosity
    //   (typically <100 KB).
    let last_activity_ms = Arc::new(AtomicU64::new(now_ms()));
    let post_processing = Arc::new(AtomicBool::new(false));
    let accumulated_stdout: Arc<tokio::sync::Mutex<String>> = Arc::default();
    let accumulated_stderr: Arc<tokio::sync::Mutex<String>> = Arc::default();

    let stdout_task = stdout.map(|out| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        let stream = format!("{stream_label}-stdout");
        let emit = emit_line.clone();
        let last_activity = last_activity_ms.clone();
        let post_proc = post_processing.clone();
        let acc = accumulated_stdout.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                last_activity.store(now_ms(), Ordering::Relaxed);
                if line_signals_post_processing(&line) {
                    post_proc.store(true, Ordering::Relaxed);
                }
                {
                    let mut buf = acc.lock().await;
                    buf.push_str(&line);
                    buf.push('\n');
                }
                let _ = emit(app.clone(), dl_id.clone(), stream.clone(), line).await;
            }
        })
    });

    let stderr_task = stderr.map(|err| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        let stream = format!("{stream_label}-stderr");
        let emit = emit_line.clone();
        let last_activity = last_activity_ms.clone();
        let acc = accumulated_stderr.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(err);
            let mut lines = reader.lines();
            let mut last_clean = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                last_activity.store(now_ms(), Ordering::Relaxed);
                {
                    let mut buf = acc.lock().await;
                    buf.push_str(&line);
                    buf.push('\n');
                }
                if let Some(clean) = emit(app.clone(), dl_id.clone(), stream.clone(), line).await {
                    last_clean = clean;
                }
            }
            last_clean
        })
    });

    // Idle watchdog poll loop.
    //
    // We deliberately avoid `child.wait()` because it would borrow
    // `&mut child` for the lifetime of the future, preventing us from
    // calling `child.start_kill()` from the same task. Instead we poll
    // `try_wait()` every 200 ms. That's plenty responsive for downloads
    // that take seconds-to-minutes per track.
    let idle_ms = u64::try_from(idle_timeout.as_millis()).unwrap_or(u64::MAX);
    let mut watchdog_killed = false;
    let exit_status = loop {
        tokio::time::sleep(Duration::from_millis(200)).await;

        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {} // still running
            Err(e) => break Err(e),
        }

        if post_processing.load(Ordering::Relaxed) {
            continue;
        }
        let elapsed = now_ms().saturating_sub(last_activity_ms.load(Ordering::Relaxed));
        if elapsed >= idle_ms {
            let _ = child.start_kill();
            watchdog_killed = true;
            // Reap the child so we don't leak the handle.
            break child.wait().await;
        }
    };

    // Drain readers so all output is captured and emitted before we
    // classify the run.
    if let Some(t) = stdout_task {
        let _ = t.await;
    }
    let last_stderr_line = if let Some(t) = stderr_task {
        t.await.unwrap_or_default()
    } else {
        String::new()
    };

    let exit_success = matches!(exit_status, Ok(s) if s.success());

    let stdout_text = accumulated_stdout.lock().await.clone();
    let stderr_text = accumulated_stderr.lock().await.clone();
    let combined = format!("{stdout_text}\n{stderr_text}");

    let error_count = parse_gamdl_error_count(&combined).unwrap_or(0);
    let friendly = classify_gamdl_traceback(&combined).map(std::string::ToString::to_string);

    Ok(CompanionRunResult {
        exit_success,
        had_soft_error: error_count > 0,
        idle_killed: watchdog_killed,
        reached_post_processing: post_processing.load(Ordering::Relaxed),
        friendly_error: friendly,
        last_stderr_line,
    })
}

// ============================================================
// Helpers
// ============================================================

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Detects the "we're done downloading, now we're remuxing/decrypting"
/// transition. yt-dlp's final progress line always contains `100% of`;
/// `[Remuxing]`, `[Decrypting]` and `Muxing` cover the other tools
/// GAMDL pipelines into.
fn line_signals_post_processing(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("100% of") || lower.contains("100.0% of") {
        return true;
    }
    lower.contains("[remux") || lower.contains("[decrypt") || lower.contains("muxing")
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signals_post_processing_for_ytdlp_finish() {
        assert!(line_signals_post_processing(
            "[download] 100% of ~ 5.04MiB at 8.7MiB/s"
        ));
    }

    #[test]
    fn signals_post_processing_for_remux_keyword() {
        assert!(line_signals_post_processing("[Remuxing] PSYCHO.m4a"));
    }

    #[test]
    fn does_not_signal_for_partial_progress() {
        assert!(!line_signals_post_processing(
            "[download]  87.4% of ~  4.41MiB at  6.48MiB/s ETA 00:01"
        ));
    }
}
