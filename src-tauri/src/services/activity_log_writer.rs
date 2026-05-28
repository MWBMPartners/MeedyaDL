// Copyright (c) 2026 MeedyaSuite
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

use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::utils::activity_log::ActivityLogEvent;

/// Retention window for `activity-*.log` files. Matches tracing log
/// retention so users have a consistent 7-day diagnostic window.
pub const ACTIVITY_LOG_RETENTION_DAYS: u64 = 7;

/// Flush cadence for the `BufWriter`. 500 ms keeps the file reasonably
/// up-to-date for live tailing without generating one syscall per event.
const FLUSH_INTERVAL: Duration = Duration::from_millis(500);

/// Channel capacity for the activity-log writer (#896).
///
/// Sized for sustained verbose-mode bursts: a heavy multi-album
/// download can emit ~1-5k events/sec; the writer flushes every
/// 500 ms (`FLUSH_INTERVAL`), so the steady-state queue depth is
/// well under 5k. The 10k cap is a 2× safety margin over realistic
/// workloads, but small enough that a slow-disk pathological case
/// (FUSE mount blocked for 30s+) bounds total queued memory to
/// roughly `CHANNEL_CAPACITY × sizeof(ActivityLogEvent)` ≈ 2 MB
/// rather than unbounded growth.
const CHANNEL_CAPACITY: usize = 10_000;

/// Atomic counter of activity-log events dropped because the
/// writer channel was full (#896). Surfaced for diagnostic
/// purposes — non-zero values indicate the writer task is falling
/// behind the emit-side, usually due to slow-disk I/O.
static DROPPED_EVENTS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Snapshot of the dropped-event counter. Test-friendly; exposed
/// so an observability surface (status bar / settings advanced
/// diagnostics) could surface the number.
#[must_use]
pub fn dropped_event_count() -> u64 {
    DROPPED_EVENTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Handle registered as Tauri managed state. Clones cheaply and sends
/// events across threads without blocking.
#[derive(Clone)]
pub struct ActivityLogWriterHandle {
    tx: Sender<ActivityLogEvent>,
}

impl ActivityLogWriterHandle {
    /// Queues an event for the background writer. Non-blocking via
    /// [`Sender::try_send`] (#896): if the channel is full
    /// (consumer falling behind), the event is **dropped silently
    /// after bumping the dropped-event counter** rather than
    /// blocking the emit-side caller. The on-disk activity log is a
    /// forensic / debugging surface — losing some events under
    /// disk-pressure is preferable to back-pressuring the entire
    /// emit-side and slowing down user-facing downloads.
    ///
    /// Receiver-dropped failures (shutdown path) are also logged
    /// at `debug!` and discarded.
    pub fn send(&self, event: ActivityLogEvent) {
        use tokio::sync::mpsc::error::TrySendError;
        match self.tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                let prev = DROPPED_EVENTS
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                // Log only the first drop in a cluster — by 100, 1000,
                // 10_000 — so a slow disk doesn't generate a flood of
                // log warnings. The counter itself is the source of
                // truth for total drops.
                if prev == 0 || prev == 99 || prev == 999 || prev == 9_999 {
                    log::warn!(
                        "activity_log_writer: channel full, dropped event ({} total drops)",
                        prev + 1
                    );
                }
            }
            Err(TrySendError::Closed(_)) => {
                log::debug!(
                    "activity_log_writer: send failed (receiver dropped — likely shutdown)"
                );
            }
        }
    }
}

/// Spawns the background writer task and returns a handle that can be
/// cloned into managed state or shared via `utils::activity_log::register_disk_writer`.
///
/// The task runs until `shutdown.is_triggered()` returns true (polled
/// on every select iteration) or the sender half is dropped.
///
/// `db_path` (#875 M3) is the path to the SQLite download index. When
/// `Some` AND the file is openable, every event written to the JSONL
/// file is ALSO inserted into the `activity_events` table — the M3
/// dual-write that makes the activity log searchable via SQL. On any
/// DB open / write failure the task logs a debug line and continues
/// writing to JSONL only; the JSONL files remain the forensic record
/// per #541 so DB outage never loses events.
pub fn start(
    log_dir: PathBuf,
    db_path: Option<PathBuf>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> ActivityLogWriterHandle {
    let (tx, rx) = channel::<ActivityLogEvent>(CHANNEL_CAPACITY);

    // Ensure the logs directory exists before the writer task starts.
    // Failure here is non-fatal — the task will retry on first write.
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::warn!(
            "activity_log_writer: failed to pre-create log dir {}: {e}",
            log_dir.display()
        );
    }

    tauri::async_runtime::spawn(async move {
        writer_task(log_dir, db_path, rx, shutdown).await;
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
    db_path: Option<PathBuf>,
    mut rx: Receiver<ActivityLogEvent>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut state: Option<OpenFile> = None;
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    // Skip the immediate tick so we don't flush before the first event.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;

    // #875 M3 — open the SQLite download index once for the task's
    // lifetime. Per-event opens would cost ~1ms × thousands of events;
    // single-open + per-event INSERT keeps overhead negligible. DB
    // failures are NEVER fatal — the JSONL writer keeps running so
    // events never get lost.
    let mut db = match db_path.as_ref() {
        Some(p) => match crate::services::download_index::DownloadIndex::open(p) {
            Ok(idx) => {
                log::info!(
                    "activity_log_writer: dual-writing to DB at {}",
                    p.display()
                );
                Some(idx)
            }
            Err(e) => {
                log::debug!(
                    "activity_log_writer: DB open at {} failed (continuing JSONL-only): {e}",
                    p.display()
                );
                None
            }
        },
        None => None,
    };

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
                        if let Some(idx) = db.as_mut() {
                            if let Err(e) = write_event_to_db(idx, &event) {
                                // Non-fatal: log debug and continue. JSONL
                                // already captured the event so nothing
                                // is lost. Subsequent events still try
                                // the DB write.
                                log::debug!(
                                    "activity_log_writer: DB insert failed (event preserved in JSONL): {e}"
                                );
                            }
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
        if let Some(idx) = db.as_mut() {
            let _ = write_event_to_db(idx, &event);
        }
    }
    if let Some(mut s) = state {
        let _ = s.writer.flush();
    }
    log::info!("activity_log_writer: stopped");
}

/// Inserts a single activity-log event into the SQLite
/// `activity_events` table (#875 M3 dual-write). The `severity`
/// enum is mapped to the DB's `level` text column; the
/// `download_id` is normalised to either the SYSTEM sentinel or the
/// concrete ID; the `source` column carries the same routing
/// information as the JSONL `download_id` field for searchability.
fn write_event_to_db(
    idx: &mut crate::services::download_index::DownloadIndex,
    event: &crate::utils::activity_log::ActivityLogEvent,
) -> Result<(), crate::services::download_index::IndexError> {
    let level = match event.severity {
        crate::utils::activity_log::LogSeverity::Info => "INFO",
        crate::utils::activity_log::LogSeverity::Warning => "WARN",
        crate::utils::activity_log::LogSeverity::Error => "ERROR",
    };
    let source = if event.download_id
        == crate::utils::activity_log::SYSTEM_LOG_ID
    {
        "system".to_string()
    } else {
        format!("download:{}", event.download_id)
    };
    idx.conn().execute(
        "INSERT INTO activity_events (timestamp, level, source, message, download_id, metadata_json)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL)",
        rusqlite::params![event.timestamp, level, source, event.line],
    )?;
    Ok(())
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
            severity: crate::utils::activity_log::LogSeverity::Info,
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
            severity: crate::utils::activity_log::LogSeverity::Info,
            line: "day one".to_string(),
            timestamp: "2026-04-22T23:59:59.999Z".to_string(),
        };
        let second = ActivityLogEvent {
            download_id: "system".to_string(),
            stream: "internal",
            severity: crate::utils::activity_log::LogSeverity::Info,
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

    #[test]
    fn write_event_to_db_inserts_and_maps_fields_correctly() {
        // #875 M3: the DB dual-write maps `LogSeverity` → `level`
        // text column, prefixes non-system download IDs with
        // `download:`, and routes the JSONL `line` into the
        // `message` column.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let mut idx = crate::services::download_index::DownloadIndex::open(&db_path).unwrap();

        // Two events: one system, one per-download. Warning severity
        // for the second so we exercise the LogSeverity → "WARN"
        // mapping branch as well as Info → "INFO".
        let sys = ActivityLogEvent {
            download_id: crate::utils::activity_log::SYSTEM_LOG_ID.to_string(),
            stream: "internal",
            severity: crate::utils::activity_log::LogSeverity::Info,
            line: "system event".to_string(),
            timestamp: "2026-05-25T10:00:00Z".to_string(),
        };
        let dl = ActivityLogEvent {
            download_id: "abc12345".to_string(),
            stream: "stderr",
            severity: crate::utils::activity_log::LogSeverity::Warning,
            line: "warning from a download".to_string(),
            timestamp: "2026-05-25T10:00:01Z".to_string(),
        };
        write_event_to_db(&mut idx, &sys).unwrap();
        write_event_to_db(&mut idx, &dl).unwrap();

        // Inspect: 2 rows, mapped levels + sources match expectations.
        let mut stmt = idx
            .conn()
            .prepare("SELECT timestamp, level, source, message FROM activity_events ORDER BY id")
            .unwrap();
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1, "INFO");
        assert_eq!(rows[0].2, "system");
        assert_eq!(rows[0].3, "system event");
        assert_eq!(rows[1].1, "WARN");
        assert_eq!(rows[1].2, "download:abc12345");
        assert_eq!(rows[1].3, "warning from a download");
    }

    // ============================================================
    // #896 bounded-channel + try_send tests
    // ============================================================

    /// Helper: build a sample event. The content doesn't matter for
    /// these tests — only that `send` succeeds or fails.
    fn sample_event(line: &str) -> ActivityLogEvent {
        ActivityLogEvent {
            download_id: "system".to_string(),
            stream: "internal",
            severity: crate::utils::activity_log::LogSeverity::Info,
            line: line.to_string(),
            timestamp: "2026-05-28T00:00:00.000Z".to_string(),
        }
    }

    /// When the channel is well below capacity, `send` queues every
    /// event. We assert the contract via the receiver count rather
    /// than `dropped_event_count()` because the latter is a
    /// process-global counter that other tests in the same suite
    /// can bump in parallel — racy by design, not useful here.
    #[tokio::test(flavor = "current_thread")]
    async fn send_within_capacity_does_not_drop() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ActivityLogEvent>(16);
        let handle = ActivityLogWriterHandle { tx };
        for i in 0..10 {
            handle.send(sample_event(&format!("line {i}")));
        }
        // Drain — all 10 should be queued.
        let mut received = 0;
        while let Ok(_evt) = rx.try_recv() {
            received += 1;
        }
        assert_eq!(received, 10, "all 10 events should fit in a 16-cap channel");
    }

    /// When the channel is FULL (consumer not draining), `send`
    /// drops events silently and bumps the global counter.
    #[tokio::test(flavor = "current_thread")]
    async fn send_when_full_drops_and_increments_counter() {
        // Capacity 2 — we deliberately never drain so the third
        // send hits the Full branch.
        let (tx, _rx_held) = tokio::sync::mpsc::channel::<ActivityLogEvent>(2);
        let handle = ActivityLogWriterHandle { tx };
        let before = dropped_event_count();

        handle.send(sample_event("a")); // fits
        handle.send(sample_event("b")); // fits
        handle.send(sample_event("c")); // dropped — channel full
        handle.send(sample_event("d")); // dropped — channel full

        let drops_after = dropped_event_count();
        assert!(
            drops_after >= before + 2,
            "expected at least 2 drops; before={before}, after={drops_after}"
        );
    }

    /// When the receiver has been dropped (e.g. writer task exited
    /// during shutdown), `send` must NOT panic. The drop counter is
    /// process-global and shared with other parallel tests, so we
    /// can't assert it stayed exactly the same — we only assert
    /// the no-panic contract here. The "receiver-dropped doesn't
    /// bump the channel-full counter" rule is enforced by the
    /// implementation (the `TrySendError::Closed` arm doesn't
    /// touch the counter); reviewing the code is sufficient since
    /// no test could reliably observe that without serialising
    /// access to the global counter.
    #[tokio::test(flavor = "current_thread")]
    async fn send_after_receiver_dropped_does_not_panic() {
        let (tx, rx) = tokio::sync::mpsc::channel::<ActivityLogEvent>(8);
        let handle = ActivityLogWriterHandle { tx };
        drop(rx); // Simulate writer-task shutdown.
        handle.send(sample_event("after-shutdown")); // must not panic
    }

    #[test]
    fn dropped_event_count_is_monotonic() {
        // The counter is process-global. We can't assert an exact
        // value (other tests may have bumped it), only that
        // subsequent reads don't decrease.
        let a = dropped_event_count();
        let b = dropped_event_count();
        assert!(b >= a);
    }
}
