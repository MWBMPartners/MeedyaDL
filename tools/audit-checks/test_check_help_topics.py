#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""
tools/audit-checks/test_check_help_topics.py
=============================================

Tests for the same-page link check in `check_help_topics.py`: a link such
as `[Setup Guide](#setup-guide)` must point at a heading that exists on
the same page, using the target-name rule GitHub uses.

Pure stdlib, no pytest, same house style as the other checks.
Run directly: `python3 tools/audit-checks/test_check_help_topics.py`
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("check_help_topics", HERE / "check_help_topics.py")
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

failures: list[str] = []


def expect(name: str, got, want) -> None:
    if got != want:
        failures.append(f"{name}: got {got!r}, wanted {want!r}")


# The target-name rule (kept in step with src/lib/headingAnchors.ts).
expect("slug basic", mod.github_slug_base("Setup Guide"), "setup-guide")
expect("slug punctuation", mod.github_slug_base("Step 5: Copy the key"), "step-5-copy-the-key")
expect("slug slash", mod.github_slug_base("Standard M4A/MP4 atoms"), "standard-m4amp4-atoms")

GOOD = "# Title\n\n## Setup Guide\n\nSee [it](#setup-guide).\n"
expect("good link is fine", mod.find_broken_same_page_anchors(GOOD), [])

BAD = "# Title\n\n## Setup Guide\n\nSee [it](#set-up-guide).\n"
expect("broken link is reported", mod.find_broken_same_page_anchors(BAD), [(5, "set-up-guide")])

DUP = "## FAQ\n\n## FAQ\n\n[a](#faq) [b](#faq-1) [c](#faq-2)\n"
expect("repeated heading is numbered", mod.find_broken_same_page_anchors(DUP), [(5, "faq-2")])

FENCED = "## Real\n\n```\n## Not a heading\n[x](#not-a-heading)\n```\n\n[y](#not-a-heading)\n"
expect(
    "fenced code is not a heading or a link",
    mod.find_broken_same_page_anchors(FENCED),
    [(8, "not-a-heading")],
)

STYLED = "## The `Wrapper` **Setup** [guide](x.md)\n\n[a](#the-wrapper-setup-guide)\n"
expect("styled heading text", mod.find_broken_same_page_anchors(STYLED), [])

if failures:
    print("FAIL\n" + "\n".join(failures))
    sys.exit(1)
print("OK — help same-page link check behaves as expected.")
