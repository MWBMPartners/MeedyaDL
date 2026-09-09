#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
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

The fix is to stop asking git a textual question ("does this patch apply
cleanly?") and instead ask the real question directly: for every dependency
whose version the fix commit changed, does the target branch's lockfile
already have that version or newer? That is a semver comparison, not a
diff, and it gives the right answer regardless of how the fix travelled —
a clean Dependabot PR, a combined PR, a hand-typed commit, or a completely
different PR that happened to bump the same package for an unrelated
reason.

WHAT THIS DOES
--------------
Given the commit that introduced a fix (`--old`/`--new`, normally
`<sha>^`/`<sha>`) and the branch to check (`--target`, e.g.
`origin/release-candidate`), for each lockfile path given on the command
line:

  1. Skip it if the fix commit didn't touch it at all.
  2. Work out which dependencies changed version in the fix commit, and
     what the fixed (new) version is for each one.
  3. Look up those same dependencies in the target branch's copy of the
     lockfile. If the target's version is already the fixed version or
     newer, that dependency is "already fixed" there. If it's older, or
     the target doesn't have it in a comparable shape at all, we can't
     say the branch is safe.

If EVERY changed dependency, in EVERY lockfile passed in, is already fixed
on the target branch, this prints "RESULT=ALREADY_FIXED" and exits 0 — the
caller should do nothing further (no PR, no issue). Otherwise it prints
"RESULT=NEEDS_PORT" and exits 1, and the caller should fall back to the
existing cherry-pick flow exactly as before. Anything this script cannot
confidently read as "already fixed" (a missing package, a file that
didn't parse, no version change found at all) deliberately falls on the
NEEDS_PORT side — the worst that costs is one more cherry-pick attempt on
a branch that turns out to be fine (caught anyway by the existing
post-cherry-pick "does this produce an actual diff?" check); the worst a
wrong ALREADY_FIXED would cost is a security fix silently never reaching a
branch that needed it, which is the exact failure this script exists to
close.

Understands two lockfile shapes:
  - `package-lock.json` (npm, lockfileVersion 2/3) — real JSON, parsed with
    the standard `json` module. Dependencies are compared by their full
    `packages` key (e.g. "node_modules/eslint/node_modules/js-yaml"), not
    just the bare package name, because npm can legitimately have two
    different versions of the same package installed at different nested
    paths — collapsing to the bare name would blur a fixed copy and an
    unfixed copy together.
  - `Cargo.lock` (cargo) — plain text, parsed with a small targeted regex
    rather than a TOML library, matching the house style already used by
    `tools/audit-checks/check_codec_registry.py` for `codecs.toml`: no
    third-party dependency, and no reliance on `tomllib`, which only
    exists on Python 3.11+. Every `[[package]]` block in every Cargo.lock
    this project has ever produced puts `version = "..."` on the line
    immediately after `name = "..."` (verified against the current
    826-package lockfile with zero exceptions), so that adjacency is used
    directly instead of writing a general TOML parser for one file shape.

Only the two lockfiles this repo actually has are relevant, but the
per-file logic is dispatched by filename so a third lockfile type could be
added later without restructuring the comparison itself.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

# A `[[package]]` header in Cargo.lock.
CARGO_PACKAGE_HEADER_RE = re.compile(r"^\[\[package\]\]\s*$")
CARGO_NAME_RE = re.compile(r'^name = "(.+)"$')
CARGO_VERSION_RE = re.compile(r'^version = "(.+)"$')


def git_show(repo_root: Path, ref: str, path: str) -> str | None:
    """Return the file's content at `ref`, or None if it doesn't exist
    there (a genuinely missing file, not a script bug — `git show` exits
    non-zero for a path that isn't in that tree, which is a normal,
    expected outcome here, e.g. a lockfile that didn't exist yet on an
    old commit)."""
    result = subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def path_changed(repo_root: Path, old_ref: str, new_ref: str, path: str) -> bool:
    """True if `path` differs between the two refs at all. Used to skip a
    lockfile the fix commit never touched, so an unrelated lockfile can
    never accidentally gate on it."""
    result = subprocess.run(
        ["git", "diff", "--name-only", f"{old_ref}", f"{new_ref}", "--", path],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode == 0 and result.stdout.strip() != ""


def parse_version(v: str) -> tuple[tuple[int, ...], str]:
    """Split a version string into its leading numeric release segments
    and whatever text follows (pre-release/build metadata), e.g.
    "3.1.7" -> ((3, 1, 7), "") and "1.2.3-beta.1" -> ((1, 2, 3), "-beta.1").
    A version with no numeric prefix at all sorts as (0,) with the whole
    original string kept as the remainder, so it never falsely compares
    as newer than an ordinary release — this is a pragmatic approximation
    of semver ordering, not a full implementation, and is only expected to
    handle the kind of straightforward patch/minor bump a security fix
    normally is."""
    m = re.match(r"^(\d+(?:\.\d+)*)(.*)$", v.strip())
    if not m:
        return (0,), v
    nums = tuple(int(p) for p in m.group(1).split("."))
    return nums, m.group(2)


def version_gte(a: str, b: str) -> bool:
    """True if version `a` is the same release as, or a newer release
    than, version `b`. Numeric release segments decide it in almost every
    real case; when they're exactly equal, a plain release (no suffix)
    outranks a pre-release/build-tagged version of the same numbers,
    matching ordinary semver precedence."""
    (na, ra), (nb, rb) = parse_version(a), parse_version(b)
    length = max(len(na), len(nb))
    na = na + (0,) * (length - len(na))
    nb = nb + (0,) * (length - len(nb))
    if na != nb:
        return na > nb
    if ra == rb or ra == "":
        return True
    if rb == "":
        return False
    return ra >= rb


def parse_npm_lock(content: str) -> dict[str, str] | None:
    """{packages key: version} for a package-lock.json, or None if it
    doesn't parse as JSON at all (a real tool failure, not "no packages")."""
    try:
        data = json.loads(content)
    except json.JSONDecodeError:
        return None
    packages = data.get("packages")
    if not isinstance(packages, dict):
        return {}
    out: dict[str, str] = {}
    for key, meta in packages.items():
        if isinstance(meta, dict) and isinstance(meta.get("version"), str):
            out[key] = meta["version"]
    return out


def parse_cargo_lock(content: str) -> dict[str, set[str]]:
    """{crate name: set of versions present} for a Cargo.lock. A set
    because cargo can legitimately resolve two different versions of the
    same crate into one lockfile."""
    out: dict[str, set[str]] = {}
    lines = content.splitlines()
    i = 0
    while i < len(lines):
        m = CARGO_NAME_RE.match(lines[i].strip())
        if m and i + 1 < len(lines):
            vm = CARGO_VERSION_RE.match(lines[i + 1].strip())
            if vm:
                out.setdefault(m.group(1), set()).add(vm.group(1))
                i += 2
                continue
        i += 1
    return out


def check_npm_lockfile(old: str, new: str, target: str | None) -> tuple[bool, list[str]]:
    """Returns (all_changed_entries_already_fixed, detail_lines)."""
    old_map = parse_npm_lock(old)
    new_map = parse_npm_lock(new)
    if old_map is None or new_map is None:
        return False, ["could not parse package-lock.json as JSON at old or new commit"]

    changed = {
        key: new_version
        for key, new_version in new_map.items()
        if key in old_map and old_map[key] != new_version
    }

    # A fix can also work by REMOVING a dependency outright rather than
    # raising its version. Those entries are in the old file and not in the
    # new one, so the comparison above never sees them. Left unhandled, a
    # removal-only fix looks like "nothing changed", which then reads as
    # "the target already has it" — and the fix is skipped while the target
    # still carries the very thing that was removed.
    removed = sorted(set(old_map) - set(new_map))

    if not changed and not removed:
        # Nothing recognisable changed. That is not the same as "the target
        # is fine": it means this check could not tell, most likely because
        # the fix took a shape it does not understand. Say so, and let the
        # cherry-pick go ahead. A cherry-pick that turns out to be redundant
        # costs a conflict somebody has to look at. A fix skipped by mistake
        # costs a branch left vulnerable, which is the whole reason this
        # file exists.
        return False, [
            "package-lock.json: no dependency version changed between the two commits, "
            "so this check cannot tell whether the target needs the fix — going ahead with the cherry-pick"
        ]

    target_map = parse_npm_lock(target) if target is not None else None
    details: list[str] = []
    all_fixed = True
    for key, fixed_version in sorted(changed.items()):
        if target_map is None:
            all_fixed = False
            details.append(f"package-lock.json: target branch's lockfile could not be read — cannot confirm \"{key}\" is fixed")
            continue
        target_version = target_map.get(key)
        if target_version is None:
            all_fixed = False
            details.append(f"package-lock.json: \"{key}\" not present in target lockfile — cannot confirm it is fixed")
        elif version_gte(target_version, fixed_version):
            details.append(f"package-lock.json: \"{key}\" is {target_version} on the target branch (fix bumped it to {fixed_version}) — already fixed")
        else:
            all_fixed = False
            details.append(f"package-lock.json: \"{key}\" is {target_version} on the target branch, behind the fixed {fixed_version} — needs the fix")

    # Anything the fix removed has to be gone from the target too, or the
    # target still has the thing that was taken out for a reason.
    for key in removed:
        if target_map is None:
            all_fixed = False
            details.append(f"package-lock.json: target branch's lockfile could not be read — cannot confirm \"{key}\" was removed there too")
        elif key in target_map:
            all_fixed = False
            details.append(f"package-lock.json: the fix removed \"{key}\", but the target branch still has it — needs the fix")
        else:
            details.append(f"package-lock.json: the fix removed \"{key}\" and the target branch does not have it either — already fixed")
    return all_fixed, details


def check_cargo_lockfile(old: str, new: str, target: str | None) -> tuple[bool, list[str]]:
    old_map = parse_cargo_lock(old)
    new_map = parse_cargo_lock(new)

    # A crate can legitimately appear more than once in Cargo.lock at
    # different versions, because two dependencies can each want their own.
    # So a fix is described by BOTH halves: which versions it took away and
    # which it brought in. Looking only at what arrived is what lets a
    # branch that still carries the vulnerable copy look fixed, simply
    # because some other, newer copy happens to be sitting beside it.
    changed: dict[str, tuple[set[str], set[str]]] = {}
    for name in set(old_map) | set(new_map):
        old_versions = old_map.get(name, set())
        new_versions = new_map.get(name, set())
        added = new_versions - old_versions
        dropped = old_versions - new_versions
        if added or dropped:
            changed[name] = (added, dropped)

    if not changed:
        # Nothing recognisable changed, which is not the same as the target
        # being fine — see the matching note on the npm side. Go ahead with
        # the cherry-pick rather than guess.
        return False, [
            "Cargo.lock: no crate version changed between the two commits, "
            "so this check cannot tell whether the target needs the fix — going ahead with the cherry-pick"
        ]

    target_map = parse_cargo_lock(target) if target is not None else {}
    details: list[str] = []
    all_fixed = True
    for name, (added, dropped) in sorted(changed.items()):
        target_versions = target_map.get(name, set())

        # The versions the fix removed must be gone from the target as well.
        # This is the half that matters: it is what proves the vulnerable
        # copy is actually gone, rather than merely outnumbered.
        still_there = sorted(dropped & target_versions)
        if still_there:
            all_fixed = False
            details.append(
                f"Cargo.lock: the fix replaced \"{name}\" {', '.join(still_there)}, "
                f"but the target branch still has {'that version' if len(still_there) == 1 else 'those versions'} — needs the fix"
            )
            continue

        if not added:
            # A pure removal, and the target does not have the removed
            # versions either. Nothing further to look for.
            details.append(f"Cargo.lock: the fix removed \"{name}\" {', '.join(sorted(dropped))} and the target branch does not have {'it' if len(dropped) == 1 else 'them'} either — already fixed")
            continue

        if not target_versions:
            all_fixed = False
            details.append(f"Cargo.lock: \"{name}\" not present in target lockfile — cannot confirm it is fixed")
            continue

        fixed_version = max(added, key=lambda v: parse_version(v)[0])
        if any(version_gte(v, fixed_version) for v in target_versions):
            best = max(target_versions, key=lambda v: parse_version(v)[0])
            details.append(f"Cargo.lock: \"{name}\" is {best} on the target branch (fix brought in {fixed_version}, and the old version it replaced is gone) — already fixed")
        else:
            all_fixed = False
            have = ", ".join(sorted(target_versions))
            details.append(f"Cargo.lock: \"{name}\" is {have} on the target branch, behind the fixed {fixed_version} — needs the fix")
    return all_fixed, details


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", default=".", help="Path to the git checkout (default: current directory)")
    parser.add_argument("--old", required=True, help="Git ref for the commit BEFORE the fix (usually <sha>^)")
    parser.add_argument("--new", required=True, help="Git ref for the fix commit itself")
    parser.add_argument("--target", required=True, help="Git ref for the branch being checked, e.g. origin/release-candidate")
    parser.add_argument("lockfiles", nargs="+", help="Repo-relative lockfile paths to check")
    args = parser.parse_args()

    repo_root = Path(args.repo_root)
    any_relevant = False
    all_fixed = True
    all_details: list[str] = []

    for path in args.lockfiles:
        if not path_changed(repo_root, args.old, args.new, path):
            continue  # this fix didn't touch this lockfile — irrelevant

        old_content = git_show(repo_root, args.old, path)
        new_content = git_show(repo_root, args.new, path)
        if old_content is None or new_content is None:
            # The fix commit's diff touched this path but one side is
            # unreadable (e.g. the file was newly added, so there is no
            # "old" version to diff against). There's no version bump to
            # compare, so this can't be asserted as fixed — fall through
            # to the existing cherry-pick flow for safety.
            any_relevant = True
            all_fixed = False
            all_details.append(f"{path}: could not read both the before and after commit — cannot confirm the target branch is fixed")
            continue

        target_content = git_show(repo_root, args.target, path)

        if Path(path).name == "package-lock.json":
            fixed, details = check_npm_lockfile(old_content, new_content, target_content)
        elif Path(path).name == "Cargo.lock":
            fixed, details = check_cargo_lockfile(old_content, new_content, target_content)
        else:
            all_fixed = False
            all_details.append(f"{path}: unrecognised lockfile type — cannot confirm the target branch is fixed")
            any_relevant = True
            continue

        any_relevant = True
        all_fixed = all_fixed and fixed
        all_details.extend(details)

    if not any_relevant:
        # The fix commit didn't touch any lockfile this script was asked
        # about at all. Nothing to assert either way — behave exactly as
        # if we'd found something unresolved, so the caller falls back to
        # its normal cherry-pick attempt rather than skipping a real port.
        print("RESULT=NEEDS_PORT")
        print("DETAIL: none of the given lockfiles were changed by this commit")
        return 1

    print(f"RESULT={'ALREADY_FIXED' if all_fixed else 'NEEDS_PORT'}")
    for line in all_details:
        print(f"DETAIL: {line}")
    return 0 if all_fixed else 1


if __name__ == "__main__":
    sys.exit(main())
