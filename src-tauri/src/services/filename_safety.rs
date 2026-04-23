// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Platform-agnostic filename safety contract (#551).
//
// This module codifies a design-time invariant that every engine
// integration (GAMDL, votify, yt-dlp, get_iplayer, …) MUST satisfy:
// the fallback / no-album filename template produces a non-empty,
// unique output even when every other metadata field is missing.
//
// The contract is a design-review tool — it is NOT a runtime guard.
// Runtime filename safety lives in `utils::fs_safe` (`safe_rename`,
// `rename_if_dest_free`, `write_deduped`). The trait here exists so
// we can prove at review time that a new engine's defaults cannot
// reintroduce the class of bug tracked by #527 / #531 / #481, where
// an empty `{title}` produced filenames like `-.mp4` and empty
// `{album}` produced folders like `[Unknown]/`.
//
// ## How to use
//
// When adding a new engine, implement `FilenameSafetyContract` for
// it (or a marker struct next to the engine's CLI builder) and add
// a unit test that calls `verify_contract(&YourImpl)`. The verifier
// enforces four invariants:
//
// 1. `stable_unique_id_placeholder` is declared in
//    `supported_placeholders`.
// 2. `fallback_file_template` contains the stable unique ID
//    placeholder (so the template is guaranteed to produce
//    distinguishing output even with all other fields empty).
// 3. `fallback_folder_template` is not a bare sentinel string like
//    `"[Unknown]"` or `"Unknown Album"`.
// 4. Neither template is the empty string.
//
// ## Parent
//
// - Issue: #551
// - Umbrella: #487

use std::collections::HashSet;

/// Placeholder syntax used by the engine's template renderer.
///
/// Different engines use different substitution conventions:
/// - GAMDL / votify: `{name}` (curly braces)
/// - yt-dlp: `%(name)s` (percent-parens)
/// - get_iplayer: `<name>` (angle brackets)
///
/// The verifier uses this to construct the literal placeholder string
/// it searches for inside the engine's fallback templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderSyntax {
    /// `{name}` — GAMDL, votify.
    CurlyBraces,
    /// `%(name)s` — yt-dlp.
    PercentParens,
    /// `<name>` — get_iplayer.
    AngleBrackets,
}

impl PlaceholderSyntax {
    /// Renders a bare placeholder name (no syntax) into the engine's
    /// literal form — e.g. `render("title_id")` on `CurlyBraces`
    /// returns `"{title_id}"`.
    #[must_use]
    pub fn render(self, placeholder_name: &str) -> String {
        match self {
            Self::CurlyBraces => format!("{{{placeholder_name}}}"),
            Self::PercentParens => format!("%({placeholder_name})s"),
            Self::AngleBrackets => format!("<{placeholder_name}>"),
        }
    }
}

/// The design-time safety contract every engine integration must
/// satisfy. See module docs.
pub trait FilenameSafetyContract {
    /// Engine identifier — matches the ID used in `engines.toml` and
    /// `engine_runner::get_command_builder`.
    fn engine_id(&self) -> &str;

    /// Default filename template used when the engine has no album /
    /// collection context (MV direct URLs, uploaded posts, sparse
    /// API responses, etc.).
    ///
    /// **Must** contain the `stable_unique_id_placeholder` in the
    /// engine's `placeholder_syntax` form, so the template produces
    /// a unique non-empty output even when every other field is empty.
    fn fallback_file_template(&self) -> &str;

    /// Default folder template for content without album context.
    ///
    /// **Must not** be a bare sentinel string like `"[Unknown]"` or
    /// `"Unknown Album"`. A safe default routes content under an
    /// artist-scoped bucket (e.g. `"{artist}/Music Videos"`) so the
    /// folder is stable and human-readable.
    fn fallback_folder_template(&self) -> &str;

    /// The placeholder name (without syntax) for the engine's stable
    /// unique ID. Examples:
    ///
    /// | Engine | Placeholder | Example value |
    /// |---|---|---|
    /// | GAMDL (Apple Music) | `title_id` | `1649434280` |
    /// | votify (Spotify) | `spotify_id` | `7qiZfU4dY1lWllzX7mPBI3` |
    /// | yt-dlp (YouTube) | `id` | `dQw4w9WgXcQ` |
    /// | get_iplayer (BBC) | `pid` | `b00snr0w` |
    ///
    /// This ID **must** be present in the engine's API response for
    /// every downloadable item — it's the primary key by definition.
    /// If your engine has items without a stable ID, the contract
    /// cannot guarantee uniqueness and the engine should not be
    /// wired until that's resolved.
    fn stable_unique_id_placeholder(&self) -> &str;

    /// All placeholders this engine's template renderer recognises,
    /// surfaced in the template builder UI so users can compose
    /// their own templates with autocomplete.
    fn supported_placeholders(&self) -> &[&str];

    /// Template syntax for this engine. Default: `CurlyBraces`
    /// (matches GAMDL and votify).
    fn placeholder_syntax(&self) -> PlaceholderSyntax {
        PlaceholderSyntax::CurlyBraces
    }
}

/// Sentinel strings that must never appear as a complete
/// `fallback_folder_template`. Each one is a literal path segment that
/// degrades the user's library organisation by routing unrelated
/// content into a single folder.
const FORBIDDEN_FOLDER_SENTINELS: &[&str] = &[
    "[Unknown]",
    "Unknown",
    "Unknown Album",
    "Unknown Artist",
    "[Unknown]/[Unknown]",
    "Unknown/Unknown",
];

/// Verifies a `FilenameSafetyContract` implementation against the
/// four design invariants. Intended for unit tests in each engine's
/// test module — any new engine integration should include
/// `verify_contract(&YourImpl).unwrap()` in its test suite.
///
/// # Errors
///
/// Returns a human-readable diagnostic when any invariant is
/// violated. The diagnostic always leads with the engine ID so
/// failures in a multi-engine test report clearly.
pub fn verify_contract(contract: &dyn FilenameSafetyContract) -> Result<(), String> {
    let engine = contract.engine_id();
    let file_tpl = contract.fallback_file_template();
    let folder_tpl = contract.fallback_folder_template();
    let id_placeholder = contract.stable_unique_id_placeholder();
    let syntax = contract.placeholder_syntax();
    let rendered_id = syntax.render(id_placeholder);
    let supported: HashSet<&&str> = contract.supported_placeholders().iter().collect();

    // Invariant 0: engine ID is non-empty (defensive).
    if engine.is_empty() {
        return Err("engine_id must not be empty".to_string());
    }

    // Invariant 1: stable unique ID placeholder is declared in
    // supported_placeholders. Prevents a refactor from silently
    // dropping the ID from the autocomplete list while leaving it
    // in the template.
    if !supported.contains(&id_placeholder) {
        return Err(format!(
            "[{engine}] stable_unique_id_placeholder '{id_placeholder}' is not listed in supported_placeholders"
        ));
    }

    // Invariant 2: fallback_file_template contains the stable
    // unique ID in the engine's native syntax. This is the core
    // uniqueness guarantee.
    if !file_tpl.contains(&rendered_id) {
        return Err(format!(
            "[{engine}] fallback_file_template '{file_tpl}' does not contain '{rendered_id}' — uniqueness not guaranteed when other fields are empty (see #527)"
        ));
    }

    // Invariant 3: fallback_folder_template is not a bare sentinel.
    for &sentinel in FORBIDDEN_FOLDER_SENTINELS {
        if folder_tpl == sentinel {
            return Err(format!(
                "[{engine}] fallback_folder_template '{folder_tpl}' is a forbidden sentinel (see #531) — route content under an artist-scoped bucket instead"
            ));
        }
    }

    // Invariant 4: both templates are non-empty.
    if file_tpl.is_empty() {
        return Err(format!("[{engine}] fallback_file_template must not be empty"));
    }
    if folder_tpl.is_empty() {
        return Err(format!("[{engine}] fallback_folder_template must not be empty"));
    }

    Ok(())
}

// ============================================================
// Conformance implementations
// ============================================================

/// GAMDL (Apple Music) — the reference implementation and the first
/// engine to retrofit the contract.
///
/// Tracks `MV_NO_ALBUM_FILE_TEMPLATE` and `MV_NO_ALBUM_FOLDER_TEMPLATE`
/// in `services::download_queue` — if those constants change, this
/// impl must change in lockstep (enforced by `gamdl_templates_match`
/// test below).
pub struct GamdlFilenameSafety;

impl FilenameSafetyContract for GamdlFilenameSafety {
    fn engine_id(&self) -> &str {
        "gamdl"
    }

    fn fallback_file_template(&self) -> &str {
        // Matches `download_queue::MV_NO_ALBUM_FILE_TEMPLATE`.
        // `{title_id}` is Apple's numeric MV / track ID, stable across
        // re-downloads. Guarantees uniqueness even when `{title}` is
        // empty or duplicated across releases.
        "{title} ({title_id})"
    }

    fn fallback_folder_template(&self) -> &str {
        // Matches `download_queue::MV_NO_ALBUM_FOLDER_TEMPLATE`.
        // `{artist}` scopes the bucket so different artists' MVs don't
        // intermix; `Music Videos` is a predictable human-readable
        // landing spot.
        "{artist}/Music Videos"
    }

    fn stable_unique_id_placeholder(&self) -> &str {
        "title_id"
    }

    fn supported_placeholders(&self) -> &[&str] {
        // Mirrors the `TEMPLATE_VARIABLES` set in
        // `src/lib/template-parser.ts`. Kept in sync manually —
        // when adding a placeholder to the frontend builder, add it
        // here too, and vice versa.
        &[
            "title",
            "title_id",
            "artist",
            "album_artist",
            "album",
            "album_id",
            "disc",
            "disc_total",
            "track",
            "track_total",
            "year",
            "release_date",
            "genre",
            "composer",
            "platform",
            "playlist_artist",
            "playlist_title",
            "playlist_id",
        ]
    }
}

/// Votify (Spotify) — stub for the planned M9 integration (#101).
///
/// Template values are placeholders based on the assumed Spotify
/// track / episode ID. Must be reviewed and tightened when the votify
/// engine is actually wired into `engine_runner::VotifyCommandBuilder`.
/// The stub is here so the trait exists and the verifier exercises
/// some non-GAMDL surface area before the integration begins.
pub struct VotifyFilenameSafety;

impl FilenameSafetyContract for VotifyFilenameSafety {
    fn engine_id(&self) -> &str {
        "votify"
    }

    fn fallback_file_template(&self) -> &str {
        "{title} ({spotify_id})"
    }

    fn fallback_folder_template(&self) -> &str {
        "{artist}/Unknown Release"
    }

    fn stable_unique_id_placeholder(&self) -> &str {
        "spotify_id"
    }

    fn supported_placeholders(&self) -> &[&str] {
        &[
            "title",
            "spotify_id",
            "artist",
            "album",
            "album_artist",
            "track",
            "disc",
            "year",
            "platform",
        ]
    }
}

/// yt-dlp (YouTube, YouTube Music, shared by BBC iPlayer fallback) —
/// stub for the planned M10 integration (#103, #104).
///
/// yt-dlp uses `%(name)s` placeholder syntax, which the verifier
/// handles via `PlaceholderSyntax::PercentParens`. The fallback
/// templates here are chosen to satisfy the contract with the video
/// `id` as the uniqueness anchor — yt-dlp guarantees every downloadable
/// item has an 11-character video ID.
pub struct YtdlpFilenameSafety;

impl FilenameSafetyContract for YtdlpFilenameSafety {
    fn engine_id(&self) -> &str {
        "ytdlp"
    }

    fn fallback_file_template(&self) -> &str {
        "%(title)s (%(id)s)"
    }

    fn fallback_folder_template(&self) -> &str {
        "%(uploader)s/Videos"
    }

    fn stable_unique_id_placeholder(&self) -> &str {
        "id"
    }

    fn supported_placeholders(&self) -> &[&str] {
        &[
            "id",
            "title",
            "uploader",
            "channel",
            "uploader_id",
            "upload_date",
            "duration",
            "ext",
            "platform",
        ]
    }

    fn placeholder_syntax(&self) -> PlaceholderSyntax {
        PlaceholderSyntax::PercentParens
    }
}

/// get_iplayer (BBC iPlayer) — stub for the planned M8 integration
/// (#102).
///
/// get_iplayer uses `<name>` placeholder syntax via its
/// `--file-prefix` flag. BBC programme IDs (PIDs) are 8-character
/// stable identifiers — the uniqueness anchor.
pub struct GetIplayerFilenameSafety;

impl FilenameSafetyContract for GetIplayerFilenameSafety {
    fn engine_id(&self) -> &str {
        "get_iplayer"
    }

    fn fallback_file_template(&self) -> &str {
        "<name> (<pid>)"
    }

    fn fallback_folder_template(&self) -> &str {
        "<channel>/iPlayer Downloads"
    }

    fn stable_unique_id_placeholder(&self) -> &str {
        "pid"
    }

    fn supported_placeholders(&self) -> &[&str] {
        &[
            "pid",
            "name",
            "channel",
            "category",
            "series",
            "episode",
            "duration",
            "firstbcast",
            "platform",
        ]
    }

    fn placeholder_syntax(&self) -> PlaceholderSyntax {
        PlaceholderSyntax::AngleBrackets
    }
}

/// Factory mirroring `engine_runner::get_command_builder`. Returns
/// `None` for unknown engine IDs.
#[must_use]
pub fn get_filename_safety_contract(
    engine_id: &str,
) -> Option<Box<dyn FilenameSafetyContract + Send + Sync>> {
    match engine_id {
        "gamdl" => Some(Box::new(GamdlFilenameSafety)),
        "votify" => Some(Box::new(VotifyFilenameSafety)),
        "ytdlp" => Some(Box::new(YtdlpFilenameSafety)),
        "get_iplayer" => Some(Box::new(GetIplayerFilenameSafety)),
        _ => None,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_syntax_renders_curly_braces() {
        assert_eq!(PlaceholderSyntax::CurlyBraces.render("title_id"), "{title_id}");
    }

    #[test]
    fn placeholder_syntax_renders_percent_parens() {
        assert_eq!(PlaceholderSyntax::PercentParens.render("id"), "%(id)s");
    }

    #[test]
    fn placeholder_syntax_renders_angle_brackets() {
        assert_eq!(PlaceholderSyntax::AngleBrackets.render("pid"), "<pid>");
    }

    // ---- Conformance: the four shipping / stub implementations ----

    #[test]
    fn gamdl_contract_verifies() {
        verify_contract(&GamdlFilenameSafety).expect("GAMDL contract must verify");
    }

    #[test]
    fn votify_contract_verifies() {
        verify_contract(&VotifyFilenameSafety).expect("votify contract must verify");
    }

    #[test]
    fn ytdlp_contract_verifies() {
        verify_contract(&YtdlpFilenameSafety).expect("yt-dlp contract must verify");
    }

    #[test]
    fn get_iplayer_contract_verifies() {
        verify_contract(&GetIplayerFilenameSafety).expect("get_iplayer contract must verify");
    }

    // ---- Lockstep with download_queue constants ----

    #[test]
    fn gamdl_templates_match_download_queue_constants() {
        // If the MV safety-net templates in download_queue.rs change,
        // this test must be updated too — the contract documents the
        // canonical templates and must not drift.
        use crate::services::download_queue::{
            MV_NO_ALBUM_FILE_TEMPLATE, MV_NO_ALBUM_FOLDER_TEMPLATE,
        };
        assert_eq!(
            GamdlFilenameSafety.fallback_file_template(),
            MV_NO_ALBUM_FILE_TEMPLATE,
            "Gamdl FilenameSafetyContract diverged from MV_NO_ALBUM_FILE_TEMPLATE"
        );
        assert_eq!(
            GamdlFilenameSafety.fallback_folder_template(),
            MV_NO_ALBUM_FOLDER_TEMPLATE,
            "Gamdl FilenameSafetyContract diverged from MV_NO_ALBUM_FOLDER_TEMPLATE"
        );
    }

    // ---- Verifier rejects known failure modes ----

    struct BadEngine {
        engine_id: &'static str,
        file_tpl: &'static str,
        folder_tpl: &'static str,
        id_ph: &'static str,
        supported: &'static [&'static str],
        syntax: PlaceholderSyntax,
    }

    impl FilenameSafetyContract for BadEngine {
        fn engine_id(&self) -> &str {
            self.engine_id
        }
        fn fallback_file_template(&self) -> &str {
            self.file_tpl
        }
        fn fallback_folder_template(&self) -> &str {
            self.folder_tpl
        }
        fn stable_unique_id_placeholder(&self) -> &str {
            self.id_ph
        }
        fn supported_placeholders(&self) -> &[&str] {
            self.supported
        }
        fn placeholder_syntax(&self) -> PlaceholderSyntax {
            self.syntax
        }
    }

    #[test]
    fn verifier_rejects_missing_id_in_template() {
        // This is the #527 root cause: template doesn't reference the
        // unique ID, so empty {title} produces degenerate filenames.
        let bad = BadEngine {
            engine_id: "test-engine",
            file_tpl: "{title}",
            folder_tpl: "{artist}/Music Videos",
            id_ph: "id",
            supported: &["title", "id", "artist"],
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        let err = verify_contract(&bad).unwrap_err();
        assert!(err.contains("does not contain '{id}'"), "err was: {err}");
        assert!(err.contains("#527"), "diagnostic should cite the originating bug");
    }

    #[test]
    fn verifier_rejects_bare_unknown_folder_sentinel() {
        let bad = BadEngine {
            engine_id: "test-engine",
            file_tpl: "{title} ({id})",
            folder_tpl: "[Unknown]",
            id_ph: "id",
            supported: &["title", "id"],
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        let err = verify_contract(&bad).unwrap_err();
        assert!(err.contains("forbidden sentinel"), "err was: {err}");
        assert!(err.contains("#531"), "diagnostic should cite the originating bug");
    }

    #[test]
    fn verifier_rejects_undeclared_id_placeholder() {
        let bad = BadEngine {
            engine_id: "test-engine",
            file_tpl: "{title} ({secret_id})",
            folder_tpl: "{artist}/Videos",
            id_ph: "secret_id",
            supported: &["title", "artist"], // secret_id not listed
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        let err = verify_contract(&bad).unwrap_err();
        assert!(
            err.contains("not listed in supported_placeholders"),
            "err was: {err}"
        );
    }

    #[test]
    fn verifier_rejects_empty_templates() {
        let bad_file = BadEngine {
            engine_id: "test-engine",
            file_tpl: "",
            folder_tpl: "{artist}/Videos",
            id_ph: "id",
            supported: &["id", "artist"],
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        assert!(verify_contract(&bad_file)
            .unwrap_err()
            .contains("fallback_file_template must not be empty"));

        let bad_folder = BadEngine {
            engine_id: "test-engine",
            file_tpl: "{title} ({id})",
            folder_tpl: "",
            id_ph: "id",
            supported: &["id", "title"],
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        assert!(verify_contract(&bad_folder)
            .unwrap_err()
            .contains("fallback_folder_template must not be empty"));
    }

    #[test]
    fn verifier_rejects_bare_unknown_album_sentinel() {
        let bad = BadEngine {
            engine_id: "test-engine",
            file_tpl: "{title} ({id})",
            folder_tpl: "Unknown Album",
            id_ph: "id",
            supported: &["title", "id"],
            syntax: PlaceholderSyntax::CurlyBraces,
        };
        assert!(verify_contract(&bad)
            .unwrap_err()
            .contains("forbidden sentinel"));
    }

    // ---- Verifier accepts good non-CurlyBraces syntaxes ----

    #[test]
    fn verifier_accepts_percent_parens_syntax() {
        struct YtdlpOk;
        impl FilenameSafetyContract for YtdlpOk {
            fn engine_id(&self) -> &str {
                "ytdlp"
            }
            fn fallback_file_template(&self) -> &str {
                "%(title)s (%(id)s)"
            }
            fn fallback_folder_template(&self) -> &str {
                "%(uploader)s/Videos"
            }
            fn stable_unique_id_placeholder(&self) -> &str {
                "id"
            }
            fn supported_placeholders(&self) -> &[&str] {
                &["id", "title", "uploader"]
            }
            fn placeholder_syntax(&self) -> PlaceholderSyntax {
                PlaceholderSyntax::PercentParens
            }
        }
        verify_contract(&YtdlpOk).expect("percent-parens verifier path should work");
    }

    #[test]
    fn factory_returns_correct_impls() {
        assert!(get_filename_safety_contract("gamdl").is_some());
        assert!(get_filename_safety_contract("votify").is_some());
        assert!(get_filename_safety_contract("ytdlp").is_some());
        assert!(get_filename_safety_contract("get_iplayer").is_some());
        assert!(get_filename_safety_contract("nonexistent").is_none());
    }

    #[test]
    fn factory_output_verifies_for_all_known_engines() {
        for engine in ["gamdl", "votify", "ytdlp", "get_iplayer"] {
            let contract = get_filename_safety_contract(engine).unwrap_or_else(|| {
                panic!("missing contract for engine '{engine}'")
            });
            verify_contract(contract.as_ref()).unwrap_or_else(|e| {
                panic!("contract for '{engine}' failed verification: {e}")
            });
        }
    }
}
