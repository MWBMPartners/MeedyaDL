---
name: project-review-rounds-find-their-own-fixes
description: Each of the first three rounds of adversarial review found faults in the previous round's fixes — including one fix worse than the bug it fixed; the fourth found nothing, which is what finished work looks like
metadata:
  type: project
---

# A fix needs reviewing as much as the thing it fixed

On the music video work (#1176) there were four rounds of adversarial review.
**Each of the first three rounds found faults in the previous round's fixes.** The fourth
found nothing at all — which is the point at which you can believe the work is
actually finished, and is why you keep going until a round comes back empty
rather than stopping at a fixed number. That is not a sign
the reviews were bad. It is what reviewing fixes actually turns up, and it is
why one round is not enough.

## The three worth remembering

**A fix that was worse than the bug.** Round one found that collapsing the
sidebar was not remembered between launches. The fix made it save. But saving
in this app writes the **whole settings file at once**, and the Settings screen
only saves when you press the button. So: open Settings, press Reset, change
your mind and do not save, then click the sidebar collapse — which is on every
screen — and a third of a second later the defaults were written over the real
settings. Output folder, cookies, templates, credentials, gone.

It was backed out rather than patched a third time, and written up as #1175
with both failed attempts recorded. Backing out is often the right answer when
a fix for something small keeps producing something large.

**A check that flagged its own commit.** Round one added a check whose entire
job is finding settings the app lets you change that nothing ever reads — and
in the same commit added a setting the check reported. Nobody read the output.
**Run a new check and read what it says before committing it.**

**A sentence that said the opposite of the truth.** Round three found wording
I had written in round two claiming the tool falls back to a lower quality when
the video was never offered "at this resolution or higher". It must be "or
below". Written confidently, in two files, in a comment explaining behaviour to
the next person.

## The habit this argues for

- Review the fix, not just the original fault.
- When a fix for a small problem starts producing bigger ones, back it out and
  write down why, rather than trying again in the same direction.
- Read the output of any check you add, in the commit that adds it.
- Prefer a change of shape that makes a fault impossible over a test that
  notices it. The song.link limiter is the example: putting the race back now
  fails to compile.

## Stop guessing at syntax you do not really read — refuse instead

The build-value audit check (batch-4 review, 24 September 2026) had to decide
whether a line in a workflow really exported a value, or was commented out.
**Three successive attempts to reason about shell comments were each beaten by
Codex**: a comment straight after an operator (`true;# echo ...`), an escaped
quote that threw the quote-tracking off, and a `#` that sat *inside* the text
the pattern matched rather than before it. Each one passed a disabled export as
working — a silent pass, the one failure the check exists to prevent. Then even
the strict replacement accepted two lines the shell rejects as errors.

What finally converged was giving up on understanding the syntax: **count only
one plain, complete form, and report everything else as "cannot confirm".** It
costs nothing, because every real line already has that form, and there is
nothing left to out-guess. The one gap it cannot close (a line switched off by
other lines, such as a heredoc) is written down in the check as a known silent
pass rather than hidden.

The habit: when a check keeps being beaten by edge cases in a language it does
not truly parse, narrow what it *accepts* instead of widening what it
*recognises*. Six rounds of review on one small script is the cost of learning
that late.

Related: [[project-comment-accuracy-hazard]], [[project-never-worked-pattern]]
