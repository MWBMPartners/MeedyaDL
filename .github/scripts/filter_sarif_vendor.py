#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""Remove CodeQL results that point into src-tauri/vendor/ before upload.

Why this exists
---------------
src-tauri/vendor/ holds third-party code copied in unchanged apart from a
dependency version (the cookie library "rookie", #1413). It is not our code
to scan: rookie holds the fixed values Chrome itself uses to protect cookies
on Linux and macOS, which only raise "hard-coded cryptographic value"
alerts nobody can act on. Leaving that folder out of the scan was the
maintainer's decision (16 Nov 2026).

The CodeQL workflow already lists the folder under `paths-ignore`, but that
setting is not applied to the Rust scan: CodeQL reads the whole Rust
project, so the four alerts kept coming back and failing the CodeQL check on
every push. Dropping the folder's results from the report before it is
uploaded is the dependable way to leave it out.

What it does
------------
For every .sarif file in the folder given, it removes each result whose
locations ALL point under src-tauri/vendor/. A result that also points at
our own code is kept, so nothing in our own code can be hidden this way.
Every other result, and every other part of the report, is left exactly as
it was. It prints how many results it removed from each file.

Usage: filter_sarif_vendor.py <folder with .sarif files>
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# The one folder whose results are dropped. Paths in a CodeQL report are
# relative to the checkout, written with forward slashes.
VENDOR_PREFIX = "src-tauri/vendor/"


def result_paths(result: dict) -> list[str]:
    """Every file path a result points at (its main and related locations)."""
    paths: list[str] = []
    for key in ("locations", "relatedLocations"):
        for location in result.get(key) or []:
            uri = (
                (location.get("physicalLocation") or {})
                .get("artifactLocation", {})
                .get("uri")
            )
            if isinstance(uri, str):
                paths.append(uri)
    return paths


def is_vendor_only(result: dict) -> bool:
    """True when the result points somewhere, and only into the vendor folder."""
    paths = result_paths(result)
    if not paths:
        # A result with no file location is never ours to drop.
        return False
    return all(p.lstrip("./").startswith(VENDOR_PREFIX) for p in paths)


def filter_report(report: dict) -> int:
    """Remove vendor-only results from a parsed report; returns how many."""
    removed = 0
    for run in report.get("runs") or []:
        results = run.get("results")
        if not isinstance(results, list):
            continue
        kept = [r for r in results if not is_vendor_only(r)]
        removed += len(results) - len(kept)
        run["results"] = kept
    return removed


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: filter_sarif_vendor.py <folder with .sarif files>")
        return 2
    folder = Path(argv[1])
    files = sorted(folder.glob("*.sarif"))
    if not files:
        # Fail loudly: an empty folder would mean nothing gets uploaded.
        print(f"No .sarif files found in {folder}")
        return 1
    for path in files:
        report = json.loads(path.read_text(encoding="utf-8"))
        removed = filter_report(report)
        path.write_text(json.dumps(report), encoding="utf-8")
        print(f"{path.name}: removed {removed} result(s) inside {VENDOR_PREFIX}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
