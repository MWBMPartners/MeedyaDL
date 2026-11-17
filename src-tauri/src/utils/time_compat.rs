// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Date and time helpers (#1449).
// ==============================
//
// MeedyaDL used to use the `chrono` library for dates; it was replaced by
// `time` because of security advisory RUSTSEC-2026-0543. Every date the app
// STORES or COMPARES is a plain piece of text (history, crash reports, the
// download index, caches), and some of that text is compared as text
// (sorted, or `timestamp >= ?` in SQL). So the text written now has to look
// exactly like the text older versions wrote. This file is the one place
// that writes dates in those forms and reads them back; nothing else in the
// app should format a date by hand or call the `time` library's own
// RFC 3339 writer, which would write `Z` and a different number of decimals.
//
// The stored forms (unchanged from the `chrono` days):
//   * Stored moment:  2026-11-17T04:12:35.123456789+00:00
//       - always UTC, always written "+00:00" (never "Z")
//       - no decimals when the nanoseconds are 0; 3 decimals when they are a
//         whole number of milliseconds; 6 when a whole number of
//         microseconds; otherwise 9.
//   * Day:            2026-11-17
//   * File stamp:     20261117-041235
//   * Tag stamp:      2026-11-17T04:12:35Z   (MeedyaMeta:LastModified)
//
// Reading old data: every form older versions wrote is read back by
// `parse_rfc3339` / `parse_rfc3339_lenient`.

use std::fmt::Write as _;

use time::format_description::well_known::{Rfc2822, Rfc3339};
use time::{Date, Month, OffsetDateTime, UtcOffset};

/// The current moment, in UTC.
pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// The current moment in the computer's own time zone.
///
/// If the computer's offset from UTC cannot be read (it can fail on some
/// systems), UTC is used instead and a debug line is logged. That only ever
/// changes a label, a session-file name or the day a daily counter rolls
/// over, never stored history.
pub fn now_local_or_utc() -> OffsetDateTime {
    match OffsetDateTime::now_local() {
        Ok(t) => t,
        Err(e) => {
            log::debug!("local time offset unavailable ({e}); using UTC");
            OffsetDateTime::now_utc()
        }
    }
}

/// The same moment in UTC, or `None` when it cannot be shown in UTC.
///
/// `time` only goes up to the year 9999, so a moment such as
/// `9999-12-31T23:30:00-01:00` has no UTC form; its own `to_offset` panics
/// on it. Every conversion to UTC in the app goes through here instead, so
/// a hostile or broken date is treated as unreadable rather than ending
/// the task (review of #1449).
pub fn to_utc(dt: OffsetDateTime) -> Option<OffsetDateTime> {
    dt.checked_to_offset(UtcOffset::UTC)
}

/// The UTC moment for writing, falling back to the latest moment `time`
/// can hold when the true one is past it. Callers that must not write a
/// made-up date (the last-updated tag) check [`to_utc`] first; only
/// `parse_rfc3339_lenient` refuses such dates by itself.
fn utc_for_writing(dt: OffsetDateTime) -> OffsetDateTime {
    to_utc(dt).unwrap_or_else(|| {
        log::debug!("date {dt} is past the year 9999 in UTC; writing the latest date instead");
        time::PrimitiveDateTime::MAX.assume_utc()
    })
}

/// Write a moment in the exact stored form described at the top of this
/// file (the same text `chrono`'s `to_rfc3339()` wrote).
pub fn to_stored_rfc3339(dt: OffsetDateTime) -> String {
    let dt = utc_for_writing(dt);
    let mut out = String::with_capacity(35);
    let year = dt.year();
    // Years 0..=9999 are four digits; anything else is written with an
    // explicit sign, as chrono did (not reachable for "now").
    if (0..=9999).contains(&year) {
        let _ = write!(out, "{year:04}");
    } else {
        let _ = write!(out, "{year:+05}");
    }
    let _ = write!(
        out,
        "-{:02}-{:02}T{:02}:{:02}:{:02}",
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    );
    let nanos = dt.nanosecond();
    if nanos != 0 {
        if nanos.is_multiple_of(1_000_000) {
            let _ = write!(out, ".{:03}", nanos / 1_000_000);
        } else if nanos.is_multiple_of(1_000) {
            let _ = write!(out, ".{:06}", nanos / 1_000);
        } else {
            let _ = write!(out, ".{nanos:09}");
        }
    }
    out.push_str("+00:00");
    out
}

/// "Now" as stored text (UTC, stored form).
pub fn now_rfc3339() -> String {
    to_stored_rfc3339(now_utc())
}

/// Read a standard RFC 3339 date (`2026-11-17T04:12:35Z`,
/// `…+05:30`, with or without decimals). `None` if it is not one.
pub fn parse_rfc3339(s: &str) -> Option<OffsetDateTime> {
    // `time` accepts any character between the date and the time; the old
    // library took only `T`, `t` or a space, so refuse anything else.
    if !matches!(s.as_bytes().get(10), Some(b'T' | b't' | b' ')) {
        return None;
    }
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

/// Like [`parse_rfc3339`] but also accepts the looser spellings the old
/// library's text-to-date conversion took: surrounding spaces, a trailing
/// `UTC`, and an offset written without a colon (`+0000`). Used on our own
/// stored text (history, caches). The result is in UTC.
pub fn parse_rfc3339_lenient(s: &str) -> Option<OffsetDateTime> {
    if let Some(dt) = parse_rfc3339(s) {
        return to_utc(dt);
    }
    let mut t = s.trim().to_string();
    // "... UTC" / "...UTC" -> "+00:00"
    if let Some(stripped) = t.strip_suffix("UTC") {
        t = format!("{}+00:00", stripped.trim_end());
    }
    // "+HHMM" / "-HHMM" at the very end -> "+HH:MM"
    let bytes = t.as_bytes();
    if bytes.len() >= 5 {
        let tail = &bytes[bytes.len() - 5..];
        if (tail[0] == b'+' || tail[0] == b'-') && tail[1..].iter().all(u8::is_ascii_digit) {
            let cut = t.len() - 5;
            t = format!(
                "{}{}{}:{}",
                &t[..cut],
                tail[0] as char,
                &t[cut + 1..cut + 3],
                &t[cut + 3..]
            );
        }
    }
    parse_rfc3339(&t).and_then(to_utc)
}

/// Read an RFC 2822 date such as an HTTP `Last-Modified` header
/// (`Fri, 18 Sep 2026 19:34:47 GMT`).
///
/// The `time` library does not check that a written weekday matches the
/// date, but the old library did (and a wrong weekday is a sign of a broken
/// value), so that check is done here.
pub fn parse_rfc2822(s: &str) -> Option<OffsetDateTime> {
    let dt = OffsetDateTime::parse(s, &Rfc2822).ok()?;
    if let Some((day_name, _)) = s.trim_start().split_once(',') {
        let written = match day_name.trim().to_ascii_lowercase().as_str() {
            "mon" => time::Weekday::Monday,
            "tue" => time::Weekday::Tuesday,
            "wed" => time::Weekday::Wednesday,
            "thu" => time::Weekday::Thursday,
            "fri" => time::Weekday::Friday,
            "sat" => time::Weekday::Saturday,
            "sun" => time::Weekday::Sunday,
            _ => return None,
        };
        if dt.weekday() != written {
            return None;
        }
    }
    Some(dt)
}

/// Read a date-only text `YYYY-MM-DD` (month and day may be one or two
/// digits, nothing may follow) as midnight UTC. Matches what the old
/// library accepted for Apple Music's date-only values.
pub fn parse_date_only_utc(s: &str) -> Option<OffsetDateTime> {
    let mut parts = s.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let digits = |p: &str, max: usize| {
        !p.is_empty() && p.len() <= max && p.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(y, 4) || !digits(m, 2) || !digits(d, 2) {
        return None;
    }
    let month = Month::try_from(m.parse::<u8>().ok()?).ok()?;
    let date =
        Date::from_calendar_date(y.parse::<i32>().ok()?, month, d.parse::<u8>().ok()?).ok()?;
    Some(date.midnight().assume_utc())
}

/// `YYYY-MM-DD`, in the moment's own offset.
pub fn ymd(dt: OffsetDateTime) -> String {
    ymd_of_date(dt.date())
}

/// `YYYY-MM-DD` of a calendar date.
pub fn ymd_of_date(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

/// `YYYYMMDD-HHMMSS` (crash file names, backup snapshot folders).
pub fn compact_stamp(dt: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

/// `YYYY-MM-DD HH:MM:SS` (export headers).
pub fn ymd_hms(dt: OffsetDateTime) -> String {
    format!("{} {}", ymd(dt), hms(dt))
}

/// `HH:MM:SS` in the moment's own offset.
pub fn hms(dt: OffsetDateTime) -> String {
    format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second())
}

/// The `…_YYYY-MM-DD_HHhMMm.log` ending used in export file names.
pub fn export_stamp(dt: OffsetDateTime) -> String {
    format!("{}_{:02}h{:02}m", ymd(dt), dt.hour(), dt.minute())
}

/// `YYYY-MM-DDTHH:MM:SSZ` in UTC, whole seconds (the `MeedyaMeta:LastModified`
/// tag form).
pub fn to_tag_utc_seconds(dt: OffsetDateTime) -> String {
    let dt = utc_for_writing(dt);
    format!("{}T{}Z", ymd(dt), hms(dt))
}

#[cfg(test)]
mod tests {
    //! The expected strings below are fixed text. While `chrono` was still
    //! in the build, a differential test proved this module produces the
    //! same output as `chrono` over 20,000 moments from 1970 to 2100 and a
    //! corpus of date texts (#1449); these tests keep the results pinned.
    use super::*;

    /// 2026-11-17 04:12:35 UTC with the given nanoseconds.
    fn moment(nanos: u32) -> OffsetDateTime {
        Date::from_calendar_date(2026, Month::November, 17)
            .unwrap()
            .with_hms_nano(4, 12, 35, nanos)
            .unwrap()
            .assume_utc()
    }

    #[test]
    fn stored_form_golden() {
        // (nanoseconds, exact text). `.100` keeps three digits, not `.1`.
        for (nanos, want) in [
            (0, "2026-11-17T04:12:35+00:00"),
            (123_000_000, "2026-11-17T04:12:35.123+00:00"),
            (100_000_000, "2026-11-17T04:12:35.100+00:00"),
            (123_456_000, "2026-11-17T04:12:35.123456+00:00"),
            (1_000, "2026-11-17T04:12:35.000001+00:00"),
            (123_456_789, "2026-11-17T04:12:35.123456789+00:00"),
            (1, "2026-11-17T04:12:35.000000001+00:00"),
        ] {
            assert_eq!(to_stored_rfc3339(moment(nanos)), want);
        }
    }

    #[test]
    fn stored_form_is_utc_whatever_the_offset() {
        let t = parse_rfc3339("2026-11-17T06:12:35+02:00").unwrap();
        assert_eq!(to_stored_rfc3339(t), "2026-11-17T04:12:35+00:00");
    }

    #[test]
    fn year_zero_golden() {
        let t = Date::from_calendar_date(0, Month::January, 1)
            .unwrap()
            .midnight()
            .assume_utc();
        assert_eq!(to_stored_rfc3339(t), "0000-01-01T00:00:00+00:00");
    }

    #[test]
    fn stored_text_sorts_like_time() {
        // History, crash reports and the SQL `timestamp >= ?` compare the
        // text, so it must sort in time order.
        let a = to_stored_rfc3339(moment(0));
        let b = to_stored_rfc3339(moment(500_000_000));
        let c = to_stored_rfc3339(moment(0) + time::Duration::minutes(1));
        assert!(a < b && b < c);
    }

    #[test]
    fn short_formats_golden() {
        let t = moment(789);
        assert_eq!(ymd(t), "2026-11-17");
        assert_eq!(compact_stamp(t), "20261117-041235");
        assert_eq!(ymd_hms(t), "2026-11-17 04:12:35");
        assert_eq!(hms(t), "04:12:35");
        assert_eq!(export_stamp(t), "2026-11-17_04h12m");
        assert_eq!(to_tag_utc_seconds(t), "2026-11-17T04:12:35Z");
    }

    #[test]
    fn hms_uses_the_moments_own_offset() {
        let t = parse_rfc3339("2026-11-17T04:12:35-05:30").unwrap();
        assert_eq!(hms(t), "04:12:35");
        assert_eq!(ymd(t), "2026-11-17");
        assert_eq!(to_tag_utc_seconds(t), "2026-11-17T09:42:35Z");
    }

    #[test]
    fn single_digit_fields_are_padded() {
        let t = Date::from_calendar_date(2026, Month::January, 5)
            .unwrap()
            .with_hms(3, 4, 5)
            .unwrap()
            .assume_utc();
        assert_eq!(compact_stamp(t), "20260105-030405");
        assert_eq!(ymd(t), "2026-01-05");
    }

    /// Every form older versions wrote (and Apple / GitHub send) reads back
    /// to the same instant.
    #[test]
    fn reads_old_and_external_forms() {
        let want = moment(0).unix_timestamp();
        for s in [
            "2026-11-17T04:12:35+00:00",
            "2026-11-17T04:12:35Z",
            "2026-11-17T04:12:35z",
            "2026-11-17T06:12:35+02:00",
            "2026-11-17T04:12:35.000+00:00",
            "2026-11-17T04:12:35.000000000Z",
        ] {
            assert_eq!(parse_rfc3339(s).unwrap().unix_timestamp(), want, "{s}");
        }
        let with_fraction = parse_rfc3339("2026-11-17T04:12:35.123456789+00:00").unwrap();
        assert_eq!(with_fraction.nanosecond(), 123_456_789);
        // Round trip.
        assert_eq!(
            to_stored_rfc3339(parse_rfc3339("2026-11-17T04:12:35.123456+00:00").unwrap()),
            "2026-11-17T04:12:35.123456+00:00"
        );
    }

    #[test]
    fn rejects_what_is_not_a_date() {
        for s in [
            "",
            "2026-11-17",
            "garbage",
            "2026-11-17T04:12:35Zjunk",
            "2026-11-17T04:12:35+0000",
        ] {
            assert!(parse_rfc3339(s).is_none(), "{s:?}");
        }
    }

    #[test]
    fn a_date_past_9999_in_utc_is_unreadable_not_a_crash() {
        // `time` cannot hold the UTC form of these; they must come back as
        // "unreadable" (or the latest date when writing), never a panic.
        assert!(parse_rfc3339_lenient("9999-12-31T23:30:00-01:00").is_none());
        let late = parse_rfc3339("9999-12-31T23:30:00-01:00").unwrap();
        assert!(to_utc(late).is_none());
        assert_eq!(
            to_stored_rfc3339(late),
            "9999-12-31T23:59:59.999999999+00:00"
        );
        assert_eq!(to_tag_utc_seconds(late), "9999-12-31T23:59:59Z");
    }

    #[test]
    fn only_t_or_space_separates_date_and_time() {
        assert!(parse_rfc3339("2026-11-17X04:12:35Z").is_none());
        assert!(parse_rfc3339("2026-11-17T04:12:35Z").is_some());
    }

    /// A leap second written as `:60` is refused. The old library accepted
    /// `:60` at any minute; it never appears in our data or Apple's.
    #[test]
    fn leap_second_is_refused() {
        assert!(parse_rfc3339("2026-11-17T04:12:60Z").is_none());
    }

    #[test]
    fn lenient_reads_the_looser_spellings() {
        let want = moment(0).unix_timestamp();
        for s in [
            "2026-11-17T04:12:35Z",
            "2026-11-17T04:12:35+00:00",
            "2026-11-17T04:12:35+0000",
            "2026-11-17 04:12:35 UTC",
            "2026-11-17T04:12:35 UTC",
            "2026-11-17 04:12:35+0000",
            "2026-11-17 04:12:35Z",
            "  2026-11-17T04:12:35Z  ",
        ] {
            assert_eq!(
                parse_rfc3339_lenient(s).unwrap().unix_timestamp(),
                want,
                "{s:?}"
            );
        }
        for s in ["garbage", "", "2026-11-17"] {
            assert!(parse_rfc3339_lenient(s).is_none(), "{s:?}");
        }
    }

    #[test]
    fn rfc2822_golden() {
        let want = 1_789_760_087; // Fri, 18 Sep 2026 19:34:47 UTC
        for s in [
            "Fri, 18 Sep 2026 19:34:47 GMT",
            "Fri, 18 Sep 2026 19:34:47 +0000",
            "Fri, 18 Sep 2026 12:34:47 -0700",
            "Fri, 18 Sep 2026 19:34:47 UT",
            "18 Sep 2026 19:34:47 GMT",
        ] {
            assert_eq!(parse_rfc2822(s).unwrap().unix_timestamp(), want, "{s}");
        }
        // A weekday that does not match the date is refused.
        assert!(parse_rfc2822("Mon, 18 Sep 2026 19:34:47 GMT").is_none());
        assert!(parse_rfc2822("garbage").is_none());
    }

    /// The tag form written to MeedyaMeta:LastModified, for every input shape
    /// Apple sends.
    #[test]
    fn last_modified_tag_golden() {
        let norm = |raw: &str| {
            let utc = match parse_rfc3339(raw) {
                Some(dt) => dt,
                None => parse_date_only_utc(raw)?,
            };
            Some(to_tag_utc_seconds(utc))
        };
        for (raw, want) in [
            ("2026-11-17T04:12:35Z", Some("2026-11-17T04:12:35Z")),
            ("2026-11-17T04:12:35.000Z", Some("2026-11-17T04:12:35Z")),
            ("2026-11-17T06:12:35+02:00", Some("2026-11-17T04:12:35Z")),
            ("2026-11-17", Some("2026-11-17T00:00:00Z")),
            ("2026-1-5", Some("2026-01-05T00:00:00Z")),
            ("2026-11-17x", None),
            ("2026-13-01", None),
            ("2026-11-17-01", None),
            ("garbage", None),
            ("", None),
        ] {
            assert_eq!(norm(raw).as_deref(), want, "{raw:?}");
        }
    }

    #[test]
    fn system_time_keeps_nanoseconds() {
        let a = parse_rfc3339("2026-11-17T04:12:35.123456789+00:00")
            .map(std::time::SystemTime::from)
            .unwrap();
        let b = std::time::UNIX_EPOCH + std::time::Duration::new(1_794_888_755, 123_456_789);
        assert_eq!(a, b);
    }

    #[test]
    fn now_helpers_work() {
        assert!(now_rfc3339().ends_with("+00:00"));
        // Whatever the offset, a local moment is the same instant as UTC.
        let d = (now_local_or_utc() - now_utc()).abs();
        assert!(d < time::Duration::seconds(5));
    }

    /// Only this file may use the library's own RFC 3339 writer, which would
    /// write `Z` and different decimals and break text ordering (#1449).
    #[test]
    fn nobody_else_uses_the_library_rfc3339_writer() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        for f in files {
            if f.ends_with("utils/time_compat.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            assert!(
                !text.contains("well_known::Rfc3339"),
                "{} uses time's own RFC 3339 writer; use time_compat",
                f.display()
            );
        }
    }
}
