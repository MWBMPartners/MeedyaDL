---
name: project-a-read-can-break-a-writes-promise
description: A narrow write that promises to change one field can still change others, if the READ it uses has side effects — the load_settings case (since renamed: load_settings is now the plain read, load_settings_at_startup the one with side effects), and how to avoid repeating it
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
- **The names were swapped in September 2026.** A warning in the
  documentation was not enough: forty-nine places called the side-effecting
  `load_settings`, and not one of them was startup. So `load_settings` now
  simply calls `read_settings_from_disk`, and the startup version is
  `load_settings_at_startup`, called only from `lib.rs`. A name that promises
  something harmless must belong to the harmless function. (Some code
  comments still describe `load_settings` as the one with side effects —
  treat those as out of date.)
- It also went wrong through the "is this a finished release?" test, which
  checked only `starts_with("0.")` and so called every 1.x alpha, beta and
  release candidate finished — making the verbose-logging switch-off fire on
  every read. That test is now one shared rule,
  `utils::version::is_unfinished_build` (a `0.` start OR a `-` suffix).

## The same lesson, again, with the one-off shutdown

The one-off "shut down after the queue" (`after_queue_once`) showed the
same shape from another side, in September 2026: it lives in the file AND in
the running app's settings cache, and each writer that read the file and wrote
it back could put a used-up shutdown back. It took several review rounds to
close, and the fix that held was ONE rule rather than one fix per writer:
every general write keeps the running app's value for that field
(`config_service::one_off_in_memory`), only `set_after_queue_once` may change
it, the queue's clear changes file and cache inside the same lock
(`update_settings_field_and_memory`), and the cache's first fill happens under
that lock too and never replaces a newer value. `SettingsCache::peek` reads
the cache without loading anything, for exactly this reason.

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
