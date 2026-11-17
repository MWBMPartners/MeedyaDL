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
    {"ruleId": "climbs-out", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("src-tauri/vendor/../src/main.rs")]},
    {"ruleId": "other-scheme", "locations": [loc(VENDOR)],
     "relatedLocations": [loc("https://example.com/x.rs")]},
]}, {"results": []}]}

removed = filter_report(report, CHECKOUT)
kept = [r["ruleId"] for r in report["runs"][0]["results"]]
assert removed == 5, removed
assert kept == ["ours", "related-ours", "flow-ours", "no-location",
                "unreadable-related", "flow-stack-ours", "stacks-ours",
                "abs-ours", "outside-only", "climbs-out", "other-scheme"], kept

# An empty folder fails rather than uploading nothing.
import tempfile  # noqa: E402
from filter_sarif_vendor import main  # noqa: E402
with tempfile.TemporaryDirectory() as empty:
    assert main(["x", empty]) == 1
assert report["runs"][1]["results"] == []
print("filter_sarif_vendor self-test passed")
