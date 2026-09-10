---
name: project-codebase-sweeps-2026-09
description: What the five September 2026 full-codebase sweeps covered (backend security, frontend/CI/deps, accessibility, comment accuracy, docs/licensing) — so nobody repeats them blindly, and what each was told not to flag
metadata:
  type: project
---

# The five September 2026 codebase sweeps

Across September 2026, five separate full-codebase sweeps ran over MeedyaDL,
each looking at a different slice of the project. Together they found
roughly eighty issues, all of which were fixed. This note records what each
sweep actually covered, so a future sweep does not repeat the same ground
from scratch, and — just as important — what each sweep was deliberately
told to leave alone, because several genuinely odd-looking things in this
codebase are odd on purpose.

## The five sweeps

1. **Rust backend security.** 142 files, roughly 111,000 lines of Rust,
   read for security defects — the kind of thing a compiler cannot catch
   because the code is syntactically fine and just does the wrong thing.
2. **Frontend, Tauri configuration, GitHub Actions, and dependencies.**
   The React/TypeScript side, `tauri.conf.json` and the Tauri capability
   files, the `.github/workflows/*.yml` pipeline, and the npm/Cargo
   dependency tree.
3. **Accessibility**, checked against WCAG 2.2 level AA, across all six of
   MeedyaDL's themes (the three platform themes plus the three
   accessibility themes — high-contrast, and the three colour-blind
   palettes).
4. **Comment accuracy** — reading the code behind every comment that
   asserts a checkable fact, rather than trusting the comment. See
   [[project-comment-accuracy-hazard]] for the full writeup of what this
   one found.
5. **Documentation and licensing** — README, help pages, the acknowledgment
   and third-party-licence files, checked against what the code actually
   does and what upstream projects actually declare.

## How they were run

Each sweep ran sequentially, not in parallel, using a deep-reasoning model
for the analysis pass (the part that has to actually understand what the
code does and decide whether something is wrong) and a cheaper model for
the fixes once a defect was confirmed (the part that is mostly mechanical
once the diagnosis is settled). Splitting the two steps this way kept the
expensive reasoning budget spent on judgement calls, not on typing out
fixes once the judgement call was already made.

## What each sweep was told NOT to report

This mattered as much as what they were told to look for — it cut a large
amount of noise that would otherwise have buried the real findings under
things that only look wrong to someone unfamiliar with a deliberate
decision already documented elsewhere in this repository:

- **Sending the macOS Safari User-Agent string from every platform** —
  Windows and Linux included — is deliberate. See
  [[project-apple-music-safari-identity]]. An audit that flags "the UA does
  not match the host OS" here is wrong; that mismatch is the entire point.
- **Remote feature flags failing silently and defaulting to enabled** is
  deliberate. See the "Remote feature availability" section of
  `.claude/CLAUDE.md` and [[project-remote-feature-control]] — the whole
  design rests on absence of data meaning "everything on", never an error
  state a user has to react to.
- **The feature-flag disk cache having no expiry** is deliberate, for the
  same reason: an expiring cache would silently re-enable a feature that
  was deliberately paused, the moment a user's cache happened to go stale —
  turning a deliberate pause into an accidental timer.
- **`assets/brand/` being proprietary**, not MIT like the rest of the
  codebase, is deliberate — it is a different licence for a different kind
  of asset, not an oversight to flag.

Reusing this list saves real effort: without it, several of the five sweeps
independently rediscovered the same "this looks wrong" reaction to the same
four deliberate decisions before being told to stand down.

## What counted as a finding

Each sweep was held to the same bar: a finding had to name **a specific
file, a specific line, and a concrete sequence of events by which it goes
wrong** — this input causes that output, or this comment describes
behaviour the code next to it does not actually have. "This could be
risky" or "this looks fragile" on its own was not a finding and was not
reported. That discipline is why roughly eighty findings across five
sweeps of a large codebase were almost all real and fixable, rather than a
long list of vague unease that would have taken longer to triage than to
have just fixed the code directly.

Related: [[project-never-worked-pattern]], [[project-comment-accuracy-hazard]],
[[project-audit-checks-inventory]], [[project-pr-security-checks]]
