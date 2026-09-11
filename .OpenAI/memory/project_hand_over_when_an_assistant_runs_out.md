---
name: project-hand-over-when-an-assistant-runs-out
description: Standing rule — when the assistant or agent doing the work hits a limit or outage, hand over to another suitable one rather than stopping, provided the context survives; hand back promptly and run a FULL review when the usual one returns
metadata:
  type: project
---

# Hand over when an assistant runs out, and hand back promptly

**Standing rule, set by the maintainer on 2026-09-11.** It applies to this
project and to every project on this machine — the same rule is in the
machine-wide instructions.

Every assistant has its own limits: a usage cap, a spend cap, a rate limit, an
outage. Hitting one is routine, not an emergency.

- **Hand the work to another suitable assistant and carry on.** Do not idle
  waiting for a reset. Do not leave a task half-finished because one service
  said no.
- **Agents count too.** A sub-agent hitting its own cap is the same situation as
  the whole service being down, and gets the same treatment.
- **Any suitable one, not only the usual partner.** The point is that the work
  continues with something capable of doing it well, not that a particular
  pairing survives.
- **Go back as soon as the usual one will take work again.** A fallback is a
  detour, not a new route. Retry the preferred one at the start of each run even
  if it failed last time — limits reset and outages end.
- **Say when the work changes hands, and why.** One sentence. A silent switch
  leaves the next reader puzzling over why the output reads differently.

## The condition that has to be met first

The thread must survive the move: what is being attempted, what has been
established, which files matter, what was already tried and rejected, and how
the result gets checked. If that cannot be carried across, write it into
`.github/HANDOFF.md` first and then hand over.

**A confident answer produced without the context that made the question
answerable is worse than no answer**, because nobody can tell the difference by
looking at it.

## Why this is safer here than it would be elsewhere

Work on this project is reviewed across different assistants. A difference in
how one of them approaches a problem tends to be caught by another rather than
shipping unnoticed. That cross-review is the safety net that makes handing over
reasonable rather than risky.

**It is a net, not a guarantee.** When the usual assistant for a piece of work
is available again, run a **full review** — not merely a review of whatever was
produced while it was away. Differences in method show up in the shape of a
whole change, not only in the lines a stand-in happened to touch. Do this often
rather than letting it accumulate.

## Which makes the handoff a live document

A hand-over is never scheduled. A cap is reached mid-sentence; an outage starts
without warning. **Whatever is written down at that moment is the entire context
the next assistant gets.**

So `.github/HANDOFF.md` has to be true *continuously*, not brought up to date at
the end of a task. The end is exactly when it will not happen, because whatever
stopped the work stopped the writing with it.

Update it as things are established, as approaches are rejected, as decisions
are taken — not once the work is finished. Treat "is the handoff true right
now?" as part of the work rather than as tidying up afterwards.

## The exception that matters most

**A review must never change hands silently.** The whole point of a second
assistant checking the first is that two different systems rarely make the same
mistake in the same place. If the reviewer is unavailable and the builder
reviews its own work, that value is gone — and the output looks exactly the same
from the outside.

When the usual reviewer cannot run: say so in the report **and** the commit
message, get whatever independence is available (a different model, or a fresh
agent with no memory of building the thing) and name which was used, and treat
the change as not yet fully reviewed until the real review happens.

## Not a licence to shop for an answer

Hand over for availability only. If an assistant refuses a task on its merits,
that is a judgement, not an outage.

## Seen in practice

On 2026-09-11 the deep-analysis model returned a monthly spend limit on all
three agents of a planning run. The work moved to the next model down, the
switch was stated plainly rather than made quietly, and the preferred one is to
be retried on the next analysis run rather than written off.

Related: [[project-session-handoff-pointer]].
