#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
#
# scripts/media-lang/check_copies.py
#
# Keeps a repository's copies of the MWBM-MEDIA-LANG policy files identical
# to the master copies in MWBMPartners/MeedyaSuite-core.
#
# THIS FILE IS ITSELF COPIED VERBATIM into every repository that uses the
# policy, and listed in that repository's lock, so an accidental edit to the
# copy is caught. A deliberate edit to the checker cannot be caught by the
# checker itself — an edited checker can skip its own check — so a change
# to this file in a consumer repository is something review must notice.
# Change it in MeedyaSuite-core only.
#
# WHY COPIES AT ALL
# -----------------
# The policy must be readable from a plain checkout of each repository, by a
# person or by any coding assistant, with no network and no submodule. So
# each repository holds real copies. Copies drift unless something checks
# them — this script is that something, run in each repository's own CI.
#
# THE LOCK FILE (default: docs/standards/MWBM-MEDIA-LANG.lock)
# -----------------------------------------------------------
# Plain text, written by --init / --update, never by hand:
#
#   # comment lines start with '#'
#   policy MWBM-MEDIA-LANG 1.0.0
#   source MWBMPartners/MeedyaSuite-core <40-character commit>
#   file <sha256> <path in this repo> <path in MeedyaSuite-core>
#   file ...
#
# WHAT A NORMAL RUN CHECKS (CI runs this mode; every check must pass)
# -------------------------------------------------------------------
# 1. The lock is well formed: one policy line, one source line, no local
#    path listed twice.
# 2. Every master path is one of the known master files (MASTER_FILES), and
#    the lock includes all the REQUIRED ones. (An independent review on
#    28 Sept 2026 showed the first version would follow a master path such
#    as "../../../other/repo/README.md" to a file in an unrelated
#    repository, and that deleting a lock line quietly stopped a file being
#    checked. Both are closed here.) The three PHP implementation files
#    travel together, PER COPY: each copy of the PHP implementation keeps
#    the master's layout (MediaLanguagePolicy.php, README.md beside it,
#    tests/run-conformance.php under it), and every copy the lock names
#    must have all three lines (PHP_BINDING_LAYOUT, below, says why).
# 3. Every local path — and the lock file's own path — stays inside this
#    repository, is not inside .git, and is not a shortcut (symbolic link).
#    Files are written by creating a new file beside the old one and moving
#    it into place, so a hard link can never carry a write out of the
#    repository either. (A second review, by Codex on 28 Sept 2026, found
#    the lock path unchecked and hard links unhandled; both closed here.)
# 4. No file git tracks in the repository carries the name of a master file
#    without being listed in the lock (so removing a line cannot hide a
#    copy). For the two PHP files whose names are too common to search for
#    everywhere (README.md, run-conformance.php), the search is by layout
#    instead: when the lock names a copy of the PHP implementation, any
#    tracked .../tests/run-conformance.php, and any README.md beside a
#    MediaLanguagePolicy.php, must be in the lock too. Asking git rather than walking the folders means build output
#    (which git ignores — SwiftPM, for one, copies bundled data into .build)
#    is never mistaken for a hand-made copy, and a folder that cannot be
#    read cannot hide one. Outside a git checkout it falls back to walking
#    the folders, and any folder it cannot read fails the check.
# 5. Every local copy has the checksum recorded in the lock.
# 6. The recorded commit is part of MeedyaSuite-core's own history on one of
#    APPROVED_BRANCHES, asked of GitHub's compare lookup. (GitHub serves a
#    commit that exists only in someone's FORK under the original
#    repository's raw address, so without this a fork could supply an
#    edited "master". Also shown by the review.)
# 7. The lock's checksums match the master files downloaded at that commit.
# A download or lookup that fails for ANY reason fails the run: a check
# that could not run is never reported as a pass.
#
# --offline skips 6 and 7, says so loudly, and refuses to run at all when
# the CI or GITHUB_ACTIONS environment variable is set.
#
# WHAT IT CANNOT DO
# -----------------
# It does not tell you a newer policy version exists — moving to one is a
# deliberate change (the policy's section 8.3). It does not check that the
# repository's code follows the policy; the conformance test cases do that.
# It cannot protect against an edited copy of itself (see above).
#
# Usage (run from the repository root):
#   python3 scripts/media-lang/check_copies.py                 # verify (CI)
#   python3 scripts/media-lang/check_copies.py --offline       # local only
#   python3 scripts/media-lang/check_copies.py --update <commit>
#   python3 scripts/media-lang/check_copies.py --init <commit> \
#       --file <local path>=<master path> [--file ...]
# Set GITHUB_TOKEN to have the commit lookup authenticated (GitHub allows
# only 60 unauthenticated lookups an hour per address).

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request

MASTER_REPO = "MWBMPartners/MeedyaSuite-core"
RAW_URL = "https://raw.githubusercontent.com/{repo}/{commit}/{path}"
COMPARE_URL = "https://api.github.com/repos/{repo}/compare/{base}...{head}"
DEFAULT_LOCK = "docs/standards/MWBM-MEDIA-LANG.lock"
POLICY_MASTER_PATH = "docs/standards/media-language-bcp47-policy.md"

# The only files a lock may point at. Exact paths, not prefixes, so no
# path trick can reach anything else. Adding a master file means adding it
# here — a change to this script, which consumers then take by --update.
REQUIRED_MASTER_FILES = (
    POLICY_MASTER_PATH,
    "tests/fixtures/bcp47-language-policy-v1.json",
    "tests/fixtures/bcp47-language-policy-v1.schema.json",
    "docs/standards/data/bcp47-language-data-v1.json",
    "docs/standards/data/bcp47-language-data-v1.schema.json",
    "scripts/media-lang/check_copies.py",
)
OPTIONAL_MASTER_FILES = (
    "bindings/php/media-language/MediaLanguagePolicy.php",
    "bindings/php/media-language/tests/run-conformance.php",
    "bindings/php/media-language/README.md",
)
MASTER_FILES = REQUIRED_MASTER_FILES + OPTIONAL_MASTER_FILES
# The PHP implementation's files, which a repository copies all together or
# not at all - and each copy of them keeps the master's layout: this maps
# each master file to where it sits inside the copy's own folder (the folder
# holding MediaLanguagePolicy.php). The layout is not a matter of taste:
# the runner loads ../MediaLanguagePolicy.php, so a copy laid out any other
# way cannot run.
#
# Every COPY the lock names must list all three files. The copies are told
# apart by that folder, worked out from each file's local path. Why per
# copy: two of the three (run-conformance.php and README.md) are left out of
# DISTINCTIVE_NAMES below, so step 4 cannot spot an unlisted copy of them by
# name. Before the all-three rule, deleting the lock line for
# tests/run-conformance.php - the file that decides whether the conformance
# tests pass - quietly stopped it being checked: a review in a consuming
# repository deleted that line, made the runner `exit(0)`, and this checker
# still reported every copy as matching. The first version of the rule then
# counted the three master paths across the WHOLE lock, so a repository
# with two copies (app1/ and app2/) could still delete app2's runner line:
# app1's three lines satisfied the count (found by Codex's review r7).
# Policy section 8.3 says deleting a lock line must not switch a check off.
PHP_BINDING_LAYOUT = {
    "bindings/php/media-language/MediaLanguagePolicy.php": "MediaLanguagePolicy.php",
    "bindings/php/media-language/README.md": "README.md",
    "bindings/php/media-language/tests/run-conformance.php": "tests/run-conformance.php",
}
PHP_BINDING_MASTER_FILES = tuple(PHP_BINDING_LAYOUT)
assert set(PHP_BINDING_MASTER_FILES) == set(OPTIONAL_MASTER_FILES)
# Names distinctive enough that a file carrying one is certainly a copy.
# README.md and run-conformance.php are left out: other files legitimately
# share those names. (Their copies are still protected: by the all-three
# rule for each PHP copy above, and by step 4's search by layout.)
DISTINCTIVE_NAMES = tuple(sorted({os.path.basename(p) for p in MASTER_FILES}
                                 - {"README.md", "run-conformance.php"}))

# Branches of MeedyaSuite-core a pinned commit may come from. The policy
# work is merged into feature/work-in-progress and then main; while that is
# under way, its own branch is approved too. A commit reachable from any of
# these is accepted.
APPROVED_BRANCHES = ("main", "feature/work-in-progress", "feature/bcp47-language-policy")

COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SHA_RE = re.compile(r"^[0-9a-f]{64}$")
SAFE_PATH_RE = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.-]*(/[A-Za-z0-9_][A-Za-z0-9_.-]*)*$")


class CheckFailed(Exception):
    """A check did not pass; the message says why in plain words."""


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def download(commit, master_path):
    """Fetch one master file at one commit. Any failure raises: the caller
    must never treat "could not download" as "matches"."""
    if master_path not in MASTER_FILES:
        raise CheckFailed(f"refusing to download {master_path!r}: not a master file")
    url = RAW_URL.format(repo=MASTER_REPO, commit=commit, path=master_path)
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return response.read()
    except (urllib.error.URLError, OSError) as exc:
        raise CheckFailed(f"could not download {url} ({exc}). The check cannot run without "
                          "it, so it fails rather than passing.")


def commit_is_on_approved_branch(commit):
    """True if the commit is an ancestor of (or equal to) an approved branch
    of MeedyaSuite-core, per GitHub's compare lookup ("behind" or
    "identical" means the branch contains the commit). A commit that exists
    only in a fork is "diverged" or unknown, so it fails. A lookup that
    cannot be made raises — it is never read as "yes"."""
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "mwbm-media-lang-check"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    last_error = None
    for branch in APPROVED_BRANCHES:
        url = COMPARE_URL.format(repo=MASTER_REPO, base=urllib.parse.quote(branch, safe=""),
                                 head=commit)
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as r:
                status = json.load(r).get("status")
        except urllib.error.HTTPError as exc:
            if exc.code == 404:
                # The branch does not exist (any more) or the commit is not
                # in this repository's network: not an approval, keep looking.
                continue
            last_error = exc
            continue
        except (urllib.error.URLError, OSError, ValueError) as exc:
            last_error = exc
            continue
        if status in ("behind", "identical"):
            return True
    if last_error is not None:
        raise CheckFailed(f"could not ask GitHub whether {commit[:12]} is part of {MASTER_REPO}'s "
                          f"history ({last_error}). The check fails rather than passing.")
    return False


def read_policy_version(text):
    """The version printed in the policy document's header table."""
    match = re.search(r"\|\s*\*\*Version\*\*\s*\|\s*`([0-9]+\.[0-9]+\.[0-9]+)`", text)
    return match.group(1) if match else None


def check_local_path(path, root):
    """A local path must be a plain relative path inside the repository,
    outside .git, and not a symbolic link (or inside one)."""
    if not SAFE_PATH_RE.match(path) or any(part in (".", "..") for part in path.split("/")):
        raise CheckFailed(f"local path {path!r} is not a plain relative path")
    if path == ".git" or path.startswith(".git/"):
        raise CheckFailed(f"local path {path!r} is inside .git")
    full = os.path.join(root, path)
    real_root = os.path.realpath(root)
    if os.path.commonpath([os.path.realpath(full), real_root]) != real_root:
        raise CheckFailed(f"local path {path!r} leads outside the repository")
    probe = root
    for part in path.split("/"):
        probe = os.path.join(probe, part)
        if os.path.islink(probe):
            raise CheckFailed(f"local path {path!r} goes through a symbolic link, which is refused")


def php_copy_folder(entry, where):
    """The folder of the PHP implementation copy that `entry` (a lock or
    --init entry for one of the PHP files) belongs to: its local path with
    the file's place in the layout (PHP_BINDING_LAYOUT) taken off the end.
    "" means the repository root. Fails if the local path does not end in
    that place - a copy laid out differently cannot run, and could not be
    told apart from another copy."""
    place = PHP_BINDING_LAYOUT[entry["master"]]
    local = entry["local"]
    if local == place:
        return ""
    if local.endswith("/" + place):
        return local[: -len(place) - 1]
    raise CheckFailed(
        f"{where}: {local} is a copy of {entry['master']}, but a copy of the PHP "
        f"implementation must keep the master's layout, so this file must be at "
        f"<the copy's folder>/{place} (the runner loads ../MediaLanguagePolicy.php).")


def php_copies(entries, where):
    """The lock's (or --init's) PHP implementation copies: a dict from each
    copy's folder to the set of master paths listed for it."""
    copies = {}
    for entry in entries:
        if entry["master"] in PHP_BINDING_LAYOUT:
            copies.setdefault(php_copy_folder(entry, where), set()).add(entry["master"])
    return copies


def check_php_binding_complete(entries, where):
    """The all-three rule for PHP_BINDING_LAYOUT, per copy: fail when any copy
    of the PHP implementation that `entries` (a lock's or an --init's) names
    lists some of its three files but not all of them."""
    for folder, masters in sorted(php_copies(entries, where).items()):
        missing = [m for m in PHP_BINDING_MASTER_FILES if m not in masters]
        if missing:
            shown = folder or "the repository root"
            raise CheckFailed(
                f"{where} names some of the PHP implementation's files but not all, for the copy "
                f"in {shown}: {', '.join(missing)} is missing. Every copy of the PHP "
                f"implementation must list all three files ({', '.join(PHP_BINDING_MASTER_FILES)}) "
                "- otherwise deleting one line would quietly stop that file being checked "
                "(policy section 8.3).")


def parse_lock(lock_path, root):
    if not os.path.isfile(lock_path):
        raise CheckFailed(f"no lock file at {lock_path}")
    policy_version = commit = None
    files, seen_local = [], set()
    with open(lock_path, encoding="utf-8") as f:
        for number, raw in enumerate(f, 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if parts[0] == "policy" and len(parts) == 3 and parts[1] == "MWBM-MEDIA-LANG":
                if policy_version is not None:
                    raise CheckFailed(f"{lock_path} line {number}: a second policy line")
                policy_version = parts[2]
            elif parts[0] == "source" and len(parts) == 3 and parts[1] == MASTER_REPO:
                if commit is not None:
                    raise CheckFailed(f"{lock_path} line {number}: a second source line")
                commit = parts[2]
            elif parts[0] == "file" and len(parts) == 4 and SHA_RE.match(parts[1]):
                entry = {"sha256": parts[1], "local": parts[2], "master": parts[3]}
                if entry["master"] not in MASTER_FILES:
                    raise CheckFailed(f"{lock_path} line {number}: {entry['master']!r} is not a master file")
                check_local_path(entry["local"], root)
                if entry["local"] in seen_local:
                    raise CheckFailed(f"{lock_path} line {number}: {entry['local']} is listed twice")
                seen_local.add(entry["local"])
                files.append(entry)
            else:
                raise CheckFailed(f"{lock_path} line {number} is not understood: {line!r}")
    if not policy_version or not commit or not COMMIT_RE.match(commit):
        raise CheckFailed(f"{lock_path} must have one policy line and one source line with a full "
                          "40-character commit")
    missing = [m for m in REQUIRED_MASTER_FILES if m not in {e["master"] for e in files}]
    if missing:
        raise CheckFailed(f"{lock_path} leaves out files every copy must have: {', '.join(missing)}")
    check_php_binding_complete(files, lock_path)
    return policy_version, commit, files


# The names step 4 collects while searching the repository: the distinctive
# ones, plus the two PHP names it checks by layout.
PHP_RUNNER_NAME = "run-conformance.php"
PHP_README_NAME = "README.md"
PHP_POLICY_NAME = "MediaLanguagePolicy.php"
SEARCHED_NAMES = frozenset(DISTINCTIVE_NAMES) | {PHP_RUNNER_NAME, PHP_README_NAME}


def unlisted_copies(root, files):
    """Files that are, or are laid out like, a copy of a master file but are
    not in the lock: a list of (path, why) pairs.

    - A file with a DISTINCTIVE_NAMES name, anywhere.
    - When the lock names a copy of the PHP implementation: EVERY tracked
      file named run-conformance.php, wherever it sits, and any README.md
      beside a MediaLanguagePolicy.php or in a locked copy's folder.
      (README.md is too common a name to search for by name alone, so it is
      found by where it sits. Before Codex's review r7 neither name was
      searched for at all; a runner that no lock line names could be run by
      CI unchecked. Until the stand-in review of revision 5 a runner was
      found only in the copy layout, <folder>/tests/run-conformance.php, so
      one moved to <copy>/ci/run-conformance.php - where a CI step could
      still run it - was not reported. A consumer that keeps a runner of its
      own under that name, not a copy of the master's, must now rename it.)

    Uses git's list of tracked files when this is a git checkout; otherwise
    walks the folders and fails if any folder cannot be read."""
    listed = {os.path.normpath(e["local"]) for e in files}
    found = set()
    if os.path.exists(os.path.join(root, ".git")):
        try:
            out = subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True,
                                 capture_output=True).stdout
        except (OSError, subprocess.CalledProcessError) as exc:
            raise CheckFailed(f"could not list the repository's files with git ({exc}); "
                              "the check fails rather than passing.")
        for raw in out.split(b"\0"):
            if raw:
                rel = raw.decode("utf-8", errors="replace")
                if os.path.basename(rel) in SEARCHED_NAMES:
                    found.add(os.path.normpath(rel))
    else:
        def refuse(exc):
            raise CheckFailed(f"could not read {exc.filename} while looking for unlisted copies "
                              f"({exc.strerror}); the check fails rather than passing.")
        for dirpath, dirnames, filenames in os.walk(root, onerror=refuse):
            dirnames[:] = [d for d in dirnames if d != ".git"]
            for d in dirnames:
                if os.path.islink(os.path.join(dirpath, d)):
                    # Nothing can be vouched for behind a linked folder (it
                    # may lead anywhere, and following links can loop), so
                    # outside a git checkout it fails the check. Inside a
                    # git checkout git lists what is tracked, links included.
                    rel = os.path.relpath(os.path.join(dirpath, d), root)
                    raise CheckFailed(f"{rel} is a link to a folder; outside a git checkout the check "
                                      "cannot see behind it, so it fails. Run it in a git checkout.")
            for name in filenames:
                if name in SEARCHED_NAMES:
                    found.add(os.path.normpath(os.path.relpath(os.path.join(dirpath, name), root)))

    unlisted = []
    for path in found - listed:
        if os.path.basename(path) in DISTINCTIVE_NAMES:
            unlisted.append((path, "has the name of a master file but is not in the lock"))

    copies = php_copies(files, "the lock")
    if copies:
        # The folders a PHP copy sits in: every copy the lock names, and
        # every folder holding a tracked MediaLanguagePolicy.php (listed or
        # not - an unlisted one is reported above by its name).
        folders = set(copies) | {os.path.dirname(p) for p in found
                                 if os.path.basename(p) == PHP_POLICY_NAME}
        for path in found - listed:
            name = os.path.basename(path)
            parent = os.path.dirname(path)
            if name == PHP_RUNNER_NAME and os.path.basename(parent) == "tests":
                unlisted.append((path, "is laid out like a copy of the PHP conformance runner "
                                       "(<folder>/tests/run-conformance.php) but is not in the lock"))
            elif name == PHP_RUNNER_NAME:
                unlisted.append((path, "has the PHP conformance runner's name but is not in the "
                                       "lock; while the lock names a copy of the PHP "
                                       "implementation, every tracked run-conformance.php must "
                                       "be in it, wherever it sits"))
            elif name == PHP_README_NAME and parent in folders:
                unlisted.append((path, "sits where a copy of the PHP implementation keeps its "
                                       "README.md but is not in the lock"))
    return sorted(unlisted)


def mismatch_message(entry, data, actual):
    """Say what is wrong with a copy, naming line-ending conversion when
    that is the whole difference (a false failure on Windows checkouts)."""
    if sha256_bytes(data.replace(b"\r\n", b"\n")) == entry["sha256"]:
        return (f"{entry['local']} differs only in its line endings — something converted them. "
                "Mark the policy copies unconverted in .gitattributes (for example "
                f"'{entry['local']} -text') and check them out again.")
    return (f"{entry['local']} has been changed (checksum {actual[:12]}…, lock says "
            f"{entry['sha256'][:12]}…). Copies must not be edited here: change the master in "
            "MeedyaSuite-core and run --update.")


def replace_file(full, data):
    """Write data to full by creating a NEW file in the same folder and
    moving it over the old name. Writing into the existing file instead
    would also change any other name hard-linked to it — possibly a file
    outside the repository."""
    folder = os.path.dirname(full) or "."
    os.makedirs(folder, exist_ok=True)
    # The new file must keep the permissions the old one had — an
    # executable checker must stay executable, and a PHP copy must stay
    # readable by a web server running as another account. mkstemp()
    # creates files readable only by their owner, so set the mode
    # explicitly: the old file's mode, or for a new file the usual default
    # (0666 less the process's umask). Found by Codex's third round.
    if os.path.exists(full):
        mode = stat.S_IMODE(os.stat(full).st_mode)
    else:
        umask = os.umask(0)
        os.umask(umask)
        mode = 0o666 & ~umask
    fd, tmp = tempfile.mkstemp(dir=folder, prefix=".media-lang-")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.chmod(tmp, mode)
        os.replace(tmp, full)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def write_lock(lock_path, policy_version, commit, files):
    lines = ["# MWBM-MEDIA-LANG copy lock. Written by scripts/media-lang/check_copies.py\n",
             "# --init / --update; do not edit by hand. Master: MWBMPartners/MeedyaSuite-core.\n",
             f"policy MWBM-MEDIA-LANG {policy_version}\n",
             f"source {MASTER_REPO} {commit}\n"]
    lines += [f"file {e['sha256']} {e['local']} {e['master']}\n" for e in files]
    replace_file(lock_path, "".join(lines).encode("utf-8"))


def fetch_into_place(commit, files, root):
    """Download EVERY file first, then write them all. A download that fails
    part-way therefore changes nothing on disk. Returns the policy version
    read from the policy document."""
    if not commit_is_on_approved_branch(commit):
        raise CheckFailed(f"{commit[:12]} is not part of {MASTER_REPO}'s history on "
                          f"{', '.join(APPROVED_BRANCHES)}; refusing to copy from it")
    downloaded = [(entry, download(commit, entry["master"])) for entry in files]
    policy_version = None
    for entry, data in downloaded:
        if entry["master"] == POLICY_MASTER_PATH:
            policy_version = read_policy_version(data.decode("utf-8"))
    if not policy_version:
        raise CheckFailed(f"the policy document at {commit[:12]} does not state a version")
    for entry, data in downloaded:
        replace_file(os.path.join(root, entry["local"]), data)
        entry["sha256"] = sha256_bytes(data)
    return policy_version


def verify(lock_path, root, offline):
    policy_version, commit, files = parse_lock(lock_path, root)
    problems = []
    for rel, why in unlisted_copies(root, files):
        problems.append(f"{rel} {why}")
    for entry in files:
        full = os.path.join(root, entry["local"])
        if not os.path.isfile(full):
            problems.append(f"{entry['local']} is missing")
            continue
        with open(full, "rb") as f:
            data = f.read()
        actual = sha256_bytes(data)
        if actual != entry["sha256"]:
            problems.append(mismatch_message(entry, data, actual))
        if entry["master"] == POLICY_MASTER_PATH:
            stated = read_policy_version(data.decode("utf-8", errors="replace"))
            if stated != policy_version:
                problems.append(f"{entry['local']} says version {stated}, lock says {policy_version}")
    if problems:
        raise CheckFailed("\n  " + "\n  ".join(problems))
    if offline:
        return (f"MWBM-MEDIA-LANG {policy_version}: {len(files)} local copies match the lock. "
                "NOT CHECKED against the master (--offline).")
    if not commit_is_on_approved_branch(commit):
        raise CheckFailed(f"the lock's commit {commit[:12]} is not part of {MASTER_REPO}'s "
                          f"history on {', '.join(APPROVED_BRANCHES)}")
    for entry in files:
        if sha256_bytes(download(commit, entry["master"])) != entry["sha256"]:
            problems.append(f"{entry['local']}: the lock's checksum does not match "
                            f"{entry['master']} at {commit[:12]} in {MASTER_REPO}")
    if problems:
        raise CheckFailed("\n  " + "\n  ".join(problems))
    return (f"MWBM-MEDIA-LANG {policy_version}: {len(files)} copies match the master at "
            f"{MASTER_REPO}@{commit[:12]}.")


def main(argv=None):
    ap = argparse.ArgumentParser(description="Check or update copies of the MWBM-MEDIA-LANG policy files.")
    ap.add_argument("--lock", default=DEFAULT_LOCK, help=f"lock file path (default {DEFAULT_LOCK})")
    ap.add_argument("--offline", action="store_true", help="check local copies only (refused in CI)")
    ap.add_argument("--update", metavar="COMMIT", help="re-fetch every locked file at COMMIT")
    ap.add_argument("--init", metavar="COMMIT", help="first-time setup at COMMIT (with --file)")
    ap.add_argument("--file", action="append", default=[], metavar="LOCAL=MASTER",
                    help="with --init: a file to copy, as local-path=master-path")
    args = ap.parse_args(argv)
    root = os.getcwd()
    try:
        # The lock file is written by --init/--update, so its path gets the
        # same checks as a copy's path, applied to the path EXACTLY as given
        # (a plain relative path, no "." or ".." parts, no symbolic link on
        # the way). An earlier version tidied the path first, so
        # "jump/../escaped.lock" was checked as "escaped.lock" but written
        # through the link "jump" — found by Codex's third round.
        check_local_path(args.lock, root)
        if args.offline and (os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS")):
            raise CheckFailed("--offline does not check the master, so it is refused in CI "
                              "(the CI or GITHUB_ACTIONS variable is set)")
        if args.update or args.init:
            commit = args.update or args.init
            if not COMMIT_RE.match(commit):
                raise CheckFailed("give a full 40-character commit, not a branch or short hash")
            if args.init:
                if not args.file:
                    raise CheckFailed("--init needs --file local=master for each file")
                files, seen = [], set()
                for spec in args.file:
                    if "=" not in spec:
                        raise CheckFailed(f"--file {spec!r} must be local-path=master-path")
                    local, master = spec.split("=", 1)
                    if master not in MASTER_FILES:
                        raise CheckFailed(f"{master!r} is not a master file")
                    check_local_path(local, root)
                    if local in seen:
                        raise CheckFailed(f"{local} is given twice")
                    seen.add(local)
                    files.append({"sha256": "", "local": local, "master": master})
                missing = [m for m in REQUIRED_MASTER_FILES if m not in {e["master"] for e in files}]
                if missing:
                    raise CheckFailed(f"--init must include every required file; missing: {', '.join(missing)}")
                check_php_binding_complete(files, "--init")
            else:
                _, _, files = parse_lock(args.lock, root)
            policy_version = fetch_into_place(commit, files, root)
            write_lock(args.lock, policy_version, commit, files)
            print(f"Copied {len(files)} files at {commit[:12]}; lock written to {args.lock} "
                  f"(policy {policy_version}). Now update code and tests as the policy changelog requires.")
            return 0
        print(verify(args.lock, root, args.offline))
        return 0
    except CheckFailed as exc:
        print(f"MWBM-MEDIA-LANG copy check FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
