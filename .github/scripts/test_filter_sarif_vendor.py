#!/usr/bin/env python3
# Copyright (c) 2024-2026 MeedyaSuite
# Licensed under the MIT License. See LICENSE file in the project root.
"""Self-test for filter_sarif_vendor.py. Run: python3 test_filter_sarif_vendor.py"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from filter_sarif_vendor import filter_report  # noqa: E402


def loc(uri):
    return {"physicalLocation": {"artifactLocation": {"uri": uri}}}


VENDOR = "src-tauri/vendor/rookie/src/browser/chromium.rs"
OURS = "src-tauri/src/main.rs"
CHECKOUT = "/home/runner/work/MeedyaDL/MeedyaDL"
PACKAGES = ["/home/runner/.cargo/registry", "/home/runner/.cargo/git", "/home/runner/.rustup"]

report = {"runs": [{"results": [
    {"ruleId": "vendor-only", "locations": [loc(VENDOR)]},
    {"ruleId": "ours", "locations": [loc(OURS)]},
    {"ruleId": "related-ours", "locations": [loc(VENDOR)], "relatedLocations": [loc(OURS)]},
    {"ruleId": "flow-ours", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [{"location": loc(OURS)}, {"location": loc(VENDOR)}]}]}]},
    {"ruleId": "flow-vendor", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [{"location": loc(VENDOR)}]}]}]},
    {"ruleId": "no-location"},
    {"ruleId": "unreadable-related", "locations": [loc(VENDOR)],
     "relatedLocations": [{"physicalLocation": {"artifactLocation": {"index": 3}}}]},
    {"ruleId": "flow-stack-ours", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [
         {"location": loc(VENDOR), "stack": {"frames": [{"location": loc(OURS)}]}}]}]}]},
    {"ruleId": "stacks-ours", "locations": [loc(VENDOR)],
     "stacks": [{"frames": [{"location": loc(OURS)}]}]},
    {"ruleId": "dot-vendor", "locations": [loc("./" + VENDOR)]},
    # The path ends in a library in the downloaded-packages cache.
    {"ruleId": "flow-outside", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [
         {"location": loc(VENDOR)},
         {"location": loc("file:///home/runner/.cargo/registry/src/aes-0.8/src/lib.rs")}]}]}]},
    # A full address that is inside the checkout is judged by where it points.
    {"ruleId": "abs-ours", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("file://" + CHECKOUT + "/" + OURS)]},
    {"ruleId": "abs-vendor", "locations": [loc("file://" + CHECKOUT + "/" + VENDOR)]},
    {"ruleId": "outside-only", "locations": [loc("/home/runner/.cargo/registry/src/x/lib.rs")]},
    # Full paths that only look different from the checkout path must count
    # as ours, never as third-party (review of fb0e1ac6).
    {"ruleId": "abs-double-slash", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("file:///home/runner/work/MeedyaDL//MeedyaDL/" + OURS)]},
    {"ruleId": "abs-dotdot", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("file:///home/runner/work/MeedyaDL/x/../MeedyaDL/" + OURS)]},
    {"ruleId": "abs-case", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("file:///home/runner/work/meedyadl/MeedyaDL/" + OURS)]},
    {"ruleId": "abs-root", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("file://" + CHECKOUT)]},
    {"ruleId": "abs-elsewhere", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("/tmp/somewhere/lib.rs")]},
    {"ruleId": "pct-dotdot", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("src-tauri/vendor/%2E%2E/src/main.rs")]},
    {"ruleId": "pct-vendor", "locations": [loc("src-tauri/vendor/rookie/src/a%20b.rs")]},
    {"ruleId": "flow-rustup", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [
         {"location": loc("file:///home/runner/.rustup/toolchains/x/lib/core.rs")}]}]}]},
    {"ruleId": "climbs-out", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("src-tauri/vendor/../src/main.rs")]},
    {"ruleId": "other-scheme", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("https://example.com/x.rs")]},
]}, {"results": []}]}

removed = filter_report(report, CHECKOUT, PACKAGES)
kept = [r["ruleId"] for r in report["runs"][0]["results"]]
assert removed == 7, removed
assert kept == ["ours", "related-ours", "flow-ours", "no-location",
                "unreadable-related", "flow-stack-ours", "stacks-ours",
                "abs-ours", "outside-only", "abs-double-slash", "abs-dotdot",
                "abs-case", "abs-root", "abs-elsewhere", "pct-dotdot",
                "climbs-out", "other-scheme"], kept

# The defaults: the checkout is the current folder and the package folders
# come from CARGO_HOME / RUSTUP_HOME.
import os  # noqa: E402
from filter_sarif_vendor import classify, package_folders  # noqa: E402
os.environ["CARGO_HOME"] = "/opt/cargo"
os.environ["RUSTUP_HOME"] = "/opt/rustup"
assert package_folders() == ["/opt/cargo/registry", "/opt/cargo/git", "/opt/rustup"]
assert classify("/opt/cargo/registry/src/aes/lib.rs", "/w") == "outside"
assert classify("/opt/cargo/bin/x.rs", "/w") == "ours"
assert classify("/opt/cargo/git/checkouts/MeedyaSuite-core-1a2b/abc/src/lib.rs", "/w") == "ours"
assert classify("/opt/cargo/git/checkouts/other-1a2b/abc/src/lib.rs", "/w") == "outside"
assert classify("file://" + os.getcwd() + "/" + VENDOR, os.getcwd()) == "vendor"

# An empty folder fails rather than uploading nothing.
import tempfile  # noqa: E402
from filter_sarif_vendor import main  # noqa: E402
with tempfile.TemporaryDirectory() as empty:
    assert main(["x", empty]) == 1
assert report["runs"][1]["results"] == []
print("filter_sarif_vendor self-test passed")
