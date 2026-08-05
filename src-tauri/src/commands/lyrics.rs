// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Word-level lyrics connectivity test IPC command (#934).
// =========================================================
//
// `fetch_syllable_lyrics` (commands::gamdl) is the "real" syllable-lyrics
// fetch used by the enrichment pipeline. This module adds a lightweight,
// side-effect-free diagnostic the Settings > Lyrics tab can call to answer
// "will word-level lyrics actually work for me?" without waiting for a full
// download.
//
// Token resolution deliberately mirrors `download_queue.rs` Step 1b (the
// enrichment pipeline's own syllable-lyrics fetch) and uses the PREMIUM
// resolver `apple_music_api::resolve_premium_feature_token()` -- NOT the
// plain `resolve_musickit_developer_token()` -- so that users relying on the
// web-player-extracted developer token (no user-provided MusicKit
// credentials configured) get an accurate, representative test rather than
// a false "not configured" failure.

use crate::services::{apple_music_api, config_service, enhanced_lyrics_service};
use serde::Serialize;
use tauri::AppHandle;

/// A publicly-known Apple Music song ID with word-level (syllable) lyrics
/// available, used as the connectivity probe target. Chosen because it is a
/// long-standing catalog entry unlikely to be pulled from the catalog.
const PROBE_SONG_ID: &str = "1175630113";

/// Result of a word-level lyrics connectivity probe.
///
/// Returned by the `test_lyrics_connection` Tauri command and rendered by
/// the "Test word-level lyrics connection" button in Settings > Lyrics.
#[derive(Debug, Clone, Serialize)]
pub struct TestLyricsConnectionResult {
    /// Which mechanism supplied the MusicKit developer token, or `None` if
    /// no token could be resolved at all. `"user_credentials"` |
    /// `"web_player"` | `null`.
    pub token_source: Option<String>,
    /// Whether a (non-expired) Media-User-Token cookie was found.
    pub music_user_token_present: bool,
    /// Coarse outcome classification: `"word"` (probe returned word-level
    /// timing), `"line"` (probe returned lyrics but only line-level
    /// timing), `"none"` (reachable but no lyrics for the probe track), or
    /// `"skipped"` (probe never ran -- missing token/cookie).
    pub granularity: String,
    /// Human-readable guidance when the result isn't a clean "word" success.
    pub error_hint: Option<String>,
    /// Overall pass/fail. `true` only for a "word" or "line" outcome.
    pub success: bool,
}

/// Classifies the outcome of a `fetch_syllable_lyrics` probe call into the
/// `(granularity, error_hint, success)` triple used to build
/// [`TestLyricsConnectionResult`].
///
/// Pure function (no I/O) so it's covered directly by unit tests below --
/// the async network probe itself is not something CI can exercise.
fn classify_probe_outcome(outcome: &Result<Option<String>, String>) -> (String, Option<String>, bool) {
    match outcome {
        Ok(Some(ttml)) => {
            if enhanced_lyrics_service::ttml_has_word_timing(ttml) {
                ("word".to_string(), None, true)
            } else {
                (
                    "line".to_string(),
                    Some(
                        "Connected, but the probe track returned line-level timing only — \
                         word-level lyrics may still work for other tracks."
                            .to_string(),
                    ),
                    true,
                )
            }
        }
        Ok(None) => (
            "none".to_string(),
            Some(
                "Endpoint reachable but Apple returned no lyrics for the probe track — \
                 authentication works; try a real download."
                    .to_string(),
            ),
            false,
        ),
        Err(e) => ("none".to_string(), Some(e.clone()), false),
    }
}

/// Tests whether MeedyaDL can currently fetch word-level (syllable) lyrics
/// from the Apple Music API, without running a full download.
///
/// Resolves credentials the same way the enrichment pipeline's Step 1b
/// does (`apple_music_api::resolve_premium_feature_token`, which falls back
/// to the web-player-extracted developer token when the user hasn't
/// configured their own MusicKit credentials), extracts the
/// `media-user-token` from the configured cookies file, then probes
/// [`apple_music_api::fetch_syllable_lyrics`] against a known-good song ID.
///
/// Never returns `Err` for expected "not configured" states -- those are
/// reported as a non-`success` [`TestLyricsConnectionResult`] with an
/// `error_hint` so the Settings UI can render actionable guidance instead
/// of a raw rejected promise.
#[tauri::command]
pub async fn test_lyrics_connection(app: AppHandle) -> Result<TestLyricsConnectionResult, String> {
    let settings = config_service::load_settings(&app).unwrap_or_default();

    // Resolve JWT via the PREMIUM resolver (same inputs/order as
    // download_queue.rs Step 1b) so web-player-token users are tested
    // accurately rather than reported as "not configured".
    let private_key = apple_music_api::get_private_key_from_keychain()
        .ok()
        .flatten();
    let token_pair = apple_music_api::resolve_premium_feature_token(
        settings.musickit_team_id.as_deref(),
        settings.musickit_key_id.as_deref(),
        private_key.as_deref(),
    )
    .map_err(|e| format!("JWT error: {e}"))?;

    let Some((jwt, source)) = token_pair else {
        return Ok(TestLyricsConnectionResult {
            token_source: None,
            music_user_token_present: false,
            granularity: "skipped".to_string(),
            error_hint: Some(
                "No MusicKit credentials or web-player token available. Sign in to Apple \
                 Music (Settings > Authentication) or configure Team ID / Key ID / private \
                 key in Settings > Advanced > API Credentials."
                    .to_string(),
            ),
            success: false,
        });
    };

    let token_source = Some(match source {
        apple_music_api::TokenSource::UserCredentials => "user_credentials".to_string(),
        apple_music_api::TokenSource::WebPlayerExtracted => "web_player".to_string(),
    });

    // Extract the Media-User-Token from the configured cookies file.
    let music_user_token = settings
        .cookies_path
        .as_deref()
        .and_then(|p| apple_music_api::extract_media_user_token(p).ok().flatten());

    let Some(music_user_token) = music_user_token else {
        return Ok(TestLyricsConnectionResult {
            token_source,
            music_user_token_present: false,
            granularity: "skipped".to_string(),
            error_hint: Some(
                "Apple Music subscriber token not found or expired. Re-import cookies from \
                 your browser in Settings > Cookies."
                    .to_string(),
            ),
            success: false,
        });
    };

    let outcome = apple_music_api::fetch_syllable_lyrics(
        &jwt,
        &settings.storefront,
        PROBE_SONG_ID,
        &music_user_token,
        Some(&settings.language),
    )
    .await;

    let (granularity, error_hint, success) = classify_probe_outcome(&outcome);

    Ok(TestLyricsConnectionResult {
        token_source,
        music_user_token_present: true,
        granularity,
        error_hint,
        success,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Word-level TTML fixture (mirrors the passing fixture in
    /// `enhanced_lyrics_service::tests::convert_word_level_ttml`).
    const WORD_LEVEL_TTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word" xml:lang="en-US">
          <head>
            <metadata>
              <ttm:title>Test Song</ttm:title>
            </metadata>
          </head>
          <body>
            <div>
              <p begin="00:12.450" end="00:15.800">
                <span begin="00:12.450" end="00:13.200">Hello </span>
                <span begin="00:13.200" end="00:14.100">world </span>
                <span begin="00:14.100" end="00:15.800">today</span>
              </p>
            </div>
          </body>
        </tt>"#;

    /// Line-level-only TTML fixture (no `<span begin="">` children).
    const LINE_LEVEL_TTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <body>
            <div>
              <p begin="00:12.450" end="00:15.800">Hello world today</p>
            </div>
          </body>
        </tt>"#;

    #[test]
    fn classify_word_level_ttml_as_word() {
        let outcome: Result<Option<String>, String> = Ok(Some(WORD_LEVEL_TTML.to_string()));
        let (granularity, hint, success) = classify_probe_outcome(&outcome);
        assert_eq!(granularity, "word");
        assert!(hint.is_none());
        assert!(success);
    }

    #[test]
    fn classify_line_level_ttml_as_line() {
        let outcome: Result<Option<String>, String> = Ok(Some(LINE_LEVEL_TTML.to_string()));
        let (granularity, hint, success) = classify_probe_outcome(&outcome);
        assert_eq!(granularity, "line");
        assert!(hint.is_some());
        assert!(success);
    }

    #[test]
    fn classify_no_lyrics_as_none() {
        let outcome: Result<Option<String>, String> = Ok(None);
        let (granularity, hint, success) = classify_probe_outcome(&outcome);
        assert_eq!(granularity, "none");
        assert!(hint.is_some());
        assert!(!success);
    }

    #[test]
    fn classify_error_passthrough() {
        let outcome: Result<Option<String>, String> = Err("HTTP 401".to_string());
        let (granularity, hint, success) = classify_probe_outcome(&outcome);
        assert_eq!(granularity, "none");
        assert_eq!(hint, Some("HTTP 401".to_string()));
        assert!(!success);
    }
}
