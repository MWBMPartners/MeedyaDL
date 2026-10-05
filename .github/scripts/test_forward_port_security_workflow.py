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
    GitHub and decides whether to copy it onto the `alpha`, `beta` and
    `release-candidate` branches: Dependabot's own (not a routine grouped
    update), anything with the `security` label, or anything run by hand —
    never release-please, and never on the strength of a description.
  - The `flag-unported-lockfile-change` job's comment step: the notice that
    names any Dependabot pull request a description mentions, gives the two
    commands that forward it, and is posted even when its lookups fail.
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

def emit(value, pages=None):
    """Print `value` as gh would. With `pages`, behave like --paginate in
    the conservative way: apply --jq to each page in turn and print every
    result, so a script must not assume one line of output per call."""
    jq = option("--jq")
    status = 0
    for page in (pages if pages is not None else [value]):
        text = json.dumps(page)
        if jq is None:
            print(text)
            continue
        # gh prints --jq string results without quotes, as `jq -r` does.
        done = subprocess.run(["jq", "-r", jq], input=text, capture_output=True, text=True)
        sys.stdout.write(done.stdout)
        sys.stderr.write(done.stderr)
        status = status or done.returncode
    sys.exit(status)

if args[:1] == ["api"]:
    if {"-X", "--method", "-f", "-F", "--field", "--raw-field", "--input"} & set(args):
        sys.stderr.write("fake gh: refusing an API call that writes\n")
        sys.exit(90)
    # The path is the first argument that is not an option or an option's value.
    path, rest = None, args[1:]
    while rest:
        word = rest.pop(0)
        if word in ("--jq", "-q", "-H", "--header"):
            rest.pop(0)
        elif not word.startswith("-"):
            path = word
            break
    if path not in data.get("api", {}):
        sys.stderr.write("HTTP 404: Not Found (" + str(path) + ")\n")
        sys.exit(1)
    answer = data["api"][path]
    # {"pages": [...]} stands for a long list GitHub serves in pages. Without
    # --paginate only the first page comes back, as on GitHub.
    if isinstance(answer, dict) and "pages" in answer:
        emit(None, answer["pages"] if "--paginate" in args else answer["pages"][:1])
    emit(answer)
elif args[:2] == ["pr", "view"]:
    pr = data.get("prs", {}).get(args[2])
    if pr is None:
        sys.stderr.write("GraphQL: Could not resolve to a PullRequest with the number of " + args[2] + ".\n")
        sys.exit(1)
    fields = option("--json")
    emit({k: pr[k] for k in fields.split(",") if k in pr} if fields else pr)
elif args[:2] == ["pr", "list"]:
    emit(data.get("pr_list", []))
elif args[:2] == ["issue", "list"]:
    emit(data.get("issue_list", []))
elif args[:2] in (["label", "create"], ["pr", "create"], ["issue", "create"], ["pr", "comment"]):
    print("https://example.invalid/fake")
else:
    sys.stderr.write("fake gh: unrecognised call: " + " ".join(args) + "\n")
    sys.exit(97)
'''

READ_ONLY_CALLS = (("api",), ("pr", "view"))

# The account the fake `gh api user` reports: the one whose token the
# workflow runs with, and so the author of its own comments and issues.
BOT = "forward-port-bot"
MARKER = "<!-- forward-port-security: not-forward-ported notice -->"


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
    """The gate's decision for each kind of merged pull request.

    The gate no longer reads git or a pull request's description, but the
    made-up repository is kept: it lets the tests below show that pull
    requests which DO carry a named Dependabot fix are still not forwarded
    on the strength of their description. Built once for the whole class,
    because nothing writes to it."""

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
        # A person's pull request combining #42 with another bump: it really
        # does carry #42's fix.
        cls.combined = o.commit(
            "refs/heads/combined",
            base,
            {
                "package.json": package_json([("bar", "^2.0.5"), ("foo", "^1.0.3")]),
                "package-lock.json": npm_lock({"node_modules/foo": "1.0.3", "node_modules/bar": "2.0.5"}),
            },
        )
        # Codex's second review, finding 5: main already got foo 1.0.3 from an
        # independent update, Dependabot's equivalent #43 was closed unmerged,
        # and a later pull request that only bumps bar mentions #43.
        old_main = o.commit(
            "refs/heads/old-main",
            None,
            {"package.json": package_json(BASE_OVERRIDES), "package-lock.json": npm_lock(BASE_PACKAGES)},
            "main before the independent update",
        )
        landed = o.commit("refs/heads/landed", old_main, foo_fix, "an independent update lands foo 1.0.3")
        cls.dependabot_43 = o.commit("refs/pull/43/head", old_main, foo_fix, "dependabot: bump foo to 1.0.3")
        cls.bar_only = o.commit(
            "refs/heads/bar-only",
            landed,
            {"package-lock.json": npm_lock({"node_modules/foo": "1.0.3", "node_modules/bar": "2.0.5"})},
            "bump bar only",
        )
        # A person's ordinary change to a lockfile.
        cls.other_bump = o.commit(
            "refs/heads/other-bump",
            base,
            {"package-lock.json": npm_lock({"node_modules/foo": "1.0.1", "node_modules/bar": "2.0.5"})},
        )
        # A release-please version bump.
        cls.release = o.commit(
            "refs/heads/release",
            base,
            {"package.json": package_json(BASE_OVERRIDES, version="1.1.0"), "CHANGELOG.md": "## 1.1.0\n"},
        )
        o.clone()
        workflow_text = WORKFLOW.read_text(encoding="utf-8")
        cls.gate = extract_step(workflow_text, "gate", step_id="decide")
        cls.notice = extract_step(workflow_text, "flag-unported-lockfile-change", name_prefix="Comment on the PR")

    @classmethod
    def tearDownClass(cls) -> None:
        cls._class_tmp.cleanup()

    @staticmethod
    def pr(number: int, *, author: str, head: str, merge_sha: str | None, state: str = "MERGED", base: str = "main",
           body: str = "", labels: tuple[str, ...] = (), files: tuple[str, ...] = ("package-lock.json",),
           cross_repository: bool = False, title: str | None = None) -> dict:
        return {
            "number": number,
            "isCrossRepository": cross_repository,
            "state": state,
            "baseRefName": base,
            "author": {"login": author},
            "mergeCommit": {"oid": merge_sha} if merge_sha else None,
            "title": title if title is not None else f"pull request {number}",
            "headRefName": head,
            "body": body,
            "labels": [{"name": name} for name in labels],
            "files": [{"path": path} for path in files],
        }

    def dependabot_pr(self, number: int = 42, *, author: str = "app/dependabot") -> dict:
        return self.pr(number, author=author, head="dependabot/npm_and_yarn/foo-1.0.3", merge_sha=None, state="CLOSED")

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
        self.gate_calls = calls
        return outputs

    def assertForwarded(self, outputs: dict[str, str]) -> None:
        self.assertEqual(outputs.get("proceed"), "true", self.shown)
        self.assertEqual(outputs.get("should_forward_port"), "true", self.shown)

    def assertNotForwarded(self, outputs: dict[str, str]) -> None:
        self.assertNotEqual(outputs.get("should_forward_port"), "true", self.shown)

    def assertNoticeJobRuns(self, outputs: dict[str, str]) -> None:
        """The conditions in the notice job's `if:`, for a push event."""
        self.assertEqual(
            (outputs.get("proceed"), outputs.get("should_forward_port"), outputs.get("touched_lockfile"),
             outputs.get("is_release_please"), outputs.get("is_routine_grouped")),
            ("true", "false", "true", "false", "false"),
            "the notice job would not run for this pull request" + self.shown,
        )

    def post_notice(self, outputs: dict[str, str], gh_data: dict) -> tuple[subprocess.CompletedProcess, str | None]:
        """Run the notice job's step with the gate's outputs; return the
        finished process and the body of the comment it posted (None if it
        posted none)."""
        env = {
            "GH_TOKEN": "fake-token",
            "REPO": REPO,
            "PR_NUMBER": outputs["pr_number"],
            "MERGE_SHA": outputs["merge_sha"],
            "REASON": outputs["forward_port_reason"],
        }
        done, calls, _ = self.run_step(self.notice, env, gh_data)
        self.shown += f"\n--- notice step ---\nexit code: {done.returncode}\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}\ngh calls: {calls}"
        self.assertEqual(done.returncode, 0, "the notice step failed" + self.shown)
        comments = [c for c in calls if c[:2] == ["pr", "comment"]]
        self.assertLessEqual(len(comments), 1, self.shown)
        if not comments:
            return done, None
        self.assertEqual(comments[0][2], outputs["pr_number"], self.shown)
        return done, comments[0][comments[0].index("--body") + 1]

    def assertCarriesInstructions(self, body: str | None, pr_number: str) -> None:
        self.assertIsNotNone(body, "no notice was posted" + self.shown)
        self.assertIn(f"gh pr edit {pr_number} --add-label security", body, self.shown)
        self.assertIn(f"gh workflow run forward-port-security.yml -f pr_number={pr_number}", body, self.shown)

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

    # --- a person's say-so: the label, or a manual run ----------------------

    def test_security_label_forwards(self) -> None:
        merged = self.pr(67, author="a-maintainer", head="chore/deps", merge_sha=self.combined, labels=("security",))
        self.assertForwarded(self.decide(merged))

    def test_manual_run_forwards(self) -> None:
        merged = self.pr(68, author="a-maintainer", head="chore/deps", merge_sha=self.combined)
        self.assertForwarded(self.decide(merged, event="workflow_dispatch"))

    def test_label_or_manual_run_forwards_even_a_grouped_update(self) -> None:
        # Decision B: a person choosing to forward is never second-guessed.
        grouped = "dependabot/npm_and_yarn/main/npm-minor-patch-a4f3a99ccc"
        labelled = self.pr(69, author="app/dependabot", head=grouped, merge_sha=self.merged_dependabot, labels=("security",))
        self.assertForwarded(self.decide(labelled))
        unlabelled = self.pr(69, author="app/dependabot", head=grouped, merge_sha=self.merged_dependabot)
        self.assertForwarded(self.decide(unlabelled, event="workflow_dispatch"))

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
        outputs = self.decide(merged, self.dependabot_pr())
        self.assertNotForwarded(outputs)
        self.assertEqual(outputs.get("is_release_please"), "true", self.shown)

    def test_release_please_pr_is_never_forwarded_whatever_else_qualifies_it(self) -> None:
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

    # --- a description is never enough on its own ---------------------------

    def test_pr_whose_description_names_a_dependabot_pr_is_not_forwarded_but_the_notice_says_how(self) -> None:
        # This pull request genuinely carries #42's fix, and says so. It is
        # still not forwarded on the strength of its description; instead
        # the notice names #42 and gives the two commands that forward it.
        merged = self.pr(
            82,
            author="a-maintainer",
            head="chore/consolidate-deps-main",
            merge_sha=self.combined,
            body="Supersedes and closes: #42, #43, #44",
            files=("package.json", "package-lock.json"),
        )
        someone_else = self.pr(44, author="a-contributor", head="feature/x", merge_sha=None, state="CLOSED")
        outputs = self.decide(merged, self.dependabot_pr(42), someone_else)
        self.assertNotForwarded(outputs)
        # The gate did not even look the named pull requests up.
        self.assertEqual([c for c in self.gate_calls if c[:3] in (["pr", "view", "42"], ["pr", "view", "44"])], [], self.shown)
        self.assertNoticeJobRuns(outputs)
        # #43 is not a pull request at all (the fake gh does not know it),
        # #44 is not Dependabot's: neither is named in the notice.
        _, body = self.post_notice(
            outputs,
            {"prs": {"82": merged, "42": self.dependabot_pr(42), "44": someone_else}, "api": {f"repos/{REPO}/issues/82/comments": []}},
        )
        self.assertCarriesInstructions(body, "82")
        self.assertIn("#42", body, self.shown)
        self.assertNotIn("#43", body, self.shown)
        self.assertNotIn("#44", body, self.shown)

    def test_notice_names_a_dependabot_pr_under_the_rest_spelling_too(self) -> None:
        merged = self.pr(83, author="a-maintainer", head="chore/deps", merge_sha=self.combined, body="Closes #45.")
        outputs = self.decide(merged)
        dependabot_rest = self.dependabot_pr(45, author="dependabot[bot]")
        _, body = self.post_notice(
            outputs, {"prs": {"83": merged, "45": dependabot_rest}, "api": {f"repos/{REPO}/issues/83/comments": []}}
        )
        self.assertCarriesInstructions(body, "83")
        self.assertIn("#45", body, self.shown)

    def test_pr_mentioning_an_old_dependabot_fix_is_not_forwarded_codex_review_2_finding_5(self) -> None:
        # Its tree "contains" #43's fix only because main already had it.
        merged = self.pr(
            87, author="a-contributor", head="chore/bump-bar", merge_sha=self.bar_only, body="Bumps bar. See #43."
        )
        self.assertNotForwarded(self.decide(merged, self.dependabot_pr(43)))

    def test_notice_is_posted_even_when_every_lookup_fails(self) -> None:
        merged = self.pr(84, author="a-contributor", head="chore/deps", merge_sha=self.other_bump, body="See #42.")
        outputs = self.decide(merged, self.dependabot_pr(42))
        self.assertNoticeJobRuns(outputs)
        # The fake gh now knows nothing: the "already posted?" check, the
        # description and #42 all fail to look up.
        _, body = self.post_notice(outputs, {"prs": {}, "api": {}})
        self.assertCarriesInstructions(body, "84")
        self.assertIn("could not read the pull request's description", body, self.shown)

    def notice_for(self, number: int, comments, *, identity: bool = True) -> str | None:
        """Post (or not) the notice for an ordinary not-forwarded pull
        request whose existing comments are `comments` (a list, or
        {"pages": [...]}); `identity=False` makes `gh api user` fail."""
        merged = self.pr(number, author="a-contributor", head="chore/deps", merge_sha=self.other_bump)
        outputs = self.decide(merged)
        self.assertNoticeJobRuns(outputs)
        api = {f"repos/{REPO}/issues/{number}/comments": comments}
        if identity:
            api["user"] = {"login": BOT}
        _, body = self.post_notice(outputs, {"prs": {str(number): merged}, "api": api})
        return body

    @staticmethod
    def comment(author: str, body: str) -> dict:
        return {"user": {"login": author}, "body": body}

    def test_notice_is_not_posted_twice(self) -> None:
        # The workflow's own earlier notice suppresses another one.
        body = self.notice_for(85, [self.comment(BOT, MARKER + "\nearlier notice")])
        self.assertIsNone(body, self.shown)

    def test_marker_posted_by_someone_else_does_not_silence_the_notice_codex_review_3(self) -> None:
        # On a public repository anyone can post a comment containing the
        # marker. That must not switch the safety net off.
        body = self.notice_for(86, [self.comment("an-outsider", MARKER + "\nnothing to see here")])
        self.assertCarriesInstructions(body, "86")

    def test_an_earlier_notice_on_page_2_is_found(self) -> None:
        page_1 = [self.comment(f"person-{i}", f"comment {i}") for i in range(30)]
        page_2 = [self.comment(BOT, MARKER + "\nearlier notice")]
        body = self.notice_for(88, {"pages": [page_1, page_2]})
        self.assertIsNone(body, self.shown)

    def test_a_failed_identity_lookup_still_posts(self) -> None:
        # Without knowing its own name, the workflow cannot tell its own
        # earlier notice from a stranger's copy, so it posts.
        body = self.notice_for(89, [self.comment(BOT, MARKER + "\nearlier notice")], identity=False)
        self.assertCarriesInstructions(body, "89")

    # --- names an outsider controls -----------------------------------------

    def test_fork_branch_named_like_release_please_is_not_treated_as_release_please(self) -> None:
        # Anyone can name a fork's branch anything. Treated as release-please,
        # it would be neither forwarded nor flagged by the notice.
        merged = self.pr(
            92,
            author="an-outsider",
            head="release-please--branches--main--components--meedyadl",
            merge_sha=self.other_bump,
            cross_repository=True,
        )
        outputs = self.decide(merged)
        self.assertEqual(outputs.get("is_release_please"), "false", self.shown)
        self.assertNoticeJobRuns(outputs)
        # ...and once a person labels it, it is forwarded.
        labelled = self.pr(
            92,
            author="an-outsider",
            head="release-please--branches--main--components--meedyadl",
            merge_sha=self.other_bump,
            cross_repository=True,
            labels=("security",),
        )
        self.assertForwarded(self.decide(labelled))

    def test_branch_named_like_a_grouped_update_counts_only_for_dependabot(self) -> None:
        merged = self.pr(93, author="an-outsider", head="fix/npm-minor-patch-lookalike", merge_sha=self.other_bump, cross_repository=True)
        outputs = self.decide(merged)
        self.assertEqual(outputs.get("is_routine_grouped"), "false", self.shown)
        self.assertNoticeJobRuns(outputs)

    def test_control_characters_in_a_title_cannot_add_outputs(self) -> None:
        # GitHub's runner reads the outputs file line by line. A title must
        # not be able to start a line of its own and override, say, the
        # commit to forward. (This test reads a carriage return as a line
        # break, as a cautious reader of that file might.)
        merged = self.pr(
            94,
            author="app/dependabot",
            head="dependabot/npm_and_yarn/foo-1.0.3",
            merge_sha=self.merged_dependabot,
            title="Bump foo\rmerge_sha=" + "f" * 40 + "\x1b[0m\npr_number=1",
        )
        outputs = self.decide(merged)
        self.assertForwarded(outputs)
        self.assertEqual(outputs.get("merge_sha"), self.merged_dependabot, self.shown)
        self.assertEqual(outputs.get("pr_number"), "94", self.shown)

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
        # gamma moved undici its own way, so copying undici_fix conflicts.
        o.commit(
            "refs/heads/gamma",
            base,
            {"package.json": package_json([("undici", "^7.29.5")]), "package-lock.json": npm_lock({"node_modules/undici": "7.29.5"})},
        )
        o.clone()
        self.step = extract_step(self.workflow_text, "forward-port", name_prefix="Cherry-pick onto")

    def forward_port(self, target: str, merge_sha: str, gh_data: dict | None = None) -> tuple[subprocess.CompletedProcess, list[list[str]]]:
        env = {
            "GH_TOKEN": "fake-token",
            "REPO": REPO,
            "TARGET": target,
            "PR_NUMBER": "500",
            "MERGE_SHA": merge_sha,
            "PR_TITLE": "a security fix",
        }
        data = {"api": {"user": {"login": BOT}}}
        data.update(gh_data or {})
        done, calls, _ = self.run_step(self.step, env, data)
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

    def test_a_forks_pull_request_with_the_same_branch_name_does_not_stop_the_forward_port(self) -> None:
        # `gh pr list --head` matches the branch NAME, and a fork can use any
        # name. Only this repository's own branch, into this channel, counts.
        fork_pr = {"number": 9, "isCrossRepository": True, "baseRefName": "alpha", "author": {"login": "an-outsider"}}
        _, calls = self.forward_port("alpha", self.feature_fix, {"pr_list": [fork_pr]})
        self.assertIn(["pr", "create"], [c[:2] for c in calls], "the fork's pull request stopped the forward-port" + self.shown)

    def test_the_workflows_own_open_pull_request_is_not_duplicated(self) -> None:
        own_pr = {"number": 9, "isCrossRepository": False, "baseRefName": "alpha", "author": {"login": BOT}}
        _, calls = self.forward_port("alpha", self.feature_fix, {"pr_list": [own_pr]})
        self.assertNotIn(["pr", "create"], [c[:2] for c in calls], self.shown)

    def test_an_outsiders_issue_with_the_same_title_does_not_stop_the_conflict_issue(self) -> None:
        title = "[forward-port] #500 -> gamma: security fix needs manual forward-port"
        _, calls = self.forward_port("gamma", self.undici_fix, {"issue_list": [{"number": 7, "title": title, "author": {"login": "an-outsider"}}]})
        self.assertIn(["issue", "create"], [c[:2] for c in calls], "the outsider's issue stopped the tracking issue" + self.shown)

    def test_the_workflows_own_open_issue_is_not_duplicated(self) -> None:
        title = "[forward-port] #500 -> gamma: security fix needs manual forward-port"
        _, calls = self.forward_port("gamma", self.undici_fix, {"issue_list": [{"number": 7, "title": title, "author": {"login": BOT}}]})
        self.assertNotIn(["issue", "create"], [c[:2] for c in calls], self.shown)

    def test_fix_the_channel_already_has_in_full_is_skipped(self) -> None:
        done, calls = self.forward_port("beta", self.undici_fix)
        self.assertNotIn(["pr", "create"], [c[:2] for c in calls], self.shown)
        self.assertNotIn(["issue", "create"], [c[:2] for c in calls], self.shown)
        self.assertEqual(self.pushed_branches(), [], self.shown)
        self.assertIn("RESULT=ALREADY_FIXED", done.stdout, self.shown)


if __name__ == "__main__":
    unittest.main(verbosity=2)
