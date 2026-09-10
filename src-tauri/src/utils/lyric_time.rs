// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Reading the time out of a lyrics or subtitle file.
//!
//! Apple Music supplies lyrics as TTML, and MeedyaDL turns that one file into
//! several other formats — word-by-word lyrics, subtitles, and two styled
//! subtitle formats. Every one of them has to read the same timestamps out of
//! the same file.
//!
//! Until this module existed, three separate copies of that reader had grown
//! up, one per output format, and they did not agree (#1158). They looked
//! identical at a glance, which is worse than looking different:
//!
//! * One trimmed spaces off the value before reading it. Two did not — and
//!   Rust refuses to read a number with a space around it, so a timestamp
//!   written as `" 00:01:30.5 "` came out as **90.5 seconds in one format and
//!   0 in the other two**. A line ninety seconds into a song jumped to the very
//!   beginning.
//! * One refused a value it could not make sense of, letting the caller skip
//!   the line. The other two quietly returned zero for the parts they could not
//!   read and carried on, so `"00:BAD:30"` became a confident 30 seconds.
//!
//! Nobody would suspect any of that, because all three read the same file. And
//! fixing one copy left the other two wrong, with nothing to say so.
//!
//! There is now one reader. It takes the stricter, trimming behaviour, because
//! that was the more correct of the two: refusing a value the caller can then
//! skip is better than inventing a number that looks real.
//!
//! # What it accepts
//!
//! TTML allows several ways of writing a time. These are the ones Apple Music
//! actually produces, plus the shorter forms that are valid and cost nothing to
//! support:
//!
//! | Written as | Means |
//! | --- | --- |
//! | `15.8s` | 15.8 seconds |
//! | `30.5` | 30.5 seconds |
//! | `02:30.5` | 2 minutes 30.5 seconds |
//! | `01:02:03.456` | 1 hour 2 minutes 3.456 seconds |
//!
//! Surrounding spaces are ignored. Anything else is refused.

/// Reads a TTML timestamp and returns the number of seconds it represents.
///
/// Spaces around the value are ignored. Anything that cannot be read in full
/// is refused rather than guessed at — if any part of it is not a number, the
/// whole value is refused, so a caller never receives a plausible-looking time
/// derived from nonsense.
///
/// # Arguments
///
/// * `time_str` -- The timestamp exactly as it appeared in the file.
///
/// # Returns
///
/// The time in seconds, or `None` when the value cannot be read.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(parse_ttml_time("01:02:03.456"), Some(3723.456));
/// assert_eq!(parse_ttml_time("  15.8s  "), Some(15.8));
/// assert_eq!(parse_ttml_time("00:BAD:30"), None);
/// ```
pub(crate) fn parse_ttml_time(time_str: &str) -> Option<f64> {
    let s = time_str.trim();

    // The "seconds" form, written as a plain number followed by an s —
    // "15.8s". This is what Apple Music uses most of the time.
    if let Some(stripped) = s.strip_suffix('s') {
        return stripped.trim().parse::<f64>().ok();
    }

    // Otherwise it is one, two or three numbers separated by colons. Each part
    // must read cleanly: a single unreadable part refuses the whole value,
    // rather than counting as zero and letting the rest through.
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        // Seconds on their own: "30.5"
        1 => parts[0].parse::<f64>().ok(),
        // Minutes and seconds: "02:30.5"
        2 => {
            let minutes: f64 = parts[0].parse().ok()?;
            let seconds: f64 = parts[1].parse().ok()?;
            Some(minutes * 60.0 + seconds)
        }
        // Hours, minutes and seconds: "01:02:03.456"
        3 => {
            let hours: f64 = parts[0].parse().ok()?;
            let minutes: f64 = parts[1].parse().ok()?;
            let seconds: f64 = parts[2].parse().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        }
        _ => None,
    }
}

/// The same reading, for callers that would rather have a number than a choice.
///
/// Two of the three original copies returned a plain number and treated
/// anything unreadable as the start of the file. That behaviour is kept for
/// them so this change does not alter what a working file produces — but it now
/// sits on top of the stricter reader, so a value with spaces around it is read
/// properly instead of collapsing to zero, and a partly-unreadable value gives
/// zero rather than a confident wrong number.
///
/// Prefer [`parse_ttml_time`] in new code: a caller that can skip a line it
/// cannot read will produce a better file than one that silently places it at
/// the beginning.
///
/// # Arguments
///
/// * `time_str` -- The timestamp exactly as it appeared in the file.
///
/// # Returns
///
/// The time in seconds, or `0.0` when the value cannot be read.
pub(crate) fn parse_ttml_time_or_zero(time_str: &str) -> f64 {
    parse_ttml_time(time_str).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How close two times have to be to count as the same. Well below
    /// anything a person could notice, and far looser than the arithmetic
    /// error, so this cannot fail for rounding reasons alone.
    const CLOSE_ENOUGH: f64 = 1e-9;

    fn assert_seconds(input: &str, expected: f64) {
        let got = parse_ttml_time(input).unwrap_or_else(|| panic!("{input:?} should be readable"));
        assert!(
            (got - expected).abs() < CLOSE_ENOUGH,
            "{input:?} gave {got}, expected {expected}"
        );
    }

    #[test]
    fn reads_the_forms_apple_music_produces() {
        assert_seconds("01:02:03.456", 3723.456);
        assert_seconds("02:30.500", 150.5);
        assert_seconds("15.8s", 15.8);
        assert_seconds("30.5", 30.5);
        assert_seconds("0", 0.0);
        assert_seconds("00:00:00.000", 0.0);
    }

    // ── The disagreements this module exists to end (#1158) ──────────────

    #[test]
    fn spaces_around_the_value_no_longer_move_a_lyric_to_the_start() {
        // The important one. Rust refuses to read a number with a space
        // around it, so the two copies that did not trim turned a padded
        // timestamp into zero — a line ninety seconds in jumped to the very
        // beginning of the song, in some output formats but not others.
        assert_seconds(" 00:01:30.500 ", 90.5);
        assert_seconds("\t02:30.500\n", 150.5);
        assert_seconds("  15.8s  ", 15.8);
        assert_seconds(" 30.5 ", 30.5);
    }

    #[test]
    fn proof_that_untrimmed_input_really_did_break() {
        // Guards the reasoning above rather than the code: if Rust ever
        // started accepting surrounding spaces, the trimming would be
        // unnecessary and this test would say so.
        assert!(
            " 00".parse::<f64>().is_err(),
            "Rust now accepts leading spaces — the trimming may be redundant"
        );
        assert!(
            "30.5 ".parse::<f64>().is_err(),
            "Rust now accepts trailing spaces — the trimming may be redundant"
        );
    }

    #[test]
    fn a_partly_unreadable_value_is_refused_rather_than_half_read() {
        // Two of the three copies counted an unreadable part as zero and
        // carried on, so this came out as a confident 30 seconds. A number
        // that looks real but was never in the file is worse than no number.
        assert_eq!(parse_ttml_time("00:BAD:30"), None);
        assert_eq!(parse_ttml_time("1:2:x"), None);
        assert_eq!(parse_ttml_time("::"), None);
    }

    #[test]
    fn nonsense_is_refused() {
        assert_eq!(parse_ttml_time("garbage"), None);
        assert_eq!(parse_ttml_time(""), None);
        assert_eq!(parse_ttml_time("   "), None);
        assert_eq!(parse_ttml_time("s"), None);
        // More parts than a time can have.
        assert_eq!(parse_ttml_time("1:2:3:4"), None);
    }

    #[test]
    fn the_lenient_wrapper_gives_zero_only_when_the_value_is_unreadable() {
        // Existing callers keep their tolerance, but now benefit from the
        // trimming underneath.
        assert!((parse_ttml_time_or_zero(" 00:01:30.500 ") - 90.5).abs() < CLOSE_ENOUGH);
        assert!((parse_ttml_time_or_zero("garbage") - 0.0).abs() < CLOSE_ENOUGH);
        assert!((parse_ttml_time_or_zero("00:BAD:30") - 0.0).abs() < CLOSE_ENOUGH);
    }

    #[test]
    fn minutes_and_seconds_may_exceed_their_usual_range() {
        // TTML does not require normalised values, and a long track can
        // legitimately carry a minutes field above 59.
        assert_seconds("90:00.000", 5400.0);
        assert_seconds("00:90.000", 90.0);
    }
}
