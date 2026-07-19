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

## Allowed user vocabulary

These terms are part of MeedyaDL's own product vocabulary and are fine to use as-is, because they're the words users already see in the app:

Apple Music, GAMDL, wrapper, cookies, queue, Activity log, Library Scan, setup wizard, storefront, Dolby Atmos, Lossless (ALAC), AAC, synced lyrics.

When referring to a specific screen or control, name it exactly as it's labelled in the UI — for example "Settings → Advanced → Wrapper", not "the wrapper settings panel."

## Translation glossary (code → user)

| Internal term | User-facing phrasing |
|---|---|
| `wrapper_decrypt_ip` | "the wrapper's decryption address" |
| TTML | "Apple's synced-lyrics format" |
| preflight check | "the checks MeedyaDL runs before starting a download" |
| enrichment | "the extra metadata/lyrics/artwork steps after a download" |
| codec priority chain | "your audio-quality fallback order" |

When you hit an internal term that isn't in this table, translate it the same way: describe the user-observable effect, not the internal name.

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

Gold-standard references: `v1.10.0-alpha.15.md` and `v1.11.0-alpha.18.md` in this directory.
