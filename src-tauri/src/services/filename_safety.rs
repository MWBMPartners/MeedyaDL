// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Engine filename-safety contract (#551).
// =======================================
//
// A **design-review tool**, not a runtime guard. Every new download-engine
// integration (votify / yt-dlp / get_iplayer / ...) is expected to
// implement `FilenameSafetyContract` so that the review PR can demonstrate
// — via compile-time `impl` and the unit tests below — that the engine's
// default templates cannot reproduce the class of bug chased across the
// Apple Music pipeline in #527 / #531 / #537:
//
//   - **Fallback filename consists only of punctuation** because every
//     placeholder resolved to the empty string (the original `"{disc} - "`
//     MV bug → `"-.mp4"`).
//   - **Fallback folder path contains a literal `[Unknown]` or
//     `Unknown Album` sentinel** as the entire path segment (silent
//     collisions between two distinct unknown items).
//   - **Two items with different stable IDs but identical metadata
//     produce the same filename**, triggering `MediaFileExists` skips
//     under `overwrite=false` with no user-visible warning.
//
// The trait carries no runtime behaviour — it's a contract surfaced via
// `impl` blocks and exercised by unit tests. Runtime enforcement remains
// the job of `fs_safe.rs` and the #487 umbrella.

use std::collections::HashSet;

/// Minimal tag set that every engine is expected to populate when
/// rendering its fallback templates. All fields are `Option<&str>` so
/// the contract can simulate the "every field is missing" failure mode
/// without constructing full engine-specific tag structs.
///
/// Engines may internally carry richer tag objects; this struct is only
/// what the contract's template-rendering assertions need.
#[derive(Debug, Default, Clone)]
pub struct FilenameTags<'a> {
    /// Human-readable title (may be empty on API miss).
    pub title: Option<&'a str>,
    /// Primary artist (may be empty).
    pub artist: Option<&'a str>,
    /// Collection / album / show name (may be empty).
    pub collection: Option<&'a str>,
    /// Engine-stable unique ID for the item being rendered.
    ///
    /// This is the placeholder that carries the uniqueness guarantee
    /// when every other field is empty. For Apple Music this is
    /// `{title_id}` / `{album_id}` / `{playlist_id}`; for Spotify it
    /// will be `{spotify_id}`; for YouTube `{id}`; for BBC iPlayer
    /// `{pid}`. **Templates rendered without a stable ID placeholder
    /// are the headline failure the contract exists to prevent.**
    pub stable_id: Option<&'a str>,
}

/// Design-review contract every engine integration must satisfy BEFORE
/// being wired into the download pipeline.
///
/// See module-level docs for the motivation and failure modes. See
/// `DEV_NOTES.md` → "Engine Filename Safety Contract (#551)" for the
/// human-facing review checklist.
pub trait FilenameSafetyContract {
    /// Stable engine identifier, matching `engines.toml`.
    fn engine_id(&self) -> &str;

    /// Default filename template used when the engine has no album /
    /// collection context to anchor the filename on.
    ///
    /// **Invariant**: the rendered filename MUST be guaranteed unique
    /// across the engine's ID space for a given content type even when
    /// every other metadata field is empty. The canonical way to
    /// satisfy this is to reference the engine's stable unique ID
    /// placeholder — `{title_id}` (Apple Music MVs), `{spotify_id}`
    /// (Spotify), `{id}` (YouTube), `{pid}` (BBC iPlayer).
    fn fallback_file_template(&self) -> &str;

    /// Default folder template used when the engine has no album /
    /// collection context.
    ///
    /// **Invariant**: MUST NOT consist of a single literal sentinel
    /// (`[Unknown]`, `Unknown Album`, `(no album)`, ...) as the whole
    /// segment. Route genuinely-unknown content to a stable, named
    /// fallback folder (`{artist}/Music Videos/`, `{artist}/Singles/`,
    /// `{channel}/`, `{programme_name}/`, ...) so two distinct unknown
    /// items don't silently collide in the same directory.
    fn fallback_folder_template(&self) -> &str;

    /// Placeholder names the engine's template renderer supports.
    ///
    /// Surfaced by the template-builder UI so users can discover which
    /// variables are available per engine. Every name returned here
    /// must be a valid `{name}` substitution in the engine's renderer.
    fn supported_placeholders(&self) -> &[&str];

    /// Name of the stable-ID placeholder this engine's fallback file
    /// template references, if any. Used by the `must_reference_stable_id`
    /// conformance check.
    ///
    /// Returning `None` is a deliberate signal that the engine's fallback
    /// template does NOT include a stable ID placeholder — the
    /// conformance assertion will fail and force the reviewer to fix the
    /// template.
    fn stable_id_placeholder(&self) -> Option<&str>;

    /// Renders the fallback file template against the provided tags.
    ///
    /// Implementations should substitute sensible fallback values for
    /// missing fields (e.g. literal `"Unknown Title"` for a missing
    /// `title`) rather than letting placeholders resolve to the empty
    /// string. The trim-only-punctuation invariant below depends on
    /// this contract.
    fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String;

    // ----------------------------------------------------------------
    // Default-provided conformance checks (engines inherit these).
    // Each corresponds to one item on the DEV_NOTES review checklist.
    // ----------------------------------------------------------------

    /// Checklist item 1 — fallback file template includes the engine's
    /// declared stable-ID placeholder.
    fn must_reference_stable_id(&self) -> Result<(), FilenameSafetyViolation> {
        let Some(placeholder) = self.stable_id_placeholder() else {
            return Err(FilenameSafetyViolation::MissingStableId {
                engine: self.engine_id().to_string(),
            });
        };
        let needle = format!("{{{placeholder}}}");
        if self.fallback_file_template().contains(&needle) {
            Ok(())
        } else {
            Err(FilenameSafetyViolation::StableIdNotInTemplate {
                engine: self.engine_id().to_string(),
                placeholder: placeholder.to_string(),
                template: self.fallback_file_template().to_string(),
            })
        }
    }

    /// Checklist item 2 — fallback folder template contains no literal
    /// `[Unknown]` / `Unknown*` segment as the entire path.
    fn must_not_be_unknown_sentinel(&self) -> Result<(), FilenameSafetyViolation> {
        let template = self.fallback_folder_template().trim();
        const FORBIDDEN: &[&str] = &[
            "[Unknown]",
            "Unknown",
            "Unknown Album",
            "Unknown Artist",
            "(no album)",
            "(unknown)",
        ];
        if FORBIDDEN.iter().any(|bad| template.eq_ignore_ascii_case(bad)) {
            return Err(FilenameSafetyViolation::UnknownSentinelFolder {
                engine: self.engine_id().to_string(),
                template: template.to_string(),
            });
        }
        Ok(())
    }

    /// Checklist item 4 — rendering the fallback file template against a
    /// fully-empty tag set still yields a filename that isn't pure
    /// punctuation or whitespace.
    fn must_survive_empty_metadata(&self) -> Result<(), FilenameSafetyViolation> {
        let rendered = self.render_fallback_filename(&FilenameTags::default());
        let meaningful = rendered
            .chars()
            .any(|c| c.is_alphanumeric() || !c.is_ascii());
        if meaningful {
            Ok(())
        } else {
            Err(FilenameSafetyViolation::EmptyRendered {
                engine: self.engine_id().to_string(),
                rendered,
            })
        }
    }

    /// Checklist item 5 — two items that differ ONLY in stable ID render
    /// to different filenames. Guards the dedup path for same-title
    /// re-releases (Clean/Explicit, remix, live, regional variants).
    fn must_disambiguate_by_stable_id(&self) -> Result<(), FilenameSafetyViolation> {
        let a = self.render_fallback_filename(&FilenameTags {
            title: Some("Song"),
            artist: Some("Artist"),
            stable_id: Some("id-A"),
            ..Default::default()
        });
        let b = self.render_fallback_filename(&FilenameTags {
            title: Some("Song"),
            artist: Some("Artist"),
            stable_id: Some("id-B"),
            ..Default::default()
        });
        if a != b {
            Ok(())
        } else {
            Err(FilenameSafetyViolation::NoStableIdDisambiguation {
                engine: self.engine_id().to_string(),
                rendered: a,
            })
        }
    }

    /// Convenience: run every default conformance check and collect
    /// every violation, not just the first.
    fn run_all_checks(&self) -> Vec<FilenameSafetyViolation> {
        let mut violations = Vec::new();
        if let Err(v) = self.must_reference_stable_id() {
            violations.push(v);
        }
        if let Err(v) = self.must_not_be_unknown_sentinel() {
            violations.push(v);
        }
        if let Err(v) = self.must_survive_empty_metadata() {
            violations.push(v);
        }
        if let Err(v) = self.must_disambiguate_by_stable_id() {
            violations.push(v);
        }
        violations
    }
}

/// Structured violation produced by the default conformance checks.
///
/// Printed by the unit tests on failure so the offending engine is
/// identified in CI output without stepping through a debugger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilenameSafetyViolation {
    /// Engine declared no stable-ID placeholder at all.
    MissingStableId { engine: String },
    /// Engine's declared stable-ID placeholder isn't referenced by the
    /// fallback file template.
    StableIdNotInTemplate {
        engine: String,
        placeholder: String,
        template: String,
    },
    /// Fallback folder template is a literal `[Unknown]`-style sentinel.
    UnknownSentinelFolder { engine: String, template: String },
    /// Fallback file template rendered against empty tags produced a
    /// filename with no alphanumeric content (pure punctuation).
    EmptyRendered { engine: String, rendered: String },
    /// Two items differing only in stable ID produced identical
    /// filenames.
    NoStableIdDisambiguation { engine: String, rendered: String },
}

impl std::fmt::Display for FilenameSafetyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingStableId { engine } => write!(
                f,
                "[{engine}] declares no stable-ID placeholder — fallback filenames cannot be disambiguated",
            ),
            Self::StableIdNotInTemplate {
                engine,
                placeholder,
                template,
            } => write!(
                f,
                "[{engine}] fallback file template {template:?} does not reference declared stable-ID placeholder {{{placeholder}}}",
            ),
            Self::UnknownSentinelFolder { engine, template } => write!(
                f,
                "[{engine}] fallback folder template {template:?} is a literal unknown-sentinel — route to a stable named folder instead",
            ),
            Self::EmptyRendered { engine, rendered } => write!(
                f,
                "[{engine}] fallback file template rendered to {rendered:?} against empty tags — no alphanumeric content",
            ),
            Self::NoStableIdDisambiguation { engine, rendered } => write!(
                f,
                "[{engine}] rendering two items that differ only in stable ID both produced {rendered:?}",
            ),
        }
    }
}

// ====================================================================
// First conformance example — GAMDL (Apple Music) music-video fallback
// ====================================================================

/// GAMDL's music-video fallback path (the original #527 / #531 / #537
/// failure mode). The rest of the GAMDL pipeline anchors on
/// `{album_folder_template}` + `{single_disc_file_template}` which
/// already satisfy the contract via `{album}` + `{title}` + `{track}`;
/// it's the no-album fallback where the hazard actually lived.
pub struct GamdlMusicVideoFallback;

impl FilenameSafetyContract for GamdlMusicVideoFallback {
    fn engine_id(&self) -> &str {
        "gamdl"
    }

    fn fallback_file_template(&self) -> &str {
        // Mirrors `MV_NO_ALBUM_FILE_TEMPLATE` in `download_queue.rs`.
        // Kept as a string literal here rather than importing the
        // constant so this module has zero cross-module coupling —
        // that way future engines can add their own conformance impls
        // without dragging `download_queue` into the compilation unit.
        "{title} ({title_id})"
    }

    fn fallback_folder_template(&self) -> &str {
        // Mirrors `MV_NO_ALBUM_FOLDER_TEMPLATE` in `download_queue.rs`.
        "{artist}/Music Videos"
    }

    fn supported_placeholders(&self) -> &[&str] {
        // Subset that the MV fallback path itself references. The full
        // GAMDL placeholder set is documented in
        // `src/lib/template-parser.ts::TEMPLATE_VARIABLES`.
        &["artist", "title", "title_id"]
    }

    fn stable_id_placeholder(&self) -> Option<&str> {
        Some("title_id")
    }

    fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String {
        // Emulate GAMDL's substitution semantics: missing `{title}`
        // falls back to the literal "Unknown Title" (matching the
        // user-facing help copy) and missing `{title_id}` falls back
        // to a deterministic `"no-id"` token. Any implementation that
        // lets either placeholder resolve to the empty string would
        // reproduce the #527 class of bug.
        let title = tags.title.unwrap_or("Unknown Title");
        let id = tags.stable_id.unwrap_or("no-id");
        format!("{title} ({id})")
    }
}

// ====================================================================
// Tests — the actual enforcement mechanism
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Every engine registered in the contract must pass every default
    /// conformance check. Add new engines (votify / yt-dlp /
    /// get_iplayer / ...) to this slice when they are wired.
    fn registered_contracts() -> Vec<Box<dyn FilenameSafetyContract>> {
        vec![Box::new(GamdlMusicVideoFallback)]
    }

    #[test]
    fn gamdl_mv_fallback_conforms() {
        let c = GamdlMusicVideoFallback;
        let violations = c.run_all_checks();
        assert!(
            violations.is_empty(),
            "GamdlMusicVideoFallback failed conformance checks: {violations:?}",
        );
    }

    #[test]
    fn all_registered_contracts_conform() {
        // Gatekeeper test — every engine in `registered_contracts()`
        // must pass every default check. Adding a new engine that
        // fails the checklist causes this to break the build.
        for contract in registered_contracts() {
            let violations = contract.run_all_checks();
            assert!(
                violations.is_empty(),
                "engine {:?} failed filename-safety checks: {}",
                contract.engine_id(),
                violations
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            );
        }
    }

    #[test]
    fn engine_ids_are_unique() {
        // Two engines registering under the same `engine_id()` would
        // silently shadow each other in the factory — catch it here.
        let ids: HashSet<String> = registered_contracts()
            .iter()
            .map(|c| c.engine_id().to_string())
            .collect();
        assert_eq!(ids.len(), registered_contracts().len());
    }

    #[test]
    fn contract_detects_missing_stable_id_placeholder() {
        // Synthetic bad-actor engine that doesn't declare a stable ID —
        // proves the contract rejects it rather than silently passing.
        struct BadEngine;
        impl FilenameSafetyContract for BadEngine {
            fn engine_id(&self) -> &str {
                "bad"
            }
            fn fallback_file_template(&self) -> &str {
                "{title}"
            }
            fn fallback_folder_template(&self) -> &str {
                "{artist}/Music Videos"
            }
            fn supported_placeholders(&self) -> &[&str] {
                &["title", "artist"]
            }
            fn stable_id_placeholder(&self) -> Option<&str> {
                None
            }
            fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String {
                tags.title.unwrap_or("").to_string()
            }
        }
        let violations = BadEngine.run_all_checks();
        assert!(violations.iter().any(|v| matches!(v, FilenameSafetyViolation::MissingStableId { .. })));
    }

    #[test]
    fn contract_detects_unknown_sentinel_folder() {
        // Synthetic engine that routes unknown content to a literal
        // `[Unknown]` folder — the exact pre-v2 GAMDL bug.
        struct LegacyEngine;
        impl FilenameSafetyContract for LegacyEngine {
            fn engine_id(&self) -> &str {
                "legacy"
            }
            fn fallback_file_template(&self) -> &str {
                "{title} ({id})"
            }
            fn fallback_folder_template(&self) -> &str {
                "[Unknown]"
            }
            fn supported_placeholders(&self) -> &[&str] {
                &["title", "id"]
            }
            fn stable_id_placeholder(&self) -> Option<&str> {
                Some("id")
            }
            fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String {
                format!(
                    "{} ({})",
                    tags.title.unwrap_or("Unknown Title"),
                    tags.stable_id.unwrap_or("no-id"),
                )
            }
        }
        let violations = LegacyEngine.run_all_checks();
        assert!(violations
            .iter()
            .any(|v| matches!(v, FilenameSafetyViolation::UnknownSentinelFolder { .. })));
    }

    #[test]
    fn contract_detects_punctuation_only_filename() {
        // Synthetic reproduction of the original #527 bug: the template
        // was `"{disc} - "` and rendered to `"-.mp4"` because
        // `{disc}` resolved to an empty string.
        struct BugIssue527;
        impl FilenameSafetyContract for BugIssue527 {
            fn engine_id(&self) -> &str {
                "bug-527"
            }
            fn fallback_file_template(&self) -> &str {
                "{disc} - "
            }
            fn fallback_folder_template(&self) -> &str {
                "{artist}/Music Videos"
            }
            fn supported_placeholders(&self) -> &[&str] {
                &["disc"]
            }
            fn stable_id_placeholder(&self) -> Option<&str> {
                Some("disc")
            }
            fn render_fallback_filename(&self, _tags: &FilenameTags<'_>) -> String {
                // Deliberately model the bug: empty disc → " - "
                " - ".to_string()
            }
        }
        let violations = BugIssue527.run_all_checks();
        assert!(violations
            .iter()
            .any(|v| matches!(v, FilenameSafetyViolation::EmptyRendered { .. })));
    }

    #[test]
    fn contract_detects_stable_id_not_disambiguating() {
        // Synthetic engine that declares a stable-ID placeholder but
        // doesn't actually substitute it during rendering.
        struct IgnoresId;
        impl FilenameSafetyContract for IgnoresId {
            fn engine_id(&self) -> &str {
                "ignores-id"
            }
            fn fallback_file_template(&self) -> &str {
                "{title} ({id})"
            }
            fn fallback_folder_template(&self) -> &str {
                "{artist}/Music Videos"
            }
            fn supported_placeholders(&self) -> &[&str] {
                &["title", "id", "artist"]
            }
            fn stable_id_placeholder(&self) -> Option<&str> {
                Some("id")
            }
            fn render_fallback_filename(&self, tags: &FilenameTags<'_>) -> String {
                // Bug: ignores the id field entirely.
                tags.title.unwrap_or("Unknown Title").to_string()
            }
        }
        let violations = IgnoresId.run_all_checks();
        assert!(violations
            .iter()
            .any(|v| matches!(v, FilenameSafetyViolation::NoStableIdDisambiguation { .. })));
    }
}
