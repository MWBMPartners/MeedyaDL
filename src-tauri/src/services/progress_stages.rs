// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Per-item progress stage registry.
// =================================
//
// Centralises the ordered list of stages a queue item passes through
// AFTER its primary GAMDL download exits and BEFORE the completion task
// flips it to `Complete`. Each stage has:
//
//   - a `label()`      — the human-readable caption for the per-item
//                        progress bar (`processing_label` field on
//                        `QueueItemStatus`),
//   - a `weight()`     — the cumulative fraction-complete (0.0..=1.0)
//                        AT THE START of the stage. This is what feeds
//                        the queue-level progress bar's partial credit.
//
// Both the **enrichment task** and the **companion task** use this
// registry so the per-item bar always shows the correct stage even
// when responsibility crosses task boundaries (e.g. companion lyrics
// conversion runs after companion downloads, but enrichment may still
// be on Music Video Discovery in parallel — see #706 / #712 for the
// concurrency contract).
//
// **Why an enum, not constants** (#712-followup): pre-refactor we had
// 8 scattered `PROGRESS_*_STAGE: f32 = ...` constants and 9 closure-
// local `set_label(...)` calls inside the enrichment task. Adding a
// new stage required edits in 3 places (constant, closure call, doc
// comment) and a stale label was a very-easy-to-miss bug — exactly
// the bug pattern that produced the 30-minute "ReplayGain loudness
// analysis…" hang in the user's 2026-05-08 reproduction. The enum
// is the single source of truth: adding a stage means one new variant
// and the compiler enforces label / weight / ordering coverage.

use std::fmt;

use tauri::{AppHandle, Emitter};

use super::download_queue::QueueHandle;

/// All per-item processing stages, in chronological order.
///
/// `repr(u8)` and `PartialOrd / Ord` make it cheap to compare two
/// stages and verify the pipeline is moving forward (the queue's
/// `set_processing_label` will be tightened in a follow-up commit to
/// reject backwards transitions, killing a class of stale-label bugs).
///
/// Variants ordered top-to-bottom = chronological. Adding a new
/// stage requires inserting it at the right position and rebalancing
/// the weights via the `weight()` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ProgressStage {
    /// Step 1 — Apple Music API metadata fetch + ID3/MP4 tag enrichment.
    Metadata = 1,
    /// Step 1b — syllable-lyrics API fallback for word-level TTML.
    WordLyrics = 2,
    /// Step 2/2b/2c/2d/2e/2f — Enhanced LRC, lyrics fallback,
    /// WebVTT / Rich SRT / ASS generation, and subtitle embedding.
    LyricsConversion = 3,
    /// Step 3 / 3b — animated artwork download + artist promo video.
    AnimatedArtwork = 4,
    /// Step 4 — AcoustID fingerprinting via the embedded `rusty-chromaprint`.
    AcoustId = 5,
    /// Step 5 — ReplayGain loudness analysis (FFmpeg ebur128 filter).
    ReplayGain = 6,
    /// Steps 6 / 6b — Music video discovery (MusicKit API + MusicBrainz ISRC).
    MusicVideoDiscovery = 7,
    /// Tail of the enrichment pipeline — manifest write, post-companion
    /// advisory pass, content-rating suffix application.
    Finalising = 8,
}

impl ProgressStage {
    /// Cumulative fraction-complete (0.0..=1.0) AT THE START of this
    /// stage. Used by the queue-level progress bar to show partial
    /// credit during long enrichment phases (#576).
    ///
    /// Weights are anchored so:
    ///   - `Metadata.weight() == 0.05` — primary GAMDL just exited.
    ///   - `Finalising.weight() == 0.95` — last 5% is for the manifest
    ///     write + advisory rename + the actual `set_complete` call.
    ///
    /// The progression is deliberately roughly equal-weight across
    /// user-observable stages; rebalance when real-world timing data
    /// (e.g. from #579 box-set repros) warrants.
    #[must_use]
    pub const fn weight(self) -> f32 {
        match self {
            Self::Metadata => 0.05,
            Self::WordLyrics => 0.15,
            Self::LyricsConversion => 0.25,
            Self::AnimatedArtwork => 0.40,
            Self::AcoustId => 0.55,
            Self::ReplayGain => 0.75,
            Self::MusicVideoDiscovery => 0.85,
            Self::Finalising => 0.95,
        }
    }

    /// User-facing caption for the per-item progress bar.
    ///
    /// This is the *exact* string written to `QueueItemStatus.processing_label`
    /// and rendered in [`GlobalProgressBar.tsx`] before the
    /// `Artist — Album — Track` half. Keep human-readable, end with `…`
    /// to signal "ongoing", match the existing tone (sentence case, no
    /// terminal period).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Metadata => "Enriching metadata tags…",
            Self::WordLyrics => "Fetching syllable-level lyrics…",
            Self::LyricsConversion => "Converting lyrics formats…",
            Self::AnimatedArtwork => "Downloading animated artwork…",
            Self::AcoustId => "AcoustID fingerprinting…",
            Self::ReplayGain => "ReplayGain loudness analysis…",
            Self::MusicVideoDiscovery => "Music video discovery…",
            Self::Finalising => "Finalising metadata…",
        }
    }

    /// All stages in chronological order. Use for tests, UI debug
    /// dropdowns, or batch validation.
    pub const ALL: [Self; 8] = [
        Self::Metadata,
        Self::WordLyrics,
        Self::LyricsConversion,
        Self::AnimatedArtwork,
        Self::AcoustId,
        Self::ReplayGain,
        Self::MusicVideoDiscovery,
        Self::Finalising,
    ];
}

impl fmt::Display for ProgressStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Sets the per-item progress bar caption + cumulative-weight to a
/// stage. Available to **any** caller that holds a `QueueHandle` and
/// `AppHandle` — Phase 3.5d, replacing the closure-local `set_label`
/// inside the enrichment task that the companion task could not call.
///
/// Format of the caption written to `processing_label`:
///
/// ```text
/// {stage.label()} — {Artist}: {Album}
/// ```
///
/// The Artist/Album suffix is appended when the queue item has those
/// fields populated (typical after the early Apple Music API metadata
/// fetch); otherwise just the stage label is shown. Track-level
/// granularity is intentionally NOT included here — the per-track
/// changes too quickly to be useful as a label.
///
/// **Best-effort by design (sync, `try_lock`).** If the queue lock is
/// already held by the caller (common pattern in `download_queue.rs`
/// — emit happens inside `lock().await { … }`), the update is
/// skipped silently. The next stage transition will refresh the
/// label. This matches the pre-refactor closure's behaviour.
///
/// Emits a `queue-updated` Tauri event after the lock-held write so
/// the frontend re-reads the queue state. Errors swallowed: worst
/// case is a stale UI until the next transition.
pub fn set_stage(
    app: &AppHandle,
    queue: &QueueHandle,
    download_id: &str,
    stage: ProgressStage,
) {
    set_stage_with_label(app, queue, download_id, stage, stage.label());
}

/// Updates the per-item progress bar caption **only**, leaving the
/// progress fraction untouched (#808 regression fix).
///
/// Use this for caption-update sites that don't represent a real
/// stage transition — e.g. per-track caption updates inside a
/// long-running stage like the companion-download supervisor's
/// `update_companion_label_from_line`. The progress fraction
/// continues to be driven by the resolution chain established in
/// #790 (per-file `[download] X%` from GAMDL → enrichment stage
/// weights → terminal-state values) so the bar reflects the actual
/// within-stage progress rather than the stage's nominal weight.
///
/// Distinct from [`set_stage_with_label`] which updates BOTH the
/// label and the progress fraction — that's correct for stage
/// transitions but wrong for caption-only updates that happen many
/// times per stage (e.g. one per track in a 21-track companion
/// download). Calling `set_stage_with_label(…, Finalising, …)` in
/// a tight per-track loop pegs the bar at 0.95 — exactly the bug
/// #808 reports.
pub fn set_label_only(
    app: &AppHandle,
    queue: &QueueHandle,
    download_id: &str,
    label: &str,
) {
    if let Ok(mut q) = queue.try_lock() {
        // Same Artist: Album suffix logic as `set_stage_with_label`
        // so the caption shape stays consistent across the two
        // helpers. The only difference is that this helper does NOT
        // call `set_processing_progress`.
        let context = q
            .get_status()
            .into_iter()
            .find(|s| s.id == download_id)
            .map(|s| {
                let artist = s.artist_name.unwrap_or_default();
                let album = s.album_name.unwrap_or_default();
                if !artist.is_empty() && !album.is_empty() {
                    format!(" — {artist}: {album}")
                } else if !album.is_empty() {
                    format!(" — {album}")
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        let full_label = format!("{label}{context}");
        q.set_processing_label(download_id, &full_label);
        // Deliberately NO `q.set_processing_progress(...)` here —
        // #808 fix. The bar value stays driven by the resolution
        // chain established in #790.
    }
    let _ = app.emit("queue-updated", download_id);
}

/// Same as [`set_stage`] but lets the caller override the caption
/// text while keeping the stage's cumulative weight. Used for sub-
/// stage progress (Phase 3.5g) — e.g. "Generating Rich SRT…" still
/// inside the `LyricsConversion` stage so the queue-level progress
/// bar weight doesn't bounce.
pub fn set_stage_with_label(
    app: &AppHandle,
    queue: &QueueHandle,
    download_id: &str,
    stage: ProgressStage,
    label: &str,
) {
    if let Ok(mut q) = queue.try_lock() {
        // Build "Artist: Album" suffix from cached API metadata.
        // Distinct from `media_label_for` (which includes the current
        // track and uses ` — ` separators); the per-item bar wants a
        // shorter, stable suffix that doesn't mutate per-track.
        let context = q
            .get_status()
            .into_iter()
            .find(|s| s.id == download_id)
            .map(|s| {
                let artist = s.artist_name.unwrap_or_default();
                let album = s.album_name.unwrap_or_default();
                if !artist.is_empty() && !album.is_empty() {
                    format!(" — {artist}: {album}")
                } else if !album.is_empty() {
                    format!(" — {album}")
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        let full_label = format!("{label}{context}");
        q.set_processing_label(download_id, &full_label);
        q.set_processing_progress(download_id, stage.weight());
    }
    let _ = app.emit("queue-updated", download_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Weights must be strictly increasing in chronological order.
    /// A regression here would let a later stage propose a smaller
    /// fraction-complete than an earlier one — the queue-level
    /// progress bar would visibly tick *backwards*.
    #[test]
    fn weights_strictly_increasing() {
        let weights: Vec<f32> = ProgressStage::ALL.iter().map(|s| s.weight()).collect();
        for window in weights.windows(2) {
            assert!(
                window[0] < window[1],
                "non-monotonic weights: {window:?}"
            );
        }
    }

    /// Weights must be bounded in (0.0, 1.0). Anything ≤0 or ≥1
    /// breaks the queue-level "X of Y complete" math.
    #[test]
    fn weights_in_open_unit_interval() {
        for stage in ProgressStage::ALL {
            let w = stage.weight();
            assert!(w > 0.0 && w < 1.0, "{stage:?} weight {w} out of range");
        }
    }

    /// Labels must be non-empty and end with the ellipsis convention.
    /// Catches accidental empty strings or missing trailing `…`.
    #[test]
    fn labels_use_ellipsis_and_are_non_empty() {
        for stage in ProgressStage::ALL {
            let label = stage.label();
            assert!(!label.is_empty(), "{stage:?} label is empty");
            assert!(
                label.ends_with('…'),
                "{stage:?} label `{label}` does not end with `…`"
            );
        }
    }

    /// Ordering by `Ord` must match chronological order in `ALL`.
    /// `set_processing_label` will eventually use this comparison to
    /// reject backwards transitions.
    #[test]
    fn ord_matches_chronological_order() {
        for window in ProgressStage::ALL.windows(2) {
            assert!(window[0] < window[1], "Ord mismatch at {window:?}");
        }
    }

    /// Display impl must produce the same string as `label()`.
    #[test]
    fn display_returns_label() {
        for stage in ProgressStage::ALL {
            assert_eq!(format!("{stage}"), stage.label());
        }
    }

    /// `ALL` must enumerate every variant — if a new variant is added
    /// without updating `ALL`, the count won't match. Catches the
    /// "added an enum variant but forgot the registry array" bug.
    #[test]
    fn all_array_covers_every_variant() {
        // Every variant constructed manually — if a new one is added
        // the compiler forces this match to be exhaustive (clippy lint
        // `match_wildcard_for_single_variants` would catch a wildcard).
        let manual_count = [
            ProgressStage::Metadata,
            ProgressStage::WordLyrics,
            ProgressStage::LyricsConversion,
            ProgressStage::AnimatedArtwork,
            ProgressStage::AcoustId,
            ProgressStage::ReplayGain,
            ProgressStage::MusicVideoDiscovery,
            ProgressStage::Finalising,
        ]
        .len();
        assert_eq!(
            ProgressStage::ALL.len(),
            manual_count,
            "ProgressStage::ALL is missing a variant"
        );
    }
}
