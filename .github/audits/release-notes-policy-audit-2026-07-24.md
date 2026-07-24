# Release-Notes Policy & Pipeline Audit — Mechanism-Disclosure Requirement (2026-07-24)

**Scope:** MeedyaDL's release-notes system — policy (`.github/release-notes/STYLE_GUIDE.md` /
`README.md`), generation pipeline (`release.yml` `ensure-release`, `release-note-gate.yml`,
`cliff.toml`, both `.tera` templates, `scripts/release-notes/*`), and all 30 committed curated
notes files — against TWO requirements:

- **(a)** plain English for non-technical users (existing, stated policy), and
- **(b)** **NEW:** never disclose the behind-the-scenes mechanism by which a feature is delivered
  (owner example: word-by-word/syllable lyrics and animated cover art are obtained via an Apple
  Music web-player token — the *capability* may be advertised; **how it is obtained may not**).

Read-only audit on branch `audit/release-notes` (off `origin/alpha` @ `497e3222`). Builds on the
prior format-only diagnosis `.github/audits/release-notes-eli5-diagnosis-2026-07-24.md` (#1046),
which this audit does not repeat except where its findings intersect requirement (b).

---

## 1. Policy gap analysis for requirement (b)

### 1.1 Where the current guide permits or encourages mechanism disclosure

| Location | Quote | Problem under requirement (b) |
|---|---|---|
| `STYLE_GUIDE.md:14` | *"Lead with the symptom or the benefit. **Never lead with the mechanism.**"* | This is an **ordering** rule, not a **confidentiality** rule. It explicitly contemplates the mechanism appearing in the sentence — just not first. Nothing anywhere in the guide says a mechanism may not be disclosed at all. |
| `STYLE_GUIDE.md:16-27` (Hard bans) | Bans are all **form**-based: code identifiers, `snake_case`, crate names, CLI flags, commit jargon. | A perfectly plain-English sentence like *"lyrics are now fetched using the Apple Music web-player token"* passes every hard ban. The ban list has no concept of *operationally confidential content*. |
| `STYLE_GUIDE.md:30-34` (Allowed user vocabulary) | *"Apple Music, GAMDL, wrapper, cookies, queue, Activity log, Library Scan, setup wizard, storefront, Dolby Atmos, Lossless (ALAC), AAC, synced lyrics."* | The list is UI-driven ("words users already see in the app") but states no *test* for admission. Without a stated boundary, authors extend it by analogy — "cookies are allowed, so surely 'token' is too" is exactly the failure mode that produced `v1.10.0-alpha.15.md:7` (see §2). |
| `STYLE_GUIDE.md:39-48` (Translation glossary) | Maps `TTML` → *"Apple's synced-lyrics format"*; closing rule: *"describe the user-observable effect, not the internal name."* | Direction is right but scope is wrong: it treats internal terms as a **readability** problem ("translate it") when some are a **confidentiality** problem ("omit it entirely"). Translating `fetch_syllable_lyrics()` "the same way" invites *"MeedyaDL now asks Apple's lyrics service directly"* — a translated, plain-English **disclosure of the acquisition path**. |
| `STYLE_GUIDE.md:112` + `README.md:50` | *"Gold-standard references: `v1.10.0-alpha.15.md` and `v1.11.0-alpha.18.md`"* | Both anointed gold standards **violate requirement (b)** (§2): alpha.15 names *"the web-player developer token"* and *"MusicKit private keys"* outright. The policy's own exemplar teaches authors to disclose. |
| `.github/release-notes/README.md:24` | *"write what the **end-user** should see at the top of the GitHub Release page"* | No confidentiality constraint anywhere in the tier-1 authoring instructions. |

### 1.2 EXACT replacement text — new STYLE_GUIDE section

Insert **after** "Hard bans" (i.e. after `STYLE_GUIDE.md:28`) as a new top-level section:

```markdown
## Never reveal how a feature is delivered

Release notes advertise **capabilities**, never **mechanisms**. Some of MeedyaDL's features
depend on non-obvious acquisition paths (how we obtain data, credentials, or media from a
service). Those paths are operational IP: describing them in release notes makes them easier
to find, easier to copy, and easier for the upstream service to close. This rule applies even
when the sentence is otherwise perfect plain English — a mechanism disclosure written at a
13-year-old's reading level is still a disclosure.

Never state, name, or paraphrase, in any visible OR collapsed part of a release body:

- **Credentials and tokens** we obtain or store on the user's behalf: developer tokens,
  web-player tokens, user tokens, JWTs, private keys, or where they are kept.
- **Acquisition paths**: which API, endpoint, page, or service a feature's data comes from;
  that something is "fetched", "extracted", "captured", or "read" from a service; retry or
  fallback behaviour against a service.
- **Protocol/stream internals**: playlist/stream formats, decryption steps, ports and
  addresses (except the user-configurable wrapper addresses, which are Settings labels).
- **Storage/crypto internals**: algorithms, iteration counts, key derivation, database
  engines, file names on disk. "Encrypted and password-protected" is the ceiling of detail.

The test: **could a competitor or the upstream service learn anything about our
implementation from this sentence that they could not learn from using the app's UI?**
If yes, cut it — describe only what the user sees.

| ✅ Say this | ❌ Never this |
|---|---|
| "Lyrics now highlight word-by-word in time with the music." | Anything naming the token, endpoint, API, or lyrics-data source used to obtain the timing. |
| "Word-by-word synced lyrics download more reliably." | "MeedyaDL now fetches syllable lyrics from Apple's lyrics endpoint when GAMDL's output lacks word timing." |
| "Animated cover art now downloads for more albums." | "Animated artwork is retrieved using the web-player developer token captured at login." |
| "Sign in to Apple Music once and premium extras just work." | "Your web-player token is extracted during login and stored in the system keychain." |
| "Your exported backup is encrypted and password-protected." | "…encrypted with AES-256-GCM + PBKDF2-HMAC-SHA256 (600 000 iterations)." |
| "MeedyaDL finds your albums instantly, even in huge libraries." | "…thanks to the new SQLite index that mirrors history.json." |
| "A hidden developer mode unlocks early features." | "Enter the Konami code to open the developer unlock." *(pending owner decision D5)* |

If a bullet cannot be written without the mechanism, the change is not user-facing — move it
to `Release-Note: none` and let the technical record live in CHANGELOG.md.
```

### 1.3 Edits to "Allowed user vocabulary" (`STYLE_GUIDE.md:30-36`)

Replace the section body with an explicit **admission test** plus the (unchanged) list:

```markdown
A term is allowed **if and only if it appears verbatim as a label in MeedyaDL's own shipped
UI** (a Settings control, page name, wizard step, or dropdown option). Users need those exact
words to find the control being described. Nothing else is admitted by analogy — in
particular, a term being "well known" or "already public in our source code" does NOT admit
it (the repository is public; release notes are still the only place we *advertise*).

Allowed: Apple Music, GAMDL, wrapper (and the three wrapper addresses as labelled in
Settings → Advanced → Wrapper), cookies, queue, Activity log, Library Scan, setup wizard,
storefront, Dolby Atmos, Lossless (ALAC), AAC, synced lyrics, MusicKit credentials (the
Settings → Authentication field label — the *user's own* credentials, never tokens MeedyaDL
obtains itself).

Explicitly NOT allowed, even though they appear in our code and docs: developer token,
web-player token, Music-User-Token, JWT, keychain, endpoint, API paths, TTML, m3u8, SQLite,
crate/library names, encryption algorithm names.
```

**Justification for the boundary rule (UI-visibility test):** GAMDL, wrapper, cookies and
storefront are all *user-operated controls* — the user must type a wrapper address, import a
cookies file, pick a storefront, and sees GAMDL's version in Settings → Tools. Omitting those
words would make the notes *less* usable ("the checks MeedyaDL runs before…" cannot name the
control to change). They disclose nothing beyond what any user of the app already sees. By
contrast the web-player token, the syllable-lyrics fetch, and the keychain storage have **no
UI surface at all** (the token machinery is deliberately invisible; even the dev-tools token
status panel is Konami-gated) — naming them gives away implementation for zero user benefit.
The test is mechanical: *is it a literal on-screen label?* — so future authors don't have to
re-litigate each term. Note the test deliberately rejects "it's in our public source code" as
an admission argument: requirement (b) is about not *advertising* the mechanism, and a GitHub
Release body (also served inside the app by the updater) is our loudest advertisement surface.

### 1.4 Edits to the translation glossary (`STYLE_GUIDE.md:39-48`)

The glossary needs a third column concept: some terms are **translated**, others are
**omitted**. Concretely:

- **Keep** `wrapper_decrypt_ip` → "the wrapper's decryption address" (UI label — Settings →
  Advanced → Wrapper), "preflight check", "enrichment", "codec priority chain" rows.
- **Change** the `TTML` row: `TTML` → *"synced lyrics"* — drop *"Apple's synced-lyrics
  format"*. Attribution of the format to Apple is the first breadcrumb of the acquisition
  path; the user-facing capability is simply "synced lyrics". *(Owner decision D6 if you feel
  "Apple's synced-lyrics format" is harmless.)*
- **Add omission rows** (translate to nothing / re-frame as capability):

| Internal term | User-facing phrasing |
|---|---|
| web-player token, developer token, Music-User-Token, JWT | *(omit — describe the capability: "word-by-word lyrics", "animated cover art", "sign in once")* |
| syllable-lyrics fetch/endpoint | "word-by-word synced lyrics" |
| keychain storage | *(omit — at most: "stored securely on your computer")* |
| AES-/PBKDF-/iteration parameters | "encrypted and password-protected" |
| SQLite / JSON / index / database file names | "MeedyaDL's download records" |
| m3u8 / HLS / stream/playlist URLs | *(omit — "downloads", "streaming quality")* |

- **Amend the closing rule** (`STYLE_GUIDE.md:48`): append — *"…and if the internal term is a
  credential, token, endpoint, or acquisition path, do not translate it: omit it (see 'Never
  reveal how a feature is delivered')."*
- **Re-anoint the gold standards** (`STYLE_GUIDE.md:112`, `README.md:50`): drop
  `v1.10.0-alpha.15.md` until scrubbed (§2); `v1.11.0-alpha.21.md` and `v1.9.1.md` are clean
  exemplars of the hardest cases (a sensitive feature announced without its mechanism; a deep
  performance fix explained purely by symptom).

### 1.5 Owner decisions (flagged)

- **D1 — the collapsed "Full technical changelog" `<details>` block.** Tier-2 bodies embed raw
  commit subjects *and commit bodies* (see §3 L2) one click away in the same release body.
  Strictly, that violates (b); pragmatically, the repo is public and CHANGELOG.md carries the
  same content by stated policy. Options: (i) keep as-is; (ii) **replace the inline dump with a
  link to CHANGELOG.md / the compare view (recommended — removes the surface from the
  *advertised* body without losing the developer record)**; (iii) keep subjects, drop bodies.
- **D2 — retro-scrub `v1.10.0-alpha.15.md`** (the one outright mechanism disclosure, §2) and
  re-apply to the published release, vs. leaving shipped history untouched. Recommended: scrub
  — the file is cited as the gold standard, so it actively teaches disclosure.
- **D3 — CHANGELOG.md stays technical.** Current policy says yes. Requirement (b) as stated
  covers *release notes*; confirm CHANGELOG.md (and commit messages) are out of scope.
- **D4 — "MusicKit credentials".** User-supplied Apple Developer credentials have a Settings
  UI, so the UI-visibility test admits the term; but it edges toward the token domain. This
  audit admits the field label and bans "MusicKit private key(s)". Confirm.
- **D5 — "Konami code".** Named in `v1.11.0-alpha.18.md:19`; `v1.11.0-alpha.21.md` shows the
  compliant phrasing ("hidden developer-access unlock"). Is the unlock method itself
  confidential, or a deliberate Easter egg? Lint seed treats it as a **warning**, not error.
- **D6 — "Apple's synced-lyrics format"** in the current glossary — keep or reduce to "synced
  lyrics" (audit recommends reducing; see §1.4).
- **D7 — legacy tags** (nightlies, `v0.x`, `v2.0.0-alpha.*`, ~29 tags): prior diagnosis
  recommended skipping; this audit concurs. Confirm wontfix.

---

## 2. Audit of the committed curated notes (all 30 files)

Grading: **good** = passes both requirements · **needs-rewrite** = plain-English/jargon
failures (requirement a) · **discloses-mechanism** = requirement (b) violation. Severity order.

| File | Verdict | Offending phrases (quoted) |
|---|---|---|
| `v1.10.0-alpha.15.md` | **discloses-mechanism** (P0) + needs-rewrite | Line 7: *"Sensitive material — Apple Music cookies, **MusicKit private keys, the web-player developer token** — is encrypted at rest with **AES-256-GCM + PBKDF2-HMAC-SHA256 (600 000 iterations)**"* — names the exact token the owner cited, plus key material and crypto parameters. Also hard-ban breaches: line 9 *"`history.json` and `activity-YYYY-MM-DD.log`"*, line 14 *"missing `playParams` no longer crash with a `KeyError`"*, line 24 *"the new shared `meedya-fingerprint` crate … no `fpcalc` binary required"*, line 3/9/30 "SQLite"/"JSON". **This file is the guide's named gold standard** (`STYLE_GUIDE.md:112`). |
| `v1.11.0-alpha.18.md` | **needs-rewrite** (P1; minor mechanism) | Line 19: *"Channel-bump UI behind Dev Access (**Konami code**)"* (→ owner decision D5); line 20: *"**API/format-compatible** … **SQLite library index** … (**settings v7 → v6 migration is a no-op**)"* — internal jargon. Also a gold-standard citation. |
| `v1.10.1.md` | **good** (P2 watch) | Line 7: *"recognises the difference between **syllable-level and word-level timing in Apple's synced-lyrics data**"* — describes the data distinction, not the acquisition path; borderline under the strictest reading of (b). Acceptable; would fail the proposed lint's "Apple's … lyrics data" warning tier only. |
| `v1.3.0.md` | good | *"wrapper decryption address setting"* — UI label (Settings → Advanced → Wrapper); allowed under the boundary rule. |
| `v1.12.0-alpha.32.md` | good | "wrapper sign-in", "Apple ID password" describe the visible sign-in feature; no acquisition-path detail. |
| `v1.11.0-alpha.21.md` | good — **model file** | Announces hidden Spotify downloading with risk framing and *without* naming engine, unlock keystrokes, or rate-limit implementation. |
| `v1.0.0.md` | good | *"the underlying downloader engine"* — exemplary abstraction. |
| `v1.10.0.md` | good | — |
| `v1.11.0-alpha.19.md` / `.20` / `.22` / `.23` / `.24` / `.25` / `.26` / `.27` / `.28` / `.29` / `.30` / `.31` | good | alpha.26's *"sensitive wrapper connection details are better protected in diagnostic exports"* is capability-level; alpha.30/.31 files are clean — the owner's complaint about those two tags is about the **published bodies**, not these files (§5). |
| `v1.3.1.md` / `v1.4.1.md` / `v1.4.4.md` / `v1.4.5.md` / `v1.5.0.md` | good | v1.4.4's *"converts it to the equivalent Apple Music link behind the scenes"* is a harmless user-level description. |
| `v1.9.0.md` / `v1.9.1.md` / `v1.9.2.md` / `v1.9.3.md` / `v1.9.4.md` | good | v1.9.0's *"`{platform}` template chip"* is a UI element. v1.9.1 is a model symptom-first deep-fix writeup. |

**Totals: 30 files — 27 good, 1 needs-rewrite (`v1.11.0-alpha.18.md`), 1 discloses-mechanism
(`v1.10.0-alpha.15.md`), 1 borderline-watch (`v1.10.1.md`).** The corpus is healthy; the two
failures are precisely the two files the policy holds up as gold standards — fix the exemplars
and the corpus stays healthy.

---

## 3. Pipeline audit — can tier-2 emit mechanism disclosure or commit-speak?

### 3.1 Trailer flow (the intended path)

1. PR body ends with `Release-Note: <line>` — gated for **presence only** by
   `release-note-gate.yml:99` (`grep -qE '^Release-Note: \S'`). **No content check.**
2. Squash-merge (PR_BODY) lands it as a commit footer; direct pushes get an advisory-only
   presence scan (`release-note-gate.yml:189-199`).
3. git-cliff parses footers; both templates select `footer.token == "Release-Note"`
   (`cliff-eli5-body.tera:6-8`, `cliff-cumulative-body.tera:14-16`), take the first line
   (`split(pat="\n") | first` — multi-line leakage impossible), and render it **verbatim**
   into the What's new/fixed/Performance/Notes sections (`cliff-eli5-body.tera:23-39`,
   `cliff-cumulative-body.tera:37-52`).
4. `release.yml` composes the body (`:359-363` ELI5 render, `:402-406` cumulative,
   `:408-435` prerelease assembly, `:376-391` stable) and ships it via `gh release
   create/edit` (`:481`, `:487-491`). `update_checker.rs` then serves that body in-app.

### 3.2 Leak paths (every place internal text can reach a published body)

| # | Path | Evidence | Risk |
|---|---|---|---|
| **L1** | **Verbatim trailers, unlinted.** A trailer written as *"Fixed web-player token refresh so animated artwork downloads again"* passes the presence-only gate and is rendered verbatim. | `release-note-gate.yml:95-104` (presence only); `cliff-eli5-body.tera:8-19` (verbatim) | **High** — this is the main authoring surface and has zero content enforcement. |
| **L2** | **`<details>` "Full technical changelog" embeds raw commit subjects AND commit bodies.** `cliff.toml`'s body template includes `{{ commit.message }}` *and* `{% if commit.body %}…{{ commit.body … }}` (`cliff.toml:71-72`); `release.yml` cats that render into the release body at `:381-387` (stable) and `:428-433` (prerelease), stripping only the `## [x.y.z]` header line (`grep -v '^## \['`). Feature-commit bodies routinely describe acquisition mechanics in detail. | `cliff.toml:60-75`; `release.yml:367,381-387,428-433` | **Medium** (collapsed, but in the published body and the in-app-served string) → owner decision D1. |
| **L3** | **Untrailered raw subjects — largely closed, one residue.** The per-release ELI5 template silently *drops* untrailered commits (no fallback branch in `cliff-eli5-body.tera` — coverage gap, not a leak). The cumulative template's "Under the hood" fallback emits only a **count + PR links**, never subjects (`cliff-cumulative-body.tera:53-55`, by design per #1033). Residue: the rendered *link text* is just `#NNNN`, but GitHub's hover-card/link expansion shows the raw PR title to the reader. | `cliff-cumulative-body.tera:32-35,53-55` | Low. |
| **L4** | **Self-heal is format-aware, not content-aware.** `detect-commit-speak.py:48-71` approves any body containing `### What's new` etc. — a mechanism-disclosing but well-formatted body is "healthy" and will never be auto-repaired; conversely a self-heal *regeneration* re-runs tier-2 and re-embeds L1/L2 output (`release.yml:474-482`). | `detect-commit-speak.py:48-54` | Medium. |
| **L5** | **`draft-notes.sh` SOURCE block.** The draft embeds the *full technical changelog* inside an HTML comment (`draft-notes.sh:163-167`, *"SOURCE (delete before publishing)"*). If a human renames `.draft` → `.md` without deleting it, `release-note-gate.yml:142` (checks only `^### `) passes, and the comment ships inside the raw body — invisible in GitHub's rendered page but present via API, "view source", and the in-app `release_body` string. | `draft-notes.sh:156-167`; gate `release-note-gate.yml:137-145` | Low-medium (silent, easy to catch with lint). |
| **L6** | **Curated files & splices are applied verbatim.** `apply-notes.sh:72` / `splice-body.py` / `release.yml:250-254` apply tier-1 files with no content check — the committed-file surface audited in §2 has no mechanical guard either. | `apply-notes.sh:66-81`; `release.yml:250-254` | Medium — mitigated by §4. |

Tier-3 static fallback (`release.yml:459-471`) and the placeholder machinery are clean.
`filter_unconventional = true` (`cliff.toml:103`) drops free-form commits entirely.

**Verdict:** the #1033/#1046 work successfully closed the *raw-subject-above-the-fold* class.
What remains open is (i) unlinted verbatim trailer content (L1), (ii) the collapsed technical
dump (L2, policy decision), and (iii) no content linting anywhere for curated files (L6) —
`grep -rn lint scripts/release-notes/ .github/workflows/release-note-gate.yml` → zero hits.

---

## 4. Enforcement design — `scripts/release-notes/lint-notes.py`

A single denylist-driven linter, runnable locally and in CI, over BOTH authoring surfaces:
`Release-Note:` trailer lines and `.github/release-notes/*.md` files.

### 4.1 Interface

```
lint-notes.py [--trailer] [--strict] [FILE ...]     # files: notes .md
echo "$LINE" | lint-notes.py --trailer              # stdin: one trailer line per line
```

- Exit 1 on any **error**-tier finding; **warning**-tier findings print `::warning` and exit 0
  (exit 1 under `--strict`). Findings printed as `path:line — [tier] rule: matched 'text'`.
- Skips fenced/inline code inside `<details>`? **No** — lints the whole file including HTML
  comments (that is what catches L5), but *excludes* the workflow-appended download footer
  (starts at `## Choose your download`) when pointed at a live body.
- Allowlist in a module-level list (product names that defeat the CamelCase heuristic):
  `MeedyaDL, MeedyaSuite, MeedyaConverter, MeedyaManager, GitHub, MusicKit, LRCGET, LRCLIB,
  YouTube, SoundCloud, AppImage, ChromeOS, VoiceOver, NVDA, MacBook, iTunes, iPlayer,
  MusicBrainz, AcoustID, ReplayGain, Raspberry Pi, Dolby, PyPI` (extend in-repo, reviewed).

### 4.2 Denylist seed

**Tier: error — mechanism / acquisition (requirement b):**

```
(?i)\b(developer|web[- ]?player|user|bearer|access|auth)\s+token\b ; \btoken\b (standalone, case-insensitive)
(?i)\bMusic-User-Token\b ; \bJWT\b ; \bprivate key\b ; \.p8\b
(?i)\bkeychain\b ; (?i)\bendpoint\b ; \b/v[0-9]+/ ; amp-api ; api\.music\.apple ; syllable-lyrics
(?i)\b(scrape[sd]?|scraping)\b ; (?i)\bextract(s|ed|ing|ion)?\b.{0,40}\b(token|cookie|credential)s?\b
(?i)\bm3u8\b ; \bHLS\b ; \bTTML\b ; (?i)\bdecrypt(ion|ing|ed)?\b(?!.{0,20}address) # allow the Settings label "decryption address"
(?i)\bAES-[0-9]+ ; \bPBKDF ; \bSHA-?[0-9]+ ; [0-9][0-9\s]*iterations
(?i)\bSQLite\b ; \bWebView\b ; (?i)\bJavaScript evaluation\b
```

**Tier: error — commit-speak / code identifiers (requirement a, mechanises the hard bans at `STYLE_GUIDE.md:16-27`):**

```
\b[a-z0-9]+_[a-z0-9]+_?[a-z0-9]*\b            # snake_case (outside allowlist)
--[a-z][a-z0-9-]+\b                           # CLI flags
\b\w+\(\)                                     # function calls foo()
`[^`]*[_(){}]`                                # backticked identifiers containing code chars
^## \[[0-9]                                   # raw git-cliff version header (from #1046 detector)
^\s*-\s*\*\*\([a-z-]+\)\*\*                   # verbatim scope prefix bullets "**(wrapper)**"
\(#[0-9]+(,\s*#[0-9]+)*\)$                    # bare issue-list citations as content
(?i)\b(refactor|wire[- ]up|hoist)\b           # commit jargon (subset; "emit/guard/gate" are common English — warning tier)
<!--\s*SOURCE                                  # draft-notes.sh residue (closes L5)
```

**Tier: warning (human review, `--strict` in nightly/manual runs):**
`(?i)Konami` (owner decision D5) · `(?i)\bAPI\b` · `(?i)\bJSON\b` · `(?i)\bdatabase\b` ·
`(?i)Apple's .{0,20}(data|format|service)` · `(?i)\b(emit|guard|gate)\b` ·
`\b[A-Z][a-z]+[A-Z]\w*\b` CamelCase heuristic (noisy → warning).

The seed is calibrated against §2: it flags every quoted offender in `v1.10.0-alpha.15.md` and
`v1.11.0-alpha.18.md`, and produces zero errors on the other 28 files (`decryption address`,
`wrapper`, `cookies`, `storefront`, `GAMDL` all pass; `v1.10.1.md` line 7 trips only the
`Apple's … data` warning).

### 4.3 Wiring

1. **`release-note-gate.yml` job 1 (`pr-trailer`)** — after the presence grep at `:99`, add:
   `printf '%s' "$PR_BODY" | grep -E '^Release-Note: ' | sed 's/^Release-Note: //' | python3 scripts/release-notes/lint-notes.py --trailer` (checkout step required; job currently has
   none). **Blocking** — closes L1 at the moment the author is looking.
2. **`release-note-gate.yml` job 2 (`release-pr-notes-file`)** — after the `^### ` shape check
   at `:142-145`, add `python3 scripts/release-notes/lint-notes.py "$NOTES_FILE"`. Blocking.
3. **New job 4 (`notes-file-lint`)** — `on: pull_request` when `paths:
   ['.github/release-notes/**.md']`: lint every changed notes file (excluding STYLE_GUIDE.md /
   README.md). Blocking — closes L6 for all future curated files, including prerelease
   backfills that never transit job 2.
4. **Job 3 (`push-trailer-advisory`)** — pipe each found trailer through `--trailer` as an
   additional `::warning` (advisory, consistent with the job's can't-un-push stance).
5. **`apply-release-notes.yml`** (§5) — lint before applying (blocking).
6. **Local**: `python3 scripts/release-notes/lint-notes.py .github/release-notes/v*.md`, plus
   an npm alias `check:release-notes` alongside `check:legal`. Pure-stdlib Python 3 — no deps,
   same runtime contract as `tools/audit-checks/*.py` (zero-finding on a clean tree; add a
   negative test when changing).
7. **One-time**: run over the full corpus in CI once the §2 rewrites land, so the tree starts
   clean and any regression is a red X.

Out of scope for the linter (flagged, not solved): L2 requires the D1 policy decision, and
published-body drift requires the §5 workflow re-run rather than a PR-time check.

---

## 5. Retrospective-fix plan

### 5.1 Which published tags need their bodies rewritten

229 tags exist; curated files exist for 30. Inventory (extends the #1046 diagnosis §3.1 with
requirement (b) and the post-backfill state — the backfill commit `51b0c93f` landed the
alpha.30/.31/.32 **files** on `alpha` at 15:40 today, but nothing has re-applied them to the
**published release bodies**, which is why the owner still sees them as poor):

| Priority | Tags | State | Action |
|---|---|---|---|
| **P0** | `v1.11.0-alpha.30`, `v1.11.0-alpha.31` | Confirmed-bad live bodies (bare bump noise / old-format commit-speak dump — diagnosis §1.2c-d); clean curated files now committed | Apply curated files to live bodies (workflow below) |
| **P0** | `v1.10.0-alpha.15` | Live body presumed = curated file → **published mechanism disclosure** ("web-player developer token", "MusicKit private keys", crypto parameters) | Rewrite file per §2 (pending owner D2), re-apply |
| **P1** | `v1.12.0-alpha.32` | Curated file committed today; live body predicted ELI5-empty (diagnosis §2.1) | Apply |
| **P1** | `v1.11.0-alpha.18` | Jargon + Konami naming (§2) | Rewrite file, re-apply |
| **P1** | `v1.12.0-alpha.33/.34/.35` (+ `.36`, currently a **stuck unpublished draft** per `c15264ae`; `.37` cutting from today's tip) | No curated files; machinery ran — verify format, likely acceptable; `.36` additionally needs `gh release edit --draft=false` after verification | Verify via `gh release view`; curate only if commit-speak or disclosure found |
| **P2 (skip — owner D7)** | ~29 legacy: nightlies, `v1.9.4-alpha.9-12`, `v1.10.0-alpha.13/14/16/17`, `v2.0.0-alpha.1-8`, `v1.0.0-rc.1`, all `v0.x` | Superseded, near-zero traffic | Leave |
| Stables | `v1.0.0`…`v1.10.1` | Clean curated files exist; verify live bodies match (v1.5.0 / v1.10.1 were historically bad and backfilled) | One idempotent pass of the workflow below confirms/fixes at zero cost |

Net: **2 mandatory format repairs + 1 mandatory content scrub + 2-5 verify/apply**, everything
else covered by one idempotent "all" pass.

### 5.2 Recommended mechanism: `.github/workflows/apply-release-notes.yml` (new, lightweight)

Re-running `release.yml` per tag rebuilds all 6 platform matrices (20+ min each) just to reach
the ~30-second `ensure-release` body edit — wasteful and risky (a full re-run also re-triggers
signing/notarisation and the finalize steps). Everything needed already exists as scripts:
`apply-notes.sh` (idempotent: diff-then-edit, `apply-notes.sh:74-77`; `gh` available on
runners) and `splice-body.py` (footer-preserving: keeps everything from `\n---\n\n## Choose
your download` onward byte-for-byte, `splice-body.py:57-69`). Spec:

```yaml
name: Apply Release Notes
on:
  workflow_dispatch:
    inputs:
      tag:
        description: "Tag to apply (vX.Y.Z[-channel.N]) or 'all' for every committed notes file"
        required: true
      dry_run:
        description: "Report intended edits without calling gh release edit"
        type: boolean
        default: false
permissions:
  contents: write        # gh release edit; no packages/id-token/etc.
concurrency:
  group: apply-release-notes
  cancel-in-progress: false
jobs:
  apply:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pinned-sha>          # repo convention: SHA-pinned
      - name: Validate input tag shape               # reuse release.yml:181 regex + literal 'all'
        env: { INPUT_TAG: "${{ inputs.tag }}" }      # env-var passing, never ${{ }} in run: (repo convention)
      - name: Lint notes files                       # python3 scripts/release-notes/lint-notes.py
        # 'all' → lint every .github/release-notes/v*.md; blocking. Closes the "backfill
        # bypasses the PR gate" hole for this workflow's own inputs.
      - name: Apply
        env: { GH_TOKEN: "${{ secrets.GITHUB_TOKEN }}", INPUT_TAG: ..., DRY_RUN: ... }
        run: |
          # tag mode:  bash scripts/release-notes/apply-notes.sh "$INPUT_TAG"
          # all mode:  for f in .github/release-notes/v*.md; do
          #              TAG="$(basename "$f" .md)"
          #              gh release view "$TAG" &>/dev/null || { echo "skip $TAG (no release)"; continue; }
          #              bash scripts/release-notes/apply-notes.sh "$TAG"
          #            done
          # dry-run: same loop but stop after the splice + diff, print the diff to the step summary.
      - name: Summary                                # per-tag applied / already-current / skipped table
```

Properties: **idempotent** (apply-notes.sh exits 0 without editing when the spliced body
already matches — re-running "all" is free); **footer-preserving** (shared `splice-body.py`
regex — the same one `release.yml:476-481` self-heal uses, so no drift); **draft-safe** (`gh
release edit --notes-file` works on drafts, so the stuck `alpha.36` draft can be fixed before
being published); **cheap** (single ubuntu job, ~seconds per tag, zero build minutes);
**auditable** (`dry_run` + step summary). Operational caveats: (i) until #1040 Phase 3 syncs
`main`, dispatch with `--ref alpha` from the CLI (`workflow_dispatch` UI buttons only appear
for workflows on the default branch — documented in CLAUDE.md "Conserving GitHub Actions
Minutes"); (ii) the workflow must be dispatched from a ref that *contains* the curated files
(they live on `alpha` today); (iii) add the workflow to the sensitive-paths list in
`pr-security.yml` check 7 by virtue of living under `.github/workflows/` (automatic).

Sequencing: **1)** land the §1 STYLE_GUIDE changes + §2 rewrites (`alpha.15`, `alpha.18`,
pending D2/D5) + §4 linter + this workflow in one PR to `alpha` → **2)** dispatch
`apply-release-notes.yml` with `tag=all` (`--ref alpha`) → repairs alpha.30/.31, applies
alpha.32, scrubs alpha.15, and verifies every stable in one run → **3)** eyeball
`alpha.33-.36` bodies, un-draft `.36`, curate only if needed → **4)** fold the linter's
denylist into the drafting habit (draft-notes.sh already tells authors to polish per
STYLE_GUIDE; the gate now enforces it).

---

*Audit by Claude (Fable 5), 2026-07-24, branch `audit/release-notes` @ `497e3222`
(origin/alpha). All file:line references are against that tree unless a tag is named.*
