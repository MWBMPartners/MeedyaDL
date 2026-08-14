#!/usr/bin/env bash
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release-notes/draft-notes.sh
# =====================================
#
# Drafts a tier-1 ELI5 release-notes skeleton for a version that is about
# to ship. Part of #1028 -- the Release-Note-trailer ELI5 pipeline.
#
# USAGE:
#   scripts/release-notes/draft-notes.sh <TAG> [PREV_TAG]
#
#   TAG       Version to draft, e.g. v1.11.0 or v1.11.0-alpha.29.
#   PREV_TAG  Optional. Defaults to the previous tag of the same channel:
#             the previous stable tag for a stable vX.Y.Z, or the
#             previous same-base same-channel tag for a prerelease
#             (e.g. v1.11.0-alpha.28 for v1.11.0-alpha.29).
#
# OUTPUT:
#   .github/release-notes/<TAG>.md.draft
#
#   The .draft suffix is deliberate -- release-note-gate.yml's
#   release-pr-notes-file job only looks for "<TAG>.md" (no suffix), so
#   this file is inert until a human (or Claude) polishes it per
#   .github/release-notes/STYLE_GUIDE.md and renames it (drop ".draft")
#   before merging to main.
#
# WHAT IT DOES:
#   1. Works out PREV_TAG if it wasn't supplied.
#   2. Seeds the ELI5 sections from any Release-Note: trailers already
#      merged since PREV_TAG (cliff-eli5-body.tera does the grouping
#      into What's new / What's fixed / Performance / Notes).
#   3. Appends the full technical git-cliff changelog for the same range,
#      wrapped in an HTML comment, as raw source material for polishing.
#
# NOTE ON RANGES: TAG is normally NOT a real git tag yet when this script
# runs -- the whole point is to satisfy release-note-gate.yml's
# release-pr-notes-file check BEFORE the release-please PR merges and the
# tag is cut. So the git-cliff range end falls back to HEAD unless TAG
# already exists (e.g. re-drafting a prerelease after the fact).
#
# See also:
#   scripts/release-notes/apply-notes.sh -- ships the polished file to a
#   GitHub Release once <TAG>.md.draft has been renamed to <TAG>.md and
#   merged to main.

set -euo pipefail

TAG="${1:-}"
PREV_TAG_ARG="${2:-}"

if [ -z "$TAG" ]; then
  echo "Usage: $0 <TAG> [PREV_TAG]" >&2
  echo "  e.g. $0 v1.11.0" >&2
  echo "       $0 v1.11.0-alpha.29 v1.11.0-alpha.28" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

ELI5_TEMPLATE=".github/cliff-eli5-body.tera"
OUT_FILE=".github/release-notes/${TAG}.md.draft"

if [ ! -f "$ELI5_TEMPLATE" ]; then
  echo "ERROR: ${ELI5_TEMPLATE} not found." >&2
  exit 1
fi

if ! command -v git-cliff >/dev/null 2>&1; then
  echo "ERROR: git-cliff is required (e.g. 'brew install git-cliff')." >&2
  exit 1
fi

# --- Work out PREV_TAG --------------------------------------------------
if [ -n "$PREV_TAG_ARG" ]; then
  PREV_TAG="$PREV_TAG_ARG"
else
  if [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    # Stable release -- previous stable tag only (no prerelease suffix).
    PATTERN='^v[0-9]+\.[0-9]+\.[0-9]+$'
  elif [[ "$TAG" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)-([A-Za-z]+)\.[0-9]+$ ]]; then
    # Prerelease -- previous tag with the same base version AND channel.
    BASE="${BASH_REMATCH[1]}"
    CHANNEL="${BASH_REMATCH[2]}"
    ESCAPED_BASE="${BASE//./\\.}"
    PATTERN="^v${ESCAPED_BASE}-${CHANNEL}\\.[0-9]+\$"
  else
    echo "ERROR: could not parse TAG '${TAG}' as vX.Y.Z or vX.Y.Z-channel.N -- pass PREV_TAG explicitly." >&2
    exit 1
  fi

  # git's own version-aware tag sort (newest first) -- portable across
  # macOS/Linux, unlike `sort -V` which BSD sort doesn't support. Exclude
  # TAG itself in case it already exists (the backfill/re-draft case);
  # the first survivor is the most recent tag before TAG.
  PREV_TAG="$(git tag --sort=-version:refname \
    | grep -E "$PATTERN" \
    | grep -v -x "$TAG" \
    | head -1 || true)"

  if [ -z "$PREV_TAG" ]; then
    echo "ERROR: could not find a previous tag matching '${PATTERN}'. Pass PREV_TAG explicitly." >&2
    exit 1
  fi
fi

echo "TAG:      $TAG"
echo "PREV_TAG: $PREV_TAG"

# --- Resolve the range end (see NOTE ON RANGES above) -------------------
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  RANGE_END="$TAG"
else
  RANGE_END="HEAD"
  echo "Note: ${TAG} is not an existing git tag yet -- rendering ${PREV_TAG}..HEAD instead."
fi

RANGE="${PREV_TAG}..${RANGE_END}"
VERSION_NO_V="${TAG#v}"

mkdir -p "$(dirname "$OUT_FILE")"

# --- ELI5 section, seeded from any Release-Note: trailers already merged ---
ELI5_BODY="$(git-cliff --strip all --tag "$TAG" --body "$(cat "$ELI5_TEMPLATE")" "$RANGE" 2>/dev/null || true)"

if ! printf '%s' "$ELI5_BODY" | grep -q '^### '; then
  # No Release-Note: trailers found in range yet (typical when drafting
  # right after the range opens) -- still scaffold all four headings so
  # the human has the right shape to fill in from the technical source
  # material below.
  ELI5_BODY="$(cat <<'EOF'
### What's new

<!-- No Release-Note: trailers found in this range yet -- add bullets, or pull from the technical changelog below. -->

### What's fixed

<!-- ditto -->

### Performance

<!-- ditto -->

### Notes

<!-- ditto -->
EOF
)"
fi

# --- Full technical render, for reference while polishing ---------------
TECH_BODY="$(git-cliff --strip all "$RANGE" 2>/dev/null || true)"

{
  echo "# MeedyaDL ${VERSION_NO_V}"
  echo
  echo "<!-- one-line summary here -->"
  echo
  printf '%s\n' "$ELI5_BODY"
  echo
  echo "<!-- SOURCE (delete before publishing): full technical changelog for ${RANGE}, kept here as reference material while polishing the sections above per .github/release-notes/STYLE_GUIDE.md."
  echo
  printf '%s\n' "$TECH_BODY"
  echo "-->"
} > "$OUT_FILE"

echo
echo "Wrote: ${OUT_FILE}"
echo "Polish per .github/release-notes/STYLE_GUIDE.md, then rename to drop .draft and commit."
