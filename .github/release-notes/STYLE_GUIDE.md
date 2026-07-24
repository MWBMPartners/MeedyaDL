# Release Notes Style Guide (ELI5)

MeedyaDL's release notes are written for the person who **uses** the app and has never seen the code. This guide defines the voice, the vocabulary, and the mechanics that turn a `Release-Note:` git trailer into a line a user actually understands.

## The one rule

> Write for someone who uses MeedyaDL but never saw the code.

Aim for roughly a 13-year-old's reading level. Every single line must answer one of two questions:

- **"What will I notice?"**
- **"What can I now do?"**

Lead with the symptom or the benefit. Never lead with the mechanism. If a sentence starts by describing *how* something was built, rewrite it to start with *what changed for the user*.

## Hard bans (in visible text)

None of the following may appear in a `Release-Note:` trailer or in a published release note:

- File names, function names, or other code identifiers
- `snake_case` or `CamelCase` tokens
- Crate/library names (`mp4ameta`, `lofty`, `reqwest`, etc.)
- CLI flags (`--wrapper-decrypt-host`), INI keys (`wrapper_decrypt_ip`), or regex patterns
- Commit-type jargon: "refactor", "emit", "guard", "gate", "wire up", "hoist", and similar
- Issue lists used as the content itself (e.g. "closes #935, #937, #940" is not a release note — it's a citation)
- Scope prefixes copied verbatim from commit subjects, e.g. `**(wrapper)**`

If a bullet needs any of the above to make sense, it hasn't been translated yet.

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
| "A hidden developer mode unlocks early features." | "Enter the Konami code to open the developer unlock." |

If a bullet cannot be written without the mechanism, the change is not user-facing — move it
to `Release-Note: none` and let the technical record live in CHANGELOG.md.

## Allowed user vocabulary

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

**Justification for the boundary rule (UI-visibility test):** GAMDL, wrapper, cookies and
storefront are all *user-operated controls* — the user must type a wrapper address, import a
cookies file, pick a storefront, and sees GAMDL's version in Settings → Tools. Omitting those
words would make the notes *less* usable ("the checks MeedyaDL runs before…" cannot name the
control to change). They disclose nothing beyond what any user of the app already sees. By
contrast the web-player token, the syllable-lyrics fetch, and the keychain storage have **no
UI surface at all** (the token machinery is deliberately invisible; even the dev-tools token
status panel is behind a hidden developer mode) — naming them gives away implementation for
zero user benefit. The test is mechanical: *is it a literal on-screen label?* — so future
authors don't have to re-litigate each term. Note the test deliberately rejects "it's in our
public source code" as an admission argument: the "Never reveal how a feature is delivered"
rule above is about not *advertising* the mechanism, and a GitHub Release body (also served
inside the app by the updater) is our loudest advertisement surface.

When referring to a specific screen or control, name it exactly as it's labelled in the UI — for example "Settings → Advanced → Wrapper", not "the wrapper settings panel."

## Translation glossary (code → user)

The glossary has two outcomes, not one: some internal terms are **translated** into plain
English, others must be **omitted** entirely because the internal term itself is the
mechanism.

| Internal term | User-facing phrasing |
|---|---|
| `wrapper_decrypt_ip` | "the wrapper's decryption address" |
| TTML | "synced lyrics" |
| preflight check | "the checks MeedyaDL runs before starting a download" |
| enrichment | "the extra metadata/lyrics/artwork steps after a download" |
| codec priority chain | "your audio-quality fallback order" |
| web-player token, developer token, Music-User-Token, JWT | *(omit — describe the capability: "word-by-word lyrics", "animated cover art", "sign in once")* |
| syllable-lyrics fetch/endpoint | "word-by-word synced lyrics" |
| keychain storage | *(omit — at most: "stored securely on your computer")* |
| AES-/PBKDF-/iteration parameters | "encrypted and password-protected" |
| SQLite / JSON / index / database file names | "MeedyaDL's download records" |
| m3u8 / HLS / stream/playlist URLs | *(omit — "downloads", "streaming quality")* |

When you hit an internal term that isn't in this table, translate it the same way: describe
the user-observable effect, not the internal name — and if the internal term is a credential,
token, endpoint, or acquisition path, do not translate it: omit it (see "Never reveal how a
feature is delivered").

## Sections (fixed order, omit when empty)

1. **What's new** — features you can see or switch on.
2. **What's fixed** — symptom first, always. What was broken, and what happens now instead.
3. **Performance** — faster or lighter, plus the effect you'd actually notice.
4. **Notes** — compatibility, changed defaults, experimental toggles, channel context, upgrade caveats.

Omit any section that has nothing in it. Don't write "### Performance" followed by "no changes this release" — just leave the heading out.

## Shape

- **Title**: `# MeedyaDL <version>`
- **Summary**: one to two sentences immediately under the title, framing the release for a user skimming the GitHub Releases page.
- **Bullets**: one to two sentences each, roughly 200 characters. Bold the headline phrase for anything significant (a new feature, a fix for something widely hit).
- **Issue links**: at most one per bullet, placed at the **end**, formatted as `([details](<url>))`. Never a bare `(#1026)`. Omit the link entirely for minor items — not every bullet needs one.
- **Deps-only / internal-only releases**: skip the sections entirely and write a single line: *"Under-the-hood housekeeping and dependency updates — nothing changes in how you use the app."*
- **Technical detail** belongs only in the collapsed "Full technical changelog" block (a `<details>` section) or the linked issue — never inline in a bullet.

## `Release-Note:` trailers (for PR authors)

Every `feat`/`fix`/`perf` PR body must end with one `Release-Note:` line per user-visible change it introduces:

```
Release-Note: Fixed wrapper connections for people running the wrapper on another computer while on an older GAMDL version.
```

If the PR has no user-visible effect (internal refactor, test-only change, dependency bump with no behaviour change), write:

```
Release-Note: none
```

Rules for the trailer itself:

- One line. Only the first line is used if you accidentally wrap it.
- No markdown headings inside the trailer.
- Written exactly the way it should appear in the release notes — it is used **verbatim**, per this guide's rules above.
- No link inside the trailer. The text must stand on its own; issue links are added separately when the release notes are assembled.

## Before → After examples

**1.**
Before (commit subject): `**(wrapper)** Emit wrapper_decrypt_ip in the wrapper-v1 INI branch (#1026)`
After: *"Fixed wrapper connections for people running the wrapper on another computer (such as a Raspberry Pi) while on an older GAMDL version."*

**2.**
Before: `**(library-scan)** Stop leaking manifest keys + detect hidden artwork (#989, #990)`
After: *"The Library Scan page no longer shows confusing internal codes in album rows, and now finds cover art even when MeedyaDL had hidden those files."*

**3.**
Before: `* bundled audit hardening + #935 syllable lyrics + repo hygiene (#947)`
After — this single commit splits into three bullets across three different sections:
- (What's new) *"Word-by-word synced lyrics now download more reliably."*
- (What's fixed) *"Security hardening for the components MeedyaDL bundles."*
- (Notes) *"Housekeeping to keep our releases building smoothly."*

**4.**
Before: `fix(updates): make no_compatible_wheel guard platform-aware (flag only ARMv7)`
After: *"Fixed an incorrect 'no compatible update available' warning that could appear on computers other than 32-bit Raspberry Pis."*

---

Gold-standard references: `v1.11.0-alpha.21.md` (a sensitive feature announced without its
mechanism) and `v1.9.1.md` (a deep performance fix explained purely by symptom) in this
directory.
