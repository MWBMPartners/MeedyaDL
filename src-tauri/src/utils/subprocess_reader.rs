// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Subprocess line-reader helper.
// ==============================
//
// Audit v2 finding #5 surveyed four subprocess reader sites
// (engine_runner stdout+stderr, companion_supervisor stdout+stderr,
// download_queue stdout+stderr — six readers total across three
// services). All share the same outer shell:
//
//     tokio::spawn(async move {
//         let reader = BufReader::new(stream);
//         let mut lines = reader.lines();
//         while let Ok(Some(line)) = lines.next_line().await {
//             // per-line work
//         }
//     })
//
// The per-line work, however, varies wildly:
//
// - **engine_runner** parses the line via `process::parse_gamdl_output`,
//   wraps it in `EngineProgress`, and emits two Tauri events. The
//   stdout and stderr readers are *truly identical* except for the
//   stream label.
//
// - **companion_supervisor** updates an `AtomicU64` watchdog
//   timestamp, sets a `post_processing` `AtomicBool` for stdout-only
//   lines that signal the post-processing phase, accumulates the line
//   into a `Mutex<String>` for forensic capture, and calls a
//   per-call `emit_line` closure that returns an `Option<String>`
//   (used by stderr to remember the last clean error message).
//
// - **download_queue** calls `emit_companion_stream_line` directly
//   on each line; stderr also remembers the last clean line.
//
// Because the per-line work diverges so far, a generic visitor
// signature that satisfies all three sites would have to carry
// state for the watchdog, accumulator, post-processing flag, and
// "last clean" return value — at which point the abstraction
// is bigger than the inline code it replaces.
//
// **What this module does:** provides ONE primitive,
// `spawn_line_reader`, that captures the truly-common shell
// (`BufReader → next_line loop → visitor`). Each callsite that
// adopts it sheds ~5 lines of plumbing. **Migration is opt-in;**
// the engine_runner pair is the natural first adopter (its two
// readers are identical except for stream label, so they collapse
// to one helper called twice). Future engine implementations
// (Votify M9, yt-dlp M10) follow the same shape and adopt the
// primitive from day one.
//
// **What this module deliberately doesn't do:** force the
// companion_supervisor / download_queue sites to migrate. Their
// per-line state (watchdog, mutex accumulator, return value
// threading) wants to live in the same closure as the per-line
// work, not threaded through a generic visitor signature. They
// keep their inline implementations and document the choice
// in-source.

use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

/// Spawn a tokio task that reads `stream` line-by-line and invokes
/// `visitor` for each line. The task exits cleanly when the stream
/// closes (process exit) or when `visitor` panics — read errors
/// (broken pipe, etc.) terminate the loop without re-emitting.
///
/// The visitor closure is `async`, so callers can `.await` Tauri
/// event emission, mutex acquisition, or other async work per line
/// without spawning further tasks.
///
/// Returns the `JoinHandle<()>` so callers can `.await` it after
/// `child.wait()` to drain remaining buffered output before exiting
/// — match the existing pattern at every callsite.
///
/// **Type bounds:** the visitor must be `Send + 'static` so it can
/// be moved into the spawned task. The future it returns must be
/// `Send` for the same reason. Captured state (e.g. an `Arc<Mutex>`
/// accumulator) must be `Send + Sync` accordingly — same constraint
/// as today's inline `tokio::spawn` blocks.
///
/// # Example
///
/// ```ignore
/// let task = spawn_line_reader(stdout, move |line| {
///     let app = app.clone();
///     let dl_id = dl_id.clone();
///     async move {
///         let event = process::parse_gamdl_output(&line);
///         let _ = app.emit("engine-output", &EngineProgress {
///             download_id: dl_id, engine: "gamdl".into(), event,
///         });
///     }
/// });
/// // ... later
/// let _ = task.await;
/// ```
pub fn spawn_line_reader<S, F, Fut>(stream: S, mut visitor: F) -> JoinHandle<()>
where
    S: AsyncRead + Unpin + Send + 'static,
    F: FnMut(String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            visitor(line).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::AsyncWriteExt;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn reads_lines_from_a_stream_until_close() {
        // Use an in-memory duplex pipe so we control both ends.
        let (mut writer, reader) = tokio::io::duplex(64);
        let collected: Arc<Mutex<Vec<String>>> = Arc::default();
        let acc = collected.clone();

        let task = spawn_line_reader(reader, move |line| {
            let acc = acc.clone();
            async move {
                acc.lock().await.push(line);
            }
        });

        writer.write_all(b"first\nsecond\nthird\n").await.unwrap();
        // Closing the writer half signals EOF, so the reader loop exits.
        drop(writer);
        let _ = task.await;

        let lines = collected.lock().await;
        assert_eq!(*lines, vec!["first", "second", "third"]);
    }

    #[tokio::test]
    async fn handles_partial_final_line_without_trailing_newline() {
        // Lines without a trailing \n still surface — tokio's
        // `lines()` yields the final unterminated buffer at EOF.
        let (mut writer, reader) = tokio::io::duplex(64);
        let collected: Arc<Mutex<Vec<String>>> = Arc::default();
        let acc = collected.clone();

        let task = spawn_line_reader(reader, move |line| {
            let acc = acc.clone();
            async move {
                acc.lock().await.push(line);
            }
        });

        writer.write_all(b"unterminated").await.unwrap();
        drop(writer);
        let _ = task.await;

        let lines = collected.lock().await;
        assert_eq!(*lines, vec!["unterminated"]);
    }

    #[tokio::test]
    async fn empty_stream_produces_zero_visitor_calls() {
        let (writer, reader) = tokio::io::duplex(64);
        let collected: Arc<Mutex<Vec<String>>> = Arc::default();
        let acc = collected.clone();

        let task = spawn_line_reader(reader, move |line| {
            let acc = acc.clone();
            async move {
                acc.lock().await.push(line);
            }
        });

        drop(writer); // EOF immediately
        let _ = task.await;

        assert!(collected.lock().await.is_empty());
    }

    #[tokio::test]
    async fn visitor_can_share_mutable_state_across_invocations() {
        // Mirrors the companion_supervisor "accumulator" pattern —
        // proves the helper supports the same caller-owned state
        // that the inline blocks use today.
        let (mut writer, reader) = tokio::io::duplex(64);
        let count: Arc<Mutex<usize>> = Arc::default();
        let count_clone = count.clone();

        let task = spawn_line_reader(reader, move |_line| {
            let c = count_clone.clone();
            async move {
                *c.lock().await += 1;
            }
        });

        writer.write_all(b"a\nb\nc\nd\ne\n").await.unwrap();
        drop(writer);
        let _ = task.await;

        assert_eq!(*count.lock().await, 5);
    }
}
