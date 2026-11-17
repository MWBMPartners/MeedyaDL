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
For every .sarif file in the folder given, it removes each result that
points into src-tauri/vendor/ and nowhere in our own code. It looks at
every location a result has (main and related locations, every step of its
data-flow paths, and stack frames). A location outside the checkout (the
downloaded-packages cache, where CodeQL reads the encryption libraries the
vendored code calls) counts as third-party too, but only when it is inside a known package
folder (Cargo's downloaded crates or the Rust toolchain): the first run of this
filter removed nothing because each alert's path ended inside such a
library. A result that also points at our own code anywhere is kept, so
nothing in our own code can be hidden this way, and so is a location whose
path cannot be read.
Every other result, and every other part of the report, is left exactly as
it was. It prints how many results it removed from each file.

Usage: filter_sarif_vendor.py <folder with .sarif files> [checkout folder]
(the checkout folder defaults to the current folder)
"""

from __future__ import annotations

import json
import os
import sys
from urllib.parse import unquote, urlparse
from pathlib import Path

# The one folder whose results are dropped. Paths in a CodeQL report are
# relative to the checkout, written with forward slashes.
VENDOR_PREFIX = "src-tauri/vendor/"


def _uri(location: dict) -> str | None:
    """The file path of one SARIF location, if it has one."""
    uri = (
        ((location or {}).get("physicalLocation") or {})
        .get("artifactLocation", {})
        .get("uri")
    )
    return uri if isinstance(uri, str) else None


def result_paths(result: dict) -> list[str]:
    """Every file path a result points at.

    That is its main and related locations, every step of its data-flow
    paths (`codeFlows`: a "hard-coded value" alert is a path alert, and its
    starting point may be in our own code even when the alert itself is
    reported inside the vendor folder), and any stack frames.
    """
    found: list[str | None] = []
    for key in ("locations", "relatedLocations"):
        found += [_uri(loc) for loc in result.get(key) or []]
    for flow in result.get("codeFlows") or []:
        for thread in flow.get("threadFlows") or []:
            for step in thread.get("locations") or []:
                found.append(_uri((step or {}).get("location") or {}))
                for frame in ((step or {}).get("stack") or {}).get("frames") or []:
                    found.append(_uri((frame or {}).get("location") or {}))
    for stack in result.get("stacks") or []:
        for frame in stack.get("frames") or []:
            found.append(_uri((frame or {}).get("location") or {}))
    # A location with no readable path is kept as "" so that it can never
    # count as inside the vendor folder: when in doubt, the result is kept.
    return ["" if p is None else p for p in found]


def package_folders() -> list[str]:
    """The folders that hold downloaded third-party packages on this machine.

    Only a full path under one of these counts as third-party: Cargo's
    downloaded crates (`registry/` and `git/` under CARGO_HOME, default
    ~/.cargo) and the Rust toolchain's own library (RUSTUP_HOME, default
    ~/.rustup). Any other full path is treated as possibly ours.
    """
    home = os.path.expanduser("~")
    cargo = os.environ.get("CARGO_HOME") or os.path.join(home, ".cargo")
    rustup = os.environ.get("RUSTUP_HOME") or os.path.join(home, ".rustup")
    return [
        os.path.join(cargo, "registry"),
        os.path.join(cargo, "git"),
        rustup,
    ]


def _under(path: str, folder: str) -> bool:
    folder = folder.rstrip("/")
    return path == folder or path.startswith(folder + "/")


def classify(path: str, checkout: str, packages: list[str] | None = None) -> str:
    """"vendor", "outside" (a downloaded package) or "ours".

    Anything that cannot be placed with certainty is "ours", so it keeps the
    result: an unreadable path, any ".." anywhere, a full path that is
    neither inside the checkout nor inside a known package folder, or an
    address of a kind this script does not know. Paths are decoded
    (percent-encoding) before they are judged, relative ones included.
    """
    if not path:
        return "ours"
    if "://" in path or path.startswith("file:"):
        parsed = urlparse(path)
        if parsed.scheme != "file" or parsed.netloc not in ("", "localhost"):
            return "ours"
        path = parsed.path
    path = unquote(path).replace("\\", "/")
    parts = path.split("/")
    if ".." in parts:
        return "ours"
    # Collapse doubled slashes and "./" so the comparisons below are exact.
    path = "/".join(part for i, part in enumerate(parts) if part or i == 0)
    path = path.replace("/./", "/").removeprefix("./")
    is_absolute = path.startswith("/") or (len(path) > 2 and path[1] == ":")
    if is_absolute:
        root = checkout.replace("\\", "/")
        if _under(path, root):
            path = path[len(root.rstrip("/")):].lstrip("/")
        elif "/meedyasuite-core" in path.lower():
            # MeedyaSuite-core is our own shared code, even when Cargo keeps
            # it in its git cache, so it is never treated as third-party.
            return "ours"
        elif any(_under(path, f.replace("\\", "/"))
                 for f in (package_folders() if packages is None else packages)):
            return "outside"
        else:
            return "ours"
    if path.startswith(VENDOR_PREFIX):
        return "vendor"
    return "ours"


def is_vendor_only(result: dict, checkout: str = "",
                   packages: list[str] | None = None) -> bool:
    """True when the result points into the vendor folder and never at our code."""
    kinds = [classify(p, checkout or os.getcwd(), packages)
             for p in result_paths(result)]
    # A result with no file location is never ours to drop, and one that
    # only points outside the checkout is not about the vendor folder.
    return "vendor" in kinds and "ours" not in kinds


def filter_report(report: dict, checkout: str = "",
                  packages: list[str] | None = None) -> int:
    """Remove vendor-only results from a parsed report; returns how many.

    Prints the paths of any result whose main location is in the vendor
    folder but which is kept, so a run that removes less than expected
    shows why.
    """
    removed = 0
    for run in report.get("runs") or []:
        results = run.get("results")
        if not isinstance(results, list):
            continue
        kept = []
        for r in results:
            if is_vendor_only(r, checkout, packages):
                continue
            kept.append(r)
            main_paths = [_uri(loc) or "" for loc in r.get("locations") or []]
            if any(classify(p, checkout or os.getcwd(), packages) == "vendor"
                   for p in main_paths):
                print(f"  kept {r.get('ruleId')}: {sorted(set(result_paths(r)))}")
        removed += len(results) - len(kept)
        run["results"] = kept
    return removed


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print("usage: filter_sarif_vendor.py <folder with .sarif files> [checkout folder]")
        return 2
    folder = Path(argv[1])
    checkout = os.path.abspath(argv[2] if len(argv) == 3 else os.getcwd())
    files = sorted(folder.glob("*.sarif"))
    if not files:
        # Fail loudly: an empty folder would mean nothing gets uploaded.
        print(f"No .sarif files found in {folder}")
        return 1
    for path in files:
        report = json.loads(path.read_text(encoding="utf-8"))
        removed = filter_report(report, checkout)
        path.write_text(json.dumps(report), encoding="utf-8")
        print(f"{path.name}: removed {removed} result(s) inside {VENDOR_PREFIX}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
