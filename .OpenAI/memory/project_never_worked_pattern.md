---
name: project-never-worked-pattern
description: Fourteen shipped MeedyaDL features turned out never to have worked once — the three recognisable shapes that let this happen, and the lesson for every future feature
metadata:
  type: project
---

# Features that shipped, passed review, and never once worked

A 2026-09 sweep of this codebase found **fourteen finished, reviewed, shipped
MeedyaDL features that turned out never to have worked** — not broken by a
later change, never functional, several of them for the entire life of the
project. Every single one looked completely healthy from the inside: the
build passed, the checks were green, nothing ever logged an error. That is
the whole reason this note exists — a healthy-looking build tells you
nothing about whether the thing it built actually does what it claims.

They fail in three recognisable shapes.

## Shape 1: switched off, with no switch to turn it on

The setting exists in the code. It defaults to off. No screen in the app
ever lets a user turn it on. Four cases:

- The analytics toggle (#1155).
- The song.link (Odesli) lookup (#295).
- The cover-art upgrader (#1159) — a fully written, fully unit-tested picker
  with no caller anywhere and no settings switch, so no matter how a user
  configured the app, nothing could ever call it.
- Crash reporting — see Shape 2 below; it is also this shape in an earlier
  form.

## Shape 2: a value that is never supplied

Several features are switched on by a value handed to the app at build
time, not by a setting a user can see — `option_env!("NAME")` in Rust,
`import.meta.env.VITE_NAME` in the frontend. Both treat a missing value as
"not configured" and then say nothing at all. Three cases, found together on
2026-09-08 (#1161, #1162, #1163):

- **Crash reporting** (`SENTRY_DSN` / `VITE_SENTRY_DSN`). This had already
  been "fixed" once, in #231 back in March, by making the code read an
  environment variable instead of a hard-coded value. Nobody ever set the
  variable. The original complaint — users who opt in believe they are
  contributing crash data but nothing is sent — stayed true for five months
  while the issue sat closed as fixed.
- **The remote pause switch** (`INTAPPS_BASE_URL` / `_APP_ID` / `_API_KEY`)
  — the mechanism for stopping a service across every installed copy of the
  app without shipping an update. Inert in every build. The emergency lever
  would have been discovered disconnected at the exact moment it was needed.
- **Developer access** (`DEV_ACCESS_HASH`) — this one was worse than inert.
  The check fell back to accepting the hash of an empty string, so the
  hidden gate opened by pressing the button without typing anything at all.
  That unlocked Spotify downloading, the daily download-cap reset, and the
  unstable update channels — to anyone who found the button, with no
  passphrase required.

See [[project-build-time-secrets]] for the full mechanism and the fix.

## Shape 3: a guard reading from a note instead of from reality

A comment states something as settled fact. A check gets narrowed because
of what the comment says. From then on the check spends its whole life
agreeing with the comment instead of ever being allowed to contradict it.

This is how the Linux in-app updater bug (#1166) survived the entire life
of the project. A comment said Linux ARM64 and ARMv7 builds "never produce
an updater signature on any release" and were "deliberately excluded" from
the one automated check that would have caught a missing update-manifest
entry. That was false — ARM64 had been signing its own output since the day
its build started doing so — and the comment is *why* nobody noticed: it
told the one thing that could have caught this to stop looking at ARM
entirely. See [[project-comment-accuracy-hazard]] for the sibling case
(the Odesli API-key leak) that this same shape produced.

## Others in the same family, so the shape stays recognisable

- No pre-flight warning had ever dismissed itself, because one side of the
  match wrote the string `"internet"` and the other side read `"Internet"`
  — a capital letter apart, forever.
- Arranging a retry did not retry: every call to `process_queue` sat inside
  an `if !should_retry` guard, so the retry path built the retry and then
  never actually ran it.
- "Open Folder", "Open File", and all three "Reveal" buttons had never
  opened anything, in any shipped release.
- The language setting never worked on its own default value.
- The security forward-port workflow had never once run, because it
  compared the GitHub API's `app/dependabot` login against the literal
  string `dependabot[bot]` — two different spellings of the same bot,
  neither of which the code recognised as the other.

## The lesson to carry forward

None of these fourteen could be seen from inside the app. Every single one
needed something *outside* the app compared against something *inside* it
— a release workflow's env blocks against the code's `option_env!` reads, a
release's actual assets against the manifest a check assumed was complete,
one system's exact string against another system's exact string. The app
itself had no way to notice any of them, because from the app's own point
of view, "nobody set this" and "nobody needs this" look identical.

So when adding a feature that depends on a value, a setting, or another
system: ask **what would compare the two** — and if the honest answer is
"nothing would", that absence is the check that needs writing, before the
feature ships, not after someone eventually goes looking for why it never
did anything.

Related: [[project-build-time-secrets]], [[project-comment-accuracy-hazard]],
[[project-codebase-sweeps-2026-09]], [[project-audit-checks-inventory]]
