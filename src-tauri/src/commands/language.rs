// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// src-tauri/src/commands/language.rs
//
// Language-list IPC commands (#1249)
// ==================================
//
// One command: `order_languages_for_display`, which puts the entries of a
// language list into the order a person should see them, per the shared
// language policy (docs/standards/media-language-bcp47-policy.md, UI-020 to
// UI-040). Its callers today are the Metadata Language list and the
// Interface Language list, both in Settings > General — the Interface
// Language list joined as of the independent review, round 3 of #1244; it
// used to just follow `LOCALES`' own array order, whatever language the
// interface was actually showing.
//
// Why the ordering is done here and not in JavaScript: the rules are
// implemented once, in the shared `meedya-lang` crate, and the policy's test
// cases check that code (src-tauri/tests/media_language_conformance.rs). A
// second copy in JavaScript would be a copy nothing checks. The frontend
// supplies only what the crate cannot know — names and alphabetical order in
// the interface language, which only the platform's own locale data can
// work out correctly (`Intl.DisplayNames`, `Intl.Collator`).

/// The most entries, preferences or language codes accepted in one call.
/// The real list has about a dozen entries; this bound only stops a
/// malformed call from making the backend sort something enormous.
const MAX_LIST_LEN: usize = 500;

/// The longest single value accepted. Far above any real language tag (the
/// app stores at most 35 characters); again only a bound against a
/// malformed call.
const MAX_VALUE_LEN: usize = 256;

/// Orders a language list for display (#1249).
///
/// **Frontend caller:** `orderLanguagesForDisplay(tags, preferences,
/// alphabeticalPrimaryOrder)` in `src/lib/tauri-commands.ts`, used by
/// `useMetadataLanguageOptions` and `useInterfaceLanguageOptions`.
///
/// # Arguments
/// * `tags` — the list entries, exactly as stored. They come back exactly
///   as given, only reordered: nothing is dropped, added or rewritten, so a
///   saved value that is not a tag is still shown (last, per LANG-026).
/// * `preferences` — the person's languages, highest priority first (the
///   interface language, then the system's preferred languages). Their
///   language groups come first (UI-020); values that are not tags are
///   ignored.
/// * `alphabetical_primary_order` — primary language codes in alphabetical
///   order of their names in the interface language (UI-040), worked out by
///   the frontend with `Intl.Collator`.
///
/// # Errors
/// Returns `Err` only for a call that is too large (see [`MAX_LIST_LEN`]
/// and [`MAX_VALUE_LEN`]). The frontend then shows the list alphabetically
/// by name instead, so a failure never leaves the list empty.
///
/// Deliberately takes no "selected" value (policy UI-050): choosing an
/// entry must never move it.
#[tauri::command]
pub fn order_languages_for_display(
    tags: Vec<String>,
    preferences: Vec<String>,
    alphabetical_primary_order: Vec<String>,
) -> Result<Vec<String>, String> {
    for (name, list) in [
        ("tags", &tags),
        ("preferences", &preferences),
        ("alphabetical_primary_order", &alphabetical_primary_order),
    ] {
        if list.len() > MAX_LIST_LEN {
            return Err(format!(
                "Too many {name} to order ({}; at most {MAX_LIST_LEN})",
                list.len()
            ));
        }
        if list.iter().any(|value| value.len() > MAX_VALUE_LEN) {
            return Err(format!(
                "A value in {name} is longer than {MAX_VALUE_LEN} characters"
            ));
        }
    }
    Ok(crate::utils::language::order_for_display(
        &tags,
        &preferences,
        &alphabetical_primary_order,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn orders_the_list_and_returns_every_entry_unchanged() {
        let tags = strings(&["ru-RU", "en-US", "zh-CN", "de-DE", "en-GB"]);
        let got = order_languages_for_display(
            tags.clone(),
            strings(&["de", "en-GB"]),
            // English names: Chinese, English, German, Russian.
            strings(&["zh", "en", "de", "ru"]),
        )
        .unwrap();
        // German first (first preference), then English (second, with the
        // exact preference en-GB ahead of en-US), then the rest by name.
        assert_eq!(got, ["de-DE", "en-GB", "en-US", "zh-CN", "ru-RU"]);
        let mut sorted_in = tags;
        sorted_in.sort();
        let mut sorted_out = got;
        sorted_out.sort();
        assert_eq!(sorted_in, sorted_out, "nothing dropped, added or rewritten");
    }

    #[test]
    fn refuses_a_call_that_is_too_large() {
        let too_many = vec!["en".to_string(); MAX_LIST_LEN + 1];
        assert!(order_languages_for_display(too_many, vec![], vec![]).is_err());
        let too_long = vec!["a".repeat(MAX_VALUE_LEN + 1)];
        assert!(order_languages_for_display(vec![], too_long, vec![]).is_err());
    }
}
