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
  passes MeedyaDL's copy of the test cases: ten sections, 241 of the file's
  290 cases (at core `aaaa585`); the three player-only sections (49 cases) are
  known but not run. It fails on an unknown section, an empty needed section,
  a missing required field, an `error` key where the schema allows none (or
  `false` anywhere), a refusal case that is answered, the crate and the
  policy copies being pinned to different core commits, or a section that
  compares fewer of its cases than the file actually has (checked by ID, not
  just a count — round 3 of the independent review, 28 Sept 2026, found a
  count could stay right even if a case were skipped partway through).
  `label` (UI-070) joined the run sections in that same round: it had been
  left as a not-needed, player-only section with no reason given, although
  section 8.1's own table lists it as needed by the "presentation" profile
  MeedyaDL declares itself as following. The Cargo.lock reader the test's
  own "two pins must move together" check depends on now collects EVERY
  `meedya-lang` package entry the file names and refuses to run unless
  there is exactly one, rather than silently using whichever entry came
  first (round 4).
- MeedyaDL's own language code is all in `src-tauri/src/utils/language.rs` —
  every use of the crate's traits is there, so an API change in the crate is a
  change in that one file. The Metadata Language list's order comes from the
  crate through `commands/language.rs` (`order_languages_for_display`); names
  come from the WebView's `Intl.DisplayNames` (`src/lib/languageOptions.ts`,
  `src/hooks/useMetadataLanguageOptions.ts`).

**The pins, and what must happen after core merges.** Since the copy-update
sweep of 28 Sept 2026, the lock AND the crate both point at core commit
`aaaa585aa145634c057c0bbdd9bd5fc11c3274a0` (the reviewed revision 6; before
that, the copies were at `904b0568…` and the crate at `995becb7…`). It is still
on core's `feature/bcp47-language-policy` branch: core has NOT merged yet. When
it does: `--update` the copies to the merge commit, re-pin the crate to the same
commit (`cargo update -p meedya-lang` only), run the conformance test and the
whole backend suite. The copies and the crate must move together, or the test
is checking the crate against the wrong answers. Still owed — tracked in #1255.

The API change that sweep brought (roles on a shared `RoleItem` parent trait,
`TagMatch::distance` as `usize`) touched only `utils/language.rs` and the test's
wrapper types, as expected. The sidecar builder now reads its language with the
three-letter reader; MeedyaDL already passed it a tag read that way, so names
did not change.

**What MeedyaDL does with it (#1245–#1251):**

- Metadata language setting: stored in standard form on save and import; a
  non-tag import is refused (this machine's value kept); an imported value
  over 256 bytes is refused before it is even read as a tag, and any refusal
  is logged by length alone, never the value itself (round 4); a non-tag
  already on disk is kept and reported once per launch, never rewritten
  (#1246). The stored value is also put into standard form right before it
  reaches GAMDL's `--language` argument — whatever its length, not only a
  tag short enough to be stored (round 4 closed that gap: a well-formed tag
  over the 35-character storage limit used to reach GAMDL completely
  unstandardised) — and right before it reaches config.ini's own `language`
  line, which the lyrics-fallback retry path reads instead of being passed
  `--language` directly (round 4) — without ever rewriting the STORED value
  itself (`utils::language::language_arg_for_gamdl` /
  `utils::language::MAX_TAG_LEN`, round 3 and round 4 of the independent
  review). The interface language setting (`ui_language`) follows the same
  import rule as the metadata language — standard form, refused and this
  machine's value kept if it is not a tag, the same 256-byte pre-parse
  refusal — with one difference: an empty value is kept, not refused,
  because it means "follow the system".
- The `l=` localisation on MeedyaDL's own Apple Music requests: a real tag, or
  none at all (#1247).
- Storefront from a language: the parsed tag's two-letter region, else the
  existing `us` fallback (#1248).
- Metadata Language list: names in the interface language, policy order,
  `zh-Hans-CN` / `zh-Hant-TW` offered (checked against Apple's own Storefront
  documentation), a saved value not in the list kept and shown (#1249). The
  Interface Language list (Settings > General > Language) now uses the same
  order (`useInterfaceLanguageOptions`, round 3 of the independent review) —
  it used to just follow the order its three entries happen to be written in.
  Its FALLBACK order (shown before the backend answers, or if it never does)
  also pins the interface language's own row first, matching where the real
  order would put it, so the list does not visibly jump the moment the
  backend replies (round 4 — French is the case that showed the gap:
  "allemand" sorts before "français" alphabetically, so the un-pinned
  fallback showed German first for a French interface).
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
