---
name: project-history-rewrite-2026-09
description: On 25 Sept 2026 the whole git history was rewritten to remove the maintainer's real name, so every commit ID and tag written before then is an OLD ID that no longer resolves
metadata:
  type: project
---

# Git history was rewritten on 25 September 2026

The maintainer's real name had been written into commits and files over the life of
the project, against the standing rule to use `Salem874` only. On 25 Sept 2026 the
whole history was rewritten to take it out, and force-pushed with the maintainer's
explicit approval, in a planned window (Actions switched off and the branch
protection ruleset paused for the push, both restored afterwards and checked).

**What that means for anyone reading older material:**

- **Every commit ID changed, and all 317 tags were moved to the new commits.** A
  commit ID quoted anywhere before 25 Sept (issue comments, pull requests,
  `.github/HANDOFF.md`, audit documents, older notes) is an OLD ID. It will not be
  found. Find the change by its content instead: `git log -S'<text>'`, or
  `git log -p -- <file>`.
- Published releases, their downloads and the in-app updater manifests were
  checked afterwards and are unchanged. Release pages point at tags, and the tags
  were carried across.
- GitHub may go on serving the old commits for a while through cached pull-request
  references. Removing those needs GitHub Support; that request is the
  maintainer's to make.
- A full backup of the OLD history was kept outside the repository (a git bundle
  beside the project folder, dated 2026-09-25). **It still contains the real
  name.** It was kept only as a safety net for the rewrite and is due to be deleted
  from about 2 Oct 2026. Never push from it or restore from it without the
  maintainer deciding to.

**Why write this down:** without it, the next person to follow an old commit ID
concludes the commit was lost or the history is broken, and may go looking for it
in the backup, which is the one copy that must not be used.

Related: [[project-standing-rules]], [[project-session-handoff-pointer]].
