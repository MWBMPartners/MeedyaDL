#!/usr/bin/env bash
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release-notes/apply-notes.sh
# =====================================
#
# Ships the tier-1 ELI5 release-notes file for a tag onto the matching
# GitHub Release, PRESERVING the existing "Choose your download" footer
# that release.yml appends once the per-platform builds finish.
#
# Part of #1028 -- this is the "also overwrites pre-created releases"
# half of the pipeline: release-please and version-bump.yml both create
# the release object ahead of (or without) a hand-written body, and this
# script is the corrective pass that replaces everything ABOVE the
# download-table footer with the polished tier-1 file, whenever it's run
# -- before the platform builds finish, after, or as a later fix-up.
#
# USAGE:
#   scripts/release-notes/apply-notes.sh <TAG> [REPO]
#
#   TAG   The release tag, e.g. v1.11.0. Must have a matching
#         .github/release-notes/<TAG>.md file (see README.md in that
#         directory and scripts/release-notes/draft-notes.sh).
#   REPO  Optional. Defaults to MWBMPartners/MeedyaDL.
#
# IDEMPOTENT: if the release body already matches "notes file + existing
# footer", this exits 0 without calling `gh release edit` again.

set -euo pipefail

TAG="${1:-}"
REPO="${2:-MWBMPartners/MeedyaDL}"

if [ -z "$TAG" ]; then
  echo "Usage: $0 <TAG> [REPO]" >&2
  echo "  e.g. $0 v1.11.0" >&2
  echo "       $0 v1.11.0 MWBMPartners/MeedyaDL" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NOTES_FILE="$REPO_ROOT/.github/release-notes/${TAG}.md"

if [ ! -f "$NOTES_FILE" ]; then
  echo "ERROR: ${NOTES_FILE} not found." >&2
  echo "Draft one with: scripts/release-notes/draft-notes.sh ${TAG}" >&2
  exit 1
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "ERROR: the GitHub CLI ('gh') is required." >&2
  exit 1
fi

# Use temp files (not env vars / command substitution) to pass the live
# release body and the notes file into python -- release bodies can be
# large once a few platform build tables are appended, and shell argv /
# environment both have size limits that a growing changelog would
# eventually hit.
BODY_FILE="$(mktemp)"
NEW_FILE="$(mktemp)"
trap 'rm -f "$BODY_FILE" "$NEW_FILE"' EXIT

echo "Fetching current release body for ${TAG} (repo: ${REPO})..."
gh release view "$TAG" --repo "$REPO" --json body -q .body > "$BODY_FILE"

# Footer-preserving splice: replace everything above the "## Choose your
# download" table with $NOTES_FILE, keeping the table (and the "---"
# divider above it) verbatim. Shared with release.yml's ensure-release
# self-heal path (#1046) -- see splice-body.py for the exact regex.
python3 "$REPO_ROOT/scripts/release-notes/splice-body.py" "$NOTES_FILE" "$BODY_FILE" "$NEW_FILE"

if diff -q "$BODY_FILE" "$NEW_FILE" >/dev/null 2>&1; then
  echo "Release ${TAG} is already current -- nothing to do."
  exit 0
fi

echo "Updating release ${TAG} body from ${NOTES_FILE}..."
gh release edit "$TAG" --repo "$REPO" --notes-file "$NEW_FILE"
echo "Done."
