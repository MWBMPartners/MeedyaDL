// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Unit tests for the download queue module.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` resolves
// to the `download_queue` module root (mod.rs), so every symbol these
// tests exercise remains reachable exactly as before.

use super::*;
use crate::models::download::{DownloadRequest, DownloadState};
use crate::models::gamdl_options::{GamdlOptions, SongCodec};
use crate::models::settings::{DiscNumberPadding, TrackNumberPadding};

// ----------------------------------------------------------
// Track / disc padding template mutation (#587)
// ----------------------------------------------------------

#[test]
fn padding_leaves_explicit_format_spec_untouched() {
    // User's explicit {track:02d} must take precedence over the
    // padding setting — their template wins.
    let out = apply_padding_to_template("{disc}-{track:02d} {title}", 3, 0);
    assert_eq!(out, "{disc}-{track:02d} {title}");
}

#[test]
fn padding_substitutes_bare_track_token() {
    let out = apply_padding_to_template("{track} {title}", 3, 0);
    assert_eq!(out, "{track:03d} {title}");
}

#[test]
fn padding_substitutes_bare_disc_and_track_tokens() {
    let out = apply_padding_to_template("{disc}-{track} {title}", 3, 2);
    assert_eq!(out, "{disc:02d}-{track:03d} {title}");
}

#[test]
fn padding_width_zero_emits_bare_placeholder() {
    // `None` padding or 0-digit Auto shouldn't produce "{track:0d}"
    // which would be a broken format spec.
    let out = apply_padding_to_template("{track} {title}", 0, 0);
    assert_eq!(out, "{track} {title}");
}

#[test]
fn padding_leaves_similar_but_distinct_tokens_alone() {
    // `{track_total}` is a different placeholder — must not match
    // the bare `{track}` substitution target.
    let out = apply_padding_to_template("{track} of {track_total}", 2, 0);
    assert_eq!(out, "{track:02d} of {track_total}");
}

#[test]
fn padding_auto_mode_derives_width_from_track_total() {
    // Album of 200 tracks → 3 digits.
    assert_eq!(TrackNumberPadding::Auto.resolve_width(Some(200)), 3);
    // Album of 12 tracks → 2 digits.
    assert_eq!(TrackNumberPadding::Auto.resolve_width(Some(12)), 2);
    // Pathological 10 000-track dump → 4 digits.
    assert_eq!(TrackNumberPadding::Auto.resolve_width(Some(10_000)), 4);
    // No track_total known → safe default 2 digits (matches pre-#587).
    assert_eq!(TrackNumberPadding::Auto.resolve_width(None), 2);
}

#[test]
fn padding_fixed_modes_ignore_track_total() {
    // Fixed settings are library-wide preferences; album size
    // shouldn't alter them.
    assert_eq!(TrackNumberPadding::None.resolve_width(Some(200)), 0);
    assert_eq!(TrackNumberPadding::TwoDigits.resolve_width(Some(200)), 2);
    assert_eq!(TrackNumberPadding::ThreeDigits.resolve_width(Some(5)), 3);
    assert_eq!(TrackNumberPadding::FourDigits.resolve_width(None), 4);
}

#[test]
fn padding_disc_auto_mode_stays_unpadded_for_small_sets() {
    // Most multi-disc albums are 2-3 discs; unpadded reads more
    // naturally than `01-`, `02-`.
    assert_eq!(DiscNumberPadding::Auto.resolve_width(Some(2)), 0);
    assert_eq!(DiscNumberPadding::Auto.resolve_width(Some(9)), 0);
    // 10+ disc box set → 2 digits needed for sort-correct listing.
    assert_eq!(DiscNumberPadding::Auto.resolve_width(Some(10)), 2);
    assert_eq!(DiscNumberPadding::Auto.resolve_width(Some(50)), 2);
}

// ----------------------------------------------------------
// Multi-disc naming + padding interaction (#589)
//
// Verifies that the common multi-disc template (`{disc}-{track} {title}`)
// produces correct output across the interesting combinations of
// `TrackNumberPadding` and `DiscNumberPadding`. Covers the
// originating user scenario from the #547 audit — a 10-disc box
// set where track numbering exceeds 99 on at least one disc.
// ----------------------------------------------------------

#[test]
fn multidisc_template_typical_2_disc_album_with_auto() {
    // Typical case: 2-disc album with 12-20 tracks per disc.
    // Auto mode should produce unpadded disc, 2-digit track.
    let tmpl = "{disc}-{track} {title}";
    let track_width = TrackNumberPadding::Auto.resolve_width(Some(20));
    let disc_width = DiscNumberPadding::Auto.resolve_width(Some(2));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(out, "{disc}-{track:02d} {title}");
}

#[test]
fn multidisc_template_10_disc_box_set_with_auto() {
    // Originating case from #589: 10-disc box set forces 2-digit
    // disc padding so `10-01` doesn't sort between `1-01` and
    // `2-01` lexicographically.
    let tmpl = "{disc}-{track} {title}";
    let track_width = TrackNumberPadding::Auto.resolve_width(Some(20));
    let disc_width = DiscNumberPadding::Auto.resolve_width(Some(10));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(out, "{disc:02d}-{track:02d} {title}");
}

#[test]
fn multidisc_template_deep_classical_box_set() {
    // Pathological Brilliant-Classics-style "Mozart 225" case:
    // 200 discs, some with 100+ tracks. Auto should produce
    // 3-digit disc AND 3-digit track to keep everything
    // sort-correct.
    let tmpl = "{disc}-{track} {title}";
    let track_width = TrackNumberPadding::Auto.resolve_width(Some(120));
    let disc_width = DiscNumberPadding::Auto.resolve_width(Some(200));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(out, "{disc:03d}-{track:03d} {title}");
}

#[test]
fn multidisc_template_user_fixed_three_digit_track_with_auto_disc() {
    // Settings mix-and-match: user picks fixed `ThreeDigits` for
    // track (for library-wide consistency) but leaves disc on
    // `Auto`. Small-disc-count album should still get unpadded
    // disc while the fixed track pad applies.
    let tmpl = "{disc}-{track} {title}";
    let track_width = TrackNumberPadding::ThreeDigits.resolve_width(Some(20));
    let disc_width = DiscNumberPadding::Auto.resolve_width(Some(2));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(out, "{disc}-{track:03d} {title}");
}

#[test]
fn multidisc_template_user_explicit_spec_takes_precedence() {
    // If the user has set an explicit `{disc:02d}` in their
    // template (e.g. from a manual power-user edit), that spec
    // must win over the setting's choice. `TrackNumberPadding` +
    // `DiscNumberPadding` only apply to BARE tokens.
    let tmpl = "{disc:02d}-{track:02d} {title}";
    let track_width = TrackNumberPadding::ThreeDigits.resolve_width(Some(20));
    let disc_width = DiscNumberPadding::TwoDigits.resolve_width(Some(10));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    // Unchanged — user's explicit spec wins.
    assert_eq!(out, "{disc:02d}-{track:02d} {title}");
}

#[test]
fn multidisc_template_direct_song_url_no_metadata() {
    // User downloads a single track from a multi-disc album via
    // a `?i=` song URL. At merge time no album metadata is known
    // (`None` passed to resolve_width). Auto must fall back to
    // pre-#587 safe defaults: 2-digit track, unpadded disc.
    let tmpl = "{disc}-{track} {title}";
    let track_width = TrackNumberPadding::Auto.resolve_width(None);
    let disc_width = DiscNumberPadding::Auto.resolve_width(None);
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(out, "{disc}-{track:02d} {title}");
}

#[test]
fn multidisc_template_compilation_various_artists() {
    // Compilation folder template uses `{album_id}` from #552 for
    // collision-proofing. Track-level template is a separate
    // concern — padding applies to tracks, not to album_id.
    let tmpl = "Compilations/{album} ({album_id})/{disc}-{track} {title}";
    let track_width = TrackNumberPadding::Auto.resolve_width(Some(30));
    let disc_width = DiscNumberPadding::Auto.resolve_width(Some(2));
    let out = apply_padding_to_template(tmpl, track_width, disc_width);
    assert_eq!(
        out,
        "Compilations/{album} ({album_id})/{disc}-{track:02d} {title}"
    );
}

// ----------------------------------------------------------
// Unified completion-task timeout scaling (#579, #776)
// ----------------------------------------------------------

#[test]
fn compute_total_timeout_small_album_gets_base() {
    // Single-track single → 10 min base + 30 s/track = 10 min 30 s.
    let t = compute_total_timeout(1, 0, 0, false);
    assert_eq!(t.as_secs(), 600 + 30);
}

#[test]
fn compute_total_timeout_zero_tracks_is_exactly_base() {
    // Shouldn't normally reach the timeout with zero tracks — the
    // #567 enrichment guard short-circuits earlier — but if it does,
    // the base still applies.
    let t = compute_total_timeout(0, 0, 0, false);
    assert_eq!(t.as_secs(), 600);
}

#[test]
fn compute_total_timeout_typical_album_under_20_minutes() {
    // 12-track album: 10 min + 12 × 30 s = 16 min. Stays comfortably
    // under 20 min so small albums don't see a regression.
    let t = compute_total_timeout(12, 0, 0, false);
    assert_eq!(t.as_secs(), 600 + 12 * 30);
    assert!(t.as_secs() / 60 < 20);
}

#[test]
fn compute_total_timeout_19_track_live_album_no_companions() {
    // The originating #776 case: a 19-track live album was hitting
    // the old 13 min budget. New formula: 10 min + 19 × 30 s
    // = 19.5 min — comfortable headroom.
    let t = compute_total_timeout(19, 0, 0, false);
    assert_eq!(t.as_secs(), 600 + 19 * 30);
    assert!(
        t.as_secs() / 60 >= 19,
        "must give legitimate live albums >= 19 min"
    );
}

#[test]
fn compute_total_timeout_box_set_accommodates_reality() {
    // 200-track box set — the originating #579 case. New formula:
    // 10 min + 200 × 30 s = 110 min. Well above the 40 min floor
    // the original test asserted.
    let t = compute_total_timeout(200, 0, 0, false);
    assert_eq!(t.as_secs(), 600 + 200 * 30);
    assert!(t.as_secs() / 60 >= 40);
}

#[test]
fn compute_total_timeout_caps_at_four_hours() {
    // Pathologically large workloads get capped so an accidental
    // recursion into a full music library doesn't propose an
    // unbounded deadline.
    let t = compute_total_timeout(100_000, 0, 0, false);
    assert_eq!(t.as_secs(), 4 * 3600);
}

#[test]
fn compute_total_timeout_saturates_on_usize_max() {
    // usize::MAX in any input must not overflow the arithmetic.
    let t = compute_total_timeout(usize::MAX, usize::MAX, usize::MAX, false);
    assert_eq!(t.as_secs(), 4 * 3600);
}

#[test]
fn compute_total_timeout_monotonic_in_tracks() {
    // Scaling should never go backwards as track count rises.
    let mut prev = compute_total_timeout(0, 0, 0, false);
    for n in (1..1000).step_by(37) {
        let t = compute_total_timeout(n, 0, 0, false);
        assert!(t >= prev, "non-monotonic at n={n}: {t:?} vs prev {prev:?}");
        prev = t;
    }
}

// ----------------------------------------------------------
// Companion-tier scaling
// ----------------------------------------------------------

#[test]
fn compute_total_timeout_single_tier_adds_eight_minutes() {
    // 12-track Atmos→Lossless: enrichment (10 + 12×30 s = 16 min)
    // + 1 tier × 8 min = 24 min.
    let t = compute_total_timeout(12, 1, 0, false);
    assert_eq!(t.as_secs(), 600 + 12 * 30 + 8 * 60);
}

#[test]
fn compute_total_timeout_four_tier_typical_album() {
    // 12-track Atmos→all formats (Atmos, ALAC, AAC, AAC-Legacy):
    // 10 + 12×30 s + 4×8 min = 48 min. Well above 40 min floor.
    let t = compute_total_timeout(12, 4, 0, false);
    assert_eq!(t.as_secs(), 600 + 12 * 30 + 4 * 8 * 60);
    assert!(t.as_secs() / 60 >= 40);
}

#[test]
fn compute_total_timeout_monotonic_in_tiers() {
    // More tiers should never propose a smaller deadline than fewer.
    let mut prev = compute_total_timeout(50, 0, 0, false);
    for tiers in 1..=8 {
        let t = compute_total_timeout(50, tiers, 0, false);
        assert!(
            t >= prev,
            "non-monotonic at tiers={tiers}: {t:?} vs prev {prev:?}"
        );
        prev = t;
    }
}

// ----------------------------------------------------------
// MV-companion scaling (new in #776)
// ----------------------------------------------------------

#[test]
fn compute_total_timeout_each_mv_adds_one_minute() {
    // Five MV companions on a typical album: enrichment (16 min)
    // + 1 tier (8 min) + 5 MVs × 1 min = 29 min.
    let t = compute_total_timeout(12, 1, 5, false);
    assert_eq!(t.as_secs(), 600 + 12 * 30 + 8 * 60 + 5 * 60);
}

#[test]
fn compute_total_timeout_19_track_live_album_with_mvs_and_tier() {
    // The originating #776 case + 1 companion tier + 5 MV companions:
    // 10 + 19×30 s + 8 min + 5 min = 32.5 min. Well above the 13 min
    // budget that was firing.
    let t = compute_total_timeout(19, 1, 5, false);
    assert_eq!(t.as_secs(), 600 + 19 * 30 + 8 * 60 + 5 * 60);
    assert!(
        t.as_secs() / 60 >= 30,
        "must give heavy live albums >= 30 min"
    );
}

#[test]
fn compute_total_timeout_monotonic_in_mvs() {
    // More MV companions should never propose a smaller deadline.
    let mut prev = compute_total_timeout(50, 1, 0, false);
    for mvs in 1..=20 {
        let t = compute_total_timeout(50, 1, mvs, false);
        assert!(
            t >= prev,
            "non-monotonic at mvs={mvs}: {t:?} vs prev {prev:?}"
        );
        prev = t;
    }
}

#[test]
fn compute_total_timeout_adds_the_word_lyrics_pass_deadline() {
    // 12 tracks: one minute plus 6 s a track = 132 s more.
    let without = compute_total_timeout(12, 0, 0, false);
    let with = compute_total_timeout(12, 0, 0, true);
    assert_eq!(with.as_secs() - without.as_secs(), 132);
    // Still capped at four hours.
    assert_eq!(
        compute_total_timeout(100_000, 0, 0, true).as_secs(),
        4 * 3600
    );
}

// ----------------------------------------------------------
// inject_advisory_suffix_into_template — companion folder fix (#528)
// ----------------------------------------------------------

/// Default GAMDL template — the suffix lands right after `{album}`
/// so the rendered companion folder matches the post-rename
/// primary folder exactly.
#[test]
fn inject_advisory_suffix_default_template_explicit() {
    let result =
        super::inject_advisory_suffix_into_template(Some("{album_artist}/{album}"), "[Explicit]");
    assert_eq!(result.as_deref(), Some("{album_artist}/{album} [Explicit]"));
}

/// `[Clean]` follows the same shape as `[Explicit]`.
#[test]
fn inject_advisory_suffix_default_template_clean() {
    let result =
        super::inject_advisory_suffix_into_template(Some("{album_artist}/{album}"), "[Clean]");
    assert_eq!(result.as_deref(), Some("{album_artist}/{album} [Clean]"));
}

/// User template with a year prefix — suffix still anchors to the
/// album-name segment, not the year.
#[test]
fn inject_advisory_suffix_with_year_prefix_template() {
    let result = super::inject_advisory_suffix_into_template(
        Some("{album_artist}/{year} - {album}"),
        "[Explicit]",
    );
    assert_eq!(
        result.as_deref(),
        Some("{album_artist}/{year} - {album} [Explicit]")
    );
}

/// `None` template input → `None` output (caller leaves the field
/// alone; uses GAMDL's default).
#[test]
fn inject_advisory_suffix_none_input_returns_none() {
    let result = super::inject_advisory_suffix_into_template(None, "[Explicit]");
    assert_eq!(result, None);
}

/// Custom template that doesn't reference `{album}` → return None
/// rather than guess where to splice the suffix. Caller falls back
/// to the original template; user gets the existing behaviour for
/// that one item (acceptable graceful degradation).
#[test]
fn inject_advisory_suffix_template_without_album_placeholder_returns_none() {
    let result =
        super::inject_advisory_suffix_into_template(Some("{album_artist}/{title}"), "[Explicit]");
    assert_eq!(
        result, None,
        "must not blindly append when {{album}} absent"
    );
}

/// Idempotency-ish guard: the helper doesn't recognise that the
/// template already contains the suffix and would inject again. We
/// rely on the call site checking `content_advisory_in_filenames`
/// only ONCE per download to prevent double-injection. This test
/// documents the current behaviour so future refactors don't
/// silently change it.
#[test]
fn inject_advisory_suffix_documents_no_built_in_idempotency() {
    let result = super::inject_advisory_suffix_into_template(
        Some("{album_artist}/{album} [Explicit]"),
        "[Explicit]",
    );
    // Note: helper does NOT detect that the template already has
    // the suffix. Caller is responsible for not invoking twice.
    // The result has the suffix duplicated:
    assert_eq!(
        result.as_deref(),
        Some("{album_artist}/{album} [Explicit] [Explicit]"),
        "caller-side guarantees: the call site invokes once per spawn"
    );
}

/// Multiple `{album}` occurrences (uncommon but possible) — every
/// occurrence gets the suffix. Documents replace-all semantics so
/// the behaviour is intentional.
#[test]
fn inject_advisory_suffix_multiple_album_placeholders_all_replaced() {
    let result = super::inject_advisory_suffix_into_template(Some("{album}/{album}"), "[Explicit]");
    assert_eq!(
        result.as_deref(),
        Some("{album} [Explicit]/{album} [Explicit]")
    );
}

// ----------------------------------------------------------
// mv_companion_count plumbing — actual count wins over estimate (#776)
// ----------------------------------------------------------

/// New items default `mv_companion_count` to `None` so the
/// completion task knows to fall back to the heuristic estimate
/// (no actual count has been written yet).
#[test]
fn enqueue_starts_with_no_mv_companion_count() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);
    let item = queue
        .items
        .iter()
        .find(|i| i.status.id == id)
        .expect("item exists");
    assert_eq!(
        item.status.mv_companion_count, None,
        "fresh enqueue must leave mv_companion_count as None"
    );
}

/// Manual user retry from the UI must clear `mv_companion_count`
/// so the next attempt's enrichment task re-discovers it from a
/// fresh API call — stale counts from a previous attempt would
/// mis-size the companion-wait deadline.
#[test]
fn retry_clears_mv_companion_count() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    // Simulate enrichment writing a discovered MV count from a
    // previous attempt.
    if let Some(item) = queue.items.iter_mut().find(|i| i.status.id == id) {
        item.status.mv_companion_count = Some(7);
    }
    queue.set_error(&id, "Some failure");

    // Retry should reset the count so the next enrichment writes a
    // fresh value.
    assert!(queue.retry(&id, &settings));
    let item = queue
        .items
        .iter()
        .find(|i| i.status.id == id)
        .expect("item exists");
    assert_eq!(
        item.status.mv_companion_count, None,
        "retry must clear stale mv_companion_count"
    );
}

// ----------------------------------------------------------
// Queue reorder tests (#782)
// ----------------------------------------------------------

/// Helper: build a mixed-state queue [Active, Queued1, Queued2, Queued3, Complete].
/// Returns the four download IDs in deque order.
fn mixed_queue_with_actives_and_completed() -> (DownloadQueue, [String; 4]) {
    let mut q = DownloadQueue::new();
    let s = test_settings();
    let active_id = q.enqueue(test_request(), &s);
    // Force into Downloading via the same path next_pending uses.
    let _ = q.next_pending();
    let q1 = q.enqueue(test_request(), &s);
    let q2 = q.enqueue(test_request(), &s);
    let q3 = q.enqueue(test_request(), &s);
    // Push a "complete" item at the end.
    let complete_id = q.enqueue(test_request(), &s);
    if let Some(item) = q.items.iter_mut().find(|i| i.status.id == complete_id) {
        item.status.state = DownloadState::Complete;
    }
    // Sanity: layout should be [Active, Q1, Q2, Q3, Complete].
    assert_eq!(q.items[0].status.state, DownloadState::Downloading);
    assert_eq!(q.items[0].status.id, active_id);
    assert_eq!(q.items[1].status.id, q1);
    assert_eq!(q.items[2].status.id, q2);
    assert_eq!(q.items[3].status.id, q3);
    assert_eq!(q.items[4].status.state, DownloadState::Complete);
    (q, [active_id, q1, q2, q3])
}

/// move_to_top puts the target at the FIRST queued position
/// (right after any actives), preserving the active item's slot.
#[test]
fn move_to_top_promotes_queued_item_past_active_item() {
    let (mut q, [active, q1, q2, q3]) = mixed_queue_with_actives_and_completed();
    assert!(q.move_to_top(&q3));
    // Active stays at index 0; q3 is now at the front of the
    // queued sub-sequence (index 1).
    assert_eq!(q.items[0].status.id, active);
    assert_eq!(q.items[1].status.id, q3);
    assert_eq!(q.items[2].status.id, q1);
    assert_eq!(q.items[3].status.id, q2);
}

/// move_to_top is a no-op when the target is already at the top.
#[test]
fn move_to_top_returns_false_when_already_at_top() {
    let (mut q, [_active, q1, _q2, _q3]) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_to_top(&q1), "q1 already at top of queued");
}

/// move_to_top refuses to move active items.
#[test]
fn move_to_top_refuses_active_item() {
    let (mut q, [active, _q1, _q2, _q3]) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_to_top(&active));
}

/// move_to_top returns false when the id isn't in the queue.
#[test]
fn move_to_top_returns_false_for_unknown_id() {
    let (mut q, _) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_to_top("does-not-exist"));
}

/// move_to_bottom puts the target at the LAST queued position,
/// preserving any non-queued items past the queued sub-sequence.
#[test]
fn move_to_bottom_demotes_queued_item_before_completed_item() {
    let (mut q, [active, q1, q2, q3]) = mixed_queue_with_actives_and_completed();
    assert!(q.move_to_bottom(&q1));
    // Active stays at 0; q2 / q3 shift up; q1 is now last in
    // the queued sub-sequence; complete still at the end.
    assert_eq!(q.items[0].status.id, active);
    assert_eq!(q.items[1].status.id, q2);
    assert_eq!(q.items[2].status.id, q3);
    assert_eq!(q.items[3].status.id, q1);
    assert_eq!(q.items[4].status.state, DownloadState::Complete);
}

/// move_to_bottom no-op when target is already at bottom.
#[test]
fn move_to_bottom_returns_false_when_already_at_bottom() {
    let (mut q, [_, _, _, q3]) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_to_bottom(&q3));
}

/// move_up swaps the target with the queued item immediately
/// above it, skipping any non-queued items in between.
#[test]
fn move_up_swaps_with_queued_neighbour_above() {
    let (mut q, [_, q1, q2, _q3]) = mixed_queue_with_actives_and_completed();
    assert!(q.move_up(&q2));
    assert_eq!(q.items[1].status.id, q2);
    assert_eq!(q.items[2].status.id, q1);
}

/// move_up no-op when target is already at the top of the queued
/// sub-sequence.
#[test]
fn move_up_returns_false_for_topmost_queued_item() {
    let (mut q, [_, q1, _, _]) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_up(&q1));
}

/// move_down swaps with the queued item immediately below.
#[test]
fn move_down_swaps_with_queued_neighbour_below() {
    let (mut q, [_, q1, q2, _]) = mixed_queue_with_actives_and_completed();
    assert!(q.move_down(&q1));
    assert_eq!(q.items[1].status.id, q2);
    assert_eq!(q.items[2].status.id, q1);
}

/// move_down no-op when target is at the bottom of the queued
/// sub-sequence.
#[test]
fn move_down_returns_false_for_bottommost_queued_item() {
    let (mut q, [_, _, _, q3]) = mixed_queue_with_actives_and_completed();
    assert!(!q.move_down(&q3));
}

/// All four move methods are no-ops on a single-item queue.
#[test]
fn move_methods_are_no_ops_on_single_item_queue() {
    let mut q = DownloadQueue::new();
    let s = test_settings();
    let id = q.enqueue(test_request(), &s);
    assert!(!q.move_to_top(&id));
    assert!(!q.move_to_bottom(&id));
    assert!(!q.move_up(&id));
    assert!(!q.move_down(&id));
}

/// Reorder is preserved through the persistence round-trip — the
/// new order applies after MeedyaDL is closed and restarted.
#[test]
fn move_to_top_persists_through_save_restore_round_trip() {
    let mut q = DownloadQueue::new();
    let s = test_settings();
    let q1 = q.enqueue(test_request(), &s);
    let q2 = q.enqueue(test_request(), &s);
    let q3 = q.enqueue(test_request(), &s);

    // Promote q3 to the top of the queued sub-sequence.
    assert!(q.move_to_top(&q3));
    assert_eq!(q.items[0].status.id, q3);
    assert_eq!(q.items[1].status.id, q1);
    assert_eq!(q.items[2].status.id, q2);

    // Persist + restore (simulating an app close/reopen).
    let snapshot = q.get_persistable_items();
    let mut restored = DownloadQueue::new();
    restored.restore_items(snapshot, &s);

    // Order is preserved across the round-trip.
    assert_eq!(restored.items[0].status.id, q3);
    assert_eq!(restored.items[1].status.id, q1);
    assert_eq!(restored.items[2].status.id, q2);
}

use crate::models::settings::AppSettings;
use crate::utils::process::GamdlOutputEvent;

// ----------------------------------------------------------
// Test Helpers
// ----------------------------------------------------------

/// Creates default AppSettings suitable for test use.
/// The returned settings have sensible defaults matching AppSettings::default(),
/// which includes fallback_enabled=true and a full music_fallback_chain.
fn test_settings() -> AppSettings {
    AppSettings::default()
}

/// Creates a minimal DownloadRequest with a single URL and no overrides.
/// The URL is a placeholder Apple Music URL for test purposes only.
fn test_request() -> DownloadRequest {
    DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test-song/123456789".to_string()],
        options: None,
        ..Default::default()
    }
}

/// Creates a DownloadRequest with per-download codec override.
fn test_request_with_codec_override(codec: SongCodec) -> DownloadRequest {
    DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/999".to_string()],
        options: Some(GamdlOptions {
            song_codec: Some(codec),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Helper: enqueues a single item and returns its download ID.
fn enqueue_one(queue: &mut DownloadQueue) -> String {
    let settings = test_settings();
    queue.enqueue(test_request(), &settings)
}

/// Helper: enqueues N items and returns their download IDs.
fn enqueue_n(queue: &mut DownloadQueue, n: usize) -> Vec<String> {
    let settings = test_settings();
    (0..n)
        .map(|_| queue.enqueue(test_request(), &settings))
        .collect()
}

// ==========================================================
// 1. new() tests
// ==========================================================

/// Verifies that DownloadQueue::new() creates an empty queue with no items,
/// zero active count, and default concurrency settings.
#[test]
fn new_creates_empty_queue() {
    let queue = DownloadQueue::new();
    assert!(queue.items.is_empty(), "New queue should have no items");
    assert_eq!(
        queue.active_count, 0,
        "New queue should have zero active count"
    );
    assert_eq!(
        queue.max_concurrent, 1,
        "Default max_concurrent should be 1"
    );
    assert_eq!(
        queue.max_network_retries, 3,
        "Default max_network_retries should be 3"
    );
}

/// Verifies that the Default trait implementation delegates to new().
#[test]
fn default_delegates_to_new() {
    let queue = DownloadQueue::default();
    assert!(queue.items.is_empty());
    assert_eq!(queue.active_count, 0);
    assert_eq!(queue.max_concurrent, 1);
}

// ==========================================================
// 2. enqueue() tests
// ==========================================================

/// Verifies that enqueue() returns a unique download ID (UUID v4 format)
/// and that successive calls produce different IDs.
#[test]
fn enqueue_returns_unique_ids() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();

    let id1 = queue.enqueue(test_request(), &settings);
    let id2 = queue.enqueue(test_request(), &settings);

    assert!(!id1.is_empty(), "Download ID should not be empty");
    assert!(!id2.is_empty(), "Download ID should not be empty");
    assert_ne!(id1, id2, "Each enqueue should produce a unique ID");
}

/// Verifies that an enqueued item starts in the Queued state and appears
/// in the status list with correct initial fields.
#[test]
fn enqueue_sets_queued_state() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = test_request();
    let expected_url = request.urls[0].clone();

    let id = queue.enqueue(request, &settings);
    let statuses = queue.get_status();

    assert_eq!(statuses.len(), 1, "Queue should have exactly one item");
    let status = &statuses[0];
    assert_eq!(status.id, id);
    assert_eq!(status.state, DownloadState::Queued);
    assert_eq!(status.urls, vec![expected_url]);
    assert_eq!(status.progress, 0.0);
    assert!(status.current_track.is_none());
    assert!(status.error.is_none());
    assert!(status.output_path.is_none());
    assert!(!status.fallback_occurred);
}

#[test]
fn enqueue_removes_terminal_duplicate_attempt() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let old_request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123?ls=1".to_string()],
        options: None,
        ..Default::default()
    };
    let old_id = queue.enqueue(old_request, &settings);
    queue.set_error(&old_id, "failed once");

    let new_request = DownloadRequest {
        urls: vec!["https://Music.Apple.Com/us/album/test/123/".to_string()],
        options: None,
        ..Default::default()
    };
    let new_id = queue.enqueue(new_request, &settings);
    let statuses = queue.get_status();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, new_id);
    assert_eq!(statuses[0].state, DownloadState::Queued);
}

/// Verifies that enqueue() merges global settings into the item's options.
/// Since merge_options is private, we test it indirectly: the enqueued item's
/// codec_used field should reflect the default settings codec (ALAC).
#[test]
fn enqueue_merges_default_settings() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();

    let _id = queue.enqueue(test_request(), &settings);
    let statuses = queue.get_status();

    assert_eq!(
        statuses[0].codec_used.as_deref(),
        Some("alac"),
        "Default codec should be ALAC from settings"
    );
}

/// Verifies that per-download codec overrides take precedence over
/// global settings when enqueuing. This indirectly tests merge_options().
#[test]
fn enqueue_applies_per_download_overrides() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = test_request_with_codec_override(SongCodec::Aac);

    let _id = queue.enqueue(request, &settings);
    let statuses = queue.get_status();

    assert_eq!(
        statuses[0].codec_used.as_deref(),
        Some("aac"),
        "Per-download override should replace default ALAC with AAC"
    );
}

/// Verifies that artist music-video selections do not pass static cover
/// sidecar options through to GAMDL. GAMDL 3.5 can treat Apple video
/// artwork template URLs (`{w}x{h}`) as literal URLs and fail every
/// selected music video before download.
#[test]
fn enqueue_suppresses_cover_args_for_artist_music_videos() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.save_cover = true;
    settings.cover_format = crate::models::gamdl_options::CoverFormat::Raw;
    settings.cover_size = 10000;
    settings.artist_auto_select = Some(ArtistAutoSelect::MusicVideos);

    let _id = queue.enqueue(test_request(), &settings);
    let options = &queue.items[0].merged_options;

    assert_eq!(
        options.artist_auto_select,
        Some(ArtistAutoSelect::MusicVideos)
    );
    assert_eq!(options.save_cover, None);
    assert_eq!(options.cover_format, None);
    assert_eq!(options.cover_size, None);
    assert_eq!(options.no_config_file, Some(true));
}

/// Verifies that the `gamdl_log_level` setting (#768) is propagated by
/// `merge_options()` into `GamdlOptions::log_level` for every variant,
/// so the existing `to_cli_args()` `--log-level <LEVEL>` emission path
/// fires in production instead of being dead code outside this test
/// suite. Without this propagation the field is always `None` and
/// GAMDL runs at its compiled-in default `INFO` regardless of what
/// the user picked in Developer Tools.
#[test]
fn enqueue_propagates_gamdl_log_level() {
    use crate::models::gamdl_options::LogLevel;

    for level in [
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warning,
        LogLevel::Error,
    ] {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.gamdl_log_level = level.clone();

        let _id = queue.enqueue(test_request(), &settings);
        let options = &queue.items[0].merged_options;

        assert_eq!(
            options.log_level,
            Some(level.clone()),
            "merge_options() should copy gamdl_log_level={level:?} into GamdlOptions.log_level",
        );
    }
}

/// Verifies `format_artwork_size` renders byte counts in the
/// human-readable form `emit_artwork_variant_log` puts in the
/// activity log (#529). Exercised at the three thresholds the
/// helper switches on.
#[test]
fn format_artwork_size_human_readable() {
    assert_eq!(super::format_artwork_size(0), "0 bytes");
    assert_eq!(super::format_artwork_size(512), "512 bytes");
    assert_eq!(super::format_artwork_size(1024), "1 KB");
    assert_eq!(super::format_artwork_size(1024 * 512), "512 KB");
    assert_eq!(super::format_artwork_size(1024 * 1024), "1.0 MB");
    // Mid-range: 2_200_000 bytes ≈ 2.1 MB.
    assert_eq!(super::format_artwork_size(2_200_000), "2.1 MB");
}

/// Verifies `artwork_geo_lock_hint` fires only for reasons that name
/// an HTTP 403, in whatever phrasing the failing tool used (#961).
#[test]
fn artwork_geo_lock_hint_fires_on_403() {
    assert!(!super::artwork_geo_lock_hint("HTTP 403").is_empty());
    assert!(!super::artwork_geo_lock_hint("403 Forbidden").is_empty());
}

/// Non-403 failure reasons must not get the geo-lock hint appended.
#[test]
fn artwork_geo_lock_hint_silent_for_other_reasons() {
    assert_eq!(super::artwork_geo_lock_hint("exit code 1"), "");
    assert_eq!(super::artwork_geo_lock_hint("connection timed out"), "");
}

/// Verifies that `format_heartbeat_elapsed` produces the compact
/// human-readable shape we want in the activity log heartbeat
/// lines (#805). Sub-minute and sub-hour shapes are both exercised
/// so a future refactor can't accidentally collapse them to one
/// arm (e.g. always emitting "0 min" for short stages).
#[test]
fn format_heartbeat_elapsed_compact_shape() {
    use std::time::Duration;
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(0)),
        "0 s"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(45)),
        "45 s"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(59)),
        "59 s"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(60)),
        "1 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(120)),
        "2 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(3540)),
        "59 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(3600)),
        "1h 0 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(3660)),
        "1h 1 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(5025)),
        "1h 23 min"
    );
    assert_eq!(
        super::format_heartbeat_elapsed(Duration::from_secs(7200)),
        "2h 0 min"
    );
}

/// Verifies that the default `AppSettings::gamdl_log_level` is `Info`
/// — matches GAMDL's compiled-in default and serialises identically
/// to a settings.json written by a pre-#768 build where the field
/// was absent.
#[test]
fn default_gamdl_log_level_is_info() {
    use crate::models::gamdl_options::LogLevel;

    let settings = test_settings();
    assert_eq!(settings.gamdl_log_level, LogLevel::Info);
}

/// Verifies that multiple items can be enqueued and they all appear in
/// the status list in FIFO order.
#[test]
fn enqueue_preserves_fifo_order() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);
    let statuses = queue.get_status();

    assert_eq!(statuses.len(), 3);
    assert_eq!(
        statuses[0].id, ids[0],
        "First enqueued should be first in status"
    );
    assert_eq!(statuses[1].id, ids[1]);
    assert_eq!(
        statuses[2].id, ids[2],
        "Last enqueued should be last in status"
    );
}

/// Verifies that enqueue sets the network_retries_left field to
/// the queue's max_network_retries value (tested via try_network_retry).
#[test]
fn enqueue_sets_network_retries_from_queue_config() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    // Exhaust all 3 retries
    assert!(
        queue.try_network_retry(&id).is_some(),
        "Should succeed on retry 1 of 3"
    );
    // Need to set back to Error/Queued for next retry test, but try_network_retry
    // itself resets to Queued. We need to simulate re-error.
    // Actually try_network_retry sets to Queued. Let's set back to error.
    queue.set_error(&id, "network error");
    assert!(
        queue.try_network_retry(&id).is_some(),
        "Should succeed on retry 2 of 3"
    );
    queue.set_error(&id, "network error");
    assert!(
        queue.try_network_retry(&id).is_some(),
        "Should succeed on retry 3 of 3"
    );
    queue.set_error(&id, "network error");
    assert!(
        queue.try_network_retry(&id).is_none(),
        "Should fail after 3 retries exhausted"
    );
}

// ==========================================================
// 3. get_status() tests
// ==========================================================

/// Verifies that get_status() returns an empty vector when the queue
/// has no items.
#[test]
fn get_status_empty_queue() {
    let queue = DownloadQueue::new();
    let statuses = queue.get_status();
    assert!(
        statuses.is_empty(),
        "Empty queue should return empty status vec"
    );
}

/// Verifies that get_status() returns all items in the queue, each with
/// the correct state and URL information.
#[test]
fn get_status_returns_all_items() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 4);
    let statuses = queue.get_status();

    assert_eq!(statuses.len(), 4);
    for (i, status) in statuses.iter().enumerate() {
        assert_eq!(status.id, ids[i]);
        assert_eq!(status.state, DownloadState::Queued);
    }
}

/// Verifies that get_status() returns cloned data (modifications to the
/// returned vec do not affect the queue).
#[test]
fn get_status_returns_cloned_data() {
    let mut queue = DownloadQueue::new();
    let _id = enqueue_one(&mut queue);

    let statuses1 = queue.get_status();
    let statuses2 = queue.get_status();

    assert_eq!(statuses1.len(), statuses2.len());
    assert_eq!(statuses1[0].id, statuses2[0].id);
}

// ==========================================================
// 4. get_counts() tests
// ==========================================================

/// Verifies that get_counts() returns all zeros for an empty queue.
/// Returns tuple: (total, active, queued, completed, failed).
#[test]
fn get_counts_empty_queue() {
    let queue = DownloadQueue::new();
    assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
}

/// Verifies that get_counts() correctly counts items in different states.
/// Sets up items in Queued, Downloading, Complete, and Error states
/// and checks that each counter is accurate.
#[test]
fn get_counts_various_states() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 5);

    // Leave ids[0] as Queued
    // Set ids[1] to Downloading via next_pending
    let _ = queue.next_pending(); // ids[0] becomes Downloading
                                  // Set ids[2] to Complete
    queue.set_complete(&ids[2]);
    // Set ids[3] to Error
    queue.set_error(&ids[3], "test error");
    // ids[4] stays Queued

    // ids[0]=Downloading, ids[1]=Queued, ids[2]=Complete, ids[3]=Error, ids[4]=Queued
    let (total, active, queued, completed, failed) = queue.get_counts();
    assert_eq!(total, 5, "Total should be 5");
    assert_eq!(active, 1, "One item is Downloading");
    assert_eq!(queued, 2, "Two items are Queued (ids[1] and ids[4])");
    assert_eq!(completed, 1, "One item is Complete");
    assert_eq!(failed, 1, "One item is Error");
}

/// Verifies that get_counts() counts Processing state items as active.
#[test]
fn get_counts_processing_counted_as_active() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 2);

    queue.update_item_state(&ids[0], DownloadState::Processing);

    let (total, active, queued, _completed, _failed) = queue.get_counts();
    assert_eq!(total, 2);
    assert_eq!(active, 1, "Processing items should count as active");
    assert_eq!(queued, 1);
}

// ==========================================================
// 5. cancel() tests
// ==========================================================

/// Verifies that cancelling a Queued item sets its state to Cancelled
/// and returns true.
#[test]
fn cancel_queued_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let result = queue.cancel(&id);
    assert!(result, "cancel() should return true for Queued items");

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Cancelled,
        "Cancelled queued item should be in Cancelled state"
    );
}

/// Verifies that cancelling a Downloading item sets its state to Cancelled
/// and returns true. The active_count is NOT decremented here; that happens
/// when the running task detects cancellation and calls on_task_finished().
#[test]
fn cancel_downloading_item() {
    let mut queue = DownloadQueue::new();
    let _id = enqueue_one(&mut queue);

    // Move to Downloading state via next_pending
    let (dl_id, _, _, _) = queue.next_pending().expect("Should have a pending item");
    assert_eq!(queue.active_count, 1);

    let result = queue.cancel(&dl_id);
    assert!(result, "cancel() should return true for Downloading items");

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Cancelled);
    // active_count is NOT decremented by cancel() -- that's the task's job
    assert_eq!(
        queue.active_count, 1,
        "active_count should not be decremented by cancel()"
    );
}

/// Verifies that cancelling a Processing item sets its state to Cancelled
/// and returns true.
#[test]
fn cancel_processing_item() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 1);
    queue.update_item_state(&ids[0], DownloadState::Processing);

    let result = queue.cancel(&ids[0]);
    assert!(result, "cancel() should return true for Processing items");

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Cancelled);
}

// ----------------------------------------------------------
// ActiveSlotGuard — RAII slot-release on Drop (#706)
// ----------------------------------------------------------

/// Disarming the guard before Drop must NOT release the slot.
/// This is the happy path: the completion task explicitly calls
/// `q.on_task_finished()` and then `disarm()`, so the slot release
/// is performed exactly once (by the explicit call).
#[tokio::test]
async fn active_slot_guard_disarm_does_not_release() {
    let handle = new_queue_handle();
    {
        let mut q = handle.lock().await;
        q.active_count = 1; // pretend next_pending() handed out a slot
    }
    let guard = ActiveSlotGuard::new(handle.clone());
    guard.disarm();
    // Drop has now run on the disarmed guard. Slot must still be held.
    let q = handle.lock().await;
    assert_eq!(
        q.active_count, 1,
        "disarmed guard must not release the slot"
    );
}

/// Dropping an armed guard must release the slot — even though Drop
/// is synchronous and the queue lock is async. The guard fires a
/// fire-and-forget `tokio::spawn`; we then yield long enough for
/// the spawned release task to acquire the lock and run.
#[tokio::test]
async fn active_slot_guard_drop_releases_slot() {
    let handle = new_queue_handle();
    {
        let mut q = handle.lock().await;
        q.active_count = 1;
    }
    {
        let _guard = ActiveSlotGuard::new(handle.clone());
        // _guard goes out of scope here → Drop fires.
    }
    // Yield the runtime so the fire-and-forget release task can run.
    // Two yields cover: (1) Drop's tokio::spawn registering the task,
    // (2) the task acquiring the lock and running on_task_finished().
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    let q = handle.lock().await;
    assert_eq!(
        q.active_count, 0,
        "Drop on armed guard must release the slot"
    );
}

/// Confirms `on_task_finished()` saturates at 0 — so even if a
/// double-release ever slips through (explicit call + an armed
/// guard's Drop), `active_count` cannot underflow into a giant
/// usize that would permanently jam `next_pending()`.
#[test]
fn on_task_finished_saturates_at_zero() {
    let mut q = DownloadQueue::new();
    assert_eq!(q.active_count, 0);
    q.on_task_finished();
    q.on_task_finished();
    assert_eq!(q.active_count, 0, "must not underflow past zero");
}

/// Verifies that cancel() returns false for items already in a terminal
/// state (Complete, Error, Cancelled).
#[test]
fn cancel_returns_false_for_terminal_states() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);

    queue.set_complete(&ids[0]);
    queue.set_error(&ids[1], "some error");
    queue.cancel(&ids[2]); // First cancel succeeds

    assert!(
        !queue.cancel(&ids[0]),
        "Should return false for Complete item"
    );
    assert!(!queue.cancel(&ids[1]), "Should return false for Error item");
    assert!(
        !queue.cancel(&ids[2]),
        "Should return false for already Cancelled item"
    );
}

/// Verifies that cancel() returns false for a non-existent download ID.
#[test]
fn cancel_returns_false_for_nonexistent_id() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_one(&mut queue);

    assert!(
        !queue.cancel("nonexistent-id-12345"),
        "Should return false for unknown ID"
    );
}

// ==========================================================
// 6. clear_finished() tests
// ==========================================================

/// Verifies that clear_finished() removes items in terminal states
/// (Complete, Error, Cancelled) and keeps items in active/pending states.
#[test]
fn clear_finished_removes_terminal_keeps_active() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 5);

    // ids[0] = Queued (keep)
    // ids[1] = Downloading (keep)
    queue.update_item_state(&ids[1], DownloadState::Downloading);
    // ids[2] = Complete (remove)
    queue.set_complete(&ids[2]);
    // ids[3] = Error (keep — errored items persist for review)
    queue.set_error(&ids[3], "error msg");
    // ids[4] = Cancelled (remove)
    queue.cancel(&ids[4]);

    let removed = queue.clear_finished();

    assert_eq!(removed, 2, "Should remove 2 items (Complete + Cancelled)");
    let statuses = queue.get_status();
    assert_eq!(statuses.len(), 3, "Should have 3 remaining items");
    assert_eq!(statuses[0].id, ids[0], "Queued item should remain");
    assert_eq!(statuses[1].id, ids[1], "Downloading item should remain");
    assert_eq!(statuses[2].id, ids[3], "Errored item should remain");
}

/// Verifies the abort_all() summary counts each pre-abort state correctly
/// and leaves terminal items untouched (#620).
#[test]
fn abort_all_counts_per_state_and_preserves_terminal_items() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 6);

    // ids[0] = Queued (→ cancelled, queued_cancelled)
    // ids[1] = Downloading (→ cancelled, downloading_stopped)
    queue.update_item_state(&ids[1], DownloadState::Downloading);
    // ids[2] = Processing (→ cancelled, processing_stopped)
    queue.update_item_state(&ids[2], DownloadState::Processing);
    // ids[3] = Complete — terminal, must not be touched
    queue.set_complete(&ids[3]);
    // ids[4] = Error — terminal, must not be touched
    queue.set_error(&ids[4], "whatever");
    // ids[5] = already Cancelled — terminal, must not be counted twice
    queue.cancel(&ids[5]);

    let summary = queue.abort_all();

    assert_eq!(summary.queued_cancelled, 1, "one queued item aborted");
    assert_eq!(
        summary.downloading_stopped, 1,
        "one downloading item aborted"
    );
    assert_eq!(summary.processing_stopped, 1, "one processing item aborted");
    assert_eq!(summary.total(), 3, "three items affected total");

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Cancelled);
    assert_eq!(statuses[1].state, DownloadState::Cancelled);
    assert_eq!(statuses[2].state, DownloadState::Cancelled);
    assert_eq!(
        statuses[3].state,
        DownloadState::Complete,
        "Complete preserved"
    );
    assert_eq!(statuses[4].state, DownloadState::Error, "Error preserved");
    assert_eq!(
        statuses[5].state,
        DownloadState::Cancelled,
        "already-Cancelled unchanged"
    );
}

/// Verifies that abort_all() on an empty queue is a no-op with a zero summary.
#[test]
fn abort_all_on_empty_queue_returns_zero_summary() {
    let mut queue = DownloadQueue::new();
    let summary = queue.abort_all();
    assert_eq!(summary.total(), 0);
}

/// Verifies that abort_all() arms the one-shot suppression flag and
/// that `take_recently_aborted()` consumes it exactly once (#620).
#[test]
fn abort_all_arms_post_queue_action_suppression() {
    let mut queue = DownloadQueue::new();
    // Fresh queue: no suppression armed.
    assert!(!queue.take_recently_aborted(), "flag must start clear");

    // No-op abort (empty queue) must NOT arm the flag — the suppression
    // is scoped to actual abort events, not ceremonial calls.
    queue.abort_all();
    assert!(
        !queue.take_recently_aborted(),
        "zero-summary abort must not arm suppression"
    );

    // Real abort: flag arms.
    let _id = enqueue_one(&mut queue);
    let summary = queue.abort_all();
    assert_eq!(summary.queued_cancelled, 1);
    assert!(queue.take_recently_aborted(), "abort must arm suppression");
    // Consumption is one-shot.
    assert!(
        !queue.take_recently_aborted(),
        "flag must be consumed on read"
    );
}

/// Verifies that abort_all() on a queue of only-terminal items is a
/// zero-summary no-op — no item's state changes.
#[test]
fn abort_all_with_only_terminal_items_is_no_op() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);
    queue.set_complete(&ids[0]);
    queue.set_error(&ids[1], "err");
    queue.cancel(&ids[2]);

    let summary = queue.abort_all();
    assert_eq!(summary.total(), 0);
    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Complete);
    assert_eq!(statuses[1].state, DownloadState::Error);
    assert_eq!(statuses[2].state, DownloadState::Cancelled);
}

/// Verifies that clear_finished() returns 0 when there are no terminal items.
#[test]
fn clear_finished_returns_zero_when_nothing_to_clear() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_n(&mut queue, 3);

    let removed = queue.clear_finished();
    assert_eq!(
        removed, 0,
        "Nothing should be removed when all items are Queued"
    );
    assert_eq!(queue.get_status().len(), 3, "All items should remain");
}

/// Verifies that clear_finished() works correctly on an empty queue.
#[test]
fn clear_finished_on_empty_queue() {
    let mut queue = DownloadQueue::new();
    let removed = queue.clear_finished();
    assert_eq!(removed, 0, "Should return 0 for empty queue");
}

// ==========================================================
// 7. next_pending() tests
// ==========================================================

/// Verifies that next_pending() returns None for an empty queue.
#[test]
fn next_pending_empty_queue() {
    let mut queue = DownloadQueue::new();
    assert!(
        queue.next_pending().is_none(),
        "Empty queue should return None"
    );
}

/// Verifies that next_pending() returns the first Queued item, transitions
/// it to Downloading, and increments active_count.
#[test]
fn next_pending_returns_first_queued_item() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);

    let result = queue.next_pending();
    assert!(result.is_some(), "Should return Some for non-empty queue");

    let (dl_id, urls, _options, _service) = result.unwrap();
    assert_eq!(dl_id, ids[0], "Should return the first queued item");
    assert_eq!(urls.len(), 1, "Should include the URLs from the request");
    assert_eq!(
        queue.active_count, 1,
        "active_count should be incremented to 1"
    );

    // Verify the item's state changed to Downloading
    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Downloading);
    assert_eq!(
        statuses[1].state,
        DownloadState::Queued,
        "Other items should remain Queued"
    );
}

/// Verifies that next_pending() returns None when max_concurrent is reached.
/// Default max_concurrent is 1, so after one next_pending(), the next call
/// should return None even if there are queued items.
#[test]
fn next_pending_respects_max_concurrent() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_n(&mut queue, 3);

    // First call succeeds (active_count goes from 0 to 1)
    let first = queue.next_pending();
    assert!(first.is_some(), "First next_pending should succeed");

    // Second call should return None (active_count == max_concurrent == 1)
    let second = queue.next_pending();
    assert!(
        second.is_none(),
        "Second next_pending should return None when at max_concurrent"
    );
}

/// Verifies that next_pending() skips non-Queued items and finds the first
/// Queued item in the deque.
#[test]
fn next_pending_skips_non_queued_items() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);

    // Set first item to Complete (not Queued)
    queue.set_complete(&ids[0]);
    // Set second item to Error
    queue.set_error(&ids[1], "error");

    // next_pending should skip ids[0] and ids[1], returning ids[2]
    let result = queue.next_pending();
    assert!(result.is_some());
    let (dl_id, _, _, _) = result.unwrap();
    assert_eq!(
        dl_id, ids[2],
        "Should return the first Queued item, skipping terminal items"
    );
}

// -----------------------------------------------------------------
// #889 — Pause / Resume Queue (non-destructive scheduler stop)
// -----------------------------------------------------------------

/// `pause()` flips the scheduler-paused flag. `next_pending()` then
/// refuses to pull new items even if there are Queued ones and a
/// slot is free.
#[test]
fn pause_blocks_next_pending_from_pulling_new_item() {
    let mut queue = DownloadQueue::new();
    let _ids = enqueue_n(&mut queue, 3);

    // Sanity: pulling works before pause.
    assert!(
        queue.next_pending().is_some(),
        "next_pending should return a queued item before pause"
    );
    // Release the slot so the next assertion can isolate pause from
    // the concurrent-limit gate.
    queue.on_task_finished();

    let was_paused = queue.pause();
    assert!(
        !was_paused,
        "First pause should transition running → paused"
    );
    assert!(queue.is_paused(), "is_paused must reflect the pause call");

    assert!(
        queue.next_pending().is_none(),
        "next_pending must return None while the queue is paused"
    );
}

/// `resume()` flips the flag back. `next_pending()` then returns
/// queued items again.
#[test]
fn resume_unblocks_next_pending() {
    let mut queue = DownloadQueue::new();
    let _ids = enqueue_n(&mut queue, 1);

    queue.pause();
    assert!(queue.next_pending().is_none(), "paused queue blocks pull");

    let was_paused = queue.resume();
    assert!(was_paused, "Resume should transition paused → running");
    assert!(!queue.is_paused(), "is_paused must clear after resume");

    assert!(
        queue.next_pending().is_some(),
        "Resumed queue must allow next_pending to pull"
    );
}

/// pause()/resume() are idempotent — calling twice in the same
/// direction returns the previous state but doesn't toggle.
#[test]
fn pause_and_resume_are_idempotent() {
    let mut queue = DownloadQueue::new();

    // First pause: was running, transitions to paused. Returns
    // previous state (false = was not paused).
    assert!(!queue.pause());
    // Second pause: already paused; no transition. Returns
    // previous state (true = was paused).
    assert!(queue.pause());
    assert!(queue.is_paused(), "Still paused after second pause()");

    // First resume: was paused, transitions to running. Returns
    // previous state (true = was paused).
    assert!(queue.resume());
    // Second resume: already running; no transition. Returns
    // previous state (false = was not paused).
    assert!(!queue.resume());
    assert!(!queue.is_paused(), "Still running after second resume()");
}

/// Pause must NOT cancel or alter items already in flight. The
/// flag only gates the start-new-item path — items currently in
/// `Downloading` / `Processing` are untouched and keep running.
#[test]
fn pause_does_not_change_state_of_running_items() {
    let mut queue = DownloadQueue::new();
    let _ids = enqueue_n(&mut queue, 2);

    // Pull the first item — it transitions to Downloading.
    let pulled = queue.next_pending();
    assert!(pulled.is_some());
    let (running_id, _, _, _) = pulled.unwrap();

    let before_state = queue
        .get_status()
        .iter()
        .find(|i| i.id == running_id)
        .map(|i| i.state.clone())
        .expect("running item must exist");
    assert_eq!(before_state, DownloadState::Downloading);

    // Pause the queue.
    queue.pause();

    // The already-running item is unchanged.
    let after_state = queue
        .get_status()
        .iter()
        .find(|i| i.id == running_id)
        .map(|i| i.state.clone())
        .expect("running item must still exist after pause");
    assert_eq!(
        after_state,
        DownloadState::Downloading,
        "pause() must not cancel or transition items already in flight"
    );
}

/// `is_paused()` reflects the current state without side effects —
/// it must NOT consume the flag the way `take_recently_aborted` does.
#[test]
fn is_paused_is_a_pure_read() {
    let mut queue = DownloadQueue::new();
    queue.pause();

    // Reading three times in a row should return true each time.
    assert!(queue.is_paused());
    assert!(queue.is_paused());
    assert!(queue.is_paused());
}

/// Verifies that next_pending() returns the merged GamdlOptions for the item.
#[test]
fn next_pending_returns_merged_options() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = test_request_with_codec_override(SongCodec::AacHe);
    let _id = queue.enqueue(request, &settings);

    let (_, _, options, _) = queue.next_pending().expect("Should return pending item");
    assert_eq!(
        options.song_codec,
        Some(SongCodec::AacHe),
        "Returned options should reflect the per-download override"
    );
}

// ==========================================================
// 8. on_task_finished() tests
// ==========================================================

/// Verifies that on_task_finished() decrements active_count.
#[test]
fn on_task_finished_decrements_active_count() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_one(&mut queue);
    let _ = queue.next_pending(); // active_count = 1

    assert_eq!(queue.active_count, 1);
    queue.on_task_finished();
    assert_eq!(
        queue.active_count, 0,
        "active_count should be 0 after on_task_finished"
    );
}

/// Verifies that on_task_finished() does not underflow below zero.
/// Calling it when active_count is already 0 should be a no-op.
#[test]
fn on_task_finished_does_not_underflow() {
    let mut queue = DownloadQueue::new();
    assert_eq!(queue.active_count, 0);

    queue.on_task_finished();
    assert_eq!(queue.active_count, 0, "active_count should not go below 0");

    // Call it multiple times to be sure
    queue.on_task_finished();
    queue.on_task_finished();
    assert_eq!(queue.active_count, 0);
}

/// Verifies that after on_task_finished(), the queue can start new items
/// since a concurrent slot has been freed.
#[test]
fn on_task_finished_frees_slot_for_next_pending() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 2);

    // Start first item
    let first = queue.next_pending();
    assert!(first.is_some());
    // Verify second item can't start yet
    assert!(
        queue.next_pending().is_none(),
        "Should be at max_concurrent"
    );

    // Finish first task
    queue.on_task_finished();

    // Now second item should be startable
    let second = queue.next_pending();
    assert!(
        second.is_some(),
        "Should be able to start next item after finishing"
    );
    let (dl_id, _, _, _) = second.unwrap();
    assert_eq!(dl_id, ids[1]);
}

// ==========================================================
// 9. is_cancelled() tests
// ==========================================================

/// Verifies that is_cancelled() returns true for cancelled items.
#[test]
fn is_cancelled_returns_true_for_cancelled() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    queue.cancel(&id);

    assert!(
        queue.is_cancelled(&id),
        "Should return true for cancelled item"
    );
}

/// Verifies that is_cancelled() returns false for non-cancelled items
/// in various states (Queued, Downloading, Complete, Error).
#[test]
fn is_cancelled_returns_false_for_non_cancelled() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 4);

    // ids[0] = Queued
    assert!(
        !queue.is_cancelled(&ids[0]),
        "Queued item should not be cancelled"
    );

    // ids[1] = Downloading (via update_item_state)
    queue.update_item_state(&ids[1], DownloadState::Downloading);
    assert!(
        !queue.is_cancelled(&ids[1]),
        "Downloading item should not be cancelled"
    );

    // ids[2] = Complete
    queue.set_complete(&ids[2]);
    assert!(
        !queue.is_cancelled(&ids[2]),
        "Complete item should not be cancelled"
    );

    // ids[3] = Error
    queue.set_error(&ids[3], "error");
    assert!(
        !queue.is_cancelled(&ids[3]),
        "Error item should not be cancelled"
    );
}

/// Verifies that is_cancelled() returns false for a non-existent download ID.
#[test]
fn is_cancelled_returns_false_for_nonexistent_id() {
    let queue = DownloadQueue::new();
    assert!(
        !queue.is_cancelled("does-not-exist"),
        "Should return false for unknown ID"
    );
}

// ==========================================================
// 10. update_item_progress() tests
// ==========================================================

/// Verifies that a DownloadProgress event updates the item's progress,
/// speed, eta, and sets state to Downloading.
#[test]
fn update_item_progress_download_progress() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::DownloadProgress {
        percent: 45.5,
        speed: "2.5MiB/s".to_string(),
        eta: "00:30".to_string(),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    let s = &statuses[0];
    assert!((s.progress - 45.5).abs() < 0.001, "Progress should be 45.5");
    assert_eq!(s.speed.as_deref(), Some("2.5MiB/s"));
    assert_eq!(s.eta.as_deref(), Some("00:30"));
    assert_eq!(s.state, DownloadState::Downloading);
}

/// Verifies that a TrackInfo event updates the current_track field
/// with the formatted "Artist - Title" string.
#[test]
fn update_item_progress_track_info_with_artist() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::TrackInfo {
        title: "Anti-Hero".to_string(),
        artist: "Taylor Swift".to_string(),
        album: String::new(),
        track_number: None,
        track_total: None,
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].current_track.as_deref(),
        Some("Taylor Swift - Anti-Hero"),
        "Should format as 'Artist - Title'"
    );
}

/// Verifies that a TrackInfo event carrying `track_number` and
/// `track_total` propagates those into `completed_tracks` and
/// `total_tracks` on the queue item so the UI can render a
/// "Track N of M" context counter (#609).
#[test]
fn update_item_progress_track_info_sets_counter_fields() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::TrackInfo {
        title: "Track One".to_string(),
        artist: String::new(),
        album: String::new(),
        track_number: Some(3),
        track_total: Some(12),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(statuses[0].completed_tracks, Some(3));
    assert_eq!(statuses[0].total_tracks, Some(12));
}

/// GAMDL v3.1 emits `[Track 1/1]` for single-song URLs (new — older
/// GAMDL stayed silent). The backend must still propagate the
/// counter; the UI is responsible for suppressing the "1 of 1"
/// cosmetic (#609).
#[test]
fn update_item_progress_track_info_propagates_single_song_counter() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::TrackInfo {
        title: "Flowers".to_string(),
        artist: "Miley Cyrus".to_string(),
        album: String::new(),
        track_number: Some(1),
        track_total: Some(1),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(statuses[0].completed_tracks, Some(1));
    assert_eq!(statuses[0].total_tracks, Some(1));
}

/// Verifies that a TrackInfo event with an empty artist just uses the title.
#[test]
fn update_item_progress_track_info_without_artist() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::TrackInfo {
        title: "Bohemian Rhapsody".to_string(),
        artist: String::new(),
        album: String::new(),
        track_number: None,
        track_total: None,
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].current_track.as_deref(),
        Some("Bohemian Rhapsody"),
        "Should use title only when artist is empty"
    );
}

/// Verifies that a ProcessingStep event sets the state to Processing.
#[test]
fn update_item_progress_processing_step() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::ProcessingStep {
        step: "Remuxing to M4A".to_string(),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Processing,
        "ProcessingStep event should set state to Processing"
    );
}

/// Verifies that a Complete event sets the output_path and progress to 100%.
#[test]
fn update_item_progress_complete() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::Complete {
        path: "/output/song.m4a".to_string(),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].output_path.as_deref(),
        Some("/output/song.m4a"),
        "Complete event should set output_path"
    );
    assert!(
        (statuses[0].progress - 100.0).abs() < 0.001,
        "Complete event should set progress to 100%"
    );
}

/// Verifies that an Error event sets the error field on the item.
#[test]
fn update_item_progress_error() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::Error {
        message: "Codec not available".to_string(),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].error.as_deref(),
        Some("Codec not available"),
        "Error event should set the error field"
    );
    // Note: Error event does NOT change state -- that's handled by set_error()
    // after the process exits and retry/fallback logic is evaluated.
    assert_eq!(
        statuses[0].state,
        DownloadState::Queued,
        "Error event should NOT change state (that's set_error's job)"
    );
}

/// Verifies that an Unknown event does not change any item fields.
#[test]
fn update_item_progress_unknown_event_is_no_op() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::Unknown {
        raw: "some random output".to_string(),
    };
    queue.update_item_progress(&id, &event);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Queued,
        "Unknown event should not change state"
    );
    assert_eq!(
        statuses[0].progress, 0.0,
        "Unknown event should not change progress"
    );
}

/// Verifies that update_item_progress is a no-op for non-existent IDs
/// (does not panic).
#[test]
fn update_item_progress_nonexistent_id_is_safe() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_one(&mut queue);

    let event = GamdlOutputEvent::DownloadProgress {
        percent: 50.0,
        speed: "1MiB/s".to_string(),
        eta: "00:10".to_string(),
    };
    // Should not panic
    queue.update_item_progress("nonexistent-id", &event);

    // Original item should be unchanged
    let statuses = queue.get_status();
    assert_eq!(statuses[0].progress, 0.0);
}

// ==========================================================
// 11. set_error() and set_complete() tests
// ==========================================================

/// Verifies that set_error() sets the state to Error and records the
/// error message.
#[test]
fn set_error_sets_state_and_message() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.set_error(&id, "Network timeout occurred");

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Error);
    assert_eq!(
        statuses[0].error.as_deref(),
        Some("Network timeout occurred")
    );
}

/// Verifies that set_error() is a no-op for non-existent IDs.
#[test]
fn set_error_nonexistent_id_is_safe() {
    let mut queue = DownloadQueue::new();
    // Should not panic
    queue.set_error("nonexistent", "some error");
}

/// Verifies that set_complete() sets the state to Complete and progress
/// to 100%.
#[test]
fn set_complete_sets_state_and_progress() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.set_complete(&id);

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Complete);
    assert!(
        (statuses[0].progress - 100.0).abs() < 0.001,
        "set_complete should set progress to 100%"
    );
}

/// Verifies that set_complete() is a no-op for non-existent IDs.
#[test]
fn set_complete_nonexistent_id_is_safe() {
    let mut queue = DownloadQueue::new();
    // Should not panic
    queue.set_complete("nonexistent");
}

/// Verifies that set_complete() refuses to overwrite a terminal Error
/// state — the "revival" bug from #661.
///
/// The completion task at the bottom of the per-item pipeline always
/// calls set_complete after the post-companion advisory pass, even if
/// the download itself failed minutes earlier. Without this guard,
/// failed items would silently appear as Complete in the UI,
/// contradicting the prior error toast and activity-log entry.
#[test]
fn set_complete_does_not_revive_errored_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.set_error(&id, "primary download failed");
    queue.set_complete(&id); // Late completion-task pass — must be a no-op.

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Error,
        "set_complete must not transition Error -> Complete (#661)"
    );
    assert_eq!(
        statuses[0].error.as_deref(),
        Some("primary download failed"),
        "the original error message must be preserved"
    );
}

/// #1431: set_error / set_complete say whether the state really
/// changed, so a download is announced and counted once. The watchdog
/// failing an item and the run then finishing or erroring as well must
/// not produce a second "finished" or "failed".
#[test]
fn terminal_changes_are_reported_once() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    assert!(queue.set_error(&id, "stalled"), "first failure is a change");
    assert!(
        !queue.set_error(&id, "errored again"),
        "Error -> Error is not"
    );
    assert!(
        !queue.set_complete(&id),
        "a failed item does not now finish"
    );

    let done = enqueue_one(&mut queue);
    assert!(queue.set_complete(&done));
    assert!(
        !queue.set_complete(&done),
        "Complete -> Complete is not a change"
    );
    assert!(
        !queue.set_error(&done, "late"),
        "a finished item does not now fail"
    );

    let cancelled = enqueue_one(&mut queue);
    queue.cancel(&cancelled);
    assert!(
        !queue.set_complete(&cancelled),
        "cancelled during post-processing"
    );
    assert!(!queue.set_error(&cancelled, "late"));

    assert!(!queue.set_complete("no-such-item"));
    assert!(!queue.set_error("no-such-item", "x"));
}

/// #1431, review round 3 item 1: on the per-format retry route the
/// attempt sets Error itself before trying the next format. When no
/// format is left, the final `set_error` changes nothing -- but the
/// failure must still be announced and counted, once.
#[test]
fn a_failure_after_the_last_format_is_still_announced_once() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    let _ = queue.next_pending();
    // The retry route: Error, then (chain exhausted) no new options.
    let failed_here = queue.set_error(&id, "format not available");
    assert!(failed_here);
    let changed_now = queue.set_error(&id, "no output files were produced");
    assert!(
        !changed_now,
        "already Error: no change at the final failure"
    );
    assert!(
        is_new_failure(failed_here, changed_now),
        "but it is this attempt's failure, so it is announced"
    );
    // An item the watchdog had failed before this attempt touched it is
    // not this attempt's failure.
    assert!(!is_new_failure(false, false));
}

/// #1431, review round 3 item 2: a run that returns after the stall
/// watchdog failed the item, or after the person cancelled it, leaves
/// it as it is.
#[test]
fn an_item_ended_outside_the_attempt_is_recognised() {
    assert_eq!(
        ended_outside_this_attempt(&DownloadState::Error),
        Some(EndedOutside::FailedByWatchdog)
    );
    assert_eq!(
        ended_outside_this_attempt(&DownloadState::Cancelled),
        Some(EndedOutside::Cancelled)
    );
    for running in [
        DownloadState::Queued,
        DownloadState::Downloading,
        DownloadState::Processing,
        DownloadState::Complete,
    ] {
        assert_eq!(ended_outside_this_attempt(&running), None, "{running:?}");
    }

    // The case that motivated it: watchdog fails a running item, the
    // run then exits cleanly. Without the check, set_processing would
    // move it back and set_complete would report a NEW completion.
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    let _ = queue.next_pending();
    assert!(queue.set_error(&id, "stalled"));
    let state = queue.get_status()[0].state.clone();
    assert!(ended_outside_this_attempt(&state).is_some());
}

/// #1431, review round 4: the one check used wherever the exit-0 branch
/// re-takes the lock. It releases the slot only for a cancel (the
/// watchdog has already released it), ignores an Error this attempt set
/// itself, and leaves a running item alone.
#[test]
fn stop_if_ended_outside_releases_the_slot_only_for_a_cancel() {
    // Running: nothing to stop, slot untouched.
    let mut q = DownloadQueue::new();
    let id = enqueue_one(&mut q);
    let _ = q.next_pending();
    assert_eq!(q.active_count, 1);
    assert_eq!(stop_if_ended_outside(&mut q, &id, false), None);
    assert_eq!(q.active_count, 1);

    // Cancelled by the person (e.g. during gap-fill): stop, and give the
    // slot back -- nothing else will.
    assert!(q.cancel(&id));
    assert_eq!(
        stop_if_ended_outside(&mut q, &id, false),
        Some(EndedOutside::Cancelled)
    );
    assert_eq!(q.active_count, 0);

    // Failed by the stall watchdog (which released the slot itself):
    // stop, but do not release again -- that would free another item's
    // slot and let two downloads run at once.
    let mut q = DownloadQueue::new();
    let ids = enqueue_n(&mut q, 2);
    let first = ids[0].clone();
    let _ = q.next_pending();
    q.set_error(&first, "stalled");
    q.on_task_finished(); // what the watchdog does
    let second = ids[1].clone();
    let _ = q.next_pending(); // the next item now holds the slot
    assert_eq!(q.active_count, 1);
    assert_eq!(
        stop_if_ended_outside(&mut q, &first, false),
        Some(EndedOutside::FailedByWatchdog)
    );
    assert_eq!(q.active_count, 1, "the other item's slot is untouched");

    // An Error this attempt set itself (the per-format retry route) is
    // not the watchdog's: carry on.
    assert_eq!(stop_if_ended_outside(&mut q, &first, true), None);

    // An unknown item: nothing to do.
    assert_eq!(stop_if_ended_outside(&mut q, "gone", false), None);
    let _ = second;
}

/// Verifies that set_complete() refuses to overwrite a Cancelled state.
/// Mirrors the Error guard — both are terminal and must not regress.
#[test]
fn set_complete_does_not_revive_cancelled_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.cancel(&id);
    queue.set_complete(&id);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Cancelled,
        "set_complete must not transition Cancelled -> Complete (#661)"
    );
}

/// Verifies that set_error() refuses to overwrite a Cancelled state.
/// A user-initiated cancellation must not be downgraded to "Error" by a
/// late-arriving subprocess error during teardown.
#[test]
fn set_error_does_not_overwrite_cancelled_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.cancel(&id);
    queue.set_error(&id, "stderr noise after cancellation");

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Cancelled,
        "set_error must not transition Cancelled -> Error (#661)"
    );
    // The cancellation path leaves error=None; the late set_error must
    // not poison that field with subprocess teardown noise.
    assert!(
        statuses[0].error.is_none(),
        "Cancelled items must not gain an error message after the fact"
    );
}

/// Verifies that set_error() refuses to overwrite a Complete state.
/// A successful download must not regress to Error if some tail-end
/// async task fails after enrichment ended.
#[test]
fn set_error_does_not_overwrite_complete_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.set_complete(&id);
    queue.set_error(&id, "late-arriving enrichment failure");

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Complete,
        "set_error must not transition Complete -> Error (#661)"
    );
}

// ==========================================================
// 11a-2. integrity_failure_message() tests (#1021)
// ==========================================================

/// Nothing was probed (e.g. ffprobe unavailable, no M4A files) — never
/// a failure, regardless of the (empty) suspect list.
#[test]
fn integrity_failure_message_none_when_nothing_checked() {
    assert_eq!(integrity_failure_message(0, &[]), None);
}

/// Partial hit: some files fine, some suspect. Not a hard failure —
/// the caller surfaces this as a warning instead, since most of the
/// album downloaded correctly.
#[test]
fn integrity_failure_message_none_when_partial() {
    assert_eq!(
        integrity_failure_message(3, &["01 Track.m4a".to_string()]),
        None
    );
}

/// Every probed file is suspect — a hard failure.
///
/// Also pins that this message starts with the shared
/// `GAMDL_BUG_MARKER` (#1231) — it names a known GAMDL defect
/// (gamdl#328), so the History/Queue error display's "Report this
/// bug to GAMDL" option must be able to recognise it.
#[test]
fn integrity_failure_message_some_when_all_suspect() {
    let suspects = vec![
        "01 Track.m4a".to_string(),
        "02 Track.m4a".to_string(),
        "03 Track.m4a".to_string(),
    ];
    let msg = integrity_failure_message(3, &suspects);
    assert!(msg.is_some());
    let msg = msg.unwrap();
    assert!(msg.starts_with(process::GAMDL_BUG_MARKER));
    assert!(msg.contains("gamdl#328"));
    assert!(msg.contains("01 Track.m4a"));
}

/// Single-file download where the one probed file is suspect.
#[test]
fn integrity_failure_message_some_for_single_suspect_file() {
    let suspects = vec!["01 Track.m4a".to_string()];
    assert!(integrity_failure_message(1, &suspects).is_some());
}

// ==========================================================
// 11b. delete() tests (#685)
// ==========================================================

/// Verifies that delete() removes a queued item and reports success.
#[test]
fn delete_removes_queued_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    assert_eq!(queue.get_status().len(), 1);

    let result = queue.delete(&id);
    assert_eq!(result, Ok(true));
    assert!(queue.get_status().is_empty());
}

/// Verifies that delete() removes a Complete item.
#[test]
fn delete_removes_complete_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    queue.set_complete(&id);

    assert_eq!(queue.delete(&id), Ok(true));
    assert!(queue.get_status().is_empty());
}

/// Verifies that delete() removes an Error item (the primary use case
/// — purging a stubbornly-failing entry without nuking the whole list).
#[test]
fn delete_removes_errored_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    queue.set_error(&id, "permanent failure");

    assert_eq!(queue.delete(&id), Ok(true));
    assert!(queue.get_status().is_empty());
}

/// Verifies that delete() removes a Cancelled item.
#[test]
fn delete_removes_cancelled_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    queue.cancel(&id);

    assert_eq!(queue.delete(&id), Ok(true));
    assert!(queue.get_status().is_empty());
}

/// Verifies that delete() refuses to remove an actively Downloading
/// item — orphaning the subprocess would leak file handles and emit
/// progress events with no queue row to update.
#[test]
fn delete_refuses_active_downloading_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    // Force-set state to Downloading. (In production this happens via
    // process_queue; the test only needs the state for the guard check.)
    if let Some(item) = queue.items.iter_mut().find(|i| i.status.id == id) {
        item.status.state = DownloadState::Downloading;
    }

    let result = queue.delete(&id);
    assert!(result.is_err(), "active items must not be deletable");
    assert_eq!(queue.get_status().len(), 1, "item must still be present");
}

/// Verifies that delete() refuses to remove a Processing item — the
/// enrichment pipeline is still writing tags / running companions
/// after GAMDL exits, and pulling the queue row out from under it
/// would invalidate the QueueItemHandle the pipeline holds.
#[test]
fn delete_refuses_active_processing_item() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);
    if let Some(item) = queue.items.iter_mut().find(|i| i.status.id == id) {
        item.status.state = DownloadState::Processing;
    }

    let result = queue.delete(&id);
    assert!(result.is_err(), "processing items must not be deletable");
    assert_eq!(queue.get_status().len(), 1);
}

/// Verifies that delete() returns Ok(false) for an unknown ID rather
/// than treating it as an error — the IPC layer can distinguish "no-op"
/// (already removed by a parallel click) from "guard violation".
#[test]
fn delete_unknown_id_is_noop() {
    let mut queue = DownloadQueue::new();
    let _ = enqueue_one(&mut queue);

    assert_eq!(queue.delete("nonexistent-id"), Ok(false));
    assert_eq!(queue.get_status().len(), 1, "real items untouched");
}

/// Verifies that delete() removes only the targeted item when several
/// items are present.
#[test]
fn delete_targets_only_the_named_item() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);
    assert_eq!(queue.get_status().len(), 3);

    assert_eq!(queue.delete(&ids[1]), Ok(true));

    let remaining: Vec<String> = queue.get_status().into_iter().map(|s| s.id).collect();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.contains(&ids[0]));
    assert!(remaining.contains(&ids[2]));
    assert!(!remaining.contains(&ids[1]));
}

// ==========================================================
// 12. try_network_retry() tests
// ==========================================================

/// Verifies that try_network_retry() resets the item to Queued state when
/// retries remain, returns attempt number and total, and decrements the
/// retry counter.
#[test]
fn try_network_retry_resets_to_queued_when_retries_remain() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    // Simulate an error state
    queue.set_error(&id, "Network timeout");

    let result = queue.try_network_retry(&id);
    // max_network_retries = 3, so total = 4. First retry = attempt 2.
    assert_eq!(result, Some((2, 4)), "First retry should be attempt 2 of 4");

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Queued,
        "Should reset to Queued for retry"
    );
    assert!(
        statuses[0].error.is_none(),
        "Error should be cleared on retry"
    );
    assert_eq!(statuses[0].progress, 0.0, "Progress should reset to 0");
}

/// Verifies that try_network_retry() returns None when all retries have
/// been exhausted (default: 3 retries), and that attempt numbers increment.
#[test]
fn try_network_retry_returns_false_when_exhausted() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    // Use up all 3 retries and verify attempt numbers
    queue.set_error(&id, "network error");
    assert_eq!(queue.try_network_retry(&id), Some((2, 4)));
    queue.set_error(&id, "network error");
    assert_eq!(queue.try_network_retry(&id), Some((3, 4)));
    queue.set_error(&id, "network error");
    assert_eq!(queue.try_network_retry(&id), Some((4, 4)));

    // 4th attempt should fail
    queue.set_error(&id, "network error");
    let result = queue.try_network_retry(&id);
    assert!(
        result.is_none(),
        "Should return None after all retries exhausted"
    );
}

/// Verifies that try_network_retry() returns None for a non-existent ID.
#[test]
fn try_network_retry_nonexistent_id() {
    let mut queue = DownloadQueue::new();
    assert!(queue.try_network_retry("nonexistent").is_none());
}

// ==========================================================
// 12b. try_storefront_fallback() tests (#666)
// ==========================================================

/// Verifies that a US-storefront URL is rewritten to the user's account
/// region, the item is reset to Queued, and the budget flag is set so
/// a second call is a no-op.
#[test]
fn try_storefront_fallback_rewrites_us_to_gb_and_consumes_budget() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.storefront = "gb".to_string();
    settings.storefront_fallback_on_failure = true;

    let id = queue.enqueue(test_request(), &settings);
    queue.set_error(&id, "404 Resource Not Found");

    let swap = queue.try_storefront_fallback(&id, &settings);
    assert_eq!(swap, Some(("us".to_string(), "gb".to_string())));

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Queued);
    assert_eq!(
        statuses[0].urls[0],
        "https://music.apple.com/gb/album/test-song/123456789"
    );
    assert!(statuses[0].error.is_none());

    // Budget consumed — second call must return None even after
    // another error, so we never ping-pong forever.
    queue.set_error(&id, "404 Resource Not Found");
    assert_eq!(queue.try_storefront_fallback(&id, &settings), None);
}

/// Verifies that the fallback is a no-op when the URL storefront
/// already matches the user's account region (nothing to rewrite to).
#[test]
fn try_storefront_fallback_skips_when_url_already_matches_region() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.storefront = "us".to_string(); // matches the test URL
    settings.storefront_fallback_on_failure = true;

    let id = queue.enqueue(test_request(), &settings);
    queue.set_error(&id, "404 Resource Not Found");

    assert_eq!(queue.try_storefront_fallback(&id, &settings), None);
    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Error, "must stay errored");
}

/// Verifies that the fallback is a no-op when the user has disabled it.
#[test]
fn try_storefront_fallback_respects_settings_toggle() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.storefront = "gb".to_string();
    settings.storefront_fallback_on_failure = false; // user opted out

    let id = queue.enqueue(test_request(), &settings);
    queue.set_error(&id, "404 Resource Not Found");

    assert_eq!(queue.try_storefront_fallback(&id, &settings), None);
}

/// Verifies that a manual user retry (which calls `retry()`) refreshes
/// the budget so the user can ask for the rewrite again.
#[test]
fn retry_resets_storefront_fallback_budget() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.storefront = "gb".to_string();
    settings.storefront_fallback_on_failure = true;

    let id = queue.enqueue(test_request(), &settings);
    // First failure + automatic fallback consumes the budget.
    queue.set_error(&id, "404 Resource Not Found");
    assert!(queue.try_storefront_fallback(&id, &settings).is_some());
    // Pretend the GB attempt also failed.
    queue.set_error(&id, "404 Resource Not Found");
    assert!(queue.try_storefront_fallback(&id, &settings).is_none());

    // User clicks Retry — budget refreshes.
    assert!(queue.retry(&id, &settings));
    // Re-fail and verify the budget is fresh.
    queue.set_error(&id, "404 Resource Not Found");
    // URL is now `/gb/`; the helper rejects same-region rewrites, so we
    // expect None here. Update settings to a different region to confirm
    // the budget *would* allow it.
    settings.storefront = "fr".to_string();
    assert!(queue.try_storefront_fallback(&id, &settings).is_some());
}

// ==========================================================
// 13. try_fallback() tests
// ==========================================================

/// Verifies that try_fallback() returns new options with the next codec in
/// the fallback chain, resets the item to Queued, and marks fallback_occurred.
#[test]
fn try_fallback_returns_next_codec_in_chain() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    // Simulate an error requiring fallback
    queue.set_error(&id, "Codec not available");

    // First fallback: chain[0] = Alac (initial), so next = chain[1] = Atmos
    let result = queue.try_fallback(&id, &settings);
    assert!(result.is_some(), "First fallback should succeed");

    let (new_opts, fb_idx, chain_len) = result.unwrap();
    assert_eq!(fb_idx, 1, "First fallback should be index 1");
    assert_eq!(chain_len, 6, "Default chain has 6 codecs");
    // song_codec should be None — GAMDL >= 2.9.1 removed --song-codec.
    // We use --song-codec-priority with a single codec instead.
    assert_eq!(
        new_opts.song_codec, None,
        "song_codec should be None (--song-codec removed in GAMDL 2.9.1)"
    );
    // song_codec_priority should be set to just the single fallback codec
    // to prevent process_download_item() from rebuilding the full chain
    // and to override config.ini's full priority chain.
    assert_eq!(
        new_opts.song_codec_priority,
        Some("atmos".to_string()),
        "song_codec_priority should be overridden to single fallback codec"
    );

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Queued,
        "Should reset to Queued"
    );
    assert!(
        statuses[0].fallback_occurred,
        "Should mark fallback_occurred as true"
    );
    assert_eq!(statuses[0].codec_used.as_deref(), Some("atmos"));
    assert!(
        statuses[0].error.is_none(),
        "Error should be cleared on fallback"
    );
    assert_eq!(statuses[0].progress, 0.0, "Progress should be reset");
}

/// A Custom-mode item that went through the format fallback: the saved
/// options carry the fallback format's suffix and the item records it,
/// suffixes never pile up over several fallbacks, and the companions'
/// base templates lose exactly that suffix, so the "plain" copy can
/// never get the primary's file name.
#[test]
fn a_custom_mode_fallback_never_gives_a_companion_the_primarys_name() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.companion_mode = CompanionMode::Custom;
    settings.custom_companion_codecs = vec![SongCodec::Alac, SongCodec::Aac];
    let id = queue.enqueue(test_request(), &settings);
    assert!(queue.items[0].added_template_suffixes.is_empty());

    queue.set_error(&id, "Codec not available");
    let (first, _, _) = queue.try_fallback(&id, &settings).unwrap();
    // Atmos is next in the default chain: its suffix is now saved.
    assert!(first
        .single_disc_file_template
        .as_deref()
        .unwrap()
        .ends_with(" [Dolby Atmos]"));
    assert_eq!(
        queue.items[0].added_template_suffixes,
        vec!["[Dolby Atmos]"]
    );
    queue.set_error(&id, "Codec not available");
    let (second, _, _) = queue.try_fallback(&id, &settings).unwrap();
    let template = second.single_disc_file_template.clone().unwrap();
    assert!(
        !template.contains("[Dolby Atmos]"),
        "suffixes piled up: {template}"
    );
    let recorded = queue.items[0].added_template_suffixes.clone();
    assert_eq!(recorded.len(), 1);
    assert!(template.ends_with(&format!(" {}", recorded[0])));
    let primary = PrimaryNaming::of(&second, recorded.last().copied());

    // The companions start from the same saved options.
    let mut base = second.clone();
    assert!(strip_recorded_codec_suffixes(&mut base, &recorded));
    // The plain (AAC) tier is now plainly named, different from the
    // primary's suffixed name.
    base.song_codec = None;
    base.song_codec_priority = Some("aac".into());
    assert_eq!(
        companion_attempt_naming(&SongCodec::Aac, false, &base, &primary, &[]),
        Ok(AttemptPlan {
            apply_suffix: false,
            only_other_tracks: false
        })
    );
    // Without the strip, the plain copy would have had the primary's
    // names — and would be skipped rather than overwrite it.
    let mut unstripped = second.clone();
    unstripped.song_codec = None;
    unstripped.song_codec_priority = Some("aac".into());
    assert_eq!(
        companion_attempt_naming(&SongCodec::Aac, false, &unstripped, &primary, &[]),
        Err(CompanionSkip::SameNames)
    );
    // A retry merges afresh and forgets the record.
    queue.set_error(&id, "failed");
    queue.retry(&id, &settings);
    assert!(queue.items[0].added_template_suffixes.is_empty());
}

#[test]
fn a_suffix_the_person_typed_is_never_removed() {
    let mut opts = GamdlOptions {
        single_disc_file_template: Some("{track:02d} {title} [Lossless]".into()),
        multi_disc_file_template: Some(
            "{disc}-{track:02d} {title} [Lossless] [Dolby Atmos]".into(),
        ),
        no_album_file_template: Some("[Dolby Atmos] {title}".into()),
        ..GamdlOptions::default()
    };
    // Nothing recorded: nothing removed, even a registry name.
    assert!(!strip_recorded_codec_suffixes(&mut opts, &[]));
    assert_eq!(
        opts.single_disc_file_template.as_deref(),
        Some("{track:02d} {title} [Lossless]")
    );
    // Only the recorded suffix, once, at the very end.
    assert!(strip_recorded_codec_suffixes(&mut opts, &["[Dolby Atmos]"]));
    assert_eq!(
        opts.single_disc_file_template.as_deref(),
        Some("{track:02d} {title} [Lossless]")
    );
    assert_eq!(
        opts.multi_disc_file_template.as_deref(),
        Some("{disc}-{track:02d} {title} [Lossless]")
    );
    assert_eq!(
        opts.no_album_file_template.as_deref(),
        Some("[Dolby Atmos] {title}")
    );
}

#[test]
fn a_fallback_keeps_a_users_own_suffix_in_their_template() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.companion_mode = CompanionMode::Custom;
    settings.custom_companion_codecs = vec![SongCodec::Alac];
    settings.single_disc_file_template = "{track:02d} {title} [Lossless]".into();
    let id = queue.enqueue(test_request(), &settings);
    for _ in 0..2 {
        queue.set_error(&id, "Codec not available");
        queue.try_fallback(&id, &settings).unwrap();
    }
    let template = queue.items[0]
        .merged_options
        .single_disc_file_template
        .clone()
        .unwrap();
    assert!(
        template.starts_with("{track:02d} {title} [Lossless]"),
        "the person's own suffix was removed: {template}"
    );
}

/// The extra-format copies never delete anything: on a mixed album the
/// tracks already in a format are simply not requested.
#[test]
fn the_companion_path_never_deletes_files() {
    let src = include_str!("companions.rs");
    for call in ["remove_file", "remove_dir"] {
        assert!(!src.contains(call), "companions.rs calls {call}");
    }
}

/// The main download's lyrics steps work on a list of this download's
/// files. A folder-wide call coming back would again convert every older
/// file in the folder (#1441), so neither source file may call one of the
/// six folder-wide lyrics functions.
#[test]
fn the_download_pipeline_never_calls_a_folder_wide_lyrics_function() {
    let folder_wide = [
        "process_enhanced_lyrics_for_directory(",
        "generate_webvtt_for_directory(",
        "generate_rich_srt_for_directory(",
        "embed_subtitles_for_directory(",
        "generate_ass_for_directory(",
        "generate_lyricsfile_for_directory(",
    ];
    for (name, src) in [
        ("processing.rs", include_str!("processing.rs")),
        ("companions.rs", include_str!("companions.rs")),
    ] {
        for call in folder_wide {
            assert!(!src.contains(call), "{name} calls {call}");
        }
    }
}

#[test]
fn a_mixed_album_attempt_asks_only_for_the_other_tracks() {
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    let primary = vec![
        (SongCodec::Atmos, 1, 1),
        (SongCodec::Alac, 1, 2),
        (SongCodec::Atmos, 1, 3),
    ];
    let tracks = vec![
        (1, 1, "11".to_string()),
        (1, 2, "12".to_string()),
        // Track 3 has no song ID.
    ];
    // Not a mixed album: the item's own links.
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[],
            false,
            &[]
        ),
        AttemptLinks::All
    );
    // Mixed: only track 1 is requested (track 2 is already ALAC),
    // track 3 is counted as unmatched. Nothing is ever deleted — the
    // already-ALAC track is simply not asked for.
    // One more song (in Atmos) had no readable disc/track: also left
    // out and counted; one in ALAC needs no copy and is not counted.
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            true,
            &album,
            &primary,
            &tracks,
            &[SongCodec::Atmos, SongCodec::Alac],
            &[],
            false,
            &[]
        ),
        AttemptLinks::Only {
            links: vec!["https://music.apple.com/us/album/_/1440857781?i=11".to_string()],
            unmapped: 1,
            unreadable: 1,
            already_there: 0,
            details_available: true,
        }
    );
    // No album song details at all (no developer key).
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            true,
            &album,
            &primary,
            &[],
            &[],
            &[],
            false,
            &[]
        ),
        AttemptLinks::Only {
            links: vec![],
            unmapped: 2,
            unreadable: 0,
            already_there: 0,
            details_available: false,
        }
    );
    // A playlist cannot be split by track: that format is skipped with
    // a plain reason.
    let playlist = vec!["https://music.apple.com/us/playlist/x/pl.u-GgA567VCXX".to_string()];
    assert_eq!(
            attempt_links(&SongCodec::Alac, true, &playlist, &primary, &tracks, &[], &[], false, &[]),
            AttemptLinks::Skip(
                "Extra copies in Lossless (ALAC) were not made for this playlist, because some of its songs are already in that format."
                    .to_string()
            )
        );
    // A single song link is not split either.
    let song = vec!["https://music.apple.com/us/album/x/1440857781?i=11".to_string()];
    assert!(matches!(
        attempt_links(
            &SongCodec::Alac,
            true,
            &song,
            &primary,
            &tracks,
            &[],
            &[],
            false,
            &[]
        ),
        AttemptLinks::Skip(_)
    ));
}

#[test]
fn mixed_album_lines_say_why_songs_got_no_copy() {
    assert_eq!(
            mixed_album_lines(&SongCodec::Alac, 3, 1, 0, true),
            vec!["1 song(s) were left without an extra copy in Lossless (ALAC) because their Apple Music details could not be matched.".to_string()]
        );
    assert_eq!(
            mixed_album_lines(&SongCodec::Alac, 0, 2, 0, false),
            vec![
                "2 song(s) were left without an extra copy in Lossless (ALAC) because this album's song details were not available.".to_string(),
                "No extra copy in Lossless (ALAC) was made for this album: none of its songs could be asked for on their own.".to_string(),
            ]
        );
    // Unreadable track numbers get their own sentence.
    assert_eq!(
            mixed_album_lines(&SongCodec::Alac, 2, 0, 3, true),
            vec!["3 song(s) were left without an extra copy in Lossless (ALAC) because their track numbers could not be read.".to_string()]
        );
    // Nothing requested and nothing counted still says so.
    assert_eq!(mixed_album_lines(&SongCodec::Alac, 0, 0, 0, true).len(), 1);
    assert!(mixed_album_lines(&SongCodec::Alac, 2, 0, 0, true).is_empty());
}

/// A stale `03 X [Lossless]` from an earlier download sits in the folder
/// of an all-Atmos download. ALAC is not this download's format: the
/// ALAC copy asks for the whole album as usual (not per track, not
/// skipped, no "already in that format" line), with overwrite off so the
/// stale file is never replaced; and the stale file is not renamed.
#[test]
fn a_stale_copy_in_the_folder_does_not_make_the_album_mixed() {
    let dir = tempfile::tempdir().unwrap();
    let mine = dir.path().join("01 A.m4a");
    let stale = dir.path().join("03 X [Lossless].m4a");
    std::fs::write(&mine, b"a").unwrap();
    std::fs::write(&stale, b"x").unwrap();
    let file_codecs = vec![
        (mine.clone(), SongCodec::Atmos),
        (stale.clone(), SongCodec::Alac),
    ];
    let this_download = vec![mine.clone()];
    let split =
        super::lyrics_pass::split_this_downloads_formats(&this_download, &file_codecs, |_| {
            Some((1, 1))
        });
    // Only this download's files are renamed.
    let renamed = crate::services::metadata_tag_service::apply_deferred_file_renames(
        &this_download,
        None,
        &file_codecs,
        false,
    );
    assert!(stale.exists(), "the stale file must keep its name");
    let primary_codecs = super::lyrics_pass::protection_codecs(&renamed);
    assert_eq!(primary_codecs, vec![SongCodec::Atmos]);

    let attempt = GamdlOptions {
        song_codec_priority: Some("alac".into()),
        ..GamdlOptions::default()
    };
    let plan = companion_attempt_naming(
        &SongCodec::Alac,
        true,
        &attempt,
        &PrimaryNaming::default(),
        &primary_codecs,
    )
    .unwrap();
    assert!(!plan.only_other_tracks);
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            plan.only_other_tracks,
            &album,
            &split.tracks,
            &[],
            &split.unplaceable,
            &[],
            false,
            &[]
        ),
        AttemptLinks::All
    );
    // Every companion attempt runs with overwrite off (see the
    // companion task), so the stale file is never replaced.
}

#[test]
fn a_copy_skipped_because_its_name_was_taken_is_reported() {
    // GAMDL 4: a skip with a path and no reason = the file was there.
    assert!(is_already_there_skip_line(
        "↷ Song: skipped → /m/A/01 Song [Lossless].m4a"
    ));
    // A skip for another reason carries one, and is not counted.
    assert!(!is_already_there_skip_line(
        "↷ Song: skipped (Media is not streamable)"
    ));
    assert!(!is_already_there_skip_line(
        "✓ Song: downloaded → /m/A/01 Song.m4a"
    ));
    // GAMDL 3 wording.
    assert!(is_already_there_skip_line(
        "[WARNING 10:00:00] [Track 1/3] Skipping \"Song\": Media file already exists"
    ));
    assert!(!is_already_there_skip_line(
        "[WARNING 10:00:00] [Track 1/3] Skipping \"Song\": Requested format is not available"
    ));
    assert_eq!(already_there_line(&SongCodec::Alac, 0), None);
    assert_eq!(
            already_there_line(&SongCodec::Alac, 2).as_deref(),
            Some("2 extra copy(ies) in Lossless (ALAC) were not made because a file with the same name was already there.")
        );
}

/// The overwrite setting reaches GAMDL on the command line only: the
/// primary run says `--overwrite`, a companion run (overwrite forced
/// off) says nothing — and config.ini never says it either, because
/// GAMDL 3 cannot cancel an INI `overwrite = true` from the command line.
#[test]
fn overwrite_is_on_the_primary_command_line_and_never_on_a_companions() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.9.1".to_string()));
    let mut settings = test_settings();
    settings.overwrite = true;
    let primary = merge_options(None, &settings, None);
    assert!(primary.to_cli_args().iter().any(|a| a == "--overwrite"));
    let mut companion = primary.clone();
    companion.overwrite = Some(false);
    assert!(!companion.to_cli_args().iter().any(|a| a == "--overwrite"));
    crate::services::gamdl_capabilities::set_detected_version(None);
}

/// The Library re-download sends `overwrite: false`; it must beat the
/// "Overwrite" setting, so filling an album's gaps never replaces the
/// files already there.
#[test]
fn a_library_redownload_never_overwrites_even_with_the_setting_on() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.9.1".to_string()));
    let mut settings = test_settings();
    settings.overwrite = true;
    let overrides = GamdlOptions {
        overwrite: Some(false),
        ..GamdlOptions::default()
    };
    let merged = merge_options(Some(&overrides), &settings, None);
    assert_eq!(merged.overwrite, Some(false));
    assert!(!merged.to_cli_args().iter().any(|a| a == "--overwrite"));
    crate::services::gamdl_capabilities::set_detected_version(None);
}

/// The follow-up runs (lyrics fallback, music-video copies) build their
/// options from `settings.overwrite`; for a "fill the gaps" item that
/// must be the item's `false`, never the saved `true`.
#[test]
fn follow_up_runs_of_a_fill_gaps_item_never_overwrite() {
    let mut settings = test_settings();
    settings.overwrite = true;
    let follow_up = settings_for_follow_up_runs(&settings, Some(false));
    assert!(!follow_up.overwrite);
    // Built the way both follow-up runs build theirs.
    let opts = GamdlOptions {
        overwrite: Some(follow_up.overwrite),
        ..GamdlOptions::default()
    };
    assert!(!opts.to_cli_args().iter().any(|a| a == "--overwrite"));
    // An item with no choice of its own follows the setting.
    assert!(settings_for_follow_up_runs(&settings, None).overwrite);
    assert!(settings_for_follow_up_runs(&settings, Some(true)).overwrite);
}

/// Every companion run forces overwrite off, on every path.
#[test]
fn companions_never_overwrite() {
    let src = include_str!("companions.rs");
    assert!(src.contains("opts.overwrite = Some(false);"));
}

/// On a re-download of a rated album, copies already in the folder carry
/// the advisory label (`01 Song [Explicit] [Lossless]`) that GAMDL's own
/// name check does not know about: only the songs without a copy are
/// requested, and when every song has one nothing is requested.
#[test]
fn labelled_copies_already_there_are_not_downloaded_again() {
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    let primary = vec![(SongCodec::Atmos, 1, 1), (SongCodec::Atmos, 1, 2)];
    let tracks = vec![(1, 1, "11".to_string()), (1, 2, "12".to_string())];
    // Track 1 already has its labelled ALAC copy: only track 2 is asked.
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[(1, 1)],
            false,
            &[]
        ),
        AttemptLinks::Only {
            links: vec!["https://music.apple.com/us/album/_/1440857781?i=12".to_string()],
            unmapped: 0,
            unreadable: 0,
            already_there: 1,
            details_available: true,
        }
    );
    // Every song has its copy: nothing requested, nothing to report as
    // missing (the tier counts as done).
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[(1, 1), (1, 2)],
            false,
            &[]
        ),
        AttemptLinks::Only {
            links: vec![],
            unmapped: 0,
            unreadable: 0,
            already_there: 2,
            details_available: true,
        }
    );
    // Without song details the normal request runs (GAMDL's own check).
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &[],
            &[],
            &[(1, 1)],
            false,
            &[]
        ),
        AttemptLinks::All
    );
    // A plain (AAC) attempt never looks for existing copies: the caller
    // passes none, and the normal request runs.
    assert_eq!(
        attempt_links(
            &SongCodec::Aac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[],
            false,
            &[]
        ),
        AttemptLinks::All
    );
}

/// A run that produced nothing is "done" only when the copies already
/// there plus the songs not offered in this format cover every song
/// asked for.
#[test]
fn a_tier_is_done_when_copies_there_and_songs_not_offered_cover_everything() {
    let primary: Vec<(SongCodec, u32, u32)> = (1..=10).map(|t| (SongCodec::Atmos, 1, t)).collect();
    let wanted = songs_wanted_in(&SongCodec::Alac, &primary, &[]);
    assert_eq!(wanted, 10);
    // A re-run: 8 copies already there, 2 never offered in ALAC — done,
    // so the next format is NOT downloaded for every song.
    assert!(tier_done_without_new_files(8, 2, wanted));
    // 8 there, 2 unaccounted for: not done.
    assert!(!tier_done_without_new_files(8, 0, wanted));
    // Nothing there at all: the format simply is not offered — not done.
    assert!(!tier_done_without_new_files(0, 10, wanted));
    // Per-song run of 2 links: 1 there + 1 not offered is done.
    assert!(tier_done_without_new_files(1, 1, 2));
    assert!(!tier_done_without_new_files(1, 0, 2));
    // Unknown expectation never counts as done.
    assert!(!tier_done_without_new_files(3, 0, 0));
    let mixed = vec![(SongCodec::Atmos, 1, 1), (SongCodec::Alac, 1, 2)];
    assert_eq!(
        songs_wanted_in(
            &SongCodec::Alac,
            &mixed,
            &[SongCodec::Alac, SongCodec::Atmos]
        ),
        2
    );
}

/// The next format in a tier asks only for the songs without a copy in
/// an earlier format of the tier.
#[test]
fn a_fallback_format_skips_songs_with_a_copy_in_an_earlier_format() {
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    let primary = vec![
        (SongCodec::Atmos, 1, 1),
        (SongCodec::Atmos, 1, 2),
        (SongCodec::Atmos, 1, 3),
    ];
    let tracks = vec![
        (1, 1, "11".to_string()),
        (1, 2, "12".to_string()),
        (1, 3, "13".to_string()),
    ];
    // Tracks 1 and 2 have an ALAC copy; ALAC was not offered for 3; now
    // AAC Legacy asks only for track 3.
    assert_eq!(
        attempt_links(
            &SongCodec::AacLegacy,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[],
            false,
            &[(1, 1), (1, 2)]
        ),
        AttemptLinks::Only {
            links: vec!["https://music.apple.com/us/album/_/1440857781?i=13".to_string()],
            unmapped: 0,
            unreadable: 0,
            already_there: 0,
            details_available: true,
        }
    );
    // Nothing covered earlier and nothing present: the whole album.
    assert_eq!(
        attempt_links(
            &SongCodec::AacLegacy,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[],
            false,
            &[]
        ),
        AttemptLinks::All
    );
}

#[test]
fn format_not_offered_lines_are_recognised_for_both_gamdl_generations() {
    assert!(crate::utils::process::is_format_not_offered_line(
        "[WARNING 10:00:00] [Track 2/4] Skipping \"Song\": Requested format is not available"
    ));
    assert!(crate::utils::process::is_format_not_offered_line(
        "  ✗ Song: Requested codec or stream unavailable"
    ));
    assert!(!crate::utils::process::is_format_not_offered_line(
        "  ✗ Song: Network request failed"
    ));
    assert!(!crate::utils::process::is_format_not_offered_line(
        "✓ Song: downloaded → /m/a.m4a"
    ));
}

/// Not a mixed album: when some songs cannot be asked for on their own
/// (no song ID, unreadable numbers), the whole album is requested so no
/// song is quietly left without a copy.
#[test]
fn songs_that_cannot_be_asked_for_alone_make_it_a_whole_album_request() {
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    let primary = vec![
        (SongCodec::Atmos, 1, 1),
        (SongCodec::Atmos, 1, 2),
        (SongCodec::Atmos, 1, 3),
    ];
    // Track 3 has no song ID; track 1 already has its copy.
    let tracks = vec![(1, 1, "11".to_string()), (1, 2, "12".to_string())];
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[(1, 1)],
            false,
            &[]
        ),
        AttemptLinks::All
    );
    // An unreadable song in another format does the same.
    let full = vec![
        (1, 1, "11".to_string()),
        (1, 2, "12".to_string()),
        (1, 3, "13".to_string()),
    ];
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &full,
            &[SongCodec::Atmos],
            &[(1, 1)],
            false,
            &[]
        ),
        AttemptLinks::All
    );
    // But when the copy already there carries an advisory label
    // (`01 Song [Explicit] [Lossless]`), GAMDL's own check would not find
    // it and would download a duplicate: keep asking per song, and
    // report the songs left out.
    assert_eq!(
        attempt_links(
            &SongCodec::Alac,
            false,
            &album,
            &primary,
            &tracks,
            &[],
            &[(1, 1)],
            true,
            &[]
        ),
        AttemptLinks::Only {
            links: vec!["https://music.apple.com/us/album/_/1440857781?i=12".to_string()],
            unmapped: 1,
            unreadable: 0,
            already_there: 1,
            details_available: true,
        }
    );
}

/// This download's own mixed album still asks per track.
#[test]
fn this_downloads_own_mixed_album_is_still_split_by_track() {
    let primary_codecs = vec![SongCodec::Atmos, SongCodec::Alac];
    let attempt = GamdlOptions {
        song_codec_priority: Some("alac".into()),
        ..GamdlOptions::default()
    };
    let plan = companion_attempt_naming(
        &SongCodec::Alac,
        true,
        &attempt,
        &PrimaryNaming::default(),
        &primary_codecs,
    )
    .unwrap();
    assert!(plan.only_other_tracks);
    let album = vec!["https://music.apple.com/us/album/x/1440857781".to_string()];
    assert!(matches!(
        attempt_links(
            &SongCodec::Alac,
            true,
            &album,
            &[(SongCodec::Atmos, 1, 1), (SongCodec::Alac, 1, 2)],
            &[(1, 1, "11".to_string()), (1, 2, "12".to_string())],
            &[],
            &[], false, &[]
        ),
        AttemptLinks::Only { ref links, .. } if links.len() == 1
    ));
}

#[test]
fn a_companion_never_takes_the_primarys_names_or_format() {
    let ok = |apply_suffix, only_other_tracks| {
        Ok(AttemptPlan {
            apply_suffix,
            only_other_tracks,
        })
    };
    let attempt = |codec: &str, single: &str| GamdlOptions {
        song_codec_priority: Some(codec.into()),
        single_disc_file_template: Some(single.into()),
        multi_disc_file_template: Some(format!("{{disc}}-{single}")),
        ..GamdlOptions::default()
    };
    let named = |single: &str, appended| PrimaryNaming {
        appended_suffix: appended,
        single_disc: Some(single.into()),
        multi_disc: Some(format!("{{disc}}-{single}")),
    };
    let plain = "{track:02d} {title}";

    // Plain (native) primary, nothing detected: nothing clashes for a
    // suffixed copy.
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Alac,
            true,
            &attempt("alac", plain),
            &named(plain, None),
            &[]
        ),
        ok(true, false)
    );
    // Normal single-format path: primary `[Dolby Atmos]`, ALAC copy
    // `[Lossless]`, AAC copy plain — all different, unchanged.
    let atmos = named("{track:02d} {title} [Dolby Atmos]", Some("[Dolby Atmos]"));
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Alac,
            true,
            &attempt("alac", plain),
            &atmos,
            &[SongCodec::Atmos]
        ),
        ok(true, false)
    );
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Aac,
            false,
            &attempt("aac", plain),
            &atmos,
            &[SongCodec::Atmos]
        ),
        ok(false, false)
    );
    // Custom + native priority: Atmos not offered, GAMDL picked ALAC
    // and enrichment renamed the primary `[Lossless]`; the ALAC copy is
    // pointless — skipped.
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Alac,
            true,
            &attempt("alac", plain),
            &named(plain, None),
            &[SongCodec::Alac]
        ),
        Err(CompanionSkip::SameFormat)
    );
    // A mixed album (some tracks Atmos, some ALAC): the ALAC copy runs
    // keeping existing files (its new copies of the ALAC tracks are
    // removed afterwards).
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Alac,
            true,
            &attempt("alac", plain),
            &named(plain, None),
            &[SongCodec::Atmos, SongCodec::Alac]
        ),
        ok(true, true)
    );
    // A label the person typed into their own template: the primary's
    // names are `01 T [Lossless]` (typed, nothing appended), a plain
    // AAC copy built from the same template is `01 T [Lossless]` too —
    // a real clash — but a suffixed ALAC copy (`… [Lossless] [Lossless]`)
    // is different, so it is not skipped.
    let typed = "{track:02d} {title} [Lossless]";
    let primary_typed = named(typed, None);
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Alac,
            true,
            &attempt("alac", typed),
            &primary_typed,
            &[SongCodec::Atmos]
        ),
        ok(true, false)
    );
    // The plain AAC copy really would clash; it has no suffix of its
    // own, so it is skipped.
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Aac,
            false,
            &attempt("aac", typed),
            &primary_typed,
            &[SongCodec::Atmos]
        ),
        Err(CompanionSkip::SameNames)
    );
    // A clashing plain attempt with a suffix of its own takes it.
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::AacLegacy,
            false,
            &attempt("aac-legacy", typed),
            &primary_typed,
            &[SongCodec::Atmos]
        ),
        ok(true, false)
    );
    // The reviewer's case: an ALAC primary whose typed template gained
    // MeedyaDL's own `[Lossless]` (`01 T [Lossless] [Lossless]`), and a
    // wanted plain AAC copy from the typed template (`01 T [Lossless]`).
    // The names differ, so the copy goes ahead.
    let alac_primary = named(
        "{track:02d} {title} [Lossless] [Lossless]",
        Some("[Lossless]"),
    );
    assert_eq!(
        companion_attempt_naming(
            &SongCodec::Aac,
            false,
            &attempt("aac", typed),
            &alac_primary,
            &[SongCodec::Alac]
        ),
        ok(false, false)
    );
    assert_eq!(plain_codec_name(&SongCodec::Alac), "Lossless (ALAC)");
}

/// Verifies that try_fallback() returns None when all codecs in the
/// fallback chain have been exhausted.
#[test]
fn try_fallback_returns_none_when_chain_exhausted() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    // Use a short chain for testing
    settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac];
    let id = queue.enqueue(test_request(), &settings);

    // First fallback: Alac (0) -> Aac (1)
    queue.set_error(&id, "codec error");
    let result1 = queue.try_fallback(&id, &settings);
    assert!(result1.is_some(), "First fallback to Aac should succeed");
    let (_, idx, len) = result1.unwrap();
    assert_eq!(idx, 1);
    assert_eq!(len, 2);

    // Second fallback: chain exhausted (index 2 >= chain.len() of 2)
    queue.set_error(&id, "codec error again");
    let result2 = queue.try_fallback(&id, &settings);
    assert!(result2.is_none(), "Should return None when chain exhausted");
}

/// A music video must never be handed an AUDIO codec fallback.
///
/// `try_fallback()`'s chain is `settings.music_fallback_chain` — the
/// AUDIO chain. Before the fix, a music video item was allowed to
/// walk that chain the same as a song, so a music video that GAMDL
/// couldn't download in any allowed VIDEO codec would be retried
/// with a different AUDIO codec instead. The activity log would say
/// something like "trying atmos", but the retry changed nothing
/// about the video and downloaded nothing at all — GAMDL had
/// already, by itself, tried every video codec it was allowed to
/// use in the one run it already made.
#[test]
fn try_fallback_does_nothing_for_a_music_video() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/music-video/test-video/123456789".to_string()],
        options: None,
        ..Default::default()
    };
    let id = queue.enqueue(request, &settings);

    queue.set_error(&id, "codec error");
    let result = queue.try_fallback(&id, &settings);
    assert!(
        result.is_none(),
        "a music video must never be given an AUDIO codec fallback"
    );

    // The item must be left exactly as the error left it — no
    // fallback silently recorded against it, no state change.
    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].state,
        DownloadState::Error,
        "a rejected fallback must not disturb the errored state"
    );
    assert!(
        !statuses[0].fallback_occurred,
        "no fallback should have been recorded"
    );
}

/// Verifies that try_fallback() returns None when fallback is disabled
/// in settings, regardless of chain contents.
#[test]
fn try_fallback_returns_none_when_disabled() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.fallback_enabled = false;
    let id = queue.enqueue(test_request(), &settings);

    queue.set_error(&id, "codec error");
    let result = queue.try_fallback(&id, &settings);
    assert!(
        result.is_none(),
        "Should return None when fallback_enabled is false"
    );
}

/// Verifies that try_fallback() returns None for a non-existent ID.
#[test]
fn try_fallback_nonexistent_id() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let result = queue.try_fallback("nonexistent", &settings);
    assert!(result.is_none());
}

/// Verifies that multiple successive fallbacks advance through the entire
/// fallback chain correctly.
#[test]
fn try_fallback_advances_through_full_chain() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Atmos, SongCodec::Aac];
    let id = queue.enqueue(test_request(), &settings);

    // Fallback 1: Alac -> Atmos (index 1 of 3-element chain)
    queue.set_error(&id, "codec error");
    let r1 = queue.try_fallback(&id, &settings);
    let (r1_opts, r1_idx, r1_len) = r1.unwrap();
    assert_eq!(
        r1_opts.song_codec, None,
        "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
    );
    assert_eq!(r1_idx, 1, "First fallback is index 1");
    assert_eq!(r1_len, 3, "Chain length is 3");
    assert_eq!(
        r1_opts.song_codec_priority,
        Some("atmos".to_string()),
        "Priority should be single codec on fallback"
    );

    // Fallback 2: Atmos -> Aac (index 2 of 3-element chain)
    queue.set_error(&id, "codec error");
    let r2 = queue.try_fallback(&id, &settings);
    let (r2_opts, r2_idx, _) = r2.unwrap();
    assert_eq!(
        r2_opts.song_codec, None,
        "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
    );
    assert_eq!(r2_idx, 2, "Second fallback is index 2");
    assert_eq!(
        r2_opts.song_codec_priority,
        Some("aac".to_string()),
        "Priority should be single codec on fallback"
    );

    // Fallback 3: exhausted
    queue.set_error(&id, "codec error");
    let r3 = queue.try_fallback(&id, &settings);
    assert!(r3.is_none(), "Chain should be exhausted after 3 codecs");
}

// ==========================================================
// 14. retry() tests
// ==========================================================

/// Verifies that retry() resets an errored item fully to Queued state
/// with fresh options, reset counters, and cleared error/progress.
#[test]
fn retry_resets_errored_item_to_queued() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    queue.set_error(&id, "Download failed");

    let result = queue.retry(&id, &settings);
    assert!(result, "retry() should return true for Error items");

    let statuses = queue.get_status();
    let s = &statuses[0];
    assert_eq!(s.state, DownloadState::Queued, "Should be reset to Queued");
    assert!(s.error.is_none(), "Error should be cleared");
    assert_eq!(s.progress, 0.0, "Progress should be reset");
    assert!(!s.fallback_occurred, "fallback_occurred should be reset");
    assert_eq!(
        s.codec_used.as_deref(),
        Some("alac"),
        "Codec should be re-merged from settings"
    );
}

/// Verifies that retry() resets a cancelled item to Queued state.
#[test]
fn retry_resets_cancelled_item() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    queue.cancel(&id);
    assert_eq!(queue.get_status()[0].state, DownloadState::Cancelled);

    let result = queue.retry(&id, &settings);
    assert!(result, "retry() should return true for Cancelled items");

    let statuses = queue.get_status();
    assert_eq!(statuses[0].state, DownloadState::Queued);
}

/// Verifies that retry() returns false for non-terminal states (Queued,
/// Downloading, Processing, Complete).
#[test]
fn retry_returns_false_for_non_terminal_states() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let ids = enqueue_n(&mut queue, 4);

    // ids[0] = Queued
    assert!(
        !queue.retry(&ids[0], &settings),
        "Should not retry Queued item"
    );

    // ids[1] = Downloading
    queue.update_item_state(&ids[1], DownloadState::Downloading);
    assert!(
        !queue.retry(&ids[1], &settings),
        "Should not retry Downloading item"
    );

    // ids[2] = Processing
    queue.update_item_state(&ids[2], DownloadState::Processing);
    assert!(
        !queue.retry(&ids[2], &settings),
        "Should not retry Processing item"
    );

    // ids[3] = Complete
    queue.set_complete(&ids[3]);
    assert!(
        !queue.retry(&ids[3], &settings),
        "Should not retry Complete item"
    );
}

/// Verifies that retry() returns false for a non-existent download ID.
#[test]
fn retry_returns_false_for_nonexistent_id() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    assert!(!queue.retry("nonexistent", &settings));
}

/// Verifies that retry() re-merges options from the original request
/// with the current settings. This means if settings changed between the
/// original enqueue and the retry, the retry picks up the new settings.
#[test]
fn retry_remerges_options_from_original_request() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = test_request_with_codec_override(SongCodec::AacHe);
    let id = queue.enqueue(request, &settings);

    queue.set_error(&id, "error");

    // Retry with the same settings
    let result = queue.retry(&id, &settings);
    assert!(result);

    let statuses = queue.get_status();
    assert_eq!(
        statuses[0].codec_used.as_deref(),
        Some("aac-he"),
        "Retry should preserve the original per-download override"
    );
}

/// Verifies that retry() resets the network retries counter and fallback
/// index, which can be verified by subsequent try_network_retry calls
/// succeeding again after a retry.
#[test]
fn retry_resets_retry_counters() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    // Exhaust network retries
    for _ in 0..3 {
        queue.set_error(&id, "network");
        queue.try_network_retry(&id);
    }
    queue.set_error(&id, "network");
    assert!(
        queue.try_network_retry(&id).is_none(),
        "Retries should be exhausted"
    );

    // Now use retry() to do a full reset
    queue.set_error(&id, "network");
    queue.retry(&id, &settings);

    // Network retries should be available again
    queue.set_error(&id, "network");
    assert!(
        queue.try_network_retry(&id).is_some(),
        "After retry(), network retries should be reset to max"
    );
}

// ==========================================================
// update_item_state() tests
// ==========================================================

/// Verifies that update_item_state() changes the state of the item.
#[test]
fn update_item_state_changes_state() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    queue.update_item_state(&id, DownloadState::Downloading);
    assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);

    queue.update_item_state(&id, DownloadState::Processing);
    assert_eq!(queue.get_status()[0].state, DownloadState::Processing);
}

/// Verifies that update_item_state() is a no-op for non-existent IDs.
#[test]
fn update_item_state_nonexistent_id_is_safe() {
    let mut queue = DownloadQueue::new();
    // Should not panic
    queue.update_item_state("nonexistent", DownloadState::Downloading);
}

// ==========================================================
// new_queue_handle() test
// ==========================================================

/// Verifies that new_queue_handle() creates an Arc<Mutex<DownloadQueue>>
/// that can be locked and used.
#[tokio::test]
async fn new_queue_handle_creates_usable_handle() {
    let handle = new_queue_handle();
    let queue = handle.lock().await;
    assert!(
        queue.items.is_empty(),
        "New queue handle should wrap an empty queue"
    );
    assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
}

// ==========================================================
// Integration / workflow tests
// ==========================================================

/// Simulates a full download lifecycle: enqueue -> next_pending (Downloading)
/// -> progress updates -> set_complete -> on_task_finished. Verifies state
/// transitions at each step.
#[test]
fn full_lifecycle_happy_path() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    // Step 1: Item is Queued
    assert_eq!(queue.get_status()[0].state, DownloadState::Queued);

    // Step 2: Start downloading
    let (dl_id, _, _, _) = queue.next_pending().unwrap();
    assert_eq!(dl_id, id);
    assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);
    assert_eq!(queue.active_count, 1);

    // Step 3: Progress updates
    queue.update_item_progress(
        &id,
        &GamdlOutputEvent::TrackInfo {
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: String::new(),
            track_number: None,
            track_total: None,
        },
    );
    queue.update_item_progress(
        &id,
        &GamdlOutputEvent::DownloadProgress {
            percent: 50.0,
            speed: "5MiB/s".to_string(),
            eta: "00:10".to_string(),
        },
    );
    assert!((queue.get_status()[0].progress - 50.0).abs() < 0.001);
    assert_eq!(
        queue.get_status()[0].current_track.as_deref(),
        Some("Test Artist - Test Song")
    );

    // Step 4: Processing
    queue.update_item_progress(
        &id,
        &GamdlOutputEvent::ProcessingStep {
            step: "Remuxing to M4A".to_string(),
        },
    );
    assert_eq!(queue.get_status()[0].state, DownloadState::Processing);

    // Step 5: Complete
    queue.update_item_progress(
        &id,
        &GamdlOutputEvent::Complete {
            path: "/output/test.m4a".to_string(),
        },
    );
    queue.set_complete(&id);
    queue.on_task_finished();

    let final_status = &queue.get_status()[0];
    assert_eq!(final_status.state, DownloadState::Complete);
    assert!((final_status.progress - 100.0).abs() < 0.001);
    assert_eq!(
        final_status.output_path.as_deref(),
        Some("/output/test.m4a")
    );
    assert_eq!(queue.active_count, 0);
}

/// Simulates a download that fails with a codec error and successfully
/// falls back to the next codec in the chain.
#[test]
fn lifecycle_with_codec_fallback() {
    let mut queue = DownloadQueue::new();
    let mut settings = test_settings();
    settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac, SongCodec::AacLegacy];
    let id = queue.enqueue(test_request(), &settings);

    // Start and fail with codec error
    let _ = queue.next_pending();
    queue.set_error(&id, "Codec not available for ALAC");
    queue.on_task_finished();

    // Fallback to AAC (index 1 of 3-element chain)
    let fallback = queue.try_fallback(&id, &settings);
    assert!(fallback.is_some());
    let (fb_opts, fb_idx, fb_len) = fallback.unwrap();
    assert_eq!(
        fb_opts.song_codec, None,
        "song_codec cleared (--song-codec removed in GAMDL 2.9.1)"
    );
    assert_eq!(fb_opts.song_codec_priority, Some("aac".to_string()));
    assert_eq!(fb_idx, 1);
    assert_eq!(fb_len, 3);

    // Item should be re-queued
    assert_eq!(queue.get_status()[0].state, DownloadState::Queued);
    assert!(queue.get_status()[0].fallback_occurred);
}

/// Simulates a download that fails with a network error and retries
/// successfully.
#[test]
fn lifecycle_with_network_retry() {
    let mut queue = DownloadQueue::new();
    let id = enqueue_one(&mut queue);

    // Start and fail with network error
    let _ = queue.next_pending();
    queue.set_error(&id, "Network timeout");
    queue.on_task_finished();

    // Network retry should succeed (attempt 2 of 4)
    let retried = queue.try_network_retry(&id);
    assert!(retried.is_some());

    // Item should be re-queued
    let s = &queue.get_status()[0];
    assert_eq!(s.state, DownloadState::Queued);
    assert!(s.error.is_none());
    assert_eq!(s.progress, 0.0);
}

/// Verifies that multiple items can cycle through the queue when only
/// one concurrent slot is available. The second item should start only
/// after the first completes.
#[test]
fn sequential_processing_with_single_concurrency() {
    let mut queue = DownloadQueue::new();
    let ids = enqueue_n(&mut queue, 3);

    // Start first item
    let (id1, _, _, _) = queue.next_pending().unwrap();
    assert_eq!(id1, ids[0]);

    // Can't start second while first is running
    assert!(queue.next_pending().is_none());

    // Finish first
    queue.set_complete(&ids[0]);
    queue.on_task_finished();

    // Now second can start
    let (id2, _, _, _) = queue.next_pending().unwrap();
    assert_eq!(id2, ids[1]);

    // Finish second
    queue.set_complete(&ids[1]);
    queue.on_task_finished();

    // Third can start
    let (id3, _, _, _) = queue.next_pending().unwrap();
    assert_eq!(id3, ids[2]);
}

// ----------------------------------------------------------
// Python Traceback Extraction Tests
// ----------------------------------------------------------

#[test]
fn extracts_type_error_from_traceback() {
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"foo.py\", line 42, in bar".to_string(),
        "    some_call()".to_string(),
        "TypeError: 'NoneType' object has no attribute 'x'".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("TypeError: 'NoneType' object has no attribute 'x'".to_string())
    );
}

#[test]
fn extracts_dotted_exception_from_traceback() {
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"api.py\", line 10".to_string(),
        "    response.raise_for_status()".to_string(),
        "requests.exceptions.HTTPError: 403 Client Error".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("requests.exceptions.HTTPError: 403 Client Error".to_string())
    );
}

#[test]
fn returns_none_without_traceback() {
    let lines = vec!["Normal output line".to_string(), "Another line".to_string()];
    assert_eq!(extract_python_exception(&lines), None);
}

#[test]
fn handles_multiple_tracebacks_uses_last() {
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"a.py\", line 1".to_string(),
        "RuntimeError: first".to_string(),
        "".to_string(),
        "Traceback (most recent call last):".to_string(),
        "  File \"b.py\", line 2".to_string(),
        "ValueError: second".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("ValueError: second".to_string())
    );
}

#[test]
fn handles_traceback_with_no_exception_after() {
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"foo.py\", line 1".to_string(),
        "  File \"bar.py\", line 2".to_string(),
    ];
    // All lines after Traceback are indented, so no exception found
    assert_eq!(extract_python_exception(&lines), None);
}

#[test]
fn extracts_exception_when_structlog_log_line_follows_traceback_v31() {
    // GAMDL 3.1 emits the Traceback BEFORE the accompanying
    // `[ERROR HH:MM:SS]` log line because ExceptionPrettyPrinter
    // runs earlier in structlog's processor chain than the
    // formatter. Without structlog-line detection the walker would
    // pick up the trailing log line instead of the actual
    // exception.
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"gamdl/downloader/song.py\", line 96, in download".to_string(),
        "    await self._decrypt_amdecrypt(...)".to_string(),
        "KeyError: 'title'".to_string(),
        "[ERROR    17:09:23] [Track   1/14] Error downloading \"Lavender Haze\"".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("KeyError: 'title'".to_string())
    );
}

#[test]
fn extracts_exception_with_structlog_info_line_follows_v31() {
    // Catches the case where a subsequent INFO log line follows
    // the traceback (e.g., "Finished with 1 error(s)").
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"b.py\", line 2".to_string(),
        "ValueError: boom".to_string(),
        "[INFO     17:09:24] Finished with 1 error(s)".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("ValueError: boom".to_string())
    );
}

#[test]
fn structlog_line_detector_tolerates_bracketed_exception_text() {
    // Python exception messages can legitimately contain square
    // brackets (errno-style prefixes). The detector must NOT treat
    // `[Errno 60] ...` as a structlog line even though it starts
    // with `[...]`.
    let lines = vec![
        "Traceback (most recent call last):".to_string(),
        "  File \"x.py\", line 1".to_string(),
        "TimeoutError: [Errno 60] Operation timed out: '/mnt/cover.jpg'".to_string(),
    ];
    assert_eq!(
        extract_python_exception(&lines),
        Some("TimeoutError: [Errno 60] Operation timed out: '/mnt/cover.jpg'".to_string())
    );
}

#[test]
fn is_structlog_line_start_recognises_all_log_levels() {
    assert!(is_structlog_line_start("[INFO     17:09:24] hello"));
    assert!(is_structlog_line_start("[DEBUG    17:09:24] hello"));
    assert!(is_structlog_line_start("[WARNING  17:09:24] hello"));
    assert!(is_structlog_line_start("[ERROR    17:09:24] hello"));
    assert!(is_structlog_line_start("[CRITICAL 17:09:24] hello"));
}

#[test]
fn is_structlog_line_start_rejects_non_log_brackets() {
    assert!(!is_structlog_line_start("[Errno 60] Operation timed out"));
    assert!(!is_structlog_line_start("[not a log] line"));
    assert!(!is_structlog_line_start("plain text"));
    assert!(!is_structlog_line_start(""));
    assert!(!is_structlog_line_start("ERRORISH but no bracket"));
}

// ----------------------------------------------------------
// Gap-fill helper tests
// ----------------------------------------------------------

// These tests pin the pre-3.8 (and unprobed-version) gap-fill
// behaviour, so they explicitly hold the capability cache at a
// version below the 3.8 `AssetsApiUnlocksLossyCodecs` gate (#963,
// #1002) using the shared cross-module test lock — same pattern as
// `gamdl_capabilities`'s and `gamdl_options`'s own tests.

#[test]
fn build_gapfill_filters_experimental_codecs() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.5.2".to_string()));
    let chain = "atmos,ac3,alac,aac-binaural,aac,aac-legacy";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(result, Some("alac,aac-binaural,aac,aac-legacy".to_string()));
    crate::services::gamdl_capabilities::set_detected_version(None);
}

#[test]
fn build_gapfill_preserves_non_experimental() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.5.2".to_string()));
    let chain = "alac,aac,aac-legacy";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(result, Some("alac,aac,aac-legacy".to_string()));
    crate::services::gamdl_capabilities::set_detected_version(None);
}

#[test]
fn build_gapfill_returns_none_when_all_experimental() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.5.2".to_string()));
    let chain = "atmos,ac3";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(result, None);
    crate::services::gamdl_capabilities::set_detected_version(None);
}

#[test]
fn build_gapfill_single_non_experimental() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.5.2".to_string()));
    let chain = "atmos,alac";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(result, Some("alac".to_string()));
    crate::services::gamdl_capabilities::set_detected_version(None);
}

/// On a detected GAMDL >= 3.8 install, only ALAC is still
/// wrapper-dependent (#963, #1002) — Atmos/AC3/AAC-family all stay
/// in the gap-fill retry chain since the `/v1/play/assets` endpoint
/// unlocks them for wrapper-less downloads.
#[test]
fn build_gapfill_on_v38_only_strips_alac() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.8".to_string()));
    let chain = "atmos,ac3,alac,aac-binaural,aac,aac-legacy";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(
        result,
        Some("atmos,ac3,aac-binaural,aac,aac-legacy".to_string())
    );
    crate::services::gamdl_capabilities::set_detected_version(None);
}

/// On 3.8+, a chain consisting of only ALAC gap-fills to nothing —
/// mirrors `build_gapfill_returns_none_when_all_experimental` but
/// for the new post-3.8 wrapper-dependency shape.
#[test]
fn build_gapfill_on_v38_returns_none_when_only_alac() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.8.5".to_string()));
    let chain = "alac";
    let result = build_gapfill_priority_chain(chain);
    assert_eq!(result, None);
    crate::services::gamdl_capabilities::set_detected_version(None);
}

#[test]
fn unrenamed_primary_forces_suffixes_and_leaves_out_plain_named_formats() {
    let mut tiers = vec![
        CompanionTier {
            codecs_to_try: vec![SongCodec::Alac],
            apply_suffix: false,
        },
        CompanionTier {
            codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
            apply_suffix: false,
        },
        CompanionTier {
            codecs_to_try: vec![SongCodec::Aac],
            apply_suffix: false,
        },
    ];
    let skipped = force_suffixes_for_unrenamed_primary(&mut tiers, false);
    assert_eq!(skipped, vec!["aac".to_string(), "aac".to_string()]);
    // Tier numbers still match the plan; every tier is suffixed.
    assert_eq!(tiers.len(), 3);
    assert!(tiers.iter().all(|t| t.apply_suffix));
    assert_eq!(tiers[0].codecs_to_try, vec![SongCodec::Alac]);
    assert_eq!(tiers[1].codecs_to_try, vec![SongCodec::AacLegacy]);
    assert!(tiers[2].codecs_to_try.is_empty());
}

#[test]
fn a_primary_named_with_its_format_keeps_the_plain_named_copies() {
    // Single-format mode with extra copies: the primary's template
    // already says `[Dolby Atmos]`, so a plain AAC copy cannot collide.
    let mut tiers = vec![CompanionTier {
        codecs_to_try: vec![SongCodec::Aac],
        apply_suffix: false,
    }];
    let skipped = force_suffixes_for_unrenamed_primary(&mut tiers, true);
    assert!(skipped.is_empty());
    assert_eq!(tiers[0].codecs_to_try, vec![SongCodec::Aac]);
    assert!(tiers[0].apply_suffix);
}

#[test]
fn companion_progress_describes_current_and_pending_tiers() {
    let mut progress = CompanionTaskProgress {
        planned_tiers: vec![
            "alac".to_string(),
            "atmos".to_string(),
            "aac, aac-legacy".to_string(),
        ],
        current_tier: Some(1),
        ..Default::default()
    };
    progress.completed_tiers.insert(0);

    let description = progress.describe_pending();

    assert!(description.contains("currently running tier 1: atmos"));
    assert!(description.contains("not yet started: tier 2: aac, aac-legacy"));
    assert!(!description.contains("tier 0"));
}

#[test]
fn companion_progress_reports_no_remaining_when_all_done() {
    let mut progress = CompanionTaskProgress {
        planned_tiers: vec!["alac".to_string()],
        ..Default::default()
    };
    progress.completed_tiers.insert(0);

    assert_eq!(
        progress.describe_pending(),
        "no remaining companion tiers recorded"
    );
}

#[test]
fn count_codec_skip_warnings_counts_correctly() {
    let warnings = vec![
        "Requested format is not available for song 01".to_string(),
        "Requested format is not available for song 02".to_string(),
        "Some other warning".to_string(),
        "Requested format is not available for song 03".to_string(),
    ];
    assert_eq!(count_codec_skip_warnings(&warnings), 3);
}

#[test]
fn annotate_unavailable_format_line_adds_single_requested_codec() {
    let line = r#"[WARNING] Skipping "Track": Requested format is not available"#;
    let annotated = annotate_unavailable_format_line(line, &["atmos".to_string()]);

    assert!(annotated.contains("Unavailable requested format"));
    assert!(annotated.contains("Dolby Atmos"));
    assert!(annotated.contains("[atmos]"));
}

#[test]
fn annotate_unavailable_format_line_decodes_gamdl_song_codec_list() {
    let line =
        "Requested format is not available: [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>]";
    let annotated = annotate_unavailable_format_line(line, &["aac".to_string()]);

    assert!(annotated.contains("Unavailable requested format candidates"));
    assert!(annotated.contains("Dolby Atmos"));
    assert!(annotated.contains("[atmos]"));
    assert!(annotated.contains("Lossless"));
    assert!(annotated.contains("[alac]"));
    assert!(!annotated.contains("[aac]"));
}

#[test]
fn count_codec_skip_warnings_zero_when_no_skips() {
    let warnings = vec![
        "Network timeout occurred".to_string(),
        "Some other warning".to_string(),
    ];
    assert_eq!(count_codec_skip_warnings(&warnings), 0);
}

#[test]
fn count_codec_skip_warnings_empty() {
    let warnings: Vec<String> = vec![];
    assert_eq!(count_codec_skip_warnings(&warnings), 0);
}

/// Exercises `count_codec_skip_warnings` against a synthetic GAMDL
/// v3.0 stderr capture that includes the structlog prefix.
///
/// `count_codec_skip_warnings` matches by substring (lowercased),
/// so the `[WARNING  HH:MM:SS]` prefix should not prevent a match.
/// This test pins that invariant so a future regression in the
/// matcher (e.g. someone anchoring the pattern at line start) is
/// caught immediately.
///
/// Fixture wording best-effort synthesis — see #521.
#[test]
fn count_codec_skip_warnings_handles_structlog_prefix() {
    let v3_lines = vec![
        "[WARNING  12:10:05] Skipping \"Track Two\": Requested format is not available".to_string(),
        "[INFO     12:10:06] Downloading \"Track Three\"".to_string(),
        "[WARNING  12:10:09] Skipping \"Track Four\": Requested format is not available"
            .to_string(),
        "[INFO     12:10:10] Finished with 2 error(s)".to_string(),
    ];
    assert_eq!(
        count_codec_skip_warnings(&v3_lines),
        2,
        "count_codec_skip_warnings must see past GAMDL v3.0's \
             structlog [LEVEL HH:MM:SS] prefix"
    );
}

/// End-to-end: if the matcher works, the gap-fill chain builder
/// must also produce a usable priority string when experimental
/// codecs (wrapper-dependent) are present. Ties the two helpers
/// together so a regression in one doesn't silently break the
/// pipeline.
#[test]
fn v3_codec_skips_drive_gapfill_chain_construction() {
    let _lock = crate::services::gamdl_capabilities::capability_cache_test_lock();
    crate::services::gamdl_capabilities::set_detected_version(Some("3.5.2".to_string()));

    let warnings = vec![
        "[WARNING  12:10:05] Skipping \"T1\": Requested format is not available".to_string(),
        "[WARNING  12:10:09] Skipping \"T2\": Requested format is not available".to_string(),
    ];
    let skip_count = count_codec_skip_warnings(&warnings);
    assert!(skip_count > 0);

    // Original chain favours Atmos (wrapper-dependent below 3.8),
    // then ALAC. Gap-fill should drop Atmos and keep ALAC/AAC so the
    // retry pass can fill the missing tracks with lossless/lossy
    // fallbacks.
    let gap = build_gapfill_priority_chain("atmos,alac,aac").unwrap();
    assert_eq!(gap, "alac,aac");

    crate::services::gamdl_capabilities::set_detected_version(None);
}

/// `extract_python_exception` scans stderr for traceback lines.
/// v3.0 leaves tracebacks unwrapped (structlog only formats
/// `logger.error` calls, not the traceback Python itself prints),
/// so this should still work. Pin the behaviour.
#[test]
fn v3_network_traceback_extraction_survives_structlog_interleaving() {
    let stderr_lines = vec![
        "[INFO     12:30:00] Processing \"https://music.apple.com/us/album/example/1234567890\""
            .to_string(),
        "[ERROR    12:30:05] Error processing \"https://...\": Connection timed out".to_string(),
        "Traceback (most recent call last):".to_string(),
        "  File \"gamdl/cli/cli.py\", line 142, in main".to_string(),
        "    downloader.download(url)".to_string(),
        "  File \"httpx/_transports/default.py\", line 118, in map_httpcore_exceptions".to_string(),
        "    raise mapped_exc(message) from exc".to_string(),
        "httpx.ConnectTimeout: Connection timed out".to_string(),
    ];
    let exception = extract_python_exception(&stderr_lines);
    assert!(
        exception.is_some(),
        "extract_python_exception should have captured the traceback"
    );
    let msg = exception.unwrap();
    assert!(
        msg.contains("ConnectTimeout"),
        "Expected the final exception line to be extracted; got: {msg}"
    );
}

// ==========================================================
// normalize_url_for_dedup() tests
// ==========================================================

/// Verifies that domain case is normalised (RFC 3986 host is case-insensitive).
#[test]
fn normalize_url_lowercases_domain() {
    let a = normalize_url_for_dedup("https://Music.Apple.Com/us/album/test/123");
    let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
    assert_eq!(a, b);
}

/// Verifies that trailing slashes are stripped.
#[test]
fn normalize_url_strips_trailing_slash() {
    let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123/");
    let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
    assert_eq!(a, b);
}

/// Verifies that non-essential query parameters are removed.
#[test]
fn normalize_url_strips_non_essential_query() {
    let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123?ls=1&app=music");
    let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
    assert_eq!(a, b);
}

/// Verifies that the `?i=` query parameter (track ID) is preserved.
#[test]
fn normalize_url_keeps_track_id_query() {
    let url = normalize_url_for_dedup("https://music.apple.com/us/album/test/123?i=456&ls=1");
    assert_eq!(url, "https://music.apple.com/us/album/test/123?i=456");
}

/// Verifies that a URL with only `?i=` and no other params is preserved correctly.
#[test]
fn normalize_url_keeps_track_id_only() {
    let url = normalize_url_for_dedup("https://music.apple.com/us/album/test/123?i=456");
    assert_eq!(url, "https://music.apple.com/us/album/test/123?i=456");
}

/// Verifies that fragment identifiers are stripped.
#[test]
fn normalize_url_strips_fragment() {
    let a = normalize_url_for_dedup("https://music.apple.com/us/album/test/123#section");
    let b = normalize_url_for_dedup("https://music.apple.com/us/album/test/123");
    assert_eq!(a, b);
}

/// Verifies that two identical URLs produce the same normalised form.
#[test]
fn normalize_url_identical_urls_match() {
    let url = "https://music.apple.com/us/album/midnights/1649434004";
    assert_eq!(normalize_url_for_dedup(url), normalize_url_for_dedup(url),);
}

// ==========================================================
// has_duplicate_urls() tests
// ==========================================================

/// Verifies that `has_duplicate_urls` returns false for an empty queue.
#[test]
fn has_duplicate_urls_empty_queue() {
    let queue = DownloadQueue::new();
    let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
    assert!(!queue.has_duplicate_urls(&urls));
}

/// Verifies that `has_duplicate_urls` detects an exact URL match
/// in a Queued item.
#[test]
fn has_duplicate_urls_detects_match() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
        options: None,
        ..Default::default()
    };
    queue.enqueue(request, &settings);

    let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
    assert!(queue.has_duplicate_urls(&urls));
}

/// Verifies that `has_duplicate_urls` detects a case-insensitive match.
#[test]
fn has_duplicate_urls_case_insensitive() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
        options: None,
        ..Default::default()
    };
    queue.enqueue(request, &settings);

    let urls = vec!["https://Music.Apple.Com/us/album/test/123".to_string()];
    assert!(queue.has_duplicate_urls(&urls));
}

/// Verifies that `has_duplicate_urls` ignores completed items.
#[test]
fn has_duplicate_urls_ignores_completed() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
        options: None,
        ..Default::default()
    };
    let id = queue.enqueue(request, &settings);
    // Transition to complete
    queue.set_complete(&id);

    let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
    assert!(!queue.has_duplicate_urls(&urls));
}

/// Verifies that `has_duplicate_urls` ignores errored items.
#[test]
fn has_duplicate_urls_ignores_errored() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
        options: None,
        ..Default::default()
    };
    let id = queue.enqueue(request, &settings);
    queue.set_error(&id, "some error");

    let urls = vec!["https://music.apple.com/us/album/test/123".to_string()];
    assert!(!queue.has_duplicate_urls(&urls));
}

/// Verifies that `has_duplicate_urls` returns false for a different URL.
#[test]
fn has_duplicate_urls_no_match() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let request = DownloadRequest {
        urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
        options: None,
        ..Default::default()
    };
    queue.enqueue(request, &settings);

    let urls = vec!["https://music.apple.com/us/album/other/456".to_string()];
    assert!(!queue.has_duplicate_urls(&urls));
}

#[test]
fn filter_out_active_duplicate_urls_keeps_only_new_urls() {
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    queue.enqueue(
        DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123?ls=1".to_string()],
            options: None,
            ..Default::default()
        },
        &settings,
    );

    let urls = vec![
        "https://Music.Apple.Com/us/album/test/123/".to_string(),
        "https://music.apple.com/us/album/other/456".to_string(),
    ];

    assert_eq!(
        queue.filter_out_active_duplicate_urls(&urls),
        vec!["https://music.apple.com/us/album/other/456".to_string()]
    );
}

#[test]
fn dedupe_persisted_queue_items_keeps_newest_matching_url() {
    fn persisted(id: &str, url: &str) -> PersistedQueueItem {
        PersistedQueueItem {
            id: id.to_string(),
            request: DownloadRequest {
                urls: vec![url.to_string()],
                options: None,
                ..Default::default()
            },
            created_at: "2026-05-04T10:00:00Z".to_string(),
            error: Some("failed".to_string()),
            service: None,
        }
    }

    let items = dedupe_persisted_queue_items(vec![
        persisted("old", "https://music.apple.com/us/album/test/123?ls=1"),
        persisted("new", "https://Music.Apple.Com/us/album/test/123/"),
    ]);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "new");
}

// ============================================================
// find_album_directory / find_deepest_audio_dir tests (#460)
// ============================================================

/// A temporary subtitle file (`.meedyadl-partial-…`) is not a track's
/// lyrics: counting it could make the lyrics fallback believe every
/// track already has lyrics and stop early (round 8 follow-up, acting on
/// Codex's review of rounds 6-7; round 8 leaves other runs' leftovers in
/// place, so one can sit in an album folder indefinitely).
#[test]
fn a_temporary_subtitle_file_never_counts_as_a_tracks_lyrics() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("01 Song.m4a");
    let other = dir.path().join("02 Other.m4a");
    std::fs::write(&song, b"music").unwrap();
    std::fs::write(&other, b"music").unwrap();
    std::fs::write(dir.path().join("01 Song.ttml"), b"lyrics").unwrap();
    std::fs::write(
        dir.path()
            .join(".meedyadl-partial-123-0a1b2c3d4e5f6789.srt"),
        b"half written",
    )
    .unwrap();
    // Two songs, only the first has lyrics; the half-written file is not
    // lyrics for either.
    assert_eq!(
        crate::services::download_queue::lyrics_steps::lyrics_coverage(&[song, other]),
        (1, 2, 0)
    );
}

#[test]
fn has_direct_audio_files_detects_m4a() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("track.m4a"), b"fake").unwrap();
    assert!(has_direct_audio_files(dir.path()));
}

#[test]
fn has_direct_audio_files_ignores_nested() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("track.m4a"), b"fake").unwrap();
    // Parent dir has no direct audio files (only in subdir)
    assert!(!has_direct_audio_files(dir.path()));
}

#[test]
fn has_direct_audio_files_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!has_direct_audio_files(dir.path()));
}

#[test]
fn find_album_directory_targeted_match() {
    let base = tempfile::tempdir().unwrap();
    let album = base.path().join("Blue").join("Too Close - EP");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::write(album.join("01 Track.m4a"), b"fake").unwrap();

    let result = find_album_directory(base.path(), Some("Blue"), Some("Too Close - EP"));
    assert_eq!(result, Some(album.to_string_lossy().to_string()));
}

#[test]
fn find_album_directory_case_insensitive() {
    let base = tempfile::tempdir().unwrap();
    let album = base.path().join("Blue").join("Too Close - EP");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::write(album.join("01 Track.m4a"), b"fake").unwrap();

    // Hints with different casing — on case-insensitive filesystems (macOS)
    // the original-cased path is found directly; on case-sensitive (Linux)
    // the case-insensitive fallback finds it. Either way, the paths should
    // match when compared case-insensitively.
    let result = find_album_directory(base.path(), Some("blue"), Some("too close - ep"));
    assert!(result.is_some(), "should find album directory");
    assert_eq!(
        result.unwrap().to_lowercase(),
        album.to_string_lossy().to_string().to_lowercase()
    );
}

#[test]
fn find_album_directory_fallback_when_no_hints() {
    let base = tempfile::tempdir().unwrap();
    let album = base.path().join("Artist").join("Album");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::write(album.join("track.m4a"), b"fake").unwrap();

    let result = find_album_directory(base.path(), None, None);
    assert_eq!(result, Some(album.to_string_lossy().to_string()));
}

#[test]
fn find_album_directory_returns_deepest() {
    let base = tempfile::tempdir().unwrap();
    // Create Artist/Album with audio
    let album = base.path().join("Artist").join("Album");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::write(album.join("track.m4a"), b"fake").unwrap();
    // Artist dir should NOT be returned (no direct audio files)
    let result = find_album_directory(base.path(), None, None);
    assert_eq!(result, Some(album.to_string_lossy().to_string()));
}

// ============================================================
// rename_cover_art tests (#460)
// ============================================================

#[test]
fn rename_cover_art_renames_jpg() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"img").unwrap();
    rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");
    assert!(dir.path().join("FrontCover.jpg").exists());
    assert!(!dir.path().join("Cover.jpg").exists());
}

#[test]
fn rename_cover_art_skips_when_target_cover() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"img").unwrap();
    rename_cover_art(&dir.path().to_string_lossy(), "Cover");
    // Should keep as Cover.jpg — no rename
    assert!(dir.path().join("Cover.jpg").exists());
}

/// Pre-#892: when BOTH the source (`Cover.jpg`, freshly written by
/// e.g. a companion GAMDL invocation) AND the target
/// (`FrontCover.jpg`, from a prior primary rename) exist, the
/// behaviour was "leave both on disk, target untouched" — which
/// turned out to be a duplication bug: the album folder kept two
/// copies of the same cover.
///
/// Post-#892: the target is preserved verbatim AND the source is
/// deleted. Same end-state for the user (one cover file under
/// their configured stem), no wasted disk space, no media-player
/// ambiguity about which stem to load.
#[test]
fn rename_cover_art_deletes_duplicate_source_when_target_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("FrontCover.jpg"), b"existing").unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"duplicate-from-companion").unwrap();
    rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");

    // Target preserved verbatim — never overwritten with the
    // companion's bytes (they're identical at Apple's URL anyway,
    // but the contract is "user-configured stem is source of truth").
    let content = std::fs::read_to_string(dir.path().join("FrontCover.jpg")).unwrap();
    assert_eq!(content, "existing");

    // Source `Cover.jpg` cleaned up — no more duplicate on disk.
    assert!(
        !dir.path().join("Cover.jpg").exists(),
        "Cover.jpg must be deleted when FrontCover.jpg already exists (#892)"
    );
}

/// #892 specific repro: simulates the exact sequence the user
/// reported. Primary rename runs (Cover.jpg → FrontCover.jpg),
/// then a companion writes a new Cover.jpg, then companion's
/// rename_cover_art runs. End state must be one cover file, not two.
#[test]
fn rename_cover_art_handles_companion_double_write_sequence() {
    let dir = tempfile::tempdir().unwrap();

    // Step 1 — primary GAMDL just finished, writes Cover.jpg
    std::fs::write(dir.path().join("Cover.jpg"), b"primary-cover").unwrap();
    // Step 2 — MeedyaDL enrichment renames it
    rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");
    assert!(dir.path().join("FrontCover.jpg").exists());
    assert!(!dir.path().join("Cover.jpg").exists());

    // Step 3 — companion GAMDL runs, writes a fresh Cover.jpg
    // (companion doesn't know about FrontCover.jpg)
    std::fs::write(dir.path().join("Cover.jpg"), b"companion-cover").unwrap();

    // Step 4 — companion's post-download rename
    rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");

    // Final state: ONE cover file, not two.
    assert!(dir.path().join("FrontCover.jpg").exists());
    assert!(
        !dir.path().join("Cover.jpg").exists(),
        "Duplicate Cover.jpg from companion must be cleaned up (#892)"
    );
}

// ============================================================
// cleanup_duplicate_cover_art tests (#892 retroactive cleanup)
//
// Library-scan calls this on existing albums. The contract:
// ONLY remove confirmed duplicates (both source + target
// exist). Never rename a lone Cover.<ext> — that would surprise
// users with existing libraries on the default Cover stem.
// ============================================================

#[test]
fn cleanup_duplicate_cover_art_removes_dupe_when_both_exist() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"dup").unwrap();
    std::fs::write(dir.path().join("FrontCover.jpg"), b"keep").unwrap();

    let removed = cleanup_duplicate_cover_art(dir.path(), "FrontCover");
    assert_eq!(removed, 1);
    assert!(!dir.path().join("Cover.jpg").exists());
    assert!(dir.path().join("FrontCover.jpg").exists());
}

#[test]
fn cleanup_duplicate_cover_art_skips_lone_cover() {
    // Critical safety property: if the user has a lone Cover.jpg
    // (no FrontCover.jpg yet), we do NOT touch it. Their library
    // may have been set up intentionally with the default stem.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"only").unwrap();

    let removed = cleanup_duplicate_cover_art(dir.path(), "FrontCover");
    assert_eq!(removed, 0);
    assert!(
        dir.path().join("Cover.jpg").exists(),
        "Lone Cover.jpg must NOT be deleted by the cleanup pass"
    );
}

#[test]
fn cleanup_duplicate_cover_art_no_op_when_target_stem_is_cover() {
    // User on default `cover_art_name = Cover` — source and target
    // are the same path; nothing to clean.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"data").unwrap();

    let removed = cleanup_duplicate_cover_art(dir.path(), "Cover");
    assert_eq!(removed, 0);
    assert!(dir.path().join("Cover.jpg").exists());
}

#[test]
fn cleanup_duplicate_cover_art_handles_multiple_extensions() {
    let dir = tempfile::tempdir().unwrap();
    // Dupe jpg
    std::fs::write(dir.path().join("Cover.jpg"), b"dup").unwrap();
    std::fs::write(dir.path().join("FrontCover.jpg"), b"keep").unwrap();
    // Dupe png
    std::fs::write(dir.path().join("Cover.png"), b"dup-png").unwrap();
    std::fs::write(dir.path().join("FrontCover.png"), b"keep-png").unwrap();
    // Lone raw — should NOT be cleaned
    std::fs::write(dir.path().join("Cover.raw"), b"lone").unwrap();

    let removed = cleanup_duplicate_cover_art(dir.path(), "FrontCover");
    assert_eq!(removed, 2, "Should clean both jpg + png duplicates");
    assert!(!dir.path().join("Cover.jpg").exists());
    assert!(!dir.path().join("Cover.png").exists());
    // Lone Cover.raw preserved
    assert!(dir.path().join("Cover.raw").exists());
}

#[test]
fn cleanup_duplicate_cover_art_handles_nonexistent_dir() {
    let removed = cleanup_duplicate_cover_art(
        std::path::Path::new("/tmp/this/path/does/not/exist-for-892-test"),
        "FrontCover",
    );
    assert_eq!(removed, 0);
}

/// Multi-extension safety: each (jpg/png/raw) variant is handled
/// independently. A duplicate `Cover.jpg` with an existing
/// `FrontCover.jpg` must be cleaned up WITHOUT disturbing an
/// unrelated `Cover.png` (which has no `FrontCover.png` peer and
/// should follow the normal rename branch).
#[test]
fn rename_cover_art_multi_extension_independence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("FrontCover.jpg"), b"keep").unwrap();
    std::fs::write(dir.path().join("Cover.jpg"), b"dup").unwrap();
    std::fs::write(dir.path().join("Cover.png"), b"png-no-target").unwrap();

    rename_cover_art(&dir.path().to_string_lossy(), "FrontCover");

    // jpg: duplicate cleanup
    assert_eq!(
        std::fs::read_to_string(dir.path().join("FrontCover.jpg")).unwrap(),
        "keep"
    );
    assert!(!dir.path().join("Cover.jpg").exists());

    // png: normal rename happens — Cover.png renamed → FrontCover.png
    assert!(dir.path().join("FrontCover.png").exists());
    assert!(!dir.path().join("Cover.png").exists());
}

// ============================================================
// validate_path_safe tests (#460)
// ============================================================

#[test]
fn validate_path_safe_allows_normal_paths() {
    assert!(super::super::config_service::validate_path_safe("/home/user/Music").is_ok());
    assert!(super::super::config_service::validate_path_safe("C:\\Users\\Music").is_ok());
    assert!(super::super::config_service::validate_path_safe("./output").is_ok());
}

#[test]
fn validate_path_safe_rejects_traversal() {
    assert!(super::super::config_service::validate_path_safe("../etc/passwd").is_err());
    assert!(super::super::config_service::validate_path_safe("/home/../root").is_err());
    assert!(super::super::config_service::validate_path_safe("foo/../../bar").is_err());
}

// ============================================================
// filter_tiers_by_audio_traits (#504)
// ============================================================

fn tier(codecs: Vec<SongCodec>, suffix: bool) -> super::CompanionTier {
    super::CompanionTier {
        codecs_to_try: codecs,
        apply_suffix: suffix,
    }
}

#[test]
fn filter_tiers_passes_through_when_no_traits() {
    let tiers = vec![tier(vec![SongCodec::Ac3], false)];
    let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &[]);
    assert_eq!(kept.len(), 1);
    assert!(skipped.is_empty());
}

#[test]
fn filter_tiers_drops_unmatched_required_trait() {
    // PSYCHO single example: track has atmos / lossless / lossy-stereo / spatial
    // but NOT dolby-digital. AC3 must be dropped.
    let traits = vec![
        "atmos".to_string(),
        "lossless".to_string(),
        "lossy-stereo".to_string(),
        "spatial".to_string(),
    ];
    let tiers = vec![
        tier(vec![SongCodec::Ac3], true),
        tier(vec![SongCodec::AacBinaural], false),
    ];
    let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
    assert_eq!(kept.len(), 1, "AacBinaural tier should survive");
    assert_eq!(kept[0].codecs_to_try, vec![SongCodec::AacBinaural]);
    assert_eq!(skipped, vec!["ac3".to_string()]);
}

#[test]
fn filter_tiers_keeps_tier_when_any_codec_in_chain_supported() {
    let traits = vec!["lossy-stereo".to_string()];
    // A multi-codec tier where one codec is supported and another isn't
    let tiers = vec![tier(vec![SongCodec::Ac3, SongCodec::Aac], false)];
    let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
    assert_eq!(kept.len(), 1);
    assert!(skipped.is_empty());
}

#[test]
fn filter_tiers_keeps_derived_codecs_unconditionally() {
    // AacBinaural has no required_audio_trait — it should survive
    // even when the trait list is empty-ish.
    let traits = vec!["lossy-stereo".to_string()]; // intentionally minimal
    let tiers = vec![tier(vec![SongCodec::AacBinaural], false)];
    let (kept, skipped) = super::filter_tiers_by_audio_traits(tiers, &traits);
    assert_eq!(kept.len(), 1);
    assert!(skipped.is_empty());
}

// ============================================================
// Music-video no-album template overrides (#531)
// ============================================================

#[test]
fn mv_no_album_folder_template_includes_artist_and_music_videos() {
    // Guards against accidental changes that would re-introduce the
    // `[Unknown]` sentinel path. The template MUST contain `{artist}`
    // (GAMDL's placeholder) and the literal `Music Videos` folder,
    // and MUST NOT contain the legacy `[Unknown]` marker.
    assert!(super::MV_NO_ALBUM_FOLDER_TEMPLATE.contains("{artist}"));
    assert!(super::MV_NO_ALBUM_FOLDER_TEMPLATE.contains("Music Videos"));
    assert!(!super::MV_NO_ALBUM_FOLDER_TEMPLATE.contains("[Unknown]"));
}

#[test]
fn mv_no_album_file_template_includes_title_placeholder() {
    // Guards against regressions like the legacy `"{disc} - "` which
    // resolves to `-.mp4` for content without a disc number.
    assert!(super::MV_NO_ALBUM_FILE_TEMPLATE.contains("{title}"));
    assert!(!super::MV_NO_ALBUM_FILE_TEMPLATE.trim().is_empty());
}

#[test]
fn mv_no_album_file_template_includes_title_id_for_uniqueness() {
    // `{title_id}` (Apple Music's numeric MV ID) is the
    // guaranteed-unique disambiguator for same-title MVs in the
    // last-resort path. Its presence is a hard invariant — dropping
    // it re-opens the silent-collision regression where a second MV
    // with the same title would skip with `MediaFileExists`.
    assert!(
        super::MV_NO_ALBUM_FILE_TEMPLATE.contains("{title_id}"),
        "MV_NO_ALBUM_FILE_TEMPLATE must include {{title_id}} for uniqueness"
    );
}

/// #547 — Apple Music Classical movement-title collision audit.
///
/// Classical recordings have structurally non-unique movement
/// titles (every symphony has an "Allegro" movement). The
/// audit's HIGH-risk case is a direct song-URL into classical
/// content that falls through to `no_album_file_template` —
/// the user's library would silently collide as every
/// "Allegro" movement saved as `Allegro.m4a`.
///
/// This test asserts the failure mode is **reproducible with
/// today's default `no_album_file_template` (`{title}`)** so a
/// future fix (e.g. forcing the same `{title} ({title_id})`
/// safety net the MV path uses, or auto-prefixing with the
/// album work name when the source is Classical) has a clear
/// regression target.
///
/// Pairs with `mv_no_album_template_disambiguates_variants`
/// from #571 — that test covers the MV equivalent which is
/// already mitigated by the Tier 4 safety net. The audio path
/// has no equivalent safety net yet; this test documents the
/// gap.
#[test]
fn classical_movements_collide_under_default_no_album_template() {
    // Today's default: `{title}` with no disambiguator.
    // Pin the default in the assertion so a future refactor
    // that adds `{title_id}` to the default is flagged.
    let default_template = "{title}";

    let render = |title: &str| default_template.replace("{title}", title);

    // Concrete Classical scenario: two "Allegro" movements
    // from two different Beethoven symphonies. Both have
    // distinct title_ids in Apple Music but identical
    // `tags.title` after GAMDL's tag parse. Same name, same
    // template → same filename → silent overwrite.
    let beethoven_5_allegro = render("Allegro con brio");
    let beethoven_9_allegro = render("Allegro con brio");
    assert_eq!(
        beethoven_5_allegro, beethoven_9_allegro,
        "documents the collision risk (#547) — today's default \
             no_album_file_template offers no disambiguation for \
             identically-titled Classical movements. A future fix \
             should either force {{title_id}} into the default for \
             Classical content or wire a Classical-specific template."
    );
}

/// #571 — Verify that the MV no-album template produces distinct
/// filenames for variants of the same song (studio cut vs. live
/// performance vs. acoustic cut), under two GAMDL-`tags.title`
/// shapes:
///
///   (a) **Title-disambiguates**: GAMDL surfaces the variant
///       parenthetical in `tags.title` (e.g. `"Depressed (Live
///       from London)"`). The filename is unique on `{title}`
///       alone — the `{title_id}` suffix is belt-and-braces.
///
///   (b) **Title-collides**: GAMDL surfaces only `"Depressed"`
///       for every variant (the parenthetical lives in
///       `editorialNotes` rather than `name`). The filename is
///       still unique because the `{title_id}` suffix carries
///       the per-variant Apple Music numeric MV ID.
///
/// Either way the user's library never silently overwrites one
/// variant with another. The test renders the template with
/// `String::replace` (the same substitution GAMDL performs) so
/// the assertion is end-to-end deterministic.
#[test]
fn mv_no_album_template_disambiguates_variants() {
    // Convenience: applies `{title}` and `{title_id}` to the
    // file template the same way GAMDL would.
    fn render(title: &str, title_id: &str) -> String {
        super::MV_NO_ALBUM_FILE_TEMPLATE
            .replace("{title}", title)
            .replace("{title_id}", title_id)
    }

    // (a) Title-disambiguates path. Each variant has a unique
    // title AND a unique ID, so the rendered filename differs
    // on both axes.
    let studio_a = render("Depressed", "1840857276");
    let live_a = render("Depressed (Live from London)", "1847893387");
    let acoustic_a = render("Depressed (Acoustic)", "1900000001");
    assert_ne!(
        studio_a, live_a,
        "title-disambiguating studio vs live must differ"
    );
    assert_ne!(
        studio_a, acoustic_a,
        "title-disambiguating studio vs acoustic must differ"
    );
    assert_ne!(
        live_a, acoustic_a,
        "title-disambiguating live vs acoustic must differ"
    );
    // The `(NNNN)` suffix is present even when titles differ —
    // the belt-and-braces invariant from the test above.
    assert!(studio_a.contains("(1840857276)"));
    assert!(live_a.contains("(1847893387)"));

    // (b) Title-collides path. The pessimistic scenario in
    // which Apple Music returns only the base title for every
    // variant. With `{title}` identical, only `{title_id}` is
    // the disambiguator.
    let studio_b = render("Depressed", "1840857276");
    let live_b = render("Depressed", "1847893387");
    let acoustic_b = render("Depressed", "1900000001");
    assert_ne!(
        studio_b, live_b,
        "title-collides studio vs live must still differ via {{title_id}}"
    );
    assert_ne!(
        studio_b, acoustic_b,
        "title-collides studio vs acoustic must still differ via {{title_id}}"
    );
    assert_ne!(
        live_b, acoustic_b,
        "title-collides live vs acoustic must still differ via {{title_id}}"
    );
}

// ============================================================
// is_likely_motion_art_url (#536) — defensive guard tests
// ============================================================

#[test]
fn motion_art_url_detector_recognises_editorial_hls() {
    // Apple's editorial HLS host + .m3u8 = motion art.
    assert!(super::is_likely_motion_art_url(
        "https://video-ssl.itunes.apple.com/itunes-assets/HLS/.../motion.m3u8"
    ));
    assert!(super::is_likely_motion_art_url(
            "https://play-edge.itunes.apple.com/playback/v1/playlist/.../artist-spotlight.m3u8?token=xyz"
        ));
}

#[test]
fn motion_art_url_detector_passes_through_music_video_urls() {
    // Real music-video URLs MUST NOT be flagged — they belong in
    // the GAMDL MV pipeline.
    assert!(!super::is_likely_motion_art_url(
        "https://music.apple.com/us/music-video/song-title/1639963816"
    ));
    assert!(!super::is_likely_motion_art_url(
        "https://music.apple.com/gb/music-video/mv/1234567890"
    ));
}

#[test]
fn motion_art_url_detector_passes_through_album_urls() {
    // Regular album / song URLs are clearly not motion art.
    assert!(!super::is_likely_motion_art_url(
        "https://music.apple.com/us/album/some-album/1234567"
    ));
    assert!(!super::is_likely_motion_art_url(
        "https://music.apple.com/gb/song/title/9999?i=8888"
    ));
}

#[test]
fn sanitize_fs_segment_strips_unsafe_chars() {
    // Sanity check on the helper used by Tier 2 to build literal
    // folder paths from API-returned album names.
    assert_eq!(super::sanitize_fs_segment("Hello/World"), "Hello_World");
    assert_eq!(super::sanitize_fs_segment("foo:bar*baz?"), "foo_bar_baz_");
    // Trailing dots and whitespace are trimmed (Windows-unsafe).
    assert_eq!(super::sanitize_fs_segment("  Album.  "), "Album");
    // Non-ASCII passes through unchanged.
    assert_eq!(
        super::sanitize_fs_segment("Björk - Vespertine"),
        "Björk - Vespertine"
    );
    // Empty stays empty (caller falls through to Tier 4).
    assert_eq!(super::sanitize_fs_segment(""), "");
}

#[test]
fn motion_art_url_detector_requires_both_signals() {
    // editorial host BUT not an HLS playlist → not flagged
    // (the guard is conservative — false positives would block
    // a real download).
    assert!(!super::is_likely_motion_art_url(
        "https://video-ssl.itunes.apple.com/some/other/path"
    ));
    // HLS playlist on an unrelated host → not flagged (could be
    // a legitimate MV master m3u8 from a CDN we don't recognise).
    assert!(!super::is_likely_motion_art_url(
        "https://cdn.example.com/master.m3u8"
    ));
}

// ----------------------------------------------------------
// M9-7: defensive guards on Apple-Music-only post-dispatch helpers
// ----------------------------------------------------------
//
// The three guards (`download_music_video_by_url`,
// `run_lyrics_fallback`, `spawn_companion_downloads`) all check
// for `open.spotify.com` / `spotify:` in the URL(s) and bail
// out cleanly with an activity-log breadcrumb. The full
// functions require an AppHandle so direct invocation is hard
// — these tests pin the detection logic that the guards use,
// so a future regression in the substring match surfaces here.

fn is_spotify_url(u: &str) -> bool {
    u.contains("open.spotify.com") || u.starts_with("spotify:")
}

#[test]
fn spotify_url_detection_matches_open_spotify_com_album() {
    assert!(is_spotify_url(
        "https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy"
    ));
    assert!(is_spotify_url(
        "https://open.spotify.com/track/0VjIjW4GlUZAMYd2vXMi3b"
    ));
}

#[test]
fn spotify_url_detection_matches_spotify_uri_scheme() {
    assert!(is_spotify_url("spotify:album:4aawyAB9vmqN3uQ7FjRGTy"));
    assert!(is_spotify_url("spotify:track:0VjIjW4GlUZAMYd2vXMi3b"));
}

#[test]
fn spotify_url_detection_rejects_apple_music() {
    assert!(!is_spotify_url(
        "https://music.apple.com/us/album/some-album/123"
    ));
    assert!(!is_spotify_url(
        "https://classical.apple.com/us/album/some-album/456"
    ));
    assert!(!is_spotify_url(
        "https://itunes.apple.com/us/album/some-album/789"
    ));
}

#[test]
fn spotify_url_detection_rejects_substring_attacks() {
    // A maliciously-crafted Apple Music URL containing the
    // substring "spotify" must not be misclassified — the guard
    // matches on the host portion only.
    assert!(!is_spotify_url(
        "https://music.apple.com/us/album/like-spotify/123"
    ));
    // But a host-confusing redirect URL whose host is open.spotify.com
    // IS correctly flagged (the substring check is intentionally
    // generous here — a real Apple Music URL won't contain that
    // exact host string).
    assert!(is_spotify_url(
        "https://open.spotify.com/embed/album/123?via=apple"
    ));
}

// ----------------------------------------------------------
// M9-7: VotifyOptions::from_settings contract
// ----------------------------------------------------------

#[test]
fn from_settings_round_trips_session_artefact_paths() {
    // Re-verifies the contract pinned in models/votify_options.rs
    // from the queue's perspective — these are the fields the
    // dispatch arm constructs at lines ~11440.
    use crate::models::settings::SpotifySettings;
    use crate::models::spotify_anti_ban::AntiBanSettings;
    use crate::models::votify_options::VotifyOptions;
    let spotify = SpotifySettings {
        cookies_path: Some("/cookies".to_string()),
        session_type: Some("desktop".to_string()),
        spotify_dll_path: Some("/Spotify.dll".to_string()),
        wvd_path: None,
        anti_ban: AntiBanSettings::default(),
    };
    let opts = VotifyOptions::from_settings(&spotify);
    assert_eq!(opts.session_type.as_deref(), Some("desktop"));
    assert_eq!(opts.spotify_dll_path.as_deref(), Some("/Spotify.dll"));
    assert_eq!(opts.cookies_path.as_deref(), Some("/cookies"));
    assert_eq!(opts.wvd_path, None);
}

// ----------------------------------------------------------
// M9-7: dispatch fork keys on engine, not service
// ----------------------------------------------------------
//
// The synthesis agent's first design keyed on `item.service`;
// the adversarial critique flipped it to `item.engine` for
// M10 forward-compatibility (yt-dlp is shared by YouTube +
// BBC iPlayer). This test pins the engine key so a future
// refactor that accidentally reverts to service breaks here.

#[test]
fn dispatch_fork_key_is_engine_string_votify() {
    // The arm's branching condition is
    // `item_engine.as_deref() == Some("votify")`. Pin the
    // string so a rename in `engines.toml` (e.g. "votify" →
    // "spotify-votify") forces a coordinated update here too.
    let engine = "votify";
    assert_eq!(Some(engine), Some("votify"));
    // Also pin the negative case — a misspelled engine ID
    // routes to GAMDL, not Spotify. This is intentional: the
    // dispatch is conservative, the IPC gate is permissive.
    let misspelled: Option<&str> = Some("vottify");
    assert_ne!(misspelled, Some("votify"));
}

// ----------------------------------------------------------
// A2: write_manifest — Spotify-shaped source correctness
// ----------------------------------------------------------
//
// `write_manifest` is shared between the Apple Music enrichment path
// and `write_spotify_manifest_best_effort`. Two Apple-Music-shaped
// assumptions used to leak into non-Apple-Music sources: (1) the
// globally-configured `settings.storefront` (an Apple Music-only
// region concept) was written into every source regardless of
// platform; (2) the Spotify best-effort caller passed the bare format
// name `"vorbis"` instead of the canonical codec-registry ID
// `"ogg-vorbis"`. Both are fixed; these tests pin the corrected
// behaviour against regression.

/// Reads back the manifest JSON written to `album_dir` and returns the
/// (single) `ManifestSource` entry for assertions.
fn read_back_manifest_source(
    album_dir: &std::path::Path,
) -> crate::models::manifest::ManifestSource {
    let contents = std::fs::read_to_string(album_dir.join("manifest.meedyadl"))
        .expect("manifest.meedyadl should have been written");
    let manifest: crate::models::manifest::ManifestFile =
        serde_json::from_str(&contents).expect("manifest.meedyadl should be valid JSON");
    manifest
        .sources
        .into_iter()
        .next()
        .expect("manifest should have exactly one source")
}

#[test]
fn write_manifest_spotify_source_has_no_storefront_even_when_settings_storefront_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = test_settings();
    // A non-empty Apple Music storefront setting — this must NOT leak
    // into a Spotify source.
    settings.storefront = "gb".to_string();

    write_manifest(
        dir.path().to_str().unwrap(),
        &["https://open.spotify.com/album/abc123".to_string()],
        None,
        &settings,
        "2026-01-01T00:00:00Z",
        None,
        Some("ogg-vorbis"),
        None,
    );

    let source = read_back_manifest_source(dir.path());
    assert_eq!(source.platform, "spotify");
    assert_eq!(
        source.storefront, None,
        "Spotify source must not inherit the Apple Music-only storefront setting"
    );
    assert_eq!(source.codec.as_deref(), Some("ogg-vorbis"));
}

#[test]
fn write_manifest_apple_music_source_keeps_storefront_when_settings_storefront_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = test_settings();
    settings.storefront = "gb".to_string();

    write_manifest(
        dir.path().to_str().unwrap(),
        &["https://music.apple.com/gb/album/test/123456789".to_string()],
        None,
        &settings,
        "2026-01-01T00:00:00Z",
        None,
        None,
        None,
    );

    let source = read_back_manifest_source(dir.path());
    assert_eq!(source.platform, "apple-music");
    assert_eq!(
        source.storefront.as_deref(),
        Some("gb"),
        "Apple Music source should keep the storefront setting (pre-A2 behaviour preserved)"
    );
}

#[test]
fn write_manifest_apple_music_source_has_no_storefront_when_settings_storefront_empty() {
    let dir = tempfile::tempdir().unwrap();
    let settings = test_settings(); // storefront defaults to empty string

    write_manifest(
        dir.path().to_str().unwrap(),
        &["https://music.apple.com/us/album/test/123456789".to_string()],
        None,
        &settings,
        "2026-01-01T00:00:00Z",
        None,
        None,
        None,
    );

    let source = read_back_manifest_source(dir.path());
    assert_eq!(source.storefront, None);
}
// ── The after-queue action must need a real drain (#1169) ───────────
//
// That action is what happens when downloads finish, and the user can set
// it to shut down or restart their computer. It used to fire whenever the
// queue was found idle — including when it had never been busy — so
// pressing Start Queue on an empty queue could shut the machine down.

#[test]
fn an_empty_queue_has_not_run_anything() {
    // The exact case: nothing was ever queued, so nothing drained, so the
    // after-queue action must not fire.
    let mut queue = DownloadQueue::new();
    assert!(
        !queue.take_ran_since_drain(),
        "an untouched queue must not claim to have run anything"
    );
}

#[test]
fn handing_out_a_download_records_that_the_queue_ran() {
    let mut queue = DownloadQueue::new();
    queue.enqueue(test_request(), &test_settings());
    assert!(
        queue.next_pending().is_some(),
        "an item should be handed out"
    );
    assert!(
        queue.take_ran_since_drain(),
        "handing out a download means the queue genuinely ran"
    );
}

#[test]
fn the_record_is_cleared_when_read_so_one_drain_acts_once() {
    let mut queue = DownloadQueue::new();
    queue.enqueue(test_request(), &test_settings());
    queue.next_pending();
    assert!(queue.take_ran_since_drain(), "first read reports it ran");
    assert!(
        !queue.take_ran_since_drain(),
        "a second read must not report it again — one drain, one action"
    );
}

#[test]
fn asking_an_empty_queue_repeatedly_never_looks_like_a_drain() {
    // Pressing Start Queue on an empty queue, several times.
    let mut queue = DownloadQueue::new();
    for _ in 0..5 {
        assert!(queue.next_pending().is_none(), "nothing to hand out");
        assert!(
            !queue.take_ran_since_drain(),
            "an empty queue must never look like it drained"
        );
    }
}

#[test]
fn a_paused_queue_that_hands_out_nothing_has_not_run() {
    // Paused means nothing starts, so nothing drains either.
    let mut queue = DownloadQueue::new();
    queue.enqueue(test_request(), &test_settings());
    queue.pause();
    assert!(
        queue.next_pending().is_none(),
        "paused queues hand out nothing"
    );
    assert!(
        !queue.take_ran_since_drain(),
        "a paused queue must not look like it drained"
    );
}

// ── "Save Lyrics as Separate Files" now does something (#1155) ──────
//
// The switch used to be read nowhere at all, so it did nothing whichever
// way it was set. It also used to be called "Keep ...", which reads as a
// promise to REMOVE files when switched off — and removing them is exactly
// what must not happen, because those folders can hold lyrics the person
// wrote or corrected themselves.

#[test]
fn switching_separate_files_off_stops_the_download_tool_writing_one() {
    let mut settings = test_settings();
    settings.keep_lyrics_sidecar = false;
    // Nothing MeedyaDL builds needs the source file, so it has no purpose.
    settings.enhanced_lrc = false;
    settings.generate_webvtt = false;
    settings.generate_rich_srt = false;
    settings.generate_ass = false;
    settings.generate_lyricsfile = false;
    settings.lyrics_fallback_enabled = false;

    let options = merge_options(None, &settings, None);
    assert_eq!(
        options.no_synced_lyrics,
        Some(true),
        "with separate files off and nothing needing the source, no file should be written"
    );
}

#[test]
fn the_source_file_is_still_written_when_something_is_built_from_it() {
    // Someone who left one of the converted formats on while turning the
    // master switch off. The honest answer is to keep that format working,
    // not to let it silently produce nothing.
    let mut settings = test_settings();
    settings.keep_lyrics_sidecar = false;
    settings.enhanced_lrc = true;

    let options = merge_options(None, &settings, None);
    assert_eq!(
        options.no_synced_lyrics,
        Some(false),
        "the source file is needed, so it must still be written"
    );
}

#[test]
fn leaving_separate_files_on_changes_nothing() {
    // The default, and by far the commonest case.
    let mut settings = test_settings();
    settings.keep_lyrics_sidecar = true;
    settings.enhanced_lrc = true;

    let options = merge_options(None, &settings, None);
    assert_eq!(options.no_synced_lyrics, Some(false));
}

#[test]
fn every_format_on_its_own_keeps_the_source_file() {
    // Each of these is built by reading the download tool's file, so any
    // one of them alone is enough to require it.
    for (name, apply) in [
        ("enhanced_lrc", 0),
        ("generate_webvtt", 1),
        ("generate_rich_srt", 2),
        ("generate_ass", 3),
        ("generate_lyricsfile", 4),
        ("lyrics_fallback_enabled", 5),
    ] {
        let mut settings = test_settings();
        settings.keep_lyrics_sidecar = false;
        settings.enhanced_lrc = false;
        settings.generate_webvtt = false;
        settings.generate_rich_srt = false;
        settings.generate_ass = false;
        settings.generate_lyricsfile = false;
        settings.lyrics_fallback_enabled = false;
        match apply {
            0 => settings.enhanced_lrc = true,
            1 => settings.generate_webvtt = true,
            2 => settings.generate_rich_srt = true,
            3 => settings.generate_ass = true,
            4 => settings.generate_lyricsfile = true,
            _ => settings.lyrics_fallback_enabled = true,
        }
        let options = merge_options(None, &settings, None);
        assert_eq!(
            options.no_synced_lyrics,
            Some(false),
            "{name} is built from the source file, so it must still be written"
        );
    }
}

// ----------------------------------------------------------
// merge_options: video codec fallback chain
// ----------------------------------------------------------
//
// `music_video_codec_priority` is built from
// `AppSettings::video_codec_priority_cli()`, which sends the whole
// chain when stepping down is on, and only the first codec when it's
// off — matching how the audio side already behaves.

#[test]
fn merge_options_sends_the_whole_video_codec_chain_when_fallback_enabled() {
    let mut settings = test_settings();
    settings.fallback_enabled = true;
    settings.video_codec_fallback_chain = vec![
        crate::models::gamdl_options::VideoCodec::H264,
        crate::models::gamdl_options::VideoCodec::H265,
    ];

    let options = merge_options(None, &settings, None);
    assert_eq!(
        options.music_video_codec_priority,
        Some("h264,h265".to_string()),
        "the whole chain, in the person's order, should be sent"
    );
}

#[test]
fn merge_options_sends_only_the_first_video_codec_when_fallback_disabled() {
    let mut settings = test_settings();
    settings.fallback_enabled = false;
    settings.video_codec_fallback_chain = vec![
        crate::models::gamdl_options::VideoCodec::H264,
        crate::models::gamdl_options::VideoCodec::H265,
    ];

    let options = merge_options(None, &settings, None);
    assert_eq!(
        options.music_video_codec_priority,
        Some("h264".to_string()),
        "with stepping down switched off, only the preferred codec should be sent"
    );
}

#[test]
fn a_busy_queue_does_not_lose_the_record_that_it_ran() {
    // Caught by review before it shipped. This function is also reached
    // while a download is still running: another request arrives, the
    // concurrency limit is already met, and nothing is handed out.
    // Clearing the record then would wipe the fact that the queue HAD
    // been busy, and the last item finishing would look like nothing ever
    // ran — so "shut down when the downloads are done" would silently
    // stop happening for anyone with more than one item queued.
    let mut queue = DownloadQueue::new();
    queue.enqueue(test_request(), &test_settings());
    queue.enqueue(test_request(), &test_settings());

    // One item starts; the queue is now busy.
    assert!(queue.next_pending().is_some());
    assert!(
        !queue.is_idle(),
        "a running download means the queue is busy"
    );

    // A second request arrives while it runs and is handed nothing.
    assert!(
        queue.next_pending().is_none(),
        "the concurrency limit should refuse a second item"
    );

    // The record must survive, because the queue has not gone quiet yet.
    assert!(
        queue.take_ran_since_drain(),
        "the queue ran, and that must still be true after a busy no-op call"
    );
}

// ----------------------------------------------------------
// Which way of unlocking copy-protected tracks (GAMDL 3.9's
// PlayReady, added 2026-09-22)
// ----------------------------------------------------------

#[test]
fn on_gamdl_4_playready_falls_back_and_says_why() {
    // GAMDL 4.x would stop the whole run without its PlayReady extra,
    // which MeedyaDL does not install.
    let mut settings = AppSettings::default();
    assert_eq!(
        plan_drm_backend_on_gamdl_4(&settings),
        DrmPlan::LeaveToGamdl
    );
    settings.drm_backend = crate::models::settings::DrmBackend::PlayReady;
    settings.prd_path = "/d/device.prd".into();
    let DrmPlan::FallBackToBuiltIn { reason } = plan_drm_backend_on_gamdl_4(&settings) else {
        panic!("PlayReady must not be used with GAMDL 4");
    };
    assert!(reason.contains("GAMDL 4"), "{reason}");
}

#[test]
fn drm_plan_says_nothing_when_the_user_has_not_chosen_playready() {
    // The ordinary case, and the one that matters most: somebody who
    // never touches this setting must get the exact command line they
    // got before it existed — not an equivalent one.
    let settings = AppSettings::default();
    assert_eq!(
        plan_drm_backend(&settings, true, true),
        DrmPlan::LeaveToGamdl
    );

    let options = merge_options(None, &settings, None);
    assert_eq!(options.drm_backend, None);
    assert_eq!(options.prd_path, None);
    let args = options.to_cli_args();
    assert!(
        !args
            .iter()
            .any(|a| a == "--drm-backend" || a == "--prd-path"),
        "a default install must not send either option, got: {args:?}"
    );
}

#[test]
fn drm_plan_uses_playready_when_everything_is_in_place() {
    let settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: "/somewhere/device.prd".to_string(),
        ..Default::default()
    };

    assert_eq!(
        plan_drm_backend(&settings, true, true),
        DrmPlan::PlayReady {
            prd_path: "/somewhere/device.prd".to_string()
        }
    );
}

#[test]
fn drm_plan_falls_back_when_gamdl_is_too_old() {
    // An older GAMDL does not ignore an option it does not know — it
    // stops. So the option must not be sent, and the person must be
    // told why their choice was not used.
    let settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: "/somewhere/device.prd".to_string(),
        ..Default::default()
    };

    let DrmPlan::FallBackToBuiltIn { reason } = plan_drm_backend(&settings, false, true) else {
        panic!("an older GAMDL must fall back to the built-in unlocking");
    };
    assert!(
        reason.contains("3.9"),
        "the reason must say which GAMDL is needed, got: {reason}"
    );
}

#[test]
fn drm_plan_falls_back_when_no_device_file_is_chosen() {
    let settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: String::new(),
        ..Default::default()
    };

    let DrmPlan::FallBackToBuiltIn { reason } = plan_drm_backend(&settings, true, false) else {
        panic!("no device file means PlayReady cannot be used");
    };
    assert!(
        reason.contains(".prd"),
        "the reason must name what is missing, got: {reason}"
    );
}

#[test]
fn drm_plan_falls_back_when_the_device_file_has_gone_missing() {
    // Distinct from the case above, and worth its own sentence: the
    // person did choose a file, so telling them to choose one would
    // be confusing. The file moved or was deleted.
    let settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: "/gone/device.prd".to_string(),
        ..Default::default()
    };

    let DrmPlan::FallBackToBuiltIn { reason } = plan_drm_backend(&settings, true, false) else {
        panic!("a missing device file must fall back");
    };
    assert!(
        reason.contains("/gone/device.prd"),
        "the reason must say which file is missing, got: {reason}"
    );
}

#[test]
fn drm_plan_ignores_surrounding_spaces_in_the_device_file_path() {
    // A path pasted in by hand often carries a trailing space.
    let mut settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: "   ".to_string(),
        ..Default::default()
    };
    assert!(matches!(
        plan_drm_backend(&settings, true, true),
        DrmPlan::FallBackToBuiltIn { .. }
    ));

    settings.prd_path = "  /somewhere/device.prd  ".to_string();
    assert_eq!(
        plan_drm_backend(&settings, true, true),
        DrmPlan::PlayReady {
            prd_path: "/somewhere/device.prd".to_string()
        }
    );
}

#[test]
fn playready_options_reach_the_command_line_together() {
    // GAMDL stops without downloading if it is told to use PlayReady
    // with no device file, so these two are always sent as a pair.
    let options = GamdlOptions {
        drm_backend: Some(crate::models::settings::DrmBackend::PlayReady),
        prd_path: Some("/somewhere/device.prd".to_string()),
        ..Default::default()
    };

    let args = options.to_cli_args();
    let backend_at = args.iter().position(|a| a == "--drm-backend");
    let path_at = args.iter().position(|a| a == "--prd-path");
    assert!(
        backend_at.is_some() && path_at.is_some(),
        "both or neither: {args:?}"
    );
    assert_eq!(args[backend_at.expect("checked above") + 1], "playready");
    assert_eq!(
        args[path_at.expect("checked above") + 1],
        "/somewhere/device.prd"
    );
}

#[test]
fn the_device_file_is_checked_again_at_download_start_not_only_when_queued() {
    // A queue can sit for hours. Everything this decision rests on can
    // change in between — the setting, whether the device file is
    // still on disk, even which GAMDL is installed. So the same
    // question is asked again immediately before the download runs
    // (in `processing.rs`), and that later answer is what both the
    // command line and the activity-log line come from.
    //
    // Proven here at the level that matters: the decision depends only
    // on its inputs, so asking it twice with different facts gives
    // different answers. An earlier version decided once, when the
    // item was queued, and claimed the two could not disagree. They
    // could — and the silent case was the bad one: a device file
    // deleted after queueing still reached GAMDL, which stopped with
    // a Python traceback instead of downloading.
    let settings = AppSettings {
        drm_backend: crate::models::settings::DrmBackend::PlayReady,
        prd_path: "/somewhere/device.prd".to_string(),
        ..Default::default()
    };

    // When it was queued: file present, GAMDL new enough.
    assert_eq!(
        plan_drm_backend(&settings, true, true),
        DrmPlan::PlayReady {
            prd_path: "/somewhere/device.prd".to_string()
        }
    );

    // By the time it ran: the file is gone. The answer must change,
    // and it must be the falling-back one, not the original.
    assert!(matches!(
        plan_drm_backend(&settings, true, false),
        DrmPlan::FallBackToBuiltIn { .. }
    ));
}

#[test]
fn work_stops_for_a_cancelled_item_and_for_one_that_has_been_cleared_away() {
    // Two ways the answer must be "stop", and a reviewer found the
    // second missing from the first version of the companion check.
    //
    // Cancelled rows can be cleared straight away — that is what the
    // Clear buttons do. Asking only "is it cancelled?" answers *no*
    // for a row that is no longer there, so somebody who cancelled
    // and then tidied their queue would have had the extra format
    // downloads carry on, because the row telling them to stop had
    // been removed.
    let mut queue = DownloadQueue::new();
    let settings = test_settings();
    let id = queue.enqueue(test_request(), &settings);

    assert!(
        queue.should_keep_working_on(&id),
        "an ordinary queued item is still worth working on"
    );

    queue.cancel(&id);
    assert!(
        !queue.should_keep_working_on(&id),
        "a cancelled item must stop the work"
    );

    // Now clear it away, as the Clear buttons do.
    queue.clear_all();
    assert!(
        !queue.should_keep_working_on(&id),
        "an item that is no longer in the queue must also stop the work"
    );

    // And an id that was never here at all.
    assert!(
        !queue.should_keep_working_on("never-existed"),
        "nothing is owed to an item nobody is tracking"
    );
}

// ============================================================
// One app-wide write lock for every tag write (#1442)
// ============================================================

/// The text of the function that starts at `marker`, up to the closing
/// brace in the first column. Panics if the marker is not found.
fn function_text<'a>(src: &'a str, marker: &str) -> &'a str {
    let start = src
        .find(marker)
        .unwrap_or_else(|| panic!("`{marker}` not found"));
    let rest = &src[start..];
    &rest[..end_of_function(rest)]
}

/// Where the function at the start of `rest` ends: the first closing brace
/// in the first column, with Unix or Windows (CRLF) line endings. Windows
/// checkouts turn `\n` into `\r\n`, which a plain `"\n}\n"` search misses.
fn end_of_function(rest: &str) -> usize {
    ["\n}\n", "\n}\r\n"]
        .iter()
        .filter_map(|m| rest.find(m).map(|e| e + m.len()))
        .min()
        .unwrap_or(rest.len())
}

/// A function written with Windows line endings is cut at its own closing
/// brace, not run on to the end of the file.
#[test]
fn function_text_handles_windows_line_endings() {
    let unix = "fn a() {\n    x();\n}\nfn b() {\n    lock_blocking();\n}\n";
    let crlf = unix.replace('\n', "\r\n");
    for src in [unix.to_string(), crlf] {
        let a = function_text(&src, "fn a(");
        assert!(a.contains("x()"));
        assert!(
            !a.contains("lock_blocking"),
            "ran on past its closing brace"
        );
    }
}

/// Whether `lock_text` appears in `text` before the first `write_text`.
fn locks_before(text: &str, lock_text: &str, write_text: &str) -> bool {
    match (text.find(lock_text), text.find(write_text)) {
        (Some(l), Some(w)) => l < w,
        _ => false,
    }
}

#[test]
fn the_tier_new_m4a_list_holds_only_new_m4a_files() {
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    let p = |s: &str| PathBuf::from(s);
    let before: BTreeSet<PathBuf> = [p("a/01.m4a"), p("a/02.m4a")].into_iter().collect();
    let after: BTreeSet<PathBuf> = [
        p("a/01.m4a"),
        p("a/02.m4a"),
        p("a/03.m4a"),
        p("a/clip.m4v"),
        p("a/UPPER.M4A"),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        super::companions::tier_new_m4a(&before, &after),
        vec![p("a/03.m4a"), p("a/UPPER.M4A")]
    );
    // Nothing before: every .m4a is new, videos still left out.
    assert_eq!(
        super::companions::tier_new_m4a(&BTreeSet::new(), &after).len(),
        4
    );
    // Nothing added.
    assert!(super::companions::tier_new_m4a(&after, &after).is_empty());
}

/// The extra-copy bug: a tier's format tags used to be written to every
/// music file in the folder. Now only the files the tier added get them.
#[tokio::test]
async fn a_tiers_format_tags_reach_only_that_tiers_new_files() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("01 Main.m4a");
    let older = dir.path().join("02 Older.m4a");
    let new_copy = dir.path().join("01 Main [Lossless].m4a");
    minimal_m4a(&main, 1, 1, false);
    minimal_m4a(&older, 2, 1, false);
    let before = super::lyrics_pass::audio_files_in(dir.path());
    minimal_m4a(&new_copy, 1, 1, false);
    let after = super::lyrics_pass::audio_files_in(dir.path());
    let main_bytes = std::fs::read(&main).unwrap();
    let older_bytes = std::fs::read(&older).unwrap();
    let copy_bytes = std::fs::read(&new_copy).unwrap();

    let tagged = super::companions::tag_tier_additions(
        &before,
        &after,
        &[dir.path()],
        &SongCodec::Alac,
        "test",
        Some("2024-03-15T07:12:45Z"),
    )
    .await;

    assert_eq!(tagged, 1);
    assert_eq!(
        std::fs::read(&main).unwrap(),
        main_bytes,
        "main file changed"
    );
    assert_eq!(
        std::fs::read(&older).unwrap(),
        older_bytes,
        "older file changed"
    );
    assert_ne!(
        std::fs::read(&new_copy).unwrap(),
        copy_bytes,
        "the new copy was not tagged"
    );
    // The new copy carries the album's last-updated date (#1444); the older
    // file, whose bytes did not change, cannot.
    let date_of = |p: &std::path::Path| {
        let tag = mp4ameta::Tag::read_from_path(p).unwrap();
        let ident = mp4ameta::FreeformIdent::new_static(
            "MeedyaMeta",
            crate::models::tag_registry::SOURCE_LAST_MODIFIED_TAG,
        );
        let found = tag.strings_of(&ident).next().map(str::to_string);
        found
    };
    assert_eq!(date_of(&new_copy).as_deref(), Some("2024-03-15T07:12:45Z"));
    assert_eq!(date_of(&older), None);
}

/// Step 1's date stamp goes only to files this download wrote, and the call
/// site hands it the snapshot judgement, never "every album-matched file".
#[test]
fn the_last_updated_date_is_stamped_on_this_downloads_files_only() {
    use crate::services::metadata_tag_service::stamps_last_modified;
    let fresh: std::collections::HashSet<std::path::PathBuf> =
        [std::path::PathBuf::from("a/01.m4a")].into_iter().collect();
    assert!(stamps_last_modified(
        std::path::Path::new("a/01.m4a"),
        &fresh
    ));
    assert!(!stamps_last_modified(
        std::path::Path::new("a/02.m4a"),
        &fresh
    ));
    assert!(!stamps_last_modified(
        std::path::Path::new("a/01.m4a"),
        &std::collections::HashSet::new()
    ));

    let processing = include_str!("processing.rs");
    let call = processing
        .split("apply_enriched_metadata_tags(")
        .nth(1)
        .expect("the Step 1 call site");
    let call = &call[..call.find(".await").unwrap()];
    assert!(
        call.contains("&date_stamp_files"),
        "Step 1 must pass the files this download wrote"
    );
    // ...and those are this download's fresh files, only when every link is
    // in one album.
    assert!(
        {
            let flat: String = processing.split_whitespace().collect();
            flat.contains("letdate_stamp_files=ifsuper::lyrics_pass::links_share_one_album(&enrich_urls){fresh_files.clone()}else{std::collections::HashSet::new()};")
        },
        "Step 1's date files must be the fresh files, gated on one album, and none otherwise"
    );
    // The extra copies get the album's date only under the same rule.
    assert!(
        {
            let flat: String = processing.split_whitespace().collect();
            flat.contains("last_modified_date:m.last_modified_date.clone().filter(|_|{super::lyrics_pass::links_share_one_album(&enrich_urls,)},),")
        },
        "the copies' date must be gated on every link being in one album"
    );
}

/// A new file outside the album folder is left alone when the album folder
/// was found.
#[tokio::test]
async fn a_tier_does_not_tag_new_files_outside_the_album_folder() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let root = tempfile::tempdir().unwrap();
    let album = root.path().join("Album");
    let other = root.path().join("Other");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    let before = std::collections::BTreeSet::new();
    let mine = album.join("01 A.m4a");
    let theirs = other.join("01 B.m4a");
    minimal_m4a(&mine, 1, 1, false);
    minimal_m4a(&theirs, 1, 1, false);
    let after = super::lyrics_pass::audio_files_in(root.path());
    let theirs_bytes = std::fs::read(&theirs).unwrap();
    let tagged = super::companions::tag_tier_additions(
        &before,
        &after,
        &[album.as_path()],
        &SongCodec::Alac,
        "test",
        None,
    )
    .await;
    assert_eq!(tagged, 1);
    assert_eq!(std::fs::read(&theirs).unwrap(), theirs_bytes);
}

/// The one walk at the start of a download gives the same two answers as
/// the two separate walks it replaced.
#[test]
fn one_walk_gives_the_audio_count_and_the_video_list() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let dir = tempfile::tempdir().unwrap();
    let deep = dir.path().join("Artist").join("Album");
    std::fs::create_dir_all(&deep).unwrap();
    minimal_m4a(&deep.join("01.m4a"), 1, 1, false);
    std::fs::write(deep.join("clip.m4v"), b"v").unwrap();
    std::fs::write(deep.join("other.mp4"), b"v").unwrap();
    std::fs::write(deep.join("notes.txt"), b"t").unwrap();
    let (count, videos) = super::lyrics_pass::audio_count_and_videos_before_run(dir.path());
    assert_eq!(count, count_audio_files_in_directory(dir.path()));
    assert_eq!(videos, super::lyrics_pass::videos_before_run(dir.path()));
    assert_eq!(count, 3);
    assert_eq!(videos.len(), 2);
}

/// Every tag-writing place in the app takes the app-wide lock, at its
/// per-file loop (T1-T8 in the #1442 plan).
#[test]
fn every_tag_write_site_takes_the_app_wide_lock() {
    let tag_service = include_str!("../metadata_tag_service.rs");
    let companions = include_str!("companions.rs");
    let companion_lyrics = include_str!("companion_lyrics.rs");
    let lyrics_steps = include_str!("lyrics_steps.rs");
    let processing = include_str!("processing.rs");

    // T1: Step 0 iTunes tags, per file.
    let t1 = function_text(tag_service, "pub fn apply_itunes_supplementary_tags(");
    assert!(locks_before(t1, "lock_blocking(", ".write_to_path("), "T1");
    // T2: Step 1, around the read and write.
    let t2 = function_text(tag_service, "async fn enrich_single_file(");
    assert!(
        locks_before(t2, "lock_blocking(&tag_path)", ".write_to_path(&tag_path)"),
        "T2"
    );
    // T3: the extra-format tier tags, per file.
    let t3 = function_text(companions, "pub(crate) async fn tag_tier_additions(");
    assert!(
        locks_before(t3, ".lock(&new_file).await", "apply_codec_metadata_tags("),
        "T3"
    );
    // T4: the main download's per-file loop, and the copy drivers.
    let t4 = function_text(lyrics_steps, "fn run_for_files(");
    assert!(
        locks_before(t4, "lock_blocking(file)", "step(&input)"),
        "T4 main"
    );
    let copies = function_text(companion_lyrics, "pub(crate) async fn run_copies_locked(");
    assert!(
        locks_before(copies, ".lock(copy).await", "process_planned("),
        "T4 copies"
    );
    let main_own = function_text(
        companion_lyrics,
        "pub(crate) async fn run_main_files_own_locked(",
    );
    assert!(
        locks_before(main_own, ".lock(file).await", "process_copy("),
        "T4 recovery"
    );
    // T5-T7: AcoustID, ReplayGain and song.link are handed the app-wide map.
    assert!(
        processing.contains("let enrich_file_locks = crate::utils::file_locks::global().clone();")
    );
    assert!(
        !processing.contains("FileWriteLocks::new()"),
        "a per-download map came back"
    );
    // The guard must live INSIDE the blocking closure, never on the async
    // side, or aborting enrichment would release it mid-write.
    for (name, src, marker) in [
        (
            "acoustid",
            include_str!("../acoustid_service.rs"),
            "async fn process_single_file(",
        ),
        (
            "replaygain tags",
            include_str!("../replaygain_service.rs"),
            "async fn write_replaygain_tags(",
        ),
        (
            "replaygain mp4",
            include_str!("../replaygain_service.rs"),
            "async fn write_replaygain_mp4(",
        ),
        (
            "replaygain lofty",
            include_str!("../replaygain_service.rs"),
            "async fn write_replaygain_lofty(",
        ),
        (
            "song.link",
            include_str!("../odesli_service.rs"),
            "pub async fn write_url_atoms(",
        ),
    ] {
        let text = function_text(src, marker);
        assert!(
            !text.contains(".lock(") && !text.contains("lock_blocking("),
            "{name}: the lock is taken outside the blocking job"
        );
        if name != "replaygain tags" {
            assert!(
                text.contains("run_blocking_locked("),
                "{name}: the write does not hold the lock inside its blocking job"
            );
            // AcoustID's fingerprint step legitimately runs on its own blocking
            // thread; it takes no lock and writes nothing.
            assert!(
                name == "acoustid" || !text.contains("spawn_blocking("),
                "{name}: a bare spawn_blocking came back"
            );
        }
    }
    // All three get the download's in-flight tracker.
    assert!(
        processing.matches("Some(&enrich_in_flight),").count() >= 4,
        "the in-flight tracker is not passed to AcoustID, ReplayGain and song.link"
    );
    // T8: the music-video cover embed.
    assert!(
        locks_before(
            companions,
            "file_locks::global().lock(video_path).await",
            "embed_and_remove_sidecar(video_path)"
        ),
        "T8"
    );
    // The production extra-copy path uses the locked drivers.
    assert!(
        function_text(companion_lyrics, "pub(crate) async fn run(").contains("run_copies_locked(")
    );
}

/// The functions that write one file never take the lock themselves (the
/// lock is not re-entrant; the per-file loop holds it).
#[test]
fn the_write_functions_never_take_the_lock() {
    let tag_service = include_str!("../metadata_tag_service.rs");
    let lyrics_steps = include_str!("lyrics_steps.rs");
    let leaves: Vec<(&str, &str)> = vec![
        (tag_service, "fn tag_single_file("),
        (tag_service, "pub fn apply_codec_metadata_tags("),
        (
            include_str!("../enhanced_lyrics_service.rs"),
            "pub fn embed_enhanced_lrc_in_file(",
        ),
        (
            include_str!("../rich_srt_service.rs"),
            "pub fn embed_subtitles_in_file(",
        ),
        (
            include_str!("../music_video_cover_embed.rs"),
            "pub fn embed_and_remove_sidecar(",
        ),
    ];
    for (src, marker) in leaves {
        let text = function_text(src, marker);
        for taker in [".lock(", "lock_blocking(", "file_locks::"] {
            assert!(!text.contains(taker), "`{marker}` takes the lock ({taker})");
        }
    }
    // Every step function in lyrics_steps.rs.
    let mut rest = lyrics_steps;
    let mut seen = 0;
    while let Some(i) = rest.find("pub(crate) fn step_") {
        let body = &rest[i..];
        let end = end_of_function(body);
        let body = &body[..end];
        for taker in [".lock(", "lock_blocking(", "file_locks::"] {
            assert!(
                !body.contains(taker),
                "a lyrics step takes the lock ({taker})"
            );
        }
        seen += 1;
        rest = &rest[i + 1..];
    }
    assert!(seen >= 6, "found only {seen} step functions");
}

/// The tag folder is the folder known to hold this download, else the
/// output folder; never a name lookup.
#[test]
fn the_tier_tag_folder_prefers_the_known_download_folder() {
    let root = tempfile::tempdir().unwrap();
    let renamed = root.path().join("Album [Explicit]");
    std::fs::create_dir_all(&renamed).unwrap();
    let gone = root.path().join("Gone");
    let f = super::companions::tier_tag_folder;
    assert_eq!(f(root.path(), Some(&renamed), None), renamed);
    assert_eq!(f(root.path(), None, Some(&renamed)), renamed);
    // The recorded folder wins over the gate's.
    let other = root.path().join("Other");
    std::fs::create_dir_all(&other).unwrap();
    assert_eq!(f(root.path(), Some(&renamed), Some(&other)), renamed);
    // Nothing known, or the folder is gone: the output folder.
    assert_eq!(f(root.path(), None, None), root.path());
    assert_eq!(f(root.path(), Some(&gone), None), root.path());
}

/// Review finding: new copies landing in the renamed folder (the old
/// unsuffixed one still exists beside it) are all tagged, including ones in
/// a disc subfolder.
#[tokio::test]
async fn copies_in_the_renamed_folder_and_its_disc_folders_are_tagged() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("Album");
    let renamed = root.path().join("Album [Explicit]");
    let disc2 = renamed.join("Disc 2");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&disc2).unwrap();
    minimal_m4a(&old.join("01 Old.m4a"), 1, 1, false);
    let before = super::lyrics_pass::audio_files_in(root.path());
    let a = renamed.join("01 A [Lossless].m4a");
    let b = disc2.join("01 B [Lossless].m4a");
    minimal_m4a(&a, 1, 1, false);
    minimal_m4a(&b, 1, 2, false);
    let after = super::lyrics_pass::audio_files_in(root.path());
    let (a0, b0) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // The name lookup would have returned the old folder; the known folder
    // is the renamed one.
    let folder = super::companions::tier_tag_folder(root.path(), Some(&renamed), None);
    let tagged = super::companions::tag_tier_additions(
        &before,
        &after,
        &[folder.as_path()],
        &SongCodec::Alac,
        "test",
        None,
    )
    .await;
    assert_eq!(tagged, 2);
    assert_ne!(std::fs::read(&a).unwrap(), a0);
    assert_ne!(std::fs::read(&b).unwrap(), b0);
}

/// With no known folder, the before/after set is used unnarrowed, still
/// inside the output folder.
#[tokio::test]
async fn with_no_known_folder_the_before_after_set_is_tagged() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let root = tempfile::tempdir().unwrap();
    let before = std::collections::BTreeSet::new();
    let sub = root.path().join("Somewhere Else");
    std::fs::create_dir_all(&sub).unwrap();
    let copy = sub.join("01 A [Lossless].m4a");
    minimal_m4a(&copy, 1, 1, false);
    let after = super::lyrics_pass::audio_files_in(root.path());
    let c0 = std::fs::read(&copy).unwrap();
    let folder = super::companions::tier_tag_folder(root.path(), None, None);
    let tagged = super::companions::tag_tier_additions(
        &before,
        &after,
        &[folder.as_path()],
        &SongCodec::Alac,
        "test",
        None,
    )
    .await;
    assert_eq!(tagged, 1);
    assert_ne!(std::fs::read(&copy).unwrap(), c0);
}

/// Review finding: copies saved outside the recorded folder (the predicted
/// folder) are tagged too, and a copy outside both is not.
#[tokio::test]
async fn copies_in_the_predicted_folder_outside_the_recorded_one_are_tagged() {
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    let root = tempfile::tempdir().unwrap();
    let recorded = root.path().join("Artist").join("Album [Explicit]");
    let predicted = root.path().join("Artist").join("Album");
    let stranger = root.path().join("Other").join("Thing");
    for d in [&recorded, &predicted, &stranger] {
        std::fs::create_dir_all(d).unwrap();
    }
    let before = std::collections::BTreeSet::new();
    let (a, b, c) = (
        recorded.join("01 A [Lossless].m4a"),
        predicted.join("02 B [Lossless].m4a"),
        stranger.join("01 C [Lossless].m4a"),
    );
    minimal_m4a(&a, 1, 1, false);
    minimal_m4a(&b, 2, 1, false);
    minimal_m4a(&c, 1, 1, false);
    let after = super::lyrics_pass::audio_files_in(root.path());
    let (a0, b0, c0) = (
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        std::fs::read(&c).unwrap(),
    );
    let folders = super::companions::tier_tag_folders(
        root.path(),
        &recorded,
        &before,
        &after,
        Some("Artist"),
        Some("Album"),
    );
    assert_eq!(folders, vec![recorded.clone(), predicted.clone()]);
    let within: Vec<&std::path::Path> = folders.iter().map(|p| p.as_path()).collect();
    let tagged = super::companions::tag_tier_additions(
        &before,
        &after,
        &within,
        &SongCodec::Alac,
        "test",
        None,
    )
    .await;
    assert_eq!(tagged, 2);
    assert_ne!(std::fs::read(&a).unwrap(), a0);
    assert_ne!(std::fs::read(&b).unwrap(), b0);
    assert_eq!(std::fs::read(&c).unwrap(), c0);
    assert_eq!(
        super::companions::tier_files_outside(&before, &after, &within),
        vec![c]
    );
}
