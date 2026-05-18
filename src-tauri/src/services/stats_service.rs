// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Lifetime download analytics (#464).
//!
//! Aggregates the persistent download history (`history.json`, owned by
//! `services::history_service`) into roll-up stats: totals, success
//! rate, codec distribution, recent activity. Computed on-demand rather
//! than stored separately — `history_service` is already the source of
//! truth for terminal-state download records and duplicating that data
//! into a stats file would create a sync hazard.
//!
//! Recomputation cost: O(N) over history length, capped at
//! `MAX_HISTORY_ENTRIES = 1000` per the history service, so a full
//! refresh is sub-millisecond on any modern machine. No caching
//! needed; the IPC handler re-reads on every request.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use tauri::AppHandle;

use crate::services::history_service::{self, HistoryEntry};

/// Roll-up statistics returned to the frontend.
///
/// All numeric fields are computed from the history JSON in a single
/// pass; missing optional fields on per-entry records (e.g., a legacy
/// entry without `codec`) are silently skipped rather than counted as
/// "unknown" — keeps the UI uncluttered when history entries pre-date
/// a particular field.
#[derive(Debug, Serialize, Default, Clone)]
pub struct LifetimeStats {
    /// Total number of history entries (success + failure combined).
    pub total: usize,
    /// Number of entries with `status == "success"`.
    pub success: usize,
    /// Number of entries with `status == "failed"`.
    pub failed: usize,
    /// Percentage in `[0.0, 100.0]`. Zero when `total == 0`.
    pub success_rate: f64,
    /// Codec distribution: codec key → count. Sorted by count
    /// descending in the IPC response so the UI doesn't need to sort.
    pub codec_distribution: Vec<CodecCount>,
    /// Top artist by download count (None when total = 0).
    pub top_artist: Option<NameCount>,
    /// Top album by download count (None when total = 0).
    pub top_album: Option<NameCount>,
    /// Downloads in the last 7 days, by ISO date (`YYYY-MM-DD`).
    /// Sparse — only dates with at least one download appear.
    /// Sorted oldest → newest so a chart can render directly.
    pub last_7_days: Vec<DayCount>,
    /// Earliest history timestamp (ISO 8601). None on empty history.
    pub earliest: Option<String>,
    /// Latest history timestamp (ISO 8601). None on empty history.
    pub latest: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CodecCount {
    pub codec: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct NameCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct DayCount {
    /// ISO date `YYYY-MM-DD`.
    pub date: String,
    pub count: usize,
}

/// IPC handler-facing wrapper: load history then compute the roll-up.
/// Errors are converted to an empty `LifetimeStats` rather than
/// surfaced — the panel should always render *something*, even on a
/// brand-new install or a corrupt history file (the history service
/// already returns empty on parse failure).
pub fn get_lifetime_stats(app: &AppHandle) -> LifetimeStats {
    let history = history_service::list_history(app);
    compute_lifetime_stats(&history)
}

/// Pure aggregation function — separated from the IPC wrapper so unit
/// tests can exercise it without a Tauri runtime.
pub fn compute_lifetime_stats(history: &[HistoryEntry]) -> LifetimeStats {
    if history.is_empty() {
        return LifetimeStats::default();
    }

    let total = history.len();
    let success = history.iter().filter(|e| e.status == "success").count();
    let failed = history.iter().filter(|e| e.status == "failed").count();
    let success_rate = if total > 0 {
        (success as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Codec distribution — only count successes (failures didn't
    // produce a file, so their codec field is meaningless even when
    // populated). Sort descending by count.
    let mut codec_map: BTreeMap<String, usize> = BTreeMap::new();
    for entry in history.iter().filter(|e| e.status == "success") {
        if let Some(codec) = &entry.codec {
            *codec_map.entry(codec.clone()).or_insert(0) += 1;
        }
    }
    let mut codec_distribution: Vec<CodecCount> = codec_map
        .into_iter()
        .map(|(codec, count)| CodecCount { codec, count })
        .collect();
    codec_distribution.sort_by(|a, b| b.count.cmp(&a.count).then(a.codec.cmp(&b.codec)));

    // Top artist / top album — count across all entries with a value.
    let top_artist = top_name(history, |e| e.artist.as_deref());
    let top_album = top_name(history, |e| e.album.as_deref());

    // Last 7 days: bucket by ISO date (UTC). Parse `completed_at` to
    // a chrono DateTime; entries with unparseable timestamps are
    // silently skipped (legacy data hygiene — never a parse error
    // worth surfacing).
    let now = Utc::now();
    let seven_days_ago = now - Duration::days(7);
    let mut day_map: BTreeMap<String, usize> = BTreeMap::new();
    for entry in history {
        let Ok(ts) = entry.completed_at.parse::<DateTime<Utc>>() else {
            continue;
        };
        if ts < seven_days_ago {
            continue;
        }
        let date_key = ts.format("%Y-%m-%d").to_string();
        *day_map.entry(date_key).or_insert(0) += 1;
    }
    let last_7_days: Vec<DayCount> = day_map
        .into_iter()
        .map(|(date, count)| DayCount { date, count })
        .collect();

    // Earliest / latest timestamps — scan once for both extremes.
    let mut earliest: Option<String> = None;
    let mut latest: Option<String> = None;
    for entry in history {
        let ts = &entry.completed_at;
        if earliest.as_ref().is_none_or(|e| ts < e) {
            earliest = Some(ts.clone());
        }
        if latest.as_ref().is_none_or(|l| ts > l) {
            latest = Some(ts.clone());
        }
    }

    LifetimeStats {
        total,
        success,
        failed,
        success_rate,
        codec_distribution,
        top_artist,
        top_album,
        last_7_days,
        earliest,
        latest,
    }
}

/// Helper: scan history with a value-extractor and return the
/// most-frequent value as a `NameCount`. Empty / None values are
/// skipped. Returns `None` when no entry has a non-empty value.
fn top_name<F>(history: &[HistoryEntry], extract: F) -> Option<NameCount>
where
    F: Fn(&HistoryEntry) -> Option<&str>,
{
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in history {
        if let Some(v) = extract(entry) {
            if v.is_empty() {
                continue;
            }
            *counts.entry(v.to_string()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(name, count)| NameCount { name, count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::history_service::HistoryEntry;

    fn entry(
        artist: Option<&str>,
        album: Option<&str>,
        codec: Option<&str>,
        status: &str,
        completed_at: &str,
    ) -> HistoryEntry {
        HistoryEntry {
            id: "test".to_string(),
            url: "https://music.apple.com/x".to_string(),
            title: None,
            artist: artist.map(String::from),
            album: album.map(String::from),
            codec: codec.map(String::from),
            file_path: None,
            started_at: completed_at.to_string(),
            completed_at: completed_at.to_string(),
            status: status.to_string(),
            error_message: None,
        }
    }

    #[test]
    fn empty_history_returns_zeroed_stats() {
        let s = compute_lifetime_stats(&[]);
        assert_eq!(s.total, 0);
        assert_eq!(s.success_rate, 0.0);
        assert!(s.codec_distribution.is_empty());
        assert!(s.top_artist.is_none());
    }

    #[test]
    fn computes_success_rate_and_codec_distribution() {
        let now = Utc::now().to_rfc3339();
        let entries = vec![
            entry(Some("A"), Some("X"), Some("alac"), "success", &now),
            entry(Some("A"), Some("X"), Some("alac"), "success", &now),
            entry(Some("B"), Some("Y"), Some("aac"), "success", &now),
            entry(Some("A"), Some("Z"), None, "failed", &now),
        ];
        let s = compute_lifetime_stats(&entries);
        assert_eq!(s.total, 4);
        assert_eq!(s.success, 3);
        assert_eq!(s.failed, 1);
        assert!((s.success_rate - 75.0).abs() < 0.01);

        // Codec distribution: alac=2, aac=1, sorted descending.
        assert_eq!(s.codec_distribution.len(), 2);
        assert_eq!(s.codec_distribution[0].codec, "alac");
        assert_eq!(s.codec_distribution[0].count, 2);
        assert_eq!(s.codec_distribution[1].codec, "aac");
        assert_eq!(s.codec_distribution[1].count, 1);

        // Top artist = "A" (3 entries), top album = "X" (2 entries).
        assert_eq!(s.top_artist.unwrap().name, "A");
        assert_eq!(s.top_album.unwrap().name, "X");
    }

    #[test]
    fn last_7_days_filters_old_entries() {
        let recent = Utc::now().to_rfc3339();
        let old = (Utc::now() - Duration::days(10)).to_rfc3339();
        let entries = vec![
            entry(Some("A"), None, Some("alac"), "success", &recent),
            entry(Some("B"), None, Some("alac"), "success", &old),
        ];
        let s = compute_lifetime_stats(&entries);
        assert_eq!(s.last_7_days.len(), 1, "old entry should be excluded");
    }

    #[test]
    fn failures_dont_pollute_codec_distribution() {
        let now = Utc::now().to_rfc3339();
        // A failed entry with a populated codec must NOT count in
        // the distribution — failures didn't produce a file.
        let entries = vec![
            entry(None, None, Some("alac"), "failed", &now),
            entry(None, None, Some("aac"), "success", &now),
        ];
        let s = compute_lifetime_stats(&entries);
        assert_eq!(s.codec_distribution.len(), 1);
        assert_eq!(s.codec_distribution[0].codec, "aac");
    }
}
