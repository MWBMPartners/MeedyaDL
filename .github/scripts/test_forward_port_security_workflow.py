#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
.github/scripts/test_forward_port_security_workflow.py

Tests for `.github/workflows/forward-port-security.yml`, run against the
workflow's REAL shell scripts.

WHAT IS TESTED
--------------
  - The `gate` job's "decide" step, which reads a merged pull request from
    GitHub and decides whether it is a security fix to copy onto the
    `alpha`, `beta` and `release-candidate` branches.
  - The `forward-port` job's cherry-pick step, to show that its "already
    has it" shortcut never skips a change it did not check.

HOW
---
Each test reads the workflow file, pulls the step's `run:` script out of it
at test time (never a copy kept here, so the test cannot drift from the
workflow), and runs that script with bash, the way GitHub runs it. A fake
`gh` program placed first on PATH answers with made-up pull request data
and records every call, so nothing reaches GitHub. Git operations run
against a throwaway local repository standing in for GitHub's copy, which
includes `refs/pull/<n>/head` the way GitHub serves it. The real
`check_lockfile_already_fixed.py` is linked into that repository where the
workflow expects to find it.

Needs only the Python standard library, `git`, `jq` and bash 4.4 or later
(GitHub's Ubuntu runners have all of them).

Run:  python3 .github/scripts/test_forward_port_security_workflow.py

To watch these tests fail against older copies, point the two variables
at them:
    FORWARD_PORT_WORKFLOW=/path/to/old/forward-port-security.yml \
    CHECK_LOCKFILE_HELPER=/path/to/old/check_lockfile_already_fixed.py \
        python3 .github/scripts/test_forward_port_security_workflow.py
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKFLOW = Path(
    os.environ.get("FORWARD_PORT_WORKFLOW") or HERE.parent / "workflows" / "forward-port-security.yml"
).resolve()
HELPER = Path(os.environ.get("CHECK_LOCKFILE_HELPER") or HERE / "check_lockfile_already_fixed.py").resolve()

REPO = "example/meedyadl"


def isolated_git_env() -> dict[str, str]:
    """`git` that ignores the machine's own settings (signing, hooks...)."""
    env = dict(os.environ)
    env.update(
        {
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "test",
            "GIT_AUTHOR_EMAIL": "test@example.invalid",
            "GIT_COMMITTER_NAME": "test",
            "GIT_COMMITTER_EMAIL": "test@example.invalid",
        }
    )
    return env


GIT_ENV = isolated_git_env()


# ---------------------------------------------------------------------------
# Pulling a step's script out of the workflow file
# ---------------------------------------------------------------------------


def indent_of(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


class Step:
    def __init__(self, run: str, env_names: set[str]) -> None:
        self.run = run
        self.env_names = env_names


def extract_step(workflow_text: str, job: str, *, step_id: str | None = None, name_prefix: str | None = None) -> Step:
    """The `run:` script and `env:` names of one step, read straight from
    the workflow file. Uses only the indentation rules YAML block scalars
    follow, which is all a GitHub workflow's steps need; it fails loudly
    rather than guess if the file does not have the expected shape."""
    lines = workflow_text.split("\n")
    jobs_at = lines.index("jobs:")
    job_at = next(i for i in range(jobs_at + 1, len(lines)) if re.fullmatch(rf"  {re.escape(job)}:\s*", lines[i]))
    job_end = next(
        (
            i
            for i in range(job_at + 1, len(lines))
            if lines[i].strip() and not lines[i].lstrip().startswith("#") and indent_of(lines[i]) <= 2
        ),
        len(lines),
    )
    steps_at = next(i for i in range(job_at + 1, job_end) if lines[i].strip() == "steps:")
    first_dash = next(i for i in range(steps_at + 1, job_end) if lines[i].lstrip().startswith("- "))
    dash = indent_of(lines[first_dash])
    starts = [i for i in range(first_dash, job_end) if indent_of(lines[i]) == dash and lines[i].lstrip().startswith("- ")]

    matches: list[Step] = []
    for n, start in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else job_end
        segment = [lines[start][:dash] + "  " + lines[start][dash + 2 :]] + lines[start + 1 : end]
        key_indent = dash + 2
        keys: dict[str, str] = {}
        env_names: set[str] = set()
        run: str | None = None
        i = 0
        while i < len(segment):
            line = segment[i]
            if indent_of(line) != key_indent or not line.strip() or line.strip().startswith("#"):
                i += 1
                continue
            key, _, value = line.strip().partition(":")
            value = value.strip()
            keys[key] = value
            block_end = i + 1
            while block_end < len(segment) and (not segment[block_end].strip() or indent_of(segment[block_end]) > key_indent):
                block_end += 1
            block = segment[i + 1 : block_end]
            if key == "env":
                for entry in block:
                    if entry.strip() and not entry.strip().startswith("#") and indent_of(entry) == key_indent + 2:
                        env_names.add(entry.strip().split(":", 1)[0])
            if key == "run" and value == "|":
                while block and not block[-1].strip():
                    block.pop()
                block_indent = min(indent_of(b) for b in block if b.strip())
                run = "\n".join(b[block_indent:] if b.strip() else "" for b in block) + "\n"
            i = block_end
        selected = (step_id is not None and keys.get("id") == step_id) or (
            name_prefix is not None and keys.get("name", "").startswith(name_prefix)
        )
        if selected:
            if run is None:
                # Only a `run: |` block is understood; anything else is
                # refused rather than half-read.
                raise AssertionError(f"step at line {start + 1} has no 'run: |' script (run: {keys.get('run')!r})")
            matches.append(Step(run, env_names))
    if len(matches) != 1:
        raise AssertionError(f"expected exactly one matching step in job '{job}', found {len(matches)}")
    if "${{" in matches[0].run:
        raise AssertionError("the step's script contains a ${{ }} expression, which only GitHub can fill in")
    return matches[0]


# ---------------------------------------------------------------------------
# A fake `gh`
# ---------------------------------------------------------------------------

# Answers from FAKE_GH_DATA (JSON), records each call in FAKE_GH_LOG (one
# JSON array per line), and refuses anything it does not recognise, so an
# unexpected call fails loudly instead of quietly returning nothing.
FAKE_GH = r'''
import json, os, subprocess, sys

args = sys.argv[1:]
with open(os.environ["FAKE_GH_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps(args) + "\n")
with open(os.environ["FAKE_GH_DATA"], encoding="utf-8") as fh:
    data = json.load(fh)

def option(name):
    return args[args.index(name) + 1] if name in args else None

def emit(value):
    text = json.dumps(value)
    jq = option("--jq")
    if jq is None:
        print(text)
        sys.exit(0)
    # gh prints --jq string results without quotes, as `jq -r` does.
    done = subprocess.run(["jq", "-r", jq], input=text, capture_output=True, text=True)
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    sys.exit(done.returncode)

if args[:1] == ["api"]:
    if {"-X", "--method", "-f", "-F", "--field", "--raw-field", "--input"} & set(args):
        sys.stderr.write("fake gh: refusing an API call that writes\n")
        sys.exit(90)
    path = args[1]
    if path not in data.get("api", {}):
        sys.stderr.write("HTTP 404: Not Found (" + path + ")\n")
        sys.exit(1)
    emit(data["api"][path])
elif args[:2] == ["pr", "view"]:
    pr = data.get("prs", {}).get(args[2])
    if pr is None:
        sys.stderr.write("GraphQL: Could not resolve to a PullRequest with the number of " + args[2] + ".\n")
        sys.exit(1)
    fields = option("--json")
    emit({k: pr[k] for k in fields.split(",") if k in pr} if fields else pr)
elif args[:2] in (["pr", "list"], ["issue", "list"]):
    emit([])
elif args[:2] in (["label", "create"], ["pr", "create"], ["issue", "create"], ["pr", "comment"]):
    print("https://example.invalid/fake")
else:
    sys.stderr.write("fake gh: unrecognised call: " + " ".join(args) + "\n")
    sys.exit(97)
'''

READ_ONLY_CALLS = (("api",), ("pr", "view"))


# ---------------------------------------------------------------------------
# Made-up repository content
# ---------------------------------------------------------------------------


def npm_lock(packages: dict[str, str]) -> str:
    entries = {"": {"name": "demo", "version": "1.0.0"}}
    entries.update({key: {"version": version} for key, version in packages.items()})
    return json.dumps({"name": "demo", "version": "1.0.0", "lockfileVersion": 3, "packages": entries}, indent=2) + "\n"


def package_json(overrides: list[tuple[str, str]], version: str = "1.0.0") -> str:
    body = ",\n".join(f'    "{name}": "{spec}"' for name, spec in overrides)
    return '{\n  "name": "demo",\n  "version": "' + version + '",\n  "overrides": {\n' + body + "\n  }\n}\n"


def cargo_lock(crates: list[tuple[str, str]]) -> str:
    out = '# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = "demo"\nversion = "1.0.0"\n'
    for name, version in crates:
        out += f'\n[[package]]\nname = "{name}"\nversion = "{version}"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n'
    return out


def cargo_toml(dependency_lines: list[str]) -> str:
    return '[package]\nname = "demo"\nversion = "1.0.0"\n\n[dependencies]\n' + "".join(f"{line}\n" for line in dependency_lines)


class Origin:
    """A bare repository standing in for GitHub's copy, plus a clone of it
    standing in for the job's checkout (`$GITHUB_WORKSPACE`)."""

    def __init__(self, root: Path) -> None:
        self.bare = root / "origin.git"
        self.workspace = root / "workspace"
        self.count = 0
        self._git(root, "init", "-q", "--bare", "-b", "main", str(self.bare))
        # Let a client fetch any commit by its ID, as GitHub allows.
        self._git(self.bare, "config", "uploadpack.allowAnySHA1InWant", "true")

    @staticmethod
    def _git(cwd: Path, *args: str, input_bytes: bytes | None = None) -> str:
        done = subprocess.run(["git", *args], cwd=cwd, input=input_bytes, capture_output=True, env=GIT_ENV, check=False)
        if done.returncode != 0:
            raise RuntimeError(f"git {' '.join(args)} failed:\n{done.stderr.decode(errors='replace')}")
        return done.stdout.decode().strip()

    def commit(self, ref: str, parent: str | None, files: dict[str, str | None], message: str = "test commit") -> str:
        """Write a commit into the bare repository at `ref` (e.g.
        refs/heads/main or refs/pull/42/head): `parent`'s files with `files`
        applied (None deletes)."""
        stream = bytearray()

        def data(text: str) -> None:
            raw = text.encode()
            stream.extend(f"data {len(raw)}\n".encode() + raw + b"\n")

        stream.extend(f"commit {ref}\ncommitter test <test@example.invalid> 0 +0000\n".encode())
        data(message)
        if parent:
            stream.extend(f"from {parent}\n".encode())
        for name, content in files.items():
            if content is None:
                stream.extend(f"D {name}\n".encode())
            else:
                stream.extend(f"M 100644 inline {name}\n".encode())
                data(content)
        self._git(self.bare, "fast-import", "--quiet", "--force", input_bytes=bytes(stream))
        return self._git(self.bare, "rev-parse", ref)

    def clone(self) -> None:
        """The job's checkout: every branch, but (as on GitHub) no
        refs/pull/* refs, which a script has to fetch for itself. The real
        helper is linked in where the workflow looks for it."""
        self._git(self.bare.parent, "clone", "-q", str(self.bare), str(self.workspace))
        scripts = self.workspace / ".github" / "scripts"
        scripts.mkdir(parents=True)
        (scripts / "check_lockfile_already_fixed.py").symlink_to(HELPER)


def find_bash() -> str:
    bash = shutil.which("bash")
    if bash is None:
        raise AssertionError("bash is not on PATH")
    version = subprocess.run([bash, "-c", "echo ${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]}"], capture_output=True, text=True).stdout.strip()
    major, minor = (int(p) for p in version.split("."))
    if (major, minor) < (4, 4):
        raise AssertionError(f"these tests need bash 4.4 or later, as GitHub's runners have; found {version} at {bash}")
    return bash


class WorkflowTestCase(unittest.TestCase):
    def setUp(self) -> None:
        self.assertTrue(WORKFLOW.is_file(), f"workflow not found at {WORKFLOW}")
        self.assertTrue(HELPER.is_file(), f"helper not found at {HELPER}")
        self.assertIsNotNone(shutil.which("jq"), "jq is not on PATH")
        self.bash = find_bash()
        self._tmp = tempfile.TemporaryDirectory(prefix="forward-port-workflow-test-")
        self.root = Path(self._tmp.name)
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        fake = bin_dir / "gh"
        fake.write_text(f"#!{sys.executable}\n{FAKE_GH}")
        fake.chmod(0o755)
        self.bin_dir = bin_dir
        self.workflow_text = WORKFLOW.read_text(encoding="utf-8")

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def run_step(self, step: Step, env: dict[str, str], gh_data: dict) -> tuple[subprocess.CompletedProcess, list[list[str]], dict[str, str]]:
        """Run one step's script as GitHub would. Returns (the finished
        process, every gh call made, the step's outputs)."""
        missing = step.env_names - set(env)
        self.assertFalse(missing, f"the test does not provide the step's env variable(s): {sorted(missing)}")
        data_file = self.root / "gh-data.json"
        data_file.write_text(json.dumps(gh_data))
        log_file = self.root / "gh-calls.log"
        log_file.write_text("")
        output_file = self.root / "github-output"
        output_file.write_text("")
        script = self.root / "step.sh"
        script.write_text(step.run)
        full_env = dict(GIT_ENV)
        full_env.update(env)
        full_env.update(
            {
                "PATH": f"{self.bin_dir}{os.pathsep}{os.environ['PATH']}",
                "FAKE_GH_DATA": str(data_file),
                "FAKE_GH_LOG": str(log_file),
                "GITHUB_OUTPUT": str(output_file),
                "GITHUB_WORKSPACE": str(self.origin.workspace),
            }
        )
        # The same flags GitHub uses for a `run:` step with the bash shell.
        done = subprocess.run(
            [self.bash, "--noprofile", "--norc", "-eo", "pipefail", str(script)],
            cwd=self.origin.workspace,
            env=full_env,
            capture_output=True,
            text=True,
            check=False,
        )
        calls = [json.loads(line) for line in log_file.read_text().splitlines() if line.strip()]
        outputs: dict[str, str] = {}
        for line in output_file.read_text().splitlines():
            key, sep, value = line.partition("=")
            if sep:
                outputs[key] = value
        return done, calls, outputs


# ---------------------------------------------------------------------------
# The gate
# ---------------------------------------------------------------------------

BASE_PACKAGES = {"node_modules/foo": "1.0.1", "node_modules/bar": "2.0.0"}
BASE_OVERRIDES = [("bar", "^2.0.0"), ("foo", "^1.0.1")]

GATE_OUTPUT_KEYS = {
    "proceed",
    "pr_number",
    "merge_sha",
    "pr_title",
    "author",
    "should_forward_port",
    "forward_port_reason",
    "touched_lockfile",
    "is_release_please",
    "is_routine_grouped",
}


class GateDecides(WorkflowTestCase):
    """The gate's decision for each kind of merged pull request. The made-up
    repository is built once for the whole class: the gate only reads from
    it (a fetch writes nothing but FETCH_HEAD in the checkout)."""

    @classmethod
    def setUpClass(cls) -> None:
        cls._class_tmp = tempfile.TemporaryDirectory(prefix="forward-port-gate-test-")
        cls.origin = o = Origin(Path(cls._class_tmp.name))
        base = o.commit(
            "refs/heads/main",
            None,
            {
                "package.json": package_json(BASE_OVERRIDES),
                "package-lock.json": npm_lock(BASE_PACKAGES),
                "src/app.ts": "export const answer = 41;\n",
            },
        )
        foo_fix = {
            "package.json": package_json([("bar", "^2.0.0"), ("foo", "^1.0.3")]),
            "package-lock.json": npm_lock({"node_modules/foo": "1.0.3", "node_modules/bar": "2.0.0"}),
        }
        # Dependabot's own pull request #42 (closed unmerged when a person
        # combined it into another). GitHub keeps serving its commit at
        # refs/pull/42/head even after Dependabot deletes the branch.
        cls.dependabot_42 = o.commit("refs/pull/42/head", base, foo_fix, "bump foo from 1.0.1 to 1.0.3")
        # A Dependabot pull request merged as-is.
        cls.merged_dependabot = o.commit("refs/heads/merged-dependabot", base, foo_fix)
        # A person's pull request combining #42 with another bump.
        cls.combined = o.commit(
            "refs/heads/combined",
            base,
            {
                "package.json": package_json([("bar", "^2.0.5"), ("foo", "^1.0.3")]),
                "package-lock.json": npm_lock({"node_modules/foo": "1.0.3", "node_modules/bar": "2.0.5"}),
            },
        )
        # A person's pull request that carries #42's bump AND changes code.
        cls.combined_with_code = o.commit(
            "refs/heads/combined-with-code",
            base,
            {**foo_fix, "src/app.ts": "export const answer = 43;\n"},
        )
        # A person's code change whose description happens to mention #42.
        cls.code_change = o.commit("refs/heads/code-change", base, {"src/app.ts": "export const answer = 42;\n"})
        # A person's dependency change that does NOT include #42's bump.
        cls.other_bump = o.commit(
            "refs/heads/other-bump",
            base,
            {
                "package.json": package_json([("bar", "^2.0.5"), ("foo", "^1.0.1")]),
                "package-lock.json": npm_lock({"node_modules/foo": "1.0.1", "node_modules/bar": "2.0.5"}),
            },
        )
        # A release-please version bump.
        cls.release = o.commit(
            "refs/heads/release",
            base,
            {"package.json": package_json(BASE_OVERRIDES, version="1.1.0"), "CHANGELOG.md": "## 1.1.0\n"},
        )
        o.clone()
        cls.gate = extract_step(WORKFLOW.read_text(encoding="utf-8"), "gate", step_id="decide")

    @classmethod
    def tearDownClass(cls) -> None:
        cls._class_tmp.cleanup()

    @staticmethod
    def pr(number: int, *, author: str, head: str, merge_sha: str | None, state: str = "MERGED", base: str = "main",
           body: str = "", labels: tuple[str, ...] = (), files: tuple[str, ...] = ("package-lock.json",)) -> dict:
        return {
            "number": number,
            "state": state,
            "baseRefName": base,
            "author": {"login": author},
            "mergeCommit": {"oid": merge_sha} if merge_sha else None,
            "title": f"pull request {number}",
            "headRefName": head,
            "body": body,
            "labels": [{"name": name} for name in labels],
            "files": [{"path": path} for path in files],
        }

    def dependabot_42_pr(self, *, state: str = "CLOSED", author: str = "app/dependabot",
                         head: str = "dependabot/npm_and_yarn/foo-1.0.3") -> dict:
        return self.pr(42, author=author, head=head, merge_sha=None, state=state)

    def decide(self, merged: dict, *others: dict, event: str = "push") -> dict[str, str]:
        """Run the gate for `merged` (pushed to main, or named by hand) and
        return its outputs, after checking it finished cleanly and only read
        from GitHub."""
        prs = {str(p["number"]): p for p in (merged, *others)}
        sha = merged["mergeCommit"]["oid"] if merged["mergeCommit"] else "0" * 40
        gh_data = {
            "prs": prs,
            "api": {
                f"repos/{REPO}/commits/{sha}/pulls": [
                    {"number": merged["number"], "merged_at": "2026-10-01T00:00:00Z" if merged["state"] == "MERGED" else None}
                ]
            },
        }
        env = {
            "GH_TOKEN": "fake-token",
            "REPO": REPO,
            "EVENT_NAME": event,
            "PUSH_SHA": sha if event == "push" else "",
            "INPUT_PR": str(merged["number"]) if event == "workflow_dispatch" else "",
        }
        done, calls, outputs = self.run_step(self.gate, env, gh_data)
        shown = f"\nexit code: {done.returncode}\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}\noutputs: {outputs}\ngh calls: {calls}"
        self.assertEqual(done.returncode, 0, "the gate step failed" + shown)
        # The gate must only ever READ from GitHub.
        for call in calls:
            self.assertTrue(
                any(tuple(call[: len(prefix)]) == prefix for prefix in READ_ONLY_CALLS), f"the gate made a non-read call: {call}" + shown
            )
        # It must have reached a decision, not stopped part way.
        self.assertIn("proceed", outputs, "the gate wrote no 'proceed' output" + shown)
        if outputs["proceed"] == "true":
            self.assertEqual(GATE_OUTPUT_KEYS - set(outputs), set(), "the gate left outputs unset" + shown)
        self.shown = shown
        return outputs

    def assertForwarded(self, outputs: dict[str, str]) -> None:
        self.assertEqual(outputs.get("proceed"), "true", self.shown)
        self.assertEqual(outputs.get("should_forward_port"), "true", self.shown)

    def assertNotForwarded(self, outputs: dict[str, str]) -> None:
        self.assertNotEqual(outputs.get("should_forward_port"), "true", self.shown)

    # --- Dependabot's own pull requests ------------------------------------

    def test_dependabot_pr_is_forwarded_under_the_graphql_spelling(self) -> None:
        merged = self.pr(64, author="app/dependabot", head="dependabot/npm_and_yarn/foo-1.0.3", merge_sha=self.merged_dependabot)
        self.assertForwarded(self.decide(merged))

    def test_dependabot_pr_is_forwarded_under_the_rest_spelling(self) -> None:
        merged = self.pr(65, author="dependabot[bot]", head="dependabot/npm_and_yarn/foo-1.0.3", merge_sha=self.merged_dependabot)
        self.assertForwarded(self.decide(merged))

    def test_routine_grouped_update_is_not_forwarded(self) -> None:
        merged = self.pr(
            66, author="app/dependabot", head="dependabot/npm_and_yarn/main/npm-minor-patch-a4f3a99ccc", merge_sha=self.merged_dependabot
        )
        outputs = self.decide(merged)
        self.assertNotForwarded(outputs)
        self.assertEqual(outputs.get("is_routine_grouped"), "true", self.shown)

    # --- release-please ------------------------------------------------------

    def test_release_please_pr_naming_a_dependabot_fix_is_not_forwarded_codex_finding_5(self) -> None:
        merged = self.pr(
            70,
            author="app/github-actions",
            head="release-please--branches--main--components--meedyadl",
            merge_sha=self.release,
            body="Release 1.1.0. Includes the fix from #42.",
            files=("package.json", "CHANGELOG.md"),
        )
        outputs = self.decide(merged, self.dependabot_42_pr())
        self.assertNotForwarded(outputs)
        self.assertEqual(outputs.get("is_release_please"), "true", self.shown)

    def test_release_please_pr_is_not_forwarded_whatever_else_qualifies_it(self) -> None:
        merged = self.pr(
            71,
            author="app/github-actions",
            head="release-please--branches--main--components--meedyadl",
            merge_sha=self.release,
            labels=("security",),
            files=("package.json", "CHANGELOG.md"),
        )
        self.assertNotForwarded(self.decide(merged))
        # ...including when someone runs the workflow by hand against it.
        self.assertNotForwarded(self.decide(merged, event="workflow_dispatch"))

    # --- the "body names a Dependabot fix it superseded" path ---------------

    def test_contributor_pr_mentioning_an_unrelated_dependabot_pr_is_not_forwarded_codex_finding_4(self) -> None:
        merged = self.pr(
            80,
            author="a-contributor",
            head="feature/answer",
            merge_sha=self.code_change,
            body="See #42 for an unrelated dependency fix.",
            files=("src/app.ts",),
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_42_pr()))

    def test_dependency_pr_that_does_not_carry_the_named_fix_is_not_forwarded(self) -> None:
        merged = self.pr(
            81,
            author="a-contributor",
            head="chore/bump-bar",
            merge_sha=self.other_bump,
            body="Bumps bar. Supersedes #42.",
            files=("package.json", "package-lock.json"),
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_42_pr()))

    def test_combining_pr_that_carries_the_named_dependabot_fix_is_forwarded(self) -> None:
        merged = self.pr(
            82,
            author="a-maintainer",
            head="chore/consolidate-deps-main",
            merge_sha=self.combined,
            body="Supersedes and closes: #42, #43",
            files=("package.json", "package-lock.json"),
        )
        # #43 is an issue, not a pull request: it must simply be passed over.
        outputs = self.decide(merged, self.dependabot_42_pr())
        self.assertForwarded(outputs)
        self.assertIn("#42", outputs.get("forward_port_reason", ""), self.shown)

    def test_pr_carrying_the_named_fix_plus_a_code_change_is_not_forwarded(self) -> None:
        # The forward-port copies the whole merge commit, so the code change
        # would ride along on the strength of Dependabot's fix.
        merged = self.pr(
            86,
            author="a-contributor",
            head="feature/answer-and-deps",
            merge_sha=self.combined_with_code,
            body="Supersedes #42.",
            files=("package.json", "package-lock.json", "src/app.ts"),
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_42_pr()))

    def test_combining_pr_naming_a_merged_dependabot_pr_is_not_forwarded(self) -> None:
        # A Dependabot pull request that was itself merged is not one this PR
        # superseded; if it was a security fix it was forwarded on its own.
        merged = self.pr(
            83, author="a-maintainer", head="chore/deps", merge_sha=self.combined, body="Follows #42.",
            files=("package.json", "package-lock.json"),
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_42_pr(state="MERGED")))

    def test_combining_pr_naming_a_grouped_dependabot_update_is_not_forwarded(self) -> None:
        merged = self.pr(
            84, author="a-maintainer", head="chore/deps", merge_sha=self.combined, body="Supersedes #42.",
            files=("package.json", "package-lock.json"),
        )
        grouped = self.dependabot_42_pr(head="dependabot/npm_and_yarn/main/npm-minor-patch-0123456789")
        self.assertNotForwarded(self.decide(merged, grouped))

    def test_combining_pr_naming_a_non_dependabot_pr_is_not_forwarded(self) -> None:
        merged = self.pr(
            85, author="a-maintainer", head="chore/deps", merge_sha=self.combined, body="Supersedes #42.",
            files=("package.json", "package-lock.json"),
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_42_pr(author="someone-else")))

    # --- pull requests the gate must not act on at all ----------------------

    def test_unmerged_pr_is_not_forwarded(self) -> None:
        open_pr = self.pr(90, author="app/dependabot", head="dependabot/npm_and_yarn/foo-1.0.3", merge_sha=None, state="OPEN")
        # Pushed: the commit's only pull request is not merged.
        outputs = self.decide(open_pr)
        self.assertEqual(outputs.get("proceed"), "false", self.shown)
        self.assertNotForwarded(outputs)
        # Run by hand against it.
        outputs = self.decide(open_pr, event="workflow_dispatch")
        self.assertEqual(outputs.get("proceed"), "false", self.shown)
        self.assertNotForwarded(outputs)

    def test_pr_not_targeting_main_is_not_forwarded(self) -> None:
        merged = self.pr(
            91, author="app/dependabot", head="dependabot/npm_and_yarn/foo-1.0.3", merge_sha=self.merged_dependabot, base="alpha"
        )
        outputs = self.decide(merged)
        self.assertEqual(outputs.get("proceed"), "false", self.shown)
        self.assertNotForwarded(outputs)


# ---------------------------------------------------------------------------
# The cherry-pick step's "already has it" shortcut
# ---------------------------------------------------------------------------


class ForwardPortShortcut(WorkflowTestCase):
    """The shortcut may skip a channel only when it has checked everything
    the fix commit changed."""

    UNSAFE = cargo_toml(['foo = { version = "1", features = ["unsafe-feature"] }', 'bar = "0.4"'])
    SAFE = cargo_toml(['foo = { version = "1", default-features = false }', 'bar = "0.4"'])

    def setUp(self) -> None:
        super().setUp()
        # Built afresh for each test: the step pushes branches into it.
        self.origin = o = Origin(self.root)
        base = o.commit(
            "refs/heads/main",
            None,
            {
                "src-tauri/Cargo.toml": self.UNSAFE,
                "src-tauri/Cargo.lock": cargo_lock([("foo", "1.2.0"), ("bar", "0.4.15")]),
                "package.json": package_json([("undici", "^7.29.0")]),
                "package-lock.json": npm_lock({"node_modules/undici": "7.29.0"}),
            },
        )
        # Codex's case: the fix turns the unsafe feature off AND bumps bar.
        self.feature_fix = o.commit(
            "refs/heads/feature-fix",
            base,
            {"src-tauri/Cargo.toml": self.SAFE, "src-tauri/Cargo.lock": cargo_lock([("foo", "1.2.0"), ("bar", "0.4.18")])},
        )
        # alpha got the bar bump on its own, but still has the unsafe feature.
        o.commit("refs/heads/alpha", base, {"src-tauri/Cargo.lock": cargo_lock([("foo", "1.2.0"), ("bar", "0.4.18")])})
        # An ordinary dependency fix that beta already has in full.
        self.undici_fix = o.commit(
            "refs/heads/undici-fix",
            base,
            {"package.json": package_json([("undici", "^7.30.0")]), "package-lock.json": npm_lock({"node_modules/undici": "7.30.0"})},
        )
        o.commit(
            "refs/heads/beta",
            base,
            {"package.json": package_json([("undici", "^7.30.0")]), "package-lock.json": npm_lock({"node_modules/undici": "7.30.0"})},
        )
        o.clone()
        self.step = extract_step(self.workflow_text, "forward-port", name_prefix="Cherry-pick onto")

    def forward_port(self, target: str, merge_sha: str) -> tuple[subprocess.CompletedProcess, list[list[str]]]:
        env = {
            "GH_TOKEN": "fake-token",
            "REPO": REPO,
            "TARGET": target,
            "PR_NUMBER": "500",
            "MERGE_SHA": merge_sha,
            "PR_TITLE": "a security fix",
        }
        done, calls, _ = self.run_step(self.step, env, {})
        self.shown = f"\nexit code: {done.returncode}\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}\ngh calls: {calls}"
        self.assertEqual(done.returncode, 0, "the forward-port step failed" + self.shown)
        return done, calls

    def pushed_branches(self) -> list[str]:
        return Origin._git(self.origin.bare, "for-each-ref", "--format=%(refname)", "refs/heads/forward-port/").split()

    def test_feature_change_plus_bump_is_carried_across_not_skipped_codex_finding_3(self) -> None:
        _, calls = self.forward_port("alpha", self.feature_fix)
        self.assertIn(["pr", "create"], [c[:2] for c in calls], "no forward-port pull request was opened" + self.shown)
        self.assertEqual(self.pushed_branches(), ["refs/heads/forward-port/security/pr-500-to-alpha"], self.shown)
        carried = Origin._git(self.origin.bare, "show", "refs/heads/forward-port/security/pr-500-to-alpha:src-tauri/Cargo.toml")
        self.assertIn("default-features = false", carried, "the Cargo.toml change did not reach the channel")

    def test_fix_the_channel_already_has_in_full_is_skipped(self) -> None:
        done, calls = self.forward_port("beta", self.undici_fix)
        self.assertNotIn(["pr", "create"], [c[:2] for c in calls], self.shown)
        self.assertNotIn(["issue", "create"], [c[:2] for c in calls], self.shown)
        self.assertEqual(self.pushed_branches(), [], self.shown)
        self.assertIn("RESULT=ALREADY_FIXED", done.stdout, self.shown)


if __name__ == "__main__":
    unittest.main(verbosity=2)
