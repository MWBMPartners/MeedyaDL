---
name: project-a-read-can-break-a-writes-promise
description: A narrow write that promises to change one field can still change others, if the READ it uses has side effects — the load_settings case, and how to avoid repeating it
metadata:
  type: project
---

# A read with side effects breaks a narrow write's promise

`update_settings_field` exists to make one promise: it changes the single field
you name and leaves every other stored setting exactly as it was. The write half
was careful — one field, a lock, the checksum, the atomic replace.

It still broke the promise, because of the **read**.

It read through `load_settings`, which is not a plain read. On a full release
that function also:

- switches verbose activity logging back off,
- records the version just seen,
- rewrites GAMDL's `config.ini`,
- sets a global logging flag.

Every one of those is right at app startup and wrong everywhere else. The result
was that somebody running with verbose logging switched on had it switched off
**and saved to disk** simply by collapsing the sidebar. Clearing a one-shot
after-queue action did the same thing.

A code review caught it before it shipped. Nothing in the write was wrong.

## What to do instead

- `read_settings_from_disk` is the plain read: open, warn on a checksum
  mismatch, parse, migrate, hand it back. No side effects.
- `load_settings` is that plus the startup actions, and its own documentation
  now says plainly that anything wanting to read or change a stored value must
  not call it.

## The general lesson

When you set out to make an operation narrow, **the read is part of the
operation**. Auditing only the write leaves half the surface unexamined. Ask of
any function you are about to read through: does this only tell me things, or
does it also change things?

It is easy to miss because the side effects usually sit far from the signature,
under a comment explaining why they are correct — and they *are* correct, for
the caller they were originally written for.

Related: [[project-comment-accuracy-hazard]] (a claim that reads as settled fact
is the first thing to check), [[project-never-worked-pattern]].
