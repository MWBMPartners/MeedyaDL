// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Deciding whether a build of this app is finished or unfinished.
//!
//! # Why this is its own file
//!
//! The app asks this question in several places and for different
//! reasons — whether to keep verbose logging on, whether to show the
//! "you are running an unfinished build" notice, whether to offer a
//! return to the last finished release. Each place had written the rule
//! out for itself.
//!
//! That is how it went wrong. The rule used to be "does the version
//! start with `0.`", which was a complete answer while the app was
//! pre-1.0 and has been wrong every day since 1.0 shipped: every alpha,
//! beta and release-candidate build has been treated as finished. So
//! verbose logging was switched off at every startup for exactly the
//! people who most need it, the testers, and the notice telling someone
//! they are on an unfinished build never appeared at all. Issue #216,
//! reopened, and issue #387.
//!
//! One copy of the rule was then fixed and the others were not, which is
//! the shape of fault this project keeps finding: a rule that guards one
//! door and not the one beside it. So it lives here, once, and the
//! places that need it call it.

/// Is this version of the app an unfinished build?
///
/// Two things make a build unfinished, and **both halves matter**:
///
/// * the version starts with `0.` — anything before 1.0 is by
///   definition not finished; or
/// * the version carries a suffix after a dash — `1.13.0-alpha.71`,
///   `1.9.4-beta.7`, `1.0.0-rc.38`.
///
/// A first attempt at fixing the old rule asked only the second
/// question, which quietly dropped the pre-1.0 case: a `0.49.2` build
/// with no suffix would have counted as finished. The maintainer caught
/// that. Ask only one of the two and the rule is wrong in one direction
/// or the other; this is why the tests below check both.
///
/// # What this deliberately does not do
///
/// It does not work out *which* kind of unfinished build it is — alpha,
/// beta or release candidate. Nothing here needs that: the channel the
/// person chose is stored in their settings, and the update checker
/// reads the channel out of a release tag separately. Answering a
/// narrower question here would mean a second rule to keep in step with
/// this one, which is the problem this file exists to end.
pub fn is_unfinished_build(version: &str) -> bool {
    version.starts_with("0.") || version.contains('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_released_build_is_finished() {
        for finished in ["1.0.0", "1.10.8", "2.4.17", "10.0.1"] {
            assert!(
                !is_unfinished_build(finished),
                "{finished} is a finished release"
            );
        }
    }

    #[test]
    fn every_kind_of_suffix_means_unfinished() {
        // The real shapes this project cuts, taken from the four channel
        // branches rather than invented.
        for unfinished in [
            "1.13.0-alpha.71",
            "1.9.4-beta.7",
            "1.0.0-rc.38",
            "2.0.0-nightly.3",
        ] {
            assert!(
                is_unfinished_build(unfinished),
                "{unfinished} is an unfinished build"
            );
        }
    }

    #[test]
    fn anything_before_one_is_unfinished_even_with_no_suffix() {
        // The half a first attempt at this fix dropped. A `0.x` build
        // has no suffix to give it away, so asking only about the suffix
        // would call it finished.
        for early in ["0.1.0", "0.38.0", "0.49.2"] {
            assert!(
                is_unfinished_build(early),
                "{early} comes before 1.0, so it is not finished"
            );
        }
    }

    #[test]
    fn this_very_build_agrees_with_the_rule() {
        // Guards against the rule and the app drifting apart. Whatever
        // version this is compiled at, the answer has to follow from the
        // version string itself and nothing else.
        let this_build = env!("CARGO_PKG_VERSION");
        assert_eq!(
            is_unfinished_build(this_build),
            this_build.starts_with("0.") || this_build.contains('-'),
            "the rule must depend only on the version string"
        );
    }

    #[test]
    fn a_version_that_makes_no_sense_is_treated_as_finished() {
        // Nothing should ever hand this an empty or malformed string,
        // but if something does, the safe answer is "finished": a
        // finished build switches verbose logging OFF, and verbose logs
        // can hold cookies and tokens. Guessing "unfinished" here would
        // leave them on.
        for nonsense in ["", "not a version", "v1.2.3"] {
            assert!(
                !is_unfinished_build(nonsense),
                "{nonsense:?} cannot be shown to be unfinished, so it is treated as finished"
            );
        }
    }
}
