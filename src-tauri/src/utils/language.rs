// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// src-tauri/src/utils/language.rs
//
// MeedyaDL's language handling, in one place (#1244)
// ===================================================
//
// Every rule about language tags comes from the shared Media Language &
// BCP 47 Policy (MWBM-MEDIA-LANG 1.0.0). MeedyaDL keeps an exact copy at
// docs/standards/media-language-bcp47-policy.md — read it before changing
// anything here; it is normative and this file does not repeat it. The
// rules themselves are implemented ONCE, in the shared `meedya-lang` crate
// from MeedyaSuite-core (pinned in src-tauri/Cargo.toml), and
// src-tauri/tests/media_language_conformance.rs proves that pinned crate
// passes the policy's test cases.
//
// This file is the thin layer between MeedyaDL and that crate. It exists
// for two reasons:
//
//   1. MeedyaDL asks a handful of specific questions of a language value —
//      "what should be stored for the metadata language setting?", "which
//      storefront does this locale point at?", "what goes in an LRC file's
//      [la:] line?", "what should this subtitle file be called?". Each has
//      one function here, so the answer is the same everywhere it is asked,
//      and each is unit-tested below.
//   2. Everything that touches the crate's traits (`LanguageItem`,
//      `TrackItem`, `PresentationItem`) is in this file and nowhere else.
//      When the crate's API changes, this is the one file to update — as
//      it was when core `aaaa585` moved `roles()` onto a shared `RoleItem`
//      parent trait. Callers elsewhere pass plain strings and booleans and
//      get plain strings back.
//
// Deliberately NOT done anywhere here, because the policy forbids it:
//   - splitting a tag on hyphens or underscores to pick out a part
//     (the parser reads the parts; see LANG-001);
//   - filling in `en`, the interface language or any other language when
//     the real one is unknown (LANG-003);
//   - adding a script or region the value did not state (LANG-024).

use std::cmp::Ordering;
use std::collections::HashMap;

use meedya_lang::{
    build_sidecar_name, canonicalise, from_legacy_three_letter, from_posix_locale,
    parse_sidecar_name, sort_for_presentation, sort_tracks, LanguageItem, LanguageTag,
    PresentationContext, PresentationItem, Role, RoleItem, TagKind, TrackItem, TrackType,
};

/// The longest language tag MeedyaDL stores or sends. RFC 5646 section
/// 4.4.1 asks every implementation to handle tags of at least 35
/// characters, so a tag up to that length is one any system receiving it
/// is expected to cope with; anything longer is refused rather than cut
/// (cutting a tag at a fixed length can leave half a part behind, which is
/// what the old 20-byte cut on settings import did).
pub const MAX_TAG_LEN: usize = 35;

// ---------------------------------------------------------------------
// Reading values
// ---------------------------------------------------------------------

/// Reads a value that is meant to be a language tag but might be an
/// operating-system locale name instead — the system locale (`en-GB` on
/// macOS and Windows, `en_US.UTF-8` on Linux), or an old settings value
/// written by hand.
///
/// A value that is already a well-formed tag is read as a tag (LANG-001).
/// Only when it is not does the locale-name conversion (LANG-004) get a
/// turn: `en_US.UTF-8` → `en-US`, `sr_RS@latin` → `sr-Latn-RS`. That is a
/// defined conversion, not a guess. `C`, `POSIX`, and anything that is
/// neither a tag nor a locale name give `None`.
pub fn read_tag_or_locale(raw: &str) -> Option<LanguageTag> {
    let tag = canonicalise(raw);
    if !tag.is_malformed() {
        return Some(tag);
    }
    from_posix_locale(raw)
}

/// True for the policy's special primary languages (LANG-025): `und` (not
/// known), `mul` (several), `mis` (no code), `zxx` (no language) and the
/// local-use range `qaa`–`qtz`. None of them names a language anyone could
/// be sent content in.
fn is_special_language(language: &str) -> bool {
    matches!(language, "und" | "mul" | "mis" | "zxx")
        || (language.len() == 3 && ("qaa"..="qtz").contains(&language))
}

// ---------------------------------------------------------------------
// The metadata language setting (#1246)
// ---------------------------------------------------------------------

/// Why a value cannot be stored as the metadata language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagProblem {
    /// Not a well-formed language tag (a language name such as `English`,
    /// a locale name such as `en_US`, an empty value, stray characters).
    NotALanguageTag,
    /// Longer than [`MAX_TAG_LEN`] characters once put into standard form.
    TooLong,
}

impl TagProblem {
    /// A plain-English reason, for error messages and logs.
    pub fn describe(self) -> &'static str {
        match self {
            TagProblem::NotALanguageTag => "is not a language tag",
            TagProblem::TooLong => "is longer than 35 characters",
        }
    }
}

/// The standard (canonical) form of a language tag that is about to be
/// stored — `EN-us` → `en-US`, the retired `iw` → `he` — or why it cannot
/// be stored. Used for the metadata language setting when the Settings
/// screen saves it and when a settings file is imported.
///
/// Only reads the value as a tag (LANG-001), never as a locale name: the
/// setting is documented and offered as a tag, and the policy keeps the
/// locale-name conversion for values that come from an operating system.
pub fn standard_tag_for_storage(raw: &str) -> Result<String, TagProblem> {
    let tag = canonicalise(raw);
    if tag.is_malformed() {
        return Err(TagProblem::NotALanguageTag);
    }
    if tag.tag.chars().count() > MAX_TAG_LEN {
        return Err(TagProblem::TooLong);
    }
    Ok(tag.tag)
}

/// True when `raw` is a well-formed language tag (in any letter case).
/// Used at startup to report — once — a metadata language setting that is
/// not one, without changing it (COMPAT-030).
pub fn is_language_tag(raw: &str) -> bool {
    !canonicalise(raw).is_malformed()
}

/// The value to send GAMDL's `--language` argument, from the stored
/// metadata language setting.
///
/// The setting is already put into standard form when it is saved or
/// imported (see `standard_tag_for_storage`, `#1246`), but a value written
/// before that existed — or one edited by hand — can still hold something
/// that is not quite in standard form: `"en-GB\n"`, a stray leading space,
/// or (only possible from a hand-edited file, since saving and importing
/// both refuse one) a well-formed tag longer than [`MAX_TAG_LEN`].
/// `canonicalise` reads a value like that as a tag just fine (it trims
/// whitespace itself and does not enforce MeedyaDL's own 35-character
/// STORAGE limit), so this sends the clean, standard form to GAMDL's
/// command line even though the STORED value is left exactly as it is —
/// this never rewrites the setting, only what is sent this one time
/// (COMPAT-030: valid metadata already on disk is kept, not rewritten
/// just because it is being used for something else).
///
/// This used to call `standard_tag_for_storage`, which also refuses a
/// well-formed tag over [`MAX_TAG_LEN`] characters — a rule about what
/// MeedyaDL is willing to STORE, not about what GAMDL can accept. That
/// meant an over-length tag reached GAMDL completely unstandardised: any
/// stray case, extra whitespace, or non-canonical part order in it was
/// sent through raw, because `TooLong` and "not a tag at all" both fell
/// into the same `Err` branch here and got the same untouched-raw
/// treatment (independent review, round 4 of #1244). Only a value that
/// cannot be read as a tag AT ALL is sent on unchanged now, exactly as it
/// always was before this function existed: GAMDL has always been given
/// whatever was stored, and refusing to send anything now would be a
/// behaviour change for a value that may still work today. Everything
/// else — too long for storage or not — is sent in standard form.
pub fn language_arg_for_gamdl(raw: &str) -> String {
    let tag = canonicalise(raw);
    if tag.is_malformed() {
        // Largely unchanged, but a NUL, carriage return or line feed is
        // removed: exactly what `config_service::sanitize_ini_value`
        // removes from the `language =` INI line, so the command-line
        // argument and the INI line carry identical text (independent
        // review, round 5 of #1244). Only a hand-edited settings file can
        // hold such a value; save and import refuse it. Kept as the same
        // three characters on purpose; `utils` cannot call into `services`.
        raw.replace(['\n', '\r', '\0'], "")
    } else {
        tag.tag
    }
}

// ---------------------------------------------------------------------
// Telling the frontend which language a tag really is (Codex's catch-up
// review of #1244, finding 1)
// ---------------------------------------------------------------------

/// A tag's standard form, and the identity to GROUP it with other tags
/// naming the same language: the primary language subtag for an ordinary
/// tag (`en-GB` and `en-US` both group as `"en"`); for anything else
/// (grandfathered, private-use, not a tag) its own lower-cased standard
/// text, so it is a stable key that never groups with a real language.
///
/// The frontend used to work this out with `Intl.Locale`, which disagrees
/// with the policy for real tags: it folds `cmn-Hans` into `zh` (the policy
/// keeps `cmn` distinct) and refuses `zh-cmn-Hans` outright. This reads the
/// tag with the same `canonicalise()` that `order_for_display` already uses
/// to group, so the two can never disagree. Returns plain strings, as this
/// file's header says every function here does.
pub fn language_identity(raw: &str) -> (String, String) {
    let tag = canonicalise(raw);
    let group = match (tag.kind, tag.language.as_deref()) {
        (TagKind::Ordinary, Some(language)) => language.to_string(),
        _ => tag.tag.to_ascii_lowercase(),
    };
    (tag.tag, group)
}

// ---------------------------------------------------------------------
// Sending a language to Apple Music (#1247)
// ---------------------------------------------------------------------

/// The language to ask Apple Music to localise a response in, or `None`
/// when there is no usable language — in which case nothing should be sent
/// at all, rather than a guess.
///
/// `None` for: a value that is neither a tag nor a locale name; a
/// grandfathered or private-use tag (not a localisation anyone offers); a
/// special code such as `und` (LANG-025); a tag over [`MAX_TAG_LEN`]
/// characters. Otherwise the tag in standard form — which, being a
/// canonical tag, contains only ASCII letters, digits and hyphens, so it
/// can never inject anything into a web address.
pub fn localisation_tag(raw: &str) -> Option<String> {
    let tag = read_tag_or_locale(raw)?;
    if !tag.is_ordinary() || tag.tag.chars().count() > MAX_TAG_LEN {
        return None;
    }
    let language = tag.language.as_deref()?;
    if is_special_language(language) {
        return None;
    }
    Some(tag.tag)
}

// ---------------------------------------------------------------------
// Storefronts (#1248)
// ---------------------------------------------------------------------

/// The Apple Music storefront a language tag or locale name points at: its
/// **region** part, lower-cased (`en-GB` → `gb`, `zh-Hant-TW` → `tw`,
/// `en_US.UTF-8` → `us`).
///
/// `None` when there is no two-letter country in it: no region at all
/// (`en`, `zh-Hant`), a multi-country area such as `419` in `es-419` (no
/// storefront is called that), or a value that is neither a tag nor a
/// locale name. Callers keep their own documented fallback (usually `us`)
/// for `None` — that fallback is a product rule about which storefront to
/// try, not a guess at anybody's language.
///
/// This replaced two pieces of code that split the text on hyphens and
/// took one piece: one gave `hant` for `zh-Hant-TW` (a script, not a
/// country — rejected, so Taiwan fell back to the US storefront), the
/// other gave `en` for a bare `en` (not a storefront at all).
pub fn storefront_from_language(raw: &str) -> Option<String> {
    let tag = read_tag_or_locale(raw)?;
    if !tag.is_ordinary() {
        return None;
    }
    let region = tag.region.as_deref()?;
    let is_country = region.len() == 2 && region.bytes().all(|b| b.is_ascii_alphabetic());
    is_country.then(|| region.to_ascii_lowercase())
}

// ---------------------------------------------------------------------
// Languages read from files (#1250)
// ---------------------------------------------------------------------

/// The language a file states, as a standard tag, for writing into
/// another file (an LRC file's `[la:]` line) — or `None` when the language
/// is not known.
///
/// Read with the policy's reader for values from files (LANG-002), because
/// a file may hold an old three-letter code (`eng` → `en`, `ger` → `de`) as
/// easily as a tag. `None` for a missing value, a value the reader cannot
/// recognise (`English`, `xx-y`), and `und` or any `und-…` tag ("not
/// known"): the caller then leaves the line out, and never writes an
/// invented language in its place (LANG-003).
pub fn known_file_language(raw: &str) -> Option<String> {
    let tag = from_legacy_three_letter(raw)?;
    if tag.is_ordinary() && tag.language.as_deref() == Some("und") {
        return None;
    }
    Some(tag.tag)
}

// ---------------------------------------------------------------------
// Music-video subtitle file names (#1251)
// ---------------------------------------------------------------------

/// What MeedyaDL knows about one subtitle / caption stream in a video, as
/// ffprobe reported it. Plain data, so the caller never touches the
/// crate's types.
#[derive(Debug, Clone, Default)]
pub struct SubtitleStreamFacts {
    /// ffprobe's `tags.language`, exactly as reported (`eng`, `fre-ca`,
    /// `en-GB`…), or `None` when the stream has none.
    pub raw_language: Option<String>,
    /// The file extension the sidecar will be written with (`vtt`, `srt`).
    pub extension: String,
    /// Disposition flags, from ffprobe's `disposition` object.
    pub hearing_impaired: bool,
    pub captions: bool,
    pub forced: bool,
    pub comment: bool,
    pub original: bool,
}

/// The name planned for one stream's sidecar file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSubtitleName {
    /// The whole file name, `{stem}.{tag}[.{role}…][.{n}].{ext}` (TEXT-030).
    pub file_name: String,
    /// The language part used in it: a standard tag, or `und`.
    pub tag: String,
    /// ffprobe's value, kept when it could not be read as a language (the
    /// file is then named `und`) so a log line can say what was there
    /// (COMPAT-040: report doubt, do not resolve it by guessing).
    pub unrecognised_language: Option<String>,
}

/// One stream as the crate's track sorting sees it. Private: the crate's
/// traits are implemented here and nowhere else in MeedyaDL.
struct SubtitleItem {
    position: usize,
    tag: LanguageTag,
    roles: Vec<Role>,
    original: bool,
}

impl LanguageItem for SubtitleItem {
    fn language(&self) -> &LanguageTag {
        &self.tag
    }
    fn is_original(&self) -> bool {
        self.original
    }
}

// Roles are declared once, on the crate's shared `RoleItem` parent trait
// (since core `aaaa585`); `TrackItem` builds on it.
impl RoleItem for SubtitleItem {
    fn roles(&self) -> &[Role] {
        &self.roles
    }
}

impl TrackItem for SubtitleItem {
    fn track_type(&self) -> TrackType {
        TrackType::Subtitle
    }
}

/// The roles a subtitle stream's dispositions give it — and only these,
/// because only these have a word in a sidecar file name (TEXT-030):
/// `hearing_impaired` or `captions` → SDH, `forced` → forced, `comment` →
/// commentary. Every other disposition (`default`, `descriptions`, …)
/// becomes no role.
fn subtitle_roles(facts: &SubtitleStreamFacts) -> Vec<Role> {
    let mut roles = Vec::new();
    if facts.hearing_impaired || facts.captions {
        roles.push(Role::Sdh);
    }
    if facts.forced {
        roles.push(Role::Forced);
    }
    if facts.comment {
        roles.push(Role::Commentary);
    }
    roles
}

/// Plans the sidecar file names for every subtitle stream of one video,
/// per TEXT-030: `{stem}.{tag}[.{role}…][.{n}].{ext}`. Returns one name per
/// stream, in the SAME order as `streams`.
///
/// - The tag is ffprobe's language read with LANG-002 (`eng` → `en`); a
///   missing or unreadable one is `und`, never a guess.
/// - Role words come from the dispositions (see `subtitle_roles`).
/// - A number is added only when two streams would otherwise get the same
///   name, counting from the second (`.2`, `.3`…). Which stream is "first"
///   follows the policy's stored track order (TRACK-050/060: full
///   subtitles, then SDH, forced, commentary), with ffprobe's own stream
///   order breaking ties — so the same video always gives the same names.
///
/// Refuses (`Err`) only if the crate refuses a name, which with fewer than
/// a billion streams cannot happen; the `Err` is there so an impossible
/// case is reported rather than hidden behind a panic.
pub fn plan_subtitle_sidecar_names(
    stem: &str,
    streams: &[SubtitleStreamFacts],
) -> Result<Vec<PlannedSubtitleName>, String> {
    let mut items: Vec<SubtitleItem> = streams
        .iter()
        .enumerate()
        .map(|(position, facts)| {
            let read = facts
                .raw_language
                .as_deref()
                .and_then(from_legacy_three_letter);
            SubtitleItem {
                position,
                tag: read.unwrap_or_else(|| canonicalise("und")),
                roles: subtitle_roles(facts),
                original: facts.original,
            }
        })
        .collect();

    // Stable sort: streams that tie keep ffprobe's order (LANG-027).
    sort_tracks(&mut items);

    let mut uses_of_name: HashMap<String, i64> = HashMap::new();
    let mut planned: Vec<Option<PlannedSubtitleName>> = vec![None; streams.len()];
    for item in &items {
        let facts = &streams[item.position];
        // `from_legacy_three_letter` never returns a malformed tag, so this
        // is the tag's own standard text (or `und`). Since core `aaaa585`
        // the builder itself also reads its language with that same reader
        // (TEXT-030, revised), so passing it an already-read tag gives the
        // same name as before — reading a read tag again changes nothing.
        let tag_text = item.tag.tag.clone();
        let plain = build_sidecar_name(stem, &tag_text, &item.roles, &facts.extension, None)
            .map_err(|e| e.to_string())?;
        let uses = uses_of_name.entry(plain.clone()).or_insert(0);
        *uses += 1;
        let file_name = if *uses == 1 {
            plain
        } else {
            build_sidecar_name(stem, &tag_text, &item.roles, &facts.extension, Some(*uses))
                .map_err(|e| e.to_string())?
        };
        let unrecognised_language = facts
            .raw_language
            .as_ref()
            .and_then(|raw| from_legacy_three_letter(raw).is_none().then(|| raw.clone()));
        planned[item.position] = Some(PlannedSubtitleName {
            file_name,
            tag: tag_text,
            unrecognised_language,
        });
    }

    planned
        .into_iter()
        .map(|p| p.ok_or_else(|| "a subtitle stream was left without a name".to_string()))
        .collect()
}

/// True when `file_name` is a language-named sidecar of the media file
/// whose stem is `stem` — `Song.en.srt`, `Song.en.sdh.2.vtt`, and also
/// the names MeedyaDL used before #1251, `Song.cc.2.en.srt` (whose first
/// part, `cc`, reads as a language part). False for the plain
/// `{stem}.{ext}` lyric sidecar (`Song.srt`), which has no language part,
/// and for anything belonging to a different stem.
///
/// Used by the lyrics pairing step so a video's own subtitle files are
/// never mistaken for song lyrics and copied onto `{stem}.srt`.
pub fn is_language_sidecar_of(stem: &str, file_name: &str) -> bool {
    parse_sidecar_name(stem, file_name).is_some_and(|parts| parts.tag.is_some())
}

// ---------------------------------------------------------------------
// Settings-list order (#1249)
// ---------------------------------------------------------------------

/// One entry of a settings list, as the crate's menu ordering sees it.
/// Private, like `SubtitleItem`.
struct MenuItem {
    value: String,
    tag: LanguageTag,
}

impl LanguageItem for MenuItem {
    fn language(&self) -> &LanguageTag {
        &self.tag
    }
}

// A plain language entry: no roles and no track kind (the traits'
// defaults). `RoleItem` is the parent `PresentationItem` builds on since core
// `aaaa585`.
impl RoleItem for MenuItem {}
impl PresentationItem for MenuItem {}

/// Orders the entries of a language list the way a person should see them
/// (UI-020 to UI-040): their preferred languages first, in their order;
/// then everything else alphabetically by name in the interface language;
/// special codes and values that are not tags last.
///
/// - `tags` — the entries, as stored (they are returned exactly as given,
///   only reordered; none is dropped, added or rewritten).
/// - `preferences` — the person's languages, highest priority first (the
///   interface language, then the system's preferred languages). Values
///   that are not tags are ignored, as UI-020 says.
/// - `alphabetical_primary_order` — the primary language codes (`en`,
///   `zh`…) in alphabetical order of their names in the interface
///   language. Only the platform can collate names correctly, so the
///   frontend works this list out with `Intl.Collator`; this function
///   compares language groups by their position in it. A code missing
///   from it goes after every listed one, ordered by code.
///
/// The ordering itself is `meedya_lang::sort_for_presentation`, the shared
/// implementation the policy's test cases check — not a copy of the rules.
pub fn order_for_display(
    tags: &[String],
    preferences: &[String],
    alphabetical_primary_order: &[String],
) -> Vec<String> {
    let mut items: Vec<MenuItem> = tags
        .iter()
        .map(|value| MenuItem {
            value: value.clone(),
            tag: canonicalise(value),
        })
        .collect();
    let context = PresentationContext {
        preferences: preferences.iter().map(|p| canonicalise(p)).collect(),
        ..PresentationContext::default()
    };
    // Primary language codes are compared in lower case — the form the
    // crate uses for a group — whatever case the frontend sent.
    let position: HashMap<String, usize> = alphabetical_primary_order
        .iter()
        .enumerate()
        .map(|(i, code)| (code.to_ascii_lowercase(), i))
        .rev() // so the FIRST occurrence of a repeated code wins
        .collect();
    let compare_groups = |a: &str, b: &str| -> Ordering {
        match (position.get(a), position.get(b)) {
            (Some(x), Some(y)) => x.cmp(y).then_with(|| a.cmp(b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.cmp(b),
        }
    };
    sort_for_presentation(&mut items, &context, compare_groups);
    items.into_iter().map(|item| item.value).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Reading a value that may be a tag or a locale name ─────────────

    #[test]
    fn a_tag_is_read_as_a_tag_and_a_locale_name_is_converted() {
        let tag = |raw: &str| read_tag_or_locale(raw).map(|t| t.tag);
        assert_eq!(tag("en-GB").as_deref(), Some("en-GB"));
        assert_eq!(tag("EN-gb").as_deref(), Some("en-GB"));
        assert_eq!(tag("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(tag("sr_RS@latin").as_deref(), Some("sr-Latn-RS"));
        assert_eq!(tag("C"), None);
        assert_eq!(tag("POSIX"), None);
        assert_eq!(tag(""), None);
        assert_eq!(tag("English"), None);
    }

    // ── The metadata language setting ───────────────────────────────────

    #[test]
    fn a_stored_tag_is_put_into_standard_form() {
        assert_eq!(standard_tag_for_storage("en-US"), Ok("en-US".to_string()));
        assert_eq!(standard_tag_for_storage("EN-us"), Ok("en-US".to_string()));
        assert_eq!(
            standard_tag_for_storage(" ja-JP\n"),
            Ok("ja-JP".to_string())
        );
        // A retired code is replaced by the registry's own replacement.
        assert_eq!(standard_tag_for_storage("iw-IL"), Ok("he-IL".to_string()));
        assert_eq!(
            standard_tag_for_storage("zh-hant-tw"),
            Ok("zh-Hant-TW".to_string())
        );
    }

    #[test]
    fn a_value_that_is_not_a_tag_is_refused_not_repaired() {
        for raw in ["", "English", "en_US", "en-US\nlanguage = xx", "en US"] {
            assert_eq!(
                standard_tag_for_storage(raw),
                Err(TagProblem::NotALanguageTag),
                "{raw:?} should be refused"
            );
        }
    }

    #[test]
    fn a_tag_longer_than_35_characters_is_refused_not_cut() {
        // A well-formed tag of 37 characters. The old import cut every value
        // to 20 bytes, which left `en-GB-u-ca-gregory-n` — half a part.
        let long = "en-GB-u-ca-gregory-nu-latn-x-abcdefgh";
        assert!(is_language_tag(long));
        assert_eq!(standard_tag_for_storage(long), Err(TagProblem::TooLong));
        let fits = "zh-Hant-TW-u-ca-gregory";
        assert_eq!(standard_tag_for_storage(fits), Ok(fits.to_string()));
    }

    // ── The GAMDL --language argument ───────────────────────────────────

    #[test]
    fn the_gamdl_language_argument_is_the_standard_form_of_a_stored_tag() {
        // A trailing newline or stray space slips through fine -- the
        // stored value itself is never touched, only what is sent here.
        assert_eq!(language_arg_for_gamdl("en-GB\n"), "en-GB");
        assert_eq!(language_arg_for_gamdl(" ja-JP"), "ja-JP");
        assert_eq!(language_arg_for_gamdl("EN-us"), "en-US");
        // Already standard: unchanged.
        assert_eq!(language_arg_for_gamdl("en-US"), "en-US");
    }

    #[test]
    fn a_tag_over_the_storage_length_limit_still_reaches_gamdl_in_standard_form() {
        // Well-formed (so `standard_tag_for_storage` would refuse it only
        // for being over MAX_TAG_LEN, not for being unreadable as a tag
        // at all) but 37 characters once canonicalised -- two past the
        // 35-character limit MeedyaDL enforces when STORING a value. Only
        // a hand-edited settings file could carry one this long, since
        // saving and importing both refuse it (`standard_tag_for_storage`
        // / `settle_imported_metadata_language`) -- but GAMDL itself has
        // no such limit, and used to receive this completely
        // unstandardised (mixed case, trailing newline and all) because
        // the old implementation treated "too long to store" the same as
        // "not a tag" (independent review, round 4 of #1244).
        let raw = "EN-gb-u-ca-gregory-nu-latn-x-abcdefgh\n";
        assert_eq!(standard_tag_for_storage(raw), Err(TagProblem::TooLong));
        let sent = language_arg_for_gamdl(raw);
        // Standardised: lower-case language, upper-case region, no
        // trailing whitespace -- the same transform a well-formed,
        // shorter tag gets.
        assert_eq!(sent, "en-GB-u-ca-gregory-nu-latn-x-abcdefgh");
        assert_ne!(sent, raw, "must not fall back to sending the raw value");
    }

    #[test]
    fn a_stored_value_that_is_not_a_tag_is_sent_to_gamdl_exactly_as_stored() {
        // Not a tag at all -- sent on unchanged, as it always was, rather
        // than silently dropping the argument GAMDL has always been given.
        assert_eq!(language_arg_for_gamdl("English"), "English");
        assert_eq!(language_arg_for_gamdl(""), "");
    }

    #[test]
    fn a_stored_value_that_is_not_a_tag_has_the_same_characters_removed_as_the_ini_line() {
        // Round 5 of #1244: same three characters as `sanitize_ini_value`.
        assert_eq!(language_arg_for_gamdl("en-US\0evil"), "en-USevil");
        assert_eq!(language_arg_for_gamdl("English\n"), "English");
        assert_eq!(language_arg_for_gamdl("English"), "English");
    }

    // ── Language identity, for the frontend ─────────────────────────────

    #[test]
    fn mandarin_groups_the_same_in_the_new_and_the_old_extlang_form() {
        assert_eq!(
            language_identity("cmn-Hans"),
            ("cmn-Hans".to_string(), "cmn".to_string())
        );
        assert_eq!(
            language_identity("zh-cmn-Hans"),
            ("cmn-Hans".to_string(), "cmn".to_string())
        );
        // Mandarin is not the Chinese macrolanguage.
        assert_eq!(language_identity("zh-Hant").1, "zh");
    }

    #[test]
    fn other_ordinary_languages_group_by_their_primary_subtag() {
        for (raw, group) in [
            ("yue", "yue"),
            ("nan", "nan"),
            ("fr-CA", "fr"),
            ("EN-gb", "en"),
        ] {
            assert_eq!(language_identity(raw).1, group, "{raw}");
        }
        assert_eq!(language_identity("EN-us").0, "en-US");
    }

    #[test]
    fn a_value_that_is_not_a_tag_groups_only_with_itself() {
        assert_eq!(language_identity("English").1, "english");
        assert_eq!(language_identity("ENGLISH").1, "english");
    }

    #[test]
    fn the_extlang_form_sits_with_mandarin_not_with_chinese_in_the_real_order() {
        // With the alphabetical order built from `language_identity`'s
        // groups, every group gets its own place: cmn-Hans and zh-cmn-Hans
        // together, zh-Hant elsewhere. (An order built from `Intl.Locale`
        // had no `cmn` entry at all, so Rust put Mandarin last.)
        let tags = strings(&["cmn-Hans", "zh-cmn-Hans", "yue", "zh-Hant", "nan", "en"]);
        let mut groups: Vec<String> = tags.iter().map(|t| language_identity(t).1).collect();
        groups.sort();
        groups.dedup();
        assert_eq!(groups, ["cmn", "en", "nan", "yue", "zh"]);
        assert_eq!(
            order_for_display(&tags, &[], &groups),
            ["cmn-Hans", "zh-cmn-Hans", "en", "nan", "yue", "zh-Hant"]
        );
    }

    #[test]
    fn the_real_order_for_every_interface_language_case_the_frontend_fallback_mirrors() {
        // The table in `src/hooks/useInterfaceLanguageOptions.test.ts` is
        // copied from this test's output. Offered locales en/de/fr; the
        // third argument is the frontend's alphabetical order by name in
        // that interface language (from `Intl`, verified on Node 26).
        let tags = strings(&["en", "de", "fr"]);
        let cases: [(&str, [&str; 3], [&str; 3]); 10] = [
            ("en", ["en", "fr", "de"], ["en", "fr", "de"]),
            ("de", ["de", "en", "fr"], ["de", "en", "fr"]),
            ("fr", ["de", "en", "fr"], ["fr", "de", "en"]),
            ("fr-FR", ["de", "en", "fr"], ["fr", "de", "en"]),
            ("fr-CA", ["de", "en", "fr"], ["fr", "de", "en"]),
            ("de-DE", ["de", "en", "fr"], ["de", "en", "fr"]),
            ("en-GB", ["en", "fr", "de"], ["en", "fr", "de"]),
            ("es", ["de", "fr", "en"], ["de", "fr", "en"]),
            ("ja", ["de", "fr", "en"], ["de", "fr", "en"]),
            ("", ["en", "fr", "de"], ["en", "fr", "de"]),
        ];
        for (ui, alphabetical, expected) in cases {
            let prefs = if ui.is_empty() {
                vec![]
            } else {
                strings(&[ui])
            };
            let got = order_for_display(&tags, &prefs, &strings(&alphabetical));
            assert_eq!(got, expected, "interface language {ui:?}");
        }
    }

    // ── Sending a language to Apple Music ───────────────────────────────

    #[test]
    fn a_locale_name_is_converted_not_mangled() {
        // The old code deleted the underscore and sent `enUS`.
        assert_eq!(localisation_tag("en_US").as_deref(), Some("en-US"));
        assert_eq!(localisation_tag("en-US").as_deref(), Some("en-US"));
        assert_eq!(
            localisation_tag("zh-Hant-TW").as_deref(),
            Some("zh-Hant-TW")
        );
    }

    #[test]
    fn nothing_is_sent_when_there_is_no_real_language() {
        for raw in [
            "",
            "   ",
            "und",
            "und-GB",
            "zxx",
            "mul",
            "qaa",
            "x-private",
            "C",
            "English",
        ] {
            assert_eq!(localisation_tag(raw), None, "{raw:?} should send nothing");
        }
        // Injection attempts are not tags, so nothing is sent at all —
        // rather than a cleaned-up remainder such as `en-USextendevil`.
        assert_eq!(localisation_tag("en-US&extend=evil"), None);
        assert_eq!(localisation_tag("?&=#/"), None);
    }

    // ── Storefronts ─────────────────────────────────────────────────────

    #[test]
    fn the_storefront_is_the_country_part_of_the_tag() {
        let sf = |raw: &str| storefront_from_language(raw);
        assert_eq!(sf("en-GB").as_deref(), Some("gb"));
        assert_eq!(sf("pt-BR").as_deref(), Some("br"));
        // The old code took the second piece — `Hant` — and gave up.
        assert_eq!(sf("zh-Hant-TW").as_deref(), Some("tw"));
        assert_eq!(sf("sr-Latn-RS").as_deref(), Some("rs"));
        // Linux locale names.
        assert_eq!(sf("en_US.UTF-8").as_deref(), Some("us"));
        assert_eq!(sf("sr_RS@latin").as_deref(), Some("rs"));
    }

    #[test]
    fn no_storefront_without_a_two_letter_country() {
        // The old INI code turned a bare `en` into the storefront `en`.
        assert_eq!(storefront_from_language("en"), None);
        assert_eq!(storefront_from_language("zh-Hant"), None);
        // A multi-country area is not a storefront.
        assert_eq!(storefront_from_language("es-419"), None);
        assert_eq!(storefront_from_language(""), None);
        assert_eq!(storefront_from_language("C"), None);
        assert_eq!(storefront_from_language("English"), None);
    }

    // ── Languages read from files ───────────────────────────────────────

    #[test]
    fn a_file_language_is_read_with_the_three_letter_reader() {
        assert_eq!(known_file_language("en-US").as_deref(), Some("en-US"));
        assert_eq!(known_file_language("EN").as_deref(), Some("en"));
        assert_eq!(known_file_language("eng").as_deref(), Some("en"));
        assert_eq!(known_file_language("ger").as_deref(), Some("de"));
        assert_eq!(known_file_language("zxx").as_deref(), Some("zxx"));
    }

    #[test]
    fn an_unknown_file_language_gives_nothing_never_english() {
        for raw in ["", "und", "UND", "und-Latn", "xxx", "English", "q!"] {
            assert_eq!(
                known_file_language(raw),
                None,
                "{raw:?} should give nothing"
            );
        }
    }

    // ── Music-video subtitle file names ─────────────────────────────────

    fn stream(lang: Option<&str>, ext: &str) -> SubtitleStreamFacts {
        SubtitleStreamFacts {
            raw_language: lang.map(str::to_string),
            extension: ext.to_string(),
            ..SubtitleStreamFacts::default()
        }
    }

    /// The same as `stream`, but marked as the video's original-language
    /// track — used below to prove `sort_tracks` actually reorders
    /// streams before they are named (see
    /// `an_original_track_gets_the_plain_name_ahead_of_a_plain_one`).
    fn original_stream(lang: Option<&str>, ext: &str) -> SubtitleStreamFacts {
        SubtitleStreamFacts {
            original: true,
            ..stream(lang, ext)
        }
    }

    fn names(stem: &str, streams: &[SubtitleStreamFacts]) -> Vec<String> {
        plan_subtitle_sidecar_names(stem, streams)
            .unwrap()
            .into_iter()
            .map(|p| p.file_name)
            .collect()
    }

    #[test]
    fn a_stream_is_named_by_its_standard_tag() {
        assert_eq!(
            names("Title", &[stream(Some("eng"), "srt")]),
            ["Title.en.srt"]
        );
        assert_eq!(
            names("Title", &[stream(Some("fre-ca"), "vtt")]),
            ["Title.fr-CA.vtt"]
        );
        assert_eq!(
            names("Mr. Robot", &[stream(Some("ger"), "srt")]),
            ["Mr. Robot.de.srt"]
        );
    }

    #[test]
    fn a_missing_or_unreadable_language_is_und_and_kept_for_the_log() {
        let planned = plan_subtitle_sidecar_names(
            "T",
            &[stream(None, "srt"), stream(Some("english"), "vtt")],
        )
        .unwrap();
        assert_eq!(planned[0].file_name, "T.und.srt");
        assert_eq!(planned[0].unrecognised_language, None);
        assert_eq!(planned[1].file_name, "T.und.vtt");
        assert_eq!(planned[1].unrecognised_language.as_deref(), Some("english"));
    }

    #[test]
    fn roles_come_from_the_dispositions() {
        let sdh = SubtitleStreamFacts {
            hearing_impaired: true,
            ..stream(Some("eng"), "srt")
        };
        let captions = SubtitleStreamFacts {
            captions: true,
            ..stream(Some("spa"), "srt")
        };
        let forced = SubtitleStreamFacts {
            forced: true,
            ..stream(Some("fra"), "srt")
        };
        let commentary = SubtitleStreamFacts {
            comment: true,
            ..stream(Some("deu"), "srt")
        };
        assert_eq!(
            names("T", &[sdh, captions, forced, commentary]),
            [
                "T.en.sdh.srt",
                "T.es.sdh.srt",
                "T.fr.forced.srt",
                "T.de.commentary.srt"
            ]
        );
    }

    #[test]
    fn a_number_only_for_a_clash_counting_from_the_second_in_track_order() {
        // Streams in ffprobe order: an English SDH track, then two plain
        // English tracks. This does NOT actually depend on `sort_tracks`
        // reordering anything -- it passes identically with that call
        // removed. The SDH stream's role gives it its own distinct name
        // ("T.en.sdh.srt") whatever order the three are processed in, so
        // it never contends for the same slot as the two plain tracks;
        // and the two plain tracks tie on everything `sort_tracks`
        // compares (same language, same roles, same track type), so a
        // stable sort leaves them in ffprobe's own order too. What this
        // test actually proves is narrower: a numbered suffix is added
        // only for a genuine name CLASH, counted in the order the streams
        // are processed, and a stream with no clash at all -- the SDH one
        // here, and the mismatched-extension pair below -- never gets a
        // number. (Independent review of #1251: an earlier version of
        // this comment claimed the test pinned the effect of track-order
        // sorting; it did not. The test that actually needs `sort_tracks`
        // to reorder its streams is
        // `an_original_track_gets_the_plain_name_ahead_of_a_plain_one`,
        // below.)
        let sdh = SubtitleStreamFacts {
            hearing_impaired: true,
            ..stream(Some("eng"), "srt")
        };
        assert_eq!(
            names(
                "T",
                &[sdh, stream(Some("eng"), "srt"), stream(Some("en"), "srt")]
            ),
            ["T.en.sdh.srt", "T.en.srt", "T.en.2.srt"]
        );
        // Same language, different extension: no clash, no number.
        assert_eq!(
            names(
                "T",
                &[stream(Some("eng"), "vtt"), stream(Some("eng"), "srt")]
            ),
            ["T.en.vtt", "T.en.srt"]
        );
    }

    #[test]
    fn an_original_track_gets_the_plain_name_ahead_of_a_plain_one() {
        // Both streams are English; only the SECOND is marked as the
        // video's original-language track. `sort_tracks` (TRACK-050)
        // promotes an original track ahead of a non-original one within
        // the same language group, so the original stream is processed
        // FIRST and claims the clash-free name -- even though it is
        // second in ffprobe's own stream order. Without the `sort_tracks`
        // call in `plan_subtitle_sidecar_names`, both streams would keep
        // ffprobe's order instead, and the numbers would land the other
        // way around (`T.en.srt`, `T.en.2.srt`). This is the test the
        // independent review asked for: unlike the test above, it cannot
        // pass unless the sort actually reorders the streams.
        assert_eq!(
            names(
                "T",
                &[
                    stream(Some("eng"), "srt"),
                    original_stream(Some("eng"), "srt")
                ]
            ),
            ["T.en.2.srt", "T.en.srt"]
        );
    }

    #[test]
    fn the_same_streams_always_give_the_same_names() {
        let streams = [
            stream(Some("eng"), "srt"),
            stream(Some("eng"), "srt"),
            stream(Some("jpn"), "srt"),
            stream(None, "srt"),
        ];
        assert_eq!(names("T", &streams), names("T", &streams));
        assert_eq!(
            names("T", &streams),
            ["T.en.srt", "T.en.2.srt", "T.ja.srt", "T.und.srt"]
        );
    }

    #[test]
    fn language_sidecars_are_recognised_in_both_the_new_and_the_old_shape() {
        assert!(is_language_sidecar_of("01 Title", "01 Title.en.srt"));
        assert!(is_language_sidecar_of("01 Title", "01 Title.en.sdh.2.vtt"));
        assert!(is_language_sidecar_of("01 Title", "01 Title.und.srt"));
        // The names used before #1251.
        assert!(is_language_sidecar_of("01 Title", "01 Title.cc.2.en.srt"));
        assert!(is_language_sidecar_of("01 Title", "01 Title.cc.3.vtt"));
        // A plain lyric sidecar, and other files, are not.
        assert!(!is_language_sidecar_of("01 Title", "01 Title.srt"));
        assert!(!is_language_sidecar_of("01 Title", "01 Title.ttml"));
        assert!(!is_language_sidecar_of(
            "01 Title",
            "01 Title [Lossless].srt"
        ));
        assert!(!is_language_sidecar_of("01 Title", "02 Other.en.srt"));
    }

    // ── Settings-list order ─────────────────────────────────────────────

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn preferences_first_then_alphabetical_by_name() {
        let tags = strings(&["en-US", "en-GB", "ja-JP", "de-DE", "fr-FR", "zh-Hans-CN"]);
        // English interface: German, English, French, Chinese, Japanese —
        // alphabetical by English name is Chinese, English, French,
        // German, Japanese.
        let alphabetical = strings(&["zh", "en", "fr", "de", "ja"]);
        let got = order_for_display(&tags, &strings(&["en", "en-GB", "fr"]), &alphabetical);
        // English group first (the interface language), with the EXACT
        // preference en-GB ahead of en-US (UI-045); then French (the next
        // preference); then the rest by name.
        assert_eq!(
            got,
            ["en-GB", "en-US", "fr-FR", "zh-Hans-CN", "de-DE", "ja-JP"]
        );
    }

    #[test]
    fn the_interface_languages_own_group_is_pinned_first_whatever_the_interface_language_is() {
        // Pins the exact case the independent review (round 4 of #1244)
        // asked for: MeedyaDL's three interface languages, ordered for a
        // FRENCH interface. In French the three names are "allemand"
        // (German), "anglais" (English) and "français" (French) --
        // alphabetically "allemand" sorts first, ahead of "français" --
        // so this only comes out interface-language-first because the
        // caller's own preference is honoured, not because it happens to
        // agree with plain alphabetical order (as it does for English and
        // German, MeedyaDL's other two interface languages, which is
        // exactly why a fault here could hide behind those two).
        let tags = strings(&["en", "de", "fr"]);
        let got = order_for_display(&tags, &strings(&["fr"]), &strings(&["de", "en", "fr"]));
        assert_eq!(got, ["fr", "de", "en"]);
    }

    #[test]
    fn a_value_that_is_not_a_tag_goes_last_and_is_kept_as_written() {
        let tags = strings(&["en_US", "ja-JP", "de-DE"]);
        let got = order_for_display(&tags, &[], &strings(&["de", "ja"]));
        assert_eq!(got, ["de-DE", "ja-JP", "en_US"]);
    }

    #[test]
    fn nothing_is_dropped_or_added_and_unlisted_codes_go_after_listed_ones() {
        let tags = strings(&["ru-RU", "ko-KR", "it-IT"]);
        let got = order_for_display(&tags, &strings(&["x-bad", "not a tag"]), &strings(&["it"]));
        assert_eq!(got, ["it-IT", "ko-KR", "ru-RU"]);
    }
}
