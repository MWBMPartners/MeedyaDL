---
name: project-queue-attempts
description: How the download queue tells one attempt of a download from the next (#1432, Nov 2026), and the maintainer's decisions on stuck downloads
metadata:
  type: project
---

**Every attempt owns its own slot and state (#1432, built and reviewed clean over 6 rounds, 17 Nov 2026).**
Each queue item records which attempt may change it (`QueueItem.attempt`, from one queue-wide
counter that is never reused or saved). A new number is given on start, every re-queue, Retry,
Cancel and Abort. Every state change goes through `find_owned`, so an old run can never finish,
fail or free the slot of a retried item. Slots are a list of attempt numbers; `release_slot` frees
only its own and is safe twice. Full detail: CLAUDE.md "Serial queue processing".

**Maintainer's decisions on stuck downloads (17 Nov 2026) — final, do not reopen without asking:**

- The stall check marks the attempt failed and frees its slot, but does NOT kill the run.
- If that run later really finishes: one activity-log line, a History entry, and ONE automatic
  retry (Apple Music). Spotify is marked complete instead, never retried. Never twice per run a
  person started.
- "Queue finished" and the after-queue action wait for a stuck run at most one hour.
- Abort, Cancel, Delete, Clear and Retry stop a stuck run.
- Removing the last waiting item of a paused queue finishes the queue straight away.
- A failed item's extra-format copies keep the slot until they end (fixed inside #1432).

**Lesson from the reviews:** the "has the queue finished?" step must never start anything.
An early version reused the queue's start path from the stop commands, which ran the pre-download
checks and started held items. It is now `finish_queue_if_drained` (pure decision `decide_drain`).
A source-reading test guards that; such tests must accept Windows line endings (use
`end_of_function` in `download_queue/tests.rs`) — one that did not failed on Windows only.

**Follow-ups:** #1451 (the same stuck/retry rule for every service — maintainer wants it
consistent), #1452 (saved-files check after a retry can count an earlier attempt's files).
