---
name: project-standing-rules
description: The maintainer's standing rules and standing tasks for MeedyaDL, restated 2026-09-21 (#1198) — plain English, one handoff, how to think and build, plugins and the Codex review loop, the after-each-task checklist, the documentation sweep, autonomy, progress tables, one PR, hand-over, a watchdog on every started job
metadata:
  type: project
---

# Standing rules and standing tasks for MeedyaDL

**Set by the maintainer on 2026-09-21 (#1198).** This is the complete, MeedyaDL-specific
version of every standing rule and standing task. The short form is at the top of
`.claude/CLAUDE.md` (and so of `.OpenAI/CONTEXT.md`); this file carries the reasons and the
exact commands. A general, project-neutral version of the same rules lives in the machine-wide
files (`~/.claude/CLAUDE.md` and `~/.codex/AGENTS.md`, kept identical).

Where a rule already had its own file, this one links to it rather than repeating it.

---

## 1. Plain English, always

Explain everything — chat replies, commit messages, PR and issue text, code comments, help
pages, release notes, this file — in plain, everyday English. No technical jargon, even to a
technical reader.

**Why:** the maintainer says jargon confuses even technically proficient people. A sentence
nobody has to re-read is worth more than one that sounds expert.

Full rule: [[feedback-plain-english-always]].

---

## 2. Keep the handoff current — it is `.github/HANDOFF.md`

**The one handoff for this project is `.github/HANDOFF.md`.** The maintainer's list says
"in `.claude/`", but the handoff is deliberately not there: two copies under `.claude/` and
`.OpenAI/` drifted apart and were deleted on 2026-09-01 (decision confirmed again on
2026-09-21). `.claude/memory/project_session_handoff_pointer.md` is only a signpost.

**Update it as the work happens, not at the end.** After each piece of work; the moment
something is learned that would change how somebody continues; before anything long-running.
A hand-over is never scheduled — whatever is written at that moment is all the next assistant
gets.

**How to update it** (from [[project-session-handoff-pointer]]):

1. Add a new section at the top whose heading contains `LATEST`, the date and the topic —
   the file currently uses `## ★★★★ LATEST — <date>: <topic>`.
2. Change the previous section's `LATEST` to `Previous`, so exactly one heading says `LATEST`.
3. Refresh the `**Last updated:**` and `**Working branch:**` lines in the header.
4. Re-read the channel version numbers from each branch's own `package.json` rather than
   trusting the line already there — it goes stale the moment a push to `alpha` cuts a version.

What it must carry: what is being attempted and why, what is established, which files
matter, **what was tried and rejected**, what is verified versus assumed, and what to do next.

---

## 3. How to think, plan and build

**Think hardest at the analysis and planning stage.** The maintainer's word for this is
"ultrathink". Typed in a live message it raises how much thinking is done before answering.
Whether it has that effect from a saved file is not verified either way — so treat this line
as the instruction, and type the word in the message when it matters.

**Use workflows.** The Workflow feature (Claude Code's way of running several agents to plan
and do a piece of work) runs only when the maintainer has opted in. The maintainer's own words
"use workflows to help plan and do the work" are that opt-in, so use workflows where they fit.

**Model tiering** (owner-mandated; first written down 2026-07-18, restated 2026-09-21):

| Stage | Model | How |
| --- | --- | --- |
| Deep analysis, deep planning, orchestration | **Opus** | **One agent at a time, in sequence — never several in parallel**, because each planning step should see what the previous one established. |
| Implementation | **Sonnet or Haiku**, whichever fits — Haiku for mechanical edits (renames, formatting, boilerplate), Sonnet for ordinary building | If the implementation is genuinely complex, use **Opus**. |
| Verification / review | never below **Opus** | Matches the dev-team plugin's own rule, and the cross-system loop in section 5. |

**Why:** the philosophy is to spend tokens where judgement is needed and save them where it is
not, while still producing correct code the first time (GIRFT — Get It Right First Time).

**Planning moved from Fable to Opus on 2026-09-23.** The maintainer's reason: the newest Opus
is cheaper than Fable and at least as good at this work, so there is no longer anything to
fall back from. The old rule — "Fable, falling back to Opus, and retry Fable next run" — is
gone, not forgotten. An older note naming Fable as the planner is simply out of date.

**Read the tier, not the model name.** The instruction is "the strongest reasoning available,
one agent at a time". Which model fills that has now changed twice and will change again.

Worth keeping for the shape of it: on 2026-09-11 Fable returned a spend limit on all three
agents of a planning run, the work moved to Opus, the switch was stated plainly, and Fable was
retried next run. That is still exactly how a hand-over should go — see
[[project-hand-over-when-an-assistant-runs-out]] — even though the model at the top of the
table has changed.

---

## 4. Use the dev-team plugin where it fits

`dev-team@dev-team` 1.4.0 is installed. Its repo-level settings live in `.dev-team/config.yml`
(committed). The mapping of the maintainer's model rule onto it, and the caveats, are here
because the plugin does not match the rule exactly.

**Which skill for which task**

| Task | Skill | Notes |
| --- | --- | --- |
| Build a feature or component from a brief | `dev-team-orchestrator` | Give it a finished brief; the planning (Opus, one agent at a time) happens before, not inside (see below). |
| Improvement or cleanup pass on existing code | `dev-team-iterate` | |
| Independent verification / QA pass | `dev-team-review` | **Read-only reporting mode only.** It treats the root `SECURITY.md` as its own list of security findings, and its repair mode writes back into it — here that file is the public security policy. It also expects a `PROJECT.md`, which this repo does not have. For reviewing Codex-built work, a plain fresh review agent (section 5) is usually the better choice. |
| Documentation generation and upkeep | `dev-team-docs` | Fits the standing documentation sweep (section 7). The help-page rules and "never hand-edit `.OpenAI/CONTEXT.md`" still apply. |
| Failing CI on an open PR | `dev-team-ci-medic` (also run by `/dev-team-watch-prs`) | **Only with `autofix=off` here** — there is no setting to force it, so pass it every time. Its default (`autofix=safe`) commits and pushes fixes to the PR branch by itself, checked only by its own Opus agent — which skips the Codex review. With `autofix=off` it diagnoses and proposes, and changes nothing. |
| Large upgrades (a Tauri major, a framework port) | `dev-team-migrate` | |
| Competitor feature-gap analysis | `dev-team-featurefind` | Useful for #911-type UI work. |
| CI / secret scanning / branch protection / release audit | `dev-team-ship` | Most of this already exists here; use it to audit, not to stand up. |
| Long autonomous loop to "production-ready" | `dev-team-autopilot` | Only with the maintainer's say-so: it creates its own `autopilot/<date>` branch and run state. |
| Security audit | `dev-team-security` | **Do not run on this repo:** it writes its list of findings into `SECURITY.md` at the repo root, which here is the public vulnerability policy with a section a workflow rewrites automatically. |
| Payments | `dev-team-stripe` | Not applicable. |
| See the effective settings / a run report | `/dev-team-config show`, `/dev-team-report` | |

**Suggestions, not just building.** The maintainer also wants the plugin used to suggest further
fixes, tweaks, enhancements and new features — `dev-team-iterate` for improvements to what
exists, `dev-team-featurefind` for what is missing. Anything outside the task in hand is
**raised as a suggestion** (an issue, or a line in the report), not built, unless the
maintainer says so.

**The model rule mapped onto the plugin — `models: economy`**

Under `economy` the plugin builds on Sonnet (`sonnet-builder`), does mechanical edits on Haiku
(`quick-edits`), writes docs on Sonnet (`sonnet-scribe`) and verifies on Opus (`opus-builder`,
never lower). That is the maintainer's implementation rule exactly.

(`models` is the plugin's cost setting: `economy`, `balanced` or `max`.)

**How it lines up:** under `economy` the plugin's "hard reasoning" step goes to **Opus**
(its own `model-routing.md`, line 72). Since planning moved to Opus on 2026-09-23, that now
matches the rule on both halves. (Before, the rule said Fable for planning, and no setting
matched both halves.) Under `balanced` or `max` it uses Fable for reasoning but builds
features on **Opus** (mechanical edits stay on Haiku and docs on Sonnet) — more expensive than
the rule asks for. So:

- Still do the deep analysis and planning **yourself, with sequential Opus agents, before
  invoking a skill**, and hand the skill a finished brief — the rule wants each planning step
  to see the one before, which a skill's own internal reasoning does not promise.
- For a one-off run where the reasoning step matters more than cost, add `models=balanced` to
  that one request.

Escalation inside the plugin is one tier at a time on a real capability failure:
Haiku → Sonnet → Opus → Fable. Two strikes: the first may be a bad brief (re-brief, same tier).

**Branches, commits and pushes:** `auto-commit` is **off** in the config, which stops the
plugin committing after every single task. It does **not** stop all plugin commits: it still
commits at each checkpoint (and once per cycle in `dev-team-iterate`), whatever the setting
says. **The plugin can push in two cases:** `dev-team-ci-medic` in its default mode (also
reached through the `/dev-team-watch-prs` command), which is why it is only used with
`autofix=off` here; and any skill run on a branch that already has an open PR, because the
plugin's single-PR rule pushes its commits to that PR's branch. So do not run dev-team skills
on a branch whose PR is open. Nothing in the settings file can enforce either restriction, so
they rely on being remembered. Outside those two cases nothing leaves this machine until you
push — and the rule
that matters is **review before it is pushed**: run the Codex review over everything the
plugin committed (`codex review --base <the commit you started from>`) before pushing. By default a run
"branches from and targets the default branch" — start the skill from the working branch and
tell it to stay there; never let it open a PR. Its safety guard is weaker than it sounds: it
blocks only pushes and force-pushes to `main`/`master` (and live payment keys), and only
while an autopilot run is in progress — ordinary skill runs are not guarded at all.

**The plugin's own files are scratch here.** It writes `HANDOFF.md`, `PROJECT.md`,
`FEATURES.md`, `VERIFICATION.md`, `verdict.json`, `MIGRATION.md` and `.dev-team/autopilot.json`.
All of them are git-ignored in this repo, so git refuses to add them unless forced
(`git add -f`) — and the plugin's own instructions tell it to commit `HANDOFF.md`, so it may
force it. `auto-handoff` is off too, but the plugin still writes `HANDOFF.md` at checkpoints.
**Before every push, check the plugin's commits** — every outgoing commit, with
`git log --stat <start>..HEAD`, not just the difference between start and end (a file added in
one commit and deleted in the next vanishes from that difference but is still pushed) — for any of
these files, and take them out if present. If a plugin step reports it could not add an
ignored file, check that the commit it was part of still happened — git stages the other
files but the step can stop before committing. Anything worth keeping goes into `.github/HANDOFF.md` or an issue. `SECURITY.md`
cannot be ignored — it is the real public policy — which is why the two skills above that
write to it are off-limits.

**Codex through the plugin:** the plugin's Codex offload needs `openai/codex-plugin-cc`, which
is enabled in `.claude/settings.json` but **not installed** on this Mac. So the cross-system
check runs through the command-line tool (section 5), not through `/codex:*` commands.

---

## 5. Every change is reviewed by the other system, until a round comes back clean

**Why:** two different systems rarely make the same mistake in the same place. Each of the
first three rounds on #1176 found faults in the previous round's fixes; the fourth found
nothing, and that is what finished work looks like — see
[[project-review-rounds-find-their-own-fixes]]. This expands the GIRFT rule in the machine-wide
file.

**When Claude Code built it, Codex reviews it.** The Codex command-line tool (`codex`) is installed. It reads the
repo-root `AGENTS.md` (which points it at the project rules) and nothing under `.OpenAI/` on
its own. From the repo root:

```sh
# Everything not yet committed (staged, unstaged, untracked) — before each commit
codex review --uncommitted

# Everything the working branch adds over its base (this branch was cut from alpha)
codex review --base alpha

# One commit
codex review --commit <commit-id>

# A focused review with your own instructions. `codex review` REFUSES a
# custom prompt alongside --uncommitted or --base ("cannot be used with
# [PROMPT]", checked 2026-09-21), so use `codex exec` in read-only mode and
# tell it what to look at. -o saves its final answer to a file.
codex exec -s read-only -m gpt-6-sol -c model_reasoning_effort=medium -o /tmp/codex-review.txt "Review the uncommitted changes in this repo (git diff HEAD, which includes staged changes, plus untracked files from git status). Check correctness, security, and that every comment and message is plain English. List each finding with file and line, or say there are none."
```

**Model and effort (maintainer's decision, 22 September 2026):** keep the model
`gpt-6-sol`, but run reviews at **medium** reasoning effort, passed on the command line
as above. The model is the right one for reviewing; the top effort tier is not needed for
it, and Codex's allowance has run out repeatedly — four times in one day, and once for
five days. **Set it per run, never by editing the Codex config file on this Mac**: that
config is the maintainer's own and covers work outside this project.

**The loop:**

1. Run the review. Read every finding.
2. Fix the real ones. For a finding that is wrong, write down why (handoff or commit body) —
   do not just ignore it.
3. Run the review again.
4. Stop when a round finds **no real problems**. A finding you are sure is wrong does not keep
   the loop going, and must never be "fixed" just to quiet the reviewer: record why it is
   wrong (commit body or handoff) and move on. Record the result: "Codex review: N rounds,
   last round clean" (or "… last round: 1 finding disputed, reason recorded").

**When Codex built it (for example during a fallback), Claude Code reviews it** — with a
**fresh** agent that has no memory of building it — a plain review agent on Opus or Fable
(or `dev-team-review` in read-only mode; see section 4). Same loop, same stopping point.

**When Codex is unavailable** (out of credit, rate limited, down): a review must never change
hands silently. Say so in the report **and** the commit message; get what independence is
available (a different model, or a fresh agent) and name which; mark the change as not yet
fully reviewed; include it in the full catch-up review when Codex is back. Full rule in
[[project-hand-over-when-an-assistant-runs-out]].

---

## 6. After each finished piece of work — the checklist

One "piece of work" = one unit with its own GitHub issue and its own commit. Do these in
order; do not start the next unit until they are done.

**The order changed on 2026-09-23.** Committing now comes BEFORE the review, and updating the
notes comes after it. The reason is practical: the cross-checker reads a **range of commits**,
so work has to be committed before it can be reviewed at all — the old order was being worked
around every single time. Two things keep that safe. **The commit message must say plainly
whether it has been independently reviewed yet**, never letting silence imply it has. And
nothing is merged on an unreviewed commit: the loop still runs until a round comes back clean.
Putting the notes last also means they describe what the work finally settled as, rather than
what it looked like halfway through.

1. **Verify it yourself.** Run the tests (`cargo test` in `src-tauri/` — what CI runs; `--lib` alone skips
   the examples written inside code comments — plus `npm run type-check` and `npm run test`) and **read their exit codes directly** — never pipe a check
   into `grep` or `tail` and then rely on `&&` (that once let a commit go in on a failing test, 2026-09-10).
   Run `rustfmt` **only on the files you touched** — never whole-crate `cargo fmt`, the tree has
   pre-existing drift and CI does not gate on it. Read the diff once for security (secrets,
   shell interpolation, paths, credentials in logs).
2. **Commit and push to the working branch.** The commit title starts with its type
   (`feat:`, `fix:`, `docs:` …, the "conventional commit" format the release tooling reads);
   every `feat`/`fix`/`perf` commit ends with a `Release-Note:` line — one plain-English sentence
   for the release notes (or `Release-Note: none`); the `Co-Authored-By`
   line the session reminder gives; the GitHub username `Salem874`, never a real name. Then
   `git push`. **Never force-push, hard-reset, or change a remote without an explicit
   instruction.** A small follow-up `docs(handoff):` commit to record the pushed commit ID is fine.
3. **Cross-system review until clean** (section 5) — of the code **and** the note changes
   from step 2, so nothing is committed unreviewed. One exception: a handoff-only update that just records progress does not wait for its own review round; the next round covers it.
4. **Update the notes so the next session can pick up:**
   - `.claude/memory/` — add or update the memory file(s), and its one-line entry in the
     index, `.claude/memory/MEMORY.md`.
   - `.claude/CLAUDE.md` — the affected bullet(s), if behaviour, settings or architecture changed.
   - `.OpenAI/memory/` — apply the same memory changes **by hand** (nothing automates this
     mirror), except `feedback_plain_english_always.md`, which stays in `.claude/` only.
     `.OpenAI/memory/MEMORY.md` must stay byte-identical to `.claude/memory/MEMORY.md` —
     check with `cmp`.
   - `./scripts/sync-claude-memory.sh` — re-copies `.claude/CLAUDE.md` over
     `.OpenAI/CONTEXT.md` (never hand-edit `CONTEXT.md`) and copies memory to the home folder.
     Check with `cmp .claude/CLAUDE.md .OpenAI/CONTEXT.md`.
   - `.github/HANDOFF.md` — the LATEST section (section 2).
5. **The GitHub issue, individually for each task:** create it if it does not exist
   (`gh issue create`); comment with what landed and the commit ID; link parent/child issues;
   add it to the project — `gh project item-add 6 --owner MWBMPartners --url <issue-url>` — and
   if that fails, **say so** in the report rather than skipping quietly; close with a completion
   comment **once the work has merged** (`gh issue close <n> --reason completed --comment "…"`);
   open follow-up issues for anything found and not done.
6. **Show the progress table** (section 10).

---

## 7. Standing task: the documentation sweep

**After each real body of work, before its pull request is opened**, do a thorough update of
every document. Not being run now (2026-09-21) — the documents were fully checked on
2026-09-11 — but it is a standing task from here on.

What it covers for MeedyaDL:

- Every `.md` file: `README.md`, `Project_Plan.md`, `DEV_NOTES.md`, `CONTRIBUTING.md`, `SECURITY.md`, `TERMS.md`, `ACKNOWLEDGEMENTS.md`,
  the audit notes under `.github/audits/`. **Not `CHANGELOG.md`:** git-cliff rebuilds it on
  each release from the commit messages, so a hand edit is lost — fix the commit messages or
  `cliff.toml` instead.
- The in-app help: `help/*.md` is the only copy of the words. Rules that apply there and
  nowhere else are in the "Documentation maintenance" bullet of `.claude/CLAUDE.md`
  (no emoji shortcodes, exact relative links, manifest line in `helpTopics.ts`).
  `tools/audit-checks/check_help_topics.py` catches the mechanical half.
- Claude memory, context and everything else in `.claude/`; the `.OpenAI/` mirror; this file.
- **OpenAPI / Swagger: does not apply to MeedyaDL.** It is a desktop app with no web API;
  its only programmatic surface is in-process Tauri IPC, which OpenAPI cannot describe and
  which has no server for Swagger UI. Decided 2026-08-03 — [[project-api-surface-determination]].
  The native-app API lives in a separate backend repository; document it there.

---

## 8. Order and bundle the work sensibly

The order of tasks in any brief is a suggestion. Reorder and bundle tasks where that is more
efficient — for example, one documentation pass after three related fixes rather than three
passes — as long as nothing is dropped and the progress table shows what was bundled.

---

## 9. Work autonomously; raise every question up front

Do all queued work without pausing, **unless** a decision or approval is genuinely needed
from the maintainer. When it is:

- **Ask at the start, not as you come across it.** Before the work begins, list every decision
  needed in one numbered block: what is being asked, why it matters, the recommended answer,
  and what will be done meanwhile.
- Word each question as simply as possible.
- Then **continue with everything that is not blocked**. A question never stops the queue.
- A decision the maintainer has already taken is final — do not reopen it.

---

## 10. Progress tables

Give frequent status updates as a table of the queued tasks:

| # | Task | Issue | Status | Notes |
| --- | --- | --- | --- | --- |
| 1 | Short plain-English name | #nnn | Done — pushed `abc1234` | Codex review: 2 rounds, last clean |
| 2 | … | #nnn | In progress | |
| 3 | … | #nnn | Blocked — waiting on decision D1 | continuing with 4 meanwhile |
| 4 | … | #nnn | Queued | |

Status words: **Queued · In progress · In review · Blocked (say on what) · Done (say the commit ID)
· Dropped (say why)**. Show the table at least after each finished unit and whenever the
queue changes.

---

## 11. One working branch, one pull request, never stacked

- All work goes to **one working branch** (the one named on the `**Working branch:**` line of
  `.github/HANDOFF.md`, cut from `alpha`). Commit and push each finished piece there (section 6).
- **One pull request to `alpha`, opened later, when the maintainer says so** — not before.
- **Never open a second PR against the same base while one is open, and never chain a PR on
  another PR's branch.** Concurrent PRs race each other: the second inherits a stale base the
  moment the first merges. Combine related work into the one PR instead. The full rule, with
  the rebase-versus-squash choice, is the "No PR stacking" bullet in `.claude/CLAUDE.md`.
- **When several PRs are combined into one, tidy up what is left** (maintainer, 2026-09-21).
  Only once the combining PR is open: close each original PR with a comment naming the PR that
  replaced it, and delete that original PR's branch — after checking its changes really are on
  the combining branch: `git cherry origin/<combining-branch> origin/<original-branch>` must
  **succeed** (exit code 0) **and** show no `+` lines. Check the exit code — when the command
  fails (for example a mistyped branch name) it prints nothing, which looks exactly like "no
  `+` lines". Run `git fetch origin` first and use the `origin/…` names, so you compare the
  branches as they are on GitHub now. And `git cherry` compares changes, not final contents: a
  change that was carried over and then undone later on the combining branch still passes it.
  So also look at the combining branch's files and confirm the change is still there (for a
  dependency PR: the versions it set are the versions the combining branch has).
  Once its changes live on the combining branch, the original's branch is no longer anyone's
  working branch. Delete **only** the branches of the PRs being replaced, and only in this
  repository — never a release branch (`main`, `alpha`, `beta`, `release-candidate`), the
  combining branch, or any other branch. Dependabot usually deletes its own branch when its
  PR closes, so "branch not found" is fine. Closing a Dependabot PR also stops Dependabot
  offering that version again, so if the combining PR is abandoned, reopen the originals
  (comment `@dependabot reopen` on each). A PR itself cannot be deleted on GitHub; closing it
  is the most that can be done.
- **Never force-push, hard-reset, or change a remote without an explicit instruction.**
- **Never push directly to a release branch** (`main`, `alpha`, `beta`, `release-candidate`)
  without an explicit instruction. Work reaches them only by merging a pull request. GitHub's
  branch rules here block deleting and force-pushing those branches, but they do **not** stop
  a plain push — so this rule is the only thing that does. (The automatic version-bump
  workflows push to them by design; that is not what this is about.)

**This replaces** the old rule "do not auto-commit or auto-push; let the user control git"
(decision 2026-09-21). Pushing to the working branch is now standing practice, not a
per-session grant.

---

## 12. Hand over when an assistant runs out; hand back promptly

If the service or agent doing the work becomes unavailable — out of credit, rate limited,
down — hand the work to another suitable one rather than stopping, **provided the context
survives the move** (which is what section 2 is for). Switch back to the usual one at the next
natural break; retry it at the start of every new run. Run a **full review** of everything
done while it was away once it is back. **A review never changes hands silently.** Written
without naming tools on purpose: today it means Claude Code and Codex, and within one tool
one model or agent falling back to another.

Full rule: [[project-hand-over-when-an-assistant-runs-out]]. The same rule is in the
machine-wide files for every project on this device.

---

## 13. Every started job gets a watchdog, so no result is missed

**Set by the maintainer on 2026-09-24.** Whenever work is started that finishes later — a
Codex review round, a CI run, a build or test run left in the background, a sub-agent, a
workflow, a scheduled check — set up something that will **come back when it finishes**, at
the moment it is started. Then act on the result and move to the next step in the queue.
Never start a job and simply carry on hoping to notice it later.

**Why:** a result nobody comes back for is lost work. A review that finished with findings
nobody read looks, from outside, exactly like a review that was never run — and the queue
stalls behind it without anything saying so.

**How, in practice:**

- **Pick a watcher that is told when the job ends, not one that guesses.** In Claude Code: a
  background command or agent reports back by itself when it exits; for something outside
  the session (a GitHub Actions run, a release), use a watcher that loops until the result
  exists, or a scheduled wake-up timed to how long the job really takes. In Codex, or any
  tool with no such notice: stay with the job and check it until it finishes.
- **Give every job a deadline** (for example `timeout 2400` on a Codex round), so a hung job
  turns into a visible failure instead of a silent wait.
- **Check the watcher is watching the right thing.** On 2026-09-24 a Codex round was started
  with a second `&` inside an already-background command; the notice fired the moment the
  outer command returned, long before Codex finished. The fix was a waiter that loops until
  Codex's output file is complete. A watcher that fires early is worse than none, because it
  looks like a result.
- **When it fires, read the actual result** (the exit code, the output file, the run's
  conclusion) before taking the next step — never assume success from the fact that it
  ended.
- **Write down what is running and how it is being watched** in `.github/HANDOFF.md`
  before leaving it. Some watchers (Claude Code's scheduled prompts) last only while the
  session is open; if the session ends, the handoff must say exactly what to run to pick
  the job back up.

---

## 14. Small habits that are also rules (harvested from earlier sessions)

- **One issue, one commit, one security read of the diff, per unit** (2026-07-18).
- **`rustfmt` only on touched files; never whole-crate `cargo fmt`.** If the tree is ever
  normalised it must be its own isolated commit.
- **Never pipe a check into `grep`/`tail` and rely on `&&`** — read the tool's own exit code.
  A push-retry loop must check git's exit code, not the pipe's.
- **Run the test suites yourself before every commit** and read the result.
- **MeedyaSuite-core: always read it from GitHub (`gh api`), never the local cargo checkout**,
  which is pinned to a branch and may lag — `project_meedyasuite_core_online_only.md`.
- **Commit titles start with their type** (`feat:`, `fix:`, `docs:` …); **`feat`/`fix`/`perf`
  commits end with a `Release-Note:` line** — one plain-English sentence (#1046).
- **Never write the maintainer's real name anywhere** — `Salem874`.

Related: [[project-session-handoff-pointer]], [[feedback-plain-english-always]],
[[project-hand-over-when-an-assistant-runs-out]], [[project-api-surface-determination]],
[[project-review-rounds-find-their-own-fixes]].
