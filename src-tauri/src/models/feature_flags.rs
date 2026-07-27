// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Remote feature-flag data model.
// ==================================
//
// Wire + on-disk shape for MeedyaDL's remote feature-availability client
// (see `services::feature_flag_service` for the resolution chain and
// `.claude/memory/project_remote_feature_control.md` for the programme
// context and threat model).
//
// ## Server-side evaluation
//
// The server returns **already-resolved booleans**. The client sends its
// own app version, platform and platform version; the server resolves any
// version / platform / rollout condition and answers with a plain
// `enabled: bool` per flag key. The client performs **no** condition
// evaluation of its own — deliberately, so a rule change on the server
// applies to already-shipped binaries instead of being frozen into
// whatever semantics each old build happened to compile in.
//
// ## Forward compatibility — never `deny_unknown_fields`
//
// Every struct here is permissive by design: unknown JSON fields are
// ignored and absent fields fall back to `#[serde(default)]`. The server
// WILL grow new fields (targeting metadata, signature envelopes, richer
// notice payloads) long after any given MeedyaDL build has shipped, and an
// older client must keep working when it meets a newer payload. Adding
// `#[serde(deny_unknown_fields)]` to any type in this file would turn every
// future server-side addition into a hard parse failure on the entire
// installed base — i.e. it would convert a routine server change into a
// fleet-wide outage of the kill switch itself.
//
// Reference: https://serde.rs/attr-default.html
// Reference: https://serde.rs/variant-attrs.html#other

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A resolved snapshot of every feature flag this build knows about.
///
/// ## CRITICAL — state separation between `verdicts` and `meta`
///
/// `verdicts` is the **only** part of this struct the UI may render or act
/// on. It answers exactly one question per flag key: is this feature
/// available right now, and is there a notice to show alongside it.
///
/// `meta` is **diagnostics ONLY and must never drive anything
/// user-visible.** It records where this snapshot came from, when it was
/// fetched, and how many consecutive refreshes have failed — information
/// for logs, the activity log, and a future developer-tools panel. It must
/// never be turned into a toast, a banner, an error dialog, a disabled
/// control, or any other on-screen artefact. A user whose network is down,
/// whose DNS is filtered, or who is simply offline on a plane must see an
/// app that behaves exactly as it did on the last successful fetch — the
/// failure is ours to log, not theirs to read about.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureFlagsSnapshot {
    /// Server-side timestamp for when this set of verdicts was produced
    /// (RFC 3339). Opaque to the client — never parsed, compared, or used
    /// to expire anything; carried through so support can correlate a
    /// user's snapshot with a server-side change.
    #[serde(default)]
    pub generated_at: Option<String>,

    /// Flag key -> resolved verdict. Keys are dot-namespaced
    /// (`"core.updater"`, `"service.apple-music"`, ...).
    ///
    /// **A missing key means ENABLED.** The compiled default is an empty
    /// map, so a build that has never reached the server — or a fork built
    /// with no credentials — has every feature on. See
    /// `feature_flag_service::is_enabled`.
    ///
    /// `alias = "features"` accepts the server naming the same object
    /// `features` without a client change.
    #[serde(default, alias = "features")]
    pub verdicts: HashMap<String, FlagVerdict>,

    /// Fetch diagnostics. **Never user-visible** — see the struct-level
    /// doc comment above.
    #[serde(default)]
    pub meta: FetchMeta,
}

/// The server's resolved answer for a single flag key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlagVerdict {
    /// Whether the feature is available. Note the asymmetry with a missing
    /// key: an *absent* key means enabled (fail-open), whereas a *present*
    /// verdict with `enabled: false` is a deliberate instruction to switch
    /// the feature off.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Optional human-readable label for the feature, used when the UI
    /// needs to name the thing being gated without hardcoding a string per
    /// flag key.
    #[serde(default)]
    pub label: Option<String>,

    /// Optional notice to show the user about this feature. Notices are
    /// shown for genuinely disabled (or degraded) features — that is a
    /// deliberate, server-authored message. This is entirely separate from
    /// the silent-on-fetch-failure rule, which concerns our own inability
    /// to reach the server and is never surfaced.
    #[serde(default)]
    pub notice: Option<FlagNotice>,

    /// Server-side timestamp of the last change to this flag (RFC 3339).
    /// Opaque to the client, same as `FeatureFlagsSnapshot::generated_at`.
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl Default for FlagVerdict {
    /// Defaults to **enabled with no notice** — the fail-open posture that
    /// runs through the whole module. An unreachable or misbehaving server
    /// must never brick an offline app.
    fn default() -> Self {
        Self {
            enabled: true,
            label: None,
            notice: None,
            updated_at: None,
        }
    }
}

/// Helper for `#[serde(default = "default_true")]` on
/// [`FlagVerdict::enabled`]. A verdict object that omits `enabled`
/// entirely resolves to enabled, matching the missing-key rule.
fn default_true() -> bool {
    true
}

/// A server-authored message about a feature, shown alongside the feature
/// when it is disabled or degraded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlagNotice {
    /// Plain-English message for the user. Server-authored: the client
    /// never composes or templates this text.
    pub message: String,

    /// How prominently to present the message.
    #[serde(default)]
    pub severity: NoticeSeverity,

    /// Optional link for "learn more" affordances (status page, help
    /// topic). Consumers must validate the scheme before opening it.
    #[serde(default)]
    pub url: Option<String>,
}

/// Presentation weight for a [`FlagNotice`].
///
/// Serialised lowercase (`"info"`, `"maintenance"`, `"deprecated"`,
/// `"critical"`). `Unknown` is the `#[serde(other)]` catch-all: a future
/// server that invents a fifth severity must NOT make an older client fail
/// to parse the whole payload, so anything unrecognised lands here — and
/// [`NoticeSeverity::render_as`] maps it back to `Info` so it renders as
/// the mildest available treatment rather than as an unstyled blank.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoticeSeverity {
    /// Informational — the default when the server omits a severity.
    #[default]
    Info,
    /// Temporary outage / planned maintenance.
    Maintenance,
    /// Feature is going away in a future release.
    Deprecated,
    /// Serious problem (legal takedown, data-loss risk, upstream break).
    Critical,
    /// Any severity string this build does not recognise. Renders as
    /// [`NoticeSeverity::Info`].
    #[serde(other)]
    Unknown,
}

impl NoticeSeverity {
    /// Maps a severity onto the closed set this build can actually render.
    /// `Unknown` degrades to `Info`; every other variant is itself.
    #[must_use]
    pub fn render_as(self) -> NoticeSeverity {
        match self {
            NoticeSeverity::Unknown => NoticeSeverity::Info,
            other => other,
        }
    }
}

/// Diagnostics about how the current snapshot was obtained.
///
/// **Never user-visible.** See the state-separation note on
/// [`FeatureFlagsSnapshot`]. Present on the IPC surface so a future
/// developer-tools panel (gated behind the Konami dev-access sentinel) can
/// show it, and so the values survive a round trip through the disk cache.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FetchMeta {
    /// Which tier of the resolution chain produced the verdicts.
    #[serde(default)]
    pub source: FlagSource,

    /// When the last *successful* remote fetch completed (RFC 3339).
    /// `None` when this build has never reached the server.
    #[serde(default)]
    pub fetched_at: Option<String>,

    /// How many refresh attempts have failed back-to-back since the last
    /// success. Reset to 0 on any successful fetch. Purely a log/telemetry
    /// signal — it must never gate, warn, or nag.
    #[serde(default)]
    pub consecutive_failures: u32,

    /// Detail of the most recent failure, for logs only. Never rendered.
    #[serde(default)]
    pub last_error: Option<String>,
}

/// Which tier of the three-tier resolution chain produced a snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagSource {
    /// Fetched from the remote endpoint this session.
    Remote,
    /// Loaded from the sticky on-disk cache.
    DiskCache,
    /// Compiled-in defaults (empty verdict map => everything enabled).
    #[default]
    Defaults,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled default snapshot carries no verdicts at all, which is
    /// what makes "missing key == enabled" equivalent to "everything on".
    #[test]
    fn default_snapshot_is_empty_and_defaults_sourced() {
        let snap = FeatureFlagsSnapshot::default();
        assert!(snap.verdicts.is_empty());
        assert_eq!(snap.meta.source, FlagSource::Defaults);
        assert_eq!(snap.meta.consecutive_failures, 0);
        assert!(snap.meta.last_error.is_none());
    }

    /// A verdict object that omits `enabled` must resolve to enabled —
    /// same fail-open posture as a missing key.
    #[test]
    fn verdict_without_enabled_field_defaults_to_enabled() {
        let v: FlagVerdict = serde_json::from_str("{}").unwrap();
        assert!(v.enabled);
        assert!(v.notice.is_none());
    }

    /// Unknown severity strings must not poison the parse — they land on
    /// `Unknown` and render as `Info`.
    #[test]
    fn unknown_severity_string_parses_to_unknown_and_renders_as_info() {
        let notice: FlagNotice =
            serde_json::from_str(r#"{"message":"hi","severity":"apocalyptic"}"#).unwrap();
        assert_eq!(notice.severity, NoticeSeverity::Unknown);
        assert_eq!(notice.severity.render_as(), NoticeSeverity::Info);
    }

    /// The four known severities round-trip through their lowercase wire
    /// representation and are NOT swallowed by the `other` catch-all.
    #[test]
    fn known_severities_round_trip_lowercase() {
        for (wire, expected) in [
            ("info", NoticeSeverity::Info),
            ("maintenance", NoticeSeverity::Maintenance),
            ("deprecated", NoticeSeverity::Deprecated),
            ("critical", NoticeSeverity::Critical),
        ] {
            let parsed: NoticeSeverity =
                serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, expected, "wire value {wire} should parse to {expected:?}");
            assert_eq!(serde_json::to_string(&expected).unwrap(), format!("\"{wire}\""));
            assert_eq!(parsed.render_as(), expected);
        }
    }

    /// Unknown top-level fields must be ignored, not rejected — the server
    /// will grow fields long after this build ships.
    #[test]
    fn unknown_fields_are_ignored_not_rejected() {
        let json = r#"{
            "generated_at": "2026-07-27T00:00:00Z",
            "verdicts": { "a.b": { "enabled": false, "some_future_field": 1 } },
            "some_future_top_level": { "nested": true }
        }"#;
        let snap: FeatureFlagsSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snap.verdicts.len(), 1);
        assert!(!snap.verdicts["a.b"].enabled);
    }

    /// The server may name the verdict object `features`; the alias keeps
    /// both spellings working without a client change.
    #[test]
    fn features_alias_is_accepted_for_verdicts() {
        let json = r#"{"features": {"core.updater": {"enabled": true}}}"#;
        let snap: FeatureFlagsSnapshot = serde_json::from_str(json).unwrap();
        assert!(snap.verdicts.contains_key("core.updater"));
    }
}
