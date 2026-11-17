<!--
  docs/standards/media-language-bcp47-policy.md
  Copyright (c) 2026 MeedyaSuite. Licensed under the MIT License.

  THIS IS THE MASTER COPY. Other repositories hold byte-identical copies,
  checked by their CI against a pinned commit of this file. Change it here
  only, bump the version, and follow "Changing this policy" below.
-->

# Media Language & BCP 47 Policy

| | |
|---|---|
| **Policy ID** | `MWBM-MEDIA-LANG` |
| **Version** | `1.0.0` |
| **Status** | Normative |
| **Date** | 2026-09-28 |
| **Master copy** | [`MWBMPartners/MeedyaSuite-core`](https://github.com/MWBMPartners/MeedyaSuite-core) — `docs/standards/media-language-bcp47-policy.md` |
| **Test cases** | `tests/fixtures/bcp47-language-policy-v1.json` (fixtures version `1.0.0`) |
| **Reference data** | `docs/standards/data/bcp47-language-data-v1.json` (data version `1.0.0`) |

This document says how every MWBM / MeedyaSuite application identifies,
stores, orders, names, matches and selects languages — for audio tracks,
subtitle tracks, lyrics, translations and multilingual metadata.

**It is the source of truth.** Not an assistant's memory, not a chat, not a
comment in one app. If code and this document disagree, the code is wrong
(or this document needs a new version — see [Changing this policy](#changing-this-policy)).
The test cases turn the rules into checks that fail a build, so the rules
hold whoever — or whatever — writes the code.

---

## Contents

1. [How to read this document](#1-how-to-read-this-document)
2. [Which parts apply to which project](#2-which-parts-apply-to-which-project)
3. [Words used here](#3-words-used-here)
4. [Part A — Stored content: identity and canonical order](#part-a--stored-content-identity-and-canonical-order)
5. [Part B — Player, library and interface presentation](#part-b--player-library-and-interface-presentation)
6. [Part C — Lyrics and text translations](#part-c--lyrics-and-text-translations)
7. [Part D — Existing content and backwards compatibility](#part-d--existing-content-and-backwards-compatibility)
8. [Conformance: test cases, data and copies](#8-conformance-test-cases-data-and-copies)
9. [Implementations](#9-implementations)
10. [Changing this policy](#changing-this-policy)
11. [Changelog](#changelog)
12. [Sources](#sources)

---

## 1. How to read this document

- **MUST** / **MUST NOT** — a requirement. Code that breaks it is a bug.
- **SHOULD** / **SHOULD NOT** — the expected behaviour. Departing from it
  needs a written reason in the code or the project's notes.
- **MAY** — allowed, not required.

Each rule has a stable ID (for example `LANG-021`). Use the ID in tests,
review comments and commit messages when it helps someone find the rule.
Do not scatter IDs through ordinary application code.

**Two orders, on purpose.** This policy defines two different ways of
ordering languages, and they MUST stay separate:

- **Canonical order** (Part A) is for what is *stored*: the order of tracks
  written into a file, the order of translations saved in a database. It
  depends only on the language tags, so it is the same on every machine, in
  every interface language, for every user.
- **Presentation order** (Part B) is for what a *person sees* in a menu. It
  depends on the interface language and the user's own preferences, so it
  is different for different people — which is exactly why it must never
  be written back into stored content.

Code MUST NOT use one where the other belongs, and MUST NOT implement both
with the same comparison function.

---

## 2. Which parts apply to which project

The policy is shared, but each project only does some of these things.
A project implements the parts marked for it, and SHOULD NOT build the rest.

| Profile | Parts | What it covers |
|---|---|---|
| **canonical** | A, D | Writing or rewriting stored content: containers, tags, databases |
| **presentation** | B | Menus and lists a person picks from; automatic selection |
| **text** | C (+ A for storage, B for display) | Lyrics, text translations, transliterations |

| Project | Profiles | Notes |
|---|---|---|
| MeedyaSuite-core | canonical, presentation, text | Holds this document and the shared Rust implementation (`meedya-lang`) |
| MeedyaDL | canonical, text; presentation only for its own settings lists | Reads source language data, writes tags, lyrics and subtitle files. Does not mux or reorder tracks itself (GAMDL and yt-dlp do) |
| MeedyaConverter | canonical; presentation for its editor | Copies, converts, remuxes, reorders and rewrites tracks |
| MeedyaManager | canonical, presentation | Edits stored metadata; shows languages in its interfaces |
| MeedyaPlayer | presentation (full), reads canonical | Menus, preferences, automatic selection |
| MeedyaSubtitler | canonical, text; small presentation part | Writes subtitle languages, roles and sidecar names |
| NetPLAYERapp | canonical now; presentation later | Stores station and programme languages |
| iHymns | text (primary), canonical, presentation | Original lyrics, translations, transliterations |
| iLyricsDB | text (primary), canonical, presentation | Lyrics, translations, imports and exports |

---

## 3. Words used here

- **Tag** — a BCP 47 language tag ([RFC 5646](https://www.rfc-editor.org/rfc/rfc5646)),
  such as `en`, `en-GB`, `zh-Hant-TW`, `es-419`.
- **Subtags** — the parts of a tag between hyphens:
  - **primary language** — the first part (`zh` in `zh-Hant-TW`);
  - **extlang** — an old three-letter extension after the language
    (`yue` in `zh-yue`); canonical form replaces it (see LANG-001);
  - **script** — four letters (`Hant`, `Latn`, `Cyrl`);
  - **region** — two letters for a country or territory (`TW`, `GB`), or
    three digits for a UN M.49 area covering several countries (`419` =
    Latin America and the Caribbean);
  - **variant** — five to eight characters, or four starting with a digit
    (`1996`, `valencia`);
  - **extension** — a single letter or digit other than `x` followed by more
    subtags (`u-ca-gregory`);
  - **private use** — `x-` followed by subtags agreed privately.
- **Language group** — every tag sharing one primary language: `en`,
  `en-GB` and `en-US` are all in the English group.
- **Special codes** — `mul` (several languages), `mis` (a language with no
  code), `und` (language not known), `zxx` (no language at all, such as an
  instrumental track), and `qaa`–`qtz` (reserved for local use).
- **Original** — the language the work was made in. A Japanese film has a
  Japanese original; its English dub is not original.
- **Role** — what a track is for, apart from its language: main programme,
  alternate mix, audio description, commentary, subtitles for the deaf and
  hard of hearing (SDH), forced subtitles, and so on.
- **Forced subtitles** — subtitles meant to be shown even to someone who
  understands the main audio: they translate only the parts in another
  language (a sign, a line of foreign dialogue).
- **Autonym** — a language's name in that language: `Deutsch`, `日本語`.
- **Localised name** — a language's name in the reader's interface
  language: `German` in an English interface, `allemand` in a French one.

---

## Part A — Stored content: identity and canonical order

Applies to anything persistent: MKV, MP4, MOV and other containers; audio
and subtitle tracks; tags inside files; lyric and text translations in
databases; any other stored content that names a language.

### LANG-001 — Canonical BCP 47 tags are the identity

A language MUST be identified by a canonical BCP 47 tag. A human-readable
name is presentation data and MUST NOT be used in place of the tag: not as
a key, not as a stored value, not as a foreign key, not in an exported
`xml:lang` or `lang` attribute.

**Canonical form** is produced by these steps, in this order (they follow
[RFC 5646 §4.5](https://www.rfc-editor.org/rfc/rfc5646#section-4.5), using
the reference data file):

1. Remove leading and trailing spaces (U+0020), tabs (U+0009), line feeds
   (U+000A) and carriage returns (U+000D) — those four characters and no
   others. Anything else, a no-break space included, is part of the value
   and makes it malformed. An empty string is not a tag.
2. If the whole tag (ignoring case) is a **grandfathered** tag in the
   registry (`i-klingon`, `en-GB-oed`, `i-default` …): use its replacement
   if the registry gives one and carry on with that; otherwise keep the tag
   exactly as the registry spells it — it is not reordered or split.
3. Check it is **well formed** under RFC 5646's grammar: hyphen-separated
   subtags of 1–8 ASCII letters and digits, in the order language, optional
   extlang, optional script, optional region, variants, extensions, private
   use. RFC 5646's grammar allows up to three extlangs, but no valid tag has
   more than one, so this policy treats more than one as malformed. The
   same variant twice, or the same extension letter twice, is malformed. Underscores are not separators (see
   LANG-004). A primary language of four to eight letters is allowed by
   the grammar but is accepted only if the registry lists it — none is
   listed today — so a language *name* such as `English` or `Deutsch` is
   malformed, not mistaken for a tag. A tag that fails is **malformed**
   (see LANG-026).
4. If the whole tag (ignoring case) is a **redundant** tag the registry
   gives a replacement for (`sgn-BR` → `bzs`), use the replacement.
5. Replace each subtag that has a registry **Preferred-Value**: languages
   (`iw` → `he`, `in` → `id`, `mo` → `ro`), regions (`DD` → `DE`,
   `BU` → `MM`), variants (`heploc` → `alalc97`). A **registered extlang**
   replaces the language before it — `zh-yue-HK` → `yue-HK` — and the
   language it leaves behind is then itself checked for a Preferred-Value:
   `ar-ajp` → `ajp` → `apc`. An extlang the registry does not list is kept
   where it is (`zh-abc` stays `zh-abc`).
6. Put **extensions** in order of their single letter (`a-…` before
   `u-…`), keeping each extension's own subtags in their written order.
7. Set the **case**: language, extlang, variants, and every subtag of an
   extension or of the private-use part in lower case; script in title case
   (`Hant`); region in upper case (`TW`); three-digit regions unchanged.
   (RFC 5646 §2.1.1 keeps subtags after a single-letter subtag in lower
   case; this is also the form Unicode CLDR uses. Stating it here removes
   any doubt about `en-x-foo-ab`: it stays lower case.)

**Canonical form is stable.** Steps 4 and 5 repeat until the tag stops
changing — a replacement can turn a tag into a redundant one, as `sgn-DD` →
`sgn-DE` → `gsg` — and if a replacement leaves the same variant twice, the
later one is dropped (`ja-Latn-hepburn-heploc-alalc97` →
`ja-Latn-hepburn-alalc97`). So canonicalising a canonical tag always
returns it unchanged. Every implementation MUST have this property, and
every test harness checks it on every `canonicalise` case.

Canonical form does **not** add or remove anything else. It does not add a
script or region (LANG-024), and it does not remove a script the registry
calls redundant: `en-Latn` stays `en-Latn`, because the content said so.

A subtag that is well formed but not in the registry (`en-JJ`: no region
`JJ` exists) is kept, and SHOULD be reported as unregistered. A deprecated subtag with no single replacement (`CS`,
Serbia and Montenegro, split into two countries) is kept and SHOULD be
reported. Neither is guessed at.

### LANG-002 — Reading old three-letter codes

Many containers and tag formats store three-letter ISO 639-2 codes (`eng`,
`ger`, `fre`): MP4's media header, Matroska's old `Language` field, ID3's
`TLAN` frame and the language field of `COMM`/`USLT`, most `ffprobe`
output. Free-text fields often hold them too — MusicBrainz Picard writes
`eng` into Vorbis and MP4 `LANGUAGE` fields. BCP 47 requires the shortest
code, so **every language value read from a file, a tag or another system
MUST go through this reader**, not straight into LANG-001. (LANG-001 alone
is for a value already known to be a BCP 47 tag, such as a tag a person
types into a tag field.)

Before the steps: remove trailing null characters (U+0000 — fixed-width
fields are padded with them) and the four whitespace characters of
LANG-001 step 1. If the field holds several values (ID3v2.4 separates them
with a null character), split them first and read each on its own; the
first is the primary language.

Then exactly one of these applies, in this order:

1. `XXX`, in any case — ID3's own "language not known" marker — means `und`.
2. Exactly three ASCII letters: lower-case it. If it is in the data file's
   `iso639_2` table, use the table's answer (`eng` → `en`; both `ger` and
   `deu` → `de`; `chi` and `zho` → `zh`; withdrawn `scc` → `sr`).
   Otherwise, if it is a registered language subtag (a genuine ISO 639-3
   code such as `yue` or `cmn`) or in the local-use range `qaa`–`qtz`,
   canonicalise it as a tag (LANG-001). Otherwise it is **unrecognised**.
3. Three letters that are **not** themselves a registered language
   subtag, a hyphen and two letters — how Matroska files written before
   version 4 give a country
   ([RFC 9559 §12](https://www.rfc-editor.org/rfc/rfc9559#section-12)):
   read the three letters by step 2, keep the two letters as the region,
   and canonicalise the result (`fre-ca` → `fr-CA`; `ger-DD` → `de-DE`). If
   the three letters are unrecognised or mean `und`, the whole value is
   **unrecognised**. When the three letters ARE a registered subtag
   (`und-GB`, `yue-HK`) the value is an ordinary tag and goes to step 4, so
   nothing it says is lost.
4. Anything else (two letters, or a longer tag) is canonicalised as a tag
   (LANG-001); if that finds it malformed, it is **unrecognised**.

An unrecognised value MUST NOT be turned into a guessed language. The
structured value is `und`, and the original text SHOULD be kept alongside
so nothing is lost and a person can fix it.

**A format's own default is not a guess.** Where a format defines a value
for a missing field — Matroska's `Language` element defaults to `eng`, and
its `FlagDefault` to 1 (RFC 9559) — a reader applies that default, because
that is what the file says. LANG-003 forbids inventing a language; it does
not forbid reading one the format defines.

### LANG-003 — Unknown is `und`, never a guess

When the language is not known, the stored value MUST be `und` (or the
format's own "unknown" marker when writing that format — ID3 uses `XXX`).
Code MUST NOT fill in `en`, `eng`, the interface language, the storefront's
language or any other default as though it were known. A database column
MAY allow "no value" instead of `und`, but MUST NOT default to a real
language.

`zxx` MUST be used, where the format allows, for content with no language
(music-and-effects tracks, instrumental lyrics placeholders), rather than
`und`.

### LANG-004 — Converting operating-system locale names

Locale names from operating systems and older libraries use a different
shape (`en_US.UTF-8`, `sr_RS@latin`). Code that reads one MUST convert it
before treating it as a tag:

1. `C` and `POSIX` mean "no language": there is no tag.
2. Drop anything from `.` (the character set) up to `@`.
3. Replace `_` with `-`.
4. A `@latin` modifier becomes the script `Latn`, and `@cyrillic` becomes
   `Cyrl`, placed after the language. Any other modifier is dropped and
   SHOULD be reported.
5. Canonicalise the result (LANG-001).

So `en_US.UTF-8` → `en-US`, `sr_RS@latin` → `sr-Latn-RS`, `zh_TW` → `zh-TW`.

The reverse is never done silently: a tag is not turned into `en_US` except
for an interface that demands that shape, and then only at that boundary.

### LANG-010 — Original first

The original language MUST come first in stored order.

- Where the container or data model records "original" as structured data
  (Matroska `FlagOriginal`, ffmpeg's `original` disposition, a database
  column, a "source language" field), use it.
- MUST NOT decide what is original from a track's position when structured
  data exists.
- The whole **language group** of an original item is promoted: if the
  original is `ja`, then `ja`, `ja-Latn` and every other `ja-…` item come
  before all other languages. Within that group, items marked original come
  first, then the rest of the group in canonical order.
- If several languages are marked original (a bilingual work), their groups
  come first, in canonical order among themselves.
- This applies to special codes too: a track marked original whose language
  is `und` (not known) or `mul` still comes first, because it is the
  original. Only a malformed value (LANG-026) is never promoted.
- Each list is ordered from its own items' markers. For tracks, that means
  each track type separately (TRACK-060): an original audio track does not
  promote subtitles of the same language unless they are marked original
  too. (Matroska's `FlagOriginal` is a per-track flag, and this keeps the
  order a plain reading of what the file says.) A data model that records
  the original language once for the whole work, rather than per item,
  marks every item whose canonical tag equals it.
- If nothing is marked original, nothing is promoted.

### LANG-020 — Order by primary language code

After the original group, language groups MUST be ordered by their primary
language subtag, compared as plain lower-case ASCII:
`de` → `en` → `es` → `fr` → `it` → `nl`.

The canonical order MUST NOT use: a language's English name; its autonym;
the operating system's sorting rules; the interface language. Each of those
would give a different stored order on different machines, or for
different users, or after a translation update.

### LANG-021 — General before specific

Within one language group, tags MUST be ordered by how specific they are:

1. the bare language — `en`
2. language + script — `zh-Hans`
3. language + region — `en-GB`
4. language + script + region — `zh-Hant-TW`
5. anything carrying an unregistered extlang, variants, extensions or
   private-use subtags, ordered among themselves by rules 1–4 applied to
   their language, script and region, then by the rest of the tag as plain
   ASCII

So `zh`, `zh-Hans`, `zh-Hant`, `zh-TW`, `zh-Hans-CN`, `zh-Hant-TW`.

### LANG-022 — Script before region

LANG-021 places every script-only form before every region-only form, and
script + region forms after both. Where two tags are at the same level,
scripts are compared as plain ASCII (`Hans` before `Hant`, `Cyrl` before
`Latn`), then regions (LANG-023).

### LANG-023 — Countries before multi-country areas

When regions are compared at the same level, two-letter country and
territory codes MUST come before three-digit area codes; each kind is then
in plain ASCII order. So `es`, `es-AR`, `es-ES`, `es-MX`, `es-419` — plain
ASCII would wrongly put `419` first.

### LANG-024 — Never invent subtags to sort

Code MUST NOT add a script, region or variant that the content did not
declare, just to decide an order: `en` stays `en` (not `en-Latn-US`), and
`zh-TW` stays `zh-TW` (not `zh-Hant-TW`), however likely the longer form.

Another part of an application MAY add information for a legitimate reason
(a person confirms it, a trusted source supplies it). If it does, what was
declared and what was inferred MUST remain distinguishable — for example a
separate field, or a record of where the value came from.

### LANG-025 — Where special codes go

After all ordinary languages (and after any original group LANG-010
promoted), in this fixed order:

1. `mul` — several languages
2. `mis` — a language with no code
3. `qaa`–`qtz` — local-use codes, in ASCII order
4. `und` — not known
5. `zxx` — no language
6. grandfathered tags that have no replacement (`i-default`), in ASCII order
7. private-use-only tags (`x-…`), in ASCII order
8. malformed values (LANG-026)

Every code in 1–5 is its own language group — each local-use code too, so
`qaa` and `qab` are two groups, in ASCII order — and within a group LANG-021
applies: `und-Latn` sorts after `und`, and `qaa-GB` after `qaa` but before
`qab`. In 6 and 7, every tag is a group of its own, keyed by the whole
canonical tag: `x-bar` and `x-foo` are two groups, not one.

### LANG-026 — Malformed values are kept, flagged and put last

A value that is not a well-formed tag MUST NOT be silently dropped,
"repaired" into a guess, or sorted as though it were a language. It keeps
its text, is reported, and goes after everything else, in the order it was
found — in stored order and in a menu alike: roles, original flags and
preferences do not reorder malformed entries among themselves (UI-045 does
not apply to them). Automatic selection breaks their ties by identifier
instead (AUTO-010). The text it keeps is the value after LANG-001 step 1's
trim: `" en_US "` keeps `en_US`, and a value of nothing but those four
whitespace characters (`" "`, a tab and a line feed) is malformed and keeps
the empty text.

### LANG-027 — Ties keep their order

Items with the same canonical tag (two English audio tracks, say) keep
their original relative order after other rules (TRACK-050) have been
applied. Every implementation MUST use a stable sort, so the result is the
same everywhere.

### NAME-010 — Names written into files use the autonym

When a self-contained file gets a human-readable track or language name
(for simple players that ignore language fields), it SHOULD be the
language's own name: `Deutsch`, `English`, `Español`, `Français`,
`Nederlands`, `日本語`, `한국어`, `简体中文` for `zh-Hans`, `繁體中文` for
`zh-Hant`. A regional form SHOULD use the language's own name for the
region where practical (`English (United Kingdom)`, `español (México)`).

### NAME-020 — A name never replaces the tag

An embedded name is a convenience for simple software. It MUST NOT be read
back as the language of the track when a language field exists, and MUST
NOT be the only place a language is recorded.

### TRACK-010 — Roles are structured data

Where a format has fields or flags for them, a track's roles MUST be
recorded there, not only in its title:

| Role | Matroska ([RFC 9559](https://www.rfc-editor.org/rfc/rfc9559)) | ffmpeg `-disposition` |
|---|---|---|
| Original language | `FlagOriginal` | `original` |
| Default | `FlagDefault` | `default` |
| Forced | `FlagForced` | `forced` |
| Commentary | `FlagCommentary` | `comment` |
| Deaf / hard of hearing (SDH) | `FlagHearingImpaired` | `hearing_impaired` (and `captions` for captions) |
| Audio description | `FlagVisualImpaired` | `visual_impaired` |
| Text descriptions of the picture | `FlagTextDescriptions` | `descriptions` |
| Dub | `FlagOriginal` = 0 (what ffmpeg's Matroska writer does with `dub`) | `dub` |

A title MAY repeat the role in words for simple players ("English — SDH").
The words MUST NOT be the only record, and a reader MUST prefer the flags
over the words.

### TRACK-020 — Original is recorded, not implied

When the original language is known it MUST be written as structured data
where the format allows (see TRACK-010), and MUST be read from there.

### TRACK-030 — Forced subtitles keep their meaning

A forced subtitle track is not an ordinary subtitle track in a different
list position. Its flag MUST be preserved on copy, convert and remux, and
it MUST NOT be offered or auto-selected as though it were the full
subtitles (see AUTO-030).

### TRACK-040 — Accessibility roles are kept

SDH, captions, audio description and text descriptions MUST be preserved
on copy, convert and remux, in both the structured field and (if present)
the title. Removing one is data loss.

### TRACK-050 — Order within a language group

In stored order, within one language group (after LANG-010's original
items), tracks are ordered by role first, then by LANG-021 to LANG-023,
then by LANG-027:

- **Audio:** main programme → alternate main mix → audio description →
  commentary → anything else.
- **Subtitles:** full → SDH / captions → forced → commentary → anything
  else.

A track with more than one role is placed by the one latest in its list.
A role an implementation does not recognise counts as **other**.

### TRACK-060 — Each track type is ordered on its own

Ordering applies within each track type. Types are not interleaved:
video, then audio, then subtitles, then anything else.

### TRACK-070 — Writing languages into containers

For self-contained files, write **both** where the format supports it:

1. the canonical tag in the format's full-tag field; and
2. a useful human-readable name (NAME-010), never a meaningless one such as
   `Track 2`, `ENG DUB` or `English AC3`.

| Format | Full tag | Old three-letter field (write it too, for older players) |
|---|---|---|
| Matroska / WebM | `LanguageBCP47` (and `TagLanguageBCP47`, `ChapLanguageBCP47`); readers MUST ignore the old field when it is present | `Language` — **bibliographic** form (`ger`, `fre`, `chi`) |
| MP4 / MOV | extended language box `elng` | media header `mdhd` language — **terminology** form (`deu`, `fra`, `zho`) |
| ID3 (MP3) | — (none exists) | `TLAN`, and the language of `COMM`/`USLT` — terminology form (ID3v2.4 says only "ISO-639-2"; this policy picks the terminology form so every writer agrees) |
| Vorbis comments (FLAC, Ogg) | `LANGUAGE` (free text: write the tag; read it with LANG-002, because other tools write `eng` there) | — |
| MP4 freeform `LANGUAGE` item | the tag (read it with LANG-002, as above) | — |
| TTML | `xml:lang` | — |
| WebVTT, SRT, ASS | no language field: use the file name (TEXT-030) and, where the format allows a header note, the tag | — |

When a three-letter field must be written, take the code for the tag's
primary language from the data file's `iso639_2_for_language` table, in
the form the format needs (the table above). A primary language in the
local-use range `qaa`–`qtz` is written as itself. For any other language
with no ISO 639-2 code, for `und`, and for a grandfathered, private-use or
malformed value, write `und` (ID3 MAY use `XXX`). The two forms
differ for twenty languages, so using the wrong one is a real error, not
a matter of taste. Converting down loses
the region and script: that is why the full-tag field MUST also be written
wherever it exists, and why the three-letter field MUST NOT be read back
when the full tag is present.

Tool-specific details (which ffmpeg or mkvmerge versions write which field)
change over time. Each project MUST check them against the tool it actually
runs, with a test, rather than rely on this table.

---

## Part B — Player, library and interface presentation

Applies to every list or menu a person chooses a language from: player
track menus, library views, translation pickers, language settings lists.
It applies to MeedyaPlayer in full, to playback and editing views in
MeedyaManager, to iHymns' and iLyricsDB's translation pickers, and to any
future player or library.

The order in a file MUST NOT be the only order a menu can show.

### UI-010 — Names in the interface language

A menu MUST normally show language names in the interface's current
language: for `de`, `German` in an English interface, `Deutsch` in a German
one, `allemand` in a French one. Names SHOULD come from the platform's own
locale data (Apple's `Locale`, the browser's `Intl.DisplayNames`, PHP's
`intl` extension) or bundled CLDR data, not from a hand-typed list.

A regional or script form SHOULD be named with its qualifier, localised:
`English (United Kingdom)`, `Chinese (Traditional)`.

### UI-011 — Autonyms are secondary in menus

The autonym MUST NOT be the only label in a general menu (unless it is the
same as the localised name). It MAY be shown as well — in a details view, a
tooltip, a second line.

### UI-020 — The user's languages first

If the user has set language preferences, the language groups of those
preferences come first, in the user's priority order.

A preference's group is its primary language: a preference for `en-GB`
brings the whole English group forward. A preference may name a special
code — `zxx` from someone who wants the music-only track, say — and it is
honoured like any other. A private-use or grandfathered preference brings
forward only its own group, which is that exact tag (LANG-025): a
preference for `x-foo` promotes `x-foo`, not `x-bar`. A malformed
preference is ignored.

### UI-030 — Then the original

The original language's group (LANG-010's meaning, special codes
included) comes next, if a preference has not already placed it. With no
preferences, it comes first. If several groups are original, they are
ordered among themselves as UI-040 orders groups — by localised name — so
a menu reads alphabetically, not by code.

### UI-040 — Then everything else, alphabetically by localised name

The remaining ordinary language groups follow, in alphabetical order of
their localised names, compared with the interface language's sorting rules
(so `éwé` sorts with the `e`s in French, not after `z`). Groups with the
same name are ordered by primary language code.

The special codes (LANG-025) come after all ordinary groups, in LANG-025's
order, and malformed values last.

**Example** — English interface, Japanese original, no preferences:
Japanese (original), Dutch, English, French, German, Spanish.

### UI-045 — Order within a language group

Within a group, in this order of importance:

1. **Role** — as TRACK-050. When the user has asked for an accessibility
   role (audio description, or SDH/captions), tracks *placed* by that role
   move ahead of the main ones within their group (AUTO-040). "Placed by"
   uses TRACK-050's rule — a track with several roles is placed by the
   latest in its list — so a forced SDH track counts as forced and does not
   move ahead.
2. **Exact preference** — a tag the user listed exactly (`en-GB`) comes
   before the group's other tags, in preference order. Only exact matches
   are promoted; `en` is not promoted by a preference for `en-GB`.
3. **Original** — items marked original.
4. **Specificity** — LANG-021 to LANG-023.
5. **Stability** — LANG-027.

None of this changes stored order.

### UI-050 — Menus do not move when something is selected

Selecting a track MUST NOT move it to the top or otherwise reorder the
menu. The choice is shown by a tick, a selected row, a radio button or the
platform's equivalent. The ordering function takes no "selected" input.

### UI-060 — Subtitles have an "Off" entry

A subtitle menu MUST offer "Off", separate from the tracks, as its first
entry. "Off" is not a language and is not sorted with them.

### UI-070 — Labels are built from structured data

A menu label is built from the language, role and (for audio) channel
layout, localised:

`English (United Kingdom) — Audio Description — 5.1`

- The parts are joined with ` — ` (space, em dash, space).
- A part that is empty (no localised name, no channel layout) is left out,
  with its separator.
- Roles appear once each, in TRACK-050's order.
- An embedded track title MUST NOT be used as the main label when language
  data exists. It MAY be shown in a details view.
- Codec, bit rate and sample rate belong in a details view, not the main
  label.

### MATCH-010 — Exact match is best

A preference and a track match **exactly** when their canonical tags are
identical. `en-US` is not an exact match for `en-GB`. A malformed value
matches nothing — not even an identical malformed value.

### MATCH-020 — Then a more general form

If a track's tag is a shorter form of the preference — the preference with
subtags removed from the end — it is a **general** match (`en` for a
preference of `en-GB`; `zh-Hant` or `zh` for `zh-Hant-TW`). Fewer removed
subtags is better.

### MATCH-030 — Then a more specific form

If the preference is a shorter form of the track's tag, it is a
**specific** match (`en-GB` for a preference of `en`). Fewer extra subtags
is better.

### MATCH-040 — Then a related form, only where suitable

Any other tag with the same primary language is a **related** match
(`en-US` for `en-GB`; `zh-TW` for `zh-Hant`) — with one exception: if both
tags state a script and the scripts differ (`zh-Hans` against `zh-Hant`,
`sr-Latn` against `sr-Cyrl`), they do **not** match at all. A reader of one
script may not be able to read the other.

Match strength, best first: exact → general → specific → related → none.
Different primary languages never match. These only ever match themselves
exactly: every tag whose primary language is `und`, `mul`, `mis` or `zxx`,
with or without further subtags (two "unknown" or "uncoded" tracks need not
be the same language — so `und-Latn` does not match `und-Latn-GB`); tags
that are private use from the start (`x-…`); and grandfathered tags with no
replacement (`i-default`). A tag
that merely *ends* in a private-use part (`en-x-foo`) is an ordinary tag.

**Distance** — for general and specific matches — counts every
hyphen-separated part added or removed, single-letter parts included:
`en-u-ca-gregory` against `en` is general, distance 3.

Matching MUST NOT change either tag. It MUST NOT use likely-subtag
inference (LANG-024): `zh-TW` and `zh-Hans` are "related", not a mismatch,
because nothing in the tags says `zh-TW` is written in Traditional script.
That is a known limitation of this version (see the changelog).

### AUTO-010 — Automatic selection is a separate decision

Choosing which track plays is a different job from ordering the menu. The
selection MUST NOT be "whatever is first in the menu", and MUST give the
same answer whatever order the tracks are listed in: where the tie-breaks
below leave two tracks level, the **track identifier** decides (the number
or ID the file gives the track), never its position in a list.
Identifiers are put in one fixed order: those made only of ASCII digits
come first, ordered as numbers (`9` before `10`) and, when equal as
numbers, as plain text (`01` before `1`); every other identifier comes
after them, in plain-text order. (Comparing a digits-only pair as numbers
but a mixed pair as text would not be a consistent order — `2` < `10` <
`1a` < `2` — so the digits-only identifiers are kept together.) Identifiers MUST be unique
within one selection: an implementation given two tracks with the same
identifier refuses with an error rather than guess. Wherever the rules
below say "canonical order", ties that canonical order would leave to list
position (LANG-026 for malformed values, LANG-027 for equal tags) are
broken by the identifier instead. It MAY use:
preferences, exact regional and script preference, original, default,
forced, roles, accessibility settings, playback context and saved choices.

A malformed preference is ignored here as in menus (UI-020): it matches
nothing, and a user whose preferences are all malformed counts as having
none — so **automatic** subtitle mode (AUTO-030) acts as **forced only**
for them.

### AUTO-020 — Choosing audio

1. Tracks placed (TRACK-050) as **commentary** or **other** are never
   chosen automatically unless every audio track is one — and then
   commentary ranks before other (TRACK-050), whether or not audio
   description was asked for. Alternate mixes and audio description can be
   chosen, but rank after the main programme.
   **Role rank** here uses each track's placing role (TRACK-050): main
   programme, then alternate, then audio description — or, when the user
   has asked for audio description, audio description, then main
   programme, then alternate. A track with both `alternate` and
   `audio_description` is placed as audio description.
2. For each preference, in order: find the tracks that match it (MATCH,
   related or better). If any do, choose the best of them by, in order:
   role rank, match strength, fewer removed or extra subtags, default flag,
   original flag, canonical order (TRACK-050), track identifier. Stop.
   Here and below, **canonical order** means the position each track would
   have in stored order (TRACK-050, TRACK-060) among *all* tracks of its
   type — including tracks that cannot be chosen — so an original track
   that is never chosen (commentary, say) still brings its language group
   forward, as it does in stored order.
3. If no preference matched: the original track(s), best by role, default
   flag, canonical order, identifier.
4. Otherwise the default track(s), by the same tie-breaks.
5. Otherwise the best by role (as in step 2), then canonical order, then
   identifier — so with nothing else to go on, a main-programme track is
   still chosen over an audio-description track nobody asked for.

### AUTO-030 — Choosing subtitles

The subtitle choice depends on the audio chosen and on a subtitle mode:

- **off** — nothing.
- **forced only** — the forced track that best matches the *audio's*
  language (related or better; then match strength, fewer removed or extra
  subtags, default flag, canonical order, identifier). Nothing if there is
  none, or if the audio's primary language is `und` (not known), `mul`
  (several) or `zxx` (none), with or without further subtags — there is
  nothing to match a forced track against. A private-use or grandfathered
  audio tag (`x-…`, `i-default`) does have something to match: a forced
  track with exactly that tag (MATCH-040).
- **always** — for each preference in order, the best-matching subtitle
  that is not forced, commentary or other (SDH first if the user has asked
  for captions, otherwise full subtitles first; then match strength, fewer
  removed or extra subtags, default flag, canonical order, identifier). If
  no preference matches: the default-flagged track among those that are not
  forced, commentary or other (ties by canonical order, identifier), else
  nothing.
- **automatic** (the usual default) — if the audio's language matches one of
  the user's preferences (related or better), or the user has none, act as
  **forced only**. Otherwise act as **always**.

A forced track is never chosen by **always**, and a full track is never
chosen by **forced only** (TRACK-030). If no audio track was chosen (a
silent video), the audio's language counts as not known: **forced only**
gives nothing, and **automatic** acts as **always** when the user has
preferences. **Canonical order** here has the meaning AUTO-020 gives it:
stored order among all subtitle tracks, including those that cannot be
chosen.

### AUTO-040 — Accessibility preferences

When the user has asked for audio description, AUTO-020 prefers
audio-description tracks within the chosen language, and UI-045 places them
first within their group. When the user has asked for captions, AUTO-030
prefers SDH / captions tracks and UI-045 places them first within their
group.

---

## Part C — Lyrics and text translations

Applies to iHymns, iLyricsDB, the lyrics features of MeedyaDL and
MeedyaManager, and any future multilingual text feature.

### TEXT-010 — Original lyrics first

The original (source) text's language comes first in stored order
(LANG-010). Translations follow in canonical order (LANG-020 to LANG-027).

### TEXT-020 — Translations and transliterations use full tags

Every stored translation and transliteration MUST carry a canonical tag.
A transliteration SHOULD state its script (`ja-Latn` for romanised
Japanese, `sr-Latn`); a transliteration with no script cannot be told apart
from the original.

### TEXT-030 — File names carry the tag, unchanged

A sidecar file named by language (subtitles, lyrics) MUST follow this
shape:

    {media file stem}.{tag}[.{role}…][.{n}].{extension}

- **The tag comes first, always.** It is the canonical tag (`Film.en-GB.srt`,
  `Song.zh-Hant.lrc`); when the language is not known, or the value is
  malformed, it is `und`. A malformed value never goes into a file name —
  it could contain characters that are unsafe in a path. A builder reads
  the language it is given with LANG-002's reader, exactly as a reader will
  read the name back — so it writes `Film.fr.srt` when given `fre`, and
  `Film.und.srt` when given a value the reader does not recognise. What a
  builder writes is therefore always what a reader reads back.
- **Role words** follow, in TRACK-050 order, from this list only: `sdh`,
  `forced`, `commentary` (`Film.en.sdh.srt`, `Film.en.forced.srt`). A
  reader also accepts `cc` and `hi` as `sdh`, because other tools write
  them.
- **A number** (`.2`, `.3` …) is added only when two sidecars would
  otherwise get the same name, numbering from the second. A builder refuses
  any other number (0, 1, a negative number, or more than nine digits —
  above 999999999 — which a reader could not read back).
- **Reading one back:** take the stem from the media file the sidecar
  belongs to — never guess where the stem ends — so `Mr. Robot.en.sdh.srt`
  beside `Mr. Robot.mkv` reads as tag `en`, role `sdh`. Because the tag is
  always first, a role word is never mistaken for a language (`sdh` is also
  the code for Southern Kurdish, and `hi` for Hindi). The first part is
  read with LANG-002's reader, so `Film.eng.srt` reads as `en`; an
  unrecognised first part reads as `und`, with the original text kept. Of
  the parts after it, role words are read as roles (each once), a part of
  one to nine ASCII digits is the number (if there are several, the last
  counts; a longer run of digits is not a number, so it is never silently
  changed into a different one), and any other part is ignored and SHOULD
  be reported: `Film.en.sdh.backup.srt` reads as tag `en`, role `sdh`.

### TEXT-040 — Presentation follows Part B

Lists of translations shown to people use localised names (UI-010) and
presentation order (UI-020 to UI-050). User preferences MAY reorder what is
shown; they MUST NOT reorder what is stored.

### TEXT-050 — Keep distinct forms distinct

Translations that differ by script or region (`zh-Hans` and `zh-Hant`;
`pt-BR` and `pt-PT`) are different translations. Storage MUST NOT collapse
them to the bare language, and a data model that can only hold bare
languages is non-conforming (fix it — see Part D).

### TEXT-060 — Do not invent distinctions

Storage MUST NOT add a script or region the source did not declare
(LANG-024). A translation declared `pt` stays `pt`.

### TEXT-070 — Export what was stored

An exported format with a language attribute (`xml:lang` in TTML, `lang`
in HTML, `inLanguage` in JSON-LD) MUST carry the tag, never a name.

---

## Part D — Existing content and backwards compatibility

### COMPAT-010 — Look before changing

Before changing code that handles languages, find out what it does today
and what data it has already written.

### COMPAT-020 — Do not rewrite people's files just to reorder them

Existing media MUST NOT be rewritten solely to change the physical track
order. The canonical order applies to content an application creates or
rebuilds, and to explicit metadata-editing operations a person asks for.

### COMPAT-030 — Keep valid metadata

Valid existing language data, flags and titles MUST be preserved when a
file or record is touched for another reason.

### COMPAT-040 — Report doubt, do not resolve it by guessing

Old data that is ambiguous (an unrecognised three-letter code, a name
where a tag should be, a language that is plainly a region code) SHOULD be
reported for a person to fix. It MUST NOT be silently turned into a more
specific or different language.

### COMPAT-050 — Migrate only where the data model requires it

A database or format change is justified when the current model cannot
represent what this policy requires (for example, it cannot store `pt-BR`).
It is not justified merely to reorder existing rows.

---

## 8. Conformance: test cases, data and copies

### 8.1 The test cases

`tests/fixtures/bcp47-language-policy-v1.json` holds the conformance cases
(schema: `bcp47-language-policy-v1.schema.json` beside it). Each case names
the rules it checks. Sections:

| Section | Checks | Needed by profile |
|---|---|---|
| `canonicalise` | LANG-001, LANG-026 | all |
| `legacy_three_letter` | LANG-002, LANG-003 | canonical, text |
| `iso639_2_write` | TRACK-070 | canonical (writes old three-letter fields) |
| `sidecar_name` | TEXT-030 | any that names or reads sidecar files |
| `posix_locale` | LANG-004 | any that reads OS locales |
| `canonical_order` | LANG-010 to LANG-027 | canonical, text |
| `track_order` | TRACK-050, TRACK-060 | canonical (tracks) |
| `presentation_order` | UI-020 to UI-050 | presentation |
| `subtitle_menu` | UI-060 | presentation (subtitles) |
| `label` | UI-070 | presentation |
| `match` | MATCH-010 to MATCH-040 | presentation, text |
| `auto_select_audio` | AUTO-010, AUTO-020, AUTO-040 | presentation (player) |
| `auto_select_subtitle` | AUTO-010, AUTO-030, AUTO-040 | presentation (player) |

Presentation cases supply the localised names and a simple sorting rule
inside the case, so the expected answer does not depend on which platform
or which version of locale data runs the test. Real code uses real locale
data; the cases check the *rules*, not the names.

An implementation MUST pass every case in every section its profile needs,
and its CI MUST run those cases on every change (a failure fails the
build). A test harness MUST also fail, rather than report success, when
the case file has a section it does not know, a section it needs is
missing or empty, or a case lacks a field the schema requires — a harness
that quietly runs fewer checks than the file holds is how a broken
implementation passes. A case marked `"error": true` checks a refusal the
rules require (a sidecar number the builder must refuse, duplicate track
identifiers): the implementation passes only if it refuses with an error;
returning any value fails the case.

### 8.2 The reference data

`docs/standards/data/bcp47-language-data-v1.json` is generated by
`scripts/media-lang/generate_language_data.py` from the IANA Language
Subtag Registry and the ISO 639-2 list as published by Debian's iso-codes
project (see [Sources](#sources)). It holds the replacements used by
LANG-001, the reading table used by LANG-002, the writing table used by
TRACK-070, and the registered subtags. Implementations read it (or an exact
copy) rather than keeping their own lists.

### 8.3 Copies in other repositories

A repository that needs this policy keeps **exact copies** of the files it
uses — normally this document, the test cases and their schema, and the
data file and its schema — in its own tree, so the policy is readable from
a plain checkout by any person or tool.

Beside them it keeps `MWBM-MEDIA-LANG.lock`, recording the policy version,
the core commit the copies came from, and each file's SHA-256 checksum and
master path. The checker `scripts/media-lang/check_copies.py` (itself
copied verbatim) runs in that repository's existing CI and fails when:

- a copy has been edited (its checksum no longer matches the lock);
- the lock leaves out one of the files every copy must have (this
  document, the test cases and their schema, the data and its schema, the
  checker); or, for any copy of the PHP implementation, names some of its
  three files but not all, or keeps them in a different layout from the
  master (its test runner loads the implementation from the folder above
  it); or a file in the repository has the name of a master file but is
  not in the lock — so deleting a lock line cannot switch a check off;
- a master path is not one of the master files, or a local path points
  outside the repository;
- the recorded commit is not part of MeedyaSuite-core's own history on an
  approved branch (GitHub will serve a commit that exists only in someone's
  fork under the original repository's address, so this is checked); or
- the lock's checksums do not match the master files at that commit
  (downloaded from GitHub; the core repository is public, so no secret is
  needed). A download or lookup failure fails the check — it never passes
  silently. The local-only mode (`--offline`) refuses to run in CI.

Copies MUST be kept byte for byte: a repository that converts line endings
marks them unconverted (`-text` in `.gitattributes`); the checker says so
when that is the cause of a mismatch.

To take a new version: run the checker's `--update <commit>` mode, which
downloads the master files at that commit, replaces the copies and rewrites
the lock; then update code and tests as the changelog requires.

A repository's agent-instruction files (`AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, `.claude/`, `.OpenAI/` …) MUST point to its copy of this
document for any work touching languages, translations, audio or subtitle
tracks, lyrics, track order or naming, language preferences or
accessibility roles — and MUST NOT paste the rules in, where they would
drift.

---

## 9. Implementations

Write the logic once per programming language, and test every one against
the same cases.

| Language | Where | Used by |
|---|---|---|
| Rust | MeedyaSuite-core, crate `meedya-lang` | MeedyaDL, MeedyaManager, future Rust apps |
| PHP | MeedyaSuite-core, `bindings/php/media-language/` (copied verbatim, like this document) | iHymns, iLyricsDB, NetPLAYERapp |
| Swift | MeedyaConverter (for now) | MeedyaConverter; to move to a shared package once MeedyaPlayer or MeedyaSubtitler has code |
| JavaScript | not yet written | Browser code SHOULD use `Intl.DisplayNames` and `Intl.Collator` for names and sorting, and call the server or a port for ordering and matching |

Each implementation keeps these jobs apart, whatever it names them:
tag parsing and canonical form; the canonical comparison (Part A); the
presentation comparison (Part B); preference matching; localised names;
role ordering; automatic selection.

---

## Changing this policy

- **Version numbers** follow the usual pattern:
  - **major** (`2.0.0`) — any change that would give a different answer to
    an existing test case, or change stored order;
  - **minor** (`1.1.0`) — new rules or cases that leave every existing
    answer the same;
  - **patch** (`1.0.1`) — wording only.
- The test cases carry their own `fixtures_version` and state the policy
  version they belong to; the data file carries its own `data_version`.
- **A data refresh is not a rule change.** When a newer IANA registry or
  ISO 639-2 list changes an answer (a code newly deprecated, say), the data
  version goes up, the affected cases are updated to the new answer and the
  fixtures version goes up by a minor step, in one change — the policy
  version does not, because no rule changed. Every implementation then
  takes the new data. A change to a *rule* follows the version numbers
  above.
- A change to ordering, matching or selection MUST update, in the same
  change: this document, its changelog, the affected cases, and every
  implementation in section 9. Silently changing behaviour is not allowed.
- Consumers move to a new version deliberately (8.3); a copy never
  updates itself.

---

## Changelog

### 1.0.0 — 2026-09-28

First version. Rules LANG-001 to LANG-027, NAME-010, NAME-020, TRACK-010
to TRACK-070, UI-010 to UI-070, MATCH-010 to MATCH-040, AUTO-010 to
AUTO-040, TEXT-010 to TEXT-070 and COMPAT-010 to COMPAT-050. Test cases
`1.0.0`; reference data `1.0.0` (IANA registry of 2026-09-17).

Known limitations, deliberately left for a later version:

- Matching does not use likely-subtag data, so `zh-TW` counts as a
  "related" match for `zh-Hans` (MATCH-040).
- No bundled localised-name data: names come from each platform.
- Unicode locale extensions (`-u-…`) are ordered but not otherwise
  interpreted.
- Macrolanguages are not related to their members: canonical form turns
  `zh-cmn-Hans` into `cmn-Hans` (the registry's own rule), and MATCH-040
  then finds no match between a `zh-Hans` preference and a `cmn-Hans`
  track; `no` and `nb` behave the same. Matching through the registry's
  macrolanguage data is planned for a later minor version, since it only
  adds matches.

Revised before first use (28 Sept 2026) after an independent review: the
copy checker was hardened (it could be pointed at another repository's
file, at a fork-only commit, or have a file removed from checking by
deleting a lock line); the ISO 639-2 data was rebuilt from the real ISO
639-2 list (50 ISO 639-5-only group codes had been included); and about
twenty points where two careful implementations could disagree were
settled, each with a test case. No version was published before this
revision, so it stays 1.0.0.

Also settled before release (28 Sept 2026), after independent reviews of
the Rust and Swift implementations, each with a test case and no change to
any existing case's answer:

- eight points the text had left open — a malformed preference matches
  nothing, not even an identical malformed value, and a user whose
  preferences are all malformed counts as having none (MATCH-010,
  AUTO-010); "canonical order" in automatic selection means stored order
  among *all* tracks of the type, including ones that cannot be chosen
  (AUTO-020, AUTO-030); when every audio track is commentary or other,
  commentary ranks before other (AUTO-020); a private-use or grandfathered
  audio tag can match a forced track with exactly that tag (AUTO-030); a
  sidecar builder reads the language it is given with LANG-002's reader
  (TEXT-030); a label lists each role once (UI-070); a label leaves out an
  empty part, with its separator (UI-070); and a malformed value
  keeps its text after LANG-001 step 1's trim, so a value of only
  whitespace keeps the empty text (LANG-026).

---

## Sources

- [RFC 5646 / BCP 47](https://www.rfc-editor.org/rfc/rfc5646) — tag syntax
  and canonical form.
- [RFC 4647](https://www.rfc-editor.org/rfc/rfc4647) — matching of language
  tags (MATCH rules follow its "lookup" idea, with the additions stated).
- [IANA Language Subtag Registry](https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry)
  — registered subtags and replacements.
- [ISO 639-2 list, Debian iso-codes](https://salsa.debian.org/iso-codes-team/iso-codes)
  — the ISO 639-2 codes with their bibliographic, terminology and ISO 639-1
  forms, copied from the ISO 639-2 Registration Authority.
- [ISO 639-2 change history](https://www.loc.gov/standards/iso639-2/php/code_changes.php)
  — withdrawn three-letter codes.
- [RFC 9559](https://www.rfc-editor.org/rfc/rfc9559) — Matroska:
  `LanguageBCP47` and the track flags.
- ISO/IEC 14496-12 — MP4/MOV media header language and the `elng` box.
- [ID3v2.4.0 structure](https://mutagen-specs.readthedocs.io/en/latest/id3/id3v2.4.0-structure.html)
  — three-letter language fields and `XXX` for unknown.
- [UN M.49](https://unstats.un.org/unsd/methodology/m49/) — numeric area
  codes such as `419`.
- [Unicode CLDR](https://cldr.unicode.org/) — localised names and sorting
  rules, via each platform.
