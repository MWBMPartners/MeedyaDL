#!/usr/bin/env bash
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release/fetch-release-body.sh
# =====================================
#
# Reads a GitHub Release's notes (its "body") and prints them — and, the
# whole point, tells a failed read apart from an empty one.
#
# USAGE:
#   scripts/release/fetch-release-body.sh <TAG> <REPO>   > body.md
#
# EXIT CODES:
#   0  the read worked. What was printed IS the release's notes, even if
#      that is nothing at all (a brand-new release genuinely has none).
#   1  every attempt failed. NOTHING is printed, and the caller must not
#      write anything back to the release on the strength of this read.
#   2  called wrongly (missing arguments).
#
# WHY THIS EXISTS — AND WHY IT IS A SCRIPT
# ----------------------------------------
# Twice in release.yml a read of a release's notes was written like this:
#
#     gh release view "$TAG" ... 2>/dev/null || echo ""
#
# That turns ANY failure — a timeout, a rate limit, GitHub having a blip —
# into an empty string. And an empty string then walks through every
# check that follows as if it were the real notes:
#
#   * In `finalize-release`, the "has the download table already been
#     added?" check found nothing in it, so the step wrote the table over
#     — not alongside, over — a release's real plain-English notes. A
#     later run then saw the table, decided "already done", and never
#     touched it again. The loss was permanent. That one was fixed first.
#
#   * In `ensure-release`, the "does this prerelease's body look like raw
#     commit messages?" check treats an empty body as "yes, needs
#     healing", so it regenerated the notes and wrote them over whatever
#     was really there. Same mistake, 2,000 lines further up the same
#     file. An independent stand-in reviewer found it, and Codex
#     confirmed it (batch 4, finding A).
#
# The fix for the first one was written inline in its own step. When the
# second turned up, copying that fix would have made two versions of the
# same rule — and "one copy of a rule got fixed and the other did not" is
# precisely how the second bug survived the first fix. So the rule lives
# here, once, and both places call it.
#
# WHAT IT DOES NOT DO
# -------------------
# It cannot tell you WHY a read failed; the reason goes to the log as a
# warning on each attempt. And three attempts is not a promise: if GitHub
# is down for longer than about four seconds, this gives up, which is the
# intended outcome — stopping is always recoverable by re-running the
# job, whereas writing over a release's notes on a guess is not.
#
# The retry count and gap match the original inline helper, which was
# chosen because the project's own notes record GitHub refusing twice
# within two seconds during a past incident: one failed attempt proves
# nothing.

set -uo pipefail

if [ "$#" -ne 2 ] || [ -z "$1" ] || [ -z "$2" ]; then
  echo "usage: $0 <TAG> <REPO>" >&2
  exit 2
fi

tag="$1"
repo="$2"

for attempt in 1 2 3; do
  # The body goes into a variable first, never straight to stdout, so a
  # read that fails half-way cannot leave half a body behind for the
  # caller to mistake for the real thing.
  if body=$(gh release view "$tag" --repo "$repo" --json body --jq '.body' 2>/dev/null); then
    printf '%s' "$body"
    exit 0
  fi
  echo "::warning::Could not read release notes for $tag (attempt $attempt of 3) — retrying" >&2
  if [ "$attempt" -lt 3 ]; then
    sleep 2
  fi
done

exit 1
