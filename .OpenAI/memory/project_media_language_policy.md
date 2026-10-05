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
  first (round 4). The "exactly one" decision is its own function on the
  lock file's text (`single_crate_pin`), tested at zero, one and two entries,
  and the `name = "meedya-lang"` line is matched with spaces or tabs around
  the `=` allowed (round 5).
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
  pins the interface language's whole primary-language GROUP first, matching
  where the real order puts it, so the list does not visibly jump when the
  backend replies. Round 4 pinned only an exact match, which passed all its
  tests (bare codes) yet still jumped for the full tags "Auto" really sends
  (`fr-FR`, `fr-CA`); round 5 groups by identity, and the test table is
  copied from the real Rust ordering for ten cases.
- **Language identity comes from the backend, not the browser** (Codex's
  catch-up review): `language_identities` (`commands/language.rs`, through
  `utils::language::language_identity`) gives each tag's standard form and
  grouping identity. `Intl.Locale` folds `cmn` into `zh` and cannot read
  `zh-cmn-Hans`, so `Intl` now supplies only names and their alphabetical
  order; `src/hooks/useLanguageIdentities.ts` is the one place identity is
  fetched. Until the backend has answered for a tag -- while pending, AND
  for good if the request keeps failing (it is retried three times, after
  0.5, 1 and 2 seconds; a call refused as too large is not retried, since
  it would be refused again) -- the tag's standard form is the tag itself, so
  nothing is merged or hidden on the browser's reading (which would hide a
  stored Mandarin `cmn-Hans-CN` behind the offered `zh-Hans-CN`); the
  browser's reading is used only to sort (Codex's review of round 5). Once
  answered, a stored `"EN-us"` shows as one row beside the offered
  `"en-US"` (until then, two rows with the same name each show their tag
  after it, as Mandarin and Chinese always do), and the dropdown is handed
  that row's value so it shows English
  selected (handing it `"EN-us"` made it show its first row); the stored
  spelling is never rewritten (COMPAT-030).
- An imported language's length check and tag check are one small function
  returning its reason (`decide_imported_language`), and the log line is built
  by a function never given the value (round 5). A stored non-tag value reaches
  GAMDL's command line with the same NUL/CR/LF removed as the `config.ini` line.
- Music-video subtitle extraction writes ffmpeg's output to a short,
  exclusively created temporary file (`.meedyadl-partial-<pid>-<random>.srt`,
  64 random bits, so a name never repeats; it was a counter from 0, which
  handed a just-removed name to the next extraction) and publishes it under the real name without ever overwriting: a hard link; on
  a drive without hard links the system's one-step rename-only-if-free
  (`fs_safe::rename_no_replace`; FAT32 on a Mac); and where that is refused
  too (exFAT on a Mac, which refuses it whenever the name is free) a copy
  into a new file created only if the name is free, deleted again on any
  failure (`fs_safe::copy_to_new_file`; stand-in review of round 6). Never a
  plain rename. That third step is not one operation: a forced stop
  part-way through its copy can leave a partly written subtitle under the
  real name, which later runs keep. If it fails, no subtitle, and an
  activity-log message saying what to do. A forced stop
  can leave a temporary file. No run deletes one it did not make: round 7's
  clean-up (process not running here AND unchanged for an hour, or any
  second name of a finished subtitle) was removed, because a name or a
  clock proves nothing about who owns a file (Codex's review of rounds 6-7).
  The next extraction in that folder writes one activity-log line saying
  how many it found, where, and that they are safe to delete once no
  MeedyaDL is saving subtitles there. A run removes its own temporary file
  only if the name refers to the file its creating handle (held open) is
  on, both read at that moment (`fs_safe::check_name_against_handle`); not
  a number stored at creation, because a Mac's FAT32 and exFAT drives
  renumber a file once it is written to or renamed. Otherwise it leaves
  and reports it. The
  check and the removal are two steps, so a replacement made in the instant
  between them would still be removed (Codex's review of rounds 6-7,
  finding 2). Failed removals are reported. The fake ffmpeg
  in the tests obeys `-y`/`-n` (ffmpeg 9.0.1 given `-n` refuses an existing
  output yet exits 0, which published an empty file). Windows/Linux branches
  type-checked, only run on macOS (Codex's catch-up review and round 5).
- `.gitattributes` protects each policy copy with
  `-text -filter -working-tree-encoding -ident`. What can still override
  those lines: a LATER line in the same file that matches the copy, a
  `.gitattributes` closer to the copy (its own folder or one between it and
  the top), and `.git/info/attributes` (always highest priority). The global
  and system attributes files have the lowest priority and cannot. The copy
  checker reports any changed bytes.
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
