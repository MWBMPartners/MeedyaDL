// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

//! Per-file write-coordination locks (#779 Option 2).
//!
//! When two enrichment stages (e.g. AcoustID + ReplayGain) write
//! freeform atoms to the same audio file via `mp4ameta::Tag::write_to_path`,
//! they conflict at the byte level: each call reads the existing
//! tag set, mutates a clone in memory, and rewrites the whole tag
//! atom back to disk. Two concurrent calls race — last write wins,
//! losing the other stage's atoms.
//!
//! `#779 Option 1` (already shipped) sidestepped this by running
//! `AcoustID || MusicBrainz lookup` in parallel and keeping
//! `ReplayGain` sequential (only MusicBrainz lookup doesn't touch
//! audio files). The conservative win was ~30% wall-clock.
//!
//! `#779 Option 2` (this module) unlocks the full parallel-stage
//! win — `AcoustID || MusicBrainz lookup || ReplayGain` all
//! concurrently — by adding a per-file lock map. Both
//! file-writing stages acquire the lock for the file they're
//! about to touch; collisions serialise at the granularity of a
//! single file's write, which is fast (mp4ameta `write_to_path`
//! is millisecond-scale; the slow part is the ffmpeg / chromaprint
//! analysis BEFORE the write).
//!
//! Locks live in an `Arc<Mutex<HashMap<…>>>` so the lock-map
//! itself is shared across stages; an entry is created lazily on
//! first access and lives until the `FileWriteLocks` is dropped.
//! For album-sized workloads (≤ 100 files) the memory cost is
//! negligible.
//!
//! ## Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! # async fn demo() {
//! let locks = Arc::new(meedyadl::utils::file_locks::FileWriteLocks::new());
//!
//! // Inside one stage's per-file iteration:
//! let guard = locks.lock(std::path::Path::new("/tmp/track.m4a")).await;
//! // … perform mp4ameta::Tag::write_to_path here …
//! drop(guard);
//! # }
//! ```
//!
//! Pass `Option<&Arc<FileWriteLocks>>` into per-file processors so
//! callers that don't need coordination (single-stage standalone
//! tests, future use cases) can pass `None` and skip the lock
//! entirely.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

/// Shared per-file lock map. Held inside an `Arc` so the same
/// instance can be passed by reference / clone into every stage
/// that needs to coordinate writes.
#[derive(Default)]
pub struct FileWriteLocks {
    /// Outer mutex guards the map structure (insert / lookup); the
    /// inner per-entry mutex guards the actual file. The outer
    /// lock is held only briefly, so contention is bounded by the
    /// number of concurrent stages, not by per-file work.
    map: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl FileWriteLocks {
    /// Construct an empty lock map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire the per-file lock for `path`, creating the entry
    /// lazily on first access. Returns an owned guard so the
    /// caller can release it explicitly (`drop(guard)`) immediately
    /// after the write, without waiting for the lexical scope to
    /// end.
    pub async fn lock(&self, path: &Path) -> OwnedMutexGuard<()> {
        let entry = {
            let mut map = self.map.lock().await;
            map.entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        // Drop the outer-map guard before awaiting the inner lock
        // so the map itself doesn't block other stages picking up
        // their own (different) file locks.
        entry.lock_owned().await
    }

    /// Test-only: number of distinct files the lock map is
    /// tracking. Lets the unit tests assert the entry-creation
    /// behaviour without exposing the inner map.
    #[cfg(test)]
    pub async fn tracked_files(&self) -> usize {
        self.map.lock().await.len()
    }
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// First call creates an entry; second call for the same path
    /// reuses it (no duplicate entry, same mutex).
    #[tokio::test]
    async fn lock_creates_entry_lazily_and_reuses_on_repeat() {
        let locks = FileWriteLocks::new();
        let path = PathBuf::from("/tmp/track-1.m4a");
        assert_eq!(locks.tracked_files().await, 0);

        let g1 = locks.lock(&path).await;
        assert_eq!(locks.tracked_files().await, 1);
        drop(g1);

        let g2 = locks.lock(&path).await;
        assert_eq!(locks.tracked_files().await, 1, "same path → no new entry");
        drop(g2);
    }

    /// Distinct paths get distinct entries.
    #[tokio::test]
    async fn distinct_paths_get_distinct_locks() {
        let locks = FileWriteLocks::new();
        let a = PathBuf::from("/tmp/track-a.m4a");
        let b = PathBuf::from("/tmp/track-b.m4a");
        let _ga = locks.lock(&a).await;
        let _gb = locks.lock(&b).await;
        assert_eq!(locks.tracked_files().await, 2);
    }

    /// The lock genuinely serialises — a second `lock()` for the
    /// same path waits until the first guard is dropped.
    #[tokio::test]
    async fn second_lock_for_same_path_serialises_behind_first() {
        let locks = Arc::new(FileWriteLocks::new());
        let path = PathBuf::from("/tmp/track-shared.m4a");
        let order = Arc::new(Mutex::new(Vec::<u8>::new()));

        let locks_a = locks.clone();
        let path_a = path.clone();
        let order_a = order.clone();
        let task_a = tokio::spawn(async move {
            let _g = locks_a.lock(&path_a).await;
            order_a.lock().await.push(1);
            // Hold the guard for a moment so task_b lines up
            // behind us.
            tokio::time::sleep(Duration::from_millis(50)).await;
            order_a.lock().await.push(2);
        });

        // Give task_a a head start so it definitely grabs the lock
        // first; task_b then arrives and blocks.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let locks_b = locks.clone();
        let path_b = path.clone();
        let order_b = order.clone();
        let task_b = tokio::spawn(async move {
            let _g = locks_b.lock(&path_b).await;
            order_b.lock().await.push(3);
        });

        let _ = tokio::join!(task_a, task_b);

        let final_order = order.lock().await.clone();
        // Task A must record both 1 and 2 before task B records 3.
        assert_eq!(
            final_order,
            vec![1, 2, 3],
            "task_b's push must be serialised after task_a releases the guard"
        );
    }

    /// Distinct paths run truly in parallel — the second lock
    /// doesn't wait for the first because they're different files.
    #[tokio::test]
    async fn distinct_paths_run_concurrently() {
        let locks = Arc::new(FileWriteLocks::new());
        let order = Arc::new(Mutex::new(Vec::<u8>::new()));

        let locks_a = locks.clone();
        let order_a = order.clone();
        let task_a = tokio::spawn(async move {
            let _g = locks_a.lock(Path::new("/tmp/a.m4a")).await;
            order_a.lock().await.push(1);
            tokio::time::sleep(Duration::from_millis(50)).await;
            order_a.lock().await.push(4);
        });

        tokio::time::sleep(Duration::from_millis(10)).await;

        let locks_b = locks.clone();
        let order_b = order.clone();
        let task_b = tokio::spawn(async move {
            let _g = locks_b.lock(Path::new("/tmp/b.m4a")).await;
            order_b.lock().await.push(2);
            tokio::time::sleep(Duration::from_millis(10)).await;
            order_b.lock().await.push(3);
        });

        let _ = tokio::join!(task_a, task_b);

        let final_order = order.lock().await.clone();
        // Task B's push (2, 3) happens DURING task A's sleep,
        // so the recorded order is 1, 2, 3, 4 — proving the
        // distinct-path locks didn't block each other.
        assert_eq!(
            final_order,
            vec![1, 2, 3, 4],
            "distinct paths must not serialise"
        );
    }
}
