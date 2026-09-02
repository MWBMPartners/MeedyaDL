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
add a new `## ★★★ LATEST — Session <date>: <topic>` section at the top, demote the previous
`LATEST` heading to plain `## ★★★ Session <date>: …`, and refresh the `**Last updated:**`
and `**Working branch:**` lines in the header.

This file exists only as a signpost for anyone (human or agent) who looks in `.claude/` first.
