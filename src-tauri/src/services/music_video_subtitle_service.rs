// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Music video subtitle / caption extraction service (#483).
// =========================================================
//
// After GAMDL downloads a music video, this service probes the output
// file for subtitle / closed-caption streams and extracts them into
// sidecar files (`.vtt` / `.srt`) next to the video. Apple Music HLS
// playlists often expose CC tracks (`mov_text`, `tx3g`, `webvtt`,
// `eia_608`) that GAMDL currently remuxes into the output container
// without extracting — media players that don't auto-detect embedded
// subtitle streams benefit from the sidecar copy.
//
// ## Flow
// 1. Run ffprobe to enumerate subtitle streams in the downloaded video,
//    with each one's language and its disposition flags.
// 2. Plan every stream's sidecar name at once, by the shared language
//    policy's rule for sidecar files (TEXT-030, #1251):
//      `{video stem}.{language tag}[.{role}…][.{n}].{ext}`
//    e.g. `Title.en.srt`, `Title.fr-CA.sdh.vtt`, `Title.en.2.srt`. The
//    planning is in `utils::language::plan_subtitle_sidecar_names`.
// 3. For each stream, invoke ffmpeg to copy/convert it into that sidecar
//    next to the video — unless the sidecar is already there, under its
//    new name or the name an older MeedyaDL gave it. ffmpeg writes to a
//    short temporary file with a random part
//    (`.meedyadl-partial-<pid>-<random>.srt`), which is put in place only
//    if the real name is still free, never replacing anything (see
//    `publish_with`), and is removed only while it is still the file this
//    run made (`remove_temporary`). Temporary files that another run, or
//    an earlier forcibly stopped one, left in the folder are never removed:
//    they are counted and reported (`report_leftover_temporaries`).
// 4. Best-effort: failures are logged but never affect the success of
//    the music video download itself.
//
// ## File names, and what changed in #1251
// Before #1251 the names were `{stem}.cc.{stream index}[.{lang}].{ext}`:
// `lang` was ffprobe's raw three-letter code (`eng`), unconverted; every
// stream was called `cc` whatever it was; and the streams' own flags
// (forced, for the deaf and hard of hearing, commentary) were ignored.
// Now the language is read with the policy's reader (`eng` → `en`,
// missing or unreadable → `und`), the roles come from the flags, and a
// number appears only when two streams would otherwise share a name.
// Players such as VLC, Plex, Jellyfin and Infuse already look for the
// `{stem}.{language}.srt` shape beside a video.
//
// Files an older version already wrote keep their old names — they are
// never renamed (policy COMPAT-020). A re-run recognises either name as
// "already extracted", and the lyrics pairing step recognises both as the
// video's own subtitles rather than song lyrics.
//
// Song lyric sidecars (`{stem}.srt`, `{stem}.ttml`…) never collide with
// these: a language-named sidecar always has a language part between the
// stem and the extension (`und` when unknown), which the old `.cc.`
// marker used to guarantee.
//
// ## Dependencies
// - `ffprobe` (shipped with FFmpeg in the managed tools dir)
// - `ffmpeg`  (shipped likewise)

use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::utils::language::{plan_subtitle_sidecar_names, SubtitleStreamFacts};

/// Metadata about a single subtitle / caption stream inside a video file.
#[derive(Debug)]
struct SubtitleStream {
    /// Stream index within the container (0-based, as ffmpeg sees it).
    index: u32,
    /// Codec name (e.g. `mov_text`, `webvtt`, `tx3g`, `eia_608`).
    codec: String,
    /// What the file name is planned from: ffprobe's language exactly as
    /// reported (`eng`, or `None` when the stream has none), the sidecar's
    /// extension, and the disposition flags.
    facts: SubtitleStreamFacts,
}

/// What happened to one stream.
#[derive(Debug, PartialEq, Eq)]
enum ExtractOutcome {
    /// A new sidecar was written here.
    Written(PathBuf),
    /// A sidecar for this stream was already there (from an earlier run,
    /// under its new name or its pre-#1251 name), so nothing was done.
    AlreadyThere(PathBuf),
}

/// The sidecar extension for a subtitle codec: WebVTT is copied as it is
/// (`.vtt`); everything else (`mov_text`, `tx3g`, `eia_608`, `subrip`) is
/// converted to SubRip (`.srt`) by ffmpeg.
fn sidecar_extension(codec: &str) -> &'static str {
    if codec.eq_ignore_ascii_case("webvtt") {
        "vtt"
    } else {
        "srt"
    }
}

/// What one call to [`extract_subtitles_to_sidecars`] did.
#[derive(Debug, Default)]
pub struct SubtitleExtraction {
    /// How many sidecars were newly written (zero if the video has no
    /// subtitle streams, or every stream was already extracted).
    pub written: usize,
    /// Plain sentences the person should see in the activity log: a
    /// subtitle that could not be saved because the drive refused every way
    /// of adding it without the risk of replacing a file (saying what they
    /// can do), a temporary file that could not be removed, or unfinished
    /// temporary subtitle files found in the folder and left alone (saying
    /// when they are safe to delete). Each is also in the log file.
    pub notices: Vec<String>,
}

/// Probe the given video file and extract every subtitle/caption stream
/// to a sidecar file alongside it.
///
/// Returns how many sidecars were newly written, and anything the person
/// should be told (see [`SubtitleExtraction`]). Errors are reported only
/// when probing fundamentally fails — per-stream failures are logged and
/// skipped.
pub async fn extract_subtitles_to_sidecars(
    ffprobe_path: &Path,
    ffmpeg_path: &Path,
    video_path: &Path,
) -> Result<SubtitleExtraction, String> {
    if !video_path.is_file() {
        return Err(format!("Video file not found: {}", video_path.display()));
    }
    let mut report = SubtitleExtraction::default();

    let streams = probe_subtitle_streams(ffprobe_path, video_path).await?;
    if streams.is_empty() {
        log::debug!("No subtitle streams in {}", video_path.display());
        return Ok(report);
    }

    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Video has no filename stem".to_string())?;

    // Every name is planned at once: whether a stream needs a number
    // depends on the others, and the order that decides it is the
    // policy's track order, not the order the loop below runs in.
    let facts: Vec<SubtitleStreamFacts> = streams.iter().map(|s| s.facts.clone()).collect();
    let planned = plan_subtitle_sidecar_names(stem, &facts)?;

    // Before extracting anything here: report temporary files already in
    // this folder, which this run did not make and so leaves alone (see the
    // function). AFTER the names are planned, because a planning failure
    // returns early above and would throw away this report, with the
    // notice in it (stand-in review of round 6, finding 9).
    if let Some(folder) = video_path.parent() {
        report_leftover_temporaries(folder, &mut report.notices);
    }

    for (stream, plan) in streams.iter().zip(planned) {
        if let Some(raw) = &plan.unrecognised_language {
            // Report doubt rather than guess (policy COMPAT-040): the file
            // is named `und`, and the original text is kept in the log.
            log::info!(
                "Subtitle stream #{} of {} says its language is {raw:?}, which is not a \
                 recognised language; its sidecar is named with `und` (not known)",
                stream.index,
                video_path.display()
            );
        }
        match extract_single_stream(
            ffmpeg_path,
            video_path,
            stem,
            stream,
            &plan.file_name,
            &mut report.notices,
        )
        .await
        {
            Ok(ExtractOutcome::Written(sidecar_path)) => {
                log::info!(
                    "Extracted subtitle stream #{} ({}, {}) → {}",
                    stream.index,
                    stream.codec,
                    plan.tag,
                    sidecar_path.display()
                );
                report.written += 1;
            }
            Ok(ExtractOutcome::AlreadyThere(sidecar_path)) => {
                log::debug!(
                    "Subtitle stream #{} already extracted as {} — skipped",
                    stream.index,
                    sidecar_path.display()
                );
            }
            Err(e) => {
                log::warn!(
                    "Failed to extract subtitle stream #{} from {}: {e}",
                    stream.index,
                    video_path.display()
                );
            }
        }
    }

    Ok(report)
}

/// Run ffprobe and parse every subtitle stream out of the video file.
async fn probe_subtitle_streams(
    ffprobe_path: &Path,
    video_path: &Path,
) -> Result<Vec<SubtitleStream>, String> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "s",
        ])
        .arg(video_path)
        .output()
        .await
        .map_err(|e| format!("ffprobe spawn failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe exited with {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("ffprobe returned invalid JSON: {e}"))?;

    Ok(parse_subtitle_streams(&json))
}

/// Reads ffprobe's `-show_streams` JSON into [`SubtitleStream`]s, in
/// ffprobe's own stream order (which the name planning uses to break
/// ties, so the same video always gives the same names). Split out from
/// [`probe_subtitle_streams`] so it can be tested without ffprobe.
fn parse_subtitle_streams(json: &serde_json::Value) -> Vec<SubtitleStream> {
    let Some(streams) = json.get("streams").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut out = Vec::with_capacity(streams.len());
    for stream in streams {
        let Some(index) = stream.get("index").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        let codec = stream
            .get("codec_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        // The language exactly as ffprobe reports it — usually a
        // three-letter code (`eng`). It is NOT defaulted to `und` here any
        // more: "none given" and "`und` given" both end up as `und` in the
        // file name, but only the planning code decides that, in one place.
        let raw_language = stream
            .get("tags")
            .and_then(|t| t.get("language"))
            .and_then(|l| l.as_str())
            .map(str::to_string);
        out.push(SubtitleStream {
            index: u32::try_from(index).unwrap_or(0),
            facts: SubtitleStreamFacts {
                raw_language,
                extension: sidecar_extension(&codec).to_string(),
                hearing_impaired: disposition_flag(stream, "hearing_impaired"),
                captions: disposition_flag(stream, "captions"),
                forced: disposition_flag(stream, "forced"),
                comment: disposition_flag(stream, "comment"),
                original: disposition_flag(stream, "original"),
            },
            codec,
        });
    }

    out
}

/// One of ffprobe's disposition flags for a stream (`disposition.forced`
/// and so on). ffprobe writes them as `0` / `1`; `true` / `false` is also
/// accepted in case a future version changes that. Missing means not set.
fn disposition_flag(stream: &serde_json::Value, name: &str) -> bool {
    match stream.get("disposition").and_then(|d| d.get(name)) {
        Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
        Some(serde_json::Value::Bool(b)) => *b,
        _ => false,
    }
}

/// The name MeedyaDL gave this stream's sidecar before #1251:
/// `{stem}.cc.{index}[.{lang}].{ext}`, where `lang` was ffprobe's language
/// exactly as reported and was left out when missing, empty or `und`.
///
/// Used only to RECOGNISE a file an earlier version already extracted, so
/// a re-run does not extract the same stream a second time under its new
/// name. Nothing is ever renamed (policy COMPAT-020).
///
/// `None` — so there is no old name to look for — when ffprobe's language
/// holds anything but ASCII letters, digits and hyphens. That language
/// text comes out of the video file, so it can hold anything, and before
/// this check a value such as `../../x` made this look for a file OUTSIDE
/// the video's folder (independent review of #1251). Only an existence
/// check was ever made there, never a write, but the check is now simply
/// not made: a real language code never contains anything else, so no
/// genuine old file is missed. A missing, empty or `und` language was left
/// out of the old name, and still is.
fn legacy_sidecar_name(stem: &str, stream: &SubtitleStream) -> Option<String> {
    let language = stream.facts.raw_language.as_deref().unwrap_or("und");
    let lang_suffix = if language == "und" || language.is_empty() {
        String::new()
    } else if language
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        format!(".{language}")
    } else {
        return None;
    };
    Some(format!(
        "{stem}.cc.{}{lang_suffix}.{}",
        stream.index, stream.facts.extension
    ))
}

/// The start of every temporary subtitle name: `.meedyadl-partial-`.
/// Defined once in `fs_safe`, beside `is_temporary_subtitle_file`, the
/// one check every place that lists subtitle files uses to skip them.
const TEMP_PREFIX: &str = crate::utils::fs_safe::TEMPORARY_SUBTITLE_PREFIX;

/// Counts the temporary subtitle files already in `folder` before this
/// extraction makes any -- every file whose name starts with
/// `.meedyadl-partial-` (`fs_safe::is_temporary_subtitle_file`, the same
/// check that makes the lyrics steps skip them) -- and, if there are any,
/// adds ONE notice for the activity log saying how many and where, and when
/// they are safe to delete. It never deletes, renames or changes any of
/// them.
///
/// **Why nothing is deleted any more** (Codex's review of rounds 6-7,
/// findings 3 and 4). Earlier versions of this branch removed such a file
/// when its process was not running on this computer and nobody had
/// changed it for an hour, or, whatever its process and age, when it was a
/// second name of a finished subtitle. The hour was added in round 7, to
/// act on the stand-in review of round 6; this reverses that decision, and
/// the clean-up with it. Neither a name nor a clock proves who owns a file,
/// and each rule deleted things it should not have:
///   - on a folder two computers share, the other computer's live
///     extraction looks as though its process has ended, and with the
///     server's clock behind this computer's, a file written a moment ago
///     already looks an hour old;
///   - an extraction suspended for over an hour (a sleeping laptop) looks
///     abandoned although it will carry on;
///   - an unrelated file that happens to have exactly this shape of name
///     was deleted, though nothing showed MeedyaDL had made it;
///   - the "second name" decision was made, and the file deleted a moment
///     later, by name: in between, the run that owned it could remove it
///     and start another extraction under the same name, which was then
///     deleted instead (finding 2).
///
/// A run therefore removes only the temporary file it made itself, on its
/// own paths (see `remove_temporary`). A file left by a forced stop (the
/// app killed, the power lost) stays until the person deletes it; this
/// notice is how they learn it is there. It is counted, and reported,
/// again at every extraction in that folder until it is gone.
///
/// Nothing takes these files for a finished subtitle: the extractor looks
/// only for the exact names it planned, and the lyrics pairing step and the
/// download queue's lyrics count skip them (`is_temporary_subtitle_file`;
/// round 8 follow-up).
fn report_leftover_temporaries(folder: &Path, notices: &mut Vec<String>) {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!(
                "Could not look for unfinished temporary subtitle files in {}: {e}",
                folder.display()
            );
            return;
        }
    };
    let found = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| crate::utils::fs_safe::is_temporary_subtitle_file(&entry.path()))
        .count();
    if found == 0 {
        return;
    }
    let (count, hidden, them, they_are) = if found == 1 {
        (
            "1 unfinished subtitle file".to_string(),
            "a hidden file whose name starts",
            "it",
            "it is",
        )
    } else {
        (
            format!("{found} unfinished subtitle files"),
            "hidden files whose names start",
            "them",
            "they are",
        )
    };
    let notice = format!(
        "Found {count} in {} ({hidden} with \"{TEMP_PREFIX}\"). MeedyaDL left {them} \
         alone, because another download, on this computer or another one sharing the \
         folder, may still be writing {them}. Once no copy of MeedyaDL is saving subtitles \
         in that folder, {they_are} safe to delete.",
        folder.display()
    );
    log::info!("{notice}");
    notices.push(notice);
}

/// A temporary subtitle file this run created, with what proves it is this
/// run's own: the handle that created it, kept open until the file is
/// removed or published. Before removing the file, the run checks that the
/// name still refers to the file this handle is on
/// (`fs_safe::check_name_against_handle`, which reads both at that moment).
///
/// Why the handle, and not a number stored at creation: the file's number
/// is not fixed on every drive -- a Mac's FAT32 and exFAT drives renumber a
/// file once data is written into it, so a number stored at creation would
/// stop matching this run's own file as soon as ffmpeg had written it, and
/// the run would leave its own temporary file behind on every extraction
/// there (an earlier commit on this branch did exactly that, found by the
/// real-drive test). And while the handle is open the file still exists,
/// so its number cannot have been handed to a new file meanwhile.
struct OwnTemporary {
    /// Where the file is: `.meedyadl-partial-<pid>-<random>.<ext>`.
    path: PathBuf,
    /// The handle that created it (see above). Never read or written.
    handle: std::fs::File,
}

/// How many times [`create_temp_sidecar`] tries a fresh random name when
/// one is taken. With 64 random bits a clash is practically impossible, so
/// more than one try means something odd is creating these names.
const TEMP_NAME_TRIES: u32 = 8;

/// Claims a temporary sidecar name, exclusively, next to the real one,
/// keeping the real extension (ffmpeg picks the output format from it):
/// `.meedyadl-partial-<pid>-<random>.{ext}`. A leading dot so it is not an
/// ordinary-looking file; the process id; then 64 bits from the operating
/// system's random source, as 16 hexadecimal digits. `create_new` makes
/// each attempt exclusive: a name that exists is never taken over.
///
/// **A name never repeats** (Codex's review of rounds 6-7, finding 2). The
/// part after the process id used to be a counter starting at 0, so the
/// first temporary file of every extraction by one process had the SAME
/// name: once one run had removed its file, the next extraction took that
/// name again, and anything that had decided to remove the old file then
/// removed the new one. A random part means a name is never handed out
/// twice, and removal is checked against the file this run's handle is on
/// anyway (see [`remove_temporary`]).
///
/// The file is opened for reading as well as writing only because Windows
/// needs read access to report which file it is (`fs_safe::handle_identity`).
///
/// **Short on purpose** (Codex's review of round 5, finding 5): it used to
/// begin with the whole video name (`.{stem}.meedyadl-partial-…`), so a
/// video whose name, and whose subtitle's name, fit the usual 255-character
/// limit could still need a temporary name over it -- 270 characters for a
/// 240-character video name -- and every extraction of it failed. At most
/// 49 characters now, whatever the video is called.
fn create_temp_sidecar(parent: &Path, extension: &str) -> Result<OwnTemporary, String> {
    use rand::RngCore;
    let pid = std::process::id();
    for _ in 0..TEMP_NAME_TRIES {
        let mut random = [0u8; 8];
        rand::rngs::OsRng.try_fill_bytes(&mut random).map_err(|e| {
            format!("could not get a random temporary subtitle file name from the system: {e}")
        })?;
        let random = u64::from_le_bytes(random);
        let candidate = parent.join(format!("{TEMP_PREFIX}{pid}-{random:016x}.{extension}"));
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(handle) => {
                return Ok(OwnTemporary {
                    path: candidate,
                    handle,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "could not create a temporary subtitle file in {}: {e}",
                    parent.display()
                ))
            }
        }
    }
    Err(format!(
        "no free temporary subtitle file name in {} after {TEMP_NAME_TRIES} tries",
        parent.display()
    ))
}

/// Removes this run's own temporary file -- but only after checking that
/// the name still refers to the file this run created (Codex's review of
/// rounds 6-7, finding 2): the file at the name, and the file the creating
/// handle is open on, both read at this moment
/// (`fs_safe::check_name_against_handle`; see [`OwnTemporary`] for why not
/// a number stored at creation).
///
/// - The name is gone: nothing to do (it counts as removed).
/// - Same file: removed. A removal that fails is REPORTED, never ignored
///   (Codex's review of round 5, finding 4): in the log, and as a notice
///   for the activity log naming the file so the person can delete it.
/// - A different file, or one that cannot be checked: LEFT ALONE, and
///   reported. It may be another run's work.
///
/// **What this still cannot do: be one step.** The check and the removal
/// are two operations, and no system offers "delete this name only if it
/// is still this file" as one. A file that replaces this one in the
/// instant between them would still be removed. Holding the handle closes
/// only the other gap, a number being reused.
fn remove_temporary(temp: OwnTemporary, notices: &mut Vec<String>) {
    use crate::utils::fs_safe::{check_name_against_handle, NameCheck};
    let OwnTemporary { path, handle } = temp;
    match check_name_against_handle(&path, &handle) {
        NameCheck::Gone => {}
        NameCheck::SameFile => match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                log::warn!(
                    "Could not remove the temporary subtitle file {}: {e}",
                    path.display()
                );
                notices.push(format!(
                    "Could not remove the temporary file {} ({e}). It is left over from \
                     saving a subtitle and can be deleted.",
                    path.display()
                ));
            }
        },
        NameCheck::OtherFile => {
            log::warn!(
                "Not removing {}: it is no longer the temporary subtitle file this run created",
                path.display()
            );
            notices.push(format!(
                "Left the file {} alone: it has the name of a temporary file MeedyaDL made \
                 while saving a subtitle, but it is now a different file, which MeedyaDL did \
                 not make.",
                path.display()
            ));
        }
        NameCheck::CannotTell(e) => {
            log::warn!(
                "Not removing {}: could not check that it is still the temporary subtitle \
                 file this run created: {e}",
                path.display()
            );
            notices.push(format!(
                "Left the temporary file {} in place: MeedyaDL could not check that it is \
                 still the file it made while saving a subtitle ({e}). It can be deleted once \
                 no copy of MeedyaDL is saving subtitles in that folder.",
                path.display()
            ));
        }
    }
    // Only now: until here the open handle kept the file's number from
    // being given to another file.
    drop(handle);
}

/// Publishes a finished temporary file under its real name WITHOUT ever
/// overwriting a file already there -- on any file system (Codex's review
/// of round 5, finding 3). See [`publish_with`] for how, and
/// [`real_publish_operations`] for the operations it uses.
fn publish_extracted_subtitle(
    temp: OwnTemporary,
    final_path: &Path,
    notices: &mut Vec<String>,
) -> Result<ExtractOutcome, String> {
    publish_with(temp, final_path, notices, &real_publish_operations())
}

/// One file-system operation of the publish step, given the temporary
/// file and the real name, in that order.
type PublishOperation = fn(&Path, &Path) -> std::io::Result<()>;

/// The file-system operations [`publish_with`] tries, in order. Passed in
/// so the tests can force the paths a drive without hard links takes, on
/// any machine. Generic so that a test can also pass an operation that
/// holds on to something (the race test stops both writers at a barrier
/// inside the hard-link step).
struct PublishOperations<L, R, C> {
    /// Step 1: a hard link from the temporary name to the real one.
    hard_link: L,
    /// Step 2: `fs_safe::rename_no_replace`.
    rename_no_replace: R,
    /// Step 3: `fs_safe::copy_to_new_file`.
    copy_to_new_file: C,
}

/// The operations the app really uses, named in this ONE place:
/// [`publish_extracted_subtitle`] uses them, and the tests of the steps
/// after the hard link start from them and replace only the hard link
/// (stand-in review of round 6, finding 2). The tests used to pass their
/// own copy of the second step to `publish_with`. Changing the real one to
/// a plain `std::fs::rename` -- which REPLACES an existing file -- then left
/// every test green, because the only test of the real route ran on a
/// drive where the hard link works and the second step is never reached.
fn real_publish_operations(
) -> PublishOperations<PublishOperation, PublishOperation, PublishOperation> {
    PublishOperations {
        hard_link: |from, to| std::fs::hard_link(from, to),
        rename_no_replace: crate::utils::fs_safe::rename_no_replace,
        copy_to_new_file: crate::utils::fs_safe::copy_to_new_file,
    }
}

/// The publish step, with its file-system operations passed in (see
/// [`PublishOperations`]; the app passes [`real_publish_operations`]).
///
/// 1. A hard link from the temporary name to the real one. It refuses
///    (`AlreadyExists`) when the real name is taken -- an atomic "only if
///    still free": two extractions racing for one name cannot both win.
///    The temporary name is then removed (it is a second name for the same
///    file, so nothing is lost if that fails; it is reported).
/// 2. ONLY when the link failed because the file system cannot make hard
///    links (`fs_safe::is_hard_link_unsupported`; FAT and exFAT drives,
///    many network shares): `fs_safe::rename_no_replace`, which asks the
///    operating system to move the file into place only if the name is
///    free, as one step with no gap. This used to be "check the name is
///    free, then a plain rename" -- a gap in which a racing extraction could
///    create the name, which the rename then replaced. A FAT32 drive on a
///    Mac takes this step (checked on macOS 27).
/// 3. ONLY when the file system cannot do that step either
///    (`fs_safe::is_no_replace_rename_unsupported`): an exFAT drive on a
///    Mac, which refuses it with "not supported" whenever the name is free
///    (stand-in review of round 6, finding 1; checked on macOS 27). The
///    subtitle is copied into a NEW file under the real name
///    (`fs_safe::copy_to_new_file`): created only if no file has that name,
///    filled, flushed, and on any failure deleted again if the name still
///    refers to the file it made (a file put there meanwhile is left alone
///    and the person told), so it too never replaces a file. Its limit,
///    which steps 1 and 2 do not have: a forced
///    stop (the app killed, the power lost) part-way through the copy
///    leaves a PARTLY written subtitle under the real name, and later runs
///    then keep it as though it were finished. Before this step existed,
///    such a drive got no subtitle at all, although version 1.10.8 saved
///    them there.
/// 4. If the copy fails too, NOTHING is published and the person is told
///    why, and what they can do, in the activity log. Never a plain rename.
///
/// Whatever happens, the temporary file is removed at the end (after a
/// successful step 2 it is already gone) -- if it is still the file this
/// run made (see [`remove_temporary`]). Any other failure of the link (no
/// permission, disk full), or of step 2, is a real failure and is reported
/// as one; it does not try the next step.
fn publish_with<L, R, C>(
    temp: OwnTemporary,
    final_path: &Path,
    notices: &mut Vec<String>,
    operations: &PublishOperations<L, R, C>,
) -> Result<ExtractOutcome, String>
where
    L: Fn(&Path, &Path) -> std::io::Result<()>,
    R: Fn(&Path, &Path) -> std::io::Result<()>,
    C: Fn(&Path, &Path) -> std::io::Result<()>,
{
    use crate::utils::fs_safe::{is_hard_link_unsupported, is_no_replace_rename_unsupported};
    let link_error = match (operations.hard_link)(&temp.path, final_path) {
        Ok(()) => {
            remove_temporary(temp, notices);
            return Ok(ExtractOutcome::Written(final_path.to_path_buf()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            remove_temporary(temp, notices);
            return Ok(ExtractOutcome::AlreadyThere(final_path.to_path_buf()));
        }
        Err(e) => e,
    };
    if !is_hard_link_unsupported(&link_error) {
        remove_temporary(temp, notices);
        return Err(format!(
            "could not publish the extracted subtitle to {}: {link_error}",
            final_path.display()
        ));
    }
    match (operations.rename_no_replace)(&temp.path, final_path) {
        // The file now lives under the real name; dropping `temp` only
        // closes this run's handle to it.
        Ok(()) => Ok(ExtractOutcome::Written(final_path.to_path_buf())),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            remove_temporary(temp, notices);
            Ok(ExtractOutcome::AlreadyThere(final_path.to_path_buf()))
        }
        Err(rename_error) if is_no_replace_rename_unsupported(&rename_error) => {
            let copied = (operations.copy_to_new_file)(&temp.path, final_path);
            remove_temporary(temp, notices);
            match copied {
                Ok(()) => Ok(ExtractOutcome::Written(final_path.to_path_buf())),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    Ok(ExtractOutcome::AlreadyThere(final_path.to_path_buf()))
                }
                Err(copy_error) => {
                    log::warn!(
                        "Not publishing {}: the file system can neither make hard links \
                         ({link_error}) nor rename without replacing ({rename_error}), and \
                         copying into a new file failed: {copy_error}",
                        final_path.display()
                    );
                    let name = final_path.file_name().map_or_else(
                        || final_path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    // A file left at the name may not be MeedyaDL's (Codex's
                    // review of rounds 6-7, finding 1), so the person is not
                    // told to delete it blindly.
                    let notice = if crate::utils::fs_safe::left_a_file_in_place(&copy_error) {
                        format!(
                            "Subtitle \"{name}\" was not saved: it could not be written to the \
                             drive ({copy_error}). A file is still at that name, but MeedyaDL \
                             could not be sure it is the partly written subtitle it had started, \
                             so it did not delete it: check what that file is before deleting \
                             it. Check that the drive is still connected, has free space and can \
                             be written to. Then, to get the subtitle, delete the music video and \
                             download it again."
                        )
                    } else {
                        format!(
                            "Subtitle \"{name}\" was not saved: it could not be written to the \
                             drive ({copy_error}). Check that the drive is still connected, has \
                             free space and can be written to. Then, to get the subtitle, delete \
                             the music video (and this subtitle file, if a partly written one is \
                             there) and download the video again."
                        )
                    };
                    notices.push(notice.clone());
                    Err(notice)
                }
            }
        }
        Err(e) => {
            remove_temporary(temp, notices);
            Err(format!(
                "could not publish the extracted subtitle to {}: {e}",
                final_path.display()
            ))
        }
    }
}

/// Extract a single subtitle stream into the sidecar file `file_name`
/// (planned by `plan_subtitle_sidecar_names`), next to the video.
///
/// **Never overwrites, and never extracts twice.** If `file_name` is
/// already there, or the name an older MeedyaDL gave this same stream
/// (`legacy_sidecar_name`), the stream counts as already extracted and
/// nothing is done.
///
/// **A failed attempt leaves no half file under the real name** (Codex's
/// catch-up review of #1244, finding 3). ffmpeg used to write straight to
/// the real name, so a run that failed part way (disk full) left a half
/// file there, and the existence check above then took it for finished
/// work on every retry. Now ffmpeg writes only to an exclusively created
/// temporary file, which is published under the real name only after
/// ffmpeg succeeded -- never overwriting, on any file system (see
/// `publish_with`) -- and removed on any failure. A removal that fails is
/// reported (log and activity log), never ignored. One exception, on a
/// drive that needs `publish_with`'s third step (an exFAT drive on a Mac):
/// a FORCED stop part-way through that step's copy can leave a partly
/// written subtitle under the real name, which later runs keep. This
/// blocking is new on this branch: v1.10.8 wrote a numbered copy instead.
///
/// Before #1251 this used `resolve_non_clobbering_path` and then checked
/// whether the result was the planned path — which can never be true
/// once the planned path exists, because that helper then returns a
/// different, numbered name. So the "already extracted" check never fired
/// and every re-run wrote a second copy (`….1.srt`). The check is now made
/// on the planned names directly.
///
/// The extension (and so the codec arguments) was chosen when the stream
/// was read — see `sidecar_extension`:
/// - `webvtt` → `.vtt` (stream copy — no transcode)
/// - all other text subs (`mov_text`, `tx3g`, `eia_608`, `subrip`) → `.srt`
async fn extract_single_stream(
    ffmpeg_path: &Path,
    video_path: &Path,
    stem: &str,
    stream: &SubtitleStream,
    file_name: &str,
    notices: &mut Vec<String>,
) -> Result<ExtractOutcome, String> {
    let parent = video_path
        .parent()
        .ok_or_else(|| "Video has no parent directory".to_string())?;

    let sidecar_path = parent.join(file_name);
    if sidecar_path.exists() {
        return Ok(ExtractOutcome::AlreadyThere(sidecar_path));
    }
    if let Some(legacy_name) = legacy_sidecar_name(stem, stream) {
        let legacy_path = parent.join(legacy_name);
        if legacy_path.exists() {
            return Ok(ExtractOutcome::AlreadyThere(legacy_path));
        }
    }

    let codec_args: &[&str] = if stream.facts.extension == "vtt" {
        // Copy WebVTT as-is — no transcode.
        &["-c:s", "copy"]
    } else {
        // Everything else lands as SRT (ffmpeg handles the conversion).
        &["-c:s", "srt"]
    };

    let temp = create_temp_sidecar(parent, &stream.facts.extension)?;

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        // `-y` here, not `-n`: the temporary file was just created by us
        // exclusively, so overwriting its empty placeholder is intended.
        // The no-overwrite promise does not weaken: ffmpeg is never given
        // the REAL name any more; that promise now lives in
        // `publish_extracted_subtitle`, which runs only after success.
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg(format!("0:{}", stream.index));
    cmd.args(codec_args);
    // ffmpeg opens this file again and empties it before writing (it does
    // not delete and recreate it), so it stays the same file, the one this
    // run's handle is on; the real-ffmpeg test checks that no temporary
    // file is left behind.
    cmd.arg(&temp.path);

    let output = match cmd.output().await {
        Ok(output) => output,
        Err(e) => {
            remove_temporary(temp, notices);
            return Err(format!("ffmpeg spawn failed: {e}"));
        }
    };

    if !output.status.success() {
        remove_temporary(temp, notices);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg exited with {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    publish_extracted_subtitle(temp, &sidecar_path, notices)
}

/// Copy existing audio lyrics sidecars (TTML, LRC, SRT, VTT, ASS) alongside
/// a freshly downloaded music video, when the video's matching song was
/// already downloaded as part of the primary album.
///
/// **Filename safety**: never overwrites. Three guards:
///   1. Source and target are canonicalised — if they resolve to the same
///      file (same directory + same stem, e.g. GAMDL happened to produce
///      `01 Title.m4a` and `01 Title.mp4` side-by-side where the
///      pre-existing `01 Title.ttml` already serves both), pairing is a
///      no-op.
///   2. If the target already exists (lyrics already paired or a prior
///      run put them there), pairing is a no-op.
///   3. Otherwise the lyric is copied with the video's stem and its
///      original extension so `01 Title.mp4` picks up `01 Title.ttml`.
///
/// The pairing strategy is deliberately permissive: we look for any lyrics
/// file under `album_dir` whose stem is a substring match of the video's
/// stem (or vice versa) and copy it next to the video with the video's
/// stem. This catches the common case where GAMDL names the song and the
/// music video similarly (typically identical title, different extension).
///
/// Silent no-op when no matches are found.
pub fn pair_song_lyrics_with_music_video(album_dir: &Path, video_path: &Path) -> usize {
    let Some(video_stem) = video_path.file_stem().and_then(|s| s.to_str()) else {
        return 0;
    };
    let Some(video_parent) = video_path.parent() else {
        return 0;
    };
    let normalised_video_stem = normalise_stem(video_stem);

    let Ok(entries) = std::fs::read_dir(album_dir) else {
        return 0;
    };

    let lyric_exts = ["ttml", "lrc", "srt", "vtt", "ass"];
    let mut copied = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Skip macOS AppleDouble shadows / Thumbs.db / .DS_Store —
        // some of these can match the lyric extension allowlist
        // (e.g., `._foo.ttml`) and produce parse failures (#577).
        if crate::utils::fs_safe::is_filesystem_sidecar(&path) {
            continue;
        }
        // Skip MeedyaDL's own temporary subtitle files
        // (`.meedyadl-partial-…`): possibly half written, or another run's
        // work in progress, never song lyrics. The loose name match below
        // took one for a video called "1", "a", "Art" or "Partial", whose
        // names it contains (round 8 follow-up, acting on Codex's review of
        // rounds 6-7; round 8 leaves other runs' leftovers in place).
        if crate::utils::fs_safe::is_temporary_subtitle_file(&path) {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !lyric_exts.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
            continue;
        }
        // The video's OWN subtitle files are not song lyrics (#1251):
        // `{stem}.en.srt`, and the pre-#1251 `{stem}.cc.2.en.srt`. Their
        // names contain the video's stem, so the loose name match below
        // used to take one of them and copy it onto `{stem}.srt`, as though
        // the video's caption track were the song's lyrics.
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| crate::utils::language::is_language_sidecar_of(video_stem, name))
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let normalised_lyric_stem = normalise_stem(stem);
        let matches = normalised_video_stem.contains(&normalised_lyric_stem)
            || normalised_lyric_stem.contains(&normalised_video_stem);
        if !matches {
            continue;
        }

        let target = video_parent.join(format!("{video_stem}.{ext}"));
        // Guard 1 & 2: never overwrite, never copy a file onto itself.
        if same_file(&path, &target) {
            continue;
        }
        if target.exists() {
            continue;
        }
        if let Err(e) = std::fs::copy(&path, &target) {
            log::debug!(
                "Failed to pair {} → {}: {e}",
                path.display(),
                target.display()
            );
        } else {
            log::debug!(
                "Paired song lyrics with music video: {} → {}",
                path.display(),
                target.display()
            );
            copied += 1;
        }
    }
    copied
}

// Collision-proofing helper `same_file` lives in `crate::utils::fs_safe`
// so every rename / write path in the app shares one implementation.
// (`resolve_non_clobbering_path` is no longer used here — see
// `extract_single_stream` for why it could not detect a re-run.)
use crate::utils::fs_safe::same_file;

/// Lowercase + strip typical decoration suffixes (codec / advisory) so
/// song and music video stems compare cleanly.
fn normalise_stem(stem: &str) -> String {
    let mut s = stem.to_ascii_lowercase();
    // Strip any `[tag]` bracket suffixes (codec / advisory) that only
    // appear on one side of the pairing.
    while let Some(open) = s.rfind('[') {
        if let Some(close) = s[open..].find(']') {
            let end = open + close + 1;
            if end == s.len() || s[end..].trim().is_empty() {
                s.truncate(open);
                s = s.trim_end().to_string();
                continue;
            }
        }
        break;
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalise_stem_strips_codec_bracket() {
        assert_eq!(normalise_stem("01 Title [Lossless]"), "01 title");
        assert_eq!(normalise_stem("Song [Explicit] [Dolby Atmos]"), "song");
    }

    #[test]
    fn normalise_stem_leaves_plain_title_alone() {
        assert_eq!(normalise_stem("Song Title"), "song title");
    }

    // NOTE: coverage for `same_file` lives alongside the helper itself in
    // `src-tauri/src/utils/fs_safe.rs`. The file-name planning is tested
    // in `src-tauri/src/utils/language.rs`.

    fn ffprobe_json() -> serde_json::Value {
        serde_json::json!({
            "streams": [
                { "index": 2, "codec_name": "mov_text",
                  "tags": { "language": "eng" },
                  "disposition": { "default": 1, "forced": 0, "hearing_impaired": 0 } },
                { "index": 3, "codec_name": "webvtt",
                  "tags": { "language": "fre" },
                  "disposition": { "forced": 1 } },
                { "index": 4, "codec_name": "mov_text",
                  "tags": { "language": "eng" },
                  "disposition": { "hearing_impaired": 1 } },
                { "index": 5, "codec_name": "tx3g",
                  "disposition": { "captions": true, "comment": 1 } },
                { "codec_name": "no index — skipped" }
            ]
        })
    }

    #[test]
    fn ffprobe_streams_are_read_with_language_extension_and_flags() {
        let streams = parse_subtitle_streams(&ffprobe_json());
        assert_eq!(streams.len(), 4);
        assert_eq!(streams[0].index, 2);
        assert_eq!(streams[0].facts.raw_language.as_deref(), Some("eng"));
        assert_eq!(streams[0].facts.extension, "srt");
        assert!(!streams[0].facts.forced && !streams[0].facts.hearing_impaired);
        assert_eq!(streams[1].facts.extension, "vtt");
        assert!(streams[1].facts.forced);
        assert!(streams[2].facts.hearing_impaired);
        assert_eq!(streams[3].facts.raw_language, None);
        assert!(streams[3].facts.captions && streams[3].facts.comment);
    }

    #[test]
    fn a_whole_video_gets_policy_names() {
        let streams = parse_subtitle_streams(&ffprobe_json());
        let facts: Vec<_> = streams.iter().map(|s| s.facts.clone()).collect();
        let names: Vec<String> = plan_subtitle_sidecar_names("01 Title", &facts)
            .unwrap()
            .into_iter()
            .map(|p| p.file_name)
            .collect();
        assert_eq!(
            names,
            [
                "01 Title.en.srt",
                "01 Title.fr.forced.vtt",
                "01 Title.en.sdh.srt",
                "01 Title.und.sdh.commentary.srt",
            ]
        );
    }

    #[test]
    fn the_pre_1251_name_is_reproduced_exactly() {
        let streams = parse_subtitle_streams(&ffprobe_json());
        let name = |i: usize| legacy_sidecar_name("01 Title", &streams[i]);
        assert_eq!(name(0).as_deref(), Some("01 Title.cc.2.eng.srt"));
        assert_eq!(name(1).as_deref(), Some("01 Title.cc.3.fre.vtt"));
        // No language (or `und`) was left out of the old name.
        assert_eq!(name(3).as_deref(), Some("01 Title.cc.5.srt"));
    }

    #[test]
    fn no_old_name_is_looked_for_when_the_files_language_is_not_plain_text() {
        // Independent review of #1251: the language comes out of the video
        // file, so `../../x` must never become part of a path.
        let json = serde_json::json!({ "streams": [
            { "index": 2, "codec_name": "mov_text", "tags": { "language": "../../x" } },
            { "index": 3, "codec_name": "mov_text", "tags": { "language": "en/x" } },
            { "index": 4, "codec_name": "mov_text", "tags": { "language": "en.x" } },
            { "index": 5, "codec_name": "mov_text", "tags": { "language": "fre-ca" } },
        ]});
        let streams = parse_subtitle_streams(&json);
        for stream in &streams[..3] {
            assert_eq!(legacy_sidecar_name("T", stream), None, "{:?}", stream.facts);
        }
        assert_eq!(
            legacy_sidecar_name("T", &streams[3]).as_deref(),
            Some("T.cc.5.fre-ca.srt")
        );
        // The new name is never built from the raw text: an unreadable
        // language is `und` (policy LANG-003), so nothing of `../../x`
        // reaches it either.
        let facts: Vec<_> = streams.iter().map(|s| s.facts.clone()).collect();
        let planned = plan_subtitle_sidecar_names("T", &facts).unwrap();
        assert_eq!(planned[0].file_name, "T.und.srt");
        assert!(planned.iter().all(|p| !p.file_name.contains('/')));
    }

    /// The whole "already extracted?" step with a hostile language, but
    /// without ffmpeg. Before the fix, the language text went straight into
    /// the old name, so `../../../x` built `V.cc.2.../../../x.srt`: the
    /// first `../` sticks to `V.cc.2.`, and the remaining two climb out of
    /// the video's folder to `a/x.srt`. (With `../../x` it lands back in
    /// the video's own folder, which is why this uses one more level. On
    /// macOS and Linux the look-up only resolves if a folder named
    /// `V.cc.2...` exists, so the test creates one; Windows tidies the
    /// `..` parts away without needing it.) A file planted there must not
    /// be taken as "already extracted". ffmpeg is given as a path that does
    /// not exist, so reaching it shows up as an error, not a crash.
    #[tokio::test]
    async fn a_hostile_language_never_makes_the_extractor_look_outside_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let videos = root.path().join("a").join("b");
        fs::create_dir_all(videos.join("V.cc.2...")).unwrap();
        let video = videos.join("V.mp4");
        fs::write(&video, "").unwrap();
        fs::write(root.path().join("a").join("x.srt"), "not ours").unwrap();
        let json = serde_json::json!({ "streams": [
            { "index": 2, "codec_name": "mov_text", "tags": { "language": "../../../x" } },
        ]});
        let stream = parse_subtitle_streams(&json).remove(0);
        let outcome = extract_single_stream(
            Path::new("/nonexistent/ffmpeg"),
            &video,
            "V",
            &stream,
            "V.und.srt",
            &mut Vec::new(),
        )
        .await;
        assert!(
            !matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_))),
            "a file outside the video's folder was taken as already extracted: {outcome:?}"
        );
    }

    #[test]
    fn pairing_never_takes_the_videos_own_subtitle_files_for_lyrics() {
        // #1251: a video with only its own extracted captions beside it —
        // in the new shape and the old one — gets no `{stem}.srt` copy.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("01 Title.mp4"), "").unwrap();
        fs::write(dir.path().join("01 Title.en.srt"), "caption").unwrap();
        fs::write(dir.path().join("01 Title.cc.2.eng.srt"), "old caption").unwrap();
        let count = pair_song_lyrics_with_music_video(dir.path(), &dir.path().join("01 Title.mp4"));
        assert_eq!(count, 0);
        assert!(!dir.path().join("01 Title.srt").exists());
    }

    /// A temporary subtitle file (`.meedyadl-partial-…`, possibly half
    /// written, possibly another run's work in progress) is never taken for
    /// song lyrics (round 8 follow-up, acting on Codex's review of rounds
    /// 6-7). The pairing match is loose -- either name containing the other
    /// -- and a temporary name contains `meedyadl`, `partial` and
    /// hexadecimal digits, so a video called "1", "a", "Art" or "Partial"
    /// had a leftover copied next to it as its `.srt`. Round 8 stopped
    /// deleting other runs' leftovers, which made that likelier. The song's
    /// real lyrics are still paired beside a leftover.
    #[test]
    fn pairing_never_takes_a_temporary_subtitle_file_for_lyrics() {
        let leftover_name = ".meedyadl-partial-123-0a1b2c3d4e5f6789.srt";
        for video in ["1", "a", "Art", "Partial"] {
            let dir = tempfile::tempdir().unwrap();
            let video_path = dir.path().join(format!("{video}.mp4"));
            fs::write(&video_path, "").unwrap();
            fs::write(dir.path().join(leftover_name), "half written").unwrap();
            let count = pair_song_lyrics_with_music_video(dir.path(), &video_path);
            assert_eq!(count, 0, "video {video:?}");
            assert!(
                !dir.path().join(format!("{video}.srt")).exists(),
                "a temporary file was paired with the video {video:?} as its lyrics"
            );
            assert_eq!(
                fs::read_to_string(dir.path().join(leftover_name)).unwrap(),
                "half written"
            );
        }

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("01 Title [Lossless].m4a"), "").unwrap();
        fs::write(dir.path().join("01 Title [Lossless].srt"), "song lyrics").unwrap();
        fs::write(dir.path().join("01 Title.mp4"), "").unwrap();
        fs::write(dir.path().join(leftover_name), "half written").unwrap();
        let count = pair_song_lyrics_with_music_video(dir.path(), &dir.path().join("01 Title.mp4"));
        assert_eq!(count, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("01 Title.srt")).unwrap(),
            "song lyrics"
        );
    }

    #[test]
    fn pairing_still_copies_the_songs_lyrics_beside_its_captions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("01 Title [Lossless].m4a"), "").unwrap();
        fs::write(dir.path().join("01 Title [Lossless].srt"), "song lyrics").unwrap();
        fs::write(dir.path().join("01 Title.mp4"), "").unwrap();
        fs::write(dir.path().join("01 Title.en.srt"), "caption").unwrap();
        let count = pair_song_lyrics_with_music_video(dir.path(), &dir.path().join("01 Title.mp4"));
        assert_eq!(count, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("01 Title.srt")).unwrap(),
            "song lyrics"
        );
    }

    /// End-to-end, against the real tools: builds a short video with three
    /// subtitle streams (English, English SDH, and no language), extracts
    /// them, checks the names, then extracts again and checks nothing new
    /// was written. Also checks a pre-#1251 file stops a stream being
    /// extracted a second time.
    ///
    /// Ignored by default because it needs `ffmpeg` and `ffprobe` on PATH,
    /// which CI machines do not have. Run it with
    /// `cargo test --lib music_video_subtitle -- --ignored`.
    #[tokio::test]
    #[ignore] // Requires ffmpeg and ffprobe on PATH
    async fn extraction_names_and_rerun_against_real_ffmpeg() {
        let dir = tempfile::tempdir().unwrap();
        let srt = dir.path().join("in.srt");
        fs::write(&srt, "1\n00:00:00,000 --> 00:00:01,000\nHello\n\n").unwrap();
        let video = dir.path().join("01 Title.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:d=1",
            ])
            .args(["-i"])
            .arg(&srt)
            .args(["-i"])
            .arg(&srt)
            .args(["-i"])
            .arg(&srt)
            .args(["-map", "0:v", "-map", "1:s", "-map", "2:s", "-map", "3:s"])
            .args(["-c:v", "mpeg4", "-c:s", "mov_text"])
            .args(["-metadata:s:s:0", "language=eng"])
            .args([
                "-metadata:s:s:1",
                "language=eng",
                "-disposition:s:1",
                "hearing_impaired",
            ])
            .args(["-metadata:s:s:2", "language=und"])
            .arg(&video)
            .status()
            .expect("ffmpeg must be on PATH for this ignored test");
        assert!(status.success(), "could not build the test video");

        let ffmpeg = Path::new("ffmpeg");
        let ffprobe = Path::new("ffprobe");
        let report = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap();
        assert_eq!(report.written, 3);
        // Real ffmpeg empties the temporary file and writes into it; it does
        // not delete and recreate it. Otherwise the file would no longer be
        // the one this run created, and would be left behind with a notice
        // (Codex's review of rounds 6-7, finding 2).
        assert!(report.notices.is_empty(), "{:?}", report.notices);
        assert!(
            !folder_listing(dir.path())
                .iter()
                .any(|n| n.starts_with(TEMP_PREFIX)),
            "a temporary file was left: {:?}",
            folder_listing(dir.path())
        );
        for name in ["01 Title.en.srt", "01 Title.en.sdh.srt", "01 Title.und.srt"] {
            // The words, not just a file: given `-n`, ffmpeg 9.0.1 refuses
            // the extractor's empty placeholder and still exits with 0, so
            // an existence check alone passed with an empty subtitle
            // (Codex's review of round 5, finding 6).
            let text = fs::read_to_string(dir.path().join(name))
                .unwrap_or_else(|e| panic!("{name} missing: {e}"));
            assert!(
                text.contains("Hello"),
                "{name} has no subtitle text: {text:?}"
            );
        }

        // A re-run writes nothing new.
        let again = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap()
            .written;
        assert_eq!(again, 0);

        // A file under the pre-#1251 name also counts as already extracted.
        fs::remove_file(dir.path().join("01 Title.en.srt")).unwrap();
        fs::write(dir.path().join("01 Title.cc.1.eng.srt"), "old").unwrap();
        let with_old = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap()
            .written;
        assert_eq!(with_old, 0);
        assert!(!dir.path().join("01 Title.en.srt").exists());
    }

    #[test]
    fn pair_skips_when_target_exists() {
        // A prior pair already placed the lyrics next to the video — must
        // not overwrite.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("01 Title.m4a"), "").unwrap();
        fs::write(dir.path().join("01 Title.ttml"), "original").unwrap();
        fs::write(dir.path().join("01 Title.mp4"), "").unwrap();
        // Pair — same stem, same dir, existing lyric is already "the" lyric
        // for both the audio and the video, so pair is a no-op and nothing
        // gets overwritten.
        let count = pair_song_lyrics_with_music_video(dir.path(), &dir.path().join("01 Title.mp4"));
        assert_eq!(count, 0);
        assert_eq!(
            fs::read_to_string(dir.path().join("01 Title.ttml")).unwrap(),
            "original"
        );
    }

    #[test]
    fn pair_copies_when_stems_differ() {
        // Audio song has codec suffix; music video has clean stem.
        // Pairing should copy the lyric under the video's stem.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("01 Title [Lossless].m4a"), "").unwrap();
        fs::write(dir.path().join("01 Title [Lossless].ttml"), "song lyrics").unwrap();
        fs::write(dir.path().join("01 Title.mp4"), "").unwrap();
        let count = pair_song_lyrics_with_music_video(dir.path(), &dir.path().join("01 Title.mp4"));
        assert_eq!(count, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("01 Title.ttml")).unwrap(),
            "song lyrics"
        );
        // Original song lyric must be untouched.
        assert_eq!(
            fs::read_to_string(dir.path().join("01 Title [Lossless].ttml")).unwrap(),
            "song lyrics"
        );
    }

    // ── A failed extraction must not block the next attempt (Codex's
    // ── catch-up review of #1244, finding 3) ─────────────────────────────

    /// A fake ffmpeg: writes `content` to its LAST argument (the output
    /// file) and exits with `code`. macOS only, like the other fake-tool
    /// tests (running a just-written script can hit "Text file busy" on
    /// Linux).
    ///
    /// It obeys ffmpeg's overwrite flags the way the real one does (Codex's
    /// review of round 5, finding 6): an output that already exists is
    /// overwritten only with `-y`; with `-n`, or with neither (input is
    /// switched off by `-nostdin`, so it cannot ask), it is refused and left
    /// exactly as it was. The real ffmpeg 9.0.1 on the build machine then
    /// prints "File '…' already exists. Exiting." and exits with code 0
    /// (checked on 4 Oct 2026), which is the dangerous case -- the caller
    /// cannot tell it apart from success -- so the fake does the same. The
    /// placeholder the extractor creates exclusively always exists, so
    /// putting `-n` back makes every extraction here publish an EMPTY file,
    /// and the content checks below fail. The earlier fake ignored every
    /// flag, so that change passed every test.
    #[cfg(target_os = "macos")]
    fn fake_ffmpeg(dir: &Path, name: &str, content: &str, code: i32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
                 overwrite=ask\n\
                 for arg; do\n\
                 \x20 case \"$arg\" in -y) overwrite=yes ;; -n) overwrite=no ;; esac\n\
                 \x20 last=$arg\n\
                 done\n\
                 if [ -e \"$last\" ] && [ \"$overwrite\" != yes ]; then\n\
                 \x20 echo \"File '$last' already exists. Exiting.\" >&2\n\
                 \x20 exit 0\n\
                 fi\n\
                 printf '%s' '{content}' > \"$last\"\n\
                 exit {code}\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The fake itself must keep obeying the overwrite flags, or the tests
    /// built on it stop meaning anything (see `fake_ffmpeg`).
    #[cfg(target_os = "macos")]
    #[test]
    fn the_fake_ffmpeg_overwrites_only_when_told_to() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_ffmpeg(dir.path(), "ffmpeg", "new", 0);
        let out = dir.path().join("out.srt");
        let run = |flag: &str| {
            fs::write(&out, "old").unwrap();
            let status = std::process::Command::new(&fake)
                .args(["-nostdin", flag, "-i", "in.mp4"])
                .arg(&out)
                .status()
                .unwrap();
            assert!(status.success());
            fs::read_to_string(&out).unwrap()
        };
        assert_eq!(run("-n"), "old", "-n must refuse an existing output");
        assert_eq!(run("-hide_banner"), "old", "no flag must refuse too");
        assert_eq!(run("-y"), "new", "-y must overwrite");
    }

    /// A fake ffprobe that reports one English `mov_text` subtitle stream
    /// (index 2), whatever it is asked. macOS only, like `fake_ffmpeg`.
    #[cfg(target_os = "macos")]
    fn fake_ffprobe(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("ffprobe");
        fs::write(
            &path,
            "#!/bin/sh\n\
             echo '{\"streams\":[{\"index\":2,\"codec_name\":\"mov_text\",\"tags\":{\"language\":\"eng\"}}]}'\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The id of a process that has certainly ended: a short-lived child
    /// (this test program, asked only for its help text), waited for. The
    /// system does not hand an id out again straight away.
    fn a_finished_process_id() -> u32 {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    /// Sets when the file at `path` was last changed.
    fn set_last_changed(path: &Path, when: std::time::SystemTime) {
        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(when).unwrap();
    }

    /// A time two hours ago: older than the hour after which an earlier
    /// version of this branch removed an untouched leftover.
    fn two_hours_ago() -> std::time::SystemTime {
        std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60)
    }

    // ── Leftover temporary files are never deleted, only reported
    // ── (Codex's review of rounds 6-7, findings 2 to 4) ─────────────────

    /// Every kind of temporary-looking file an earlier version of this
    /// branch deleted, each dated two hours back so the old hour rule would
    /// have applied: another run of this app (this process's own id), a run
    /// whose process has ended (on this computer, or one that is still
    /// working on another computer sharing the folder), a second name of a
    /// finished subtitle, and an unrelated file whose name happens to have
    /// exactly this app's shape. All are left exactly as they were, and one
    /// activity-log notice says how many there are, where, and when they
    /// are safe to delete.
    #[test]
    fn leftover_temporary_files_are_left_alone_and_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        let ended = a_finished_process_id();
        let published = dir.path().join("V.en.srt");
        fs::write(&published, "1\nHello").unwrap();
        let second_name = format!(".meedyadl-partial-{ended}-2.srt");
        fs::hard_link(&published, dir.path().join(&second_name)).unwrap();
        let files = [
            (
                format!(".meedyadl-partial-{}-0.srt", std::process::id()),
                "another run of this app",
            ),
            (
                format!(".meedyadl-partial-{ended}-0.srt"),
                "a process not running here",
            ),
            (
                ".meedyadl-partial-12345-0.srt".to_string(),
                "not MeedyaDL's at all",
            ),
            (
                format!(".meedyadl-partial-{ended}-0a1b2c3d4e5f6789.vtt"),
                "a newer-shaped name",
            ),
        ];
        for (name, text) in &files {
            fs::write(dir.path().join(name), text).unwrap();
            set_last_changed(&dir.path().join(name), two_hours_ago());
        }
        let before = folder_listing(dir.path());

        let mut notices = Vec::new();
        report_leftover_temporaries(dir.path(), &mut notices);

        assert_eq!(folder_listing(dir.path()), before, "a leftover was removed");
        for (name, text) in &files {
            assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), *text);
        }
        assert_eq!(fs::read_to_string(&published).unwrap(), "1\nHello");
        assert_eq!(notices.len(), 1, "{notices:?}");
        let notice = &notices[0];
        assert!(
            notice.contains("Found 5 unfinished subtitle files in "),
            "{notice}"
        );
        assert!(
            notice.contains(&dir.path().display().to_string()),
            "{notice}"
        );
        assert!(notice.contains("\".meedyadl-partial-\""), "{notice}");
        assert!(notice.contains("left them alone"), "{notice}");
        assert!(
            notice.contains("Once no copy of MeedyaDL is saving subtitles in that folder, they are safe to delete."),
            "{notice}"
        );
    }

    #[test]
    fn one_leftover_is_reported_in_the_singular() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".meedyadl-partial-7-0.srt"), "half").unwrap();
        let mut notices = Vec::new();
        report_leftover_temporaries(dir.path(), &mut notices);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains("Found 1 unfinished subtitle file in "),
            "{notices:?}"
        );
        assert!(notices[0].contains("it is safe to delete."), "{notices:?}");
        assert!(dir.path().join(".meedyadl-partial-7-0.srt").exists());
    }

    /// Every file whose name starts with the temporary prefix is counted
    /// (`fs_safe::is_temporary_subtitle_file`, the check the lyrics steps
    /// use to skip them), and nothing else: not a name with the prefix
    /// elsewhere in it, and not a folder. Nothing is reported for a folder
    /// with none.
    #[test]
    fn only_names_starting_with_the_temporary_prefix_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "meedyadl-partial-1-0.srt",
            ".V.meedyadl-partial-1-0.srt",
            "x.meedyadl-partial-1-0.srt",
            "V.en.srt",
        ] {
            fs::write(dir.path().join(name), "keep").unwrap();
        }
        fs::create_dir(dir.path().join(".meedyadl-partial-1-1.srt")).unwrap();
        let mut notices = Vec::new();
        report_leftover_temporaries(dir.path(), &mut notices);
        assert!(notices.is_empty(), "{notices:?}");

        for name in [".meedyadl-partial-1-0.txt", ".meedyadl-partial-abc.srt"] {
            fs::write(dir.path().join(name), "keep").unwrap();
        }
        report_leftover_temporaries(dir.path(), &mut notices);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains("Found 2 unfinished subtitle files"),
            "{notices:?}"
        );
    }

    /// The whole extraction with leftovers in the folder: it still writes
    /// its subtitle (a leftover never counts as an existing subtitle),
    /// leaves every leftover as it was, and reports them once.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn an_extraction_writes_its_subtitle_and_leaves_leftovers_alone() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let video = dir.path().join("V.mp4");
        fs::write(&video, "").unwrap();
        let leftovers = [
            format!(".meedyadl-partial-{}-0.srt", a_finished_process_id()),
            format!(".meedyadl-partial-{}-1.srt", std::process::id()),
        ];
        for name in &leftovers {
            fs::write(dir.path().join(name), "half").unwrap();
            set_last_changed(&dir.path().join(name), two_hours_ago());
        }

        let ffprobe = fake_ffprobe(tools.path());
        let ffmpeg = fake_ffmpeg(tools.path(), "ffmpeg", "1\nHello", 0);
        let report = extract_subtitles_to_sidecars(&ffprobe, &ffmpeg, &video)
            .await
            .unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("V.en.srt")).unwrap(),
            "1\nHello"
        );
        let mut expected: Vec<String> = leftovers.to_vec();
        expected.extend(["V.en.srt".to_string(), "V.mp4".to_string()]);
        expected.sort();
        assert_eq!(folder_listing(dir.path()), expected);
        for name in &leftovers {
            assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), "half");
        }
        assert_eq!(report.notices.len(), 1, "{:?}", report.notices);
        assert!(
            report.notices[0].contains("Found 2 unfinished subtitle files"),
            "{:?}",
            report.notices
        );
    }

    fn only_stream() -> SubtitleStream {
        let json = serde_json::json!({ "streams": [
            { "index": 2, "codec_name": "mov_text", "tags": { "language": "eng" } },
        ]});
        parse_subtitle_streams(&json).remove(0)
    }

    fn folder_listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn a_failed_extraction_leaves_nothing_and_the_next_attempt_writes_the_subtitle() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let video = dir.path().join("V.mp4");
        fs::write(&video, "").unwrap();
        let stream = only_stream();

        // First attempt: ffmpeg writes half a file, then fails.
        let failing = fake_ffmpeg(tools.path(), "ffmpeg-fails", "1\n00:00:00", 1);
        let first =
            extract_single_stream(&failing, &video, "V", &stream, "V.en.srt", &mut Vec::new())
                .await;
        assert!(first.is_err(), "{first:?}");
        assert_eq!(
            folder_listing(dir.path()),
            ["V.mp4"],
            "nothing may be left behind"
        );

        // Second attempt, ffmpeg working: the subtitle is written.
        let working = fake_ffmpeg(tools.path(), "ffmpeg-works", "1\nHello", 0);
        let second =
            extract_single_stream(&working, &video, "V", &stream, "V.en.srt", &mut Vec::new())
                .await;
        assert!(
            matches!(second, Ok(ExtractOutcome::Written(_))),
            "{second:?}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("V.en.srt")).unwrap(),
            "1\nHello"
        );
        assert_eq!(folder_listing(dir.path()), ["V.en.srt", "V.mp4"]);
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn an_existing_subtitle_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let video = dir.path().join("V.mp4");
        fs::write(&video, "").unwrap();
        fs::write(dir.path().join("V.en.srt"), "mine").unwrap();
        let working = fake_ffmpeg(tools.path(), "ffmpeg-works", "theirs", 0);
        let outcome = extract_single_stream(
            &working,
            &video,
            "V",
            &only_stream(),
            "V.en.srt",
            &mut Vec::new(),
        )
        .await;
        assert!(
            matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_))),
            "{outcome:?}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("V.en.srt")).unwrap(),
            "mine"
        );
    }

    /// Codex's review of round 5, finding 5: the temporary name used to
    /// carry the whole video name (`.{stem}.meedyadl-partial-<pid>-<n>.srt`),
    /// so a 240-character video name -- whose subtitle name, 247 characters,
    /// fits the usual 255-character limit -- needed a 270-character
    /// temporary name, and every extraction failed.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn a_long_video_name_still_extracts() {
        let dir = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let stem = "a".repeat(240);
        let video = dir.path().join(format!("{stem}.mp4"));
        fs::write(&video, "").unwrap();
        let working = fake_ffmpeg(tools.path(), "ffmpeg-works", "1\nHello", 0);
        let name = format!("{stem}.en.srt");
        let outcome = extract_single_stream(
            &working,
            &video,
            &stem,
            &only_stream(),
            &name,
            &mut Vec::new(),
        )
        .await;
        assert!(
            matches!(outcome, Ok(ExtractOutcome::Written(_))),
            "{outcome:?}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(&name)).unwrap(),
            "1\nHello"
        );
        assert_eq!(folder_listing(dir.path()), [name, format!("{stem}.mp4")]);
    }

    #[test]
    fn publishing_never_overwrites_a_file_that_appeared_meanwhile() {
        // The name becomes taken between the existence check and the
        // publish step: the other file wins, ours is thrown away.
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let temp_path = temp.path.clone();
        let fin = dir.path().join("V.en.srt");
        fs::write(&fin, "theirs").unwrap();
        let outcome = publish_extracted_subtitle(temp, &fin, &mut Vec::new());
        assert!(
            matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_))),
            "{outcome:?}"
        );
        assert_eq!(fs::read_to_string(&fin).unwrap(), "theirs");
        assert!(!temp_path.exists());
    }

    // ── Publishing never overwrites, on any file system (Codex's review
    // ── of round 5, finding 3). The hard link is FORCED to fail the way a
    // ── FAT or exFAT drive makes it fail, so these run the other path on
    // ── any machine, with the app's REAL later steps
    // ── (`real_publish_operations`; stand-in review of round 6, finding 2).

    /// The error a drive without hard links gives (see
    /// `fs_safe::is_hard_link_unsupported`).
    fn no_hard_links(_: &Path, _: &Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    /// The app's own publish operations, with only the hard link replaced
    /// by one that fails the way a FAT or exFAT drive makes it fail.
    fn real_operations_without_hard_links(
    ) -> PublishOperations<PublishOperation, PublishOperation, PublishOperation> {
        PublishOperations {
            hard_link: no_hard_links,
            ..real_publish_operations()
        }
    }

    /// The error a drive gives when it cannot rename without replacing (see
    /// `fs_safe::is_no_replace_rename_unsupported`): what an exFAT drive on
    /// a Mac answers whenever the name is free.
    fn no_rename_without_replacing(_: &Path, _: &Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    /// The app's own operations as on an exFAT drive on a Mac: no hard
    /// links, no one-step rename without replacing, so the REAL third step
    /// runs (stand-in review of round 6, finding 1).
    fn real_operations_as_on_mac_exfat(
    ) -> PublishOperations<PublishOperation, PublishOperation, PublishOperation> {
        PublishOperations {
            hard_link: no_hard_links,
            rename_no_replace: no_rename_without_replacing,
            ..real_publish_operations()
        }
    }

    /// The app's own second and third steps, reached as on a drive without
    /// hard links, refuse a taken name. These fail if the real operation
    /// becomes a plain rename (stand-in review of round 6, finding 2), or
    /// if the third step stops creating the file only if it is new.
    #[test]
    fn without_hard_links_an_existing_subtitle_is_never_replaced() {
        for (drive, operations) in [
            ("no hard links", real_operations_without_hard_links()),
            ("exFAT on a Mac", real_operations_as_on_mac_exfat()),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let temp = own_temporary(dir.path(), "ours");
            let fin = dir.path().join("V.en.srt");
            fs::write(&fin, "theirs").unwrap();
            let mut notices = Vec::new();
            let outcome = publish_with(temp, &fin, &mut notices, &operations);
            assert!(
                matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_))),
                "{drive}: {outcome:?}"
            );
            assert_eq!(fs::read_to_string(&fin).unwrap(), "theirs", "{drive}");
            assert_eq!(folder_listing(dir.path()), ["V.en.srt"], "{drive}");
            assert!(notices.is_empty(), "{drive}: {notices:?}");
        }
    }

    #[test]
    fn without_hard_links_a_free_name_is_published() {
        for (drive, operations) in [
            ("no hard links", real_operations_without_hard_links()),
            ("exFAT on a Mac", real_operations_as_on_mac_exfat()),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let temp = own_temporary(dir.path(), "ours");
            let fin = dir.path().join("V.en.srt");
            let mut notices = Vec::new();
            let outcome = publish_with(temp, &fin, &mut notices, &operations);
            assert!(
                matches!(outcome, Ok(ExtractOutcome::Written(_))),
                "{drive}: {outcome:?}"
            );
            assert_eq!(fs::read_to_string(&fin).unwrap(), "ours", "{drive}");
            // The temporary file is gone in both cases: moved by the
            // second step, removed after the third step's copy.
            assert_eq!(folder_listing(dir.path()), ["V.en.srt"], "{drive}");
            assert!(notices.is_empty(), "{drive}: {notices:?}");
        }
    }

    /// Two extractions publish to one name at the same moment on a drive
    /// without hard links: exactly one wins, the winner's text is what is
    /// there, the loser's is thrown away, and no temporary file is left.
    /// Both are held at a barrier inside the (failing) link step, so they
    /// reach the later steps together. Repeated, because a race shows only
    /// sometimes; with "check the name is free, then rename" this reported
    /// two winners. Run twice: as on a drive where the second step works,
    /// and as on an exFAT drive on a Mac, where the third step does the
    /// work (stand-in review of round 6, finding 1).
    #[test]
    fn without_hard_links_two_racing_publishes_never_overwrite_each_other() {
        use std::sync::Barrier;
        for (drive, rename_no_replace) in [
            ("no hard links", real_publish_operations().rename_no_replace),
            (
                "exFAT on a Mac",
                no_rename_without_replacing as PublishOperation,
            ),
        ] {
            for round in 0..100 {
                let dir = tempfile::tempdir().unwrap();
                let fin = dir.path().join("V.en.srt");
                let temps: Vec<OwnTemporary> = (0..2)
                    .map(|i| own_temporary(dir.path(), &format!("writer {i}")))
                    .collect();
                let barrier = Barrier::new(2);
                let outcomes: Vec<_> = std::thread::scope(|scope| {
                    let handles: Vec<_> = temps
                        .into_iter()
                        .map(|temp| {
                            let (barrier, fin) = (&barrier, &fin);
                            scope.spawn(move || {
                                let operations = PublishOperations {
                                    hard_link: |a: &Path, b: &Path| {
                                        barrier.wait();
                                        no_hard_links(a, b)
                                    },
                                    rename_no_replace,
                                    copy_to_new_file: real_publish_operations().copy_to_new_file,
                                };
                                publish_with(temp, fin, &mut Vec::new(), &operations)
                            })
                        })
                        .collect();
                    handles.into_iter().map(|h| h.join().unwrap()).collect()
                });
                let winners: Vec<usize> = outcomes
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| matches!(o, Ok(ExtractOutcome::Written(_))))
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(winners.len(), 1, "{drive}, round {round}: {outcomes:?}");
                assert!(
                    outcomes
                        .iter()
                        .any(|o| matches!(o, Ok(ExtractOutcome::AlreadyThere(_)))),
                    "{drive}, round {round}: {outcomes:?}"
                );
                assert_eq!(
                    fs::read_to_string(&fin).unwrap(),
                    format!("writer {}", winners[0]),
                    "{drive}, round {round}"
                );
                assert_eq!(
                    folder_listing(dir.path()),
                    ["V.en.srt"],
                    "{drive}, round {round}"
                );
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn einval_from_the_no_replace_step_counts_as_not_supported() {
        // `EINVAL` (what Linux answers for a file system without
        // `RENAME_NOREPLACE`) leads to the third step, not to a failure.
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let fin = dir.path().join("V.en.srt");
        let mut notices = Vec::new();
        fn einval(_: &Path, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::from_raw_os_error(libc::EINVAL))
        }
        let operations = PublishOperations {
            rename_no_replace: einval as PublishOperation,
            ..real_operations_as_on_mac_exfat()
        };
        let outcome = publish_with(temp, &fin, &mut notices, &operations);
        assert!(
            matches!(outcome, Ok(ExtractOutcome::Written(_))),
            "{outcome:?}"
        );
        assert_eq!(fs::read_to_string(&fin).unwrap(), "ours");
        assert_eq!(folder_listing(dir.path()), ["V.en.srt"]);
        assert!(notices.is_empty(), "{notices:?}");
    }

    #[test]
    fn a_drive_that_refuses_every_step_gets_nothing_and_is_told_what_to_do() {
        // No hard links, no rename without replacing, and the copy into a
        // new file fails too (a full drive, say): nothing published, never
        // a plain rename, and the activity-log message says what to do.
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let fin = dir.path().join("V.en.srt");
        let mut notices = Vec::new();
        let operations = PublishOperations {
            hard_link: no_hard_links,
            rename_no_replace: no_rename_without_replacing,
            copy_to_new_file: |_: &Path, _: &Path| {
                Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
            },
        };
        let outcome = publish_with(temp, &fin, &mut notices, &operations);
        assert!(outcome.is_err(), "{outcome:?}");
        assert!(!fin.exists());
        assert_eq!(folder_listing(dir.path()), Vec::<String>::new());
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains("\"V.en.srt\" was not saved"),
            "{notices:?}"
        );
        assert!(
            notices[0].contains("has free space")
                && notices[0].contains("download the video again"),
            "the message must say what the person can do: {notices:?}"
        );
    }

    /// Codex's review of rounds 6-7, finding 1, through the publish step:
    /// the third step's copy fails, and before it cleans up, another file
    /// is put at the subtitle's name. That file stays, the subtitle is not
    /// saved, and the person is told a file is at that name and to check it
    /// before deleting it -- not told to delete it. The failure is made by
    /// copying from a folder (Unix only, see `fs_safe`'s tests).
    #[cfg(unix)]
    #[test]
    fn a_file_put_at_the_name_while_the_copy_fails_is_left_and_the_person_told() {
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let fin = dir.path().join("V.en.srt");
        let folder = tempfile::tempdir().unwrap();
        let operations = PublishOperations {
            hard_link: no_hard_links,
            rename_no_replace: no_rename_without_replacing,
            copy_to_new_file: |_: &Path, to: &Path| {
                crate::utils::fs_safe::copy_to_new_file_with(folder.path(), to, |to| {
                    fs::rename(to, to.with_file_name("moved away")).unwrap();
                    fs::write(to, "somebody else's").unwrap();
                })
            },
        };
        let mut notices = Vec::new();
        let outcome = publish_with(temp, &fin, &mut notices, &operations);
        assert!(outcome.is_err(), "{outcome:?}");
        assert_eq!(fs::read_to_string(&fin).unwrap(), "somebody else's");
        assert_eq!(notices.len(), 1, "{notices:?}");
        let notice = &notices[0];
        assert!(notice.contains("\"V.en.srt\" was not saved"), "{notice}");
        assert!(notice.contains("A file is still at that name"), "{notice}");
        assert!(
            notice.contains("check what that file is before deleting it"),
            "{notice}"
        );
        assert!(!notice.contains("and this subtitle file"), "{notice}");
    }

    #[test]
    fn any_other_failure_is_a_failure_not_a_fallback() {
        // No permission, say. A link failure that is not "not supported"
        // never tries the second step; a second-step failure that is not
        // "not supported" never tries the third. Reported either way, with
        // the temporary file removed.
        let denied = |_: &Path, _: &Path| -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        };
        for link_works_as_on_exfat in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let temp = own_temporary(dir.path(), "ours");
            let temp_path = temp.path.clone();
            let fin = dir.path().join("V.en.srt");
            let (tried_rename, tried_copy) =
                (std::cell::Cell::new(false), std::cell::Cell::new(false));
            let operations = PublishOperations {
                hard_link: |a: &Path, b: &Path| {
                    if link_works_as_on_exfat {
                        no_hard_links(a, b)
                    } else {
                        denied(a, b)
                    }
                },
                rename_no_replace: |a: &Path, b: &Path| {
                    tried_rename.set(true);
                    denied(a, b)
                },
                copy_to_new_file: |_: &Path, _: &Path| {
                    tried_copy.set(true);
                    Ok(())
                },
            };
            let outcome = publish_with(temp, &fin, &mut Vec::new(), &operations);
            assert!(outcome.is_err(), "{outcome:?}");
            assert_eq!(tried_rename.get(), link_works_as_on_exfat);
            assert!(!tried_copy.get());
            assert!(!fin.exists());
            assert!(!temp_path.exists());
        }
    }

    /// The whole extraction, on a REAL drive of any kind: a folder named by
    /// the `MEEDYADL_SUBTITLE_DRIVE_TEST_DIR` environment variable, for
    /// example a FAT32 or exFAT disk image attached with `hdiutil` (stand-in
    /// review of round 6, finding 1: on a Mac, an exFAT drive refuses both
    /// hard links and the one-step no-replace rename, and the unit tests
    /// above can only imitate that). Uses the fake ffprobe and ffmpeg, so
    /// macOS only, and ignored by default. Run it with
    /// `MEEDYADL_SUBTITLE_DRIVE_TEST_DIR=<folder> cargo test --lib
    /// on_a_real_drive -- --ignored --nocapture`. It works in a new
    /// sub-folder and deletes it afterwards. Without the variable it says
    /// "skipped" and checks nothing, so the command given for the real
    /// ffmpeg test above (which also runs this one) still works.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore] // Needs a folder on the drive to test, named by the variable.
    async fn on_a_real_drive_subtitles_are_saved_and_never_replace_a_file() {
        let Ok(drive) = std::env::var("MEEDYADL_SUBTITLE_DRIVE_TEST_DIR") else {
            eprintln!(
                "skipped: set MEEDYADL_SUBTITLE_DRIVE_TEST_DIR to a folder on the drive to test"
            );
            return;
        };
        let base = PathBuf::from(drive).join(format!("meedyadl-drive-test-{}", std::process::id()));
        let fresh = |name: &str| {
            let dir = base.join(name);
            fs::create_dir_all(&dir).unwrap();
            dir
        };
        // What a drive like this adds by itself (`._V.mp4` on FAT and exFAT
        // on a Mac) is not ours and is left out of every listing.
        let listing = |dir: &Path| -> Vec<String> {
            folder_listing(dir)
                .into_iter()
                .filter(|n| !crate::utils::fs_safe::is_filesystem_sidecar(Path::new(n)))
                .collect()
        };
        let tools = tempfile::tempdir().unwrap();
        let ffprobe = fake_ffprobe(tools.path());
        let ffmpeg = fake_ffmpeg(tools.path(), "ffmpeg", "1\nHello", 0);
        let mut problems: Vec<String> = Vec::new();

        // 1. An ordinary extraction saves the subtitle.
        let dir = fresh("1-plain");
        fs::write(dir.join("V.mp4"), "").unwrap();
        let report = extract_subtitles_to_sidecars(&ffprobe, &ffmpeg, &dir.join("V.mp4")).await;
        eprintln!("1 plain: {report:?} {:?}", listing(&dir));
        if !matches!(&report, Ok(r) if r.written == 1 && r.notices.is_empty())
            || fs::read_to_string(dir.join("V.en.srt")).ok().as_deref() != Some("1\nHello")
            || listing(&dir) != ["V.en.srt", "V.mp4"]
        {
            problems.push(format!("1 plain: {report:?} {:?}", listing(&dir)));
        }

        // 2. A file already at the real name is never replaced, even when
        //    the publish step is reached directly (no early check).
        let dir = fresh("2-taken");
        let taken = dir.join("V.en.srt");
        fs::write(&taken, "mine").unwrap();
        let temp = own_temporary(&dir, "ours");
        let outcome = publish_extracted_subtitle(temp, &taken, &mut Vec::new());
        eprintln!("2 taken: {outcome:?} {:?}", listing(&dir));
        if !matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_)))
            || fs::read_to_string(&taken).unwrap() != "mine"
            || listing(&dir) != ["V.en.srt"]
        {
            problems.push(format!("2 taken: {outcome:?} {:?}", listing(&dir)));
        }

        // 2b. The third step itself, on this drive, refuses a taken name
        //     (on most drives the second step answers first, so this is the
        //     only way to reach it with the name already taken).
        let dir = fresh("2b-taken-copy");
        let taken = dir.join("V.en.srt");
        fs::write(&taken, "mine").unwrap();
        let temp = own_temporary(&dir, "ours");
        let outcome = publish_with(
            temp,
            &taken,
            &mut Vec::new(),
            &real_operations_as_on_mac_exfat(),
        );
        eprintln!("2b taken, third step: {outcome:?} {:?}", listing(&dir));
        if !matches!(outcome, Ok(ExtractOutcome::AlreadyThere(_)))
            || fs::read_to_string(&taken).unwrap() != "mine"
            || listing(&dir) != ["V.en.srt"]
        {
            problems.push(format!("2b taken copy: {outcome:?} {:?}", listing(&dir)));
        }

        // 2c. A copy that fails on this drive deletes the file it made (the
        //     check that the name still refers to that file works here),
        //     and 2d. leaves a file put at the name meanwhile (Codex's
        //     review of rounds 6-7, finding 1). Failed by copying from a
        //     folder, which can be opened but not read.
        let dir = fresh("2c-copy-fails");
        let folder = fresh("2c-not-a-file");
        let result = crate::utils::fs_safe::copy_to_new_file(&folder, &dir.join("V.en.srt"));
        eprintln!("2c failed copy: {result:?} {:?}", listing(&dir));
        if result.is_ok() || !listing(&dir).is_empty() {
            problems.push(format!("2c failed copy: {result:?} {:?}", listing(&dir)));
        }
        let dir = fresh("2d-copy-fails-replaced");
        let to = dir.join("V.en.srt");
        let result = crate::utils::fs_safe::copy_to_new_file_with(&folder, &to, |to| {
            fs::rename(to, to.with_file_name("moved away")).unwrap();
            fs::write(to, "somebody else's").unwrap();
        });
        eprintln!("2d failed copy, replaced: {result:?} {:?}", listing(&dir));
        if !result
            .as_ref()
            .is_err_and(crate::utils::fs_safe::left_a_file_in_place)
            || fs::read_to_string(&to).ok().as_deref() != Some("somebody else's")
        {
            problems.push(format!(
                "2d failed copy, replaced: {result:?} {:?}",
                listing(&dir)
            ));
        }

        // 3. A 240-character video name, and 4. odd characters.
        for (case, stem) in [
            ("3-long", "a".repeat(240)),
            ("4-odd", "V ; $(x) 'q' é [E] #%".to_string()),
        ] {
            let dir = fresh(case);
            let video = dir.join(format!("{stem}.mp4"));
            fs::write(&video, "").unwrap();
            let report = extract_subtitles_to_sidecars(&ffprobe, &ffmpeg, &video).await;
            eprintln!(
                "{case}: {:?}",
                report.as_ref().map(|r| (r.written, &r.notices))
            );
            if !matches!(&report, Ok(r) if r.written == 1 && r.notices.is_empty()) {
                problems.push(format!("{case}: {report:?} {:?}", listing(&dir)));
            }
        }

        // 5. Forty rounds of two extractions publishing to one name at the
        //    same moment, with the real publish step: exactly one winner,
        //    the winner's text in place, and no temporary file left.
        let dir = fresh("5-race");
        let mut outcomes_seen = std::collections::BTreeMap::new();
        for round in 0..40 {
            let fin = dir.join(format!("R{round}.en.srt"));
            let temps: Vec<OwnTemporary> = (0..2)
                .map(|i| own_temporary(&dir, &format!("writer {i}")))
                .collect();
            let temp_paths: Vec<PathBuf> = temps.iter().map(|t| t.path.clone()).collect();
            let barrier = std::sync::Barrier::new(2);
            let (outs, notices): (Vec<_>, Vec<_>) = std::thread::scope(|scope| {
                let handles: Vec<_> = temps
                    .into_iter()
                    .map(|temp| {
                        let (barrier, fin) = (&barrier, &fin);
                        scope.spawn(move || {
                            barrier.wait();
                            let mut notices = Vec::new();
                            let outcome = publish_extracted_subtitle(temp, fin, &mut notices);
                            (outcome, notices)
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).unzip()
            });
            let kinds: Vec<&str> = outs
                .iter()
                .map(|o| match o {
                    Ok(ExtractOutcome::Written(_)) => "written",
                    Ok(ExtractOutcome::AlreadyThere(_)) => "already there",
                    Err(_) => "refused",
                })
                .collect();
            *outcomes_seen.entry(kinds.join(" + ")).or_insert(0) += 1;
            let winners: Vec<usize> = (0..2).filter(|&i| kinds[i] == "written").collect();
            if winners.len() != 1
                || fs::read_to_string(&fin).ok() != Some(format!("writer {}", winners[0]))
                || temp_paths.iter().any(|t| t.exists())
                || notices.iter().any(|n: &Vec<String>| !n.is_empty())
            {
                problems.push(format!("5 race, round {round}: {outs:?} {notices:?}"));
            }
        }
        eprintln!("5 race: {outcomes_seen:?}");
        if listing(&dir).iter().any(|n| n.starts_with(TEMP_PREFIX)) {
            problems.push(format!("5 race: leftovers {:?}", listing(&dir)));
        }

        // Tidy up. On an exFAT drive a Mac can still be writing its `._`
        // side files while the folder is deleted, which then fails with
        // "Directory not empty"; a second try clears it. A folder that
        // still cannot be deleted is reported, not counted as a failure:
        // this test is about the subtitles, not about tidying up.
        let tidied = (0..5).any(|_| fs::remove_dir_all(&base).is_ok() || !base.exists());
        if !tidied {
            eprintln!("could not delete the test folder {}", base.display());
        }
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// A temporary file of this run's own, holding `text`. Created the way
    /// the app creates one (`create_temp_sidecar`), so it carries the open
    /// handle a later removal is checked against; then filled in place, as
    /// ffmpeg fills it.
    fn own_temporary(dir: &Path, text: &str) -> OwnTemporary {
        let temp = create_temp_sidecar(dir, "srt").unwrap();
        fs::write(&temp.path, text).unwrap();
        temp
    }

    /// The part of a temporary name after the process id and before the
    /// extension.
    fn random_part(temp: &OwnTemporary) -> String {
        let name = temp
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let prefix = format!("{TEMP_PREFIX}{}-", std::process::id());
        name.strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".srt"))
            .unwrap_or_else(|| panic!("unexpected temporary name {name}"))
            .to_string()
    }

    #[test]
    fn temporary_names_are_exclusive_short_random_and_keep_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let temps: Vec<OwnTemporary> = (0..20)
            .map(|_| create_temp_sidecar(dir.path(), "srt").unwrap())
            .collect();
        let mut names = std::collections::BTreeSet::new();
        for temp in &temps {
            let name = temp
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            assert!(temp.path.extension().is_some_and(|e| e == "srt"), "{name}");
            assert!(name.len() <= 49, "{name}");
            assert!(
                crate::utils::fs_safe::is_temporary_subtitle_file(&temp.path),
                "{name}"
            );
            let random = random_part(temp);
            assert_eq!(random.len(), 16, "64 random bits as 16 hex digits: {name}");
            assert!(
                random
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "{name}"
            );
            names.insert(name);
        }
        assert_eq!(names.len(), temps.len(), "a name repeated: {names:?}");
    }

    /// Codex's review of rounds 6-7, finding 2: the part after the process
    /// id was a counter starting at 0, so once a run had removed its
    /// temporary file, the next extraction got the very same name. Now a
    /// removed name is not handed out again.
    #[test]
    fn a_removed_temporary_name_is_not_given_out_again() {
        let dir = tempfile::tempdir().unwrap();
        let first = create_temp_sidecar(dir.path(), "srt").unwrap();
        let first_path = first.path.clone();
        let mut notices = Vec::new();
        remove_temporary(first, &mut notices);
        assert!(!first_path.exists());
        assert!(notices.is_empty(), "{notices:?}");
        for _ in 0..20 {
            let next = create_temp_sidecar(dir.path(), "srt").unwrap();
            assert_ne!(
                next.path, first_path,
                "the removed name was given out again"
            );
            remove_temporary(next, &mut notices);
        }
        assert!(notices.is_empty(), "{notices:?}");
    }

    /// The reviewer's sequence (Codex's review of rounds 6-7, finding 2):
    /// a run has decided its temporary file can go -- here, just after the
    /// hard link has published it -- and before the removal happens, its
    /// temporary name is taken by a NEW file (another run's new temporary,
    /// say). The new file must survive: the removal is checked against the
    /// file this run's creating handle is on.
    #[test]
    fn a_new_file_that_took_the_temporary_name_meanwhile_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let fin = dir.path().join("V.en.srt");
        let temp = own_temporary(dir.path(), "ours");
        let temp_path = temp.path.clone();
        let operations = PublishOperations {
            hard_link: |from: &Path, to: &Path| {
                fs::hard_link(from, to)?;
                // In between: the old name is let go of and a new file is
                // created under it.
                fs::remove_file(from).unwrap();
                fs::write(from, "another run's new temporary").unwrap();
                Ok(())
            },
            rename_no_replace: real_publish_operations().rename_no_replace,
            copy_to_new_file: real_publish_operations().copy_to_new_file,
        };
        let mut notices = Vec::new();
        let outcome = publish_with(temp, &fin, &mut notices, &operations);
        assert!(
            matches!(outcome, Ok(ExtractOutcome::Written(_))),
            "{outcome:?}"
        );
        assert_eq!(fs::read_to_string(&fin).unwrap(), "ours");
        assert_eq!(
            fs::read_to_string(&temp_path).ok().as_deref(),
            Some("another run's new temporary"),
            "the new file under the temporary name was deleted"
        );
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].contains("Left the file"), "{notices:?}");
        assert!(notices[0].contains("did not make"), "{notices:?}");
    }

    /// The same check on every path that removes a temporary file: the
    /// file moved away and replaced by another one under the same name
    /// before a failed extraction tidies up.
    #[test]
    fn a_replaced_temporary_file_is_never_removed() {
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let temp_path = temp.path.clone();
        fs::rename(&temp_path, dir.path().join("moved")).unwrap();
        fs::write(&temp_path, "someone else's").unwrap();
        let mut notices = Vec::new();
        remove_temporary(temp, &mut notices);
        assert_eq!(fs::read_to_string(&temp_path).unwrap(), "someone else's");
        assert_eq!(
            fs::read_to_string(dir.path().join("moved")).unwrap(),
            "ours"
        );
        assert_eq!(notices.len(), 1, "{notices:?}");
    }

    /// When the name cannot be checked at all, the file is left in place and
    /// that is reported. Made unreadable by taking away permission to look
    /// inside the folder (Unix; skipped when running as the superuser, who
    /// may look anyway).
    #[cfg(unix)]
    #[test]
    fn a_temporary_file_that_cannot_be_checked_is_left_and_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let temp_path = temp.path.clone();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o600)).unwrap();
        let blocked = fs::symlink_metadata(&temp_path).is_err();
        let mut notices = Vec::new();
        remove_temporary(temp, &mut notices);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            eprintln!("skipped: this user may look inside a folder without permission");
            return;
        }
        assert!(temp_path.exists());
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].contains("could not check"), "{notices:?}");
    }

    /// A removal that fails is reported, never ignored (Codex's review of
    /// round 5, finding 4). Made to fail by taking away permission to
    /// change the folder (Unix; skipped as the superuser).
    #[cfg(unix)]
    #[test]
    fn a_temporary_file_that_cannot_be_removed_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let temp = own_temporary(dir.path(), "ours");
        let temp_path = temp.path.clone();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let blocked = fs::write(dir.path().join("probe"), "").is_err();
        let mut notices = Vec::new();
        remove_temporary(temp, &mut notices);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        if !blocked {
            eprintln!("skipped: this user may change a read-only folder");
            return;
        }
        assert!(temp_path.exists());
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].contains("Could not remove"), "{notices:?}");
    }
}
