---
name: project-issue-sweep-2026-09-21
description: A full GitHub issues sweep (63 open + 611 closed) checked against the code on alpha, not against what the issues or the docs claimed — 15 issues had to be reopened because the code did not back up the closing comment
metadata:
  type: project
---

# The 21 September 2026 issue sweep

Every issue on this repository — all 63 that were open and all 611 that were
closed — was checked one more time. The check was against the actual code on
`alpha` at commit `bfb7b25f`, not against what the issue said, what a previous
closing comment said, or what a project document said. That distinction
matters: several issues were closed with a comment that turned out not to
match the code at all.

## How it was done

A team of Fable agents worked through the issues in batches (the open ones as
one batch, the closed ones split into seven batches of roughly 88), each agent
reading every issue's full body and comment history and then reading the
relevant source file at the line the issue pointed to, rather than trusting
the issue's own description of what the code does. A separate Opus agent then
re-checked every proposed close and every proposed reopen — the two verdicts
with the most consequence — against the same code, before anything was acted
on. It overruled two proposed closes (#295 and #995, which got comments
instead) and agreed with all fifteen reopens. A Sonnet agent then carried out
the agreed changes on GitHub, and a separate Sonnet agent corrected this
project's own notes (recorded here and in `.claude/CLAUDE.md`).

## The totals

Out of 674 issues checked (63 open, 611 closed):

- **39** got an explanatory comment — the issue's own state (open or closed)
  was correct, but the closing note or the current description no longer
  matched the code, so a status check was posted to say what is actually true
  today.
- **5** open issues were closed as genuinely done, because the code now fully
  matches what they asked for: **#1034** (the last open finding from the
  full-codebase security audit — the embedded MusicKit token was removed),
  **#1072** (the browser User-Agent version is now injected at build time
  instead of hand-maintained), **#1075** (the in-app Help screen now loads
  every page straight from `help/*.md` instead of a second, hand-typed copy),
  **#1162** (an empty developer-access passphrase is now refused instead of
  silently accepted), and **#1176** (music videos now step down through video
  codecs the same way audio does).
- **15** closed issues were reopened, because the code does not do what the
  closing comment said it does:
  - **#95** — the MusicBrainz storefront-rewrite and the AcoustID-to-MusicBrainz
    bridge are both dead code; only the plain ID lookup half is actually used.
  - **#216** — the check that is supposed to detect a pre-release build looks
    for a version starting with `"0."`, so every build since 1.0 (including
    every alpha, beta, and RC) is now wrongly treated as a full release.
  - **#273** — only 2 of the 5 external tools (FFmpeg, N_m3u8DL-RE) are ever
    checked for an update; the other 3 just sit there once installed.
  - **#329** — ReplayGain tagging for MKV/WebM/OGV video files always fails
    silently; the tag-writing library in use cannot open those containers.
  - **#352** — the shared codec-detection crate was never wired in beyond one
    file; codec detection is still done locally, same as before the issue.
  - **#387** — the "What's New" modal is gated behind the same broken `"0."`
    pre-release check as #216, so it never shows on any current build either.
  - **#393** — only half of "WebView memory auto-mitigation" landed (writing a
    session log); nothing actually watches memory use or reacts to it.
  - **#397** — the "pip install integrity" fix only logs where a package was
    installed; nothing computes or checks a hash, so nothing is verified.
  - **#423** — BBC iPlayer settings were never added to the shared per-service
    settings struct; the work was left on a branch that no longer exists.
  - **#424** — the same thing, for shared dependency management between
    engines; also lost on a branch that no longer exists.
  - **#426** — per-service sign-in status was never built for anything beyond
    Apple Music's own wrapper sign-in.
  - **#431** — the frontend wrappers and types for multi-service commands
    were never added; the store meant to use them is still switched off.
  - **#759** — enrichment gap-fill can scan and show what is missing, but
    nothing re-runs a missing step, and the promised follow-up issue for that
    was never filed.
  - **#949** — the milestone-number fix only reached the two Help pages users
    actually see; four other files still have the numbers scrambled.
  - **#984** — the checksum check for the offline-installer bundle never
    actually runs, because it looks for a filename the mirror does not
    publish.
- **2** proposals to close were overruled by the Opus re-check and left open
  with a status comment instead: **#295** (song.link integration — the local
  half is done, but the remote on/off switch the reopen note asked for was
  never built) and **#995** (channel release workflows no longer re-resolve
  every dependency on `alpha`, but a manual run of the same workflow on `main`
  still would, because `main`'s copies were never updated).
- The rest — **615** issues — needed no change at all: the open ones are
  still genuinely open, and the closed ones are still genuinely done.

## The project board

Separately, GitHub Project 6 ("MeedyaDL Development") was brought in line
with this state: **38** open issues that had no board entry were added, and
**74** existing entries were corrected — including **32 closed issues that
were still showing "In Progress"**, plus 4 open issues wrongly showing
"Done" (#216, #229, #273, #329 — all now correctly "Todo" following the
reopens/status above).

## The lesson

Fifteen issues had been marked closed as done, but the feature the closing
comment described was not actually in the code. A closing comment is a claim
about what the code does, and like any such claim it needs checking against
the code itself, not taken on trust because someone wrote "done" at the time.
This is the same hazard as a comment inside the code that asserts something
false, just moved from a `//` line to a GitHub comment box. See
[[project-never-worked-pattern]] for the shape this takes when a feature
never worked in the first place, and [[project-comment-accuracy-hazard]] for
the same discipline applied to comments written directly in the code.
