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
// Nothing here deletes a file (a test checks the source for delete calls).
// A `.ttml`, `.vtt`, `.ass` or `.lyrics` file that was already on disk is
// never replaced. The `.lrc` and Rich `.srt` files are always written fresh,
// exactly as the main download's Steps 2 and 2d do.

use super::lyrics_pass::{
    audio_files_in, fresh_at_snapshot, place_track, read_audio_numbers, read_copy_evidence,
    AdvisoryScope, AudioFileNumbers, Placement,
};
use super::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

// ============================================================
// Pure decisions
// ============================================================

/// How good the lyrics next to one audio file are, from what is on disk.
/// Ordered from worst to best, so `>` means "better".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LyricsQuality {
    /// No lyrics file of any kind.
    None,
    /// No `.ttml`, but a `.lrc` or `.srt` (for example from the lyrics
    /// fallback, or a non-TTML format chosen in GAMDL).
    OtherOnly,
    /// A `.ttml` with line timing only.
    LineTtml,
    /// A `.ttml` with word-by-word timing.
    WordTimedTtml,
}

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
}

/// The plain-English activity-log line for a run, or `None` when nothing
/// was done and there is nothing to say.
pub(crate) fn summary_line(r: &Summary) -> Option<String> {
    let done = r.from_main + r.own + r.none + r.unchanged;
    if r.stopped {
        return (r.total > 0).then(|| {
            format!(
                "Adding lyrics to the extra copies stopped early ({done} of {} done).",
                r.total
            )
        });
    }
    let added = r.from_main + r.own;
    if added == 0 {
        return None;
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
    Some(line)
}

// ============================================================
// File work (synchronous; called directly inside the companion task)
// ============================================================

/// The lyrics files next to one audio file, read into memory.
#[derive(Debug, Clone, Default)]
struct LyricTexts {
    /// The `.ttml` text.
    ttml: Option<String>,
    /// The `.lrc` text.
    lrc: Option<String>,
    /// The `.srt` text.
    srt: Option<String>,
}

/// Reads a text file, `None` when it is missing or unreadable.
fn read_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Reads the `.ttml`, `.lrc` and `.srt` files that share an audio file's
/// name.
fn read_lyric_texts(audio: &Path) -> LyricTexts {
    LyricTexts {
        ttml: read_text(&audio.with_extension("ttml")),
        lrc: read_text(&audio.with_extension("lrc")),
        srt: read_text(&audio.with_extension("srt")),
    }
}

/// How good a set of lyric texts is.
fn quality_of_texts(t: &LyricTexts) -> LyricsQuality {
    if let Some(ttml) = &t.ttml {
        return if super::enhanced_lyrics_service::ttml_has_word_timing(ttml) {
            LyricsQuality::WordTimedTtml
        } else {
            LyricsQuality::LineTtml
        };
    }
    if t.lrc.is_some() || t.srt.is_some() {
        LyricsQuality::OtherOnly
    } else {
        LyricsQuality::None
    }
}

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

/// Writes a file and says whether it worked (a failure is only logged).
fn write_file(path: &Path, content: impl AsRef<[u8]>) -> bool {
    match std::fs::write(path, content) {
        Ok(()) => true,
        Err(e) => {
            log::debug!("Extra copy lyrics: could not write {}: {e}", path.display());
            false
        }
    }
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
fn process_copy(
    copy: &Path,
    primary_audio: Option<&Path>,
    s: &AppSettings,
    written_by_this_run: &dyn Fn(&Path) -> bool,
    artist: Option<&str>,
    default_title: Option<&str>,
    stop_requested: &dyn Fn() -> bool,
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
    let primary_texts = primary_audio.map(read_lyric_texts);
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
    // The text of a sidecar for the later steps: what is on disk now, else
    // what the source had in memory (when separate files are switched off
    // nothing was written, but the source can still be converted).
    let disk_or = |ext: &str, fallback: &Option<String>| -> Option<String> {
        read_text(&copy.with_extension(ext)).or_else(|| fallback.clone())
    };

    // Step 2: Enhanced LRC. The `.lrc` file only when separate files are
    // kept; the lyrics always go inside the music file.
    if s.enhanced_lrc && !stop() {
        if let Some(ttml) = ttml {
            match super::enhanced_lyrics_service::ttml_to_enhanced_lrc(ttml) {
                Ok(result) => {
                    if keep {
                        did_work |= write_file(&copy.with_extension("lrc"), &result.lrc_content);
                    }
                    match super::enhanced_lyrics_service::embed_enhanced_lrc_in_file(
                        copy,
                        &result.lrc_content,
                    ) {
                        Ok(_) => did_work = true,
                        Err(e) => log::debug!(
                            "Extra copy lyrics: could not embed in {}: {e}",
                            copy.display()
                        ),
                    }
                }
                Err(e) => log::debug!("Extra copy lyrics: Enhanced LRC failed: {e}"),
            }
        }
    }

    // Step 2c: WebVTT (TTML, then SRT, then LRC), skipped if one exists.
    if s.generate_webvtt && keep && !stop() {
        let vtt_path = copy.with_extension("vtt");
        if !vtt_path.exists() {
            let vtt = ttml
                .and_then(|t| super::webvtt_service::ttml_to_webvtt(t).ok())
                .or_else(|| {
                    disk_or("srt", &texts.srt).map(|t| super::webvtt_service::srt_to_webvtt(&t))
                })
                .or_else(|| {
                    disk_or("lrc", &texts.lrc).map(|t| super::webvtt_service::lrc_to_webvtt(&t))
                });
            if let Some(vtt) = vtt {
                did_work |= write_file(&vtt_path, vtt);
            }
        }
    }

    // Step 2d: rich SRT (TTML, then WebVTT); replaces a plain SRT.
    if s.generate_rich_srt && keep && !stop() {
        let srt = ttml
            .and_then(|t| super::rich_srt_service::ttml_to_rich_srt(t).ok())
            .or_else(|| {
                read_text(&copy.with_extension("vtt"))
                    .and_then(|v| super::rich_srt_service::webvtt_to_rich_srt(&v).ok())
            });
        if let Some(srt) = srt {
            did_work |= write_file(&copy.with_extension("srt"), srt);
        }
    }

    // Step 2e: embed the subtitle files in the music file (not gated on
    // keeping separate files).
    if s.embed_subtitles && !stop() {
        // With separate files switched off, a copy given the main download's
        // `.srt` has no file of its own, so use the text read from the main
        // download — as the main file itself gets it embedded.
        let srt = read_text(&copy.with_extension("srt")).or_else(|| {
            if from_main {
                texts.srt.clone()
            } else {
                None
            }
        });
        let vtt = read_text(&copy.with_extension("vtt"));
        if srt.is_some() || vtt.is_some() {
            match super::rich_srt_service::embed_subtitles_in_file(
                copy,
                srt.as_deref(),
                vtt.as_deref(),
            ) {
                Ok(_) => did_work = true,
                Err(e) => log::debug!("Extra copy lyrics: could not embed subtitles: {e}"),
            }
        }
    }

    // Step 2f: ASS (TTML, then WebVTT), titled with the COPY's own name.
    if s.generate_ass && keep && !stop() {
        let ass_path = copy.with_extension("ass");
        if !ass_path.exists() {
            let ass = ttml
                .and_then(|t| super::ass_subtitle_service::ttml_to_ass(t, &stem).ok())
                .or_else(|| {
                    read_text(&copy.with_extension("vtt"))
                        .and_then(|v| super::ass_subtitle_service::webvtt_to_ass(&v, &stem).ok())
                });
            if let Some(ass) = ass {
                did_work |= write_file(&ass_path, ass);
            }
        }
    }

    // Step 2g: Lyricsfile, skipped if one exists.
    if s.generate_lyricsfile && keep && !stop() {
        let path = copy.with_extension("lyrics");
        if !path.exists() {
            if let Some(ttml) = ttml {
                match super::lyricsfile_service::lyricsfile_from_ttml(
                    ttml,
                    &stem,
                    default_title.unwrap_or("Untitled"),
                    artist.unwrap_or("Unknown Artist"),
                ) {
                    Ok(yaml) => did_work |= write_file(&path, yaml),
                    Err(e) => log::debug!("Extra copy lyrics: Lyricsfile failed: {e}"),
                }
            }
        }
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
fn resolve_extra_dir(
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

/// Gives every new copy of this download its lyrics, as described at the
/// top of this file. `stop` is asked before each copy; when it says yes the
/// run ends and the summary says so (files already done stay done).
///
/// Settings are read by the caller when this starts, not when enrichment
/// ran, so a switch changed in between applies to the copies.
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
    // Nothing switched on: do not even look at the folder.
    if !(settings.enhanced_lrc
        || settings.generate_webvtt
        || settings.generate_rich_srt
        || settings.generate_ass
        || settings.generate_lyricsfile
        || settings.embed_subtitles)
    {
        return Summary::default();
    }
    let copies = this_downloads_copies(scope, extra_dir, since);
    let mut summary = Summary {
        total: copies.len(),
        ..Summary::default()
    };
    if copies.is_empty() {
        return summary;
    }
    let primaries: Vec<Evidence> = scope
        .this_download
        .iter()
        .filter(|p| p.is_file())
        .map(|p| read_evidence(p))
        .collect();
    let single_disc = single_disc_album(album_tracks, &primaries);

    for copy in &copies {
        if stop() {
            summary.stopped = true;
            return summary;
        }
        let evidence = read_evidence(copy);
        let outside = !copy.starts_with(&scope.dir);
        let primary = match_primary(&evidence, &primaries, single_disc, outside)
            .map(|i| primaries[i].path.clone());
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
        match process_copy(
            copy,
            primary.as_deref(),
            settings,
            &written_by_this_run,
            lyrics_artist,
            default_title,
            stop,
        ) {
            CopyOutcome::FromMain { word_timed } => {
                summary.from_main += 1;
                if word_timed {
                    summary.from_main_word_timed += 1;
                }
            }
            CopyOutcome::Own => summary.own += 1,
            CopyOutcome::Unchanged => summary.unchanged += 1,
            CopyOutcome::Nothing => summary.none += 1,
            // A stop cut this copy short: report it, and do not count the
            // copy as done.
            CopyOutcome::Stopped => {
                summary.stopped = true;
                return summary;
            }
        }
    }
    summary
}

/// The whole step for the companion task: works out where the copies are
/// (including a folder outside the album folder) and runs
/// [`run_copies`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    settings: &AppSettings,
    scope: &AdvisoryScope,
    output_dir: &Path,
    artist_hint: Option<&str>,
    album_hint: Option<&str>,
    since: SystemTime,
    album_tracks: &[(u32, u32, String)],
    lyricsfile_artist: Option<&str>,
    default_title: Option<&str>,
    stop: &dyn Fn() -> bool,
) -> Summary {
    let extra_dir = resolve_extra_dir(output_dir, artist_hint, album_hint, &scope.dir);
    run_copies(
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
            ..AppSettings::default()
        };
        assert_eq!(
            run_copies(&s, &scope, None, past(), &[], None, None, &|| false),
            Summary::default()
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
}
