// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Persistent on-disk activity log writer (#541).
// ================================================
//
// Mirrors every `ActivityLogEvent` emitted through `utils::activity_log`
// to a daily-rotating, append-only text file at
// `{app_data_dir}/logs/activity-YYYY-MM-DD.log`.
//
// Design goals
// ------------
// * **Never block the emit hot path.** Producers push events onto an
//   `UnboundedSender` and return immediately. All disk I/O happens in a
//   single background task.
// * **No memory leak under burst.** `BufWriter` caps in-memory state at
//   the writer's internal buffer. The channel is unbounded but drained
//   every 500 ms by the flush tick.
// * **Graceful shutdown.** The writer polls the shared `ShutdownSignal`
//   and flushes + exits on trigger, so the final events from the tail
//   end of a session are not lost.
// * **UTC date rollover.** At write time we check if the event's UTC
//   date differs from the currently-open file's date, and transparently
//   reopen the next day's file.
//
// Line format
// -----------
//
// ```text
// 2026-04-22T14:03:21.482Z  [dl_abc123] [internal] Track 03/14 — downloading
// 2026-04-22T14:03:22.001Z  [system   ] [stderr  ] [VERBOSE] gamdl: …
// ```
//
// The pipe-free, space-padded layout is grep-friendly and lines up
// vertically when viewed in a plain-text editor.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::utils::activity_log::ActivityLogEvent;

/// Retention window for `activity-*.log` files. Matches tracing log
/// retention so users have a consistent 7-day diagnostic window.
pub const ACTIVITY_LOG_RETENTION_DAYS: u64 = 7;

/// Flush cadence for the `BufWriter`. 500 ms keeps the file reasonably
/// up-to-date for live tailing without generating one syscall per event.
const FLUSH_INTERVAL: Duration = Duration::from_millis(500);

/// Handle registered as Tauri managed state. Clones cheaply and sends
/// events across threads without blocking.
#[derive(Clone)]
pub struct ActivityLogWriterHandle {
    tx: UnboundedSender<ActivityLogEvent>,
}

impl ActivityLogWriterHandle {
    /// Queues an event for the background writer. Non-blocking; fails
    /// only when the receiver has been dropped (i.e., during shutdown).
    /// Send failures are logged at `debug!` and discarded — we never
    /// interrupt a download because a log write couldn't be buffered.
    pub fn send(&self, event: ActivityLogEvent) {
        if let Err(e) = self.tx.send(event) {
            log::debug!("activity_log_writer: send failed (receiver dropped): {e}");
        }
    }
}

/// Spawns the background writer task and returns a handle that can be
/// cloned into managed state or shared via `utils::activity_log::register_disk_writer`.
///
/// The task runs until `shutdown.is_triggered()` returns true (polled
/// on every select iteration) or the sender half is dropped.
pub fn start(
    log_dir: PathBuf,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> ActivityLogWriterHandle {
    let (tx, rx) = unbounded_channel::<ActivityLogEvent>();

    // Ensure the logs directory exists before the writer task starts.
    // Failure here is non-fatal — the task will retry on first write.
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::warn!(
            "activity_log_writer: failed to pre-create log dir {}: {e}",
            log_dir.display()
        );
    }

    tauri::async_runtime::spawn(async move {
        writer_task(log_dir, rx, shutdown).await;
    });

    ActivityLogWriterHandle { tx }
}

/// The background writer task.
///
/// Owns a `BufWriter<File>` anchored to today's UTC date. On date
/// rollover (first event written on a new UTC day) the file is flushed,
/// closed, and replaced. Flushing also happens every `FLUSH_INTERVAL`
/// tick and once more at shutdown.
async fn writer_task(
    log_dir: PathBuf,
    mut rx: UnboundedReceiver<ActivityLogEvent>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut state: Option<OpenFile> = None;
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    // Skip the immediate tick so we don't flush before the first event.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;

    log::info!(
        "activity_log_writer: started, writing to {} (retention {}d)",
        log_dir.display(),
        ACTIVITY_LOG_RETENTION_DAYS
    );

    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        tokio::select! {
            maybe_event = rx.recv() => {
                match maybe_event {
                    Some(event) => {
                        if let Err(e) = write_event(&log_dir, &mut state, &event) {
                            log::warn!("activity_log_writer: write failed: {e}");
                        }
                    }
                    None => {
                        // Sender dropped — no more events will arrive.
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                if let Some(s) = state.as_mut() {
                    let _ = s.writer.flush();
                }
            }
        }
    }

    // Drain any events already in the queue before exiting.
    while let Ok(event) = rx.try_recv() {
        let _ = write_event(&log_dir, &mut state, &event);
    }
    if let Some(mut s) = state {
        let _ = s.writer.flush();
    }
    log::info!("activity_log_writer: stopped");
}

/// State for the currently-open log file.
struct OpenFile {
    date: String, // YYYY-MM-DD (UTC)
    writer: BufWriter<std::fs::File>,
}

/// Appends a single event to the on-disk log, rolling over the file
/// if the event's UTC date has advanced past the open file's date.
fn write_event(
    log_dir: &Path,
    state: &mut Option<OpenFile>,
    event: &ActivityLogEvent,
) -> std::io::Result<()> {
    // Derive the UTC date from the event's ISO 8601 timestamp so the
    // filename matches the entries' own timestamps (no off-by-one at
    // local midnight).
    let event_date = event
        .timestamp
        .get(..10)
        .filter(|d| d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-')
        .unwrap_or("")
        .to_string();

    // Fall back to "today" if the timestamp is malformed for any reason.
    let date = if event_date.is_empty() {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    } else {
        event_date
    };

    // Rollover: close the old file and open the new one.
    let need_open = match state {
        Some(s) => s.date != date,
        None => true,
    };
    if need_open {
        if let Some(s) = state.as_mut() {
            let _ = s.writer.flush();
        }
        let path = log_dir.join(format!("activity-{date}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        *state = Some(OpenFile {
            date,
            writer: BufWriter::new(file),
        });
    }

    // Safe: we just assigned Some(...) above.
    let Some(s) = state.as_mut() else {
        return Ok(());
    };

    // Pad the id and stream columns so lines align in a text editor.
    // Download IDs are 8 chars in the UI; pad to 10 to also accommodate
    // the "system" sentinel.
    let id_column = if event.download_id.len() >= 10 {
        event.download_id[..10].to_string()
    } else {
        format!("{:<10}", event.download_id)
    };
    let stream_column = format!("{:<8}", event.stream);

    writeln!(
        s.writer,
        "{}  [{}] [{}] {}",
        event.timestamp, id_column, stream_column, event.line
    )?;

    Ok(())
}

/// Counts and returns the paths of existing `activity-*.log` files
/// within the given directory, sorted newest-first by filename (which
/// matches date ordering). Used by the export command.
pub fn list_log_files(log_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            if name.starts_with("activity-") && name.ends_with(".log") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    // Sort descending by filename (latest date first).
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_log_files_filters_and_sorts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("activity-2026-04-22.log"), "").unwrap();
        std::fs::write(dir.join("activity-2026-04-20.log"), "").unwrap();
        std::fs::write(dir.join("activity-2026-04-21.log"), "").unwrap();
        std::fs::write(dir.join("meedyadl.2026-04-22.log"), "").unwrap();
        std::fs::write(dir.join("session-2026-04-22.log"), "").unwrap();

        let files = list_log_files(dir);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            names,
            vec![
                "activity-2026-04-22.log".to_string(),
                "activity-2026-04-21.log".to_string(),
                "activity-2026-04-20.log".to_string(),
            ]
        );
    }

    #[test]
    fn write_event_creates_dated_file_and_formats_line() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let mut state: Option<OpenFile> = None;

        let event = ActivityLogEvent {
            download_id: "abc12345".to_string(),
            stream: "internal",
            line: "hello world".to_string(),
            timestamp: "2026-04-22T14:03:21.482Z".to_string(),
        };

        write_event(dir, &mut state, &event).unwrap();

        // Flush for the assertion below.
        if let Some(s) = state.as_mut() {
            s.writer.flush().unwrap();
        }

        let path = dir.join("activity-2026-04-22.log");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("2026-04-22T14:03:21.482Z"));
        assert!(contents.contains("[abc12345  ]"));
        assert!(contents.contains("[internal]"));
        assert!(contents.contains("hello world"));
    }

    #[test]
    fn write_event_rolls_over_on_date_change() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let mut state: Option<OpenFile> = None;

        let first = ActivityLogEvent {
            download_id: "system".to_string(),
            stream: "internal",
            line: "day one".to_string(),
            timestamp: "2026-04-22T23:59:59.999Z".to_string(),
        };
        let second = ActivityLogEvent {
            download_id: "system".to_string(),
            stream: "internal",
            line: "day two".to_string(),
            timestamp: "2026-04-23T00:00:00.001Z".to_string(),
        };

        write_event(dir, &mut state, &first).unwrap();
        write_event(dir, &mut state, &second).unwrap();

        if let Some(s) = state.as_mut() {
            s.writer.flush().unwrap();
        }

        let day_one = std::fs::read_to_string(dir.join("activity-2026-04-22.log")).unwrap();
        let day_two = std::fs::read_to_string(dir.join("activity-2026-04-23.log")).unwrap();
        assert!(day_one.contains("day one"));
        assert!(!day_one.contains("day two"));
        assert!(day_two.contains("day two"));
        assert!(!day_two.contains("day one"));
    }
}
