// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// The lyrics and subtitle steps, one music file at a time (#1441).
//
// Enrichment's Steps 2, 2c, 2d, 2e, 2f and 2g each used to hand a whole
// folder to a folder-wide service function. That also converted older files
// that happened to sit in the same folder (rewriting their `.lrc` and `.srt`,
// re-embedding their lyrics), and it missed disc sub-folders. This module is
// the one per-file implementation of those steps:
//
// - The extra-format copies' driver (`companion_lyrics::process_copy`) calls
//   the step functions below in order, for one copy.
// - The main download's Steps 2 and 2c to 2g call the `*_for_files` batch
//   wrappers over THIS download's list of files
//   ([`main_lyrics_targets`]), so no other file in the folder is touched.
//
// Every function here works on the music files it is handed and the lyric
// files that share their names. Nothing here deletes a file (a test checks
// the source for delete calls). A `.vtt`, `.ass` or `.lyrics` file that is
// already on disk is replaced only when the caller says so through
// `StepInput::replace_existing`; the main download's own steps never do.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

// ============================================================
// What is next to a music file
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

/// The lyrics files next to one audio file, read into memory.
#[derive(Debug, Clone, Default)]
pub(crate) struct LyricTexts {
    /// The `.ttml` text.
    pub ttml: Option<String>,
    /// The `.lrc` text.
    pub lrc: Option<String>,
    /// The `.srt` text.
    pub srt: Option<String>,
}

/// Reads a text file, `None` when it is missing or unreadable.
pub(crate) fn read_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Reads the `.ttml`, `.lrc` and `.srt` files that share an audio file's
/// name.
pub(crate) fn read_lyric_texts(audio: &Path) -> LyricTexts {
    LyricTexts {
        ttml: read_text(&audio.with_extension("ttml")),
        lrc: read_text(&audio.with_extension("lrc")),
        srt: read_text(&audio.with_extension("srt")),
    }
}

/// How good a set of lyric texts is.
pub(crate) fn quality_of_texts(t: &LyricTexts) -> LyricsQuality {
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

/// Writes a file and says whether it worked (a failure is only logged).
pub(crate) fn write_file(path: &Path, content: impl AsRef<[u8]>) -> bool {
    match std::fs::write(path, content) {
        Ok(()) => true,
        Err(e) => {
            log::debug!("Lyrics step: could not write {}: {e}", path.display());
            false
        }
    }
}

// ============================================================
// One step for one file
// ============================================================

/// Everything one step needs to know about one music file.
pub(crate) struct StepInput<'a> {
    /// The music file the lyric files belong to (and that lyrics are
    /// embedded in).
    pub audio: &'a Path,
    /// The file's name without its extension.
    pub stem: &'a str,
    /// The text the conversions start from.
    pub source: &'a LyricTexts,
    /// The source came from a different file (the main download's lyrics
    /// given to an extra copy). Only the subtitle embedding reads this: such
    /// a file has no `.srt` of its own to read back when separate files are
    /// switched off.
    pub from_other: bool,
    /// "Keep separate lyrics files" is on. Only Step 2 reads this (it
    /// writes the `.lrc` file only when it is on, but always embeds); the
    /// callers have already checked it for the later steps.
    pub keep: bool,
    /// May this existing `.vtt`, `.ass` or `.lyrics` file be replaced?
    /// The extra copies and the main download's own steps never replace one
    /// (`|_| false`).
    pub replace_existing: &'a dyn Fn(&Path) -> bool,
}

/// What one step did for one file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepDone {
    /// A file was written or lyrics were embedded.
    pub wrote: bool,
}

/// Whether an existing output file stands in the way of writing a new one.
fn blocked_by_existing(i: &StepInput, path: &Path) -> bool {
    path.exists() && !(i.replace_existing)(path)
}

/// The text of a lyric file for the later steps: what is on disk now, else
/// what the source had in memory (when separate files are switched off
/// nothing was written, but the source can still be converted).
fn disk_or(audio: &Path, ext: &str, fallback: &Option<String>) -> Option<String> {
    read_text(&audio.with_extension(ext)).or_else(|| fallback.clone())
}

/// Step 2: Enhanced LRC. Turns the `.ttml` into a `.lrc` file (only when
/// separate files are kept) and always embeds the lyrics in the music file.
/// Embedding happens even when the `.lrc` file could not be written. Does
/// nothing without a `.ttml`.
pub(crate) fn step_enhanced_lrc(i: &StepInput) -> StepDone {
    let mut done = StepDone::default();
    let Some(ttml) = i.source.ttml.as_deref() else {
        return done;
    };
    match super::enhanced_lyrics_service::ttml_to_enhanced_lrc(ttml) {
        Ok(result) => {
            if i.keep {
                done.wrote |= write_file(&i.audio.with_extension("lrc"), &result.lrc_content);
            }
            match super::enhanced_lyrics_service::embed_enhanced_lrc_in_file(
                i.audio,
                &result.lrc_content,
            ) {
                Ok(_) => done.wrote = true,
                Err(e) => log::debug!("Lyrics step: could not embed in {}: {e}", i.audio.display()),
            }
        }
        Err(e) => log::debug!("Lyrics step: Enhanced LRC failed: {e}"),
    }
    done
}

/// Step 2c: WebVTT, from the `.ttml`, else the `.srt`, else the `.lrc`.
/// Skipped when a `.vtt` file is already there (unless the caller allows
/// replacing it).
pub(crate) fn step_webvtt(i: &StepInput) -> StepDone {
    let mut done = StepDone::default();
    let vtt_path = i.audio.with_extension("vtt");
    if blocked_by_existing(i, &vtt_path) {
        return done;
    }
    let vtt = i
        .source
        .ttml
        .as_deref()
        .and_then(|t| super::webvtt_service::ttml_to_webvtt(t).ok())
        .or_else(|| {
            disk_or(i.audio, "srt", &i.source.srt).map(|t| super::webvtt_service::srt_to_webvtt(&t))
        })
        .or_else(|| {
            disk_or(i.audio, "lrc", &i.source.lrc).map(|t| super::webvtt_service::lrc_to_webvtt(&t))
        });
    if let Some(vtt) = vtt {
        done.wrote = write_file(&vtt_path, vtt);
    }
    done
}

/// Step 2d: rich SRT, from the `.ttml`, else the `.vtt`. Always written
/// fresh, replacing a plain `.srt`.
pub(crate) fn step_rich_srt(i: &StepInput) -> StepDone {
    let mut done = StepDone::default();
    let srt = i
        .source
        .ttml
        .as_deref()
        .and_then(|t| super::rich_srt_service::ttml_to_rich_srt(t).ok())
        .or_else(|| {
            read_text(&i.audio.with_extension("vtt"))
                .and_then(|v| super::rich_srt_service::webvtt_to_rich_srt(&v).ok())
        });
    if let Some(srt) = srt {
        done.wrote = write_file(&i.audio.with_extension("srt"), srt);
    }
    done
}

/// Step 2e: embeds the `.srt` and `.vtt` text in the music file. Not gated
/// on keeping separate files. With the source from another file and no `.srt`
/// of its own on disk, the source's `.srt` text is embedded.
pub(crate) fn step_embed_subtitles(i: &StepInput) -> StepDone {
    let mut done = StepDone::default();
    let srt = read_text(&i.audio.with_extension("srt")).or_else(|| {
        if i.from_other {
            i.source.srt.clone()
        } else {
            None
        }
    });
    let vtt = read_text(&i.audio.with_extension("vtt"));
    if srt.is_some() || vtt.is_some() {
        match super::rich_srt_service::embed_subtitles_in_file(
            i.audio,
            srt.as_deref(),
            vtt.as_deref(),
        ) {
            Ok(_) => done.wrote = true,
            Err(e) => log::debug!("Lyrics step: could not embed subtitles: {e}"),
        }
    }
    done
}

/// Step 2f: ASS, from the `.ttml`, else the `.vtt`, titled with the file's
/// own name. Skipped when an `.ass` file is already there (unless the caller
/// allows replacing it).
pub(crate) fn step_ass(i: &StepInput) -> StepDone {
    let mut done = StepDone::default();
    let ass_path = i.audio.with_extension("ass");
    if blocked_by_existing(i, &ass_path) {
        return done;
    }
    let ass = i
        .source
        .ttml
        .as_deref()
        .and_then(|t| super::ass_subtitle_service::ttml_to_ass(t, i.stem).ok())
        .or_else(|| {
            read_text(&i.audio.with_extension("vtt"))
                .and_then(|v| super::ass_subtitle_service::webvtt_to_ass(&v, i.stem).ok())
        });
    if let Some(ass) = ass {
        done.wrote = write_file(&ass_path, ass);
    }
    done
}

/// Step 2g: a Lyricsfile (`.lyrics`) from the `.ttml`. Skipped when one is
/// already there (unless the caller allows replacing it): it may carry the
/// person's own edits. `title` and `artist` are the fallbacks used when the
/// file name gives none.
pub(crate) fn step_lyricsfile(i: &StepInput, title: &str, artist: &str) -> StepDone {
    let mut done = StepDone::default();
    let path = i.audio.with_extension("lyrics");
    if blocked_by_existing(i, &path) {
        return done;
    }
    if let Some(ttml) = i.source.ttml.as_deref() {
        match super::lyricsfile_service::lyricsfile_from_ttml(ttml, i.stem, title, artist) {
            Ok(yaml) => done.wrote = write_file(&path, yaml),
            Err(e) => log::debug!("Lyrics step: Lyricsfile failed: {e}"),
        }
    }
    done
}

// ============================================================
// The main download: one step over a list of files
// ============================================================

/// Runs one step on each listed file, each time with the file's own lyric
/// files as the source and nothing ever replaced. Each file's step runs under
/// the app-wide write lock for that file, so this must be called from a
/// blocking thread (`spawn_blocking`) or a plain thread. Files that are gone, and
/// operating-system shadow files, are skipped. Returns how many files the
/// step did something for.
fn run_for_files(
    files: &[PathBuf],
    keep: bool,
    mut step: impl FnMut(&StepInput) -> StepDone,
) -> usize {
    let never_replace = |_: &Path| false;
    let mut count = 0;
    for file in files {
        if !file.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(file) {
            continue;
        }
        let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // The app-wide write lock for this file, taken here in the per-file
        // loop and held for this one step (the steps rewrite the file's tags
        // when they embed). The step functions themselves never lock. These
        // batch wrappers run on a blocking thread, where waiting is allowed.
        let _write_guard = crate::utils::file_locks::global().lock_blocking(file);
        let texts = read_lyric_texts(file);
        let input = StepInput {
            audio: file,
            stem,
            source: &texts,
            from_other: false,
            keep,
            replace_existing: &never_replace,
        };
        if step(&input).wrote {
            count += 1;
        }
    }
    count
}

/// Step 2 for the main download's files: Enhanced LRC. `keep` is "keep
/// separate lyrics files". Returns how many files got lyrics.
pub(crate) fn enhanced_lrc_for_files(files: &[PathBuf], keep: bool) -> Result<usize, String> {
    Ok(run_for_files(files, keep, step_enhanced_lrc))
}

/// Step 2c for the main download's files: WebVTT. The caller has already
/// checked that separate files are kept.
pub(crate) fn webvtt_for_files(files: &[PathBuf]) -> Result<usize, String> {
    Ok(run_for_files(files, true, step_webvtt))
}

/// Step 2d for the main download's files: rich SRT. The caller has already
/// checked that separate files are kept.
pub(crate) fn rich_srt_for_files(files: &[PathBuf]) -> Result<usize, String> {
    Ok(run_for_files(files, true, step_rich_srt))
}

/// Step 2e for the main download's files: subtitle embedding.
pub(crate) fn embed_subtitles_for_files(files: &[PathBuf]) -> Result<usize, String> {
    Ok(run_for_files(files, true, step_embed_subtitles))
}

/// Step 2f for the main download's files: ASS. The caller has already
/// checked that separate files are kept.
pub(crate) fn ass_for_files(files: &[PathBuf]) -> Result<usize, String> {
    Ok(run_for_files(files, true, step_ass))
}

/// Step 2g for the main download's files: Lyricsfile. The caller has
/// already checked that separate files are kept. `title` and `artist` are
/// the fallbacks the file names cannot give.
pub(crate) fn lyricsfile_for_files(
    files: &[PathBuf],
    title: &str,
    artist: &str,
) -> Result<usize, String> {
    Ok(run_for_files(files, true, |i| {
        step_lyricsfile(i, title, artist)
    }))
}

// ============================================================
// Which files the main download's lyrics steps work on
// ============================================================

/// The files the main download's lyrics steps work on: this download's
/// music files, this download's music videos (judged when the download
/// finished, before any tag rewrite), and any file a word-by-word `.ttml`
/// was written beside. Sorted, no duplicates. Older files in the folder are
/// never in the list.
pub(crate) fn main_lyrics_targets(
    this_download: &[PathBuf],
    fresh_videos: &[PathBuf],
    step_1b_wrote_beside: &[PathBuf],
) -> Vec<PathBuf> {
    this_download
        .iter()
        .chain(fresh_videos)
        .chain(step_1b_wrote_beside)
        .cloned()
        .collect::<std::collections::BTreeSet<PathBuf>>()
        .into_iter()
        .collect()
}

/// The provisional list of the lyrics steps' files, known as soon as the
/// download's snapshot is taken (before Step 1b and the tag rewrite): the
/// music files written since the start, and the fresh videos. Replaced by
/// the exact list ([`main_lyrics_targets`]) once it is worked out.
pub(crate) fn provisional_main_targets(
    primary_files: &[PathBuf],
    fresh_files: &std::collections::HashSet<PathBuf>,
    fresh_videos: &[PathBuf],
) -> Vec<PathBuf> {
    let fresh: Vec<PathBuf> = primary_files
        .iter()
        .filter(|p| fresh_files.contains(*p))
        .cloned()
        .collect();
    main_lyrics_targets(&fresh, fresh_videos, &[])
}

/// How well the listed files are covered by lyrics: `(files with a
/// `.ttml`, `.lrc` or `.srt` beside them, music files, video files)`.
/// Only the listed files are looked at, so an older file in the same folder
/// can neither raise nor lower the coverage.
pub(crate) fn lyrics_coverage(targets: &[PathBuf]) -> (usize, usize, usize) {
    let (mut with_lyrics, mut audio, mut video) = (0, 0, 0);
    for file in targets {
        if !file.is_file() || crate::utils::fs_safe::is_filesystem_sidecar(file) {
            continue;
        }
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "m4a" => audio += 1,
            "m4v" | "mp4" => video += 1,
            _ => continue,
        }
        if ["ttml", "lrc", "srt"]
            .iter()
            .any(|e| file.with_extension(e).is_file())
        {
            with_lyrics += 1;
        }
    }
    (with_lyrics, audio, video)
}

// ============================================================
// Writes still running when enrichment is abandoned (#1441)
// ============================================================

/// Counts the file-writing jobs enrichment has handed to a blocking thread
/// between its snapshot of the main download's files and the end of Step 2g.
/// Each such job holds a SHARED (read) hold for as long as it runs. A task
/// that wants to finish the lyrics itself after enrichment was abandoned
/// (its deadline) takes the EXCLUSIVE (write) hold first, so it never writes
/// the same files at the same time as a job the abandoned task left running:
/// aborting a task cannot stop a blocking closure that has already started.
pub(crate) type InFlightWrites = Arc<tokio::sync::RwLock<()>>;

/// A fresh, empty tracker.
pub(crate) fn new_in_flight_writes() -> InFlightWrites {
    Arc::new(tokio::sync::RwLock::new(()))
}

/// Runs `job` on a blocking thread while holding a shared hold on
/// `in_flight`. The hold is taken BEFORE the job is handed over and moves
/// into it, so it lasts until the job really ends, even if the caller is
/// abandoned in the meantime.
pub(crate) async fn run_blocking_held<T, F>(
    in_flight: &InFlightWrites,
    job: F,
) -> Result<T, tokio::task::JoinError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let held = in_flight.clone().read_owned().await;
    tokio::task::spawn_blocking(move || {
        let _held = held;
        job()
    })
    .await
}

/// How long a catch-up waits for abandoned writing jobs to end before it
/// gives up rather than race them.
pub(crate) const CATCH_UP_WAIT: Duration = Duration::from_secs(5 * 60);

/// What [`wait_to_catch_up`] found.
pub(crate) enum CatchUpWait {
    /// Nothing is waiting to be finished.
    NothingPending,
    /// Every writing job has ended; keep this while writing.
    #[allow(dead_code)] // kept for its Drop: it is the hold itself
    Ready(tokio::sync::OwnedRwLockWriteGuard<()>),
    /// A writing job was still running after the wait: skip the catch-up.
    TimedOut,
}

/// Waits (at most `limit`) until no abandoned writing job is running, if the
/// main download has files whose lyrics are still unfinished. The returned
/// hold must be kept until the catch-up has finished writing.
pub(crate) async fn wait_to_catch_up(
    pending: Option<&Arc<StdMutex<Option<Vec<PathBuf>>>>>,
    in_flight: &InFlightWrites,
    limit: Duration,
) -> CatchUpWait {
    let has_pending = pending
        .and_then(|p| p.lock().ok().map(|g| g.is_some()))
        .unwrap_or(false);
    if !has_pending {
        return CatchUpWait::NothingPending;
    }
    match tokio::time::timeout(limit, in_flight.clone().write_owned()).await {
        Ok(guard) => CatchUpWait::Ready(guard),
        Err(_) => CatchUpWait::TimedOut,
    }
}

/// The artist and title the Lyricsfile step writes for a catch-up. The
/// details from enrichment win; otherwise the names MeedyaDL already holds
/// for the queue item stand in (the album name for the title). Never a
/// placeholder: a `.lyrics` file is kept once written, so a made-up name
/// would stay for good. `None` for the artist means "write none".
pub(crate) fn lyricsfile_names_for_catch_up(
    gate_artist: Option<&str>,
    gate_title: Option<&str>,
    queue_artist: Option<&str>,
    queue_album: Option<&str>,
) -> (Option<String>, Option<String>) {
    let pick = |a: Option<&str>, b: Option<&str>| {
        [a, b]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .map(str::to_string)
    };
    (
        pick(gate_artist, queue_artist),
        pick(gate_title, queue_album),
    )
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::metadata_tag_service::test_support::minimal_m4a;

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

    const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n";
    const LRC: &str = "[00:01.00]Hello\n";
    const VTT: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n\n";

    /// Every file in a folder (top level), name to bytes, sorted.
    fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
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
        out.sort();
        out
    }

    /// The music files in a folder (top level), sorted.
    fn media_in(dir: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| matches!(e, "m4a" | "m4v" | "mp4"))
            })
            .collect();
        v.sort();
        v
    }

    /// Builds the same set of fixtures in a folder: every kind of source,
    /// awkward names, and existing output files.
    fn fixtures(dir: &Path) {
        let track = |name: &str| {
            let p = dir.join(format!("{name}.m4a"));
            minimal_m4a(&p, 1, 1, false);
            p
        };
        // TTML only (word timed), TTML only (line), SRT only, LRC only,
        // VTT only, a Japanese + emoji name, existing outputs.
        track("01 Word");
        std::fs::write(dir.join("01 Word.ttml"), WORD_TTML).unwrap();
        track("02 Line");
        std::fs::write(dir.join("02 Line.ttml"), LINE_TTML).unwrap();
        track("03 Srt only");
        std::fs::write(dir.join("03 Srt only.srt"), SRT).unwrap();
        track("04 Lrc only");
        std::fs::write(dir.join("04 Lrc only.lrc"), LRC).unwrap();
        track("05 Vtt only");
        std::fs::write(dir.join("05 Vtt only.vtt"), VTT).unwrap();
        track("06 日本語 🎵 [Lossless]");
        std::fs::write(dir.join("06 日本語 🎵 [Lossless].ttml"), WORD_TTML).unwrap();
        track("07 Has outputs");
        std::fs::write(dir.join("07 Has outputs.ttml"), WORD_TTML).unwrap();
        std::fs::write(dir.join("07 Has outputs.vtt"), "mine vtt").unwrap();
        std::fs::write(dir.join("07 Has outputs.ass"), "mine ass").unwrap();
        std::fs::write(dir.join("07 Has outputs.lyrics"), "mine lyrics").unwrap();
        std::fs::write(dir.join("07 Has outputs.srt"), "plain srt").unwrap();
        track("08 No lyrics");
    }

    /// Two identical folders, one run folder-wide and one per file.
    fn parity(whole_folder: impl Fn(&str), per_file: impl Fn(&[PathBuf]), prepare: impl Fn(&Path)) {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fixtures(a.path());
        fixtures(b.path());
        prepare(a.path());
        prepare(b.path());
        whole_folder(&a.path().to_string_lossy());
        per_file(&media_in(b.path()));
        let (sa, sb) = (snapshot(a.path()), snapshot(b.path()));
        assert_eq!(
            sa.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            sb.iter().map(|(n, _)| n).collect::<Vec<_>>(),
            "the same files must exist"
        );
        for ((name, a_bytes), (_, b_bytes)) in sa.iter().zip(sb.iter()) {
            assert_eq!(a_bytes, b_bytes, "{name} differs between the two routes");
        }
    }

    // ---- 1. parity with the folder-wide functions ----

    #[test]
    fn enhanced_lrc_matches_the_folder_wide_function() {
        for keep in [true, false] {
            parity(
                |d| {
                    crate::services::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
                        d, keep,
                    )
                    .unwrap();
                },
                |f| {
                    enhanced_lrc_for_files(f, keep).unwrap();
                },
                |_| {},
            );
        }
    }

    #[test]
    fn webvtt_matches_the_folder_wide_function() {
        parity(
            |d| {
                crate::services::webvtt_service::generate_webvtt_for_directory(d).unwrap();
            },
            |f| {
                webvtt_for_files(f).unwrap();
            },
            |_| {},
        );
    }

    #[test]
    fn rich_srt_matches_the_folder_wide_function() {
        parity(
            |d| {
                crate::services::rich_srt_service::generate_rich_srt_for_directory(d).unwrap();
            },
            |f| {
                rich_srt_for_files(f).unwrap();
            },
            |_| {},
        );
    }

    #[test]
    fn subtitle_embedding_matches_the_folder_wide_function() {
        // Give the files `.srt` and `.vtt` text first, so there is
        // something to embed on both routes.
        let prepare = |dir: &Path| {
            std::fs::write(dir.join("01 Word.srt"), SRT).unwrap();
            std::fs::write(dir.join("01 Word.vtt"), VTT).unwrap();
        };
        parity(
            |d| {
                crate::services::rich_srt_service::embed_subtitles_for_directory(d).unwrap();
            },
            |f| {
                embed_subtitles_for_files(f).unwrap();
            },
            prepare,
        );
    }

    #[test]
    fn ass_matches_the_folder_wide_function() {
        parity(
            |d| {
                crate::services::ass_subtitle_service::generate_ass_for_directory(d).unwrap();
            },
            |f| {
                ass_for_files(f).unwrap();
            },
            // A `.vtt` source for the Rich/ASS fallback.
            |_| {},
        );
    }

    #[test]
    fn lyricsfile_matches_the_folder_wide_function() {
        parity(
            |d| {
                crate::services::lyricsfile_service::generate_lyricsfile_for_directory(
                    d,
                    "Album Title",
                    "Album Artist",
                )
                .unwrap();
            },
            |f| {
                lyricsfile_for_files(f, "Album Title", "Album Artist").unwrap();
            },
            |_| {},
        );
    }

    #[test]
    fn the_whole_chain_in_order_matches_the_folder_wide_chain() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fixtures(a.path());
        fixtures(b.path());
        let d = a.path().to_string_lossy().to_string();
        crate::services::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(&d, true)
            .unwrap();
        crate::services::webvtt_service::generate_webvtt_for_directory(&d).unwrap();
        crate::services::rich_srt_service::generate_rich_srt_for_directory(&d).unwrap();
        crate::services::rich_srt_service::embed_subtitles_for_directory(&d).unwrap();
        crate::services::ass_subtitle_service::generate_ass_for_directory(&d).unwrap();
        crate::services::lyricsfile_service::generate_lyricsfile_for_directory(&d, "T", "A")
            .unwrap();
        let files = media_in(b.path());
        enhanced_lrc_for_files(&files, true).unwrap();
        webvtt_for_files(&files).unwrap();
        rich_srt_for_files(&files).unwrap();
        embed_subtitles_for_files(&files).unwrap();
        ass_for_files(&files).unwrap();
        lyricsfile_for_files(&files, "T", "A").unwrap();
        assert_eq!(snapshot(a.path()), snapshot(b.path()));
    }

    // ---- 2. a ttml with no music file, and the failed-write embed ----

    #[test]
    fn a_ttml_with_no_music_file_is_not_converted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lonely.ttml"), WORD_TTML).unwrap();
        // The folder-wide function converts it (the known difference)...
        crate::services::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            &dir.path().to_string_lossy(),
            true,
        )
        .unwrap();
        assert!(dir.path().join("lonely.lrc").exists());
        std::fs::remove_file(dir.path().join("lonely.lrc")).unwrap();
        // ...the per-file route has no music file to belong to.
        let files = vec![dir.path().join("lonely.m4a")];
        assert_eq!(enhanced_lrc_for_files(&files, true), Ok(0));
        assert!(!dir.path().join("lonely.lrc").exists());
    }

    // ---- 3. older files are never touched ----

    /// An older song with all its files, and a new song with only a `.ttml`.
    fn old_and_new(dir: &Path) -> (PathBuf, PathBuf) {
        let old = dir.join("01 Old.m4a");
        minimal_m4a(&old, 1, 1, false);
        std::fs::write(dir.join("01 Old.ttml"), WORD_TTML).unwrap();
        std::fs::write(dir.join("01 Old.lrc"), "old lrc").unwrap();
        std::fs::write(dir.join("01 Old.vtt"), "old vtt").unwrap();
        std::fs::write(dir.join("01 Old.srt"), "old srt").unwrap();
        let new = dir.join("02 New.m4a");
        minimal_m4a(&new, 2, 1, false);
        std::fs::write(dir.join("02 New.ttml"), WORD_TTML).unwrap();
        (old, new)
    }

    #[test]
    fn every_wrapper_leaves_older_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = old_and_new(dir.path());
        let old_before = snapshot(dir.path())
            .into_iter()
            .filter(|(n, _)| n.starts_with("01 Old"))
            .collect::<Vec<_>>();
        let targets = vec![new.clone()];
        enhanced_lrc_for_files(&targets, true).unwrap();
        webvtt_for_files(&targets).unwrap();
        rich_srt_for_files(&targets).unwrap();
        embed_subtitles_for_files(&targets).unwrap();
        ass_for_files(&targets).unwrap();
        lyricsfile_for_files(&targets, "T", "A").unwrap();
        let old_after = snapshot(dir.path())
            .into_iter()
            .filter(|(n, _)| n.starts_with("01 Old"))
            .collect::<Vec<_>>();
        assert_eq!(old_before, old_after, "an older song's files changed");
        for ext in ["ass", "lyrics"] {
            assert!(
                !dir.path().join(format!("01 Old.{ext}")).exists(),
                "01 Old.{ext} was created"
            );
        }
        assert!(old.is_file());
        // The new song did get its files.
        for ext in ["lrc", "vtt", "srt", "ass", "lyrics"] {
            assert!(
                dir.path().join(format!("02 New.{ext}")).exists(),
                "02 New.{ext} was not made"
            );
        }
    }

    #[test]
    fn an_older_song_is_not_given_embedded_lyrics() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = old_and_new(dir.path());
        let old_bytes = std::fs::read(&old).unwrap();
        enhanced_lrc_for_files(std::slice::from_ref(&new), true).unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), old_bytes);
        assert_ne!(
            std::fs::read(&new).unwrap(),
            {
                let other = dir.path().join("blank.m4a");
                minimal_m4a(&other, 2, 1, false);
                std::fs::read(&other).unwrap()
            },
            "the new song should carry embedded lyrics"
        );
    }

    // ---- 4. disc sub-folders ----

    #[test]
    fn disc_subfolder_files_get_their_lyrics_as_well_as_top_level_ones() {
        let dir = tempfile::tempdir().unwrap();
        let disc = dir.path().join("Disc 1");
        std::fs::create_dir(&disc).unwrap();
        let top = dir.path().join("00 Top.m4a");
        let song = disc.join("01 A.m4a");
        for (file, stem_dir) in [(&top, dir.path()), (&song, disc.as_path())] {
            minimal_m4a(file, 1, 1, false);
            let stem = file.file_stem().unwrap().to_string_lossy().to_string();
            std::fs::write(stem_dir.join(format!("{stem}.ttml")), WORD_TTML).unwrap();
        }
        // The list comes from the same walk the download uses for its own
        // files (this folder and its disc folders).
        let found =
            crate::services::metadata_tag_service::collect_m4a_files(&dir.path().to_string_lossy());
        let targets = main_lyrics_targets(&found, &[], &[]);
        assert_eq!(targets.len(), 2);
        assert_eq!(enhanced_lrc_for_files(&targets, true), Ok(2));
        assert!(dir.path().join("00 Top.lrc").exists());
        assert!(disc.join("01 A.lrc").exists());
        // The folder-wide function never looked inside the disc folder.
        let other = tempfile::tempdir().unwrap();
        let d2 = other.path().join("Disc 1");
        std::fs::create_dir(&d2).unwrap();
        minimal_m4a(&d2.join("01 A.m4a"), 1, 1, false);
        std::fs::write(d2.join("01 A.ttml"), WORD_TTML).unwrap();
        crate::services::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            &other.path().to_string_lossy(),
            true,
        )
        .unwrap();
        assert!(!d2.join("01 A.lrc").exists());
    }

    // ---- 5. the list of files ----

    #[test]
    fn the_target_list_is_a_sorted_union_without_duplicates() {
        let p = |s: &str| PathBuf::from(s);
        let list = main_lyrics_targets(
            &[p("/m/b.m4a"), p("/m/a.m4a")],
            &[p("/m/v.m4v")],
            &[p("/m/a.m4a"), p("/m/c.m4a")],
        );
        assert_eq!(
            list,
            vec![p("/m/a.m4a"), p("/m/b.m4a"), p("/m/c.m4a"), p("/m/v.m4v")]
        );
    }

    #[test]
    fn the_provisional_list_holds_only_fresh_files_and_videos() {
        let p = |s: &str| PathBuf::from(s);
        let fresh: std::collections::HashSet<PathBuf> = [p("/m/new.m4a")].into_iter().collect();
        let list = provisional_main_targets(
            &[p("/m/new.m4a"), p("/m/old.m4a")],
            &fresh,
            &[p("/m/v.m4v")],
        );
        assert_eq!(list, vec![p("/m/new.m4a"), p("/m/v.m4v")]);
        // A music-video-only download has a list before Step 2 too.
        let only_video =
            provisional_main_targets(&[], &std::collections::HashSet::new(), &[p("/m/v.m4v")]);
        assert_eq!(only_video, vec![p("/m/v.m4v")]);
    }

    #[test]
    fn a_music_video_only_download_still_gets_its_lyrics() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("Artist - Video.m4v");
        minimal_m4a(&video, 1, 1, false);
        std::fs::write(dir.path().join("Artist - Video.ttml"), WORD_TTML).unwrap();
        // An older video in the same shared folder.
        let older = dir.path().join("Older Video.m4v");
        minimal_m4a(&older, 1, 1, false);
        std::fs::write(dir.path().join("Older Video.ttml"), WORD_TTML).unwrap();
        // No audio files at all: the list holds only the fresh video.
        let targets = main_lyrics_targets(&[], std::slice::from_ref(&video), &[]);
        assert_eq!(targets, vec![video.clone()]);
        assert_eq!(enhanced_lrc_for_files(&targets, true), Ok(1));
        assert!(dir.path().join("Artist - Video.lrc").exists());
        assert!(!dir.path().join("Older Video.lrc").exists());
    }

    #[test]
    fn coverage_counts_only_the_listed_files() {
        let dir = tempfile::tempdir().unwrap();
        let (_, new) = old_and_new(dir.path());
        // The older song has lyrics, the new one has a `.ttml` too.
        assert_eq!(lyrics_coverage(std::slice::from_ref(&new)), (1, 1, 0));
        // An older file without lyrics in the folder does not lower it.
        let bare = dir.path().join("09 Bare.m4a");
        minimal_m4a(&bare, 9, 1, false);
        assert_eq!(lyrics_coverage(std::slice::from_ref(&new)), (1, 1, 0));
        assert_eq!(lyrics_coverage(&[new, bare]), (1, 2, 0));
        // Videos are counted as video; a missing file is ignored.
        let video = dir.path().join("v.m4v");
        minimal_m4a(&video, 1, 1, false);
        assert_eq!(
            lyrics_coverage(&[video, dir.path().join("gone.m4a")]),
            (0, 0, 1)
        );
    }

    #[test]
    fn a_half_written_subtitle_file_never_counts_as_lyrics() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("01 Song.m4a");
        minimal_m4a(&song, 1, 1, false);
        std::fs::write(
            dir.path()
                .join(".meedyadl-partial-123-0a1b2c3d4e5f6789.srt"),
            b"half written",
        )
        .unwrap();
        assert_eq!(lyrics_coverage(&[song]), (0, 1, 0));
    }

    // ---- 6. replace_existing is honoured (for later use) ----

    #[test]
    fn existing_files_are_replaced_only_when_the_caller_allows_it() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("01 A.m4a");
        minimal_m4a(&song, 1, 1, false);
        std::fs::write(dir.path().join("01 A.vtt"), "mine").unwrap();
        std::fs::write(dir.path().join("01 A.ass"), "mine").unwrap();
        std::fs::write(dir.path().join("01 A.lyrics"), "mine").unwrap();
        let texts = LyricTexts {
            ttml: Some(WORD_TTML.to_string()),
            ..LyricTexts::default()
        };
        let run = |replace: &dyn Fn(&Path) -> bool| {
            let i = StepInput {
                audio: &song,
                stem: "01 A",
                source: &texts,
                from_other: false,
                keep: true,
                replace_existing: replace,
            };
            [
                step_webvtt(&i).wrote,
                step_ass(&i).wrote,
                step_lyricsfile(&i, "T", "A").wrote,
            ]
        };
        assert_eq!(run(&|_| false), [false, false, false]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("01 A.vtt")).unwrap(),
            "mine"
        );
        assert_eq!(run(&|_| true), [true, true, true]);
        assert_ne!(
            std::fs::read_to_string(dir.path().join("01 A.vtt")).unwrap(),
            "mine"
        );
    }

    // ---- 8. the catch-up waits for abandoned writing jobs (#1441) ----

    fn pending_with(files: Vec<PathBuf>) -> Arc<StdMutex<Option<Vec<PathBuf>>>> {
        Arc::new(StdMutex::new(Some(files)))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn catch_up_waits_for_a_writing_job_the_abandoned_task_left_running() {
        let in_flight = new_in_flight_writes();
        let pending = pending_with(vec![PathBuf::from("a.m4a")]);
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        // The task that started the job is abandoned straight away; the job
        // keeps running on its blocking thread.
        let tracker = in_flight.clone();
        let task = tokio::spawn(async move {
            run_blocking_held(&tracker, move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
            .await
        });
        started_rx.recv().unwrap();
        task.abort();
        let _ = task.await;
        // While the job still runs, the catch-up must not go ahead.
        let early = wait_to_catch_up(Some(&pending), &in_flight, Duration::from_millis(100)).await;
        assert!(matches!(early, CatchUpWait::TimedOut));
        // Once it ends, the catch-up may.
        release_tx.send(()).unwrap();
        let late = wait_to_catch_up(Some(&pending), &in_flight, Duration::from_secs(10)).await;
        assert!(matches!(late, CatchUpWait::Ready(_)));
    }

    #[tokio::test]
    async fn catch_up_has_nothing_to_wait_for_without_a_pending_list() {
        let in_flight = new_in_flight_writes();
        let _busy = in_flight.clone().read_owned().await;
        let none = Arc::new(StdMutex::new(None));
        assert!(matches!(
            wait_to_catch_up(Some(&none), &in_flight, Duration::from_millis(10)).await,
            CatchUpWait::NothingPending
        ));
        assert!(matches!(
            wait_to_catch_up(None, &in_flight, Duration::from_millis(10)).await,
            CatchUpWait::NothingPending
        ));
    }

    #[test]
    fn catch_up_names_prefer_enrichments_then_the_queue_item_never_a_placeholder() {
        let n = |a, b, c, d| lyricsfile_names_for_catch_up(a, b, c, d);
        assert_eq!(
            n(Some("A"), Some("T"), Some("QA"), Some("QB")),
            (Some("A".into()), Some("T".into()))
        );
        assert_eq!(
            n(None, None, Some("QA"), Some("QB")),
            (Some("QA".into()), Some("QB".into()))
        );
        assert_eq!(
            n(Some("  "), None, Some("QA"), None),
            (Some("QA".into()), None)
        );
        assert_eq!(n(None, None, None, None), (None, None));
    }

    // ---- 7. nothing here deletes ----

    #[test]
    fn this_module_never_deletes_files() {
        let src = include_str!("lyrics_steps.rs");
        // Assembled so this test's own text does not trip the check.
        for call in [["remove", "_file"].concat(), ["remove", "_dir"].concat()] {
            let outside_tests = src.split("#[cfg(test)]").next().unwrap();
            assert!(
                !outside_tests.contains(&call),
                "lyrics_steps.rs calls {call}"
            );
        }
    }
}
