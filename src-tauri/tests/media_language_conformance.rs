// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// src-tauri/tests/media_language_conformance.rs
//
// Language policy conformance test (#1244, #1245)
// ===============================================
//
// MeedyaDL follows the shared Media Language & BCP 47 Policy
// (MWBM-MEDIA-LANG 1.0.0 — docs/standards/media-language-bcp47-policy.md).
// It does not implement the rules itself: it calls the shared `meedya-lang`
// crate, pinned by commit in src-tauri/Cargo.toml. This test proves that
// the pinned version of the crate gives exactly the answers MeedyaDL's own
// copy of the policy's test cases expects
// (tests/fixtures/bcp47-language-policy-v1.json).
//
// Why a consumer needs this at all, when core already runs the same cases:
// the crate and the copy of the test cases are pinned separately. If one
// is moved without the other — a re-pin of the crate to a commit with
// different rules, or a copy update with new answers — core's own test
// still passes (it checks core's crate against core's cases) while
// MeedyaDL quietly behaves differently from the policy it carries. This
// test is the only thing that compares the two things MeedyaDL actually
// ships with.
//
// Until the independent review of round 3 (28 Sept 2026), nothing actually
// checked that those two pins pointed at the SAME commit of core — the
// module comment above and .claude/memory/project_media_language_policy.md
// both warned about the failure mode without anything catching it. The
// test now reads the crate's commit out of src-tauri/Cargo.lock and the
// copies' commit out of docs/standards/MWBM-MEDIA-LANG.lock and refuses to
// run any case at all unless they agree — see `read_crate_pin` and
// `read_copies_pin` below.
//
// Which sections run (policy section 2 and the table in section 8.1):
// MeedyaDL's profiles are "canonical" and "text", plus "presentation" for
// its own settings lists only. That needs ten of the thirteen sections —
// NEEDED_SECTIONS below (the independent review's round 3 added `label`,
// which the crate implements and section 8.1 lists for the presentation
// profile; it had been left out with no reason given). The other three
// (the subtitle menu's "Off" entry, and automatic audio and subtitle
// selection) are player features MeedyaDL does not have; they are KNOWN
// (so the file carrying them is not an error) but deliberately not run,
// and the test prints that it skipped them so nobody mistakes the count
// for the whole file.
//
// Policy 8.1 requires a harness to FAIL — never quietly pass — when:
//   * the case file has a section it does not know        -> checked
//   * a section it needs is missing or empty               -> checked
//   * a case lacks a field the schema requires             -> checked
//   * a case marked "error": true is answered with a value -> checked
// Serde's derive refuses a struct whose non-Option field is missing,
// which covers most required fields; `require_present` closes the one gap
// that leaves (a field the schema requires to be PRESENT but allows to be
// `null` — an `Option<T>` cannot tell "missing" from "null" by itself).
//
// A fifth thing policy 8.1 implies, that this file used to get wrong: a
// section must run EVERY ONE of its cases, not merely as many as it has.
// Every per-section loop below used to count with a plain `run += 1` at
// the top of the loop body, then compare that count against the number of
// cases in the file. A `continue` placed anywhere AFTER that line would
// still leave the count looking complete — the case was "counted" before
// it was skipped — so a harness that silently stopped comparing a case
// partway through would report the same "ran N of N" line as one that
// checked everything. Independent review, round 3: every loop below now
// collects the IDs of the cases it actually finished comparing, in each
// section, and checks that list against every ID the file has for that
// section — an exact match, not a count, checked with `assert_eq!` right
// where each section's loop ends.
//
// This file is adapted from core's own harness,
// crates/meedya-lang/tests/conformance.rs at the pinned commit. The helper
// shapes are reused on purpose: two harnesses reading the same file the
// same way cannot disagree about what a case means.
//
// What this test CANNOT tell you: whether MeedyaDL's own code calls the
// crate where it should. That is what the unit tests beside each caller
// (settings, storefront, lyrics, subtitle names) are for.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use meedya_lang::{
    build_sidecar_name, canonicalise, embedded_data_version, from_legacy_three_letter,
    from_posix_locale, iso639_2_write, label, match_tags, parse_sidecar_name, sort_canonical,
    sort_for_presentation, sort_tracks, LanguageItem, LanguageTag, MatchLevel, PresentationContext,
    PresentationItem, PresentationKind, Role, RoleItem, TrackItem, TrackType,
};

// ---------------------------------------------------------------------
// Which sections this harness knows, and which it runs
// ---------------------------------------------------------------------

/// The sections MeedyaDL's profiles need, in the order the policy's own
/// table (section 8.1) lists them. Every one must be present and non-empty,
/// and every case in each must pass.
const NEEDED_SECTIONS: &[&str] = &[
    "canonicalise",
    "legacy_three_letter",
    "iso639_2_write",
    "posix_locale",
    "sidecar_name",
    "canonical_order",
    "track_order",
    "presentation_order",
    "label",
    "match",
];

/// Sections the policy defines that MeedyaDL's profiles do not need —
/// player features (UI-060 "Off" entry, AUTO-* automatic selection).
/// Known, so their presence is not an error; not run.
///
/// `label` (UI-070) used to be listed here too, with no reason given for
/// treating it as a player-only feature — the independent review of round
/// 3 pointed out that section 8.1's own table lists `label` as needed by
/// the "presentation" profile, which MeedyaDL declares itself as
/// following (see the module comment above), so it has moved up into
/// NEEDED_SECTIONS.
const KNOWN_NOT_NEEDED_SECTIONS: &[&str] =
    &["subtitle_menu", "auto_select_audio", "auto_select_subtitle"];

/// Top-level keys that are not sections at all.
const HEADER_KEYS: &[&str] = &[
    "$schema",
    "policy",
    "policy_version",
    "fixtures_version",
    "data_version",
];

/// The only needed section whose cases can require a refusal
/// (`"error": true`): a sidecar number the builder must refuse (TEXT-030) —
/// and within it, only its "build" cases. An `error` key anywhere else (any
/// other needed section, or a "parse" case) is refused outright, whatever
/// its value: the schema does not allow it there, and a harness that
/// quietly ignored it could be reading a case the file meant as a refusal
/// as an ordinary one. (Tightened after the independent review: this used
/// to refuse only `"error": true`, and only outside `sidecar_name`.)
const SECTIONS_WITH_REFUSAL_CASES: &[&str] = &["sidecar_name"];

/// The keys a non-null `expected` of a sidecar "parse" case must carry
/// (schema: required, but `tag`, `unrecognised` and `number` may be
/// `null`). Checked by hand because an `Option<T>` field cannot tell a
/// missing key from a `null` one.
const SIDECAR_PARSE_EXPECTED_KEYS: &[&str] =
    &["tag", "unrecognised", "roles", "number", "extension"];

// ---------------------------------------------------------------------
// Fixture-shape robustness helpers (policy 8.1)
// ---------------------------------------------------------------------

/// Panics unless every one of `keys` is present on `case` (a `null` value
/// counts as present; only a missing key does not).
fn require_present(case: &Value, keys: &[&str], id_hint: &str) {
    let obj = case
        .as_object()
        .unwrap_or_else(|| panic!("{id_hint}: fixture case is not a JSON object"));
    for key in keys {
        assert!(
            obj.contains_key(*key),
            "{id_hint}: fixture case is missing required field {key:?}"
        );
    }
}

fn case_id_hint(case: &Value, section: &str) -> String {
    case.get("id")
        .and_then(Value::as_str)
        .map(|id| format!("{section}/{id}"))
        .unwrap_or_else(|| format!("{section}/<no id>"))
}

/// Every case ID the fixture file lists for `section`, in file order —
/// what a section's loop must produce, one-for-one, having actually
/// compared each one (see the module comment on the "ran N of M" fix).
fn expected_ids_for(section: &str, raw_value: &Value) -> Vec<String> {
    raw_value[section]
        .as_array()
        .unwrap_or_else(|| panic!("the test case file has no array section {section:?}"))
        .iter()
        .map(|case| {
            case.get("id")
                .and_then(Value::as_str)
                .unwrap_or("<no id>")
                .to_string()
        })
        .collect()
}

// ---------------------------------------------------------------------
// The two pins must move together (independent review, round 3)
// ---------------------------------------------------------------------
//
// The `meedya-lang` crate (pinned by `rev` in src-tauri/Cargo.toml, and
// therefore also recorded in src-tauri/Cargo.lock) and MeedyaDL's copy of
// the policy's test cases (pinned by docs/standards/MWBM-MEDIA-LANG.lock,
// written by scripts/media-lang/check_copies.py) are two SEPARATE pins
// into the same commit history of MWBMPartners/MeedyaSuite-core. Nothing
// before this compared them — a re-pin of one without the other would
// leave this test quietly checking the crate against a set of answers
// worked out for a different revision of the policy, which is exactly the
// failure mode the module comment at the top of this file warns about.
// These two functions read both pins straight off disk, and the test
// refuses to run a single case unless they agree.

/// Whether `line` is a `[[package]]` block's `name = "meedya-lang"` line.
/// Key and value are compared SEPARATELY, each trimmed of spaces and tabs,
/// so `name="meedya-lang"` and `name   =   "meedya-lang"` both count.
/// (Independent review, round 5 of #1244: the old test compared the whole
/// line to one fixed string with exactly one space either side of `=`, so an
/// entry written with other spacing was silently not counted — the
/// "exactly one entry" rule then missed it.)
fn is_meedya_lang_name_line(line: &str) -> bool {
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };
    key.trim() == "name" && value.trim() == "\"meedya-lang\""
}

/// Every commit `raw` (a Cargo.lock's text) resolved a `meedya-lang`
/// package entry to — one per `[[package]] name = "meedya-lang"` block
/// found, in the order they appear.
///
/// Cargo.lock is TOML, but only these lines are needed here, so a small
/// line scan is used instead of pulling in a TOML parser as a test-only
/// dependency. A `[[package]]` block's `source` line looks like
/// `source = "git+https://…?rev=<hex>#<hex>"` — the commit after the `#`
/// is what Cargo actually built against (the `rev=` query parameter is
/// what Cargo.toml asked for; the two are the same commit on a normal,
/// up-to-date checkout, but the fragment is the one Cargo treats as
/// authoritative).
///
/// Split out from `read_crate_pin` (independent review, round 4 of
/// #1244) so the parsing itself is directly testable on a fixed sample
/// string, without needing a Cargo.lock on disk that names the crate
/// more than once. Kept `pub(crate)`-free and free-standing rather than
/// a method, matching `read_crate_pin`'s own original shape.
fn crate_pins(raw: &str) -> Vec<String> {
    let mut commits = Vec::new();
    let mut lines = raw.lines();
    while let Some(line) = lines.next() {
        if !is_meedya_lang_name_line(line) {
            continue;
        }
        // The `source` line is a few lines below `name` within the same
        // `[[package]]` block; stop at the next blank line (the end of
        // the block) if it is somehow not there.
        let mut found_source_line = false;
        for line in lines.by_ref() {
            if line.trim().is_empty() {
                break;
            }
            let Some(rest) = line.trim().strip_prefix("source = \"") else {
                continue;
            };
            let rest = rest.trim_end_matches('"');
            let commit = rest
                .rsplit('#')
                .next()
                .filter(|commit| !commit.is_empty())
                .unwrap_or_else(|| {
                    panic!("Cargo.lock's meedya-lang source line has no commit: {line:?}")
                });
            commits.push(commit.to_string());
            found_source_line = true;
            break;
        }
        if !found_source_line {
            panic!("Cargo.lock's meedya-lang package has no \"source\" line");
        }
    }
    commits
}

/// The commit `src-tauri/Cargo.lock` actually resolved `meedya-lang` to.
///
/// Panics unless the file names a `meedya-lang` package entry exactly
/// ONCE. Before this, the reader stopped at the FIRST entry it found and
/// used its commit without checking whether a second one existed
/// (independent review, round 4 of #1244) — Cargo can resolve the same
/// git dependency to two different commits in one lock file when two
/// different `Cargo.toml` entries in the workspace ask for different
/// refs of it, and a reader that silently picks whichever block happens
/// to come first could end up comparing against a commit that is not
/// the one `src-tauri/Cargo.toml`'s own `rev` actually asks for — quietly
/// defeating the whole point of the check this feeds (see the module
/// comment above: "The two pins must move together").
fn read_crate_pin() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock");
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path}: {e}"));
    single_crate_pin(&raw)
}

/// The zero / one / many decision, on the lock file's TEXT so it can be
/// tested at zero, one and two entries (independent review, round 5 of
/// #1244: the round-4 version of this match was only ever run against the
/// real file, which names the crate once, so a wrong pattern that took the
/// first of several would not have been noticed).
fn single_crate_pin(raw: &str) -> String {
    match crate_pins(raw).as_slice() {
        [] => panic!("Cargo.lock has no meedya-lang package entry to read a commit from"),
        [only] => only.clone(),
        many => panic!(
            "Cargo.lock names a meedya-lang package {} times ({many:?}) — expected exactly \
             one, so there is no single commit to compare against \
             docs/standards/MWBM-MEDIA-LANG.lock",
            many.len()
        ),
    }
}

#[test]
fn crate_pins_finds_every_entry_not_just_the_first() {
    // A Cargo.lock-shaped sample naming `meedya-lang` TWICE with two
    // different commits -- the shape a real lock file takes when two
    // different `Cargo.toml` entries in the workspace ask for different
    // refs of the same git dependency. Before this round, `read_crate_pin`
    // would have returned only the first commit (`aaaaaaa…`) with no sign
    // that a second, different one existed at all.
    let sample = "\
[[package]]
name = \"other-crate\"
version = \"1.0.0\"

[[package]]
name = \"meedya-lang\"
version = \"0.2.0\"
source = \"git+https://github.com/MWBMPartners/MeedyaSuite-core?rev=aaaaaaa1111111111111111111111111111111#aaaaaaa1111111111111111111111111111111\"
dependencies = [
 \"serde\",
 \"serde_json\",
]

[[package]]
name = \"meedya-lang\"
version = \"0.3.0\"
source = \"git+https://github.com/MWBMPartners/MeedyaSuite-core?rev=bbbbbbb2222222222222222222222222222222#bbbbbbb2222222222222222222222222222222\"
dependencies = [
 \"serde\",
]
";
    assert_eq!(
        crate_pins(sample),
        vec![
            "aaaaaaa1111111111111111111111111111111".to_string(),
            "bbbbbbb2222222222222222222222222222222".to_string(),
        ]
    );
}

#[test]
fn crate_pins_finds_the_one_entry_an_ordinary_lock_file_has() {
    let sample = "\
[[package]]
name = \"meedya-lang\"
version = \"0.2.0\"
source = \"git+https://github.com/MWBMPartners/MeedyaSuite-core?rev=ccccccc3333333333333333333333333333333#ccccccc3333333333333333333333333333333\"
dependencies = [
 \"serde\",
]
";
    assert_eq!(
        crate_pins(sample),
        vec!["ccccccc3333333333333333333333333333333".to_string()]
    );
}

fn lock_entry(spaced_name: &str, commit: &str) -> String {
    format!(
        "[[package]]\n{spaced_name}\nversion = \"0.2.0\"\nsource = \"git+https://example.invalid/core?rev={commit}#{commit}\"\n\n"
    )
}

#[test]
fn the_name_line_is_recognised_whatever_the_spacing_around_the_equals_sign() {
    for ok in [
        "name = \"meedya-lang\"",
        "name=\"meedya-lang\"",
        "name   =   \"meedya-lang\"",
        "\tname\t=\t\"meedya-lang\"\t",
    ] {
        assert!(is_meedya_lang_name_line(ok), "{ok:?}");
    }
    for bad in [
        "name = \"other\"",
        "version = \"meedya-lang\"",
        "// name = \"meedya-lang\"",
        "",
    ] {
        assert!(!is_meedya_lang_name_line(bad), "{bad:?}");
    }
}

#[test]
#[should_panic(expected = "has no meedya-lang package entry")]
fn single_crate_pin_panics_on_zero_entries() {
    single_crate_pin("[[package]]\nname = \"other\"\n");
}

#[test]
fn single_crate_pin_returns_the_one_entry() {
    assert_eq!(
        single_crate_pin(&lock_entry("name = \"meedya-lang\"", "c1c1")),
        "c1c1"
    );
}

#[test]
#[should_panic(expected = "expected exactly")]
fn single_crate_pin_panics_on_two_entries() {
    let two = format!(
        "{}{}",
        lock_entry("name = \"meedya-lang\"", "a1a1"),
        lock_entry("name = \"meedya-lang\"", "b2b2")
    );
    single_crate_pin(&two);
}

#[test]
#[should_panic(expected = "expected exactly")]
fn single_crate_pin_counts_an_oddly_spaced_second_entry() {
    let two = format!(
        "{}{}",
        lock_entry("name = \"meedya-lang\"", "a1a1"),
        lock_entry("name   =   \"meedya-lang\"", "b2b2")
    );
    single_crate_pin(&two);
}

/// The commit `docs/standards/MWBM-MEDIA-LANG.lock` records the policy
/// copies as having come from — written by
/// `scripts/media-lang/check_copies.py --init`/`--update`.
fn read_copies_pin() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/standards/MWBM-MEDIA-LANG.lock"
    );
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path}: {e}"));
    for line in raw.lines() {
        let Some(rest) = line.strip_prefix("source ") else {
            continue;
        };
        return rest
            .split_whitespace()
            .last()
            .unwrap_or_else(|| panic!("MWBM-MEDIA-LANG.lock's source line has no commit: {line:?}"))
            .to_string();
    }
    panic!("MWBM-MEDIA-LANG.lock has no \"source\" line to read a commit from");
}

// ---------------------------------------------------------------------
// Fixture file shape — only the sections this harness runs
// ---------------------------------------------------------------------

#[derive(Deserialize)]
struct Fixtures {
    policy: String,
    policy_version: String,
    /// Required by the schema. A plain `String` so a file without it fails
    /// to read, rather than being accepted (independent review).
    fixtures_version: String,
    data_version: String,
    canonicalise: Vec<CanonicaliseCase>,
    legacy_three_letter: Vec<SimpleCase>,
    iso639_2_write: Vec<Iso639WriteCase>,
    posix_locale: Vec<SimpleCase>,
    sidecar_name: Vec<SidecarCase>,
    canonical_order: Vec<OrderCase>,
    track_order: Vec<TrackOrderCase>,
    presentation_order: Vec<PresentationCase>,
    label: Vec<LabelCase>,
    #[serde(rename = "match")]
    match_cases: Vec<MatchCase>,
}

#[derive(Deserialize)]
struct CanonicaliseCase {
    id: String,
    input: String,
    expected: Option<String>,
    kind: String,
}

#[derive(Deserialize)]
struct SimpleCase {
    id: String,
    input: String,
    expected: Option<String>,
}

#[derive(Deserialize)]
struct Iso639WriteExpected {
    b: String,
    t: String,
}

#[derive(Deserialize)]
struct Iso639WriteCase {
    id: String,
    input: String,
    expected: Iso639WriteExpected,
}

#[derive(Deserialize, Debug)]
struct SidecarExpected {
    tag: Option<String>,
    unrecognised: Option<String>,
    roles: Vec<String>,
    number: Option<u32>,
    extension: String,
}

/// The two shapes of a `sidecar_name` case, told apart by `mode` — the
/// schema's `oneOf`, so a case with the wrong fields for its mode fails to
/// read rather than being accepted. `Build::number` is `i64` so a case's
/// negative number (which the builder must refuse) has somewhere to go.
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
enum SidecarCase {
    Build {
        id: String,
        stem: String,
        tag: String,
        roles: Vec<String>,
        extension: String,
        number: Option<i64>,
        expected: Option<String>,
        #[serde(default)]
        error: bool,
    },
    Parse {
        id: String,
        stem: String,
        filename: String,
        expected: Option<SidecarExpected>,
    },
}

#[derive(Deserialize)]
struct OrderItem {
    id: Option<String>,
    tag: String,
    original: Option<bool>,
}

#[derive(Deserialize)]
struct OrderCase {
    id: String,
    items: Vec<OrderItem>,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct TrackDef {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    tag: String,
    roles: Vec<String>,
    original: Option<bool>,
}

#[derive(Deserialize)]
struct TrackOrderCase {
    id: String,
    tracks: Vec<TrackDef>,
    expected: Vec<String>,
}

#[derive(Deserialize, Default)]
struct AccessibilityDef {
    audio_description: Option<bool>,
    captions: Option<bool>,
}

#[derive(Deserialize)]
struct PresentationItemDef {
    id: String,
    tag: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    roles: Vec<String>,
    original: Option<bool>,
}

#[derive(Deserialize)]
struct PresentationCase {
    id: String,
    preferences: Vec<String>,
    accessibility: AccessibilityDef,
    collation_keys: HashMap<String, String>,
    items: Vec<PresentationItemDef>,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct MatchExpected {
    level: String,
    /// `usize`, the type the crate returns since core `aaaa585`.
    distance: usize,
}

#[derive(Deserialize)]
struct MatchCase {
    id: String,
    preference: String,
    candidate: String,
    expected: MatchExpected,
}

/// A `label` (UI-070) case: structured data that already carries its
/// localised names, so the crate's own `label()` function only has to
/// join and order them — see the fixture schema's own description of this
/// section.
#[derive(Deserialize)]
struct LabelCase {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    language_name: String,
    roles: Vec<String>,
    role_names: HashMap<String, String>,
    /// `null` (no channel layout, e.g. a subtitle track) or an empty
    /// string (an audio track whose layout is not known) both mean "add
    /// nothing"; `label()` treats them the same way (see its own doc
    /// comment), so one `Option<String>` covers both — a `null` becomes
    /// `None` and an empty string stays `Some(String::new())`, and
    /// `label()`'s `channels.filter(|ch| !ch.is_empty())` treats the
    /// second exactly like the first.
    channels: Option<String>,
    expected: String,
}

// ---------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------

fn role_from_str(s: &str) -> Role {
    match s {
        "alternate" => Role::Alternate,
        "audio_description" => Role::AudioDescription,
        "commentary" => Role::Commentary,
        "sdh" => Role::Sdh,
        "forced" => Role::Forced,
        "other" => Role::Other,
        other => panic!("fixture uses an unknown role {other:?}"),
    }
}

fn roles_from_strs(strs: &[String]) -> Vec<Role> {
    strs.iter().map(|s| role_from_str(s)).collect()
}

/// The reverse of `role_from_str` — the fixture's own word for a role, so
/// a `label` case's `role_names` map (keyed by that same word) can be
/// looked up from the `Role` values `label()` hands back to its
/// `role_name` callback.
fn role_to_str(role: Role) -> &'static str {
    match role {
        Role::Alternate => "alternate",
        Role::AudioDescription => "audio_description",
        Role::Commentary => "commentary",
        Role::Sdh => "sdh",
        Role::Forced => "forced",
        Role::Other => "other",
    }
}

fn track_type_from_str(s: &str) -> TrackType {
    match s {
        "video" => TrackType::Video,
        "audio" => TrackType::Audio,
        "subtitle" => TrackType::Subtitle,
        "other" => TrackType::Other,
        other => panic!("fixture uses an unknown track type {other:?}"),
    }
}

fn presentation_kind_from_str(s: &str) -> PresentationKind {
    match s {
        "audio" => PresentationKind::Audio,
        "subtitle" => PresentationKind::Subtitle,
        "text" => PresentationKind::Text,
        other => panic!("fixture uses an unknown item type {other:?}"),
    }
}

fn match_level_from_str(s: &str) -> MatchLevel {
    match s {
        "exact" => MatchLevel::Exact,
        "general" => MatchLevel::General,
        "specific" => MatchLevel::Specific,
        "related" => MatchLevel::Related,
        "none" => MatchLevel::None,
        other => panic!("fixture uses an unknown match level {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Wrapper item types, one per trait the crate exposes
// ---------------------------------------------------------------------

struct OrderTestItem {
    id: String,
    tag: LanguageTag,
    original: bool,
}

impl LanguageItem for OrderTestItem {
    fn language(&self) -> &LanguageTag {
        &self.tag
    }
    fn is_original(&self) -> bool {
        self.original
    }
}

struct TrackTestItem {
    id: String,
    tag: LanguageTag,
    track_type: TrackType,
    roles: Vec<Role>,
    original: bool,
}

impl LanguageItem for TrackTestItem {
    fn language(&self) -> &LanguageTag {
        &self.tag
    }
    fn is_original(&self) -> bool {
        self.original
    }
}

// Since core `aaaa585`, `roles()` lives on the shared `RoleItem` parent
// trait, which `TrackItem` and `PresentationItem` both build on.
impl RoleItem for TrackTestItem {
    fn roles(&self) -> &[Role] {
        &self.roles
    }
}

impl TrackItem for TrackTestItem {
    fn track_type(&self) -> TrackType {
        self.track_type
    }
}

struct PresentationTestItem {
    id: String,
    tag: LanguageTag,
    kind: Option<PresentationKind>,
    roles: Vec<Role>,
    original: bool,
}

impl LanguageItem for PresentationTestItem {
    fn language(&self) -> &LanguageTag {
        &self.tag
    }
    fn is_original(&self) -> bool {
        self.original
    }
}

impl RoleItem for PresentationTestItem {
    fn roles(&self) -> &[Role] {
        &self.roles
    }
}

impl PresentationItem for PresentationTestItem {
    fn kind(&self) -> Option<PresentationKind> {
        self.kind
    }
}

/// The harness's stand-in for a real localised-name collation, exactly as
/// the fixture schema describes it: compare `collation_keys` as plain
/// strings, then the subtag. MeedyaDL's real settings list passes an order
/// worked out by the platform's own collator instead (see
/// `commands::language::order_languages_for_display`).
fn compare_groups_for<'a>(
    collation_keys: &'a HashMap<String, String>,
) -> impl Fn(&str, &str) -> Ordering + 'a {
    move |a: &str, b: &str| {
        let ka = collation_keys.get(a).map(String::as_str).unwrap_or(a);
        let kb = collation_keys.get(b).map(String::as_str).unwrap_or(b);
        ka.cmp(kb).then_with(|| a.cmp(b))
    }
}

// ---------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------

#[test]
fn media_language_policy_cases_pass_through_the_pinned_crate() {
    // The crate and the policy copies must be pinned to the SAME commit of
    // core (see the module comment above) — checked FIRST, before a single
    // case is read, because every other assertion in this test is
    // meaningless if the two pins have already drifted apart.
    let crate_pin = read_crate_pin();
    let copies_pin = read_copies_pin();
    assert_eq!(
        crate_pin, copies_pin,
        "the meedya-lang crate (src-tauri/Cargo.lock: {crate_pin}) and MeedyaDL's copy of the \
         policy's test cases (docs/standards/MWBM-MEDIA-LANG.lock: {copies_pin}) are pinned to \
         DIFFERENT commits of MWBMPartners/MeedyaSuite-core — they must move together, or every \
         case below would be checking the crate against answers worked out for a different \
         revision of the policy. Re-pin the crate to the copies' commit (`cargo update -p \
         meedya-lang` after editing the `rev` in src-tauri/Cargo.toml), or update the copies to \
         the crate's commit (`python3 scripts/media-lang/check_copies.py --update {crate_pin}`)."
    );

    // The copy of the test cases at the repository root — the one the copy
    // checker (scripts/media-lang/check_copies.py) keeps identical to the
    // master. Never a copy inside src-tauri/: that would be a second copy
    // nothing checks.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/bcp47-language-policy-v1.json"
    );
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path}: {e}"));

    // Read twice on purpose: loosely, to check the file's SHAPE (unknown
    // sections, missing or empty needed sections, required fields,
    // refusal cases) independently of which Rust fields happen to be
    // optional; and strictly, for the typed values each case runs on.
    let raw_value: Value =
        serde_json::from_str(&raw).expect("the test case file is not valid JSON");
    let top_level = raw_value
        .as_object()
        .expect("the test case file's top level is not a JSON object");

    for key in top_level.keys() {
        let known = HEADER_KEYS.contains(&key.as_str())
            || NEEDED_SECTIONS.contains(&key.as_str())
            || KNOWN_NOT_NEEDED_SECTIONS.contains(&key.as_str());
        assert!(
            known,
            "the test case file has a section {key:?} this harness does not know — policy 8.1 \
             requires failing on an unknown section, not silently ignoring it. Decide whether \
             MeedyaDL's profiles need it and add it to NEEDED_SECTIONS or \
             KNOWN_NOT_NEEDED_SECTIONS."
        );
    }

    for section in NEEDED_SECTIONS {
        let cases = top_level
            .get(*section)
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("the test case file has no array section {section:?}"));
        assert!(
            !cases.is_empty(),
            "section {section:?} is empty — policy 8.1 requires failing on an empty section \
             this harness needs, not reporting success having run nothing"
        );
        if !SECTIONS_WITH_REFUSAL_CASES.contains(section) {
            for case in cases {
                assert!(
                    case.get("error").is_none(),
                    "{}: an \"error\" key in a section that has no refusal cases",
                    case_id_hint(case, section)
                );
            }
        }
    }

    let fixtures: Fixtures =
        serde_json::from_str(&raw).expect("the test case file does not match the expected shape");

    assert_eq!(fixtures.policy, "MWBM-MEDIA-LANG");
    // The test cases' own version moves by a minor step when the reference
    // data is refreshed (policy "Changing this policy"), so only its major
    // part is tied to the policy version this harness was written for.
    assert!(
        fixtures.fixtures_version.starts_with("1."),
        "fixtures_version {:?} is not a 1.x version of the test cases",
        fixtures.fixtures_version
    );
    assert_eq!(
        fixtures.policy_version, "1.0.0",
        "MeedyaDL's copy of the test cases is for a policy version this harness was not \
         written for — review the policy changelog before changing this line"
    );
    assert_eq!(
        fixtures.data_version,
        embedded_data_version(),
        "the test cases were worked out against different reference data from the data the \
         pinned meedya-lang embeds — the crate and the copies have been moved separately"
    );

    let mut failures: Vec<String> = Vec::new();
    // (section, cases in the file, cases run) — printed at the end.
    let mut counts: Vec<(&str, usize, usize)> = Vec::new();

    // -- canonicalise (LANG-001, LANG-026) ------------------------------
    // Plus the stability check every harness must make: re-canonicalising
    // an expected (non-malformed) answer must give it back unchanged.
    let mut ids_run: Vec<String> = Vec::new();
    let mut stability_checks_run = 0;
    for (idx, case) in fixtures.canonicalise.iter().enumerate() {
        let raw_case = &raw_value["canonicalise"][idx];
        require_present(
            raw_case,
            &["id", "rules", "input", "expected", "kind"],
            &case_id_hint(raw_case, "canonicalise"),
        );
        let got = canonicalise(&case.input);
        let got_kind = match got.kind {
            meedya_lang::TagKind::Ordinary => "ordinary",
            meedya_lang::TagKind::Grandfathered => "grandfathered",
            meedya_lang::TagKind::PrivateUse => "privateuse",
            meedya_lang::TagKind::Malformed => "malformed",
        };
        let got_expected = if got.is_malformed() {
            None
        } else {
            Some(got.tag.clone())
        };
        if got_expected != case.expected || got_kind != case.kind {
            failures.push(format!(
                "canonicalise/{}: canonicalise({:?}) = ({:?}, {:?}), expected ({:?}, {:?})",
                case.id, case.input, got_expected, got_kind, case.expected, case.kind
            ));
        }
        if let Some(expected) = &case.expected {
            stability_checks_run += 1;
            let again = canonicalise(expected);
            if again.is_malformed() || &again.tag != expected {
                failures.push(format!(
                    "canonicalise/{} (stability): canonicalise({expected:?}) did not give it \
                     back unchanged — canonical form must be a fixed point",
                    case.id
                ));
            }
        }
        // Recorded last, after every check this case gets — see the
        // module comment on why a plain counter at the top of the loop
        // could not tell a skipped case from one actually compared.
        ids_run.push(case.id.clone());
    }
    let stability_checks_in_file = fixtures
        .canonicalise
        .iter()
        .filter(|c| c.expected.is_some())
        .count();
    assert_eq!(
        stability_checks_run, stability_checks_in_file,
        "ran {stability_checks_run} stability checks but the file has {stability_checks_in_file} \
         canonicalise cases with an expected tag"
    );
    assert_eq!(
        ids_run,
        expected_ids_for("canonicalise", &raw_value),
        "section \"canonicalise\" did not compare every one of its cases, in file order"
    );
    counts.push(("canonicalise", fixtures.canonicalise.len(), ids_run.len()));

    // -- legacy_three_letter (LANG-002, LANG-003) -----------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.legacy_three_letter.iter().enumerate() {
        let raw_case = &raw_value["legacy_three_letter"][idx];
        require_present(
            raw_case,
            &["id", "rules", "input", "expected"],
            &case_id_hint(raw_case, "legacy_three_letter"),
        );
        let got = from_legacy_three_letter(&case.input).map(|t| t.tag);
        if got != case.expected {
            failures.push(format!(
                "legacy_three_letter/{}: from_legacy_three_letter({:?}) = {:?}, expected {:?}",
                case.id, case.input, got, case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("legacy_three_letter", &raw_value),
        "section \"legacy_three_letter\" did not compare every one of its cases, in file order"
    );
    counts.push((
        "legacy_three_letter",
        fixtures.legacy_three_letter.len(),
        ids_run.len(),
    ));

    // -- iso639_2_write (TRACK-070) -------------------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.iso639_2_write.iter().enumerate() {
        let raw_case = &raw_value["iso639_2_write"][idx];
        require_present(
            raw_case,
            &["id", "rules", "input", "expected"],
            &case_id_hint(raw_case, "iso639_2_write"),
        );
        let got = iso639_2_write(&canonicalise(&case.input));
        if got.bibliographic != case.expected.b || got.terminology != case.expected.t {
            failures.push(format!(
                "iso639_2_write/{}: iso639_2_write({:?}) = {{b: {:?}, t: {:?}}}, expected \
                 {{b: {:?}, t: {:?}}}",
                case.id,
                case.input,
                got.bibliographic,
                got.terminology,
                case.expected.b,
                case.expected.t
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("iso639_2_write", &raw_value),
        "section \"iso639_2_write\" did not compare every one of its cases, in file order"
    );
    counts.push((
        "iso639_2_write",
        fixtures.iso639_2_write.len(),
        ids_run.len(),
    ));

    // -- posix_locale (LANG-004) ----------------------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.posix_locale.iter().enumerate() {
        let raw_case = &raw_value["posix_locale"][idx];
        require_present(
            raw_case,
            &["id", "rules", "input", "expected"],
            &case_id_hint(raw_case, "posix_locale"),
        );
        let got = from_posix_locale(&case.input).map(|t| t.tag);
        if got != case.expected {
            failures.push(format!(
                "posix_locale/{}: from_posix_locale({:?}) = {:?}, expected {:?}",
                case.id, case.input, got, case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("posix_locale", &raw_value),
        "section \"posix_locale\" did not compare every one of its cases, in file order"
    );
    counts.push(("posix_locale", fixtures.posix_locale.len(), ids_run.len()));

    // -- sidecar_name (TEXT-030) ----------------------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.sidecar_name.iter().enumerate() {
        let raw_case = &raw_value["sidecar_name"][idx];
        let id_hint = case_id_hint(raw_case, "sidecar_name");
        match case {
            SidecarCase::Build {
                id,
                stem,
                tag,
                roles,
                extension,
                number,
                expected,
                error,
            } => {
                require_present(
                    raw_case,
                    &[
                        "id",
                        "rules",
                        "mode",
                        "stem",
                        "tag",
                        "roles",
                        "extension",
                        "number",
                        "expected",
                    ],
                    &id_hint,
                );
                // Three shape rules the schema states and core's own
                // harness checks since core `aaaa585`: `error` may only
                // ever be `true` (a case that is not a refusal leaves it
                // out); a refusal case expects `null`; any other build case
                // expects a file name.
                assert!(
                    raw_case
                        .get("error")
                        .is_none_or(|e| e == &Value::Bool(true)),
                    "{id_hint}: \"error\" may only be true — leave it out instead of false"
                );
                if *error {
                    assert!(
                        expected.is_none(),
                        "{id_hint}: a refusal case (error: true) must expect null"
                    );
                } else {
                    assert!(
                        expected.is_some(),
                        "{id_hint}: a build case that is not a refusal must expect a file name"
                    );
                }
                let got =
                    build_sidecar_name(stem, tag, &roles_from_strs(roles), extension, *number);
                if *error {
                    // A refusal case passes ONLY if the builder refuses.
                    if got.is_ok() {
                        failures.push(format!(
                            "sidecar_name/{id}: build_sidecar_name(.., number={number:?}) = \
                             {got:?}, expected a refusal (this case carries error: true)"
                        ));
                    }
                } else {
                    match (&got, expected) {
                        (Ok(name), Some(exp)) if name == exp => {}
                        _ => failures.push(format!(
                            "sidecar_name/{id}: build_sidecar_name({stem:?}, {tag:?}, ..) = \
                             {got:?}, expected {expected:?}"
                        )),
                    }
                }
                ids_run.push(id.clone());
            }
            SidecarCase::Parse {
                id,
                stem,
                filename,
                expected,
            } => {
                require_present(
                    raw_case,
                    &["id", "rules", "mode", "stem", "filename", "expected"],
                    &id_hint,
                );
                // Only "build" cases can ask for a refusal; the schema
                // allows no `error` key on a "parse" case at all.
                assert!(
                    raw_case.get("error").is_none(),
                    "{id_hint}: an \"error\" key on a parse case"
                );
                if !raw_case["expected"].is_null() {
                    require_present(
                        &raw_case["expected"],
                        SIDECAR_PARSE_EXPECTED_KEYS,
                        &format!("{id_hint} (expected)"),
                    );
                }
                let got = parse_sidecar_name(stem, filename);
                let matches = match (&got, expected) {
                    (None, None) => true,
                    (Some(g), Some(e)) => {
                        g.tag == e.tag
                            && g.unrecognised == e.unrecognised
                            && g.roles == roles_from_strs(&e.roles)
                            && g.number == e.number
                            && g.extension == e.extension
                    }
                    _ => false,
                };
                if !matches {
                    failures.push(format!(
                        "sidecar_name/{id}: parse_sidecar_name({stem:?}, {filename:?}) = \
                         {got:?}, expected {expected:?}"
                    ));
                }
                ids_run.push(id.clone());
            }
        }
    }
    assert_eq!(
        ids_run,
        expected_ids_for("sidecar_name", &raw_value),
        "section \"sidecar_name\" did not compare every one of its cases, in file order"
    );
    counts.push(("sidecar_name", fixtures.sidecar_name.len(), ids_run.len()));

    // -- canonical_order (LANG-010 to LANG-027) -------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.canonical_order.iter().enumerate() {
        let raw_case = &raw_value["canonical_order"][idx];
        require_present(
            raw_case,
            &["id", "rules", "description", "items", "expected"],
            &case_id_hint(raw_case, "canonical_order"),
        );
        let mut items: Vec<OrderTestItem> = case
            .items
            .iter()
            .map(|it| OrderTestItem {
                id: it.id.clone().unwrap_or_else(|| it.tag.clone()),
                tag: canonicalise(&it.tag),
                original: it.original.unwrap_or(false),
            })
            .collect();
        sort_canonical(&mut items);
        let got: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        if got != case.expected {
            failures.push(format!(
                "canonical_order/{}: {:?}, expected {:?}",
                case.id, got, case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("canonical_order", &raw_value),
        "section \"canonical_order\" did not compare every one of its cases, in file order"
    );
    counts.push((
        "canonical_order",
        fixtures.canonical_order.len(),
        ids_run.len(),
    ));

    // -- track_order (TRACK-050, TRACK-060) ------------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.track_order.iter().enumerate() {
        let raw_case = &raw_value["track_order"][idx];
        require_present(
            raw_case,
            &["id", "rules", "description", "tracks", "expected"],
            &case_id_hint(raw_case, "track_order"),
        );
        let mut tracks: Vec<TrackTestItem> = case
            .tracks
            .iter()
            .map(|t| TrackTestItem {
                id: t.id.clone(),
                tag: canonicalise(&t.tag),
                track_type: track_type_from_str(&t.kind),
                roles: roles_from_strs(&t.roles),
                original: t.original.unwrap_or(false),
            })
            .collect();
        sort_tracks(&mut tracks);
        let got: Vec<&str> = tracks.iter().map(|t| t.id.as_str()).collect();
        if got != case.expected {
            failures.push(format!(
                "track_order/{}: {:?}, expected {:?}",
                case.id, got, case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("track_order", &raw_value),
        "section \"track_order\" did not compare every one of its cases, in file order"
    );
    counts.push(("track_order", fixtures.track_order.len(), ids_run.len()));

    // -- presentation_order (UI-020 to UI-050) ---------------------------
    // Some cases carry a `selected` item. It is deliberately never passed
    // to the sort (the crate's sort takes no "selected" input at all —
    // UI-050: selecting something must not move it); those cases check
    // that the order is the same as it would be with nothing selected.
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.presentation_order.iter().enumerate() {
        let raw_case = &raw_value["presentation_order"][idx];
        require_present(
            raw_case,
            &[
                "id",
                "rules",
                "description",
                "preferences",
                "accessibility",
                "display_names",
                "collation_keys",
                "items",
                "expected",
            ],
            &case_id_hint(raw_case, "presentation_order"),
        );
        let mut items: Vec<PresentationTestItem> = case
            .items
            .iter()
            .map(|it| PresentationTestItem {
                id: it.id.clone(),
                tag: canonicalise(&it.tag),
                kind: it.kind.as_deref().map(presentation_kind_from_str),
                roles: roles_from_strs(&it.roles),
                original: it.original.unwrap_or(false),
            })
            .collect();
        let context = PresentationContext {
            preferences: case.preferences.iter().map(|p| canonicalise(p)).collect(),
            accessibility: meedya_lang::Accessibility {
                audio_description: case.accessibility.audio_description.unwrap_or(false),
                captions: case.accessibility.captions.unwrap_or(false),
            },
        };
        sort_for_presentation(
            &mut items,
            &context,
            compare_groups_for(&case.collation_keys),
        );
        let got: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        if got != case.expected {
            failures.push(format!(
                "presentation_order/{}: {:?}, expected {:?}",
                case.id, got, case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("presentation_order", &raw_value),
        "section \"presentation_order\" did not compare every one of its cases, in file order"
    );
    counts.push((
        "presentation_order",
        fixtures.presentation_order.len(),
        ids_run.len(),
    ));

    // -- label (UI-070) ---------------------------------------------------
    // Added by the independent review of round 3: section 8.1's own table
    // lists `label` as needed by the "presentation" profile, which is one
    // of the two profiles MeedyaDL declares itself as following (see the
    // module comment above) — it had been left in KNOWN_NOT_NEEDED_SECTIONS
    // with no reason given, alongside genuinely player-only features this
    // app does not have.
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.label.iter().enumerate() {
        let raw_case = &raw_value["label"][idx];
        require_present(
            raw_case,
            &[
                "id",
                "rules",
                "type",
                "language_name",
                "roles",
                "role_names",
                "channels",
                "expected",
            ],
            &case_id_hint(raw_case, "label"),
        );
        let track_type = track_type_from_str(&case.kind);
        let roles = roles_from_strs(&case.roles);
        let role_names = &case.role_names;
        let got = label(
            track_type,
            &case.language_name,
            &roles,
            |role| {
                role_names
                    .get(role_to_str(role))
                    .cloned()
                    .unwrap_or_default()
            },
            case.channels.as_deref(),
        );
        if got != case.expected {
            failures.push(format!(
                "label/{}: label({:?}, {:?}, {:?}, .., {:?}) = {:?}, expected {:?}",
                case.id,
                track_type,
                case.language_name,
                case.roles,
                case.channels,
                got,
                case.expected
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("label", &raw_value),
        "section \"label\" did not compare every one of its cases, in file order"
    );
    counts.push(("label", fixtures.label.len(), ids_run.len()));

    // -- match (MATCH-010 to MATCH-040) ----------------------------------
    let mut ids_run: Vec<String> = Vec::new();
    for (idx, case) in fixtures.match_cases.iter().enumerate() {
        let raw_case = &raw_value["match"][idx];
        require_present(
            raw_case,
            &["id", "rules", "preference", "candidate", "expected"],
            &case_id_hint(raw_case, "match"),
        );
        let got = match_tags(
            &canonicalise(&case.preference),
            &canonicalise(&case.candidate),
        );
        let expected_level = match_level_from_str(&case.expected.level);
        if got.level != expected_level || got.distance != case.expected.distance {
            failures.push(format!(
                "match/{}: match({:?}, {:?}) = ({:?}, {}), expected ({:?}, {})",
                case.id,
                case.preference,
                case.candidate,
                got.level,
                got.distance,
                expected_level,
                case.expected.distance
            ));
        }
        ids_run.push(case.id.clone());
    }
    assert_eq!(
        ids_run,
        expected_ids_for("match", &raw_value),
        "section \"match\" did not compare every one of its cases, in file order"
    );
    counts.push(("match", fixtures.match_cases.len(), ids_run.len()));

    // -- Report ------------------------------------------------------------
    // Visible with `cargo test --test media_language_conformance -- --nocapture`,
    // and always shown when the test fails.
    let total_in_file: usize = counts.iter().map(|c| c.1).sum();
    let total_run: usize = counts.iter().map(|c| c.2).sum();
    println!(
        "MWBM-MEDIA-LANG {} conformance (test cases {}, meedya-lang data {}):",
        fixtures.policy_version,
        fixtures.fixtures_version,
        embedded_data_version()
    );
    for (section, in_file, ran) in &counts {
        println!("  {section:<20} {ran:>4} of {in_file:>4} cases run");
    }
    println!(
        "  {:<20} {stability_checks_run:>4} canonical-form stability checks",
        "(canonicalise)"
    );
    for section in KNOWN_NOT_NEEDED_SECTIONS {
        let n = top_level
            .get(*section)
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        println!("  {section:<20} {n:>4} cases NOT run (a player feature MeedyaDL does not have)");
    }
    println!(
        "  total: {total_run} cases run in {} sections, {} failed",
        counts.len(),
        failures.len()
    );

    // Every needed section must have run every one of its cases — a
    // section quietly running fewer checks than the file holds is exactly
    // how a broken implementation passes.
    assert_eq!(
        counts.len(),
        NEEDED_SECTIONS.len(),
        "a needed section was not run"
    );
    for (section, in_file, ran) in &counts {
        assert_eq!(
            in_file, ran,
            "section {section} ran {ran} of its {in_file} cases"
        );
    }

    assert!(
        failures.is_empty(),
        "{} of {} conformance cases (plus {} stability checks) failed:\n{}",
        failures.len(),
        total_in_file,
        stability_checks_run,
        failures.join("\n")
    );
}
