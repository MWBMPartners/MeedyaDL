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

report = {"runs": [{"results": [
    {"ruleId": "vendor-only", "locations": [loc(VENDOR)]},
    {"ruleId": "ours", "locations": [loc(OURS)]},
    {"ruleId": "related-ours", "locations": [loc(VENDOR)], "relatedLocations": [loc(OURS)]},
    {"ruleId": "flow-ours", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [{"location": loc(OURS)}, {"location": loc(VENDOR)}]}]}]},
    {"ruleId": "flow-vendor", "locations": [loc(VENDOR)],
     "codeFlows": [{"threadFlows": [{"locations": [{"location": loc(VENDOR)}]}]}]},
    {"ruleId": "no-location"},
]}, {"results": []}]}

removed = filter_report(report)
kept = [r["ruleId"] for r in report["runs"][0]["results"]]
assert removed == 2, removed
assert kept == ["ours", "related-ours", "flow-ours", "no-location"], kept
assert report["runs"][1]["results"] == []
print("filter_sarif_vendor self-test passed")
