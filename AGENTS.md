# AGENTS.md — how Codex works in this repository

This file is for Codex (OpenAI's coding assistant). It exists because **Codex reads
`AGENTS.md` files** (or an `AGENTS.override.md`, or other names only if set up to) — the machine-wide `~/.codex/AGENTS.md`, then this one at the repository
root — and nothing else unprompted. This repository keeps its shared project context under
`.OpenAI/`, which Codex would otherwise never see. Until this file was added (2026-09-21,
issue #1198), Codex worked here without the project rules. Claude Code has the same content
through `.claude/`; both tools are meant to work by the same rules.

## Read these before doing any work

1. **`.OpenAI/CONTEXT.md`** — the project context. It is a byte-for-byte copy of
   `.claude/CLAUDE.md`, refreshed by `scripts/sync-claude-memory.sh`; never edit it by hand.
   It is large (about 230 KB), so read the **"Standing rules and standing tasks (read first)"**
   section at the top and the **"Conventions"** section first, and search the rest as
   reference when you need a specific subsystem.
2. **`.OpenAI/memory/`** — one file per project fact, indexed by `.OpenAI/memory/MEMORY.md`.
   It mirrors `.claude/memory/` (kept identical by hand; the index must stay byte-identical).
3. **`.OpenAI/memory/project_standing_rules.md`** — the full standing rules, with the reasons
   and the exact commands. The list below is the short form.
4. **`.github/HANDOFF.md`** — the one session handoff. Read its `LATEST` section to see
   what is in progress, what was tried and rejected, and what to do next.

## The standing rules, in brief

- **Plain English, always.** No technical jargon in anything written or said — even to a
  technical reader. Say what a technical name means when one is unavoidable.
- **Never write the maintainer's real name.** Use the GitHub username `Salem874`, everywhere.
- **One handoff: `.github/HANDOFF.md`.** Update it as the work happens, not at the end. Do
  not create another handoff anywhere else.
- **Strongest model for thinking, cheapest capable model for building; checking is never done
  by a weaker model than the building** (in Claude Code that means never below Opus). Plan
  first, one step at a time.
- **Use helper plugins where they fit, including to suggest further fixes, tweaks,
  enhancements and new features** — raised as suggestions, not built unless the maintainer
  says so.
- **Every change is reviewed by the other system until a round finds no real problems.** When
  Codex builds something, Claude Code reviews it with a fresh agent; when Claude Code builds it,
  Codex reviews it (`codex review --uncommitted` before a commit, `codex review --base alpha`
  for the whole branch; neither accepts extra instructions, so a focused review uses
  `codex exec -s read-only "<what to check>"`). Fix, re-review, repeat. A finding you are sure
  is wrong is recorded with the reason, never "fixed" just to quiet the reviewer. Record the
  round count.
- **After each finished piece of work, in this order:** verify it yourself (read the
  checks' real exit codes; format only touched files; one security read of the diff);
  commit and push to the working branch, saying in the commit message whether it has been
  independently reviewed yet; the review loop over that commit's range, fixing and
  re-committing until a round comes back clean; update `.claude/memory/` and its
  `.OpenAI/memory/` mirror, `.claude/CLAUDE.md` (then run `./scripts/sync-claude-memory.sh`
  to refresh `CONTEXT.md`), and `.github/HANDOFF.md`; update that task's GitHub issue; show
  the progress table. The commit comes before the review (changed 23 Sept 2026) because the
  cross-checker reads a range of commits; nothing is merged until the review is clean.
- **One working branch, one pull request to `alpha` opened later when the maintainer says
  so, never stacked.** Never force-push, hard-reset, or change a remote without an explicit
  instruction, and never push directly to `main`, `alpha`, `beta` or `release-candidate`
  without one — work reaches them only by merging a PR. When several PRs are combined into one, and only once the combining PR is
  open: close each original (naming its replacement) and delete that original's branch, after
  confirming its changes are on the combining branch. Only those branches — never a release
  branch or the combining branch.
- **Work autonomously; ask every question up front** in one numbered block, then continue
  with everything not blocked. A decision the maintainer has taken is final.
- **Reorder and bundle tasks** where that is more efficient, as long as nothing is dropped.
- **Progress tables:** give frequent updates as a table of the queued tasks and the status of
  each.
- **The documentation sweep is a standing task** after each real body of work, before its
  PR. OpenAPI/Swagger does not apply here: MeedyaDL is a desktop app with no web API.
- **If a service or agent runs out — credit, rate limit, outage — hand the work to another
  suitable one** only if the context survives the move; switch back at the next natural
  break, and try the usual one first at the start of every new run; when it is back, run a
  full review of everything done while it was away; a review never changes hands silently.
- **Every started job gets a watchdog.** When you start anything that finishes later (a
  review, a CI run, a background build or test), arrange at that moment to come back when it
  ends — and if nothing will tell you, stay with it and check until it finishes. Give it a
  deadline, read the real result before the next step, and note in the handoff what is still
  running. A result nobody comes back for is lost work.

## Conventions that catch people out

- Every source file starts with the copyright header; every function gets a real comment
  explaining the why. A commit title starts with its type (`feat:`, `fix:`, `docs:` …), which
  the release tooling reads; `feat`/`fix`/`perf` commits end with a `Release-Note:` line — one
  plain-English sentence for the release notes (or `Release-Note: none`).
- `help/*.md` is the only copy of the in-app help — no emoji shortcodes, exact relative
  links, and a manifest line in `src/components/help/helpTopics.ts` for every page.
- Never pipe a check into `grep` or `tail` and then rely on `&&`; the pipe hides the exit code.
- `rustfmt` only on files you touched; the tree has pre-existing drift and CI does not gate on it.
- Read MeedyaSuite-core from GitHub, never from the local cargo checkout.
