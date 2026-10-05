#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/test_check_polish.py
=======================================

Proves check_polish.py catches what it claims to: it copies the parts of
the repository the check reads into a throw-away folder, confirms that
copy is clean, then plants ONE fault at a time and runs the real check
against the copy (`--root`) as a separate program.

Each planted fault must produce all three of:
  - exit code 1 under --strict,
  - the expected section heading and a bullet naming the fault,
  - the check's own last line ("Polish check finished: N finding(s).").

The last point matters as much as the first two. A check that crashes
part-way prints a traceback -- which can contain the very words a test is
looking for -- and exits non-zero, so "exit 1 and the words are there" on
their own could pass a crash as a catch. Requiring the final line means
the run must have reached its end. The last case here proves that guard
works, by running a copy of the check that crashes on purpose and
requiring the harness to call it a failure.

This file quotes the patterns the check hunts for; check_polish.py lists
it in EXEMPT_FILES for that reason.

Pure stdlib, same house style as the checks. Takes about a minute (each
run is a fresh process). Run directly:
    python3 tools/audit-checks/test_check_polish.py
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CHECK = Path(__file__).resolve().parent / "check_polish.py"
FINAL_LINE = re.compile(r"^Polish check finished: (\d+) finding\(s\)\.$", re.M)

# What the check reads. Copied once; each case changes one file and puts
# it back afterwards.
COPY_DIRS = ["help", "src", "public", "src-tauri/src", "src-tauri/capabilities", "src-tauri/icons"]
COPY_FILES = [
    "README.md",
    "package.json",
    ".release-please-manifest.json",
    "vite.config.ts",
    "src-tauri/tauri.conf.json",
    "src-tauri/Cargo.toml",
    # Where the files in public/ are used from (the unused-file rule).
    "index.html",
    "src-tauri/engines.toml",
]


def make_copy(dest: Path) -> None:
    ignore = shutil.ignore_patterns("node_modules", "android", "ios", "*.icns.tmp")
    for d in COPY_DIRS:
        shutil.copytree(REPO / d, dest / d, ignore=ignore)
    for f in COPY_FILES:
        (dest / f).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / f, dest / f)


def run_check(root: Path, check: Path = CHECK) -> tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, str(check), "--strict", "--root", str(root)],
        capture_output=True,
        text=True,
        timeout=300,
    )
    return proc.returncode, proc.stdout + proc.stderr


def outcome(code: int, output: str, section: str, needle: str) -> str | None:
    """None when the planted fault was caught properly, else what went wrong."""
    final = FINAL_LINE.search(output)
    if not final:
        return "the check did not reach its last line (it crashed or stopped early)"
    if code != 1:
        return f"exit code {code}, expected 1"
    if int(final.group(1)) < 1:
        return "the final line reports no findings"
    if f"### {section}" not in output:
        return f'no "### {section}" section'
    if not any(needle in line for line in output.splitlines() if "•" in line):
        return f'no finding mentioning "{needle}"'
    return None


def edit(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    assert old in text, f"{path}: cannot plant fault, {old!r} not found"
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


# Each case: (description, section, text the finding must mention, plant).
# `plant(root)` makes one change in the copy; the harness restores every
# file it touched afterwards.
def cases():
    return [
        (
            "a placeholder address in help prose",
            "Placeholder address or unfinished wording where people can see it",
            "example.com",
            lambda r: append(r / "help/faq.md", "\nSee https://example.com for more.\n"),
        ),
        (
            "an issue number in a translation",
            "Internal reference on screen (issue number or old version note)",
            "#1234",
            lambda r: edit(r / "public/locales/en/translation.json", '"title": "Help",', '"title": "Help (#1234)",'),
        ),
        (
            "an old version note on screen",
            "Internal reference on screen (issue number or old version note)",
            "Requires GAMDL 2.9",
            lambda r: edit(
                r / "src/components/settings/tabs/TemplatesTab.tsx",
                'description="Folder structure for playlist downloads."',
                'description="Folder structure for playlist downloads. Requires GAMDL 2.9.1+."',
            ),
        ),
        (
            "build files that disagree on the version",
            "Name or version disagrees between the build files",
            "disagree on the version",
            lambda r: sub(r / "src-tauri/Cargo.toml", r'(?m)^version = "[^"]+"', 'version = "0.0.1"'),
        ),
        (
            "no native window title",
            "Screens do not name the native window",
            "setTitle",
            lambda r: edit(r / "src/lib/windowTitle.ts", ".setTitle(title)", ".setName(title)"),
        ),
        (
            "a missing icon",
            "Icon missing or the wrong size",
            "32x32.png",
            lambda r: (r / "src-tauri/icons/32x32.png").unlink(),
        ),
        (
            "a page router with no fallback",
            "An unknown page has nowhere to go",
            "default:",
            lambda r: sub(r / "src/App.tsx", r"\n\s*default:\n\s*return <DownloadForm />;[^\n]*", ""),
        ),
        (
            "developer wording on the crash screen",
            "The crash screen is not a way forward",
            "React Error",
            lambda r: edit(r / "src/components/layout/CrashScreen.tsx", "Something went wrong on this screen", "React Error Caught"),
        ),
        (
            "a page header that is not an h1",
            "Heading structure: one <h1> per screen, no heading inside a button",
            "PageHeader.tsx",
            lambda r: sub(r / "src/components/layout/PageHeader.tsx", r"<h1 (className=\"text-xl[^>]*>\{title\})</h1>", r"<h2 \1</h2>"),
        ),
        (
            "a heading inside a button",
            "Heading structure: one <h1> per screen, no heading inside a button",
            "SettingsSection.tsx",
            lambda r: edit(
                r / "src/components/common/SettingsSection.tsx",
                '<span className="flex-1 min-w-0">{title}</span>',
                '<h3 className="flex-1 min-w-0">{title}</h3>',
            ),
        ),
        (
            "an icon-only button with no name",
            "Icon-only button with no name for screen readers",
            "has no visible text",
            lambda r: append(r / "src/components/layout/CrashScreen.tsx", "\nexport const Nameless = () => <button type=\"button\" onClick={() => window.close()}><X /></button>;\n"),
        ),
        (
            "console.log in shipped code",
            "Debug output left in shipped code",
            "console.log",
            lambda r: append(r / "src/lib/windowTitle.ts", "\nconsole.log('debug');\n"),
        ),
        (
            "println! in the backend",
            "Debug output left in shipped code",
            "println!",
            lambda r: edit(
                r / "src-tauri/src/services/spotify_anti_ban.rs",
                "pub fn is_developer_preview_package(package: &str) -> bool {\n",
                'pub fn is_developer_preview_package(package: &str) -> bool {\n    println!("checking {package}");\n',
            ),
        ),
        (
            "a Word draft in help/",
            "Draft, backup or test file in a folder that ships",
            "notes-draft.docx",
            lambda r: (r / "help/notes-draft.docx").write_bytes(b"PK\x03\x04"),
        ),
        (
            # Polish pass M13: a file in public/ that nothing uses.
            "an unused image in public/",
            "A file in public/ ships to everyone, but nothing uses it",
            "logo-unused.png",
            lambda r: (r / "public/logo-unused.png").write_bytes(b"\x89PNG\r\n"),
        ),
        (
            "a silenced bundle-size warning",
            "The bundle-size warning is silenced, or a page meant to load on demand is loaded at start-up",
            "chunkSizeWarningLimit",
            lambda r: edit(r / "vite.config.ts", "rolldownOptions: {", "chunkSizeWarningLimit: 1500,\n\n    rolldownOptions: {"),
        ),
        (
            "the Help page imported up front again",
            "The bundle-size warning is silenced, or a page meant to load on demand is loaded at start-up",
            "HelpViewer is imported up front",
            lambda r: sub(
                r / "src/App.tsx",
                r"const HelpViewer = lazy\(.*\n",
                "import { HelpViewer } from './components/help';\n",
            ),
        ),
        (
            'a link to "#"',
            "Link or button that does nothing",
            'href="#"',
            lambda r: append(r / "src/components/layout/CrashScreen.tsx", '\nexport const Nowhere = () => <a href="#">More</a>;\n'),
        ),
        (
            "a Settings place that does not exist",
            'A "Settings > ..." place that does not exist',
            "Imaginary Section",
            lambda r: append(r / "help/faq.md", "\nTurn it on in **Settings > Lyrics > Imaginary Section**.\n"),
        ),
        (
            "a claim of a feature with no code (BPM)",
            "A feature is claimed that has no working code behind it",
            "BPM tagging",
            lambda r: append(r / "README.md", "\nMeedyaDL writes BPM tags during BPM analysis.\n"),
        ),
        (
            "a claim that Spotify is for everyone",
            "A feature is claimed that has no working code behind it",
            "Spotify downloads for everyone",
            lambda r: append(r / "help/downloading-music.md", "\nPaste an Apple Music or Spotify link.\n"),
        ),
        (
            "a message naming the hidden unlock",
            "A message names the hidden developer unlock",
            "hidden unlock",
            lambda r: edit(
                r / "src-tauri/src/services/spotify_anti_ban.rs",
                "support for that service is planned.\";",
                "support for that service is planned. (Konami code)\";",
            ),
        ),
        (
            "a ratchet count going up",
            "Ratchet: a count that may only go down has moved",
            "above the ceiling",
            lambda r: append(r / "src/components/layout/CrashScreen.tsx", '\nexport const Extra = () => <button type="button" onClick={() => window.close()}>Close</button>;\n'),
        ),
        (
            # Polish pass M8: the raw error as a toast's whole message, and
            # interpolated into one, are both counted (the ceiling is 0).
            "a toast that shows the raw error interpolated into its message",
            "Ratchet: a count that may only go down has moved",
            "toast whose message is the raw error",
            lambda r: append(
                r / "src/components/layout/CrashScreen.tsx",
                "\nexport function warn(addToast: (m: string, t: string) => void, err: unknown) { addToast(`Could not save: ${err}`, 'error'); }\n",
            ),
        ),
        (
            "a toast whose whole message is the raw error",
            "Ratchet: a count that may only go down has moved",
            "toast whose message is the raw error",
            lambda r: append(
                r / "src/components/layout/CrashScreen.tsx",
                "\nexport function warn(addToast: (m: string, t: string) => void, err: unknown) { addToast(err instanceof Error ? err.message : String(err), 'error'); }\n",
            ),
        ),
        (
            # Polish pass M17: three full stops in on-screen text, in a
            # screen and in a translation. (Spread syntax such as
            # `{...props}` is all over the unmodified copy, which must stay
            # clean, so that side is covered by the first test.)
            "three full stops in on-screen text",
            "Ratchet: a count that may only go down has moved",
            'three full stops "..."',
            lambda r: append(r / "src/components/layout/CrashScreen.tsx", "\nexport const Waiting = () => <p>Loading...</p>;\n"),
        ),
        (
            "three full stops in a translation",
            "Ratchet: a count that may only go down has moved",
            'three full stops "..."',
            lambda r: sub(r / "public/locales/en/translation.json", r'"Checking…"', '"Checking..."'),
        ),
    ]


def append(path: Path, text: str) -> None:
    path.write_text(path.read_text(encoding="utf-8") + text, encoding="utf-8")


def sub(path: Path, pattern: str, repl: str) -> None:
    text = path.read_text(encoding="utf-8")
    new, n = re.subn(pattern, repl, text, count=1)
    assert n == 1, f"{path}: cannot plant fault, pattern not found"
    path.write_text(new, encoding="utf-8")


def snapshot(root: Path) -> dict[Path, bytes]:
    return {p: p.read_bytes() for p in root.rglob("*") if p.is_file()}


def restore(root: Path, before: dict[Path, bytes]) -> None:
    for p in [p for p in root.rglob("*") if p.is_file()]:
        if p not in before:
            p.unlink()
    for p, data in before.items():
        if not p.exists() or p.read_bytes() != data:
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)


def main() -> int:
    failures = 0
    with tempfile.TemporaryDirectory(prefix="polish-selftest-") as tmp:
        root = Path(tmp) / "repo"
        make_copy(root)

        code, output = run_check(root)
        final = FINAL_LINE.search(output)
        if code == 0 and final and final.group(1) == "0":
            print("ok    the unmodified copy is clean")
        else:
            failures += 1
            print(f"FAIL  the unmodified copy is not clean (exit {code}):\n{output[-2000:]}")

        before = snapshot(root)
        for description, section, needle, plant in cases():
            plant(root)
            code, output = run_check(root)
            problem = outcome(code, output, section, needle)
            restore(root, before)
            if problem:
                failures += 1
                print(f"FAIL  {description}: {problem}\n{output[-1500:]}")
            else:
                print(f"ok    {description}")

        # The guard itself: a check that crashes part-way must never count
        # as having caught anything, even though it exits non-zero and its
        # traceback can contain the words a case looks for.
        broken = Path(tmp) / "check_polish_crashing.py"
        broken.write_text(
            CHECK.read_text(encoding="utf-8").replace(
                "    for rule in RULES:\n        rule()",
                '    print("### Placeholder address or unfinished wording where people can see it")\n'
                '    print("  • help/faq.md:1 — \\"example.com\\" in text people read")\n'
                '    raise RuntimeError("deliberate crash")',
            ),
            encoding="utf-8",
        )
        code, output = run_check(root, broken)
        problem = outcome(code, output, "Placeholder address or unfinished wording where people can see it", "example.com")
        if problem and "did not reach its last line" in problem:
            print("ok    a check that crashes part-way is reported as a failure, not a catch")
        else:
            failures += 1
            print(f"FAIL  a crashing check was not caught by the harness (problem={problem!r})")

        # A count below its ceiling must be reported, so the ceiling gets
        # lowered in the same change. Every ceiling is 0 since the polish
        # pass, so no count can drop below it in the copy; instead the
        # check itself is copied with one ceiling raised. (This used to turn
        # a hand-made <select> into a <div>; none are left.)
        raised = Path(tmp) / "check_polish_raised_ceiling.py"
        raised_text = CHECK.read_text(encoding="utf-8").replace('    "raw_select": 0,\n', '    "raw_select": 1,\n', 1)
        raised.write_text(raised_text, encoding="utf-8")
        code, output = run_check(root, raised)
        problem = outcome(code, output, "Ratchet: a count that may only go down has moved", "below the ceiling")
        if '"raw_select": 1,' not in raised_text:
            problem = "could not raise the ceiling in the copy of the check"
        if problem:
            failures += 1
            print(f"FAIL  a ratchet count going down without lowering the ceiling: {problem}\n{output[-1500:]}")
        else:
            print("ok    a ratchet count going down without lowering the ceiling")

    total = len(cases()) + 3
    print(f"\n{total - failures}/{total} passed")
    # The last line, always, for the same reason as in the check itself.
    print("test_check_polish finished.")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
