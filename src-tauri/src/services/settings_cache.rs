// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! In-process `AppSettings` cache to eliminate redundant disk reads
//! on the queue hot path (#690).
//!
//! ## Why
//!
//! Pre-#690, every queue-completion-side branch in `download_queue.rs`
//! called `load_settings_for_queue()` which round-trips through
//! `config_service::load_settings()` and re-reads `settings.json` from
//! disk on each invocation. A 50-item batch with active companions +
//! enrichment can hit `load_settings_for_queue` 20+ times per item
//! (each tier, each enrichment step, each completion check), so a
//! large batch was doing 1000+ redundant disk reads in a tight
//! window.
//!
//! Settings change frequency vs queue completion frequency is tiny in
//! practice — the user toggles a setting once, then runs N downloads.
//! So an in-process cache populated at startup and refreshed only on
//! `save_settings` is safe AND eliminates the I/O.
//!
//! ## Safety model
//!
//! Stale cache failure mode collapses to "fall through to disk" —
//! the cache is a performance optimisation, not the source of truth.
//! Every read site that calls [`SettingsCache::get_or_load`] gets
//! either the cached value (fast) or a fresh disk read (slow but
//! always-correct). If the cache and disk ever diverge for any
//! reason, the next `save_settings` IPC call re-syncs them.
//!
//! ## Concurrency
//!
//! `Arc<RwLock<Option<AppSettings>>>` — multiple readers + occasional
//! writer. `Option<>` because we lazy-initialise on first access
//! rather than blocking the app startup path on a disk read; the
//! first reader pays the disk-read cost, every subsequent reader
//! sees the cached value.

use std::sync::{Arc, RwLock};

use crate::models::settings::AppSettings;
use crate::services::config_service;

/// Tauri-managed-state wrapper around the in-process `AppSettings`
/// cache (#690). Cloning is cheap — the `Arc` is shared, the
/// `RwLock` provides interior mutability.
#[derive(Clone)]
pub struct SettingsCache {
    inner: Arc<RwLock<Option<AppSettings>>>,
}

impl SettingsCache {
    /// Construct an empty cache. The first reader populates it from
    /// disk via [`Self::get_or_load`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the cached settings if present, otherwise loads from
    /// disk, stores in the cache, and returns the loaded value.
    /// On disk-read failure returns `AppSettings::default()` AND
    /// caches that default — matches the pre-#690 behaviour of
    /// `load_settings_for_queue`.
    ///
    /// The clone is necessary because `RwLock::read` returns a guard
    /// that can't outlive the lock; consumers want an owned value
    /// they can pass around. `AppSettings` is `Clone` and the
    /// clone cost is dominated by a handful of `String` allocations
    /// — orders of magnitude cheaper than a disk read.
    pub fn get_or_load(&self, app: &tauri::AppHandle) -> AppSettings {
        // Fast path: cache populated. Drop the read guard before
        // returning the clone so writers aren't blocked.
        if let Ok(guard) = self.inner.read() {
            if let Some(ref settings) = *guard {
                return settings.clone();
            }
        }

        // Slow path: cache empty. Load from disk, populate cache,
        // return the loaded value. Use the same error-tolerant
        // shape as `load_settings_for_queue`'s pre-#690 logic.
        let loaded = match config_service::load_settings(app) {
            Ok(settings) => settings,
            Err(e) => {
                log::warn!("Failed to load settings for cache: {e}, using defaults");
                AppSettings::default()
            }
        };
        if let Ok(mut guard) = self.inner.write() {
            *guard = Some(loaded.clone());
        }
        loaded
    }

    /// Force-refresh the cache with the given settings. Called by
    /// the `save_settings` IPC after a successful disk write so the
    /// cache stays in sync without forcing every reader to discover
    /// the staleness on its own.
    pub fn refresh(&self, settings: AppSettings) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = Some(settings);
        }
    }

    /// Apply an in-place transformation to the cached settings.
    /// Used by `execute_after_queue_action` to clear the one-shot
    /// flag in the cache atomically with the disk write — without
    /// this, the next reader after a one-shot clear would see the
    /// stale pre-clear value until `save_settings` is called.
    ///
    /// Returns the post-mutation snapshot for callers that want to
    /// then persist it back to disk via `config_service::save_settings`.
    /// When the cache is empty (no reader has populated it yet),
    /// returns `None` — caller should fall back to its own load.
    pub fn mutate<F>(&self, f: F) -> Option<AppSettings>
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut guard = self.inner.write().ok()?;
        let settings = guard.as_mut()?;
        f(settings);
        Some(settings.clone())
    }
}

impl Default for SettingsCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clearing the one-shot action in the cache must leave every other
    /// cached setting exactly as it was.
    ///
    /// This is the failure path of a one-shot after-queue action. When the
    /// queue empties and the "just this once" action has run, the flag has
    /// to be cleared or it fires again next time — and these actions
    /// include shutting the computer down, so firing one twice is not a
    /// small annoyance. Normally clearing it on disk refreshes this cache
    /// as a side effect. When that write fails (a full disk, a folder that
    /// has become read-only) the cache has to be cleared here instead, and
    /// it must clear ONLY that field: the cached settings are what the
    /// rest of the app reads until something reloads them, so overwriting
    /// them with anything else would spread the damage.
    #[test]
    fn clearing_the_one_shot_action_leaves_every_other_setting_alone() {
        let cache = SettingsCache::new();

        let before = AppSettings {
            after_queue_once: Some(crate::models::settings::AfterQueueAction::ShutdownComputer),
            output_path: "/somewhere/the/person/chose".to_string(),
            verbose_activity_log: true,
            ..Default::default()
        };
        cache.refresh(before.clone());

        let after = cache
            .mutate(|s| s.after_queue_once = None)
            .expect("the cache was populated, so mutate should return the new value");

        assert_eq!(
            after.after_queue_once, None,
            "the one-shot must be gone, or it fires again when the queue next empties"
        );

        // Everything else must be untouched. Compared as JSON because
        // AppSettings has no equality of its own — and comparing the whole
        // serialised form means a field added to AppSettings in future is
        // covered here automatically, without anyone remembering to come
        // back and extend this test.
        let expected = AppSettings { after_queue_once: None, ..before };
        assert_eq!(
            serde_json::to_value(&after).expect("serialise the changed settings"),
            serde_json::to_value(&expected).expect("serialise the expected settings"),
            "clearing the one-shot must not disturb any other cached setting"
        );
    }

    /// A fresh cache starts empty.
    #[test]
    fn new_cache_is_empty() {
        let cache = SettingsCache::new();
        let guard = cache.inner.read().unwrap();
        assert!(guard.is_none());
    }

    /// `refresh` populates the cache and subsequent
    /// (non-disk-touching) checks see the new value.
    #[test]
    fn refresh_populates_cache() {
        let cache = SettingsCache::new();
        cache.refresh(AppSettings::default());
        let guard = cache.inner.read().unwrap();
        assert!(guard.is_some());
    }

    /// `mutate` returns the post-mutation snapshot when the cache
    /// is populated, and `None` when it's empty (lazy-init case).
    #[test]
    fn mutate_returns_snapshot_after_change() {
        let cache = SettingsCache::new();
        assert!(cache.mutate(|s| s.overwrite = true).is_none(), "empty cache returns None");
        cache.refresh(AppSettings::default());
        let snapshot = cache
            .mutate(|s| s.overwrite = true)
            .expect("populated cache returns snapshot");
        assert!(snapshot.overwrite, "mutation must be visible in the snapshot");
    }

    /// Cloning the cache shares the underlying state — both clones
    /// see refreshes performed via the other handle. Important
    /// because the cache is cloned into Tauri-managed state and
    /// captured by every queue task.
    #[test]
    fn clones_share_underlying_state() {
        let a = SettingsCache::new();
        let b = a.clone();
        a.refresh(AppSettings::default());
        let guard = b.inner.read().unwrap();
        assert!(guard.is_some(), "clone must see refreshes performed via the original");
    }
}
