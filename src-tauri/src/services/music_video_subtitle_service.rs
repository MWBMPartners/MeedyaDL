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
//    new name or the name an older MeedyaDL gave it.
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

/// Probe the given video file and extract every subtitle/caption stream
/// to a sidecar file alongside it.
///
/// Returns `Ok(count)` where `count` is the number of sidecars newly
/// written (zero if the video has no subtitle streams, or every stream was
/// already extracted by an earlier run). Errors are reported only when
/// probing fundamentally fails — per-stream failures are logged and
/// skipped.
pub async fn extract_subtitles_to_sidecars(
    ffprobe_path: &Path,
    ffmpeg_path: &Path,
    video_path: &Path,
) -> Result<usize, String> {
    if !video_path.is_file() {
        return Err(format!("Video file not found: {}", video_path.display()));
    }

    let streams = probe_subtitle_streams(ffprobe_path, video_path).await?;
    if streams.is_empty() {
        log::debug!("No subtitle streams in {}", video_path.display());
        return Ok(0);
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

    let mut extracted = 0;
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
        match extract_single_stream(ffmpeg_path, video_path, stem, stream, &plan.file_name).await {
            Ok(ExtractOutcome::Written(sidecar_path)) => {
                log::info!(
                    "Extracted subtitle stream #{} ({}, {}) → {}",
                    stream.index,
                    stream.codec,
                    plan.tag,
                    sidecar_path.display()
                );
                extracted += 1;
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

    Ok(extracted)
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
fn legacy_sidecar_name(stem: &str, stream: &SubtitleStream) -> String {
    let language = stream.facts.raw_language.as_deref().unwrap_or("und");
    let lang_suffix = if language == "und" || language.is_empty() {
        String::new()
    } else {
        format!(".{language}")
    };
    format!(
        "{stem}.cc.{}{lang_suffix}.{}",
        stream.index, stream.facts.extension
    )
}

/// Extract a single subtitle stream into the sidecar file `file_name`
/// (planned by `plan_subtitle_sidecar_names`), next to the video.
///
/// **Never overwrites, and never extracts twice.** If `file_name` is
/// already there, or the name an older MeedyaDL gave this same stream
/// (`legacy_sidecar_name`), the stream counts as already extracted and
/// nothing is done. ffmpeg is also run with `-n` (refuse to overwrite).
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
) -> Result<ExtractOutcome, String> {
    let parent = video_path
        .parent()
        .ok_or_else(|| "Video has no parent directory".to_string())?;

    let sidecar_path = parent.join(file_name);
    if sidecar_path.exists() {
        return Ok(ExtractOutcome::AlreadyThere(sidecar_path));
    }
    let legacy_path = parent.join(legacy_sidecar_name(stem, stream));
    if legacy_path.exists() {
        return Ok(ExtractOutcome::AlreadyThere(legacy_path));
    }

    let codec_args: &[&str] = if stream.facts.extension == "vtt" {
        // Copy WebVTT as-is — no transcode.
        &["-c:s", "copy"]
    } else {
        // Everything else lands as SRT (ffmpeg handles the conversion).
        &["-c:s", "srt"]
    };

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        // Deliberately DO NOT pass `-y` — we never want ffmpeg to silently
        // overwrite an existing file. The existence checks above already
        // skipped a name that is taken, but belt-and-braces.
        .arg("-n")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg(format!("0:{}", stream.index));
    cmd.args(codec_args);
    cmd.arg(&sidecar_path);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("ffmpeg spawn failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg exited with {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    Ok(ExtractOutcome::Written(sidecar_path))
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
        assert_eq!(
            legacy_sidecar_name("01 Title", &streams[0]),
            "01 Title.cc.2.eng.srt"
        );
        assert_eq!(
            legacy_sidecar_name("01 Title", &streams[1]),
            "01 Title.cc.3.fre.vtt"
        );
        // No language (or `und`) was left out of the old name.
        assert_eq!(
            legacy_sidecar_name("01 Title", &streams[3]),
            "01 Title.cc.5.srt"
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
        let written = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap();
        assert_eq!(written, 3);
        for name in ["01 Title.en.srt", "01 Title.en.sdh.srt", "01 Title.und.srt"] {
            assert!(dir.path().join(name).exists(), "{name} missing");
        }

        // A re-run writes nothing new.
        let again = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap();
        assert_eq!(again, 0);

        // A file under the pre-#1251 name also counts as already extracted.
        fs::remove_file(dir.path().join("01 Title.en.srt")).unwrap();
        fs::write(dir.path().join("01 Title.cc.1.eng.srt"), "old").unwrap();
        let with_old = extract_subtitles_to_sidecars(ffprobe, ffmpeg, &video)
            .await
            .unwrap();
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
}
