// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Lyrics for the extra-format copies (companions), #1440.
//
// An extra-format copy (for example the Lossless copy next to a Dolby Atmos
// download) is a separate file with its own name. Before this module it got
// only whatever lyrics GAMDL happened to write beside it, converted by a
// folder-wide pass that also touched the main download's files and older
// files. The main download, meanwhile, may have received better lyrics
// (word-by-word timing from enrichment's own lookup or from GAMDL's word-level
// pass), and none of that reached the copies.
//
// This module gives every NEW copy of THIS download the same lyrics
// treatment as the main download's matching track:
//
// 1. The copy is matched to the main download's file of the same song (the
//    song ID first, else the same folder plus disc and track number).
// 2. A source is chosen per track: the main download's lyrics only when they
//    are strictly better than the copy's own, otherwise the copy's own.
// 3. Every lyric and subtitle file the person switched on is built again for
//    the copy, in the same order and under the same switches as enrichment's
//    Steps 2 to 2g. Only the `.ttml` source file is ever copied across; all
//    other files are rebuilt from it, because the `.ass` file contains the
//    file name and a finished `.vtt` or `.lyrics` file in the main
//    download's folder may be older or edited by the person.
//
// The per-file steps themselves are in `lyrics_steps.rs`, shared with the
// main download (#1441); this module decides which copy gets which source
// and drives the steps.
//
// The other way round works too (#1442): when an extra copy came with better
// lyrics than the main download's file of the same song, the main file is
// brought up to them first, so every other copy of that song gets them too.
// This only ever touches files THIS download made (see [`upgrade_main`]).
//
// Nothing here deletes a file (a test checks the source for delete calls).
// A `.ttml`, `.vtt`, `.ass` or `.lyrics` file that was already on disk is
// never replaced for a copy; for the main download's upgrade only one that
// this download made may be. The `.lrc` and Rich `.srt` files are always
// written fresh, exactly as the main download's Steps 2 and 2d do.

use super::lyrics_pass::{
    audio_files_in, fresh_at_snapshot, place_track, read_audio_numbers, read_copy_evidence,
    AdvisoryScope, AudioFileNumbers, Placement,
};
use super::lyrics_steps::{
    quality_of_texts, read_lyric_texts, step_ass, step_embed_subtitles, step_enhanced_lrc,
    step_lyricsfile, step_rich_srt, step_webvtt, write_file, StepInput,
};
// Re-exported so this module's own tests (and the code below) keep using the
// short name; the type itself moved to `lyrics_steps.rs` with the steps.
pub(crate) use super::lyrics_steps::LyricsQuality;
use super::*;
use crate::utils::file_locks::FileWriteLocks;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

// ============================================================
// Pure decisions
// ============================================================

/// Where a copy's lyrics come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// The main download's lyrics (strictly better than the copy's own).
    Primary,
    /// The copy's own lyrics files.
    Own,
    /// There is nothing to use.
    Nothing,
}

/// Chooses the lyrics source for one copy. `primary` is `None` when the
/// copy could not be matched to a main-download file (or that file is
/// unclear), in which case only the copy's own lyrics can be used.
///
/// The rule in one line: use the main download's lyrics only when they are
/// strictly better than the copy's own; otherwise the copy's own (which
/// means no write is needed when they are equal, and the copy's own
/// word-by-word file beats a line-level main download).
pub(crate) fn choose_source(primary: Option<LyricsQuality>, own: LyricsQuality) -> Source {
    if let Some(p) = primary {
        if p > own {
            return Source::Primary;
        }
    }
    if own > LyricsQuality::None {
        Source::Own
    } else {
        Source::Nothing
    }
}

/// Whether a copy's `.ttml` file may be written from the main download's.
///
/// A copy's `.ttml` that is already there may be replaced only when it was
/// written by this run of extra-format downloads (an older one could be the
/// person's own edit, and nothing already on disk is touched). With none
/// there, one is created only when "keep separate lyrics files" is on:
/// with it off the folder is kept tidy and the lyrics are used in memory.
pub(crate) fn may_write_ttml(keep: bool, own_exists: bool, own_written_this_run: bool) -> bool {
    if own_exists {
        own_written_this_run
    } else {
        keep
    }
}

/// What is known about one audio file, to pair a copy with the main
/// download's file of the same song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Evidence {
    /// The file.
    pub path: PathBuf,
    /// The folder it is in.
    pub dir: PathBuf,
    /// Apple Music's song ID from the file's tags, when present.
    pub song_id: Option<String>,
    /// Disc number, when it could be read.
    pub disc: Option<u32>,
    /// Track number, when it could be read.
    pub track: Option<u32>,
}

/// Reads Apple Music's song ID (the `cnID` tag) from a file, `None` when it
/// is missing or the file cannot be read.
fn read_song_id(path: &Path) -> Option<String> {
    let tag = mp4ameta::Tag::read_from_path(path).ok()?;
    let id = tag
        .data_of(&mp4ameta::Fourcc(*b"cnID"))
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
        });
    id
}

/// Reads [`Evidence`] for a file: song ID, disc and track from its tags,
/// the numbers falling back to the file name.
pub(crate) fn read_evidence(path: &Path) -> Evidence {
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    // The song ID is read on its own, so it is kept even when the disc and
    // track numbers cannot be read.
    let song_id = read_song_id(path);
    if let Some(e) = read_copy_evidence(path) {
        return Evidence {
            path: path.to_path_buf(),
            dir,
            song_id: e.song_id.or(song_id),
            disc: Some(e.disc),
            track: Some(e.track),
        };
    }
    let n = read_audio_numbers(path, false, None);
    Evidence {
        path: path.to_path_buf(),
        dir,
        song_id,
        disc: n.disc,
        track: n.track,
    }
}

/// Whether the album is a single-disc one: every disc in the album's track
/// list is 1, or (with no track list) every main-download file with a known
/// disc is on disc 1. Decides whether a file with no disc number counts as
/// disc 1.
pub(crate) fn single_disc_album(
    album_tracks: &[(u32, u32, String)],
    primaries: &[Evidence],
) -> bool {
    if album_tracks.is_empty() {
        primaries.iter().all(|p| p.disc.is_none_or(|d| d == 1))
    } else {
        album_tracks.iter().all(|(disc, _, _)| *disc == 1)
    }
}

/// Finds the main download's file for one extra copy, or `None` when there
/// is no single clear answer.
///
/// 1. The song ID, when both have one: exactly one main file with the same
///    ID is the match; more than one is unclear.
/// 2. Otherwise the same folder plus disc and track number (never across
///    folders, so two albums of a playlist whose tracks share numbers cannot
///    be mixed up), judged by the same `place_track` rule as enrichment.
///    Two files whose song IDs are both known and different are never
///    paired, whatever their numbers say.
///
/// `song_id_only` is set for a copy that sits in a different folder from the
/// main download's: it may only be matched by song ID.
pub(crate) fn match_primary(
    copy: &Evidence,
    primaries: &[Evidence],
    single_disc: bool,
    song_id_only: bool,
) -> Option<usize> {
    if let Some(id) = &copy.song_id {
        let same: Vec<usize> = primaries
            .iter()
            .enumerate()
            .filter(|(_, p)| p.song_id.as_ref() == Some(id))
            .map(|(i, _)| i)
            .collect();
        match same.len() {
            1 => return Some(same[0]),
            0 => {}
            _ => return None,
        }
    }
    if song_id_only {
        return None;
    }
    let disc = copy.disc.or(if single_disc { Some(1) } else { None })?;
    let track = copy.track?;
    let in_folder: Vec<(usize, AudioFileNumbers)> = primaries
        .iter()
        .enumerate()
        .filter(|(_, p)| p.dir == copy.dir)
        .filter(|(_, p)| match (&p.song_id, &copy.song_id) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        })
        .map(|(i, p)| {
            (
                i,
                AudioFileNumbers {
                    path: p.path.clone(),
                    disc: p.disc,
                    track: p.track,
                    fresh: false,
                    name_matches_primary: false,
                },
            )
        })
        .collect();
    let files: Vec<AudioFileNumbers> = in_folder.iter().map(|(_, f)| f.clone()).collect();
    match place_track(disc, track, &files, single_disc) {
        Placement::Found(path) => in_folder
            .iter()
            .find(|(_, f)| f.path == path)
            .map(|(i, _)| *i),
        Placement::NotInDownload | Placement::Unclear => None,
    }
}

/// What happened over a whole run, for the one activity-log line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    /// How many copies were looked at (or would have been, when stopped).
    pub total: usize,
    /// Copies that used the main download's lyrics.
    pub from_main: usize,
    /// Of those, how many had word-by-word timing.
    pub from_main_word_timed: usize,
    /// Copies that used their own lyrics.
    pub own: usize,
    /// Copies with no lyrics to use at all.
    pub none: usize,
    /// Copies that had a lyrics source but nothing was added (every file
    /// was already there, or a write failed).
    pub unchanged: usize,
    /// The run was stopped (cancel, quit or deadline) before it finished.
    pub stopped: bool,
    /// Songs of the main download that were given better lyrics by one of
    /// their extra copies.
    pub main_upgraded: usize,
}

/// The plain-English activity-log line for a run, or `None` when nothing
/// was done and there is nothing to say.
pub(crate) fn summary_line(r: &Summary) -> Option<String> {
    let done = r.from_main + r.own + r.none + r.unchanged;
    let upgraded_line = (r.main_upgraded > 0).then(|| {
        let noun = if r.main_upgraded == 1 {
            "song"
        } else {
            "songs"
        };
        format!(
            "{} {noun} in the main download got better lyrics from an extra copy.",
            r.main_upgraded
        )
    });
    if r.stopped {
        let stopped = (r.total > 0 || r.main_upgraded > 0).then(|| {
            format!(
                "Adding lyrics to the extra copies stopped early ({done} of {} done).",
                r.total
            )
        });
        return match (stopped, upgraded_line) {
            (Some(s), Some(u)) => Some(format!("{s} {u}")),
            (Some(s), None) => Some(s),
            (None, u) => u,
        };
    }
    let added = r.from_main + r.own;
    if added == 0 {
        return upgraded_line;
    }
    let noun = if added == 1 { "copy" } else { "copies" };
    let word_by_word = if r.from_main > 0 && r.from_main_word_timed == r.from_main {
        "word-by-word "
    } else {
        ""
    };
    let mut parts: Vec<String> = Vec::new();
    if r.from_main > 0 {
        parts.push(format!(
            "{} used the main download's {word_by_word}lyrics",
            r.from_main
        ));
    }
    if r.own > 0 {
        parts.push(format!("{} used their own", r.own));
    }
    let mut line = format!(
        "Lyrics added to {added} extra {noun}: {}.",
        parts.join(", ")
    );
    if r.none > 0 {
        line.push_str(&format!(" {} had no lyrics to use.", r.none));
    }
    if r.unchanged > 0 {
        line.push_str(&format!(" {} needed nothing added.", r.unchanged));
    }
    if let Some(extra) = upgraded_line {
        line.push(' ');
        line.push_str(&extra);
    }
    Some(line)
}

// ============================================================
// File work (synchronous; called directly inside the companion task)
// ============================================================

/// Whether a file was written at or after `since` (two seconds of slack
/// for file systems that store times coarsely). Used only for copies saved
/// OUTSIDE the album folder, where no snapshot of the folder was taken, so
/// file times are all there is to go on. (A folder whose clock lags this
/// computer's can fool it; inside the album folder the snapshot is used.)
fn written_since(path: &Path, since: SystemTime) -> bool {
    let since = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|m| m >= since)
}

/// What one copy ended up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyOutcome {
    /// Used the main download's lyrics; the flag says word-by-word.
    FromMain { word_timed: bool },
    /// Used its own.
    Own,
    /// A lyrics source existed, but nothing was written or embedded (every
    /// file it would make was already there, or a write failed).
    Unchanged,
    /// Nothing to use.
    Nothing,
    /// A stop (cancel, quit or deadline) cut this copy's steps short. It is
    /// not counted as done, whatever files were written before the stop.
    Stopped,
}

/// Gives one copy its lyrics and subtitle files. `primary_audio` is the
/// main download's file of the same song (at its final name), when one was
/// matched. `written_by_this_run` says whether a `.ttml` already on disk
/// was written by this run of the extra-format downloads (not an older one).
///
/// Follows enrichment's Steps 2, 2c, 2d, 2e, 2f and 2g, in that order and
/// under the same switches. Failures are only logged (debug), never raised.
///
/// `stop` is asked before each step, so a cancel, quit or deadline ends the
/// work between steps rather than only between copies. The copy counts as
/// having got lyrics only if a file was really written or embedded. If a
/// stop actually skipped a step, the result is `CopyOutcome::Stopped`.
/// `artist` and `default_title` are what the main download's Lyricsfile
/// step uses (the album artist and the album's first song name).
///
/// `replace_existing` says whether an existing `.vtt`, `.ass` or `.lyrics`
/// file may be replaced. Extra copies and the main download's own catch-up
/// never replace one (`|_| false`); the main download's upgrade from a better
/// copy allows it only for files this download made.
#[allow(clippy::too_many_arguments)]
fn process_copy(
    copy: &Path,
    primary_audio: Option<&Path>,
    s: &AppSettings,
    written_by_this_run: &dyn Fn(&Path) -> bool,
    replace_existing: &dyn Fn(&Path) -> bool,
    artist: Option<&str>,
    default_title: Option<&str>,
    stop_requested: &dyn Fn() -> bool,
    source_ttml_only: bool,
) -> CopyOutcome {
    // Remember whether a stop really skipped a step: `stop` is only asked
    // when the step would otherwise run.
    let cut_short = std::cell::Cell::new(false);
    let stop = || {
        let yes = stop_requested();
        if yes {
            cut_short.set(true);
        }
        yes
    };
    let mut did_work = false;
    let keep = s.keep_lyrics_sidecar;
    let own_texts = read_lyric_texts(copy);
    let own_quality = quality_of_texts(&own_texts);
    // For an upgrade (`source_ttml_only`), only the source's `.ttml` is
    // used: it is the one file checked as written by this download, while
    // a `.lrc`/`.srt` beside it could be a leftover nothing can date.
    let primary_texts = primary_audio.map(|p| {
        let t = read_lyric_texts(p);
        if source_ttml_only {
            super::lyrics_steps::LyricTexts {
                ttml: t.ttml,
                ..super::lyrics_steps::LyricTexts::default()
            }
        } else {
            t
        }
    });
    let primary_quality = primary_texts.as_ref().map(quality_of_texts);

    let source = choose_source(primary_quality, own_quality);
    let (texts, from_main) = match source {
        Source::Nothing => return CopyOutcome::Nothing,
        Source::Own => (own_texts.clone(), false),
        Source::Primary => (primary_texts.clone().unwrap_or_default(), true),
    };

    // Route A: bring the main download's source files across. Only the
    // `.ttml` (and, for a main download that has only `.lrc`/`.srt`, those
    // two) are copied; everything else is rebuilt below.
    if from_main {
        if let Some(ttml) = &texts.ttml {
            let target = copy.with_extension("ttml");
            let exists = target.exists();
            if may_write_ttml(keep, exists, exists && written_by_this_run(&target)) {
                did_work |= write_file(&target, ttml);
            }
        } else {
            for (ext, text) in [("lrc", &texts.lrc), ("srt", &texts.srt)] {
                let target = copy.with_extension(ext);
                if let (true, false, Some(text)) = (keep, target.exists(), text) {
                    did_work |= write_file(&target, text);
                }
            }
        }
    }

    let stem = copy
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_string();
    let ttml: Option<&str> = texts.ttml.as_deref();

    // The steps themselves live in `lyrics_steps.rs`, shared with the main
    // download. Each is run only when its switch is on and no stop is asked
    // for; an existing `.vtt`, `.ass` or `.lyrics` is replaced only when the
    // caller allows it for that file.
    let input = StepInput {
        audio: copy,
        stem: &stem,
        source: &texts,
        from_other: from_main,
        keep,
        replace_existing,
    };

    // Step 2: Enhanced LRC. The `.lrc` file only when separate files are
    // kept; the lyrics always go inside the music file.
    if s.enhanced_lrc && !stop() {
        did_work |= step_enhanced_lrc(&input).wrote;
    }
    // Step 2c: WebVTT (TTML, then SRT, then LRC), skipped if one exists.
    if s.generate_webvtt && keep && !stop() {
        did_work |= step_webvtt(&input).wrote;
    }
    // Step 2d: rich SRT (TTML, then WebVTT); replaces a plain SRT.
    if s.generate_rich_srt && keep && !stop() {
        did_work |= step_rich_srt(&input).wrote;
    }
    // Step 2e: embed the subtitle files in the music file (not gated on
    // keeping separate files).
    if s.embed_subtitles && !stop() {
        did_work |= step_embed_subtitles(&input).wrote;
    }
    // Step 2f: ASS (TTML, then WebVTT), titled with the COPY's own name.
    if s.generate_ass && keep && !stop() {
        did_work |= step_ass(&input).wrote;
    }
    // Step 2g: Lyricsfile, skipped if one exists.
    if s.generate_lyricsfile && keep && !stop() {
        did_work |= step_lyricsfile(
            &input,
            default_title.unwrap_or("Untitled"),
            artist.unwrap_or("Unknown Artist"),
        )
        .wrote;
    }

    if cut_short.get() {
        CopyOutcome::Stopped
    } else if !did_work {
        CopyOutcome::Unchanged
    } else if from_main {
        CopyOutcome::FromMain {
            word_timed: ttml.is_some_and(super::enhanced_lyrics_service::ttml_has_word_timing),
        }
    } else {
        CopyOutcome::Own
    }
}

/// The audio files that are new copies of THIS download: the `.m4a` files
/// in the album folder that were not there before the extra-format
/// downloads started and are not the main download's own files, plus
/// (only when `extra_dir` is a folder outside the album folder) `.m4a`
/// files there written since `since`. Older files in the folder are never
/// included.
pub(crate) fn this_downloads_copies(
    scope: &AdvisoryScope,
    extra_dir: Option<&Path>,
    since: SystemTime,
) -> Vec<PathBuf> {
    let is_m4a = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("m4a"))
    };
    let mut copies: Vec<PathBuf> = audio_files_in(&scope.dir)
        .difference(&scope.before_companions)
        .filter(|p| is_m4a(p) && !scope.this_download.contains(p))
        .cloned()
        .collect();
    if let Some(extra) = extra_dir {
        if !extra.starts_with(&scope.dir) {
            let candidates: Vec<PathBuf> = audio_files_in(extra)
                .into_iter()
                .filter(|p| is_m4a(p) && !scope.this_download.contains(p))
                .collect();
            let fresh = fresh_at_snapshot(&candidates, Some(since));
            for p in candidates {
                if fresh.contains(&p) && !copies.contains(&p) {
                    copies.push(p);
                }
            }
        }
    }
    copies.sort();
    copies
}

/// The folder the extra copies were saved in, when it is NOT inside the
/// main download's album folder (the predicted-suffix folder case); `None`
/// otherwise.
pub(crate) fn resolve_extra_dir(
    output_dir: &Path,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
    scope_dir: &Path,
) -> Option<PathBuf> {
    let artist = artist_hint.filter(|s| !s.is_empty());
    let album = album_hint.filter(|s| !s.is_empty());
    if artist.is_none() || album.is_none() || !output_dir.is_dir() {
        return None;
    }
    let found = PathBuf::from(super::find_album_directory(output_dir, artist, album)?);
    (!found.starts_with(scope_dir) && !scope_dir.starts_with(&found)).then_some(found)
}

/// Whether any lyric or subtitle conversion is switched on. With none,
/// there is nothing to do and the folder is not even looked at.
fn any_conversion_switched_on(settings: &AppSettings) -> bool {
    settings.enhanced_lrc
        || settings.generate_webvtt
        || settings.generate_rich_srt
        || settings.generate_ass
        || settings.generate_lyricsfile
        || settings.embed_subtitles
}

/// Whether the extra copies have anything to do at all: a conversion is
/// switched on, or separate lyrics files are kept. With only the second, the
/// one thing to do is copy a better `.ttml` across from the main download
/// (or up from a better copy to the main download). With neither, the folder
/// is not even looked at.
fn copy_work_switched_on(settings: &AppSettings) -> bool {
    any_conversion_switched_on(settings) || settings.keep_lyrics_sidecar
}

// ---- Without a recorded scope (#1441) ----

/// What was in the album folder at one moment: its music files and its
/// `.ttml` files. Taken just before the extra-format copies start, for the
/// case where enrichment never recorded a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderSnapshot {
    /// The folder.
    pub dir: PathBuf,
    /// Every music file in it then.
    pub audio: std::collections::BTreeSet<PathBuf>,
    /// Every `.ttml` file in it then.
    pub ttml: std::collections::BTreeSet<PathBuf>,
}

impl FolderSnapshot {
    /// Notes what is in `dir` now.
    pub(crate) fn take(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            audio: audio_files_in(dir),
            ttml: super::lyrics_pass::ttml_files_in(dir),
        }
    }
}

/// Finds the album folder the extra copies went into: the folder enrichment
/// recorded when it is known and still there, else the one the artist and
/// album names point at. `None` when neither gives a folder.
pub(crate) fn resolve_album_folder(
    output_dir: &Path,
    known: Option<&Path>,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
) -> Option<PathBuf> {
    if let Some(dir) = known.filter(|d| d.is_dir()) {
        return Some(dir.to_path_buf());
    }
    if !output_dir.is_dir() {
        return None;
    }
    // An empty name is the same as a missing one: it would otherwise be
    // joined onto the download folder and point at the whole library (#839).
    let artist = artist_hint.filter(|s| !s.is_empty());
    let album = album_hint.filter(|s| !s.is_empty());
    super::find_album_directory(output_dir, artist, album).map(PathBuf::from)
}

/// The scope the lyrics step uses when enrichment never recorded one (the
/// gate was dropped, or the failure path). It holds no main-download files,
/// so every new copy is given its own lyrics.
///
/// What counts as "already there before the extra copies started": the
/// snapshot, when it was taken of the same folder; otherwise every file not
/// written since `since` (two seconds of slack). Files the snapshot lists
/// are never new, whatever their file time says.
pub(crate) fn fallback_scope(
    dir_after: &Path,
    before: Option<&FolderSnapshot>,
    since: SystemTime,
) -> AdvisoryScope {
    let (audio, ttml) = match before {
        Some(snapshot) if snapshot.dir == dir_after => {
            (snapshot.audio.clone(), snapshot.ttml.clone())
        }
        _ => (
            audio_files_in(dir_after)
                .into_iter()
                .filter(|p| !written_since(p, since))
                .collect(),
            super::lyrics_pass::ttml_files_in(dir_after)
                .into_iter()
                .filter(|p| !written_since(p, since))
                .collect(),
        ),
    };
    AdvisoryScope {
        dir: dir_after.to_path_buf(),
        this_download: Vec::new(),
        before_companions: audio,
        ttml_before_companions: ttml,
        // Nothing from the main download is known here, so there is nothing
        // it could be upgraded from a copy either.
        download_started_at: None,
        written_this_run: None,
    }
}

/// The settings to use for finishing the main download's own files: with
/// no artist known, the Lyricsfile step is left out. A `.lyrics` file is
/// kept once written, so a placeholder name would stay for good.
fn settings_for_main_files(settings: &AppSettings, lyrics_artist: Option<&str>) -> AppSettings {
    if settings.generate_lyricsfile && lyrics_artist.is_none() {
        AppSettings {
            generate_lyricsfile: false,
            ..settings.clone()
        }
    } else {
        settings.clone()
    }
}

/// Folds one file's result into the summary for the main download's own
/// files. Returns `true` when the run must end (a stop cut it short).
fn tally_main(summary: &mut Summary, outcome: CopyOutcome) -> bool {
    match outcome {
        CopyOutcome::Own => summary.own += 1,
        CopyOutcome::Unchanged => summary.unchanged += 1,
        CopyOutcome::Nothing => summary.none += 1,
        CopyOutcome::FromMain { .. } => {}
        CopyOutcome::Stopped => {
            summary.stopped = true;
            return true;
        }
    }
    false
}

/// Finishes the lyrics of the main download's own files when enrichment
/// stopped before it was done with them. Each file gets its own lyrics
/// files converted, in the same order and under the same switches as the
/// extra copies; every step can safely be repeated (`.lrc` and `.srt` are
/// rewritten, embeds are overwritten, `.vtt`, `.ass` and `.lyrics` files
/// already there are kept). Older files in the folder are not in `files`.
///
/// This version takes no write locks, so it is for tests and for code that
/// holds the locks itself. The app uses [`run_main_files_own_locked`].
#[cfg(test)]
pub(crate) fn run_main_files_own(
    settings: &AppSettings,
    files: &[PathBuf],
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> Summary {
    let settings = &settings_for_main_files(settings, lyrics_artist);
    if !any_conversion_switched_on(settings) {
        return Summary::default();
    }
    let files: Vec<&PathBuf> = files.iter().filter(|p| p.is_file()).collect();
    let mut summary = Summary {
        total: files.len(),
        ..Summary::default()
    };
    for file in files {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let outcome = process_copy(
            file,
            None,
            settings,
            &|_| false,
            &|_| false,
            lyrics_artist,
            default_title,
            stop,
            false,
        );
        if tally_main(&mut summary, outcome) {
            return summary;
        }
    }
    summary
}

/// Holds the files of a locked catch-up that are not finished yet. If the
/// catch-up's future is dropped part-way (the extra-copy task is aborted
/// while it waits for a file's lock), dropping this puts the unfinished
/// files back on the shared "pending" list, merged with anything already
/// there, so the completion task's own catch-up still finds them. A file is
/// only taken off with [`PendingRestore::done`], after its writing has
/// finished. The files are put back even when the catch-up was stopped by
/// cancel or quit: the completion task's catch-up checks those itself, so
/// a leftover list is harmless.
///
/// One exception: if the drop happens while the thread is panicking, the file
/// that was being worked on is NOT put back. A file that makes the lyrics
/// code panic would otherwise be handed to the completion task's catch-up,
/// which would panic on it too.
pub(crate) struct PendingRestore {
    pending: Option<Arc<StdMutex<Option<Vec<PathBuf>>>>>,
    remaining: Vec<PathBuf>,
    /// The file whose steps are running right now, if any.
    current: Option<PathBuf>,
}

impl PendingRestore {
    fn new(pending: Option<Arc<StdMutex<Option<Vec<PathBuf>>>>>, files: &[PathBuf]) -> Self {
        Self {
            pending,
            remaining: files.to_vec(),
            current: None,
        }
    }

    /// Notes that `file`'s steps are about to run.
    fn begin(&mut self, file: &Path) {
        self.current = Some(file.to_path_buf());
    }

    /// Notes that `file` is completely finished (or is not a file any more).
    fn done(&mut self, file: &Path) {
        self.remaining.retain(|p| p != file);
        if self.current.as_deref() == Some(file) {
            self.current = None;
        }
    }
}

impl Drop for PendingRestore {
    fn drop(&mut self) {
        // A panic while a file was being worked on: that file is given up on,
        // so it is not retried (and crashed on again) by the catch-up.
        if std::thread::panicking() {
            if let Some(file) = self.current.take() {
                self.remaining.retain(|p| p != &file);
            }
        }
        if self.remaining.is_empty() {
            return;
        }
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let rest = std::mem::take(&mut self.remaining);
        // A poisoned lock still holds a usable list.
        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_mut() {
            Some(existing) => {
                for p in rest {
                    if !existing.contains(&p) {
                        existing.push(p);
                    }
                }
            }
            None => *guard = Some(rest),
        }
    }
}

/// Same as the plain version, but takes the app-wide write lock for each
/// file (see `utils::file_locks`) before that file's steps, and holds it
/// only for that file. The lock is taken here, in the loop, never inside
/// the steps that write. Each file is marked finished in `restore` only
/// after its steps are done, so a dropped future leaves the rest recoverable.
pub(crate) async fn run_main_files_own_locked(
    locks: &Arc<FileWriteLocks>,
    settings: &AppSettings,
    files: &[PathBuf],
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &(dyn Fn() -> bool + Sync),
    restore: &mut PendingRestore,
) -> Summary {
    let settings = &settings_for_main_files(settings, lyrics_artist);
    if !any_conversion_switched_on(settings) {
        restore.remaining.clear();
        return Summary::default();
    }
    for p in files.iter().filter(|p| !p.is_file()) {
        restore.done(p);
    }
    let files: Vec<&PathBuf> = files.iter().filter(|p| p.is_file()).collect();
    let mut summary = Summary {
        total: files.len(),
        ..Summary::default()
    };
    for file in files {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let _guard = locks.lock(file).await;
        // From here until it is marked finished, a panic must not put this
        // file back on the list (see `PendingRestore`).
        restore.begin(file);
        let outcome = process_copy(
            file,
            None,
            settings,
            &|_| false,
            &|_| false,
            lyrics_artist,
            default_title,
            stop,
            false,
        );
        if !matches!(outcome, CopyOutcome::Stopped) {
            restore.done(file);
        }
        if tally_main(&mut summary, outcome) {
            return summary;
        }
    }
    summary
}

/// Takes the main download's unfinished-lyrics list (if enrichment left
/// one). Returns `None`, leaving the list in place, when `stop` already
/// says stop (abort, cancel, shutdown), or when there is no list.
fn take_pending(
    pending: Option<&Arc<StdMutex<Option<Vec<PathBuf>>>>>,
    stop: &dyn Fn() -> bool,
) -> Option<Vec<PathBuf>> {
    if stop() {
        return None;
    }
    pending?.lock().ok()?.take()
}

/// Takes the main download's unfinished-lyrics list (if enrichment left
/// one) and finishes those files, without write locks (tests). Needs no
/// extra copy and no recorded scope: the files are unfinished either way.
#[cfg(test)]
pub(crate) fn recover_pending(
    pending: Option<&Arc<StdMutex<Option<Vec<PathBuf>>>>>,
    settings: &AppSettings,
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> Option<Summary> {
    let files = take_pending(pending, stop)?;
    Some(run_main_files_own(
        settings,
        &files,
        lyrics_artist,
        default_title,
        stop,
    ))
}

/// [`recover_pending`] for the app: takes the app-wide write lock per file.
/// If this future is dropped while waiting for a file's lock, the files not
/// yet finished go back on `pending` (see [`PendingRestore`]).
pub(crate) async fn recover_pending_locked(
    locks: &Arc<FileWriteLocks>,
    pending: Option<&Arc<StdMutex<Option<Vec<PathBuf>>>>>,
    settings: &AppSettings,
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &(dyn Fn() -> bool + Sync),
) -> Option<Summary> {
    let files = take_pending(pending, stop)?;
    let mut restore = PendingRestore::new(pending.cloned(), &files);
    Some(
        run_main_files_own_locked(
            locks,
            settings,
            &files,
            lyrics_artist,
            default_title,
            stop,
            &mut restore,
        )
        .await,
    )
}

/// The activity-log line for [`run_main_files_own_locked`], or `None` when it
/// finished nothing.
pub(crate) fn recovery_line(r: &Summary) -> Option<String> {
    (r.own > 0).then(|| {
        let noun = if r.own == 1 { "song" } else { "songs" };
        format!(
            "Lyrics finished for {} {noun} of the main download that were left unfinished.",
            r.own
        )
    })
}

/// What [`plan_copies`] works out before any file is written: the new
/// copies, and what is needed to pair each with the main download's file.
pub(crate) struct CopyPlan {
    /// This download's new copies, in order.
    copies: Vec<PathBuf>,
    /// What is known about each of the main download's files.
    primaries: Vec<Evidence>,
    /// Whether the album is a single-disc one.
    single_disc: bool,
    /// Main-download files that an extra copy has better lyrics for.
    upgrades: Vec<PlannedUpgrade>,
}

/// One main-download file that should be given the lyrics of an extra copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedUpgrade {
    /// The main download's file, at its final name.
    main: PathBuf,
    /// The extra copy whose lyrics are better.
    from_copy: PathBuf,
    /// How good the copy's lyrics are.
    quality: LyricsQuality,
}

/// Picks, for each main-download file, the one extra copy to upgrade it from.
/// Each candidate is `(main file, copy, copy's lyrics quality, main file's
/// lyrics quality)`. A copy that is not strictly better than the main file is
/// no candidate. When several copies of one song qualify, the best wins and a
/// tie goes to the first copy by path, so the answer never changes between
/// runs. The result is sorted by main file.
pub(crate) fn pick_upgrades(
    candidates: Vec<(PathBuf, PathBuf, LyricsQuality, LyricsQuality)>,
) -> Vec<PlannedUpgrade> {
    let mut better: Vec<PlannedUpgrade> = candidates
        .into_iter()
        .filter(|(_, _, copy_q, main_q)| copy_q > main_q)
        .map(|(main, from_copy, quality, _)| PlannedUpgrade {
            main,
            from_copy,
            quality,
        })
        .collect();
    better.sort_by(|a, b| {
        a.main
            .cmp(&b.main)
            .then(b.quality.cmp(&a.quality))
            .then(a.from_copy.cmp(&b.from_copy))
    });
    better.dedup_by(|later, first| later.main == first.main);
    better
}

/// Finds the main download's file for one extra copy, by song ID, else by
/// folder plus disc and track (see [`match_primary`]).
fn match_copy(
    copy: &Path,
    primaries: &[Evidence],
    single_disc: bool,
    scope: &AdvisoryScope,
) -> Option<PathBuf> {
    let evidence = read_evidence(copy);
    let outside = !copy.starts_with(&scope.dir);
    match_primary(&evidence, primaries, single_disc, outside).map(|i| primaries[i].path.clone())
}

/// Works out which files are this download's new copies. Writes nothing.
/// Empty when no conversion is switched on and separate lyric files are not
/// kept (the folder is then not even looked at).
pub(crate) fn plan_copies(
    settings: &AppSettings,
    scope: &AdvisoryScope,
    extra_dir: Option<&Path>,
    since: SystemTime,
    album_tracks: &[(u32, u32, String)],
) -> CopyPlan {
    if !copy_work_switched_on(settings) {
        return CopyPlan {
            copies: Vec::new(),
            primaries: Vec::new(),
            single_disc: false,
            upgrades: Vec::new(),
        };
    }
    let copies = this_downloads_copies(scope, extra_dir, since);
    if copies.is_empty() {
        return CopyPlan {
            copies,
            primaries: Vec::new(),
            single_disc: false,
            upgrades: Vec::new(),
        };
    }
    let primaries: Vec<Evidence> = scope
        .this_download
        .iter()
        .filter(|p| p.is_file())
        .map(|p| read_evidence(p))
        .collect();
    let single_disc = single_disc_album(album_tracks, &primaries);
    // Which main files an extra copy has better lyrics for.
    let candidates: Vec<(PathBuf, PathBuf, LyricsQuality, LyricsQuality)> = copies
        .iter()
        .filter_map(|copy| {
            let main = match_copy(copy, &primaries, single_disc, scope)?;
            // Only a main file this run wrote may be upgraded; unknown means none.
            if !scope
                .written_this_run
                .as_ref()
                .is_some_and(|set| set.contains(&main))
            {
                return None;
            }
            // The upgrade source must be a `.ttml` this download wrote, never an
            // older leftover in the folder. A copy with no `.ttml` is never a
            // source: its `.lrc`/`.srt` could be leftovers nothing can check.
            if !copy_ttml_is_this_runs(scope, copy, since) {
                return None;
            }
            let copy_quality = quality_of_texts(&read_lyric_texts(copy));
            let main_quality = quality_of_texts(&read_lyric_texts(&main));
            Some((main, copy.clone(), copy_quality, main_quality))
        })
        .collect();
    CopyPlan {
        copies,
        primaries,
        single_disc,
        upgrades: pick_upgrades(candidates),
    }
}

/// Whether the extra copy has a `.ttml` and it was written by this run
/// (`false` when it has none):
/// inside the album folder, not in the snapshot taken before the copies
/// started; elsewhere, by file time.
fn copy_ttml_is_this_runs(scope: &AdvisoryScope, copy: &Path, since: SystemTime) -> bool {
    let ttml = copy.with_extension("ttml");
    if !ttml.exists() {
        return false;
    }
    if ttml.starts_with(&scope.dir) {
        !scope.ttml_before_companions.contains(&ttml)
    } else {
        written_since(&ttml, since)
    }
}

/// Whether `path` was made by this download (its file time is not older than
/// the download's start, with two seconds of slack). `false` when the start is
/// not known, so nothing is ever mistaken for this download's.
fn made_by_this_download(scope: &AdvisoryScope, path: &Path) -> bool {
    fresh_at_snapshot(&[path.to_path_buf()], scope.download_started_at).contains(path)
}

/// Gives ONE main-download file the lyrics of the extra copy that has better
/// ones. Takes no lock itself: the caller holds the app-wide write lock for
/// the main file (the steps below rewrite its tags), or is a test.
///
/// It is the same work as for a copy with the roles swapped (the copy is the
/// source, the main file is the target), under one extra safety rule: only
/// files THIS download made are ever changed.
///
/// - If the main file already has a `.ttml`, `.lrc` or `.srt` that this
///   download did not make (an older file with the same name), nothing at all
///   is done for it: its lyrics are not ours to replace.
/// - A `.vtt`, `.ass` or `.lyrics` file is rebuilt only when this download
///   made it. An older one stays exactly as it is.
/// - Nothing is replaced when the download's start time is unknown.
fn upgrade_main(
    up: &PlannedUpgrade,
    settings: &AppSettings,
    scope: &AdvisoryScope,
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> CopyOutcome {
    let made_here = |p: &Path| made_by_this_download(scope, p);
    let all_sources_ours = ["ttml", "lrc", "srt"]
        .iter()
        .map(|ext| up.main.with_extension(ext))
        .filter(|p| p.exists())
        .all(|p| made_here(&p));
    if !all_sources_ours {
        return CopyOutcome::Unchanged;
    }
    process_copy(
        &up.main,
        Some(&up.from_copy),
        settings,
        &made_here,
        &made_here,
        lyrics_artist,
        default_title,
        stop,
        true,
    )
}

/// Folds the result of one main-file upgrade into the summary. Returns `true`
/// when the run must end (a stop cut it short).
fn tally_upgrade(summary: &mut Summary, outcome: CopyOutcome) -> bool {
    match outcome {
        CopyOutcome::FromMain { .. } => summary.main_upgraded += 1,
        CopyOutcome::Stopped => {
            summary.stopped = true;
            return true;
        }
        CopyOutcome::Own | CopyOutcome::Unchanged | CopyOutcome::Nothing => {}
    }
    false
}

/// Gives ONE planned copy its lyrics. Takes no lock itself: the caller
/// holds the app-wide write lock for `copy` (the steps below rewrite its
/// tags), or is a test.
#[allow(clippy::too_many_arguments)]
fn process_planned(
    plan: &CopyPlan,
    copy: &Path,
    settings: &AppSettings,
    scope: &AdvisoryScope,
    since: SystemTime,
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> CopyOutcome {
    let primary = match_copy(copy, &plan.primaries, plan.single_disc, scope);
    // Inside the album folder a `.ttml` is this run's if it was not in
    // the snapshot taken before the extra copies started. Outside it
    // there is no snapshot, so file times decide.
    let written_by_this_run = |target: &Path| {
        if target.starts_with(&scope.dir) {
            !scope.ttml_before_companions.contains(target)
        } else {
            written_since(target, since)
        }
    };
    process_copy(
        copy,
        primary.as_deref(),
        settings,
        &written_by_this_run,
        &|_| false,
        lyrics_artist,
        default_title,
        stop,
        false,
    )
}

/// Folds one copy's result into the summary. Returns `true` when the run
/// must end (a stop cut this copy short; it is not counted as done).
fn tally_copy(summary: &mut Summary, outcome: CopyOutcome) -> bool {
    match outcome {
        CopyOutcome::FromMain { word_timed } => {
            summary.from_main += 1;
            if word_timed {
                summary.from_main_word_timed += 1;
            }
        }
        CopyOutcome::Own => summary.own += 1,
        CopyOutcome::Unchanged => summary.unchanged += 1,
        CopyOutcome::Nothing => summary.none += 1,
        CopyOutcome::Stopped => {
            summary.stopped = true;
            return true;
        }
    }
    false
}

/// Gives every new copy of this download its lyrics, as described at the
/// top of this file. `stop` is asked before each copy; when it says yes the
/// run ends and the summary says so (files already done stay done).
///
/// Settings are read by the caller when this starts, not when enrichment
/// ran, so a switch changed in between applies to the copies.
///
/// This version takes no write locks: it is for tests. The app uses
/// [`run_copies_locked`], which gives the same result.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_copies(
    settings: &AppSettings,
    scope: &AdvisoryScope,
    extra_dir: Option<&Path>,
    since: SystemTime,
    album_tracks: &[(u32, u32, String)],
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> Summary {
    let plan = plan_copies(settings, scope, extra_dir, since, album_tracks);
    let mut summary = Summary {
        total: plan.copies.len(),
        ..Summary::default()
    };
    // Main files first, so every copy of the same song then gets the better
    // lyrics from the main file.
    for up in &plan.upgrades {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let outcome = upgrade_main(up, settings, scope, lyrics_artist, default_title, stop);
        if tally_upgrade(&mut summary, outcome) {
            return summary;
        }
    }
    for copy in &plan.copies {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let outcome = process_planned(
            &plan,
            copy,
            settings,
            scope,
            since,
            lyrics_artist,
            default_title,
            stop,
        );
        if tally_copy(&mut summary, outcome) {
            return summary;
        }
    }
    summary
}

/// [`run_copies`] for the app: takes the app-wide write lock for each copy
/// (in the loop, not inside the steps) and holds it only while that copy's
/// steps run.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_copies_locked(
    locks: &Arc<FileWriteLocks>,
    settings: &AppSettings,
    scope: &AdvisoryScope,
    extra_dir: Option<&Path>,
    since: SystemTime,
    album_tracks: &[(u32, u32, String)],
    lyrics_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &(dyn Fn() -> bool + Sync),
) -> Summary {
    let plan = plan_copies(settings, scope, extra_dir, since, album_tracks);
    let mut summary = Summary {
        total: plan.copies.len(),
        ..Summary::default()
    };
    // Main files first (each under its own lock, released before the next),
    // so every copy of the same song then gets the better lyrics from the
    // main file.
    for up in &plan.upgrades {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let _guard = locks.lock(&up.main).await;
        let outcome = upgrade_main(up, settings, scope, lyrics_artist, default_title, stop);
        if tally_upgrade(&mut summary, outcome) {
            return summary;
        }
    }
    for copy in &plan.copies {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let _guard = locks.lock(copy).await;
        let outcome = process_planned(
            &plan,
            copy,
            settings,
            scope,
            since,
            lyrics_artist,
            default_title,
            stop,
        );
        if tally_copy(&mut summary, outcome) {
            return summary;
        }
    }
    summary
}

/// The whole step for the companion task: works out where the copies are
/// (including a folder outside the album folder) and runs
/// [`run_copies_locked`] under the app-wide write lock.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    settings: &AppSettings,
    scope: &AdvisoryScope,
    output_dir: &Path,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
    since: SystemTime,
    album_tracks: &[(u32, u32, String)],
    lyricsfile_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &(dyn Fn() -> bool + Sync),
) -> Summary {
    let extra_dir = resolve_extra_dir(output_dir, artist_hint, album_hint, &scope.dir);
    run_copies_locked(
        crate::utils::file_locks::global(),
        settings,
        scope,
        extra_dir.as_deref(),
        since,
        album_tracks,
        // Same rule as the main download's Step 2g: the album artist, or
        // "Unknown Artist" when the album details are missing.
        lyricsfile_artist,
        default_title,
        stop,
    )
    .await
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::download_queue::lyrics_pass::ttml_files_in;
    use crate::services::metadata_tag_service::test_support::minimal_m4a;
    use std::collections::BTreeSet;

    const WORD_TTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word" xml:lang="en-US">
          <head><metadata><ttm:title>Test Song</ttm:title></metadata></head>
          <body><div>
            <p begin="00:12.450" end="00:15.800">
              <span begin="00:12.450" end="00:13.200">Hello </span>
              <span begin="00:13.200" end="00:14.100">world </span>
              <span begin="00:14.100" end="00:15.800">today</span>
            </p>
          </div></body>
        </tt>"#;

    const LINE_TTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <body><div>
            <p begin="00:12.450" end="00:15.800">Hello world today</p>
          </div></body>
        </tt>"#;

    /// Everything switched on, separate files kept.
    fn all_on() -> AppSettings {
        AppSettings {
            enhanced_lrc: true,
            generate_webvtt: true,
            generate_rich_srt: true,
            generate_ass: true,
            generate_lyricsfile: true,
            embed_subtitles: true,
            keep_lyrics_sidecar: true,
            ..AppSettings::default()
        }
    }

    /// A temp album folder with the main download's file (disc 1 track 1,
    /// "01 T [Dolby Atmos].m4a") and a scope that treats it as the main
    /// download. Returns the folder, the scope and the main file.
    fn album() -> (tempfile::TempDir, AdvisoryScope, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("01 T [Dolby Atmos].m4a");
        minimal_m4a(&main, 1, 1, false);
        let scope = AdvisoryScope {
            dir: dir.path().to_path_buf(),
            this_download: vec![main.clone()],
            before_companions: audio_files_in(dir.path()),
            ttml_before_companions: ttml_files_in(dir.path()),
            download_started_at: None,
            written_this_run: Some(std::iter::once(main.clone()).collect()),
        };
        (dir, scope, main)
    }

    /// Adds a new extra copy (disc 1 track 1) after the scope was taken.
    fn add_copy(dir: &Path) -> PathBuf {
        let copy = dir.join("01 T [Lossless].m4a");
        minimal_m4a(&copy, 1, 1, false);
        copy
    }

    fn ev(
        name: &str,
        dir: &str,
        id: Option<&str>,
        disc: Option<u32>,
        track: Option<u32>,
    ) -> Evidence {
        Evidence {
            path: PathBuf::from(dir).join(name),
            dir: PathBuf::from(dir),
            song_id: id.map(str::to_string),
            disc,
            track,
        }
    }

    fn past() -> SystemTime {
        SystemTime::now() - Duration::from_secs(3600)
    }

    // ---- 1. choose_source ----

    #[test]
    fn choose_source_follows_the_table() {
        use LyricsQuality::{LineTtml, None as Nil, OtherOnly, WordTimedTtml};
        // No match or unclear.
        assert_eq!(choose_source(None, WordTimedTtml), Source::Own);
        assert_eq!(choose_source(None, OtherOnly), Source::Own);
        assert_eq!(choose_source(None, Nil), Source::Nothing);
        // Main has none.
        assert_eq!(choose_source(Some(Nil), LineTtml), Source::Own);
        assert_eq!(choose_source(Some(Nil), Nil), Source::Nothing);
        // Main strictly better, including when the copy has nothing.
        assert_eq!(
            choose_source(Some(WordTimedTtml), LineTtml),
            Source::Primary
        );
        assert_eq!(choose_source(Some(WordTimedTtml), Nil), Source::Primary);
        assert_eq!(choose_source(Some(LineTtml), OtherOnly), Source::Primary);
        assert_eq!(choose_source(Some(OtherOnly), Nil), Source::Primary);
        // Ties use the copy's own.
        assert_eq!(
            choose_source(Some(WordTimedTtml), WordTimedTtml),
            Source::Own
        );
        assert_eq!(choose_source(Some(LineTtml), LineTtml), Source::Own);
        // The copy's own is better.
        assert_eq!(choose_source(Some(LineTtml), WordTimedTtml), Source::Own);
    }

    // ---- 2. match_primary ----

    #[test]
    fn match_primary_prefers_the_song_id() {
        let primaries = vec![
            ev("a.m4a", "/m", Some("11"), Some(1), Some(1)),
            ev("b.m4a", "/m", Some("12"), Some(1), Some(2)),
        ];
        // Disc and track point at the first; the song ID says the second.
        let copy = ev("c.m4a", "/m", Some("12"), Some(1), Some(1));
        assert_eq!(match_primary(&copy, &primaries, true, false), Some(1));
    }

    #[test]
    fn match_primary_refuses_a_song_id_shared_by_two_files() {
        let primaries = vec![
            ev("a.m4a", "/m", Some("11"), Some(1), Some(1)),
            ev("b.m4a", "/m", Some("11"), Some(1), Some(2)),
        ];
        let copy = ev("c.m4a", "/m", Some("11"), Some(1), Some(1));
        assert_eq!(match_primary(&copy, &primaries, true, false), None);
    }

    #[test]
    fn match_primary_uses_folder_disc_and_track() {
        let primaries = vec![
            ev("a.m4a", "/m", None, Some(1), Some(1)),
            ev("b.m4a", "/m", None, Some(1), Some(2)),
        ];
        let copy = ev("c.m4a", "/m", None, Some(1), Some(2));
        assert_eq!(match_primary(&copy, &primaries, true, false), Some(1));
        // A copy in a different folder is never matched this way.
        let outside = ev("c.m4a", "/other", None, Some(1), Some(2));
        assert_eq!(match_primary(&outside, &primaries, true, false), None);
        assert_eq!(match_primary(&copy, &primaries, true, true), None);
    }

    #[test]
    fn match_primary_refuses_two_files_with_the_same_numbers() {
        // An older copy of the track sits in the same folder.
        let primaries = vec![
            ev("a.m4a", "/m", None, Some(1), Some(1)),
            ev("old.m4a", "/m", None, Some(1), Some(1)),
        ];
        let copy = ev("c.m4a", "/m", None, Some(1), Some(1));
        assert_eq!(match_primary(&copy, &primaries, true, false), None);
    }

    #[test]
    fn match_primary_never_crosses_albums_in_a_playlist() {
        // Two albums, both with disc 1 track 1, in different folders.
        let primaries = vec![
            ev("a.m4a", "/m/one", None, Some(1), Some(1)),
            ev("b.m4a", "/m/two", None, Some(1), Some(1)),
        ];
        let copy = ev("c.m4a", "/m/two", None, Some(1), Some(1));
        assert_eq!(match_primary(&copy, &primaries, true, false), Some(1));
        let nowhere = ev("c.m4a", "/m/three", None, Some(1), Some(1));
        assert_eq!(match_primary(&nowhere, &primaries, true, false), None);
    }

    #[test]
    fn match_primary_will_not_guess_an_unknown_disc_on_a_multi_disc_album() {
        let primaries = vec![
            ev("a.m4a", "/m", None, Some(1), Some(1)),
            ev("b.m4a", "/m", None, Some(2), Some(1)),
        ];
        let copy = ev("c.m4a", "/m", None, None, Some(1));
        assert_eq!(match_primary(&copy, &primaries, false, false), None);
        // On a single-disc album the same copy counts as disc 1.
        let one = vec![ev("a.m4a", "/m", None, Some(1), Some(1))];
        assert_eq!(match_primary(&copy, &one, true, false), Some(0));
    }

    #[test]
    fn single_disc_follows_the_track_list_then_the_files() {
        let two_discs = vec![(1, 1, "1".to_string()), (2, 1, "2".to_string())];
        assert!(!single_disc_album(&two_discs, &[]));
        assert!(single_disc_album(&[(1, 1, "1".to_string())], &[]));
        let files = vec![ev("a.m4a", "/m", None, Some(2), Some(1))];
        assert!(!single_disc_album(&[], &files));
        assert!(single_disc_album(
            &[],
            &[ev("a.m4a", "/m", None, None, Some(1))]
        ));
    }

    // ---- 3. may_write_ttml ----

    #[test]
    fn may_write_ttml_covers_every_combination() {
        // keep, own_exists, written_this_run
        assert!(may_write_ttml(true, false, false));
        assert!(!may_write_ttml(false, false, false));
        // An existing file is replaced only when this run wrote it.
        assert!(may_write_ttml(true, true, true));
        assert!(may_write_ttml(false, true, true));
        assert!(!may_write_ttml(true, true, false));
        assert!(!may_write_ttml(false, true, false));
    }

    // ---- 4. summary_line ----

    #[test]
    fn summary_line_says_nothing_when_nothing_was_done() {
        assert_eq!(summary_line(&Summary::default()), None);
        let only_none = Summary {
            total: 2,
            none: 2,
            ..Summary::default()
        };
        assert_eq!(summary_line(&only_none), None);
    }

    #[test]
    fn summary_line_uses_plain_wording() {
        let word = Summary {
            total: 12,
            from_main: 10,
            from_main_word_timed: 10,
            own: 2,
            none: 0,
            unchanged: 0,
            stopped: false,
            main_upgraded: 0,
        };
        assert_eq!(
            summary_line(&word).unwrap(),
            "Lyrics added to 12 extra copies: 10 used the main download's word-by-word lyrics, 2 used their own."
        );
        // Not all word-timed: no "word-by-word".
        let mixed = Summary {
            from_main_word_timed: 4,
            ..word
        };
        assert!(!summary_line(&mixed).unwrap().contains("word-by-word"));
        // The "had no lyrics" clause, and the singular.
        let none = Summary {
            total: 3,
            own: 1,
            none: 2,
            ..Summary::default()
        };
        assert_eq!(
            summary_line(&none).unwrap(),
            "Lyrics added to 1 extra copy: 1 used their own. 2 had no lyrics to use."
        );
    }

    #[test]
    fn summary_line_reports_a_stop() {
        let stopped = Summary {
            total: 12,
            from_main: 4,
            own: 2,
            stopped: true,
            ..Summary::default()
        };
        assert_eq!(
            summary_line(&stopped).unwrap(),
            "Adding lyrics to the extra copies stopped early (6 of 12 done)."
        );
    }

    // ---- 6. main download's word-by-word lyrics reach the copy ----

    #[test]
    fn the_main_downloads_better_lyrics_replace_the_copys_own() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        // The copy's own line-level file, written just now by this run.
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.from_main, 1);
        assert_eq!(s.from_main_word_timed, 1);
        let ttml = std::fs::read_to_string(copy.with_extension("ttml")).unwrap();
        assert!(ttml.contains("Hello"), "the copy's ttml was replaced");
        assert!(ttml.contains("<span"), "with the word-timed one");
        let lrc = std::fs::read_to_string(copy.with_extension("lrc")).unwrap();
        assert!(lrc.contains("<00:"), "word timestamps in the copy's lrc");
        for ext in ["vtt", "srt", "ass", "lyrics"] {
            assert!(copy.with_extension(ext).is_file(), ".{ext} should exist");
        }
        // The lyrics were also put inside the music file.
        let tag = mp4ameta::Tag::read_from_path(&copy).unwrap();
        assert!(tag.lyrics().is_some_and(|l| l.contains("<00:")));
    }

    // ---- 7. keep off creates no files ----

    #[test]
    fn with_separate_files_off_no_new_file_is_created() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        let settings = AppSettings {
            keep_lyrics_sidecar: false,
            ..all_on()
        };

        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.from_main, 1);
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                !copy.with_extension(ext).exists(),
                ".{ext} must not be created"
            );
        }
        // The lyrics still went inside the music file.
        let tag = mp4ameta::Tag::read_from_path(&copy).unwrap();
        assert!(tag.lyrics().is_some_and(|l| l.contains("<00:")));
    }

    // ---- 8. files already there are left alone ----

    #[test]
    fn files_already_on_disk_are_not_replaced() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        for ext in ["vtt", "ass", "lyrics"] {
            std::fs::write(copy.with_extension(ext), "mine").unwrap();
        }
        // An older .ttml: it was in the folder before the extra copies
        // started, so it is not "written by this run" and must not be
        // replaced.
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };

        run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        for ext in ["vtt", "ass", "lyrics"] {
            assert_eq!(
                std::fs::read_to_string(copy.with_extension(ext)).unwrap(),
                "mine",
                ".{ext} was replaced"
            );
        }
        assert_eq!(
            std::fs::read_to_string(copy.with_extension("ttml")).unwrap(),
            LINE_TTML
        );
    }

    // ---- 9. the .ass file carries the copy's own name ----

    #[test]
    fn the_ass_title_is_the_copys_own_name() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        let ass = std::fs::read_to_string(copy.with_extension("ass")).unwrap();
        assert!(ass.contains("01 T [Lossless]"), "{ass}");
        assert!(!ass.contains("Dolby Atmos"), "{ass}");
    }

    // ---- 10. only new copies are touched ----

    #[test]
    fn only_the_new_copies_are_touched() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        // An older track already in the folder before the extra copies.
        let older = dir.path().join("05 Old.m4a");
        minimal_m4a(&older, 5, 1, false);
        std::fs::write(older.with_extension("ttml"), LINE_TTML).unwrap();
        let before: BTreeSet<PathBuf> = audio_files_in(dir.path());
        let scope = AdvisoryScope {
            before_companions: before,
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let main_bytes = std::fs::read(&main).unwrap();
        let older_bytes = std::fs::read(&older).unwrap();
        let copy = add_copy(dir.path());

        run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert!(copy.with_extension("lrc").is_file());
        assert_eq!(std::fs::read(&main).unwrap(), main_bytes);
        assert_eq!(std::fs::read(&older).unwrap(), older_bytes);
        assert_eq!(
            std::fs::read_to_string(main.with_extension("ttml")).unwrap(),
            WORD_TTML
        );
        for ext in ["lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(!main.with_extension(ext).exists(), "main got .{ext}");
            assert!(!older.with_extension(ext).exists(), "older got .{ext}");
        }
    }

    // ---- 11. a main download with only .lrc ----

    #[test]
    fn a_main_download_with_only_an_lrc_file_gives_the_copy_lrc_and_vtt() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("lrc"), "[00:12.45]Hello world today\n").unwrap();
        let copy = add_copy(dir.path());
        let settings = AppSettings {
            enhanced_lrc: false,
            generate_webvtt: true,
            generate_rich_srt: false,
            generate_ass: false,
            generate_lyricsfile: false,
            embed_subtitles: false,
            keep_lyrics_sidecar: true,
            ..AppSettings::default()
        };

        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.from_main, 1);
        assert_eq!(s.from_main_word_timed, 0);
        assert!(copy.with_extension("lrc").is_file());
        let vtt = std::fs::read_to_string(copy.with_extension("vtt")).unwrap();
        assert!(vtt.starts_with("WEBVTT"));
    }

    // ---- 12. an abort before the call writes nothing ----

    #[test]
    fn a_stop_before_the_run_writes_nothing() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| true);
        assert!(s.stopped);
        assert_eq!(s.total, 1);
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(!copy.with_extension(ext).exists(), ".{ext} was written");
        }
    }

    // ---- review fixes ----

    #[test]
    fn two_files_with_different_known_song_ids_are_never_paired() {
        let primaries = vec![ev("a.m4a", "/a", Some("1"), Some(1), Some(1))];
        let copy = ev("b.m4a", "/a", Some("2"), Some(1), Some(1));
        assert_eq!(match_primary(&copy, &primaries, true, false), None);
        // An unknown song ID on either side still pairs by numbers.
        let no_id = ev("b.m4a", "/a", None, Some(1), Some(1));
        assert_eq!(match_primary(&no_id, &primaries, true, false), Some(0));
        let primaries_no_id = vec![ev("a.m4a", "/a", None, Some(1), Some(1))];
        assert_eq!(match_primary(&copy, &primaries_no_id, true, false), Some(0));
    }

    #[test]
    fn the_song_id_is_read_even_when_disc_and_track_cannot_be() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.m4a");
        minimal_m4a(&path, 1, 1, false);
        let mut tag = mp4ameta::Tag::read_from_path(&path).unwrap();
        tag.set_data(
            mp4ameta::Fourcc(*b"cnID"),
            mp4ameta::Data::BeSigned(1_440_857_795_u32.to_be_bytes().to_vec()),
        );
        tag.remove_track();
        tag.remove_disc();
        tag.write_to_path(&path).unwrap();
        let e = read_evidence(&path);
        assert_eq!(e.song_id.as_deref(), Some("1440857795"));
        assert_eq!(e.track, None);
    }

    #[test]
    fn a_copy_with_nothing_to_add_is_not_counted_as_given_lyrics() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        // Only the Lyricsfile is switched on, and it is already there.
        let settings = AppSettings {
            enhanced_lrc: false,
            generate_webvtt: false,
            generate_rich_srt: false,
            generate_ass: false,
            embed_subtitles: false,
            generate_lyricsfile: true,
            keep_lyrics_sidecar: true,
            ..AppSettings::default()
        };
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();
        std::fs::write(copy.with_extension("lyrics"), "mine").unwrap();
        // The main download's ttml is better but an older ttml of the copy
        // (already there before the extra copies started) is left alone, so
        // nothing is written.
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!((s.from_main, s.own, s.unchanged), (0, 0, 1));
        assert_eq!(summary_line(&s), None);
        let some = Summary {
            total: 2,
            own: 1,
            unchanged: 1,
            ..Summary::default()
        };
        assert_eq!(
            summary_line(&some).unwrap(),
            "Lyrics added to 1 extra copy: 1 used their own. 1 needed nothing added."
        );
    }

    #[test]
    fn a_stop_between_steps_ends_the_work_inside_one_copy() {
        use std::cell::Cell;
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        // Allow the first question (before the Enhanced LRC step), then stop.
        let asked = Cell::new(0);
        let stop = || {
            asked.set(asked.get() + 1);
            asked.get() > 2
        };
        run_copies(&all_on(), &scope, None, past(), &[], None, None, &stop);
        assert!(copy.with_extension("lrc").is_file());
        for ext in ["vtt", "srt", "ass", "lyrics"] {
            assert!(!copy.with_extension(ext).exists(), ".{ext} was written");
        }
    }

    #[test]
    fn a_stop_during_the_last_copy_is_reported_as_stopped() {
        use std::cell::Cell;
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        add_copy(dir.path());
        // The loop's own check passes; a step inside the copy stops it.
        let asked = Cell::new(0);
        let stop = || {
            asked.set(asked.get() + 1);
            asked.get() > 2
        };
        let summary = run_copies(&all_on(), &scope, None, past(), &[], None, None, &stop);
        assert!(
            summary.stopped,
            "a stop inside the last copy must be reported"
        );
        // The interrupted copy is not counted as done, so the line cannot
        // say "1 of 1 done" about work that was cut short.
        assert_eq!(summary.from_main + summary.own + summary.unchanged, 0);
        assert_eq!(
            summary_line(&summary).unwrap(),
            "Adding lyrics to the extra copies stopped early (0 of 1 done)."
        );
    }

    #[test]
    fn a_stop_after_the_last_copy_finished_is_not_reported_as_stopped() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        // The Lyricsfile is the last thing written. The stop only turns
        // "yes" once it exists, so it arrives when nothing is left to cut.
        let last = copy.with_extension("lyrics");
        let summary = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| {
            last.exists()
        });
        assert!(last.is_file(), "every step ran");
        assert!(!summary.stopped, "nothing was interrupted");
        assert_eq!(summary.from_main, 1);
        assert!(!summary_line(&summary).unwrap().contains("stopped"));
    }

    #[test]
    fn a_ttml_is_this_runs_by_the_snapshot_not_by_file_time() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = add_copy(dir.path());
        // The copy's `.ttml` was already there before the extra copies
        // started, and the run "started" an hour in the past, so by file
        // time alone it would look new and be replaced. The snapshot says
        // it is older, so it must stay.
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(
            std::fs::read_to_string(copy.with_extension("ttml")).unwrap(),
            LINE_TTML
        );
    }

    #[test]
    fn subtitles_are_embedded_from_the_main_downloads_srt_with_files_off() {
        let (dir, scope, main) = album();
        // The main download has only a fallback `.srt`, no `.ttml`.
        std::fs::write(
            main.with_extension("srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nHello\n",
        )
        .unwrap();
        let copy = add_copy(dir.path());
        let settings = AppSettings {
            enhanced_lrc: false,
            generate_webvtt: false,
            generate_rich_srt: false,
            generate_ass: false,
            generate_lyricsfile: false,
            embed_subtitles: true,
            keep_lyrics_sidecar: false,
            ..AppSettings::default()
        };
        run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert!(
            !copy.with_extension("srt").exists(),
            "no separate file with files off"
        );
        let tag = mp4ameta::Tag::read_from_path(&copy).unwrap();
        let embedded = tag
            .data_of(&mp4ameta::FreeformIdent::new_static(
                "com.apple.iTunes",
                "subtitles-srt",
            ))
            .next()
            .and_then(|d| d.string().map(str::to_owned));
        assert_eq!(embedded.as_deref().map(|t| t.contains("Hello")), Some(true));
    }

    #[test]
    fn the_lyricsfile_uses_the_main_downloads_artist_and_title() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        // A name with no title part, so the default title is what shows.
        let copy = dir.path().join("x.m4a");
        minimal_m4a(&copy, 1, 1, false);
        let settings = AppSettings {
            enhanced_lrc: false,
            generate_webvtt: false,
            generate_rich_srt: false,
            generate_ass: false,
            embed_subtitles: false,
            generate_lyricsfile: true,
            keep_lyrics_sidecar: true,
            ..AppSettings::default()
        };
        run_copies(
            &settings,
            &scope,
            None,
            past(),
            &[],
            Some("Album Artist"),
            Some("Default Title"),
            &|| false,
        );
        let yaml = std::fs::read_to_string(copy.with_extension("lyrics")).unwrap();
        assert!(yaml.contains("Album Artist"), "{yaml}");
    }

    // ---- 13. nothing here deletes ----

    #[test]
    fn this_module_never_deletes_files() {
        let src = include_str!("companion_lyrics.rs");
        // Assembled so this test's own text does not trip the check.
        for call in [["remove", "_file"].concat(), ["remove", "_dir"].concat()] {
            let outside_tests = src.split("#[cfg(test)]").next().unwrap();
            assert!(
                !outside_tests.contains(&call),
                "companion_lyrics.rs calls {call}"
            );
        }
    }

    // ---- 14. the final pass moves every kind of file with the copy ----

    #[test]
    fn the_final_advisory_pass_moves_all_six_kinds_with_a_copy() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let copy = dir.path().join("01 T [Lossless].m4a");
        minimal_m4a(&copy, 1, 1, true);
        run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(copy.with_extension(ext).is_file(), "no .{ext} before");
        }
        crate::services::metadata_tag_service::apply_advisory_suffixes_to_files(
            std::slice::from_ref(&copy),
        );
        for ext in ["m4a", "ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                dir.path()
                    .join(format!("01 T [Explicit] [Lossless].{ext}"))
                    .is_file(),
                ".{ext} should have moved"
            );
        }
    }

    // ---- extra checks on the pure file helpers ----

    #[test]
    fn nothing_switched_on_does_not_look_at_the_folder() {
        let (dir, scope, _main) = album();
        add_copy(dir.path());
        let s = AppSettings {
            enhanced_lrc: false,
            generate_webvtt: false,
            generate_rich_srt: false,
            generate_ass: false,
            generate_lyricsfile: false,
            embed_subtitles: false,
            keep_lyrics_sidecar: false,
            ..AppSettings::default()
        };
        assert_eq!(
            run_copies(&s, &scope, None, past(), &[], None, None, &|| false),
            Summary::default()
        );
    }

    // ---- 15. no recorded scope (#1441) ----

    /// Sets a file's modified time to an hour ago.
    fn make_old(path: &Path) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(past()).unwrap();
    }

    #[test]
    fn the_fallback_scope_trusts_the_snapshot_over_file_times() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("01 Older.m4a");
        minimal_m4a(&older, 1, 1, false);
        let snapshot = FolderSnapshot::take(dir.path());
        let copy = add_copy(dir.path());
        let since = SystemTime::now() - Duration::from_secs(3600);
        // The older file has a brand-new file time (a tag rewrite) yet is in
        // the snapshot, so it is not a new copy.
        let scope = fallback_scope(dir.path(), Some(&snapshot), since);
        assert!(scope.this_download.is_empty());
        assert_eq!(this_downloads_copies(&scope, None, since), vec![copy]);
    }

    #[test]
    fn the_fallback_scope_uses_file_times_without_a_matching_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("01 Older.m4a");
        minimal_m4a(&older, 1, 1, false);
        make_old(&older);
        std::fs::write(dir.path().join("01 Older.ttml"), LINE_TTML).unwrap();
        make_old(&dir.path().join("01 Older.ttml"));
        let since = SystemTime::now() - Duration::from_secs(60);
        let copy = add_copy(dir.path());
        // No snapshot, then a snapshot of some other folder: same answer.
        let elsewhere = tempfile::tempdir().unwrap();
        let other = FolderSnapshot::take(elsewhere.path());
        for before in [None, Some(&other)] {
            let scope = fallback_scope(dir.path(), before, since);
            assert_eq!(
                this_downloads_copies(&scope, None, since),
                vec![copy.clone()],
                "only the file written since the start is new"
            );
            assert!(scope
                .ttml_before_companions
                .contains(&dir.path().join("01 Older.ttml")));
        }
    }

    #[test]
    fn without_a_scope_each_new_copy_gets_its_own_lyrics_and_older_files_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("01 Older.m4a");
        minimal_m4a(&older, 1, 1, false);
        std::fs::write(dir.path().join("01 Older.ttml"), WORD_TTML).unwrap();
        make_old(&older);
        make_old(&dir.path().join("01 Older.ttml"));
        let since = SystemTime::now() - Duration::from_secs(60);
        let copy = dir.path().join("02 New.m4a");
        minimal_m4a(&copy, 2, 1, false);
        std::fs::write(dir.path().join("02 New.ttml"), WORD_TTML).unwrap();
        let scope = fallback_scope(dir.path(), None, since);
        let older_before = (
            std::fs::read(&older).unwrap(),
            std::fs::read(dir.path().join("01 Older.ttml")).unwrap(),
        );
        let summary = run_copies(&all_on(), &scope, None, since, &[], None, None, &|| false);
        assert_eq!((summary.total, summary.own), (1, 1));
        assert!(dir.path().join("02 New.lrc").exists());
        assert!(dir.path().join("02 New.vtt").exists());
        // The older song: nothing added, nothing changed.
        for ext in ["lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                !dir.path().join(format!("01 Older.{ext}")).exists(),
                "{ext}"
            );
        }
        assert_eq!(std::fs::read(&older).unwrap(), older_before.0);
        assert_eq!(
            std::fs::read(dir.path().join("01 Older.ttml")).unwrap(),
            older_before.1
        );
        let _ = copy;
    }

    #[test]
    fn unfinished_main_files_are_finished_and_older_files_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("01 Main.m4a");
        minimal_m4a(&main, 1, 1, false);
        std::fs::write(dir.path().join("01 Main.ttml"), WORD_TTML).unwrap();
        let older = dir.path().join("02 Older.m4a");
        minimal_m4a(&older, 2, 1, false);
        std::fs::write(dir.path().join("02 Older.ttml"), WORD_TTML).unwrap();
        let older_bytes = std::fs::read(&older).unwrap();
        let files = vec![main.clone()];
        let first = run_main_files_own(&all_on(), &files, Some("A"), Some("T"), &|| false);
        assert_eq!(first.own, 1);
        assert!(dir.path().join("01 Main.lrc").exists());
        assert!(dir.path().join("01 Main.ass").exists());
        assert!(!dir.path().join("02 Older.lrc").exists());
        assert_eq!(std::fs::read(&older).unwrap(), older_bytes);
        // Running it again gives the same files, byte for byte.
        let names = |d: &Path| {
            let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(d)
                .unwrap()
                .flatten()
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().to_string(),
                        std::fs::read(e.path()).unwrap(),
                    )
                })
                .collect();
            v.sort();
            v
        };
        let after_first = names(dir.path());
        run_main_files_own(&all_on(), &files, Some("A"), Some("T"), &|| false);
        assert_eq!(names(dir.path()), after_first);
        assert_eq!(
            recovery_line(&first).as_deref(),
            Some("Lyrics finished for 1 song of the main download that were left unfinished.")
        );
        assert_eq!(recovery_line(&Summary::default()), None);
    }

    #[test]
    fn pending_main_lyrics_are_recovered_without_any_extra_copy_or_scope() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("01 Main.m4a");
        minimal_m4a(&main, 1, 1, false);
        std::fs::write(dir.path().join("01 Main.ttml"), WORD_TTML).unwrap();
        let pending = Arc::new(StdMutex::new(Some(vec![main])));
        // A stop leaves the list in place and writes nothing.
        assert!(recover_pending(Some(&pending), &all_on(), None, None, &|| true).is_none());
        assert!(pending.lock().unwrap().is_some());
        assert!(!dir.path().join("01 Main.lrc").exists());
        // Without a stop the files are finished and the list is taken.
        let r = recover_pending(Some(&pending), &all_on(), None, None, &|| false).unwrap();
        assert_eq!(r.own, 1);
        assert!(dir.path().join("01 Main.lrc").exists());
        assert!(pending.lock().unwrap().is_none());
        // Nothing pending (or no handle at all): nothing to do.
        assert!(recover_pending(Some(&pending), &all_on(), None, None, &|| false).is_none());
        assert!(recover_pending(None, &all_on(), None, None, &|| false).is_none());
    }

    /// If the locked catch-up is dropped while waiting for a file's lock (the
    /// extra-copy task aborted at its deadline), the files not yet finished
    /// go back on the pending list; the finished one does not.
    #[tokio::test]
    async fn dropped_locked_catch_up_returns_unfinished_files_to_pending() {
        let dir = tempfile::tempdir().unwrap();
        let mk = |name: &str| {
            let f = dir.path().join(format!("{name}.m4a"));
            minimal_m4a(&f, 1, 1, false);
            std::fs::write(dir.path().join(format!("{name}.ttml")), WORD_TTML).unwrap();
            f
        };
        let (a, b, c) = (mk("01 A"), mk("02 B"), mk("03 C"));
        let pending = Arc::new(StdMutex::new(Some(vec![a.clone(), b.clone(), c.clone()])));
        let locks = Arc::new(FileWriteLocks::new());
        // Another task holds b's lock for the whole test.
        let held = locks.lock(&b).await;
        let settings = all_on();
        let fut = recover_pending_locked(&locks, Some(&pending), &settings, None, None, &|| false);
        let waited = tokio::time::timeout(Duration::from_millis(500), fut).await;
        assert!(waited.is_err(), "the catch-up should still be waiting on b");
        // a was finished before the wait; the future is now dropped.
        assert!(dir.path().join("01 A.lrc").exists());
        assert!(!dir.path().join("02 B.lrc").exists());
        let left = pending.lock().unwrap().clone().expect("remainder put back");
        assert_eq!(left, vec![b.clone(), c.clone()]);
        // A later catch-up (the completion task's) finishes them.
        drop(held);
        let r = tokio::time::timeout(
            Duration::from_secs(10),
            recover_pending_locked(&locks, Some(&pending), &settings, None, None, &|| false),
        )
        .await
        .expect("no hang")
        .expect("a list was there");
        assert_eq!(r.own, 2);
        assert!(dir.path().join("03 C.lrc").exists());
        assert!(pending.lock().unwrap().is_none());
    }

    #[test]
    fn catch_up_writes_no_lyricsfile_when_no_artist_is_known() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("01 Main.m4a");
        minimal_m4a(&main, 1, 1, false);
        std::fs::write(dir.path().join("01 Main.ttml"), WORD_TTML).unwrap();
        // No artist: the other steps run, the Lyricsfile step does not.
        let r = run_main_files_own(&all_on(), std::slice::from_ref(&main), None, None, &|| {
            false
        });
        assert_eq!(r.own, 1);
        assert!(dir.path().join("01 Main.lrc").exists());
        assert!(!dir.path().join("01 Main.lyrics").exists());
        // With an artist it is written, under that artist's name.
        let r = run_main_files_own(&all_on(), &[main], Some("Real Artist"), Some("T"), &|| {
            false
        });
        assert_eq!(r.total, 1);
        let text = std::fs::read_to_string(dir.path().join("01 Main.lyrics")).unwrap();
        assert!(text.contains("Real Artist"));
        assert!(!text.contains("Unknown Artist"));
    }

    #[test]
    fn a_stop_ends_the_recovery_of_unfinished_main_files() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("01 Main.m4a");
        minimal_m4a(&main, 1, 1, false);
        std::fs::write(dir.path().join("01 Main.ttml"), WORD_TTML).unwrap();
        let r = run_main_files_own(&all_on(), &[main], None, None, &|| true);
        assert!(r.stopped);
        assert!(!dir.path().join("01 Main.lrc").exists());
    }

    #[test]
    fn the_album_folder_is_the_recorded_one_when_it_is_still_there() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone");
        assert_eq!(
            resolve_album_folder(dir.path(), Some(dir.path()), None, None),
            Some(dir.path().to_path_buf())
        );
        // A recorded folder that is gone, an output folder with nothing in
        // it, and no names: nothing is found.
        assert_eq!(
            resolve_album_folder(dir.path(), Some(&missing), Some(""), Some("")),
            None
        );
        assert_eq!(
            resolve_album_folder(&missing, None, Some("A"), Some("B")),
            None
        );
    }

    #[test]
    fn copies_are_only_new_m4a_files_that_are_not_the_main_downloads() {
        let (dir, scope, main) = album();
        let copy = add_copy(dir.path());
        std::fs::write(dir.path().join("02 Video.m4v"), b"v").unwrap();
        let found = this_downloads_copies(&scope, None, past());
        assert_eq!(found, vec![copy]);
        assert!(!found.contains(&main));
    }

    // ---- 16. the app-wide write lock (#1442) ----

    /// Every file in a folder, name to bytes, sorted.
    fn folder_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().to_string(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    }

    /// Builds one album with a main file, word-timed lyrics and one new copy.
    fn album_with_copy() -> (tempfile::TempDir, AdvisoryScope, PathBuf) {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let copy = add_copy(dir.path());
        (dir, scope, copy)
    }

    #[tokio::test]
    async fn the_locked_driver_gives_the_same_summary_and_files() {
        let (dir_a, scope_a, _) = album_with_copy();
        let (dir_b, scope_b, _) = album_with_copy();
        let plain = run_copies(&all_on(), &scope_a, None, past(), &[], None, None, &|| {
            false
        });
        let locked = run_copies_locked(
            crate::utils::file_locks::global(),
            &all_on(),
            &scope_b,
            None,
            past(),
            &[],
            None,
            None,
            &|| false,
        )
        .await;
        assert_eq!(plain, locked);
        assert_eq!(plain.from_main, 1);
        assert_eq!(folder_bytes(dir_a.path()), folder_bytes(dir_b.path()));
    }

    #[tokio::test]
    async fn a_copy_held_by_another_task_is_written_only_after_release() {
        let (dir, scope, copy) = album_with_copy();
        let held = crate::utils::file_locks::global().lock(&copy).await;
        let settings = all_on();
        let fut = run_copies_locked(
            crate::utils::file_locks::global(),
            &settings,
            &scope,
            None,
            past(),
            &[],
            None,
            None,
            &|| false,
        );
        tokio::pin!(fut);
        let waited = tokio::time::timeout(Duration::from_millis(100), &mut fut).await;
        assert!(waited.is_err(), "must wait while the copy is locked");
        assert!(
            !copy.with_extension("lrc").exists(),
            "nothing may be written while the lock is held"
        );
        drop(held);
        let summary = fut.await;
        assert_eq!(summary.from_main, 1);
        assert!(copy.with_extension("lrc").is_file());
        drop(dir);
    }

    // ---- 17. every conversion off, separate files kept (#1442 b) ----

    /// Every conversion off; only "keep separate lyrics files" is on.
    fn only_keep_on() -> AppSettings {
        AppSettings {
            enhanced_lrc: false,
            generate_webvtt: false,
            generate_rich_srt: false,
            generate_ass: false,
            generate_lyricsfile: false,
            embed_subtitles: false,
            keep_lyrics_sidecar: true,
            ..AppSettings::default()
        }
    }

    #[test]
    fn copy_work_needs_a_conversion_or_kept_files() {
        assert!(copy_work_switched_on(&all_on()));
        assert!(copy_work_switched_on(&only_keep_on()));
        let nothing = AppSettings {
            keep_lyrics_sidecar: false,
            ..only_keep_on()
        };
        assert!(!copy_work_switched_on(&nothing));
        let conversion_only = AppSettings {
            enhanced_lrc: true,
            keep_lyrics_sidecar: false,
            ..nothing
        };
        assert!(copy_work_switched_on(&conversion_only));
    }

    #[test]
    fn with_every_conversion_off_a_better_ttml_still_reaches_the_copy() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let copy = add_copy(dir.path());
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();

        let s = run_copies(
            &only_keep_on(),
            &scope,
            None,
            past(),
            &[],
            None,
            None,
            &|| false,
        );
        assert_eq!(s.from_main, 1);
        let ttml = std::fs::read_to_string(copy.with_extension("ttml")).unwrap();
        assert!(ttml.contains("<span"), "the better ttml was copied across");
        for ext in ["lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                !copy.with_extension(ext).exists(),
                ".{ext} must not be created: every conversion is off"
            );
        }
    }

    #[test]
    fn with_every_conversion_and_keeping_off_nothing_is_touched() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let copy = add_copy(dir.path());
        std::fs::write(copy.with_extension("ttml"), LINE_TTML).unwrap();
        let before = folder_bytes(dir.path());
        let settings = AppSettings {
            keep_lyrics_sidecar: false,
            ..only_keep_on()
        };
        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s, Summary::default());
        assert_eq!(folder_bytes(dir.path()), before);
    }

    #[test]
    fn with_every_conversion_off_a_missing_ttml_is_created_for_the_copy() {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), WORD_TTML).unwrap();
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let copy = add_copy(dir.path());
        let s = run_copies(
            &only_keep_on(),
            &scope,
            None,
            past(),
            &[],
            None,
            None,
            &|| false,
        );
        assert_eq!(s.from_main, 1);
        assert!(copy.with_extension("ttml").is_file());
        assert!(!copy.with_extension("lrc").exists());
    }

    // ---- 18. the main download is upgraded from a better copy (#1442 c) ----

    /// A moment before this download's files were made (ten minutes ago).
    fn started() -> SystemTime {
        SystemTime::now() - Duration::from_secs(600)
    }

    /// An album whose main file has line-level lyrics and every conversion
    /// done by this download. The scope knows when the download started.
    fn album_with_finished_main() -> (tempfile::TempDir, AdvisoryScope, PathBuf) {
        let (dir, scope, main) = album();
        std::fs::write(main.with_extension("ttml"), LINE_TTML).unwrap();
        run_main_files_own(
            &all_on(),
            std::slice::from_ref(&main),
            Some("A"),
            Some("T"),
            &|| false,
        );
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(main.with_extension(ext).is_file(), "setup: no .{ext}");
        }
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            download_started_at: Some(started()),
            ..scope
        };
        (dir, scope, main)
    }

    /// Gives the copy word-by-word lyrics of its own (written by this run).
    fn give_word_lyrics(copy: &Path) {
        std::fs::write(copy.with_extension("ttml"), WORD_TTML).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn pick_upgrades_keeps_one_copy_per_main_file_and_only_better_ones() {
        use LyricsQuality::*;
        let m1 = PathBuf::from("/a/01.m4a");
        let m2 = PathBuf::from("/a/02.m4a");
        let picked = pick_upgrades(vec![
            // Two copies of song 1: the word-timed one wins.
            (m1.clone(), PathBuf::from("/a/01 z.m4a"), LineTtml, None),
            (
                m1.clone(),
                PathBuf::from("/a/01 b.m4a"),
                WordTimedTtml,
                None,
            ),
            // Equal: not better. Lower: not better.
            (m2.clone(), PathBuf::from("/a/02 a.m4a"), LineTtml, LineTtml),
            (
                PathBuf::from("/a/03.m4a"),
                PathBuf::from("/a/03 a.m4a"),
                None,
                LineTtml,
            ),
        ]);
        assert_eq!(
            picked,
            vec![PlannedUpgrade {
                main: m1,
                from_copy: PathBuf::from("/a/01 b.m4a"),
                quality: WordTimedTtml,
            }]
        );
    }

    #[test]
    fn pick_upgrades_breaks_a_tie_by_the_first_path() {
        use LyricsQuality::*;
        let m = PathBuf::from("/a/01.m4a");
        for order in [[0, 1], [1, 0]] {
            let names = ["/a/01 a.m4a", "/a/01 b.m4a"];
            let picked = pick_upgrades(
                order
                    .iter()
                    .map(|i| (m.clone(), PathBuf::from(names[*i]), WordTimedTtml, LineTtml))
                    .collect(),
            );
            assert_eq!(picked.len(), 1);
            assert_eq!(picked[0].from_copy, PathBuf::from("/a/01 a.m4a"));
        }
    }

    #[test]
    fn a_better_copy_upgrades_the_main_download() {
        let (dir, scope, main) = album_with_finished_main();
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let vtt_before = read(&main.with_extension("vtt"));
        let lyricsfile_before = read(&main.with_extension("lyrics"));

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        assert!(!s.stopped);
        assert!(read(&main.with_extension("ttml")).contains("<span"));
        assert!(read(&main.with_extension("lrc")).contains("<00:"));
        assert_ne!(read(&main.with_extension("vtt")), vtt_before);
        // The ASS file has no word timing, so it comes out the same; what
        // matters is that it was rebuilt under the main file's own name.
        let ass = read(&main.with_extension("ass"));
        // The ASS title is the MAIN file's own name, not the copy's.
        assert!(ass.contains("01 T [Dolby Atmos]"), "{ass}");
        assert!(!ass.contains("01 T [Lossless]"));
        assert_ne!(read(&main.with_extension("lyrics")), lyricsfile_before);
        // The lyrics inside the main file were upgraded too.
        let tag = mp4ameta::Tag::read_from_path(&main).unwrap();
        assert!(tag.lyrics().is_some_and(|l| l.contains("<00:")));
        let line = summary_line(&s).unwrap();
        assert!(
            line.contains("1 song in the main download got better lyrics from an extra copy."),
            "{line}"
        );
    }

    #[test]
    fn another_copy_of_the_same_song_gets_the_upgraded_lyrics() {
        let (dir, scope, main) = album_with_finished_main();
        let better = add_copy(dir.path());
        give_word_lyrics(&better);
        let other = dir.path().join("01 T [AAC].m4a");
        minimal_m4a(&other, 1, 1, false);
        std::fs::write(other.with_extension("ttml"), LINE_TTML).unwrap();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        assert_eq!(s.total, 2);
        // The line-level copy was given the main file's (now word-by-word)
        // lyrics, not left with its own.
        assert!(read(&other.with_extension("ttml")).contains("<span"));
        assert!(read(&other.with_extension("lrc")).contains("<00:"));
        assert!(read(&main.with_extension("ttml")).contains("<span"));
    }

    #[test]
    fn the_upgrade_replaces_only_files_this_download_made() {
        let (dir, scope, main) = album_with_finished_main();
        // Older files with the main file's names: the person's own.
        for ext in ["vtt", "ass", "lyrics"] {
            let p = main.with_extension(ext);
            std::fs::write(&p, "older").unwrap();
            make_old(&p);
        }
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        // What this download made is rebuilt ...
        assert!(read(&main.with_extension("ttml")).contains("<span"));
        assert!(read(&main.with_extension("lrc")).contains("<00:"));
        // ... what was there before is exactly as it was.
        for ext in ["vtt", "ass", "lyrics"] {
            assert_eq!(read(&main.with_extension(ext)), "older", ".{ext}");
        }
    }

    #[test]
    fn an_older_main_ttml_means_the_main_file_is_not_touched_at_all() {
        let (dir, scope, main) = album_with_finished_main();
        make_old(&main.with_extension("ttml"));
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let before: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("01 T [Dolby Atmos]"))
            .collect();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        let after: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("01 T [Dolby Atmos]"))
            .collect();
        assert_eq!(after, before, "no file of the main download may change");
    }

    #[test]
    fn an_older_lrc_beside_a_main_file_without_a_ttml_is_left_alone() {
        let (dir, scope, main) = album();
        let lrc = main.with_extension("lrc");
        std::fs::write(&lrc, "[00:01.00]older").unwrap();
        make_old(&lrc);
        let scope = AdvisoryScope {
            ttml_before_companions: ttml_files_in(dir.path()),
            download_started_at: Some(started()),
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let main_audio = std::fs::read(&main).unwrap();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        assert_eq!(read(&lrc), "[00:01.00]older");
        assert!(!main.with_extension("ttml").exists());
        assert_eq!(std::fs::read(&main).unwrap(), main_audio);
    }

    #[test]
    fn nothing_is_replaced_when_the_download_start_is_unknown() {
        let (dir, scope, main) = album_with_finished_main();
        let scope = AdvisoryScope {
            download_started_at: None,
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let before: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("01 T [Dolby Atmos]"))
            .collect();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        let after: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("01 T [Dolby Atmos]"))
            .collect();
        assert_eq!(after, before);
        assert!(read(&main.with_extension("ttml")).contains("Hello"));
    }

    #[test]
    fn older_songs_in_the_folder_are_never_touched_by_an_upgrade() {
        let (dir, _scope, main) = album();
        std::fs::write(main.with_extension("ttml"), LINE_TTML).unwrap();
        // An older song with its own lyrics files.
        let old = dir.path().join("02 Old.m4a");
        minimal_m4a(&old, 1, 2, false);
        for ext in ["ttml", "lrc", "vtt"] {
            let p = old.with_extension(ext);
            std::fs::write(&p, "older song").unwrap();
            make_old(&p);
        }
        make_old(&old);
        let scope = AdvisoryScope {
            dir: dir.path().to_path_buf(),
            this_download: vec![main.clone()],
            before_companions: audio_files_in(dir.path()),
            ttml_before_companions: ttml_files_in(dir.path()),
            download_started_at: Some(started()),
            written_this_run: Some(std::iter::once(main.clone()).collect()),
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let before: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("02 Old"))
            .collect();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        let after: Vec<_> = folder_bytes(dir.path())
            .into_iter()
            .filter(|(name, _)| name.starts_with("02 Old"))
            .collect();
        assert_eq!(after, before, "the older song's files must not change");
    }

    #[test]
    fn with_separate_files_off_the_upgrade_creates_no_file_but_embeds() {
        let (dir, scope, main) = album();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let settings = AppSettings {
            keep_lyrics_sidecar: false,
            ..all_on()
        };

        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                !main.with_extension(ext).exists(),
                ".{ext} must not be created for the main file"
            );
        }
        let tag = mp4ameta::Tag::read_from_path(&main).unwrap();
        assert!(tag.lyrics().is_some_and(|l| l.contains("<00:")));
    }

    #[test]
    fn a_stop_before_an_upgrade_changes_nothing_and_is_reported() {
        let (dir, scope, main) = album_with_finished_main();
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let ttml_before = read(&main.with_extension("ttml"));
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| true);
        assert!(s.stopped);
        assert_eq!(s.main_upgraded, 0);
        assert_eq!(read(&main.with_extension("ttml")), ttml_before);
    }

    #[test]
    fn a_main_file_this_run_did_not_write_is_not_upgraded() {
        // Named like this run's files (so in `this_download`), but older: not
        // in the set of files this run wrote.
        let (dir, scope, main) = album();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            written_this_run: Some(std::collections::BTreeSet::new()),
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let main_audio = std::fs::read(&main).unwrap();

        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        for ext in ["ttml", "lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(!main.with_extension(ext).exists(), ".{ext} must not appear");
        }
        assert_eq!(std::fs::read(&main).unwrap(), main_audio);
    }

    #[test]
    fn a_main_file_this_run_wrote_is_upgraded_even_with_no_lyric_files() {
        let (dir, scope, main) = album();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            ..scope
        };
        assert!(scope.written_this_run.is_some());
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        assert!(read(&main.with_extension("ttml")).contains("<span"));
    }

    #[test]
    fn an_upgrade_never_embeds_a_leftover_srt_from_the_copy() {
        let (dir, scope, main) = album();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        // A leftover `.srt` beside the copy, from an older download.
        std::fs::write(
            copy.with_extension("srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nOLD LEFTOVER\n",
        )
        .unwrap();
        let settings = AppSettings {
            embed_subtitles: true,
            keep_lyrics_sidecar: false,
            ..all_on()
        };
        let s = run_copies(&settings, &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 1);
        let tag = mp4ameta::Tag::read_from_path(&main).unwrap();
        let embedded: Option<String> = tag
            .data_of(&mp4ameta::FreeformIdent::new_static(
                "com.apple.iTunes",
                "subtitles-srt",
            ))
            .next()
            .and_then(|d| d.string().map(str::to_owned));
        assert!(
            !embedded.unwrap_or_default().contains("OLD LEFTOVER"),
            "the copy's leftover .srt reached the main file"
        );
    }

    #[test]
    fn nothing_is_upgraded_when_the_written_set_is_unknown() {
        let (dir, scope, main) = album();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            written_this_run: None,
            ..scope
        };
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        assert!(!main.with_extension("ttml").exists());
    }

    #[test]
    fn a_leftover_ttml_beside_a_copy_never_becomes_the_main_files_lyrics() {
        let (dir, scope, main) = album();
        let copy = add_copy(dir.path());
        // The copy's `.ttml` was already in the folder before the copies started.
        give_word_lyrics(&copy);
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        assert!(!main.with_extension("ttml").exists());
    }

    #[test]
    fn a_copy_with_only_a_leftover_lrc_is_never_an_upgrade_source() {
        let (dir, scope, main) = album();
        let copy = add_copy(dir.path());
        // No `.ttml` beside the copy, only an older `.lrc` nothing can check.
        std::fs::write(copy.with_extension("lrc"), "[00:01.00]Old line\n").unwrap();
        let scope = AdvisoryScope {
            download_started_at: Some(started()),
            ttml_before_companions: ttml_files_in(dir.path()),
            ..scope
        };
        let s = run_copies(&all_on(), &scope, None, past(), &[], None, None, &|| false);
        assert_eq!(s.main_upgraded, 0);
        assert!(!main.with_extension("lrc").exists());
    }

    #[test]
    fn summary_line_mentions_upgrades_when_stopped_early() {
        let r = Summary {
            total: 3,
            unchanged: 1,
            stopped: true,
            main_upgraded: 2,
            ..Summary::default()
        };
        let line = summary_line(&r).unwrap();
        assert!(line.contains("stopped early (1 of 3 done)"), "{line}");
        assert!(
            line.contains("2 songs in the main download got better lyrics"),
            "{line}"
        );
    }

    #[test]
    fn summary_line_mentions_main_upgrades() {
        let only_main = Summary {
            total: 1,
            unchanged: 1,
            main_upgraded: 2,
            ..Summary::default()
        };
        assert_eq!(
            summary_line(&only_main).unwrap(),
            "2 songs in the main download got better lyrics from an extra copy."
        );
        let both = Summary {
            total: 1,
            from_main: 1,
            main_upgraded: 1,
            ..Summary::default()
        };
        let line = summary_line(&both).unwrap();
        assert!(line.starts_with("Lyrics added to 1 extra copy:"), "{line}");
        assert!(line.ends_with("1 song in the main download got better lyrics from an extra copy."));
    }

    #[tokio::test]
    async fn an_upgrade_waits_for_the_main_files_lock() {
        let (dir, scope, main) = album_with_finished_main();
        let copy = add_copy(dir.path());
        give_word_lyrics(&copy);
        let held = crate::utils::file_locks::global().lock(&main).await;
        let settings = all_on();
        let fut = run_copies_locked(
            crate::utils::file_locks::global(),
            &settings,
            &scope,
            None,
            past(),
            &[],
            None,
            None,
            &|| false,
        );
        tokio::pin!(fut);
        let waited = tokio::time::timeout(Duration::from_millis(100), &mut fut).await;
        assert!(waited.is_err(), "must wait while the main file is locked");
        assert!(read(&main.with_extension("ttml")).contains("Hello"));
        drop(held);
        let summary = tokio::time::timeout(Duration::from_secs(20), fut)
            .await
            .expect("the upgrade must finish once the lock is free");
        assert_eq!(summary.main_upgraded, 1);
        assert!(read(&main.with_extension("ttml")).contains("<span"));
    }

    // ---- 19. a panic is not retried (#1442 hardening) ----

    #[test]
    fn a_file_that_panics_is_not_put_back_but_the_others_are() {
        let pending = Arc::new(StdMutex::new(None));
        let a = PathBuf::from("/x/a.m4a");
        let b = PathBuf::from("/x/b.m4a");
        let files = vec![a.clone(), b.clone()];
        let mut restore = PendingRestore::new(Some(pending.clone()), &files);
        restore.begin(&a);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _held = restore;
            panic!("test panic while a file is being worked on");
        }));
        assert!(result.is_err());
        assert_eq!(*pending.lock().unwrap(), Some(vec![b]));
    }

    #[test]
    fn without_a_panic_the_unfinished_file_is_put_back() {
        let pending = Arc::new(StdMutex::new(None));
        let a = PathBuf::from("/x/a.m4a");
        let b = PathBuf::from("/x/b.m4a");
        let files = vec![a.clone(), b.clone()];
        let mut restore = PendingRestore::new(Some(pending.clone()), &files);
        restore.begin(&a);
        drop(restore);
        assert_eq!(*pending.lock().unwrap(), Some(vec![a, b]));
    }
}
