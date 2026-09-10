#!/usr/bin/env bash
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/release/updater-manifest.sh
# ====================================
#
# Rebuilds and checks the `latest.json` updater manifest for a release.
# `latest.json` is the small file the app downloads to find out whether a
# newer version exists and, if so, which file to fetch for this exact
# machine. If a machine's entry is missing from it, that machine is never
# offered an update at all — quietly, with nothing failing anywhere.
#
# WHY THIS IS A SCRIPT AND NOT INLINE WORKFLOW STEPS
# --------------------------------------------------
# Two workflows need this exact logic: release.yml (every release) and
# fix-updater-manifest.yml (repairing a release by hand afterwards). It
# used to be copied into both. On 2026-09-10 the six Linux .deb/.rpm keys
# were added to release.yml only, so the repair tool would have DELETED
# six working update paths from any release it was pointed at — and its
# own check would have called the result "complete", because that check
# was written from the same six-name list as the builder it was checking.
# A list written by hand in more than one place drifts. This is that list,
# in one place.
#
# USAGE:
#   scripts/release/updater-manifest.sh build  <TAG>
#   scripts/release/updater-manifest.sh verify <TAG> <strict|warn>
#
# `build` rebuilds the manifest from the release's own signed assets and
# uploads it. `verify` re-downloads whatever is live on the release right
# now and checks it carries every key this release actually signed.
#
# ENVIRONMENT:
#   GITHUB_TOKEN        required — used by `gh`
#   GITHUB_REPOSITORY   the "owner/name" of the repo (set by Actions);
#                       `REPO` overrides it, which is what the tests use
#   UPDATER_WORKDIR     where `build` keeps downloaded signatures
#   UPDATER_VERIFY_DIR  where `verify` keeps the live manifest it fetches

set -euo pipefail

# Which repository to talk to. In GitHub Actions this is always set for
# us; letting `REPO` win means a test harness can point the script at a
# fake without pretending to be Actions.
REPO="${REPO:-${GITHUB_REPOSITORY:-}}"

# Working directories. Overridable so two runs (or a test) cannot tread
# on each other's files.
WORKDIR="${UPDATER_WORKDIR:-/tmp/updater}"
VERIFY_DIR="${UPDATER_VERIFY_DIR:-/tmp/updater-verify}"

# ── The one and only list of what this app ships and what key it gets ──
#
# Each row is: <signature filename>|<download filename>|<manifest key(s)>
#
# A row exists per installer FORMAT, not per platform, because each format
# is installed by a different tool (dpkg for .deb, rpm for .rpm) and
# carries its own signature. Handing a .deb user an .rpm fails the same
# way as handing them an AppImage — which is exactly what used to happen,
# because the only Linux key was the bare "linux-x86_64" one pointing at
# the AppImage, and the updater falls back to that when the more specific
# "linux-x86_64-deb" key is absent. An update offered and then failing is
# worse than no update offered.
#
# Windows rows carry two keys. The updater asks for "{os}-{arch}-nsis"
# first and falls back to "{os}-{arch}", so both point at the same file
# with the same signature.
#
# TO ADD A PLATFORM: add a row here. Nothing else needs editing — the
# build, the guard and the verify all read this function.
manifest_rows() {
  local v="$1"
  cat <<EOF
MeedyaDL.app.tar.gz.sig|MeedyaDL.app.tar.gz|darwin-aarch64
MeedyaDL_${v}_x64-setup.exe.sig|MeedyaDL_${v}_x64-setup.exe|windows-x86_64 windows-x86_64-nsis
MeedyaDL_${v}_arm64-setup.exe.sig|MeedyaDL_${v}_arm64-setup.exe|windows-aarch64 windows-aarch64-nsis
MeedyaDL_${v}_amd64.AppImage.sig|MeedyaDL_${v}_amd64.AppImage|linux-x86_64
MeedyaDL_${v}_amd64.deb.sig|MeedyaDL_${v}_amd64.deb|linux-x86_64-deb
MeedyaDL-${v}-1.x86_64.rpm.sig|MeedyaDL-${v}-1.x86_64.rpm|linux-x86_64-rpm
MeedyaDL_${v}_arm64.deb.sig|MeedyaDL_${v}_arm64.deb|linux-aarch64-deb
MeedyaDL-${v}-1.aarch64.rpm.sig|MeedyaDL-${v}-1.aarch64.rpm|linux-aarch64-rpm
MeedyaDL_${v}_armv7.deb.sig|MeedyaDL_${v}_armv7.deb|linux-armv7-deb
MeedyaDL-${v}-1.armv7.rpm.sig|MeedyaDL-${v}-1.armv7.rpm|linux-armv7-rpm
EOF
}

# Reject a tag carrying shell metacharacters before it reaches any
# command below (#977). release.yml already did this for its own tag;
# the repair tool never did — it passed a hand-typed workflow input
# straight through to `gh` and to string interpolation, unchecked.
require_valid_tag() {
  if ! [[ "$1" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]; then
    echo "::error::Invalid release tag format: '$1' (expected vX.Y.Z or vX.Y.Z-suffix)"
    exit 1
  fi
}

# Fail early and clearly rather than letting `gh` produce something
# confusing several steps later.
require_repo() {
  if [ -z "$REPO" ]; then
    echo "::error::No repository known — set GITHUB_REPOSITORY (Actions does this automatically) or REPO."
    exit 1
  fi
}

# Helper: fetch the contents of one .sig file from the release.
#
# Hardened after the v1.13.0-alpha.58 incident: windows-x86_64 and
# windows-x86_64-nsis silently vanished from latest.json even though
# every signature had been uploaded to the release 30 minutes earlier.
# The cause was two transient `gh release download` failures inside a
# ~2 second window — invisible because `2>/dev/null` threw away gh's own
# error text and the function had no retry, so one hiccup was treated
# exactly the same as "this platform never built".
#
# STDOUT DISCIPLINE: this function's stdout is captured by the caller as
# `SIG=$(download_sig ...)`, so only the signature bytes may ever reach
# stdout. Every diagnostic line below is explicitly redirected to stderr
# (`>&2` / `1>&2`) so it can never contaminate a signature and corrupt
# latest.json.
#
# Reads TAG, REPO, ASSETS and WORKDIR from the surrounding build run.
download_sig() {
  local sig_name="$1"
  if ! echo "$ASSETS" | grep -qF "$sig_name"; then
    return 1
  fi

  local attempt
  for attempt in 1 2 3; do
    # gh's stdout AND stderr both go to our stderr here — the download
    # writes the file to disk, it doesn't need to print anything to our
    # stdout, and dropping "2>/dev/null" means a real failure now shows
    # up in the log instead of vanishing.
    #
    # "|| true" so a failed download does not stop the whole script: this
    # loop is the retry, and the real test of success is the file check
    # immediately below, not gh's exit code.
    gh release download "$TAG" --repo "$REPO" --pattern "$sig_name" --dir "$WORKDIR" --clobber 1>&2 || true

    # -s (has content), not -f (merely exists): a zero-byte file left
    # behind by a truncated download must count as a failure, not a
    # false "success" with an empty signature.
    if [ -s "${WORKDIR}/${sig_name}" ]; then
      cat "${WORKDIR}/${sig_name}"
      return 0
    fi

    echo "::warning::download_sig: attempt ${attempt}/3 for ${sig_name} produced no usable file" >&2
    if [ "$attempt" -eq 1 ]; then
      sleep 5
    elif [ "$attempt" -eq 2 ]; then
      sleep 10
    fi
  done
  return 1
}

# ─────────────────────────── build ───────────────────────────
#
# Rebuilds latest.json from the release's real assets and uploads it.
#
# This exists because the parallel per-platform build jobs each upload
# their own latest.json containing only their own entry, and the last one
# to finish wins — losing every other platform. This runs once, after all
# of them, and puts the complete picture back.
cmd_build() {
  TAG="$1"
  require_valid_tag "$TAG"
  require_repo

  local version base
  version="${TAG#v}"
  base="https://github.com/${REPO}/releases/download/${TAG}"

  # Create the working directory up front. This used to exist only by
  # accident, as a side effect of the "download latest.json" call below
  # finding something to fetch — which meant a zero-platform run failed
  # (or didn't) in a way nobody had actually decided on. Creating it here
  # means every write below always has somewhere to land. The safety net
  # that accident used to provide is restored on purpose, explicitly,
  # right before the upload further down.
  mkdir -p "$WORKDIR"

  # Fetch whatever manifest is currently published for this tag. It is
  # used for two things: the notes and publication date, and — more
  # importantly — as the starting point for the rebuild.
  gh release download "$TAG" --repo "$REPO" --pattern "latest.json" --dir "$WORKDIR" --clobber 2>/dev/null || true

  local notes="" pub_date=""
  if [ -f "${WORKDIR}/latest.json" ]; then
    notes=$(jq -r '.notes // ""' "${WORKDIR}/latest.json")
    pub_date=$(jq -r '.pub_date // ""' "${WORKDIR}/latest.json")
  fi
  : "${notes:=See CHANGELOG.md for details.}"
  : "${pub_date:=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")}"

  # List every asset on the release, so we know which platforms actually
  # built and uploaded something this time.
  ASSETS=$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets[].name')

  # Diagnostic: dump the full asset listing to the log now, before any
  # download is attempted. If a platform goes missing from latest.json
  # again, this line tells us in one glance whether the asset was
  # actually there (a download problem, like alpha.58) or never got
  # uploaded in the first place (a build problem).
  echo "Release asset listing for ${TAG}:" >&2
  echo "$ASSETS" >&2

  # Seed from whatever is already published for THIS tag, rather than
  # starting from empty.
  #
  # WHY: starting from "{}" makes this a REPLACE, not a repair. Run the
  # repair tool on a healthy release and any key it doesn't know how to
  # write is deleted — which is precisely what would have happened before
  # this change, for all six Linux package keys. Seeding means a rebuild
  # can only add or refresh an entry, never remove one. It also means a
  # transient signature-download failure no longer deletes a key that was
  # already correct.
  #
  # The version guard matters: only seed if the existing file is for the
  # same version. A manifest left behind from a different release would
  # otherwise contribute URLs pointing at the wrong files.
  local platforms="{}"
  if [ -f "${WORKDIR}/latest.json" ]; then
    local existing_version
    existing_version=$(jq -r '.version // ""' "${WORKDIR}/latest.json")
    if [ "$existing_version" = "$version" ]; then
      # Insist it really is an object before adopting it. Everything
      # below adds keys to this value, so anything else (a published file
      # that got mangled, a hand-edited one) would produce a manifest the
      # app cannot read at all — worse than the gap we came to fix.
      platforms=$(jq -c 'if (.platforms | type) == "object" then .platforms else {} end' "${WORKDIR}/latest.json")
      echo "Seeded from the published manifest ($(echo "$platforms" | jq 'length') existing key(s)) — this rebuild can add and refresh, never remove." >&2
    else
      echo "Not seeding: the published manifest is for version '${existing_version}', not '${version}' — its URLs would point at the wrong files." >&2
    fi
  fi

  # Walk the one list of platforms. Every row whose signature can be
  # fetched contributes its key (or, for Windows, both of its keys).
  local sig_name asset_name keys key sig
  while IFS='|' read -r sig_name asset_name keys; do
    [ -n "$sig_name" ] || continue
    if sig=$(download_sig "$sig_name"); then
      for key in $keys; do
        platforms=$(echo "$platforms" | jq -c \
          --arg sig "$sig" --arg url "${base}/${asset_name}" --arg k "$key" \
          '. + {($k): {"signature": $sig, "url": $url}}')
        echo "Added ${key} to latest.json"
      done
    fi
  done < <(manifest_rows "$version")

  # Build the final latest.json.
  jq -n \
    --arg version "$version" \
    --arg notes "$notes" \
    --arg pub_date "$pub_date" \
    --argjson platforms "$platforms" \
    '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' \
    > "${WORKDIR}/latest.json"

  local platform_count
  platform_count=$(jq '.platforms | length' "${WORKDIR}/latest.json")
  echo "Rebuilt latest.json with ${platform_count} platform(s)"
  jq '.platforms | keys' "${WORKDIR}/latest.json"

  # Check every installer this release actually built against what ended
  # up in the manifest — instead of trusting a fixed list of platform
  # names written down by hand.
  #
  # #1166: this guard used to walk a fixed list of exactly four
  # platforms, with a comment claiming Linux ARM64 and ARMv7 "never
  # produce an updater signature on any release... they are deliberately
  # excluded from this list, or every run would warn about them." That
  # was false for ARM64 from the day its build started signing its
  # output, and it is the whole reason nobody noticed: the comment told
  # this check to stop looking at ARM entirely, so the check went on
  # agreeing with the comment for the entire life of the project instead
  # of ever contradicting it. A belief written into a comment and never
  # re-checked against the real assets is exactly how that bug survived.
  #
  # So this version looks at $ASSETS — the real file listing for this
  # release, already fetched above — rather than a list of names someone
  # has to remember to extend.
  local guard_failed=0
  while IFS='|' read -r sig_name asset_name keys; do
    [ -n "$sig_name" ] || continue

    if ! echo "$ASSETS" | grep -qF "$asset_name"; then
      # This installer was never built or never uploaded this run (e.g.
      # the experimental ARM Linux jobs are allowed to fail outright).
      # Nothing to check — there is no package to be missing an update
      # path for.
      continue
    fi

    if ! echo "$ASSETS" | grep -qF "$sig_name"; then
      # The installer exists but was never signed. This is a real, known
      # state during the ARMv7 rollout (and would be for any future
      # installer added without wiring up signing first), so it is a
      # warning, not a failure: the package can still be downloaded and
      # installed by hand, it just can't be offered as an automatic
      # update yet.
      echo "::warning::${asset_name} was published for ${TAG} with no matching .sig — it cannot be offered as an in-app update until it is signed."
      continue
    fi

    # The installer is signed. There is now no excuse for the manifest
    # not to carry its key(s) — if it doesn't, that is exactly the class
    # of bug this whole step exists to catch, so it fails the run rather
    # than warning into a log nobody is watching.
    for key in $keys; do
      if ! jq -e --arg p "$key" '.platforms | has($p)' "${WORKDIR}/latest.json" > /dev/null; then
        echo "::error::${asset_name} was published with a signature for ${TAG}, but latest.json has no '${key}' entry — in-app updates will not be offered for it."
        guard_failed=1
      fi
    done
  done < <(manifest_rows "$version")

  if [ "$guard_failed" -eq 1 ]; then
    exit 1
  fi

  # Guard against publishing an empty manifest. Before `mkdir -p` was
  # added above, a zero-platform result failed by accident: the `jq -n`
  # write would error because the directory didn't exist yet, and the
  # step went red under `-e` with nothing uploaded. Creating the
  # directory removes that accident, so the zero-platform case is now
  # checked and failed on purpose here instead of by luck.
  if [ "$platform_count" -eq 0 ]; then
    echo "::error::No platform signatures were found for ${TAG} — refusing to publish an empty latest.json (this would silently disable in-app updates for every platform)."
    exit 1
  fi

  gh release upload "$TAG" "${WORKDIR}/latest.json" --repo "$REPO" --clobber
  echo "Uploaded rebuilt latest.json"
}

# ─────────────────────────── verify ───────────────────────────
#
# Re-downloads the manifest that is actually LIVE on the release and
# checks it carries every key this release signed.
#
# The second argument decides what a gap means:
#
#   strict — used by release.yml, and only when every platform build
#            succeeded. A gap then is a real regression worth turning
#            the run red over.
#   warn   — used by fix-updater-manifest.yml. That workflow is
#            dispatched by a human specifically for releases where a
#            platform is already known to be missing — a manifest short a
#            few keys is the CORRECT, intended outcome of running it. If
#            it went red every time it was used for its most common
#            purpose, it would teach the operator that a red run here is
#            normal and worth ignoring, which is the exact failure mode
#            this whole effort exists to avoid. release.yml also drops to
#            warn when a build already failed, so the run isn't given a
#            second, redundant red X for an already-known failure.
cmd_verify() {
  TAG="$1"
  local mode="$2"
  require_valid_tag "$TAG"
  require_repo

  if [ "$mode" != "strict" ] && [ "$mode" != "warn" ]; then
    echo "::error::verify needs a mode of 'strict' or 'warn', got '${mode}'."
    exit 1
  fi

  local expected_version="${TAG#v}"

  # A separate directory from the build step, so this always re-fetches
  # the file that is actually live on the release right now, rather than
  # trusting a local copy left lying around from earlier.
  rm -rf "$VERIFY_DIR"
  mkdir -p "$VERIFY_DIR"

  # One place that decides whether a problem is fatal or merely noted.
  report() {
    if [ "$mode" = "strict" ]; then
      echo "::error::$1"
    else
      echo "::warning::$1"
    fi
  }

  local download_ok=0 attempt
  for attempt in 1 2 3; do
    if gh release download "$TAG" --repo "$REPO" --pattern "latest.json" --dir "$VERIFY_DIR" --clobber 1>&2; then
      download_ok=1
      break
    fi
    echo "::warning::Verify step: attempt ${attempt}/3 to download the published latest.json failed" >&2
    if [ "$attempt" -eq 1 ]; then
      sleep 5
    elif [ "$attempt" -eq 2 ]; then
      sleep 10
    fi
  done

  if [ "$download_ok" -ne 1 ]; then
    report "Could not download the published latest.json for ${TAG} after 3 attempts — unable to verify the updater manifest."
    if [ "$mode" = "strict" ]; then
      exit 1
    fi
    echo "Skipping further checks (the manifest could not be downloaded)."
    exit 0
  fi

  local manifest="${VERIFY_DIR}/latest.json"

  # latest.json stores the bare version, e.g. "1.13.0-alpha.58" — never
  # the "v"-prefixed tag. Comparing against the raw tag would fail this
  # check on every single release, so it is deliberately compared
  # against ${TAG#v}.
  local actual_version
  actual_version=$(jq -r '.version // ""' "$manifest")
  local version_ok=1
  if [ "$actual_version" != "$expected_version" ]; then
    report "Published latest.json version mismatch for ${TAG}: expected '${expected_version}', found '${actual_version}'."
    version_ok=0
  else
    echo "latest.json version OK: ${actual_version}"
  fi

  # Which keys to expect is decided by what this release actually
  # signed, not by a list written by hand.
  #
  # This is the #1166 lesson applied to the checker itself. The list this
  # replaces named six keys and printed "6/6 ... looks complete". A check
  # whose expected set is written by the same hand as the thing it checks
  # will agree with that hand forever — which is exactly how a manifest
  # missing half its keys passed as complete.
  #
  # It also removes the need to special-case the experimental ARM Linux
  # builds. If an ARM build failed there is no signature, so nothing is
  # expected and nothing warns — no tiering, no exception list.
  # If this listing fails, `assets` is empty and every row below is
  # skipped, which lands on the "nothing to check against" report further
  # down — a message that respects strict/warn, rather than the script
  # simply stopping here with an unexplained non-zero exit.
  local assets
  assets=$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets[].name' || true)

  local expected=0 missing=0
  local sig_name asset_name keys key sig
  while IFS='|' read -r sig_name asset_name keys; do
    [ -n "$sig_name" ] || continue
    # No signature published means this platform got no update path from
    # this release, and none was ever promised. Nothing to check.
    if ! echo "$assets" | grep -qF "$sig_name"; then
      continue
    fi
    for key in $keys; do
      expected=$((expected + 1))
      sig=$(jq -r --arg k "$key" '.platforms[$k].signature // ""' "$manifest")
      if [ -z "$sig" ]; then
        report "Published latest.json for ${TAG} is missing a non-empty signature for '${key}', even though ${asset_name} was signed for this release."
        missing=$((missing + 1))
      fi
    done
  done < <(manifest_rows "$expected_version")

  # No signatures at all is not a clean bill of health — it means either
  # every build failed or the asset listing could not be read. Say so
  # rather than printing a reassuring tick over an empty check.
  if [ "$expected" -eq 0 ]; then
    report "No signed installers were found on ${TAG}, so there was nothing to check the updater manifest against."
    if [ "$mode" = "strict" ]; then
      exit 1
    fi
    exit 0
  fi

  if [ "$missing" -eq 0 ] && [ "$version_ok" -eq 1 ]; then
    echo "✓ Published latest.json for ${TAG} carries all ${expected} key(s) that this release signed."
  fi

  if [ "$mode" = "strict" ] && { [ "$missing" -gt 0 ] || [ "$version_ok" -eq 0 ]; }; then
    exit 1
  fi
}

# ─────────────────────────── dispatch ───────────────────────────
main() {
  local subcommand="${1:-}"
  case "$subcommand" in
    build)
      if [ "$#" -ne 2 ]; then
        echo "::error::usage: $0 build <TAG>"
        exit 1
      fi
      cmd_build "$2"
      ;;
    verify)
      if [ "$#" -ne 3 ]; then
        echo "::error::usage: $0 verify <TAG> <strict|warn>"
        exit 1
      fi
      cmd_verify "$2" "$3"
      ;;
    *)
      echo "::error::usage: $0 build <TAG> | $0 verify <TAG> <strict|warn>"
      exit 1
      ;;
  esac
}

main "$@"
