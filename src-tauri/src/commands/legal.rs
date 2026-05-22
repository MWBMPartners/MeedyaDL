// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! IPC commands that surface the MeedyaDL legal / compliance documents
//! (ACKNOWLEDGEMENTS.md and THIRD_PARTY_LICENSES.md) to the frontend.
//!
//! Both files are **embedded at compile time** via `include_str!()`, so
//! they ride inside the compiled binary on every platform without
//! depending on the Tauri `bundle.resources` config or any runtime
//! resource-path probing. This is the simplest correct way to satisfy
//! the MIT/BSD/LGPL/GPL preserve-the-notice obligation that attaches
//! when MeedyaDL is redistributed (whether in tiny-installer or
//! offline-installer form) — the notices physically travel with the
//! binary that's doing the redistribution.
//!
//! Surfaced through Help > About > "Open Source Acknowledgements".
//! Resolves the pre-#802 dangling reference in
//! `src/components/help/HelpViewer.tsx` to "the ACKNOWLEDGEMENTS file
//! included with this application" — there was no IPC for it before
//! and the file wasn't in `bundle.resources` either, so the link
//! pointed at nothing.

/// Returns the verbatim contents of the repo-root `ACKNOWLEDGEMENTS.md`
/// file (the inventory of every direct dependency, its licence, and a
/// one-line purpose). Embedded at compile time — no I/O at call time.
#[tauri::command]
pub fn get_acknowledgements_text() -> String {
    include_str!("../../../ACKNOWLEDGEMENTS.md").to_string()
}

/// Returns the verbatim contents of the repo-root
/// `THIRD_PARTY_LICENSES.md` file (the actual MIT/BSD/Unlicense/LGPL/
/// GPL/PSF licence text for the bundled engines and tools, plus the
/// written offer for source code for the LGPL/GPL components shipped
/// in the offline-installer artefact). Embedded at compile time.
///
/// The frontend renders this through the same `react-markdown` pipeline
/// that `HelpViewer` uses for the help pages, so the verbatim text gets
/// proper monospace rendering for the licence blocks.
#[tauri::command]
pub fn get_third_party_licenses_text() -> String {
    include_str!("../../../THIRD_PARTY_LICENSES.md").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded `ACKNOWLEDGEMENTS.md` must be non-empty and start
    /// with the document's `# Acknowledgements` heading — a regression
    /// guard against accidentally pointing `include_str!()` at the
    /// wrong file or stripping the heading during a rewrite.
    #[test]
    fn acknowledgements_text_starts_with_expected_heading() {
        let text = get_acknowledgements_text();
        assert!(!text.is_empty(), "ACKNOWLEDGEMENTS.md should not be empty");
        assert!(
            text.starts_with("# Acknowledgements"),
            "ACKNOWLEDGEMENTS.md should start with '# Acknowledgements', got: {:?}",
            &text[..text.len().min(50)]
        );
    }

    /// The embedded `THIRD_PARTY_LICENSES.md` must include the GAMDL
    /// MIT copyright line — without it the file fails the MIT
    /// "preserve the copyright notice" obligation that #802 set out
    /// to satisfy.
    #[test]
    fn third_party_licenses_includes_gamdl_mit_copyright() {
        let text = get_third_party_licenses_text();
        assert!(!text.is_empty(), "THIRD_PARTY_LICENSES.md should not be empty");
        assert!(
            text.contains("Copyright (c) 2024 Glomatico"),
            "THIRD_PARTY_LICENSES.md must preserve the GAMDL MIT copyright line"
        );
    }

    /// The embedded `THIRD_PARTY_LICENSES.md` must include the LGPL
    /// source-offer language for FFmpeg — the obligation that turns
    /// MIT/BSD compliance into LGPL compliance for offline-installer
    /// bundles.
    #[test]
    fn third_party_licenses_includes_ffmpeg_source_offer() {
        let text = get_third_party_licenses_text();
        assert!(
            text.contains("Source Offers"),
            "THIRD_PARTY_LICENSES.md must include the LGPL/GPL Source Offers section"
        );
        assert!(
            text.to_lowercase().contains("ffmpeg"),
            "THIRD_PARTY_LICENSES.md must name FFmpeg in the source offers"
        );
    }
}
