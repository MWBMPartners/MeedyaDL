#!/usr/bin/env python3
# Copyright (c) 2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
Outbound User-Agent consistency check.

MeedyaDL sends outbound HTTP requests under one of FOUR constants/functions,
all defined in `src-tauri/src/utils/http_client.rs`. The policy is a
four-way split (Groups A-D):

  A. `APP_USER_AGENT` — MeedyaDL's own identity string
     (`"MeedyaDL/{CARGO_PKG_VERSION} (+https://github.com/MWBMPartners/MeedyaDL)"`).
     Sent to (1) first-party endpoints — anything belonging to MWBM
     Partners / MWBM Partners Ltd / MeedyaSuite / Meedya / Scriptkey,
     including the `MWBMPartners`, `Skriptey`, and `MeedyaSuite` GitHub
     orgs (covers `service_status.rs`'s `raw.githubusercontent.com/
     MWBMPartners/...` reads) — and (2) integrations that specifically
     require identification: the GitHub API (`update_checker.rs` 6 sites,
     `dependency_manager.rs` 2 sites — GitHub's own guidance asks
     integrations to identify themselves, and it's the channel GitHub uses
     to contact maintainers of a misbehaving integration) and MusicBrainz
     (`musicbrainz_service.rs` — a licensed API whose Terms of Service
     require an accurate, identifying UA).
  B. `SAFARI_MACOS_USER_AGENT` — a fixed macOS Safari UA string. Sent
     ALWAYS to Apple Music endpoints (`apple_music_api.rs`,
     `commands/credentials.rs`, `animated_artwork_service.rs`'s
     `ffmpeg -user_agent`), regardless of the host OS — Apple's edges
     expect a Safari client, so even a Windows/Linux install presents as
     macOS Safari here.
  C. `browser_user_agent()` — a genuine, platform-appropriate browser UA
     selected at runtime by host OS family (macOS/Windows/Linux; OS family
     only, never architecture). Sent to everything else: Odesli
     (`odesli_service.rs`), PyPI (`pip_engine_service.rs`), and any other
     generic third-party asset download. A Safari UA arriving from a
     Windows host would itself be an implausible, anomaly-signalling
     combination — this constant exists so the UA stays plausible for
     the host OS.
  D. `full_user_agent()` — a platform-bearing variant (`"MeedyaDL/{version}
     ({OSName} {Arch}/{OSVersion})"`), reserved for MWBM-IntAppsAPI (our
     own backend, `feature_flag_service.rs`). Never sent to a third party.

Before `APP_USER_AGENT` existed, the identity string was hand-typed at every
reqwest call site — some as `"MeedyaDL"`, some as `"meedyadl"`, and one
(`musicbrainz_service.rs`) as a hardcoded `"MeedyaDL/0.6"` that silently went
stale as the app version moved through the 1.x line while the literal never
did. None of that drift is visible to the Rust compiler: a `&str` literal
type-checks identically to a `&'static str` constant, so nothing short of a
dedicated scan catches a new hand-typed UA sneaking back in.

This is the MeedyaDL analog of `check_ipc_commands.py` /
`check_codec_registry.py` — a pure cross-source reference validator with no
compiler link enforcing the invariant, run as an advisory PR check plus a
local `--strict` gate.

Rule: any `.header("User-Agent", "<literal>")` or `.user_agent("<literal>")`
call whose argument is a STRING LITERAL is a finding. An IDENTIFIER argument
(`APP_USER_AGENT`, `SAFARI_MACOS_USER_AGENT`, `browser_user_agent()`, a
local `ua` variable, ...) always PASSES — that is exactly the shape every
migrated call site now has. Note `SAFARI_MACOS_USER_AGENT`'s own definition
is a `pub const SAFARI_MACOS_USER_AGENT: &str = "Mozilla/5.0 ...";`
declaration, which matches neither pattern (it isn't a `.header(...)` or
`.user_agent(...)` call), so no allowlist is needed for the deliberate
Apple-impersonation UA or its definition site.

Exit code:
  0 — no findings, OR findings without --strict
  1 — at least one finding AND --strict was passed

Usage:
  python3 tools/audit-checks/check_user_agent.py [--strict]
  python3 tools/audit-checks/check_user_agent.py --self-test
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_SRC = REPO_ROOT / "src-tauri" / "src"

# `.header("User-Agent", "<literal>")` — only matches when the second
# argument is itself a quoted string literal. An identifier second argument
# (no surrounding quotes) does not match, so `.header("User-Agent", ua)` and
# `.header("User-Agent", APP_USER_AGENT)` both pass.
HEADER_UA_STRING_RE = re.compile(r'\.header\(\s*"User-Agent"\s*,\s*"[^"]*"\s*\)')
# `.user_agent("<literal>")` — same shape, reqwest's dedicated builder
# method. `.user_agent(APP_USER_AGENT)` / `.user_agent(USER_AGENT)` pass.
USER_AGENT_METHOD_STRING_RE = re.compile(r'\.user_agent\(\s*"[^"]*"\s*\)')


def strip_line_comments(text: str) -> str:
    """Strip // line comments and /* */ block comments while preserving line
    numbers (block comments are replaced by an equal count of newlines so
    reported line numbers still line up with the original file)."""
    text = re.sub(
        r"/\*.*?\*/",
        lambda m: "\n" * m.group(0).count("\n"),
        text,
        flags=re.DOTALL,
    )
    # Strip // to end-of-line. This is a heuristic (it will also blank a //
    # that appears inside a string literal), which is acceptable here: a
    # false strip can only hide a finding, never invent one.
    text = re.sub(r"//[^\n]*", "", text)
    return text


def find_hardcoded_user_agents(text: str) -> list[tuple[int, str]]:
    """Scan already-comment-stripped Rust source text for hardcoded
    User-Agent string literals. Returns a list of (line_no, matched_snippet)
    in file order."""
    findings: list[tuple[int, str]] = []
    for pattern in (HEADER_UA_STRING_RE, USER_AGENT_METHOD_STRING_RE):
        for m in pattern.finditer(text):
            line_no = text[: m.start()].count("\n") + 1
            findings.append((line_no, m.group(0)))
    findings.sort(key=lambda f: f[0])
    return findings


def scan_repo() -> list[tuple[str, int, str]]:
    """Scan every .rs file under src-tauri/src for hardcoded User-Agent
    literals. Returns [(relative_path, line_no, snippet)]."""
    results: list[tuple[str, int, str]] = []
    for rs in sorted(RUST_SRC.rglob("*.rs")):
        try:
            raw = rs.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        stripped = strip_line_comments(raw)
        rel = str(rs.relative_to(REPO_ROOT))
        for line_no, snippet in find_hardcoded_user_agents(stripped):
            results.append((rel, line_no, snippet))
    return results


def check() -> int:
    findings = scan_repo()

    print(f"Rust files scanned under src-tauri/src : "
          f"{len(list(RUST_SRC.rglob('*.rs')))}")
    print(f"Hardcoded User-Agent literals found     : {len(findings)}")
    print()

    if findings:
        print("### Hardcoded User-Agent string literal (use one of the four named "
              "UA constants/functions instead)\n")
        for rel, ln, snippet in findings:
            print(f"  • {rel}:{ln} — `{snippet}` hardcodes a User-Agent string literal; "
                  f"use crate::utils::http_client::APP_USER_AGENT (first-party endpoints "
                  f"+ GitHub + MusicBrainz), SAFARI_MACOS_USER_AGENT (Apple Music, always), "
                  f"or browser_user_agent() (everything else — platform-appropriate)")
        print()
    else:
        print("OK — no hardcoded User-Agent string literals outside the constant definitions.")

    if findings and "--strict" in sys.argv:
        return 1
    return 0


# ---------------------------------------------------------------------------
# Self-test / negative fixture.
#
# Per the audit-checks house rule ("add a negative test when changing them"),
# this in-process fixture proves the two regexes actually catch the drift
# they're meant to catch, and don't false-positive on the passing shapes the
# migration (WP1) left behind. Run with `--self-test`; does not touch disk or
# require a real MeedyaDL checkout, so it can run in isolation.
# ---------------------------------------------------------------------------

_SELF_TEST_SHOULD_FLAG = [
    # The original three-variant drift this checker exists to catch.
    '.header("User-Agent", "MeedyaDL")',
    '.header("User-Agent", "meedyadl")',
    # A `.user_agent(...)` builder call with a hand-typed literal.
    '.user_agent("MeedyaDL/0.6 (https://example.invalid)")',
    # Empty-string edge case must still match (it's still a literal).
    '.header("User-Agent", "")',
]

_SELF_TEST_SHOULD_PASS = [
    # The migrated shape: identifier argument, not a string literal.
    '.header("User-Agent", crate::utils::http_client::APP_USER_AGENT)',
    '.header("User-Agent", crate::utils::http_client::SAFARI_MACOS_USER_AGENT)',
    '.header("User-Agent", SAFARI_MACOS_USER_AGENT)',
    '.header("User-Agent", crate::utils::http_client::browser_user_agent())',
    '.user_agent(APP_USER_AGENT)',
    '.user_agent(SAFARI_MACOS_USER_AGENT)',
    '.user_agent(browser_user_agent())',
    '.user_agent(USER_AGENT)',
    '.user_agent(ua)',
    # The deliberate Apple-impersonation constant's OWN definition —
    # matches neither pattern (not a `.header(`/`.user_agent(` call site).
    'pub const SAFARI_MACOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)";',
    # An unrelated header entirely.
    '.header("Accept", "application/vnd.github.v3+json")',
]


def self_test() -> int:
    """Runs the negative fixture. Prints PASS/FAIL per case and returns a
    process exit code (0 all good, 1 any mismatch)."""
    failures = 0

    for snippet in _SELF_TEST_SHOULD_FLAG:
        found = find_hardcoded_user_agents(snippet)
        if not found:
            print(f"FAIL (should have been flagged, was not): {snippet!r}")
            failures += 1
        else:
            print(f"PASS (correctly flagged): {snippet!r}")

    for snippet in _SELF_TEST_SHOULD_PASS:
        found = find_hardcoded_user_agents(snippet)
        if found:
            print(f"FAIL (should have passed, was flagged): {snippet!r} -> {found}")
            failures += 1
        else:
            print(f"PASS (correctly not flagged): {snippet!r}")

    print()
    if failures:
        print(f"{failures} self-test case(s) failed.")
        return 1
    print("All self-test cases behaved as expected.")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    sys.exit(check())
