---
name: feedback-plain-english-always
description: Standing rule — explain everything in plain English, never technical jargon, even to technical readers
metadata:
  type: feedback
---

**Standing rule, set by the maintainer on 2026-09-07.** When feeding anything back —
chat replies, explanations, commit messages, pull request text, issue text, release
notes, documentation, in-app help — write in plain, everyday English. Do not use
technical jargon.

**Why:** jargon is confusing *even for technically proficient developers*. The
maintainer said so explicitly. Shorthand that feels precise to the writer regularly
costs the reader time, because they have to decode it before they can judge it. A
sentence nobody has to re-read is worth more than a sentence that sounds expert.

**How to apply:**

- Say what happened and what it means for someone using or running the thing, before
  naming the machinery. "Every release build failed because two halves of the same
  toolkit were on different versions" lands; "major/minor mismatch in the Tauri
  dependency graph" does not.
- A file name, function name, or standard (WCAG, ISRC) is fine when it is genuinely
  the thing being discussed — but say in ordinary words what it is and why it matters.
- More words are fine, and better, when they make the meaning clearer. Never compress
  an explanation into jargon to save space.
- Prefer short sentences. Break long ones in two.
- Explain the "why", not just the "what".
- Avoid unexplained abbreviations and internal shorthand on first use.
- Cut filler that sounds impressive and says nothing.

**This does not lower the standard of the work.** The code, the analysis and the
precision stay exactly as rigorous. Only the way it is explained changes.

This reinforces the same rule already in the maintainer's global instructions — it is
repeated here because it was restated as a standing rule for this project
specifically. See also [[project-session-handoff-pointer]].
