#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Check whether a channel branch already has a lockfile security fix.

WHY THIS EXISTS
---------------
`forward-port-security.yml` used to find out whether a channel branch
"already has" a security fix only by trying to `git cherry-pick` the fix
commit onto it, and treating a conflict as "needs a hand-written PR" —
which is wrong for a dependency lockfile. A lockfile is one giant
machine-generated block of text; the same one-line dependency bump can
sit in a completely different place in the file on two branches that have
otherwise drifted, so a plain textual cherry-pick conflicts constantly even
when the branch is perfectly healthy. That happened for real: three
forward-port attempts (#1143, #1144, #1145) all conflicted and each opened
a "please forward-port this by hand" issue, and all three were false
alarms — every target branch already had the fix. See issue #1165.

So instead of asking git a textual question ("does this patch apply
cleanly?"), this asks the real question directly: does the target branch's
lockfile already have every dependency change the fix made? That is a
version comparison, not a diff, so drift elsewhere in the file cannot
produce a false conflict.

THE ONE RULE EVERYTHING HERE FOLLOWS
------------------------------------
A wrong "already fixed" makes the workflow skip a branch silently, with no
pull request and no issue, so a security fix never reaches a branch that
needed it. A wrong "needs the fix" only costs a cherry-pick attempt, a pull
request or an issue that a person can close. So whenever this script cannot
be sure, it answers "needs the fix". Every "cannot tell" below means
exactly that.

WHAT COUNTS AS "ALREADY FIXED"
------------------------------
Given the commit before the fix (`--old`, normally `<sha>^`), the fix
commit (`--new`) and the branch to check (`--target`), the answer is
"already fixed" only if ALL of these hold:

  1. Only dependency files changed. Every file the fix commit changed is
     one of the lockfiles named on the command line or a manifest named
     with `--manifest` (the workflow names `package.json` and
     `src-tauri/Cargo.toml`). A code change, a workflow change, anything
     else, means "needs the fix": this script cannot see whether the target
     has that change, so it must not let the workflow skip it. (Codex found
     the case where this mattered: a fix that turns off an unsafe Cargo
     feature AND bumps a crate. A branch that already had the bump still
     had the unsafe feature, and the old shortcut skipped the whole fix.)

  2. Every manifest change is already on the target. For each named
     manifest the fix changed, every line the fix added must already be in
     the target's copy of that file, and every line it removed must be
     absent from it. Lines are compared after trimming leading and trailing
     whitespace and one trailing comma, so `"undici": "^7.30.0"` (the last
     entry in a JSON object) matches `"undici": "^7.30.0",` (the same entry
     with another after it). Note what this cannot do: a line that only
     changed its trailing comma counts as both removed and added, so it can
     never be satisfied, and a line moved to another part of the file (into
     another TOML table, say) is not noticed as moved. The first only costs
     a cherry-pick; the second is why the comparison stays this literal
     rather than trying to be clever about it.

  3. At least one named lockfile changed, and every package whose versions
     the fix changed is fixed on the target:
       - EVERY copy counts. npm can install the same package at several
         nested paths (`node_modules/a/node_modules/x`), under an alias
         (an entry at `node_modules/y` whose `name` is `x`), and Cargo can
         lock several versions of one crate. A package is identified by its
         real name, and every copy of it on the target must be at or above
         the version the fix moved it to. (Codex found both ways the old
         check missed this: a vulnerable nested npm copy beside a fixed
         top-level one, and a vulnerable Cargo copy beside a fixed one.)
       - Release lines are kept apart. Versions are grouped by the line a
         semver range would treat as compatible: the major version from 1.0
         up, each `0.x` below that, and each `0.0.x` on its own. If the
         target has a copy on a line the fix did not touch, there is nothing
         to compare it with, so the answer is "cannot tell".
       - A package the fix ADDED must be on the target, at or above the
         added version. (Codex found the old check ignored additions
         whenever anything else also changed.)
       - A package the fix REMOVED must be gone from the target — every
         copy, at any version.
       - A fix that moves a version DOWN (to step back from a bad release,
         say) cannot be described as "at or above", so it is "cannot tell".

Anything this script cannot read — a lockfile missing or unparseable on
any side, a version it cannot parse, two pre-release labels it would have
to put in order (`1.0.3-beta.2` against `1.0.3-beta.10`), a manifest
change git reports without any changed lines (a binary or permissions-only
change) — is "cannot tell".

If every check passes it prints "RESULT=ALREADY_FIXED" and exits 0.
Otherwise it prints "RESULT=NEEDS_PORT" and exits 1, and the workflow goes
on to its normal cherry-pick (which still has its own "did this change
anything?" check afterwards). A crash exits non-zero without printing
ALREADY_FIXED, which the workflow also treats as "needs the fix". DETAIL
lines after the RESULT line say, in plain words, what was found.

WHAT THIS CANNOT KNOW
---------------------
  - Whether any version is actually vulnerable. It compares the target
    with what main moved to; it never reads an advisory database. (That is
    `channel-security-audit.yml`'s job.)
  - Whether two copies with the same name and version come from different
    places (a registry release and a git fork, say). They count as the same.
  - Anything about a lockfile type other than `package-lock.json` (npm,
    lockfileVersion 2 or 3) and `Cargo.lock`. Any other named lockfile the
    fix changed is "cannot tell".

THE NARROWER MODE THE WORKFLOW'S GATE USES
------------------------------------------
`--lockfile-changes-only` answers a different question: "are this commit's
lockfile changes all contained in that commit?" The gate uses it to check
that a combining pull request really carries the Dependabot fix its
description names (fix = the Dependabot pull request's own commit, target
= the combining pull request's merge commit). In this mode files other than
the named lockfiles are ignored, because that pull request's other files are
checked separately, and `--manifest` is refused. The workflow's
"already fixed" shortcut never uses this mode.

HOW THE FILES ARE READ
----------------------
  - `package-lock.json` is real JSON, read with the standard `json` module.
    Its `packages` section is used; a file without one (lockfileVersion 1)
    is "cannot read". Entries marked `"link": true` are pointers to a local
    folder rather than installed copies, and are skipped.
  - `Cargo.lock` is read block by block with a small hand-written reader
    rather than a TOML library, matching the house style of
    `tools/audit-checks/check_codec_registry.py`: no third-party
    dependency, and no reliance on `tomllib`, which only exists on Python
    3.11+. Each `[[package]]` block's `name` and `version` keys are read
    wherever they sit in the block; a block with a name but no readable
    version is kept as "version unknown", so it can never vanish from the
    comparison. (The earlier reader assumed `version` always sat on the
    line straight after `name`, and silently dropped any block where it
    did not.)
"""

from __future__ import annotations

import argparse
import functools
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

# A `[[package]]` header in Cargo.lock, and any other table header (which
# ends the current package block — e.g. `[metadata]`, `[[patch.unused]]`).
CARGO_PACKAGE_HEADER_RE = re.compile(r"^\[\[package\]\]\s*$")
CARGO_ANY_HEADER_RE = re.compile(r"^\[")
CARGO_NAME_RE = re.compile(r'^name\s*=\s*"(.+)"\s*$')
CARGO_VERSION_RE = re.compile(r'^version\s*=\s*"(.+)"\s*$')

# The leading numeric part of a version ("1.2.3") and whatever follows it.
VERSION_RE = re.compile(r"^(\d+(?:\.\d+)*)(.*)$")

# Every copy of every package, by name: {name: [version, ...]}. A version
# of None means "this copy exists, but its version could not be read".
Copies = dict[str, list]


# ---------------------------------------------------------------------------
# Reading from git
# ---------------------------------------------------------------------------


def run_git(repo_root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", *args],
        cwd=repo_root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )


def git_show(repo_root: Path, ref: str, path: str) -> str | None:
    """The file's content at `ref`, or None if it cannot be read there (the
    file does not exist on that commit, or the ref itself is bad). Both are
    "cannot read" to every caller; neither is ever read as an empty file."""
    result = run_git(repo_root, "show", f"{ref}:{path}")
    if result.returncode != 0:
        return None
    return result.stdout


def changed_files(repo_root: Path, old_ref: str, new_ref: str) -> list[str] | None:
    """Every file that differs between the two commits, or None if git
    could not say. `--no-renames` so a renamed file shows both its names."""
    result = run_git(repo_root, "diff", "--no-renames", "--name-only", old_ref, new_ref)
    if result.returncode != 0:
        return None
    return [line for line in result.stdout.split("\n") if line]


def manifest_line_changes(repo_root: Path, old_ref: str, new_ref: str, path: str) -> tuple[list[str], list[str]] | None:
    """(lines the fix added, lines the fix removed) in one file, as git's own
    diff reports them, or None if git could not produce the diff."""
    result = run_git(
        repo_root,
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--no-renames",
        "--unified=0",
        old_ref,
        new_ref,
        "--",
        path,
    )
    if result.returncode != 0:
        return None
    added: list[str] = []
    removed: list[str] = []
    in_hunk = False
    for line in result.stdout.split("\n"):
        # Everything before the first "@@" is the file header ("--- a/...",
        # "+++ b/..."), which must not be mistaken for changed lines.
        if line.startswith("@@"):
            in_hunk = True
        elif in_hunk and line.startswith("+"):
            added.append(line[1:])
        elif in_hunk and line.startswith("-"):
            removed.append(line[1:])
    return added, removed


# ---------------------------------------------------------------------------
# Versions
# ---------------------------------------------------------------------------


def parse_version(v: str) -> tuple[tuple[int, ...], str] | None:
    """Split "1.2.3-beta.1" into ((1, 2, 3), "-beta.1"), padding the numbers
    to at least three parts. None if the version does not start with a
    number at all — such a version is "cannot read"."""
    m = VERSION_RE.match(v.strip())
    if not m:
        return None
    nums = tuple(int(p) for p in m.group(1).split("."))
    nums = nums + (0,) * (3 - len(nums))
    return nums, m.group(2)


def version_gte(a: str, b: str) -> bool:
    """True only if `a` is certainly the same release as `b` or a newer one.

    The numbers decide it whenever they differ. When they are equal, a plain
    release outranks a pre-release of the same numbers, as in semver. Two
    DIFFERENT suffixes on equal numbers ("-beta.2" against "-beta.10", or
    build labels such as "+build1") are not put in order: this used to
    compare them as plain text, which calls "-beta.2" newer than
    "-beta.10". It now answers False, which every caller reads as "needs the
    fix". An unreadable version is never "certainly newer"."""
    pa, pb = parse_version(a), parse_version(b)
    if pa is None or pb is None:
        return False
    (na, ra), (nb, rb) = pa, pb
    length = max(len(na), len(nb))
    na = na + (0,) * (length - len(na))
    nb = nb + (0,) * (length - len(nb))
    if na != nb:
        return na > nb
    if ra == rb or ra == "":
        return True
    return False


def version_sort_key(v: str):
    """A best-effort ordering, used only to pick which version to SHOW in
    a message. Decisions never rely on it; they use `version_gte`."""
    parsed = parse_version(v)
    return (parsed[0], parsed[1] == "", parsed[1]) if parsed else ((), False, v)


def release_line(v: str) -> tuple[int, ...]:
    """The group of versions a semver range treats as compatible with `v`:
    (major,) from 1.0 up, (0, minor) for 0.x, (0, 0, patch) for 0.0.x.
    Only called on versions that have already been checked as readable."""
    nums, _ = parse_version(v)  # type: ignore[misc]
    major, minor, patch = nums[0], nums[1], nums[2]
    if major > 0:
        return (major,)
    if minor > 0:
        return (0, minor)
    return (0, 0, patch)


def describe_line(line: tuple[int, ...]) -> str:
    if len(line) == 1:
        return f"{line[0]}.x"
    if len(line) == 2:
        return f"0.{line[1]}.x"
    return f"0.0.{line[2]}"


def by_line(versions: list[str]) -> dict[tuple[int, ...], Counter]:
    grouped: dict[tuple[int, ...], Counter] = {}
    for v in versions:
        grouped.setdefault(release_line(v), Counter())[v] += 1
    return grouped


def show_versions(versions) -> str:
    unique = sorted(set(versions), key=version_sort_key)
    return ", ".join(unique)


# ---------------------------------------------------------------------------
# Reading lockfiles
# ---------------------------------------------------------------------------


def npm_package_name(key: str, meta: dict) -> str:
    """The real name of the package installed at `key`. An aliased install
    records its real name in "name"; otherwise the name is whatever follows
    the last `node_modules/` (which keeps scoped names like `@a/b` whole).
    The root entry ("") is the project itself."""
    if key == "":
        return ""
    name = meta.get("name")
    if isinstance(name, str) and name:
        return name
    if "node_modules/" in key:
        return key.rsplit("node_modules/", 1)[1]
    return key  # a workspace folder such as "packages/x"


def parse_npm_lock(content: str) -> Copies | None:
    """Every installed copy, by real package name, from a package-lock.json,
    or None if it is not JSON or has no `packages` section to read."""
    try:
        data = json.loads(content)
    except json.JSONDecodeError:
        return None
    packages = data.get("packages") if isinstance(data, dict) else None
    if not isinstance(packages, dict):
        return None
    out: Copies = {}
    for key, meta in packages.items():
        if not isinstance(meta, dict):
            continue
        # A link points at a local folder that has its own entry; it is not
        # an installed copy of anything.
        if meta.get("link") is True:
            continue
        version = meta.get("version")
        out.setdefault(npm_package_name(key, meta), []).append(version if isinstance(version, str) else None)
    return out


def parse_cargo_lock(content: str) -> Copies | None:
    """Every locked crate copy, by name, from a Cargo.lock, or None if the
    file has no `[[package]]` block at all (a real Cargo.lock always lists
    at least the project's own crate, so none means it could not be read)."""
    out: Copies = {}
    blocks = 0
    name: str | None = None
    version: str | None = None
    in_block = False

    def finish() -> None:
        if in_block and name is not None:
            out.setdefault(name, []).append(version)

    for raw in content.split("\n"):
        line = raw.strip()
        if CARGO_PACKAGE_HEADER_RE.match(line):
            finish()
            in_block, name, version = True, None, None
            blocks += 1
            continue
        if CARGO_ANY_HEADER_RE.match(line):
            finish()
            in_block, name, version = False, None, None
            continue
        if not in_block:
            continue
        m = CARGO_NAME_RE.match(line)
        if m:
            name = m.group(1)
            continue
        m = CARGO_VERSION_RE.match(line)
        if m:
            version = m.group(1)
    finish()
    return out if blocks else None


# ---------------------------------------------------------------------------
# Deciding
# ---------------------------------------------------------------------------


def judge_package(label: str, name: str, old: list, new: list, target: list) -> tuple[bool, list[str]]:
    """Is one package, whose copies the fix changed, fixed on the target?"""
    shown = f'"{name}"' if name else "the project's own entry"

    for side, copies in (("before the fix", old), ("after the fix", new), ("on the target branch", target)):
        unreadable = [v for v in copies if v is None or parse_version(v) is None]
        if unreadable:
            what = ", ".join("(no version)" if v is None else v for v in unreadable)
            return False, [f"{label}: {shown} has a copy {side} whose version cannot be read ({what}) — cannot tell, so treating it as needing the fix"]

    if not new:
        if target:
            return False, [f"{label}: the fix removed every copy of {shown}, but the target branch still has {show_versions(target)} — needs the fix"]
        return True, [f"{label}: the fix removed {shown} and the target branch does not have it either — already fixed"]

    if not target:
        did = "added" if not old else "changed"
        return False, [f"{label}: the fix {did} {shown}, but the target branch has no copy of it — needs the fix"]

    old_lines, new_lines, target_lines = by_line(old), by_line(new), by_line(target)
    touched = {
        line
        for line in set(old_lines) | set(new_lines)
        if old_lines.get(line, Counter()) != new_lines.get(line, Counter())
    }

    problems: list[str] = []
    passes: list[str] = []

    for line in sorted(target_lines):
        if line not in touched:
            problems.append(
                f"{label}: the target branch has {shown} {show_versions(target_lines[line].elements())} on the "
                f"{describe_line(line)} line, which the fix did not change — cannot tell whether that copy is safe, "
                "so treating it as needing the fix"
            )

    for line in sorted(touched):
        old_here = old_lines.get(line, Counter())
        new_here = new_lines.get(line, Counter())
        target_here = list(target_lines.get(line, Counter()).elements())

        if not new_here:
            if target_here:
                problems.append(
                    f"{label}: the fix removed every {describe_line(line)} copy of {shown}, but the target branch "
                    f"still has {show_versions(target_here)} — needs the fix"
                )
            continue

        # What the fix moved this line TO: the versions it brought in, or,
        # if it only removed copies (leaving ones that were already there),
        # every version left. The target must be at or above all of them.
        brought_in = list((new_here - old_here).keys()) or list(new_here.keys())
        # What it moved FROM: the versions it took copies away from.
        taken_away = list((old_here - new_here).keys())

        # Each version taken away must be below the newest version brought
        # in. Otherwise the fix went down, or sideways, and "at or above"
        # does not describe it.
        not_upgrades = [v for v in taken_away if all(version_gte(v, x) for x in brought_in)]
        if not_upgrades:
            problems.append(
                f"{label}: the fix moved {shown} from {show_versions(not_upgrades)} to {show_versions(brought_in)}, "
                "which is not a plain upgrade — cannot tell, so treating it as needing the fix"
            )
            continue

        if not target_here:
            problems.append(
                f"{label}: the fix brought in {shown} {show_versions(brought_in)}, but the target branch has no "
                f"{describe_line(line)} copy of it — needs the fix"
            )
            continue

        behind = [v for v in target_here if not all(version_gte(v, x) for x in brought_in)]
        if behind:
            problems.append(
                f"{label}: the target branch has {shown} {show_versions(behind)}, not at or above the fixed "
                f"{show_versions(brought_in)} — needs the fix"
            )
        else:
            passes.append(
                f"{label}: every {describe_line(line)} copy of {shown} on the target branch "
                f"({show_versions(target_here)}) is at or above the fixed {show_versions(brought_in)} — already fixed"
            )

    if problems:
        return False, problems
    return True, passes


def compare_lockfile(label: str, old: Copies | None, new: Copies | None, target: Copies | None) -> tuple[bool, list[str]]:
    """Is every package whose copies the fix changed fixed on the target?"""
    if old is None or new is None:
        return False, [f"{label}: could not read the lockfile before or after the fix — cannot tell, so treating it as needing the fix"]

    changed = sorted(
        name for name in set(old) | set(new) if Counter(old.get(name, [])) != Counter(new.get(name, []))
    )
    if not changed:
        # Nothing recognisable changed. That is not the same as "the target
        # is fine": it means this check could not tell, most likely because
        # the fix took a shape it does not understand.
        return False, [
            f"{label}: no package's versions changed between the two commits, so this check cannot tell whether "
            "the target needs the fix — going ahead with the cherry-pick"
        ]

    if target is None:
        return False, [
            f"{label}: the target branch's lockfile is missing or could not be read — cannot confirm any of "
            f"{len(changed)} changed package(s) is fixed there"
        ]

    all_fixed = True
    details: list[str] = []
    for name in changed:
        fixed, lines = judge_package(label, name, old.get(name, []), new.get(name, []), target.get(name, []))
        all_fixed = all_fixed and fixed
        details.extend(lines)
    return all_fixed, details


def normalise_manifest_line(line: str) -> str:
    """Trim surrounding whitespace and one trailing comma."""
    text = line.strip()
    if text.endswith(","):
        text = text[:-1].rstrip()
    return text


def compare_manifest(path: str, added: list[str], removed: list[str], target: str | None) -> tuple[bool, list[str]]:
    """Is every line the fix added already on the target, and every line it
    removed gone from it?"""
    if not added and not removed:
        return False, [f"{path}: git reports a change but no changed lines (a binary or permissions-only change?) — cannot tell, so treating it as needing the fix"]
    if target is None:
        return False, [f"{path}: the target branch has no copy of this file that can be read — needs the fix"]

    target_lines = {normalise_manifest_line(line) for line in target.split("\n")}
    details: list[str] = []
    fixed = True
    for line in added:
        if normalise_manifest_line(line) not in target_lines:
            fixed = False
            details.append(f"{path}: the fix added the line '{line.strip()}', which the target branch does not have — needs the fix")
    for line in removed:
        if normalise_manifest_line(line) in target_lines:
            fixed = False
            details.append(f"{path}: the fix removed the line '{line.strip()}', which the target branch still has — needs the fix")
    if fixed:
        details.append(f"{path}: the target branch already has every line the fix added ({len(added)}) and none it removed ({len(removed)})")
    return fixed, details


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check whether a channel branch already has a lockfile security fix (see the module docstring)."
    )
    parser.add_argument("--repo-root", default=".", help="Path to the git checkout (default: current directory)")
    parser.add_argument("--old", required=True, help="Git ref for the commit BEFORE the fix (usually <sha>^)")
    parser.add_argument("--new", required=True, help="Git ref for the fix commit itself")
    parser.add_argument("--target", required=True, help="Git ref for the branch being checked, e.g. origin/release-candidate")
    parser.add_argument(
        "--manifest",
        action="append",
        default=[],
        metavar="PATH",
        help="A dependency manifest the fix may also change (repeatable). Its changed lines must already be on the target.",
    )
    parser.add_argument(
        "--lockfile-changes-only",
        action="store_true",
        help="Only ask whether the named lockfiles' changes are on the target; ignore every other file. Not for the forward-port shortcut.",
    )
    parser.add_argument("lockfiles", nargs="+", help="Repo-relative lockfile paths to check")
    args = parser.parse_args()
    if args.lockfile_changes_only and args.manifest:
        parser.error("--lockfile-changes-only ignores every file but the lockfiles, so --manifest cannot be used with it")

    repo_root = Path(args.repo_root)
    all_fixed = True
    any_lockfile_changed = False
    details: list[str] = []

    files = changed_files(repo_root, args.old, args.new)
    if files is None:
        print("RESULT=NEEDS_PORT")
        print("DETAIL: git could not list the files the fix commit changed — cannot tell, so treating it as needing the fix")
        return 1

    if not args.lockfile_changes_only:
        allowed = set(args.lockfiles) | set(args.manifest)
        others = [f for f in files if f not in allowed]
        if others:
            all_fixed = False
            details.append(
                f"the fix also changed {len(others)} file(s) that are not a lockfile or a named dependency manifest, "
                "and this check cannot see whether the target branch has those changes — needs the fix: " + ", ".join(others)
            )
        for path in args.manifest:
            if path not in files:
                continue
            changes = manifest_line_changes(repo_root, args.old, args.new, path)
            if changes is None:
                all_fixed = False
                details.append(f"{path}: git could not show what the fix changed — cannot tell, so treating it as needing the fix")
                continue
            fixed, lines = compare_manifest(path, changes[0], changes[1], git_show(repo_root, args.target, path))
            all_fixed = all_fixed and fixed
            details.extend(lines)

    for path in args.lockfiles:
        if path not in files:
            continue  # this fix didn't touch this lockfile
        any_lockfile_changed = True

        name = Path(path).name
        if name == "package-lock.json":
            parse = parse_npm_lock
        elif name == "Cargo.lock":
            parse = parse_cargo_lock
        else:
            all_fixed = False
            details.append(f"{path}: unrecognised lockfile type — cannot tell, so treating it as needing the fix")
            continue

        def read(ref: str):
            content = git_show(repo_root, ref, path)
            return None if content is None else parse(content)

        fixed, lines = compare_lockfile(path, read(args.old), read(args.new), read(args.target))
        all_fixed = all_fixed and fixed
        details.extend(lines)

    if not any_lockfile_changed:
        # The fix changed none of the named lockfiles, so there is no
        # dependency version to compare. Nothing to assert either way —
        # fall back to the normal cherry-pick rather than skip a real port.
        all_fixed = False
        details.append("none of the given lockfiles were changed by this commit — cannot tell, so treating it as needing the fix")

    print(f"RESULT={'ALREADY_FIXED' if all_fixed else 'NEEDS_PORT'}")
    for line in details:
        print(f"DETAIL: {line}")
    return 0 if all_fixed else 1


if __name__ == "__main__":
    sys.exit(main())
