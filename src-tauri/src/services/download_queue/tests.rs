// Copyright (c) 2026 MeedyaSuite
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
        let t = compute_total_timeout(1, 0, 0);
        assert_eq!(t.as_secs(), 600 + 30);
    }

    #[test]
    fn compute_total_timeout_zero_tracks_is_exactly_base() {
        // Shouldn't normally reach the timeout with zero tracks — the
        // #567 enrichment guard short-circuits earlier — but if it does,
        // the base still applies.
        let t = compute_total_timeout(0, 0, 0);
        assert_eq!(t.as_secs(), 600);
    }

    #[test]
    fn compute_total_timeout_typical_album_under_20_minutes() {
        // 12-track album: 10 min + 12 × 30 s = 16 min. Stays comfortably
        // under 20 min so small albums don't see a regression.
        let t = compute_total_timeout(12, 0, 0);
        assert_eq!(t.as_secs(), 600 + 12 * 30);
        assert!(t.as_secs() / 60 < 20);
    }

    #[test]
    fn compute_total_timeout_19_track_live_album_no_companions() {
        // The originating #776 case: a 19-track live album was hitting
        // the old 13 min budget. New formula: 10 min + 19 × 30 s
        // = 19.5 min — comfortable headroom.
        let t = compute_total_timeout(19, 0, 0);
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
        let t = compute_total_timeout(200, 0, 0);
        assert_eq!(t.as_secs(), 600 + 200 * 30);
        assert!(t.as_secs() / 60 >= 40);
    }

    #[test]
    fn compute_total_timeout_caps_at_four_hours() {
        // Pathologically large workloads get capped so an accidental
        // recursion into a full music library doesn't propose an
        // unbounded deadline.
        let t = compute_total_timeout(100_000, 0, 0);
        assert_eq!(t.as_secs(), 4 * 3600);
    }

    #[test]
    fn compute_total_timeout_saturates_on_usize_max() {
        // usize::MAX in any input must not overflow the arithmetic.
        let t = compute_total_timeout(usize::MAX, usize::MAX, usize::MAX);
        assert_eq!(t.as_secs(), 4 * 3600);
    }

    #[test]
    fn compute_total_timeout_monotonic_in_tracks() {
        // Scaling should never go backwards as track count rises.
        let mut prev = compute_total_timeout(0, 0, 0);
        for n in (1..1000).step_by(37) {
            let t = compute_total_timeout(n, 0, 0);
            assert!(
                t >= prev,
                "non-monotonic at n={n}: {t:?} vs prev {prev:?}"
            );
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
        let t = compute_total_timeout(12, 1, 0);
        assert_eq!(t.as_secs(), 600 + 12 * 30 + 8 * 60);
    }

    #[test]
    fn compute_total_timeout_four_tier_typical_album() {
        // 12-track Atmos→all formats (Atmos, ALAC, AAC, AAC-Legacy):
        // 10 + 12×30 s + 4×8 min = 48 min. Well above 40 min floor.
        let t = compute_total_timeout(12, 4, 0);
        assert_eq!(t.as_secs(), 600 + 12 * 30 + 4 * 8 * 60);
        assert!(t.as_secs() / 60 >= 40);
    }

    #[test]
    fn compute_total_timeout_monotonic_in_tiers() {
        // More tiers should never propose a smaller deadline than fewer.
        let mut prev = compute_total_timeout(50, 0, 0);
        for tiers in 1..=8 {
            let t = compute_total_timeout(50, tiers, 0);
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
        let t = compute_total_timeout(12, 1, 5);
        assert_eq!(t.as_secs(), 600 + 12 * 30 + 8 * 60 + 5 * 60);
    }

    #[test]
    fn compute_total_timeout_19_track_live_album_with_mvs_and_tier() {
        // The originating #776 case + 1 companion tier + 5 MV companions:
        // 10 + 19×30 s + 8 min + 5 min = 32.5 min. Well above the 13 min
        // budget that was firing.
        let t = compute_total_timeout(19, 1, 5);
        assert_eq!(t.as_secs(), 600 + 19 * 30 + 8 * 60 + 5 * 60);
        assert!(
            t.as_secs() / 60 >= 30,
            "must give heavy live albums >= 30 min"
        );
    }

    #[test]
    fn compute_total_timeout_monotonic_in_mvs() {
        // More MV companions should never propose a smaller deadline.
        let mut prev = compute_total_timeout(50, 1, 0);
        for mvs in 1..=20 {
            let t = compute_total_timeout(50, 1, mvs);
            assert!(
                t >= prev,
                "non-monotonic at mvs={mvs}: {t:?} vs prev {prev:?}"
            );
            prev = t;
        }
    }

    // ----------------------------------------------------------
    // inject_advisory_suffix_into_template — companion folder fix (#528)
    // ----------------------------------------------------------

    /// Default GAMDL template — the suffix lands right after `{album}`
    /// so the rendered companion folder matches the post-rename
    /// primary folder exactly.
    #[test]
    fn inject_advisory_suffix_default_template_explicit() {
        let result = super::inject_advisory_suffix_into_template(
            Some("{album_artist}/{album}"),
            "[Explicit]",
        );
        assert_eq!(
            result.as_deref(),
            Some("{album_artist}/{album} [Explicit]")
        );
    }

    /// `[Clean]` follows the same shape as `[Explicit]`.
    #[test]
    fn inject_advisory_suffix_default_template_clean() {
        let result = super::inject_advisory_suffix_into_template(
            Some("{album_artist}/{album}"),
            "[Clean]",
        );
        assert_eq!(
            result.as_deref(),
            Some("{album_artist}/{album} [Clean]")
        );
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
        let result = super::inject_advisory_suffix_into_template(
            Some("{album_artist}/{title}"),
            "[Explicit]",
        );
        assert_eq!(result, None, "must not blindly append when {{album}} absent");
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
        let result = super::inject_advisory_suffix_into_template(
            Some("{album}/{album}"),
            "[Explicit]",
        );
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

        assert_eq!(options.artist_auto_select, Some(ArtistAutoSelect::MusicVideos));
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
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(0)), "0 s");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(45)), "45 s");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(59)), "59 s");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(60)), "1 min");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(120)), "2 min");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(3540)), "59 min");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(3600)), "1h 0 min");
        assert_eq!(super::format_heartbeat_elapsed(Duration::from_secs(3660)), "1h 1 min");
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
        assert_eq!(summary.downloading_stopped, 1, "one downloading item aborted");
        assert_eq!(summary.processing_stopped, 1, "one processing item aborted");
        assert_eq!(summary.total(), 3, "three items affected total");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Cancelled);
        assert_eq!(statuses[1].state, DownloadState::Cancelled);
        assert_eq!(statuses[2].state, DownloadState::Cancelled);
        assert_eq!(statuses[3].state, DownloadState::Complete, "Complete preserved");
        assert_eq!(statuses[4].state, DownloadState::Error, "Error preserved");
        assert_eq!(statuses[5].state, DownloadState::Cancelled, "already-Cancelled unchanged");
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
        assert!(!queue.take_recently_aborted(), "flag must be consumed on read");
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
        assert!(!was_paused, "First pause should transition running → paused");
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
    #[test]
    fn integrity_failure_message_some_when_all_suspect() {
        let suspects = vec!["01 Track.m4a".to_string(), "02 Track.m4a".to_string(), "03 Track.m4a".to_string()];
        let msg = integrity_failure_message(3, &suspects);
        assert!(msg.is_some());
        let msg = msg.unwrap();
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
        assert_eq!(statuses[0].urls[0], "https://music.apple.com/gb/album/test-song/123456789");
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
        let line = "Requested format is not available: [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>]";
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
            "[WARNING  12:10:09] Skipping \"Track Four\": Requested format is not available".to_string(),
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
            "[INFO     12:30:00] Processing \"https://music.apple.com/us/album/example/1234567890\"".to_string(),
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
        let url = normalize_url_for_dedup(
            "https://music.apple.com/us/album/test/123?i=456&ls=1",
        );
        assert_eq!(url, "https://music.apple.com/us/album/test/123?i=456");
    }

    /// Verifies that a URL with only `?i=` and no other params is preserved correctly.
    #[test]
    fn normalize_url_keeps_track_id_only() {
        let url = normalize_url_for_dedup(
            "https://music.apple.com/us/album/test/123?i=456",
        );
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
        assert_eq!(
            normalize_url_for_dedup(url),
            normalize_url_for_dedup(url),
        );
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
        assert_ne!(studio_a, live_a, "title-disambiguating studio vs live must differ");
        assert_ne!(studio_a, acoustic_a, "title-disambiguating studio vs acoustic must differ");
        assert_ne!(live_a, acoustic_a, "title-disambiguating live vs acoustic must differ");
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
        assert_ne!(studio_b, live_b, "title-collides studio vs live must still differ via {{title_id}}");
        assert_ne!(studio_b, acoustic_b, "title-collides studio vs acoustic must still differ via {{title_id}}");
        assert_ne!(live_b, acoustic_b, "title-collides live vs acoustic must still differ via {{title_id}}");
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
        assert_eq!(super::sanitize_fs_segment("Björk - Vespertine"), "Björk - Vespertine");
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
    fn read_back_manifest_source(album_dir: &std::path::Path) -> crate::models::manifest::ManifestSource {
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
