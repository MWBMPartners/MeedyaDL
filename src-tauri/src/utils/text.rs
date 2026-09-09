// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Cutting text short without crashing on ordinary punctuation.
// ===============================================================
//
// A Rust `String` is stored as bytes, not as one slot per character. Most
// characters used in everyday English text take one byte each, but a
// curly quote, a long dash, an accented letter, or almost anything outside
// plain ASCII takes two, three, or four bytes. If you cut the string at a
// fixed byte position — `&s[..200]`, `s.truncate(N)` — and that position
// happens to land in the middle of one of those multi-byte characters,
// Rust refuses to produce the broken half-character and stops the whole
// program instead.
//
// This is not a rare edge case. Apple Music album notes, GitHub release
// notes, and lyrics files all use curly quotes and long dashes routinely,
// so cutting them at a fixed byte count will eventually land mid-character
// and crash — not on malicious input, just on ordinary writing.
//
// The fix is always the same shape: walk backwards from the cut point,
// one byte at a time, until we land on a position that is the START of a
// character (byte 0 always is, so this can never walk off the front of
// the string). `str::is_char_boundary` tells us exactly that. The same
// fix already exists in two other places in this codebase
// (`commands::settings::sanitize_imported_settings`'s inner `truncate`
// helper, and `utils::bounded_log::BoundedLineBuffer::push`) — this module
// exists so new call sites can reuse the same logic instead of
// hand-rolling it a fourth, fifth, sixth time.

/// Returns the longest prefix of `s` that is at most `max_bytes` bytes
/// long and ends on a whole character.
///
/// If `s` is already within the limit, it is returned unchanged. Otherwise
/// the cut point is walked back (at most 3 bytes, since UTF-8 characters
/// are never more than 4 bytes) to the nearest character boundary at or
/// before `max_bytes`, so the result is always valid UTF-8 — never a
/// half-written character.
#[must_use]
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut cut = max_bytes;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    &s[..cut]
}

/// Shortens an owned `String` in place to at most `max_bytes` bytes,
/// cutting at the nearest whole character rather than a fixed byte
/// position. Prefer [`truncate_str`] when you only need a borrowed view;
/// use this when the caller already owns a growable `String` and wants
/// it shortened without allocating a new one.
pub fn truncate_string_in_place(s: &mut String, max_bytes: usize) {
    if s.len() > max_bytes {
        let mut cut = max_bytes;
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
}

#[cfg(test)]
mod tests {
    use super::{truncate_str, truncate_string_in_place};

    #[test]
    fn short_text_is_returned_unchanged() {
        assert_eq!(truncate_str("hello", 200), "hello");
    }

    #[test]
    fn cutting_mid_multibyte_character_does_not_panic() {
        // 400 euro signs is 1200 bytes (3 bytes each). Cutting at byte
        // 1024 the naive way lands inside the 342nd euro sign. This must
        // not panic, and the result must be valid UTF-8 no matter what.
        let s = "€".repeat(400);
        let cut = truncate_str(&s, 1024);
        assert!(cut.len() <= 1024);
        // Every returned byte sequence must itself be valid UTF-8 — if we
        // had cut mid-character, building this `String` would panic.
        let _ = cut.to_string();
    }

    #[test]
    fn result_never_exceeds_the_requested_length() {
        let s = "a—b—c—d—e"; // long dashes are 3 bytes each in UTF-8
        for max in 0..s.len() {
            let cut = truncate_str(s, max);
            assert!(cut.len() <= max);
        }
    }

    #[test]
    fn in_place_variant_matches_the_borrowed_one() {
        let mut owned = "€".repeat(400);
        let borrowed_expected = truncate_str(&"€".repeat(400), 1024).to_string();
        truncate_string_in_place(&mut owned, 1024);
        assert_eq!(owned, borrowed_expected);
    }

    #[test]
    fn in_place_variant_leaves_short_strings_untouched() {
        let mut owned = "hello".to_string();
        truncate_string_in_place(&mut owned, 200);
        assert_eq!(owned, "hello");
    }
}
