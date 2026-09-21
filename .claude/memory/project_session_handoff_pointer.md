---
name: project-session-handoff-pointer
description: Where the canonical MeedyaDL session handoff lives (it is NOT in .claude/)
metadata:
  type: project
---

The single canonical session handoff for MeedyaDL is **`.github/HANDOFF.md`** — not a file
under `.claude/`.

Dated handoff duplicates previously kept in `.claude/memory/` and
`.OpenAI/memory/project_session_handoff_2026_07_26.md` were **deliberately deleted**
(2026-09-01 push) because they drifted out of sync with the real one and caused confusion.

**Do not recreate a handoff document under `.claude/`.** Update `.github/HANDOFF.md` instead:
add a new section at the top whose heading contains `LATEST`, the date and the topic (the file
currently uses `## ★★★★ LATEST — <date>: <topic>`), change the previous section's `LATEST` to
`Previous` so exactly one heading says `LATEST`, and refresh the `**Last updated:**`
and `**Working branch:**` lines in the header.

**Reaffirmed 2026-09-21 (#1198).** The maintainer's restated standing rules said "update our Handoff documentation in `.claude/`". Asked directly, the maintainer chose to keep `.github/HANDOFF.md` as the one handoff, for the same reason as before. So wherever a rule or brief says "the handoff in `.claude/`", it means this file's target, `.github/HANDOFF.md`.

This file exists only as a signpost for anyone (human or agent) who looks in `.claude/` first.
