// Copyright (c) 2024-2026 MeedyaSuite
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
//! ## One lock map for the whole app (#1442)
//!
//! Every write that rewrites an existing music or video file's tags
//! takes the lock for that file from ONE map shared by the whole
//! process, reached through [`global()`]. Downloads overlap more than
//! the one-item-at-a-time queue suggests: a failed download's extra
//! copies run on their own, the next queued item can be the same
//! album folder, and actions such as a Library re-download run beside
//! a running download. A map made per download would not see any of
//! those, so the lock is only worth having if it is the same map
//! everywhere.
//!
//! The rules for using it:
//!
//! * Take the lock where the loop over files is, never inside the
//!   function that writes one file. The lock is not re-entrant: a
//!   write function that locked for itself, called from a loop that
//!   already holds the lock for that file, would wait for itself.
//! * In async code use [`FileWriteLocks::lock`] and `.await` it.
//! * Use [`FileWriteLocks::lock_blocking`] only inside a
//!   `spawn_blocking` closure or on a plain thread. Called on an async
//!   thread it panics (this is tokio's rule, and a test documents it).
//! * Never hold two file locks at once, and never wait for anything
//!   else (the queue, another task, a GAMDL run) while holding one.
//!   That is what keeps several downloads from deadlocking each other.
//! * A rename cannot be covered, because the lock is keyed by path.
//!
//! An entry is removed as soon as nobody holds or waits for it, so
//! the map does not grow with every file ever tagged. Keys are tidied
//! first (`a/./b.m4a` and `a//b.m4a` are the same key; on macOS and
//! Windows, where file names are normally not case-sensitive, so are
//! `A/B.M4A` and `a/b.m4a`).
//!
//! ## Usage
//!
//! ```no_run
//! # async fn demo() {
//! let locks = meedyadl::utils::file_locks::global();
//!
//! // Inside one stage's per-file iteration:
//! let guard = locks.lock(std::path::Path::new("/tmp/track.m4a")).await;
//! // ... perform the tag write here ...
//! drop(guard);
//! # }
//! ```
//!
//! Per-file processors that only sometimes need coordination take an
//! `Option<&Arc<FileWriteLocks>>`; production passes
//! `Some(global())`, tests may pass their own instance or `None`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use tokio::sync::{Mutex, OwnedMutexGuard};

/// Shared per-file lock map. Held inside an `Arc` so the same
/// instance can be passed by reference / clone into every stage
/// that needs to coordinate writes.
#[derive(Default)]
pub struct FileWriteLocks {
    /// The outer map. It is only ever held for an insert, a clone or
    /// a remove, never across an `.await`, so an ordinary (not async)
    /// mutex is right here and lets a guard tidy up when dropped.
    /// The inner per-entry mutex guards the actual file.
    map: std::sync::Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

/// The one process-wide map (see the module notes).
static GLOBAL: LazyLock<Arc<FileWriteLocks>> = LazyLock::new(|| Arc::new(FileWriteLocks::new()));

/// The one lock map every tag write in the app uses.
#[must_use]
pub fn global() -> &'static Arc<FileWriteLocks> {
    &GLOBAL
}

/// Holds the write lock for one file. Dropping it releases the lock
/// and, if nobody else holds or waits for that file, removes the
/// file's entry from the map.
pub struct FileWriteGuard {
    /// The tidied key the entry is stored under.
    key: PathBuf,
    /// The file lock itself; `None` only while being dropped.
    inner: Option<OwnedMutexGuard<()>>,
    /// The map the entry lives in.
    owner: Arc<FileWriteLocks>,
}

impl Drop for FileWriteGuard {
    fn drop(&mut self) {
        // Take the map lock FIRST. A newcomer needs it to find or
        // make the entry, so while we hold it nobody can pick the
        // entry up between "release" and "remove".
        let mut map = self.owner.map.lock().unwrap_or_else(|e| e.into_inner());
        // Release the file lock (wakes the next waiter, if any).
        drop(self.inner.take());
        // A waiter holds its own clone of the entry, taken under the
        // map lock, so a count of one means nobody holds or waits.
        if map
            .get(&self.key)
            .is_some_and(|e| Arc::strong_count(e) == 1)
        {
            map.remove(&self.key);
        }
    }
}

/// Tidy a path into the key it is stored under: drops `.` parts and
/// doubled separators, and on macOS and Windows ignores letter case.
/// It does no disk access (the file may not exist yet).
fn normalise_key(path: &Path) -> PathBuf {
    let tidy: PathBuf = path.components().collect();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        // Lower-casing can only make two different files share a
        // lock, which costs a little waiting, never a missed lock.
        PathBuf::from(tidy.to_string_lossy().to_lowercase())
    } else {
        tidy
    }
}

impl FileWriteLocks {
    /// Construct an empty lock map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Find or make the entry for `key` and take our own clone of it,
    /// all under the map lock (so it cannot be removed under us).
    fn entry_for(&self, key: &Path) -> Arc<Mutex<()>> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(key.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Wait for and take the write lock for `path` from async code.
    /// Release it by dropping the guard.
    pub async fn lock(self: &Arc<Self>, path: &Path) -> FileWriteGuard {
        let key = normalise_key(path);
        let entry = self.entry_for(&key);
        // The map lock is already released here, so waiting for this
        // file never holds up other files.
        let inner = entry.lock_owned().await;
        FileWriteGuard {
            key,
            inner: Some(inner),
            owner: Arc::clone(self),
        }
    }

    /// Wait for and take the write lock for `path` from a thread that
    /// may block: inside `spawn_blocking`, or a plain thread. Calling
    /// it on an async thread panics.
    pub fn lock_blocking(self: &Arc<Self>, path: &Path) -> FileWriteGuard {
        let key = normalise_key(path);
        let entry = self.entry_for(&key);
        let inner = entry.blocking_lock_owned();
        FileWriteGuard {
            key,
            inner: Some(inner),
            owner: Arc::clone(self),
        }
    }

    /// Number of files the map currently holds or waits for. Entries
    /// are removed when unused, so this is 0 when nothing is locked.
    #[cfg(test)]
    pub fn tracked_files(&self) -> usize {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).len()
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
    async fn lock_creates_entry_lazily_and_removes_it_when_unused() {
        let locks = Arc::new(FileWriteLocks::new());
        let path = PathBuf::from("/tmp/track-1.m4a");
        assert_eq!(locks.tracked_files(), 0);

        let g1 = locks.lock(&path).await;
        assert_eq!(locks.tracked_files(), 1);
        drop(g1);
        assert_eq!(locks.tracked_files(), 0, "unused entries are removed");

        let g2 = locks.lock(&path).await;
        assert_eq!(locks.tracked_files(), 1);
        drop(g2);
        assert_eq!(locks.tracked_files(), 0);
    }

    /// Distinct paths get distinct entries.
    #[tokio::test]
    async fn distinct_paths_get_distinct_locks() {
        let locks = Arc::new(FileWriteLocks::new());
        let a = PathBuf::from("/tmp/track-a.m4a");
        let b = PathBuf::from("/tmp/track-b.m4a");
        let _ga = locks.lock(&a).await;
        let _gb = locks.lock(&b).await;
        assert_eq!(locks.tracked_files(), 2);
    }

    /// While a second task waits, dropping the first guard keeps the
    /// entry and hands it to the waiter; when all are done the map is
    /// empty again.
    #[tokio::test]
    async fn entry_survives_while_a_waiter_exists_then_is_removed() {
        let locks = Arc::new(FileWriteLocks::new());
        let path = PathBuf::from("/tmp/track-wait.m4a");
        let first = locks.lock(&path).await;

        let (l2, p2) = (locks.clone(), path.clone());
        let waiter = tokio::spawn(async move {
            let _g = l2.lock(&p2).await;
        });
        // Let the waiter queue up behind us.
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!waiter.is_finished());

        drop(first);
        waiter.await.unwrap();
        assert_eq!(locks.tracked_files(), 0, "no holder and no waiter left");
    }

    /// Differently spelled paths for one file share one entry.
    #[tokio::test]
    async fn tidied_keys_share_one_entry() {
        let locks = Arc::new(FileWriteLocks::new());
        let g = locks.lock(Path::new("/tmp/a/./b.m4a")).await;
        let (l2, done) = (
            locks.clone(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        let done2 = done.clone();
        let t = tokio::spawn(async move {
            let _g = l2.lock(Path::new("/tmp/a//b.m4a")).await;
            done2.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(
            !done.load(std::sync::atomic::Ordering::SeqCst),
            "same file, so it must wait"
        );
        assert_eq!(locks.tracked_files(), 1);
        drop(g);
        t.await.unwrap();
        assert_eq!(locks.tracked_files(), 0);
    }

    /// On macOS and Windows, upper and lower case spell the same file.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn case_is_ignored_on_case_insensitive_systems() {
        assert_eq!(
            normalise_key(Path::new("/tmp/A/B.M4A")),
            normalise_key(Path::new("/tmp/a/b.m4a"))
        );
    }

    /// The lock key tidying itself (no matter the system).
    #[test]
    fn normalise_key_drops_dots_and_double_separators() {
        assert_eq!(
            normalise_key(Path::new("/tmp/a/./b.m4a")),
            normalise_key(Path::new("/tmp/a//b.m4a"))
        );
    }

    /// A blocking waiter (as used inside `spawn_blocking`) waits for an
    /// async holder and gets the lock after release, on the default
    /// current-thread test runtime.
    #[tokio::test]
    async fn blocking_waiter_waits_for_async_holder() {
        let locks = Arc::new(FileWriteLocks::new());
        let path = PathBuf::from("/tmp/track-mixed.m4a");
        let g = locks.lock(&path).await;

        let (l2, p2) = (locks.clone(), path.clone());
        let h = tokio::task::spawn_blocking(move || {
            let _g = l2.lock_blocking(&p2);
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!h.is_finished(), "must still be waiting");
        drop(g);
        h.await.unwrap();
        assert_eq!(locks.tracked_files(), 0);
    }

    /// `lock_blocking` called straight on an async thread panics
    /// (tokio's rule). It is only for `spawn_blocking` and plain threads.
    #[tokio::test]
    #[should_panic]
    async fn lock_blocking_on_an_async_thread_panics() {
        let locks = Arc::new(FileWriteLocks::new());
        let _g = locks.lock_blocking(Path::new("/tmp/never.m4a"));
    }

    /// Eight tasks, half async and half blocking, all work on one path:
    /// never more than one is inside the section, and the map ends empty.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn many_mixed_waiters_never_overlap() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let locks = Arc::new(FileWriteLocks::new());
        let path = PathBuf::from("/tmp/track-many.m4a");
        let inside = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for i in 0..8 {
            let (l, p, ins, mx) = (
                locks.clone(),
                path.clone(),
                inside.clone(),
                max_seen.clone(),
            );
            let section = move || {
                let n = ins.fetch_add(1, Ordering::SeqCst) + 1;
                mx.fetch_max(n, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                ins.fetch_sub(1, Ordering::SeqCst);
            };
            if i % 2 == 0 {
                handles.push(tokio::spawn(async move {
                    let _g = l.lock(&p).await;
                    section();
                }));
            } else {
                handles.push(tokio::spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        let _g = l.lock_blocking(&p);
                        section();
                    })
                    .await
                    .unwrap();
                }));
            }
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(
            max_seen.load(Ordering::SeqCst),
            1,
            "writes must never overlap"
        );
        assert_eq!(locks.tracked_files(), 0);
    }

    /// The global map is one object, and works for a unique path.
    #[tokio::test]
    async fn global_is_one_shared_map() {
        assert!(Arc::ptr_eq(global(), global()));
        let dir = std::env::temp_dir().join(format!("meedyadl-lock-{}", std::process::id()));
        let g = global().lock(&dir.join("unique.m4a")).await;
        drop(g);
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
