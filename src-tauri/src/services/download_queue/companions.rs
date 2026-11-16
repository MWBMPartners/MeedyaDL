// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Companion & music-video downloads, progress heartbeat, and companion task spawning.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;

/// A planned companion download tier. Each tier represents one additional
/// GAMDL invocation to download the same content in a different codec.
pub(crate) struct CompanionTier {
    /// Codecs to try in order for this companion tier. The first codec that
    /// succeeds ends the tier (remaining codecs are skipped). If all fail,
    /// the tier is skipped silently.
    pub(crate) codecs_to_try: Vec<SongCodec>,
    /// Whether to apply a codec suffix to this companion's file templates.
    /// `true` means this companion gets a suffixed filename (e.g., `[Lossless]`);
    /// `false` means this companion gets the clean (unsuffixed) filename.
    pub(crate) apply_suffix: bool,
}

/// Epoch-milliseconds helper used by `run_download_with_events` for
/// the primary GAMDL idle watchdog (#508). Local copy rather than
/// importing from `companion_supervisor` to avoid a tight coupling
/// between the two modules — the helper is trivially small.
pub(crate) fn now_epoch_ms_primary() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Reads the union of `audioTraits` for a queue item, returning an
/// empty `Vec` when the item is missing or has no traits captured.
/// Used at companion-spawn time to feed the audioTraits-aware filter
/// in `filter_tiers_by_audio_traits`.
pub(crate) async fn read_audio_traits(queue: &QueueHandle, dl_id: &str) -> Vec<String> {
    let q = queue.lock().await;
    q.items
        .iter()
        .find(|i| i.status.id == dl_id)
        .map(|i| i.status.audio_traits.clone())
        .unwrap_or_default()
}

/// Filters a planned companion-tier list against the union of
/// `audioTraits` reported by the Apple Music catalog API for this
/// download's tracks (#504).
///
/// A tier is kept when at least one of its codecs has either:
///   - no `required_audio_trait()` (the codec is derived from another
///     stream, e.g. binaural — leave the decision to GAMDL), or
///   - a required trait that appears in `available_traits`.
///
/// When `available_traits` is empty (API metadata wasn't reachable) we
/// pass every tier through unchanged so the existing best-effort
/// behaviour is preserved.
pub(crate) fn filter_tiers_by_audio_traits(
    tiers: Vec<CompanionTier>,
    available_traits: &[String],
) -> (Vec<CompanionTier>, Vec<String>) {
    if available_traits.is_empty() {
        // #772 investigation: log the no-data path so the
        // forensic record makes it clear when API metadata
        // failed (vs the pre-filter genuinely dropping a tier).
        log::debug!(
            "filter_tiers_by_audio_traits: audioTraits unavailable from Apple Music API \
             — passing all tiers through unfiltered (no AC3/Atmos pre-filter applied)"
        );
        return (tiers, Vec::new());
    }
    // #772 investigation: trace decision-making per tier so users
    // gathering AC3-false-negative evidence can paste the raw log
    // lines rather than reasoning from the absence of a tier.
    // Always emits at debug level — surfaces in the on-disk
    // activity log when `--log-level=Debug` is configured (#768)
    // and stays out of the user-facing UI by default.
    log::debug!("filter_tiers_by_audio_traits: available_traits = {available_traits:?}");
    let mut skipped = Vec::new();
    let kept: Vec<CompanionTier> = tiers
        .into_iter()
        .filter(|tier| {
            let any_codec_supported =
                tier.codecs_to_try
                    .iter()
                    .any(|c| match c.required_audio_trait() {
                        None => true,
                        Some(needed) => available_traits.iter().any(|t| t == needed),
                    });
            let names: Vec<&str> = tier
                .codecs_to_try
                .iter()
                .map(SongCodec::to_cli_string)
                .collect();
            // Per-tier decision trace (#772). The keep/skip
            // shape names the codec list AND the
            // required-trait → matched-trait outcome so a user
            // pasting these lines can answer the AC3-false-
            // negative question without guessing.
            let trait_details: Vec<String> = tier
                .codecs_to_try
                .iter()
                .map(|c| match c.required_audio_trait() {
                    None => format!("{}=ok(no-trait)", c.to_cli_string()),
                    Some(needed) => {
                        let matched = available_traits.iter().any(|t| t == needed);
                        format!(
                            "{}=needs[{needed}]={}",
                            c.to_cli_string(),
                            if matched { "matched" } else { "missing" }
                        )
                    }
                })
                .collect();
            log::debug!(
                "filter_tiers_by_audio_traits: tier=[{}] decision={} ({})",
                names.join(","),
                if any_codec_supported { "keep" } else { "drop" },
                trait_details.join(", "),
            );
            if !any_codec_supported {
                skipped.push(names.join(","));
            }
            any_codec_supported
        })
        .collect();
    (kept, skipped)
}

/// Lossy AAC fallback chain ordered for the detected GAMDL release (#853).
///
/// - On GAMDL ≥ 3.6, `SongCodec::AacLegacy` serialises as `aac-web`. The
///   web-player path (`is_web == true`) goes through
///   `apple_music_api.get_webplayback()` and requires only MusicKit JWT
///   auth — it works in cookie-only mode. Plain `Aac` on 3.6 still uses
///   the m3u8 path which requires wrapper-v2 for FairPlay decrypt and
///   would fail for users without a running wrapper-v2 daemon. So we
///   try `aac-web` first.
/// - On GAMDL ≤ 3.5.x, both codecs use the m3u8 path and either works
///   the same. We keep the historical order (`Aac` first) to preserve
///   the exact CLI emission of every prior release.
///
/// (#873 fix: the else branch on the original PR #855 version recursed
/// into itself, which would stack-overflow any GAMDL ≤ 3.5.x user. The
/// recursive call has been replaced with the intended historical
/// `[Aac, AacLegacy]` vector here as part of the drift-resolution merge.)
pub(crate) fn lossy_chain_for_runtime() -> Vec<SongCodec> {
    use crate::services::gamdl_capabilities::{supports, GamdlFeature};
    if supports(GamdlFeature::AacWebCodecRename) {
        vec![SongCodec::AacLegacy, SongCodec::Aac]
    } else {
        vec![SongCodec::Aac, SongCodec::AacLegacy]
    }
}

/// Works out which extra copies of an album to download alongside the
/// primary one, and in what order to try them.
///
/// A "companion" is a second (or third, or fourth) copy of the same
/// album downloaded in a different audio format — for example, when the
/// primary download is Dolby Atmos, a companion download can fetch the
/// same album again in plain ALAC (lossless stereo) so the user has a
/// version that plays on ordinary headphones too, without them having to
/// ask for it separately every time.
///
/// The result is an ordered list of tiers (`CompanionTier`). Tiers run
/// one after another — never at the same time, since two GAMDL
/// processes writing into the same album folder at once would step on
/// each other's files. Within a single tier, `codecs_to_try` is also an
/// ordered list: the first codec that actually succeeds wins and the
/// rest of that tier's list is skipped; if every codec in the tier
/// fails, that whole tier is quietly skipped rather than failing the
/// download.
///
/// **Filename tagging rule:** a companion whose codec is not the only
/// other format the user will end up with gets a tag in its filename —
/// like `[Lossless]` — so the two copies sitting in the same folder can
/// be told apart at a glance. The exception is whichever companion ends
/// up being the *most* universally compatible one for the chosen mode
/// (for example, plain AAC when both Atmos and lossless copies also
/// exist): that one is treated as the "default" copy a music player
/// would reach for, so it keeps a clean, untagged filename instead —
/// see each `CompanionTier`'s `apply_suffix` field, and
/// `needs_primary_suffix` (in `options.rs`) for the matching rule
/// applied to the primary download itself.
pub(crate) fn plan_companions(
    mode: &CompanionMode,
    primary_codec: &str,
    custom_codecs: &[SongCodec],
) -> Vec<CompanionTier> {
    match mode {
        // No companions in any scenario
        CompanionMode::Disabled => vec![],

        // Atmos → ALAC companion (clean filename); nothing for other codecs
        CompanionMode::AtmosToLossless => {
            if primary_codec == "atmos" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Alac],
                    apply_suffix: false, // ALAC companion gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Maximum coverage:
        //   Atmos → ALAC [Lossless] + AAC (clean)
        //   ALAC → AAC (clean)
        CompanionMode::AtmosToLosslessAndLossy => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix (AAC exists too)
                    },
                    CompanionTier {
                        codecs_to_try: lossy_chain_for_runtime(),
                        apply_suffix: false, // Lossy AAC gets clean filename
                    },
                ]
            } else if primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: lossy_chain_for_runtime(),
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Any specialist → lossy companion (clean filename)
        CompanionMode::SpecialistToLossy => {
            if primary_codec == "atmos" || primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: lossy_chain_for_runtime(),
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Atmos → all formats: AC3 [Dolby Digital] + ALAC [Lossless] + AAC (clean)
        // 4 files per track total (Atmos + 3 companions)
        CompanionMode::AtmosToAllFormats => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Ac3],
                        apply_suffix: true, // AC3 gets [Dolby Digital] suffix
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix
                    },
                    CompanionTier {
                        codecs_to_try: lossy_chain_for_runtime(),
                        apply_suffix: false, // AAC gets clean filename
                    },
                ]
            } else {
                vec![]
            }
        }

        // Custom: each user-selected codec becomes its own tier.
        // The last tier gets the clean filename; earlier tiers get suffixes.
        // All user-selected codecs are included — even if one matches the
        // primary setting. With native priority the actual codec GAMDL picks
        // may differ from the requested primary, so the user's explicit
        // selections must be respected.
        CompanionMode::Custom => {
            let filtered: Vec<&SongCodec> = custom_codecs.iter().collect();

            if filtered.is_empty() {
                return vec![];
            }

            let last_idx = filtered.len() - 1;
            filtered
                .into_iter()
                .enumerate()
                .map(|(i, codec)| CompanionTier {
                    codecs_to_try: vec![codec.clone()],
                    // Last companion gets clean filename; others get suffix
                    apply_suffix: i != last_idx,
                })
                .collect()
        }
    }
}

/// Build the codec-registry-ID tier matrix recorded in
/// `ManifestSource.companion_tiers` (#766, Phase 2 of #717/5b).
///
/// Tier 0 is the primary download's codec (one element — fallback chains
/// are not surfaced here since the manifest only records the codec the
/// download actually completed with). Tiers 1..N mirror the
/// `plan_companions()` output, each tier's `codecs_to_try` mapped to
/// canonical registry IDs via `song_codec_to_registry_id`.
///
/// Returns `None` when `primary_codec_cli` cannot be parsed back to a
/// `SongCodec` (defensive — should never happen in practice since the
/// primary codec string flows from a `SongCodec::to_cli_string()` call).
/// `None` means "don't write companion_tiers" and the planner falls
/// back to its track-number-only diff for this manifest.
pub(crate) fn build_manifest_companion_tiers(
    settings: &AppSettings,
    primary_codec_cli: &str,
) -> Option<Vec<Vec<String>>> {
    let primary = SongCodec::from_cli_string(primary_codec_cli)?;
    let mut tiers: Vec<Vec<String>> = vec![vec![song_codec_to_registry_id(&primary).to_string()]];

    for tier in plan_companions(
        &settings.companion_mode,
        primary_codec_cli,
        &settings.custom_companion_codecs,
    ) {
        tiers.push(
            tier.codecs_to_try
                .iter()
                .map(|c| song_codec_to_registry_id(c).to_string())
                .collect(),
        );
    }

    Some(tiers)
}

/// The suffix [`apply_codec_suffix`] would add for these options: the
/// suffix of `song_codec`, else of the first codec in
/// `song_codec_priority` (companion and fallback runs set `song_codec` to
/// `None` and give one codec there). `None` when there is no codec or it
/// has no suffix entry.
pub(crate) fn codec_suffix_for_options(options: &GamdlOptions) -> Option<&'static str> {
    if let Some(codec) = &options.song_codec {
        return codec_suffix(codec);
    }
    let first = options.song_codec_priority.as_deref()?.split(',').next()?;
    codec_suffix(&SongCodec::from_cli_string(first)?)
}

/// Appends a codec-specific suffix to all file naming templates in a
/// `GamdlOptions` struct.
///
/// When companion downloads are enabled, multiple codec versions of the
/// same track land in the same album directory. To prevent filename
/// collisions and clearly identify the format, this function appends the
/// codec's suffix (e.g., ` [Lossless]` or ` [Dolby Atmos]`) to the
/// filename portion of every file template.
///
/// The most universally compatible companion uses the original (unsuffixed)
/// template, so it gets a "clean" filename (e.g., `01 Song Title.m4a`)
/// while specialist formats get tagged filenames (e.g.,
/// `01 Song Title [Lossless].m4a` or `01 Song Title [Dolby Atmos].m4a`).
///
/// This modifies the following templates:
/// - `single_disc_file_template` (most common: `{track:02d} {title}`)
/// - `multi_disc_file_template` (`{disc}-{track:02d} {title}`)
/// - `no_album_file_template` (`{title}`)
/// - `playlist_file_template` (`Playlists/{playlist_artist}/{playlist_title}`)
///
/// Returns `true` if a suffix was applied, `false` if the codec has no suffix.
pub(crate) fn apply_codec_suffix(options: &mut GamdlOptions) -> bool {
    let Some(suffix) = codec_suffix_for_options(options) else {
        return false;
    };

    // For each file template, append the suffix to the existing value.
    // If the template is None (not set), use the GAMDL default with the suffix.
    if let Some(ref template) = options.single_disc_file_template {
        options.single_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.single_disc_file_template = Some(format!("{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.multi_disc_file_template {
        options.multi_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.multi_disc_file_template =
            Some(format!("{{disc}}-{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.no_album_file_template {
        options.no_album_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.no_album_file_template = Some(format!("{{title}} {suffix}"));
    }

    if let Some(ref template) = options.playlist_file_template {
        options.playlist_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.playlist_file_template = Some(format!(
            "Playlists/{{playlist_artist}}/{{playlist_title}} {suffix}"
        ));
    }

    true
}

/// Removes exactly the format suffixes MeedyaDL itself appended to the
/// file templates (`recorded`, newest last; kept on the queue item as
/// `QueueItem::added_template_suffixes`), each once, from the very end of
/// each template, where [`apply_codec_suffix`] put it. Returns whether
/// anything was removed.
///
/// Why this is needed: the format fallback (`DownloadQueue::try_fallback`)
/// may add the new format's suffix to the item's saved options, and later
/// runs, and the extra-format copies, start from those. Without this the
/// "plain" copy would carry the primary's suffix and get exactly the
/// primary's file name, and suffixes would pile up over several fallbacks.
///
/// Only what MeedyaDL recorded is removed, so a suffix the person typed
/// into their own template (`{track:02d} {title} [Lossless]`) is never
/// touched. Nothing older needs guessing: a queue saved to disk keeps only
/// the original request and is merged afresh on restore, so a restored
/// item never carries an added suffix.
pub(crate) fn strip_recorded_codec_suffixes(options: &mut GamdlOptions, recorded: &[&str]) -> bool {
    let mut changed = false;
    for template in [
        &mut options.single_disc_file_template,
        &mut options.multi_disc_file_template,
        &mut options.no_album_file_template,
        &mut options.playlist_file_template,
    ]
    .into_iter()
    .flatten()
    {
        for suffix in recorded.iter().rev().filter(|s| !s.is_empty()) {
            let tail = format!(" {suffix}");
            if template.ends_with(&tail) {
                let keep = template.len() - tail.len();
                template.truncate(keep);
                changed = true;
            }
        }
    }
    changed
}

/// Why a companion attempt is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompanionSkip {
    /// The main download is already in this format (a second copy would
    /// be pointless, and would have the main download's names).
    SameFormat,
    /// Its files would have the same names as the main download's.
    SameNames,
}

/// How one companion attempt goes ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttemptPlan {
    /// Add the format suffix to its file names.
    pub apply_suffix: bool,
    /// Some of the primary's own tracks are already in this very format
    /// (an album where native format priority mixed formats). The attempt
    /// then asks only for the OTHER tracks, as per-track song links
    /// ([`attempt_links`]), so a track already in this format is never
    /// requested at all (an advisory-renamed primary such as
    /// `02 T [Explicit] [Lossless]` would not stop GAMDL saving
    /// `02 T [Lossless]` beside it). Nothing is ever deleted. Overwrite is
    /// still forced off for that run, for safety.
    pub only_other_tracks: bool,
}

/// Which links one companion attempt asks GAMDL for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttemptLinks {
    /// The item's own links, as for the primary run.
    All,
    /// Only these per-track song links. `left_out` tracks get no copy:
    /// no song ID for them, or their disc/track number could not be read.
    /// `details_available` is false when there were no album song details
    /// at all (no developer key, for example).
    Only {
        links: Vec<String>,
        /// Songs with no song ID.
        unmapped: usize,
        /// Songs whose disc/track number could not be read.
        unreadable: usize,
        /// Songs not requested because their copy in this format is
        /// already in the folder (possibly with an advisory label).
        already_there: usize,
        details_available: bool,
    },
    /// Not run, with the plain reason for the activity log.
    Skip(String),
}

/// Works out the links for one attempt: the item's links, or per-track
/// song links — on a mixed album, for the tracks not already in `codec`;
/// and whenever some of this download's tracks already have a copy in this
/// format in the folder (`existing_copies`, e.g. a labelled copy from an
/// earlier run), for the others only.
///
/// A mixed album that is not a single album link (a playlist, an artist,
/// several links) cannot be split by track, so that format's copies are
/// skipped with a plain reason. Existing copies alone never cause a skip:
/// without a single album link or song details the normal request runs,
/// and GAMDL's own name check applies.
// Every argument is one fact about the album the decision needs; a
// struct would only move the list to the one caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn attempt_links(
    codec: &SongCodec,
    only_other_tracks: bool,
    item_urls: &[String],
    primary_tracks: &[(SongCodec, u32, u32)],
    album_tracks: &[(u32, u32, String)],
    unplaceable: &[SongCodec],
    existing_copies: &[(u32, u32)],
    labelled_copies: bool,
    covered_earlier: &[(u32, u32)],
) -> AttemptLinks {
    // This download's tracks that already have their copy.
    let mut already_there: Vec<(u32, u32)> = Vec::new();
    for (c, d, t) in primary_tracks {
        if c != codec && existing_copies.contains(&(*d, *t)) && !already_there.contains(&(*d, *t)) {
            already_there.push((*d, *t));
        }
    }
    if !only_other_tracks && already_there.is_empty() && covered_earlier.is_empty() {
        return AttemptLinks::All;
    }
    let name = plain_codec_name(codec);
    let parsed: Vec<_> = item_urls
        .iter()
        .filter_map(|u| apple_music_api::parse_apple_music_url(u))
        .collect();
    let album = match parsed.as_slice() {
        [p] if p.content_type == "album"
            && p.song_id.is_none()
            && item_urls.len() == 1
            && (only_other_tracks || !album_tracks.is_empty()) =>
        {
            p
        }
        _ if !only_other_tracks => return AttemptLinks::All,
        _ => {
            let what = if parsed.iter().any(|p| p.content_type == "playlist") {
                "this playlist"
            } else {
                "this download"
            };
            return AttemptLinks::Skip(format!(
                "Extra copies in {name} were not made for {what}, because some of its songs are already in that format."
            ));
        }
    };
    let (links, unmapped) = super::lyrics_pass::other_track_links(
        codec,
        primary_tracks,
        album_tracks,
        &album.storefront,
        &album.album_id,
        &already_there
            .iter()
            .chain(covered_earlier.iter())
            .copied()
            .collect::<Vec<_>>(),
    );
    // Songs whose numbers could not be read cannot be requested on their
    // own; those not already in this format are left out too.
    let unplaceable_elsewhere = unplaceable.iter().filter(|c| *c != codec).count();
    // Not a mixed album: if some songs cannot be asked for on their own,
    // ask for the whole album instead — but only when GAMDL's own name
    // check will recognise every copy already there (none carries an
    // advisory label) and no song is covered by an earlier format of this
    // tier. Otherwise keep the per-song request and report the songs left
    // out, rather than download duplicates.
    if !only_other_tracks
        && (!unmapped.is_empty() || unplaceable_elsewhere > 0)
        && !labelled_copies
        && covered_earlier.is_empty()
    {
        return AttemptLinks::All;
    }
    AttemptLinks::Only {
        links,
        unmapped: unmapped.len(),
        unreadable: unplaceable_elsewhere,
        already_there: already_there.len(),
        details_available: !album_tracks.is_empty(),
    }
}

/// The plain activity-log lines for a mixed-album attempt that asks for
/// `links` songs: one line for songs with no song ID (`unmapped`), one for
/// songs whose track numbers could not be read (`unreadable`), and always
/// one when nothing at all is requested.
pub(crate) fn mixed_album_lines(
    codec: &SongCodec,
    links: usize,
    unmapped: usize,
    unreadable: usize,
    details_available: bool,
) -> Vec<String> {
    let name = plain_codec_name(codec);
    let mut lines = Vec::new();
    if unmapped > 0 {
        let why = if details_available {
            "their Apple Music details could not be matched"
        } else {
            "this album's song details were not available"
        };
        lines.push(format!(
            "{unmapped} song(s) were left without an extra copy in {name} because {why}."
        ));
    }
    if unreadable > 0 {
        lines.push(format!(
            "{unreadable} song(s) were left without an extra copy in {name} because their track numbers could not be read."
        ));
    }
    if links == 0 {
        lines.push(format!(
            "No extra copy in {name} was made for this album: none of its songs could be asked for on their own."
        ));
    }
    lines
}

/// How the primary run named its files, for the companions' guard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrimaryNaming {
    /// The format suffix MeedyaDL itself appended to the primary run's
    /// file templates (its own single-format suffix, or the one a format
    /// fallback recorded) — never a label the person typed.
    pub appended_suffix: Option<&'static str>,
    /// The primary run's actual single-disc file template.
    pub single_disc: Option<String>,
    /// The primary run's actual multi-disc file template.
    pub multi_disc: Option<String>,
}

impl PrimaryNaming {
    /// Taken from the primary run's options, with the suffix MeedyaDL
    /// appended to them.
    pub(crate) fn of(options: &GamdlOptions, appended_suffix: Option<&'static str>) -> Self {
        Self {
            appended_suffix,
            single_disc: options.single_disc_file_template.clone(),
            multi_disc: options.multi_disc_file_template.clone(),
        }
    }
}

/// Whether an attempt's file names would be the primary's: its built
/// single-disc or multi-disc file template equals the primary's (an unset
/// template counts as GAMDL's default, which is what it means).
fn templates_clash(attempt: &GamdlOptions, primary: &PrimaryNaming) -> bool {
    // GAMDL's own default file templates, used when a template is unset.
    // They must match GAMDL's defaults, or two unset templates would be
    // compared wrongly.
    const SINGLE_DEFAULT: &str = "{track:02d} {title}";
    const MULTI_DEFAULT: &str = "{disc}-{track:02d} {title}";
    let same = |a: Option<&str>, b: Option<&str>, default: &str| {
        a.unwrap_or(default) == b.unwrap_or(default)
    };
    same(
        attempt.single_disc_file_template.as_deref(),
        primary.single_disc.as_deref(),
        SINGLE_DEFAULT,
    ) || same(
        attempt.multi_disc_file_template.as_deref(),
        primary.multi_disc.as_deref(),
        MULTI_DEFAULT,
    )
}

/// What one companion attempt may do so it never replaces the primary's
/// files: go ahead (possibly with a suffix the tier did not plan, to tell
/// it apart, or keeping existing files), or skip it.
///
/// - `attempt`: the attempt's options with its format set and no suffix
///   added yet.
/// - `primary`: how the primary run named its files. Names are compared
///   as fully built templates, so a label the person typed into their own
///   template counts only where it really makes two names equal.
/// - `primary_codecs`: the formats detected in the primary's files (from
///   the gate; empty when unknown). All of them this format: skipped —
///   a second copy is pointless, and on the native-priority path its
///   names are the primary's after enrichment renamed it by the detected
///   format. Some of them (an album where native priority mixed formats):
///   goes ahead with `only_other_tracks`.
///
/// A plain attempt beside a plain-named primary that enrichment did not
/// rename is covered by [`force_suffixes_for_unrenamed_primary`].
pub(crate) fn companion_attempt_naming(
    codec: &SongCodec,
    apply_suffix: bool,
    attempt: &GamdlOptions,
    primary: &PrimaryNaming,
    primary_codecs: &[SongCodec],
) -> Result<AttemptPlan, CompanionSkip> {
    if !primary_codecs.is_empty() && primary_codecs.iter().all(|c| c == codec) {
        return Err(CompanionSkip::SameFormat);
    }
    let mixed = primary_codecs.contains(codec);
    let built = |apply: bool| {
        let mut o = attempt.clone();
        if apply {
            apply_codec_suffix(&mut o);
        }
        o
    };
    let plan = |apply_suffix, only_other_tracks| AttemptPlan {
        apply_suffix,
        only_other_tracks,
    };
    if !templates_clash(&built(apply_suffix), primary) {
        return Ok(plan(apply_suffix, mixed));
    }
    // A suffix of its own tells it apart.
    let has_own = codec_suffix(codec).is_some_and(|s| !s.is_empty());
    if !apply_suffix && has_own && !templates_clash(&built(true), primary) {
        return Ok(plan(true, mixed));
    }
    if mixed {
        // The clashing names belong to the primary's own tracks in this
        // format; keep them, copy the others.
        return Ok(plan(apply_suffix, true));
    }
    Err(CompanionSkip::SameNames)
}

/// Whether a GAMDL output line says an item was skipped because a file
/// of that name was already there (companions always run with overwrite
/// off). GAMDL 4.x: `↷ Title: skipped → path` with no reason in brackets
/// (a skip for another reason carries one). GAMDL 3.x: a "Skipping" line
/// that says the file already exists (wording to be confirmed on a real
/// machine).
pub(crate) fn is_already_there_skip_line(line: &str) -> bool {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix('↷') {
        return rest.contains(": skipped →") && !rest.contains(": skipped (");
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.contains("skipping") && lower.contains("already exist")
}

/// Whether a run that produced no new files is still done: some copies
/// were already there, and the copies already there plus the songs GAMDL
/// said are not offered in this format cover EVERY song asked for. Then a
/// different format is not tried for the whole album (it would add a
/// second copy of every song). Otherwise the next format in the tier is
/// tried. `expected` 0 (unknown) never counts as done.
pub(crate) fn tier_done_without_new_files(
    already_there: usize,
    not_offered: usize,
    expected: usize,
) -> bool {
    expected > 0 && already_there > 0 && already_there + not_offered >= expected
}

/// How many of this download's songs an album-wide attempt in `codec`
/// asks for: those with readable numbers not already in that format
/// (counted once each), plus those with unreadable numbers in another
/// format.
pub(crate) fn songs_wanted_in(
    codec: &SongCodec,
    primary_tracks: &[(SongCodec, u32, u32)],
    unplaceable: &[SongCodec],
) -> usize {
    let mut wanted: Vec<(u32, u32)> = Vec::new();
    let already: Vec<(u32, u32)> = primary_tracks
        .iter()
        .filter(|(c, _, _)| c == codec)
        .map(|(_, d, t)| (*d, *t))
        .collect();
    for (_, d, t) in primary_tracks {
        if !already.contains(&(*d, *t)) && !wanted.contains(&(*d, *t)) {
            wanted.push((*d, *t));
        }
    }
    wanted.len() + unplaceable.iter().filter(|c| *c != codec).count()
}

/// The `.m4a` files an extra-format tier added: in `after` but not in
/// `before` (the music and video files in the folder before and after the
/// tier's GAMDL run). Only `.m4a` files count; videos are never format-tagged.
/// With an empty `before`, every `.m4a` in `after` is new. Used so a tier's
/// format tags reach only the files that tier added.
pub(crate) fn tier_new_m4a(
    before: &std::collections::BTreeSet<std::path::PathBuf>,
    after: &std::collections::BTreeSet<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    after
        .iter()
        .filter(|p| !before.contains(*p))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("m4a"))
        })
        .cloned()
        .collect()
}

/// Writes an extra-format tier's format tags to the files that tier added
/// (see [`tier_new_m4a`]), each under the app-wide write lock. With
/// `within`, files outside that folder are left out. Returns how many files
/// were tagged. Failures are only logged.
pub(crate) async fn tag_tier_additions(
    before: &std::collections::BTreeSet<std::path::PathBuf>,
    after: &std::collections::BTreeSet<std::path::PathBuf>,
    within: Option<&std::path::Path>,
    codec: &SongCodec,
    dl_id: &str,
) -> usize {
    let mut tagged = 0usize;
    for new_file in tier_new_m4a(before, after) {
        if within.is_some_and(|dir| !new_file.starts_with(dir)) {
            continue;
        }
        let _guard = crate::utils::file_locks::global().lock(&new_file).await;
        match super::metadata_tag_service::apply_codec_metadata_tags(
            &new_file.to_string_lossy(),
            codec,
        ) {
            Ok(count) => tagged += count,
            Err(e) => {
                log::debug!("Companion metadata tagging failed for {dl_id}: {e}");
            }
        }
    }
    tagged
}

/// The plain line for copies not made because a file with the same name
/// was already there, or `None` when there were none.
pub(crate) fn already_there_line(codec: &SongCodec, count: usize) -> Option<String> {
    (count > 0).then(|| {
        format!(
            "{count} extra copy(ies) in {} were not made because a file with the same name was already there.",
            plain_codec_name(codec)
        )
    })
}

/// A format's name for the activity log, without the "(Experimental)"
/// note the settings list shows.
pub(crate) fn plain_codec_name(codec: &SongCodec) -> &str {
    codec.display_name().trim_end_matches(" (Experimental)")
}

// ============================================================
// Music video companion downloads
// ============================================================

/// Downloads music videos as companions via the Apple Music API (Step 6).
///
/// Called inside the enrichment pipeline's async block (after Step 5: ReplayGain).
/// Queries the Apple Music API to find music videos related to the downloaded
/// tracks, then spawns a GAMDL invocation for each available music video.
///
/// This is a fire-and-forget operation — failures are logged but do not
/// affect the primary download status or queue progression. Gracefully
/// skips if MusicKit credentials are not configured — Step 6b (MusicBrainz)
/// provides a credential-free fallback path.
///
/// # Requirements
/// - `settings.music_video_companion` must be `true`
/// - MusicKit developer token required (user credentials or embedded build token)
/// - `album_metadata` provides song IDs (reused from enrichment Step 1)
pub(crate) async fn spawn_music_video_companion_inner(
    app: &tauri::AppHandle,
    dl_id: &str,
    urls: &[String],
    album_metadata: Option<&super::apple_music_api::AlbumMetadata>,
    settings: &crate::models::settings::AppSettings,
    shutdown: &ShutdownSignal,
    parent_album_path: Option<&str>,
) {
    // Early exit if the original URL is already a music-video URL
    // (no self-referencing companion).
    if let Some(first_url) = urls.first() {
        if let Some(parsed) = super::apple_music_api::parse_apple_music_url(first_url) {
            if parsed.content_type == "music-video" {
                log::debug!(
                    "Music video companion skipped for {dl_id}: URL is already a music-video"
                );
                return;
            }
        }
    }

    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match super::apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        _ => None,
    };

    // Extract storefront from the original URL
    let storefront = match urls
        .first()
        .and_then(|u| super::apple_music_api::parse_apple_music_url(u))
    {
        Some(parsed) => parsed.storefront,
        None => {
            log::debug!(
                "Music video companion skipped for {dl_id}: could not parse URL storefront"
            );
            return;
        }
    };

    // Extract song IDs from the enrichment Step 1 metadata (already fetched once
    // and passed here to avoid duplicate API calls). These IDs are used to query
    // which songs have available music videos on Apple Music.
    let song_ids: Vec<String> = match album_metadata {
        Some(meta) => meta.tracks.iter().map(|t| t.song_id.clone()).collect(),
        None => {
            log::debug!("Music video companion skipped for {dl_id}: no album metadata available");
            return;
        }
    };

    if song_ids.is_empty() {
        log::debug!("Music video companion skipped for {dl_id}: no tracks in metadata");
        return;
    }

    emit_download_log(
        app,
        dl_id,
        &format!("Looking up music videos for {} track(s)...", song_ids.len()),
    );

    // Resolve MusicKit token for API call (premium feature resolver with web player fallback).
    let (jwt, token_source) = match super::apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            // No token at all — usually means the user enabled MV
            // companion in Settings without providing MusicKit
            // credentials. Surface the actionable next step instead
            // of debug-logging silently (#942).
            emit_download_log(
                app,
                dl_id,
                "Music video lookup skipped — MusicKit credentials required (Settings > Codec & Resolution > Video Quality)",
            );
            return;
        }
        Err(e) => {
            // Token resolution erred (e.g., invalid private key PEM,
            // expired embedded token). Surface the full error so the
            // user can route to the right setting (#942).
            emit_download_warn(
                app,
                dl_id,
                &format!("Music video lookup skipped — MusicKit token resolution failed: {e}"),
            );
            return;
        }
    };

    crate::utils::activity_log::emit_verbose_download_log(
        app,
        dl_id,
        &format!("Music video companion: using MusicKit token from {token_source}"),
    );

    // Fetch music video relationships
    let relations =
        match super::apple_music_api::fetch_music_video_relations(&jwt, &storefront, &song_ids)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // Relation lookup failed (network, API error, etc.) —
                // surface the actual error rather than the generic
                // "lookup failed" (#942).
                emit_download_warn(
                    app,
                    dl_id,
                    &format!("Music video relation lookup failed: {e}"),
                );
                return;
            }
        };

    // Deduplicate by music video ID. An album may have the same music video
    // linked from multiple songs (e.g., a lead single's video referenced by
    // both the single and album versions). HashSet::insert() returns true if
    // the ID is new, false if already seen — we only keep the first occurrence.
    let mut seen_ids = std::collections::HashSet::new();
    let unique_relations: Vec<_> = relations
        .into_iter()
        .filter(|r| seen_ids.insert(r.music_video_id.clone()))
        .collect();

    if unique_relations.is_empty() {
        emit_download_log(app, dl_id, "No music videos found for this album");
        return;
    }

    emit_download_log(
        app,
        dl_id,
        &format!(
            "Found {} music video(s) — downloading as companions",
            unique_relations.len()
        ),
    );

    // Snapshot the video-file set under the user's output root BEFORE any
    // MV download runs, so the post-loop summary message can report the
    // actual number of files produced rather than blindly claiming
    // "{N} video(s)" when GAMDL silently failed every download (#774-class
    // false-positive that mirrors Phase 3.5h for audio companions).
    //
    // Empty `output_path` ⇒ user is on the default (per-OS resolution
    // happens inside GAMDL) — skip the snapshot in that case and fall
    // back to the count-of-attempts message. Better than emitting a
    // misleading "0 of N downloaded" when we just don't have visibility.
    let video_count_tracking = (!settings.output_path.is_empty()).then(|| {
        (
            settings.output_path.clone(),
            snapshot_video_files(&settings.output_path).len(),
        )
    });

    // Download each music video using the shared helper
    for relation in &unique_relations {
        if shutdown.is_triggered() {
            log::info!("Music video companions stopping early for {dl_id} (app shutting down)");
            // Even on early-exit, give the user an accurate summary if
            // we know what landed on disk. Without this, they'd see the
            // last "Downloading music video: X" line without ever
            // learning whether anything succeeded.
            if let Some((root, pre_count)) = video_count_tracking {
                let post_count = snapshot_video_files(&root).len();
                let new_files = post_count.saturating_sub(pre_count);
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Music video companion downloads stopped — {new_files} of {} attempted before shutdown",
                        unique_relations.len()
                    ),
                );
            }
            return;
        }

        let mv_url =
            super::apple_music_api::build_music_video_url(&storefront, &relation.music_video_id);
        let mv_name = relation.name.as_deref().unwrap_or("unknown");

        emit_download_log(app, dl_id, &format!("Downloading music video: {mv_name}"));

        download_music_video_by_url(app, dl_id, &mv_url, mv_name, settings, parent_album_path)
            .await;
    }

    // Honest completion summary (#774-class false-positive fix). When
    // we have a tracked output root, diff the snapshot against the
    // post-loop video-file set so the reported count reflects what
    // actually downloaded — not the number of attempts. When we don't
    // have a tracked root (empty output_path), fall back to the
    // attempts-count phrasing so we never claim a number we can't
    // verify.
    let summary = match video_count_tracking {
        Some((root, pre_count)) => {
            let post_count = snapshot_video_files(&root).len();
            let new_files = post_count.saturating_sub(pre_count);
            let total_attempted = unique_relations.len();
            if new_files == 0 {
                format!(
                    "Music video companion downloads finished — 0 of {total_attempted} produced any files (no compatible streams or all attempts failed)"
                )
            } else if new_files < total_attempted {
                format!(
                    "Music video companion downloads complete — {new_files} of {total_attempted} downloaded ({} unavailable or failed)",
                    total_attempted - new_files
                )
            } else {
                format!("Music video companion downloads complete ({new_files} video(s))")
            }
        }
        None => format!(
            "Music video companion downloads attempted ({} video(s) — file count not verified)",
            unique_relations.len()
        ),
    };
    emit_download_log(app, dl_id, &summary);
}

/// Downloads a single music video given its Apple Music URL.
///
/// Emit the `\r`-split segments of a single raw output line to the
/// activity log, matching the main GAMDL reader's coalescing rules.
///
/// yt-dlp and N_m3u8DL-RE (used for music videos / HLS) overwrite
/// terminal progress in place with `\r` rather than `\n`, which means
/// `AsyncBufReadExt::lines()` returns a single line containing many
/// `[download]` progress segments. Emitting that line as-is produces a
/// 100KB+ unreadable blob in the activity log.
///
/// This helper splits on `\r`, strips ANSI escapes, and emits either:
/// - the **last non-empty segment only** in normal mode (keeps activity
///   log scrollable — earlier segments would be overwritten in a real
///   terminal anyway), or
/// - **every** non-empty segment in verbose mode, so users get the full
///   speed / ETA / percentage trail when debugging.
///
/// Shared by companion audio downloads and music-video companion
/// downloads so their progress renders consistently with the primary
/// GAMDL reader.
/// Parses a single companion-GAMDL output line and, if it is a
/// `TrackInfo` event, overwrites the per-item `processing_label`
/// with a track-aware caption (#799). No-op on lines that are not
/// `TrackInfo` events.
///
/// Caption format mirrors what users already see for the primary
/// download caption, just prefixed with the companion tier + codec:
///
/// ```text
/// Companion (tier 2 — atmos): "We Are the Champions (Ding a Dang Dong)" — 8 of 8
/// ```
///
/// When the GAMDL line carries `title` only (no track counter) the
/// counter clause is omitted. When neither is present the line is
/// ignored — we keep the previous label rather than risk replacing
/// a richer one with an empty placeholder.
///
/// Called from the LineEmitter closure inside
/// `spawn_companion_downloads` for every line. Cheap single-line
/// parse; no allocation when the event isn't `TrackInfo`.
pub(crate) async fn update_companion_label_from_line(
    app: &tauri::AppHandle,
    queue: &QueueHandle,
    dl_id: &str,
    tier_idx: usize,
    codec_name: &str,
    raw_line: &str,
) {
    // `parse_gamdl_output` already strips ANSI codes and handles `\r`
    // overwrites; we deliberately don't pre-clean here so the
    // pre-strip shape matches what the parser was tuned for.
    let event = crate::utils::process::parse_gamdl_output(raw_line);
    let crate::utils::process::GamdlOutputEvent::TrackInfo {
        track_number,
        track_total,
        title,
        ..
    } = event
    else {
        return;
    };

    // Compose the caption. Both `track_number` + `track_total` are
    // required for the counter clause — partial track info would
    // mislead more than it informs. `title` is always `String`
    // (parser substitutes an empty string when missing) so guard
    // against the empty case explicitly.
    let title_trimmed = title.trim();
    if title_trimmed.is_empty() {
        return;
    }
    let caption = match (track_number, track_total) {
        (Some(c), Some(t)) => {
            format!("Companion (tier {tier_idx} — {codec_name}): \"{title_trimmed}\" — {c} of {t}")
        }
        _ => format!("Companion (tier {tier_idx} — {codec_name}): \"{title_trimmed}\""),
    };

    // #808 fix: use `set_label_only` rather than
    // `set_stage_with_label(…, Finalising, …)` here. The per-track
    // caption update fires many times during a single companion
    // download (one per track in a 21-track album = 21 calls), and
    // every `set_stage_with_label` call resets the bar to the
    // stage's weight — `Finalising` is 0.95, which pegged the bar
    // at 95% for the entire companion run regardless of actual
    // within-companion progress.
    set_label_only(app, queue, dl_id, &caption);

    // #836: drive the per-item bar between 95% and ~99% so the
    // companion phase no longer appears frozen for 30+ minutes on
    // big multi-tier runs. The pre-fix design assumed the
    // companion subprocess's `[download] X%` events would advance
    // the bar (#808 comment), but in practice each companion track
    // is its own GAMDL run that resets `[download]` to 0% on each
    // track start. The visible bar therefore oscillated within a
    // single track and never advanced across tracks — users saw
    // "95%" for the entire companion phase.
    //
    // Strategy: advance the bar by `(track_number / track_total) *
    // 0.04` within the 0.95..0.99 reserve, so each track-start
    // tick produces a small but visible forward movement. The
    // reserve's last 1% (0.99..1.00) is left for the post-companion
    // advisory pass and `set_complete`.
    //
    // This is per-TIER progress — multi-tier runs (e.g. Custom
    // mode with [ac3, alac, atmos]) will replay 0.95 → 0.99 once
    // per tier rather than spreading the 4% slice across all
    // tiers. Sufficient to remove the "stuck" appearance; precise
    // multi-tier mapping is a follow-up if users want it.
    if let (Some(n), Some(t)) = (track_number, track_total) {
        if t > 0 {
            // Cap at 0.99 so the bar can't accidentally hit 1.0
            // before the advisory pass / set_complete fires.
            let fraction = (n as f32 / t as f32).clamp(0.0, 1.0);
            let bar_value = 0.95 + fraction * 0.04;
            let mut q = queue.lock().await;
            q.set_processing_progress(dl_id, bar_value);
        }
    }
}

/// Sends one raw line from a companion GAMDL process to the app.
///
/// A progress bar redraws itself by writing `\r` (carriage return) and
/// then new text on the same line, so one "line" can hold many updates.
/// This splits the line on `\r` and, for every non-empty piece:
///
/// 1. strips colour codes, parses it and sends it as a `gamdl-output`
///    event, so the speed, ETA and percentage on the progress bar stay live;
/// 2. shows it in the activity log, but only the LAST piece, unless verbose
///    logging is on (then every piece is shown). The log file on disk gets
///    every piece either way; `emit_subprocess_line` does that.
///
/// `stream` is `"stdout"` or `"stderr"`. Returns the cleaned text of the
/// last non-empty piece, or `None` if the line was blank.
///
/// See:
/// https://doc.rust-lang.org/std/primitive.str.html#method.split
pub(crate) async fn emit_companion_stream_line(
    app: &tauri::AppHandle,
    dl_id: &str,
    stream: &'static str,
    raw_line: &str,
) -> Option<String> {
    let segments: Vec<&str> = raw_line.split('\r').collect();
    let verbose = crate::utils::activity_log::is_verbose_logging();
    let last_segment_idx = segments.iter().rposition(|s| !s.trim().is_empty());
    let mut last_clean = None;

    for (idx, segment) in segments.iter().enumerate() {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let clean_line = crate::utils::process::strip_ansi_codes(segment);
        last_clean = Some(clean_line.clone());

        // Drive the progress bar off EVERY segment so the speed/ETA/%
        // stay live even when we suppress earlier segments from the log.
        let event = crate::utils::process::parse_gamdl_output(&clean_line);
        let progress = gamdl_service::GamdlProgress {
            download_id: dl_id.to_string(),
            event,
        };
        let _ = app.emit("gamdl-output", &progress);

        // Emit to activity-log: last segment only (normal) or all (verbose).
        // Disk mirror happens unconditionally inside the helper (#541) —
        // the file is the forensic record. Phase 3.5e: routes through
        // `emit_subprocess_line` instead of constructing the event +
        // calling `app.emit` directly so future emission rules only
        // need to touch one place.
        let show_in_ui = verbose || Some(idx) == last_segment_idx;
        crate::utils::activity_log::emit_subprocess_line(
            app, dl_id, stream, clean_line, show_in_ui,
        );
    }

    last_clean
}

/// Recursively collect every `.mp4` / `.m4v` file under the given root.
///
/// Used to snapshot the video-file set before a music video download so we
/// can diff the new set afterwards and pinpoint which files GAMDL just
/// produced. Depth-limited at 4 levels to keep the scan bounded on large
/// music libraries.
pub(crate) fn snapshot_video_files(root: &str) -> std::collections::HashSet<std::path::PathBuf> {
    let root_path = std::path::Path::new(root);
    if !root_path.is_dir() {
        return std::collections::HashSet::new();
    }
    // Migrated to the shared `utils::fs_walk::walk_dir_depth` helper
    // (#716 finding #1, v1.0.3 prep). Depth limit of 4 preserved —
    // matches the GAMDL `Output/Artist/Album/Disc/file` shape with one
    // level of headroom for compilation-style nesting. Net diff: 17 LOC
    // → 9 LOC, identical mp4/m4v filtering.
    crate::utils::fs_walk::walk_dir_depth(root_path, 4, |path| {
        if !path.is_file() {
            return None;
        }
        let ext = path.extension().and_then(|e| e.to_str())?;
        let ext_lc = ext.to_ascii_lowercase();
        if ext_lc == "mp4" || ext_lc == "m4v" {
            Some(path.to_path_buf())
        } else {
            None
        }
    })
    .into_iter()
    .collect()
}

/// Identify freshly-created music video files (those not present in
/// `pre_existing`) and run subtitle extraction + lyrics pairing on each.
///
/// Fire-and-forget: any failure is logged but never fails the download.
pub(crate) async fn extract_music_video_subtitles_for_new_files(
    app: &tauri::AppHandle,
    dl_id: &str,
    output_root: &str,
    pre_existing: &std::collections::HashSet<std::path::PathBuf>,
) {
    let ffprobe_path = match super::metadata_tag_service::get_ffprobe_path(app) {
        Ok(p) => p,
        Err(e) => {
            log::debug!("Skipping music video subtitle extraction for {dl_id}: {e}");
            return;
        }
    };
    let ffmpeg_path = super::dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_path.exists() {
        log::debug!("Skipping music video subtitle extraction for {dl_id}: ffmpeg missing");
        return;
    }

    // Diff pre/post video sets to isolate the new files.
    let post_existing = snapshot_video_files(output_root);
    let new_videos: Vec<_> = post_existing.difference(pre_existing).cloned().collect();

    if new_videos.is_empty() {
        log::debug!("No new music video files detected for {dl_id}");
        return;
    }

    for video_path in &new_videos {
        // 0. Defensive filename-safety classification (#532).
        //    GAMDL's MV pipeline is the highest-risk source for
        //    degenerate output paths (#527 was the RC-blocker shape
        //    landing here). The classifier surfaces a warn-severity
        //    activity-log line so the user can investigate
        //    suspicious or broken paths even after the Tier 4
        //    safety net catches the worst case.
        match crate::utils::fs_safe::classify_path_components(video_path) {
            crate::utils::fs_safe::FilenameClassification::Ok => {}
            crate::utils::fs_safe::FilenameClassification::Suspicious { reason } => {
                emit_download_warn(
                    app,
                    dl_id,
                    &format!(
                        "Filename safety: music video at '{}' is suspicious — {reason}",
                        video_path.display()
                    ),
                );
            }
            crate::utils::fs_safe::FilenameClassification::Degenerate { reason } => {
                emit_download_error(
                    app,
                    dl_id,
                    &format!(
                        "Filename safety (#532): music video at '{}' is degenerate — {reason}. The Tier 4 safety net should have prevented this; please report as a bug.",
                        video_path.display()
                    ),
                );
            }
        }

        // 1. Extract any embedded subtitle / caption streams to sidecars.
        match super::music_video_subtitle_service::extract_subtitles_to_sidecars(
            &ffprobe_path,
            &ffmpeg_path,
            video_path,
        )
        .await
        {
            Ok(report) => {
                // Anything the person needs to know, even when nothing
                // failed outright: a subtitle that could not be saved
                // because the drive refused every way of adding it without
                // the risk of replacing a file, or a temporary file that
                // could not be removed (Codex's review of round 5, findings
                // 3 and 4; stand-in review of round 6, finding 1).
                for notice in &report.notices {
                    emit_download_warn(app, dl_id, notice);
                }
                if report.written == 0 {
                    log::debug!("No new subtitle sidecars for {}", video_path.display());
                } else {
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Extracted {} subtitle/caption track(s) from {}",
                            report.written,
                            video_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("music video")
                        ),
                    );
                }
            }
            Err(e) => {
                log::warn!(
                    "Music video subtitle extraction failed for {}: {e}",
                    video_path.display()
                );
            }
        }

        // 2. Pair any matching song lyrics sidecars from the album folder
        //    (works when the music video was a companion to an album track).
        if let Some(album_dir) = video_path.parent() {
            let paired = super::music_video_subtitle_service::pair_song_lyrics_with_music_video(
                album_dir, video_path,
            );
            if paired > 0 {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Paired {paired} song-lyrics file(s) with {}",
                        video_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("music video")
                    ),
                );
            }
        }

        // 3. Embed the sidecar cover thumbnail into the MP4 and
        //    delete it (#533 / #569). Gated on the
        //    `music_video_embed_cover_sidecar` setting (default
        //    true). Most modern players read the embedded poster
        //    atom from the MP4 directly, so the sidecar is just
        //    library clutter. The verify-before-delete logic in
        //    `embed_and_remove_sidecar` makes the embed safe — a
        //    failed write never loses the only cover copy.
        let settings_for_mv = load_settings_for_queue(app);
        if settings_for_mv.music_video_embed_cover_sidecar {
            use super::music_video_cover_embed::{embed_and_remove_sidecar, EmbedOutcome};
            let video_filename = video_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("music video")
                .to_string();
            // The cover write rewrites the video's tags, so it takes the
            // app-wide write lock (taken here, never inside the embed).
            let embed_outcome = {
                let _guard = crate::utils::file_locks::global().lock(video_path).await;
                embed_and_remove_sidecar(video_path)
            };
            match embed_outcome {
                EmbedOutcome::Embedded {
                    sidecar_filename,
                    bytes_embedded,
                } => {
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Music video: embedded {sidecar_filename} ({} bytes) into {video_filename} and removed sidecar",
                            bytes_embedded,
                        ),
                    );
                }
                EmbedOutcome::NoSidecar => {
                    log::debug!("No cover sidecar found next to {}", video_path.display());
                }
                EmbedOutcome::Failed {
                    sidecar_filename,
                    reason,
                } => {
                    // Sidecar kept (safe by design) — warn so the
                    // user knows the embed didn't happen.
                    log::warn!(
                        "Music video cover-embed failed for {}: {reason}",
                        video_path.display(),
                    );
                    emit_download_warn(
                        app,
                        dl_id,
                        &format!(
                            "Music video cover embed failed — {sidecar_filename} kept as sidecar: {reason}"
                        ),
                    );
                }
            }
        }
    }
}

/// Folder template applied to GAMDL music-video downloads when the MV
/// has no album context (direct `/music-video/` URLs AND upstream
/// iTunes Lookup did not return album linkage). Uses a fixed
/// `{artist}/Music Videos` layout rather than inheriting the user's
/// `no_album_folder_template` — that setting is audio-oriented and
/// legacy installs may still hold the pre-v2 default `"{artist}/[Unknown]"`
/// which produces a literal `[Unknown]` directory (#531).
///
/// ## Resolution order for MV → album folder
///
/// This constant is the **last-resort** template. The actual placement
/// cascade is (highest to lowest priority):
///
/// 1. **GAMDL's internal iTunes Lookup** (`interface_music_video.py`):
///    if the MV's iTunes entry exposes a collection row, GAMDL
///    populates `tags.album` / `tags.album_artist` and uses
///    `album_folder_template` — MV lands alongside the audio tracks
///    in `{album_artist}/{album}/`. GAMDL handles this natively.
/// 2. **Apple Music Catalog API** (not yet wired in — tracked in #537):
///    when iTunes returns no collection row, fall back to Apple's
///    `music-videos/{id}?include=albums` endpoint and pre-fill
///    `no_album_folder_template` with the resolved literal path.
/// 3. **MeedyaDL-known parent album context** (not yet wired in — #537):
///    when the MV is discovered as a companion to an album URL we
///    already downloaded, we *know* the parent album regardless of
///    what either API says — override the folder template directly.
/// 4. **This constant**: `{artist}/Music Videos/` — reached only when
///    all three lookups above fail or are unavailable. Safe and
///    predictable; never empty.
///
/// ## Filename-safety contract (#551)
///
/// `services::filename_safety::GamdlMusicVideoFallback` mirrors this
/// constant (and `MV_NO_ALBUM_FILE_TEMPLATE`) as string literals so the
/// design-review checks can prove the engine's no-album fallback is
/// collision-safe without dragging this module into the contract's
/// compilation unit. If either constant changes, update the literals
/// in `services/filename_safety.rs` too.
///
/// ## Not reached by uploaded videos (#549, decided 2026-05-17)
///
/// Apple Music's label/artist-uploaded videos (backstage clips, live
/// sessions, interviews) have their own GAMDL entry points
/// (`downloader_uploaded_video.py` / `interface_uploaded_video.py`) and
/// tag shape (`{artist, date, title, title_id, storefront}` — no album
/// context). The MeedyaDL decision is to **accept** uploaded-video URLs
/// (they reach GAMDL via the URL audit catch-all in `commands::gamdl`),
/// but **defer wiring** them through `download_music_video_by_url()`
/// until a concrete test URL is available — the uploaded-video URL
/// shape is undocumented publicly and we can't safely add a regex
/// without an example. The audit log explicitly names uploaded videos
/// in the WARN line so users understand what they're seeing. Until a
/// test URL surfaces and the follow-up wiring lands, an uploaded-video
/// download inherits the user's audio-oriented `no_album_*` templates
/// — same collision risk class as #527/#531, different URL scheme.
/// The GAMDL `--uploaded-video-quality` flag is already a pass-through
/// (`GamdlOptions.uploaded_video_quality`).
pub(crate) const MV_NO_ALBUM_FOLDER_TEMPLATE: &str = "{artist}/Music Videos";

/// File template applied to GAMDL music-video downloads when the MV has
/// no album context. Uses `{title} ({title_id})` so the filename is
/// **guaranteed unique within the Apple Music catalogue** — `{title_id}`
/// is the numeric MV ID, deterministic across re-downloads and unique
/// per MV. Legacy installs may still hold the pre-v2 default
/// `"{disc} - "` which produces empty `-.mp4` filenames for content
/// without a `{disc}` (#531).
///
/// ## Why include `{title_id}` instead of just `{title}`?
///
/// Same-artist MVs with identical titles do occur in real catalogues
/// (Clean/Explicit cuts, remixes, live versions, region-specific
/// re-releases). With `overwrite=false` (MeedyaDL's default), the
/// second download would silently skip with a `MediaFileExists`
/// exception — no data loss but no user-visible warning either.
///
/// `{title_id}` is chosen over any datetime suffix because datetimes
/// defeat GAMDL's own dedup: every re-download would create a new file
/// rather than being recognised as the same MV.
///
/// This template is only reached in the last-resort path (see the
/// docstring on `MV_NO_ALBUM_FOLDER_TEMPLATE`). Apple-Music-linked or
/// iTunes-linked MVs land in their album folder via GAMDL's native
/// `single_disc_file_template` / `multi_disc_file_template` and do not
/// use this constant at all — their filenames remain clean.
pub(crate) const MV_NO_ALBUM_FILE_TEMPLATE: &str = "{title} ({title_id})";

/// Heuristic check that returns `true` when the supplied URL looks
/// like it came from Apple Music's `editorialVideo` block rather than
/// from the music-video catalog. Motion-art / spotlight HLS streams
/// are hosted under a distinct subdomain pattern and never have a
/// `music.apple.com/<region>/music-video/...` shape.
///
/// Used by [`download_music_video_by_url`] as a defensive guard
/// against the #536 failure mode where a motion-art URL would be
/// passed in by mistake (e.g. a future caller refactor that confuses
/// `relationships.music-videos.data` with `editorialVideo`).
///
/// Two positive signals:
///   1. Host contains "video-ssl.itunes.apple.com" /
///      "play-edge.itunes.apple.com" → these are Apple's HLS hosts
///      for editorial / motion art, never used for music-video
///      master m3u8s (which come from the GAMDL DRM resolver path).
///   2. Path ends in `.m3u8` AND there's no `/music-video/` segment
///      anywhere in the URL.
///
/// False negative is tolerable (we just fall through and GAMDL tells
/// us the URL is bad); false positive would block a real MV, so the
/// signals are deliberately conservative.
pub(crate) fn is_likely_motion_art_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains("/music-video/") {
        return false; // unambiguously an MV catalog URL
    }
    // Apple's editorial HLS hosts.
    let looks_like_editorial_host = lower.contains("video-ssl.itunes.apple.com")
        || lower.contains("play-edge.itunes.apple.com")
        || lower.contains("itunes.apple.com/video-cdn");
    let looks_like_hls = lower.ends_with(".m3u8") || lower.contains(".m3u8?");
    looks_like_editorial_host && looks_like_hls
}

/// **MV filename-resolution Tier 2** (#558): try to resolve the MV's
/// parent album via Apple Music's
/// `music-videos/{id}?include=albums` endpoint and return a literal
/// folder path (e.g. `Anne-Marie/Psycho - Single`) suitable for
/// passing as `no_album_folder_template`.
///
/// Returns `None` on every fail-soft condition (URL parse miss, no
/// MusicKit credentials, 404, 401/403, missing album name/artist) so
/// callers always fall through to Tier 4 cleanly. Network errors are
/// logged at debug but also produce `None` — Tier 4 is a correct
/// safety net, not a degraded mode.
///
/// **Why a literal path instead of a template string?** GAMDL renders
/// `no_album_folder_template` against the MV's own tag bag — which
/// doesn't carry `{album}` / `{album_artist}` for unanchored MVs.
/// We could pre-render the user's `album_folder_template` here, but
/// that risks template-syntax drift (curly-brace placeholders the
/// user has customised). Passing the literal path bypasses GAMDL's
/// template engine for this specific call without affecting global
/// settings — same trick #531 uses for the Tier 4 safety net.
pub(crate) async fn try_resolve_mv_album_folder_via_catalog_api(
    app: &tauri::AppHandle,
    video_url: &str,
    settings: &crate::models::settings::AppSettings,
) -> Option<String> {
    // Parse the MV URL to extract storefront + video_id.
    let parsed = crate::services::apple_music_api::parse_apple_music_url(video_url)?;
    if parsed.content_type != "music-video" {
        return None;
    }
    // For music-video URLs the parser stores the MV's numeric ID in
    // `album_id` (the regex reuses the album-style numeric-tail
    // capture group). Per parse_apple_music_url's contract, this is
    // populated for every music-video URL the regex matches.
    let storefront = parsed.storefront;
    let video_id = parsed.album_id;

    // Resolve MusicKit JWT (same path as animated artwork / syllable
    // lyrics). Skip Tier 2 silently when no credentials are available
    // — Tier 4 still gives a correct result.
    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match crate::services::apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        Ok(None) => None,
        Err(e) => {
            log::debug!("Tier 2 skipped — keychain read failed: {e}");
            return None;
        }
    };
    let token_pair = match crate::services::apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            log::debug!("Tier 2 skipped — no MusicKit token available");
            return None;
        }
        Err(e) => {
            log::debug!("Tier 2 skipped — token resolution failed: {e}");
            return None;
        }
    };
    let (jwt, _src) = token_pair;

    // Fetch + parse. fetch_music_video_album_linkage already maps
    // 404/401/403 to Ok(None); anything else (Err) we treat as a
    // Tier-2 miss and fall through.
    let linkage = match crate::services::apple_music_api::fetch_music_video_album_linkage(
        &jwt,
        &storefront,
        &video_id,
    )
    .await
    {
        Ok(Some(l)) => l,
        Ok(None) => return None,
        Err(e) => {
            log::debug!("Tier 2 lookup failed for {video_url}: {e} — falling through");
            // Mark `app` as used to keep the signature stable when
            // the activity-log import is needed for non-debug logs.
            let _ = app;
            return None;
        }
    };

    // Build the literal folder path. Strip filesystem-unsafe chars
    // the way GAMDL's template engine does (`/ \ : * ? " < > |` →
    // `_`, plus trim leading/trailing dots and whitespace which
    // collapse to invisible files on Unix and bare-name conflicts
    // on Windows). The result is safe on every FS MeedyaDL targets.
    let safe_artist = sanitize_fs_segment(&linkage.artist_name);
    let safe_album = sanitize_fs_segment(&linkage.album_name);
    if safe_artist.is_empty() || safe_album.is_empty() {
        log::debug!("Tier 2 produced empty artist/album after sanitisation — falling through");
        return None;
    }
    Some(format!("{safe_artist}/{safe_album}"))
}

/// Strip filesystem-unsafe characters from a path segment. Mirrors
/// the sanitisation GAMDL applies internally to album / artist names
/// before rendering them into folder templates, so the resulting
/// path is identical to what GAMDL would produce when rendering
/// `{album_artist}/{album}` itself.
pub(crate) fn sanitize_fs_segment(raw: &str) -> String {
    // Characters that cannot appear in a file or folder name on at least one
    // supported system (Windows is the strictest).
    const UNSAFE: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    raw.chars()
        .map(|c| {
            if UNSAFE.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string()
}

/// Shared helper used by both the MusicKit-based video companion pipeline
/// (Step 6) and the MusicBrainz fallback discovery (Step 6b). Builds a
/// minimal `GamdlOptions` using the user's video quality settings and
/// spawns a GAMDL subprocess. Fire-and-forget: failures are logged but
/// do not affect the primary download.
///
/// Returns `true` if the download succeeded, `false` otherwise.
pub(crate) async fn download_music_video_by_url(
    app: &tauri::AppHandle,
    dl_id: &str,
    video_url: &str,
    video_label: &str,
    settings: &crate::models::settings::AppSettings,
    parent_album_path: Option<&str>,
) -> bool {
    // M9-7 belt-and-braces: this function is only called from Apple
    // Music enrichment branches (Step 6 MV companion @ ~4321 and
    // MusicBrainz fallback @ ~10035), both of which are nested under
    // `is_apple_music` checks. The defensive early-return here catches
    // any future regression that would route a Spotify item through
    // GAMDL's MV pipeline — votify has no equivalent and the GAMDL
    // build would silently fail on the unsupported URL host.
    if video_url.contains("open.spotify.com") || video_url.starts_with("spotify:") {
        log::warn!("download_music_video_by_url called with Spotify URL — skipping (M9-7 guard)");
        emit_download_log(
            app,
            dl_id,
            "Music video companion skipped — not supported for Spotify items",
        );
        return false;
    }
    // Defensive guard (#536): motion-art HLS URLs (album cover /
    // portrait cover / album spotlight / artist spotlight) come from
    // the API's `editorialVideo` block and are processed by
    // `animated_artwork_service` with fixed filenames (FrontCover.mp4
    // / FrontCoverPortrait.mp4 / AlbumSpotlightCover.mp4 /
    // ArtistSpotlightCover.mp4). They are NOT DRM-protected music
    // videos and must never reach GAMDL's MV pipeline — which would
    // try to apply DRM unwrapping and route the file through
    // `no_album_*` templates, producing `[Unknown]/-.mp4`-style
    // collision-prone paths (the #527 / #532 failure mode).
    //
    // Architecturally the two pipelines read different API fields and
    // cannot cross-pollinate by construction (motion art comes from
    // `extend=editorialVideo` URLs; MVs come from `relationships.
    // music-videos.data[*].attributes.url`). This guard is a
    // belt-and-braces sanity check that catches any future regression
    // — if a motion-art URL somehow reaches here, we bail out with a
    // clear log + activity-log line rather than corrupt the user's
    // motion-art assets.
    if is_likely_motion_art_url(video_url) {
        log::warn!(
            "Refusing to route motion-art URL through GAMDL MV pipeline (#536): {video_label} \
             — should go through animated_artwork_service instead"
        );
        emit_app_log(
            app,
            &format!(
                "Skipped MV download — URL looks like motion-art (cover / spotlight) which doesn't belong in the music-video pipeline: {video_label}"
            ),
        );
        return false;
    }

    // Inherit the user's filename/folder templates, tool paths, and metadata
    // settings so the music video output matches what the primary pipeline
    // produces. Without these, GAMDL falls back to its own defaults which
    // can yield empty filenames like "-.mp4" for music videos (#481).
    //
    // The `no_album_*` templates are an exception: a direct
    // `/music-video/` URL has no album context, so GAMDL routes it
    // through the no-album template path. The user's audio-oriented
    // no-album templates are unsuitable here — legacy installs may still
    // have them set to `"{artist}/[Unknown]"` + `"{disc} - "` (the
    // pre-v2 defaults), which yield literal `[Unknown]` directories and
    // empty `-.mp4` filenames for MVs (#531). Override with MV-safe
    // fixed templates regardless of user settings.
    //
    // Folder-template cascade (#558 Tier 2 + #559 Tier 3 + Tier 4
    // safety net). Tier precedence is deliberate:
    //
    //   Tier 3 wins over Tier 2 because the local parent-album path
    //   is more trustworthy than the API — the user has already
    //   downloaded a specific release into a specific folder, and
    //   the API might point at a different re-release of the same
    //   recording.
    //
    //   Tier 2 wins over Tier 4 because Apple Music's album linkage,
    //   when present, gives the user-facing album-folder layout
    //   instead of the generic `{artist}/Music Videos/` bucket.
    //
    //   Tier 4 is the always-safe last resort.
    let resolved_no_album_folder_template = if let Some(parent_path) = parent_album_path {
        // Tier 3: parent album context known from caller (MV companion
        // or MusicBrainz fallback within an album-scoped enrichment
        // task). Use the literal on-disk path directly.
        log::info!("MV folder resolved via Tier 3 (parent album context): {parent_path}");
        emit_app_log(
            app,
            &format!("MV folder routed to parent album via Tier 3: {video_label} → {parent_path}"),
        );
        parent_path.to_string()
    } else if let Some(literal_path) =
        try_resolve_mv_album_folder_via_catalog_api(app, video_url, settings).await
    {
        // Tier 2: Apple Music Catalog endpoint returned an album linkage.
        log::info!(
            "MV folder resolved via Tier 2 (Apple Music Catalog album linkage): {literal_path}"
        );
        emit_app_log(
            app,
            &format!(
                "MV folder routed to album linkage via Tier 2: {video_label} → {literal_path}"
            ),
        );
        literal_path
    } else {
        // Tier 4: safety net.
        log::debug!("Tier 2 + Tier 3 missed for MV {video_label} — using Tier 4 safety net");
        MV_NO_ALBUM_FOLDER_TEMPLATE.to_string()
    };

    let opts = crate::models::gamdl_options::GamdlOptions {
        output_path: crate::services::config_service::usable_download_folder(&settings.output_path),
        // The overwrite setting, on the command line (config.ini no longer
        // carries it; this run used to inherit it from there).
        overwrite: Some(settings.overwrite),
        music_video_resolution: Some(settings.default_video_resolution.clone()),
        music_video_codec_priority: Some(settings.video_codec_priority_cli()),
        music_video_remux_format: Some(settings.default_video_remux_format.clone()),
        temp_path: Some(if settings.temp_path.is_empty() {
            std::env::temp_dir()
                .join("MeedyaDL")
                .to_string_lossy()
                .to_string()
        } else {
            settings.temp_path.clone()
        }),
        cookies_path: settings.cookies_path.clone(),
        use_wrapper: Some(settings.use_wrapper),
        wrapper_account_url: if settings.use_wrapper {
            Some(settings.wrapper_account_url.clone())
        } else {
            None
        },
        // Filename / folder templates — album-context paths inherit the
        // user's templates (MVs discovered via album URLs land alongside
        // their album tracks). The no-album paths force fixed MV-safe
        // templates — see rationale above.
        album_folder_template: Some(settings.album_folder_template.clone()),
        compilation_folder_template: Some(settings.compilation_folder_template.clone()),
        no_album_folder_template: Some(resolved_no_album_folder_template),
        single_disc_file_template: Some(settings.single_disc_file_template.clone()),
        multi_disc_file_template: Some(settings.multi_disc_file_template.clone()),
        no_album_file_template: Some(MV_NO_ALBUM_FILE_TEMPLATE.to_string()),
        playlist_file_template: Some(settings.playlist_file_template.clone()),
        // Tool paths (ffmpeg, mp4decrypt, mp4box, N_m3u8DL-RE) so GAMDL can
        // resolve the managed binaries instead of relying on PATH lookup.
        ffmpeg_path: settings.ffmpeg_path.clone(),
        mp4decrypt_path: settings.mp4decrypt_path.clone(),
        mp4box_path: settings.mp4box_path.clone(),
        nm3u8dlre_path: settings.nm3u8dlre_path.clone(),
        // Metadata / language so music-video tags are localised consistently.
        // Sent in its standard form -- see `language_arg_for_gamdl`'s own
        // comment (independent review, round 3 of #1244) for why the
        // STORED setting is left untouched and only what reaches GAMDL's
        // command line here is cleaned up.
        language: Some(crate::utils::language::language_arg_for_gamdl(
            &settings.language,
        )),
        truncate: settings.truncate,
        download_mode: Some(settings.download_mode.clone()),
        remux_mode: Some(settings.remux_mode.clone()),
        no_config_file: Some(true),
        ..Default::default()
    };

    // Which way of unlocking copy-protected tracks to use.
    //
    // This one is built from scratch rather than copied from the
    // download's own options, and it passes `no_config_file`, so nothing
    // reaches it by accident — every setting it honours has to be named
    // here. A whole-branch review found the new unlocking choice missing:
    // somebody who had chosen PlayReady got it for their music and
    // quietly not for their music videos, which is the kind of gap
    // nobody reports because nothing looks wrong.
    //
    // Decided through the same function as everywhere else, so a device
    // file that has since been deleted is handled identically here.
    let mut opts = opts;
    if let super::options::DrmPlan::PlayReady { prd_path } =
        super::options::plan_drm_backend_for_now(settings)
    {
        opts.drm_backend = Some(crate::models::settings::DrmBackend::PlayReady);
        opts.prd_path = Some(prd_path);
    }

    let urls = vec![video_url.to_string()];
    let mut cmd = match super::gamdl_service::build_gamdl_command_public(app, &urls, &opts) {
        Ok(c) => c,
        Err(e) => {
            log::debug!("Music video download command build failed for {dl_id}: {e}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video download failed ({video_label}): {e}"),
            );
            return false;
        }
    };

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Stop the program if the task waiting for it is dropped. Without
    // this, the task goes away and the program it started carries on,
    // writing files nobody is waiting for. The supervised companion runs
    // have always done this; this one was missed. Found by a full review
    // of the codebase.
    //
    // When that actually happens here: this download runs inside the
    // enrichment step (Step 6 / 6b), which has a deadline. When the
    // deadline passes, the step is aborted, this task is dropped with
    // it, and this setting stops the program. That is the only case.
    //
    // What it does NOT do — an earlier version of this comment said it
    // did, and it was wrong (checked 27 Sept 2026, #1215 finding 5):
    //
    // - Cancel does not stop it. Cancelling marks the item cancelled,
    //   but nothing ends or aborts the enrichment step, and the loops
    //   that call this look only at whether the app is shutting down,
    //   and only between videos. A video already downloading carries on
    //   to the end after Cancel. Making Cancel stop it is a change of
    //   behaviour, raised as its own issue.
    // - Quitting the app does not trigger it either. The app ends its
    //   whole process at once without dropping running tasks, so this
    //   setting never fires then. What becomes of a program still
    //   running at that moment has not been checked.
    cmd.kill_on_drop(true);

    // Snapshot the set of video files under the output directory BEFORE
    // GAMDL runs so we can identify exactly which files were freshly
    // produced for this music video (#483). Used for subtitle extraction.
    let pre_existing_videos = snapshot_video_files(&settings.output_path);

    // Stream stdout/stderr line-by-line (with `\r` splitting) instead of
    // buffering the entire process output with `wait_with_output()`.
    // Streaming keeps the progress bar live AND prevents yt-dlp's
    // carriage-return progress blob from arriving as a single 100KB
    // activity-log row (see `emit_companion_stream_line` for rationale).
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::debug!("Failed to spawn music video download for {dl_id}: {e}");
            return false;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_task = stdout.map(|out| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                emit_companion_stream_line(&app, &dl_id, "stdout", &line).await;
            }
        })
    });

    let stderr_task = stderr.map(|err| {
        let app = app.clone();
        let dl_id = dl_id.to_string();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(err);
            let mut lines = reader.lines();
            let mut last = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(clean) = emit_companion_stream_line(&app, &dl_id, "stderr", &line).await
                {
                    last = clean;
                }
            }
            last
        })
    });

    let status = child.wait().await;
    if let Some(t) = stdout_task {
        let _ = t.await;
    }
    let last_err = if let Some(t) = stderr_task {
        t.await.unwrap_or_default()
    } else {
        String::new()
    };

    match status {
        Ok(s) if s.success() => {
            // Finishing without an error is not the same as having downloaded
            // something (#1155).
            //
            // When Apple Music does not offer a video in any format the user
            // allows, the download tool says so as a warning, moves on, and
            // finishes successfully. This used to report "Music video
            // downloaded" on the strength of that success, so the activity log
            // claimed a file that was never written.
            //
            // Checked by looking for a new file rather than by reading the
            // tool's message, because the message is worded identically for
            // songs and videos and only the format list inside it differs.
            let new_videos = snapshot_video_files(&settings.output_path)
                .difference(&pre_existing_videos)
                .count();
            if new_videos == 0 {
                log::info!(
                    "Music video produced no file for {dl_id}: {video_label} \
                     (finished without error, but nothing was written)"
                );
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Music video not available in any of your video formats — \
                         skipped: {video_label}"
                    ),
                );
                return false;
            }
            log::info!("Music video downloaded for {dl_id}: {video_label}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video downloaded: {video_label}"),
            );
            // Post-process freshly downloaded music videos: extract any
            // embedded subtitle / caption streams into sidecar files and
            // (best-effort) copy the matching song's lyrics alongside.
            extract_music_video_subtitles_for_new_files(
                app,
                dl_id,
                &settings.output_path,
                &pre_existing_videos,
            )
            .await;
            true
        }
        Ok(_) => {
            let shown_err = if last_err.is_empty() {
                "unknown error".to_string()
            } else {
                last_err
            };
            log::debug!("Music video download failed for {dl_id} ({video_label}): {shown_err}");
            emit_download_log(
                app,
                dl_id,
                &format!("Music video failed ({video_label}): {shown_err}"),
            );
            false
        }
        Err(e) => {
            log::debug!("Music video process error for {dl_id}: {e}");
            false
        }
    }
}

// ============================================================
// Lyrics format fallback
// ============================================================

/// Run lyrics format fallback when the primary format didn't produce
/// lyrics for all tracks.
///
/// After the primary download (typically with `--synced-lyrics-format ttml`),
/// checks if every one of THIS download's media files (`targets`) has a
/// corresponding lyrics sidecar; older files in the same folder are not
/// counted either way (#1441). If not,
/// retries with fallback formats in content-type-specific order:
/// - **Audio** (`.m4a`): TTML → LRC → SRT
/// - **Video** (`.m4v`/`.mp4`): TTML → SRT → LRC
///
/// Each fallback attempt spawns GAMDL with `--synced-lyrics-format <fmt>
/// --synced-lyrics-only`. The chain stops as soon as lyrics coverage
/// matches the number of media files (or all formats are exhausted).
pub(crate) async fn run_lyrics_fallback(
    app: &tauri::AppHandle,
    dl_id: &str,
    targets: &[std::path::PathBuf],
    urls: &[String],
    settings: &crate::models::settings::AppSettings,
) {
    // M9-7 belt-and-braces: lyrics fallback uses GAMDL's
    // `synced_lyrics_only` flag which only accepts Apple Music URLs.
    // The caller (line ~9072) is nested under Apple Music's lyrics
    // gap pass, so reaching here with a Spotify URL is a future
    // regression. Skip cleanly.
    if urls
        .iter()
        .any(|u| u.contains("open.spotify.com") || u.starts_with("spotify:"))
    {
        log::warn!("run_lyrics_fallback called with Spotify URL(s) — skipping (M9-7 guard)");
        emit_download_log(
            app,
            dl_id,
            "Lyrics fallback skipped — not supported for Spotify items",
        );
        return;
    }

    let (lyrics_count, audio_count, video_count) = super::lyrics_steps::lyrics_coverage(targets);
    let total_media = audio_count + video_count;

    if total_media == 0 {
        return; // No media files to match lyrics against
    }

    if lyrics_count >= total_media {
        log::debug!("Lyrics fallback not needed for {dl_id}: {lyrics_count}/{total_media} tracks have lyrics");
        return; // All tracks already have lyrics
    }

    // GAMDL 4.x has no lyrics-only mode: the request would be dropped and
    // the run would download the whole album again, in GAMDL's default
    // format, beside (or over) the files already there. 4.x writes what
    // lyrics it has (`.lrc` or `.txt`) during the main download, so there
    // is nothing for this step to do (found in review round 4). Checked
    // only once lyrics really are missing, so the line is not written on
    // every download (review round 5).
    if crate::services::gamdl_capabilities::detected_cli_generation()
        == crate::services::gamdl_capabilities::GamdlCliGeneration::V4
    {
        emit_download_log(
            app,
            dl_id,
            "Lyrics fallback skipped — GAMDL 4 has no lyrics-only download",
        );
        return;
    }

    // Determine fallback chain based on content type.
    // If there are more video files than audio, treat it as video content.
    let is_video = video_count > audio_count;
    let fallback_chain: Vec<LyricsFormat> = if is_video {
        // Video: TTML (already tried) → SRT → LRC
        vec![LyricsFormat::Srt, LyricsFormat::Lrc]
    } else {
        // Audio: TTML (already tried) → LRC → SRT
        vec![LyricsFormat::Lrc, LyricsFormat::Srt]
    };

    emit_download_log(
        app,
        dl_id,
        &format!(
            "Lyrics fallback: {lyrics_count}/{total_media} tracks have lyrics — trying alternative formats"
        ),
    );

    for format in &fallback_chain {
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Lyrics fallback: trying {} format...",
                format.to_cli_string()
            ),
        );

        // Build minimal options for lyrics-only download using struct init
        // syntax (clippy: field_reassign_with_default). Carries over auth
        // and template settings so lyrics land in the correct folder.
        let opts = GamdlOptions {
            synced_lyrics_format: Some(format.clone()),
            synced_lyrics_only: Some(true),
            output_path: crate::services::config_service::usable_download_folder(
                &settings.output_path,
            ),
            temp_path: Some(if settings.temp_path.is_empty() {
                std::env::temp_dir()
                    .join("MeedyaDL")
                    .to_string_lossy()
                    .to_string()
            } else {
                settings.temp_path.clone()
            }),
            cookies_path: settings.cookies_path.clone(),
            use_wrapper: Some(settings.use_wrapper),
            wrapper_account_url: if settings.use_wrapper {
                Some(settings.wrapper_account_url.clone())
            } else {
                None
            },
            single_disc_file_template: Some(settings.single_disc_file_template.clone()),
            multi_disc_file_template: Some(settings.multi_disc_file_template.clone()),
            no_album_file_template: Some(settings.no_album_file_template.clone()),
            playlist_file_template: Some(settings.playlist_file_template.clone()),
            album_folder_template: Some(settings.album_folder_template.clone()),
            compilation_folder_template: Some(settings.compilation_folder_template.clone()),
            no_album_folder_template: Some(settings.no_album_folder_template.clone()),
            overwrite: Some(settings.overwrite),
            ..Default::default()
        };

        // Build and run GAMDL command
        let mut cmd = match super::gamdl_service::build_gamdl_command_public(app, urls, &opts) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Lyrics fallback command build failed for {dl_id}: {e}");
                continue;
            }
        };

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Stop the program if the task waiting for it is dropped.
        // Without this, the task goes away and the program it started
        // carries on, writing files nobody is waiting for. The supervised
        // companion runs have always done this; this one was missed.
        // Found by a full review of the codebase.
        //
        // When that actually happens here: this lyrics fallback runs
        // inside the enrichment step (Step 2b), which has a deadline.
        // When the deadline passes, the step is aborted, this task is
        // dropped with it, and this setting stops the program. That is
        // the only case.
        //
        // What it does NOT do — an earlier version of this comment said
        // it did, and it was wrong (checked 27 Sept 2026, #1215 finding
        // 5):
        //
        // - Cancel does not stop it. Cancelling marks the item cancelled,
        //   but nothing ends or aborts the enrichment step, and this loop
        //   does not look at the item's state at all — the app-shutdown
        //   check is made once, by the caller, before the fallback
        //   starts. Making Cancel stop it is a change of behaviour,
        //   raised as its own issue.
        // - Quitting the app does not trigger it either. The app ends its
        //   whole process at once without dropping running tasks, so this
        //   setting never fires then. What becomes of a program still
        //   running at that moment has not been checked.
        cmd.kill_on_drop(true);

        match cmd.spawn() {
            Ok(child) => match child.wait_with_output().await {
                Ok(output) if output.status.success() => {
                    log::info!(
                        "Lyrics fallback ({}) succeeded for {dl_id}",
                        format.to_cli_string()
                    );
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Lyrics fallback: {} format downloaded successfully",
                            format.to_cli_string()
                        ),
                    );
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let last_err = stderr.lines().last().unwrap_or("unknown error");
                    log::debug!(
                        "Lyrics fallback ({}) failed for {dl_id}: {last_err}",
                        format.to_cli_string()
                    );
                    emit_download_log(
                        app,
                        dl_id,
                        &format!(
                            "Lyrics fallback: {} format failed — {}",
                            format.to_cli_string(),
                            last_err
                        ),
                    );
                    continue; // Try next format in chain
                }
                Err(e) => {
                    log::debug!("Lyrics fallback process error for {dl_id}: {e}");
                    continue;
                }
            },
            Err(e) => {
                log::debug!("Failed to spawn lyrics fallback for {dl_id}: {e}");
                continue;
            }
        }

        // Re-check lyrics coverage after this fallback attempt
        let new_lyrics_count = super::lyrics_steps::lyrics_coverage(targets).0;
        if new_lyrics_count >= total_media {
            emit_download_log(
                app,
                dl_id,
                &format!("Lyrics fallback complete: {new_lyrics_count}/{total_media} tracks now have lyrics"),
            );
            return; // Coverage complete, stop the fallback chain
        }
    }

    // All fallback formats exhausted
    let final_count = super::lyrics_steps::lyrics_coverage(targets).0;
    emit_download_log(
        app,
        dl_id,
        &format!(
            "Lyrics fallback exhausted: {final_count}/{total_media} tracks have lyrics (some tracks may not have lyrics on Apple Music)"
        ),
    );
}

// ============================================================
// Queue processing: runs downloads and handles fallback/retry
/// Spawns companion and lyrics companion downloads as background tasks.
///
/// Companions are independent downloads of the same content in different
/// Detects the actual audio codec of downloaded files by running ffprobe on
/// the first `.m4a` file in the output directory.
///
/// When native priority is used (`--song-codec-priority atmos,alac,aac,...`),
/// GAMDL silently falls back through the chain. The requested codec may be
/// "atmos" but the actual files may be ALAC. This function probes the real
/// codec so companion downloads are planned against the actual content.
///
/// Returns the CLI string of the detected codec (e.g., "alac", "atmos"),
/// or falls back to `requested_codec` if detection fails.
pub(crate) async fn detect_actual_primary_codec(
    app: &tauri::AppHandle,
    dl_id: &str,
    output_path: Option<&str>,
    requested_codec: &str,
) -> String {
    let Some(dir) = output_path else {
        return requested_codec.to_string();
    };

    let dir_path = std::path::Path::new(dir);
    if !dir_path.is_dir() {
        return requested_codec.to_string();
    }

    // Find the first .m4a file in the output directory
    let first_m4a = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .find(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"))
            })
            .map(|e| e.path()),
        Err(_) => None,
    };

    let Some(m4a_path) = first_m4a else {
        log::debug!("No .m4a files in output dir for codec detection — using requested codec");
        return requested_codec.to_string();
    };

    // Parse the requested codec for the ffprobe fallback path
    let requested_song_codec =
        crate::models::gamdl_options::SongCodec::from_cli_string(requested_codec)
            .unwrap_or(crate::models::gamdl_options::SongCodec::Aac);

    // Try MediaInfo first (more reliable, especially for Atmos/AC3 distinction)
    if let Some(mediainfo_bin) = super::mediainfo_service::get_mediainfo_path(app) {
        if let Some(result) =
            super::mediainfo_service::detect_codec(&mediainfo_bin, &m4a_path).await
        {
            let actual_str = result.codec.to_cli_string().to_string();
            if actual_str != requested_codec {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Actual codec detected via MediaInfo: {} (requested: {}) — companions adjusted",
                        actual_str, requested_codec
                    ),
                );
            }
            return actual_str;
        }
        // MediaInfo present but detection failed for this file
        emit_download_log(
            app,
            dl_id,
            "MediaInfo codec detection failed — falling back to ffprobe",
        );
    }

    // Fall back to ffprobe if MediaInfo unavailable or failed
    if let Ok(ffprobe) = super::metadata_tag_service::get_ffprobe_path(app) {
        if let Some(info) =
            super::metadata_tag_service::detect_audio_info(&ffprobe, &m4a_path).await
        {
            let actual = super::metadata_tag_service::resolve_codec_from_ffprobe(
                &info,
                &requested_song_codec,
            );
            let actual_str = actual.to_cli_string().to_string();
            if actual_str != requested_codec {
                emit_download_log(
                    app,
                    dl_id,
                    &format!(
                        "Actual codec detected via ffprobe: {} (requested: {}) — companions adjusted",
                        actual_str, requested_codec
                    ),
                );
            }
            return actual_str;
        }
        // ffprobe present but detection failed
        emit_download_log(
            app,
            dl_id,
            "ffprobe codec detection also failed — using requested codec for companions",
        );
    } else {
        // Neither MediaInfo nor ffprobe available
        emit_download_log(
            app,
            dl_id,
            "No codec detection tools available — using requested codec for companions",
        );
    }

    requested_codec.to_string()
}

/// codecs (e.g., ALAC companion for an Atmos primary). They fire regardless
/// of whether the primary download succeeded or failed — the companion codec
/// may succeed where the primary format was unavailable.
///
/// This is called after any terminal download outcome (success or final
/// failure), but NOT after fallback/network retries (where the download
/// will be re-attempted and companions will fire on the final outcome).
/// Spawns companion downloads and returns a JoinHandle that resolves when
/// all companion tiers have completed. Returns None if no companions are needed.
/// Default cadence for the long-running-stage heartbeat ticker (#805).
///
/// Companion downloads + post-companion enrichment can occupy the
/// activity log with zero output for tens of minutes at a time
/// (per-track download lines all fire up front; then post-processing,
/// FFmpeg remux, tag write, lyrics conversion, ReplayGain etc. all
/// happen silently). Users reasonably conclude the app has stalled.
///
/// 120 s strikes a balance: short enough that no silent stretch
/// exceeds a couple of minutes; long enough that healthy short
/// downloads don't get spammed (a typical 8-track album with
/// companions finishes in well under 2 min, so the heartbeat never
/// even fires).
pub(crate) const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);

/// Lightweight handle for a heartbeat ticker spawned via
/// [`start_heartbeat_ticker`]. Owning the handle keeps the ticker
/// running; dropping the handle (or calling [`stop`](Self::stop))
/// signals the ticker to exit on its next tick.
///
/// Pairs naturally with [`CompanionTaskHandle`]'s existing
/// cooperative-cancel `Arc<AtomicBool>` — the same flag stops both
/// the parent stage and its heartbeat ticker, so when a stage
/// completes (success, error, or abort) the heartbeat dies with it
/// and there's no risk of orphaned tickers chatting about a stage
/// that's already over.
pub(crate) struct HeartbeatTicker {
    cancel: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl HeartbeatTicker {
    /// Signals the ticker to exit on the next tick (or
    /// immediately, if it's currently waiting on the abort signal).
    /// Idempotent — calling more than once is harmless.
    pub(crate) fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.handle.abort();
    }
}

impl Drop for HeartbeatTicker {
    /// Stops the ticker when the handle goes out of scope, so a heartbeat
    /// can never outlive the stage it reports on.
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawns a tokio task that emits an `emit_download_log` heartbeat
/// every [`HEARTBEAT_INTERVAL`] for the given download item, naming
/// the current stage (read from the queue's `processing_label`) and
/// the wall-clock elapsed time since the stage started. Returns a
/// [`HeartbeatTicker`] whose drop signals the ticker to stop (#805).
///
/// The ticker reads the cooperative-cancel flag every tick and exits
/// when set. Callers that already track a stage with an
/// `Arc<AtomicBool>` abort flag (e.g. the companion supervisor's
/// flag in [`CompanionTaskHandle`]) should pass *the same flag* here
/// so the heartbeat stops automatically when the parent stage does.
///
/// Heartbeat line format (matches the existing `[MeedyaDL]` internal-
/// emission style; the hourglass prefix lets users visually scan
/// past heartbeats when they're not the focus):
///
/// ```text
/// 17:17:38 [d3ba7a54] [MeedyaDL] ⏳ Still working — Companion: downloading atmos (tier 2)… — 8 min elapsed
/// ```
///
/// When the queue's `processing_label` is empty (no work to describe
/// — usually a brief state-transition window) the heartbeat is
/// suppressed for that tick. This avoids spurious "Still working —
/// (no label) — 2 min elapsed" lines during normal transitions.
pub(crate) fn start_heartbeat_ticker(
    app: tauri::AppHandle,
    queue: QueueHandle,
    dl_id: String,
    cancel: Arc<AtomicBool>,
    stage_kind: &'static str,
) -> HeartbeatTicker {
    let handle = tokio::spawn({
        let cancel = Arc::clone(&cancel);
        async move {
            let start = std::time::Instant::now();
            let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
            // Skip the immediate tick — we don't want a heartbeat at
            // t=0 saying "0 min elapsed" right after the stage starts.
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await;

            loop {
                interval.tick().await;
                if cancel.load(Ordering::Relaxed) {
                    return;
                }

                // Look up the current stage label from the queue.
                // The lock is released before emit so the heartbeat
                // emission doesn't hold it.
                let label = {
                    let q = queue.lock().await;
                    q.items
                        .iter()
                        .find(|i| i.status.id == dl_id)
                        .and_then(|i| i.status.processing_label.clone())
                };

                let Some(label) = label.filter(|s| !s.trim().is_empty()) else {
                    // Stage between transitions — suppress this tick.
                    continue;
                };

                // #836: dedup the stage prefix. Companion captions
                // already start with `"Companion: "` (set via
                // `set_label_only(…, &caption)` in
                // `update_companion_label_from_line`, where the caption
                // is `"Companion: downloading atmos (tier 2)…"` and the
                // like). The heartbeat formatter would then produce
                // `"⏳ Still working — Companion: Companion: downloading
                // atmos…"` — the user's 2026-05-18 / v1.8.1 screenshot
                // shows the duplication verbatim. Strip the leading
                // `"{stage_kind}: "` from `label` if it's already
                // there, so the format string's own prefix is the
                // only one users see.
                let stage_prefix = format!("{stage_kind}: ");
                let display_label = label
                    .strip_prefix(stage_prefix.as_str())
                    .unwrap_or(label.as_str());

                let elapsed = format_heartbeat_elapsed(start.elapsed());
                emit_download_log(
                    &app,
                    &dl_id,
                    &format!(
                        "⏳ Still working — {stage_kind}: {display_label} — {elapsed} elapsed"
                    ),
                );
            }
        }
    });

    HeartbeatTicker { cancel, handle }
}

/// Emits one activity-log line per animated-artwork variant
/// (#529). Replaces the pre-#529 single-line summary that lied
/// about availability when an offered variant failed to download.
///
/// One of three deterministic lines per call, depending on the
/// `VariantStatus` discriminant:
///
/// - `NotOffered` → "Animated artwork: {variant} not offered by Apple Music" (info)
/// - `Downloaded { path, size_bytes }` → "Animated artwork: {variant} downloaded ({size} → {path})" (info)
/// - `DownloadFailed { reason, .. }` → "Animated artwork: {variant} download failed — {reason}" (warn-tagged)
///
/// `variant_label` is "square" or "portrait" (the user-facing
/// term). Filesize is rendered with a human readable suffix
/// (KB / MB) so the line reads cleanly in a log.
pub(crate) fn emit_artwork_variant_log(
    app: &tauri::AppHandle,
    dl_id: &str,
    variant_label: &'static str,
    status: &super::animated_artwork_service::VariantStatus,
) {
    use super::animated_artwork_service::VariantStatus;
    match status {
        VariantStatus::NotOffered => {
            emit_download_log(
                app,
                dl_id,
                &format!("Animated artwork: {variant_label} not offered by Apple Music"),
            );
        }
        VariantStatus::Downloaded { path, size_bytes } => {
            emit_download_log(
                app,
                dl_id,
                &format!(
                    "Animated artwork: {variant_label} downloaded ({} → {path})",
                    format_artwork_size(*size_bytes),
                ),
            );
        }
        VariantStatus::DownloadFailed { reason, .. } => {
            // Use the warn-severity emitter so the line is colour-
            // coded amber in the Activity Log (#793), matching the
            // existing convention for "the work was attempted but
            // didn't land on disk" outcomes.
            //
            // #961: append a geo-lock hint when the failure reason names
            // an HTTP 403 -- the classic symptom of an HLS URL minted for
            // a fallback storefront's region being fetched from outside
            // it. Empty string for any other reason (timeout, DNS, etc.)
            // so the line reads exactly as it did before this addition.
            emit_download_warn(
                app,
                dl_id,
                &format!(
                    "Animated artwork: {variant_label} download failed — {reason}{}",
                    artwork_geo_lock_hint(reason),
                ),
            );
        }
    }
}

/// Returns an actionable geo-lock hint when an animated-artwork download
/// failure `reason` names an HTTP 403 status, else an empty string (#961).
///
/// A 403 on an animated-artwork HLS fetch is the classic symptom of a
/// storefront-region mismatch: the m3u8/segment URLs Apple returns are
/// minted for the storefront that served the album metadata, and a
/// fallback-storefront lookup (see `AlbumMetadata::fallback_storefront`)
/// can hand back URLs scoped to a region the account/network isn't
/// authorised to fetch from.
///
/// Pure and side-effect-free so it's directly unit-testable.
pub(crate) fn artwork_geo_lock_hint(reason: &str) -> &'static str {
    if reason.contains("403") {
        " (this often means the artwork URL is geo-locked to a different storefront region — see the metadata warning above if one was logged)"
    } else {
        ""
    }
}

/// Renders an animated-artwork file size as a compact human string
/// (e.g. `"2.1 MB"`, `"512 KB"`). Used only by
/// `emit_artwork_variant_log` for the success-path line.
pub(crate) fn format_artwork_size(bytes: u64) -> String {
    // Binary units (1 KB = 1024 bytes).
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} bytes")
    }
}

/// Formats a stage-elapsed `Duration` as a compact human string for
/// heartbeat lines: `"2 min"`, `"1h 23 min"`, `"45 s"` (the last
/// shouldn't appear in practice because we skip the first tick, but
/// is included for completeness in case of unusual cadence overrides).
pub(crate) fn format_heartbeat_elapsed(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs < 60 {
        return format!("{total_secs} s");
    }
    let total_mins = total_secs / 60;
    if total_mins < 60 {
        return format!("{total_mins} min");
    }
    let hours = total_mins / 60;
    let mins = total_mins % 60;
    format!("{hours}h {mins} min")
}

/// Handle returned by [`spawn_companion_downloads`].
///
/// Wraps the spawned task's `JoinHandle` together with progress metadata
/// for timeout/advisory logging. The cooperative-cancel flag is retained
/// for the synchronous companion lyrics conversion checkpoints. The
/// completion watcher first emits a soft advisory, then uses this flag
/// only if the second hard deadline is also exceeded.
///
/// The optional `heartbeat` field owns the [`HeartbeatTicker`] spawned
/// for the companion phase (#805). When the supervisor task finishes
/// (success, failure, or abort) it drops `heartbeat` and the ticker
/// exits cleanly.
pub(crate) struct CompanionTaskHandle {
    pub(crate) handle: tokio::task::JoinHandle<()>,
    aborted: Arc<AtomicBool>,
    progress: Arc<StdMutex<CompanionTaskProgress>>,
    #[allow(dead_code)] // Held for its Drop; not directly read.
    heartbeat: Option<HeartbeatTicker>,
}

impl CompanionTaskHandle {
    /// Signals the companion task to stop processing at the next
    /// cooperative checkpoint *and* aborts the async runtime task.
    /// Also stops the heartbeat ticker (#805) so it doesn't continue
    /// chatting about a stage that the user has cancelled.
    pub(crate) fn abort(&mut self) {
        self.aborted.store(true, Ordering::Relaxed);
        self.handle.abort();
        if let Some(hb) = self.heartbeat.take() {
            hb.stop();
        }
    }
}

/// If a `CompanionTaskHandle` is ever dropped WITHOUT `abort()` having
/// been called first — the owning task panicked, was itself aborted, or
/// simply forgot the call on some exit path — the companion download
/// this handle was watching would otherwise keep running in the
/// background for an item that is already finished, or already marked
/// as failed. Nobody would be checking on it any more, and it could go
/// on for as long as GAMDL takes.
///
/// `abort()` is already safe to call more than once (setting a flag
/// that's already set, aborting an already-aborted task, and stopping
/// an already-stopped heartbeat are all no-ops), so `Drop` simply calls
/// it again as a safety net. This is the same shape as `ActiveSlotGuard`
/// elsewhere in this module: the normal exit path calls the cleanup
/// explicitly, and `Drop` only has work left to do if something skipped
/// that call.
impl Drop for CompanionTaskHandle {
    /// Runs `abort()` as a safety net; see the note on this `impl`.
    fn drop(&mut self) {
        self.abort();
    }
}

impl CompanionTaskHandle {
    /// A short plain sentence saying which companion tier is running and
    /// which have not started, for the timeout log lines. If the progress
    /// lock is poisoned it says so instead of failing.
    pub(crate) fn describe_pending(&self) -> String {
        self.progress
            .lock()
            .map(|progress| progress.describe_pending())
            .unwrap_or_else(|_| "pending companion state unavailable".to_string())
    }

    /// Number of companion tiers planned for this item. Used by the
    /// completion task to size its companion-phase timeout proportionally
    /// to the actual workload — a 4-tier "Atmos → all formats" item
    /// legitimately needs much more wall-clock time than a 1-tier
    /// "Atmos → Lossless" item.
    pub(crate) fn tier_count(&self) -> usize {
        self.progress
            .lock()
            .map(|progress| progress.planned_tiers.len())
            .unwrap_or(0)
    }
}

/// What the companion task has done so far, shared (behind a mutex) with
/// the completion task so a timeout message can name the unfinished tiers.
/// Tiers are numbered from 0, in the order `plan_companions` returned them.
#[derive(Default)]
pub(crate) struct CompanionTaskProgress {
    pub(crate) planned_tiers: Vec<String>,
    pub(crate) current_tier: Option<usize>,
    pub(crate) completed_tiers: HashSet<usize>,
    pub(crate) exhausted_tiers: HashSet<usize>,
}

impl CompanionTaskProgress {
    /// Builds the text for `CompanionTaskHandle::describe_pending`: the tier
    /// running now (if any), then the tiers not yet started (not current,
    /// completed or exhausted). Says so plainly when nothing is left.
    pub(crate) fn describe_pending(&self) -> String {
        let mut parts = Vec::new();

        if let Some(idx) = self.current_tier {
            if let Some(label) = self.planned_tiers.get(idx) {
                parts.push(format!("currently running tier {idx}: {label}"));
            }
        }

        let not_started: Vec<String> = self
            .planned_tiers
            .iter()
            .enumerate()
            .filter(|(idx, _)| {
                Some(*idx) != self.current_tier
                    && !self.completed_tiers.contains(idx)
                    && !self.exhausted_tiers.contains(idx)
            })
            .map(|(idx, label)| format!("tier {idx}: {label}"))
            .collect();
        if !not_started.is_empty() {
            parts.push(format!("not yet started: {}", not_started.join("; ")));
        }

        if parts.is_empty() {
            "no remaining companion tiers recorded".to_string()
        } else {
            parts.join("; ")
        }
    }
}

/// Used when the primary files may still have their plain names: every
/// tier gets a format suffix, and formats whose suffix is empty (standard
/// AAC — its copy would have exactly the primary's plain name) are taken
/// out. A tier left with no format stays in place (so tier numbers still
/// match the plan) and is passed over. Returns the formats taken out.
///
/// `primary_named_with_format`: the primary run's own file template
/// already carried a format suffix (single-format mode with extra copies),
/// so its files never had the plain name and a plain-named copy cannot
/// collide; nothing is taken out then.
pub(crate) fn force_suffixes_for_unrenamed_primary(
    tiers: &mut [CompanionTier],
    primary_named_with_format: bool,
) -> Vec<String> {
    let mut skipped = Vec::new();
    for tier in tiers.iter_mut() {
        tier.apply_suffix = true;
        if primary_named_with_format {
            continue;
        }
        tier.codecs_to_try.retain(|c| {
            let keep = codec_suffix(c).is_some_and(|s| !s.is_empty());
            if !keep {
                skipped.push(c.to_cli_string().to_string());
            }
            keep
        });
    }
    skipped
}

/// Holds a companion task back until enrichment says it may start
/// (`lyrics_pass::CompanionGate`), and gives it the download folder the
/// primary run used, to turn the real album folder into a template.
pub(crate) struct CompanionHold {
    /// The companion task's end of the gate.
    pub waiter: super::lyrics_pass::CompanionGateWaiter,
    /// The primary run's resolved download folder.
    pub download_root: Option<String>,
    /// What the final advisory pass may touch, filled by enrichment right
    /// before it opens the gate. The companion task reads it afterwards to
    /// find this download's new copies and give them their lyrics
    /// (`companion_lyrics.rs`).
    pub advisory_scope: Arc<StdMutex<Option<super::lyrics_pass::AdvisoryScope>>>,
    /// The main download's files whose lyrics steps had started but not
    /// finished when enrichment ended. `Some` only if enrichment stopped
    /// early; the companion task then finishes their lyrics itself
    /// (`companion_lyrics::run_main_files_own`).
    pub pending_main_lyrics: Arc<StdMutex<Option<Vec<std::path::PathBuf>>>>,
    /// The writing jobs enrichment may have left running when it was
    /// abandoned; the catch-up waits for them to end
    /// (`lyrics_steps::wait_to_catch_up`).
    pub in_flight_writes: super::lyrics_steps::InFlightWrites,
}

/// Plans and starts the companion downloads for one finished primary
/// download: extra GAMDL runs that fetch the same content in other formats
/// (see `plan_companions` and the `CompanionMode` setting). When extra
/// lyrics formats are chosen in settings, it also starts one small
/// fire-and-forget GAMDL run per format for the lyrics files only.
///
/// Returns `None` when there is no format task to wait for: the item is a
/// Spotify link (companions are a GAMDL feature), or no format tiers remain
/// after the plan and the Apple Music `audioTraits` filter
/// (`available_audio_traits`). Lyrics-only runs may still have started in
/// that second case; nothing tracks them. Otherwise returns a
/// [`CompanionTaskHandle`] the completion task uses to wait, time out or
/// abort. The queue item stays in `Processing` until the task ends.
///
/// `force_all_suffixes` makes every tier use a labelled file name; it is
/// needed when the primary's real format was not known in advance, so the
/// primary's plain names cannot collide with a companion's. `held_until`
/// (see [`CompanionHold`]) keeps the task waiting until enrichment has
/// finished renaming the primary's files. `primary` describes how the
/// primary's files were named, so attempts never reuse its names.
///
/// This function itself does no downloading: the work happens in the task
/// it spawns, which checks `shutdown` and its own abort flag between steps.
///
/// See:
/// https://docs.rs/tokio/latest/tokio/task/fn.spawn.html
// The argument list is long because this function is the seam between
// the queue's per-download state (app, queue, dl_id, urls, shutdown),
// the GAMDL invocation context (primary codec, base options,
// force_all_suffixes), and the audioTraits pre-filter from #504. All
// of them are needed inside the spawned task and a struct wrapper
// would just move the repetition to the three call sites. Suppress
// the clippy nit rather than introduce indirection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_companion_downloads(
    app: &tauri::AppHandle,
    queue: &QueueHandle,
    dl_id: &str,
    urls: &[String],
    primary_codec_str: &str,
    companion_base_options: &GamdlOptions,
    shutdown: &ShutdownSignal,
    force_all_suffixes: bool,
    available_audio_traits: &[String],
    held_until: Option<CompanionHold>,
    primary: PrimaryNaming,
) -> Option<CompanionTaskHandle> {
    // M9-7 belt-and-braces: companion downloads use GAMDL's
    // codec-tier model (Atmos → ALAC → AAC fan-out) which has no
    // votify equivalent. votify uses a single `--audio-quality` CSV
    // priority — no tier loop, no `force_all_suffixes`, no
    // `available_audio_traits` filter. Callers at 8119 / 10243 /
    // 10999 are all on the GAMDL post-primary path, but a Spotify
    // item reaching here via a future code path would crash GAMDL
    // with the unsupported URL host. Skip cleanly.
    if urls
        .iter()
        .any(|u| u.contains("open.spotify.com") || u.starts_with("spotify:"))
    {
        log::warn!("spawn_companion_downloads called with Spotify URL(s) — skipping (M9-7 guard)");
        emit_download_log(
            app,
            dl_id,
            "Companion downloads skipped — not supported for Spotify items \
             (votify uses --audio-quality priority instead of tier-and-codec)",
        );
        return None;
    }

    let companion_settings = load_settings_for_queue(app);
    let raw_tiers = plan_companions(
        &companion_settings.companion_mode,
        primary_codec_str,
        &companion_settings.custom_companion_codecs,
    );

    // Drop tiers whose codec the Apple Music API has already told us
    // isn't offered for this track (#504). When no traits are available
    // the filter is a no-op and we fall through to GAMDL as before.
    let (mut companion_tiers, skipped) =
        filter_tiers_by_audio_traits(raw_tiers, available_audio_traits);
    for codec_list in &skipped {
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Companion skipped: {codec_list} not available for this track on Apple Music \
                 (audioTraits: [{}])",
                available_audio_traits.join(", ")
            ),
        );
    }

    // When native priority was used, the primary download has clean filenames
    // (because we don't know the actual codec until GAMDL finishes). Force ALL
    // companion tiers to use suffixed filenames to prevent collisions between
    // the primary's clean filenames and the "most compatible" companion tier
    // that would normally also get clean filenames.
    if force_all_suffixes {
        for tier in &mut companion_tiers {
            tier.apply_suffix = true;
        }
    }

    let codec_handle = if companion_tiers.is_empty() {
        // Nothing to hold back; dropping the hold is harmless.
        drop(held_until);
        None
    } else {
        let comp_app = app.clone();
        let comp_queue = queue.clone();
        let comp_urls = urls.to_vec();
        // The caller has already removed any suffix MeedyaDL added to
        // these templates (`strip_recorded_codec_suffixes`).
        let mut comp_base_opts = companion_base_options.clone();
        let mut companion_tiers = companion_tiers;
        let comp_dl_id = dl_id.to_string();
        let comp_shutdown = shutdown.clone();
        // Per-task cooperative-cancel flag (#663). The completion task
        // sets this to `true` before calling `handle.abort()` so the
        // synchronous extra-copy lyrics step and the tier
        // loop can bail out at their next loop boundary instead of
        // emitting activity-log events for many minutes after abort.
        let comp_aborted = Arc::new(AtomicBool::new(false));
        let aborted_for_task = comp_aborted.clone();
        let companion_progress = Arc::new(StdMutex::new(CompanionTaskProgress {
            planned_tiers: companion_tiers
                .iter()
                .map(|tier| {
                    let codec_names: Vec<&str> = tier
                        .codecs_to_try
                        .iter()
                        .map(SongCodec::to_cli_string)
                        .collect();
                    codec_names.join(", ")
                })
                .collect(),
            ..Default::default()
        }));
        let progress_for_task = companion_progress.clone();

        emit_download_log(
            app,
            dl_id,
            &format!(
                "──── Companion downloads (mode: {:?}) ────",
                companion_settings.companion_mode,
            ),
        );
        // Log per-tier codec details before spawning the async task
        for (tier_idx, tier) in companion_tiers.iter().enumerate() {
            let codec_names: Vec<&str> = tier
                .codecs_to_try
                .iter()
                .map(SongCodec::to_cli_string)
                .collect();
            emit_download_log(
                app,
                dl_id,
                &format!(
                    "Companion tier {}: trying {}",
                    tier_idx,
                    codec_names.join(", ")
                ),
            );
        }

        let handle = tokio::spawn(async move {
            // Held until enrichment has finished its lyrics steps and its
            // renames (16 Nov 2026), so nothing lands beside the primary
            // files while their names may still change. Cancel, app
            // shutdown and the completion task's abort still end the wait.
            // Every suffix and format the primary's files carry, for the
            // guard before each attempt. The template's suffix is known now;
            // the formats detected in the files (and the suffixes enrichment
            // renamed them with) arrive with the gate.
            let mut primary_codecs: Vec<SongCodec> = Vec::new();
            let mut primary_tracks: Vec<(SongCodec, u32, u32)> = Vec::new();
            let mut album_tracks: Vec<(u32, u32, String)> = Vec::new();
            let mut companion_album_dir: Option<std::path::PathBuf> = None;
            let mut gate_album_name: Option<String> = None;
            let mut unplaceable: Vec<SongCodec> = Vec::new();
            let mut lyricsfile_artist: Option<String> = None;
            let mut lyricsfile_default_title: Option<String> = None;
            // Where enrichment recorded this download's files, and when the
            // gate opened (to tell a lyrics file written by this run from an
            // older one). Both stay `None` on the failure path, which has no
            // gate; the lyrics step then builds its own scope (below).
            let mut lyrics_scope_handle: Option<
                Arc<StdMutex<Option<super::lyrics_pass::AdvisoryScope>>>,
            > = None;
            // Main-download files whose lyrics steps enrichment began but did
            // not finish (see `CompanionHold::pending_main_lyrics`).
            let mut pending_main_handle: Option<Arc<StdMutex<Option<Vec<std::path::PathBuf>>>>> =
                None;
            let mut in_flight_handle: Option<super::lyrics_steps::InFlightWrites> = None;
            let mut gate_opened_at: Option<std::time::SystemTime> = None;
            if let Some(hold) = held_until {
                lyrics_scope_handle = Some(hold.advisory_scope.clone());
                pending_main_handle = Some(hold.pending_main_lyrics.clone());
                in_flight_handle = Some(hold.in_flight_writes.clone());
                let mut waiter = hold.waiter;
                let state = super::lyrics_pass::wait_for_companion_gate(&mut waiter, || {
                    let queue = comp_queue.clone();
                    let id = comp_dl_id.clone();
                    let stopping =
                        comp_shutdown.is_triggered() || aborted_for_task.load(Ordering::Relaxed);
                    async move { stopping || !queue.lock().await.should_keep_working_on(&id) }
                })
                .await;
                let Some(state) = state else {
                    log::info!(
                        "Companion downloads for {comp_dl_id} not started (stopped while waiting)"
                    );
                    return;
                };
                gate_opened_at = Some(std::time::SystemTime::now());
                primary_codecs.clone_from(&state.primary_codecs);
                primary_tracks.clone_from(&state.primary_tracks);
                album_tracks.clone_from(&state.album_tracks);
                companion_album_dir = state.album_dir.as_deref().map(std::path::PathBuf::from);
                gate_album_name.clone_from(&state.album_name);
                unplaceable.clone_from(&state.unplaceable);
                lyricsfile_artist.clone_from(&state.lyricsfile_artist);
                lyricsfile_default_title.clone_from(&state.lyricsfile_default_title);
                if !state.renames_done {
                    // The primary files may still have their plain names
                    // (enrichment ended early, or the format could not be
                    // told): give every extra copy a format suffix, and
                    // leave out any format whose suffix is empty (standard
                    // AAC), so none can land on a primary file's name.
                    let skipped = force_suffixes_for_unrenamed_primary(
                        &mut companion_tiers,
                        primary.appended_suffix.is_some(),
                    );
                    if !skipped.is_empty() {
                        emit_download_log(
                            &comp_app,
                            &comp_dl_id,
                            &format!(
                                "Extra format skipped so it cannot replace the main download's files: {}",
                                skipped.join(", ")
                            ),
                        );
                    }
                }
                // Write into the folder that really holds the primary
                // files (#528), whatever GAMDL's template would call it.
                // Only when the companion saves under the same download
                // folder the primary run used.
                if let (Some(dir), Some(root)) =
                    (state.album_dir.as_deref(), hold.download_root.as_deref())
                {
                    let own_root = comp_base_opts
                        .output_path
                        .as_deref()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or(root);
                    if own_root == root
                        && super::lyrics_pass::file_templates_allow_fixed_folder(&comp_base_opts)
                    {
                        if let Some(template) = super::lyrics_pass::literal_folder_template(
                            std::path::Path::new(dir),
                            std::path::Path::new(root),
                        ) {
                            comp_base_opts.album_folder_template = Some(template.clone());
                            comp_base_opts.compilation_folder_template = Some(template);
                            comp_base_opts.output_path = Some(root.to_string());
                        }
                    }
                }
                emit_download_log(
                    &comp_app,
                    &comp_dl_id,
                    "Starting the extra format downloads",
                );
            }

            // When enrichment never recorded a scope (the gate was dropped, or
            // the failure path), the lyrics step cannot know which files are
            // new copies. So, before any extra copy is downloaded, note what
            // is in the album folder now and when the downloads started; the
            // lyrics step then treats only files that appear after this as
            // this download's copies (#1441).
            let tiers_started_at = std::time::SystemTime::now();
            let scope_recorded = lyrics_scope_handle
                .as_ref()
                .is_some_and(|h| h.lock().is_ok_and(|g| g.is_some()));
            let folder_before: Option<super::companion_lyrics::FolderSnapshot> = if scope_recorded {
                None
            } else if let Some(output_dir) = comp_base_opts.output_path.as_deref() {
                let (artist_hint, album_hint) = {
                    let q = comp_queue.lock().await;
                    q.items
                        .iter()
                        .find(|i| i.status.id == comp_dl_id)
                        .map(|i| (i.status.artist_name.clone(), i.status.album_name.clone()))
                        .unwrap_or((None, None))
                };
                super::companion_lyrics::resolve_album_folder(
                    std::path::Path::new(output_dir),
                    companion_album_dir.as_deref(),
                    artist_hint.as_deref(),
                    album_hint.as_deref(),
                )
                .map(|dir| super::companion_lyrics::FolderSnapshot::take(&dir))
            } else {
                None
            };

            // `any_tier_produced_files` is set the first time a tier actually
            // writes new audio files to disk (#843). After the tier loop we
            // run the companion lyrics conversion exactly once if the flag is
            // set — moving it out of the loop fixes the duplicated walk-the-
            // library symptom from v1.8.x where multi-tier items (e.g. Atmos
            // tier 2 + AAC-Legacy tier 4 both succeeding) re-converted the
            // same TTML files once per successful tier.
            let mut any_tier_produced_files = false;

            // Process each companion tier sequentially
            for (tier_idx, tier) in companion_tiers.iter().enumerate() {
                // Check for app shutdown between tiers
                if comp_shutdown.is_triggered() {
                    log::info!("Companion downloads stopping early (app shutting down)");
                    return;
                }
                // Has the person cancelled this item?
                //
                // Until a full review of the codebase found this, nothing
                // after the first download ever asked. Cancel marked the
                // item cancelled on screen and stopped the download that
                // was running — and then the extra copies in other
                // formats carried on, sometimes for many minutes, still
                // writing files and still holding the queue's one slot,
                // so the next item sat waiting behind work somebody had
                // already asked to stop.
                //
                // Checked between tiers, which is where the other two
                // stop conditions are checked, and for the same reason:
                // it is the point where nothing is half-done.
                if !comp_queue.lock().await.should_keep_working_on(&comp_dl_id) {
                    log::info!("Companion downloads stopping early — {comp_dl_id} was cancelled");
                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        "Cancelled — stopping the extra format downloads.",
                    );
                    return;
                }
                // Cooperative-cancel check (#663). If the completion
                // task fired the deadline timeout while we were in
                // sync code, leave before launching another GAMDL.
                if aborted_for_task.load(Ordering::Relaxed) {
                    let pending = progress_for_task
                        .lock()
                        .map(|progress| progress.describe_pending())
                        .unwrap_or_else(|_| "pending companion state unavailable".to_string());
                    log::info!(
                        "Companion downloads aborted via cooperative flag for {comp_dl_id} — \
                         skipping remaining tiers: {pending}"
                    );
                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!(
                            "Companion task aborted — skipping remaining companions: {pending}"
                        ),
                    );
                    return;
                }

                // A tier emptied by `force_suffixes_for_unrenamed_primary`.
                if tier.codecs_to_try.is_empty() {
                    continue;
                }
                let mut tier_succeeded = false;
                if let Ok(mut progress) = progress_for_task.lock() {
                    progress.current_tier = Some(tier_idx);
                }

                // Songs found with a copy in an earlier format of this tier.
                let mut tier_covered: Vec<(u32, u32)> = Vec::new();
                // Try each codec in the tier until one succeeds
                for codec in &tier.codecs_to_try {
                    // Phase 3.5g: surface the active companion stage to the
                    // per-item progress bar. Pre-3.5d this was impossible
                    // (set_label was a closure local to the enrichment task);
                    // now we use the shared `set_stage_with_label` helper.
                    // Companion downloads happen AFTER enrichment finishes,
                    // so the bar would otherwise still display "Finalising
                    // metadata…" while companion GAMDL is the actual work.
                    set_stage_with_label(
                        &comp_app,
                        &comp_queue,
                        &comp_dl_id,
                        ProgressStage::Finalising,
                        &format!(
                            "Companion: downloading {} (tier {})…",
                            codec.to_cli_string(),
                            tier_idx
                        ),
                    );
                    // Asked again before EVERY attempt, not only between
                    // tiers. A tier can hold several formats and try each
                    // in turn, so checking only at the tier boundary
                    // meant a cancellation during the first attempt still
                    // let the next one start — fresh download work, begun
                    // after the person said stop. A reviewer caught the
                    // gap in the first version of this check.
                    if !comp_queue.lock().await.should_keep_working_on(&comp_dl_id) {
                        log::info!("Companion downloads stopping — {comp_dl_id} was cancelled");
                        emit_download_log(
                            &comp_app,
                            &comp_dl_id,
                            "Cancelled — stopping the extra format downloads.",
                        );
                        return;
                    }

                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!(
                            "Companion tier {}: attempting {}...",
                            tier_idx,
                            codec.to_cli_string()
                        ),
                    );

                    let mut opts = comp_base_opts.clone();
                    // Use --song-codec-priority with a single codec value.
                    // GAMDL >= 2.9.1 removed the --song-codec flag entirely;
                    // passing a single-element priority achieves the same effect.
                    opts.song_codec = None;
                    opts.song_codec_priority = Some(codec.to_runtime_cli_string().to_string());

                    // Final guard: this copy's file names must never be the
                    // primary's (same suffix in the same folder would
                    // overwrite it, or with overwrite off silently not
                    // save the copy).
                    let plan = match companion_attempt_naming(
                        codec,
                        tier.apply_suffix,
                        &opts,
                        &primary,
                        &primary_codecs,
                    ) {
                        Ok(plan) => plan,
                        Err(why) => {
                            let reason = match why {
                                CompanionSkip::SameFormat => {
                                    "the main download is already in that format"
                                }
                                CompanionSkip::SameNames => {
                                    "its files would have had the same names as the main download"
                                }
                            };
                            emit_download_log(
                                &comp_app,
                                &comp_dl_id,
                                &format!(
                                    "An extra copy in {} was skipped because {reason}.",
                                    plain_codec_name(codec)
                                ),
                            );
                            continue;
                        }
                    };
                    // If this tier needs a suffix (e.g., ALAC
                    // companion in AtmosToLosslessAndLossy mode
                    // gets [Lossless]), apply it to the options.
                    if plan.apply_suffix {
                        apply_codec_suffix(&mut opts);
                    }
                    // Copies in this format already in the album folder from
                    // an earlier run — possibly carrying an advisory label
                    // GAMDL's own name check does not know about. Only for a
                    // suffixed attempt; a plain one (standard AAC) relies on
                    // GAMDL's check.
                    let existing_copies: super::lyrics_pass::ExistingCopies = match (
                        companion_album_dir.as_deref(),
                        codec_suffix(codec).filter(|s| plan.apply_suffix && !s.is_empty()),
                    ) {
                        (Some(dir), Some(suffix)) => super::lyrics_pass::tracks_with_existing_copy(
                            dir,
                            suffix,
                            super::lyrics_pass::read_copy_evidence,
                            &album_tracks,
                            gate_album_name.as_deref(),
                        ),
                        _ => super::lyrics_pass::ExistingCopies::default(),
                    };
                    // Songs with a copy in an EARLIER format of this tier
                    // (found before that attempt): not asked for again in
                    // this one, so a fallback format never adds a second
                    // copy of a song that already has one.
                    let covered_earlier = tier_covered.clone();
                    for t in &existing_copies.tracks {
                        if !tier_covered.contains(t) {
                            tier_covered.push(*t);
                        }
                    }
                    // A mixed album (see `AttemptPlan::only_other_tracks`),
                    // or copies already there: ask only for the rest.
                    let attempt_urls: Vec<String> = match attempt_links(
                        codec,
                        plan.only_other_tracks,
                        &comp_urls,
                        &primary_tracks,
                        &album_tracks,
                        &unplaceable,
                        &existing_copies.tracks,
                        existing_copies.labelled,
                        &covered_earlier,
                    ) {
                        AttemptLinks::All => comp_urls.clone(),
                        AttemptLinks::Skip(reason) => {
                            emit_download_log(&comp_app, &comp_dl_id, &reason);
                            continue;
                        }
                        AttemptLinks::Only {
                            links,
                            unmapped,
                            unreadable,
                            already_there: present,
                            details_available,
                        } => {
                            if let Some(line) = already_there_line(codec, present) {
                                emit_download_log(&comp_app, &comp_dl_id, &line);
                            }
                            // Every song that can be asked for already has its
                            // copy: the tier is done (songs that could not be
                            // asked for are reported below, and a different
                            // format is not tried just because of them).
                            if links.is_empty() && present > 0 {
                                for line in mixed_album_lines(
                                    codec,
                                    1, // not "nothing was requested": copies are there
                                    unmapped,
                                    unreadable,
                                    details_available,
                                ) {
                                    emit_download_log(&comp_app, &comp_dl_id, &line);
                                }
                                tier_succeeded = true;
                                if let Ok(mut progress) = progress_for_task.lock() {
                                    progress.completed_tiers.insert(tier_idx);
                                    progress.current_tier = None;
                                }
                                break;
                            }
                            for line in mixed_album_lines(
                                codec,
                                links.len(),
                                unmapped,
                                unreadable,
                                details_available,
                            ) {
                                emit_download_log(&comp_app, &comp_dl_id, &line);
                            }
                            if links.is_empty() {
                                continue;
                            }
                            links
                        }
                    };
                    let requested_songs = (attempt_urls != comp_urls).then_some(attempt_urls.len());
                    // How many songs this attempt asks for: the song links, or
                    // for the whole album this download's songs not already
                    // in this format (0 when unknown).
                    let expected_songs = requested_songs
                        .unwrap_or_else(|| songs_wanted_in(codec, &primary_tracks, &unplaceable));
                    // An extra-format copy only ever ADDS files: it never
                    // replaces one, whatever the person's overwrite setting
                    // says. A file of the same name may be the main
                    // download's (enriched) file, another album's, or an
                    // earlier download's — none of which a copy may destroy.
                    // The trade-off: re-downloading does not refresh extra
                    // copies already on disk. Each skip of that kind is
                    // counted from GAMDL's output and reported in one line.
                    opts.overwrite = Some(false);
                    let already_there = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    // Songs GAMDL said are not offered in this format.
                    let not_offered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    // If not suffixed, the base options already
                    // have clean (unsuffixed) templates.

                    // Build the GAMDL CLI command for the companion
                    let mut cmd = match gamdl_service::build_gamdl_command_public(
                        &comp_app,
                        &attempt_urls,
                        &opts,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            log::debug!(
                                "Companion tier {}: failed to build \
                                 command ({}) for {}: {}",
                                tier_idx,
                                codec.to_cli_string(),
                                comp_dl_id,
                                e
                            );
                            continue; // Try next codec in tier
                        }
                    };

                    // Log the GAMDL CLI args so users can verify the codec request
                    emit_verbose_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!(
                            "Companion GAMDL args: --song-codec-priority {}",
                            codec.to_cli_string()
                        ),
                    );

                    // Pipe stdout/stderr and stream line-by-line to the activity log.
                    // This gives users real-time visibility into companion downloads
                    // (per-track progress, [download] fragments, errors).
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::piped());

                    let stream_codec = codec.to_cli_string().to_string();

                    // Hand the child off to the companion supervisor:
                    //   - kill_on_drop(true) so an aborted task reaps GAMDL (#501)
                    //   - parses `Finished with N error(s)` to detect soft errors (#500)
                    //   - watches for stdout/stderr silence and kills the child after
                    //     `gamdl_idle_timeout_minutes` of inactivity (#505), pausing the
                    //     watchdog once a `100% of` line marks the post-processing phase (#503)
                    let supervisor_app = comp_app.clone();
                    let supervisor_dl = comp_dl_id.clone();
                    let label_for_emit = format!("companion-tier-{tier_idx}");
                    let companion_settings = load_settings_for_queue(&comp_app);
                    // Same rule as the main download (at least 30 minutes
                    // for GAMDL 4.x, which is silent while it downloads):
                    // see `gamdl_capabilities::idle_minutes_for`.
                    let companion_idle_minutes =
                        crate::services::gamdl_capabilities::idle_minutes_for_installed(
                            companion_settings.gamdl_idle_timeout_minutes,
                        );
                    let idle_timeout =
                        std::time::Duration::from_secs(u64::from(companion_idle_minutes) * 60);
                    // Track-aware companion caption (#799). The
                    // per-tier label set at tier start
                    // ("Companion: downloading atmos (tier 2)…") is
                    // overwritten on each `TrackInfo` event with the
                    // current track name + counter, so the top
                    // progress bar tells the user *what track* is
                    // running — not just the codec and tier.
                    //
                    // `parse_gamdl_output` is called twice for each
                    // line (once inside `emit_companion_stream_line`
                    // and once here) — cheap single-line parses,
                    // worth it to avoid changing the LineEmitter
                    // signature shared with the MV companion path.
                    let tier_label_codec = codec.to_cli_string().to_string();
                    let tier_idx_for_label = tier_idx;
                    let queue_for_label = comp_queue.clone();
                    let app_for_label = comp_app.clone();
                    let dl_for_label = comp_dl_id.clone();
                    let already_there_counter = already_there.clone();
                    let not_offered_counter = not_offered.clone();
                    let emitter: super::companion_supervisor::LineEmitter =
                        std::sync::Arc::new(move |app, dl, stream, line| {
                            if is_already_there_skip_line(&line) {
                                already_there_counter.fetch_add(1, Ordering::Relaxed);
                            } else if process::is_format_not_offered_line(&line) {
                                not_offered_counter.fetch_add(1, Ordering::Relaxed);
                            }
                            // Captured-by-value clones for the per-line update path.
                            let codec_name = tier_label_codec.clone();
                            let tier_n = tier_idx_for_label;
                            let queue_clone = queue_for_label.clone();
                            let app_clone_inner = app_for_label.clone();
                            let dl_clone_inner = dl_for_label.clone();
                            Box::pin(async move {
                                let kind = if stream.contains("stderr") {
                                    "stderr"
                                } else {
                                    "stdout"
                                };
                                // #799: update the per-item caption on every TrackInfo.
                                update_companion_label_from_line(
                                    &app_clone_inner,
                                    &queue_clone,
                                    &dl_clone_inner,
                                    tier_n,
                                    &codec_name,
                                    &line,
                                )
                                .await;
                                emit_companion_stream_line(&app, &dl, kind, &line).await
                            })
                        });

                    // Phase 3.5h: snapshot the audio-file count BEFORE running
                    // companion GAMDL. After a "successful" exit (GAMDL exits 0
                    // even when it skipped every track because the requested
                    // format wasn't available — `Skipping … format is not
                    // available` is a warning, not an error in GAMDL's view),
                    // we compare against this snapshot to detect the false-
                    // positive "complete" case the user flagged on 2026-05-08:
                    // companion AC3 ran on a track Apple Music doesn't ship in
                    // AC3, GAMDL produced 0 files, but the activity log said
                    // "Companion download complete (tier 0, codec: ac3)".
                    //
                    // The list itself (not just its length) is kept: the tier's
                    // format tags are written only to the files this tier adds
                    // (`tier_new_m4a`), never to the main download's files or to
                    // older ones. Same walk and same filter as
                    // `count_audio_files_in_directory`, so the count is unchanged.
                    let before_tier: std::collections::BTreeSet<std::path::PathBuf> = opts
                        .output_path
                        .as_deref()
                        .map(std::path::Path::new)
                        .map(super::lyrics_pass::audio_files_in)
                        .unwrap_or_default();
                    let pre_run_audio_count = before_tier.len();

                    let run_result = super::companion_supervisor::run_supervised(
                        &supervisor_app,
                        &supervisor_dl,
                        &label_for_emit,
                        cmd,
                        idle_timeout,
                        emitter,
                    )
                    .await;
                    if let Some(line) =
                        already_there_line(codec, already_there.load(Ordering::Relaxed))
                    {
                        emit_download_log(&comp_app, &comp_dl_id, &line);
                    }

                    // Surface the post-processing transition to the queue UI so
                    // the per-item bar switches from `DOWNLOADING…` to
                    // `PROCESSING (remux / decrypt)…` instead of looking frozen
                    // while mp4decrypt / ffmpeg / mp4box run silently (#503).
                    if let Ok(ref r) = run_result {
                        if r.reached_post_processing {
                            let mut q = comp_queue.lock().await;
                            q.set_processing_label(
                                &supervisor_dl,
                                &format!(
                                    "Post-processing companion ({}): remux / decrypt",
                                    stream_codec
                                ),
                            );
                        }
                    }

                    match run_result {
                        Ok(r) if r.exit_success && !r.had_soft_error => {
                            // Phase 3.5h: verify files actually landed before
                            // claiming "complete". GAMDL exits 0 with 0 errors
                            // when it skips every track due to format
                            // unavailability ("Skipping … format is not
                            // available" is a warning, not an error). Without
                            // this check, the companion task records a false
                            // success — confusing for the user when checking
                            // history / diagnosing missing companions.
                            let after_tier: std::collections::BTreeSet<std::path::PathBuf> = opts
                                .output_path
                                .as_deref()
                                .map(std::path::Path::new)
                                .map(super::lyrics_pass::audio_files_in)
                                .unwrap_or_default();
                            let post_run_audio_count = after_tier.len();
                            if post_run_audio_count <= pre_run_audio_count
                                && tier_done_without_new_files(
                                    already_there.load(Ordering::Relaxed),
                                    not_offered.load(Ordering::Relaxed),
                                    expected_songs,
                                )
                            {
                                let skipped = not_offered.load(Ordering::Relaxed);
                                if skipped > 0 {
                                    emit_download_log(
                                        &comp_app,
                                        &comp_dl_id,
                                        &format!(
                                            "{skipped} song(s) are not offered in {}.",
                                            plain_codec_name(codec)
                                        ),
                                    );
                                }
                                // No new files, but GAMDL said the copies were
                                // already there: the tier is satisfied (the
                                // "already there" line has been written).
                                {
                                    let mut q = comp_queue.lock().await;
                                    q.clear_processing_label(&supervisor_dl);
                                }
                                tier_succeeded = true;
                                if let Ok(mut progress) = progress_for_task.lock() {
                                    progress.completed_tiers.insert(tier_idx);
                                    progress.current_tier = None;
                                }
                                break;
                            }
                            if post_run_audio_count <= pre_run_audio_count {
                                // No new files landed — the codec wasn't
                                // available for any track in this URL set.
                                emit_download_log(
                                    &comp_app,
                                    &comp_dl_id,
                                    &match requested_songs {
                                        Some(n) => format!(
                                            "Companion (tier {}, codec: {}): no compatible format available for the {n} song(s) requested — skipped (no files produced)",
                                            tier_idx, stream_codec,
                                        ),
                                        None => format!(
                                            "Companion (tier {}, codec: {}): no compatible format available — skipped (no files produced)",
                                            tier_idx, stream_codec,
                                        ),
                                    },
                                );
                                // Do NOT mark this tier as succeeded; let the
                                // next codec in the tier (if any) be tried.
                                // We still treat this as a non-failure
                                // (companion exited cleanly), so we don't
                                // emit `companion-downloaded` either.
                                // Reset post-processing label since there's
                                // nothing to post-process.
                                {
                                    let mut q = comp_queue.lock().await;
                                    q.clear_processing_label(&supervisor_dl);
                                }
                                continue;
                            }

                            emit_download_log(
                                &comp_app,
                                &comp_dl_id,
                                &format!(
                                    "Companion download complete (tier {}, codec: {}, {} new file(s))",
                                    tier_idx,
                                    stream_codec,
                                    post_run_audio_count - pre_run_audio_count,
                                ),
                            );
                            let _ = comp_app.emit("companion-downloaded", &comp_dl_id);

                            if let Some(ref output_dir) = opts.output_path {
                                // Rename cover art per user setting (#448)
                                let comp_settings = load_settings_for_queue(&comp_app);
                                rename_cover_art(
                                    output_dir,
                                    comp_settings.cover_art_name.to_filename_stem(),
                                );

                                // Scope the lyrics conversion AND the tag pass
                                // (#816) to the album we just produced —
                                // falls back to a recursive walk over the
                                // whole output root only when the artist/
                                // album hints are unavailable (#502).
                                let (artist_hint, album_hint) = {
                                    let q = comp_queue.lock().await;
                                    q.items
                                        .iter()
                                        .find(|i| i.status.id == comp_dl_id)
                                        .map(|i| {
                                            (
                                                i.status.artist_name.clone(),
                                                i.status.album_name.clone(),
                                            )
                                        })
                                        .unwrap_or((None, None))
                                };

                                // **#816 fix**: the codec-metadata tagger
                                // walks every M4A under the path it's given.
                                // Pre-#816, we passed `output_dir` (the
                                // user's whole output root, e.g.
                                // `/Volumes/DriveC/[MeedyaDL]/[AppleMusic]`)
                                // which made the tagger walk the user's
                                // ENTIRE library on every tier completion —
                                // 8000+ files tagged per tier × 4 tiers ×
                                // 220 queue items = ~18-36 hours of wasted
                                // tag-write work per queue run, AND silently
                                // overwriting hand-edited tags on
                                // previously-completed items. Now scope to
                                // the current item's resolved album dir via
                                // the same hints the lyrics conversion uses.
                                let tag_pass_target = find_album_directory(
                                    std::path::Path::new(output_dir),
                                    artist_hint.as_deref(),
                                    album_hint.as_deref(),
                                )
                                .unwrap_or_else(|| output_dir.clone());
                                // **Only the files THIS tier added** get its format
                                // tags (#1442). The folder-wide pass used to write
                                // this tier's tags (for example "lossless") into the
                                // main download's files, earlier copies and older
                                // downloads in the same folder. When the album
                                // folder was found, files outside it are left out
                                // too (another download's files can appear while
                                // this tier runs). Each file is tagged under the
                                // app-wide write lock; the tagging function itself
                                // takes no lock.
                                let album_dir = std::path::Path::new(&tag_pass_target);
                                let narrowed = album_dir.is_dir() && tag_pass_target != *output_dir;
                                let tagged = tag_tier_additions(
                                    &before_tier,
                                    &after_tier,
                                    narrowed.then_some(album_dir),
                                    codec,
                                    &comp_dl_id,
                                )
                                .await;
                                if tagged > 0 {
                                    log::info!(
                                        "Tagged {} companion file(s) with {} metadata in {} for {}",
                                        tagged,
                                        stream_codec,
                                        tag_pass_target,
                                        comp_dl_id
                                    );
                                }

                                // #843: lyrics conversion was previously called
                                // here. It now runs ONCE after the tier loop
                                // completes (TTML files don't change between
                                // tiers — companions inherit them from the
                                // primary download), so multi-tier items
                                // don't pay the conversion cost N times.
                                any_tier_produced_files = true;
                            }

                            // Clear the post-processing label now that we're done.
                            {
                                let mut q = comp_queue.lock().await;
                                q.clear_processing_label(&supervisor_dl);
                            }

                            tier_succeeded = true;
                            if let Ok(mut progress) = progress_for_task.lock() {
                                progress.completed_tiers.insert(tier_idx);
                                progress.current_tier = None;
                            }
                            break;
                        }
                        Ok(r) => {
                            // Build the most informative failure message we can:
                            //   - friendly_error wins (translated traceback, #500)
                            //   - then idle_killed (#505)
                            //   - then had_soft_error notes the GAMDL summary (#500)
                            //   - last resort: whatever stderr ended on
                            let detail = if let Some(msg) = r.friendly_error {
                                msg
                            } else if r.idle_killed {
                                format!(
                                    "GAMDL was idle for {companion_idle_minutes} min — terminated by watchdog"
                                )
                            } else if r.had_soft_error {
                                "GAMDL exited 0 but reported per-track errors — treating as failure"
                                    .to_string()
                            } else if r.last_stderr_line.is_empty() {
                                "unknown error".to_string()
                            } else {
                                r.last_stderr_line.clone()
                            };
                            emit_download_log(
                                &comp_app,
                                &comp_dl_id,
                                &format!(
                                    "Companion tier {}: {} failed — {}",
                                    tier_idx, stream_codec, detail
                                ),
                            );
                            // Reset post-processing label on failure too.
                            {
                                let mut q = comp_queue.lock().await;
                                q.clear_processing_label(&supervisor_dl);
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to spawn companion ({stream_codec}): {e}");
                        }
                    }
                }

                if !tier_succeeded {
                    if let Ok(mut progress) = progress_for_task.lock() {
                        progress.exhausted_tiers.insert(tier_idx);
                        progress.current_tier = None;
                    }
                    log::debug!("Companion tier {tier_idx} exhausted all codecs for {comp_dl_id}");
                    emit_download_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!("Companion tier {tier_idx}: all codecs exhausted"),
                    );
                }
            }

            // #843: run the companion lyrics conversion ONCE for the whole
            // item, after every tier has finished. Pre-fix, this ran inside
            // the tier-success branch — multi-tier items (e.g. Atmos tier 2
            // + AAC-Legacy tier 4 both succeeding) re-walked the user's
            // library once per successful tier, doubling/tripling wasted
            // work on big libraries. TTML files don't change between tiers,
            // so converting them once is correct.
            // Finish the main download's own lyrics first, if enrichment
            // stopped before it was done with them (its deadline, an early
            // return). This does not depend on an extra copy having been
            // made: the main files are unfinished either way. It stops for
            // cancel and shutdown (`catch_up_stop`, and the checks right
            // here), but not for the deadline — see `catch_up_stop`.
            let settings = load_settings_for_queue(&comp_app);
            let stop_aborted = aborted_for_task.clone();
            let stop_shutdown = comp_shutdown.clone();
            let stop_queue = comp_queue.clone();
            let stop_dl_id = comp_dl_id.clone();
            // Also stops when the item is cancelled mid-run. A queue that
            // is busy right now is not waited for: the next step asks
            // again.
            let stop = move || {
                stop_aborted.load(Ordering::Relaxed)
                    || stop_shutdown.is_triggered()
                    || stop_queue
                        .try_lock()
                        .is_ok_and(|q| !q.should_keep_working_on(&stop_dl_id))
            };
            // The catch-up's own stop check leaves out the deadline flag:
            // `recover_pending` takes the whole list before it starts, so a
            // deadline landing part-way would drop the files not yet reached
            // and the completion task's own catch-up would find nothing left.
            // The completion task waits for this task after an abort anyway,
            // so it simply waits for the catch-up to finish. Cancel and quit
            // still stop it.
            let catch_up_shutdown = comp_shutdown.clone();
            let catch_up_queue = comp_queue.clone();
            let catch_up_dl_id = comp_dl_id.clone();
            let catch_up_stop = move || {
                catch_up_shutdown.is_triggered()
                    || catch_up_queue
                        .try_lock()
                        .is_ok_and(|q| !q.should_keep_working_on(&catch_up_dl_id))
            };
            if comp_queue.lock().await.should_keep_working_on(&comp_dl_id) {
                // Wait for any writing job an abandoned enrichment left
                // running, so the two never write the same file at once.
                let wait = match in_flight_handle.as_ref() {
                    Some(in_flight) => {
                        super::lyrics_steps::wait_to_catch_up(
                            pending_main_handle.as_ref(),
                            in_flight,
                            super::lyrics_steps::CATCH_UP_WAIT,
                        )
                        .await
                    }
                    None => super::lyrics_steps::CatchUpWait::NothingPending,
                };
                match wait {
                    super::lyrics_steps::CatchUpWait::TimedOut => {
                        log::debug!(
                            "Lyrics catch-up skipped for {comp_dl_id}: a writing job was still running"
                        );
                    }
                    other => {
                        // Held until the catch-up has finished writing.
                        let _hold = other;
                        let (queue_artist, queue_album) = {
                            let q = comp_queue.lock().await;
                            q.items
                                .iter()
                                .find(|i| i.status.id == comp_dl_id)
                                .map(|i| {
                                    (i.status.artist_name.clone(), i.status.album_name.clone())
                                })
                                .unwrap_or((None, None))
                        };
                        let (names_artist, names_title) =
                            super::lyrics_steps::lyricsfile_names_for_catch_up(
                                lyricsfile_artist.as_deref(),
                                lyricsfile_default_title.as_deref(),
                                queue_artist.as_deref(),
                                queue_album.as_deref(),
                            );
                        if let Some(line) = super::companion_lyrics::recover_pending_locked(
                            crate::utils::file_locks::global(),
                            pending_main_handle.as_ref(),
                            &settings,
                            names_artist.as_deref(),
                            names_title.as_deref(),
                            &catch_up_stop,
                        )
                        .await
                        .as_ref()
                        .and_then(super::companion_lyrics::recovery_line)
                        {
                            emit_download_log(&comp_app, &comp_dl_id, &line);
                        }
                    }
                }
            }
            if any_tier_produced_files && !aborted_for_task.load(Ordering::Relaxed) {
                let Some(output_dir) = comp_base_opts.output_path.as_deref() else {
                    return;
                };
                // Re-read hints from the queue item — the enrichment
                // task may have populated them via API mid-flight
                // even though they were absent at companion-loop start.
                let (artist_hint, album_hint) = {
                    let q = comp_queue.lock().await;
                    q.items
                        .iter()
                        .find(|i| i.status.id == comp_dl_id)
                        .map(|i| (i.status.artist_name.clone(), i.status.album_name.clone()))
                        .unwrap_or((None, None))
                };
                set_stage_with_label(
                    &comp_app,
                    &comp_queue,
                    &comp_dl_id,
                    ProgressStage::Finalising,
                    "Companion: converting lyrics formats…",
                );
                // #1440/#1441: only THIS download's new copies are given
                // lyrics, matched to the main download's files
                // (`companion_lyrics.rs`). With a recorded scope that is
                // exact. Without one (the gate was dropped, or the failure
                // path) the scope is rebuilt from what appeared in the album
                // folder after the extra copies started, and every such copy
                // gets its own lyrics. There is no folder-wide pass any more.
                let recorded_scope = lyrics_scope_handle
                    .as_ref()
                    .and_then(|h| h.lock().ok().and_then(|g| g.clone()));
                // A cancelled item gets no more lyrics work.
                if !comp_queue.lock().await.should_keep_working_on(&comp_dl_id) {
                    log::info!("Extra copy lyrics skipped — {comp_dl_id} was cancelled");
                    return;
                }
                let output_dir = std::path::PathBuf::from(output_dir);
                let (scope, since) = if let Some(scope) = recorded_scope {
                    (scope, gate_opened_at.unwrap_or(tiers_started_at))
                } else {
                    let Some(dir_after) = super::companion_lyrics::resolve_album_folder(
                        &output_dir,
                        companion_album_dir.as_deref(),
                        artist_hint.as_deref(),
                        album_hint.as_deref(),
                    ) else {
                        log::debug!(
                            "Extra copy lyrics: no album folder found for {comp_dl_id} — skipping"
                        );
                        return;
                    };
                    (
                        super::companion_lyrics::fallback_scope(
                            &dir_after,
                            folder_before.as_ref(),
                            tiers_started_at,
                        ),
                        tiers_started_at,
                    )
                };
                set_stage_with_label(
                    &comp_app,
                    &comp_queue,
                    &comp_dl_id,
                    ProgressStage::Finalising,
                    "Extra copies: adding lyrics…",
                );
                // Run right here, not in `spawn_blocking`: aborting the task
                // cannot stop a blocking closure that has already started,
                // which could keep writing files after the final rename pass
                // began. Done in this task, the completion task's wait covers
                // it (#663), and the stop check is asked before every step.
                // The names the album details gave; when they were never set,
                // the queue item's own names stand in.
                let (copy_artist, copy_title) = super::lyrics_steps::lyricsfile_names_for_catch_up(
                    lyricsfile_artist.as_deref(),
                    lyricsfile_default_title.as_deref(),
                    artist_hint.as_deref(),
                    album_hint.as_deref(),
                );
                let summary = super::companion_lyrics::run(
                    &settings,
                    &scope,
                    &output_dir,
                    artist_hint.as_deref(),
                    album_hint.as_deref(),
                    since,
                    &album_tracks,
                    copy_artist.as_deref(),
                    copy_title.as_deref(),
                    &stop,
                )
                .await;
                if let Some(line) = super::companion_lyrics::summary_line(&summary) {
                    emit_download_log(&comp_app, &comp_dl_id, &line);
                }
            }
        });
        // Heartbeat ticker for the companion phase (#805). Shares the
        // cooperative-cancel flag with the supervisor task — when the
        // supervisor finishes (success / failure / abort) the flag
        // flips and the heartbeat exits on its next tick. Dropping
        // `CompanionTaskHandle` also drops the ticker via Drop, so
        // the ticker never outlives the handle either.
        let heartbeat = Some(start_heartbeat_ticker(
            app.clone(),
            queue.clone(),
            dl_id.to_string(),
            comp_aborted.clone(),
            "Companion",
        ));

        Some(CompanionTaskHandle {
            handle,
            aborted: comp_aborted,
            progress: companion_progress,
            heartbeat,
        })
    };

    // === Lyrics companion downloads (background, fire-and-forget) ===
    // When the user has selected additional lyrics formats beyond the
    // primary, spawn a lightweight GAMDL invocation for each extra
    // format using --synced-lyrics-only. This produces sidecar lyrics
    // files (.lrc, .srt, .ttml) without re-downloading audio.
    let lyrics_settings = load_settings_for_queue(app);
    // GAMDL 4.x has no lyrics-only mode, so each of these runs would
    // become a full re-download of the album (found in review round 4).
    // Skipped, with one line saying why; 4.x writes `.lrc` / `.txt`
    // during the main download.
    let lyrics_only_unavailable = crate::services::gamdl_capabilities::detected_cli_generation()
        == crate::services::gamdl_capabilities::GamdlCliGeneration::V4;
    if lyrics_only_unavailable
        && !lyrics_settings.companion_lyrics_formats.is_empty()
        && (!lyrics_settings.no_synced_lyrics || lyrics_settings.embed_lyrics_and_sidecar)
    {
        emit_download_log(
            app,
            dl_id,
            "Extra lyrics formats skipped — GAMDL 4 has no lyrics-only download",
        );
    }
    if !lyrics_only_unavailable
        && !lyrics_settings.companion_lyrics_formats.is_empty()
        && (!lyrics_settings.no_synced_lyrics || lyrics_settings.embed_lyrics_and_sidecar)
    {
        let lyrics_app = app.clone();
        let lyrics_urls = urls.to_vec();
        let lyrics_base_opts = companion_base_options.clone();
        let lyrics_dl_id = dl_id.to_string();
        let lyrics_formats = lyrics_settings.companion_lyrics_formats.clone();
        let lyrics_shutdown = shutdown.clone();

        let format_names: Vec<&str> = lyrics_formats
            .iter()
            .map(LyricsFormat::to_cli_string)
            .collect();
        emit_download_log(
            app,
            dl_id,
            &format!(
                "Downloading companion lyrics formats: {}",
                format_names.join(", ")
            ),
        );

        tokio::spawn(async move {
            for format in &lyrics_formats {
                // Check for app shutdown between lyrics format iterations
                if lyrics_shutdown.is_triggered() {
                    log::info!("Lyrics companion downloads stopping early (app shutting down)");
                    return;
                }
                let mut opts = lyrics_base_opts.clone();
                opts.synced_lyrics_format = Some(format.clone());
                opts.synced_lyrics_only = Some(true);

                let mut cmd = match gamdl_service::build_gamdl_command_public(
                    &lyrics_app,
                    &lyrics_urls,
                    &opts,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        log::debug!(
                            "Lyrics companion ({}) command error: {e}",
                            format.to_cli_string()
                        );
                        continue;
                    }
                };

                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                // Stop the program if the task waiting for it is dropped.
                // The supervised companion runs have always done this;
                // this one was missed. Found by a full review of the
                // codebase.
                //
                // Today this never actually fires here, and an earlier
                // version of this comment claimed it did — "when the item
                // is cancelled, a deadline fires, or the app quits". None
                // of those is true (checked 27 Sept 2026, #1215 finding 5):
                //
                // - This task is started and let go of (the `tokio::spawn`
                //   above keeps no handle), so nothing can ever abort it.
                //   Not Cancel, which does not reach it; not the companion
                //   deadline, which aborts the separate supervised run,
                //   not this task.
                // - Between formats it looks only at whether the app is
                //   shutting down. A format already downloading always
                //   runs to the end.
                // - Quitting the app ends its whole process at once
                //   without dropping running tasks, so this setting does
                //   not fire then either. What becomes of a program still
                //   running at that moment has not been checked.
                //
                // It stays because it costs nothing and is the right
                // setting the day this task is given a handle something
                // can abort. Making Cancel stop these downloads is a
                // change of behaviour, raised as its own issue.
                cmd.kill_on_drop(true);

                emit_download_log(
                    &lyrics_app,
                    &lyrics_dl_id,
                    &format!(
                        "Lyrics companion: downloading {}...",
                        format.to_cli_string()
                    ),
                );

                match cmd.spawn() {
                    Ok(child) => match child.wait_with_output().await {
                        Ok(output) if output.status.success() => {
                            log::info!(
                                "Lyrics companion ({}) \
                                     downloaded for {}",
                                format.to_cli_string(),
                                lyrics_dl_id
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!(
                                    "Lyrics companion ({}) downloaded",
                                    format.to_cli_string(),
                                ),
                            );
                            let _ = lyrics_app.emit(
                                "lyrics-companion-downloaded",
                                serde_json::json!({
                                    "download_id": lyrics_dl_id,
                                    "format": format.to_cli_string(),
                                }),
                            );
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let last_line = stderr.lines().last().unwrap_or("");
                            log::debug!(
                                "Lyrics companion ({}) \
                                     failed for {}: {}",
                                format.to_cli_string(),
                                lyrics_dl_id,
                                last_line
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!(
                                    "Lyrics companion ({}) failed: {}",
                                    format.to_cli_string(),
                                    last_line,
                                ),
                            );
                        }
                        Err(e) => {
                            log::debug!(
                                "Lyrics companion process \
                                     error: {e}"
                            );
                            emit_download_log(
                                &lyrics_app,
                                &lyrics_dl_id,
                                &format!("Lyrics companion process error: {e}"),
                            );
                        }
                    },
                    Err(e) => {
                        log::debug!("Failed to spawn lyrics companion: {e}");
                        emit_download_log(
                            &lyrics_app,
                            &lyrics_dl_id,
                            &format!("Failed to spawn lyrics companion: {e}"),
                        );
                    }
                }
            }
        });
    }

    // Return the codec companion handle if it exists.
    // (Lyrics companions are fire-and-forget — they're fast and non-critical.)
    codec_handle
}
