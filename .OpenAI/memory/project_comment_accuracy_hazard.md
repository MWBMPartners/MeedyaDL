---
name: project-comment-accuracy-hazard
description: A comment asserting a checkable fact must be treated as a claim to verify, not as information — a sweep found seventeen wrong comments, two of them hiding real bugs for months
metadata:
  type: project
---

# Comments in this codebase are claims, not information

A September 2026 sweep read the actual code behind every comment in this
repository that asserts something checkable — "this file does X", "these
things happen in parallel", "this path never includes Y" — instead of
taking the comment's word for it. It found **seventeen comments that were
wrong**. Two of those had been hiding real bugs for months, because the
comment was the reason nobody went looking.

The standing rule this sweep produced: **treat a comment stating a
checkable fact as a claim to verify against the code, never as information
you can act on unchecked.** A comment does not compile, does not run, and
nothing anywhere complains when it stops being true the moment the code
around it changes.

## The two that cost real money

**Linux ARM never produced an updater signature — a comment said, falsely,
that it never would.** A comment stated as settled fact that Linux ARM64
and ARMv7 builds "never produce an updater signature on any release", and
that fact was used to justify excluding ARM from the one automated check
that would otherwise have caught a missing entry in the update manifest
(#1166). The comment was false for ARM64 from the day its build started
signing its own output. The bug it hid: every Linux `.deb`/`.rpm` user —
x64 included, not just ARM — was silently offered AppImage bytes by the
in-app updater and had no working in-app update path at all, for the
entire time the comment stood unchallenged. See
[[project-never-worked-pattern]] Shape 3 for the general pattern this is
an instance of.

**A song.link API key was being written into the log file.** A comment in
`odesli_service.rs` said the third-party web library's own error text never
includes the request address, so it was safe to include that formatted
error text in a log line without redacting anything first. It does include
the address. The address carries the API key as a query parameter. So the
key was being written into plain-text logs, on every request that failed
in the way that comment assumed could not happen.

## Other shapes this sweep found

- **Five of twelve "in parallel" claims were false.** A comment says two
  things happen at the same time; the code actually awaits them one after
  another. In Rust this matters precisely: a future does nothing until it
  is polled, so building two futures and then awaiting each one in turn is
  sequential, full stop, no matter what a comment three lines above says.
- **Sixteen comments pointed at files that do not exist** — the file named
  in the comment had been renamed, split into its own directory, or moved,
  and the comment kept pointing at wherever it used to be.
- **A doc comment ran straight into the next function with no blank line
  between them**, so the documentation-generation tooling attached it to
  the wrong function entirely — correct words, attached to the wrong piece
  of code.
- **A function called `is_service_implemented` still answered "no" for
  Spotify long after Spotify actually worked.** It had no callers anywhere
  except its own test, so rather than fix a check nothing used, it was
  deleted outright.

## The two checks that now catch most of this

- **`check_comment_paths.py`** resolves every file path named in a
  Rust/TypeScript/JavaScript/Python comment against the real filesystem,
  and reports any that do not exist. This is exact — a path either exists
  or it does not — so it has no false-positive risk to manage.
- **`check_concurrency_claims.py`** flags a comment containing "parallel"
  or "concurrently" that sits inside or right above a block of code
  containing none of the mechanisms that would actually make that true
  (`join!`, `join_all`, `try_join`, `spawn`, `Promise.all`, `allSettled`).
  This one is a **rough heuristic and says so in its own docstring** — it
  keeps a named, documented `EXCEPTIONS` list for claims that are true but
  whose mechanism lives somewhere the local code block cannot show (for
  example, concurrency that happens one call site away, inside a helper
  the comment's block merely invokes).

Both are wired into `pr-security.yml` and are covered in more detail,
alongside the other seven audit scripts, in
[[project-audit-checks-inventory]].

Related: [[project-never-worked-pattern]], [[project-codebase-sweeps-2026-09]],
[[project-audit-checks-inventory]]
