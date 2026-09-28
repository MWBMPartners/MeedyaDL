---
name: project-media-language-policy
description: MeedyaDL follows the shared Media Language & BCP 47 Policy (MWBM-MEDIA-LANG 1.0.0) — where the copies, the checker, the shared crate and MeedyaDL's own language code live, what is pinned where, and what was decided (lyric files are not renamed)
metadata:
  type: project
---

# The shared language policy (MWBM-MEDIA-LANG)

**What it is.** One rulebook, shared by every MWBM / MeedyaSuite app, for how
languages are identified, stored, ordered, named, matched and chosen — for
audio and subtitle tracks, lyrics, translations and metadata. The master lives
in MWBMPartners/MeedyaSuite-core. MeedyaDL keeps an exact copy at
`docs/standards/media-language-bcp47-policy.md`; that copy is normative. Read
it before touching anything about languages — do not rely on this note for the
rules, which is exactly how copies drift.

**Which parts apply to MeedyaDL** (policy section 2): *canonical* (what is
stored or written), *text* (lyrics), and *presentation* only for MeedyaDL's own
settings lists. MeedyaDL does not reorder or mux tracks itself — GAMDL and
yt-dlp do.

**Where things are (set up 28 Sept 2026, umbrella issue #1244):**

- Copies of the six master files, at the same paths as in core:
  `docs/standards/` (policy, reference data and schema),
  `tests/fixtures/` (the test cases and schema),
  `scripts/media-lang/check_copies.py` (the checker). Pinned by
  `docs/standards/MWBM-MEDIA-LANG.lock`, written by the checker. `-text` in
  `.gitattributes` so no line-ending conversion changes a byte. **Never edit a
  copy** — change the master, then `python3 scripts/media-lang/check_copies.py
  --update <commit>`.
- CI: the `Language policy copies` job in `.github/workflows/ci.yml` runs the
  checker (online, with `GITHUB_TOKEN` so GitHub's lookups are not
  rate-limited). Locally: `GITHUB_TOKEN="$(gh auth token)" python3
  scripts/media-lang/check_copies.py`.
- The rules are implemented once, in core's crate `meedya-lang`, pinned by
  `rev` in `src-tauri/Cargo.toml`.
- `src-tauri/tests/media_language_conformance.rs` proves the pinned crate
  passes MeedyaDL's copy of the test cases: nine sections, 222 cases; the four
  player-only sections are known but not run. It fails on an unknown section,
  an empty needed section, a missing required field, or a refusal case that is
  answered.
- MeedyaDL's own language code is all in `src-tauri/src/utils/language.rs` —
  every use of the crate's traits is there, so an API change in the crate is a
  change in that one file. The Metadata Language list's order comes from the
  crate through `commands/language.rs` (`order_languages_for_display`); names
  come from the WebView's `Intl.DisplayNames` (`src/lib/languageOptions.ts`,
  `src/hooks/useMetadataLanguageOptions.ts`).

**The pins, and what must happen after core merges.** The lock points at core
commit `904b0568…`, the crate at `995becb7…`; both are on core's
`feature/bcp47-language-policy` branch until core merges it. Then: `--update`
the copies to the merge commit, re-pin the crate to the same commit (`cargo
update -p meedya-lang` only), run the conformance test and the whole backend
suite. The copies and the crate must move together, or the test is checking the
crate against the wrong answers. Tracked in #1255. A core revision that changes
parts of the crate's API (its item traits merged under one parent) was
announced on 28 Sept 2026 — expected to touch only `utils/language.rs` and the
test's wrapper types.

**What MeedyaDL does with it (#1245–#1251):**

- Metadata language setting: stored in standard form on save and import; a
  non-tag import is refused (this machine's value kept); a non-tag already on
  disk is kept and reported once per launch, never rewritten (#1246).
- The `l=` localisation on MeedyaDL's own Apple Music requests: a real tag, or
  none at all (#1247).
- Storefront from a language: the parsed tag's two-letter region, else the
  existing `us` fallback (#1248).
- Metadata Language list: names in the interface language, policy order,
  `zh-Hans-CN` / `zh-Hant-TW` offered (checked against Apple's own Storefront
  documentation), a saved value not in the list kept and shown (#1249).
- LRC `[la:]`: the standard tag or nothing; WebVTT from TTML gets a
  `NOTE language: <tag>` comment block (#1250).
- Music-video subtitle files: `{video}.{tag}[.{role}…][.{n}].{ext}`; old
  `.cc.` names kept on disk and recognised (#1251).

**Decided — do not undo without the maintainer:**

- **Lyric sidecars are NOT renamed.** The policy (TRACK-070, TEXT-030) says
  formats without a language field carry the language in the file name, but
  MeedyaDL's lyric files are deliberately `{song}.lrc` / `.srt` / `.vtt` /
  `.ass` with no language in the name: that exact name is what music players
  look for. Only music-video subtitle tracks (which are the video's, not the
  song's) get language names.
- No settings migration and no renaming of existing files (COMPAT-020/030/050).
- The `.lyrics` (Lyricsfile) language comes from the shared `meedya-lyrics`
  crate and is fixed there, not patched in MeedyaDL.

**Not built yet (follow-ups):** album/track details fetched without the chosen
language (#1252); embedded subtitles carry no language or role (#1253); code
that assumes the first audio stream is the main one (#1254).
