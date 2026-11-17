// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Word-level (word-by-word) lyrics after a download: MeedyaDL's own
// lookup (enrichment Step 1b), and the GAMDL 4.0.2+ pass that fills in the
// tracks that lookup could not check.
//
// The order, decided by the maintainer (16 Nov 2026):
//   1. MeedyaDL's own lookup first, on every GAMDL version (`run_step_1b`);
//   2. on GAMDL 4.0.2 or later, a second GAMDL run that asks only for the
//      word-by-word lyrics file — and only for tracks step 1 could not ask
//      about (no developer key, no usable sign-in, a refused request, a
//      file it could not place). Never for "Apple Music has none", and not
//      run at all when nothing needs it (`needs_gamdl_fallback`,
//      `should_run_word_lyrics_pass`);
//   3. every lyrics-file conversion after both, then the per-file renames.
//
// The GAMDL run works only because the audio is still exactly where GAMDL
// put it: GAMDL skips a track whose file already exists (overwrite off),
// and then still refreshes its `.ttml`. So it runs before ANY rename —
// the album folder's advisory suffix included — with the primary run's
// own templates, download folder and GAMDL version, and refuses if a
// primary file has moved (`primary_files_unmoved`).
//
// The extra-format copies (companions) are held until the renames are
// done (`CompanionGate`), so nothing lands beside the primary files while
// their names may still change, and companions write into the folder that
// really holds the files (#528).

use super::*;
use std::path::{Path, PathBuf};

use crate::services::apple_music_api::{AlbumMetadata, TrackMetadata};

// ============================================================
// Step 1b: per-track result
// ============================================================

/// What MeedyaDL's own word-level lookup found for one track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordLyricsOutcome {
    /// The track's `.ttml` has word-by-word timing (already, or now).
    WordTimed,
    /// Apple Music has no word-by-word lyrics for it (or no lyrics at all).
    /// Never a reason to ask GAMDL again: it would get the same answer.
    AppleHasNone,
    /// The question could not be asked or was not answered: no developer
    /// key or sign-in, a refused or failed request, or the app stopping.
    CouldNotAsk,
    /// Word-by-word lyrics came back but could not be saved next to the
    /// right file (no single matching file, or the write failed).
    NotPlaced,
}

/// One track's result from [`run_step_1b`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackLyricsResult {
    /// What happened.
    pub outcome: WordLyricsOutcome,
    /// Whether the track was on the list to look up once a lookup could
    /// start (as opposed to already word-timed, or marked as having no
    /// lyrics). Only these are counted in the summary line.
    pub looked_up: bool,
    /// The word-by-word `.ttml` file written for this track, when one was.
    /// The lyrics steps use it to be sure they also work on the music file
    /// it sits beside (#1441).
    pub wrote_ttml: Option<PathBuf>,
}

/// What [`run_step_1b`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step1b {
    /// Not run at all: no album details from Apple Music (a song or
    /// library link, no developer key), or Enhanced LRC is off.
    NotRun,
    /// Run, with one result per track of this download.
    Ran(Vec<TrackLyricsResult>),
}

/// Sorts one lookup answer. `placed` is whether the word-by-word lyrics
/// (if any) were written next to the right track.
///
/// A document with no word timing is "Apple has none" — and is not
/// written, so it cannot replace the line-by-line `.ttml` GAMDL saved.
pub(crate) fn classify<E>(fetch: &Result<Option<String>, E>, placed: bool) -> WordLyricsOutcome {
    match fetch {
        Ok(Some(ttml)) if enhanced_lyrics_service::ttml_has_word_timing(ttml) => {
            if placed {
                WordLyricsOutcome::WordTimed
            } else {
                WordLyricsOutcome::NotPlaced
            }
        }
        Ok(_) => WordLyricsOutcome::AppleHasNone,
        Err(_) => WordLyricsOutcome::CouldNotAsk,
    }
}

/// Whether the GAMDL pass has anything to do after Step 1b.
///
/// Yes when Step 1b did not run, or left any track it could not check or
/// could not save. No when every track is word-timed or has none on Apple
/// Music.
pub(crate) fn needs_gamdl_fallback(result: &Step1b) -> bool {
    match result {
        Step1b::NotRun => true,
        Step1b::Ran(tracks) => tracks.iter().any(|t| {
            matches!(
                t.outcome,
                WordLyricsOutcome::CouldNotAsk | WordLyricsOutcome::NotPlaced
            )
        }),
    }
}

/// The one activity-log line for Step 1b, or `None` when nothing was
/// looked up.
pub(crate) fn step_1b_summary(
    results: &[TrackLyricsResult],
    stopped_on_sign_in: bool,
) -> Option<String> {
    let looked: Vec<_> = results.iter().filter(|r| r.looked_up).collect();
    let total = looked.len();
    if total == 0 {
        return None;
    }
    let count = |o: WordLyricsOutcome| looked.iter().filter(|r| r.outcome == o).count();
    let added = count(WordLyricsOutcome::WordTimed);
    let none = count(WordLyricsOutcome::AppleHasNone);
    let unchecked = count(WordLyricsOutcome::CouldNotAsk);
    let unsaved = count(WordLyricsOutcome::NotPlaced);

    if added == total {
        return Some(format!("Word-level lyrics added for {added} track(s)"));
    }
    if none == total {
        return Some(format!(
            "Word-level lyrics: Apple Music has none for any of the {total} track(s) — keeping line-by-line lyrics"
        ));
    }
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("{added} added"));
    }
    if none > 0 {
        parts.push(format!("{none} not available on Apple Music"));
    }
    if unchecked > 0 {
        parts.push(format!("{unchecked} could not be checked"));
    }
    if unsaved > 0 {
        parts.push(format!("{unsaved} could not be saved"));
    }
    let mut line = format!(
        "Word-level lyrics: {} (of {total} track(s))",
        parts.join(", ")
    );
    if stopped_on_sign_in {
        line.push_str(" — stopped early because Apple Music did not accept the sign-in");
    }
    Some(line)
}

// ============================================================
// Step 1b: which file is which track
// ============================================================

/// A downloaded audio file and the disc and track numbers it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioFileNumbers {
    pub path: PathBuf,
    /// `None` when the number could not be read.
    pub disc: Option<u32>,
    /// `None` when the number could not be read.
    pub track: Option<u32>,
    /// Written (or rewritten) since this download started, judged when
    /// enrichment took its snapshot — before Step 0 rewrites every file's
    /// tags and makes them all look new. Only used to choose between two
    /// files claiming the same track (this download's and an older copy).
    pub fresh: bool,
    /// The name carries the same format suffix as the primary run's file
    /// template (or none, when the template has none). The second way to
    /// choose between two files claiming the same track.
    pub name_matches_primary: bool,
}

/// Which snapshot files were written since `since` (the download's start),
/// with two seconds of slack for file systems that store times coarsely.
/// Taken at snapshot time, before Step 0 rewrites tags. `None` (start
/// unknown) marks nothing as fresh.
pub(crate) fn fresh_at_snapshot(
    files: &[PathBuf],
    since: Option<std::time::SystemTime>,
) -> std::collections::HashSet<PathBuf> {
    let Some(since) = since else {
        return std::collections::HashSet::new();
    };
    let since = since
        .checked_sub(std::time::Duration::from_secs(2))
        .unwrap_or(since);
    files
        .iter()
        .filter(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .is_ok_and(|modified| modified >= since)
        })
        .cloned()
        .collect()
}

/// Whether a file name ends the way the primary run named its files:
/// with the format suffix MeedyaDL appended to its templates
/// (`expected`), or, when it appended none, with no format suffix at all.
/// Only the END of the name is looked at — where the suffix goes — so a
/// label the person typed elsewhere in their template does not count.
pub(crate) fn stem_matches_primary_name(stem: &str, expected: Option<&str>) -> bool {
    match expected {
        Some(s) => stem.ends_with(&format!(" {s}")),
        None => !metadata_tag_service::registry_codec_suffixes()
            .iter()
            .any(|s| stem.ends_with(&format!(" {s}"))),
    }
}

/// Disc and track numbers from a file name, for a file whose tags cannot
/// be read: `1-01 Title` → disc 1, track 1; `01 Title` → track 1, disc
/// unknown. Anything else gives neither.
pub(crate) fn numbers_from_file_name(stem: &str) -> (Option<u32>, Option<u32>) {
    let first_len = stem.chars().take_while(char::is_ascii_digit).count();
    if first_len == 0 {
        return (None, None);
    }
    let first: Option<u32> = stem[..first_len].parse().ok();
    let rest = &stem[first_len..];
    if let Some(after_dash) = rest.strip_prefix('-') {
        let second_len = after_dash.chars().take_while(char::is_ascii_digit).count();
        let after_second = &after_dash[second_len..];
        if second_len > 0 && (after_second.is_empty() || after_second.starts_with(' ')) {
            return (first, after_dash[..second_len].parse().ok());
        }
        return (None, None);
    }
    if rest.is_empty() || rest.starts_with(' ') {
        return (None, first);
    }
    (None, None)
}

/// Reads a file's disc and track numbers from its tags (GAMDL writes
/// them), falling back to its name. A file with a track number and no
/// disc number in its tags counts as disc 1, as elsewhere in enrichment.
/// `fresh` comes from [`fresh_at_snapshot`]; `primary_suffix` is the
/// format suffix MeedyaDL appended to the primary run's templates.
pub(crate) fn read_audio_numbers(
    path: &Path,
    fresh: bool,
    primary_suffix: Option<&str>,
) -> AudioFileNumbers {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let name_matches_primary = stem_matches_primary_name(stem, primary_suffix);
    if let Ok(tag) = mp4ameta::Tag::read_from_path(path) {
        if let Some(track) = tag.track_number() {
            return AudioFileNumbers {
                path: path.to_path_buf(),
                disc: Some(u32::from(tag.disc_number().unwrap_or(1))),
                track: Some(u32::from(track)),
                fresh,
                name_matches_primary,
            };
        }
    }
    let (disc, track) = numbers_from_file_name(stem);
    AudioFileNumbers {
        path: path.to_path_buf(),
        disc,
        track,
        fresh,
        name_matches_primary,
    }
}

/// Where a track's file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Placement {
    /// Exactly one file is this track.
    Found(PathBuf),
    /// No file is this track, and every file's numbers are known: the
    /// track is simply not part of this download (a single song from an
    /// album, for example).
    NotInDownload,
    /// More than one file could be this track (two discs both with a
    /// "01"), or some file's numbers are unknown.
    Unclear,
}

/// Finds the one file that is disc `disc`, track `track`.
///
/// A file whose disc is unknown (named `01 Title`, tags unreadable)
/// matches only on a single-disc album, as disc 1 — on a multi-disc album
/// it could be any disc, so it makes the answer [`Placement::Unclear`]
/// rather than a guess. (The old lookup matched on the `"01 "` name prefix
/// alone, so with two discs it could overwrite the wrong disc's lyrics,
/// and a `1-01 Title` name never matched at all.)
pub(crate) fn place_track(
    disc: u32,
    track: u32,
    files: &[AudioFileNumbers],
    single_disc: bool,
) -> Placement {
    let disc_of = |f: &AudioFileNumbers| f.disc.or(if single_disc { Some(1) } else { None });
    let matches: Vec<&AudioFileNumbers> = files
        .iter()
        .filter(|f| f.track == Some(track) && disc_of(f) == Some(disc))
        .collect();
    match matches.len() {
        1 => Placement::Found(matches[0].path.clone()),
        0 if files
            .iter()
            .all(|f| f.track.is_some() && disc_of(f).is_some()) =>
        {
            Placement::NotInDownload
        }
        0 => Placement::Unclear,
        _ => {
            // Several files claim the track (a copy from an earlier
            // download in the same folder): the one this download wrote,
            // if exactly one was; else the one named like the primary run's
            // template, if exactly one is.
            let fresh: Vec<_> = matches.iter().filter(|f| f.fresh).collect();
            let named: Vec<_> = matches.iter().filter(|f| f.name_matches_primary).collect();
            if fresh.len() == 1 {
                Placement::Found(fresh[0].path.clone())
            } else if named.len() == 1 {
                Placement::Found(named[0].path.clone())
            } else {
                Placement::Unclear
            }
        }
    }
}

/// The music files Step 1b wrote a word-by-word `.ttml` beside: the files
/// in `primary_files` whose `.ttml` is one of the files Step 1b wrote.
/// Empty when Step 1b did not run.
pub(crate) fn files_step_1b_wrote_beside(
    result: &Step1b,
    primary_files: &[PathBuf],
) -> Vec<PathBuf> {
    let Step1b::Ran(tracks) = result else {
        return Vec::new();
    };
    let wrote: std::collections::HashSet<&PathBuf> = tracks
        .iter()
        .filter_map(|t| t.wrote_ttml.as_ref())
        .collect();
    primary_files
        .iter()
        .filter(|p| wrote.contains(&p.with_extension("ttml")))
        .cloned()
        .collect()
}

/// The music videos (`.m4v`, `.mp4`) directly in `dir` or one folder level
/// below it. Shallow on purpose: when the album folder falls back to the
/// whole download folder, a deep walk would read every file in the library.
pub(crate) fn video_files_in_depth1(dir: &Path) -> Vec<PathBuf> {
    video_files_to_depth(dir, 1)
}

/// The music videos in `dir` down to `depth` folder levels, skipping
/// file-system sidecars. The one filter [`video_files_in_depth1`] and the
/// pre-run list ([`videos_before_run`]) share.
pub(crate) fn video_files_to_depth(dir: &Path, depth: u32) -> Vec<PathBuf> {
    crate::utils::fs_walk::walk_dir_depth(dir, depth, |path| {
        if !path.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(path) {
            return None;
        }
        let ext = path.extension().and_then(|e| e.to_str())?;
        ["m4v", "mp4"]
            .iter()
            .any(|x| ext.eq_ignore_ascii_case(x))
            .then(|| path.to_path_buf())
    })
}

/// A path in a form that compares equal however it was spelled: resolved
/// when the file exists, as given otherwise.
fn comparable(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The music videos already in the download folder, taken when the PRIMARY
/// run starts (the same moment as the audio-file count), so a video that
/// appears afterwards can be told from one that was already there without
/// relying on file times (a folder on a machine whose clock lags writes
/// times in the past). Deep enough to reach `Artist/Album/` folders.
#[cfg(test)]
pub(crate) fn videos_before_run(download_folder: &Path) -> std::collections::HashSet<PathBuf> {
    video_files_to_depth(download_folder, 10)
        .iter()
        .map(|p| comparable(p))
        .collect()
}

/// Everything the start of the primary run needs from ONE walk of the
/// download folder: how many music or video files are in it (the same
/// count as `count_audio_files_in_directory`), the music videos in it (the
/// same list as [`videos_before_run`], resolved paths), and the `.m4a` music
/// files in it, as PLAIN paths under `download_folder` (so a file can later
/// be told from one this run wrote without relying on file times alone,
/// #1446; the later look-through starts from the same plain folder, so the
/// paths match without resolving any of them). The music list is `None`
/// when the walk could not read some folder (anything but "not found"): a
/// list that may be missing files would make every older file look new, so
/// the time rule decides alone then.
pub(crate) fn files_before_run(
    download_folder: &Path,
) -> (
    usize,
    std::collections::HashSet<PathBuf>,
    Option<std::sync::Arc<std::collections::HashSet<PathBuf>>>,
) {
    let (files, complete) =
        crate::utils::fs_walk::walk_dir_depth_reporting(download_folder, 10, |path| {
            if !path.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(path) {
                return None;
            }
            let ext = path.extension().and_then(|e| e.to_str())?;
            ["m4a", "m4v", "mp4"]
                .iter()
                .any(|x| ext.eq_ignore_ascii_case(x))
                .then(|| path.to_path_buf())
        });
    let has_ext = |p: &Path, exts: &[&str]| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
    };
    let videos = files
        .iter()
        .filter(|p| has_ext(p, &["m4v", "mp4"]))
        .map(|p| comparable(p))
        .collect();
    let music = complete.then(|| {
        std::sync::Arc::new(
            files
                .iter()
                .filter(|p| has_ext(p, &["m4a"]))
                .cloned()
                .collect::<std::collections::HashSet<PathBuf>>(),
        )
    });
    (files.len(), videos, music)
}

/// Whether a music file was written by this run (`path` is a plain path
/// under the download folder, as in the before-run list).
///
/// With the list of files taken before the run (`before`): a file not in it
/// is this run's; a file in it counts only when the run had overwrite on
/// (`overwrite`; only then can GAMDL rewrite an existing file) and it was
/// modified after the start (`since`, with the same two seconds of slack as
/// [`fresh_at_snapshot`]). Without the list (it could not be taken) the file
/// time decides alone, and with no time nothing counts. A file GAMDL skipped
/// because it was already there is in the list and, with overwrite off,
/// never this run's.
pub(crate) fn written_this_run(
    path: &Path,
    before: Option<&std::collections::HashSet<PathBuf>>,
    since: Option<std::time::SystemTime>,
    overwrite: bool,
) -> bool {
    let by_time = fresh_at_snapshot(&[path.to_path_buf()], since).contains(path);
    match before {
        Some(set) => !set.contains(path) || (overwrite && by_time),
        None => by_time,
    }
}

/// This download's music videos among `current` (the videos in the album
/// folder now). With the pre-run list (`before`), every video not in it is
/// this download's; a video that was already there still counts when it was
/// written after the start (a re-download over it), by file time. Without
/// the list (it could not be taken), the file-time rule decides alone.
pub(crate) fn this_downloads_videos(
    current: &[PathBuf],
    before: Option<&std::collections::HashSet<PathBuf>>,
    since: Option<std::time::SystemTime>,
) -> Vec<PathBuf> {
    let by_time = fresh_at_snapshot(current, since);
    current
        .iter()
        .filter(|p| by_time.contains(*p) || before.is_some_and(|set| !set.contains(&comparable(p))))
        .cloned()
        .collect()
}

// ============================================================
// Step 1b: the lookup
// ============================================================

/// MeedyaDL's own word-level lyrics lookup (enrichment Step 1b), for the
/// primary download's files only.
///
/// Writes word-by-word `.ttml` files next to the tracks they belong to,
/// writes one summary line, and returns a result per track so the GAMDL
/// pass knows whether it has anything to do. Never fails the download.
///
/// `set_label` updates the progress caption.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub(crate) async fn run_step_1b(
    app: &AppHandle,
    dl_id: &str,
    settings: &AppSettings,
    metadata: Option<&AlbumMetadata>,
    urls: &[String],
    primary_files: &[PathBuf],
    fresh_files: &std::collections::HashSet<PathBuf>,
    primary_suffix: Option<&str>,
    shutdown: &ShutdownSignal,
    set_label: &(dyn Fn(&str) + Sync),
) -> Step1b {
    if !settings.enhanced_lrc {
        return Step1b::NotRun;
    }
    let Some(metadata) = metadata else {
        return Step1b::NotRun;
    };

    // Which file is which track — worked out before anything else, so an
    // album that is already fully word-timed never gets a sign-in message.
    let numbers: Vec<AudioFileNumbers> = primary_files
        .iter()
        .map(|p| read_audio_numbers(p, fresh_files.contains(p), primary_suffix))
        .collect();
    let single_disc = metadata.tracks.iter().all(|t| t.disc_number <= 1);

    let mut results: Vec<TrackLyricsResult> = Vec::new();
    // Tracks to look up, with the `.ttml` to write (None: no single file).
    let mut to_ask: Vec<(&TrackMetadata, Option<PathBuf>)> = Vec::new();
    for track in &metadata.tracks {
        let placement = place_track(track.disc_number, track.track_number, &numbers, single_disc);
        if placement == Placement::NotInDownload {
            continue;
        }
        // Apple omits `hasLyrics` on some tracks that do have them, so only
        // an explicit `false` means none (a missing flag is looked up).
        if track.has_lyrics == Some(false) {
            results.push(TrackLyricsResult {
                outcome: WordLyricsOutcome::AppleHasNone,
                looked_up: false,
                wrote_ttml: None,
            });
            continue;
        }
        let sidecar = match placement {
            Placement::Found(audio) => Some(audio.with_extension("ttml")),
            _ => None,
        };
        // Already word-timed (span presence is the signal, #969 — Apple
        // labels word-timed TTML inconsistently).
        let already = sidecar
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|c| enhanced_lyrics_service::ttml_has_word_timing(&c));
        if already {
            results.push(TrackLyricsResult {
                outcome: WordLyricsOutcome::WordTimed,
                looked_up: false,
                wrote_ttml: None,
            });
        } else {
            to_ask.push((track, sidecar));
        }
    }
    if to_ask.is_empty() {
        return Step1b::Ran(results);
    }

    // Developer key (MusicKit credentials, else the web player's) and the
    // person's own sign-in from the cookies file.
    let private_key = apple_music_api::get_private_key_from_keychain()
        .ok()
        .flatten();
    let jwt = apple_music_api::resolve_premium_feature_token(
        settings.musickit_team_id.as_deref(),
        settings.musickit_key_id.as_deref(),
        private_key.as_deref(),
    )
    .ok()
    .flatten()
    .map(|(token, source)| {
        log::debug!("Syllable-lyrics: using MusicKit token from {source}");
        token
    });
    let music_user_token = match settings.cookies_path.as_deref() {
        Some(p) => match apple_music_api::extract_media_user_token(p) {
            Ok(Some(token)) => Some(token),
            Ok(None) => {
                emit_download_log(
                    app,
                    dl_id,
                    "Word-level lyrics skipped: Apple Music cookies expired or missing. Re-import from your browser.",
                );
                None
            }
            Err(_) => None,
        },
        None => None,
    };

    let (Some(jwt), Some(token)) = (jwt, music_user_token) else {
        set_label("Skipping word-level lyrics (no credentials)");
        log::debug!(
            "Syllable-lyrics skipped for {dl_id}: MusicKit JWT or Music-User-Token unavailable"
        );
        results.extend(to_ask.iter().map(|_| TrackLyricsResult {
            outcome: WordLyricsOutcome::CouldNotAsk,
            looked_up: false,
            wrote_ttml: None,
        }));
        return Step1b::Ran(results);
    };

    set_label("Fetching word-level lyrics…");
    // The country to ask Apple Music about. The saved setting is empty
    // when Storefront is on Auto-detect, so it is never used as it is.
    let storefront = lyrics_lookup_storefront(
        urls,
        &settings.storefront,
        crate::services::login_window_service::detect_storefront(),
    );
    let (asked, stopped_on_sign_in) = ask_each_track(
        to_ask,
        |song_id| {
            let song_id = song_id.to_string();
            let jwt = jwt.clone();
            let token = token.clone();
            let storefront = storefront.clone();
            async move {
                apple_music_api::fetch_syllable_lyrics_with_status(
                    &jwt,
                    &storefront,
                    &song_id,
                    &token,
                    Some(&settings.language),
                )
                .await
            }
        },
        || shutdown.is_triggered(),
        std::time::Duration::from_millis(200),
    )
    .await;
    results.extend(asked);

    if let Some(line) = step_1b_summary(&results, stopped_on_sign_in) {
        emit_download_log(app, dl_id, &line);
    }
    Step1b::Ran(results)
}

/// Asks about each track in turn and saves word-by-word lyrics beside it.
///
/// - Lyrics with no word timing are NOT written: they would replace the
///   line-by-line `.ttml` GAMDL saved with nothing better.
/// - A refused sign-in (HTTP 401/403) stops the asking; every track not
///   yet asked counts as "could not be checked", as does every track left
///   when `stopping` says the app is closing.
/// - `pause` is waited between requests.
///
/// Returns the results and whether it stopped on a refused sign-in.
pub(crate) async fn ask_each_track<F, Fut>(
    to_ask: Vec<(&TrackMetadata, Option<PathBuf>)>,
    mut fetch: F,
    stopping: impl Fn() -> bool,
    pause: std::time::Duration,
) -> (Vec<TrackLyricsResult>, bool)
where
    F: FnMut(&str) -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>, apple_music_api::SyllableLyricsError>>,
{
    let mut results = Vec::with_capacity(to_ask.len());
    let mut stopped_on_sign_in = false;
    let total = to_ask.len();
    for (index, (track, sidecar)) in to_ask.into_iter().enumerate() {
        if stopped_on_sign_in || stopping() {
            results.push(TrackLyricsResult {
                outcome: WordLyricsOutcome::CouldNotAsk,
                looked_up: true,
                wrote_ttml: None,
            });
            continue;
        }
        let answer = fetch(&track.song_id).await;
        let has_words =
            matches!(&answer, Ok(Some(t)) if enhanced_lyrics_service::ttml_has_word_timing(t));
        let wrote_ttml: Option<PathBuf> = match (&answer, &sidecar) {
            (Ok(Some(ttml)), Some(path)) if has_words => match std::fs::write(path, ttml) {
                Ok(()) => Some(path.clone()),
                Err(e) => {
                    log::debug!("Failed to write TTML for track {}: {e}", track.song_id);
                    None
                }
            },
            _ => None,
        };
        let placed = wrote_ttml.is_some();
        if let Err(e) = &answer {
            log::debug!(
                "Syllable-lyrics fetch failed for disc {} track {} (song {}): {e}",
                track.disc_number,
                track.track_number,
                track.song_id
            );
            // A refused sign-in affects every track: stop asking. Judged
            // on the HTTP status, never the message (which contains the
            // song ID — `1440140198` contains "401").
            if e.is_refused_sign_in() {
                stopped_on_sign_in = true;
            }
        }
        results.push(TrackLyricsResult {
            outcome: classify(&answer, placed),
            looked_up: true,
            wrote_ttml,
        });
        // A short pause between requests, so Apple Music is not flooded.
        if index + 1 < total && !stopped_on_sign_in && !pause.is_zero() {
            tokio::time::sleep(pause).await;
        }
    }
    (results, stopped_on_sign_in)
}

// ============================================================
// The GAMDL pass (4.0.2 and later)
// ============================================================

/// The options for the GAMDL pass: the primary run's own options (same
/// sign-in, formats, folder and file templates), with:
///
/// - `download_folder`: the folder the primary run really used, given
///   explicitly so nothing is re-read from settings when the pass starts;
/// - overwrite off, so every track whose file already exists is skipped
///   rather than downloaded again — GAMDL still refreshes its `.ttml`;
/// - no saved cover, no playlist file, lyrics files on;
/// - the word-by-word lyrics file asked for.
///
/// The templates are NOT changed: the pass runs before any rename, the
/// album folder's included, so the primary's own templates point at the
/// files exactly.
///
/// Never sends `--overwrite`, `--save-cover`, `--file-cover-size`,
/// `--save-playlist` or `--no-lyrics-file` (tested).
pub(crate) fn plan_word_lyrics_pass(primary: &GamdlOptions, download_folder: &str) -> GamdlOptions {
    let mut options = primary.clone();
    options.output_path = Some(download_folder.to_string());
    options.overwrite = Some(false);
    options.save_cover = Some(false);
    options.save_playlist = None;
    options.no_synced_lyrics = Some(false);
    options.synced_lyrics_only = None;
    options.synced_lyrics_format = Some(LyricsFormat::Ttml);
    options.word_timed_lyrics_file = Some(true);
    options
}

/// The options for one GAMDL 3 lyrics recovery run (Step 2b, #1436): the
/// primary run's own options (same sign-in, wrapper addresses, folder and
/// file templates, including any format label MeedyaDL added to the file
/// names), with:
///
/// - `download_folder`: the folder the primary run really used;
/// - lyrics only, in `format`;
/// - overwrite OFF, whatever the primary run had. A lyrics-only run with
///   overwrite on rewrites the lyrics file of every track, which would
///   replace the word-by-word Enhanced LRC files Step 2 has just made; this
///   run is only meant to fill gaps, so it never replaces a file;
/// - no saved cover, no playlist file, no word-by-word lyrics file, and the
///   settings file left out (`no_config_file`): GAMDL 3 reads that file as
///   its defaults and its `save_cover = true` line would still make this
///   run save a fresh `Cover` picture beside the one already renamed to the
///   person's chosen name. Everything the file carries is already in the
///   primary run's own options (they are built from the same settings), so
///   leaving it out loses nothing.
///
/// Like the word-by-word pass, this runs before any rename, so the primary's
/// templates point at the files exactly. It used to be built from Settings
/// instead, which missed a format label added to the file names and the
/// newer wrapper settings, so lyrics could land beside the wrong name.
pub(crate) fn plan_lyrics_recovery_run(
    primary: &GamdlOptions,
    download_folder: &str,
    format: LyricsFormat,
) -> GamdlOptions {
    let mut options = primary.clone();
    options.output_path = Some(download_folder.to_string());
    options.overwrite = Some(false);
    options.save_cover = Some(false);
    options.save_playlist = None;
    options.no_config_file = Some(true);
    options.no_synced_lyrics = Some(false);
    options.synced_lyrics_only = Some(true);
    options.synced_lyrics_format = Some(format);
    options.word_timed_lyrics_file = None;
    options
}

/// Whether the links are ones the pass may run for: albums and songs
/// only. Not artist links (they mean many albums), music videos (no word
/// lyrics), playlists (their tracks are not in one album folder), or
/// anything unrecognised.
pub(crate) fn links_allow_word_lyrics_pass(urls: &[String]) -> bool {
    !urls.is_empty()
        && urls.iter().all(|u| {
            apple_music_api::parse_apple_music_url(u)
                .is_some_and(|p| p.content_type == "album" || p.content_type == "song")
        })
}

/// Everything that decides whether the GAMDL pass runs.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct WordLyricsPassConditions {
    /// The installed GAMDL is 4.0.2 or later.
    pub gamdl_can: bool,
    /// Enhanced LRC is on (word-level lyrics are wanted at all).
    pub enhanced_lrc: bool,
    /// The primary run saved lyrics files.
    pub lyrics_files_on: bool,
    /// An Apple Music album or song link ([`links_allow_word_lyrics_pass`]).
    pub links_allow: bool,
    /// A link to the person's own library.
    pub is_library: bool,
    /// The primary run reported nothing skipped or failed, and needed no
    /// gap-fill: every track's file is where GAMDL expects it.
    pub primary_clean: bool,
    /// The app is closing or the item was cancelled.
    pub stopping: bool,
    /// [`needs_gamdl_fallback`].
    pub needed: bool,
    /// The GAMDL version installed now is the one the primary run used
    /// (an upgrade in between could name or place files differently).
    pub same_gamdl_version: bool,
    /// The primary run's download folder is known.
    pub download_folder_known: bool,
}

/// Whether the GAMDL pass runs.
pub(crate) fn should_run_word_lyrics_pass(c: &WordLyricsPassConditions) -> bool {
    c.gamdl_can
        && c.enhanced_lrc
        && c.lyrics_files_on
        && c.links_allow
        && !c.is_library
        && c.primary_clean
        && !c.stopping
        && c.needed
        && c.same_gamdl_version
        && c.download_folder_known
}

/// The safety net: every primary file is still where it was. If any has
/// moved, GAMDL would not find it and would download the track again, so
/// the pass does not run.
pub(crate) fn primary_files_unmoved(files: &[PathBuf]) -> bool {
    !files.is_empty() && files.iter().all(|p| p.is_file())
}

/// Moves a path under `old_dir` to the same place under `new_dir` (after
/// the album folder was renamed). A path elsewhere is kept as it is.
pub(crate) fn remap_path(path: &Path, old_dir: &Path, new_dir: &Path) -> PathBuf {
    match path.strip_prefix(old_dir) {
        Ok(rel) => new_dir.join(rel),
        Err(_) => path.to_path_buf(),
    }
}

/// [`remap_path`] for a list.
pub(crate) fn remap_under_renamed_folder(
    files: &[PathBuf],
    old_dir: &Path,
    new_dir: &Path,
) -> Vec<PathBuf> {
    files
        .iter()
        .map(|p| remap_path(p, old_dir, new_dir))
        .collect()
}

/// How long the pass may run: one minute, plus six seconds a track, at
/// most fifteen minutes. It downloads no music, only lyrics.
pub(crate) fn word_lyrics_pass_deadline(track_count: usize) -> std::time::Duration {
    let secs = 60u64
        .saturating_add(6u64.saturating_mul(track_count as u64))
        .min(15 * 60);
    std::time::Duration::from_secs(secs)
}

/// How many of these audio files have a word-timed `.ttml` beside them.
pub(crate) fn count_word_timed_sidecars(files: &[PathBuf]) -> usize {
    files
        .iter()
        .filter(|audio| {
            std::fs::read_to_string(audio.with_extension("ttml"))
                .is_ok_and(|c| enhanced_lyrics_service::ttml_has_word_timing(&c))
        })
        .count()
}

/// Every audio file under `dir` (same depth and file kinds as
/// `count_audio_files_in_directory`).
pub(crate) fn audio_files_in(dir: &Path) -> std::collections::BTreeSet<PathBuf> {
    crate::utils::fs_walk::walk_dir_depth(dir, 10, |path| {
        if !path.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(path) {
            return None;
        }
        let ext = path.extension().and_then(|e| e.to_str())?;
        ["m4a", "m4v", "mp4"]
            .iter()
            .any(|x| ext.eq_ignore_ascii_case(x))
            .then(|| path.to_path_buf())
    })
    .into_iter()
    .collect()
}

/// Every `.ttml` lyrics file under `dir` (same depth as `audio_files_in`).
pub(crate) fn ttml_files_in(dir: &Path) -> std::collections::BTreeSet<PathBuf> {
    crate::utils::fs_walk::walk_dir_depth(dir, 10, |path| {
        if !path.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(path) {
            return None;
        }
        let ext = path.extension().and_then(|e| e.to_str())?;
        ext.eq_ignore_ascii_case("ttml").then(|| path.to_path_buf())
    })
    .into_iter()
    .collect()
}

/// How many paths in `after` were not in `before`.
pub(crate) fn new_audio_files(
    before: &std::collections::BTreeSet<PathBuf>,
    after: &std::collections::BTreeSet<PathBuf>,
) -> usize {
    after.difference(before).count()
}

// ============================================================
// Holding the extra-format copies (companions) back
// ============================================================

/// What enrichment tells the held companion task when it lets it start.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CompanionGateState {
    /// Enrichment has said "go" (or ended in any way at all).
    pub released: bool,
    /// The per-file renames ran, so every primary file has its final name
    /// (format suffix included). When false — enrichment ended early, or
    /// the format could not be told — the companions put a format suffix
    /// on every file, so none can land on a primary file's name.
    pub renames_done: bool,
    /// The album folder that really holds the primary files (after the
    /// advisory rename, or not), for album and song links. Companions
    /// write there, whatever GAMDL's template would make of the name.
    pub album_dir: Option<String>,
    /// The formats detected in the primary's files (one per format; native
    /// format priority can mix them within an album). Companions skip
    /// these formats, or on a mixed album keep the primary's own tracks.
    pub primary_codecs: Vec<SongCodec>,
    /// Each of THIS download's primary tracks: detected format, disc and
    /// track number. On a mixed album a companion requests only the tracks
    /// not already in its format.
    pub primary_tracks: Vec<(SongCodec, u32, u32)>,
    /// The album's tracks from Apple Music: disc, track, song ID — to build
    /// per-track song links.
    pub album_tracks: Vec<(u32, u32, String)>,
    /// What Apple Music said about the album itself: its name and the date
    /// it was last updated.
    pub album: GateAlbumFacts,
    /// The other album folders this download wrote songs into (a download
    /// can span several albums, #1446), whose songs enrichment tagged.
    /// Extra-format copies landing there get their format tags and their own
    /// album's stamps too. Not used for renames, lyrics or the advisory
    /// pass, which stay with the main folder.
    pub other_album_dirs: Vec<PathBuf>,
    /// The format of each of this download's primary files whose disc or
    /// track number could not be read: they cannot be requested on their
    /// own, and are counted in the "left without an extra copy" line.
    pub unplaceable: Vec<SongCodec>,
    /// The artist the main download's Lyricsfile step uses (the album
    /// artist from Apple Music), so the extra copies' `.lyrics` files name
    /// the same artist.
    pub lyricsfile_artist: Option<String>,
    /// The song title the main download's Lyricsfile step falls back to
    /// (the album's first song), likewise.
    pub lyricsfile_default_title: Option<String>,
}

/// Facts about the album, taken from Apple Music and handed to the
/// extra-format task through the gate.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct GateAlbumFacts {
    /// The album's name, to tell its existing copies from another release's
    /// when a file has no song ID.
    pub name: Option<String>,
    /// What each new extra-format copy is stamped with from its own album:
    /// the date the service last updated it (#1444, #1446). A copy is matched
    /// to an album by its song number; with one album every copy gets it.
    pub stamps: crate::services::metadata_tag_service::DownloadStamps,
}

/// The most other albums whose details are looked up for one download.
/// Beyond this the rest keep only the tags they were downloaded with.
pub(crate) const MAX_OTHER_ALBUM_LOOKUPS: usize = 25;

/// The albums, other than the primary one, that a download's links point at,
/// as `(storefront, album number)` pairs to look up (#1446).
///
/// Takes every link that reads as an album link (with or without a song) and
/// has an album number; library links, music videos and playlists are
/// ignored. Duplicates are removed (first in link order wins), the primary
/// album is left out, and the list is cut at [`MAX_OTHER_ALBUM_LOOKUPS`].
/// The second value is how many albums were left out by the cut.
pub(crate) fn other_albums_to_fetch(
    urls: &[String],
    primary_album_id: Option<&str>,
) -> (Vec<(String, String)>, usize) {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut wanted: Vec<(String, String)> = Vec::new();
    for url in urls {
        let Some(parsed) = crate::services::apple_music_api::parse_apple_music_url(url) else {
            continue;
        };
        if parsed.content_type != "album" || parsed.album_id.is_empty() {
            continue;
        }
        if Some(parsed.album_id.as_str()) == primary_album_id {
            continue;
        }
        if seen.insert(parsed.album_id.clone()) {
            wanted.push((parsed.storefront, parsed.album_id));
        }
    }
    let left_out = wanted.len().saturating_sub(MAX_OTHER_ALBUM_LOOKUPS);
    wanted.truncate(MAX_OTHER_ALBUM_LOOKUPS);
    (wanted, left_out)
}

/// Whether every link in a download points at the same Apple Music album
/// (album links and single-song links into one album). Only then is the
/// album's own last-updated date right for every extra-format copy: a
/// playlist or artist item can be rewritten into song links from several
/// albums, and one album's date must not be stamped on another's songs
/// (#1444). Empty, unreadable or library links give `false`.
pub(crate) fn links_share_one_album(urls: &[String]) -> bool {
    let mut first: Option<String> = None;
    for url in urls {
        let Some(parsed) = crate::services::apple_music_api::parse_apple_music_url(url) else {
            return false;
        };
        // Only album links (with or without `?i=` for one song) count: a
        // music-video link carries its own ID in `album_id`.
        if parsed.content_type != "album" || parsed.album_id.is_empty() {
            return false;
        }
        match &first {
            None => first = Some(parsed.album_id),
            Some(id) if *id == parsed.album_id => {}
            Some(_) => return false,
        }
    }
    first.is_some()
}

// ============================================================
// Other album folders (#1446)
// ============================================================

/// The most files tagged in a download's other album folders.
pub(crate) const MAX_OTHER_FOLDER_FILES: usize = 2000;

/// Every song number that belongs to this download: each song of every
/// album whose details were actually looked up (its current number and any
/// earlier ones). Albums past the lookup cap, or whose lookup failed, add
/// nothing, so their files are left alone.
pub(crate) fn download_song_ids(albums: &[AlbumMetadata]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for album in albums {
        for track in &album.tracks {
            if !track.song_id.is_empty() {
                ids.insert(track.song_id.clone());
            }
            ids.extend(metadata_tag_service::former_ids_from_json(&track.raw_json));
        }
    }
    ids
}

/// Whether the look through the download folder for this run's files is
/// needed: only when the download spans several albums and the run was GAMDL
/// 3.x (or the version is unknown). GAMDL 3.x reports no saved paths at its
/// normal log level; GAMDL 4.x reports every one, so there nothing is walked.
pub(crate) fn needs_fresh_walk(gamdl_version: Option<&str>, multi_album: bool) -> bool {
    if !multi_album {
        return false;
    }
    let major = gamdl_version
        .and_then(|v| v.trim().trim_start_matches('v').split('.').next())
        .and_then(|m| m.parse::<u32>().ok());
    major.is_none_or(|m| m < 4)
}

/// Every `.m4a` file under `output_dir` (up to ten levels down, like
/// [`audio_files_in`]) that this run wrote ([`written_this_run`]), as plain
/// paths under `output_dir`. The one place the download folder is looked
/// through for a GAMDL 3.x run's files in other album folders; the
/// song-number check in [`other_album_files`] filters what it finds. `stop`
/// is checked in every folder: when it says stop, the walk ends at once and
/// returns `None`.
pub(crate) fn fresh_m4a_under(
    output_dir: &Path,
    before: Option<&std::collections::HashSet<PathBuf>>,
    since: Option<std::time::SystemTime>,
    overwrite: bool,
    stop: &dyn Fn() -> bool,
) -> Option<Vec<PathBuf>> {
    fn walk(
        dir: &Path,
        depth: u32,
        before: Option<&std::collections::HashSet<PathBuf>>,
        since: Option<std::time::SystemTime>,
        overwrite: bool,
        stop: &dyn Fn() -> bool,
        found: &mut Vec<PathBuf>,
    ) -> bool {
        if depth > 10 {
            return true;
        }
        if stop() {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return true;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !walk(&path, depth + 1, before, since, overwrite, stop, found) {
                    return false;
                }
            } else if path.is_file()
                && !crate::utils::fs_safe::is_filesystem_sidecar(&path)
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("m4a"))
                && written_this_run(&path, before, since, overwrite)
            {
                found.push(path);
            }
        }
        true
    }
    let mut found = Vec::new();
    if !walk(output_dir, 0, before, since, overwrite, stop, &mut found) {
        return None;
    }
    found.sort();
    Some(found)
}

/// This run's files in album folders other than the main one, grouped by
/// folder (sorted), and whether some were left out by the cap.
///
/// Candidates are the paths the runs reported (`reported`) and the files
/// found by the look-through (`walked`). Each candidate is RESOLVED
/// ([`comparable`]) only to judge where the file really is (a `..` path, a
/// different letter case or a link is judged by its target) and is then
/// mapped back under the plain `output_dir` (`output_dir` joined with the
/// resolved path below the resolved download folder). The folders and files
/// RETURNED are always these plain paths, never resolved ones, so everything
/// downstream that compares them with plain listings of the download folder
/// (the extra-format tagging) keeps matching. A file counts only if it is an
/// existing `.m4a` under `output_dir` and not under `album_dir` (Step 1
/// already tags those), is not a filesystem leftover, **was written by this
/// run** ([`written_this_run`], using `before`, `since` and `overwrite`; a
/// file GAMDL skipped as already there never counts), and its own song
/// number (read by `song_id_of`) is one of those looked up for this download
/// (`download_song_ids`). At most [`MAX_OTHER_FOLDER_FILES`] files are
/// returned; the flag is true when an eligible file was left out.
#[allow(clippy::too_many_arguments)]
pub(crate) fn other_album_files(
    album_dir: &Path,
    output_dir: &Path,
    reported: &[PathBuf],
    walked: &[PathBuf],
    before: Option<&std::collections::HashSet<PathBuf>>,
    since: Option<std::time::SystemTime>,
    overwrite: bool,
    download_song_ids: &std::collections::HashSet<String>,
    song_id_of: impl Fn(&Path) -> Option<String>,
) -> (std::collections::BTreeMap<PathBuf, Vec<PathBuf>>, bool) {
    let resolved_album = comparable(album_dir);
    let resolved_output = comparable(output_dir);
    // Resolved path -> the plain path to return (first spelling wins), so the
    // same file reached two ways is handled once.
    let mut candidates: std::collections::BTreeMap<PathBuf, PathBuf> =
        std::collections::BTreeMap::new();
    for original in reported.iter().chain(walked.iter()) {
        let resolved = comparable(original);
        let Ok(below) = resolved.strip_prefix(&resolved_output) else {
            continue;
        };
        let plain = output_dir.join(below);
        candidates.entry(resolved).or_insert(plain);
    }
    let mut by_folder: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> =
        std::collections::BTreeMap::new();
    let mut total = 0usize;
    let mut left_out = false;
    for (resolved, plain) in candidates {
        let is_m4a = plain
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("m4a"));
        if !is_m4a
            || resolved.starts_with(&resolved_album)
            || !resolved.is_file()
            || crate::utils::fs_safe::is_filesystem_sidecar(&resolved)
            || !written_this_run(&plain, before, since, overwrite)
        {
            continue;
        }
        let Some(id) = song_id_of(&plain) else {
            continue;
        };
        if !download_song_ids.contains(&id) {
            continue;
        }
        if total >= MAX_OTHER_FOLDER_FILES {
            left_out = true;
            break;
        }
        if let Some(folder) = plain.parent() {
            by_folder
                .entry(folder.to_path_buf())
                .or_default()
                .push(plain.clone());
            total += 1;
        }
    }
    (by_folder, left_out)
}

/// Sets a flag when dropped. Held by [`walk_fresh_m4a_stoppable`] so that if
/// the task running it is aborted (the future is dropped mid-wait), the
/// blocking walk is told to stop and ends at its next folder.
pub(crate) struct StopOnDrop(pub(crate) Arc<AtomicBool>);

impl Drop for StopOnDrop {
    /// Tells the walk to stop.
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Looks through the download folder for this run's `.m4a` files
/// ([`fresh_m4a_under`]) on a blocking thread, stopping early when the item
/// is cancelled, the app is shutting down, or this future is dropped
/// (checked every quarter second from here, and in every folder inside the
/// walk). Returns `None` when it was stopped, never started because the item
/// was already cancelled, or the walk failed.
pub(crate) async fn walk_fresh_m4a_stoppable(
    root: &Path,
    before: Option<Arc<std::collections::HashSet<PathBuf>>>,
    since: Option<std::time::SystemTime>,
    overwrite: bool,
    queue: &QueueHandle,
    dl_id: &str,
    shutdown: &ShutdownSignal,
) -> Option<Vec<PathBuf>> {
    if shutdown.is_triggered() || queue.lock().await.is_cancelled(dl_id) {
        return None;
    }
    let stop = Arc::new(AtomicBool::new(false));
    // Dropped with this future, so an aborted task ends the walk too.
    let _stop_on_drop = StopOnDrop(Arc::clone(&stop));
    let walk_stop = Arc::clone(&stop);
    let shutdown_flag = shutdown.flag();
    let walk_root = root.to_path_buf();
    let mut handle = tokio::task::spawn_blocking(move || {
        fresh_m4a_under(&walk_root, before.as_deref(), since, overwrite, &|| {
            walk_stop.load(Ordering::Relaxed) || shutdown_flag.load(Ordering::Relaxed)
        })
    });
    loop {
        tokio::select! {
            result = &mut handle => return result.ok().flatten(),
            () = tokio::time::sleep(std::time::Duration::from_millis(250)) => {
                if shutdown.is_triggered() || queue.lock().await.is_cancelled(dl_id) {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Enrichment's end of the gate. Dropping it — an early return, a panic,
/// the enrichment deadline aborting the task — releases the companions
/// too, so they are never left waiting and the item never stalls.
pub(crate) struct CompanionGate(tokio::sync::watch::Sender<CompanionGateState>);

/// The companion task's end of the gate.
pub(crate) type CompanionGateWaiter = tokio::sync::watch::Receiver<CompanionGateState>;

/// A new gate, closed.
pub(crate) fn companion_gate() -> (CompanionGate, CompanionGateWaiter) {
    let (tx, rx) = tokio::sync::watch::channel(CompanionGateState::default());
    (CompanionGate(tx), rx)
}

impl CompanionGate {
    /// Records the folder that holds the primary files now.
    pub(crate) fn set_album_dir(&self, dir: Option<String>) {
        self.0.send_modify(|s| s.album_dir = dir);
    }

    /// Records the other album folders whose songs enrichment tagged.
    pub(crate) fn set_other_album_dirs(&self, dirs: Vec<PathBuf>) {
        self.0.send_modify(|s| s.other_album_dirs = dirs);
    }

    /// Records the artist and fallback title the main download's Lyricsfile
    /// step uses, for the extra copies' `.lyrics` files.
    pub(crate) fn set_lyricsfile_defaults(&self, artist: Option<String>, title: Option<String>) {
        self.0.send_modify(|s| {
            s.lyricsfile_artist = artist;
            s.lyricsfile_default_title = title;
        });
    }

    /// Lets the companions start, saying whether the per-file renames ran
    /// and which formats were detected in the primary's files.
    pub(crate) fn release(
        self,
        renames_done: bool,
        primary_codecs: Vec<SongCodec>,
        primary_tracks: Vec<(SongCodec, u32, u32)>,
        album_tracks: Vec<(u32, u32, String)>,
        unplaceable: Vec<SongCodec>,
        album: GateAlbumFacts,
    ) {
        self.0.send_modify(|s| {
            s.renames_done = renames_done;
            s.primary_codecs = primary_codecs;
            s.primary_tracks = primary_tracks;
            s.album_tracks = album_tracks;
            s.unplaceable = unplaceable;
            s.album = album;
        });
        // `Drop` sets `released`.
    }
}

impl Drop for CompanionGate {
    /// Marks the gate as released, so waiting companions are never left
    /// stuck if enrichment ends without calling `release`.
    fn drop(&mut self) {
        self.0.send_modify(|s| s.released = true);
    }
}

/// Waits for the gate to open, checking every half second whether the
/// wait should end anyway (`give_up`: cancelled, app closing, the task
/// aborted). Returns the gate's state, or `None` when it gave up.
pub(crate) async fn wait_for_companion_gate<F, Fut>(
    waiter: &mut CompanionGateWaiter,
    mut give_up: F,
) -> Option<CompanionGateState>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    loop {
        if waiter.borrow().released {
            return Some(waiter.borrow().clone());
        }
        tokio::select! {
            changed = waiter.changed() => {
                if changed.is_err() {
                    // The sender is gone without a final word: treat as
                    // released with nothing promised.
                    let mut state = waiter.borrow().clone();
                    state.released = true;
                    return Some(state);
                }
            }
            () = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                if give_up().await {
                    return None;
                }
            }
        }
    }
}

/// The album and compilation folder template for a companion, so it writes
/// into `album_dir` itself: the folder's path under `download_root`, as a
/// fixed template (braces doubled so GAMDL reads them literally). `None`
/// when `album_dir` is not inside `download_root` or is the root itself —
/// the companion then keeps its usual template.
///
/// A fixed path, not `{album} [Explicit]`, because GAMDL cleans names: an
/// album called "DAMN." is saved as `DAMN_`, which the suffix rule
/// (`DAMN_ [Explicit]`) and a template (`DAMN. [Explicit]`) spell
/// differently. GAMDL cleans each part of a fixed path the same way, and
/// a name it has already cleaned stays as it is.
pub(crate) fn literal_folder_template(album_dir: &Path, download_root: &Path) -> Option<String> {
    let rel = album_dir.strip_prefix(download_root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            std::path::Component::Normal(p) => {
                p.to_str().map(|s| s.replace('{', "{{").replace('}', "}}"))
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Whether the fixed folder template from [`literal_folder_template`] may
/// be used with these options: only when neither album file template
/// (single-disc, multi-disc) adds a folder level of its own.
///
/// With a file template such as `Disc {disc}/{track:02d} {title}`, the
/// folder enrichment found may be the disc folder rather than the album
/// folder, and GAMDL would add the template's own `Disc N` under the fixed
/// path (`Album/Disc 2/Disc 1/…`). Working out how many levels to strip
/// would depend on which folder enrichment happened to find, so the fixed
/// template is simply not used then, and the usual template applies.
///
/// The playlist and no-album file templates are not checked: the fixed
/// template replaces only the album and compilation FOLDER templates, and
/// GAMDL pairs those with the two album file templates alone.
pub(crate) fn file_templates_allow_fixed_folder(options: &GamdlOptions) -> bool {
    [
        options.single_disc_file_template.as_deref(),
        options.multi_disc_file_template.as_deref(),
    ]
    .into_iter()
    .flatten()
    .all(|t| !t.contains('/') && !t.contains('\\'))
}

/// What is known about one file in the folder, to decide whether it is
/// one of THIS download's own files ([`is_this_downloads_file`]).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FileEvidence {
    /// Written since the download started (judged at snapshot time).
    pub fresh: bool,
    /// Its name ends the way this run names its files
    /// ([`stem_matches_primary_name`]).
    pub name_matches_primary: bool,
    /// Whether MeedyaDL appended a format suffix to this run's templates.
    /// Without one, "named like this run" only means "no format suffix",
    /// which every plain AAC file and every unrenamed file also has.
    pub primary_has_suffix: bool,
    /// Whether its album matched Apple Music's (`None`: no album details,
    /// a playlist).
    pub album_matched: Option<bool>,
    /// For a single-song link: whether the file is that song (its disc and
    /// track). `None` for any other link.
    pub is_requested_song: Option<bool>,
}

/// Whether a file in the folder counts as one of THIS download's own
/// primary files — the ones renamed, and whose formats the extra-format
/// copies are judged against.
///
/// - Written by this run: always.
/// - A single-song link: otherwise only the requested song itself (album
///   not failing to match), never the album's other tracks in the folder.
/// - Otherwise named the way this run names its files: when the run's
///   templates carry a format suffix, unless its album failed to match;
///   when they carry none, only if its album positively matched.
pub(crate) fn is_this_downloads_file(e: &FileEvidence) -> bool {
    if e.fresh {
        return true;
    }
    if let Some(is_song) = e.is_requested_song {
        return is_song && e.album_matched != Some(false);
    }
    if !e.name_matches_primary {
        return false;
    }
    if e.primary_has_suffix {
        e.album_matched != Some(false)
    } else {
        e.album_matched == Some(true)
    }
}

/// What the final advisory pass after the extra-format copies may touch:
/// this download's own files (their final names) and whatever audio files
/// appeared in the folder after enrichment's renames (the companions'
/// copies). Set by enrichment right before it lets the companions start.
#[derive(Debug, Clone, Default)]
pub(crate) struct AdvisoryScope {
    /// The album folder.
    pub dir: PathBuf,
    /// This download's files, at their final names.
    pub this_download: Vec<PathBuf>,
    /// Every audio file in `dir` just before the companions started.
    pub before_companions: std::collections::BTreeSet<PathBuf>,
    /// Every `.ttml` file in `dir` at the same moment. It lets the lyrics
    /// step tell a `.ttml` an extra copy's download wrote from one that was
    /// already there, by whether it is listed here, not by its file time:
    /// a network or cloud folder can keep a clock that lags this computer's.
    pub ttml_before_companions: std::collections::BTreeSet<PathBuf>,
    /// When this download started. It lets the extra-copy step tell a lyric
    /// file this download made from an older one (by file time, the same
    /// rule [`fresh_at_snapshot`] uses). `None` (not known) means no file
    /// counts as made by this download, so nothing is ever replaced.
    pub download_started_at: Option<std::time::SystemTime>,
    /// The main download's files THIS run actually wrote (judged at snapshot
    /// time, before Step 0 retags), at their final names. A file merely named
    /// the way this run names its files (an older song GAMDL skipped) is in
    /// `this_download` but not here. Only these are ever upgraded from an
    /// extra copy. `None` (not known) means no main file is upgraded.
    pub written_this_run: Option<std::collections::BTreeSet<PathBuf>>,
}

/// The files the final advisory pass handles: this download's files that
/// still exist, plus audio files in the folder that were not there before
/// the companions started. Older files in the folder are never included.
pub(crate) fn advisory_pass_files(scope: &AdvisoryScope) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = scope
        .this_download
        .iter()
        .filter(|p| p.is_file())
        .cloned()
        .collect();
    for new in audio_files_in(&scope.dir).difference(&scope.before_companions) {
        if !files.contains(new) {
            files.push(new.clone());
        }
    }
    files
}

/// This download's files sorted for the companions (see
/// [`split_this_downloads_formats`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThisDownloadsFormats {
    /// Each of this download's tagged files with readable numbers:
    /// format, disc, track.
    pub tracks: Vec<(SongCodec, u32, u32)>,
    /// The format of each of this download's tagged files whose disc or
    /// track number could not be read.
    pub unplaceable: Vec<SongCodec>,
}

/// Picks this download's files (`this_download`) out of the tagged files
/// (`file_codecs`, every file in the folder) and sorts them by whether
/// their disc and track can be read (`numbers`). Formats recorded for
/// OTHER files feed nothing: in single-format mode every tagged file is
/// labelled with the requested format, so they would be wrong anyway.
pub(crate) fn split_this_downloads_formats(
    this_download: &[PathBuf],
    file_codecs: &[(PathBuf, SongCodec)],
    numbers: impl Fn(&Path) -> Option<(u32, u32)>,
) -> ThisDownloadsFormats {
    let mut out = ThisDownloadsFormats::default();
    for (path, codec) in file_codecs {
        if this_download.contains(path) {
            match numbers(path) {
                Some((d, t)) => out.tracks.push((codec.clone(), d, t)),
                None => out.unplaceable.push(codec.clone()),
            }
        }
    }
    out
}

/// The companions' format protection list: the format detected in every
/// file the deferred renames named by format, once each. Built from the
/// renames themselves so it always covers what was actually renamed —
/// including a file of this download that the album guard (#452) kept out
/// of the per-track list.
pub(crate) fn protection_codecs(
    renamed: &[crate::services::metadata_tag_service::DeferredRename],
) -> Vec<SongCodec> {
    let mut codecs: Vec<SongCodec> = Vec::new();
    for c in renamed.iter().filter_map(|r| r.detected.as_ref()) {
        if !codecs.contains(c) {
            codecs.push(c.clone());
        }
    }
    codecs
}

/// Song links for an extra-format copy: one per track of THIS download
/// whose primary file is NOT already in `codec` and that has no copy in
/// the folder already (`existing_copies`), built from the album's song IDs
/// (`album_tracks`: disc, track, song ID). Returns the links and the tracks
/// that could not be matched to a song ID.
pub(crate) fn other_track_links(
    codec: &SongCodec,
    primary_tracks: &[(SongCodec, u32, u32)],
    album_tracks: &[(u32, u32, String)],
    storefront: &str,
    album_id: &str,
    existing_copies: &[(u32, u32)],
) -> (Vec<String>, Vec<(u32, u32)>) {
    let already: Vec<(u32, u32)> = primary_tracks
        .iter()
        .filter(|(c, _, _)| c == codec)
        .map(|(_, d, t)| (*d, *t))
        .chain(existing_copies.iter().copied())
        .collect();
    let mut wanted: Vec<(u32, u32)> = Vec::new();
    for (_, d, t) in primary_tracks {
        if !already.contains(&(*d, *t)) && !wanted.contains(&(*d, *t)) {
            wanted.push((*d, *t));
        }
    }
    let mut links = Vec::new();
    let mut unmapped = Vec::new();
    for (d, t) in wanted {
        let link = album_tracks
            .iter()
            .find(|(ad, at, _)| *ad == d && *at == t)
            .and_then(|(_, _, id)| {
                crate::services::duplicate_detector::build_track_url(storefront, album_id, id)
            });
        match link {
            Some(l) => links.push(l),
            None => unmapped.push((d, t)),
        }
    }
    (links, unmapped)
}

/// What an existing file in the folder says about itself, to decide
/// whether it is this album's copy of a track ([`copy_belongs_to_album`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CopyEvidence {
    pub disc: u32,
    pub track: u32,
    /// Apple Music's song ID from the file's `cnID` atom, when present.
    pub song_id: Option<String>,
    /// The album name from the file's tags, when present.
    pub album: Option<String>,
}

/// Reads [`CopyEvidence`] from a file: disc and track (tags, else name),
/// the `cnID` song ID and the album name. `None` when the disc or track
/// cannot be read.
pub(crate) fn read_copy_evidence(path: &Path) -> Option<CopyEvidence> {
    let numbers = read_audio_numbers(path, false, None);
    let (disc, track) = numbers.disc.zip(numbers.track)?;
    let tag = mp4ameta::Tag::read_from_path(path).ok();
    let song_id = tag.as_ref().and_then(|t| {
        t.data_of(&mp4ameta::Fourcc(*b"cnID"))
            .find_map(|d| match d {
                mp4ameta::Data::BeSigned(bytes) | mp4ameta::Data::Reserved(bytes)
                    if !bytes.is_empty() && bytes.len() <= 8 =>
                {
                    let mut value: i64 = 0;
                    for b in bytes {
                        value = (value << 8) | i64::from(*b);
                    }
                    (value > 0).then(|| value.to_string())
                }
                _ => None,
            })
    });
    let album = tag.as_ref().and_then(|t| t.album().map(str::to_string));
    Some(CopyEvidence {
        disc,
        track,
        song_id,
        album,
    })
}

/// Whether an existing file is this album's copy of its track: its song
/// ID equals the album's song ID for that disc and track; or, without a
/// song ID, its album name matches the album's. Neither readable: not
/// counted (better to ask GAMDL than to wrongly skip a wanted copy — a
/// different release with the same name can share the folder).
pub(crate) fn copy_belongs_to_album(
    e: &CopyEvidence,
    album_tracks: &[(u32, u32, String)],
    album_name: Option<&str>,
) -> bool {
    if let Some(id) = &e.song_id {
        return album_tracks
            .iter()
            .any(|(d, t, sid)| *d == e.disc && *t == e.track && sid == id);
    }
    match (e.album.as_deref(), album_name) {
        (Some(file_album), Some(name)) => metadata_tag_service::album_names_match(file_album, name),
        _ => false,
    }
}

/// The tracks (disc, track) that already have THIS album's copy in the
/// attempt's format in `dir`: an .m4a whose name ENDS with that format's
/// suffix (`suffix`, e.g. `[Lossless]`) — so an advisory label before it,
/// as in `01 Song [Explicit] [Lossless]`, still counts — and that belongs to
/// this album ([`copy_belongs_to_album`]). GAMDL itself only checks the
/// exact name it would write (`01 Song [Lossless]`), so a labelled copy
/// from an earlier run would be downloaded again and kept as a numbered
/// duplicate.
pub(crate) fn tracks_with_existing_copy(
    dir: &Path,
    suffix: &str,
    evidence: impl Fn(&Path) -> Option<CopyEvidence>,
    album_tracks: &[(u32, u32, String)],
    album_name: Option<&str>,
) -> ExistingCopies {
    let mut found = ExistingCopies::default();
    if suffix.is_empty() {
        return found;
    }
    let tail = format!(" {suffix}");
    for file in metadata_tag_service::collect_m4a_files(&dir.to_string_lossy()) {
        let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(before) = stem.strip_suffix(&tail) else {
            continue;
        };
        if let Some(e) = evidence(&file) {
            if copy_belongs_to_album(&e, album_tracks, album_name) {
                // An advisory label just before the format suffix: a name
                // GAMDL's own check would not recognise.
                let lower = before.to_ascii_lowercase();
                if lower.ends_with(" [explicit]") || lower.ends_with(" [clean]") {
                    found.labelled = true;
                }
                if !found.tracks.contains(&(e.disc, e.track)) {
                    found.tracks.push((e.disc, e.track));
                }
            }
        }
    }
    found
}

/// The copies [`tracks_with_existing_copy`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ExistingCopies {
    /// The tracks (disc, track) with a copy.
    pub tracks: Vec<(u32, u32)>,
    /// At least one of them carries an advisory label
    /// (`01 Song [Explicit] [Lossless]`), so its name is not the one GAMDL
    /// would write and GAMDL's own check would not find it.
    pub labelled: bool,
}

/// How the pass ended, for its summary line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassEnd {
    /// GAMDL finished (whatever its exit code).
    Finished { success: bool },
    /// It could not start, ran out of time, or was stopped.
    Stopped,
}

/// The pass's one summary line.
pub(crate) fn word_lyrics_pass_summary(gained: usize, end: PassEnd) -> String {
    if gained > 0 {
        return format!("Word-level lyrics added for {gained} more track(s)");
    }
    match end {
        PassEnd::Finished { success: true } => {
            "Word-level lyrics: none available for the remaining tracks — keeping line-by-line lyrics"
                .to_string()
        }
        _ => "Word-level lyrics: the remaining tracks could not be checked this time".to_string(),
    }
}

/// Runs the GAMDL pass. Never fails the item: a failed, slow or stopped
/// run just leaves the line-by-line lyrics in place.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_word_lyrics_pass(
    app: &AppHandle,
    dl_id: &str,
    urls: &[String],
    options: &GamdlOptions,
    queue: &QueueHandle,
    shutdown: &ShutdownSignal,
    album_dir: &str,
    primary_files: &[PathBuf],
) {
    let before = count_word_timed_sidecars(primary_files);
    let audio_before = audio_files_in(Path::new(album_dir));

    let end = 'run: {
        let mut cmd = match gamdl_service::build_gamdl_command_public(app, urls, options) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Word-level lyrics pass could not be built for {dl_id}: {e}");
                break 'run PassEnd::Stopped;
            }
        };
        // Its output is not needed: what counts is the files it leaves.
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        // Stop the program if this task is dropped (the enrichment
        // deadline aborting it).
        cmd.kill_on_drop(true);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Word-level lyrics pass could not start for {dl_id}: {e}");
                break 'run PassEnd::Stopped;
            }
        };
        let deadline = tokio::time::Instant::now() + word_lyrics_pass_deadline(primary_files.len());
        loop {
            tokio::select! {
                status = child.wait() => {
                    break 'run match status {
                        Ok(s) => {
                            if !s.success() {
                                log::debug!("Word-level lyrics pass for {dl_id} exited with {s}");
                            }
                            PassEnd::Finished { success: s.success() }
                        }
                        Err(e) => {
                            log::debug!("Word-level lyrics pass for {dl_id} failed: {e}");
                            PassEnd::Stopped
                        }
                    };
                }
                () = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    let cancelled = { queue.lock().await.is_cancelled(dl_id) };
                    if cancelled || shutdown.is_triggered() || tokio::time::Instant::now() >= deadline {
                        log::info!(
                            "Word-level lyrics pass for {dl_id} stopped (cancelled={cancelled}, shutdown={}, deadline passed={})",
                            shutdown.is_triggered(),
                            tokio::time::Instant::now() >= deadline,
                        );
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        break 'run PassEnd::Stopped;
                    }
                }
            }
        }
    };

    // Only files that were not there before count — compared as a set of
    // paths, not a count, so nothing else changing in the folder can be
    // mistaken for (or hide) a re-download.
    let added = new_audio_files(&audio_before, &audio_files_in(Path::new(album_dir)));
    if added > 0 {
        log::warn!(
            "Word-level lyrics pass for {dl_id} added {added} audio file(s) (expected none)"
        );
        emit_download_warn(
            app,
            dl_id,
            &format!(
                "The word-level lyrics check also saved {added} music file(s) that were not there before — please check the album folder"
            ),
        );
    }
    let after = count_word_timed_sidecars(primary_files);
    emit_download_log(
        app,
        dl_id,
        &word_lyrics_pass_summary(after.saturating_sub(before), end),
    );
}

/// The country (storefront) code used for the word-by-word lyrics lookup.
///
/// Order of choice, the same one the rest of the app uses when it has to
/// work out a country:
///
/// 1. The country in the download's own link (`music.apple.com/gb/...`),
///    because that is the catalogue the person actually asked for.
/// 2. The Storefront setting, when it is not empty (it is empty while the
///    setting is on Auto-detect).
/// 3. The country of the computer's language settings (`detected`).
/// 4. `us`, which is always a valid Apple Music country.
///
/// Pure (the computer's own country is handed in) so it can be tested
/// without depending on the machine the tests run on.
pub(crate) fn lyrics_lookup_storefront(
    urls: &[String],
    setting: &str,
    detected: Option<String>,
) -> String {
    if let Some(from_link) = urls
        .iter()
        .find_map(|u| crate::services::apple_music_api::parse_apple_music_url(u))
        .map(|p| p.storefront)
        .filter(|s| !s.trim().is_empty())
    {
        return from_link.to_ascii_lowercase();
    }
    let setting = setting.trim();
    if !setting.is_empty() {
        return setting.to_ascii_lowercase();
    }
    detected
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "us".to_string())
}

#[cfg(test)]
mod tests {

    #[test]
    fn other_albums_to_fetch_takes_every_album_link() {
        let album = |sf: &str, id: u32| format!("https://music.apple.com/{sf}/album/x/{id}");
        let urls = vec![
            album("us", 1),
            // A song link into album 2, then album 2 again: one lookup.
            format!("{}?i=22", album("us", 2)),
            album("us", 2),
            // Album 3 in another storefront.
            album("gb", 3),
            // Not albums: a playlist, a music video, a library link, junk.
            "https://music.apple.com/us/playlist/x/pl.abc".to_string(),
            "https://music.apple.com/us/music-video/x/555".to_string(),
            "https://music.apple.com/us/library/albums/l.zzz".to_string(),
            "not a link".to_string(),
        ];
        let (wanted, left_out) = other_albums_to_fetch(&urls, Some("1"));
        assert_eq!(
            wanted,
            vec![
                ("us".to_string(), "2".to_string()),
                ("gb".to_string(), "3".to_string())
            ]
        );
        assert_eq!(left_out, 0);
        // Without a known primary album, album 1 is looked up too.
        assert_eq!(other_albums_to_fetch(&urls, None).0.len(), 3);

        // The cap: 30 albums gives 25 lookups and says 5 were left out.
        let many: Vec<String> = (100..130).map(|i| album("us", i)).collect();
        let (wanted, left_out) = other_albums_to_fetch(&many, None);
        assert_eq!(wanted.len(), MAX_OTHER_ALBUM_LOOKUPS);
        assert_eq!(left_out, 5);
        assert_eq!(wanted[0].1, "100");
        assert!(other_albums_to_fetch(&[], None).0.is_empty());
    }

    #[test]
    fn links_share_one_album_only_for_one_album() {
        let a = "https://music.apple.com/us/album/x/1111111111".to_string();
        let a_song = "https://music.apple.com/us/album/x/1111111111?i=2222222222".to_string();
        let b_song = "https://music.apple.com/us/album/y/3333333333?i=4444444444".to_string();
        assert!(links_share_one_album(std::slice::from_ref(&a)));
        assert!(links_share_one_album(&[a.clone(), a_song.clone()]));
        assert!(!links_share_one_album(&[a_song, b_song]));
        assert!(!links_share_one_album(&[]));
        assert!(!links_share_one_album(&["not a link".to_string()]));
        let video = "https://music.apple.com/us/music-video/v/5555555555".to_string();
        assert!(!links_share_one_album(std::slice::from_ref(&video)));
    }
    use super::*;

    const WORD_TTML: &str = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="0s" end="2s"><span begin="0s" end="1s">Hi</span></p></div></body></tt>"#;
    const LINE_TTML: &str = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="0s" end="2s">Hi</p></div></body></tt>"#;

    fn result(outcome: WordLyricsOutcome, looked_up: bool) -> TrackLyricsResult {
        TrackLyricsResult {
            outcome,
            looked_up,
            wrote_ttml: None,
        }
    }

    // ---------------- classification ----------------

    #[test]
    fn classify_covers_every_answer() {
        assert_eq!(
            classify::<String>(&Ok(Some(WORD_TTML.into())), true),
            WordLyricsOutcome::WordTimed
        );
        assert_eq!(
            classify::<String>(&Ok(Some(WORD_TTML.into())), false),
            WordLyricsOutcome::NotPlaced
        );
        // Lyrics with no word timing count as "Apple has none".
        assert_eq!(
            classify::<String>(&Ok(Some(LINE_TTML.into())), true),
            WordLyricsOutcome::AppleHasNone
        );
        assert_eq!(
            classify::<String>(&Ok(Some(LINE_TTML.into())), false),
            WordLyricsOutcome::AppleHasNone
        );
        assert_eq!(
            classify::<String>(&Ok(None), false),
            WordLyricsOutcome::AppleHasNone
        );
        assert_eq!(
            classify::<String>(&Err("HTTP 500".into()), false),
            WordLyricsOutcome::CouldNotAsk
        );
    }

    fn track(song_id: &str, disc: u32, number: u32) -> TrackMetadata {
        serde_json::from_value(serde_json::json!({
            "song_id": song_id,
            "isrc": null,
            "content_rating": null,
            "artist_id": null,
            "artist_name": null,
            "name": "T",
            "track_number": number,
            "disc_number": disc,
            "audio_traits": [],
            "is_apple_digital_master": null,
            "release_date": null,
            "composer_name": null,
            "duration_in_millis": null,
            "has_lyrics": null,
            "play_params_id": null,
            "url": null,
            "preview_url": null,
            "genre_names": [],
        }))
        .unwrap()
    }

    fn refused() -> apple_music_api::SyllableLyricsError {
        apple_music_api::SyllableLyricsError {
            status: Some(401),
            message: "Syllable-lyrics auth failed (HTTP 401)".into(),
        }
    }

    #[tokio::test]
    async fn a_saved_word_timed_file_is_recorded_so_the_lyrics_steps_can_reach_it() {
        let dir = tempfile::tempdir().unwrap();
        let song_a = dir.path().join("01 A.m4a");
        let song_b = dir.path().join("02 B.m4a");
        let tracks = [track("1", 1, 1), track("2", 1, 2)];
        let (results, _) = ask_each_track(
            vec![
                (&tracks[0], Some(song_a.with_extension("ttml"))),
                (&tracks[1], Some(song_b.with_extension("ttml"))),
            ],
            |id| {
                // Only the first answer has word timing.
                let answer = Ok(Some(
                    if id == "1" { WORD_TTML } else { LINE_TTML }.to_string(),
                ));
                async move { answer }
            },
            || false,
            std::time::Duration::ZERO,
        )
        .await;
        assert_eq!(results[0].wrote_ttml, Some(song_a.with_extension("ttml")));
        assert_eq!(results[1].wrote_ttml, None);
        let ran = Step1b::Ran(results);
        // The music file the file was written beside is found from the list.
        assert_eq!(
            files_step_1b_wrote_beside(&ran, &[song_a.clone(), song_b]),
            vec![song_a]
        );
        assert!(files_step_1b_wrote_beside(&Step1b::NotRun, &[]).is_empty());
    }

    #[test]
    fn videos_are_found_in_the_folder_and_its_disc_folders_only() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Disc 1");
        let deep = sub.join("deeper");
        std::fs::create_dir_all(&deep).unwrap();
        for p in [
            dir.path().join("a.m4v"),
            dir.path().join("b.MP4"),
            dir.path().join("song.m4a"),
            dir.path().join("._shadow.m4v"),
            sub.join("c.m4v"),
            deep.join("d.m4v"),
        ] {
            std::fs::write(p, b"x").unwrap();
        }
        let mut found = video_files_in_depth1(dir.path());
        found.sort();
        let mut expected = vec![
            dir.path().join("a.m4v"),
            dir.path().join("b.MP4"),
            sub.join("c.m4v"),
        ];
        expected.sort();
        assert_eq!(found, expected);
    }

    #[tokio::test]
    async fn a_song_id_containing_401_is_not_a_refused_sign_in() {
        // The message names the song; "1440140198" contains "401". Only
        // the HTTP status decides.
        let tracks = [track("1440140198", 1, 1), track("2", 1, 2)];
        let (results, stopped) = ask_each_track(
            vec![(&tracks[0], None), (&tracks[1], None)],
            |id| {
                let answer = if id == "1440140198" {
                    Err(apple_music_api::SyllableLyricsError {
                        status: Some(500),
                        message: "Syllable-lyrics API returned HTTP 500 for song 1440140198".into(),
                    })
                } else {
                    Ok(None)
                };
                async move { answer }
            },
            || false,
            std::time::Duration::ZERO,
        )
        .await;
        assert!(!stopped);
        assert_eq!(results[0].outcome, WordLyricsOutcome::CouldNotAsk);
        // The second track was still asked.
        assert_eq!(results[1].outcome, WordLyricsOutcome::AppleHasNone);
        assert!(!apple_music_api::SyllableLyricsError {
            status: None,
            message: "request failed for song 1440140198".into()
        }
        .is_refused_sign_in());
        assert!(refused().is_refused_sign_in());
    }

    #[tokio::test]
    async fn asking_writes_only_word_timed_lyrics_and_stops_on_a_refused_sign_in() {
        let dir = tempfile::tempdir().unwrap();
        let ttml = |n: &str| dir.path().join(format!("{n}.ttml"));
        std::fs::write(ttml("02"), LINE_TTML).unwrap();
        let tracks = [
            track("1", 1, 1),
            track("2", 1, 2),
            track("3", 1, 3),
            track("4", 1, 4),
            track("5", 1, 5),
        ];
        let to_ask = vec![
            (&tracks[0], Some(ttml("01"))), // word-timed: written
            (&tracks[1], Some(ttml("02"))), // line-only answer: NOT written
            (&tracks[2], None),             // word-timed but nowhere to put it
            (&tracks[3], Some(ttml("04"))), // refused sign-in
            (&tracks[4], Some(ttml("05"))), // never asked
        ];
        let asked = std::cell::RefCell::new(Vec::new());
        let (results, stopped) = ask_each_track(
            to_ask,
            |id| {
                asked.borrow_mut().push(id.to_string());
                let answer = match id {
                    "1" | "3" => Ok(Some(WORD_TTML.to_string())),
                    "2" => Ok(Some(LINE_TTML.to_string())),
                    _ => Err(refused()),
                };
                async move { answer }
            },
            || false,
            std::time::Duration::ZERO,
        )
        .await;

        let outcomes: Vec<_> = results.iter().map(|r| r.outcome).collect();
        assert_eq!(
            outcomes,
            vec![
                WordLyricsOutcome::WordTimed,
                WordLyricsOutcome::AppleHasNone,
                WordLyricsOutcome::NotPlaced,
                WordLyricsOutcome::CouldNotAsk,
                WordLyricsOutcome::CouldNotAsk,
            ]
        );
        assert!(stopped);
        assert!(results.iter().all(|r| r.looked_up));
        assert_eq!(*asked.borrow(), vec!["1", "2", "3", "4"]);
        assert_eq!(std::fs::read_to_string(ttml("01")).unwrap(), WORD_TTML);
        assert_eq!(std::fs::read_to_string(ttml("02")).unwrap(), LINE_TTML);
        assert!(!ttml("04").exists());
        assert!(needs_gamdl_fallback(&Step1b::Ran(results)));
    }

    #[tokio::test]
    async fn tracks_left_when_the_app_closes_could_not_be_checked() {
        let tracks = [track("1", 1, 1), track("2", 1, 2)];
        let (results, stopped) = ask_each_track(
            vec![(&tracks[0], None), (&tracks[1], None)],
            |_| async { Ok(None) },
            || true,
            std::time::Duration::ZERO,
        )
        .await;
        assert!(!stopped);
        assert!(results
            .iter()
            .all(|r| r.outcome == WordLyricsOutcome::CouldNotAsk));
    }

    #[test]
    fn the_gamdl_pass_is_needed_only_for_tracks_that_could_not_be_checked_or_saved() {
        assert!(needs_gamdl_fallback(&Step1b::NotRun));
        assert!(!needs_gamdl_fallback(&Step1b::Ran(vec![])));
        assert!(!needs_gamdl_fallback(&Step1b::Ran(vec![
            result(WordLyricsOutcome::WordTimed, true),
            result(WordLyricsOutcome::AppleHasNone, true),
            result(WordLyricsOutcome::AppleHasNone, false),
        ])));
        assert!(needs_gamdl_fallback(&Step1b::Ran(vec![
            result(WordLyricsOutcome::WordTimed, true),
            result(WordLyricsOutcome::CouldNotAsk, true),
        ])));
        assert!(needs_gamdl_fallback(&Step1b::Ran(vec![result(
            WordLyricsOutcome::NotPlaced,
            true
        )])));
        // Not asked because there was no sign-in: still needs the pass.
        assert!(needs_gamdl_fallback(&Step1b::Ran(vec![result(
            WordLyricsOutcome::CouldNotAsk,
            false
        )])));
    }

    #[test]
    fn the_summary_is_one_plain_line_and_none_when_nothing_was_looked_up() {
        assert_eq!(
            step_1b_summary(&[result(WordLyricsOutcome::WordTimed, false)], false),
            None
        );
        assert_eq!(
            step_1b_summary(&[result(WordLyricsOutcome::WordTimed, true)], false).unwrap(),
            "Word-level lyrics added for 1 track(s)"
        );
        assert!(
            step_1b_summary(&[result(WordLyricsOutcome::AppleHasNone, true)], false)
                .unwrap()
                .contains("has none")
        );
        let mixed = step_1b_summary(
            &[
                result(WordLyricsOutcome::WordTimed, true),
                result(WordLyricsOutcome::CouldNotAsk, true),
                result(WordLyricsOutcome::CouldNotAsk, true),
                result(WordLyricsOutcome::NotPlaced, true),
            ],
            true,
        )
        .unwrap();
        assert_eq!(
            mixed,
            "Word-level lyrics: 1 added, 2 could not be checked, 1 could not be saved (of 4 track(s)) — stopped early because Apple Music did not accept the sign-in"
        );
    }

    // ---------------- placement ----------------

    fn numbered(path: &str, disc: Option<u32>, track: Option<u32>) -> AudioFileNumbers {
        AudioFileNumbers {
            path: PathBuf::from(path),
            disc,
            track,
            fresh: false,
            name_matches_primary: false,
        }
    }

    #[test]
    fn file_names_give_disc_and_track() {
        assert_eq!(numbers_from_file_name("1-01 Title"), (Some(1), Some(1)));
        assert_eq!(
            numbers_from_file_name("2-13 Title [Lossless]"),
            (Some(2), Some(13))
        );
        assert_eq!(numbers_from_file_name("01 Title"), (None, Some(1)));
        assert_eq!(numbers_from_file_name("07"), (None, Some(7)));
        assert_eq!(numbers_from_file_name("Title"), (None, None));
        assert_eq!(numbers_from_file_name("1999.Title"), (None, None));
        assert_eq!(numbers_from_file_name("1-Title"), (None, None));
    }

    #[test]
    fn two_discs_each_with_a_track_one_are_told_apart() {
        let files = vec![
            numbered("/a/1-01 A.m4a", Some(1), Some(1)),
            numbered("/a/2-01 B.m4a", Some(2), Some(1)),
        ];
        assert_eq!(
            place_track(2, 1, &files, false),
            Placement::Found("/a/2-01 B.m4a".into())
        );
        assert_eq!(
            place_track(1, 1, &files, false),
            Placement::Found("/a/1-01 A.m4a".into())
        );
        assert_eq!(place_track(1, 2, &files, false), Placement::NotInDownload);
    }

    #[test]
    fn a_name_without_a_disc_number_is_placed_only_on_a_single_disc_album() {
        let files = vec![
            numbered("/a/01 A.m4a", None, Some(1)),
            numbered("/a/02 B.m4a", None, Some(2)),
        ];
        assert_eq!(
            place_track(1, 2, &files, true),
            Placement::Found("/a/02 B.m4a".into())
        );
        // Multi-disc: "01 " could be either disc, so it is not guessed.
        assert_eq!(place_track(1, 1, &files, false), Placement::Unclear);
        assert_eq!(place_track(2, 1, &files, false), Placement::Unclear);
    }

    #[test]
    fn two_files_claiming_the_same_track_are_unclear_unless_only_one_is_new() {
        let mut files = vec![
            numbered("/a/01 A [Lossless].m4a", Some(1), Some(1)),
            numbered("/a/01 A.m4a", Some(1), Some(1)),
        ];
        assert_eq!(place_track(1, 1, &files, true), Placement::Unclear);
        // The copy this download wrote wins over one left from before.
        files[1].fresh = true;
        assert_eq!(
            place_track(1, 1, &files, true),
            Placement::Found("/a/01 A.m4a".into())
        );
        files[0].fresh = true;
        assert_eq!(place_track(1, 1, &files, true), Placement::Unclear);
    }

    #[test]
    fn a_re_download_beside_an_older_copy_is_placed_by_its_name() {
        // Re-download into a folder that still holds the earlier run's
        // extra-format copy. Step 0 has rewritten both files' tags, so the
        // snapshot's freshness is what counts — and with overwrite off the
        // primary file may not be fresh either. The primary's template has
        // no format suffix, so the plain-named file is the primary one.
        let mut files = vec![
            numbered("/a/01 A [Lossless].m4a", Some(1), Some(1)),
            numbered("/a/01 A.m4a", Some(1), Some(1)),
        ];
        files[1].name_matches_primary = stem_matches_primary_name("01 A", None);
        files[0].name_matches_primary = stem_matches_primary_name("01 A [Lossless]", None);
        assert_eq!(
            place_track(1, 1, &files, true),
            Placement::Found("/a/01 A.m4a".into())
        );
    }

    #[test]
    fn only_the_end_of_a_name_decides_whether_it_is_the_primarys() {
        assert!(stem_matches_primary_name(
            "01 A [Dolby Atmos]",
            Some("[Dolby Atmos]")
        ));
        assert!(!stem_matches_primary_name(
            "01 A [Lossless]",
            Some("[Dolby Atmos]")
        ));
        assert!(!stem_matches_primary_name("01 A", Some("[Dolby Atmos]")));
        assert!(stem_matches_primary_name("01 A [Explicit]", None));
        // A label the person typed in the middle of their template does
        // not make the name a format-suffixed one.
        assert!(stem_matches_primary_name("01 [Lossless] A", None));
        assert!(!stem_matches_primary_name("01 A [Lossless]", None));
    }

    #[test]
    fn a_mixed_album_requests_only_the_tracks_not_already_in_that_format() {
        // Track 1 Atmos, track 2 ALAC, track 3 Atmos; track 3 has no song ID.
        let primary = vec![
            (SongCodec::Atmos, 1, 1),
            (SongCodec::Alac, 1, 2),
            (SongCodec::Atmos, 1, 3),
        ];
        let album = vec![(1, 1, "101".to_string()), (1, 2, "102".to_string())];
        let (links, unmapped) =
            other_track_links(&SongCodec::Alac, &primary, &album, "gb", "999", &[]);
        assert_eq!(
            links,
            vec!["https://music.apple.com/gb/album/_/999?i=101".to_string()]
        );
        assert_eq!(unmapped, vec![(1, 3)]);
        // Track 2 (already ALAC) is never requested.
        assert!(!links.iter().any(|l| l.ends_with("?i=102")));
    }

    #[test]
    fn a_renamed_file_the_album_guard_left_out_is_still_protected() {
        use crate::services::metadata_tag_service::{
            apply_deferred_file_renames, renames_complete,
        };
        // The primary's only file is ALAC, but its album tag failed the
        // #452 guard (say, edition wording). It is still renamed by format…
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("01 T.m4a");
        std::fs::write(&audio, b"a").unwrap();
        let renamed = apply_deferred_file_renames(
            std::slice::from_ref(&audio),
            false,
            &[],
            &[],
            &[(audio.clone(), SongCodec::Alac)],
            false,
        );
        assert!(renamed.iter().all(|r| r.original_path == audio));
        assert!(renames_complete(&renamed));
        assert!(dir.path().join("01 T [Lossless].m4a").exists());
        // …so ALAC is on the protection list, and an ALAC companion is
        // skipped rather than saving `01 T [Lossless]` over it.
        let protect = protection_codecs(&renamed);
        assert_eq!(protect, vec![SongCodec::Alac]);
        let attempt = GamdlOptions {
            song_codec_priority: Some("alac".into()),
            ..GamdlOptions::default()
        };
        assert_eq!(
            super::super::companion_attempt_naming(
                &SongCodec::Alac,
                true,
                &attempt,
                &super::super::PrimaryNaming::default(),
                &protect,
            ),
            Err(super::super::CompanionSkip::SameFormat)
        );
    }

    #[test]
    fn a_stale_copy_in_the_folder_is_not_this_downloads_format() {
        // This download: two Atmos tracks (one unreadable). In the folder
        // too: a stale `03 X [Lossless]` from an earlier download.
        let a = PathBuf::from("/m/01 A.m4a");
        let b = PathBuf::from("/m/02 B.m4a");
        let stale = PathBuf::from("/m/03 X [Lossless].m4a");
        let files = vec![
            (a.clone(), SongCodec::Atmos),
            (b.clone(), SongCodec::Atmos),
            (stale, SongCodec::Alac),
        ];
        let split = split_this_downloads_formats(&[a.clone(), b], &files, |p| {
            (p == a.as_path()).then_some((1, 1))
        });
        // The stale file feeds nothing.
        assert_eq!(split.tracks, vec![(SongCodec::Atmos, 1, 1)]);
        assert_eq!(split.unplaceable, vec![SongCodec::Atmos]);
    }

    #[test]
    fn a_labelled_copy_from_an_earlier_run_counts_as_already_there() {
        use crate::services::metadata_tag_service::test_support::minimal_m4a;
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        minimal_m4a(&d.join("01 Song [Explicit] [Lossless].m4a"), 1, 1, true);
        minimal_m4a(&d.join("02 Other [Dolby Atmos].m4a"), 2, 1, false);
        minimal_m4a(&d.join("03 Third.m4a"), 3, 1, false);
        // The test files carry no song ID or album name, so the evidence is
        // given by hand: track 1 belongs to this album.
        let tracks = vec![(1, 1, "101".to_string())];
        let mine = |p: &Path| {
            let n = read_audio_numbers(p, false, None);
            let (disc, track) = n.disc.zip(n.track)?;
            Some(CopyEvidence {
                disc,
                track,
                song_id: Some("101".to_string()),
                album: None,
            })
        };
        let found = tracks_with_existing_copy(d, "[Lossless]", mine, &tracks, None);
        assert_eq!(found.tracks, vec![(1, 1)]);
        assert!(
            found.labelled,
            "`01 Song [Explicit] [Lossless]` is labelled"
        );
        // An unlabelled copy is not.
        std::fs::remove_file(d.join("01 Song [Explicit] [Lossless].m4a")).unwrap();
        minimal_m4a(&d.join("01 Song [Lossless].m4a"), 1, 1, false);
        let found = tracks_with_existing_copy(d, "[Lossless]", mine, &tracks, None);
        assert_eq!(found.tracks, vec![(1, 1)]);
        assert!(!found.labelled);
        // An empty suffix (standard AAC) is left to GAMDL's own name check.
        assert!(tracks_with_existing_copy(d, "", mine, &tracks, None)
            .tracks
            .is_empty());
        // No song ID and no album name: not counted.
        let unknown = |p: &Path| {
            let n = read_audio_numbers(p, false, None);
            let (disc, track) = n.disc.zip(n.track)?;
            Some(CopyEvidence {
                disc,
                track,
                song_id: None,
                album: None,
            })
        };
        assert!(
            tracks_with_existing_copy(d, "[Lossless]", unknown, &tracks, Some("A"))
                .tracks
                .is_empty()
        );
    }

    #[test]
    fn a_different_release_sharing_the_folder_is_not_this_albums_copy() {
        let album = vec![(1, 1, "101".to_string()), (1, 2, "102".to_string())];
        let ev = |song_id: Option<&str>, name: Option<&str>| CopyEvidence {
            disc: 1,
            track: 1,
            song_id: song_id.map(str::to_string),
            album: name.map(str::to_string),
        };
        // The deluxe edition's track 1 (another song ID) in the same folder.
        assert!(!copy_belongs_to_album(
            &ev(Some("901"), Some("Album")),
            &album,
            Some("Album")
        ));
        assert!(copy_belongs_to_album(
            &ev(Some("101"), None),
            &album,
            Some("Album")
        ));
        // No song ID: the album name decides.
        assert!(copy_belongs_to_album(
            &ev(None, Some("Album")),
            &album,
            Some("Album")
        ));
        assert!(!copy_belongs_to_album(
            &ev(None, Some("Album (Deluxe)")),
            &album,
            Some("Album")
        ));
        // Nothing to go on: not counted.
        assert!(!copy_belongs_to_album(
            &ev(None, None),
            &album,
            Some("Album")
        ));
        assert!(!copy_belongs_to_album(
            &ev(None, Some("Album")),
            &album,
            None
        ));
    }

    #[test]
    fn the_song_id_atom_is_read_from_a_real_file() {
        use crate::services::metadata_tag_service::test_support::minimal_m4a;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("01 Song [Lossless].m4a");
        minimal_m4a(&path, 1, 1, false);
        let mut tag = mp4ameta::Tag::read_from_path(&path).unwrap();
        tag.set_data(
            mp4ameta::Fourcc(*b"cnID"),
            mp4ameta::Data::BeSigned(1_440_857_795_u32.to_be_bytes().to_vec()),
        );
        tag.set_album("Album");
        tag.write_to_path(&path).unwrap();
        let e = read_copy_evidence(&path).unwrap();
        assert_eq!(e.song_id.as_deref(), Some("1440857795"));
        assert_eq!(e.album.as_deref(), Some("Album"));
        assert_eq!((e.disc, e.track), (1, 1));
    }

    #[test]
    fn files_from_another_album_never_count_as_this_downloads() {
        let e = |fresh, name, suffix, album, song| FileEvidence {
            fresh,
            name_matches_primary: name,
            primary_has_suffix: suffix,
            album_matched: album,
            is_requested_song: song,
        };
        // Written by this download: always, even if the album guard failed.
        assert!(is_this_downloads_file(&e(
            true,
            false,
            true,
            Some(false),
            None
        )));
        // Named like this run's suffixed files: unless the album failed.
        assert!(is_this_downloads_file(&e(
            false,
            true,
            true,
            Some(true),
            None
        )));
        assert!(is_this_downloads_file(&e(false, true, true, None, None)));
        assert!(!is_this_downloads_file(&e(
            false,
            true,
            true,
            Some(false),
            None
        )));
        // No suffix in this run's templates (native priority): a plain name
        // proves nothing, so only a positive album match counts.
        assert!(is_this_downloads_file(&e(
            false,
            true,
            false,
            Some(true),
            None
        )));
        assert!(!is_this_downloads_file(&e(false, true, false, None, None)));
        // An older copy in another format.
        assert!(!is_this_downloads_file(&e(
            false,
            false,
            true,
            Some(true),
            None
        )));
    }

    #[test]
    fn a_single_song_link_counts_only_that_song() {
        let e = |fresh, song| FileEvidence {
            fresh,
            name_matches_primary: true,
            primary_has_suffix: false,
            album_matched: Some(true),
            is_requested_song: Some(song),
        };
        // The album's other tracks already in the folder (perhaps in the
        // companion's format) are not this download's — so no false
        // "already in that format".
        assert!(!is_this_downloads_file(&e(false, false)));
        assert!(is_this_downloads_file(&e(false, true)));
        assert!(is_this_downloads_file(&e(true, false)));
    }

    #[test]
    fn the_final_advisory_pass_leaves_older_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("05 Old.m4a");
        let mine = dir.path().join("01 A [Explicit].m4a");
        std::fs::write(&older, b"o").unwrap();
        std::fs::write(&mine, b"a").unwrap();
        let scope = AdvisoryScope {
            dir: dir.path().to_path_buf(),
            this_download: vec![mine.clone()],
            before_companions: audio_files_in(dir.path()),
            ttml_before_companions: ttml_files_in(dir.path()),
            download_started_at: None,
            written_this_run: None,
        };
        // A companion copy arrives afterwards.
        let copy = dir.path().join("01 A [Lossless].m4a");
        std::fs::write(&copy, b"c").unwrap();
        let mut files = advisory_pass_files(&scope);
        files.sort();
        assert_eq!(files, vec![mine, copy]);
        assert!(!advisory_pass_files(&scope).contains(&older));
    }

    #[test]
    fn freshness_is_judged_from_the_snapshot_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("01 A.m4a");
        std::fs::write(&path, b"a").unwrap();
        let files = vec![path.clone()];
        let started = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        assert!(fresh_at_snapshot(&files, Some(started)).contains(&path));
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        assert!(fresh_at_snapshot(&files, Some(later)).is_empty());
        assert!(fresh_at_snapshot(&files, None).is_empty());
    }

    #[test]
    fn an_unreadable_file_makes_missing_tracks_unclear_not_absent() {
        let files = vec![
            numbered("/a/01 A.m4a", Some(1), Some(1)),
            numbered("/a/Bonus.m4a", None, None),
        ];
        assert_eq!(place_track(1, 2, &files, true), Placement::Unclear);
        assert_eq!(
            place_track(1, 1, &files, true),
            Placement::Found("/a/01 A.m4a".into())
        );
    }

    #[test]
    fn tags_tell_two_discs_apart_even_when_both_files_start_with_01() {
        use crate::services::metadata_tag_service::test_support::minimal_m4a;
        let dir = tempfile::tempdir().unwrap();
        let d1 = dir.path().join("Disc 1");
        let d2 = dir.path().join("Disc 2");
        std::fs::create_dir(&d1).unwrap();
        std::fs::create_dir(&d2).unwrap();
        let first = d1.join("01 Opening.m4a");
        let second = d2.join("01 Encore.m4a");
        minimal_m4a(&first, 1, 1, false);
        minimal_m4a(&second, 1, 2, false);
        let files = vec![
            read_audio_numbers(&first, false, None),
            read_audio_numbers(&second, false, None),
        ];
        assert_eq!(place_track(2, 1, &files, false), Placement::Found(second));
        assert_eq!(place_track(1, 1, &files, false), Placement::Found(first));
    }

    #[test]
    fn unreadable_tags_fall_back_to_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2-05 Song.m4a");
        std::fs::write(&path, b"not an mp4").unwrap();
        let mut expected = numbered(path.to_str().unwrap(), Some(2), Some(5));
        expected.name_matches_primary = true;
        assert_eq!(read_audio_numbers(&path, false, None), expected);
        assert!(read_audio_numbers(&path, true, None).fresh);
    }

    // ---------------- the GAMDL pass: options ----------------

    fn primary() -> GamdlOptions {
        GamdlOptions {
            song_codec_priority: Some("atmos,alac".into()),
            overwrite: Some(true),
            save_cover: Some(true),
            cover_size: Some(3000),
            save_playlist: Some(true),
            no_synced_lyrics: Some(true),
            synced_lyrics_format: Some(LyricsFormat::Lrc),
            album_folder_template: Some("{album_artist}/{album}".into()),
            compilation_folder_template: Some("Compilations/{album}".into()),
            single_disc_file_template: Some("{track:02d} {title} [Dolby Atmos]".into()),
            output_path: Some("/Music".into()),
            ..GamdlOptions::default()
        }
    }

    #[test]
    fn the_pass_keeps_names_and_never_overwrites_or_saves_extras() {
        let plan = plan_word_lyrics_pass(&primary(), "/Resolved/Music");
        // The primary run's resolved folder, given explicitly.
        assert_eq!(plan.output_path.as_deref(), Some("/Resolved/Music"));
        assert_eq!(plan.overwrite, Some(false));
        assert_eq!(plan.save_cover, Some(false));
        assert_eq!(plan.save_playlist, None);
        assert_eq!(plan.no_synced_lyrics, Some(false));
        assert_eq!(plan.synced_lyrics_format, Some(LyricsFormat::Ttml));
        assert_eq!(plan.word_timed_lyrics_file, Some(true));
        // The primary's own (format-suffixed) file template and folder.
        assert_eq!(
            plan.single_disc_file_template.as_deref(),
            Some("{track:02d} {title} [Dolby Atmos]")
        );
        assert_eq!(
            plan.album_folder_template.as_deref(),
            Some("{album_artist}/{album}")
        );
        assert_eq!(plan.song_codec_priority.as_deref(), Some("atmos,alac"));

        let _lock = gamdl_capabilities::capability_cache_test_lock();
        gamdl_capabilities::set_detected_version(Some("4.0.2".to_string()));
        let args = plan.to_cli_args_v4();
        for never in [
            "--overwrite",
            "--save-cover",
            "--file-cover-size",
            "--save-playlist",
            "--no-lyrics-file",
        ] {
            assert!(!args.iter().any(|a| a == never), "sent {never}");
        }
        let at = args
            .iter()
            .position(|a| a == "--lyrics-file-format")
            .unwrap();
        assert_eq!(args[at + 1], "enhanced-ttml");
        // The embedded cover size is sent as the primary run sent it.
        assert!(args.iter().any(|a| a == "--embedded-cover-size"));

        // 4.0.1 would get plain ttml (and the pass never runs there).
        gamdl_capabilities::set_detected_version(Some("4.0.1".to_string()));
        let args = plan.to_cli_args_v4();
        let at = args
            .iter()
            .position(|a| a == "--lyrics-file-format")
            .unwrap();
        assert_eq!(args[at + 1], "ttml");
        gamdl_capabilities::set_detected_version(None);
    }

    #[test]
    fn the_pass_keeps_the_primarys_own_folder_templates() {
        // It runs before the album folder is renamed, so no suffix is
        // added to any template.
        let plan = plan_word_lyrics_pass(&primary(), "/Music");
        assert_eq!(
            plan.album_folder_template.as_deref(),
            Some("{album_artist}/{album}")
        );
        assert_eq!(
            plan.compilation_folder_template.as_deref(),
            Some("Compilations/{album}")
        );
    }

    #[test]
    fn the_gamdl_3_lyrics_recovery_uses_the_primarys_own_names_and_folder() {
        // #1436: built from the primary run, not from Settings, so a format
        // label MeedyaDL added to the file names is kept and the lyrics
        // land beside the files the primary run really wrote.
        let plan = plan_lyrics_recovery_run(&primary(), "/Resolved/Music", LyricsFormat::Srt);
        assert_eq!(plan.output_path.as_deref(), Some("/Resolved/Music"));
        assert_eq!(
            plan.single_disc_file_template.as_deref(),
            Some("{track:02d} {title} [Dolby Atmos]")
        );
        assert_eq!(
            plan.album_folder_template.as_deref(),
            Some("{album_artist}/{album}")
        );
        // Lyrics only, in the asked-for format, nothing extra saved.
        assert_eq!(plan.synced_lyrics_only, Some(true));
        assert_eq!(plan.synced_lyrics_format, Some(LyricsFormat::Srt));
        assert_eq!(plan.no_synced_lyrics, Some(false));
        assert_eq!(plan.save_cover, Some(false));
        assert_eq!(plan.save_playlist, None);
        assert_eq!(plan.word_timed_lyrics_file, None);
        // Overwrite is always off, even when the primary run had it on, so
        // the word-by-word lyrics files Step 2 made are never replaced.
        assert_eq!(plan.overwrite, Some(false));
        // The settings file is left out, so its `save_cover = true` cannot
        // make this run save a fresh Cover picture.
        assert_eq!(plan.no_config_file, Some(true));
        let args = plan.to_cli_args();
        assert!(!args
            .iter()
            .any(|a| a == "--overwrite" || a == "--save-cover"));
        assert!(args.iter().any(|a| a == "--no-config-file"));
    }

    // ---------------- the GAMDL pass: when it runs ----------------

    fn all_yes() -> WordLyricsPassConditions {
        WordLyricsPassConditions {
            gamdl_can: true,
            enhanced_lrc: true,
            lyrics_files_on: true,
            links_allow: true,
            is_library: false,
            primary_clean: true,
            stopping: false,
            needed: true,
            same_gamdl_version: true,
            download_folder_known: true,
        }
    }

    #[test]
    fn the_pass_runs_only_when_every_condition_holds() {
        assert!(should_run_word_lyrics_pass(&all_yes()));
        let each_no: [fn(&mut WordLyricsPassConditions); 10] = [
            |c| c.gamdl_can = false,
            |c| c.enhanced_lrc = false,
            |c| c.lyrics_files_on = false,
            |c| c.links_allow = false,
            |c| c.is_library = true,
            |c| c.primary_clean = false,
            |c| c.stopping = true,
            |c| c.needed = false,
            |c| c.same_gamdl_version = false,
            |c| c.download_folder_known = false,
        ];
        for (i, change) in each_no.iter().enumerate() {
            let mut c = all_yes();
            change(&mut c);
            assert!(
                !should_run_word_lyrics_pass(&c),
                "condition {i} should stop it"
            );
        }
    }

    #[test]
    fn the_installed_version_decides_whether_gamdl_can_run_the_pass() {
        let _lock = gamdl_capabilities::capability_cache_test_lock();
        for (version, can) in [("3.9.1", false), ("4.0.1", false), ("4.0.2", true)] {
            gamdl_capabilities::set_detected_version(Some(version.to_string()));
            assert_eq!(
                gamdl_capabilities::supports(
                    gamdl_capabilities::GamdlFeature::GamdlFourWordTimedLyricsFile
                ),
                can,
                "{version}"
            );
        }
        gamdl_capabilities::set_detected_version(None);
    }

    #[test]
    fn only_album_and_song_links_allow_the_pass() {
        assert!(links_allow_word_lyrics_pass(&[
            "https://music.apple.com/us/album/x/1440857781".into()
        ]));
        assert!(links_allow_word_lyrics_pass(&[
            "https://music.apple.com/us/album/x/1440857781?i=1440857795".into()
        ]));
        assert!(!links_allow_word_lyrics_pass(&[
            "https://music.apple.com/us/artist/x/368433979".into()
        ]));
        assert!(!links_allow_word_lyrics_pass(&[
            "https://music.apple.com/us/music-video/x/1445789862".into()
        ]));
        assert!(!links_allow_word_lyrics_pass(&[
            "https://music.apple.com/us/playlist/x/pl.u-GgA567VCXX".into()
        ]));
        assert!(!links_allow_word_lyrics_pass(&[]));
    }

    // ---------------- the GAMDL pass: files ----------------

    #[test]
    fn after_the_folder_rename_the_files_are_still_unmoved_but_not_after_a_file_rename() {
        let root = tempfile::tempdir().unwrap();
        let old_dir = root.path().join("Album");
        std::fs::create_dir(&old_dir).unwrap();
        let audio = old_dir.join("01 T.m4a");
        std::fs::write(&audio, b"a").unwrap();
        let snapshot = vec![audio];

        // The album folder rename first.
        let new_dir = root.path().join("Album [Explicit]");
        std::fs::rename(&old_dir, &new_dir).unwrap();
        assert!(!primary_files_unmoved(&snapshot));
        let mapped = remap_under_renamed_folder(&snapshot, &old_dir, &new_dir);
        assert!(primary_files_unmoved(&mapped));
        // Where the pass's suffixed folder template predicts the file.
        assert_eq!(mapped[0], new_dir.join("01 T.m4a"));

        // A per-file rename, and the pass must refuse.
        std::fs::rename(&mapped[0], new_dir.join("01 T [Explicit].m4a")).unwrap();
        assert!(!primary_files_unmoved(&mapped));
        assert!(!primary_files_unmoved(&[]));
    }

    #[test]
    fn word_timed_sidecars_are_counted_for_the_given_tracks_only() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("01 A.m4a");
        let b = dir.path().join("02 B.m4a");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        std::fs::write(dir.path().join("01 A.ttml"), WORD_TTML).unwrap();
        std::fs::write(dir.path().join("02 B.ttml"), LINE_TTML).unwrap();
        // A companion's word-timed file is not one of ours.
        std::fs::write(dir.path().join("02 B [Lossless].ttml"), WORD_TTML).unwrap();
        assert_eq!(count_word_timed_sidecars(&[a, b]), 1);
    }

    #[test]
    fn the_deadline_scales_and_is_capped() {
        assert_eq!(word_lyrics_pass_deadline(0).as_secs(), 60);
        assert_eq!(word_lyrics_pass_deadline(10).as_secs(), 120);
        assert_eq!(word_lyrics_pass_deadline(10_000).as_secs(), 15 * 60);
    }

    #[test]
    fn the_pass_summary_says_what_happened() {
        assert_eq!(
            word_lyrics_pass_summary(3, PassEnd::Stopped),
            "Word-level lyrics added for 3 more track(s)"
        );
        assert!(
            word_lyrics_pass_summary(0, PassEnd::Finished { success: true })
                .contains("none available")
        );
        assert!(
            word_lyrics_pass_summary(0, PassEnd::Finished { success: false })
                .contains("could not be checked")
        );
        assert!(word_lyrics_pass_summary(0, PassEnd::Stopped).contains("could not be checked"));
    }

    // ---------------- new files, companion gate, companion folder ----------------

    #[test]
    fn only_files_that_were_not_there_before_count_as_new() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("01 A.m4a"), b"a").unwrap();
        std::fs::write(dir.path().join("01 A.ttml"), b"t").unwrap();
        let before = audio_files_in(dir.path());
        assert_eq!(before.len(), 1);
        // A rewritten file is not new; a new path is.
        std::fs::write(dir.path().join("01 A.m4a"), b"aa").unwrap();
        assert_eq!(new_audio_files(&before, &audio_files_in(dir.path())), 0);
        std::fs::write(dir.path().join("02 B.m4a"), b"b").unwrap();
        assert_eq!(new_audio_files(&before, &audio_files_in(dir.path())), 1);
        // One file leaving while another arrives is still one new file
        // (a count would have said nothing changed).
        std::fs::remove_file(dir.path().join("01 A.m4a")).unwrap();
        assert_eq!(new_audio_files(&before, &audio_files_in(dir.path())), 1);
    }

    #[tokio::test]
    async fn the_gate_opens_on_release_with_what_enrichment_said() {
        let (gate, mut waiter) = companion_gate();
        gate.set_album_dir(Some("/Music/A/Album [Explicit]".into()));
        gate.set_lyricsfile_defaults(Some("Artist".into()), Some("First".into()));
        let waiting =
            tokio::spawn(
                async move { wait_for_companion_gate(&mut waiter, || async { false }).await },
            );
        tokio::task::yield_now().await;
        gate.release(
            true,
            vec![SongCodec::Alac],
            vec![(SongCodec::Alac, 1, 1)],
            vec![(1, 1, "100".to_string())],
            vec![SongCodec::Aac],
            GateAlbumFacts {
                name: Some("Album".to_string()),
                stamps: crate::services::metadata_tag_service::DownloadStamps {
                    albums: vec![crate::services::metadata_tag_service::AlbumStamp {
                        last_modified_date: Some("2024-03-15T07:12:45Z".to_string()),
                        tracks: Vec::new(),
                        ..Default::default()
                    }],
                    single_album_download: true,
                },
            },
        );
        let state = waiting.await.unwrap().unwrap();
        assert!(state.released);
        assert!(state.renames_done);
        assert_eq!(state.primary_codecs, vec![SongCodec::Alac]);
        assert_eq!(state.primary_tracks, vec![(SongCodec::Alac, 1, 1)]);
        assert_eq!(state.album_tracks, vec![(1, 1, "100".to_string())]);
        assert_eq!(state.album.name.as_deref(), Some("Album"));
        let (stamped, _) = state.album.stamps.pick(None, 1, Some(1)).unwrap();
        assert_eq!(
            stamped.last_modified_date.as_deref(),
            Some("2024-03-15T07:12:45Z")
        );
        assert_eq!(state.unplaceable, vec![SongCodec::Aac]);
        assert_eq!(state.lyricsfile_artist.as_deref(), Some("Artist"));
        assert_eq!(state.lyricsfile_default_title.as_deref(), Some("First"));
        assert_eq!(
            state.album_dir.as_deref(),
            Some("/Music/A/Album [Explicit]")
        );
    }

    #[tokio::test]
    async fn an_enrichment_that_ends_early_still_opens_the_gate_without_promising_renames() {
        let (gate, mut waiter) = companion_gate();
        gate.set_album_dir(Some("/Music/A/Album".into()));
        // An early return, a panic or an abort all just drop it.
        drop(gate);
        let state = wait_for_companion_gate(&mut waiter, || async { false })
            .await
            .unwrap();
        assert!(state.released);
        assert!(!state.renames_done, "companions must suffix every file");
        assert_eq!(state.album_dir.as_deref(), Some("/Music/A/Album"));
    }

    #[tokio::test]
    async fn a_cancel_while_waiting_ends_the_wait() {
        let (_gate, mut waiter) = companion_gate();
        let state = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            wait_for_companion_gate(&mut waiter, || async { true }),
        )
        .await
        .expect("gave up within the half-second check");
        assert_eq!(state, None);
    }

    #[test]
    fn companions_get_the_real_folder_as_a_fixed_template() {
        let root = Path::new("/Music");
        assert_eq!(
            literal_folder_template(Path::new("/Music/Kendrick Lamar/DAMN_ [Explicit]"), root)
                .as_deref(),
            Some("Kendrick Lamar/DAMN_ [Explicit]")
        );
        // Braces are doubled so GAMDL reads them as text.
        assert_eq!(
            literal_folder_template(Path::new("/Music/A/{Live}"), root).as_deref(),
            Some("A/{{Live}}")
        );
        assert_eq!(literal_folder_template(Path::new("/Music"), root), None);
        assert_eq!(
            literal_folder_template(Path::new("/Elsewhere/A"), root),
            None
        );
    }

    #[test]
    fn a_file_template_with_its_own_folder_level_turns_the_fixed_folder_off() {
        let plain = GamdlOptions {
            single_disc_file_template: Some("{track:02d} {title}".into()),
            multi_disc_file_template: Some("{disc}-{track:02d} {title}".into()),
            // A playlist file template is not used for album links.
            playlist_file_template: Some(
                "Playlists/{playlist_artist}/{playlist_title} ({playlist_id})".into(),
            ),
            ..GamdlOptions::default()
        };
        assert!(file_templates_allow_fixed_folder(&plain));
        assert!(file_templates_allow_fixed_folder(&GamdlOptions::default()));
        let disc_folders = GamdlOptions {
            multi_disc_file_template: Some("Disc {disc}/{track:02d} {title}".into()),
            ..plain.clone()
        };
        assert!(!file_templates_allow_fixed_folder(&disc_folders));
        let single = GamdlOptions {
            single_disc_file_template: Some("CD/{track:02d} {title}".into()),
            ..plain
        };
        assert!(!file_templates_allow_fixed_folder(&single));
    }

    #[test]
    fn a_video_on_a_lagging_clock_is_still_this_downloads_by_the_pre_run_list() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("Older.m4v");
        std::fs::write(&older, b"x").unwrap();
        let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&older)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        let before = videos_before_run(dir.path());
        assert!(before.contains(&std::fs::canonicalize(&older).unwrap()));
        // The new video's file time lags: it was "written" long before the
        // download started.
        let new = dir.path().join("New.m4v");
        std::fs::write(&new, b"y").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&new)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        let since = Some(std::time::SystemTime::now());
        let now = video_files_in_depth1(dir.path());
        assert_eq!(now.len(), 2);
        let mine = this_downloads_videos(&now, Some(&before), since);
        assert_eq!(mine, vec![new.clone()]);
        // Without the list the time rule alone decides, as before.
        assert!(this_downloads_videos(&now, None, since).is_empty());
    }

    #[test]
    fn a_video_rewritten_by_this_download_counts_even_if_it_was_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("V.m4v");
        std::fs::write(&video, b"x").unwrap();
        let before = videos_before_run(dir.path());
        let since = Some(std::time::SystemTime::now() - std::time::Duration::from_secs(60));
        let mine = this_downloads_videos(std::slice::from_ref(&video), Some(&before), since);
        assert_eq!(mine, vec![video]);
    }

    #[test]
    fn lyrics_storefront_prefers_the_link_then_setting_then_detected_then_us() {
        let link = vec!["https://music.apple.com/gb/album/x/123".to_string()];
        assert_eq!(
            lyrics_lookup_storefront(&link, "de", Some("fr".into())),
            "gb"
        );
        assert_eq!(lyrics_lookup_storefront(&[], "DE", Some("fr".into())), "de");
        assert_eq!(lyrics_lookup_storefront(&[], "", Some("fr".into())), "fr");
        assert_eq!(lyrics_lookup_storefront(&[], "  ", None), "us");
        // A link that is not an Apple Music link gives no country.
        let other = vec!["https://example.com/x".to_string()];
        assert_eq!(lyrics_lookup_storefront(&other, "", None), "us");
    }
}
