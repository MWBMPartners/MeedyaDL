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

  2. The target says exactly what the fix says about every package the fix
     touched in a manifest, compared as data. Each named manifest is parsed
     (`package.json` as JSON, `Cargo.toml` with Python's own `tomllib`) and
     every place in it that refers to a package is listed:
       - `package.json`: `dependencies`, `devDependencies`,
         `optionalDependencies`, `peerDependencies` (an `npm:` alias counts
         for the package it installs), `resolutions` (every package a key
         names), and `overrides` at any depth (as a key, or as the parent of
         nested keys, or as a `$name` reference);
       - `Cargo.toml`: every dependency table (`dependencies`,
         `dev-dependencies`, `build-dependencies`, and the old spellings
         `dev_dependencies` and `build_dependencies`, at the top level and
         under `target.<platform>.`, and `workspace.dependencies`), where an
         entry counts for its key AND for the crate its `package = "..."`
         names; the `patch.<registry>` and `replace` entries for it; and
         every `[features]` value that names it (`foo/x`, `foo?/x`,
         `dep:foo`, or a plain `foo`).
     A package is touched when any place naming it was added, changed or
     removed by the fix. For each one, the list of places naming it in the
     fix's manifest and in the target's must be identical: the same places,
     every value exactly equal. So the target cannot re-enable through some
     other mention — another table, a platform table, a feature, its own
     entry when the fix changed the workspace one — what the fix took away.
     (Two reviews of #1275 found exactly those holes in the earlier rule,
     which compared only the places the fix itself changed.) An entry that
     takes settings from the workspace (`workspace = true`) also needs its
     `[workspace.dependencies]` entry to be in the same file on both sides;
     if that table is in another Cargo.toml (a workspace member) the
     inherited settings cannot be checked, which means "needs the fix".
     When the fix changes a Cargo.toml, the target's whole `[features]`
     table and its `edition` and `resolver` settings (under `[package]`,
     `[workspace]` and `[workspace.package]`) must also be identical to the
     fix's. Features can switch on other features, which switch on a
     dependency's features (`default = ["legacy"]`, `legacy =
     ["foo/risky"]`), and the edition and resolver decide how Cargo merges
     the features asked for by normal, test and build dependencies; this
     check does not try to work out either, so any difference is "needs the
     fix" (Codex's catch-up review of #1275; issue #1312).
     Anything else the fix changed in a manifest — a build setting, a
     script, a feature list, anything outside the dependency tables — means
     "needs the fix", and so does a manifest that cannot be read, parsed, or
     classified on any side.

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

HOW THE FILES ARE READ
----------------------
  - `package-lock.json` is real JSON, read with the standard `json` module.
    Its `packages` section is used; a file without one (lockfileVersion 1)
    is "cannot read". Entries marked `"link": true` are pointers to a local
    folder rather than installed copies, and are skipped.
  - `package.json` is read with `json`, and `Cargo.toml` with `tomllib`
    (Python 3.11 or later, which GitHub's runners have). On an older
    Python there is no `tomllib`, so a changed `Cargo.toml` is "cannot
    tell". Cargo's old underscore spellings (`dev_dependencies`,
    `build_dependencies`) are read as the dependency tables they are.
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
import copy
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

try:  # Python 3.11 or later
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - only on an old Python
    tomllib = None

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


# A dependency entry that is not there at all (distinct from any real value).
MISSING = object()

NPM_DEPENDENCY_SECTIONS = (
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
    "overrides",
    "resolutions",
)
# The old underscore spellings are included: Cargo still accepts them (with a
# warning) on edition 2021, so a channel could use one to switch a feature
# back on. Left out, they were read as "not a dependency table" and missed
# (issue #1312).
CARGO_DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies", "dev_dependencies", "build_dependencies")


class ManifestError(Exception):
    """A manifest that cannot be read the way this script needs."""


def parse_manifest(path: str, content: str):
    name = Path(path).name
    if name == "package.json":
        doc = json.loads(content)
    elif name == "Cargo.toml":
        if tomllib is None:
            raise ManifestError("this Python has no tomllib (it needs Python 3.11 or later)")
        doc = tomllib.loads(content)
    else:
        raise ManifestError("not a manifest type this check understands")
    if not isinstance(doc, dict):
        raise ManifestError("the top level is not an object/table")
    return doc


def split_manifest(path: str, doc: dict) -> tuple[dict[tuple, object], dict]:
    """(every dependency entry keyed by where it sits, everything else).

    The position is the table path plus the dependency's name, e.g.
    ("dependencies", "foo"), ("target", "cfg(windows)", "dependencies",
    "foo"), ("patch", "crates-io", "foo") or ("overrides", "brace-expansion").
    A dependency table that is not a table at all is a ManifestError."""
    entries: dict[tuple, object] = {}
    rest = copy.deepcopy(doc)

    def take(container: dict, key: str, prefix: tuple) -> None:
        table = container.pop(key)
        if not isinstance(table, dict):
            raise ManifestError(f"'{' > '.join(prefix)}' is not a table")
        for dep, value in table.items():
            entries[prefix + (dep,)] = value

    def table_at(container: dict, key: str) -> dict | None:
        value = container.get(key)
        if value is not None and not isinstance(value, dict):
            raise ManifestError(f"'{key}' is not a table")
        return value

    if Path(path).name == "package.json":
        for section in NPM_DEPENDENCY_SECTIONS:
            if section in rest:
                take(rest, section, (section,))
        return entries, rest

    for table in CARGO_DEPENDENCY_TABLES:
        if table in rest:
            take(rest, table, (table,))
    workspace = table_at(rest, "workspace")
    if workspace is not None and "dependencies" in workspace:
        take(workspace, "dependencies", ("workspace", "dependencies"))
    targets = table_at(rest, "target")
    for platform, platform_tables in (targets or {}).items():
        if not isinstance(platform_tables, dict):
            raise ManifestError(f"'target > {platform}' is not a table")
        for table in CARGO_DEPENDENCY_TABLES:
            if table in platform_tables:
                take(platform_tables, table, ("target", platform, table))
    patches = table_at(rest, "patch")
    if patches is not None:
        rest.pop("patch")
        for registry in list(patches):
            take(patches, registry, ("patch", registry))
    if table_at(rest, "replace") is not None:
        take(rest, "replace", ("replace",))
    return entries, rest


def inherits_from_workspace(entry) -> bool:
    """True if a Cargo dependency entry takes settings from the workspace
    (`foo = { workspace = true, ... }`). Any `workspace` value other than
    `false` counts, so an odd value errs toward checking more, not less."""
    return isinstance(entry, dict) and "workspace" in entry and entry["workspace"] is not False


def npm_spec_name(spec: str) -> str:
    """The package a key such as `foo`, `foo@^1` or `@scope/foo@1` names."""
    if spec.startswith("@"):
        scope, slash, rest = spec[1:].partition("/")
        name = rest.split("@", 1)[0]
        if not slash or not scope or not name:
            raise ManifestError(f"cannot tell which package {spec!r} names")
        return f"@{scope}/{name}"
    name = spec.split("@", 1)[0]
    if not name:
        raise ManifestError(f"cannot tell which package {spec!r} names")
    return name


def npm_resolution_names(key: str) -> set[str]:
    """Every package a yarn-style `resolutions` key names (`foo`,
    `bar/foo`, `**/foo`, `@scope/bar/@scope/foo@1`)."""
    parts = key.split("/")
    names: set[str] = set()
    i = 0
    while i < len(parts):
        part = parts[i]
        if part.startswith("@") and i + 1 < len(parts):
            part = part + "/" + parts[i + 1]
            i += 1
        if part not in ("*", "**", ""):
            names.add(npm_spec_name(part))
        i += 1
    if not names:
        raise ManifestError(f"cannot tell which package the resolution {key!r} names")
    return names


def cargo_replace_name(spec: str) -> str:
    """The crate a `[replace]` key (`foo:1.0.0`, `foo@1.0.0` or
    `<registry>#foo@1.0.0`) names."""
    tail = spec.split("#", 1)[-1]
    name = re.split(r"[:@]", tail, 1)[0]
    if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
        raise ManifestError(f"cannot tell which crate the [replace] key {spec!r} names")
    return name


# Every place a manifest refers to a package: {place: (value, names)}, where
# `place` is the table path plus key and `names` every package it refers to.
Mentions = dict[tuple, tuple[object, set[str]]]


def manifest_mentions(path: str, doc: dict) -> Mentions:
    """Every place in a parsed manifest that refers to a package — not just
    the dependency tables, but anything else that can switch a package or
    one of its features on (see compare_manifest). Anything that cannot be
    classified is a ManifestError, which the caller reads as "needs the fix"."""
    out: Mentions = {}

    def table_at(container: dict, key: str, label: str) -> dict:
        value = container.get(key, {})
        if not isinstance(value, dict):
            raise ManifestError(f"'{label}' is not a table")
        return value

    if Path(path).name == "package.json":
        for section in ("dependencies", "devDependencies", "optionalDependencies", "peerDependencies"):
            for key, value in table_at(doc, section, section).items():
                names = {npm_spec_name(key)}
                # "bar": "npm:foo@1" installs foo under the name bar.
                if isinstance(value, str) and value.startswith("npm:"):
                    names.add(npm_spec_name(value[4:]))
                out[(section, key)] = (value, names)
        for key, value in table_at(doc, "resolutions", "resolutions").items():
            out[("resolutions", key)] = (value, npm_resolution_names(key))

        def walk(prefix: tuple, table: dict, parent: str) -> None:
            for key, value in table.items():
                # "." sets the version of the package the table belongs to.
                name = parent if key == "." else npm_spec_name(key)
                names = {name}
                # "$foo" means "the version of foo in my dependencies".
                if isinstance(value, str) and value.startswith("$"):
                    names.add(npm_spec_name(value[1:]))
                out[prefix + (key,)] = (value, names)
                if isinstance(value, dict):
                    walk(prefix + (key,), value, name)

        walk(("overrides",), table_at(doc, "overrides", "overrides"), "")
        return out

    def entry_names(key: str, entry) -> set[str]:
        names = {key}
        if isinstance(entry, dict) and "package" in entry:
            if not isinstance(entry["package"], str) or not entry["package"]:
                raise ManifestError(f"the 'package' of '{key}' is not a crate name")
            names.add(entry["package"])
        return names

    tables: list[tuple[tuple, dict]] = []
    for table in CARGO_DEPENDENCY_TABLES:
        tables.append(((table,), table_at(doc, table, table)))
    workspace_deps = table_at(table_at(doc, "workspace", "workspace"), "dependencies", "workspace > dependencies")
    tables.append((("workspace", "dependencies"), workspace_deps))
    for platform, platform_tables in table_at(doc, "target", "target").items():
        if not isinstance(platform_tables, dict):
            raise ManifestError(f"'target > {platform}' is not a table")
        for table in CARGO_DEPENDENCY_TABLES:
            tables.append((("target", platform, table), table_at(platform_tables, table, f"target > {platform} > {table}")))
    for registry, patches in table_at(doc, "patch", "patch").items():
        if not isinstance(patches, dict):
            raise ManifestError(f"'patch > {registry}' is not a table")
        tables.append((("patch", registry), patches))

    # Which crate(s) each dependency key stands for, for reading [features].
    by_key: dict[str, set[str]] = {}
    for prefix, table in tables:
        for key, entry in table.items():
            names = entry_names(key, entry)
            # An inheriting entry is whatever its workspace entry says it is.
            if inherits_from_workspace(entry) and key in workspace_deps:
                names |= entry_names(key, workspace_deps[key])
            out[prefix + (key,)] = (entry, names)
            by_key.setdefault(key, set()).update(names)
    for spec, entry in table_at(doc, "replace", "replace").items():
        out[("replace", spec)] = (entry, {cargo_replace_name(spec)} | entry_names(cargo_replace_name(spec), entry))
    for feature, values in table_at(doc, "features", "features").items():
        if not isinstance(values, list) or not all(isinstance(v, str) for v in values):
            raise ManifestError(f"feature '{feature}' is not a list of names")
        for value in values:
            # "dep:foo", "foo/bar", "foo?/bar", or a plain "foo" (which can
            # switch on an optional dependency called foo).
            key = value[4:] if value.startswith("dep:") else value.split("/", 1)[0].rstrip("?")
            out[("features", feature, value)] = (value, {key} | by_key.get(key, set()))
    return out


def show_value(value) -> str:
    text = json.dumps(value, sort_keys=True, default=str)
    return text if len(text) <= 120 else text[:117] + "..."


def compare_manifest(path: str, old: str | None, new: str | None, target: str | None) -> tuple[bool, list[str]]:
    """Does the target say exactly what the fix's manifest says about every
    package the fix touched — and did the fix change nothing else in it?"""
    sides: list[tuple[dict, dict, Mentions]] = []
    for side, text in (("before the fix", old), ("after the fix", new), ("on the target branch", target)):
        if text is None:
            return False, [f"{path}: the file could not be read {side} — cannot tell, so treating it as needing the fix"]
        try:
            doc = parse_manifest(path, text)
            sides.append((doc, split_manifest(path, doc)[1], manifest_mentions(path, doc)))
        except (ValueError, ManifestError) as error:  # json and tomllib errors are ValueErrors
            return False, [f"{path}: the file {side} could not be parsed or understood ({error}) — cannot tell, so treating it as needing the fix"]
    (_, old_rest, old_mentions), (new_doc, new_rest, new_mentions), (target_doc, _, target_mentions) = sides

    fixed = True
    details: list[str] = []
    if old_rest != new_rest:
        fixed = False
        details.append(
            f"{path}: the fix changed something other than dependency entries (a build setting, a script, a "
            "feature list...), which this check cannot compare — needs the fix"
        )

    # Which packages did the fix touch? Every package named by any place the
    # fix added, changed or removed.
    changed = {
        place
        for place in set(old_mentions) | set(new_mentions)
        if old_mentions.get(place, (MISSING,))[0] != new_mentions.get(place, (MISSING,))[0]
    }
    touched: set[str] = set()
    for place in changed:
        for mentions in (old_mentions, new_mentions):
            if place in mentions:
                touched |= mentions[place][1]

    def about(mentions: Mentions, package: str) -> dict:
        """Everything this manifest says about one package, place by place."""
        return {place: value for place, (value, names) in mentions.items() if package in names}

    for package in sorted(touched):
        shown = f'"{package}"' if package else "the project itself"
        want, have = about(new_mentions, package), about(target_mentions, package)
        if want == have:
            details.append(f"{path}: every place that mentions {shown} ({len(want)}) is the same on the target branch")
            continue
        fixed = False
        for place in sorted(set(want) | set(have)):
            where = " > ".join(place)
            if place not in have:
                details.append(f"{path}: the fix has {where} = {show_value(want[place])}, which the target branch does not have — needs the fix")
            elif place not in want:
                details.append(f"{path}: the target branch also has {where} = {show_value(have[place])}, which the fix does not — needs the fix")
            elif want[place] != have[place]:
                details.append(f"{path}: {where} is {show_value(want[place])} after the fix but {show_value(have[place])} on the target branch — needs the fix")

    # An entry that inherits from [workspace.dependencies] is only checked if
    # that table is in THIS file on both sides. When it lives in another
    # Cargo.toml (a workspace member), both sides can look identical here
    # while the settings that matter differ, so that is "needs the fix".
    # (The comparison above already includes the workspace entry itself
    # whenever it is in this file, which is all the rest of the rule from
    # Codex's fourth review needed.)
    missing_inherited: set[tuple] = set()
    for mentions in (old_mentions, new_mentions, target_mentions):
        for place, (value, names) in mentions.items():
            dependency_place = place[0] in CARGO_DEPENDENCY_TABLES or place[0] == "target"
            if dependency_place and names & touched and inherits_from_workspace(value):
                inherited = ("workspace", "dependencies", place[-1])
                if inherited not in new_mentions or inherited not in target_mentions:
                    missing_inherited.add((place, inherited))
    for place, inherited in sorted(missing_inherited):
        fixed = False
        details.append(
            f"{path}: {' > '.join(place)} takes settings from {' > '.join(inherited)}, which is not in this file on "
            "both sides (it may live in another Cargo.toml) — the inherited settings could not be checked, so "
            "treating it as needing the fix"
        )

    if Path(path).name == "Cargo.toml":
        for what, want, have in cargo_whole_settings(new_doc, target_doc):
            if want != have:
                fixed = False
                details.append(
                    f"{path}: {what} differs: {show_value(want)} after the fix, {show_value(have)} on the target "
                    "branch — needs the fix"
                )

    if fixed and not changed:
        details.append(f"{path}: the fix changed only the file's layout, not what it says")
    return fixed, details


def cargo_whole_settings(new_doc: dict, target_doc: dict) -> list[tuple[str, object, object]]:
    """Settings of a Cargo.toml that must be identical on the target as a
    whole, not package by package: (what, after the fix, on the target).

    Why the whole [features] table: a feature can switch on another feature,
    which switches on a dependency's feature (`default = ["legacy"]`,
    `legacy = ["foo/risky"]`). Following that chain is Cargo's job, not this
    check's; listing the mentions of `foo` missed a channel whose `default`
    reached `foo/risky` through `legacy` (Codex's catch-up review of #1275).
    So any difference in [features] at all means "needs the fix". It is
    strict on purpose: an unrelated extra feature also counts.
    Why `edition` and `resolver`: they decide how Cargo merges the features
    asked for by normal, dev and build dependencies. With the old resolver
    ("1", the default before edition 2021) a feature the fix kept only for
    tests is still built into the shipped crate (issue #1312). They are read
    from [package] and [workspace] (and the edition from [workspace.package],
    which members can inherit). A missing value counts as a value, so
    "missing on one side only" is a difference."""

    def dig(doc: dict, *keys: str):
        value = doc
        for key in keys:
            if not isinstance(value, dict) or key not in value:
                return MISSING_SETTING
            value = value[key]
        return value

    return [
        (what, dig(new_doc, *keys), dig(target_doc, *keys))
        for what, keys in (
            ("[features]", ("features",)),
            ("[package] edition", ("package", "edition")),
            ("[package] resolver", ("package", "resolver")),
            ("[workspace] resolver", ("workspace", "resolver")),
            ("[workspace.package] edition", ("workspace", "package", "edition")),
        )
    ]


class _NotSet:
    """A setting that is not there: equal only to itself (a real value from
    a parsed file can never equal it), and shown as "(not set)"."""

    def __repr__(self) -> str:
        return "(not set)"

    __str__ = __repr__


MISSING_SETTING = _NotSet()


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
        help="A dependency manifest (package.json or Cargo.toml) the fix may also change (repeatable). Every dependency entry it changed must match on the target.",
    )
    parser.add_argument("lockfiles", nargs="+", help="Repo-relative lockfile paths to check")
    args = parser.parse_args()

    repo_root = Path(args.repo_root)
    all_fixed = True
    any_lockfile_changed = False
    details: list[str] = []

    files = changed_files(repo_root, args.old, args.new)
    if files is None:
        print("RESULT=NEEDS_PORT")
        print("DETAIL: git could not list the files the fix commit changed — cannot tell, so treating it as needing the fix")
        return 1

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
        fixed, lines = compare_manifest(
            path,
            git_show(repo_root, args.old, path),
            git_show(repo_root, args.new, path),
            git_show(repo_root, args.target, path),
        )
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
